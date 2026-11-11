//! La sauvegarde du dossier prive dans le Drive, par differences : ce qu'elle sauve, l'inventaire, l'index, ce qui
//! persiste. Le passage lui-meme (`sauvegarder`, le client Drive du poste) est `gestes::sauvegarde`.
//!
//! Ce sont des FONCTIONS, pas des routes : leurs appelants sont l'ouverture et la veille, la ligne de commande, la
//! restauration, et l'interface qui lit `derniere` et `rien_n_a_bouge`.
//!
//! CE QU'ELLE SAUVE : tout le dossier des donnees (`config/`, `engagements/`, `sorties/`, `retours/`,
//! `gabarits/`, `journal/`, `reference/`), coffre compris (`coffre.bin`, scelle par le mot de passe du logiciel, qui
//! l'ouvre sur un poste neuf apres `moteur restaurer`), sauf le jeton Google
//! (`google-jeton.bin`, le compte se relie), l'etat de la machine (`.compteurs.verrou`, l'index lui-meme) et le rebut.
//!
//! ELLE NE SUPPRIME JAMAIS RIEN dans le Drive : le trait `Drive` n'a d'ailleurs aucune
//! suppression. Un fichier efface ici reste la-bas.
//!
//! COMMENT ELLE SAIT CE QUI A CHANGE : par l'empreinte MD5 que Google publie
//! (`md5Checksum`). L'index local (`config/.sauvegarde.json`) ne sert qu'a repondre SANS RESEAU
//! (`rien_n_a_bouge`) et a garder les identifiants des dossiers Drive ; il n'est jamais cru pour
//! DECIDER d'un televersement.

use std::collections::BTreeMap;
use socle::echec::Classe;
use socle::paris::Naif;
use std::io::Read;
use std::path::{Path, PathBuf};

use md5::{Digest, Md5};

use envois::drive::ErreurGoogle;

pub const RACINE_DRIVE: &str = "PuppetPaie";
pub const DOSSIER_DRIVE: &str = "sauvegarde";
pub const INDEX: &str = ".sauvegarde.json";

/// Ce qui ne monte pas (`NOMS_EXCLUS`).
const NOMS_EXCLUS: [&str; 6] = ["google-jeton.bin", ".jeton-interface", ".compteurs.verrou", ".sauvegarde.json", ".ouverture.json", ".ouverture-etape.json"];
const SUFFIXES_EXCLUS: [&str; 4] = [".pyc", ".tmp", ".lock", ".part"];
const DOSSIERS_EXCLUS: [&str; 2] = ["__pycache__", ".git"];

/// Les signes d'une absence de reseau, SANS `timed out` : un delai de LECTURE depasse (un serveur qui repond trop
/// lentement, un dossier de Drive qui ne repond pas) n'arrete pas TOUTE la sauvegarde (« Hors connexion : sauvegarde au
/// prochain passage ») quand le reseau est la. Un delai depasse est un incident du dossier ou du fichier ; l'absence de reseau reste « injoignable »,
/// `getaddrinfo`, `Network is unreachable`, `Name or service not known`... (un delai de CONNEXION dit « Reseau injoignable :
/// timed out » : il garde son « injoignable »).
const SIGNES_HORS_LIGNE: [&str; 10] = [
    "injoignable",
    "getaddrinfo",
    "errno 11001",
    "errno 11004",
    "winerror 10051",
    "winerror 10060",
    "winerror 10065",
    "network is unreachable",
    "name or service not known",
    "temporary failure in name resolution",
];

/// `hors_ligne` : ce message dit-il que le reseau manque, plutot qu'un vrai refus ?
pub fn hors_ligne(erreur: &str) -> bool {
    let texte = erreur.to_lowercase();
    SIGNES_HORS_LIGNE.iter().any(|s| texte.contains(s))
}

/// L'etat d'un fichier a l'envoi (`Fichier.etat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatFichier {
    Neuf,
    Modifie,
}

impl EtatFichier {
    pub fn texte(self) -> &'static str {
        match self {
            EtatFichier::Neuf => "neuf",
            EtatFichier::Modifie => "modifie",
        }
    }
}

/// Un fichier depose (`Fichier`).
#[derive(Debug, Clone, PartialEq)]
pub struct Fichier {
    pub relatif: String,
    pub octets: u64,
    pub etat: EtatFichier,
}

