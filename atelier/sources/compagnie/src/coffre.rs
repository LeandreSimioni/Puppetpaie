//! Le coffre (`moteur coffre`) : les mots de passe des organismes, scelles par
//! AES-256-GCM. Le fichier `coffre.bin` porte sa forme (`VERSION = 2`) ; la cle du poste est a part (`dossier_cle`).
//!
//! Une cle maitresse de 32 octets scelle les secrets. Elle est scellee deux
//! fois : par la cle du poste (hors des donnees) et par le mot de passe du
//! logiciel (scrypt, parametres ecrits dans le fichier). Les fiches (ce qui est
//! pose, depuis quand) sont en clair et scellent les secrets avec elles.
//!
//! Les secrets ne passent que par l'entree et la sortie standard.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use serde::{Deserialize, Serialize};

use socle::base64::ecrire as base64_ecrire;
use socle::echec::Classe;
use socle::json_ascii::dumps;

pub const FICHIER: &str = "coffre.bin";
pub const FICHIER_CLE: &str = "cle-poste.bin";
/// Le nom de la seance gardee, que `Coffre::chemin_gardee` complete de l'empreinte du coffre (un fichier par coffre).
pub const GARDEE: &str = "ouverture-gardee.bin";
pub const VERSION: i64 = 2;
pub const CONSULTATION: &str = "consultation";
pub const DEPOT: &str = "depot";
pub const DESCRIPTION: &str = "PuppetPaie - mots de passe des organismes";

const SCRYPT_N: u64 = 32768;
const SCRYPT_R: u64 = 8;
const SCRYPT_P: u64 = 1;
const SCRYPT_MAXMEM: u64 = 96 * 1024 * 1024;
pub const LONGUEUR_MINIMALE: usize = 6;
pub const LONGUEUR_MINIMALE_LOGICIEL: usize = 6;

const AAD_POSTE: &[u8] = b"puppetpaie/enveloppe/poste";
const AAD_MOTDEPASSE: &[u8] = b"puppetpaie/enveloppe/motdepasse";
const AAD_GARDEE: &[u8] = b"puppetpaie/ouverture-gardee";

pub struct Service {
    pub nom: &'static str,
    pub libelle: &'static str,
    pub variable: &'static str,
    pub sert_a: &'static str,
    pub avec_compte: bool,
}

pub const SERVICES: [Service; 3] = [
    Service {
        nom: "net-entreprises",
        libelle: "net-entreprises (DSN)",
        variable: "NET_ENTREPRISES_MOTDEPASSE",
        sert_a: "relever les retours, lister les dépôts, déposer une DSN",
        avec_compte: false,
    },
    Service {
        nom: "urssaf",
        libelle: "Urssaf (DPAE)",
        variable: "URSSAF_MOTDEPASSE",
        sert_a: "vérifier les identifiants et déposer une DPAE",
        avec_compte: false,
    },
    Service {
        nom: "courriel",
        libelle: "Boîte courriel (IMAP et SMTP)",
        variable: "COURRIEL_MOTDEPASSE",
        sert_a: "envoyer les documents et les contrats, relever les signatures",
        avec_compte: true,
    },
];

// ------------------------------------------------------------------ les formes

/// LE COFFRE TEL QU'IL S'ECRIT (`coffre.bin`, format `VERSION`) : les parametres de la derivation du mot de passe du
/// logiciel, les enveloppes de la cle maitresse, les fiches en clair (ce qui est pose, depuis quand) et les secrets
/// scelles avec elles. Un coffre neuf n'a pas encore de secrets : il les recoit a sa premiere ecriture.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Contenu {
    version: i64,
    kdf: Kdf,
    enveloppes: Enveloppes,
    fiches: BTreeMap<String, FichePosee>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secrets: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logiciel_pose_le: Option<String>,
}

/// LES PARAMETRES DE LA DERIVATION DU MOT DE PASSE DU LOGICIEL (scrypt) : ecrits dans le coffre, verifies avant de
/// deriver (`deriver`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kdf {
    pub algo: String,
    pub n: u64,
    pub r: u64,
    pub p: u64,
    pub sel: String,
}

/// LES ENVELOPPES DE LA CLE MAITRESSE, en base64 : scellee par la cle du poste, et par le mot de passe du logiciel
/// quand il est pose.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Enveloppes {
    poste: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    motdepasse: Option<String>,
}

/// CE QUE LE COFFRE DIT EN CLAIR D'UN SECRET POSE : depuis quand, son libelle, et l'adresse de la boite pour un service
/// qui se pose avec elle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FichePosee {
    pose_le: String,
    libelle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    compte: Option<String>,
}

/// LA SEANCE GARDEE TELLE QU'ELLE SE SCELLE (`garder_ouverture`) : le mot de passe du logiciel, jusqu'a quand, et le
/// coffre qu'il ouvre.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Gardee {
    motdepasse: String,
    fin: String,
    coffre: String,
}

/// CE QUE LE COFFRE DIT D'UN SERVICE (`services`) : s'il est pose, depuis quand, par l'environnement, avec quel
/// compte ; jamais sa valeur.
#[derive(Debug, Clone, Serialize)]
pub struct EtatService {
    pub service: &'static str,
    pub libelle: &'static str,
    pub sert_a: &'static str,
    pub pose: bool,
    pub pose_le: Option<String>,
    pub variable: &'static str,
    pub dans_environnement: bool,
    pub compte: Option<String>,
    pub avec_compte: bool,
}

/// POURQUOI UN SECRET SE LIT (`Coffre::lire`) : une consultation (relever, verifier, envoyer), ou un depot, qu'un
/// humain a confirme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Usage {
    Consultation,
    Depot,
}

impl Usage {
    fn lu(usage: &str) -> R<Usage> {
        match usage {
            CONSULTATION => Ok(Usage::Consultation),
            DEPOT => Ok(Usage::Depot),
            _ => Err(refus(format!(
                "usage doit valoir {} ou {}. Lire un secret sans dire pourquoi n'est pas permis.",
                repr_chaine(CONSULTATION),
                repr_chaine(DEPOT)
            ))),
        }
    }
}

// ------------------------------------------------------------------ les refus

/// Un refus : sa classe et sa phrase.
#[derive(Debug, Clone)]
pub struct Echec {
    pub classe: Classe,
    pub message: String,
}

pub type R<T> = Result<T, Echec>;

/// Le refus du coffre tel qu'un geste le rend (`socle::echec::Echec`) : sa classe et sa phrase.
pub fn vers_route(x: Echec) -> socle::echec::Echec {
    socle::echec::Echec { classe: Some(x.classe), message: x.message }
}

fn echec(classe: Classe, message: impl Into<String>) -> Echec {
    Echec { classe, message: message.into() }
}

/// `Coffre` : le coffre refuse, la phrase est ecrite pour etre lue.
fn refus(message: impl Into<String>) -> Echec {
    echec(Classe::Coffre, message)
}

