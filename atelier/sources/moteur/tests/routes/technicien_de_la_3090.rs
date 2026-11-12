//! UN TECHNICIEN DE LA 3090, DEUX REGLES QU'AUCUNE FICTIVE NE JOUE : le Fil de fer n'a pas de permanent, l'Hotel de
//! Bourgogne est en 1285 et ne paie rien lui-meme en 2025. Les Treteaux du Banc (3090), avant leur premiere saisie, y
//! gagnent un regisseur lumiere (REG130 : grille « technique », agent de maitrise), FORTUNY, et une premiere embauche au
//! 02/01/2025.
//!
//! - LA PRIME D'ANCIENNETE DU TECHNICIEN AU MOIS DE L'ANNEXE I. « Après 3 ans de présence, il est attribué à chaque
//!   technicien une prime d'ancienneté de 1 % du salaire conventionnel, versée mensuellement et calculée de la façon
//!   suivante : 4e année : 1 % ; 5e année : 2 % ; 6e année : 3 % et ainsi de suite jusqu'au plafond de 10 % après 13
//!   années de présence. » (3090, annexe I, art. 3.11, KALIARTI000028157577,
//!   `atelier/reference/sources/kali-entrepot/articles.jsonl.gz#3090`). Le salaire conventionnel d'un agent de maitrise
//!   d'un theatre jusqu'a 200 places : « Par mois | 2 185,56 » (avenant du 21/07/2025, en vigueur le 01/09/2025,
//!   `atelier/reference/sources/legifrance-3090-avenant-salaires-20250721.md`, l. 273-275).
//! - LE COMPLEMENT AU-DELA DU SEUIL DU BAREME 2025. Le taux de maladie de 2025 est de 7 % (BOSS, COT_AM_SS_METROP,
//!   « ;7;01/01/2025;30/04/2025;OK », `atelier/reference/boss-table-parametres-2026.csv`), « réduit de 6 points » sous
//!   2,25 SMIC (CSS L. 241-2-1, version 2025, `atelier/reference/sources/legi-entrepot/articles.jsonl.gz#securite-sociale` ;
//!   Audiens : « Réduction de 6 points si salaire < 2,25 SMIC* »,
//!   `atelier/reference/fiches-parametrage/Audiens-Webconf-Nouveautes-2026.txt`, l. 38-39) ; celui des allocations
//!   familiales, de 3,45 % (BOSS, COT_AF_SS_METROP), « réduit de 1,8 point » sous 3,3 SMIC (L. 241-6-1, version 2025 ;
//!   D. 241-3-2 : « est égal à 3,3 fois le montant »). Au-dela du seuil, le taux plein : 7 + 6 = 13 %, 3,45 + 1,8 = 5,25 %.
//!   Le SMIC horaire de 2025 : 11,88 (BOSS, SMIC_H_METROP).

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dec, dossier_de_test, nombre, paie_ecrite, reussi, route, valider};

/// Le jour de chaque geste : celui ou les Treteaux commencent.
const JOUR: &str = "2026-11-02";

/// Les Treteaux du Banc avant leur premiere saisie, une premiere embauche au 02/01/2025, et FORTUNY, regisseur lumiere,
/// present depuis le 15/06/2015 (sa date d'anciennete).
fn les_treteaux_et_leur_regisseur(test: &str) -> PathBuf {
    let d = dossier_de_test("technicien_de_la_3090", test);
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"route\":\"apercu\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    let (_, c) = rejouer_dossier(&sc, &d);
    reussi(&route(&c, "enregistrer-fiche", json!({"nom": "entreprise", "cle": null, "changements": [{"case": "premiere_embauche", "valeur": "2025-01-02"}]}), JOUR));
    let cases = json!([
        ["prenom_affichage", "Mariano"], ["nom_affichage", "Fortuny"], ["nir", "1750544999003"], ["nir_cle", "17"],
        ["date_naissance", "1975-05-01"], ["lieu_naissance_affichage", "Nantes"], ["departement_naissance", "44"],
        ["pays_naissance", "FR"], ["code_nationalite_dsn", "01"], ["adresse_affichage", "7 rue de la Paix"],
        ["code_postal", "44000"], ["commune_affichage", "Nantes"], ["email", "mariano.fortuny@exemple.invalid"],
        ["emploi_affichage", "Régisseur lumière"], ["statut", "technicien"], ["statut_categoriel_rc", "04"],
        ["pcs_ese", "465b"], ["code_emploi_ft", "REG130"], ["date_anciennete", "2015-06-15"],
        ["statut_conventionnel", "05"], ["jours_travailles", ["lundi", "mardi", "mercredi", "jeudi", "vendredi"]], ["actif", true]
    ]);
    let changements: Vec<Value> = cases.as_array().unwrap().iter().map(|c| json!({"case": c[0], "valeur": c[1]})).collect();
    let r = reussi(&route(&c, "enregistrer-fiche", json!({"nom": "salarie", "cle": null, "changements": changements}), JOUR)).clone();
    assert_eq!(r["cle"], json!("FORTUNY"), "{}", r);
    c
}

/// La ligne `id` d'une paie ecrite (une cotisation ou une rubrique de remuneration, par `cle`).
fn ligne<'p>(paie: &'p Value, liste: &str, cle: &str, id: &str) -> &'p Value {
    paie["paie"][liste].as_array().and_then(|l| l.iter().find(|x| x[cle] == id)).unwrap_or_else(|| panic!("pas de ligne {} : {}", id, paie))
}

