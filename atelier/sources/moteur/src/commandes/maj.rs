//! `moteur maj` : la mise a jour en ligne de commande -- la chercher, l'appliquer, revenir a la precedente. Ce que la mise
//! a jour recoit, verifie et applique : `compagnie::maj` ; la publier, par le flux « Publier » : `publication`.

use compagnie::maj::{appliquer, installation, revenir, verifier, version_lue};

/// `moteur maj [--sans-telecharger] [--appliquer] [--revenir]`.
pub fn commande(args: &[String]) -> i32 {
    // D'abord savoir ou l'on est installe : sans cela, `installation()` retombe sur le dossier ou le programme a ete
    // compile.
    compagnie::installation::fixer_le_code();
    let drapeau = |n: &str| args.iter().any(|a| a == n);
    let r = if drapeau("--appliquer") {
        match compagnie::instance::ouverte(&installation(), compagnie::instance::Plage::DU_POSTE) {
            Ok(true) => Err("PuppetPaie tourne : fermer la fenetre, puis appliquer".into()),
            Ok(false) => appliquer(&installation()).map(|r| format!("Version {} en place ; la precedente est dans {}", version_lue(&r.version), r.avant)),
            Err(e) => Err(String::from(e)),
        }
    } else if drapeau("--revenir") {
        match compagnie::instance::ouverte(&installation(), compagnie::instance::Plage::DU_POSTE) {
            Ok(true) => Err("PuppetPaie tourne : fermer la fenetre, puis revenir".into()),
            Ok(false) => revenir(&installation()).map(|v| format!("Retour a la version {}", version_lue(&v))),
            Err(e) => Err(String::from(e)),
        }
    } else {
        Ok(verifier(!drapeau("--sans-telecharger")))
    };
    match r {
        Ok(t) => {
            dire!("{}", t);
            0
        }
        Err(e) => {
            dire!("! {}", e);
            2
        }
    }
}
