//! CE QU'UNE LIGNE DU BORDEREAU URSSAF LIT (`organismes-base.json`, `ctp_urssaf.lignes[]`) : une assiette agregee du
//! bordereau (`assiette`) ou un montant (`montant`). Deux listes fermees, declarees une fois : chaque valeur et son nom, le
//! seul endroit ou il s'ecrit ; une ligne qui en nomme une autre se refuse a la lecture du fichier. Ce que chacune somme,
//! la DSN le dit (`declarations::dsn::assiettes_ctp`).

use crate::assiette::Assiette;
use socle::noms_fermes;

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
        /// Les artistes intermittents d'un etablissement hors du Bas-Rhin, du Haut-Rhin et de la Moselle (CTP 312).
        BrutArtisteHorsAlsaceMoselle = "brut_artiste_hors_alsace_moselle",
        /// Les artistes intermittents d'un etablissement du Bas-Rhin, du Haut-Rhin ou de la Moselle (CTP 313, qui porte
        /// leur regime local).
        BrutArtisteAlsaceMoselle = "brut_artiste_alsace_moselle",
        /// Les salaries d'un etablissement du Bas-Rhin, du Haut-Rhin ou de la Moselle dont le regime local ne passe pas
        /// par le CTP 313 : tous, sauf les artistes intermittents (CTP 381).
        BrutRegimeLocal = "brut_regime_local",
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

    /// Ne somme-t-elle que les salaries d'un etablissement d'Alsace-Moselle ? Sa ligne ne s'ecrit alors que si elle
    /// n'est pas nulle, comme celle d'une population (`statut`, `nature`) : un CTP de la zone n'a rien a faire au
    /// bordereau d'une compagnie qui n'y emploie personne.
    pub fn de_l_alsace_moselle(self) -> bool {
        matches!(self, AssietteDuBordereau::BrutArtisteAlsaceMoselle | AssietteDuBordereau::BrutRegimeLocal)
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