/// Le refus du coffre, son chemin a part.
fn refus_a(message: impl Into<String>, p: &Path) -> Echec {
    refus(socle::echec::avec(message, socle::echec::APart { chemin: Some(affiche(p)), ..socle::echec::APart::default() }))
}

fn oserr(e: &std::io::Error, p: &Path) -> Echec {
    echec(Classe::Disque, socle::fichiers::panne_fichier(e, p))
}

// ------------------------------------------------- les lectures de valeurs

fn affiche(p: &Path) -> String {
    p.display().to_string()
}

/// Le chemin tel que `pathlib` l'ecrit : sans `.` ni separateur double.
fn normal(p: PathBuf) -> PathBuf {
    let mut sortie = PathBuf::new();
    for c in p.components() {
        if c != Component::CurDir {
            sortie.push(c.as_os_str());
        }
    }
    if sortie.as_os_str().is_empty() {
        sortie.push(".");
    }
    sortie
}

/// Le chemin absolu, liens suivis aussi loin que le chemin existe.
fn resoudre(p: &Path) -> R<PathBuf> {
    let absolu = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().map_err(|e| oserr(&e, p))?.join(p) };
    let mut reste: Vec<std::ffi::OsString> = Vec::new();
    let mut tete = absolu;
    loop {
        if let Ok(c) = std::fs::canonicalize(&tete) {
            let mut sortie = c;
            for r in reste.iter().rev() {
                if r == ".." {
                    sortie.pop();
                } else if r != "." {
                    sortie.push(r);
                }
            }
            let t = sortie.to_string_lossy().to_string();
            return Ok(match t.strip_prefix(r"\\?\") {
                Some(s) if cfg!(windows) => PathBuf::from(s),
                _ => sortie,
            });
        }
        match (tete.file_name().map(|n| n.to_os_string()), tete.parent().map(Path::to_path_buf)) {
            (Some(n), Some(parent)) => {
                reste.push(n);
                tete = parent;
            }
            _ => return Ok(tete),
        }
    }
}

use socle::texte::blanc as espace;

fn strip(s: &str) -> &str {
    s.trim_matches(espace)
}

use socle::fichiers::imprimable;

/// Une chaine entre apostrophes, echappee, telle qu'une phrase de refus la cite.
fn repr_chaine(s: &str) -> String {
    let guillemet = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut t = String::new();
    t.push(guillemet);
    for c in s.chars() {
        match c {
            '\\' => t.push_str("\\\\"),
            '\n' => t.push_str("\\n"),
            '\r' => t.push_str("\\r"),
            '\t' => t.push_str("\\t"),
            c if c == guillemet => {
                t.push('\\');
                t.push(c);
            }
            c if !imprimable(c) => {
                let n = c as u32;
                if n < 0x100 {
                    t.push_str(&format!("\\x{:02x}", n));
                } else if n < 0x10000 {
                    t.push_str(&format!("\\u{:04x}", n));
                } else {
                    t.push_str(&format!("\\U{:08x}", n));
                }
            }
            c => t.push(c),
        }
    }
    t.push(guillemet);
    t
}

/// Le base64 lu avec indulgence : ce qui n'est pas de l'alphabet est
/// ignore, le remplissage s'arrete la ou il est complet.
pub fn d64(t: &str) -> R<Vec<u8>> {
    if !t.is_ascii() {
        return Err(echec(Classe::Illisible, "du base64 ne porte que des caracteres ASCII"));
    }
    let (mut sortie, mut quad, mut reste, mut bourrage) = (Vec::new(), 0u8, 0u8, 0u8);
    for c in t.bytes() {
        if c == b'=' {
            if quad >= 2 {
                bourrage += 1;
                if quad + bourrage >= 4 {
                    return Ok(sortie);
                }
            }
            continue;
        }
        let Some(x) = ALPHABET.iter().position(|a| *a == c) else { continue };
        let x = x as u8;
        bourrage = 0;
        match quad {
            0 => {
                quad = 1;
                reste = x;
            }
            1 => {
                quad = 2;
                sortie.push(reste << 2 | x >> 4);
                reste = x & 0x0f;
            }
            2 => {
                quad = 3;
                sortie.push(reste << 4 | x >> 2);
                reste = x & 0x03;
            }
            _ => {
                quad = 0;
                sortie.push(reste << 6 | x);
                reste = 0;
            }
        }
    }
    match quad {
        0 => Ok(sortie),
        1 => Err(echec(
            Classe::Illisible,
            format!(
                "base64 illisible : {} caracteres utiles, un de plus qu'un multiple de 4",
                sortie.len() / 3 * 4 + 1
            ),
        )),
        _ => Err(echec(Classe::Illisible, "base64 illisible : le remplissage est incomplet")),
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64(o: &[u8]) -> String {
    base64_ecrire(o)
}

fn existe(p: &Path) -> R<bool> {
    match std::fs::metadata(p) {
        Ok(_) => Ok(true),
        Err(e) => match e.raw_os_error() {
            Some(2 | 20 | 9 | 40) => Ok(false),
            _ if e.kind() == std::io::ErrorKind::NotFound || cfg!(windows) => Ok(false),
            _ => Err(oserr(&e, p)),
        },
    }
}

fn lire_octets(p: &Path) -> R<Vec<u8>> {
    std::fs::read(p).map_err(|e| oserr(&e, p))
}

/// La cle du poste telle qu'elle est posee. Une cle plus courte que 32 octets peut etre celle qu'une autre commande
/// est en train d'ecrire (creee, pas encore remplie : reste 158) : on la relit un instant avant de la dire abimee.
fn lire_cle_posee(p: &Path) -> R<Vec<u8>> {
    let mut cle = lire_octets(p)?;
    for _ in 0..50 {
        if cle.len() >= 32 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cle = lire_octets(p)?;
    }
    Ok(cle)
}

fn effacer(p: &Path) -> R<()> {
    match std::fs::remove_file(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(oserr(&e, p)),
    }
}

fn dossier_de(p: &Path) -> R<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| oserr(&e, d))?;
    }
    Ok(())
}

#[cfg(unix)]
fn permissions(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn permissions(_: &Path, _: u32) {}

fn aleas(n: usize) -> R<Vec<u8>> {
    let mut o = vec![0u8; n];
    getrandom::fill(&mut o).map_err(|e| echec(Classe::Disque, format!("pas d'alea du systeme : {}", e)))?;
    Ok(o)
}

// ---------------------------------------------------------------- le scellage

fn boite(cle: &[u8]) -> R<Aes256Gcm> {
    Aes256Gcm::new_from_slice(cle).map_err(|_| echec(Classe::Illisible, "une cle AES-GCM fait 32 octets"))
}

/// nonce (12 octets) suivi du scelle.
pub fn sceller(cle: &[u8], clair: &[u8], aad: &[u8]) -> R<Vec<u8>> {
    let b = boite(cle)?;
    let nonce = aleas(12)?;
    let mut sortie = nonce.clone();
    let scelle = b
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: clair, aad })
        .map_err(|_| echec(Classe::Illisible, "chiffrement impossible"))?;
    sortie.extend(scelle);
    Ok(sortie)
}

