//! Les natures de contrat : ce que la DSN ecrit de la STRUCTURE d'un contrat, lu dans `donnees/contrats-base.json`.
//!
//! Ce module ne decide rien et ne calcule rien : il lit le fichier par sa forme (`lu`, la seule lecture), et rend des
//! valeurs.

use serde::{Deserialize, Serialize};
use socle::fichiers::{forme_en_cache, repr, FormeGardee};
use socle::echec::R;
use socle::num::Decimal;
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

/// Les types de contrat de la DPAE (Format 120 v5.3, C_TYPE_CONTRAT).
pub const TYPES_CONTRAT_DPAE: [(&str, &str); 3] = [("1", "CDD"), ("2", "CDI"), ("3", "CTT")];

/// La face de paie d'une nature : aux cachets et aux heures, ou au mois.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Paie {
    Activite,
    Mensuelle,
}

/// La rubrique du motif de recours (S21.G00.40.021), dont une nature peut dire les valeurs (`champs`).
const MOTIF_DE_RECOURS: &str = "S21.G00.40.021";

/// UNE LISTE DE VALEURS DE LA NORME DSN, close : la rubrique qu'elle remplit, chaque code, et la version de la norme ou
/// il entre dans la liste (`depuis`) ou en sort (`jusqu_a`) quand il n'est pas dans toutes. L'ensemble de ses codes est
/// l'union des listes des versions livrees, et chaque code n'est que dans celles que ses bornes disent : un test de
/// structure le tient, contre les listes relevees (`moteur/tests/fichiers/structure.rs`,
/// `formats-dsn-*-base.json`). La DSN n'ecrit un code que dans un fichier d'une version qui l'a
/// (`declarations::dsn::Fichier::ecrire_code`).
pub trait ListeDeLaNorme: Copy + std::fmt::Display + 'static {
    /// La rubrique que la liste remplit.
    const RUBRIQUE: &'static str;
    /// Toutes ses valeurs, dans l'ordre de leurs codes.
    fn toutes() -> &'static [Self];
    /// Son code.
    fn code_de_la_norme(self) -> &'static str;
    /// Celle de ce code, s'il en est une.
    fn du_code_de_la_norme(code: &str) -> Option<Self>;
    /// Ce que la liste est, pour une phrase (« Motif de recours »).
    fn quoi() -> &'static str;
    /// La premiere version de la norme dont la liste a ce code (`None` : des la plus ancienne livree).
    fn depuis(self) -> Option<&'static str>;
    /// La derniere version de la norme dont la liste a ce code (`None` : jusqu'a la plus recente livree).
    fn jusqu_a(self) -> Option<&'static str>;
}

/// UNE LISTE FERMEE DE CODES DE LA NORME, declaree une fois : chaque variante et son code, le seul endroit ou il s'ecrit.
/// Elle se lit et s'ecrit par son code ; un code qu'elle n'a pas se refuse a la lecture, en disant ceux qu'elle a.
/// Dans l'ordre de leurs codes : un objet indexe par elle s'ecrit comme quand son code etait un texte.
macro_rules! codes_fermes {
    ($(#[$a:meta])* $nom:ident, $quoi:literal, $rubrique:literal {
        $( $(#[$va:meta])* $v:ident = $code:literal $(depuis $depuis:literal)? $(jusqu_a $jusqu:literal)?, )*
    }) => {
        $(#[$a])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $nom {
            $( $(#[$va])* $v, )*
        }

        impl $nom {
            pub const TOUS: &'static [$nom] = &[ $( $nom::$v, )* ];

            /// Son code, le seul endroit ou il s'ecrit.
            pub fn code(self) -> &'static str {
                match self { $( $nom::$v => $code, )* }
            }

            /// Celui de ce code, s'il en est un.
            pub fn du_code(code: &str) -> Option<$nom> {
                $nom::TOUS.iter().copied().find(|m| m.code() == code)
            }
        }

        impl $crate::natures::ListeDeLaNorme for $nom {
            const RUBRIQUE: &'static str = $rubrique;

            fn toutes() -> &'static [$nom] {
                $nom::TOUS
            }

            fn code_de_la_norme(self) -> &'static str {
                self.code()
            }

            fn du_code_de_la_norme(code: &str) -> Option<$nom> {
                $nom::du_code(code)
            }

            fn quoi() -> &'static str {
                $quoi
            }

            fn depuis(self) -> Option<&'static str> {
                // La borne que la declaration ecrit, ou aucune.
                match self { $( $nom::$v => <[&'static str]>::first(&[$( $depuis )?]).copied(), )* }
            }

            fn jusqu_a(self) -> Option<&'static str> {
                match self { $( $nom::$v => <[&'static str]>::first(&[$( $jusqu )?]).copied(), )* }
            }
        }

        impl Ord for $nom {
            fn cmp(&self, autre: &Self) -> std::cmp::Ordering {
                self.code().cmp(autre.code())
            }
        }

        impl PartialOrd for $nom {
            fn partial_cmp(&self, autre: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(autre))
            }
        }

        impl std::fmt::Display for $nom {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str(self.code())
            }
        }

        impl serde::Serialize for $nom {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.code())
            }
        }

        impl<'de> serde::Deserialize<'de> for $nom {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let code = <String as serde::Deserialize>::deserialize(d)?;
                $nom::du_code(&code).ok_or_else(|| {
                    let connus: Vec<&str> = $nom::TOUS.iter().map(|m| m.code()).collect();
                    serde::de::Error::custom(format!("{} inconnu : {}. Connus : {}.", $quoi, socle::fichiers::repr(&code), connus.join(", ")))
                })
            }
        }
    };
}
pub(crate) use codes_fermes;

