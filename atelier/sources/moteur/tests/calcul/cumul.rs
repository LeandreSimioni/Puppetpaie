//! Le plafond d'enseignement de la fiche livree (`logiciel/donnees/rubriques-base.json`, France Travail cite).

use crate::outils;
use droit::cumul::plafond_enseignement;
use serde_json::json;
use socle::date::Jour;

/// « Retenues pour les 507 heures dans la limite de 70 h (moins de 50 ans) ou 120 h (50 ans et plus) ; l'age retenu est
/// celui a la fin du contrat » (la fiche `enseignement`, France Travail, francetravail.fr/spectacle, 10/09/2026).
#[test]
fn le_plafond_d_enseignement_suit_l_age_a_la_date() {
    let fiche = outils::lire(&outils::code_livre(), "rubriques-base.json")["rubriques"]["enseignement"]["plafond_annuel_heures"].clone();
    assert_eq!(fiche, json!({"moins_de_50_ans": 70, "50_ans_et_plus": 120}));
    let racine = outils::dossier_de_test("cumul", "plafond");
    let j = outils::le_jour;
    let plafond = |naissance: Option<Jour>, date: Jour| socle::grandeurs::ecrit::sans_zero::dit(plafond_enseignement(&racine, naissance, date).unwrap());
    assert_eq!(plafond(None, j("2026-09-27")), "70", "sans date de naissance, le plafond bas");
    assert_eq!(plafond(Some(j("1976-09-27")), j("2026-09-27")), "120", "50 ans le jour meme");
    assert_eq!(plafond(Some(j("1976-09-28")), j("2026-09-27")), "70", "50 ans le lendemain");
    assert_eq!(plafond(Some(j("1976-02-29")), j("2026-02-28")), "70", "ne un 29 fevrier : pas encore 50 ans le 28");
    assert_eq!(plafond(Some(j("1976-02-29")), j("2026-03-01")), "120");
    assert_eq!(plafond(Some(j("1975-12-31")), j("2025-12-31")), "120");
    assert_eq!(plafond(Some(j("2010-05-05")), j("2026-09-27")), "70");
}
