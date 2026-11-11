//! Le canal vers l'editeur : le POST JSON vers le serveur de l'editeur, pour les incidents
//! (`rapport`), « Un probleme, une idee ? » (`retour`, `reponses`) et le nombre de paies de l'annee (`paies`).
//!
//! CHAQUE ACTION A SES FORMES : ce qui part (une structure `serde`, ecrite en JSON ASCII par `socle::json_ascii`) et ce
//! que le serveur rend (`Identite`, `Rapport`, `Retour`, `Reponses`, `Paies` ; un refus du serveur, `{"erreur"}`), lu par
//! `socle::forme`. Les formes sont celles du serveur (`infra/filet/filet_serveur.py` du site de l'editeur, `do_POST`).
//!
//! Le PAQUET SOUDE, bride pour toujours (`bridage::paquet_soude`) ne garde que
//! les actions de l'editeur (et `poste`) : les autres levent `EditionBridee`.
//!
//! UNE CLE PAR POSTE, AUCUN SECRET PARTAGE. Le
//! serveur donne a chaque poste son numero et sa cle (`POST /poste`) ; les
//! actions du poste portent le numero dans le corps et la cle dans `X-Filet-Cle`.
//!
//! Jamais le vrai serveur dans les essais : l'adresse vient de la
//! configuration, et les essais en donnent une locale. Quand la configuration
//! ne dit rien, c'est `SERVEUR_PAR_DEFAUT` : en essai, le garde-fou
//! reseau (`garde_reseau`) le refuse avant toute connexion ; un essai qui veut
//! le voir nomme un faux mandataire local comme seule porte (`tests/outils`,
//! `mandataire`), qui refuse.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use socle::echec::{Classe, Echec};
use envois::transport::{envoyer, Panne, Requete};

/// Ce que le paquet soude (bride pour toujours) peut poster.
pub const ACTIONS_BRIDEES: [&str; 4] = ["poste", "rapport", "retour", "reponses"];

/// Ce que dit le moteur a un 401 du serveur sur une action du poste : l'identite se redemande.
pub const REDEMANDER_IDENTITE: &str = "Le serveur ne reconnait plus ce poste : retirer \"cle\" de config/diagnostic.json pour lui redemander une identite.";

/// Le serveur de l'editeur QUAND LA CONFIGURATION NE DIT RIEN : les
/// incidents, « Un probleme, une idee ? » et leurs reponses. Il repond sur `filet.puppetpaie.fr` (le serveur vit dans le depot du site de l'editeur, `infra/filet/`). Ce qu'une compagnie a
/// pose le remplace toujours : `serveur` de `config/diagnostic.json`, ou `EDITEUR.json` du
/// paquet. Jamais dans les essais : le garde-fou reseau (`garde_reseau`) le refuse ; un test pose son serveur local, ou
/// nomme un faux mandataire local comme seule porte.
pub const SERVEUR_PAR_DEFAUT: &str = "https://filet.puppetpaie.fr";

/// Le filet ne doit jamais faire attendre une paie.
pub const DELAI: u64 = 15;

/// Une erreur du filet, que les appelants rattrapent (le passage suivant reessaie), ou une autre panne, un refus qui
/// remonte.
#[derive(Debug)]
pub enum ErreurFilet {
    /// Le serveur ne repond pas, ou refuse : ce qu'il en dit, et son statut HTTP (0 sans reponse).
    Filet { message: String, statut: u16 },
    /// Une autre panne, sous sa classe.
    Autre(Echec),
}

impl ErreurFilet {
    pub(crate) fn filet(message: String, statut: u16) -> ErreurFilet {
        ErreurFilet::Filet { message, statut }
    }

    /// Une vraie erreur du filet, que les appelants rattrapent ; sinon une autre panne, qui remonte.
    pub fn est_filet(&self) -> bool {
        matches!(self, ErreurFilet::Filet { .. })
    }

    /// Ce que le bilan d'un passage en dit.
    pub fn dite(self) -> String {
        match self {
            ErreurFilet::Filet { message, .. } => message,
            ErreurFilet::Autre(e) => e.phrase,
        }
    }

    /// Le refus qui remonte de la route.
    #[track_caller]
    pub fn en_echec(self) -> Echec {
        match self {
            ErreurFilet::Filet { message, .. } => socle::echec::erreur(Classe::Filet, message),
            ErreurFilet::Autre(e) => e,
        }
    }
}

// ------------------------------------------------------------------ ce que le serveur rend

/// `POST /poste` : l'identite que le serveur donne a ce poste (`creer_poste`), rendue une seule fois.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identite {
    pub poste: String,
    pub cle: String,
}

/// `POST /rapport` : les cles `(empreinte, quand)` recues, et celles que le serveur a ecartees (une ligne mal formee :
/// le poste la marque deposee quand meme). Le serveur rend le `quand` que le poste a envoye, tel quel
/// (`rapporter` de `infra/filet/filet_serveur.py`) : l'instant de la ligne.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rapport {
    pub recus: Vec<(String, socle::paris::InstantEcrit)>,
    pub ecartes: Vec<(String, socle::paris::InstantEcrit)>,
}

