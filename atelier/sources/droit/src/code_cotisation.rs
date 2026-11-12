//! Le code d'une cotisation individuelle de la DSN (S21.G00.81.001) : la liste entiere de la norme, et la cle qui
//! designe une cotisation de la DSN dans une couche du bareme.

use crate::natures::codes_fermes;

codes_fermes! {
    /// UN CODE DE COTISATION (S21.G00.81.001), la liste entiere de la norme (cahier technique DSN 2026.1 p. 303-305,
    /// `atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt` l. 165-357 : « X [3,3] » puis la liste ; 2027,
    /// `texte-2027/bloc-S21.G00.81.txt` l. 127-320, qui retire 105 et ajoute 147, 148, 922 et 923). Ce qu'un bareme
    /// declare sous chacun, sa cotisation de la DSN le dit (`forme_bareme::CotisationDsn::code81`).
    CodeCotisation81, "Code de cotisation", "S21.G00.81.001" {
        ApprentiLoi1979 = "001",
        ApprentiLoi1987 = "002",
        ApprentiLoi1992 = "003",
        ContratDAccesALEmploi = "004",
        ContratDAccompagnementDansLEmploi = "006",
        ContratDeProfessionnalisation = "008",
        AssociationsIntermediaires = "009",
        BassinsDEmploiARedynamiser = "010",
        CreateurOuRepreneur = "011",
        Dom = "012",
        AideADomicile = "013",
        EntreprisesInnovantes = "014",
        ZonesFranchesUrbaines = "015",
        InteretGeneralZrr = "016",
        AideSociale = "017",
        ReductionGenerale = "018",
        RestructurationDeLaDefense = "019",
        EmbaucheZrr = "020",
        DeductionHeuresSupplementaires = "021",
        GratificationDeStage = "022",
        CetVersPlanDEpargneRetraite = "023",
        ChantierDInsertion = "025",
        Cuma = "027",
        TravailleurOccasionnel = "028",
        ReductionPetitPoolIeg = "029",
        MaladieIegComplementairePatronale = "030",
        MaladieIegComplementaireSalariale = "031",
        MaladieIegSolidarite = "032",
        ComplementDInvaliditeIeg = "033",
        DroitCommunIegAdossePatronale = "034",
        SpecialIegAdossePatronale = "035",
        SpecialIegNonAdossePatronale = "036",
        DroitCommunIegAdosseSalariale = "037",
        SpecialIegNonAdosseSalariale = "038",
        PetitPoolIeg = "039",
        AssuranceChomage = "040",
        AssuranceChomageMajoree1 = "041",
        AssuranceChomageMajoree2 = "042",
        AssuranceChomageMajoree3 = "043",
        ExonerationChomageMoinsDe26Ans = "044",
        AccidentDuTravail = "045",
        AefBourseDeLEmploi = "046",
        AefCesa = "047",
        Ags = "048",
        Fnal = "049",
        FormationAdefa = "051",
        FormationAdditionnelleFafsea = "053",
        FormationArefa = "054",
        FormationFafsea = "056",
        FormationFafseaCdd = "057",
        FormationFafseaCommunesForestieres = "058",
        PrevoyanceAssuranceMutuelle = "059",
        IrcantecTrancheA = "060",
        IrcantecTrancheB = "061",
        Crpcen = "065",
        CaisseDeCongesSpectacles = "066",
        SolidariteAutonomie = "068",
        PreretraiteDepuis2007 = "069",
        PreretraiteTauxNormal = "070",
        ForfaitSocial = "071",
        Csg = "072",
        CsgCrdsEpargneSalariale = "073",
        AllocationsFamiliales = "074",
        AssuranceMaladie = "075",
        AssuranceVieillesse = "076",
        RetenueALaSourceNonResidents = "077",
        PenaliteEmploiSenior = "078",
        Crds = "079",
        VersementMobilite = "081",
        VersementMobiliteAdditionnel = "082",
        ExonerationVersementMobilite = "088",
        ContratInitiativeEmploi = "089",
        AccueillantsFamiliaux = "090",
        SanteAuTravail = "091",
        Apecita = "092",
        IndemnitesDeMiseALaRetraite = "093",
        AllocationsFamilialesSicae = "094",
        CrpnpacRetraite = "096",
        CrpnpacAssurance = "097",
        CrpnpacMajoration = "098",
        StockOptions = "099",
        DialogueSocial = "100",
        CoutInaptitude = "101",
        ComplementAllocationsFamiliales = "102",
        ActionsGratuites = "103",
        Penibilite = "104",
        AgircArrcoYComprisApec = "105" jusqu_a P26V01,
        ReductionGeneraleRetraiteComplementaire = "106",
        ForfaitMarin = "107",
        DemiRoleMarin = "108",
        ApprentiRetraiteComplementaire = "109",
        LodeomRetraite130A220 = "110",
        AideADomicileRetraite = "111",
        LodeomRetraite170A270 = "112",
        LodeomRetraite170A350 = "113",
        ReductionHeuresSupplementaires = "114",
        RegimeLocalAlsaceMoselle = "115",
        RegularisationPrudhomale = "116",
        FormationProfessionnelle = "128",
        CpfCdd = "129",
        TaxeDApprentissage = "130",
        AgircArrco = "131",
        Apec = "132",
        MaladieMayotte = "133",
        DialogueSocialConventionnel = "140",
        FormationConventionnelle = "141",
        AgircArrcoPatronaleT1 = "142",
        CompteDePrevention = "143",
        CavecClasseC = "144",
        CavecClasseD = "145",
        AgircArrcoPatronaleT2 = "146",
        PartageDeLaValorisation = "147" depuis P27V01,
        CetPatronale = "148" depuis P27V01,
        FpNormalesSalariale = "300",
        FpNormalesPatronale = "301",
        FpSurcotisationHuitTrimestres = "302",
        FpValidationSalariale = "303",
        FpNbiSalariale = "304",
        FpNbiPatronale = "305",
        FpIndemniteDeFeuSalariale = "306",
        FpIndemniteDeFeuPatronale = "307",
        FpSapeurPompier = "308",
        FpAidesSoignantesSalariale = "309",
        FpAidesSoignantesPatronale = "310",
        FpRafpSalariale = "311",
        FpRafpPatronale = "312",
        FpIssPssSalariale = "313",
        FpIssPssPatronale = "314",
        FpIrSalariale = "315",
        FpIrPatronale = "316",
        FpImtSalariale = "317",
        FpImtPatronale = "318",
        FpInvaliditeTemporaire = "319",
        FpSurcotisation = "320",
        FpRachatDesEtudes = "321",
        FpAideADomicile = "322",
        FpRaepPatronale = "323",
        FpRaepSalariale = "324",
        FpValidationPatronale = "325",
        FpRegularisationSalariale = "326",
        FpRegularisationPatronale = "327",
        CnbfForfaitaire = "330",
        CnbfProportionnelle = "331",
        CnbfComplementaire = "332",
        EmploiPenitentiaire1 = "333",
        EmploiPenitentiaire2 = "334",
        EpargneRetraite = "901",
        FormationArtisans = "902",
        Afnca = "903",
        Anefa = "904",
        Ascpa = "905",
        Provea = "906",
        ComplementAssuranceMaladie = "907",
        TaxeForfaitaireCddu = "908",
        RetraitesSupplementaires = "909",
        CriseSanitaire = "910",
        CriseSanitaireVigne = "911",
        ForfaitSocial10 = "912",
        IndemniteInflation = "913",
        SapeursPompiersUrssaf = "914",
        SapeursPompiersAgircArrco = "915",
        Zfrr = "916",
        NouvelleCotisationD = "917",
        VersementMobiliteRegional = "918",
        ContratDeValorisationDeLExperience = "919",
        NouvelleCotisationB = "920",
        NouvelleCotisationC = "921",
        NouvelleCotisationE = "922" depuis P27V01,
        NouvelleCotisationA = "923" depuis P27V01,
    }
}

