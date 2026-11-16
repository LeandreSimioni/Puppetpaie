//! L'ARGENT ET LES QUANTITES : un montant, un taux, des heures, des cachets, des jours, des services, des semaines, des
//! unites d'un forfait, un coefficient, un montant unitaire (« Le glossaire »). Chacun porte un decimal exact, son echelle gardee (`24.0` reste `24.0`) ; il n'en sort
//! que par une porte nommee (`decimal`), la ou une ecriture ou la norme l'exige. Seules les operations qui ont un sens
//! se font : montant + montant, montant x taux = montant, heures x montant unitaire = montant ; montant + heures ne
//! compile pas. L'arithmetique est celle de toute la paie (`num::add`, `mul`, `div` : `d28`) ; l'arrondi au centime se
//! fait par `Montant::au_centime`, la ou une somme se fixe, et nulle part ailleurs.
//!
//! Une facon d'ecrire n'est pas un type : chaque forme dit la sienne par l'attribut serde du champ
//! (`#[serde(serialize_with = "socle::grandeurs::ecrit::centimes::serialize")]`), dans `ecrit`. Une grandeur se lit
//! d'un nombre JSON, jamais d'un texte, comme `forme::decimal` (un entier garde son echelle nulle) ; dans la charge d'une
//! route (`forme::lire_saisie`), un texte est ce qu'une personne a tape, converti une fois (`lu::tapee`) ; un champ lu comme
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
        /// Le signe de son unite, tel qu'une personne le tape apres le nombre (« % », « € ») ; aucun pour une grandeur
        /// qui n'en a pas.
        const SIGNES: &'static [&'static str];
        /// Ce qu'une case de cette grandeur attend, dit a qui la remplit (« un nombre d'heures »).
        const ATTENDU: &'static str;
    }
}

use scelle::Scelle;

