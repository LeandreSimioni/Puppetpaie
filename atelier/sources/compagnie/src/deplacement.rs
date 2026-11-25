//! DEPLACER LE LOGICIEL : le dossier de l'installation (celui qui porte `logiciel/`, et a cote les compagnies, `maj/`,
//! les fictives) va ailleurs, entier, LOGICIEL FERME (Leandre, 25/11/2026 : « on propose Documents de base et les gens
//! peuvent changer », « a l'installation et dans un reglage dans le logiciel »).
//!
//! LE GESTE. La route `logiciel-deplacer` (`routes::accueil`) dit ou est le logiciel ; un dossier dans sa charge, elle
//! verifie tout ce qui se verifie avant (`verifier`) et pose la demande (`maj/deplacer.json`, `demander`). Le
//! deplacement se fait au lancement, comme une mise a jour (`au_lancement`, avant toute fenetre) ; la fenetre qui a servi
//! la route le lance aussitot et se ferme (`lancer_s_il_est_demande`).
//!
//! LE RELAIS, HORS DU DOSSIER. Windows ne deplace pas un dossier qui porte un programme en marche, ni le dossier courant
//! d'un processus. Le programme de la fenetre se copie donc dans le dossier temporaire du poste
//! (`PuppetPaie-deplacement-<numero>/`, avec le chargeur de WebView2 qu'il charge au demarrage), s'y lance avec
//! `deplacer-relais`, son dossier courant la-bas, et le lanceur s'arrete sans rien ouvrir. Le relais attend sa fin
//! (`maj::attendre_fin`), deplace, et rouvre la fenetre au nouvel endroit -- ou a l'ancien, si rien n'a bouge. Le
//! relais de la mise a jour, lui, reste dans `maj/` : il ne deplace pas l'installation.
//!
//! LE DEPLACEMENT (`deplacer`). Chaque entree de l'installation se RENOMME dans le nouveau dossier quand c'est le meme
//! disque : instantane, sans place en double. Un autre disque (le systeme refuse le renommage, `CrossesDevices`) : tout se
//! COPIE, la copie se compare a l'original (chaque fichier, sa taille, chaque dossier), puis l'original s'efface. Ce qui
//! pointe vers l'ancien dossier se reecrit ensuite (sous Windows, la cle de desinstallation et le raccourci du menu
//! Demarrer que l'installeur a poses, `repointer`). AU MOINDRE ECHEC, TOUT REVIENT (les renommes repartent, la copie
//! s'efface, les pointeurs reviennent) et la fenetre se rouvre a l'ancien endroit : une mise a jour ne ferme jamais une
//! compagnie. Un renommage refuse (un programme qui tient encore un fichier : le relais de la mise a jour qui vient de
//! lancer la fenetre, un processus du navigateur) se retente tant que le dossier ne se laisse pas deplacer, une minute au
//! plus ; une copie ne se retente pas.
//!
//! L'ANCIENNE INSTALLATION (`vers_les_documents`). Les postes installes avant l'installeur qui propose les Documents
//! vivent dans le dossier de l'utilisateur lui-meme (`installation::dossier_de_l_utilisateur`, `AppData\Local\PuppetPaie`
//! sous Windows), cache. Une installation qui EST ce dossier, et n'est pas portable, va UNE FOIS, d'office, au lancement,
//! dans `Documents\PuppetPaie` (le dossier Documents tel que Windows le dit, comme l'installeur, `$DOCUMENTS`) ; le
//! dossier des fenetres (`installation::DOSSIER_DES_FENETRES`), qui est a l'utilisateur, reste ou il est. Ce qui empeche
//! le deplacement (un `Documents\PuppetPaie` qui n'est pas vide, la place) se retient (`maj/deplacement.json`) et la route
//! le dit ; rien ne se retente d'office ensuite. Une installation portable ne bouge jamais d'office.
//!
//! CE QUI S'EST PASSE se retient dans `maj/deplacement.json` (`Dernier`), la ou l'installation est au bout, et dans le
//! journal de la mise a jour (`maj::noter`).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use socle::echec::{refus, R};
use socle::grandeurs::Octets;
use socle::paris::Moment;

use crate::maj::{dossier_maj, noter};

