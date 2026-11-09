//! Les routes du coffre et des mots de passe : ce qui est pose (`coffre-etat`, `google-etat`), poser ou oublier un
//! secret (`coffre-poser`, avec le compte DPAE pour l'Urssaf, `coffre-oublier`), lier ou delier le compte Google
//! (`google-autoriser`, `google-oublier`), changer ou retirer le mot de
//! passe du logiciel (`mot-de-passe-changer`, `mot-de-passe-retirer`), et ce que l'administrateur est pour la compagnie
//! (`administrateur-etat`, `administrateur-rattacher`, `administrateur-detacher` ; `gestes/administrateur.rs`).
//!
//! Aucune valeur de secret ne sort : la reponse est l'etat du coffre, des presences et des dates. Ces routes passent
//! par la porte du mot de passe du logiciel s'il est pose (`exiger_mot_de_passe`) ; le coffre lui-meme est
//! `coffre.rs`.
//!
//! UN SECRET NE SE LIT JAMAIS SUR L'ENTREE STANDARD. La page tape les siens dans ses cases et les met dans la charge ; par
//! `moteur route`, ils en sont retires (`SECRETS`, `service::commande`) et le moteur les demande lui-meme a un humain :
//! le mot de passe du logiciel par `demande::autoriser_acte`, un secret a poser par `demande::saisir_deux_fois` (sa
//! fenetre, sinon le terminal sans echo, sinon un refus).

use droit::fiche::Lue as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use compagnie::coffre::{Coffre, Echec as EchecCoffre};
use crate::gestes::administrateur::Poste;
use crate::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, OPERATION};
use crate::catalogue::Route;
use crate::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use crate::SansCharge;
use socle::ecran::Pastille;
use droit::contexte::Contexte;
use compagnie::coffre::vers_route;
use envois::google::{jeton_par_rafraichissement, Application, DELAI, PERMISSIONS};
use socle::echec::{erreur, Classe, Echec, R};
use socle::fichiers::MARQUEUR_COPIE;
use crate::catalogue::{Ecran, Place};

const SERVICE: Champ = Champ { nom: "service", libelle: Affiche::new("Service"), forme: Forme::Texte, sens: "net-entreprises, urssaf ou courriel", avance: false };
const MOT_DE_PASSE: Champ = Champ { nom: "motdepasse", libelle: Affiche::new("Mot de passe du logiciel"), forme: Forme::Texte, sens: "le mot de passe du logiciel, tapé dans la page ; par moteur route, le moteur le demande lui-même à un humain", avance: false };
const ADMINISTRATEUR: Champ = Champ { nom: "administrateur", libelle: Affiche::new("Mot de passe de l'administrateur"), forme: Forme::Texte, sens: "le mot de passe de l'administrateur, tapé dans la page ; par moteur route, le moteur le demande à un humain", avance: false };
const PREUVE: Refus = Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe du logiciel, tapé par un humain ; rien n'est modifié" };
const ETAT: &str = "l'état du coffre : ce qui est posé, depuis quand, jamais les valeurs";

/// Les secrets qu'une charge de ce module peut porter, retires de celle qui arrive par `moteur route` (le mot de passe
/// du logiciel, `motdepasse`, l'est de toutes).
pub(crate) const SECRETS: [&str; 4] = ["valeur", "nouveau", "confirmation", "administrateur"];


