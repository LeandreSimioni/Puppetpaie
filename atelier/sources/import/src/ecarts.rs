//! LES ECARTS DE CALCUL DE L'IMPORT : a la fin de chaque import, et a l'ouverture de la compagnie, chaque paie importee
//! pas encore comparee se compare a ce que ce moteur aurait calcule de la meme saisie (`engagement::ecarts`), et les
//! ecarts partent chez l'editeur, pour voir ou deux logiciels lisent les textes autrement. Les engagements importes
//! restent tels que l'autre logiciel les a ecrits ; rien ne s'ecrit dans la compagnie que la ligne du journal des
//! incidents et le registre des paies comparees (`journal/ecarts-import.json`, leurs empreintes).
//!
//! UN INCIDENT DE FONCTIONNEMENT COMME LES AUTRES : rien ne s'affiche ni ne se demande. Nature `logiciel`, `ou` =
//! `ecarts_import`, un incident par sorte d'ecart (la cotisation, la grandeur, le logiciel d'origine) ; une paie qui ne
//! se compare pas en fait un par raison, et les libelles de cotisation que la table de lecture ne rattache a rien, un
//! par logiciel. LE GENRE NOMME LA CAUSE, JAMAIS LE CAS : des codes du logiciel (la cotisation, la grandeur et le
//! logiciel d'un ecart ; la sorte de raison d'une paie non comparee, `engagement::ecarts::SorteDeNonComparaison` ; le
//! logiciel des lignes non rattachees), sans aucune valeur de la compagnie, pour que le serveur range sous un meme ticket
//! une cause, d'une compagnie a l'autre ; ce qui distingue deux rapports d'une cause est dans leur detail, signe
//! (`noter`). Puis l'envoi ordinaire
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
//! UNE PAIE SE COMPARE UNE FOIS, sauf quand le calcul l'a refusee (une configuration a completer) ou que le logiciel ne
//! livre pas de quoi la calculer : elle se compare de nouveau a l'ouverture suivante. Une paie d'avant
//! `engagement::ecarts::COMPAREE_TOUJOURS_DES` que le logiciel ne sait pas calculer n'est pas une comparaison : ni
//! incident, ni registre. UN RAPPORT DEJA NOTE NE SE NOTE PAS DEUX FOIS (meme resume, meme detail : sa
//! signature, que garde `journal/ecarts-import-notes.json`) : un import rejoue ne renvoie rien.

use droit::fiche::Lue as _;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

use crate::caviardage::{self, Connus};
use compagnie::incidents;
use droit::contexte::Contexte;
use droit::fiche::AnnexeConvention;
use engagement::ecarts::{CeQuiDiffere, Comparaison, MesureDeLEcart, OrigineDuTauxAtmp, RegroupementDeComparaison, SorteDeNonComparaison};
use engagement::importe::LogicielDUnBulletin;
use socle::echec::R;
use socle::fichiers::{Empreinte, EmpreinteCourte};

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
        cotisation: RegroupementDeComparaison,
        libelles: Vec<String>,
        grandeur: &'static str,
        logiciel: LogicielDuRapport,
        idcc: Option<droit::fiche::Idcc>,
        annexe: Option<AnnexeConvention>,
        /// L'organisation patronale de la compagnie, caviardee : un texte du rapport, plus une organisation.
        organisation_patronale: Option<String>,
        cas: Vec<Cas>,
    },
    NonComparee {
        sorte: SorteDeNonComparaison,
        raison: String,
        idcc: Option<droit::fiche::Idcc>,
        annexe: Option<AnnexeConvention>,
        cas: Vec<Cas>,
    },
    NonRattachees {
        logiciel: LogicielDuRapport,
        libelles: Vec<String>,
        idcc: Option<droit::fiche::Idcc>,
    },
}

/// LE LOGICIEL QU'UN RAPPORT NOMME : celui que le lecteur a reconnu au bulletin, sinon « autre ». Il s'ecrit et se trie
/// comme ce texte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicielDuRapport(Option<LogicielDUnBulletin>);

