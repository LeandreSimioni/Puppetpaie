//! UN ARGUMENT DE LIGNE DE COMMANDE : ce qu'une personne a tape apres une option (`--jour 2026-11-03`, `--mois 2026-11`).
//! C'est la porte par laquelle ce qu'on tape devient un jour ou un mois ; le refus cite ce qui a ete tape.

use crate::date::Jour;
use crate::echec::R;
use crate::mois::Mois;

/// Le texte d'un argument, tel qu'il a ete tape.
#[derive(Debug, Clone, Copy)]
pub struct Argument<'a>(&'a str);

impl<'a> Argument<'a> {
    /// L'argument `texte`, tape sur la ligne de commande.
    pub fn tape(texte: &'a str) -> Argument<'a> {
        Argument(texte)
    }

    /// Un jour, dans l'un des cinq formats de `date`.
    pub fn jour(self) -> R<Jour> {
        Jour::lire(self.0)
    }

    /// Un mois `AAAA-MM`.
    pub fn mois(self) -> R<Mois> {
        Mois::lire(self.0)
    }
}