/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Coffre);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::CoffreEtat,
        geste: "Dire quels mots de passe sont posés, et depuis quand ; jamais leur valeur.",
        charge: &[],
        exemple: r#"{}"#,
        rend: ETAT,
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::GoogleEtat,
        geste: "Dire si le compte Google de la sauvegarde est lié.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "lie, compte, et ce que la page en montre",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::CoffrePoser,
        geste: "Poser le mot de passe d'un organisme ; pour l'Urssaf, avec le compte des DPAE.",
        charge: &[
            SERVICE,
            Champ { nom: "valeur", libelle: Affiche::new("Mot de passe"), forme: Forme::Texte, sens: "le mot de passe à poser, tapé dans la page ; par moteur route, le moteur le fait taper deux fois à un humain", avance: false },
            Champ { nom: "compte", libelle: Affiche::new("Identifiant du compte"), forme: Forme::Texte, sens: "l'identifiant du compte (l'adresse, pour la boîte courriel)", avance: false },
            Champ { nom: "nom", libelle: Affiche::new("Nom"), forme: Forme::Texte, sens: "Urssaf : le nom du compte qui dépose les DPAE ; s'il change, la route ne pose que le compte quand `valeur` manque", avance: false },
            Champ { nom: "prenom", libelle: Affiche::new("Prénom"), forme: Forme::Texte, sens: "Urssaf : son prénom", avance: false },
            MOT_DE_PASSE,
        ],
        exemple: r#"{"service":"urssaf"}"#,
        rend: "l'état du coffre, et ce qu'une tentative de connexion en dit : phrase, couleur",
        refus: &[PREUVE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 400,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::CoffreVerifier,
        geste: "Essayer une fois le mot de passe posé d'un organisme.",
        charge: &[SERVICE],
        exemple: r#"{"service":"urssaf"}"#,
        rend: "service, ok, message (la première ligne de l'erreur), conseil, pastille",
        refus: &[Refus { classe: Classe::Refuse, corriger: "poser d'abord le mot de passe de ce service" }],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::CoffreOublier,
        geste: "Retirer du coffre le mot de passe d'un organisme.",
        charge: &[SERVICE, MOT_DE_PASSE],
        exemple: r#"{"service":"urssaf"}"#,
        rend: ETAT,
        refus: &[PREUVE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::MotDePasseChanger,
        geste: "Poser ou changer le mot de passe du logiciel.",
        charge: &[
            MOT_DE_PASSE,
            Champ { nom: "nouveau", libelle: Affiche::new("Nouveau mot de passe"), forme: Forme::Texte, sens: "le nouveau mot de passe, tapé dans la page ; par moteur route, le moteur le fait taper deux fois à un humain", avance: false },
            Champ { nom: "confirmation", libelle: Affiche::new("Confirmation"), forme: Forme::Texte, sens: "le même, retapé dans la page", avance: false },
        ],
        exemple: r#"{}"#,
        rend: ETAT,
        refus: &[PREUVE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::MotDePasseRetirer,
        geste: "Retirer le mot de passe du logiciel.",
        charge: &[MOT_DE_PASSE],
        exemple: r#"{}"#,
        rend: ETAT,
        refus: &[PREUVE, OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::GoogleAutoriser,
        geste: "Lier le compte Google de la sauvegarde : Google s'ouvre, la personne accepte.",
        charge: &[MOT_DE_PASSE],
        exemple: r#"{}"#,
        rend: "compte : l'adresse du compte Google lié ; la fenêtre de Google s'ouvre dans le navigateur, son adresse est dite sur la sortie d'erreur",
        refus: &[PREUVE, Refus { classe: Classe::Refuse, corriger: "ce que la phrase dit (Google a refusé, aucune réponse en cinq minutes, l'application absente) ; rien n'est enregistré" }],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::GoogleOublier,
        geste: "Délier le compte Google de la sauvegarde : le jeton du poste s'efface.",
        charge: &[MOT_DE_PASSE],
        exemple: r#"{}"#,
        rend: "l'état du compte Google",
        refus: &[PREUVE, OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::AdministrateurEtat,
        geste: "Dire ce que l'administrateur est pour cette compagnie.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "compagnies (celles de l'administrateur), rattachee, autres, peut_rattacher, question (« même administrateur ? »), mot_de_passe_pose, pastille",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Lecture,
    },
    Fiche {
        route: Route::AdministrateurRattacher,
        geste: "Rattacher cette compagnie à l'administrateur : son mot de passe l'ouvre aussi.",
        charge: &[
            ADMINISTRATEUR,
            Champ { nom: "motdepasse", libelle: Affiche::new("Mot de passe actuel"), forme: Forme::Texte, sens: "le mot de passe actuel de cette compagnie, s'il est posé ; par moteur route, le moteur le demande à un humain", avance: false },
        ],
        exemple: r#"{}"#,
        rend: "compagnies (celles de l'administrateur), etat, phrase",
        refus: &[PREUVE, OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 200,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::AdministrateurDetacher,
        geste: "Détacher cette compagnie : elle reçoit son propre mot de passe.",
        charge: &[
            ADMINISTRATEUR,
            Champ { nom: "nouveau", libelle: Affiche::new("Nouveau mot de passe"), forme: Forme::Texte, sens: "son propre mot de passe, tapé dans la page ; par moteur route, le moteur le fait taper deux fois à un humain", avance: false },
            Champ { nom: "confirmation", libelle: Affiche::new("Confirmation"), forme: Forme::Texte, sens: "le même, retapé dans la page", avance: false },
        ],
        exemple: r#"{}"#,
        rend: "etat, phrase",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 120,
        ecran: ECRAN,
    },
];

// ------------------------------------------------------------------ les charges

/// La charge qui nomme un service du coffre.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ChargeService {
    service: String,
}

/// La charge de `coffre-poser` : le service, le secret (absent : un humain le tape), l'adresse de la boite courriel, et
/// pour l'Urssaf le nom et le prenom du compte qui depose les DPAE.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargePoser {
    service: String,
    valeur: Option<String>,
    compte: Option<String>,
    nom: Option<String>,
    prenom: Option<String>,
}

/// La charge de `mot-de-passe-changer` : le nouveau mot de passe et sa confirmation (absents : un humain le tape deux
/// fois).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeChanger {
    nouveau: Option<String>,
    confirmation: Option<String>,
}

/// LA CHARGE DE `administrateur-rattacher` ET `administrateur-detacher` : le mot de passe de l'administrateur (absent : un
/// humain le tape) ; pour se detacher, le nouveau mot de passe de la compagnie et sa confirmation.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeAdministrateur {
    pub administrateur: Option<String>,
    pub nouveau: Option<String>,
    pub confirmation: Option<String>,
}

// ------------------------------------------------------------------ ce que les routes rendent

/// Ce que la page lit d'une compagnie fictive, ou rien ne se branche.
#[derive(Debug, Serialize)]
struct Fictive {
    google: Pastille,
}

/// LE COMPTE QU'UN MOT DE PASSE OUVRE (`identifiant_du_compte`) : en clair, ce qui en manque, et s'il se saisit sur cette
/// ligne (`ici`).
#[derive(Debug, Serialize)]
pub(crate) struct Identifiant {
    texte: Option<String>,
    pub(crate) manque: Vec<&'static str>,
    ici: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    incomplet_dit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ou: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manque_dit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nom: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prenom: Option<String>,
}

/// Un service du coffre, tel que l'ecran le montre : ce que `compagnie::coffre` en dit (jamais la valeur), le compte qu'il
/// ouvre, son bouton et sa pastille.
#[derive(Debug, Serialize)]
pub(crate) struct ServiceDit {
    pub(crate) service: &'static str,
    pub(crate) libelle: &'static str,
    pub(crate) sert_a: &'static str,
    pub(crate) pose: bool,
    pose_le: Option<String>,
    pub(crate) environnement: bool,
    compte: Option<String>,
    avec_compte: bool,
    pub(crate) identifiant: Identifiant,
    bouton: &'static str,
    pastille: Pastille,
    #[serde(skip_serializing_if = "Option::is_none")]
    oublier: Option<String>,
}

/// L'ETAT DU COFFRE : ce qui est pose, pour quel usage, depuis quand ; jamais les valeurs.
#[derive(Debug, Serialize)]
pub(crate) struct EtatDuCoffre {
    phrase: String,
    pub(crate) services: Vec<ServiceDit>,
    mot_de_passe_pose: bool,
    bouton_mot_de_passe: &'static str,
    cle_poste: bool,
    chemin_cle: String,
    fictive: Fictive,
    pastille_mot_de_passe: Pastille,
    #[serde(skip_serializing_if = "Option::is_none")]
    couleur: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ecran: Option<EcranDuCoffre>,
}

impl EtatDuCoffre {
    /// La phrase de l'etat : `fait` (le geste fait), puis chaque service et ce que l'ecran en dit.
    fn dit(mut self, fait: &str) -> EtatDuCoffre {
        let services: Vec<String> = self.services.iter().map(|s| format!("{} : {}", s.libelle, s.pastille.mots)).collect();
        self.phrase = format!("{}{}.", fait, services.join(" ; "));
        self
    }
}

/// Ce que rend `google-etat` : le compte lie, s'il repond, ce qu'il a accorde ; jamais le jeton.
#[derive(Debug, Serialize)]
struct EtatGoogle {
    phrase: String,
    application: Option<String>,
    permissions: Vec<String>,
    accordees: Vec<String>,
    autorise: bool,
    compte: Option<String>,
    verifie: bool,
    compte_dit: Option<String>,
    bouton: &'static str,
    probleme: Option<String>,
    pastille: Pastille,
}

/// Ce que rend `coffre-verifier` : ce que `compagnie::coffre_verifier` en dit, et sa phrase.
#[derive(Debug, Serialize)]
struct Verifie {
    phrase: String,
    #[serde(flatten)]
    verdict: compagnie::coffre_verifier::Verdict,
}

/// Ce que rend `google-oublier`.
#[derive(Debug, Serialize)]
struct GoogleOublie {
    phrase: &'static str,
    efface: bool,
}

/// Ce que rend `google-autoriser` : le compte lie.
#[derive(Debug, Serialize)]
struct GoogleLie {
    phrase: String,
    compte: String,
}

/// Ce que rend `administrateur-etat` : la phrase qui le dit, et l'etat.
#[derive(Debug, Serialize)]
struct EtatAdministrateurDit {
    phrase: String,
    #[serde(flatten)]
    etat: crate::gestes::administrateur::EtatAdministrateur,
}

/// Ce que rendent `administrateur-rattacher` et `administrateur-detacher`.
#[derive(Debug, Serialize)]
pub struct Rattachement {
    phrase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    compagnies: Option<Vec<String>>,
    etat: crate::gestes::administrateur::EtatAdministrateur,
}

macro_rules! dites {
    ($($t:ty),*) => {
        $(impl Dite for $t {
            fn phrase(&self) -> &str {
                &self.phrase
            }
        })*
    };
}
dites!(EtatDuCoffre, EtatGoogle, Verifie, GoogleLie, EtatAdministrateurDit, Rattachement);

impl Dite for GoogleOublie {
    fn phrase(&self) -> &str {
        self.phrase
    }
}

// ------------------------------------------------------------------ les routes

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    let mdp = charge.mot_de_passe();
    match porte.route {
        Route::CoffreEtat => {
            let SansCharge {} = charge.lire()?;
            let mut e = coffre_etat(ctx)?;
            e.ecran = Some(ecran_du_coffre(ctx, &mut e)?);
            let etat: EtatDuCoffre = e.dit("");
            en_json(etat)
        }
        Route::GoogleEtat => {
            let SansCharge {} = charge.lire()?;
            en_json(google_etat(ctx)?)
        }
        Route::CoffrePoser => en_json(coffre_poser(ctx, charge.lire()?, mdp)?),
        Route::CoffreVerifier => {
            let c: ChargeService = charge.lire()?;
            let verdict = compagnie::coffre_verifier::route(ctx, &c.service)?;
            let mots = &verdict.pastille.mots;
            // Un compte incomplet se dit avec ce qui lui manque et ou le donner (`identifiant_du_compte`).
            let manque = manque_du_compte(&identifiant_du_compte(&ctx.entreprise, &c.service)?);
            let conseil: String = verdict.conseil.iter().map(|c| format!(" {}", c)).collect();
            let phrase = match manque {
                Some(m) if !verdict.ok => format!("{} : {}, {}.", libelle_du_service(&c.service), mots, m),
                _ => format!("{} : {}. {}{}", libelle_du_service(&c.service), mots, verdict.message, conseil).trim().to_string(),
            };
            en_json(Verifie { phrase, verdict })
        }
        Route::CoffreOublier => {
            let etat: EtatDuCoffre = coffre_oublier(ctx, charge.lire()?, mdp)?.dit("Mot de passe retiré du coffre. ");
            en_json(etat)
        }
        Route::MotDePasseChanger => {
            let etat: EtatDuCoffre = mot_de_passe_changer(ctx, charge.lire()?, mdp)?.dit("Mot de passe du logiciel posé. ");
            en_json(etat)
        }
        Route::MotDePasseRetirer => {
            let SansCharge {} = charge.lire()?;
            let etat: EtatDuCoffre = mot_de_passe_retirer(ctx, mdp)?.dit("Mot de passe du logiciel retiré. ");
            en_json(etat)
        }
        Route::GoogleOublier => {
            let SansCharge {} = charge.lire()?;
            en_json(google_oublier(ctx, mdp)?)
        }
        Route::GoogleAutoriser => {
            let SansCharge {} = charge.lire()?;
            let compte = google_autoriser(ctx, mdp)?;
            en_json(GoogleLie { phrase: format!("Drive lié : {}. La prochaine ouverture sauvegarde.", compte), compte })
        }
        Route::AdministrateurEtat => {
            let SansCharge {} = charge.lire()?;
            let etat = administrateur_etat(&ctx.racine)?;
            en_json(EtatAdministrateurDit { phrase: format!("Mot de passe de la compagnie : {}.", etat.pastille.mots), etat })
        }
        Route::AdministrateurRattacher | Route::AdministrateurDetacher => en_json(administrateur(&ctx.racine, porte.route, charge.lire()?, mdp, &Poste::de_ce_poste())?),
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

// ------------------------------------------------------------------ l'administrateur

/// Ce que l'administrateur de l'installation est pour la compagnie `racine` (`administrateur::etat`).
pub fn administrateur_etat(racine: &Path) -> R<crate::gestes::administrateur::EtatAdministrateur> {
    let inst = crate::gestes::administrateur::installation_de(racine);
    crate::gestes::administrateur::etat(&inst, racine, &Poste::de_ce_poste()).map_err(Echec::from)
}

/// `administrateur-rattacher` et `administrateur-detacher`, sur le coffre de `poste` : tout se decide dans
/// `administrateur.rs`. Le mot de passe de l'administrateur vient de la charge (la page), sinon il est tape par un humain ;
/// `mdp`, le mot de passe actuel de la compagnie, tape pour la route.
pub fn administrateur(racine: &Path, nom: Route, charge: ChargeAdministrateur, mdp: Option<&str>, poste: &Poste) -> R<Rattachement> {
    use crate::gestes::administrateur as adm;
    let inst = adm::installation_de(racine);
    let admin = match charge.administrateur {
        Some(m) => m,
        None => compagnie::demande::saisir_secret(
            "PuppetPaie : le mot de passe de l'administrateur",
            &["Celui qui ouvre les compagnies de l'administrateur.".to_string(), "La frappe ne s'affiche pas.".to_string()],
            "mot de passe (rien ne s'affiche) : ",
        )
        .map_err(refus)?,
    };
    if nom == Route::AdministrateurRattacher {
        // Une compagnie qui a deja son mot de passe le donne aussi : c'est lui que celui de l'administrateur remplace.
        let actuel = match mdp {
            Some(m) => Some(m.to_string()),
            None if poste.coffre(racine).ouverture_exigee().map_err(en_operation)? => {
                exiger_mot_de_passe(racine, None, "rattacher cette compagnie à l'administrateur")?;
                actuel(None)
            }
            None => None,
        };
        let noms = adm::rattacher(&inst, racine, &admin, actuel.as_deref(), poste).map_err(refus)?;
        let phrase = format!("Compagnie rattachée à l'administrateur : {} s'ouvrent avec le même mot de passe.", en_liste(&noms));
        return Ok(Rattachement { phrase, compagnies: Some(noms), etat: adm::etat(&inst, racine, poste).map_err(Echec::from)? });
    }
    let lignes = ["Le NOUVEAU mot de passe de cette compagnie, à elle seule.".to_string(), "La frappe ne s'affiche pas.".to_string()];
    let nouveau = secret_a_poser(charge.nouveau, "PuppetPaie : son propre mot de passe", &lignes)?;
    // La page donne la confirmation ; un humain qui tape deux fois l'a deja confirme.
    let confirmation = match charge.confirmation {
        Some(c) => c,
        None => nouveau.clone(),
    };
    adm::detacher(&inst, racine, &admin, &nouveau, &confirmation, poste).map_err(refus)?;
    Ok(Rattachement { phrase: "Compagnie détachée : elle s'ouvre avec son propre mot de passe.".to_string(), compagnies: None, etat: adm::etat(&inst, racine, poste).map_err(Echec::from)? })
}

/// Des noms en une liste : « A », « A et B », « A, B et C ».
fn en_liste(n: &[String]) -> String {
    match n {
        [] => String::new(),
        [un] => un.clone(),
        [devant @ .., dernier] => format!("{} et {}", devant.join(", "), dernier),
    }
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}

/// `exiger_mot_de_passe` : le mot de passe du logiciel, s'il est pose, pour un acte qui ENGAGE sans rien envoyer. Decide
/// ICI, jamais par la page. `motdepasse` : celui que la page a fait taper pour la route.
pub(crate) fn exiger_mot_de_passe(racine: &Path, motdepasse: Option<&str>, quoi: &str) -> R<()> {
    let coffre = compagnie::coffre::Coffre::du_poste(racine, "");
    let vers = vers_route;
    if !coffre.ouverture_exigee().map_err(vers)? {
        return Ok(());
    }
    let Some(mdp) = motdepasse else {
        // Rien n'a ete tape pour cette charge : le moteur demande lui-meme a un humain (fenetre, terminal), ou il refuse.
        // Un « oui » ne laisse que l'ouverture du coffre pour la suite de la commande ; la seance ne vaut pas ici.
        let acte = compagnie::demande::Acte::nouveau(quoi, "", "");
        return match compagnie::demande::autoriser_acte(racine, &acte) {
            compagnie::demande::Verdict::Accepte => Ok(()),
            compagnie::demande::Verdict::Refuse(phrase) => Err(erreur(
                Classe::PreuveInsuffisante,
                format!("Un mot de passe du logiciel est pose : il faut le taper pour {}. {}", quoi, phrase.replace("Rien n'est parti", "Rien n'a ete modifie")),
            )),
        };
    };
    if !coffre.verifier_ouverture(mdp).map_err(vers)? {
        return Err(erreur(Classe::PreuveInsuffisante, "Mot de passe du logiciel refuse. Rien n'a ete modifie."));
    }
    Ok(())
}

/// Un secret a poser : celui de la charge (la page), sinon tape deux fois par un humain (`demande::saisir_deux_fois`).
fn secret_a_poser(donne: Option<String>, titre: &str, lignes: &[String]) -> R<String> {
    match donne {
        Some(v) => Ok(v),
        None => compagnie::demande::saisir_deux_fois(titre, lignes, "mot de passe (rien ne s'affiche) : ").map_err(refus),
    }
}

/// Le mot de passe actuel du logiciel : celui que la page a fait taper, sinon celui que l'humain vient de taper pour cette
/// route (`exiger_mot_de_passe`).
fn actuel(mdp: Option<&str>) -> Option<String> {
    match mdp {
        Some(m) => Some(m.to_string()),
        None => compagnie::coffre::mot_de_passe_retenu(),
    }
}

/// Le refus du coffre : `Coffre` se rend `Refuse` (la page le montre tel quel), tout le reste (`Disque`...) garde
/// sa classe.
fn en_operation(x: EchecCoffre) -> Echec {
    Echec { classe: Some(if x.classe == Classe::Coffre { Classe::Refuse } else { x.classe }), message: x.message }
}

/// Le refus du coffre tel quel (`coffre.oublier_ouverture` ne l'attrapait pas).
fn en_coffre(x: EchecCoffre) -> Echec {
    Echec { classe: Some(x.classe), message: x.message }
}

/// L'heure de Paris, a la minute.
fn maintenant() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_minutes(t)
}

fn coffre(ctx: &Contexte) -> Coffre {
    Coffre::du_poste(&ctx.racine, &maintenant())
}

// ------------------------------------------------------------------ le coffre

/// Le libelle d'un service du coffre, tel que l'ecran le dit (« Urssaf (DPAE) ») ; son nom s'il n'est pas connu.
fn libelle_du_service(nom: &str) -> String {
    match compagnie::coffre::SERVICES.iter().find(|s| s.nom == nom) {
        Some(s) => s.libelle.to_string(),
        None => nom.to_string(),
    }
}

/// Ce qui manque au compte d'un service (`identifiant_du_compte`), en mots, et ou le donner ; rien s'il est complet.
fn manque_du_compte(identifiant: &Identifiant) -> Option<String> {
    if identifiant.manque.is_empty() {
        return None;
    }
    let manque: Vec<&str> = identifiant
        .manque
        .iter()
        .map(|c| match *c {
            "siret" => "le SIRET",
            "nom" => "le nom",
            "prenom" => "le prénom",
            autre => autre,
        })
        .collect();
    let ou = match (identifiant.ou, identifiant.ici) {
        (Some(ou), _) => format!(" ({})", ou),
        (None, true) => " (à donner avec le mot de passe)".to_string(),
        (None, false) => String::new(),
    };
    Some(format!("il manque au compte {}{}", manque.join(", "), ou))
}

/// Range le mot de passe d'un organisme, ou de la boite courriel avec son adresse (`compte`) ; pour l'Urssaf, avec le
/// nom et le prenom du compte qui depose les DPAE (`nom`, `prenom`), ecrits dans la fiche entreprise s'ils changent. Un
/// seul geste, une seule preuve : tout se verifie avant d'ecrire. Rend l'etat du coffre, sans valeur. Rien a poser se
/// refuse avant la porte du mot de passe du logiciel : il n'y a rien a autoriser.
fn coffre_poser(ctx: &Contexte, c: ChargePoser, mdp: Option<&str>) -> R<EtatDuCoffre> {
    let service = compagnie::coffre::service_connu(&c.service).map_err(en_operation)?;
    let compte = if c.nom.is_some() || c.prenom.is_some() { compte_dpae_change(ctx, c.nom.as_deref(), c.prenom.as_deref())? } else { None };
    // Sans `valeur` (`moteur route`) : le compte seul s'il change, sinon le secret, tape par un humain apres la porte.
    let donne = c.valeur.as_deref().is_some_and(|v| !socle::texte::strip(v).is_empty());
    let tape = c.valeur.is_none() && compte.is_none();
    if !tape && !donne && compte.is_none() {
        return Err(refus(format!("Tapez le mot de passe {} à poser : rien n'a été posé.", service.nom)));
    }
    let pose_un_secret = tape || donne;
    exiger_mot_de_passe(&ctx.racine, mdp, if pose_un_secret { "poser un mot de passe" } else { "changer le compte qui depose les DPAE" })?;
    let lignes = [format!("Service : {}", service.libelle), "La frappe ne s'affiche pas : elle est scellée dans le coffre, nulle part ailleurs.".to_string()];
    let valeur = if pose_un_secret { Some(secret_a_poser(c.valeur, "PuppetPaie : poser un mot de passe", &lignes)?) } else { None };
    let valeur = valeur.filter(|v| !socle::texte::strip(v).is_empty());
    let entreprise = match compte {
        Some(propres) => ecrire_compte_dpae(ctx, propres)?,
        None => ctx.entreprise.clone(),
    };
    if let Some(v) = &valeur {
        coffre(ctx).poser(service.nom, v, c.compte.as_deref()).map_err(en_operation)?;
    }
    let etat = coffre_etat_de_la_fiche(&ctx.racine, &entreprise)?;
    // PUIS UNE TENTATIVE, UNE SEULE (`coffre_verifier`) : un mot de passe faux se voit maintenant, pas au depot. Pas si
    // l'identifiant est incomplet : l'echec serait certain, et chaque echec rapproche du blocage du compte.
    let quoi = format!("{} {} enregistré", if valeur.is_some() { "Mot de passe" } else { "Compte" }, libelle_du_service(service.nom));
    let manque = etat.services.iter().filter(|s| s.service == service.nom).find_map(|s| manque_du_compte(&s.identifiant));
    let (phrase, couleur) = match manque {
        Some(manque) => (format!("{} ; pas essayé : {}.", quoi, manque), "gris"),
        None => match compagnie::coffre_verifier::route(ctx, service.nom) {
            Ok(v) if v.ok => (format!("{}, et vérifié : la connexion passe.", quoi), "vert"),
            Ok(v) => {
                let conseil: String = v.conseil.iter().map(|c| format!(" — {}", c)).collect();
                (format!("{} ; connexion refusée : {}{}", quoi, v.message, conseil), "rouge")
            }
            Err(e) => (format!("{} ; vérification impossible : {}", quoi, e.phrase()), "rouge"),
        },
    };
    Ok(EtatDuCoffre { phrase, couleur: Some(couleur), ..etat })
}

/// `coffre-oublier` : retire un secret du coffre.
fn coffre_oublier(ctx: &Contexte, c: ChargeService, mdp: Option<&str>) -> R<EtatDuCoffre> {
    exiger_mot_de_passe(&ctx.racine, mdp, "retirer un mot de passe")?;
    let service = compagnie::coffre::service_connu(&c.service).map_err(en_operation)?;
    coffre(ctx).oublier(service.nom).map_err(en_operation)?;
    coffre_etat(ctx)
}

/// Le nom et le prenom du compte Urssaf qui depose les DPAE, tels que la charge les dit, s'ils changent ceux de la fiche ;
/// `None` s'ils sont ceux de la fiche. SANS ACCENTS, DANS LA CASSE TAPEE : la DUE MtoM refuse les accents. Un nom ou un
/// prenom qui manque se refuse.
fn compte_dpae_change(ctx: &Contexte, nom: Option<&str>, prenom: Option<&str>) -> R<Option<Vec<(&'static str, String)>>> {
    let actuel = ctx.entreprise.api_dpae.dite();
    // Sans accents, les blancs ramenes a un seul ; un texte qui ne dit rien ne dit pas le nom.
    let propre = |v: Option<&str>| v.map(|t| declarations::champs::sans_accents(t).split(socle::texte::blanc).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ")).filter(|t| !t.is_empty());
    let propres = [("nom", propre(nom)), ("prenom", propre(prenom))];
    let dans_la_fiche = |cle: &str| match (actuel, cle) {
        (Some(a), "nom") => a.nom.remplie().map(String::from),
        (Some(a), _) => a.prenom.remplie().map(String::from),
        (None, _) => None,
    };
    if propres.iter().all(|(k, v)| dans_la_fiche(k) == *v) {
        return Ok(None);
    }
    let mut dits = Vec::new();
    for (cle, v) in propres {
        match v {
            Some(v) => dits.push((cle, v)),
            None => {
                return Err(refus(format!(
                    "Le {} du compte Urssaf manque. Il est celui du compte ouvert sur urssaf.fr pour la DPAE -- pas forcement celui de la personne qui signe. Rien n'a ete modifie.",
                    cle
                )))
            }
        }
    }
    Ok(Some(dits))
}

/// Ecrit le compte DPAE dans la fiche entreprise, par l'ecrivain de la fiche (`fiches::poser_entreprise`), date du jour ;
/// les autres cles d'`api_dpae` sont gardees. Rend la fiche telle qu'elle vaut aujourd'hui.
fn ecrire_compte_dpae(ctx: &Contexte, propres: Vec<(&'static str, String)>) -> R<droit::fiche::ChampsEntreprise> {
    // Une fiche qui ne dit pas encore de compte DPAE en recoit un, avec ce nom et ce prenom.
    let mut cfg = match ctx.entreprise.api_dpae.dite() {
        Some(c) => c.clone(),
        None => droit::fiche::ApiDpae { ..Default::default() },
    };
    for (cle, v) in propres {
        let case = if cle == "nom" { &mut cfg.nom } else { &mut cfg.prenom };
        *case = Some(v);
    }
    crate::fiches::poser_entreprise(ctx, droit::fiche::ChangementEntreprise::api_dpae(Some(cfg)), Some(ctx.aujourdhui))?;
    Ok(droit::fiche::lire_fichier::<droit::fiche::FicheEntrepriseDatee>(&ctx.racine.join(droit::contexte::ENTREPRISE), droit::contexte::ENTREPRISE).map_err(Echec::from)?.au(&ctx.aujourdhui))
}

// ------------------------------------------------------------------ le mot de passe du logiciel

/// Pose ou change le mot de passe du logiciel. L'actuel est exige s'il existe, et il ouvre aussi le coffre quand la cle
/// de ce poste ne l'ouvre plus.
fn mot_de_passe_changer(ctx: &Contexte, c: ChargeChanger, mdp: Option<&str>) -> R<EtatDuCoffre> {
    exiger_mot_de_passe(&ctx.racine, mdp, "changer le mot de passe du logiciel")?;
    // La page tape le nouveau et sa confirmation dans ses cases ; par `moteur route`, l'humain le tape deux fois.
    let (nouveau, confirmation) = match (c.nouveau, c.confirmation) {
        (Some(n), Some(k)) => (n, k),
        (Some(_), None) => return Err(refus("La confirmation du nouveau mot de passe manque. Rien n'a ete modifie.")),
        (None, _) => {
            let lignes = ["Le NOUVEAU mot de passe du logiciel (pas celui d'un organisme).".to_string(), "La frappe ne s'affiche pas.".to_string()];
            let m = secret_a_poser(None, "PuppetPaie : mot de passe du logiciel", &lignes)?;
            (m.clone(), m)
        }
    };
    if nouveau != confirmation {
        return Err(refus("Les deux saisies du nouveau mot de passe different. Rien n'a ete modifie."));
    }
    let actuel = actuel(mdp);
    // UNE COMPAGNIE DE L'ADMINISTRATEUR : son mot de passe est celui de toutes ses compagnies, et il se change pour toutes a
    // la fois (`administrateur::changer`) ; l'actuel les ouvre chacune, tout se prepare puis tout se remplace.
    let inst = crate::gestes::administrateur::installation_de(&ctx.racine);
    if crate::gestes::administrateur::est_rattachee(&inst, &ctx.racine).map_err(Echec::from)? {
        let Some(actuel) = actuel else {
            return Err(refus("Tapez le mot de passe actuel de l'administrateur : le nouveau le remplace dans toutes ses compagnies. Rien n'a ete modifie."));
        };
        crate::gestes::administrateur::changer(&inst, &actuel, &nouveau, &confirmation, &crate::gestes::administrateur::Poste::de_ce_poste()).map_err(refus)?;
        return coffre_etat(ctx);
    }
    coffre(ctx).poser_ouverture(&nouveau, actuel.as_deref()).map_err(en_operation)?;
    coffre_etat(ctx)
}

/// Plus de mot de passe du logiciel. L'actuel est exige.
fn mot_de_passe_retirer(ctx: &Contexte, mdp: Option<&str>) -> R<EtatDuCoffre> {
    if crate::gestes::administrateur::est_rattachee(&crate::gestes::administrateur::installation_de(&ctx.racine), &ctx.racine).map_err(Echec::from)? {
        return Err(refus("Cette compagnie est geree par l'administrateur : son mot de passe ouvre aussi les autres. La detacher d'abord (elle recoit alors son propre mot de passe), puis le retirer. Rien n'a ete modifie."));
    }
    exiger_mot_de_passe(&ctx.racine, mdp, "retirer le mot de passe du logiciel")?;
    coffre(ctx).oublier_ouverture(actuel(mdp).as_deref()).map_err(en_coffre)?;
    coffre_etat(ctx)
}

// ------------------------------------------------------------------ Google

/// Efface le jeton local, sans reseau. L'acces reste accorde cote Google, et se retire sur myaccount.google.com.
fn google_oublier(ctx: &Contexte, mdp: Option<&str>) -> R<GoogleOublie> {
    exiger_mot_de_passe(&ctx.racine, mdp, "oublier le compte Google")?;
    let p = ctx.racine.join(JETON_GOOGLE);
    let efface = p.exists();
    if efface {
        std::fs::remove_file(&p).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&p, &e)))?;
    }
    Ok(GoogleOublie { phrase: "Compte Google délié : la sauvegarde ne va plus au Drive. L'accès accordé côté Google se retire sur myaccount.google.com.", efface })
}

