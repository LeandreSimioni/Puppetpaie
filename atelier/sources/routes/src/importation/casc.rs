//! LA REGULARISATION DE LA CASC-SVP DES MOIS IMPORTES : le dossier que prepare `exporter regularisation-casc-svp`, et
//! `casc-regularise`, qui note qu'une annee est regularisee. Un mois tenu dans un autre logiciel a pu partir sans la
//! CASC-SVP ; PuppetPaie la declare lui-meme dans les DSN qu'il fait (`declarations::dsn`).
//!
//! Tout se lit dans les engagements importes (`engagement::importe`) : les paies versees (le brut que la DSN deposee ou
//! le bulletin dit), la fiche du salarie, le contrat atteste (numero, bornes, cachets, heures et jours). Un mois dont la
//! DSN importee porte deja la CASC-SVP (`importation::ecrite::lire_dsn_casc`), ou deja regularise
//! (`config/casc-svp.json`), n'y entre pas.
//!
//! Le classeur du CASC-SVP se recree avec les colonnes du modele (`donnees/modeles/casc-svp-liste-nominative.xlsx`) :
//! voir `liste_nominative_xlsx`.

use droit::fiche::Lue as _;
use socle::echec::Classe;
use std::path::Path;

use rust_decimal::RoundingStrategy;
use serde::Serialize;
use std::collections::BTreeMap;

use declarations::champs::Saisie;
use droit::contexte::Contexte;
use socle::ecriture::ecrire_json;
use socle::echec::R;
use socle::echec::erreur;
use socle::mois::Mois;
use socle::num::Decimal;
use socle::texte::{euros, virgule};

#[track_caller]
fn refus(m: impl Into<String>) -> socle::echec::Echec {
    erreur(Classe::Refuse, m)
}

fn jjmmaaaa(iso: &str) -> String {
    if iso.is_empty() {
        return String::new();
    }
    let c: Vec<char> = iso.chars().collect();
    let t = |a: usize, b: usize| -> String { c.iter().skip(a).take(b - a).collect() };
    format!("{}/{}/{}", t(8, 10), t(5, 7), t(0, 4))
}

/// Ce qui a ete verse a un salarie un mois importe, tel que l'engagement importe le porte.
struct Versee {
    salarie: droit::fiche::FicheSalarie,
    brut: Decimal,
    numero: Option<String>,
    debut: Option<String>,
    fin: Option<String>,
    /// Les cachets, heures et jours que le contrat atteste, quand il tient dans le mois.
    activite: BTreeMap<&'static str, Decimal>,
}

/// Les paies versees de `mois`, d'apres les engagements importes : une par engagement qui a un brut ce mois-la. Un
/// salarie sans fiche refuse (sa ligne de la liste nominative ne se devine pas).
fn versees_du_mois(ctx: &Contexte, mois: Mois) -> R<Vec<Versee>> {
    let mut out = Vec::new();
    let du_mois = |d: &Option<String>| d.as_deref().and_then(|d| d.get(..7)).and_then(Mois::du_texte) == Some(mois);
    for i in engagement::importe::du_mois(&ctx.racine, mois)? {
        let Some(p) = i.paies_lues()?.into_iter().find(|p| p.mois == mois) else { continue };
        let Some(brut) = p.paie.brut else { continue };
        let cle = p.salarie.clone();
        let salarie = ctx.salaries.get(cle.as_str()).cloned().ok_or_else(|| refus(format!("{} : la fiche du salarie « {} » d'un engagement importe n'existe pas.", mois, cle)))?;
        let (debut, fin) = (p.paie.debut.map(|j| j.iso()), p.paie.fin.map(|j| j.iso()));
        let mut activite = BTreeMap::new();
        if let Some(c) = i.contrat_lu()? {
            if du_mois(&c.debut) && du_mois(&c.fin) {
                for (quoi, n) in [("jours", c.contrat.jours), ("cachets", c.contrat.cachets), ("heures", c.contrat.heures)] {
                    if let Some(v) = n.filter(|n| *n != 0) {
                        activite.insert(quoi, Decimal::from(v));
                    }
                }
            }
        }
        let numero = Some(p.paie.contrat.clone()).filter(|n| !n.is_empty() && p.paie.declare_par != engagement::importe::Declaration::Bulletin);
        out.push(Versee { salarie, brut, numero, debut, fin, activite });
    }
    Ok(out)
}

/// Un mois du dossier.
#[derive(Serialize)]
pub struct MoisReg {
    pub mois: Mois,
    pub libelle: String,
    pub masse: Decimal,
    pub taux: Decimal,
    pub cotisation: Decimal,
    pub source: String,
}

/// Une annee du dossier : un virement, et ce que l'ecran en dit.
#[derive(Serialize)]
pub struct AnneeReg {
    pub annee: String,
    pub masse: Decimal,
    pub taux: Decimal,
    pub cotisation: Decimal,
    pub libelle_virement: String,
    pub libelle_dit: String,
    pub date_libelle: String,
}