/// Le compte rendu d'un passage (`Sauvegarde`).
#[derive(Debug, Clone, PartialEq)]
pub struct Sauvegarde {
    pub racine_drive: String,
    pub compte: Option<String>,
    pub deposes: Vec<Fichier>,
    pub identiques: usize,
    pub octets_deposes: u64,
    pub exclus: usize,
    /// Purges entre l'inventaire et l'envoi : pas un incident.
    pub disparus: usize,
    /// La duree du passage, en secondes, au dixieme.
    pub secondes: socle::num::Decimal,
    /// Le detail brut de ce qui n'est pas passe (`dossier : erreur`), pour la ligne de commande. La fenetre ne le montre
    /// pas : un dossier reporte au passage suivant n'est pas un incident.
    pub incidents: Vec<String>,
    /// Ce qui n'a pas pu monter a ce passage, reporte au suivant.
    pub reportes: Vec<Reporte>,
    /// La phrase de ce qui PERSISTE (`souci`), apres ce passage : `None` tant que ce n'est que passager.
    pub persistant: Option<String>,
    /// Pas de reseau : arretee au premier dossier.
    pub hors_ligne: bool,
}

/// Un dossier (tout ce qu'il porte) ou un fichier qui n'a pas pu monter a un passage : il reste hors de l'index, et le
/// passage suivant le reprend.
#[derive(Debug, Clone, PartialEq)]
pub struct Reporte {
    /// Le chemin relatif du dossier (`.` pour la racine) ou du fichier.
    pub quoi: String,
    pub dossier: bool,
    /// La cause, en clair (`cause_de`).
    pub cause: String,
}

impl Reporte {
    /// La cle de l'index (`echecs`) : un dossier finit par `/`.
    pub fn cle(&self) -> String {
        if self.dossier {
            format!("{}/", self.quoi)
        } else {
            self.quoi.clone()
        }
    }
}

/// Les causes, en clair. Le detail brut (« HTTP 500 : Internal Error ») reste dans `incidents`, pour la ligne de commande.
pub const GOOGLE_INDISPONIBLE: &str = "Google indisponible";
const GOOGLE_MUET: &str = "Google ne répond pas";
const GOOGLE_REFUSE: &str = "Google refuse l'envoi";
const FICHIER_ILLISIBLE: &str = "fichier illisible sur ce poste";

/// La cause d'une erreur, en clair : passagere chez Google (`drive::passagere`, deja retentee par le client), un delai
/// depasse, un refus de Google, ou un fichier du poste qui ne se lit pas.
pub fn cause_de(e: &ErreurGoogle) -> &'static str {
    if envois::drive::passagere(e) {
        GOOGLE_INDISPONIBLE
    } else if e.echec.classe == Some(Classe::DelaiDepasse) || e.echec.phrase.to_lowercase().contains("timed out") {
        GOOGLE_MUET
    } else if e.echec.classe == Some(Classe::Disque) {
        FICHIER_ILLISIBLE
    } else {
        GOOGLE_REFUSE
    }
}

impl Sauvegarde {
    /// La ligne de la sauvegarde. Ce qui est reporte au prochain passage s'y dit en passant, sans alarme : c'est
    /// `persistant` (la trace, `sauvegarde_souci`) qui dit ce qui dure.
    pub fn resume(&self) -> String {
        if self.hors_ligne {
            return "Hors connexion : sauvegarde au prochain passage.".into();
        }
        let reporte = if self.reportes.is_empty() { String::new() } else { format!(" ; {} au prochain passage", combien(&self.reportes, "reporte", "reportes")) };
        if self.deposes.is_empty() {
            if reporte.is_empty() {
                return format!("Sauvegarde a jour ({} fichiers).", self.identiques);
            }
            return format!("Sauvegarde : {} fichier(s) inchange(s){}.", self.identiques, reporte);
        }
        format!("Sauvegarde : {} fichier(s) envoye(s), {}, {} inchange(s){}.", self.deposes.len(), lisible(self.octets_deposes), self.identiques, reporte)
    }
}

/// « 1 dossier reporte », « 2 fichiers reportes », « 1 dossier et 2 fichiers reportes ».
fn combien(reportes: &[Reporte], un: &str, plusieurs: &str) -> String {
    let (nd, nf) = (reportes.iter().filter(|r| r.dossier).count(), reportes.iter().filter(|r| !r.dossier).count());
    let nom = |n: usize, quoi: &str| format!("{} {}{}", n, quoi, if n > 1 { "s" } else { "" });
    let (texte, pluriel) = match (nd, nf) {
        (0, f) => (nom(f, "fichier"), f > 1),
        (d, 0) => (nom(d, "dossier"), d > 1),
        (d, f) => (format!("{} et {}", nom(d, "dossier"), nom(f, "fichier")), true),
    };
    format!("{} {}", texte, if pluriel { plusieurs } else { un })
}

