//! Identifie les sources de la compagnie : `PAIE_SOURCES` (l'empreinte des sources de la compagnie et de chaque crate
//! dont elle depend, et du verrou de l'espace : `../empreinte.rs`), que la machine du diagnostic porte
//! (`diagnostic::machine`). La marque de l'etat garde de l'ouverture est celle de `routes/build.rs`, qui couvre aussi
//! les routes : celle-ci ne lit rien au-dessus de la compagnie, qu'une retouche du haut ne recompile pas.

include!("../empreinte.rs");

/// Le verrou commun, que la marque couvre avec les sources.
const VERROU: &str = "../Cargo.lock";

fn main() {
    println!("cargo:rustc-env=PAIE_SOURCES={:016x}", empreinte_des_sources(VERROU));
}
