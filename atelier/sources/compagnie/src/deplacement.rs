//! DEPLACER LE LOGICIEL : le dossier de l'installation (celui qui porte `logiciel/`, et a cote les compagnies, `maj/`,
//! les fictives) va ailleurs, entier, LOGICIEL FERME (Leandre, 25/11/2026 : « on propose Documents de base et les gens
//! peuvent changer », « a l'installation et dans un reglage dans le logiciel »). RIEN NE SE DEPLACE D'OFFICE : l'accueil
//! dit ou est le logiciel et propose le geste (`routes::accueil::deplacement`) ; une ancienne installation, cachee, se
//! voit proposer « Ranger dans Documents ».
//!
//! LE GESTE. La route `logiciel-deplacer` verifie tout ce qui se verifie avant (`verifier`) et pose la demande
//! (`maj/deplacer.json`, `demander`). La fenetre qui a servi la route la fait partir aussitot et se ferme
//! (`lancer_s_il_est_demande`) ; posee en ligne de commande, elle part au prochain lancement de la fenetre
//! (`au_lancement`). PARTIR, c'est : effacer la demande (un retrait qui echoue ne lance rien : la demande se rejouerait a
//! chaque lancement), ecrire le DEPLACEMENT EN COURS (`DeplacementEnCours`), puis lancer le relais.
//!
//! LE DEPLACEMENT EN COURS vit hors de l'installation, dans le dossier de l'utilisateur
//! (`installation::dossier_de_l_utilisateur`, `EN_COURS`) : on l'y retrouve d'ou que l'on lance, l'ancien dossier ou le
//! nouveau. Le relais le tient de bout en bout par un VERROU du systeme sur un fichier voisin (`VERROU`), que le systeme
//! rend quand le relais meurt. Chaque etape s'y ecrit SUR LE DISQUE avant d'etre faite (`Etape`,
//! `socle::ecriture::ecrire_json_sur_le_disque`). Chacun des deux dossiers, l'ancien et le neuf, porte en plus une MARQUE
//! (`MARQUE`) le temps du deplacement : une installation sait ainsi d'elle-meme qu'elle se deplace, meme quand le
//! deplacement en cours ne se lit plus. Une installation que le deplacement CONCERNE (`ici` : le deplacement en cours la
//! nomme, ou il ne se lit pas et elle porte la marque) ne s'ouvre pas : la fenetre le dit dans une boite et s'arrete,
//! avant meme de prendre son verrou (`en_cours_au_lancement`) ; la ligne de commande refuse (`refuser_s_il_se_deplace`).
//! Toute autre installation du poste s'ouvre, quoi qu'il en soit du deplacement en cours. Le relais au travail : on
//! attend. Le relais mort (le poste eteint en route) : le lancement suivant lance un relais qui REPREND, finit ce qui
//! etait fini ou defait ce qui etait commence (`reprendre`).
//!
//! LE RELAIS, HORS DU DOSSIER. Windows ne deplace pas un dossier qui porte un programme en marche, ni le dossier courant
//! d'un processus. Le programme de la fenetre se copie donc dans le dossier temporaire du poste
//! (`PuppetPaie-deplacement-<numero>/`, avec le chargeur de WebView2 qu'il charge au demarrage), s'y lance avec
//! `deplacer-relais`, son dossier courant la-bas, et le lanceur s'arrete sans rien ouvrir. Le relais prend le verrou,
//! attend la fin du lanceur (`maj::attendre_fin`), deplace, puis rouvre la fenetre la ou est le logiciel. Le relais de la
//! mise a jour, lui, reste dans `maj/` : il ne deplace pas l'installation.
//!
//! LE DEPLACEMENT. Chaque entree de l'installation se RENOMME dans le nouveau dossier quand c'est le meme disque :
//! instantane, sans place en double ; `logiciel/` (le programme que le raccourci lance) part le dernier et revient le
//! premier, pour qu'une coupure en route laisse le programme la ou le raccourci le cherche. Un autre disque (le systeme refuse le renommage, `CrossesDevices`) : tout se COPIE,
//! chaque fichier copie s'ecrit sur le disque (`sync_all`) et son contenu se compare a l'original (une empreinte
//! SHA-256), puis l'original s'efface ; un lien ne se copie pas (le deplacement est refuse). Ce qui pointe vers l'ancien
//! dossier se reecrit (sous Windows, la cle de desinstallation et le raccourci du menu Demarrer que l'installeur a poses,
//! `repointer`). AU MOINDRE ECHEC, TOUT REVIENT (les renommes repartent, la copie s'efface, les pointeurs reviennent) et la
//! fenetre se rouvre a l'ancien endroit. Un renommage refuse (un programme qui tient encore un fichier) se retente, tout
//! revenu, une seconde plus tard, une minute au plus ; une copie ne se retente pas.
//!
//! CE QUI S'EST PASSE se dit en verite (`Dernier`, `Issue`) : deplace, reste entier a l'ancien endroit, ou COUPE (des
//! renommes ne sont pas revenus : ce qui est ou se dit, nom par nom). Il se retient dans le dossier de l'utilisateur
//! (`DERNIER`), d'ou l'accueil le redit. Coupe, la fenetre ne se rouvre pas (le logiciel n'est entier nulle part) : une
//! boite le dit, et le deplacement en cours reste ecrit, avec ses marques ; le lancement suivant retente de tout ramener.
//! Une boite ne se pose qu'une fois le verrou rendu : un lancement pendant qu'elle attend lit l'etat vrai.
//!
//! LE MOT DE PASSE. Deplacer le logiciel se prouve comme un acte reel (la route, `routes::accueil::deplacement`) : un
//! LLM qui jouerait l'exemple d'une fiche ne deplace rien.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{ErrorKind, Read};
use std::path::{Component, Path, PathBuf, Prefix};

use serde::{Deserialize, Serialize};
use socle::echec::{refus, R};
use socle::grandeurs::Octets;
use socle::paris::Moment;

use crate::maj::{dossier_maj, noter};

/// La demande, dans `maj/` de l'installation : le dossier ou le logiciel va.
const DEMANDE: &str = "deplacer.json";
/// Le deplacement en cours, dans le dossier de l'utilisateur : `DeplacementEnCours`.
const EN_COURS: &str = "deplacement-en-cours.json";
/// Le fichier que le relais verrouille tant qu'il travaille, dans le dossier de l'utilisateur.
const VERROU: &str = "deplacement-en-cours.verrou";
/// La marque d'un dossier qui se deplace (l'ancien et le neuf), un fichier vide : sa presence seule dit.
const MARQUE: &str = ".deplacement-en-cours";
/// Ce qui s'est passe la derniere fois, dans le dossier de l'utilisateur : `Dernier`.
const DERNIER: &str = "deplacement.json";
/// La sous-commande du relais (`fenetre/src/main.rs`).
pub const RELAIS: &str = "deplacer-relais";
/// Le dossier, dans les Documents, ou une ancienne installation se range : celui que l'installeur propose
/// (`atelier/installeur/puppetpaie.nsi`, `$DOCUMENTS\PuppetPaie`).
const DANS_LES_DOCUMENTS: &str = "PuppetPaie";
/// Le fichier d'essai qui dit, avant le deplacement, si le nouveau dossier est sur le meme disque et s'il s'y ecrit.
const ESSAI: &str = ".essai-deplacement";
/// Combien de fois un renommage refuse se retente (une seconde entre deux) : le temps qu'un programme lache le dossier.
const ESSAIS_DU_RENOMMAGE: u32 = 60;
/// Les variables que Windows pose pour chaque dossier que OneDrive synchronise (personnel, professionnel).
const VARIABLES_ONEDRIVE: [&str; 3] = ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"];

/// `maj/deplacer.json` : le dossier ou le logiciel va.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Demande {
    vers: PathBuf,
    quoi: String,
}

/// Ou en est un deplacement en cours : chaque etape s'ecrit AVANT d'etre faite, et dit a qui reprend quoi finir ou
/// defaire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Etape {
    /// Rien n'a bouge : le relais attend la fin du lanceur.
    Attente,
    /// Les entrees se renomment : ou est chacune se lit sur le disque (dans l'ancien dossier ou dans le nouveau).
    Renommage,
    /// Les entrees se copient : l'original est entier.
    Copie,
    /// La copie est sur le disque, comparee, et les pointeurs reecrits : l'original s'efface.
    Effacement,
}

