//! Le plafond d'un contrat, depuis le texte : arrete du 24/01/1975, art. 3, al. 2 (un artiste
//! engage moins de cinq jours continus : douze fois le plafond horaire par journee de travail accomplie) et, sinon,
//! CSS R. 242-2 (le plafond mensuel reduit a due proportion des jours du contrat). Hors de l'arrete, un
//! intermittent a le plafond proratise sur les jours ayant donne lieu a remuneration (BOSS, assiette generale, § 1090,
//! tolerance appliquee). Textes : atelier/reference/sources/arrete-1975-artistes-du-spectacle/,
//! code-de-la-securite-sociale/, boss/ (§ 680, 690, 700, 1090, 1100).
//!
//! Le moteur se confronte a un oracle ecrit depuis le texte (`oracle`, sans le moteur) sur une grille de 23 808 cas
//! (2025-2026, tous les mois, deux departs, durees 1 a 62 jours, 0, 1, 3 ou 6 cachets, artiste ou technicien), et a
//! `plafond-dates.json`, ecrit a la main depuis les memes textes.

use socle::grandeurs::{Jours, Montant, MontantUnitaire};
use droit::plafond::{plafond_urssaf, pour_paie, EngagementPlafond, Pss};
use socle::date::{Duree, Jour, Periode};
use socle::mois::{Annee, Mois};
use socle::echec::R;
use serde::Deserialize;
use std::collections::HashMap;
use crate::outils::typee::{chemin_donnee, code_livre, lire, lire_chemin};

/// Le montant qu'un exemple ecrit.
fn montant(t: &str) -> Montant {
    socle::grandeurs::lu::en_texte(t).unwrap()
}

/// `plafond-dates.json`, ecrit a la main depuis les textes : ses lignes sont des listes, lues par leur forme ; un
/// plafond attendu y est ecrit en texte (`montant`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cas {
    #[serde(rename = "_note")]
    _note: String,
    /// debut, fin, statut, journees remunerees, plafond attendu, ce que le cas montre.
    dates: Vec<(Jour, Jour, String, Vec<Jour>, String, String)>,
    /// debut, fin de la periode d'un permanent, plafond attendu.
    mensuel: Vec<(Jour, Jour, String)>,
    /// debut, fin, statut, ce que le cas montre : un contrat sans journee remuneree datee.
    sans_date: Vec<(Jour, Jour, String, String)>,
}

/// Du bareme livre, la seule section que ce test lit : le plafond de la securite sociale.
#[derive(Deserialize)]
struct BaremePss {
    plafond_securite_sociale: PssLu,
}

#[derive(Deserialize)]
struct PssLu {
    mensuel: Montant,
    horaire: MontantUnitaire,
}

/// Le plafond de chaque annee de la table.
fn pss_depuis_table(table: &HashMap<Annee, Pss>) -> impl Fn(Annee) -> R<Pss> + '_ {
    move |annee| table.get(&annee).copied().ok_or_else(|| socle::refus!("pas de plafond pour {}", annee))
}

/// Des jours dits.
fn jours(n: u32) -> Jours {
    Jours::fixes(n)
}

/// Le jour (an, mois, jour) que le test ecrit, fait par le socle.
macro_rules! jour_ecrit {
    ($an:expr, $mois:expr, $jour:expr) => {
        Jour::nouveau($an, $mois, $jour).unwrap()
    };
}

/// La periode du `d` au `f`.
fn du_au(d: Jour, f: Jour) -> Periode {
    Periode::du_au(d, f).unwrap()
}

/// Le PSS : la section `plafond_securite_sociale` du bareme de l'annee (`donnees/baremes/AAAA.json`).
fn table() -> HashMap<Annee, Pss> {
    let mut t = HashMap::new();
    for annee in [Annee::constante(2025), Annee::constante(2026)] {
        let b: BaremePss = lire(&code_livre(), &format!("baremes/{}.json", annee));
        t.insert(annee, Pss::en_euros_entiers(b.plafond_securite_sociale.mensuel, b.plafond_securite_sociale.horaire));
    }
    t
}

/// Le jour `n` jours apres `depuis`.
fn ajouter(depuis: Jour, n: u32) -> Jour {
    depuis.apres(Duree::jours(n)).unwrap()
}