pub fn desceller(cle: &[u8], scelle: &[u8], aad: &[u8]) -> R<Vec<u8>> {
    let b = boite(cle)?;
    if scelle.len() < 13 {
        return Err(refus(format!("Scelle tronque : {} octets.", scelle.len())));
    }
    b.decrypt(Nonce::from_slice(&scelle[..12]), Payload { msg: &scelle[12..], aad }).map_err(|_| {
        refus(
            "Le scelle refuse de s'ouvrir : ce n'est pas la bonne cle, ou le \
             contenu a ete modifie.",
        )
    })
}

/// Ce que les secrets scelles portent avec eux : les fiches, ecrites serrees et triees (le scelle refuse des fiches
/// changees).
fn aad_fiches(fiches: &BTreeMap<String, FichePosee>) -> Vec<u8> {
    dumps(fiches, None, true, true).into_bytes()
}

/// La cle scrypt de 32 octets. Les parametres sont ceux du fichier : ils sont
/// verifies avant la derivation, memoire comprise (`SCRYPT_MAXMEM`).
pub fn deriver(motdepasse: &str, kdf: &Kdf) -> R<Vec<u8>> {
    // Le fil de fond de la fenetre ne dechiffre rien : aucun mot de passe ne s'y derive.
    if socle::fil_de_fond::ici() {
        return Err(refus(socle::fil_de_fond::refus("le coffre")));
    }
    if kdf.algo != "scrypt" {
        return Err(refus(format!("Ce coffre dit avoir ete ferme par {}, que cette version ne sait pas ouvrir.", repr_chaine(&kdf.algo))));
    }
    let sel = d64(&kdf.sel)?;
    scrypt_32(motdepasse.as_bytes(), &sel, i128::from(kdf.n), i128::from(kdf.r), i128::from(kdf.p))
}

fn scrypt_32(motdepasse: &[u8], sel: &[u8], n: i128, r: i128, p: i128) -> R<Vec<u8>> {
    let invalide = || echec(Classe::Illisible, "parametres scrypt incompatibles (n, r, p)");
    let memoire_max = || echec(Classe::Illisible, "parametres scrypt : la memoire demandee depasse la borne");
    if n < 2 || n & (n - 1) != 0 {
        return Err(echec(Classe::Illisible, "le parametre scrypt n est une puissance de 2"));
    }
    if r == 0 || p == 0 {
        return Err(invalide());
    }
    if r > u32::MAX as i128 || p > u32::MAX as i128 {
        return Err(echec(Classe::Illisible, "parametres scrypt : r ou p trop grand"));
    }
    if p > ((1i128 << 30) - 1) / r {
        return Err(memoire_max());
    }
    let log_n = n.trailing_zeros() as i128;
    if 16 * r <= 63 && n >= 1i128 << (16 * r) {
        return Err(memoire_max());
    }
    let memoire = 128 * p * r + 128 * r * (n + 2);
    if memoire > SCRYPT_MAXMEM as i128 || 128 * p * r > i32::MAX as i128 {
        return Err(memoire_max());
    }
    let params = scrypt::Params::new(log_n as u8, r as u32, p as u32, 32).map_err(|_| invalide())?;
    let mut sortie = vec![0u8; 32];
    scrypt::scrypt(motdepasse, sel, &params, &mut sortie).map_err(|_| invalide())?;
    Ok(sortie)
}

// ------------------------------------------------------------- le poste et ses chemins

