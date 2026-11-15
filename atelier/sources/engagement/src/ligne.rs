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
use socle::forme::{Nombre, NombreLu};

use socle::date::Jour;
use socle::mois::Mois;
use socle::echec::{refus, Echec, R};
use socle::grandeurs::{courte, sans_zero, Coefficient, Heures, HeuresParUnite, Jours, Montant, MontantUnitaire, Taux};
use socle::num::Decimal;

/// UN NOMBRE TEL QU'IL EST SAISI : un nombre, ou un nombre ecrit en texte. La page envoie `""` pour une case vide ; la
/// forme canonique ecrit ses nombres en texte (`ContratEcrit`) ; un flottant garde son ecriture (`130.0`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Chiffre {
    Nombre(NombreLu),
    Texte(String),
}

impl Chiffre {
    /// Un decimal, en nombre a virgule (`NombreLu::de_decimal`).
    pub fn de_decimal(x: Decimal) -> Chiffre {
        Chiffre::Nombre(NombreLu::de_decimal(x))
    }

    /// Un entier, en nombre.
    pub fn entier(n: i64) -> Chiffre {
        Chiffre::Nombre(n.into())
    }

    /// Une case vide : un texte blanc.
    pub fn vide(&self) -> bool {
        matches!(self, Chiffre::Texte(s) if s.trim().is_empty())
    }

    /// Ni zero, ni un texte vide.
    pub fn vrai(&self) -> bool {
        match self {
            Chiffre::Nombre(n) => !n.est_nul(),
            Chiffre::Texte(s) => !s.is_empty(),
        }
    }

    /// Le decimal : un entier tel quel, un nombre a virgule par sa forme la plus courte, un texte lu.
    pub fn dec(&self) -> R<Decimal> {
        match self {
            Chiffre::Nombre(n) => n.decimal(),
            Chiffre::Texte(s) => socle::num::dec_texte(s),
        }
    }

    /// Le decimal dont un nombre a virgule garde son echelle (`130.0`, sa forme ecrite) : la lecture d'un nombre d'une
    /// chaine encore en decimal (le maintien de salaire) ; une grandeur se lit par `grandeur`.
    pub fn dec_avec_echelle(&self) -> R<Decimal> {
        match self {
            Chiffre::Nombre(n) => n.decimal_avec_echelle(),
            Chiffre::Texte(s) => socle::num::dec_texte(s),
        }
    }

    /// LA GRANDEUR QU'IL DIT, son echelle gardee : un nombre comme une donnee l'ecrit (`socle::grandeurs::lu::ecrite`), un
    /// texte comme un nombre ecrit en texte (`lu::en_texte`) ; la porte de sa lecture.
    pub fn grandeur<G: socle::grandeurs::Grandeur>(&self) -> R<G> {
        match self {
            Chiffre::Nombre(n) => socle::grandeurs::lu::ecrite(n),
            Chiffre::Texte(s) => socle::grandeurs::lu::en_texte(s),
        }
    }

    /// Le taux, la quantite ou la quotite qu'il dit, dans sa forme la plus courte (`socle::num::court`) : un nombre, ou
    /// un nombre ecrit en texte (sans virgule).
    pub fn court(&self) -> R<Decimal> {
        match self {
            Chiffre::Nombre(n) => n.decimal().map(socle::num::court),
            Chiffre::Texte(s) => socle::num::dec_texte(s).map(socle::num::court).map_err(|_| refus(socle::num::illisible(s))),
        }
    }

    /// Le nombre en texte, tel qu'un message le cite : un texte tel quel, un nombre sous sa forme ecrite.
    pub fn en_texte(&self) -> String {
        match self {
            Chiffre::Texte(s) => s.clone(),
            Chiffre::Nombre(n) => n.ecrit(),
        }
    }

    /// Sa forme canonique : un nombre en texte normalise (`socle::num::nombre`), un texte tel quel.
    pub fn canon(&self) -> R<Chiffre> {
        Ok(match self {
            Chiffre::Nombre(_) => Chiffre::Texte(socle::num::nombre(self.dec()?)),
            Chiffre::Texte(s) => Chiffre::Texte(s.clone()),
        })
    }
}

