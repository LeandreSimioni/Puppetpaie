//! UNE ZONE D'UNE PIECE LUE : le texte qu'une piece porte a un endroit que sa norme fixe -- une rubrique d'une DSN ou
//! d'une DPAE, une position d'une AEM, un element d'un retour XML, une cellule d'un CSV livre, une date dans le nom d'un
//! fichier depose. C'est la seule porte par laquelle le texte d'une piece devient un jour, un mois, un instant, une heure
//! ou un entier ecrit en chiffres : la zone
//! se fabrique avec le format que la norme de la piece donne a cet endroit (`format`, un type par format), que le
//! lecteur de chaque piece passe ; elle ne se lit que dans ce format. Ce n'est pas une preuve d'origine : c'est la lecture
//! typee de ce qu'on lit, au format de son texte.

use crate::echec::R;

/// La lecture d'un format, que seule une `Zone` appelle : le trait ne se nomme pas hors de ce module, et sa lecture
/// exige un jeton que seul ce module fabrique (un generique qui prend un `Format` ne l'appelle pas).
mod scelle {
    pub struct Jeton(());

    pub(super) const JETON: Jeton = Jeton(());

    pub trait Scelle {
        type Lu;
        fn lire(self, texte: &str, jeton: Jeton) -> Self::Lu;
    }
}

/// Un format qu'une norme donne a une zone. Scelle : seuls les formats de `format` en sont, et leur lecture ne
/// s'appelle que par la zone (`Zone::valeur`).
pub trait Format: scelle::Scelle + Copy {}

/// Le texte d'une zone d'une piece, tel que la piece l'ecrit, et le format que sa norme lui donne.
#[derive(Debug, Clone, Copy)]
pub struct Zone<'a, F: Format> {
    texte: &'a str,
    format: F,
}

impl<'a, F: Format> Zone<'a, F> {
    /// La zone qui porte `texte`, au format `format`.
    pub fn lue(texte: &'a str, format: F) -> Zone<'a, F> {
        Zone { texte, format }
    }

    /// Ce qu'elle dit, lu dans son format.
    pub fn valeur(&self) -> <F as scelle::Scelle>::Lu {
        self.format.lire(self.texte, scelle::JETON)
    }
}

/// UN ENTIER QU'UNE ZONE ECRIT EN CHIFFRES SEULS (`format::Chiffres`), quand la zone ne dit pas seule ce qu'il compte :
/// la colonne d'une AEM imprimee, la case « NB JOURS OU CACHETS » d'un certificat. Il ne se lit qu'en la grandeur que
/// sa place dit (`en`, une fois), son echelle nulle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntierDUneZone(pub(crate) i64);

/// LES FORMATS, un type chacun.
pub mod format {
    use super::scelle::{Jeton, Scelle};
    use super::{EntierDUneZone, Format, R};
    use crate::date::Jour;
    use crate::mois::{Annee, Mois, MoisDeLAnnee, RangDansLAnnee};
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

    /// Un instant `AAAA-MM-JJTHH:MM:SS`, heure de Paris (`paris::ecrit::seconde`).
    #[derive(Debug, Clone, Copy)]
    pub struct InstantIso;

    /// Un instant `AAAAMMJJHHMMSS`, lu comme l'heure de Paris (`paris::ecrit::compacte`).
    #[derive(Debug, Clone, Copy)]
    pub struct InstantCompact;

    /// Une date de XML Schema (`paris::date_xsd`) : son jour et son fuseau, tels qu'ecrits.
    #[derive(Debug, Clone, Copy)]
    pub struct DateXsd;

    /// Une heure de XML Schema (`paris::heure_xsd`) : `paris::HeureXsdLue`.
    #[derive(Debug, Clone, Copy)]
    pub struct HeureXsd;

    /// Une heure du jour `HHMM` (l'horodatage d'une DPAE relue, H_CREATION) : quatre chiffres, et rien d'autre.
    #[derive(Debug, Clone, Copy)]
    pub struct Hhmm;

