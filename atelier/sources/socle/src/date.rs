//! Le jour civil : pas de `chrono`, pas de fuseau, pas d'horloge. Un jour est un triplet (an, mois, jour) du calendrier
//! gregorien proleptique, des annees 1 a 9999.
//!
//! `lire` admet cinq formats, essayes dans un ordre fixe : `AAAA-MM-JJ`, `JJ/MM/AAAA`, `JJMMAAAA`, `AAAAMMJJ`,
//! `JJ-MM-AAAA`. Chaque champ admet sa forme courte (`2026-3-5`) ; le premier format qui lit la chaine entiere gagne,
//! et un format entame ne revient pas en arriere (`20260315` n'est pas lu en `JJMMAAAA`). Ce parcours est celui des
//! fichiers deja ecrits : une date lue aujourd'hui se lit comme elle s'est toujours lue.

use crate::echec::{refus, R};
use crate::fraction::{Fraction, Moitie};
use crate::num::{self, Decimal};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Jour {
    pub(crate) an: i32,
    pub(crate) mois: u32,
    pub(crate) jour: u32,
}

/// UNE DUREE DE DROIT (« Le glossaire ») : des jours (`Duree<EnJours>`) ou des mois de calendrier (`Duree<EnMois>`), de
/// date a date ; jamais negative ; son unite est dans son type. Une duree qu'un texte fixe s'ecrit `Duree::jours`,
/// `Duree::mois`, `Duree::annees` (douze mois) ; un fichier la dit en nombre, lu en elle a sa porte (`en_jours`,
/// `en_mois` et leurs facultatives). Elle se compte depuis un jour (`Jour::apres`, `Jour::avant`), un mois (`Mois::apres`,
/// `Mois::avant`) ou une annee (`Annee::les_precedentes`), en mois ; elle se mesure du premier au dernier jour d'une
/// suite de jours (`Duree::du_au`), ou des jours ecoules entre deux jours (`Duree::ecoulee`) ; deux durees de la meme unite se comparent et s'ajoutent ; elle se dit dans une
/// phrase (`Display` : « 3 jours », « 2 mois »).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Duree<U>(u32, std::marker::PhantomData<U>);

/// L'unite d'une duree de droit en jours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnJours;

/// L'unite d'une duree de droit en mois de calendrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnMois;

/// Une unite de duree : `EnJours` ou `EnMois`, et aucune autre.
pub trait Unite: unite::Scellee {}
impl Unite for EnJours {}
impl Unite for EnMois {}

mod unite {
    use super::{Jour, R};

    /// Le jeton de ses methodes : il ne se fabrique qu'ici (`JETON`), et elles ne s'appellent pas hors de ce module, meme
    /// par un generique qui prend une `Unite`.
    pub struct Jeton(());

    pub(super) const JETON: Jeton = Jeton(());

    /// Ce que fait une unite, que seul ce module appelle : se compter depuis un jour, se dire.
    pub trait Scellee: Copy {
        /// Le jour `n` unites plus loin (`n` negatif : plus tot).
        fn decaler(j: &Jour, n: i64, jeton: Jeton) -> R<Jour>;
        fn dire(n: u32, f: &mut std::fmt::Formatter<'_>, jeton: Jeton) -> std::fmt::Result;
    }

    impl Scellee for super::EnJours {
        fn decaler(j: &Jour, n: i64, _: Jeton) -> R<Jour> {
            j.plus_jours(n)
        }
        fn dire(n: u32, f: &mut std::fmt::Formatter<'_>, _: Jeton) -> std::fmt::Result {
            if n <= 1 {
                write!(f, "{} jour", n)
            } else {
                write!(f, "{} jours", n)
            }
        }
    }

    impl Scellee for super::EnMois {
        fn decaler(j: &Jour, n: i64, _: Jeton) -> R<Jour> {
            j.plus_mois(n)
        }
        fn dire(n: u32, f: &mut std::fmt::Formatter<'_>, _: Jeton) -> std::fmt::Result {
            write!(f, "{} mois", n)
        }
    }
}

impl<U: Unite> Duree<U> {
    const fn de(n: u32) -> Duree<U> {
        Duree(n, std::marker::PhantomData)
    }

    /// ELLE ET `autre` BOUT A BOUT.
    pub fn plus(self, autre: Duree<U>) -> Duree<U> {
        Duree::de(self.0 + autre.0)
    }

    /// SA MOITIE, arrondie a l'unite du dessous (C. trav. L1225-54 : un conge « pris en compte pour moitié »).
    pub fn moitie(self) -> Duree<U> {
        Duree::de(self.0 / 2)
    }

    /// `n` fois elle, bout a bout (`n` : un compte).
    pub fn fois(self, n: u32) -> Duree<U> {
        Duree::de(self.0 * n)
    }

    /// Est-elle nulle ?
    pub fn nulle(self) -> bool {
        self.0 == 0
    }
}

impl Duree<EnJours> {
    /// `n` jours, qu'un texte fixe.
    pub const fn jours(n: u32) -> Duree<EnJours> {
        Duree::de(n)
    }

    /// LES JOURS DU `debut` AU `fin`, l'un et l'autre compris ; aucun quand `fin` precede `debut`.
    pub fn du_au(debut: &Jour, fin: &Jour) -> Duree<EnJours> {
        Duree::de_l_ecart(fin.moins(debut) + 1)
    }

    /// LES JOURS ECOULES DE `depuis` A `au` : du lendemain de `depuis` a `au`, compris ; aucun quand `au` ne suit pas
    /// `depuis` (« il reste 3 jours », l'age d'une source).
    pub fn ecoulee(depuis: &Jour, au: &Jour) -> Duree<EnJours> {
        Duree::de_l_ecart(au.moins(depuis))
    }

    /// Un ecart de jours ; negatif, aucun. De 0001-01-01 a 9999-12-31, il tient sur 32 bits.
    fn de_l_ecart(n: i64) -> Duree<EnJours> {
        Duree::de(if n > 0 { n as u32 } else { 0 })
    }

    /// AUTANT DE JOURS DE 24 HEURES, une duree mesuree : le decalage de la montre au jour pose (`temps`).
    pub(crate) fn de_24_heures(self) -> std::time::Duration {
        std::time::Duration::from_secs(u64::from(self.0) * 86_400)
    }

