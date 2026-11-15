//! Le jour civil : pas de `chrono`, pas de fuseau, pas d'horloge. Un jour est un triplet (an, mois, jour) du calendrier
//! gregorien proleptique, des annees 1 a 9999.
//!
//! `lire` admet cinq formats, essayes dans un ordre fixe : `AAAA-MM-JJ`, `JJ/MM/AAAA`, `JJMMAAAA`, `AAAAMMJJ`,
//! `JJ-MM-AAAA`. Chaque champ admet sa forme courte (`2026-3-5`) ; le premier format qui lit la chaine entiere gagne,
//! et un format entame ne revient pas en arriere (`20260315` n'est pas lu en `JJMMAAAA`). Ce parcours est celui des
//! fichiers deja ecrits : une date lue aujourd'hui se lit comme elle s'est toujours lue.

use crate::echec::{refus, R};
use crate::num::{self, Decimal};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Jour {
    pub an: i32,
    pub mois: u32,
    pub jour: u32,
}

/// UNE DUREE DE DROIT (« Le glossaire ») : des jours, ou des mois de calendrier, de date a date ; jamais negative. Une
/// duree qu'un texte fixe s'ecrit `Duree::jours`, `Duree::mois`, `Duree::annees` ; un fichier la dit en nombre, lu en
/// elle a sa porte (`en_jours`, `en_mois` et leurs facultatives). Elle se compte depuis un jour (`Jour::apres`,
/// `Jour::avant`), un mois (`Mois::apres`, `Mois::avant`) ou une annee (`Annee::les_precedentes`) ; elle se mesure du
/// premier au dernier jour d'une suite de jours (`Duree::du_au`) ; deux durees de la meme unite se comparent ; elle se
/// dit dans une phrase (`Display` : « 3 jours », « 2 mois »).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Duree(Compte);

/// L'unite d'une duree, et son nombre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Compte {
    Jours(u32),
    Mois(u32),
}

impl Duree {
    /// `n` jours, qu'un texte fixe.
    pub const fn jours(n: u32) -> Duree {
        Duree(Compte::Jours(n))
    }

    /// `n` mois de calendrier, qu'un texte fixe.
    pub const fn mois(n: u32) -> Duree {
        Duree(Compte::Mois(n))
    }

    /// `n` annees, qu'un texte fixe : douze mois de calendrier chacune.
    pub const fn annees(n: u32) -> Duree {
        Duree(Compte::Mois(n * 12))
    }

    /// LES JOURS DU `debut` AU `fin`, l'un et l'autre compris ; aucun quand `fin` precede `debut`.
    pub fn du_au(debut: &Jour, fin: &Jour) -> Duree {
        // De 0001-01-01 a 9999-12-31, l'ecart tient sur 32 bits ; negatif, la suite est vide.
        let n = fin.moins(debut) + 1;
        Duree(Compte::Jours(if n > 0 { n as u32 } else { 0 }))
    }

    /// LES JOURS QU'ELLE COMPTE A PARTIR DE `debut` : ses jours, ou les jours calendaires de ses mois comptes du `debut`
    /// au meme quantieme (`Jour::apres`), ce jour exclu.
    #[track_caller]
    pub fn en_jours_depuis(self, debut: &Jour) -> R<Duree> {
        match self.0 {
            Compte::Jours(_) => Ok(self),
            Compte::Mois(_) => Ok(Duree::du_au(debut, &debut.apres(self)?.plus_jours(-1)?)),
        }
    }

    /// UN JOUR PAR SEMAINE COMMENCEE d'une duree en jours (C. trav. L. 1242-10 : « un jour par semaine ») ; refus d'une
    /// duree en mois, qui ne se dit pas en semaines.
    #[track_caller]
    pub fn un_jour_par_semaine_commencee(self) -> R<Duree> {
        match self.0 {
            Compte::Jours(n) => Ok(Duree(Compte::Jours(n.div_ceil(7)))),
            Compte::Mois(_) => Err(refus(format!("{} ne se compte pas en semaines", self))),
        }
    }

    /// ELLE ET `autre` BOUT A BOUT, de la meme unite ; refus de jours et de mois ensemble.
    #[track_caller]
    pub fn plus(self, autre: Duree) -> R<Duree> {
        match (self.0, autre.0) {
            (Compte::Jours(a), Compte::Jours(b)) => Ok(Duree(Compte::Jours(a + b))),
            (Compte::Mois(a), Compte::Mois(b)) => Ok(Duree(Compte::Mois(a + b))),
            _ => Err(refus(format!("{} et {} ne s'ajoutent pas : des jours et des mois", self, autre))),
        }
    }