/// Les `duree` premiers jours du contrat.
fn jours_du_contrat(debut: Jour, duree: u32) -> Vec<Jour> {
    (0..duree).map(|i| ajouter(debut, i)).collect()
}

/// L'oracle du texte, ecrit sans le moteur. `t` : la table du bareme.
fn oracle(t: &HashMap<Annee, Pss>, duree: u32, artiste: bool, journees: &[Jour]) -> Result<Montant, ()> {
    let pss = |a: Annee| t.get(&a).copied().ok_or(());
    if artiste && duree < 5 {
        if journees.is_empty() {
            return Err(());
        }
        // Arrete du 24/01/1975, art. 3, al. 2 : 12 x le plafond horaire par journee de travail accomplie.
        let mut total = Montant::ZERO;
        for x in journees {
            total += pss(x.annee())?.horaire().fois(12);
        }
        return Ok(total.au_centime());
    }
    // BOSS § 1090 (tolerance) et § 700 : le plafond mensuel x jours remuneres dans le mois / jours du mois, mois par
    // mois. Sans jour remunere, rien a compter : refus.
    if journees.is_empty() {
        return Err(());
    }
    let mut par_mois: std::collections::BTreeMap<Mois, usize> = Default::default();
    for x in journees.iter().collect::<std::collections::BTreeSet<_>>() {
        *par_mois.entry(x.mois_civil()).or_default() += 1;
    }
    let mut total = Montant::ZERO;
    for (m, n) in par_mois {
        let a = m.annee();
        let dim = match m.numero() {
            4 | 6 | 9 | 11 => 30,
            2 if a.bissextile() => 29,
            2 => 28,
            _ => 31,
        };
        total += pss(a)?.mensuel().fois(n).reparti_sur(dim);
    }
    Ok(total.au_centime())
}

