//! La paie d'un mois decoupee en periodes elementaires : un permanent dont le statut, la categorie ou la quotite
//! change en cours de mois, ou dont un avenant prend effet en cours de mois.
//!
//! Textes (`atelier/reference/`) :
//! - norme DSN 2026.1, § 2.1.3.2, p. 28 (`norme-dsn/texte/chapitre-2-principes-de-constitution.txt`, l. 500-511) : « La
//!   periode elementaire de paie constitue une sous-partie de la periode de paie. Le decoupage [...] decoule de
//!   modalites differentes de calcul de la paie en cas d'evolution des parametres de calcul de la paie et notamment du
//!   statut du salarie (par exemple, passage de non cadre a cadre en cours de periode de paie) » ; « La periode a
//!   declarer dans le bloc Remuneration est cette periode elementaire de paie, lorsque le logiciel de paie produit
//!   effectivement un decoupage » ; § 2.1.3.3 et 2.1.3.5, p. 29 : les elements du bloc 51 et les mesures d'activite
//!   (53) « par periode elementaire de paie » ;
//! - FAQ Agirc-Arrco « Changement de situation en cours de mois », v2 (`sources/agirc-arrco/`) : « le salarie concerne
//!   par ce changement doit faire l'objet d'un decoupage de ses bases assujetties declarees en DSN » ; « Le salaire brut
//!   et le plafond de securite sociale doivent etre proratises selon la duree de chacune des periodes
//!   infra-mensuelles » (exemple 1 : non cadre -> cadre au 10 avril, 9/30 et 21/30) ; pour un passage a temps partiel,
//!   le plafond et le salaire de la periode « au prorata temporis de la duree du travail » (exemple 2 : 15/30 x
//!   70/151,67) ; les bases 02 et 03 datees de chaque periode (78.002, 78.003) ; cahier Agirc-Arrco CT2026, § 6.1 ;
//! - fiche net-entreprises 1265 (`sources/net-entreprises-base-connaissances/`, l. 52) : « Pour tout changement
//!   intervenant en cours de mois, il convient de ventiler les bases assujetties par periodes afin de permettre un
//!   calcul correspondant a la situation exacte du salarie » (tous les organismes, Urssaf compris) ;
//! - CSS R. 242-2, I (`sources/code-de-la-securite-sociale/articles-20261010.md`) : le plafond « reduit[s] a due
//!   proportion du nombre de jours de la periode pendant laquelle les personnes sont employees », et pour un temps
//!   partiel « a due proportion de la duree de travail inscrite a leur contrat de travail au titre de la periode ou ils
//!   sont presents » ; BOSS, assiette generale, § 730 (`sources/boss/`) : « les differentes regles d'ajustement du
//!   plafond sont cumulables ».
//!
//! Le droit dit ou la paie se coupe (`decouper`, `jour_de_l_avenant`) et quelle part du mois chaque periode porte
//! (`part_du_mois`) ; l'engagement calcule chaque periode et les assemble (`engagement::periodes`).

use socle::date::{Jour, Periode};
use socle::mois::Mois;

use crate::conges::JoursTravailles;
use socle::echec::R;
use socle::grandeurs::ecrit::sans_zero::dit;
use socle::grandeurs::{Coefficient, HeuresParUnite, Jours};
use socle::num::Decimal;

/// Les jours ouvres d'un mois de la CCN 1285, art. X.3 : « 1/21 du montant du salaire mensuel »
/// (`atelier/reference/sources/kali-1285-x3-remuneration-artistes-20260929.md`, l. 20).
const JOURS_DU_MOIS_CCNEAC: Jours = Jours::du_decimal(Decimal::from_parts(21, 0, 0, false, 0));

/// Ce que `avant` (les termes du contrat avant un avenant en cours de mois) peut dire.
pub const TERMES_D_AVANT: [&str; 2] = ["salaire_mensuel", "quotite_contrat"];

/// Les termes d'avant un avenant qu'un contrat dit (`TERMES_D_AVANT`, dans cet ordre) : chacun dit ou non.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermesDAvant {
    pub salaire_mensuel: bool,
    pub quotite_contrat: bool,
}

