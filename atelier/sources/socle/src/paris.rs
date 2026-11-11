//! L'heure locale du poste : Europe/Paris, sans lire le fuseau du systeme.
//!
//! Les dates et heures que le logiciel ecrit sont celles de Paris, quel que soit le fuseau du poste. Le moteur n'a pas
//! de fuseau : il applique la regle de l'heure d'ete de l'Union europeenne
//! (directive 2000/84/CE, art. 2 et 3, en vigueur a compter de 2002 ;
//! `atelier/reference/sources/union-europeenne/eurlex-directive-2000-84-ce-heure-d-ete-20010202.txt`) :
//! UTC+2 du dernier dimanche de mars (1 h UTC) au dernier dimanche d'octobre
//! (1 h UTC), UTC+1 le reste de l'annee. Avant 2002, d'autres directives
//! reglaient la periode ; aucune date de fichier du logiciel n'y remonte.
//!
//! UN INSTANT EST UN `Moment` : des secondes depuis 1970, dont le jour, en UTC comme a Paris, est un jour du calendrier
//! (`Jour` : annees 1 a 9999). Il ne se construit que par ses constructeurs, qui le verifient a sa source (l'horloge,
//! un jour pose, un instant ecrit dans un fichier, la date d'un fichier) : les fonctions d'ici sont totales.

use crate::date::Jour;

/// Le premier instant : le 01/01/0001 a 0 h UTC.
const PREMIER: i64 = (1 - 719_163) * 86_400;
/// Le dernier instant dont l'heure de Paris, arrondie a la seconde, tombe encore le 31/12/9999 : deux heures et une
/// seconde avant sa fin en UTC (l'heure d'ete ajoute deux heures).
const DERNIER: i64 = (3_652_059 + 1 - 719_163) * 86_400 - 7_201;

/// Les nanosecondes d'une seconde.
const NANOS: i128 = 1_000_000_000;

/// UN INSTANT, en nanosecondes depuis le 01/01/1970 (UTC), dans le calendrier (le module) : un entier, exact. La montre
/// le donne a la microseconde ; un instant ecrit dans un fichier se lit tel qu'il est ecrit, a la nanoseconde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Moment(i128);

impl Moment {
    /// L'instant de `ns` nanosecondes depuis 1970 ; `None` hors du calendrier.
    fn de_nanos(ns: i128) -> Option<Moment> {
        (ns >= PREMIER as i128 * NANOS && ns <= DERNIER as i128 * NANOS).then_some(Moment(ns))
    }

    /// L'instant de `t` secondes entieres depuis 1970 ; `None` hors du calendrier.
    pub fn de_secondes(t: i64) -> Option<Moment> {
        Moment::de_nanos(t as i128 * NANOS)
    }

    /// L'instant d'une duree depuis 1970 (la montre, la date d'un fichier), a la nanoseconde ; `avant_1970` : avant.
    pub fn de_duree(d: std::time::Duration, avant_1970: bool) -> Option<Moment> {
        let ns = d.as_nanos() as i128;
        Moment::de_nanos(if avant_1970 { -ns } else { ns })
    }

    /// L'instant de `jour` a `heure:minute:seconde`, heure de Paris ; `None` hors du calendrier.
    pub fn a_paris(jour: Jour, heure: i64, minute: i64, seconde: i64) -> Option<Moment> {
        let local = (jour.ordinal() - 719_163) * 86_400 + heure * 3_600;
        // Le decalage de Paris a cet instant : celui de l'instant local lu comme UTC, corrige une fois (un changement
        // d'heure ne tombe jamais a l'heure d'un pas).
        let approx = local - decalage_brut(local)?;
        Moment::de_secondes(local - decalage_brut(approx)? + minute * 60 + seconde)
    }

    /// Les secondes entieres depuis 1970 (l'instant arrondi a la seconde d'avant).
    pub fn entieres(self) -> i64 {
        self.0.div_euclid(NANOS) as i64
    }

    /// Les nanosecondes passees depuis la seconde entiere (`entieres`), de 0 a 999 999 999.
    pub fn nanosecondes(self) -> u32 {
        self.0.rem_euclid(NANOS) as u32
    }

    /// L'instant arrondi a la microseconde la plus proche (au pair a egalite) : la montre ne dit pas plus fin.
    /// Les bornes du calendrier sont des secondes entieres : l'arrondi y reste.
    pub fn a_la_microseconde(self) -> Moment {
        let us = self.0.div_euclid(1_000);
        let reste = self.0.rem_euclid(1_000);
        let us = if reste > 500 || (reste == 500 && us % 2 != 0) { us + 1 } else { us };
        Moment(us * 1_000)
    }

    /// `secondes` plus tard (moins tot : negatives) ; `None` hors du calendrier.
    pub fn plus(self, secondes: i64) -> Option<Moment> {
        Moment::de_nanos(self.0 + secondes as i128 * NANOS)
    }

    /// `d` plus tard, a la nanoseconde ; `None` hors du calendrier.
    pub fn plus_duree(self, d: std::time::Duration) -> Option<Moment> {
        Moment::de_nanos(self.0 + d.as_nanos() as i128)
    }

    /// La duree de `avant` a cet instant ; `None` si `avant` est apres.
    pub fn depuis(self, avant: Moment) -> Option<std::time::Duration> {
        let ns = self.0.checked_sub(avant.0).filter(|ns| *ns >= 0)?;
        Some(std::time::Duration::new((ns / NANOS) as u64, (ns % NANOS) as u32))
    }

