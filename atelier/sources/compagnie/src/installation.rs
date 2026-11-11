//! L'INSTALLATION : ce qu'est une compagnie sur le poste, et les compagnies que l'accueil propose.
//!
//! A cote du code : `prive/` (la vraie), `compagnies/*` (les autres vraies), `fictives/*` (les compagnies fictives, livrees
//! avec le logiciel). La copie de developpement (`prive-dev/`) n'est jamais proposee. Bride (`bridage.rs`), l'accueil ne
//! montre que les fictives. Une compagnie est une copie, une fictive (livree ou ajoutee) d'apres son marqueur ; ajoutee,
//! elle dit le jour et le chemin de son ajout (`creation.json`). En ajouter une, en supprimer une, garder au deblocage
//! celles qu'on a ajoutees en demo : `routes::accueil`.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use socle::echec::R;
use socle::fichiers::dossier_du_code;

/// Le journal de la fenetre, sous la racine de la compagnie qu'elle sert.
pub const JOURNAL_INTERFACE: &str = "journal/interface.log";
/// La memoire de l'horloge d'une fictive (`horloge`), sous la racine de la compagnie : la compagnie d'essai ne la copie
/// pas (`essai`).
pub const FICHIER_HORLOGE: &str = "config/horloge.json";

pub const MARQUEUR: &str = "COPIE-DE-DEVELOPPEMENT.json";
/// La couleur d'une compagnie qui n'en a pas choisi : le rouge de la charte de l'accueil.
const COULEUR_DE_LA_CHARTE: &str = "#B22222";
/// La couleur de la compagnie, sous sa racine.
const AFFICHAGE: &str = "config/affichage.json";
const DOSSIER_ORIGINE: &str = ".origine";
/// Ce qui appartient au poste et survit a une remise a zero.
const PROPRES_AU_POSTE: [&str; 4] =
    [crate::incidents::DIAGNOSTIC, crate::problemes::SIGNALEMENTS, crate::incidents::INCIDENTS, JOURNAL_INTERFACE];

/// Le dossier qui porte `p`. La racine d'un disque n'a pas de dossier au-dessus d'elle : elle se porte elle-meme.
pub fn dessus(p: &Path) -> PathBuf {
    match p.parent() {
        Some(d) => d.to_path_buf(),
        None => p.to_path_buf(),
    }
}

/// Le dossier `donnees/` (le code livre : baremes, echeancier...) : `PUPPETPAIE_CODE` si elle est posee, sinon celui que
/// l'on trouve a cote du programme (`logiciel/puppetpaie.exe` -> `logiciel/donnees` ; `atelier/sources/moteur/bin/<systeme>/moteur`
/// -> `logiciel/donnees` en remontant), a defaut celui du depot qui a compile le programme.
/// Le chemin est rendu absolu et sans `..` : le moteur remonte de `moteur/` a `logiciel/` puis au depot par `parent()`.
pub fn fixer_le_code() {
    let valide = |p: &Path| p.join("echeancier-base.json").is_file();
    let mut candidats: Vec<PathBuf> = Vec::new();
    match std::env::var_os("PUPPETPAIE_CODE").filter(|v| !v.is_empty()) {
        // Posee, elle seule : rendue absolue et sans `..` quand elle nomme un dossier.
        Some(pose) => candidats.push(PathBuf::from(pose)),
        None => {
            if let Ok(exe) = std::env::current_exe() {
                for a in exe.ancestors().skip(1) {
                    candidats.push(a.join("donnees"));
                    candidats.push(a.join("logiciel").join("donnees"));
                }
            }
            candidats.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees"));
        }
    }
    for c in candidats {
        if !valide(&c) {
            continue;
        }
        // Un candidat qui ne se resout pas n'est pas le dossier du code : le suivant.
        if let Ok(abs) = c.canonicalize() {
            // Sous Windows, `canonicalize` rend un chemin `\\?\C:\...` que le reste du moteur ne compare pas.
            let abs = match abs.to_string_lossy().strip_prefix(r"\\?\") {
                Some(sans) => PathBuf::from(sans),
                None => abs.clone(),
            };
            std::env::set_var("PUPPETPAIE_CODE", abs);
            return;
        }
    }
}

