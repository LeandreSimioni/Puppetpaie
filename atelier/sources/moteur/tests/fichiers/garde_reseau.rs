//! Le garde-fou reseau des essais (`socle/src/garde_reseau.rs`) tient pour TOUTE sortie du moteur, et pour tout
//! `moteur` qu'un essai lance : un essai ne joint jamais un serveur reel (celui de l'editeur, net-entreprises, l'Urssaf,
//! Google, la DILA, le depot des versions, une boite courriel). Cargo pose la variable (`.cargo/config.toml`, `[env]`) ;
//! ces essais verifient, dans les sources, que rien ne la contourne :
//! - chaque fichier du moteur dont le code ouvre une connexion passe par `garde_reseau` : il l'appelle, ou il l'est (une
//!   connexion vers la machine elle-meme se demande a `garde_reseau::vers_cette_machine`) ;
//! - chaque essai qui lance un programme dans un environnement vide (`env_clear`) y repose la variable.
//! Ni l'un ni l'autre n'ecarte un fichier : ils lisent le code, commentaires et textes blanchis, de chaque crate de
//! l'espace (`structure::dossiers_des_crates` : chaque dossier qui a son `Cargo.toml`).
//!
//! La preuve par le comportement est dans `incidents_filet` (`sans_serveur_pose_rien_ne_sort_vers_le_serveur_de_l_editeur`)
//! et dans le moteur (`garde_reseau::tests`).

use std::path::Path;

use crate::structure::{espace, essais_des_crates, sources_des_crates};

/// Ce qui ouvre une connexion (ou un navigateur) vers une adresse.
const SORTIES: [&str; 8] = ["ureq::Agent", "ureq::get(", "ureq::post(", "TcpStream::connect", "SmtpTransport::", "UdpSocket", "xdg-open", "rundll32"];

/// Le code d'un source, commentaires et textes blanchis, sauf les textes qui nomment un programme a lancer (`xdg-open`,
/// `rundll32`) : ce qui ouvre une connexion s'y lit.
fn code(f: &Path) -> String {
    let t = std::fs::read_to_string(f).unwrap();
    let c: String = crate::structure::code_sans_textes(&t).into_iter().collect();
    let programmes = crate::structure::litteraux_rs(&t).into_iter().filter(|(_, s)| SORTIES.contains(&s.as_str())).map(|(_, s)| s);
    c + "\n" + &programmes.collect::<Vec<_>>().join("\n")
}

#[test]
fn chaque_sortie_du_moteur_passe_par_le_garde_fou() {
    // Le module du garde-fou, tel que le compilateur le nomme (`socle::garde_reseau`).
    let garde = std::any::type_name_of_val(&socle::garde_reseau::en_essai).rsplit_once("::").unwrap().0.to_string();
    let racine = espace();
    let mut fautifs = Vec::new();
    for f in sources_des_crates() {
        let rel = f.strip_prefix(&racine).unwrap().to_string_lossy().replace('\\', "/");
        let module = crate::structure::module_de(&rel);
        let code = code(&f);
        let ouvre = SORTIES.iter().any(|s| code.contains(s));
        if ouvre && module != garde && !code.contains("garde_reseau::") {
            fautifs.push(rel);
        }
    }
    assert!(fautifs.is_empty(), "ces fichiers ouvrent une connexion sans passer par `garde_reseau` : {:?}", fautifs);
}

#[test]
fn chaque_environnement_vide_d_un_essai_repose_le_garde_fou() {
    let racine = espace();
    let mut fautifs = Vec::new();
    for f in essais_des_crates() {
        let texte: String = crate::structure::code_sans_textes(&std::fs::read_to_string(&f).unwrap()).into_iter().collect();
        let lignes: Vec<&str> = texte.lines().collect();
        for (i, l) in lignes.iter().enumerate() {
            if !l.contains("env_clear()") {
                continue;
            }
            if !lignes[i..(i + 3).min(lignes.len())].iter().any(|x| x.contains("garde_reseau::VARIABLE")) {
                fautifs.push(format!("{}:{}", f.strip_prefix(&racine).unwrap().display(), i + 1));
            }
        }
    }
    assert!(fautifs.is_empty(), "un `env_clear` qui ne repose pas `garde_reseau::VARIABLE` juste apres : {:?}", fautifs);
}

/// Cargo pose la variable pour cet essai, et donc pour tout `moteur` qu'il lance sans vider l'environnement.
#[test]
fn cargo_pose_le_garde_fou() {
    assert!(std::env::var_os(socle::garde_reseau::VARIABLE).is_some(), "`.cargo/config.toml` du moteur doit poser {}", socle::garde_reseau::VARIABLE);
}