/// L'application Google du logiciel, livree avec lui dans `google-application.json` (a cote du code), `client_id` et
/// `client_secret`. Le secret ne s'ecrit pas dans le source Rust.
const FICHIER_APPLICATION: &str = "google-application.json";

/// L'application Google propre a la compagnie, sous `config/`, et le jeton scelle.
const FICHIER_IDENTIFIANTS: &str = "google.json";
pub(crate) const FICHIER_JETON: &str = "google-jeton.bin";
/// Le jeton Google, sous la racine de la compagnie.
pub const JETON_GOOGLE: &str = "config/google-jeton.bin";

/// La description scellee avec le jeton.
pub(crate) const DESCRIPTION: &str = "moteur de paie - jeton Google";

fn config(racine: &Path) -> PathBuf {
    racine.join(socle::fichiers::CONFIG)
}

/// `ErreurGoogle` : la phrase est ecrite pour etre lue.
#[track_caller]
fn erreur_google(message: impl Into<String>) -> Echec {
    erreur(Classe::Google, message)
}

/// L'APPLICATION GOOGLE TELLE QU'ELLE S'ECRIT (`google.json` de la compagnie, `google-application.json` livre) : son
/// identifiant et son secret.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationEcrite {
    client_id: String,
    client_secret: String,
}

