//! LA LIGNE DE PAIE : une rubrique, et les valeurs que sa forme porte. Une variante par forme, declaree ici, l'etiquette
//! `forme` ecrite par le moteur :
//! - celles qu'une personne (ou un LLM) saisit, rangees a la porte (`LigneDonnee::ranger`) selon ce que le catalogue
//!   demande de leur rubrique (`saisie`, et la nature `frais`) : `unites`, `unites_et_heures`, `frais`, `pourcentage`,
//!   `montant`, `avantage_en_nature` ;
//! - celles que le moteur compose (`composee` : un contrat paye au mois, `conges::lignes_du_mois`, `heures_en_plus`, le
//!   solde, la part des frais au-dela de leur limite), et celles d'un arret (`retenue_arret`, `imprevus`), de ses
//!   indemnites journalieres et de son maintien (`indemnite_journaliere`, `maintien`, `maintien.rs`) ;
//! - celle qu'un bulletin importe imprime (`importee`, `importe::LigneImprimee`).
//!
//! Ses nombres sont des decimaux exacts, ecrits en texte ; une case pas encore remplie est `null`. Une fois rangee, la
//! ligne se relit strictement, sans retourner au catalogue.

use std::collections::BTreeMap;

use droit::autre_revenu_brut::AutreElementDeRevenuBrut;
use droit::catalogue::{CodeDeRubrique, Unite};
use serde::{Deserialize, Serialize};
use socle::forme::Nombre;

use socle::date::Jour;
use socle::mois::Mois;
use socle::echec::{refus, Echec, R};
use socle::grandeurs::{courte, sans_zero, Coefficient, Heures, HeuresParUnite, Jours, Montant, MontantUnitaire, Taux};

/// UNE DATE D'UNE LIGNE : le jour nu (`"2026-10-02"`), ou detaille quand il dit sa quantite ou ou il se joue
/// (`{"jour": "2026-10-02", "quantite": {"heures": "4"}, "lieu": "Berlin"}`, `JourDeLigne`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DateDeLigne {
    Nue(Jour),
    Detaillee(JourDeLigne),
}

/// UN JOUR D'UNE LIGNE QUI DIT CE QU'IL PORTE : sa quantite, dans l'unite de la ligne (4 des 10 heures d'une ligne ;
/// Leandre, 25/11/2026 : « si je mets 3 jours, 12 heures, il met 4 heures par jour »), et ou il se joue. Chacun ne
/// s'ecrit que s'il est dit ; une date qui ne dit ni l'un ni l'autre est nue. Une ligne dont aucun jour ne dit sa
/// quantite porte sur chacun sa part de son total, le reste sur les premiers ; des qu'un jour dit la sienne, un jour qui
/// n'en dit pas est vide (`LignePaie::quantites_des_jours`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JourDeLigne {
    pub jour: Jour,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantite: Option<QuantiteDeLigne>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lieu: Option<String>,
}

impl DateDeLigne {
    /// Le jour.
    pub fn jour(&self) -> Jour {
        match self {
            DateDeLigne::Nue(j) => *j,
            DateDeLigne::Detaillee(d) => d.jour,
        }
    }

    /// Le lieu que la date dit d'elle-meme, tel qu'il est ecrit ; `None` s'il est vide.
    pub fn lieu(&self) -> Option<String> {
        match self {
            DateDeLigne::Detaillee(JourDeLigne { lieu: Some(lieu), .. }) if !lieu.is_empty() => Some(lieu.clone()),
            _ => None,
        }
    }

    /// La quantite que la date dit porter, dans l'unite de sa ligne ; `None` : elle n'en dit pas (ce qu'elle porte alors,
    /// `LignePaie::quantites_des_jours`).
    pub fn quantite(&self) -> Option<QuantiteDeLigne> {
        match self {
            DateDeLigne::Detaillee(d) => d.quantite,
            DateDeLigne::Nue(_) => None,
        }
    }

    /// Le jour `jour`, au lieu `lieu` s'il est dit.
    pub fn au(jour: Jour, lieu: String) -> DateDeLigne {
        DateDeLigne::de(jour, None, Some(lieu))
    }

    /// Le jour `jour`, sa quantite `quantite` et son lieu `lieu` quand ils sont dits (un lieu vide ne l'est pas) ; ni l'un
    /// ni l'autre : nu.
    pub fn de(jour: Jour, quantite: Option<QuantiteDeLigne>, lieu: Option<String>) -> DateDeLigne {
        let lieu = lieu.filter(|l| !l.is_empty());
        match (quantite, lieu) {
            (None, None) => DateDeLigne::Nue(jour),
            (quantite, lieu) => DateDeLigne::Detaillee(JourDeLigne { jour, quantite, lieu }),
        }
    }

    /// La meme date, portant la quantite `q` (`None` : plus aucune), son lieu garde.
    pub fn portant(&self, q: Option<QuantiteDeLigne>) -> DateDeLigne {
        DateDeLigne::de(self.jour(), q, self.lieu())
    }
}

/// UNE CASE D'UNE LIGNE DE FRAIS, cochee : la compagnie garde le justificatif de la depense ; la nuit est a Paris ou en
/// petite couronne ; le salarie est contraint de prendre son repas au restaurant ; la compagnie a paye l'hotel d'un repas
/// a l'etranger (`crate::frais_au_dela`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseDeFrais {
    Justificatif,
    Paris,
    Restaurant,
    Loge,
}

impl CaseDeFrais {
    /// Sa cle, telle que la ligne et l'ecran l'ecrivent.
    pub fn cle(self) -> &'static str {
        match self {
            CaseDeFrais::Justificatif => "justificatif",
            CaseDeFrais::Paris => "paris",
            CaseDeFrais::Restaurant => "restaurant",
            CaseDeFrais::Loge => "loge",
        }
    }
}

/// UNE LIGNE EN UNITES : quantite x taux (`saisie` : `quantite`, `taux`), ses jours, ou elle se joue. Une case pas encore
/// remplie est `null`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unites {
    pub rubrique: CodeDeRubrique,
    pub quantite: Option<QuantiteDeLigne>,
    #[serde(with = "texte_option")]
    pub taux: Option<MontantUnitaire>,
    pub lieu: Option<String>,
    /// Ses jours du mois.
    pub dates: Vec<DateDeLigne>,
    /// Ses jours des mois suivants.
    pub planning: Vec<DateDeLigne>,
}

/// UNE LIGNE EN UNITES QUI DIT SES HEURES par unite (`saisie` : `quantite`, `taux`, `heures` ; `QuantiteDeLigne::heures`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitesEtHeures {
    pub rubrique: CodeDeRubrique,
    pub quantite: Option<QuantiteDeLigne>,
    #[serde(with = "texte_option")]
    pub taux: Option<MontantUnitaire>,
    #[serde(with = "texte_option")]
    pub heures: Option<HeuresParUnite>,
    pub lieu: Option<String>,
    pub dates: Vec<DateDeLigne>,
    pub planning: Vec<DateDeLigne>,
}

/// UNE LIGNE DE FRAIS AU FORFAIT (nature `frais`, `saisie` : `quantite`, `taux`) : son lieu hors de la metropole
/// (`LieuDeFrais` ; `null` : la metropole) et ses cases cochees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frais {
    pub rubrique: CodeDeRubrique,
    pub quantite: Option<QuantiteDeLigne>,
    #[serde(with = "texte_option")]
    pub taux: Option<MontantUnitaire>,
    pub territoire: Option<LieuDeFrais>,
    pub cases: Vec<CaseDeFrais>,
    /// UNE CLE GARDEE, QUI N'EST PAS UNE CASE (`FormeSaisie::gardees`) : le lieu qu'une ligne de frais disait. Rien ne le
    /// lit (un frais ne fait aucun jour du contrat ; le lieu qui change la limite d'un repas ou d'une nuitee est son
    /// territoire) ; une ligne ecrite le porte, et elle se relit telle quelle.
    pub lieu: Option<String>,
    pub dates: Vec<DateDeLigne>,
    pub planning: Vec<DateDeLigne>,
    /// Ce que le moteur pose sur elle pour l'ecran ; rien ne s'ecrit.
    #[serde(skip)]
    pub marques: Marques,
}

impl Frais {
    /// La case `case` est-elle cochee ?
    pub fn cochee(&self, case: CaseDeFrais) -> bool {
        self.cases.contains(&case)
    }
}

/// LE LIEU D'UNE LIGNE DE NUITEE OU DE REPAS HORS DE LA METROPOLE (le choix « Lieu », sous « Avancé » ; absent : la
/// metropole) : un territoire hors metropole (`TerritoireHorsMetropole`), ou un lieu a l'etranger, par son code de la
/// table des missions (`droit::missions::LieuDeMission`). Il s'ecrit par son texte (`dom`, `pacifique`, `taaf`,
/// `monaco`, ou le code du lieu) et se lit a la porte par lui.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum LieuDeFrais {
    HorsMetropole(TerritoireHorsMetropole),
    Etranger(droit::missions::LieuDeMission),
}

impl std::fmt::Display for LieuDeFrais {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LieuDeFrais::HorsMetropole(t) => f.write_str(t.texte()),
            LieuDeFrais::Etranger(l) => f.write_str(l.texte()),
        }
    }
}

impl From<LieuDeFrais> for String {
    fn from(l: LieuDeFrais) -> String {
        l.to_string()
    }
}

impl TryFrom<String> for LieuDeFrais {
    type Error = Echec;
    /// Un territoire hors metropole par sa cle, sinon un lieu a l'etranger par son code.
    fn try_from(texte: String) -> R<LieuDeFrais> {
        match TerritoireHorsMetropole::TOUS.into_iter().find(|t| t.texte() == texte) {
            Some(t) => Ok(LieuDeFrais::HorsMetropole(t)),
            None => Ok(LieuDeFrais::Etranger(droit::missions::LieuDeMission::lire(&texte)?)),
        }
    }
}

/// UN TERRITOIRE HORS DE LA METROPOLE qu'une ligne de frais dit : un territoire de `frais_hors_metropole` du bareme
/// (`dom`, `pacifique`, `monaco`), ou les Terres australes et antarctiques (`taaf`), ou aucun forfait ne vaut. Le choix
/// « Lieu » les propose avant les lieux a l'etranger, dans cet ordre (`TOUS`), chacun dit par `dit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerritoireHorsMetropole {
    Dom,
    Pacifique,
    Taaf,
    Monaco,
}

impl TerritoireHorsMetropole {
    pub const TOUS: [TerritoireHorsMetropole; 4] = [TerritoireHorsMetropole::Dom, TerritoireHorsMetropole::Pacifique, TerritoireHorsMetropole::Taaf, TerritoireHorsMetropole::Monaco];

    /// Sa cle : celle du bareme (`frais_hors_metropole`), ou `taaf`.
    pub fn texte(self) -> &'static str {
        match self {
            TerritoireHorsMetropole::Dom => "dom",
            TerritoireHorsMetropole::Pacifique => "pacifique",
            TerritoireHorsMetropole::Taaf => "taaf",
            TerritoireHorsMetropole::Monaco => "monaco",
        }
    }

    /// Ce que le choix « Lieu » en dit.
    pub fn dit(self) -> &'static str {
        match self {
            TerritoireHorsMetropole::Dom => "Outre-mer : Antilles, Guyane, Réunion, Mayotte, Saint-Pierre-et-Miquelon, Saint-Barthélemy, Saint-Martin",
            TerritoireHorsMetropole::Pacifique => "Outre-mer : Nouvelle-Calédonie, Wallis-et-Futuna, Polynésie française",
            TerritoireHorsMetropole::Taaf => "Outre-mer : Terres australes et antarctiques françaises",
            TerritoireHorsMetropole::Monaco => "Monaco",
        }
    }
}

/// UNE LIGNE EN POURCENTAGE (`saisie` : `base`, `taux_majoration`) : sa base, en euros, et son pourcentage ; elle se
/// paie en quantite (la base) et taux (le pourcentage sur cent).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnPourcentage {
    pub rubrique: CodeDeRubrique,
    #[serde(with = "texte_option")]
    pub base: Option<Montant>,
    #[serde(with = "texte_option")]
    pub taux_majoration: Option<Taux>,
}

/// UNE LIGNE AU MONTANT (`saisie` : `montant` : une prime, un acompte, une indemnite, un defraiement au reel) : une fois
/// son montant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuMontant {
    pub rubrique: CodeDeRubrique,
    #[serde(with = "texte_option")]
    pub montant: Option<Montant>,
}

/// UN AVANTAGE EN NATURE (`saisie` : `montant`, `genre`) : son montant, et son genre (S21.G00.54.001,
/// `droit::catalogue::code_54` ; `null` : celui de la rubrique).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvantageEnNature {
    pub rubrique: CodeDeRubrique,
    #[serde(with = "texte_option")]
    pub montant: Option<Montant>,
    pub genre: Option<AutreElementDeRevenuBrut>,
}

/// UNE LIGNE QUE LE MOTEUR COMPOSE : ce qu'elle paie (sa quantite et son taux, dans l'unite de sa rubrique), ses heures
/// par unite quand elle en dit, ce qu'elle dit d'elle-meme sur le bulletin (`calcul::LigneBrut::precision`), ses jours.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composee {
    pub rubrique: CodeDeRubrique,
    pub paiement: PaiementDeLigne,
    #[serde(with = "texte_option")]
    pub heures: Option<HeuresParUnite>,
    pub precision: Option<String>,
    pub dates: Vec<DateDeLigne>,
}

