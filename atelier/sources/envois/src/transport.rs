//! Le transport vers net-entreprises et l'Urssaf : le client HTTP, l'authentification, les consultations, et l'envoi du
//! corps d'un depot (DSN, DPAE).
//!
//! CE MODULE NE CHERCHE JAMAIS UN SECRET : le mot de passe arrive de l'appelant, sert le temps de l'authentification
//! et n'est ecrit nulle part ; il n'ouvre pas le coffre et n'en parle pas. Le jeton, lui, vaut une session : c'est
//! l'appelant qui le garde en memoire et le repasse.
//!
//! Les pannes : `Organisme` (message et `code`) ; un delai de lecture (`DelaiDepasse`), une reponse coupee, une connexion
//! rompue, un gzip illisible (`Reseau`) portent leur `classe`, que l'appelant releve.

use droit::fiche::Lue as _;
use socle::echec::Classe;
use std::io::Read;
use std::sync::mpsc;
use std::time::Duration;

use ureq::http;

pub const USER_AGENT: &str = "Client-DSN (PuppetPaie/1.0; PuppetPaie)";

/// Les messages de l'API par statut HTTP (guide, section 8).
pub const MESSAGES: [(u16, &str); 7] = [
    (401, "Authentification refusee, ou declarant non inscrit au service DSN."),
    (
        403,
        "Compte temporairement bloque (trop de tentatives), ou tentative d'acces a des donnees deposees par quelqu'un d'autre.",
    ),
    (404, "Service ou identifiant inconnu. Peut aussi signaler une interruption de service."),
    (406, "Format de reponse demande non supporte."),
    (422, "Parametres invalides."),
    (
        429,
        "Contraintes de consultation non respectees : trop d'appels, ou plage de recherche trop large. Attendre et relancer.",
    ),
    (503, "Service indisponible."),
];

const DELAI_APPEL: u64 = 60;
pub const DELAI_DEPOT: u64 = 120;
const MAX_REDIRECTIONS: usize = 10;

/// Ce que le guide dit d'un statut HTTP, s'il en dit quelque chose.
pub fn message_du_statut(code: u16) -> Option<&'static str> {
    MESSAGES.iter().find(|(c, _)| *c == code).map(|(_, m)| *m)
}

/// `HTTP <code> <ou>`, puis ce que le guide dit du statut et le debut du corps de la reponse, chacun sur sa ligne.
fn phrase_du_statut(code: u16, ou: &str, corps: Option<String>) -> String {
    let mut m = format!("HTTP {} {}", code, ou);
    for ligne in message_du_statut(code).map(String::from).into_iter().chain(corps.filter(|c| !socle::texte::strip(c).is_empty())) {
        m.push_str("\n  ");
        m.push_str(&ligne);
    }
    m
}

// ----------------------------------------------------------------- les pannes

/// Ce qui peut mal tourner, en trois formes.
#[derive(Debug)]
pub enum Panne {
    /// Un statut HTTP hors 2xx ; le corps, lu en entier (ou la panne de sa lecture).
    Statut { code: u16, corps: Result<Vec<u8>, Box<Panne>> },
    /// La connexion n'a pas abouti : la raison.
    Reseau(String),
    /// Une panne après l'envoi, sous sa classe.
    Brute { classe: Classe, message: String },
}

impl Panne {
    fn brute(classe: Classe, message: impl Into<String>) -> Panne {
        Panne::Brute { classe, message: message.into() }
    }
}

/// UNE PANNE D'UN ORGANISME, telle que la releve la note : sa classe (`Organisme`, `DepotRefuse`, ou celle d'une panne
/// brute), sa phrase, et le statut HTTP quand il y en a un (0 : le reseau injoignable).
#[derive(Debug, Clone, PartialEq)]
pub struct EchecApi {
    pub erreur: String,
    pub classe: Classe,
    pub code: Option<i64>,
}

impl EchecApi {
    fn nouveau(classe: Classe, erreur: String, code: Option<i64>) -> EchecApi {
        EchecApi { erreur, classe, code }
    }
}

