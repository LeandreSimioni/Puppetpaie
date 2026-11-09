//! La ligne de commande de ce qui n'est pas un geste de l'ecran, `moteur COMMANDE ...` : le coffre (la seance, et les
//! secrets tapes sans echo, que l'entree standard ne porte jamais), la restauration d'un poste neuf depuis le Drive, et
//! l'enquete des depots (`depots`). Les commandes de l'editeur (`atelier::commandes`) passent par le meme cadre : `main`
//! les donne a `executer`. Un geste de l'ecran se fait par sa route : `moteur routes` les dit, `moteur route` en sert une
//! (`--texte` : ses phrases).
//!
//! Une commande = un module, qui declare ses options (`analyse::Spec`) et rend un code de sortie ; sa sortie est un
//! texte, ou l'enveloppe `--json` (`rendu::rendre`).
//!
//! LE MOT DE PASSE. Chaque commande installe au demarrage la demande reelle (`demande::installer`). Une
//! consultation qui trouve le coffre ferme -- ni la cle du poste, ni un mot de passe tape, ni une seance -- le demande
//! par `demande::ouvrir_seance` (la fenetre du moteur s'il y en a une ; le terminal, sans echo ; sinon un refus avec sa phrase), jamais
//! par autre chose.

pub mod actes;
pub mod analyse;
pub mod coffre;
pub mod compagnie;
pub mod depots;
pub mod maj;
pub mod rendu;
pub mod restaurer;

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::Echec;
use analyse::{Args, Spec};

/// Ce qu'une commande trouve en arrivant : la compagnie, le jour, et ses options.
pub struct Cadre {
    pub racine: PathBuf,
    /// `--racine` et `--date` des routes du moteur (`service::options`).
    pub o: HashMap<String, String>,
    pub ctx: Contexte,
    pub aujourdhui: Jour,
    /// Le nom de la commande, pour le journal des incidents.
    pub commande: &'static str,
}

/// Ce que porte l'incident d'une commande qui tombe : son nom.
#[derive(serde::Serialize)]
struct DeLaCommande {
    commande: &'static str,
}

impl Cadre {
    /// Un plantage : une ligne dans le journal des incidents (sa classe et sa nature), puis la phrase seule a l'ecran
    /// (sortie d'erreur), code 1.
    pub fn plantage(&self, e: Echec) -> i32 {
        let nature = crate::compagnie::incidents::nature_de(e.classe);
        let ou = if nature == crate::compagnie::incidents::REFUS { self.commande } else { "" };
        if let Err(f) = crate::compagnie::incidents::depuis_exception(&self.racine, e.classe, &e.message, Some(nature), ou, DeLaCommande { commande: self.commande }, None) {
            eprintln!("(Ce refus ne se note pas au journal des incidents : {})", socle::echec::phrase_de(&f.message));
        }
        eprintln!("{}", e.message);
        1
    }

    /// Ouvre le coffre si la cle du poste ne le fait pas :
    /// rien a demander quand la cle du poste l'ouvre, ou qu'une seance est gardee ; sinon la seance se demande par
    /// `demande::ouvrir_seance`. Un refus ne coupe pas la commande : le secret vient peut-etre de
    /// l'environnement, et sinon la lecture du coffre dira elle-meme ce qui manque. La phrase est dite.
    pub fn ouvrir_le_coffre(&self) {
        let c = crate::compagnie::coffre::Coffre::du_poste(&self.racine, "");
        match c.ouvre_a_ce_poste() {
            Ok(true) => return,
            Ok(false) => {}
            // Un coffre illisible : la phrase le dit, et la lecture du secret refusera.
            Err(e) => {
                eprintln!("{}", socle::echec::phrase_de(&e.message));
                return;
            }
        }
        if matches!(c.ouverture_gardee(), Ok(Some(_))) {
            // La seance gardee ouvre le coffre (`Coffre::ouvrir`) : rien a retenir.
            return;
        }
        // Sur la sortie d'ERREUR : les phrases d'une seance ne doivent pas polluer la sortie standard d'un `--json`.
        eprintln!();
        eprintln!("Le coffre de ce poste ne s'ouvre pas avec sa cle. Ouverture de la seance : le mot de passe du logiciel, une");
        eprintln!("fois, pour {} h.", crate::compagnie::demande::HEURES_SEANCE);
        if let crate::compagnie::demande::Verdict::Refuse(phrase) = crate::compagnie::demande::ouvrir_seance(&self.racine) {
            eprintln!("! {}", phrase);
        }
    }
}

