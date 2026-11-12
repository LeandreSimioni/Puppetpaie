//! La nature d'une declaration DSN (S20.G00.05.001) : la liste entiere de la norme.

use crate::natures::codes_fermes;

codes_fermes! {
    /// LA NATURE D'UNE DECLARATION (S20.G00.05.001), la liste entiere de la norme (cahier technique DSN 2026.1,
    /// `atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt` l. 46-52 ; 2027, `texte-2027/bloc-S20.G00.05.txt`
    /// l. 47-59, la meme liste). Le fichier l'ecrit et le journal des envois la garde par son code.
    NatureDeLaDeclaration, "Code de nature de la déclaration", "S20.G00.05.001" {
        Mensuelle = "01",
        SignalementArret = "04",
        SignalementReprise = "05",
        SignalementFinDeContrat = "07",
        SignalementAmorcage = "08",
        Substitution = "09",
        SignalementDpae = "10",
    }
}

impl NatureDeLaDeclaration {
    /// La nature en clair, telle que la personne la lit (le signalement d'amorcage est, pour elle, la demande du taux de
    /// prelevement a la source qu'il porte).
    pub fn libelle(self) -> &'static str {
        match self {
            NatureDeLaDeclaration::Mensuelle => "DSN mensuelle",
            NatureDeLaDeclaration::SignalementArret => "Signalement arrêt de travail",
            NatureDeLaDeclaration::SignalementReprise => "Signalement reprise suite à arrêt de travail",
            NatureDeLaDeclaration::SignalementFinDeContrat => "Signalement fin du contrat de travail unique",
            NatureDeLaDeclaration::SignalementAmorcage => "Demande de taux de prélèvement à la source",
            NatureDeLaDeclaration::Substitution => "DSN de substitution",
            NatureDeLaDeclaration::SignalementDpae => "Signalement déclaration préalable à l'embauche",
        }
    }

    /// UN SIGNALEMENT QUE LE LOGICIEL ENVOIE : arret de travail (04), reprise (05), fin du contrat de travail unique (07),
    /// amorcage des donnees variables (08).
    pub fn signalement_envoye(self) -> bool {
        matches!(
            self,
            NatureDeLaDeclaration::SignalementArret
                | NatureDeLaDeclaration::SignalementReprise
                | NatureDeLaDeclaration::SignalementFinDeContrat
                | NatureDeLaDeclaration::SignalementAmorcage
        )
    }
}

codes_fermes! {
    /// L'ENVOI D'UN FICHIER, D'ESSAI OU REEL (S10.G00.00.005, « Code envoi du fichier d'essai ou réel »), la liste entiere
    /// de la norme (cahier technique DSN 2026.1, `atelier/reference/norme-dsn/texte/bloc-S10.G00.00.txt` ; relevee dans
    /// `formats-dsn-*-base.json`).
    EssaiOuReel, "Code envoi du fichier d'essai ou réel", "S10.G00.00.005" {
        Essai = "01",
        Reel = "02",
    }
}

codes_fermes! {
    /// LA NATURE DE L'EVENEMENT DECLENCHEUR D'UN SIGNALEMENT D'AMORCAGE DES DONNEES VARIABLES (S20.G00.05.011), la liste
    /// entiere de la norme (cahier technique DSN 2026.1, `atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt` ;
    /// relevee dans `formats-dsn-*-base.json`). Ceux que le logiciel envoie : `declarations::amorcage`.
    NatureDEvenementDeclencheur, "Nature de l'événement déclencheur", "S20.G00.05.011" {
        Embauche = "01",
        Mutation = "02",
        FinDeDispenseDAffiliation = "03",
        ChangementDeParametresDuContratCollectif = "04",
        NouveauContratCollectif = "05",
        AppelDeTauxNonSalarie = "06",
        ChangementDeDeclarant = "07",
    }
}
