//! LA PUBLICATION D'UNE VERSION, chez l'editeur : les archives signees et `derniere.json`. Le flux « Publier » du depot
//! public (`.github/workflows/publier.yml`) fabrique `logiciel/`, attend l'approbation de l'editeur, signe par
//! `publier_depuis_l_environnement` et pose les archives, `derniere.json` et l'installeur dans une Release, que
//! `maj::DEPOT_PAR_DEFAUT` lit. Ce que les installations recoivent, verifient et appliquent est dans `compagnie::maj` ;
//! la forme des archives y est ecrite en tete : c'est un contrat avec toutes les installations posees. La seule cle qui
//! signe est celle de GitHub (`CLE_PUBLIQUE_GITHUB`), dont la graine n'existe que dans le travail approuve.

use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use compagnie::maj::{
    b64, condensat, courante, derniere_lue, ecrire_json, lire_depot, Archives, ArchivePubliee, FictivesPubliees, Publiee, nom_d_archive, nom_d_installation, plus_recente, programme_de,
    signature_valide, systeme_du_fichier, version_lue, CLE_PUBLIQUE_GITHUB, DERNIERE, LIVRAISON, LIVREE, LOGICIEL,
    PROPRES_A_L_INSTALLATION, SYSTEMES, TOUT_SYSTEME,
};

use socle::echec::{au_chemin, du_disque, refus, R};
use socle::fichiers::NomDeFichier;

/// La variable d'environnement ou le flux « Publier » pose la graine (base64) : lue par `publier_depuis_l_environnement`,
/// jamais affichee.
pub const VARIABLE_GRAINE: &str = "PUPPETPAIE_CLE_SIGNATURE";
/// Ce qui ne part jamais dans une archive, ni ne bouge a la bascule.
const EXCLUS: [&str; 6] = [".git", ".avant", ".apres", "__pycache__", ".pytest_cache", "target"];
/// Les programmes d'un systeme Unix, dans son dossier de `logiciel/` (`linux/`, `mac/`) : la fenetre, et le moteur (pour un
/// Linux sans WebKitGTK, qui ouvre la meme page dans le navigateur ; et le flux « Publier » signe avec celui de Linux).
/// Un zip ne garde le bit d'execution que si on l'y marque : `archiver` le marque sur eux, `deballer` le remet.
const PROGRAMMES_UNIX: [&str; 2] = ["puppetpaie", "moteur"];

fn signer(octets: &[u8], privee: &[u8]) -> R<String> {
    use ed25519_dalek::Signer;
    let k = <[u8; 32]>::try_from(privee).map_err(|_| refus("cle privee illisible (32 octets attendus)"))?;
    Ok(socle::base64::ecrire(&ed25519_dalek::SigningKey::from_bytes(&k).sign(octets).to_bytes()))
}

/// Les entrees du dossier `dossier`, dans l'ordre des noms.
fn entrees(dossier: &Path) -> R<Vec<std::fs::DirEntry>> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(dossier).map_err(|e| socle::echec::du_disque(dossier, &e))? {
        v.push(e.map_err(|e| socle::echec::du_disque(dossier, &e))?);
    }
    v.sort_by_key(|e| e.file_name());
    Ok(v)
}

/// L'entree `e` est-elle un dossier ?
fn est_un_dossier(e: &std::fs::DirEntry) -> R<bool> {
    Ok(e.file_type().map_err(|x| socle::echec::du_disque(&e.path(), &x))?.is_dir())
}

/// Le dossier qui porte `logiciel/` (`code`) : les fictives livrees y sont, a cote de lui.
fn base(code: &Path) -> R<&Path> {
    code.parent().ok_or_else(|| au_chemin("Le logiciel n'a pas de dossier parent : les fictives livrées ne s'y trouvent pas.", code))
}