/// Une case dite et non vide (`Chiffre::vide`).
pub fn dite(c: &Option<Chiffre>) -> Option<&Chiffre> {
    c.as_ref().filter(|x| !x.vide())
}

/// UNE DATE D'UNE LIGNE : le jour nu (`"2026-10-02"`), ou detaille quand il dit ou il se joue
/// (`{"jour": "2026-10-02", "lieu": "Berlin"}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DateDeLigne {
    Nue(Jour),
    Detaillee(JourEtLieu),
}

/// Le jour d'une date detaillee, et son lieu : une date qui ne dit pas de lieu est nue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JourEtLieu {
    pub jour: Jour,
    pub lieu: String,
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
            DateDeLigne::Detaillee(JourEtLieu { lieu, .. }) if !lieu.is_empty() => Some(lieu.clone()),
            _ => None,
        }
    }

    /// Le jour `jour`, au lieu `lieu` s'il est dit.
    pub fn au(jour: Jour, lieu: String) -> DateDeLigne {
        if lieu.is_empty() {
            DateDeLigne::Nue(jour)
        } else {
            DateDeLigne::Detaillee(JourEtLieu { jour, lieu })
        }
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
    #[serde(with = "ecrit_option")]
    pub quantite: Option<Nombre>,
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
    #[serde(with = "ecrit_option")]
    pub quantite: Option<Nombre>,
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
    #[serde(with = "ecrit_option")]
    pub quantite: Option<Nombre>,
    #[serde(with = "texte_option")]
    pub taux: Option<MontantUnitaire>,
    pub territoire: Option<LieuDeFrais>,
    pub cases: Vec<CaseDeFrais>,
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

/// UNE LIGNE QUE LE MOTEUR COMPOSE : quantite x taux, ses heures par unite quand elle en dit, ce qu'elle dit d'elle-meme
/// sur le bulletin (`calcul::LigneBrut::precision`), ses jours.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composee {
    pub rubrique: CodeDeRubrique,
    #[serde(with = "ecrit")]
    pub quantite: Nombre,
    #[serde(with = "texte")]
    pub taux: MontantUnitaire,
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
    #[serde(with = "ecrit")]
    pub quantite: Nombre,
    #[serde(with = "texte")]
    pub taux: MontantUnitaire,
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
    #[serde(with = "ecrit")]
    pub quantite: Nombre,
    #[serde(with = "texte")]
    pub taux: MontantUnitaire,
    #[serde(skip)]
    pub reconstitution: Option<Box<crate::maintien::Reconstitution>>,
}

/// LA LIGNE QU'UN BULLETIN IMPORTE IMPRIME (`importe::LigneImprimee`), rattachee par l'import a une rubrique : son
/// montant, tel qu'il le lit, qui ne se recompose jamais (l'import lit au mieux : ce que le bulletin dit se garde) ; sa
/// quantite, son taux et ses heures, s'il les lit, pour l'affichage. Une ligne imprimee qu'aucune rubrique ne rattache,
/// ou sans montant, garde son libelle dans la paie importee (`importe::remuneration_lue`) : elle n'est pas une ligne de
/// paie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Importee {
    pub rubrique: CodeDeRubrique,
    #[serde(with = "ecrit_option")]
    pub quantite: Option<Nombre>,
    #[serde(with = "texte_option")]
    pub taux: Option<MontantUnitaire>,
    #[serde(with = "texte_option")]
    pub heures: Option<HeuresParUnite>,
    #[serde(with = "texte")]
    pub montant: Montant,
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
    Importee(Importee),
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

/// La quantite d'une ligne, ecrite sans son unite (sa rubrique la dit, `QuantiteDeLigne`) : en texte, son echelle gardee,
/// comme `texte` ; lue d'un nombre ou d'un texte.
mod ecrit {
    use socle::forme::Nombre;

    pub fn serialize<S: serde::Serializer>(n: &Nombre, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&n.0.to_string())
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Nombre, D::Error> {
        socle::forme::decimal(d).map(Nombre)
    }
}

