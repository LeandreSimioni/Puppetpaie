//! LA DUREE D'UNE JOURNEE DE TRAVAIL SAISIE. Les heures qu'un engagement fait travailler chaque jour, ligne par ligne :
//! une ligne a l'heure, ou dont les heures se saisissent par unite (une journee, un service), compte celles de chacun de
//! ses jours (sa quantite du jour, `LignePaie::quantites_des_jours` : un jour qui ne dit pas la sienne porte sa part de
//! la quantite de la ligne, le reste sur les premiers). Un cachet (son forfait de 12 h n'est pas une duree de travail),
//! une absence, un conge, un maintien de salaire ne comptent pas.
//!
//! - Au-dela de 24 heures en un jour, la saisie se refuse : une journee n'en a que 24 (une faute de frappe, 25 pour 2,5).
//! - Au-dela de 10 heures, le moteur le dit sans refuser : « La duree quotidienne de travail effectif par salarie ne
//!   peut exceder dix heures, sauf » une derogation de l'inspecteur du travail, l'urgence, ou une convention ou un
//!   accord qui la porte jusqu'a douze heures (code du travail, L. 3121-18 et L. 3121-19,
//!   `atelier/reference/sources/code-du-travail/articles-20261030.md`, « La duree maximale du travail et le repos
//!   quotidien »). C'est l'employeur qui en repond : une tournee a des journees longues, et la convention du spectacle
//!   vivant prive (3090) porte la journee a 12 heures pour certains metiers et certaines annexes.

use std::collections::BTreeMap;

use crate::LignePaie;

use droit::catalogue::rubrique;
use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::R;
use socle::grandeurs::Heures;

/// La duree quotidienne maximale de travail effectif (L. 3121-18) : 10 heures.
const MAXIMUM_QUOTIDIEN: Heures = Heures::fixes(10);

/// Une journee : 24 heures.
const JOURNEE: Heures = Heures::fixes(24);

/// Les heures de travail de chaque jour des lignes `lignes` d'un engagement.
fn heures_par_jour(ctx: &Contexte, lignes: &[LignePaie]) -> R<BTreeMap<Jour, Heures>> {
    let mut jours: BTreeMap<Jour, Heures> = BTreeMap::new();
    for l in lignes {
        let r = rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?;
        let travail = r.nature == droit::catalogue::NatureDeRubrique::Salaire
            && matches!(r.unite, droit::catalogue::Unite::Heure | droit::catalogue::Unite::Jour | droit::catalogue::Unite::Service)
            && r.heures_forfait.is_none()
            && !(r.arret_de_travail || r.conges_pris || r.suspension);
        if !travail {
            continue;
        }
        // Une ligne de travail se compte jour par jour ; un jour vide ne compte rien.
        let Some(quantites) = l.quantites_des_jours(r.unite) else { continue };
        let par_unite = r.heures_par_unite(l.heures());
        for (d, q) in l.dates().iter().zip(quantites) {
            if let Some(q) = q {
                *jours.entry(d.jour()).or_insert(Heures::ZERO) += q.heures(par_unite);
            }
        }
    }
    Ok(jours)
}

/// LE REFUS : un jour de plus de 24 heures dans l'engagement `e` de `qui`. `None` quand il n'y en a pas.
pub fn plus_d_une_journee(ctx: &Contexte, lignes: &[LignePaie], qui: &str) -> R<Option<String>> {
    let trop: Vec<(Jour, Heures)> = heures_par_jour(ctx, lignes)?.into_iter().filter(|(_, h)| *h > JOURNEE).collect();
    let Some((j, h)) = trop.first() else { return Ok(None) };
    Ok(Some(format!(
        "{} : {} de travail le {}{} : saisissez les heures de ce jour-là, ou répartissez-les sur plus de jours.",
        qui,
        socle::texte::heures(*h),
        j.jj_mm_aaaa(),
        crate::JOURNEE_DE_PLUS_DE_24
    )))
}

/// CE QUI SE DIT SANS SE REFUSER : les jours de l'engagement `e` de `qui` au-dela de la duree quotidienne maximale
/// (L. 3121-18). `None` quand il n'y en a pas.
pub fn au_dela_du_maximum(ctx: &Contexte, lignes: &[LignePaie], qui: &str) -> R<Option<String>> {
    let longs: Vec<(Jour, Heures)> = heures_par_jour(ctx, lignes)?.into_iter().filter(|(_, h)| *h > MAXIMUM_QUOTIDIEN && *h <= JOURNEE).collect();
    if longs.is_empty() {
        return Ok(None);
    }
    let dits: Vec<String> = longs.iter().map(|(j, h)| format!("{} le {}", socle::texte::heures(*h), j.jj_mm())).collect();
    Ok(Some(format!(
        "{} : {}. La durée quotidienne de travail effectif ne peut dépasser 10 heures (code du travail, L. 3121-18), sauf \
         une convention ou un accord qui la porte jusqu'à 12 heures (L. 3121-19), une dérogation de l'inspecteur du travail \
         ou l'urgence (L. 3121-18, 1° et 2°).",
        qui,
        dits.join(", ")
    )))
}
