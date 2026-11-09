//! Identifie les sources des routes : `PAIE_SOURCES` (l'empreinte des sources du socle, du droit, de l'engagement, de la
//! compagnie, de l'import et des routes, et du verrou de l'espace : `../empreinte.rs`). L'etat garde de l'ouverture
//! (`etat_garde`) porte cette marque : un autre moteur ne reprend pas un resultat qu'il n'a pas calcule.

include!("../empreinte.rs");

/// Ce que la marque couvre : les sources de ces crates, et le verrou commun.
const DOSSIERS: [&str; 6] = ["../socle/src", "../droit/src", "../engagement/src", "../compagnie/src", "../import/src", "src"];
const VERROU: &str = "../Cargo.lock";

fn main() {
    println!("cargo:rustc-env=PAIE_SOURCES={:016x}", empreinte_des_sources(&DOSSIERS, VERROU));
}
