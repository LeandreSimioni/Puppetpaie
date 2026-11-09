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

use serde::{Deserialize, Serialize};

use compagnie::incidents::{self, BilanIncidents, Ligne, Nature, Paquet, PourLEditeur, DEPOSE, EN_ATTENTE};
use compagnie::nombre_de_paies::{AnneePartie, ContenuParti};
use compagnie::problemes::{self, Apercu, BilanMessages, CaptureDeLaPage, EtatDuMessage, SignalementVu};
use crate::catalogue::{Acte, Affiche, Champ, Forme, Fiche, FORME, OPERATION};
use crate::catalogue::Route;
use crate::catalogue::{Ecran, Place};
use crate::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use crate::SansCharge;
use socle::ecran::Pastille;
use socle::echec::{erreur, Classe, R};

const TEXTE: Champ = Champ { nom: "texte", libelle: Affiche::new("Message"), forme: Forme::Texte, sens: "le message, en mots", avance: false };
const OU: Champ = Champ { nom: "ecran", libelle: Affiche::new("Où l'on était"), forme: Forme::Texte, sens: "où l'on était (l'écran, ou ce que le LLM a tenté : la route, la charge sans donnée personnelle, la réponse)", avance: false };
const CAPTURES: Champ = Champ { nom: "captures", libelle: Affiche::new("Captures"), forme: Forme::Objet, sens: "[{type, donnees}] : des images en base64", avance: true };
const QUOI: Champ = Champ { nom: "quoi", libelle: Affiche::new("Nature"), forme: Forme::Texte, sens: "une seule nature d'incident ; toutes sinon", avance: true };
const DIAGNOSTIC_REND: &str = "envoi (vrai : actif), total, en_attente, groupes (ce qui part, mot pour mot), pastille";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Signaler);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::SignalementApercu,
        geste: "Montrer, mot pour mot, ce qu'un message au développeur emporterait.",
        charge: &[TEXTE, OU, CAPTURES],
        exemple: r#"{"texte":"La phrase de la DSN est coupée.","ecran":"Paie, novembre 2026"}"#,
        rend: "corps (ce qui part), caviarde, serveur, captures, incidents",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::SignalementEnvoyer,
        geste: "Écrire au développeur (un problème, une idée) ; le message part seul.",
        charge: &[TEXTE, OU, CAPTURES],
        exemple: r#"{"texte":"La phrase de la DSN est coupée.","ecran":"Paie, novembre 2026"}"#,
        rend: "signalements (chacun son état et son départ)",
        refus: &[OPERATION, FORME],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Signalements,
        geste: "Dire les messages écrits au développeur, et ses réponses.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "signalements (chacun son état, sa pastille, ses réponses)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::SignalementsLus,
        geste: "Marquer lues les réponses du développeur.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "lues (le nombre marqué)",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::SignalementsPartir,
        geste: "Faire partir chez le développeur les messages qui attendent.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "le bilan du passage (partis, reste, erreur, occupe), phrase ; ne refuse jamais",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler, Affiche::new("Envoyer les messages qui attendent")),
    },
    Fiche {
        route: Route::IncidentNavigateur,
        geste: "Noter ce qui a cassé dans la page.",
        charge: &[Champ { nom: "message", libelle: Affiche::new("Ce que la page a vu"), forme: Forme::Texte, sens: "ce que la page a vu (et genre, fichier, ligne, colonne, pile, canal, ou_dans_la_page)", avance: false }],
        exemple: r#"{"genre":"TypeError","message":"exemple","fichier":"","ligne":0,"canal":"erreur","ou_dans_la_page":"paie"}"#,
        rend: "note (vrai : noté), empreinte",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::DiagnosticEtat,
        geste: "Dire ce qui est parti chez le développeur, et ce qui attend.",
        charge: &[],
        exemple: r#"{}"#,
        rend: DIAGNOSTIC_REND,
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::DiagnosticEnvoyer,
        geste: "Envoyer au développeur les incidents qui attendent.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "ce que diagnostic-etat rend, bilan et couleur",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::DiagnosticRegler,
        geste: "Couper l'envoi des incidents au développeur, ou le rétablir.",
        charge: &[Champ { nom: "actif", libelle: Affiche::new("Envoyer les incidents"), forme: Forme::OuiNon, sens: "vrai : les envoyer", avance: false }],
        exemple: r#"{"actif":true}"#,
        rend: DIAGNOSTIC_REND,
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::IncidentsLire,
        geste: "Lire les incidents gardés sur le poste.",
        charge: &[QUOI, Champ { nom: "en_attente", libelle: Affiche::new("Seulement ceux qui attendent"), forme: Forme::OuiNon, sens: "vrai : seulement ceux qui ne sont pas partis", avance: true }],
        exemple: r#"{}"#,
        rend: "phrase, phrases (une par incident), lignes (les plus récentes en dernier)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler, Affiche::new("Lire les incidents gardés")),
    },
    Fiche {
        route: Route::IncidentsRegrouper,
        geste: "Regrouper les incidents gardés, le plus fréquent en tête.",
        charge: &[QUOI],
        exemple: r#"{}"#,
        rend: "phrase, phrases (une par paquet), paquets",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler, Affiche::new("Regrouper les incidents")),
    },
    Fiche {
        route: Route::IncidentsEnvoyer,
        geste: "Déposer chez le développeur les incidents qui attendent.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "envoyes, reste, erreur, occupe, phrase ; ne refuse jamais",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: Ecran::Auto(Place::Signaler, Affiche::new("Envoyer les incidents qui attendent")),
    },
    Fiche {
        route: Route::IncidentsOublier,
        geste: "Effacer les incidents gardés, tous ou d'une nature.",
        charge: &[QUOI],
        exemple: r#"{"quoi":"logiciel"}"#,
        rend: "retires (le nombre), phrase",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
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
    std::fs::create_dir_all(&racine).map_err(|e| erreur(Classe::du_disque(&e), socle::echec::du_disque(&racine, &e)))?;
    route_de(&racine, nom, charge)
}