// LE MOT DE PASSE TAPE OUVRE TOUT.
// La ou la cle du poste n'ouvre pas le coffre -- un autre compte, un AppData virtuel --, le mot de passe
// du logiciel tape par un humain pour CETTE commande l'ouvre, pour tout ce qu'elle lit : consultation, envois,
// verification des comptes, releves, depots. Il arrive par la demande (`motdepasse` d'une route) ou par
// `PUPPETPAIE_MOTDEPASSE`, la variable que les fenetres posent apres l'avoir fait taper. Il ne change rien au poste
// et ne s'ecrit nulle part. Par fil, et le temps d'une route : la fenetre sert ses routes dans son
// processus, une par fil a la fois, et `routes::commande` remet en sortant ce qu'il a trouve, quoi qu'il arrive ;
// les tests, chacun sur son fil, ne se le transmettent pas.
thread_local! {
    static TAPE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Retient, pour cette commande, le mot de passe du logiciel qu'un humain vient de taper.
pub fn retenir_mot_de_passe_tape(motdepasse: &str) {
    if !motdepasse.is_empty() {
        TAPE.with(|t| *t.borrow_mut() = Some(motdepasse.to_string()));
    }
}

/// Oublie le mot de passe retenu pour ce fil (la fin d'une commande, ou d'un test).
pub fn oublier_mot_de_passe_tape() {
    TAPE.with(|t| *t.borrow_mut() = None);
}

/// Le mot de passe du logiciel qu'un humain a tape pour cette route (`demande::autoriser_acte` le retient), et rien
/// d'autre : ni la variable, ni la seance. Le geste qui a besoin de l'actuel (le changer, le retirer) le prend ici quand la
/// charge ne le porte pas.
pub fn mot_de_passe_retenu() -> Option<String> {
    TAPE.with(|t| t.borrow().clone())
}

/// Le mot de passe tape pour cette commande : celui de la demande, sinon `PUPPETPAIE_MOTDEPASSE`.
fn mot_de_passe_tape() -> Option<String> {
    TAPE.with(|t| t.borrow().clone()).or_else(|| env_non_vide("PUPPETPAIE_MOTDEPASSE"))
}

/// Le marqueur d'un dossier PORTABLE, a cote de `logiciel/` :
/// le logiciel, ses donnees et la cle qui les scelle voyagent ensemble -- une cle USB -- et s'ouvrent sur
/// n'importe quel ordinateur, quel que soit le systeme. La cle du poste vit alors dans ce dossier, pas sur la machine.
pub const MARQUEUR_PORTABLE: &str = "PORTABLE.json";
/// Ce que porte le marqueur : sa presence seule compte, la phrase le dit a qui ouvre le fichier. L'archive d'installation
/// de tous les systemes le porte (`moteur::publication`, l'archive d'installation), la route `portable-completer` le pose ;
/// l'installeur Windows ecrit la meme phrase (`atelier/installeur/puppetpaie.nsi`).
pub const CONTENU_PORTABLE: &str = "{\"_quoi\": \"Ce dossier est une installation PORTABLE de PuppetPaie : le logiciel, ses donnees et la cle qui les scelle (cle-poste.bin) vivent ici, ensemble, et s'ouvrent sur n'importe quel ordinateur. Effacer ce fichier rattache le coffre a la machine.\"}\n";

/// Le dossier portable, s'il y en a un : ce qui est a cote de `logiciel/` quand il porte `PORTABLE.json`.
pub fn dossier_portable() -> Option<PathBuf> {
    let base = crate::installation::base();
    base.join(MARQUEUR_PORTABLE).is_file().then_some(base)
}

/// L'identifiant du paquet de la fenetre (`identifier` de `fenetre/tauri.conf.json`) : il nomme son dossier sous macOS.
pub const IDENTIFIANT_DU_PAQUET: &str = "fr.puppetpaie.fenetre";

/// Ou vit la cle du poste : `PUPPETPAIE_CLE` la deplace ; sinon le dossier portable (`dossier_portable`), s'il y en a
/// un ; sinon la machine, `%APPDATA%\PuppetPaie` (Windows), `~/Library/Application Support/fr.puppetpaie.fenetre`
/// (macOS) ou `$XDG_CONFIG_HOME/puppetpaie` (`~/.config/puppetpaie`).
/// Le dossier de la personne : `HOME` (vide, la racine), sinon `USERPROFILE` sous Windows, sinon ce que le systeme en dit,
/// sinon la racine.
pub fn maison_de(env: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    let dit = match env("HOME") {
        Some(h) if h.is_empty() => Some("/".to_string()),
        Some(h) => Some(h),
        None if cfg!(windows) => env("USERPROFILE"),
        None => None,
    };
    #[allow(deprecated)]
    match (dit, std::env::home_dir()) {
        (Some(h), _) => PathBuf::from(h),
        (None, Some(h)) => h,
        (None, None) => PathBuf::from("/"),
    }
}

/// La variable `nom` telle que le moteur la lit pour situer un dossier de la personne : `HOME` telle quelle (vide, elle
/// dit la racine), les autres si elles disent quelque chose.
pub fn env_du_poste(nom: &str) -> Option<String> {
    if nom == "HOME" {
        env_tel_quel(nom)
    } else {
        env_non_vide(nom)
    }
}

pub fn dossier_cle_de(env: &dyn Fn(&str) -> Option<String>, portable: Option<&Path>) -> PathBuf {
    let maison = || maison_de(env);
    if let Some(force) = env("PUPPETPAIE_CLE") {
        let p = match force.strip_prefix('~') {
            Some(suite) if suite.is_empty() || suite.starts_with('/') || (cfg!(windows) && suite.starts_with('\\')) => {
                PathBuf::from(format!("{}{}", affiche(&maison()), suite))
            }
            _ => PathBuf::from(force),
        };
        return normal(p);
    }
    if let Some(dossier) = portable {
        return normal(dossier.to_path_buf());
    }
    // LE DOSSIER DE CONFIGURATION DU SYSTEME, et quand la variable ne le dit pas, celui que le systeme lui donne par
    // defaut (`atelier/reference/sources/dossiers-de-configuration/`) :
    // - Windows, `APPDATA` : FOLDERID_RoamingAppData, « Default Path %APPDATA% (%USERPROFILE%\AppData\Roaming) »
    //   (Microsoft, KNOWNFOLDERID) ;
    // - macOS, `~/Library/Application Support/<identifiant du paquet>` : « Contains all app-specific data and support
    //   files » ; « all of these items should be put in a subdirectory whose name matches the bundle identifier of the
    //   app » (Apple, macOS Library Directory Details) ;
    // - ailleurs, `XDG_CONFIG_HOME` : « If $XDG_CONFIG_HOME is either not set or empty, a default equal to $HOME/.config
    //   should be used » ; un chemin relatif ne vaut rien (« If an implementation encounters a relative path in any of
    //   these variables it should consider the path invalid and ignore it ») (XDG Base Directory Specification 0.8).
    if cfg!(windows) {
        let base = match env("APPDATA") {
            Some(b) if !b.is_empty() => PathBuf::from(b),
            Some(_) | None => maison().join("AppData").join("Roaming"),
        };
        return normal(base.join("PuppetPaie"));
    }
    if cfg!(target_os = "macos") {
        return normal(maison().join("Library").join("Application Support").join(IDENTIFIANT_DU_PAQUET));
    }
    let base = match env("XDG_CONFIG_HOME") {
        Some(b) if Path::new(&b).is_absolute() => PathBuf::from(b),
        Some(_) | None => maison().join(".config"),
    };
    normal(base.join("puppetpaie"))
}

/// La variable `nom` telle que l'environnement la porte, si elle est posee et se lit.
fn env_tel_quel(nom: &str) -> Option<String> {
    match std::env::var(nom) {
        Ok(v) => Some(v),
        Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(_)) => None,
    }
}

/// La variable `nom`, si elle dit quelque chose.
fn env_non_vide(nom: &str) -> Option<String> {
    env_tel_quel(nom).filter(|v| !v.is_empty())
}

/// Le coffre d'une compagnie, vu depuis un poste.
pub struct Coffre {
    pub racine: PathBuf,
    pub dossier_cle: PathBuf,
    /// L'heure locale, `AAAA-MM-JJTHH:MM` : donnee par l'appelant.
    pub maintenant: String,
    /// Les variables d'environnement des services (elles passent devant le coffre).
    pub variables: BTreeMap<String, String>,
}

impl Coffre {
    /// Le coffre de `racine` avec l'environnement de ce processus.
    pub fn du_poste(racine: impl Into<PathBuf>, maintenant: &str) -> Coffre {
        Coffre {
            racine: racine.into(),
            dossier_cle: Coffre::dossier_cle_du_poste(),
            maintenant: maintenant.to_string(),
            variables: SERVICES.iter().filter_map(|s| env_non_vide(s.variable).map(|v| (s.variable.to_string(), v))).collect(),
        }
    }

    /// Le coffre de `racine` avec la cle du poste dans `dossier_cle`, sans variable de service : celui que l'administrateur
    /// rescelle (`administrateur.rs`), et que ses essais montent sans toucher a l'environnement du processus.
    pub fn sur(racine: impl Into<PathBuf>, dossier_cle: impl Into<PathBuf>, maintenant: &str) -> Coffre {
        Coffre { racine: racine.into(), dossier_cle: dossier_cle.into(), maintenant: maintenant.to_string(), variables: BTreeMap::new() }
    }

    /// Le dossier de la cle de CE poste, tel que `du_poste` le trouve.
    pub fn dossier_cle_du_poste() -> PathBuf {
        dossier_cle_de(&env_du_poste, dossier_portable().as_deref())
    }

    /// L'heure locale `AAAA-MM-JJTHH:MM` : celle qu'on a donnee, sinon l'horloge. Les lecteurs du moteur font leur
    /// coffre sans heure (`du_poste(racine, "")`) et la seance, elle, expire : une heure vide ne doit pas la rendre eternelle.
    fn heure(&self) -> String {
        if !self.maintenant.is_empty() {
            return self.maintenant.clone();
        }
        socle::paris::iso_de_naif(socle::paris::maintenant_naif())[..16].to_string()
    }

    pub fn chemin(&self) -> PathBuf {
        self.racine.join(socle::fichiers::CONFIG).join(FICHIER)
    }

    pub fn chemin_cle(&self) -> PathBuf {
        self.dossier_cle.join(FICHIER_CLE)
    }

