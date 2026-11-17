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
    pub ce: Option<crate::numeros::NumeroDeCertificat>,
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
    pub deduction_forfaitaire: Option<crate::calcul::DeductionForfaitaire>,
    pub engagement: FormeDEngagement,
    #[serde(with = "nombre")]
    pub heures: Heures,
    #[serde(with = "nombre")]
    pub heures_absence: Heures,
    #[serde(with = "nombre")]
    pub heures_travaillees: Heures,
    #[serde(deserialize_with = "lignes_relues")]
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

/// Une ligne de cotisation (`calcul::LigneCotisation`) sous sa forme ecrite, declaree une fois. Sa base `B` s'ecrit en
/// nombre, des euros ou des heures, sans son unite (`BaseDeCotisation::en_nombre`) : la forme se lit sa base en nombre
/// (`FormeDeCotisation<socle::forme::Nombre>`, la seule qui se lit), que `relue` lit aussitot dans la grandeur que sa
/// cotisation compte (`BaseDeCotisation::relue`) ; elle ne circule que sa base typee (`FormeDeCotisation`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(serialize = "B: Copy + Into<BaseDeCotisation>", deserialize = "B: Deserialize<'de> + Into<socle::forme::Nombre>"))]
pub struct FormeDeCotisation<B = BaseDeCotisation> {
    /// La cotisation de la ligne.
    pub id: droit::forme_bareme::IdDeCotisation,
    /// Sa famille.
    pub famille: droit::forme_bareme::FamilleDeCotisation,
    pub libelle: String,
    #[serde(serialize_with = "base_en_nombre")]
    pub base: B,
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

impl FormeDeCotisation<socle::forme::Nombre> {
    /// La ligne lue, sa base dans la grandeur que sa cotisation compte (`BaseDeCotisation::relue`) : la porte de sa
    /// lecture.
    fn relue(self) -> FormeDeCotisation {
        FormeDeCotisation {
            base: BaseDeCotisation::relue(&self.id, self.base),
            id: self.id,
            famille: self.famille,
            libelle: self.libelle,
            taux_pat: self.taux_pat,
            montant_pat: self.montant_pat,
            taux_sal: self.taux_sal,
            montant_sal: self.montant_sal,
            masquer_base_et_taux: self.masquer_base_et_taux,
        }
    }
}

/// Les lignes de cotisation d'une paie ecrite, lues sa base en nombre puis relues (`FormeDeCotisation::relue`).
fn lignes_relues<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<FormeDeCotisation>, D::Error> {
    <Vec<FormeDeCotisation<socle::forme::Nombre>> as Deserialize>::deserialize(d).map(|l| l.into_iter().map(FormeDeCotisation::relue).collect())
}

/// La base d'une ligne, ecrite sans son unite (`BaseDeCotisation::en_nombre`).
fn base_en_nombre<B: Copy + Into<BaseDeCotisation>, S: serde::Serializer>(b: &B, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&(*b).into().en_nombre())
}

/// Une ligne de remuneration (`calcul::LigneBrut`) sous sa forme ecrite, avec sa designation au bulletin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormeDeRemuneration {
    pub code: droit::catalogue::CodeDeRubrique,
    pub libelle: String,
    pub unite: droit::catalogue::Unite,
    /// Ce qu'elle paie, sa quantite et son taux dans l'unite de sa rubrique, par sa forme (`{"cachets": ["2", "150"]}`),
    /// sans zero inutile (`calcul::LigneBrut::forme`) ; `null` : celle d'un bulletin importe qui ne le dit pas.
    pub paiement: Option<crate::ligne::PaiementDeLigne>,
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

/// UN ENGAGEMENT SOUS SA FORME CANONIQUE (`Engagement::canon`) : ses champs dans l'ordre ou ils s'ecrivent, les nombres
/// en texte normalise, puis ce qui s'en deduit (sa periode, sa quotite, ses mois).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormeDEngagement {
    pub salarie: droit::contexte::IdSalarie,
    pub spectacle: Option<droit::contexte::IdSpectacle>,
    pub dates_travail: Vec<Jour>,
    pub lignes: Vec<LignePaie>,
    pub contrat: socle::date::PeriodeOuverte,
    pub numero: Option<crate::numeros::NumeroContrat>,
    pub mois: Mois,
    pub planning: Vec<Jour>,
    pub planning_lieux: BTreeMap<Jour, String>,
    #[serde(with = "nombre")]
    pub jours: socle::grandeurs::Jours,
    #[serde(with = "nombre")]
    pub jours_contrat: socle::grandeurs::Jours,
    pub date_paiement: Option<Jour>,
    #[serde(with = "nombre::option")]
    pub taux_cachet: Option<MontantUnitaire>,
    #[serde(with = "nombre")]
    pub cachets: socle::grandeurs::Cachets,
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
    /// Sa periode de paie (`Engagement::periode_payee`).
    pub periode: socle::date::Periode,
    /// Sa quotite (`Engagement::quotite`), dans son unite.
    pub quotite: crate::QuotiteDuContrat,
    pub sans_paie: bool,
    pub place: crate::PlaceDuMois,
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
        s.collect_map(t.iter().map(|(a, b)| (a, b.en_nombre())))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<Assiette, BaseDeCotisation>, D::Error> {
        <BTreeMap<Assiette, socle::forme::Nombre> as serde::Deserialize>::deserialize(d).map(|t| t.into_iter().map(|(a, n)| (a, BaseDeCotisation::de_l_assiette(a, n))).collect())
    }
}

