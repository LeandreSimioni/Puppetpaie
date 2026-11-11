//! UN LOGICIEL = UN ADMINISTRATEUR. Une installation de PuppetPaie
//! (le dossier qui porte `logiciel/`, et a cote `prive/`, `compagnies/*`, `fictives/*` : `compagnie/installation.rs`)
//! appartient a une personne qui gere une ou plusieurs compagnies ; qui veut deux administrateurs installe un second
//! logiciel. Les compagnies fictives restent hors de tout cela.
//!
//! CE QUI EST SCELLE PAR QUOI. Rien de neuf : chaque compagnie garde SON coffre (`config/coffre.bin`, `coffre.rs`), sa
//! cle maitresse scellee deux fois, par la cle du poste et par le mot de passe du logiciel. RATTACHER une compagnie, c'est
//! sceller son enveloppe « mot de passe » par le mot de passe de l'administrateur (`Coffre::preparer_ouverture`) ; le
//! detacher, la resceller par un mot de passe a elle. Chaque coffre reste propre a sa compagnie : sa cle maitresse ne
//! sert qu'a lui ; seul le mot de passe qui en ouvre l'enveloppe est le meme.
//!
//! LE REGISTRE : `administrateur.json`, a la racine de l'installation (a cote de `compagnies/` : il voyage avec elle, une
//! cle USB porte tout), en sa forme (`Registre`). AUCUN SECRET : la liste des dossiers rattaches, relatifs a
//! l'installation, et depuis quand. Il dit quelles compagnies s'ouvrent ensemble ; ce qui le prouve, c'est que le mot de
//! passe ouvre chacun des coffres (`verifier_ouverture`), verifie a chaque geste.
//!
//! LES GESTES. `rattacher` (le mot de passe de l'administrateur, verifie sur une compagnie deja rattachee -- ou, quand il n'y
//! a pas encore d'administrateur, sur les compagnies deja la : toutes celles qu'il ouvre deviennent les siennes),
//! `detacher` (un mot de passe propre, tape deux fois), `changer` (le mot de passe de l'administrateur rescelle toutes ses
//! compagnies : tout se prepare, puis tout se remplace ; un echec au milieu remet en etat, aucune ne change). Rien de cela
//! n'est un acte qui engage : un acte reel (depot, envoi, signature) redemande toujours le mot de passe au moment de l'acte.
//!
//! LE TABLEAU DE BORD : pour chaque compagnie rattachee, les declarations obligatoires, triees par
//! urgence avec leur date limite -- la DSN mensuelle, les signalements (arret, reprise anticipee, fin de contrat) et ce que
//! `obligations.rs` fait naitre, les DPAE. Tout vient des RAPPELS de la compagnie (`routes::rappels`, route `rappels` :
//! les mois a declarer et leur echeance, `obligations::de_l_arret` et `de_la_fin` pour les delais et leur etat) : ce module
//! n'en recalcule aucun. Il y ajoute, pour ce qui part par la DSN, ou en est le fichier ecrit (a tester, en test, teste,
//! refuse) : la meme lecture du journal que l'etape de depot du mois (`etats::repetition`, `etats::deja_depose`). Le
//! tableau d'une compagnie se garde dans `config/.tableau.json` (`TableauCompagnie`) : l'accueil le lit tel quel, sans
//! calcul (`tableau_garde`) ; il se refait par la route `administrateur-tableau`, que la fenetre lance en fond.
//!
//! LE DEPOT GROUPE (`deposer`, route `administrateur-deposer`) : ce qui part par la DSN et qui est PRET (le moteur le
//! decide ici : en test, un fichier ecrit qui n'a pas encore ete essaye ; en reel, un fichier dont le test est revenu
//! conforme) se depose compagnie par compagnie, par la route `deposer` de chacune -- la meme que la page d'une compagnie,
//! avec tous ses controles et son bilan. Une compagnie qui refuse n'arrete pas les autres. En reel, UN mot de passe tape
//! au moment de l'acte (dans la charge, par la fenetre ; sinon le moteur le demande lui-meme a un humain,
//! `demande::mot_de_passe_de_l_acte`) sert de preuve a chaque depot, qui le reverifie sur son propre coffre.
//! Ce qui est hors DSN (la declaration d'accident, l'attestation de salaire) s'affiche avec son echeance et la marche a
//! suivre : rien ne le depose.

use socle::date::Jour;
use socle::echec::Classe;
use crate::enveloppe::Options;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use compagnie::coffre::Coffre;
use compagnie::installation::{self as compagnies, Compagnie};
use crate::catalogue::Route;
use crate::depots::{ChargeDeposer, Designation, Quoi};
use crate::enveloppe::Dite;
use crate::rappels::EcheanceDite;
use droit::contexte::Contexte;
use envois::etats;
use socle::echec::erreur;
use socle::echec::Echec;
use socle::echec::R;
use socle::mois::Mois;

/// Le registre de l'administrateur, a la racine de l'installation.
pub const FICHIER: &str = "administrateur.json";
/// Le dernier tableau connu d'une compagnie (hors de l'empreinte des etats des mois : `etat_garde::HORS_DE_L_ECRAN`).
pub const TABLEAU: &str = "config/.tableau.json";

/// Ce que le registre dit de lui-meme, a qui l'ouvre.
const QUOI_DU_REGISTRE: &str = "Les compagnies que gere l'administrateur de ce logiciel : un seul mot de passe les ouvre toutes. Aucun secret ici.";

// ------------------------------------------------------------------ le poste

/// Ou est la cle du poste, et quelle heure il est : ce qu'il faut pour monter le coffre d'une compagnie.
#[derive(Clone, Debug)]
pub struct Poste {
    pub dossier_cle: PathBuf,
    /// `AAAA-MM-JJTHH:MM` : le coffre note quand un mot de passe est pose.
    pub maintenant: String,
}

impl Poste {
    /// Ce poste-ci : la cle la ou `Coffre::du_poste` la trouve, l'heure de Paris.
    pub fn de_ce_poste() -> Poste {
        Poste { dossier_cle: Coffre::dossier_cle_du_poste(), maintenant: maintenant_minutes() }
    }

    pub fn coffre(&self, racine: &Path) -> Coffre {
        Coffre::sur(racine, &self.dossier_cle, &self.maintenant)
    }
}

/// L'heure de Paris, a la minute : ce que le coffre note comme date de pose.
pub fn maintenant_minutes() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_minutes(t)
}

// ------------------------------------------------------------------ le registre

/// LE REGISTRE TEL QU'IL S'ECRIT (`administrateur.json`) : ce qu'il est, et les compagnies rattachees.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registre {
    quoi: String,
    compagnies: Vec<EntreeRegistre>,
}

/// Une compagnie rattachee : son dossier, relatif a l'installation, et depuis quand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntreeRegistre {
    dossier: String,
    depuis: String,
}

/// L'installation d'une compagnie : `<installation>/compagnies/<nom>` ou `<installation>/prive` (ou `fictives/<nom>`).
pub(crate) fn installation_de(racine: &Path) -> PathBuf {
    let parent = match racine.parent() {
        Some(p) => p,
        None => racine,
    };
    match parent.parent() {
        Some(haut) if parent.file_name().is_some_and(|n| n == "compagnies" || n == compagnies::DOSSIER_FICTIVES) => haut.to_path_buf(),
        Some(_) | None => parent.to_path_buf(),
    }
}

