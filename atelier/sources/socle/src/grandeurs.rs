//! L'ARGENT ET LES QUANTITES : un montant, un taux, des heures, des cachets, des jours, des services, des semaines, des
//! unites d'un forfait, un coefficient, un montant unitaire (« Le glossaire »). Chacun porte un decimal exact, son echelle gardee (`24.0` reste `24.0`) ; il n'en sort
//! que par une porte nommee (`decimal`), la ou une ecriture ou la norme l'exige. Seules les operations qui ont un sens
//! se font : montant + montant, montant x taux = montant, heures x montant unitaire = montant ; montant + heures ne
//! compile pas. L'arithmetique est celle de toute la paie (`num::add`, `mul`, `div` : `d28`) ; l'arrondi au centime se
//! fait par `Montant::au_centime`, la ou une somme se fixe, et nulle part ailleurs.
//!
//! Une facon d'ecrire n'est pas un type : chaque forme dit la sienne par l'attribut serde du champ
//! (`#[serde(serialize_with = "socle::grandeurs::ecrit::centimes::serialize")]`), dans `ecrit`. Une grandeur se lit
//! d'un nombre JSON, jamais d'un texte, comme `forme::decimal` (un entier garde son echelle nulle) ; un champ lu comme
//! `forme::court` le dit
//! (`#[serde(deserialize_with = "socle::grandeurs::lu::court::deserialize")]`).

use crate::num::{self, Decimal};
use rust_decimal::{MathematicalOps, RoundingStrategy};

/// Ce que toute grandeur porte : un decimal. Il n'en sort que par `decimal`, nomme sur chaque type ; ce trait ne sert
/// qu'aux ecritures (`ecrit`) et aux lectures (`lu`) de ce module, scelle (`scelle::Scelle` ne se nomme pas dehors).
pub trait Grandeur: Copy + scelle::Scelle {}

mod scelle {
    use crate::num::Decimal;

    pub trait Scelle {
        fn en_decimal(self) -> Decimal;
        fn du(d: Decimal) -> Self;
    }
}

use scelle::Scelle;

/// Une grandeur par nom : son decimal, sa porte de sortie, l'addition et la soustraction de deux grandeurs de meme nom,
/// sa lecture.
macro_rules! grandeurs {
    ($($nom:ident, $doc:literal;)*) => {$(
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $nom(Decimal);

        impl $nom {
            /// La grandeur que dit ce decimal, tel qu'il est (son echelle gardee).
            pub const fn du_decimal(d: Decimal) -> $nom {
                $nom(d)
            }

            /// Le decimal nu, a la porte qui l'exige (une ecriture, la norme).
            pub const fn decimal(self) -> Decimal {
                self.0
            }

            /// Est-elle nulle ?
            pub fn est_nul(self) -> bool {
                self.0.is_zero()
            }

            /// Sa valeur absolue.
            pub fn abs(self) -> $nom {
                $nom(self.0.abs())
            }
        }

        impl std::ops::AddAssign for $nom {
            fn add_assign(&mut self, o: $nom) {
                *self = *self + o;
            }
        }

        impl std::ops::SubAssign for $nom {
            fn sub_assign(&mut self, o: $nom) {
                *self = *self - o;
            }
        }

        impl Scelle for $nom {
            fn en_decimal(self) -> Decimal {
                self.0
            }
            fn du(d: Decimal) -> $nom {
                $nom(d)
            }
        }

        impl Grandeur for $nom {}

        impl<'de> serde::Deserialize<'de> for $nom {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<$nom, D::Error> {
                crate::forme::decimal_en_nombre(d).map($nom)
            }
        }

        impl std::ops::Add for $nom {
            type Output = $nom;
            fn add(self, o: $nom) -> $nom {
                $nom(num::add(self.0, o.0))
            }
        }

        impl std::ops::Sub for $nom {
            type Output = $nom;
            fn sub(self, o: $nom) -> $nom {
                $nom(num::sub(self.0, o.0))
            }
        }

        impl std::ops::Neg for $nom {
            type Output = $nom;
            fn neg(self) -> $nom {
                $nom(-self.0)
            }
        }

        impl std::iter::Sum for $nom {
            fn sum<I: Iterator<Item = $nom>>(i: I) -> $nom {
                i.fold($nom(Decimal::ZERO), |a, b| a + b)
            }
        }
    )*};
}

