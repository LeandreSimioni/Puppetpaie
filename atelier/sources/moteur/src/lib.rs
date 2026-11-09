//! Le moteur de PuppetPaie au-dessus des envois (`envois`), des documents et des declarations : la ligne de commande
//! `moteur`, les routes de l'interface, la compagnie (`compagnie`), l'import (`import`) et l'atelier (`atelier`).
//!
//! Le JSON ne se touche qu'a la porte (`routes::enveloppe`) : sous elle, chaque donnee se lit et s'ecrit en sa forme.

pub mod apercu_dsn;
#[macro_use]
pub mod commandes;
pub mod atelier;
pub mod compagnie;
pub mod etat_garde;
pub mod historique_mois;
pub mod horloge;
pub mod import;
pub mod interface;
pub mod obligations;
pub mod regularisation;
pub mod routes;
pub mod service;
pub mod simulation_courriels;
