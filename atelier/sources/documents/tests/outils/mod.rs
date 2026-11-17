//! Les aides des tests des documents, sans JSON brut : une donnee de test se lit PAR LA PORTE DU SOCLE, en texte,
//! directement dans sa forme (`socle::forme::lire_texte`) ; un montant ou un taux se lit dans sa grandeur.

use std::path::Path;

use serde::de::DeserializeOwned;

/// Le jour que le test ecrit (`AAAA-MM-JJ`), lu par sa forme.
pub fn le_jour(ecrit: &str) -> socle::date::Jour {
    socle::forme::lire_texte(&format!("\"{}\"", ecrit), "le jour du test").unwrap()
}

/// La grandeur qu'une donnee de test ecrit comme la DSN l'ecrit (`12.30`).
pub fn dec<G: socle::grandeurs::Grandeur>(s: &str) -> G {
    socle::grandeurs::lu::norme(s).unwrap_or_else(|| panic!("nombre illisible : {:?}", s))
}

/// `documents/tests/donnees/<nom>`, lu par la porte du socle, dans sa forme.
pub fn lire_donnee<T: DeserializeOwned>(nom: &str) -> T {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees").join(nom);
    let texte = std::fs::read_to_string(&chemin).unwrap_or_else(|e| panic!("{} illisible : {}", chemin.display(), e));
    socle::forme::lire_texte(&texte, &format!("{} ne se lit pas.", chemin.display()))
        .unwrap_or_else(|e| panic!("{} : {}", chemin.display(), e.phrase))
}