/// `deplacement-en-cours.json` : le deplacement que le relais fait, ou qu'un relais reprendra.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeplacementEnCours {
    /// L'installation, resolue.
    de: PathBuf,
    /// Le nouveau dossier, resolu.
    vers: PathBuf,
    /// Le nouveau dossier a ete cree par le deplacement : il part avec ce qui revient.
    cree: bool,
    /// Les entrees de l'installation qui partent.
    noms: Vec<PathBuf>,
    etape: Etape,
    quoi: String,
}

/// Ce qu'est devenu un deplacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Issue {
    /// Le logiciel est au nouvel endroit, entier.
    Deplace,
    /// Le logiciel est reste a l'ancien endroit, entier.
    Reste,
    /// Le logiciel n'est entier nulle part : des entrees ne sont pas revenues (la phrase dit ce qui est ou).
    Coupe,
}

/// `deplacement.json` du dossier de l'utilisateur : le dernier deplacement, ou ce qui l'a empeche.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dernier {
    #[serde(with = "socle::paris::ecrit::minute")]
    pub quand: Moment,
    pub de: PathBuf,
    pub vers: PathBuf,
    pub issue: Issue,
    /// Ce qu'on en dit, a l'ecran (l'accueil la redit).
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
/// pour l'ecran, `routes::affiche`) ; un lecteur reseau dit son serveur et son partage.
pub fn dit(p: &Path) -> String {
    p.components()
        .filter_map(|c| match c {
            Component::RootDir => None,
            Component::Prefix(x) => Some(match x.kind() {
                Prefix::Disk(d) | Prefix::VerbatimDisk(d) => format!("{}:", d as char),
                Prefix::UNC(s, h) | Prefix::VerbatimUNC(s, h) => format!("{} › {}", s.to_string_lossy(), h.to_string_lossy()),
                _ => x.as_os_str().to_string_lossy().into_owned(),
            }),
            autre => Some(autre.as_os_str().to_string_lossy().into_owned()),
        })
        .collect::<Vec<_>>()
        .join(" › ")
}

/// Le chemin resolu d'un dossier, sans le prefixe `\\?\` de Windows ; un lecteur reseau (`\\?\UNC\serveur\partage`)
/// redevient `\\serveur\partage`.
fn resolu(p: &Path) -> R<PathBuf> {
    let r = socle::fichiers::resolu(p).map_err(|e| socle::echec::du_disque(p, &e))?;
    let t = r.to_string_lossy();
    Ok(match (t.strip_prefix(r"\\?\UNC\"), t.strip_prefix(r"\\?\")) {
        (Some(reseau), _) => PathBuf::from(format!(r"\\{}", reseau)),
        (None, Some(sans)) => PathBuf::from(sans),
        (None, None) => r.clone(),
    })
}

/// Le chemin `p` resolu jusqu'ou il existe : le plus proche dossier au-dessus qui existe se resout, et le reste s'y
/// ajoute tel qu'il est ecrit. Une compagnie dont le dossier part en route se reconnait ainsi dans le deplacement qui
/// l'emporte, au lieu d'une erreur du disque.
fn resolu_ce_qui_existe(p: &Path) -> R<PathBuf> {
    for dessus in p.ancestors() {
        if socle::fichiers::existe_lu(dessus)? {
            let reste = p.strip_prefix(dessus).map_err(|_| socle::echec::au_chemin("Ce chemin ne se lit pas.", p))?;
            return Ok(resolu(dessus)?.join(reste));
        }
    }
    Err(socle::echec::au_chemin("Aucun dossier de ce chemin n'existe.", p))
}

/// Un fichier du deplacement, dans le dossier de l'utilisateur.
fn chez_l_utilisateur(nom: &str) -> PathBuf {
    crate::installation::dossier_de_l_utilisateur().join(nom)
}

/// L'installation `inst` est-elle le dossier de l'utilisateur lui-meme (une ancienne installation de Windows) ?
fn est_le_dossier_de_l_utilisateur(inst: &Path) -> R<bool> {
    let utilisateur = crate::installation::dossier_de_l_utilisateur();
    Ok(utilisateur.is_dir() && resolu(&utilisateur)? == resolu(inst)?)
}

/// Ce qui, dans l'installation `inst`, reste ou il est : sa marque, et, quand elle est le dossier de l'utilisateur, ce
/// qui est a l'utilisateur, pas a l'installation (le dossier des fenetres, et les fichiers du deplacement lui-meme).
fn ce_qui_reste(inst: &Path) -> R<Vec<OsString>> {
    let mut v = vec![OsString::from(MARQUE)];
    if est_le_dossier_de_l_utilisateur(inst)? {
        v.extend([crate::installation::DOSSIER_DES_FENETRES, EN_COURS, VERROU, DERNIER].into_iter().map(OsString::from));
    }
    Ok(v)
}

/// Les entrees de l'installation qui partent, par nom, `logiciel/` la derniere (il revient le premier : `revenir`).
fn entrees(inst: &Path, reste: &[OsString]) -> R<Vec<OsString>> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(inst).map_err(|e| socle::echec::du_disque(inst, &e))? {
        let nom = e.map_err(|e| socle::echec::du_disque(inst, &e))?.file_name();
        if !reste.contains(&nom) {
            v.push(nom);
        }
    }
    v.sort_by_key(|n| n.as_os_str() == crate::maj::LOGICIEL);
    Ok(v)
}

/// Pose la marque du deplacement dans le dossier `d`, sur le disque.
fn marquer(d: &Path) -> R<()> {
    let p = d.join(MARQUE);
    std::fs::File::create(&p).and_then(|f| f.sync_all()).map_err(|e| socle::echec::du_disque(&p, &e))
}

/// Retire la marque du deplacement du dossier `d` (rien si elle n'y est pas).
fn demarquer(d: &Path) -> R<()> {
    socle::fichiers::effacer(&d.join(MARQUE))
}

/// Ce que porte un chemin d'un arbre, pour comparer une copie a l'original. Un lien ne se suit pas.
#[derive(Debug, PartialEq, Eq)]
enum Trace {
    Dossier,
    Fichier(Octets),
    Lien,
}

/// L'arbre des `noms` sous `racine` : chaque chemin relatif, et ce qu'il porte, sans suivre les liens.
fn arbre(racine: &Path, noms: &[OsString]) -> std::io::Result<BTreeMap<PathBuf, Trace>> {
    fn sous(racine: &Path, rel: &Path, t: &mut BTreeMap<PathBuf, Trace>) -> std::io::Result<()> {
        let m = std::fs::symlink_metadata(racine.join(rel))?;
        if m.file_type().is_symlink() {
            t.insert(rel.to_path_buf(), Trace::Lien);
        } else if m.is_dir() {
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

/// TOUT CE QUI SE VERIFIE AVANT LE DEPLACEMENT de l'installation `inst` vers `vers` : un dossier dit en entier, qui n'est
/// ni elle ni dedans, qui n'existe pas (le dossier qui le portera, si) ou est vide ; qu'on peut y ecrire ; et, sur un
/// autre disque, que la place suffit et qu'aucun lien n'est a copier. Un depot de travail (qui porte `.git`) ne se
/// deplace pas.
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
        false => return Err(socle::echec::au_chemin("Le dossier qui le portera n'existe pas. Rien n'a changé.", parent)),
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
                return Err(socle::echec::au_chemin("Ce nom est celui d'un fichier, pas d'un dossier. Rien n'a changé.", &vers));
            }
            // La marque d'un deplacement arrete en route n'est pas un contenu : le dossier qui ne porte qu'elle est vide.
            for e in std::fs::read_dir(&vers).map_err(|e| socle::echec::du_disque(&vers, &e))? {
                if e.map_err(|e| socle::echec::du_disque(&vers, &e))?.file_name() != MARQUE {
                    return Err(socle::echec::au_chemin("Ce dossier n'est pas vide : choisir un dossier neuf, ou vide. Rien n'a changé.", &vers));
                }
            }
            vers.clone()
        }
        false => crate::installation::dessus(&vers),
    };
    // L'essai : un fichier de l'installation renomme dans le nouveau lieu dit le meme disque et qu'on y ecrit.
    let essai = inst.join(ESSAI);
    let la_bas = lieu.join(ESSAI);
    std::fs::write(&essai, b"").map_err(|e| socle::echec::du_disque(&essai, &e))?;
    match std::fs::rename(&essai, &la_bas) {
        Ok(()) => socle::fichiers::effacer(&la_bas)?,
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            socle::fichiers::effacer(&essai)?;
            if let Err(e) = std::fs::write(&la_bas, b"") {
                return Err(pas_d_ecriture(&lieu, &e));
            }
            socle::fichiers::effacer(&la_bas)?;
            let t = arbre(&inst, &entrees(&inst, &ce_qui_reste(&inst)?)?).map_err(|e| socle::echec::du_disque(&inst, &e))?;
            if let Some(lien) = t.iter().find(|(_, x)| **x == Trace::Lien).map(|(p, _)| p) {
                return Err(socle::echec::au_chemin(LIEN, &inst.join(lien)));
            }
            let faut = t.values().fold(Octets::ZERO, |s, x| match x {
                Trace::Fichier(o) => s + *o,
                Trace::Dossier | Trace::Lien => s,
            });
            let libre = crate::diagnostic::octets_libres(&lieu)
                .map_err(|e| refus(format!("La place libre de ce disque ne se lit pas ({}). Rien n'a changé.", e)))?;
            if Octets::lue(libre) < faut {
                return Err(refus(format!("La place manque sur ce disque : il faut {}, il n'y a que {}. Rien n'a changé.", faut, Octets::lue(libre))));
            }
        }
        Err(e) => {
            socle::fichiers::effacer(&essai)?;
            return Err(pas_d_ecriture(&lieu, &e));
        }
    }
    Ok(Verdict::Possible(vers))
}

