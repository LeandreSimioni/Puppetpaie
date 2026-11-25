//! La sauvegarde du dossier prive dans le Drive (`sauvegarde.rs`) contre un FAUX `Drive` en memoire
//! (`tests/faux_drive`) : le dossier de la compagnie, l'inventaire et les exclusions, l'index ; deux compagnies d'un meme
//! compte qui ne se melent pas, et le dossier mis a la corbeille qui se renvoie entier.
//!
//! AUCUN RESEAU, AUCUN GOOGLE : le faux ne tient que des dossiers et des fichiers en memoire. Chaque test monte sa
//! compagnie dans un dossier temporaire a lui (jamais `prive/`, jamais un vrai coffre ni jeton : `coffre.bin` et
//! `google-jeton.bin` n'y sont que des textes inventés, pour montrer lequel part et lequel ne part pas).

use crate::faux_drive;

use std::path::{Path, PathBuf};

use faux_drive::{ecrire, racine as racine_de, FauxDrive};
use envois::drive::Drive;
use compagnie::sauvegarde::{EtatFichier, Sauvegarde};
use routes::gestes::sauvegarde::sauvegarder;

const RD: &str = "PuppetPaie";

fn racine(nom: &str) -> PathBuf {
    racine_de("sauvegarde", nom)
}

