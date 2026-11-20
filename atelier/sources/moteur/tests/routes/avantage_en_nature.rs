//! L'AVANTAGE EN NATURE N'EST PAS UNE SOMME RECUE : le bulletin porte « Le montant de la somme effectivement reçue par
//! le salarié » (C. trav. R. 3243-1, 10°, `atelier/reference/sources/code-du-travail/xml/R3243-1.xml`). L'avantage entre
//! dans le brut et dans le montant net social, qui compte « les avantages et accessoires en nature ou en argent » (arrete
//! du 25/02/2016, art. 1er, II, 1°, `atelier/reference/sources/montant-net-social/legi-arrete-20160225-bulletin.md`) ; le
//! net a payer, non. Le banc des Treteaux : FULLER, deux cachets en novembre 2026, et un repas fourni de 20 €.

use serde_json::json;

use crate::outils::rejeu::{rejouer_dossier, scenario};
use socle::grandeurs::Montant;

use crate::outils::{dossier_de_test, grandeur, identite, le_mois, paie_ecrite, reussi, route, valider_la_carte};

#[test]
fn le_net_a_payer_ne_compte_pas_l_avantage_en_nature() {
    let d = dossier_de_test("avantage_en_nature", "net_a_payer");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-11-02";
    let mois = le_mois("2026-11");
    let lignes = json!([
        {"rubrique": "cachet", "quantite": {"cachets": 2}, "taux": 150, "lieu": "Salle fictive de Rezé", "dates": ["2026-11-20", "2026-11-21"], "planning": []},
        {"rubrique": "avantage_nature", "montant": 20, "genre": "02"},
    ]);
    let carte = json!({"ecrite": identite(&c, mois, "FULLER", None)});
    reussi(&route(&c, "enregistrer-saisie", json!({"mois": "2026-11", "carte": carte, "changements": [{"case": "lignes", "valeur": lignes}]}), jour));
    reussi(&valider_la_carte(&c, mois, "FULLER", jour));

    let p = paie_ecrite(&c, "2026-11", "FULLER", None);
    let m = |cle: &str| grandeur::<Montant>(p[cle].as_str().unwrap_or_else(|| panic!("{} : {}", cle, p)));
    let avantage = grandeur::<Montant>("20");
    let remuneration = p["remuneration"].as_array().unwrap();
    assert!(remuneration.iter().any(|l| l["code"] == "avantage_nature" && grandeur::<Montant>(l["montant"].as_str().unwrap()) == avantage), "{}", p);
    // Aucune autre ligne hors assiette : le net d'avant (brut moins cotisations salariales) est le montant net social.
    assert!(remuneration.iter().all(|l| l["soumis"] == json!(true)), "{}", p);
    assert_eq!(m("net_social"), m("brut") - m("total_sal"), "{}", p);
    assert_eq!(m("net_a_payer"), m("net_social") - avantage, "{}", p);
}
