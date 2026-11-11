//! LES ECARTS DE CALCUL DE L'IMPORT : a la fin de chaque import, et a l'ouverture de la compagnie, chaque paie importee
//! pas encore comparee se compare a ce que ce moteur aurait calcule de la meme saisie (`engagement::ecarts`), et les
//! ecarts partent chez l'editeur, pour voir ou deux logiciels lisent les textes autrement. Les engagements importes
//! restent tels que l'autre logiciel les a ecrits ; rien ne s'ecrit dans la compagnie que la ligne du journal des
//! incidents et le registre des paies comparees (`journal/ecarts-import.json`, leurs empreintes).
//!
//! UN INCIDENT DE FONCTIONNEMENT COMME LES AUTRES : rien ne s'affiche ni ne se demande. Nature `logiciel`, `ou` =
//! `ecarts_import`, un incident par sorte d'ecart (la cotisation, la grandeur, le logiciel d'origine) pour que le
//! serveur les regroupe d'une compagnie a l'autre ; une paie qui ne se compare pas en fait un par raison, et les
//! libelles de cotisation que la table de lecture ne rattache a rien, un par logiciel. Puis l'envoi ordinaire
//! (`incidents::envoyer`), qui ne part que si l'envoi a l'editeur est actif sur ce poste.
//!
//! CE QUI PART NE DESIGNE PERSONNE : ni nom, prenom, NIR, naissance ou adresse, ni salaire, brut, net ou montant d'une
//! paie. Seulement la cotisation et les libelles du bulletin, la grandeur, notre taux et le leur, l'ecart RELATIF d'une
//! base ou d'un montant, le mois, l'IDCC et l'annexe de la convention, l'organisation patronale de la compagnie (elle
//! change le taux de la retraite complementaire), la nature, le statut et la qualite de cadre,
//! d'ou vient le taux AT/MP du calcul (la compagnie ou le bulletin), le logiciel reconnu.
//! Sans le fil ni les ecritures du diagnostic (`Note::sans_contexte`) : leurs charges de routes portent des montants.
//! Le texte passe quand meme par le caviardage (`caviardage`, les fiches de la compagnie connues), et un montant ecrit
//! dans une raison (un refus du calcul) devient `<MONTANT>`.
//!
//! UNE PAIE SE COMPARE UNE FOIS, sauf quand le calcul l'a refusee (une configuration a completer) : elle se compare de
//! nouveau a l'ouverture suivante. UN RAPPORT DEJA NOTE NE SE NOTE PAS DEUX FOIS (meme empreinte, meme resume, meme
//! detail) : un import rejoue ne renvoie rien.

use droit::fiche::Lue as _;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

use crate::caviardage::{self, Connus};
use compagnie::incidents;
use declarations::champs::Saisie;
use droit::contexte::Contexte;
use engagement::ecarts::{Comparaison, Grandeur};
use socle::echec::R;

/// L'endroit, dans le journal des incidents.
pub const OU: &str = "ecarts_import";

/// Un incident a noter : son genre (ce qui le regroupe), son resume, son detail.
#[derive(Debug, Clone, PartialEq)]
pub struct Rapport {
    pub genre: String,
    pub resume: String,
    pub detail: Detail,
}

/// LE DETAIL D'UN RAPPORT : un ecart de calcul (une cotisation, une grandeur, un logiciel, et chaque paie), une paie qui
/// ne se compare pas (sa raison), des lignes de cotisation que la table ne rattache a rien.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Detail {
    Ecart {
        cotisation: String,
        libelles: Vec<String>,
        grandeur: &'static str,
        logiciel: String,
        idcc: Option<String>,
        annexe: Option<String>,
        organisation_patronale: Option<String>,
        cas: Vec<Cas>,
    },
    NonComparee {
        raison: String,
        idcc: Option<String>,
        annexe: Option<String>,
        cas: Vec<Cas>,
    },
    NonRattachees {
        logiciel: String,
        libelles: Vec<String>,
        idcc: Option<String>,
    },
}

