//! LA FIN D'ESSAI D'UN CDDU A L'INITIATIVE DE L'EMPLOYEUR (C. trav. L1221-25,
//! `atelier/reference/sources/code-du-travail/xml/L1221-25.xml`) : « Lorsqu'il est mis fin, par l'employeur, au contrat en
//! cours ou au terme de la période d'essai définie [...] à l'article L. 1242-10 pour les contrats stipulant une période
//! d'essai d'au moins une semaine, le salarié est prévenu dans un délai qui ne peut être inférieur à : 1° Vingt-quatre heures
//! en deçà de huit jours de présence » ; « son inexécution ouvre droit pour le salarié, sauf s'il a commis une faute grave,
//! à une indemnité compensatrice ». Le banc des Treteaux : FULLER, en CDDU les 20 et 21/11/2026, voit son essai rompu
//! (034) le 21, son dernier jour : le delai de vingt-quatre heures expire le 22, apres la fin. Le montant, la compagnie le
//! dit ; sans lui le calcul refuse en nommant la case et les dates ; avec la faute grave, ou un essai de moins d'une
//! semaine, rien n'est du.

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use socle::grandeurs::Montant;

use crate::outils::{dossier_de_test, grandeur, identite, le_mois, refus, reussi, route};

#[test]
fn le_delai_de_prevenance_d_un_cddu_suit_l1221_25() {
    let d = dossier_de_test("fin_d_essai_d_un_cddu", "l1221_25");
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    let jour = "2026-11-02";
    let mois = le_mois("2026-11");
    let changer = |changements: Value| {
        let carte = json!({"ecrite": identite(&c, mois, "FULLER", None)});
        reussi(&route(&c, "enregistrer-saisie", json!({"mois": "2026-11", "carte": carte, "changements": changements}), jour)).clone()
    };
    let apercu = || route(&c, "apercu", json!({"mois": "2026-11", "modifications": []}), jour);
    let indemnite = |r: &Value| -> Option<Montant> {
        let paies = reussi(r)["paies"].as_array().unwrap().clone();
        let fuller = paies.iter().find(|p| p["salarie"]["cle"] == "FULLER").expect("la paie de FULLER");
        fuller["remuneration"].as_array().unwrap().iter().find(|l| l["rubrique"] == "indemnite_delai_prevenance").map(|l| grandeur(l["montant"].as_str().unwrap()))
    };

    changer(json!([
        {"case": "motif_rupture", "valeur": "034"},
        {"case": "notification_rupture", "valeur": "2026-11-21"},
        {"case": "periode_essai_jours", "valeur": 7},
    ]));
    // Due, et le montant n'est pas dit : le calcul refuse en nommant la case et le delai non effectue.
    let (_, phrase) = refus(&apercu());
    assert!(phrase.contains("FULLER") && phrase.contains("Indemnité du délai de prévenance") && phrase.contains("du 22/11/2026 au 22/11/2026"), "{}", phrase);

    // Dit : la ligne le paie.
    changer(json!([{"case": "indemnite_delai_prevenance", "valeur": 150}]));
    assert_eq!(indemnite(&apercu()), Some(grandeur::<Montant>("150")));

    // « sauf s'il a commis une faute grave » : rien n'est du.
    changer(json!([{"case": "faute_grave", "valeur": true}]));
    assert_eq!(indemnite(&apercu()), None);

    // Un essai de moins d'une semaine : L1221-25 ne donne aucun delai a ce CDD.
    changer(json!([{"case": "faute_grave", "valeur": null}, {"case": "periode_essai_jours", "valeur": 5}]));
    assert_eq!(indemnite(&apercu()), None);
}