/// LE JETON GOOGLE TEL QU'IL SE SCELLE (`sceller_le_jeton`) : le rafraichissement, le compte, les permissions accordees.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JetonEcrit {
    rafraichissement: String,
    compte: String,
    permissions: Vec<String>,
}

/// L'application qui s'autorise, et d'ou elle vient (`google.json` de la compagnie, ou le logiciel).
pub(crate) fn identifiants(racine: &Path) -> R<(Application, String)> {
    let p = config(racine).join(FICHIER_IDENTIFIANTS);
    if p.exists() {
        // Un `google.json` illisible est un refus, pas un plantage.
        let a: ApplicationEcrite = socle::forme::lire_fichier(&p, "L'application Google de la compagnie (google.json) ne se lit pas : la corriger, ou la supprimer pour revenir à celle du logiciel.")
            .map_err(erreur_google)?;
        if a.client_id.is_empty() || a.client_secret.is_empty() {
            return Err(erreur_google(socle::echec::avec(
                "L'application Google de la compagnie est incomplète : il y manque son identifiant ou son secret. La supprimer suffit à revenir à celle du logiciel.",
                socle::echec::APart { chemin: Some(p.to_string_lossy().into_owned()), ..socle::echec::APart::default() },
            )));
        }
        return Ok((Application { client_id: a.client_id, client_secret: a.client_secret }, p.to_string_lossy().into_owned()));
    }
    let livre = socle::fichiers::dossier_du_code().join(FICHIER_APPLICATION);
    if !livre.exists() {
        return Err(erreur_google(socle::echec::avec(
            "L'application Google du logiciel n'est pas renseignée : elle est livrée avec chaque version publiée ; une copie de travail du dépôt ne l'a pas. Sinon, la compagnie peut poser sa propre application Google.",
            socle::echec::APart {
                chemin: Some(p.to_string_lossy().into_owned()),
                fichier: Some(livre.to_string_lossy().into_owned()),
                detail: Some("« Publier » la pose depuis le secret PUPPETPAIE_GOOGLE_APPLICATION ; celle de la compagnie s'écrit à `chemin` (client_id, client_secret).".to_string()),
                ..socle::echec::APart::default()
            },
        )));
    }
    let a: ApplicationEcrite = socle::forme::lire_fichier(&livre, "L'application Google livrée avec le logiciel ne se lit pas : le signaler au développeur.").map_err(erreur_google)?;
    Ok((Application { client_id: a.client_id, client_secret: a.client_secret }, "le logiciel".into()))
}