/// La demande, dans `maj/` : le dossier ou le logiciel va.
const DEMANDE: &str = "deplacer.json";
/// Ce qui s'est passe la derniere fois, dans `maj/` : `Dernier`.
const DERNIER: &str = "deplacement.json";
/// La sous-commande du relais (`fenetre/src/main.rs`).
pub const RELAIS: &str = "deplacer-relais";
/// Le dossier, dans les Documents, ou une ancienne installation va : celui que l'installeur propose
/// (`atelier/installeur/puppetpaie.nsi`, `$DOCUMENTS\PuppetPaie`).
const DANS_LES_DOCUMENTS: &str = "PuppetPaie";
/// Le fichier d'essai qui dit, avant le deplacement, si le nouveau dossier est sur le meme disque et s'il s'y ecrit.
const ESSAI: &str = ".essai-deplacement";
/// Combien de fois un renommage refuse se retente (une seconde entre deux) : le temps qu'un programme lache le dossier.
const ESSAIS_DU_RENOMMAGE: u32 = 60;

/// `maj/deplacer.json` : le dossier ou le logiciel va, au lancement.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Demande {
    vers: PathBuf,
    quoi: String,
}

/// Ce qu'est devenu un deplacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Issue {
    /// Le logiciel est au nouvel endroit.
    Deplace,
    /// Le logiciel est reste ou il etait, entier.
    Reste,
}

/// `maj/deplacement.json` : le dernier deplacement, ou ce qui l'a empeche.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dernier {
    #[serde(with = "socle::paris::ecrit::minute")]
    pub quand: Moment,
    pub de: PathBuf,
    pub vers: PathBuf,
    pub issue: Issue,
    /// Ce qu'on en dit, a l'ecran (la route `logiciel-deplacer` la redit).
    pub phrase: String,
}

/// Ce que `verifier` conclut.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Le logiciel est deja dans ce dossier : rien ne change.
    DejaLa,
    /// Le deplacement peut se faire, vers ce dossier (resolu).
    Possible(PathBuf),
}

/// UN DOSSIER DIT A L'ECRAN, comme l'explorateur le montre : ses noms separes de « › » (un chemin ecrit n'est pas fait
/// pour l'ecran, `routes::affiche`).
pub fn dit(p: &Path) -> String {
    p.components()
        .filter(|c| !matches!(c, std::path::Component::RootDir))
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" › ")
}

/// Le chemin resolu d'un dossier, sans le prefixe `\\?\` de Windows.
fn resolu(p: &Path) -> R<PathBuf> {
    let r = socle::fichiers::resolu(p).map_err(|e| socle::echec::du_disque(p, &e))?;
    Ok(match r.to_string_lossy().strip_prefix(r"\\?\") {
        Some(sans) => PathBuf::from(sans),
        None => r.clone(),
    })
}

/// Ce qui, dans l'installation `inst`, reste ou il est : le dossier des fenetres quand l'installation est le dossier de
/// l'utilisateur lui-meme (il est a l'utilisateur, pas a l'installation).
fn ce_qui_reste(inst: &Path) -> R<Vec<OsString>> {
    let utilisateur = crate::installation::dossier_de_l_utilisateur();
    if utilisateur.is_dir() && resolu(&utilisateur)? == resolu(inst)? {
        return Ok(vec![OsString::from(crate::installation::DOSSIER_DES_FENETRES)]);
    }
    Ok(Vec::new())
}

/// Les entrees de l'installation qui partent, par nom.
fn entrees(inst: &Path, reste: &[OsString]) -> R<Vec<OsString>> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(inst).map_err(|e| socle::echec::du_disque(inst, &e))? {
        let nom = e.map_err(|e| socle::echec::du_disque(inst, &e))?.file_name();
        if !reste.contains(&nom) {
            v.push(nom);
        }
    }
    Ok(v)
}

/// Ce que porte un chemin d'un arbre, pour comparer une copie a l'original.
#[derive(Debug, PartialEq, Eq)]
enum Trace {
    Dossier,
    Fichier(Octets),
}

