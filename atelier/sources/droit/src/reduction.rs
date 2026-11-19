//! La reduction generale degressive unique (CSS L. 241-13, D. 241-7 ; parametres dans la section `reduction_generale`
//! du bareme) : le coefficient d'un brut paye pour des heures, son montant et ses parts Urssaf, chomage et
//! Agirc-Arrco, sa limite sous deduction forfaitaire specifique, et le seuil de SMIC d'une cotisation qui ne s'applique
//! qu'au-dela. Le droit recoit le bareme, le brut et les heures ; l'engagement l'appelle a son calcul.

use serde::Serialize;

use crate::fiche::Statut;
use crate::forme_bareme::ParametresDeLaReduction;
use socle::echec::R;
use socle::grandeurs::{Coefficient, Heures, Montant, Taux};

/// Un parametre exige de la section.
fn requis<T>(v: Option<T>, cle: &str) -> R<T> {
    v.ok_or_else(|| socle::fichiers::absent(cle))
}

/// Le refus d'une division par zero : `a_nul`, le dividende est nul aussi.
fn par_zero(a_nul: bool) -> socle::echec::Echec {
    socle::echec::refus(if a_nul { "zéro divisé par zéro" } else { "division par zéro" })
}

/// `a / b`, ou le refus d'une division par zero.
fn diviser(a: Coefficient, b: Coefficient) -> R<Coefficient> {
    a.sur(b).ok_or_else(|| par_zero(a.est_nul()))
}

/// La reduction generale appliquee : ce que la DSN declare. Elle s'ecrit chaque nombre en texte (`nombre`).
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReductionGenerale {
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub coefficient: Coefficient,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub montant: Montant,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub smic_reference: Montant,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub heures: Heures,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub urssaf: Montant,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub chomage: Montant,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub agirc_arrco: Montant,
}

/// 0,6 : le diviseur du parametre T de la formule de la reduction (`FormuleDeReduction::Fillon`).
const SIX_DIXIEMES: Coefficient = Coefficient::dixiemes(6);

/// 2 : le diviseur du crochet de la formule de la reduction.
const DEUX: Coefficient = Coefficient::entier_fixe(2);

/// La reduction generale degressive unique d'un contrat, ou `None` (art. D. 241-7 CSS ; `rg` : la section
/// `reduction_generale` de la pile).
pub fn reduction_generale(
    rg: Option<&ParametresDeLaReduction>, statut: Option<Statut>, brut: Montant, heures: Heures, caisse_conges: bool,
) -> R<Option<ReductionGenerale>> {
    let Some(rg) = rg else { return Ok(None) };
    let Some(statut) = statut else {
        return Err(socle::echec::refus("La fiche du salarié ne dit pas son statut (artiste ou technicien), dont dépend la réduction générale : le remplir."));
    };
    if !rg.statuts.iter().flatten().any(|s| *s == statut) || brut <= Montant::ZERO || heures <= Heures::ZERO {
        return Ok(None);
    }
    let smic_ref = (heures * requis(rg.smic_horaire_reference, "smic_horaire_reference")?).au_centime();
    let plafond = requis(rg.plafond_smic, "plafond_smic")?;
    let fillon = rg.formule == Some(crate::forme_bareme::FormuleDeReduction::Fillon);
    let mut coefficient;
    if brut >= smic_ref * plafond {
        coefficient = Coefficient::ZERO;
    } else {
        // Le SMIC de reference plafonne rapporte au brut (positif, deja vu).
        let rapport = (smic_ref * plafond).rapport(brut).ok_or_else(|| par_zero(false))?;
        if fillon {
            let t = requis(rg.t.map(|n| n.0), "t")?;
            let x = diviser(t, SIX_DIXIEMES)? * (rapport - Coefficient::UN);
            coefficient = socle::grandeurs::decimales_ecrites(t.min(x).au_dix_millieme(), 4);
        } else {
            let crochet = Coefficient::UN.min(diviser(rapport - Coefficient::UN, DEUX)?);
            let puissance = requis(rg.puissance, "puissance")?;
            // La phrase du refus cite les deux nombres : sa porte.
            let terme = crochet
                .puissance(puissance)
                .ok_or_else(|| {
                    socle::refus!("la réduction générale ne se calcule pas : {} puissance {}", socle::grandeurs::ecrit::texte::dit(crochet), socle::grandeurs::ecrit::texte::dit(puissance))
                })?;
            coefficient = requis(rg.tmin, "tmin")? + requis(rg.tdelta, "tdelta")? * terme;
        }
    }
    if let Some(m) = rg.majoration_caisse_conges {
        if caisse_conges && !coefficient.est_nul() {
            coefficient = diviser(coefficient * m.numerateur, m.denominateur)?;
        }
    }
    let coefficient = if coefficient.est_nul() { coefficient } else { socle::grandeurs::decimales_ecrites(coefficient.au_dix_millieme(), 4) };
    reduction_au_coefficient(Some(rg), coefficient, brut, heures).map(Some)
}

