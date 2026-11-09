//! Un serveur HTTP/1.1 minuscule, sur 127.0.0.1 seulement : ce que la fenetre de PuppetPaie interroge.
//!
//! Un fil par connexion, la connexion gardee ouverte (HTTP/1.1, `Content-Length` toujours pose) : en HTTP/1.0, Windows
//! coupait parfois un gros envoi avant que la page l'ait lu. Le serveur ne ferme jamais le premier.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

/// Au-dela, une requete est refusee : la page n'envoie que du JSON et des images de quelques Mo.
const CORPS_MAX: usize = 64 * 1024 * 1024;

pub struct Requete {
    pub methode: String,
    pub chemin: String,
    pub params: HashMap<String, String>,
    pub entetes: HashMap<String, String>,
    pub corps: Vec<u8>,
}

impl Requete {
    /// Le parametre `nom` de l'adresse (`?nom=…`), s'il y est.
    pub fn param(&self, nom: &str) -> Option<&str> {
        self.params.get(nom).map(String::as_str)
    }

    pub fn entete(&self, nom: &str) -> Option<&str> {
        self.entetes.get(&nom.to_ascii_lowercase()).map(String::as_str)
    }

    pub fn cookie(&self, nom: &str) -> Option<String> {
        self.entete("cookie")?.split(';').find_map(|c| {
            let (k, v) = c.trim().split_once('=')?;
            (k == nom).then(|| v.trim_matches('"').to_string())
        })
    }
}

pub struct Reponse {
    pub code: u16,
    pub type_mime: String,
    pub corps: Vec<u8>,
    pub entetes: Vec<(String, String)>,
}

impl Reponse {
    pub fn nouvelle(code: u16, type_mime: &str, corps: impl Into<Vec<u8>>) -> Reponse {
        Reponse { code, type_mime: type_mime.into(), corps: corps.into(), entetes: Vec::new() }
    }

    /// Une reponse en JSON, ecrite depuis sa forme (`socle::ecriture`).
    pub fn json<T: serde::Serialize + ?Sized>(code: u16, v: &T) -> Reponse {
        Reponse::nouvelle(code, "application/json; charset=utf-8", socle::ecriture::json_en_ligne(v).into_bytes())
    }

    pub fn avec(mut self, nom: &str, valeur: impl Into<String>) -> Reponse {
        self.entetes.push((nom.into(), valeur.into()));
        self
    }
}

fn phrase(code: u16) -> &'static str {
    match code {
        200 => "OK",
        303 => "See Other",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    }
}

/// `%xx` -> l'octet, `+` -> espace (comme `unquote_plus`). Un `%` que deux chiffres hexadecimaux ne suivent pas reste
/// tel quel : `u8::from_str_radix` accepterait un signe (`%+1`), on ne lui donne que des chiffres.
pub fn decoder_pourcent(s: &str) -> String {
    let o = s.as_bytes();
    let mut v = Vec::with_capacity(o.len());
    let mut i = 0;
    while i < o.len() {
        match o[i] {
            b'+' => v.push(b' '),
            b'%' if i + 2 < o.len() && o[i + 1].is_ascii_hexdigit() && o[i + 2].is_ascii_hexdigit() => {
                v.push(hexa(o[i + 1]) * 16 + hexa(o[i + 2]));
                i += 2;
            }
            b => v.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&v).into_owned()
}

/// La valeur d'un chiffre hexadecimal (`decoder_pourcent` ne lui donne que des chiffres).
fn hexa(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        _ => c - b'A' + 10,
    }
}

/// La longueur du corps (`Content-Length`), ou le code du refus : illisible (400), trop gros (413).
fn longueur_du_corps(v: &str) -> Result<usize, u16> {
    let n: usize = v.trim().parse().map_err(|_| 400u16)?;
    if n > CORPS_MAX {
        return Err(413);
    }
    Ok(n)
}

/// La requete suivante du flux : lue, refusee (son code), ou `None` quand le client a fini (le flux est clos).
fn lire_requete(flux: &mut BufReader<TcpStream>) -> std::io::Result<Option<Result<Requete, u16>>> {
    let mut ligne = String::new();
    if flux.read_line(&mut ligne)? == 0 {
        return Ok(None);
    }
    let mut morceaux = ligne.split_whitespace();
    let (Some(methode), Some(cible)) = (morceaux.next(), morceaux.next()) else { return Ok(Some(Err(400))) };
    let (methode, cible) = (methode.to_string(), cible.to_string());
    let mut entetes = HashMap::new();
    loop {
        let mut l = String::new();
        if flux.read_line(&mut l)? == 0 {
            return Ok(None);
        }
        let l = l.trim_end();
        if l.is_empty() {
            break;
        }
        if let Some((k, v)) = l.split_once(':') {
            entetes.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    // Sans `Content-Length`, la requete n'a pas de corps.
    let mut corps = Vec::new();
    const LONGUEUR: &str = "content-length";
    if let Some(v) = entetes.get(LONGUEUR) {
        match longueur_du_corps(v) {
            Ok(n) => corps.resize(n, 0),
            Err(code) => return Ok(Some(Err(code))),
        }
    }
    flux.read_exact(&mut corps)?;
    let (chemin, requete) = match cible.split_once('?') {
        Some(c) => c,
        None => (cible.as_str(), ""),
    };
    let params = requete
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (decoder_pourcent(k), decoder_pourcent(v)),
            None => (decoder_pourcent(p), String::new()),
        })
        .collect();
    Ok(Some(Ok(Requete { methode, chemin: decoder_pourcent(chemin), params, entetes, corps })))
}

