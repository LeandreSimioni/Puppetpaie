//! LE RELEVE D'UN PDF, UNE FOIS PAR GESTE. Le tri (`tri.rs`) et chaque lecteur (`bulletin_lu`, `certificat_lu`,
//! `aem_lu`) lisent les pages que la fabrique releve (`fabrique_pdf::inspection::inspecter`) : pendant une route d'import
//! (`Releves::ouvrir`), chaque fichier se releve une fois, et tous reprennent ce releve tant que le fichier est le meme
//! (son chemin, sa taille, sa date). Hors d'une route d'import, chaque lecture releve le fichier.
//!
//! LE RELEVE SE LIT EN SA FORME (`Inspection`), celle que la fabrique ecrit (`fabrique-pdf/src/inspection.rs`) : une cle
//! qu'elle n'ecrit pas, ou qui manque, refuse le releve.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

/// Une boite sur la page : x0, y0, x1, y1, dans le repere de la page non tournee.
pub type Boite = [f64; 4];

/// CE QUE LA FABRIQUE RELEVE D'UN PDF (`fabrique_pdf::inspection`) : son chiffrement, ses metadonnees, ses pages.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inspection {
    pub chiffrement: Option<Chiffrement>,
    pub metadonnees: BTreeMap<String, String>,
    pub pages: Vec<PageRelevee>,
}

/// Le chiffrement d'un PDF, tel que la fabrique le dit.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chiffrement {
    pub filtre: String,
    #[serde(rename = "V")]
    pub v: i64,
    #[serde(rename = "R")]
    pub r: i64,
    pub longueur: i64,
    #[serde(rename = "P")]
    pub p: i64,
    pub methode: String,
}

/// Une page relevee : sa taille, sa rotation, ses polices, ses images, ses traits et ses mots.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageRelevee {
    pub largeur: f64,
    pub hauteur: f64,
    pub rotation: i64,
    pub polices: Vec<String>,
    pub images: Vec<Boite>,
    pub traits: Vec<Boite>,
    pub mots: Vec<MotReleve>,
}

/// Un mot de la page, sa boite et sa ligne.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotReleve {
    pub texte: String,
    pub rect: Boite,
    pub ligne: usize,
}

/// Ce que la fabrique releve d'un PDF : ses pages et ce qu'elle en signale, page par page.
pub struct Releve {
    pub releve: Inspection,
    pub avertissements: Vec<String>,
}

/// Pourquoi un PDF ne se releve pas.
pub enum Refus {
    /// La fabrique ne l'ouvre pas (sa raison, qui commence par le chemin).
    Fabrique(String),
    /// Le releve rendu ne se lit pas.
    Releve(String),
}

impl Refus {
    /// La phrase des lecteurs.
    pub fn phrase(&self) -> String {
        match self {
            Refus::Fabrique(r) => format!("PDF que la fabrique n'ouvre pas : {}", r),
            Refus::Releve(e) => format!("releve du PDF illisible ({})", e),
        }
    }
}

/// Le fichier releve : son chemin, sa taille, sa date.
type Cle = (PathBuf, u64, SystemTime);

thread_local! {
    static MEMO: RefCell<(u32, HashMap<Cle, Rc<Releve>>)> = RefCell::new((0, HashMap::new()));
}

/// Une route d'import ouverte : chaque fichier s'y releve une fois ; le memo se vide quand la derniere se ferme.
pub struct Releves;

impl Releves {
    pub fn ouvrir() -> Releves {
        MEMO.with(|m| m.borrow_mut().0 += 1);
        Releves
    }
}

impl Drop for Releves {
    fn drop(&mut self) {
        MEMO.with(|m| {
            let mut m = m.borrow_mut();
            m.0 -= 1;
            if m.0 == 0 {
                m.1.clear();
            }
        });
    }
}

/// Le releve du PDF `chemin`. Un fichier dont la taille ou la date ne se lisent pas ne s'ouvre pas : la fabrique le dit.
pub fn du_pdf(chemin: &Path) -> Result<Rc<Releve>, Refus> {
    let meta = std::fs::metadata(chemin).map_err(|e| Refus::Fabrique(format!("{} illisible ({})", chemin.display(), e)))?;
    let date = meta.modified().map_err(|e| Refus::Fabrique(format!("{} illisible ({})", chemin.display(), e)))?;
    let cle: Cle = (chemin.to_path_buf(), meta.len(), date);
    if let Some(r) = MEMO.with(|m| m.borrow().1.get(&cle).cloned()) {
        return Ok(r);
    }
    let (texte, avertissements) = fabrique_pdf::inspection::inspecter(&chemin.to_string_lossy()).map_err(|r| Refus::Fabrique(r.0))?;
    let releve: Inspection = socle::forme::lire_texte(&texte, "Le relevé du PDF n'a pas la forme que la fabrique écrit.").map_err(Refus::Releve)?;
    let r = Rc::new(Releve { releve, avertissements });
    MEMO.with(|m| {
        let mut m = m.borrow_mut();
        if m.0 > 0 {
            m.1.insert(cle, r.clone());
        }
    });
    Ok(r)
}
