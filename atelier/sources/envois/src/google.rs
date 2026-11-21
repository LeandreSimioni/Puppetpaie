//! Ce que le Drive et l'autorisation Google partagent : les adresses et les permissions de Google, l'application qui
//! s'autorise, l'encodage d'un formulaire, les reponses de Google (le jeton, l'erreur) lues en leur forme, la phrase
//! d'une reponse d'erreur et le jeton d'acces obtenu pour un jeton de rafraichissement. Rien ici ne lit le poste : le
//! jeton, les identifiants et les adresses viennent de l'appelant.

use serde::Deserialize;

use socle::echec::{Classe, Echec, R};
use socle::garde_reseau::Url;

use crate::drive::ErreurGoogle;
use crate::transport::{envoyer, Panne, Requete};

/// L'adresse des jetons de Google.
pub const JETONS: Url = Url::du_moteur("https://oauth2.googleapis.com/token");

/// L'adresse des jetons d'un Google qui repond a `base` (un serveur local), au meme chemin que `JETONS`.
pub fn jetons_de_la_base(base: &Url) -> Url {
    base.joindre("/token")
}

/// Les permissions que le logiciel demande : les fichiers qu'il cree dans le Drive, et l'adresse du compte.
pub const PERMISSIONS: [&str; 2] =
    ["https://www.googleapis.com/auth/drive.file", "https://www.googleapis.com/auth/userinfo.email"];

/// Le delai d'un appel a Google.
pub const DELAI: std::time::Duration = std::time::Duration::from_secs(60);

/// L'APPLICATION GOOGLE QUI DEMANDE L'AUTORISATION (celle du logiciel, ou celle que la compagnie a posee) : ses
/// identifiants, que l'appelant a lus.
#[derive(Debug, Clone)]
pub struct Application {
    pub client_id: String,
    pub client_secret: socle::secret::Secret,
}

/// CE QUE GOOGLE REND A UNE DEMANDE DE JETON (`/token`) : le jeton d'acces, et pour un premier code d'autorisation le
/// jeton de rafraichissement et les permissions accordees. Une reponse de Google : les autres cles qu'il ecrit sont
/// ignorees a la porte (`socle::forme::lire_reponse_tiers`).
#[derive(Debug, Deserialize)]
pub struct ReponseJeton {
    pub access_token: Option<socle::secret::Secret>,
    pub refresh_token: Option<socle::secret::Secret>,
    pub scope: Option<String>,
}
impl socle::forme::ReponseDUnTiers for ReponseJeton {}

/// Le texte d'une reponse de Google lu en sa forme `T`, par la porte des tiers ; une reponse qui ne se lit pas rend
/// `phrase`.
pub fn lue<T: socle::forme::ReponseDUnTiers>(brut: Vec<u8>, phrase: &str) -> R<T> {
    let texte = String::from_utf8(brut).map_err(|e| socle::echec::refus_avec(phrase, socle::echec::Lieu { detail: Some(e.to_string()), ..socle::echec::Lieu::default() }))?;
    socle::forme::lire_reponse_tiers(&texte, phrase)
}

/// L'ERREUR QUE GOOGLE REND : celle de l'autorisation (`{"error": "invalid_grant", "error_description": …}`) ou celle d'une
/// API (`{"error": {"code", "message", "status", …}}`). Une reponse de Google : les autres cles sont ignorees a la porte.
#[derive(Debug, Deserialize)]
struct ReponseDErreur {
    error: ErreurDite,
    error_description: Option<String>,
}
impl socle::forme::ReponseDUnTiers for ReponseDErreur {}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ErreurDite {
    Code(String),
    Api(ErreurDApi),
}

/// L'erreur d'une API de Google, dans `ReponseDErreur` : les autres cles sont ignorees a la porte.
#[derive(Debug, Deserialize)]
struct ErreurDApi {
    message: Option<String>,
    status: Option<String>,
}
impl socle::forme::ReponseDUnTiers for ErreurDApi {}

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

/// Ce que Google dit d'une reponse d'erreur qui demande un geste de la compagnie (l'autorisation perdue, une permission
/// qui manque), en sa phrase ; `None` pour toute autre erreur de Google, que `panne_dite` dit. Un corps qui n'est pas une
/// erreur de Google (la page d'un relais, un corps vide) : le tiers a mal repondu, son refus cite le statut et le corps.
fn traduire(statut: ureq::http::StatusCode, corps: &str) -> R<Option<&'static str>> {
    let (code_g, _) = socle::forme::lire_reponse_tiers::<ReponseDErreur>(corps, &format!("HTTP {} : {}", statut.as_str(), corps))?.code_et_detail();
    Ok(traduite(statut, corps, code_g.as_deref()))
}

/// La phrase d'une erreur de Google de code `code_g` qui demande un geste de la compagnie.
fn traduite(statut: ureq::http::StatusCode, corps: &str, code_g: Option<&str>) -> Option<&'static str> {
    if code_g == Some("invalid_grant") {
        return Some(
            "Autorisation Google perdue ou expirée : le plus souvent, l'écran de consentement est resté en mode « Test », ou Google a révoqué le jeton au bout de 7 jours. Le passer « En production » dans la console, puis lier de nouveau le Drive (Configuration › Mots de passe).",
        );
    }
    if statut == ureq::http::StatusCode::FORBIDDEN && corps.to_lowercase().contains("insufficient") {
        return Some("Permission refusée : l'autorisation en cours ne couvre pas cette action. Lier de nouveau le Drive (Configuration › Mots de passe) pour redemander les permissions.");
    }
    None
}