    /// UN JOUR PAR SEMAINE COMMENCEE (C. trav. L. 1242-10 : « un jour par semaine »).
    pub fn un_jour_par_semaine_commencee(self) -> Duree<EnJours> {
        Duree::de(self.0.div_ceil(7))
    }

    /// DITE EN ABREGE, « 3 j » : la case d'un tableau qui la montre (les jours avant le reexamen des droits).
    pub fn dite_en_abrege(self) -> String {
        format!("{} j", self.0)
    }

    /// SON NOMBRE DE JOURS, A L'ECRITURE D'UNE NORME qui le dit en nombre (la DSN : S21.G00.86.003).
    pub fn jours_a_ecrire(self) -> u32 {
        self.0
    }

    /// Lue d'un nombre entier de jours : `#[serde(deserialize_with = "socle::date::Duree::en_jours")]` ; dans la charge
    /// d'une route (`forme::lire_saisie`), aussi le nombre qu'une personne a tape (`grandeurs::lu::entier`, « 30 »).
    pub fn en_jours<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Duree<EnJours>, D::Error> {
        let n = crate::grandeurs::lu::entier(d, "un nombre entier de jours")?;
        u32::try_from(n).map(Duree::jours).map_err(serde::de::Error::custom)
    }

    /// `en_jours`, facultatif : `#[serde(default, deserialize_with = "socle::date::Duree::en_jours_opt")]`.
    pub fn en_jours_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Duree<EnJours>>, D::Error> {
        /// Une duree lue par `en_jours`, la ou un champ ne se pose pas (un `Option`).
        struct Lue(Duree<EnJours>);
        impl<'de> serde::Deserialize<'de> for Lue {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Lue, D::Error> {
                Duree::en_jours(d).map(Lue)
            }
        }
        <Option<Lue> as serde::Deserialize>::deserialize(d).map(|l| l.map(|l| l.0))
    }

    /// Ecrite en son nombre de jours (un champ `…jours` d'un fichier) : `#[serde(serialize_with =
    /// "socle::date::Duree::ecrite_en_jours")]`.
    pub fn ecrite_en_jours<S: serde::Serializer>(d: &Duree<EnJours>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(d.0)
    }

    /// `ecrite_en_jours`, facultative (`null` quand elle n'est pas dite) : `#[serde(serialize_with =
    /// "socle::date::Duree::ecrite_en_jours_opt")]`.
    pub fn ecrite_en_jours_opt<S: serde::Serializer>(d: &Option<Duree<EnJours>>, s: S) -> Result<S::Ok, S::Error> {
        match d {
            None => s.serialize_none(),
            Some(d) => s.serialize_some(&d.0),
        }
    }
}

impl Duree<EnMois> {
    /// `n` mois de calendrier, qu'un texte fixe.
    pub const fn mois(n: u32) -> Duree<EnMois> {
        Duree::de(n)
    }

    /// `n` annees, qu'un texte fixe : douze mois de calendrier chacune.
    pub const fn annees(n: u32) -> Duree<EnMois> {
        Duree::de(n * 12)
    }

    /// LES JOURS CALENDAIRES QU'ELLE COMPTE A PARTIR DE `debut` : du `debut` au meme quantieme ses mois plus tard
    /// (`Jour::apres`), ce jour exclu.
    #[track_caller]
    pub fn en_jours_depuis(self, debut: &Jour) -> R<Duree<EnJours>> {
        Ok(Duree::du_au(debut, &debut.apres(self)?.plus_jours(-1)?))
    }

    /// Ses mois, comptes depuis un mois (`Mois::apres`).
    pub(crate) fn nombre_de_mois(self) -> i64 {
        i64::from(self.0)
    }

    /// Lue d'un nombre entier de mois : `#[serde(deserialize_with = "socle::date::Duree::en_mois")]`.
    pub fn en_mois<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Duree<EnMois>, D::Error> {
        <u32 as serde::Deserialize>::deserialize(d).map(Duree::mois)
    }

    /// `en_mois`, facultatif : `#[serde(default, deserialize_with = "socle::date::Duree::en_mois_opt")]`.
    pub fn en_mois_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Duree<EnMois>>, D::Error> {
        <Option<u32> as serde::Deserialize>::deserialize(d).map(|n| n.map(Duree::mois))
    }

    /// Ecrite en son nombre de mois, facultative (un champ `…mois` d'un fichier) : `#[serde(serialize_with =
    /// "socle::date::Duree::ecrite_en_mois_opt")]`.
    pub fn ecrite_en_mois_opt<S: serde::Serializer>(d: &Option<Duree<EnMois>>, s: S) -> Result<S::Ok, S::Error> {
        match d {
            None => s.serialize_none(),
            Some(d) => s.serialize_some(&d.0),
        }
    }
}

/// « 1 jour », « 3 jours », « 2 mois ».
impl<U: Unite> std::fmt::Display for Duree<U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        U::dire(self.0, f, unite::JETON)
    }
}

// LES FERIES QU'UN CALENDRIER SAUTE se donnent une annee a la fois (`feries`) : les onze fetes legales
// (`feries::feries`), ou ceux que la compagnie chome (`|a| feries::chomes(a, dits)`, sa fiche) ; chaque regle garde les
// siens.

/// UN JOUR DE LA SEMAINE (« Le glossaire »), du lundi au dimanche, dans cet ordre : celui d'un jour
/// (`Jour::jour_de_la_semaine`), ou une case d'une fiche (les jours travailles d'un permanent). Il s'ecrit et se dit par
/// son nom (« lundi »), la seule liste des noms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JourDeLaSemaine {
    Lundi,
    Mardi,
    Mercredi,
    Jeudi,
    Vendredi,
    Samedi,
    Dimanche,
}

impl JourDeLaSemaine {
    /// Tous, du lundi au dimanche.
    pub const TOUS: [JourDeLaSemaine; 7] = [
        JourDeLaSemaine::Lundi,
        JourDeLaSemaine::Mardi,
        JourDeLaSemaine::Mercredi,
        JourDeLaSemaine::Jeudi,
        JourDeLaSemaine::Vendredi,
        JourDeLaSemaine::Samedi,
        JourDeLaSemaine::Dimanche,
    ];

    /// Leurs noms, du lundi au dimanche, tels qu'ils s'ecrivent et se disent.
    pub const NOMS: [&'static str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

    /// Son nom (« lundi »).
    pub fn nom(self) -> &'static str {
        JourDeLaSemaine::NOMS[self as usize]
    }