/// LE JETON GOOGLE DE LA COMPAGNIE : aucun (le Drive n'est pas lie), celui qui se lit, ou la phrase qui dit pourquoi il
/// ne se lit pas (le coffre qui refuse dit quoi faire ; le client Drive la rend telle quelle).
enum Jeton {
    Absent,
    Lu(JetonEcrit),
    Illisible(String),
}

impl Jeton {
    /// Le jeton scelle sous la racine de la compagnie.
    fn de(racine: &Path) -> Jeton {
        let p = config(racine).join(FICHIER_JETON);
        if !p.exists() {
            return Jeton::Absent;
        }
        match Jeton::ouvrir(racine, &p) {
            Ok(j) => Jeton::Lu(j),
            Err(phrase) => Jeton::Illisible(phrase),
        }
    }

    fn ouvrir(racine: &Path, p: &Path) -> Result<JetonEcrit, String> {
        // Une lecture impossible (droits) se range avec les refus du coffre : l'etat se lit.
        let brut = std::fs::read(p).map_err(|e| socle::echec::du_disque(p, &e))?;
        let clair = compagnie::coffre::Coffre::du_poste(racine, "").deproteger(&brut, DESCRIPTION).map_err(|e| {
            format!("{} Pour le jeton Google, le remede est : relier de nouveau le Drive (Configuration > Mots de passe).", e.message)
        })?;
        let texte = String::from_utf8(clair).map_err(|e| socle::echec::abime(p, e))?;
        socle::forme::lire_texte(&texte, "Le jeton Google ne se lit pas : relier de nouveau le Drive (Configuration › Mots de passe).")
    }
}