#[test]
fn la_grille_depuis_le_texte() {
    let t = table();
    let pss = pss_depuis_table(&t);
    let mut cas = 0;
    for annee in [2025, 2026] {
        for mois in 1..=12u32 {
            for dep in [1, 20] {
                let debut = jour_ecrit!(annee, mois, dep);
                for duree in 1..=62u32 {
                    let fin = ajouter(debut, duree - 1);
                    for c in [0, 1, 3, 6] {
                        // Les journees : un cachet par jour depuis le debut, tant qu'il y a des jours.
                        let journees = jours_du_contrat(debut, c.min(duree));
                        for artiste in [true, false] {
                            cas += 1;
                            let voulu = oracle(&t, duree, artiste, &journees);
                            let lu = plafond_urssaf(&du_au(debut, fin), artiste, &pss, &journees);
                            match (&voulu, &lu) {
                                (Ok(v), Ok(l)) => assert_eq!(l, v, "{:?}->{:?} {} cachets, artiste {}", debut, fin, c, artiste),
                                (Err(()), Err(m)) if journees.is_empty() => {
                                    let regle = if artiste && duree < 5 { "arrete du 24/01/1975" } else { "BOSS, assiette generale, § 1090" };
                                    assert!(m.phrase.contains(regle), "{}", m.phrase)
                                }
                                (Err(()), Err(m)) => assert!(m.phrase.contains("2027"), "pas de plafond pour 2027 : {}", m.phrase),
                                _ => panic!("{:?}->{:?} {} cachets, artiste {} : texte {:?}, moteur {:?}", debut, fin, c, artiste, voulu, lu),
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cas, 2 * 12 * 2 * 62 * 4 * 2);
}

#[test]
fn les_dates_et_le_mensuel() {
    let c: Cas = lire_chemin(&chemin_donnee("plafond-dates.json"));
    let t = table();
    let pss = pss_depuis_table(&t);
    for (debut, fin, statut, journees, voulu, quoi) in &c.dates {
        let lu = plafond_urssaf(&du_au(*debut, *fin), statut == "artiste", &pss, journees).unwrap();
        assert_eq!(lu, montant(voulu), "{}", quoi);
    }
    for (debut, fin, voulu) in &c.mensuel {
        // Le contrat d'un permanent ne se lit pas : celui de sa periode.
        let e = EngagementPlafond {
            mensuelle: true,
            artiste: true,
            periode: du_au(*debut, *fin),
            contrat: du_au(*debut, *fin), dates_travail: vec![], jours: jours(0),
        };
        assert_eq!(pour_paie(&e, &pss).unwrap(), montant(voulu), "{:?} {:?}", debut, fin);
    }
    // pour_paie d'un intermittent = plafond_urssaf avec ses dates.
    let e = EngagementPlafond {
        mensuelle: false, artiste: true, periode: du_au(jour_ecrit!(2026, 5, 20), jour_ecrit!(2026, 5, 23)),
        contrat: du_au(jour_ecrit!(2026, 5, 20), jour_ecrit!(2026, 5, 23)),
        dates_travail: vec![jour_ecrit!(2026, 5, 20), jour_ecrit!(2026, 5, 22)], jours: jours(2),
    };
    assert_eq!(pour_paie(&e, &pss).unwrap(), montant("720.00"));
    assert_eq!(pour_paie(&e, &pss).unwrap(), plafond_urssaf(&e.contrat, true, &pss, &e.dates_travail).unwrap());
    // Une journee dite (`jours`) et non datee refuse ; aucune journee dans la paie, aucun plafond.
    let err = pour_paie(&EngagementPlafond { jours: jours(3), ..e.clone() }, &pss).unwrap_err().phrase;
    assert!(err.contains("3 jour(s) travaille(s) pour 2 date(s)"), "{}", err);
    assert_eq!(pour_paie(&EngagementPlafond { jours: jours(0), dates_travail: vec![], ..e.clone() }, &pss).unwrap(), Montant::ZERO);
    // Un technicien : le prorata de ses jours remuneres (BOSS § 1090), 4005 x 2 / 31.
    assert_eq!(pour_paie(&EngagementPlafond { artiste: false, ..e.clone() }, &pss).unwrap(), montant("258.39"));
    // Une paie de janvier d'un contrat du 20/01 au 10/02/2026 ne compte que ses jours (R. 242-2, « pour
    // chaque paie ») ; une date hors de la periode de paie refuse.
    let janvier = EngagementPlafond {
        mensuelle: false, artiste: true, periode: du_au(jour_ecrit!(2026, 1, 20), jour_ecrit!(2026, 1, 31)),
        contrat: du_au(jour_ecrit!(2026, 1, 20), jour_ecrit!(2026, 2, 10)), dates_travail: vec![jour_ecrit!(2026, 1, 20), jour_ecrit!(2026, 1, 22), jour_ecrit!(2026, 1, 24)], jours: jours(3),
    };
    assert_eq!(pour_paie(&janvier, &pss).unwrap(), montant("387.58"));
    let err = pour_paie(&EngagementPlafond { dates_travail: vec![jour_ecrit!(2026, 1, 20), jour_ecrit!(2026, 1, 22), jour_ecrit!(2026, 2, 3)], ..janvier }, &pss).unwrap_err().phrase;
    assert!(err.contains("hors de la periode de paie"), "{}", err);
    // Sans jour remunere date, rien ne se compte : refus.
    for (debut, fin, statut, quoi) in &c.sans_date {
        let err = plafond_urssaf(&du_au(*debut, *fin), statut == "artiste", &pss, &[]).unwrap_err().phrase;
        assert!(err.contains("0 jour(s) travaille(s) pour 0 date(s)"), "{} : {}", quoi, err);
    }
}

#[test]
fn bornes_incluses_et_exemples_des_bulletins() {
    let t = table();
    let pss = pss_depuis_table(&t);
    // FAVART 24/05/2026 : un jour, une journee -> 30 x 12 = 360.
    let r = plafond_urssaf(&Periode::du_jour(jour_ecrit!(2026, 5, 24)), true, &pss, &[jour_ecrit!(2026, 5, 24)]).unwrap();
    assert_eq!(r, Montant::euros(360));
    // FAVART 13-22/05/2026 : dix jours, un jour remunere, 4005 / 31 (BOSS § 1090).
    let r = plafond_urssaf(&du_au(jour_ecrit!(2026, 5, 13), jour_ecrit!(2026, 5, 22)), true, &pss, &[jour_ecrit!(2026, 5, 15)]).unwrap();
    assert_eq!(r, montant("129.19"));
}
