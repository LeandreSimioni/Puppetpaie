//! Lire les fichiers livres avec le code (`forme_en_cache`) et dire ou est le code.
//!
//! Le cache est cle par chemin ET par signature (date de modification,
//! taille) : un fichier reecrit se relit.

use crate::echec::{Echec, R};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

// ------------------------------------------------ le registre des lectures
//
// Tout ce que le calcul lit sur le disque passe par `lire_texte`, `existe`,
// `est_dossier` et `lister`. Quand un registre est ouvert (`suivre`), chacun
// y note ce qu'il a vu : l'empreinte du contenu d'un fichier, `absent`, la
// liste des `.json` d'un dossier. C'est la liste des entrees du calcul
// (`engagement::empreinte`). Un cache qui ne note rien quand il sert (le compteur des conges) se cle par
// `generation` : un calcul suivi a la sienne, a lui seul, et repart a froid ; hors registre, la generation est 0 et
// ce cache sert d'une route a l'autre (la fenetre garde le moteur charge). Les baremes composes gardent ce qu'ils ont lu
// et le notent quand ils servent (`garde_notee`).

struct Registre {
    racine: PathBuf,
    code: PathBuf,
    /// Ce qu'il a note, sous le texte de chaque cle (`CleDeLecture`) : il ne le rend que lu (`rendre`).
    entrees: BTreeMap<String, ValeurLue>,
    /// Le numero de ce registre, unique dans le processus.
    numero: u64,
}

static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// La cle des caches qui ne notent pas : le numero du registre ouvert SUR CE FIL, 0 sans registre. Un calcul suivi ne
/// sert ainsi que ce qu'il a compose lui-meme (tout ce qu'il lit se note), meme si un autre fil compose en meme temps.
pub fn generation() -> u64 {
    REGISTRE.with(|r| r.borrow().last().into_iter().map(|g| g.numero).sum())
}

thread_local! {
    static REGISTRE: RefCell<Vec<Registre>> = const { RefCell::new(Vec::new()) };
}

/// Ouvre un registre : les chemins sous `racine` ou `code` s'y notent relatifs. Un calcul suivi peut en faire un autre
/// (la paie d'un mois lit celle du mois d'avant, qui se calcule alors) : le registre neuf se pose au-dessus de celui qui
/// est ouvert, et `rendre` redonne celui-ci a son calcul.
pub fn suivre(racine: &Path, code: &Path) {
    // Un registre neuf, c'est un calcul « a froid » : un cache qui ne note rien quand il sert (`generation`)
    // ne lui sert que ce qu'il compose, pour que la liste des entrees soit celle d'un processus neuf.
    let numero = GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    REGISTRE.with(|r| r.borrow_mut().push(Registre { racine: racine.to_path_buf(), code: code.to_path_buf(), entrees: BTreeMap::new(), numero }));
}

/// Un registre est-il ouvert sur ce fil ?
pub fn registre_ouvert() -> bool {
    suivi()
}

/// Au-dela, un cache du processus se vide (`ranger`).
pub const CACHE_MAX: usize = 512;

/// Range `v` sous `cle` dans un cache du processus. La fenetre garde le moteur charge : chaque calcul
/// suivi ajoute ses cles (`generation`), et un cache plein se vide plutot que de grossir tant que la fenetre tourne.
pub fn ranger<K: Eq + std::hash::Hash, V>(cache: &Mutex<HashMap<K, V>>, cle: K, v: V) {
    let mut c = crate::memoire::tenu(cache);
    if c.len() >= CACHE_MAX {
        c.clear();
    }
    c.insert(cle, v);
}

/// Ce qu'un cache qui note garde (`garde_notee`) : la valeur, et ce que sa composition a lu, sous la base (racine, code)
/// du registre ou elle s'est faite ; rien quand elle s'est faite hors registre.
pub struct Notee<V> {
    valeur: V,
    lu: Option<(PathBuf, PathBuf, BTreeMap<String, ValeurLue>)>,
}

/// UN CACHE QUI NOTE : la valeur que `composer` fait (un bareme fondu) se garde sous `cle` avec ce que sa composition a
/// lu. Servie dans un registre ouvert, elle y note ces entrees, comme si elle venait de se composer : la liste des
/// entrees d'un calcul suivi reste celle d'un processus neuf, sans recomposer. Une valeur faite hors registre, ou sous
/// une autre base, se recompose la premiere fois qu'un registre la demande. `cle` dit ce dont la valeur depend.
pub fn garde_notee<V: Clone, E>(cache: &Mutex<HashMap<String, Notee<V>>>, cle: String, composer: impl FnOnce() -> Result<V, E>) -> Result<V, E> {
    let base = REGISTRE.with(|r| r.borrow().last().map(|g| (g.racine.clone(), g.code.clone())));
    let servie = {
        let c = crate::memoire::tenu(cache);
        c.get(&cle).and_then(|n| match (&base, &n.lu) {
            (None, _) => Some((n.valeur.clone(), None)),
            (Some((r, k)), Some((r2, k2, entrees))) if r == r2 && k == k2 => Some((n.valeur.clone(), Some(entrees.clone()))),
            _ => None,
        })
    };
    if let Some((v, entrees)) = servie {
        if let Some(entrees) = entrees {
            REGISTRE.with(|r| {
                if let Some(g) = r.borrow_mut().last_mut() {
                    for (k, x) in entrees {
                        g.entrees.entry(k).or_insert(x);
                    }
                }
            });
        }
        return Ok(v);
    }
    let (v, lu) = match base {
        None => (composer()?, None),
        Some((r, k)) => {
            suivre(&r, &k);
            let v = composer();
            let entrees = rendre_notees();
            (v?, Some((r, k, entrees)))
        }
    };
    ranger(cache, cle, Notee { valeur: v.clone(), lu });
    Ok(v)
}

/// Ferme le dernier registre ouvert et rend ce qu'il a note, chaque cle relue (`CleDeLecture::lire`) et triee comme son
/// texte. Ce qu'un calcul fait au milieu d'un autre a lu, celui qui l'a demande en depend aussi : ses entrees s'ajoutent
/// au registre d'en dessous. Une cle notee qui ne se relit pas refuse ; le registre est ferme quand meme.
pub fn rendre<S, L, P>() -> R<BTreeMap<CleDeLecture<S, L, P>, ValeurLue>>
where
    S: std::fmt::Display + serde::Serialize + serde::de::DeserializeOwned + TryFrom<String, Error = Echec> + Eq,
    L: std::fmt::Display + TryFrom<String, Error = Echec> + Eq,
    P: serde::Serialize + serde::de::DeserializeOwned + Eq,
{
    rendre_notees().into_iter().map(|(k, v)| Ok((CleDeLecture::lire(&k)?, v))).collect()
}