/// `ecrit`, une case : `null` tant qu'elle n'est pas remplie.
mod ecrit_option {
    use socle::forme::Nombre;

    pub fn serialize<S: serde::Serializer>(n: &Option<Nombre>, s: S) -> Result<S::Ok, S::Error> {
        match n {
            Some(n) => super::ecrit::serialize(n, s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Nombre>, D::Error> {
        socle::forme::decimal_opt(d).map(|o| o.map(Nombre))
    }
}

/// LA QUANTITE D'UNE LIGNE DE PAIE, dans l'unite que sa rubrique dit (`droit::catalogue::Unite`) : des heures, des
/// cachets, des jours, des services, des semaines, la part du mois payee (1 pour un mois entier), le compte d'un forfait
/// (3 repas), ou la base en euros d'une ligne en pourcentage. Elle se construit quand la rubrique est lue (`de_l_unite`) :
/// une ligne ecrite ne garde que son nombre (`LignePaie::quantite_ecrite`), et le reprend dans son unite a la relecture
/// (`LignePaie::quantite`, `calcul::LigneBrut::de_la_forme`). Quantite x montant unitaire = montant (`fois`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuantiteDeLigne {
    Heures(Heures),
    Cachets(socle::grandeurs::Cachets),
    Jours(Jours),
    Services(socle::grandeurs::Services),
    Semaines(socle::grandeurs::Semaines),
    PartDuMois(socle::grandeurs::Coefficient),
    Unites(socle::grandeurs::Unites),
    Base(Montant),
}

impl QuantiteDeLigne {
    /// Une fois : la quantite d'une ligne qui se paie en une fois (une prime, une indemnite), son montant pour une unite.
    pub const UNE_FOIS: QuantiteDeLigne = QuantiteDeLigne::Unites(socle::grandeurs::Unites::UNE);

    /// Le nombre `d` d'une ligne dans l'unite `unite` de sa rubrique.
    pub fn de_l_unite(unite: Unite, d: Decimal) -> QuantiteDeLigne {
        match unite {
            Unite::Heure => QuantiteDeLigne::Heures(Heures::du_decimal(d)),
            Unite::Cachet => QuantiteDeLigne::Cachets(socle::grandeurs::Cachets::du_decimal(d)),
            Unite::Jour => QuantiteDeLigne::Jours(Jours::du_decimal(d)),
            Unite::Service => QuantiteDeLigne::Services(socle::grandeurs::Services::du_decimal(d)),
            Unite::Semaine => QuantiteDeLigne::Semaines(socle::grandeurs::Semaines::du_decimal(d)),
            Unite::Mois => QuantiteDeLigne::PartDuMois(socle::grandeurs::Coefficient::du_decimal(d)),
            Unite::Forfait => QuantiteDeLigne::Unites(socle::grandeurs::Unites::du_decimal(d)),
            Unite::Pourcentage => QuantiteDeLigne::Base(Montant::du_decimal(d)),
        }
    }

    /// Son nombre, a la porte qui l'exige (une ecriture, une phrase, `droit::catalogue::Rubrique::heures`).
    pub fn decimal(self) -> Decimal {
        match self {
            QuantiteDeLigne::Heures(q) => q.decimal(),
            QuantiteDeLigne::Cachets(q) => q.decimal(),
            QuantiteDeLigne::Jours(q) => q.decimal(),
            QuantiteDeLigne::Services(q) => q.decimal(),
            QuantiteDeLigne::Semaines(q) => q.decimal(),
            QuantiteDeLigne::PartDuMois(q) => q.decimal(),
            QuantiteDeLigne::Unites(q) => q.decimal(),
            QuantiteDeLigne::Base(q) => q.decimal(),
        }
    }

    /// Est-elle nulle ?
    pub fn est_nul(self) -> bool {
        self.decimal().is_zero()
    }

    /// Dans sa forme la plus courte, une decimale au moins (`socle::grandeurs::courte`), comme la ligne l'ecrit.
    pub fn courte(self) -> QuantiteDeLigne {
        use QuantiteDeLigne as Q;
        match self {
            Q::Heures(q) => Q::Heures(courte(q)),
            Q::Cachets(q) => Q::Cachets(courte(q)),
            Q::Jours(q) => Q::Jours(courte(q)),
            Q::Services(q) => Q::Services(courte(q)),
            Q::Semaines(q) => Q::Semaines(courte(q)),
            Q::PartDuMois(q) => Q::PartDuMois(courte(q)),
            Q::Unites(q) => Q::Unites(courte(q)),
            Q::Base(q) => Q::Base(courte(q)),
        }
    }

    /// LES HEURES QU'ELLE FAIT COMPTER, a `par_unite` heures par unite (`Rubrique::heures_par_unite` : le forfait de sa
    /// rubrique, sinon celles que la ligne dit) : quantite x heures par unite, au centieme d'heure comme la DSN l'ecrit
    /// (14 services de 4 h : 56 h ; un mois de permanent : la part du mois payee x la quotite du contrat). Sans heures par
    /// unite, une quantite d'heures compte ses heures, une autre n'en compte aucune. Le nombre rendu est dans sa forme la
    /// plus courte (`courte`). Une ligne en pourcentage n'a pas d'heures par unite : refus.
    pub fn heures(self, par_unite: Option<HeuresParUnite>) -> R<Heures> {
        use QuantiteDeLigne as Q;
        Ok(match (par_unite, self) {
            (Some(p), q) => match q.fois(p) {
                Some(h) => courte(h.au_centieme()),
                None => return Err(socle::refus!("une ligne en pourcentage ne compte pas d'heures par unité : le signaler au développeur")),
            },
            (None, Q::Heures(q)) => courte(q),
            (None, _) => courte(Heures::ZERO),
        })
    }

    /// QUANTITE x HEURES PAR UNITE, exactes (14 services de 4 h : 56 h) ; `None` : une ligne en pourcentage, dont la
    /// quantite est une base en euros, n'a pas d'heures par unite.
    pub fn fois(self, par_unite: HeuresParUnite) -> Option<Heures> {
        use QuantiteDeLigne as Q;
        match self {
            Q::Heures(q) => Some(q * par_unite),
            Q::Cachets(q) => Some(q * par_unite),
            Q::Jours(q) => Some(q * par_unite),
            Q::Services(q) => Some(q * par_unite),
            Q::Semaines(q) => Some(q * par_unite),
            Q::PartDuMois(q) => Some(q * par_unite),
            Q::Unites(q) => Some(q * par_unite),
            Q::Base(_) => None,
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
            (Q::Base(a), Q::Base(b)) => Q::Base(a + b),
            _ => return None,
        })
    }
}

/// LE TAUX D'UNE LIGNE DE PAIE : ce que paie une unite de sa quantite (un montant unitaire : par heure, par cachet, le
/// montant d'une ligne au montant pour sa seule unite), ou, pour une ligne en pourcentage, la part de sa base qu'elle
/// paie (son pourcentage sur cent, un coefficient). Une ligne ecrite ne garde que son nombre, et reprend son taux dans
/// l'unite de sa rubrique a la relecture (`de_l_unite`). Quantite x taux = montant (`QuantiteDeLigne::fois`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TauxDeLigne {
    ParUnite(MontantUnitaire),
    PartDeLaBase(Coefficient),
}