/// Les fichiers de `logiciel/` pour l'archive, dans l'ordre des noms (meme contenu, meme condensat).
fn fichiers(code: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> R<()> {
    for e in entrees(&code.join(rel))? {
        let nom = e.file_name().to_string_lossy().into_owned();
        let r = rel.join(&nom);
        if est_un_dossier(&e)? {
            if !EXCLUS.contains(&nom.as_str()) {
                fichiers(code, &r, out)?;
            }
        } else if !nom.ends_with(".pyc") && !nom.ends_with(".pyo") && !(rel == Path::new("") && PROPRES_A_L_INSTALLATION.contains(&nom.as_str())) {
            out.push(r);
        }
    }
    Ok(())
}

/// Ce qu'une compagnie fictive porte sur un poste sans que ce soit la compagnie livree : ses secrets et son jeton (une
/// fictive ouverte pose elle aussi un coffre), le journal de la fenetre. Ni l'archive ni le depot ne les portent (les memes
/// exclusions que le `.gitignore` du depot public, « LES COMPAGNIES FICTIVES »).
/// La marque d'une livraison (`LIVREE`) non plus : la livraison l'ajoute elle-meme.
const HORS_D_UNE_FICTIVE_LIVREE: [&str; 5] =
    ["config/coffre.bin", routes::coffre::JETON_GOOGLE, "config/.jeton-interface", compagnie::installation::JOURNAL_INTERFACE, LIVREE];

/// LES COMPAGNIES FICTIVES LIVREES, a cote du logiciel (`<code>/../fictives/`) : les fichiers que porte l'archive
/// d'installation (`archiver_installation`), chemins relatifs a `fictives/`, dans l'ordre des noms. Une fictive est un dossier que son marqueur dit fictif
/// (`compagnies::est_fictive`) ; ce qui commence par un point (`.origine/`, les photos d'origine du poste, et les dossiers
/// d'une remise a zero en cours) n'en est pas. Sans aucune fictive, une archive decompressee ouvrirait un accueil vide
/// (le logiciel y est bride) : refus.
fn fictives_livrees(code: &Path) -> R<Vec<PathBuf>> {
    let dossier = base(code)?.join(compagnie::installation::DOSSIER_FICTIVES);
    if !dossier.is_dir() {
        return Err(au_chemin("Aucune compagnie fictive : leur dossier n'existe pas, et une archive sans elles s'ouvrirait bridée sur un accueil vide.", &dossier));
    }
    let mut out = Vec::new();
    let mut n = 0;
    for e in entrees(&dossier)? {
        let nom = e.file_name().to_string_lossy().into_owned();
        if nom.starts_with('.') || EXCLUS.contains(&nom.as_str()) {
            continue;
        }
        if !est_un_dossier(&e)? {
            out.push(PathBuf::from(&nom));
            continue;
        }
        // Une fictive AJOUTEE sur le poste porte les donnees qu'on y a mises : elle ne se livre jamais.
        if !compagnie::installation::est_fictive_livree(&e.path())? {
            continue;
        }
        n += 1;
        let mut dans = Vec::new();
        fichiers(&e.path(), Path::new(""), &mut dans)?;
        for r in dans {
            let rs = r.to_string_lossy().replace('\\', "/");
            if !HORS_D_UNE_FICTIVE_LIVREE.contains(&rs.as_str()) {
                out.push(Path::new(&nom).join(r));
            }
        }
    }
    if n == 0 {
        return Err(au_chemin("Aucune compagnie fictive dans leur dossier : une archive sans elles s'ouvrirait bridée sur un accueil vide.", &dossier));
    }
    Ok(out)
}

/// Une entree d'archive : son nom dans le zip, le fichier qu'elle porte, et si c'est un programme Unix (executable).
type Entree = (String, PathBuf, bool);

/// Les entrees du logiciel (`code`) pour un systeme (`SYSTEMES`) ou tous (`TOUT_SYSTEME`), chacune nommee `<tete><chemin>` :
/// le contenu de `logiciel/`, sans caches ni depot git ni version d'avant ; pour un systeme, les fichiers communs et les
/// siens seulement (`systeme_du_fichier`). Les programmes Unix (`linux/puppetpaie`, `mac/moteur`...) sont marques
/// executables, et eux seuls.
fn entrees_du_logiciel(code: &Path, systeme: &str, tete: &str) -> R<Vec<Entree>> {
    if systeme != TOUT_SYSTEME && !SYSTEMES.contains(&systeme) {
        return Err(socle::refus!("« {} » n'est pas un systeme", systeme));
    }
    let mut liste = Vec::new();
    fichiers(code, Path::new(""), &mut liste)?;
    let mut out = Vec::new();
    for rel in liste {
        let nom = rel.to_string_lossy().replace('\\', "/");
        let du_systeme = systeme_du_fichier(&nom);
        if systeme != TOUT_SYSTEME && du_systeme.is_some_and(|s| s != systeme) {
            continue;
        }
        let programme_unix = du_systeme.is_some_and(|s| s != "windows") && nom.rsplit('/').next().is_some_and(|n| PROGRAMMES_UNIX.contains(&n));
        out.push((format!("{}{}", tete, nom), code.join(&rel), programme_unix));
    }
    Ok(out)
}

/// Ce qui se livre des compagnies fictives (`fictives_livrees`) : les entrees nommees `fictives/<chemin>` ; les fichiers
/// que la livraison ajoute (la marque `.livree.json` de chaque fictive, et `.livraison.json`) ; et L'EMPREINTE de toute
/// la livraison. Une empreinte est le SHA-256 des lignes `<nom>\t<SHA-256 du fichier>`, dans l'ordre des entrees (pour
/// une fictive, ses seuls fichiers, nommes depuis son dossier) : elle ne depend que des fichiers livres et de leurs noms,
/// ni de la compression ni des dates, et deux publications aux memes fictives ont la meme.
fn entrees_des_fictives(code: &Path) -> R<(Vec<Entree>, Vec<(String, Vec<u8>)>, String)> {
    let racine = compagnie::installation::DOSSIER_FICTIVES;
    let dossier = base(code)?.join(racine);
    let hex = |h: Sha256| h.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();
    let mut entrees = Vec::new();
    let mut toute = Sha256::new();
    let mut par_fictive: std::collections::BTreeMap<String, Sha256> = std::collections::BTreeMap::new();
    for rel in fictives_livrees(code)? {
        let r = rel.to_string_lossy().replace('\\', "/");
        let chemin = dossier.join(&rel);
        let sha = condensat(&std::fs::read(&chemin).map_err(|e| du_disque(&chemin, &e))?);
        toute.update(format!("{}/{}\t{}\n", racine, r, sha).as_bytes());
        if let Some((fictive, dedans)) = r.split_once('/') {
            par_fictive.entry(fictive.to_string()).or_default().update(format!("{}\t{}\n", dedans, sha).as_bytes());
        }
        entrees.push((format!("{}/{}", racine, r), chemin, false));
    }
    let empreinte = hex(toute);
    let mut en_plus = Vec::new();
    let mut posees = std::collections::BTreeMap::new();
    for (nom, h) in par_fictive {
        let e = hex(h);
        en_plus.push((format!("{}/{}/{}", racine, nom, LIVREE), format!("{}\n", socle::ecriture::json_en_ligne(&compagnie::maj::Livree { empreinte: e.clone() })).into_bytes()));
        posees.insert(NomDeFichier::lire(&nom)?, e);
    }
    let l = socle::ecriture::json_indente(&compagnie::maj::Livraison { empreinte: Some(empreinte.clone()), posees }, 2).into_bytes();
    en_plus.push((format!("{}/{}", racine, LIVRAISON), l));
    Ok((entrees, en_plus, empreinte))
}

/// Un zip deterministe (dates fixes, ordre donne) : les `entrees`, puis les fichiers `en_plus` (nom, octets).
fn zipper(entrees: &[Entree], en_plus: &[(String, Vec<u8>)]) -> R<Vec<u8>> {
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let date = zip::DateTime::from_date_and_time(2020, 1, 1, 0, 0, 0)
        .map_err(|e| socle::echec::refus_avec("La date fixe des entrées de l'archive ne s'écrit pas.", socle::echec::Lieu { detail: Some(format!("{:?}", e)), ..socle::echec::Lieu::default() }))?;
    let options = |executable: bool| {
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(date)
            .unix_permissions(if executable { 0o755 } else { 0o644 })
    };
    for (nom, chemin, executable) in entrees {
        z.start_file(nom.as_str(), options(*executable)).map_err(|e| refus(e.to_string()))?;
        z.write_all(&std::fs::read(chemin).map_err(|e| du_disque(chemin, &e))?).map_err(|e| refus(e.to_string()))?;
    }
    for (nom, octets) in en_plus {
        z.start_file(nom.as_str(), options(false)).map_err(|e| refus(e.to_string()))?;
        z.write_all(octets).map_err(|e| refus(e.to_string()))?;
    }
    Ok(z.finish().map_err(|e| refus(e.to_string()))?.into_inner())
}

/// L'ARCHIVE DE MISE A JOUR d'un systeme (`SYSTEMES`) ou de tous (`TOUT_SYSTEME`) : LE CONTENU DE `logiciel/` A SA RACINE,
/// sans `fictives/`. Sa forme est un contrat avec toutes les installations posees (la tete de ce fichier) : elle ne change
/// plus. C'est elle que `derniere.json` nomme sous `archives`, que l'editeur signe, que `telecharger` prend.
fn archiver(code: &Path, systeme: &str) -> R<Vec<u8>> {
    zipper(&entrees_du_logiciel(code, systeme, "")?, &[])
}

/// L'ARCHIVE DES FICTIVES (`puppetpaie-<version>-fictives.zip`) : `fictives/`, ses compagnies fictives livrees, la marque
/// de chacune et `.livraison.json` (`entrees_des_fictives`), et l'empreinte de la livraison. Signee comme les archives de
/// mise a jour, nommee dans `derniere.json` sous `fictives` ; une installation ne la telecharge que si l'empreinte n'est
/// pas celle qu'elle retient (`preparer_les_fictives`).
fn archiver_fictives(code: &Path) -> R<(Vec<u8>, String)> {
    let (entrees, en_plus, empreinte) = entrees_des_fictives(code)?;
    Ok((zipper(&entrees, &en_plus)?, empreinte))
}

/// L'ARCHIVE D'INSTALLATION d'un systeme ou de tous : `logiciel/` (ce que porte l'archive de mise a jour,
/// sous `logiciel/`) et, a cote, `fictives/` (exactement ce que porte l'archive des fictives : une installation neuve
/// retient la livraison qu'elle a, et ne la retelecharge pas). Decompressee dans un dossier neuf, elle donne ce que pose
/// l'installeur Windows : une installation bridee, avec ses fictives. Elle ne sert qu'a installer : aucune
/// installation ne la telecharge, et `deballer` la refuse comme mise a jour. Elle pourrait se composer des deux autres
/// (les decompresser l'une dans `logiciel/`, l'autre a cote) ; elle reste une archive, pour qu'installer soit telecharger
/// un fichier et le decompresser.
/// Celle de TOUS LES SYSTEMES est le dossier portable : elle porte aussi, a sa racine, le marqueur `PORTABLE.json`
/// (`coffre::MARQUEUR_PORTABLE`) ; decompressee sur une cle USB, elle s'ouvre sous Windows, Linux et macOS, et la cle du
/// poste naitra a cote.
fn archiver_installation(code: &Path, systeme: &str) -> R<Vec<u8>> {
    let mut entrees = entrees_du_logiciel(code, systeme, &format!("{}/", LOGICIEL))?;
    let (fictives, mut en_plus, _) = entrees_des_fictives(code)?;
    entrees.extend(fictives);
    if systeme == TOUT_SYSTEME {
        en_plus.push((compagnie::coffre::MARQUEUR_PORTABLE.to_string(), compagnie::coffre::CONTENU_PORTABLE.as_bytes().to_vec()));
    }
    zipper(&entrees, &en_plus)
}

/// Le nom de l'archive des fictives d'une version : `puppetpaie-<version>-fictives.zip`.
fn nom_des_fictives(version: &str) -> R<NomDeFichier> {
    NomDeFichier::lire(&format!("puppetpaie-{}-fictives.zip", version))
}

/// LES MARQUES DE LA LIVRAISON, posees dans les fictives livrees a cote de `code` (`moteur maj --marquer-fictives --logiciel
/// L`) : `fictives/.livraison.json` et `fictives/<nom>/.livree.json`, exactement ce que porte l'archive des fictives. Le
/// flux « Publier » les pose avant de fabriquer l'installeur Windows, qui les pose a son tour : une installation de
/// l'installeur retient sa livraison comme une installation de l'archive, et ne retelecharge ni ne remplace des fictives
/// qui n'ont pas change. Rien de ce que porte une livraison n'en depend (`fictives_livrees` ne les livre pas). Rend
/// l'empreinte.
///
/// D'ABORD, LES FICTIVES DE REFERENCE (`scenario::fictives_de_reference`) : chacune se refait de son scenario, rejoue
/// jusqu'au jour de la publication (`scenario::livrer`), dans `fictives/<nom>` -- ce qu'une compagnie bien tenue a fait
/// avant ce jour ; la suite est au visiteur. Un rejeu qui s'ecarte arrete la publication.
pub fn marquer_les_fictives(code: &Path) -> R<String> {
    let base = base(code)?;
    if std::env::var_os(socle::garde_reseau::VARIABLE).is_none() {
        std::env::set_var(socle::garde_reseau::VARIABLE, "1");
    }
    for (nom, scenario) in routes::scenario::fictives_de_reference(base)? {
        let dest = base.join(compagnie::installation::DOSSIER_FICTIVES).join(&nom);
        routes::scenario::livrer(&scenario, &dest, Some(droit::contexte::jour_reel()))?;
    }
    let (_, en_plus, empreinte) = entrees_des_fictives(code)?;
    for (nom, octets) in en_plus {
        std::fs::write(base.join(&nom), octets).map_err(|e| socle::refus!("{} : {}", nom, e))?;
    }
    Ok(empreinte)
}

/// LES FICTIVES QUE POSE L'INSTALLEUR WINDOWS (`moteur maj --poser-fictives DOSSIER --logiciel L`) : dans `DOSSIER`,
/// exactement ce que porte l'archive des fictives (`entrees_des_fictives`), `fictives/` et ses marques comprises. Les
/// fictives livrees se refont de leur scenario et ne sont pas suivies par Git : l'installeur les prend ici, pas dans le
/// depot. Rend l'empreinte.
pub fn poser_les_fictives(code: &Path, dossier: &Path) -> R<String> {
    let (entrees, en_plus, empreinte) = entrees_des_fictives(code)?;
    let ecrire = |nom: &str, octets: &[u8]| -> R<()> {
        let dest = dossier.join(nom);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p).map_err(|e| du_disque(p, &e))?;
        }
        std::fs::write(&dest, octets).map_err(|e| du_disque(&dest, &e))
    };
    for (nom, chemin, _) in entrees {
        ecrire(&nom, &std::fs::read(&chemin).map_err(|e| du_disque(&chemin, &e))?)?;
    }
    for (nom, octets) in en_plus {
        ecrire(&nom, &octets)?;
    }
    Ok(empreinte)
}