grandeurs! {
    Montant, "UN MONTANT : une somme en euros (`1512.40`).";
    Taux, "UN TAUX : un pourcentage applique a une base, ecrit en pourcent comme la norme et les baremes (`6.90` : 6,90 %).";
    Heures, "DES HEURES : une quantite d'heures (`151.67`).";
    Cachets, "DES CACHETS : une quantite de cachets (`2`).";
    Jours, "DES JOURS : une quantite de jours (`4.92`).";
    Coefficient, "UN COEFFICIENT : un multiplicateur sans unite, ni taux ni montant (l'abattement de la CSG `0.9825`).";
    MontantUnitaire, "UN MONTANT UNITAIRE : des euros par unite (par heure, par cachet, par jour : `1.50`) ; le « taux » d'une ligne de paie, le plafond ou le SMIC horaire.";
    Services, "DES SERVICES : une quantite de services de repetition (`14`).";
    Semaines, "DES SEMAINES : une quantite de semaines (`2`).";
    Unites, "DES UNITES : le compte d'un forfait (`3` repas a 15 €).";
    HeuresParUnite, "DES HEURES PAR UNITE : les heures d'un service, d'un cachet, d'un forfait (`4` : 14 services de 4 h) ; quantite x heures par unite = heures.";
}

/// UNE GRANDEUR SANS ZERO INUTILE (`12.50` devient `12.5`, `24.0` devient `24`, `-0` devient `0`), comme l'ecrit
/// `ecrit::sans_zero` : la forme canonique des empreintes et des oracles.
pub fn sans_zero<G: Grandeur>(g: G) -> G {
    G::du(g.en_decimal().normalize())
}

/// LA GRANDEUR ENTIERE QU'ELLE EST, son echelle ramenee a zero (`4.0` jours : `4`) ; `None` : elle n'est pas entiere
/// (`4.5` jours). Des jours ou des cachets dits « en nombre entier » se tiennent ainsi.
pub fn entiere<G: Grandeur>(g: G) -> Option<G> {
    let d = g.en_decimal();
    match d.fract().is_zero() {
        true => Some(G::du(d.trunc())),
        false => None,
    }
}

/// UNE GRANDEUR SANS ZERO INUTILE, UNE DECIMALE AU MOINS (`24` s'ecrit `24.0`, `12.50` `12.5`), comme l'ecrit
/// `num::court` et la lit `lu::court` : celle d'un taux ou d'heures qu'une paie garde.
pub fn courte<G: Grandeur>(g: G) -> G {
    G::du(num::court(g.en_decimal()))
}

/// LE SIGNE QU'UNE LIGNE DONNE A SES GRANDEURS : les garder (`Plus`), ou les retirer (`Moins`, l'oppose). Un zero n'a pas
/// de signe : il reste le zero qu'il est, son echelle gardee. Signe x grandeur = grandeur de meme nom (`de`). Ecrit `+`
/// ou `-` (le `sens` d'une rubrique de `rubriques-base.json`) ; un autre signe se refuse a la lecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Signe {
    #[serde(rename = "+")]
    Plus,
    #[serde(rename = "-")]
    Moins,
}

impl Signe {
    /// La grandeur `g` sous ce signe.
    pub fn de<G: Grandeur>(self, g: G) -> G {
        let d = g.en_decimal();
        match self {
            Signe::Moins if !d.is_zero() => G::du(-d),
            Signe::Plus | Signe::Moins => g,
        }
    }
}

impl Montant {
    /// Zero au centime, ses deux decimales ecrites (`0.00`).
    pub const ZERO: Montant = Montant(Decimal::from_parts(0, 0, 0, false, 2));

    /// Un centime (`0.01`) : l'ecart de montant qu'un arrondi fait sur une ligne.
    pub const UN_CENTIME: Montant = Montant(Decimal::from_parts(1, 0, 0, false, 2));

    /// Ce montant `n` fois (un centime par ligne, un multiple du plafond) : montant x compte = montant. Un compte n'est
    /// pas une grandeur (« Le glossaire »).
    pub fn fois(self, n: usize) -> Montant {
        Montant(num::mul(self.0, Decimal::from(n)))
    }

    /// LE RAPPORT de ce montant a `o` : montant / montant = coefficient ; `None` quand `o` est nul.
    pub fn rapport(self, o: Montant) -> Option<Coefficient> {
        (!o.0.is_zero()).then(|| Coefficient(num::div(self.0, o.0)))
    }

    /// AU CENTIME, LA MOITIE VERS LE HAUT, ses deux decimales ecrites (2,125 donne 2,13 ; -2,125 donne -2,13, comme son
    /// oppose ; 12,5 s'ecrit 12,50) : l'arrondi au centime de la paie, le seul. Fonde par la convergence que dit
    /// `atelier/reference/sources/arrondi-euro/LISEZ-MOI.md` (« Le centime : ce que disent les autres textes ») : le CGI,
    /// ann. III, art. 39 C, 3°, f) (`legifrance-cgi-annexe3-art39c-20261114.txt` : « la fraction de décimale égale à
    /// 0,005 comptant pour 0,01 »), le BOFiP BOI-IR-PAS-20-20-30-10, § 180 (`bofip-boi-ir-pas-20-20-30-10-20230626.txt`),
    /// le reglement (CE) n° 1103/97, art. 5 (`eurlex-reglement-1103-97-introduction-euro-19970619.txt`), et pour l'euro
    /// le CSS L. 133-10 (`code-de-la-securite-sociale/articles-20261013.md`) ; le negatif comme son oppose, par la meme
    /// symetrie que l'euro (LISEZ-MOI, « Ce qui est une inference »).
    pub fn au_centime(self) -> Montant {
        let mut c = self.0.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        c.rescale(2);
        Montant(c)
    }

