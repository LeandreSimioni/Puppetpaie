//! L'autorisation du compte Google, SANS navigateur : le flux « application de bureau » avec PKCE.
//!
//! Le moteur ecoute sur `127.0.0.1`, port tire au hasard, affiche l'URL de la fenetre de consentement et attend que Google y renvoie
//! le code. **Il n'ouvre aucun navigateur** : la personne ouvre l'URL elle-meme.
//!
//! - Le verificateur PKCE (64 octets aleatoires du systeme, base64 URL), le `state` (24 octets) ne sont ecrits nulle part et ne
//!   sortent pas : seuls le defi (SHA-256 du verificateur) et le `state` figurent dans l'URL.
//! - Le jeton de rafraichissement va du corps de la reponse de Google a l'appelant (`JetonGoogle`), qui le scelle (`moteur::routes::coffre::sceller_le_jeton`,
//!   cle du poste) sans passer par une sortie, un journal ni un message d'erreur ; le secret de l'application ne sert que dans
//!   le corps du POST. Une phrase d'erreur ne cite jamais un jeton ni un code.
//! - L'attente est un fil + `mpsc::recv_timeout` : jamais de `sleep`.
//!
//! Le texte de l'erreur renvoyee par Google est echappe dans la page de retour (sinon : HTML injecte, au prix d'un `state` valide) ;
//! une reponse sans `access_token`, un corps illisible, une adresse de compte vide, un code absent donnent une phrase et rien n'est
//! ecrit. Les pannes de reseau (delai depasse) sont une phrase.
//! L'abandon par Ctrl-C arrete le processus : rien n'est ecrit.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use serde::de::IgnoredAny;
use sha2::{Digest, Sha256};

use socle::echec::Echec;
use crate::google::{encoder, lue, phrase_de_panne, Application, ReponseJeton, DELAI, JETONS, PERMISSIONS};
use crate::transport::{envoyer, Requete};

/// CE QUE GOOGLE DIT DU COMPTE (`userinfo`) : son adresse. Les autres cles que Google ecrit sont declarees, sans servir.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReponseCompte {
    email: Option<String>,
    #[serde(rename = "sub")]
    _sujet: Option<IgnoredAny>,
    #[serde(rename = "picture")]
    _image: Option<IgnoredAny>,
    #[serde(rename = "email_verified")]
    _verifie: Option<IgnoredAny>,
    #[serde(rename = "name")]
    _nom: Option<IgnoredAny>,
    #[serde(rename = "given_name")]
    _prenom: Option<IgnoredAny>,
    #[serde(rename = "family_name")]
    _nom_de_famille: Option<IgnoredAny>,
    #[serde(rename = "hd")]
    _domaine: Option<IgnoredAny>,
    #[serde(rename = "locale")]
    _langue: Option<IgnoredAny>,
    #[serde(rename = "profile")]
    _profil: Option<IgnoredAny>,
}

/// `google.AUTORISATION`, `google.USERINFO`, `google.DELAI_AUTORISATION`.
const AUTORISATION: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const USERINFO: &str = "https://www.googleapis.com/oauth2/v3/userinfo";
pub const DELAI_AUTORISATION: u64 = 300;

/// `google.PAGE_RETOUR`.
fn page_retour(titre: &str, couleur: &str, texte: &str) -> String {
    format!(
        "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\">\n<title>{t}</title>\n<style>body{{font:16px/1.6 system-ui,\"Segoe UI\",sans-serif;background:#14141a;\ncolor:#e8e8ec;display:grid;place-items:center;height:100vh;margin:0;text-align:center}}\nh1{{font-size:20px;color:{c};margin:0 0 8px}}p{{color:#9a9aa8;margin:0}}</style>\n<div><h1>{t}</h1><p>{x}</p></div></html>",
        t = titre,
        c = couleur,
        x = texte
    )
}

fn echapper(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#x27;")
}

fn refus(message: impl Into<String>) -> Echec {
    Echec { classe: Some(socle::echec::Classe::Refuse),message: message.into() }
}

