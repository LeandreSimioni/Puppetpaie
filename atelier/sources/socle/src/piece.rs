//! UNE ZONE D'UNE PIECE LUE : le texte qu'une piece porte a un endroit que sa norme fixe -- une rubrique d'une DSN ou
//! d'une DPAE, une position d'une AEM, un element d'un retour XML, une cellule d'un CSV livre, une date dans le nom d'un
//! fichier depose. C'est la seule porte par laquelle le texte d'une piece devient un jour, un mois ou un instant : chaque
//! lecture dit le format que la norme de la piece ecrit, et rien d'autre ne lit un texte en date hors de `socle`.

use crate::date::Jour;
use crate::echec::R;
use crate::mois::Mois;
use crate::paris::{DateXsd, HeureXsd, InstantCompact, InstantEcrit};

/// Le texte d'une zone d'une piece, tel que la piece l'ecrit.
#[derive(Debug, Clone, Copy)]
pub struct Zone<'a>(&'a str);

impl<'a> Zone<'a> {
    /// La zone qui porte `texte`.
    pub fn lue(texte: &'a str) -> Zone<'a> {
        Zone(texte)
    }

    /// Un jour `JJMMAAAA` (la DSN, la DPAE, un compte rendu d'organisme) : huit chiffres, et rien d'autre.
    pub fn jour_jjmmaaaa(self) -> Option<Jour> {
        crate::date::lire_jjmmaaaa(self.0)
    }

    /// Un jour `AAAAMMJJ` (l'AEM, le nom d'un fichier depose).
    pub fn jour_aaaammjj(self) -> Option<Jour> {
        crate::date::lire_aaaammjj(self.0)
    }

    /// Un jour `AAAA-MM-JJ`, et cette forme seule (une rubrique de nature date d'un retour NEOReS).
    pub fn jour_iso(self) -> Option<Jour> {
        crate::date::jour_iso(self.0).filter(|j| j.iso() == self.0)
    }

    /// Un jour ecrit dans l'un des cinq formats de `date` (un CSV livre, un PDF importe, un compte rendu d'organisme).
    pub fn jour(self) -> Option<Jour> {
        Jour::du_texte(self.0)
    }

    /// Le mois d'une DSN (S20.G00.05.005) : `01MMAAAA`.
    pub fn mois_dsn(self) -> R<Mois> {
        Mois::de_la_dsn(self.0)
    }

    /// Un mois `MMAAAA` (l'AEM imprimee, ses chiffres case a case) : six chiffres, et rien d'autre.
    pub fn mois_mmaaaa(self) -> Option<Mois> {
        let t = self.0;
        (t.len() == 6 && t.is_ascii()).then(|| Mois::du_texte(&format!("{}-{}", &t[2..], &t[..2]))).flatten()
    }

    /// Un instant `AAAA-MM-JJTHH:MM:SS`.
    pub fn instant_ecrit(self) -> Option<InstantEcrit> {
        InstantEcrit::lire(self.0)
    }

    /// Un instant `AAAAMMJJHHMMSS`.
    pub fn instant_compact(self) -> Option<InstantCompact> {
        InstantCompact::lire(self.0)
    }

    /// Une date de XML Schema.
    pub fn date_xsd(self) -> Option<DateXsd> {
        DateXsd::lire(self.0)
    }

    /// Une heure de XML Schema.
    pub fn heure_xsd(self) -> Option<HeureXsd> {
        HeureXsd::lire(self.0)
    }
}