/// LA RETENUE D'UN ARRET DE TRAVAIL (`imprevus::lignes_du_mois`) : ses heures d'absence au taux horaire ; elle tombe du
/// calcul du mois sans l'absence (`maintien::au_net`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetenueArret {
    pub rubrique: CodeDeRubrique,
    pub paiement: PaiementDeLigne,
}

/// UNE INDEMNITE JOURNALIERE SUBROGEE (`maintien::lignes`) : ses jours a son taux, aucune heure (S21.G00.53 : les heures
/// de l'absence restent celles de sa retenue) ; la nette dit l'assiette du prelevement a la source qu'elle reverse
/// (`Assiette::IjSubrogeesAssiettePas`). Elle tombe d'un calcul sans indemnite (`maintien::au_net`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndemniteJournaliere {
    pub rubrique: CodeDeRubrique,
    #[serde(with = "texte")]
    pub jours: Jours,
    #[serde(with = "texte")]
    pub taux: MontantUnitaire,
    #[serde(with = "texte_option")]
    pub assiette_pas: Option<Montant>,
}

/// LE MAINTIEN DU SALAIRE (`maintien::lignes`) : une somme, pas des heures de travail. Garanti en net, il porte ce qu'il
/// faut pour le reconstituer (`maintien::au_net`), qui ne s'ecrit pas : le calcul le remplace par son montant
/// (`maintien::avec_le_montant`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Maintien {
    pub rubrique: CodeDeRubrique,
    pub paiement: PaiementDeLigne,
    #[serde(skip)]
    pub reconstitution: Option<Box<crate::maintien::Reconstitution>>,
}

/// UNE LIGNE DE PAIE, sa forme dite (`forme`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "forme", rename_all = "snake_case", deny_unknown_fields)]
pub enum LignePaie {
    Unites(Unites),
    UnitesEtHeures(UnitesEtHeures),
    Frais(Frais),
    Pourcentage(EnPourcentage),
    Montant(AuMontant),
    AvantageEnNature(AvantageEnNature),
    Composee(Composee),
    RetenueArret(RetenueArret),
    IndemniteJournaliere(IndemniteJournaliere),
    Maintien(Maintien),
}

/// CE QUE LE MOTEUR POSE SUR UNE LIGNE DE FRAIS CALCULEE, pour l'ecran (`frais_au_dela`) : son rang parmi les lignes de
/// la carte, les cases qui ont lieu d'etre et leur aide, et le taux saisi avant que sa part au-dela de la limite s'en
/// detache.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Marques {
    pub rang: Option<RangDeLigne>,
    pub cases: Vec<CaseDeFrais>,
    pub aides: BTreeMap<CaseDeFrais, String>,
    pub taux_saisi: Option<MontantUnitaire>,
}

/// LE RANG D'UNE LIGNE parmi les lignes de sa carte, 0 pour la premiere : ce que la page lit pour la designer (une case
/// d'une ligne de frais, une ligne payee pendant un imprevu). Il s'ecrit en nombre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RangDeLigne(usize);

impl RangDeLigne {
    /// Chaque ligne de la carte (ou ce qui se range comme elles), avec son rang.
    pub fn de_chacun<I: IntoIterator>(liste: I) -> impl Iterator<Item = (RangDeLigne, I::Item)> {
        liste.into_iter().enumerate().map(|(i, x)| (RangDeLigne(i), x))
    }
}

/// Un nombre d'une ligne : ecrit en texte, son echelle gardee (`ecrit::texte`) ; lu d'un nombre ou d'un texte.
mod texte {
    pub use socle::grandeurs::ecrit::texte::serialize;
    pub use socle::grandeurs::lu::nombre_ou_texte::deserialize;
}

/// `texte`, une case : `null` tant qu'elle n'est pas remplie.
mod texte_option {
    pub use socle::grandeurs::ecrit::texte::option::serialize;
    pub use socle::grandeurs::lu::nombre_ou_texte::option::deserialize;
}

/// LA QUANTITE ET LE TAUX D'UNE LIGNE EN TEXTE, sans leur unite, son echelle gardee (`socle::grandeurs::ecrit::texte`) :
/// une reponse qui les donne a cote de leur rubrique (l'ecran du mois). Chaque grandeur s'ecrit par l'ecriture du socle.
pub mod en_texte {
    use super::{PaiementDeLigne, QuantiteDeLigne, TauxDeLigne};
    use socle::grandeurs::ecrit::texte::serialize as ecrit;

    /// La quantite que le paiement `p` paie ; celle d'une ligne en pourcentage, sa base.
    pub fn quantite<S: serde::Serializer>(p: &PaiementDeLigne, s: S) -> Result<S::Ok, S::Error> {
        match p.quantite_ou_base() {
            Ok(q) => la_quantite(&q, s),
            Err(b) => ecrit(&b, s),
        }
    }

    fn la_quantite<S: serde::Serializer>(q: &QuantiteDeLigne, s: S) -> Result<S::Ok, S::Error> {
        match q {
            QuantiteDeLigne::Heures(g) => ecrit(g, s),
            QuantiteDeLigne::Cachets(g) => ecrit(g, s),
            QuantiteDeLigne::Jours(g) => ecrit(g, s),
            QuantiteDeLigne::Services(g) => ecrit(g, s),
            QuantiteDeLigne::Semaines(g) => ecrit(g, s),
            QuantiteDeLigne::PartDuMois(g) => ecrit(g, s),
            QuantiteDeLigne::Unites(g) => ecrit(g, s),
        }
    }

    pub fn taux<S: serde::Serializer>(t: &TauxDeLigne, s: S) -> Result<S::Ok, S::Error> {
        match t {
            TauxDeLigne::ParUnite(g) => ecrit(g, s),
            TauxDeLigne::PartDeLaBase(g) => ecrit(g, s),
        }
    }

    /// Un taux qui peut manquer : `None` s'ecrit `null`.
    pub mod option {
        use super::TauxDeLigne;

        pub fn taux<S: serde::Serializer>(t: &Option<TauxDeLigne>, s: S) -> Result<S::Ok, S::Error> {
            match t {
                Some(t) => super::taux(t, s),
                None => s.serialize_none(),
            }
        }
    }
}

/// Chaque variante de `QuantiteDeLigne`, sa grandeur nommee `$g` : `$e`, du meme type pour toutes.
macro_rules! chaque_quantite {
    ($q:expr, $g:ident => $e:expr) => {
        match $q {
            QuantiteDeLigne::Heures($g) => $e,
            QuantiteDeLigne::Cachets($g) => $e,
            QuantiteDeLigne::Jours($g) => $e,
            QuantiteDeLigne::Services($g) => $e,
            QuantiteDeLigne::Semaines($g) => $e,
            QuantiteDeLigne::PartDuMois($g) => $e,
            QuantiteDeLigne::Unites($g) => $e,
        }
    };
}

/// Chaque variante de `QuantiteDeLigne`, sa grandeur nommee `$g` devenue `$e` (une grandeur de meme nom), dans la meme
/// variante.
macro_rules! chaque_quantite_devient {
    ($q:expr, $g:ident => $e:expr) => {
        match $q {
            QuantiteDeLigne::Heures($g) => QuantiteDeLigne::Heures($e),
            QuantiteDeLigne::Cachets($g) => QuantiteDeLigne::Cachets($e),
            QuantiteDeLigne::Jours($g) => QuantiteDeLigne::Jours($e),
            QuantiteDeLigne::Services($g) => QuantiteDeLigne::Services($e),
            QuantiteDeLigne::Semaines($g) => QuantiteDeLigne::Semaines($e),
            QuantiteDeLigne::PartDuMois($g) => QuantiteDeLigne::PartDuMois($e),
            QuantiteDeLigne::Unites($g) => QuantiteDeLigne::Unites($e),
        }
    };
}

/// LA QUANTITE D'UNE LIGNE DE PAIE, dans l'unite que sa rubrique dit (`droit::catalogue::Unite`) : des heures, des
/// cachets, des jours, des services, des semaines, la part du mois payee (1 pour un mois entier), le compte d'un forfait
/// (3 repas). Une ligne en pourcentage n'en a pas : elle paie une part de sa base (`PaiementDeLigne::Base`). Elle s'ecrit
/// par sa forme : sa grandeur, puis son nombre en texte (`{"cachets": "2"}`) ; une charge peut ecrire son nombre en
/// nombre (`{"cachets": 2}`). La cle de sa forme est celle de son unite (`cle`), dite la seulement. Une ligne saisie la
/// range une fois dans l'unite de sa rubrique (`LigneDonnee::ranger`). Quantite x montant unitaire = montant (`fois`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuantiteDeLigne {
    Heures(Heures),
    Cachets(socle::grandeurs::Cachets),
    Jours(Jours),
    Services(socle::grandeurs::Services),
    Semaines(socle::grandeurs::Semaines),
    PartDuMois(socle::grandeurs::Coefficient),
    Unites(socle::grandeurs::Unites),
}

/// Une grandeur ecrite en texte (`texte`), la valeur de la forme d'une quantite.
struct EnTexte<G>(G);

impl<G: socle::grandeurs::Grandeur> Serialize for EnTexte<G> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        texte::serialize(&self.0, s)
    }
}

impl<'de, G: socle::grandeurs::Grandeur> Deserialize<'de> for EnTexte<G> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<EnTexte<G>, D::Error> {
        texte::deserialize(d).map(EnTexte)
    }
}

impl Serialize for QuantiteDeLigne {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(1))?;
        chaque_quantite!(self, g => m.serialize_entry(self.cle_ecrite(), &EnTexte(*g)))?;
        m.end()
    }
}

impl<'de> Deserialize<'de> for QuantiteDeLigne {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<QuantiteDeLigne, D::Error> {
        struct Visiteur;
        impl<'de> serde::de::Visitor<'de> for Visiteur {
            type Value = QuantiteDeLigne;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("une quantite dans son unite : {\"cachets\": \"2\"}")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<QuantiteDeLigne, A::Error> {
                use QuantiteDeLigne as Q;
                let Some(cle) = m.next_key::<String>()? else { return Err(serde::de::Error::custom("une quantite dit son unite")) };
                let cle_de = QuantiteDeLigne::cle;
                let q = match Some(cle.as_str()) {
                    c if c == cle_de(Unite::Heure) => Q::Heures(m.next_value::<EnTexte<_>>()?.0),
                    c if c == cle_de(Unite::Cachet) => Q::Cachets(m.next_value::<EnTexte<_>>()?.0),
                    c if c == cle_de(Unite::Jour) => Q::Jours(m.next_value::<EnTexte<_>>()?.0),
                    c if c == cle_de(Unite::Service) => Q::Services(m.next_value::<EnTexte<_>>()?.0),
                    c if c == cle_de(Unite::Semaine) => Q::Semaines(m.next_value::<EnTexte<_>>()?.0),
                    c if c == cle_de(Unite::Mois) => Q::PartDuMois(m.next_value::<EnTexte<_>>()?.0),
                    c if c == cle_de(Unite::Forfait) => Q::Unites(m.next_value::<EnTexte<_>>()?.0),
                    _ => return Err(serde::de::Error::custom(format!("{} n'est pas l'unite d'une quantite", socle::ecriture::json_en_ligne(&cle)))),
                };
                if m.next_key::<String>()?.is_some() {
                    return Err(serde::de::Error::custom("une quantite dit une seule unite"));
                }
                Ok(q)
            }
        }
        d.deserialize_map(Visiteur)
    }
}

impl QuantiteDeLigne {
    /// Une fois : la quantite d'une ligne qui se paie en une fois (une prime, une indemnite), son montant pour une unite.
    pub const UNE_FOIS: QuantiteDeLigne = QuantiteDeLigne::Unites(socle::grandeurs::Unites::UNE);

    /// LA QUANTITE QU'UNE LIGNE D'UN BULLETIN IMPRIME (`q`, un nombre lu de la page), dans l'unite `unite` de la rubrique a
    /// laquelle l'import la rattache : la porte de sa lecture ; `None` : le pourcentage, dont le nombre imprime est une
    /// base (`PaiementDeLigne::imprime`). `Err` : des cachets a virgule, qui ne sont pas un nombre de cachets
    /// (`socle::forme::Nombre::cachets`).
    pub fn imprimee(unite: Unite, q: Nombre) -> Result<Option<QuantiteDeLigne>, String> {
        Ok(Some(match unite {
            Unite::Heure => QuantiteDeLigne::Heures(q.grandeur()),
            Unite::Cachet => QuantiteDeLigne::Cachets(q.cachets()?),
            Unite::Jour => QuantiteDeLigne::Jours(q.grandeur()),
            Unite::Service => QuantiteDeLigne::Services(q.grandeur()),
            Unite::Semaine => QuantiteDeLigne::Semaines(q.grandeur()),
            Unite::Mois => QuantiteDeLigne::PartDuMois(q.grandeur()),
            Unite::Forfait => QuantiteDeLigne::Unites(q.grandeur()),
            Unite::Pourcentage => return Ok(None),
        }))
    }