/// La cle de GitHub fabriquee : le fichier de la graine (secrete), celui de la cle publique, et la cle publique.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CleDeGithub {
    pub graine: String,
    pub publique: String,
    pub cle_publique: String,
}

/// LA PUBLICATION, le logiciel, la graine et la cle publique donnes : les archives de `c` (une par systeme dont la
/// fenetre y est, et celle de tous les systemes, qui porte ceux-la et le dit sous `systemes`), chacune signee et verifiee,
/// et `derniere.json` qui les nomme, dans le dossier `d` ; a cote, les archives d'installation (Linux, Mac, tous), ni
/// signees ni nommees, rendues sous `installations`.
fn publier_signe(c: &Path, d: &Path, privee: &[u8], cle_publique: &str, notes: &str, calcul: &str, exigee: &str) -> R<Publication> {
    let version = courante(c)?;
    let depot = d.display().to_string();
    std::fs::create_dir_all(&d).map_err(|e| refus(e.to_string()))?;
    if d.join(DERNIERE).exists() {
        // Le `derniere.json` de la Release precedente, lu comme chaque installation le lit (`derniere_lue`).
        let avant = derniere_lue(&lire_depot(&depot, &const { NomDeFichier::du_moteur(DERNIERE) })?)?;
        let av = avant.version.as_str();
        if !plus_recente(&version, av)? {
            return Err(socle::refus!(
                "le depot est deja en {} ; augmenter VERSION (aujourd'hui {}, le fichier s'ecrit AAAA.MM.JJ : {}) avant de publier",
                version_lue(av),
                version_lue(&version),
                version
            ));
        }
    }
    // Une archive par systeme dont la fenetre est dans `logiciel/` (le flux les pose toutes trois ; un poste qui publie
    // depuis son propre `logiciel/` n'a que la sienne), et celle de tous les systemes. Sans aucune fenetre, aucune
    // installation ne pourrait lancer la version : rien n'est publie.
    let presents: Vec<&str> = SYSTEMES.into_iter().filter(|s| programme_de(c, s, "puppetpaie").is_file()).collect();
    if presents.is_empty() {
        let attendues = SYSTEMES.map(|s| programme_de(Path::new(""), s, "puppetpaie").to_string_lossy().replace('\\', "/")).join(", ");
        return Err(socle::echec::refus_avec(
            "Le logiciel n'a la fenêtre d'aucun système : aucune installation ne pourrait lancer cette version.",
            socle::echec::Lieu { chemin: Some(c.to_string_lossy().into_owned()), detail: Some(format!("attendue : {}", attendues)), ..socle::echec::Lieu::default() },
        ));
    }
    let mut archives = Archives::default();
    let mut tailles: Vec<(NomDeFichier, usize)> = Vec::new();
    let mut faites: Vec<(NomDeFichier, Vec<u8>)> = Vec::new();
    for systeme in presents.iter().copied().chain([TOUT_SYSTEME]) {
        let octets = archiver(c, systeme)?;
        let signature = signer(&octets, privee)?;
        if !signature_valide(&octets, &signature, cle_publique) {
            return Err(autre_graine());
        }
        let nom = nom_d_archive(&version, systeme)?;
        let systemes = (systeme == TOUT_SYSTEME).then(|| presents.iter().map(|s| s.to_string()).collect());
        archives.poser(systeme, ArchivePubliee { archive: nom.clone(), sha256: condensat(&octets), signature, systemes });
        tailles.push((nom.clone(), octets.len()));
        faites.push((nom, octets));
    }
    // LES FICTIVES, a part (`archiver_fictives`) : signees comme les archives, nommees sous `fictives` avec leur empreinte.
    let (octets, empreinte) = archiver_fictives(c)?;
    let signature = signer(&octets, privee)?;
    if !signature_valide(&octets, &signature, cle_publique) {
        return Err(autre_graine());
    }
    let nom = nom_des_fictives(&version)?;
    let fictives = FictivesPubliees { archive: nom.clone(), sha256: condensat(&octets), signature, empreinte };
    tailles.push((nom.clone(), octets.len()));
    faites.push((nom, octets));
    // LES ARCHIVES D'INSTALLATION, a cote (`archiver_installation`) : Linux et Mac s'ils sont publies, et tous les
    // systemes (le dossier portable) ; Windows a son installeur. Ni signees ni nommees dans `derniere.json` : aucune
    // installation ne les telecharge.
    for systeme in presents.iter().copied().filter(|s| *s != "windows").chain([TOUT_SYSTEME]) {
        let octets = archiver_installation(c, systeme)?;
        let nom = nom_d_installation(&version, systeme)?;
        tailles.push((nom.clone(), octets.len()));
        faites.push((nom, octets));
    }
    // Rien n'est ecrit avant que toutes soient signees et verifiees.
    for (nom, octets) in &faites {
        std::fs::write(d.join(nom), octets).map_err(|e| refus(e.to_string()))?;
    }
    // LES INSTALLATIONS POSEES QUI NE CONNAISSENT QU'UNE ARCHIVE (celle de Windows) lisent a la racine `archive`,
    // `sha256` et `signature` : l'archive de Windows y est nommee, et elles se mettent a jour.
    let windows = archives.windows.clone();
    let manifeste = Publiee {
        version,
        publiee_le: socle::temps::secondes().jour_de_paris(),
        notes: notes.to_string(),
        calcul: calcul.to_string(),
        exigee: exigee.to_string(),
        archives,
        fictives,
        archive: windows.as_ref().map(|w| w.archive.clone()),
        sha256: windows.as_ref().map(|w| w.sha256.clone()),
        signature: windows.map(|w| w.signature),
    };
    ecrire_json(&d.join(DERNIERE), &manifeste)?;
    Ok(Publication { manifeste, tailles, dossier: d.to_path_buf() })
}