/// LA COTISATION DE LA DSN QU'UNE COUCHE DU BAREME NOMME (les cles de `cotisations_dsn` d'une categorie, d'une nature,
/// de la surcouche) : son code (81.001) et, quand plusieurs cotisations du corps partagent ce code, son affectation
/// (`CotisationDsn::affectation`), ecrits `code` ou `code:affectation` (`059:203`). Une cle vide, un code hors de la
/// liste, une affectation vide se refusent a la lecture.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct CleDeCotisationDsn {
    pub code: CodeCotisation81,
    pub affectation: Option<String>,
}

impl CleDeCotisationDsn {
    /// La cle ecrite `texte`.
    pub fn lire(texte: &str) -> socle::echec::R<CleDeCotisationDsn> {
        let (code, affectation) = match texte.split_once(':') {
            Some((c, a)) if !a.is_empty() => (c, Some(a.to_string())),
            Some(_) => return Err(socle::refus!("cle de cotisation de la DSN {} : affectation vide", socle::fichiers::repr(texte))),
            None => (texte, None),
        };
        let code = CodeCotisation81::du_code(code).ok_or_else(|| {
            let connus: Vec<&str> = CodeCotisation81::TOUS.iter().map(|c| c.code()).collect();
            socle::refus!("Code de cotisation inconnu : {}. Connus : {}.", socle::fichiers::repr(code), connus.join(", "))
        })?;
        Ok(CleDeCotisationDsn { code, affectation })
    }
}

