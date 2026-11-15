//! LE MOIS CIVIL `AAAA-MM` : la cle d'un mois de paie, d'un fichier d'engagements (`engagements/2026-11.json`), d'un
//! dossier de documents, d'une echeance. Il se lit une fois, a sa porte : `serde` (la meme chaine dans le JSON), le nom
//! d'un fichier ou d'un dossier lu sur le disque (`fichiers::nommes_d_un_mois`), un argument tape (`socle::ligne::Argument`), une zone d'une
//! piece lue (`socle::piece::Zone`) ; un texte n'en devient pas un ailleurs (`lire` n'est pas publique). Il ne s'ecrit
//! que sous cette forme ; son annee, son numero, son nom, le suivant et le precedent se lisent de lui.
//!
//! Quatre chiffres, un tiret, un numero de 01 a 12, rien d'autre : le mois d'une paie est celui que sa DSN declare, « le
//! premier jour du mois civil au titre duquel est etablie la paie », de forme `(01)(0[1-9]|1[0-2])(20)[0-9]{2}`
//! (S20.G00.05.005, `atelier/reference/norme-dsn/texte-2027/bloc-S20.G00.05.txt` l. 122-129). La norme n'admet « 00 »
//! qu'au mois d'une date de naissance inconnue (S21.G00.30.006, `bloc-S21.G00.30.txt` l. 143-148), ce que
//! `declarations::champs` lit du mois de naissance d'un NIR : jamais au mois d'une paie. Un mois nomme aussi un fichier ou
//! un dossier : `../config` ou `2026-06/envoi` n'en sont pas.

use crate::date::{jours_du_mois, repr_texte, Jour};
use crate::echec::{refus, R};

/// Un mois civil, de 0001-01 a 9999-12. L'ordre est celui du calendrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mois {
    an: i32,
    numero: u32,
}

/// Les trois graphies d'un nom de mois que le moteur ecrit, chacune la ou elle s'ecrivait deja :
/// - `Titre` (« Aout ») : noms de fichiers (`DSN_Aout2026.dsn`), titres et sections des etats ;
/// - `Minuscule` (« aout ») : sans accent, dans une phrase (le libelle d'un mois d'engagement, d'un dossier) ;
/// - `Accentuee` (« août ») : dans une phrase en francais (courriels, contrats, avenants, ecrans).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graphie {
    Titre,
    Minuscule,
    Accentuee,
}

const MOIS_TITRE: [&str; 12] = ["Janvier", "Fevrier", "Mars", "Avril", "Mai", "Juin", "Juillet", "Aout", "Septembre", "Octobre", "Novembre", "Decembre"];
const MOIS_MINUSCULE: [&str; 12] = ["janvier", "fevrier", "mars", "avril", "mai", "juin", "juillet", "aout", "septembre", "octobre", "novembre", "decembre"];
const MOIS_ACCENTUE: [&str; 12] = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];

/// Le refus d'un numero de mois hors de 1 a 12 : « 00 » n'est pas decembre, « 13 » n'est pas janvier.
pub fn mois_impossible(mois: impl std::fmt::Display) -> String {
    format!("mois impossible : {}", mois)
}

impl Mois {
    /// Le mois `numero` (1 a 12) de l'annee `an` (1 a 9999) ; refus sinon.
    pub fn nouveau(an: i32, numero: u32) -> R<Mois> {
        if !(1..=9999).contains(&an) {
            return Err(refus(crate::date::annee_impossible(an)));
        }
        if !(1..=12).contains(&numero) {
            return Err(refus(mois_impossible(numero)));
        }
        Ok(Mois { an, numero })
    }

    /// Le mois qu'un texte depose fixe, ecrit en constante (`const`) : hors du calendrier, le code ne compile pas.
    pub const fn constante(an: i32, numero: u32) -> Mois {
        assert!(1 <= an && an <= 9999 && 1 <= numero && numero <= 12, "mois hors du calendrier");
        Mois { an, numero }
    }

