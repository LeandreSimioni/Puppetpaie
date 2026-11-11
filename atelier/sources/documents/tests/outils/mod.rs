//! Les aides des tests des documents, sans JSON brut : une donnee de test se lit PAR LA PORTE DU SOCLE, en texte,
//! directement dans sa forme (`socle::forme::lire_texte`) ; un montant se lit en decimal.

use std::path::Path;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::de::DeserializeOwned;

pub fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap_or_else(|_| panic!("montant illisible : {:?}", s))
}

/// `documents/tests/donnees/<nom>`, lu par la porte du socle, dans sa forme.
pub fn lire_donnee<T: DeserializeOwned>(nom: &str) -> T {
    let chemin = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees").join(nom);
    let texte = std::fs::read_to_string(&chemin).unwrap_or_else(|e| panic!("{} illisible : {}", chemin.display(), e));
    socle::forme::lire_texte(&texte, &format!("{} ne se lit pas.", chemin.display()))
        .unwrap_or_else(|e| panic!("{} : {}", chemin.display(), e.phrase))
}