// ------------------------------------------------------------------ les charges

/// La charge de `diagnostic-regler`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeRegler {
    actif: bool,
}

/// La charge de `incidents-lire` : une seule nature, ou toutes ; ceux qui attendent seulement (absent : tous).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeIncidents {
    quoi: Option<Nature>,
    en_attente: Option<bool>,
}

/// La charge de `incidents-regrouper` et `incidents-oublier` : une seule nature, ou toutes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeNature {
    quoi: Option<Nature>,
}

/// La charge d'un message au developpeur : son texte, ou l'on etait, ses captures.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeMessage {
    texte: String,
    ecran: Option<String>,
    captures: Option<Vec<CaptureDeLaPage>>,
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
    phrase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    retires: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lues: Option<usize>,
}

/// Ce que rend `incident-navigateur`.
#[derive(Debug, Serialize)]
struct IncidentNote {
    phrase: &'static str,
    note: bool,
    empreinte: Option<String>,
}

/// Ce que rendent `incidents-lire` et `incidents-regrouper` : les lignes gardees (`compagnie::incidents`), en phrases.
#[derive(Debug, Serialize)]
struct Incidents {
    phrase: String,
    phrases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lignes: Option<Vec<Ligne>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    paquets: Option<Vec<Paquet>>,
}

/// Ce que rend un passage d'envoi (`incidents-envoyer`, `signalements-partir`) : son bilan, et sa phrase.
#[derive(Debug, Serialize)]
struct Envoi<B: Serialize> {
    phrase: String,
    #[serde(flatten)]
    bilan: B,
}

/// Ce que rend `signalement-apercu` : ce que `compagnie::problemes` en dit, et sa phrase.
#[derive(Debug, Serialize)]
struct Message {
    phrase: String,
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
    pastille: Option<Pastille>,
    depart: Pastille,
}

/// Ce que rend `signalements` (et ce que `signalement-envoyer` rend avec lui) : les messages, les reponses non lues.
#[derive(Debug, Serialize)]
pub(crate) struct Signalements {
    phrase: String,
    signalements: Vec<SignalementDit>,
    non_lues: usize,
    reponses_dit: Option<String>,
    phrases: Vec<String>,
    ecran: EcranDuMessage,
}

/// Ce que rend `signalement-envoyer` : le message enregistre, et la liste des messages.
#[derive(Debug, Serialize)]
struct Envoye {
    phrase: String,
    id: String,
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
    premiere: Option<String>,
    derniere: Option<String>,
    parti_le: Option<String>,
    contenu: Contenu,
    dernier_dit: String,
    quoi_dit: &'static str,
    pastille: Pastille,
}