/// Le dossier du logiciel (`logiciel/`) : celui qui porte le dossier du code (`fixer_le_code` l'a rendu absolu et sans
/// `..`).
pub fn logiciel() -> PathBuf {
    dessus(&dossier_du_code())
}

/// Ce qui est a cote du logiciel : `prive/`, `fictives/`, `compagnies/`, `maj/`. Le temps d'un rejeu de scenario
/// (`scenario::rejouer`), l'installation que le rejeu a montee, et elle seule.
pub fn base() -> PathBuf {
    // Un fil tombe en tenant le verrou ne change pas ce qu'il garde : la valeur se lit quand meme.
    let rejeu = match BASE_DU_REJEU.read() {
        Ok(b) => b.clone(),
        Err(tombe) => tombe.into_inner().clone(),
    };
    match rejeu {
        Some(b) => b,
        None => dessus(&logiciel()),
    }
}

/// L'INSTALLATION DU REJEU : un rejeu de scenario monte sa propre installation, bridee en toutes lettres
/// (`.edition.json`), et le moteur n'en lit pas d'autre -- ni le `.edition.json` du depot, ni le paquet soude d'a cote du
/// code. Pose et retiree par `scenario::rejouer` seulement ; un poste n'en a jamais.
static BASE_DU_REJEU: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);

pub fn poser_l_installation_du_rejeu(inst: Option<PathBuf>) {
    match BASE_DU_REJEU.write() {
        Ok(mut b) => *b = inst,
        Err(tombe) => *tombe.into_inner() = inst,
    }
}

/// Le fichier `p` lu en sa forme, s'il est la : `None` sans fichier.
fn lire_s_il_est_la<T: DeserializeOwned>(p: &Path, phrase: &str) -> R<Option<T>> {
    if !p.is_file() {
        return Ok(None);
    }
    socle::forme::lire_fichier(p, phrase).map(Some)
}

/// La vraie compagnie : `prive/` a cote du code, sinon le code lui-meme.
pub fn dossier_reel() -> PathBuf {
    let voisin = base().join("prive");
    if voisin.join(socle::fichiers::CONFIG).is_dir() {
        voisin
    } else {
        logiciel()
    }
}

pub fn est_copie(racine: &Path) -> bool {
    racine.join(MARQUEUR).is_file()
}

/// Le dossier des COMPAGNIES FICTIVES, a cote du logiciel, livrees
/// avec le logiciel, rien n'en part : leurs envois sont joues.
pub const DOSSIER_FICTIVES: &str = "fictives";
/// Le marqueur d'une compagnie fictive : `COPIE-DE-DEVELOPPEMENT.json`, dont `copie_de` commence par ceci.
pub const MARQUE_FICTIVE: &str = "compagnie fictive";

/// LE MARQUEUR D'UNE COPIE (`COPIE-DE-DEVELOPPEMENT.json`) : ce qu'elle copie (`copie_de`), quand, et ce qu'elle est.
/// Ses ecrivains : la livraison d'une fictive (`scenario::poser_livree` : « compagnie fictive <son dossier> », `quand` le
/// dernier geste rejoue, s'il en a un), le « + » d'un logiciel bride (`accueil::ajouter_venant`, avec la marque `ajoutee`), le banc
/// d'essai (`scenario::rendre_simulee`), la compagnie d'essai (`essai::monter`, avec la fictive copiee, `fictive`).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marqueur {
    pub copie_de: String,
    /// La fictive que copie une compagnie d'essai (son nom).
    #[serde(rename = "depuis", skip_serializing_if = "Option::is_none")]
    pub fictive: Option<String>,
    /// LA MARQUE D'UNE FICTIVE AJOUTEE (`est_ajoutee`) : `true`, sur elle seule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ajoutee: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quand: Option<socle::paris::InstantEcrit>,
    pub quoi: String,
}