fn echec_brut(p: Panne) -> EchecApi {
    match p {
        Panne::Brute { classe, message } => EchecApi::nouveau(classe, message, None),
        // Les appelants traduisent les deux autres avant d'arriver ici.
        Panne::Reseau(r) => EchecApi::nouveau(Classe::Reseau, r, None),
        Panne::Statut { code, .. } => EchecApi::nouveau(Classe::Reseau, format!("HTTP {}", code), Some(code as i64)),
    }
}

// ------------------------------------------------------- les petits outils

use socle::texte::blanc as blanc;

/// Les mots d'un texte, separes par du blanc.
fn mots(s: &str) -> Vec<&str> {
    s.split(blanc).filter(|m| !m.is_empty()).collect()
}

/// `s[:n]` sur des points de code.
fn debut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Des octets lus en UTF-8, un caractère de remplacement pour ce qui ne se lit pas.
fn utf8_remplace(o: &[u8]) -> String {
    String::from_utf8_lossy(o).into_owned()
}

/// Jamais un identifiant dans un message.
pub fn sans_secret(url: &str) -> &str {
    avant(url, &['?'])
}

/// Ce qui precede le premier des `signes` ; aucun : tout le texte.
fn avant<'a>(t: &'a str, signes: &[char]) -> &'a str {
    match t.find(signes) {
        Some(i) => &t[..i],
        None => t,
    }
}

/// Un texte lu en nombre entier ; `None` s'il n'en est pas un.
fn entier(t: &str) -> Option<u64> {
    if t.is_empty() || !t.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    t.bytes().try_fold(0u64, |n, c| n.checked_mul(10)?.checked_add(u64::from(c - b'0')))
}