/// Ferme le dernier registre ouvert et rend ce qu'il a note, sous le texte de ses cles.
fn rendre_notees() -> BTreeMap<String, ValeurLue> {
    REGISTRE.with(|r| {
        let mut pile = r.borrow_mut();
        let entrees: BTreeMap<String, ValeurLue> = pile.pop().into_iter().flat_map(|g| g.entrees).collect();
        if let Some(dessous) = pile.last_mut() {
            for (k, v) in &entrees {
                dessous.entrees.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        entrees
    })
}

/// Ferme tous les registres de ce fil : un calcul tombe a pu en laisser d'ouverts.
pub fn fermer_tout() {
    REGISTRE.with(|r| r.borrow_mut().clear());
}

fn suivi() -> bool {
    REGISTRE.with(|r| !r.borrow().is_empty())
}

/// Le texte de la cle d'une lecture de `chemin` du genre `genre` (`CleDeLecture::Fichier`, `Existe`, `EstDossier`,
/// `Dossier`) : `genre racine:chemin` / `genre code:chemin` / `genre abs:chemin`, l'ecriture de `CheminLu`. Le registre
/// garde ce texte ; il ne le rend que lu (`rendre`).
fn cle_de(genre: &str, chemin: &Path) -> String {
    REGISTRE.with(|r| {
        let r = r.borrow();
        let r = r.last().expect("registre ouvert");
        for (nom, base) in [(RACINE, &r.racine), (CODE, &r.code)] {
            if let Some(rel) = sous(chemin, base) {
                return format!("{} {}", genre, base_et_chemin(nom, &CheminRelatif::en_barres(rel)));
            }
        }
        format!("{} {}", genre, base_et_chemin(ABSOLU, &chemin.to_string_lossy()))
    })
}

/// L'ecriture d'un chemin lu, `<base>:<chemin>` : la seule, que `CheminLu` et la cle notee (`cle_de`) prennent.
fn base_et_chemin(base: &str, chemin: &str) -> String {
    format!("{}:{}", base, chemin)
}

fn noter(genre: &str, chemin: &Path, valeur: ValeurLue) {
    if suivi() {
        let k = cle_de(genre, chemin);
        REGISTRE.with(|r| {
            if let Some(g) = r.borrow_mut().last_mut() {
                g.entrees.insert(k, valeur);
            }
        });
    }
}

/// Note une lecture qui n'est pas celle d'un fichier : le jour, la marque du code, les fiches, les imprevus d'un
/// salarie, une lecture datee. Le registre en garde le texte (`CleDeLecture`, son ecriture).
pub fn noter_lecture<S: std::fmt::Display + serde::Serialize, L: std::fmt::Display, P: serde::Serialize>(cle: &CleDeLecture<S, L, P>, valeur: ValeurLue) {
    REGISTRE.with(|r| {
        if let Some(g) = r.borrow_mut().last_mut() {
            g.entrees.insert(cle.to_string(), valeur);
        }
    });
}

const RACINE: &str = "racine";
const CODE: &str = "code";
const ABSOLU: &str = "abs";
const FICHIER: &str = "fichier";
const EXISTE: &str = "existe";
const EST_DOSSIER: &str = "dossier?";
const DOSSIER: &str = "dossier";

/// Le magasin des imprevus, sous la racine de la compagnie (`engagement::imprevus::DOSSIER`) : la cle des imprevus d'un
/// salarie le nomme (`CleDeLecture::Imprevus`).
pub const MAGASIN_DES_IMPREVUS: &str = "imprevus";

/// LE CHEMIN QU'UNE LECTURE NOTEE NOMME : sous la compagnie (`racine:<chemin>`), sous le code livre (`code:<chemin>`), ou
/// ailleurs, en absolu (`abs:<chemin>`). Aucun chemin sous une base : la base elle-meme (`racine:`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CheminLu {
    Racine(Option<CheminRelatif>),
    Code(Option<CheminRelatif>),
    Absolu(PathBuf),
}

impl CheminLu {
    /// Le chemin ecrit `texte` (`<base>:<chemin>`), ou la phrase qui le refuse.
    fn lire(texte: &str) -> R<CheminLu> {
        let rel = |r: &str| if r.is_empty() { Ok(None) } else { CheminRelatif::lire(r).map(Some) };
        match texte.split_once(':') {
            Some((RACINE, r)) => Ok(CheminLu::Racine(rel(r)?)),
            Some((CODE, r)) => Ok(CheminLu::Code(rel(r)?)),
            Some((ABSOLU, p)) => Ok(CheminLu::Absolu(PathBuf::from(p))),
            _ => Err(crate::refus!("{} n'est pas le chemin d'une lecture notée : le signaler au développeur.", repr(texte))),
        }
    }

    /// Le chemin sur le disque, sous la compagnie `racine` ou le code `code`.
    pub fn sur_le_disque(&self, racine: &Path, code: &Path) -> PathBuf {
        match self {
            CheminLu::Racine(Some(c)) => racine.join(c),
            CheminLu::Racine(None) => racine.to_path_buf(),
            CheminLu::Code(Some(c)) => code.join(c),
            CheminLu::Code(None) => code.to_path_buf(),
            CheminLu::Absolu(p) => p.clone(),
        }
    }

    /// Le chemin est-il le dossier `dossier` de la compagnie, ou dessous ?
    pub fn dans_la_compagnie(&self, dossier: &str) -> bool {
        match self {
            CheminLu::Racine(Some(c)) => c.texte() == dossier || c.texte().strip_prefix(dossier).is_some_and(|r| r.starts_with('/')),
            _ => false,
        }
    }
}

impl std::fmt::Display for CheminLu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let texte = match self {
            CheminLu::Racine(Some(c)) => base_et_chemin(RACINE, &c.to_string()),
            CheminLu::Racine(None) => base_et_chemin(RACINE, ""),
            CheminLu::Code(Some(c)) => base_et_chemin(CODE, &c.to_string()),
            CheminLu::Code(None) => base_et_chemin(CODE, ""),
            CheminLu::Absolu(p) => base_et_chemin(ABSOLU, &p.to_string_lossy()),
        };
        f.write_str(&texte)
    }
}

/// CE DONT UNE PAIE DEPEND, UNE LECTURE NOTEE (`engagement::empreinte`) : un fichier lu, un chemin dont on a demande
/// s'il existe ou s'il est un dossier, un dossier parcouru, la marque du code qui calcule, le jour, les fiches d'une
/// paie, les imprevus d'un salarie, une lecture datee. `S` est la cle du salarie, `L` le lieu d'une mission, `P` la cle
/// du spectacle, dont les types vivent au-dessus du socle (`droit::contexte::CleLue`). Elle s'ecrit par son texte
/// (`Display`), se trie comme lui, et ne se relit qu'ici (`lire` : le registre, `lectures`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CleDeLecture<S, L, P> {
    /// `fichier <chemin>` : le contenu d'un fichier.
    Fichier(CheminLu),
    /// `existe <chemin>` : le chemin existe-t-il ?
    Existe(CheminLu),
    /// `dossier? <chemin>` : le chemin est-il un dossier ?
    EstDossier(CheminLu),
    /// `dossier <chemin>` : les `.json` d'un dossier.
    Dossier(CheminLu),
    /// `moteur` : la marque du code qui calcule.
    Moteur,
    /// `date` : le jour du contexte.
    Date,
    /// `aujourdhui` : le jour de l'horloge.
    Aujourdhui,
    /// `fiches [<mois>,<salarie>,<spectacle>]` : l'entreprise, la fiche du salarie et celle du spectacle au dernier jour
    /// du mois.
    Fiches { mois: crate::mois::Mois, salarie: S, spectacle: Option<P> },
    /// `imprevus racine:imprevus <salarie>` : les imprevus d'un salarie.
    Imprevus { salarie: S },
    /// Une lecture qui porte un jour ou un mois.
    Datee(LectureDatee<S, L>),
}

impl<S, L, P> CleDeLecture<S, L, P> {
    /// Le chemin que nomme une lecture d'un fichier ou d'un dossier.
    pub fn chemin(&self) -> Option<&CheminLu> {
        match self {
            CleDeLecture::Fichier(c) | CleDeLecture::Existe(c) | CleDeLecture::EstDossier(c) | CleDeLecture::Dossier(c) => Some(c),
            _ => None,
        }
    }
}

impl<S: std::fmt::Display + serde::Serialize, L: std::fmt::Display, P: serde::Serialize> std::fmt::Display for CleDeLecture<S, L, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CleDeLecture::Fichier(c) => write!(f, "{} {}", FICHIER, c),
            CleDeLecture::Existe(c) => write!(f, "{} {}", EXISTE, c),
            CleDeLecture::EstDossier(c) => write!(f, "{} {}", EST_DOSSIER, c),
            CleDeLecture::Dossier(c) => write!(f, "{} {}", DOSSIER, c),
            CleDeLecture::Moteur => f.write_str("moteur"),
            CleDeLecture::Date => f.write_str("date"),
            CleDeLecture::Aujourdhui => f.write_str("aujourdhui"),
            CleDeLecture::Fiches { mois, salarie, spectacle } => write!(f, "fiches {}", crate::ecriture::json_en_ligne(&(mois, salarie, spectacle))),
            CleDeLecture::Imprevus { salarie } => write!(f, "{m} {}:{m} {}", RACINE, salarie, m = MAGASIN_DES_IMPREVUS),
            CleDeLecture::Datee(l) => write!(f, "{}", l),
        }
    }
}

