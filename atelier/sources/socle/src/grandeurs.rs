//! L'ARGENT ET LES QUANTITES : un montant, un taux, des heures, des cachets, des jours, des services, des semaines, des
//! unites d'un forfait, un coefficient, un montant unitaire (« Le glossaire »). Chacun porte un decimal exact, son echelle gardee (`24.0` reste `24.0`) ; il n'en sort
//! qu'au socle (`decimal`, interne), la ou une ecriture ou la norme l'exige. Seules les operations qui ont un sens
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

use crate::fraction::{Fraction, Moitie};
use crate::num::{self, Decimal};
use rust_decimal::{MathematicalOps, RoundingStrategy};

/// Ce que toute grandeur porte : un decimal. Il n'en sort qu'au socle (`decimal`, interne) ; ce trait ne sert
/// qu'aux ecritures (`ecrit`) et aux lectures (`lu`) de ce module, scelle (`scelle::Scelle` ne se nomme pas dehors).
pub trait Grandeur: Copy + scelle::Scelle {}

mod scelle {
    use crate::num::Decimal;

    /// LE JETON DES METHODES SCELLEES : il ne se fabrique qu'ici (`JETON`). Un trait scelle interdit de l'implementer,
    /// pas d'appeler ses methodes par un generique qui prend une grandeur (`B::du(a.en_decimal())` rouvrirait un montant
    /// en heures) : chacune exige ce jeton, que seul ce module donne. Hors du socle, une grandeur ne se rouvre pas.
    pub struct Jeton(());

    pub(super) const JETON: Jeton = Jeton(());

    pub trait Scelle {
        fn en_decimal(self, jeton: Jeton) -> Decimal;
        fn du(d: Decimal, jeton: Jeton) -> Self;
        /// Le signe de son unite, tel qu'une personne le tape apres le nombre (« % », « € ») ; aucun pour une grandeur
        /// qui n'en a pas.
        const SIGNES: &'static [&'static str];
        /// Ce qu'une case de cette grandeur attend, dit a qui la remplit (« un nombre d'heures »).
        const ATTENDU: &'static str;
        /// Ne se dit-elle qu'en nombre entier (des cachets) ?
        const ENTIERE: bool;
    }
}

use scelle::{Scelle, JETON};

/// LA GRANDEUR QUE DIT CE DECIMAL, toute grandeur, des cachets compris : les lectures et ecritures de ce module, qui
/// savent ce que le decimal est (un entier, une grandeur deja de ce nom).
fn de_decimal<G: Grandeur>(d: Decimal) -> G {
    G::du(d, JETON)
}

/// UNE GRANDEUR QUI SE FRACTIONNE : toute grandeur sauf les cachets, qui ne se comptent qu'en entiers (« 1,5 cachet,
/// ça n'existe pas »). Seule elle se prend d'un decimal sans refus (`du_decimal`, `forme::Nombre::grandeur`,
/// `forme::Court::grandeur`) ; des cachets se lisent par une porte qui refuse un nombre a virgule (`lue`).
pub trait Fractionnable: Grandeur {}

/// LE DECIMAL NU D'UNE GRANDEUR, aux seules ecritures et lectures du socle (une ecriture, la norme) : hors du socle, ni
/// lui ni `rust_decimal` ne se nomment.
pub(crate) fn decimal<G: Grandeur>(g: G) -> Decimal {
    g.en_decimal(JETON)
}

/// LA GRANDEUR QUE DIT CE DECIMAL, tel qu'il est (son echelle gardee) : une grandeur qui se fractionne, au socle seul.
pub(crate) fn du_decimal<G: Fractionnable>(d: Decimal) -> G {
    de_decimal::<G>(d)
}

/// LA GRANDEUR QUE DIT LE DECIMAL `d` LU A UNE PORTE (un fichier, une saisie, une piece) ; `cite` : le nombre tel que le
/// refus le cite. Des cachets a virgule se refusent : un nombre de cachets est entier.
fn lue<G: Grandeur>(d: Decimal, cite: &str) -> Result<G, String> {
    match G::ENTIERE && !d.fract().is_zero() {
        true => Err(format!("{} n'est pas un nombre entier : on attend {}", cite, G::ATTENDU)),
        false => Ok(de_decimal::<G>(d)),
    }
}

/// `lue`, le nombre cite tel qu'il est ecrit.
fn lue_du<G: Grandeur>(d: Decimal) -> Result<G, String> {
    lue(d, &crate::fichiers::repr(&d.to_string()))
}

/// UNE GRANDEUR QUI N'EST PAS NULLE : un diviseur que la division prend sans refus (`MontantUnitaire::par_jour`). Elle
/// se lit d'une grandeur, `None` quand elle est nulle (`de`), ou se fixe d'un texte (`Jours::non_nuls`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonNul<G: Grandeur>(G);

impl<G: Grandeur> NonNul<G> {
    /// Cette grandeur, si elle n'est pas nulle.
    pub fn de(g: G) -> Option<NonNul<G>> {
        (!crate::grandeurs::decimal(g).is_zero()).then_some(NonNul(g))
    }
}

