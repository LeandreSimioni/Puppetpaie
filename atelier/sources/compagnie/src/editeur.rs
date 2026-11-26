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
//! actions du poste portent le numero dans le corps et la cle dans `X-Filet-Cle`. Seul le binaire de l'atelier (le
//! nuage) porte celle du poste de l'editeur, lue dans son environnement (`incidents::identite_de_l_atelier`), que le
//! serveur ne plafonne pas.
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
use socle::garde_reseau::Url;
use socle::mois::Annee;
use envois::transport::Panne;
use ureq::http::StatusCode;

use crate::incidents::{CleDIncident, NumeroDePoste};

/// Ce que le paquet soude (bride pour toujours) peut poster.
pub const ACTIONS_BRIDEES: [&str; 4] = ["poste", "rapport", "retour", "reponses"];

/// Ce que dit le moteur a un 401 du serveur sur une action du poste : l'identite se redemande.
pub const REDEMANDER_IDENTITE: &str = "Le serveur ne reconnait plus ce poste : retirer \"cle\" de config/diagnostic.json pour lui redemander une identite.";

/// Le serveur de l'editeur QUAND LA CONFIGURATION NE DIT RIEN : les
/// incidents, « Un probleme, une idee ? » et leurs reponses : le filet (`garde_reseau::FILET`). Ce qu'une compagnie a
/// pose le remplace toujours : `serveur` de `config/diagnostic.json`, ou `EDITEUR.json` du
/// paquet. Jamais dans les essais : le garde-fou reseau (`garde_reseau`) le refuse ; un test pose son serveur local, ou
/// nomme un faux mandataire local comme seule porte. Dans le navigateur, seul le filet se joint (`garde_reseau::par_l_hote`).
pub const SERVEUR_PAR_DEFAUT: Url = socle::garde_reseau::FILET;

/// Le filet ne doit jamais faire attendre une paie.
pub const DELAI: std::time::Duration = std::time::Duration::from_secs(15);

/// Une erreur du filet, que les appelants rattrapent (le passage suivant reessaie), ou une autre panne, un refus qui
/// remonte.
#[derive(Debug)]
pub enum ErreurFilet {
    /// Le serveur ne repond pas, ou refuse : ce qu'il en dit, et son statut HTTP quand il a repondu.
    Filet { message: String, statut: Option<StatusCode> },
    /// Une autre panne, sous sa classe.
    Autre(Echec),
}

impl ErreurFilet {
    pub(crate) fn filet(message: String, statut: Option<StatusCode>) -> ErreurFilet {
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
    pub poste: NumeroDePoste,
    pub cle: socle::secret::Secret,
}

/// `POST /rapport` : les cles `(empreinte, quand)` recues, et celles que le serveur a ecartees (une ligne mal formee :
/// le poste la marque deposee quand meme). Le serveur rend le `quand` que le poste a envoye, tel quel
/// (`rapporter` de `infra/filet/filet_serveur.py`) : l'instant de la ligne.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rapport {
    #[serde(deserialize_with = "cles_lues")]
    pub recus: Vec<(CleDIncident, socle::paris::Moment)>,
    #[serde(deserialize_with = "cles_lues")]
    pub ecartes: Vec<(CleDIncident, socle::paris::Moment)>,
}

/// Les cles `(empreinte, quand)` d'un rapport, le `quand` ecrit `AAAA-MM-JJTHH:MM:SS` (`socle::paris::ecrit::seconde`).
fn cles_lues<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<(CleDIncident, socle::paris::Moment)>, D::Error> {
    Vec::<(CleDIncident, String)>::deserialize(d)?
        .into_iter()
        .map(|(e, q)| Ok((e, socle::paris::ecrit::seconde::deserialize(serde::de::value::StrDeserializer::<D::Error>::new(&q))?)))
        .collect()
}

/// LE NUMERO D'UN RETOUR : celui que le serveur du filet donne a un message au developpeur, et que l'utilisateur lit
/// (`R-` et six hexadecimaux majuscules, `RE_NUMERO_RETOUR` de `infra/filet/filet_serveur.py`). Ce n'est pas le
/// numero du ticket qui le suit (`ticket_numero`). Il s'ecrit et se lit par son texte ; une autre forme se refuse.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NumeroDeRetour(String);

