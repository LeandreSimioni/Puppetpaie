//! Le moteur de PuppetPaie au-dessus des routes (la crate `routes` : la porte du moteur, et ce qu'elles font) : la ligne
//! de commande `moteur` (`commandes`), l'interface (`interface` : le serveur de la fenetre, la page d'accueil, la veille
//! qui tourne) et l'atelier (`atelier`).
//!
//! Le JSON ne se touche qu'a la porte (`routes::enveloppe`) : le moteur n'en touche pas.

#[macro_use]
extern crate socle;

pub mod commandes;
pub mod atelier;
pub mod interface;
pub mod publication;
