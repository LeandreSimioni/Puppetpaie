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

use compagnie::coffre::Coffre;
use crate::gestes::administrateur::Poste;
use crate::catalogue::{Acte, Affiche, Fiche, Refus, OPERATION};
use crate::declaration::{charge, cle, dit, ChampDuType, ChargeDuType, Legende};
use crate::enveloppe::MotDePasseTape;
use crate::catalogue::Route;
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use crate::SansCharge;
use crate::ecran::{EcranRendu, RangDeLEcran};
use crate::imprevus::GesteDeRoute;
use droit::fiche::CaseDeFiche;
use socle::ecran::Pastille;
use droit::contexte::Contexte;
use envois::google::{jeton_par_rafraichissement, Application, DELAI, PERMISSIONS};
use socle::echec::{erreur, Classe, Echec, R};
use socle::fichiers::MARQUEUR_COPIE;
use crate::catalogue::{Ecran, Place};

const SERVICE: Legende = dit("Service", "le service du coffre");
const MOT_DE_PASSE: Legende = dit("Mot de passe du logiciel", "le mot de passe du logiciel, tapé dans la page ; par moteur route, le moteur le demande lui-même à un humain");
const ADMINISTRATEUR: Legende = dit("Mot de passe de l'administrateur", "le mot de passe de l'administrateur, tapé dans la page ; par moteur route, le moteur le demande à un humain");
const CONFIRMATION: Legende = dit("Confirmation", "le même, retapé dans la page");

// Deux charges que `charge!` ne peut pas dire : leur structure sert aussi une route qui ne prend pas le mot de passe tape
// (`coffre-verifier`, `administrateur-detacher`) ; leurs noms sont ecrits ici.
const CHARGE_OUBLIER: ChargeDuType = ChargeDuType {
    champs: &[ChampDuType::du_type::<_, &'static compagnie::coffre::Service>("service", SERVICE), ChampDuType::du_type::<_, MotDePasseTape>(crate::enveloppe::MOT_DE_PASSE, MOT_DE_PASSE)],
};
const CHARGE_RATTACHER: ChargeDuType = ChargeDuType {
    champs: &[
        ChampDuType::du_type::<_, Option<socle::secret::Secret>>("administrateur", ADMINISTRATEUR),
        ChampDuType::du_type::<_, MotDePasseTape>(crate::enveloppe::MOT_DE_PASSE, dit("Mot de passe actuel", "le mot de passe actuel de cette compagnie, s'il est posé ; par moteur route, le moteur le demande à un humain")),
    ],
};
/// Un service du coffre, nomme par une charge : la liste de ses services.
impl crate::declaration::ValeurDeLaCharge<crate::declaration::ParSaForme> for &'static compagnie::coffre::Service {
    const FORME: crate::catalogue::Forme = crate::catalogue::Forme::Choix(crate::catalogue::ListeDeChoix::de::<compagnie::coffre::Service>());
}

const PREUVE: Refus = Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe du logiciel, tapé par un humain ; rien n'est modifié" };
const ETAT: &str = "l'état du coffre : ce qui est posé, depuis quand, jamais les valeurs";
const ECRAN_REND: &str = "phrase, encarts, rangs : le mot de passe du logiciel, l'administrateur, chaque service du coffre et le compte Google, chacun son titre, ce qu'il dit, sa pastille et son état, et ses gestes (leurs cases, ce que leur boîte dit, la demande du mot de passe du logiciel quand il est posé) ; jamais une valeur";

/// Les secrets qu'une charge de ce module peut porter, retires de celle qui arrive par `moteur route` (le mot de passe
/// du logiciel, `motdepasse`, l'est de toutes).
pub(crate) const SECRETS: [&str; 4] = ["valeur", "nouveau", "confirmation", "administrateur"];