/// L'arbre des `noms` sous `racine` : chaque chemin relatif, et ce qu'il porte.
fn arbre(racine: &Path, noms: &[OsString]) -> std::io::Result<BTreeMap<PathBuf, Trace>> {
    fn sous(racine: &Path, rel: &Path, t: &mut BTreeMap<PathBuf, Trace>) -> std::io::Result<()> {
        let m = std::fs::metadata(racine.join(rel))?;
        if m.is_dir() {
            t.insert(rel.to_path_buf(), Trace::Dossier);
            for e in std::fs::read_dir(racine.join(rel))? {
                sous(racine, &rel.join(e?.file_name()), t)?;
            }
        } else {
            t.insert(rel.to_path_buf(), Trace::Fichier(Octets::du_disque(&m)));
        }
        Ok(())
    }
    let mut t = BTreeMap::new();
    for n in noms {
        sous(racine, Path::new(n), &mut t)?;
    }
    Ok(t)
}

/// La place que prennent les `noms` de `racine`.
fn taille(racine: &Path, noms: &[OsString]) -> R<Octets> {
    let t = arbre(racine, noms).map_err(|e| socle::echec::du_disque(racine, &e))?;
    Ok(t.values().fold(Octets::ZERO, |s, x| match x {
        Trace::Fichier(o) => s + *o,
        Trace::Dossier => s,
    }))
}

/// TOUT CE QUI SE VERIFIE AVANT LE DEPLACEMENT de l'installation `inst` vers `vers` : un dossier dit en entier, qui n'est
/// ni elle ni dedans, qui n'existe pas (le dossier qui le portera, si) ou est vide ; qu'on peut y ecrire ; et, sur un
/// autre disque, que la place suffit. Un depot de travail (qui porte `.git`) ne se deplace pas.
pub fn verifier(inst: &Path, vers: &Path) -> R<Verdict> {
    if cfg!(target_os = "wasi") {
        return Err(refus("Dans le navigateur, le logiciel ne se déplace pas. Rien n'a changé."));
    }
    if !vers.is_absolute() {
        return Err(refus("Le dossier se dit en entier, depuis son disque. Rien n'a changé."));
    }
    let inst = resolu(inst)?;
    if inst.join(".git").exists() {
        return Err(refus("Ce logiciel est un dépôt de travail : il ne se déplace pas. Rien n'a changé."));
    }
    let existe = socle::fichiers::existe_lu(vers)?;
    let (Some(parent), Some(nom)) = (vers.parent(), vers.file_name()) else {
        return Err(refus("Le logiciel va dans un dossier, pas à la racine d'un disque. Rien n'a changé."));
    };
    let vers = match existe {
        true => resolu(vers)?,
        false if parent.is_dir() => resolu(parent)?.join(nom),
        false => return Err(refus(format!("Le dossier qui le portera n'existe pas : {}. Rien n'a changé.", dit(parent)))),
    };
    if vers == inst {
        return Ok(Verdict::DejaLa);
    }
    if socle::fichiers::sous(&vers, &inst).is_some() {
        return Err(refus("Le nouveau dossier ne peut pas être dans celui du logiciel. Rien n'a changé."));
    }
    let lieu = match existe {
        true => {
            if !vers.is_dir() {
                return Err(refus(format!("{} est un fichier, pas un dossier. Rien n'a changé.", dit(&vers))));
            }
            let mut d = std::fs::read_dir(&vers).map_err(|e| socle::echec::du_disque(&vers, &e))?;
            if d.next().is_some() {
                return Err(refus(format!("Le dossier {} n'est pas vide : choisir un dossier neuf, ou vide. Rien n'a changé.", dit(&vers))));
            }
            vers.clone()
        }
        false => dessus(&vers),
    };
    // L'essai : un fichier de l'installation renomme dans le nouveau lieu dit le meme disque et qu'on y ecrit.
    let essai = inst.join(ESSAI);
    let la_bas = lieu.join(ESSAI);
    std::fs::write(&essai, b"").map_err(|e| socle::echec::du_disque(&essai, &e))?;
    let renomme = std::fs::rename(&essai, &la_bas);
    match renomme {
        Ok(()) => socle::fichiers::effacer(&la_bas)?,
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            socle::fichiers::effacer(&essai)?;
            if let Err(e) = std::fs::write(&la_bas, b"") {
                return Err(refus(format!("On ne peut pas écrire dans {} ({}). Rien n'a changé.", dit(&lieu), e)));
            }
            socle::fichiers::effacer(&la_bas)?;
            let faut = taille(&inst, &entrees(&inst, &ce_qui_reste(&inst)?)?)?;
            let libre = crate::diagnostic::octets_libres(&lieu)
                .map_err(|e| refus(format!("La place libre de ce disque ne se lit pas ({}). Rien n'a changé.", e)))?;
            if Octets::lue(libre) < faut {
                return Err(refus(format!("La place manque sur ce disque : il faut {}, il n'y a que {}. Rien n'a changé.", faut, Octets::lue(libre))));
            }
        }
        Err(e) => {
            socle::fichiers::effacer(&essai)?;
            return Err(refus(format!("On ne peut pas écrire dans {} ({}). Rien n'a changé.", dit(&lieu), e)));
        }
    }
    Ok(Verdict::Possible(vers))
}