/// Le chemin absolu, liens suivis. UN DOSSIER QUI N'EXISTE PLUS (une compagnie effacee que le registre nomme encore) n'a
/// pas de liens a suivre : il se compare par son chemin absolu tel qu'il s'ecrit, ce que le systeme en dit sans le
/// toucher (`std::path::absolute` : « Makes the path absolute without accessing the filesystem »).
fn absolu(p: &Path) -> R<PathBuf> {
    let c = match std::fs::canonicalize(p) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => std::path::absolute(p).map_err(|e| socle::echec::du_disque(p, &e))?,
        Err(e) => return Err(socle::echec::du_disque(p, &e)),
    };
    Ok(match c.to_string_lossy().strip_prefix(r"\\?\") {
        Some(s) => PathBuf::from(s),
        None => c.clone(),
    })
}

/// Le dossier d'une compagnie, relatif a l'installation, en barres droites (`compagnies/les-pupazzi`, `prive`) ; `None`
/// pour un dossier hors de l'installation.
fn relatif(inst: &Path, racine: &Path) -> R<Option<String>> {
    let r = absolu(racine)?;
    let i = absolu(inst)?;
    if !r.starts_with(&i) {
        return Ok(None);
    }
    let rel: PathBuf = r.components().skip(i.components().count()).collect();
    let t = rel.to_string_lossy().replace('\\', "/");
    Ok((!t.is_empty()).then_some(t))
}

/// Les compagnies du registre : aucune sans registre ; un registre qui ne se lit pas se refuse.
fn lire_registre(inst: &Path) -> R<Vec<EntreeRegistre>> {
    let p = inst.join(FICHIER);
    if !p.is_file() {
        return Ok(Vec::new());
    }
    let r: Registre = socle::forme::lire_fichier(&p, "Le registre de l'administrateur ne se lit pas.")?;
    Ok(r.compagnies)
}

fn ecrire_registre(inst: &Path, entrees: &[EntreeRegistre]) -> R<()> {
    let p = inst.join(FICHIER);
    if entrees.is_empty() {
        return socle::ecriture::effacer(&p);
    }
    socle::ecriture::ecrire_json(&p, &Registre { quoi: QUOI_DU_REGISTRE.into(), compagnies: entrees.to_vec() }, 2)
}

/// Les compagnies rattachees a l'administrateur, telles qu'elles sont sur le disque (un dossier parti n'y est plus). Une
/// compagnie fictive n'y entre pas : `rattacher` la refuse.
pub fn rattachees(inst: &Path) -> R<Vec<PathBuf>> {
    Ok(lire_registre(inst)?.into_iter().map(|e| inst.join(e.dossier)).filter(|racine| racine.join(droit::contexte::ENTREPRISE).is_file()).collect())
}

/// UNE COMPAGNIE SUPPRIMEE (`accueil::suppression`) sort du registre : une compagnie creee plus tard sous
/// le meme dossier n'en herite pas. Rien a faire si elle n'y est pas.
pub fn oublier(inst: &Path, racine: &Path) -> R<()> {
    let Some(rel) = relatif(inst, racine)? else { return Ok(()) };
    let avant = lire_registre(inst)?;
    let apres: Vec<EntreeRegistre> = avant.iter().filter(|e| e.dossier != rel).cloned().collect();
    if apres.len() == avant.len() {
        return Ok(());
    }
    ecrire_registre(inst, &apres)
}

/// La compagnie `racine` est-elle rattachee a l'administrateur de son installation ?
pub fn est_rattachee(inst: &Path, racine: &Path) -> R<bool> {
    let Some(rel) = relatif(inst, racine)? else { return Ok(false) };
    for r in rattachees(inst)? {
        if relatif(inst, &r)?.as_deref() == Some(rel.as_str()) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `liste` sans la compagnie `racine`.
fn sans(liste: Vec<PathBuf>, inst: &Path, racine: &Path) -> R<Vec<PathBuf>> {
    let rel = relatif(inst, racine)?;
    let mut out = Vec::new();
    for r in liste {
        if relatif(inst, &r)? != rel {
            out.push(r);
        }
    }
    Ok(out)
}

/// Les compagnies reelles de l'installation (ni fictive, ni copie), avec leur fiche d'accueil.
fn reelles(inst: &Path) -> R<Vec<Compagnie>> {
    let mut v = Vec::new();
    for c in compagnies::liste_de(inst)? {
        if c.reelle && !compagnies::est_fictive(&c.racine)? {
            v.push(c);
        }
    }
    Ok(v)
}

/// La fiche d'accueil de la compagnie `racine`, si l'installation la liste.
fn fiche_de(inst: &Path, racine: &Path) -> R<Option<Compagnie>> {
    let rel = relatif(inst, racine)?;
    for c in compagnies::liste_de(inst)? {
        if relatif(inst, &c.racine)? == rel {
            return Ok(Some(c));
        }
    }
    Ok(None)
}

/// Le nom de la compagnie `racine` : celui de sa fiche d'accueil, sinon celui de son dossier.
fn nom_de(inst: &Path, racine: &Path) -> R<String> {
    Ok(match (fiche_de(inst, racine)?, racine.file_name()) {
        (Some(c), _) => c.nom,
        (None, Some(n)) => n.to_string_lossy().into_owned(),
        (None, None) => racine.to_string_lossy().into_owned(),
    })
}

/// Les noms des compagnies `racines`.
fn noms_de(inst: &Path, racines: &[PathBuf]) -> R<Vec<String>> {
    racines.iter().map(|r| nom_de(inst, r)).collect()
}

fn en_liste(noms: &[String]) -> String {
    match noms {
        [] => String::new(),
        [un] => un.clone(),
        [tete @ .., dernier] => format!("{} et {}", tete.join(", "), dernier),
    }
}

/// Le coffre de la compagnie `racine` exige-t-il un mot de passe ?
fn exigee(poste: &Poste, racine: &Path) -> R<bool> {
    poste.coffre(racine).ouverture_exigee()
}

/// Le mot de passe ouvre-t-il le coffre de la compagnie `racine` ?
fn ouvre(poste: &Poste, racine: &Path, motdepasse: &str) -> R<bool> {
    if motdepasse.is_empty() {
        return Ok(false);
    }
    poste.coffre(racine).verifier_ouverture(motdepasse)
}

/// Le mot de passe ouvre-t-il le coffre de cette compagnie ?
pub fn mot_de_passe_ouvre(poste: &Poste, racine: &Path, motdepasse: &str) -> R<bool> {
    ouvre(poste, racine, motdepasse)
}

/// Une des compagnies `racines` s'ouvre-t-elle par `motdepasse` ?
fn ouvre_une(poste: &Poste, racines: &[PathBuf], motdepasse: &str) -> R<bool> {
    for r in racines {
        if ouvre(poste, r, motdepasse)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// L'administrateur de l'installation, tel que l'ecran le dit : les compagnies qu'il gere, et ce que la compagnie
/// `racine` est pour lui. `question` : les compagnies dont le mot de passe ouvrirait aussi `racine`, quand il y a lieu de
/// demander « meme administrateur ? » (une compagnie reelle sans mot de passe, et au moins une autre qui en a un).
pub(crate) fn etat(inst: &Path, racine: &Path, poste: &Poste) -> R<EtatAdministrateur> {
    let groupe = rattachees(inst)?;
    let compagnies = noms_de(inst, &groupe)?;
    let fictive = compagnies::est_fictive(racine)?;
    let rattachee = !fictive && est_rattachee(inst, racine)?;
    // Les autres compagnies que le mot de passe de l'administrateur ouvre : celles du registre, sinon celles qui ont deja
    // un mot de passe (il n'y a pas encore d'administrateur : l'une d'elles le devient).
    let autres: Vec<String> = if !groupe.is_empty() {
        noms_de(inst, &sans(groupe.clone(), inst, racine)?)?
    } else {
        let ici = relatif(inst, racine)?;
        let mut a = Vec::new();
        for c in reelles(inst)? {
            if relatif(inst, &c.racine)? != ici && exigee(poste, &c.racine)? {
                a.push(c.nom);
            }
        }
        a
    };
    // Les autres, en liste lisible (« A, B et C »), et ce que le titre du mot de passe en dit.
    let autres_dit = en_liste(&autres);
    let bridee = compagnie::bridage::est_bride_de(inst);
    let pose = exigee(poste, racine)?;
    Ok(EtatAdministrateur {
        administrateur: !groupe.is_empty(),
        compagnies,
        rattachee,
        fictive,
        ouvre_aussi_dit: (!autres_dit.is_empty()).then(|| format!("il ouvre aussi {}", autres_dit)),
        peut_rattacher: !fictive && !rattachee && !autres.is_empty() && !bridee,
        question: !fictive && !rattachee && !autres.is_empty() && !pose && !bridee,
        mot_de_passe_pose: pose,
        // La pastille de la compagnie pour l'administrateur : rattachee, faite ; son propre mot de passe, une information.
        pastille: if rattachee { socle::ecran::Pastille::de("fait", "rattachée") } else { socle::ecran::Pastille::de("information", "son propre mot de passe") },
        autres,
        autres_dit,
    })
}

/// Ce que l'administrateur est pour une compagnie (`etat`), ce que l'ecran des mots de passe et la page du mot de passe
/// lisent.
#[derive(Debug, serde::Serialize)]
pub struct EtatAdministrateur {
    /// Les compagnies qu'il gere.
    pub(crate) compagnies: Vec<String>,
    /// Vrai : l'installation a un administrateur.
    pub(crate) administrateur: bool,
    pub(crate) rattachee: bool,
    pub(crate) fictive: bool,
    /// Les autres compagnies que son mot de passe ouvre, et en liste lisible.
    pub(crate) autres: Vec<String>,
    pub(crate) autres_dit: String,
    pub(crate) ouvre_aussi_dit: Option<String>,
    pub(crate) peut_rattacher: bool,
    pub(crate) question: bool,
    pub(crate) mot_de_passe_pose: bool,
    pub(crate) pastille: socle::ecran::Pastille,
}

/// Remplace `chemin` par `octets`, d'un coup (a cote puis a la place) ; 0600 sous Unix.
fn remplacer(chemin: &Path, octets: &[u8]) -> R<()> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).map_err(|e| socle::echec::du_disque(&d, &e))?;
    }
    let tmp = chemin.with_extension("bin.administrateur");
    std::fs::write(&tmp, octets).map_err(|e| socle::echec::du_disque(&tmp, &e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).map_err(|e| socle::echec::du_disque(&tmp, &e))?;
    }
    std::fs::rename(&tmp, chemin).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        socle::echec::du_disque(&chemin, &e)
    })
}