/// Une paie d'un rapport : son mois, ce qui la range (le logiciel d'une paie non comparee ; la nature, le statut, la
/// qualite de cadre, d'ou vient le taux AT/MP), et pour un ecart, notre taux, le leur et l'ecart relatif.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Cas {
    mois: socle::mois::Mois,
    #[serde(skip_serializing_if = "Option::is_none")]
    logiciel: Option<String>,
    nature: droit::natures::NatureDeContrat,
    statut: Option<String>,
    cadre: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    taux_atmp: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notre_taux: Option<socle::num::Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    leur_taux: Option<socle::num::Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ecart_relatif: Option<socle::num::Decimal>,
}

fn montant() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| Regex::new(r"\d[\d\u{a0}\u{202f} .]*[,.]\d{2}\b").expect("motif d'un montant"))
}

/// Le caviardage des rapports (`caviardage::caviarder`), sur les noms connus de la compagnie : un meme texte (un mois, une
/// nature, qui reviennent a chaque cas) ne se caviarde qu'une fois.
struct Propre<'a> {
    connus: &'a Connus,
    vus: std::collections::HashMap<String, String>,
}

impl Propre<'_> {
    fn texte(&mut self, texte: &str) -> String {
        if let Some(t) = self.vus.get(texte) {
            return t.clone();
        }
        let t = caviardage::caviarder(texte, self.connus).texte;
        self.vus.insert(texte.to_string(), t.clone());
        t
    }

    fn textes(&mut self, l: &[String]) -> Vec<String> {
        l.iter().map(|x| self.texte(x)).collect()
    }

    fn option(&mut self, x: &Option<String>) -> Option<String> {
        x.as_ref().map(|t| self.texte(t))
    }
}

/// Le registre des paies importees deja comparees (leurs empreintes, `engagement::ecarts::empreinte`).
fn registre(racine: &Path) -> PathBuf {
    racine.join(envois::journal::JOURNAL).join("ecarts-import.json")
}

/// Les empreintes des paies deja comparees ; aucune tant que le registre n'existe pas.
fn comparees(racine: &Path) -> R<BTreeSet<String>> {
    let p = registre(racine);
    if !socle::fichiers::existe(&p) {
        return Ok(BTreeSet::new());
    }
    let lues: Vec<String> = socle::forme::lire_fichier(&p, "Le registre des paies importées comparées ne se lit pas : le signaler au développeur.")?;
    Ok(lues.into_iter().collect())
}

/// Les rapports des paies importees de la compagnie que le registre ne dit pas deja comparees, et les empreintes des
/// paies qui ne sont plus a comparer : comparees, ou que leurs pieces empechent de comparer (une paie que le calcul a
/// refusee se compare de nouveau la fois suivante).
pub fn rapports(ctx: &Contexte) -> R<(Vec<Rapport>, Vec<String>)> {
    let importes = engagement::importe::tous(&ctx.racine)?;
    let deja = comparees(&ctx.racine)?;
    let comparaisons: Vec<Comparaison> = importes.iter().flat_map(|i| engagement::ecarts::comparer(&ctx.racine, i, &|e| deja.contains(e))).collect();
    let faites: Vec<String> = comparaisons.iter().filter(|c| !c.a_refaire).map(|c| c.empreinte.clone()).collect();
    Ok((rapports_de(ctx, &comparaisons)?, faites))
}

