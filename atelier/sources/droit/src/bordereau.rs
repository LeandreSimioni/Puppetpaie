//! CE QU'UNE LIGNE DU BORDEREAU URSSAF LIT (`organismes-base.json`, `ctp_urssaf.lignes[]`) : une assiette agregee du
//! bordereau (`assiette`) ou un montant (`montant`). Deux listes fermees, declarees une fois : chaque valeur et son nom, le
//! seul endroit ou il s'ecrit ; une ligne qui en nomme une autre se refuse a la lecture du fichier. Ce que chacune somme,
//! la DSN le dit (`declarations::dsn::assiettes_ctp`).

use crate::assiette::Assiette;
use socle::fichiers::repr;

macro_rules! noms_fermes {
    ($(#[$a:meta])* $nom:ident, $quoi:literal { $( $(#[$va:meta])* $v:ident = $n:literal, )* }) => {
        $(#[$a])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $nom {
            $( $(#[$va])* $v, )*
        }

        impl $nom {
            pub const TOUTES: &'static [$nom] = &[ $( $nom::$v, )* ];

            /// Son nom, le seul endroit ou il s'ecrit.
            pub fn nom(self) -> &'static str {
                match self { $( $nom::$v => $n, )* }
            }
        }

        impl std::fmt::Display for $nom {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str(self.nom())
            }
        }

        impl<'de> serde::Deserialize<'de> for $nom {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let nom = <String as serde::Deserialize>::deserialize(d)?;
                $nom::TOUTES.iter().copied().find(|a| a.nom() == nom).ok_or_else(|| {
                    let connues: Vec<&str> = $nom::TOUTES.iter().map(|a| a.nom()).collect();
                    serde::de::Error::custom(format!("{} inconnu : {}. Connus : {}.", $quoi, repr(&nom), connues.join(", ")))
                })
            }
        }
    };
}

noms_fermes! {
    /// UNE ASSIETTE AGREGEE DU BORDEREAU URSSAF (S21.G00.23.004) : la somme, sur les paies du mois, d'une assiette de
    /// chacune, pour une population (toutes, les permanents, les intermittents, les artistes, les techniciens).
    AssietteDuBordereau, "Assiette du bordereau" {
        Brut = "brut",
        Csg = "csg",
        BrutX115 = "brut_x115",
        BrutX110 = "brut_x110",
        BrutPermanent = "brut_permanent",
        CsgPermanent = "csg_permanent",
        T1Permanent = "t1_permanent",
        T1X115Permanent = "t1_x115_permanent",
        ChomagePermanent = "chomage_permanent",
        CpfCdd = "cpf_cdd",
        CfpPermanent = "cfp_permanent",
        BrutIntermittent = "brut_intermittent",
        CsgIntermittent = "csg_intermittent",
        TaPrincipale = "ta_principale",
        BrutTechnicien = "brut_technicien",
        BrutX115Technicien = "brut_x115_technicien",
        BrutArtiste = "brut_artiste",
        BrutX115Artiste = "brut_x115_artiste",
    }
}

impl AssietteDuBordereau {
    /// L'assiette du calcul que celle-ci somme sur toutes les paies du mois, quand elle n'est que cela ; `None` : elle
    /// somme pour une population, ou autre chose qu'une assiette.
    pub fn de_toutes_les_paies(self) -> Option<Assiette> {
        match self {
            AssietteDuBordereau::Brut => Some(Assiette::Brut),
            AssietteDuBordereau::Csg => Some(Assiette::Csg),
            AssietteDuBordereau::BrutX115 => Some(Assiette::BrutX115),
            AssietteDuBordereau::BrutX110 => Some(Assiette::BrutX110),
            _ => None,
        }
    }
}

noms_fermes! {
    /// UN MONTANT DU BORDEREAU URSSAF (S21.G00.23.005) : une reduction ou une deduction, que le nominatif declare.
    MontantDuBordereau, "Montant du bordereau" {
        ReductionGeneralePermanent = "reduction_generale_permanent",
        ReductionGeneraleUrssaf = "reduction_generale_urssaf",
        ReductionHeuresEnPlus = "reduction_heures_en_plus",
        DeductionHeuresSupplementaires = "deduction_heures_supplementaires",
    }
}