/// Le fichier `chemin` tel qu'il est, pour le remettre (`None` : il n'existe pas).
fn tel_qu_il_est(chemin: &Path) -> R<Option<Vec<u8>>> {
    match std::fs::read(chemin) {
        Ok(o) => Ok(Some(o)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(socle::echec::du_disque(chemin, &e)),
    }
}

/// Remet un fichier dans l'etat d'avant (`None` : il n'existait pas). Ce qui ne se remet pas se dit.
fn remettre(chemin: &Path, avant: &Option<Vec<u8>>) -> R<()> {
    match avant {
        Some(o) => remplacer(chemin, o),
        None => socle::ecriture::effacer(chemin),
    }
}

/// Le refus `phrase`, et le refus de remise en etat qu'il a demandee, ajoute a sa phrase.
#[track_caller]
fn et_la_remise(phrase: String, remise: R<()>) -> Echec {
    match remise {
        Ok(()) => socle::echec::refus(phrase),
        Err(m) => socle::refus!("{} Et un fichier n'est pas revenu en arrière : {}", phrase, m.phrase),
    }
}

/// RATTACHER `racine` a l'administrateur : son coffre se rescelle par `admin` (le mot de passe de l'administrateur). `admin`
/// est verifie d'abord sur une compagnie deja rattachee ; sans administrateur encore, il doit ouvrir au moins une des
/// compagnies qui ont deja un mot de passe, et toutes celles qu'il ouvre deviennent les siennes. Une compagnie qui a deja
/// son mot de passe donne aussi `actuel` (c'est lui que le rattachement remplace). Un refus n'ecrit rien. Rend les noms des
/// compagnies de l'administrateur.
pub fn rattacher(inst: &Path, racine: &Path, admin: &str, actuel: Option<&str>, poste: &Poste) -> R<Vec<String>> {
    if compagnie::bridage::est_bride_de(inst) {
        return Err(socle::echec::refus("Le logiciel est bridé : il ne gère que des compagnies fictives."));
    }
    if compagnies::est_fictive(racine)? {
        return Err(socle::echec::refus("Une compagnie fictive s'ouvre sans mot de passe : elle n'a pas d'administrateur."));
    }
    let Some(rel) = relatif(inst, racine)? else {
        let a_part = HorsDeLInstallation { chemin: racine.to_string_lossy().into_owned(), installation: inst.to_string_lossy().into_owned() };
        return Err(socle::echec::refus_avec("Ce dossier n'est pas une compagnie de cette installation.", socle::echec::APart::forme(a_part)));
    };
    if est_rattachee(inst, racine)? {
        return Err(socle::echec::refus("Cette compagnie est déjà gérée par l'administrateur."));
    }
    if admin.is_empty() {
        return Err(socle::echec::refus("Tapez le mot de passe de l'administrateur. Rien n'a changé."));
    }
    let groupe = rattachees(inst)?;
    let mut nouvelles: Vec<PathBuf> = Vec::new();
    if groupe.is_empty() {
        let mut candidates: Vec<Compagnie> = Vec::new();
        for c in reelles(inst)? {
            if relatif(inst, &c.racine)?.as_deref() != Some(rel.as_str()) && exigee(poste, &c.racine)? {
                candidates.push(c);
            }
        }
        if candidates.is_empty() {
            return Err(socle::echec::refus("Aucune autre compagnie n'a encore de mot de passe : choisissez celui de cette compagnie."));
        }
        for c in &candidates {
            if ouvre(poste, &c.racine, admin)? {
                nouvelles.push(c.racine.clone());
            }
        }
        if nouvelles.is_empty() {
            let noms: Vec<String> = candidates.iter().map(|c| c.nom.clone()).collect();
            return Err(socle::refus!("Ce mot de passe n'ouvre pas {} : ce n'est pas celui de l'administrateur. Rien n'a changé.", en_liste(&noms)));
        }
    } else if !ouvre_une(poste, &groupe, admin)? {
        return Err(socle::echec::refus("Ce n'est pas le mot de passe de l'administrateur. Rien n'a changé."));
    }
    let coffre = poste.coffre(racine);
    let deja = coffre.ouverture_exigee()?;
    let actuel = actuel.filter(|a| !a.is_empty());
    if deja {
        match actuel {
            None => return Err(socle::echec::refus("Tapez aussi le mot de passe actuel de cette compagnie : c'est lui que celui de l'administrateur remplace. Rien n'a changé.")),
            Some(a) if !ouvre(poste, racine, a)? => return Err(socle::echec::refus("Le mot de passe actuel de cette compagnie est refusé. Rien n'a changé.")),
            Some(_) => {}
        }
    }
    let octets = coffre.preparer_ouverture(admin, actuel).map_err(|e| e.reformuler(|m| format!("{} Rien n'a changé.", m.trim_end_matches(" Rien n'a ete pose."))))?;
    let chemin = coffre.chemin();
    let avant = tel_qu_il_est(&chemin)?;
    remplacer(&chemin, &octets).map_err(|m| socle::refus!("Le coffre de cette compagnie ne s'écrit pas ({}). Rien n'a changé.", m.phrase))?;
    let mut entrees = lire_registre(inst)?;
    for r in nouvelles.iter().chain(std::iter::once(&racine.to_path_buf())) {
        if let Some(dossier) = relatif(inst, r)? {
            if !entrees.iter().any(|x| x.dossier == dossier) {
                entrees.push(EntreeRegistre { dossier, depuis: poste.maintenant.clone() });
            }
        }
    }
    if let Err(m) = ecrire_registre(inst, &entrees) {
        let phrase = format!("Le registre de l'administrateur ne s'écrit pas ({}). Rien n'a changé.", m.phrase);
        return Err(et_la_remise(phrase, remettre(&chemin, &avant)));
    }
    noms_de(inst, &rattachees(inst)?)
}

/// Ce que le refus d'un dossier hors de l'installation porte a part : le dossier, et l'installation.
#[derive(Serialize)]
struct HorsDeLInstallation {
    chemin: String,
    installation: String,
}

/// DETACHER `racine` : elle recoit son propre mot de passe (`nouveau`, tape deux fois), et celui de l'administrateur
/// (`admin`, verifie) ne l'ouvre plus. Un refus n'ecrit rien.
pub fn detacher(inst: &Path, racine: &Path, admin: &str, nouveau: &str, confirmation: &str, poste: &Poste) -> R<()> {
    if !est_rattachee(inst, racine)? {
        return Err(socle::echec::refus("Cette compagnie n'est pas gérée par l'administrateur : elle a déjà son propre mot de passe."));
    }
    if !ouvre(poste, racine, admin)? {
        return Err(socle::echec::refus("Ce n'est pas le mot de passe de l'administrateur. Rien n'a changé."));
    }
    if nouveau != confirmation {
        return Err(socle::echec::refus("Les deux saisies du nouveau mot de passe diffèrent. Rien n'a changé."));
    }
    if nouveau == admin {
        return Err(socle::echec::refus("Choisissez un mot de passe différent de celui de l'administrateur : c'est lui qui la sépare des autres. Rien n'a changé."));
    }
    let coffre = poste.coffre(racine);
    let octets = coffre.preparer_ouverture(nouveau, Some(admin)).map_err(|e| e.reformuler(|m| format!("{} Rien n'a changé.", m.trim_end_matches(" Rien n'a ete pose."))))?;
    let rel = relatif(inst, racine)?;
    let avant_registre = lire_registre(inst)?;
    let reste: Vec<EntreeRegistre> = avant_registre.iter().filter(|e| Some(&e.dossier) != rel.as_ref()).cloned().collect();
    ecrire_registre(inst, &reste).map_err(|m| socle::refus!("Le registre de l'administrateur ne s'écrit pas ({}). Rien n'a changé.", m.phrase))?;
    if let Err(m) = remplacer(&coffre.chemin(), &octets) {
        let phrase = format!("Le coffre de cette compagnie ne s'écrit pas ({}). Rien n'a changé.", m.phrase);
        return Err(et_la_remise(phrase, ecrire_registre(inst, &avant_registre)));
    }
    Ok(())
}

/// CHANGER le mot de passe de l'administrateur : toutes ses compagnies se rescellent d'un coup. `actuel` doit ouvrir
/// chacune ; tout se prepare d'abord (si une seule refuse, rien ne s'ecrit) ; puis chaque coffre est remplace, et si un
/// remplacement echoue, ceux deja faits reviennent a leur etat d'avant. Rend les noms des compagnies rescellees.
pub fn changer(inst: &Path, actuel: &str, nouveau: &str, confirmation: &str, poste: &Poste) -> R<Vec<String>> {
    let groupe = rattachees(inst)?;
    if groupe.is_empty() {
        return Err(socle::echec::refus("Ce logiciel n'a pas d'administrateur : chaque compagnie a son propre mot de passe."));
    }
    if nouveau != confirmation {
        return Err(socle::echec::refus("Les deux saisies du nouveau mot de passe diffèrent. Rien n'a changé."));
    }
    let mut prets: Vec<(PathBuf, Vec<u8>, Option<Vec<u8>>)> = Vec::new();
    for r in &groupe {
        let nom = nom_de(inst, r)?;
        if !ouvre(poste, r, actuel)? {
            return Err(socle::refus!("Le mot de passe actuel n'ouvre pas {} : rien n'a changé, dans aucune compagnie.", nom));
        }
        let coffre = poste.coffre(r);
        let octets = coffre
            .preparer_ouverture(nouveau, Some(actuel))
            .map_err(|e| e.reformuler(|m| format!("{} : {} Rien n'a changé, dans aucune compagnie.", nom, m.trim_end_matches(" Rien n'a ete pose."))))?;
        let chemin = coffre.chemin();
        let avant = tel_qu_il_est(&chemin)?;
        prets.push((chemin, octets, avant));
    }
    for (i, (chemin, octets, _)) in prets.iter().enumerate() {
        if let Err(m) = remplacer(chemin, octets) {
            let phrase = format!("Un coffre ne s'écrit pas ({}) : tous sont revenus à l'ancien mot de passe.", m.phrase);
            let remise = prets[..i].iter().map(|(c, _, avant)| remettre(c, avant)).collect::<R<Vec<()>>>().map(|_| ());
            return Err(et_la_remise(phrase, remise));
        }
    }
    noms_de(inst, &groupe)
}

// ------------------------------------------------------------------ le tableau d'une compagnie

/// CE QU'EST UNE DECLARATION DU TABLEAU : la DSN d'un mois, un signalement, une declaration hors DSN (a faire sur le
/// service de l'organisme), une DPAE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Genre {
    Dsn,
    Signalement,
    HorsDsn,
    Dpae,
}