/// Le dossier qui porte `p`.
fn dessus(p: &Path) -> PathBuf {
    crate::installation::dessus(p)
}

/// LE DEPLACEMENT DEMANDE (la route `logiciel-deplacer`) : verifie, puis pose `maj/deplacer.json`. Rend le verdict.
pub fn demander(inst: &Path, vers: &Path) -> R<Verdict> {
    let v = verifier(inst, vers)?;
    match &v {
        Verdict::DejaLa => socle::fichiers::effacer(&dossier_maj(inst).join(DEMANDE))?,
        Verdict::Possible(vers) => crate::maj::ecrire_json(
            &dossier_maj(inst).join(DEMANDE),
            &Demande { vers: vers.clone(), quoi: "Le dossier où le logiciel va, logiciel fermé (compagnie::deplacement).".to_string() },
        )?,
    }
    Ok(v)
}

/// La demande en attente, s'il y en a une.
fn demande(inst: &Path) -> R<Option<Demande>> {
    let p = dossier_maj(inst).join(DEMANDE);
    if !p.is_file() {
        return Ok(None);
    }
    socle::forme::lire_fichier(&p, "La demande de déplacement du logiciel (maj/deplacer.json) ne se lit pas : l'effacer, et redemander.").map(Some)
}

/// Le dernier deplacement de l'installation, ou ce qui l'a empeche ; `None` s'il n'y en a jamais eu.
pub fn dernier(inst: &Path) -> R<Option<Dernier>> {
    let p = dossier_maj(inst).join(DERNIER);
    if !p.is_file() {
        return Ok(None);
    }
    socle::forme::lire_fichier(&p, "Le dernier déplacement du logiciel (maj/deplacement.json) ne se lit pas.").map(Some)
}

/// Retient ce qui s'est passe, la ou l'installation est (`ou`), et le note au journal de la mise a jour.
fn retenir(ou: &Path, de: &Path, vers: &Path, issue: Issue, phrase: String) {
    noter(ou, &format!("Deplacement : {}", phrase));
    let d = Dernier { quand: socle::temps::secondes().a_la_minute(), de: de.to_path_buf(), vers: vers.to_path_buf(), issue, phrase };
    if let Err(e) = crate::maj::ecrire_json(&dossier_maj(ou).join(DERNIER), &d) {
        noter(ou, &format!("Deplacement : ce qui s'est passe ne se retient pas ({}).", e.phrase));
    }
}

/// La phrase d'un deplacement qui n'a pas eu lieu.
fn pas_alle(vers: &Path, pourquoi: &str) -> String {
    format!("Le logiciel n'est pas allé dans {} : {} Il est resté ici, entier.", dit(vers), pourquoi)
}

/// L'ANCIENNE INSTALLATION (voir l'en-tete) : le dossier des Documents ou elle va, une fois ; `None` pour toute autre
/// installation, une portable, ou une qui a deja un deplacement retenu.
fn vers_les_documents(inst: &Path) -> R<Option<PathBuf>> {
    if inst.join(crate::coffre::MARQUEUR_PORTABLE).is_file() || dernier(inst)?.is_some() || ce_qui_reste(inst)?.is_empty() {
        return Ok(None);
    }
    Ok(documents()?.map(|d| d.join(DANS_LES_DOCUMENTS)))
}