// ---------------------------------------------------------------- ce qui persiste

/// CE QUI PERSISTE, ET SEULEMENT CELA. Un dossier reporte au prochain passage n'est pas un
/// incident a montrer : Google rend parfois un 500 que le passage suivant ne voit plus. Ce qui se dit, c'est ce qui dure :
/// le meme dossier (ou fichier) en echec sur `PASSAGES_QUI_DURENT` passages de suite (une heure, a vingt minutes d'une veille a
/// l'autre), ou depuis `UNE_JOURNEE`, ou une journee entiere sans sauvegarde complete.
pub const PASSAGES_QUI_DURENT: i64 = 3;
pub const UNE_JOURNEE: i64 = 86_400;

/// « un dossier n'a pas pu être envoyé », « 2 fichiers n'ont pas pu être envoyés ».
fn ce_qui_manque(nd: usize, nf: usize) -> String {
    let nom = |n: usize, quoi: &str| if n == 1 { format!("un {}", quoi) } else { format!("{} {}s", n, quoi) };
    let (texte, pluriel) = match (nd, nf) {
        (0, f) => (nom(f, "fichier"), f > 1),
        (d, 0) => (nom(d, "dossier"), d > 1),
        (d, f) => (format!("{} et {}", nom(d, "dossier"), nom(f, "fichier")), true),
    };
    if pluriel {
        format!("{} n'ont pas pu être envoyés", texte)
    } else {
        format!("{} n'a pas pu être envoyé", texte)
    }
}

/// « depuis 09:51 » le jour meme, « depuis hier », « depuis le 18/10 ».
fn depuis_quand(depuis: Naif, maintenant: Naif) -> String {
    let iso = depuis.iso();
    match maintenant.jour().moins(&depuis.jour()) {
        0 => format!("depuis {}", &iso[11..16]),
        1 => "depuis hier".to_string(),
        _ => format!("depuis le {}/{}", &iso[8..10], &iso[5..7]),
    }
}


/// La phrase de ce qui persiste dans l'index (`echecs`, `derniere_complete`), a l'heure naive `maintenant`, ou `None`.
pub fn phrase_persistante(index: &Index, maintenant: Naif) -> Result<Option<String>, ErreurGoogle> {
    let Some(echecs) = index.echecs.as_ref().filter(|e| !e.is_empty()) else { return Ok(None) };
    // (cle, depuis, cause) de chaque echec, et ceux qui durent.
    let mut tous: Vec<(&String, Naif, &str, bool)> = Vec::new();
    for (cle, e) in echecs {
        let depuis = e.depuis.naif();
        tous.push((cle, depuis, e.cause.as_str(), e.passages >= PASSAGES_QUI_DURENT || maintenant.depuis(depuis) >= UNE_JOURNEE));
    }
    let complete = index.derniere_complete.map(|c| c.naif());
    let durent: Vec<_> = tous.iter().filter(|t| t.3).collect();
    let (retenus, par_l_item): (Vec<_>, bool) = if !durent.is_empty() {
        (durent, true)
    } else {
        match complete.filter(|c| maintenant.depuis(*c) >= UNE_JOURNEE) {
            Some(_) => (tous.iter().collect(), false),
            None => return Ok(None),
        }
    };
    let nd = retenus.iter().filter(|t| t.0.ends_with('/')).count();
    let nf = retenus.len() - nd;
    // Le plus ancien : sa cause, et depuis quand (le plus ancien qui dure, ou la derniere sauvegarde complete).
    let Some(ancien) = retenus.iter().min_by_key(|t| t.1) else { return Ok(None) };
    let depuis = match (par_l_item, complete) {
        (false, Some(c)) => c,
        _ => ancien.1,
    };
    let quand = depuis_quand(depuis, maintenant);
    Ok(Some(if par_l_item {
        format!("Sauvegarde : {} {} ({}) ; le logiciel réessaie.", ce_qui_manque(nd, nf), quand, ancien.2)
    } else {
        format!("Sauvegarde : {} ({}), pas de sauvegarde complète {} ; le logiciel réessaie.", ce_qui_manque(nd, nf), ancien.2, quand)
    }))
}

