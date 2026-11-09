//! Identifie le commit des sources : `PAIE_COMMIT`. La marque des sources que l'etat garde de l'ouverture porte est
//! celle des routes (`routes/build.rs`).

fn main() {
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
