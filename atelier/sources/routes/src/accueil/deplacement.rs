//! OU EST LE LOGICIEL, ET LE DEPLACER, A L'ACCUEIL (`compagnie::deplacement`) : l'accueil dit d'emblee ou sont le
//! logiciel et ses compagnies (`logiciel-ou`, l'ecran rendu de `Place::Accueil`) avec le geste qui les deplace
//! (`logiciel-deplacer`, sa charge : le dossier). Rien ne se deplace d'office. Une ancienne installation, cachee dans le
//! dossier de l'utilisateur, voit en plus ce qu'il en est et le geste « Ranger dans Documents ». Un dossier que OneDrive
//! synchronise se dit dans la boite du geste, avant de partir. Ce que le dernier deplacement est devenu (ou ce qui l'a
//! empeche) se redit en encart ; illisible, l'encart le dit, et la lecture repond quand meme.

use std::path::{Path, PathBuf};

use serde::Serialize;

use compagnie::deplacement::{avertir_onedrive, dernier_ici, dit, racines_onedrive, ranger_dans_les_documents, synchronise_par_onedrive, Issue, Verdict};
use socle::echec::{Classe, R};
use socle::ecran::{EtatDEcran, Pastille};

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
        rend: "phrase, dossier, encarts (le dernier déplacement ; une ancienne installation, cachée), rangs : où il est, et les gestes qui le déplacent",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Lit,
        phrase: 200,
        ecran: Ecran::Rendu(Place::Accueil),
    },
    Fiche {
        route: Route::LogicielDeplacer,
        geste: "Déplacer le logiciel et ses compagnies dans un dossier, logiciel fermé.",
        charge: &ChargeDeplacer::CHARGE,
        exemple: r#"{"dossier":"D:\\PuppetPaie"}"#,
        rend: "phrase, dossier (où il est), vers (où il va) ; la fenêtre ouverte se ferme, et se rouvre au nouvel endroit",
        refus: &[Refus { classe: Classe::Refuse, corriger: "un dossier neuf ou vide, dit en entier, où l'on écrit, avec la place ; rien n'a changé" }],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 400,
        ecran: Ecran::Geste(Place::Accueil),
    },
];

const CHANGER: Affiche = Affiche::new("Changer de dossier");
const RANGER: Affiche = Affiche::new("Ranger dans Documents");

charge! {
    /// La charge de `logiciel-deplacer` : le dossier ou le logiciel va.
    #[derive(Debug, Clone, Serialize)]
    pub struct ChargeDeplacer {
        pub dossier: PathBuf = legende("Nouveau dossier", "le dossier où le logiciel et ses compagnies vont, dit en entier depuis son disque : neuf, ou vide"),
    }
}

/// CE QU'UN GESTE DE CET ECRAN PORTE : rien (la case du dossier se saisit dans sa boite), ou le dossier des Documents
/// ou ranger une ancienne installation.
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
        Route::LogicielDeplacer => deplacer(inst, charge.lire()?),
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

/// Ce que l'on sait avant de mettre le logiciel dans `vers` : un dossier que OneDrive synchronise.
fn onedrive(vers: &Path) -> Option<String> {
    synchronise_par_onedrive(vers).map(|r| avertir_onedrive(&r))
}

/// `logiciel-ou` : ou est le logiciel, ce que le dernier deplacement est devenu, et les gestes qui le deplacent. Ce qui
/// ne se lit pas se dit en encart : regarder n'est jamais refuse.
fn ou_est(inst: &Path) -> OuEstLeLogiciel {
    let mut encarts = Vec::new();
    match dernier_ici(inst) {
        Ok(Some(d)) => {
            let etat = match d.issue {
                Issue::Deplace => EtatDEcran::Fait,
                Issue::Reste | Issue::Coupe => EtatDEcran::Refuse,
            };
            encarts.push(Encart::de(etat, format!("Le dernier déplacement, le {}", socle::paris::ecrit::saisie::texte(d.quand)), d.phrase));
        }
        Ok(None) => {}
        Err(e) => encarts.push(Encart::de(EtatDEcran::Refuse, "Le dernier déplacement", e.phrase)),
    }
    let mut gestes = Vec::new();
    if !cfg!(target_os = "wasi") {
        // Ce que la boite dit avant de partir : ce qui se passe, et chaque dossier que OneDrive synchronise ici.
        let mut phrases = vec!["Le logiciel se ferme, s'y déplace avec ses compagnies, puis se rouvre là.".to_string()];
        phrases.extend(racines_onedrive().iter().map(|r| avertir_onedrive(r)));
        gestes.push(GesteDeRoute::avec_cases(Route::LogicielDeplacer, VersLeDossier::ASaisir(SansCharge {}), CHANGER, vec!["dossier"], SansCharge {}, SansCharge {}).disant(phrases));
        match ranger_dans_les_documents(inst) {
            Ok(Some(documents)) => {
                encarts.push(Encart::de(
                    EtatDEcran::AFaire,
                    "Un dossier caché",
                    format!("Le logiciel est dans un dossier caché de Windows, que l'on ne retrouve pas dans l'explorateur. « Ranger dans Documents » le met dans : {}, avec ses compagnies.", dit(&documents)),
                ));
                let mut phrases = vec![format!("Le logiciel se ferme, va dans {} avec ses compagnies, puis se rouvre là.", dit(&documents))];
                phrases.extend(onedrive(&documents));
                gestes.insert(0, GesteDeRoute::avec_cases(Route::LogicielDeplacer, VersLeDossier::Dit(ChargeDeplacer { dossier: documents }), RANGER, Vec::new(), SansCharge {}, SansCharge {}).disant(phrases).principal());
            }
            Ok(None) => {}
            Err(e) => encarts.push(Encart::de(EtatDEcran::Refuse, "Un dossier caché", format!("Le logiciel est dans un dossier caché, et le dossier Documents ne se lit pas : {}", e.phrase))),
        }
    }
    let ici = format!("Le logiciel et ses compagnies sont dans : {}.", dit(inst));
    let rang = RangDeLEcran { titre: "Le logiciel et ses compagnies".to_string(), dit: vec![ici.clone()], pastille: Pastille::de(EtatDEcran::Information, "sur ce poste"), etat: None, montre: None, gestes };
    OuEstLeLogiciel {
        phrase: socle::phrase!("{}", ici),
        dossier: inst.to_path_buf(),
        ecran: EcranRendu { encarts, rangs: vec![rang], cases: Vec::new(), historique: Vec::new(), enregistrer: None },
    }
}

/// `logiciel-deplacer` sur l'installation `inst` : verifie et pose la demande (`compagnie::deplacement::demander`).
fn deplacer(inst: &Path, c: ChargeDeplacer) -> R<Rendu> {
    match compagnie::deplacement::demander(inst, &c.dossier).map_err(|e| e.reclasser(Classe::Refuse))? {
        Verdict::DejaLa => en_json(Deplacement { phrase: socle::phrase!("Le logiciel est déjà dans : {}. Rien ne change.", dit(inst)), dossier: inst.to_path_buf(), vers: None }),
        Verdict::Possible(v) => {
            let suite = match onedrive(&v) {
                Some(o) => format!(" {}", o),
                None => String::new(),
            };
            en_json(Deplacement {
                phrase: socle::phrase!(
                    "Le logiciel va dans : {}. Il se ferme, s'y déplace avec ses compagnies, puis se rouvre là ; s'il n'est pas ouvert, ce sera à son prochain lancement.{}",
                    dit(&v),
                    suite
                ),
                dossier: inst.to_path_buf(),
                vers: Some(v),
            })
        }
    }
}
