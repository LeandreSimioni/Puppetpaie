//! L'ATTESTATION EMPLOYEUR (AER) SE COMPARE A LA PAIE (`documents::aer::ecarts`), sur les Treteaux du banc : la
//! comedienne FULLER, deux cachets les 20 et 21/11/2026, sous le contrat FU1.26.01 que la DSN de novembre a declare.
//! Chaque colonne imprimee se rapproche de la paie de meme numero et de meme periode, ses cachets et ses heures egaux aux
//! siens ; une colonne qu'aucune paie n'explique, ou une periode de la paie qu'aucune colonne ne porte, est un ecart :
//! signer, ce serait certifier exacte une attestation qui ajoute ou retire une periode d'emploi.

use documents::aer::{self, Ligne};
use engagement::numeros::NumeroContrat;
use socle::grandeurs::Cachets;

use crate::outils::le_jour as j;
use crate::outils::rejeu::rejouer;

/// Une colonne de l'attestation : ce contrat, ces dates (`AAAA-MM-JJ`), ces cachets, aucune heure (une comedienne au
/// cachet).
fn colonne(numero: &str, debut: &str, fin: &str, cachets: u32) -> Ligne {
    Ligne { debut: Some(j(debut)), fin: Some(j(fin)), numero: Some(NumeroContrat::lire(numero).unwrap()), cachets: Some(Cachets::fixes(cachets)), heures: None }
}

#[test]
fn l_attestation_se_compare_a_la_paie_de_son_contrat() {
    let (r, c) = rejouer("treteaux-du-banc", &crate::outils::dossier_de_test("aer", "treteaux"));
    let ctx = droit::contexte::Contexte::nouveau(&c, Some(r.fin.expect("un scenario sans geste").jour_de_paris())).unwrap();
    let departs = envois::journal::lu(&c).unwrap().departs;
    let Ok((_, paies)) = documents::ecrire_documents::paies_aux_numeros_declares(&ctx, crate::outils::le_mois("2026-11"), None, &departs) else {
        panic!("les paies de novembre ne se lisent pas")
    };
    let juste = colonne("FU1.26.01", "2026-11-20", "2026-11-21", 2);
    assert_eq!(aer::ecarts(std::slice::from_ref(&juste), &paies).unwrap(), Vec::<String>::new());

    // Un cachet de plus sur l'attestation : un ecart, qui dit les deux nombres.
    let e = aer::ecarts(&[colonne("FU1.26.01", "2026-11-20", "2026-11-21", 3)], &paies).unwrap();
    assert_eq!(e.len(), 1, "{:?}", e);
    assert!(["FU1.26.01", "3 cachets", "2"].iter().all(|x| e[0].contains(x)), "{:?}", e);

    // Une periode en trop : la colonne d'octobre, qu'aucune paie n'explique.
    let octobre = colonne("FU1.26.01", "2026-10-05", "2026-10-06", 1);
    let e = aer::ecarts(&[juste.clone(), octobre], &paies).unwrap();
    assert_eq!(e.len(), 1, "{:?}", e);
    assert!(e[0].contains("2026-10-05") && e[0].contains("aucune paie"), "{:?}", e);

    // Une periode en moins : la paie de novembre qu'aucune colonne ne porte.
    let e = aer::ecarts(&[], &paies).unwrap();
    assert_eq!(e.len(), 1, "{:?}", e);
    assert!(e[0].contains("2026-11-20") && e[0].contains("ne porte pas"), "{:?}", e);

    // Le meme contrat sous un autre numero (le compteur d'aujourd'hui, pas celui de la DSN) ne s'explique pas : la colonne
    // en trop, et la paie en moins.
    let e = aer::ecarts(&[colonne("FU1.26.02", "2026-11-20", "2026-11-21", 2)], &paies).unwrap();
    assert_eq!(e.len(), 2, "{:?}", e);
}