/// Une requete refusee avant d'etre lue : l'enveloppe d'un refus, que la page sait lire (`d.refus.phrase`), et non un
/// corps vide sur lequel `r.json()` levait une SyntaxError.
fn refus_de_lecture(code: u16) -> Reponse {
    let message = match code {
        413 => format!(
            "La demande est trop grosse pour PuppetPaie (plus de {} Mo) : rien n'a été lu ni enregistré.",
            CORPS_MAX / (1024 * 1024)
        ),
        _ => format!("La demande n'a pas pu être lue (code {}).", code),
    };
    let refus = crate::routes::enveloppe::Refus::nouveau(socle::echec::Classe::RequeteRefusee, message);
    Reponse::json(code, &crate::routes::enveloppe::Reponse::refusee(refus))
}

/// LA CONNEXION FINIT sur une erreur du flux : le client parti (la fenetre fermee pendant un releve) ou silencieux trop
/// longtemps n'est pas un defaut, il n'y a plus personne a qui le dire ; une autre erreur se dit sur la sortie d'erreur.
fn fin_de_connexion(e: std::io::Error) {
    use std::io::ErrorKind::*;
    if !matches!(e.kind(), TimedOut | WouldBlock | ConnectionReset | ConnectionAborted | BrokenPipe | UnexpectedEof) {
        eprintln!("Une connexion de la fenêtre s'arrête : {}.", e);
    }
}

fn ecrire_reponse(flux: &mut TcpStream, r: &Reponse) -> std::io::Result<()> {
    let mut tete = format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n", r.code, phrase(r.code), r.type_mime, r.corps.len());
    // Rien ne se garde, sauf ce qui le dit (une feuille ou un script nomme par son empreinte, `interface/mod.rs`).
    if !r.entetes.iter().any(|(k, _)| k == "Cache-Control") {
        tete.push_str("Cache-Control: no-cache\r\n");
    }
    if !r.entetes.iter().any(|(k, _)| k == "Content-Security-Policy") {
        tete.push_str("Content-Security-Policy: default-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'none'\r\n");
    }
    for (k, v) in &r.entetes {
        tete.push_str(&format!("{}: {}\r\n", k, v));
    }
    tete.push_str("\r\n");
    // UNE SEULE ECRITURE, SANS NAGLE : la tete puis le corps en deux `write`, puis la lecture de la requete
    // suivante, c'est le motif ou le reste du corps attend l'accuse de reception de la tete, que l'autre bout differe
    // (jusqu'a 200 ms sous Windows ; sous Linux, en boucle locale, l'accuse part aussitot et rien ne se mesure). Tout part
    // d'un coup, sans attente (`servir_connexion` : `set_nodelay`).
    let mut tout = Vec::with_capacity(tete.len() + r.corps.len());
    tout.extend_from_slice(tete.as_bytes());
    tout.extend_from_slice(&r.corps);
    flux.write_all(&tout)?;
    flux.flush()
}

fn servir_connexion(flux: TcpStream, traiter: Arc<dyn Fn(Requete) -> Reponse + Send + Sync>) {
    let _ = flux.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = flux.set_nodelay(true);
    let mut sortie = match flux.try_clone() {
        Ok(s) => s,
        Err(e) => return fin_de_connexion(e),
    };
    let mut entree = BufReader::new(flux);
    loop {
        let r = match lire_requete(&mut entree) {
            Ok(Some(r)) => r,
            Ok(None) => return,
            Err(e) => return fin_de_connexion(e),
        };
        let (reponse, refusee) = match r {
            Ok(req) => (traiter(req), false),
            Err(code) => (refus_de_lecture(code), true),
        };
        if let Err(e) = ecrire_reponse(&mut sortie, &reponse) {
            return fin_de_connexion(e);
        }
        // Le corps d'une requete refusee n'a pas ete lu : la suite du flux n'est plus une requete. On ferme.
        if refusee {
            return;
        }
    }
}

/// Ecoute sur 127.0.0.1:`port` (le port fixe s'il est libre, sinon un port tire par le systeme). Rend l'ecouteur et son port.
pub fn ecouter(port: u16) -> std::io::Result<(TcpListener, u16)> {
    let l = TcpListener::bind(("127.0.0.1", port)).or_else(|_| TcpListener::bind(("127.0.0.1", 0)))?;
    sans_heritage(&l);
    let p = l.local_addr()?.port();
    Ok((l, p))
}

/// Une prise d'ecoute ne passe jamais a un processus enfant (une route, le relais de la mise a jour). La bibliotheque
/// standard la cree deja non heritable, mais un fournisseur reseau intercale (un antivirus) peut perdre ce drapeau : un
/// enfant la gardait alors ouverte apres la mort de la fenetre (port tenu par un PID mort).
pub fn sans_heritage(l: &TcpListener) {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawSocket;
        extern "system" {
            fn SetHandleInformation(objet: isize, masque: u32, drapeaux: u32) -> i32;
        }
        const HANDLE_FLAG_INHERIT: u32 = 1;
        // SAFETY : la prise est vivante (empruntee) ; on ne change que son drapeau d'heritage.
        unsafe {
            SetHandleInformation(l.as_raw_socket() as isize, HANDLE_FLAG_INHERIT, 0);
        }
    }
    #[cfg(not(windows))]
    let _ = l;
}

/// Sert pour toujours ; chaque connexion dans son propre fil, chaque requete traitee par `traiter`.
pub fn servir(ecouteur: TcpListener, traiter: Arc<dyn Fn(Requete) -> Reponse + Send + Sync>) {
    for flux in ecouteur.incoming().flatten() {
        let t = traiter.clone();
        crate::routes::fil_de_routes(move || servir_connexion(flux, t));
    }
}
