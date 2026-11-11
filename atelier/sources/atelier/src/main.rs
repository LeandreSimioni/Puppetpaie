//! `moteur-atelier` : le binaire du nuage, jamais livre. La ligne de commande du moteur (`moteur::commandes`), plus les
//! commandes de l'atelier (`atelier::commandes`) ; construit avec la feature `atelier` de ce qu'il prend, il peut etre
//! pointe vers de faux serveurs locaux et refuse tout acte reel.

fn usage() -> ! {
    eprintln!(
        "usage : moteur-atelier [--dev] COMMANDE [options]   (la ligne de commande du produit et de l'atelier : moteur-atelier aide)\n\
         \x20       moteur-atelier route [--racine R] [--date D]   (route et charge en JSON sur l entree standard : les routes de l interface)\n\
         \x20       moteur-atelier routes [NOM] [--avance] [--json]   (le catalogue des routes, et la fiche d une route)\n\
         \x20       moteur-atelier pdf ...   (la ligne de commande de la fabrique : moteur-atelier pdf --aide)\n\
         \x20       moteur-atelier scenario ...   (les scenarios du banc d'essai : rejouer, verifier un banc, elaguer)\n\
         \x20       moteur-atelier guet [--seuil N] [--commits] [--transcript F]   (la veille du contexte d'un sous-agent)\n\
         \x20       moteur-atelier verifier-avant-commit [--indexes]   (le garde avant commit : rien de reel dans ce que git emporterait)\n\
         \x20       moteur-atelier empreintes   (l'empreinte de chaque texte depose, dans le LISEZ-MOI de son dossier)\n\
         \x20       moteur-atelier generer-tables SOURCES ESPACE   (les tables Unicode du moteur, depuis atelier/reference/sources, dans atelier/sources)\n\
         \x20       moteur-atelier extraire naf|usages|formats|urssaf SOURCES... SORTIE.json [--remplacer]   (les referentiels : moteur-atelier extraire --aide)\n\
         \x20       moteur-atelier releve-codes ... | moteur-atelier releve-conventions ...   (les textes LEGI et KALI de la DILA : --aide)\n\
         \x20       moteur-atelier releve-missions SOURCES DONNEES   (les tables des missions a l'etranger de la DGFiP, avant de publier)\n\
         \x20       moteur-atelier fabriquer-bulletins DESCRIPTION.json DOSSIER [--gabarit coolpaie|spaiectacle]   (des bulletins fictifs)"
    );
    std::process::exit(2)
}

fn main() -> std::process::ExitCode {
    // Une redirection vers autre chose qu'un serveur local arrete tout.
    if let Err(m) = atelier::redirection::verifier_environnement() {
        eprintln!("{}", m.phrase);
        return std::process::ExitCode::from(2);
    }
    compagnie::incidents::brancher_l_engagement();
    let args = socle::ligne::arguments();
    // Les commandes de l'atelier que l'on lance par leur nom ; le reste est la ligne de commande du moteur, avec celles de
    // l'atelier qui passent par son cadre.
    if let Some(nom) = args.first().map(socle::ligne::Argument::texte) {
        if let Some(code) = atelier::commandes::lancer(nom, &args[1..]) {
            return std::process::ExitCode::from(code.clamp(0, 255) as u8);
        }
    }
    match moteur::commandes::ligne_de_commande(&args, atelier::commandes::commandes()) {
        Some(code) => code,
        None => usage(),
    }
}