    /// Le montant que le taux `t` en prend, exact (`au_centime` le fixe).
    pub fn fois_taux(self, t: Taux) -> Montant {
        Montant(num::div(num::mul(self.0, t.0), Decimal::ONE_HUNDRED))
    }

    /// La part de ce montant que `part` heures font sur `tout` (le plafond d'un temps partiel), exacte : le produit,
    /// puis le quotient (`au_centime` la fixe). `tout` n'est pas nul : l'appelant le refuse.
    pub fn prorata(self, part: Heures, tout: Heures) -> Montant {
        Montant(num::div(num::mul(self.0, part.0), tout.0))
    }

    /// La part de ce montant que `part` fait sur `tout` (une assiette repartie au prorata du brut de chaque rubrique),
    /// exacte : le produit, puis le quotient. Montant x (montant / montant) = montant. `tout` n'est pas nul : l'appelant
    /// le garde.
    pub fn au_prorata(self, part: Montant, tout: Montant) -> Montant {
        Montant(num::div(num::mul(self.0, part.0), tout.0))
    }

    /// Ce montant pour chacune des heures `h` (le SMIC horaire d'un SMIC de reference) : montant / heures = montant
    /// unitaire ; `None` quand `h` est nul.
    pub fn par_heure(self, h: Heures) -> Option<MontantUnitaire> {
        (!h.0.is_zero()).then(|| MontantUnitaire(num::div(self.0, h.0)))
    }

    /// Ce montant divise par `c` (une reduction partagee par les points de sa formule) : montant / coefficient =
    /// montant ; `None` quand `c` est nul.
    pub fn sur(self, c: Coefficient) -> Option<Montant> {
        (!c.0.is_zero()).then(|| Montant(num::div(self.0, c.0)))
    }

    /// Ce montant pour chacun des jours `j` (le salaire journalier des paies d'un arret) : montant / jours = montant
    /// unitaire ; `None` quand `j` est nul.
    pub fn par_jour(self, j: Jours) -> Option<MontantUnitaire> {
        (!j.0.is_zero()).then(|| MontantUnitaire(num::div(self.0, j.0)))
    }

    /// Ce montant, celui d'une seule unite (une indemnite versee en une fois : une ligne d'une unite ; le plafond du
    /// salaire journalier, un pourcentage du plafond annuel pour un jour : CSS R433-2) : montant / une unite = montant
    /// unitaire.
    pub fn pour_une_unite(self) -> MontantUnitaire {
        MontantUnitaire(self.0)
    }

    /// Ce montant reparti egalement sur `n` (la moyenne des bruts de douze mois, le tiers de trois) : montant / compte =
    /// montant, exact (`au_centime` le fixe). Un compte n'est pas une grandeur (« Le glossaire ») : il vient d'un
    /// `len()`, jamais nul (l'appelant le refuse).
    pub fn reparti_sur(self, n: usize) -> Montant {
        Montant(num::div(self.0, Decimal::from(n)))
    }

    /// Sa partie entiere, sans arrondi (le plafond de la Securite sociale, que le bareme lit en euros entiers).
    pub fn partie_entiere(self) -> Montant {
        Montant(self.0.trunc())
    }
}

impl std::ops::Mul<Coefficient> for Montant {
    type Output = Montant;
    fn mul(self, c: Coefficient) -> Montant {
        Montant(num::mul(self.0, c.0))
    }
}

/// Coefficient x coefficient = coefficient.
impl std::ops::Mul for Coefficient {
    type Output = Coefficient;
    fn mul(self, c: Coefficient) -> Coefficient {
        Coefficient(num::mul(self.0, c.0))
    }
}

impl Coefficient {
    /// Zero.
    pub const ZERO: Coefficient = Coefficient(Decimal::ZERO);

    /// Un.
    pub const UN: Coefficient = Coefficient(Decimal::ONE);

    /// Ce coefficient divise par `c` : coefficient / coefficient = coefficient ; `None` quand `c` est nul.
    pub fn sur(self, c: Coefficient) -> Option<Coefficient> {
        (!c.0.is_zero()).then(|| Coefficient(num::div(self.0, c.0)))
    }

    /// Ce coefficient a la puissance `p` : coefficient ^ coefficient = coefficient (un exposant n'a pas d'unite, et un
    /// coefficient eleve a une puissance n'en prend pas) ; `None` quand elle ne se calcule pas.
    pub fn puissance(self, p: Coefficient) -> Option<Coefficient> {
        self.0.checked_powd(p.0).map(Coefficient)
    }