codes_fermes! {
    /// UN MOTIF DE RECOURS (S21.G00.40.021), la liste entiere de la norme (cahier technique DSN 2026.1 p. 194-195,
    /// `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt` ; classeur 2027.1.2, type `DSN_Motifs_Recours`). Ceux
    /// qu'une nature offre a la saisie, et ce que chacun porte en plus (la personne remplacee), `contrats-base.json` le
    /// dit (`natures.cdd.champs."S21.G00.40.021".valeurs`, `Proprietes::motif_de_remplacement`,
    /// `Proprietes::motif_recours_refuse`) ; le CDDU fixe le sien (05, `Proprietes::motif_recours_fixe`).
    MotifDeRecours, "Motif de recours", "S21.G00.40.021" {
        Remplacement = "01",
        Accroissement = "02",
        Saisonnier = "03",
        Vendanges = "04",
        ContratDUsage = "05",
        ObjetDefini = "06",
        RemplacementDUnChefDEntreprise = "07",
        RemplacementDUnChefDExploitationAgricole = "08",
        Insertion = "09",
        ComplementDeFormation = "10",
        Apprentissage = "11",
        RemplacementATempsPartiel = "12",
        AttenteDeSuppression = "13",
        ContratDeVoyage = "14",
        InterimaireBoeth = "15",
    }
}

codes_fermes! {
    /// UN MOTIF DE RUPTURE (S21.G00.62.002), la liste entiere de la norme (cahier technique DSN 2026.1 p. 270-274,
    /// `atelier/reference/norme-dsn/texte/bloc-S21.G00.62.txt` ; classeur 2027.1.2, type
    /// `RAC_Rupture_Motif_DSN_phase_1`). Ceux qu'une nature permet, `contrats-base.json` le dit
    /// (`natures.<n>.fin_de_contrat.motifs`, `Proprietes::motif_rupture_refuse`). 072 n'est que dans la liste 2027
    /// (`texte-2027/bloc-S21.G00.62.txt` l. 245).
    MotifDeRupture, "Motif de rupture", "S21.G00.62.002" {
        LicenciementLiquidationJudiciaire = "011",
        LicenciementFermetureDeLEtablissement = "012",
        LicenciementEconomique = "014",
        LicenciementFinDeChantier = "015",
        LicenciementAutreMotif = "020",
        AutreFinEconomique = "025",
        RuptureEconomiqueCsp = "026",
        FinDeCdd = "031",
        FinDeMissionDInterim = "032",
        InaptitudeDuCdd = "033",
        FinDEssaiParLEmployeur = "034",
        FinDEssaiParLeSalarie = "035",
        RuptureAnticipeeParLEmployeur = "036",
        RuptureAnticipeeParLeSalarie = "037",
        MiseALaRetraite = "038",
        DepartALaRetraite = "039",
        RuptureConventionnelle = "043",
        PriseDActe = "058",
        Demission = "059",
        DecesDeLEmployeur = "065",
        DecesDuSalarie = "066",
        RevocationAbandonDePoste = "070",
        RevocationSanctionPenale = "071",
        RevocationSanctionDisciplinaire = "072" depuis "P27V01",
        FinDApprentissage = "081",
        ResiliationJudiciaire = "082",
        ForceMajeure = "083",
        CommunAccord = "084",
        FinDeMandat = "085",
        LicenciementCats = "086",
        LicenciementFauteGrave = "087",
        LicenciementFauteLourde = "088",
        LicenciementForceMajeure = "089",
        InaptitudeNonProfessionnelle = "091",
        InaptitudeProfessionnelle = "092",
        LicenciementDecisionAdministrative = "093",
        ArretDeTournage = "094",
        RuptureAnticipeeFauteGrave = "095",
        RuptureAnticipeeFauteLourde = "096",
        RuptureAnticipeeFermeture = "097",
        RetraitDEnfant = "098",
        Annulation = "099",
        Mutation = "100",
        RuptureConventionnelleCollective = "110",
        CongeDeMobilite = "111",
        AccordDePerformanceCollective = "112",
        LicenciementMotifSpecifique = "113",
        EntreeEnPap = "114",
        LicenciementLoiDe2017 = "115",
        FinDeLigneDeService = "116",
        LicenciementRefusCsp = "117",
        FinDeCape = "118",
        FinDAffectation = "119",
        TransfertSansRupture = "998",
        FinDeRelation = "999",
    }
}