impl NumeroDeRetour {
    /// Le numero ecrit `texte` ; refus s'il n'en a pas la forme.
    pub fn lire(texte: &str) -> socle::echec::R<NumeroDeRetour> {
        let forme = match texte.strip_prefix("R-") {
            Some(h) => h.len() == 6 && h.bytes().all(|o| o.is_ascii_digit() || (b'A'..=b'F').contains(&o)),
            None => false,
        };
        if !forme {
            return Err(socle::refus!("Numéro de retour {} : il s'écrit « R- » suivi de six chiffres hexadécimaux majuscules.", socle::fichiers::repr(texte)));
        }
        Ok(NumeroDeRetour(texte.to_string()))
    }

    /// Le numero tel qu'ecrit.
    pub fn texte(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NumeroDeRetour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for NumeroDeRetour {
    type Error = Echec;
    fn try_from(s: String) -> socle::echec::R<NumeroDeRetour> {
        NumeroDeRetour::lire(&s)
    }
}

impl From<NumeroDeRetour> for String {
    fn from(n: NumeroDeRetour) -> String {
        n.0
    }
}

/// LE NUMERO DU TICKET qui suit un retour chez l'editeur, que le serveur du filet rend avec son adresse (`ticket_numero`) :
/// un entier, ecrit et lu tel quel, dit « ticket n° 12 ».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NumeroDeTicket(u64);

impl std::fmt::Display for NumeroDeTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// `POST /retour` : le numero que l'utilisateur lit, et l'adresse ou l'editeur le suit si elle est deja ouverte (le
/// serveur ecrit `""` sinon : aucune ; son numero absent).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retour {
    pub numero: NumeroDeRetour,
    #[serde(deserialize_with = "socle::forme::absent_si_vide")]
    pub ticket: Option<Url>,
    pub ticket_numero: Option<NumeroDeTicket>,
}

/// `POST /reponses` : les reponses aux retours de ce poste.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reponses {
    pub retours: Vec<ReponseDuServeur>,
}

/// La reponse a un retour : vide tant que l'editeur n'a pas repondu (`repondu_le` : `""` tant qu'il n'a pas repondu,
/// sinon la minute UTC de la reponse ; `socle::paris::ecrit::minute_utc::vide_ou`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReponseDuServeur {
    pub numero: NumeroDeRetour,
    pub reponse: String,
    #[serde(deserialize_with = "socle::paris::ecrit::minute_utc::vide_ou")]
    pub repondu_le: Option<socle::paris::Moment>,
}

/// `POST /paies` : ce que le serveur a recu.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paies {
    pub recu: bool,
    pub annee: Annee,
    pub paies: usize,
}

/// `POST /vie` : le serveur a note que ce poste s'est servi du logiciel ce jour-la (`signal_de_vie`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vie {
    pub ok: bool,
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
    poste: &'a NumeroDePoste,
    #[serde(flatten)]
    corps: C,
}

#[derive(Serialize)]
struct Incidents<'a, I: Serialize> {
    incidents: &'a [I],
}

#[derive(Serialize)]
struct CompteDesPaies {
    annee: Annee,
    paies: usize,
}

#[derive(Serialize)]
struct Numeros<'a> {
    numeros: &'a [NumeroDeRetour],
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
fn statut_dit(code: StatusCode) -> String {
    match PHRASES.iter().find(|(statut, _)| *statut == code) {
        Some((_, p)) => format!("HTTP Error {}: {}", code.as_str(), p),
        None => format!("HTTP Error {}", code.as_str()),
    }
}

