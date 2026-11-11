// L'EMPREINTE DES SOURCES, que les `build.rs` de l'espace incluent (`include!`) : FNV-1a 64 bits sur les fichiers des
// dossiers dits et sur le verrou de l'espace, dans l'ordre des noms ; chaque dossier et le verrou refont l'empreinte quand
// ils changent. Un dossier ou un fichier qui ne se lit pas arrete la construction en le disant : une empreinte qui
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

fn empreinte_des_sources(dossiers: &[&str], verrou: &str) -> u64 {
    let mut liste = Vec::new();
    for d in dossiers {
        fichiers(std::path::Path::new(d), &mut liste);
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
    for d in dossiers {
        println!("cargo:rerun-if-changed={}", d);
    }
    println!("cargo:rerun-if-changed={}", verrou);
    h
}