#[derive(Serialize)]
pub struct SalarieReg {
    pub annee: String,
    pub salarie: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_naissance: Option<String>,
    pub adresse: String,
    pub masse: Decimal,
}

/// Le salarie d'une ligne de la liste nominative, tel que sa fiche le dit.
#[derive(Serialize)]
pub struct IndividuReg {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nom: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prenom: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_naissance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adresse: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_postal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commune: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Le contrat d'une ligne de la liste nominative, tel que l'engagement importe et la fiche le disent.
#[derive(Serialize)]
pub struct ContratReg {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numero: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debut: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pcs_ese: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emploi: Option<String>,
}

#[derive(Serialize)]
pub struct EmploiReg {
    pub annee: String,
    pub mois: Mois,
    pub individu: IndividuReg,
    pub contrat: ContratReg,
    pub activite: BTreeMap<&'static str, Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debut: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fin: Option<String>,
    pub brut: Decimal,
    pub fichier: String,
}

/// Un courriel du dossier, et son en-tete tel que l'ecran le dit.
#[derive(Serialize)]
pub struct Courriel {
    pub a: String,
    pub objet: String,
    pub corps: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub piece_jointe: Option<&'static str>,
    pub entete_dit: String,
}

impl Courriel {
    fn nouveau(a: String, objet: String, corps: String, piece_jointe: Option<&'static str>) -> Courriel {
        let entete_dit = format!("à {}, objet {}", a, objet);
        Courriel { a, objet, corps, piece_jointe, entete_dit }
    }
}

/// La regularisation CASC-SVP d'une annee : ce qui manque, par salarie et par mois, et ce qui en part. `etat` la nomme
/// parmi les etats qu'`exporter` rend.
#[derive(Serialize)]
pub struct Regularisation {
    pub etat: &'static str,
    pub periode: String,
    pub virements_titre: String,
    pub mois: Vec<MoisReg>,
    pub annees: Vec<AnneeReg>,
    pub salaries: Vec<SalarieReg>,
    pub total: Decimal,
    pub a_faire: Vec<String>,
    pub courriel_audiens: Courriel,
    pub courriel_casc: Courriel,
    pub avertissements: Vec<String>,
    pub fichiers: Vec<String>,
    pub dossier: String,
    pub boite_liee: bool,
    pub emplois: Vec<EmploiReg>,
}

/// Ce que demande une regularisation (`regularisation`).
#[derive(Default)]
pub struct Demande {
    pub mois: Option<Mois>,
    pub annee: Option<String>,
    pub toutes_les_annees: bool,
    pub jusqu_a: Option<Mois>,
    pub annees: Vec<String>,
    /// {annee: AAAA-MM-JJ}
    pub vire_le: Vec<(String, String)>,
    /// La boite courriel de la compagnie est-elle liee ? Le dossier le recopie (`Regularisation::boite_liee`).
    pub boite_liee: bool,
}

/// Le plafond mensuel de la Securite sociale du mois `mois` (`AAAA-MM`), tel que le socle livre de son annee le dit
/// (un plafond national : ni la convention ni la compagnie n'y changent rien) ; aucun quand le logiciel ne livre pas le
/// bareme de cette annee.
fn plafond_mensuel(mois: Mois) -> R<Option<Decimal>> {
    let an = mois.annee();
    let Some(socle) = droit::contexte::socle_livre_lu(an).map_err(|m| m.reclasser(Classe::Illisible))? else { return Ok(None) };
    match socle.plafond_securite_sociale {
        Some(p) => Ok(Some(p.mensuel)),
        None => Err(erreur(Classe::DefautDuLogiciel, format!("Le barème livré de {} n'a pas le plafond de la Sécurité sociale : le signaler au développeur.", an))),
    }
}

/// L'EXEMPTION DE LA CASC-SVP AU MOIS `m` (« AAAA-MM ») : la fiche entreprise de ce mois, lue avec les cles de l'effectif
/// `eff` de son annee (`droit::bareme::casc_exemptee`), comme la DSN du mois la lit.
pub(crate) fn exemptee_au_mois(ctx: &Contexte, eff: &droit::contexte::Effectif, mois: Mois) -> R<bool> {
    let du_mois = ctx.au(&mois.dernier_jour()).map_err(|e| e.reclasser(Classe::Illisible))?;
    let an = mois.annee();
    Ok(droit::bareme::casc_exemptee(&du_mois.entreprise, eff.cinquante(an).as_ref(), eff.cse(an).as_ref()))
}

/// La regularisation CASC-SVP que demande `d`, calculee sans rien ecrire (l'ecriture des fichiers : `ecrire`).
pub fn regularisation(ctx: &Contexte, d: &Demande) -> R<Regularisation> {
    let base = droit::adhesions::casc_svp_base().map_err(|m| m.reclasser(Classe::Disque))?;
    let ent = &ctx.entreprise;
    let idcc = ent.idcc.ecrite("idcc").map_err(|e| e.reclasser(Classe::Refuse))?;
    if idcc != base.idcc {
        return Err(refus(format!(
            "La CASC-SVP est une cotisation de la convention 3090 ; la fiche entreprise dit {}. Il n'y a rien a regulariser.",
            idcc
        )));
    }
    let eff = ctx.effectif().map_err(|e| e.reclasser(Classe::Refuse))?;
    let faits = declarations::casc::mois_regularises(ctx).map_err(|m| m.reclasser(Classe::Disque))?;
    let depuis = Mois::lire(&base.depuis)?;
    let mut candidats: Vec<Mois> = Vec::new();
    for m in import::mois::mois_importes(&ctx.racine)? {
        let lu = Mois::lire(&m)?;
        if lu >= depuis && !faits.contains(&m) {
            candidats.push(lu);
        }
    }
    let annee_de = |m: &Mois| format!("{:04}", m.annee());
    if let Some(m) = d.mois {
        candidats.retain(|x| m == *x);
    } else if let (Some(a), false) = (d.annee.as_ref().filter(|a| !a.is_empty()), d.toutes_les_annees) {
        candidats.retain(|x| annee_de(x) == *a);
    }
    if let Some(j) = d.jusqu_a {
        candidats.retain(|x| *x <= j);
    }
    if !d.annees.is_empty() {
        candidats.retain(|x| d.annees.iter().any(|a| annee_de(x) == *a));
    }
    // Un mois d'une annee ou la compagnie est exemptee (article 4.22 B) ne doit rien : l'exemption se lit au mois.
    let mut exemptes: Vec<Mois> = vec![];
    for &m in &candidats {
        if exemptee_au_mois(ctx, &eff, m)? {
            exemptes.push(m);
        }
    }
    candidats.retain(|m| !exemptes.contains(m));
    let liste = |v: &[Mois]| v.iter().map(Mois::to_string).collect::<Vec<_>>().join(", ");
    if candidats.is_empty() && !exemptes.is_empty() {
        return Err(refus(format!(
            "L'entreprise est hors du dispositif CASC-SVP (article 4.22 B : au moins 50 salaries, un CSE qui contribue au moins autant) pour {} : il n'y a rien a regulariser.",
            liste(&exemptes)
        )));
    }
    let mut avert: Vec<String> = vec![];
    if !exemptes.is_empty() {
        avert.push(format!("Hors du dispositif CASC-SVP (article 4.22 B), donc hors du dossier : {}.", liste(&exemptes)));
    }
    let mut portees: Vec<Mois> = vec![];
    let mut lues: Vec<(Mois, Vec<Versee>)> = vec![];
    let contrat_casc = base.dsn.contrat.clone();
    for &m in &candidats {
        let paies = versees_du_mois(ctx, m)?;
        if paies.is_empty() {
            continue;
        }
        let dsn = crate::importation::ecrite::dsn_importee(ctx, m).map_err(|e| e.reclasser(Classe::Refuse))?;
        // Un mois sans DSN importee, ou dont la DSN ne se lit pas, ne la porte pas.
        let porte = dsn.as_deref().map(|c| crate::importation::ecrite::lire_dsn_casc(c, &contrat_casc)).transpose()?.flatten().is_some_and(|(_, porte)| porte);
        if porte {
            portees.push(m);
            continue;
        }
        lues.push((m, paies));
    }
    if lues.is_empty() {
        if !faits.is_empty() {
            return Err(refus(format!(
                "Tout est regularise : {}. Les DSN suivantes portent la CASC-SVP elles-memes.",
                deja_faites(ctx)?
            )));
        }
        return Err(refus(format!(
            "Aucun mois importe sans la CASC-SVP dans sa DSN depuis {} dans la periode demandee : rien a regulariser.",
            depuis
        )));
    }
    if !faits.is_empty() {
        let mut tries = faits.clone();
        tries.sort();
        avert.push(format!("Deja regularises, donc hors du dossier : {}.", tries.join(", ")));
    }
    if !portees.is_empty() {
        avert.push(format!("La DSN porte deja la CASC-SVP, donc hors du dossier : {}.", liste(&portees)));
    }

    let mut par_mois: Vec<MoisReg> = vec![];
    let mut par_salarie: Vec<((String, String), SalarieReg)> = vec![];
    let mut emplois: Vec<EmploiReg> = vec![];
    for (m, paies) in &lues {
        let source = format!("engagements importes de {}", m);
        let mut masse = Decimal::ZERO;
        let mut mois_sal: Vec<(String, String, Decimal)> = vec![];
        let an = annee_de(m);
        for p in paies {
            // Les colonnes de l'etat nomment les cases de la fiche, lue par sa table.
            let fiche = Saisie::de_la_fiche("salarie", &p.salarie.champs)?;
            let champ = |cle: &str| fiche.mot(cle);
            let brut = p.brut;
            // La liste nominative se tient par NIR : une fiche sans NIR ne s'y range pas.
            let nir = champ("nir").ok_or_else(|| refus(format!("{} : la fiche du salarié « {} » n'a pas de NIR ; la liste nominative de la régularisation le demande.", m, p.salarie.id)))?;
            masse += brut;
            match mois_sal.iter_mut().find(|(n, _, _)| *n == nir) {
                Some((_, _, t)) => *t += brut,
                None => mois_sal.push((nir.clone(), match champ("nom") {
                    Some(n) => n,
                    None => nir.clone(),
                }, brut)),
            }
            let cle = (an.clone(), nir.clone());
            let pos = match par_salarie.iter().position(|(k, _)| *k == cle) {
                Some(i) => i,
                None => {
                    let adresse: Vec<String> = ["adresse", "code_postal", "commune"].iter().filter_map(|k| champ(k)).collect();
                    par_salarie.push((
                        cle,
                        SalarieReg {
                            annee: an.clone(),
                            salarie: [champ("nom"), champ("prenom")].into_iter().flatten().collect::<Vec<_>>().join(" "),
                            nir: Some(nir.clone()),
                            date_naissance: champ("date_naissance"),
                            adresse: adresse.join(" "),
                            masse: Decimal::ZERO,
                        },
                    ));
                    par_salarie.len() - 1
                }
            };
            par_salarie[pos].1.masse += brut;
            let individu = IndividuReg {
                nir: champ("nir"),
                nom: champ("nom"),
                prenom: champ("prenom"),
                date_naissance: champ("date_naissance"),
                adresse: champ("adresse"),
                code_postal: champ("code_postal"),
                commune: champ("commune"),
                email: champ("email"),
            };
            let contrat = ContratReg { numero: p.numero.clone(), debut: p.debut.clone(), fin: p.fin.clone(), pcs_ese: champ("pcs_ese"), emploi: champ("emploi") };
            emplois.push(EmploiReg {
                annee: an.clone(),
                mois: *m,
                individu,
                activite: p.activite.clone(),
                debut: p.debut.clone(),
                fin: p.fin.clone(),
                contrat,
                brut,
                fichier: source.clone(),
            });
        }
        if let Some(plafond) = plafond_mensuel(*m)? {
            for (_, nom, b) in &mois_sal {
                if *b > plafond {
                    avert.push(format!("{}, {} : brut {} au-dessus du plafond mensuel -- la T1 n'est plus égale au brut, base à recalculer.", m, nom, euros(*b)));
                }
            }
        }
        let taux = base.taux.de(&an).ok_or_else(|| {
            refus(format!(
                "Le taux de la CASC-SVP pour {} n'est pas releve (donnees/casc-svp-base.json) : l'export refuse plutot que de prolonger celui d'une autre annee.",
                an
            ))
        })?;
        let mut cot = (masse * taux / Decimal::from(100)).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        cot.rescale(2);
        par_mois.push(MoisReg { mois: *m, libelle: m.libelle(socle::mois::Graphie::Accentuee), masse, taux, cotisation: cot, source });
    }

    let siret = ent.siret.ecrite("siret").map_err(|e| e.reclasser(Classe::Refuse))?.to_string();
    let mut annees: Vec<AnneeReg> = vec![];
    for r in &par_mois {
        let an = annee_de(&r.mois);
        let pos = match annees.iter().position(|a| a.annee == an) {
            Some(p) => p,
            None => {
                let libelle_virement = base.reglement.libelle.replace("{siret}", &siret).replace("{annee}", &an);
                annees.push(AnneeReg {
                    libelle_dit: format!("libellé « {} »", libelle_virement),
                    date_libelle: format!("Virement {} fait le", an),
                    libelle_virement,
                    annee: an,
                    masse: Decimal::ZERO,
                    taux: r.taux,
                    cotisation: Decimal::ZERO,
                });
                annees.len() - 1
            }
        };
        annees[pos].masse += r.masse;
        annees[pos].cotisation += r.cotisation;
    }
    let total = annees.iter().fold(Decimal::ZERO, |t, a| t + a.cotisation);

    let nom = ent.raison_sociale.ecrite("raison_sociale").map_err(|e| e.reclasser(Classe::Refuse))?.to_string();
    let ou = |a: &Option<String>, b: &Option<String>| a.remplie().or_else(|| b.remplie()).map(String::from);
    // Le signataire de la fiche, s'il est dit ; sans lui, les courriels ne sont pas signes.
    let signe: String = ent
        .signataire
        .dite()
        .map(|sig| [ou(&sig.prenom_affichage, &sig.prenom), ou(&sig.nom_affichage, &sig.nom)].into_iter().flatten().collect::<Vec<_>>().join(" "))
        .into_iter()
        .collect();
    let (premier, dernier) = (&par_mois[0].libelle, &par_mois[par_mois.len() - 1].libelle);
    let periode = if premier == dernier { premier.clone() } else { format!("{} à {}", premier, dernier) };

    let mut tableau = vec!["| mois | masse | cotisation |".to_string(), "|---|---:|---:|".to_string()];
    for a in &annees {
        for r in &par_mois {
            if annee_de(&r.mois) == a.annee {
                tableau.push(format!("| {} | {} | {} |", r.libelle, euros(r.masse), euros(r.cotisation)));
            }
        }
        tableau.push(format!("| **{}, à {} %** | **{}** | **{}** |", a.annee, virgule(a.taux), euros(a.masse), euros(a.cotisation)));
    }
    let dates: Vec<(&String, &String)> = d.vire_le.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| (k, v)).collect();
    let virements: Vec<String> = annees
        .iter()
        .map(|a| {
            // Une date de virement que la demande ne dit pas reste a remplir a la main.
            let date = match dates.iter().find(|(k, _)| **k == a.annee).map(|(_, v)| jjmmaaaa(v)).filter(|j| !j.is_empty()) {
                Some(j) => j,
                None => "__/__/____".to_string(),
            };
            format!("- le {}, {}, libellé « {} » ;", date, euros(a.cotisation), a.libelle_virement)
        })
        .collect();
    let beneficiaire = base.reglement.beneficiaire.clone();
    let corps_audiens = format!(
        "Bonjour,\n\nNotre logiciel de paie n'était pas paramétré pour déclarer la cotisation CASC-SVP en DSN (contrat CASCSVP) : elle manque à nos DSN de {}. Nous régularisons par virement, comme l'indique la page employeurs du CASC-SVP, et nos DSN la porteront désormais.\n\n{} — SIRET {} — IDCC 3090.\n\nMasses salariales soumises à cotisation (tranche 1 Agirc-Arrco) :\n\n{}\n\nVirements effectués vers {} :\n{}\n\nLa liste nominative des salariés part en parallèle au CASC-SVP.\n\nBien cordialement,\n\n{}\n{}\n",
        periode, nom, siret, tableau.join("\n"), beneficiaire, virements.join("\n"), signe, nom
    );
    let a_audiens = base.courriel_audiens.a.clone();
    let corps_casc = format!(
        "Bonjour,\n\nNotre logiciel de paie ne déclarait pas la cotisation CASC-SVP en DSN. Nous régularisons auprès d'Audiens ({}) les cotisations de {}, et vous trouverez en pièce jointe la liste nominative de nos salariés pour cette période.\n\n{} — SIRET {} — IDCC 3090.\n\nBien cordialement,\n\n{}\n{}\n",
        a_audiens, periode, nom, siret, signe, nom
    );
    let courriel_audiens = Courriel::nouveau(a_audiens.clone(), base.courriel_audiens.objet.replace("{siret}", &siret), corps_audiens, None);
    let courriel_casc = Courriel::nouveau(
        base.courriel_casc.a.clone(),
        base.courriel_casc.objet.replace("{siret}", &siret),
        corps_casc,
        Some("CASC-SVP_liste-nominative.xlsx (le modele du CASC-SVP, rempli)"),
    );
    let mut a_faire = vec![format!(
        "Faire {} virement(s), un par année, à {} — l'IBAN se lit sur la page employeurs du CASC-SVP ({}) ou dans l'espace Audiens :",
        annees.len(),
        beneficiaire,
        base.page
    )];
    for a in &annees {
        a_faire.push(format!("    {} : {}, libellé « {} »", a.annee, euros(a.cotisation), a.libelle_virement));
    }
    a_faire.push(format!("Reporter la date de chaque virement dans le courriel à Audiens, puis l'envoyer à {} (objet : {}).", courriel_audiens.a, courriel_audiens.objet));
    a_faire.push(format!(
        "Envoyer à {} (objet : {}) le courriel au CASC-SVP, avec en pièce jointe CASC-SVP_liste-nominative.xlsx — leur modèle, déjà rempli.",
        courriel_casc.a, courriel_casc.objet
    ));
    a_faire.push("Une fois les virements faits, le noter dans le logiciel : Exports, bouton « Noter comme régularisé » (route `casc-regularise`) — les mois sortent alors des dossiers suivants.".to_string());
    a_faire.push("Vérifier que la fiche de paramétrage Audiens porte les deux lignes CASC-SVP, pour que les DSN suivantes la déclarent.".to_string());

