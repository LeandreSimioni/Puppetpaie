//! Les routes du filet et du canal de l'editeur :
//!
//! - `incident-navigateur` : ce qui casse dans la page (le journal des incidents ecrit, et c'est tout) ;
//! - `diagnostic-etat`, `diagnostic-envoyer`, `diagnostic-regler` : ce qui est parti chez l'editeur, et l'envoi de ce
//!   qui attend ;
//! - `signalements`, `signalement-apercu`, `signalement-envoyer` (enregistre, ne part pas), `signalements-partir` (le
//!   depart, en fond), `signalements-lus` : « Un probleme, une idee ? » ;
//! - `incidents-lire`, `incidents-regrouper`, `incidents-oublier`, `incidents-envoyer` : les incidents gardes, pour la
//!   veille et la ligne de commande.
//!
//! Le canal de l'editeur est `editeur.rs` ; jamais d'autre serveur que celui de la configuration.

use std::path::Path;

use serde::Serialize;

use compagnie::incidents::{self, BilanIncidents, Ligne, Nature, Paquet, PourLEditeur, DEPOSE, EN_ATTENTE};
use compagnie::nombre_de_paies::{AnneePartie, ContenuParti};
use compagnie::problemes::{self, Apercu, BilanMessages, CaptureDeLaPage, EtatDuMessage, SignalementVu};
use crate::catalogue::{Acte, Affiche, Champ, Forme, Fiche, FORME, OPERATION};
use crate::catalogue::Route;
use crate::catalogue::{Ecran, Place};
use crate::declaration::{charge, dit, Legende};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use crate::SansCharge;
use socle::ecran::Pastille;
use socle::echec::{Classe, R};
use socle::paris::Moment;

