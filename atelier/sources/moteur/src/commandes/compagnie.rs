//! Le choix de la compagnie :
//!
//!   - `PUPPETPAIE_DONNEES` si elle est posee ;
//!   - sinon le dossier `prive/` voisin du code, s'il porte un `config/` ;
//!   - sinon le code lui-meme (installation d'un seul tenant) ;
//!   - `moteur --dev COMMANDE` : la copie de developpement `prive-dev/`, si le mode developpeur est allume et la copie
//!     faite (la commande pose alors `PUPPETPAIE_DONNEES`, pour le reste du moteur).
//!
//! Et le jour : celui de Paris, pas celui d'UTC. `PUPPETPAIE_AUJOURDHUI`, quand elle
//! est posee, regle `aujourdhui` ; sinon on la pose a l'heure de Paris pour que tout le moteur lise le meme jour.

use std::path::{Path, PathBuf};

use socle::date::Jour;

use compagnie::installation::MARQUEUR;
const FICHIER_MODE: &str = ".mode-developpeur.json";
const NOM_DEV: &str = "prive-dev";

/// Le dossier qui porte `logiciel/` (celui du depot). `fixer_le_code` a rendu le chemin du
/// code absolu et sans `..`.
fn parent_du_logiciel() -> PathBuf {
    // `<...>/logiciel/donnees` -> `<...>/logiciel` -> `<...>`
    compagnie::installation::dessus(&compagnie::installation::logiciel())
}

/// Dans l'atelier (`atelier/prive-dev`), pas a cote de ce que les gens recoivent (a la racine : `logiciel/`, `fictives/`, `prive/`).
pub fn dossier_dev() -> PathBuf {
    parent_du_logiciel().join("atelier").join(NOM_DEV)
}

/// `PUPPETPAIE_CLE` le deplace (c'est ainsi que les tests le tiennent hors du poste).
fn chemin_mode() -> PathBuf {
    match std::env::var_os("PUPPETPAIE_CLE").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d).join(FICHIER_MODE),
        // Le mode developpeur est un reglage de l'atelier : il vit dans `atelier/`.
        None => parent_du_logiciel().join("atelier").join(FICHIER_MODE),
    }
}

/// `.mode-developpeur.json` : le mode developpeur de ce poste, allume ou non.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ModeDeveloppeur {
    actif: bool,
}

/// Le mode developpeur est-il allume ? Sans `.mode-developpeur.json`, il est eteint.
pub fn mode_actif() -> Result<bool, String> {
    let p = chemin_mode();
    if !p.is_file() {
        return Ok(false);
    }
    let m: ModeDeveloppeur = socle::forme::lire_fichier(&p, "Le mode développeur (.mode-developpeur.json) ne se lit pas : il dit {\"actif\": true} ou {\"actif\": false}.")?;
    Ok(m.actif)
}

/// La copie de developpement est-elle faite ? Son marqueur le dit, dans sa forme (`compagnie::installation::Marqueur`).
pub fn copie_faite() -> Result<bool, String> {
    Ok(compagnie::installation::marqueur(&dossier_dev())?.is_some())
}

/// Le jour de Paris, aujourd'hui.
pub fn jour_de_paris() -> Jour {
    socle::temps::jour_de_paris()
}

/// Pose `PUPPETPAIE_AUJOURDHUI` a l'heure de Paris quand rien ne l'a posee.
pub fn poser_le_jour() -> Jour {
    let paris = jour_de_paris();
    if socle::temps::jour_pose().is_none() {
        std::env::set_var("PUPPETPAIE_AUJOURDHUI", paris.iso());
    }
    paris
}

/// La racine de la compagnie de cette commande. `dev` : la copie de developpement, ou la phrase du refus.
pub fn racine(dev: bool) -> Result<PathBuf, String> {
    if dev {
        if !mode_actif()? {
            return Err(socle::echec::avec(
                "Le mode développeur est éteint sur ce poste : la copie de développement ne s'y ouvre pas.",
                socle::echec::APart { chemin: Some(chemin_mode().to_string_lossy().into_owned()), detail: Some("{\"actif\": true} l'allume".to_string()), ..Default::default() },
            ));
        }
        if !copie_faite()? {
            return Err(socle::echec::avec(
                "Ce poste n'a pas de copie de développement.",
                socle::echec::APart { chemin: Some(dossier_dev().join(MARQUEUR).to_string_lossy().into_owned()), ..Default::default() },
            ));
        }
        let d = dossier_dev();
        std::env::set_var("PUPPETPAIE_DONNEES", &d);
        return Ok(d);
    }
    Ok(crate::routes::ouverture::racine_du_poste())
}

/// La compagnie est-elle une copie (developpement ou compagnie fictive) ?
pub fn est_copie(racine: &Path) -> bool {
    racine.join(MARQUEUR).is_file()
}