/// La phrase HTTP de chaque statut qui en a une.
const PHRASES: [(StatusCode, &str); 62] = [
    (StatusCode::CONTINUE, "Continue"),
    (StatusCode::SWITCHING_PROTOCOLS, "Switching Protocols"),
    (StatusCode::PROCESSING, "Processing"),
    (StatusCode::EARLY_HINTS, "Early Hints"),
    (StatusCode::OK, "OK"),
    (StatusCode::CREATED, "Created"),
    (StatusCode::ACCEPTED, "Accepted"),
    (StatusCode::NON_AUTHORITATIVE_INFORMATION, "Non-Authoritative Information"),
    (StatusCode::NO_CONTENT, "No Content"),
    (StatusCode::RESET_CONTENT, "Reset Content"),
    (StatusCode::PARTIAL_CONTENT, "Partial Content"),
    (StatusCode::MULTI_STATUS, "Multi-Status"),
    (StatusCode::ALREADY_REPORTED, "Already Reported"),
    (StatusCode::IM_USED, "IM Used"),
    (StatusCode::MULTIPLE_CHOICES, "Multiple Choices"),
    (StatusCode::MOVED_PERMANENTLY, "Moved Permanently"),
    (StatusCode::FOUND, "Found"),
    (StatusCode::SEE_OTHER, "See Other"),
    (StatusCode::NOT_MODIFIED, "Not Modified"),
    (StatusCode::USE_PROXY, "Use Proxy"),
    (StatusCode::TEMPORARY_REDIRECT, "Temporary Redirect"),
    (StatusCode::PERMANENT_REDIRECT, "Permanent Redirect"),
    (StatusCode::BAD_REQUEST, "Bad Request"),
    (StatusCode::UNAUTHORIZED, "Unauthorized"),
    (StatusCode::PAYMENT_REQUIRED, "Payment Required"),
    (StatusCode::FORBIDDEN, "Forbidden"),
    (StatusCode::NOT_FOUND, "Not Found"),
    (StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed"),
    (StatusCode::NOT_ACCEPTABLE, "Not Acceptable"),
    (StatusCode::PROXY_AUTHENTICATION_REQUIRED, "Proxy Authentication Required"),
    (StatusCode::REQUEST_TIMEOUT, "Request Timeout"),
    (StatusCode::CONFLICT, "Conflict"),
    (StatusCode::GONE, "Gone"),
    (StatusCode::LENGTH_REQUIRED, "Length Required"),
    (StatusCode::PRECONDITION_FAILED, "Precondition Failed"),
    (StatusCode::PAYLOAD_TOO_LARGE, "Request Entity Too Large"),
    (StatusCode::URI_TOO_LONG, "Request-URI Too Long"),
    (StatusCode::UNSUPPORTED_MEDIA_TYPE, "Unsupported Media Type"),
    (StatusCode::RANGE_NOT_SATISFIABLE, "Requested Range Not Satisfiable"),
    (StatusCode::EXPECTATION_FAILED, "Expectation Failed"),
    (StatusCode::IM_A_TEAPOT, "I'm a Teapot"),
    (StatusCode::MISDIRECTED_REQUEST, "Misdirected Request"),
    (StatusCode::UNPROCESSABLE_ENTITY, "Unprocessable Entity"),
    (StatusCode::LOCKED, "Locked"),
    (StatusCode::FAILED_DEPENDENCY, "Failed Dependency"),
    (StatusCode::TOO_EARLY, "Too Early"),
    (StatusCode::UPGRADE_REQUIRED, "Upgrade Required"),
    (StatusCode::PRECONDITION_REQUIRED, "Precondition Required"),
    (StatusCode::TOO_MANY_REQUESTS, "Too Many Requests"),
    (StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE, "Request Header Fields Too Large"),
    (StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS, "Unavailable For Legal Reasons"),
    (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"),
    (StatusCode::NOT_IMPLEMENTED, "Not Implemented"),
    (StatusCode::BAD_GATEWAY, "Bad Gateway"),
    (StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable"),
    (StatusCode::GATEWAY_TIMEOUT, "Gateway Timeout"),
    (StatusCode::HTTP_VERSION_NOT_SUPPORTED, "HTTP Version Not Supported"),
    (StatusCode::VARIANT_ALSO_NEGOTIATES, "Variant Also Negotiates"),
    (StatusCode::INSUFFICIENT_STORAGE, "Insufficient Storage"),
    (StatusCode::LOOP_DETECTED, "Loop Detected"),
    (StatusCode::NOT_EXTENDED, "Not Extended"),
    (StatusCode::NETWORK_AUTHENTICATION_REQUIRED, "Network Authentication Required"),
];

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
fn refus_dit(code: StatusCode, corps: &Result<Vec<u8>, Box<Panne>>) -> String {
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
pub fn poster<C: Serialize + ?Sized, T: DeserializeOwned>(serveur: &Url, action: &str, corps: &C, entetes: &[(&str, &str)], delai: std::time::Duration) -> Result<T, ErreurFilet> {
    if crate::bridage::paquet_soude() && !ACTIONS_BRIDEES.contains(&action) {
        return Err(ErreurFilet::Autre(socle::echec::erreur(Classe::EditionBridee, edition_bridee(action))));
    }
    let donnees = socle::json_ascii::dumps(corps, None, false, false);
    let url = Url::lire(&format!("{}/{}", serveur.texte().trim_end_matches('/'), action)).map_err(ErreurFilet::Autre)?;
    // Aucun codage demandé.
    let mut en_tetes: Vec<(&str, &str)> = vec![("User-Agent", "PuppetPaie-filet"), ("Accept-Encoding", "identity")];
    en_tetes.extend_from_slice(entetes);
    match post_au_filet(&url, &en_tetes, donnees.as_bytes(), delai) {
        Ok(brut) => {
            // Une reponse qui ne se lit pas se range avec les pannes de reseau : le passage suivant reessaie.
            let texte = match std::str::from_utf8(&brut) {
                Ok(t) => t.trim_start_matches('\u{feff}'),
                Err(e) => return Err(ErreurFilet::filet(format!("le serveur de l'éditeur est injoignable ({}).", e), None)),
            };
            socle::forme::lire_texte::<T>(texte, "réponse illisible")
                .map_err(|e| ErreurFilet::filet(format!("le serveur de l'éditeur est injoignable ({}).", e.phrase.trim_end_matches('.')), None))
        }
        Err(Panne::Statut { code, corps }) => Err(ErreurFilet::filet(format!("le serveur de l'éditeur a répondu : {}", refus_dit(code, &corps)), Some(code))),
        Err(Panne::Reseau(raison)) => Err(ErreurFilet::filet(format!("le serveur de l'éditeur est injoignable ({}).", raison.trim_end_matches('.')), None)),
        Err(Panne::Brute(e)) => match e.classe {
            // Une connexion rompue ou qui ne répond plus : le filet est injoignable.
            Some(Classe::DelaiDepasse | Classe::Reseau) => {
                Err(ErreurFilet::filet(format!("le serveur de l'éditeur est injoignable ({}).", e.phrase.trim_end_matches('.')), None))
            }
            _ => Err(ErreurFilet::Autre(e)),
        },
    }
}

/// LE POST JSON AU SERVEUR DE L'EDITEUR, tel que le systeme le fait : par le transport du moteur (`transport::envoyer`) ;
/// rend le corps de la reponse, ou sa panne.
#[cfg(not(target_os = "wasi"))]
fn post_au_filet(url: &Url, entetes: &[(&str, &str)], donnees: &[u8], delai: std::time::Duration) -> Result<Vec<u8>, Panne> {
    let r = envois::transport::Requete {
        donnees: Some(donnees),
        content_type: Some("application/json"),
        post: true,
        entetes,
        sans_gzip: true,
        ..envois::transport::Requete::get(url, delai)
    };
    envois::transport::envoyer(&r).map(|(brut, _)| brut)
}

/// DANS LE NAVIGATEUR, le module n'a aucune prise : le POST passe par l'hote JavaScript, la seule porte vers dehors, et
/// vers le seul filet (`garde_reseau::par_l_hote`). Ce qu'il rend prend la forme du transport : un code hors 2xx est un
/// `Statut` avec son corps ; une panne de l'hote ou un refus de la porte, un `Reseau` (l'incident reste en attente).
#[cfg(target_os = "wasi")]
fn post_au_filet(url: &Url, entetes: &[(&str, &str)], donnees: &[u8], delai: std::time::Duration) -> Result<Vec<u8>, Panne> {
    let mut avec_le_type = entetes.to_vec();
    avec_le_type.push(("Content-Type", "application/json"));
    let r = socle::garde_reseau::par_l_hote(url, &avec_le_type, donnees, delai).map_err(|e| Panne::Reseau(e.phrase))?;
    let code = StatusCode::from_u16(r.code).map_err(|e| Panne::Reseau(format!("code HTTP {} illisible ({})", r.code, e)))?;
    match code.is_success() {
        true => Ok(r.corps),
        false => Err(Panne::Statut { code, corps: Ok(r.corps) }),
    }
}

/// D'OU VIENT L'IDENTITE DU POSTE que porte un client : ce qu'il y a a faire quand le serveur ne la reconnait plus en
/// depend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrigineDeLIdentite {
    /// `config/diagnostic.json` de la compagnie qui la porte, ou le serveur l'a donnee (`POST /poste`).
    Diagnostic,
    /// L'environnement du binaire de l'atelier : l'identite du poste de l'editeur (`incidents::identite_de_l_atelier`).
    #[cfg(feature = "atelier")]
    Atelier,
}

impl OrigineDeLIdentite {
    /// Ce qu'il y a a faire quand le serveur ne reconnait plus l'identite (un 401).
    fn a_refaire(self) -> String {
        match self {
            OrigineDeLIdentite::Diagnostic => REDEMANDER_IDENTITE.to_string(),
            #[cfg(feature = "atelier")]
            OrigineDeLIdentite::Atelier => crate::incidents::identite_de_l_atelier_refusee(),
        }
    }
}

/// Le serveur et l'identite du poste -- `(numero, cle)` donnee par le serveur -- et d'ou elle vient.
pub struct Client {
    /// Le serveur, tel que le reglage le dit ; `poster` ote sa barre finale.
    pub serveur: Url,
    pub poste: (NumeroDePoste, socle::secret::Secret),
    origine: OrigineDeLIdentite,
}

impl Client {
    /// Le client des actions de l'editeur, avec l'identite de ce poste et d'ou elle vient.
    pub fn du_poste(serveur: &Url, numero: &NumeroDePoste, cle_du_poste: &socle::secret::Secret, origine: OrigineDeLIdentite) -> Client {
        Client { serveur: serveur.clone(), poste: (numero.clone(), cle_du_poste.clone()), origine }
    }

    /// `POST /poste` : l'identite que le serveur donne a ce poste, rendue une seule fois (il n'en garde que l'empreinte).
    pub fn demander_poste(serveur: &Url) -> Result<Identite, ErreurFilet> {
        poster(serveur, "poste", &Rien {}, &[], DELAI)
    }

    /// Une action du poste : le numero dans le corps, la cle dans l'en-tete. Un 401
    /// du serveur (poste inconnu, revoque, ou cle fausse : il ne dit pas lequel) se
    /// complete de ce qu'il y a a faire.
    fn post_du_poste<C: Serialize, T: DeserializeOwned>(&self, action: &str, corps: C, delai: std::time::Duration) -> Result<T, ErreurFilet> {
        let (numero, cle) = &self.poste;
        match poster(&self.serveur, action, &DuPoste { poste: numero, corps }, &[("X-Filet-Cle", cle.devoiler())], delai) {
            Err(ErreurFilet::Filet { message, statut: Some(StatusCode::UNAUTHORIZED) }) => {
                Err(ErreurFilet::filet(format!("{} {}", message, self.origine.a_refaire()), Some(StatusCode::UNAUTHORIZED)))
            }
            r => r,
        }
    }

    /// `POST /rapport` : un lot d'incidents, tels qu'ils partent (`incidents::PourLEditeur`).
    pub fn rapporter<I: Serialize>(&self, incidents: &[I]) -> Result<Rapport, ErreurFilet> {
        self.post_du_poste("rapport", Incidents { incidents }, DELAI)
    }

    /// Le nombre de paies de l'annee (`nombre_de_paies`) : `{"annee", "paies"}` et le numero du poste, rien
    /// d'autre.
    pub fn compter_paies(&self, annee: Annee, paies: usize) -> Result<Paies, ErreurFilet> {
        self.post_du_poste("paies", CompteDesPaies { annee, paies }, DELAI)
    }

    /// Le signal de vie du poste (`signal_de_vie`) : le numero du poste, rien d'autre.
    pub fn signaler_la_vie(&self) -> Result<Vie, ErreurFilet> {
        self.post_du_poste("vie", Rien {}, DELAI)
    }

    /// Un message au developpeur (`problemes::Corps`) : un texte, des incidents et des captures ; le delai est plus long.
    pub fn signaler<C: Serialize>(&self, corps: &C) -> Result<Retour, ErreurFilet> {
        self.post_du_poste("retour", corps, std::time::Duration::from_secs(60))
    }

    /// `POST /reponses` : les reponses aux messages `numeros`.
    pub fn reponses(&self, numeros: &[NumeroDeRetour]) -> Result<Reponses, ErreurFilet> {
        self.post_du_poste("reponses", Numeros { numeros }, DELAI)
    }
}
