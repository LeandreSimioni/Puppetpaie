//! Les trois verrous Cargo de l'atelier (celui de l'espace `Cargo.lock`, `fabrique-pdf/Cargo.lock`, `fenetre/Cargo.lock`) disent la
//! meme chose des paquets de l'atelier. « Publier » (`.github/workflows/publier.yml`) compile la fenetre avec `--locked` :
//! une dependance ajoutee au moteur sans que le verrou de la fenetre la connaisse fait echouer la publication sous Windows
//! et Mac, et rien d'autre ne le dit avant. Pour
//! remettre la fenetre a jour sans toucher aux autres versions epinglees : `cargo update --workspace` dans `fenetre/`.
//!
//! On ne lit pas le TOML entier : seulement, dans chaque verrou, le bloc `[[package]]` du paquet et sa liste
//! `dependencies` (les noms, sans la version qui les distingue quand deux versions cohabitent).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn sources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Les noms des dependances du paquet `nom` dans le verrou `verrou`, ou `None` s'il n'y figure pas.
fn dependances(verrou: &Path, nom: &str) -> Option<BTreeSet<String>> {
    let texte = std::fs::read_to_string(verrou).unwrap_or_else(|e| panic!("{} : {}", verrou.display(), e));
    let mut dans_le_bloc = false;
    let mut dans_la_liste = false;
    let mut deps = BTreeSet::new();
    for ligne in texte.lines() {
        let l = ligne.trim();
        if l == "[[package]]" {
            if dans_le_bloc {
                return Some(deps);
            }
            continue;
        }
        if l == format!("name = \"{}\"", nom) {
            dans_le_bloc = true;
            continue;
        }
        if !dans_le_bloc {
            continue;
        }
        if l.starts_with("dependencies = [") {
            dans_la_liste = !l.ends_with(']');
            continue;
        }
        if dans_la_liste {
            if l == "]" {
                dans_la_liste = false;
            } else {
                let d = l.trim_matches(|c| c == '"' || c == ',');
                deps.insert(d.split(' ').next().unwrap().to_string());
            }
        }
    }
    dans_le_bloc.then_some(deps)
}

/// Les dependances que le `Cargo.toml` du paquet ne prend que pour ses essais (`[dev-dependencies]`) : le verrou de
/// l'espace les porte, celui de la fenetre non (Cargo ne resout pas les essais d'un paquet pris par son chemin).
fn des_essais_seulement(paquet: &str) -> BTreeSet<String> {
    let t = std::fs::read_to_string(sources().join(paquet).join("Cargo.toml")).unwrap();
    let noms = |section: &str| -> BTreeSet<String> {
        t.split(&format!("\n{}\n", section))
            .nth(1)
            .map(|s| s.split("\n[").next().unwrap())
            .into_iter()
            .flat_map(|s| s.lines())
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.split([' ', '=']).next().unwrap().to_string())
            .collect()
    };
    noms("[dev-dependencies]").difference(&noms("[dependencies]")).cloned().collect()
}

#[test]
fn le_verrou_de_la_fenetre_connait_les_dependances_du_moteur_et_de_la_fabrique() {
    let fenetre = sources().join("fenetre/Cargo.lock");
    for (paquet, verrou) in [("socle", "Cargo.lock"), ("droit", "Cargo.lock"), ("engagement", "Cargo.lock"), ("documents", "Cargo.lock"), ("declarations", "Cargo.lock"), ("envois", "Cargo.lock"), ("compagnie", "Cargo.lock"), ("import", "Cargo.lock"), ("routes", "Cargo.lock"), ("moteur", "Cargo.lock"), ("fabrique-pdf", "fabrique-pdf/Cargo.lock")] {
        let sien = sources().join(verrou);
        let essais = des_essais_seulement(paquet);
        let attendu: BTreeSet<String> = dependances(&sien, paquet).unwrap_or_else(|| panic!("{} absent de son propre verrou", paquet)).difference(&essais).cloned().collect();
        let vu = dependances(&fenetre, paquet).unwrap_or_else(|| panic!("{} absent du verrou de la fenetre", paquet));
        let manquantes: Vec<_> = attendu.difference(&vu).collect();
        let en_trop: Vec<_> = vu.difference(&attendu).collect();
        assert!(
            manquantes.is_empty() && en_trop.is_empty(),
            "fenetre/Cargo.lock ne dit pas la meme chose que {} sur `{}` (manque {:?}, en trop {:?}) : \
             `cargo update --workspace` dans atelier/sources/fenetre/, sinon « Publier » refuse (--locked)",
            verrou, paquet, manquantes, en_trop
        );
    }
}
