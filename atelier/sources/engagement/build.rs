//! Identifie les sources du calcul : `ENGAGEMENT_SOURCES` (l'empreinte des fichiers de `src/` du socle, du droit et de
//! l'engagement, et du verrou de l'espace `Cargo.lock` : `../empreinte.rs`). Chaque paie ecrite porte cette marque dans
//! son empreinte (`empreinte::MOTEUR`) : un autre calcul la refait.

include!("../empreinte.rs");

/// Ce que la marque couvre : les sources des trois crates du calcul, et le verrou commun.
const DOSSIERS: [&str; 3] = ["../socle/src", "../droit/src", "src"];
const VERROU: &str = "../Cargo.lock";

fn main() {
    println!("cargo:rustc-env=ENGAGEMENT_SOURCES={:016x}", empreinte_des_sources(&DOSSIERS, VERROU));
}