fn montant(v: &Value) -> rust_decimal::Decimal {
    dec(v.as_str().unwrap_or_else(|| panic!("un montant : {}", v)))
}

/// En CDI depuis le 02/11/2026 a 2 400 € par mois pour 151,67 h, FORTUNY a, au 30/11/2026, 11 annees pleines de
/// presence depuis le 15/06/2015 : il est dans sa 12e annee, 12 - 3 = 9 % (« 4e année : 1 % », un point par annee, sous
/// le plafond de 10 %). Theatre jusqu'a 200 places, agent de maitrise : 2 185,56 x 9 % = 196,7004, soit 196,70 €, et un
/// brut de 2 400 + 196,70 = 2 596,70 €.
#[test]
fn la_prime_d_anciennete_d_un_technicien_au_mois_de_l_annexe_1() {
    let c = les_treteaux_et_leur_regisseur("prime_d_anciennete");
    let entreprise = json!([["annexe_convention", "1"], ["jauge_annexe_1", "<=200"], ["emploie_des_permanents", true], ["date_de_versement_permanents", "fin du mois"]]);
    let changements: Vec<Value> = entreprise.as_array().unwrap().iter().map(|c| json!({"case": c[0], "valeur": c[1]})).collect();
    reussi(&route(&c, "enregistrer-fiche", json!({"nom": "entreprise", "cle": null, "confirme": true, "changements": changements}), JOUR));
    // Un permanent de la 3090 est affilie a la complementaire sante (L. 911-7) : son niveau se lit sur son adhesion.
    let sante = json!([{"case": "sante_svp", "valeur": {"niveau": "1", "conjoint": false, "enfants": 0}}]);
    reussi(&route(&c, "enregistrer-fiche", json!({"nom": "salarie", "cle": "FORTUNY", "changements": sante}), JOUR));
    let cdi = json!({"salarie": "FORTUNY", "nature": "cdi", "debut": "2026-11-02", "salaire_mensuel": 2400, "quotite_contrat": nombre("151.67")});
    reussi(&valider(&c, "2026-11", None, &cdi, JOUR));

    let paie = paie_ecrite(&c, "2026-11", "FORTUNY", None);
    assert_eq!(montant(&ligne(&paie, "remuneration", "code", "prime_anciennete")["montant"]), dec("196.70"), "{}", paie);
    assert_eq!(montant(&paie["paie"]["brut"]), dec("2596.70"), "{}", paie);
}

/// Deux contrats d'usage de 10 heures, en 2025. Le seuil se compare au SMIC de reference des heures payees
/// (`droit::reduction::au_dela_du_seuil`) : 11,88 x 10 = 118,80 €, soit 267,30 € a 2,25 SMIC et 392,04 € a 3,3 SMIC.
/// - Mars, 10 h a 51,73 € : 517,30 €, au-dela des deux seuils. Maladie 517,30 x 13 % = 67,249, soit 67,25 € ;
///   allocations familiales 517,30 x 5,25 % = 27,15825, soit 27,16 €.
/// - Avril, 10 h a 30 € : 300 €, au-dela du seul seuil de la maladie. Maladie 300 x 13 % = 39,00 € ; allocations
///   familiales au taux reduit, 300 x 3,45 % = 10,35 €.
#[test]
fn le_complement_au_dela_du_seuil_du_bareme_2025() {
    let c = les_treteaux_et_leur_regisseur("complement_2025");
    // Le taux AT/MP de 2025 : celui que les Treteaux ont dit pour 2026, des le 01/01/2025.
    let atmp = json!([{"case": "taux_atmp", "valeur": {"code_risque": "923AD", "valeur": nombre("1.1"), "depuis": "2025-01-01"}}]);
    reussi(&route(&c, "enregistrer-fiche", json!({"nom": "entreprise", "cle": null, "changements": atmp}), JOUR));
    let heures = |mois: &str, jour: &str, taux: &str| -> Value {
        let taux: serde_json::Number = taux.parse().unwrap();
        let carte = json!({"salarie": "FORTUNY", "nature": "cddu_spectacle", "spectacle": "le-banc-des-menteurs",
            "lignes": [{"rubrique": "heure", "quantite": 10, "taux": taux, "lieu": null, "dates": [jour], "planning": []}]});
        reussi(&valider(&c, mois, None, &carte, JOUR));
        paie_ecrite(&c, mois, "FORTUNY", None)
    };
    let patronale = |paie: &Value, id: &str| (montant(&ligne(paie, "lignes", "id", id)["taux_pat"]), montant(&ligne(paie, "lignes", "id", id)["montant_pat"]));

    let mars = heures("2025-03", "2025-03-14", "51.73");
    assert_eq!(montant(&mars["paie"]["brut"]), dec("517.30"), "{}", mars);
    assert_eq!(patronale(&mars, "maladie"), (dec("13"), dec("67.25")), "{}", mars);
    assert_eq!(patronale(&mars, "famille"), (dec("5.25"), dec("27.16")), "{}", mars);

    let avril = heures("2025-04", "2025-04-11", "30.0");
    assert_eq!(montant(&avril["paie"]["brut"]), dec("300"), "{}", avril);
    assert_eq!(patronale(&avril, "maladie"), (dec("13"), dec("39.00")), "{}", avril);
    assert_eq!(patronale(&avril, "famille"), (dec("3.45"), dec("10.35")), "{}", avril);
}