impl std::fmt::Display for CleDeCotisationDsn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.affectation {
            Some(a) => write!(f, "{}:{}", self.code, a),
            None => write!(f, "{}", self.code),
        }
    }
}

impl TryFrom<String> for CleDeCotisationDsn {
    type Error = socle::echec::Echec;
    fn try_from(s: String) -> socle::echec::R<CleDeCotisationDsn> {
        CleDeCotisationDsn::lire(&s)
    }
}

/// LA CLE DE TOTALISATION D'UNE COTISATION DE LA DSN : le versement (bloc S21.G00.20) dont le montant la compte. Le
/// montant d'un versement est le « montant des cotisations réglées » (S21.G00.20.005,
/// `atelier/reference/norme-dsn/texte/bloc-S21.G00.20.txt` l. 164-166) ; il est la somme des cles de ce versement
/// (`organismes::Versement::codes81`, `organismes-base.json`). Une cle est le code de la cotisation (81.001) et, quand
/// ce code va a deux versements selon le contrat, celui qui le recouvre, ecrits `code` ou `code:recouvreur` (`018:urssaf`,
/// `018:chomage`, `040:urssaf`). Une cotisation que le bareme ne cle pas se totalise sous son code (`From`). Une cle
/// vide, un code hors de la liste, un recouvreur inconnu se refusent a la lecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct CleDeTotalisation {
    pub code: CodeCotisation81,
    pub recouvreur: Option<Recouvreur>,
}

/// QUI RECOUVRE UNE COTISATION DONT LE CODE VA A DEUX VERSEMENTS : l'Urssaf (le chomage, l'AGS et la reduction generale
/// d'un permanent), ou l'assurance chomage, France Travail (la part chomage de la reduction generale d'un intermittent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Recouvreur {
    Urssaf,
    Chomage,
}

impl Recouvreur {
    /// Son nom dans une cle (`code:recouvreur`).
    pub fn nom(self) -> &'static str {
        match self {
            Recouvreur::Urssaf => "urssaf",
            Recouvreur::Chomage => "chomage",
        }
    }
}

impl CleDeTotalisation {
    /// La cle ecrite `texte`.
    pub fn lire(texte: &str) -> socle::echec::R<CleDeTotalisation> {
        let (code, recouvreur) = match texte.split_once(':') {
            Some((c, "urssaf")) => (c, Some(Recouvreur::Urssaf)),
            Some((c, "chomage")) => (c, Some(Recouvreur::Chomage)),
            Some((_, r)) => {
                return Err(socle::refus!(
                    "cle de totalisation {} : recouvreur {} inconnu (connus : urssaf, chomage)",
                    socle::fichiers::repr(texte),
                    socle::fichiers::repr(r)
                ))
            }
            None => (texte, None),
        };
        let code = CodeCotisation81::du_code(code).ok_or_else(|| {
            let connus: Vec<&str> = CodeCotisation81::TOUS.iter().map(|c| c.code()).collect();
            socle::refus!("Code de cotisation inconnu : {}. Connus : {}.", socle::fichiers::repr(code), connus.join(", "))
        })?;
        Ok(CleDeTotalisation { code, recouvreur })
    }
}

impl From<CodeCotisation81> for CleDeTotalisation {
    /// La cle d'une cotisation que le bareme ne cle pas : son code.
    fn from(code: CodeCotisation81) -> CleDeTotalisation {
        CleDeTotalisation { code, recouvreur: None }
    }
}

impl std::fmt::Display for CleDeTotalisation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.recouvreur {
            Some(r) => write!(f, "{}:{}", self.code, r.nom()),
            None => write!(f, "{}", self.code),
        }
    }
}

impl TryFrom<String> for CleDeTotalisation {
    type Error = socle::echec::Echec;
    fn try_from(s: String) -> socle::echec::R<CleDeTotalisation> {
        CleDeTotalisation::lire(&s)
    }
}
