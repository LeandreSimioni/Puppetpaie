//! Identifie les sources de la compagnie : `PAIE_SOURCES` (l'empreinte des sources du socle, du droit, de l'engagement et
//! de la compagnie, et du verrou de l'espace : `../empreinte.rs`), que la machine du diagnostic porte
//! (`diagnostic::machine`). La marque de l'etat garde de l'ouverture est celle de `moteur/build.rs`, qui couvre aussi le
//! moteur : celle-ci ne lit rien au-dessus de la compagnie, qu'une retouche du haut ne recompile pas.

include!("../empreinte.rs");

/// Ce que la marque couvre : les sources du socle, du droit, de l'engagement et de la compagnie, et le verrou commun.
const DOSSIERS: [&str; 4] = ["../socle/src", "../droit/src", "../engagement/src", "src"];
const VERROU: &str = "../Cargo.lock";

fn main() {
    println!("cargo:rustc-env=PAIE_SOURCES={:016x}", empreinte_des_sources(&DOSSIERS, VERROU));
}
