//! Le garde-fou reseau des essais : AUCUN ESSAI NE SORT DE LA MACHINE.
//!
//! Pourquoi : tout ce qui retombe sur une adresse reelle (le serveur de l'editeur, net-entreprises, l'Urssaf, Google,
//! la DILA, le depot des versions, une boite courriel) partirait des qu'un essai l'oublie, sur tout poste qui a
//! internet : un essai sans serveur pose joindrait le vrai serveur de l'editeur, qui inscrirait une compagnie et
//! enverrait un courriel. Reparer chaque essai ne suffit pas.
//!
//! La regle, en un seul endroit : quand la variable `PUPPETPAIE_ESSAIS_HORS_RESEAU` est posee -- Cargo la pose pour tout
//! processus qu'il lance, donc pour chaque essai et chaque `moteur` qu'un essai lance (`atelier/sources/.cargo/config.toml`,
//! `[env]`, `force`) --, une sortie vers une autre machine que celle-ci est REFUSEE avant toute connexion. La seule
//! porte : un mandataire LOCAL que l'essai nomme dans cette variable meme (`outils::mandataire`, un faux qui note a qui
//! le moteur voulait parler et refuse) ; il remplace alors le mandataire de l'environnement (celui d'un conteneur, qui
//! laisse passer, n'est jamais pris). Vers la machine elle-meme (les faux serveurs des essais), rien ne change.
//!
//! Hors essais (l'executable livre, lance hors de Cargo), la variable n'existe pas : rien ne change.
//!
//! DANS LE NAVIGATEUR (la cible WASI, la demo), le module n'a aucune prise reseau : toute sortie s'y refuse ici, avant
//! toute connexion (`SANS_PRISE`), et l'appelant fait de ce refus ce qu'il fait d'un serveur injoignable. Une seule
//! porte y mene dehors, vers le seul filet (`FILET`) : `par_l_hote`, le POST que l'hote JavaScript fait pour le module
//! (`systeme::poster_par_l_hote`), que seul le canal de l'editeur prend (`compagnie::editeur::poster` : les incidents
//! de la demo vont au filet, comme ceux de toute installation).
//!
//! Chaque sortie du moteur passe par ici : `transport::envoyer` (HTTP : editeur, net-entreprises, Urssaf, Google),
//! `sources::telecharger`, `entrepot_dila`, `maj` (le depot des versions), `courriel` (IMAP, SMTP),
//! `interface::poste::ouvrir_adresse` (le navigateur). L'essai `garde_reseau` (`tests/fichiers/`) le verifie dans les
//! sources : une nouvelle sortie qui n'y passerait pas le fait echouer.

use crate::echec::{Echec, R};
use std::net::IpAddr;

/// La variable du garde-fou. Posee (n'importe quelle valeur) : aucune sortie hors de la machine. Sa valeur peut nommer
/// un mandataire local (`http://127.0.0.1:<port>`), seule porte alors permise.
pub const VARIABLE: &str = "PUPPETPAIE_ESSAIS_HORS_RESEAU";

/// UNE CONNEXION VERS LA MACHINE ELLE-MEME (127.0.0.1, le port `port`), en essai comme hors essai : elle ne sort pas, le
/// garde-fou n'a rien a en refuser. Le type de l'adresse le tient : aucune autre machine ne se nomme ici. `delai` borne
/// l'attente de la connexion.
pub fn vers_cette_machine(port: u16, delai: Option<std::time::Duration>) -> std::io::Result<std::net::TcpStream> {
    let adresse = std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port));
    match delai {
        Some(d) => std::net::TcpStream::connect_timeout(&adresse, d),
        None => std::net::TcpStream::connect(adresse),
    }
}

/// UNE ADRESSE que le moteur appelle ou montre : un service (Google, net-entreprises, l'Urssaf, le serveur du filet, un
/// depot des mises a jour), un texte (Legifrance, le CASC-SVP). Elle commence par `http://` ou `https://` : c'est tout ce
/// que le code en sait (la RFC 3986 n'est pas deposee). Elle s'ecrit et s'envoie au caractere pres telle qu'on l'a lue ;
/// une adresse que le code compose se fait par `joindre`, une que le moteur ecrit en Rust par `du_moteur`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Url(std::borrow::Cow<'static, str>);