    /// UNE PAR DATE, `n` dates : des cachets, des journees ou des services, qui se comptent ainsi ; `None` pour une
    /// autre unite (des heures ne se deduisent pas d'un nombre de jours).
    pub fn par_dates(unite: Unite, n: usize) -> Option<QuantiteDeLigne> {
        match unite {
            Unite::Cachet => Some(QuantiteDeLigne::Cachets(socle::grandeurs::Cachets::de_dates(n))),
            Unite::Jour => Some(QuantiteDeLigne::Jours(Jours::de_dates(n))),
            Unite::Service => Some(QuantiteDeLigne::Services(socle::grandeurs::Services::comptes(n))),
            Unite::Heure | Unite::Semaine | Unite::Mois | Unite::Forfait | Unite::Pourcentage => None,
        }
    }

    /// SE COMPTE-T-ELLE JOUR PAR JOUR dans l'unite `unite` (des heures, des cachets, des journees, des services, des
    /// unites d'un forfait) : chaque jour d'une telle ligne porte sa part (`JourDeLigne::quantite`). Des semaines, une part
    /// du mois, un pourcentage ne se comptent pas par jour.
    pub fn se_compte_par_jour(unite: Unite) -> bool {
        matches!(unite, Unite::Heure | Unite::Cachet | Unite::Jour | Unite::Service | Unite::Forfait)
    }

    /// ELLE, REPARTIE SUR `n` JOURS, le reste sur les premiers (`socle::grandeurs::repartie`, la regle « A » : 10 h sur
    /// 3 jours, 4 · 3 · 3).
    pub fn repartie(self, n: usize) -> Vec<QuantiteDeLigne> {
        use socle::grandeurs::repartie;
        use QuantiteDeLigne as Q;
        match self {
            Q::Heures(g) => repartie(g, n).into_iter().map(Q::Heures).collect(),
            Q::Cachets(g) => repartie(g, n).into_iter().map(Q::Cachets).collect(),
            Q::Jours(g) => repartie(g, n).into_iter().map(Q::Jours).collect(),
            Q::Services(g) => repartie(g, n).into_iter().map(Q::Services).collect(),
            Q::Semaines(g) => repartie(g, n).into_iter().map(Q::Semaines).collect(),
            Q::PartDuMois(g) => repartie(g, n).into_iter().map(Q::PartDuMois).collect(),
            Q::Unites(g) => repartie(g, n).into_iter().map(Q::Unites).collect(),
        }
    }

    /// RIEN, dans l'unite `unite` (une ligne du planning dont aucune date ne compte la quantite, a completer) ; `None` :
    /// le pourcentage, qui n'a pas de quantite.
    pub fn nulle(unite: Unite) -> Option<QuantiteDeLigne> {
        use socle::grandeurs::nulle;
        Some(match unite {
            Unite::Heure => QuantiteDeLigne::Heures(nulle()),
            Unite::Cachet => QuantiteDeLigne::Cachets(nulle()),
            Unite::Jour => QuantiteDeLigne::Jours(nulle()),
            Unite::Service => QuantiteDeLigne::Services(nulle()),
            Unite::Semaine => QuantiteDeLigne::Semaines(nulle()),
            Unite::Mois => QuantiteDeLigne::PartDuMois(nulle()),
            Unite::Forfait => QuantiteDeLigne::Unites(nulle()),
            Unite::Pourcentage => return None,
        })
    }

    /// UNE, dans l'unite `unite` : la quantite d'une ligne au montant (`PaiementDeLigne::une_fois`) ; `None` : le
    /// pourcentage, qui n'a pas de quantite.
    pub fn une(unite: Unite) -> Option<QuantiteDeLigne> {
        use socle::grandeurs::une;
        Some(match unite {
            Unite::Heure => QuantiteDeLigne::Heures(une()),
            Unite::Cachet => QuantiteDeLigne::Cachets(une()),
            Unite::Jour => QuantiteDeLigne::Jours(une()),
            Unite::Service => QuantiteDeLigne::Services(une()),
            Unite::Semaine => QuantiteDeLigne::Semaines(une()),
            Unite::Mois => QuantiteDeLigne::PartDuMois(une()),
            Unite::Forfait => QuantiteDeLigne::Unites(une()),
            Unite::Pourcentage => return None,
        })
    }

    /// L'UNITE DE SA RUBRIQUE, que sa grandeur dit.
    pub fn unite(self) -> Unite {
        match self {
            QuantiteDeLigne::Heures(_) => Unite::Heure,
            QuantiteDeLigne::Cachets(_) => Unite::Cachet,
            QuantiteDeLigne::Jours(_) => Unite::Jour,
            QuantiteDeLigne::Services(_) => Unite::Service,
            QuantiteDeLigne::Semaines(_) => Unite::Semaine,
            QuantiteDeLigne::PartDuMois(_) => Unite::Mois,
            QuantiteDeLigne::Unites(_) => Unite::Forfait,
        }
    }

    /// La cle de sa forme (`{"cachets": …}`).
    fn cle_ecrite(self) -> &'static str {
        match self {
            QuantiteDeLigne::Heures(_) => "heures",
            QuantiteDeLigne::Cachets(_) => "cachets",
            QuantiteDeLigne::Jours(_) => "jours",
            QuantiteDeLigne::Services(_) => "services",
            QuantiteDeLigne::Semaines(_) => "semaines",
            QuantiteDeLigne::PartDuMois(_) => "part_du_mois",
            QuantiteDeLigne::Unites(_) => "unites",
        }
    }

    /// LA CLE DE SA FORME dans l'unite `unite` (`{"cachets": …}`) : ce que la page ecrit pour une ligne de cette rubrique
    /// (`routes::ouverture::RubriqueDeLaPage`), ce qu'un refus de la porte cite ; `None` : le pourcentage, qui n'a pas
    /// de quantite.
    pub fn cle(unite: Unite) -> Option<&'static str> {
        QuantiteDeLigne::nulle(unite).map(QuantiteDeLigne::cle_ecrite)
    }

    /// Est-elle negative ?
    pub fn est_negative(self) -> bool {
        chaque_quantite!(self, q => q < socle::grandeurs::nulle())
    }

    /// Sans zero inutile (`socle::grandeurs::sans_zero`) : la forme canonique d'une ligne.
    pub fn sans_zero(self) -> QuantiteDeLigne {
        chaque_quantite_devient!(self, q => sans_zero(q))
    }

    /// AU CENTIEME, la moitie au pair, sans zero inutile (`socle::grandeurs::au_centieme_au_pair`) : ce que la designation
    /// d'une ligne en dit.
    pub fn au_centieme(self) -> QuantiteDeLigne {
        chaque_quantite_devient!(self, q => socle::grandeurs::au_centieme_au_pair(q))
    }

    /// Se dit-elle au pluriel (`socle::texte::au_pluriel` : des deux) ?
    pub fn au_pluriel(self) -> bool {
        chaque_quantite!(self, q => socle::texte::au_pluriel(q))
    }

    /// Est-elle un nombre entier (`socle::grandeurs::entiere`) ?
    pub fn entiere(self) -> bool {
        chaque_quantite!(self, q => socle::grandeurs::entiere(q).is_some())
    }

    /// Est-elle nulle ?
    pub fn est_nul(self) -> bool {
        chaque_quantite!(self, q => q.est_nul())
    }

    /// SON UNITE, ACCORDEE A ELLE (« heures », « cachet » ; au singulier jusqu'a deux exclus, `au_pluriel`) : la
    /// designation d'une ligne (`calcul`), et ce qu'une rangee d'un jour montre sous sa rubrique. Le mois ne prend pas d's.
    pub fn unite_dite(self) -> String {
        let unite = self.unite();
        let s = if self.au_pluriel() && unite != Unite::Mois { "s" } else { "" };
        format!("{}{}", unite, s)
    }

    /// TELLE QU'UNE PHRASE LA DIT, sans son unite (« 151,67 », « 2 »).
    pub fn dit(self) -> String {
        chaque_quantite!(self, q => socle::texte::virgule(q))
    }

    /// LES HEURES QU'ELLE FAIT COMPTER, a `par_unite` heures par unite (`Rubrique::heures_par_unite` : le forfait de sa
    /// rubrique, sinon celles que la ligne dit) : quantite x heures par unite, au centieme d'heure comme la DSN l'ecrit
    /// (14 services de 4 h : 56 h ; un mois de permanent : la part du mois payee x la quotite du contrat). Sans heures par
    /// unite, une quantite d'heures compte ses heures, une autre n'en compte aucune. Le nombre rendu est dans sa forme la
    /// plus courte (`courte`).
    pub fn heures(self, par_unite: Option<HeuresParUnite>) -> Heures {
        use QuantiteDeLigne as Q;
        match (par_unite, self) {
            (Some(p), q) => courte(q.fois(p).au_centieme()),
            (None, Q::Heures(q)) => courte(q),
            (None, _) => courte(Heures::ZERO),
        }
    }

    /// LES HEURES PAR UNITE QU'UN TOTAL `total` FAIT sur cette quantite (`Heures::par_unite` : 8 h sur 2 services, 4 h) ;
    /// `None` : une quantite nulle.
    pub fn partage(self, total: Heures) -> Option<HeuresParUnite> {
        chaque_quantite!(self, q => total.par_unite(q))
    }

    /// QUANTITE x HEURES PAR UNITE, exactes (14 services de 4 h : 56 h).
    pub fn fois(self, par_unite: HeuresParUnite) -> Heures {
        chaque_quantite!(self, q => q * par_unite)
    }

    /// N'EST-ELLE PAS PLUS QUE `o`, dans la meme grandeur (une ligne coupee, `crate::lignes_coupees_a_la_fin`) ? Deux
    /// grandeurs ne se comparent pas : non.
    pub fn pas_plus_que(self, o: QuantiteDeLigne) -> bool {
        use QuantiteDeLigne as Q;
        match (self, o) {
            (Q::Heures(a), Q::Heures(b)) => a <= b,
            (Q::Cachets(a), Q::Cachets(b)) => a <= b,
            (Q::Jours(a), Q::Jours(b)) => a <= b,
            (Q::Services(a), Q::Services(b)) => a <= b,
            (Q::Semaines(a), Q::Semaines(b)) => a <= b,
            (Q::PartDuMois(a), Q::PartDuMois(b)) => a <= b,
            (Q::Unites(a), Q::Unites(b)) => a <= b,
            _ => false,
        }
    }

    /// Deux quantites de la meme grandeur ajoutees (deux lignes de la meme rubrique) ; `None` : deux grandeurs, qui ne
    /// s'ajoutent pas.
    pub fn plus(self, o: QuantiteDeLigne) -> Option<QuantiteDeLigne> {
        use QuantiteDeLigne as Q;
        Some(match (self, o) {
            (Q::Heures(a), Q::Heures(b)) => Q::Heures(a + b),
            (Q::Cachets(a), Q::Cachets(b)) => Q::Cachets(a + b),
            (Q::Jours(a), Q::Jours(b)) => Q::Jours(a + b),
            (Q::Services(a), Q::Services(b)) => Q::Services(a + b),
            (Q::Semaines(a), Q::Semaines(b)) => Q::Semaines(a + b),
            (Q::PartDuMois(a), Q::PartDuMois(b)) => Q::PartDuMois(a + b),
            (Q::Unites(a), Q::Unites(b)) => Q::Unites(a + b),
            _ => return None,
        })
    }
}

/// Les heures d'une ligne qui paie une part de sa base : aucune ; une rubrique en pourcentage qui dirait des heures par
/// unite se refuse.
fn sans_quantite(par_unite: Option<HeuresParUnite>) -> R<Heures> {
    match par_unite {
        Some(_) => Err(socle::refus!("une ligne en pourcentage ne compte pas d'heures par unité : le signaler au développeur")),
        None => Ok(courte(Heures::ZERO)),
    }
}

/// LE TAUX D'UNE LIGNE DE PAIE : ce que paie une unite de sa quantite (un montant unitaire : par heure, par cachet, le
/// montant d'une ligne au montant pour sa seule unite), ou, pour une ligne en pourcentage, la part de sa base qu'elle
/// paie (son pourcentage sur cent, un coefficient). Quantite x taux = montant (`PaiementDeLigne::montant`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TauxDeLigne {
    ParUnite(MontantUnitaire),
    PartDeLaBase(Coefficient),
}

impl TauxDeLigne {
    /// Est-il nul ?
    pub fn est_nul(self) -> bool {
        match self {
            TauxDeLigne::ParUnite(u) => u.est_nul(),
            TauxDeLigne::PartDeLaBase(c) => c.est_nul(),
        }
    }

    /// LE MONTANT UNITAIRE d'une ligne qui se paie a l'unite (un cachet, une heure, un salaire mensuel) ; `None` : une
    /// ligne en pourcentage, dont le taux est une part de sa base.
    pub fn par_unite(self) -> Option<MontantUnitaire> {
        match self {
            TauxDeLigne::ParUnite(u) => Some(u),
            TauxDeLigne::PartDeLaBase(_) => None,
        }
    }