/// OU EN EST UN FICHIER DSN ECRIT (mensuelle ou signalement), lu du journal : a tester (ecrit, pas encore essaye), en
/// test (parti, bilan attendu), teste (conforme), test refuse, ou la paie changee depuis l'ecriture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Etape {
    ATester,
    EnTest,
    Testee,
    TestRefuse,
    PaieChangee,
}

impl Etape {
    /// Ce qu'on dit d'un fichier a cette etape.
    fn marche(self) -> &'static str {
        match self {
            Etape::ATester => "Écrite : prête pour le dépôt de test.",
            Etape::EnTest => "Partie en test, compte rendu attendu : le dépôt réel se fait une fois le test revenu conforme.",
            Etape::Testee => "Testée, conforme : prête à envoyer pour de vrai.",
            Etape::TestRefuse => "Le test a été refusé : à corriger dans la compagnie, puis à retester.",
            Etape::PaieChangee => "La paie a changé depuis l'écriture : réécrire le mois, le tester, puis l'envoyer.",
        }
    }
}

/// Ce qui est pret a partir : en test (ecrit, pas encore essaye), en reel (teste, conforme, et le fichier est reel).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pret {
    pub test: bool,
    pub reel: bool,
}

/// UNE DECLARATION DU TABLEAU D'UNE COMPAGNIE, telle qu'elle se garde (`TableauCompagnie`) : sa cle (celle du rappel),
/// ce qu'elle est, son titre, son mois (une DSN), sa date limite et comment la dire, son detail, la marche a suivre, les
/// lieux ou la faire (hors DSN), le fichier ecrit et ou il en est, et ce qui est pret a partir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub cle: String,
    pub en_retard: bool,
    pub genre: Genre,
    pub titre: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mois: Option<Mois>,
    pub butoir: Option<Jour>,
    /// Quand, en une phrase (« le 05/12/2026 à 12:00 »).
    #[serde(rename = "quand")]
    pub quand_dit: Option<String>,
    pub detail: String,
    pub marche: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ou: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fichier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etape: Option<Etape>,
    pub pret: Pret,
}

