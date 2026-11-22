//! LE FORMULAIRE PUBLIE DE « REMPLIR MA FICHE » : `moteur-atelier formulaire-fiche` ecrit, depuis la declaration du
//! moteur (`documents::fiche_questionnaire::formulaire_publie`, avec l'exemple de la deduction forfaitaire que le moteur
//! calcule, `routes::fiches::exemple_dfs`), le fichier `atelier/site/fiche/formulaires/<empreinte>.js`
//! que la page charge. Un fichier par forme, jamais retire : un lien deja envoye nomme le sien. Le fichier se commite ;
//! un test de structure tient qu'il est celui que le moteur declare
//! (`moteur/tests/fichiers/structure.rs`, `le_formulaire_publie_est_celui_que_le_moteur_declare`).

use std::path::{Path, PathBuf};

use socle::echec::R;

/// Le dossier des formulaires publies, relatif a la racine du depot.
pub const FORMULAIRES: &str = "atelier/site/fiche/formulaires";

/// Ecrit le formulaire de ce moteur sous `depot` ; rend son chemin. Un fichier deja la, au meme contenu, ne change pas.
pub fn ecrire(depot: &Path) -> R<PathBuf> {
    let f = documents::fiche_questionnaire::formulaire_publie(routes::fiches::exemple_dfs()?.as_ref());
    let dossier = depot.join(FORMULAIRES);
    std::fs::create_dir_all(&dossier).map_err(|e| socle::echec::du_disque(&dossier, &e))?;
    let chemin = dossier.join(f.nom_du_fichier());
    std::fs::write(&chemin, f.contenu.as_bytes()).map_err(|e| socle::echec::du_disque(&chemin, &e))?;
    Ok(chemin)
}

/// `moteur-atelier formulaire-fiche`, depuis le depot.
pub fn commande(args: &[socle::ligne::Argument]) -> std::process::ExitCode {
    use std::process::ExitCode;
    if !args.is_empty() {
        eprintln!("usage : moteur-atelier formulaire-fiche (sans argument, depuis le dépôt)");
        return ExitCode::from(2);
    }
    let r = std::env::current_dir().map_err(|e| socle::echec::refus(e.to_string())).and_then(|d| crate::verifier_avant_commit::racine_du_depot(&d)).and_then(|d| ecrire(&d));
    match r {
        Ok(chemin) => {
            println!("Formulaire de « Remplir ma fiche » écrit : {}", chemin.display());
            ExitCode::SUCCESS
        }
        Err(m) => {
            eprintln!("{}", m.en_lignes());
            ExitCode::FAILURE
        }
    }
}