/// Un lien de l'installation, que la copie sur un autre disque ne suit pas.
const LIEN: &str = "Un fichier du logiciel est un lien : il ne se copie pas sur un autre disque. Rien n'a changé.";

/// Le refus d'un dossier ou l'on n'ecrit pas : le dossier et ce que le systeme a dit, a part.
fn pas_d_ecriture(lieu: &Path, e: &std::io::Error) -> socle::echec::Echec {
    socle::echec::refus_avec(
        "On ne peut pas écrire dans ce dossier. Rien n'a changé.",
        socle::echec::Lieu { chemin: Some(lieu.to_string_lossy().into_owned()), detail: Some(e.to_string()), ..socle::echec::Lieu::default() },
    )
}

/// LE DEPLACEMENT DEMANDE (la route `logiciel-deplacer`) : verifie ; possible, `avant` dit s'il peut partir (ce que la
/// route exige de plus : la personne qui le sait, son mot de passe), puis la demande se pose (`maj/deplacer.json`). Rend
/// le verdict. Deja la : rien ne change, rien n'est exige.
pub fn demander(inst: &Path, vers: &Path, avant: impl FnOnce(&Path) -> R<()>) -> R<Verdict> {
    let v = verifier(inst, vers)?;
    match &v {
        Verdict::DejaLa => socle::fichiers::effacer(&dossier_maj(inst).join(DEMANDE))?,
        Verdict::Possible(vers) => {
            avant(vers)?;
            crate::maj::ecrire_json(
                &dossier_maj(inst).join(DEMANDE),
                &Demande { vers: vers.clone(), quoi: "Le dossier où le logiciel va, logiciel fermé (compagnie::deplacement).".to_string() },
            )?
        }
    }
    Ok(v)
}

/// La demande en attente, s'il y en a une.
fn demande(inst: &Path) -> R<Option<Demande>> {
    let p = dossier_maj(inst).join(DEMANDE);
    if !p.is_file() {
        return Ok(None);
    }
    socle::forme::lire_fichier(&p, "La demande de déplacement du logiciel ne se lit pas.").map(Some)
}

/// Le deplacement en cours, s'il y en a un (pour n'importe quelle installation du poste).
fn en_cours() -> R<Option<DeplacementEnCours>> {
    let p = chez_l_utilisateur(EN_COURS);
    if !p.is_file() {
        return Ok(None);
    }
    socle::forme::lire_fichier(&p, "Le journal du déplacement du logiciel ne se lit pas.").map(Some)
}

/// Ce qui se dit d'un deplacement en cours dont le journal ne se lit pas, la ou il concerne l'installation.
const ILLISIBLE: &str = "Ce logiciel s'est arrêté au milieu d'un déplacement, et le journal de ce déplacement ne se lit pas : on ne sait pas où sont ses dossiers, et rien ne s'y ouvre. Regarder ce dossier et celui où il allait, puis effacer ce journal.";

/// Ecrit le deplacement en cours, sur le disque avant de rendre la main : l'etape qui suit peut detruire.
fn ecrire_en_cours(ec: &DeplacementEnCours) -> R<()> {
    socle::ecriture::ecrire_json_sur_le_disque(&chez_l_utilisateur(EN_COURS), ec, 1)
}

/// `de` ou `vers` est-il l'installation `inst` ?
fn concerne(de: &Path, vers: &Path, inst: &Path) -> R<bool> {
    let i = resolu(inst)?;
    Ok(i == de || i == vers)
}

/// Ce qu'un deplacement en cours est pour une installation.
enum Ici {
    /// Il ne la concerne pas, ou il n'y en a pas.
    Rien,
    /// Il la concerne, et se lit.
    Lu(DeplacementEnCours),
    /// Son journal ne se lit pas, et l'installation porte la marque d'un deplacement.
    Illisible(socle::echec::Echec),
}

/// LE DEPLACEMENT EN COURS, VU DE L'INSTALLATION `inst` (ou de celle de la compagnie `racine`) : il la concerne s'il la
/// nomme ; s'il ne se lit pas, s'il porte la marque (`MARQUE`, dans elle ou au-dessus de la compagnie). Une installation
/// qu'il ne concerne pas n'en a rien a savoir, qu'il se lise ou non.
fn ici(inst: &Path, racine: Option<&Path>) -> R<Ici> {
    let marquee = inst.join(MARQUE).is_file() || racine.is_some_and(|r| r.ancestors().any(|a| a.join(MARQUE).is_file()));
    let ec = match en_cours() {
        Ok(None) => return Ok(Ici::Rien),
        Ok(Some(ec)) => ec,
        Err(e) if marquee => return Ok(Ici::Illisible(e)),
        Err(_) => return Ok(Ici::Rien),
    };
    let dedans = match racine {
        Some(r) => {
            let r = resolu_ce_qui_existe(r)?;
            socle::fichiers::sous(&r, &ec.de).is_some() || socle::fichiers::sous(&r, &ec.vers).is_some()
        }
        None => false,
    };
    match dedans || concerne(&ec.de, &ec.vers, inst)? {
        true => Ok(Ici::Lu(ec)),
        false => Ok(Ici::Rien),
    }
}

/// Le fichier du verrou du relais, ouvert (cree s'il manque).
fn fichier_du_verrou() -> R<std::fs::File> {
    let p = chez_l_utilisateur(VERROU);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| socle::echec::du_disque(d, &e))?;
    }
    std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&p).map_err(|e| socle::echec::du_disque(&p, &e))
}

/// Un relais tient-il le verrou (il travaille) ?
fn relais_au_travail() -> R<bool> {
    let f = fichier_du_verrou()?;
    match f.try_lock() {
        Ok(()) => Ok(false),
        Err(std::fs::TryLockError::WouldBlock) => Ok(true),
        Err(std::fs::TryLockError::Error(e)) => Err(socle::refus!("Le verrou du déplacement ne se lit pas : {}", e)),
    }
}

/// La phrase d'un deplacement en cours, selon qu'un relais y travaille (ou qu'on ne le sait pas), en mots ; ou il va se
/// dit a part.
fn phrase_en_cours(au_travail: &R<bool>) -> String {
    match au_travail {
        Ok(true) => "Le logiciel se déplace : rien ne s'y ouvre avant la fin, et sa fenêtre se rouvre d'elle-même.".to_string(),
        Ok(false) => "Le déplacement du logiciel s'est arrêté avant la fin : ouvrir le logiciel le finit, ou remet tout à l'ancien endroit.".to_string(),
        Err(e) => format!(
            "Le logiciel se déplace, et l'on ne sait pas si le déplacement travaille encore ({}) : rien ne s'y ouvre ; réessayer dans un moment.",
            e.phrase.trim_end_matches('.')
        ),
    }
}