    /// LA SEANCE GARDEE DE CE COFFRE (`ouverture-gardee-<empreinte>.bin`, a cote de la cle du poste). Une par coffre :
    /// la cle du poste sert a toutes les compagnies et a toutes les installations d'une machine ; avec une seule
    /// seance pour toutes, celle qu'on ouvrait pour l'une effacait celle de l'autre. Le nom porte l'empreinte du chemin du
    /// coffre : deux coffres ne se marchent jamais dessus ; la seance garde en plus ce chemin, et ne sert qu'a lui.
    pub fn chemin_gardee(&self) -> R<PathBuf> {
        use sha2::{Digest, Sha256};
        let h = Sha256::digest(affiche(&resoudre(&self.chemin())?).as_bytes());
        let court: String = h.iter().take(6).map(|o| format!("{:02x}", o)).collect();
        Ok(self.dossier_cle.join(format!("{}-{}.bin", GARDEE.trim_end_matches(".bin"), court)))
    }

    pub fn cle_posee(&self) -> R<bool> {
        existe(&self.chemin_cle())
    }

    /// Les 32 octets de ce poste, ou None. `creer` en tire de neufs.
    fn cle_poste(&self, creer: bool) -> R<Option<Vec<u8>>> {
        // Le fil de fond de la fenetre ne dechiffre rien : la cle du poste ne s'y sert pas.
        if socle::fil_de_fond::ici() {
            return Err(refus(socle::fil_de_fond::refus("le coffre")));
        }
        let p = self.chemin_cle();
        if existe(&p)? {
            let cle = lire_cle_posee(&p)?;
            if cle.len() != 32 {
                return Err(refus(format!(
                    "La cle de ce poste ({}) fait {} octets au lieu de 32 : elle \
                     est abimee. La supprimer, puis rouvrir le coffre avec le mot \
                     de passe du logiciel -- une cle neuve sera posee.",
                    affiche(&p),
                    cle.len()
                )));
            }
            return Ok(Some(cle));
        }
        if !creer {
            return Ok(None);
        }
        dossier_de(&p)?;
        if let Some(d) = p.parent() {
            permissions(d, 0o700);
        }
        let cle = aleas(32)?;
        // 0600 des l'ouverture, et O_EXCL : pas d'instant ou la cle se lit.
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = match options.open(&p) {
            Ok(f) => f,
            // Une autre commande l'a posee entre `existe` et ici : c'est la sienne qui vaut, sinon le
            // coffre se scellerait avec deux cles. On la relit, elle et elle seule.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return self.cle_poste(false),
            Err(e) => return Err(oserr(&e, &p)),
        };
        f.write_all(&cle).map_err(|e| oserr(&e, &p))?;
        Ok(Some(cle))
    }

    fn cle_ou_creer(&self) -> R<Vec<u8>> {
        self.cle_poste(true)?.ok_or_else(|| echec(Classe::Disque, "la cle de ce poste n'a pas pu etre posee"))
    }

    // -------------------------------------------- proteger / deproteger

    pub fn proteger(&self, donnees: &[u8], description: &str) -> R<Vec<u8>> {
        sceller(&self.cle_ou_creer()?, donnees, description.as_bytes())
    }

    pub fn deproteger(&self, donnees: &[u8], description: &str) -> R<Vec<u8>> {
        let Some(cle) = self.cle_poste(false)? else {
            return Err(refus(format!(
                "La cle de ce poste est absente ({}) : ce fichier a ete protege \
                 sur une autre machine, ou la cle a ete perdue. Il faut reposer ce \
                 qu'il contenait.",
                affiche(&self.chemin_cle())
            )));
        };
        desceller(&cle, donnees, description.as_bytes())
    }

    // ------------------------------------------------------------ le fichier

    /// Le coffre tel qu'il est sur le disque ; `None` s'il n'y en a pas. Un coffre d'une autre forme ou d'une autre
    /// version se refuse : il ne se convertit pas.
    fn lire_fichier(&self) -> R<Option<Contenu>> {
        let p = self.chemin();
        if !existe(&p)? {
            return Ok(None);
        }
        let brut = lire_octets(&p)?;
        let lu = match std::str::from_utf8(&brut) {
            Ok(t) => socle::forme::lire_texte::<Contenu>(t, "Le coffre ne se lit pas.").map_err(|m| socle::echec::phrase_de(&m)),
            Err(e) => Err(e.to_string()),
        };
        match lu {
            Ok(c) if c.version == VERSION => Ok(Some(c)),
            Ok(c) => Err(self.refus_de_format(format!("version {}", c.version))),
            Err(detail) => Err(self.refus_de_format(detail)),
        }
    }

    /// Le refus d'un coffre qui n'est pas au format de cette version : ce qu'il faut reposer ; le chemin et ce qui ne va
    /// pas a part.
    fn refus_de_format(&self, detail: String) -> Echec {
        refus(socle::echec::avec(
            "Le coffre n'est pas au format de cette version -- il est plus \
             ancien, ou abime. Il ne se convertit pas : le supprimer, avec \
             le jeton Google et la cle de signature qui sont fermes de la \
             meme facon, puis reposer ce qu'ils contenaient :\n\
             \x20 route mot-de-passe-changer\n\
             \x20 route coffre-poser, service net-entreprises\n\
             \x20 route coffre-poser, service urssaf\n\
             \x20 route coffre-poser, service courriel, compte <adresse>\n\
             Puis relier le Drive, et refaire une cle de signature si ce poste \
             publie les mises a jour.",
            socle::echec::APart { chemin: Some(affiche(&self.chemin())), detail: Some(detail), ..socle::echec::APart::default() },
        ))
    }

    /// Un coffre neuf : (cle maitresse, le contenu a ecrire). L'enveloppe du
    /// poste est posee tout de suite ; celle du mot de passe vient avec lui.
    fn creer(&self) -> R<(Vec<u8>, Contenu)> {
        let maitresse = aleas(32)?;
        let kdf = Kdf { algo: "scrypt".into(), n: SCRYPT_N, r: SCRYPT_R, p: SCRYPT_P, sel: b64(&aleas(16)?) };
        let enveloppes = Enveloppes { poste: b64(&sceller(&self.cle_ou_creer()?, &maitresse, AAD_POSTE)?), motdepasse: None };
        Ok((maitresse, Contenu { version: VERSION, kdf, enveloppes, fiches: BTreeMap::new(), secrets: None, logiciel_pose_le: None }))
    }