    /// A quatre decimales, la moitie vers le haut, ses quatre decimales ecrites : le coefficient de la reduction generale
    /// (`droit::reduction`).
    pub fn au_dix_millieme(self) -> Coefficient {
        let mut c = self.0.round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero);
        c.rescale(4);
        Coefficient(c)
    }

    /// Sa partie entiere, sans arrondi (un multiple de plafonds du bareme : `8` pour huit plafonds) : la partie entiere
    /// d'un coefficient est un coefficient (tronquer n'ajoute pas d'unite).
    pub fn entier(self) -> Coefficient {
        Coefficient(self.0.trunc())
    }

    /// A quatre decimales, la moitie au pair (un ecart relatif : aucun texte deposé ne dit cet arrondi).
    pub fn a_quatre_decimales(self) -> Coefficient {
        Coefficient(self.0.round_dp(4))
    }

    /// Ce coefficient ecrit en taux, en pourcent (`0.10` donne `10.00`) : coefficient x 100 = taux.
    pub fn en_taux(self) -> Taux {
        Taux(num::mul(self.0, Decimal::from(100)))
    }
}

impl Taux {
    /// Cinq dix-milliemes de point (`0.0005`) : l'ecart qu'un arrondi fait sur un taux imprime.
    pub const CINQ_DIX_MILLIEMES: Taux = Taux(Decimal::from_parts(5, 0, 0, false, 4));

    /// A deux decimales, la moitie au pair (aucun texte deposé ne dit l'arrondi d'un taux moyen : `heures_en_plus`).
    pub fn a_deux_decimales(self) -> Taux {
        Taux(self.0.round_dp(2))
    }
}

impl Cachets {
    /// Zero cachet.
    pub const ZERO: Cachets = Cachets(Decimal::ZERO);

    /// Un cachet (le singulier d'une phrase : « 1 cachet », « 2 cachets »).
    pub const UN: Cachets = Cachets(Decimal::ONE);

    /// Les cachets que `n` dates comptent, un par date : un compte devient des cachets.
    pub fn de_dates(n: usize) -> Cachets {
        Cachets(Decimal::from(n))
    }

    /// Sa partie entiere (`2.5` cachets en comptent `2`) : un nombre de cachets declares se compte en entiers.
    pub fn partie_entiere(self) -> Cachets {
        Cachets(self.0.trunc())
    }
}

impl Jours {
    /// Zero jour.
    pub const ZERO: Jours = Jours(Decimal::ZERO);

    /// Un jour (le singulier d'une phrase).
    pub const UN: Jours = Jours(Decimal::ONE);

    /// Les jours que `n` dates comptent, une par jour : un compte devient des jours.
    pub fn de_dates(n: usize) -> Jours {
        Jours(Decimal::from(n))
    }

    /// Un demi-jour.
    pub const DEMI: Jours = Jours(Decimal::from_parts(5, 0, 0, false, 1));

    /// Au demi-jour le plus proche, la moitie loin de zero (1,04 jour : 1 ; 1,3 : 1,5) : des jours qu'une phrase dit
    /// « environ ». Aucun texte deposé ne dit cet arrondi.
    pub fn au_demi(self) -> Jours {
        let deux = Decimal::from(2);
        Jours(num::div(num::mul(self.0, deux).round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero), deux))
    }

    /// LE RAPPORT de ces jours a `o` (une quotite en jours sur sa reference) : jours / jours = coefficient ; `None` quand
    /// `o` est nul.
    pub fn rapport(self, o: Jours) -> Option<Coefficient> {
        (!o.0.is_zero()).then(|| Coefficient(num::div(self.0, o.0)))
    }
}


/// Des jours fois un coefficient (le coefficient de l'art. 30 des annexes 8 et 10 : un jour de travail retire 1,3 jour
/// d'allocation) : des jours.
impl std::ops::Mul<Coefficient> for Jours {
    type Output = Jours;
    fn mul(self, c: Coefficient) -> Jours {
        Jours(num::mul(self.0, c.0))
    }
}

impl Services {
    /// Les services que `n` compte, un par un : un compte devient des services.
    pub fn comptes(n: usize) -> Services {
        Services(Decimal::from(n))
    }
}

impl Montant {
    /// Les heures que ce montant paie a `par_heure` l'heure : montant / montant unitaire = heures ; `None` quand
    /// `par_heure` est nul.
    pub fn en_heures(self, par_heure: MontantUnitaire) -> Option<Heures> {
        (!par_heure.0.is_zero()).then(|| Heures(num::div(self.0, par_heure.0)))
    }
}

impl HeuresParUnite {
    /// Zero heure par unite (une indemnite, le maintien : ce n'est pas du travail).
    pub const ZERO: HeuresParUnite = HeuresParUnite(Decimal::ZERO);
}

