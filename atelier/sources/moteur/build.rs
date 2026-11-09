//! Identifie les sources du binaire : `PAIE_SOURCES` (l'empreinte des sources du socle, du droit, de l'engagement, de la
//! compagnie, de l'import et du moteur, et du verrou de l'espace : `../empreinte.rs`). L'etat garde de l'ouverture
//! (`etat_garde`) porte cette marque : un autre moteur ne reprend pas un resultat qu'il n'a pas calcule.

include!("../empreinte.rs");

/// Ce que la marque couvre : les sources de ces crates, et le verrou commun.
const DOSSIERS: [&str; 6] = ["../socle/src", "../droit/src", "../engagement/src", "../compagnie/src", "../import/src", "src"];
const VERROU: &str = "../Cargo.lock";

fn main() {
    println!("cargo:rustc-env=PAIE_SOURCES={:016x}", empreinte_des_sources(&DOSSIERS, VERROU));
    // LE COMMIT DES SOURCES, que la fenetre de l'atelier montre a la place d'une version publiee : `logiciel/VERSION`
    // ne change qu'a la publication. Sans git, rien.
    let commit = std::process::Command::new("git")
        .args(["log", "-1", "--format=%h du %cd", "--date=format:%d/%m/%Y"])
        .output()
        .ok()
        .filter(|s| s.status.success())
        .map(|s| String::from_utf8_lossy(&s.stdout).trim().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=PAIE_COMMIT={}", commit);
    // Un commit neuf refait la marque : le journal de HEAD bouge a chaque commit.
    if let Some(git) = std::process::Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .ok()
        .filter(|s| s.status.success())
        .map(|s| String::from_utf8_lossy(&s.stdout).trim().to_string())
    {
        println!("cargo:rerun-if-changed={}/HEAD", git);
        println!("cargo:rerun-if-changed={}/logs/HEAD", git);
    }
}