    /// TEL QU'UNE PHRASE LE DIT (un contrat, un avenant) : un montant unitaire en euros (« 120,00 € »), la part d'une
    /// ligne en pourcentage en pourcent (« 10,00 % »).
    pub fn dit(self) -> String {
        match self {
            TauxDeLigne::ParUnite(u) => socle::texte::euros(u),
            TauxDeLigne::PartDeLaBase(c) => socle::texte::pourcent(c.en_taux()),
        }
    }
}

/// CE QUE PAIE UNE LIGNE DE PAIE : sa quantite et son taux, qui vont ensemble, dans l'unite de sa rubrique (`unite`) :
/// une quantite a un montant unitaire, ou, pour une rubrique en pourcentage, une base en euros a la part qu'elle en
/// paie. Quantite x taux = montant (`montant`), toujours. Il s'ecrit par sa forme : sa grandeur, puis
/// sa quantite et son taux en texte (`{"cachets": ["2", "150.00"]}`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PaiementDeLigne {
    Heures(#[serde(with = "texte")] Heures, #[serde(with = "texte")] MontantUnitaire),
    Cachets(#[serde(with = "texte")] socle::grandeurs::Cachets, #[serde(with = "texte")] MontantUnitaire),
    Jours(#[serde(with = "texte")] Jours, #[serde(with = "texte")] MontantUnitaire),
    Services(#[serde(with = "texte")] socle::grandeurs::Services, #[serde(with = "texte")] MontantUnitaire),
    Semaines(#[serde(with = "texte")] socle::grandeurs::Semaines, #[serde(with = "texte")] MontantUnitaire),
    PartDuMois(#[serde(with = "texte")] Coefficient, #[serde(with = "texte")] MontantUnitaire),
    Unites(#[serde(with = "texte")] socle::grandeurs::Unites, #[serde(with = "texte")] MontantUnitaire),
    Base(#[serde(with = "texte")] Montant, #[serde(with = "texte")] Coefficient),
}

impl PaiementDeLigne {
    /// CE QU'UNE LIGNE D'UN BULLETIN IMPORTE PAIE, sa quantite `q` et son taux `t` tels que la page les imprime (des
    /// nombres lus, `import::bulletin_lu`), dans l'unite `unite` de la rubrique a laquelle l'import la rattache : la porte
    /// de leur lecture. Le taux d'une rubrique en pourcentage s'imprime en pourcent (un « 10 » : 10 %) : la part de la
    /// base qu'il paie se prend comme le calcul la prend d'une ligne en pourcentage (`Taux::en_coefficient`). `Err` : des
    /// cachets a virgule, qui ne sont pas un nombre de cachets (`socle::forme::Nombre::cachets`).
    pub fn imprime(unite: Unite, q: Nombre, t: Nombre) -> Result<PaiementDeLigne, String> {
        use PaiementDeLigne as P;
        Ok(match unite {
            Unite::Heure => P::Heures(q.grandeur(), t.grandeur()),
            Unite::Cachet => P::Cachets(q.cachets()?, t.grandeur()),
            Unite::Jour => P::Jours(q.grandeur(), t.grandeur()),
            Unite::Service => P::Services(q.grandeur(), t.grandeur()),
            Unite::Semaine => P::Semaines(q.grandeur(), t.grandeur()),
            Unite::Mois => P::PartDuMois(q.grandeur(), t.grandeur()),
            Unite::Forfait => P::Unites(q.grandeur(), t.grandeur()),
            Unite::Pourcentage => P::Base(q.grandeur(), t.grandeur::<Taux>().en_coefficient()),
        })
    }

    /// CE QUE PAIE UNE LIGNE AU MONTANT `montant` (une prime, une indemnite, un avantage en nature), quelle que soit
    /// l'unite `unite` de sa rubrique : une unite a son montant ; dans une rubrique en pourcentage, sa base entiere.
    pub fn une_fois(unite: Unite, montant: Montant) -> PaiementDeLigne {
        use socle::grandeurs::une;
        use PaiementDeLigne as P;
        let u = montant.pour_une_unite();
        match unite {
            Unite::Heure => P::Heures(une(), u),
            Unite::Cachet => P::Cachets(une(), u),
            Unite::Jour => P::Jours(une(), u),
            Unite::Service => P::Services(une(), u),
            Unite::Semaine => P::Semaines(une(), u),
            Unite::Mois => P::PartDuMois(une(), u),
            Unite::Forfait => P::Unites(une(), u),
            Unite::Pourcentage => P::Base(montant, Coefficient::UN),
        }
    }

    /// L'unite de sa rubrique, que sa quantite dit (le pourcentage, pour une base).
    pub fn unite(self) -> Unite {
        match self.quantite_ou_base() {
            Ok(q) => q.unite(),
            Err(_) => Unite::Pourcentage,
        }
    }

    /// LE MEME PAIEMENT A UN AUTRE MONTANT UNITAIRE `u` (le tarif de la convention) ; `None` : une ligne en pourcentage,
    /// dont le taux est une part de sa base.
    pub fn avec_taux(self, u: MontantUnitaire) -> Option<PaiementDeLigne> {
        use PaiementDeLigne as P;
        Some(match self {
            P::Heures(q, _) => P::Heures(q, u),
            P::Cachets(q, _) => P::Cachets(q, u),
            P::Jours(q, _) => P::Jours(q, u),
            P::Services(q, _) => P::Services(q, u),
            P::Semaines(q, _) => P::Semaines(q, u),
            P::PartDuMois(q, _) => P::PartDuMois(q, u),
            P::Unites(q, _) => P::Unites(q, u),
            P::Base(_, _) => return None,
        })
    }

    /// Sans zero inutile (`socle::grandeurs::sans_zero`), sa quantite et son taux : la forme canonique d'une ligne, et
    /// celle de la paie ecrite (`calcul::EcritSansZero`).
    pub fn sans_zero(self) -> PaiementDeLigne {
        use PaiementDeLigne as P;
        match self {
            P::Heures(q, u) => P::Heures(sans_zero(q), sans_zero(u)),
            P::Cachets(q, u) => P::Cachets(sans_zero(q), sans_zero(u)),
            P::Jours(q, u) => P::Jours(sans_zero(q), sans_zero(u)),
            P::Services(q, u) => P::Services(sans_zero(q), sans_zero(u)),
            P::Semaines(q, u) => P::Semaines(sans_zero(q), sans_zero(u)),
            P::PartDuMois(q, u) => P::PartDuMois(sans_zero(q), sans_zero(u)),
            P::Unites(q, u) => P::Unites(sans_zero(q), sans_zero(u)),
            P::Base(b, c) => P::Base(sans_zero(b), sans_zero(c)),
        }
    }

    /// Dans leur forme la plus courte, une decimale au moins (`socle::grandeurs::courte`), sa quantite et son taux.
    pub fn courte(self) -> PaiementDeLigne {
        use PaiementDeLigne as P;
        match self {
            P::Heures(q, u) => P::Heures(courte(q), courte(u)),
            P::Cachets(q, u) => P::Cachets(courte(q), courte(u)),
            P::Jours(q, u) => P::Jours(courte(q), courte(u)),
            P::Services(q, u) => P::Services(courte(q), courte(u)),
            P::Semaines(q, u) => P::Semaines(courte(q), courte(u)),
            P::PartDuMois(q, u) => P::PartDuMois(courte(q), courte(u)),
            P::Unites(q, u) => P::Unites(courte(q), courte(u)),
            P::Base(b, c) => P::Base(courte(b), courte(c)),
        }
    }

    /// LA QUANTITE `q` A SON MONTANT UNITAIRE `u` (une ligne en pourcentage paie une base, `Base`, et n'a pas de
    /// quantite).
    pub fn de(q: QuantiteDeLigne, u: MontantUnitaire) -> PaiementDeLigne {
        use PaiementDeLigne as P;
        use QuantiteDeLigne as Q;
        match q {
            Q::Heures(q) => P::Heures(q, u),
            Q::Cachets(q) => P::Cachets(q, u),
            Q::Jours(q) => P::Jours(q, u),
            Q::Services(q) => P::Services(q, u),
            Q::Semaines(q) => P::Semaines(q, u),
            Q::PartDuMois(q) => P::PartDuMois(q, u),
            Q::Unites(q) => P::Unites(q, u),
        }
    }

    /// DEUX PAIEMENTS D'UNE MEME RUBRIQUE AJOUTES (`calcul::en_bref`) : leurs quantites, ou leurs bases, au taux du
    /// premier ; `None` : deux grandeurs, qui ne s'ajoutent pas.
    pub fn plus(self, o: PaiementDeLigne) -> Option<PaiementDeLigne> {
        match (self.quantite_ou_base(), o.quantite_ou_base(), self) {
            (Ok(a), Ok(b), _) => a.plus(b).and_then(|q| self.avec_quantite(q)),
            (Err(a), Err(b), PaiementDeLigne::Base(_, c)) => Some(PaiementDeLigne::Base(a + b, c)),
            _ => None,
        }
    }

    /// Sa quantite, ou la base en euros d'une ligne en pourcentage, qui n'a pas de quantite.
    pub(crate) fn quantite_ou_base(self) -> Result<QuantiteDeLigne, Montant> {
        Ok(match self {
            PaiementDeLigne::Heures(q, _) => QuantiteDeLigne::Heures(q),
            PaiementDeLigne::Cachets(q, _) => QuantiteDeLigne::Cachets(q),
            PaiementDeLigne::Jours(q, _) => QuantiteDeLigne::Jours(q),
            PaiementDeLigne::Services(q, _) => QuantiteDeLigne::Services(q),
            PaiementDeLigne::Semaines(q, _) => QuantiteDeLigne::Semaines(q),
            PaiementDeLigne::PartDuMois(q, _) => QuantiteDeLigne::PartDuMois(q),
            PaiementDeLigne::Unites(q, _) => QuantiteDeLigne::Unites(q),
            PaiementDeLigne::Base(b, _) => return Err(b),
        })
    }

    /// Sa quantite ; `None` : une ligne en pourcentage, qui paie une part de sa base.
    pub fn quantite(self) -> Option<QuantiteDeLigne> {
        self.quantite_ou_base().ok()
    }

    /// LES HEURES QU'IL FAIT COMPTER, a `par_unite` heures par unite (`QuantiteDeLigne::heures`) ; une base n'en compte
    /// aucune.
    pub fn heures(self, par_unite: Option<HeuresParUnite>) -> R<Heures> {
        match self.quantite_ou_base() {
            Ok(q) => Ok(q.heures(par_unite)),
            Err(_) => sans_quantite(par_unite),
        }
    }

    /// SA QUANTITE TELLE QU'UNE PHRASE LA DIT, sans son unite (« 151,67 », « 2 ») ; la base d'une ligne en pourcentage,
    /// en euros (« 1 512,40 € »).
    pub fn quantite_dite(self) -> String {
        match self.quantite_ou_base() {
            Ok(q) => q.dit(),
            Err(b) => socle::texte::euros(b),
        }
    }

    /// LE MEME PAIEMENT, SA QUANTITE DEVENUE `q` (deux lignes d'une meme rubrique additionnees, `calcul::en_bref`) ;
    /// `None` : `q` n'est pas de sa grandeur.
    pub fn avec_quantite(self, q: QuantiteDeLigne) -> Option<PaiementDeLigne> {
        use PaiementDeLigne as P;
        use QuantiteDeLigne as Q;
        Some(match (self, q) {
            (P::Heures(_, u), Q::Heures(q)) => P::Heures(q, u),
            (P::Cachets(_, u), Q::Cachets(q)) => P::Cachets(q, u),
            (P::Jours(_, u), Q::Jours(q)) => P::Jours(q, u),
            (P::Services(_, u), Q::Services(q)) => P::Services(q, u),
            (P::Semaines(_, u), Q::Semaines(q)) => P::Semaines(q, u),
            (P::PartDuMois(_, u), Q::PartDuMois(q)) => P::PartDuMois(q, u),
            (P::Unites(_, u), Q::Unites(q)) => P::Unites(q, u),
            _ => return None,
        })
    }

    /// Son taux.
    pub fn taux(self) -> TauxDeLigne {
        match self {
            PaiementDeLigne::Heures(_, u)
            | PaiementDeLigne::Cachets(_, u)
            | PaiementDeLigne::Jours(_, u)
            | PaiementDeLigne::Services(_, u)
            | PaiementDeLigne::Semaines(_, u)
            | PaiementDeLigne::PartDuMois(_, u)
            | PaiementDeLigne::Unites(_, u) => TauxDeLigne::ParUnite(u),
            PaiementDeLigne::Base(_, c) => TauxDeLigne::PartDeLaBase(c),
        }
    }

    /// Ce qu'elle paie, un montant exact (`Montant::au_centime` le fixe) : quantite x montant unitaire, ou base x la part
    /// qu'une ligne en pourcentage en paie.
    pub fn montant(self) -> Montant {
        match self {
            PaiementDeLigne::Heures(q, u) => q * u,
            PaiementDeLigne::Cachets(q, u) => q * u,
            PaiementDeLigne::Jours(q, u) => q * u,
            PaiementDeLigne::Services(q, u) => q * u,
            PaiementDeLigne::Semaines(q, u) => q * u,
            PaiementDeLigne::PartDuMois(q, u) => q * u,
            PaiementDeLigne::Unites(q, u) => q * u,
            PaiementDeLigne::Base(b, c) => b * c,
        }
    }
}

impl LignePaie {
    /// LA LIGNE QUE LE MOTEUR COMPOSE de `rubrique`, `quantite` x `taux`, sans heures, sans precision, sans jour.
    /// UNE LIGNE D'UNE RUBRIQUE DE LA PIECE D'UN SALARIE AU MOIS (`droit::catalogue::RubriqueDuMois`), que seul le calcul
    /// qui tient la cle d'un contrat paye au mois (`droit::natures::AuMois`) compose : sur la paie d'un intermittent, elle
    /// ne compile pas.
    pub fn du_mois(_cle: &droit::natures::AuMois, rubrique: droit::catalogue::RubriqueDuMois, paiement: PaiementDeLigne) -> LignePaie {
        LignePaie::composee(rubrique.code(), paiement)
    }

    /// LE REVERSEMENT DES INDEMNITES JOURNALIERES NETTES d'une compagnie subrogee (`maintien::lignes`) : une rubrique de la
    /// piece d'un salarie au mois (`RubriqueDuMois::IjssNettes`), que seul le calcul qui tient sa cle compose.
    pub fn ij_nettes(_cle: &droit::natures::AuMois, jours: Jours, taux: MontantUnitaire, assiette_pas: Montant) -> LignePaie {
        let rubrique = droit::catalogue::RubriqueDuMois::IjssNettes.code();
        LignePaie::IndemniteJournaliere(IndemniteJournaliere { rubrique, jours, taux, assiette_pas: Some(assiette_pas) })
    }

    /// La ligne de `rubrique` qui paie `paiement` (sa quantite et son taux, dans l'unite de sa rubrique).
    pub fn composee(rubrique: CodeDeRubrique, paiement: PaiementDeLigne) -> LignePaie {
        LignePaie::Composee(Composee { rubrique, paiement, heures: None, precision: None, dates: Vec::new() })
    }

    /// LA LIGNE QUE LE MOTEUR COMPOSE D'UNE RUBRIQUE EN POURCENTAGE : sa base et son pourcentage (une indemnite de fin de
    /// contrat), dans la forme d'une ligne en pourcentage.
    pub fn en_pourcentage(rubrique: CodeDeRubrique, base: Montant, pourcentage: Taux) -> LignePaie {
        LignePaie::Pourcentage(EnPourcentage { rubrique, base: Some(base), taux_majoration: Some(pourcentage) })
    }

    /// Le code de sa rubrique au catalogue (`droit::catalogue::rubrique`).
    pub fn rubrique(&self) -> &CodeDeRubrique {
        match self {
            LignePaie::Unites(l) => &l.rubrique,
            LignePaie::UnitesEtHeures(l) => &l.rubrique,
            LignePaie::Frais(l) => &l.rubrique,
            LignePaie::Pourcentage(l) => &l.rubrique,
            LignePaie::Montant(l) => &l.rubrique,
            LignePaie::AvantageEnNature(l) => &l.rubrique,
            LignePaie::Composee(l) => &l.rubrique,
            LignePaie::RetenueArret(l) => &l.rubrique,
            LignePaie::IndemniteJournaliere(l) => &l.rubrique,
            LignePaie::Maintien(l) => &l.rubrique,
        }
    }

    /// SA QUANTITE, dans l'unite `unite` de sa rubrique : celle qu'elle dit (rangee dans cette unite a la porte,
    /// `LigneDonnee::ranger`), celle de ce qu'elle paie (une ligne composee, importee, une ligne au montant :
    /// `PaiementDeLigne::une_fois`), ses jours d'indemnites. `None` : la case n'est pas remplie, ou la ligne paie une
    /// part de sa base (une ligne en pourcentage, qui n'a pas de quantite).
    pub fn quantite(&self, unite: Unite) -> Option<QuantiteDeLigne> {
        match self {
            LignePaie::Unites(l) => l.quantite,
            LignePaie::UnitesEtHeures(l) => l.quantite,
            LignePaie::Frais(l) => l.quantite,
            LignePaie::Pourcentage(_) => None,
            LignePaie::Montant(_) | LignePaie::AvantageEnNature(_) => QuantiteDeLigne::une(unite),
            LignePaie::Composee(l) => l.paiement.quantite(),
            LignePaie::RetenueArret(l) => l.paiement.quantite(),
            LignePaie::IndemniteJournaliere(l) => Some(QuantiteDeLigne::Jours(l.jours)),
            LignePaie::Maintien(l) => l.paiement.quantite(),
        }
    }

    /// LES HEURES QU'ELLE FAIT COMPTER, dans l'unite `unite` de sa rubrique, a `par_unite` heures par unite
    /// (`QuantiteDeLigne::heures`) ; une ligne qui paie une part de sa base n'en compte aucune. Une case qui n'est pas
    /// remplie se refuse en se nommant.
    pub fn heures_comptees(&self, unite: Unite, par_unite: Option<HeuresParUnite>) -> R<Heures> {
        match self.quantite(unite) {
            Some(q) => Ok(q.heures(par_unite)),
            None => {
                self.base(unite).ok_or_else(|| socle::fichiers::absent("quantite"))?;
                sans_quantite(par_unite)
            }
        }
    }

    /// SA BASE EN EUROS, quand elle paie une part de sa base dans l'unite `unite` de sa rubrique (une ligne en
    /// pourcentage, une ligne au montant d'une rubrique en pourcentage, ce que le moteur ou l'import a pose ainsi) ;
    /// `None` : elle n'en dit pas, ou elle paie une quantite (`quantite`).
    pub fn base(&self, unite: Unite) -> Option<Montant> {
        match self {
            LignePaie::Pourcentage(l) => l.base,
            LignePaie::Montant(AuMontant { montant, .. }) | LignePaie::AvantageEnNature(AvantageEnNature { montant, .. }) => montant.filter(|_| unite == Unite::Pourcentage),
            LignePaie::Composee(Composee { paiement, .. }) | LignePaie::RetenueArret(RetenueArret { paiement, .. }) | LignePaie::Maintien(Maintien { paiement, .. }) => paiement.quantite_ou_base().err(),
            LignePaie::Unites(_) | LignePaie::UnitesEtHeures(_) | LignePaie::Frais(_) | LignePaie::IndemniteJournaliere(_) => None,
        }
    }

    /// SON TAUX (`TauxDeLigne`) : celui qu'elle dit, la part de sa base qu'une ligne en pourcentage paie, le montant d'une
    /// ligne au montant pour sa seule unite ; `None` : la case n'est pas remplie.
    pub fn taux(&self) -> Option<TauxDeLigne> {
        use TauxDeLigne::{ParUnite, PartDeLaBase};
        match self {
            LignePaie::Unites(l) => l.taux.map(ParUnite),
            LignePaie::UnitesEtHeures(l) => l.taux.map(ParUnite),
            LignePaie::Frais(l) => l.taux.map(ParUnite),
            LignePaie::Pourcentage(l) => l.taux_majoration.map(|t| PartDeLaBase(t.en_coefficient())),
            LignePaie::Montant(l) => l.montant.map(|m| ParUnite(m.pour_une_unite())),
            LignePaie::AvantageEnNature(l) => l.montant.map(|m| ParUnite(m.pour_une_unite())),
            LignePaie::Composee(l) => Some(l.paiement.taux()),
            LignePaie::RetenueArret(l) => Some(l.paiement.taux()),
            LignePaie::IndemniteJournaliere(l) => Some(ParUnite(l.taux)),
            LignePaie::Maintien(l) => Some(l.paiement.taux()),
        }
    }

    /// LE MONTANT UNITAIRE QU'ELLE DIT dans sa case (une ligne en unites ou de frais : ce que la visee ajuste), vide ou
    /// dit ; `None` : une ligne dont le taux n'est pas une case.
    pub fn taux_saisi(&self) -> Option<Option<MontantUnitaire>> {
        match self {
            LignePaie::Unites(l) => Some(l.taux),
            LignePaie::UnitesEtHeures(l) => Some(l.taux),
            LignePaie::Frais(l) => Some(l.taux),
            _ => None,
        }
    }

    /// Sa case du taux (`taux_saisi`), a changer.
    pub fn taux_saisi_mut(&mut self) -> Option<&mut Option<MontantUnitaire>> {
        match self {
            LignePaie::Unites(l) => Some(&mut l.taux),
            LignePaie::UnitesEtHeures(l) => Some(&mut l.taux),
            LignePaie::Frais(l) => Some(&mut l.taux),
            _ => None,
        }
    }

    /// CE QU'ELLE PAIE, dans l'unite `unite` de sa rubrique : ce que le moteur a compose (dans l'unite de sa rubrique), ce
    /// que l'import a pose en la rattachant, une ligne au montant son montant (`PaiementDeLigne::une_fois`), toute autre sa
    /// quantite (rangee dans l'unite de sa rubrique a la porte) a son taux (`PaiementDeLigne::de`). Une case qui n'est pas
    /// remplie se refuse en se nommant ; une quantite et un taux qui ne vont pas ensemble (un fichier ecrit a la main)
    /// aussi.
    pub fn paiement(&self, unite: Unite) -> R<PaiementDeLigne> {
        match self {
            LignePaie::Montant(AuMontant { montant, .. }) | LignePaie::AvantageEnNature(AvantageEnNature { montant, .. }) => {
                Ok(PaiementDeLigne::une_fois(unite, montant.ok_or_else(|| socle::fichiers::absent("montant"))?))
            }
            LignePaie::Composee(Composee { paiement, .. }) | LignePaie::RetenueArret(RetenueArret { paiement, .. }) | LignePaie::Maintien(Maintien { paiement, .. }) => Ok(*paiement),
            LignePaie::IndemniteJournaliere(l) => Ok(PaiementDeLigne::Jours(l.jours, l.taux)),
            LignePaie::Pourcentage(l) => {
                let base = l.base.ok_or_else(|| socle::fichiers::absent("base"))?;
                let taux = l.taux_majoration.ok_or_else(|| socle::fichiers::absent("taux"))?;
                Ok(PaiementDeLigne::Base(base, taux.en_coefficient()))
            }
            LignePaie::Unites(Unites { quantite, taux, .. }) | LignePaie::UnitesEtHeures(UnitesEtHeures { quantite, taux, .. }) | LignePaie::Frais(Frais { quantite, taux, .. }) => {
                let quantite = quantite.ok_or_else(|| socle::fichiers::absent("quantite"))?;
                let taux = taux.ok_or_else(|| socle::fichiers::absent("taux"))?;
                Ok(PaiementDeLigne::de(quantite, taux))
            }
        }
    }

    /// SES HEURES PAR UNITE, quand elle en dit (`QuantiteDeLigne::heures`) ; aucune pour une indemnite journaliere et le
    /// maintien, qui ne sont pas du travail.
    pub fn heures(&self) -> Option<HeuresParUnite> {
        match self {
            LignePaie::UnitesEtHeures(l) => l.heures,
            LignePaie::Composee(l) => l.heures,
            LignePaie::IndemniteJournaliere(_) | LignePaie::Maintien(_) => Some(HeuresParUnite::ZERO),
            LignePaie::Unites(_) | LignePaie::Frais(_) | LignePaie::Pourcentage(_) | LignePaie::Montant(_) | LignePaie::AvantageEnNature(_) | LignePaie::RetenueArret(_) => None,
        }
    }

    /// Ses jours du mois.
    pub fn dates(&self) -> &[DateDeLigne] {
        match self {
            LignePaie::Unites(l) => &l.dates,
            LignePaie::UnitesEtHeures(l) => &l.dates,
            LignePaie::Frais(l) => &l.dates,
            LignePaie::Composee(l) => &l.dates,
            _ => &[],
        }
    }

    /// Ses jours des mois suivants.
    pub fn planning(&self) -> &[DateDeLigne] {
        match self {
            LignePaie::Unites(l) => &l.planning,
            LignePaie::UnitesEtHeures(l) => &l.planning,
            LignePaie::Frais(l) => &l.planning,
            _ => &[],
        }
    }

    /// Ses jours du mois et ceux des mois suivants deviennent `dates` et `planning` ; une ligne qui n'a pas de jours n'en
    /// prend pas.
    pub fn avec_les_jours(&self, dates: Vec<DateDeLigne>, planning: Vec<DateDeLigne>) -> LignePaie {
        let mut l = self.clone();
        match &mut l {
            LignePaie::Unites(x) => (x.dates, x.planning) = (dates, planning),
            LignePaie::UnitesEtHeures(x) => (x.dates, x.planning) = (dates, planning),
            LignePaie::Frais(x) => (x.dates, x.planning) = (dates, planning),
            _ => {}
        }
        l
    }

    /// SES JOURS RANGES PAR MOIS, ceux de `mois` et d'avant en dates, ceux d'apres au planning, quelle que soit la cle qui
    /// les donne (la page les donne en une seule cle : `ranger`) ; une ligne sans jours reste ce qu'elle est.
    pub fn jours_ranges(&self, mois: Mois) -> LignePaie {
        let tous: Vec<DateDeLigne> = self.dates().iter().chain(self.planning()).cloned().collect();
        let (dates, planning): (Vec<DateDeLigne>, Vec<DateDeLigne>) = tous.into_iter().partition(|d| d.jour().mois_civil() <= mois);
        self.avec_les_jours(dates, planning)
    }

    /// SA QUANTITE ET CELLE DE CHACUN DE SES JOURS DU MOIS, TENUES ENSEMBLE (Leandre, 25/11/2026 : « si je mets 3 jours,
    /// 12 heures, il met 4 heures par jour » ; « Si je modifie, c'est la modification qui compte » ; un jour ajoute aux
    /// jours deja la arrive vide) : la ligne telle que l'appelant la donne, d'unite `unite`, ses jours deja ranges par mois.
    /// - `tapee` (l'appelant a dit sa quantite) : elle se repartit sur ses jours, le reste sur les premiers
    ///   (`QuantiteDeLigne::repartie` : 12 h sur 3 jours, 4 · 4 · 4) ; ce que disaient ses jours ne compte plus.
    /// - sinon, sa quantite est la somme des jours qui en disent une ; un jour qui n'en dit pas reste vide (4 · 3 · 3 puis un
    ///   jour ajoute : 4 · 3 · 3 · vide, 10 ; un jour retire ote la sienne).
    ///
    /// Elle s'ecrit sous sa seule forme : des jours dont les quantites sont la repartition de son total ne les disent pas
    /// (`quantites_des_jours` les relit telles), ils ne les disent que quand ils en different (un jour modifie, un jour
    /// vide). Une ligne qui ne se compte pas jour par jour, et un jour d'un mois suivant (il se dit dans la carte de son
    /// mois), ne portent aucune quantite de jour. Ses heures par unite (le forfait d'un cachet) ne changent pas.
    fn jours_tenus(self, tapee: bool, unite: Unite) -> R<LignePaie> {
        let planning: Vec<DateDeLigne> = self.planning().iter().map(|d| d.portant(None)).collect();
        if !QuantiteDeLigne::se_compte_par_jour(unite) || self.quantite_dite().is_none() {
            let dates: Vec<DateDeLigne> = self.dates().iter().map(|d| d.portant(None)).collect();
            return Ok(self.avec_les_jours(dates, planning));
        }
        let n = self.dates().len();
        let (quantite, parts): (Option<QuantiteDeLigne>, Vec<Option<QuantiteDeLigne>>) = if tapee {
            let q = self.quantite_dite().flatten();
            let parts = match q {
                Some(q) => q.repartie(n).into_iter().map(Some).collect(),
                None => vec![None; n],
            };
            (q, parts)
        } else {
            let parts: Vec<Option<QuantiteDeLigne>> = self.dates().iter().map(DateDeLigne::quantite).collect();
            let mut somme: Option<QuantiteDeLigne> = None;
            for q in parts.iter().flatten() {
                somme = Some(match somme {
                    None => *q,
                    Some(s) => s.plus(*q).ok_or_else(|| refus("deux jours d'une ligne en deux unités : le signaler au développeur"))?,
                });
            }
            (somme, parts)
        };
        let repartie: Option<Vec<Option<QuantiteDeLigne>>> = quantite.map(|q| q.repartie(n).into_iter().map(Some).collect());
        let parts = if repartie.as_ref() == Some(&parts) { vec![None; n] } else { parts };
        let dates: Vec<DateDeLigne> = self.dates().iter().zip(parts).map(|(d, q)| d.portant(q)).collect();
        Ok(self.avec_les_jours(dates, planning).avec_la_quantite(quantite))
    }

    /// LA QUANTITE DE CHACUN DE SES JOURS DU MOIS, dans l'ordre de ses dates, d'une ligne d'unite `unite` qui se compte
    /// jour par jour (`jours_tenus`) : quand un de ses jours dit sa quantite, celle que chacun dit (un jour qui n'en dit
    /// pas est vide) ; sinon, chaque jour porte sa part de la quantite de la ligne, le reste sur les premiers
    /// (`QuantiteDeLigne::repartie`) ; aucune sans quantite. `None` : une ligne qui ne se compte pas jour par jour.
    pub fn quantites_des_jours(&self, unite: Unite) -> Option<Vec<Option<QuantiteDeLigne>>> {
        if !QuantiteDeLigne::se_compte_par_jour(unite) {
            return None;
        }
        let dite = self.quantite_dite()?;
        let n = self.dates().len();
        let des_jours: Vec<Option<QuantiteDeLigne>> = self.dates().iter().map(DateDeLigne::quantite).collect();
        if des_jours.iter().any(Option::is_some) {
            return Some(des_jours);
        }
        Some(match dite {
            Some(q) => q.repartie(n).into_iter().map(Some).collect(),
            None => vec![None; n],
        })
    }

    /// CE QU'ELLE PAIE, quand ce qui le dit est rempli : quantite x taux (ou sa base x son pourcentage), tels que ses
    /// cases les disent dans l'unite `unite` de sa rubrique ; `None` : une case vide, ou une ligne au montant, dont la
    /// case est le montant.
    pub fn montant_dit(&self, unite: Unite) -> Option<Montant> {
        if matches!(self, LignePaie::Montant(_) | LignePaie::AvantageEnNature(_)) {
            return None;
        }
        match (self.quantite(unite), self.base(unite), self.taux()) {
            (Some(q), _, Some(TauxDeLigne::ParUnite(u))) => Some(PaiementDeLigne::de(q, u).montant()),
            (None, Some(b), Some(TauxDeLigne::PartDeLaBase(c))) => Some(PaiementDeLigne::Base(b, c).montant()),
            _ => None,
        }
    }

    /// La quantite qu'elle dit dans sa case, vide ou dite ; `None` : une ligne dont la quantite n'est pas une case.
    fn quantite_dite(&self) -> Option<Option<QuantiteDeLigne>> {
        match self {
            LignePaie::Unites(x) => Some(x.quantite),
            LignePaie::UnitesEtHeures(x) => Some(x.quantite),
            LignePaie::Frais(x) => Some(x.quantite),
            _ => None,
        }
    }

    /// La ligne, la case de sa quantite devenue `quantite` ; une ligne dont la quantite n'est pas une case reste telle.
    fn avec_la_quantite(self, quantite: Option<QuantiteDeLigne>) -> LignePaie {
        let mut l = self;
        match &mut l {
            LignePaie::Unites(x) => x.quantite = quantite,
            LignePaie::UnitesEtHeures(x) => x.quantite = quantite,
            LignePaie::Frais(x) => x.quantite = quantite,
            _ => {}
        }
        l
    }

    /// La ligne sans sa quantite (celle d'une ligne coupee a la fin d'un contrat, `crate::lignes_coupees_a_la_fin`) ; une
    /// ligne dont la quantite n'est pas une case telle quelle.
    pub fn sans_quantite(&self) -> LignePaie {
        let mut l = self.clone();
        match &mut l {
            LignePaie::Unites(x) => x.quantite = None,
            LignePaie::UnitesEtHeures(x) => x.quantite = None,
            LignePaie::Frais(x) => x.quantite = None,
            _ => {}
        }
        l
    }

    /// La ligne sans ce que le moteur pose pour l'ecran (`Marques`), telle qu'elle s'ecrit.
    pub fn sans_marques(&self) -> LignePaie {
        let mut l = self.clone();
        if let LignePaie::Frais(f) = &mut l {
            f.marques = Marques::default();
        }
        l
    }

    /// Ou se joue la ligne, quand elle le dit.
    pub fn lieu(&self) -> Option<&str> {
        match self {
            LignePaie::Unites(l) => l.lieu.as_deref(),
            LignePaie::UnitesEtHeures(l) => l.lieu.as_deref(),
            _ => None,
        }
    }

    /// Ce que la ligne dit d'elle-meme sur le bulletin (`calcul::LigneBrut::precision`).
    pub fn precision(&self) -> Option<&str> {
        match self {
            LignePaie::Composee(l) => l.precision.as_deref(),
            _ => None,
        }
    }

    /// Le genre d'un avantage en nature, s'il est dit (`droit::catalogue::code_54`).
    pub fn genre(&self) -> Option<AutreElementDeRevenuBrut> {
        match self {
            LignePaie::AvantageEnNature(l) => l.genre,
            _ => None,
        }
    }

    /// La ligne de frais, si c'en est une.
    pub fn frais(&self) -> Option<&Frais> {
        match self {
            LignePaie::Frais(l) => Some(l),
            _ => None,
        }
    }

    /// La ligne de frais, a changer.
    pub fn frais_mut(&mut self) -> Option<&mut Frais> {
        match self {
            LignePaie::Frais(l) => Some(l),
            _ => None,
        }
    }

    /// La case `case` de la ligne de frais est-elle cochee ?
    pub fn cochee(&self, case: CaseDeFrais) -> bool {
        self.frais().is_some_and(|f| f.cases.contains(&case))
    }

    /// Ce que le moteur a pose sur elle pour l'ecran (une ligne de frais).
    pub fn marques(&self) -> Option<&Marques> {
        self.frais().map(|f| &f.marques)
    }

    /// Une indemnite journaliere subrogee ?
    pub fn est_indemnite_journaliere(&self) -> bool {
        matches!(self, LignePaie::IndemniteJournaliere(_))
    }

    /// La retenue d'un arret de travail ?
    pub fn est_retenue_arret(&self) -> bool {
        matches!(self, LignePaie::RetenueArret(_))
    }

    /// L'assiette du prelevement a la source d'une indemnite journaliere nette reversee.
    pub fn assiette_pas(&self) -> Option<Montant> {
        match self {
            LignePaie::IndemniteJournaliere(l) => l.assiette_pas,
            _ => None,
        }
    }

    /// Ce qu'il faut pour reconstituer un maintien garanti en net.
    pub fn reconstitution(&self) -> Option<&crate::maintien::Reconstitution> {
        match self {
            LignePaie::Maintien(l) => l.reconstitution.as_deref(),
            _ => None,
        }
    }

    /// Ses jours du mois.
    pub fn jours(&self) -> Vec<Jour> {
        self.dates().iter().map(DateDeLigne::jour).collect()
    }

    /// Le premier jour de la ligne (ses dates, puis son planning) ; `None` : elle n'en a pas.
    pub fn premier_jour(&self) -> Option<Jour> {
        self.dates().first().or(self.planning().first()).map(DateDeLigne::jour)
    }

    /// Une ligne en pourcentage, sa base et son pourcentage positifs ; une autre passe.
    pub fn verifier_pourcentage(&self) -> R<()> {
        let LignePaie::Pourcentage(l) = self else { return Ok(()) };
        if l.base.is_some_and(|b| b < Montant::ZERO) || l.taux_majoration.is_some_and(|p| p < Taux::ZERO) {
            return Err(socle::refus!("Ligne << {} >> : base et pourcentage sont positifs.", l.rubrique));
        }
        Ok(())
    }

    /// SA FORME CANONIQUE, pour les empreintes et les oracles : ses nombres en texte normalise, sans ce que le moteur pose
    /// pour l'ecran (`Marques`).
    pub fn canon(&self) -> LignePaie {
        fn o<G: socle::grandeurs::Grandeur>(x: Option<G>) -> Option<G> {
            x.map(sans_zero)
        }
        fn qo(x: Option<QuantiteDeLigne>) -> Option<QuantiteDeLigne> {
            x.map(QuantiteDeLigne::sans_zero)
        }
        fn jo(x: Vec<DateDeLigne>) -> Vec<DateDeLigne> {
            x.iter().map(|d| d.portant(qo(d.quantite()))).collect()
        }
        match self.clone() {
            LignePaie::Unites(l) => LignePaie::Unites(Unites { quantite: qo(l.quantite), taux: o(l.taux), dates: jo(l.dates), ..l }),
            LignePaie::UnitesEtHeures(l) => {
                LignePaie::UnitesEtHeures(UnitesEtHeures { quantite: qo(l.quantite), taux: o(l.taux), heures: o(l.heures), dates: jo(l.dates), ..l })
            }
            LignePaie::Frais(l) => LignePaie::Frais(Frais { quantite: qo(l.quantite), taux: o(l.taux), marques: Marques::default(), dates: jo(l.dates), ..l }),
            LignePaie::Pourcentage(l) => LignePaie::Pourcentage(EnPourcentage { base: o(l.base), taux_majoration: o(l.taux_majoration), ..l }),
            LignePaie::Montant(l) => LignePaie::Montant(AuMontant { montant: o(l.montant), ..l }),
            LignePaie::AvantageEnNature(l) => LignePaie::AvantageEnNature(AvantageEnNature { montant: o(l.montant), ..l }),
            LignePaie::Composee(l) => LignePaie::Composee(Composee { paiement: l.paiement.sans_zero(), heures: o(l.heures), ..l }),
            LignePaie::RetenueArret(l) => LignePaie::RetenueArret(RetenueArret { paiement: l.paiement.sans_zero(), ..l }),
            LignePaie::IndemniteJournaliere(l) => {
                LignePaie::IndemniteJournaliere(IndemniteJournaliere { jours: sans_zero(l.jours), taux: sans_zero(l.taux), assiette_pas: o(l.assiette_pas), ..l })
            }
            LignePaie::Maintien(l) => LignePaie::Maintien(Maintien { paiement: l.paiement.sans_zero(), ..l }),
        }
    }
}

// ------------------------------------------------------------------ la porte

/// LES VALEURS QU'UNE LIGNE SAISIE PEUT DIRE, chacune une cle de sa forme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cle {
    Rubrique,
    Quantite,
    Taux,
    Heures,
    Base,
    TauxMajoration,
    Montant,
    Genre,
    Territoire,
    Cases,
    Lieu,
    Dates,
    Planning,
}

impl Cle {
    /// Une cle que la ligne peut omettre : absente, elle est vide (le lieu, les jours, le planning, et pour un repas ou
    /// une nuitee le territoire et les cases).
    pub fn facultative(self) -> bool {
        matches!(self, Cle::Territoire | Cle::Cases | Cle::Lieu | Cle::Dates | Cle::Planning)
    }

    /// Sa cle, telle que la ligne l'ecrit.
    pub fn nom(self) -> &'static str {
        match self {
            Cle::Rubrique => "rubrique",
            Cle::Quantite => "quantite",
            Cle::Taux => "taux",
            Cle::Heures => "heures",
            Cle::Base => "base",
            Cle::TauxMajoration => "taux_majoration",
            Cle::Montant => "montant",
            Cle::Genre => "genre",
            Cle::Territoire => "territoire",
            Cle::Cases => "cases",
            Cle::Lieu => "lieu",
            Cle::Dates => "dates",
            Cle::Planning => "planning",
        }
    }

    /// LE RANG DE SA COLONNE parmi celles des lignes d'une carte, quelle que soit la forme de chacune (Leandre,
    /// 25/11/2026 : « Les dates, ça doit être le premier truc », « C'est rubrique et après c'est direct la quantité »,
    /// puis « d'abord la quantité, ensuite quoi, et ensuite le taux », « mettre le lieu à la fin de la ligne ») : les
    /// jours, la quantite (ou la base), la rubrique, le taux (ou le pourcentage), les heures par unite, le lieu, puis ce
    /// que dit une ligne de frais ou un avantage, et le montant au bout.
    pub fn colonne(self) -> usize {
        match self {
            Cle::Dates => 0,
            Cle::Planning => 1,
            Cle::Quantite => 2,
            Cle::Base => 3,
            Cle::Rubrique => 4,
            Cle::Taux => 5,
            Cle::TauxMajoration => 6,
            Cle::Heures => 7,
            Cle::Lieu => 8,
            Cle::Territoire => 9,
            Cle::Cases => 10,
            Cle::Genre => 11,
            Cle::Montant => 12,
        }
    }

    /// SA PLACE SUR LA LIGNE : le territoire d'un repas ou d'une nuitee (hors de la metropole, une cle que seule une
    /// ligne de frais a limite porte, `FormeSaisie::Frais`) est sous « Avancé » ; les autres cles de sa forme sont de base.
    pub fn place(self) -> crate::carte::PlaceDeLaCase {
        use crate::carte::PlaceDeLaCase as P;
        match self {
            Cle::Territoire => P::Avance,
            Cle::Rubrique | Cle::Quantite | Cle::Taux | Cle::Heures | Cle::Base | Cle::TauxMajoration | Cle::Montant | Cle::Genre | Cle::Cases | Cle::Lieu | Cle::Dates | Cle::Planning => P::Base,
        }
    }
}

/// LA FORME QUE LE CATALOGUE DEMANDE D'UNE LIGNE DE SA RUBRIQUE : ce que dit sa `saisie`, et sa nature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormeSaisie {
    Unites,
    UnitesEtHeures,
    /// Un repas ou une nuitee : sa limite d'exoneration se choisit par le lieu et les cases (`frais_au_dela`).
    Frais,
    /// Les autres frais au forfait : ni lieu de limite ni case, aucune limite ne leur est attachee.
    FraisSansLimite,
    Pourcentage,
    Montant,
    AvantageEnNature,
}

impl FormeSaisie {
    /// LA FORME D'UNE LIGNE DE LA RUBRIQUE `r`, telle que son catalogue la declare (`saisie` ; `jours`, celle de la
    /// semaine, ce sont les dates de la ligne) ; `None` : une rubrique que le moteur pose (`saisie` vide), qui ne se saisit
    /// pas. Une `saisie` que le moteur ne sait pas ranger, ou qui ne va pas avec son unite (une base et un pourcentage
    /// pour une rubrique en pourcentage, une quantite et un taux pour les autres), est une rubrique mal decrite : refus.
    pub fn de(r: &droit::catalogue::Rubrique) -> R<Option<FormeSaisie>> {
        use droit::catalogue::CaseSaisie as C;
        let mut dit: Vec<C> = r.saisie.iter().copied().filter(|s| *s != C::Jours).collect();
        dit.sort_unstable();
        let frais = r.nature == droit::catalogue::NatureDeRubrique::Frais;
        let pourcentage = r.unite == Unite::Pourcentage;
        Ok(Some(match (dit.as_slice(), frais) {
            ([C::Quantite, C::Taux], false) if !pourcentage => FormeSaisie::Unites,
            ([C::Quantite, C::Taux], true) if !pourcentage && crate::a_une_limite(&r.code) => FormeSaisie::Frais,
            ([C::Quantite, C::Taux], true) if !pourcentage => FormeSaisie::FraisSansLimite,
            ([C::Quantite, C::Taux, C::Heures], false) if !pourcentage => FormeSaisie::UnitesEtHeures,
            ([C::Base, C::TauxMajoration], _) if pourcentage => FormeSaisie::Pourcentage,
            ([C::Montant], _) => FormeSaisie::Montant,
            ([C::Montant, C::Genre], _) => FormeSaisie::AvantageEnNature,
            ([], _) => return Ok(None),
            _ => {
                return Err(socle::refus!(
                    "La rubrique « {} » est mal décrite : sa saisie ({}) ne dit pas une forme de ligne. Le signaler au développeur.",
                    r.libelle,
                    r.saisie.iter().map(|s| s.cle()).collect::<Vec<_>>().join(", ")
                ))
            }
        }))
    }

    /// Les cles qu'une ligne de cette forme peut dire (la route etat les donne a la page, `rubriques[].cles`), dans l'ordre de la ligne : celles de `Cle::facultative` peuvent
    /// s'omettre, les autres se donnent (`null` tant que la case est vide). Une autre cle n'existe pas pour cette forme.
    pub fn cles(self) -> &'static [Cle] {
        use Cle::*;
        match self {
            FormeSaisie::Unites => &[Dates, Planning, Quantite, Rubrique, Taux, Lieu],
            FormeSaisie::UnitesEtHeures => &[Dates, Planning, Quantite, Rubrique, Taux, Heures, Lieu],
            // Une ligne de frais n'a pas de case de lieu (`gardees`) : le lieu d'un repas ou d'une nuitee hors de la
            // metropole, qui change sa limite, est son territoire.
            FormeSaisie::Frais => &[Dates, Planning, Quantite, Rubrique, Taux, Territoire, Cases],
            FormeSaisie::FraisSansLimite => &[Dates, Planning, Quantite, Rubrique, Taux],
            FormeSaisie::Pourcentage => &[Base, Rubrique, TauxMajoration],
            FormeSaisie::Montant => &[Rubrique, Montant],
            FormeSaisie::AvantageEnNature => &[Rubrique, Montant, Genre],
        }
    }

    /// LES CLES GARDEES de cette forme : une ligne peut les dire (facultatives) et les garde telles qu'elles sont
    /// ecrites, mais aucune case ne les montre ni ne les saisit (elles ne sont pas dans `cles`). Le lieu d'une ligne de
    /// frais : rien ne le lit, et une ligne ecrite le porte.
    pub fn gardees(self) -> &'static [Cle] {
        match self {
            FormeSaisie::Frais | FormeSaisie::FraisSansLimite => &[Cle::Lieu],
            FormeSaisie::Unites | FormeSaisie::UnitesEtHeures | FormeSaisie::Pourcentage | FormeSaisie::Montant | FormeSaisie::AvantageEnNature => &[],
        }
    }
}

/// UNE LIGNE TELLE QUE L'APPELANT LA DONNE (la page, un LLM : le changement de la case `lignes`, `carte::Changement`) :
/// sa rubrique et les valeurs qu'elle dit, chacune une `Cle` lue a son type (absente : `None` ; `null` : une case vide).
/// Une cle inconnue ou repetee refuse a la lecture ; les cles que la rubrique demande se verifient a la porte
/// (`LigneDonnee::ranger`), avec le catalogue.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LigneDonnee {
    pub rubrique: Option<CodeDeRubrique>,
    pub quantite: Option<Option<QuantiteDeLigne>>,
    pub taux: Option<Option<MontantUnitaire>>,
    pub heures: Option<Option<HeuresParUnite>>,
    pub base: Option<Option<Montant>>,
    pub taux_majoration: Option<Option<Taux>>,
    pub montant: Option<Option<Montant>>,
    pub genre: Option<Option<AutreElementDeRevenuBrut>>,
    pub territoire: Option<Option<LieuDeFrais>>,
    pub cases: Option<Vec<CaseDeFrais>>,
    pub lieu: Option<Option<String>>,
    pub dates: Option<Vec<DateDeLigne>>,
    pub planning: Option<Vec<DateDeLigne>>,
}

impl<'de> Deserialize<'de> for LigneDonnee {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<LigneDonnee, D::Error> {
        struct Visiteur;
        impl<'de> serde::de::Visitor<'de> for Visiteur {
            type Value = LigneDonnee;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("une ligne de paie : {\"rubrique\": …, et les valeurs que sa rubrique demande}")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<LigneDonnee, A::Error> {
                /// Une valeur posee une fois : une cle repetee refuse.
                fn une<T, E: serde::de::Error>(case: &mut Option<T>, v: T, cle: Cle) -> Result<(), E> {
                    if case.is_some() {
                        return Err(E::custom(format!("la clé {} est répétée", cle.nom())));
                    }
                    *case = Some(v);
                    Ok(())
                }
                /// Un nombre, ou `null`.
                struct Nombre<G>(Option<G>);
                impl<'de, G: socle::grandeurs::Grandeur> Deserialize<'de> for Nombre<G> {
                    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Nombre<G>, D::Error> {
                        socle::grandeurs::lu::nombre_ou_texte::option::deserialize(d).map(Nombre)
                    }
                }
                let mut l = LigneDonnee::default();
                while let Some(cle) = m.next_key::<Cle>()? {
                    match cle {
                        Cle::Rubrique => une(&mut l.rubrique, m.next_value::<CodeDeRubrique>()?, cle)?,
                        Cle::Quantite => une(&mut l.quantite, m.next_value::<Option<QuantiteDeLigne>>()?, cle)?,
                        Cle::Taux => une(&mut l.taux, m.next_value::<Nombre<_>>()?.0, cle)?,
                        Cle::Heures => une(&mut l.heures, m.next_value::<Nombre<_>>()?.0, cle)?,
                        Cle::Base => une(&mut l.base, m.next_value::<Nombre<_>>()?.0, cle)?,
                        Cle::TauxMajoration => une(&mut l.taux_majoration, m.next_value::<Nombre<_>>()?.0, cle)?,
                        Cle::Montant => une(&mut l.montant, m.next_value::<Nombre<_>>()?.0, cle)?,
                        Cle::Genre => une(&mut l.genre, m.next_value::<Option<AutreElementDeRevenuBrut>>()?, cle)?,
                        Cle::Territoire => une(&mut l.territoire, m.next_value::<Option<LieuDeFrais>>()?, cle)?,
                        Cle::Cases => une(&mut l.cases, m.next_value::<Vec<CaseDeFrais>>()?, cle)?,
                        Cle::Lieu => une(&mut l.lieu, m.next_value::<Option<String>>()?, cle)?,
                        Cle::Dates => une(&mut l.dates, m.next_value::<Vec<DateDeLigne>>()?, cle)?,
                        Cle::Planning => une(&mut l.planning, m.next_value::<Vec<DateDeLigne>>()?, cle)?,
                    }
                }
                Ok(l)
            }
        }
        d.deserialize_map(Visiteur)
    }
}

/// Elle s'ecrit comme elle se lit : chaque cle qu'elle dit, ses grandeurs en nombre (une route qui rend la charge
/// d'`enregistrer-saisie`, `carte::Changement::qui_font`).
impl Serialize for LigneDonnee {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        /// Un nombre, ou `null`.
        struct Nombre<'a, G>(&'a Option<G>);
        impl<G: socle::grandeurs::Grandeur> Serialize for Nombre<'_, G> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                socle::grandeurs::ecrit::nombre_json::option::serialize(self.0, s)
            }
        }
        let mut m = s.serialize_map(None)?;
        if let Some(x) = &self.rubrique {
            m.serialize_entry(Cle::Rubrique.nom(), x)?;
        }
        if let Some(x) = &self.quantite {
            m.serialize_entry(Cle::Quantite.nom(), x)?;
        }
        if let Some(x) = &self.taux {
            m.serialize_entry(Cle::Taux.nom(), &Nombre(x))?;
        }
        if let Some(x) = &self.heures {
            m.serialize_entry(Cle::Heures.nom(), &Nombre(x))?;
        }
        if let Some(x) = &self.base {
            m.serialize_entry(Cle::Base.nom(), &Nombre(x))?;
        }
        if let Some(x) = &self.taux_majoration {
            m.serialize_entry(Cle::TauxMajoration.nom(), &Nombre(x))?;
        }
        if let Some(x) = &self.montant {
            m.serialize_entry(Cle::Montant.nom(), &Nombre(x))?;
        }
        if let Some(x) = &self.genre {
            m.serialize_entry(Cle::Genre.nom(), x)?;
        }
        if let Some(x) = &self.territoire {
            m.serialize_entry(Cle::Territoire.nom(), x)?;
        }
        if let Some(x) = &self.cases {
            m.serialize_entry(Cle::Cases.nom(), x)?;
        }
        if let Some(x) = &self.lieu {
            m.serialize_entry(Cle::Lieu.nom(), x)?;
        }
        if let Some(x) = &self.dates {
            m.serialize_entry(Cle::Dates.nom(), x)?;
        }
        if let Some(x) = &self.planning {
            m.serialize_entry(Cle::Planning.nom(), x)?;
        }
        m.end()
    }
}