/// `POST /retour` : le numero que l'utilisateur lit, et l'adresse ou l'editeur le suit si elle est deja ouverte (vide
/// sinon ; son numero absent).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retour {
    pub numero: String,
    pub ticket: String,
    pub ticket_numero: Option<i64>,
}

/// `POST /reponses` : les reponses aux retours de ce poste.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reponses {
    pub retours: Vec<ReponseDuServeur>,
}

/// La reponse a un retour : vide tant que l'editeur n'a pas repondu.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReponseDuServeur {
    pub numero: String,
    pub reponse: String,
    #[serde(deserialize_with = "socle::paris::MinuteUtc::ou_vide")]
    pub repondu_le: Option<socle::paris::MinuteUtc>,
}

/// `POST /paies` : ce que le serveur a recu.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paies {
    pub recu: bool,
    pub annee: i32,
    pub paies: usize,
}

/// Le corps d'un refus du serveur (un statut hors 2xx).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefusDuServeur {
    erreur: String,
}

// ------------------------------------------------------------------ ce qui part

/// `POST /poste` ne porte rien.
#[derive(Serialize)]
struct Rien {}

/// Une action du poste : le numero du poste, et ce qu'elle porte.
#[derive(Serialize)]
struct DuPoste<'a, C: Serialize> {
    poste: &'a str,
    #[serde(flatten)]
    corps: C,
}

#[derive(Serialize)]
struct Incidents<'a, I: Serialize> {
    incidents: &'a [I],
}

#[derive(Serialize)]
struct CompteDesPaies {
    annee: i32,
    paies: usize,
}

#[derive(Serialize)]
struct Numeros<'a> {
    numeros: &'a [String],
}

/// Le message d'`EditionBridee(quoi)`.
fn edition_bridee(quoi: &str) -> String {
    format!(
        "Logiciel bride : cet envoi n'existe pas dans ce paquet ({}). Debloque, \
         c'est ici que le fichier part vers l'organisme ou que le courriel \
         s'envoie ; ici, rien ne quitte le poste.",
        quoi
    )
}

/// Un code de réponse HTTP en mots : son code, et sa phrase quand le code en a une.
fn statut_dit(code: u16) -> String {
    match phrase(code) {
        Some(p) => format!("HTTP Error {}: {}", code, p),
        None => format!("HTTP Error {}", code),
    }
}

/// La phrase HTTP d'un code de réponse, s'il en a une.
fn phrase(code: u16) -> Option<&'static str> {
    Some(match code {
        100 => "Continue",
        101 => "Switching Protocols",
        102 => "Processing",
        103 => "Early Hints",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        207 => "Multi-Status",
        208 => "Already Reported",
        226 => "IM Used",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        305 => "Use Proxy",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Request Entity Too Large",
        414 => "Request-URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Requested Range Not Satisfiable",
        417 => "Expectation Failed",
        418 => "I'm a Teapot",
        421 => "Misdirected Request",
        422 => "Unprocessable Entity",
        423 => "Locked",
        424 => "Failed Dependency",
        425 => "Too Early",
        426 => "Upgrade Required",
        428 => "Precondition Required",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        451 => "Unavailable For Legal Reasons",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",
        506 => "Variant Also Negotiates",
        507 => "Insufficient Storage",
        508 => "Loop Detected",
        510 => "Not Extended",
        511 => "Network Authentication Required",
        _sans_phrase => return None,
    })
}

/// Une panne du transport, en mots.
fn panne_dite(p: &Panne) -> String {
    match p {
        Panne::Statut { code, .. } => statut_dit(*code),
        Panne::Reseau(raison) => raison.trim_end_matches('.').to_string(),
        Panne::Brute(e) => e.phrase.trim_end_matches('.').to_string(),
    }
}

/// Ce que le serveur dit d'un refus : son `erreur` ; un corps vide ou qui n'a pas cette forme laisse le statut seul le
/// dire, et dit pourquoi.
fn refus_dit(code: u16, corps: &Result<Vec<u8>, Box<Panne>>) -> String {
    let propre = statut_dit(code);
    match corps {
        Err(p) => format!("{} (corps de la réponse non lu : {})", propre, panne_dite(p)),
        Ok(o) if o.is_empty() => propre,
        Ok(o) => match std::str::from_utf8(o) {
            Err(e) => format!("{} (corps de la réponse illisible : {})", propre, e),
            Ok(t) => match socle::forme::lire_texte::<RefusDuServeur>(t.trim_start_matches('\u{feff}'), "corps de la réponse illisible") {
                Ok(r) if !r.erreur.is_empty() => r.erreur,
                Ok(_) => propre,
                Err(e) => format!("{} ({})", propre, e.phrase.trim_end_matches('.')),
            },
        },
    }
}

