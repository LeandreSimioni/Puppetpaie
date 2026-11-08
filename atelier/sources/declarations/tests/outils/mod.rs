//! Les aides des tests des declarations : les donnees livrees, et une saisie de fiche
//! lue par la porte du socle, dans la forme que la table des cases donne a chacune (`socle::forme::lire_texte_selon`).

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
        .unwrap_or_else(|e| panic!("{} : {}", chemin.display(), socle::echec::phrase_de(&e)))
}

/// La saisie de la fiche `nom` qui dit ces cases-la (des textes), lue par la porte comme une charge de l'ecran.
pub fn saisie(nom: &str, cases: &[(&str, &str)]) -> declarations::champs::Saisie {
    let texte: Vec<String> = cases.iter().map(|(k, v)| format!("{}: {}", socle::ecriture::json_en_ligne(k), socle::ecriture::json_en_ligne(v))).collect();
    let lecture = declarations::champs::lecture(nom).unwrap_or_else(|e| panic!("{}", e.message));
    socle::forme::lire_texte_selon(&format!("{{{}}}", texte.join(", ")), lecture, "La saisie du test ne se lit pas.")
        .unwrap_or_else(|e| panic!("{}", socle::echec::phrase_de(&e)))
}