codes_fermes! {
    /// LA MODALITE D'EXERCICE DU TEMPS DE TRAVAIL (S21.G00.40.014), la liste entiere de la norme (cahier technique DSN
    /// 2026.1 p. 189, `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt` ; classeur 2027.1.2, type
    /// `Travail_Modalite`). Le CDDU fixe la sienne (99, `Proprietes::modalite_fixee`) ; un engagement d'une autre
    /// nature la dit, ou la calcule (`engagement::Engagement::modalite_temps_travail`).
    ModaliteDuTempsDeTravail, "Modalité d'exercice du temps de travail", "S21.G00.40.014" {
        TempsPlein = "10",
        TempsPartiel = "20",
        TempsAlterne = "30",
        Cpa2004 = "40",
        TempsPartielDeDroit = "41",
        TempsPartielDeDroitPourEnfant = "42",
        NonConcerne = "99",
    }
}

/// UNE NATURE DE CONTRAT, et la seule facon d'en nommer une : personne n'en ajoute hors du logiciel, une nature neuve
/// arrive par une mise a jour. Elle s'ecrit par sa cle (`cle`), celle de `natures` dans `contrats-base.json`, qui dit ses
/// proprietes (`proprietes`) ; une cle qui n'en est pas une se refuse a la lecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NatureDeContrat {
    CdduSpectacle,
    Cdi,
    Cdd,
}

impl NatureDeContrat {
    pub const TOUTES: [NatureDeContrat; 3] = [NatureDeContrat::CdduSpectacle, NatureDeContrat::Cdi, NatureDeContrat::Cdd];

    /// Sa cle, le seul endroit ou elle s'ecrit.
    pub fn cle(self) -> &'static str {
        match self {
            NatureDeContrat::CdduSpectacle => "cddu_spectacle",
            NatureDeContrat::Cdi => "cdi",
            NatureDeContrat::Cdd => "cdd",
        }
    }

    /// LA SEULE LECTURE D'UNE NATURE : ses proprietes, telles que `contrats-base.json` les dit.
    pub fn proprietes(self) -> R<Arc<Nature>> {
        Ok(lu()?.par_nature[self as usize].clone())
    }
}

/// Dans l'ordre de leurs cles : un objet indexe par nature s'ecrit comme quand sa cle etait un texte.
impl Ord for NatureDeContrat {
    fn cmp(&self, autre: &Self) -> std::cmp::Ordering {
        self.cle().cmp(autre.cle())
    }
}

impl PartialOrd for NatureDeContrat {
    fn partial_cmp(&self, autre: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(autre))
    }
}

impl std::fmt::Display for NatureDeContrat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.cle())
    }
}

impl Serialize for NatureDeContrat {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.cle())
    }
}

impl<'de> Deserialize<'de> for NatureDeContrat {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let cle = String::deserialize(d)?;
        match NatureDeContrat::TOUTES.into_iter().find(|n| n.cle() == cle) {
            Some(n) => Ok(n),
            None => {
                let mut connues: Vec<&str> = NatureDeContrat::TOUTES.iter().map(|n| n.cle()).collect();
                connues.sort();
                Err(serde::de::Error::custom(format!("Nature de contrat inconnue : {}. Connues : {}.", repr(&cle), connues.join(", "))))
            }
        }
    }
}

/// UNE NATURE DE CONTRAT ET SES PROPRIETES (`Proprietes`, `natures.<cle>` de `contrats-base.json`).
#[derive(Debug, Clone, PartialEq)]
pub struct Nature {
    pub cle: NatureDeContrat,
    proprietes: Proprietes,
}

impl std::ops::Deref for Nature {
    type Target = Proprietes;
    fn deref(&self) -> &Proprietes {
        &self.proprietes
    }
}

/// CE QUE `contrats-base.json` DIT D'UNE NATURE (`natures.<cle>`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proprietes {
    pub libelle: String,
    /// Ce que le nom court veut dire, pour qui hesite en choisissant.
    pub aide: String,
    #[serde(rename = "source")]
    _source: Option<String>,
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
    pub paie: Paie,
    pub duree_indeterminee: bool,
    bareme: Bareme,
    unite_quotite: UniteDeQuotite,
    #[serde(rename = "quotite_reference_defaut")]
    quotite_reference: Option<QuotiteDeReference>,
    /// rubrique -> valeur fixe.
    codes: CodesFixes,
    /// rubrique -> d'ou la valeur vient.
    champs: BTreeMap<String, Champ>,
    /// rubrique -> pourquoi cette nature ne l'ecrit pas.
    #[serde(rename = "absents")]
    _absents: BTreeMap<String, String>,
    /// rubrique -> le controle qui la refuse pour cette nature.
    interdits: BTreeMap<String, String>,
    /// Le contrat ecrit : son modele, celui de son avenant, ses mentions obligatoires.
    contrat_ecrit: Option<ContratEcrit>,
    dpae: Dpae,
    fin_de_contrat: FinDeContrat,
    remunerations: Remunerations,
    activites: Activites,
}

/// La section `par_nature` du bareme a appliquer par-dessus le statut (`None` : le corps du bareme).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bareme {
    section: Option<String>,
    #[serde(rename = "note")]
    _note: String,
}

