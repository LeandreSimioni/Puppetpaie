//! Le droit du moteur : les baremes dates et leurs trois etages (le socle livre, la convention, la compagnie), les
//! natures de contrat, les catalogues de rubriques et d'emplois, les clauses des conventions, le plafond de la securite
//! sociale, les conges et la fin de contrat des permanents, la caisse des conges spectacles. Tout se lit a une date.
//! Le droit ne connait pas l'engagement : il recoit ce qu'il juge (une fiche, une nature, une date), il ne va pas le
//! chercher.

/// UNE CLE DU LOGICIEL, OPAQUE : un texte non vide, sans blancs de bord, gardé tel que lu ; il s'ecrit et se trie comme
/// lui. INFERENCE pour chacune (aucun texte n'en fixe la forme : ce sont
/// des cles des fichiers livres) ; aucune liste dans le code. Une cle que le moteur nomme est une constante
/// (`du_moteur`, dont le compilateur refuse une cle vide) ; une cle d'un fichier se lit (`lire`, `try_from`). `$quoi`
/// nomme la notion dans le refus.
#[macro_export]
macro_rules! cle_opaque {
    ($(#[$m:meta])* pub struct $nom:ident, $quoi:literal;) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $nom(std::borrow::Cow<'static, str>);

        impl $nom {
            /// La cle ecrite `texte` ; vide ou entouree de blancs, elle se refuse.
            pub fn lire(texte: &str) -> socle::echec::R<$nom> {
                if texte.is_empty() || texte.trim() != texte {
                    return Err(socle::refus!("{} : {} n'en est pas une (vide, ou entourée de blancs).", $quoi, socle::fichiers::repr(texte)));
                }
                Ok($nom(std::borrow::Cow::Owned(texte.to_string())))
            }

            /// LA CLE QUE LE MOTEUR NOMME, ecrite en Rust : une constante, dont le compilateur refuse une cle vide.
            pub const fn du_moteur(cle: &'static str) -> $nom {
                assert!(!cle.is_empty(), $quoi);
                $nom(std::borrow::Cow::Borrowed(cle))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $nom {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl TryFrom<String> for $nom {
            type Error = socle::echec::Echec;
            fn try_from(s: String) -> socle::echec::R<$nom> {
                $nom::lire(&s)
            }
        }

        impl From<$nom> for String {
            fn from(c: $nom) -> String {
                c.0.into_owned()
            }
        }
    };
}

pub mod adhesions;
pub mod apprentissage;
pub mod arrets;
pub mod assiette;
pub mod bareme;
pub mod base_assujettie;
pub mod bordereau;
pub mod caisse_conges;
pub mod catalogue;
pub mod clauses;
pub mod code_cotisation;
pub mod conges;
pub mod cotisation_etablissement;
pub mod cumul;
pub mod cpf_cdd;
pub mod contexte;
pub mod echeances;
pub mod emplois;
pub mod essai;
pub mod fiche;
pub mod heures_en_plus;
pub mod maintien;
pub mod majoration_chomage;
pub mod marqueurs;
pub mod minima;
pub mod missions;
pub mod nationalite;
pub mod nature_declaration;
pub mod natures;
pub mod organismes;
pub mod pas;
pub mod permanents;
pub mod periodes;
pub mod plafond;
pub mod preavis;
pub mod reduction;
pub mod forme_bareme;
pub mod source;
pub mod type_de_prime;
pub mod type_declaration;
pub mod type_taux_pas;
pub mod autre_revenu_brut;
pub mod versement;
