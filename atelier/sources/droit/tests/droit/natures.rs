//! `contrat_court` des natures de contrat (`contrats-base.json`), contre `natures-court.json`.
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit la reference.

use crate::outils::typee::{reference, verifier, Ligne};
use droit::natures::{contrat_court, NatureDeContrat};
use serde::{Deserialize, Serialize};
use socle::date::Jour;

/// Un jour tel que la reference l'ecrit : (an, mois, jour).
type JourEcrit = (i32, u32, u32);

fn jour((an, mois, jour): JourEcrit) -> Jour {
    Jour { an, mois, jour }
}

/// Un contrat et le jour ou on le lit : est-il court ?
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CasCourt {
    nature: NatureDeContrat,
    debut: JourEcrit,
    fin: Option<JourEcrit>,
    le: JourEcrit,
}

#[test]
fn contrat_court_des_natures() {
    let lignes: Vec<Ligne<CasCourt, bool>> = reference("natures-court.json");
    assert!(lignes.len() >= 39);
    let obtenu = lignes
        .into_iter()
        .map(|l| {
            let n = l.cas.nature.proprietes().unwrap();
            let court = contrat_court(&n, jour(l.cas.debut), l.cas.fin.map(jour), jour(l.cas.le)).unwrap();
            Ligne { cas: l.cas, attendu: court }
        })
        .collect();
    verifier("natures-court.json", obtenu);
}