    /// LA GRAINE DES ALEAS EN SUITE FIXE (`temps::aleas`) : les 64 bits du nombre binaire IEEE 754 (double precision) le
    /// plus proche de ses secondes (au pair a egalite), calcules en entiers. Les scenarios enregistres et les fictives
    /// tirent leurs aleas de cette graine : un nom de courriel joue, un code de signature, en dependent.
    pub fn graine(self) -> u64 {
        if self.0 == 0 {
            return 0;
        }
        let signe = if self.0 < 0 { 1u64 << 63 } else { 0 };
        let a = self.0.unsigned_abs();
        // `a << k` est entre 2^82 et 2^83 : son quotient par 10^9 tient entre 2^52 et 2^53,6, la mantisse a peu pres.
        let mut k = 83 - (128 - a.leading_zeros()) as i32;
        let decale = |k: i32| if k >= 0 { a << k } else { a >> -k };
        let mut q = decale(k) / NANOS as u128;
        if q >= 1 << 53 {
            k -= 1;
            q = decale(k) / NANOS as u128;
        }
        // `a` tient sous 2^69 (l'an 9999) : `k` reste positif, rien ne se perd au decalage.
        let reste = decale(k) % NANOS as u128;
        if 2 * reste > NANOS as u128 || (2 * reste == NANOS as u128 && q % 2 == 1) {
            q += 1;
        }
        if q == 1 << 53 {
            q = 1 << 52;
            k -= 1;
        }
        let exposant = (52 - k + 1023) as u64;
        signe | exposant << 52 | (q as u64 - (1 << 52))
    }

    /// L'instant tel qu'un fichier l'ecrit (`lu` le relit) : ses secondes depuis 1970, en nombre a virgule (`1767600000.0`,
    /// `1793606401.2339187`), sans zero en queue de la fraction.
    pub fn ecrit(self) -> String {
        let fraction = format!("{:09}", self.nanosecondes());
        let fraction = fraction.trim_end_matches('0');
        format!("{}.{}", self.entieres(), if fraction.is_empty() { "0" } else { fraction })
    }

    /// L'instant d'un nombre de secondes ecrit (`1767600000`, `1793606401.2339187`, `1.7676e9`), exact, arrondi a la
    /// nanoseconde (au pair a egalite) ; `None` s'il ne se lit pas ou sort du calendrier.
    pub(crate) fn lu(texte: &str) -> Option<Moment> {
        let d = crate::ecriture::decouper(texte)?;
        let moins = d.moins;
        let chiffres = d.chiffres.trim_start_matches('0');
        // La valeur vaut `chiffres` x 10^puissance nanosecondes ; 10^39 ne tient plus sur 128 bits.
        let puissance = d.exposant.checked_sub((d.chiffres.len() - d.entiers) as i64)?.checked_add(9)?;
        if puissance.unsigned_abs() > 38 {
            return None;
        }
        let mut n: i128 = 0;
        for c in chiffres.bytes() {
            n = n.checked_mul(10)?.checked_add(i128::from(c - b'0'))?;
        }
        let ns = if puissance >= 0 {
            n.checked_mul(10i128.pow(puissance as u32))?
        } else {
            let diviseur = 10i128.pow(puissance.unsigned_abs() as u32);
            let (q, r) = (n / diviseur, n % diviseur);
            if r > diviseur - r || (r == diviseur - r && q % 2 == 1) { q + 1 } else { q }
        };
        Moment::de_nanos(if moins { -ns } else { ns })
    }

    /// Le jour de Paris de cet instant.
    pub fn jour_de_paris(self) -> Jour {
        self.local_naif().jour()
    }

    /// Le jour UTC de cet instant.
    pub fn jour_utc(self) -> Jour {
        dans_le_calendrier(self.entieres().div_euclid(86_400))
    }

    /// L'heure de Paris de cet instant, en secondes d'une epoque « naive » (l'instant arrondi a la seconde d'avant).
    pub fn local_naif(self) -> Naif {
        let t = self.entieres();
        Naif(t + decalage(self))
    }
}

/// UN INSTANT ECRIT DANS UN FICHIER (un scenario) : ses secondes depuis 1970, un nombre a virgule (`ecrit`), lues
/// contre le calendrier (`lu`).
impl serde::Serialize for Moment {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        crate::forme::nombre_du_texte(&self.ecrit()).serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for Moment {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Moment, D::Error> {
        let t = crate::forme::nombre_en_texte(d)?;
        Moment::lu(&t).ok_or_else(|| serde::de::Error::custom(format!("{} s n'est pas un instant du calendrier (années 1 à 9999)", t)))
    }
}

/// UNE HEURE DE PARIS, en secondes d'une epoque « naive » (l'heure locale lue comme si c'etait de l'UTC) : de quoi
/// additionner des heures et comparer deux heures locales. Son jour est un jour du calendrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Naif(i64);

impl Naif {
    /// L'heure locale naive d'un `AAAA-MM-JJ[THH:MM[:SS]]`, ou `None` (ce qui ne se lit pas, ou qui sort du calendrier).
    pub(crate) fn lire(iso: &str) -> Option<Naif> {
        let c: Vec<char> = iso.chars().collect();
        let n = |a: usize, b: usize| -> Option<i64> {
            let t: String = c.get(a..b)?.iter().collect();
            // Des chiffres seuls, au plus quatre : leur valeur se compte, sans lecture qui puisse echouer.
            t.chars().all(|x| x.is_ascii_digit()).then(|| t.bytes().fold(0i64, |n, x| n * 10 + i64::from(x - b'0')))
        };
        let jour = Jour::nouveau(n(0, 4)? as i32, n(5, 7)? as u32, n(8, 10)? as u32)?;
        let (h, m, s) = if c.len() >= 16 { (n(11, 13)?, n(14, 16)?, if c.len() >= 19 { n(17, 19)? } else { 0 }) } else { (0, 0, 0) };
        Naif::du_jour(jour, h, m, s)
    }

