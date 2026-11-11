//! Identifie les sources du calcul : `ENGAGEMENT_SOURCES` (l'empreinte des sources de l'engagement et de chaque crate
//! dont il depend, et du verrou de l'espace `Cargo.lock` : `../empreinte.rs`). Chaque paie ecrite porte cette marque dans
//! son empreinte (`empreinte::MOTEUR`) : un autre calcul la refait.

include!("../empreinte.rs");

/// Le verrou commun, que la marque couvre avec les sources.
const VERROU: &str = "../Cargo.lock";

fn main() {
    println!("cargo:rustc-env=ENGAGEMENT_SOURCES={:016x}", empreinte_des_sources(VERROU));
}