impl TauxDeLigne {
    /// Son nombre, a la porte qui l'exige (une ecriture, une phrase).
    pub fn decimal(self) -> Decimal {
        match self {
            TauxDeLigne::ParUnite(u) => u.decimal(),
            TauxDeLigne::PartDeLaBase(c) => c.decimal(),
        }
    }

    /// Est-il nul ?
    pub fn est_nul(self) -> bool {
        self.decimal().is_zero()
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

/// CE QUE PAIE UNE LIGNE DE PAIE : sa quantite et son taux, qui vont ensemble. L'unite de sa rubrique les dit, a ce seul
/// endroit (`de_l_unite`) : une quantite a un montant unitaire, ou, pour une rubrique en pourcentage, une base en euros
/// a la part qu'elle en paie. Quantite x taux = montant (`montant`), toujours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaiementDeLigne {
    Heures(Heures, MontantUnitaire),
    Cachets(socle::grandeurs::Cachets, MontantUnitaire),
    Jours(Jours, MontantUnitaire),
    Services(socle::grandeurs::Services, MontantUnitaire),
    Semaines(socle::grandeurs::Semaines, MontantUnitaire),
    PartDuMois(Coefficient, MontantUnitaire),
    Unites(socle::grandeurs::Unites, MontantUnitaire),
    Base(Montant, Coefficient),
}

impl PaiementDeLigne {
    /// La quantite `q` et le taux `t` d'une ligne, dans l'unite `unite` de sa rubrique.
    pub fn de_l_unite(unite: Unite, q: Decimal, t: Decimal) -> PaiementDeLigne {
        let u = MontantUnitaire::du_decimal(t);
        match unite {
            Unite::Heure => PaiementDeLigne::Heures(Heures::du_decimal(q), u),
            Unite::Cachet => PaiementDeLigne::Cachets(socle::grandeurs::Cachets::du_decimal(q), u),
            Unite::Jour => PaiementDeLigne::Jours(Jours::du_decimal(q), u),
            Unite::Service => PaiementDeLigne::Services(socle::grandeurs::Services::du_decimal(q), u),
            Unite::Semaine => PaiementDeLigne::Semaines(socle::grandeurs::Semaines::du_decimal(q), u),
            Unite::Mois => PaiementDeLigne::PartDuMois(Coefficient::du_decimal(q), u),
            Unite::Forfait => PaiementDeLigne::Unites(socle::grandeurs::Unites::du_decimal(q), u),
            Unite::Pourcentage => PaiementDeLigne::Base(Montant::du_decimal(q), Coefficient::du_decimal(t)),
        }
    }

