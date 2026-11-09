//! LES GESTES DE LA COMPAGNIE QUI PASSENT PAR LES ROUTES, au niveau des routes : l'ouverture et l'administrateur, les
//! organismes et les conseils, le depart des pieces et les depots (les portes de l'acte reel tenues, le depot, ses
//! suites), la signature et son releve, l'export de l'exercice, le releve des retours, l'archivage et le passage de la
//! sauvegarde dans le Drive, l'ecriture des taux de PAS recus.

pub mod administrateur;
pub mod archivage;
pub mod conseils;
pub mod depart;
pub mod depot_acte;
pub mod exercice_ecrit;
pub mod marques_depot;
pub mod organismes;
pub mod ouverture;
pub mod releve;
pub mod releve_signatures;
pub mod sauvegarde;
pub mod signature;
pub mod suites;
pub mod taux_pas;
