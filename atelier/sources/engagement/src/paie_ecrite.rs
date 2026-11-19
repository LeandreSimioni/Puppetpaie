//! LA PAIE ECRITE : ce que la validation d'un engagement ecrit de sa paie d'un mois (`etats::calculer_les_rangs`), dans le
//! fichier de ce mois (`paies`), et ce que ses envois y posent ensuite (le numero du bulletin, le premier N CE, le
//! bulletin qu'elle rectifie, pourquoi elle s'est recalculee). Une seule forme, lue avec le fichier des paies (`paies`) et ecrite
//! par serde ; une cle inconnue refuse en se nommant.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use socle::date::Jour;
use socle::mois::Mois;
use socle::grandeurs::{Heures, Montant, MontantUnitaire};

use crate::calcul::{BaseDeCotisation, LigneBrut, LigneCotisation};
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
    pub lignes: Vec<LigneCotisation>,
    #[serde(with = "nombre")]
    pub net_a_payer: Montant,
    #[serde(with = "nombre")]
    pub net_imposable: Montant,
    #[serde(with = "nombre")]
    pub net_social: Montant,
    pub numero_bulletin: Option<crate::numeros::NumeroDeBulletin>,
    pub reduction_generale: Option<droit::reduction::ReductionGenerale>,
    pub remuneration: Vec<LigneBrut>,
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
    /// La part du brut que l'abattement de la CSG a abattue (`calcul::Contenu::part_abattue_csg`), toujours ecrite par le
    /// calcul ; absente, la paie a abattu tout son brut soumis. Elle se lit par `part_abattue_csg()`, nulle part ailleurs.
    #[serde(default, with = "nombre::option")]
    pub part_abattue_csg: Option<Montant>,
    /// Un mois decoupe : ses periodes elementaires, chacune une paie. Vide pour un mois d'un seul tenant.
    pub periodes: Vec<FormeDePaie<A>>,
    /// Le plafond d'un salarie a temps partiel tel que le calcul l'a applique (`calcul::PlafondTempsPartiel`) : la forme
    /// ecrite seule le porte.
    pub plafond_temps_partiel: Option<crate::calcul::PlafondTempsPartiel>,
}

impl<A> FormeDePaie<A> {
    /// LA PART DU BRUT QUE L'ABATTEMENT DE LA CSG A ABATTUE DANS CETTE PAIE, telle qu'elle l'ecrit. Une paie qui ne
    /// l'ecrit pas a abattu de 1,75 % chacune de ses rubriques soumises, sans limite : sa part abattue est son brut
    /// soumis (`brut`, la somme de ces rubriques). Ce qui le fonde est dans ses chiffres : sa base de CSG (`base_csg`)
    /// est ce brut abattu, plus la prevoyance patronale (CSS L. 136-1-1, `calcul`, 7.4). Ce n'est pas un repli : c'est
    /// le fait de cette paie, et le seul endroit ou il se dit.
    pub fn part_abattue_csg(&self) -> Montant {
        match self.part_abattue_csg {
            Some(m) => m,
            None => self.brut,
        }
    }
}

/// Une fiche nommee par sa cle (le salarie, le spectacle d'une paie).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cle<I> {
    pub cle: I,
}

/// Les lignes de cotisation d'une paie ecrite, lues sa base en nombre puis relues (`calcul::LigneCotisation::relue`).
fn lignes_relues<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<LigneCotisation>, D::Error> {
    <Vec<LigneCotisation<socle::forme::Nombre>> as Deserialize>::deserialize(d).map(|l| l.into_iter().map(LigneCotisation::relue).collect())
}

/// La base d'une ligne de cotisation, ecrite sans son unite (`BaseDeCotisation::en_nombre`).
pub(crate) fn base_en_nombre<B: Copy + Into<BaseDeCotisation>, S: serde::Serializer>(b: &B, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&(*b).into().en_nombre())
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
    pub emploi: droit::emplois::EmploiEngagement,
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
    pub quotite: droit::natures::QuotiteDuContrat,
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

