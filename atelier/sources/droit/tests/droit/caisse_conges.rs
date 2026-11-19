//! Qui l'employeur declare a la caisse des Conges Spectacles (`droit::caisse_conges::affilie`). Textes
//! (`atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md`) : code du travail D. 7121-40 (« le personnel
//! artistique et technique ») et D. 7121-41 (« qu'ils n'ont pas employe de facon continue pendant les douze mois
//! precedant la demande de conge ») ; fiche Audiens (« quelle que soit [...] la nature du contrat de travail (CDD de
//! droit commun, CDD d'usage, etc) », sous contrat a duree determinee « de moins de 12 mois »). Aucun ne renvoie a la
//! liste de l'annexe 8 : le technicien en CDD est declare, que la liste de sa convention le nomme ou non (la liste 6,
//! IDCC 1285, nomme le staffeur, la liste 7, IDCC 3090, non : `unedic-annexe8-convention-20241115.txt`, art. 71).

use droit::caisse_conges::{affilie, douze_mois_ou_plus};
use droit::contexte::IdSalarie;
use droit::fiche::{FicheSalarie, SalarieDuContrat};
use droit::natures::NatureDeContrat as N;
use socle::date::{Jour, PeriodeOuverte};

use crate::outils::le_jour as j;
use crate::outils::typee::lu;

#[test]
fn qui_est_declare_a_la_caisse() {
    let (cddu, cdd, cdi) = (N::CdduSpectacle.proprietes().unwrap(), N::Cdd.proprietes().unwrap(), N::Cdi.proprietes().unwrap());
    let comedien = fiche(r#"{"code_emploi_ft": "COM010", "statut": "artiste"}"#, None);
    let regisseur = |idcc: &str| fiche(r#"{"code_emploi_ft": "REG130", "statut": "technicien"}"#, Some(idcc));
    let sans_metier = fiche(r#"{"statut": "technicien"}"#, Some("3090"));
    let (debut, trois_mois, un_an) = (j("2026-09-01"), j("2026-11-30"), j("2027-08-31"));
    assert!(douze_mois_ou_plus(&socle::date::Periode::du_au(debut, un_an).unwrap()).unwrap());
    let contrat = |fin: Option<Jour>| PeriodeOuverte::du(debut, fin).unwrap();
    let (trois_mois, un_an, sans_terme) = (contrat(Some(trois_mois)), contrat(Some(un_an)), contrat(None));
    assert!(affilie(&cddu, &comedien, &trois_mois).unwrap());
    assert!(!affilie(&cddu, &comedien, &un_an).unwrap(), "un CDDU de douze mois");
    assert!(affilie(&cdd, &comedien, &trois_mois).unwrap(), "un artiste en CDD de droit commun");
    assert!(!affilie(&cdd, &comedien, &un_an).unwrap(), "un CDD de douze mois");
    assert!(affilie(&cdd, &regisseur("3090"), &trois_mois).unwrap(), "un technicien en CDD");
    assert!(affilie(&cddu, &regisseur("1922"), &trois_mois).unwrap(), "un technicien en CDDU");
    // Le staffeur est sur la liste du spectacle vivant subventionne (1285), pas sur celle du prive (3090) : la caisse ne
    // suit pas la liste.
    let staffeur = |idcc: &str| fiche(r#"{"code_emploi_ft": "STA010", "statut": "technicien"}"#, Some(idcc));
    assert!(affilie(&cdd, &staffeur("3090"), &trois_mois).unwrap(), "un technicien hors de la liste en CDD");
    assert!(droit::caisse_conges::cddu_hors_des_annexes(&cddu, &staffeur("3090")).unwrap(), "son CDDU est hors des annexes");
    assert!(!droit::caisse_conges::cddu_hors_des_annexes(&cddu, &staffeur("1285")).unwrap(), "la liste 6 le nomme");
    assert!(!affilie(&cdd, &sans_metier, &trois_mois).unwrap(), "un metier hors de la nomenclature");
    assert!(!affilie(&cdi, &comedien, &sans_terme).unwrap(), "un CDI");
    assert!(affilie(&cdd, &comedien, &sans_terme).unwrap_err().phrase.contains("sans terme"));
}

/// La fiche telle que la paie la lit : ses champs, et la convention de l'employeur.
fn fiche(champs: &str, idcc: Option<&str>) -> SalarieDuContrat {
    let champs = lu(champs);
    let mut s = SalarieDuContrat::de(FicheSalarie { id: IdSalarie::lire("X").unwrap(), champs });
    s.idcc_de_l_employeur = idcc.map(|i| droit::fiche::Idcc::lire(i).unwrap());
    s
}
