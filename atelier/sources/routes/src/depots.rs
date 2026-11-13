//! Les routes des declarations : ecrire ou reprendre la DSN ou la DPAE du mois et dire ce
//! qu'elle declare (`depot-apercu`), la deposer (`deposer`), suivre un depot (`suivre-depot`), le bilan de son test (`bilan-repetition`).
//!
//! Les formats sont ailleurs (`dpae`, la DSN de `declarations`, la fabrique de PDF) : ce module enchaine -- charger,
//! choisir, refuser, nommer, ecrire, et passer a `envois` ce qui part. Le jour est celui du contexte
//! (`PUPPETPAIE_AUJOURDHUI`) ; l'horodatage d'un fichier partiel de DPAE est la minute de l'horloge, a l'heure de Paris.

use droit::fiche::Lue as _;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use compagnie::preuve::Preuve;
use crate::gestes::depot_acte;
use crate::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, MANQUE, MOIS, OPERATION};
use crate::catalogue::Route;
use declarations::dpae::{Chemins, Embauche, Parametres};
use droit::contexte::{Contexte, IdSalarie};
use engagement::calcul::Paie;
use engagement::numeros::NumeroContrat;
use envois::etats;
use envois::releve_retours;
use socle::date::Jour;
use socle::paris::Moment;
use socle::echec::{erreur, Classe, Echec, R};
use socle::fichiers::{dossier_du_code, NomDeFichier};
use socle::mois::{Graphie, Mois};
use serde::{Deserialize, Serialize};
use envois::depot_acte::DepotFait;
use envois::etats::Repetition;
use envois::ligne::Ligne;
use crate::apercu_dsn::ApercuDsn;
use super::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use super::Encart;
use socle::ecran::Pastille;
use crate::catalogue::{Ecran, Place};

// LA DESIGNATION d'une declaration (`designee`) : la DSN ou la DPAE d'un mois, ou un signalement ecrit.
const QUOI: Champ = Champ { nom: "quoi", libelle: Affiche::new("DPAE ou DSN"), forme: Forme::Texte, sens: "dpae ou dsn", avance: false };
const ETABLISSEMENT: Champ = Champ { nom: "etablissement", libelle: Affiche::new("Établissement"), forme: Forme::Texte, sens: "le NIC de l'établissement (DSN), le principal sinon", avance: true };
const ENGAGEMENTS: Champ = Champ { nom: "engagements", libelle: Affiche::new("Embauches"), forme: Forme::Objet, sens: "seulement ces embauches (DPAE) : [{salarie, numero}]", avance: true };
const SIGNALEMENT: Champ = Champ { nom: "fichier", libelle: Affiche::new("Signalement"), forme: Forme::Texte, sens: "un signalement écrit, tel que sa route le rend, à la place du mois", avance: true };
/// L'exemple : la DPAE de novembre, que `depot-apercu` ecrit.
const DPAE_DE_L_EXEMPLE: &str = r#"{"quoi":"dpae","mois":"2026-11"}"#;
const ILLISIBLE_FICHIER: Refus = Refus { classe: Classe::FichierAbsent, corriger: "le signalement : le fichier que sa route a rendu" };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::DepotApercu,
        geste: "Écrire ou reprendre la DSN ou la DPAE du mois ; dire ce qu'elle déclare.",
        charge: &[QUOI, MOIS, ETABLISSEMENT, ENGAGEMENTS, SIGNALEMENT],
        exemple: DPAE_DE_L_EXEMPLE,
        rend: "ce qu'elle déclare (description, embauches, ou salariés, recapitulatif avec son titre et sa phrase, ordres et ordres_phrase), deja_depose, repetition (où en est son test, titre et phrase de l'encart), encarts (titre, phrase, couleur), phrase, avertissements, informations",
        refus: &[ILLISIBLE_FICHIER, MANQUE, OPERATION],
        acte: Acte::Ecrit,
        phrase: 300,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Deposer,
        geste: "Déposer la DSN ou la DPAE du mois : en test, ou en réel après son test.",
        charge: &[
            QUOI,
            MOIS,
            ETABLISSEMENT,
            ENGAGEMENTS,
            SIGNALEMENT,
            Champ { nom: "reel", libelle: Affiche::new("Dépôt réel"), forme: Forme::OuiNon, sens: "vrai : le dépôt réel (le mot de passe du logiciel, tapé par un humain au moment de l'acte)", avance: false },
            Champ { nom: "forcer", libelle: Affiche::new("Annule et remplace"), forme: Forme::OuiNon, sens: "vrai : l'annule-et-remplace d'une DSN déjà déposée, après l'avertissement", avance: true },
            Champ { nom: "contournement", libelle: Affiche::new("Motif d'un dépôt réel sans test"), forme: Forme::Texte, sens: "le motif écrit d'un dépôt réel sans test", avance: true },
        ],
        exemple: DPAE_DE_L_EXEMPLE,
        rend: "le dépôt : accepté ou non, le retour de l'organisme, ses suites (chaque demande de taux), phrase, avis (l'encart à montrer après la DSN réelle d'un mois qui a des salariés)",
        refus: &[
            Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe du logiciel, tapé par un humain ; rien n'est parti" },
            Refus { classe: Classe::RepetitionManquante, corriger: "déposer d'abord ce fichier en test" },
            Refus { classe: Classe::DsnDejaDeposee, corriger: "rien, ou renvoyer avec forcer si l'annule-et-remplace est voulu" },
            ILLISIBLE_FICHIER,
            OPERATION,
        ],
        acte: Acte::Engage,
        phrase: 400,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::SuivreDepot,
        geste: "Après un dépôt réel, relever les retours et dire s'il est passé ou refusé.",
        charge: &[QUOI, Champ { nom: "flux", libelle: Affiche::new("Flux"), forme: Forme::Texte, sens: "le flux du dépôt, tel que deposer le rend", avance: false }, MOIS],
        exemple: r#"{"quoi":"dpae"}"#,
        rend: "fini, passe, refuse, incident (le relevé qui n'a pu se faire), phrase et couleur",
        refus: &[MANQUE],
        acte: Acte::Ecrit,
        phrase: 80,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::BilanRepetition,
        geste: "Dire où en est le test de la DSN ou de la DPAE ; chercher son compte rendu.",
        charge: &[QUOI, MOIS, ETABLISSEMENT, ENGAGEMENTS, SIGNALEMENT],
        exemple: DPAE_DE_L_EXEMPLE,
        rend: "etat (testee : le réel peut partir ; en_test ; test_refuse, et son motif ; ecrite), titre et phrase de l'encart, conforme, incident",
        refus: &[ILLISIBLE_FICHIER, MANQUE, OPERATION],
        acte: Acte::Ecrit,
        phrase: 500,
        ecran: ECRAN,
    },
];

// ------------------------------------------------------------------ les formes

/// CE QUI SE DEPOSE : la DPAE ou la DSN (un signalement est une DSN).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Quoi {
    Dpae,
    Dsn,
}

impl Quoi {
    /// Sa sorte au journal des envois.
    pub(crate) fn acte(self) -> envois::journal::Acte {
        match self {
            Quoi::Dpae => envois::journal::Acte::Dpae,
            Quoi::Dsn => envois::journal::Acte::Dsn,
        }
    }
}

/// Une embauche que la charge designe : le salarie et le numero du contrat de son engagement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EmbaucheDesignee {
    pub(crate) salarie: IdSalarie,
    pub(crate) numero: NumeroContrat,
}

/// LA DESIGNATION D'UNE DECLARATION, la charge de `depot-apercu` et de `bilan-repetition` : la DSN ou la DPAE d'un mois
/// (`mois`, et l'etablissement d'une DSN, les embauches d'une DPAE), ou un signalement ecrit (`fichier`). C'est aussi
/// la charge du geste qui ouvre la boite du depot (`etapes::Action`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Designation {
    pub(crate) quoi: Quoi,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mois: Option<Mois>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) etablissement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) engagements: Option<Vec<EmbaucheDesignee>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fichier: Option<String>,
}

impl Designation {
    /// La declaration du mois `mois`.
    pub fn du_mois(quoi: Quoi, mois: Mois) -> Designation {
        Designation { quoi, mois: Some(mois), etablissement: None, engagements: None, fichier: None }
    }

