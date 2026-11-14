//! LA PAIE ECRITE : ce que la validation d'un engagement ecrit de sa paie d'un mois (`etats::calculer_les_rangs`), dans le
//! fichier de ce mois (`paies`), et ce que ses envois y posent ensuite (le numero du bulletin, le premier N CE, le
//! bulletin qu'elle rectifie, pourquoi elle s'est recalculee). Une seule forme, lue avec le fichier des paies (`paies`) et ecrite
//! par serde ; une cle inconnue refuse en se nommant.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use socle::date::Jour;
use socle::mois::Mois;
use socle::grandeurs::{Heures, Montant, MontantUnitaire, Taux};

use crate::calcul::BaseDeCotisation;
use crate::{Avis, ContratEcrit, LignePaie};

/// UNE PAIE ECRITE.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaieEcrite {
    /// La paie, sous sa forme ecrite (`relire::ecrire`).
    pub paie: FormeDePaie<Avis>,
    /// Les fiches du salarie telles que le calcul les a lues.
    pub fiches: crate::relire::FichesDuCalcul,
    /// Ce que la saisie du mois posait pour tous ses engagements.
    pub saisie_du_mois: crate::saisie::CeQueLeMoisDit,
    /// Ce dont le calcul dependait (`empreinte`).
    pub empreinte: Empreinte,
    /// Le prelevement a la source (`etats::prelevement`).
    pub pas: Option<droit::pas::Versement>,
    /// Le compteur de conges d'un permanent a la fin du mois, et ce qu'il tient pour le mois suivant.
    pub conges: Option<crate::conges::CompteurConges>,
    pub conges_avant: Option<crate::conges::Avant>,
    /// Le numero du bulletin et le premier N CE, ecrits quand ses documents sont partis (`etats::poser_numeros_des_documents`).
    pub bulletin: Option<crate::numeros::NumeroDeBulletin>,
    pub ce: Option<i64>,
    /// Le numero du bulletin d'une paie partie que celle-ci rectifie.
    pub rectifie: Option<crate::numeros::NumeroDeBulletin>,
    /// Pourquoi la paie s'est recalculee d'elle-meme, et quand.
    pub recalcul: Option<Recalcul>,
}

/// CE DONT LE CALCUL D'UNE PAIE DEPENDAIT : l'empreinte de chaque chose lue, les regles datees et les valeurs datees de la
/// compagnie (`empreinte`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empreinte {
    /// Chaque lecture notee et ce qu'elle valait, ecrites par leur texte (`socle::fichiers::lectures`).
    #[serde(with = "socle::fichiers::lectures")]
    pub entrees: BTreeMap<droit::contexte::CleLue, socle::fichiers::ValeurLue>,
    pub regles: Vec<Jour>,
    pub configuration: Vec<Jour>,
}

/// Une paie recalculee d'elle-meme : quand, et pourquoi (`etats::raison`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recalcul {
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub le: socle::paris::Moment,
    pub raison: String,
}

impl PaieEcrite {
    /// Le compteur de conges sans ses avertissements, pour comparer deux paies sur leurs chiffres.
    pub fn conges_sans_avis(&self) -> Option<crate::conges::CompteurConges> {
        self.conges.clone().map(|c| crate::conges::CompteurConges { avertissements: Vec::new(), ..c })
    }
}

/// UNE PAIE SOUS SA FORME ECRITE, champ par champ, dans l'ordre ou elle s'ecrit ; les montants en texte normalise
/// (`socle::num::nombre`). C'est sa forme canonique (`calcul::Paie::canon`, pour les empreintes et les oracles), un avis
/// `A` par son seul texte, et sa forme ecrite (`relire::ecrire`), un avis `{texte, info}`, avec les lignes de
/// l'engagement telles quelles et le plafond d'un temps partiel ; `relire` la relit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = "A: Deserialize<'de>"))]
#[serde(deny_unknown_fields)]
pub struct FormeDePaie<A> {
    #[serde(with = "nombre")]
    pub assiette_chomage: Montant,
    pub avertissements: Vec<A>,
    #[serde(with = "nombre")]
    pub base_csg: Montant,
    #[serde(with = "bases")]
    pub bases: BTreeMap<droit::assiette::Assiette, BaseDeCotisation>,
    #[serde(with = "nombre")]
    pub brut: Montant,
    #[serde(with = "nombre")]
    pub cout_employeur: Montant,
    pub deduction_forfaitaire: Option<FormeDeLaDeduction>,
    pub engagement: FormeDEngagement,
    #[serde(with = "nombre")]
    pub heures: Heures,
    #[serde(with = "nombre")]
    pub heures_absence: Heures,
    #[serde(with = "nombre")]
    pub heures_travaillees: Heures,
    pub lignes: Vec<FormeDeCotisation>,
    #[serde(with = "nombre")]
    pub net_a_payer: Montant,
    #[serde(with = "nombre")]
    pub net_imposable: Montant,
    #[serde(with = "nombre")]
    pub net_social: Montant,
    pub numero_bulletin: Option<crate::numeros::NumeroDeBulletin>,
    pub reduction_generale: Option<droit::reduction::ReductionGenerale>,
    pub remuneration: Vec<FormeDeRemuneration>,
    #[serde(with = "nombre")]
    pub salaire_de_base: Montant,
    #[serde(with = "nombre")]
    pub salaire_retabli: Montant,
    pub salarie: Cle<droit::contexte::IdSalarie>,
    /// Le SMIC horaire de la periode.
    #[serde(with = "nombre")]
    pub smic: MontantUnitaire,
    pub spectacle: Option<Cle<droit::contexte::IdSpectacle>>,
    #[serde(with = "nombre")]
    pub total_pat: Montant,
    #[serde(with = "nombre")]
    pub total_sal: Montant,
    #[serde(with = "nombre")]
    pub plafond: Montant,
    #[serde(with = "nombre")]
    pub plafond_urssaf: Montant,
    /// Un mois decoupe : ses periodes elementaires, chacune une paie. Vide pour un mois d'un seul tenant.
    pub periodes: Vec<FormeDePaie<A>>,
    /// Le plafond d'un salarie a temps partiel tel que le calcul l'a applique (`calcul::PlafondTempsPartiel`) : la forme
    /// ecrite seule le porte.
    pub plafond_temps_partiel: Option<crate::calcul::PlafondTempsPartiel>,
}

