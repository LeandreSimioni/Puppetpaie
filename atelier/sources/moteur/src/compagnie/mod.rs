//! LA COMPAGNIE : ce qui la garde et la fait vivre a cote de ses engagements, sans rien prendre aux routes. L'installation
//! qui la porte sur le poste (`installation`, `instance`, ce qu'elle demande au poste, l'auteur du logiciel), le coffre et
//! ses deux mots de passe, la preuve d'un acte reel et le secret de l'organisme, la demande d'un acte reel, la sauvegarde
//! et la restauration, la mise a jour, le bridage, le filet, les incidents et les signalements, le journal des pieces
//! ecrites, les clients de la consultation, les organismes et les taux de PAS recus.

pub mod amorcage_demande;
pub mod auteur;
pub mod boite;
pub mod bridage;
pub mod coffre;
pub mod coffre_verifier;
pub mod consultation;
pub mod demande;
pub mod etat_dsn;
pub mod diagnostic;
pub mod editeur;
pub mod essai;
pub mod incidents;
pub mod installation;
pub mod instance;
pub mod maj;
pub mod nombre_de_paies;
pub mod organismes;
pub mod pieces_ecrites;
pub mod poste;
pub mod preuve;
pub mod problemes;
pub mod restauration;
pub mod sauvegarde;
pub mod taux_pas;

/// QUI ECRIT : ce que les routes et les gestes de la compagnie inscrivent, la fenetre comme la ligne de commande
/// (`moteur route`, qui sert les memes).
pub(crate) const INTERFACE: &str = "interface";

/// `socle::paris::maintenant` a la minute, `AAAA-MM-JJ HH:MM` : l'heure d'une saisie.
pub(crate) fn maintenant() -> String {
    socle::paris::maintenant()[..16].replacen('T', " ", 1)
}

/// Une racine d'essai (dans le dossier temporaire, jamais `prive/` ni `fictives/`) : les trois fiches vides de quoi ouvrir un
/// `Contexte`, et, si `fictive`, le marqueur d'une compagnie fictive.
#[cfg(test)]
pub(crate) fn racine_d_essai(fictive: bool) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let r = std::env::temp_dir().join(format!("paie-horloge-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&r);
    std::fs::create_dir_all(r.join(socle::fichiers::CONFIG)).unwrap();
    for f in ["entreprise.json", "salaries.json", "spectacles.json"] {
        std::fs::write(r.join(socle::fichiers::CONFIG).join(f), "{}").unwrap();
    }
    if fictive {
        std::fs::write(r.join("COPIE-DE-DEVELOPPEMENT.json"), r#"{"copie_de": "compagnie fictive essai", "quoi": "essai"}"#).unwrap();
    }
    r
}