/// S21.G00.40.011, et le plafond de 40.012 et 40.013 (CCH-12 du bloc 40, 31.00 en journee ; CCH-13, 250.00 en heure).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct UniteDeQuotite {
    valeur: String,
    #[serde(deserialize_with = "socle::forme::court")]
    plafond: Decimal,
    #[serde(rename = "quoi")]
    _quoi: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuotiteDeReference {
    #[serde(deserialize_with = "socle::forme::court")]
    valeur: Decimal,
    #[serde(rename = "quoi")]
    _quoi: String,
}

/// Une rubrique a valeur fixe, et ce que cette valeur dit.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeFixe {
    valeur: String,
    #[serde(rename = "quoi")]
    _quoi: String,
}

/// LES VALEURS FIXES D'UNE NATURE (`codes` de `contrats-base.json`) : celles d'une liste que le logiciel declare
/// (`ListeDeLaNorme` : la modalite du temps de travail, le motif de recours) se lisent dans leur `enum` ; les autres
/// restent le texte que la DSN ecrit.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "BTreeMap<String, CodeFixe>")]
struct CodesFixes {
    modalite: Option<ModaliteDuTempsDeTravail>,
    motif_recours: Option<MotifDeRecours>,
    autres: BTreeMap<String, CodeFixe>,
}

/// La valeur fixe de la liste `T`, otee de `codes` : un code que la liste n'a pas se refuse, par l'erreur que la lecture
/// de serde attend (`CodesFixes::try_from`).
fn code_fixe<T: ListeDeLaNorme>(codes: &mut BTreeMap<String, CodeFixe>) -> Result<Option<T>, <CodesFixes as TryFrom<BTreeMap<String, CodeFixe>>>::Error> {
    match codes.remove(T::RUBRIQUE) {
        None => Ok(None),
        Some(c) => T::du_code_de_la_norme(&c.valeur).map(Some).ok_or_else(|| {
            let connus: Vec<&str> = T::toutes().iter().map(|m| m.code_de_la_norme()).collect();
            socle::echec::refus(format!("{} {} : {} inconnu. Connus : {}.", T::RUBRIQUE, T::quoi(), repr(&c.valeur), connus.join(", ")))
        }),
    }
}

impl TryFrom<BTreeMap<String, CodeFixe>> for CodesFixes {
    type Error = socle::echec::Echec;
    fn try_from(mut codes: BTreeMap<String, CodeFixe>) -> Result<CodesFixes, Self::Error> {
        let modalite = code_fixe(&mut codes)?;
        let motif_recours = code_fixe(&mut codes)?;
        Ok(CodesFixes { modalite, motif_recours, autres: codes })
    }
}

/// Une rubrique dont la valeur se lit sur l'engagement ou la fiche. Le motif de recours (S21.G00.40.021) d'un CDD porte
/// en plus ses `valeurs` et s'il est `obligatoire`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Champ {
    #[serde(rename = "source")]
    _source: String,
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: Option<String>,
    obligatoire: Option<bool>,
    valeurs: Option<Vec<RecoursOffert>>,
}

/// UN MOTIF DE RECOURS (S21.G00.40.021) qu'une nature propose.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoursOffert {
    pub code: MotifDeRecours,
    pub libelle: String,
    /// Le CDD conclu au titre du 1° de L1242-2 nomme la personne remplacee (L1242-12, 1°).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personne_remplacee: Option<bool>,
}

/// UN MOTIF DE RUPTURE (S21.G00.62.002) que la CCH-11 autorise pour la nature, et ce qu'il ouvre (`dit`, sa `source`).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuptureOfferte {
    pub code: MotifDeRupture,
    pub libelle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContratEcrit {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "source")]
    _source: String,
    #[serde(rename = "note")]
    _note: String,
    /// Le nom du contrat, tel qu'une phrase le dit (« contrat de travail à durée déterminée »).
    nom: String,
    modele: String,
    avenant: String,
    phrases: Phrases,
    mentions: Vec<Mention>,
}

/// LES PHRASES DE RENVOI du contrat ecrit (R. 1221-35 permet le renvoi aux textes) ; le motif, celui d'une nature qui
/// fixe le sien (le CDDU).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phrases {
    pub motif: Option<String>,
    pub formation: String,
    pub conges: String,
    pub rupture: String,
}

/// UNE MENTION que la loi exige du contrat ecrit : son marqueur, le texte qui l'exige, et si elle n'est due que dite.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mention {
    pub marqueur: String,
    pub texte: String,
    pub si_dite: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Dpae {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "source")]
    _source: crate::source::Fondement,
    type_contrat: TypeContratDpae,
    date_fin: DateFinDpae,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeContratDpae {
    valeur: String,
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct DateFinDpae {
    obligatoire: bool,
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
}

/// Le bloc S21.G00.62.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct FinDeContrat {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
    /// Le motif quand le contrat va a son terme (`None` pour un CDI : il se saisit).
    motif_par_defaut: Option<MotifParDefaut>,
    motifs: Vec<RuptureOfferte>,
    signalement_fctu: SignalementFctu,
    circuit_derogatoire: Option<CircuitDerogatoire>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct MotifParDefaut {
    valeur: MotifDeRupture,
    #[serde(rename = "quoi")]
    _quoi: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignalementFctu {
    valeur: bool,
    #[serde(rename = "note")]
    _note: String,
}

/// La modalite de declaration de la fin d'un contrat d'usage (S21.G00.62.017).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct CircuitDerogatoire {
    rubrique: String,
    motifs: Vec<MotifDeRupture>,
    applique: String,
    non_applique: String,
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "source")]
    _source: String,
    #[serde(rename = "note")]
    _note: String,
}

