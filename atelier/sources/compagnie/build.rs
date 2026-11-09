//! Identifie les sources de la compagnie : `PAIE_SOURCES` (FNV-1a 64 bits sur les fichiers de `src/` du socle, du droit,
//! de l'engagement et de la compagnie, et sur le verrou de l'espace `Cargo.lock`, dans l'ordre des noms), que la machine
//! du diagnostic porte (`diagnostic::machine`). La marque de l'etat garde de l'ouverture est celle de `moteur/build.rs`,
//! qui couvre aussi le moteur : celle-ci ne lit rien au-dessus de la compagnie, qu'une retouche du haut ne recompile pas.

use std::path::{Path, PathBuf};

/// Ce que la marque couvre : les sources du socle, du droit, de l'engagement et de la compagnie, et le verrou commun.
const DOSSIERS: [&str; 4] = ["../socle/src", "../droit/src", "../engagement/src", "src"];
const VERROU: &str = "../Cargo.lock";

fn fichiers(dossier: &Path, out: &mut Vec<PathBuf>) {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dossier).into_iter().flatten().flatten().map(|e| e.path()).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            fichiers(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let mut liste = Vec::new();
    for d in DOSSIERS {
        fichiers(Path::new(d), &mut liste);
    }
    liste.push(PathBuf::from(VERROU));
    let mut h: u64 = 0xcbf29ce484222325;
    for p in &liste {
        let nom = p.to_string_lossy().replace('\\', "/");
        let contenu = std::fs::read(p).unwrap_or_default();
        for o in nom.bytes().chain([0]).chain(contenu) {
            h = (h ^ o as u64).wrapping_mul(0x100000001b3);
        }
    }
    for d in DOSSIERS {
        println!("cargo:rerun-if-changed={}", d);
    }
    println!("cargo:rerun-if-changed={}", VERROU);
    println!("cargo:rustc-env=PAIE_SOURCES={:016x}", h);
}