/// Y a-t-il un jeton utilisable, sans rien demander au reseau : un jeton qui se lit. Ne leve jamais (l'ouverture
/// l'appelle) : un jeton illisible, un coffre qui refuse, c'est « non ».
pub fn autorise(racine: &Path) -> bool {
    matches!(Jeton::de(racine), Jeton::Lu(_))
}

/// L'adresse du compte, sans reseau ; `None` si pas autorise (un jeton illisible ne l'est pas).
pub(crate) fn compte_autorise(racine: &Path) -> Option<String> {
    match Jeton::de(racine) {
        Jeton::Lu(j) => Some(j.compte),
        Jeton::Absent | Jeton::Illisible(_) => None,
    }
}

/// Le faux Google qui remplace Google : dans l'atelier, celui d'`envois::redirection` ; en essai, celui que l'essai
/// nomme (`VARIABLE_GOOGLE_ESSAIS`, une adresse locale seulement) ; `None` hors de l'un et de l'autre.
fn faux_google() -> Option<String> {
    #[cfg(feature = "atelier")]
    if let Some(b) = envois::redirection::base(envois::redirection::VARIABLE_GOOGLE) {
        return Some(b);
    }
    match std::env::var(VARIABLE_GOOGLE_ESSAIS) {
        Ok(b) if socle::garde_reseau::en_essai() && envois::redirection::est_locale(&b) => Some(b.trim_end_matches('/').to_string()),
        Ok(_) | Err(std::env::VarError::NotPresent) | Err(std::env::VarError::NotUnicode(_)) => None,
    }
}

/// Les adresses de Google, ou celles du faux Google (`faux_google`).
fn adresses_du_poste() -> envois::drive::Adresses {
    match faux_google() {
        Some(b) => envois::drive::Adresses::de_la_base(&b),
        None => envois::drive::Adresses::de_google(),
    }
}

/// LE CLIENT DU DRIVE DE LA COMPAGNIE : refuse une copie de developpement (qui ecrirait dans le Drive de la VRAIE
/// compagnie), lit l'application (`google.json` ou celle du logiciel), le jeton scelle par la cle du poste et l'adresse
/// du compte, et les passe au client (`drive::Client::nouveau`). Aucun appel reseau.
pub fn client_drive(racine: &Path) -> Result<envois::drive::Client, envois::drive::ErreurGoogle> {
    if let Some(refus) = refus_de_copie(racine) {
        return Err(refus.into());
    }
    let (application, _origine) = identifiants(racine)?;
    let (rafraichissement, compte) = match Jeton::de(racine) {
        Jeton::Absent => (Ok(None), None),
        Jeton::Lu(j) => (Ok(Some(j.rafraichissement)), Some(j.compte)),
        Jeton::Illisible(phrase) => (Err(phrase), None),
    };
    let autorisation = envois::drive::Autorisation { application, rafraichissement, compte };
    Ok(envois::drive::Client::nouveau(autorisation, adresses_du_poste()))
}

/// La variable d'un faux Google des essais : sa base locale (`http://127.0.0.1:<port>`), lue en essai seulement
/// (`garde_reseau::en_essai`, que Cargo pose).
pub const VARIABLE_GOOGLE_ESSAIS: &str = "PUPPETPAIE_ESSAIS_GOOGLE";

/// Les adresses de l'autorisation : celles de Google, ou celles du faux Google (`faux_google`).
fn adresses_de_l_autorisation() -> envois::google_autorisation::Adresses {
    match faux_google() {
        Some(b) => envois::google_autorisation::Adresses::de_la_base(&b),
        None => envois::google_autorisation::Adresses::de_google(),
    }
}

/// LIER LE COMPTE GOOGLE (`google-autoriser`) : le mot de passe du logiciel s'il est pose, puis l'autorisation
/// (`google_autorisation::autoriser`) : le moteur ouvre la fenetre de consentement de Google dans le navigateur du poste
/// et dit son adresse sur la sortie d'erreur (un terminal sans navigateur la copie), la personne accepte, Google renvoie
/// le code au moteur, qui scelle le jeton. Rend l'adresse du compte lie. La page et `moteur route` l'appellent pareil.
fn google_autoriser(ctx: &Contexte, mdp: Option<&str>) -> R<String> {
    exiger_mot_de_passe(&ctx.racine, mdp, "lier le compte Google")?;
    let (application, _) = identifiants(&ctx.racine).map_err(|e| Echec { classe: Some(Classe::Refuse), message: e.message })?;
    let dire_url = |u: &str| {
        eprintln!("La fenetre de Google, a ouvrir dans le navigateur si elle ne s'ouvre pas seule : {}", u);
        if let Err(e) = compagnie::poste::ouvrir_adresse(u) {
            eprintln!("Le navigateur ne s'ouvre pas : {}", e);
        }
    };
    let jeton = envois::google_autorisation::autoriser(&application, &adresses_de_l_autorisation(), envois::google_autorisation::DELAI_AUTORISATION, &dire_url, &|m| eprintln!("{}", m))?;
    sceller_le_jeton(&ctx.racine, &jeton)
}

/// Le jeton que l'autorisation a obtenu (`google_autorisation::autoriser`) et l'adresse du compte, dans un seul fichier
/// scelle par la cle du poste. Rend l'adresse du compte lie.
fn sceller_le_jeton(racine: &Path, jeton: &envois::google_autorisation::JetonGoogle) -> R<String> {
    let ecrit = JetonEcrit { rafraichissement: jeton.rafraichissement.clone(), compte: jeton.compte.clone(), permissions: jeton.permissions.clone() };
    let brut = socle::json_ascii::dumps(&ecrit, None, false, false);
    let scelle = compagnie::coffre::Coffre::du_poste(racine, "")
        .proteger(brut.as_bytes(), DESCRIPTION)
        .map_err(|e| erreur_google(format!("Le jeton n'a pas pu etre scelle, rien n'est enregistre : {}", e.message)))?;
    let dossier = config(racine);
    std::fs::create_dir_all(&dossier).map_err(|e| erreur_google(socle::echec::du_disque(&dossier, &e)))?;
    let chemin = dossier.join(FICHIER_JETON);
    std::fs::write(&chemin, scelle).map_err(|e| erreur_google(socle::echec::du_disque(&chemin, &e)))?;
    Ok(jeton.compte.clone())
}

/// Une permission, par son dernier mot (`.../drive.file` : `drive.file`).
fn courte(p: &str) -> String {
    match p.rsplit_once('/') {
        Some((_, fin)) => fin.to_string(),
        None => p.to_string(),
    }
}

