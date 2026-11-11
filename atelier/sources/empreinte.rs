// L'EMPREINTE DES SOURCES, que les `build.rs` de l'espace incluent (`include!`) : FNV-1a 64 bits sur les fichiers des
// sources de la crate et de celles dont elle depend (`dossiers_des_sources`, lues dans les manifestes) et sur le verrou de
// l'espace, dans l'ordre des noms ; chaque dossier, chaque manifeste et le verrou refont l'empreinte quand ils changent. Un dossier ou un fichier qui ne se lit pas arrete la construction en le disant : une empreinte qui
// l'aurait saute ne dirait plus ce qui est compile.

fn illisible(chemin: &std::path::Path, e: std::io::Error) -> ! {
    panic!("l'empreinte des sources ne lit pas {} : {}", chemin.display(), e)
}

fn fichiers(dossier: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let mut v: Vec<std::path::PathBuf> = Vec::new();
    for e in std::fs::read_dir(dossier).unwrap_or_else(|e| illisible(dossier, e)) {
        v.push(e.unwrap_or_else(|e| illisible(dossier, e)).path());
    }
    v.sort();
    for p in v {
        if p.is_dir() {
            fichiers(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// LES DOSSIERS QUE LA MARQUE COUVRE, lus dans les manifestes : les sources (`src`) de chaque crate de l'espace dont la
/// crate qui se construit depend, de proche en proche, dans l'ordre ou ses manifestes les nomment, puis les siennes.
fn dossiers_des_sources() -> Vec<std::path::PathBuf> {
    let mut crates = vec![std::path::PathBuf::from(".")];
    let mut i = 0;
    while i < crates.len() {
        let manifeste = crates[i].join("Cargo.toml");
        let texte = std::fs::read_to_string(&manifeste).unwrap_or_else(|e| illisible(&manifeste, e));
        println!("cargo:rerun-if-changed={}", manifeste.display());
        for nom in dependances_de_l_espace(&texte) {
            let d = std::path::Path::new("..").join(nom);
            if !crates.contains(&d) {
                crates.push(d);
            }
        }
        i += 1;
    }
    let mut dossiers: Vec<std::path::PathBuf> = crates[1..].iter().map(|c| c.join("src")).collect();
    dossiers.push(std::path::PathBuf::from("src"));
    dossiers
}

/// Les crates de l'espace dont un manifeste depend : le dernier element du `path` de chaque dependance de ses tables
/// `[dependencies]` (et `[target.….dependencies]`). Celles de ses essais (`[dev-dependencies]`) et de sa construction
/// (`[build-dependencies]`) ne sont pas compilees dans la crate.
fn dependances_de_l_espace(manifeste: &str) -> Vec<String> {
    let mut des_dependances = false;
    let mut noms = Vec::new();
    for ligne in manifeste.lines().map(str::trim) {
        if ligne.starts_with('[') {
            des_dependances = ligne.ends_with("dependencies]") && !ligne.ends_with("dev-dependencies]") && !ligne.ends_with("build-dependencies]");
            continue;
        }
        if !des_dependances {
            continue;
        }
        if let Some((chemin, _)) = ligne.split_once("path = \"").and_then(|(_, reste)| reste.split_once('"')) {
            if let Some(nom) = std::path::Path::new(chemin).file_name() {
                noms.push(nom.to_string_lossy().into_owned());
            }
        }
    }
    noms
}

fn empreinte_des_sources(verrou: &str) -> u64 {
    let dossiers = dossiers_des_sources();
    let mut liste = Vec::new();
    for d in &dossiers {
        fichiers(d, &mut liste);
    }
    liste.push(std::path::PathBuf::from(verrou));
    let mut h: u64 = 0xcbf29ce484222325;
    for p in &liste {
        let nom = p.to_string_lossy().replace('\\', "/");
        let contenu = std::fs::read(p).unwrap_or_else(|e| illisible(p, e));
        for o in nom.bytes().chain([0]).chain(contenu) {
            h = (h ^ o as u64).wrapping_mul(0x100000001b3);
        }
    }
    for d in &dossiers {
        println!("cargo:rerun-if-changed={}", d.display());
    }
    println!("cargo:rerun-if-changed={}", verrou);
    h
}