/// Une grandeur par nom : son decimal, sa porte de sortie, l'addition et la soustraction de deux grandeurs de meme nom,
/// sa lecture.
macro_rules! grandeurs {
    ($($nom:ident, [$($signe:literal),*], $attendu:literal, $doc:literal, $sorte:ident;)*) => {
        /// Les signes d'unite de toutes les grandeurs : un signe d'une autre unite que celle de la case se refuse.
        const TOUS_LES_SIGNES: &[&str] = &[$($($signe,)*)*];
        $(
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $nom(Decimal);

        sorte_de_grandeur!($nom, $sorte);

        impl $nom {
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
            fn en_decimal(self, _: scelle::Jeton) -> Decimal {
                self.0
            }
            fn du(d: Decimal, _: scelle::Jeton) -> $nom {
                $nom(d)
            }
            const SIGNES: &'static [&'static str] = &[$($signe),*];
            const ATTENDU: &'static str = $attendu;
            const ENTIERE: bool = sorte_de_grandeur!(@entiere $sorte);
        }

        impl Grandeur for $nom {}

        impl crate::ecriture::EcritureDeCase for $nom {
            fn ecrire_la_case<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                ecrit::nombre_json::serialize(self, s)
            }
        }

        impl<'de> serde::Deserialize<'de> for $nom {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<$nom, D::Error> {
                crate::forme::decimal_en_nombre(d, lu::tapee_en_decimal::<$nom>).and_then(|x| lue_du(x).map_err(serde::de::Error::custom))
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

/// Ce qu'une grandeur est de plus selon sa sorte : une grandeur qui se fractionne (`fractionnable`) se prend d'un
/// decimal sans refus (`du_decimal`, `Fractionnable`) ; des cachets (`entiere`) ne se lisent qu'entiers (`lue`).
macro_rules! sorte_de_grandeur {
    ($nom:ident, fractionnable) => {
        impl Fractionnable for $nom {}
    };
    ($nom:ident, entiere) => {};
    (@entiere fractionnable) => {
        false
    };
    (@entiere entiere) => {
        true
    };
}

grandeurs! {
    Montant, ["€"], "un montant en euros", "UN MONTANT : une somme en euros (`1512.40`).", fractionnable;
    Taux, ["%"], "un taux en pourcent", "UN TAUX : un pourcentage applique a une base, ecrit en pourcent comme la norme et les baremes (`6.90` : 6,90 %).", fractionnable;
    Heures, ["h"], "un nombre d'heures", "DES HEURES : une quantite d'heures (`151.67`).", fractionnable;
    Cachets, [], "un nombre de cachets", "DES CACHETS : un nombre entier de cachets (`2`) ; chaque porte refuse un nombre a virgule (`lue`), et aucune operation n'en fait un.", entiere;
    Jours, [], "un nombre de jours", "DES JOURS : une quantite de jours (`4.92`).", fractionnable;
    Coefficient, [], "un nombre", "UN COEFFICIENT : un multiplicateur sans unite, ni taux ni montant (l'abattement de la CSG `0.9825`).", fractionnable;
    MontantUnitaire, ["€"], "un montant en euros", "UN MONTANT UNITAIRE : des euros par unite (par heure, par cachet, par jour : `1.50`) ; le « taux » d'une ligne de paie, le plafond ou le SMIC horaire.", fractionnable;
    Services, [], "un nombre de services", "DES SERVICES : une quantite de services de repetition (`14`).", fractionnable;
    Semaines, [], "un nombre de semaines", "DES SEMAINES : une quantite de semaines (`2`).", fractionnable;
    Unites, [], "un nombre d'unités", "DES UNITES : le compte d'un forfait (`3` repas a 15 €).", fractionnable;
    HeuresParUnite, ["h"], "un nombre d'heures", "DES HEURES PAR UNITE : les heures d'un service, d'un cachet, d'un forfait (`4` : 14 services de 4 h) ; quantite x heures par unite = heures.", fractionnable;
    JoursParSemaine, [], "un nombre de jours par semaine", "DES JOURS PAR SEMAINE : un rythme, les jours que chaque semaine donne (`1` : « 1 jour par semaine calendaire » d'anciennete, un preavis d'une convention) ; semaines x jours par semaine = jours.", fractionnable;
    Effectif, [], "un nombre de personnes", "UN EFFECTIF : un nombre de personnes, eventuellement moyen (`11.42`) : celui que l'Urssaf calcule, les beneficiaires de l'OETH, les alternants de la CSA, un seuil d'effectif.", fractionnable;
    MontantEnDevise, [], "un montant dans sa monnaie", "UN MONTANT EN DEVISE : une somme dans la monnaie de son lieu, qui n'est pas forcement l'euro (`120` dollars : l'indemnite journaliere de mission d'un lieu a l'etranger) ; sa monnaie se dit a cote (`droit::missions::CodeMonnaie`).", fractionnable;
    TauxDeChange, [], "un taux de change", "UN TAUX DE CHANGE : des euros pour une unite d'une monnaie (`0.92` : le taux de chancellerie d'une monnaie pour un lieu) ; montant en devise x taux de change = montant.", fractionnable;
}

impl TauxDeChange {
    /// L'euro pour l'euro : un montant en euros se prend tel quel.
    pub const UN: TauxDeChange = TauxDeChange(Decimal::ONE);
}

/// Un montant dans sa monnaie fois le taux de change de cette monnaie : des euros, exacts (`Montant::au_centime` les
/// fixe).
impl std::ops::Mul<TauxDeChange> for MontantEnDevise {
    type Output = Montant;
    fn mul(self, t: TauxDeChange) -> Montant {
        Montant(num::mul(self.0, t.0))
    }
}

/// UNE GRANDEUR SANS ZERO INUTILE (`12.50` devient `12.5`, `24.0` devient `24`, `-0` devient `0`), comme l'ecrit
/// `ecrit::sans_zero` : la forme canonique des empreintes et des oracles.
pub fn sans_zero<G: Grandeur>(g: G) -> G {
    de_decimal::<G>(crate::grandeurs::decimal(g).normalize())
}

/// LA GRANDEUR NULLE, son echelle nulle (`0`) : la quantite d'une ligne qu'aucune date ne compte.
pub fn nulle<G: Grandeur>() -> G {
    de_decimal::<G>(Decimal::ZERO)
}

/// UNE, son echelle nulle (`1`) : la quantite d'une ligne qui se paie en une fois.
pub fn une<G: Grandeur>() -> G {
    de_decimal::<G>(Decimal::ONE)
}

/// LA GRANDEUR AU CENTIEME, la moitie au pair, sans zero inutile (`2.125` : `2.12`, `4.50` : `4.5`) : ce qu'une phrase
/// dit d'une quantite (la designation d'une ligne de paie), dite par `texte::virgule` ; un taux horaire qu'un minimum
/// ou une ligne ramene a l'heure (`engagement::minima` ; aucun texte depose ne dit cet arrondi).
pub fn au_centieme_au_pair<G: Grandeur>(g: G) -> G {
    de_decimal::<G>(crate::grandeurs::decimal(g).round_dp(2).normalize())
}

/// LA GRANDEUR ENTIERE QU'ELLE EST, son echelle ramenee a zero (`4.0` jours : `4`) ; `None` : elle n'est pas entiere
/// (`4.5` jours). Des jours ou des cachets dits « en nombre entier » se tiennent ainsi.
pub fn entiere<G: Grandeur>(g: G) -> Option<G> {
    let d = crate::grandeurs::decimal(g);
    match d.fract().is_zero() {
        true => Some(de_decimal::<G>(d.trunc())),
        false => None,
    }
}

/// LA GRANDEUR, SES `n` DECIMALES ECRITES (`12.5` a deux : `12.50`) : son echelle portee a `n` quand elle est moindre,
/// sa valeur la meme ; une ecriture, pas un arrondi (une grandeur ecrite plus fine reste telle quelle). Chacune apres son
/// arrondi : le coefficient de la reduction generale et les mois d'un preavis qu'une convention dit en jours (quatre),
/// le taux d'une ligne 23 du bordereau et le taux patronal reel d'une provision de conges (deux), un taux de PAS
/// (`aux_centiemes`).
pub fn decimales_ecrites<G: Grandeur>(g: G, n: u32) -> G {
    let mut d = crate::grandeurs::decimal(g);
    if d.scale() < n {
        d.rescale(n);
    }
    de_decimal::<G>(d)
}

/// LA GRANDEUR A DEUX DECIMALES ECRITES qu'elle est (`decimales_ecrites`, `5.5` : `5.50`) ; `None` : elle est ecrite
/// plus fine que le centieme (`5.505`, `5.500`), comme `ecrit::centimes` la refuse, ou elle porte un signe moins
/// (`-0.00` compris). Un taux de PAS se tient ainsi.
pub fn aux_centiemes<G: Grandeur>(g: G) -> Option<G> {
    let d = crate::grandeurs::decimal(g);
    if d.scale() > 2 || d.is_sign_negative() {
        return None;
    }
    Some(decimales_ecrites(g, 2))
}

/// UNE GRANDEUR SANS ZERO INUTILE, UNE DECIMALE AU MOINS (`24` s'ecrit `24.0`, `12.50` `12.5`), comme l'ecrit
/// `num::court` et la lit `lu::court` : celle d'un taux ou d'heures qu'une paie garde.
pub fn courte<G: Grandeur>(g: G) -> G {
    de_decimal::<G>(num::court(crate::grandeurs::decimal(g)))
}

/// UNE GRANDEUR REPARTIE EN `n` PARTS, le reste sur les premieres (Leandre, 25/11/2026, la regle « A » : 10 h sur 3 jours,
/// 4 · 3 · 3 ; 4 cachets sur 3 jours, 2 · 1 · 1) : chaque part a la partie entiere du quotient, les premieres une unite
/// de plus tant qu'il en reste une, et ce qui reste de moins d'une unite va a la suivante. Les parts, sans zero inutile,
/// font la grandeur ; une grandeur negative se repartit de meme, sous son signe. Aucune part pour `n` nul.
pub fn repartie<G: Grandeur>(g: G, n: usize) -> Vec<G> {
    if n == 0 {
        return Vec::new();
    }
    let d = decimal(g);
    let a = d.abs();
    let parts = Decimal::from(n);
    let base = num::div(a, parts).trunc();
    let mut reste = num::sub(a, num::mul(base, parts));
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let plus = reste.min(Decimal::ONE);
        reste = num::sub(reste, plus);
        let part = num::add(base, plus).normalize();
        out.push(de_decimal::<G>(if d.is_sign_negative() { -part } else { part }));
    }
    out
}

/// Montant unitaire x coefficient = montant unitaire (la moitie du salaire journalier).
impl std::ops::Mul<Coefficient> for MontantUnitaire {
    type Output = MontantUnitaire;
    fn mul(self, c: Coefficient) -> MontantUnitaire {
        MontantUnitaire(num::mul(self.0, c.0))
    }
}

/// UNE GRANDEUR DIVISEE PAR UN COEFFICIENT NON NUL : grandeur / coefficient = grandeur de meme nom (une valeur livree
/// qu'un conseil cite sur trente et un jours, `routes::gestes::conseils`).
macro_rules! divisee_par_un_coefficient {
    ($($g:ident),*) => {
        $(
            impl std::ops::Div<NonNul<Coefficient>> for $g {
                type Output = $g;
                fn div(self, c: NonNul<Coefficient>) -> $g {
                    $g(num::div(self.0, c.0 .0))
                }
            }
        )*
    };
}

divisee_par_un_coefficient!(Montant, MontantUnitaire, Taux, Coefficient);

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
        let d = crate::grandeurs::decimal(g);
        match self {
            Signe::Moins if !d.is_zero() => de_decimal::<G>(-d),
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
        Montant(num::div(num::mul(self.0, part.en_decimal(JETON)), tout.en_decimal(JETON)))
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

    /// Un coefficient qu'un texte fixe, en dixiemes (`6` : 0,6, le diviseur du parametre T de la reduction generale).
    pub const fn dixiemes(n: u32) -> Coefficient {
        Coefficient(Decimal::from_parts(n, 0, 0, false, 1))
    }

    /// Un coefficient qu'un texte fixe, en dix-milliemes (`9825` : 0,9825, la part du brut soumise a la CSG apres
    /// l'abattement de 1,75 % pour frais professionnels).
    pub const fn dix_milliemes(n: u32) -> Coefficient {
        Coefficient(Decimal::from_parts(n, 0, 0, false, 4))
    }

    /// Un coefficient qu'un texte fixe, en centiemes (`65` : 65 %).
    pub const fn centiemes(n: u32) -> Coefficient {
        Coefficient(Decimal::from_parts(n, 0, 0, false, 2))
    }

    /// Une fraction qu'un texte fixe (`2` sur `3` : les deux tiers de D1226-1, 2°), exacte a vingt-huit chiffres
    /// (`num::div`).
    pub fn fraction(numerateur: u32, denominateur: std::num::NonZeroU32) -> Coefficient {
        Coefficient(num::div(Decimal::from(numerateur), Decimal::from(denominateur.get())))
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

    /// A quatre decimales au plus, la moitie vers le haut : le coefficient de la reduction generale (`droit::reduction`).
    pub fn au_dix_millieme(self) -> Coefficient {
        Coefficient(self.0.round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero))
    }

    /// Sa partie entiere, sans arrondi (un multiple de plafonds du bareme : `8` pour huit plafonds) : la partie entiere
    /// d'un coefficient est un coefficient (tronquer n'ajoute pas d'unite).
    pub fn entier(self) -> Coefficient {
        Coefficient(self.0.trunc())
    }

    /// A `n` DECIMALES AU PLUS, LA MOITIE AU PAIR : un facteur que la visee d'un montant cherche (`engagement::viser`, a
    /// vingt decimales ; dit a six), un ecart relatif (a quatre), la part recue d'une mise a jour (a six), les mois d'un
    /// preavis qu'une convention dit en jours (a quatre, comme l'arithmetique des clauses les fixe). Aucun texte deposé
    /// ne dit cet arrondi.
    pub fn a_decimales(self, n: u32) -> Coefficient {
        Coefficient(self.0.round_dp(n))
    }

    /// Ce coefficient ecrit en taux, en pourcent (`0.10` donne `10.00`) : coefficient x 100 = taux.
    pub fn en_taux(self) -> Taux {
        Taux(num::mul(self.0, Decimal::from(100)))
    }

    /// CE COEFFICIENT EN MOIS DE CALENDRIER ENTIERS (les mois d'un preavis, que son terme compte : `Jour::apres`), ou
    /// pourquoi il ne l'est pas (`MoisNonEntiers`).
    pub fn en_mois_entiers(self) -> Result<crate::date::Duree<crate::date::EnMois>, MoisNonEntiers> {
        use rust_decimal::prelude::ToPrimitive;
        match (self.0.fract().is_zero(), self.0.to_u32()) {
            (false, _) => Err(MoisNonEntiers::Fraction),
            (true, Some(n)) => Ok(crate::date::Duree::mois(n)),
            (true, None) => Err(MoisNonEntiers::HorsDuCalendrier),
        }
    }
}

/// POURQUOI UN COEFFICIENT N'EST PAS UN NOMBRE ENTIER DE MOIS DE CALENDRIER (`Coefficient::en_mois_entiers`) : une
/// fraction (`1.5`), ou un entier que le calendrier ne compte pas (un negatif, un nombre trop grand).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoisNonEntiers {
    Fraction,
    HorsDuCalendrier,
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

    /// Un taux ecrit en milliemes de point (`2836` : 2,836 %), ses trois decimales ecrites.
    pub const fn milliemes_de_point(n: u32) -> Taux {
        Taux(Decimal::from_parts(n, 0, 0, false, 3))
    }

    /// Au centieme de point, la moitie vers zero (sans zero ajoute : `3.9` reste `3.9`) : la part salariale de la
    /// retraite complementaire en tranche 1 (`droit::bareme::parts_t1`).
    pub fn au_centieme_la_moitie_vers_zero(self) -> Taux {
        Taux(self.0.round_dp_with_strategy(2, RoundingStrategy::MidpointTowardZero))
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

/// Taux x coefficient = taux (le taux AT/MP par le facteur d'une ligne, une part d'un taux : la part salariale de la
/// retraite complementaire en tranche 1).
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

    /// Des cachets qu'un texte fixe, entiers (`5` : l'engagement a la semaine d'un artiste de la 3097).
    pub const fn fixes(n: u32) -> Cachets {
        Cachets(Decimal::from_parts(n, 0, 0, false, 0))
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

    /// Des jours qu'un texte fixe, entiers (`21` : les jours d'un mois incomplet d'un artiste, CCN 1285, art. X.3).
    pub const fn fixes(n: u32) -> Jours {
        Jours(Decimal::from_parts(n, 0, 0, false, 0))
    }

    /// Des jours qu'un texte fixe, entiers et jamais nuls (la semaine de 5 jours d'un artiste de la 3097, titre III) :
    /// un diviseur.
    pub const fn non_nuls(n: std::num::NonZeroU32) -> NonNul<Jours> {
        NonNul(Jours::fixes(n.get()))
    }

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

/// Les jours que des semaines donnent a tant de jours chacune (« 1 jour par semaine calendaire » d'anciennete : un
/// preavis de la convention) : semaines x jours par semaine = jours.
impl std::ops::Mul<JoursParSemaine> for Semaines {
    type Output = Jours;
    fn mul(self, par_semaine: JoursParSemaine) -> Jours {
        Jours(num::mul(self.0, par_semaine.0))
    }
}

impl Jours {
    /// CES JOURS EN MOIS DE TRENTE JOURS (un preavis qu'une convention dit en jours) : jours / 30 = des mois de salaire,
    /// un coefficient, exact (`num::div`) ; le diviseur est fixe, la division se fait toujours.
    pub fn en_mois_de_trente_jours(self) -> Coefficient {
        Coefficient(num::div(self.0, Decimal::from(30)))
    }
}

impl Unites {
    /// Une unite : la quantite d'une ligne qui se paie en une fois (une prime, une indemnite).
    pub const UNE: Unites = Unites(Decimal::ONE);

    /// Ces unites reparties egalement sur `n` jours (la quantite d'une ligne de frais a l'etranger : celle d'un jour) :
    /// unites / compte = unites. Un compte n'est pas une grandeur (« Le glossaire ») : il vient d'un `len()`, jamais nul
    /// (l'appelant le refuse).
    pub fn reparties_sur(self, n: usize) -> Unites {
        Unites(num::div(self.0, Decimal::from(n)))
    }

    /// Ces unites `n` fois (la quantite d'un jour fois les jours d'une ligne) : unites x compte = unites. Un compte n'est
    /// pas une grandeur (« Le glossaire »).
    pub fn fois(self, n: usize) -> Unites {
        Unites(num::mul(self.0, Decimal::from(n)))
    }
}

impl Montant {
    /// Ce montant pour chacune des unites `u` d'un forfait (ce qu'une ligne de frais paie au-dela de sa limite, sur ses
    /// repas ou ses nuitees) : montant / unites = montant unitaire ; `None` quand `u` est nul.
    pub fn par_unite(self, u: Unites) -> Option<MontantUnitaire> {
        (!u.0.is_zero()).then(|| MontantUnitaire(num::div(self.0, u.0)))
    }

    /// Les heures que ce montant paie a `par_heure` l'heure : montant / montant unitaire = heures ; `None` quand
    /// `par_heure` est nul.
    pub fn en_heures(self, par_heure: MontantUnitaire) -> Option<Heures> {
        (!par_heure.0.is_zero()).then(|| Heures(num::div(self.0, par_heure.0)))
    }
}

impl HeuresParUnite {
    /// Zero heure par unite (une indemnite, le maintien : ce n'est pas du travail).
    pub const ZERO: HeuresParUnite = HeuresParUnite(Decimal::ZERO);

    /// Des heures par unite qu'un texte fixe, entieres (`35` : la semaine de 35 h).
    pub const fn fixes(n: u32) -> HeuresParUnite {
        HeuresParUnite(Decimal::from_parts(n, 0, 0, false, 0))
    }

    /// Des heures par unite qu'un texte fixe, en dixiemes d'heure (`315` : 31,5 h, la semaine de 4,5 journees de 7 h).
    pub const fn dixiemes(n: u32) -> HeuresParUnite {
        HeuresParUnite(Decimal::from_parts(n, 0, 0, false, 1))
    }
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

    /// Des heures qu'un texte fixe, entieres (`10` : la duree quotidienne maximale de travail effectif, L. 3121-18).
    pub const fn fixes(n: u32) -> Heures {
        Heures(Decimal::from_parts(n, 0, 0, false, 0))
    }

    /// Des heures qu'un texte fixe, en centiemes d'heure (`15167` : 151,67 h, la duree legale d'un mois, L. 3121-27).
    pub const fn centiemes(n: u32) -> Heures {
        Heures(Decimal::from_parts(n, 0, 0, false, 2))
    }

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

    /// CES HEURES POUR CHACUNE DES `u` UNITES d'une quantite (des services, des journees, des semaines), qui peut etre
    /// a virgule (8 h sur 2 services : 4 h le service ; 3,62 h sur une demi-journee : 7,24 h) : heures / quantite =
    /// heures par unite ; `None` quand `u` est nulle. `par_jour` partage sur un compte entier de jours.
    pub fn par_unite<G: Grandeur>(self, u: G) -> Option<HeuresParUnite> {
        let u = decimal(u);
        (!u.is_zero()).then(|| HeuresParUnite(num::div(self.0, u)))
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

    /// Un milliardieme de personne (`0.000000001`) : l'ecart qu'une moyenne d'effectifs laisse sous un centieme qu'elle
    /// atteint juste (un tiers fois trois).
    pub const UN_MILLIARDIEME: Effectif = Effectif(Decimal::from_parts(1, 0, 0, false, 9));

    /// Cet effectif reparti egalement sur `n` (les jours d'un mois, les mois d'une annee : une moyenne) : effectif / compte
    /// = effectif. Un compte n'est pas une grandeur (« Le glossaire ») : il vient d'un `len()`, jamais nul (l'appelant le
    /// refuse).
    pub fn reparti_sur(self, n: usize) -> Effectif {
        Effectif(num::div(self.0, Decimal::from(n)))
    }

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

/// Effectif x coefficient = effectif (une personne a due proportion de ses jours ou de sa duree contractuelle, CSS
/// R. 130-1, II).
impl std::ops::Mul<Coefficient> for Effectif {
    type Output = Effectif;
    fn mul(self, c: Coefficient) -> Effectif {
        Effectif(num::mul(self.0, c.0))
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

    /// Sa partie entiere, sans arrondi (le plafond horaire de la Securite sociale, que le bareme lit en euros entiers).
    pub fn partie_entiere(self) -> MontantUnitaire {
        MontantUnitaire(self.0.trunc())
    }

    /// Ce montant d'une unite pour chacune des heures `h` qu'elle compte (le salaire mensuel sur les heures par mois : le
    /// taux horaire) : montant unitaire / heures = montant unitaire ; `None` quand `h` est nul.
    pub fn par_heure(self, h: Heures) -> Option<MontantUnitaire> {
        (!h.0.is_zero()).then(|| MontantUnitaire(num::div(self.0, h.0)))
    }

    /// A quatre decimales, la moitie au pair : le taux horaire majore d'une heure en plus (`engagement::heures_en_plus` ;
    /// aucun texte deposé ne dit cet arrondi).
    pub fn a_quatre_decimales(self) -> MontantUnitaire {
        MontantUnitaire(self.0.round_dp(4))
    }

    /// Le montant d'une unite reparti sur les jours `j` qu'elle compte (le salaire d'un mois sur ses jours ouvrables :
    /// le salaire d'un jour ; la semaine de 5 jours d'un artiste de la 3097 : le prix d'un jour) : montant unitaire /
    /// jours = montant unitaire, exact. Des jours non nuls : `NonNul::de`, ou `Jours::non_nuls` pour ceux qu'un texte
    /// fixe.
    pub fn par_jour(self, j: NonNul<Jours>) -> MontantUnitaire {
        MontantUnitaire(num::div(self.0, j.0 .0))
    }

    /// Le prix d'une unite pour chacune des heures `h` par unite qu'elle paie (une semaine de 35 h, la valeur d'une
    /// grille des minima pour ses heures : le taux horaire) : montant unitaire / heures par unite = montant unitaire ;
    /// `None` quand `h` est nul. Un mensuel se ramene aux heures du mois, des `Heures`, par `par_heure`.
    pub fn par_heure_de_l_unite(self, h: HeuresParUnite) -> Option<MontantUnitaire> {
        (!h.0.is_zero()).then(|| MontantUnitaire(num::div(self.0, h.0)))
    }

    /// Au centime, vers zero : un minimum conventionnel ramene a l'heure ou au cachet (`engagement::minima` ; aucun
    /// texte depose ne dit cet arrondi).
    pub fn au_centime_vers_zero(self) -> MontantUnitaire {
        MontantUnitaire(self.0.round_dp_with_strategy(2, RoundingStrategy::ToZero))
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

/// UNE TAILLE DE FICHIER, EN OCTETS (« Le glossaire », taille d'un fichier) : une image posee et son poids vise
/// (`routes::identite`), ce que la sauvegarde envoie et ce que la restauration redescend, une archive, une capture, ce
/// qu'une mise a jour recoit. Un fichier l'ecrit en octets (sa serde : un nombre) ; une saisie la tape en Ko (`ko`,
/// « 20,5 ») ; elle se dit d'une seule facon (`Display`). 1 Ko = 1 024 octets, 1 Mo = 1 024 Ko.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Octets(u64);

/// Au-dela de mille Ko, une taille se dit en Mo.
const MILLE_KO: Octets = Octets::en_ko(1000);

impl Octets {
    /// Rien.
    pub const ZERO: Octets = Octets(0);

    /// `ko` Ko.
    pub const fn en_ko(ko: u64) -> Octets {
        Octets(ko * 1024)
    }

    /// LA TAILLE DE `octets`, a la porte qui les tient (un fichier lu, une image allegee).
    pub fn de(octets: &[u8]) -> Octets {
        Octets(octets.len() as u64)
    }

    /// LA TAILLE D'UN FICHIER DU DISQUE, que le systeme dit (`std::fs::metadata`).
    pub fn du_disque(m: &std::fs::Metadata) -> Octets {
        Octets(m.len())
    }

    /// LA TAILLE QU'UN TIERS DIT EN OCTETS (la taille d'un fichier que Google rend, la longueur d'une reponse HTTP), a la
    /// porte qui la lit.
    pub const fn lue(octets: u64) -> Octets {
        Octets(octets)
    }

    /// LA TAILLE QU'UN TIERS ECRIT EN CHIFFRES SEULS (la longueur qu'une reponse HTTP annonce, `Content-Length`), a la
    /// porte qui la lit ; `None` pour tout autre texte (`num::entier_de_chiffres`).
    pub fn ecrite(t: &str) -> Option<Octets> {
        crate::num::entier_de_chiffres(t).map(|n| Octets(n.unsigned_abs()))
    }

    /// LA TAILLE QUE `fabrique_pdf` COMPTE (une longueur en memoire : l'image recue avant d'etre allegee), a sa porte.
    pub fn de_longueur(n: usize) -> Octets {
        Octets(n as u64)
    }

    /// SON NOMBRE D'OCTETS, a la porte qui le prend (l'allegement d'une image : `fabrique_pdf::alleger`).
    pub fn en_usize(self) -> usize {
        match usize::try_from(self.0) {
            Ok(n) => n,
            Err(_) => unreachable!("une taille de fichier tient en memoire : {} octets", self.0),
        }
    }

    /// La part de `tout` qu'elle est (ce qu'une mise a jour a recu de son archive) ; `None` quand `tout` est nul.
    pub fn rapport(self, tout: Octets) -> Option<Coefficient> {
        (tout.0 > 0).then(|| Coefficient(Decimal::from(self.0) / Decimal::from(tout.0)))
    }

    /// Son nombre de Ko, au centieme.
    fn ko_dits(self) -> Decimal {
        (Decimal::from(self.0) / Decimal::from(1024)).round_dp(2)
    }

    /// Son nombre de Mo, au dixieme.
    fn mo_dits(self) -> Decimal {
        (Decimal::from(self.0) / Decimal::from(1024 * 1024)).round_dp(1)
    }

    /// LUE D'UN NOMBRE DE KO, jamais negatif (`#[serde(deserialize_with = "socle::grandeurs::Octets::ko")]`) : un nombre ;
    /// dans une saisie (`forme::lire_saisie`), aussi ce qu'une personne a tape (« 20,5 », lu par `lu::ko_tapes`) ; un
    /// octet entame ne compte pas.
    pub fn ko<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Octets, D::Error> {
        let ko = crate::forme::decimal_en_nombre(d, lu::ko_tapes)?;
        match rust_decimal::prelude::ToPrimitive::to_u64(&(ko * Decimal::from(1024)).trunc()) {
            Some(n) => Ok(Octets(n)),
            None => Err(serde::de::Error::custom(format!("{} n'est pas un poids : on attend un nombre de Ko positif", crate::fichiers::repr(&ko.to_string())))),
        }
    }

    /// `ko`, facultatif : `#[serde(default, deserialize_with = "socle::grandeurs::Octets::ko_opt")]`.
    pub fn ko_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Octets>, D::Error> {
        /// Un poids lu par `ko`, la ou un champ ne se pose pas (un `Option`).
        struct Lu(Octets);
        impl<'de> serde::Deserialize<'de> for Lu {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Lu, D::Error> {
                Octets::ko(d).map(Lu)
            }
        }
        <Option<Lu> as serde::Deserialize>::deserialize(d).map(|l| l.map(|l| l.0))
    }

    /// ECRITE EN KO, facultative (`null` quand elle n'est pas dite) : `#[serde(serialize_with =
    /// "socle::grandeurs::Octets::ecrite_en_ko_opt")]`.
    pub fn ecrite_en_ko_opt<S: serde::Serializer>(o: &Option<Octets>, s: S) -> Result<S::Ok, S::Error> {
        crate::forme::ecrire_a_virgule_opt(&o.map(Octets::ko_dits), s)
    }
}

impl std::ops::Add for Octets {
    type Output = Octets;
    fn add(self, o: Octets) -> Octets {
        Octets(self.0 + o.0)
    }
}

impl std::ops::AddAssign for Octets {
    fn add_assign(&mut self, o: Octets) {
        self.0 += o.0;
    }
}

impl std::iter::Sum for Octets {
    fn sum<I: Iterator<Item = Octets>>(i: I) -> Octets {
        i.fold(Octets::ZERO, |a, b| a + b)
    }
}

/// LA SEULE FACON DE DIRE UNE TAILLE : en Ko au centieme (« 20 Ko », « 20,5 Ko »), en Mo au dixieme au-dela de mille Ko
/// (« 1,5 Mo »).
impl std::fmt::Display for Octets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if *self < MILLE_KO {
            write!(f, "{} Ko", crate::texte::nombre_dit(self.ko_dits()))
        } else {
            write!(f, "{} Mo", crate::texte::nombre_dit(self.mo_dits()))
        }
    }
}

/// UN ECART EN OCTETS DANS UN FICHIER LU (« Le glossaire ») : la position d'un octet depuis le debut du fichier, ou
/// l'ecart entre deux positions (le debut et la taille du repertoire d'une archive zip, le decalage d'un de ses membres,
/// PKWARE APPNOTE 4.3, que `envois::retours` lit). Signe : une archive abimee place un membre avant le debut du fichier,
/// ce que la lecture refuse en le disant (« negative seek value -5 »). Il se lit d'un champ de l'archive (`lu`) ou d'une
/// position dans des octets en memoire (`de_position`), s'ajoute, se retranche, se compare et s'ecrit en nombre
/// (`Display`) ; il ne redevient une position en memoire que s'il ne precede pas le debut (`au_plus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EcartEnOctets(i128);

impl EcartEnOctets {
    /// Aucun ecart.
    pub const ZERO: EcartEnOctets = EcartEnOctets(0);

    /// L'ECART QU'UN CHAMP D'UN FICHIER ECRIT, sans signe, a la porte qui le lit.
    pub fn lu(n: u64) -> EcartEnOctets {
        EcartEnOctets(i128::from(n))
    }

    /// LA POSITION `n` dans des octets en memoire (ou une longueur en memoire).
    pub fn de_position(n: usize) -> EcartEnOctets {
        EcartEnOctets(n as i128)
    }

    /// LA POSITION QU'IL EST, dans des octets en memoire de longueur `longueur` : lui, ou `longueur` s'il la depasse ;
    /// `None` s'il precede le debut.
    pub fn au_plus(self, longueur: usize) -> Option<usize> {
        (self.0 >= 0).then(|| if self.0 > longueur as i128 { longueur } else { self.0 as usize })
    }
}

impl std::ops::Add for EcartEnOctets {
    type Output = EcartEnOctets;
    fn add(self, o: EcartEnOctets) -> EcartEnOctets {
        EcartEnOctets(self.0.saturating_add(o.0))
    }
}

impl std::ops::AddAssign for EcartEnOctets {
    fn add_assign(&mut self, o: EcartEnOctets) {
        *self = *self + o;
    }
}

impl std::ops::Sub for EcartEnOctets {
    type Output = EcartEnOctets;
    fn sub(self, o: EcartEnOctets) -> EcartEnOctets {
        EcartEnOctets(self.0.saturating_sub(o.0))
    }
}

/// En nombre d'octets, signe (« 120 », « -5 »).
impl std::fmt::Display for EcartEnOctets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl crate::forme::Nombre {
    /// LA GRANDEUR QU'IL DIT (la quantite et le taux qu'une ligne d'un bulletin imprime, dans l'unite de sa rubrique),
    /// son echelle gardee : la porte de sa lecture.
    pub fn grandeur<G: Fractionnable>(self) -> G {
        de_decimal::<G>(self.0)
    }

    /// LES CACHETS QU'IL DIT (la quantite qu'une ligne d'un bulletin imprime dans une rubrique qui compte des cachets) :
    /// la porte de leur lecture ; un nombre a virgule se refuse (`lue`), et le refus le cite tel qu'imprime.
    pub fn cachets(self) -> Result<Cachets, String> {
        lue(self.0, &crate::fichiers::repr(&crate::texte::nombre_dit(self.0)))
    }
}

/// DES MOIS DE SALAIRE : ce qu'une indemnite de rupture compte en salaire de reference (R1234-2 : « un quart de mois de
/// salaire par année d'ancienneté » ; le palier, le plafond, le plancher d'une convention). Exacts, rien ne se perd avant
/// le centime (`fraction::Fraction`) : un nombre de mois tel qu'un texte le dit (`Deserialize`), ou compte sur une
/// anciennete (ses annees fois la part de mois de salaire par annee : `date::Anciennete::par_annee_dans_la_tranche`). Ils
/// s'ajoutent, se comparent, se majorent (mois de salaire x coefficient) ; salaire de reference x mois de salaire = le
/// montant au centime, leur seule porte vers l'argent ; ils s'ecrivent a vingt-huit chiffres (`ecrits`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MoisDeSalaire(Fraction);

impl MoisDeSalaire {
    /// Aucun.
    pub const ZERO: MoisDeSalaire = MoisDeSalaire(Fraction::ZERO);

    /// Les mois de salaire `f`, comptes sur une anciennete : `date::Anciennete`.
    pub(crate) fn comptes(f: Fraction) -> MoisDeSalaire {
        MoisDeSalaire(f)
    }

    /// Ses mois, ecrits a vingt-huit chiffres, sans zero inutile quand ils tombent juste (`0.5`,
    /// `2.083333333333333333333333333`) : ce qu'un oracle en cite.
    pub fn ecrits(self) -> String {
        self.0.en_decimal().to_string()
    }

    /// `ecrits`, sans aucun zero inutile (`1.0` : `1`) : ce qu'une phrase en dit (le palier d'un depart a la retraite).
    pub fn ecrits_sans_zero(self) -> String {
        self.0.en_decimal().normalize().to_string()
    }
}

/// Mois de salaire + mois de salaire = mois de salaire, exacts.
impl std::ops::Add for MoisDeSalaire {
    type Output = MoisDeSalaire;
    fn add(self, o: MoisDeSalaire) -> MoisDeSalaire {
        MoisDeSalaire(self.0 + o.0)
    }
}

/// Mois de salaire x coefficient = mois de salaire (une majoration de l'indemnite : 1 + son pourcentage), exacts.
impl std::ops::Mul<Coefficient> for MoisDeSalaire {
    type Output = MoisDeSalaire;
    fn mul(self, c: Coefficient) -> MoisDeSalaire {
        MoisDeSalaire(self.0 * Fraction::du_decimal(c.0))
    }
}

/// Salaire de reference x mois de salaire = montant, AU CENTIME : la valeur exacte, arrondie une seule fois, la moitie
/// loin de zero (la regle de `Montant::au_centime`). Un montant au-dela de ce qu'un montant peut valoir (vingt-huit
/// chiffres : un salaire de reference absurde) se refuse, en le disant.
impl std::ops::Mul<MoisDeSalaire> for Montant {
    type Output = crate::echec::R<Montant>;
    fn mul(self, m: MoisDeSalaire) -> crate::echec::R<Montant> {
        match (Fraction::du_decimal(self.0) * m.0).arrondi(2, Moitie::LoinDeZero) {
            Some(d) => Ok(Montant(d)),
            None => Err(crate::refus!(
                "Le montant de {} € × {} mois de salaire dépasse ce qu'un montant peut valoir : le salaire de référence est à vérifier.",
                crate::texte::euros(self),
                m.ecrits_sans_zero()
            )),
        }
    }
}

/// Lus d'un nombre de mois, comme un coefficient (un palier, un plafond, un plancher d'un bareme), exactement.
impl<'de> serde::Deserialize<'de> for MoisDeSalaire {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<MoisDeSalaire, D::Error> {
        <Coefficient as serde::Deserialize>::deserialize(d).map(|c| MoisDeSalaire(Fraction::du_decimal(c.0)))
    }
}

/// UN RANG DANS UNE GRILLE DE `N`, numerotee de 1 a `N` (l'echelon d'un emploi de la CCNEAC, une grille de douze :
/// `droit::fiche::EchelonCcneac`). Il se lit d'un nombre entier, ou d'un texte tape dans une saisie (`lu`, `tape`, la
/// lecture d'un entier jamais negatif) ; hors de 1 a `N`, il se refuse en la phrase de sa notion (`hors`, qui recoit le
/// nombre tel qu'il s'ecrit). Il s'ecrit en nombre ; il prend sa ligne dans la grille (`ligne`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct RangDansUneGrille<const N: u8>(u8);

impl<const N: u8> RangDansUneGrille<N> {
    /// Le rang `n`, ou la phrase de `hors` hors de 1 a `N`.
    fn de(n: u64, hors: fn(&str) -> String) -> Result<RangDansUneGrille<N>, String> {
        match u8::try_from(n).ok().filter(|n| (1..=N).contains(n)) {
            Some(n) => Ok(RangDansUneGrille(n)),
            None => Err(hors(&n.to_string())),
        }
    }

    /// Le rang qu'une personne a tape (`texte`), ou le refus qui dit pourquoi ce n'en est pas un (`attendu` : ce que la
    /// case attend ; `hors` : la phrase d'un nombre hors de la grille).
    pub fn tape(texte: &str, attendu: &'static str, hors: fn(&str) -> String) -> crate::echec::R<RangDansUneGrille<N>> {
        lu::entier_tape(texte, attendu).and_then(|n| RangDansUneGrille::de(n, hors)).map_err(crate::echec::refus)
    }

    /// Le rang qu'une forme lit (un fichier, la charge d'une route) : un nombre entier, ou dans une saisie le texte tape
    /// (`lu::entier`).
    pub fn lu<'de, D: serde::Deserializer<'de>>(d: D, attendu: &'static str, hors: fn(&str) -> String) -> Result<RangDansUneGrille<N>, D::Error> {
        RangDansUneGrille::de(lu::entier(d, attendu)?, hors).map_err(serde::de::Error::custom)
    }

    /// SA LIGNE DANS `grille`, la grille de ses `N` rangs dans leur ordre (le premier rang, la premiere ligne) ; `None`
    /// quand la grille en compte moins que lui.
    pub fn ligne<T>(self, grille: &[T]) -> Option<&T> {
        grille.get(usize::from(self.0) - 1)
    }
}

impl<const N: u8> std::fmt::Display for RangDansUneGrille<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

/// UN DECALAGE DANS UNE SUITE QUI TOURNE : tant de crans apres (positif) ou avant (negatif) ; il se lit d'un nombre
/// entier signe (`1` le suivant, `-1` le precedent : « un autre » conseil du jour, `routes::gestes::conseils`). Il decale
/// l'element du jour d'une `SuiteQuiTourne` ; son nombre ne sort pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(transparent)]
pub struct DecalageDansUneSuite(i64);

/// UNE SUITE QUI TOURNE AU FIL DES JOURS : ses elements dans leur ordre (jamais aucun), dont chaque jour montre un. Le
/// pas de la rotation est le premier entier a partir de N x 0,618 qui est premier avec N : deux jours voisins ne tombent
/// pas sur deux elements voisins (la banque des conseils est rangee par domaine), et N jours de suite les montrent tous ;
/// un decalage (« un autre ») avance ou recule d'autant de crans, en tournant. Elle rend l'element (`du_jour`) : son rang
/// ne sort pas. Les conseils « Le saviez-vous ? » du jour (`routes::gestes::conseils`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteQuiTourne<T>(Vec<T>);

impl<T> SuiteQuiTourne<T> {
    /// La suite de ces elements, dans leur ordre ; `None` s'il n'y en a aucun.
    pub fn de(elements: Vec<T>) -> Option<SuiteQuiTourne<T>> {
        (!elements.is_empty()).then_some(SuiteQuiTourne(elements))
    }

    /// L'ELEMENT DU JOUR `jour`, decale de `decalage` : le rang `(ordinal x pas) mod N`, puis `(rang + decalage) mod N`
    /// (un decalage negatif recule, et passe du premier au dernier).
    pub fn du_jour(mut self, jour: &crate::date::Jour, decalage: Option<DecalageDansUneSuite>) -> T {
        let n = self.0.len();
        let mut pas = (n * 618).div_ceil(1000).max(1);
        while pgcd(pas, n) != 1 {
            pas += 1;
        }
        // Un compte tient en 128 bits signes ; le reste, de 0 a `n` exclu, est un compte, que seul l'element prend.
        let du_jour = i128::from(jour.ordinal()) * pas as i128;
        let decale = match decalage {
            Some(d) => du_jour.rem_euclid(n as i128) + i128::from(d.0),
            None => du_jour,
        };
        self.0.swap_remove(decale.rem_euclid(n as i128) as usize)
    }
}

/// Le plus grand diviseur commun de deux comptes.
fn pgcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        pgcd(b, a % b)
    }
}

impl crate::piece::EntierDUneZone {
    /// LA GRANDEUR QU'IL COMPTE, celle que sa place dit (des heures, des cachets, des jours), son echelle nulle : la
    /// porte de sa lecture. Un entier : des cachets aussi.
    pub fn en<G: Grandeur>(self) -> G {
        de_decimal::<G>(Decimal::from(self.0))
    }
}

impl crate::forme::Court {
    /// LA GRANDEUR QU'IL DIT, lu dans sa forme la plus courte avant de savoir sa place (un element d'un tableau ecrit
    /// `[bas, haut, taux]`) : la porte de sa lecture.
    pub fn grandeur<G: Fractionnable>(self) -> G {
        de_decimal::<G>(self.0)
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
        nombre_tape(t, <G as crate::grandeurs::Scelle>::SIGNES, <G as crate::grandeurs::Scelle>::ATTENDU).and_then(|d| super::lue(d, &crate::fichiers::repr(t)))
    }

    /// LE NOMBRE QU'UNE PERSONNE A TAPE, LA SEULE LECTURE D'UN TEXTE TAPE (`tapee`, `entier_tape`) : les regles de
    /// `tapee` ; `signes` : ceux de l'unite de la case ; `attendu` : ce qu'elle attend, dit dans le refus.
    fn nombre_tape(t: &str, signes: &[&str], attendu: &str) -> Result<Decimal, String> {
        let cite = crate::fichiers::repr(t);
        let blanc = |c: char| c.is_whitespace() || matches!(c, '\u{a0}' | '\u{202f}');
        let illisible = || format!("{} ne se lit pas : on attend {}, écrit en chiffres avec une virgule pour les décimales (« 1 500,50 »)", cite, attendu);
        let mut net = t.trim_matches(blanc);
        if let Some(sans) = signes.iter().find_map(|s| net.strip_suffix(s)) {
            net = sans.trim_end_matches(blanc);
        }
        if let Some(s) = signes.iter().find(|s| net.contains(**s)) {
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
        Decimal::from_str_exact(&ecrit).map_err(|_| format!("{} est trop long : on attend {}", cite, attendu))
    }

    /// UN NOMBRE ENTIER, JAMAIS NEGATIF, QU'UNE PERSONNE A TAPE (un compte, des jours de droit, une annee) : lu par
    /// `nombre_tape` (« 1 500 »), puis sa forme ; `attendu` : ce que la case attend. Aussi l'echelon tape d'une fiche
    /// (`droit::fiche::EchelonCcneac`).
    pub(crate) fn entier_tape(t: &str, attendu: &str) -> Result<u64, String> {
        let d = nombre_tape(t, &[], attendu)?;
        match d.fract().is_zero().then(|| rust_decimal::prelude::ToPrimitive::to_u64(&d.trunc())).flatten() {
            Some(n) => Ok(n),
            None => Err(pas_entier(t, attendu)),
        }
    }

    /// UN NOMBRE DE KO QU'UNE PERSONNE A TAPE (le poids vise d'une image, `Octets::ko`), lu par `nombre_tape`.
    pub(crate) fn ko_tapes(t: &str) -> Result<Decimal, String> {
        nombre_tape(t, &["Ko", "ko"], "un nombre de Ko")
    }

    /// Le refus d'un nombre `ecrit` (tape, ou un nombre JSON) qui n'est pas un entier jamais negatif.
    fn pas_entier(ecrit: &str, attendu: &str) -> String {
        format!("{} n'est pas un nombre entier positif : on attend {}", crate::fichiers::repr(ecrit), attendu)
    }

    /// LA LECTURE D'UN NOMBRE ENTIER, JAMAIS NEGATIF (un compte, des jours de droit, une annee, l'echelon d'une fiche) :
    /// dans une saisie (`forme::lire_saisie`, la charge d'une route), le texte qu'une personne a tape et le nombre JSON
    /// passent par la meme lecture, `entier_tape` (`3.0` vaut 3 comme « 3,0 » ; `1e3` se refuse comme « 1e3 ») ; dans un
    /// fichier, sa forme stricte : un nombre entier, jamais un texte. `attendu` : ce que la case attend.
    pub(crate) fn entier<'de, D: serde::Deserializer<'de>>(d: D, attendu: &'static str) -> Result<u64, D::Error> {
        struct Entier(&'static str);
        impl<'de> serde::de::Visitor<'de> for Entier {
            type Value = u64;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str(self.0)
            }
            fn visit_u64<E: serde::de::Error>(self, u: u64) -> Result<u64, E> {
                Ok(u)
            }
            fn visit_i64<E: serde::de::Error>(self, i: i64) -> Result<u64, E> {
                u64::try_from(i).map_err(|_| E::custom(pas_entier(&i.to_string(), self.0)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, m: A) -> Result<u64, A::Error> {
                let ecrit = crate::forme::texte_a_virgule(m)?;
                match crate::forme::en_saisie() {
                    true => entier_tape(&ecrit, self.0).map_err(serde::de::Error::custom),
                    false => Err(serde::de::Error::custom(pas_entier(&ecrit, self.0))),
                }
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<u64, E> {
                match crate::forme::en_saisie() {
                    true => entier_tape(s, self.0).map_err(E::custom),
                    false => Err(E::custom(format!("« {} » est écrit en texte : écrire le nombre sans guillemets", s))),
                }
            }
        }
        d.deserialize_any(Entier(attendu))
    }

    /// UN COMPTE (« combien de… », `usize` : les enfants couverts d'une complementaire sante) lu par `entier`.
    pub mod compte {
        /// Un compte facultatif : absent (avec `#[serde(default)]`) ou nul, il est vide.
        pub mod option {
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<usize>, D::Error> {
                /// Un compte lu ainsi, la ou un champ ne se pose pas (un `Option`).
                struct Lu(usize);
                impl<'de> serde::Deserialize<'de> for Lu {
                    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Lu, D::Error> {
                        let n = super::super::entier(d, "un nombre entier")?;
                        usize::try_from(n).map(Lu).map_err(serde::de::Error::custom)
                    }
                }
                <Option<Lu> as serde::Deserialize>::deserialize(d).map(|l| l.map(|l| l.0))
            }
        }
    }

    /// `tapee`, son decimal : la conversion que la lecture d'une saisie passe a `forme` (`forme::decimal_en_nombre`).
    pub(crate) fn tapee_en_decimal<G: Grandeur>(t: &str) -> Result<Decimal, String> {
        tapee::<G>(t).map(crate::grandeurs::decimal::<G>)
    }

    /// LA GRANDEUR QU'UN TEXTE D'UNE PIECE ECRIT (une case d'un formulaire, d'une attestation, d'un bulletin), a la
    /// francaise ou non (`forme::Nombre::a_virgule` : « 12,5 », « 12.50 »), son echelle gardee : la porte de sa lecture ;
    /// `None` : le texte n'est pas un nombre, ou pas un nombre de sa grandeur (des cachets a virgule, `super::lue`).
    pub fn texte<G: Grandeur>(t: &str) -> Option<G> {
        crate::forme::Nombre::a_virgule(t).and_then(|n| super::lue_du(n.0).ok())
    }

    /// LA GRANDEUR QU'UN FICHIER LIVRE ECRIT DANS UN TEXTE (l'operande d'un gabarit des conseils : `*12`, `/31`), le point
    /// decimal, son echelle gardee : la porte de sa lecture ; `Err` : pourquoi le texte n'est pas un nombre de sa grandeur.
    pub fn ecrite<G: Grandeur>(t: &str) -> Result<G, String> {
        t.parse::<Decimal>().map_err(|e| e.to_string()).and_then(|d| super::lue(d, &crate::fichiers::repr(t)))
    }

    /// LA GRANDEUR QU'UNE RUBRIQUE DE LA NORME ECRIT (une DSN : `151.67`), comme `forme::Nombre::de_la_norme` la lit, son
    /// echelle gardee : la porte de sa lecture ; `None` : le texte n'est pas un nombre, ou pas un nombre de sa grandeur
    /// (des cachets a virgule, `super::lue`).
    pub fn norme<G: Grandeur>(t: &str) -> Option<G> {
        crate::forme::Nombre::de_la_norme(t).and_then(|n| super::lue_du(n.0).ok())
    }

    /// LA GRANDEUR QU'UN TEXTE D'UNE PIECE ECRIT EN ENTIER (`num::entier_lu` : une zone a largeur fixe de l'AEM, `012`),
    /// son echelle nulle : la porte de sa lecture ; `None` : le texte n'est pas un entier, ou il deborde.
    pub fn texte_entier<G: Grandeur>(t: &str) -> Option<G> {
        match num::entier_lu(t).map(|n| Decimal::try_from_i128_with_scale(n, 0)) {
            Some(Ok(d)) => Some(super::de_decimal::<G>(d)),
            Some(Err(_)) | None => None,
        }
    }

    /// LE MONTANT QU'UNE ZONE D'UNE PIECE ECRIT EN CENTIMES ENTIERS (`num::entier_lu` : le brut de l'AEM, positions
    /// 659-668, `000123456`) : cent centimes font un euro, le meme entier a deux decimales ; la porte de sa lecture.
    /// `None` : le texte n'est pas un entier, ou il deborde.
    pub fn en_centimes(t: &str) -> Option<crate::grandeurs::Montant> {
        num::entier_lu(t).and_then(|c| Decimal::try_from_i128_with_scale(c, 2).ok()).map(crate::grandeurs::Montant)
    }

    /// LA GRANDEUR QU'UN NOMBRE ECRIT EN TEXTE DIT (`num::dec_texte`, comme `forme::decimal` lit un texte : « 12.50 »),
    /// son echelle gardee : la porte de sa lecture. Un texte illisible, ou qui n'est pas un nombre de sa grandeur (des
    /// cachets a virgule, `super::lue`), se refuse.
    pub fn en_texte<G: Grandeur>(t: &str) -> crate::echec::R<G> {
        num::dec_texte(t).and_then(|d| super::lue(d, &crate::fichiers::repr(t)).map_err(crate::echec::refus))
    }

    /// Comme `forme::decimal` : un nombre, ou un nombre ecrit en texte (une ligne de paie, une paie ecrite : le moteur
    /// les ecrit en texte) ; dans une saisie (la charge d'une route), un texte est ce qu'une personne a tape, converti par
    /// `tapee`.
    pub mod nombre_ou_texte {
        use crate::grandeurs::Grandeur;

        pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<G, D::Error> {
            crate::forme::decimal_ou_tape(d, super::tapee_en_decimal::<G>).and_then(|x| crate::grandeurs::lue_du(x).map_err(serde::de::Error::custom))
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
            crate::forme::court(d).and_then(|x| crate::grandeurs::lue_du(x).map_err(serde::de::Error::custom))
        }

        /// Une grandeur lue ainsi, la ou un champ ne se pose pas (une valeur datee, un element d'une liste ou d'un
        /// tableau ecrit `[bas, haut, taux]`) ; `Option<Lue<G>>` : absente ou nulle, elle est vide.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Lue<G>(pub G);

        impl<'de, G: Grandeur> serde::Deserialize<'de> for Lue<G> {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Lue<G>, D::Error> {
                deserialize(d).map(Lue)
            }
        }

        /// Un `Option` lu ainsi (`Lue`) : absent (avec `#[serde(default)]`) ou nul, il est vide.
        pub mod option {
            use super::Lue;
            use crate::grandeurs::Grandeur;

            pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<Option<G>, D::Error> {
                <Option<Lue<G>> as serde::Deserialize>::deserialize(d).map(|c| c.map(|Lue(g)| g))
            }
        }

        /// Une liste facultative lue ainsi (`Lue`) : absente (avec `#[serde(default)]`) ou nulle, elle est vide.
        pub mod liste_option {
            use super::Lue;
            use crate::grandeurs::Grandeur;

            pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Vec<G>>, D::Error> {
                <Option<Vec<Lue<G>>> as serde::Deserialize>::deserialize(d).map(|l| l.map(|l| l.into_iter().map(|Lue(g)| g).collect()))
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
            crate::grandeurs::decimal(g).to_string()
        }

        /// Relue exactement de son texte, son echelle gardee (un nombre JSON ne se lit pas ainsi).
        pub fn deserialize<'de, G: Grandeur, D: serde::Deserializer<'de>>(d: D) -> Result<G, D::Error> {
            use serde::Deserialize;
            let t = String::deserialize(d)?;
            match t.parse::<crate::num::Decimal>() {
                Ok(x) => crate::grandeurs::lue(x, &crate::ecriture::json_en_ligne(&t)).map_err(serde::de::Error::custom),
                Err(_) => Err(serde::de::Error::custom(format!("{} n'est pas un nombre", crate::ecriture::json_en_ligne(&t)))),
            }
        }

        ecriture!();
    }

    /// En texte, a deux decimales (`12.50`) ; un decimal plus fin que le centime ne s'ecrit pas ainsi.
    pub mod centimes {
        use crate::grandeurs::Grandeur;

        pub fn texte<G: Grandeur>(g: G) -> Result<String, String> {
            let mut c = crate::grandeurs::decimal(g);
            if c.scale() > 2 {
                return Err(format!("{} est plus fin que le centime", c));
            }
            c.rescale(2);
            Ok(c.to_string())
        }

        ecriture!();
    }

    /// En texte, dans sa forme la plus courte, une decimale au moins (`24` et `24.00` s'ecrivent `24.0`, `0.10` s'ecrit
    /// `0.1`), comme `forme::court` la lit : un taux d'un effet du bareme rendu tel que le bareme le lit.
    pub mod court {
        use crate::grandeurs::Grandeur;

        /// La grandeur telle qu'une phrase la cite (`7.0`).
        pub fn dit<G: Grandeur>(g: G) -> String {
            crate::num::court(crate::grandeurs::decimal(g)).to_string()
        }
    }

    /// En texte, sans zero inutile (`12.5`, `12`).
    pub mod sans_zero {
        use crate::grandeurs::Grandeur;

        pub fn texte<G: Grandeur>(g: G) -> Result<String, String> {
            Ok(dit(g))
        }

        /// La meme, telle qu'une phrase ou une case la cite (`2`, `151.67`).
        pub fn dit<G: Grandeur>(g: G) -> String {
            crate::grandeurs::decimal(g).normalize().to_string()
        }

        ecriture!();
    }

    /// En texte, dans sa forme la plus courte (`12.5`, `12` ; « -0 » s'ecrit « 0 » : `num::nombre`), relue d'un nombre
    /// ou d'un texte (`forme::decimal`) : l'ecriture des nombres d'une paie ecrite.
    pub mod nombre {
        use crate::grandeurs::Grandeur;

        pub fn serialize<G: Grandeur, S: serde::Serializer>(g: &G, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_str(&texte(*g))
        }

        /// Le meme texte, la ou une forme l'ecrit sans son attribut (une valeur d'un dictionnaire).
        pub fn texte<G: Grandeur>(g: G) -> String {
            crate::num::nombre(crate::grandeurs::decimal(g))
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
            let d = crate::grandeurs::decimal(*g).normalize();
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

    /// En nombre JSON a virgule, exact, comme les fichiers l'ecrivent (`130.0`, `1.1` : `forme::ecrire_a_virgule`).
    pub mod a_virgule {
        use crate::grandeurs::Grandeur;

        pub fn serialize<G: Grandeur, S: serde::Serializer>(g: &G, s: S) -> Result<S::Ok, S::Error> {
            crate::forme::ecrire_a_virgule(&crate::grandeurs::decimal(*g), s)
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
    /// montant au centime, un taux a ses decimales. La seule ecriture des grandeurs de la DSN, et des retours de la norme
    /// que le simulateur des organismes fabrique (le taux de PAS, le taux AT/MP notifie).
    pub mod norme {
        use crate::grandeurs::{Grandeur, Montant, Taux};
        use crate::num::Decimal;
        use rust_decimal::RoundingStrategy;

        /// Un montant au centime (`Montant::au_centime`), ses deux decimales ecrites (`12.30`) ; un zero s'ecrit sans
        /// signe (`0.00`, jamais `-0.00`).
        pub fn montant(m: Montant) -> String {
            let c = m.au_centime();
            if c.est_nul() { Montant::ZERO } else { c }.0.to_string()
        }

        /// Un taux a `decimales` decimales, le demi au pair (`Taux::au_pair`), toutes ecrites (`1.50`, `6.900`).
        pub fn taux(t: Taux, decimales: u32) -> String {
            let mut d = t.au_pair(decimales).0;
            d.rescale(decimales);
            d.to_string()
        }

        /// Une quantite (des heures, des jours, des cachets) au centieme, la moitie vers le haut, ses deux decimales
        /// ecrites (`151.67`, `2.00`) ; un zero s'ecrit sans signe (`0.00`, jamais `-0.00`). Son signe se pose avant
        /// (`Signe::de`).
        pub fn quantite<G: Grandeur>(q: G) -> String {
            let mut c = crate::grandeurs::decimal(q).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
            if c.is_zero() {
                c = Decimal::ZERO;
            }
            c.rescale(2);
            c.to_string()
        }
    }

    /// UN MONTANT EN CENTIMES, sans separateur, sur `chiffres` chiffres completes de zeros a gauche (`000000001234` pour
    /// 12,34 €, la zone montant de Quadra), au centime (`Montant::au_centime`).
    pub mod en_centimes {
        use crate::grandeurs::Montant;

        pub fn chiffres(m: Montant, chiffres: usize) -> String {
            format!("{:0w$}", m.au_centime().0.mantissa(), w = chiffres)
        }
    }

    /// En nombre entier JSON (`2`) ; un decimal qui n'est pas un entier ne s'ecrit pas ainsi. Un entier ecrit avec des
    /// decimales nulles en est un : la DSN ecrit ses mesures avec deux decimales (`2.00`, cahier technique : « avec
    /// decimale ») et `2.00` s'ecrit `2`.
    pub mod entier {
        use crate::grandeurs::Grandeur;

        pub fn serialize<G: Grandeur, S: serde::Serializer>(g: &G, s: S) -> Result<S::Ok, S::Error> {
            let d = crate::grandeurs::decimal(*g).normalize();
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
        let c = |t: &str| Montant(Decimal::from_str(t).unwrap()).au_centime().0.to_string();
        assert_eq!(c("2.675"), "2.68");
        assert_eq!(c("2.125"), "2.13");
        assert_eq!(c("258.336"), "258.34");
        assert_eq!(c("-0.005"), "-0.01");
        assert_eq!(c("12.5"), "12.50");
    }
}