impl LogicielDuRapport {
    fn dit(&self) -> &str {
        match &self.0 {
            Some(l) => l.as_str(),
            None => "autre",
        }
    }
}

impl Ord for LogicielDuRapport {
    fn cmp(&self, autre: &Self) -> std::cmp::Ordering {
        self.dit().cmp(autre.dit())
    }
}

impl PartialOrd for LogicielDuRapport {
    fn partial_cmp(&self, autre: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(autre))
    }
}

impl std::fmt::Display for LogicielDuRapport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.dit())
    }
}

impl Serialize for LogicielDuRapport {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.dit())
    }
}

/// Une paie d'un rapport : son mois, ce qui la range (le logiciel d'une paie non comparee ; la nature, le statut, la
/// qualite de cadre, d'ou vient le taux AT/MP), et pour un ecart, notre taux, le leur et l'ecart relatif.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Cas {
    mois: socle::mois::Mois,
    #[serde(skip_serializing_if = "Option::is_none")]
    logiciel: Option<LogicielDUnBulletin>,
    nature: droit::natures::NatureDeContrat,
    statut: Option<droit::fiche::Statut>,
    cadre: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    taux_atmp: Option<OrigineDuTauxAtmp>,
    /// Ecrits dans leur forme la plus courte ; « -0 » est 0 (`ecrit::sans_zero`).
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::grandeurs::ecrit::sans_zero::option::serialize")]
    notre_taux: Option<socle::grandeurs::Taux>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::grandeurs::ecrit::sans_zero::option::serialize")]
    leur_taux: Option<socle::grandeurs::Taux>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::grandeurs::ecrit::sans_zero::option::serialize")]
    ecart_relatif: Option<socle::grandeurs::Coefficient>,
    /// Les annees dont le calcul a pris l'effectif que les paies reprises proposent (`Comparaison::effectif_propose`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    effectif_propose: Vec<socle::mois::Annee>,
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

    /// L'organisation patronale, caviardee comme tout texte du rapport : le texte caviarde n'est pas une organisation, il
    /// reste texte.
    fn organisation(&mut self, x: Option<&droit::forme_bareme::OrganisationPatronale>) -> Option<String> {
        x.map(|o| self.texte(o.as_str()))
    }
}

/// Le registre des paies importees deja comparees (leurs empreintes, `engagement::ecarts::empreinte`).
fn registre(racine: &Path) -> PathBuf {
    racine.join(envois::journal::JOURNAL).join("ecarts-import.json")
}

/// Les empreintes des paies deja comparees ; aucune tant que le registre n'existe pas.
fn comparees(racine: &Path) -> R<BTreeSet<EmpreinteCourte>> {
    let p = registre(racine);
    if !socle::fichiers::existe(&p) {
        return Ok(BTreeSet::new());
    }
    let lues: Vec<EmpreinteCourte> =socle::forme::lire_fichier(&p, "Le registre des paies importées comparées ne se lit pas : le signaler au développeur.")?;
    Ok(lues.into_iter().collect())
}

/// Le registre des rapports deja notes au journal des incidents (leurs signatures : `noter`).
fn registre_des_notes(racine: &Path) -> PathBuf {
    racine.join(envois::journal::JOURNAL).join("ecarts-import-notes.json")
}

/// Les signatures des rapports deja notes ; aucune tant que leur registre n'existe pas.
fn signatures_notees(racine: &Path) -> R<BTreeSet<EmpreinteCourte>> {
    let p = registre_des_notes(racine);
    if !socle::fichiers::existe(&p) {
        return Ok(BTreeSet::new());
    }
    let lues: Vec<EmpreinteCourte> = socle::forme::lire_fichier(&p, "Le registre des écarts d'import notés ne se lit pas : le signaler au développeur.")?;
    Ok(lues.into_iter().collect())
}