fn rapports_de(ctx: &Contexte, comparaisons: &[Comparaison]) -> R<Vec<Rapport>> {
    let e = &ctx.entreprise;
    let (idcc, annexe, organisation) =
        (e.idcc.remplie().map(String::from), e.annexe_convention.dite().map(|a| a.as_str().to_string()), e.organisation_patronale.remplie().map(String::from));
    let mut connus = Connus::default();
    ajouter_les_fiches(ctx, &mut connus)?;
    let mut propre = Propre { connus: &connus, vus: Default::default() };
    // Le logiciel que le lecteur a reconnu au bulletin ; un autre se dit « autre ».
    let logiciel = |c: &Comparaison| match &c.logiciel {
        Some(l) => l.clone(),
        None => "autre".to_string(),
    };
    // Sa forme la plus courte ; « -0 » est 0.
    let nombre = |t: Option<socle::num::Decimal>| t.map(|d| if d.is_zero() { socle::num::Decimal::ZERO } else { d.normalize() });
    let cas = |c: &Comparaison, p: &mut Propre| Cas {
        mois: c.mois,
        logiciel: None,
        nature: c.nature,
        statut: p.option(&c.statut),
        cadre: c.cadre,
        taux_atmp: c.taux_atmp,
        notre_taux: None,
        leur_taux: None,
        ecart_relatif: None,
    };

    let mut ecarts: BTreeMap<(String, Grandeur, String), (Vec<String>, Vec<Cas>)> = BTreeMap::new();
    let mut refus: BTreeMap<String, Vec<Cas>> = BTreeMap::new();
    let mut seules: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in comparaisons {
        match &c.ecarts {
            Err(raison) => {
                let raison = montant().replace_all(&propre.texte(&raison.phrase), "<MONTANT>").into_owned();
                let x = Cas { logiciel: propre.option(&c.logiciel), taux_atmp: None, ..cas(c, &mut propre) };
                refus.entry(raison).or_default().push(x);
            }
            Ok(liste) => {
                for e in liste {
                    let x = Cas { notre_taux: nombre(e.notre_taux), leur_taux: nombre(e.leur_taux), ecart_relatif: nombre(e.ecart_relatif), ..cas(c, &mut propre) };
                    let (libelles, v) = ecarts.entry((e.cotisation.clone(), e.grandeur, logiciel(c))).or_default();
                    for l in &e.libelles {
                        if !libelles.contains(l) {
                            libelles.push(l.clone());
                        }
                    }
                    v.push(x);
                }
            }
        }
        for l in &c.non_rattachees {
            let v = seules.entry(logiciel(c)).or_default();
            if !v.contains(l) {
                v.push(l.clone());
            }
        }
    }
    let mut sortie = Vec::new();
    for ((cotisation, grandeur, logiciel), (libelles, cas)) in ecarts {
        let resume = format!(
            "Écart de calcul à l'import : {} ({}), {}, bulletin {} ; {} paie(s).",
            cotisation,
            if libelles.is_empty() { "total du bulletin".to_string() } else { libelles.join(" ; ") },
            grandeur.dite(),
            logiciel,
            cas.len()
        );
        let genre = format!("{} · {} · {}", cotisation, grandeur.code(), logiciel);
        let detail = Detail::Ecart {
            cotisation: propre.texte(&cotisation),
            libelles: propre.textes(&libelles),
            grandeur: grandeur.code(),
            logiciel: propre.texte(&logiciel),
            idcc: idcc.clone(),
            annexe: annexe.clone(),
            organisation_patronale: propre.option(&organisation),
            cas,
        };
        sortie.push(Rapport { genre: propre.texte(&genre), resume: propre.texte(&resume), detail });
    }
    for (raison, cas) in refus {
        let resume = format!("Paie importée non comparée : {} ; {} paie(s).", raison, cas.len());
        let genre: String = format!("non comparée : {}", raison).chars().take(120).collect();
        let detail = Detail::NonComparee { raison: propre.texte(&raison), idcc: idcc.clone(), annexe: annexe.clone(), cas };
        sortie.push(Rapport { genre: propre.texte(&genre), resume: propre.texte(&resume), detail });
    }
    for (logiciel, libelles) in seules {
        let resume = format!("Lignes de cotisation non comparées (bulletin {}) : la table de lecture ne les rattache à aucune cotisation : {}.", logiciel, libelles.join(" ; "));
        let genre = format!("cotisation non rattachée · {}", logiciel);
        let detail = Detail::NonRattachees { logiciel: propre.texte(&logiciel), libelles: propre.textes(&libelles), idcc: idcc.clone() };
        sortie.push(Rapport { genre: propre.texte(&genre), resume: propre.texte(&resume), detail });
    }
    Ok(sortie)
}

/// A la fin d'un import : les rapports notes, puis l'envoi ordinaire. Ne dit rien a l'ecran : rend le nombre de rapports
/// notes.
pub fn rapporter(ctx: &Contexte) -> R<usize> {
    let notes = noter(ctx)?;
    if notes > 0 {
        incidents::envoyer(&ctx.racine);
    }
    Ok(notes)
}

