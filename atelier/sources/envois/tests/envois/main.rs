//! Les tests des envois sans compagnie : ce que le transport envoie a net-entreprises et ce qu'il garde pour lui, la
//! bascule d'un depot en test et la preuve d'un depot reel, la lecture des retours reels transcrits, la boite simulee,
//! un envoi coupe en route. Rien ne sort de la machine (un faux serveur local). Un fichier seul :
//! `cargo nextest run --cargo-profile essais -p envois -E 'test(/^<fichier>::/)'`.

// Les aides, dont le faux serveur que les essais du moteur prennent aussi : publiques, chacun n'en prend que ce qu'il
// emploie.
#[path = "../outils/mod.rs"]
pub mod outils;

mod courriel;
mod courriel_lecture;
mod depot;
mod envoi;
mod journal;
mod retours;
mod transport;