/// D'ou et vers ou, pour une boite.
fn de_vers(ec: &DeplacementEnCours) -> String {
    format!("De : {}. Vers : {}.", dit(&ec.de), dit(&ec.vers))
}

/// CE QUE LA LECTURE « OU EST LE LOGICIEL » DIT D'UN DEPLACEMENT EN COURS qui concerne l'installation `inst` (elle
/// repond pendant qu'il dure) : sa phrase, en mots (il travaille ; il s'est arrete, et ce qui le finit ; son journal
/// illisible, et quoi faire), puis les lignes qui disent les dossiers ; `None` s'il ne la concerne pas.
pub fn en_cours_dit(inst: &Path) -> Option<(String, Vec<String>)> {
    match ici(inst, None) {
        Ok(Ici::Rien) => None,
        Ok(Ici::Lu(ec)) => Some((phrase_en_cours(&relais_au_travail()), vec![de_vers(&ec)])),
        Ok(Ici::Illisible(e)) => Some((format!("{} ({}.)", ILLISIBLE, e.phrase.trim_end_matches('.')), vec![format!("Ce journal : {}.", dit(&chez_l_utilisateur(EN_COURS)))])),
        Err(e) => Some((format!("On ne sait pas si ce logiciel se déplace : {}", e.phrase), Vec::new())),
    }
}

/// POUR LA LIGNE DE COMMANDE : un deplacement en cours qui concerne l'installation `inst` (ou celle de la compagnie
/// `racine`) refuse (rien ne s'y lit ni ne s'y ecrit tant qu'il dure), ses dossiers a part ; sinon rien.
pub fn refuser_s_il_se_deplace(inst: &Path, racine: Option<&Path>) -> R<()> {
    match ici(inst, racine)? {
        Ici::Rien => Ok(()),
        Ici::Lu(ec) => Err(socle::echec::refus_avec(
            phrase_en_cours(&relais_au_travail()),
            socle::echec::Lieu { chemin: Some(ec.de.to_string_lossy().into_owned()), detail: Some(format!("vers {}", ec.vers.to_string_lossy())), ..socle::echec::Lieu::default() },
        )),
        Ici::Illisible(e) => Err(socle::echec::refus_avec(
            ILLISIBLE,
            socle::echec::Lieu { chemin: Some(chez_l_utilisateur(EN_COURS).to_string_lossy().into_owned()), detail: Some(e.phrase), ..socle::echec::Lieu::default() },
        )),
    }
}

/// Le dernier deplacement de l'installation `inst` (vers elle ou depuis elle), ou ce qui l'a empeche ; `None` s'il n'y
/// en a pas.
pub fn dernier_ici(inst: &Path) -> R<Option<Dernier>> {
    let p = chez_l_utilisateur(DERNIER);
    if !p.is_file() {
        return Ok(None);
    }
    let d: Dernier = socle::forme::lire_fichier(&p, "Le dernier déplacement du logiciel ne se lit pas.")?;
    Ok(concerne(&d.de, &d.vers, inst)?.then_some(d))
}

/// Ce qui est dit a la personne quand le programme ne s'ouvre pas : une boite sous Windows (la fenetre n'a pas de
/// console), la sortie d'erreur ailleurs.
fn dire(texte: &str) {
    #[cfg(windows)]
    socle::systeme::dire_dans_une_boite("PuppetPaie", texte);
    #[cfg(not(windows))]
    eprintln!("{}", texte);
}

/// Retient ce qui s'est passe (`DERNIER`), et le note au journal de la mise a jour la ou le logiciel est entier.
fn retenir(fin: &Dernier) {
    let ecrit = crate::maj::ecrire_json(&chez_l_utilisateur(DERNIER), fin);
    if let Some(ou) = fin.ou() {
        noter(ou, &format!("Deplacement : {}", fin.phrase));
        if let Err(e) = ecrit {
            noter(ou, &format!("Deplacement : ce qui s'est passe ne se retient pas ({}).", e.phrase));
        }
    }
}

impl Dernier {
    /// Ce qui s'est passe a l'instant : `issue`, et ce qu'on en dit.
    fn maintenant(de: &Path, vers: &Path, issue: Issue, phrase: String) -> Dernier {
        Dernier { quand: socle::temps::secondes().a_la_minute(), de: de.to_path_buf(), vers: vers.to_path_buf(), issue, phrase }
    }

    /// Le logiciel est arrive dans `vers`.
    fn deplace(de: &Path, vers: &Path, restes: Vec<String>) -> Dernier {
        let phrase = match restes.is_empty() {
            true => format!("Le logiciel est arrivé ici, depuis {}.", dit(de)),
            false => format!("Le logiciel est arrivé ici, depuis {}. L'ancien dossier ne s'est pas effacé entier : {}", dit(de), restes.join(" ; ")),
        };
        Dernier::maintenant(de, vers, Issue::Deplace, phrase)
    }

    /// Le logiciel est reste dans `de`, entier, pour la raison `pourquoi`.
    fn reste(de: &Path, vers: &Path, pourquoi: &str) -> Dernier {
        let phrase = format!("Le logiciel n'est pas allé dans {} : {} Il est resté dans {}, entier.", dit(vers), pourquoi, dit(de));
        Dernier::maintenant(de, vers, Issue::Reste, phrase)
    }

    /// Le logiciel n'est entier nulle part : `pourquoi`, puis ce qui est ou (`ou_est`).
    fn coupe(de: &Path, vers: &Path, pourquoi: &str, ou_est: &str) -> Dernier {
        let phrase = format!(
            "Le déplacement du logiciel de {} vers {} s'est arrêté en route ({}) et tout n'est pas revenu : {}. Le relancer réessaie de tout remettre dans {}.",
            dit(de),
            dit(vers),
            pourquoi.trim_end_matches('.'),
            ou_est,
            dit(de)
        );
        Dernier::maintenant(de, vers, Issue::Coupe, phrase)
    }

    /// Ou le logiciel est entier ; `None` s'il est coupe.
    fn ou(&self) -> Option<&Path> {
        match self.issue {
            Issue::Deplace => Some(&self.vers),
            Issue::Reste => Some(&self.de),
            Issue::Coupe => None,
        }
    }
}

/// PARTIR : la demande s'efface (sinon rien ne part : elle se rejouerait a chaque lancement), l'installation se marque,
/// le deplacement en cours s'ecrit, et le relais se lance ; s'il ne se lance pas, le deplacement en cours et la marque
/// s'effacent.
fn partir(inst: &Path, vers: &Path) -> R<()> {
    socle::fichiers::effacer(&dossier_maj(inst).join(DEMANDE))
        .map_err(|e| e.reformuler(|m| format!("la demande ne s'efface pas ({}) ; rien ne part, pour ne pas la rejouer à chaque lancement.", m)))?;
    let de = resolu(inst)?;
    marquer(&de)?;
    let marque = de.join(MARQUE);
    let ec = DeplacementEnCours {
        de,
        vers: vers.to_path_buf(),
        cree: false,
        noms: Vec::new(),
        etape: Etape::Attente,
        quoi: "Le déplacement du logiciel en cours (compagnie::deplacement) : rien ne s'ouvre dans ces deux dossiers tant qu'il est là.".to_string(),
    };
    if let Err(e) = ecrire_en_cours(&ec) {
        return Err(socle::fichiers::effacer_apres(e, &marque));
    }
    lancer_relais(inst).map_err(|e| socle::fichiers::effacer_apres(socle::fichiers::effacer_apres(e, &chez_l_utilisateur(EN_COURS)), &marque))
}

