//! LE TAUX AT/MP QUE LA CARSAT NOTIFIE SE POSE DE LUI-MEME. La notification arrive en retour DSN de nature 34 (« CR métier
//! taux AT », guide de l'API DSN v6.8, §5.26.1), au format des bilans : la declaration a laquelle elle est rattachee
//! (`SIREN`, `nic_affectation`) et un element `taux_AT` (`code_risque`, `taux`, `date_effet`, `date_calcul`…), specification
//! du schema des bilans VT2.2 (`atelier/reference/sources/gip-mds-api-dsn/`, LISEZ-MOI, « La notification du taux AT/MP »).
//! Le code et le taux se lisent (regle 1) et se posent dans la fiche entreprise a leur date d'effet ; un etablissement qui
//! n'est pas celui de la compagnie ne pose rien.

use std::path::{Path, PathBuf};

use routes::accueil::ajouter_venant;
use compagnie::installation::Venue;
use serde_json::{json, Value};

use crate::outils::{datee, dossier_de_test, ecrire, fiche_au, lire};

fn notification(racine: &Path, nom: &str, siren: &str, nic: &str, taux_at: &[(&str, &str, &str, &str)]) -> (Value, PathBuf) {
    let taux: String = taux_at
        .iter()
        .map(|(code, taux, effet, calcul)| {
            format!("<taux_AT><code_ctn>I</code_ctn><section>01</section><code_risque>{}</code_risque><taux>{}</taux><date_effet>{}</date_effet><date_calcul>{}</date_calcul></taux_AT>", code, taux, effet, calcul)
        })
        .collect();
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" profil=\"DSN\" version=\"v02r02\"><declaration>\
         <declaration_identification><rang>1</rang><nature>01</nature><SIREN>{}</SIREN><nic_affectation>{}</nic_affectation></declaration_identification>\
         <declaration_bilan><etat>OK</etat>{}</declaration_bilan></declaration></ns2:rapport>",
        siren, nic, taux
    );
    let chemin = racine.join("retours").join(nom);
    std::fs::create_dir_all(chemin.parent().unwrap()).unwrap();
    std::fs::write(&chemin, xml).unwrap();
    (json!({"nature": "34", "publication": "20251215100000", "statut": "OK"}), chemin)
}

fn suites(racine: &Path, notifications: Vec<(Value, PathBuf)>) -> Vec<String> {
    let ctx = droit::contexte::Contexte::nouveau(racine, None).unwrap();
    let notifications = notifications.into_iter().map(|(v, p)| (serde_json::from_value(v).unwrap(), p)).collect();
    let s = envois::suites::Suites { taux_atmp: notifications, ..Default::default() };
    let faites = routes::gestes::suites::faire(&ctx, s).unwrap();
    assert!(faites.incidents.is_empty(), "{:?}", faites.incidents);
    faites.a_faire
}

#[test]
fn la_notification_lue_pose_le_taux_de_l_annee_a_sa_date_d_effet() {
    let racine = ajouter_venant(&dossier_de_test("notification_atmp", "pose"), "Les Tréteaux Notifiés", Venue::Neuve).unwrap().racine;
    let mut ent = fiche_au::<droit::fiche::FicheEntrepriseDatee>(&lire(&racine, "config/entreprise.json"), socle::date::Jour::nouveau(2025, 12, 15).unwrap());
    ent["siren"] = json!("123456873");
    ent["nic"] = json!("00017");
    ecrire(&racine, "config/entreprise.json", &datee(&ent));

    // Effet au 1er janvier : le taux de l'annee, avec la notification pour source.
    let a_faire = suites(&racine, vec![notification(&racine, "n1.xml", "123456873", "00017", &[("923AD", "1.70", "20260101", "20251210")])]);
    assert!(a_faire.is_empty(), "{:?}", a_faire);
    let jour = |a, m, d| socle::date::Jour::nouveau(a, m, d).unwrap();
    let taux_au = |j: socle::date::Jour| {
        let f: droit::fiche::FicheEntrepriseDatee = droit::fiche::lire_fichier(&racine.join("config/entreprise.json"), "config/entreprise.json").unwrap();
        droit::fiche::valeur_au(f.taux_atmp.as_deref().unwrap(), &j).unwrap()
    };
    let t = taux_au(jour(2026, 1, 1));
    assert_eq!((t.code_risque.as_ref().map(droit::bareme::CodeRisqueAt::as_str), t.valeur.clone(), t.depuis, t.provenance), (Some("923AD"), Some(droit::fiche::Montant::Flottant(socle::num::Decimal::new(17, 1))), Some(jour(2026, 1, 1)), Some(droit::fiche::ProvenanceAtmp::Notification)), "{:?}", t);
    assert_eq!(t.notification.and_then(|n| n.effet), Some(jour(2026, 1, 1)));

    // Effet en cours d'annee : le taux d'avant jusqu'a la date d'effet, le notifie ensuite.
    let a_faire = suites(&racine, vec![notification(&racine, "n2.xml", "123456873", "00017", &[("923AD", "1,90", "20260701", "20260615")])]);
    assert!(a_faire.is_empty(), "{:?}", a_faire);
    let au = |j| taux_au(j).valeur;
    assert_eq!((au(jour(2026, 6, 30)), au(jour(2026, 7, 1))), (Some(droit::fiche::Montant::Flottant(socle::num::Decimal::new(17, 1))), Some(droit::fiche::Montant::Flottant(socle::num::Decimal::new(19, 1)))));

    // Un autre etablissement : rien ne se pose, et cela se dit.
    let a_faire = suites(&racine, vec![notification(&racine, "n3.xml", "987654321", "00011", &[("923AD", "2.10", "20270101", "20261210")])]);
    assert_eq!(a_faire.len(), 1, "{:?}", a_faire);
    assert_eq!(taux_au(jour(2027, 6, 1)).depuis, Some(jour(2026, 7, 1)));
}