/// LE TABLEAU D'UNE COMPAGNIE tel qu'il se garde (`config/.tableau.json`) : son jour, ses declarations, l'empreinte de la
/// compagnie qu'il a lue (`etat_garde::empreinte_de`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableauCompagnie {
    pub le: Jour,
    pub declarations: Vec<Declaration>,
    pub empreinte: String,
}

/// Ou en est un fichier DSN ecrit (mensuelle ou signalement), lu du journal : `None` quand un depot reel accepte de ce
/// contenu est note. Les memes lectures que l'etape de depot du mois (`crate::etapes`, la route `etat-du-mois`).
fn etape_du_fichier(ctx: &Contexte, lignes: &[envois::journal::Ligne], chemin: &Path, mensuelle: Option<Mois>) -> R<Option<Etape>> {
    let octets = std::fs::read(chemin).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque))?;
    let empreinte = envois::journal::empreinte(&octets);
    if etats::deja_depose(lignes, envois::journal::Acte::Dsn, &empreinte).is_some() {
        return Ok(None);
    }
    if let Some(m) = mensuelle {
        if crate::etapes::Moteur::nouveau(ctx).paie_changee_depuis_l_ecriture(m, chemin)?.is_some() {
            return Ok(Some(Etape::PaieChangee));
        }
    }
    let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : un fichier écrit sans nom."))?;
    let r = etats::repetition(lignes, envois::journal::Acte::Dsn, &empreinte, &nom);
    Ok(Some(match r.etat {
        etats::EtatDuTest::Ecrite => Etape::ATester,
        etats::EtatDuTest::EnTest => Etape::EnTest,
        etats::EtatDuTest::Testee => Etape::Testee,
        etats::EtatDuTest::TestRefuse => Etape::TestRefuse,
    }))
}

/// Le fichier d'un signalement ecrit, s'il y en a un : `sorties/<mois>/<nom>` (le nom compare sans casse).
fn fichier_ecrit(racine: &Path, mois: socle::mois::Mois, nom: &str) -> R<Option<PathBuf>> {
    let dossier = racine.join("sorties").join(mois.to_string());
    Ok(fichiers_de(&dossier)?.into_iter().find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(nom))))
}

/// Les fichiers du dossier `dossier` : aucun s'il n'existe pas.
fn fichiers_de(dossier: &Path) -> R<Vec<PathBuf>> {
    let entrees = match std::fs::read_dir(dossier) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(socle::echec::du_disque(dossier, &e).reclasser(Classe::du_disque(&e))),
    };
    let mut out = Vec::new();
    for e in entrees {
        let p = e.map_err(|e| socle::echec::du_disque(dossier, &e).reclasser(Classe::du_disque(&e)))?.path();
        if p.is_file() {
            out.push(p);
        }
    }
    Ok(out)
}

/// Le fichier que l'ecriture d'un signalement pose pour cette obligation (`routes::imprevus` : `arret-signaler`,
/// `fin-contrat-signaler`), d'apres la cle du rappel (`<obligation>:<salarie>:<jour>`, `fin-contrat:<salarie>:<fin>`).
fn fichier_du_signalement(ctx: &Contexte, cle: &str) -> R<Option<PathBuf>> {
    let mut morceaux = cle.splitn(3, ':');
    let (Some(genre), Some(salarie), Some(jour)) = (morceaux.next(), morceaux.next(), morceaux.next()) else { return Ok(None) };
    let Some(mois) = jour.get(..7).and_then(socle::mois::Mois::du_texte) else { return Ok(None) };
    let compact = jour.replace('-', "");
    let nom = match genre {
        "signalement_arret" => format!("SignalementArret_{}_{}.dsn", salarie, compact),
        // L'annule-et-remplace s'ecrit a cote du signalement parti, sous `_AR<ordre>` : le dernier ecrit.
        "signalement_arret_rectificatif" => {
            let tete = format!("signalementarret_{}_{}_ar", salarie, compact).to_lowercase();
            let rang = |p: &PathBuf| -> Option<i64> {
                let n = p.file_name()?.to_string_lossy().to_lowercase();
                // Un nom qui ne finit pas par un rang n'est pas un annule-et-remplace.
                n.strip_prefix(&tete)?.strip_suffix(".dsn")?.parse::<i64>().into_iter().next()
            };
            let dossier = ctx.racine.join("sorties").join(mois.to_string());
            return Ok(fichiers_de(&dossier)?.into_iter().filter_map(|p| rang(&p).map(|r| (r, p))).max_by_key(|(r, _)| *r).map(|(_, p)| p));
        }
        "signalement_reprise" => format!("SignalementReprise_{}_{}.dsn", salarie, compact),
        "fin-contrat" => {
            let cle = ctx.cle_du_salarie(salarie)?;
            format!("SignalementFinContrat_{}_{}.dsn", cle, compact)
        }
        _ => return Ok(None),
    };
    fichier_ecrit(&ctx.racine, mois, &nom)
}

/// Le defaut d'un rappel qui ne porte pas ce que son genre exige.
fn rappel_incomplet(quoi: &str) -> Echec {
    erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : un rappel {} ne porte pas ce qu'il faut pour le tableau.", quoi))
}