/// Le marqueur de `racine` : `None` si elle n'en a pas (elle n'est pas une copie).
pub fn marqueur(racine: &Path) -> R<Option<Marqueur>> {
    lire_s_il_est_la(&racine.join(MARQUEUR), "Le marqueur de la compagnie (COPIE-DE-DEVELOPPEMENT.json) ne se lit pas.")
}

/// Ce que le marqueur d'une copie dit qu'elle copie (`copie_de`) ; `None` hors d'une copie.
pub fn copie_de(racine: &Path) -> R<Option<String>> {
    Ok(marqueur(racine)?.map(|m| m.copie_de))
}

/// Une COMPAGNIE FICTIVE : son marqueur le dit (ses envois sont joues, rien n'en part).
pub fn est_fictive(racine: &Path) -> R<bool> {
    Ok(copie_de(racine)?.is_some_and(|c| c.starts_with(MARQUE_FICTIVE)))
}

/// LA FICTIVE AJOUTEE : la compagnie que le « + » d'un logiciel bride ajoute. C'est une
/// fictive (`est_fictive` : elle vit sous `fictives/`, s'ouvre sans mot de passe, ne retient aucune version, rien ne part
/// d'elle a l'editeur qu'un incident ou un message), mais elle porte des donnees qu'on y a mises, pas celles d'une
/// fictive livree : ses envois NE SONT PAS JOUES, ILS SONT REFUSES (`bridage::branchement`), elle n'a pas de photo
/// d'origine ni de remise a zero, et la livraison des fictives (`compagnie::maj`) ne la touche jamais. SA MARQUE : le champ
/// `ajoutee` de son marqueur (`Marqueur`, que `accueil::ajouter_venant` ecrit) -- lu ICI et nulle part ailleurs.
///
/// UN DOSSIER DE `fictives/` SANS MARQUEUR DE FICTIVE (un marqueur retire) se lit aussi comme ajoute : l'accueil le
/// propose parmi les fictives, rien ne doit en partir, et rien n'en est joue.
pub fn est_ajoutee(racine: &Path) -> R<bool> {
    let m = marqueur(racine)?;
    let marque = m.as_ref().is_some_and(|m| m.ajoutee == Some(true));
    let fictive = m.as_ref().is_some_and(|m| m.copie_de.starts_with(MARQUE_FICTIVE));
    let sous_les_fictives = racine.parent().and_then(Path::file_name).is_some_and(|n| n == DOSSIER_FICTIVES);
    Ok(marque || (sous_les_fictives && !fictive))
}

/// UNE FICTIVE LIVREE : une fictive qui n'est pas ajoutee. Ses envois sont joues ; elle a sa photo d'origine et sa remise
/// a zero.
pub fn est_fictive_livree(racine: &Path) -> R<bool> {
    Ok(est_fictive(racine)? && !est_ajoutee(racine)?)
}

/// Le prefixe du dossier d'une fictive ajoutee. Les fictives livrees s'appellent `fictive-…` (le
/// dossier `fictives/` du depot) : une fictive ajoutee ne prend jamais le nom d'une livree, et la livraison, qui ne pose
/// pas une fictive dont le nom est pris (`maj::poser_les_fictives_pretes`), n'en est jamais empechee.
pub const PREFIXE_AJOUTEE: &str = "ajoutee-";

/// La fiche entreprise telle qu'elle vaut aujourd'hui.
fn fiche(racine: &Path) -> R<droit::fiche::ChampsEntreprise> {
    let f: droit::fiche::FicheEntrepriseDatee = droit::fiche::lire_fichier(&racine.join(droit::contexte::ENTREPRISE), droit::contexte::ENTREPRISE)?;
    Ok(f.au(&droit::contexte::jour_reel()))
}

/// Un texte de la fiche, s'il dit quelque chose.
fn dit(t: Option<&str>) -> Option<String> {
    t.filter(|s| !s.is_empty()).map(String::from)
}