/// AU LANCEMENT, avant toute fenetre : un deplacement demande, ou celui d'une ancienne installation, part au relais.
/// Rend `true` si le relais est parti (le programme doit alors s'arreter sans rien ouvrir). Ce qui l'empeche se retient,
/// et le logiciel s'ouvre ou il est.
pub fn au_lancement() -> bool {
    let inst = crate::installation::base();
    let vers = match demande(&inst) {
        Ok(Some(d)) => d.vers,
        Ok(None) => match vers_les_documents(&inst) {
            Ok(Some(v)) => match verifier(&inst, &v) {
                Ok(Verdict::Possible(v)) => v,
                Ok(Verdict::DejaLa) => return false,
                Err(e) => {
                    retenir(&inst, &inst, &v, Issue::Reste, pas_alle(&v, &e.phrase));
                    return false;
                }
            },
            Ok(None) => return false,
            Err(e) => {
                noter(&inst, &format!("Deplacement : l'ancienne installation ne se reconnait pas ({}).", e.phrase));
                return false;
            }
        },
        Err(e) => {
            noter(&inst, &format!("Deplacement : {}", e.phrase));
            socle::fichiers::effacer_apres_coup(&dossier_maj(&inst).join(DEMANDE));
            return false;
        }
    };
    match lancer_relais(&inst, &vers) {
        Ok(()) => true,
        Err(e) => {
            socle::fichiers::effacer_apres_coup(&dossier_maj(&inst).join(DEMANDE));
            retenir(&inst, &inst, &vers, Issue::Reste, pas_alle(&vers, &format!("le relais ne se lance pas ({}).", e.phrase)));
            false
        }
    }
}

/// APRES LA ROUTE, DANS LA FENETRE : une demande posee part au relais tout de suite ; a la fenetre de se fermer aussitot
/// (`true`). Le relais ne se lance pas : la demande s'efface, et le refus le dit.
pub fn lancer_s_il_est_demande(inst: &Path) -> R<bool> {
    let Some(d) = demande(inst)? else { return Ok(false) };
    lancer_relais(inst, &d.vers).map_err(|e| {
        socle::fichiers::effacer_apres_coup(&dossier_maj(inst).join(DEMANDE));
        e.reformuler(|m| format!("Le logiciel ne se déplace pas : {} Rien n'a changé.", m))
    })?;
    Ok(true)
}

/// Le programme de la fenetre, copie dans le dossier temporaire du poste, prend le relais ; il attendra la fin de CE
/// processus.
fn lancer_relais(inst: &Path, vers: &Path) -> R<()> {
    let logiciel = inst.join(crate::maj::LOGICIEL);
    let source = crate::maj::programme(&logiciel, "puppetpaie");
    if !source.is_file() {
        return Err(socle::echec::refus_avec("Le programme de la fenêtre manque.", socle::echec::Lieu { chemin: Some(source.to_string_lossy().into_owned()), ..socle::echec::Lieu::default() }));
    }
    let numero = socle::processus::numero();
    let ici = socle::fichiers::dossier_temporaire().join(format!("PuppetPaie-deplacement-{}", numero));
    socle::fichiers::effacer(&ici)?;
    std::fs::create_dir_all(&ici).map_err(|e| socle::echec::du_disque(&ici, &e))?;
    let relais = ici.join(source.file_name().ok_or_else(|| refus("Le programme de la fenêtre n'a pas de nom."))?);
    std::fs::copy(&source, &relais).map_err(|e| socle::echec::du_disque(&relais, &e))?;
    // La fenetre Windows compilee hors MSVC charge WebView2Loader.dll au demarrage : elle doit suivre le relais.
    let chargeur = crate::maj::dossier_programmes(&logiciel).join("WebView2Loader.dll");
    if cfg!(windows) && chargeur.is_file() {
        std::fs::copy(&chargeur, ici.join("WebView2Loader.dll")).map_err(|e| socle::echec::du_disque(&chargeur, &e))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&relais, std::fs::Permissions::from_mode(0o755)).map_err(|e| socle::echec::du_disque(&relais, &e))?;
    }
    let mut c = std::process::Command::new(&relais);
    c.arg(RELAIS).arg("--installation").arg(inst).arg("--vers").arg(vers).arg("--attendre").arg(numero.to_string());
    c.current_dir(&ici);
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c.spawn().map_err(|e| socle::refus!("le relais ne se lance pas : {}", e))?;
    Ok(())
}

/// Le code de sortie du relais mal appele : il n'a rien a deplacer.
fn relais_mal_appele() -> std::process::ExitCode {
    std::process::ExitCode::from(2)
}