/// Une declaration du tableau, a partir d'un rappel de la compagnie ; `None` pour un rappel qui n'en est pas une, ou une
/// declaration deja deposee.
fn declaration(ctx: &Contexte, lignes: &[envois::journal::Ligne], r: &EcheanceDite) -> R<Option<Declaration>> {
    let mut fichier: Option<PathBuf> = None;
    let mut mensuelle: Option<Mois> = None;
    let mut d = match r.quoi.as_str() {
        "dsn" => {
            let mois = r.mois.ok_or_else(|| rappel_incomplet("de DSN"))?;
            let e = r.echeance_depot.as_ref().ok_or_else(|| rappel_incomplet("de DSN"))?;
            let marche = match r.etape.as_str() {
                "a_saisir" => Some("Saisir le mois dans la compagnie (vide s'il n'y a pas eu de paie : une DSN « néant » se dépose aussi)."),
                "a_produire" => Some("Écrire le mois dans la compagnie : la DSN s'écrit avec les bulletins, puis se teste."),
                "a_corriger" => Some("Corriger dans la compagnie, réécrire le mois, puis retester."),
                _ => None,
            };
            if r.etape == "a_deposer" {
                fichier = r.fichier.as_ref().map(PathBuf::from);
                mensuelle = Some(mois);
            }
            Declaration {
                cle: r.cle.clone(),
                en_retard: r.en_retard,
                genre: Genre::Dsn,
                titre: format!("DSN de {}", mois.libelle(socle::mois::Graphie::Accentuee)),
                mois: Some(mois),
                butoir: Some(e.jour),
                quand_dit: Some(e.phrase.clone()),
                detail: r.detail.clone(),
                marche: marche.map(String::from),
                ou: None,
                fichier: None,
                etape: None,
                pret: Pret::default(),
            }
        }
        "obligation" => {
            let ob = r.obligation.as_ref().ok_or_else(|| rappel_incomplet("d'obligation"))?;
            let quand_dit = ob.butoir.map(|b| match ob.butoir_heure.as_deref() {
                Some("24:00") => format!("le {}, jusqu'à minuit", b.jj_mm_aaaa()),
                Some(h) => format!("le {} à {}", b.jj_mm_aaaa(), h),
                None => format!("le {}", b.jj_mm_aaaa()),
            });
            let detail = if ob.phrase.is_empty() { r.detail.clone() } else { ob.phrase.clone() };
            let (genre, marche, ou) = if ob.cle.nom().starts_with("signalement_") {
                fichier = fichier_du_signalement(ctx, &r.cle)?;
                let marche = if r.a_saisir.is_empty() {
                    "Écrire le signalement dans la compagnie (Imprévus, ou la colonne du mois), puis le tester."
                } else {
                    "Compléter le contrat dans la compagnie, puis écrire le signalement."
                };
                (Genre::Signalement, marche, None)
            } else {
                // HORS DSN : la declaration d'accident, l'attestation de salaire. Le logiciel ne la fait pas : ou la faire.
                let lieux: Option<Vec<String>> = match &ob.ou {
                    Some(declarations::obligations::Ou::Lieux(l)) => Some(l.iter().map(|x| x.libelle.clone()).collect()),
                    Some(declarations::obligations::Ou::Phrase(_)) | None => None,
                };
                (Genre::HorsDsn, "Elle ne part pas avec la DSN : à faire sur le service de l'organisme, puis à dire faite dans la compagnie.", lieux)
            };
            Declaration { cle: r.cle.clone(), en_retard: r.en_retard, genre, titre: r.titre.clone(), mois: None, butoir: ob.butoir, quand_dit, detail, marche: Some(marche.into()), ou, fichier: None, etape: None, pret: Pret::default() }
        }
        "dpae" => {
            let jour = r.date;
            Declaration {
                cle: r.cle.clone(),
                en_retard: r.en_retard,
                genre: Genre::Dpae,
                titre: r.titre.clone(),
                mois: None,
                quand_dit: jour.map(|j| format!("le {}", j.jj_mm_aaaa())),
                butoir: jour,
                detail: r.detail.clone(),
                marche: Some("La DPAE se dépose dans la compagnie, avant la première heure de travail.".into()),
                ou: None,
                fichier: None,
                etape: None,
                pret: Pret::default(),
            }
        }
        _ => return Ok(None),
    };
    if let Some(f) = fichier.filter(|f| f.is_file()) {
        // Le depot reel de ce contenu est deja note : le rappel se taira au prochain releve.
        let Some(etape) = etape_du_fichier(ctx, lignes, &f, mensuelle)? else { return Ok(None) };
        let octets = std::fs::read(&f).map_err(|e| socle::echec::du_disque(&f, &e).reclasser(Classe::du_disque(&e)))?;
        let reel_code = declarations::dsn_lue::lire(&octets, "")?.reel();
        d.pret = Pret { test: etape == Etape::ATester, reel: etape == Etape::Testee && reel_code };
        d.fichier = Some(f.to_string_lossy().into_owned());
        d.etape = Some(etape);
        d.marche = Some(etape.marche().into());
    }
    Ok(Some(d))
}

/// Le plus presse d'abord : en retard, puis par date limite ; sans date, a la fin.
fn cle_d_urgence(en_retard: bool, butoir: &Option<Jour>) -> (bool, bool, Option<Jour>) {
    (!en_retard, butoir.is_none(), *butoir)
}

/// LE TABLEAU DE LA COMPAGNIE `ctx` : ses declarations obligatoires, du plus presse au moins presse, chacune avec sa date
/// limite et, pour ce qui part par la DSN, ou en est son fichier. Ecrit dans `config/.tableau.json` (ce que l'accueil lit).
pub(crate) fn tableau_compagnie(ctx: &Contexte) -> R<TableauCompagnie> {
    // L'EMPREINTE DE TOUTE LA COMPAGNIE, prise avant (celle de l'etat d'ouverture, `etat_garde::empreinte_de`) : le fil de
    // la fenetre ne le refait pas tant qu'elle n'a pas bouge (`FileDesEtats::faire_le_tableau`).
    let empreinte = crate::etat_garde::empreinte_de(&ctx.racine, crate::etat_garde::OUVERTURE, ctx.aujourdhui);
    let rappels = crate::rappels::rappels(ctx).map_err(|e| e.reclasser(Classe::Refuse))?;
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let mut declarations = Vec::new();
    for r in &rappels.rappels {
        if let Some(d) = declaration(ctx, &lignes, r)? {
            declarations.push(d);
        }
    }
    declarations.sort_by_key(|d| cle_d_urgence(d.en_retard, &d.butoir));
    let t = TableauCompagnie { le: ctx.aujourdhui, declarations, empreinte };
    socle::ecriture::ecrire_json(&ctx.racine.join(TABLEAU), &t, 2)?;
    Ok(t)
}

fn o_de(o: &Options, racine: &Path) -> Options {
    Options { racine: Some(racine.to_path_buf()), date: o.date }
}

/// Le tableau d'une compagnie, refait (sa propre compagnie de routes, sa racine en argument) ; son refus en phrase.
fn refaire_compagnie(o: &Options, racine: &Path) -> R<TableauCompagnie> {
    let oc = o_de(o, racine);
    let ctx = crate::service::contexte(&oc)?;
    tableau_compagnie(&ctx)
}

/// UNE COMPAGNIE DU TABLEAU DE L'ADMINISTRATEUR : son identifiant et sa couleur a l'accueil, son nom, le jour de son
/// tableau, ce qui l'empeche de se faire, combien de declarations, et combien sont pretes.
#[derive(Debug, Serialize)]
pub struct CompagnieDuTableau {
    pub id: Option<String>,
    pub nom: String,
    pub couleur: Option<String>,
    pub le: Option<Jour>,
    pub erreur: Option<String>,
    pub declarations: usize,
    pub pretes_test: usize,
    pub pretes_reel: usize,
}

/// UNE DECLARATION DU TABLEAU DE L'ADMINISTRATEUR : celle de sa compagnie, la compagnie qui la porte, et sa date limite
/// telle que la ligne la dit.
#[derive(Debug, Serialize)]
pub struct DeclarationDuTableau {
    #[serde(flatten)]
    pub declaration: Declaration,
    pub compagnie: String,
    pub id: Option<String>,
    pub couleur: Option<String>,
    pub limite_dit: Option<String>,
}

/// Ce que le depot groupe propose, en test ou en reel : combien de declarations, de compagnies, s'il se propose (au
/// moins deux compagnies), son bouton, et le titre de la demande de mot de passe (en reel).
#[derive(Debug, Serialize)]
pub struct Propose {
    pub declarations: usize,
    pub compagnies: usize,
    pub propose: bool,
    pub bouton: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titre: Option<String>,
}

/// Le depot groupe, en test et en reel.
#[derive(Debug, Serialize)]
pub struct Groupe {
    pub test: Propose,
    pub reel: Propose,
}

/// LE TABLEAU DE L'ADMINISTRATEUR : sa phrase et ses lignes, ses compagnies, les declarations de toutes du plus presse au
/// moins presse, ce que le depot groupe propose, et s'il est le garde (sans calcul) ou refait.
#[derive(Debug, Serialize)]
pub struct TableauAdministrateur {
    pub phrase: String,
    pub phrases: Vec<String>,
    pub compagnies: Vec<CompagnieDuTableau>,
    pub declarations: Vec<DeclarationDuTableau>,
    pub groupe: Groupe,
    pub garde: bool,
}

