//! Identifie les sources des routes : `PAIE_SOURCES` (l'empreinte des sources des routes et de chaque crate dont elles
//! dependent, et du verrou de l'espace : `../empreinte.rs`). L'etat garde de l'ouverture (`etat_garde`) porte cette
//! marque : un autre moteur ne reprend pas un resultat qu'il n'a pas calcule.

include!("../empreinte.rs");

/// Le verrou commun, que la marque couvre avec les sources.
const VERROU: &str = "../Cargo.lock";

fn main() {
    println!("cargo:rustc-env=PAIE_SOURCES={:016x}", empreinte_des_sources(VERROU));
}