/// La reponse du serveur a l'action `action`, lue en sa forme, ou l'`ErreurFilet` qu'elle a causee.
pub fn poster<C: Serialize + ?Sized, T: DeserializeOwned>(serveur: &str, action: &str, corps: &C, entetes: &[(&str, &str)], delai: u64) -> Result<T, ErreurFilet> {
    if crate::bridage::paquet_soude() && !ACTIONS_BRIDEES.contains(&action) {
        return Err(ErreurFilet::Autre(socle::echec::erreur(Classe::EditionBridee, edition_bridee(action))));
    }
    let donnees = socle::json_ascii::dumps(corps, None, false, false);
    let url = format!("{}/{}", serveur.trim_end_matches('/'), action);
    // Aucun codage demandé.
    let mut en_tetes: Vec<(&str, &str)> = vec![("User-Agent", "PuppetPaie-filet"), ("Accept-Encoding", "identity")];
    en_tetes.extend_from_slice(entetes);
    let r = Requete {
        url: &url,
        donnees: Some(donnees.as_bytes()),
        content_type: Some("application/json"),
        post: true,
        delai,
        entetes: &en_tetes,
        sans_gzip: true,
        ..Default::default()
    };
    match envoyer(&r) {
        Ok((brut, _)) => {
            // Une reponse qui ne se lit pas se range avec les pannes de reseau : le passage suivant reessaie.
            let texte = match std::str::from_utf8(&brut) {
                Ok(t) => t.trim_start_matches('\u{feff}'),
                Err(e) => return Err(ErreurFilet::filet(format!("le serveur de l'éditeur est injoignable ({}).", e), 0)),
            };
            socle::forme::lire_texte::<T>(texte, "réponse illisible")
                .map_err(|e| ErreurFilet::filet(format!("le serveur de l'éditeur est injoignable ({}).", e.phrase.trim_end_matches('.')), 0))
        }
        Err(Panne::Statut { code, corps }) => Err(ErreurFilet::filet(format!("le serveur de l'éditeur a répondu : {}", refus_dit(code, &corps)), code)),
        Err(Panne::Reseau(raison)) => Err(ErreurFilet::filet(format!("le serveur de l'éditeur est injoignable ({}).", raison.trim_end_matches('.')), 0)),
        Err(Panne::Brute(e)) => match e.classe {
            // Une connexion rompue ou qui ne répond plus : le filet est injoignable.
            Some(Classe::DelaiDepasse | Classe::Reseau) => {
                Err(ErreurFilet::filet(format!("le serveur de l'éditeur est injoignable ({}).", e.phrase.trim_end_matches('.')), 0))
            }
            _ => Err(ErreurFilet::Autre(e)),
        },
    }
}

/// Le serveur et l'identite du poste -- `(numero, cle)` donnee par le serveur.
pub struct Client {
    pub serveur: String,
    pub poste: (String, String),
}

impl Client {
    /// Le client des actions de l'editeur, avec l'identite de ce poste.
    pub fn du_poste(serveur: &str, numero: &str, cle_du_poste: &str) -> Client {
        Client { serveur: serveur.trim_end_matches('/').to_string(), poste: (numero.to_string(), cle_du_poste.to_string()) }
    }

    /// `POST /poste` : l'identite que le serveur donne a ce poste, rendue une seule fois (il n'en garde que l'empreinte).
    pub fn demander_poste(serveur: &str) -> Result<Identite, ErreurFilet> {
        poster(serveur, "poste", &Rien {}, &[], DELAI)
    }

    /// Une action du poste : le numero dans le corps, la cle dans l'en-tete. Un 401
    /// du serveur (poste inconnu, revoque, ou cle fausse : il ne dit pas lequel) se
    /// complete de ce qu'il y a a faire.
    fn post_du_poste<C: Serialize, T: DeserializeOwned>(&self, action: &str, corps: C, delai: u64) -> Result<T, ErreurFilet> {
        let (numero, cle) = &self.poste;
        match poster(&self.serveur, action, &DuPoste { poste: numero, corps }, &[("X-Filet-Cle", cle.as_str())], delai) {
            Err(ErreurFilet::Filet { message, statut: 401 }) => Err(ErreurFilet::filet(format!("{} {}", message, REDEMANDER_IDENTITE), 401)),
            r => r,
        }
    }

    /// `POST /rapport` : un lot d'incidents, tels qu'ils partent (`incidents::PourLEditeur`).
    pub fn rapporter<I: Serialize>(&self, incidents: &[I]) -> Result<Rapport, ErreurFilet> {
        self.post_du_poste("rapport", Incidents { incidents }, DELAI)
    }

    /// Le nombre de paies de l'annee (`nombre_de_paies`) : `{"annee", "paies"}` et le numero du poste, rien
    /// d'autre.
    pub fn compter_paies(&self, annee: i32, paies: usize) -> Result<Paies, ErreurFilet> {
        self.post_du_poste("paies", CompteDesPaies { annee, paies }, DELAI)
    }

    /// Un message au developpeur (`problemes::Corps`) : un texte, des incidents et des captures ; le delai est plus long.
    pub fn signaler<C: Serialize>(&self, corps: &C) -> Result<Retour, ErreurFilet> {
        self.post_du_poste("retour", corps, 60)
    }

    /// `POST /reponses` : les reponses aux messages `numeros`.
    pub fn reponses(&self, numeros: &[String]) -> Result<Reponses, ErreurFilet> {
        self.post_du_poste("reponses", Numeros { numeros }, DELAI)
    }
}