    /// Le signalement ecrit `fichier`.
    pub(crate) fn du_fichier(fichier: impl Into<String>) -> Designation {
        Designation { quoi: Quoi::Dsn, mois: None, etablissement: None, engagements: None, fichier: Some(fichier.into()) }
    }

    /// L'etablissement nomme (son NIC), s'il l'est.
    fn nic(&self) -> Option<&str> {
        self.etablissement.as_deref()
    }
}

/// La charge de `deposer` : la designation, et ce que le depot demande (le reel, l'annule-et-remplace voulu, le motif
/// d'un reel sans test) ; absents, ils ne sont pas demandes. Le mot de passe du logiciel est a l'enveloppe.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeDeposer {
    quoi: Quoi,
    #[serde(skip_serializing_if = "Option::is_none")]
    mois: Option<Mois>,
    #[serde(skip_serializing_if = "Option::is_none")]
    etablissement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    engagements: Option<Vec<EmbaucheDesignee>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fichier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reel: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    forcer: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contournement: Option<String>,
}

/// Le depot d'une declaration tel qu'un programme le demande a la route `deposer` : sa charge, et le mot de passe du
/// logiciel tape pour l'acte, a sa cle de l'enveloppe.
#[derive(Debug, Serialize)]
pub(crate) struct DepotDemande {
    #[serde(flatten)]
    charge: ChargeDeposer,
    #[serde(skip_serializing_if = "Option::is_none")]
    motdepasse: Option<String>,
}

impl ChargeDeposer {
    /// Le depot de la declaration `d`, reel ou de test, sous le mot de passe tape.
    pub(crate) fn de(d: Designation, reel: bool, motdepasse: Option<String>) -> DepotDemande {
        let Designation { quoi, mois, etablissement, engagements, fichier } = d;
        DepotDemande { charge: ChargeDeposer { quoi, mois, etablissement, engagements, fichier, reel: Some(reel), forcer: None, contournement: None }, motdepasse }
    }
}

/// La charge de `suivre-depot` : ce qui est parti (`flux`, tel que `deposer` le rend ; absent, tout flux) et son mois.
/// Le depot groupe de l'administrateur la rend pour chaque declaration partie en reel (`administrateur::ASuivre`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeSuivre {
    pub(crate) quoi: Quoi,
    /// Tel que `deposer` le rend : `""` quand le depot n'a pas de flux.
    #[serde(default, deserialize_with = "socle::forme::vide_si_aucun::deserialize")]
    pub(crate) flux: Option<envois::retours::IdDeFlux>,
    pub(crate) mois: Option<Mois>,
}

/// CE QUE LA BOITE DU DEPOT DIT, autour de ce que le fichier declare (`A`, l'apercu d'une DSN ou d'une DPAE) : sa
/// phrase, son titre, ses encarts, et les phrases pour `moteur route --texte`.
#[derive(Debug, Serialize)]
pub(crate) struct Dit<A> {
    phrase: String,
    titre: &'static str,
    encarts: Vec<Encart>,
    phrases: Vec<String>,
    avertissements: Vec<String>,
    informations: Vec<String>,
    #[serde(flatten)]
    apercu: A,
}

/// La reponse de `depot-apercu` : ce que la DSN ou la DPAE designee declare.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum DepotApercu {
    Dsn(Dit<ApercuDsn>),
    Dpae(Dit<ApercuDpae>),
}

/// La reponse de `deposer` : le depot fait, sa phrase, et l'avis a montrer apres la DSN reelle d'un mois qui a des
/// salaries.
#[derive(Debug, Serialize)]
pub(crate) struct Deposee {
    phrase: String,
    #[serde(flatten)]
    fait: DepotFait,
    #[serde(skip_serializing_if = "Option::is_none")]
    avis: Option<Encart>,
}

/// La reponse de `suivre-depot` : le depot tranche ou non, passe ou refuse, le releve qui n'a pu se faire, la phrase
/// et sa couleur.
#[derive(Debug, Serialize)]
pub(crate) struct Suivi {
    pub(crate) fini: bool,
    passe: bool,
    refuse: bool,
    incident: String,
    phrase: String,
    couleur: &'static str,
}

/// La reponse de `bilan-repetition` : ou en est le test (`etats::Repetition`), et le releve qui n'a pu se faire.
#[derive(Debug, Serialize)]
pub(crate) struct BilanDuTest {
    #[serde(flatten)]
    repetition: Repetition,
    incident: String,
}

/// Une route de ce module.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::DepotApercu => en_json(depot_apercu(ctx, charge.lire()?)?),
        Route::Deposer => en_json(deposer(ctx, charge.lire()?, charge.mot_de_passe())?),
        Route::SuivreDepot => en_json(suivre_depot(ctx, charge.lire()?)?),
        Route::BilanRepetition => en_json(bilan_repetition(ctx, charge.lire()?)?),
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

impl Dite for DepotApercu {
    fn phrase(&self) -> &str {
        match self {
            DepotApercu::Dsn(d) => &d.phrase,
            DepotApercu::Dpae(d) => &d.phrase,
        }
    }
}

impl Dite for Deposee {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for Suivi {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for BilanDuTest {
    fn phrase(&self) -> &str {
        &self.repetition.phrase
    }
}

/// CE QUE LA BOITE DU DEPOT DIT DU FICHIER, encart par encart, dans l'ordre ou elle les montre : l'annule-et-remplace
/// qu'il porte, le prelevement a la source sur la grille par defaut (CGI, art. 204 H), les embauches deja declarees, le
/// fichier deja depose, le fichier code envoi de test.
fn encarts_du_depot(apercu: &DepotApercu) -> Vec<Encart> {
    let mut e = Vec::new();
    let (deja_depose, reel) = match apercu {
        DepotApercu::Dsn(Dit { apercu: a, .. }) => {
            if let Some(remplace) = &a.remplace {
                e.push(Encart::de(socle::ecran::EtatDEcran::Information, "Annule et remplace", format!("Ce fichier remplace la déclaration {}.", remplace)));
            }
            // Un fichier qui remplace la DSN partie porte son ordre (`apercu_dsn::remplacement_du_fichier`).
            if let (Some(r), Some(ordre)) = (a.remplacement.as_ref().filter(|r| r.fichier_remplace), &a.ordre) {
                let partie: String = r.etat.dernier_ordre.iter().map(|o| format!(" (ordre {})", o)).collect();
                e.push(Encart::de(
                    socle::ecran::EtatDEcran::AFaire,
                    format!("Annule-et-remplace intégral — ordre {}", ordre),
                    format!(
                        "Il remplace la DSN réelle déjà partie pour ce mois{}, ordres de télépaiement compris : c'est lui qui vaudra. Admis jusqu'au {} à minuit.",
                        partie,
                        r.etat.limite.jj_mm_aaaa()
                    ),
                ));
            }
            if a.pas_bareme > 0 {
                let n = a.pas_bareme;
                e.push(Encart::de(
                    socle::ecran::EtatDEcran::AFaire,
                    format!("Prélèvement à la source : {} versement{} sur {} sur la grille par défaut", n, if n > 1 { "s" } else { "" }, a.pas_versements),
                    "Sans taux transmis par la DGFiP, la grille de l'article 204 H du CGI s'applique ; le dépôt n'est pas bloqué. Elle prélève souvent plus que le taux réel, et le salarié ne récupère la différence qu'à sa déclaration de revenus. Relever les retours avant de déposer peut changer cela.",
                ));
            }
            (&a.deja_depose, a.reel)
        }
        DepotApercu::Dpae(Dit { apercu: a, .. }) => {
            let deja = a.embauches.iter().filter(|x| x.deja_declaree).count();
            if deja > 0 {
                e.push(Encart::de(
                    socle::ecran::EtatDEcran::AFaire,
                    format!("{} déjà envoyée{}", socle::ecran::compte(deja, "embauche", "embauches"), if deja > 1 { "s" } else { "" }),
                    "Le fichier porte tout le mois : le déposer les redéclare, et l'Urssaf reçoit deux déclarations pour un seul contrat.",
                ));
            }
            (&a.deja_depose, a.reel)
        }
    };
    if let Some(deja) = deja_depose {
        e.push(Encart::de(
            socle::ecran::EtatDEcran::AFaire,
            "Ce fichier a déjà été déposé",
            format!("Le {}, en mode {}. Redéposer le même fichier ne remplace rien : un annule-et-remplace se produit, il ne se redépose pas.", deja.quand().dit(), if deja.reel() { "réel" } else { "test" }),
        ));
    }
    if !reel {
        e.push(Encart::de(socle::ecran::EtatDEcran::Information, "Envoi de test", "Le fichier est codé envoi de test : déposé pour de vrai, il ne déclarerait rien et rien ne serait conservé."));
    }
    e
}

/// LES PASTILLES DES LIGNES DU JOURNAL QUE CES ROUTES RENDENT (`routes::pastille`) : la ligne d'un certificat de
/// conformite DPAE (`enregistree`) des retours d'un depot, enregistree, faite ; deja existante, une information ;
/// refusee, refusee.
fn poser_les_pastilles(v: &mut Option<Ligne>) {
    v.iter_mut().for_each(etats::poser_les_pastilles);
}

/// Les pastilles des lignes du journal qu'un test porte (`poser_les_pastilles`).
fn pastilles_du_test(r: &mut Repetition) {
    r.lignes_mut().for_each(etats::poser_les_pastilles);
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}

fn ecrire_texte(chemin: &Path, texte: &str) -> R<()> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).map_err(|e| socle::echec::du_disque(&d, &e))?;
    }
    std::fs::write(chemin, texte).map_err(|e| socle::echec::du_disque(&chemin, &e))
}

