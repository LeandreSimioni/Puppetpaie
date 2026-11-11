//! LA COPIE D'AVANT : un fichier de la compagnie qu'on va remplacer se copie d'abord, horodate, dans le dossier `avant/`
//! a cote de lui ; seules les dernieres copies se gardent.

use std::path::Path;

use socle::echec::{erreur, Classe, Echec, R};

const GARDER_COPIES: usize = 20;

/// La copie horodatee du fichier, avant de le
/// remplacer (l'heure de Paris), et seulement les `GARDER_COPIES` dernieres.
pub fn sauver_avant(chemin: &Path) -> R<()> {
    if !chemin.exists() {
        return Ok(());
    }
    let io = |e: std::io::Error| Echec { classe: Some(Classe::du_disque(&e)), ..socle::echec::du_disque(chemin, &e) };
    let Some(parent) = chemin.parent() else {
        return Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : le fichier à copier n'a pas de dossier."));
    };
    let dossier = parent.join("avant");
    std::fs::create_dir_all(&dossier).map_err(io)?;
    let nom = socle::fichiers::nom_du(chemin);
    let maintenant = socle::temps::secondes();
    let horodatage = socle::paris::ecrit::seconde::texte(maintenant).replace(['-', ':'], "").replace('T', "-");
    std::fs::copy(chemin, dossier.join(format!("{}.{}", nom, horodatage))).map_err(io)?;
    let prefixe = format!("{}.", nom);
    let mut anciennes: Vec<String> = Vec::new();
    for e in std::fs::read_dir(&dossier).map_err(io)? {
        let n = e.map_err(io)?.file_name().to_string_lossy().into_owned();
        if n.starts_with(&prefixe) {
            anciennes.push(n);
        }
    }
    anciennes.sort();
    if anciennes.len() > GARDER_COPIES {
        for vieille in &anciennes[..anciennes.len() - GARDER_COPIES] {
            socle::fichiers::effacer(&dossier.join(vieille))?;
        }
    }
    Ok(())
}