/// AU LANCEMENT DE LA FENETRE, AVANT LE VERROU DE SON INSTALLATION (qui ecrit son creneau dans l'installation) : un
/// deplacement en cours la concerne (`ici`). Rend `true` si le programme doit s'arreter sans rien ouvrir : un relais y
/// travaille (ou on ne le sait pas), on le dit ; aucun relais, un relais le reprend ; illisible, on le dit (on ne sait
/// pas ou sont les dossiers). Une installation qu'il ne concerne pas s'ouvre.
pub fn en_cours_au_lancement() -> bool {
    let inst = crate::installation::base();
    let texte = match ici(&inst, None) {
        Ok(Ici::Rien) => return false,
        Ok(Ici::Lu(ec)) => match relais_au_travail() {
            Ok(false) => match lancer_relais(&inst) {
                Ok(()) => return true,
                Err(e) => format!("{} Le relais qui le reprend ne se lance pas : {} {}", phrase_en_cours(&Ok(false)), e.phrase, de_vers(&ec)),
            },
            autre => format!("{} {}", phrase_en_cours(&autre), de_vers(&ec)),
        },
        Ok(Ici::Illisible(e)) => format!("{} ({}) Ce journal : {}.", ILLISIBLE, e.phrase.trim_end_matches('.'), dit(&chez_l_utilisateur(EN_COURS))),
        Err(e) => format!("On ne sait pas si ce logiciel se déplace : {} Rien ne s'ouvre ; réessayer dans un moment.", e.phrase),
    };
    dire(&texte);
    true
}

/// AU LANCEMENT DE LA FENETRE, le verrou de son installation pris (aucune autre fenetre ne l'a ouverte) : une demande
/// posee part (`partir`) ; rend `true`, le programme s'arrete sans rien ouvrir. Ce qui l'empeche se retient, et le
/// logiciel s'ouvre ou il est.
pub fn au_lancement() -> bool {
    let inst = crate::installation::base();
    let d = match demande(&inst) {
        Ok(None) => return false,
        Ok(Some(d)) => d,
        Err(e) => {
            let pourquoi = match socle::fichiers::effacer(&dossier_maj(&inst).join(DEMANDE)) {
                Ok(()) => format!("{} Elle est effacée : redemander.", e.phrase),
                Err(x) => format!("{} Elle ne s'efface pas non plus ({}).", e.phrase, x.phrase),
            };
            retenir(&Dernier::maintenant(&inst, &inst, Issue::Reste, pourquoi));
            return false;
        }
    };
    match partir(&inst, &d.vers) {
        Ok(()) => true,
        Err(e) => {
            retenir(&Dernier::reste(&inst, &d.vers, &e.phrase));
            false
        }
    }
}

/// LE DOSSIER OU UN DEPLACEMENT DEMANDE VA, s'il y en a un en attente dans l'installation `inst` (pose en ligne de
/// commande, il part au prochain lancement de la fenetre) ; la demande illisible : son refus.
pub fn demande_posee(inst: &Path) -> R<Option<PathBuf>> {
    Ok(demande(inst)?.map(|d| d.vers))
}

/// APRES LA ROUTE, DANS LA FENETRE, SES ENVOIS FINIS : une demande posee part (`partir`) ; a la fenetre de se fermer
/// aussitot (`true`). Elle ne part pas : le refus le dit.
pub fn lancer_s_il_est_demande(inst: &Path) -> R<bool> {
    let Some(d) = demande(inst)? else { return Ok(false) };
    partir(inst, &d.vers).map_err(|e| e.reformuler(|m| format!("Le logiciel ne se déplace pas : {} Rien n'a changé.", m)))?;
    Ok(true)
}