/// Les trois caracteres que le HTML reserve (`&`, `<`, `>`) echappes.
pub fn echapper(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `AAAAMMJJhhmmss` depuis un instant ISO (`AAAA-MM-JJ[Thh:mm:ss...]`), la forme des plages de consultation ; `None` :
/// l'instant ne se lit pas.
fn horodatage(iso: &str) -> Option<String> {
    let c: Vec<char> = iso.chars().collect();
    let chiffres = |a: usize, b: usize| -> Option<String> {
        let t: String = c.get(a..b)?.iter().collect();
        t.chars().all(|ch| ch.is_ascii_digit()).then_some(t)
    };
    let jour = format!("{}{}{}", chiffres(0, 4)?, chiffres(5, 7)?, chiffres(8, 10)?);
    let heure = if c.len() >= 19 { format!("{}{}{}", chiffres(11, 13)?, chiffres(14, 16)?, chiffres(17, 19)?) } else { "000000".to_string() };
    Some(jour + &heure)
}

// ------------------------------------------------------------------- gzip

/// Decompresse un corps gzip ; les refus (tronque, en-tete invalide, somme de controle) sont dits en francais.
pub fn gunzip(donnees: &[u8]) -> Result<Vec<u8>, Panne> {
    let eof = || Panne::brute(Classe::Reseau, "réponse gzip tronquée");
    let mut sortie = Vec::new();
    let mut reste = donnees;
    loop {
        if reste.is_empty() {
            return Ok(sortie);
        }
        if reste.len() < 2 || reste[0] != 0x1f || reste[1] != 0x8b {
            let debut: Vec<String> = reste.iter().take(2).map(|o| format!("{:02x}", o)).collect();
            return Err(Panne::brute(Classe::Reseau, format!("réponse attendue en gzip, elle commence par {}", debut.join(" "))));
        }
        let mut pos = 2;
        let lire = |pos: &mut usize, n: usize| -> Result<(), Panne> {
            if reste.len() < *pos + n {
                return Err(eof());
            }
            *pos += n;
            Ok(())
        };
        if reste.len() < 10 {
            return Err(eof());
        }
        let (methode, drapeaux) = (reste[2], reste[3]);
        pos += 8;
        if methode != 8 {
            return Err(Panne::brute(Classe::Reseau, "réponse gzip : méthode de compression inconnue"));
        }
        if drapeaux & 4 != 0 {
            if reste.len() < pos + 2 {
                return Err(eof());
            }
            let n = u16::from_le_bytes([reste[pos], reste[pos + 1]]) as usize;
            pos += 2;
            lire(&mut pos, n)?;
        }
        for drapeau in [8u8, 16u8] {
            if drapeaux & drapeau != 0 {
                loop {
                    match reste.get(pos) {
                        None => break,
                        Some(0) => {
                            pos += 1;
                            break;
                        }
                        Some(_) => pos += 1,
                    }
                }
            }
        }
        if drapeaux & 2 != 0 {
            lire(&mut pos, 2)?;
        }
        let mut d = flate2::Decompress::new(false);
        let mut membre = Vec::new();
        let en_entree = &reste[pos..];
        loop {
            let avant_in = d.total_in() as usize;
            let avant_out = d.total_out();
            membre.reserve(32 * 1024);
            let st = d
                .decompress_vec(&en_entree[avant_in..], &mut membre, flate2::FlushDecompress::None)
                .map_err(|e| Panne::brute(Classe::Reseau,format!("réponse gzip illisible : {}", e)))?;
            if st == flate2::Status::StreamEnd {
                break;
            }
            if d.total_in() as usize == avant_in && d.total_out() == avant_out {
                return Err(eof());
            }
        }
        let utilises = d.total_in() as usize;
        let apres = &en_entree[utilises..];
        if apres.len() < 8 {
            return Err(eof());
        }
        let crc = u32::from_le_bytes([apres[0], apres[1], apres[2], apres[3]]);
        let longueur = u32::from_le_bytes([apres[4], apres[5], apres[6], apres[7]]);
        let mut h = flate2::Crc::new();
        h.update(&membre);
        if crc != h.sum() {
            return Err(Panne::brute(Classe::Reseau, "réponse gzip : somme de contrôle fausse"));
        }
        if longueur != (membre.len() as u64 & 0xffff_ffff) as u32 {
            return Err(Panne::brute(Classe::Reseau, "réponse gzip : longueur fausse"));
        }
        sortie.extend_from_slice(&membre);
        let mut suite = &apres[8..];
        while let Some((0, r)) = suite.split_first().map(|(a, r)| (*a, r)) {
            suite = r;
        }
        reste = suite;
    }
}

// ------------------------------------------------------------------- HTTP

pub type Entetes = Vec<(String, String)>;

/// `Content-Type` -> `Content-Type`, comme un serveur les ecrit.
fn nom_canonique(n: &str) -> String {
    n.split('-')
        .map(|m| {
            let mut c = m.chars();
            let premiere: String = c.next().map(|p| p.to_ascii_uppercase()).into_iter().collect();
            premiere + c.as_str()
        })
        .collect::<Vec<_>>()
        .join("-")
}

fn errno_texte(e: &std::io::Error) -> String {
    match e.raw_os_error() {
        Some(n) => {
            let t = e.to_string();
            let suffixe = format!(" (os error {})", n);
            let t = match t.strip_suffix(&suffixe) {
                Some(sans) => sans,
                None => &t,
            };
            format!("[Errno {}] {}", n, t)
        }
        None => e.to_string(),
    }
}

fn delai_expire(https: bool) -> &'static str {
    if https {
        "The read operation timed out"
    } else {
        "timed out"
    }
}

/// Une panne de `ureq` en refus du moteur : `Panne::Reseau` avant la reponse, une panne brute apres l'envoi.
fn panne_de(e: ureq::Error, https: bool) -> Panne {
    use std::io::ErrorKind as K;
    use ureq::{Error, Timeout};
    match e {
        Error::Timeout(Timeout::Connect) | Error::Timeout(Timeout::Resolve) => Panne::Reseau("timed out".into()),
        Error::Timeout(_) => Panne::brute(Classe::DelaiDepasse, delai_expire(https)),
        Error::HostNotFound => Panne::Reseau("[Errno -2] Name or service not known".into()),
        Error::Io(io) => match io.kind() {
            K::UnexpectedEof => Panne::brute(Classe::Reseau, "Remote end closed connection without response"),
            // Observe sur le service reel : la connexion est rompue pendant
            // l'etablissement (mandataire, TLS) : la connexion n'a pas abouti.
            // ureq ne dit pas a quel moment.
            K::ConnectionReset | K::ConnectionAborted => Panne::Reseau(errno_texte(&io)),
            K::TimedOut if io.raw_os_error().is_none() => Panne::Reseau("timed out".into()),
            _ => Panne::Reseau(errno_texte(&io)),
        },
        Error::Protocol(p) => Panne::brute(Classe::Reseau, p.to_string()),
        Error::Rustls(r) => Panne::Reseau(format!("[SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: {}", r)),
        Error::Tls(t) => Panne::Reseau(format!("[SSL] {}", t)),
        Error::BadUri(u) => Panne::brute(Classe::Illisible, format!("unknown url type: {:?}", u)),
        autre => Panne::Reseau(autre.to_string()),
    }
}

