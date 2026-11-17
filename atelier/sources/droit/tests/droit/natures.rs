//! `contrat_court` des natures de contrat (`contrats-base.json`), contre `natures-court.json`.
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit la reference.

use crate::outils::typee::{reference, verifier, Ligne};
use droit::natures::{contrat_court, NatureDeContrat};
use serde::{Deserialize, Serialize};
use socle::date::{Jour, PeriodeOuverte};
use socle::forme::NombreLu;

/// Un jour tel que la reference l'ecrit : (an, mois, jour), trois nombres.
type JourEcrit = (NombreLu, NombreLu, NombreLu);

/// Le jour qu'ils ecrivent, lu par sa porte (`socle::piece`) : `AAAA-MM-JJ`.
fn jour((an, mois, jour): &JourEcrit) -> Jour {
    let n = |x: &NombreLu| x.entier_ecrit().unwrap().unwrap();
    let iso = format!("{:0>4}-{:0>2}-{:0>2}", n(an), n(mois), n(jour));
    socle::piece::Zone::lue(&iso, socle::piece::format::JourIso).valeur().unwrap()
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
            let contrat = PeriodeOuverte::du(jour(&l.cas.debut), l.cas.fin.as_ref().map(jour)).unwrap();
            let court = contrat_court(&n, &contrat, jour(&l.cas.le)).unwrap();
            Ligne { cas: l.cas, attendu: court }
        })
        .collect();
    verifier("natures-court.json", obtenu);
}
