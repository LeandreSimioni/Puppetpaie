//! LES EMPREINTES DES TEXTES DEPOSES : chaque fichier de `atelier/reference/sources/` est une copie, octet pour octet,
//! de ce qu'un autre a publie, et son empreinte (SHA-256) s'ecrit dans le `LISEZ-MOI.md` de son dossier (le plus proche
//! en remontant), dans sa derniere section, `## Empreintes`. `moteur-atelier empreintes` les ecrit ; le garde avant
//! commit les lit : un fichier dont l'empreinte est celle que son `LISEZ-MOI` porte est le texte de son emetteur, et un
//! numero qui y a la forme d'un NIR, d'un IBAN ou d'un BIC est le sien (l'exemple d'une documentation officielle, ou un
//! nombre qui n'en est pas un). Un fichier change apres son depot, ou sans empreinte, se garde comme le reste ; un
//! `LISEZ-MOI` (nos notes) n'a pas d'empreinte.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use socle::echec::R;

/// Le dossier des textes deposes, relatif a la racine du depot.
pub const TEXTES: &str = "atelier/reference/sources";

/// Le nom du `LISEZ-MOI` de chaque dossier.
pub const LISEZ_MOI: &str = "LISEZ-MOI.md";

/// Le titre de la section des empreintes, la derniere du `LISEZ-MOI`.
const SECTION: &str = "## Empreintes";

/// Ce que la section dit d'elle-meme, sous son titre.
const CHAPEAU: &str = "Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).";

/// L'empreinte d'un contenu : le SHA-256, en hexadecimal minuscule.
pub fn empreinte(o: &[u8]) -> String {
    Sha256::digest(o).iter().map(|b| format!("{:02x}", b)).collect()
}

/// Les fichiers de `dossier`, a toute profondeur, tries ; aucun s'il n'existe pas (un depot sans textes deposes).
fn fichiers(dossier: &Path, out: &mut Vec<PathBuf>) -> R<()> {
    let Some(entrees) = socle::fichiers::entrees(dossier)? else { return Ok(()) };
    for p in entrees {
        if p.is_dir() {
            fichiers(&p, out)?;
        } else {
            out.push(p);
        }
    }
    Ok(())
}

/// Le `LISEZ-MOI` qui porte l'empreinte de `fichier` : celui de son dossier, sinon du plus proche en remontant jusqu'a
/// `racine` (les textes deposes). `None` : aucun.
fn lisez_moi_de(racine: &Path, fichier: &Path) -> Option<PathBuf> {
    fichier.ancestors().skip(1).take_while(|d| d.starts_with(racine)).map(|d| d.join(LISEZ_MOI)).find(|l| l.is_file())
}

/// Le texte d'un `LISEZ-MOI` sans sa section des empreintes (toujours la derniere).
fn sans_empreintes(texte: &str) -> &str {
    match texte.find(&format!("\n{}\n", SECTION)) {
        Some(i) => &texte[..i],
        None => texte,
    }
}