    /// SA MOITIE, arrondie a l'unite du dessous (C. trav. L1225-54 : un conge « pris en compte pour moitié »).
    pub fn moitie(self) -> Duree {
        match self.0 {
            Compte::Jours(a) => Duree(Compte::Jours(a / 2)),
            Compte::Mois(a) => Duree(Compte::Mois(a / 2)),
        }
    }

    /// `n` fois elle, bout a bout (`n` : un compte).
    pub fn fois(self, n: u32) -> Duree {
        match self.0 {
            Compte::Jours(a) => Duree(Compte::Jours(a * n)),
            Compte::Mois(a) => Duree(Compte::Mois(a * n)),
        }
    }

    /// Ses mois, comptes depuis le mois `m` (`Mois::apres`) ; refus d'une duree en jours, qui ne compte pas des mois.
    #[track_caller]
    pub(crate) fn mois_pour(self, m: crate::mois::Mois) -> R<i64> {
        match self.0 {
            Compte::Mois(n) => Ok(i64::from(n)),
            Compte::Jours(_) => Err(refus(format!("{} ne se comptent pas en mois depuis {}", self, m))),
        }
    }

    /// Est-elle nulle ?
    pub fn nulle(self) -> bool {
        matches!(self.0, Compte::Jours(0) | Compte::Mois(0))
    }

    /// Lue d'un nombre entier de jours : `#[serde(deserialize_with = "socle::date::Duree::en_jours")]`.
    pub fn en_jours<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Duree, D::Error> {
        <u32 as serde::Deserialize>::deserialize(d).map(Duree::jours)
    }

    /// `en_jours`, facultatif : `#[serde(default, deserialize_with = "socle::date::Duree::en_jours_opt")]`.
    pub fn en_jours_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Duree>, D::Error> {
        <Option<u32> as serde::Deserialize>::deserialize(d).map(|n| n.map(Duree::jours))
    }

    /// Lue d'un nombre entier de mois : `#[serde(deserialize_with = "socle::date::Duree::en_mois")]`.
    pub fn en_mois<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Duree, D::Error> {
        <u32 as serde::Deserialize>::deserialize(d).map(Duree::mois)
    }

    /// SON NOMBRE DE JOURS, A L'ECRITURE D'UNE NORME qui le dit en nombre (la DSN : S21.G00.86.003) ; une duree en mois
    /// ne s'y ecrit pas.
    #[track_caller]
    pub fn jours_a_ecrire(self) -> R<u32> {
        match self.0 {
            Compte::Jours(n) => Ok(n),
            Compte::Mois(_) => Err(refus(format!("{} ne s'écrivent pas en jours", self))),
        }
    }

    /// Ecrite en son nombre de jours (un champ `…jours` d'un fichier) : `#[serde(serialize_with =
    /// "socle::date::Duree::ecrite_en_jours")]` ; une duree en mois ne s'y ecrit pas.
    pub fn ecrite_en_jours<S: serde::Serializer>(d: &Duree, s: S) -> Result<S::Ok, S::Error> {
        match d.0 {
            Compte::Jours(n) => s.serialize_u32(n),
            Compte::Mois(_) => Err(serde::ser::Error::custom(format!("{} ne s'écrivent pas en jours", d))),
        }
    }

    /// Ecrite en son nombre de mois, facultative (un champ `…mois` d'un fichier) : `#[serde(serialize_with =
    /// "socle::date::Duree::ecrite_en_mois_opt")]` ; une duree en jours ne s'y ecrit pas.
    pub fn ecrite_en_mois_opt<S: serde::Serializer>(d: &Option<Duree>, s: S) -> Result<S::Ok, S::Error> {
        match d.map(|d| d.0) {
            None => s.serialize_none(),
            Some(Compte::Mois(n)) => s.serialize_some(&n),
            Some(Compte::Jours(_)) => Err(serde::ser::Error::custom("une durée en jours ne s'écrit pas en mois")),
        }
    }

    /// `en_mois`, facultatif : `#[serde(default, deserialize_with = "socle::date::Duree::en_mois_opt")]`.
    pub fn en_mois_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Duree>, D::Error> {
        <Option<u32> as serde::Deserialize>::deserialize(d).map(|n| n.map(Duree::mois))
    }
}

/// Deux durees de la meme unite se comparent ; des jours et des mois, non (`None`).
impl PartialOrd for Duree {
    fn partial_cmp(&self, autre: &Duree) -> Option<std::cmp::Ordering> {
        match (self.0, autre.0) {
            (Compte::Jours(a), Compte::Jours(b)) | (Compte::Mois(a), Compte::Mois(b)) => Some(a.cmp(&b)),
            _ => None,
        }
    }
}