/// `texte` commence-t-il par `prefixe` ? (utilisable a la compilation)
const fn commence_par(texte: &[u8], prefixe: &[u8]) -> bool {
    if texte.len() < prefixe.len() {
        return false;
    }
    let mut i = 0;
    while i < prefixe.len() {
        if texte[i] != prefixe[i] {
            return false;
        }
        i += 1;
    }
    true
}

impl Url {
    /// Une adresse que le moteur ecrit en Rust ; une autre forme ne compile pas (dans une constante).
    pub const fn du_moteur(texte: &'static str) -> Url {
        assert!(commence_par(texte.as_bytes(), b"http://") || commence_par(texte.as_bytes(), b"https://"), "une adresse commence par http:// ou https://");
        Url(std::borrow::Cow::Borrowed(texte))
    }

    /// L'adresse `texte`, ou la phrase qui la refuse.
    pub fn lire(texte: &str) -> R<Url> {
        if !(texte.starts_with("http://") || texte.starts_with("https://")) {
            return Err(crate::refus!("{} n'est pas une adresse : elle commence par http:// ou https://.", crate::fichiers::repr(texte)));
        }
        Ok(Url(std::borrow::Cow::Owned(texte.to_string())))
    }

    /// L'adresse suivie de `suite`, telle quelle (`/drive/v3/files`, `?q=…`) : le debut ne change pas.
    pub fn joindre(&self, suite: &str) -> Url {
        Url(std::borrow::Cow::Owned(format!("{}{}", self.0, suite)))
    }

    /// L'adresse telle qu'ecrite.
    pub fn texte(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Url {
    type Error = Echec;
    fn try_from(s: String) -> R<Url> {
        Url::lire(&s)
    }
}

impl From<Url> for String {
    fn from(u: Url) -> String {
        u.0.into_owned()
    }
}

/// Ce que le garde-fou dit d'une sortie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sortie {
    /// Hors essais : rien ne change (le mandataire de l'environnement fait foi).
    Libre,
    /// En essai, vers la machine elle-meme : directe, sans mandataire.
    Locale,
    /// En essai, vers dehors : par le seul mandataire local que l'essai a nomme.
    ParLaPorte(String),
}

/// Le nom d'hote d'une adresse (`https://hote:port/chemin` -> `hote` ; `[::1]` -> `::1`).
pub fn hote_de(url: &str) -> String {
    let reste = match url.split_once("://") {
        Some((_, r)) => r,
        None => url,
    };
    let autorite = avant(reste, &['/', '?', '#']);
    let hote = match autorite.rsplit_once('@') {
        Some((_, h)) => h,
        None => autorite,
    };
    let nom = match hote.strip_prefix('[') {
        Some(h) => avant(h, &[']']),
        None => avant(hote, &[':']),
    };
    nom.to_lowercase()
}

/// `t` jusqu'au premier de ces caracteres, ou entier.
fn avant<'a>(t: &'a str, fins: &[char]) -> &'a str {
    match t.find(fins) {
        Some(i) => &t[..i],
        None => t,
    }
}

/// Cette machine : `localhost` (et ses sous-noms), 127.0.0.0/8, `::1`.
pub fn est_locale(hote: &str) -> bool {
    let h = hote.trim_start_matches('[').trim_end_matches(']').to_lowercase();
    // Un hote est une adresse ou un nom : l'adresse qu'il ecrit, s'il en ecrit une (un nom n'en ecrit aucune), est-elle
    // celle de cette machine ?
    h == "localhost" || h.ends_with(".localhost") || h.parse::<IpAddr>().iter().any(IpAddr::is_loopback)
}

/// En essai ? (la variable est posee, meme vide).
pub fn en_essai() -> bool {
    std::env::var_os(VARIABLE).is_some()
}

fn refus(hote: &str) -> Echec {
    crate::echec::refus(format!(
        "Garde-fou des essais ({}) : aucune sortie vers {} ; un essai pose un serveur local, ou nomme un faux mandataire local dans cette variable.",
        VARIABLE, hote
    ))
}