impl Dite for TableauAdministrateur {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// LE TABLEAU DE L'ADMINISTRATEUR, a partir du tableau de chaque compagnie (`garde` : celui qui est garde, sans calcul ;
/// sinon refait). Les declarations de toutes, du plus presse au moins presse, ce que le depot groupe propose, et ses
/// phrases : combien de declarations, puis chacune (sa compagnie, ce qu'elle est, quand).
fn assembler(inst: &Path, par_compagnie: Vec<(PathBuf, R<TableauCompagnie>)>, garde: bool) -> R<TableauAdministrateur> {
    let mut cies = Vec::new();
    let mut toutes: Vec<DeclarationDuTableau> = Vec::new();
    let (mut test_n, mut test_k, mut reel_n, mut reel_k) = (0, 0, 0, 0);
    for (racine, t) in par_compagnie {
        let fiche = fiche_de(inst, &racine)?;
        let nom = match &fiche {
            Some(c) => c.nom.clone(),
            None => nom_de(inst, &racine)?,
        };
        let id = fiche.as_ref().map(|c| c.id.clone());
        let couleur = fiche.as_ref().map(|c| c.couleur.clone());
        let (decl, le, erreur) = match t {
            Ok(t) => (t.declarations, Some(t.le), None),
            Err(m) => (Vec::new(), None, Some(m.phrase)),
        };
        let pt = decl.iter().filter(|d| d.pret.test).count();
        let pr = decl.iter().filter(|d| d.pret.reel).count();
        test_n += pt;
        reel_n += pr;
        test_k += usize::from(pt > 0);
        reel_k += usize::from(pr > 0);
        cies.push(CompagnieDuTableau { id: id.clone(), nom: nom.clone(), couleur: couleur.clone(), le, erreur, declarations: decl.len(), pretes_test: pt, pretes_reel: pr });
        for d in decl {
            // Sa date limite, telle que la ligne la dit.
            let limite_dit = d.quand_dit.as_ref().map(|q| if d.en_retard { format!("en retard — c'était {}", q) } else { format!("au plus tard {}", q) });
            toutes.push(DeclarationDuTableau { declaration: d, compagnie: nom.clone(), id: id.clone(), couleur: couleur.clone(), limite_dit });
        }
    }
    toutes.sort_by_key(|d| cle_d_urgence(d.declaration.en_retard, &d.declaration.butoir));
    let phrases: Vec<String> = toutes
        .iter()
        .map(|d| {
            let quand: String = d.declaration.quand_dit.iter().map(String::as_str).collect();
            format!("{} : {}, {}{}", d.compagnie, d.declaration.titre, if d.declaration.en_retard { "en retard, c'était " } else { "au plus tard " }, quand)
        })
        .collect();
    let phrase = format!("Déclarations à faire : {} ; compagnies : {}.", phrases.len(), cies.len());
    Ok(TableauAdministrateur {
        phrase,
        phrases,
        compagnies: cies,
        declarations: toutes,
        // RIEN DE GROUPE tant qu'il n'y a pas au moins deux compagnies avec quelque chose de pret : sinon le depot se fait
        // comme d'habitude, dans la compagnie.
        groupe: Groupe {
            test: Propose { declarations: test_n, compagnies: test_k, propose: test_k >= 2, bouton: format!("Tester les {} DSN écrites", test_n), titre: None },
            reel: Propose { declarations: reel_n, compagnies: reel_k, propose: reel_k >= 2, bouton: format!("Déposer les {} DSN prêtes", reel_n), titre: Some(format!("Déposer {} DSN", reel_n)) },
        },
        garde,
    })
}

/// Le tableau de l'administrateur a partir du tableau de chaque compagnie, deja fait ailleurs (le fil de la fenetre) : celui de chacune, ou son refus en phrase.
pub fn tableau_de(inst: &Path, par_compagnie: Vec<(PathBuf, R<TableauCompagnie>)>) -> R<TableauAdministrateur> {
    assembler(inst, par_compagnie, false)
}

/// Le tableau tel qu'il est garde : AUCUN CALCUL, la lecture de `config/.tableau.json` de chaque compagnie rattachee.
/// L'accueil le montre tout de suite ; la fenetre le refait ensuite en fond (`tableau`).
pub(crate) fn tableau_garde(inst: &Path) -> R<TableauAdministrateur> {
    let par = rattachees(inst)?
        .into_iter()
        .map(|r| {
            let f = r.join(TABLEAU);
            let t = if f.is_file() { socle::forme::lire_fichier(&f, "Le tableau gardé de la compagnie ne se lit pas.") } else { Err(socle::echec::refus("Pas encore lu : le tableau se fait.")) };
            (r, t)
        })
        .collect();
    assembler(inst, par, true)
}

/// Le tableau refait, compagnie par compagnie. `relever` : avant, pour ce qui est parti en test et attend son compte rendu,
/// aller le chercher (la route `bilan-repetition` de la compagnie, comme la page d'une compagnie le fait).
pub fn tableau(o: &Options, inst: &Path, relever: bool) -> R<TableauAdministrateur> {
    let mut par = Vec::new();
    for r in rattachees(inst)? {
        if relever {
            // Ce qui se releve ou non se voit dans le tableau refait juste apres : le releve n'en dit rien de plus.
            if let Ok(t) = refaire_compagnie(o, &r) {
                for d in t.declarations.iter().filter(|d| d.etape == Some(Etape::EnTest)) {
                    let charge = en_charge(designation(d)?)?;
                    let _ = crate::servir_la_route(&o_de(o, &r), Route::BilanRepetition, &charge);
                }
            }
        }
        let t = refaire_compagnie(o, &r);
        par.push((r, t));
    }
    assembler(inst, par, false)
}

// ------------------------------------------------------------------ le depot groupe

/// CE QU'UNE DECLARATION DU DEPOT GROUPE A DONNE : sa compagnie, ce qu'elle est, son fichier, si elle est partie (et la
/// phrase qui le dit), ou son refus et sa classe.
#[derive(Debug, Serialize)]
pub struct Resultat {
    pub compagnie: String,
    pub titre: Option<String>,
    pub fichier: Option<String>,
    pub genre: Option<Genre>,
    pub parti: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phrase: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classe: Option<Classe>,
    /// Le flux de ce qui est parti (ce que `suivre-depot` prend pour suivre ses retours).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flux: Option<String>,
    /// Une declaration partie en reel, dont les retours se suivent (`administrateur-suivre`) : sa compagnie et ce que
    /// `suivre-depot` prend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suivre: Option<ASuivre>,
}

/// CE QUI SE SUIT APRES UN DEPOT REEL DU GROUPE : la compagnie (son dossier, relatif a l'installation, tel que le registre
/// le nomme) et la charge de sa route `suivre-depot`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ASuivre {
    pub dossier: String,
    #[serde(flatten)]
    pub(crate) depot: crate::depots::ChargeSuivre,
}

/// Ou en est une declaration suivie : sa compagnie, et ce que sa route `suivre-depot` rend.
#[derive(Debug, Serialize)]
pub struct Suivie {
    pub compagnie: String,
    #[serde(flatten)]
    pub(crate) suivi: crate::depots::Suivi,
}

/// CE QUE REND LE SUIVI DU DEPOT GROUPE : chaque declaration suivie, et si toutes sont tranchees (`fini`).
#[derive(Debug, Serialize)]
pub struct SuivisDuGroupe {
    pub phrase: String,
    pub fini: bool,
    pub suivies: Vec<Suivie>,
}