/// « 1 jour », « 3 jours », « 2 mois ».
impl std::fmt::Display for Duree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Compte::Jours(n) if n <= 1 => write!(f, "{} jour", n),
            Compte::Jours(n) => write!(f, "{} jours", n),
            Compte::Mois(n) => write!(f, "{} mois", n),
        }
    }
}

/// UNE ANCIENNETE (« Le glossaire ») : le temps passe depuis un jour, en mois de calendrier ; un age est l'anciennete
/// depuis la naissance. Elle se compte ici, de deux jours, et nulle part ailleurs : en mois et fraction du mois entame
/// (`entre`), ou en mois revolus (`revolue`) et en annees revolues (`ans_revolus`, `age`). Un seuil qu'un texte dit en
/// mois ou en annees se lit en elle a sa porte (`Deserialize` : des mois ; `en_annees`, `en_mois_entiers`,
/// `en_annees_entieres`) et elle se compare a lui ; elle se dit dans l'unite que le texte demande (`annees`,
/// `nombre_d_ans_revolus`, `semaines_entieres`), et dans une phrase en mois (`Display`, `mois`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Anciennete(Decimal);

impl Anciennete {
    pub const ZERO: Anciennete = Anciennete(Decimal::ZERO);

    /// LES MOIS COMPLETS DE `depuis` A `au`, `au` compris, ET LA FRACTION DU MOIS ENTAME (ses jours sur ceux du mois de
    /// `au`) ; rien quand `au` precede `depuis`. Le compte d'un preavis, d'une indemnite de fin de contrat, d'un maintien
    /// de salaire, d'un ferie chome (le code du travail).
    pub fn entre(depuis: &Jour, au: &Jour) -> Anciennete {
        if au < depuis {
            return Anciennete::ZERO;
        }
        let mut mois = (au.an as i64 - depuis.an as i64) * 12 + (au.mois as i64 - depuis.mois as i64);
        let anniv = depuis.jour as i64;
        let reste = if au.jour as i64 + 1 >= anniv {
            au.jour as i64 + 1 - anniv
        } else {
            mois -= 1;
            let (an, precedent) = if au.mois > 1 { (au.an, au.mois - 1) } else { (au.an - 1, 12) };
            jours_du_mois(an, precedent) as i64 + au.jour as i64 + 1 - anniv
        };
        let dans_le_mois = Decimal::from(jours_du_mois(au.an, au.mois));
        Anciennete(num::add(Decimal::from(mois), num::div(Decimal::from(reste), dans_le_mois)))
    }

    /// LES MOIS REVOLUS DE `depuis` AU `au` : un de plus a chaque quantieme de `depuis` atteint ; negatif quand `au`
    /// precede `depuis`. Le compte d'un texte qui dit des mois ou des ans revolus (une grille, la prime de presence,
    /// l'ouverture d'une prevoyance, un age).
    pub fn revolue(depuis: &Jour, au: &Jour) -> Anciennete {
        let mois = (au.an as i64 - depuis.an as i64) * 12 + (au.mois as i64 - depuis.mois as i64) - i64::from(au.jour < depuis.jour);
        Anciennete(Decimal::from(mois))
    }

    /// LE PREMIER JOUR OU L'ANCIENNETE REVOLUE DEPUIS `depuis` LA VAUT (`revolue`) : le meme quantieme autant de mois plus
    /// tard, ou le premier du mois suivant quand ce quantieme n'existe pas (31 janvier + 1 mois : le 1er mars). Une fraction
    /// de mois n'est atteinte a aucun quantieme, ni un jour hors du calendrier : `None`.
    pub fn atteinte(self, depuis: &Jour) -> Option<Jour> {
        use rust_decimal::prelude::ToPrimitive;
        if !self.0.fract().is_zero() {
            return None;
        }
        let rang = depuis.an as i64 * 12 + (depuis.mois as i64 - 1) + self.0.to_i64()?;
        let (an, mois) = ((rang / 12) as i32, (rang % 12) as u32 + 1);
        Jour::nouveau(an, mois, depuis.jour).or_else(|| {
            let (an, mois) = if mois == 12 { (an + 1, 1) } else { (an, mois + 1) };
            Jour::nouveau(an, mois, 1)
        })
    }

    /// L'ANCIENNETE DITE EN JOURS, DE `depuis` AU `au`, l'un et l'autre compris ; aucun quand `au` precede `depuis` :
    /// celle que la DSN declare (S21.G00.86.003, « en jours »).
    pub fn en_jours(depuis: &Jour, au: &Jour) -> Duree {
        Duree::du_au(depuis, au)
    }