    /// `jour` a `heure:minute:seconde` ; `None` si l'heure deborde hors du calendrier.
    pub fn du_jour(jour: Jour, heure: i64, minute: i64, seconde: i64) -> Option<Naif> {
        Naif::de_secondes((jour.ordinal() - 719_163) * 86_400 + heure * 3_600 + minute * 60 + seconde)
    }

    /// L'heure de `local` secondes naives depuis 1970 ; `None` hors du calendrier.
    pub fn de_secondes(local: i64) -> Option<Naif> {
        Jour::depuis_jours_unix(local.div_euclid(86_400)).map(|_| Naif(local))
    }

    /// Les secondes passees depuis minuit, ce jour-la.
    pub fn seconde_du_jour(self) -> i64 {
        self.0.rem_euclid(86_400)
    }

    /// `secondes` plus tard (plus tot : negatives), refuse hors du calendrier.
    #[track_caller]
    pub fn plus(self, secondes: i64) -> crate::echec::R<Naif> {
        match Naif::de_secondes(self.0 + secondes) {
            Some(n) => Ok(n),
            None => Err(crate::echec::refus(crate::date::HORS_CALENDRIER)),
        }
    }

    /// Les secondes de `self` depuis `avant`.
    pub fn depuis(self, avant: Naif) -> i64 {
        self.0 - avant.0
    }

    /// Le jour de cette heure.
    pub fn jour(self) -> Jour {
        dans_le_calendrier(self.0.div_euclid(86_400))
    }

    /// « 12/09/2026 à 18:18 » : le jour et l'heure a la minute, tels que l'ecran les dit.
    pub fn dit(self) -> String {
        let reste = self.0.rem_euclid(86_400);
        format!("{} à {:02}:{:02}", self.jour().jj_mm_aaaa(), reste / 3_600, (reste % 3_600) / 60)
    }

