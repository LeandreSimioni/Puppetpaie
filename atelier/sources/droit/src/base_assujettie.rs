//! Le code d'une base assujettie de la DSN (S21.G00.78.001) : la liste entiere de la norme, et ce qui est propre a
//! chaque code.

use crate::natures::codes_fermes;

codes_fermes! {
    /// UN CODE DE BASE ASSUJETTIE (S21.G00.78.001), la liste entiere de la norme (cahier technique DSN 2026.1,
    /// `atelier/reference/norme-dsn/texte/bloc-S21.G00.78.txt` l. 209-267 ; 2027,
    /// `texte-2027/bloc-S21.G00.78.txt` l. 163-223, qui ajoute 63 et 64). Ce qu'un barème déclare sous chacun, son
    /// `bloc78` le dit (`forme_bareme::CotisationDsn`).
    BaseAssujettie, "Code de base assujettie", "S21.G00.78.001" {
        BrutePlafonnee = "02",
        BruteDeplafonnee = "03",
        Csg = "04",
        ForfaitSocial = "05",
        AssuranceChomage = "07",
        RetraiteCprpf = "08",
        CompensationMaladieCprpf = "09",
        BruteFiscale = "10",
        Forfaitaire = "11",
        Cice = "12",
        ForfaitSocial8 = "13",
        ForfaitSocial20 = "14",
        BruteIeg = "15",
        ComplementInvaliditeIeg = "16",
        PetitPoolIeg = "17",
        PlafonneeMaladieIeg = "18",
        Crpcen = "19",
        CaissesDeCongesPayes = "20",
        OppbtpPermanents = "21",
        BruteSpecifique = "22",
        ExceptionnelleAgircArrco = "23",
        PlafonneeSpecifique = "24",
        ContributionLiberatoire = "25",
        CongesSpectacles = "27",
        Ircantec = "28",
        PrevoyanceSanteRetraite = "31",
        AvantagesDePreretraite = "33",
        IntemperiesGrosOeuvre = "34",
        IntemperiesSecondOeuvre = "35",
        DerogatoireBatiment = "36",
        TravailleurOccasionnel = "38",
        DerogatoireTravauxPublics = "39",
        DerogatoirePartenairesBatiment = "40",
        CrpnpacTauxNormal = "41",
        CrpnpacTauxMajore = "42",
        PlafonneeExceptionnelleAgircArrco = "43",
        ForfaitSocial16 = "44",
        PlafonneeIcpAgircArrco = "45",
        SrePension = "46",
        SreAccessoires = "47",
        Cnracl = "48",
        Rafp = "49",
        Fspoeie = "50",
        RatpTranche2 = "52",
        Ratp = "53",
        ForfaitSocial10 = "54",
        PenibiliteManutentionPortuaire = "55",
        NouvelleBaseA = "56",
        VersementMobilite = "57",
        EmploiPenitentiaire1 = "59",
        EmploiPenitentiaire2 = "60",
        NouvelleBaseD = "61",
        NouvelleBaseE = "62",
        NouvelleBaseB = "63" depuis "P27V01",
        NouvelleBaseC = "64" depuis "P27V01",
    }
}

impl BaseAssujettie {
    /// LA DEDUCTION FORFAITAIRE SPECIFIQUE ABAT CETTE BASE (`atelier/reference/sources/dfs/LISEZ-MOI.md`, « La DSN ») : 02
    /// et 03, l'assiette des cotisations de securite sociale (CSS R. 242-1, al. 3 ; arrete du 4 septembre 2025 relatif
    /// aux frais professionnels, art. 1er et 9), et 23 et 43, celles d'Agirc-Arrco (cahier d'aide a la codification
    /// Agirc-Arrco CT2026, l. 3490-3498 : « La solution ainsi retenue pour le regime general de la Securite sociale
    /// s'applique en principe au regime AGIRC-ARRCO »). Que la base 78 porte l'assiette abattue, et pas seulement son
    /// 81.003 : le meme cahier declare la « Base VRP multicartes apres abattement pour frais professionnels » en 02 et 03
    /// (l. 6230-6236), et la norme 2026.1, chapitre 2 (p. 60-61,
    /// `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt` l. 2116-2160), part du « montant brut
    /// [...] servant de base au calcul des cotisations de securite sociale » et y AJOUTE « le montant de l'abattement
    /// forfaitaire d'assiette pour frais professionnels » : ce brut-la est abattu. INFERENCE, ecrite comme telle : aucun
    /// texte depose ne le dit d'un artiste en toutes lettres (la fiche net-entreprises 1336 ne dit rien des bases). Ne
    /// sont pas abattues : 04 (la CSG, son propre abattement), 07 (le chomage d'un intermittent, « avant application de
    /// l'abattement », annexes VIII et X, art. 49), 27 (les Conges Spectacles, « avant toute deduction forfaitaire
    /// specifique », bloc 78 p. 292) et 31 (la prevoyance, dont les textes ne renvoient pas a l'assiette de la securite
    /// sociale).
    pub fn abattue_par_la_dfs(self) -> bool {
        matches!(
            self,
            BaseAssujettie::BrutePlafonnee
                | BaseAssujettie::BruteDeplafonnee
                | BaseAssujettie::ExceptionnelleAgircArrco
                | BaseAssujettie::PlafonneeExceptionnelleAgircArrco
        )
    }
}