/// Une quantite fois ses heures par unite : des heures (14 services de 4 h : 56 h).
macro_rules! aux_heures_par_unite {
    ($($q:ident),*) => {$(
        impl std::ops::Mul<HeuresParUnite> for $q {
            type Output = Heures;
            fn mul(self, h: HeuresParUnite) -> Heures {
                Heures(num::mul(self.0, h.0))
            }
        }
    )*};
}

aux_heures_par_unite!(Heures, Cachets, Jours, Services, Semaines, Unites, Coefficient);

impl Heures {
    /// Zero heure.
    pub const ZERO: Heures = Heures(Decimal::ZERO);

    /// Une heure (le singulier d'une phrase).
    pub const UNE: Heures = Heures(Decimal::ONE);

    /// Au centieme d'heure, la moitie vers le haut, ses deux decimales ecrites (`7.5` s'ecrit `7.50`) : des heures
    /// ramenees au jour, au mois ou a la semaine. Aucun texte deposé ne dit cet arrondi.
    pub fn au_centieme(self) -> Heures {
        let mut c = self.0.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        c.rescale(2);
        Heures(c)
    }

    /// A deux decimales, la moitie au pair (les heures exactes d'un rapport, que le bulletin dit au centieme : aucun texte
    /// deposé ne dit cet arrondi).
    pub fn a_deux_decimales(self) -> Heures {
        Heures(self.0.round_dp(2))
    }

    /// Ces heures `n` fois (les heures d'un cachet fois les jours d'un contrat) : heures x compte = heures. Un compte
    /// n'est pas une grandeur (« Le glossaire »), comme pour `Montant::fois`.
    pub fn fois(self, n: usize) -> Heures {
        Heures(num::mul(self.0, Decimal::from(n)))
    }

    /// Ces heures reparties egalement sur `n` jours (les heures d'une ligne sur ses dates) : heures / compte = heures. Un
    /// compte n'est pas une grandeur (« Le glossaire ») : il vient d'un `len()`, jamais nul (l'appelant le refuse).
    pub fn reparties_sur(self, n: usize) -> Heures {
        Heures(num::div(self.0, Decimal::from(n)))
    }

    /// En heures entieres, la moitie au pair : les heures d'un contrat que le cumul des annexes 8 et 10 compte en entiers
    /// (`droit::cumul::Declare`).
    pub fn entieres(self) -> Heures {
        let mut e = self.0.round_dp_with_strategy(0, RoundingStrategy::MidpointNearestEven);
        e.rescale(0);
        Heures(e)
    }

    /// LE RAPPORT de ces heures a `o` (des heures comptees sur un seuil) : heures / heures = coefficient ; `None` quand
    /// `o` est nul.
    pub fn rapport(self, o: Heures) -> Option<Coefficient> {
        (!o.0.is_zero()).then(|| Coefficient(num::div(self.0, o.0)))
    }

    /// Les jours que ces heures font, a `par_jour` heures le jour (art. 30 des annexes 8 et 10 : 10 h font un jour) :
    /// heures / heures par unite = jours ; `None` quand `par_jour` est nul.
    pub fn en_jours(self, par_jour: HeuresParUnite) -> Option<Jours> {
        (!par_jour.0.is_zero()).then(|| Jours(num::div(self.0, par_jour.0)))
    }

    /// Ces heures reparties egalement sur `n` jours, en heures par jour (35 h sur cinq jours : 7 h le jour) : heures /
    /// compte = heures par unite, l'unite etant le jour. Un compte n'est pas une grandeur (« Le glossaire »).
    pub fn par_jour(self, n: usize) -> HeuresParUnite {
        HeuresParUnite(num::div(self.0, Decimal::from(n)))
    }

    /// Leur partie entiere, vers le bas (`13.9` h : `13`) : les heures entieres qu'une somme couvre.
    pub fn entieres_en_dessous(self) -> Heures {
        Heures(self.0.floor())
    }

    /// LES HEURES D'UN MOIS PAYE, PAR MOIS : la quotite mensuelle d'un contrat paye au mois (`151.67`), au centieme
    /// d'heure et dans sa forme la plus courte, en heures par unite, l'unite etant le mois ; la ligne « mois », dont la
    /// quantite est la part du mois payee, compte ainsi ses heures (`QuantiteDeLigne::heures`).
    pub fn par_mois(self) -> HeuresParUnite {
        HeuresParUnite(num::court(self.au_centieme().0))
    }

    /// Les cachets qu'il faut pour faire ces heures, a `par_cachet` heures chacun, le dernier entame compte (13 h a 12 h le
    /// cachet : 2 cachets) ; `None` : un cachet de zero heure n'en fait aucune.
    pub fn cachets_pour(self, par_cachet: HeuresParUnite) -> Option<Cachets> {
        if par_cachet.0.is_zero() {
            return None;
        }
        let mut c = num::div(self.0, par_cachet.0).ceil();
        c.rescale(0);
        Some(Cachets(c))
    }
}

