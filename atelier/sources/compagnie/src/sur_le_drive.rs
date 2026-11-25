//! CE QUE LE COMPTE GOOGLE LIE PORTE SOUS `PuppetPaie/` : chaque dossier, ce qu'il est (le dossier d'une compagnie, par
//! sa marque ; l'ancien dossier commun ; un autre dossier d'une ancienne version), la date de sa derniere sauvegarde et
//! son volume quand sa marque les dit. La portee `drive.file` ne laisse voir que ce que l'application a cree
//! (`envois::drive`). Ce que la vue du Drive, la reprise d'une compagnie et la suppression d'un dossier lisent
//! (`routes::drive`, `routes::accueil::drive`), et la restauration en ligne de commande.

use envois::drive::{Drive, ErreurGoogle, IdDrive, MarqueDuDossier};
use socle::grandeurs::Octets;
use socle::paris::Moment;

use crate::sauvegarde::{derniere_notee, IdentiteSurLeDrive, LaMain, SAUVEGARDE};

/// CE QU'EST UN DOSSIER SOUS LA RACINE DU DRIVE : le dossier d'une compagnie (sa marque dit son identite), l'ancien
/// dossier commun (`sauvegarde`, ou toutes les compagnies d'un meme compte ecrivaient, homonymes compris), ou un autre
/// dossier sans marque (une archive de mois d'une ancienne version).
#[derive(Debug, Clone, PartialEq)]
pub enum GenreDuDossier {
    Compagnie(IdentiteSurLeDrive),
    AncienneSauvegarde,
    Ancien,
}

/// UN DOSSIER SOUS LA RACINE DU DRIVE, tel que la vue le montre : son identifiant, son nom, ce qu'il est, la date de sa
/// derniere sauvegarde et son volume (ce que sa marque dit ; rien sans marque), et ce qui ne s'en lit pas.
#[derive(Debug, Clone)]
pub struct DossierVu {
    pub id: IdDrive,
    pub nom: String,
    pub genre: GenreDuDossier,
    pub derniere: Option<Moment>,
    pub octets: Option<Octets>,
    /// La main sur le dossier (`LaMain`) : aucune quand sa marque n'en dit rien ; `Err` : elle ne se lit pas, et pourquoi,
    /// detail compris, que `sauvegarde::main_illisible_dite` dit (illisible n'est pas aucune : la sauvegarde ne part pas,
    /// `SansLaMain::Illisible`).
    pub main: Result<Option<LaMain>, socle::echec::Echec>,
    /// Ce qui ne se lit pas dans sa marque, une phrase par chose, en mots : le dossier se montre quand meme. La main
    /// illisible n'y est pas : `main` la dit.
    pub illisible: Vec<String>,
}

impl DossierVu {
    /// Se reprend-il comme une compagnie : le dossier d'une compagnie, ou l'ancien dossier commun ?
    pub fn se_reprend(&self) -> bool {
        matches!(self.genre, GenreDuDossier::Compagnie(_) | GenreDuDossier::AncienneSauvegarde)
    }
}

/// Le dossier `d`, lu depuis sa marque : ce qui ne s'en lit pas se dit dans `illisible`, sans fermer la vue.
fn vu(d: envois::drive::SousDossier) -> DossierVu {
    // Sans marque, le dossier ne dit ni identite, ni date, ni volume, ni main.
    let mut illisible = Vec::new();
    let main = LaMain::de(d.marque.as_ref());
    let derniere = match derniere_notee(d.marque.as_ref()) {
        Ok(m) => m,
        Err(_) => {
            illisible.push("La date de sa dernière sauvegarde, notée sur le Drive, ne se lit pas.".to_string());
            None
        }
    };
    let (compagnie, octets) = match d.marque {
        Some(MarqueDuDossier { compagnie, octets, .. }) => (compagnie, octets),
        None => (None, None),
    };
    let genre = match compagnie.map(|c| IdentiteSurLeDrive::lire(&c)) {
        Some(Ok(i)) => GenreDuDossier::Compagnie(i),
        Some(Err(e)) => {
            illisible.push(format!("Sa marque ne se lit pas : {}", e.phrase));
            GenreDuDossier::Ancien
        }
        None if d.nom == SAUVEGARDE => GenreDuDossier::AncienneSauvegarde,
        None => GenreDuDossier::Ancien,
    };
    let octets = octets.and_then(|t| match Octets::ecrite(&t) {
        Some(o) => Some(o),
        None => {
            illisible.push("Son volume, noté sur le Drive, ne se lit pas.".to_string());
            None
        }
    });
    DossierVu { id: d.id, nom: d.nom, genre, derniere, octets, main, illisible }
}

/// LES DOSSIERS SOUS LA RACINE `racine_drive` DU DRIVE DE `client`, dans l'ordre ou Drive les rend, de toutes les racines
/// de ce nom (deux postes qui l'ont creee en meme temps en laissent deux). Ne cree rien : sans racine, aucun dossier.
pub fn voir(client: &mut dyn Drive, racine_drive: &str) -> Result<Vec<DossierVu>, ErreurGoogle> {
    let mut vus = Vec::new();
    for racine in client.sous_dossiers(&IdDrive::RACINE)?.into_iter().filter(|d| d.nom == racine_drive) {
        vus.extend(client.sous_dossiers(&racine.id)?.into_iter().map(vu));
    }
    Ok(vus)
}

/// Le dossier `id` de la vue `vus` ; un dossier qu'elle ne montre pas se refuse : rien ne se fait sur lui.
pub fn le_dossier(vus: Vec<DossierVu>, id: &IdDrive, racine_drive: &str) -> Result<DossierVu, ErreurGoogle> {
    vus.into_iter().find(|d| d.id == *id).ok_or_else(|| {
        ErreurGoogle::nouvelle(
            socle::echec::Classe::Refuse,
            format!("Ce dossier n'est pas sous {} dans le Drive lié (il a peut-être été supprimé entre-temps) : rien n'est fait. Relire la liste.", racine_drive),
        )
    })
}

/// Le dossier de la compagnie que la sauvegarde de `racine` remplit (son identite, `config/drive.json`), s'il existe deja.
pub fn est_celui_de(vu: &DossierVu, identite: Option<&IdentiteSurLeDrive>) -> bool {
    matches!((&vu.genre, identite), (GenreDuDossier::Compagnie(i), Some(j)) if i == j)
}