/// Le mandataire de l'environnement : `https_proxy` pour
/// une adresse https, `http_proxy` pour une adresse http, et `no_proxy`
/// (noms, suffixes de domaine, `*`) qui l'ecarte. La machine elle-meme (les faux
/// serveurs de l'atelier et des essais) ne passe jamais par un mandataire.
fn mandataire(url: &str) -> Result<Option<ureq::Proxy>, Panne> {
    let Some((schema, reste)) = url.split_once("://") else { return Ok(None) };
    // La variable en minuscules, sinon en capitales ; posee mais vide, ou hors d'Unicode, elle ne dit rien.
    let lue = |n: &str| match std::env::var(n) {
        Ok(v) => Some(v),
        Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(..)) => None,
    };
    let var = |n: &str| lue(&n.to_lowercase()).or_else(|| lue(&n.to_uppercase())).filter(|v| !v.is_empty());
    let Some(valeur) = var(&format!("{}_proxy", schema.to_lowercase())) else { return Ok(None) };
    let autorite = avant(reste, &['/', '?', '#']);
    let hote = match autorite.rsplit_once('@') {
        Some((_, h)) => h,
        None => autorite,
    };
    let nom = match hote.strip_prefix('[') {
        Some(entre_crochets) => avant(entre_crochets, &[']']),
        None => avant(hote, &[':']),
    };
    if matches!(nom.to_lowercase().as_str(), "127.0.0.1" | "localhost" | "::1") {
        return Ok(None);
    }
    if let Some(np) = var("no_proxy") {
        for e in np.split(',') {
            let e = socle::texte::strip(e).trim_start_matches('.').to_lowercase();
            if e == "*" {
                return Ok(None);
            }
            if e.is_empty() {
                continue;
            }
            let (h, a) = (nom.to_lowercase(), hote.to_lowercase());
            let correspond = |x: &str| x == e || x.ends_with(&format!(".{}", e));
            if correspond(&h) || correspond(&a) {
                return Ok(None);
            }
        }
    }
    ureq::Proxy::new(&valeur).map(Some).map_err(|e| Panne::Reseau(format!("mandataire illisible ({}_proxy) : {}", schema.to_lowercase(), e)))
}

/// L'agent d'une demande. En essai, le garde-fou reseau (`garde_reseau`) le regle : rien ne sort de la machine.
fn agent(delai: u64, url: &str) -> Result<ureq::Agent, Panne> {
    let tls = url.starts_with("https:");
    let d = Some(Duration::from_secs(delai));
    let mut c = ureq::Agent::config_builder()
        .proxy(mandataire(url)?)
        .http_status_as_error(false)
        .max_redirects(0)
        .max_redirects_will_error(false)
        .user_agent(USER_AGENT)
        .accept("")
        .timeout_resolve(d)
        .timeout_connect(d)
        .timeout_send_request(d)
        .timeout_send_body(d)
        .timeout_recv_response(d);
    if tls {
        c = c.tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build());
    }
    let c = socle::garde_reseau::regler(c, url).map_err(|e| Panne::Reseau(e.into()))?;
    Ok(c.build().into())
}