/// UNE PANNE DU TRANSPORT VERS GOOGLE (`url` : l'adresse appelee), telle que la compagnie la lit : une autorisation perdue
/// ou une permission qui manque, dites par `traduire` ; tout le reste par `transport::panne_dite` (« Google n'a pas
/// répondu comme attendu », « Google injoignable », « Google n'a pas répondu à temps »). Le detail technique (le statut,
/// le message de Google ou le corps, ce que le systeme a dit) va a part ; `statut` porte le code d'une reponse
/// d'erreur, de quoi decider d'une nouvelle tentative (`drive::passagere`). Un statut ou un reseau injoignable est de
/// classe `Google`, une panne brute garde la sienne (un delai depasse reste `DelaiDepasse`).
#[track_caller]
pub fn panne_de_google(url: &Url, p: Panne) -> ErreurGoogle {
    let p = match p {
        Panne::Statut { code, corps: Ok(corps) } => {
            // Le transport demande `gzip` : une reponse d'erreur peut arriver compressee.
            let corps = if corps.starts_with(&[0x1f, 0x8b]) {
                match crate::transport::gunzip(&corps) {
                    Ok(c) => c,
                    Err(illisible) => return ErreurGoogle { statut: Some(code), ..ErreurGoogle::from(crate::transport::panne_dite("Google", Classe::Google, url, illisible).echec) },
                }
            } else {
                corps
            };
            match traduire(code, &String::from_utf8_lossy(&corps)) {
                Ok(Some(phrase)) => {
                    let detail = format!("HTTP {} : {}", code.as_str(), String::from_utf8_lossy(&corps));
                    socle::echec::noter_echec(Some(Classe::Google), &detail);
                    let a_part = socle::echec::Lieu { detail: Some(detail), ..socle::echec::Lieu::default() };
                    return ErreurGoogle { echec: Echec { classe: Some(Classe::Google), phrase: phrase.to_string(), a_part: Some(a_part.into()) }, statut: Some(code) };
                }
                Ok(None) => Panne::Statut { code, corps: Ok(corps) },
                // Pas une erreur de Google : une panne brute, dans la classe de son refus, le statut garde.
                Err(pas_de_google) => {
                    return ErreurGoogle { statut: Some(code), ..ErreurGoogle::from(crate::transport::panne_dite("Google", Classe::Google, url, Panne::Brute(pas_de_google)).echec) };
                }
            }
        }
        // Une reponse d'erreur dont le corps ne se lit pas : la panne de sa lecture, dans sa classe, et le statut.
        Panne::Statut { code, corps: Err(illisible) } => {
            return ErreurGoogle { statut: Some(code), ..ErreurGoogle::from(crate::transport::panne_dite("Google", Classe::Google, url, *illisible).echec) };
        }
        autre => autre,
    };
    let e = crate::transport::panne_dite("Google", Classe::Google, url, p);
    ErreurGoogle { echec: e.echec, statut: e.code }
}

/// Le jeton d'acces que Google rend pour un jeton de rafraichissement : `Ok(Ok(jeton))`, `Ok(Err(..))` pour un refus
/// de Google, traduit, `Err` pour une panne brute. `adresse` : `JETONS`, ou un serveur local.
pub fn jeton_par_rafraichissement(adresse: &Url, application: &Application, rafraichissement: &socle::secret::Secret, delai: std::time::Duration) -> R<Result<socle::secret::Secret, ErreurGoogle>> {
    let corps = encoder(&[
        ("client_id", application.client_id.clone()),
        ("client_secret", application.client_secret.devoiler().to_string()),
        ("refresh_token", rafraichissement.devoiler().to_string()),
        ("grant_type", "refresh_token".into()),
    ]);
    let requete = Requete {
        donnees: Some(corps.as_bytes()),
        content_type: Some("application/x-www-form-urlencoded"),
        post: true,
        ..Requete::get(adresse, delai)
    };
    match envoyer(&requete) {
        Ok((brut, _)) => {
            // Une reponse 200 sans jeton d'acces, ou illisible, est un refus de Google comme un autre : une phrase.
            let reponse: ReponseJeton = match lue(brut, "Google a rendu une reponse illisible a la demande de jeton. Reessayer plus tard.") {
                Ok(r) => r,
                Err(e) => return Ok(Err(ErreurGoogle::from(e.reclasser(Classe::Google)))),
            };
            match reponse.access_token.filter(|t| !t.devoiler().is_empty()) {
                Some(acces) => Ok(Ok(acces)),
                None => Ok(Err(ErreurGoogle::google("Google n'a pas rendu de jeton d'acces. Reessayer plus tard ; si cela dure, lier de nouveau le compte."))),
            }
        }
        // Un refus de Google (un statut, le reseau injoignable) se rend ; une panne brute refuse.
        Err(panne) => {
            let e = panne_de_google(adresse, panne);
            match e.echec.classe {
                Some(Classe::Google) => Ok(Err(e)),
                _ => Err(e.echec),
            }
        }
    }
}