/// Ce qu'une publication a pose : `derniere.json` (`manifeste`), chaque archive et sa taille en octets (celles de mise a
/// jour, celle des fictives, puis celles d'installation), et le dossier.
pub struct Publication {
    pub manifeste: Publiee,
    pub tailles: Vec<(NomDeFichier, usize)>,
    pub dossier: PathBuf,
}

impl Publication {
    /// Ce que `moteur maj --publier-github` dit des archives publiees : leur nom et leur taille.
    pub(crate) fn dire_les_archives(&self) -> String {
        self.tailles.iter().map(|(nom, n)| format!("{} ({} Ko)", nom, n / 1024)).collect::<Vec<_>>().join(", ")
    }
}

/// LA CLE DE GITHUB, fabriquee sur le poste de l'editeur (`moteur maj --cle-github [DOSSIER]`). Une cle neuve,
/// dediee : la GRAINE (base64) va dans un fichier, `cle-github-graine.txt`, que l'editeur ouvre pour la coller dans le
/// secret `PUPPETPAIE_CLE_SIGNATURE` de l'environnement `publication`, puis efface ; elle ne passe jamais a l'ecran ni
/// dans un journal. La cle PUBLIQUE (pas secrete) va dans `cle-github-publique.txt` et dans ce qui est rendu : elle se
/// reporte dans `CLE_PUBLIQUE_GITHUB`. Refuse si la graine d'une cle precedente est encore la (rien n'est ecrase).
pub fn generer_cle_github(dossier: &Path) -> R<CleDeGithub> {
    let fichier = dossier.join("cle-github-graine.txt");
    let fichier_publique = dossier.join("cle-github-publique.txt");
    if fichier.exists() {
        return Err(au_chemin(
            "La graine d'une clé précédente est encore là : la coller dans le secret GitHub si elle n'y est pas, puis l'effacer, avant d'en faire une autre.",
            &fichier,
        ));
    }
    std::fs::create_dir_all(dossier).map_err(|e| refus(e.to_string()))?;
    let mut graine = [0u8; 32];
    getrandom::fill(&mut graine).map_err(|e| refus(e.to_string()))?;
    let publique = socle::base64::ecrire(ed25519_dalek::SigningKey::from_bytes(&graine).verifying_key().as_bytes());
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let mut f = o.open(&fichier).map_err(|e| du_disque(&fichier, &e))?;
    f.write_all(socle::base64::ecrire(&graine).as_bytes()).map_err(|e| refus(e.to_string()))?;
    graine.fill(0);
    std::fs::write(&fichier_publique, format!("{}\n", publique)).map_err(|e| refus(e.to_string()))?;
    Ok(CleDeGithub { graine: fichier.display().to_string(), publique: fichier_publique.display().to_string(), cle_publique: publique })
}