    /// Sa quantite.
    pub fn quantite(self) -> QuantiteDeLigne {
        match self {
            PaiementDeLigne::Heures(q, _) => QuantiteDeLigne::Heures(q),
            PaiementDeLigne::Cachets(q, _) => QuantiteDeLigne::Cachets(q),
            PaiementDeLigne::Jours(q, _) => QuantiteDeLigne::Jours(q),
            PaiementDeLigne::Services(q, _) => QuantiteDeLigne::Services(q),
            PaiementDeLigne::Semaines(q, _) => QuantiteDeLigne::Semaines(q),
            PaiementDeLigne::PartDuMois(q, _) => QuantiteDeLigne::PartDuMois(q),
            PaiementDeLigne::Unites(q, _) => QuantiteDeLigne::Unites(q),
            PaiementDeLigne::Base(b, _) => QuantiteDeLigne::Base(b),
        }
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
    pub fn du_mois(_cle: &droit::natures::AuMois, rubrique: droit::catalogue::RubriqueDuMois, quantite: QuantiteDeLigne, taux: MontantUnitaire) -> LignePaie {
        LignePaie::composee(rubrique.code(), quantite, taux)
    }

    /// LE REVERSEMENT DES INDEMNITES JOURNALIERES NETTES d'une compagnie subrogee (`maintien::lignes`) : une rubrique de la
    /// piece d'un salarie au mois (`RubriqueDuMois::IjssNettes`), que seul le calcul qui tient sa cle compose.
    pub fn ij_nettes(_cle: &droit::natures::AuMois, jours: Jours, taux: MontantUnitaire, assiette_pas: Montant) -> LignePaie {
        let rubrique = droit::catalogue::RubriqueDuMois::IjssNettes.code();
        LignePaie::IndemniteJournaliere(IndemniteJournaliere { rubrique, jours, taux, assiette_pas: Some(assiette_pas) })
    }

    /// La ligne de `rubrique`, `quantite` x `taux` : elle n'ecrit que le nombre de sa quantite (sa rubrique en dit l'unite
    /// a la relecture, `QuantiteDeLigne::de_l_unite`).
    pub fn composee(rubrique: CodeDeRubrique, quantite: QuantiteDeLigne, taux: MontantUnitaire) -> LignePaie {
        LignePaie::Composee(Composee { rubrique, quantite: Nombre(quantite.decimal()), taux, heures: None, precision: None, dates: Vec::new() })
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
            LignePaie::Importee(l) => &l.rubrique,
        }
    }

