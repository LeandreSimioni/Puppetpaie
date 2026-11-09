//! `moteur maj` : la mise a jour en ligne de commande -- la chercher, l'appliquer, revenir a la precedente -- et, chez
//! l'editeur, la publier (`atelier::publier`). Ce que la mise a jour recoit, verifie et applique : `compagnie::maj`.

use std::path::{Path, PathBuf};

use crate::atelier::publier::{generer_cle, generer_cle_github, marquer_les_fictives, poser_les_fictives, publier, publier_depuis_l_environnement};
use crate::compagnie::maj::{appliquer, dossier_maj, installation, revenir, verifier, version_lue};

/// `moteur maj [--sans-telecharger] [--appliquer] [--revenir] [--publier [DEPOT] --notes T --calcul T
/// --exigee T] [--cle [--remplacer]] [--cle-github [DOSSIER]] [--publier-github --logiciel L --sortie S --notes T
/// --calcul T --exigee T] [--marquer-fictives --logiciel L] [--poser-fictives DOSSIER --logiciel L]`.
pub fn commande(args: &[String]) -> i32 {
    // D'abord savoir ou l'on est installe : sans cela, `installation()` retombe sur le dossier ou le programme a ete
    // compile (un chemin du conteneur, que Windows lit sur le lecteur courant : la graine de `--cle-github` irait
    // dans `D:\home\user\...\maj\`).
    crate::compagnie::installation::fixer_le_code();
    let drapeau = |n: &str| args.iter().any(|a| a == n);
    let valeur = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).filter(|v| !v.starts_with("--")).cloned();
    // Les gestes de la publication par GitHub : ni compagnie ni coffre.
    let github = if drapeau("--cle-github") {
        // Sans dossier dit, la graine va dans `maj/` de l'installation.
        let dossier = match valeur("--cle-github") {
            Some(d) => PathBuf::from(d),
            None => dossier_maj(&installation()),
        };
        Some(generer_cle_github(&dossier).map(|r| {
            format!(
                "Cle de GitHub fabriquee.\nLa GRAINE (secrete) est dans : {}\n  l'ouvrir, copier tout son contenu dans le secret PUPPETPAIE_CLE_SIGNATURE de l'environnement \
                 « publication » du depot public, puis EFFACER ce fichier.\nLa cle publique (pas secrete), aussi dans {} :\n  {}\n  a reporter dans moteur/src/compagnie/maj.rs, CLE_PUBLIQUE_GITHUB.",
                r.graine, r.publique, r.cle_publique
            )
        }))
    } else if drapeau("--publier-github") {
        match (valeur("--logiciel"), valeur("--sortie")) {
            (Some(l), Some(s)) => Some(
                publier_depuis_l_environnement(Path::new(&l), Path::new(&s), &phrase_publiee(valeur("--notes")), &phrase_publiee(valeur("--calcul")), &phrase_publiee(valeur("--exigee")))
                    .map(|r| format!("Signee : {} ({}) dans {}", r.manifeste.version, r.dire_les_archives(), r.dossier.display())),
            ),
            _ => Some(Err(options_manquantes("La publication demande le dossier du logiciel et celui où écrire.", "--logiciel DOSSIER --sortie DOSSIER"))),
        }
    } else if drapeau("--marquer-fictives") {
        Some(match valeur("--logiciel") {
            Some(l) => marquer_les_fictives(Path::new(&l)).map(|e| format!("Fictives marquees : livraison {}.", e)),
            None => Err(options_manquantes("Marquer les fictives demande le dossier du logiciel.", "--logiciel DOSSIER")),
        })
    } else if drapeau("--poser-fictives") {
        Some(match (valeur("--logiciel"), valeur("--poser-fictives")) {
            (Some(l), Some(d)) => poser_les_fictives(Path::new(&l), Path::new(&d)).map(|e| format!("Fictives posees dans {} : livraison {}.", d, e)),
            _ => Err(options_manquantes("Poser les fictives demande le dossier du logiciel et celui où les poser.", "--poser-fictives DOSSIER --logiciel DOSSIER")),
        })
    } else {
        None
    };
    if let Some(r) = github {
        return match r {
            Ok(t) => {
                dire!("{}", t);
                0
            }
            Err(e) => {
                dire!("! {}", e);
                2
            }
        };
    }
    let racine = crate::routes::ouverture::racine_du_poste();
    let r = if drapeau("--cle") {
        generer_cle(&racine, drapeau("--remplacer")).map(|r| {
            format!(
                "Cle privee ecrite (scellee par le coffre : le mot de passe du logiciel l'ouvre sur un autre poste) : {}\nCle publique, a reporter dans moteur/src/compagnie/maj.rs, CLE_PUBLIQUE :\n  {}{}",
                r.chemin,
                r.cle_publique,
                if r.correspond { "" } else { "\n  (le code porte une autre cle publique : tant qu'elle n'est pas reportee, `publier` refuse)" }
            )
        })
    } else if drapeau("--publier") {
        publier(&racine, valeur("--publier").as_deref(), &phrase_publiee(valeur("--notes")), &phrase_publiee(valeur("--calcul")), &phrase_publiee(valeur("--exigee")))
            .map(|r| format!("Publiee : {} ({}) dans {}", version_lue(&r.manifeste.version), r.dire_les_archives(), r.dossier.display()))
    } else if drapeau("--appliquer") {
        match crate::compagnie::instance::ouverte(&installation(), crate::compagnie::instance::Plage::DU_POSTE) {
            Ok(true) => Err("PuppetPaie tourne : fermer la fenetre, puis appliquer".into()),
            Ok(false) => appliquer(&installation()).map(|r| format!("Version {} en place ; la precedente est dans {}", version_lue(&r.version), r.avant)),
            Err(e) => Err(e),
        }
    } else if drapeau("--revenir") {
        match crate::compagnie::instance::ouverte(&installation(), crate::compagnie::instance::Plage::DU_POSTE) {
            Ok(true) => Err("PuppetPaie tourne : fermer la fenetre, puis revenir".into()),
            Ok(false) => revenir(&installation()).map(|v| format!("Retour a la version {}", version_lue(&v))),
            Err(e) => Err(e),
        }
    } else {
        Ok(verifier(!drapeau("--sans-telecharger")))
    };
    match r {
        Ok(t) => {
            dire!("{}", t);
            0
        }
        Err(e) => {
            dire!("! {}", e);
            2
        }
    }
}

/// Une phrase de « Publier » (`notes`, `calcul`, `exigee`) telle que `derniere.json` l'ecrit : la phrase dite, ou rien. La
/// forme publiee porte toujours sa cle, vide quand l'editeur ne dit rien ; elle ne change plus.
fn phrase_publiee(dite: Option<String>) -> String {
    dite.into_iter().collect()
}

/// Le refus d'une commande a qui manquent des options : la phrase, et les options a part.
fn options_manquantes(phrase: &str, options: &str) -> String {
    socle::echec::avec(phrase, socle::echec::APart { detail: Some(options.to_string()), ..Default::default() })
}