/// L'ecran « ce qui est parti chez l'editeur » : le serveur, si l'envoi est actif, les comptes, les groupes.
#[derive(Debug, Serialize)]
struct Diagnostic {
    phrase: String,
    serveur: String,
    envoi: bool,
    total: usize,
    partis: u64,
    en_attente: u64,
    dernier_envoi: Option<String>,
    paies_annuelles: Vec<AnneePartie>,
    groupes: Vec<Groupe>,
    pastille: Pastille,
    ecran: EcranDuDiagnostic,
    #[serde(skip_serializing_if = "Option::is_none")]
    bilan: Option<BilanIncidents>,
    #[serde(skip_serializing_if = "Option::is_none")]
    couleur: Option<&'static str>,
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
dites!(Compte, Incidents, Message, Signalements, Envoye, Diagnostic);

impl<B: Serialize> Dite for Envoi<B> {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for IncidentNote {
    fn phrase(&self) -> &str {
        self.phrase
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
                phrase: if ligne.is_some() { "Incident noté." } else { "Rien n'est noté." },
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
            etat.couleur = Some(socle::ecran::couleur_de_l_etat(if bilan.erreur.is_some() { "refuse" } else { "fait" }));
            etat.phrase = match &bilan.erreur {
                Some(e) => format!("Envoi impossible : {}", e),
                None => format!("{} incident(s) envoyé(s).", bilan.envoyes),
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
                "Le signalement des dysfonctionnements est réactivé.".to_string()
            } else {
                "Le signalement des dysfonctionnements est désactivé : rien ne part plus chez l'éditeur.".to_string()
            };
            en_json(etat)
        }
        Route::IncidentsLire => {
            let c: ChargeIncidents = charge.lire()?;
            let lignes = incidents::lire(racine, c.quoi, c.en_attente == Some(true))?;
            en_json(Incidents { phrase: format!("{} incident(s) gardé(s) sur ce poste.", lignes.len()), phrases: lignes.iter().map(phrase_d_incident).collect(), lignes: Some(lignes), paquets: None })
        }
        Route::IncidentsRegrouper => {
            let c: ChargeNature = charge.lire()?;
            let paquets = incidents::regrouper(racine, c.quoi)?;
            en_json(Incidents { phrase: format!("{} sorte(s) d'incident gardée(s) sur ce poste.", paquets.len()), phrases: paquets.iter().map(phrase_de_paquet).collect(), lignes: None, paquets: Some(paquets) })
        }
        Route::IncidentsOublier => {
            let c: ChargeNature = charge.lire()?;
            let n = incidents::oublier(racine, c.quoi)?;
            en_json(Compte { phrase: format!("{} incident(s) effacé(s) de ce poste.", n), retires: Some(n), lues: None })
        }
        Route::SignalementApercu => {
            let c: ChargeMessage = charge.lire()?;
            let apercu = problemes::apercu(racine, &c.texte, c.ecran.as_deref(), c.captures())?;
            let phrase = format!("Ce message partirait chez {}, avec {} incidents et {} captures.", apercu.serveur, apercu.incidents, apercu.captures);
            en_json(Message { phrase, apercu })
        }
        Route::SignalementEnvoyer => {
            let c: ChargeMessage = charge.lire()?;
            let e = problemes::envoyer(racine, &c.texte, c.ecran.as_deref(), c.captures())?;
            let liste = signalements_route(racine)?;
            let depart = liste.signalements.iter().find(|s| s.vu.garde.id == e.id).map(|s| s.depart.mots.clone());
            let dit = if e.deja { "Ce message est déjà enregistré." } else { "Message enregistré pour le développeur." };
            let phrase = match depart {
                Some(d) => format!("{} {}", dit, d),
                None => dit.to_string(),
            };
            en_json(Envoye { phrase, id: e.id, deja: e.deja, signalements: liste.signalements, non_lues: liste.non_lues, reponses_dit: liste.reponses_dit, ecran: liste.ecran })
        }
        Route::SignalementsLus => {
            let SansCharge {} = charge.lire()?;
            let n = problemes::signalements_lus(racine)?;
            en_json(Compte { phrase: format!("Réponses marquées lues : {}.", n), retires: None, lues: Some(n) })
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
        Pastille::de("information", "désactivé")
    } else if total == 0 {
        Pastille::de("fait", "rien à signaler")
    } else if attente > 0 {
        Pastille::de("information", &format!("{} en attente", attente))
    } else {
        Pastille::de("fait", "tout est parti")
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
    let mut groupes: Vec<(String, Groupe)> = Vec::new();
    for l in &lignes {
        let cle = if l.quoi.se_regroupe() { l.empreinte.clone() } else { format!("{}|{}", l.empreinte, l.quand) };
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
                    premiere: Some(l.quand.clone()),
                    derniere: Some(l.quand.clone()),
                    parti_le: None,
                    contenu: Contenu::Incident(Box::new(incidents::pour_l_editeur(racine, l))),
                    dernier_dit: String::new(),
                    quoi_dit: quoi_dit(l.quoi),
                    pastille: Pastille::de("information", ""),
                };
                groupes.push((cle, g));
                groupes.len() - 1
            }
        };
        let g = &mut groupes[i].1;
        // Un incident revu (ses doublons, `fois`) compte autant de fois qu'il a eu lieu.
        g.combien += l.fois;
        g.derniere = Some(l.derniere.clone());
        g.dernier_dit = format!("dernier le {}", socle::ecran::quand(&l.derniere));
        if !l.resume.is_empty() {
            g.resume = l.resume.clone();
        }
        g.contenu = Contenu::Incident(Box::new(incidents::pour_l_editeur(racine, l)));
        if l.envoi.etat == DEPOSE {
            g.partis += l.fois;
            if let Some(q) = &l.envoi.quand {
                g.parti_le = Some(q.clone());
            }
        } else if l.envoi.etat == EN_ATTENTE {
            g.en_attente += l.fois;
        }
    }
    let mut liste: Vec<Groupe> = groupes.into_iter().map(|(_, g)| g).collect();
    for g in liste.iter_mut() {
        g.pastille = match &g.parti_le {
            _ if g.en_attente > 0 => Pastille::de("information", &format!("{} en attente", g.en_attente)),
            Some(le) => Pastille::de("fait", &format!("parti le {}", socle::ecran::quand(le))),
            None => Pastille::de("information", "pas encore parti"),
        };
    }
    let partis: Vec<String> = liste.iter().filter_map(|g| g.parti_le.clone()).collect();
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
            premiere: a.parti_le.clone(),
            derniere: a.parti_le.clone(),
            parti_le: a.parti_le.clone(),
            contenu: Contenu::Paies(a.contenu.clone()),
            dernier_dit: if let Some(q) = &a.parti_le { format!("dernier le {}", socle::ecran::quand(q)) } else { "pas encore parti".to_string() },
            quoi_dit: "nombre de paies",
            pastille: match &a.parti_le {
                Some(q) => Pastille::de("fait", &format!("parti le {}", socle::ecran::quand(q))),
                None => Pastille::de("information", "pas encore parti"),
            },
        });
    }
    // Les plus recents d'abord ; tri stable.
    liste.sort_by(|a, b| b.derniere.cmp(&a.derniere));
    let envoi = incidents::envoi(racine)?;
    let dernier_envoi = partis.into_iter().max();
    let total = lignes.len();
    let ecran = ecran_du_diagnostic(envoi, &serveur, total, partis_total, dernier_envoi.as_deref(), annees.first());
    Ok(Diagnostic {
        phrase: format!(
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
        ecran,
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
                EtatDuMessage::EnCours => (Some(Pastille::de("information", "en cours d'envoi")), Pastille::de("information", "Envoi en cours…")),
                EtatDuMessage::PasParti => (Some(Pastille::de("information", "pas encore parti")), Pastille::de("information", "Pas encore parti")),
                EtatDuMessage::Arrive => (None, Pastille::de("fait", "Message envoyé.")),
            };
            SignalementDit { quand_dit: socle::ecran::quand(&vu.garde.quand), pastille, depart, vu }
        })
        .collect();
    let reponses_dit = (non_lues > 0).then(|| socle::ecran::compte(non_lues, "réponse", "réponses"));
    let phrases = dits.iter().map(|s| format!("« {} » : {}", s.vu.garde.texte, s.depart.mots.to_lowercase())).collect();
    Ok(Signalements {
        phrase: format!("Messages au développeur : {} ; réponses non lues : {}.", dits.len(), non_lues),
        signalements: dits,
        non_lues,
        reponses_dit,
        phrases,
        ecran: ecran_du_message(),
    })
}