    /// La cle maitresse, ou None si rien ne l'ouvre ici : la cle du poste
    /// d'abord, le mot de passe du logiciel ensuite -- celui qu'on donne, sinon
    /// celui qu'un humain a tape pour cette commande (`mot_de_passe_tape`).
    ///
    /// LA SEANCE EN TROISIEME : si ni la cle du poste ni un mot de passe donne ou tape n'ouvrent, le mot de passe
    /// GARDE par `garder_ouverture` (12 h) est essaye. Il ouvre le coffre ; il n'AUTORISE aucun acte reel (cela, seule
    /// `demande::autoriser_acte` le fait).
    fn ouvrir(&self, contenu: &Contenu, motdepasse: Option<&str>) -> R<Option<Vec<u8>>> {
        let tape = mot_de_passe_tape();
        let donne = motdepasse.filter(|m| !m.is_empty()).or(tape.as_deref());
        if let Some(m) = self.ouvrir_avec(contenu, donne)? {
            return Ok(Some(m));
        }
        if donne.is_some() {
            // Un mot de passe donne qui n'ouvre pas : la seance ne le repare pas, et ne le contredit pas.
            return Ok(None);
        }
        match self.ouverture_gardee()? {
            Some(garde) => self.ouvrir_avec(contenu, Some(&garde)),
            None => Ok(None),
        }
    }

    fn ouvrir_avec(&self, contenu: &Contenu, motdepasse: Option<&str>) -> R<Option<Vec<u8>>> {
        if let Some(cle) = self.cle_poste(false)? {
            match desceller(&cle, &d64(&contenu.enveloppes.poste)?, AAD_POSTE) {
                Ok(m) => return Ok(Some(m)),
                Err(x) if x.classe == Classe::Coffre => {}
                Err(x) => return Err(x),
            }
        }
        if let (Some(mdp), Some(e)) = (motdepasse.filter(|m| !m.is_empty()), &contenu.enveloppes.motdepasse) {
            let r = deriver(mdp, &contenu.kdf).and_then(|k| desceller(&k, &d64(e)?, AAD_MOTDEPASSE));
            return match r {
                Ok(m) => Ok(Some(m)),
                Err(x) if x.classe == Classe::Coffre => Ok(None),
                Err(x) => Err(x),
            };
        }
        Ok(None)
    }

    fn ouvrir_ou_lever(&self, motdepasse: Option<&str>) -> R<(Vec<u8>, Contenu)> {
        let Some(contenu) = self.lire_fichier()? else {
            return self.creer();
        };
        match self.ouvrir(&contenu, motdepasse)? {
            Some(m) => Ok((m, contenu)),
            None => Err(self.refus_ferme()?),
        }
    }

    /// Le refus d'un coffre que rien n'ouvre ici.
    fn refus_ferme(&self) -> R<Echec> {
        let absente = if self.cle_posee()? { "" } else { ", et elle est absente" };
        Ok(refus_a(
            format!(
                "Ce coffre ne s'ouvre pas ici : la cle de ce poste n'est pas \
                 celle qui l'a ferme{}. Le mot de passe du logiciel l'ouvre -- et \
                 reposera une cle pour ce poste :\n\
                 \x20 route mot-de-passe-changer",
                absente
            ),
            &self.chemin_cle(),
        ))
    }

    /// La cle de CE poste ouvre-t-elle le coffre qui est la ? Vrai aussi sans
    /// coffre : il n'y a rien qui resiste.
    pub fn ouvre_a_ce_poste(&self) -> R<bool> {
        match self.lire_fichier()? {
            Some(contenu) => Ok(self.ouvrir_avec(&contenu, None)?.is_some()),
            None => Ok(true),
        }
    }

    /// Les secrets du coffre, descelles : aucun dans un coffre neuf.
    fn secrets(&self, contenu: &Contenu, maitresse: &[u8]) -> R<BTreeMap<String, String>> {
        let Some(scelles) = &contenu.secrets else { return Ok(BTreeMap::new()) };
        let clair = desceller(maitresse, &d64(scelles)?, &aad_fiches(&contenu.fiches))?;
        let t = String::from_utf8(clair).map_err(|_| echec(Classe::Illisible, "Les secrets du coffre ne se lisent pas."))?;
        socle::forme::lire_texte(&t, "Les secrets du coffre ne se lisent pas.").map_err(|m| echec(Classe::Illisible, m))
    }

    fn poste_ouvre(&self, enveloppes: &Enveloppes, maitresse: &[u8]) -> R<bool> {
        let Some(cle) = self.cle_poste(false)? else { return Ok(false) };
        match desceller(&cle, &d64(&enveloppes.poste)?, AAD_POSTE) {
            Ok(m) => Ok(m == maitresse),
            Err(x) if x.classe == Classe::Coffre => Ok(false),
            Err(x) => Err(x),
        }
    }

    /// Reecrit le coffre : les fiches en clair, les secrets scelles avec elles.
    fn enregistrer(&self, contenu: Contenu, maitresse: &[u8], secrets: &BTreeMap<String, String>) -> R<PathBuf> {
        let octets = self.octets_a_ecrire(contenu, maitresse, secrets)?;
        self.ecrire_octets(&octets)
    }

    /// Ecrit `octets` a la place du coffre (0600).
    fn ecrire_octets(&self, octets: &[u8]) -> R<PathBuf> {
        let p = self.chemin();
        dossier_de(&p)?;
        std::fs::write(&p, octets).map_err(|e| oserr(&e, &p))?;
        permissions(&p, 0o600);
        Ok(p)
    }

    /// Ce que `enregistrer` ecrirait, sans l'ecrire.
    fn octets_a_ecrire(&self, contenu: Contenu, maitresse: &[u8], secrets: &BTreeMap<String, String>) -> R<Vec<u8>> {
        let mut c = contenu;
        c.version = VERSION;
        let clair = dumps(secrets, None, false, true);
        c.secrets = Some(b64(&sceller(maitresse, clair.as_bytes(), &aad_fiches(&c.fiches))?));
        // L'enveloppe du poste est refaite si elle ne vaut plus ici :
        // une sauvegarde restauree, ouverte par le mot de passe, rend son
        // autonomie a la machine.
        if !self.poste_ouvre(&c.enveloppes, maitresse)? {
            c.enveloppes.poste = b64(&sceller(&self.cle_ou_creer()?, maitresse, AAD_POSTE)?);
        }
        Ok(dumps(&c, Some(2), false, true).into_bytes())
    }

    // ------------------------------------------------------------- l'usage

    pub fn poser(&self, service: &str, motdepasse: &str, compte: Option<&str>) -> R<PathBuf> {
        let fiche = service_connu(service)?;
        if strip(motdepasse).is_empty() {
            return Err(refus("Mot de passe vide : rien n'a ete pose."));
        }
        let longueur = motdepasse.chars().count();
        if longueur < LONGUEUR_MINIMALE {
            return Err(refus(format!(
                "Mot de passe de {} caractere(s) : c'est trop court pour un compte \
                 d'organisme, et c'est presque toujours le signe que la saisie \
                 masquee n'a pas tout capture. Rien n'a ete pose.\n\
                 Reessayer dans une fenetre, qui ne depend pas du terminal :\n\
                 \x20 route coffre-poser, service {}",
                longueur, service
            )));
        }
        // L'adresse de la boite, en minuscules ; une adresse qui ne dit rien n'en est pas une.
        let compte = compte.map(|c| strip(c).to_lowercase()).filter(|c| !c.is_empty());
        if fiche.avec_compte {
            let manque = match &compte {
                None => Some("elle manque".to_string()),
                Some(c) if !c.contains('@') || c.contains(' ') => Some(format!("{} n'en est pas une", repr_chaine(c))),
                Some(_) => None,
            };
            if let Some(manque) = manque {
                return Err(refus(format!(
                    "Ce service se pose avec l'adresse de la boite (compte@domaine) : \
                     {}. Rien n'a ete pose.",
                    manque
                )));
            }
        }
        let (maitresse, mut contenu) = self.ouvrir_ou_lever(None)?;
        let mut secrets = self.secrets(&contenu, &maitresse)?;
        secrets.insert(service.into(), motdepasse.into());
        contenu.fiches.insert(service.into(), FichePosee { pose_le: self.pose_le()?, libelle: fiche.libelle.into(), compte });
        self.enregistrer(contenu, &maitresse, &secrets)
    }