    /// CE QU'ELLE A DE PLUS QUE `seuil` ; rien quand elle ne l'atteint pas.
    pub fn au_dela_de(self, seuil: Anciennete) -> Anciennete {
        if self.0 > seuil.0 {
            Anciennete(num::sub(self.0, seuil.0))
        } else {
            Anciennete::ZERO
        }
    }

    /// COMBIEN DE FOIS ENTIERES `pas` Y TIENT (un compte : « dix jours par période entière de cinq ans ») ; refus d'un
    /// `pas` nul.
    #[track_caller]
    pub fn fois_entieres(self, pas: Anciennete) -> R<u32> {
        use rust_decimal::prelude::ToPrimitive;
        if pas.0 <= Decimal::ZERO {
            return Err(refus(format!("une ancienneté de {} mois ne se compte pas par périodes", pas.0)));
        }
        match num::div(self.0, pas.0).floor().to_u32() {
            Some(n) => Ok(n),
            None => Err(refus(crate::num::illisible(&self.0.to_string()))),
        }
    }

    /// L'AGE au `au` de qui est ne le `naissance` : ses annees revolues.
    pub fn age(naissance: &Jour, au: &Jour) -> Anciennete {
        Anciennete::revolue(naissance, au).ans_revolus()
    }

    /// `d` mois (un seuil d'un texte, un oracle) : la porte d'un nombre de mois.
    pub fn de_mois(d: Decimal) -> Anciennete {
        Anciennete(d)
    }

    /// `d` annees (un seuil d'un texte, un oracle) : la porte d'un nombre d'annees, douze mois chacune.
    pub fn d_annees(d: Decimal) -> Anciennete {
        Anciennete(num::mul(d, Decimal::from(12)))
    }

    /// Les mois qui ne finissent pas une annee, de 0 a 12 exclu.
    fn reste_de_l_annee(self) -> Decimal {
        let r = self.0 % Decimal::from(12);
        if r < Decimal::ZERO {
            r + Decimal::from(12)
        } else {
            r
        }
    }

    /// SES ANNEES REVOLUES, en anciennete : les mois des annees entieres.
    pub fn ans_revolus(self) -> Anciennete {
        Anciennete(num::sub(self.0, self.reste_de_l_annee()))
    }

    /// LE NOMBRE DE SES ANNEES ENTIERES, ce qu'un texte compte (la prime de presence, l'allongement d'un maintien).
    #[track_caller]
    pub fn nombre_d_ans_revolus(self) -> R<i64> {
        use rust_decimal::prelude::ToPrimitive;
        match num::div(self.ans_revolus().0, Decimal::from(12)).trunc().to_i64() {
            Some(n) => Ok(n),
            None => Err(refus(crate::num::illisible(&self.0.to_string()))),
        }
    }

    /// SES MOIS COMPLETS : le mois entame ne compte pas (R1234-1).
    pub fn mois_complets(self) -> Anciennete {
        Anciennete(self.0.floor())
    }

    /// DITE EN ANNEES, a douze mois l'annee : l'unite d'un bareme qui compte par annee (R1234-2, D1237-1).
    pub fn annees(self) -> Decimal {
        num::div(self.0, Decimal::from(12))
    }

    /// DITE EN ANNEES, a vingt-huit chiffres (`d28`) : l'unite des baremes d'une convention (`droit::clauses`).
    pub fn annees_d28(self) -> crate::d28::P {
        crate::d28::P::depuis(self.0).div(crate::d28::P::entier(12))
    }

    /// LES SEMAINES CALENDAIRES ENTIERES qu'elle compte, cinquante-deux par annee : un preavis dit en jours par semaine
    /// d'anciennete (`droit::clauses`).
    pub fn semaines_entieres(self) -> crate::d28::P {
        crate::d28::P::depuis(self.0).mul(crate::d28::P::entier(52)).div(crate::d28::P::entier(12)).plancher()
    }

    /// Le nombre de ses mois, a la porte d'une phrase ou d'une ecriture.
    pub fn mois(self) -> Decimal {
        self.0
    }

    /// Lue d'un nombre de mois facultatif, `null` compris : `#[serde(deserialize_with = "socle::date::Anciennete::en_mois_opt")]`.
    pub fn en_mois_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Anciennete>, D::Error> {
        crate::forme::decimal_opt(d).map(|a| a.map(Anciennete))
    }

    /// Lue d'un nombre d'annees (un champ `…_annees` ou `age_…` d'un fichier) : `#[serde(deserialize_with = "socle::date::Anciennete::en_annees")]`.
    pub fn en_annees<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Anciennete, D::Error> {
        crate::forme::decimal(d).map(Anciennete::d_annees)
    }

