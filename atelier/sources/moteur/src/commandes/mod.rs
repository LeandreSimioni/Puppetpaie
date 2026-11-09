//! La ligne de commande de ce qui n'est pas un geste de l'ecran, `moteur COMMANDE ...` : le coffre (la seance, et les
//! secrets tapes sans echo, que l'entree standard ne porte jamais), la restauration d'un poste neuf depuis le Drive, et
//! l'enquete des depots (`depots`). `ligne_de_commande` aiguille toute la ligne de commande du moteur ; le binaire de
//! l'atelier (`moteur-atelier`, la crate `atelier`) la reprend et y donne ses commandes, qui passent par le meme cadre. Un geste de l'ecran se fait par sa route : `moteur routes` les dit, `moteur route` en sert une
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
        let nature = ::compagnie::incidents::nature_de(e.classe);
        let ou = if nature == ::compagnie::incidents::REFUS { self.commande } else { "" };
        if let Err(f) = ::compagnie::incidents::depuis_exception(&self.racine, e.classe, &e.message, Some(nature), ou, DeLaCommande { commande: self.commande }, None) {
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
        let c = ::compagnie::coffre::Coffre::du_poste(&self.racine, "");
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
        eprintln!("fois, pour {} h.", ::compagnie::demande::HEURES_SEANCE);
        if let ::compagnie::demande::Verdict::Refuse(phrase) = ::compagnie::demande::ouvrir_seance(&self.racine) {
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

/// Les commandes du produit, puis `de_l_atelier` (celles de `moteur-atelier`, aucune dans `moteur`), dans cet ordre.
fn commandes(de_l_atelier: Vec<Commande>) -> Vec<Commande> {
    let mut v = vec![
        Commande { nom: "coffre", aide: "les mots de passe des organismes, scelles sur le disque (ce qui est pose)", cachee: false, spec: &coffre::SPEC, executer: coffre::executer },
        // L'enquete de l'editeur sur le poste d'une compagnie : elle marche, elle ne figure pas dans l'aide.
        Commande { nom: "depots", aide: "lister les depots vus par net-entreprises (enquete)", cachee: true, spec: &depots::SPEC, executer: depots::executer },
        Commande { nom: "restaurer", aide: "redescendre le dossier prive du Drive (poste neuf, ordinateur perdu)", cachee: false, spec: &restaurer::SPEC, executer: restaurer::executer },
    ];
    v.extend(de_l_atelier);
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

/// LA LIGNE DE COMMANDE DU MOTEUR (`args`, sans le nom du programme) : la fabrique de PDF, les routes, leur catalogue, la
/// mise a jour et la publication par GitHub, la compagnie d'essai, la fenetre, puis les commandes du produit et
/// `de_l_atelier` (`executer`). Son code de sortie ; `None` quand il faut dire l'usage.
pub fn ligne_de_commande(args: &[String], de_l_atelier: Vec<Commande>) -> Option<std::process::ExitCode> {
    ::compagnie::incidents::brancher_l_engagement();
    let code = |c: i32| std::process::ExitCode::from(c.clamp(0, 255) as u8);
    Some(match args.first().map(String::as_str) {
        // Les commandes de la fabrique, servies par sa bibliotheque.
        Some("pdf") => fabrique_pdf::ligne_de_commande::executer(&args[1..]),
        // Les routes de l'interface, une par processus (la demande JSON sur l'entree standard).
        Some("route") => match routes::service::commande("route", &args[1..]) {
            Some(c) => std::process::exit(c),
            None => return None,
        },
        // Le catalogue des routes, tire de leurs fiches.
        Some("routes") => code(routes::catalogue::commande(&args[1..])),
        // La mise a jour : verifier, appliquer, revenir ; et la publication par GitHub.
        Some("maj") => code(match crate::publication::commande(&args[1..]) {
            Some(c) => c,
            None => maj::commande(&args[1..]),
        }),
        // La compagnie d'essai : un vrai depot de TEST sans `prive/`, l'identite lue dans `PUPPETPAIE_COMPAGNIE`.
        Some("essai") => code(::compagnie::essai::commande(&args[1..])),
        // La fenetre, dans le navigateur du poste : `moteur interface`.
        Some("interface") => code(crate::interface::commande(&args[1..])),
        // Le reste : les commandes du produit et celles de l'atelier, par leur cadre (`moteur [--dev] COMMANDE ...`).
        Some(_) => code(executer(args, de_l_atelier)),
        None => return None,
    })
}

/// `moteur [--dev] COMMANDE ...`, parmi les commandes du produit et `de_l_atelier`. Rend le code de sortie.
fn executer(args: &[String], de_l_atelier: Vec<Commande>) -> i32 {
    let mut args = args;
    let mut dev = false;
    if args.first().map(String::as_str) == Some("--dev") {
        dev = true;
        args = &args[1..];
    }
    let table = commandes(de_l_atelier);
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
    ::compagnie::installation::fixer_le_code();
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
    let _canaux = ::compagnie::demande::installer(Rc::new(::compagnie::demande::Reel::du_poste()));
    (c.executer)(&cadre, &a)
}
