//! Tests du champ `largeur_max` de `texte_fixe` (FORMAT.md) : le corps est réduit pour que le texte
//! tienne : le corps d'origine tant que le texte y tient, sinon `corps × place ÷ largeur`, jamais sous 6 pt.

use fabrique_pdf::mesure::{mesure, Mesure};
use lopdf::content::Content;
use lopdf::Document;
use serde_json::{json, Value};

fn description(blocs: Vec<Value>) -> Value {
    json!({
        "format": 2,
        "page": {"largeur": 400, "hauteur": 500,
                 "marges": {"haut": 0, "droite": 0, "bas": 0, "gauche": 0}},
        "police": {"taille": 10, "interligne": mesure("1.25"), "couleur": "#000000"},
        "documents": [{"blocs": blocs}]
    })
}

fn texte_fixe(texte: &str, taille: Mesure, extra: Value) -> Value {
    let mut b = json!({"type": "texte_fixe", "texte": texte, "x": 20, "y": 100, "taille": taille});
    for (k, v) in extra.as_object().unwrap() {
        b[k] = v.clone();
    }
    b
}

/// `n` points.
fn pt(n: i64) -> Mesure {
    Mesure::entier(n)
}

/// Les corps (opérateur `Tf`) des textes de la première page, dans l'ordre.
fn corps_poses(pdf: &[u8]) -> Vec<Mesure> {
    let doc = Document::load_mem(pdf).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let flux = doc.get_page_content(page).unwrap();
    Content::decode(&flux)
        .unwrap()
        .operations
        .iter()
        .filter(|o| o.operator == "Tf")
        .map(|o| Mesure::de_l_objet(&o.operands[1]).unwrap_or_else(|| panic!("corps inattendu {:?}", o.operands[1])))
        .collect()
}

/// La largeur d'un texte en points, d'après les avances de Liberation (les métriques d'Helvetica,
/// de Times ou de Courier) — ce que le `get_text_length` de PyMuPDF donne pour la police standard.
fn largeur(fichier: &str, texte: &str, corps: Mesure) -> Mesure {
    let donnees = std::fs::read(format!("{}/polices/{}", env!("CARGO_MANIFEST_DIR"), fichier)).unwrap();
    let face = ttf_parser::Face::parse(&donnees, 0).unwrap();
    let unites: i64 = texte.chars().map(|c| face.glyph_hor_advance(face.glyph_index(c).unwrap()).unwrap() as i64).sum();
    corps.fois_rapport(unites, face.units_per_em() as i64).unwrap()
}

/// Le corps qui tient, recalculé à part du code de la fabrique.
fn corps_qui_tient(fichier: &str, texte: &str, corps: Mesure, place: Mesure) -> Mesure {
    let l = largeur(fichier, texte, corps);
    if l <= place {
        corps
    } else {
        corps.fois_rapport(place.en_millioniemes(), l.en_millioniemes()).unwrap().max(pt(6))
    }
}

/// `a` vaut `b` à `tolerance` près.
fn proche(a: Mesure, b: Mesure, tolerance: &str) -> bool {
    (a - b).abs() < mesure(tolerance)
}

fn fabriquer(blocs: Vec<Value>) -> Vec<u8> {
    lue(description(blocs)).unwrap_or_else(|e| panic!("{}", e.phrase))
}

/// La description en texte, lue par la porte puis fabriquée.
fn lue(v: Value) -> Result<Vec<u8>, fabrique_pdf::Refus> {
    fabrique_pdf::Description::lire(&v.to_string()).and_then(|d| fabrique_pdf::fabriquer(&d))
}

#[test]
fn le_corps_demande_est_gardé_quand_le_texte_tient() {
    let t = "Société Fictive";
    let corps = mesure("13.92");
    let l = largeur("LiberationSans-Regular.ttf", t, corps);
    let pdf = fabriquer(vec![
        texte_fixe(t, corps, json!({"largeur_max": l + mesure("0.5")})),
        texte_fixe(t, corps, json!({})),
    ]);
    let c = corps_poses(&pdf);
    assert_eq!(c.len(), 2);
    assert!(proche(c[0], corps, "0.001") && proche(c[1], corps, "0.001"), "{:?}", c);
}