    /// Un mois ecrit `AAAA-MM` ; toute autre forme se refuse.
    pub(crate) fn lire(texte: &str) -> R<Mois> {
        let b = texte.as_bytes();
        if !(b.len() == 7 && b[4] == b'-' && b[..4].iter().chain(&b[5..]).all(u8::is_ascii_digit)) {
            return Err(refus(format!("mois illisible : {} (attendu : AAAA-MM)", repr_texte(texte))));
        }
        Mois::nouveau(texte[..4].parse().map_err(|_| refus(crate::date::HORS_CALENDRIER))?, texte[5..].parse().map_err(|_| refus(crate::date::HORS_CALENDRIER))?)
    }

    /// Le mois ecrit `AAAA-MM`, s'il en est un (`lire` dit pourquoi un texte n'en est pas un).
    pub(crate) fn du_texte(texte: &str) -> Option<Mois> {
        let b = texte.as_bytes();
        if !(b.len() == 7 && b[4] == b'-' && b[..4].iter().chain(&b[5..]).all(u8::is_ascii_digit)) {
            return None;
        }
        let nombre = |s: &[u8]| s.iter().fold(0u32, |n, c| n * 10 + u32::from(c - b'0'));
        let (an, numero) = (nombre(&b[..4]) as i32, nombre(&b[5..]));
        ((1..=9999).contains(&an) && (1..=12).contains(&numero)).then_some(Mois { an, numero })
    }

    /// Le mois tel qu'une DSN le declare (S20.G00.05.005) : `01MMAAAA`, le premier jour du mois ; toute autre forme se
    /// refuse.
    pub(crate) fn de_la_dsn(texte: &str) -> R<Mois> {
        let b = texte.as_bytes();
        if !(b.len() == 8 && b.starts_with(b"01") && b.iter().all(u8::is_ascii_digit)) {
            return Err(refus(format!("mois de DSN illisible : {} (attendu : 01MMAAAA)", repr_texte(texte))));
        }
        Mois::nouveau(texte[4..].parse().map_err(|_| refus(crate::date::HORS_CALENDRIER))?, texte[2..4].parse().map_err(|_| refus(crate::date::HORS_CALENDRIER))?)
    }

    /// Le mois que nomme un fichier ou un dossier du disque, sans son extension (`engagements/2026-11.json`,
    /// `sorties/2026-11/`), lu par `fichiers::nommes_d_un_mois` ; `None` : ce nom n'est pas un mois.
    pub(crate) fn du_nom(nom: &std::ffi::OsStr) -> Option<Mois> {
        nom.to_str().and_then(Mois::du_texte)
    }

    /// Le mois du jour `j`.
    pub fn du_jour(j: &Jour) -> Mois {
        Mois { an: j.an, numero: j.mois }
    }

    /// Son annee civile.
    pub fn annee(self) -> Annee {
        Annee(self.an)
    }

    /// Le numero du mois, de 1 a 12.
    pub fn numero(self) -> u32 {
        self.numero
    }