    /// `en_annees`, facultatif : `#[serde(default, deserialize_with = "socle::date::Anciennete::en_annees_opt")]`.
    pub fn en_annees_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Anciennete>, D::Error> {
        crate::forme::decimal_opt(d).map(|a| a.map(Anciennete::d_annees))
    }

    /// Lue d'un nombre entier d'annees.
    pub fn en_annees_entieres<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Anciennete, D::Error> {
        <i64 as serde::Deserialize>::deserialize(d).map(|a| Anciennete::d_annees(Decimal::from(a)))
    }

    /// Lue d'un nombre entier de mois.
    pub fn en_mois_entiers<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Anciennete, D::Error> {
        <i64 as serde::Deserialize>::deserialize(d).map(|m| Anciennete(Decimal::from(m)))
    }

    /// `en_mois_entiers`, facultatif : `#[serde(default, deserialize_with = "socle::date::Anciennete::en_mois_entiers_opt")]`.
    pub fn en_mois_entiers_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Anciennete>, D::Error> {
        <Option<i64> as serde::Deserialize>::deserialize(d).map(|m| m.map(|m| Anciennete(Decimal::from(m))))
    }
}

/// Lue d'un nombre de mois, comme `forme::decimal`.
impl<'de> serde::Deserialize<'de> for Anciennete {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Anciennete, D::Error> {
        crate::forme::decimal(d).map(Anciennete)
    }
}

/// Ses mois, tels qu'ils sont (une phrase).
impl std::fmt::Display for Anciennete {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

const JOURS_PAR_MOIS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Le rang du 1970-01-01 a compter du 0001-01-01, qui vaut 1.
const ORDINAL_EPOQUE_UNIX: i64 = 719_163;

pub fn bissextile(an: i32) -> bool {
    (an % 4 == 0 && an % 100 != 0) || an % 400 == 0
}

/// `calendar.monthrange(an, mois)[1]`, pour un mois de 1 a 12.
pub fn jours_du_mois(an: i32, mois: u32) -> u32 {
    if mois == 2 && bissextile(an) {
        29
    } else {
        JOURS_PAR_MOIS[(mois - 1) as usize]
    }
}

impl Jour {
    /// `None` si le jour n'existe pas ; l'annee va de 1 a 9999.
    pub fn nouveau(an: i32, mois: u32, jour: u32) -> Option<Jour> {
        if !(1..=9999).contains(&an) || !(1..=12).contains(&mois) {
            return None;
        }
        if jour < 1 || jour > jours_du_mois(an, mois) {
            return None;
        }
        Some(Jour { an, mois, jour })
    }

    /// Une date ecrite dans l'un des cinq formats.
    #[track_caller]
    pub(crate) fn lire(texte: &str) -> R<Jour> {
        match Jour::du_texte(texte) { Some(j) => Ok(j), None => Err(refus(format!("date illisible : {}", repr_texte(texte.trim())))) }
    }

    /// Le jour que `texte` ecrit, s'il en ecrit un (`lire` dit pourquoi il n'en ecrit pas) : une phrase qui cite un jour
    /// le dit en clair, et ce qui n'est pas une date tel qu'il est ecrit.
    pub(crate) fn du_texte(texte: &str) -> Option<Jour> {
        let s = texte.trim();
        // La forme qu'ecrit le moteur (`iso`), lue d'emblee : c'est celle que le premier format apparierait.
        let o = s.as_bytes();
        if o.len() == 10 && o[4] == b'-' && o[7] == b'-' && [0, 1, 2, 3, 5, 6, 8, 9].iter().all(|&i| o[i].is_ascii_digit()) {
            let n = |a: usize, b: usize| o[a..b].iter().fold(0u32, |n, c| n * 10 + u32::from(c - b'0'));
            if let Some(j) = Jour::nouveau(n(0, 4) as i32, n(5, 7), n(8, 10)) {
                return Some(j);
            }
        }
        let c: Vec<char> = s.chars().collect();
        FORMATS.iter().find_map(|format| lire_format(format, &c))
    }

    /// `AAAA-MM-JJ` (`iso`).
    pub fn iso(&self) -> String {
        iso(self.an, self.mois, self.jour)
    }

    /// `JJ/MM/AAAA`.
    pub fn jj_mm_aaaa(&self) -> String {
        format!("{:02}/{:02}/{:04}", self.jour, self.mois, self.an)
    }

    /// `JJ/MM`, sans l'annee.
    pub fn jj_mm(&self) -> String {
        format!("{:02}/{:02}", self.jour, self.mois)
    }