/// Le detail d'un rapport tel qu'il se note : sa signature (ce qui le distingue d'un autre rapport de la meme cause), et
/// ce qu'il dit.
#[derive(Serialize)]
struct DetailSigne<'a> {
    signature: EmpreinteCourte,
    #[serde(flatten)]
    detail: &'a Detail,
}

/// UN ECART CONNU NOTE SUR UNE PAIE IMPORTEE (`engagement::ecarts_connus::EcartConnuConstate`), une ligne de
/// `journal/ecarts-connus.json` : la paie (son empreinte, `engagement::ecarts::empreinte`), ce que la ligne connue dit
/// et les deux montants de sa part, le notre (au taux du texte) et celui du bulletin. Ce que la carte de la paie importee
/// explique a la compagnie (`mois`) ; une paie se compare une fois, ce qu'elle a constate se garde.
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EcartConnuNote {
    pub paie: EmpreinteCourte,
    pub logiciel: LogicielDUnBulletin,
    pub cotisation: RegroupementDeComparaison,
    pub grandeur: engagement::ecarts_connus::PartDuTaux,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub applique: socle::grandeurs::Taux,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub texte: socle::grandeurs::Taux,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub notre: socle::grandeurs::Montant,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub leur: socle::grandeurs::Montant,
    pub explication: String,
}

impl EcartConnuNote {
    /// Ce que la carte de la paie importee en dit : l'explication, et l'ecart chiffre de la part.
    pub fn phrase(&self) -> String {
        let part = match self.grandeur {
            engagement::ecarts_connus::PartDuTaux::Employeur => "part employeur",
            engagement::ecarts_connus::PartDuTaux::Salarie => "part salariale",
        };
        format!(
            "{} Sur ce bulletin ({}) : {} au lieu de {}, soit {} d'écart.",
            self.explication,
            part,
            socle::texte::euros(self.leur),
            socle::texte::euros(self.notre),
            socle::texte::euros((self.leur - self.notre).abs())
        )
    }
}

/// Le registre des ecarts connus notes sur les paies importees.
fn registre_des_connus(racine: &Path) -> PathBuf {
    racine.join(envois::journal::JOURNAL).join("ecarts-connus.json")
}

/// LES ECARTS CONNUS NOTES SUR LES PAIES IMPORTEES (`EcartConnuNote`), la seule lecture ; aucun tant que leur registre
/// n'existe pas.
pub fn ecarts_connus_notes(racine: &Path) -> R<Vec<EcartConnuNote>> {
    let p = registre_des_connus(racine);
    if !socle::fichiers::existe(&p) {
        return Ok(Vec::new());
    }
    socle::forme::lire_fichier(&p, "Le registre des écarts connus des paies importées ne se lit pas : le signaler au développeur.")
}

/// UN REGISTRE DES ECARTS D'IMPORT QUI NE SE LIT PAS (`journal/ecarts-connus.json`, `journal/ecarts-import-notes.json`) :
/// il ne ferme rien (« Le logiciel est doux ») ; l'editeur le sait par un incident de fonctionnement, dont le genre nomme
/// le registre et rien de la compagnie. Un incident qui ne se note pas se dit sur la sortie d'erreur.
pub fn noter_registre_illisible(racine: &Path, registre: &str, e: &socle::echec::Echec) {
    #[derive(Serialize)]
    struct Illisible<'a> {
        registre: &'a str,
        raison: &'a str,
    }
    let quoi = format!("registre illisible · {}", registre);
    let resume = format!("Le registre {} ne se lit pas : {}", registre, e.phrase);
    let note = incidents::Note { nature: incidents::LOGICIEL, resume: &resume, ou: OU, quoi: &quoi, detail: Illisible { registre, raison: &e.phrase }, trace: None, sans_contexte: true };
    if let Err(x) = incidents::noter(racine, &note) {
        eprintln!("Un registre des écarts d'import illisible ne se note pas au journal des incidents : {}", x.phrase);
    }
}