    /// Le jour de rang `rang` depuis le lundi (0), modulo la semaine.
    fn de_rang(rang: i64) -> JourDeLaSemaine {
        JourDeLaSemaine::TOUS[rang.rem_euclid(7) as usize]
    }
}

impl std::fmt::Display for JourDeLaSemaine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.nom())
    }
}

impl serde::Serialize for JourDeLaSemaine {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nom())
    }
}

impl<'de> serde::Deserialize<'de> for JourDeLaSemaine {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<JourDeLaSemaine, D::Error> {
        /// Son nom, et rien d'autre ; un autre mot se refuse en disant les noms.
        struct ParSonNom;
        impl serde::de::Visitor<'_> for ParSonNom {
            type Value = JourDeLaSemaine;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("variant identifier")
            }
            fn visit_str<E: serde::de::Error>(self, mot: &str) -> Result<JourDeLaSemaine, E> {
                match JourDeLaSemaine::TOUS.into_iter().find(|j| j.nom() == mot) {
                    Some(j) => Ok(j),
                    None => Err(E::unknown_variant(mot, &JourDeLaSemaine::NOMS)),
                }
            }
        }
        d.deserialize_str(ParSonNom)
    }
}

/// LA SEMAINE D'UN CALENDRIER (« Le glossaire ») : du lundi au vendredi (les jours ouvres), ou du lundi au samedi (les
/// jours ouvrables).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Semaine {
    DuLundiAuVendredi,
    DuLundiAuSamedi,
}

impl Semaine {
    /// Le jour `j` de la semaine en est-il ?
    pub fn contient(self, j: JourDeLaSemaine) -> bool {
        match self {
            Semaine::DuLundiAuVendredi => j <= JourDeLaSemaine::Vendredi,
            Semaine::DuLundiAuSamedi => j <= JourDeLaSemaine::Samedi,
        }
    }
}

/// LE CALENDRIER D'UNE SEMAINE (`semaine`), moins les feries qu'on lui donne : pourquoi `jour` n'en est pas (« samedi »,
/// « dimanche », « Assomption, fête légale »), ou `None` s'il en est.
pub(crate) fn hors_de_la_semaine(jour: Jour, semaine: Semaine, feries: &impl Fn(crate::mois::Annee) -> R<crate::feries::Jours>) -> R<Option<String>> {
    let jds = jour.jour_de_la_semaine();
    if !semaine.contient(jds) {
        return Ok(Some(jds.nom().to_string()));
    }
    Ok(feries(jour.annee())?.into_iter().find(|(j, _)| *j == jour).map(|(_, nom)| format!("{}, fête légale", nom)))
}

/// Le `n`-ieme jour de `semaine` apres `depuis` (`plus_tard`) ou avant lui, `depuis` exclu.
fn compter(depuis: Jour, n: u32, plus_tard: bool, semaine: Semaine, feries: &impl Fn(crate::mois::Annee) -> R<crate::feries::Jours>) -> R<Jour> {
    let (mut jour, mut reste) = (depuis, n);
    while reste > 0 {
        jour = jour.plus_jours(if plus_tard { 1 } else { -1 })?;
        if hors_de_la_semaine(jour, semaine, feries)?.is_none() {
            reste -= 1;
        }
    }
    Ok(jour)
}

/// DES JOURS OUVRES : un compte de jours du lundi au vendredi, moins les feries qu'on donne au calendrier (CSS R. 133-14,
/// II ; BOI-IR-PAS-30-10-20, § 30). Ce n'est pas une `Duree` : il ne devient un jour que par le calendrier (`apres`),
/// qui sait quels jours comptent. Un texte le fixe (`de`), ou un fichier le dit en nombre, lu en lui a sa porte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoursOuvres(u32);

impl JoursOuvres {
    /// `n` jours ouvres, qu'un texte fixe.
    pub const fn de(n: u32) -> JoursOuvres {
        JoursOuvres(n)
    }

    /// Le jour ouvre qui tombe ce nombre de jours ouvres apres `depuis`, `depuis` exclu.
    pub fn apres(self, depuis: Jour, feries: impl Fn(crate::mois::Annee) -> R<crate::feries::Jours>) -> R<Jour> {
        compter(depuis, self.0, true, Semaine::DuLundiAuVendredi, &feries)
    }

    /// Le jour ouvre qui tombe ce nombre de jours ouvres avant `depuis`, `depuis` exclu.
    pub fn avant(self, depuis: Jour, feries: impl Fn(crate::mois::Annee) -> R<crate::feries::Jours>) -> R<Jour> {
        compter(depuis, self.0, false, Semaine::DuLundiAuVendredi, &feries)
    }

    /// LE PREMIER JOUR OUVRE A PARTIR DE `jour`, lui compris, et chaque jour saute avec ce qu'il est (« samedi »,
    /// « Assomption, fête légale »).
    pub fn premier_a_partir_de(jour: Jour, feries: impl Fn(crate::mois::Annee) -> R<crate::feries::Jours>) -> R<(Jour, Vec<(Jour, String)>)> {
        let (mut jour, mut sautes) = (jour, Vec::new());
        while let Some(pourquoi) = hors_de_la_semaine(jour, Semaine::DuLundiAuVendredi, &feries)? {
            sautes.push((jour, pourquoi));
            jour = jour.plus_jours(1)?;
        }
        Ok((jour, sautes))
    }
}

impl<'de> serde::Deserialize<'de> for JoursOuvres {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<JoursOuvres, D::Error> {
        <u32 as serde::Deserialize>::deserialize(d).map(JoursOuvres)
    }
}

/// DES JOURS OUVRABLES : un compte de jours du lundi au samedi, moins les feries qu'on donne au calendrier (C. trav.
/// L. 1242-13 ; CCN 1285, titre XV). Ce n'est pas une `Duree` : il ne devient un jour que par le calendrier (`apres`,
/// `avant`). Un texte le fixe (`de`), ou un fichier le dit en nombre, lu en lui a sa porte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoursOuvrables(u32);

impl JoursOuvrables {
    /// `n` jours ouvrables, qu'un texte fixe.
    pub const fn de(n: u32) -> JoursOuvrables {
        JoursOuvrables(n)
    }

