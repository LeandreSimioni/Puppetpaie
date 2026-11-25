//! OU EST LE LOGICIEL, ET LE DEPLACER, A L'ACCUEIL (`compagnie::deplacement`) : l'accueil dit d'emblee ou sont le
//! logiciel et ses compagnies (`logiciel-ou`, l'ecran rendu de `Place::Accueil`) avec le geste qui les deplace
//! (`logiciel-deplacer`, sa charge : le dossier). Rien ne se deplace d'office. Une ancienne installation, cachee dans le
//! dossier de l'utilisateur, voit en plus ce qu'il en est et le geste « Ranger dans Documents ». Ce que le dernier
//! deplacement est devenu (ou ce qui l'a empeche) se redit en encart ; illisible, l'encart le dit, et la lecture repond
//! quand meme. Les dossiers se disent dans les lignes du rang, jamais dans une phrase (un nom de dossier n'est pas fait
//! pour une phrase : `affiche::defaut`). Un deplacement EN COURS : la lecture repond pendant qu'il dure (la porte des
//! routes la laisse passer, `commande_et_route`) et dit ou il en est, sans offrir de geste. Un deplacement DEMANDE qui
//! attend (pose en ligne de commande, il part au prochain lancement) se dit, et s'annule par le meme geste, vers le
//! dossier ou le logiciel est deja (« deja la » efface la demande, sans rien exiger).
//!
//! LE MEME DISQUE SEULEMENT ; POUR UNE CLE USB, RENDRE PORTABLE (Leandre, 25/11/2026). Le deplacement ne va que sur le
//! disque ou est le logiciel ; a cote, le geste « Rendre portable » (`logiciel-rendre-portable`) met la cle du poste dans
//! le dossier du logiciel et pose le marqueur (`compagnie::coffre::rendre_portable`) : la personne copie ensuite elle-meme
//! le dossier sur la cle, et n'efface l'ancien qu'une fois la copie reussie. Deja portable, le geste n'est pas offert.
//!
//! DEPLACER SE PROUVE COMME UN ACTE REEL : le mot de passe du logiciel, tape par une personne au moment du geste (dans
//! la page, la boite du mot de passe ; par `moteur route`, le moteur le demande lui-meme a un humain, ou refuse). Celui
//! d'une compagnie reelle du poste, la premiere des tuiles qui en a un (`qui_prouve`) : il n'y a pas de mot de passe de
//! l'installation ; un coffre illisible passe la main a la suivante. Aucune n'en a : rien ne le prouve, le geste n'est
//! pas offert et la route refuse. LA DEMANDE DU MOT DE PASSE NOMME LE DOSSIER, en mots, et dit, sous OneDrive, ce qui
//! partirait dans le nuage (`geste_vers`) : celle de « Ranger dans Documents », et celle que le moteur pose lui-meme
//! (`moteur route`) ; celle de « Changer de dossier », rendue avant que le dossier soit saisi, renvoie a la boite.
//!
//! ONEDRIVE se dit AVANT de partir (la fenetre se ferme des que le deplacement part) : un dossier qu'il synchronise est
//! refuse tant que la charge ne dit pas qu'on le sait (`onedrive`, une case que la boite ne montre que si OneDrive
//! synchronise un dossier du poste). « Ranger dans Documents » le dit dans sa boite, et sa charge le porte.

use std::path::{Path, PathBuf};

use serde::Serialize;

use compagnie::coffre::Coffre;
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
        rend: "phrase, dossier, encarts (un déplacement en cours ou demandé ; le dernier ; une ancienne installation, cachée), rangs : où il est, les dossiers que OneDrive synchronise, et les gestes qui le déplacent ou annulent la demande",
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
            Refus { classe: Classe::Refuse, corriger: "un dossier neuf ou vide, dit en entier, sur le même disque, où l'on écrit, hors de OneDrive ou en le sachant ; pour une clé USB, rendre le logiciel portable puis copier son dossier soi-même ; rien n'a changé" },
        ],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Engage,
        phrase: 400,
        ecran: Ecran::Geste(Place::Accueil),
    },
    Fiche {
        route: Route::LogicielRendrePortable,
        geste: "Rendre le logiciel portable, avec le mot de passe du logiciel : la clé du poste qui ouvre les coffres va dans son dossier.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "phrase (ce qu'il reste à faire : copier soi-même le dossier sur une clé USB), dossier",
        refus: &[
            Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe du logiciel d'une compagnie de ce poste, tapé par un humain ; rien n'a changé" },
            Refus { classe: Classe::Refuse, corriger: "le dossier du logiciel porte déjà une autre clé du poste, ou ne s'écrit pas ; rien n'a changé" },
        ],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Engage,
        phrase: 400,
        ecran: Ecran::Geste(Place::Accueil),
    },
];