/// Une grandeur par nom : son decimal, sa porte de sortie, l'addition et la soustraction de deux grandeurs de meme nom,
/// sa lecture.
macro_rules! grandeurs {
    ($($nom:ident, [$($signe:literal),*], $attendu:literal, $doc:literal;)*) => {
        /// Les signes d'unite de toutes les grandeurs : un signe d'une autre unite que celle de la case se refuse.
        const TOUS_LES_SIGNES: &[&str] = &[$($($signe,)*)*];
        $(
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
            const SIGNES: &'static [&'static str] = &[$($signe),*];
            const ATTENDU: &'static str = $attendu;
        }

        impl Grandeur for $nom {}

        impl<'de> serde::Deserialize<'de> for $nom {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<$nom, D::Error> {
                crate::forme::decimal_en_nombre(d, lu::tapee_en_decimal::<$nom>).map($nom)
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
    Montant, ["€"], "un montant en euros", "UN MONTANT : une somme en euros (`1512.40`).";
    Taux, ["%"], "un taux en pourcent", "UN TAUX : un pourcentage applique a une base, ecrit en pourcent comme la norme et les baremes (`6.90` : 6,90 %).";
    Heures, ["h"], "un nombre d'heures", "DES HEURES : une quantite d'heures (`151.67`).";
    Cachets, [], "un nombre de cachets", "DES CACHETS : une quantite de cachets (`2`).";
    Jours, [], "un nombre de jours", "DES JOURS : une quantite de jours (`4.92`).";
    Coefficient, [], "un nombre", "UN COEFFICIENT : un multiplicateur sans unite, ni taux ni montant (l'abattement de la CSG `0.9825`).";
    MontantUnitaire, ["€"], "un montant en euros", "UN MONTANT UNITAIRE : des euros par unite (par heure, par cachet, par jour : `1.50`) ; le « taux » d'une ligne de paie, le plafond ou le SMIC horaire.";
    Services, [], "un nombre de services", "DES SERVICES : une quantite de services de repetition (`14`).";
    Semaines, [], "un nombre de semaines", "DES SEMAINES : une quantite de semaines (`2`).";
    Unites, [], "un nombre d'unités", "DES UNITES : le compte d'un forfait (`3` repas a 15 €).";
    HeuresParUnite, ["h"], "un nombre d'heures", "DES HEURES PAR UNITE : les heures d'un service, d'un cachet, d'un forfait (`4` : 14 services de 4 h) ; quantite x heures par unite = heures.";
    Effectif, [], "un nombre de personnes", "UN EFFECTIF : un nombre de personnes, eventuellement moyen (`11.42`) : celui que l'Urssaf calcule, les beneficiaires de l'OETH, les alternants de la CSA, un seuil d'effectif.";
}

/// UNE GRANDEUR SANS ZERO INUTILE (`12.50` devient `12.5`, `24.0` devient `24`, `-0` devient `0`), comme l'ecrit
/// `ecrit::sans_zero` : la forme canonique des empreintes et des oracles.
pub fn sans_zero<G: Grandeur>(g: G) -> G {
    G::du(g.en_decimal().normalize())
}

/// LA GRANDEUR NULLE, son echelle nulle (`0`) : la quantite d'une ligne qu'aucune date ne compte.
pub fn nulle<G: Grandeur>() -> G {
    G::du(Decimal::ZERO)
}

/// UNE, son echelle nulle (`1`) : la quantite d'une ligne qui se paie en une fois.
pub fn une<G: Grandeur>() -> G {
    G::du(Decimal::ONE)
}

/// LA GRANDEUR AU CENTIEME, la moitie au pair, sans zero inutile (`2.125` : `2.12`, `4.50` : `4.5`) : ce qu'une phrase
/// dit d'une quantite (la designation d'une ligne de paie), dite par `texte::virgule`.
pub fn au_centieme_au_pair<G: Grandeur>(g: G) -> G {
    G::du(g.en_decimal().round_dp(2).normalize())
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

/// LA GRANDEUR A DEUX DECIMALES ECRITES qu'elle est, son echelle portee a deux (`5.5` : `5.50`) ; `None` : elle est
/// ecrite plus fine que le centieme (`5.505`, `5.500`), comme `ecrit::centimes` la refuse, ou elle porte un signe moins
/// (`-0.00` compris). Un taux de PAS se tient ainsi.
pub fn aux_centiemes<G: Grandeur>(g: G) -> Option<G> {
    let mut d = g.en_decimal();
    if d.scale() > 2 || d.is_sign_negative() {
        return None;
    }
    d.rescale(2);
    Some(G::du(d))
}

/// UNE GRANDEUR SANS ZERO INUTILE, UNE DECIMALE AU MOINS (`24` s'ecrit `24.0`, `12.50` `12.5`), comme l'ecrit
/// `num::court` et la lit `lu::court` : celle d'un taux ou d'heures qu'une paie garde.
pub fn courte<G: Grandeur>(g: G) -> G {
    G::du(num::court(g.en_decimal()))
}

/// LE SIGNE D'UNE GRANDEUR (celui qu'une ligne de paie donne a ses grandeurs, l'annulation d'un montant declare dans
/// la DSN) : la garder (`Plus`), ou la retirer (`Moins`, l'oppose). Un zero n'a pas
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

    /// La part de ce montant que `part` fait sur `tout`, d'une meme grandeur (des heures : le plafond d'un temps partiel ;
    /// des jours : le dixieme des jours pris sur ceux qu'une periode a acquis ; des montants : une assiette repartie au
    /// prorata du brut de chaque rubrique), exacte : le produit, puis le quotient (`au_centime` la fixe). `tout` n'est pas
    /// nul : l'appelant le refuse.
    pub fn prorata<Q: Grandeur>(self, part: Q, tout: Q) -> Montant {
        Montant(num::div(num::mul(self.0, part.en_decimal()), tout.en_decimal()))
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

    /// A L'EURO LE PLUS PROCHE, la moitie vers le haut, sans decimale (`12.50` donne `13`) : le brut et la base qu'un
    /// certificat Congés Spectacles ecrit en euros entiers.
    pub fn a_l_euro(self) -> Montant {
        let mut e = self.0.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
        e.rescale(0);
        Montant(e)
    }

    /// A L'EURO LE PLUS PROCHE, la moitie au pair, sans decimale (`12.50` donne `12`) : la taxe sur les salaires due et
    /// sa decote, telles que l'etat « taxe sur les salaires » les calcule (aucun texte depose ne dit cet arrondi).
    pub fn a_l_euro_au_pair(self) -> Montant {
        let mut e = self.0.round_dp_with_strategy(0, RoundingStrategy::MidpointNearestEven);
        e.rescale(0);
        Montant(e)
    }

    /// Un montant qu'un texte fixe, en euros entiers (`4000` : le seuil d'un versement unique de la taxe sur les salaires).
    pub const fn euros(n: u32) -> Montant {
        Montant(Decimal::from_parts(n, 0, 0, false, 0))
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

    /// Un coefficient qu'un texte fixe, entier (`400` : 400 fois le SMIC horaire, D5212-20).
    pub const fn entier_fixe(n: u32) -> Coefficient {
        Coefficient(Decimal::from_parts(n, 0, 0, false, 0))
    }

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

    /// A `n` decimales au plus, la moitie au pair : un facteur que la visee d'un montant cherche (`engagement::viser`, a
    /// vingt decimales ; dit a six). Aucun texte ne fixe cet arrondi : ce n'est pas un calcul de paie.
    pub fn a_decimales(self, n: u32) -> Coefficient {
        Coefficient(self.0.round_dp(n))
    }

    /// Ce coefficient ecrit en taux, en pourcent (`0.10` donne `10.00`) : coefficient x 100 = taux.
    pub fn en_taux(self) -> Taux {
        Taux(num::mul(self.0, Decimal::from(100)))
    }
}

impl Taux {
    /// Zero pour cent, ses deux decimales ecrites (`0.00`).
    pub const ZERO: Taux = Taux(Decimal::from_parts(0, 0, 0, false, 2));

    /// Cinq dix-milliemes de point (`0.0005`) : l'ecart qu'un arrondi fait sur un taux imprime.
    pub const CINQ_DIX_MILLIEMES: Taux = Taux(Decimal::from_parts(5, 0, 0, false, 4));

    /// Cent pour cent (`100`) : le tout, qu'une part ne depasse pas.
    pub const CENT_POUR_CENT: Taux = Taux(Decimal::ONE_HUNDRED);

    /// Ce taux en coefficient (`10.00` donne `0.10`) : taux / 100 = coefficient (l'inverse de `Coefficient::en_taux`).
    pub fn en_coefficient(self) -> Coefficient {
        Coefficient(num::div(self.0, Decimal::ONE_HUNDRED))
    }

    /// A deux decimales, la moitie au pair (aucun texte deposé ne dit l'arrondi d'un taux moyen : `heures_en_plus`).
    pub fn a_deux_decimales(self) -> Taux {
        Taux(self.0.round_dp(2))
    }

    /// A `decimales` decimales au plus, la moitie au pair, dans sa forme la plus courte (`courte`) : un taux qu'un
    /// bareme compose (une part d'un total, un taux par un facteur).
    pub fn au_pair(self, decimales: u32) -> Taux {
        Taux(num::court(self.0.round_dp_with_strategy(decimales, RoundingStrategy::MidpointNearestEven)))
    }

    /// A `decimales` decimales au plus, la moitie loin de zero, dans sa forme la plus courte (`courte`) : la somme des
    /// composants d'un taux, telle qu'un bareme l'affiche.
    pub fn loin_de_zero(self, decimales: u32) -> Taux {
        Taux(num::court(self.0.round_dp_with_strategy(decimales, RoundingStrategy::MidpointAwayFromZero)))
    }

    /// Un taux qu'un texte fixe, en points entiers (`20` : 20 %).
    pub const fn points(n: u32) -> Taux {
        Taux(Decimal::from_parts(n, 0, 0, false, 0))
    }

    /// Un taux qu'un texte fixe, en centiemes de point (`40` : 0,40 %).
    pub const fn centiemes_de_point(n: u32) -> Taux {
        Taux(Decimal::from_parts(n, 0, 0, false, 2))
    }

    /// Ce taux `n` fois (le taux d'un enfant couvert, pour chacun) : taux x compte = taux. Un compte n'est pas une
    /// grandeur (« Le glossaire »).
    pub fn fois(self, n: usize) -> Taux {
        Taux(num::mul(self.0, Decimal::from(n)))
    }

    /// LE RAPPORT de ce taux a `o` : taux / taux = coefficient ; `None` quand `o` est nul.
    pub fn rapport(self, o: Taux) -> Option<Coefficient> {
        (!o.0.is_zero()).then(|| Coefficient(num::div(self.0, o.0)))
    }
}

/// Taux x coefficient = taux (le taux AT/MP par le facteur d'une ligne, une part d'un taux).
impl std::ops::Mul<Coefficient> for Taux {
    type Output = Taux;
    fn mul(self, c: Coefficient) -> Taux {
        Taux(num::mul(self.0, c.0))
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

    /// Ces jours `n` fois (les jours acquis par mois fois les jours de presence du mois) : jours x compte = jours. Un
    /// compte n'est pas une grandeur (« Le glossaire »).
    pub fn fois(self, n: usize) -> Jours {
        Jours(num::mul(self.0, Decimal::from(n)))
    }

    /// Ces jours repartis egalement sur `n` (les jours du mois) : jours / compte = jours. Un compte n'est pas une
    /// grandeur (« Le glossaire ») : il vient d'un `count()`, jamais nul (l'appelant le garde).
    pub fn reparties_sur(self, n: usize) -> Jours {
        Jours(num::div(self.0, Decimal::from(n)))
    }

    /// A L'ENTIER SUPERIEUR (une duree de conge, C. trav. L3141-7 : un nombre de jours qui n'est pas entier « est porté
    /// au nombre entier immédiatement supérieur ») ; des jours deja entiers gardent leur ecriture (`30.0`).
    pub fn a_l_entier_superieur(self) -> Jours {
        match self.0.fract().is_zero() {
            true => self,
            false => Jours(self.0.ceil()),
        }
    }

    /// Au demi-jour superieur, sans zero inutile (0,1 jour : 0,5 ; 0,6 : 1) : le compte de conges qu'une phrase ou un
    /// etat affiche (un affichage, pas une regle).
    pub fn au_demi_superieur(self) -> Jours {
        let demi = (self.0 * Decimal::from(2)).ceil();
        Jours(num::div(demi, Decimal::from(2)).normalize())
    }

    /// A deux decimales, la moitie au pair (des jours que le bulletin dit au centieme : aucun texte deposé ne dit cet
    /// arrondi).
    pub fn a_deux_decimales(self) -> Jours {
        Jours(self.0.round_dp(2))
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

impl Unites {
    /// Une unite : la quantite d'une ligne qui se paie en une fois (une prime, une indemnite).
    pub const UNE: Unites = Unites(Decimal::ONE);
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

au_montant_unitaire!(Heures, Cachets, Jours, Services, Semaines, Unites, Effectif);

impl Effectif {
    /// Personne.
    pub const ZERO: Effectif = Effectif(Decimal::ZERO);

    /// Un effectif qu'un texte fixe, en personnes (`250` : le seuil de la contribution supplementaire a l'apprentissage).
    pub const fn personnes(n: u32) -> Effectif {
        Effectif(Decimal::from_parts(n, 0, 0, false, 0))
    }

    /// Cet effectif `n` fois : effectif x compte = effectif. Un compte n'est pas une grandeur (« Le glossaire »).
    pub fn fois(self, n: usize) -> Effectif {
        Effectif(num::mul(self.0, Decimal::from(n)))
    }

    /// La part de cet effectif que le taux `t` en prend, exacte (6 % de l'effectif d'assujettissement : l'obligation
    /// d'emploi, D5212-2) : effectif x taux = effectif.
    pub fn fois_taux(self, t: Taux) -> Effectif {
        Effectif(num::div(num::mul(self.0, t.0), Decimal::ONE_HUNDRED))
    }

    /// Sa partie entiere, sans arrondi (l'obligation d'emploi « arrondie a l'unite inferieure », D5212-2).
    pub fn partie_entiere(self) -> Effectif {
        Effectif(self.0.trunc())
    }

    /// Tronque au centieme (CSS R. 130-1, I : « il n'est pas tenu compte de la fraction d'effectif au-delà de la deuxième
    /// décimale »).
    pub fn tronque_au_centieme(self) -> Effectif {
        Effectif(self.0.round_dp_with_strategy(2, RoundingStrategy::ToZero))
    }

    /// A deux decimales au plus, la moitie au pair : un effectif tel qu'une phrase le dit.
    pub fn a_deux_decimales(self) -> Effectif {
        Effectif(self.0.round_dp(2))
    }

    /// LE RAPPORT de cet effectif a `o` (les alternants sur l'effectif total : le quotient de la CSA) : effectif / effectif
    /// = coefficient ; `None` quand `o` est nul.
    pub fn rapport(self, o: Effectif) -> Option<Coefficient> {
        (!o.0.is_zero()).then(|| Coefficient(num::div(self.0, o.0)))
    }
}

impl MontantUnitaire {
    /// Zero.
    pub const ZERO: MontantUnitaire = MontantUnitaire(Decimal::ZERO);

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

    /// Le montant d'une unite reparti sur les jours `j` qu'elle compte (le salaire d'un mois sur ses jours ouvrables :
    /// le salaire d'un jour) : montant unitaire / jours = montant unitaire ; `None` quand `j` est nul.
    pub fn par_jour(self, j: Jours) -> Option<MontantUnitaire> {
        (!j.0.is_zero()).then(|| MontantUnitaire(num::div(self.0, j.0)))
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

/// LE NOMBRE ECRIT D'UNE CASE (`forme::NombreEcrit`) : sa grandeur, et la grandeur qu'on y ecrit, a sa porte.
impl crate::forme::NombreEcrit {
    /// LA GRANDEUR QU'IL DIT dans la case `case` (un taux, des jours, un montant), son echelle gardee : un nombre tel
    /// qu'il est, un texte tape par la seule conversion d'une saisie (`lu::tapee`) ; la porte de sa lecture. Ce qui ne se
    /// convertit pas se refuse en nommant la case.
    pub fn en<G: Grandeur>(&self, case: &str) -> crate::echec::R<G> {
        let d = match self {
            crate::forme::NombreEcrit::Entier(i) => Decimal::from(*i),
            crate::forme::NombreEcrit::Naturel(u) => Decimal::from(*u),
            crate::forme::NombreEcrit::Flottant(x) => *x,
            crate::forme::NombreEcrit::Texte(t) => return lu::tapee(t).map_err(|p| crate::echec::refus(crate::forme::case_refusee(case, &p))),
        };
        Ok(G::du(d))
    }

    /// LA GRANDEUR `g` ECRITE DANS UNE CASE (le taux AT/MP qu'une DSN reprise declare) : un nombre a virgule, dans sa
    /// forme la plus courte (`courte`) ; la porte de la case.
    pub fn de_grandeur<G: Grandeur>(g: G) -> crate::forme::NombreEcrit {
        crate::forme::NombreEcrit::Flottant(courte(g).en_decimal())
    }
}

impl crate::forme::Nombre {
    /// LA GRANDEUR QU'IL DIT (la quantite et le taux qu'une ligne d'un bulletin imprime, dans l'unite de sa rubrique),
    /// son echelle gardee : la porte de sa lecture.
    pub fn grandeur<G: Grandeur>(self) -> G {
        G::du(self.0)
    }
}

/// LES LECTURES D'UNE GRANDEUR AUTRES QUE LA SIENNE (`forme::decimal_en_nombre`) : un champ les dit par son attribut serde
/// (`…::deserialize`, `…::option::deserialize` pour un `Option`).
pub mod lu {
    use crate::grandeurs::Grandeur;
    use crate::num::{self, Decimal};

    /// LA GRANDEUR QU'UNE PERSONNE A TAPEE, la seule conversion d'une saisie (Leandre, 16/11 : « si la personne ecrit
    /// 1,5 %, on transforme en 1,5 »), son echelle gardee (« 2,50 » : `2.50`). Elle ne devine rien et n'admet que : des
    /// chiffres ; un signe moins en tete ; un seul separateur decimal, la virgule ou le point ; des blancs (l'espace,
    /// l'insecable, l'insecable fine) entre les groupes de trois chiffres de la partie entiere (« 1 500,50 ») ; le signe de
    /// l'unite de la grandeur (`Scelle::SIGNES` : « % » d'un taux, « € » d'un montant, « h » des heures) a la fin
    /// seulement, un blanc avant lui permis (« 1,5 % ») ; des blancs autour. Le separateur est toujours decimal (« 1.500 »
    /// : `1.500`) : les milliers ne se separent que d'un blanc. Tout le reste se refuse et dit pourquoi (« 1 5 », « 1e3 »,
    /// « 1_000 », « %5 », « 1h30 », le signe d'une autre unite).
    pub fn tapee<G: Grandeur>(t: &str) -> Result<G, String> {
        let cite = crate::fichiers::repr(t);
        let attendu = <G as crate::grandeurs::Scelle>::ATTENDU;
        let blanc = |c: char| c.is_whitespace() || matches!(c, '\u{a0}' | '\u{202f}');
        let illisible = || format!("{} ne se lit pas : on attend {}, écrit en chiffres avec une virgule pour les décimales (« 1 500,50 »)", cite, attendu);
        let mut net = t.trim_matches(blanc);
        if let Some(sans) = <G as crate::grandeurs::Scelle>::SIGNES.iter().find_map(|s| net.strip_suffix(s)) {
            net = sans.trim_end_matches(blanc);
        }
        if let Some(s) = <G as crate::grandeurs::Scelle>::SIGNES.iter().find(|s| net.contains(**s)) {
            return Err(format!("{} ne se lit pas : on attend {}, le signe « {} » à la fin seulement", cite, attendu, s));
        }
        if super::TOUS_LES_SIGNES.iter().any(|s| net.contains(s)) {
            return Err(format!("{} porte le signe d'une autre unité : on attend {}", cite, attendu));
        }
        let (moins, net) = match net.strip_prefix('-') {
            Some(reste) => ("-", reste),
            None => ("", net),
        };
        let separateurs = net.matches(['.', ',']).count();
        if separateurs > 1 {
            return Err(format!("{} porte plus d'un séparateur : on attend {}, avec une seule virgule pour les décimales, les milliers séparés d'une espace (« 1 500,50 »)", cite, attendu));
        }
        let (entier, decimales) = match net.split_once(['.', ',']) {
            Some((e, d)) => (e, Some(d)),
            None => (net, None),
        };
        let groupes: Vec<&str> = entier.split(blanc).collect();
        let chiffres = |g: &str| !g.is_empty() && g.chars().all(|c| c.is_ascii_digit());
        let groupes_justes = match groupes.split_first() {
            Some((premier, [])) => premier.is_empty() || chiffres(premier),
            Some((premier, suivants)) => chiffres(premier) && premier.len() <= 3 && suivants.iter().all(|g| chiffres(g) && g.len() == 3),
            None => false,
        };
        if !groupes_justes || decimales.is_some_and(|d| !chiffres(d)) || (entier.is_empty() && decimales.is_none()) {
            return Err(illisible());
        }
        let entier: String = groupes.concat();
        let ecrit = match decimales {
            Some(d) => format!("{}{}.{}", moins, if entier.is_empty() { "0" } else { &entier }, d),
            None => format!("{}{}", moins, entier),
        };
        Decimal::from_str_exact(&ecrit).map(<G as crate::grandeurs::Scelle>::du).map_err(|_| format!("{} est trop long : on attend {}", cite, attendu))
    }

    /// `tapee`, son decimal : la conversion que la lecture d'une saisie passe a `forme` (`forme::decimal_en_nombre`).
    pub(crate) fn tapee_en_decimal<G: Grandeur>(t: &str) -> Result<Decimal, String> {
        tapee::<G>(t).map(<G as crate::grandeurs::Scelle>::en_decimal)
    }

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

    /// LA GRANDEUR QU'UN NOMBRE ECRIT EN TEXTE DIT (`num::dec_texte`, comme `forme::decimal` lit un texte : « 12.50 »),
    /// son echelle gardee : la porte de sa lecture. Un texte illisible se refuse.
    pub fn en_texte<G: Grandeur>(t: &str) -> crate::echec::R<G> {
        num::dec_texte(t).map(<G as crate::grandeurs::Scelle>::du)
    }

    /// Comme `forme::decimal` : un nombre, ou un nombre ecrit en texte (une ligne de paie, une paie ecrite : le moteur
    /// les ecrit en texte) ; dans une saisie (la charge d'une route), un texte est ce qu'une personne a tape, converti par
    /// `tapee`.
    pub mod nombre_ou_texte {
        use crate::grandeurs::Grandeur;

        pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<G, D::Error> {
            crate::forme::decimal_ou_tape(d, super::tapee_en_decimal::<G>).map(<G as crate::grandeurs::Scelle>::du)
        }

        /// Une grandeur lue ainsi, la ou un champ ne se pose pas (un `Option`).
        struct Lue<G>(G);

        impl<'de, G: Grandeur> serde::Deserialize<'de> for Lue<G> {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Lue<G>, D::Error> {
                deserialize(d).map(Lue)
            }
        }

        /// Un `Option` lu ainsi : absent (avec `#[serde(default)]`) ou nul, il est vide.
        pub mod option {
            use crate::grandeurs::Grandeur;

            pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<Option<G>, D::Error> {
                <Option<super::Lue<G>> as serde::Deserialize>::deserialize(d).map(|o| o.map(|l| l.0))
            }
        }
    }

    /// UNE GRANDEUR DITE EN NOMBRE ENTIER, POSITIF (des jours deja indemnises) : lue comme la grandeur (un nombre ; dans
    /// une saisie, ce qui a ete tape), son echelle ramenee a zero (`entiere`) ; une autre se refuse a sa lecture.
    pub mod entiere {
        use crate::grandeurs::Grandeur;

        pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<G, D::Error> {
            let d = crate::forme::decimal_en_nombre(d, super::tapee_en_decimal::<G>)?;
            match crate::grandeurs::entiere(G::du(d)).filter(|g| !g.en_decimal().is_sign_negative()) {
                Some(g) => Ok(g),
                None => Err(serde::de::Error::custom(crate::forme::refus_du_tape(format!("{} n'est pas un nombre entier positif : on attend {}", crate::texte::nombre_dit(d), G::ATTENDU)))),
            }
        }

        /// Une grandeur lue ainsi, la ou un champ ne se pose pas (un `Option`).
        struct Lue<G>(G);

        impl<'de, G: Grandeur> serde::Deserialize<'de> for Lue<G> {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Lue<G>, D::Error> {
                deserialize(d).map(Lue)
            }
        }

        /// Un `Option` lu ainsi : absent (avec `#[serde(default)]`) ou nul, il est vide.
        pub mod option {
            use crate::grandeurs::Grandeur;

            pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<Option<G>, D::Error> {
                <Option<super::Lue<G>> as serde::Deserialize>::deserialize(d).map(|o| o.map(|l| l.0))
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
            Ok(dit(g))
        }

        /// La meme, telle qu'une phrase la cite (`12.50`).
        pub fn dit<G: Grandeur>(g: G) -> String {
            g.en_decimal().to_string()
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

    /// TELLE QUE LA NORME DSN L'ECRIT dans ses rubriques (cahier technique : « avec decimale », le point decimal) : un
    /// montant au centime, un taux a ses decimales. La seule ecriture des grandeurs de la DSN.
    pub mod norme {
        use crate::grandeurs::{Grandeur, Montant, Taux};
        use crate::num::Decimal;
        use rust_decimal::RoundingStrategy;

        /// Un montant au centime (`Montant::au_centime`), ses deux decimales ecrites (`12.30`) ; un zero s'ecrit sans
        /// signe (`0.00`, jamais `-0.00`).
        pub fn montant(m: Montant) -> String {
            let c = m.au_centime();
            if c.est_nul() { Montant::ZERO } else { c }.decimal().to_string()
        }

        /// Un taux a `decimales` decimales, le demi au pair (`Taux::au_pair`), toutes ecrites (`1.50`, `6.900`).
        pub fn taux(t: Taux, decimales: u32) -> String {
            let mut d = t.au_pair(decimales).decimal();
            d.rescale(decimales);
            d.to_string()
        }

        /// Une quantite (des heures, des jours, des cachets) au centieme, la moitie vers le haut, ses deux decimales
        /// ecrites (`151.67`, `2.00`) ; un zero s'ecrit sans signe (`0.00`, jamais `-0.00`). Son signe se pose avant
        /// (`Signe::de`).
        pub fn quantite<G: Grandeur>(q: G) -> String {
            let mut c = q.en_decimal().round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
            if c.is_zero() {
                c = Decimal::ZERO;
            }
            c.rescale(2);
            c.to_string()
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