/// Le nom d'une compagnie tel que sa tuile le dit : `nom_affichage`, sinon la raison sociale, sinon son dossier.
/// UNE FICHE QUI NE SE LIT PAS n'enleve pas la compagnie de l'accueil : elle se nomme par son dossier (on peut encore
/// l'ouvrir, qui dit pourquoi, ou la supprimer).
pub fn nom_de(racine: &Path) -> R<String> {
    Ok(nom_et_lieu(racine)?.0)
}

/// Le nom et le lieu de la tuile de `racine` ; une fiche qui ne se lit pas : le nom de son dossier, et pour lieu ce qui
/// ne se lit pas.
fn nom_et_lieu(racine: &Path) -> R<(String, String)> {
    let ent = match fiche(racine) {
        Ok(ent) => ent,
        Err(pourquoi) => return Ok((nom_dossier(racine)?, pourquoi.phrase)),
    };
    let nom = match dit(ent.nom_affichage.as_deref()).or_else(|| dit(ent.raison_sociale.as_deref())) {
        Some(n) => n,
        None => nom_dossier(racine)?,
    };
    Ok((nom, lieu(&ent)))
}

fn titre(s: &str) -> String {
    let mut sortie = String::new();
    let mut debut = true;
    for c in s.chars() {
        if c.is_alphabetic() {
            if debut {
                sortie.extend(c.to_uppercase());
            } else {
                sortie.extend(c.to_lowercase());
            }
            debut = false;
        } else {
            sortie.push(c);
            debut = true;
        }
    }
    sortie
}

/// Le lieu de la tuile : la commune (`commune_affichage`, sinon la commune declaree, en titre) et la convention.
fn lieu(ent: &droit::fiche::ChampsEntreprise) -> String {
    let commune = dit(ent.commune_affichage.as_deref()).or_else(|| dit(ent.commune.as_deref()).map(|c| titre(&c)));
    let idcc = dit(ent.idcc.as_deref()).map(|i| format!("IDCC {}", i));
    [commune, idcc].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

fn couleur_valide(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())
}

/// `config/affichage.json` : la couleur que la compagnie a choisie (`poser_couleur`).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Affichage {
    couleur: String,
}

/// La couleur d'une compagnie (`config/affichage.json`) ; celle de la charte tant qu'elle n'en a pas choisi.
pub fn couleur(racine: &Path) -> R<String> {
    let p = racine.join(AFFICHAGE);
    match lire_s_il_est_la::<Affichage>(&p, "La couleur de la compagnie (config/affichage.json) ne se lit pas.")? {
        Some(a) if couleur_valide(&a.couleur) => Ok(a.couleur),
        Some(a) => Err(socle::echec::abime(&p, format!("la couleur « {} » n'a pas la forme #RRGGBB", a.couleur))),
        None => Ok(COULEUR_DE_LA_CHARTE.to_string()),
    }
}

/// Le logo de la compagnie, s'il est pose (svg d'abord).
pub fn logo(racine: &Path) -> Option<PathBuf> {
    ["logo.svg", "logo.png"].iter().map(|n| racine.join("gabarits").join("identite").join(n)).find(|p| p.is_file())
}

#[derive(Clone, Debug)]
pub struct Compagnie {
    pub id: String,
    pub nom: String,
    pub lieu: String,
    pub racine: PathBuf,
    pub reelle: bool,
    pub couleur: String,
}

/// La nature d'une fictive que son nom porte parfois, entre parentheses.
const NATURES_DITES: [&str; 3] = [" (fictive)", " (démonstration)", " (demonstration)"];

/// La tuile de la compagnie `racine` (`nom_et_lieu`).
pub fn entree(id: String, racine: &Path, reelle: bool) -> R<Compagnie> {
    let (mut n, lieu) = nom_et_lieu(racine)?;
    if !reelle {
        // Le nom d'une fictive porte parfois sa nature, entre parentheses : la tuile la dit deja.
        for suffixe in NATURES_DITES {
            n = n.replace(suffixe, "");
        }
    }
    Ok(Compagnie { id, nom: n, lieu, racine: racine.to_path_buf(), reelle, couleur: couleur(racine)? })
}

