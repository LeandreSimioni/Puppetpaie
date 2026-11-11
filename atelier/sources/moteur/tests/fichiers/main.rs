//! Les fichiers de la compagnie et ce qui en part : ecriture, journal, coffre, sauvegarde, lecture des courriels, Drive, et la
//! ligne de commande.
//!
//! Un binaire pour tout le groupe : moins d'éditions de liens, moins de
//! disque, et les tests des différents fichiers se répartissent sur les cœurs. Un fichier seul :
//! `cargo nextest run --cargo-profile essais -E 'binary(fichiers) & test(/^<fichier>::/)'` (le nom du fichier est le nom du module).
//! Aucun de ces fichiers ne change l'environnement du processus (`set_var`) : ceux qui le font restent seuls dans `tests/`.

// Les aides communes a plusieurs binaires d'essai : publiques, chacun n'en prend que ce qu'il emploie.
#[path = "../outils/mod.rs"]
pub mod outils;
#[path = "../bulletins_fictifs/mod.rs"]
pub mod bulletins_fictifs;
#[path = "../faux_drive/mod.rs"]
pub mod faux_drive;

mod bridage;
mod bulletin_lu;
mod caviardage;
mod coffre;
mod sources;
mod drive;
mod essai;
mod garde_reseau;
mod installations;
mod mise_a_jour;
mod restauration;
mod sauvegarde;
mod structure;
mod verrous_cargo;