/// `souci` : ce qui persiste de la sauvegarde, dit en clair (« Sauvegarde : un dossier n'a pas pu être envoyé depuis hier
/// (Google indisponible) ; le logiciel réessaie. »), ou `None` si tout est passe, ou si ce n'est que passager, ou si
/// aucune sauvegarde n'est partie. SANS RESEAU : lu dans l'index. La trace de l'ouverture le porte (`sauvegarde_souci`),
/// la fenetre le montre a la place de la sauvegarde.
pub fn souci(racine: &Path) -> Result<Option<String>, ErreurGoogle> {
    match lire_index(racine)? {
        Some(index) => phrase_persistante(&index, socle::paris::maintenant_naif()),
        None => Ok(None),
    }
}

/// Des octets en mega-octets (2^20), au dixieme : « 3.5 ».
pub fn mo(octets: u64) -> String {
    match socle::num::quotient(u128::from(octets), 1_048_576, 1) {
        Some(t) => t,
        None => unreachable!("un mega-octet n'est pas nul"),
    }
}

pub(crate) fn lisible(octets: u64) -> String {
    if octets >= 1024 * 1024 {
        format!("{} Mo", mo(octets))
    } else if octets >= 1024 {
        format!("{} Ko", octets / 1024)
    } else {
        format!("{} o", octets)
    }
}

// ---------------------------------------------------------------- l'inventaire

/// La taille et la date de modification (secondes entieres) d'un fichier (`inventaire`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Etat {
    pub octets: u64,
    pub mtime: i64,
}

/// Un nom ou un suffixe exclu, ou un dossier exclu n'importe ou dans le chemin relatif
/// (le nom du fichier compris). La restauration s'en sert aussi : elle ne redescend que ce que la sauvegarde envoie.
pub(crate) fn ecarte(relatif: &Path) -> bool {
    let exclu = |nom: &str| NOMS_EXCLUS.contains(&nom) || SUFFIXES_EXCLUS.iter().any(|s| nom.ends_with(s));
    if relatif.file_name().is_some_and(|n| exclu(&n.to_string_lossy())) {
        return true;
    }
    relatif.components().any(|c| DOSSIERS_EXCLUS.contains(&c.as_os_str().to_string_lossy().as_ref()))
}

/// `empreinte` : MD5 du contenu, le meme que Drive publie. Lu par morceaux.
pub fn empreinte(chemin: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(chemin)?;
    let mut h = Md5::new();
    let mut bloc = vec![0u8; 1024 * 256];
    loop {
        let n = f.read(&mut bloc)?;
        if n == 0 {
            break;
        }
        h.update(&bloc[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{:02x}", b)).collect())
}

/// Les entrees (fichiers et liens, pas les dossiers) sous `dossier`, relatives a `racine`. Un dossier lien n'est pas
/// suivi. Un dossier qui disparait pendant le parcours (`engagements/avant/` ne garde que les dernieres copies) n'a plus
/// rien a sauver ; un dossier qui ne se lit pas arrete l'inventaire.
fn parcourir(racine: &Path, dossier: &Path, sortie: &mut Vec<PathBuf>) -> Result<(), ErreurGoogle> {
    let entrees = match std::fs::read_dir(dossier) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(ErreurGoogle::fichier(&e, dossier)),
    };
    for e in entrees {
        let e = e.map_err(|x| ErreurGoogle::fichier(&x, dossier))?;
        let chemin = e.path();
        let genre = match e.file_type() {
            Ok(t) => t,
            Err(x) if x.kind() == std::io::ErrorKind::NotFound => continue,
            Err(x) => return Err(ErreurGoogle::fichier(&x, &chemin)),
        };
        if genre.is_dir() {
            parcourir(racine, &chemin, sortie)?;
        } else if let Ok(rel) = chemin.strip_prefix(racine) {
            sortie.push(rel.to_path_buf());
        }
    }
    Ok(())
}

/// `inventaire` : les fichiers triés par chemin (composante par
/// composante, pas en texte : `a/x` passe avant `a b/x`), et le nombre d'ecartes. Ne lit aucun
/// contenu. Un fichier qui disparait entre le parcours et sa lecture n'est plus a sauver.
pub fn inventaire(racine: &Path) -> Result<(Vec<(String, Etat)>, usize), ErreurGoogle> {
    let mut chemins = Vec::new();
    parcourir(racine, racine, &mut chemins)?;
    chemins.sort();
    let (mut trouves, mut ecartes) = (Vec::new(), 0);
    for rel in chemins {
        let complet = racine.join(&rel);
        if !complet.is_file() {
            continue;
        }
        if ecarte(&rel) {
            ecartes += 1;
            continue;
        }
        let m = match std::fs::metadata(&complet) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(ErreurGoogle::fichier(&e, &complet)),
        };
        let relatif = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
        // La date du fichier dit s'il a change depuis son dernier envoi : datee hors du calendrier, elle ne le dit pas, et
        // la sauvegarde s'arrete en le nommant plutot que de l'envoyer a l'aveugle.
        let Some(date) = socle::paris::mtime(&m) else {
            return Err(ErreurGoogle::nouvelle(
                Classe::Disque,
                format!("{} est daté hors du calendrier (années 1 à 9999) : régler la date du fichier.", socle::date::repr_texte(&complet.display().to_string())),
            ));
        };
        trouves.push((relatif, Etat { octets: m.len(), mtime: date.entieres() }));
    }
    Ok((trouves, ecartes))
}

// --------------------------------------------------------------------- l'index

/// `chemin_index` : `config/`, et pas `journal/` : de l'etat machine, pas un document. Il est
/// ecarte de la sauvegarde (`NOMS_EXCLUS`).
pub fn chemin_index(racine: &Path) -> PathBuf {
    racine.join(socle::fichiers::CONFIG).join(INDEX)
}

/// L'INDEX DE LA SAUVEGARDE (`config/.sauvegarde.json`) : quand le dernier passage s'est fait, vers quelle racine du
/// Drive, les identifiants des dossiers Drive (par chemin `racine/sauvegarde/…`), l'etat de chaque fichier passe ; et, s'il
/// reste un echec, la derniere sauvegarde complete et ce qui reste en echec (`souci`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    pub quand: socle::paris::HeureDeSaisie,
    pub racine_drive: String,
    pub dossiers: BTreeMap<String, String>,
    pub fichiers: BTreeMap<String, Etat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derniere_complete: Option<socle::paris::HeureDeSaisie>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echecs: Option<BTreeMap<String, EchecIndexe>>,
}