    /// `AAAA-MM-JJ HH:MM`.
    fn pose_le(&self) -> R<String> {
        if self.maintenant.len() != 16 {
            return Err(echec(Classe::Illisible, "demande illisible : il faut `maintenant` (AAAA-MM-JJTHH:MM)"));
        }
        Ok(self.maintenant.replacen('T', " ", 1))
    }

    pub fn oublier(&self, service: &str) -> R<bool> {
        service_connu(service)?;
        let Some(contenu) = self.lire_fichier()? else { return Ok(false) };
        if !contenu.fiches.contains_key(service) {
            return Ok(false);
        }
        let (maitresse, mut contenu) = self.ouvrir_ou_lever(None)?;
        let mut secrets = self.secrets(&contenu, &maitresse)?;
        contenu.fiches.remove(service);
        secrets.remove(service);
        if contenu.fiches.is_empty() && contenu.enveloppes.motdepasse.is_none() {
            effacer(&self.chemin())?;
            return Ok(true);
        }
        self.enregistrer(contenu, &maitresse, &secrets)?;
        Ok(true)
    }

    /// Les fiches EN CLAIR. N'ouvre rien, ne derive rien ; un coffre d'un autre format n'en a pas.
    fn fiches(&self) -> R<BTreeMap<String, FichePosee>> {
        match self.lire_fichier() {
            Ok(Some(c)) => Ok(c.fiches),
            Ok(None) => Ok(BTreeMap::new()),
            Err(x) if x.classe == Classe::Coffre => Ok(BTreeMap::new()),
            Err(x) => Err(x),
        }
    }

    pub fn pose(&self, service: &str) -> R<bool> {
        Ok(self.fiches()?.contains_key(service))
    }

    /// Ce qui est pose, depuis quand, pour chaque service : jamais une valeur.
    pub fn services(&self) -> R<Vec<EtatService>> {
        let fiches = self.fiches()?;
        Ok(SERVICES
            .iter()
            .map(|s| {
                let f = fiches.get(s.nom);
                EtatService {
                    service: s.nom,
                    libelle: s.libelle,
                    sert_a: s.sert_a,
                    pose: f.is_some(),
                    pose_le: f.map(|f| f.pose_le.clone()),
                    variable: s.variable,
                    dans_environnement: self.variables.contains_key(s.variable),
                    compte: f.and_then(|f| f.compte.clone()),
                    avec_compte: s.avec_compte,
                }
            })
            .collect())
    }

    /// L'adresse posee avec le secret, s'il y en a une.
    pub fn compte_pose(&self, service: &str) -> R<Option<String>> {
        service_connu(service)?;
        Ok(self.fiches()?.remove(service).and_then(|f| f.compte))
    }

    /// Le mot de passe du service, ou None. L'environnement l'emporte sur le
    /// coffre. `usage` dit pourquoi il se lit : `CONSULTATION` ou `DEPOT`. `ouverture` : le mot de passe du logiciel,
    /// quand l'appelant l'a fait taper ; ce module ne le demande jamais.
    pub fn lire(&self, service: &str, usage: &str, confirme: bool, ouverture: Option<&str>) -> R<Option<String>> {
        let fiche = service_connu(service)?;
        let nom = fiche.nom;
        let usage = Usage::lu(usage)?;
        if usage == Usage::Depot && !confirme {
            return Err(refus(
                "Un depot ne puise dans le coffre qu'apres confirmation d'un \
                 humain. `confirme` doit porter ce qui a ete confirme -- le mois \
                 recopie, ou la reponse de la fenetre.",
            ));
        }
        if let Some(v) = self.variables.get(fiche.variable) {
            return Ok(Some(v.clone()));
        }
        let Some(contenu) = self.lire_fichier()? else { return Ok(None) };
        if !contenu.fiches.contains_key(nom) {
            return Ok(None);
        }
        let Some(maitresse) = self.ouvrir(&contenu, ouverture)? else {
            let absente = if self.cle_posee()? { "" } else { ", et elle est absente" };
            let donne = if ouverture.is_some_and(|o| !o.is_empty()) || mot_de_passe_tape().is_some() { "ne l'ouvre pas non plus" } else { "n'a pas ete donne" };
            return Err(refus_a(
                format!(
                    "Le secret {} est dans le coffre, mais rien ici ne l'ouvre : la \
                     cle de ce poste n'est pas celle qui l'a ferme{}, et le mot \
                     de passe du logiciel {}.\n\
                     Le donner ouvre le coffre sans rien changer au poste ; le \
                     reposer rend a cette machine son ouverture sans mot de passe :\n\
                     \x20 route mot-de-passe-changer",
                    nom, absente, donne
                ),
                &self.chemin_cle(),
            ));
        };
        let mut secrets = self.secrets(&contenu, &maitresse)?;
        Ok(secrets.remove(nom).filter(|s| !s.is_empty()))
    }

    pub fn disponible(&self, service: &str) -> R<bool> {
        let fiche = service_connu(service)?;
        Ok(self.variables.contains_key(fiche.variable) || self.pose(service)?)
    }

    // ------------------------------------------- le mot de passe du logiciel

    /// Pose (ou change) le mot de passe du logiciel. `ancien` sert quand la cle
    /// du poste n'ouvre pas le coffre.
    pub fn poser_ouverture(&self, motdepasse: &str, ancien: Option<&str>) -> R<PathBuf> {
        let octets = self.preparer_ouverture(motdepasse, ancien)?;
        self.ecrire_octets(&octets)
    }