/// Le jour ou un avenant decoupe la paie de la periode `payee` : sa `date_modification`, quand le contrat dit
/// les termes d'avant (`avant`) et qu'elle tombe apres le premier jour de la periode payee. Sans `avant`, rien ; un
/// `avant` incomplet, sans date, ou qui ne decoupe rien refuse. `qui` nomme le contrat dans un refus.
pub fn jour_de_l_avenant(avant: Option<TermesDAvant>, date_modification: Option<Jour>, payee: &Periode, qui: &str) -> R<Option<Jour>> {
    let Some(avant) = avant else { return Ok(None) };
    let dits = [avant.salaire_mensuel, avant.quotite_contrat];
    if let Some((k, _)) = TERMES_D_AVANT.iter().zip(dits).find(|(_, dit)| !dit) {
        return Err(socle::echec::cle(
            format!("{} : l'avenant ne dit pas tous les termes d'avant ; la période d'avant se paie sur le salaire et la durée du travail d'alors, les deux.", qui),
            &format!("avant.{}", k),
        ));
    }
    let Some(j) = date_modification else {
        return Err(socle::echec::cle(format!("{} : l'avenant ne dit pas quand il prend effet : remplir « À partir de » dans « Modifier l'engagement ».", qui), "date_modification"));
    };
    if j <= payee.debut() || !payee.contient(&j) {
        return Err(socle::echec::cle(
            format!(
                "{} : l'avenant prend effet le {}, hors de la période payée du {} au {} : il ne la coupe pas ; un avenant au premier jour de la période se paie sur les termes de la saisie, tout le mois.",
                qui,
                j.jj_mm_aaaa(),
                payee.debut().jj_mm_aaaa(),
                payee.fin().jj_mm_aaaa()
            ),
            "avant",
        ));
    }
    Ok(Some(j))
}

/// Les periodes elementaires de `p` coupee aux `jours` ou quelque chose change (chacun est le premier jour
/// d'une periode) ; un jour hors de la periode ne coupe rien.
pub fn decouper(p: Periode, mut jours: Vec<Jour>) -> R<Vec<Periode>> {
    jours.retain(|j| *j > p.debut() && p.contient(j));
    jours.sort();
    jours.dedup();
    let mut out = Vec::new();
    let mut d = p.debut();
    for j in jours {
        out.push(Periode::du_au(d, j.plus_jours(-1)?)?);
        d = j;
    }
    out.push(Periode::du_au(d, p.fin())?);
    Ok(out)
}

/// La regle qui paie un mois incomplet d'un contrat paye au mois (`part_du_mois`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegleDuMoisIncomplet {
    /// Le salaire mensuel au quotient des heures du mois dans l'entreprise, par les heures de la periode.
    HoraireReel,
    /// CCN 1285, art. X.3 : 1/21 du salaire mensuel par jour ouvre du contrat (artiste, premier ou dernier mois).
    Ccneac1285Artiste,
}

/// La regle d'un mois incomplet : celle de la convention qui en a une, sinon l'horaire reel. La CCNEAC (IDCC 1285),
/// art. X.3 (`atelier/reference/sources/kali-1285-x3-remuneration-artistes-20260929.md`, l. 20), pour les emplois de la
/// filiere artistique : « Si le premier et/ou le dernier mois civil est incomplet, le salaire de celui-ci est calcule en
/// multipliant 1/21 du montant du salaire mensuel inscrit au contrat [...] par le nombre de jours ouvres figurant au
/// contrat ». `contrat_incomplet` : le contrat commence apres le premier jour du mois ou finit avant le dernier (un
/// avenant qui coupe un mois entier n'en fait pas un premier ni un dernier mois).
pub fn regle_du_mois_incomplet(idcc: Option<&crate::fiche::Idcc>, statut: crate::fiche::Statut, contrat_incomplet: bool) -> RegleDuMoisIncomplet {
    if idcc == Some(&crate::clauses::IDCC_CCNEAC) && statut == crate::fiche::Statut::Artiste && contrat_incomplet {
        RegleDuMoisIncomplet::Ccneac1285Artiste
    } else {
        RegleDuMoisIncomplet::HoraireReel
    }
}

/// L'horaire d'un salarie paye au mois, pour un mois : ses jours travailles (`conges::JoursTravailles`, sa fiche), et
/// les feries chomes du mois qu'il ne voit pas payes (`conges::feries_non_payes`, L3133-3).
#[derive(Debug, Clone, PartialEq)]
pub struct Horaire {
    pub jours: JoursTravailles,
    pub feries_non_payes: Vec<(Jour, String)>,
}