const CHANGER: Affiche = Affiche::new("Changer de dossier");
const RANGER: Affiche = Affiche::new("Ranger dans Documents");
const ANNULER: Affiche = Affiche::new("Annuler le déplacement demandé");
const PORTABLE: Affiche = Affiche::new("Rendre portable");
/// Le geste, tel que la demande du mot de passe le dit.
const GESTE_PORTABLE: &str = "rendre le logiciel portable : la clé de ce poste qui ouvre les coffres partira avec son dossier";
/// Ce qu'il reste a faire une fois le logiciel portable : la copie, a la main.
const COPIER_SOI_MEME: &str = "Copiez son dossier sur une clé USB, vérifiez que la copie a réussi, et seulement alors supprimez l'ancien.";
/// Le geste, tel que la demande du mot de passe le dit, quand le dossier se saisit dans la meme boite.
const GESTE_DEPLACER: &str = "déplacer le logiciel et ses compagnies dans le dossier saisi dans la boîte";
/// Aucune compagnie du poste n'a de mot de passe du logiciel.
const SANS_MOT_DE_PASSE: &str = "Aucune compagnie de ce poste n'a de mot de passe du logiciel : rien ne prouve qu'une personne fait ce geste. Rien n'a changé.";

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

/// Ce que rend `logiciel-rendre-portable`.
#[derive(Debug, Serialize)]
struct LogicielPortable {
    phrase: Phrase,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    dossier: PathBuf,
}