impl<S, L, P> CleDeLecture<S, L, P>
where
    S: serde::de::DeserializeOwned + TryFrom<String, Error = Echec>,
    L: TryFrom<String, Error = Echec>,
    P: serde::de::DeserializeOwned,
{
    /// La cle ecrite `texte`, ou la phrase qui la refuse : la seule porte par laquelle une cle notee redevient un type.
    fn lire(texte: &str) -> R<CleDeLecture<S, L, P>> {
        match texte {
            "moteur" => return Ok(CleDeLecture::Moteur),
            "date" => return Ok(CleDeLecture::Date),
            "aujourdhui" => return Ok(CleDeLecture::Aujourdhui),
            _ => {}
        }
        if let Some(j) = texte.strip_prefix("fiches ") {
            let (mois, salarie, spectacle) = crate::forme::lire_texte::<(crate::mois::Mois, S, Option<P>)>(j, crate::echec::PAIE_ILLISIBLE)?;
            return Ok(CleDeLecture::Fiches { mois, salarie, spectacle });
        }
        if let Some(s) = texte.strip_prefix(&format!("{m} {}:{m} ", RACINE, m = MAGASIN_DES_IMPREVUS)) {
            return Ok(CleDeLecture::Imprevus { salarie: S::try_from(s.to_string())? });
        }
        if let Some(l) = LectureDatee::lue(texte) {
            return Ok(CleDeLecture::Datee(l?));
        }
        let inconnue = || crate::refus!("L'empreinte d'une paie note la lecture « {} », que le registre des lectures n'écrit pas : le signaler au développeur.", texte);
        let Some((genre, chemin)) = texte.split_once(' ') else { return Err(inconnue()) };
        let sorte: fn(CheminLu) -> CleDeLecture<S, L, P> = match genre {
            FICHIER => CleDeLecture::Fichier,
            EXISTE => CleDeLecture::Existe,
            EST_DOSSIER => CleDeLecture::EstDossier,
            DOSSIER => CleDeLecture::Dossier,
            _ => return Err(inconnue()),
        };
        Ok(sorte(CheminLu::lire(chemin)?))
    }
}

impl<S: std::fmt::Display + serde::Serialize + Eq, L: std::fmt::Display + Eq, P: serde::Serialize + Eq> Ord for CleDeLecture<S, L, P> {
    fn cmp(&self, autre: &Self) -> std::cmp::Ordering {
        self.to_string().cmp(&autre.to_string())
    }
}

impl<S: std::fmt::Display + serde::Serialize + Eq, L: std::fmt::Display + Eq, P: serde::Serialize + Eq> PartialOrd for CleDeLecture<S, L, P> {
    fn partial_cmp(&self, autre: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(autre))
    }
}

/// CE QU'UNE LECTURE NOTEE VALAIT. Elle s'ecrit par son texte (`Display`) ; relue, elle se lit selon la sorte de sa cle
/// (`lue`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValeurLue {
    /// L'empreinte d'un contenu : un fichier, la liste des `.json` d'un dossier, une donnee hachee (les fiches, les
    /// imprevus, les paies validees, les mensuels d'avant).
    Empreinte(Empreinte),
    /// `absent` : le fichier ou le dossier n'existe pas.
    Absent,
    /// `illisible : <raison>` : il ne se lit pas.
    Illisible(String),
    /// `oui`, `non` : le chemin existe, ou est un dossier.
    OuiNon(bool),
    /// Le jour lu (`date`, `aujourdhui`), en AAAA-MM-JJ.
    Jour(crate::date::Jour),
    /// La marque du code qui calcule.
    Moteur(String),
    /// La saisie d'un mois : `<ce que le mois pose pour tous>:<ses engagements>`, deux empreintes.
    Saisie { mois: Empreinte, engagements: Empreinte },
    /// L'empreinte composee d'une donnee, ecrite par son seul ecrivain : le taux de PAS d'un salarie a un jour
    /// (`droit::pas`), l'indemnite de mission d'un jour (`droit::missions`).
    Composee(String),
}

impl std::fmt::Display for ValeurLue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValeurLue::Empreinte(e) => write!(f, "{}", e),
            ValeurLue::Absent => f.write_str(ABSENT),
            ValeurLue::Illisible(r) => write!(f, "{}{}", ILLISIBLE, r),
            ValeurLue::OuiNon(true) => f.write_str("oui"),
            ValeurLue::OuiNon(false) => f.write_str("non"),
            ValeurLue::Jour(j) => f.write_str(&j.iso()),
            ValeurLue::Moteur(m) => f.write_str(m),
            ValeurLue::Saisie { mois, engagements } => write!(f, "{}:{}", mois, engagements),
            ValeurLue::Composee(t) => f.write_str(t),
        }
    }
}

const ABSENT: &str = "absent";
const ILLISIBLE: &str = "illisible : ";

impl ValeurLue {
    /// La valeur ecrite `texte` de la lecture `cle`, lue selon sa sorte, ou la phrase qui la refuse.
    fn lue<S: std::fmt::Display + serde::Serialize, L: std::fmt::Display, P: serde::Serialize>(cle: &CleDeLecture<S, L, P>, texte: String) -> R<ValeurLue> {
        let refus = || crate::refus!("La lecture « {} » d'une paie vaut {}, que le registre des lectures n'écrit pas : le signaler au développeur.", cle, repr(&texte));
        let empreinte = |t: &str| Empreinte::lire(t).map_err(|_| refus());
        Ok(match cle {
            CleDeLecture::Fichier(_) | CleDeLecture::Dossier(_) => match texte.as_str() {
                ABSENT => ValeurLue::Absent,
                t => match t.strip_prefix(ILLISIBLE) {
                    Some(r) => ValeurLue::Illisible(r.to_string()),
                    None => ValeurLue::Empreinte(empreinte(t)?),
                },
            },
            CleDeLecture::Existe(_) | CleDeLecture::EstDossier(_) => match texte.as_str() {
                "oui" => ValeurLue::OuiNon(true),
                "non" => ValeurLue::OuiNon(false),
                _ => return Err(refus()),
            },
            CleDeLecture::Moteur if !texte.is_empty() => ValeurLue::Moteur(texte),
            CleDeLecture::Date | CleDeLecture::Aujourdhui => ValeurLue::Jour(crate::date::Jour::lire(&texte).map_err(|_| refus())?),
            CleDeLecture::Fiches { .. } | CleDeLecture::Imprevus { .. } | CleDeLecture::Datee(LectureDatee::Mensuels { .. } | LectureDatee::Paies { .. }) => ValeurLue::Empreinte(empreinte(&texte)?),
            CleDeLecture::Datee(LectureDatee::Saisie { .. }) => match texte.split_once(':') {
                Some((m, e)) => ValeurLue::Saisie { mois: empreinte(m)?, engagements: empreinte(e)? },
                None => return Err(refus()),
            },
            CleDeLecture::Datee(LectureDatee::Pas { .. } | LectureDatee::Mission { .. }) if !texte.is_empty() => ValeurLue::Composee(texte),
            CleDeLecture::Moteur | CleDeLecture::Datee(_) => return Err(refus()),
        })
    }
}

/// L'ECRITURE DES LECTURES D'UNE PAIE (`#[serde(with = "socle::fichiers::lectures")]`, `engagement::paie_ecrite`) :
/// chaque cle par son texte, dans l'ordre de son texte, et sa valeur par son texte ; relue, chaque cle se lit
/// (`CleDeLecture::lire`) et sa valeur selon la sorte de la cle (`ValeurLue::lue`).
pub mod lectures {
    use super::{CleDeLecture, Echec, ValeurLue};
    use std::collections::BTreeMap;

    pub fn serialize<S, L, P, Z>(m: &BTreeMap<CleDeLecture<S, L, P>, ValeurLue>, z: Z) -> Result<Z::Ok, Z::Error>
    where
        S: std::fmt::Display + serde::Serialize,
        L: std::fmt::Display,
        P: serde::Serialize,
        Z: serde::Serializer,
    {
        z.collect_map(m.iter().map(|(k, v)| (k.to_string(), v.to_string())))
    }