/// Une fiche nommee par sa cle (le salarie, le spectacle d'une paie).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cle<I> {
    pub cle: I,
}

/// Une ligne de cotisation (`calcul::LigneCotisation`) sous sa forme ecrite. Sa base s'ecrit en nombre, des euros ou des
/// heures : relue, elle prend la grandeur que sa cotisation dit (`BaseDeCotisation::relue`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormeDeCotisation {
    /// La cotisation de la ligne.
    pub id: droit::forme_bareme::IdDeCotisation,
    /// Sa famille.
    pub famille: droit::forme_bareme::FamilleDeCotisation,
    pub libelle: String,
    #[serde(serialize_with = "nombre_ecrit")]
    pub base: socle::forme::Nombre,
    #[serde(with = "nombre::option")]
    pub taux_pat: Option<Taux>,
    #[serde(with = "nombre")]
    pub montant_pat: Montant,
    #[serde(with = "nombre::option")]
    pub taux_sal: Option<Taux>,
    #[serde(with = "nombre")]
    pub montant_sal: Montant,
    pub masquer_base_et_taux: bool,
}

/// Un nombre ecrit sans son unite (sa ligne la dit a la relecture), en texte normalise (`socle::num::nombre`) ; il se
/// relit d'un nombre ou d'un texte (`socle::forme::Nombre`).
fn nombre_ecrit<S: serde::Serializer>(n: &socle::forme::Nombre, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&socle::num::nombre(n.0))
}

/// Une ligne de remuneration (`calcul::LigneBrut`) sous sa forme ecrite, avec sa designation au bulletin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormeDeRemuneration {
    pub code: droit::catalogue::CodeDeRubrique,
    pub libelle: String,
    pub unite: droit::catalogue::Unite,
    /// Sa quantite, sans son unite : relue, elle la prend de `unite` (`calcul::LigneBrut::de_la_forme`).
    #[serde(serialize_with = "nombre_ecrit")]
    pub quantite: socle::forme::Nombre,
    /// Son taux, sans son unite : relu, il la prend de `unite` (`ligne::TauxDeLigne::de_l_unite`).
    #[serde(serialize_with = "nombre_ecrit")]
    pub taux: socle::forme::Nombre,
    #[serde(with = "nombre")]
    pub montant: Montant,
    #[serde(with = "nombre")]
    pub heures_annexes: Heures,
    pub soumis: bool,
    pub imposable: bool,
    pub conges_spectacles: bool,
    pub salaire_de_base: bool,
    pub retabli: bool,
    pub conges_pris: bool,
    pub dsn_bloc52: Option<droit::type_de_prime::TypeDePrime>,
    pub arret_de_travail: bool,
    pub dsn_autres_bruts: Option<droit::autre_revenu_brut::AutreElementDeRevenuBrut>,
    pub precision: Option<String>,
    pub designation: String,
}

/// La deduction forfaitaire specifique (`calcul::DeductionForfaitaire`) sous sa forme ecrite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormeDeLaDeduction {
    pub categorie: droit::forme_bareme::CategorieDfs,
    pub profession: droit::fiche::ProfessionDfs,
    #[serde(with = "nombre")]
    pub taux: Taux,
    pub rubriques: Vec<droit::forme_bareme::IdDeCotisation>,
    #[serde(with = "nombre")]
    pub montant: Montant,
    #[serde(with = "nombre")]
    pub deja: Montant,
}

