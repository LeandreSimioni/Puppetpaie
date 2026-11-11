//! UNE ZONE D'UNE PIECE LUE : le texte qu'une piece porte a un endroit que sa norme fixe -- une rubrique d'une DSN ou
//! d'une DPAE, une position d'une AEM, un element d'un retour XML, une cellule d'un CSV livre, une date dans le nom d'un
//! fichier depose. C'est la seule porte par laquelle le texte d'une piece devient un jour, un mois ou un instant : la zone
//! se fabrique avec le format que la norme de la piece donne a cet endroit (`format`, un type par format), que le
//! lecteur de chaque piece passe ; elle ne se lit que dans ce format. Ce n'est pas une preuve d'origine : c'est la lecture
//! typee de ce qu'on lit, au format de son texte.

use crate::echec::R;

/// La lecture d'un format, que seule une `Zone` appelle : le trait ne se nomme pas hors de ce module.
mod scelle {
    pub trait Scelle {
        type Lu;
        fn lire(texte: &str) -> Self::Lu;
    }
}

/// Un format qu'une norme donne a une zone. Scelle : seuls les formats de `format` en sont, et leur lecture ne
/// s'appelle que par la zone (`Zone::valeur`).
pub trait Format: scelle::Scelle {}

/// Le texte d'une zone d'une piece, tel que la piece l'ecrit, et le format que sa norme lui donne.
#[derive(Debug, Clone, Copy)]
pub struct Zone<'a, F: Format> {
    texte: &'a str,
    format: std::marker::PhantomData<F>,
}

impl<'a, F: Format> Zone<'a, F> {
    /// La zone qui porte `texte`, au format `_format`.
    pub fn lue(texte: &'a str, _format: F) -> Zone<'a, F> {
        Zone { texte, format: std::marker::PhantomData }
    }

    /// Ce qu'elle dit, lu dans son format.
    pub fn valeur(&self) -> <F as scelle::Scelle>::Lu {
        <F as scelle::Scelle>::lire(self.texte)
    }
}

/// LES FORMATS, un type chacun.
pub mod format {
    use super::scelle::Scelle;
    use super::{Format, R};
    use crate::date::Jour;
    use crate::mois::Mois;
    use crate::paris;

    /// Un jour `JJMMAAAA` (la DSN, la DPAE, un compte rendu d'organisme) : huit chiffres, et rien d'autre.
    #[derive(Debug, Clone, Copy)]
    pub struct Jjmmaaaa;

    /// Un jour `AAAAMMJJ` (l'AEM, le nom d'un fichier depose, une date KALI).
    #[derive(Debug, Clone, Copy)]
    pub struct Aaaammjj;

    /// Un jour `AAAA-MM-JJ`, et cette forme seule (une rubrique de nature date d'un retour NEOReS).
    #[derive(Debug, Clone, Copy)]
    pub struct JourIso;

    /// Un jour ecrit dans l'un des cinq formats de `date` (un CSV livre, un PDF importe, un compte rendu d'organisme).
    #[derive(Debug, Clone, Copy)]
    pub struct JourEcrit;

    /// Le mois d'une DSN (S20.G00.05.005) : `01MMAAAA`.
    #[derive(Debug, Clone, Copy)]
    pub struct MoisDsn;

    /// Un mois `MMAAAA` (l'AEM imprimee, ses chiffres case a case) : six chiffres, et rien d'autre.
    #[derive(Debug, Clone, Copy)]
    pub struct Mmaaaa;

    /// Un instant `AAAA-MM-JJTHH:MM:SS`.
    #[derive(Debug, Clone, Copy)]
    pub struct InstantIso;

    /// Un instant `AAAAMMJJHHMMSS`.
    #[derive(Debug, Clone, Copy)]
    pub struct InstantCompact;

    /// Une date de XML Schema.
    #[derive(Debug, Clone, Copy)]
    pub struct DateXsd;

    /// Une heure de XML Schema.
    #[derive(Debug, Clone, Copy)]
    pub struct HeureXsd;

    impl Format for Jjmmaaaa {}
    impl Scelle for Jjmmaaaa {
        type Lu = Option<Jour>;
        fn lire(t: &str) -> Option<Jour> {
            crate::date::lire_jjmmaaaa(t)
        }
    }

    impl Format for Aaaammjj {}
    impl Scelle for Aaaammjj {
        type Lu = Option<Jour>;
        fn lire(t: &str) -> Option<Jour> {
            crate::date::lire_aaaammjj(t)
        }
    }

    impl Format for JourIso {}
    impl Scelle for JourIso {
        type Lu = Option<Jour>;
        fn lire(t: &str) -> Option<Jour> {
            crate::date::jour_iso(t).filter(|j| j.iso() == t)
        }
    }

    impl Format for JourEcrit {}
    impl Scelle for JourEcrit {
        type Lu = Option<Jour>;
        fn lire(t: &str) -> Option<Jour> {
            Jour::du_texte(t)
        }
    }

    impl Format for MoisDsn {}
    impl Scelle for MoisDsn {
        type Lu = R<Mois>;
        fn lire(t: &str) -> R<Mois> {
            Mois::de_la_dsn(t)
        }
    }

    impl Format for Mmaaaa {}
    impl Scelle for Mmaaaa {
        type Lu = Option<Mois>;
        fn lire(t: &str) -> Option<Mois> {
            (t.len() == 6 && t.is_ascii()).then(|| Mois::du_texte(&format!("{}-{}", &t[2..], &t[..2]))).flatten()
        }
    }

    impl Format for InstantIso {}
    impl Scelle for InstantIso {
        type Lu = Option<paris::InstantEcrit>;
        fn lire(t: &str) -> Option<paris::InstantEcrit> {
            paris::InstantEcrit::lire(t)
        }
    }

    impl Format for InstantCompact {}
    impl Scelle for InstantCompact {
        type Lu = Option<paris::InstantCompact>;
        fn lire(t: &str) -> Option<paris::InstantCompact> {
            paris::InstantCompact::lire(t)
        }
    }

    impl Format for DateXsd {}
    impl Scelle for DateXsd {
        type Lu = Option<paris::DateXsd>;
        fn lire(t: &str) -> Option<paris::DateXsd> {
            paris::DateXsd::lire(t)
        }
    }

    impl Format for HeureXsd {}
    impl Scelle for HeureXsd {
        type Lu = Option<paris::HeureXsd>;
        fn lire(t: &str) -> Option<paris::HeureXsd> {
            paris::HeureXsd::lire(t)
        }
    }
}