/// Un dossier dont le nom commence par un point n'est jamais une compagnie : la photo d'origine des fictives
/// (`.origine/`), le reste d'une compagnie supprimee qui ne s'est pas efface (`suppression::effacer`).
pub fn est_cache(p: &Path) -> bool {
    p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'))
}

/// Les compagnies sous `dossier` (celles qui ont leur fiche entreprise), par ordre de dossier ; aucune s'il n'existe pas.
fn sous_dossiers(dossier: &Path) -> R<Vec<PathBuf>> {
    let entrees = match std::fs::read_dir(dossier) {
        Ok(it) => it,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(socle::echec::du_disque(dossier, &e)),
    };
    let mut v = Vec::new();
    for e in entrees {
        let d = e.map_err(|e| socle::echec::du_disque(dossier, &e))?.path();
        if !est_cache(&d) && d.join(droit::contexte::ENTREPRISE).is_file() {
            v.push(d);
        }
    }
    v.sort();
    Ok(v)
}

/// Le nom du dossier d'une compagnie.
pub fn nom_dossier(p: &Path) -> R<String> {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| {
        socle::echec::refus_avec("Ce n'est pas le dossier d'une compagnie.", socle::echec::Lieu { chemin: Some(p.to_string_lossy().into_owned()), ..Default::default() })
    })
}

/// Les compagnies de l'installation `b` (le dossier qui porte `logiciel/`, `prive/`, `compagnies/`, `fictives/`), dans
/// l'ordre des tuiles. L'administrateur (`administrateur.rs`) lit celles de l'installation qu'on lui donne.
pub fn liste_de(b: &Path) -> R<Vec<Compagnie>> {
    let mut out = Vec::new();
    if !crate::bridage::est_bride_de(b) {
        let reel = if b == base() { dossier_reel() } else { b.join("prive") };
        if reel.join(socle::fichiers::CONFIG).is_dir() {
            out.push(entree("reelle".into(), &reel, true)?);
        }
        for d in sous_dossiers(&b.join("compagnies"))? {
            out.push(entree(format!("cie:{}", nom_dossier(&d)?), &d, true)?);
        }
    }
    for d in sous_dossiers(&b.join(DOSSIER_FICTIVES))? {
        out.push(entree(format!("fictive:{}", nom_dossier(&d)?), &d, false)?);
    }
    Ok(out)
}

/// LES COMPAGNIES QUE LA FENETRE TIENT A JOUR EN FOND des son lancement (`Fenetre::prechauffer`) : les
/// vraies compagnies de l'installation `b` (pas les fictives), la derniere ouverte d'abord -- la date de son journal de
/// la fenetre (`journal/interface.log`), que chaque ouverture recommence ; jamais ouverte (sans journal), a la fin.
pub fn a_tenir_a_jour(b: &Path) -> R<Vec<PathBuf>> {
    let mut datees = Vec::new();
    for c in liste_de(b)?.into_iter().filter(|c| c.reelle) {
        let journal = c.racine.join(JOURNAL_INTERFACE);
        let date = match std::fs::metadata(&journal).and_then(|m| m.modified()) {
            Ok(d) => Some(d),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(socle::echec::du_disque(&journal, &e)),
        };
        datees.push((date, c.racine));
    }
    datees.sort_by_key(|(d, _)| std::cmp::Reverse(*d));
    Ok(datees.into_iter().map(|(_, r)| r).collect())
}

/// Le dossier d'une tuile -- et seulement d'une tuile : l'identifiant ne designe jamais un chemin qu'on n'a pas propose.
pub fn racine_choisie(inst: &Path, id: &str) -> R<PathBuf> {
    liste_de(inst)?
        .into_iter()
        .find(|c| c.id == id)
        .map(|c| c.racine)
        .ok_or_else(|| socle::refus!("Compagnie inconnue ou absente de cette version : {}", id))
}