/// Lit le corps par morceaux : un silence de `delai`
/// secondes entre deux morceaux, pas une duree totale.
fn lire_corps(corps: ureq::Body, delai: u64, attendu: Option<u64>, https: bool) -> Result<Vec<u8>, Panne> {
    let (tx, rx) = mpsc::channel::<Result<Vec<u8>, std::io::Error>>();
    std::thread::spawn(move || {
        let mut r = corps.into_reader();
        let mut tampon = vec![0u8; 64 * 1024];
        loop {
            match r.read(&mut tampon) {
                Ok(0) => {
                    let _ = tx.send(Ok(Vec::new()));
                    return;
                }
                Ok(n) => {
                    // Le lecteur est parti (delai depasse) : plus personne n'attend la suite.
                    if let Err(parti) = tx.send(Ok(tampon[..n].to_vec())) {
                        drop(parti);
                        return;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            }
        }
    });
    let mut tout = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(delai)) {
            Ok(Ok(m)) if m.is_empty() => return Ok(tout),
            Ok(Ok(m)) => tout.extend_from_slice(&m),
            Ok(Err(e)) => {
                if matches!(e.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted) {
                    return Err(Panne::brute(Classe::Reseau, errno_texte(&e)));
                }
                return Err(coupee(tout.len(), attendu));
            }
            // Le silence a dure le delai (ou le fil de lecture s'est arrete sans rien dire).
            Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => return Err(Panne::brute(Classe::DelaiDepasse, delai_expire(https))),
        }
    }
}

/// N octets lus, M attendus de plus.
fn coupee(lus: usize, attendu: Option<u64>) -> Panne {
    let message = match attendu {
        Some(a) if a as usize > lus => format!("IncompleteRead({} bytes read, {} more expected)", lus, a as usize - lus),
        _ => format!("IncompleteRead({} bytes read)", lus),
    };
    Panne::brute(Classe::Reseau, message)
}

/// L'adresse d'un `Location` de redirection, lue depuis l'adresse demandée.
fn joindre(base: &str, loc: &str) -> String {
    let loc = loc.replace(' ', "%20");
    if let Some(i) = loc.find("://") {
        if loc[..i].chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
            return loc;
        }
    }
    let (schema, reste) = match base.split_once("://") {
        Some((s, r)) => (s, r),
        None => return loc,
    };
    if let Some(l) = loc.strip_prefix("//") {
        return format!("{}://{}", schema, l);
    }
    let (autorite, chemin) = match reste.find(['/', '?']) {
        Some(i) => (&reste[..i], &reste[i..]),
        None => (reste, ""),
    };
    if loc.starts_with('/') {
        return format!("{}://{}{}", schema, autorite, loc);
    }
    let chemin = avant(chemin, &['?', '#']);
    let dossier = match chemin.rfind('/') {
        Some(i) => &chemin[..=i],
        None => "/",
    };
    format!("{}://{}{}{}", schema, autorite, dossier, loc)
}

fn en_tetes_de(parts: &http::response::Parts) -> Entetes {
    let mut v: Entetes = Vec::new();
    for (n, val) in parts.headers.iter() {
        let nom = nom_canonique(n.as_str());
        let val = String::from_utf8_lossy(val.as_bytes()).into_owned();
        // Un en-tête répété : le premier l'emporte.
        if !v.iter().any(|(k, _)| *k == nom) {
            v.push((nom, val));
        }
    }
    v
}

pub fn entete<'a>(e: &'a Entetes, nom: &str) -> Option<&'a str> {
    e.iter().find(|(k, _)| k.eq_ignore_ascii_case(nom)).map(|(_, v)| v.as_str())
}

/// Ce qu'on envoie : une requete du guide.
#[derive(Default)]
pub struct Requete<'a> {
    pub url: &'a str,
    pub jeton: Option<&'a str>,
    pub donnees: Option<&'a [u8]>,
    pub content_type: Option<&'a str>,
    /// L'en-tete `Content-Encoding: gzip` d'un depot.
    pub corps_gzip: bool,
    /// Un depot part toujours en POST, meme vide.
    pub post: bool,
    pub delai: u64,
    /// Des en-tetes en plus (le canal de l'editeur : `User-Agent`, `X-Filet-Cle`).
    pub entetes: &'a [(&'a str, &'a str)],
    /// Ne pas demander `gzip` (le canal de l'editeur ne le demande pas).
    pub sans_gzip: bool,
    /// Une autre methode que GET ou POST (`PATCH` du Drive) ; elle part avec son corps, meme vide.
    pub methode: Option<&'a str>,
}

