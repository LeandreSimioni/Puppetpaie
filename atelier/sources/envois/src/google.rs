//! Ce que le Drive et l'autorisation Google partagent : les adresses et les permissions de Google, l'application qui
//! s'autorise, l'encodage d'un formulaire, les reponses de Google (le jeton, l'erreur) lues en leur forme, la phrase
//! d'une reponse d'erreur et le jeton d'acces obtenu pour un jeton de rafraichissement. Rien ici ne lit le poste : le
//! jeton, les identifiants et les adresses viennent de l'appelant.

use serde::de::IgnoredAny;
use serde::Deserialize;

use socle::echec::{erreur, Classe, Echec};

use crate::transport::{envoyer, Panne, Requete};

type R<T> = Result<T, Echec>;

/// L'adresse des jetons de Google.
pub const JETONS: &str = "https://oauth2.googleapis.com/token";

/// Les permissions que le logiciel demande : les fichiers qu'il cree dans le Drive, et l'adresse du compte.
pub const PERMISSIONS: [&str; 2] =
    ["https://www.googleapis.com/auth/drive.file", "https://www.googleapis.com/auth/userinfo.email"];

/// Le delai d'un appel a Google, en secondes.
pub const DELAI: u64 = 60;

/// L'APPLICATION GOOGLE QUI DEMANDE L'AUTORISATION (celle du logiciel, ou celle que la compagnie a posee) : ses
/// identifiants, que l'appelant a lus.
#[derive(Debug, Clone)]
pub struct Application {
    pub client_id: String,
    pub client_secret: String,
}

/// CE QUE GOOGLE REND A UNE DEMANDE DE JETON (`/token`) : le jeton d'acces, et pour un premier code d'autorisation le
/// jeton de rafraichissement et les permissions accordees. Les autres cles que Google ecrit sont declarees, sans servir.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReponseJeton {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    #[serde(rename = "expires_in")]
    _expire_dans: Option<IgnoredAny>,
    #[serde(rename = "token_type")]
    _sorte: Option<IgnoredAny>,
    #[serde(rename = "id_token")]
    _identite: Option<IgnoredAny>,
    #[serde(rename = "refresh_token_expires_in")]
    _rafraichissement_expire_dans: Option<IgnoredAny>,
}

/// Le texte d'une reponse de Google lu en sa forme `T` ; une reponse qui ne se lit pas rend `phrase`.
pub fn lue<T: serde::de::DeserializeOwned>(brut: Vec<u8>, phrase: &str) -> Result<T, String> {
    let texte = String::from_utf8(brut).map_err(|e| socle::echec::avec(phrase, socle::echec::APart { detail: Some(e.to_string()), ..socle::echec::APart::default() }))?;
    socle::forme::lire_texte(&texte, phrase)
}

/// L'ERREUR QUE GOOGLE REND : celle de l'autorisation (`{"error": "invalid_grant", "error_description": …}`) ou celle d'une
/// API (`{"error": {"code", "message", "status", …}}`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReponseDErreur {
    error: ErreurDite,
    error_description: Option<String>,
    #[serde(rename = "error_uri")]
    _adresse: Option<IgnoredAny>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ErreurDite {
    Code(String),
    Api(ErreurDApi),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ErreurDApi {
    message: Option<String>,
    status: Option<String>,
    #[serde(rename = "code")]
    _code: Option<IgnoredAny>,
    #[serde(rename = "errors")]
    _erreurs: Option<IgnoredAny>,
    #[serde(rename = "details")]
    _details: Option<IgnoredAny>,
}

impl ReponseDErreur {
    /// Le code de Google (`invalid_grant`, `PERMISSION_DENIED`…) et sa phrase, quand il la dit.
    fn code_et_detail(self) -> (Option<String>, Option<String>) {
        let non_vide = |t: Option<String>| t.filter(|x| !x.is_empty());
        match self.error {
            ErreurDite::Code(code) => (Some(code), non_vide(self.error_description)),
            ErreurDite::Api(e) => (e.status, non_vide(e.message)),
        }
    }
}

/// Un formulaire `application/x-www-form-urlencoded` : cles et valeurs encodees, l'espace en `+`.
pub fn encoder(champs: &[(&str, String)]) -> String {
    let q = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => (b as char).to_string(),
                b' ' => "+".into(),
                _ => format!("%{:02X}", b),
            })
            .collect()
    };
    champs.iter().map(|(k, v)| format!("{}={}", q(k), q(v))).collect::<Vec<_>>().join("&")
}