/// LE DECOMPTE DE LA PART DU MOIS qu'une periode payee porte de son salaire mensuel (`part_du_mois`) : la part, et ce
/// qui la compte (ses jours, sa regle, ses feries non payes), que le bulletin dit. La part seule est un `Coefficient`,
/// le nombre qu'une ligne de paie garde (`engagement::ligne::QuantiteDeLigne::PartDuMois`).
#[derive(Debug, Clone, PartialEq)]
pub struct DecompteDuMois {
    /// La fraction du salaire mensuel payee : 1 pour un mois entier.
    pub fraction: Coefficient,
    /// Les jours travailles payes de la periode (`JoursTravailles::jours_entre`, moins les feries non payes).
    pub jours: Jours,
    /// Ceux du mois entier (horaire reel), ou 21 (CCN 1285).
    pub jours_du_mois: Jours,
    pub regle: RegleDuMoisIncomplet,
    /// Les jours de la semaine que l'horaire compte.
    pub jours_travailles: JoursTravailles,
    /// Les feries chomes de la periode que le salarie ne voit pas payes (L3133-3).
    pub feries_non_payes: Vec<(Jour, String)>,
}

impl DecompteDuMois {
    /// Ce que la ligne du bulletin dit de la part payee, pour un contrat de `heures_par_jour` heures par jour travaille ;
    /// `None` pour un mois entier.
    pub fn sur_le_bulletin(&self, heures_par_jour: HeuresParUnite) -> Option<String> {
        if self.fraction.decimal() == Decimal::ONE {
            return None;
        }
        let part = match self.regle {
            RegleDuMoisIncomplet::HoraireReel => format!(
                "{} h sur les {} h du mois",
                heures_en_mots(heures_par_jour, self.jours),
                heures_en_mots(heures_par_jour, self.jours_du_mois)
            ),
            RegleDuMoisIncomplet::Ccneac1285Artiste => format!("{} jour(s) ouvré(s) à 1/21 du mois", dit(self.jours)),
        };
        Some(match self.feries_non_payes.as_slice() {
            [] => part,
            f => format!("{}, férié du {} non payé", part, f.iter().map(|(j, _)| j.jj_mm_aaaa()).collect::<Vec<_>>().join(", ")),
        })
    }

    /// La methode, en mots, et son texte : ce que l'apercu du bulletin dit d'un mois paye en partie.
    pub fn methode(&self, heures_par_jour: HeuresParUnite) -> String {
        let mut dit = match self.regle {
            RegleDuMoisIncomplet::HoraireReel => format!(
                "{} h de travail sur les {} h que l'horaire compte dans le mois ({} jours travaillés de {} h, {}, jours \
fériés compris), payées au salaire mensuel divisé par les heures du mois (Cass. soc., 20/01/1999, n° 96-45.042 : « la \
rémunération horaire » multipliée par « le nombre d'heures de travail réellement effectuées »)",
                heures_en_mots(heures_par_jour, self.jours),
                heures_en_mots(heures_par_jour, self.jours_du_mois),
                dit(self.jours_du_mois),
                socle::texte::virgule(heures_par_jour),
                self.jours_travailles.en_mots()
            ),
            RegleDuMoisIncomplet::Ccneac1285Artiste => format!(
                "{} jour(s) ouvré(s) du contrat dans le mois ({}), payés chacun 1/21 du salaire mensuel (CCN 1285, \
art. X.3 : le premier ou le dernier mois incomplet d'un artiste)",
                dit(self.jours),
                self.jours_travailles.en_mots()
            ),
        };
        if self.regle == RegleDuMoisIncomplet::Ccneac1285Artiste && self.fraction.decimal() > Decimal::ONE {
            dit.push_str(
                ". C'est plus que le salaire mensuel : l'article X.3 ne borne pas le mois incomplet, le logiciel non plus",
            );
        }
        if !self.feries_non_payes.is_empty() {
            dit.push_str(&format!(
                ". Le férié chômé du {} n'est pas payé : moins de trois mois d'ancienneté ce jour-là (L. 3133-3 ; le 1er \
mai l'est toujours, L. 3133-5)",
                dates_en_mots(&self.feries_non_payes)
            ));
        }
        dit
    }
}