/// Ou ces routes se voient a l'ecran (une place par module) : `coffre-etat` le rend, ses rangs les rendent en gestes.
const GESTE: Ecran = Ecran::Geste(Place::Coffre);
const GESTE_AVANCE: Ecran = Ecran::GesteAvance(Place::Coffre);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::CoffreEtat,
        geste: "Dire quels mots de passe sont posés, et depuis quand ; jamais leur valeur.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: ECRAN_REND,
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Rendu(Place::Coffre),
    },
    Fiche {
        route: Route::GoogleEtat,
        geste: "Essayer une fois le compte Google de la sauvegarde.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "lie, compte, et ce que la page en montre",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: GESTE,
    },
    Fiche {
        route: Route::CoffrePoser,
        geste: "Poser le mot de passe d'un organisme ; pour l'Urssaf, avec le compte des DPAE.",
        charge: &ChargePoser::CHARGE,
        exemple: r#"{"service":"urssaf"}"#,
        rend: "l'état du coffre, et ce qu'une tentative de connexion en dit : phrase, couleur",
        refus: &[PREUVE, OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 400,
        ecran: GESTE,
    },
    Fiche {
        route: Route::CoffreVerifier,
        geste: "Essayer une fois le mot de passe posé d'un organisme.",
        charge: &ChargeService::CHARGE,
        exemple: r#"{"service":"urssaf"}"#,
        rend: "service, ok, message (la première ligne de l'erreur), conseil, pastille",
        refus: &[Refus { classe: Classe::Refuse, corriger: "poser d'abord le mot de passe de ce service" }],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: GESTE,
    },
    Fiche {
        route: Route::CoffreOublier,
        geste: "Retirer du coffre le mot de passe d'un organisme.",
        charge: &CHARGE_OUBLIER,
        exemple: r#"{"service":"urssaf"}"#,
        rend: ETAT,
        refus: &[PREUVE, OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: GESTE,
    },
    Fiche {
        route: Route::MotDePasseChanger,
        geste: "Poser ou changer le mot de passe du logiciel.",
        charge: &ChargeChanger::CHARGE,
        exemple: r#"{}"#,
        rend: ETAT,
        refus: &[PREUVE, OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: GESTE,
    },
    Fiche {
        route: Route::MotDePasseRetirer,
        geste: "Retirer le mot de passe du logiciel.",
        charge: &SeulLeMotDePasse::CHARGE,
        exemple: r#"{}"#,
        rend: ETAT,
        refus: &[PREUVE, OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: GESTE_AVANCE,
    },
    Fiche {
        route: Route::GoogleAutoriser,
        geste: "Lier le compte Google de la sauvegarde : Google s'ouvre, la personne accepte.",
        charge: &SeulLeMotDePasse::CHARGE,
        exemple: r#"{}"#,
        rend: "compte : l'adresse du compte Google lié ; la fenêtre de Google s'ouvre dans le navigateur, son adresse est dite sur la sortie d'erreur",
        refus: &[PREUVE, Refus { classe: Classe::Refuse, corriger: "ce que la phrase dit (Google a refusé, aucune réponse en cinq minutes, l'application absente) ; rien n'est enregistré" }],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: GESTE,
    },
    Fiche {
        route: Route::GoogleOublier,
        geste: "Délier le compte Google de la sauvegarde : le jeton du poste s'efface.",
        charge: &SeulLeMotDePasse::CHARGE,
        exemple: r#"{}"#,
        rend: "l'état du compte Google",
        refus: &[PREUVE, OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: GESTE_AVANCE,
    },
    Fiche {
        route: Route::AdministrateurEtat,
        geste: "Dire ce que l'administrateur est pour cette compagnie.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "compagnies (celles de l'administrateur), rattachee, autres, peut_rattacher, question (« même administrateur ? »), mot_de_passe_pose, pastille",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::LectureAvancee,
    },
    Fiche {
        route: Route::AdministrateurRattacher,
        geste: "Rattacher cette compagnie à l'administrateur : son mot de passe l'ouvre aussi.",
        charge: &CHARGE_RATTACHER,
        exemple: r#"{}"#,
        rend: "compagnies (celles de l'administrateur), etat, phrase",
        refus: &[PREUVE, OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 200,
        ecran: GESTE_AVANCE,
    },
    Fiche {
        route: Route::AdministrateurDetacher,
        geste: "Détacher cette compagnie : elle reçoit son propre mot de passe.",
        charge: &ChargeAdministrateur::CHARGE,
        exemple: r#"{}"#,
        rend: "etat, phrase",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 120,
        ecran: GESTE_AVANCE,
    },
];

// ------------------------------------------------------------------ les charges

charge! {
    /// La charge qui nomme un service du coffre.
    #[derive(Debug)]
    struct ChargeService {
        service: &'static compagnie::coffre::Service = SERVICE,
    }
}

charge! {
    motdepasse = MOT_DE_PASSE;
    /// La charge de `coffre-poser` : le service, le secret (absent : un humain le tape), l'adresse de la boite courriel, et
    /// pour l'Urssaf le nom et le prenom du compte qui depose les DPAE.
    #[derive(Debug)]
    struct ChargePoser {
        service: &'static compagnie::coffre::Service = SERVICE,
        valeur: Option<socle::secret::Secret> = dit("Mot de passe", "le mot de passe à poser, tapé dans la page ; par moteur route, le moteur le fait taper deux fois à un humain"),
        compte: Option<String> = dit("Identifiant du compte", "l'identifiant du compte (l'adresse, pour la boîte courriel)"),
        nom: Option<String> = dit("Nom", "Urssaf : le nom du compte qui dépose les DPAE ; s'il change, la route ne pose que le compte quand `valeur` manque"),
        prenom: Option<String> = dit("Prénom", "Urssaf : son prénom"),
    }
}

charge! {
    motdepasse = MOT_DE_PASSE;
    /// La charge de `mot-de-passe-changer` : le nouveau mot de passe et sa confirmation (absents : un humain le tape deux
    /// fois).
    #[derive(Debug)]
    struct ChargeChanger {
        nouveau: Option<socle::secret::Secret> = dit("Nouveau mot de passe", "le nouveau mot de passe, tapé dans la page ; par moteur route, le moteur le fait taper deux fois à un humain"),
        confirmation: Option<socle::secret::Secret> = CONFIRMATION,
    }
}

charge! {
    motdepasse = MOT_DE_PASSE;
    /// La charge d'une route qui ne prend que le mot de passe du logiciel.
    #[derive(Debug)]
    struct SeulLeMotDePasse {}
}

charge! {
    /// LA CHARGE DE `administrateur-rattacher` ET `administrateur-detacher` : le mot de passe de l'administrateur (absent : un
    /// humain le tape) ; pour se detacher, le nouveau mot de passe de la compagnie et sa confirmation.
    #[derive(Debug)]
    pub struct ChargeAdministrateur {
        pub administrateur: Option<socle::secret::Secret> = ADMINISTRATEUR,
        pub nouveau: Option<socle::secret::Secret> = dit("Nouveau mot de passe", "son propre mot de passe, tapé dans la page ; par moteur route, le moteur le fait taper deux fois à un humain"),
        pub confirmation: Option<socle::secret::Secret> = CONFIRMATION,
    }
}

// ------------------------------------------------------------------ ce que les routes rendent

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

/// Un service du coffre : ce que `compagnie::coffre` en dit (jamais la valeur), le compte qu'il ouvre et sa pastille.
#[derive(Debug, Serialize)]
pub(crate) struct ServiceDit {
    pub(crate) service: &'static str,
    pub(crate) libelle: &'static str,
    pub(crate) sert_a: &'static str,
    pub(crate) pose: bool,
    #[serde(with = "socle::paris::ecrit::saisie::option")]
    pose_le: Option<socle::paris::Moment>,
    pub(crate) environnement: bool,
    compte: Option<String>,
    avec_compte: bool,
    pub(crate) identifiant: Identifiant,
    pastille: Pastille,
}

/// L'ETAT DU COFFRE : ce qui est pose, pour quel usage, depuis quand ; jamais les valeurs.
#[derive(Debug, Serialize)]
pub(crate) struct EtatDuCoffre {
    phrase: Phrase,
    pub(crate) services: Vec<ServiceDit>,
    mot_de_passe_pose: bool,
    cle_poste: bool,
    chemin_cle: String,
    pastille_mot_de_passe: Pastille,
    #[serde(skip_serializing_if = "Option::is_none")]
    couleur: Option<&'static str>,
}

impl EtatDuCoffre {
    /// La phrase de l'etat : `fait` (le geste fait), puis chaque service et ce que l'ecran en dit.
    fn dit(mut self, fait: &str) -> EtatDuCoffre {
        let services: Vec<String> = self.services.iter().map(|s| format!("{} : {}", s.libelle, s.pastille.mots)).collect();
        self.phrase = socle::phrase!("{}{}.", fait, services.join(" ; "));
        self
    }
}

/// Ce que rend `google-etat` : le compte lie, s'il repond, ce qu'il a accorde ; jamais le jeton.
#[derive(Debug, Serialize)]
struct EtatGoogle {
    phrase: Phrase,
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
    phrase: Phrase,
    #[serde(flatten)]
    verdict: compagnie::coffre_verifier::Verdict,
}

/// Ce que rend `google-oublier`.
#[derive(Debug, Serialize)]
struct GoogleOublie {
    phrase: Phrase,
    efface: bool,
}

/// Ce que rend `google-autoriser` : le compte lie.
#[derive(Debug, Serialize)]
struct GoogleLie {
    phrase: Phrase,
    compte: String,
}

/// Ce que rend `administrateur-etat` : la phrase qui le dit, et l'etat.
#[derive(Debug, Serialize)]
struct EtatAdministrateurDit {
    phrase: Phrase,
    #[serde(flatten)]
    etat: crate::gestes::administrateur::EtatAdministrateur,
}

/// Ce que rendent `administrateur-rattacher` et `administrateur-detacher`.
#[derive(Debug, Serialize)]
pub struct Rattachement {
    phrase: Phrase,
    #[serde(skip_serializing_if = "Option::is_none")]
    compagnies: Option<Vec<String>>,
    etat: crate::gestes::administrateur::EtatAdministrateur,
}

macro_rules! dites {
    ($($t:ty),*) => {
        $(impl Dite for $t {
            fn phrase(&self) -> &Phrase {
                &self.phrase
            }
        })*
    };
}
dites!(EtatDuCoffre, EtatGoogle, Verifie, GoogleLie, EtatAdministrateurDit, Rattachement, CoffreDit);

impl Dite for GoogleOublie {
    fn phrase(&self) -> &Phrase {
        &self.phrase
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
            let etat: EtatDuCoffre = coffre_etat(ctx)?.dit("");
            let ecran = ecran_du_coffre(ctx, &etat)?;
            en_json(CoffreDit { phrase: etat.phrase, ecran })
        }
        Route::GoogleEtat => {
            let SansCharge {} = charge.lire()?;
            en_json(google_etat(ctx)?)
        }
        Route::CoffrePoser => en_json(coffre_poser(ctx, charge.lire()?, mdp)?),
        Route::CoffreVerifier => {
            let c: ChargeService = charge.lire()?;
            let c = c.service.nom;
            let verdict = compagnie::coffre_verifier::route(ctx, c)?;
            let mots = &verdict.pastille.mots;
            // Un compte incomplet se dit avec ce qui lui manque et ou le donner (`identifiant_du_compte`).
            let manque = manque_du_compte(&identifiant_du_compte(&ctx.entreprise, c)?);
            let conseil: String = verdict.conseil.iter().map(|c| format!(" {}", c)).collect();
            let phrase = match manque {
                Some(m) if !verdict.ok => socle::phrase!("{} : {}, {}.", libelle_du_service(c), mots, m),
                _ => socle::phrase!("{}", format!("{} : {}. {}{}", libelle_du_service(c), mots, verdict.message, conseil).trim()),
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
            let SeulLeMotDePasse {} = charge.lire()?;
            let etat: EtatDuCoffre = mot_de_passe_retirer(ctx, mdp)?.dit("Mot de passe du logiciel retiré. ");
            en_json(etat)
        }
        Route::GoogleOublier => {
            let SeulLeMotDePasse {} = charge.lire()?;
            en_json(google_oublier(ctx, mdp)?)
        }
        Route::GoogleAutoriser => {
            let SeulLeMotDePasse {} = charge.lire()?;
            let compte = google_autoriser(ctx, mdp)?;
            // La sauvegarde part au passage suivant de la veille (`interface::veille`, `Veille::tour`), ou a l'ouverture
            // suivante en ligne de commande : la premiere envoie tout le dossier.
            let phrase = socle::phrase!(
                "Drive lié : {}. La sauvegarde du Drive commence au prochain relevé. La première est longue : gardez une connexion internet stable et laissez le logiciel ouvert jusqu'à la fin.",
                compte
            );
            en_json(GoogleLie { phrase, compte })
        }
        Route::AdministrateurEtat => {
            let SansCharge {} = charge.lire()?;
            let etat = administrateur_etat(&ctx.racine)?;
            en_json(EtatAdministrateurDit { phrase: socle::phrase!("Mot de passe de la compagnie : {}.", etat.pastille.mots), etat })
        }
        Route::AdministrateurRattacher | Route::AdministrateurDetacher => en_json(administrateur(&ctx.racine, porte.route, charge.lire()?, mdp, &Poste::de_ce_poste())?),
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

// ------------------------------------------------------------------ l'administrateur

/// Ce que l'administrateur de l'installation est pour la compagnie `racine` (`administrateur::etat`).
pub fn administrateur_etat(racine: &Path) -> R<crate::gestes::administrateur::EtatAdministrateur> {
    let inst = compagnie::installation::IdDeCompagnie::installation_de(racine);
    crate::gestes::administrateur::etat(&inst, racine, &Poste::de_ce_poste())}

/// `administrateur-rattacher` et `administrateur-detacher`, sur le coffre de `poste` : tout se decide dans
/// `administrateur.rs`. Le mot de passe de l'administrateur vient de la charge (la page), sinon il est tape par un humain ;
/// `mdp`, le mot de passe actuel de la compagnie, tape pour la route.
pub fn administrateur(racine: &Path, nom: Route, charge: ChargeAdministrateur, mdp: Option<&socle::secret::Secret>, poste: &Poste) -> R<Rattachement> {
    use crate::gestes::administrateur as adm;
    let inst = compagnie::installation::IdDeCompagnie::installation_de(racine);
    let admin = match charge.administrateur {
        Some(m) => m,
        None => compagnie::demande::saisir_secret(
            "PuppetPaie : le mot de passe de l'administrateur",
            &["Celui qui ouvre les compagnies de l'administrateur.".to_string(), "La frappe ne s'affiche pas.".to_string()],
            "mot de passe (rien ne s'affiche) : ",
        )
        .map_err(|e| e.reclasser(Classe::Refuse))?,
    };
    if nom == Route::AdministrateurRattacher {
        // Une compagnie qui a deja son mot de passe le donne aussi : c'est lui que celui de l'administrateur remplace.
        let actuel = match mdp {
            Some(m) => Some(m.clone()),
            None if poste.coffre(racine).ouverture_exigee().map_err(en_operation)? => {
                exiger_mot_de_passe(racine, None, "rattacher cette compagnie à l'administrateur")?;
                actuel(None)
            }
            None => None,
        };
        let noms = adm::rattacher(&inst, racine, &admin, actuel.as_ref(), poste).map_err(|e| e.reclasser(Classe::Refuse))?;
        let phrase = socle::phrase!("Compagnie rattachée à l'administrateur : {} s'ouvrent avec le même mot de passe.", en_liste(&noms));
        return Ok(Rattachement { phrase, compagnies: Some(noms), etat: adm::etat(&inst, racine, poste)? });
    }
    let lignes = ["Le NOUVEAU mot de passe de cette compagnie, à elle seule.".to_string(), "La frappe ne s'affiche pas.".to_string()];
    let nouveau = secret_a_poser(charge.nouveau, "PuppetPaie : son propre mot de passe", &lignes)?;
    // La page donne la confirmation ; un humain qui tape deux fois l'a deja confirme.
    let confirmation = match charge.confirmation {
        Some(c) => c,
        None => nouveau.clone(),
    };
    adm::detacher(&inst, racine, &admin, &nouveau, &confirmation, poste).map_err(|e| e.reclasser(Classe::Refuse))?;
    Ok(Rattachement { phrase: Phrase::fixe("Compagnie détachée : elle s'ouvre avec son propre mot de passe."), compagnies: None, etat: adm::etat(&inst, racine, poste)? })
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

/// QUI OUVRE LE COFFRE DE `racine` : elle, ou l'administrateur auquel elle est rattachee (`administrateur::qui_ouvre`).
pub fn qui_ouvre(racine: &Path) -> R<compagnie::demande::QuiOuvre> {
    let inst = compagnie::installation::IdDeCompagnie::installation_de(racine);
    crate::gestes::administrateur::qui_ouvre(&inst, racine)
}

/// LA DEMANDE DU MOT DE PASSE DU LOGICIEL de `racine`, pour `geste` : la seule facon de la composer. La route qui rend un
/// geste a mot de passe la rend a la page ; la route qui le fait la pose elle-meme a un humain quand rien n'a ete tape.
pub fn demande(racine: &Path, geste: &str) -> R<compagnie::demande::DemandeDuMotDePasse> {
    Ok(compagnie::demande::DemandeDuMotDePasse::de(&qui_ouvre(racine)?, geste))
}

/// `exiger_mot_de_passe` : le mot de passe du logiciel, s'il est pose, pour un acte qui ENGAGE sans rien envoyer (`geste`,
/// tel que la demande le dit). Decide ICI, jamais par la page. `motdepasse` : celui que la page a fait taper pour la route.
pub(crate) fn exiger_mot_de_passe(racine: &Path, motdepasse: Option<&socle::secret::Secret>, geste: &str) -> R<()> {
    let coffre = compagnie::coffre::Coffre::du_poste(racine);
    if !coffre.ouverture_exigee()? {
        return Ok(());
    }
    let demande = demande(racine, geste)?;
    let Some(mdp) = motdepasse else {
        // Rien n'a ete tape pour cette charge : le moteur demande lui-meme a un humain (fenetre, terminal), ou il refuse.
        // Un « oui » ne laisse que l'ouverture du coffre pour la suite de la commande ; la seance ne vaut pas ici.
        return match compagnie::demande::autoriser_acte(racine, &demande) {
            compagnie::demande::Verdict::Accepte => Ok(()),
            compagnie::demande::Verdict::Refuse(e) => Err(e.reformuler(|m| m.replace("Rien n'est parti", "Rien n'a été modifié")).reclasser(Classe::PreuveInsuffisante)),
        };
    };
    if !coffre.verifier_ouverture(mdp)? {
        return Err(erreur(Classe::PreuveInsuffisante, demande.refus("Rien n'a été modifié.")));
    }
    Ok(())
}

/// Le geste « Poser » d'un service du coffre, tel que la demande du mot de passe le dit : pour l'Urssaf, dont le compte se
/// saisit sur la meme ligne (`Identifiant::ici`), le compte et le mot de passe.
fn geste_poser(libelle: &str, ici: bool) -> String {
    if ici {
        format!("enregistrer le compte et le mot de passe du service « {} »", libelle)
    } else {
        format!("poser le mot de passe du service « {} »", libelle)
    }
}

/// Le geste « Oublier » d'un service du coffre.
fn geste_oublier(libelle: &str) -> String {
    format!("retirer du coffre le mot de passe du service « {} »", libelle)
}

/// Les gestes du compte Google.
const GESTE_GOOGLE_LIER: &str = "lier le compte Google de la sauvegarde";
const GESTE_GOOGLE_OUBLIER: &str = "oublier le compte Google de la sauvegarde";

/// Un secret a poser : celui de la charge (la page), sinon tape deux fois par un humain (`demande::saisir_deux_fois`).
fn secret_a_poser(donne: Option<socle::secret::Secret>, titre: &str, lignes: &[String]) -> R<socle::secret::Secret> {
    match donne {
        Some(v) => Ok(v),
        None => compagnie::demande::saisir_deux_fois(titre, lignes, "mot de passe (rien ne s'affiche) : ").map_err(|e| e.reclasser(Classe::Refuse)),
    }
}

/// Le mot de passe actuel du logiciel : celui que la page a fait taper, sinon celui que l'humain vient de taper pour cette
/// route (`exiger_mot_de_passe`).
fn actuel(mdp: Option<&socle::secret::Secret>) -> Option<socle::secret::Secret> {
    match mdp {
        Some(m) => Some(m.clone()),
        None => compagnie::coffre::mot_de_passe_retenu(),
    }
}

/// Le refus du coffre : `Coffre` se rend `Refuse` (la page le montre tel quel), tout le reste (`Disque`...) garde
/// sa classe.
fn en_operation(x: Echec) -> Echec {
    match x.classe {
        Some(Classe::Coffre) => x.reclasser(Classe::Refuse),
        _ => x,
    }
}

fn coffre(ctx: &Contexte) -> Coffre {
    Coffre::du_poste(&ctx.racine)
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
fn coffre_poser(ctx: &Contexte, c: ChargePoser, mdp: Option<&socle::secret::Secret>) -> R<EtatDuCoffre> {
    let service = c.service;
    let compte = if c.nom.is_some() || c.prenom.is_some() { compte_dpae_change(ctx, c.nom.as_deref(), c.prenom.as_deref())? } else { None };
    // Sans `valeur` (`moteur route`) : le compte seul s'il change, sinon le secret, tape par un humain apres la porte.
    let donne = c.valeur.as_ref().is_some_and(|v| !socle::texte::strip(v.devoiler()).is_empty());
    let tape = c.valeur.is_none() && compte.is_none();
    if !tape && !donne && compte.is_none() {
        return Err(refus(format!("Tapez le mot de passe {} à poser : rien n'a été posé.", service.nom)));
    }
    let pose_un_secret = tape || donne;
    exiger_mot_de_passe(&ctx.racine, mdp, &geste_poser(service.libelle, identifiant_du_compte(&ctx.entreprise, service.nom)?.ici))?;
    let lignes = [format!("Service : {}", service.libelle), "La frappe ne s'affiche pas : elle est scellée dans le coffre, nulle part ailleurs.".to_string()];
    let valeur = if pose_un_secret { Some(secret_a_poser(c.valeur, "PuppetPaie : poser un mot de passe", &lignes)?) } else { None };
    let valeur = valeur.filter(|v| !socle::texte::strip(v.devoiler()).is_empty());
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
        Some(manque) => (socle::phrase!("{} ; pas essayé : {}.", quoi, manque), "gris"),
        None => match compagnie::coffre_verifier::route(ctx, service.nom) {
            Ok(v) if v.ok => (socle::phrase!("{}, et vérifié : la connexion passe.", quoi), "vert"),
            Ok(v) => {
                let conseil: String = v.conseil.iter().map(|c| format!(" — {}", c)).collect();
                (socle::phrase!("{} ; connexion refusée : {}{}", quoi, v.message, conseil), "rouge")
            }
            Err(e) => (socle::phrase!("{} ; vérification impossible : {}", quoi, e.phrase), "rouge"),
        },
    };
    Ok(EtatDuCoffre { phrase, couleur: Some(couleur), ..etat })
}

/// `coffre-oublier` : retire un secret du coffre.
fn coffre_oublier(ctx: &Contexte, c: ChargeService, mdp: Option<&socle::secret::Secret>) -> R<EtatDuCoffre> {
    let service = c.service;
    exiger_mot_de_passe(&ctx.racine, mdp, &geste_oublier(service.libelle))?;
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
    crate::fiches::poser_entreprise(ctx, droit::fiche::ChangementEntreprise::ApiDpae(Some(cfg)), Some(ctx.aujourdhui))?;
    Ok(droit::fiche::lire_fiche_posee::<droit::fiche::FicheEntrepriseDatee>(&droit::contexte::ENTREPRISE.dans(&ctx.racine), droit::fiche::NomDeFiche::Entreprise)?.au(&ctx.aujourdhui))
}

// ------------------------------------------------------------------ le mot de passe du logiciel

/// Pose ou change le mot de passe du logiciel. L'actuel est exige s'il existe, et il ouvre aussi le coffre quand la cle
/// de ce poste ne l'ouvre plus.
fn mot_de_passe_changer(ctx: &Contexte, c: ChargeChanger, mdp: Option<&socle::secret::Secret>) -> R<EtatDuCoffre> {
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
    if !nouveau.egal_a(confirmation.devoiler()) {
        return Err(refus("Les deux saisies du nouveau mot de passe different. Rien n'a ete modifie."));
    }
    let actuel = actuel(mdp);
    // UNE COMPAGNIE DE L'ADMINISTRATEUR : son mot de passe est celui de toutes ses compagnies, et il se change pour toutes a
    // la fois (`administrateur::changer`) ; l'actuel les ouvre chacune, tout se prepare puis tout se remplace.
    let inst = compagnie::installation::IdDeCompagnie::installation_de(&ctx.racine);
    if crate::gestes::administrateur::est_rattachee(&inst, &ctx.racine)? {
        let Some(actuel) = actuel else {
            return Err(refus("Tapez le mot de passe actuel de l'administrateur : le nouveau le remplace dans toutes ses compagnies. Rien n'a ete modifie."));
        };
        crate::gestes::administrateur::changer(&inst, &actuel, &nouveau, &confirmation, &crate::gestes::administrateur::Poste::de_ce_poste()).map_err(|e| e.reclasser(Classe::Refuse))?;
        return coffre_etat(ctx);
    }
    coffre(ctx).poser_ouverture(&nouveau, actuel.as_ref()).map_err(en_operation)?;
    coffre_etat(ctx)
}

/// Plus de mot de passe du logiciel. L'actuel est exige.
fn mot_de_passe_retirer(ctx: &Contexte, mdp: Option<&socle::secret::Secret>) -> R<EtatDuCoffre> {
    if crate::gestes::administrateur::est_rattachee(&compagnie::installation::IdDeCompagnie::installation_de(&ctx.racine), &ctx.racine)? {
        return Err(refus("Cette compagnie est geree par l'administrateur : son mot de passe ouvre aussi les autres. La detacher d'abord (elle recoit alors son propre mot de passe), puis le retirer. Rien n'a ete modifie."));
    }
    exiger_mot_de_passe(&ctx.racine, mdp, "retirer le mot de passe du logiciel")?;
    coffre(ctx).oublier_ouverture(actuel(mdp).as_ref())?;
    coffre_etat(ctx)
}

// ------------------------------------------------------------------ Google

/// Efface le jeton local, sans reseau. L'acces reste accorde cote Google, et se retire sur myaccount.google.com.
fn google_oublier(ctx: &Contexte, mdp: Option<&socle::secret::Secret>) -> R<GoogleOublie> {
    exiger_mot_de_passe(&ctx.racine, mdp, GESTE_GOOGLE_OUBLIER)?;
    let p = socle::fichiers::JETON_GOOGLE.dans(&ctx.racine);
    let efface = p.exists();
    if efface {
        std::fs::remove_file(&p).map_err(|e| socle::echec::du_disque(&p, &e).reclasser(Classe::Disque))?;
    }
    Ok(GoogleOublie { phrase: Phrase::fixe("Compte Google délié : la sauvegarde ne va plus au Drive. L'accès accordé côté Google se retire sur myaccount.google.com."), efface })
}

/// L'application Google du logiciel, livree avec lui dans `google-application.json` (a cote du code), `client_id` et
/// `client_secret`. Le secret ne s'ecrit pas dans le source Rust.
const FICHIER_APPLICATION: socle::fichiers::CheminRelatif = socle::fichiers::CheminRelatif::du_moteur("google-application.json");

/// L'application Google propre a la compagnie, sous `config/` ; le jeton scelle : `socle::fichiers::JETON_GOOGLE`.
const FICHIER_IDENTIFIANTS: &str = "google.json";

/// La description scellee avec le jeton.
pub(crate) const DESCRIPTION: &str = "moteur de paie - jeton Google";

fn config(racine: &Path) -> PathBuf {
    racine.join(socle::fichiers::CONFIG)
}


/// L'APPLICATION GOOGLE TELLE QU'ELLE S'ECRIT (`google.json` de la compagnie, `google-application.json` livre) : son
/// identifiant et son secret.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationEcrite {
    client_id: String,
    client_secret: socle::secret::Secret,
}

/// LE JETON GOOGLE TEL QU'IL SE SCELLE (`sceller_le_jeton`) : le rafraichissement, le compte, les permissions accordees.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JetonEcrit {
    #[serde(serialize_with = "socle::secret::ecrit")]
    rafraichissement: socle::secret::Secret,
    compte: String,
    permissions: Vec<String>,
}

/// L'application qui s'autorise, et d'ou elle vient (`google.json` de la compagnie, ou le logiciel).
pub(crate) fn identifiants(racine: &Path) -> R<(Application, String)> {
    let p = config(racine).join(FICHIER_IDENTIFIANTS);
    if p.exists() {
        // Un `google.json` illisible est un refus, pas un plantage.
        let a: ApplicationEcrite = socle::forme::lire_fichier(&p, "L'application Google de la compagnie (google.json) ne se lit pas : la corriger, ou la supprimer pour revenir à celle du logiciel.")
            .map_err(|e| e.reclasser(Classe::Google))?;
        if a.client_id.is_empty() || a.client_secret.devoiler().is_empty() {
            let lieu = socle::echec::Lieu { chemin: Some(p.to_string_lossy().into_owned()), ..socle::echec::Lieu::default() };
            return Err(Echec {
                a_part: Some(lieu.into()),
                ..erreur(Classe::Google, "L'application Google de la compagnie est incomplète : il y manque son identifiant ou son secret. La supprimer suffit à revenir à celle du logiciel.")
            });
        }
        return Ok((Application { client_id: a.client_id, client_secret: a.client_secret }, p.to_string_lossy().into_owned()));
    }
    let livre = FICHIER_APPLICATION.dans(&socle::fichiers::dossier_du_code());
    if !livre.exists() {
        let lieu = socle::echec::Lieu {
            chemin: Some(p.to_string_lossy().into_owned()),
            fichier: Some(FICHIER_APPLICATION),
            detail: Some("« Publier » la pose depuis le secret PUPPETPAIE_GOOGLE_APPLICATION ; celle de la compagnie s'écrit à `chemin` (client_id, client_secret).".to_string()),
            ..socle::echec::Lieu::default()
        };
        return Err(Echec {
            a_part: Some(lieu.into()),
            ..erreur(Classe::Google, "L'application Google du logiciel n'est pas renseignée : elle est livrée avec chaque version publiée ; une copie de travail du dépôt ne l'a pas. Sinon, la compagnie peut poser sa propre application Google.")
        });
    }
    let a: ApplicationEcrite = socle::forme::lire_fichier(&livre, "L'application Google livrée avec le logiciel ne se lit pas : le signaler au développeur.").map_err(|e| e.reclasser(Classe::Google))?;
    Ok((Application { client_id: a.client_id, client_secret: a.client_secret }, "le logiciel".into()))
}

/// LE JETON GOOGLE DE LA COMPAGNIE : aucun (le Drive n'est pas lie), celui qui se lit, ou la phrase qui dit pourquoi il
/// ne se lit pas (le coffre qui refuse dit quoi faire ; le client Drive la rend telle quelle).
enum Jeton {
    Absent,
    Lu(JetonEcrit),
    Illisible(Echec),
}

impl Jeton {
    /// Le jeton scelle sous la racine de la compagnie.
    fn de(racine: &Path) -> Jeton {
        let p = socle::fichiers::JETON_GOOGLE.dans(racine);
        if !p.exists() {
            return Jeton::Absent;
        }
        match Jeton::ouvrir(racine, &p) {
            Ok(j) => Jeton::Lu(j),
            Err(e) => Jeton::Illisible(e),
        }
    }

    fn ouvrir(racine: &Path, p: &Path) -> R<JetonEcrit> {
        // Une lecture impossible (droits) se range avec les refus du coffre : l'etat se lit.
        let brut = std::fs::read(p).map_err(|e| socle::echec::du_disque(p, &e))?;
        let clair = compagnie::coffre::Coffre::du_poste(racine).deproteger(&brut, DESCRIPTION).map_err(|e| {
            e.reformuler(|m| format!("{} Pour le jeton Google, le remede est : relier de nouveau le Drive (Configuration > Mots de passe).", m))
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

/// QUAND LE DRIVE A ETE LIE, sans rien ouvrir : la date du fichier du jeton scelle (`sceller_le_jeton` l'ecrit a chaque
/// liaison), lue dans ses metadonnees ; `None` : aucun jeton. `Err` : sa date ne se lit pas.
pub(crate) fn lie_le(racine: &Path) -> R<Option<socle::paris::Moment>> {
    let p = socle::fichiers::JETON_GOOGLE.dans(racine);
    match std::fs::metadata(&p) {
        Ok(m) => socle::paris::mtime(&m).map(Some).ok_or_else(|| socle::echec::au_chemin("La date du jeton Google ne se lit pas : relier de nouveau le Drive (Configuration › Mots de passe).", &p)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(socle::echec::du_disque(&p, &e)),
    }
}

/// L'adresse du compte lie, sans reseau ; `None` : aucun jeton. Un jeton qui ne se lit pas n'est pas « aucun » : il se
/// refuse, et l'ecran le dit (le relier de nouveau).
pub(crate) fn compte_autorise(racine: &Path) -> R<Option<String>> {
    match Jeton::de(racine) {
        Jeton::Lu(j) => Ok(Some(j.compte)),
        Jeton::Absent => Ok(None),
        Jeton::Illisible(e) => Err(e),
    }
}

/// Ce que la boite de « Lier le Drive » dit avant d'ouvrir Google, a l'accueil comme dans une compagnie.
pub(crate) const OUVRIR_GOOGLE: [&str; 2] = [
    "Le logiciel ouvre la fenêtre de Google dans votre navigateur : choisissez le compte, acceptez, puis revenez ici.",
    "Si Google dit que l'application n'est pas validée : Paramètres avancés › Accéder à PuppetPaie.",
];

/// Ce que la boite de « Lier le Drive » dit, une ligne par phrase.
pub(crate) fn ouvrir_google_dit() -> Vec<String> {
    OUVRIR_GOOGLE.iter().map(|p| p.to_string()).collect()
}

/// Le faux Google qui remplace Google : dans l'atelier, celui d'`envois::redirection` ; en essai, celui que l'essai
/// nomme (`VARIABLE_GOOGLE_ESSAIS`, une adresse locale seulement) ; `None` hors de l'un et de l'autre.
fn faux_google() -> R<Option<socle::garde_reseau::Url>> {
    #[cfg(feature = "atelier")]
    if let Some(b) = envois::redirection::base(envois::redirection::VARIABLE_GOOGLE) {
        return Ok(Some(b));
    }
    match std::env::var(VARIABLE_GOOGLE_ESSAIS) {
        Ok(b) if socle::garde_reseau::en_essai() => Ok(match socle::garde_reseau::Url::lire(b.trim_end_matches('/')) {
            Ok(u) if envois::redirection::est_locale(&u) => Some(u),
            // Une adresse qui n'est pas locale : le vrai Google.
            Ok(_) | Err(_) => None,
        }),
        // Hors essai : le vrai Google.
        Ok(_) | Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(refus(format!("La variable d'environnement {} n'est pas du texte : elle ne se lit pas.", VARIABLE_GOOGLE_ESSAIS))),
    }
}

/// Les adresses de Google, ou celles du faux Google (`faux_google`).
fn adresses_du_poste() -> R<envois::drive::Adresses> {
    Ok(match faux_google()? {
        Some(b) => envois::drive::Adresses::de_la_base(&b),
        None => envois::drive::Adresses::de_google(),
    })
}

/// LE CLIENT DU DRIVE DE LA COMPAGNIE : refuse une copie de developpement (qui ecrirait dans le Drive de la VRAIE
/// compagnie), lit l'application (`google.json` ou celle du logiciel), le jeton scelle par la cle du poste et l'adresse
/// du compte, et les passe au client (`drive::Client::nouveau`). Aucun appel reseau.
pub fn client_drive(racine: &Path) -> Result<envois::drive::Client, envois::drive::ErreurGoogle> {
    let (autorisation, adresses) = autorisation_du_drive(racine)?;
    Ok(envois::drive::Client::nouveau(autorisation, adresses))
}

/// Ce que le client du Drive (`client_drive`) et la corbeille (`corbeille_du_drive`) recoivent : l'autorisation lue sous
/// `racine` et les adresses du poste. Aucun appel reseau.
fn autorisation_du_drive(racine: &Path) -> Result<(envois::drive::Autorisation, envois::drive::Adresses), envois::drive::ErreurGoogle> {
    if let Some(refus) = refus_de_copie(racine) {
        return Err(refus.into());
    }
    let (application, _origine) = identifiants(racine)?;
    let (rafraichissement, compte) = match Jeton::de(racine) {
        Jeton::Absent => (Ok(None), None),
        Jeton::Lu(j) => (Ok(Some(j.rafraichissement)), Some(j.compte)),
        Jeton::Illisible(e) => (Err(envois::drive::ErreurGoogle::from(e.reclasser(Classe::Google))), None),
    };
    Ok((envois::drive::Autorisation { application, rafraichissement, compte }, adresses_du_poste()?))
}

/// LA CORBEILLE DU DRIVE DE LA COMPAGNIE (`envois::drive::Corbeille`), pour le seul geste `geste` qui met un dossier a la
/// corbeille (`routes::drive`) : le mot de passe du logiciel d'abord, comme un acte reel (`preuve::verifier_preuve` ; tape
/// dans la page, `mdp`, sinon le moteur le demande a un humain), puis l'autorisation lue ici, que rien d'autre ne donne.
pub(crate) fn corbeille_du_drive(ctx: &Contexte, mdp: Option<&socle::secret::Secret>, geste: &str) -> R<envois::drive::Corbeille> {
    let demande = demande(&ctx.racine, geste)?;
    let preuve = compagnie::preuve::Preuve::du_mot_de_passe(mdp.cloned());
    compagnie::preuve::verifier_preuve(ctx, Some(&preuve), &demande).map_err(|e| e.reformuler(|m| m.replace("Rien n'est parti", "Rien n'est supprimé")))?;
    let (autorisation, adresses) = autorisation_du_drive(&ctx.racine).map_err(|e| e.echec)?;
    Ok(envois::drive::Corbeille::nouvelle(autorisation, adresses))
}

/// La variable d'un faux Google des essais : sa base locale (`http://127.0.0.1:<port>`), lue en essai seulement
/// (`garde_reseau::en_essai`, que Cargo pose).
pub const VARIABLE_GOOGLE_ESSAIS: &str = "PUPPETPAIE_ESSAIS_GOOGLE";

/// Les adresses de l'autorisation : celles de Google, ou celles du faux Google (`faux_google`).
fn adresses_de_l_autorisation() -> R<envois::google_autorisation::Adresses> {
    Ok(match faux_google()? {
        Some(b) => envois::google_autorisation::Adresses::de_la_base(&b),
        None => envois::google_autorisation::Adresses::de_google(),
    })
}

/// LIER LE COMPTE GOOGLE (`google-autoriser`) : le mot de passe du logiciel s'il est pose, puis l'autorisation
/// (`google_autorisation::autoriser`) : le moteur ouvre la fenetre de consentement de Google dans le navigateur du poste
/// et dit son adresse sur la sortie d'erreur (un terminal sans navigateur la copie), la personne accepte, Google renvoie
/// le code au moteur, qui scelle le jeton. Rend l'adresse du compte lie. La page et `moteur route` l'appellent pareil.
fn google_autoriser(ctx: &Contexte, mdp: Option<&socle::secret::Secret>) -> R<String> {
    exiger_mot_de_passe(&ctx.racine, mdp, GESTE_GOOGLE_LIER)?;
    lier_google(&ctx.racine)
}

/// L'AUTORISATION DE GOOGLE, SCELLEE SOUS `racine` (`config/google-jeton.bin`) : celle d'une compagnie (`google-autoriser`),
/// ou celle de l'accueil, sous l'installation (`drive-lier`, `accueil::drive`). Rend l'adresse du compte lie.
pub(crate) fn lier_google(racine: &Path) -> R<String> {
    let (application, _) = identifiants(racine).map_err(|e| e.reclasser(Classe::Refuse))?;
    let dire_url = |u: &socle::garde_reseau::Url| {
        eprintln!("La fenetre de Google, a ouvrir dans le navigateur si elle ne s'ouvre pas seule : {}", u);
        if let Err(e) = compagnie::poste::ouvrir_adresse(u) {
            eprintln!("Le navigateur ne s'ouvre pas : {}", e.phrase);
        }
    };
    let jeton = envois::google_autorisation::autoriser(&application, &adresses_de_l_autorisation()?,envois::google_autorisation::DELAI_AUTORISATION, &dire_url, &|m| eprintln!("{}", m))?;
    sceller_le_jeton(racine, &jeton)
}

/// Le jeton que l'autorisation a obtenu (`google_autorisation::autoriser`) et l'adresse du compte, dans un seul fichier
/// scelle par la cle du poste. Rend l'adresse du compte lie.
fn sceller_le_jeton(racine: &Path, jeton: &envois::google_autorisation::JetonGoogle) -> R<String> {
    let ecrit = JetonEcrit { rafraichissement: jeton.rafraichissement.clone(), compte: jeton.compte.clone(), permissions: jeton.permissions.clone() };
    let brut = socle::json_ascii::dumps(&ecrit, None, false, false);
    let scelle = compagnie::coffre::Coffre::du_poste(racine)
        .proteger(brut.as_bytes(), DESCRIPTION)
        .map_err(|e| e.reformuler(|m| format!("Le jeton n'a pas pu etre scelle, rien n'est enregistre : {}", m)).reclasser(Classe::Google))?;
    let dossier = config(racine);
    std::fs::create_dir_all(&dossier).map_err(|e| socle::echec::du_disque(&dossier, &e).reclasser(Classe::Google))?;
    let chemin = socle::fichiers::JETON_GOOGLE.dans(racine);
    std::fs::write(&chemin, scelle).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::Google))?;
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
            (None, _, _) => (None, Pastille::de(socle::ecran::EtatDEcran::AFaire, "aucun Drive lié")),
            (Some(c), true, _) => (Some(c.clone()), Pastille::de(socle::ecran::EtatDEcran::Fait, "lié")),
            (Some(c), false, Some(p)) => (Some(format!("{} : {}", c, p)), Pastille::de(socle::ecran::EtatDEcran::AFaire, "lié, mais")),
            (Some(c), false, None) => (Some(format!("{} : l'autorisation ne répond pas", c)), Pastille::de(socle::ecran::EtatDEcran::AFaire, "lié, mais")),
        };
        EtatGoogle {
            phrase: match &compte_dit {
                Some(c) => socle::phrase!("Google Drive : {} ({}).", pastille.mots, c),
                None => socle::phrase!("Google Drive : {}.", pastille.mots),
            },
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
    match jeton_par_rafraichissement(&adresses_du_poste()?.jetons, &application, &jeton.rafraichissement, DELAI)? {
        Ok(_) => Ok(etat(Some(jeton.compte), true, None, accordees)),
        Err(probleme) => Ok(etat(Some(jeton.compte), false, Some(probleme.echec.phrase), accordees)),
    }
}

/// `identifiant_du_compte` : le compte que ce mot de passe ouvre, en clair ; ce qui en manque ; et s'il se saisit sur
/// cette ligne (`ici`).
fn identifiant_du_compte(ent: &droit::fiche::ChampsEntreprise, service: &str) -> R<Identifiant> {
    match service {
        "net-entreprises" => {
            let d = envois::transport::declarant(ent);
            let manque: Vec<&'static str> = [("siret", d.siret.is_none()), ("nom", d.nom.is_none()), ("prenom", d.prenom.is_none())].into_iter().filter(|(_, manque)| *manque).map(|(c, _)| c).collect();
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
    let coffre = compagnie::coffre::Coffre::du_poste(racine);
    let secrets = coffre.services()?;
    // UNE COMPAGNIE DONT LES ENVOIS SONT JOUES n'a rien a poser : chaque pastille le dit, en gris.
    let simule = compagnie::bridage::branchement(racine)?.joue();
    let mut services = Vec::new();
    for s in secrets {
        // Sa pastille : simule, une information ; fourni par l'environnement (il passe devant le coffre), a voir ; pose,
        // fait ; absent, a poser.
        let pastille = if simule {
            Pastille::de(socle::ecran::EtatDEcran::Information, "simulé")
        } else if s.dans_environnement {
            Pastille::de(socle::ecran::EtatDEcran::AFaire, "fourni par l'environnement")
        } else if s.pose {
            match s.pose_le {
                Some(le) => Pastille::de(socle::ecran::EtatDEcran::Fait, &format!("posé le {}", le.jour_de_paris().jj_mm_aaaa())),
                None => Pastille::de(socle::ecran::EtatDEcran::Fait, "posé"),
            }
        } else {
            Pastille::de(socle::ecran::EtatDEcran::AFaire, "absent")
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
            identifiant,
            pastille,
        });
    }
    let mot_de_passe_pose = coffre.ouverture_exigee()?;
    let cle_poste = coffre.cle_posee()?;
    Ok(EtatDuCoffre {
        phrase: Phrase::fixe(""),
        services,
        mot_de_passe_pose,
        cle_poste,
        chemin_cle: coffre.chemin_cle().to_string_lossy().into_owned(),
        // La pastille du mot de passe du logiciel (rattache a l'administrateur, son rang dit a combien de compagnies).
        pastille_mot_de_passe: if simule {
            Pastille::de(socle::ecran::EtatDEcran::Information, "simulé")
        } else if mot_de_passe_pose {
            Pastille::de(socle::ecran::EtatDEcran::Fait, "posé")
        } else {
            Pastille::de(socle::ecran::EtatDEcran::AFaire, "aucun")
        },
        couleur: None,
    })
}

// ------------------------------------------------------------------ l'ecran des mots de passe

/// Ce qui designe la chose d'un rang de l'ecran des mots de passe aux gestes de ce rang : le service du coffre ; aucun
/// pour le mot de passe du logiciel, l'administrateur et le compte Google.
#[derive(Debug, Clone, Serialize)]
struct ServiceDuRang {
    #[serde(skip_serializing_if = "Option::is_none")]
    service: Option<&'static str>,
}

/// Ce que montrent d'abord les cases du geste qui pose un mot de passe d'organisme : le compte Urssaf que la fiche
/// Entreprise dit (son prenom, son nom), l'adresse de la boite courriel posee.
#[derive(Debug, Clone, Default, Serialize)]
struct CompteMontre {
    #[serde(skip_serializing_if = "Option::is_none")]
    prenom: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nom: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compte: Option<String>,
}

/// L'ecran des mots de passe : un rang par chose, que designe son service (`ServiceDuRang`), sans case ; les cases du
/// geste qui pose un mot de passe d'organisme montrent d'abord son compte (`CompteMontre`) et n'offrent aucun choix.
type EcranDuCoffre = EcranRendu<ServiceDuRang, CaseDeFiche, CompteMontre>;
type GesteDuCoffre = GesteDeRoute<ServiceDuRang, CompteMontre, SansCharge>;

/// Ce que rend `coffre-etat` : la phrase de l'etat du coffre, et l'ecran des mots de passe.
#[derive(Debug, Serialize)]
struct CoffreDit {
    phrase: Phrase,
    #[serde(flatten)]
    ecran: EcranDuCoffre,
}

// Les mots de l'ecran, en constantes : un texte fautif ne compile pas (`Affiche::new`).
const POSER: Affiche = Affiche::new("Poser…");
const CHANGER: Affiche = Affiche::new("Changer…");
const RETIRER: Affiche = Affiche::new("Retirer…");
const VERIFIER: Affiche = Affiche::new("Vérifier");
const OUBLIER: Affiche = Affiche::new("Oublier…");
const RATTACHER: Affiche = Affiche::new("Rattacher…");
const DETACHER: Affiche = Affiche::new("Détacher…");
const LIER: Affiche = Affiche::new("Lier le Drive");
const RELIER: Affiche = Affiche::new("Relier le Drive");

/// L'ECRAN DES MOTS DE PASSE (`coffre-etat`), dans l'ordre : le mot de passe du logiciel, l'administrateur (quand la
/// compagnie lui est rattachee ou peut l'etre), chaque service du coffre, le compte Google. Pour chacun son titre, ce
/// qu'il dit, sa pastille et ce qui la suit, et les gestes que le moteur ouvre, chacun ses cases, ce que sa boite dit et
/// la demande du mot de passe du logiciel quand il est pose. En tete : ce que l'ecran est, et l'encart d'une compagnie
/// fictive (rien ne s'y pose : aucun geste) ou d'un poste sans cle. Le compte Google se dit sans reseau (`compte_autorise`) ;
/// « Vérifier » l'essaie (`google-etat`).
fn ecran_du_coffre(ctx: &Contexte, e: &EtatDuCoffre) -> R<EcranDuCoffre> {
    let fictive = compagnie::bridage::branchement(&ctx.racine)?.joue();
    let adm = if fictive { None } else { Some(administrateur_etat(&ctx.racine)?) };
    let (en_liste, rattachee, peut_rattacher) = match &adm {
        Some(a) => (a.autres_dit.clone(), a.rattachee, a.peut_rattacher),
        None => (String::new(), false, false),
    };
    let pose = e.mot_de_passe_pose;
    // La demande du mot de passe du logiciel avant un geste qui l'exige, quand il est pose.
    let exigee = |geste: &str| -> R<Option<compagnie::demande::DemandeDuMotDePasse>> { if pose { Ok(Some(demande(&ctx.racine, geste)?)) } else { Ok(None) } };
    let geste = |route: Route, service: Option<&'static str>, libelle: Affiche, cases: Vec<&'static str>| -> GesteDuCoffre {
        GesteDeRoute::avec_cases(route, ServiceDuRang { service }, libelle, cases, CompteMontre::default(), SansCharge {})
    };
    let encart = if fictive {
        Some(super::Encart::de(socle::ecran::EtatDEcran::Information, "Compagnie fictive : rien ne se pose ici", "Les dépôts de DPAE et de DSN, leurs retours, les courriels et les signatures sont simulés : aucun mot de passe n'est demandé, et rien ne quitte le poste. Dans une compagnie réelle, c'est ici que se posent les accès à net-entreprises, à l'Urssaf et à la boîte courriel."))
    } else if !e.cle_poste {
        Some(super::Encart::de(socle::ecran::EtatDEcran::Information, "Cet ordinateur n'a pas encore sa clé du coffre", "Rien n'est perdu : elle se refait toute seule la prochaine fois que le mot de passe du logiciel est tapé (ou posé, ci-dessous). Elle vit à part des données, pour qu'une sauvegarde n'emporte jamais de quoi ouvrir le coffre."))
    } else {
        None
    };
    let mut encarts = vec![super::Encart::de(socle::ecran::EtatDEcran::Information, "", "Les mots de passe dont le logiciel a besoin pour déposer, relever et envoyer. Un mot de passe posé ne se relit plus : l'écran dit seulement s'il est posé.")];
    encarts.extend(encart);
    let mut rangs = Vec::new();

    // LE MOT DE PASSE DU LOGICIEL : le poser, le changer (l'actuel se tape dans la boite, au-dessus du nouveau : il est ce
    // que le geste remplace, pas sa preuve), le retirer (pas d'une compagnie rattachee : le moteur le refuse).
    let quand = "demandé à l'ouverture et à chaque dépôt réel, envoi ou signature";
    let dit = if rattachee {
        format!("Il ouvre aussi {} ; {}.", en_liste, quand)
    } else if pose || fictive {
        format!("Il est {}.", quand)
    } else {
        "Rien ne peut partir en réel tant qu'il n'est pas posé.".to_string()
    };
    let mut gestes = Vec::new();
    if !fictive {
        let nouveau = if rattachee { "Le nouveau : six caractères au moins ; il remplacera l'actuel dans toutes ses compagnies." } else { "Le nouveau : six caractères au moins." };
        let (neuf, confirmation) = (cle!(ChargeChanger, nouveau), cle!(ChargeChanger, confirmation));
        let cases = if pose { vec![crate::enveloppe::MOT_DE_PASSE, neuf, confirmation] } else { vec![neuf, confirmation] };
        gestes.push(geste(Route::MotDePasseChanger, None, if pose { CHANGER } else { POSER }, cases).disant(vec![nouveau.to_string()]));
        if pose && !rattachee {
            let retirer = "Retirer le mot de passe du logiciel ? La fenêtre s'ouvrira sans rien demander et le coffre ne sera plus protégé.";
            gestes.push(geste(Route::MotDePasseRetirer, None, RETIRER, vec![crate::enveloppe::MOT_DE_PASSE]).disant(vec![retirer.to_string()]));
        }
    }
    let titre = if rattachee { "Le mot de passe de l'administrateur" } else { "Le mot de passe du logiciel" };
    rangs.push(RangDeLEcran { titre: titre.to_string(), dit: vec![dit], pastille: e.pastille_mot_de_passe.clone(), etat: None, montre: None, gestes });

    // L'ADMINISTRATEUR, quand la compagnie lui est rattachee ou peut l'etre : s'en detacher, ou s'y rattacher.
    if let Some(a) = adm.as_ref().filter(|_| rattachee || peut_rattacher) {
        let (dit, note, g) = if rattachee {
            (
                format!("Cette compagnie s'ouvre avec {}, d'un seul mot de passe.", en_liste),
                "Détachée, elle a son propre mot de passe : celui de l'administrateur ne l'ouvre plus, et elle dépose seule.",
                geste(Route::AdministrateurDetacher, None, DETACHER, vec![cle!(ChargeAdministrateur, administrateur), cle!(ChargeAdministrateur, nouveau), cle!(ChargeAdministrateur, confirmation)]),
            )
        } else {
            let administrateur = cle!(ChargeAdministrateur, administrateur);
            let cases = if pose { vec![administrateur, crate::enveloppe::MOT_DE_PASSE] } else { vec![administrateur] };
            (
                format!("Le mot de passe de {} peut ouvrir aussi cette compagnie.", en_liste),
                "Rattachée, elle s'ouvre avec le mot de passe de l'administrateur, et ses déclarations prêtes partent avec celles des autres compagnies, d'un seul mot de passe.",
                geste(Route::AdministrateurRattacher, None, RATTACHER, cases),
            )
        };
        rangs.push(RangDeLEcran { titre: "L'administrateur".to_string(), dit: vec![dit], pastille: a.pastille.clone(), etat: None, montre: None, gestes: vec![g.disant(vec![note.to_string()])] });
    }

    // CHAQUE SERVICE DU COFFRE : a quoi il sert, le compte qu'il ouvre, l'adresse posee ; le verifier (une tentative, une
    // seule), le poser ou le changer (pour l'Urssaf avec le compte des DPAE, pour la boite avec son adresse), l'oublier.
    for s in &e.services {
        // Ce qu'il sert a faire, puis le compte qu'il ouvre : sans lui, un mot de passe juste sur un identifiant faux
        // echoue pareil. Ce qui manque au compte se dit, et ou le donner.
        let mut dit = vec![s.sert_a.to_string()];
        if !fictive {
            let id = &s.identifiant;
            let compte = match (&id.texte, &id.manque_dit, &id.incomplet_dit, id.ou) {
                (Some(t), ..) => Some(t.clone()),
                (None, Some(m), ..) => Some(m.clone()),
                (None, None, Some(i), Some(ou)) => Some(format!("{} : à compléter dans la {}", i, ou)),
                (None, None, Some(i), None) => Some(i.clone()),
                (None, None, None, _) => None,
            };
            dit.extend(compte);
        }
        let etat = if !fictive && !s.environnement && s.pose { s.compte.clone() } else { None };
        let mut gestes = Vec::new();
        if !fictive {
            let id = &s.identifiant;
            if s.pose || s.environnement {
                gestes.push(geste(Route::CoffreVerifier, Some(s.service), VERIFIER, Vec::new()));
            }
            let (cases, note) = if id.ici {
                (vec![cle!(ChargePoser, valeur), cle!(ChargePoser, nom), cle!(ChargePoser, prenom)], Some("Le compte ouvert sur urssaf.fr pour la DPAE (service « DPAE Service plus ») : ce n'est pas celui de net-entreprises. Les accents sont retirés, la DPAE les refuse."))
            } else if s.avec_compte {
                (vec![cle!(ChargePoser, valeur), cle!(ChargePoser, compte)], Some("Pour Gmail : validation en deux étapes activée, puis myaccount.google.com › Sécurité › Mots de passe des applications. C'est ce mot de passe-là, pas celui du compte."))
            } else {
                (vec![cle!(ChargePoser, valeur)], None)
            };
            let montre = CompteMontre { prenom: id.prenom.clone(), nom: id.nom.clone(), compte: if s.avec_compte { s.compte.clone() } else { None } };
            let poser = GesteDeRoute::avec_cases(Route::CoffrePoser, ServiceDuRang { service: Some(s.service) }, if s.pose { CHANGER } else { POSER }, cases, montre, SansCharge {});
            gestes.push(poser.disant(note.into_iter().map(String::from).collect()).exigeant(exigee(&geste_poser(s.libelle, id.ici))?));
            if s.pose {
                let oublier = format!("Oublier le mot de passe {} ? Les retours et les dépôts ne seront plus possibles jusqu'à ce qu'il soit reposé.", s.libelle);
                gestes.push(geste(Route::CoffreOublier, Some(s.service), OUBLIER, Vec::new()).disant(vec![oublier]).exigeant(exigee(&geste_oublier(s.libelle))?));
            }
        }
        rangs.push(RangDeLEcran { titre: s.libelle.to_string(), dit, pastille: s.pastille.clone(), etat, montre: None, gestes });
    }

    // LE COMPTE GOOGLE, dit sans reseau : le lier (le moteur ouvre Google dans le navigateur et attend qu'on y accepte),
    // le verifier (une tentative), l'oublier.
    let dit = vec!["La sauvegarde du dossier privé et l'archivage des mois, dans son Drive ; rien d'autre.".to_string()];
    let mut dit = dit;
    // Un jeton qui ne se lit pas n'est pas « aucun Drive lié » : il se dit, et se relie.
    let (lie, illisible) = match if fictive { Ok(None) } else { compte_autorise(&ctx.racine) } {
        Ok(compte) => (compte, false),
        Err(_) => {
            dit.push("Le jeton de ce compte Google ne se lit pas sur ce poste : relier le Drive de nouveau.".to_string());
            (None, true)
        }
    };
    let pastille = match (fictive, &lie, illisible) {
        (true, _, _) => Pastille::de(socle::ecran::EtatDEcran::Information, "non lié"),
        (false, _, true) => Pastille::de(socle::ecran::EtatDEcran::Refuse, "illisible"),
        (false, Some(_), false) => Pastille::de(socle::ecran::EtatDEcran::Fait, "lié"),
        (false, None, false) => Pastille::de(socle::ecran::EtatDEcran::AFaire, "aucun Drive lié"),
    };
    let mut gestes = Vec::new();
    if !fictive {
        gestes.push(geste(Route::GoogleAutoriser, None, if lie.is_some() || illisible { RELIER } else { LIER }, Vec::new()).disant(ouvrir_google_dit()).exigeant(exigee(GESTE_GOOGLE_LIER)?));
        if lie.is_some() || illisible {
            gestes.push(geste(Route::GoogleEtat, None, VERIFIER, Vec::new()));
            let oublier = "Oublier le compte Google ? Le jeton local est effacé : plus de sauvegarde ni d'archivage jusqu'à ce qu'un Drive soit relié. L'accès accordé côté Google, lui, se retire sur myaccount.google.com › Sécurité › Vos connexions à des applications tierces.";
            gestes.push(geste(Route::GoogleOublier, None, OUBLIER, Vec::new()).disant(vec![oublier.to_string()]).exigeant(exigee(GESTE_GOOGLE_OUBLIER)?));
        }
    }
    rangs.push(RangDeLEcran { titre: "Compte Google".to_string(), dit, pastille, etat: lie, montre: None, gestes });

    Ok(EcranRendu { encarts, rangs, cases: Vec::new(), historique: Vec::new(), enregistrer: None })
}
