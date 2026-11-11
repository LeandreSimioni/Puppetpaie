//! LES ASSIETTES DU CALCUL : ce sur quoi une cotisation se calcule (`base` d'une rubrique ou d'une cotisation du bareme,
//! `base78` d'une cotisation de la DSN), et les montants qu'une paie garde a cote d'elles pour la DSN (`Paie::bases`).
//! Une liste fermee, declaree une fois : chaque assiette et son nom, le seul endroit ou il s'ecrit, celui des baremes
//! livres et des paies ecrites. Un bareme qui nomme une assiette que la liste n'a pas se refuse a sa lecture.

use serde::{Deserialize, Serialize};
use socle::fichiers::repr;

macro_rules! assiettes {
    ($( $(#[$va:meta])* $v:ident = $nom:literal, )*) => {
        /// UNE ASSIETTE DU CALCUL (voir le module).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Assiette {
            $( $(#[$va])* $v, )*
        }

        impl Assiette {
            pub const TOUTES: &'static [Assiette] = &[ $( Assiette::$v, )* ];

            /// Son nom, le seul endroit ou il s'ecrit.
            pub fn nom(self) -> &'static str {
                match self { $( Assiette::$v => $nom, )* }
            }
        }
    };
}

assiettes! {
    /// Le brut soumis.
    Brut = "brut",
    /// Le brut des rubriques que le catalogue met dans l'assiette des Conges Spectacles, d'un contrat declare a la caisse.
    BrutCs = "brut_cs",
    /// L'assiette de la CSG et de la CRDS.
    Csg = "csg",
    /// Le brut majore pour les caisses de conges (CSS L. 243-1-3, D. 243-0-2).
    BrutX115 = "brut_x115",
    BrutX110 = "brut_x110",
    /// Le brut, et ses formes majorees, apres la deduction forfaitaire specifique.
    BrutDfs = "brut_dfs",
    BrutX115Dfs = "brut_x115_dfs",
    BrutX110Dfs = "brut_x110_dfs",
    /// Les tranches d'un intermittent, sur son plafond journalier.
    T1Journaliere = "t1_journaliere",
    T2Journaliere = "t2_journaliere",
    T2PrevoyanceJournaliere = "t2_prevoyance_journaliere",
    Apec = "apec",
    ApecT2Journaliere = "apec_t2_journaliere",
    Cet = "cet",
    /// Les tranches d'un salarie paye au mois, sur le plafond de sa periode.
    T1Mensuelle = "t1_mensuelle",
    T1MensuelleX115 = "t1_mensuelle_x115",
    T2Mensuelle = "t2_mensuelle",
    ApecMensuelle = "apec_mensuelle",
    CetMensuelle = "cet_mensuelle",
    T1T2Mensuelle = "t1_t2_mensuelle",
    T2Conventionnelle = "t2_conventionnelle",
    /// Le plafond de la periode sans la correction du temps partiel : l'assiette du forfait de sante.
    PssPeriode = "pss_periode",
    /// Le brut dans la limite de quatre plafonds : le chomage et l'AGS.
    BrutChomage = "brut_chomage",
    /// Le brut d'un intermittent dans la limite de son plafond, et sa forme majoree ; et apres la deduction forfaitaire.
    BrutPlafonne = "brut_plafonne",
    BrutX115Plafonne = "brut_x115_plafonne",
    BrutPlafonneDfs = "brut_plafonne_dfs",
    BrutX115PlafonneDfs = "brut_x115_plafonne_dfs",
    /// Une base de la DSN qui ne porte aucune assiette : elle vaut zero. Le calcul ne l'ecrit jamais dans une paie.
    Zero = "zero",
    /// Les deux termes du rapport du SMIC d'un mois d'absence (S21.G00.51, types 028 et 029).
    RemunerationPercueHorsEna = "remuneration_percue_hors_ena",
    RemunerationHabituelleHorsEna = "remuneration_habituelle_hors_ena",
    /// Les heures en plus d'un salarie paye au mois : leur remuneration, leur nombre, leur montant net exonere d'impot
    /// (S21.G00.58.003 = 01).
    HeuresEnPlus = "heures_en_plus",
    HeuresEnPlusHeures = "heures_en_plus_heures",
    HeuresEnPlusExonere = "heures_en_plus_exonere",
    /// Les indemnites journalieres que la compagnie subrogee reverse, et leur assiette du prelevement a la source.
    IjSubrogeesNettes = "ij_subrogees_nettes",
    IjSubrogeesAssiettePas = "ij_subrogees_assiette_pas",
}

impl Assiette {
    /// UNE ASSIETTE PAR RUBRIQUE DE SALAIRE se calcule sur chaque rubrique soumise (le calcul, §7.4), et une cotisation
    /// s'y somme rubrique par rubrique, chacune arrondie ; toute autre est globale : une seule valeur pour la paie.
    pub fn par_rubrique(self) -> bool {
        matches!(
            self,
            Assiette::Brut
                | Assiette::BrutCs
                | Assiette::BrutX115
                | Assiette::BrutX110
                | Assiette::BrutDfs
                | Assiette::BrutX115Dfs
                | Assiette::BrutX110Dfs
        )
    }

    /// SA FORME APRES LA DEDUCTION FORFAITAIRE SPECIFIQUE, quand elle en a une : le brut abattu, et les assiettes
    /// majorees et plafonnees qui partent de lui (lettre circulaire Acoss 2013-019, annexe 1, R6 ;
    /// `atelier/reference/sources/majoration-caisses-conges/`, `atelier/reference/sources/dfs/`).
    pub fn abattue(self) -> Option<Assiette> {
        match self {
            Assiette::Brut => Some(Assiette::BrutDfs),
            Assiette::BrutX115 => Some(Assiette::BrutX115Dfs),
            Assiette::BrutX110 => Some(Assiette::BrutX110Dfs),
            Assiette::BrutPlafonne => Some(Assiette::BrutPlafonneDfs),
            Assiette::BrutX115Plafonne => Some(Assiette::BrutX115PlafonneDfs),
            _ => None,
        }
    }
}

/// Dans l'ordre de leurs noms : une table indexee par elle s'ecrit comme quand son nom etait un texte.
impl Ord for Assiette {
    fn cmp(&self, autre: &Self) -> std::cmp::Ordering {
        self.nom().cmp(autre.nom())
    }
}

impl PartialOrd for Assiette {
    fn partial_cmp(&self, autre: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(autre))
    }
}

impl std::fmt::Display for Assiette {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.nom())
    }
}

impl Serialize for Assiette {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nom())
    }
}

impl<'de> Deserialize<'de> for Assiette {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let nom = String::deserialize(d)?;
        Assiette::TOUTES.iter().copied().find(|a| a.nom() == nom).ok_or_else(|| {
            let mut connues: Vec<&str> = Assiette::TOUTES.iter().map(|a| a.nom()).collect();
            connues.sort();
            serde::de::Error::custom(format!("Assiette inconnue : {}. Connues : {}.", repr(&nom), connues.join(", ")))
        })
    }
}