impl LignePaie {
    /// LA LIGNE TELLE QU'UN APPELANT LA DONNE (`LigneDonnee`), celle que sa porte range en elle (`LigneDonnee::ranger`) :
    /// chaque cle de sa forme, et une cle facultative (`Cle::facultative`) seulement quand elle dit quelque chose. Une
    /// ligne que le moteur pose (composee, retenue ou indemnite d'un arret, maintien) ne se donne pas : `None`.
    pub fn donnee(&self) -> Option<LigneDonnee> {
        let texte = |x: &Option<String>| x.clone().map(Some);
        let jours = |x: &Vec<DateDeLigne>| (!x.is_empty()).then(|| x.clone());
        Some(match self {
            LignePaie::Unites(l) => LigneDonnee {
                rubrique: Some(l.rubrique.clone()),
                quantite: Some(l.quantite),
                taux: Some(l.taux),
                lieu: texte(&l.lieu),
                dates: jours(&l.dates),
                planning: jours(&l.planning),
                ..LigneDonnee::default()
            },
            LignePaie::UnitesEtHeures(l) => LigneDonnee {
                rubrique: Some(l.rubrique.clone()),
                quantite: Some(l.quantite),
                taux: Some(l.taux),
                heures: Some(l.heures),
                lieu: texte(&l.lieu),
                dates: jours(&l.dates),
                planning: jours(&l.planning),
                ..LigneDonnee::default()
            },
            LignePaie::Frais(l) => LigneDonnee {
                rubrique: Some(l.rubrique.clone()),
                quantite: Some(l.quantite),
                taux: Some(l.taux),
                territoire: l.territoire.clone().map(Some),
                cases: (!l.cases.is_empty()).then(|| l.cases.clone()),
                lieu: texte(&l.lieu),
                dates: jours(&l.dates),
                planning: jours(&l.planning),
                ..LigneDonnee::default()
            },
            LignePaie::Pourcentage(l) => LigneDonnee { rubrique: Some(l.rubrique.clone()), base: Some(l.base), taux_majoration: Some(l.taux_majoration), ..LigneDonnee::default() },
            LignePaie::Montant(l) => LigneDonnee { rubrique: Some(l.rubrique.clone()), montant: Some(l.montant), ..LigneDonnee::default() },
            LignePaie::AvantageEnNature(l) => LigneDonnee { rubrique: Some(l.rubrique.clone()), montant: Some(l.montant), genre: Some(l.genre.clone()), ..LigneDonnee::default() },
            LignePaie::Composee(_) | LignePaie::RetenueArret(_) | LignePaie::IndemniteJournaliere(_) | LignePaie::Maintien(_) => return None,
        })
    }
}