    /// `AAAA-MM-JJTHH:MM:SS`.
    pub fn iso(self) -> String {
        let j = self.jour();
        let reste = self.0.rem_euclid(86_400);
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", j.an, j.mois, j.jour, reste / 3_600, (reste % 3_600) / 60, reste % 60)
    }
}

/// UNE HEURE NAIVE ECRITE DANS UN FICHIER (l'enregistrement d'un message) : ses secondes, lues contre le calendrier.
impl serde::Serialize for Naif {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i64(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Naif {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Naif, D::Error> {
        let t = i64::deserialize(d)?;
        Naif::de_secondes(t).ok_or_else(|| serde::de::Error::custom(format!("{} s n'est pas une heure du calendrier (années 1 à 9999)", t)))
    }
}

/// UN INSTANT ECRIT A LA SECONDE : l'heure de Paris d'un geste, `AAAA-MM-JJTHH:MM:SS` (la saisie d'un imprevu, la
/// validation d'une version, le depart d'un envoi). Il se lit et s'ecrit dans cette forme, et elle seule, octet pour
/// octet ; deux instants se comparent comme leurs textes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstantEcrit(Naif);

/// UN INSTANT ECRIT A LA MINUTE : l'heure de Paris `AAAA-MM-JJTHH:MM` (la pose d'un secret dans le coffre, la fin d'une
/// seance). Meme regle que l'instant a la seconde, dans sa forme a lui.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MinuteEcrite(Naif);

impl InstantEcrit {
    /// L'heure de Paris de cet instant, comme `iso_secondes` l'ecrit (la fraction arrondie a la microseconde).
    pub fn de(t: Moment) -> InstantEcrit {
        InstantEcrit(Naif(local_arrondi(t)))
    }

    /// L'heure de Paris a cet instant (`socle::temps`).
    pub fn maintenant() -> InstantEcrit {
        InstantEcrit::de(crate::temps::secondes())
    }

    /// Une heure naive, tronquee a la seconde (elle l'est deja).
    pub fn du_naif(n: Naif) -> InstantEcrit {
        InstantEcrit(n)
    }

    /// `AAAA-MM-JJTHH:MM:SS`, et rien d'autre ; `None` sinon.
    pub(crate) fn lire(texte: &str) -> Option<InstantEcrit> {
        let n = Naif::lire(texte)?;
        (n.iso() == texte).then_some(InstantEcrit(n))
    }

    /// L'heure naive.
    pub fn naif(self) -> Naif {
        self.0
    }

    /// Son jour.
    pub fn jour(self) -> Jour {
        self.0.jour()
    }

    /// `AAAA-MM-JJTHH:MM:SS`.
    pub fn iso(self) -> String {
        self.0.iso()
    }

    /// Tel que l'ecran le dit : « 12/09/2026 à 18:18 ».
    pub fn dit(self) -> String {
        self.0.dit()
    }
}

impl MinuteEcrite {
    /// L'heure de Paris de cet instant, tronquee a la minute (la fraction de seconde s'arrondit d'abord a la
    /// microseconde, au pair le plus proche).
    pub fn de(t: Moment) -> MinuteEcrite {
        MinuteEcrite::du_naif(Naif(local_arrondi(t)))
    }

    /// L'heure de Paris a cet instant (`socle::temps`).
    pub fn maintenant() -> MinuteEcrite {
        MinuteEcrite::de(crate::temps::secondes())
    }

    /// Une heure naive, tronquee a la minute.
    pub fn du_naif(n: Naif) -> MinuteEcrite {
        MinuteEcrite(Naif(n.0 - n.0.rem_euclid(60)))
    }

    /// `AAAA-MM-JJTHH:MM`, et rien d'autre ; `None` sinon.
    pub(crate) fn lire(texte: &str) -> Option<MinuteEcrite> {
        let n = Naif::lire(texte)?;
        (texte.len() == 16 && n.iso()[..16] == *texte).then_some(MinuteEcrite(n))
    }

    /// L'heure naive.
    pub fn naif(self) -> Naif {
        self.0
    }

    /// Son jour.
    pub fn jour(self) -> Jour {
        self.0.jour()
    }

    /// `AAAA-MM-JJTHH:MM`.
    pub fn iso(self) -> String {
        self.0.iso()[..16].to_string()
    }

    /// Tel que l'ecran le dit : « 12/09/2026 à 18:18 ».
    pub fn dit(self) -> String {
        self.0.dit()
    }
}

/// L'HEURE D'UNE SAISIE : l'heure de Paris `AAAA-MM-JJ HH:MM`, une espace entre le jour et l'heure (la saisie d'un
/// imprevu, de la connaissance d'un arret). Meme regle que l'instant a la seconde, dans sa forme a elle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeureDeSaisie(Naif);

impl HeureDeSaisie {
    /// L'heure de Paris de cet instant, tronquee a la minute.
    pub fn de(t: Moment) -> HeureDeSaisie {
        HeureDeSaisie(MinuteEcrite::de(t).0)
    }

    /// La meme minute, ecrite en heure de saisie.
    pub fn de_la_minute(m: MinuteEcrite) -> HeureDeSaisie {
        HeureDeSaisie(m.0)
    }

    /// L'heure de Paris a cet instant (`socle::temps`), tronquee a la minute.
    pub fn maintenant() -> HeureDeSaisie {
        HeureDeSaisie(MinuteEcrite::maintenant().0)
    }

    /// `AAAA-MM-JJ HH:MM`, et rien d'autre ; `None` sinon.
    pub(crate) fn lire(texte: &str) -> Option<HeureDeSaisie> {
        let n = Naif::lire(&texte.replacen(' ', "T", 1))?;
        let h = HeureDeSaisie(MinuteEcrite::du_naif(n).0);
        (h.iso() == texte).then_some(h)
    }

    /// L'heure naive.
    pub fn naif(self) -> Naif {
        self.0
    }

    /// Son jour.
    pub fn jour(self) -> Jour {
        self.0.jour()
    }

    /// `AAAA-MM-JJ HH:MM`.
    pub fn iso(self) -> String {
        self.0.iso()[..16].replacen('T', " ", 1)
    }

    /// Tel que l'ecran le dit : « 12/09/2026 à 18:18 ».
    pub fn dit(self) -> String {
        self.0.dit()
    }
}

impl std::fmt::Display for HeureDeSaisie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.iso())
    }
}

impl serde::Serialize for HeureDeSaisie {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.iso())
    }
}

impl<'de> serde::Deserialize<'de> for HeureDeSaisie {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<HeureDeSaisie, D::Error> {
        let s = String::deserialize(d)?;
        HeureDeSaisie::lire(&s).ok_or_else(|| serde::de::Error::custom(format!("{} n'est pas une heure AAAA-MM-JJ HH:MM", crate::date::repr_texte(&s))))
    }
}

/// UNE MINUTE DITE A LA FRANCAISE : l'heure de Paris `JJ/MM/AAAA HH:MM` (l'heure d'un passage de l'ouverture, que le
/// bandeau montre). Meme regle que l'instant a la seconde, dans sa forme a elle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MinuteDite(Naif);

impl MinuteDite {
    /// L'heure de Paris a cet instant (`socle::temps`), tronquee a la minute.
    pub fn maintenant() -> MinuteDite {
        MinuteDite(MinuteEcrite::maintenant().0)
    }

    /// `JJ/MM/AAAA HH:MM`, et rien d'autre ; `None` sinon.
    pub(crate) fn lire(texte: &str) -> Option<MinuteDite> {
        let c: Vec<char> = texte.chars().collect();
        if c.len() != 16 {
            return None;
        }
        let t = |a: usize, b: usize| -> String { c[a..b].iter().collect() };
        let n = Naif::lire(&format!("{}-{}-{}T{}", t(6, 10), t(3, 5), t(0, 2), t(11, 16)))?;
        let m = MinuteDite(MinuteEcrite::du_naif(n).0);
        (m.texte() == texte).then_some(m)
    }

    /// L'heure naive.
    pub fn naif(self) -> Naif {
        self.0
    }

