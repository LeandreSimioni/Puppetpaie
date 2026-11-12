//! Le type d'une prime, d'une gratification ou d'une indemnite (S21.G00.52.001) : la liste entiere de la norme.

use crate::natures::codes_fermes;

codes_fermes! {
    /// LE TYPE D'UNE PRIME OU D'UNE INDEMNITE DU BLOC 52 (S21.G00.52.001), la liste entiere de la norme (cahier technique
    /// DSN 2026.1, `atelier/reference/norme-dsn/texte/bloc-S21.G00.52.txt` ; relevee dans `formats-dsn-*-base.json`).
    /// Celui qu'une rubrique declare, `rubriques-base.json` le dit (`dsn_bloc52`) ; ceux que les controles CCH-21 a CCH-29
    /// lient a des motifs de rupture, `permanents-base.json` (`indemnites_de_rupture.codes`) ; ceux d'une indemnite
    /// compensatrice de preavis, `fctu-base.json` (`types_preavis.codes_52_de_l_indemnite_de_preavis`).
    TypeDePrime, "Type de prime", "S21.G00.52.001" {
        RuptureConventionnelle = "001",
        CessationForceeDesMandataires = "002",
        MiseALaRetraiteLegale = "003",
        MiseALaRetraiteConventionnelle = "004",
        DepartALaRetraiteLegal = "005",
        DepartALaRetraiteConventionnel = "006",
        LicenciementLegal = "007",
        LicenciementLegalSupplementaire = "008",
        LicenciementLegalSpeciale = "009",
        LicenciementLegalSpecifique = "010",
        FinDeCdd = "011",
        FinDeMission = "012",
        Journalistes = "013",
        Clientele = "014",
        PersonnelNaviguant = "015",
        Apprenti = "016",
        DommagesEtInterets = "017",
        Sinistre = "018",
        NonConcurrence = "019",
        CompensatriceDeCongesPayes = "020",
        Conventionnelle = "021",
        Transactionnelle = "022",
        CompensatriceDePreavis = "023",
        CompteEpargneTemps = "025",
        PrimeExceptionnelleRattachee = "026",
        PrimeLieeALActivite = "027",
        PrimeNonLieeALActivite = "028",
        PrimeReposCompensateur = "029",
        PrimeRachatCet = "030",
        PreavisPourInaptitude = "032",
        ConciliationPrudhomale = "033",
        CongesPayesD3141 = "034",
        ComplementALaChargeDeLEtat = "039",
        FpTechnicite = "040",
        FpSujetionsSpeciales = "041",
        FpRisque = "042",
        FpPrimeDeSujetionsSpeciales = "043",
        FpSujetionSpecifique = "044",
        NonRemiseDuContratDeMission = "045",
        CongesPayesL3141 = "046",
        FpGipa = "047",
        Residence = "048",
        FpSupplementFamilial = "049",
        FpIfse = "050",
        FpCia = "051",
        SupplementClasse4 = "052",
        SupplementClasses1A8 = "053",
        SupplementAgentsDeConduite = "054",
        TransactionnelleExoneree = "055",
        Expatriation = "900",
        Impatriation = "901",
        NouveauTypePepa = "902",
        AutrePrimeExoneree = "903",
        PartageDeLaValeurNonImposable = "904",
        PartageDeLaValeurImposable = "905",
        PartageDeLaValeurCsg = "906",
        NouveauTypeB = "907",
        NouveauTypeC = "908",
        NouveauTypeD = "909",
        NouveauTypeE = "910",
        NouveauTypeA = "911",
    }
}

impl crate::fiche::FormeDeCase for TypeDePrime {}