impl Dite for LogicielPortable {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
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
        Route::LogicielRendrePortable => {
            let SansCharge {} = charge.lire()?;
            rendre_portable(inst, charge.mot_de_passe())
        }
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

/// LA COMPAGNIE DONT LE MOT DE PASSE PROUVE qu'une personne deplace le logiciel : la premiere compagnie reelle de
/// l'installation `inst`, dans l'ordre des tuiles, dont le coffre a un mot de passe du logiciel ; aucune : `None`.
fn qui_prouve(inst: &Path) -> R<Option<PathBuf>> {
    // Un coffre qui ne se lit pas ne fait pas disparaitre le geste : la compagnie suivante sert. Aucune ne sert, et un
    // coffre ne se lisait pas : son refus.
    let mut illisible = None;
    for (id, r) in compagnie::installation::compagnies(inst, |_| true)? {
        let reelle = match id {
            IdDeCompagnie::Fictive(_) => false,
            IdDeCompagnie::Reelle => r.join(socle::fichiers::CONFIG).is_dir(),
            IdDeCompagnie::Cie(_) => true,
        };
        if !reelle {
            continue;
        }
        match Coffre::du_poste(&r).ouverture_exigee() {
            Ok(true) => return Ok(Some(r)),
            Ok(false) => {}
            Err(e) => {
                illisible.get_or_insert(e);
            }
        }
    }
    match illisible {
        Some(e) => Err(e),
        None => Ok(None),
    }
}

/// LE GESTE, tel que la demande du mot de passe le dit : le dossier ou le logiciel va (en mots), et, sous OneDrive, ce
/// qui partirait dans le nuage. Un dossier encore a saisir : le geste seul.
fn geste_vers(vers: Option<&Path>) -> String {
    match vers {
        None => GESTE_DEPLACER.to_string(),
        Some(v) => {
            let mut g = format!("déplacer le logiciel et ses compagnies dans : {}", dit(v));
            if synchronise_par_onedrive(v).is_some() {
                g.push_str(". OneDrive synchronise ce dossier : le coffre du logiciel, les paies et les numéros de sécurité sociale partiront dans le nuage de Microsoft");
            }
            g
        }
    }
}

/// LA PREUVE du `geste` (le deplacer vers un dossier, `geste_vers` ; le rendre portable) : le mot de passe que la page a
/// fait taper (`mdp`), sinon le moteur le demande lui-meme a un humain, en disant le geste ; verifie sur le coffre de la
/// compagnie qui prouve (`qui_prouve`).
fn prouver(inst: &Path, geste: &str, mdp: Option<&Secret>) -> R<()> {
    let Some(racine) = qui_prouve(inst)? else { return Err(erreur(Classe::PreuveInsuffisante, SANS_MOT_DE_PASSE)) };
    let demande = crate::coffre::demande(&racine, geste)?;
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
    // UN DEPLACEMENT EN COURS : la lecture repond, dit ou il en est et quoi faire ; aucun geste ne s'offre avant sa fin.
    let en_cours = compagnie::deplacement::en_cours_dit(inst);
    if let Some((phrase, lignes)) = &en_cours {
        encarts.push(Encart::de(EtatDEcran::AFaire, "Un déplacement en cours", phrase.clone()));
        dit_du_rang.extend(lignes.iter().cloned());
    }
    // UNE DEMANDE QUI ATTEND (posee en ligne de commande) : elle partira au prochain lancement ; elle s'annule (le meme
    // geste, vers le dossier ou le logiciel est deja : rien n'est exige).
    let attend = en_cours.is_none() && compagnie::deplacement::une_demande_attend(inst);
    if attend {
        let phrase = match compagnie::deplacement::demande_posee(inst) {
            Ok(Some(vers)) => {
                dit_du_rang.push(format!("Le déplacement demandé va dans : {}.", dit(&vers)));
                "Un déplacement du logiciel est demandé : il partira au prochain lancement du logiciel, qui se fermera pour se déplacer. Il s'annule ici.".to_string()
            }
            Ok(None) => "Un déplacement du logiciel est demandé : il partira au prochain lancement du logiciel. Il s'annule ici.".to_string(),
            Err(e) => format!("Un déplacement du logiciel est demandé, et sa demande ne se lit pas : {} Il s'annule ici.", e.phrase),
        };
        encarts.push(Encart::de(EtatDEcran::AFaire, "Un déplacement demandé", phrase));
        let annuler = ChargeDeplacer { dossier: inst.to_path_buf(), onedrive: None };
        gestes.push(GesteDeRoute::avec_cases(Route::LogicielDeplacer, VersLeDossier::Dit(annuler), ANNULER, Vec::new(), SansCharge {}, SansCharge {}).principal());
    }
    let portable = inst.join(compagnie::coffre::MARQUEUR_PORTABLE).is_file();
    if portable {
        dit_du_rang.push(format!("Il est portable. {}", COPIER_SOI_MEME));
    }
    if !cfg!(target_os = "wasi") && en_cours.is_none() {
        let onedrive = racines_onedrive();
        dit_du_rang.extend(onedrive.iter().map(|r| format!("OneDrive synchronise : {}.", dit(r))));
        match qui_prouve(inst) {
            Ok(Some(qui)) => {
                let demande = match crate::coffre::demande(&qui, &geste_vers(None)) {
                    Ok(d) => d,
                    Err(e) => {
                        encarts.push(Encart::de(EtatDEcran::Refuse, "Le déplacer", e.phrase));
                        return rendre(inst, encarts, dit_du_rang, gestes);
                    }
                };
                // Ce que la boite dit avant de partir : ce qui se passe, et ce que OneDrive ferait du dossier.
                let mut phrases = vec!["Le logiciel se ferme, s'y déplace avec ses compagnies, puis se rouvre là.".to_string()];
                let mut cases = vec!["dossier"];
                if !onedrive.is_empty() {
                    phrases.push(format!("{} Un tel dossier ne se choisit qu'en le sachant.", AVERTIR_ONEDRIVE));
                    cases.push("onedrive");
                }
                gestes.push(GesteDeRoute::avec_cases(Route::LogicielDeplacer, VersLeDossier::ASaisir(SansCharge {}), CHANGER, cases, SansCharge {}, SansCharge {}).disant(phrases).exigeant(Some(demande.clone())));
                // RENDRE PORTABLE, a cote : pour emporter le logiciel sur une cle USB, que la personne copie elle-meme.
                // Deja portable : le geste n'est pas offert.
                if !portable {
                    match crate::coffre::demande(&qui, GESTE_PORTABLE) {
                        Ok(d) => {
                            let phrases = vec![format!("La clé de ce poste qui ouvre les coffres va dans le dossier du logiciel, qui s'ouvre alors aussi sur un autre ordinateur. Ensuite : {}", COPIER_SOI_MEME)];
                            gestes.push(GesteDeRoute::avec_cases(Route::LogicielRendrePortable, VersLeDossier::ASaisir(SansCharge {}), PORTABLE, Vec::new(), SansCharge {}, SansCharge {}).disant(phrases).exigeant(Some(d)));
                        }
                        Err(e) => encarts.push(Encart::de(EtatDEcran::Refuse, "Le rendre portable", e.phrase)),
                    }
                }
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
                        // La demande du mot de passe nomme les Documents, et ce que OneDrive en ferait.
                        let demande = crate::coffre::demande(&qui, &geste_vers(Some(&documents)));
                        let charge = ChargeDeplacer { dossier: documents, onedrive: sous_onedrive.then_some(true) };
                        match demande {
                            Ok(demande) => {
                                let ranger = GesteDeRoute::avec_cases(Route::LogicielDeplacer, VersLeDossier::Dit(charge), RANGER, Vec::new(), SansCharge {}, SansCharge {}).disant(phrases).exigeant(Some(demande));
                                // Une demande qui attend garde le geste principal (l'annuler) ; sinon, ranger l'est.
                                match attend {
                                    true => gestes.push(ranger),
                                    false => gestes.insert(0, ranger.principal()),
                                }
                            }
                            Err(e) => encarts.push(Encart::de(EtatDEcran::Refuse, "Ranger dans Documents", e.phrase)),
                        }
                    }
                    Ok(None) => {}
                    Err(e) => encarts.push(Encart::de(EtatDEcran::Refuse, "Un dossier caché", format!("Le logiciel est dans un dossier caché, et le dossier Documents ne se lit pas : {}", e.phrase))),
                }
            }
            Ok(None) => dit_du_rang.push("Le déplacer ou le rendre portable demande le mot de passe du logiciel d'une compagnie de ce poste : aucune n'en a encore.".to_string()),
            Err(e) => encarts.push(Encart::de(EtatDEcran::Refuse, "Le déplacer", e.phrase)),
        }
    }
    rendre(inst, encarts, dit_du_rang, gestes)
}