/// Un passage d'envoi (incidents ou messages) : ce qui est parti, ce qui attend, et pourquoi.
fn phrase_d_envoi(partis: usize, reste: usize, erreur: Option<&str>, occupe: bool) -> String {
    if occupe {
        return "Un autre envoi est en cours : il prend aussi ceux-ci.".to_string();
    }
    let mut p = format!("{} parti(s) chez le développeur ; {} en attente.", partis, reste);
    if let Some(e) = erreur {
        p.push_str(&format!(" {}", e));
    }
    p
}

/// Un incident garde : quand, sa nature, ce qu'il dit, et s'il est parti.
fn phrase_d_incident(l: &Ligne) -> String {
    format!("{} · {} · {} — {}", socle::ecran::quand(&l.quand), l.quoi.nom(), l.resume, if l.envoi.etat == DEPOSE { "parti" } else { "pas encore parti" })
}

/// Un paquet d'incidents semblables : combien, sa nature, ce qu'il dit, le dernier.
fn phrase_de_paquet(p: &Paquet) -> String {
    format!("{} fois · {} · {} — le dernier le {}", p.combien, p.quoi.nom(), p.resume, socle::ecran::quand(&p.derniere))
}

/// La boite qui avertit avant de couper l'envoi.
#[derive(Debug, Serialize)]
struct Desactiver {
    titre: &'static str,
    phrase: &'static str,
    note: &'static str,
    question: &'static str,
}