/// Le corps de la reponse (decompresse si `Content-Encoding: gzip`) et ses
/// en-tetes ; une panne sinon (`Panne::Statut` pour un statut hors 2xx).
pub fn envoyer(r: &Requete) -> Result<(Vec<u8>, Entetes), Panne> {
    let https = r.url.starts_with("https:");
    let mut url = r.url.to_string();
    let mut donnees = r.donnees;
    let mut methode = r.methode;
    let mut post = r.post || methode.is_some() || donnees.is_some_and(|d| !d.is_empty());
    let mut avec_entetes_du_corps = true;
    let mut sauts = 0;
    let (parts, corps) = loop {
        let verbe = match methode {
            Some(m) => m,
            None if post => "POST",
            None => "GET",
        };
        let mut b = http::Request::builder().method(verbe).uri(&url);
        if !r.sans_gzip {
            b = b.header("Accept-Encoding", "gzip");
        }
        for (nom, valeur) in r.entetes {
            b = b.header(*nom, *valeur);
        }
        if avec_entetes_du_corps {
            // Un POST sans type de contenu part en formulaire.
            match r.content_type {
                Some(ct) => b = b.header("Content-Type", ct),
                None if post => b = b.header("Content-Type", "application/x-www-form-urlencoded"),
                None => {}
            }
            if r.corps_gzip {
                b = b.header("Content-Encoding", "gzip");
            }
        }
        // Le jeton ne suit une redirection que vers le meme hote.
        if let Some(j) = r.jeton {
            if sauts == 0 || meme_hote(r.url, &url) {
                b = b.header("Authorization", format!("DSNLogin jeton={}", j));
            }
        }
        let ag = agent(r.delai, &url)?;
        let rep = if post {
            // Un POST sans donnees part avec un corps vide.
            let corps: Vec<u8> = donnees.into_iter().flatten().copied().collect();
            ag.run(b.body(corps).map_err(|e| Panne::brute(Classe::Illisible, e.to_string()))?)
        } else {
            ag.run(b.body(()).map_err(|e| Panne::brute(Classe::Illisible, e.to_string()))?)
        }
        .map_err(|e| panne_de(e, https))?;
        let (parts, corps) = rep.into_parts();
        let code = parts.status.as_u16();
        let suite = matches!(code, 301 | 302 | 303 | 307 | 308) && (!post || code <= 303) && sauts < MAX_REDIRECTIONS;
        // Une adresse de redirection hors de l'ASCII ne se suit pas.
        let loc = parts.headers.get(http::header::LOCATION).and_then(|v| match v.to_str() {
            Ok(l) => Some(l.to_string()),
            Err(hors_ascii) => {
                drop(hors_ascii);
                None
            }
        });
        match (suite, loc) {
            (true, Some(l)) => {
                url = joindre(&url, &l);
                if post {
                    post = false;
                    methode = None;
                    donnees = None;
                    avec_entetes_du_corps = false;
                }
                sauts += 1;
                continue;
            }
            _ => break (parts, corps),
        }
    };
    let code = parts.status.as_u16();
    let entetes = en_tetes_de(&parts);
    // Une longueur annoncee qui ne se lit pas n'annonce rien.
    let attendu = entete(&entetes, "Content-Length").and_then(|v| entier(v.trim()));
    let brut = lire_corps(corps, r.delai, attendu, https);
    if !(200..300).contains(&code) {
        return Err(Panne::Statut { code, corps: brut.map_err(Box::new) });
    }
    let mut brut = brut?;
    if entete(&entetes, "Content-Encoding") == Some("gzip") {
        brut = gunzip(&brut)?;
    }
    Ok((brut, entetes))
}

fn meme_hote(a: &str, b: &str) -> bool {
    let hote = |u: &str| u.split_once("://").map(|(_, r)| avant(r, &['/', '?', '#']).to_string());
    hote(a) == hote(b)
}

// ------------------------------------------------- l'API des organismes

/// Le debut lisible du corps d'une reponse d'erreur ; un corps qui ne s'est pas lu ne dit rien de plus que le statut.
fn debut_du_corps(corps: Result<Vec<u8>, Box<Panne>>, n: usize) -> Option<String> {
    match corps {
        Ok(c) => Some(debut(&utf8_remplace(&c), n)),
        Err(illisible) => {
            drop(illisible);
            None
        }
    }
}