    /// Un entier ecrit en chiffres seuls, de un a dix-huit, quand sa place dit seule ce qu'il compte (une case d'un
    /// formulaire, la colonne d'une AEM imprimee) : tout autre texte (vide, un signe, un autre caractere, plus long) n'en
    /// est pas un. Il se lit en `EntierDUneZone`, que sa place lit dans sa grandeur.
    #[derive(Debug, Clone, Copy)]
    pub struct Chiffres;

    /// Une annee en chiffres seuls, de un a dix-huit (`2026` : celle d'un numero de bulletin `12/2026`, du nom d'une
    /// surcouche `2026.json`, d'une date imprimee case a case) : `None` quand le texte n'en est pas l'ecriture, et le
    /// refus d'une annee hors de 1 a 9999 (`Annee::nouvelle`).
    #[derive(Debug, Clone, Copy)]
    pub struct AnneeEnChiffres;

    /// Une annee en deux chiffres, de 2000 a 2099 (`26` : celle d'un numero de contrat `FA1.26.16`).
    #[derive(Debug, Clone, Copy)]
    pub struct Aa;

    /// Le numero d'un mois en chiffres seuls (`3`, `03`), de 1 a 12 (le mois d'une date imprimee case a case).
    #[derive(Debug, Clone, Copy)]
    pub struct MoisEnChiffres;

    /// Le jour du mois `0` en chiffres seuls (`5`, `05`) : le jour d'une date imprimee case a case, dans son mois.
    #[derive(Debug, Clone, Copy)]
    pub struct JourDuMois(pub Mois);

    /// Le rang d'un numero dans la suite de son annee, en chiffres seuls (`16` : `FA1.26.16` ; `12` : `12/2026`).
    #[derive(Debug, Clone, Copy)]
    pub struct Rang;

    /// L'en-tete `Date` d'un courriel (RFC 5322 : `Mon, 13 Oct 2026 09:12:00 +0200`), lu comme `paris::date_rfc5322` le dit.
    #[derive(Debug, Clone, Copy)]
    pub struct DateRfc5322;

    impl Format for Jjmmaaaa {}
    impl Scelle for Jjmmaaaa {
        type Lu = Option<Jour>;
        fn lire(self, t: &str, _: Jeton) -> Option<Jour> {
            crate::date::lire_jjmmaaaa(t)
        }
    }

    impl Format for Aaaammjj {}
    impl Scelle for Aaaammjj {
        type Lu = Option<Jour>;
        fn lire(self, t: &str, _: Jeton) -> Option<Jour> {
            crate::date::lire_aaaammjj(t)
        }
    }

    impl Format for JourIso {}
    impl Scelle for JourIso {
        type Lu = Option<Jour>;
        fn lire(self, t: &str, _: Jeton) -> Option<Jour> {
            crate::date::jour_iso(t).filter(|j| j.iso() == t)
        }
    }

    impl Format for JourEcrit {}
    impl Scelle for JourEcrit {
        type Lu = Option<Jour>;
        fn lire(self, t: &str, _: Jeton) -> Option<Jour> {
            Jour::du_texte(t)
        }
    }

    impl Format for MoisDsn {}
    impl Scelle for MoisDsn {
        type Lu = R<Mois>;
        fn lire(self, t: &str, _: Jeton) -> R<Mois> {
            Mois::de_la_dsn(t)
        }
    }

    impl Format for Mmaaaa {}
    impl Scelle for Mmaaaa {
        type Lu = Option<Mois>;
        fn lire(self, t: &str, _: Jeton) -> Option<Mois> {
            (t.len() == 6 && t.is_ascii()).then(|| Mois::du_texte(&format!("{}-{}", &t[2..], &t[..2]))).flatten()
        }
    }

