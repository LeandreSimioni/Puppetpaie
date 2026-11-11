//! LES ARGUMENTS DE LA LIGNE DE COMMANDE : ce qu'une personne a tape apres le nom du programme (`--jour 2026-11-03`,
//! `--mois 2026-11`). Seul ce module lit les arguments du processus (`arguments`) et en fabrique ; les parseurs de la
//! ligne de commande les recoivent tels quels. C'est la porte par laquelle ce qu'on tape devient un jour ou un mois ; le
//! refus cite ce qui a ete tape.

use crate::date::Jour;
use crate::echec::R;
use crate::mois::Mois;

/// Un argument du processus, tel qu'il a ete tape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argument(String);

/// Les arguments du processus, sans le nom du programme.
pub fn arguments() -> Vec<Argument> {
    std::env::args().skip(1).map(Argument).collect()
}

/// Les textes des arguments `args`, pour un parseur qui n'en lit ni jour ni mois.
pub fn textes(args: &[Argument]) -> Vec<String> {
    args.iter().map(|a| a.0.clone()).collect()
}

impl Argument {
    /// Le texte tape.
    pub fn texte(&self) -> &str {
        &self.0
    }

    /// `--nom=valeur` : le nom de l'option et sa valeur, tapee avec elle ; `None` sans `=` ou hors d'une option longue.
    pub fn option_collee(&self) -> Option<(&str, Argument)> {
        let (nom, valeur) = self.0.strip_prefix("--")?.split_once('=')?;
        Some((nom, Argument(valeur.to_string())))
    }

    /// Un jour, dans l'un des cinq formats de `date`.
    pub fn jour(&self) -> R<Jour> {
        Jour::lire(&self.0)
    }

    /// Un mois `AAAA-MM`.
    pub fn mois(&self) -> R<Mois> {
        Mois::lire(&self.0)
    }
}