/// UN ENGAGEMENT SOUS SA FORME CANONIQUE (`Engagement::canon`) : ses champs dans l'ordre ou ils s'ecrivent, les nombres
/// en texte normalise, puis ce qui s'en deduit (sa periode, sa quotite, ses mois).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormeDEngagement {
    pub salarie: droit::contexte::IdSalarie,
    pub spectacle: Option<droit::contexte::IdSpectacle>,
    pub dates_travail: Vec<Jour>,
    pub lignes: Vec<LignePaie>,
    pub debut: Jour,
    pub fin: Option<Jour>,
    pub numero: Option<crate::numeros::NumeroContrat>,
    pub mois: Mois,
    pub planning: Vec<Jour>,
    pub planning_lieux: BTreeMap<Jour, String>,
    #[serde(with = "entier")]
    pub jours: i64,
    #[serde(with = "entier")]
    pub jours_contrat: i64,
    pub date_paiement: Option<Jour>,
    #[serde(with = "nombre::option")]
    pub taux_cachet: Option<MontantUnitaire>,
    #[serde(with = "entier")]
    pub cachets: i64,
    #[serde(with = "nombre::option")]
    pub heures: Option<Heures>,
    pub lieu: String,
    pub lieux: BTreeMap<Jour, String>,
    pub emploi: Option<String>,
    pub emploi_affichage: Option<String>,
    pub pcs_ese: Option<droit::fiche::CodePcsEse>,
    pub code_emploi_ft: Option<droit::fiche::CodeEmploiFt>,
    pub statut: Option<droit::fiche::Statut>,
    pub statut_conventionnel: Option<droit::fiche::StatutConventionnel>,
    pub statut_categoriel_rc: Option<droit::fiche::StatutCategorielRc>,
    pub etablissement: Option<droit::fiche::Nic>,
    pub nature: droit::natures::NatureDeContrat,
    #[serde(with = "nombre::option")]
    pub quotite_contrat: Option<Heures>,
    #[serde(with = "nombre::option")]
    pub quotite_reference_saisie: Option<Heures>,
    pub modalite_temps_travail_saisie: Option<droit::natures::ModaliteDuTempsDeTravail>,
    pub motif_recours: Option<droit::natures::MotifDeRecours>,
    pub motif_rupture: Option<droit::natures::MotifDeRupture>,
    pub notification_rupture: Option<Jour>,
    pub preavis: Option<crate::saisie::Preavis>,
    pub transaction: Option<droit::preavis::TransactionEnCours>,
    pub suspensions: Vec<crate::imprevus::Suspension>,
    pub contrat_ecrit: Option<ContratEcrit>,
    pub debut_periode: Jour,
    pub fin_periode: Jour,
    #[serde(with = "nombre")]
    pub quotite: Heures,
    pub sans_paie: bool,
    pub premier_mois: bool,
    pub dernier_mois: bool,
    pub mois_du_contrat: Vec<Mois>,
}

/// Une grandeur en texte normalise (`socle::num::nombre`), relue d'un nombre ou d'un texte.
use socle::grandeurs::ecrit::nombre;

/// Les assiettes d'une paie, chacune en nombre (`socle::num::nombre`) ; relue, sa valeur est ce que l'assiette compte
/// (`BaseDeCotisation::de_l_assiette`).
mod bases {
    use crate::calcul::BaseDeCotisation;
    use droit::assiette::Assiette;
    use std::collections::BTreeMap;

    pub fn serialize<S: serde::Serializer>(t: &BTreeMap<Assiette, BaseDeCotisation>, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(t.iter().map(|(a, b)| (a, socle::num::nombre(b.decimal()))))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<Assiette, BaseDeCotisation>, D::Error> {
        <BTreeMap<Assiette, socle::forme::Nombre> as serde::Deserialize>::deserialize(d).map(|t| t.into_iter().map(|(a, n)| (a, BaseDeCotisation::de_l_assiette(a, n.0))).collect())
    }
}

/// Un entier en texte.
pub(crate) mod entier {
    use serde::Deserialize;
    pub fn serialize<S: serde::Serializer>(x: &i64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&x.to_string())
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
        let t = String::deserialize(d)?;
        t.trim().parse().map_err(|_| serde::de::Error::custom(format!("{} n'est pas un entier", socle::ecriture::json_en_ligne(&t))))
    }
}

/// Un decimal en texte tel qu'il est (`Decimal::to_string`, son echelle gardee), relu exactement.
pub(crate) mod decimal_texte {
    use serde::Deserialize;
    use socle::num::Decimal;
    pub fn serialize<S: serde::Serializer>(x: &Decimal, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&x.to_string())
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Decimal, D::Error> {
        let t = String::deserialize(d)?;
        t.parse().map_err(|_| serde::de::Error::custom(format!("{} n'est pas un nombre", socle::ecriture::json_en_ligne(&t))))
    }
}
