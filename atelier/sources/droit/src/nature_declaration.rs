//! La nature d'une declaration DSN (S20.G00.05.001) : la liste entiere de la norme.

use socle::mode::Mode;

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

/// LE MODE D'UN ENVOI (`socle::mode::Mode`), TEL QUE LA DSN L'ECRIT (S10.G00.00.005, « Code envoi du fichier d'essai ou
/// réel ») : `01` essai, `02` reel, la liste entiere de la norme (cahier technique DSN 2026.1,
/// `atelier/reference/norme-dsn/texte/bloc-S10.G00.00.txt` ; relevee dans `formats-dsn-*-base.json`). Un bilan, un
/// rapport, un NEOReS (RO.B000.R013) et un compte rendu d'organisme complementaire (`CodeEnvoi`) le reprennent.
impl crate::natures::ListeDeLaNorme for Mode {
    const RUBRIQUE: crate::natures::RubriqueDeLaNorme = crate::natures::RubriqueDeLaNorme::du_moteur("S10.G00.00.005");

    fn toutes() -> &'static [Mode] {
        &[Mode::Test, Mode::Reel]
    }

    fn code_de_la_norme(self) -> &'static str {
        match self {
            Mode::Test => "01",
            Mode::Reel => "02",
        }
    }

    fn du_code_de_la_norme(code: &str) -> Option<Mode> {
        match code {
            "01" => Some(Mode::Test),
            "02" => Some(Mode::Reel),
            _ => None,
        }
    }

    fn quoi() -> &'static str {
        "Code envoi du fichier d'essai ou réel"
    }

    fn depuis(self) -> Option<crate::natures::VersionDeLaNorme> {
        None
    }

    fn jusqu_a(self) -> Option<crate::natures::VersionDeLaNorme> {
        None
    }
}

/// L'ECRITURE D'UN MODE PAR SON CODE DE LA NORME (S10.G00.00.005), celle d'un retour lu qu'une route rend : `01`, `02`.
pub mod code_d_envoi {
    use socle::mode::Mode;

    use crate::natures::ListeDeLaNorme;

    pub fn serialize<S: serde::Serializer>(m: &Option<Mode>, s: S) -> Result<S::Ok, S::Error> {
        match m {
            Some(m) => s.serialize_some(m.code_de_la_norme()),
            None => s.serialize_none(),
        }
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