impl Dite for SuivisDuGroupe {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// LE SUIVI DU DEPOT GROUPE, juste apres un depot reel : chaque declaration partie se suit dans sa compagnie, par la meme
/// lecture que la page d'une compagnie (`routes::depots::suivre_depot`). Une compagnie que le registre ne nomme pas se
/// refuse. Une LECTURE : rien ne part.
pub fn suivre(o: &Options, inst: &Path, a_suivre: Vec<ASuivre>) -> R<SuivisDuGroupe> {
    let groupe = rattachees(inst)?;
    let mut suivies = Vec::new();
    for s in a_suivre {
        let racine = inst.join(&s.dossier);
        if !groupe.contains(&racine) {
            return Err(erreur(Classe::Refuse, format!("La compagnie « {} » n'est pas rattachée à l'administrateur : rien n'est suivi.", s.dossier)));
        }
        let ctx = crate::service::contexte(&o_de(o, &racine))?;
        let suivi = crate::depots::suivre_depot(&ctx, s.depot)?;
        suivies.push(Suivie { compagnie: nom_de(inst, &racine)?, suivi });
    }
    let fini = suivies.iter().all(|s| s.suivi.fini);
    let tranches = suivies.iter().filter(|s| s.suivi.fini).count();
    let attendent = suivies.len() - tranches;
    let phrase = if fini {
        format!("{} : retours reçus.", socle::ecran::compte(tranches, "déclaration tranchée", "déclarations tranchées"))
    } else {
        format!("{} attendent encore le retour de l'organisme.", socle::ecran::compte(attendent, "déclaration", "déclarations"))
    };
    Ok(SuivisDuGroupe { phrase, fini, suivies })
}

/// CE QUE REND LE DEPOT GROUPE : reel ou de test, combien sont partis et refuses, sa phrase, chaque declaration, et le
/// tableau refait.
#[derive(Debug, Serialize)]
pub struct DepotGroupe {
    pub phrase: String,
    pub reel: bool,
    pub partis: usize,
    pub refuses: usize,
    pub resultats: Vec<Resultat>,
    pub tableau: TableauAdministrateur,
}

impl Dite for DepotGroupe {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// LE DEPOT GROUPE : ce qui est pret (`reel` : teste et conforme ; sinon : ecrit et pas encore essaye) dans les compagnies
/// de l'administrateur part, compagnie par compagnie, par la route `deposer` de chacune. `motdepasse` (reel) : celui que la
/// fenetre a fait taper au moment de l'acte ; sans lui, le moteur le demande a un humain. Rend, declaration par
/// declaration, ce qui est parti et ce qui a refuse, et le tableau refait.
pub fn deposer(o: &Options, inst: &Path, reel: bool, motdepasse: Option<String>) -> R<DepotGroupe> {
    let groupe = rattachees(inst)?;
    if groupe.is_empty() {
        return Err(erreur(Classe::Refuse, "Ce logiciel n'a pas d'administrateur : chaque compagnie dépose depuis sa page. Rien n'est parti."));
    }
    // Ce qui est pret, decide maintenant, sur l'etat d'aujourd'hui (pas sur le tableau garde).
    let mut a_faire: Vec<(PathBuf, String, Declaration)> = Vec::new();
    let mut resultats: Vec<Resultat> = Vec::new();
    for r in &groupe {
        let nom = nom_de(inst, r)?;
        match refaire_compagnie(o, r) {
            Ok(t) => {
                for d in t.declarations {
                    if if reel { d.pret.reel } else { d.pret.test } {
                        a_faire.push((r.clone(), nom.clone(), d));
                    }
                }
            }
            Err(m) => resultats.push(Resultat { compagnie: nom, titre: None, fichier: None, genre: None, parti: false, phrase: None, refus: Some(m.phrase), classe: None, flux: None, suivre: None }),
        }
    }
    // RIEN DE PRET : rien ne part, et la phrase dit pourquoi (ce n'est pas un refus : il n'y a rien a corriger).
    if a_faire.is_empty() {
        let phrase = if reel {
            "Aucune déclaration n'est prête à partir pour de vrai : une déclaration part en réel après un test revenu conforme."
        } else {
            "Aucune déclaration écrite n'attend son dépôt de test."
        };
        let refuses = resultats.len();
        return Ok(DepotGroupe { phrase: phrase.into(), reel, partis: 0, refuses, resultats, tableau: tableau(o, inst, false)? });
    }
    let mut mdp: Option<String> = None;
    if reel {
        let m = match motdepasse.filter(|m| !m.is_empty()) {
            Some(m) => m,
            None => {
                let acte = compagnie::demande::Acte::nouveau(
                    format!("depot REEL de {} declaration(s), dans {} compagnie(s)", a_faire.len(), groupe.len()),
                    None,
                    "net-entreprises (les declarations partent a l'Urssaf et aux organismes)",
                );
                compagnie::demande::mot_de_passe_de_l_acte(&a_faire[0].0, &acte).map_err(|e| e.reclasser(Classe::PreuveInsuffisante))?
            }
        };
        if !ouvre_une(&Poste::de_ce_poste(), &groupe, &m)? {
            return Err(erreur(Classe::PreuveInsuffisante, "Mot de passe de l'administrateur refusé. Rien n'est parti."));
        }
        mdp = Some(m);
    }
    for (racine, nom, d) in &a_faire {
        let charge = en_charge(ChargeDeposer::de(designation(d)?, reel, mdp.clone()))?;
        let avant = envois::journal::lu(racine)?.lignes.len();
        let rep = crate::servir_la_route(&o_de(o, racine), Route::Deposer, &charge);
        compagnie::coffre::oublier_mot_de_passe_tape();
        let (parti, phrase, refus, classe, flux) = match rep.en_refus() {
            Ok(_) => {
                let phrase = if reel {
                    "Partie pour de vrai : son bilan arrive avec les retours."
                } else {
                    "Partie en test, compte rendu attendu : le dépôt réel se fait une fois le test revenu conforme."
                };
                // Le flux de ce qui est parti : la ligne que le depot a ecrite au journal des envois.
                let journal = envois::journal::lu(racine)?;
                let flux = journal.lignes.get(avant..).and_then(|nouvelles| nouvelles.last()).map(|l| l.flux().to_string());
                (true, Some(phrase), None, None, flux)
            }
            Err(r) => (false, None, Some(r.phrase), r.classe, None),
        };
        // Une declaration partie en reel se suit : la DPAE, la DSN et ses signalements (un depot hors DSN n'a pas de retour
        // a suivre ici).
        let quoi = match d.genre {
            Genre::Dpae => Some(Quoi::Dpae),
            Genre::Dsn | Genre::Signalement => Some(Quoi::Dsn),
            Genre::HorsDsn => None,
        };
        let suivre = match (reel && parti, quoi, racine.strip_prefix(inst)) {
            (true, Some(quoi), Ok(dossier)) => Some(ASuivre {
                dossier: dossier.to_string_lossy().into_owned(),
                depot: crate::depots::ChargeSuivre { quoi, flux: flux.clone(), mois: d.mois },
            }),
            _ => None,
        };
        resultats.push(Resultat { compagnie: nom.clone(), titre: Some(d.titre.clone()), fichier: d.fichier.clone(), genre: Some(d.genre), parti, phrase, refus, classe, flux, suivre });
    }
    let partis = resultats.iter().filter(|r| r.parti).count();
    let refuses = resultats.len() - partis;
    let refuses_dit: String = (refuses > 0).then(|| format!(", {}", socle::ecran::compte(refuses, "refusée", "refusées"))).into_iter().collect();
    let phrase = format!("{}{}.", socle::ecran::compte(partis, "déclaration partie", "déclarations parties"), refuses_dit);
    Ok(DepotGroupe { phrase, reel, partis, refuses, resultats, tableau: tableau(o, inst, false)? })
}

/// La designation de la declaration `d` du tableau : la DSN de son mois, ou le signalement ecrit.
fn designation(d: &Declaration) -> R<Designation> {
    match (d.genre, d.mois, &d.fichier) {
        (Genre::Dsn, Some(m), _) => Ok(Designation::du_mois(Quoi::Dsn, m)),
        (_, _, Some(f)) => Ok(Designation::du_fichier(f.clone())),
        _ => Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : une déclaration prête sans mois ni fichier.")),
    }
}

/// Une charge de route, telle que `servir_la_route` la lit.
fn en_charge(c: impl serde::Serialize) -> R<crate::enveloppe::Charge> {
    crate::enveloppe::Charge::de_forme(&c)
}