/// LE RELAIS (`puppetpaie deplacer-relais --installation I --vers V --attendre PID`), lance du dossier temporaire du
/// poste : attend la fin du programme qui l'a lance, deplace (`deplacer`), retient ce qui s'est passe, et rouvre la
/// fenetre la ou est l'installation au bout.
pub fn relais(args: &[socle::ligne::Argument]) -> std::process::ExitCode {
    let args = socle::ligne::textes(args);
    let valeur = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned();
    let (Some(inst), Some(vers)) = (valeur("--installation").map(PathBuf::from), valeur("--vers").map(PathBuf::from)) else {
        return relais_mal_appele();
    };
    // La demande part, quoi qu'il arrive : un deplacement qui echoue ne se rejoue pas a chaque lancement.
    let attendu = match valeur("--attendre").map(|p| p.parse::<u32>()) {
        Some(Ok(p)) => match crate::maj::attendre_fin(p, &inst) {
            Ok(true) => Ok(()),
            Ok(false) => Err("le programme qui l'a demandé ne s'est pas arrêté.".to_string()),
            Err(e) => Err(format!("on ne sait pas si le programme qui l'a demandé s'est arrêté ({}).", e.phrase)),
        },
        Some(Err(e)) => Err(format!("le numéro du programme à attendre ne se lit pas ({}).", e)),
        None => Err("le relais ne dit pas quel programme attendre.".to_string()),
    };
    socle::fichiers::effacer_apres_coup(&dossier_maj(&inst).join(DEMANDE));
    let ou = match attendu.and_then(|()| deplacer(&inst, &vers).map_err(|e| e.phrase)) {
        Ok(restes) => {
            let phrase = match restes.is_empty() {
                true => format!("Le logiciel est arrivé ici, depuis {}.", dit(&inst)),
                false => format!("Le logiciel est arrivé ici, depuis {}. L'ancien dossier ne s'est pas effacé entier : {}", dit(&inst), restes.join(" ; ")),
            };
            retenir(&vers, &inst, &vers, Issue::Deplace, phrase);
            vers
        }
        Err(pourquoi) => {
            retenir(&inst, &inst, &vers, Issue::Reste, pas_alle(&vers, &pourquoi));
            inst
        }
    };
    let logiciel = ou.join(crate::maj::LOGICIEL);
    if let Err(e) = std::process::Command::new(crate::maj::programme(&logiciel, "puppetpaie")).current_dir(&logiciel).spawn() {
        noter(&ou, &format!("La fenetre ne se relance pas apres le deplacement ({}) : la rouvrir a la main.", e));
    }
    std::process::ExitCode::SUCCESS
}

/// LE DEPLACEMENT DE L'INSTALLATION `de` VERS `vers` (voir l'en-tete), logiciel ferme. Rend ce qui, sur un autre disque,
/// n'a pas pu s'effacer de l'ancien dossier (le logiciel est au nouveau, entier). Une erreur : tout est revenu.
fn deplacer(de: &Path, vers: &Path) -> R<Vec<String>> {
    let mut essai = 1;
    loop {
        match deplacer_une_fois(de, vers) {
            Err((e, Retenter::Oui)) if essai < ESSAIS_DU_RENOMMAGE => {
                essai += 1;
                std::thread::sleep(std::time::Duration::from_secs(1));
                noter(de, &format!("Deplacement : le dossier ne se laisse pas encore deplacer ({}) ; nouvel essai.", e.phrase));
            }
            Err((e, _)) => return Err(e),
            Ok(restes) => return Ok(restes),
        }
    }
}

/// Un echec du deplacement, tout revenu : se retente-t-il ?
enum Retenter {
    Oui,
    Non,
}

