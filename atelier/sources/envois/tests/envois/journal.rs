//! Le journal des envois n'a que des ajouts (ce qui est parti se lit, regle 4), et l'empreinte d'un fichier y reste
//! comparable a celles deja ecrites : c'est elle qui dit qu'un fichier est deja parti.

use documents::fiche_questionnaire::CodeDeDemande;
use droit::fiche::Courriel;
use envois::journal::{self, Acte, DetailDsn, Envoi, Ligne, Mode, Revenu, RetourFiche};
use socle::mois::Mois;

use crate::outils::dossier_de_test;

fn envoi<D, R>(mode: Mode, detail: D) -> Envoi<D, R> {
    let mut e = Envoi::nouveau(mode, "test", true, "", "", detail);
    e.mois = Some(Mois::nouveau(2026, 9).unwrap());
    e.flux = Some(envois::retours::IdDeFlux::lire("F1").unwrap());
    e
}

/// Le `jour` d'octobre 2026 a `heure` pile.
fn instant(jour: u32, heure: i64) -> socle::paris::Moment {
    socle::paris::Moment::a_paris(socle::date::Jour::nouveau(2026, 10, jour).unwrap(), heure, 0, 0).unwrap()
}

/// La ligne datee du `jour` d'octobre 2026 a `heure`, que le test dit.
fn a(jour: u32, heure: i64) -> journal::Datation {
    journal::Datation::Lue { quand: instant(jour, heure).into(), origine: "le test".into() }
}

fn dsn(mode: Mode) -> Ligne {
    Ligne::Dsn(envoi(mode, DetailDsn::nouveau(false)))
}

fn fiche(code: &str) -> Ligne {
    Ligne::FicheDemande(envoi(Mode::Reel, journal::DetailFiche { code: CodeDeDemande::lire(code).unwrap(), adresse: Courriel::lire("a@b.invalid").unwrap(), prenom: Some("A".into()), message: None, expediteur: None }))
}

#[test]
fn noter_ajoute_a_la_fin_et_lire_filtre() {
    let d = dossier_de_test("journal", "ajouts");
    let l1 = journal::noter(&d, dsn(Mode::Reel), None, a(3, 10)).unwrap();
    assert_eq!((l1.quand().iso(), l1.nombre_de_retours()), ("2026-10-03T10:00:00".to_string(), 0));
    let avant = std::fs::read(journal::chemin(&d)).unwrap();
    journal::noter(&d, Ligne::Dpae(envoi(Mode::Test, journal::DetailDpae::nouveau(false, false))), None, a(3, 11)).unwrap();
    assert!(std::fs::read(journal::chemin(&d)).unwrap().starts_with(&avant), "ce qui etait ecrit au journal a ete reecrit");
    assert_eq!(journal::lire(&d, None, None, false).unwrap().len(), 2);
    assert_eq!(journal::lire(&d, Some(Acte::Dpae), None, false).unwrap().len(), 1);
    assert_eq!(journal::lire(&d, None, None, true).unwrap().len(), 1);
    assert_eq!(journal::lire(&d, None, Some(Mois::nouveau(2026, 8).unwrap()), false).unwrap().len(), 0);

    // DEUX POSTES (regle 9 b) : chacun ajoute au meme journal ce qu'il a fait, et range le meme retour ; leurs journaux mis
    // bout a bout disent tout, sans rien compter deux fois.
    journal::noter(&d, fiche("FIC-AAAAAA"), None, a(3, 12)).unwrap();
    let e = dossier_de_test("journal", "second-poste");
    std::fs::create_dir_all(e.join("journal")).unwrap();
    std::fs::copy(journal::chemin(&d), journal::chemin(&e)).unwrap();
    let retour = RetourFiche { code: CodeDeDemande::lire("FIC-AAAAAA").unwrap(), abandon: true, salarie: None, illisible: None, erreurs: None, quand: instant(4, 8), qui: "t".into(), de: None, message: None, reponse: "".into() };
    for (poste, quoi) in [(&d, fiche("FIC-BBBBBB")), (&e, dsn(Mode::Test))] {
        journal::noter(poste, quoi, None, a(4, 9)).unwrap();
        journal::rattacher(poste, |l| l.acte() == Acte::FicheDemande, Revenu::Fiche(retour.clone())).unwrap();
    }
    let mut reunis = std::fs::read(journal::chemin(&d)).unwrap();
    reunis.extend(std::fs::read(journal::chemin(&e)).unwrap());
    std::fs::write(journal::chemin(&e), reunis).unwrap();
    let lignes = journal::lire(&e, None, None, false).unwrap();
    let quoi: Vec<Acte> = lignes.iter().map(Ligne::acte).collect();
    assert_eq!(quoi, [Acte::Dsn, Acte::Dpae, Acte::FicheDemande, Acte::FicheDemande, Acte::Dsn]);
    assert_eq!(lignes[2].fiche().unwrap().retours, [retour.clone()]);
    assert_eq!(lignes[3].fiche().unwrap().retours, [retour]);

    // UNE LIGNE QUI NE SE LIT PAS STRICTEMENT arrete la lecture, sa ligne et sa cle nommees : rien ne se devine.
    let f = dossier_de_test("journal", "abime");
    std::fs::create_dir_all(f.join("journal")).unwrap();
    let mut texte = std::fs::read_to_string(journal::chemin(&d)).unwrap();
    texte.push_str(&texte.lines().next().unwrap().replace("\"flux\":", "\"fluxx\":"));
    texte.push('\n');
    std::fs::write(journal::chemin(&f), texte).unwrap();
    let refus = journal::lu(&f).unwrap_err();
    assert!(refus.phrase.starts_with("Le journal des envois de la compagnie ne se lit pas : sa ligne 6 est abîmée"), "{:?}", refus);
    assert!(format!("{:?}", refus).contains("fluxx"), "{:?}", refus);
}

#[test]
fn l_empreinte_reste_comparable_a_celles_du_journal() {
    // Les 16 premiers chiffres hexadecimaux du SHA-256, comme celles deja ecrites au journal.
    assert_eq!(socle::fichiers::Empreinte::des_octets(b"abc").courte().texte(), "ba7816bf8f01cfea");
    let d = dossier_de_test("journal", "empreinte");
    let f = d.join("DSN.dsn");
    std::fs::write(&f, b"abc").unwrap();
    let l = journal::noter(&d, dsn(Mode::Reel), Some(&f), a(3, 10)).unwrap();
    assert_eq!((l.fichier().map(|f| f.texte()), l.empreinte().map(|e| e.texte())), (Some("DSN.dsn"), Some("ba7816bf8f01cfea")));
    let l = journal::noter(&d, dsn(Mode::Reel), Some(&d.join("absent.dsn")), a(3, 10)).unwrap();
    assert_eq!((l.fichier().map(|f| f.texte()), l.empreinte()), (Some("absent.dsn"), None));
}