const QUOI: Legende = dit("Nature", "une seule nature d'incident ; toutes sinon").avancee();
const DIAGNOSTIC_REND: &str = "envoi (vrai : actif), total, en_attente, groupes (ce qui part, mot pour mot), pastille";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Signaler);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::SignalementApercu,
        geste: "Montrer, mot pour mot, ce qu'un message au développeur emporterait.",
        charge: &ChargeMessage::CHARGE,
        exemple: r#"{"texte":"La phrase de la DSN est coupée.","ecran":"Paie, novembre 2026"}"#,
        rend: "corps (ce qui part), caviarde, serveur, captures, incidents",
        refus: &[OPERATION, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::SignalementEnvoyer,
        geste: "Écrire au développeur (un problème, une idée) ; le message part seul.",
        charge: &ChargeMessage::CHARGE,
        exemple: r#"{"texte":"La phrase de la DSN est coupée.","ecran":"Paie, novembre 2026"}"#,
        rend: "signalements (chacun son état et son départ)",
        refus: &[OPERATION, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Signalements,
        geste: "Dire les messages écrits au développeur, et ses réponses.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "signalements (chacun son état, sa pastille, ses réponses)",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::SignalementsLus,
        geste: "Marquer lues les réponses du développeur.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "lues (le nombre marqué)",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::SignalementsPartir,
        geste: "Faire partir chez le développeur les messages qui attendent.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "le bilan du passage (partis, reste, erreur, occupe), phrase ; ne refuse jamais",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler, Affiche::new("Envoyer les messages qui attendent")),
    },
    Fiche {
        route: Route::IncidentNavigateur,
        geste: "Noter ce qui a cassé dans la page.",
        charge: &[Champ { nom: "message", libelle: Affiche::new("Ce que la page a vu"), forme: Forme::Texte, sens: "ce que la page a vu (et genre, fichier, ligne, colonne, pile, canal, ou_dans_la_page)", avance: false }],
        exemple: r#"{"genre":"TypeError","message":"exemple","canal":"erreur","ou_dans_la_page":"paie"}"#,
        rend: "note (vrai : noté), empreinte",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Signaler),
    },
    Fiche {
        route: Route::DiagnosticEtat,
        geste: "Dire ce qui est parti chez le développeur, et ce qui attend.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: DIAGNOSTIC_REND,
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler, Affiche::new("Voir ce qui est parti")),
    },
    Fiche {
        route: Route::DiagnosticEnvoyer,
        geste: "Envoyer au développeur les incidents qui attendent.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "ce que diagnostic-etat rend, bilan et couleur",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler, Affiche::new("Envoyer maintenant ce qui attend")),
    },
    Fiche {
        route: Route::DiagnosticRegler,
        geste: "Couper l'envoi des incidents au développeur, ou le rétablir.",
        charge: &ChargeRegler::CHARGE,
        exemple: r#"{"actif":true}"#,
        rend: DIAGNOSTIC_REND,
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler, Affiche::new("Régler l'envoi des incidents")),
    },
    Fiche {
        route: Route::IncidentsLire,
        geste: "Lire les incidents gardés sur le poste.",
        charge: &ChargeIncidents::CHARGE,
        exemple: r#"{}"#,
        rend: "phrase, phrases (une par incident), lignes (les plus récentes en dernier)",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler, Affiche::new("Lire les incidents gardés")),
    },
    Fiche {
        route: Route::IncidentsRegrouper,
        geste: "Regrouper les incidents gardés, le plus fréquent en tête.",
        charge: &ChargeNature::CHARGE,
        exemple: r#"{}"#,
        rend: "phrase, phrases (une par paquet), paquets",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler, Affiche::new("Regrouper les incidents")),
    },
    Fiche {
        route: Route::IncidentsEnvoyer,
        geste: "Déposer chez le développeur les incidents qui attendent.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "envoyes, reste, erreur, occupe, phrase ; ne refuse jamais",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler, Affiche::new("Envoyer les incidents qui attendent")),
    },
    Fiche {
        route: Route::IncidentsOublier,
        geste: "Effacer les incidents gardés, tous ou d'une nature.",
        charge: &ChargeNature::CHARGE,
        exemple: r#"{"quoi":"logiciel"}"#,
        rend: "retires (le nombre), phrase",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler, Affiche::new("Effacer les incidents")),
    },
];
/// Une route de ce module, pour la compagnie de la porte, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    route_de(&porte.ctx.racine, porte.route, charge)
}

/// LE DOSSIER DE L'INSTALLATION QUI GARDE CE QU'ON ECRIT AU DEVELOPPEUR SANS COMPAGNIE (depuis l'accueil, ou en ligne de
/// commande sans `--racine`) : ses messages, leur depart et l'identite du poste, comme une compagnie les garde dans son
/// dossier ; aucune compagnie n'y est lue, le message part sans ses renseignements.
pub fn sans_compagnie(o: &super::enveloppe::Options) -> std::path::PathBuf {
    super::accueil::installation(o).join(".accueil")
}

/// Une route de ce module, sans compagnie (`sans_compagnie`).
pub(crate) fn route_sans_compagnie(o: &super::enveloppe::Options, nom: Route, charge: &Charge) -> R<Rendu> {
    let racine = sans_compagnie(o);
    std::fs::create_dir_all(&racine).map_err(|e| socle::echec::du_disque(&racine, &e).reclasser(Classe::du_disque(&e)))?;
    route_de(&racine, nom, charge)
}

// ------------------------------------------------------------------ les charges

charge! {
    /// La charge de `diagnostic-regler`.
    #[derive(Debug)]
    struct ChargeRegler {
        actif: bool = dit("Envoyer les incidents", "vrai : les envoyer"),
    }
}

charge! {
    /// La charge de `incidents-lire` : une seule nature, ou toutes ; ceux qui attendent seulement (absent : tous).
    #[derive(Debug)]
    struct ChargeIncidents {
        quoi: Option<Nature> = QUOI,
        en_attente: Option<bool> = dit("Seulement ceux qui attendent", "vrai : seulement ceux qui ne sont pas partis").avancee(),
    }
}

