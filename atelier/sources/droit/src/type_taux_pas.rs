//! Le type d'un taux de prelevement a la source (S21.G00.50.007) : la liste entiere de la norme.

use crate::natures::codes_fermes;

codes_fermes! {
    /// LE TYPE D'UN TAUX DE PAS (S21.G00.50.007), la liste entiere de la norme (cahier technique DSN 2026.1,
    /// `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt` ; relevee dans `formats-dsn-*-base.json`). 01 est le taux
    /// que la DGFiP transmet ; les autres, la grille de l'article 204 H du CGI, ou l'indu d'un exercice anterieur.
    TypeDeTauxPas, "Type de taux de PAS", "S21.G00.50.007" {
        TransmisParLaDgfip = "01",
        BaremeMensuelMetropole = "13",
        BaremeMathematiqueMetropole = "17",
        BaremeMensuelGuadeloupeReunionMartinique = "23",
        BaremeMathematiqueGuadeloupeReunionMartinique = "27",
        BaremeMensuelGuyaneMayotte = "33",
        BaremeMathematiqueGuyaneMayotte = "37",
        InduDUnExerciceAnterieur = "99",
    }
}

impl TypeDeTauxPas {
    /// Le type en clair, tel que l'etat du prelevement l'ecrit.
    pub fn libelle(self) -> &'static str {
        match self {
            TypeDeTauxPas::TransmisParLaDgfip => "Taux transmis par la DGFIP",
            TypeDeTauxPas::BaremeMensuelMetropole => "Bareme mensuel metropole",
            TypeDeTauxPas::BaremeMathematiqueMetropole => "Bareme mathematique sur base mensuelle metropole",
            TypeDeTauxPas::BaremeMensuelGuadeloupeReunionMartinique => "Bareme mensuel Guadeloupe, Reunion et Martinique",
            TypeDeTauxPas::BaremeMathematiqueGuadeloupeReunionMartinique => "Bareme mathematique sur base mensuelle Guadeloupe, Reunion, Martinique",
            TypeDeTauxPas::BaremeMensuelGuyaneMayotte => "Bareme mensuel Guyane et Mayotte",
            TypeDeTauxPas::BaremeMathematiqueGuyaneMayotte => "Bareme mathematique sur base mensuelle Guyane et Mayotte",
            TypeDeTauxPas::InduDUnExerciceAnterieur => "Indu relatif a un exercice anterieur - pas de taux de PAS",
        }
    }

    /// Le taux que la DGFiP a transmis (01) : lui seul a un identifiant et se perime.
    pub fn transmis(self) -> bool {
        self == TypeDeTauxPas::TransmisParLaDgfip
    }
}

impl crate::fiche::FormeDeCase for TypeDeTauxPas {}
