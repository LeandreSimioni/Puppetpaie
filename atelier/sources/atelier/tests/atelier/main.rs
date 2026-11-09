//! Les essais de l'atelier, dans sa propre cible (la feature `atelier` de ce qu'il prend) : le garde avant commit et le
//! verrou des donnees reelles qu'il porte, et la fidelite des referentiels livres aux textes deposes. Un fichier seul :
//! `cargo nextest run --cargo-profile essais -p atelier -E 'test(/^<fichier>::/)'`.

mod outils;

mod donnees_reelles;
mod referentiels;
mod verifier_avant_commit;