    let boite = d.boite_liee;
    Ok(Regularisation {
        etat: "regularisation-casc-svp",
        periode,
        virements_titre: if annees.len() > 1 { format!("Les {} virements", annees.len()) } else { "Le virement".to_string() },
        mois: par_mois,
        annees,
        salaries: par_salarie.into_iter().map(|(_, s)| s).collect(),
        total,
        a_faire,
        courriel_audiens,
        courriel_casc,
        avertissements: avert,
        fichiers: vec![],
        dossier: String::new(),
        boite_liee: boite,
        emplois,
    })
}

fn fichier_suivi(ctx: &Contexte) -> std::path::PathBuf {
    ctx.racine.join(declarations::casc::CASC_SVP)
}

/// UNE CELLULE DE LA LISTE : ce que la fiche dit ; une case qu'elle ne dit pas reste une cellule vide.
fn cellule(o: &Option<String>) -> String {
    o.iter().cloned().collect()
}

/// Les annees deja notees regularisees, en une phrase : l'annee, le montant vire (en euros s'il se lit en nombre) et le
/// jour du virement.
fn deja_faites(ctx: &Contexte) -> R<String> {
    let d = declarations::casc::lue(ctx).map_err(|m| m.reclasser(Classe::Illisible))?;
    Ok(d.iter()
        .flat_map(|d| &d.regularisations)
        .map(|x| {
            let montant = match socle::num::decimal_lu(&x.montant.trim().replace(',', ".")) {
                Some(m) => euros(m),
                None => x.montant.clone(),
            };
            format!("{} ({}, viré le {})", x.annee, montant, jjmmaaaa(&x.vire_le))
        })
        .collect::<Vec<_>>()
        .join(", "))
}