    /// Le jour ouvrable qui tombe ce nombre de jours ouvrables apres `depuis`, `depuis` exclu.
    pub fn apres(self, depuis: Jour, feries: impl Fn(crate::mois::Annee) -> R<crate::feries::Jours>) -> R<Jour> {
        compter(depuis, self.0, true, Semaine::DuLundiAuSamedi, &feries)
    }

    /// Le jour ouvrable qui tombe ce nombre de jours ouvrables avant `depuis`, `depuis` exclu (« les quinze jours
    /// ouvrables précédant »).
    pub fn avant(self, depuis: Jour, feries: impl Fn(crate::mois::Annee) -> R<crate::feries::Jours>) -> R<Jour> {
        compter(depuis, self.0, false, Semaine::DuLundiAuSamedi, &feries)
    }
}

impl<'de> serde::Deserialize<'de> for JoursOuvrables {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<JoursOuvrables, D::Error> {
        <u32 as serde::Deserialize>::deserialize(d).map(JoursOuvrables)
    }
}

/// UNE PERIODE (« Le glossaire ») : deux jours bornes, l'un et l'autre compris, le premier pas apres le second (la
/// periode d'un contrat, d'une paie, d'un arret, d'une fenetre de calcul, d'un certificat, d'un export). Elle ne se
/// construit que par `du_au`, qui refuse un debut apres la fin ; ses bornes, ses jours et sa duree se lisent d'elle.
/// Elle s'ecrit et se lit `{"du": "AAAA-MM-JJ", "au": "AAAA-MM-JJ"}` ; elle se dit dans une phrase (`Display` : « du
/// 1er au 15 octobre 2026 »).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Periode {
    debut: Jour,
    fin: Jour,
}

impl Periode {
    /// DU `debut` AU `fin` ; refus quand `debut` est apres `fin`.
    #[track_caller]
    pub fn du_au(debut: Jour, fin: Jour) -> R<Periode> {
        match debut <= fin {
            true => Ok(Periode { debut, fin }),
            false => Err(refus(format!("période impossible : le {} est après le {}", debut.jj_mm_aaaa(), fin.jj_mm_aaaa()))),
        }
    }

    /// Le seul jour `j`.
    pub fn du_jour(j: Jour) -> Periode {
        Periode { debut: j, fin: j }
    }

    /// Le mois civil `m` entier.
    pub fn du_mois(m: crate::mois::Mois) -> Periode {
        Periode { debut: m.premier_jour(), fin: m.dernier_jour() }
    }

    /// Son premier jour.
    pub fn debut(&self) -> Jour {
        self.debut
    }

    /// Son dernier jour.
    pub fn fin(&self) -> Jour {
        self.fin
    }

    /// `j` en est-il ?
    pub fn contient(&self, j: &Jour) -> bool {
        self.debut <= *j && *j <= self.fin
    }

    /// Ses jours communs avec `autre`, s'il y en a.
    pub fn croise(&self, autre: &Periode) -> Option<Periode> {
        let debut = self.debut.max(autre.debut);
        let fin = self.fin.min(autre.fin);
        match debut <= fin {
            true => Some(Periode { debut, fin }),
            false => None,
        }
    }

    /// La plus courte periode qui la contient AVEC `autre` : du plus tot de leurs debuts au plus tard de leurs fins.
    pub fn avec(&self, autre: &Periode) -> Periode {
        Periode { debut: self.debut.min(autre.debut), fin: self.fin.max(autre.fin) }
    }

    /// Les mois civils qu'elle touche, du premier au dernier.
    pub fn mois(&self) -> Vec<crate::mois::Mois> {
        crate::mois::Mois::du_jour(&self.debut).jusqu_a(crate::mois::Mois::du_jour(&self.fin))
    }

    /// Le nombre de ses jours, l'un et l'autre bouts compris.
    pub fn duree(&self) -> Duree<EnJours> {
        Duree::du_au(&self.debut, &self.fin)
    }

    /// L'anciennete qu'elle donne, de son premier jour a son dernier (`Anciennete::entre`) : celle d'un contrat, au
    /// preavis et aux indemnites de fin de contrat.
    pub fn anciennete(&self) -> Anciennete {
        Anciennete::entre(&self.debut, &self.fin)
    }

    /// Ses jours, du premier au dernier.
    pub fn jours(&self) -> impl Iterator<Item = Jour> {
        let (premier, dernier) = (self.debut.ordinal(), self.fin.ordinal());
        // Chaque rang entre deux jours du calendrier est un jour du calendrier.
        (premier..=dernier).filter_map(Jour::depuis_ordinal)
    }
}

/// « du 1er au 15 octobre 2026 », « du 28 septembre au 3 octobre 2026 », « du 28 décembre 2025 au 3 janvier 2026 »,
/// « le 3 octobre 2026 ».
impl std::fmt::Display for Periode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let quantieme = |j: &Jour| if j.jour == 1 { "1er".to_string() } else { j.jour.to_string() };
        let mois = |j: &Jour| j.mois_civil().nom(crate::mois::Graphie::Accentuee);
        let (d, a) = (&self.debut, &self.fin);
        if d == a {
            write!(f, "le {} {} {}", quantieme(d), mois(d), d.an)
        } else if d.an != a.an {
            write!(f, "du {} {} {} au {} {} {}", quantieme(d), mois(d), d.an, quantieme(a), mois(a), a.an)
        } else if d.mois != a.mois {
            write!(f, "du {} {} au {} {} {}", quantieme(d), mois(d), quantieme(a), mois(a), a.an)
        } else {
            write!(f, "du {} au {} {} {}", quantieme(d), quantieme(a), mois(a), a.an)
        }
    }
}

/// La forme ecrite d'une periode.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PeriodeEcrite {
    du: Jour,
    au: Jour,
}

impl serde::Serialize for Periode {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        PeriodeEcrite { du: self.debut, au: self.fin }.serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for Periode {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Periode, D::Error> {
        let e = PeriodeEcrite::deserialize(d)?;
        match e.du <= e.au {
            true => Ok(Periode { debut: e.du, fin: e.au }),
            false => Err(serde::de::Error::custom(format!("période impossible : le {} est après le {}", e.du.iso(), e.au.iso()))),
        }
    }
}

/// UNE PERIODE OUVERTE (« Le glossaire ») : un premier jour, et un dernier jour quand il est dit (le contrat qui a
/// commence et dont la fin n'est pas dite : un CDI declare en DSN, S21.G00.40.010 absente) ; la fin, quand elle est
/// dite, n'est pas avant le debut. Elle ne se construit que par `du`, qui refuse une fin avant le debut. Elle s'ecrit et
/// se lit `{"du": "AAAA-MM-JJ", "au": "AAAA-MM-JJ" | null}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeriodeOuverte {
    debut: Jour,
    fin: Option<Jour>,
}

