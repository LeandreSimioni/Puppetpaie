//! Le type d'un autre element de revenu brut (S21.G00.54.001) : la liste entiere de la norme.

use crate::natures::codes_fermes;

codes_fermes! {
    /// LE TYPE D'UN AUTRE ELEMENT DE REVENU BRUT (S21.G00.54.001), la liste entiere de la norme (cahier technique DSN,
    /// `atelier/reference/norme-dsn/` ; relevee dans `formats-dsn-*-base.json`). Une rubrique du catalogue le dit
    /// (`dsn_autres_bruts`) ; la DSN l'ecrit en bloc 54. 52 et 53 (la PPVE) n'existent qu'a partir de P27V01.
    AutreElementDeRevenuBrut, "Type d'autre élément de revenu brut", "S21.G00.54.001" {
        SommeVerseeParUnTiers = "01",
        AvantageRepas = "02",
        AvantageLogement = "03",
        AvantageVehicule = "04",
        AvantageNtic = "05",
        AvantageAutres = "06",
        FraisRemboursesAuForfait = "07",
        FraisPrisEnChargeParLEmployeur = "08",
        FraisRemboursesAuReel = "09",
        DeductionForfaitaireSpecifique = "10",
        Participation = "11",
        Interessement = "12",
        AbondementPee = "14",
        AbondementPei = "15",
        AbondementRetraiteCollectif = "16",
        TitresRestaurant = "17",
        TransportsPublics = "18",
        TransportsPersonnels = "19",
        DroitDAuteur = "25",
        DroitDeDoublage = "26",
        DroitDeRediffusion = "27",
        AvantagesDePreretraite = "31",
        CetVersUneEpargneRetraite = "33",
        PrestationsFamiliales = "34",
        FraisReintegresDfs = "35",
        CetEnPointsRafp = "36",
        ParticipationInteressementImmediats = "37",
        NouveauTypeA = "50",
        NouveauTypeB = "51",
        PpveVerseeImmediatement = "52" depuis "P27V01",
        PpveAffecteeAUnPlan = "53" depuis "P27V01",
        ServicesALaPersonne = "90",
        ChequesVacances = "91",
        CotisationPatronaleSante = "92",
        CotisationPrevoyanceRetraiteSupplementaire = "93",
        NouveauTypeC = "94",
        NouveauTypeD = "95",
        NouveauTypeE = "96",
    }
}

impl AutreElementDeRevenuBrut {
    /// Un des cinq avantages en nature (02 a 06), dont une ligne de paie peut preciser le genre.
    pub fn avantage_en_nature(self) -> bool {
        matches!(
            self,
            AutreElementDeRevenuBrut::AvantageRepas
                | AutreElementDeRevenuBrut::AvantageLogement
                | AutreElementDeRevenuBrut::AvantageVehicule
                | AutreElementDeRevenuBrut::AvantageNtic
                | AutreElementDeRevenuBrut::AvantageAutres
        )
    }
}

impl crate::fiche::FormeDeCase for AutreElementDeRevenuBrut {}
