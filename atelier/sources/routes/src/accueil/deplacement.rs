//! OU EST LE LOGICIEL, ET LE DEPLACER, A L'ACCUEIL (`compagnie::deplacement`) : l'accueil dit d'emblee ou sont le
//! logiciel et ses compagnies (`logiciel-ou`, l'ecran rendu de `Place::Accueil`) avec le geste qui les deplace
//! (`logiciel-deplacer`, sa charge : le dossier). Rien ne se deplace d'office. Une ancienne installation, cachee dans le
//! dossier de l'utilisateur, voit en plus ce qu'il en est et le geste « Ranger dans Documents ». Ce que le dernier
//! deplacement est devenu (ou ce qui l'a empeche) se redit en encart ; illisible, l'encart le dit, et la lecture repond
//! quand meme. Les dossiers se disent dans les lignes du rang, jamais dans une phrase (un nom de dossier n'est pas fait
//! pour une phrase : `affiche::defaut`).
//!
//! DEPLACER SE PROUVE COMME UN ACTE REEL : le mot de passe du logiciel, tape par une personne au moment du geste (dans
//! la page, la boite du mot de passe ; par `moteur route`, le moteur le demande lui-meme a un humain, ou refuse). Celui
//! d'une compagnie reelle du poste, la premiere des tuiles qui en a un (`qui_prouve`) : il n'y a pas de mot de passe de
//! l'installation. Aucune n'en a : rien ne le prouve, le geste n'est pas offert et la route refuse.
//!
//! ONEDRIVE se dit AVANT de partir (la fenetre se ferme des que le deplacement part) : un dossier qu'il synchronise est
//! refuse tant que la charge ne dit pas qu'on le sait (`onedrive`, une case que la boite ne montre que si OneDrive
//! synchronise un dossier du poste). « Ranger dans Documents » le dit dans sa boite, et sa charge le porte.

use std::path::{Path, PathBuf};

use serde::Serialize;

use compagnie::coffre::Coffre;
use compagnie::demande::DemandeDuMotDePasse;
use compagnie::deplacement::{dernier_ici, dit, racines_onedrive, ranger_dans_les_documents, synchronise_par_onedrive, Issue, Verdict, AVERTIR_ONEDRIVE};
use compagnie::installation::IdDeCompagnie;
use socle::echec::{erreur, refus, Classe, R};
use socle::ecran::{EtatDEcran, Pastille};
use socle::secret::Secret;

use crate::catalogue::{Acte, Affiche, Ecran, Fiche, Place, Refus, Route, OPERATION};
use crate::declaration::{charge, dit as legende};
use crate::ecran::{EcranRendu, RangDeLEcran};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Rendu};
use crate::imprevus::GesteDeRoute;
use crate::{Encart, SansCharge};

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::LogicielOu,
        geste: "Dire où sont le logiciel et ses compagnies, et le geste qui les déplace.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "phrase, dossier, encarts (le dernier déplacement ; une ancienne installation, cachée), rangs : où il est, les dossiers que OneDrive synchronise, et les gestes qui le déplacent quand une compagnie a un mot de passe du logiciel",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Lit,
        phrase: 200,
        ecran: Ecran::Rendu(Place::Accueil),
    },
    Fiche {
        route: Route::LogicielDeplacer,
        geste: "Déplacer le logiciel et ses compagnies, avec le mot de passe du logiciel.",
        charge: &ChargeDeplacer::CHARGE,
        exemple: r#"{"dossier":"D:\\PuppetPaie"}"#,
        rend: "phrase, dossier (où il est), vers (où il va) ; la fenêtre ouverte se ferme, et se rouvre au nouvel endroit",
        refus: &[
            Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe du logiciel d'une compagnie de ce poste, tapé par un humain ; rien n'a changé" },
            Refus { classe: Classe::Refuse, corriger: "un dossier neuf ou vide, dit en entier, où l'on écrit, avec la place, hors de OneDrive ou en le sachant ; rien n'a changé" },
        ],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Engage,
        phrase: 400,
        ecran: Ecran::Geste(Place::Accueil),
    },
];