impl PeriodeOuverte {
    /// DU `debut`, AU `fin` quand elle est dite ; refus quand `fin` est avant `debut`.
    #[track_caller]
    pub fn du(debut: Jour, fin: Option<Jour>) -> R<PeriodeOuverte> {
        match fin {
            Some(f) if f < debut => Err(refus(format!("période impossible : le {} est après le {}", debut.jj_mm_aaaa(), f.jj_mm_aaaa()))),
            _ => Ok(PeriodeOuverte { debut, fin }),
        }
    }

    /// Son premier jour.
    pub fn debut(&self) -> Jour {
        self.debut
    }

    /// Son dernier jour, quand il est dit.
    pub fn fin(&self) -> Option<Jour> {
        self.fin
    }

    /// La periode qu'elle est quand sa fin est dite.
    pub fn fermee(&self) -> Option<Periode> {
        self.fin.map(|fin| Periode { debut: self.debut, fin })
    }

    /// La periode qu'elle est, FERMEE AU `jour` quand sa fin n'est pas dite ; refus quand `jour` est avant son debut.
    #[track_caller]
    pub fn fermee_au(&self, jour: Jour) -> R<Periode> {
        match self.fin {
            Some(fin) => Periode::du_au(self.debut, fin),
            None => Periode::du_au(self.debut, jour),
        }
    }

    /// Elle SANS SA FIN : le meme premier jour, aucun dernier dit.
    pub fn sans_fin(&self) -> PeriodeOuverte {
        PeriodeOuverte { debut: self.debut, fin: None }
    }

    /// `j` en est-il ? Sans fin dite, tout jour a partir du premier en est.
    pub fn contient(&self, j: &Jour) -> bool {
        self.debut <= *j && self.fin.is_none_or(|f| *j <= f)
    }

    /// Ses jours communs avec `autre`, s'il y en a.
    pub fn croise(&self, autre: &Periode) -> Option<Periode> {
        let debut = self.debut.max(autre.debut);
        let fin = match self.fin {
            Some(f) => f.min(autre.fin),
            None => autre.fin,
        };
        match debut <= fin {
            true => Some(Periode { debut, fin }),
            false => None,
        }
    }
}

/// Une periode est une periode ouverte dont la fin est dite.
impl From<Periode> for PeriodeOuverte {
    fn from(p: Periode) -> PeriodeOuverte {
        PeriodeOuverte { debut: p.debut, fin: Some(p.fin) }
    }
}

/// La forme ecrite d'une periode ouverte.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PeriodeOuverteEcrite {
    du: Jour,
    au: Option<Jour>,
}

impl serde::Serialize for PeriodeOuverte {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        PeriodeOuverteEcrite { du: self.debut, au: self.fin }.serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for PeriodeOuverte {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<PeriodeOuverte, D::Error> {
        let e = PeriodeOuverteEcrite::deserialize(d)?;
        match e.au {
            Some(au) if au < e.du => Err(serde::de::Error::custom(format!("période impossible : le {} est après le {}", e.du.iso(), au.iso()))),
            _ => Ok(PeriodeOuverte { debut: e.du, fin: e.au }),
        }
    }
}

/// UNE ANCIENNETE (« Le glossaire ») : le temps passe depuis un jour, en mois de calendrier ; un age est l'anciennete
/// depuis la naissance. Elle se compte ici, de deux jours, et nulle part ailleurs : en mois et fraction du mois entame
/// (`entre`), ou en mois revolus (`revolue`) et en annees revolues (`ans_revolus`, `age`). Un seuil qu'un texte dit en
/// mois ou en annees se lit en elle a sa porte (`Deserialize` : des mois ; `en_annees`, `en_mois_entiers`,
/// `en_annees_entieres`) et elle se compare a lui ; elle se dit dans l'unite que le texte demande (`annees_ecrites`,
/// `semaines_entieres`), et dans une phrase en mois (`mois_ecrits`, `mois_dits`) ou en annees revolues
/// (`ans_revolus_dits`).
/// Exacte : ses mois sont une fraction (`fraction::Fraction`), des jours sur la longueur de leur mois comme des annees de
/// douze mois, sans coupure ; elle ne devient un decimal qu'a ses ecritures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Anciennete(Fraction);

/// Douze mois, l'annee d'une anciennete.
fn douze() -> Fraction {
    Fraction::entier(12)
}

impl Anciennete {
    pub const ZERO: Anciennete = Anciennete(Fraction::ZERO);

    /// LES MOIS COMPLETS DE `depuis` A `au`, `au` compris, ET LA FRACTION DU MOIS ENTAME (ses jours sur ceux du mois de
    /// `au`, exacte) ; rien quand `au` precede `depuis`. Le compte d'un preavis, d'une indemnite de fin de contrat, d'un
    /// maintien de salaire, d'un ferie chome (le code du travail).
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
        Anciennete(Fraction::entier(i128::from(mois)) + Fraction::de(i128::from(reste), u128::from(jours_du_mois(au.an, au.mois))))
    }

    /// LES MOIS REVOLUS DE `depuis` AU `au` : un de plus a chaque quantieme de `depuis` atteint ; negatif quand `au`
    /// precede `depuis`. Le compte d'un texte qui dit des mois ou des ans revolus (une grille, la prime de presence,
    /// l'ouverture d'une prevoyance, un age).
    pub fn revolue(depuis: &Jour, au: &Jour) -> Anciennete {
        let mois = (au.an as i64 - depuis.an as i64) * 12 + (au.mois as i64 - depuis.mois as i64) - i64::from(au.jour < depuis.jour);
        Anciennete(Fraction::entier(i128::from(mois)))
    }