fn deplacer_une_fois(de: &Path, vers: &Path) -> Result<Vec<String>, (socle::echec::Echec, Retenter)> {
    let non = |e: socle::echec::Echec| (e, Retenter::Non);
    let reste = ce_qui_reste(de).map_err(non)?;
    let vers = match verifier(de, vers).map_err(non)? {
        Verdict::DejaLa => return Ok(Vec::new()),
        Verdict::Possible(v) => v,
    };
    let cree = !socle::fichiers::existe_lu(&vers).map_err(non)?;
    if cree {
        std::fs::create_dir(&vers).map_err(|e| non(socle::echec::du_disque(&vers, &e)))?;
    }
    // Le dossier neuf que le deplacement a cree part avec ce qui revient.
    let sans_le_neuf = |e: socle::echec::Echec| -> socle::echec::Echec {
        match cree {
            true => match std::fs::remove_dir(&vers) {
                Ok(()) => e,
                Err(x) => e.reformuler(|m| format!("{} (le dossier {} est resté, vide : {})", m, dit(&vers), x)),
            },
            false => e,
        }
    };
    let noms = entrees(de, &reste).map_err(|e| non(sans_le_neuf(e)))?;
    let mut renommes: Vec<OsString> = Vec::new();
    let mut en_copie = false;
    for nom in &noms {
        match std::fs::rename(de.join(nom), vers.join(nom)) {
            Ok(()) => renommes.push(nom.clone()),
            Err(e) if e.kind() == ErrorKind::CrossesDevices && renommes.is_empty() => {
                en_copie = true;
                break;
            }
            Err(e) => {
                let e = socle::echec::du_disque(&de.join(nom), &e);
                return Err(match defaire_les_renommes(de, &vers, &renommes) {
                    Ok(()) => (sans_le_neuf(e), Retenter::Oui),
                    Err(x) => (e.reformuler(|m| format!("{} ; et tout n'est pas revenu : {}", m, x)), Retenter::Non),
                });
            }
        }
    }
    if en_copie {
        if let Err(e) = copier_et_comparer(de, &vers, &noms) {
            return Err(non(sans_le_neuf(defaire_la_copie(&vers, &noms, e))));
        }
    }
    if let Err(e) = repointer(de, &vers) {
        let e = match repointer(&vers, de) {
            Ok(()) => e,
            Err(x) => e.reformuler(|m| format!("{} ; et ce qui pointait vers l'ancien dossier n'y pointe plus entier : {}", m, x.phrase)),
        };
        return Err(match en_copie {
            true => non(sans_le_neuf(defaire_la_copie(&vers, &noms, e))),
            false => match defaire_les_renommes(de, &vers, &renommes) {
                Ok(()) => non(sans_le_neuf(e)),
                Err(x) => non(e.reformuler(|m| format!("{} ; et tout n'est pas revenu : {}", m, x))),
            },
        });
    }
    // Le logiciel est au nouvel endroit, entier : ce qui reste de l'ancien s'efface, et ce qui ne s'efface pas se dit.
    let mut restes = Vec::new();
    if en_copie {
        for nom in &noms {
            if let Err(e) = socle::fichiers::effacer(&de.join(nom)) {
                restes.push(e.phrase);
            }
        }
    }
    if reste.is_empty() {
        if let Err(e) = std::fs::remove_dir(de) {
            restes.push(format!("{} ({})", dit(de), e));
        }
    }
    Ok(restes)
}

/// Les `renommes` reviennent de `vers` dans `de` ; ce qui ne revient pas se dit.
fn defaire_les_renommes(de: &Path, vers: &Path, renommes: &[OsString]) -> Result<(), String> {
    let pas_revenus: Vec<String> = renommes
        .iter()
        .filter_map(|nom| std::fs::rename(vers.join(nom), de.join(nom)).err().map(|e| format!("{} ({})", nom.to_string_lossy(), e)))
        .collect();
    match pas_revenus.is_empty() {
        true => Ok(()),
        false => Err(pas_revenus.join(", ")),
    }
}

/// La copie dans `vers` s'efface ; l'echec `e` le dit si elle ne s'efface pas entiere.
fn defaire_la_copie(vers: &Path, noms: &[OsString], e: socle::echec::Echec) -> socle::echec::Echec {
    noms.iter().fold(e, |e, nom| socle::fichiers::effacer_apres(e, &vers.join(nom)))
}

/// Copie les `noms` de `de` dans `vers`, puis compare : chaque dossier, chaque fichier et sa taille.
fn copier_et_comparer(de: &Path, vers: &Path, noms: &[OsString]) -> R<()> {
    fn copier(de: &Path, vers: &Path) -> std::io::Result<()> {
        if std::fs::metadata(de)?.is_dir() {
            std::fs::create_dir(vers)?;
            for e in std::fs::read_dir(de)? {
                let nom = e?.file_name();
                copier(&de.join(&nom), &vers.join(&nom))?;
            }
            return Ok(());
        }
        std::fs::copy(de, vers).map(|_| ())
    }
    for nom in noms {
        copier(&de.join(nom), &vers.join(nom)).map_err(|e| socle::echec::du_disque(&de.join(nom), &e))?;
    }
    let avant = arbre(de, noms).map_err(|e| socle::echec::du_disque(de, &e))?;
    let apres = arbre(vers, noms).map_err(|e| socle::echec::du_disque(vers, &e))?;
    if avant != apres {
        let differents = avant.iter().filter(|(p, t)| apres.get(*p) != Some(*t)).count() + apres.keys().filter(|p| !avant.contains_key(*p)).count();
        return Err(refus(format!("La copie n'est pas arrivée entière : {} fichier(s) ou dossier(s) manquent ou diffèrent.", differents)));
    }
    Ok(())
}