/// Le programme de la fenetre, copie dans le dossier temporaire du poste, prend le relais ; il attendra la fin de CE
/// processus.
fn lancer_relais(inst: &Path) -> R<()> {
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
    c.arg(RELAIS).arg("--attendre").arg(numero.to_string());
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

/// LE RELAIS (`puppetpaie deplacer-relais --attendre PID`), lance du dossier temporaire du poste : prend le verrou (un
/// autre relais au travail, il attend sa fin), relit le deplacement en cours (parti : un autre relais l'a fini, rien a
/// faire), attend la fin du programme qui l'a lance, deplace ou reprend, retient ce qui s'est passe, efface le
/// deplacement en cours, rend le verrou, et rouvre la fenetre la ou le logiciel est entier. Coupe, ou le deplacement en
/// cours qui ne s'efface pas : une boite le dit, et la fenetre ne se rouvre pas (elle relancerait le relais sans fin).
pub fn relais(args: &[socle::ligne::Argument]) -> std::process::ExitCode {
    let args = socle::ligne::textes(args);
    let Some(Ok(lanceur)) = args.iter().position(|a| a == "--attendre").and_then(|i| args.get(i + 1)).map(|p| p.parse::<u32>()) else {
        return std::process::ExitCode::from(2);
    };
    let verrou = match fichier_du_verrou().and_then(|f| f.lock().map(|()| f).map_err(|e| socle::refus!("Le verrou du déplacement ne se prend pas : {}", e))) {
        Ok(f) => f,
        Err(e) => {
            dire(&format!("Le logiciel ne se déplace pas : {}", e.phrase));
            return std::process::ExitCode::FAILURE;
        }
    };
    let ec = match en_cours() {
        Ok(Some(ec)) => ec,
        Ok(None) => return std::process::ExitCode::SUCCESS,
        Err(e) => {
            drop(verrou);
            dire(&format!("{} ({}) Ce journal : {}.", ILLISIBLE, e.phrase.trim_end_matches('.'), dit(&chez_l_utilisateur(EN_COURS))));
            return std::process::ExitCode::FAILURE;
        }
    };
    let (de, vers) = (ec.de.clone(), ec.vers.clone());
    // Le lanceur seul s'attend : celui qui a demande le deplacement (`partir`), ou celui qui le reprend
    // (`en_cours_au_lancement`). Un numero ecrit plus tot pourrait etre, depuis, celui d'un autre programme.
    let arrete = match crate::maj::attendre_fin(lanceur, &de) {
        Ok(true) => Ok(()),
        Ok(false) => Err("le programme qui l'a lancé ne s'est pas arrêté.".to_string()),
        Err(e) => Err(format!("on ne sait pas si le programme qui l'a lancé s'est arrêté ({}).", e.phrase)),
    };
    let fin = match arrete {
        // Rien n'a ete touche par ce relais : ce qui est ou se lit sur le disque, pas dans le journal.
        Err(pourquoi) => match rien_n_est_parti(&ec) {
            Ok(true) => Dernier::reste(&de, &vers, &pourquoi),
            Ok(false) => Dernier::coupe(&de, &vers, &pourquoi, "rien n'a été repris, une partie est dans le nouveau dossier"),
            Err(e) => Dernier::coupe(&de, &vers, &pourquoi, &format!("rien n'a été repris, et ce qui est où ne se lit pas ({})", e.phrase.trim_end_matches('.'))),
        },
        Ok(()) => reprendre(ec),
    };
    retenir(&fin);
    // Coupe, le deplacement en cours et ses marques restent : le lancement suivant reprend. Sinon le journal s'efface,
    // puis les marques. La boite ne se pose qu'apres le verrou rendu.
    let suite = match fin.ou() {
        None => Err(fin.phrase.clone()),
        Some(ou) => match socle::fichiers::effacer(&chez_l_utilisateur(EN_COURS)) {
            Err(e) => Err(format!(
                "{} Le journal du déplacement ne s'efface pas ({}) : l'effacer, puis ouvrir le logiciel. Ce journal : {}.",
                fin.phrase,
                e.phrase.trim_end_matches('.'),
                dit(&chez_l_utilisateur(EN_COURS))
            )),
            Ok(()) => {
                for d in [&de, &vers] {
                    if let Err(e) = demarquer(d) {
                        noter(ou, &format!("Deplacement : la marque du deplacement ne s'efface pas ({}).", e.phrase));
                    }
                }
                Ok(ou.to_path_buf())
            }
        },
    };
    drop(verrou);
    let ou = match suite {
        Ok(ou) => ou,
        Err(texte) => {
            dire(&texte);
            return std::process::ExitCode::FAILURE;
        }
    };
    let logiciel = ou.join(crate::maj::LOGICIEL);
    if let Err(e) = std::process::Command::new(crate::maj::programme(&logiciel, "puppetpaie")).current_dir(&logiciel).spawn() {
        dire(&format!("{} La fenêtre ne se rouvre pas ({}) : ouvrir le logiciel dans {}.", fin.phrase, e, dit(&ou)));
    }
    std::process::ExitCode::SUCCESS
}

/// LE DEPLACEMENT, du debut : verifie de nouveau, cree le nouveau dossier s'il le faut, ecrit l'etape, renomme ; sur un
/// autre disque, copie. Un renommage refuse : tout revient, et l'on retente, une minute au plus.
fn commencer(mut ec: DeplacementEnCours) -> Dernier {
    let de = ec.de.clone();
    let mut essai = 1;
    loop {
        let vers = match verifier(&de, &ec.vers) {
            Ok(Verdict::Possible(v)) => v,
            Ok(Verdict::DejaLa) => return Dernier::reste(&de, &ec.vers, "c'est déjà là qu'il est."),
            Err(e) => return Dernier::reste(&de, &ec.vers, &e.phrase),
        };
        ec.vers = vers.clone();
        let preparer = || -> R<(bool, Vec<PathBuf>)> {
            let noms = entrees(&de, &ce_qui_reste(&de)?)?.into_iter().map(PathBuf::from).collect();
            let cree = !socle::fichiers::existe_lu(&vers)?;
            if cree {
                std::fs::create_dir(&vers).map_err(|e| socle::echec::du_disque(&vers, &e))?;
            }
            marquer(&vers).map_err(|e| match cree {
                true => socle::fichiers::effacer_apres(e, &vers),
                false => e,
            })?;
            Ok((cree, noms))
        };
        match preparer() {
            Ok((cree, noms)) => {
                ec.cree = cree;
                ec.noms = noms;
            }
            Err(e) => return Dernier::reste(&de, &vers, &e.phrase),
        }
        ec.etape = Etape::Renommage;
        if let Err(e) = ecrire_en_cours(&ec) {
            return match revenir(&ec, false) {
                Ok(()) => Dernier::reste(&de, &vers, &e.phrase),
                Err(ou) => Dernier::coupe(&de, &vers, &e.phrase, &ou),
            };
        }
        let mut renommes = 0;
        let mut refuse = None;
        for nom in &ec.noms {
            match std::fs::rename(de.join(nom), vers.join(nom)) {
                Ok(()) => renommes += 1,
                Err(e) if e.kind() == ErrorKind::CrossesDevices && renommes == 0 => {
                    ec.etape = Etape::Copie;
                    return copier(ec);
                }
                Err(e) => {
                    refuse = Some(socle::echec::du_disque(&de.join(nom), &e).phrase);
                    break;
                }
            }
        }
        let Some(pourquoi) = refuse else { return finir_les_renommages(ec) };
        if let Err(ou) = revenir(&ec, false) {
            return Dernier::coupe(&de, &vers, &pourquoi, &ou);
        }
        ec.etape = Etape::Attente;
        if essai >= ESSAIS_DU_RENOMMAGE || ecrire_en_cours(&ec).is_err() {
            return Dernier::reste(&de, &vers, &pourquoi);
        }
        noter(&de, &format!("Deplacement : le dossier ne se laisse pas encore deplacer ({}) ; nouvel essai.", pourquoi));
        essai += 1;
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// LES NOMS D'UN DEPLACEMENT, LUS SUR LE DISQUE : ceux du journal, ceux de l'ancien dossier (hors ce qui reste) et ceux
/// du nouveau (hors sa marque), `logiciel/` le dernier. Le journal peut retarder sur le disque (sous Windows, son nouveau
/// nom n'est pas sur et certain d'y etre quand le courant tombe) : il ne dit pas seul ce qui est ou.
fn noms_sur_le_disque(ec: &DeplacementEnCours) -> R<Vec<PathBuf>> {
    let mut v: Vec<OsString> = ec.noms.iter().map(|n| n.clone().into_os_string()).collect();
    let mut ajouter = |noms: Vec<OsString>| {
        for n in noms {
            if !v.contains(&n) {
                v.push(n);
            }
        }
    };
    if ec.de.is_dir() {
        ajouter(entrees(&ec.de, &ce_qui_reste(&ec.de)?)?);
    }
    if ec.vers.is_dir() {
        ajouter(entrees(&ec.vers, &[OsString::from(MARQUE)])?);
    }
    v.sort_by_key(|n| n.as_os_str() == crate::maj::LOGICIEL);
    Ok(v.into_iter().map(PathBuf::from).collect())
}

/// Rien du deplacement n'est dans le nouveau dossier (lu sur le disque).
fn rien_n_est_parti(ec: &DeplacementEnCours) -> R<bool> {
    Ok(noms_sur_le_disque(ec)?.iter().all(|n| std::fs::symlink_metadata(ec.vers.join(n)).is_err()))
}

/// UN DEPLACEMENT REPRIS. LA REPRISE NE CROIT PAS LE JOURNAL SUR PAROLE : elle regarde les deux dossiers, nom par nom.
/// - Rien dans le nouveau : a l'etape d'attente, le deplacement commence ; sinon tout revient (les pointeurs aussi).
/// - Aucun nom des deux cotes (des renommages) : tout dans le nouveau, c'est fini ; sinon tout revient.
/// - Des noms des deux cotes (une copie) : si tout ce que la copie porte est encore dans l'ancien, l'ancien est entier,
///   la copie s'efface ; si l'ancien a perdu des noms (son effacement avait commence, la copie comparee et entiere) et
///   que tout ce qui lui reste est pareil dans la copie, l'effacement se finit ; autrement, rien n'est touche : coupe.
fn reprendre(mut ec: DeplacementEnCours) -> Dernier {
    let (de, vers) = (ec.de.clone(), ec.vers.clone());
    let arrete = "le déplacement s'était arrêté en route, le poste éteint ou le relais arrêté.";
    match noms_sur_le_disque(&ec) {
        Ok(noms) => ec.noms = noms,
        Err(e) => return Dernier::coupe(&de, &vers, arrete, &format!("rien n'est touché : ce qui est où ne se lit pas ({})", e.phrase.trim_end_matches('.'))),
    }
    let la = |d: &Path, n: &Path| std::fs::symlink_metadata(d.join(n)).is_ok();
    let dans_le_neuf = ec.noms.iter().filter(|n| la(&vers, n)).count();
    let dans_l_ancien = ec.noms.iter().filter(|n| la(&de, n)).count();
    let des_deux = ec.noms.iter().filter(|n| la(&de, n) && la(&vers, n)).count();
    if dans_le_neuf == 0 {
        if ec.etape == Etape::Attente {
            return commencer(ec);
        }
        return match revenir(&ec, true) {
            Ok(()) => Dernier::reste(&de, &vers, arrete),
            Err(ou) => Dernier::coupe(&de, &vers, arrete, &ou),
        };
    }
    if des_deux == 0 {
        if dans_l_ancien == 0 {
            return finir_les_renommages(ec);
        }
        return match revenir(&ec, true) {
            Ok(()) => Dernier::reste(&de, &vers, arrete),
            Err(ou) => Dernier::coupe(&de, &vers, arrete, &ou),
        };
    }
    let presents = |d: &Path| ec.noms.iter().filter(|n| la(d, n)).map(|n| n.clone().into_os_string()).collect::<Vec<_>>();
    let (a, b) = match (arbre(&de, &presents(&de)), arbre(&vers, &presents(&vers))) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => return Dernier::coupe(&de, &vers, arrete, &format!("rien n'est touché : ce qui est où ne se lit pas ({})", e)),
    };
    if b.keys().all(|k| a.contains_key(k)) {
        return Dernier::reste(&de, &vers, &defaire_la_copie(&ec, arrete));
    }
    if a.iter().all(|(k, t)| b.get(k) == Some(t)) {
        let mut restes = Vec::new();
        if let Err(e) = repointer(&de, &vers) {
            restes.push(format!("ce qui pointait vers l'ancien dossier ne s'est pas réécrit ({})", e.phrase));
        }
        return effacer_l_ancien(&ec, restes);
    }
    Dernier::coupe(
        &de,
        &vers,
        arrete,
        &format!("rien n'est touché : l'ancien dossier et la copie dans {} diffèrent l'un de l'autre, et aucun des deux n'est sûrement entier", dit(&vers)),
    )
}

/// Tout est renomme : ce qui pointe vers l'ancien dossier se reecrit (sinon tout revient), puis l'ancien dossier
/// s'efface s'il est vide.
fn finir_les_renommages(ec: DeplacementEnCours) -> Dernier {
    if let Err(e) = dossier_sur_le_disque(&ec.vers).and_then(|()| dossier_sur_le_disque(&ec.de)).and_then(|()| repointer(&ec.de, &ec.vers)) {
        return match revenir(&ec, true) {
            Ok(()) => Dernier::reste(&ec.de, &ec.vers, &e.phrase),
            Err(ou) => Dernier::coupe(&ec.de, &ec.vers, &e.phrase, &ou),
        };
    }
    effacer_l_ancien(&ec, Vec::new())
}

/// Le logiciel est au nouvel endroit, entier : ce qui reste de l'ancien s'efface, et ce qui ne s'efface pas se dit.
fn effacer_l_ancien(ec: &DeplacementEnCours, mut restes: Vec<String>) -> Dernier {
    for nom in &ec.noms {
        let p = ec.de.join(nom);
        if std::fs::symlink_metadata(&p).is_ok() {
            if let Err(e) = socle::fichiers::effacer(&p) {
                restes.push(e.phrase);
            }
        }
    }
    // Le dossier de l'utilisateur reste (il porte les fenetres et le journal du deplacement) ; un autre, vide, s'efface
    // avec sa marque. Deja parti (un effacement repris) : rien a faire.
    if std::fs::symlink_metadata(&ec.de).is_ok() {
        match est_le_dossier_de_l_utilisateur(&ec.de) {
            Ok(true) => {}
            Ok(false) => {
                if let Err(e) = demarquer(&ec.de).and_then(|()| std::fs::remove_dir(&ec.de).map_err(|e| socle::echec::du_disque(&ec.de, &e))) {
                    restes.push(e.phrase);
                }
            }
            Err(e) => restes.push(e.phrase),
        }
    }
    Dernier::deplace(&ec.de, &ec.vers, restes)
}

/// TOUT REVIENT apres des renommages : (si `pointeurs`) ce qui pointe vers le nouveau dossier revient a l'ancien ;
/// chaque entree qui est dans le nouveau dossier, et pas dans l'ancien, y revient, `logiciel/` la premiere ; tout
/// revenu, le nouveau dossier perd sa marque, et, cree par le deplacement, s'efface s'il est vide. Ce qui ne revient
/// pas : ce qui est ou, en mots.
fn revenir(ec: &DeplacementEnCours, pointeurs: bool) -> Result<(), String> {
    let (de, vers) = (&ec.de, &ec.vers);
    let mut fautes = Vec::new();
    if pointeurs {
        if let Err(e) = repointer(vers, de) {
            fautes.push(format!("ce qui pointait vers {} n'y pointe plus entier ({})", dit(de), e.phrase));
        }
    }
    let mut la_bas = Vec::new();
    for nom in ec.noms.iter().rev() {
        let (ici, ailleurs) = (de.join(nom), vers.join(nom));
        if std::fs::symlink_metadata(&ailleurs).is_err() {
            continue;
        }
        // Le meme nom des deux cotes : aucun ne s'ecrase sur l'autre, et le logiciel n'est pas entier.
        if std::fs::symlink_metadata(&ici).is_ok() {
            la_bas.push(format!("{} (le même nom est aussi dans {})", nom.to_string_lossy(), dit(de)));
            continue;
        }
        if let Err(e) = std::fs::rename(&ailleurs, &ici) {
            la_bas.push(format!("{} ({})", nom.to_string_lossy(), e));
        }
    }
    if !la_bas.is_empty() {
        fautes.push(format!("dans {} : {} ; le reste dans {}", dit(vers), la_bas.join(", "), dit(de)));
    }
    if fautes.is_empty() {
        if let Err(e) = demarquer(vers) {
            noter(de, &format!("Deplacement : la marque du nouveau dossier ne s'efface pas ({}).", e.phrase));
        }
    }
    if ec.cree {
        if let Err(e) = std::fs::remove_dir(vers) {
            if fautes.is_empty() {
                // Le logiciel est revenu entier : le dossier vide qui reste n'est qu'un reste.
                noter(de, &format!("Deplacement : le dossier {} est reste, vide ({}).", dit(vers), e));
            }
        }
    }
    match fautes.is_empty() {
        true => Ok(()),
        false => Err(fautes.join(" ; ")),
    }
}

/// LA COPIE SUR UN AUTRE DISQUE : chaque entree se copie (chaque fichier ecrit sur le disque), la copie se compare a
/// l'original (chaque dossier, chaque fichier, sa taille et son empreinte), ce qui pointe vers l'ancien dossier se
/// reecrit, l'etape d'effacement s'ecrit, puis l'original s'efface. Au moindre echec, la copie s'efface et les
/// pointeurs reviennent : l'original est entier.
fn copier(mut ec: DeplacementEnCours) -> Dernier {
    let (de, vers) = (ec.de.clone(), ec.vers.clone());
    let noms: Vec<OsString> = ec.noms.iter().map(|n| n.clone().into_os_string()).collect();
    let fait = ecrire_en_cours(&ec).and_then(|()| copier_et_comparer(&de, &vers, &noms)).and_then(|()| {
        repointer(&de, &vers)?;
        ec.etape = Etape::Effacement;
        ecrire_en_cours(&ec)
    });
    match fait {
        Ok(()) => effacer_l_ancien(&ec, Vec::new()),
        Err(e) => Dernier::reste(&de, &vers, &defaire_la_copie(&ec, &e.phrase)),
    }
}

/// La copie s'efface, les pointeurs reviennent a l'ancien dossier, le nouveau dossier cree s'efface : l'original est
/// entier. Rend `pourquoi`, suivi de ce qui ne s'est pas defait.
fn defaire_la_copie(ec: &DeplacementEnCours, pourquoi: &str) -> String {
    let mut restes = Vec::new();
    if let Err(e) = repointer(&ec.vers, &ec.de) {
        restes.push(format!("ce qui pointait vers {} n'y pointe plus entier ({})", dit(&ec.de), e.phrase));
    }
    for nom in &ec.noms {
        let p = ec.vers.join(nom);
        if std::fs::symlink_metadata(&p).is_ok() {
            if let Err(e) = socle::fichiers::effacer(&p) {
                restes.push(format!("la copie ne s'efface pas entière ({})", e.phrase));
            }
        }
    }
    if restes.is_empty() {
        if let Err(e) = demarquer(&ec.vers) {
            restes.push(format!("la marque du nouveau dossier ne s'efface pas ({})", e.phrase));
        }
    }
    if ec.cree && restes.is_empty() {
        if let Err(e) = std::fs::remove_dir(&ec.vers) {
            restes.push(format!("le dossier {} est resté ({})", dit(&ec.vers), e));
        }
    }
    match restes.is_empty() {
        true => pourquoi.to_string(),
        false => format!("{} ({}).", pourquoi.trim_end_matches('.'), restes.join(" ; ")),
    }
}

/// COPIE UN FICHIER, sur le disque avant de rendre la main : le contenu s'ecrit dans un fichier neuf, ouvert en
/// ecriture, qui se vide sur le disque (`sync_all` ; sous Windows il exige un fichier ouvert en ecriture), puis prend
/// les dates et les droits de l'original (`m`) : un fichier en lecture seule se copie, et le reste.
fn copier_un_fichier(de: &Path, vers: &Path, m: &std::fs::Metadata) -> std::io::Result<()> {
    let mut source = std::fs::File::open(de)?;
    let mut copie = std::fs::OpenOptions::new().write(true).create_new(true).open(vers)?;
    std::io::copy(&mut source, &mut copie)?;
    copie.set_times(std::fs::FileTimes::new().set_modified(m.modified()?).set_accessed(m.accessed()?))?;
    copie.sync_all()?;
    drop(copie);
    std::fs::set_permissions(vers, m.permissions())
}

/// Les noms du dossier `d` sur le disque : hors de Windows, le dossier ouvert se vide (`sync_all`). Sous Windows, la
/// bibliotheque standard n'ouvre pas un dossier : rien (la reprise regarde les dossiers, `reprendre`).
#[cfg(unix)]
fn dossier_sur_le_disque(d: &Path) -> R<()> {
    std::fs::File::open(d).and_then(|f| f.sync_all()).map_err(|e| socle::echec::du_disque(d, &e))
}

#[cfg(not(unix))]
fn dossier_sur_le_disque(_d: &Path) -> R<()> {
    Ok(())
}

/// L'empreinte SHA-256 du contenu d'un fichier.
fn empreinte(p: &Path) -> std::io::Result<[u8; 32]> {
    use sha2::{Digest, Sha256};
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut tampon = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut tampon)?;
        if n == 0 {
            return Ok(h.finalize().into());
        }
        h.update(&tampon[..n]);
    }
}

