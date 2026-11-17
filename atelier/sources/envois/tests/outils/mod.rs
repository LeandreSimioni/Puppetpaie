//! Les aides des tests des envois : un dossier par test, le faux serveur HTTP.

use std::path::{Path, PathBuf};

pub mod serveur;
pub use serveur::*;

/// Le mois que le test ecrit (`AAAA-MM`), lu par sa forme.
pub fn le_mois(ecrit: &str) -> socle::mois::Mois {
    socle::forme::lire_texte(&format!("\"{}\"", ecrit), "le mois du test").unwrap()
}

/// Un dossier vide pour un test, sous le dossier temporaire de Cargo.
pub fn dossier_de_test(fichier: &str, test: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("envois").join(fichier).join(test);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

