//! Les routes de l'interface (`moteur route`) : chaque test monte sa compagnie fictive dans un dossier a lui.
//!
//! Un binaire pour tout le groupe : moins d'éditions de liens, moins de
//! disque, et les tests des différents fichiers se répartissent sur les cœurs. Un fichier seul :
//! `cargo nextest run --cargo-profile essais -E 'binary(routes) & test(/^<fichier>::/)'` (le nom du fichier est le nom du module).
//! Aucun de ces fichiers ne change l'environnement du processus (`set_var`) : ceux qui le font restent seuls dans `tests/`.

// Les aides communes a plusieurs binaires d'essai : publiques, chacun n'en prend que ce qu'il emploie.
#[path = "../outils/mod.rs"]
pub mod outils;
#[path = "../bulletins_fictifs/mod.rs"]
pub mod bulletins_fictifs;
#[path = "../faux_drive/mod.rs"]
pub mod faux_drive;

mod demarrage;
mod import_neuve;
mod compagnie_supprimee;
mod contrat_a_cheval;
mod homonymes;
mod case_de_depot;
mod notification_atmp;
mod net_a_virer;
mod duree_du_travail;
mod arret_prolonge;
mod signalement_apres_echeance;
mod cdd_d_artiste;
mod dsn_sans_salarie;
mod une_liste_de_ce_qui_est_du;
mod frais_de_deplacement;
mod second_contrat_au_mois;
mod identite_qui_arrive;
mod technicien_au_service;
mod technicien_de_la_3090;