/// Une panne d'appel en `Organisme`.
fn en_erreur_api(url: &str, p: Panne) -> EchecApi {
    match p {
        Panne::Statut { code, corps } => {
            EchecApi::nouveau(Classe::Organisme, phrase_du_statut(code, &format!("sur {}", sans_secret(url)), debut_du_corps(corps, 400)), Some(code as i64))
        }
        Panne::Reseau(r) => EchecApi::nouveau(Classe::Organisme, format!("Reseau injoignable pour {} : {}", sans_secret(url), r), Some(0)),
        brute => echec_brut(brute),
    }
}

/// Un GET sous le jeton de la session : le corps et les en-tetes ; une panne en `Organisme`.
pub fn appeler(url: &str, jeton: Option<&str>) -> Result<(Vec<u8>, Entetes), EchecApi> {
    let r = Requete { url, jeton, delai: DELAI_APPEL, ..Default::default() };
    envoyer(&r).map_err(|p| en_erreur_api(url, p))
}

/// La consultation d'une plage : l'adresse de base, puis `debut[/fin]` en `AAAAMMJJhhmmss` (des instants ISO).
pub fn consulter(base: &str, jeton: Option<&str>, debut: &str, fin: Option<&str>) -> Result<Vec<u8>, EchecApi> {
    let mut url = base.to_string();
    let illisible = |iso: &str| EchecApi::nouveau(Classe::Illisible, format!("horodatage illisible : {}", iso), None);
    url.push_str(&horodatage(debut).ok_or_else(|| illisible(debut))?);
    if let Some(fin) = fin {
        url.push('/');
        url.push_str(&horodatage(fin).ok_or_else(|| illisible(fin))?);
    }
    appeler(&url, jeton).map(|(corps, _)| corps)
}

/// LE DECLARANT QUI S'AUTHENTIFIE : le SIRET, le nom et le prenom du compte ouvert pour le service (vides : la fiche ne
/// les dit pas).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Identite {
    pub siret: Option<String>,
    pub nom: Option<String>,
    pub prenom: Option<String>,
}

impl Identite {
    /// Ce qui lui manque pour s'authentifier, dit en mots.
    pub fn manquants(&self) -> Vec<&'static str> {
        [(&self.siret, "le SIRET"), (&self.nom, "le nom"), (&self.prenom, "le prénom")].into_iter().filter(|(c, _)| c.is_none()).map(|(_, m)| m).collect()
    }
}

/// L'identite du declarant inscrit au service DSN, telle que l'authentification l'envoie : celle que la fiche dit
/// (`declarant`), sinon le SIRET et le contact DSN coupe au premier espace (le nom, puis le prenom).
pub fn declarant(ent: &droit::fiche::ChampsEntreprise) -> Identite {
    if let Some(d) = ent.declarant.dite().filter(|d| !d.est_vide()) {
        return Identite { siret: d.siret.en_texte(), nom: d.nom.en_texte(), prenom: d.prenom.en_texte() };
    }
    let mut i = Identite { siret: ent.siret.en_texte(), nom: None, prenom: None };
    if let Some(brut) = ent.declarant_dsn.dite().and_then(|d| d.nom_prenom.portee()) {
        let morceaux = mots(socle::texte::strip(brut));
        if morceaux.len() >= 2 {
            i.nom = Some(morceaux[0].to_string());
            i.prenom = Some(morceaux[1..].join(" "));
        }
    }
    i
}

/// Ce qu'une authentification envoie : le declarant inscrit au service, le mot de passe deja prepare par l'appelant, le
/// service, et le jeton d'une session deja ouverte.
pub struct Authentification<'a> {
    pub url: &'a str,
    pub declarant: &'a Identite,
    pub motdepasse: Option<&'a str>,
    /// La variable d'environnement que le refus nomme quand le mot de passe manque.
    pub variable: &'a str,
    pub service: Option<&'a str>,
    pub jeton: Option<&'a str>,
}

/// UNE SESSION OUVERTE : son jeton, et l'heure ou il expire si le service la dit (l'en-tete `Expires`).
#[derive(Debug, Clone)]
pub struct Session {
    pub jeton: String,
    pub expire_le: Option<String>,
}