    /// `JJMMAAAA`.
    pub fn jjmmaaaa(&self) -> String {
        format!("{:02}{:02}{:04}", self.jour, self.mois, self.an)
    }

    /// `AAAAMMJJ`.
    pub fn aaaammjj(&self) -> String {
        format!("{:04}{:02}{:02}", self.an, self.mois, self.jour)
    }

    /// Le 0001-01-01 vaut 1. Formule des jours depuis
    /// l'epoque (annee commencant en mars, cycles de 400 ans).
    pub fn ordinal(&self) -> i64 {
        let a = i64::from(self.an) - i64::from(self.mois <= 2);
        let ere = a.div_euclid(400);
        let annee_d_ere = a - ere * 400;
        let m = i64::from(self.mois);
        let jour_d_annee = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.jour) - 1;
        let jour_d_ere = annee_d_ere * 365 + annee_d_ere / 4 - annee_d_ere / 100 + jour_d_annee;
        ere * 146_097 + jour_d_ere - 719_468 + ORDINAL_EPOQUE_UNIX
    }

    /// Le jour de rang `ordinal` (le 0001-01-01 vaut 1). `None` hors de 0001-01-01 .. 9999-12-31.
    pub fn depuis_ordinal(ordinal: i64) -> Option<Jour> {
        let z = ordinal - ORDINAL_EPOQUE_UNIX + 719_468;
        let ere = z.div_euclid(146_097);
        let jour_d_ere = z - ere * 146_097;
        let annee_d_ere = (jour_d_ere - jour_d_ere / 1460 + jour_d_ere / 36_524 - jour_d_ere / 146_096) / 365;
        let jour_d_annee = jour_d_ere - (365 * annee_d_ere + annee_d_ere / 4 - annee_d_ere / 100);
        let mp = (5 * jour_d_annee + 2) / 153;
        let jour = (jour_d_annee - (153 * mp + 2) / 5 + 1) as u32;
        let mois = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let an = annee_d_ere + ere * 400 + i64::from(mois <= 2);
        if !(1..=9999).contains(&an) {
            return None;
        }
        Jour::nouveau(an as i32, mois, jour)
    }

    /// Le jour du `n`-ieme jour apres l'epoque Unix (1970-01-01), pour l'horloge.
    pub fn depuis_jours_unix(jours: i64) -> Option<Jour> {
        Jour::depuis_ordinal(jours + ORDINAL_EPOQUE_UNIX)
    }

    /// `date + timedelta(days=n)`, refuse hors des annees 1 a 9999.
    #[track_caller]
    pub fn plus_jours(&self, n: i64) -> R<Jour> {
        match Jour::depuis_ordinal(self.ordinal() + n) { Some(j) => Ok(j), None => Err(refus(HORS_CALENDRIER)) }
    }

    /// LE JOUR QUI SUIT DE `d` CELUI-CI : `d` jours plus tard, ou le meme quantieme `d` mois de calendrier plus tard (le
    /// dernier jour du mois s'il ne l'a pas) ; refus hors des annees 1 a 9999.
    #[track_caller]
    pub fn apres(&self, d: Duree) -> R<Jour> {
        match d.0 {
            Compte::Jours(n) => self.plus_jours(i64::from(n)),
            Compte::Mois(n) => self.plus_mois(i64::from(n)),
        }
    }

    /// LE JOUR QUI PRECEDE DE `d` CELUI-CI : `d` jours plus tot, ou le meme quantieme `d` mois de calendrier plus tot (le
    /// dernier jour du mois s'il ne l'a pas) ; refus hors des annees 1 a 9999.
    #[track_caller]
    pub fn avant(&self, d: Duree) -> R<Jour> {
        match d.0 {
            Compte::Jours(n) => self.plus_jours(-i64::from(n)),
            Compte::Mois(n) => self.plus_mois(-i64::from(n)),
        }
    }

    /// Le meme quantieme `n` mois de calendrier plus tard (`n` negatif : plus tot), ou le dernier jour du mois s'il ne
    /// l'a pas.
    #[track_caller]
    fn plus_mois(&self, n: i64) -> R<Jour> {
        let total = self.an as i64 * 12 + (self.mois as i64 - 1) + n;
        let (an, mois) = (total.div_euclid(12) as i32, (total.rem_euclid(12) + 1) as u32);
        let jour = self.jour.min(jours_du_mois(an, mois));
        match Jour::nouveau(an, mois, jour) { Some(j) => Ok(j), None => Err(refus(date_impossible(an, mois, jour))) }
    }

    /// `(self - autre).days`.
    pub fn moins(&self, autre: &Jour) -> i64 {
        self.ordinal() - autre.ordinal()
    }

