//! LES NOMENCLATURES DU BLOC « Arret de travail - S21.G00.60 » (`logiciel/donnees/arrets-base.json`) : motif de l'arret,
//! motif de la reprise, subrogation. Leur forme est declaree ici et `bloc_60` est la seule fonction qui lit le fichier ;
//! l'origine d'un arret et son maintien (`maintien::Origine::du_motif`), le bulletin (`engagement::imprevus::lignes_du_mois`),
//! le signalement et la DSN (`declarations`), les obligations et la page des imprevus (le moteur) la lisent par elle.

use crate::maintien::Origine;
use crate::natures::codes_fermes;
use socle::echec::R;
use socle::fichiers::{forme_en_cache, FormeGardee};

codes_fermes! {
    /// LE MOTIF DE L'ARRET (S21.G00.60.001), la liste entiere de la norme (cahier technique DSN 2026.1,
    /// `atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt` ; relevee dans `formats-dsn-*-base.json`). 25 n'est que
    /// dans la liste 2027 (`formats-dsn-p27v01-base.json`). Ce que chacun emporte, `arrets-base.json` le dit
    /// (`motifs_arret`).
    MotifDArret, "Motif de l'arrêt", "S21.G00.60.001" {
        Maladie = "01",
        Maternite = "02",
        Paternite = "03",
        AccidentDeTrajet = "04",
        MaladieProfessionnelle = "05",
        AccidentDuTravail = "06",
        FemmeEnceinteDispensee = "07",
        Adoption = "09",
        FpMaladieImputableAuService = "10",
        FpVictimesDeGuerre = "11",
        FpLongueDuree = "12",
        FpLongueMaladie = "13",
        FpInvaliditeTemporaire = "14",
        TempsPartielTherapeutiqueMaladie = "15",
        TempsPartielTherapeutiqueAccidentDuTravail = "16",
        TempsPartielTherapeutiqueTrajet = "17",
        TempsPartielTherapeutiqueMaladieProfessionnelle = "18",
        DeuilDEnfant = "19",
        CongeSupplementaireDeNaissance = "20",
        NouveauMotifB = "21",
        NouveauMotifC = "22",
        NouveauMotifD = "23",
        NouveauMotifE = "24",
        NouveauMotifA = "25" depuis P27V01,
        Annulation = "99",
    }
}

codes_fermes! {
    /// LE MOTIF DE LA REPRISE (S21.G00.60.011), la liste entiere de la norme (cahier technique DSN 2026.1,
    /// `atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt` ; relevee dans `formats-dsn-*-base.json`).
    MotifDeReprise, "Motif de la reprise", "S21.G00.60.011" {
        Normale = "01",
        TempsPartielTherapeutique = "02",
        TempsPartielRaisonPersonnelle = "03",
    }
}

codes_fermes! {
    /// LA SUBROGATION D'UN ARRET (S21.G00.60.004), la liste entiere de la norme (cahier technique DSN 2026.1,
    /// `atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt` ; relevee dans `formats-dsn-*-base.json`). Ce que la
    /// compagnie dit d'un arret ; la case « subrogation » de la fiche d'un permanent est une autre chose
    /// (`fiche::Subrogation`).
    SubrogationDeLArret, "Subrogation", "S21.G00.60.004" {
        Oui = "01",
        Non = "02",
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bloc60 {
    pub quoi: String,
    pub note: String,
    pub motifs_arret: Nomenclature<MotifDArret>,
    pub motifs_reprise: Nomenclature<MotifDeReprise>,
    pub subrogation: Nomenclature<SubrogationDeLArret>,
}

/// Une nomenclature : ses codes dans l'ordre du fichier, et ses notes (rubrique, drapeaux).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nomenclature<T> {
    pub codes: Vec<CodeDuBloc60<T>>,
    pub quoi: Option<String>,
    pub note: Option<String>,
}

/// Un code de la norme, tel que la page le propose : son libelle, la ligne du bulletin qui nomme l'absence
/// (`rubrique`), s'il se saisit, et ce qu'il emporte (la date de l'accident, l'origine qui ouvre le maintien de la loi,
/// un temps partiel therapeutique, une valeur de reserve, l'annulation).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodeDuBloc60<T> {
    pub code: T,
    pub libelle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rubrique: Option<crate::catalogue::CodeDeRubrique>,
    pub saisissable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_accident: Option<bool>,
    /// Ce qui a cause l'arret, quand c'est une maladie ou un accident (L1226-1) : le seul motif qui appelle le maintien.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origine: Option<Origine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temps_partiel_therapeutique: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserve: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annulation: Option<bool>,
    #[serde(skip_serializing)]
    pub note: Option<String>,
}

impl<T: Copy + PartialEq> Nomenclature<T> {
    /// Le code `c` de la nomenclature, s'il y est.
    pub fn code(&self, c: T) -> Option<&CodeDuBloc60<T>> {
        self.codes.iter().find(|x| x.code == c)
    }
}

impl<T: Clone> Nomenclature<T> {
    /// Ceux que la page propose (`saisissable`).
    pub fn saisissables(&self) -> Vec<CodeDuBloc60<T>> {
        self.codes.iter().filter(|c| c.saisissable).cloned().collect()
    }
}

/// `arrets-base.json`, lu par sa forme (le texte est lu une fois tant qu'il ne change pas).
pub fn bloc_60() -> R<Bloc60> {
    static CACHE: std::sync::OnceLock<FormeGardee<Bloc60>> = std::sync::OnceLock::new();
    let b = forme_en_cache(&CACHE, "arrets-base.json", "Les codes du bloc arrêt de travail livrés avec le logiciel ne se lisent pas : le signaler au développeur.", Ok)?;
    Ok((*b).clone())
}

/// Le motif d'arret (`arrets-base.json`, `motifs_arret`), s'il existe.
pub fn motif_d_arret(code: MotifDArret) -> R<Option<CodeDuBloc60<MotifDArret>>> {
    Ok(bloc_60()?.motifs_arret.codes.into_iter().find(|c| c.code == code))
}