/// L'ecriture de la regularisation : `exports/casc-svp/` -- les CSV, les deux courriels, la liste des gestes.
pub fn ecrire(ctx: &Contexte, r: &mut Regularisation) -> R<()> {
    let dossier = ctx.racine.join("exports").join("casc-svp");
    let io = |p: &Path, e: std::io::Error| socle::echec::du_disque(&p, &e).reclasser(Classe::Disque);
    std::fs::create_dir_all(&dossier).map_err(|e| io(&dossier, e))?;
    let virgule = |d: &Decimal| d.to_string().replace('.', ",");
    let mut fichiers: Vec<std::path::PathBuf> = vec![];
    // Le classeur modele : voir `liste_nominative_xlsx`.
    liste_nominative_xlsx(ctx, r, &dossier, &mut fichiers)?;
    let ecrit_csv = |nom: &str, entetes: &[&str], lignes: Vec<Vec<String>>| -> R<std::path::PathBuf> {
        let mut t = socle::csv::ligne(&entetes.iter().map(|s| s.to_string()).collect::<Vec<_>>(), ';');
        for l in &lignes {
            t.push_str(&socle::csv::ligne(l, ';'));
        }
        let p = dossier.join(nom);
        std::fs::write(&p, format!("\u{feff}{}", t)).map_err(|e| io(&p, e))?;
        Ok(p)
    };
    fichiers.push(ecrit_csv(
        "CASC-SVP_masses-par-mois.csv",
        &["mois", "masse", "taux %", "cotisation", "source"],
        r.mois.iter().map(|x| vec![x.mois.to_string(), virgule(&x.masse), virgule(&x.taux), virgule(&x.cotisation), x.source.clone()]).collect(),
    )?);
    fichiers.push(ecrit_csv(
        "CASC-SVP_liste-nominative.csv",
        &["annee", "salarie", "NIR", "date de naissance", "adresse", "masse"],
        r.salaries
            .iter()
            .map(|s| vec![s.annee.clone(), s.salarie.clone(), cellule(&s.nir), cellule(&s.date_naissance), s.adresse.clone(), virgule(&s.masse)])
            .collect(),
    )?);
    for (nom, c) in [("CASC-SVP_courriel-audiens.txt", &r.courriel_audiens), ("CASC-SVP_courriel-casc.txt", &r.courriel_casc)] {
        let p = dossier.join(nom);
        std::fs::write(&p, format!("À : {}\nObjet : {}\n\n{}", c.a, c.objet, c.corps)).map_err(|e| io(&p, e))?;
        fichiers.push(p);
    }
    let p = dossier.join("CASC-SVP_a-faire.txt");
    let verifier = if r.avertissements.is_empty() { String::new() } else { format!("\nÀ vérifier :\n{}", r.avertissements.join("\n")) };
    std::fs::write(&p, format!("Régularisation CASC-SVP — {} — total {}\n\n{}\n{}\n", r.periode, euros(r.total), r.a_faire.join("\n"), verifier)).map_err(|e| io(&p, e))?;
    fichiers.insert(0, p);
    r.fichiers = fichiers.iter().map(|f| f.display().to_string()).collect();
    r.dossier = dossier.display().to_string();
    Ok(())
}