    pub fn jours_du_mois(&self) -> u32 {
        jours_du_mois(self.an, self.mois)
    }

    /// 0 = lundi ... 6 = dimanche.
    pub fn jour_de_la_semaine(&self) -> u32 {
        ((self.ordinal() + 6) % 7) as u32
    }

    /// Le mois civil du jour.
    pub fn mois_civil(&self) -> crate::mois::Mois {
        crate::mois::Mois::du_jour(self)
    }

    /// L'annee civile du jour.
    pub fn annee(&self) -> crate::mois::Annee {
        crate::mois::Annee::du_jour(self)
    }
}

/// Les refus d'une date qui n'existe pas, avec l'annee, le mois ou la date en cause.
pub fn annee_impossible(an: impl std::fmt::Display) -> String {
    format!("année impossible : {}", an)
}

use crate::mois::mois_impossible;

pub fn jour_impossible(an: impl Into<i64>, mois: impl Into<i64>, jour: impl Into<i64>) -> String {
    format!("date impossible : {}", iso(an, mois, jour))
}

/// `AAAA-MM-JJ` de trois nombres tels qu'ils sont, la seule ecriture ISO d'une date : celle d'un `Jour`, d'une date
/// impossible, d'une date de naissance declaree dont le jour ou le mois sont inconnus (`00`, `99`).
pub fn iso(an: impl Into<i64>, mois: impl Into<i64>, jour: impl Into<i64>) -> String {
    format!("{:04}-{:02}-{:02}", an.into(), mois.into(), jour.into())
}

/// Ce qui rend une date impossible : l'annee d'abord, sinon le mois, sinon le jour.
pub fn date_impossible(an: impl Into<i64>, mois: impl Into<i64>, jour: impl Into<i64>) -> String {
    let (an, mois, jour) = (an.into(), mois.into(), jour.into());
    if !(1..=9999).contains(&an) {
        annee_impossible(an)
    } else if !(1..=12).contains(&mois) {
        mois_impossible(mois)
    } else {
        jour_impossible(an, mois, jour)
    }
}

pub const HORS_CALENDRIER: &str = "date hors du calendrier (années 1 à 9999)";


// ------------------------------------------------------------ la lecture d'une date

#[derive(Clone, Copy)]
enum Champ {
    An,
    Mois,
    Jour,
    Lit(char),
}

use Champ::*;

/// `%Y-%m-%d`, `%d/%m/%Y`, `%d%m%Y`, `%Y%m%d`, `%d-%m-%Y`, dans cet ordre.
const FORMATS: [&[Champ]; 5] = [
    &[An, Lit('-'), Mois, Lit('-'), Jour],
    &[Jour, Lit('/'), Mois, Lit('/'), An],
    &[Jour, Mois, An],
    &[An, Mois, Jour],
    &[Jour, Lit('-'), Mois, Lit('-'), An],
];

fn chiffre(c: &[char], i: usize) -> bool {
    c.get(i).is_some_and(|x| x.is_ascii_digit())
}

/// Les longueurs que le champ peut avaler en `pos`, dans l'ordre des
/// formes admises : la plus longue d'abord.
fn longueurs(champ: Champ, c: &[char], pos: usize) -> Vec<usize> {
    let a = c.get(pos).copied();
    let b = c.get(pos + 1).copied();
    let mut l = Vec::new();
    match champ {
        An => {
            if (0..4).all(|k| chiffre(c, pos + k)) {
                l.push(4);
            }
        }
        // 1[0-2] | 0[1-9] | [1-9]
        Mois => {
            if a == Some('1') && matches!(b, Some('0'..='2')) {
                l.push(2);
            }
            if a == Some('0') && matches!(b, Some('1'..='9')) {
                l.push(2);
            }
            if matches!(a, Some('1'..='9')) {
                l.push(1);
            }
        }
        // 3[01] | [12]\d | 0[1-9] | [1-9] | espace[1-9]
        Jour => {
            if a == Some('3') && matches!(b, Some('0'..='1')) {
                l.push(2);
            }
            if matches!(a, Some('1'..='2')) && chiffre(c, pos + 1) {
                l.push(2);
            }
            if a == Some('0') && matches!(b, Some('1'..='9')) {
                l.push(2);
            }
            if matches!(a, Some('1'..='9')) {
                l.push(1);
            }
            if a == Some(' ') && matches!(b, Some('1'..='9')) {
                l.push(2);
            }
        }
        Lit(x) => {
            if a == Some(x) {
                l.push(1);
            }
        }
    }
    l
}