    /// Ce que `poser_ouverture` ecrirait, SANS L'ECRIRE : les octets du coffre rescelle par `motdepasse`. Sert a qui doit
    /// resceller plusieurs coffres d'un coup (l'administrateur, `administrateur.rs`) : tout se prepare, puis tout se remplace.
    pub fn preparer_ouverture(&self, motdepasse: &str, ancien: Option<&str>) -> R<Vec<u8>> {
        if strip(motdepasse).chars().count() < LONGUEUR_MINIMALE_LOGICIEL {
            return Err(refus(format!(
                "Mot de passe du logiciel trop court : {} caracteres au moins. \
                 Il n'est plus seulement un oui ou non depuis le 20/09/2026 -- il \
                 ouvre le coffre sur une machine qui n'a pas sa cle, donc qui \
                 vole le fichier peut l'essayer hors ligne. Rien n'a ete pose.",
                LONGUEUR_MINIMALE_LOGICIEL
            )));
        }
        let (maitresse, mut c) = self.ouvrir_ou_lever(ancien)?;
        c.enveloppes.motdepasse = Some(b64(&sceller(&deriver(motdepasse, &c.kdf)?, &maitresse, AAD_MOTDEPASSE)?));
        c.logiciel_pose_le = Some(self.pose_le()?);
        let secrets = self.secrets(&c, &maitresse)?;
        self.octets_a_ecrire(c, &maitresse, &secrets)
    }

    /// Un mot de passe du logiciel a-t-il ete pose ?
    pub fn ouverture_exigee(&self) -> R<bool> {
        match self.lire_fichier() {
            Ok(c) => Ok(c.is_some_and(|c| c.enveloppes.motdepasse.is_some())),
            Err(x) if x.classe == Classe::Coffre => Ok(false),
            Err(x) => Err(x),
        }
    }

    /// Retire l'enveloppe du mot de passe du logiciel.
    pub fn oublier_ouverture(&self, ancien: Option<&str>) -> R<bool> {
        if !self.lire_fichier()?.is_some_and(|c| c.enveloppes.motdepasse.is_some()) {
            return Ok(false);
        }
        let (maitresse, mut c) = self.ouvrir_ou_lever(ancien)?;
        if c.fiches.is_empty() {
            effacer(&self.chemin())?;
            return Ok(true);
        }
        let secrets = self.secrets(&c, &maitresse)?;
        c.enveloppes.motdepasse = None;
        c.logiciel_pose_le = None;
        self.enregistrer(c, &maitresse, &secrets)?;
        Ok(true)
    }

    /// Le mot de passe du logiciel est-il le bon ? L'enveloppe le dit en
    /// s'ouvrant : GCM ne connait pas le « presque ».
    pub fn verifier_ouverture(&self, motdepasse: &str) -> R<bool> {
        let contenu = match self.lire_fichier() {
            Ok(Some(c)) => c,
            Ok(None) => return Ok(false),
            Err(x) if x.classe == Classe::Coffre => return Ok(false),
            Err(x) => return Err(x),
        };
        let Some(e) = &contenu.enveloppes.motdepasse else { return Ok(false) };
        let r = deriver(motdepasse, &contenu.kdf).and_then(|k| desceller(&k, &d64(e)?, AAD_MOTDEPASSE));
        match r {
            Ok(_) => Ok(true),
            Err(x) if x.classe == Classe::Coffre => Ok(false),
            Err(x) => Err(x),
        }
    }

    // -------------------------------------------- l'ouverture gardee

    /// Garde le mot de passe du logiciel jusqu'a `fin` (`AAAA-MM-JJTHH:MM`).
    /// Refuse un mot de passe qui n'ouvre pas le coffre.
    pub fn garder_ouverture(&self, motdepasse: &str, fin: &str) -> R<String> {
        if !self.verifier_ouverture(motdepasse)? {
            return Err(refus("Mot de passe du logiciel refuse : rien n'est garde."));
        }
        let d = Gardee { motdepasse: motdepasse.into(), fin: fin.into(), coffre: affiche(&resoudre(&self.chemin())?) };
        let clair = dumps(&d, None, false, false);
        let p = self.chemin_gardee()?;
        dossier_de(&p)?;
        std::fs::write(&p, sceller(&self.cle_ou_creer()?, clair.as_bytes(), AAD_GARDEE)?).map_err(|e| oserr(&e, &p))?;
        Ok(fin.to_string())
    }

    /// La gardee dechiffree, s'il y en a une ; `CoffreIllisible` si elle ne s'ouvre pas ou ne se lit pas.
    fn gardee(&self) -> R<Option<Gardee>> {
        let p = self.chemin_gardee()?;
        let Some(cle) = self.cle_poste(false)? else { return Ok(None) };
        if !existe(&p)? {
            return Ok(None);
        }
        let illisible = |detail: String| echec(Classe::CoffreIllisible, socle::echec::avec("La seance gardee ne se lit pas.", socle::echec::APart { detail: Some(detail), ..socle::echec::APart::default() }));
        let clair = match lire_octets(&p).and_then(|o| desceller(&cle, &o, AAD_GARDEE)) {
            Ok(c) => c,
            Err(x) if x.classe == Classe::Coffre => return Err(illisible(x.message)),
            Err(x) => return Err(x),
        };
        let texte = String::from_utf8(clair).map_err(|e| illisible(e.to_string()))?;
        socle::forme::lire_texte(&texte, "La seance gardee ne se lit pas.").map(Some).map_err(illisible)
    }

    /// Le mot de passe garde, s'il y en a un, pas expire, pour CE coffre, et
    /// qu'il l'ouvre encore. Une gardee expiree ou perimee s'efface.
    pub fn ouverture_gardee(&self) -> R<Option<String>> {
        let d = match self.gardee() {
            Ok(Some(d)) => d,
            Ok(None) => return Ok(None),
            Err(x) if x.classe == Classe::CoffreIllisible => {
                self.oublier_ouverture_gardee()?;
                return Ok(None);
            }
            Err(x) => return Err(x),
        };
        if d.fin < self.heure() {
            self.oublier_ouverture_gardee()?;
            return Ok(None);
        }
        if d.coffre != affiche(&resoudre(&self.chemin())?) {
            return Ok(None);
        }
        if !self.verifier_ouverture(&d.motdepasse)? {
            self.oublier_ouverture_gardee()?;
            return Ok(None);
        }
        Ok(Some(d.motdepasse))
    }

    /// L'heure de fin de la gardee en cours, sans rien verifier d'autre.
    pub fn fin_ouverture_gardee(&self) -> R<Option<String>> {
        let d = match self.gardee() {
            Ok(Some(d)) => d,
            Ok(None) => return Ok(None),
            Err(x) if x.classe == Classe::CoffreIllisible => return Ok(None),
            Err(x) => return Err(x),
        };
        Ok(Some(d.fin).filter(|f| *f >= self.heure()))
    }

    pub fn oublier_ouverture_gardee(&self) -> R<bool> {
        let p = self.chemin_gardee()?;
        if existe(&p)? {
            std::fs::remove_file(&p).map_err(|e| oserr(&e, &p))?;
            return Ok(true);
        }
        Ok(false)
    }
}

/// Le service du coffre nomme `service`, ou le refus d'un nom qu'aucun ne porte.
pub fn service_connu(service: &str) -> R<&'static Service> {
    if let Some(s) = SERVICES.iter().find(|s| s.nom == service) {
        return Ok(s);
    }
    let mut noms: Vec<&str> = SERVICES.iter().map(|s| s.nom).collect();
    noms.sort_unstable();
    Err(refus(format!("Service inconnu : {}. Connus : {}.", repr_chaine(service), noms.join(", "))))
}
