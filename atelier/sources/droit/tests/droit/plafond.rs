//! Le plafond d'un contrat, depuis le texte : arrete du 24/01/1975, art. 3, al. 2 (un artiste
//! engage moins de cinq jours continus : douze fois le plafond horaire par journee de travail accomplie) et, sinon,
//! CSS R. 242-2, I, al. 3 (le plafond mensuel reduit a due proportion des jours calendaires du contrat, quels que soient
//! les jours travailles ; BOSS, assiette generale, § 700 et 740 ; § 1090 : l'artiste n'est pas eligible a la reduction
//! du plafond). Textes : atelier/reference/sources/arrete-1975-artistes-du-spectacle/, code-de-la-securite-sociale/,
//! boss/ (§ 680, 690, 700, 740, 1090, 1100).
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
use crate::outils::le_jour as j;
use crate::outils::l_annee;
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
    /// debut, fin, statut, journees de travail, plafond attendu, ce que le cas montre.
    dates: Vec<(Jour, Jour, String, Vec<Jour>, String, String)>,
    /// debut, fin de la periode d'un permanent, plafond attendu.
    mensuel: Vec<(Jour, Jour, String)>,
    /// debut, fin, statut, ce que le cas montre : un artiste engage moins de cinq jours, sans journee datee.
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

/// La periode du `d` au `f`.
fn du_au(d: Jour, f: Jour) -> Periode {
    Periode::du_au(d, f).unwrap()
}