    /// `JJ/MM/AAAA HH:MM`.
    pub fn texte(self) -> String {
        let reste = self.0.seconde_du_jour();
        format!("{} {:02}:{:02}", self.0.jour().jj_mm_aaaa(), reste / 3_600, (reste % 3_600) / 60)
    }
}

impl std::fmt::Display for MinuteDite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.texte())
    }
}

impl serde::Serialize for MinuteDite {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.texte())
    }
}

impl<'de> serde::Deserialize<'de> for MinuteDite {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<MinuteDite, D::Error> {
        let s = String::deserialize(d)?;
        MinuteDite::lire(&s).ok_or_else(|| serde::de::Error::custom(format!("{} n'est pas une heure JJ/MM/AAAA HH:MM", crate::date::repr_texte(&s))))
    }
}

impl std::fmt::Display for InstantEcrit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.iso())
    }
}

impl std::fmt::Display for MinuteEcrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.iso())
    }
}

impl serde::Serialize for InstantEcrit {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.iso())
    }
}

impl<'de> serde::Deserialize<'de> for InstantEcrit {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<InstantEcrit, D::Error> {
        let s = String::deserialize(d)?;
        InstantEcrit::lire(&s).ok_or_else(|| serde::de::Error::custom(format!("{} n'est pas un instant AAAA-MM-JJTHH:MM:SS", crate::date::repr_texte(&s))))
    }
}

impl serde::Serialize for MinuteEcrite {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.iso())
    }
}

impl<'de> serde::Deserialize<'de> for MinuteEcrite {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<MinuteEcrite, D::Error> {
        let s = String::deserialize(d)?;
        MinuteEcrite::lire(&s).ok_or_else(|| serde::de::Error::custom(format!("{} n'est pas un instant AAAA-MM-JJTHH:MM", crate::date::repr_texte(&s))))
    }
}

/// L'HEURE D'UNE LIGNE DU JOURNAL DES ENVOIS : l'instant du geste a la seconde (`AAAA-MM-JJTHH:MM:SS` : un envoi, un pas
/// de l'horloge, la reprise d'un certificat DPAE ou d'un bilan qui dit son heure), ou le jour seul (`AAAA-MM-JJ` : la
/// reprise d'un bilan qui ne dit que sa date de reception). Il se lit et s'ecrit dans l'une de ces deux formes, octet pour
/// octet, et deux heures se comparent comme leurs textes (un jour seul precede les instants de ce jour).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeureDuJournal {
    Instant(InstantEcrit),
    Jour(Jour),
}

impl HeureDuJournal {
    /// `AAAA-MM-JJTHH:MM:SS` ou `AAAA-MM-JJ`, et rien d'autre ; `None` sinon.
    pub(crate) fn lire(texte: &str) -> Option<HeureDuJournal> {
        match InstantEcrit::lire(texte) {
            Some(i) => Some(HeureDuJournal::Instant(i)),
            None => crate::date::jour_iso(texte).filter(|j| j.iso() == texte).map(HeureDuJournal::Jour),
        }
    }

    /// Son jour.
    pub fn jour(self) -> Jour {
        match self {
            HeureDuJournal::Instant(i) => i.jour(),
            HeureDuJournal::Jour(j) => j,
        }
    }

    /// Sa forme ecrite.
    pub fn iso(self) -> String {
        match self {
            HeureDuJournal::Instant(i) => i.iso(),
            HeureDuJournal::Jour(j) => j.iso(),
        }
    }

    /// Tel que l'ecran le dit : « 12/09/2026 à 18:18 », ou « 12/09/2026 » pour un jour seul.
    pub fn dit(self) -> String {
        match self {
            HeureDuJournal::Instant(i) => i.dit(),
            HeureDuJournal::Jour(j) => j.jj_mm_aaaa(),
        }
    }
}

impl From<InstantEcrit> for HeureDuJournal {
    fn from(i: InstantEcrit) -> HeureDuJournal {
        HeureDuJournal::Instant(i)
    }
}

impl PartialOrd for HeureDuJournal {
    fn partial_cmp(&self, autre: &HeureDuJournal) -> Option<std::cmp::Ordering> {
        Some(self.cmp(autre))
    }
}

impl Ord for HeureDuJournal {
    fn cmp(&self, autre: &HeureDuJournal) -> std::cmp::Ordering {
        self.iso().cmp(&autre.iso())
    }
}

impl std::fmt::Display for HeureDuJournal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.iso())
    }
}

impl serde::Serialize for HeureDuJournal {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.iso())
    }
}

impl<'de> serde::Deserialize<'de> for HeureDuJournal {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<HeureDuJournal, D::Error> {
        let s = String::deserialize(d)?;
        HeureDuJournal::lire(&s).ok_or_else(|| serde::de::Error::custom(format!("{} n'est ni un instant AAAA-MM-JJTHH:MM:SS ni un jour AAAA-MM-JJ", crate::date::repr_texte(&s))))
    }
}

/// UNE MINUTE UTC, `AAAA-MM-JJTHH:MMZ` : l'heure ou le serveur du filet a releve la reponse a un retour (`iso` de
/// `infra/filet/filet_serveur.py`, `FORMAT = "%Y-%m-%dT%H:%M"` suivi de `Z`). Elle se lit et s'ecrit dans cette forme,
/// et elle seule, octet pour octet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MinuteUtc(Naif);