/// Une copie de developpement n'ecrit jamais dans le Drive de la vraie compagnie. Le refus a sa classe,
/// `CopieDeDeveloppement`.
pub(crate) fn refus_de_copie(racine: &Path) -> Option<Echec> {
    Path::new(racine).join(MARQUEUR_COPIE).is_file().then(|| {
        erreur(
            Classe::CopieDeDeveloppement,
            "Copie de developpement : la sauvegarde ou l'archivage Drive ne part jamais d'ici. Les essais se font en \
             test ; pour le faire en vrai, ouvrir la vraie compagnie.",
        )
    })
}

/// `google-etat` : ce que la page dit du compte Google (lie, a quelle date), jamais le jeton.
fn google_etat(ctx: &Contexte) -> R<EtatGoogle> {
    // L'application ne sert qu'a rafraichir un jeton : sans Drive lie, son absence (une copie de travail du depot) n'est
    // pas un refus, et l'etat dit seulement « pas lie ». Un `config/google.json` pose mais illisible ou incomplet en
    // reste un : c'est un fichier de l'utilisateur, a corriger.
    let application = match identifiants(&ctx.racine) {
        Err(e) if config(&ctx.racine).join(FICHIER_IDENTIFIANTS).exists() => return Err(e),
        a => a,
    };
    let origine = match &application {
        Ok((_, o)) => Some(o.clone()),
        Err(Echec { .. }) => None,
    };
    let permissions: Vec<String> = PERMISSIONS.iter().map(|p| courte(p)).collect();
    let etat = |compte: Option<String>, verifie: bool, probleme: Option<String>, accordees: Vec<String>| -> EtatGoogle {
        // Ce que la carte dit du compte lie, son bouton et sa pastille : aucun Drive lie, a faire (la sauvegarde attend) ;
        // lie et l'autorisation tient, fait ; lie mais l'autorisation ne repond pas, a faire.
        let (compte_dit, pastille) = match (&compte, verifie, &probleme) {
            (None, _, _) => (None, Pastille::de("a_faire", "aucun Drive lié")),
            (Some(c), true, _) => (Some(c.clone()), Pastille::de("fait", "lié")),
            (Some(c), false, Some(p)) => (Some(format!("{} : {}", c, p)), Pastille::de("a_faire", "lié, mais")),
            (Some(c), false, None) => (Some(format!("{} : l'autorisation ne répond pas", c)), Pastille::de("a_faire", "lié, mais")),
        };
        EtatGoogle {
            phrase: format!("Google Drive : {}.", pastille.mots),
            application: origine.clone(),
            permissions: permissions.clone(),
            accordees,
            autorise: compte.is_some(),
            bouton: if compte.is_some() { "Relier le Drive" } else { "Lier le Drive" },
            compte,
            verifie,
            compte_dit,
            probleme,
            pastille,
        }
    };
    // Un jeton illisible, c'est « pas autorise ».
    let jeton = match Jeton::de(&ctx.racine) {
        Jeton::Lu(j) => j,
        Jeton::Absent | Jeton::Illisible(_) => return Ok(etat(None, false, None, Vec::new())),
    };
    let accordees = if jeton.permissions.is_empty() { permissions.clone() } else { jeton.permissions.iter().map(|p| courte(p)).collect() };
    // Une copie de developpement ne rafraichit pas le jeton (`client_drive` la refuse aussi).
    if let Some(refus) = refus_de_copie(&ctx.racine) {
        return Err(refus);
    }
    let (application, _) = application?;
    match jeton_par_rafraichissement(&adresses_du_poste().jetons, &application, &jeton.rafraichissement, DELAI)? {
        Ok(_) => Ok(etat(Some(jeton.compte), true, None, accordees)),
        Err(probleme) => Ok(etat(Some(jeton.compte), false, Some(probleme), accordees)),
    }
}

/// `identifiant_du_compte` : le compte que ce mot de passe ouvre, en clair ; ce qui en manque ; et s'il se saisit sur
/// cette ligne (`ici`).
fn identifiant_du_compte(ent: &droit::fiche::ChampsEntreprise, service: &str) -> R<Identifiant> {
    match service {
        "net-entreprises" => {
            let d = envois::transport::declarant(ent);
            let manque: Vec<&'static str> = [("siret", &d.siret), ("nom", &d.nom), ("prenom", &d.prenom)].into_iter().filter(|(_, v)| v.is_none()).map(|(c, _)| c).collect();
            let texte = match (&d.siret, &d.nom, &d.prenom) {
                (Some(siret), Some(nom), Some(prenom)) => Some(format!("compte de {} {}, SIRET {}", nom, prenom, siret)),
                _ => None,
            };
            let incomplet_dit = (!manque.is_empty()).then(|| format!("identifiant incomplet ({})", d.manquants().join(", ")));
            Ok(Identifiant { texte, manque, ici: false, incomplet_dit, ou: Some("fiche Entreprise › Qui les organismes contactent"), manque_dit: None, nom: None, prenom: None })
        }
        "urssaf" => {
            let cfg = ent.api_dpae.dite();
            let nom = cfg.and_then(|c| c.nom.remplie());
            let prenom = cfg.and_then(|c| c.prenom.remplie());
            let siret = match cfg.and_then(|c| c.siret.remplie()) {
                Some(s) => Some(s),
                None => ent.siret.portee(),
            };
            let cases = [("nom", "le nom", nom), ("prenom", "le prénom", prenom), ("siret", "le SIRET", siret)];
            let manque: Vec<&'static str> = cases.iter().filter(|(_, _, v)| v.is_none()).map(|(c, _, _)| *c).collect();
            // Ce qui manque, dit : le compte de la DPAE ne part pas sans lui.
            let noms_manquants: Vec<&str> = cases.iter().filter(|(_, _, v)| v.is_none()).map(|(_, dit, _)| *dit).collect();
            let manque_dit = (!manque.is_empty()).then(|| format!("le compte Urssaf n'est pas dit ({}) : la DPAE ne partira pas", noms_manquants.join(", ")));
            let texte = match (prenom, nom, siret) {
                (Some(prenom), Some(nom), Some(siret)) => Some(format!("compte de {} {}, SIRET {}", prenom, nom, siret)),
                _ => None,
            };
            Ok(Identifiant {
                texte,
                manque,
                ici: true,
                incomplet_dit: None,
                ou: None,
                manque_dit,
                nom: cfg.and_then(|c| c.nom.portee()).map(String::from),
                prenom: cfg.and_then(|c| c.prenom.portee()).map(String::from),
            })
        }
        _ => Ok(Identifiant { texte: None, manque: Vec::new(), ici: false, incomplet_dit: None, ou: None, manque_dit: None, nom: None, prenom: None }),
    }
}

/// `coffre_etat` : ce qui est pose, pour quel usage, depuis quand. Jamais les valeurs.
fn coffre_etat(ctx: &Contexte) -> R<EtatDuCoffre> {
    coffre_etat_de_la_fiche(&ctx.racine, &ctx.entreprise)
}

