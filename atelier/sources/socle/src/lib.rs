//! Le socle du moteur : ce que toute partie du dessus lit sans rien savoir de la paie. Les dates et l'heure de Paris, le
//! temps (celui de la machine, ou le temps simule de l'atelier), les nombres et leur arrondi, le texte, les fichiers lus
//! et ecrits, l'echec et sa memoire, le garde-fou reseau. Aucun droit, aucun metier.

pub mod base64;
pub mod csv;
pub(crate) mod d28;
pub mod date;
pub mod echec;
pub mod ecran;
pub mod ecriture;
pub mod feries;
pub mod fichiers;
pub mod fil_de_fond;
pub mod forme;
pub(crate) mod fraction;
pub mod garde_reseau;
pub mod grandeurs;
pub mod html;
pub mod iban;
pub mod json_ascii;
pub mod lecture_xlsx;
pub mod ligne;
pub mod memoire;
pub mod mode;
pub mod mois;
pub mod nfkc;
pub(crate) mod num;
pub mod paris;
pub mod pays;
pub mod piece;
pub mod processus;
pub mod secret;
pub mod nfkd_ascii;
#[cfg(any(windows, target_os = "wasi"))]
pub mod systeme;
pub mod tables_nfkc;
pub mod temps;
pub mod texte;
pub mod verrou;

/// UNE PHRASE DITE AUX GENS QUI SE COMPOSE (`format!`) : `texte::Phrase`.
#[macro_export]
macro_rules! phrase {
    ($($arg:tt)*) => {
        $crate::texte::Phrase::de_format(format_args!($($arg)*))
    };
}

/// UN REFUS ORDINAIRE DONT LA PHRASE SE COMPOSE (`format!`) : `echec::refus`, son endroit note la ou il s'ecrit.
#[macro_export]
macro_rules! refus {
    ($($arg:tt)*) => {
        $crate::echec::refus(format!($($arg)*))
    };
}

/// Ecrit une ligne sur la sortie standard (`au_terminal`).
#[macro_export]
macro_rules! dire {
    () => {
        $crate::au_terminal(format_args!("\n"))
    };
    ($($arg:tt)*) => {
        $crate::au_terminal(format_args!("{}\n", format_args!($($arg)*)))
    };
}

/// Ecrit sans saut de ligne, comme `dire!`.
#[macro_export]
macro_rules! dire_sans_ligne {
    ($($arg:tt)*) => {
        $crate::au_terminal(format_args!($($arg)*))
    };
}

/// UNE LISTE FERMEE DE NOMS QUE LE MOTEUR DONNE, declaree une fois : chaque variante et son nom, le seul endroit ou il
/// s'ecrit. Elle se lit et s'ecrit par son nom ; un nom qu'elle n'a pas se refuse a la lecture, en nommant ceux qu'elle
/// a. Dans l'ordre de ses noms : un objet indexe par elle s'ecrit comme quand son nom etait un texte. Une liste qu'un
/// texte depose dit est `droit::codes_fermes!`.
#[macro_export]
macro_rules! noms_fermes {
    ($(#[$a:meta])* $nom:ident, $quoi:literal { $( $(#[$va:meta])* $v:ident = $n:literal, )* }) => {
        $(#[$a])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $nom {
            $( $(#[$va])* $v, )*
        }

        impl $nom {
            pub const TOUTES: &'static [$nom] = &[ $( $nom::$v, )* ];

            /// Son nom, le seul endroit ou il s'ecrit.
            pub const fn nom(self) -> &'static str {
                match self { $( $nom::$v => $n, )* }
            }

            /// Celle de ce nom, s'il en est une.
            pub fn du_nom(nom: &str) -> Option<$nom> {
                $nom::TOUTES.iter().copied().find(|a| a.nom() == nom)
            }
        }

        impl Ord for $nom {
            fn cmp(&self, autre: &Self) -> std::cmp::Ordering {
                self.nom().cmp(autre.nom())
            }
        }

        impl PartialOrd for $nom {
            fn partial_cmp(&self, autre: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(autre))
            }
        }

        impl std::fmt::Display for $nom {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str(self.nom())
            }
        }

        impl serde::Serialize for $nom {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.nom())
            }
        }

        impl<'de> serde::Deserialize<'de> for $nom {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let nom = <String as serde::Deserialize>::deserialize(d)?;
                $nom::du_nom(&nom).ok_or_else(|| {
                    let connus: Vec<&str> = $nom::TOUTES.iter().map(|a| a.nom()).collect();
                    serde::de::Error::custom(format!("{} inconnu : {}. Connus : {}.", $quoi, $crate::fichiers::repr(&nom), connus.join(", ")))
                })
            }
        }
    };
}

/// Ecrit sur la sortie standard. Un tuyau ferme (`| head`) n'est pas une erreur du programme : son lecteur a fini. Une
/// autre panne du terminal n'a personne a qui se dire : elle est notee (`echec::noter_echec`, que les incidents lisent).
#[doc(hidden)]
pub fn au_terminal(texte: std::fmt::Arguments<'_>) {
    use std::io::Write;
    match std::io::stdout().write_fmt(texte) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
        Err(e) => echec::noter_echec(Some(echec::Classe::du_disque(&e)), &format!("la sortie standard refuse d'ecrire : {}", e)),
    }
}