pub type Executer = fn(&Cadre, &Args) -> i32;

pub struct Commande {
    pub nom: &'static str,
    pub aide: &'static str,
    pub cachee: bool,
    pub spec: &'static Spec,
    pub executer: Executer,
}

/// Les commandes du produit, puis `de_l_editeur` (`atelier::commandes::commandes`), dans cet ordre.
fn commandes(de_l_editeur: Vec<Commande>) -> Vec<Commande> {
    let mut v = vec![
        Commande { nom: "coffre", aide: "les mots de passe des organismes, scelles sur le disque (ce qui est pose)", cachee: false, spec: &coffre::SPEC, executer: coffre::executer },
        // L'enquete de l'editeur sur le poste d'une compagnie : elle marche, elle ne figure pas dans l'aide.
        Commande { nom: "depots", aide: "lister les depots vus par net-entreprises (enquete)", cachee: true, spec: &depots::SPEC, executer: depots::executer },
        Commande { nom: "restaurer", aide: "redescendre le dossier prive du Drive (poste neuf, ordinateur perdu)", cachee: false, spec: &restaurer::SPEC, executer: restaurer::executer },
    ];
    v.extend(de_l_editeur);
    v
}

fn aide(table: &[Commande]) -> String {
    let mut t = String::from(
        "usage: moteur [--dev] COMMANDE [options]\n\nPuppetPaie - paie du spectacle vivant\n\ncommandes :\n",
    );
    for c in table.iter().filter(|c| !c.cachee) {
        t.push_str(&format!("  {:<11} {}\n", c.nom, c.aide));
    }
    t.push_str("\nTout ce que fait la fenetre : `moteur routes` (le catalogue), `moteur route` (une route, `--texte` : ses phrases). `moteur pdf` est la fabrique de PDF.\n");
    t
}

/// `moteur [--dev] COMMANDE ...`, parmi les commandes du produit et `de_l_editeur`. Rend le code de sortie.
pub fn executer(args: &[String], de_l_editeur: Vec<Commande>) -> i32 {
    let mut args = args;
    let mut dev = false;
    if args.first().map(String::as_str) == Some("--dev") {
        dev = true;
        args = &args[1..];
    }
    let table = commandes(de_l_editeur);
    let Some(nom) = args.first().map(String::as_str) else {
        eprintln!("{}", aide(&table));
        return 2;
    };
    if matches!(nom, "-h" | "--help" | "--aide" | "aide") {
        dire!("{}", aide(&table));
        return 0;
    }
    let Some(c) = table.iter().find(|c| c.nom == nom) else {
        eprintln!("moteur: error: commande inconnue : {}\n\n{}", nom, aide(&table));
        return 2;
    };
    let reste = &args[1..];
    if reste.iter().any(|a| a == "-h" || a == "--help" || a == "--aide") {
        dire!("{}", analyse::usage(c.spec));
        return 0;
    }
    let a = match analyse::analyser_ou_sortir(c.spec, reste) {
        Ok(a) => a,
        Err(code) => return code,
    };
    // La commande est lancee a la main : sans PUPPETPAIE_CODE, le code est celui qu'on trouve a cote du programme.
    crate::compagnie::installation::fixer_le_code();
    let racine = match compagnie::racine(dev) {
        Ok(r) => r,
        Err(phrase) => {
            eprintln!("{}", phrase);
            return 1;
        }
    };
    let aujourdhui = compagnie::poser_le_jour();
    let mut o = HashMap::new();
    o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
    o.insert("date".to_string(), aujourdhui.iso());
    let ctx = match Contexte::nouveau(&racine, Some(aujourdhui)) {
        Ok(c) => c,
        Err(m) => {
            eprintln!("{}", m);
            return 1;
        }
    };
    let cadre = Cadre { racine, o, ctx, aujourdhui, commande: c.nom };
    // La demande du mot de passe du logiciel est celle du moteur : fenetre s'il y en a une, sinon le terminal.
    let _canaux = crate::compagnie::demande::installer(Rc::new(crate::compagnie::demande::Reel::du_poste()));
    (c.executer)(&cadre, &a)
}
