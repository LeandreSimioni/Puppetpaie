//! LE MODE D'UN ENVOI : reel (il engage), ou test (un depot de test, qui ne conserve rien). Il s'ecrit par son nom
//! (`reel`, `test`) ; la DSN l'ecrit par son code (S10.G00.00.005), a sa porte (`droit::nature_declaration`).

use serde::{Deserialize, Serialize};

/// Le mode d'un envoi, dans l'ordre : le test avant le reel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Test,
    Reel,
}

impl Mode {
    /// Son nom, tel qu'il s'ecrit.
    pub fn nom(self) -> &'static str {
        match self {
            Mode::Reel => "reel",
            Mode::Test => "test",
        }
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.pad(self.nom())
    }
}