    /// Le nom du mois dans la graphie `g`.
    pub fn nom(self, g: Graphie) -> &'static str {
        self.de_l_annee().nom(g)
    }

    /// Le mois de l'annee qu'il est, sans son annee.
    pub fn de_l_annee(self) -> MoisDeLAnnee {
        MoisDeLAnnee(self.numero)
    }

    /// « octobre 2026 », dans la graphie `g`.
    pub fn libelle(self, g: Graphie) -> String {
        format!("{} {:04}", self.nom(g), self.an)
    }

    /// L'etiquette du mois dans le nom d'un fichier : « Aout2026 » (le nom en titre, sans accent).
    pub fn etiquette(self) -> String {
        format!("{}{:04}", self.nom(Graphie::Titre), self.an)
    }

    /// Le mois `n` mois plus loin (en arriere si `n` est negatif) ; refus hors des annees 1 a 9999.
    pub fn plus(self, n: i32) -> R<Mois> {
        let rang = i64::from(self.an) * 12 + i64::from(self.numero) - 1 + i64::from(n);
        let an = i32::try_from(rang.div_euclid(12)).map_err(|_| refus(crate::date::HORS_CALENDRIER))?;
        Mois::nouveau(an, (rang.rem_euclid(12) + 1) as u32).map_err(|_| refus(crate::date::HORS_CALENDRIER))
    }

    /// Le mois suivant (decembre 9999 n'en a pas : il se rend lui-meme, aucun mois de paie n'y va).
    pub fn suivant(self) -> Mois {
        match (self.an, self.numero) {
            (9999, 12) => self,
            (an, 12) => Mois { an: an + 1, numero: 1 },
            (an, numero) => Mois { an, numero: numero + 1 },
        }
    }

    /// Le mois precedent (janvier de l'an 1 n'en a pas : il se rend lui-meme).
    pub fn precedent(self) -> Mois {
        match (self.an, self.numero) {
            (1, 1) => self,
            (an, 1) => Mois { an: an - 1, numero: 12 },
            (an, numero) => Mois { an, numero: numero - 1 },
        }
    }

    /// Le nombre de jours du mois.
    pub fn jours(self) -> u32 {
        jours_du_mois(self.an, self.numero)
    }

    /// Le premier jour du mois.
    pub fn premier_jour(self) -> Jour {
        Jour { an: self.an, mois: self.numero, jour: 1 }
    }

    /// Le dernier jour du mois.
    pub fn dernier_jour(self) -> Jour {
        Jour { an: self.an, mois: self.numero, jour: self.jours() }
    }

    /// Le jour `jour` du mois, s'il existe.
    pub fn jour(self, jour: u32) -> Option<Jour> {
        Jour::nouveau(self.an, self.numero, jour)
    }

    /// Le jour `jour` du mois, ou le refus qui dit la date impossible.
    #[track_caller]
    pub fn jour_exige(self, jour: u32) -> R<Jour> {
        match self.jour(jour) {
            Some(j) => Ok(j),
            None => Err(refus(crate::date::jour_impossible(self.an, self.numero, jour))),
        }
    }

    /// Le jour `j` est-il dans ce mois ?
    pub fn contient(self, j: &Jour) -> bool {
        Mois::du_jour(j) == self
    }

    /// Les mois de `debut` a `fin` inclus (`Engagement::mois_du_contrat`) ; aucun si `fin` precede `debut`.
    pub fn entre(debut: &Jour, fin: &Jour) -> Vec<Mois> {
        Mois::du_jour(debut).jusqu_a(Mois::du_jour(fin))
    }

    /// Les mois de celui-ci a `dernier` inclus ; aucun si `dernier` le precede.
    pub fn jusqu_a(self, dernier: Mois) -> Vec<Mois> {
        let mut m = self;
        let mut sortie = Vec::new();
        while m <= dernier {
            sortie.push(m);
            if m.suivant() == m {
                break;
            }
            m = m.suivant();
        }
        sortie
    }
}

/// UN MOIS DE L'ANNEE, sans annee : janvier a decembre (la colonne d'un mois au recapitulatif annuel). Il ne se
/// fabrique que d'un mois civil (`Mois::de_l_annee`) ou parmi les douze (`MoisDeLAnnee::TOUS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoisDeLAnnee(u32);

impl MoisDeLAnnee {
    /// Les douze, de janvier a decembre.
    pub const TOUS: [MoisDeLAnnee; 12] = [
        MoisDeLAnnee(1), MoisDeLAnnee(2), MoisDeLAnnee(3), MoisDeLAnnee(4), MoisDeLAnnee(5), MoisDeLAnnee(6),
        MoisDeLAnnee(7), MoisDeLAnnee(8), MoisDeLAnnee(9), MoisDeLAnnee(10), MoisDeLAnnee(11), MoisDeLAnnee(12),
    ];

    /// Mars (le 31 mars borne l'annee de conges des certificats Congés Spectacles).
    pub const MARS: MoisDeLAnnee = MoisDeLAnnee(3);

    /// Son numero, de 1 a 12.
    pub fn numero(self) -> u32 {
        self.0
    }

