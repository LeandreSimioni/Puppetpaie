//! Le type d'une declaration DSN (S20.G00.05.002) : la liste entiere de la norme.

use crate::natures::codes_fermes;

codes_fermes! {
    /// LE TYPE D'UNE DECLARATION (S20.G00.05.002), la liste entiere de la norme (cahier technique DSN 2026.1,
    /// `atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt` l. 85-89 ; 2027, `texte-2027/bloc-S20.G00.05.txt`
    /// l. 87-91, la meme liste). Le fichier l'ecrit et le journal des envois le garde par son code.
    TypeDeLaDeclaration, "Type de la déclaration", "S20.G00.05.002" {
        Normale = "01",
        NormaleSansIndividu = "02",
        AnnuleEtRemplace = "03",
        Annule = "04",
        AnnuleEtRemplaceSansIndividu = "05",
    }
}

impl TypeDeLaDeclaration {
    /// Le type en clair, tel que la personne le lit.
    pub fn libelle(self) -> &'static str {
        match self {
            TypeDeLaDeclaration::Normale => "déclaration normale",
            TypeDeLaDeclaration::NormaleSansIndividu => "déclaration normale sans individu",
            TypeDeLaDeclaration::AnnuleEtRemplace => "annule et remplace intégral",
            TypeDeLaDeclaration::Annule => "déclaration annule",
            TypeDeLaDeclaration::AnnuleEtRemplaceSansIndividu => "annule et remplace sans individu",
        }
    }

    /// UNE DECLARATION QUI ANNULE ET REMPLACE (03, 05) : elle remplace la precedente du meme mois.
    pub fn remplace(self) -> bool {
        matches!(self, TypeDeLaDeclaration::AnnuleEtRemplace | TypeDeLaDeclaration::AnnuleEtRemplaceSansIndividu)
    }
}