/// Le nom du registre des ecarts connus, tel que l'incident qui le dit illisible le nomme.
pub const REGISTRE_DES_CONNUS: &str = "journal/ecarts-connus.json";
/// Le nom du registre des rapports notes, de meme.
pub const REGISTRE_DES_NOTES: &str = "journal/ecarts-import-notes.json";

/// Ce qu'une revue des paies importees rend : les rapports, les empreintes des paies qui ne sont plus a comparer, et les
/// ecarts connus que les paies comparees ont constates.
pub struct Revue {
    pub rapports: Vec<Rapport>,
    pub faites: Vec<EmpreinteCourte>,
    pub connus: Vec<EcartConnuNote>,
}

/// Les rapports des paies importees de la compagnie que le registre ne dit pas deja comparees, les empreintes des
/// paies qui ne sont plus a comparer : comparees, ou que leurs pieces empechent de comparer (une paie que le calcul a
/// refusee se compare de nouveau la fois suivante), et les ecarts connus constates sur celles-ci.
pub fn rapports(ctx: &Contexte) -> R<Revue> {
    let importes = engagement::importe::tous(&ctx.racine)?;
    let deja = comparees(&ctx.racine)?;
    let comparaisons: Vec<Comparaison> = importes.iter().flat_map(|i| engagement::ecarts::comparer(&ctx.racine, i, &|e| deja.contains(e))).collect();
    let faites: Vec<EmpreinteCourte> = comparaisons.iter().filter(|c| !c.a_refaire).map(|c| c.empreinte.clone()).collect();
    let connus = comparaisons
        .iter()
        .filter(|c| !c.a_refaire)
        .flat_map(|c| {
            c.connus.iter().map(|k| EcartConnuNote {
                paie: c.empreinte.clone(),
                logiciel: k.connu.logiciel.clone(),
                cotisation: k.connu.cotisation.clone(),
                grandeur: k.connu.grandeur,
                applique: k.connu.applique,
                texte: k.connu.texte,
                notre: k.notre,
                leur: k.leur,
                explication: k.connu.explication.clone(),
            })
        })
        .collect();
    Ok(Revue { rapports: rapports_de(ctx, &comparaisons)?, faites, connus })
}