/// La reponse de `logiciel-ou` : ses encarts, et un rang qui dit les dossiers et porte les gestes.
fn rendre(inst: &Path, encarts: Vec<Encart>, dit_du_rang: Vec<String>, gestes: Vec<GesteDeRoute<VersLeDossier, SansCharge, SansCharge>>) -> OuEstLeLogiciel {
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
        prouver(inst, &geste_vers(Some(vers)), mdp)
    };
    let demandee = compagnie::deplacement::une_demande_attend(inst);
    let verdict = compagnie::deplacement::demander(inst, &c.dossier, avant).map_err(|e| match e.classe {
        Some(Classe::PreuveInsuffisante) => e,
        _ => e.reclasser(Classe::Refuse),
    })?;
    match verdict {
        Verdict::DejaLa => en_json(Deplacement {
            phrase: Phrase::fixe(match demandee {
                true => "Le déplacement demandé est annulé : le logiciel reste dans son dossier.",
                false => "Le logiciel est déjà dans ce dossier. Rien ne change.",
            }),
            dossier: inst.to_path_buf(),
            vers: None,
        }),
        Verdict::Possible(v) => en_json(Deplacement {
            phrase: Phrase::fixe("Le logiciel va dans le dossier demandé. Il se ferme, s'y déplace avec ses compagnies, puis se rouvre là ; s'il n'est pas ouvert, ce sera à son prochain lancement."),
            dossier: inst.to_path_buf(),
            vers: Some(v),
        }),
    }
}

/// `logiciel-rendre-portable` sur l'installation `inst` : deja portable, rien ne change et rien n'est exige ; sinon le
/// mot de passe se prouve, puis la cle du poste va dans le dossier et le marqueur se pose
/// (`compagnie::coffre::rendre_portable`). La phrase dit ce qu'il reste a faire a la main : la copie.
fn rendre_portable(inst: &Path, mdp: Option<&Secret>) -> R<Rendu> {
    let deja = inst.join(compagnie::coffre::MARQUEUR_PORTABLE).is_file();
    if !deja {
        prouver(inst, GESTE_PORTABLE, mdp)?;
    }
    let rendu = compagnie::coffre::rendre_portable(inst).map_err(|e| match e.classe {
        Some(Classe::PreuveInsuffisante) => e,
        _ => e.reclasser(Classe::Refuse),
    })?;
    let phrase = match rendu {
        true => socle::phrase!(
            "Le logiciel est portable : la clé de ce poste qui ouvre ses coffres est dans son dossier, et le poste garde la sienne. {} Sur un autre ordinateur, le coffre, les mots de passe et le compte Google s'y ouvrent avec la même clé.",
            COPIER_SOI_MEME
        ),
        false => socle::phrase!("Le logiciel est déjà portable : rien ne change. {}", COPIER_SOI_MEME),
    };
    en_json(LogicielPortable { phrase, dossier: inst.to_path_buf() })
}