/// `publier_signe` depuis une graine en base64 (celle du secret de GitHub), verifiee contre `cle_publique` : refuse si
/// la cle publique n'est pas reportee (vide) ou si la graine n'est pas la sienne. Aucun message ne porte la graine.
fn publier_par_graine(logiciel: &Path, sortie: &Path, graine_b64: &str, cle_publique: &str, notes: &str, calcul: &str, exigee: &str) -> R<Publication> {
    if cle_publique.is_empty() {
        return Err(socle::echec::refus_avec(
            "La clé publique de GitHub n'est pas reportée dans le logiciel : rien n'est publié.",
            socle::echec::Lieu { cle: Some("CLE_PUBLIQUE_GITHUB".to_string()), ..socle::echec::Lieu::default() },
        ));
    }
    // Le refus ne dit pas pourquoi la base64 ne se lit pas : il ne porte rien de la graine.
    let graine = match b64(graine_b64.trim()) {
        Ok(g) if g.len() == 32 => g,
        _ => return Err(socle::refus!("{} illisible (32 octets en base64 attendus)", VARIABLE_GRAINE)),
    };
    publier_signe(logiciel, sortie, &graine, cle_publique, notes, calcul, exigee)
}

/// Le refus d'une graine qui n'est pas celle de la cle publique : rien n'est publie.
#[track_caller]
fn autre_graine() -> socle::echec::Echec {
    socle::echec::refus_avec(
        "La graine de signature n'est pas celle de la clé publique du logiciel : rien n'est publié.",
        socle::echec::Lieu { cle: Some(VARIABLE_GRAINE.to_string()), ..socle::echec::Lieu::default() },
    )
}