impl MinuteUtc {
    /// `AAAA-MM-JJTHH:MMZ`, et rien d'autre ; `None` sinon.
    pub(crate) fn lire(texte: &str) -> Option<MinuteUtc> {
        MinuteEcrite::lire(texte.strip_suffix('Z')?).map(|m| MinuteUtc(m.0))
    }

    /// `AAAA-MM-JJTHH:MMZ`.
    pub fn iso(self) -> String {
        format!("{}Z", MinuteEcrite(self.0).iso())
    }
}

impl std::fmt::Display for MinuteUtc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.iso())
    }
}

impl serde::Serialize for MinuteUtc {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.iso())
    }
}

impl<'de> serde::Deserialize<'de> for MinuteUtc {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<MinuteUtc, D::Error> {
        let s = String::deserialize(d)?;
        MinuteUtc::lire(&s).ok_or_else(|| serde::de::Error::custom(format!("{} n'est pas une heure AAAA-MM-JJTHH:MMZ", crate::date::repr_texte(&s))))
    }
}

/// UN INSTANT ECRIT D'UN BLOC, `AAAAMMJJHHMMSS` : la date d'un depot que la plate-forme DSN liste (guide
/// d'implementation de l'API DSN v6.8, « Date du dépôt », `depot.date`, format `yyyyMMddhhmmss`, [14,14] ;
/// `atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`). Le guide ne dit
/// pas son fuseau : c'est l'heure telle qu'elle est ecrite. Elle se lit et s'ecrit dans cette forme, octet pour octet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstantCompact(Naif);

impl InstantCompact {
    /// Quatorze chiffres `AAAAMMJJHHMMSS` d'une heure du calendrier, et rien d'autre ; `None` sinon.
    pub(crate) fn lire(texte: &str) -> Option<InstantCompact> {
        if texte.len() != 14 || !texte.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let iso = format!("{}-{}-{}T{}:{}:{}", &texte[..4], &texte[4..6], &texte[6..8], &texte[8..10], &texte[10..12], &texte[12..14]);
        InstantEcrit::lire(&iso).map(|i| InstantCompact(i.0))
    }

    /// Une heure naive, a la seconde.
    pub fn du_naif(n: Naif) -> InstantCompact {
        InstantCompact(n)
    }

    /// L'heure naive, telle qu'elle est ecrite.
    pub fn naif(self) -> Naif {
        self.0
    }

    /// `AAAAMMJJHHMMSS`.
    pub fn compact(self) -> String {
        self.0.iso().chars().filter(char::is_ascii_digit).collect()
    }
}

impl std::fmt::Display for InstantCompact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.compact())
    }
}

impl serde::Serialize for InstantCompact {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.compact())
    }
}

impl<'de> serde::Deserialize<'de> for InstantCompact {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<InstantCompact, D::Error> {
        let s = String::deserialize(d)?;
        InstantCompact::lire(&s).ok_or_else(|| serde::de::Error::custom(format!("{} n'est pas un instant AAAAMMJJHHMMSS", crate::date::repr_texte(&s))))
    }
}

/// LE FUSEAU QU'UNE DATE OU UNE HEURE DE XML SCHEMA PEUT PORTER (XML Schema Part 2, § 3.2.7.3 : `Z`, ou `(+|-)hh:mm`,
/// `hh` de 00 a 14, `mm` de 00 a 59, et 14 seulement avec `:00`). Il garde ce qui est ecrit : son signe et ses chiffres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FuseauXsd {
    Z,
    Decalage { moins: bool, heures: u8, minutes: u8 },
}

impl FuseauXsd {
    /// `Z` ou `(+|-)hh:mm`, et rien d'autre ; `None` sinon.
    fn lire(texte: &str) -> Option<FuseauXsd> {
        if texte == "Z" {
            return Some(FuseauXsd::Z);
        }
        let b = texte.as_bytes();
        if b.len() != 6 || !matches!(b[0], b'+' | b'-') || b[3] != b':' {
            return None;
        }
        let (heures, minutes) = (deux_chiffres(&b[1..3])?, deux_chiffres(&b[4..6])?);
        (heures < 14 && minutes < 60 || heures == 14 && minutes == 0).then_some(FuseauXsd::Decalage { moins: b[0] == b'-', heures, minutes })
    }

    /// Son decalage sur UTC, en secondes (`Z` : zero).
    pub fn secondes(self) -> i64 {
        match self {
            FuseauXsd::Z => 0,
            FuseauXsd::Decalage { moins, heures, minutes } => (if moins { -1 } else { 1 }) * (i64::from(heures) * 3_600 + i64::from(minutes) * 60),
        }
    }
}

impl std::fmt::Display for FuseauXsd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FuseauXsd::Z => f.write_str("Z"),
            FuseauXsd::Decalage { moins, heures, minutes } => write!(f, "{}{:02}:{:02}", if *moins { '-' } else { '+' }, heures, minutes),
        }
    }
}

/// UNE DATE DE XML SCHEMA (`date`, XML Schema Part 2, § 3.2.9) : `AAAA-MM-JJ`, suivie ou non d'un fuseau (`FuseauXsd`) ;
/// la date de reception d'un bilan (`date_reception`, schema des bilans harmonises, l. 318-322 ;
/// `atelier/reference/sources/gip-mds-api-dsn/net-entreprises-dsn-bilans-v02r01-20230703.xsd`). Une annee hors de 1 a
/// 9999 (un signe moins, plus de quatre chiffres), que le schema permet, sort du calendrier du moteur : elle se refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateXsd {
    pub jour: Jour,
    pub fuseau: Option<FuseauXsd>,
}