/// La reduction d'un coefficient deja fixe (celui de la regularisation annuelle d'un permanent).
pub fn reduction_au_coefficient(rg: Option<&ParametresDeLaReduction>, coefficient: Coefficient, brut: Montant, heures: Heures) -> R<ReductionGenerale> {
    let rg = rg.ok_or_else(|| socle::echec::a_la_cle("Le barème de l'année ne porte pas la réduction générale des cotisations : le signaler au développeur.", "reduction_generale", ""))?;
    let smic_ref = (heures * requis(rg.smic_horaire_reference, "smic_horaire_reference")?).au_centime();
    let montant = (brut * coefficient).au_centime();
    repartir_la_reduction(rg, coefficient, montant, smic_ref, heures)
}

fn repartir_la_reduction(rg: &ParametresDeLaReduction, coefficient: Coefficient, montant: Montant, smic_ref: Montant, heures: Heures) -> R<ReductionGenerale> {
    let total_points = if rg.formule == Some(crate::forme_bareme::FormuleDeReduction::Fillon) { requis(rg.t.map(|n| n.0), "t")? } else { requis(rg.tmin, "tmin")? + requis(rg.tdelta, "tdelta")? };
    // La part d'un organisme : le montant au taux de ses points, partage par les points de la formule.
    let part = |taux: Option<Taux>, cle: &str| -> R<Montant> {
        if montant.est_nul() {
            return Ok(Montant::ZERO);
        }
        let aux_points = montant.fois_taux(requis(taux, cle)?);
        Ok(aux_points.sur(total_points).ok_or_else(|| par_zero(aux_points.est_nul()))?.au_centime())
    };
    let chomage = part(rg.imputation.as_ref().and_then(|i| i.chomage).map(|n| n.0), "imputation.chomage")?;
    let agirc = part(rg.imputation.as_ref().and_then(|i| i.agirc_arrco), "imputation.agirc_arrco")?;
    Ok(ReductionGenerale {
        coefficient, montant, smic_reference: smic_ref, heures,
        urssaf: montant - chomage - agirc, chomage, agirc_arrco: agirc,
    })
}

/// La reduction generale d'une paie sous deduction forfaitaire specifique. CSS L. 241-13, III : elle << ne tient compte
/// des deductions au titre de frais professionnels calculees forfaitairement en pourcentage de cette remuneration que
/// dans des limites et conditions fixees par arrete >> ; arrete du 4 decembre 2019, art. 1er (redaction de l'arrete du
/// 4 septembre 2025, art. 11 ; `atelier/reference/sources/dfs/arrete-2019-reduction-generale/`) : << Le montant de la
/// reduction generale [...] calcule pour les remunerations apres application de l'abattement forfaitaire [...] ne peut
/// exceder, pour ces remunerations, 130 % du montant la reduction calcule sans application du meme abattement. >> La
/// reduction se calcule donc sur le brut abattu (`abattu`), dans la limite (`limite`, lue au bareme :
/// `deduction_forfaitaire.limite_reduction_generale`) de celle du brut entier.
pub fn reduction_generale_sous_dfs(
    rg: Option<&ParametresDeLaReduction>, limite: Taux, statut: Option<Statut>, brut: Montant, abattu: Montant, heures: Heures, caisse_conges: bool,
) -> R<Option<ReductionGenerale>> {
    let Some(apres) = reduction_generale(rg, statut, abattu, heures, caisse_conges)? else { return Ok(None) };
    // Le brut entier (au moins le brut abattu) ouvre la reduction des qu'elle s'ouvre sur l'abattu (memes parametres,
    // meme statut, memes heures).
    let Some(sans) = reduction_generale(rg, statut, brut, heures, caisse_conges)? else {
        return Err(socle::echec::refus("La réduction générale s'ouvre sur le brut abattu mais pas sur le brut entier : le signaler au développeur."));
    };
    let limite = sans.montant.fois_taux(limite).au_centime();
    if apres.montant <= limite {
        return Ok(Some(apres));
    }
    let rg = rg.ok_or_else(|| socle::fichiers::absent("reduction_generale"))?;
    Ok(Some(repartir_la_reduction(rg, apres.coefficient, limite, apres.smic_reference, apres.heures)?))
}

/// La remuneration du contrat depasse-t-elle `seuil_smic` fois le SMIC de reference (SMIC horaire de la reduction x
/// heures remunerees) ? Une ligne sans seuil (le fichier n'en pose pas) n'appelle pas cette question : elle vaut
/// toujours. Un seuil sans SMIC de reference ou sans heures ne se compare pas : il se refuse.
pub fn au_dela_du_seuil(rg: Option<&ParametresDeLaReduction>, brut: Montant, heures: Heures, seuil: Coefficient) -> R<bool> {
    let smic = requis(rg.and_then(|r| r.smic_horaire_reference), "reduction_generale.smic_horaire_reference")?;
    if heures <= Heures::ZERO {
        return Err(socle::echec::refus("Un seuil de SMIC se compare à des heures rémunérées : la paie n'en porte aucune."));
    }
    Ok(brut > (heures * smic).au_centime() * seuil)
}