    /// Son nom dans la graphie `g` : la seule table des noms de mois du moteur.
    pub fn nom(self, g: Graphie) -> &'static str {
        let table = match g {
            Graphie::Titre => &MOIS_TITRE,
            Graphie::Minuscule => &MOIS_MINUSCULE,
            Graphie::Accentuee => &MOIS_ACCENTUE,
        };
        table[(self.0 - 1) as usize]
    }
}

/// La lecture de serde : un mois de l'annee s'ecrit par son numero (`3` : mars) dans un fichier qui le dit ainsi
/// (`echeancier-base.json`) ; hors de 1 a 12, refus.
impl<'de> serde::Deserialize<'de> for MoisDeLAnnee {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<MoisDeLAnnee, D::Error> {
        let n = <u32 as serde::Deserialize>::deserialize(d)?;
        match (1..=12).contains(&n) {
            true => Ok(MoisDeLAnnee(n)),
            false => Err(serde::de::Error::custom(format!("{} n'est pas un mois de l'année (1 à 12)", n))),
        }
    }
}

/// UNE ANNEE CIVILE, de 0001 a 9999 : la cle d'une annee de salaires (la CASC-SVP se regularise et se taxe par annee,
/// `donnees/casc-svp-base.json`). Elle se lit une fois (`serde`) et s'ecrit en nombre dans le JSON (`2025`) ; cle d'un
/// objet, en texte, comme toute cle (`"2025"`) ; l'ordre est celui du calendrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Annee(i32);

impl Annee {
    /// L'annee `an` (1 a 9999) ; refus sinon.
    pub fn nouvelle(an: i64) -> R<Annee> {
        if !(1..=9999).contains(&an) {
            return Err(refus(crate::date::annee_impossible(an)));
        }
        // De 1 a 9999, l'annee tient sur 32 bits.
        Ok(Annee(an as i32))
    }

    /// L'annee du mois `m`.
    pub fn du_mois(m: Mois) -> Annee {
        Annee(m.an)
    }

    /// L'annee `an` qu'un texte depose fixe, ecrite en constante (`const`) : hors de 1 a 9999, le code ne compile pas.
    pub const fn constante(an: i32) -> Annee {
        assert!(1 <= an && an <= 9999, "année hors du calendrier");
        Annee(an)
    }

    /// Son millesime, au calcul du calendrier (`feries`) et aux ecritures du socle.
    pub(crate) fn nombre(self) -> i32 {
        self.0
    }

    /// L'annee du jour `j`.
    pub fn du_jour(j: &Jour) -> Annee {
        Annee(j.an)
    }

    /// L'annee `n` ans plus loin (en arriere si `n` est negatif) ; refus hors des annees 1 a 9999.
    pub fn plus(self, n: i32) -> R<Annee> {
        Annee::nouvelle(i64::from(self.0) + i64::from(n))
    }

    /// L'annee d'avant ; refus avant l'an 1.
    pub fn precedente(self) -> R<Annee> {
        self.plus(-1)
    }

    /// L'annee d'apres ; refus apres 9999.
    pub fn suivante(self) -> R<Annee> {
        self.plus(1)
    }

    /// Les annees de celle-ci a `derniere` incluse ; aucune si `derniere` la precede.
    pub fn jusqu_a(self, derniere: Annee) -> Vec<Annee> {
        (self.0..=derniere.0).map(Annee).collect()
    }

    /// Les `n` annees qui la precedent, de la plus ancienne a celle d'avant ; celles d'avant l'an 1 n'existent pas.
    pub fn les_precedentes(self, n: usize) -> Vec<Annee> {
        let debut = (i64::from(self.0) - n as i64).max(1);
        (debut..i64::from(self.0)).map(|a| Annee(a as i32)).collect()
    }

    /// Le mois `m` de l'annee.
    pub fn mois(self, m: MoisDeLAnnee) -> Mois {
        Mois { an: self.0, numero: m.0 }
    }