/// Les blocs S21.G00.51 d'une paie.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Remunerations {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: Option<String>,
    types: Vec<TypeDeRemuneration>,
    /// Les quatre blocs 51 valent-ils tous le brut ? (vrai sur un CDDU)
    toutes_au_brut: bool,
}

/// UN TYPE DE REMUNERATION OBLIGATOIRE (S21.G00.51.011, cahier technique § 2.1.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum TypeDeRemuneration {
    /// 001 - Rémunération brute non plafonnée.
    #[serde(rename = "001")]
    Brute,
    /// 002 - Salaire brut servant aux calculs des droits de l'Assurance chômage.
    #[serde(rename = "002")]
    AssietteChomage,
    /// 003 - Salaire rétabli - reconstitué.
    #[serde(rename = "003")]
    SalaireRetabli,
    /// 010 - Salaire de base.
    #[serde(rename = "010")]
    SalaireDeBase,
}

impl TypeDeRemuneration {
    pub fn code(self) -> &'static str {
        match self {
            TypeDeRemuneration::Brute => "001",
            TypeDeRemuneration::AssietteChomage => "002",
            TypeDeRemuneration::SalaireRetabli => "003",
            TypeDeRemuneration::SalaireDeBase => "010",
        }
    }
}

/// Les blocs S21.G00.53 sous la remuneration de type 002.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Activites {
    #[serde(rename = "quoi")]
    _quoi: String,
    unites: Vec<UniteDActivite>,
    absences: Option<UniteDAbsence>,
}

/// Ce qu'une unite d'activite (S21.G00.53) compte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mesure {
    Jours,
    Cachets,
    Heures,
}

/// UNE UNITE D'ACTIVITE (S21.G00.53.003), le type d'activite (53.001) et ce qu'elle mesure.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UniteDActivite {
    pub code: String,
    pub quoi: String,
    pub mesure: Mesure,
    #[serde(rename = "type")]
    pub type_activite: String,
    /// Elle ne s'ecrit pas quand elle vaut zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si_non_nul: Option<bool>,
    /// Elle ne s'ecrit que pour un contrat declare a la caisse des Conges Spectacles (`caisse_conges::affilie`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si_caisse_conges: Option<bool>,
}

/// L'unite des absences (S21.G00.53, type 02).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UniteDAbsence {
    pub code: String,
    #[serde(rename = "type")]
    pub type_activite: String,
    pub quoi: String,
}

/// LA CLE DE LA PIECE D'UN PERMANENT (`fiche::PieceFermee`) : seule une nature payee au mois la donne
/// (`Nature::au_mois`). Les cases d'un salarie au mois ne se lisent qu'avec elle ; un calcul qui n'a pas en main la nature
/// d'un engagement au mois ne les lit pas, et la fiche ne les montre qu'a qui peut en avoir un
/// (`routes::fiches::montres`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuMois(());

impl Proprietes {
    /// Paye au mois : un permanent.
    pub fn mensuelle(&self) -> bool {
        self.paie == Paie::Mensuelle
    }

    /// La cle de la piece d'un permanent, si la nature est payee au mois.
    pub fn au_mois(&self) -> Option<AuMois> {
        self.mensuelle().then_some(AuMois(()))
    }

    pub fn permanent(&self) -> bool {
        self.mensuelle()
    }

    /// La section `par_nature` du bareme a appliquer par-dessus le statut, ou `None` : le corps du bareme.
    pub fn bareme(&self) -> Option<&str> {
        self.bareme.section.as_deref()
    }

    /// S21.G00.40.011.
    pub fn unite_quotite(&self) -> &str {
        &self.unite_quotite.valeur
    }

    /// Le plafond de 40.012 et 40.013 : CCH-12 du bloc 40, 31.00 en journee ; CCH-13, 250.00 en heure.
    pub fn plafond_quotite(&self) -> Decimal {
        self.unite_quotite.plafond
    }

    /// La quotite de reference d'un contrat au mois (la duree legale, L. 3121-27), ou `None` : la nature n'en a pas.
    pub fn quotite_reference_defaut(&self) -> Option<Decimal> {
        self.quotite_reference.as_ref().map(|q| q.valeur)
    }

    /// Le modele livre du contrat ecrit de cette nature, ou `None` : elle n'en a pas.
    pub fn modele_contrat(&self) -> Option<String> {
        self.contrat_ecrit.as_ref().map(|c| c.modele.clone())
    }

    /// Le nom de son contrat ecrit, tel qu'une phrase le dit (« contrat de travail à durée déterminée »).
    pub fn nom_du_contrat(&self) -> String {
        match &self.contrat_ecrit {
            Some(c) => c.nom.clone(),
            None => "contrat de travail".into(),
        }
    }

    /// Le modele livre de l'avenant de son contrat ecrit.
    pub fn modele_avenant(&self) -> Option<String> {
        self.contrat_ecrit.as_ref().map(|c| c.avenant.clone())
    }