/// Le PSS : la section `plafond_securite_sociale` du bareme de l'annee (`donnees/baremes/AAAA.json`).
fn table() -> HashMap<Annee, Pss> {
    let mut t = HashMap::new();
    for annee in [l_annee("2025"), l_annee("2026")] {
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

/// L'oracle du texte, ecrit sans le moteur. `t` : la table du bareme ; `debut` et `duree` : le contrat.
fn oracle(t: &HashMap<Annee, Pss>, debut: Jour, duree: u32, artiste: bool, journees: &[Jour]) -> Result<Montant, ()> {
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
    // CSS R. 242-2, I, al. 3 et BOSS § 700 : le plafond mensuel x jours calendaires du contrat dans le mois / jours du
    // mois, mois par mois, quels que soient les jours travailles (BOSS § 1090 : l'artiste n'est pas eligible a la
    // reduction du plafond).
    let mut par_mois: std::collections::BTreeMap<Mois, usize> = Default::default();
    for x in jours_du_contrat(debut, duree) {
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
    for annee in [l_annee("2025"), l_annee("2026")] {
        for mois in annee.les_mois() {
            // Le 1er et le 20 du mois.
            for debut in [mois.premier_jour(), ajouter(mois.premier_jour(), 19)] {
                for duree in 1..=62u32 {
                    let fin = ajouter(debut, duree - 1);
                    for c in [0, 1, 3, 6] {
                        // Les journees : un cachet par jour depuis le debut, tant qu'il y a des jours.
                        let journees = jours_du_contrat(debut, c.min(duree));
                        for artiste in [true, false] {
                            cas += 1;
                            let voulu = oracle(&t, debut, duree, artiste, &journees);
                            let lu = plafond_urssaf(&du_au(debut, fin), artiste, &pss, &journees);
                            match (&voulu, &lu) {
                                (Ok(v), Ok(l)) => assert_eq!(l, v, "{:?}->{:?} {} cachets, artiste {}", debut, fin, c, artiste),
                                (Err(()), Err(m)) if journees.is_empty() && artiste && duree < 5 => {
                                    assert!(m.phrase.contains("arrete du 24/01/1975"), "{}", m.phrase)
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
        // Le plafond d'un permanent, sans absence ni temps partiel : le prorata calendaire de sa periode.
        let lu = droit::plafond::plafond_de_la_periode(&du_au(*debut, *fin), &droit::plafond::JoursSansRemuneration::aucun(), None, &pss).unwrap();
        assert_eq!(lu, montant(voulu), "{:?} {:?}", debut, fin);
    }
    // pour_paie d'un artiste engage moins de cinq jours = plafond_urssaf avec ses dates.
    let e = EngagementPlafond {
        artiste: true, periode: du_au(j("2026-05-20"), j("2026-05-23")),
        contrat: du_au(j("2026-05-20"), j("2026-05-23")),
        dates_travail: vec![j("2026-05-20"), j("2026-05-22")], jours: jours(2),
    };
    assert_eq!(pour_paie(&e, &pss).unwrap(), montant("720.00"));
    assert_eq!(pour_paie(&e, &pss).unwrap(), plafond_urssaf(&e.contrat, true, &pss, &e.dates_travail).unwrap());
    // Une journee dite (`jours`) et non datee refuse ; aucune journee dans la paie, aucun plafond.
    let err = pour_paie(&EngagementPlafond { jours: jours(3), ..e.clone() }, &pss).unwrap_err().phrase;
    assert!(err.contains("3 jour(s) travaille(s) pour 2 date(s)"), "{}", err);
    assert_eq!(pour_paie(&EngagementPlafond { jours: jours(0), dates_travail: vec![], ..e.clone() }, &pss).unwrap(), Montant::ZERO);
    // Une date hors de la periode de paie refuse : contrat du 30/01 au 02/02/2026, paie de janvier.
    let a_cheval = EngagementPlafond {
        artiste: true, periode: du_au(j("2026-01-30"), j("2026-01-31")), contrat: du_au(j("2026-01-30"), j("2026-02-02")),
        dates_travail: vec![j("2026-01-30"), j("2026-02-02")], jours: jours(2),
    };
    let err = pour_paie(&a_cheval, &pss).unwrap_err().phrase;
    assert!(err.contains("hors de la periode de paie"), "{}", err);
    // Un technicien : le prorata calendaire de son contrat, 4005 x 4 / 31, quels que soient ses jours travailles.
    assert_eq!(pour_paie(&EngagementPlafond { artiste: false, ..e.clone() }, &pss).unwrap(), montant("516.77"));
    // Un contrat du 20/01 au 10/02/2026 : la paie de janvier a les 12 jours du contrat en janvier, 4005 x 12 / 31,
    // celle de fevrier les 10 jours de fevrier, 4005 x 10 / 28 (R. 242-2, « pour chaque paie »). Ni les jours dits ni
    // les dates n'y comptent : sans journee datee, ou avec des jours dits sans date, le plafond est le meme.
    let janvier = EngagementPlafond {
        artiste: true, periode: du_au(j("2026-01-20"), j("2026-01-31")),
        contrat: du_au(j("2026-01-20"), j("2026-02-10")), dates_travail: vec![j("2026-01-20"), j("2026-01-22"), j("2026-01-24")], jours: jours(3),
    };
    assert_eq!(pour_paie(&janvier, &pss).unwrap(), montant("1550.32"));
    assert_eq!(pour_paie(&EngagementPlafond { dates_travail: vec![], ..janvier.clone() }, &pss).unwrap(), montant("1550.32"));
    let fevrier = EngagementPlafond { periode: du_au(j("2026-02-01"), j("2026-02-10")), dates_travail: vec![], jours: jours(0), ..janvier };
    assert_eq!(pour_paie(&fevrier, &pss).unwrap(), montant("1430.36"));
    // Un artiste engage moins de cinq jours sans journee datee : l'arrete de 1975 les compte, refus.
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
    let r = plafond_urssaf(&Periode::du_jour(j("2026-05-24")), true, &pss, &[j("2026-05-24")]).unwrap();
    assert_eq!(r, Montant::euros(360));
    // FAVART 13-22/05/2026 : dix jours, un jour travaille, 4005 x 10 / 31 (R. 242-2, I, al. 3).
    let r = plafond_urssaf(&du_au(j("2026-05-13"), j("2026-05-22")), true, &pss, &[j("2026-05-15")]).unwrap();
    assert_eq!(r, montant("1291.94"));
}

/// LA REGULARISATION SUR L'ANNEE, UN CUMUL PAR ASSIETTE (CSS R. 242-2, II ; BOSS, assiette generale, § 1220-1230 et
/// § 290 : `boss/boss-assiette-generale-20260801.txt`, l. 352-360 et 1697-1717). Chaque paie de l'annee, dans l'ordre :
/// ses assiettes regularisees calculees sur ce qui la precede, puis ajoutees a l'annee telles qu'elle les applique, dans
/// les assiettes qu'elle porte. Les plafonds de 2026 (4 005 par mois ; 5 jours d'un contrat de janvier, 4 005 x 5 / 31 =
/// 645,97 ; 10 jours de mai, 1 291,94) et les bornes du bareme livre (huit plafonds pour la tranche 2, quatre pour la
/// tranche 2 conventionnelle, l'Apec et l'abattement de la CSG). Tout recalcule a la main.
#[test]
fn la_regularisation_cumule_chaque_assiette_sur_sa_base_et_ses_paies() {
    use droit::plafond::{assiettes_plafonnees, AnneeAnterieure, Plafonnement, PorteesParLaPaie, Regularisees, RemunerationsRegularisees};
    let quatre = socle::grandeurs::Coefficient::entier_fixe(4);
    let tranches = droit::forme_bareme::TranchesDuPlafond {
        t2: socle::grandeurs::Coefficient::entier_fixe(8),
        t2_conventionnelle: quatre,
        chomage: quatre,
        apec: quatre,
        source: None,
        note: None,
    };
    // Une paie : ses remunerations, son plafond, les assiettes qu'elle porte ; rend ses assiettes regularisees et les
    // ajoute a l'annee (une paie d'intermittent n'applique que la tranche 1 et la part abattue de la CSG).
    let payer = |annee: &mut AnneeAnterieure, cotisations: &str, brut: &str, plafond: &str, portees: PorteesParLaPaie| -> Regularisees {
        let r = RemunerationsRegularisees { cotisations: montant(cotisations), brut: montant(brut) };
        let a = assiettes_plafonnees(r, montant(plafond), quatre, &tranches, &Plafonnement::Regularise(*annee)).regularisees;
        let appliquees = match portees {
            PorteesParLaPaie::Toutes => a,
            PorteesParLaPaie::SecuriteSociale => Regularisees { t1: a.t1, abattue_csg: a.abattue_csg, ..Regularisees::ZERO },
        };
        annee.ajouter(portees, r, montant(plafond), appliquees);
        a
    };
    let intermittent = PorteesParLaPaie::SecuriteSociale;

    // 1. Un technicien, deux contrats : A (5 jours, 645,97, 1 000 €), B (10 jours, 1 291,94, 500 €). Contrat par
    // contrat : 645,97 + 500 = 1 145,97 en vieillesse plafonnee ; regularise : 1 500, sous 1 937,91.
    let mut annee = AnneeAnterieure::debut();
    let a = payer(&mut annee, "1000", "1000", "645.97", intermittent);
    assert_eq!((a.t1, a.abattue_csg), (montant("645.97"), montant("1000")));
    let b = payer(&mut annee, "500", "500", "1291.94", intermittent);
    // 1 500 - 645,97 ; la CSG : 1 500 sous 4 x 1 937,91, moins 1 000.
    assert_eq!((b.t1, b.abattue_csg), (montant("854.03"), montant("500")));

    // 2. Le meme technicien sous deduction forfaitaire specifique (regisseur, 16 % en 2026) : la vieillesse sur le brut
    // abattu (840, puis 420), la CSG sur le brut entier. Contrat par contrat : 645,97 + 420 = 1 065,97 ; regularise :
    // 1 260 (840 + 420, sous 1 937,91), et la CSG abat 1 000 puis 500.
    let mut annee = AnneeAnterieure::debut();
    let a = payer(&mut annee, "840", "1000", "645.97", intermittent);
    assert_eq!((a.t1, a.abattue_csg), (montant("645.97"), montant("1000")));
    let b = payer(&mut annee, "420", "500", "1291.94", intermittent);
    // 1 260 - 645,97 : plus que le brut abattu de B, il rattrape A.
    assert_eq!((b.t1, b.abattue_csg), (montant("614.03"), montant("500")));

    // 3. Un CDI (janvier : 3 000 sur 4 005 ; mars : 4 700 sur 4 005) et un contrat de technicien en fevrier (5 jours,
    // 645,97, 1 000 €) : une seule masse pour la tranche 1 et la CSG ; la tranche 2 et la CET du CDI ne cumulent que ses
    // paies.
    let mut annee = AnneeAnterieure::debut();
    let janvier = payer(&mut annee, "3000", "3000", "4005", PorteesParLaPaie::Toutes);
    assert_eq!((janvier.t1, janvier.t2, janvier.cet), (montant("3000"), Montant::ZERO, Montant::ZERO));
    // Le technicien prend le plafond que le CDI n'a pas use : 4 000 sous 4 650,97, moins 3 000 (645,97 contrat seul).
    let fevrier = payer(&mut annee, "1000", "1000", "645.97", intermittent);
    assert_eq!((fevrier.t1, fevrier.abattue_csg), (montant("1000"), montant("1000")));
    let mars = payer(&mut annee, "4700", "4700", "4005", PorteesParLaPaie::Toutes);
    // Tranche 1 : 8 700 au-dessus de 8 655,97 (4 005 + 645,97 + 4 005), donc 8 655,97 - 3 000 - 1 000.
    assert_eq!(mars.t1, montant("4655.97"));
    // Tranche 2 et CET, sur les seules paies du CDI : 7 700 sous 8 010 ; ni tranche 2 ni CET (melees au technicien,
    // 8 700 au-dessus de 8 655,97, la CET serait due).
    assert_eq!((mars.t2, mars.cet, mars.t1_t2), (Montant::ZERO, Montant::ZERO, montant("4700")));
    // La CSG : 8 700 sous 4 x 8 655,97, moins 3 000 et 1 000.
    assert_eq!(mars.abattue_csg, montant("4700"));
}