/// ECRIT LES EMPREINTES de chaque texte depose sous `depot` dans le `LISEZ-MOI` qui le porte (sa section, refaite
/// entiere a chaque fois) : son chemin relatif au dossier du `LISEZ-MOI`, et son SHA-256. Un texte qu'aucun `LISEZ-MOI`
/// ne porte refuse (chaque dossier a le sien : atelier/AGENTS.md, « Les sources »), et rien n'est ecrit. Rend le
/// nombre de textes.
pub fn ecrire(depot: &Path) -> R<usize> {
    let racine = depot.join(TEXTES);
    let mut tous = Vec::new();
    fichiers(&racine, &mut tous)?;
    let mut par_lisez_moi: BTreeMap<PathBuf, Vec<(String, String)>> = BTreeMap::new();
    let mut sans = Vec::new();
    for f in tous.iter().filter(|f| f.file_name().is_none_or(|n| n != LISEZ_MOI)) {
        let Some(l) = lisez_moi_de(&racine, f) else {
            sans.push(f.display().to_string());
            continue;
        };
        let dossier = match l.parent() {
            Some(d) => d,
            None => return Err(socle::refus!("{} n'a pas de dossier", l.display())),
        };
        let rel = match f.strip_prefix(dossier) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(e) => return Err(socle::refus!("{} n'est pas sous {} : {}", f.display(), dossier.display(), e)),
        };
        let o = std::fs::read(f).map_err(|e| socle::echec::du_disque(f, &e))?;
        par_lisez_moi.entry(l).or_default().push((rel, empreinte(&o)));
    }
    if !sans.is_empty() {
        return Err(socle::refus!("Ces textes déposés n'ont aucun {} pour porter leur empreinte : {}. Rien n'est écrit.", LISEZ_MOI, sans.join(", ")));
    }
    let mut n = 0;
    for (l, lignes) in &par_lisez_moi {
        let texte = std::fs::read_to_string(l).map_err(|e| socle::echec::du_disque(l, &e))?;
        let mut neuf = sans_empreintes(&texte).trim_end().to_string();
        neuf.push_str(&format!("\n\n{}\n\n{}\n\n", SECTION, CHAPEAU));
        for (rel, e) in lignes {
            neuf.push_str(&format!("- `{}` : `{}`\n", rel, e));
        }
        n += lignes.len();
        if neuf != texte {
            std::fs::write(l, neuf).map_err(|e| socle::echec::du_disque(l, &e))?;
        }
    }
    Ok(n)
}

/// LES EMPREINTES ECRITES : pour chaque texte que la section d'un `LISEZ-MOI` sous `depot` nomme, son chemin relatif a
/// la racine du depot et son SHA-256. Une ligne de la section qui n'a pas la forme qu'`ecrire` lui donne refuse.
pub fn lues(depot: &Path) -> R<BTreeMap<String, String>> {
    let racine = depot.join(TEXTES);
    let mut tous = Vec::new();
    fichiers(&racine, &mut tous)?;
    let mut out = BTreeMap::new();
    for l in tous.iter().filter(|f| f.file_name().is_some_and(|n| n == LISEZ_MOI)) {
        let texte = std::fs::read_to_string(l).map_err(|e| socle::echec::du_disque(l, &e))?;
        let Some(i) = texte.find(&format!("\n{}\n", SECTION)) else { continue };
        let Some(parent) = l.parent() else { return Err(socle::refus!("{} n'a pas de dossier", l.display())) };
        let dossier = match parent.strip_prefix(depot) {
            Ok(d) => d.to_string_lossy().replace('\\', "/"),
            Err(e) => return Err(socle::refus!("{} n'est pas sous le dépôt : {}", l.display(), e)),
        };
        for ligne in texte[i..].lines().filter(|x| x.starts_with("- `")) {
            let lue = ligne.strip_prefix("- `").and_then(|x| x.split_once("` : `")).and_then(|(rel, e)| e.strip_suffix('`').map(|e| (rel, e)));
            let Some((rel, e)) = lue.filter(|(_, e)| e.len() == 64 && e.bytes().all(|b| b.is_ascii_hexdigit())) else {
                return Err(socle::refus!("{} : la ligne « {} » de ses empreintes ne se lit pas (`moteur-atelier empreintes` la réécrit)", l.display(), ligne));
            };
            out.insert(format!("{}/{}", dossier, rel), e.to_string());
        }
    }
    Ok(out)
}

/// La commande `moteur-atelier empreintes` : ecrit les empreintes des textes deposes du depot ou elle est lancee.
pub fn commande(args: &[socle::ligne::Argument]) -> i32 {
    let args: &[String] = &socle::ligne::textes(args);
    if !args.is_empty() {
        eprintln!("usage : moteur-atelier empreintes (sans argument, depuis le dépôt)");
        return 2;
    }
    let r = std::env::current_dir().map_err(|e| socle::echec::refus(e.to_string())).and_then(|d| crate::verifier_avant_commit::racine_du_depot(&d)).and_then(|d| ecrire(&d));
    match r {
        Ok(n) => {
            println!("{} empreintes écrites dans les {} de {}.", n, LISEZ_MOI, TEXTES);
            0
        }
        Err(m) => {
            eprintln!("{}", m.en_lignes());
            1
        }
    }
}
