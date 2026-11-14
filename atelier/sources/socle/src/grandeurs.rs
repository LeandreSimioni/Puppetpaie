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
use rust_decimal::RoundingStrategy;

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
}

/// LA FORME CANONIQUE D'UNE GRANDEUR, pour les empreintes et les oracles : sans zero inutile (`12.50` devient `12.5`,
/// `-0` devient `0`), comme l'ecrit `num::nombre`.
pub fn forme_courte<G: Grandeur>(g: G) -> G {
    G::du(g.en_decimal().normalize())
}

/// LA FORME LA PLUS COURTE D'UNE GRANDEUR, une decimale au moins (`24` s'ecrit `24.0`, `12.50` `12.5`), comme l'ecrit
/// `num::court` : celle d'un taux ou d'heures qu'une paie garde.
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

    /// Ce montant `n` fois (un centime par ligne) : montant x compte = montant.
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
}

impl std::ops::Mul<Coefficient> for Montant {
    type Output = Montant;
    fn mul(self, c: Coefficient) -> Montant {
        Montant(num::mul(self.0, c.0))
    }
}

impl Coefficient {
    /// A quatre decimales, la moitie au pair (un ecart relatif : aucun texte deposé ne dit cet arrondi).
    pub fn a_quatre_decimales(self) -> Coefficient {
        Coefficient(self.0.round_dp(4))
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
}

impl Heures {
    /// Zero heure.
    pub const ZERO: Heures = Heures(Decimal::ZERO);

    /// Au centieme d'heure, la moitie vers le haut, ses deux decimales ecrites (`7.5` s'ecrit `7.50`) : des heures
    /// ramenees au jour, au mois ou a la semaine. Aucun texte deposé ne dit cet arrondi.
    pub fn au_centieme(self) -> Heures {
        let mut c = self.0.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        c.rescale(2);
        Heures(c)
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
            Ok(g.en_decimal().normalize().to_string())
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