/// Une quantite fois son montant unitaire : un montant.
macro_rules! au_montant_unitaire {
    ($($q:ident),*) => {$(
        impl std::ops::Mul<MontantUnitaire> for $q {
            type Output = Montant;
            fn mul(self, u: MontantUnitaire) -> Montant {
                Montant(num::mul(self.0, u.0))
            }
        }
    )*};
}

au_montant_unitaire!(Heures, Cachets, Jours, Services, Semaines, Unites);

impl MontantUnitaire {
    /// Ce montant unitaire pour `n` unites comptees (le taux d'un cachet fois les cachets d'une periode) : montant
    /// unitaire x compte = montant. Un compte n'est pas une grandeur (« Le glossaire ») : il vient d'un `len()`.
    pub fn fois(self, n: usize) -> Montant {
        Montant(num::mul(self.0, Decimal::from(n)))
    }

    /// Au centime, par l'arrondi de la paie (`Montant::au_centime`) : l'indemnite d'un jour.
    pub fn au_centime(self) -> MontantUnitaire {
        MontantUnitaire(Montant(self.0).au_centime().0)
    }

    /// Le montant unitaire que le taux `t` en prend, exact (`au_centime` le fixe) : montant unitaire x taux = montant
    /// unitaire.
    pub fn fois_taux(self, t: Taux) -> MontantUnitaire {
        MontantUnitaire(num::div(num::mul(self.0, t.0), Decimal::ONE_HUNDRED))
    }

    /// Ce montant unitaire fois le coefficient `c` (la moitie du salaire journalier) : montant unitaire x coefficient =
    /// montant unitaire.
    pub fn fois_coefficient(self, c: Coefficient) -> MontantUnitaire {
        MontantUnitaire(num::mul(self.0, c.0))
    }

    /// Sa partie entiere, sans arrondi (le plafond horaire de la Securite sociale, que le bareme lit en euros entiers).
    pub fn partie_entiere(self) -> MontantUnitaire {
        MontantUnitaire(self.0.trunc())
    }
}

/// La part d'une unite (la part du mois payee : 12/31) fois le montant de l'unite entiere (le salaire mensuel) : un
/// montant.
impl std::ops::Mul<MontantUnitaire> for Coefficient {
    type Output = Montant;
    fn mul(self, u: MontantUnitaire) -> Montant {
        Montant(num::mul(self.0, u.0))
    }
}

/// LES LECTURES D'UNE GRANDEUR AUTRES QUE LA SIENNE (`forme::decimal_en_nombre`) : un champ les dit par son attribut serde
/// (`…::deserialize`, `…::option::deserialize` pour un `Option`).
pub mod lu {
    use crate::grandeurs::Grandeur;
    use crate::num::{self, Decimal};

    /// LA GRANDEUR QU'UN TEXTE D'UNE PIECE ECRIT (une case d'un formulaire, d'une attestation, d'un bulletin), a la
    /// francaise ou non (`num::decimal_a_virgule` : « 12,5 », « 12.50 »), son echelle gardee : la porte de sa lecture ;
    /// `None` : le texte n'est pas un nombre.
    pub fn texte<G: Grandeur>(t: &str) -> Option<G> {
        num::decimal_a_virgule(t).map(<G as crate::grandeurs::Scelle>::du)
    }

    /// LA GRANDEUR QU'UNE RUBRIQUE DE LA NORME ECRIT (une DSN : `151.67`), comme `num::decimal_lu` la lit, son echelle
    /// gardee : la porte de sa lecture ; `None` : le texte n'est pas un nombre.
    pub fn norme<G: Grandeur>(t: &str) -> Option<G> {
        num::decimal_lu(t).map(<G as crate::grandeurs::Scelle>::du)
    }

    /// LA GRANDEUR QU'UN TEXTE D'UNE PIECE ECRIT EN ENTIER (`num::entier_lu` : une zone a largeur fixe de l'AEM, `012`),
    /// son echelle nulle : la porte de sa lecture ; `None` : le texte n'est pas un entier, ou il deborde.
    pub fn texte_entier<G: Grandeur>(t: &str) -> Option<G> {
        match num::entier_lu(t).map(|n| Decimal::try_from_i128_with_scale(n, 0)) {
            Some(Ok(d)) => Some(<G as crate::grandeurs::Scelle>::du(d)),
            Some(Err(_)) | None => None,
        }
    }