    /// Ses douze mois, de janvier a decembre.
    pub fn les_mois(self) -> [Mois; 12] {
        MoisDeLAnnee::TOUS.map(|m| self.mois(m))
    }

    /// Janvier de l'annee.
    pub fn janvier(self) -> Mois {
        Mois { an: self.0, numero: 1 }
    }

    /// Decembre de l'annee.
    pub fn decembre(self) -> Mois {
        Mois { an: self.0, numero: 12 }
    }

    /// Le 1er janvier.
    pub fn premier_jour(self) -> Jour {
        self.janvier().premier_jour()
    }

    /// Le 31 decembre.
    pub fn dernier_jour(self) -> Jour {
        self.decembre().dernier_jour()
    }

    /// Est-elle bissextile ?
    pub fn bissextile(self) -> bool {
        crate::date::bissextile(self.0)
    }
}

impl std::fmt::Display for Annee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}", self.0)
    }
}

impl serde::Serialize for Annee {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i32(self.0)
    }
}

/// La lecture de serde, un nombre (une cle d'objet, son texte), dont le refus se dit dans le sien.
impl<'de> serde::Deserialize<'de> for Annee {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Annee, D::Error> {
        Annee::nouvelle(<i64 as serde::Deserialize>::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Le mois dont parle un acte, ou le jour qui le date (l'evenement d'un signalement) : ecrit `AAAA-MM` ou `AAAA-MM-JJ`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoisOuJour {
    Mois(Mois),
    Jour(Jour),
}

impl MoisOuJour {
    /// Son mois : le mois meme, ou celui du jour.
    pub fn mois(self) -> Mois {
        match self {
            MoisOuJour::Mois(m) => m,
            MoisOuJour::Jour(j) => j.mois_civil(),
        }
    }
}

impl std::fmt::Display for MoisOuJour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoisOuJour::Mois(m) => m.fmt(f),
            MoisOuJour::Jour(j) => f.write_str(&j.iso()),
        }
    }
}

/// Deux se comparent comme leurs textes : un mois precede les jours de ce mois.
impl PartialOrd for MoisOuJour {
    fn partial_cmp(&self, autre: &MoisOuJour) -> Option<std::cmp::Ordering> {
        Some(self.cmp(autre))
    }
}

impl Ord for MoisOuJour {
    fn cmp(&self, autre: &MoisOuJour) -> std::cmp::Ordering {
        self.to_string().cmp(&autre.to_string())
    }
}

impl serde::Serialize for MoisOuJour {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl std::fmt::Display for Mois {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}", self.an, self.numero)
    }
}

impl serde::Serialize for Mois {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// La lecture de serde, dont le refus se dit dans le sien.
impl<'de> serde::Deserialize<'de> for Mois {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Mois, D::Error> {
        Mois::lire(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

impl From<Mois> for String {
    fn from(m: Mois) -> String {
        m.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un mois se lit `AAAA-MM`, de 01 a 12, et rien d'autre (S20.G00.05.005) ; le JSON garde la meme chaine.
    #[test]
    fn un_mois_se_lit_aaaa_mm_et_rien_d_autre() {
        let m = Mois::lire("2026-12").unwrap();
        assert_eq!((m.annee(), m.numero(), m.to_string()), (Annee::nouvelle(2026).unwrap(), 12, "2026-12".to_string()));
        assert_eq!(m.suivant().to_string(), "2027-01");
        assert_eq!(m.suivant().precedent(), m);
        for faux in ["2026-13", "2026-00", "2026-1", "../config", "2026-06/envoi", "2026-12-01", " 2026-12"] {
            assert!(Mois::lire(faux).is_err(), "{} n'est pas un mois", faux);
        }
        assert_eq!(serde_json::to_value(m).unwrap(), serde_json::json!("2026-12"));
        assert_eq!(serde_json::from_value::<Mois>(serde_json::json!("2026-02")).unwrap().dernier_jour().iso(), "2026-02-28");
        assert!(serde_json::from_value::<Mois>(serde_json::json!("2026-13")).is_err());
    }
}
