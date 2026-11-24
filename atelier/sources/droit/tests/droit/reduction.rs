//! La reduction generale d'un technicien declare a la caisse des Conges Spectacles (`droit::reduction`). Textes :
//! CSS D. 241-7, II (`atelier/reference/sources/code-de-la-securite-sociale/xml/D241-7.xml` : Tmin 0,0200, Tdelta
//! 0,3781, P 1,75, le SMIC « applicable au 1er janvier 2026 », le resultat « arrondi à quatre décimales ») ; D. 241-10,
//! I et IV (`atelier/reference/sources/code-de-la-securite-sociale/xml/D241-10.xml`, LEGIARTI000054252223) : « Coefficient = (Tmin +
//! (Tdelta x [...]P)) x b », b = 100/90 pour les professions dont les conges sont payes par une caisse.

use crate::outils::typee::{code_livre, lire};
use droit::reduction::reduction_generale;
use serde::Deserialize;
use socle::grandeurs::{Coefficient, Heures, Montant};

/// Du bareme livre, la seule partie que ce test lit.
#[derive(Deserialize)]
struct Bareme {
    par_statut: ParStatut,
}

#[derive(Deserialize)]
struct ParStatut {
    technicien: Technicien,
}

#[derive(Deserialize)]
struct Technicien {
    reduction_generale: droit::forme_bareme::ParametresDeLaReduction,
}

/// La grandeur qu'un exemple ecrit.
fn lue<G: socle::grandeurs::Grandeur>(t: &str) -> G {
    socle::grandeurs::lu::en_texte(t).unwrap()
}

#[test]
fn le_coefficient_majore_ne_s_arrondit_qu_une_fois() {
    let bareme: Bareme = lire(&code_livre(), "baremes/2026.json");
    let technicien = Some(&bareme.par_statut.technicien.reduction_generale);
    // Une journee de 7 h payee 100 : SMIC de reference 12,02 x 7 = 84,14 ; (0,02 + 0,3781 x 0,7621^1,75) = 0,25503...
    let (brut, heures): (Montant, Heures) = (lue("100"), lue("7"));
    let sans = reduction_generale(technicien, Some(droit::fiche::Statut::Technicien), brut, heures, false).unwrap().unwrap();
    assert_eq!(sans.coefficient, lue::<Coefficient>("0.2550"));
    // ... x 100/90 = 0,28337... -> 0,2834 ; arrondir avant la majoration donnerait 0,2550 x 100/90 -> 0,2833.
    let avec = reduction_generale(technicien, Some(droit::fiche::Statut::Technicien), brut, heures, true).unwrap().unwrap();
    assert_eq!(avec.coefficient, lue::<Coefficient>("0.2834"));
    assert_eq!(avec.montant, lue::<Montant>("28.34"));
}

/// LA REDUCTION SOUS DEDUCTION FORFAITAIRE SPECIFIQUE, PLAFONNEE A 130 % DE CELLE DU BRUT SEUL : BOSS, Allegements
/// generaux (`atelier/reference/sources/boss/boss-allegements-generaux-20261123.txt`), § 190 (l. 241-252 : avec la
/// deduction, « la rémunération intègre les sommes versées au titre du remboursement de frais professionnels ») et § 350
/// (l. 523-560 : « plafonné à 130 % du montant qui aurait été calculé si cette déduction n'avait pas été prise en
/// compte », les frais restant alors dehors). Ses deux exemples : 70 salaries, temps plein, DFS de 20 %, 2 200 € de brut
/// et 200 € puis 100 € de frais, le SMIC de 1 823,03 € qu'ils citent (12,02 x 151,6667 h). Leurs coefficients (24,43 %,
/// 34,87 %, 38,82 %) supposent un Tdelta d'environ 0,3773, que le tableau du meme BOSS (§ 440, l. 677-680) ne donne pas :
/// recalcules ici avec le sien, 0,3821 pour 50 salaries et plus, arrondis a quatre decimales (§ 380).
/// - sans la deduction, sur 2 200 € : (3 x 1 823,03 / 2 200 - 1) / 2 = 0,742975 ; puissance 1,75 : 0,594573 ;
///   0,02 + 0,3821 x 0,594573 = 0,2472 ; 2 200 x 0,2472 = 543,84 ; 130 % : 706,99 (le BOSS : 537,53, 698,79) ;
/// - 200 € de frais : (2 200 + 200) x 0,8 = 1 920 ; 0,924242 ; 0,871215 ; 0,3529 ; 677,57 sous 706,99 : la reduction
///   reste entiere (le BOSS : 669,52 sous 698,79) ;
/// - 100 € de frais : (2 200 + 100) x 0,8 = 1 840 ; 0,986166 ; 0,975916 ; 0,3929 ; 722,94 au-dessus de 706,99 : elle est
///   ramenee a 706,99 (le BOSS : 714,32 ramenee a 698,79).
#[test]
fn sous_dfs_la_reduction_se_limite_a_130_pourcent_de_celle_du_brut_sans_les_frais() {
    let bareme: Bareme = lire(&code_livre(), "baremes/2026.json");
    let mut parametres = bareme.par_statut.technicien.reduction_generale.clone();
    parametres.tdelta = Some(lue("0.3821"));
    let technicien = Some(droit::fiche::Statut::Technicien);
    let heures: Heures = lue("151.6667");
    let (brut, limite): (Montant, socle::grandeurs::Taux) = (lue("2200"), lue("130"));
    let sous_dfs = |abattu: &str| droit::reduction::reduction_generale_sous_dfs(Some(&parametres), limite, technicien, brut, lue(abattu), heures, false).unwrap().unwrap();
    let sans = reduction_generale(Some(&parametres), technicien, brut, heures, false).unwrap().unwrap();
    assert_eq!((sans.smic_reference, sans.coefficient, sans.montant), (lue("1823.03"), lue("0.2472"), lue("543.84")));
    let deux_cents = sous_dfs("1920");
    assert_eq!((deux_cents.coefficient, deux_cents.montant), (lue("0.3529"), lue::<Montant>("677.57")));
    let cent = sous_dfs("1840");
    assert_eq!((cent.coefficient, cent.montant), (lue("0.3929"), lue::<Montant>("706.99")));
}