const ENTETES: [(u32, u16, &str); 42] = [
    (0, 0, "Les 3 lignes du haut de la feuille sont à supprimer avant d'enregistrer le fichier au format .CSV (séparateur point-virgule) pour télétransmission."),
    (0, 26, "Salariés autre qu'Intermittents"),
    (0, 30, "Salariés Intermittents"),
    (1, 26, "Nombre d'heures"),
    (1, 30, "\"A\" si artiste\n\"T\" si technicien"),
    (1, 31, "Techniciens"),
    (1, 33, "Artistes"),
    (2, 0, "mention\n\"CASC-SVP\""),
    (2, 1, "N°Siret"),
    (2, 2, "VIDE"),
    (2, 3, "VIDE"),
    (2, 4, "Année de déclaration concernée"),
    (2, 5, "Date de constitution du Fichier"),
    (2, 6, "Nom de naissance"),
    (2, 7, "Nom d'artiste\nNom Marital"),
    (2, 8, "Prénom"),
    (2, 9, "Date de Naissance"),
    (2, 10, "Numéro de Sécurité Sociale avec Clé"),
    (2, 11, "n° App ou Bal / couloir / escalier"),
    (2, 12, "Entrée / bâtiment / immeuble / résidence"),
    (2, 13, "Numéro et libellé de la voie"),
    (2, 14, "Lieu dit ou boîte postale"),
    (2, 15, "Code Postal  en France"),
    (2, 16, "Libellé Commune"),
    (2, 17, "Code postal si hors France"),
    (2, 18, "Pays"),
    (2, 19, "Adresse mail personnelle"),
    (2, 20, "Adresse mail professionnelle"),
    (2, 21, "Libellé de l'Emploi"),
    (2, 22, "Date de début d' emploi"),
    (2, 23, "Date de fin d'emploi"),
    (2, 24, "Salaire de la période"),
    (2, 25, "\"I\" si intermittent\n\"P\" si permanent"),
    (2, 26, "VIDE"),
    (2, 28, "VIDE"),
    (2, 29, "VIDE"),
    (2, 31, "NbJ"),
    (2, 32, "NbH"),
    (2, 33, "NbC"),
    (2, 34, "NbH"),
    (2, 35, "Si P : \ntotalisation des salaires ligne à ligne ex: (ligne1 =salaire1, ligne2=salaire 1+ salaire2, ligne 3=salaire1+2+3.....)"),
    (2, 36, "Si I : \ntotalisation des salaires ligne à ligne ex: (ligne1 =salaire1, ligne2=salaire 1+ salaire2, ligne 3=salaire1+2+3.....)"),
];