// ------------------------------------------------------------------ dpae

/// Les noms `DPAE_<etiquette>_AAAAMMJJ-HHMM.txt` d'un dossier, du plus grand au plus petit
/// (le plus récent d'abord). Un dossier absent n'en a aucun ; un dossier qui ne se lit pas se refuse.
fn fichiers_partiels(dossier: &Path, etiq: &str) -> R<Vec<PathBuf>> {
    let debut = format!("DPAE_{}_", etiq);
    let lecture = match std::fs::read_dir(dossier) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(erreur_io(&e, dossier)),
    };
    let mut out = Vec::new();
    for e in lecture {
        let e = e.map_err(|x| erreur_io(&x, dossier))?;
        let Some(nom) = e.file_name().to_str().map(str::to_string) else { continue };
        let Some(reste) = nom.strip_prefix(&debut) else { continue };
        let Some(reste) = reste.strip_suffix(".txt") else { continue };
        let c: Vec<char> = reste.chars().collect();
        // `AAAAMMJJ-HHMM`, et `AAAAMMJJ-HHMM-N` : le nom que prend un fichier partiel quand un autre porte deja le sien
        // dans la meme minute (`nom_libre_du_partiel`).
        let rang_ok = c.len() == 13 || (c.len() > 14 && c[13] == '-' && c[14..].iter().all(char::is_ascii_digit));
        if c.len() >= 13 && c[8] == '-' && rang_ok {
            out.push(e.path());
        }
    }
    out.sort();
    out.reverse();
    Ok(out)
}

/// La marque `_AAAAMMJJ-HHMM` d'un fichier partiel ecrit maintenant.
fn marque_a_la_minute() -> String {
    let maintenant = socle::temps::secondes().a_la_seconde();
    let (j, s) = (maintenant.jour_de_paris(), maintenant.seconde_du_jour_de_paris());
    format!("_{}-{:02}{:02}", j.aaaammjj(), s / 3_600, s / 60 % 60)
}

/// La marque `_AAAAMMJJ-HHMM` d'un fichier partiel, suivie de `-2`, `-3`... tant que `DPAE_<etiq><marque>.txt` existe deja.
pub(crate) fn nom_libre_du_partiel(dossier: &Path, etiq: &str, marque: &str) -> String {
    let mut essai = marque.to_string();
    let mut rang = 2;
    while dossier.join(format!("DPAE_{}{}.txt", etiq, essai)).exists() {
        essai = format!("{}-{}", marque, rang);
        rang += 1;
    }
    essai
}

fn latin1(octets: &[u8]) -> String {
    octets.iter().map(|&o| o as char).collect()
}

/// Le code Urssaf que la DPAE de cette compagnie porte.
fn code_urssaf(ent: &droit::fiche::ChampsEntreprise) -> R<String> {
    if let Some(ecrit) = ent.code_urssaf.portee() {
        return Ok(ecrit.trim().to_string());
    }
    let base = declarations::urssaf::base()?;
    let territoire = ent.code_postal.portee().and_then(|cp| base.du_code_postal(cp));
    territoire.ok_or_else(|| {
        socle::echec::cle("L'Urssaf de la compagnie ne se lit pas dans son code postal : poser son code Urssaf dans la fiche Entreprise.", "code_urssaf")
            .reclasser(Classe::SaisieIncomplete)
    })
}

/// `Embauche` de la DPAE, lue d'une paie sans montants.
fn embauche_dpae(p: &Paie, spectacle: bool) -> Embauche {
    Embauche {
        salarie: p.salarie.clone(),
        debut: p.engagement.debut,
        fin: p.engagement.fin,
        nature: p.engagement.nature.clone(),
        spectacle: p.spectacle.clone().filter(|_| spectacle),
    }
}