impl DateXsd {
    /// `AAAA-MM-JJ` et son fuseau s'il en a un, et rien d'autre ; `None` sinon.
    pub(crate) fn lire(texte: &str) -> Option<DateXsd> {
        let (j, f) = (texte.get(..10)?, texte.get(10..)?);
        let jour = crate::date::jour_iso(j).filter(|x| x.iso() == j)?;
        let fuseau = if f.is_empty() { None } else { Some(FuseauXsd::lire(f)?) };
        Some(DateXsd { jour, fuseau })
    }
}

impl std::fmt::Display for DateXsd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.jour.iso())?;
        match self.fuseau {
            Some(z) => z.fmt(f),
            None => Ok(()),
        }
    }
}

/// UNE HEURE DE XML SCHEMA (`time`, XML Schema Part 2, § 3.2.8) : `hh:mm:ss`, une fraction de seconde ou non (`.` et des
/// chiffres), un fuseau ou non (`FuseauXsd`) ; `24:00:00` est la fin du jour (XML Schema 1.1, § 3.3.8). L'heure de
/// reception d'un bilan (`heure_reception`, schema des bilans harmonises, l. 323-327 ; meme fichier que `DateXsd`). Elle
/// garde ce qui est ecrit : la fraction telle quelle (ses chiffres), le fuseau tel quel.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeureXsd {
    /// Les secondes depuis minuit, de 0 a 86 400 (`24:00:00`).
    pub seconde: i64,
    /// Les chiffres apres le point, tels qu'ecrits.
    pub fraction: Option<String>,
    pub fuseau: Option<FuseauXsd>,
}

impl HeureXsd {
    /// `hh:mm:ss[.f…][fuseau]`, et rien d'autre ; `None` sinon.
    pub(crate) fn lire(texte: &str) -> Option<HeureXsd> {
        let b = texte.as_bytes();
        if b.len() < 8 || b[2] != b':' || b[5] != b':' {
            return None;
        }
        let (h, m, s) = (deux_chiffres(&b[..2])?, deux_chiffres(&b[3..5])?, deux_chiffres(&b[6..8])?);
        let reste = &texte[8..];
        let (fraction, reste) = match reste.strip_prefix('.') {
            Some(r) => {
                let n = r.bytes().take_while(u8::is_ascii_digit).count();
                if n == 0 {
                    return None;
                }
                (Some(r[..n].to_string()), &r[n..])
            }
            None => (None, reste),
        };
        let fuseau = if reste.is_empty() { None } else { Some(FuseauXsd::lire(reste)?) };
        let fin_du_jour = h == 24 && m == 0 && s == 0 && fraction.as_deref().is_none_or(|f| f.bytes().all(|c| c == b'0'));
        if !(h < 24 && m < 60 && s < 60 || fin_du_jour) {
            return None;
        }
        Some(HeureXsd { seconde: i64::from(h) * 3_600 + i64::from(m) * 60 + i64::from(s), fraction, fuseau })
    }

    /// La fraction est-elle nulle (absente, ou rien que des zeros) : l'heure tombe-t-elle sur sa seconde ?
    pub fn a_la_seconde(&self) -> bool {
        self.fraction.as_deref().is_none_or(|f| f.bytes().all(|c| c == b'0'))
    }

    /// « 09:36 » : l'heure et la minute telles qu'ecrites.
    pub fn hh_mm(&self) -> String {
        format!("{:02}:{:02}", self.seconde / 3_600, (self.seconde % 3_600) / 60)
    }
}

impl std::fmt::Display for HeureXsd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{:02}", self.hh_mm(), self.seconde % 60)?;
        if let Some(x) = &self.fraction {
            write!(f, ".{}", x)?;
        }
        match self.fuseau {
            Some(z) => z.fmt(f),
            None => Ok(()),
        }
    }
}

/// Deux chiffres ASCII, leur valeur ; `None` sinon.
fn deux_chiffres(b: &[u8]) -> Option<u8> {
    (b.len() == 2 && b.iter().all(u8::is_ascii_digit)).then(|| (b[0] - b'0') * 10 + (b[1] - b'0'))
}

/// Le jour d'un jour unix qu'un `Moment` ou un `Naif` porte : il est dans le calendrier, par leur construction.
fn dans_le_calendrier(jours: i64) -> Jour {
    Jour::depuis_jours_unix(jours).expect("un instant est dans le calendrier")
}

/// Le jour unix (nombre de jours depuis le 01/01/1970) du dernier dimanche du
/// mois `mois` de l'annee `an`.
fn dernier_dimanche(an: i32, mois: u32) -> i64 {
    let dernier = Jour::nouveau(an, mois, crate::date::jours_du_mois(an, mois)).expect("fin de mois");
    let n = dernier.ordinal() - 719_163;
    // Le 01/01/1970 etait un jeudi : (n + 4) % 7 vaut 0 pour un dimanche.
    n - (n + 4).rem_euclid(7)
}