/// Copie les `noms` de `de` dans `vers`, chaque fichier ecrit sur le disque, sans suivre ni copier un lien ; puis
/// compare : chaque dossier, chaque fichier, sa taille et son empreinte.
fn copier_et_comparer(de: &Path, vers: &Path, noms: &[OsString]) -> R<()> {
    fn copier(de: &Path, vers: &Path) -> R<()> {
        let m = std::fs::symlink_metadata(de).map_err(|e| socle::echec::du_disque(de, &e))?;
        if m.file_type().is_symlink() {
            return Err(socle::echec::au_chemin(LIEN, de));
        }
        if m.is_dir() {
            std::fs::create_dir(vers).map_err(|e| socle::echec::du_disque(vers, &e))?;
            for e in std::fs::read_dir(de).map_err(|e| socle::echec::du_disque(de, &e))? {
                let nom = e.map_err(|e| socle::echec::du_disque(de, &e))?.file_name();
                copier(&de.join(&nom), &vers.join(&nom))?;
            }
            // Ses noms sur le disque avant que rien ne s'efface (la ou le systeme le permet).
            return dossier_sur_le_disque(vers);
        }
        copier_un_fichier(de, vers, &m).map_err(|e| socle::echec::du_disque(vers, &e))
    }
    for nom in noms {
        copier(&de.join(nom), &vers.join(nom))?;
    }
    dossier_sur_le_disque(vers)?;
    if let Some(dessus) = vers.parent() {
        dossier_sur_le_disque(dessus)?;
    }
    let avant = arbre(de, noms).map_err(|e| socle::echec::du_disque(de, &e))?;
    let apres = arbre(vers, noms).map_err(|e| socle::echec::du_disque(vers, &e))?;
    let mut differents = avant.iter().filter(|(p, t)| apres.get(*p) != Some(*t)).count() + apres.keys().filter(|p| !avant.contains_key(*p)).count();
    for (rel, _) in avant.iter().filter(|(_, t)| matches!(t, Trace::Fichier(_))) {
        let (a, b) = (de.join(rel), vers.join(rel));
        if empreinte(&a).map_err(|e| socle::echec::du_disque(&a, &e))? != empreinte(&b).map_err(|e| socle::echec::du_disque(&b, &e))? {
            differents += 1;
        }
    }
    if differents > 0 {
        return Err(refus(format!("La copie n'est pas arrivée entière : {} fichier(s) ou dossier(s) manquent ou diffèrent.", differents)));
    }
    Ok(())
}