/// Des feries, en mots : « 06/04/2026 (Lundi de Pâques) et 14/05/2026 (Ascension) ».
fn dates_en_mots(feries: &[(Jour, String)]) -> String {
    let mots: Vec<String> = feries.iter().map(|(j, nom)| format!("{} ({})", j.jj_mm_aaaa(), nom)).collect();
    match mots.split_last() {
        Some((dernier, tete)) if !tete.is_empty() => format!("{} et {}", tete.join(", "), dernier),
        _ => mots.join(""),
    }
}

/// `jours` jours de `heures_par_jour` heures, au centieme, ecrits a la francaise.
fn heures_en_mots(heures_par_jour: HeuresParUnite, jours: Jours) -> String {
    socle::texte::virgule((jours * heures_par_jour).au_centieme())
}

/// LA PART DU SALAIRE MENSUEL que porte la periode payee [`d`, `f`] du mois qui finit le `fin_mois`.
///
/// L'HORAIRE REEL : « la retenue par heure d'absence d'un salarie paye au mois doit etre en principe egale au quotient du
/// salaire mensuel par le nombre d'heures de travail dans l'entreprise pour le mois considere », et le salaire d'un mois
/// incomplet se determine « en multipliant la remuneration horaire par le nombre d'heures de travail reellement
/// effectuees » : un salaire au 25/30e est casse (Cass. soc., 20/01/1999, n° 96-45.042, `atelier/reference/sources/
/// mois-incomplet/legifrance-cass-soc-19990120-96-45042-20261030.md` ; meme quotient : Cass. soc., 11/02/1982,
/// n° 80-40.359, et 14/05/1987, n° 84-43.854, une embauche au 9 fevrier payee 117 h sur les 156 h du mois,
/// `../suspension-non-remuneree/`). Les heures du mois dans l'entreprise et celles de la periode sont les jours
/// travailles du salarie (`conges::JoursTravailles::jours_entre` : sa fiche, feries compris, fiche DSN 2693, exemple 4)
/// fois les heures d'un jour, les memes pour les deux : la part est le quotient des jours travailles. Un ferie chome que
/// le salarie ne voit pas paye (`Horaire::feries_non_payes`, L3133-3) reste une heure du mois et n'est pas une heure
/// payee de la periode.
///
/// LA CCN 1285, pour un artiste (`regle_du_mois_incomplet`) : les jours travailles de la periode, a 1/21 chacun.
/// INFERENCE NOMMEE : « les jours ouvres figurant au contrat » sont les jours que l'horaire du salarie programme, feries
/// compris. L'article ne borne pas le resultat au salaire mensuel (un mois de 23 jours ouvres dont un manque en donne
/// 22/21) : le logiciel ne le borne pas, et le dit (`DecompteDuMois::methode`).
///
/// Une periode qui couvre le mois entier, sans ferie non paye, en porte tout le salaire. Le plafond de la securite
/// sociale, lui, se proratise aux jours calendaires (CSS R. 242-2, `plafond::prorata_calendaire`) : ce n'est pas le
/// salaire.
pub fn part_du_mois(periode: &Periode, mois: Mois, regle: RegleDuMoisIncomplet, horaire: &Horaire) -> R<DecompteDuMois> {
    let le_mois = Periode::du_mois(mois);
    let feries_non_payes: Vec<(Jour, String)> =
        horaire.feries_non_payes.iter().filter(|(j, _)| periode.contient(j) && horaire.jours.travaille(j)).cloned().collect();
    let jours = horaire.jours.jours_entre(periode) - Jours::de_dates(feries_non_payes.len());
    let du_mois = horaire.jours.jours_entre(&le_mois);
    let entier = periode.croise(&le_mois) == Some(le_mois) && feries_non_payes.is_empty();
    let jours_du_mois = match regle {
        RegleDuMoisIncomplet::Ccneac1285Artiste if !entier => JOURS_DU_MOIS_CCNEAC,
        _ => du_mois,
    };
    let fraction = match entier {
        true => Coefficient::UN,
        false => jours.rapport(jours_du_mois).ok_or_else(|| socle::refus!("L'horaire du salarié ne compte aucun jour travaillé dans le mois : la part du salaire ne se dit pas."))?,
    };
    Ok(DecompteDuMois { fraction, jours, jours_du_mois, regle, jours_travailles: horaire.jours.clone(), feries_non_payes })
}