/// LA PUBLICATION PAR LE FLUX DE GITHUB (`moteur maj --publier-github --logiciel L --sortie S`) : les archives de `L`
/// signees par la graine de `PUPPETPAIE_CLE_SIGNATURE`, verifiee contre `CLE_PUBLIQUE_GITHUB`, et `derniere.json`, dans
/// `S`. Si `S` porte deja le `derniere.json` de la derniere Release (le flux l'y telecharge), une version qui n'est pas
/// plus recente est refusee (`publier_signe`). Ni le reglage « editeur » ni le coffre n'y jouent : le secret n'existe
/// que dans le travail approuve par l'editeur.
pub fn publier_depuis_l_environnement(logiciel: &Path, sortie: &Path, notes: &str, calcul: &str, exigee: &str) -> R<Publication> {
    // Le refus ne dit rien de la valeur : c'est la graine.
    let g = match std::env::var(VARIABLE_GRAINE) {
        Ok(g) if !g.trim().is_empty() => g,
        Ok(_) | Err(std::env::VarError::NotPresent) => return Err(socle::refus!("{} absente : rien n'est publie", VARIABLE_GRAINE)),
        Err(std::env::VarError::NotUnicode(_)) => return Err(socle::refus!("{} n'est pas du texte : rien n'est publie", VARIABLE_GRAINE)),
    };
    publier_par_graine(logiciel, sortie, &g, CLE_PUBLIQUE_GITHUB, notes, calcul, exigee)
}