#[test]
fn le_corps_est_reduit_au_prorata_de_la_place() {
    let t = "COMPAGNIE FICTIVE DES ESSAIS RÉUNIS";
    for (place, corps) in [(pt(200), mesure("13.92")), (pt(150), pt(10)), (pt(180), pt(12))] {
        let attendu = corps_qui_tient("LiberationSans-Regular.ttf", t, corps, place);
        assert!(attendu < corps && attendu > pt(6), "le cas ne réduit pas : {}", attendu);
        let pdf = fabriquer(vec![texte_fixe(t, corps, json!({"largeur_max": place}))]);
        let c = corps_poses(&pdf);
        assert!(proche(c[0], attendu, "0.01"), "place {} : corps {} au lieu de {}", place, c[0], attendu);
        // et le texte réduit tient dans la place
        assert!(largeur("LiberationSans-Regular.ttf", t, c[0]) <= place + mesure("0.05"));
    }
}

#[test]
fn le_corps_ne_descend_pas_sous_six_points() {
    let t = "UN TEXTE BIEN TROP LONG POUR UNE PLACE SI ÉTROITE";
    let pdf = fabriquer(vec![texte_fixe(t, pt(12), json!({"largeur_max": 20}))]);
    let c = corps_poses(&pdf);
    assert!(proche(c[0], pt(6), "0.001"), "{:?}", c);
    // Comme dans corps_qui_tient : le plancher vaut aussi pour un corps demandé plus petit.
    assert_eq!(corps_qui_tient("LiberationSans-Regular.ttf", t, pt(5), pt(20)), pt(6));
    let pdf = fabriquer(vec![texte_fixe(t, pt(5), json!({"largeur_max": 20}))]);
    assert!(proche(corps_poses(&pdf)[0], pt(6), "0.001"));
}

#[test]
fn la_mesure_suit_la_police_et_le_gras() {
    let t = "Raison sociale fictive et trop longue";
    for (police, gras, fichier) in [
        ("times", false, "LiberationSerif-Regular.ttf"),
        ("times", true, "LiberationSerif-Bold.ttf"),
        ("courier", false, "LiberationMono-Regular.ttf"),
        ("helvetica", true, "LiberationSans-Bold.ttf"),
    ] {
        let attendu = corps_qui_tient(fichier, t, pt(11), pt(90));
        assert!(attendu < pt(11), "{} : le cas ne réduit pas", fichier);
        let pdf = fabriquer(vec![texte_fixe(t, pt(11), json!({"police": police, "gras": gras, "largeur_max": 90}))]);
        let c = corps_poses(&pdf);
        assert!(proche(c[0], attendu, "0.01"), "{} : {} au lieu de {}", fichier, c[0], attendu);
    }
}

#[test]
fn largeur_max_ne_change_ni_l_alignement_ni_les_autres_blocs() {
    // Le texte réduit s'aligne sur sa largeur réduite (bord droit en x) ; un bloc voisin sans
    // largeur_max garde son corps.
    let pdf = fabriquer(vec![
        texte_fixe("TEXTE ASSEZ LONG POUR DEBORDER", pt(12), json!({"alignement": "droite", "x": 300, "largeur_max": 60})),
        texte_fixe("VOISIN", pt(12), json!({"y": 150})),
    ]);
    let c = corps_poses(&pdf);
    assert_eq!(c.len(), 2);
    assert!(c[0] < pt(12) && proche(c[1], pt(12), "0.001"), "{:?}", c);
}

#[test]
fn une_largeur_max_nulle_ou_negative_est_refusee() {
    for v in [0, -5] {
        let e = lue(description(vec![texte_fixe("x", pt(10), json!({"largeur_max": v}))])).expect_err("refus attendu");
        assert!(e.phrase.contains("largeur_max") && !e.phrase.contains('\n'), "{:?}", e.phrase);
    }
}