    impl Format for InstantIso {}
    impl Scelle for InstantIso {
        type Lu = Option<paris::Moment>;
        fn lire(self, t: &str, _: Jeton) -> Option<paris::Moment> {
            paris::ecrit::seconde::lu(t)
        }
    }

    impl Format for InstantCompact {}
    impl Scelle for InstantCompact {
        type Lu = Option<paris::Moment>;
        fn lire(self, t: &str, _: Jeton) -> Option<paris::Moment> {
            paris::ecrit::compacte::lu(t)
        }
    }

    impl Format for DateXsd {}
    impl Scelle for DateXsd {
        type Lu = Option<(Jour, Option<paris::FuseauXsd>)>;
        fn lire(self, t: &str, _: Jeton) -> Option<(Jour, Option<paris::FuseauXsd>)> {
            paris::date_xsd(t)
        }
    }

    impl Format for HeureXsd {}
    impl Scelle for HeureXsd {
        type Lu = Option<paris::HeureXsdLue>;
        fn lire(self, t: &str, _: Jeton) -> Option<paris::HeureXsdLue> {
            paris::heure_xsd(t)
        }
    }

    impl Format for Hhmm {}
    impl Scelle for Hhmm {
        type Lu = Option<paris::HeureDuJour>;
        fn lire(self, t: &str, _: Jeton) -> Option<paris::HeureDuJour> {
            if t.len() != 4 || !t.is_ascii() {
                return None;
            }
            paris::HeureDuJour::lue(crate::num::entier_de_chiffres(&t[..2])?, crate::num::entier_de_chiffres(&t[2..])?)
        }
    }

    impl Format for Chiffres {}
    impl Scelle for Chiffres {
        type Lu = Option<EntierDUneZone>;
        fn lire(self, t: &str, _: Jeton) -> Option<EntierDUneZone> {
            crate::num::entier_de_chiffres(t).map(EntierDUneZone)
        }
    }

    impl Format for AnneeEnChiffres {}
    impl Scelle for AnneeEnChiffres {
        type Lu = Option<R<Annee>>;
        fn lire(self, t: &str, _: Jeton) -> Option<R<Annee>> {
            crate::num::entier_de_chiffres(t).map(Annee::nouvelle)
        }
    }

    impl Format for Aa {}
    impl Scelle for Aa {
        type Lu = Option<Annee>;
        fn lire(self, t: &str, _: Jeton) -> Option<Annee> {
            match t.len() {
                2 => Annee::nouvelle(2000 + crate::num::entier_de_chiffres(t)?).ok(),
                _ => None,
            }
        }
    }

    impl Format for MoisEnChiffres {}
    impl Scelle for MoisEnChiffres {
        type Lu = Option<MoisDeLAnnee>;
        fn lire(self, t: &str, _: Jeton) -> Option<MoisDeLAnnee> {
            let n = crate::num::entier_de_chiffres(t)?;
            MoisDeLAnnee::TOUS.into_iter().find(|m| i64::from(m.numero()) == n)
        }
    }

    impl Format for JourDuMois {}
    impl Scelle for JourDuMois {
        type Lu = Option<Jour>;
        fn lire(self, t: &str, _: Jeton) -> Option<Jour> {
            self.0.jour(u32::try_from(crate::num::entier_de_chiffres(t)?).ok()?)
        }
    }

    impl Format for Rang {}
    impl Scelle for Rang {
        type Lu = Option<RangDansLAnnee>;
        fn lire(self, t: &str, _: Jeton) -> Option<RangDansLAnnee> {
            u32::try_from(crate::num::entier_de_chiffres(t)?).ok().map(RangDansLAnnee::lu)
        }
    }

    impl Format for DateRfc5322 {}
    impl Scelle for DateRfc5322 {
        type Lu = Option<paris::Moment>;
        fn lire(self, t: &str, _: Jeton) -> Option<paris::Moment> {
            paris::date_rfc5322(t)
        }
    }
}
