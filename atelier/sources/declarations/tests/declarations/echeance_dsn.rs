//! L'echeance de depot d'une DSN mensuelle (`declarations/src/echeance_dsn.rs`), contre ses textes deposes :
//!
//! - le jour : CSS R. 243-6, II (`atelier/reference/sources/echeances-dsn/legi-securite-sociale-R243-6-LEGIARTI000034668597.xml`),
//!   le 5 ou le 15 du mois suivant ;
//! - le report : CSS R. 133-14, I (`atelier/reference/sources/code-de-la-securite-sociale/xml/R133-14.xml`), « Si le delai
//!   [...] expire un jour ferie ou non ouvre, il est proroge jusqu'au jour ouvrable suivant » ;
//! - la pratique du point de depot : `atelier/reference/sources/echeances-dsn/net-entreprises-calendrier-des-declarations-20261015.txt` ;
//! - midi : cahier technique DSN 2026.1, § 1.4.2 (`atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt`) ;
//! - les fetes legales : L. 3133-1 (`logiciel/donnees/feries-base.json`).

use socle::date::Jour;
use declarations::echeance_dsn::{reporter, Echeance, MINUTE_LIMITE};

/// L'echeance de la DSN du mois `numero` de l'annee `an`.
fn du_mois(an: i32, numero: u32, jour: u32) -> socle::echec::R<Echeance> {
    declarations::echeance_dsn::du_mois(socle::mois::Mois::nouveau(an, numero).unwrap(), jour)
}

fn j(an: i32, mois: u32, jour: u32) -> Jour {
    Jour::nouveau(an, mois, jour).unwrap()
}

/// Le calendrier de net-entreprises (releve du 15/10/2026) : DSN d'octobre 2026 au 15, le dimanche 15/11/2026 devient le
/// lundi 16/11/2026 ; R. 133-14, I.
#[test]
fn un_dimanche_reporte_au_lundi() {
    let e = du_mois(2026, 10, 15).unwrap();
    assert_eq!((e.nominale, e.jour, e.reportee()), (j(2026, 11, 15), j(2026, 11, 16), true));
    assert!(e.pourquoi().unwrap().contains("dimanche"), "{:?}", e.pourquoi());
}

/// Le meme calendrier : DSN de novembre 2026 au 5, le samedi 05/12/2026 devient le lundi 07/12/2026 -- le samedi est non
/// ouvre pour le point de depot.
#[test]
fn un_samedi_reporte_au_lundi() {
    let e = du_mois(2026, 11, 5).unwrap();
    assert_eq!((e.nominale, e.jour), (j(2026, 12, 5), j(2026, 12, 7)));
    assert_eq!(e.sautes.len(), 2, "le samedi et le dimanche");
}

/// R. 133-14, I, « un jour ferie » ; L. 3133-1, 8° (`feries-base.json`) : le mercredi 15/08/2029, Assomption, reporte la
/// DSN de juillet 2029 au jeudi 16/08/2029.
#[test]
fn un_ferie_reporte_au_jour_ouvre_suivant() {
    let e = du_mois(2029, 7, 15).unwrap();
    assert_eq!((e.nominale, e.jour), (j(2029, 8, 15), j(2029, 8, 16)));
    assert!(e.pourquoi().unwrap().contains("Assomption"), "{:?}", e.pourquoi());
    // Le dimanche 05/04/2026, puis le lundi de Paques 06/04/2026 (L. 3133-1, 2°) : le mardi 07/04/2026.
    assert_eq!(du_mois(2026, 3, 5).unwrap().jour, j(2026, 4, 7));
}

/// INFERENCE (aucune piece officielle ne montre ce cas, `echeance_dsn.rs`, inference 2) : le vendredi 15/08/2025, ferie,
/// reporte au lundi 18/08/2025, pas au samedi.
#[test]
fn un_vendredi_ferie_reporte_au_lundi_par_inference() {
    assert_eq!(du_mois(2025, 7, 15).unwrap().jour, j(2025, 8, 18));
}

/// Le calendrier de net-entreprises : jeudi 05/11/2026, jeudi 15/10/2026 et mardi 15/12/2026 ne bougent pas (R. 243-6, II,
/// seul) ; le passage de decembre a janvier.
#[test]
fn une_date_ouvree_ne_bouge_pas() {
    for (mois, jour, attendu) in [(10, 5, j(2026, 11, 5)), (9, 15, j(2026, 10, 15)), (11, 15, j(2026, 12, 15)), (8, 15, j(2026, 9, 15))] {
        let e = du_mois(2026, mois, jour).unwrap();
        assert_eq!((e.jour, e.reportee(), e.pourquoi()), (attendu, false, None), "{} au {}", mois, jour);
    }
    // Le 05/01/2027 est un mardi : la DSN de decembre 2026 au 5.
    assert_eq!(du_mois(2026, 12, 5).unwrap().jour, j(2027, 1, 5));
}

/// Cahier technique § 1.4.2 : l'heure limite est midi ; l'annule-et-remplace s'arrete la veille a minuit -- la veille de
/// l'echeance reportee (inference 3).
#[test]
fn l_heure_limite_est_midi_et_la_veille_suit_le_report() {
    let e = reporter(j(2026, 11, 15)).unwrap();
    assert_eq!(MINUTE_LIMITE, 12 * 60);
    let v = e.dite().unwrap();
    assert_eq!(
        (v.limite.jour_de_paris().iso().as_str(), v.limite.seconde_du_jour_de_paris(), v.nominale.iso().as_str()),
        ("2026-11-16", 12 * 3_600, "2026-11-15")
    );
    assert_eq!(v.veille_annule_et_remplace.map(|j| j.iso()).as_deref(), Some("2026-11-15"));
    assert!(!v.inference.is_empty(), "l'inference se dit comme telle");
    assert!(v.textes.iter().any(|t| t.contains("R133-14.xml")));
    assert!(e.phrase().contains("lundi 16/11/2026"), "{}", e.phrase());
}