/// CE QUE L'ECRAN DU FILET DIT (`ecran` des routes `diagnostic-*`) : la ligne de son etat, la boite qui avertit avant de
/// couper l'envoi, et celle qui montre ce qui est parti.
#[derive(Debug, Serialize)]
struct EcranDuDiagnostic {
    compte: Option<String>,
    couper: &'static str,
    desactiver: Desactiver,
    parti: String,
    rien: &'static str,
}

fn ecran_du_diagnostic(envoi: bool, serveur: &str, total: usize, partis: u64, dernier_envoi: Option<&str>, annee: Option<&AnneePartie>) -> EcranDuDiagnostic {
    let mut compte = match (envoi, dernier_envoi) {
        (false, _) => Some("désactivé : les incidents restent sur cet ordinateur, rien ne part".to_string()),
        (true, Some(d)) => Some(format!("{} incident(s) parti(s), le dernier le {}", partis, socle::ecran::quand(d))),
        (true, None) if total > 0 => Some("rien n'est encore parti".to_string()),
        (true, None) => None,
    };
    if let (true, Some(a)) = (envoi, annee) {
        let mut paies = format!("nombre de paies de {}", a.annee);
        if let Some(q) = &a.parti_le {
            paies.push_str(&format!(" parti le {}", socle::ecran::quand(q)));
        }
        compte = Some(match compte {
            Some(c) => format!("{} · {}", c, paies),
            None => paies,
        });
    }
    EcranDuDiagnostic {
        compte,
        couper: if envoi { "Désactiver" } else { "Réactiver" },
        desactiver: Desactiver {
            titre: "Désactiver le signalement des dysfonctionnements ?",
            phrase: "Si vous le désactivez, l'éditeur ne verra plus les problèmes de votre logiciel, et risque de ne pas pouvoir les corriger avant qu'ils vous arrivent, ou qu'ils arrivent à d'autres compagnies. Il ne saura plus non plus combien de paies PuppetPaie fait chez vous : le logiciel est gratuit, c'est sa seule façon de savoir s'il sert.",
            note: "Ce qui part : chaque dysfonctionnement avec de quoi le reproduire (ce que le logiciel faisait, ses données, les 3 minutes d'avant, cet ordinateur), et une fois par an, à partir du 16 janvier, le nombre de paies faites l'année précédente. Les noms, NIR, adresses, IBAN et coordonnées de vos salariés n'en partent pas : ils sont remplacés par leur forme. Les incidents resteront notés sur cet ordinateur, et vous pourrez réactiver l'envoi à tout moment.",
            question: "Voulez-vous vraiment continuer ?",
        },
        parti: format!(
            "Quand quelque chose casse, PuppetPaie le note et l'envoie au serveur de l'éditeur, pour qu'il le corrige. Une fois par an, à partir du 16 janvier, il envoie aussi le nombre de paies faites l'année précédente : le logiciel est gratuit, c'est la seule façon pour l'éditeur de savoir s'il sert. Ce qui part est affiché ici tel quel : ce que le logiciel faisait et ses données (montants, dates, codes, spectacles, lieux), ce que vous et le logiciel avez fait les 3 minutes d'avant, et cet ordinateur (système, version, mémoire, disque, écran). Les noms, NIR, adresses, IBAN, naissances, courriels et téléphones des salariés sont remplacés par leur forme (« salarié n°2 », « NIR présent, clé valide ») ; la compagnie n'est pas nommée et le poste est désigné par un numéro tiré au hasard. Serveur : {}.",
            serveur
        ),
        rien: "Rien n'a été noté.",
    }
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