charge! {
    /// La charge de `incidents-regrouper` et `incidents-oublier` : une seule nature, ou toutes.
    #[derive(Debug)]
    struct ChargeNature {
        quoi: Option<Nature> = QUOI,
    }
}

charge! {
    /// La charge d'un message au developpeur : son texte, ou l'on etait, ses captures.
    #[derive(Debug)]
    struct ChargeMessage {
        texte: String = dit("Message", "le message, en mots"),
        ecran: Option<String> = dit("Où l'on était", "où l'on était (l'écran, ou ce que le LLM a tenté : la route, la charge sans donnée personnelle, la réponse)"),
        captures: Option<Vec<CaptureDeLaPage>> = dit("Captures", "[{type, donnees}] : des images en base64").avancee(),
    }
}

impl ChargeMessage {
    fn captures(&self) -> &[CaptureDeLaPage] {
        match &self.captures {
            Some(c) => c,
            None => &[],
        }
    }
}

// ------------------------------------------------------------------ ce que les routes rendent

/// Ce que rend une route qui ne dit qu'un compte : sa phrase, et ce qu'elle a compte.
#[derive(Debug, Serialize)]
struct Compte {
    phrase: Phrase,
    #[serde(skip_serializing_if = "Option::is_none")]
    retires: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lues: Option<usize>,
}

/// Ce que rend `incident-navigateur`.
#[derive(Debug, Serialize)]
struct IncidentNote {
    phrase: Phrase,
    note: bool,
    empreinte: Option<incidents::CleDIncident>,
}

/// Ce que rendent `incidents-lire` et `incidents-regrouper` : les lignes gardees (`compagnie::incidents`), en phrases.
#[derive(Debug, Serialize)]
struct Incidents {
    phrase: Phrase,
    phrases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lignes: Option<Vec<Ligne>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    paquets: Option<Vec<Paquet>>,
}

/// Ce que rend un passage d'envoi (`incidents-envoyer`, `signalements-partir`) : son bilan, et sa phrase.
#[derive(Debug, Serialize)]
struct Envoi<B: Serialize> {
    phrase: Phrase,
    #[serde(flatten)]
    bilan: B,
}

/// Ce que rend `signalement-apercu` : ce que `compagnie::problemes` en dit, et sa phrase.
#[derive(Debug, Serialize)]
struct Message {
    phrase: Phrase,
    #[serde(flatten)]
    apercu: Apercu,
}

/// Un message au developpeur, tel que l'ecran le liste : ce que `compagnie::problemes` en dit, son jour en mots, sa
/// pastille et son depart.
#[derive(Debug, Serialize)]
struct SignalementDit {
    #[serde(flatten)]
    vu: SignalementVu,
    quand_dit: String,
    /// Ou l'editeur suit le signalement, dit en mots pour la page ; absent quand rien ne le suit.
    ticket_dit: Option<String>,
    pastille: Option<Pastille>,
    depart: Pastille,
}

/// Ce que rend `signalements` (et ce que `signalement-envoyer` rend avec lui) : les messages, les reponses non lues.
#[derive(Debug, Serialize)]
pub(crate) struct Signalements {
    phrase: Phrase,
    signalements: Vec<SignalementDit>,
    non_lues: usize,
    reponses_dit: Option<String>,
    phrases: Vec<String>,
    ecran: EcranDuMessage,
}

/// Ce que rend `signalement-envoyer` : le message enregistre, et la liste des messages.
#[derive(Debug, Serialize)]
struct Envoye {
    phrase: Phrase,
    id: problemes::IdDeMessage,
    deja: bool,
    signalements: Vec<SignalementDit>,
    non_lues: usize,
    reponses_dit: Option<String>,
    ecran: EcranDuMessage,
}

/// Exactement ce qui est parti d'un groupe : le dernier incident tel qu'il part, ou le nombre de paies d'une annee.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Contenu {
    Incident(Box<PourLEditeur>),
    Paies(ContenuParti),
}