/// Les cellules fusionnees du modele : (premiere ligne, premiere colonne, derniere ligne, derniere colonne), a partir de 0.
const FUSIONS: [(u32, u16, u32, u16); 7] = [(0, 0, 1, 25), (0, 26, 0, 29), (0, 30, 0, 34), (1, 26, 1, 29), (1, 30, 2, 30), (1, 31, 1, 32), (1, 33, 1, 34)];

/// Un `Decimal` entier ou non, comme `n(q)` : un nombre s'il y en a un, rien pour zero ou absent.
fn quantite_xlsx(q: Option<&Decimal>) -> Option<f64> {
    use rust_decimal::prelude::ToPrimitive;
    let q = q?;
    if q.is_zero() {
        return None;
    }
    q.to_f64()
}

/// La liste nominative du CASC-SVP, une ligne par emploi, a partir de la 4e ligne.
///
/// Le modele officiel (`donnees/modeles/casc-svp-liste-nominative.xlsx`) ne s'ouvre pas : le classeur est RECREE par
/// `rust_xlsxwriter` avec les memes colonnes (37), les memes trois lignes de consignes (memes textes, memes cellules
/// fusionnees) et les memes valeurs ; la mise en forme est plus simple (gras, retour a la ligne, volet fige sous la
/// 3e ligne). Les consignes de la 4e ligne du modele (« NE PAS REMPLIR »...) n'y sont pas : elles sont
/// effacees avant d'ecrire.
fn liste_nominative_xlsx(ctx: &Contexte, r: &mut Regularisation, dossier: &Path, fichiers: &mut Vec<std::path::PathBuf>) -> R<()> {
    use rust_xlsxwriter::{Format, FormatAlign, Workbook};
    let xl = |e: rust_xlsxwriter::XlsxError| erreur(Classe::Disque, format!("classeur CASC-SVP : {}", e));
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("Feuil1").map_err(xl)?;
    let titre = Format::new().set_bold().set_text_wrap().set_align(FormatAlign::Center).set_align(FormatAlign::VerticalCenter);
    let entete = Format::new().set_text_wrap().set_align(FormatAlign::Top);
    for (a, b, c, d) in FUSIONS {
        ws.merge_range(a, b, c, d, "", &titre).map_err(xl)?;
    }
    for (ligne, col, texte) in ENTETES {
        ws.write_string_with_format(ligne, col, texte, if ligne < 2 { &titre } else { &entete }).map_err(xl)?;
    }
    ws.set_row_height(2, 132.0).map_err(xl)?;
    ws.set_freeze_panes(3, 0).map_err(xl)?;
    ws.set_column_range_width(0, 36, 14.0).map_err(xl)?;
    let siret = ctx.entreprise.siret.ecrite("siret").map_err(|e| e.reclasser(Classe::Refuse))?.to_string();
    let aujourdhui = droit::contexte::jour_reel().jj_mm_aaaa();
    let mut cumul: Vec<((String, Option<String>), Decimal)> = Vec::new();
    let texte_de = cellule;
    let coupe = |t: String, n: usize| -> String { t.chars().take(n).collect() };
    let arrondi = |d: Decimal| -> f64 { socle::num::flottant(d.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)) };
    for (n, x) in r.emplois.iter().enumerate() {
        let ligne = 3 + n as u32;
        let (i, c) = (&x.individu, &x.contrat);
        let artiste = c.pcs_ese.as_deref().is_some_and(|p| p.starts_with("354"));
        let nir = i.nir.clone();
        let cle = (x.annee.clone(), nir.clone());
        let total = match cumul.iter_mut().find(|(k, _)| *k == cle) {
            Some((_, t)) => {
                *t += x.brut;
                *t
            }
            None => {
                cumul.push((cle, x.brut));
                x.brut
            }
        };
        let cle_nir = cellule(&nir.as_deref().and_then(declarations::champs::cle_nir));
        let jj = |iso: &Option<String>| -> String {
            match iso {
                Some(t) if !t.is_empty() => jjmmaaaa(t),
                _ => String::new(),
            }
        };
        let textes: Vec<(u16, String)> = vec![
            (0, "CASC-SVP".to_string()),
            (1, siret.clone()),
            (4, x.annee.clone()),
            (5, aujourdhui.clone()),
            (6, coupe(texte_de(&i.nom), 30)),
            (8, coupe(texte_de(&i.prenom), 32)),
            (9, jj(&i.date_naissance)),
            (10, format!("{}{}", texte_de(&nir), cle_nir)),
            (13, coupe(texte_de(&i.adresse), 32)),
            (15, coupe(texte_de(&i.code_postal), 5)),
            (16, coupe(texte_de(&i.commune), 26)),
            (19, texte_de(&i.email)),
            (21, coupe(texte_de(&c.emploi), 34)),
            (22, jj(&x.debut)),
            (23, jj(&x.fin)),
            (25, "I".to_string()),
            (30, if artiste { "A" } else { "T" }.to_string()),
        ];
        for (col, t) in textes {
            ws.write_string(ligne, col, &t).map_err(xl)?;
        }
        ws.write_number(ligne, 24, arrondi(x.brut)).map_err(xl)?;
        ws.write_number(ligne, 36, arrondi(total)).map_err(xl)?;
        let act = |q: &str| x.activite.get(q);
        let colonnes: [(u16, &str); 2] = if artiste { [(33, "cachets"), (34, "heures")] } else { [(31, "jours"), (32, "heures")] };
        for (col, quoi) in colonnes {
            if let Some(v) = quantite_xlsx(act(quoi)) {
                ws.write_number(ligne, col, v).map_err(xl)?;
            }
        }
    }
    let sans: Vec<&EmploiReg> = r.emplois.iter().filter(|x| x.activite.is_empty()).collect();
    if !sans.is_empty() {
        let mut mois: Vec<Mois> = sans.iter().map(|x| x.mois).collect();
        mois.sort();
        mois.dedup();
        let mois: Vec<String> = mois.iter().map(Mois::to_string).collect();
        r.avertissements.push(format!(
            "Liste nominative : {} emploi(s) sans jours, cachets ni heures dans leur DSN ({}) -- colonnes laissees vides.",
            sans.len(),
            mois.join(", ")
        ));
    }
    let chemin = dossier.join("CASC-SVP_liste-nominative.xlsx");
    wb.save(&chemin).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque))?;
    fichiers.push(chemin);
    Ok(())
}