/// Les deux adresses que l'autorisation appelle : les jetons et l'adresse du compte. Les vraies par defaut ; l'appelant
/// peut pointer sur un serveur local (`de_la_base`).
#[derive(Debug, Clone)]
pub struct Adresses {
    pub jetons: String,
    pub compte: String,
}

impl Adresses {
    /// Les adresses de Google.
    pub fn de_google() -> Adresses {
        Adresses { jetons: JETONS.to_string(), compte: USERINFO.to_string() }
    }

    /// Les adresses d'un Google qui repond a `base` (un serveur local).
    pub fn de_la_base(base: &str) -> Adresses {
        Adresses { jetons: format!("{}/token", base), compte: format!("{}/userinfo", base) }
    }
}

/// `n` octets du systeme, base64 URL sans `=`.
fn aleatoire_url(n: usize) -> Result<String, Echec> {
    let mut o = vec![0u8; n];
    getrandom::fill(&mut o).map_err(|e| refus(format!("Le systeme ne donne pas d'aleas : {}.", e)))?;
    Ok(base64_url(&o))
}

fn base64_url(o: &[u8]) -> String {
    socle::base64::ecrire(o).replace('+', "-").replace('/', "_").trim_end_matches('=').to_string()
}

fn egaux(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `%XX` et `+`, les valeurs vides ecartees.
fn decoder(t: &str) -> String {
    let b = t.as_bytes();
    let mut sortie = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => sortie.push(b' '),
            b'%' => {
                let hexa = |o: Option<&u8>| o.and_then(|c| (*c as char).to_digit(16));
                match (hexa(b.get(i + 1)), hexa(b.get(i + 2))) {
                    (Some(h), Some(l)) => {
                        sortie.push((h * 16 + l) as u8);
                        i += 2;
                    }
                    // Un `%` que deux chiffres hexadecimaux ne suivent pas reste tel quel.
                    _ => sortie.push(b'%'),
                }
            }
            c => sortie.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&sortie).into_owned()
}

/// La premiere valeur non vide de `cle` dans la requete, s'il y en a une.
fn premiere_valeur(requete: &str, cle: &str) -> Option<String> {
    requete.split('&').filter_map(|p| p.split_once('=')).find(|(k, v)| decoder(k) == cle && !v.is_empty()).map(|(_, v)| decoder(v))
}

/// Ce que Google a renvoye a l'adresse locale : le code d'autorisation, ou l'erreur.
struct Retour {
    code: Option<String>,
    erreur: Option<String>,
}

/// Sert une connexion : `Some(retour)` quand c'est la reponse de Google (le `state` est le bon), `None` sinon (on continue d'attendre).
/// Un `state` faux : 403. Une autre methode que GET : 501, comme `BaseHTTPRequestHandler`.
fn servir(flux: &mut TcpStream, etat: &str) -> Option<Retour> {
    let _ = flux.set_read_timeout(Some(Duration::from_secs(5)));
    let mut tampon = [0u8; 8192];
    let mut lu = 0;
    while lu < tampon.len() {
        match flux.read(&mut tampon[lu..]) {
            Ok(0) => break,
            // Une connexion coupee en route n'est pas la reponse de Google : on continue d'attendre.
            Err(coupee) => {
                drop(coupee);
                return None;
            }
            Ok(n) => {
                lu += n;
                if tampon[..lu].windows(2).any(|w| w == b"\r\n") {
                    break;
                }
            }
        }
    }
    let texte = String::from_utf8_lossy(&tampon[..lu]).into_owned();
    let repondre_vide = |flux: &mut TcpStream, statut: &str| {
        let _ = flux.write_all(format!("HTTP/1.1 {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", statut).as_bytes());
    };
    // La ligne de la demande : `GET <cible> HTTP/1.1`. Toute autre methode, ou une ligne qui n'en est pas une : 501.
    let Some(cible) = texte.lines().next().and_then(|l| l.strip_prefix("GET ")).and_then(|r| r.split(' ').next()) else {
        repondre_vide(flux, "501 Not Implemented");
        return None;
    };
    // Sans requete, pas de `state` : 403.
    let Some((_, requete)) = cible.split_once('?') else {
        repondre_vide(flux, "403 Forbidden");
        return None;
    };
    let requete = match requete.split_once('#') {
        Some((avant, _)) => avant,
        None => requete,
    };
    if !premiere_valeur(requete, "state").is_some_and(|s| egaux(&s, etat)) {
        repondre_vide(flux, "403 Forbidden");
        return None;
    }
    let retour = Retour { code: premiere_valeur(requete, "code"), erreur: premiere_valeur(requete, "error") };
    let page = match &retour.erreur {
        None => page_retour("C'est bon.", "#4f9f6f", "Tu peux fermer cette fenetre et revenir a PuppetPaie."),
        Some(e) => page_retour("Autorisation refusee", "#d05353", &echapper(e)),
    };
    let corps = page.into_bytes();
    let _ = flux.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", corps.len()).as_bytes());
    let _ = flux.write_all(&corps);
    Some(retour)
}