/// Le decalage de Paris sur UTC, en secondes, a `t` secondes unix ; `None` si son jour UTC n'est pas du calendrier.
fn decalage_brut(t: i64) -> Option<i64> {
    let an = Jour::depuis_jours_unix(t.div_euclid(86_400))?.an;
    let debut = dernier_dimanche(an, 3) * 86_400 + 3_600;
    let fin = dernier_dimanche(an, 10) * 86_400 + 3_600;
    Some(if t >= debut && t < fin { 7_200 } else { 3_600 })
}

/// Le decalage de Paris sur UTC, en secondes, a cet instant (arrondi a la seconde d'avant).
pub fn decalage(t: Moment) -> i64 {
    let s = t.entieres();
    match decalage_brut(s) {
        Some(d) => d,
        None => unreachable!("un instant est dans le calendrier : {} s", s),
    }
}

/// L'heure de Paris d'un instant, `AAAA-MM-JJTHH:MM:SS`.
pub fn iso_secondes(t: Moment) -> String {
    let local = local_arrondi(t);
    let j = dans_le_calendrier(local.div_euclid(86_400));
    let reste = local.rem_euclid(86_400);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", j.an, j.mois, j.jour, reste / 3_600, (reste % 3_600) / 60, reste % 60)
}

/// Les secondes locales de Paris d'un instant, apres l'arrondi a la microseconde (au pair le plus proche) ; elles restent
/// dans le calendrier (`DERNIER` en garde la marge).
fn local_arrondi(t: Moment) -> i64 {
    let utc = t.a_la_microseconde().entieres();
    match decalage_brut(utc) {
        Some(d) => utc + d,
        None => unreachable!("un instant est dans le calendrier : {} s", utc),
    }
}

/// LA DATE DE MODIFICATION D'UN FICHIER, une donnee du dehors : les secondes et la fraction en nanosecondes, lues
/// contre le calendrier ; `None` : le systeme date le fichier hors du calendrier. Son lecteur dit ce que l'absence veut
/// dire pour lui.
/// Hors Unix (Windows), la meme valeur par `modified()`.
#[cfg(unix)]
pub fn mtime(m: &std::fs::Metadata) -> Option<Moment> {
    use std::os::unix::fs::MetadataExt;
    Moment::de_nanos(m.mtime() as i128 * NANOS + m.mtime_nsec() as i128)
}

#[cfg(not(unix))]
pub fn mtime(m: &std::fs::Metadata) -> Option<Moment> {
    // Windows date tout fichier (`ftLastWriteTime`) ; une date d'avant 1970 compte en secondes negatives, comme `mtime` de
    // Unix.
    let date = match m.modified() {
        Ok(t) => t,
        Err(e) => unreachable!("Windows date tout fichier : {}", e),
    };
    match date.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => Moment::de_duree(d, false),
        Err(avant_1970) => Moment::de_duree(avant_1970.duration(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(t: i64) -> Moment {
        Moment::de_secondes(t).unwrap()
    }

    /// Directive 2000/84/CE, art. 2 : « la période de l'heure d'été commence […] à 1 heure du matin, temps universel, le
    /// dernier dimanche de mars » (`atelier/reference/sources/union-europeenne/eurlex-directive-2000-84-ce-heure-d-ete-20010202.txt`).
    #[test]
    fn l_heure_d_ete_commence_le_dernier_dimanche_de_mars_a_1_h_utc() {
        // 2026 : dimanche 29 mars. 00:59:59 UTC encore l'hiver, 01:00:00 l'ete.
        let t = Jour::nouveau(2026, 3, 29).unwrap().ordinal() - 719_163;
        assert_eq!(decalage(a(t * 86_400 + 3_599)), 3_600);
        assert_eq!(decalage(a(t * 86_400 + 3_600)), 7_200);
        assert_eq!(MinuteEcrite::de(a(t * 86_400 + 3_599)).iso(), "2026-03-29T01:59");
        assert_eq!(MinuteEcrite::de(a(t * 86_400 + 3_600)).iso(), "2026-03-29T03:00");
    }

    /// Directive 2000/84/CE, art. 3 : la période « se termine […] à 1 heure du matin, temps universel, le dernier dimanche
    /// d'octobre » (même fichier).
    #[test]
    fn l_heure_d_ete_finit_le_dernier_dimanche_d_octobre_a_1_h_utc() {
        // 2026 : dimanche 25 octobre.
        let t = Jour::nouveau(2026, 10, 25).unwrap().ordinal() - 719_163;
        assert_eq!(decalage(a(t * 86_400 + 3_599)), 7_200);
        assert_eq!(decalage(a(t * 86_400 + 3_600)), 3_600);
        assert_eq!(MinuteEcrite::de(a(t * 86_400 + 3_599)).iso(), "2026-10-25T02:59");
        assert_eq!(MinuteEcrite::de(a(t * 86_400 + 3_600)).iso(), "2026-10-25T02:00");
    }

}

/// L'heure de Paris a cet instant (`socle::temps`, donc le temps simule quand il est pose) : `AAAA-MM-JJTHH:MM:SS`,
/// l'heure d'une ligne de journal.
pub fn maintenant() -> String {
    iso_secondes(crate::temps::secondes())
}

/// L'heure de Paris a cet instant, en secondes d'une epoque « naive » : de quoi additionner des heures et comparer deux
/// heures locales.
pub fn maintenant_naif() -> Naif {
    crate::temps::secondes().local_naif()
}