/// La DPAE, due AVANT le premier jour travaille : tout le mois `le_mois`, ou les embauches `selection` (le salarie et le
/// numero du contrat de l'engagement) dans un fichier horodate a cote du complet. La phrase rendue dit combien
/// d'embauches elle declare ; la demande de taux de PAS part avec son depot (`depot_acte::deposer_dpae`).
pub(crate) fn dpae(ctx: &Contexte, le_mois: Mois, selection: Option<&[EmbaucheDesignee]>) -> R<Dpae> {
    let mois = le_mois.to_string();
    // L'heure d'embauche que la DPAE declare.
    let heure = declarations::dpae::HEURE_EMBAUCHE;

    // Les engagements valides du mois (rien ne part d'un engagement en saisie), vus comme des embauches, sans relire leur
    // paie.
    let journal = envois::journal::lu(&ctx.racine)?;
    let data = engagement::etats::validees_du_mois(ctx, le_mois, &journal.departs)?.map(|(d, _)| d)
        .ok_or_else(|| erreur(Classe::FichierAbsent, format!("Aucune saisie pour {}.", mois)))?;
    let paies: Vec<Paie> = {
        let engs = engagement::engagements_de_la_saisie(ctx, &data)?;
        engs.iter().map(|e| engagement::embauche(ctx, e)).collect::<Result<_, _>>()?
    };
    let etiq = le_mois.etiquette();
    let sortie = ctx.racine.join("sorties").join(&mois);
    // Une embauche par son salarie et son numero de contrat ; une paie sans numero n'en a pas a declarer.
    let cle_de = |p: &Paie| -> Option<(IdSalarie, NumeroContrat)> { p.engagement.numero.as_ref().map(|n| (p.salarie.id.clone(), n.clone())) };

    // LES DPAE A FAIRE DU MOIS (`Moteur::embauches_a_declarer`, la seule liste) : une embauche deja partie en DPAE
    // reelle, ou un contrat en cours depuis un mois precedent (sa DPAE part avec son premier mois), n'y est pas.
    let a_faire: BTreeSet<(IdSalarie, NumeroContrat)> = crate::etapes::Moteur::nouveau(ctx)
        .embauches_a_declarer(Some(le_mois))?
        .into_iter()
        .map(|e| (e.salarie, e.numero))
        .collect();
    let mut retenues: Vec<&Paie> = paies.iter().filter(|p| cle_de(p).is_some_and(|k| a_faire.contains(&k))).collect();
    let mut marque = String::new();
    if let Some(sel) = selection {
        let voulues: BTreeSet<(IdSalarie, NumeroContrat)> = sel.iter().map(|e| (e.salarie.clone(), e.numero.clone())).collect();
        let joint = |s: BTreeSet<&(IdSalarie, NumeroContrat)>| s.into_iter().map(|(a, b)| format!("{} (contrat {})", a, b)).collect::<Vec<_>>().join(", ");
        let hors: BTreeSet<&(IdSalarie, NumeroContrat)> = voulues.difference(&a_faire).collect();
        if !hors.is_empty() {
            let tous = hors.len() == voulues.len();
            let phrase = format!(
                "Aucune DPAE a faire pour : {} (deja partie, en cours depuis un mois precedent, ou absente de la saisie de {}). Rien n'est ecrit.",
                joint(hors),
                mois
            );
            return if tous { Ok(Dpae::Rien(phrase)) } else { Err(refus(phrase)) };
        }
        retenues.retain(|p| cle_de(p).is_some_and(|k| voulues.contains(&k)));
        marque = marque_a_la_minute();
    }
    if retenues.is_empty() {
        return Ok(Dpae::Rien(format!("Aucune DPAE à faire en {}. Rien n'est écrit.", le_mois.libelle(Graphie::Accentuee))));
    }

    // RETOURS WINDOWS, D : DEUX LIGNES IDENTIQUES d'une saisie declareraient deux fois la meme embauche : la DPAE ne se
    // produit pas, le refus dit lesquelles (salarie, jour) et qu'il faut corriger la saisie. Ni le numero de contrat ni
    // la validation ne distinguent deux lignes.
    // Chaque carte sans son numero ni sa validation : deux cartes qui ne different que par eux sont la meme embauche.
    let sans_numero: Vec<engagement::saisie::Carte> = data
        .engagements
        .iter()
        .map(|c| engagement::saisie::Carte { numero: None, valide: None, ..c.clone() })
        .collect();
    if sans_numero.len() == paies.len() {
        let retenue = |i: usize| retenues.iter().any(|r| std::ptr::eq(*r, &paies[i]));
        let mut doubles: Vec<String> = Vec::new();
        for i in 1..paies.len() {
            if retenue(i) && (0..i).any(|j| retenue(j) && sans_numero[i] == sans_numero[j]) {
                let texte = format!("{} au {}", paies[i].salarie.id, paies[i].engagement.debut.iso());
                if !doubles.contains(&texte) {
                    doubles.push(texte);
                }
            }
        }
        if !doubles.is_empty() {
            return Err(refus(format!(
                "Deux lignes identiques dans la saisie de {} declareraient deux fois la meme embauche : {}. Corrigez la saisie (une seule ligne par embauche). Rien n'est ecrit.",
                mois,
                doubles.join(", ")
            )));
        }
    }

    // LE MEME CONTENU, LE MEME FICHIER : une DPAE deja ecrite qui dit la meme chose (la selection, ou tout ce qui reste a
    // declarer) se reprend telle quelle : le test qu'elle a passe vaut pour le reel (regle 10).
    let ent = ctx.entreprise_et_gabarits();
    let code = code_urssaf(&ctx.entreprise)?;
    let chemins = Chemins {
        defaut: dossier_du_code().join("modeles").join("due_tel_gabarit.txt"),
        poste: ctx.gabarits(),
    };
    let heure_texte = heure;
    let embauches: Vec<Embauche> = retenues.iter().map(|p| embauche_dpae(p, false)).collect();
    let mut f: Option<PathBuf> = None;
    {
        let parametres = Parametres {
            horodatage: socle::temps::secondes().a_la_minute(),
            heure_embauche: heure_texte.to_string(),
            code_urssaf: Some(code.to_string()),
            ent: Some(&ent),
            chemins: &chemins,
        };
        let voulu = declarations::dpae::contenu(&embauches, &parametres)?;
        let voulu = latin1(&voulu);
        let mut pareils = Vec::new();
        let complet = sortie.join(format!("DPAE_{}.txt", etiq));
        // LA DPAE DU MOIS DEJA PARTIE ne se recrit pas (regle 4) : ce qui reste a declarer se cherche, et s'ecrit, sous un
        // nom a la minute, a cote.
        let partiel = !marque.is_empty() || compagnie::pieces_ecrites::partie_le(&ctx.racine, &complet)?.is_some();
        let candidats = if !partiel { vec![complet].into_iter().filter(|c| c.is_file()).collect() } else { fichiers_partiels(&sortie, &etiq)? };
        for x in candidats {
            let contenu = std::fs::read(&x).map_err(|e| socle::echec::du_disque(&x, &e).reclasser(Classe::Disque))?;
            if declarations::dpae::meme_declaration(&latin1(&contenu), &voulu) {
                pareils.push(x);
            }
        }
        let mut repetes = Vec::new();
        for x in &pareils {
            let empreinte = envois::journal::empreinte_fichier(x)?;
            let nom = NomDeFichier::du(x)?;
            if etats::repetition(&journal.lignes, Quoi::Dpae.acte(), &empreinte, &nom).conforme.is_some() {
                repetes.push(x.clone());
            }
        }
        if !pareils.is_empty() {
            // Celle qui a son test conforme, sinon la plus recente.
            let choisi = match repetes.first() {
                Some(r) => r.clone(),
                None => pareils[0].clone(),
            };
            let nom = NomDeFichier::du(&choisi)?;
            marque = nom
                .texte()
                .strip_prefix(&format!("DPAE_{}", etiq))
                .and_then(|r| r.strip_suffix(".txt"))
                .ok_or_else(|| socle::echec::a_la_cle("Défaut du logiciel : une DPAE du mois ne porte pas son nom.", "fichier", &nom).reclasser(Classe::DefautDuLogiciel))?
                .to_string();
            f = Some(choisi);
        } else if partiel && marque.is_empty() {
            marque = marque_a_la_minute();
        }
    }
    // JAMAIS PAR-DESSUS UN AUTRE (deux DPAE jouees dans la meme minute s'ecraseraient) : si le
    // nom a la minute est pris par une declaration DIFFERENTE, le fichier prend `-2`, `-3`... ; le nom ordinaire ne change pas.
    if f.is_none() && !marque.is_empty() {
        marque = nom_libre_du_partiel(&sortie, &etiq, &marque);
    }
    let recap = sortie.join(format!("DPAE_{}{}_recapitulatif.txt", etiq, marque));
    let f = match f {
        Some(f) => f,
        None => {
            let cible = sortie.join(format!("DPAE_{}{}.txt", etiq, marque));
            ecrire_la_dpae(ctx, &retenues, &cible, &recap, heure, socle::temps::secondes().a_la_seconde())?;
            compagnie::pieces_ecrites::noter_piece_ecrite(&ctx.racine, &compagnie::pieces_ecrites::cle_de_piece(&cible), None, None)?;
            cible
        }
    };

    let n = retenues.len();
    Ok(Dpae::Ecrite(DpaeEcrite {
        chemin: f,
        phrase: format!("DPAE {} écrite : {} embauche{}.", socle::texte::de(&le_mois.libelle(Graphie::Accentuee)), n, if n > 1 { "s" } else { "" }),
    }))
}

/// CE QUE `dpae` REND : la DPAE ecrite, ou rien a declarer de ce que la charge designe (tout est parti, ou rien n'est
/// valide), avec la phrase qui le dit. Un manque (une case de fiche, deux lignes identiques) est un refus, jamais
/// « rien » : celui qui depose doit le lire.
pub(crate) enum Dpae {
    Ecrite(DpaeEcrite),
    Rien(String),
}

/// La DPAE ecrite : le chemin de son fichier (son `.zip` et son recapitulatif en clair sont a cote), et la phrase qui
/// dit combien d'embauches elle declare.
pub(crate) struct DpaeEcrite {
    pub(crate) chemin: PathBuf,
    pub(crate) phrase: String,
}

/// LES FICHIERS D'UNE DPAE : l'enregistrement de chaque embauche de `retenues` dans `cible` (et son `.zip`), horodate
/// `quand` (a Paris, a la minute), l'heure d'embauche `heure` ; et son recapitulatif en clair dans `recap`.
/// Ce que `dpae` ecrit, et ce que `gestes::depart::piece_partie` refait d'une DPAE partie dont le fichier manque.
pub(crate) fn ecrire_la_dpae(ctx: &Contexte, retenues: &[&Paie], cible: &Path, recap: &Path, heure: &str, quand: Moment) -> R<()> {
    let ent = ctx.entreprise_et_gabarits();
    let code = code_urssaf(&ctx.entreprise)?;
    let chemins = Chemins { defaut: dossier_du_code().join("modeles").join("due_tel_gabarit.txt"), poste: ctx.gabarits() };
    let embauches: Vec<Embauche> = retenues.iter().map(|p| embauche_dpae(p, false)).collect();
    let parametres = Parametres {
        horodatage: quand.a_la_minute(),
        heure_embauche: heure.to_string(),
        code_urssaf: Some(code.to_string()),
        ent: Some(&ent),
        chemins: &chemins,
    };
    declarations::dpae::ecrire(&embauches, cible, &parametres, true)?;
    let pour_recap: Vec<Embauche> = retenues.iter().map(|p| embauche_dpae(p, true)).collect();
    ecrire_texte(recap, &declarations::dpae::recapitulatif(&pour_recap, heure)?)
}