impl LigneDonnee {
    /// Les cles qu'elle dit, dans l'ordre de `Cle`.
    fn dites(&self) -> Vec<Cle> {
        let mut out = Vec::new();
        let mut si = |dite: bool, c: Cle| {
            if dite {
                out.push(c);
            }
        };
        si(self.rubrique.is_some(), Cle::Rubrique);
        si(self.quantite.is_some(), Cle::Quantite);
        si(self.taux.is_some(), Cle::Taux);
        si(self.heures.is_some(), Cle::Heures);
        si(self.base.is_some(), Cle::Base);
        si(self.taux_majoration.is_some(), Cle::TauxMajoration);
        si(self.montant.is_some(), Cle::Montant);
        si(self.genre.is_some(), Cle::Genre);
        si(self.territoire.is_some(), Cle::Territoire);
        si(self.cases.is_some(), Cle::Cases);
        si(self.lieu.is_some(), Cle::Lieu);
        si(self.dates.is_some(), Cle::Dates);
        si(self.planning.is_some(), Cle::Planning);
        out
    }

    /// LE JOUR OU SA RUBRIQUE SE LIT : son premier jour (ses dates, puis son planning), sinon le dernier jour de `mois`,
    /// celui de la carte.
    pub fn jour(&self, mois: Mois) -> Jour {
        let premier = self.dates.iter().chain(self.planning.iter()).flatten().next();
        match premier {
            Some(d) => d.jour(),
            None => mois.dernier_jour(),
        }
    }