    pub fn deserialize<'de, S, L, P, D>(d: D) -> Result<BTreeMap<CleDeLecture<S, L, P>, ValeurLue>, D::Error>
    where
        S: std::fmt::Display + serde::Serialize + serde::de::DeserializeOwned + TryFrom<String, Error = Echec> + Eq,
        L: std::fmt::Display + TryFrom<String, Error = Echec> + Eq,
        P: serde::Serialize + serde::de::DeserializeOwned + Eq,
        D: serde::Deserializer<'de>,
    {
        struct Visiteur<S, L, P>(std::marker::PhantomData<(S, L, P)>);
        impl<'de, S, L, P> serde::de::Visitor<'de> for Visiteur<S, L, P>
        where
            S: std::fmt::Display + serde::Serialize + serde::de::DeserializeOwned + TryFrom<String, Error = Echec> + Eq,
            L: std::fmt::Display + TryFrom<String, Error = Echec> + Eq,
            P: serde::Serialize + serde::de::DeserializeOwned + Eq,
        {
            type Value = BTreeMap<CleDeLecture<S, L, P>, ValeurLue>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("les lectures notees d'une paie")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut m = BTreeMap::new();
                while let Some(k) = a.next_key::<String>()? {
                    let cle = CleDeLecture::lire(&k).map_err(serde::de::Error::custom)?;
                    let v = ValeurLue::lue(&cle, a.next_value::<String>()?).map_err(serde::de::Error::custom)?;
                    m.insert(cle, v);
                }
                Ok(m)
            }
        }
        d.deserialize_map(Visiteur(std::marker::PhantomData))
    }
}

/// UNE LECTURE NOTEE QUI PORTE UN JOUR OU UN MOIS : sa cle s'ecrit ici (`Display`) et se relit ici (`lue`), la seule
/// porte par laquelle la cle d'une empreinte ecrite redevient un jour ou un mois. `S` est la cle du salarie, `L` le
/// lieu d'une mission, dont les types vivent au-dessus du socle (`droit::contexte::IdSalarie`,
/// `droit::missions::LieuDeMission`) : la lecture les lit par eux (`TryFrom<String>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LectureDatee<S, L> {
    /// `pas <salarie> <jour> [<mois>]` : le taux de PAS du salarie a ce jour, pour la paie de ce mois.
    Pas { salarie: S, le: crate::date::Jour, mois: Option<crate::mois::Mois> },
    /// `mission <lieu> <jour>` : l'indemnite de mission d'un jour a l'etranger.
    Mission { lieu: L, jour: crate::date::Jour },
    /// `mensuels <mois> <salarie>` : les mensuels d'avant d'un salarie.
    Mensuels { mois: crate::mois::Mois, salarie: S },
    /// `paies <mois> <salarie>` : les paies validees d'un salarie au mois.
    Paies { mois: crate::mois::Mois, salarie: S },
    /// `saisie <mois> [<salarie>]` : la saisie du mois, ou celle d'un salarie.
    Saisie { mois: crate::mois::Mois, salarie: Option<S> },
}

impl<S: std::fmt::Display, L: std::fmt::Display> std::fmt::Display for LectureDatee<S, L> {
    /// La cle sous laquelle elle se note.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LectureDatee::Pas { salarie, le, mois: Some(m) } => write!(f, "pas {} {} {}", salarie, le.iso(), m),
            LectureDatee::Pas { salarie, le, mois: None } => write!(f, "pas {} {}", salarie, le.iso()),
            LectureDatee::Mission { lieu, jour } => write!(f, "mission {} {}", lieu, jour.iso()),
            LectureDatee::Mensuels { mois, salarie } => write!(f, "mensuels {} {}", mois, salarie),
            LectureDatee::Paies { mois, salarie } => write!(f, "paies {} {}", mois, salarie),
            LectureDatee::Saisie { mois, salarie: Some(s) } => write!(f, "saisie {} {}", mois, s),
            LectureDatee::Saisie { mois, salarie: None } => write!(f, "saisie {}", mois),
        }
    }
}

impl<S: TryFrom<String, Error = crate::echec::Echec>, L: TryFrom<String, Error = crate::echec::Echec>> LectureDatee<S, L> {
    /// La lecture que nomme la cle `cle` d'une empreinte ecrite ; `None` : ce n'en est pas une ; un jour, un mois, une
    /// cle de salarie ou un lieu qui ne se lit pas refuse.
    fn lue(cle: &str) -> Option<crate::echec::R<LectureDatee<S, L>>> {
        use crate::date::Jour;
        use crate::mois::Mois;
        let sal = |s: &str| S::try_from(s.to_string());
        if let Some(r) = cle.strip_prefix("pas ") {
            let mut parts = r.split(' ');
            let (salarie, jour, mois) = (parts.next()?, parts.next()?, parts.next());
            return Some((|| Ok(LectureDatee::Pas { salarie: sal(salarie)?, le: Jour::lire(jour)?, mois: mois.map(Mois::lire).transpose()? }))());
        }
        if let Some((lieu, jour)) = cle.strip_prefix("mission ").and_then(|r| r.split_once(' ')) {
            return Some((|| Ok(LectureDatee::Mission { lieu: L::try_from(lieu.to_string())?, jour: Jour::lire(jour)? }))());
        }
        if let Some((mois, salarie)) = cle.strip_prefix("mensuels ").and_then(|r| r.split_once(' ')) {
            return Some((|| Ok(LectureDatee::Mensuels { mois: Mois::lire(mois)?, salarie: sal(salarie)? }))());
        }
        if let Some((mois, salarie)) = cle.strip_prefix("paies ").and_then(|r| r.split_once(' ')) {
            return Some((|| Ok(LectureDatee::Paies { mois: Mois::lire(mois)?, salarie: sal(salarie)? }))());
        }
        let reste = cle.strip_prefix("saisie ")?;
        let (mois, salarie) = match reste.split_once(' ') {
            Some((m, s)) => (m, Some(s)),
            None => (reste, None),
        };
        Some((|| Ok(LectureDatee::Saisie { mois: Mois::lire(mois)?, salarie: salarie.map(sal).transpose()? }))())
    }
}

/// Ce que vaut un fichier pour l'empreinte : le hachage de son contenu, `absent` s'il n'existe pas, `illisible` (et la
/// raison) s'il ne se lit pas.
pub fn empreinte_fichier(chemin: &Path) -> ValeurLue {
    match std::fs::read(chemin) {
        Ok(o) => ValeurLue::Empreinte(empreinte_des_octets(chemin, o)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ValeurLue::Absent,
        Err(e) => ValeurLue::Illisible(e.to_string()),
    }
}

/// LE HACHAGE DES OCTETS `octets`, lus de `chemin`, fait une fois par contenu : les referentiels du code pesent plus
/// d'un megaoctet, et chaque calcul suivi, chaque empreinte relue d'une paie les nomme tous. Le hachage garde ne sert
/// que si les octets sont exactement ceux qu'il a haches : il ne se perime jamais.
fn empreinte_des_octets(chemin: &Path, octets: Vec<u8>) -> Empreinte {
    type Empreintes = Mutex<HashMap<PathBuf, (Vec<u8>, Empreinte)>>;
    static EMPREINTES: OnceLock<Empreintes> = OnceLock::new();
    let cache = EMPREINTES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((vus, e)) = crate::memoire::tenu(cache).get(chemin) {
        if *vus == octets {
            return e.clone();
        }
    }
    let e = Empreinte::des_octets(&octets);
    ranger(cache, chemin.to_path_buf(), (octets, e.clone()));
    e
}

fn noms(chemin: &Path) -> std::io::Result<Vec<String>> {
    let mut n = Vec::new();
    for e in std::fs::read_dir(chemin)? {
        let nom = e?.file_name().to_string_lossy().into_owned();
        if nom.ends_with(".json") {
            n.push(nom);
        }
    }
    n.sort();
    Ok(n)
}

/// L'empreinte d'un dossier : les noms de ses `.json`, tries ; `absent` s'il n'existe pas, `illisible` (et la raison)
/// s'il ne se lit pas : ni l'un ni l'autre n'est l'empreinte d'un dossier lu.
pub fn empreinte_dossier(chemin: &Path) -> ValeurLue {
    empreinte_des_noms(&noms(chemin))
}

fn empreinte_des_noms(noms: &std::io::Result<Vec<String>>) -> ValeurLue {
    match noms {
        Ok(n) => ValeurLue::Empreinte(Empreinte::des_octets(n.join("\n").as_bytes())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ValeurLue::Absent,
        Err(e) => ValeurLue::Illisible(e.to_string()),
    }
}

/// Un fichier servi par un cache : le hacher une fois si le registre ne l'a pas encore.
pub fn noter_si_besoin(chemin: &Path) {
    if suivi() {
        let k = cle_de(FICHIER, chemin);
        let connu = REGISTRE.with(|r| r.borrow().last().is_some_and(|g| g.entrees.contains_key(&k)));
        if !connu {
            noter(FICHIER, chemin, empreinte_fichier(chemin));
        }
    }
}

/// `std::fs::read_to_string`, note au registre.
pub fn lire_texte(chemin: &Path) -> std::io::Result<String> {
    match std::fs::read(chemin) {
        Ok(o) => {
            if suivi() {
                noter(FICHIER, chemin, ValeurLue::Empreinte(empreinte_des_octets(chemin, o.clone())));
            }
            String::from_utf8(o).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "stream did not contain valid UTF-8"))
        }
        Err(e) => {
            noter(FICHIER, chemin, ValeurLue::Absent);
            Err(e)
        }
    }
}

