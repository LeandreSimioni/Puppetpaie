//! L'ARGENT ET LES QUANTITES : un montant, un taux, des heures, des cachets, des jours, un coefficient, un montant
//! unitaire (« Le glossaire »). Chacun porte un decimal exact, son echelle gardee (`24.0` reste `24.0`) ; il n'en sort
//! que par une porte nommee (`decimal`), la ou une ecriture ou la norme l'exige. Seules les operations qui ont un sens
//! se font : montant + montant, montant x taux = montant, heures x montant unitaire = montant ; montant + heures ne
//! compile pas. L'arithmetique est celle de toute la paie (`num::add`, `mul`, `div` : `d28`) ; l'arrondi au centime se
//! fait par `Montant::au_centime`, la ou une somme se fixe (reglement (CE) n° 1103/97, art. 5, `num::eur`).
//!
//! Une facon d'ecrire n'est pas un type : chaque forme dit la sienne par l'attribut serde du champ
//! (`#[serde(serialize_with = "socle::grandeurs::ecrit::centimes::serialize")]`), dans `ecrit`.

use crate::num::{self, Decimal};

/// Ce que toute grandeur porte : un decimal, qui n'en sort que par une porte nommee. Les ecritures (`ecrit`) s'en
/// servent.
pub trait Grandeur: Copy {
    /// Le decimal nu.
    fn en_decimal(self) -> Decimal;
}

/// Une grandeur par nom : son decimal, sa porte de sortie, l'addition et la soustraction de deux grandeurs de meme nom.
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
        }

        impl Grandeur for $nom {
            fn en_decimal(self) -> Decimal {
                self.0
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
    MontantUnitaire, "UN MONTANT UNITAIRE : des euros par unite d'une ligne de paie (par heure, par cachet : `1.50`).";
}

impl Montant {
    /// Zero au centime, ses deux decimales ecrites (`0.00`).
    pub const ZERO: Montant = Montant(Decimal::from_parts(0, 0, 0, false, 2));

    /// Au centime, la moitie vers le haut, ses deux decimales ecrites (`num::eur2`).
    pub fn au_centime(self) -> Montant {
        Montant(num::eur2(self.0))
    }

    /// Le montant que le taux `t` en prend, exact (`num::pct` sans son arrondi).
    pub fn fois_taux(self, t: Taux) -> Montant {
        Montant(num::div(num::mul(self.0, t.0), Decimal::ONE_HUNDRED))
    }
}

impl std::ops::Mul<Coefficient> for Montant {
    type Output = Montant;
    fn mul(self, c: Coefficient) -> Montant {
        Montant(num::mul(self.0, c.0))
    }
}

impl Taux {
    /// A deux decimales, la moitie au pair.
    pub fn a_deux_decimales(self) -> Taux {
        Taux(self.0.round_dp(2))
    }
}

impl Heures {
    /// Zero heure.
    pub const ZERO: Heures = Heures(Decimal::ZERO);
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

au_montant_unitaire!(Heures, Cachets, Jours);

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

    /// En texte, son echelle gardee (`12.5`, `12.50`).
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

    /// En nombre entier JSON (`2`) ; un decimal qui n'est pas un entier ne s'ecrit pas ainsi.
    pub mod entier {
        pub fn serialize<G: crate::grandeurs::Grandeur, S: serde::Serializer>(g: &G, s: S) -> Result<S::Ok, S::Error> {
            let d = g.en_decimal();
            match (d.scale(), i64::try_from(d.mantissa())) {
                (0, Ok(n)) => s.serialize_i64(n),
                _ => Err(serde::ser::Error::custom(format!("{} n'est pas un entier", d))),
            }
        }
    }
}
