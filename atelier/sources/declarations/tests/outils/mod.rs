//! Les aides des tests des declarations : les donnees livrees, et une saisie de fiche
//! lue dans la forme que la table des cases donne a chacune (`Saisie::de_la_fiche`).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

/// Les donnees livrees (`logiciel/donnees`).
pub fn code_livre() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees")
}

/// Le fichier `rel` sous `racine`, lu par la porte du socle, dans son type.
pub fn lire<T: DeserializeOwned>(racine: &Path, rel: &str) -> T {
    let chemin = racine.join(rel);
    let texte = std::fs::read_to_string(&chemin).unwrap_or_else(|e| panic!("{} illisible : {}", chemin.display(), e));
    socle::forme::lire_texte(&texte, &format!("{} ne se lit pas.", chemin.display()))
        .unwrap_or_else(|e| panic!("{} : {}", chemin.display(), e.phrase))
}

/// La saisie de la fiche `nom` qui dit ces cases-la (des textes), lue selon la table comme une fiche
/// (`Saisie::de_la_fiche`).
pub fn saisie(nom: &str, cases: &[(&str, &str)]) -> declarations::champs::Saisie {
    let fiche: std::collections::BTreeMap<&str, &str> = cases.iter().copied().collect();
    declarations::champs::Saisie::de_la_fiche(nom, &fiche).unwrap_or_else(|e| panic!("{}", e.phrase))
}
