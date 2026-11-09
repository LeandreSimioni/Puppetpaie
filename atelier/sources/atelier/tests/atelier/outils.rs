//! Les aides des essais de l'atelier : les donnees livrees, un dossier par test, une fiche datee, les oracles.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde_json::Value;

#[path = "../../../socle/tests/outils/oracles.rs"]
mod oracles;
pub use oracles::*;

/// Les données livrées (`logiciel/donnees`).
pub fn code_livre() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees")
}

/// `CARGO_TARGET_TMPDIR/<fichier>/<test>`, vidé puis créé : le dossier d'un test, à lui seul.
pub fn dossier_de_test(fichier: &str, test: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(fichier).join(test);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Une fiche plate en fiche datee : chaque valeur (hors cles en `_`) en `[{"valeur": v}]`.
pub fn datee(plate: &Value) -> Value {
    Value::Object(plate.as_object().unwrap().iter().map(|(k, v)| (k.clone(), if k.starts_with('_') { v.clone() } else { serde_json::json!([{"valeur": v}]) })).collect())
}