    /// Les phrases de renvoi de son contrat ecrit.
    pub fn phrases(&self) -> Option<&Phrases> {
        self.contrat_ecrit.as_ref().map(|c| &c.phrases)
    }

    /// Les mentions obligatoires de son contrat ecrit, dans l'ordre de `contrats-base.json`.
    pub fn mentions_obligatoires(&self) -> Option<&[Mention]> {
        self.contrat_ecrit.as_ref().map(|c| c.mentions.as_slice())
    }

    /// Le motif de recours `code` appelle-t-il le nom et la qualification de la personne remplacee ? (L1242-12, 1° :
    /// le CDD conclu au titre du 1° de L1242-2.)
    pub fn motif_de_remplacement(&self, code: MotifDeRecours) -> bool {
        self.motifs_recours().into_iter().flatten().any(|m| m.code == code && m.personne_remplacee == Some(true))
    }

    /// Le libelle du motif de recours `code`, tel que `contrats-base.json` le dit.
    pub fn libelle_motif_recours(&self, code: MotifDeRecours) -> Option<String> {
        self.motifs_recours().into_iter().flatten().find(|m| m.code == code).map(|m| m.libelle.clone())
    }

    /// La valeur fixe d'une rubrique pour cette nature, ou `None` (absente comme interdite : `interdit` les distingue).
    /// La modalite du temps de travail et le motif de recours fixes se lisent dans leur `enum` (`modalite_fixee`,
    /// `motif_recours_fixe`).
    pub fn code(&self, rubrique: &str) -> Option<String> {
        self.codes.autres.get(rubrique).map(|c| c.valeur.clone())
    }

    /// La modalite du temps de travail (S21.G00.40.014) que la nature fixe (le CDDU : 99), ou `None` : elle se dit sur
    /// l'engagement.
    pub fn modalite_fixee(&self) -> Option<ModaliteDuTempsDeTravail> {
        self.codes.modalite
    }

    /// Le motif de recours (S21.G00.40.021) que la nature fixe (le CDDU : 05, contrat d'usage), ou `None`.
    pub fn motif_recours_fixe(&self) -> Option<MotifDeRecours> {
        self.codes.motif_recours
    }

    /// Le controle qui refuse cette rubrique pour cette nature, ou `None`.
    pub fn interdit(&self, rubrique: &str) -> Option<String> {
        self.interdits.get(rubrique).cloned()
    }

    /// Cette nature ecrit-elle cette rubrique ? (valeur fixe ou champ)
    pub fn ecrit(&self, rubrique: &str) -> bool {
        let liste = (rubrique == ModaliteDuTempsDeTravail::RUBRIQUE && self.codes.modalite.is_some())
            || (rubrique == MotifDeRecours::RUBRIQUE && self.codes.motif_recours.is_some());
        liste || self.codes.autres.contains_key(rubrique) || self.champs.contains_key(rubrique)
    }

    /// Le motif de rupture quand le contrat va a son terme (`None` pour un CDI).
    pub fn motif_rupture_par_defaut(&self) -> Option<MotifDeRupture> {
        self.fin_de_contrat.motif_par_defaut.as_ref().map(|m| m.valeur)
    }

    pub fn motifs_rupture(&self) -> &[RuptureOfferte] {
        &self.fin_de_contrat.motifs
    }

    /// Le refus d'un motif de rupture (`S21.G00.62.002`) que cette nature ne declare pas, ou `None`. Les `motifs` de
    /// `contrats-base.json` sont ceux que la CCH-11 de la rubrique autorise pour la nature (cahier technique DSN
    /// 2026.1, p. 270-273, `norme-dsn/texte/bloc-S21.G00.62.txt`) : un motif hors de la liste serait rejete (091 et 092,
    /// l'inaptitude, ne valent que pour les natures 01, 07, 08, 09, 50, 53, 54, 82, 91 ; un CDD rompu pour inaptitude
    /// est en 033). Le CDDU, de nature 02, a la liste du CDD. Une nature sans liste n'est pas controlee.
    pub fn motif_rupture_refuse(&self, motif: MotifDeRupture, numero: &str) -> Option<String> {
        let motifs = self.motifs_rupture();
        if motifs.is_empty() || motifs.iter().any(|m| m.code == motif) {
            return None;
        }
        let connus: Vec<String> = motifs.iter().map(|m| format!("{} ({})", m.code, m.libelle)).collect();
        Some(format!(
            "Contrat {} ({}) : le motif de rupture {} n'est pas autorise pour cette nature de contrat (CCH-11 de S21.G00.62.002, cahier technique DSN p. 270-273) ; la declaration serait rejetee. Motifs : {}. Rien n'est ecrit.",
            numero,
            self.libelle,
            motif,
            connus.join(", ")
        ))
    }

    /// Les motifs de recours (S21.G00.40.021) que cette nature accepte.
    pub fn motifs_recours(&self) -> Option<&[RecoursOffert]> {
        self.champs.get(MOTIF_DE_RECOURS).and_then(|c| c.valeurs.as_deref())
    }