pub fn poser_couleur(inst: &Path, id: &str, valeur: &str) -> R<String> {
    if !couleur_valide(valeur) {
        return Err(socle::refus!("Couleur attendue sous la forme #RRGGBB : '{}'", valeur));
    }
    let c = liste_de(inst)?.into_iter().find(|c| c.id == id && c.reelle).ok_or_else(|| socle::echec::refus("Seule une compagnie reelle a une couleur a choisir."))?;
    let v = valeur.to_uppercase();
    socle::ecriture::ecrire_json(&c.racine.join(AFFICHAGE), &Affichage { couleur: v.clone() }, 2)?;
    Ok(v)
}

pub fn nom_de_dossier(nom: &str) -> String {
    let mut s = String::new();
    for c in nom.to_lowercase().chars() {
        let pli = match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => "a",
            'é' | 'è' | 'ê' | 'ë' => "e",
            'î' | 'ï' | 'í' | 'ì' => "i",
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => "o",
            'ù' | 'û' | 'ü' | 'ú' => "u",
            'ç' => "c",
            'ñ' => "n",
            'ÿ' => "y",
            'œ' => "oe",
            'æ' => "ae",
            c if c.is_ascii_alphanumeric() => {
                s.push(c);
                continue;
            }
            c if !c.is_ascii() => "",
            _ => "-",
        };
        s.push_str(pli);
    }
    let mut net = String::new();
    for c in s.chars() {
        if c == '-' && (net.is_empty() || net.ends_with('-')) {
            continue;
        }
        net.push(c);
    }
    let net: String = net.trim_end_matches('-').chars().take(40).collect();
    if net.is_empty() {
        "compagnie".into()
    } else {
        net
    }
}

/// LE JOUR OU UNE COMPAGNIE A ETE AJOUTEE : `config/creation.json`, ecrit par `accueil::ajouter_venant` et nulle part
/// ailleurs ; il suit la compagnie quand elle change de dossier. Une compagnie d'avant (une fictive livree, la vraie
/// compagnie d'une installation) n'en a pas.
pub const FICHIER_CREATION: &str = "creation.json";

/// `config/creation.json` : le jour ou la compagnie a ete ajoutee, et par quel chemin du « + ».
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Creation {
    pub le: socle::date::Jour,
    pub venue: Venue,
    pub quoi: String,
}

fn creation(racine: &Path) -> R<Option<Creation>> {
    lire_s_il_est_la(&racine.join(socle::fichiers::CONFIG).join(FICHIER_CREATION), "Le jour d'ajout de la compagnie (config/creation.json) ne se lit pas.")
}

/// Le jour ou la compagnie a ete ajoutee ; `None` pour une compagnie d'avant.
pub fn cree_le(racine: &Path) -> R<Option<socle::date::Jour>> {
    Ok(creation(racine)?.map(|c| c.le))
}

/// D'OU VIENT UNE COMPAGNIE AJOUTEE : le « + » propose les deux chemins. Une compagnie neuve commence ses paies ici ;
/// une compagnie qui vient d'un logiciel precedent commence par reprendre son passe (`import`), et la page s'ouvre sur
/// l'import tant qu'il n'a rien repris (`routes::ouverture::etat`, `commencer_par_l_import`). `creation.json` le garde
/// (`venue`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Venue {
    Neuve,
    LogicielPrecedent,
}

impl Venue {
    /// Le mot du geste de l'accueil et de `creation.json`.
    pub fn mot(self) -> &'static str {
        match self {
            Venue::Neuve => "neuve",
            Venue::LogicielPrecedent => "logiciel_precedent",
        }
    }
}

/// Le chemin du « + » par lequel la compagnie est venue ; `None` pour une compagnie d'avant.
pub fn venue(racine: &Path) -> R<Option<Venue>> {
    Ok(creation(racine)?.map(|c| c.venue))
}

fn origine(racine: &Path) -> R<PathBuf> {
    Ok(dessus(racine).join(DOSSIER_ORIGINE).join(nom_dossier(racine)?))
}