/// Le message lisible d'une reponse d'erreur de Google. Un corps qui n'est pas une erreur de Google (la page d'un relais,
/// un corps vide) se cite tel quel.
pub fn traduire(code: u16, corps: &str) -> String {
    let (code_g, detail) = match socle::forme::lire_texte::<ReponseDErreur>(corps, "Google a rendu une erreur.") {
        Ok(e) => e.code_et_detail(),
        Err(pas_de_google) => {
            drop(pas_de_google);
            (None, None)
        }
    };
    if code_g.as_deref() == Some("invalid_grant") || corps.contains("invalid_grant") {
        return "Autorisation Google perdue ou expiree.\n\
                \x20 Cause la plus frequente : l'ecran de consentement est reste\n\
                \x20 en mode « Test », ou Google revoque le jeton au bout de 7\n\
                \x20 jours. Le passer « En production » dans la console, puis\n\
                \x20 relier le Drive (Configuration > Mots de passe)."
            .into();
    }
    if code == 403 && corps.to_lowercase().contains("insufficient") {
        return "Permission refusee : l'autorisation en cours ne couvre pas\n\
                \x20 cette action. Relier le Drive (Configuration > Mots de\n\
                \x20 passe) pour\n\
                \x20 redemander les permissions."
            .into();
    }
    match detail {
        Some(d) => format!("HTTP {} : {}", code, d),
        None => format!("HTTP {} : {}", code, corps),
    }
}

/// Une panne du transport : `Ok(phrase)` pour un statut de Google (traduit) ou un reseau injoignable,
/// `Err((classe, message))` pour une panne brute (delai depasse, connexion fermee, corps illisible...).
pub fn phrase_de_panne(p: Panne) -> Result<String, (Classe, String)> {
    match p {
        Panne::Statut { code, corps } => {
            let corps = corps.map_err(|p| brute(*p))?;
            // Le transport demande `gzip` : une reponse d'erreur peut arriver compressee.
            let corps = if corps.starts_with(&[0x1f, 0x8b]) { crate::transport::gunzip(&corps).map_err(brute)? } else { corps };
            Ok(traduire(code, &String::from_utf8_lossy(&corps)))
        }
        Panne::Reseau(raison) => Ok(format!("Reseau injoignable : {}", raison)),
        autre => Err(brute(autre)),
    }
}

/// Une panne brute (`Panne::Brute`) : sa classe et son message ; une autre se dit par sa phrase.
fn brute(p: Panne) -> (Classe, String) {
    match p {
        Panne::Brute { classe, message, .. } => (classe, message),
        Panne::Reseau(raison) => (Classe::Reseau, format!("Reseau injoignable : {}", raison)),
        Panne::Statut { code, .. } => (Classe::Reseau, format!("HTTP {}", code)),
    }
}

/// Le jeton d'acces que Google rend pour un jeton de rafraichissement : `Ok(Ok(jeton))`, `Ok(Err(phrase))` pour un refus
/// de Google, `Err` pour une panne brute. `adresse` : `JETONS`, ou un serveur local.
pub fn jeton_par_rafraichissement(adresse: &str, application: &Application, rafraichissement: &str, delai: u64) -> R<Result<String, String>> {
    let corps = encoder(&[
        ("client_id", application.client_id.clone()),
        ("client_secret", application.client_secret.clone()),
        ("refresh_token", rafraichissement.to_string()),
        ("grant_type", "refresh_token".into()),
    ]);
    let requete = Requete {
        url: adresse,
        jeton: None,
        donnees: Some(corps.as_bytes()),
        content_type: Some("application/x-www-form-urlencoded"),
        corps_gzip: false,
        post: true,
        delai,
        ..Default::default()
    };
    match envoyer(&requete) {
        Ok((brut, _)) => {
            // Une reponse 200 sans jeton d'acces, ou illisible, est un refus de Google comme un autre : une phrase.
            let reponse: ReponseJeton = match lue(brut, "Google a rendu une reponse illisible a la demande de jeton. Reessayer plus tard.") {
                Ok(r) => r,
                Err(phrase) => return Ok(Err(phrase)),
            };
            match reponse.access_token.filter(|t| !t.is_empty()) {
                Some(acces) => Ok(Ok(acces)),
                None => Ok(Err("Google n'a pas rendu de jeton d'acces. Reessayer plus tard ; si cela dure, lier de nouveau le compte.".into())),
            }
        }
        Err(panne) => match phrase_de_panne(panne) {
            Ok(phrase) => Ok(Err(phrase)),
            Err((classe, message)) => Err(erreur(classe, message)),
        },
    }
}