    /// Le refus d'un motif de recours saisi (`S21.G00.40.021`) que cette nature n'accepte pas, ou `None` : la nature en
    /// propose une partie (`valeurs` de `contrats-base.json`). Une nature sans liste (le CDDU, qui fixe le sien ; le CDI)
    /// n'est pas controlee.
    pub fn motif_recours_refuse(&self, motif: MotifDeRecours, qui: &str) -> Option<String> {
        let motifs = match self.motifs_recours() {
            Some(m) if !m.is_empty() && !m.iter().any(|m| m.code == motif) => m,
            _ => return None,
        };
        let connus: Vec<String> = motifs.iter().map(|m| format!("{} ({})", m.code, m.libelle)).collect();
        Some(format!(
            "« {} » n'est pas un motif de recours d'un {} ({}) ; la DSN le déclare par son code (S21.G00.40.021). Choisir l'un de : {}. Rien n'est écrit.",
            motif,
            self.libelle,
            qui,
            connus.join(", ")
        ))
    }

    pub fn motif_recours_exige(&self) -> bool {
        self.champs.get(MOTIF_DE_RECOURS).is_some_and(|c| c.obligatoire == Some(true))
    }

    /// La fin de ce contrat, pour ce motif de rupture (S21.G00.62.002 ; `None` : le defaut de la nature), part-elle par
    /// le circuit derogatoire ? Pour un CDDU, le seul 031 (CCH-12 de S21.G00.62.017, fiche 2062).
    pub fn circuit_derogatoire_pour(&self, motif: Option<MotifDeRupture>) -> bool {
        self.fin_de_contrat.circuit_derogatoire.as_ref().is_some_and(|c| motif.or_else(|| self.motif_rupture_par_defaut()).is_some_and(|m| c.motifs.contains(&m)))
    }

    /// LA REGLE, ET SON SEUL ENDROIT : la fin de ce contrat, pour ce motif de rupture (`None` : le defaut de la
    /// nature), se declare-t-elle par un signalement de fin de contrat (nature 07) ? Toute fin de contrat donne lieu a
    /// l'attestation employeur (R. 1234-9), par le FCTU en DSN (fiche net-entreprises 119) ; le CDDU n'en est dispense
    /// que par le circuit derogatoire, pour le 031 : « Tous les autres motifs de rupture doivent etre declares par
    /// l'emission d'un signalement FCTU » (fiche 2062, `atelier/reference/sources/net-entreprises-base-connaissances/
    /// net-entreprises-fiche2062-derogations-fin-de-contrat-fctu-20250905.txt`). Les listes des fins a signaler (ligne de
    /// commande, page, rappels), le FCTU et l'ecran de saisie la lisent ici.
    pub fn signalement_fctu(&self, motif: Option<MotifDeRupture>) -> bool {
        self.fin_de_contrat.signalement_fctu.valeur && !self.circuit_derogatoire_pour(motif)
    }

    /// Les rubriques du bloc 62 que la nature fixe, pour ce motif de rupture : rubrique -> valeur. La modalite de
    /// declaration de la fin d'un contrat d'usage (S21.G00.62.017) suit le motif : « 01 » au circuit derogatoire,
    /// « 02 » hors de lui (CCH-11 et CCH-12, cahier technique DSN 2026.1 p. 277, `atelier/reference/norme-dsn/texte/
    /// bloc-S21.G00.62.txt`).
    pub fn codes_fin(&self, motif: Option<MotifDeRupture>) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        if let Some(c) = &self.fin_de_contrat.circuit_derogatoire {
            let valeur = if self.circuit_derogatoire_pour(motif) { &c.applique } else { &c.non_applique };
            out.insert(c.rubrique.clone(), valeur.clone());
        }
        out
    }

    /// C_TYPE_CONTRAT du Format 120 : 1 CDD, 2 CDI, 3 CTT.
    pub fn type_contrat_dpae(&self) -> String {
        self.dpae.type_contrat.valeur.clone()
    }

    /// La DPAE de ce contrat porte-t-elle une date de fin ?
    pub fn date_fin_dpae(&self) -> bool {
        self.dpae.date_fin.obligatoire
    }

    pub fn types_remuneration(&self) -> &[TypeDeRemuneration] {
        &self.remunerations.types
    }

    /// Les quatre blocs 51 valent-ils tous le brut ? (vrai sur un CDDU)
    pub fn remunerations_au_brut(&self) -> bool {
        self.remunerations.toutes_au_brut
    }

    pub fn unites_activite(&self) -> &[UniteDActivite] {
        &self.activites.unites
    }

    pub fn unite_absence(&self) -> Option<&UniteDAbsence> {
        self.activites.absences.as_ref()
    }
}

// ------------------------------------ l'abattement << contrats courts >> du PAS

pub const DUREE_CONTRAT_COURT_MOIS: u32 = 2;

use socle::date::Jour;