const CHANGER: Affiche = Affiche::new("Changer de dossier");
const RANGER: Affiche = Affiche::new("Ranger dans Documents");
/// Le geste, tel que la demande du mot de passe le dit.
const GESTE_DEPLACER: &str = "déplacer le logiciel et ses compagnies dans un autre dossier";
/// Aucune compagnie du poste n'a de mot de passe du logiciel.
const SANS_MOT_DE_PASSE: &str = "Aucune compagnie de ce poste n'a de mot de passe du logiciel : rien ne prouve qu'une personne déplace le logiciel. Rien n'a changé.";

charge! {
    motdepasse = legende("Mot de passe du logiciel", "celui d'une compagnie de ce poste, tapé dans la page ; par moteur route, le moteur le demande lui-même à un humain");
    /// La charge de `logiciel-deplacer` : le dossier ou le logiciel va, et, sous OneDrive, qu'on le sait.
    #[derive(Debug, Clone, Serialize)]
    pub struct ChargeDeplacer {
        pub dossier: PathBuf = legende("Nouveau dossier", "le dossier où le logiciel et ses compagnies vont, dit en entier depuis son disque : neuf, ou vide"),
        #[serde(skip_serializing_if = "Option::is_none")]
        pub onedrive: Option<bool> = legende("Je sais que OneDrive le synchronise", "vrai : le dossier est synchronisé par OneDrive, et on l'y met en le sachant ; sans cela, un tel dossier est refusé"),
    }
}

/// CE QU'UN GESTE DE CET ECRAN PORTE : rien (les cases se saisissent dans sa boite), ou le dossier des Documents ou
/// ranger une ancienne installation.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum VersLeDossier {
    ASaisir(SansCharge),
    Dit(ChargeDeplacer),
}

/// Ce que rend `logiciel-ou`.
#[derive(Debug, Serialize)]
struct OuEstLeLogiciel {
    phrase: Phrase,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    dossier: PathBuf,
    #[serde(flatten)]
    ecran: EcranRendu<VersLeDossier>,
}

/// Ce que rend `logiciel-deplacer`.
#[derive(Debug, Serialize)]
struct Deplacement {
    phrase: Phrase,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    dossier: PathBuf,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin", skip_serializing_if = "Option::is_none")]
    vers: Option<PathBuf>,
}