/// Un chemin entre apostrophes, pour PowerShell (une apostrophe s'y double).
#[cfg(windows)]
fn entre_apostrophes(p: &Path) -> String {
    format!("'{}'", p.to_string_lossy().replace('\'', "''"))
}

/// Un script PowerShell (`powershell.exe -NoProfile -NonInteractive -Command`), sa sortie en UTF-8 ; son refus sinon.
#[cfg(windows)]
fn powershell(script: &str) -> R<String> {
    use std::os::windows::process::CommandExt;
    let tete = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $ErrorActionPreference = 'Stop'; ";
    let o = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(format!("{}{}", tete, script))
        .stdin(std::process::Stdio::null())
        .creation_flags(0x0800_0000)
        .output()
        .map_err(|e| socle::refus!("PowerShell ne se lance pas : {}", e))?;
    if !o.status.success() {
        return Err(socle::refus!("PowerShell refuse : {}", String::from_utf8_lossy(&o.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Le dossier Documents de l'utilisateur, tel que Windows le dit (`MyDocuments`, celui de `$DOCUMENTS` de l'installeur) ;
/// `None` hors de Windows, ou une ancienne installation ne se deplace pas d'office.
#[cfg(windows)]
fn documents() -> R<Option<PathBuf>> {
    let d = powershell("[Environment]::GetFolderPath('MyDocuments')")?;
    match d.is_empty() {
        true => Err(refus("Windows ne dit pas où est le dossier Documents.")),
        false => Ok(Some(PathBuf::from(d))),
    }
}

#[cfg(not(windows))]
fn documents() -> R<Option<PathBuf>> {
    Ok(None)
}

/// CE QUI POINTE VERS L'INSTALLATION se reecrit de `de` a `vers` : ce que l'installeur a pose
/// (`atelier/installeur/puppetpaie.nsi`), la cle de desinstallation sous `HKCU` (`InstallLocation`, `DisplayIcon`,
/// `UninstallString`, `QuietUninstallString`), si elle est celle de cette installation, et le raccourci du menu Demarrer
/// (`PuppetPaie.lnk` des programmes de l'utilisateur), s'il lance le programme de cette installation. Hors de Windows,
/// rien.
#[cfg(windows)]
fn repointer(de: &Path, vers: &Path) -> R<()> {
    let script = format!(
        "$de = {de}; $vers = {vers}; \
         function Remplace([string]$v) {{ if (-not $v) {{ return $v }}; [regex]::Replace($v, [regex]::Escape($de), $vers.Replace('$', '$$'), 'IgnoreCase') }}; \
         $cle = 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PuppetPaie'; \
         if (Test-Path -LiteralPath $cle) {{ \
           $p = Get-ItemProperty -LiteralPath $cle; \
           if ($p.InstallLocation -and ($p.InstallLocation.TrimEnd('\\') -eq $de)) {{ \
             foreach ($n in 'InstallLocation', 'DisplayIcon', 'UninstallString', 'QuietUninstallString') {{ \
               if ($p.$n) {{ Set-ItemProperty -LiteralPath $cle -Name $n -Value (Remplace $p.$n) }} }} }} }}; \
         $lnk = Join-Path ([Environment]::GetFolderPath('Programs')) 'PuppetPaie.lnk'; \
         if (Test-Path -LiteralPath $lnk) {{ \
           $s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk); \
           if ($s.TargetPath.StartsWith($de + '\\', [StringComparison]::OrdinalIgnoreCase)) {{ \
             $s.TargetPath = Remplace $s.TargetPath; $s.WorkingDirectory = Remplace $s.WorkingDirectory; \
             $s.IconLocation = Remplace $s.IconLocation; $s.Save() }} }}",
        de = entre_apostrophes(de),
        vers = entre_apostrophes(vers),
    );
    powershell(&script).map(|_| ())
}

#[cfg(not(windows))]
fn repointer(_de: &Path, _vers: &Path) -> R<()> {
    Ok(())
}
