//! L'allocation journaliere d'un intermittent (annexes VIII et X, art. 14), sur l'exemple 6 du guide de France Travail
//! (`atelier/reference/sources/chomage-intermittents/francetravail-guide-intermittents-20261020.txt`) : un technicien de
//! l'annexe 8, 800 heures et 18 000 € sur 12 mois ; « 39,80 + 12,20 + 12,78 = 64,78 € », chaque partie tronquee au
//! centime.

use droit::allocation::{allocation, Diviseurs};
use droit::emplois::AnnexeChomage;
use socle::grandeurs::lu::en_texte;
use socle::grandeurs::{Heures, Montant};

#[test]
fn l_exemple_6_du_guide_donne_64_78_euros() {
    let t = allocation().unwrap();
    let sr: Montant = en_texte("18000").unwrap();
    let nht: Heures = en_texte("800").unwrap();
    let plafond: Montant = en_texte("1000").unwrap();
    let e = t.estimer(AnnexeChomage::Viii, sr, nht, plafond, Diviseurs::DeLAnnexe).unwrap();
    let m = |x: &str| -> Montant { en_texte(x).unwrap() };
    assert_eq!((e.a, e.b, e.c, e.retenue), (m("39.80"), m("12.20"), m("12.78"), m("64.78")));
}