    /// LA LIGNE RANGEE DANS SA FORME (`FormeSaisie::de` de sa rubrique `r`, lue au catalogue a son jour, `jour`) : elle
    /// dit exactement les cles que cette forme demande, sinon le refus dit lesquelles donner et lesquelles retirer.
    pub fn ranger(self, r: &droit::catalogue::Rubrique) -> R<LignePaie> {
        let Some(forme) = FormeSaisie::de(r)? else {
            return Err(socle::refus!("« {} » ne se saisit pas : le moteur la pose lui-même. Retirez la ligne.", r.libelle));
        };
        let voulues = forme.cles();
        let dites = self.dites();
        // La quantite d'une ligne qui se compte jour par jour se tait quand on ne l'a pas tapee : elle est la somme de ses
        // jours (`LignePaie::jours_tenus`).
        let se_tait = |c: &Cle| *c == Cle::Quantite && QuantiteDeLigne::se_compte_par_jour(r.unite);
        let manquent: Vec<&str> = voulues.iter().filter(|c| !c.facultative() && !se_tait(c) && !dites.contains(c)).map(|c| c.nom()).collect();
        let de_trop: Vec<&str> = dites.iter().filter(|c| !voulues.contains(c) && !forme.gardees().contains(c)).map(|c| c.nom()).collect();
        if !manquent.is_empty() || !de_trop.is_empty() {
            let liste = |l: &[&str]| l.iter().map(|c| format!("« {} »", c)).collect::<Vec<_>>().join(", ");
            let mut quoi = Vec::new();
            if !manquent.is_empty() {
                quoi.push(format!("donnez {} (null tant que la case est vide)", liste(&manquent)));
            }
            if !de_trop.is_empty() {
                quoi.push(format!("retirez {}", liste(&de_trop)));
            }
            return Err(socle::refus!(
                "la ligne « {} » dit {} : {}.",
                r.libelle,
                liste(&voulues.iter().map(|c| c.nom()).collect::<Vec<_>>()),
                quoi.join(" ; ")
            ));
        }
        // Une case vide s'ecrit `null`, jamais un texte vide.
        if self.lieu.as_ref().is_some_and(|t| t.as_ref().is_some_and(|t| t.trim().is_empty())) {
            return Err(socle::refus!("la ligne « {} » : « {} » est vide ; écrivez null.", r.libelle, Cle::Lieu.nom()));
        }
        // Sa quantite se dit dans l'unite de sa rubrique : la porte la range la, une fois.
        if let Some(Some(q)) = self.quantite {
            if q.unite() != r.unite {
                let Some(cle) = QuantiteDeLigne::cle(r.unite) else {
                    return Err(socle::refus!("la ligne « {} » : sa rubrique est en pourcentage, elle dit sa base et son pourcentage.", r.libelle));
                };
                return Err(socle::refus!("la ligne « {} » : sa quantité se dit en {} : {{\"{}\": …}}.", r.libelle, cle, cle));
            }
        }
        // Sans quantite tapee, celle de chaque jour se dit dans l'unite de la ligne : elle fait sa quantite. Tapee, la
        // quantite de la ligne remplace ce que disaient ses jours (`LignePaie::jours_tenus`).
        let jours_dits = self.dates.iter().chain(self.planning.iter()).flatten().filter_map(DateDeLigne::quantite).filter(|_| self.quantite.is_none());
        for q in jours_dits {
            if q.unite() != r.unite {
                let cle = QuantiteDeLigne::cle(r.unite).ok_or_else(|| refus("une rubrique qui se compte par jour sans quantité : le signaler au développeur"))?;
                return Err(socle::refus!("la ligne « {} » : la quantité de chaque jour se dit en {} : {{\"{}\": …}}.", r.libelle, cle, cle));
            }
        }
        let rubrique = r.code.clone();
        // Chaque cle obligatoire est dite (la verification vient d'etre faite) ; une facultative absente est vide.
        fn dit<G>(x: Option<Option<G>>) -> Option<G> {
            x.flatten()
        }
        let jours = |x: Option<Vec<DateDeLigne>>| x.into_iter().flatten().collect::<Vec<_>>();
        let texte = |x: Option<Option<String>>| x.flatten();
        Ok(match forme {
            FormeSaisie::Unites => LignePaie::Unites(Unites {
                rubrique,
                quantite: dit(self.quantite),
                taux: dit(self.taux),
                lieu: texte(self.lieu),
                dates: jours(self.dates),
                planning: jours(self.planning),
            }),
            FormeSaisie::UnitesEtHeures => LignePaie::UnitesEtHeures(UnitesEtHeures {
                rubrique,
                quantite: dit(self.quantite),
                taux: dit(self.taux),
                heures: dit(self.heures),
                lieu: texte(self.lieu),
                dates: jours(self.dates),
                planning: jours(self.planning),
            }),
            FormeSaisie::Frais => {
                let mut cases = self.cases.into_iter().flatten().collect::<Vec<_>>();
                cases.sort();
                cases.dedup();
                LignePaie::Frais(Frais {
                    rubrique,
                    quantite: dit(self.quantite),
                    taux: dit(self.taux),
                    territoire: self.territoire.flatten(),
                    cases,
                    lieu: texte(self.lieu),
                    dates: jours(self.dates),
                    planning: jours(self.planning),
                    marques: Marques::default(),
                })
            }
            FormeSaisie::FraisSansLimite => LignePaie::Frais(Frais {
                rubrique,
                quantite: dit(self.quantite),
                taux: dit(self.taux),
                territoire: None,
                cases: Vec::new(),
                lieu: texte(self.lieu),
                dates: jours(self.dates),
                planning: jours(self.planning),
                marques: Marques::default(),
            }),
            FormeSaisie::Pourcentage => LignePaie::Pourcentage(EnPourcentage { rubrique, base: dit(self.base), taux_majoration: dit(self.taux_majoration) }),
            FormeSaisie::Montant => LignePaie::Montant(AuMontant { rubrique, montant: dit(self.montant) }),
            FormeSaisie::AvantageEnNature => LignePaie::AvantageEnNature(AvantageEnNature { rubrique, montant: dit(self.montant), genre: self.genre.flatten() }),
        })
    }
}