/// Un script PowerShell (`powershell.exe -NoProfile -NonInteractive -Command`), sa sortie en UTF-8 ; son refus sinon.
/// Les chemins passent par l'environnement du processus (`variables`), jamais par le texte du script : aucune apostrophe
/// d'un nom de dossier (droite ou typographique) ne peut le casser.
#[cfg(windows)]
fn powershell(script: &str, variables: &[(&str, &Path)]) -> R<String> {
    use std::os::windows::process::CommandExt;
    let tete = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $ErrorActionPreference = 'Stop'; ";
    let mut c = std::process::Command::new("powershell.exe");
    c.args(["-NoProfile", "-NonInteractive", "-Command"]).arg(format!("{}{}", tete, script));
    for (n, v) in variables {
        c.env(n, v);
    }
    let o = c
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
/// `None` hors de Windows.
#[cfg(windows)]
fn documents() -> R<Option<PathBuf>> {
    let d = powershell("[Environment]::GetFolderPath('MyDocuments')", &[])?;
    match d.is_empty() {
        true => Err(refus("Windows ne dit pas où est le dossier Documents.")),
        false => Ok(Some(PathBuf::from(d))),
    }
}

#[cfg(not(windows))]
fn documents() -> R<Option<PathBuf>> {
    Ok(None)
}

/// L'ANCIENNE INSTALLATION : celle qui EST le dossier de l'utilisateur (`installation::dossier_de_l_utilisateur`,
/// `AppData\Local\PuppetPaie` sous Windows, un dossier cache), et n'est pas portable. Rend le dossier des Documents ou
/// la ranger (`Documents\PuppetPaie`) ; `None` pour toute autre installation, ou hors de Windows.
pub fn ranger_dans_les_documents(inst: &Path) -> R<Option<PathBuf>> {
    if inst.join(crate::coffre::MARQUEUR_PORTABLE).is_file() || !est_le_dossier_de_l_utilisateur(inst)? {
        return Ok(None);
    }
    Ok(documents()?.map(|d| d.join(DANS_LES_DOCUMENTS)))
}

/// Le dossier que OneDrive synchronise et qui porte `p`, s'il y en a un : Windows nomme chacun dans une variable
/// (`VARIABLES_ONEDRIVE`) ; un chemin dessous est synchronise.
pub fn synchronise_par_onedrive(p: &Path) -> Option<PathBuf> {
    let bas = |x: &Path| x.to_string_lossy().to_lowercase().trim_end_matches(['\\', '/']).to_string();
    let p = bas(p);
    racines_onedrive().into_iter().find(|r| {
        let r = bas(r);
        p == r || p.starts_with(&format!("{}\\", r)) || p.starts_with(&format!("{}/", r))
    })
}

/// Les dossiers que OneDrive synchronise sur ce poste (`VARIABLES_ONEDRIVE`), chacun une fois.
pub fn racines_onedrive() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    for r in VARIABLES_ONEDRIVE.iter().filter_map(|n| crate::coffre::env_du_poste(n)).map(PathBuf::from) {
        if !v.contains(&r) {
            v.push(r);
        }
    }
    v
}

/// Ce qu'il faut savoir avant de mettre le logiciel dans un dossier que OneDrive synchronise (lequel se dit a part).
pub const AVERTIR_ONEDRIVE: &str = "Un dossier que OneDrive synchronise enverrait le coffre du logiciel, les paies et les numéros de sécurité sociale dans le nuage de Microsoft, et OneDrive peut bloquer un fichier pendant qu'il s'écrit.";

/// CE QUI POINTE VERS L'INSTALLATION se reecrit de `de` a `vers` : ce que l'installeur a pose
/// (`atelier/installeur/puppetpaie.nsi`), la cle de desinstallation sous `HKCU` (`InstallLocation`, `DisplayIcon`,
/// `UninstallString`, `QuietUninstallString`), si elle est celle de cette installation, et le raccourci du menu Demarrer
/// (`PuppetPaie.lnk` des programmes de l'utilisateur), s'il lance le programme de cette installation. Deja reecrit, rien
/// ne change. Hors de Windows, rien.
#[cfg(windows)]
fn repointer(de: &Path, vers: &Path) -> R<()> {
    let script = "$de = $env:PUPPETPAIE_DEPLACEMENT_DE; $vers = $env:PUPPETPAIE_DEPLACEMENT_VERS; \
         function Remplace([string]$v) { if (-not $v) { return $v }; [regex]::Replace($v, [regex]::Escape($de), $vers.Replace('$', '$$'), 'IgnoreCase') }; \
         $cle = 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PuppetPaie'; \
         if (Test-Path -LiteralPath $cle) { \
           $p = Get-ItemProperty -LiteralPath $cle; \
           if ($p.InstallLocation -and ($p.InstallLocation.TrimEnd('\\') -eq $de)) { \
             foreach ($n in 'InstallLocation', 'DisplayIcon', 'UninstallString', 'QuietUninstallString') { \
               if ($p.$n) { Set-ItemProperty -LiteralPath $cle -Name $n -Value (Remplace $p.$n) } } } }; \
         $lnk = Join-Path ([Environment]::GetFolderPath('Programs')) 'PuppetPaie.lnk'; \
         if (Test-Path -LiteralPath $lnk) { \
           $s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk); \
           if ($s.TargetPath.StartsWith($de + '\\', [StringComparison]::OrdinalIgnoreCase)) { \
             $s.TargetPath = Remplace $s.TargetPath; $s.WorkingDirectory = Remplace $s.WorkingDirectory; \
             $s.IconLocation = Remplace $s.IconLocation; $s.Save() } }";
    powershell(script, &[("PUPPETPAIE_DEPLACEMENT_DE", de), ("PUPPETPAIE_DEPLACEMENT_VERS", vers)]).map(|_| ())
}

#[cfg(not(windows))]
fn repointer(_de: &Path, _vers: &Path) -> R<()> {
    Ok(())
}
