//! LA PUBLICATION PAR GITHUB, `moteur maj` dans le flux « Publier » (`.github/workflows/publier.yml`), qui l'appelle sur
//! le `moteur` qu'il vient de compiler : signer une version (`publier`), poser et marquer les fictives livrees, et
//! fabriquer la cle de GitHub. Une installation ne publie rien : la mise a jour qu'elle cherche, applique ou defait est
//! `commandes::maj`.

mod publier;

use std::path::{Path, PathBuf};

use publier::{generer_cle_github, marquer_les_fictives, poser_les_fictives, publier_depuis_l_environnement};
pub use publier::VARIABLE_GRAINE;
use compagnie::maj::{dossier_maj, installation};

/// Les gestes de la publication de `moteur maj` (`--cle-github [DOSSIER]`, `--publier-github --logiciel L --sortie S
/// --notes T --calcul T --exigee T`, `--marquer-fictives --logiciel L`, `--poser-fictives DOSSIER --logiciel L`) : leur
/// code de sortie ; `None` quand `args` n'en demande aucun.
pub fn commande(args: &[socle::ligne::Argument]) -> Option<i32> {
    let args = socle::ligne::textes(args);
    let drapeau = |n: &str| args.iter().any(|a| a == n);
    let valeur = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).filter(|v| !v.starts_with("--")).cloned();
    if !["--cle-github", "--publier-github", "--marquer-fictives", "--poser-fictives"].iter().any(|d| drapeau(d)) {
        return None;
    }
    // D'abord savoir ou l'on est installe : sans cela, `installation()` retombe sur le dossier ou le programme a ete
    // compile (un chemin du conteneur, que Windows lit sur le lecteur courant : la graine de `--cle-github` irait
    // dans `D:\home\user\...\maj\`).
    let code = compagnie::installation::fixer_le_code();
    // Ni compagnie ni coffre.
    let r = if let Err(e) = code {
        Err(e)
    } else if drapeau("--cle-github") {
        // Sans dossier dit, la graine va dans `maj/` de l'installation.
        let dossier = match valeur("--cle-github") {
            Some(d) => PathBuf::from(d),
            None => dossier_maj(&installation()),
        };
        generer_cle_github(&dossier).map(|r| {
            format!(
                "Cle de GitHub fabriquee.\nLa GRAINE (secrete) est dans : {}\n  l'ouvrir, copier tout son contenu dans le secret PUPPETPAIE_CLE_SIGNATURE de l'environnement \
                 « publication » du depot public, puis EFFACER ce fichier.\nLa cle publique (pas secrete), aussi dans {} :\n  {}\n  a reporter dans compagnie/src/maj.rs, CLE_PUBLIQUE_GITHUB.",
                r.graine.display(), r.publique.display(), r.cle_publique
            )
        })
    } else if drapeau("--publier-github") {
        match (valeur("--logiciel"), valeur("--sortie")) {
            (Some(l), Some(s)) => publier_depuis_l_environnement(Path::new(&l), Path::new(&s), &phrase_publiee(valeur("--notes")), &phrase_publiee(valeur("--calcul")), &phrase_publiee(valeur("--exigee")))
                .map(|r| format!("Signee : {} ({}) dans {}", r.manifeste.version, r.dire_les_archives(), r.dossier.display())),
            _ => Err(options_manquantes("La publication demande le dossier du logiciel et celui où écrire.", "--logiciel DOSSIER --sortie DOSSIER")),
        }
    } else if drapeau("--marquer-fictives") {
        match valeur("--logiciel") {
            Some(l) => marquer_les_fictives(Path::new(&l)).map(|e| format!("Fictives marquees : livraison {}.", e)),
            None => Err(options_manquantes("Marquer les fictives demande le dossier du logiciel.", "--logiciel DOSSIER")),
        }
    } else {
        match (valeur("--logiciel"), valeur("--poser-fictives")) {
            (Some(l), Some(d)) => poser_les_fictives(Path::new(&l), Path::new(&d)).map(|e| format!("Fictives posees dans {} : livraison {}.", d, e)),
            _ => Err(options_manquantes("Poser les fictives demande le dossier du logiciel et celui où les poser.", "--poser-fictives DOSSIER --logiciel DOSSIER")),
        }
    };
    Some(match r {
        Ok(t) => {
            dire!("{}", t);
            0
        }
        Err(e) => {
            dire!("! {}", e.en_lignes());
            2
        }
    })
}

/// Une phrase de « Publier » (`notes`, `calcul`, `exigee`) telle que `derniere.json` l'ecrit : la phrase dite, ou rien. La
/// forme publiee porte toujours sa cle, vide quand l'editeur ne dit rien ; elle ne change plus.
fn phrase_publiee(dite: Option<String>) -> String {
    dite.into_iter().collect()
}

/// Le refus d'une commande a qui manquent des options : la phrase, et les options a part.
fn options_manquantes(phrase: &str, options: &str) -> socle::echec::Echec {
    socle::echec::refus_avec(phrase, socle::echec::Lieu { detail: Some(options.to_string()), ..Default::default() })
}