    /// SA QUANTITE TELLE QU'ELLE L'ECRIT, sans son unite (que sa rubrique dit : `quantite`) : celle qu'elle dit, sa base en
    /// pourcentage, une fois un montant, ses jours d'indemnites ; ce qu'une phrase cite, ou ce que deux lignes de la meme
    /// rubrique comparent. `None` : la case n'est pas remplie.
    pub fn quantite_ecrite(&self) -> Option<Decimal> {
        match self {
            LignePaie::Unites(l) => l.quantite.map(|n| n.0),
            LignePaie::UnitesEtHeures(l) => l.quantite.map(|n| n.0),
            LignePaie::Frais(l) => l.quantite.map(|n| n.0),
            LignePaie::Pourcentage(l) => l.base.map(Montant::decimal),
            LignePaie::Montant(_) | LignePaie::AvantageEnNature(_) => Some(Decimal::ONE),
            LignePaie::Composee(l) => Some(l.quantite.0),
            LignePaie::RetenueArret(l) => Some(l.quantite.0),
            LignePaie::IndemniteJournaliere(l) => Some(l.jours.decimal()),
            LignePaie::Maintien(l) => Some(l.quantite.0),
            LignePaie::Importee(l) => l.quantite.map(|n| n.0),
        }
    }

    /// SA QUANTITE, dans l'unite `unite` de sa rubrique (`QuantiteDeLigne::de_l_unite`) ; `None` : la case n'est pas
    /// remplie.
    pub fn quantite(&self, unite: Unite) -> Option<QuantiteDeLigne> {
        self.quantite_ecrite().map(|d| QuantiteDeLigne::de_l_unite(unite, d))
    }

    /// SON TAUX (`TauxDeLigne`) : celui qu'elle dit, la part de sa base qu'une ligne en pourcentage paie, le montant d'une
    /// ligne au montant pour sa seule unite ; `None` : la case n'est pas remplie.
    pub fn taux(&self) -> Option<TauxDeLigne> {
        use TauxDeLigne::{ParUnite, PartDeLaBase};
        match self {
            LignePaie::Unites(l) => l.taux.map(ParUnite),
            LignePaie::UnitesEtHeures(l) => l.taux.map(ParUnite),
            LignePaie::Frais(l) => l.taux.map(ParUnite),
            LignePaie::Pourcentage(l) => l.taux_majoration.map(|t| PartDeLaBase(sur_cent(t))),
            LignePaie::Montant(l) => l.montant.map(|m| ParUnite(MontantUnitaire::du_decimal(m.decimal()))),
            LignePaie::AvantageEnNature(l) => l.montant.map(|m| ParUnite(MontantUnitaire::du_decimal(m.decimal()))),
            LignePaie::Composee(l) => Some(ParUnite(l.taux)),
            LignePaie::RetenueArret(l) => Some(ParUnite(l.taux)),
            LignePaie::IndemniteJournaliere(l) => Some(ParUnite(l.taux)),
            LignePaie::Maintien(l) => Some(ParUnite(l.taux)),
            LignePaie::Importee(l) => l.taux.map(ParUnite),
        }
    }

    /// CE QU'ELLE PAIE, sa quantite et son taux dans l'unite `unite` de sa rubrique (`PaiementDeLigne::de_l_unite`) ; une
    /// case qui n'est pas remplie se refuse en se nommant.
    pub fn paiement(&self, unite: Unite) -> R<PaiementDeLigne> {
        let quantite = self.quantite_ecrite().ok_or_else(|| socle::fichiers::absent("quantite"))?;
        let taux = self.taux().ok_or_else(|| socle::fichiers::absent("taux"))?;
        Ok(PaiementDeLigne::de_l_unite(unite, quantite, taux.decimal()))
    }