/// Le nombre de `len` caracteres de `c` a partir de `de`, ses blancs autour retires ; 0 quand ce n'est pas un nombre.
fn nombre(c: &[char], de: usize, len: usize) -> u32 {
    let t: String = c[de..de + len].iter().collect();
    let t = t.trim();
    if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
        return 0;
    }
    t.bytes().fold(0u32, |n, b| n.saturating_mul(10).saturating_add(u32::from(b - b'0')))
}

/// Premier appariement de l'expression reguliere (avec retour sur les
/// alternatives), sans exiger la fin de chaine.
fn apparier(format: &[Champ], c: &[char], pos: usize, vus: &mut [u32; 3]) -> Option<usize> {
    let Some((champ, reste)) = format.split_first() else {
        return Some(pos);
    };
    for len in longueurs(*champ, c, pos) {
        let ancien = *vus;
        match champ {
            An => vus[0] = nombre(c, pos, len),
            Mois => vus[1] = nombre(c, pos, len),
            Jour => vus[2] = nombre(c, pos, len),
            Lit(_) => {}
        }
        if let Some(fin) = apparier(reste, c, pos + len, vus) {
            return Some(fin);
        }
        *vus = ancien;
    }
    None
}

/// Une date dans un format ; `None` s'il ne s'y lit pas (pas
/// d'appariement, reste de chaine, jour inexistant).
fn lire_format(format: &[Champ], c: &[char]) -> Option<Jour> {
    let mut vus = [0u32; 3];
    let fin = apparier(format, c, 0, &mut vus)?;
    if fin != c.len() {
        return None;
    }
    Jour::nouveau(vus[0] as i32, vus[1], vus[2])
}

/// Une date `AAAAMMJJ`, ou `None`.
/// `JJMMAAAA` (la DSN, la DPAE) : huit chiffres ASCII, et rien d'autre ; `None` sinon, ou hors du calendrier.
pub(crate) fn lire_jjmmaaaa(v: &str) -> Option<Jour> {
    if v.len() != 8 || !v.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = |a: usize, b: usize| v.as_bytes()[a..b].iter().fold(0u32, |x, c| x * 10 + u32::from(c - b'0'));
    Jour::nouveau(n(4, 8) as i32, n(2, 4), n(0, 2))
}

pub(crate) fn lire_aaaammjj(s: &str) -> Option<Jour> {
    let c: Vec<char> = s.chars().collect();
    lire_format(FORMATS[3], &c)
}

// ------------------------------------------------------ les formes ecrites

/// Un texte cite dans un message : entre guillemets simples, sauf s'il porte une apostrophe
/// et aucun guillemet double.
pub fn repr_texte(s: &str) -> String {
    let guillemet = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut o = String::new();
    o.push(guillemet);
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c == guillemet => {
                o.push('\\');
                o.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => o.push_str(&format!("\\x{:02x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push(guillemet);
    o
}

/// `AAAA-MM-JJ` ou `AAAAMMJJ`.
pub(crate) fn jour_iso(texte: &str) -> Option<Jour> {
    let chiffres = |t: &str| t.chars().all(|c| c.is_ascii_digit());
    let (a, m, j) = match texte.len() {
        10 if texte.is_char_boundary(4) && texte.is_char_boundary(7) && &texte[4..5] == "-" && &texte[7..8] == "-" => (&texte[..4], &texte[5..7], &texte[8..10]),
        8 if texte.is_ascii() => (&texte[..4], &texte[4..6], &texte[6..8]),
        _ => return None,
    };
    if !(chiffres(a) && chiffres(m) && chiffres(j)) {
        return None;
    }
    // Des chiffres seuls, quatre ou deux : leur valeur se compte, sans lecture qui puisse echouer.
    let valeur = |t: &str| t.bytes().fold(0u32, |n, c| n * 10 + u32::from(c - b'0'));
    Jour::nouveau(valeur(a) as i32, valeur(m), valeur(j))
}


/// UN JOUR DANS UNE DONNEE LUE (`serde`) : la forme qu'ecrit le moteur, `AAAA-MM-JJ`, et elle seule ; il s'ecrit de meme.
/// Les autres formats de `lire` sont ceux d'une saisie ou d'une piece, qui se lisent avant d'entrer dans une donnee.
impl serde::Serialize for Jour {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.iso())
    }
}

impl<'de> serde::Deserialize<'de> for Jour {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Jour, D::Error> {
        let s = String::deserialize(d)?;
        match Jour::lire(&s) {
            Ok(j) if j.iso() == s => Ok(j),
            _ => Err(serde::de::Error::custom(format!("{} n'est pas une date AAAA-MM-JJ", repr_texte(&s)))),
        }
    }
}