#[cfg(test)]
mod tests_dpae_partielles {
    use super::*;

    fn dossier() -> PathBuf {
        let r = crate::horloge::racine_d_essai(false).join("sorties").join("2026-10");
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    #[test]
    fn deux_dpae_dans_la_meme_minute_ne_s_ecrasent_jamais() {
        // Le nom `DPAE_<mois>_<AAAAMMJJ-HHMM>.txt` est a la minute : la seconde declaration de la minute ecraserait la
        // premiere, et le depot du premier jour ne serait plus relisible tel qu'il a ete depose.
        let d = dossier();
        let marque = "_20261007-1815";
        // Cas ordinaire : le nom ne change pas.
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", marque), marque);
        std::fs::write(d.join("DPAE_Octobre2026_20261007-1815.txt"), "premiere").unwrap();
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", marque), "_20261007-1815-2");
        std::fs::write(d.join("DPAE_Octobre2026_20261007-1815-2.txt"), "seconde").unwrap();
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", marque), "_20261007-1815-3");
        // Une autre minute, ou un autre mois, garde son nom.
        assert_eq!(nom_libre_du_partiel(&d, "Octobre2026", "_20261007-1816"), "_20261007-1816");
        assert_eq!(nom_libre_du_partiel(&d, "Novembre2026", marque), marque);
        // Les fichiers deja ecrits sont intacts.
        assert_eq!(std::fs::read_to_string(d.join("DPAE_Octobre2026_20261007-1815.txt")).unwrap(), "premiere");
    }
}

// ------------------------------------------------------------------------ deposer

/// CE QU'ECRIRE LA DECLARATION A DIT : les avertissements et les informations de la DSN fabriquee (`DsnFabriquee::avis`),
/// ou la phrase de la DPAE ecrite.
#[derive(Debug, Default)]
pub struct DitALEcriture {
    phrase: Option<String>,
    avertissements: Vec<String>,
    informations: Vec<String>,
}

/// LA DECLARATION QUE LA DESIGNATION DESIGNE (son chemin), et ce que son ecriture a dit. La DSN du mois (`quoi` : dsn,
/// `mois`, `etablissement`) : fabriquee depuis les engagements valides (`fabriquer_dsn`, qui reprend tel quel le fichier
/// qui dit la meme chose : le test vaut pour le reel, regle 10) ; partie en reel, celle qui est partie, sauf si elle est
/// a corriger. La DPAE du mois (`quoi` : dpae, `mois`, `engagements`) : celle de ces embauches, ou de celles qui restent
/// a declarer (`dpae`, meme reprise) ; une fois partie, il n'en reste aucune a declarer, et c'est celle qui est partie
/// (`partie`), jamais refaite (regle 4). `fichier` : un signalement ecrit.
pub fn designee(ctx: &Contexte, d: &Designation) -> R<(String, DitALEcriture)> {
    // Un signalement se nomme par son fichier, la declaration du mois par son mois : les deux ne designent pas la meme.
    let (le_mois, fichier) = (d.mois, d.fichier.as_deref().filter(|f| !f.is_empty()));
    let Some(le_mois) = le_mois else {
        return fichier.map(|f| (f.to_string(), DitALEcriture::default())).ok_or_else(|| socle::fichiers::absent("fichier").reclasser(Classe::Manque));
    };
    if fichier.is_some() {
        return Err(socle::echec::a_la_cle(
            "Le fichier et le mois désignent deux déclarations : le fichier seul pour un signalement écrit, le mois seul pour la \
             déclaration du mois. Rien n'est parti.",
            "fichier",
            "mois",
        )
        .reclasser(Classe::Refuse));
    }
    if d.quoi == Quoi::Dsn {
        // UNE DSN PARTIE EN REEL NE SE REFAIT QUE SI CE QU'ELLE A EMPORTE EST A CORRIGER (regle 4) : un engagement du
        // mois change depuis, ajoute ou retire (`Moteur::etat_remplacement`, `a_envoyer`) ; sinon, c'est celle qui est
        // partie. Une DSN refusee au bilan n'est pas partie (regle 5).
        let nic = d.nic();
        let e = compagnie::etat_dsn::du_mois(ctx, le_mois, nic)?;
        if e.deposee() && !e.refusee() && !crate::etapes::Moteur::nouveau(ctx).etat_remplacement(le_mois, nic)?.a_envoyer {
            if let Some(c) = partie(ctx, d, le_mois)? {
                return Ok((c, DitALEcriture::default()));
            }
        }
        let dsn = crate::gestes::depart::fabriquer_dsn(ctx, le_mois, "interface")?;
        let chemin = match d.etablissement.as_deref().filter(|n| !n.is_empty()) {
            Some(n) => dsn.fichiers.iter().find(|f| f.file_name().is_some_and(|x| x.to_string_lossy().contains(&format!("_{}", n)))),
            None => dsn.fichiers.first(),
        };
        let chemin = chemin.ok_or_else(|| refus("Aucune DSN de cet etablissement ce mois-ci. Rien n'est parti."))?;
        let (avertissements, informations) = dsn.avis();
        return Ok((chemin.to_string_lossy().into_owned(), DitALEcriture { phrase: None, avertissements, informations }));
    }
    // Ce qui reste a declarer s'ecrit, et un manque refuse ; quand rien ne reste, c'est la DPAE partie.
    match dpae(ctx, le_mois, d.engagements.as_deref())? {
        Dpae::Ecrite(w) => Ok((w.chemin.to_string_lossy().into_owned(), DitALEcriture { phrase: Some(w.phrase), ..Default::default() })),
        Dpae::Rien(phrase) => partie(ctx, d, le_mois)?.map(|c| (c, DitALEcriture::default())).ok_or_else(|| refus(phrase)),
    }
}

/// CE QUI EST PARTI POUR LA DESIGNATION `d` du mois `mois`, en test ou en reel, lu au journal (`envois::journal::lu`) :
/// le fichier de la derniere ligne du mois, du meme etablissement (DSN mensuelle) ou des memes embauches (DPAE, quand la
/// designation les nomme). Rien ne se fabrique.
fn partie(ctx: &Contexte, d: &Designation, mois: Mois) -> R<Option<String>> {
    let lignes = envois::journal::lu(&ctx.racine)?.lignes;
    // Une fiche qui ne dit pas encore son NIC n'a que son siege, et aucune DSN n'en est partie.
    let principal = ctx.entreprise.nic_principal();
    let nic = d.etablissement.as_deref().filter(|n| !n.is_empty()).or(principal);
    let voulues: Option<BTreeSet<etats::Paire>> = d.engagements.as_ref().filter(|e| !e.is_empty()).map(|e| e.iter().map(|x| (x.salarie.clone(), Some(x.numero.clone()))).collect());
    let ligne = envois::journal::filtrer(&lignes, Some(d.quoi.acte()), Some(mois), false).rev().find(|l| match l {
        // Une ligne de DSN qui ne nomme pas son etablissement est celle de l'etablissement principal.
        Ligne::Dsn(_) => {
            let de = l.etablissement().or(principal);
            l.fichier().is_some_and(|f| f.texte().starts_with("DSN_")) && de == nic
        }
        Ligne::Dpae(e) => voulues.as_ref().is_none_or(|v| e.detail.embauches.iter().map(|x| (x.salarie.clone(), x.numero.clone())).collect::<BTreeSet<_>>() == *v),
        // `filtrer` n'a rendu que des lignes de l'acte demande, une DSN ou une DPAE.
        Ligne::Documents(_) | Ligne::ContratSignature(_) | Ligne::ContratContresigne(_) | Ligne::RegularisationCasc(_) | Ligne::DossierSalarie(_) | Ligne::FicheDemande(_) => false,
    });
    Ok(ligne.and_then(Ligne::fichier).map(|f| ctx.racine.join("sorties").join(mois.to_string()).join(f).to_string_lossy().into_owned()))
}

/// `deposer` : `motdepasse` est celui DU LOGICIEL, et il ne sert qu'a prouver la presence. Il arrive par le
/// corps du POST, il n'est ni journalise ni renvoye. `contournement` : le motif ECRIT d'un depot reel sans
/// repetition. `forcer` : l'annule-et-remplace voulu, apres l'avertissement que la page affiche sur
/// `DsnDejaDeposee` -- jamais par defaut.
fn deposer(ctx: &Contexte, c: ChargeDeposer, motdepasse: Option<&str>) -> R<Deposee> {
    let ChargeDeposer { quoi, mois, etablissement, engagements, fichier, reel, forcer, contournement } = c;
    let (reel, forcer) = (reel == Some(true), forcer == Some(true));
    let d = Designation { quoi, mois, etablissement, engagements, fichier };
    let (fichier, _) = designee(ctx, &d)?;
    let preuve = Preuve::du_mot_de_passe(motdepasse.map(String::from));
    // Le motif ecrit d'un reel sans test ; aucun motif, aucun contournement (`depot_acte::exiger_repetition`).
    let contournement: String = contournement.iter().map(|c| c.trim()).collect();
    let contournement = contournement.as_str();
    let mut fait = match quoi {
        Quoi::Dsn => depot_acte::deposer_dsn(ctx, &fichier, reel, Some(&preuve), "interface", forcer, contournement)?,
        Quoi::Dpae => depot_acte::deposer_dpae(ctx, &fichier, reel, Some(&preuve), "interface", false, contournement)?,
    };
    // LA PHRASE DU DEPOT : ou en est son test, ou ce qu'ont dit les retours du reel s'ils sont deja la (le simulateur repond
    // dans le geste) ; puis ses suites.
    let tranche = if !reel {
        let b = bilan_de_la_repetition(ctx, &Designation::du_fichier(fichier.clone()).avec(quoi))?;
        format!("{} {}", b.titre, b.phrase)
    } else if fait.ligne.nombre_de_retours() > 0 {
        suivre_depot(ctx, ChargeSuivre { quoi, flux: fait.flux.clone(), mois: Some(fait.mois) })?.phrase
    } else {
        "Dépôt réel effectué — ses retours sont attendus.".to_string()
    };
    let phrase = std::iter::once(tranche).chain(fait.suites.iter().cloned()).collect::<Vec<_>>().join(" ");
    // APRES LA DSN REELLE D'UN MOIS QUI A DES SALARIES, la boite d'avis : leurs documents partent de l'etape 4.
    let avis = (reel && quoi == Quoi::Dsn && !declarations::dsn_lue::lire(&lire_octets(&resoudre(&ctx.racine, &fichier)?)?, None)?.individus.is_empty())
        .then(|| Encart::de(socle::ecran::EtatDEcran::Information, "Les documents des salariés", "Ils partent de l'étape 4, « Envoyer les documents », qui dit ce qu'il leur manque encore."));
    etats::poser_les_pastilles(&mut fait.ligne);
    poser_les_pastilles(&mut fait.deja);
    Ok(Deposee { phrase, fait, avis })
}

impl Designation {
    /// La meme designation, pour `quoi`.
    fn avec(self, quoi: Quoi) -> Designation {
        Designation { quoi, ..self }
    }
}

// ------------------------------------------------------------------ suivre-depot

/// `suivre_le_depot` : juste apres un depot REEL, relever ses retours et dire s'il est tranche. Une LECTURE. Fini quand
/// chaque embauche du depot est passee ou refusee (DPAE), quand le bilan du mois est la (DSN).
pub(crate) fn suivre_depot(ctx: &Contexte, c: ChargeSuivre) -> R<Suivi> {
    let ChargeSuivre { quoi, flux, mois } = c;
    let dpae = quoi == Quoi::Dpae;
    let (debut, fin) = releve_retours::derniere_journee()?;
    let releve = if dpae {
        crate::gestes::releve::relever_retours_dpae(ctx, debut, fin, false, flux.as_ref())
    } else {
        crate::gestes::releve::relever_retours(ctx, debut, fin, false, "production", None, None)
    };
    let incident = match releve {
        Ok(r) => releve_retours::incidents_en_phrase(&r),
        Err(e) => e.phrase,
    };
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    // La ligne du journal du depot que le flux nomme ; sans flux, aucune.
    let du_flux = |q: envois::journal::Acte| flux.as_ref().and_then(|f| lignes.iter().rev().find(|l| l.acte() == q && l.flux() == Some(f)));
    let (passe, refuse);
    if dpae {
        let etat = etats::etat_des_embauches(&lignes);
        // Une embauche que l'etat ne connait pas (une ligne non acceptee) n'est ni passee ni refusee.
        let embauches: Vec<Option<&etats::EtatEmbauche>> = du_flux(Quoi::Dpae.acte())
            .and_then(Ligne::dpae)
            .into_iter()
            .flat_map(|e| e.detail.embauches.iter().map(|x| etat.get(&(x.salarie.clone(), x.numero.clone()))))
            .collect();
        passe = !embauches.is_empty() && embauches.iter().all(|x| x.is_some_and(|x| x.passee));
        refuse = embauches.iter().any(|x| x.is_some_and(|x| x.refusee));
    } else {
        let ligne = du_flux(Quoi::Dsn.acte());
        let nic = ligne.and_then(Ligne::etablissement);
        // Sans son mois, rien ne dit ou en est la DSN : elle attend.
        let e = mois.map(|m| compagnie::etat_dsn::du_mois(ctx, m, nic)).transpose()?;
        passe = e.as_ref().is_some_and(|e| e.etat == compagnie::etat_dsn::Etat::Passee);
        refuse = e.as_ref().is_some_and(|e| e.refusee());
    }
    // Ce que l'ecran en dit, et en quelle couleur, une fois le depot tranche.
    let nom = if dpae { "La DPAE" } else { "La DSN" };
    let (phrase, etat) = if passe {
        (format!("{} est passée — retour de l'organisme reçu.", nom), socle::ecran::EtatDEcran::Fait)
    } else if refuse {
        (format!("{} est refusée — voir le motif dans l'étape.", nom), socle::ecran::EtatDEcran::Refuse)
    } else {
        (format!("{} attend le retour de l'organisme.", nom), socle::ecran::EtatDEcran::EnAttente)
    };
    Ok(Suivi { fini: passe || refuse, passe, refuse, incident, phrase, couleur: socle::ecran::couleur_de_l_etat(etat) })
}

// ------------------------------------------------------------------ bilan-repetition

/// `bilan_de_la_repetition` : ou en est le test de CE fichier -- et, s'il attend son compte rendu, aller le
/// chercher maintenant. Une LECTURE : rien ne part. Rend `etats::Repetition`, et `incident` si le releve n'a
/// pas pu se faire (le test reste ce qu'il est ; on le redira au passage suivant).
fn bilan_repetition(ctx: &Contexte, d: Designation) -> R<BilanDuTest> {
    let mut repetition = bilan_de_la_repetition(ctx, &d)?;
    let mut incident = String::new();
    if repetition.etat == etats::EtatDuTest::EnTest {
        let (debut, fin) = releve_retours::derniere_journee()?;
        let releve = match d.quoi {
            Quoi::Dpae => {
                let flux = repetition.flux_conforme();
                crate::gestes::releve::relever_retours_dpae(ctx, debut, fin, false, flux.as_ref())
            }
            Quoi::Dsn => crate::gestes::releve::relever_retours(ctx, debut, fin, false, "production", None, None),
        };
        incident = match releve {
            Ok(rel) => releve_retours::incidents_en_phrase(&rel),
            Err(e) => e.phrase,
        };
        repetition = bilan_de_la_repetition(ctx, &d)?;
    }
    pastilles_du_test(&mut repetition);
    Ok(BilanDuTest { repetition, incident })
}

/// L'erreur du disque, sous sa classe (`FichierAbsent`, `Disque`).
fn erreur_io(e: &std::io::Error, p: &Path) -> Echec {
    erreur(Classe::du_disque(e), socle::fichiers::panne_fichier(e, p))
}

// ------------------------------------------------------------------ les chemins

/// Un chemin POSIX normalisé : les `//` et les `.` intermédiaires s'en vont, un `/`
/// de tete reste (deux, s'il y en a exactement deux).
pub(crate) fn normaliser(t: &str) -> String {
    let tete = if t.starts_with("//") && !t.starts_with("///") {
        "//"
    } else if t.starts_with('/') {
        "/"
    } else {
        ""
    };
    let morceaux: Vec<&str> = t.split('/').filter(|m| !m.is_empty() && *m != ".").collect();
    let corps = morceaux.join("/");
    if tete.is_empty() && corps.is_empty() {
        return ".".to_string();
    }
    format!("{}{}", tete, corps)
}

fn existe(t: &str) -> bool {
    !t.contains('\0') && Path::new(t).exists()
}

/// Un chemin tape a la main, tel quel s'il existe, sinon sous les donnees. Rend le texte du chemin, normalisé. La
/// declaration d'un mois se designe par le mois de la charge (`Designation::mois`), jamais par un chemin.
pub(crate) fn resoudre(racine: &Path, t: &str) -> R<String> {
    if t.chars().all(socle::texte::blanc) {
        return Err(erreur(Classe::FichierAbsent, "Aucun fichier indique."));
    }
    let p = normaliser(t);
    if existe(&p) || p.starts_with('/') {
        return Ok(p);
    }
    let base = normaliser(&racine.display().to_string());
    let candidat = normaliser(&format!("{}/{}", base, p));
    if existe(&candidat) {
        return Ok(candidat);
    }
    Ok(p)
}

/// `read_bytes`, l'erreur du disque sous sa classe.
fn empreinte_du_fichier(chemin: &str) -> R<socle::fichiers::EmpreinteCourte> {
    if chemin.contains('\0') {
        return Err(erreur(Classe::Illisible, "embedded null byte"));
    }
    let p = Path::new(chemin);
    let octets = std::fs::read(p).map_err(|e| erreur_io(&e, p))?;
    Ok(socle::fichiers::Empreinte::des_octets(&octets).courte())
}

// ------------------------------------------------------------------ depot-apercu, la DPAE

/// Le contenu d'un fichier, en octets.
pub(crate) fn lire_octets(chemin: &str) -> R<Vec<u8>> {
    if chemin.contains('\0') {
        return Err(erreur(Classe::Illisible, "embedded null byte"));
    }
    let p = Path::new(chemin);
    std::fs::read(p).map_err(|e| erreur_io(&e, p))
}

/// Un jour `JJMMAAAA` du fichier DPAE ; `None` : ce n'est pas un jour.
fn jour_jjmmaaaa(v: &str) -> Option<Jour> {
    socle::piece::Zone::lue(v, socle::piece::format::Jjmmaaaa).valeur()
}

/// L'identite d'une fiche, telle qu'une DPAE l'ecrit (`apercu_depot_dpae`).
struct IdentiteDeFiche {
    cle: IdSalarie,
    nir: Option<String>,
    nom: Option<String>,
    prenom: Option<String>,
    naissance: Option<Jour>,
}

/// UNE EMBAUCHE QUE LE FICHIER DPAE DECLARE : le salarie (`?` s'il n'est pas de la configuration), son nom, son contrat
/// (numero, bornes, nature), si elle est deja declaree, et sa pastille.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EmbaucheDuFichier {
    /// La fiche que l'embauche designe ; `None` : aucune fiche ne porte son identite.
    pub(crate) salarie: Option<IdSalarie>,
    pub(crate) nom: String,
    pub(crate) debut: Jour,
    pub(crate) numero: Option<NumeroContrat>,
    pub(crate) fin: Option<Jour>,
    pub(crate) type_contrat: Option<String>,
    pub(crate) deja_declaree: bool,
    pastille: Pastille,
}