/// `Path::exists`, note.
pub fn existe(chemin: &Path) -> bool {
    let v = chemin.exists();
    noter(EXISTE, chemin, ValeurLue::OuiNon(v));
    v
}

/// `Path::is_dir`, note.
pub fn est_dossier(chemin: &Path) -> bool {
    let v = chemin.is_dir();
    noter(EST_DOSSIER, chemin, ValeurLue::OuiNon(v));
    v
}

/// Le chemin `chemin` relatif a `base`, s'il est dessous (`Path::strip_prefix`, composant par composant) ; `None` s'il
/// n'y est pas : un etat, pas une erreur.
pub fn sous<'a>(chemin: &'a Path, base: &Path) -> Option<&'a Path> {
    let mut reste = chemin.components();
    base.components().all(|b| reste.next() == Some(b)).then(|| reste.as_path())
}

/// `chemin` est-il sur le disque (un lien compte, sans le suivre) ? Le disque qui refuse de le dire se dit.
pub fn present(chemin: &Path) -> R<bool> {
    match std::fs::symlink_metadata(chemin) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(crate::echec::du_disque(chemin, &e)),
    }
}

/// EFFACE `chemin`, un fichier ou un dossier entier : un chemin qui n'existe pas est deja efface ; le disque qui refuse
/// se dit (`echec::du_disque`).
pub fn effacer(chemin: &Path) -> R<()> {
    effacer_du_disque(chemin).map_err(|e| crate::echec::du_disque(chemin, &e))
}

/// `effacer`, tel que le disque le dit.
pub(crate) fn effacer_du_disque(chemin: &Path) -> std::io::Result<()> {
    let fait = match std::fs::symlink_metadata(chemin) {
        Ok(m) if m.is_dir() => std::fs::remove_dir_all(chemin),
        Ok(_) => std::fs::remove_file(chemin),
        Err(e) => Err(e),
    };
    match fait {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        autre => autre,
    }
}

/// UN RESTE QU'ON EFFACE APRES COUP, quand ce qu'il fallait faire est fait (ou a la sortie d'une portee, ou rien ne
/// remonte) : un echec n'y defait rien ; il se note (`echec::noter_echec`, que les incidents lisent), et le reste s'efface
/// la fois suivante.
pub fn effacer_apres_coup(chemin: &Path) {
    if let Err(e) = effacer_du_disque(chemin) {
        crate::echec::noter_echec(Some(crate::echec::Classe::du_disque(&e)), &format!("Un reste ne s'efface pas ({}) : {}", chemin.display(), e));
    }
}

/// UN NETTOYAGE APRES UN ECHEC : `effacer(chemin)`, et l'echec `premier` rendu ; si le nettoyage lui-meme echoue, sa
/// phrase suit celle du premier, qui garde sa classe.
pub fn effacer_apres(premier: crate::echec::Echec, chemin: &Path) -> crate::echec::Echec {
    match effacer(chemin) {
        Ok(()) => premier,
        Err(nettoyage) => premier.reformuler(|p| format!("{}\n{}", p, nettoyage.phrase)),
    }
}

/// Les `.json` d'un dossier, tries ; `None` s'il n'existe pas ; le disque qui refuse se dit (`echec::du_disque`). Note :
/// ce qui apparait ou disparait dans le dossier change l'empreinte.
pub fn lister(dossier: &Path) -> R<Option<Vec<PathBuf>>> {
    let n = noms(dossier);
    noter(DOSSIER, dossier, empreinte_des_noms(&n));
    match n {
        Ok(n) => Ok(Some(n.into_iter().map(|x| dossier.join(x)).collect())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(crate::echec::du_disque(dossier, &e)),
    }
}

/// Ce que porte un dossier (fichiers et dossiers), trie par chemin ; `None` s'il n'existe pas ; le disque qui refuse
/// de le lire se dit (`echec::du_disque`). Rien n'est note : ce n'est pas une entree d'un calcul (`lister`).
pub fn entrees(dossier: &Path) -> R<Option<Vec<PathBuf>>> {
    let lus = match std::fs::read_dir(dossier) {
        Ok(lus) => lus,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(crate::echec::du_disque(dossier, &e)),
    };
    let mut v = Vec::new();
    for e in lus {
        v.push(e.map_err(|e| crate::echec::du_disque(dossier, &e))?.path());
    }
    v.sort();
    Ok(Some(v))
}

/// Un fichier ou un dossier qu'un dossier du disque porte sous le nom d'un mois : c'est la porte par laquelle le nom lu
/// sur le disque devient un mois (`Mois::du_nom`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NommeDUnMois {
    pub mois: crate::mois::Mois,
    pub chemin: PathBuf,
}

/// Ce que `dossier` porte sous le nom d'un mois, trie par chemin (aucun s'il n'existe pas) : les fichiers
/// `AAAA-MM.<extension>`, ou, sans extension, les dossiers `AAAA-MM`. `noter` : la lecture se note au registre du calcul
/// (`lister`) ; sinon, rien n'est note (`entrees`). Le disque qui refuse de le lire se dit.
pub fn nommes_d_un_mois(dossier: &Path, extension: Option<&str>, noter: bool) -> R<Vec<NommeDUnMois>> {
    let lus = if noter { lister(dossier)? } else { entrees(dossier)? };
    Ok(lus
        .into_iter()
        .flatten()
        .filter_map(|chemin| {
            let mois = match extension {
                Some(x) => chemin.extension().filter(|e| *e == x).and(chemin.file_stem()).and_then(crate::mois::Mois::du_nom),
                None => chemin.file_name().and_then(crate::mois::Mois::du_nom).filter(|_| chemin.is_dir()),
            };
            mois.map(|mois| NommeDUnMois { mois, chemin })
        })
        .collect())
}

/// Le dossier des referentiels livres (`donnees/`) : `PUPPETPAIE_CODE`, que tout programme pose au demarrage
/// (`compagnie::installation::fixer_le_code`, qui refuse en la nommant s'il ne trouve aucun dossier). Non posee, c'est un
/// test de `cargo` qui lit le code dans son processus, sans programme : celui du depot qui le compile.
pub fn dossier_du_code() -> PathBuf {
    match std::env::var_os("PUPPETPAIE_CODE") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees"),
    }
}

/// La signature d'un fichier : (date de modification en ns, taille), ou `None`.
///
/// LE CODE LIVRE NE BOUGE PAS PENDANT UNE ROUTE : le temps d'une route (`code_fige`), la signature d'un
/// fichier du code (`dossier_du_code`) se lit une fois. Les caches du moteur la demandaient a chaque service -- mille
/// fois `norme-dsn-base.json` pour un seul `etat` --, et sous Windows chaque demande ouvre le fichier, que l'antivirus
/// inspecte. Les fichiers de la compagnie, eux, se relisent toujours : une route les ecrit.
pub fn signature(chemin: &Path) -> R<Option<(u128, u64)>> {
    let memo = CODE_FIGE.with(|c| c.borrow().as_ref().filter(|(code, _)| chemin.starts_with(code)).map(|(_, m)| m.get(chemin).copied()));
    match memo {
        Some(Some(s)) => Ok(s),
        Some(None) => {
            let s = signature_lue(chemin)?;
            CODE_FIGE.with(|c| {
                if let Some((_, m)) = c.borrow_mut().as_mut() {
                    m.insert(chemin.to_path_buf(), s);
                }
            });
            Ok(s)
        }
        None => signature_lue(chemin),
    }
}