fn copier_arbre(de: &Path, vers: &Path, rel: &Path, sauf: &[&str]) -> std::io::Result<()> {
    std::fs::create_dir_all(vers.join(rel))?;
    for e in std::fs::read_dir(de.join(rel))? {
        let e = e?;
        let r = rel.join(e.file_name());
        let rs = r.to_string_lossy().replace('\\', "/");
        if sauf.contains(&rs.as_str()) || e.file_name() == "__pycache__" {
            continue;
        }
        if e.file_type()?.is_dir() {
            copier_arbre(de, vers, &r, sauf)?;
        } else {
            std::fs::copy(e.path(), vers.join(&r))?;
        }
    }
    Ok(())
}

/// Le dossier `d` retire, s'il est la.
fn retirer_dossier(d: &Path) -> R<()> {
    match std::fs::remove_dir_all(d) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(socle::echec::du_disque(d, &e)),
    }
}

/// La photo d'origine d'une fictive, prise avant le premier geste (un dossier `.origine/<nom>/`).
pub fn figer_origine(racine: &Path) -> R<()> {
    if !est_fictive_livree(racine)? {
        return Ok(());
    }
    let o = origine(racine)?;
    if o.is_dir() {
        return Ok(());
    }
    let provisoire = o.with_extension("part");
    retirer_dossier(&provisoire)?;
    copier_arbre(racine, &provisoire, Path::new(""), &PROPRES_AU_POSTE).map_err(|e| socle::echec::du_disque(&provisoire, &e))?;
    std::fs::rename(&provisoire, &o).map_err(|e| socle::echec::du_disque(&o, &e))
}

/// La fictive revient a sa photo d'origine ; ce qui appartient au poste est garde. Rend (nom, deja).
pub fn remettre_a_zero(inst: &Path, id: &str) -> R<(String, bool)> {
    let mut trouvee = None;
    for c in liste_de(inst)? {
        if c.id == id && !c.reelle && est_fictive_livree(&c.racine)? {
            trouvee = Some(c);
        }
    }
    let c = trouvee.ok_or_else(|| socle::echec::refus("Seule une compagnie fictive livrée se remet à zéro."))?;
    let o = origine(&c.racine)?;
    if !o.is_dir() {
        return Ok((c.nom, true));
    }
    let parent = dessus(&c.racine);
    let n = nom_dossier(&c.racine)?;
    let neuve = parent.join(format!(".{}.neuve", n));
    let ancienne = parent.join(format!(".{}.ancienne", n));
    for d in [&neuve, &ancienne] {
        retirer_dossier(d)?;
    }
    copier_arbre(&o, &neuve, Path::new(""), &[]).map_err(|e| socle::echec::du_disque(&neuve, &e))?;
    for rel in PROPRES_AU_POSTE {
        let source = c.racine.join(rel);
        let octets = match std::fs::read(&source) {
            Ok(o) => o,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(socle::echec::du_disque(&source, &e)),
        };
        let cible = neuve.join(rel);
        let d = dessus(&cible);
        std::fs::create_dir_all(&d).map_err(|e| socle::echec::du_disque(&d, &e))?;
        std::fs::write(&cible, octets).map_err(|e| socle::echec::du_disque(&cible, &e))?;
    }
    std::fs::rename(&c.racine, &ancienne).map_err(|e| socle::echec::du_disque(&c.racine, &e))?;
    std::fs::rename(&neuve, &c.racine).map_err(|e| socle::echec::du_disque(&neuve, &e))?;
    retirer_dossier(&ancienne)?;
    Ok((c.nom, false))
}

/// L'etiquette du bandeau d'une compagnie qui n'est pas reelle (deja en HTML).
pub fn texte_copie(racine: &Path) -> R<String> {
    if est_ajoutee(racine)? {
        return Ok("compagnie fictive &mdash; version d&eacute;mo, rien ne part".into());
    }
    if est_fictive(racine)? {
        return Ok("compagnie fictive &mdash; les envois sont simul&eacute;s".into());
    }
    Ok("copie de test &mdash; rien ne part en r&eacute;el".into())
}