/// Une compagnie : 9 fichiers a sauvegarder (son identite sur le Drive comprise, `config/drive.json`), 8 qui ne partent
/// pas (sans coffre : `le_coffre_monte_le_jeton_google_non`). Sa fiche ne se lit pas : son dossier sur le Drive porte le
/// nom de son dossier, `sauvegarde-<nom>`.
fn monde(nom: &str) -> PathBuf {
    let r = racine(nom);
    ecrire(&r, "config/drive.json", &format!(r#"{{"identite": "cie:{:0>32}", "quoi": "essai"}}"#, nom.len()));
    for (rel, contenu) in [
        ("notes.txt", "notes"),
        ("config/entreprise.json", "{\"a\": 1}"),
        ("config/avant/ancien.json", "{}"),
        ("journal/h.md", "# h"),
        ("paies/2026-06/dupont.json", "d"),
        ("paies/avant/vieux.json", "v"),
        ("sorties/2026-06/bulletin.pdf", "%PDF"),
        ("sorties/2026-06/x.tmp/garde.txt", "dans un dossier x.tmp"),
        // ce qui ne part pas
        ("config/google-jeton.bin", "JETON-INVENTE"),
        ("config/.compteurs.verrou", "v"),
        ("config/.ouverture.json", "{}"),
        ("config/diagnostic.json", "{\"poste\": \"invente\"}"),
        ("ouverture.etat.json", "{}"),
        ("paies/__pycache__/m.cpython-311.pyc", "p"),
        ("paies/2026-06/brouillon.tmp", "t"),
        (".git/config", "g"),
    ] {
        ecrire(&r, rel, contenu);
    }
    r
}

/// Ce que la premiere passe fait, dossier par dossier (dans l'ordre des noms de dossier).
const DOSSIERS: [(&str, &[&str]); 8] = [
    (".", &["notes.txt"]),
    ("config", &["drive.json", "entreprise.json"]),
    ("config/avant", &["ancien.json"]),
    ("journal", &["h.md"]),
    ("paies/2026-06", &["dupont.json"]),
    ("paies/avant", &["vieux.json"]),
    ("sorties/2026-06", &["bulletin.pdf"]),
    ("sorties/2026-06/x.tmp", &["garde.txt"]),
];

/// Le dossier de la compagnie `nom` sur le Drive.
fn dossier_de(nom: &str) -> String {
    format!("{}/sauvegarde-{}", RD, nom)
}

fn chemin_drive(nom: &str, dossier: &str) -> String {
    if dossier == "." {
        format!("{}/sauvegarde", dossier_de(nom))
    } else {
        format!("{}/sauvegarde/{}", dossier_de(nom), dossier)
    }
}

fn journal_de_la_premiere_passe(nom: &str) -> Vec<String> {
    let mut j = vec![format!("chemin {}", RD), format!("sous_dossiers {}", RD), format!("creer_dossier {}", dossier_de(nom))];
    for (dossier, noms) in DOSSIERS {
        let c = chemin_drive(nom, dossier);
        j.push(format!("chemin {}", c));
        j.push(format!("lister {}", c));
        for n in noms {
            j.push(format!("televerser {}/{} connu=-", c, n));
        }
    }
    j.push(format!("marquer {}", dossier_de(nom)));
    j
}

fn passer(r: &Path, d: &mut FauxDrive) -> Sauvegarde {
    sauvegarder(r, RD, false, Some(d as &mut dyn Drive), &mut |_, _| Ok(())).unwrap()
}

// ------------------------------------------------------------ la premiere passe et l'index

#[test]
fn premiere_passe_tout_part_dans_le_dossier_de_la_compagnie() {
    let r = monde("premiere");
    let mut d = FauxDrive::nouveau();
    let s = passer(&r, &mut d);
    assert_eq!(d.journal, journal_de_la_premiere_passe("premiere"));
    assert_eq!(s.deposes.len(), 9);
    assert!(s.deposes.iter().all(|f| f.etat == EtatFichier::Neuf));
    assert_eq!((s.identiques, s.exclus, s.disparus, s.hors_ligne), (0, 8, 0, false));
    assert!(s.incidents.is_empty(), "{:?}", s.incidents);
    assert_eq!(s.compte.as_deref(), Some("compte@exemple.invalid"));
    // L'arbre du Drive : le contenu de chaque fichier, et rien de ce qui est propre au poste.
    let arbre = d.arbre();
    assert_eq!(arbre.len(), 9);
    assert_eq!(arbre[&format!("{}/config/entreprise.json", chemin_drive("premiere", "."))], ["{\"a\": 1}"]);
    assert!(arbre.keys().all(|k| !k.contains("coffre.bin") && !k.contains("google-jeton") && !k.contains("diagnostic") && !k.contains("ouverture.etat") && !k.contains(".git") && !k.contains("pycache")));
    // Le dossier porte la marque de la compagnie, la date de cette sauvegarde et son volume.
    let id = d.id_du_dossier(&dossier_de("premiere"));
    let marque = &d.marques[&id];
    assert_eq!(marque.compagnie.as_deref(), Some(format!("cie:{:0>32}", 8).as_str()));
    assert!(marque.derniere.is_some() && marque.octets.is_some(), "{:?}", marque);
}

/// Le coffre monte, le jeton Google non (une decision d'editeur). Le coffre est scelle par le mot de passe du logiciel : c'est ce qui l'ouvre sur un
/// poste neuf apres une restauration. Le jeton Google, lui, se refait en reliant le compte.
#[test]
fn le_coffre_monte_le_jeton_google_non() {
    let r = monde("coffre-monte");
    ecrire(&r, "config/coffre.bin", "COFFRE-INVENTE");
    let mut d = FauxDrive::nouveau();
    let s = passer(&r, &mut d);
    let arbre = d.arbre();
    assert_eq!(arbre[&format!("{}/config/coffre.bin", chemin_drive("coffre-monte", "."))], ["COFFRE-INVENTE"]);
    assert!(arbre.keys().all(|k| !k.contains("google-jeton")));
    assert_eq!((s.deposes.len(), s.exclus), (10, 8));
}

// ------------------------------------------------------------ rien de change

#[test]
fn rien_de_change_rien_ne_part_et_les_dossiers_connus_ne_sont_pas_recherches() {
    let r = monde("rien");
    let mut d = FauxDrive::nouveau();
    passer(&r, &mut d);
    d.journal.clear();
    let s = passer(&r, &mut d);
    assert!(s.deposes.is_empty());
    assert_eq!((s.identiques, s.exclus), (9, 9)); // l'index ecrit s'ajoute aux ecartes
    assert_eq!(d.appels("televerser"), 0);
    assert_eq!(d.appels("chemin"), 0, "les identifiants de dossiers viennent de l'index");
    assert_eq!(d.appels("en_place"), 1, "le dossier de la compagnie se verifie une fois");
    assert_eq!(d.appels("lister"), 8, "un listage par dossier");
    assert_eq!(d.appels("marquer"), 0, "rien d'envoye, la marque ne change pas");
    assert_eq!(s.resume(), "Sauvegarde a jour (9 fichiers).");
    assert_eq!(d.arbre().len(), 9);
}

// ------------------------------------------------------------ deux compagnies, un compte

/// DEUX COMPAGNIES D'UN MEME COMPTE GOOGLE NE SE MELENT PLUS (#594) : chacune remplit son dossier, que sa marque
/// designe ; mettre le dossier de l'une a la corbeille ne touche pas l'autre, et sa sauvegarde suivante renvoie tout dans
/// un dossier neuf.
#[test]
fn deux_compagnies_d_un_compte_ont_chacune_leur_dossier() {
    let (a, b) = (monde("une"), monde("deux"));
    let mut d = FauxDrive::nouveau();
    passer(&a, &mut d);
    passer(&b, &mut d);
    let arbre = d.arbre();
    assert_eq!(arbre.len(), 18);
    assert_eq!(arbre.keys().filter(|k| k.starts_with(&format!("{}/", dossier_de("une")))).count(), 9);
    assert_eq!(arbre.keys().filter(|k| k.starts_with(&format!("{}/", dossier_de("deux")))).count(), 9);
    let id = d.id_du_dossier(&dossier_de("une"));
    d.mettre_a_la_corbeille(&id);
    let s = passer(&a, &mut d);
    assert_eq!(s.deposes.len(), 9, "tout repart : {:?}", s);
    assert_eq!(d.appels("creer_dossier"), 3, "un dossier neuf pour la premiere, rien pour la seconde");
}

#[test]
fn copie_de_developpement_refus_avant_tout() {
    let r = monde("copie");
    ecrire(&r, "COPIE-DE-DEVELOPPEMENT.json", r#"{"copie_de": "prive", "quoi": "copie de developpement"}"#);
    // Sans client donne : le vrai se construit, et refuse une copie de developpement avant tout (pas de reseau, pas d'index).
    let e = sauvegarder(&r, RD, false, None, &mut |_, _| Ok(())).unwrap_err();
    assert_eq!(e.echec.classe, Some(socle::echec::Classe::CopieDeDeveloppement));
    assert!(!r.join("config/.sauvegarde.json").exists());
}