/// `None` pour un fichier absent : rien ne se garde de lui ; un fichier qui ne dit pas sa date (illisible, d'avant 1970)
/// se refuse.
fn signature_lue(chemin: &Path) -> R<Option<(u128, u64)>> {
    // Un fichier absent n'a pas de signature : rien ne se garde de lui.
    if !chemin.is_file() {
        return Ok(None);
    }
    let a_part = |detail: String| crate::echec::Lieu { chemin: Some(chemin.to_string_lossy().into_owned()), detail: Some(detail), ..Default::default() };
    let illisible = |e: std::io::Error| crate::echec::refus_avec("Un fichier ne dit pas sa date.", a_part(e.to_string()));
    let m = std::fs::metadata(chemin).map_err(illisible)?;
    let d = m.modified().map_err(illisible)?.duration_since(std::time::UNIX_EPOCH).map_err(|e| {
        crate::echec::refus_avec("Un fichier est daté d'avant 1970 : régler la date du fichier.", a_part(format!("{} s avant 1970", e.duration().as_secs())))
    })?;
    Ok(Some((d.as_nanos(), m.len())))
}

type SignaturesDuCode = Option<(PathBuf, HashMap<PathBuf, Option<(u128, u64)>>)>;

thread_local! {
    static CODE_FIGE: RefCell<SignaturesDuCode> = const { RefCell::new(None) };
}

/// Le temps d'une route, le code livre est fige (`signature`). Rendu par `code_fige`, il le libere en tombant ; une
/// route lue depuis une autre ne fige rien de plus.
pub struct CodeFige(bool);

#[must_use = "le code n'est fige que tant que son garde vit"]
pub fn code_fige() -> CodeFige {
    CodeFige(CODE_FIGE.with(|c| {
        let mut c = c.borrow_mut();
        if c.is_some() {
            return false;
        }
        *c = Some((dossier_du_code(), HashMap::new()));
        true
    }))
}

impl Drop for CodeFige {
    fn drop(&mut self) {
        if self.0 {
            CODE_FIGE.with(|c| *c.borrow_mut() = None);
        }
    }
}

/// Le cache d'une table livree lue par sa forme (`forme_en_cache`) : le chemin, sa signature, la table construite.
pub type FormeGardee<T> = Mutex<Option<(PathBuf, Option<(u128, u64)>, Arc<T>)>>;

/// UNE TABLE LIVREE (`donnees/<nom>`), lue par sa forme `B` (`forme::lire_fichier`, `phrase` pour son refus) et
/// construite par `construire` une fois tant que son fichier ne change pas (chemin, date, taille) ; servie du cache, elle
/// se note quand meme au registre d'un calcul suivi. Un calcul la consulte a chaque champ : elle ne se relit pas a chaque
/// fois.
#[track_caller]
pub fn forme_en_cache<B: serde::de::DeserializeOwned, T>(cache: &'static OnceLock<FormeGardee<T>>, nom: &str, phrase: &str, construire: impl Fn(B) -> R<T>) -> R<Arc<T>> {
    let chemin = dossier_du_code().join(nom);
    let sig = signature(&chemin)?;
    let cache = cache.get_or_init(|| Mutex::new(None));
    if let Some((p, s, v)) = crate::memoire::tenu(cache).as_ref() {
        if *p == chemin && sig.is_some() && *s == sig {
            noter_si_besoin(&chemin);
            return Ok(v.clone());
        }
    }
    let v = Arc::new(construire(crate::forme::lire_fichier(&chemin, phrase)?)?);
    *crate::memoire::tenu(cache) = Some((chemin, sig, v.clone()));
    Ok(v)
}

/// Un texte cite dans un message : entre guillemets simples (doubles s'il porte une apostrophe et aucun guillemet),
/// echappe.
pub fn repr(s: &str) -> String {
    let guillemet = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::from(guillemet);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == guillemet => {
                out.push('\\');
                out.push(c);
            }
            c if !imprimable(c) => {
                let n = c as u32;
                if n < 0x100 {
                    out.push_str(&format!("\\x{:02x}", n));
                } else if n < 0x10000 {
                    out.push_str(&format!("\\u{:04x}", n));
                } else {
                    out.push_str(&format!("\\U{:08x}", n));
                }
            }
            c => out.push(c),
        }
    }
    out.push(guillemet);
    out
}

/// Ni controle, ni separateur (hors espace), ni format,
/// ni usage prive. Les points de code non attribues ne sont pas connus ici.
pub fn imprimable(c: char) -> bool {
    let n = c as u32;
    if c == ' ' {
        return true;
    }
    !(c.is_control()
        || c.is_whitespace()
        || matches!(n, 0xad | 0x600..=0x605 | 0x61c | 0x6dd | 0x70f | 0x8e2 | 0x180e | 0x200b..=0x200f | 0x202a..=0x202e
            | 0x2060..=0x2064 | 0x2066..=0x206f | 0xfeff | 0xfff9..=0xfffb | 0x110bd | 0x1d173..=0x1d17a | 0xe0001 | 0xe0020..=0xe007f
            | 0xe000..=0xf8ff | 0xf0000..=0x10ffff | 0xfdd0..=0xfdef)
        || n & 0xfffe == 0xfffe)
}

// ------------------------------------------------ les refus d'une donnee lue
//
// Un fichier ou une demande dont un champ manque ou n'a pas la forme attendue :
// la phrase nomme le champ, ce qui etait attendu et ce qui a ete lu.

/// « champ 'prenom' absent ».
#[track_caller]
pub fn absent(cle: &str) -> Echec {
    crate::echec::cle("Il manque une donnée attendue.", cle)
}

/// Une valeur dite en francais, depuis son nom de type (`NoneType`, `bool`, `int`, `float`, `str`, `list`, `dict`).
pub fn nature(nom_type: &str) -> &'static str {
    match nom_type {
        "dict" => "un objet",
        "list" | "tuple" => "une liste",
        "str" => "un texte",
        "int" => "un nombre entier",
        "float" => "un nombre",
        "bool" => "un booléen",
        "NoneType" => "null",
        _ => "une valeur d'une autre forme",
    }
}

/// « un objet est attendu, lu : une liste ».
pub fn attendu(quoi: &str, nom_type: &str) -> String {
    let accord = if quoi.starts_with("une ") { "e" } else { "" };
    format!("{} est attendu{}, lu : {}", quoi, accord, nature(nom_type))
}

/// « liste trop courte : <ce qui y etait cherche> ».
pub fn trop_courte(quoi: &str) -> String {
    format!("liste trop courte : {}", quoi)
}

/// Un nombre que l'on voudrait entier et qui ne tient pas : infini, ou hors de ce qu'un entier porte.
pub fn entier_hors_limites(n: &str) -> String {
    format!("nombre entier hors limites : {}", n)
}

/// Le refus du disque : « fichier introuvable : '<chemin>' », « accès refusé : '<chemin>' »,
/// sinon « fichier illisible : '<chemin>' : <cause du systeme> ».
pub fn panne_fichier(e: &std::io::Error, chemin: &Path) -> String {
    let c = repr(&chemin.to_string_lossy());
    match e.kind() {
        std::io::ErrorKind::NotFound => format!("fichier introuvable : {}", c),
        std::io::ErrorKind::PermissionDenied => format!("accès refusé : {}", c),
        _ if chemin.is_dir() => format!("un dossier, pas un fichier : {}", c),
        _ => format!("fichier illisible : {} : {}", c, e),
    }
}

/// « fichier introuvable : '<chemin>' ».
pub fn introuvable(chemin: &Path) -> String {
    format!("fichier introuvable : {}", repr(&chemin.to_string_lossy()))
}

/// « écriture impossible : '<chemin>' : <cause du systeme> ».
pub fn ecriture_impossible(e: &std::io::Error, chemin: &Path) -> String {
    format!("écriture impossible : {} : {}", repr(&chemin.to_string_lossy()), e)
}

/// Le dossier de la configuration d'une compagnie, sous sa racine.
pub const CONFIG: &str = "config";

/// Le marqueur d'une compagnie qui n'est pas la vraie (une compagnie fictive, une copie de developpement).
pub const MARQUEUR_COPIE: &str = "COPIE-DE-DEVELOPPEMENT.json";