/// L'etat du coffre sur une fiche entreprise donnee : `coffre_poser` le relit sur la fiche qu'il vient d'ecrire, les
/// rappels y lisent les acces qui manquent (`routes::rappels`).
pub(crate) fn coffre_etat_de_la_fiche(racine: &Path, entreprise: &droit::fiche::ChampsEntreprise) -> R<EtatDuCoffre> {
    let coffre = compagnie::coffre::Coffre::du_poste(racine, "");
    let secrets = coffre.services().map_err(vers_route)?;
    // UNE COMPAGNIE DONT LES ENVOIS SONT JOUES n'a rien a poser : chaque pastille le dit, en gris.
    let simule = compagnie::bridage::branchement(racine)?.joue();
    let mut services = Vec::new();
    for s in secrets {
        // Sa pastille : simule, une information ; fourni par l'environnement (il passe devant le coffre), a voir ; pose,
        // fait ; absent, a poser.
        let pastille = if simule {
            Pastille::de("information", "simulé")
        } else if s.dans_environnement {
            Pastille::de("a_faire", "fourni par l'environnement")
        } else if s.pose {
            match s.pose_le.as_deref().and_then(|t| t.get(..10)) {
                Some(le) => Pastille::de("fait", &format!("posé le {}", socle::ecran::quand(le))),
                None => Pastille::de("fait", "posé"),
            }
        } else {
            Pastille::de("a_faire", "absent")
        };
        let identifiant = identifiant_du_compte(entreprise, s.service)?;
        services.push(ServiceDit {
            service: s.service,
            libelle: s.libelle,
            sert_a: s.sert_a,
            pose: s.pose,
            pose_le: s.pose_le,
            environnement: s.dans_environnement,
            compte: s.compte,
            avec_compte: s.avec_compte,
            bouton: if identifiant.ici { "Enregistrer" } else { "Poser" },
            identifiant,
            pastille,
            oublier: None,
        });
    }
    let mot_de_passe_pose = coffre.ouverture_exigee().map_err(vers_route)?;
    let cle_poste = coffre.cle_posee().map_err(vers_route)?;
    Ok(EtatDuCoffre {
        phrase: String::new(),
        services,
        mot_de_passe_pose,
        bouton_mot_de_passe: if mot_de_passe_pose { "Enregistrer" } else { "Poser" },
        cle_poste,
        chemin_cle: coffre.chemin_cle().to_string_lossy().into_owned(),
        // Ce que montrent le filet et le compte Google d'une compagnie fictive, ou rien ne se branche : une information.
        fictive: Fictive { google: Pastille::de("information", "non lié") },
        // La pastille du mot de passe du logiciel (rattache a l'administrateur, la page dit a combien de compagnies).
        pastille_mot_de_passe: if simule {
            Pastille::de("information", "simulé")
        } else if mot_de_passe_pose {
            Pastille::de("fait", "posé")
        } else {
            Pastille::de("a_faire", "aucun")
        },
        couleur: None,
        ecran: None,
    })
}

/// La ligne du mot de passe du logiciel, telle que l'ecran la dit.
#[derive(Debug, Serialize)]
struct LigneDuLogiciel {
    nom: &'static str,
    dit: String,
    nouveau: &'static str,
    retirer: &'static str,
}

/// La ligne de l'administrateur, quand la compagnie lui est rattachee ou peut l'etre.
#[derive(Debug, Serialize)]
struct LigneDeLAdministrateur {
    dit: String,
    note: &'static str,
}

/// CE QUE L'ECRAN DES MOTS DE PASSE DIT (`ecran` de `coffre-etat`).
#[derive(Debug, Serialize)]
struct EcranDuCoffre {
    intro: &'static str,
    encart: Option<super::Encart>,
    logiciel: LigneDuLogiciel,
    administrateur: Option<LigneDeLAdministrateur>,
    urssaf: &'static str,
    courriel: &'static str,
    verifier: &'static str,
    google: &'static str,
    google_ouvert: &'static str,
    google_oublier: &'static str,
    diagnostic: &'static str,
}

/// CE QUE L'ECRAN DES MOTS DE PASSE DIT (`ecran` de `coffre-etat`) : son introduction, l'encart d'une compagnie fictive ou
/// d'un poste sans cle, la ligne du mot de passe du logiciel et celle de l'administrateur, les notes des comptes Urssaf
/// et courriel, a quoi servent le compte Google et le filet, et ce que demandent les confirmations. La page n'en ecrit
/// rien ; chaque service porte aussi ce que sa confirmation d'oubli demande (`oublier`).
fn ecran_du_coffre(ctx: &Contexte, e: &mut EtatDuCoffre) -> R<EcranDuCoffre> {
    let fictive = compagnie::bridage::branchement(&ctx.racine)?.joue();
    let adm = if fictive { None } else { Some(administrateur_etat(&ctx.racine)?) };
    let (en_liste, rattachee, peut_rattacher) = match &adm {
        Some(a) => (a.autres_dit.clone(), a.rattachee, a.peut_rattacher),
        None => (String::new(), false, false),
    };
    for s in e.services.iter_mut() {
        s.oublier = Some(format!("Oublier le mot de passe {} ? Les retours et les dépôts ne seront plus possibles jusqu'à ce qu'il soit reposé.", s.libelle));
    }
    let encart = if fictive {
        Some(super::Encart::de("information", "Compagnie fictive : les champs sont grisés", "Les dépôts de DPAE et de DSN, leurs retours, les courriels et les signatures sont simulés : aucun mot de passe n'est demandé, et rien ne quitte le poste. Dans une compagnie réelle, c'est ici que se posent les accès à net-entreprises, à l'Urssaf et à la boîte courriel."))
    } else if !e.cle_poste {
        Some(super::Encart::de("information", "Cet ordinateur n'a pas encore sa clé du coffre", "Rien n'est perdu : elle se refait toute seule la prochaine fois que le mot de passe du logiciel est tapé (ou posé, ci-dessous). Elle vit à part des données, pour qu'une sauvegarde n'emporte jamais de quoi ouvrir le coffre."))
    } else {
        None
    };
    let demande = "demandé à l'ouverture et à chaque dépôt réel, envoi ou signature";
    Ok(EcranDuCoffre {
        intro: "Les mots de passe dont le logiciel a besoin pour déposer, relever et envoyer. La page ne voit jamais un mot de passe, seulement s'il est posé.",
        encart,
        logiciel: LigneDuLogiciel {
            nom: if rattachee { "Le mot de passe de l'administrateur" } else { "Le mot de passe du logiciel" },
            dit: if rattachee {
                format!("il ouvre aussi {} ; {}", en_liste, demande)
            } else if e.mot_de_passe_pose || fictive {
                demande.to_string()
            } else {
                "rien ne peut partir en réel tant qu'il n'est pas posé".to_string()
            },
            nouveau: if rattachee { "six caractères au moins ; il remplacera l'actuel dans toutes ses compagnies" } else { "six caractères au moins" },
            retirer: "Retirer le mot de passe du logiciel ? La fenêtre s'ouvrira sans rien demander et le coffre ne sera plus protégé.",
        },
        administrateur: if rattachee {
            Some(LigneDeLAdministrateur { dit: format!("cette compagnie s'ouvre avec {}, d'un seul mot de passe", en_liste), note: "Détachée, elle a son propre mot de passe : celui de l'administrateur ne l'ouvre plus, et elle dépose seule." })
        } else if peut_rattacher {
            Some(LigneDeLAdministrateur { dit: format!("le mot de passe de {} peut ouvrir aussi cette compagnie", en_liste), note: "Rattachée, elle s'ouvre avec le mot de passe de l'administrateur, et ses déclarations prêtes partent avec celles des autres compagnies, d'un seul mot de passe." })
        } else {
            None
        },
        urssaf: "Le compte ouvert sur urssaf.fr pour la DPAE (service « DPAE Service plus ») : ce n'est pas celui de net-entreprises. Les accents sont retirés, la DPAE les refuse. Un mot de passe vide garde celui qui est posé.",
        courriel: "Pour Gmail : validation en deux étapes activée, puis myaccount.google.com › Sécurité › Mots de passe des applications. C'est ce mot de passe-là, pas celui du compte.",
        verifier: "Une tentative de connexion chez l'organisme, une seule.",
        google: "la sauvegarde du dossier privé et l'archivage des mois, dans son Drive ; rien d'autre",
        google_ouvert: "La fenêtre Google est ouverte dans votre navigateur : choisissez le compte, acceptez, puis revenez ici ; cette ligne se met à jour toute seule. Si Google dit que l'application n'est pas validée : Paramètres avancés › Accéder à PuppetPaie.",
        google_oublier: "Oublier le compte Google ? Le jeton local est effacé : plus de sauvegarde ni d'archivage jusqu'à ce qu'un Drive soit relié. L'accès accordé côté Google, lui, se retire sur myaccount.google.com › Sécurité › Vos connexions à des applications tierces.",
        diagnostic: "ce qui casse part chez l'éditeur, avec de quoi le reproduire (ses données, les 3 minutes d'avant, cet ordinateur), et une fois par an le nombre de paies faites ; les noms, NIR, adresses et coordonnées des salariés n'en partent pas",
    })
}