/// Le systeme n'a aucune prise reseau : dans le navigateur (WASI), le module ne se connecte a rien.
const SANS_PRISE: bool = cfg!(target_os = "wasi");

/// La decision pour une sortie vers `hote`.
pub fn sortie_vers(hote: &str) -> R<Sortie> {
    if SANS_PRISE {
        return Err(crate::echec::refus(format!("Dans le navigateur, le logiciel ne se connecte à aucun serveur : rien ne part vers {}.", hote)));
    }
    // LE FIL DE FOND de la fenetre (`fil_de_fond`) : il tourne avant tout mot de passe, et ne sort jamais.
    if crate::fil_de_fond::ici() {
        return Err(crate::echec::refus(crate::fil_de_fond::refus(&format!("une sortie vers {}", hote))));
    }
    let Some(valeur) = std::env::var_os(VARIABLE) else { return Ok(Sortie::Libre) };
    if est_locale(hote) {
        return Ok(Sortie::Locale);
    }
    let porte = valeur.to_string_lossy().trim().to_string();
    if porte.starts_with("http://") && est_locale(&hote_de(&porte)) {
        return Ok(Sortie::ParLaPorte(porte));
    }
    Err(refus(hote))
}

/// LE FILET, le serveur de l'editeur : les incidents, « Un probleme, une idee ? » et leurs reponses. Il repond sur
/// `filet.puppetpaie.fr` (le serveur vit dans le depot du site de l'editeur, `infra/filet/`). C'est le serveur du canal de
/// l'editeur quand la configuration ne dit rien (`compagnie::editeur::SERVEUR_PAR_DEFAUT`), et la seule adresse que le
/// navigateur joint (`par_l_hote`).
pub const FILET: Url = Url::du_moteur("https://filet.puppetpaie.fr");

/// UN POST DU MODULE, tel que l'hote du navigateur le recoit (en JSON) : l'adresse, les en-tetes, le corps en base64,
/// l'attente au plus.
#[cfg(target_os = "wasi")]
#[derive(serde::Serialize)]
struct DemandeALHote<'a> {
    adresse: &'a str,
    entetes: &'a [(&'a str, &'a str)],
    corps: String,
    delai_ms: u64,
}

/// CE QUE L'HOTE DU NAVIGATEUR REND d'un POST (en JSON) : la reponse du serveur, quel que soit son code
/// (`{"reponse": {"code", "corps"}}`, le corps en base64), ou la panne qui l'a empeche (`{"panne": "…"}` : pas de reseau,
/// le serveur ne repond pas, le navigateur refuse).
#[cfg(target_os = "wasi")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
enum ReponseDeLHote {
    Reponse(RenduDeLHoteLu),
    Panne(String),
}

/// La reponse du serveur, telle que l'hote l'ecrit.
#[cfg(target_os = "wasi")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RenduDeLHoteLu {
    code: u16,
    corps: String,
}

/// LA REPONSE DU SERVEUR, par l'hote du navigateur : son code HTTP, son corps.
#[cfg(target_os = "wasi")]
pub struct RenduDeLHote {
    pub code: u16,
    pub corps: Vec<u8>,
}