/// CE QUE LE FICHIER DPAE DECLARE (`apercu_depot_dpae`) : son nom, sa taille, son indicateur, son mois (celui de sa
/// premiere embauche), ses embauches, s'il est deja depose (`deja_depose` : la ligne du journal) et son test.
#[derive(Debug, Serialize)]
pub(crate) struct ApercuDpae {
    pub(crate) description: String,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub(crate) fichier: PathBuf,
    pub(crate) nom: NomDeFichier,
    pub(crate) enregistrements: usize,
    pub(crate) octets: usize,
    pub(crate) reel: bool,
    pub(crate) indicateur: String,
    pub(crate) mois: Option<Mois>,
    pub(crate) embauches: Vec<EmbaucheDuFichier>,
    pub(crate) deja_depose: Option<Ligne>,
    pub(crate) repetition: Repetition,
}

/// `apercu_depot_dpae` : ce que le fichier DPAE declare, enregistrement par enregistrement.
pub(crate) fn apercu_depot_dpae(ctx: &Contexte, fichier: &str) -> R<ApercuDpae> {
    let chemin = resoudre(&ctx.racine, fichier)?;
    let contenu = lire_octets(&chemin)?;
    let texte: String = contenu.iter().map(|&b| b as char).collect();
    let enrs = declarations::dpae::relire(&texte);
    let m = crate::etapes::Moteur::nouveau(ctx);
    let lignes_journal = m.lignes()?;
    let faites = etats::embauches_par_depot(&lignes_journal);
    // L'IDENTITE DE CHAQUE FICHE, telle que la DPAE l'ecrit : son NIR, et sans lui son nom, son prenom (en capitales) et
    // sa date de naissance (`JJMMAAAA`).
    let fiches: Vec<IdentiteDeFiche> = ctx
        .salaries
        .iter()
        .map(|(k, f)| IdentiteDeFiche {
            cle: k.clone(),
            nir: f.champs.nir.remplie().map(String::from),
            nom: f.champs.nom.portee().map(|t| t.trim().to_uppercase()),
            prenom: f.champs.prenom.portee().map(|t| t.trim().to_uppercase()),
            naissance: f.champs.date_naissance.dite().copied(),
        })
        .collect();
    // LE NUMERO DU CONTRAT DE CHAQUE EMBAUCHE : celui de l'engagement valide du salarie qui commence ce jour-la. C'est lui
    // qui relie la DPAE, partie ou refusee, a son engagement.
    let departs = m.departs()?;
    let mut numeros: HashMap<(IdSalarie, Option<Jour>), NumeroContrat> = HashMap::new();
    // La nature de chaque embauche, par son nom court (« CDDU ») : le fichier ne dit que CDD, CDI ou CTT (Format 120).
    let mut natures: HashMap<(IdSalarie, Option<Jour>), String> = HashMap::new();
    let mut mois_vus: Vec<socle::mois::Mois> = Vec::new();
    let mut lignes = Vec::new();
    let mut jours: Vec<Jour> = Vec::new();
    for e in &enrs {
        let Some(j) = e.date_embauche.as_deref().and_then(jour_jjmmaaaa) else { continue };
        let m_e = j.mois_civil();
        if !mois_vus.contains(&m_e) {
            for etat in engagement::etats::tels_qu_ecrits(ctx, m_e, &departs)? {
                let Some(v) = etat.valide() else { continue };
                let brut = &*v.objet().brut;
                let Some(cle) = &brut.saisie.carte.salarie else { continue };
                let id = (cle.clone(), engagement::debut_brut(brut));
                natures.insert(id.clone(), brut.saisie.carte.nature_lue()?.libelle.clone());
                if let Some(n) = &brut.saisie.carte.numero {
                    numeros.insert(id, n.clone());
                }
            }
            mois_vus.push(m_e);
        }
    }
    for e in &enrs {
        // Sans NIR, la DPAE declare un numero temporaire (zone a blanc) : l'embauche se reconnait a l'identite qu'elle
        // porte, nom, prenom et date de naissance (`dpae::enregistrement`).
        let trouve: Option<&IdentiteDeFiche> = match e.nir.as_deref() {
            Some(nir) => fiches.iter().find(|f| f.nir.as_deref() == Some(nir)),
            None => {
                let (nom, prenom, naissance) = (e.nom.as_deref(), e.prenom.as_deref(), e.date_naissance.as_deref().map(jour_jjmmaaaa));
                fiches.iter().find(|f| nom.is_some() && f.nom.as_deref() == nom && f.prenom.as_deref() == prenom && f.naissance.map(Some) == naissance)
            }
        };
        let debut = e
            .date_embauche
            .as_deref()
            .and_then(jour_jjmmaaaa)
            .ok_or_else(|| erreur(Classe::Illisible, "Une embauche du fichier DPAE ne porte pas sa date d'embauche (JJMMAAAA)."))?;
        jours.push(debut);
        let id = trouve.map(|f| (f.cle.clone(), Some(debut)));
        let nom = match trouve.and_then(|f| ctx.salaries.get(&f.cle)).and_then(|f| f.champs.nomme()) {
            Some(n) => n,
            None => "salarié inconnu de la configuration".to_string(),
        };
        // La nature de l'engagement, par son nom court ; sans engagement, le type que le fichier ecrit (Format 120).
        let type_contrat = match (id.as_ref().and_then(|k| natures.get(k)), e.type_contrat.as_deref()) {
            (Some(n), _) => Some(n.clone()),
            (None, Some(t)) => Some(match droit::natures::TYPES_CONTRAT_DPAE.iter().find(|(k, _)| *k == t) {
                Some((_, l)) => l.to_string(),
                None => format!("type {}", t),
            }),
            (None, None) => None,
        };
        let numero = id.as_ref().and_then(|k| numeros.get(k)).cloned();
        let deja = trouve.zip(numero.as_ref()).is_some_and(|(f, n)| faites.contains_key(&(f.cle.clone(), Some(n.clone()))));
        lignes.push(EmbaucheDuFichier {
            salarie: trouve.map(|f| f.cle.clone()),
            nom,
            debut,
            numero,
            fin: e.date_fin.as_deref().and_then(jour_jjmmaaaa),
            type_contrat,
            deja_declaree: deja,
            // Deja declaree, a voir (elle partirait deux fois) ; a declarer, une information.
            pastille: if deja { Pastille::de(socle::ecran::EtatDEcran::AFaire, "déjà déclarée") } else { Pastille::de(socle::ecran::EtatDEcran::Information, "à déclarer") },
        });
    }
    let mois = jours.iter().map(|j| j.mois_civil()).min();
    let empreinte = socle::fichiers::Empreinte::des_octets(&contenu).courte();
    let nom_fichier = NomDeFichier::du(Path::new(&chemin))?;
    let mut deja_depose = etats::deja_depose(&lignes_journal, Quoi::Dpae.acte(), &empreinte);
    poser_les_pastilles(&mut deja_depose);
    let mut repetition = etats::repetition(&lignes_journal, Quoi::Dpae.acte(), &empreinte, &nom_fichier);
    pastilles_du_test(&mut repetition);
    Ok(ApercuDpae {
        description: format!("{} — {} enregistrement{}, {} octets, indicateur {}.", nom_fichier, enrs.len(), if enrs.len() > 1 { "s" } else { "" }, contenu.len(), envois::depot::indicateur(&contenu)),
        fichier: PathBuf::from(chemin),
        nom: nom_fichier,
        enregistrements: enrs.len(),
        octets: contenu.len(),
        reel: envois::depot::est_reel(&contenu),
        indicateur: envois::depot::indicateur(&contenu),
        mois,
        embauches: lignes,
        deja_depose,
        repetition,
    })
}