pub use declarations::casc::RegularisationNotee;

/// La route `casc-regularise` note qu'une annee est regularisee. S'ecrit
/// APRES le virement, jamais avant.
pub fn marquer_regularise(ctx: &Contexte, annee: &str, vire_le: &str, montant: &str, qui: &str, aujourd_hui: &str) -> R<RegularisationNotee> {
    let r = regularisation(ctx, &Demande { annee: Some(annee.to_string()), ..Default::default() })?;
    let p = fichier_suivi(ctx);
    let mut d = match declarations::casc::lue(ctx).map_err(|m| m.reclasser(Classe::Illisible))? {
        Some(d) => d,
        None => declarations::casc::CascSvp {
            quoi: "Les regularisations CASC-SVP faites hors DSN. Un mois ici ne revient plus dans un dossier de regularisation.".into(),
            regularisations: vec![],
        },
    };
    let calcule = r.mois.iter().fold(Decimal::ZERO, |t, m| t + m.cotisation);
    let entree = RegularisationNotee {
        annee: annee.to_string(),
        mois: r.mois.iter().map(|m| m.mois.to_string()).collect(),
        montant: montant.to_string(),
        calcule: calcule.to_string(),
        vire_le: vire_le.to_string(),
        note_le: aujourd_hui.to_string(),
        qui: qui.to_string(),
    };
    d.regularisations.push(entree.clone());
    ecrire_json(&p, &d, 2).map_err(|m| m.reclasser(Classe::Disque))?;
    Ok(entree)
}