    /// LE PREMIER JOUR OU L'ANCIENNETE REVOLUE DEPUIS `depuis` LA VAUT (`revolue`) : le meme quantieme autant de mois plus
    /// tard, ou le premier du mois suivant quand ce quantieme n'existe pas (31 janvier + 1 mois : le 1er mars). Une fraction
    /// de mois n'est atteinte a aucun quantieme, ni un jour hors du calendrier : `None`.
    pub fn atteinte(self, depuis: &Jour) -> Option<Jour> {
        let mois = i64::try_from(self.0.en_entier()?).ok()?;
        let rang = depuis.an as i64 * 12 + (depuis.mois as i64 - 1) + mois;
        let (an, mois) = ((rang / 12) as i32, (rang % 12) as u32 + 1);
        Jour::nouveau(an, mois, depuis.jour).or_else(|| {
            let (an, mois) = if mois == 12 { (an + 1, 1) } else { (an, mois + 1) };
            Jour::nouveau(an, mois, 1)
        })
    }

    /// L'ANCIENNETE DITE EN JOURS, DE `depuis` AU `au`, l'un et l'autre compris ; aucun quand `au` precede `depuis` :
    /// celle que la DSN declare (S21.G00.86.003, « en jours »).
    pub fn en_jours(depuis: &Jour, au: &Jour) -> Duree<EnJours> {
        Duree::du_au(depuis, au)
    }

    /// CE QU'ELLE A DE PLUS QUE `seuil` ; rien quand elle ne l'atteint pas.
    pub fn au_dela_de(self, seuil: Anciennete) -> Anciennete {
        if self.0 > seuil.0 {
            Anciennete(self.0 - seuil.0)
        } else {
            Anciennete::ZERO
        }
    }

    /// COMBIEN DE FOIS ENTIERES `pas` Y TIENT (un compte : « dix jours par période entière de cinq ans ») ; refus d'un
    /// `pas` nul.
    #[track_caller]
    pub fn fois_entieres(self, pas: Anciennete) -> R<u32> {
        if pas.0 <= Fraction::ZERO {
            return Err(refus(format!("une ancienneté de {} mois ne se compte pas par périodes", pas.mois_ecrits())));
        }
        match (self.0 / pas.0).plancher().en_entier().and_then(|n| u32::try_from(n).ok()) {
            Some(n) => Ok(n),
            None => Err(refus(crate::forme::illisible(&self.mois_ecrits()))),
        }
    }

    /// L'AGE au `au` de qui est ne le `naissance` : ses annees revolues.
    pub fn age(naissance: &Jour, au: &Jour) -> Anciennete {
        Anciennete::revolue(naissance, au).ans_revolus()
    }

    /// `n` mois qu'un texte fixe (L3133-3 : « trois mois d'ancienneté »).
    pub fn mois_fixes(n: u32) -> Anciennete {
        Anciennete(Fraction::entier(i128::from(n)))
    }

    /// `n` annees qu'un texte fixe, douze mois chacune.
    pub fn ans(n: u32) -> Anciennete {
        Anciennete(Fraction::entier(i128::from(n) * 12))
    }

    /// `d` annees (un seuil d'un texte, un oracle) : la porte d'un nombre d'annees, douze mois chacune.
    fn d_annees(d: Decimal) -> Anciennete {
        Anciennete(Fraction::du_decimal(d) * douze())
    }

    /// SES ANNEES, a douze mois l'annee, exactes.
    fn annees(self) -> Fraction {
        self.0 / douze()
    }

    /// SES ANNEES REVOLUES, en anciennete : les mois des annees entieres.
    pub fn ans_revolus(self) -> Anciennete {
        Anciennete(self.annees().plancher() * douze())
    }

    /// SES ANNEES REVOLUES DITES DANS UNE PHRASE, en nombre entier (`3` ; `-1` pour une anciennete revolue negative,
    /// `revolue`) : les annees de presence d'une prime, le palier d'une indemnite, le seuil d'un minimum.
    pub fn ans_revolus_dits(self) -> String {
        self.annees().plancher().en_decimal().to_string()
    }

    /// Ecrite en nombre entier d'annees (`3`), comme `en_annees_entieres` la relit :
    /// `#[serde(serialize_with = "socle::date::Anciennete::ecrite_en_annees_entieres")]`. Une anciennete qui n'est pas
    /// d'annees entieres (`ans_revolus` la fait telle) ne s'ecrit pas ainsi.
    pub fn ecrite_en_annees_entieres<S: serde::Serializer>(a: &Anciennete, s: S) -> Result<S::Ok, S::Error> {
        match a.annees().en_entier().and_then(|n| i64::try_from(n).ok()) {
            Some(n) => s.serialize_i64(n),
            None => Err(serde::ser::Error::custom(format!("une ancienneté de {} mois n'est pas un nombre entier d'années", a.mois_ecrits()))),
        }
    }

    /// `ecrite_en_annees_entieres`, facultative : `None` s'ecrit `null`.
    pub fn ecrite_en_annees_entieres_opt<S: serde::Serializer>(a: &Option<Anciennete>, s: S) -> Result<S::Ok, S::Error> {
        match a {
            Some(a) => Anciennete::ecrite_en_annees_entieres(a, s),
            None => s.serialize_none(),
        }
    }

    /// Au demi-mois superieur, sans zero inutile (3,1 mois : 3,5) : l'anciennete qu'une phrase dit en mois (un affichage,
    /// pas une regle).
    pub fn au_demi_mois_superieur(self) -> Anciennete {
        let deux = Fraction::entier(2);
        Anciennete((self.0 * deux).plafond() / deux)
    }

    /// SES MOIS COMPLETS : le mois entame ne compte pas (R1234-1).
    pub fn mois_complets(self) -> Anciennete {
        Anciennete(self.0.plancher())
    }

    /// SES ANNEES DITES DANS UNE PHRASE, au dixieme, la moitie au pair, une decimale ecrite (`12.5`, `3.0`).
    pub fn annees_au_dixieme(self) -> String {
        match self.annees().arrondi(1, Moitie::AuPair) {
            Some(a) => a.to_string(),
            None => crate::fraction::deborde(),
        }
    }

    /// LES SEMAINES CALENDAIRES ENTIERES qu'elle compte, cinquante-deux par annee : un preavis dit en jours par semaine
    /// d'anciennete (`droit::clauses`). Exact ; son resultat est entier.
    pub fn semaines_entieres(self) -> crate::grandeurs::Semaines {
        crate::grandeurs::du_decimal((self.annees() * Fraction::entier(52)).plancher().en_decimal())
    }