/// Les paies importees pas encore comparees se comparent, leurs rapports se notent au journal des incidents (sauf ceux
/// qui y sont deja), et le registre retient celles qui ne sont plus a comparer. A l'ouverture de la compagnie comme a
/// la fin d'un import : une paie qu'une configuration incomplete empechait de calculer (l'effectif de l'entreprise...)
/// se compare des que la compagnie est complete. Rend le nombre de rapports notes ; un incident qui ne s'ecrit pas ne se
/// compte pas.
pub fn noter(ctx: &Contexte) -> R<usize> {
    let racine = &ctx.racine;
    if engagement::importe::tous(racine)?.is_empty() {
        return Ok(0);
    }
    let (rapports, faites) = rapports(ctx)?;
    let deja = incidents::lire(racine, Some(incidents::LOGICIEL), false)?;
    let mut notes = 0;
    for r in rapports {
        // Ce qui reconnait un rapport deja note : le journal tronque les longs textes et ne relit pas un detail, le
        // rapport se reconnait a la signature de ce qu'il dit en entier, que son genre porte (et donc son empreinte).
        let signature = socle::fichiers::sha256_hex(format!("{}|{}", r.resume, socle::ecriture::json_en_ligne(&r.detail)).as_bytes())[..16].to_string();
        let genre = format!("{} · {}", r.genre, signature);
        let empreinte = incidents::empreinte(incidents::LOGICIEL, OU, &genre);
        if deja.iter().any(|l| l.empreinte == empreinte) {
            continue;
        }
        let note = incidents::Note { nature: incidents::LOGICIEL, resume: &r.resume, ou: OU, quoi: &genre, detail: &r.detail, trace: None, sans_contexte: true };
        match incidents::noter(racine, &note) {
            Ok(Some(_)) => notes += 1,
            Ok(None) => {}
            Err(e) => eprintln!("Un écart d'import ne se note pas au journal des incidents : {}", e.phrase),
        }
    }
    if !faites.is_empty() {
        let mut toutes = comparees(racine)?;
        toutes.extend(faites);
        socle::ecriture::ecrire_json(&registre(racine), &toutes, 0)?;
    }
    Ok(notes)
}

/// Ce que les fiches de la compagnie disent de ses salaries, ajoute aux valeurs connues ; la compagnie, protegee.
fn ajouter_les_fiches(ctx: &Contexte, c: &mut Connus) -> R<()> {
    let pousser = |v: &mut Vec<String>, x: Option<String>| {
        if let Some(x) = x.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
            if !v.contains(&x) {
                v.push(x);
            }
        }
    };
    for (_, f) in &ctx.salaries {
        // Les cases que le releve compare, par la table de la fiche.
        let s = Saisie::de_la_fiche(droit::fiche::NomDeFiche::Salarie, &f.champs)?;
        use droit::fiche::CaseSalarie as S;
        let t = |k: S| s.mot(k);
        for k in [S::Nom, S::NomAffichage] {
            pousser(&mut c.noms, t(k));
        }
        pousser(&mut c.noms, t(S::CodeSalarie).filter(|x| x.chars().filter(|c| c.is_alphabetic()).count() >= 2));
        for k in [S::Prenom, S::PrenomAffichage] {
            pousser(&mut c.prenoms, t(k));
        }
        pousser(&mut c.nirs, t(S::Nir));
        if let (Some(n), Some(k)) = (t(S::Nir), t(S::NirCle)) {
            pousser(&mut c.nirs, Some(format!("{}{}", n, k)));
        }
        pousser(&mut c.naissances, t(S::DateNaissance));
        for k in [S::Adresse, S::AdresseAffichage] {
            pousser(&mut c.adresses, t(k));
        }
        pousser(&mut c.codes_postaux, t(S::CodePostal));
        for k in [S::Commune, S::CommuneAffichage, S::LieuNaissance, S::LieuNaissanceAffichage] {
            pousser(&mut c.communes, t(k));
        }
        pousser(&mut c.courriels, t(S::Email));
        pousser(&mut c.conges_spectacles, t(S::NumCongesSpectacles));
    }
    let e = &ctx.entreprise;
    for k in [e.raison_sociale.remplie(), e.nom_affichage.remplie(), e.adresse.remplie(), e.adresse_affichage.remplie(), e.complement_adresse.remplie(), e.complement_adresse_affichage.remplie(), e.code_postal.remplie(), e.commune.remplie(), e.commune_affichage.remplie()] {
        pousser(&mut c.entreprise, k.map(String::from));
    }
    Ok(())
}
