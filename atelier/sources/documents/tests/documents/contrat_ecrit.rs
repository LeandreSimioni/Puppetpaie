//! LES MENTIONS QUE LA LOI EXIGE D'UN CONTRAT ECRIT : le CDD « est établi par écrit » avec ses mentions (code du travail
//! L. 1242-12, `atelier/reference/sources/code-du-travail/xml/L1242-12.xml`), tout salarié reçoit par écrit les
//! informations de R. 1221-34 (`xml/R1221-34.xml`). Chaque modèle livré se lit (chacun de ses marqueurs est connu) et
//! porte ces mentions ; un modèle qui en perd une est refusé, et le refus nomme la mention et son texte.

use std::path::Path;

use documents::contrat_ecrit::manques;
use documents::marqueurs::{ModeleLu, Valeurs};
use droit::marqueurs::Marqueur;

const MODELES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../logiciel/donnees/modeles");

fn texte(nom: &str) -> String {
    std::fs::read_to_string(Path::new(MODELES).join(nom)).unwrap()
}

#[test]
fn chaque_modele_livre_se_lit_et_porte_les_mentions_de_l1242_12_et_r1221_34() {
    for f in std::fs::read_dir(MODELES).unwrap() {
        let p = f.unwrap().path();
        if p.extension().is_some_and(|e| e == "modele") {
            ModeleLu::du_fichier(&p).unwrap_or_else(|r| panic!("{} : {}", p.display(), r.phrase));
        }
    }
    for nature in droit::natures::NatureDeContrat::TOUTES {
        let nat = nature.proprietes().unwrap();
        let nom = nat.modele_contrat().unwrap();
        assert!(nat.mentions_obligatoires().unwrap().len() >= 15, "{} : ses mentions", nature);
        let dits = manques(&nat, &texte(nom.texte()), None).unwrap();
        assert!(dits.is_empty(), "{} : {:?}", nom, dits);
    }
}

#[test]
fn un_modele_de_cdd_sans_son_motif_ni_le_terme_est_refuse() {
    let nat = droit::natures::NatureDeContrat::Cdd.proprietes().unwrap();
    let t = texte("contrat-cdd.modele").replace("{{contrat.motif}}", "un surcroît").replace("{{contrat.fin}}", "la fin du projet");
    let dits = manques(&nat, &t, None).unwrap().join(" ");
    assert!(dits.contains("{{contrat.motif}}") && dits.contains("L. 1242-12, premier alinéa"), "{}", dits);
    assert!(dits.contains("{{contrat.fin}}") && dits.contains("L. 1242-12, 2°"), "{}", dits);
}

#[test]
fn la_personne_remplacee_n_est_due_que_pour_un_remplacement() {
    let nat = droit::natures::NatureDeContrat::Cdd.proprietes().unwrap();
    let t = texte("contrat-cdd.modele").replace("{{contrat.personne_remplacee}}", "");
    assert!(manques(&nat, &t, Some(&Valeurs::new())).unwrap().is_empty());
    let avec = Valeurs::from([(Marqueur::ContratPersonneRemplacee, "Mme X, chargée de production".to_string())]);
    let dits = manques(&nat, &t, Some(&avec)).unwrap().join(" ");
    assert!(dits.contains("L. 1242-12, 1°"), "{}", dits);
}