/// Un groupe d'incidents semblables, tel que l'ecran « ce qui est parti chez l'editeur » le montre.
#[derive(Debug, Serialize)]
struct Groupe {
    quoi: &'static str,
    ou: String,
    genre: Option<String>,
    resume: String,
    combien: u64,
    partis: u64,
    en_attente: u64,
    #[serde(with = "socle::paris::ecrit::seconde::option")]
    premiere: Option<Moment>,
    #[serde(with = "socle::paris::ecrit::seconde::option")]
    derniere: Option<Moment>,
    #[serde(with = "socle::paris::ecrit::seconde::option")]
    parti_le: Option<Moment>,
    contenu: Contenu,
    dernier_dit: String,
    quoi_dit: &'static str,
    pastille: Pastille,
}

/// L'ecran « ce qui est parti chez l'editeur » : le serveur, si l'envoi est actif, les comptes, les groupes.
#[derive(Debug, Serialize)]
struct Diagnostic {
    phrase: Phrase,
    serveur: socle::garde_reseau::Url,
    envoi: bool,
    total: usize,
    partis: u64,
    en_attente: u64,
    #[serde(with = "socle::paris::ecrit::seconde::option")]
    dernier_envoi: Option<Moment>,
    paies_annuelles: Vec<AnneePartie>,
    groupes: Vec<Groupe>,
    pastille: Pastille,
    #[serde(skip_serializing_if = "Option::is_none")]
    bilan: Option<BilanIncidents>,
    #[serde(skip_serializing_if = "Option::is_none")]
    couleur: Option<&'static str>,
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
dites!(Compte, Incidents, Message, Signalements, Envoye, Diagnostic);

impl<B: Serialize> Dite for Envoi<B> {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for IncidentNote {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

// ------------------------------------------------------------------ les routes

fn route_de(racine: &Path, nom: Route, charge: &Charge) -> R<Rendu> {
    match nom {
        Route::Signalements => {
            let SansCharge {} = charge.lire()?;
            en_json(signalements_route(racine)?)
        }
        Route::IncidentNavigateur => {
            // Ce que la page a vu (`incidents::RapportNavigateur`), que le journal des incidents met en forme.
            let ligne = incidents::depuis_navigateur(racine, &charge.lire()?)?;
            en_json(IncidentNote {
                phrase: Phrase::fixe(if ligne.is_some() { "Incident noté." } else { "Rien n'est noté." }),
                note: ligne.is_some(),
                empreinte: ligne.map(|l| l.empreinte),
            })
        }
        Route::DiagnosticEtat => {
            let SansCharge {} = charge.lire()?;
            en_json(diagnostic_etat(racine)?)
        }
        Route::DiagnosticEnvoyer => {
            let SansCharge {} = charge.lire()?;
            let bilan = incidents::envoyer(racine);
            let mut etat = diagnostic_etat(racine)?;
            // La couleur du message qui le dit : un envoi impossible est refuse, sinon parti.
            etat.couleur = Some(socle::ecran::couleur_de_l_etat(if bilan.erreur.is_some() { socle::ecran::EtatDEcran::Refuse } else { socle::ecran::EtatDEcran::Fait }));
            etat.phrase = match &bilan.erreur {
                Some(e) => socle::phrase!("Envoi impossible : {}", e),
                None => socle::phrase!("{} incident(s) envoyé(s).", bilan.envoyes),
            };
            etat.bilan = Some(bilan);
            en_json(etat)
        }
        Route::IncidentsEnvoyer => {
            let SansCharge {} = charge.lire()?;
            let bilan = incidents::envoyer(racine);
            en_json(Envoi { phrase: phrase_d_envoi(bilan.envoyes, bilan.reste, bilan.erreur.as_deref(), bilan.occupe), bilan })
        }
        // Couper ou rendre l'envoi des incidents a l'editeur.
        Route::DiagnosticRegler => {
            let c: ChargeRegler = charge.lire()?;
            incidents::regler_envoi(racine, c.actif)?;
            let mut etat = diagnostic_etat(racine)?;
            etat.phrase = if etat.envoi {
                Phrase::fixe("Le signalement des dysfonctionnements est réactivé.")
            } else {
                Phrase::fixe("Le signalement des dysfonctionnements est désactivé : rien ne part plus chez l'éditeur.")
            };
            en_json(etat)
        }
        Route::IncidentsLire => {
            let c: ChargeIncidents = charge.lire()?;
            let lignes = incidents::lire(racine, c.quoi, c.en_attente == Some(true))?;
            en_json(Incidents { phrase: socle::phrase!("{} incident(s) gardé(s) sur ce poste.", lignes.len()), phrases: lignes.iter().map(phrase_d_incident).collect(), lignes: Some(lignes), paquets: None })
        }
        Route::IncidentsRegrouper => {
            let c: ChargeNature = charge.lire()?;
            let paquets = incidents::regrouper(racine, c.quoi)?;
            en_json(Incidents { phrase: socle::phrase!("{} sorte(s) d'incident gardée(s) sur ce poste.", paquets.len()), phrases: paquets.iter().map(phrase_de_paquet).collect(), lignes: None, paquets: Some(paquets) })
        }
        Route::IncidentsOublier => {
            let c: ChargeNature = charge.lire()?;
            let n = incidents::oublier(racine, c.quoi)?;
            en_json(Compte { phrase: socle::phrase!("{} incident(s) effacé(s) de ce poste.", n), retires: Some(n), lues: None })
        }
        Route::SignalementApercu => {
            let c: ChargeMessage = charge.lire()?;
            let apercu = problemes::apercu(racine, &c.texte, c.ecran.as_deref(), c.captures())?;
            let phrase = socle::phrase!("Ce message partirait chez {}, avec {} incidents et {} captures.", apercu.serveur, apercu.incidents, apercu.captures);
            en_json(Message { phrase, apercu })
        }
        Route::SignalementEnvoyer => {
            let c: ChargeMessage = charge.lire()?;
            let e = problemes::envoyer(racine, &c.texte, c.ecran.as_deref(), c.captures())?;
            let liste = signalements_route(racine)?;
            let depart = liste.signalements.iter().find(|s| s.vu.garde.id == e.id).map(|s| s.depart.mots.clone());
            let dit = if e.deja { "Ce message est déjà enregistré." } else { "Message enregistré pour le développeur." };
            let phrase = match depart {
                Some(d) => socle::phrase!("{} {}", dit, d),
                None => Phrase::fixe(dit),
            };
            en_json(Envoye { phrase, id: e.id, deja: e.deja, signalements: liste.signalements, non_lues: liste.non_lues, reponses_dit: liste.reponses_dit, ecran: liste.ecran })
        }
        Route::SignalementsLus => {
            let SansCharge {} = charge.lire()?;
            let n = problemes::signalements_lus(racine)?;
            en_json(Compte { phrase: socle::phrase!("Réponses marquées lues : {}.", n), retires: None, lues: Some(n) })
        }
        Route::SignalementsPartir => {
            let SansCharge {} = charge.lire()?;
            let bilan: BilanMessages = problemes::partir(racine);
            en_json(Envoi { phrase: phrase_d_envoi(bilan.partis, bilan.reste, bilan.erreur.as_deref(), bilan.occupe), bilan })
        }
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

/// LES PASTILLES DU DIAGNOSTIC (`couleur_de_l_etat`) : coupe par la compagnie, une information ; rien de note ou tout
/// parti, fait ; des incidents en attente, une information (ils partent d'eux-memes).
fn pastille_du_diagnostic(envoi: bool, total: usize, attente: u64) -> Pastille {
    if !envoi {
        Pastille::de(socle::ecran::EtatDEcran::Information, "désactivé")
    } else if total == 0 {
        Pastille::de(socle::ecran::EtatDEcran::Fait, "rien à signaler")
    } else if attente > 0 {
        Pastille::de(socle::ecran::EtatDEcran::Information, &format!("{} en attente", attente))
    } else {
        Pastille::de(socle::ecran::EtatDEcran::Fait, "tout est parti")
    }
}

/// Le nom de ce qui est parti, tel que la boite le titre.
fn quoi_dit(quoi: Nature) -> &'static str {
    match quoi {
        Nature::Logiciel => "plantage",
        Nature::Refus => "refus du moteur",
        Nature::Dpae => "DPAE rejetée",
        Nature::Dsn => "DSN rejetée",
    }
}

/// `diagnostic_etat` : l'ecran « ce qui est parti chez l'editeur ». Le journal des incidents, regroupe, avec pour chaque
/// groupe ce qui est parti et ce qui attend -- et, pour le dernier de chaque groupe, EXACTEMENT ce qui part
/// (`pour_l_editeur`), pas un resume : l'utilisateur lit ce qui quitte son poste.
fn diagnostic_etat(racine: &Path) -> R<Diagnostic> {
    let (serveur, _) = incidents::configuration(racine)?;
    let lignes = incidents::lire(racine, None, false)?;
    // Un groupe par cle d'incident ; un rejet de declaration, qui ne se regroupe pas, a la sienne avec son instant.
    let mut groupes: Vec<((incidents::CleDIncident, Option<Moment>), Groupe)> = Vec::new();
    for l in &lignes {
        let cle = (l.empreinte.clone(), (!l.quoi.se_regroupe()).then_some(l.quand));
        let i = match groupes.iter().position(|(k, _)| *k == cle) {
            Some(i) => i,
            None => {
                let g = Groupe {
                    quoi: l.quoi.nom(),
                    ou: l.ou.clone(),
                    genre: Some(l.genre.clone()),
                    resume: l.resume.clone(),
                    combien: 0,
                    partis: 0,
                    en_attente: 0,
                    premiere: Some(l.quand),
                    derniere: Some(l.quand),
                    parti_le: None,
                    contenu: Contenu::Incident(Box::new(incidents::pour_l_editeur(racine, l))),
                    dernier_dit: String::new(),
                    quoi_dit: quoi_dit(l.quoi),
                    pastille: Pastille::de(socle::ecran::EtatDEcran::Information, ""),
                };
                groupes.push((cle, g));
                groupes.len() - 1
            }
        };
        let g = &mut groupes[i].1;
        // Un incident revu (ses doublons, `fois`) compte autant de fois qu'il a eu lieu.
        g.combien += l.fois;
        g.derniere = Some(l.derniere);
        g.dernier_dit = format!("dernier le {}", l.derniere.dit());
        if !l.resume.is_empty() {
            g.resume = l.resume.clone();
        }
        g.contenu = Contenu::Incident(Box::new(incidents::pour_l_editeur(racine, l)));
        if l.envoi.etat == DEPOSE {
            g.partis += l.fois;
            if let Some(q) = l.envoi.quand {
                g.parti_le = Some(q);
            }
        } else if l.envoi.etat == EN_ATTENTE {
            g.en_attente += l.fois;
        }
    }
    let mut liste: Vec<Groupe> = groupes.into_iter().map(|(_, g)| g).collect();
    for g in liste.iter_mut() {
        g.pastille = match &g.parti_le {
            _ if g.en_attente > 0 => Pastille::de(socle::ecran::EtatDEcran::Information, &format!("{} en attente", g.en_attente)),
            Some(le) => Pastille::de(socle::ecran::EtatDEcran::Fait, &format!("parti le {}", le.dit())),
            None => Pastille::de(socle::ecran::EtatDEcran::Information, "pas encore parti"),
        };
    }
    let partis: Vec<Moment> =liste.iter().filter_map(|g| g.parti_le).collect();
    let (partis_total, en_attente_total) = (liste.iter().map(|g| g.partis).sum::<u64>(), liste.iter().map(|g| g.en_attente).sum::<u64>());
    // LE NOMBRE DE PAIES DE L'ANNEE se voit dans la meme liste : une entree par annee partie, avec exactement ce qui a
    // quitte le poste. Ce n'est pas un incident : il ne compte ni dans `total`, ni dans `partis`.
    let annees = compagnie::nombre_de_paies::annees_parties(racine)?;
    for a in &annees {
        liste.push(Groupe {
            quoi: "paies",
            ou: "une fois par an".to_string(),
            genre: None,
            resume: format!("{} : {} paie(s) faite(s)", a.annee, a.paies),
            combien: 1,
            partis: 1,
            en_attente: 0,
            premiere: a.parti_le,
            derniere: a.parti_le,
            parti_le: a.parti_le,
            contenu: Contenu::Paies(a.contenu.clone()),
            dernier_dit: if let Some(q) = a.parti_le { format!("dernier le {}", q.dit()) } else { "pas encore parti".to_string() },
            quoi_dit: "nombre de paies",
            pastille: match a.parti_le {
                Some(q) => Pastille::de(socle::ecran::EtatDEcran::Fait, &format!("parti le {}", q.dit())),
                None => Pastille::de(socle::ecran::EtatDEcran::Information, "pas encore parti"),
            },
        });
    }
    // Les plus recents d'abord ; tri stable.
    liste.sort_by(|a, b| b.derniere.cmp(&a.derniere));
    let envoi = incidents::envoi(racine)?;
    let dernier_envoi = partis.into_iter().max();
    let total = lignes.len();
    Ok(Diagnostic {
        phrase: socle::phrase!(
            "Signalement des dysfonctionnements : {} ; incidents gardés : {}, partis : {}, en attente : {}.",
            if envoi { "actif" } else { "désactivé" },
            total,
            partis_total,
            en_attente_total
        ),
        pastille: pastille_du_diagnostic(envoi, total, en_attente_total),
        serveur,
        envoi,
        total,
        partis: partis_total,
        en_attente: en_attente_total,
        dernier_envoi,
        paies_annuelles: annees,
        groupes: liste,
        bilan: None,
        couleur: None,
    })
}

/// La route `signalements` : les messages faits depuis ce poste, et les reponses non lues.
pub(crate) fn signalements_route(racine: &Path) -> R<Signalements> {
    let liste = problemes::signalements(racine)?;
    // Les reponses que la boite n'a pas encore montrees.
    let non_lues = liste.iter().filter(|s| s.garde.reponse.is_some() && s.garde.lu != Some(true)).count();
    let dits: Vec<SignalementDit> = liste
        .into_iter()
        .map(|vu| {
            // Son depart, que la boite du message dit juste apres l'envoi : PuppetPaie renvoie de lui-meme ce qui n'est
            // pas parti, rien n'est a faire (une information).
            let (pastille, depart) = match vu.etat {
                EtatDuMessage::EnCours => (Some(Pastille::de(socle::ecran::EtatDEcran::Information, "en cours d'envoi")), Pastille::de(socle::ecran::EtatDEcran::Information, "Envoi en cours…")),
                EtatDuMessage::PasParti => (Some(Pastille::de(socle::ecran::EtatDEcran::Information, "pas encore parti")), Pastille::de(socle::ecran::EtatDEcran::Information, "Pas encore parti")),
                EtatDuMessage::Arrive => (None, Pastille::de(socle::ecran::EtatDEcran::Fait, "Message envoyé.")),
            };
            let ticket_dit = vu.garde.ticket.as_ref().map(|_| match vu.garde.ticket_numero {
                Some(n) => format!("ticket n° {}", n),
                None => "ticket".to_string(),
            });
            SignalementDit { quand_dit: vu.garde.quand.dit(), ticket_dit, pastille, depart, vu }
        })
        .collect();
    let reponses_dit = (non_lues > 0).then(|| socle::ecran::compte(non_lues, "réponse", "réponses"));
    let phrases = dits.iter().map(|s| format!("« {} » : {}", s.vu.garde.texte, s.depart.mots.to_lowercase())).collect();
    Ok(Signalements {
        phrase: socle::phrase!("Messages au développeur : {} ; réponses non lues : {}.", dits.len(), non_lues),
        signalements: dits,
        non_lues,
        reponses_dit,
        phrases,
        ecran: ecran_du_message(),
    })
}

/// Un passage d'envoi (incidents ou messages) : ce qui est parti, ce qui attend, et pourquoi.
fn phrase_d_envoi(partis: usize, reste: usize, erreur: Option<&str>, occupe: bool) -> Phrase {
    if occupe {
        return Phrase::fixe("Un autre envoi est en cours : il prend aussi ceux-ci.");
    }
    let erreur: String = erreur.iter().map(|e| format!(" {}", e)).collect();
    socle::phrase!("{} parti(s) chez le développeur ; {} en attente.{}", partis, reste, erreur)
}

/// Un incident garde : quand, sa nature, ce qu'il dit, et s'il est parti.
fn phrase_d_incident(l: &Ligne) -> String {
    format!("{} · {} · {} — {}", l.quand.dit(), l.quoi.nom(), l.resume, if l.envoi.etat == DEPOSE { "parti" } else { "pas encore parti" })
}

/// Un paquet d'incidents semblables : combien, sa nature, ce qu'il dit, le dernier.
fn phrase_de_paquet(p: &Paquet) -> String {
    format!("{} fois · {} · {} — le dernier le {}", p.combien, p.quoi.nom(), p.resume, p.derniere.dit())
}

/// CE QUE LA BOITE « Un problème, une idée ? » DIT (`ecran` de `signalements` et `signalement-envoyer`) : la question, les
/// captures, ce qui part avec le message, et ce qu'elle dit d'un message envoye.
#[derive(Debug, Serialize)]
pub(crate) struct EcranDuMessage {
    question: &'static str,
    captures: String,
    ce_qui_part: &'static str,
    trop: String,
    format: &'static str,
    taille: &'static str,
    ratee: &'static str,
    merci: &'static str,
    merci_parti: &'static str,
    renverra: &'static str,
    deja: &'static str,
    attente: &'static str,
}

fn ecran_du_message() -> EcranDuMessage {
    EcranDuMessage {
        question: "Ce qui ne va pas, ce qui vous manque, ce que vous aimeriez :",
        captures: format!("Captures d'écran, {} au plus : collez-les (Ctrl+V), choisissez des images ou capturez l'écran.", problemes::CAPTURES_MAX),
        ce_qui_part: "Pour qu'on trouve la panne sans vous redemander, partent avec votre message : cet ordinateur (système, version, mémoire, disque, écran), tout ce que vous et PuppetPaie avez fait ces 3 dernières minutes, et les derniers dysfonctionnements notés. Les noms, NIR, adresses, IBAN, naissances, courriels et téléphones de vos salariés y sont remplacés par leur forme (« salarié n°2 ») et la compagnie n'est pas nommée ; montants, dates et codes restent tels quels.",
        trop: format!("{} captures au plus.", problemes::CAPTURES_MAX),
        format: "Une capture doit être une image PNG ou JPEG.",
        taille: "Une capture dépasse 1,5 Mo : réduisez-la, ou découpez-la.",
        ratee: "La capture n'a pas pu se faire.",
        merci: "Merci, votre message est enregistré.",
        merci_parti: "Merci ! La réponse de l'éditeur viendra ici, dans « Vos messages ».",
        renverra: "PuppetPaie le renverra de lui-même.",
        deja: "Ce message était déjà enregistré : il ne part qu'une fois.",
        attente: "arrivé, en attente de réponse",
    }
}
