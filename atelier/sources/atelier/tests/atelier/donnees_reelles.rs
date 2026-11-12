//! Le verrou des données réelles (règle de fond 9a), sur ce que Git emporterait hors de `prive/`, en texte : la liste
//! du garde avant commit (`verifier_avant_commit::ce_qui_part`, suivis et non suivis, les `.gitignore` appliqués par Git
//! lui-même), lue une fois pour deux verrous. Nulle part un NIR qui ne soit pas inventé (`nirs_non_inventes`, sauf
//! un texte déposé tel que son émetteur l'a publié, que son empreinte prouve : `atelier::empreintes`) ; dans le code et ce que reçoit une
//! compagnie (`DU_CODE`), pas le nom de la compagnie du poste (règle 0 : le logiciel est pour toute compagnie), hors de
//! la mention de l'auteur (`SIGNATURE`). Et aucun test ne construit un chemin vers `prive/`. Il ne voit ni les adresses
//! ni les noms de personnes : les écrire ici pour les chercher, ce serait les publier.
//!
//! Un NIR inventé porte `999`, `123456` ou `000` dans ses treize chiffres (une commune ou un rang que personne n'a) ; un
//! numéro qui ne suit pas la convention est tenu pour réel, et refusé. L'échec dit le fichier et la ligne, jamais le
//! numéro : la sortie des tests se lit dans les journaux publics des flux.

use std::path::{Path, PathBuf};

use regex::Regex;

use atelier::verifier_avant_commit::{ce_qui_part, depose_tel_quel, nirs_non_inventes, programme_compile};

/// En deux morceaux, pour que ce fichier ne porte pas lui-même ce qu'il refuse.
const NOM_REEL: &str = concat!("ueu", "eue");

/// Le code et ce que reçoit une compagnie, relatifs à la racine du dépôt : là où un cas propre à la compagnie du poste
/// n'a pas sa place. Le reste du dépôt peut nommer l'éditeur : les textes déposés (`atelier/reference/`), la
/// documentation (`atelier/docs/`, les `LISEZ-MOI`, `AGENTS.md`), les maquettes, les licences. Aucune structure ne
/// sépare l'un de l'autre (les lanceurs, `.claude/`, `.github/`, les outils n'ont pas de `Cargo.toml`) : la liste le dit.
const DU_CODE: [&str; 10] = [
    "logiciel/",
    "fictives/",
    "atelier/fictives/",
    "atelier/sources/",
    "atelier/outils/",
    "atelier/installeur/",
    ".claude/",
    ".github/",
    "PuppetPaie.cmd",
    "atelier/PuppetPaie.",
];

/// LA SIGNATURE DE L'EDITEUR : l'editeur et la compagnie du poste portent le meme nom, et le logiciel le signe
/// (`compagnie::auteur` : la mention de l'auteur et ses sites). Le verrou du nom laisse passer ces valeurs-la, telles que
/// le logiciel les porte, et elles seules : il ne nomme aucun fichier.
fn signature() -> Vec<String> {
    let a = compagnie::auteur::auteur();
    let mut v: Vec<String> = [a.phrase, a.nom, a.adresse].into_iter().chain(compagnie::auteur::SITES).map(str::to_lowercase).collect();
    // La plus longue d'abord : la phrase contient le nom.
    v.sort_by_key(|s| std::cmp::Reverse(s.len()));
    v
}

fn depot() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn part() -> Vec<String> {
    ce_qui_part(&depot(), false).unwrap_or_else(|m| panic!("ce verrou lit ce que Git emporterait : {}", m.phrase))
}

fn ligne(o: &[u8], i: usize) -> usize {
    o[..i].iter().filter(|&&b| b == b'\n').count() + 1
}

#[test]
fn rien_de_la_compagnie_reelle_n_est_ecrit_dans_ce_qui_part() {
    let (mut nirs, mut noms) = (Vec::new(), Vec::new());
    let deposes = atelier::empreintes::lues(&depot()).unwrap_or_else(|m| panic!("les empreintes des textes déposés se lisent : {}", m.phrase));
    let signature = signature();
    for rel in part() {
        // Ce que Git emporterait et qui ne se lit pas n'a pas été fouillé : le verrou échoue en le nommant.
        let o = match std::fs::read(depot().join(&rel)) {
            Ok(o) => o,
            Err(e) => panic!("{} part avec Git et ne se lit pas : {}", rel, e),
        };
        if DU_CODE.iter().any(|d| rel.starts_with(d)) && rel.to_lowercase().contains(NOM_REEL) {
            noms.push(rel.clone());
        }
        // Un programme compilé se fabrique depuis les sources fouillées ici : ses constantes ont la forme d'un NIR, et il
        // porte l'identifiant de l'application que ses sources disent. Son contenu ne se fouille pas.
        if programme_compile(&o) {
            continue;
        }
        if !depose_tel_quel(&deposes, &rel, &o) {
            nirs.extend(nirs_non_inventes(&o).into_iter().map(|(i, _)| format!("{}:{}", rel, ligne(&o, i))));
        }
        if DU_CODE.iter().any(|d| rel.starts_with(d)) {
            // La signature de l'editeur, blanchie a sa place (meme longueur : les lignes restent).
            let mut bas = String::from_utf8_lossy(&o).to_lowercase();
            for s in &signature {
                bas = bas.replace(s.as_str(), &" ".repeat(s.len()));
            }
            let position = bas.find(NOM_REEL);
            noms.extend(position.map(|i| format!("{}:{}", rel, ligne(bas.as_bytes(), i))));
        }
    }
    let mut echec = String::new();
    if !nirs.is_empty() {
        echec += &format!("NIR sans la marque d'un numéro inventé (999, 123456 ou 000) : {:#?}\n", nirs);
    }
    if !noms.is_empty() {
        echec += &format!("le logiciel est pour toute compagnie, il ne porte pas le nom de son banc d'essai : {:#?}\n", noms);
    }
    assert!(echec.is_empty(), "{}", echec);
}

/// Un chemin vers les données du poste construit depuis le code : le parent d'un chemin suivi de `prive`, ou un relatif
/// qui remonte. Un dossier de test qui s'appelle `prive` (`monde().join("prive")`) n'est pas visé.
#[test]
fn aucun_test_ne_construit_un_chemin_vers_prive() {
    let re = Regex::new(r#"(?:parent\(\)|"\.\.")[^;\n]*?\.join\(\s*"prive"|\.\./prive"#).unwrap();
    let mut trouves = Vec::new();
    for rel in part() {
        let dans_les_tests = rel.strip_prefix("atelier/sources/").is_some_and(|r| r.split('/').any(|c| c == "tests"));
        if !dans_les_tests || !rel.ends_with(".rs") {
            continue;
        }
        let t = std::fs::read_to_string(depot().join(&rel)).unwrap();
        trouves.extend(re.find_iter(&t).map(|m| format!("{}:{}", rel, ligne(t.as_bytes(), m.start()))));
    }
    assert!(trouves.is_empty(), "{:#?}", trouves);
}