    /// LES MOIS DE SALAIRE QU'UN BAREME PAR ANNEE D'ANCIENNETE LUI DONNE (R1234-2 : « un quart de mois de salaire par
    /// année d'ancienneté pour les années jusqu'à dix ans ») : ses annees au-dela de `bas` et jusqu'a `haut` (`None` :
    /// sans borne ; une tranche ; de `ZERO` sans borne, toutes ses annees : un bareme par bandes, un prorata, la mise a la
    /// retraite d'une convention), chacune `par_annee` mois de salaire ; rien quand elle n'atteint pas `bas`. Exacts : ses
    /// annees dans la tranche fois `par_annee` (`grandeurs::MoisDeSalaire`), sans coupure jusqu'au montant.
    pub fn par_annee_dans_la_tranche(self, bas: Anciennete, haut: Option<Anciennete>, par_annee: crate::grandeurs::Coefficient) -> crate::grandeurs::MoisDeSalaire {
        let haut = match haut {
            Some(h) => h.0,
            None => self.0,
        };
        let part = Anciennete((self.0.min(haut) - bas.0).max(Fraction::ZERO));
        crate::grandeurs::MoisDeSalaire::comptes(part.annees() * Fraction::du_decimal(crate::grandeurs::decimal(par_annee)))
    }

    /// Ses mois en decimal, comme `entre` les compte : ses mois entiers, plus la fraction du mois entame a vingt-huit
    /// chiffres (un decimal n'en porte pas plus), leur somme a vingt-huit chiffres.
    fn ecrite(self) -> Decimal {
        let entiers = self.0.plancher();
        num::add(entiers.en_decimal(), (self.0 - entiers).en_decimal())
    }

    /// SES MOIS ECRITS, le point decimal, a vingt-huit chiffres (`23.48387096774193548387096774`, `24`) : l'anciennete
    /// qu'une phrase d'une convention cite (« aucune tranche … ne couvre 23.48… mois »).
    pub fn mois_ecrits(self) -> String {
        self.ecrite().to_string()
    }

    /// SES ANNEES ECRITES, a douze mois l'annee, le point decimal, a vingt-huit chiffres (`2`, `0.5`) : le seuil d'une
    /// indemnite cite dans une phrase.
    pub fn annees_ecrites(self) -> String {
        self.annees().en_decimal().to_string()
    }

    /// Ecrite en mois, en texte, son echelle gardee (`mois_ecrits`), comme sa lecture la relit (`Deserialize`) :
    /// `#[serde(serialize_with = "socle::date::Anciennete::ecrite_en_mois")]`.
    pub fn ecrite_en_mois<S: serde::Serializer>(a: &Anciennete, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&a.mois_ecrits())
    }

    /// SES MOIS DITS DANS UNE PHRASE, tels qu'ils sont, la virgule decimale, sans zero inutile (« 6 », « 3,5 »).
    pub fn mois_dits(self) -> String {
        crate::texte::nombre_dit(self.ecrite())
    }

    /// SES MOIS DITS DANS UNE PHRASE AU DIXIEME, la moitie au pair (« 7,3 ») : l'anciennete qu'un maintien de salaire
    /// compare a son minimum.
    pub fn mois_dits_au_dixieme(self) -> String {
        match self.0.arrondi(1, Moitie::AuPair) {
            Some(m) => crate::texte::nombre_dit(m),
            None => crate::fraction::deborde(),
        }
    }

    /// Lue d'un nombre de mois, exactement tel qu'il est ecrit : sa porte.
    fn de_mois(d: Decimal) -> Anciennete {
        Anciennete(Fraction::du_decimal(d))
    }

    /// Lue d'un nombre de mois facultatif, `null` compris : `#[serde(deserialize_with = "socle::date::Anciennete::en_mois_opt")]`.
    pub fn en_mois_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Anciennete>, D::Error> {
        crate::forme::decimal_opt(d).map(|a| a.map(Anciennete::de_mois))
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
        <i64 as serde::Deserialize>::deserialize(d).map(Anciennete::ans_lus)
    }

    /// `n` annees entieres lues : douze mois chacune.
    fn ans_lus(n: i64) -> Anciennete {
        Anciennete(Fraction::entier(i128::from(n)) * douze())
    }

    /// `en_annees_entieres`, facultatif : `#[serde(default, deserialize_with = "socle::date::Anciennete::en_annees_entieres_opt")]`.
    pub fn en_annees_entieres_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Anciennete>, D::Error> {
        <Option<i64> as serde::Deserialize>::deserialize(d).map(|a| a.map(Anciennete::ans_lus))
    }

    /// Lue d'un nombre entier de mois.
    pub fn en_mois_entiers<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Anciennete, D::Error> {
        <i64 as serde::Deserialize>::deserialize(d).map(|m| Anciennete(Fraction::entier(i128::from(m))))
    }

    /// `en_mois_entiers`, facultatif : `#[serde(default, deserialize_with = "socle::date::Anciennete::en_mois_entiers_opt")]`.
    pub fn en_mois_entiers_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Anciennete>, D::Error> {
        <Option<i64> as serde::Deserialize>::deserialize(d).map(|m| m.map(|m| Anciennete(Fraction::entier(i128::from(m)))))
    }
}

/// Lue d'un nombre de mois, comme `forme::decimal`, exactement tel qu'il est ecrit.
impl<'de> serde::Deserialize<'de> for Anciennete {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Anciennete, D::Error> {
        crate::forme::decimal(d).map(Anciennete::de_mois)
    }
}

const JOURS_PAR_MOIS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Le rang du 1970-01-01 a compter du 0001-01-01, qui vaut 1.
const ORDINAL_EPOQUE_UNIX: i64 = 719_163;

pub(crate) fn bissextile(an: i32) -> bool {
    (an % 4 == 0 && an % 100 != 0) || an % 400 == 0
}

/// `calendar.monthrange(an, mois)[1]`, pour un mois de 1 a 12.
pub(crate) fn jours_du_mois(an: i32, mois: u32) -> u32 {
    if mois == 2 && bissextile(an) {
        29
    } else {
        JOURS_PAR_MOIS[(mois - 1) as usize]
    }
}

