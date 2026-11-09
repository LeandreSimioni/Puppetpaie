//! Le moteur de PuppetPaie au-dessus des envois (`envois`), des documents et des declarations : la ligne de commande
//! `moteur`, les routes de l'interface, la compagnie (`compagnie` : ce qui la garde, et l'installation qui la porte sur
//! le poste), l'import (`import`) et l'atelier (`atelier`). Au niveau des routes, ce que l'accueil fait des compagnies
//! (`accueil`) et ce que la veille dit a la page (`veille`).
//!
//! Le JSON ne se touche qu'a la porte (`routes::enveloppe`) : sous elle, chaque donnee se lit et s'ecrit en sa forme.

pub mod accueil;
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
pub mod veille;