/// `depot-apercu` : la declaration designee, ecrite ou reprise (`designee`), et ce qu'elle declare, avec ce que son
/// ecriture a dit (ce que deux engagements disent pareil se dit une fois) ; les encarts de la boite et sa phrase.
fn depot_apercu(ctx: &Contexte, d: Designation) -> R<DepotApercu> {
    let (fichier, dit) = designee(ctx, &d)?;
    let une_fois = |l: Vec<String>| {
        let mut vus: Vec<String> = Vec::new();
        l.into_iter().filter(|x| if vus.contains(x) { false } else { vus.push(x.clone()); true }).collect::<Vec<_>>()
    };
    let (avertissements, informations) = (une_fois(dit.avertissements), une_fois(dit.informations));
    let ecrite = dit.phrase;
    let mut a = match d.quoi {
        Quoi::Dsn => {
            let mut a = crate::apercu_dsn::apercu_depot_dsn(ctx, &fichier)?;
            poser_les_pastilles(&mut a.deja_depose);
            pastilles_du_test(&mut a.repetition);
            // Une DSN se dit par sa nature, son mois et ses salaries quand rien d'autre n'est a dire.
            let phrase = match ecrite {
                Some(p) => p,
                None => format!("{} {} : {}.", a.libelle_nature, socle::texte::de(&a.mois.libelle(Graphie::Accentuee)), socle::ecran::compte(a.salaries, "salarié", "salariés")),
            };
            DepotApercu::Dsn(Dit { phrase, titre: "Déposer la DSN", encarts: Vec::new(), phrases: Vec::new(), avertissements, informations, apercu: a })
        }
        Quoi::Dpae => {
            let a = apercu_depot_dpae(ctx, &fichier)?;
            // Une DPAE se dit par son mois et ses embauches quand rien d'autre n'est a dire.
            let embauches = socle::ecran::compte(a.embauches.len(), "embauche", "embauches");
            let phrase = match (ecrite, a.mois) {
                (Some(p), _) => p,
                (None, Some(m)) => format!("DPAE {} : {}.", socle::texte::de(&m.libelle(Graphie::Accentuee)), embauches),
                (None, None) => format!("DPAE : {}.", embauches),
            };
            DepotApercu::Dpae(Dit { phrase, titre: "Déposer la DPAE", encarts: Vec::new(), phrases: Vec::new(), avertissements, informations, apercu: a })
        }
    };
    // Les phrases de ses parties (le remplacement, le recapitulatif, le test), a la suite de la sienne.
    let encarts = encarts_du_depot(&a);
    let mut lignes: Vec<String> = match &a {
        DepotApercu::Dsn(Dit { apercu: x, .. }) => {
            let recap = match &x.recapitulatif {
                Some(crate::apercu_dsn::Recapitulatif::Lu(r)) => Some(r.phrase.clone()),
                _ => None,
            };
            [x.remplacement.as_ref().map(|r| r.etat.phrase.clone()), recap, Some(x.repetition.phrase.clone())].into_iter().flatten().collect()
        }
        DepotApercu::Dpae(Dit { apercu: x, .. }) => vec![x.repetition.phrase.clone()],
    };
    lignes.retain(|p| !p.is_empty());
    lignes.extend(encarts.iter().map(Encart::en_ligne));
    match &mut a {
        DepotApercu::Dsn(dit) => {
            lignes.extend(dit.avertissements.iter().cloned());
            dit.encarts = encarts;
            dit.phrases = lignes;
        }
        DepotApercu::Dpae(dit) => {
            lignes.extend(dit.avertissements.iter().cloned());
            dit.encarts = encarts;
            dit.phrases = lignes;
        }
    }
    Ok(a)
}

/// Ou en est le test de ce qui est parti pour la designation (`partie`), ou du signalement nomme.
fn bilan_de_la_repetition(ctx: &Contexte, d: &Designation) -> R<Repetition> {
    let fichier = match d.mois {
        Some(m) => partie(ctx, d, m)?.ok_or_else(|| refus(format!("Rien n'est parti pour {} : le déposer d'abord en test.", m)))?,
        None => d.fichier.clone().filter(|f| !f.is_empty()).ok_or_else(|| socle::fichiers::absent("fichier").reclasser(Classe::Manque))?,
    };
    let chemin = resoudre(&ctx.racine, &fichier)?;
    let empreinte = empreinte_du_fichier(&chemin)?;
    let nom = NomDeFichier::du(Path::new(&chemin))?;
    let lignes = crate::etapes::Moteur::nouveau(ctx).lignes()?;
    Ok(etats::repetition(&lignes, d.quoi.acte(), &empreinte, &nom))
}