#[cfg(test)]
mod tests {
    use super::*;
    use compagnie::maj::{courante, deballer, signature_valide, CLE_PUBLIQUE, TOUT_SYSTEME};

    /// Une compagnie fictive livree a cote du logiciel : `<base>/fictives/fictive-essai/` (son marqueur et sa fiche), et le
    /// LISEZ-MOI du dossier.
    fn une_fictive(base: &Path) {
        let f = base.join("fictives").join("fictive-essai");
        std::fs::create_dir_all(f.join(socle::fichiers::CONFIG)).unwrap();
        std::fs::write(f.join("COPIE-DE-DEVELOPPEMENT.json"), "{\"copie_de\": \"compagnie fictive (essai)\", \"quoi\": \"essai\"}").unwrap();
        std::fs::write(f.join(droit::contexte::ENTREPRISE), "{\"nom_affichage\": \"Essai\"}").unwrap();
        std::fs::write(base.join("fictives").join("LISEZ-MOI.md"), "Les fictives.\n").unwrap();
    }

    fn noms_de(octets: &[u8]) -> Vec<String> {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(octets)).unwrap();
        (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect()
    }

    /// L'archive d'un logiciel, signee par une cle neuve, se verifie avec sa cle publique et se deballe a l'identique.
    #[test]
    fn archive_signee_deballee() {
        let base = std::env::temp_dir().join(format!("puppetpaie-maj-{}", std::process::id()));
        let code = base.join("logiciel");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(code.join("donnees")).unwrap();
        std::fs::create_dir_all(code.join("target")).unwrap();
        std::fs::write(code.join("VERSION"), "2026.10.09\n").unwrap();
        std::fs::write(code.join("donnees").join("a.json"), "{}").unwrap();
        std::fs::write(code.join("target").join("gros"), "a ne pas livrer").unwrap();
        // L'archive de mise a jour ne demande aucune fictive ; celle d'installation, si : decompressee, elle s'ouvrirait
        // bridee sur un accueil vide.
        let octets = archiver(&code, TOUT_SYSTEME).unwrap();
        assert!(archiver_installation(&code, TOUT_SYSTEME).unwrap_err().phrase.contains("Aucune compagnie fictive"));
        une_fictive(&base);
        // Ce qui est du poste et pas de la fictive livree : sa photo d'origine, ses secrets, le journal
        // de la fenetre ; et un dossier qui n'est pas une fictive.
        let f = base.join("fictives");
        for (rel, t) in [
            (".origine/fictive-essai/config/entreprise.json", "{}"),
            ("fictive-essai/config/coffre.bin", "scelle"),
            ("fictive-essai/journal/interface.log", "chemin du poste"),
            ("pas-une-fictive/config/entreprise.json", "{}"),
        ] {
            std::fs::create_dir_all(f.join(rel).parent().unwrap()).unwrap();
            std::fs::write(f.join(rel), t).unwrap();
        }
        // LE CONTRAT : le contenu de `logiciel/` a la racine, rien d'autre, et les fictives n'y changent rien.
        assert_eq!(octets, archiver(&code, TOUT_SYSTEME).unwrap(), "le zip est deterministe, et sans fictives");
        let mut noms = noms_de(&octets);
        noms.sort();
        assert_eq!(noms, ["VERSION", "donnees/a.json"]);
        let installation = archiver_installation(&code, TOUT_SYSTEME).unwrap();
        let noms = noms_de(&installation);
        assert!(noms.contains(&"logiciel/VERSION".to_string()) && noms.contains(&"logiciel/donnees/a.json".to_string()), "{:?}", noms);
        assert!(noms.contains(&"fictives/LISEZ-MOI.md".to_string()) && noms.contains(&"fictives/fictive-essai/config/entreprise.json".to_string()), "{:?}", noms);
        assert!(noms.contains(&"fictives/fictive-essai/COPIE-DE-DEVELOPPEMENT.json".to_string()), "{:?}", noms);
        // Celle de tous les systemes est le dossier portable : son marqueur a la racine, que le moteur lit (`coffre::dossier_portable`).
        assert!(noms.contains(&compagnie::coffre::MARQUEUR_PORTABLE.to_string()), "{:?}", noms);
        // Les trois fichiers livres, la marque de la fictive et ce que retient la livraison : rien du poste.
        assert!(noms.contains(&"fictives/fictive-essai/.livree.json".to_string()) && noms.contains(&"fictives/.livraison.json".to_string()), "{:?}", noms);
        assert_eq!(noms.iter().filter(|n| n.starts_with("fictives/")).count(), 5, "rien du poste : {:?}", noms);
        assert_eq!(installation, archiver_installation(&code, TOUT_SYSTEME).unwrap(), "le zip est deterministe");
        // L'archive des fictives porte exactement le `fictives/` de l'archive d'installation ; son empreinte ne depend que
        // du contenu livre.
        let (fictives, empreinte) = archiver_fictives(&code).unwrap();
        assert_eq!(noms_de(&fictives), noms.iter().filter(|n| n.starts_with("fictives/")).cloned().collect::<Vec<_>>());
        std::fs::write(f.join("fictive-essai/config/coffre.bin"), "autre scelle").unwrap();
        assert_eq!(archiver_fictives(&code).unwrap().1, empreinte, "ce que la livraison ne porte pas ne change pas l'empreinte");
        std::fs::write(f.join("fictive-essai/config/entreprise.json"), "{\"nom_affichage\": \"Autre\"}").unwrap();
        assert_ne!(archiver_fictives(&code).unwrap().1, empreinte, "ce qu'elle porte, si");
        let graine = [7u8; 32];
        let k = ed25519_dalek::SigningKey::from_bytes(&graine);
        let publique = socle::base64::ecrire(k.verifying_key().as_bytes());
        let sig = signer(&octets, &graine).unwrap();
        assert!(signature_valide(&octets, &sig, &publique));
        assert!(!signature_valide(&octets, &sig, CLE_PUBLIQUE));
        let mut altere = octets.clone();
        let n = altere.len() / 2;
        altere[n] ^= 1;
        assert!(!signature_valide(&altere, &sig, &publique));
        let neuf = base.join("maj-neuve").join("logiciel");
        deballer(&octets, &neuf).unwrap();
        assert_eq!(courante(&neuf).unwrap(), "2026.10.09");
        assert!(neuf.join("donnees").join("a.json").is_file());
        assert!(!neuf.join("target").exists());
        // Une archive d'installation n'est pas une mise a jour : refusee, rien n'est ecrit.
        let ailleurs = base.join("maj-installation").join("logiciel");
        assert!(deballer(&installation, &ailleurs).unwrap_err().phrase.contains("archive d'installation"));
        assert!(!ailleurs.exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}