/// L'authentification du guide de l'API DSN : la session ouverte. Sans mot de passe ou sans declarant complet, rien ne
/// part. Le mot de passe ne survit pas a cet appel et n'entre dans aucun message.
pub fn authentifier(a: &Authentification) -> Result<Session, EchecApi> {
    let variable = a.variable;
    let mdp = match a.motdepasse {
        Some(m) if !m.is_empty() => m,
        _ => {
            return Err(EchecApi::nouveau(
                Classe::Organisme,
                format!(
                    "Mot de passe absent.\n\
Trois facons de le fournir, dans cet ordre de priorite :\n\n\
\x20 1. le passer a Client(motdepasse=...) -- ce que font les\n\
\x20    fenetres et les commandes apres avoir vu l'humain ;\n\
\x20 2. la variable d'environnement {v} ;\n\
\x20    PowerShell :  $env:{v} = (Read-Host \"mot de passe\")\n\
\x20 3. le coffre, pour les lectures automatiques :\n\
\x20    route coffre-poser, service net-entreprises\n",
                    v = variable
                ),
                Some(0),
            ))
        }
    };
    let decl = a.declarant;
    let (Some(siret), Some(nom), Some(prenom)) = (&decl.siret, &decl.nom, &decl.prenom) else {
        return Err(EchecApi::nouveau(
            Classe::Organisme,
            format!(
                "Le compte qui se connecte est incomplet, il manque {} : ceux du compte ouvert pour ce service, sans accent. Rien n'est parti.",
                decl.manquants().join(", ")
            ),
            Some(0),
        ));
    };
    let service = a.service.filter(|s| !s.is_empty());
    let corps = format!(
        "<identifiants><siret>{}</siret><nom>{}</nom><prenom>{}</prenom><motdepasse>{}</motdepasse>{}</identifiants>",
        siret,
        nom,
        prenom,
        echapper(mdp),
        // Le service ne s'ecrit que s'il est dit.
        service.iter().map(|s| format!("<service>{}</service>", s)).collect::<String>()
    );
    let url = a.url;
    let r = Requete {
        url,
        jeton: a.jeton,
        donnees: Some(corps.as_bytes()),
        content_type: Some("application/xml"),
        corps_gzip: false,
        post: false,
        delai: DELAI_APPEL,
        ..Default::default()
    };
    let (brut, entetes) = envoyer(&r).map_err(|p| en_erreur_api(url, p))?;
    let jeton: String = brut.iter().map(|&b| if b < 0x80 { b as char } else { '\u{fffd}' }).collect();
    Ok(Session { jeton: socle::texte::strip(&jeton).to_string(), expire_le: entete(&entetes, "Expires").map(String::from) })
}

/// L'envoi du corps d'un depot (deja prepare : octets gzip) : POST `text/plain`,
/// `Content-Encoding: gzip`, jeton DSNLogin. `forme` dit comment une panne se
/// raconte, `dsn` (`DepotRefuse`) ou `dpae` (`Organisme`).
pub fn depot(url: &str, jeton: &str, corps_gzip: &[u8], forme: &str, delai: u64) -> Result<(Vec<u8>, Entetes), EchecApi> {
    let r = Requete {
        url,
        jeton: Some(jeton),
        donnees: Some(corps_gzip),
        content_type: Some("text/plain"),
        corps_gzip: true,
        post: true,
        delai,
        ..Default::default()
    };
    envoyer(&r).map_err(|p| match (forme, p) {
            ("dsn", Panne::Statut { code, corps }) => match corps {
                // Le corps de la reponse passe par `sans_secret`.
                Ok(c) => EchecApi::nouveau(
                    Classe::DepotRefuse,
                    format!("Le point de depot a repondu {}.\n{}", code, sans_secret(&debut(&utf8_remplace(&c), 800))),
                    None,
                ),
                Err(p) => echec_brut(*p),
            },
            ("dsn", Panne::Reseau(r)) => EchecApi::nouveau(Classe::DepotRefuse, format!("Pas de reponse du point de depot : {}", r), None),
            (_, Panne::Statut { code, corps }) => {
                EchecApi::nouveau(Classe::Organisme, phrase_du_statut(code, &format!("au depot sur {}", sans_secret(url)), debut_du_corps(corps, 400)), None)
            }
            (_, Panne::Reseau(r)) => EchecApi::nouveau(Classe::Organisme, format!("Reseau injoignable pour {} : {}", sans_secret(url), r), None),
            (_, brute) => echec_brut(brute),
        })
}