/// La reponse de Google lue en sa forme `T`, ou la phrase de la panne.
fn appeler<T: serde::de::DeserializeOwned>(requete: &Requete) -> Result<T, Echec> {
    match envoyer(requete) {
        Ok((brut, _)) => lue(brut, "Google a rendu une reponse illisible. Aucune autorisation enregistree. Recommencer plus tard.").map_err(refus),
        Err(panne) => match phrase_de_panne(panne) {
            Ok(phrase) => Err(refus(phrase)),
            Err((_, message)) => Err(refus(format!("Google ne repond pas : {}. Aucune autorisation enregistree.", message))),
        },
    }
}

/// L'autorisation sans navigateur. `identifiants` : l'application (client_id, client_secret), lue par l'appelant ;
/// `adresses` : celles de Google, ou d'un serveur local. `dire_url` recoit l'URL de la fenetre de consentement (la personne
/// l'ouvre), `dire` les avertissements. Rend le jeton obtenu, que l'appelant scelle. `delai` : l'attente de la reponse de
/// Google, en secondes.
pub fn autoriser(
    application: &Application,
    adresses: &Adresses,
    delai: u64,
    dire_url: &dyn Fn(&str),
    dire: &dyn Fn(&str),
) -> Result<JetonGoogle, Echec> {
    let verificateur = aleatoire_url(64)?;
    let defi = base64_url(&Sha256::digest(verificateur.as_bytes()));
    let etat = aleatoire_url(24)?;

    let ecouteur = TcpListener::bind("127.0.0.1:0").map_err(|e| refus(format!("Impossible d'ecouter sur 127.0.0.1 : {}.", e)))?;
    let adresse = ecouteur.local_addr().map_err(|e| refus(format!("Impossible d'ecouter sur 127.0.0.1 : {}.", e)))?;
    let redirection = format!("http://127.0.0.1:{}/", adresse.port());
    let url = format!(
        "{}?{}",
        AUTORISATION,
        encoder(&[
            ("client_id", application.client_id.clone()),
            ("redirect_uri", redirection.clone()),
            ("response_type", "code".into()),
            ("scope", PERMISSIONS.join(" ")),
            ("access_type", "offline".into()),
            // Sans « consent », Google ne renvoie le jeton de rafraichissement qu'a la toute premiere autorisation.
            ("prompt", "consent".into()),
            ("code_challenge", defi),
            ("code_challenge_method", "S256".into()),
            ("state", etat.clone()),
        ])
    );

    let (tx, rx) = mpsc::channel::<Retour>();
    let fini = Arc::new(AtomicBool::new(false));
    let fil_fini = Arc::clone(&fini);
    let fil_etat = etat.clone();
    std::thread::spawn(move || {
        for flux in ecouteur.incoming() {
            if fil_fini.load(Ordering::SeqCst) {
                break;
            }
            // Une connexion qui echoue n'est pas la reponse de Google : on continue d'attendre.
            let mut flux = match flux {
                Ok(f) => f,
                Err(manquee) => {
                    drop(manquee);
                    continue;
                }
            };
            if let Some(r) = servir(&mut flux, &fil_etat) {
                let _ = tx.send(r);
                break;
            }
        }
    });
    dire_url(&url);

    let recu = match rx.recv_timeout(Duration::from_secs(delai)) {
        Ok(r) => r,
        // Le delai passe (ou le fil s'est arrete sans reponse) : la personne n'a pas accepte.
        Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {
            // Reveille le fil, qui attend une connexion, pour qu'il rende le port.
            fini.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect(adresse);
            return Err(refus(format!(
                "Aucune reponse de Google en {} minutes : aucune autorisation enregistree. Recommencer, et accepter dans la fenetre du navigateur.",
                std::cmp::max(1, delai / 60)
            )));
        }
    };
    if let Some(e) = recu.erreur {
        return Err(refus(format!("Google a refuse : {}", e)));
    }
    let Some(code) = recu.code else {
        return Err(refus("Google n'a renvoye aucun code d'autorisation : aucune autorisation enregistree. Recommencer."));
    };

    let corps = encoder(&[
        ("client_id", application.client_id.clone()),
        ("client_secret", application.client_secret.clone()),
        ("code", code),
        ("code_verifier", verificateur),
        ("grant_type", "authorization_code".into()),
        ("redirect_uri", redirection),
    ]);
    let reponse: ReponseJeton = appeler(&Requete {
        url: &adresses.jetons,
        donnees: Some(corps.as_bytes()),
        content_type: Some("application/x-www-form-urlencoded"),
        post: true,
        delai: DELAI,
        ..Default::default()
    })?;
    let non_vide = |t: Option<String>| t.filter(|x| !x.is_empty());
    let Some(rafraichissement) = non_vide(reponse.refresh_token) else {
        return Err(refus(
            "Google n'a pas renvoye de jeton de rafraichissement. Retirer\nl'acces de l'application sur myaccount.google.com > Securite >\nVos connexions a des applications tierces, puis recommencer.",
        ));
    };
    let Some(acces) = non_vide(reponse.access_token) else {
        return Err(refus("Google n'a pas renvoye de jeton d'acces : aucune autorisation enregistree. Recommencer."));
    };
    // Sans `scope`, Google n'a accorde aucune permission.
    let mut accordees: Vec<String> = reponse.scope.iter().flat_map(|s| s.split_whitespace()).map(str::to_string).collect();
    accordees.sort();
    accordees.dedup();
    let courte = |p: &'static str| match p.rsplit_once('/') {
        Some((_, c)) => c,
        None => p,
    };
    let oubliees: Vec<&str> = PERMISSIONS.iter().filter(|p| !accordees.iter().any(|a| a == *p)).map(|p| courte(p)).collect();
    if !oubliees.is_empty() {
        dire(&format!(
            "Attention : permission(s) non accordee(s) : {}.\n  Les cases correspondantes n'ont pas ete cochees dans la fenetre.",
            oubliees.join(", ")
        ));
    }

    let autorisation = format!("Bearer {}", acces);
    let entetes = [("Authorization", autorisation.as_str())];
    let infos: ReponseCompte = appeler(&Requete { url: &adresses.compte, entetes: &entetes, delai: DELAI, ..Default::default() })?;
    let Some(compte) = non_vide(infos.email) else {
        return Err(refus("Google n'a pas donne l'adresse du compte : aucune autorisation enregistree. Recommencer."));
    };

    Ok(JetonGoogle { rafraichissement, compte, permissions: accordees })
}

/// Ce que l'autorisation obtient : le jeton de rafraichissement, l'adresse du compte, les permissions accordees.
/// L'appelant le scelle (`moteur::routes::coffre::sceller_le_jeton`) : ce flux n'ouvre jamais le coffre.
pub struct JetonGoogle {
    pub rafraichissement: String,
    pub compte: String,
    pub permissions: Vec<String>,
}