    /// LES DEUX GRANDEURS QU'UN MEME TEXTE ENTIER D'UNE PIECE PEUT DIRE (la case « NB JOURS OU CACHETS » d'un
    /// certificat, qui ne dit pas laquelle), lu une fois comme `texte_entier` ; `None` : le texte n'est pas un entier.
    pub fn texte_entier_en_deux<A: Grandeur, B: Grandeur>(t: &str) -> Option<(A, B)> {
        match num::entier_lu(t).map(|n| Decimal::try_from_i128_with_scale(n, 0)) {
            Some(Ok(d)) => Some((<A as crate::grandeurs::Scelle>::du(d), <B as crate::grandeurs::Scelle>::du(d))),
            Some(Err(_)) | None => None,
        }
    }

    /// Comme `forme::decimal` : un nombre, ou un nombre ecrit en texte (une ligne de paie, une paie ecrite : le moteur
    /// les ecrit en texte, la page envoie un nombre ou un texte).
    pub mod nombre_ou_texte {
        use crate::grandeurs::Grandeur;

        pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<G, D::Error> {
            crate::forme::decimal(d).map(<G as crate::grandeurs::Scelle>::du)
        }

        /// Un `Option` lu ainsi : absent (avec `#[serde(default)]`) ou nul, il est vide.
        pub mod option {
            use crate::grandeurs::Grandeur;

            pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<Option<G>, D::Error> {
                crate::forme::decimal_opt(d).map(|o| o.map(<G as crate::grandeurs::Scelle>::du))
            }
        }
    }

    /// Comme `forme::court` : un nombre (jamais un texte), dans sa forme la plus courte, une decimale au moins (`24` se
    /// lit `24.0`).
    pub mod court {
        use crate::grandeurs::Grandeur;

        pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<G, D::Error> {
            crate::forme::court(d).map(<G as crate::grandeurs::Scelle>::du)
        }

        /// Un `Option` lu ainsi : absent (avec `#[serde(default)]`) ou nul, il est vide.
        pub mod option {
            use crate::grandeurs::Grandeur;

            pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<Option<G>, D::Error> {
                <Option<crate::forme::Court> as serde::Deserialize>::deserialize(d).map(|c| c.map(|c| <G as crate::grandeurs::Scelle>::du(c.0)))
            }
        }

        /// Une liste facultative lue ainsi : absente (avec `#[serde(default)]`) ou nulle, elle est vide.
        pub mod liste_option {
            use crate::grandeurs::Grandeur;

            pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Vec<G>>, D::Error> {
                <Option<Vec<crate::forme::Court>> as serde::Deserialize>::deserialize(d)
                    .map(|l| l.map(|l| l.into_iter().map(|c| <G as crate::grandeurs::Scelle>::du(c.0)).collect()))
            }
        }
    }
}

/// LES ECRITURES D'UNE GRANDEUR : une facon d'ecrire n'est pas un type. Chaque forme dit la sienne par l'attribut serde
/// du champ (`…::serialize`, `…::option::serialize` pour un `Option`).
pub mod ecrit {
    /// Le `serialize` d'une ecriture `texte` (une grandeur en texte, ou pourquoi elle ne s'ecrit pas ainsi), et celui de
    /// son `Option`. Une phrase qui cite une grandeur l'ecrit par le meme `texte`.
    macro_rules! ecriture {
        () => {
            pub fn serialize<G: Grandeur, S: serde::Serializer>(g: &G, s: S) -> Result<S::Ok, S::Error> {
                match texte(*g) {
                    Ok(t) => s.serialize_str(&t),
                    Err(e) => Err(serde::ser::Error::custom(e)),
                }
            }

            /// Un `Option` dans cette ecriture : `None` s'ecrit `null`.
            pub mod option {
                use crate::grandeurs::Grandeur;

                pub fn serialize<G: Grandeur, S: serde::Serializer>(g: &Option<G>, s: S) -> Result<S::Ok, S::Error> {
                    match g {
                        Some(g) => super::serialize(g, s),
                        None => s.serialize_none(),
                    }
                }
            }
        };
    }

    /// En texte, son echelle gardee (`12.5`, `12.50`) : la grandeur arrive deja au centime ou normalisee, l'ecriture ne
    /// change pas son echelle.
    pub mod texte {
        use crate::grandeurs::Grandeur;

        pub fn texte<G: Grandeur>(g: G) -> Result<String, String> {
            Ok(g.en_decimal().to_string())
        }

        /// Relue exactement de son texte, son echelle gardee (un nombre JSON ne se lit pas ainsi).
        pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<G, D::Error> {
            use serde::Deserialize;
            let t = String::deserialize(d)?;
            match t.parse::<crate::num::Decimal>() {
                Ok(x) => Ok(<G as crate::grandeurs::Scelle>::du(x)),
                Err(_) => Err(serde::de::Error::custom(format!("{} n'est pas un nombre", crate::ecriture::json_en_ligne(&t)))),
            }
        }

        ecriture!();
    }

    /// En texte, a deux decimales (`12.50`) ; un decimal plus fin que le centime ne s'ecrit pas ainsi.
    pub mod centimes {
        use crate::grandeurs::Grandeur;