/// Le modele `nom` (gabarit de contrat, de DPAE, de certificat) : celui de la compagnie (son dossier `gabarits`),
/// a defaut celui du poste (`poste`), a defaut celui livre avec le logiciel (`donnees/modeles/`). Une compagnie qui
/// ecrit ses propres clauses passe devant ; une copie de la configuration du poste sans son dossier de gabarits reste la
/// meme compagnie et garde ceux du poste (son gabarit DPAE porte son centre de medecine du travail). Une compagnie qui
/// porte `MARQUEUR_COPIE` n'emprunte rien au poste : ses documents ne prennent jamais les clauses de la compagnie reelle.
pub fn modele_de(nom: &str, gabarits: Option<&Path>, poste: &Path) -> PathBuf {
    let propre = match gabarits {
        Some(g) => g.to_path_buf(),
        None => poste.to_path_buf(),
    };
    let copie = propre.parent().is_some_and(|p| p.join(MARQUEUR_COPIE).is_file());
    let dossiers: Vec<&Path> = if copie { vec![&propre] } else { vec![&propre, poste] };
    for dossier in dossiers {
        if dossier.join(nom).exists() {
            return dossier.join(nom);
        }
    }
    dossier_du_code().join("modeles").join(nom)
}

/// LE NOM D'UN FICHIER, seul, sans son dossier : le fichier parti (`Contrat_ALBARET_AL3.26.01_Mai2026.pdf`), le modele
/// d'un contrat (`contrat-cddu.modele`), l'archive d'une version (`puppetpaie-2026.11.12-windows.zip`). Il n'est pas
/// vide, ne porte ni `/`, ni `\`, ni caractere nul, et n'est ni `.` ni `..` : joint a un dossier, il y reste. Il s'ecrit
/// tel quel et se trie par son texte.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct NomDeFichier(std::borrow::Cow<'static, str>);

impl NomDeFichier {
    /// Le nom `texte`, ou la phrase qui le refuse.
    pub fn lire(texte: &str) -> R<NomDeFichier> {
        if !NomDeFichier::a_sa_forme(texte) {
            return Err(crate::refus!("{} n'est pas un nom de fichier : un nom seul, sans dossier.", repr(texte)));
        }
        Ok(NomDeFichier(std::borrow::Cow::Owned(texte.to_string())))
    }

    const fn a_sa_forme(texte: &str) -> bool {
        let o = texte.as_bytes();
        if o.is_empty() || matches!(o, b"." | b"..") {
            return false;
        }
        let mut i = 0;
        while i < o.len() {
            if matches!(o[i], b'/' | b'\\' | 0) {
                return false;
            }
            i += 1;
        }
        true
    }

    /// LE NOM QUE LE MOTEUR NOMME, ecrit en Rust : une constante, dont le compilateur refuse une forme fausse.
    pub const fn du_moteur(nom: &'static str) -> NomDeFichier {
        assert!(NomDeFichier::a_sa_forme(nom), "nom de fichier mal forme");
        NomDeFichier(std::borrow::Cow::Borrowed(nom))
    }

    /// LE NOM DU FICHIER `chemin` ; un chemin qui ne finit pas par un nom (`/`, `..`) se refuse. Un nom qui n'est pas en
    /// UTF-8 garde son caractere illisible (`to_string_lossy`).
    pub fn du(chemin: &Path) -> R<NomDeFichier> {
        match chemin.file_name() {
            Some(n) => NomDeFichier::lire(&n.to_string_lossy()),
            None => Err(crate::refus!("{} ne nomme pas un fichier.", repr(&chemin.to_string_lossy()))),
        }
    }