/// LA SEULE PORTE DU NAVIGATEUR VERS DEHORS : un POST vers le filet (`FILET`), que l'hote JavaScript fait pour le module
/// (`systeme::poster_par_l_hote`). Toute autre adresse, et le fil de fond (`fil_de_fond`), se refusent ici, avant que
/// rien ne parte ; la panne de l'hote (pas de reseau, une reponse qui ne se lit pas) est un refus de classe `Reseau`,
/// que l'appelant fait sienne comme celle d'un serveur injoignable.
#[cfg(target_os = "wasi")]
pub fn par_l_hote(url: &Url, entetes: &[(&str, &str)], corps: &[u8], delai: std::time::Duration) -> R<RenduDeLHote> {
    let hote = hote_de(url.texte());
    if crate::fil_de_fond::ici() {
        return Err(crate::echec::refus(crate::fil_de_fond::refus(&format!("une sortie vers {}", hote))));
    }
    // L'adresse entiere commence par celle du filet, suivie d'un chemin : le schema, l'hote et le port sont ceux du
    // filet, et rien de ce qui suit (`@`, `\`) ne les change pour le navigateur.
    if !url.texte().strip_prefix(FILET.texte()).is_some_and(|reste| reste.starts_with('/')) {
        return Err(crate::echec::refus(format!("Dans le navigateur, le logiciel ne joint que le serveur de l'éditeur : rien ne part vers {}.", hote)));
    }
    let delai_ms = u64::try_from(delai.as_millis()).map_err(|e| crate::refus!("L'attente d'un envoi au filet ne se dit pas en millisecondes ({}).", e))?;
    let demande = DemandeALHote { adresse: url.texte(), entetes, corps: crate::base64::ecrire(corps), delai_ms };
    let brut = crate::systeme::poster_par_l_hote(crate::ecriture::json_en_ligne(&demande).as_bytes());
    let panne = |raison: String| crate::echec::erreur(crate::echec::Classe::Reseau, raison);
    let texte = std::str::from_utf8(&brut).map_err(|e| panne(format!("la réponse du navigateur n'est pas du texte ({})", e)))?;
    match crate::forme::lire_texte::<ReponseDeLHote>(texte, "La réponse du navigateur à un envoi ne se lit pas.").map_err(|e| panne(e.phrase))? {
        ReponseDeLHote::Reponse(r) => {
            let corps = crate::base64::lire(&r.corps).map_err(|e| panne(e.phrase))?;
            Ok(RenduDeLHote { code: r.code, corps })
        }
        ReponseDeLHote::Panne(raison) => Err(panne(raison)),
    }
}

/// La decision pour une adresse (`https://...`).
pub fn sortie(url: &Url) -> R<Sortie> {
    sortie_vers(&hote_de(url.texte()))
}

/// Une connexion directe, sans mandataire possible (IMAP, SMTP, le navigateur) : en essai, vers cette machine seulement.
pub fn directe(hote: &str) -> R<()> {
    match sortie_vers(hote)? {
        Sortie::Libre | Sortie::Locale => Ok(()),
        Sortie::ParLaPorte(_) => Err(refus(hote)),
    }
}

/// Un agent `ureq` pour `url`, regle par le garde-fou : hors essais, tel quel ; en essai, sans mandataire vers cette
/// machine, par la porte seule vers dehors, refuse sinon.
pub fn regler(c: ureq::config::ConfigBuilder<ureq::typestate::AgentScope>, url: &Url) -> R<ureq::config::ConfigBuilder<ureq::typestate::AgentScope>> {
    Ok(match sortie(url)? {
        Sortie::Libre => c,
        Sortie::Locale => c.proxy(None),
        Sortie::ParLaPorte(p) => c.proxy(Some(ureq::Proxy::new(&p).map_err(|e| crate::echec::refus(format!("{} : mandataire illisible ({}) : {}", VARIABLE, p, e)))?)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cargo pose la variable pour tout essai (`.cargo/config.toml`, `[env]`) : ce processus-ci est un essai, et rien ne
    /// sort vers le serveur de l'editeur, ni vers aucun autre ; la machine elle-meme reste joignable.
    #[test]
    fn dans_un_essai_rien_ne_sort_de_la_machine() {
        assert!(en_essai(), "Cargo doit poser {} pour les essais (.cargo/config.toml)", VARIABLE);
        for url in ["https://filet.puppetpaie.fr", "https://services.net-entreprises.fr/", "https://oauth2.googleapis.com/token", "https://github.com/LeandreSimioni/PuppetPaie/releases/latest/download"] {
            assert!(sortie(&Url::lire(url).unwrap()).is_err(), "{}", url);
        }
        assert!(directe("smtp.gmail.com").is_err());
        assert_eq!(sortie(&Url::du_moteur("http://127.0.0.1:8080/x")).ok(), Some(Sortie::Locale));
        assert!(directe("localhost").is_ok());
    }
}