        pub fn texte<G: Grandeur>(g: G) -> Result<String, String> {
            let mut c = g.en_decimal();
            if c.scale() > 2 {
                return Err(format!("{} est plus fin que le centime", c));
            }
            c.rescale(2);
            Ok(c.to_string())
        }

        ecriture!();
    }

    /// En texte, sans zero inutile (`12.5`, `12`).
    pub mod sans_zero {
        use crate::grandeurs::Grandeur;

        pub fn texte<G: Grandeur>(g: G) -> Result<String, String> {
            Ok(dit(g))
        }

        /// La meme, telle qu'une phrase ou une case la cite (`2`, `151.67`).
        pub fn dit<G: Grandeur>(g: G) -> String {
            g.en_decimal().normalize().to_string()
        }

        ecriture!();
    }

    /// En texte, dans sa forme la plus courte (`12.5`, `12` ; « -0 » s'ecrit « 0 » : `num::nombre`), relue d'un nombre
    /// ou d'un texte (`forme::decimal`) : l'ecriture des nombres d'une paie ecrite.
    pub mod nombre {
        use crate::grandeurs::Grandeur;

        pub fn serialize<G: Grandeur, S: serde::Serializer>(g: &G, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_str(&crate::num::nombre(g.en_decimal()))
        }

        pub use crate::grandeurs::lu::nombre_ou_texte::deserialize;

        /// Un `Option` dans cette ecriture : `None` s'ecrit `null`, et se relit absent ou nul.
        pub mod option {
            use crate::grandeurs::Grandeur;

            pub use crate::grandeurs::lu::nombre_ou_texte::option::deserialize;

            pub fn serialize<G: Grandeur, S: serde::Serializer>(g: &Option<G>, s: S) -> Result<S::Ok, S::Error> {
                match g {
                    Some(g) => super::serialize(g, s),
                    None => s.serialize_none(),
                }
            }
        }
    }

    /// En nombre JSON, sans zero inutile : entier quand elle l'est (`12`, pas `12.0`), a virgule sinon (`4.5`).
    pub mod nombre_json {
        use crate::grandeurs::Grandeur;

        pub fn serialize<G: Grandeur, S: serde::Serializer>(g: &G, s: S) -> Result<S::Ok, S::Error> {
            let d = g.en_decimal().normalize();
            match (d.scale(), i64::try_from(d.mantissa())) {
                (0, Ok(n)) => s.serialize_i64(n),
                _ => crate::forme::ecrire_a_virgule(&d, s),
            }
        }

        /// Un `Option` dans cette ecriture : `None` s'ecrit `null`.
        pub mod option {
            use crate::grandeurs::Grandeur;

            pub fn serialize<G: Grandeur, S: serde::Serializer>(g: &Option<G>, s: S) -> Result<S::Ok, S::Error> {
                match g {
                    Some(g) => super::serialize(g, s),
                    None => s.serialize_none(),
                }
            }
        }
    }

    /// En nombre entier JSON (`2`) ; un decimal qui n'est pas un entier ne s'ecrit pas ainsi.
    pub mod entier {
        use crate::grandeurs::Grandeur;

        pub fn serialize<G: Grandeur, S: serde::Serializer>(g: &G, s: S) -> Result<S::Ok, S::Error> {
            let d = g.en_decimal();
            match (d.scale(), i64::try_from(d.mantissa())) {
                (0, Ok(n)) => s.serialize_i64(n),
                _ => Err(serde::ser::Error::custom(format!("{} n'est pas un entier", d))),
            }
        }

        /// Un `Option` dans cette ecriture : `None` s'ecrit `null`.
        pub mod option {
            use crate::grandeurs::Grandeur;

            pub fn serialize<G: Grandeur, S: serde::Serializer>(g: &Option<G>, s: S) -> Result<S::Ok, S::Error> {
                match g {
                    Some(g) => super::serialize(g, s),
                    None => s.serialize_none(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// CGI, ann. III, art. 39 C, 3°, f) ; BOFiP BOI-IR-PAS-20-20-30-10, § 180 ; reglement (CE) n° 1103/97, art. 5
    /// (`atelier/reference/sources/arrondi-euro/`) : le demi-centime compte pour un centime ; le negatif s'arrondit comme
    /// son oppose par INFERENCE (`arrondi-euro/LISEZ-MOI.md`).
    #[test]
    fn au_centime_arrondit_le_demi_vers_le_haut() {
        let c = |t: &str| Montant::du_decimal(Decimal::from_str(t).unwrap()).au_centime().decimal().to_string();
        assert_eq!(c("2.675"), "2.68");
        assert_eq!(c("2.125"), "2.13");
        assert_eq!(c("258.336"), "258.34");
        assert_eq!(c("-0.005"), "-0.01");
        assert_eq!(c("12.5"), "12.50");
    }
}
