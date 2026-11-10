//! Le coffre (`compagnie::coffre`) : poser, relire, oublier un secret, le mot de passe du logiciel, la cle du poste, le scelle,
//! sur des coffres et des mots de passe inventes, chacun dans son dossier (jamais la vraie cle du poste).

use crate::outils::chemin_unique;
use compagnie::coffre::{deriver, Coffre, Kdf, CONSULTATION};
use socle::json_ascii::dumps;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn dossier() -> PathBuf {
    let d = chemin_unique("coffre");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Un poste (sa cle) et une compagnie (sa racine), a eux.
fn coffre() -> Coffre {
    let base = dossier();
    std::fs::create_dir_all(base.join("racine/config")).unwrap();
    Coffre { racine: base.join("racine"), dossier_cle: base.join("poste"), maintenant: "2026-10-03T14:05".into(), variables: BTreeMap::new() }
}

/// Le meme coffre vu d'un autre poste.
fn autre_poste(c: &Coffre) -> Coffre {
    Coffre { racine: c.racine.clone(), dossier_cle: dossier(), maintenant: c.maintenant.clone(), variables: BTreeMap::new() }
}

fn lire(c: &Coffre, ouverture: Option<&str>) -> Result<Option<String>, String> {
    c.lire("net-entreprises", CONSULTATION, false, ouverture).map_err(String::from)
}

#[test]
fn un_secret_se_pose_et_se_relit_sans_etre_en_clair() {
    let c = coffre();
    c.poser("urssaf", "UnSecretTresReconnaissable", None).unwrap();
    let octets = std::fs::read(c.chemin()).unwrap();
    assert!(!String::from_utf8_lossy(&octets).contains("UnSecretTresReconnaissable"));
    let r = c.lire("urssaf", CONSULTATION, false, None).unwrap();
    assert_eq!(r.as_deref(), Some("UnSecretTresReconnaissable"));
    assert_eq!(lire(&c, None).unwrap(), None);
    assert!(c.pose("urssaf").unwrap() && !c.pose("courriel").unwrap());
}

#[test]
fn l_etat_ne_rend_jamais_la_valeur() {
    let c = coffre();
    c.poser("urssaf", "MotDePasseTresSecret42", None).unwrap();
    let etat = serde_json::to_string(&c.services().unwrap()).unwrap();
    assert!(!etat.contains("MotDePasseTresSecret42"));
    assert!(etat.contains("\"pose_le\":\"2026-10-03 14:05\""));
}

#[test]
fn le_mot_de_passe_du_logiciel_ouvre_l_enveloppe_et_rien_d_autre() {
    let c = coffre();
    assert!(!c.ouverture_exigee().unwrap());
    assert!(!c.verifier_ouverture("sesame-123").unwrap());
    c.poser_ouverture("sesame-123", None).unwrap();
    assert!(c.ouverture_exigee().unwrap());
    assert!(c.verifier_ouverture("sesame-123").unwrap());
    assert!(!c.verifier_ouverture("sesame-123 ").unwrap());
    assert!(!c.verifier_ouverture("").unwrap());
    assert!(!String::from_utf8_lossy(&std::fs::read(c.chemin()).unwrap()).contains("sesame-123"));
    assert!(c.poser_ouverture("12345", None).unwrap_err().phrase.starts_with("Mot de passe du logiciel trop court : 6 caracteres"));
    assert!(c.oublier_ouverture(None).unwrap());
    assert!(!c.ouverture_exigee().unwrap() && !c.chemin().exists());
}

#[test]
fn un_coffre_apporte_sur_un_autre_poste_s_ouvre_par_le_mot_de_passe() {
    let c = coffre();
    c.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    c.poser_ouverture("sesame-123", None).unwrap();
    let ailleurs = autre_poste(&c);
    assert!(!ailleurs.ouvre_a_ce_poste().unwrap());
    assert!(lire(&ailleurs, None).unwrap_err().contains("n'a pas ete donne"));
    assert!(lire(&ailleurs, Some("faux-faux")).unwrap_err().contains("ne l'ouvre pas non plus"));
    // le mot de passe ouvre sans rien changer au poste
    assert_eq!(lire(&ailleurs, Some("sesame-123")).unwrap().as_deref(), Some("unvraimotdepasse"));
    assert!(!ailleurs.chemin_cle().exists());
    // et reposer le mot de passe repose une cle : le poste retrouve son autonomie
    ailleurs.poser_ouverture("sesame-124", Some("sesame-123")).unwrap();
    assert!(ailleurs.ouvre_a_ce_poste().unwrap());
    assert_eq!(lire(&ailleurs, None).unwrap().as_deref(), Some("unvraimotdepasse"));
    assert!(ailleurs.verifier_ouverture("sesame-124").unwrap());
}

#[test]
fn sans_cle_ni_mot_de_passe_le_coffre_refuse_et_le_dit() {
    let c = coffre();
    c.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    std::fs::remove_file(c.chemin_cle()).unwrap();
    assert!(lire(&c, None).unwrap_err().contains("et elle est absente"));
    assert!(c.poser("urssaf", "unvraimotdepasse", None).unwrap_err().phrase.starts_with("Ce coffre ne s'ouvre pas ici"));
}

#[test]
fn un_fichier_altere_ou_d_une_autre_version_est_refuse() {
    let c = coffre();
    c.poser("net-entreprises", "unvraimotdepasse", None).unwrap();
    let bon = std::fs::read_to_string(c.chemin()).unwrap();
    let mut v: Value = serde_json::from_str(&bon).unwrap();
    let t = v["secrets"].as_str().unwrap().to_string();
    v["secrets"] = json!(format!("{}{}", if t.starts_with('A') { "B" } else { "A" }, &t[1..]));
    std::fs::write(c.chemin(), dumps(&v, Some(2), false, true)).unwrap();
    assert!(lire(&c, None).unwrap_err().starts_with("Le scelle refuse"));
    for contenu in ["", "{}", "[]", "pas du json", "{\"version\": 1}"] {
        std::fs::write(c.chemin(), contenu).unwrap();
        let e = lire(&c, None).unwrap_err();
        assert!(e.contains("n'est pas au format de cette version"), "{:?}", contenu);
        assert!(!c.ouverture_exigee().unwrap());
        assert!(c.services().unwrap().iter().all(|s| !s.pose));
    }
}

#[test]
fn scrypt_est_celui_de_la_rfc_7914_et_refuse_les_parametres_d_un_fichier_piege() {
    let kdf = |algo: &str, n: u64, r: u64, p: u64, sel: &str| Kdf { algo: algo.into(), n, r, p, sel: sel.into() };
    let cle = deriver("password", &kdf("scrypt", 1024, 8, 16, "TmFDbA==")).unwrap();
    assert_eq!(cle.iter().map(|b| format!("{:02x}", b)).collect::<String>(), "fdbabe1c9d3472007856e7190d01e9fe7c6ad7cbc8237830e77376634b373162");
    // n qui n'est pas une puissance de 2, r nul, une memoire demesuree
    for (n, r, p) in [(3, 8, 1), (2, 0, 1), (131072, 8, 1)] {
        assert!(deriver("x", &kdf("scrypt", n, r, p, "AAAA")).is_err(), "n={} r={} p={}", n, r, p);
    }
    assert!(deriver("x", &kdf("argon2", 1024, 8, 1, "AAAA")).unwrap_err().phrase.contains("argon2"), "le refus nomme l'algorithme");
}

#[test]
fn l_ouverture_gardee_vaut_pour_son_coffre_et_expire() {
    let c = coffre();
    c.poser_ouverture("sesame-123", None).unwrap();
    assert!(c.garder_ouverture("faux-faux", "2026-10-04T02:05").unwrap_err().phrase.starts_with("Mot de passe du logiciel refuse"));
    assert_eq!(c.ouverture_gardee().unwrap(), None);
    c.garder_ouverture("sesame-123", "2026-10-04T02:05").unwrap();
    assert!(!String::from_utf8_lossy(&std::fs::read(c.chemin_gardee().unwrap()).unwrap()).contains("sesame-123"));
    assert_eq!(c.ouverture_gardee().unwrap().as_deref(), Some("sesame-123"));
    assert_eq!(c.fin_ouverture_gardee().unwrap().as_deref(), Some("2026-10-04T02:05"));
    // un autre coffre, meme poste : rien
    let autre = Coffre { racine: dossier().join("autre"), dossier_cle: c.dossier_cle.clone(), maintenant: c.maintenant.clone(), variables: BTreeMap::new() };
    assert_eq!(autre.ouverture_gardee().unwrap(), None);
    assert!(c.chemin_gardee().unwrap().exists());
    // expiree : elle s'efface
    let tard = Coffre { racine: c.racine.clone(), dossier_cle: c.dossier_cle.clone(), maintenant: "2026-10-04T02:06".into(), variables: BTreeMap::new() };
    assert_eq!(tard.fin_ouverture_gardee().unwrap(), None);
    assert_eq!(tard.ouverture_gardee().unwrap(), None);
    assert!(!c.chemin_gardee().unwrap().exists());
}

/// Reste 158 : deux commandes qui posent la cle du poste en meme temps (deux tests, ou l'interface et la ligne de
/// commande au premier lancement) ne se font pas echouer l'une l'autre sur « File exists », et ne scellent pas avec
/// deux cles differentes : la seconde lit celle que la premiere a posee.
#[test]
fn la_cle_du_poste_posee_par_plusieurs_a_la_fois_est_une_seule() {
    let c = coffre();
    let n = 16;
    let barriere = std::sync::Arc::new(std::sync::Barrier::new(n));
    let fils: Vec<_> = (0..n)
        .map(|i| {
            let (racine, poste, b) = (c.racine.clone(), c.dossier_cle.clone(), barriere.clone());
            std::thread::spawn(move || {
                let k = Coffre { racine, dossier_cle: poste, maintenant: "2026-10-03T14:05".into(), variables: BTreeMap::new() };
                b.wait();
                k.proteger(format!("secret {}", i).as_bytes(), "essai").map_err(|e| format!("{:?}", e))
            })
        })
        .collect();
    let scelles: Vec<_> = fils.into_iter().map(|f| f.join().unwrap()).collect();
    for (i, s) in scelles.into_iter().enumerate() {
        let s = s.unwrap_or_else(|e| panic!("fil {} : {}", i, e));
        assert_eq!(c.deproteger(&s, "essai").unwrap(), format!("secret {}", i).into_bytes(), "fil {}", i);
    }
    assert_eq!(std::fs::read(c.chemin_cle()).unwrap().len(), 32);
}
