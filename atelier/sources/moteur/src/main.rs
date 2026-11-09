//! `moteur` : la ligne de commande du moteur Rust (`commandes::ligne_de_commande`). Se parse a la main.

fn usage() -> ! {
    eprintln!(
        "usage : moteur [--dev] COMMANDE [options]   (la ligne de commande du produit : moteur aide)\n\
         \x20       moteur route [--racine R] [--date D]   (route et charge en JSON sur l entree standard : les routes de l interface)\n\
         \x20       moteur routes [NOM] [--avance] [--json]   (le catalogue des routes, et la fiche d une route)\n\
         \x20       moteur pdf ...   (la ligne de commande de la fabrique : moteur pdf --aide)"
    );
    std::process::exit(2)
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match moteur::commandes::ligne_de_commande(&args, Vec::new()) {
        Some(code) => code,
        None => usage(),
    }
}
