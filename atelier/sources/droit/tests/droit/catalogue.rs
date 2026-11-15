//! Le catalogue des rubriques de chaque racine de `tests/donnees/racines/` (ce que la compagnie ajuste et ajoute ; ses
//! rubriques datees, lues a la date ; ses refus).

use crate::outils::typee::chemin_donnee;
use droit::catalogue::{catalogue, rubrique};
use socle::date::Jour;

fn jour(an: i32, mois: u32, j: u32) -> Jour {
    Jour::nouveau(an, mois, j).unwrap()
}

#[test]
fn les_catalogues_et_les_refus() {
    // Une rubrique de la compagnie changee a une date : la nouvelle valeur a partir d'elle, l'ancienne avant.
    let a = chemin_donnee("racines").join("a");
    let libelle = |j: Jour| rubrique(&a, &droit::catalogue::CACHET, &j).unwrap().libelle;
    assert_eq!((libelle(jour(2026, 12, 31)), libelle(jour(2027, 1, 1))), ("Cachet maison".to_string(), "Cachet maison 2027".to_string()));
    // Un catalogue de compagnie mal decrit se refuse, sans rien remplir a la place.
    let mut refus = 0;
    for entree in std::fs::read_dir(chemin_donnee("racines")).unwrap() {
        let entree = entree.unwrap();
        if entree.file_name().to_string_lossy().starts_with("refus_") {
            assert!(catalogue(&entree.path(), &jour(2026, 10, 28)).is_err(), "{:?}", entree.file_name());
            refus += 1;
        }
    }
    assert_eq!(refus, 9);
}