    /// Le nom tel qu'ecrit.
    pub fn texte(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NomDeFichier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<Path> for NomDeFichier {
    fn as_ref(&self) -> &Path {
        Path::new(self.texte())
    }
}

impl TryFrom<String> for NomDeFichier {
    type Error = Echec;
    fn try_from(s: String) -> R<NomDeFichier> {
        NomDeFichier::lire(&s)
    }
}

impl From<NomDeFichier> for String {
    fn from(n: NomDeFichier) -> String {
        n.0.into_owned()
    }
}

/// UN CHEMIN RELATIF a une racine que son champ dit (la compagnie, le dossier trie d'un import) :
/// `retours/depots/20260203-100000_depot-dsn_202601.txt`, `sorties/2026-06/DSN_….dsn`. Il n'est pas vide, n'a pas de
/// racine, et chacun de ses noms est un `NomDeFichier` (ni `.` ni `..`) : joint a sa racine, il y reste. IL S'ECRIT AVEC
/// DES BARRES DROITES, sur tous les systemes : une cle notee avec la barre de Windows ne se retrouverait pas
/// (`racine:engagements/AAAA-MM.json`, `engagement::empreinte`), et Windows accepte la barre droite pour le rejoindre.
/// Il s'ecrit tel quel et se trie par son texte.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct CheminRelatif(std::borrow::Cow<'static, str>);

impl CheminRelatif {
    /// Le chemin `texte`, ou la phrase qui le refuse.
    pub fn lire(texte: &str) -> R<CheminRelatif> {
        if !CheminRelatif::a_sa_forme(texte) {
            return Err(crate::refus!("{} n'est pas un chemin relatif : des noms separes par des barres droites.", repr(texte)));
        }
        Ok(CheminRelatif(std::borrow::Cow::Owned(texte.to_string())))
    }

    /// Des noms (`NomDeFichier::a_sa_forme`) separes par des barres droites.
    const fn a_sa_forme(texte: &str) -> bool {
        let o = texte.as_bytes();
        let (mut debut, mut i) = (0, 0);
        while i <= o.len() {
            if i == o.len() || o[i] == b'/' {
                let n = i - debut;
                if n == 0 || (n == 1 && o[debut] == b'.') || (n == 2 && o[debut] == b'.' && o[debut + 1] == b'.') {
                    return false;
                }
                debut = i + 1;
            } else if matches!(o[i], b'\\' | 0) {
                return false;
            }
            i += 1;
        }
        true
    }

    /// LE CHEMIN QUE LE MOTEUR NOMME, ecrit en Rust : une constante, dont le compilateur refuse une forme fausse.
    pub const fn du_moteur(chemin: &'static str) -> CheminRelatif {
        assert!(CheminRelatif::a_sa_forme(chemin), "chemin relatif mal forme");
        CheminRelatif(std::borrow::Cow::Borrowed(chemin))
    }

    /// LE CHEMIN DE `chemin` SOUS `racine` ; un chemin qui n'y est pas, ou qui est la racine elle-meme, se refuse.
    pub fn sous(racine: &Path, chemin: &Path) -> R<CheminRelatif> {
        match sous(chemin, racine) {
            Some(rel) => CheminRelatif::du(rel),
            None => Err(crate::refus!("{} n'est pas sous {}.", repr(&chemin.to_string_lossy()), repr(&racine.to_string_lossy()))),
        }
    }

    /// LE CHEMIN RELATIF `rel`, ecrit avec des barres droites ; un chemin qui a une racine, ou qui remonte (`..`), se
    /// refuse.
    pub fn du(rel: &Path) -> R<CheminRelatif> {
        CheminRelatif::lire(&CheminRelatif::en_barres(rel))
    }

    /// Le chemin relatif `rel`, ecrit avec des barres droites ; un nom qui n'est pas en UTF-8 garde son caractere
    /// illisible (`to_string_lossy`).
    pub(crate) fn en_barres(rel: &Path) -> String {
        rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
    }

    /// LE DOSSIER DES PIECES DU MOIS `mois` : `sorties/AAAA-MM`.
    pub fn sorties(mois: crate::mois::Mois) -> CheminRelatif {
        CheminRelatif(std::borrow::Cow::Owned(format!("sorties/{}", mois)))
    }

    /// Ce chemin, suivi du nom `nom`.
    pub fn joint(&self, nom: &NomDeFichier) -> CheminRelatif {
        CheminRelatif(std::borrow::Cow::Owned(format!("{}/{}", self.0, nom)))
    }

    /// Le chemin du dossier qui le contient ; `None` pour un nom seul, a la racine.
    pub fn parent(&self) -> Option<CheminRelatif> {
        self.0.rsplit_once('/').map(|(d, _)| CheminRelatif(std::borrow::Cow::Owned(d.to_string())))
    }

    /// Son dernier nom : celui du fichier.
    pub fn nom(&self) -> NomDeFichier {
        let nom = match self.0.rsplit_once('/') {
            Some((_, n)) => n,
            None => &self.0,
        };
        NomDeFichier(std::borrow::Cow::Owned(nom.to_string()))
    }

    /// Le chemin tel qu'ecrit.
    pub fn texte(&self) -> &str {
        &self.0
    }
}

impl From<NomDeFichier> for CheminRelatif {
    fn from(n: NomDeFichier) -> CheminRelatif {
        CheminRelatif(n.0)
    }
}

impl std::fmt::Display for CheminRelatif {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<Path> for CheminRelatif {
    fn as_ref(&self) -> &Path {
        Path::new(self.texte())
    }
}

impl TryFrom<String> for CheminRelatif {
    type Error = Echec;
    fn try_from(s: String) -> R<CheminRelatif> {
        CheminRelatif::lire(&s)
    }
}

impl From<CheminRelatif> for String {
    fn from(c: CheminRelatif) -> String {
        c.0.into_owned()
    }
}

/// L'EMPREINTE D'UN CONTENU : son SHA-256 entier, 64 caracteres hexadecimaux minuscules (une source deposee, une
/// archive de mise a jour, un exercice clos, un etat garde). Elle ne se fait que par `des_octets` (ou `du_hachage`, pour
/// un contenu hache par morceaux) ; lue, elle refuse une autre forme. Elle s'ecrit telle quelle et se trie par son texte.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Empreinte(String);

impl Empreinte {
    /// L'empreinte des octets `octets`.
    pub fn des_octets(octets: &[u8]) -> Empreinte {
        Empreinte::du_hachage(Sha256::new_with_prefix(octets))
    }

    /// L'empreinte d'un contenu hache par morceaux dans `h`.
    pub fn du_hachage(h: Sha256) -> Empreinte {
        Empreinte(hexa(&h.finalize()))
    }

    /// L'empreinte du contenu du fichier `chemin`, ou la panne qui empeche de le lire.
    pub fn du_fichier(chemin: &Path) -> R<Empreinte> {
        std::fs::read(chemin).map(|o| Empreinte::des_octets(&o)).map_err(|e| crate::echec::du_disque(chemin, &e))
    }

    /// L'empreinte `texte`, ou la phrase qui la refuse.
    pub fn lire(texte: &str) -> R<Empreinte> {
        if !est_hexa(texte, 64) {
            return Err(crate::refus!("{} n'est pas une empreinte : 64 caracteres hexadecimaux minuscules.", repr(texte)));
        }
        Ok(Empreinte(texte.to_string()))
    }

    /// SES 16 PREMIERS CARACTERES : ce que le journal ecrit d'un fichier parti.
    pub fn courte(&self) -> EmpreinteCourte {
        EmpreinteCourte(self.0[..16].to_string())
    }

    /// L'empreinte telle qu'ecrite.
    pub fn texte(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Empreinte {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Empreinte {
    type Error = Echec;
    fn try_from(s: String) -> R<Empreinte> {
        Empreinte::lire(&s)
    }
}

impl From<Empreinte> for String {
    fn from(e: Empreinte) -> String {
        e.0
    }
}

/// L'EMPREINTE COURTE D'UN CONTENU : les 16 premiers caracteres de son SHA-256, ce que le journal des envois ecrit d'un
/// fichier parti et d'un modele fige, ce qui reconnait une paie importee deja comparee, une DPAE due, l'etat d'un mois.
/// Elle ne se fait que par `Empreinte::courte` ; lue, elle refuse une autre forme ; on n'en retrouve pas l'entiere.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct EmpreinteCourte(String);

impl EmpreinteCourte {
    /// L'empreinte courte `texte`, ou la phrase qui la refuse.
    pub fn lire(texte: &str) -> R<EmpreinteCourte> {
        if !est_hexa(texte, 16) {
            return Err(crate::refus!("{} n'est pas une empreinte courte : 16 caracteres hexadecimaux minuscules.", repr(texte)));
        }
        Ok(EmpreinteCourte(texte.to_string()))
    }

    /// L'empreinte courte telle qu'ecrite.
    pub fn texte(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EmpreinteCourte {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for EmpreinteCourte {
    type Error = Echec;
    fn try_from(s: String) -> R<EmpreinteCourte> {
        EmpreinteCourte::lire(&s)
    }
}

impl From<EmpreinteCourte> for String {
    fn from(e: EmpreinteCourte) -> String {
        e.0
    }
}

/// Les octets `octets` en hexadecimal minuscule : l'ecriture d'une empreinte.
pub fn hexa(octets: &[u8]) -> String {
    octets.iter().map(|o| format!("{:02x}", o)).collect()
}

/// `texte` est-il fait de `longueur` caracteres hexadecimaux minuscules : la forme d'une empreinte ecrite.
pub fn est_hexa(texte: &str, longueur: usize) -> bool {
    texte.len() == longueur && texte.bytes().all(|o| matches!(o, b'0'..=b'9' | b'a'..=b'f'))
}

/// CE QUI S'ECRIT EN CHEMIN dans une reponse : un chemin du disque, absent ou non, seul ou en liste.
pub trait EcritEnChemin {
    fn ecrire<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error>;
}

impl EcritEnChemin for PathBuf {
    fn ecrire<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string_lossy())
    }
}

impl<C: EcritEnChemin> EcritEnChemin for Option<C> {
    fn ecrire<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Some(c) => c.ecrire(s),
            None => s.serialize_none(),
        }
    }
}

impl<C: EcritEnChemin> EcritEnChemin for Vec<C> {
    fn ecrire<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        struct Un<'a, C>(&'a C);
        impl<C: EcritEnChemin> serde::Serialize for Un<'_, C> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                self.0.ecrire(s)
            }
        }
        let mut seq = s.serialize_seq(Some(self.len()))?;
        for c in self {
            seq.serialize_element(&Un(c))?;
        }
        seq.end()
    }
}

/// UN CHEMIN DU DISQUE RENDU PAR UNE ROUTE (`#[serde(serialize_with = "socle::fichiers::ecrire_chemin")]`) : son texte,
/// un nom qui n'est pas en UTF-8 gardant son caractere illisible (`to_string_lossy`), pour que la reponse parte quand
/// meme : la compagnie peut etre rangee dans un tel dossier.
pub fn ecrire_chemin<C: EcritEnChemin, S: serde::Serializer>(c: &C, s: S) -> Result<S::Ok, S::Error> {
    c.ecrire(s)
}

#[cfg(test)]
mod essais {
    use super::*;

    /// Un calcul suivi qui en fait un autre (la paie de juin d'un contrat sans terme lit celle de mai, qui se calcule) :
    /// le calcul de dessous garde son registre, et ce que celui du dessus a lu s'y ajoute ; sinon la paie s'ecrirait avec
    /// la seule marque du moteur, et ne se recalculerait plus quand ce qu'elle lit change.
    #[test]
    fn un_calcul_dans_un_autre_rend_son_registre_a_celui_qui_l_a_demande() {
        type Cle = CleDeLecture<NomDeFichier, NomDeFichier, NomDeFichier>;
        let saisie = |m: &str| -> R<Cle> { Ok(CleDeLecture::Datee(LectureDatee::Saisie { mois: crate::mois::Mois::lire(m)?, salarie: None })) };
        let textes = |r: R<BTreeMap<Cle, ValeurLue>>| -> R<Vec<String>> { Ok(r?.keys().map(|k| k.to_string()).collect()) };
        let (racine, code) = (Path::new("/compagnie"), Path::new("/code"));
        let marque = ValeurLue::Moteur("m".into());
        suivre(racine, code);
        noter_lecture(&saisie("2026-06").expect("mois"), marque.clone());
        suivre(racine, code);
        noter_lecture(&saisie("2026-05").expect("mois"), marque.clone());
        let dessus = textes(rendre());
        noter_lecture(&Cle::Moteur, marque);
        let dessous = textes(rendre());
        assert_eq!(dessus.expect("rendu"), ["saisie 2026-05"]);
        assert_eq!(dessous.expect("rendu"), ["moteur", "saisie 2026-05", "saisie 2026-06"]);
        assert!(!registre_ouvert());
    }
}
