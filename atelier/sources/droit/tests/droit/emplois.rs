//! `resoudre` sur les codes de la fictive, du catalogue et de la nomenclature, et un code inconnu ; contre la reference
//! `emplois-resolus.json`, que `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit.

use crate::outils::typee::{cas_de, verifier, Ligne};
use droit::emplois::{resoudre, CodeEmploi};
use serde::{Deserialize, Serialize};

/// Un code, la case cadre de la fiche (vide, oui ou non), le feminin.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CasResolu {
    code: String,
    cadre: Option<bool>,
    feminin: bool,
}

/// Les sept champs declares, ou le refus.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Resolution {
    #[serde(rename = "champs")]
    Champs {
        emploi: String,
        emploi_affichage: String,
        pcs_ese: Option<String>,
        code_emploi_ft: Option<String>,
        statut: String,
        statut_conventionnel: Option<String>,
        statut_categoriel_rc: String,
    },
    #[serde(rename = "refus")]
    Refus(String),
}

fn resolution(c: &CasResolu) -> Resolution {
    // Le code tel qu'une saisie le range, puis lu (`CodeEmploi`).
    match CodeEmploi::lire(&CodeEmploi::rangee(&c.code)).and_then(|code| resoudre(&code, c.cadre, c.feminin, None)) {
        Ok(r) => Resolution::Champs {
            emploi: r.emploi,
            emploi_affichage: r.emploi_affichage,
            pcs_ese: r.pcs_ese,
            code_emploi_ft: r.code_emploi_ft,
            statut: r.statut.to_string(),
            statut_conventionnel: r.statut_conventionnel.map(|s| s.to_string()),
            statut_categoriel_rc: r.statut_categoriel_rc.to_string(),
        },
        Err(m) => Resolution::Refus(m.phrase),
    }
}

#[test]
fn resoudre_un_metier() {
    let cas: Vec<CasResolu> = cas_de("emplois-resolus.json");
    let refus = cas.iter().map(resolution).filter(|r| matches!(r, Resolution::Refus(_))).count();
    assert!(cas.len() - refus > 700, "{}", cas.len());
    assert_eq!(refus, 42); // XXX999 et le code vide (quatre variantes chacun), et les 17 metiers hors catalogue sans `cadre` dit (deux variantes)
    verifier("emplois-resolus.json", cas.into_iter().map(|c| Ligne { attendu: resolution(&c), cas: c }).collect());
    // Le dramaturge est cadre dans la 1285 (groupe B de la filiere artistique, art. XI.2.1) ; la grille 3090 ne le
    // nomme pas, le defaut du catalogue vaut.
    let cadre = |idcc| resoudre(&CodeEmploi::lire("DRA010").unwrap(), None, false, Some(idcc)).unwrap();
    assert_eq!((cadre("1285").statut_conventionnel.map(|s| s.as_str()), cadre("1285").statut_categoriel_rc.as_str()), (Some("04"), "01"));
    assert_eq!((cadre("3090").statut_conventionnel.map(|s| s.as_str()), cadre("3090").statut_categoriel_rc.as_str()), (Some("05"), "04"));
}
