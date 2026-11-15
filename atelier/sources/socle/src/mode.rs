//! LE MODE D'UN ENVOI : reel (il engage), ou test (un depot de test, qui ne conserve rien). Le journal des envois
//! l'ecrit par son nom (`reel`, `test`) ; une charge ou une reponse de route qui dit `reel` (ou `test`) l'ecrit en
//! booleen (`en_booleen`, `test_en_booleen`) ; la DSN l'ecrit par son code (S10.G00.00.005), a sa porte
//! (`droit::nature_declaration`).

use serde::{Deserialize, Serialize};

/// Le mode d'un envoi, dans l'ordre : le test avant le reel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Test,
    Reel,
}

impl Mode {
    /// Son nom, tel que le journal des envois l'ecrit.
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

/// L'ECRITURE D'UN MODE EN BOOLEEN `reel` (une charge ou une reponse de route) : vrai en reel, faux en test.
pub mod en_booleen {
    use super::Mode;

    pub fn serialize<S: serde::Serializer>(m: &Mode, s: S) -> Result<S::Ok, S::Error> {
        match m {
            Mode::Reel => s.serialize_bool(true),
            Mode::Test => s.serialize_bool(false),
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Mode, D::Error> {
        match <bool as serde::Deserialize>::deserialize(d)? {
            true => Ok(Mode::Reel),
            false => Ok(Mode::Test),
        }
    }

    /// La meme ecriture, du mode qu'un fichier lu dit, ou ne dit pas (une reponse qui dit si le fichier est reel) : vrai
    /// s'il dit reel, faux sinon. Elle ne se relit pas.
    pub mod si_dit {
        use super::super::Mode;

        pub fn serialize<S: serde::Serializer>(m: &Option<Mode>, s: S) -> Result<S::Ok, S::Error> {
            match m {
                Some(Mode::Reel) => s.serialize_bool(true),
                Some(Mode::Test) | None => s.serialize_bool(false),
            }
        }
    }

    /// La meme ecriture, d'un mode qu'une charge peut taire.
    pub mod option {
        use super::super::Mode;

        pub fn serialize<S: serde::Serializer>(m: &Option<Mode>, s: S) -> Result<S::Ok, S::Error> {
            match m {
                Some(m) => super::serialize(m, s),
                None => s.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Mode>, D::Error> {
            match <Option<bool> as serde::Deserialize>::deserialize(d)? {
                Some(true) => Ok(Some(Mode::Reel)),
                Some(false) => Ok(Some(Mode::Test)),
                None => Ok(None),
            }
        }
    }
}

/// L'ECRITURE D'UN MODE EN BOOLEEN `test` (la charge d'un signalement qui dit son fichier d'essai) : vrai en test, faux
/// en reel ; une charge peut la taire.
pub mod test_en_booleen {
    pub mod option {
        use super::super::Mode;

        pub fn serialize<S: serde::Serializer>(m: &Option<Mode>, s: S) -> Result<S::Ok, S::Error> {
            match m {
                Some(Mode::Test) => s.serialize_some(&true),
                Some(Mode::Reel) => s.serialize_some(&false),
                None => s.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Mode>, D::Error> {
            match <Option<bool> as serde::Deserialize>::deserialize(d)? {
                Some(true) => Ok(Some(Mode::Test)),
                Some(false) => Ok(Some(Mode::Reel)),
                None => Ok(None),
            }
        }
    }
}