fn rapports_de(ctx: &Contexte, comparaisons: &[Comparaison]) -> R<Vec<Rapport>> {
    let e = &ctx.entreprise;
    let (idcc, annexe, organisation) =
        (e.idcc.dite().cloned(), e.annexe_convention.dite().copied(), e.organisation_patronale.dite().cloned());
    let mut connus = Connus::default();
    ajouter_les_fiches(ctx, &mut connus)?;
    let mut propre = Propre { connus: &connus, vus: Default::default() };
    // Le logiciel que le lecteur a reconnu au bulletin ; un autre se dit « autre ».
    let logiciel = |c: &Comparaison| LogicielDuRapport(c.logiciel.clone());
    let cas = |c: &Comparaison| Cas {
        mois: c.mois,
        logiciel: None,
        nature: c.nature,
        statut: c.statut,
        cadre: c.cadre,
        taux_atmp: c.taux_atmp,
        notre_taux: None,
        leur_taux: None,
        ecart_relatif: None,
        effectif_propose: c.effectif_propose.clone(),
    };

    let mut ecarts: BTreeMap<(RegroupementDeComparaison, CeQuiDiffere, LogicielDuRapport), (Vec<String>, Vec<Cas>)> = BTreeMap::new();
    let mut refus: BTreeMap<(SorteDeNonComparaison, String), Vec<Cas>> = BTreeMap::new();
    let mut seules: BTreeMap<LogicielDuRapport, Vec<String>> = BTreeMap::new();
    for c in comparaisons {
        match &c.ecarts {
            Err(non) => {
                let raison = montant().replace_all(&propre.texte(&non.raison.phrase), "<MONTANT>").into_owned();
                let x = Cas { logiciel: c.logiciel.clone(), taux_atmp: None, ..cas(c) };
                refus.entry((non.sorte, raison)).or_default().push(x);
            }
            Ok(liste) => {
                for e in liste {
                    // Ce que l'ecart mesure, ecrit dans le rapport.
                    let (notre_taux, leur_taux, ecart_relatif) = match e.mesure {
                        MesureDeLEcart::TauxSalarie { notre, leur } | MesureDeLEcart::TauxEmployeur { notre, leur } => (Some(notre), Some(leur), None),
                        MesureDeLEcart::Base(r) | MesureDeLEcart::MontantSalarie(r) | MesureDeLEcart::MontantEmployeur(r) => (None, None, r),
                        MesureDeLEcart::AbsenteDuBulletin | MesureDeLEcart::AbsenteDuCalcul => (None, None, None),
                    };
                    let x = Cas { notre_taux, leur_taux, ecart_relatif, ..cas(c) };
                    let (libelles, v) = ecarts.entry((e.cotisation.clone(), e.mesure.ce_qui_differe(), logiciel(c))).or_default();
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
        // CE QUE LE RESUME DIT SE LIT EN MOTS (il se montre a l'ecran, `incidents-lire`) : les libelles que le bulletin
        // imprime, ou le nom du total ; la cle de la cotisation reste dans le genre et le detail.
        let nommee = match (&cotisation, libelles.is_empty()) {
            (RegroupementDeComparaison::Total(t), _) => t.dit().to_string(),
            (_, false) => libelles.join(" ; "),
            (RegroupementDeComparaison::Autres, true) => "les autres contributions dues par l'employeur".to_string(),
            (RegroupementDeComparaison::Convention, true) => "les cotisations de la convention collective".to_string(),
            (RegroupementDeComparaison::Apec, true) => "l'Apec".to_string(),
            (RegroupementDeComparaison::Cotisation(_), true) => "une cotisation que le bulletin ne nomme pas".to_string(),
        };
        let resume = format!(
            "Écart de calcul à l'import : {}, {}, bulletin {} ; {} paie(s).",
            nommee,
            grandeur.dite(),
            logiciel,
            cas.len()
        );
        let genre = genre_d_un_ecart(&cotisation, grandeur, &logiciel);
        let detail = Detail::Ecart {
            cotisation,
            libelles: propre.textes(&libelles),
            grandeur: grandeur.code(),
            logiciel,
            idcc: idcc.clone(),
            annexe,
            organisation_patronale: propre.organisation(organisation.as_ref()),
            cas,
        };
        sortie.push(Rapport { genre, resume: propre.texte(&resume), detail });
    }
    for ((sorte, raison), cas) in refus {
        let resume = format!("Paie importée non comparée : {} ; {} paie(s).", raison, cas.len());
        let genre = format!("non comparée · {}", sorte.code());
        let detail = Detail::NonComparee { sorte, raison: propre.texte(&raison), idcc: idcc.clone(), annexe, cas };
        sortie.push(Rapport { genre, resume: propre.texte(&resume), detail });
    }
    for (logiciel, libelles) in seules {
        let resume = format!("Lignes de cotisation non comparées (bulletin {}) : la table de lecture ne les rattache à aucune cotisation : {}.", logiciel, libelles.join(" ; "));
        let genre = format!("cotisation non rattachée · {}", logiciel);
        let detail = Detail::NonRattachees { logiciel, libelles: propre.textes(&libelles), idcc: idcc.clone() };
        sortie.push(Rapport { genre, resume: propre.texte(&resume), detail });
    }
    Ok(sortie)
}

/// LE GENRE D'UN ECART, sa cause et rien de son cas : la cotisation (du bareme, ou le regroupement que le modele du
/// bulletin imprime), ce qui differe, le logiciel reconnu (une cle de la table de lecture, ou « autre »).
fn genre_d_un_ecart(cotisation: &RegroupementDeComparaison, grandeur: CeQuiDiffere, logiciel: &LogicielDuRapport) -> String {
    format!("{} · {} · {}", cotisation, grandeur.code(), logiciel)
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
    let Revue { rapports, mut faites, connus } = rapports(ctx)?;
    // UN REGISTRE DES RAPPORTS NOTES ILLISIBLE ne ferme pas la comparaison : les rapports se notent (le journal des
    // incidents compte ses doublons), le registre illisible reste tel quel pour qu'on le lise, et un incident le dit.
    let lisible = match signatures_notees(racine) {
        Ok(s) => Some(s),
        Err(e) => {
            noter_registre_illisible(racine, REGISTRE_DES_NOTES, &e);
            None
        }
    };
    let mut signees = lisible.clone().unwrap_or_default();
    let avant = signees.len();
    let mut notes = 0;
    for r in rapports {
        // CE QUI RECONNAIT UN RAPPORT DEJA NOTE : la signature de ce qu'il dit en entier (son resume et son detail), que
        // le registre des rapports notes garde ; le genre ne dit que la cause, pour que le serveur regroupe sous un meme
        // ticket tous les rapports d'une cause, d'une compagnie a l'autre.
        let signature = Empreinte::des_octets(format!("{}|{}", r.resume, socle::ecriture::json_en_ligne(&r.detail)).as_bytes()).courte();
        if signees.contains(&signature) {
            continue;
        }
        let detail = DetailSigne { signature: signature.clone(), detail: &r.detail };
        let note = incidents::Note { nature: incidents::LOGICIEL, resume: &r.resume, ou: OU, quoi: &r.genre, detail: &detail, trace: None, sans_contexte: true };
        match incidents::noter(racine, &note) {
            Ok(Some(_)) => {
                notes += 1;
                signees.insert(signature);
            }
            Ok(None) => {}
            Err(e) => eprintln!("Un écart d'import ne se note pas au journal des incidents : {}", e.phrase),
        }
    }
    if lisible.is_some() && signees.len() > avant {
        socle::ecriture::ecrire_json(&registre_des_notes(racine), &signees, 0)?;
    }
    // Les ecarts connus constates, avant le registre qui dit leurs paies comparees : une paie qui les a constates ne se
    // compare plus, ils se gardent. Leur registre illisible ne se reecrit pas (ce qu'il garde se perdrait) : ces paies-la
    // restent a comparer, et un incident le dit.
    if !connus.is_empty() {
        match ecarts_connus_notes(racine) {
            Ok(mut notes_connus) => {
                for k in connus {
                    if !notes_connus.contains(&k) {
                        notes_connus.push(k);
                    }
                }
                socle::ecriture::ecrire_json(&registre_des_connus(racine), &notes_connus, 2)?;
            }
            Err(e) => {
                noter_registre_illisible(racine, REGISTRE_DES_CONNUS, &e);
                faites.retain(|f| !connus.iter().any(|k| k.paie == *f));
            }
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
        // Les cases que le releve compare, telles que la fiche les ecrit.
        use droit::fiche::{CaseSalarie as S, FicheSaisie as _};
        let t = |k: S| f.champs.ecrite(k.into()).mot().map(str::to_string);
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
        // Le numero est la partie `numero` de sa case (`droit::fiche::NumeroCongesSpectacles`).
        pousser(&mut c.conges_spectacles, f.champs.ecrite_au_chemin(S::NumCongesSpectacles.into(), &["numero"]).mot().map(str::to_string));
    }
    let e = &ctx.entreprise;
    for k in [e.raison_sociale.remplie(), e.nom_affichage.remplie(), e.adresse.remplie(), e.adresse_affichage.remplie(), e.complement_adresse.remplie(), e.complement_adresse_affichage.remplie(), e.code_postal.remplie(), e.commune.remplie(), e.commune_affichage.remplie()] {
        pousser(&mut c.entreprise, k.map(String::from));
    }
    Ok(())
}