/// Ce qui reste en echec d'un passage a l'autre : depuis quand, combien de passages, la cause en clair (`cause_de`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EchecIndexe {
    pub depuis: socle::paris::HeureDeSaisie,
    pub passages: i64,
    pub cause: String,
}


/// L'index, ou `None` si aucune sauvegarde ne l'a encore ecrit. Un index qui ne se lit pas se refuse.
pub fn lire_index(racine: &Path) -> Result<Option<Index>, ErreurGoogle> {
    let p = chemin_index(racine);
    if !p.exists() {
        return Ok(None);
    }
    socle::forme::lire_fichier(&p, "L'index de la sauvegarde ne se lit pas : le mettre de côté, la sauvegarde suivante le refait.").map(Some).map_err(|m| ErreurGoogle::from(m.reclasser(Classe::Illisible)))
}

pub fn ecrire_index(racine: &Path, index: &Index) -> Result<(), ErreurGoogle> {
    socle::ecriture::ecrire_json(&chemin_index(racine), index, 1).map_err(|e| ErreurGoogle::from(e.reclasser(Classe::Disque)))
}

/// `rien_n_a_bouge` : le disque est-il identique au dernier passage ? SANS RESEAU. Taille et date
/// suffisent : on ne decide pas d'un televersement, seulement s'il vaut la peine d'aller voir. Au
/// moindre doute (index absent, fichier apparu ou disparu), la reponse est non.
///
/// `racine_drive` compte : un index ecrit pour une autre racine
/// du Drive (`--racine-drive`) laisserait la veille croire que tout est sauvegarde la ou on l'attend. Un index d'une autre
/// racine (ou qui n'en dit aucune) donne non.
pub fn rien_n_a_bouge(racine: &Path, racine_drive: &str) -> Result<bool, ErreurGoogle> {
    let Some(index) = lire_index(racine)? else { return Ok(false) };
    if index.racine_drive != racine_drive || index.fichiers.is_empty() {
        return Ok(false);
    }
    let connus = &index.fichiers;
    let (presents, _) = inventaire(racine)?;
    if presents.len() != connus.len() {
        return Ok(false);
    }
    Ok(presents.iter().all(|(relatif, etat)| connus.get(relatif) == Some(etat)))
}

/// Ce que dit l'index du dernier passage (`derniere`) : quand, combien, vers quelle racine du Drive. Pour l'affichage.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Derniere {
    pub quand: socle::paris::HeureDeSaisie,
    pub fichiers: usize,
    pub racine_drive: String,
}

/// Le dernier passage, ou `None` si aucune sauvegarde n'est encore partie.
pub fn derniere(racine: &Path) -> Result<Option<Derniere>, ErreurGoogle> {
    Ok(lire_index(racine)?.map(|i| Derniere { fichiers: i.fichiers.len(), quand: i.quand, racine_drive: i.racine_drive }))
}