    /// LE MONTANT QU'UN BULLETIN IMPORTE LUI DONNE (une ligne importee), qui ne se recompose pas ; `None` pour toute
    /// autre ligne, dont le montant est ce qu'elle paie (`paiement`).
    pub fn montant_lu(&self) -> Option<Montant> {
        match self {
            LignePaie::Importee(l) => Some(l.montant),
            _ => None,
        }
    }

    /// Le taux de la ligne devient `taux` (le tarif de la convention, la limite d'un frais) ; une ligne dont le taux n'est
    /// pas une case n'en change pas : refus.
    pub fn poser_taux(&mut self, taux: MontantUnitaire) -> R<()> {
        match self {
            LignePaie::Unites(l) => l.taux = Some(taux),
            LignePaie::UnitesEtHeures(l) => l.taux = Some(taux),
            LignePaie::Frais(l) => l.taux = Some(taux),
            LignePaie::Composee(l) => l.taux = taux,
            _ => return Err(socle::refus!("la ligne {} ne prend pas de taux : le signaler au développeur", self.rubrique())),
        }
        Ok(())
    }

    /// SES HEURES PAR UNITE, quand elle en dit (`QuantiteDeLigne::heures`) ; aucune pour une indemnite journaliere et le
    /// maintien, qui ne sont pas du travail.
    pub fn heures(&self) -> Option<HeuresParUnite> {
        match self {
            LignePaie::UnitesEtHeures(l) => l.heures,
            LignePaie::Composee(l) => l.heures,
            LignePaie::Importee(l) => l.heures,
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
            LignePaie::Frais(l) => l.lieu.as_deref(),
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
        if l.base.is_some_and(|b| b.decimal() < Decimal::ZERO) || l.taux_majoration.is_some_and(|p| p.decimal() < Decimal::ZERO) {
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
        // Une quantite, sans zero inutile, comme `sans_zero` une grandeur.
        fn q(x: Nombre) -> Nombre {
            Nombre(x.0.normalize())
        }
        fn qo(x: Option<Nombre>) -> Option<Nombre> {
            x.map(q)
        }
        match self.clone() {
            LignePaie::Unites(l) => LignePaie::Unites(Unites { quantite: qo(l.quantite), taux: o(l.taux), ..l }),
            LignePaie::UnitesEtHeures(l) => LignePaie::UnitesEtHeures(UnitesEtHeures { quantite: qo(l.quantite), taux: o(l.taux), heures: o(l.heures), ..l }),
            LignePaie::Frais(l) => LignePaie::Frais(Frais { quantite: qo(l.quantite), taux: o(l.taux), marques: Marques::default(), ..l }),
            LignePaie::Pourcentage(l) => LignePaie::Pourcentage(EnPourcentage { base: o(l.base), taux_majoration: o(l.taux_majoration), ..l }),
            LignePaie::Montant(l) => LignePaie::Montant(AuMontant { montant: o(l.montant), ..l }),
            LignePaie::AvantageEnNature(l) => LignePaie::AvantageEnNature(AvantageEnNature { montant: o(l.montant), ..l }),
            LignePaie::Composee(l) => LignePaie::Composee(Composee { quantite: q(l.quantite), taux: sans_zero(l.taux), heures: o(l.heures), ..l }),
            LignePaie::RetenueArret(l) => LignePaie::RetenueArret(RetenueArret { quantite: q(l.quantite), taux: sans_zero(l.taux), ..l }),
            LignePaie::IndemniteJournaliere(l) => {
                LignePaie::IndemniteJournaliere(IndemniteJournaliere { jours: sans_zero(l.jours), taux: sans_zero(l.taux), assiette_pas: o(l.assiette_pas), ..l })
            }
            LignePaie::Maintien(l) => LignePaie::Maintien(Maintien { quantite: q(l.quantite), taux: sans_zero(l.taux), ..l }),
            LignePaie::Importee(l) => LignePaie::Importee(Importee { quantite: qo(l.quantite), taux: o(l.taux), heures: o(l.heures), montant: sans_zero(l.montant), ..l }),
        }
    }
}

/// UN POURCENTAGE SUR CENT, la part de la base : le quotient exact, a l'exposant ideal, qui garde au moins l'echelle du
/// pourcentage (`12.5` -> `0.125`, `30.0` -> `0.30`).
fn sur_cent(pct: Taux) -> Coefficient {
    let pct = pct.decimal();
    let mut taux = socle::num::div(pct, Decimal::ONE_HUNDRED).normalize();
    if taux.scale() < pct.scale() {
        taux.rescale(pct.scale());
    }
    Coefficient::du_decimal(taux)
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

    /// Les noms des cles qu'une ligne de cette forme dit (la route etat les donne a la page, `rubriques[].cles`).
    pub fn noms(self) -> Vec<&'static str> {
        self.cles().iter().map(|c| c.nom()).collect()
    }

    /// Les cles qu'une ligne de cette forme peut dire, dans l'ordre de la ligne : celles de `Cle::facultative` peuvent
    /// s'omettre, les autres se donnent (`null` tant que la case est vide). Une autre cle n'existe pas pour cette forme.
    pub fn cles(self) -> &'static [Cle] {
        use Cle::*;
        match self {
            FormeSaisie::Unites => &[Rubrique, Quantite, Taux, Lieu, Dates, Planning],
            FormeSaisie::UnitesEtHeures => &[Rubrique, Quantite, Taux, Heures, Lieu, Dates, Planning],
            FormeSaisie::Frais => &[Rubrique, Quantite, Taux, Territoire, Cases, Lieu, Dates, Planning],
            FormeSaisie::FraisSansLimite => &[Rubrique, Quantite, Taux, Lieu, Dates, Planning],
            FormeSaisie::Pourcentage => &[Rubrique, Base, TauxMajoration],
            FormeSaisie::Montant => &[Rubrique, Montant],
            FormeSaisie::AvantageEnNature => &[Rubrique, Montant, Genre],
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
    pub quantite: Option<Option<Nombre>>,
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
                /// Une quantite, sans son unite (`ecrit_option`), ou `null`.
                struct Quantite(Option<socle::forme::Nombre>);
                impl<'de> Deserialize<'de> for Quantite {
                    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Quantite, D::Error> {
                        ecrit_option::deserialize(d).map(Quantite)
                    }
                }
                let mut l = LigneDonnee::default();
                while let Some(cle) = m.next_key::<Cle>()? {
                    match cle {
                        Cle::Rubrique => une(&mut l.rubrique, m.next_value::<CodeDeRubrique>()?, cle)?,
                        Cle::Quantite => une(&mut l.quantite, m.next_value::<Quantite>()?.0, cle)?,
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
        let manquent: Vec<&str> = voulues.iter().filter(|c| !c.facultative() && !dites.contains(c)).map(|c| c.nom()).collect();
        let de_trop: Vec<&str> = dites.iter().filter(|c| !voulues.contains(c)).map(|c| c.nom()).collect();
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
/// catalogue de la compagnie a son jour (`LigneDonnee::jour`). Le premier refus nomme le salarie (`qui`) et la ligne ;
/// rien ne s'ecrit.
pub fn ranger(racine: &std::path::Path, mois: Mois, lignes: Vec<LigneDonnee>, qui: &str) -> R<Vec<LignePaie>> {
    let mut out = Vec::new();
    for (i, l) in lignes.into_iter().enumerate() {
        let dans_la_ligne = |m: Echec| m.reformuler(|p| format!("{} : sa ligne {} : {} Rien n'est écrit.", qui, i + 1, p));
        let Some(code) = l.rubrique.clone() else {
            return Err(dans_la_ligne(refus("elle dit sa rubrique (« rubrique »).")));
        };
        let r = droit::catalogue::rubrique_a_saisir(racine, &code, &l.jour(mois)).map_err(dans_la_ligne)?;
        out.push(l.ranger(&r).map_err(dans_la_ligne)?);
    }
    Ok(out)
}
