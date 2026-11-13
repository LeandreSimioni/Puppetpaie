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

/// La decision pour une sortie vers `hote`.
pub fn sortie_vers(hote: &str) -> R<Sortie> {
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