impl Dite for OuEstLeLogiciel {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for Deplacement {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// Une route de ce module, sur l'installation `inst`.
pub fn route(inst: &Path, nom: Route, charge: &Charge) -> R<Rendu> {
    match nom {
        Route::LogicielOu => {
            let SansCharge {} = charge.lire()?;
            en_json(ou_est(inst))
        }
        Route::LogicielDeplacer => deplacer(inst, charge.lire()?, charge.mot_de_passe()),
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

/// LA COMPAGNIE DONT LE MOT DE PASSE PROUVE qu'une personne deplace le logiciel : la premiere compagnie reelle de
/// l'installation `inst`, dans l'ordre des tuiles, dont le coffre a un mot de passe du logiciel ; aucune : `None`.
fn qui_prouve(inst: &Path) -> R<Option<PathBuf>> {
    for (id, r) in compagnie::installation::compagnies(inst, |_| true)? {
        let reelle = match id {
            IdDeCompagnie::Fictive(_) => false,
            IdDeCompagnie::Reelle => r.join(socle::fichiers::CONFIG).is_dir(),
            IdDeCompagnie::Cie(_) => true,
        };
        if reelle && Coffre::du_poste(&r).ouverture_exigee()? {
            return Ok(Some(r));
        }
    }
    Ok(None)
}

/// La demande du mot de passe que le geste exige ; `None` : aucune compagnie n'en a.
fn demande(inst: &Path) -> R<Option<DemandeDuMotDePasse>> {
    qui_prouve(inst)?.map(|r| crate::coffre::demande(&r, GESTE_DEPLACER)).transpose()
}

/// LA PREUVE : le mot de passe que la page a fait taper (`mdp`), sinon le moteur le demande lui-meme a un humain ;
/// verifie sur le coffre de la compagnie qui prouve (`qui_prouve`).
fn prouver(inst: &Path, mdp: Option<&Secret>) -> R<()> {
    let Some(racine) = qui_prouve(inst)? else { return Err(erreur(Classe::PreuveInsuffisante, SANS_MOT_DE_PASSE)) };
    let demande = crate::coffre::demande(&racine, GESTE_DEPLACER)?;
    let tape = match mdp.filter(|m| !m.devoiler().is_empty()) {
        Some(m) => m.clone(),
        None => compagnie::demande::mot_de_passe_de_l_acte(&racine, &demande)
            .map_err(|e| e.reformuler(|p| format!("{} Rien n'a changé.", p)).reclasser(Classe::PreuveInsuffisante))?,
    };
    if !Coffre::du_poste(&racine).verifier_ouverture(&tape)? {
        return Err(erreur(Classe::PreuveInsuffisante, demande.refus("Rien n'a changé.")));
    }
    Ok(())
}

/// `logiciel-ou` : ou est le logiciel, ce que le dernier deplacement est devenu, et les gestes qui le deplacent. Ce qui
/// ne se lit pas se dit en encart : regarder n'est jamais refuse.
fn ou_est(inst: &Path) -> OuEstLeLogiciel {
    let mut encarts = Vec::new();
    let mut dit_du_rang = vec![format!("Ils sont dans : {}.", dit(inst))];
    match dernier_ici(inst) {
        Ok(Some(d)) => {
            let (etat, phrase) = match d.issue {
                Issue::Deplace => (EtatDEcran::Fait, "Le logiciel est arrivé ici, avec ses compagnies."),
                Issue::Reste => (EtatDEcran::Refuse, "Le logiciel n'a pas été déplacé : il est resté entier à son ancien endroit, et le rang ci-dessous dit pourquoi."),
                Issue::Coupe => (EtatDEcran::Refuse, "Le déplacement s'est arrêté en route, et tout n'est pas revenu : le rang ci-dessous dit ce qui est où."),
            };
            encarts.push(Encart::de(etat, format!("Le dernier déplacement, le {}", socle::paris::ecrit::saisie::texte(d.quand)), phrase));
            dit_du_rang.push(d.phrase);
        }
        Ok(None) => {}
        Err(e) => encarts.push(Encart::de(EtatDEcran::Refuse, "Le dernier déplacement", e.phrase)),
    }
    let mut gestes = Vec::new();
    if !cfg!(target_os = "wasi") {
        let onedrive = racines_onedrive();
        dit_du_rang.extend(onedrive.iter().map(|r| format!("OneDrive synchronise : {}.", dit(r))));
        match demande(inst) {
            Ok(Some(demande)) => {
                // Ce que la boite dit avant de partir : ce qui se passe, et ce que OneDrive ferait du dossier.
                let mut phrases = vec!["Le logiciel se ferme, s'y déplace avec ses compagnies, puis se rouvre là.".to_string()];
                let mut cases = vec!["dossier"];
                if !onedrive.is_empty() {
                    phrases.push(format!("{} Un tel dossier ne se choisit qu'en le sachant.", AVERTIR_ONEDRIVE));
                    cases.push("onedrive");
                }
                gestes.push(GesteDeRoute::avec_cases(Route::LogicielDeplacer, VersLeDossier::ASaisir(SansCharge {}), CHANGER, cases, SansCharge {}, SansCharge {}).disant(phrases).exigeant(Some(demande.clone())));
                match ranger_dans_les_documents(inst) {
                    Ok(Some(documents)) => {
                        encarts.push(Encart::de(
                            EtatDEcran::AFaire,
                            "Un dossier caché",
                            "Le logiciel est dans un dossier caché de Windows, que l'on ne retrouve pas dans l'explorateur. « Ranger dans Documents » le met dans le dossier Documents, avec ses compagnies.",
                        ));
                        dit_du_rang.push(format!("Ranger dans Documents le met dans : {}.", dit(&documents)));
                        let sous_onedrive = synchronise_par_onedrive(&documents).is_some();
                        let mut phrases = vec!["Le logiciel se ferme, va dans le dossier Documents avec ses compagnies, puis se rouvre là.".to_string()];
                        if sous_onedrive {
                            phrases.push(format!("{} Le dossier Documents de ce poste en est un.", AVERTIR_ONEDRIVE));
                        }
                        let charge = ChargeDeplacer { dossier: documents, onedrive: sous_onedrive.then_some(true) };
                        gestes.insert(0, GesteDeRoute::avec_cases(Route::LogicielDeplacer, VersLeDossier::Dit(charge), RANGER, Vec::new(), SansCharge {}, SansCharge {}).disant(phrases).exigeant(Some(demande)).principal());
                    }
                    Ok(None) => {}
                    Err(e) => encarts.push(Encart::de(EtatDEcran::Refuse, "Un dossier caché", format!("Le logiciel est dans un dossier caché, et le dossier Documents ne se lit pas : {}", e.phrase))),
                }
            }
            Ok(None) => dit_du_rang.push("Le déplacer demande le mot de passe du logiciel d'une compagnie de ce poste : aucune n'en a encore.".to_string()),
            Err(e) => encarts.push(Encart::de(EtatDEcran::Refuse, "Le déplacer", e.phrase)),
        }
    }
    let rang = RangDeLEcran { titre: "Le logiciel et ses compagnies".to_string(), dit: dit_du_rang, pastille: Pastille::de(EtatDEcran::Information, "sur ce poste"), etat: None, montre: None, gestes };
    OuEstLeLogiciel {
        phrase: Phrase::fixe("Le logiciel et ses compagnies : leur dossier sur ce poste, et le geste qui les déplace."),
        dossier: inst.to_path_buf(),
        ecran: EcranRendu { encarts, rangs: vec![rang], cases: Vec::new(), historique: Vec::new(), enregistrer: None },
    }
}

/// `logiciel-deplacer` sur l'installation `inst` : verifie ; possible, un dossier sous OneDrive se refuse tant que la
/// charge ne dit pas qu'on le sait, puis le mot de passe se prouve, et la demande se pose
/// (`compagnie::deplacement::demander`). Deja la : rien ne change, rien n'est exige.
fn deplacer(inst: &Path, c: ChargeDeplacer, mdp: Option<&Secret>) -> R<Rendu> {
    let avant = |vers: &Path| -> R<()> {
        if synchronise_par_onedrive(vers).is_some() && c.onedrive != Some(true) {
            return Err(refus(format!("{} Pour l'y mettre quand même, dire « Je sais que OneDrive le synchronise ». Rien n'a changé.", AVERTIR_ONEDRIVE)));
        }
        prouver(inst, mdp)
    };
    let verdict = compagnie::deplacement::demander(inst, &c.dossier, avant).map_err(|e| match e.classe {
        Some(Classe::PreuveInsuffisante) => e,
        _ => e.reclasser(Classe::Refuse),
    })?;
    match verdict {
        Verdict::DejaLa => en_json(Deplacement { phrase: Phrase::fixe("Le logiciel est déjà dans ce dossier. Rien ne change."), dossier: inst.to_path_buf(), vers: None }),
        Verdict::Possible(v) => en_json(Deplacement {
            phrase: Phrase::fixe("Le logiciel va dans le dossier demandé. Il se ferme, s'y déplace avec ses compagnies, puis se rouvre là ; s'il n'est pas ouvert, ce sera à son prochain lancement."),
            dossier: inst.to_path_buf(),
            vers: Some(v),
        }),
    }
}