/// L'abattement contrats courts s'applique-t-il a ce versement ? `debut` et
/// `fin` sont les bornes du CONTRAT SIGNE (`fin` a `None` : terme imprecis),
/// `le` la date du versement. Le terme : la meme quantieme deux mois plus tard, ramenee au dernier jour du mois
/// (`Jour::plus_mois`).
pub fn contrat_court(nat: &Nature, debut: Jour, fin: Option<Jour>, le: Jour) -> R<bool> {
    if nat.duree_indeterminee {
        return Ok(false);
    }
    let terme = debut.plus_mois(i64::from(DUREE_CONTRAT_COURT_MOIS))?;
    if let Some(f) = fin {
        if f > terme {
            return Ok(false);
        }
    }
    Ok(le <= terme)
}

// ------------------------------------------------------------------- lecture

/// LE FICHIER `contrats-base.json`, tel qu'il se lit.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fichier {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
    #[serde(rename = "source")]
    _source: crate::source::Fondement,
    categories_d_emploi: CategoriesDEmploi,
    /// Dans l'ordre du fichier.
    #[serde(deserialize_with = "socle::forme::en_ordre")]
    natures: Vec<(NatureDeContrat, Proprietes)>,
}

/// `categories_d_emploi` de `contrats-base.json`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CategoriesDEmploi {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "source")]
    _source: String,
    /// Le libelle de la norme, par classement conventionnel (S21.G00.40.002).
    par_classement: BTreeMap<String, String>,
    artiste_non_cadre: ArtisteNonCadre,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtisteNonCadre {
    libelle: String,
    #[serde(rename = "note")]
    _note: String,
}

/// Les natures de contrat, construites : dans l'ordre du fichier, chacune a sa place dans `NatureDeContrat::TOUTES`, les
/// categories d'emploi.
struct Contrats {
    natures: Arc<Vec<Arc<Nature>>>,
    par_nature: [Arc<Nature>; 3],
    categories: CategoriesDEmploi,
}

const ILLISIBLE: &str = "Les natures de contrat livrées avec le logiciel ne se lisent pas : le signaler au développeur.";

/// LA SEULE LECTURE DE `contrats-base.json` : sa forme (une cle qui n'est pas une `NatureDeContrat` refuse), puis ce
/// qu'elle ne dit pas seule : chaque nature du logiciel y est, et son type de contrat de la DPAE est l'un des trois du
/// Format 120.
fn lu() -> R<Arc<Contrats>> {
    static CACHE: OnceLock<FormeGardee<Contrats>> = OnceLock::new();
    forme_en_cache(&CACHE, "contrats-base.json", ILLISIBLE, |f: Fichier| {
        let mut natures = Vec::new();
        for (cle, proprietes) in f.natures {
            if !TYPES_CONTRAT_DPAE.iter().any(|(k, _)| *k == proprietes.dpae.type_contrat.valeur) {
                return Err(socle::echec::a_la_cle(ILLISIBLE, &format!("natures.{}.dpae.type_contrat.valeur", cle), format!("{} n'est pas un type de contrat du Format 120 (1, 2 ou 3)", repr(&proprietes.dpae.type_contrat.valeur))));
            }
            natures.push(Arc::new(Nature { cle, proprietes }));
        }
        let de = |n: NatureDeContrat| -> R<Arc<Nature>> {
            match natures.iter().find(|x| x.cle == n) {
                Some(x) => Ok(x.clone()),
                None => Err(socle::echec::a_la_cle(ILLISIBLE, "natures", format!("la nature {} n'y est pas", n))),
            }
        };
        let par_nature = [de(NatureDeContrat::TOUTES[0])?, de(NatureDeContrat::TOUTES[1])?, de(NatureDeContrat::TOUTES[2])?];
        Ok(Contrats { natures: Arc::new(natures), par_nature, categories: f.categories_d_emploi })
    })
}

/// La categorie d'emploi d'un classement conventionnel (S21.G00.40.002), telle que le contrat ecrit l'imprime
/// (`categories_d_emploi` de `contrats-base.json`, libelles de la norme ; R1221-34, 3°). L'`artiste` qui n'est pas cadre
/// (03, 04) : « non-cadre », la categorie de sa convention (3090, art. 6.3 ; 1285, art. XI.2.1), pas le libelle du 05.
pub fn categorie_d_emploi(classement: &str, artiste: bool) -> R<Option<String>> {
    let c = &lu()?.categories;
    if artiste && !matches!(classement, "03" | "04") {
        return Ok(Some(c.artiste_non_cadre.libelle.clone()));
    }
    Ok(c.par_classement.get(classement).cloned())
}

/// Toutes les natures, dans l'ordre du fichier.
pub fn natures() -> R<Arc<Vec<Arc<Nature>>>> {
    Ok(lu()?.natures.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CCH-18 du 40.005 (cahier technique 2026.1, p. 185, `bloc-S21.G00.40.txt` l. 325-329) : « 999SPT pour les emplois
    /// permanents du spectacle » ; un CDDU garde le code de la nomenclature.
    #[test]
    fn la_nature_fixe_999spt_pour_les_permanents_seulement() {
        for n in [NatureDeContrat::Cdi, NatureDeContrat::Cdd] {
            assert_eq!(n.proprietes().unwrap().code("S21.G00.40.005").as_deref(), Some("999SPT"), "{}", n);
        }
        assert_eq!(NatureDeContrat::CdduSpectacle.proprietes().unwrap().code("S21.G00.40.005"), None);
    }
}