/// LES LIGNES QU'UN APPELANT DONNE A UNE CARTE DE `mois`, RANGEES (`LigneDonnee::ranger`) : chacune sa rubrique lue au
/// catalogue de la compagnie a son jour (`LigneDonnee::jour`), ses jours ranges par le mois de la carte
/// (`LignePaie::jours_ranges` : l'appelant peut tous les donner dans `dates`). Le premier refus nomme le salarie
/// (`qui`) et la ligne ; rien ne s'ecrit.
pub fn ranger(racine: &std::path::Path, mois: Mois, lignes: Vec<LigneDonnee>, qui: &str) -> R<Vec<LignePaie>> {
    let mut out = Vec::new();
    for (i, l) in lignes.into_iter().enumerate() {
        let dans_la_ligne = |m: Echec| m.reformuler(|p| format!("{} : sa ligne {} : {} Rien n'est écrit.", qui, i + 1, p));
        let Some(code) = l.rubrique.clone() else {
            return Err(dans_la_ligne(refus("elle dit sa rubrique (« rubrique »).")));
        };
        let r = droit::catalogue::rubrique_a_saisir(racine, &code, &l.jour(mois)).map_err(dans_la_ligne)?;
        // La quantite tapee se repartit ; sinon, elle est la somme de ses jours (`LignePaie::jours_tenus`).
        let tapee = l.quantite.is_some();
        let rangee = l.ranger(&r).map_err(dans_la_ligne)?.jours_ranges(mois);
        // Un jour d'un mois suivant dit sa quantite dans la carte de son mois, ou sa ligne vient (`lignes_du_planning`).
        if !tapee && rangee.planning().iter().any(|d| d.quantite().is_some()) {
            return Err(dans_la_ligne(socle::refus!("un jour d'un mois suivant ne dit pas sa quantité : elle se dit dans la carte de son mois.")));
        }
        out.push(rangee.jours_tenus(tapee, r.unite).map_err(dans_la_ligne)?);
    }
    Ok(out)
}