impl Jour {
    /// `None` si le jour n'existe pas ; l'annee va de 1 a 9999.
    pub(crate) fn nouveau(an: i32, mois: u32, jour: u32) -> Option<Jour> {
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

    /// `J/MM`, le quantieme sans zero devant, sans l'annee (« 5/03 » : la periode courte d'un bulletin fictif).
    pub fn j_mm(&self) -> String {
        format!("{}/{:02}", self.jour, self.mois)
    }

    /// `J/MM/AA`, le quantieme sans zero devant, l'annee en ses deux derniers chiffres (« 5/03/26 » : la periode courte
    /// d'un bulletin fictif).
    pub fn j_mm_aa(&self) -> String {
        format!("{}/{:02}/{}", self.jour, self.mois, self.annee().aa())
    }

    /// `JJMMAAAA`.
    pub fn jjmmaaaa(&self) -> String {
        format!("{:02}{:02}{:04}", self.jour, self.mois, self.an)
    }

    /// `AAAAMMJJ`.
    pub fn aaaammjj(&self) -> String {
        format!("{:04}{:02}{:02}", self.an, self.mois, self.jour)
    }

    /// `JJ/MM/AA`, l'annee en ses deux derniers chiffres (la date d'un certificat Congés Spectacles, d'un bulletin
    /// fictif).
    pub fn jj_mm_aa(&self) -> String {
        format!("{:02}/{:02}/{}", self.jour, self.mois, self.annee().aa())
    }

    /// `JJMMAA` (la date d'une ecriture d'un export comptable).
    pub fn jjmmaa(&self) -> String {
        format!("{:02}{:02}{}", self.jour, self.mois, self.annee().aa())
    }

    /// `JJ`, `MM` et `AAAA`, chacun dans sa case d'un formulaire (l'attestation employeur), ou les trois separees d'un
    /// blanc (`cases().join(" ")` : le certificat Congés Spectacles).
    pub fn cases(&self) -> [String; 3] {
        [format!("{:02}", self.jour), format!("{:02}", self.mois), format!("{:04}", self.an)]
    }

    /// Son quantieme dans son mois, ecrit sans zero devant (« 5 ») : le seul endroit ou le numero d'un jour dans son
    /// mois sort d'un jour, a une ecriture.
    pub fn quantieme_ecrit(&self) -> String {
        self.jour.to_string()
    }

    /// « mercredi 23 septembre 2026 ».
    pub fn en_clair(&self) -> String {
        format!("{} {} {} {}", self.jour_de_la_semaine(), self.jour, self.mois_civil().nom(crate::mois::Graphie::Accentuee), self.an)
    }

    /// Le premier jour de son mois ?
    pub fn premier_du_mois(&self) -> bool {
        self.jour == 1
    }

    /// A-t-il le meme quantieme que `autre` (le 31 d'un mois et le 31 d'un autre) ?
    pub fn meme_quantieme(&self, autre: &Jour) -> bool {
        self.jour == autre.jour
    }

    /// Le 0001-01-01 vaut 1. Formule des jours depuis
    /// l'epoque (annee commencant en mars, cycles de 400 ans).
    pub(crate) fn ordinal(&self) -> i64 {
        let a = i64::from(self.an) - i64::from(self.mois <= 2);
        let ere = a.div_euclid(400);
        let annee_d_ere = a - ere * 400;
        let m = i64::from(self.mois);
        let jour_d_annee = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.jour) - 1;
        let jour_d_ere = annee_d_ere * 365 + annee_d_ere / 4 - annee_d_ere / 100 + jour_d_annee;
        ere * 146_097 + jour_d_ere - 719_468 + ORDINAL_EPOQUE_UNIX
    }

    /// Le jour de rang `ordinal` (le 0001-01-01 vaut 1). `None` hors de 0001-01-01 .. 9999-12-31.
    pub(crate) fn depuis_ordinal(ordinal: i64) -> Option<Jour> {
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
    pub(crate) fn depuis_jours_unix(jours: i64) -> Option<Jour> {
        Jour::depuis_ordinal(jours + ORDINAL_EPOQUE_UNIX)
    }

    /// `date + timedelta(days=n)`, refuse hors des annees 1 a 9999 ; hors du socle, un jour se decale d'une duree
    /// (`apres`, `avant`).
    #[track_caller]
    pub(crate) fn plus_jours(&self, n: i64) -> R<Jour> {
        match Jour::depuis_ordinal(self.ordinal() + n) { Some(j) => Ok(j), None => Err(refus(HORS_CALENDRIER)) }
    }

    /// LE JOUR QUI SUIT DE `d` CELUI-CI : `d` jours plus tard, ou le meme quantieme `d` mois de calendrier plus tard (le
    /// dernier jour du mois s'il ne l'a pas) ; refus hors des annees 1 a 9999.
    #[track_caller]
    pub fn apres<U: Unite>(&self, d: Duree<U>) -> R<Jour> {
        U::decaler(self, i64::from(d.0), unite::JETON)
    }

    /// LE JOUR QUI PRECEDE DE `d` CELUI-CI : `d` jours plus tot, ou le meme quantieme `d` mois de calendrier plus tot (le
    /// dernier jour du mois s'il ne l'a pas) ; refus hors des annees 1 a 9999.
    #[track_caller]
    pub fn avant<U: Unite>(&self, d: Duree<U>) -> R<Jour> {
        U::decaler(self, -i64::from(d.0), unite::JETON)
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
    pub(crate) fn moins(&self, autre: &Jour) -> i64 {
        self.ordinal() - autre.ordinal()
    }

    pub fn jours_du_mois(&self) -> u32 {
        jours_du_mois(self.an, self.mois)
    }

    /// Son jour de la semaine.
    pub fn jour_de_la_semaine(&self) -> JourDeLaSemaine {
        JourDeLaSemaine::de_rang(self.ordinal() + 6)
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

pub(crate) fn jour_impossible(an: impl Into<i64>, mois: impl Into<i64>, jour: impl Into<i64>) -> String {
    format!("date impossible : {}", iso(an, mois, jour))
}

/// `AAAA-MM-JJ` de trois nombres tels qu'ils sont, la seule ecriture ISO d'une date : celle d'un `Jour`, d'une date
/// impossible, d'une date de naissance declaree dont le jour ou le mois sont inconnus (`00`, `99`).
pub(crate) fn iso(an: impl Into<i64>, mois: impl Into<i64>, jour: impl Into<i64>) -> String {
    format!("{:04}-{:02}-{:02}", an.into(), mois.into(), jour.into())
}

/// Ce qui rend une date impossible : l'annee d'abord, sinon le mois, sinon le jour.
pub(crate) fn date_impossible(an: impl Into<i64>, mois: impl Into<i64>, jour: impl Into<i64>) -> String {
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

