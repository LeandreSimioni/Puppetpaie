//! SUPPRIMER UNE COMPAGNIE, ET GARDER AU DEBLOCAGE CELLES QU'ON A AJOUTEES EN DEMO. Tout se decide ici ; la fenetre (`interface/mod.rs`) passe la saisie de la page et lache ce qu'elle tient
//! de la compagnie (sa veille, sa file sur le fil des calculs, son journal) au moment ou on le lui dit.
//!
//! SUPPRIMER (`supprimer`). Une FICTIVE LIVREE ne se supprime pas : elle se remet a zero (`installation::remettre_a_zero`),
//! et la route le refuse (`genre`). Une FICTIVE AJOUTEE (`installation::est_ajoutee`) se supprime sur une confirmation simple
//! de la page. Une COMPAGNIE REELLE (`prive/`, `compagnies/<nom>`) porte des documents qu'un employeur conserve (le double
//! des bulletins, cinq ans : code du travail, article L3243-4, `atelier/reference/sources/code-du-travail/
//! articles-20261021.md`) : la supprimer est irreversible, et se confirme PAR LE MOT DE PASSE DU LOGICIEL, tape par un
//! humain, comme un acte reel -- le popup de la page (`demanderMotDePasse`, `js/boites.js`) le met dans la charge ;
//! sans lui, le moteur le demande lui-meme (`demande::mot_de_passe_de_l_acte` : fenetre, terminal, ou refus). Il est
//! verifie sur le coffre de la compagnie ; un refus n'efface rien.
//!
//! EFFACER SUREMENT (`effacer`). Sous Windows, un dossier dont un fichier est ouvert s'efface a moitie. Le dossier est
//! d'abord RENOMME en dossier cache a cote (`.supprimee-<nom>-<instant>`) -- d'un coup : la tuile disparait --, puis
//! efface ; ce qui ne s'efface pas reste cache (l'accueil, la mise a jour et l'administrateur ignorent un dossier qui
//! commence par un point) et s'efface au lancement suivant (`effacer_les_restes`). Avec la compagnie partent : son
//! entree au registre de l'administrateur (`administrateur::oublier`), sa seance gardee a cote de la cle du poste
//! (`Coffre::oublier_ouverture_gardee`). Le prechauffage (`installation::a_tenir_a_jour`) et la derniere ouverte (la date
//! de son `journal/interface.log`) se lisent sur le disque : rien a defaire.
//!
//! AU DEBLOCAGE (`a_garder`, `repondre`). Un logiciel debloque qui porte encore des fictives ajoutees demande, a l'accueil,
//! lesquelles garder ; tant qu'il en reste, la question revient. Celle qu'on GARDE devient une compagnie reelle
//! (`garder`) : elle passe sous `compagnies/`, perd son marqueur de fictive (`COPIE-DE-DEVELOPPEMENT.json`) et recoit sa
//! MENTION DE DEMO (`config/demo.json` : quand elle a ete creee, quand elle a ete gardee, les mois saisis jusque-la). Celle
//! qu'on ne garde pas est supprimee, par `supprimer` (une fictive ajoutee : sans mot de passe).
//!
//! CE QUI SUIT UNE COMPAGNIE GARDEE, tel quel : ses saisies, ses mois importes, ses journaux, ses fichiers ecrits. SON
//! COFFRE AUSSI : dans une fictive ajoutee il n'est scelle que par la cle du poste (elle s'ouvre sans mot de passe) ; une
//! compagnie reelle sans mot de passe du logiciel le fait choisir a sa premiere ouverture (`Phase::MotDePasse`, premiere),
//! et `Coffre::poser_ouverture` rescelle alors ce coffre, la cle du poste l'ouvrant -- ce qui y a ete pose en demo (un mot
//! de passe d'organisme) est garde, scelle comme celui de toute compagnie reelle. Rien n'est a reposer. Ses etats gardes
//! se refont seuls : leur empreinte porte le chemin de la compagnie (`etat_garde::empreinte_de`).
//!
//! LES PAIES FAITES EN DEMO n'ont ete ni declarees ni envoyees : en demo, tout envoi est refuse avant d'ecrire au journal
//! des depots (`bridage::branchement`). La page d'un mois les montre donc a faire (une DPAE « rien n'est parti », meme
//! pour une embauche passee : `routes::etapes`, `etats.dpae.fait` nul) ; la mention de demo le dit en plus, sur chaque mois
//! saisi en demo (`mention`, lue par la route `etat`). ELLES COMPTENT au nombre de paies annuel (`nombre_de_paies`) si
//! elles sont validees : ce sont des paies que le logiciel a faites, la regle du compte (une paie validee) ne regarde pas
//! si elle est partie ; une paie non validee ne compte pas, en demo comme ailleurs.

use std::path::{Path, PathBuf};


use compagnie::installation::{self, Compagnie};
use crate::gestes::administrateur::Poste;

/// La mention de demo d'une compagnie gardee.
pub const FICHIER_DEMO: &str = "config/demo.json";
/// Le debut du nom d'un dossier en cours d'effacement.
pub const PREFIXE_RESTE: &str = ".supprimee-";
/// Ce que la boite d'une compagnie reelle rappelle avant de la supprimer (code du travail, article L3243-4, `atelier/
/// reference/sources/code-du-travail/articles-20261021.md`).
pub const CONSERVATION: &str = "L'employeur conserve un double des bulletins de paie pendant cinq ans (code du travail, article L3243-4).";

/// Ce qu'est une compagnie pour sa suppression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Genre {
    /// Une fictive ajoutee : une confirmation simple.
    Ajoutee,
    /// Une compagnie reelle : le mot de passe du logiciel.
    Reelle,
}

/// Le dossier `p` resolu, et comparable (sous Windows, sans le `\\?\` de `canonicalize`) ; un dossier qui ne se resout
/// pas se refuse.
fn absolu(p: &Path) -> Result<PathBuf, String> {
    let c = std::fs::canonicalize(p).map_err(|e| socle::echec::du_disque(&p, &e))?;
    let t = c.to_string_lossy();
    Ok(match t.strip_prefix(r"\\?\") {
        Some(s) => PathBuf::from(s),
        None => c.clone(),
    })
}

/// La compagnie `racine` est-elle une compagnie de l'installation `inst` -- `prive/`, `compagnies/<nom>`,
/// `fictives/<nom>`, et rien d'autre (ni `logiciel/`, ni un dossier cache) ?
fn de_l_installation(inst: &Path, racine: &Path) -> Result<bool, String> {
    let (i, r) = (absolu(inst)?, absolu(racine)?);
    let Some(nom) = r.file_name().map(|n| n.to_string_lossy().into_owned()) else { return Ok(false) };
    if nom.starts_with('.') || !r.join(droit::contexte::ENTREPRISE).is_file() {
        return Ok(false);
    }
    Ok(r == i.join("prive") || r.parent().is_some_and(|p| p == i.join("compagnies") || p == i.join(installation::DOSSIER_FICTIVES)))
}

/// Ce qu'est `racine` pour sa suppression, ou pourquoi elle ne se supprime pas.
pub fn genre(inst: &Path, racine: &Path) -> Result<Genre, String> {
    if !de_l_installation(inst, racine)? {
        return Err(socle::echec::avec(
            "Ce dossier n'est pas une compagnie de ce logiciel : rien n'a été supprimé.",
            socle::echec::APart { chemin: Some(racine.to_string_lossy().into_owned()), ..Default::default() },
        ));
    }
    if installation::est_ajoutee(racine)? {
        return Ok(Genre::Ajoutee);
    }
    if installation::est_fictive(racine)? {
        return Err("Une compagnie fictive livrée ne se supprime pas : elle se remet à zéro, depuis sa tuile de l'accueil.".into());
    }
    if installation::est_copie(racine) {
        return Err("Une copie de test ne se supprime pas depuis le logiciel.".into());
    }
    Ok(Genre::Reelle)
}

/// LA QUESTION de la suppression, telle que la page la pose (route `compagnie-supprimer`, sans `confirme`) : le nom, le genre, et pour
/// une compagnie reelle le rappel de la conservation et la sauvegarde a proposer (un Drive lie : la date du dernier
/// passage). Une compagnie qui ne se supprime pas rend, sans genre, la phrase qui dit pourquoi.
pub fn question(racine: &Path) -> Result<Suppression, String> {
    let inst = crate::gestes::administrateur::installation_de(racine);
    let nom = installation::nom_de(racine)?;
    let g = match genre(&inst, racine) {
        Ok(g) => g,
        Err(phrase) => return Ok(Suppression { phrase, ..Suppression::de(nom) }),
    };
    let sauvegarde = match g {
        Genre::Reelle => {
            let liee = racine.join(socle::fichiers::CONFIG).join(crate::routes::coffre::FICHIER_JETON).is_file();
            let derniere = compagnie::sauvegarde::derniere(racine).map_err(|e| e.message)?.map(|d| d.quand);
            let phrase = match &derniere {
                Some(q) => format!("Dernière sauvegarde dans le Drive : {}.", socle::ecran::quand(q)),
                None => "Aucune sauvegarde n'est encore partie dans le Drive.".to_string(),
            };
            Some(SauvegardeAProposer { liee, derniere, phrase })
        }
        Genre::Ajoutee => None,
    };
    Ok(Suppression {
        phrase: format!("Supprimer {} ?", nom),
        titre_dit: Some(format!("Supprimer {} ?", nom)),
        genre: Some(if g == Genre::Ajoutee { "ajoutee" } else { "reelle" }),
        mot_de_passe: Some(g == Genre::Reelle),
        conservation: (g == Genre::Reelle).then_some(CONSERVATION),
        sauvegarde,
        avertissement: Some(format!("Tout le dossier de {} sera effacé de ce poste : fiches, paies, documents, journaux. Cela ne se défait pas.", nom)),
        ..Suppression::de(nom)
    })
}

/// La charge de `compagnie-supprimer` : effacer (`confirme`), ou seulement dire ce qui sera efface. Le mot de passe du
/// logiciel est celui de l'enveloppe (`Charge::mot_de_passe`).
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeSupprimer {
    pub confirme: Option<bool>,
}

/// La sauvegarde a proposer avant de supprimer une compagnie reelle : un Drive lie, la date du dernier passage, la phrase.
#[derive(Debug, serde::Serialize)]
pub struct SauvegardeAProposer {
    liee: bool,
    derniere: Option<String>,
    phrase: String,
}

/// CE QUE REND `compagnie-supprimer` : sans `confirme`, la question (le nom, le genre, ce qui sera efface, la
/// conservation et la sauvegarde d'une compagnie reelle) -- sans genre, la phrase qui dit pourquoi elle ne se supprime
/// pas ; avec, la compagnie supprimee.
#[derive(Debug, serde::Serialize)]
pub struct Suppression {
    phrase: String,
    nom: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    titre_dit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    genre: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mot_de_passe: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conservation: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sauvegarde: Option<SauvegardeAProposer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avertissement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    supprimee: Option<String>,
}

impl Suppression {
    fn de(nom: String) -> Suppression {
        Suppression { phrase: String::new(), nom, titre_dit: None, genre: None, mot_de_passe: None, conservation: None, sauvegarde: None, avertissement: None, supprimee: None }
    }
}

impl crate::routes::enveloppe::Dite for Suppression {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// LA ROUTE `compagnie-supprimer` : avertir, puis effacer. Sans `confirme` : la question (`question`) et l'avertissement,
/// qui dit ce qui sera efface ; rien ne bouge. Avec `confirme` : la suppression (`supprimer`), la preuve d'une compagnie
/// reelle comprise (`motdepasse`, que la page a fait taper ; sinon le moteur le demande lui-meme a un humain). `avant` :
/// ce que la fenetre lache de la compagnie avant qu'elle parte (rien, par `moteur route`).
pub fn route(racine: &Path, charge: ChargeSupprimer, motdepasse: Option<&str>, avant: &dyn Fn(&Path)) -> Result<Suppression, String> {
    if charge.confirme != Some(true) {
        return question(racine);
    }
    let inst = crate::gestes::administrateur::installation_de(racine);
    let nom = supprimer(&inst, racine, motdepasse, &Poste::de_ce_poste(), avant)?;
    Ok(Suppression { phrase: format!("{} est supprimée de ce poste.", nom), supprimee: Some(nom.clone()), ..Suppression::de(nom) })
}

/// La preuve d'une suppression reelle : le mot de passe du logiciel, verifie sur le coffre de la compagnie. Vide : le
/// moteur le demande lui-meme (`demande::mot_de_passe_de_l_acte`). Un coffre sans mot de passe du logiciel ne prouve rien.
fn prouver(racine: &Path, nom: &str, motdepasse: Option<&str>, poste: &Poste) -> Result<(), String> {
    let coffre = poste.coffre(racine);
    if !coffre.ouverture_exigee().map_err(|e| e.message)? {
        return Err("Aucun mot de passe du logiciel n'est posé pour cette compagnie : rien ne prouve qui la supprime. Rien n'a été supprimé.".into());
    }
    let tape = match motdepasse.filter(|m| !m.is_empty()) {
        Some(m) => m.to_string(),
        None => {
            let acte = compagnie::demande::Acte::nouveau(format!("supprimer définitivement la compagnie {}", nom), "", "");
            compagnie::demande::mot_de_passe_de_l_acte(racine, &acte).map_err(|m| format!("{} Rien n'a été supprimé.", m))?
        }
    };
    if !coffre.verifier_ouverture(&tape).map_err(|e| e.message)? {
        return Err("Mot de passe du logiciel refusé. Rien n'a été supprimé.".into());
    }
    Ok(())
}

/// SUPPRIMER `racine`, compagnie de l'installation `inst` : la preuve d'abord (une compagnie reelle), puis `avant` (la
/// fenetre lache ce qu'elle tient de la compagnie), puis l'effacement (`effacer`). Rend le nom de la compagnie. Un refus
/// n'efface rien et n'appelle pas `avant`.
pub fn supprimer(inst: &Path, racine: &Path, motdepasse: Option<&str>, poste: &Poste, avant: &dyn Fn(&Path)) -> Result<String, String> {
    let g = genre(inst, racine)?;
    let nom = installation::nom_de(racine)?;
    if g == Genre::Reelle {
        prouver(racine, &nom, motdepasse, poste)?;
    }
    avant(racine);
    let _ = poste.coffre(racine).oublier_ouverture_gardee();
    crate::gestes::administrateur::oublier(inst, racine)?;
    effacer(racine)?;
    Ok(nom)
}

/// Le dossier `racine` renomme en dossier cache a cote, puis efface ; ce qui resiste reste cache, et s'efface au lancement
/// suivant (`effacer_les_restes`). Echoue seulement si le renommage echoue : alors rien n'a bouge.
fn effacer(racine: &Path) -> Result<(), String> {
    let parent = racine.parent().ok_or("Un dossier sans parent ne se supprime pas.")?;
    let nom = racine.file_name().ok_or("Un dossier sans nom ne se supprime pas.")?.to_string_lossy().into_owned();
    let instant = socle::temps::maintenant_ms();
    let cache = parent.join(format!("{}{}-{}", PREFIXE_RESTE, nom, instant));
    std::fs::rename(racine, &cache).map_err(|e| {
        socle::echec::avec(
            "La compagnie ne se supprime pas : fermez ce qui l'utilise, puis recommencez. Rien n'a été supprimé.",
            socle::echec::APart { chemin: Some(racine.to_string_lossy().into_owned()), detail: Some(e.to_string()), ..Default::default() },
        )
    })?;
    // Ce qui resiste reste cache, et part au lancement suivant (`effacer_les_restes`).
    if let Err(e) = std::fs::remove_dir_all(&cache) {
        eprintln!("{}", socle::echec::du_disque(&cache, &e));
    }
    Ok(())
}

/// AU LANCEMENT : les restes d'une suppression qui ne s'etaient pas effaces. Rend ceux qui sont partis.
pub fn effacer_les_restes(inst: &Path) -> Vec<PathBuf> {
    let mut partis = Vec::new();
    for d in [inst.to_path_buf(), inst.join("compagnies"), inst.join(installation::DOSSIER_FICTIVES)] {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.file_name().to_string_lossy().starts_with(PREFIXE_RESTE) && std::fs::remove_dir_all(e.path()).is_ok() {
                partis.push(e.path());
            }
        }
    }
    partis
}

// ------------------------------------------------------------------ au deblocage

/// LES FICTIVES AJOUTEES A GARDER OU NON : sur un logiciel debloque, celles qui sont encore la ; bride, aucune (la question
/// ne se pose pas).
pub fn a_garder(inst: &Path) -> Result<Vec<Compagnie>, String> {
    if compagnie::bridage::est_bride_de(inst) {
        return Ok(Vec::new());
    }
    let mut ajoutees = Vec::new();
    for c in installation::liste_de(inst)? {
        if !c.reelle && installation::est_ajoutee(&c.racine)? {
            ajoutees.push(c);
        }
    }
    Ok(ajoutees)
}

/// GARDER la fictive ajoutee `racine` : elle devient une compagnie reelle, sous `compagnies/` (un nom de dossier tire du
/// sien, jamais celui d'une compagnie deja la), sans marqueur de fictive, avec sa mention de demo. Rend la compagnie.
fn garder(inst: &Path, racine: &Path) -> Result<Compagnie, String> {
    let nom = installation::nom_de(racine)?;
    let creee_le = installation::marqueur(racine)?
        .and_then(|m| m.quand)
        .ok_or("Le marqueur de la compagnie créée en démo ne dit pas quand elle a été créée : rien n'a changé.")?;
    let parent = inst.join("compagnies");
    std::fs::create_dir_all(&parent).map_err(|e| socle::echec::du_disque(&parent, &e))?;
    let base = installation::nom_de_dossier(&nom);
    let mut cible = parent.join(&base);
    let mut n = 2;
    while cible.exists() {
        cible = parent.join(format!("{}-{}", base, n));
        n += 1;
    }
    let mention = FichierDemo {
        creee_le,
        gardee_le: socle::paris::maintenant(),
        mois: engagement::stock::mois_saisis(racine).map_err(|e| socle::echec::Echec::from(e).phrase())?.into_iter().map(String::from).collect(),
        quoi: QUOI_DEMO.to_string(),
    };
    socle::ecriture::ecrire_json(&racine.join(FICHIER_DEMO), &mention, 2)?;
    if let Err(e) = std::fs::rename(racine, &cible) {
        let _ = std::fs::remove_file(racine.join(FICHIER_DEMO));
        return Err(format!("{} ne se garde pas ({}) : rien n'a changé.", nom, e));
    }
    // Sans marqueur : une compagnie reelle. S'il ne s'efface pas, elle resterait une fictive ajoutee hors de `fictives/` :
    // le dire, plutot que la laisser envoyer sans le savoir... ou ne jamais envoyer.
    let a_part = || socle::echec::APart { chemin: Some(cible.to_string_lossy().into_owned()), ..Default::default() };
    std::fs::remove_file(cible.join(installation::MARQUEUR))
        .map_err(|e| socle::echec::avec("Le marqueur de la fictive ne s'efface pas.", socle::echec::APart { detail: Some(e.to_string()), ..a_part() }))?;
    let gardee = absolu(&cible)?;
    for c in installation::liste_de(inst)? {
        if absolu(&c.racine)? == gardee {
            return Ok(c);
        }
    }
    Err(socle::echec::avec(format!("{} a été gardée, mais l'accueil ne la trouve pas.", nom), a_part()))
}

/// LA REPONSE A LA QUESTION DU DEBLOCAGE : `garder` et `ne_pas_garder`, des identifiants de tuile (`fictive:<dossier>`)
/// parmi `a_garder`. Chacune est gardee ou supprimee ; une fictive ajoutee nommee nulle part reste, et la question
/// reviendra pour elle. `lacher` : ce que la fenetre tient de chacune (sa file sur le fil des calculs), avant qu'elle
/// bouge. Rend ce qui est garde, supprime, et chaque echec : un echec sur l'une n'arrete pas les autres.
pub fn repondre(inst: &Path, garder_ids: &[String], ne_pas_garder: &[String], poste: &Poste, lacher: &dyn Fn(&Path)) -> Result<Reponse, String> {
    if compagnie::bridage::est_bride_de(inst) {
        return Err("Le logiciel est bridé : ses compagnies ajoutées restent des compagnies fictives.".into());
    }
    if let Some(id) = garder_ids.iter().find(|id| ne_pas_garder.contains(id)) {
        return Err(format!("{} est à la fois gardée et supprimée : rien n'a changé.", id));
    }
    let proposees = a_garder(inst)?;
    let inconnue = garder_ids.iter().chain(ne_pas_garder).find(|id| !proposees.iter().any(|c| &c.id == *id));
    if let Some(id) = inconnue {
        return Err(format!("{} n'est pas une compagnie créée en démo : rien n'a changé.", id));
    }
    let (mut gardees, mut supprimees, mut erreurs) = (Vec::new(), Vec::new(), Vec::new());
    for c in proposees {
        if garder_ids.contains(&c.id) {
            lacher(&c.racine);
            match garder(inst, &c.racine) {
                Ok(r) => gardees.push(Gardee { id: r.id, nom: r.nom }),
                Err(m) => erreurs.push(m),
            }
        } else if ne_pas_garder.contains(&c.id) {
            match supprimer(inst, &c.racine, None, poste, lacher) {
                Ok(nom) => supprimees.push(nom),
                Err(m) => erreurs.push(m),
            }
        }
    }
    Ok(Reponse { gardees, supprimees, erreurs })
}

/// Une compagnie gardee : son identifiant de tuile, son nom.
#[derive(Debug, serde::Serialize)]
pub struct Gardee {
    pub id: String,
    pub nom: String,
}

/// LA REPONSE AU DEBLOCAGE (`repondre`) : les compagnies gardees, les noms des supprimees, et chaque echec en phrase.
#[derive(Debug, serde::Serialize)]
pub struct Reponse {
    pub gardees: Vec<Gardee>,
    pub supprimees: Vec<String>,
    pub erreurs: Vec<String>,
}

// --------------------------------------------------------- la mention de demo

/// `config/demo.json` : quand la compagnie a ete creee en demo et gardee, et les mois saisis en demo qu'elle n'a pas
/// retravailles depuis. `garder` l'ecrit, `sortir_de_la_demo` le reecrit, `lire_la_demo` seule le lit.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct FichierDemo {
    creee_le: String,
    gardee_le: String,
    mois: Vec<String>,
    quoi: String,
}

const QUOI_DEMO: &str = "Cette compagnie a ete creee dans la version demo de PuppetPaie, puis gardee au deblocage : rien de ce qui y a ete fait avant n'est parti (ni DPAE, ni contrat a la signature, ni DSN, ni document).";

/// `config/demo.json`, lu par sa forme ; `None` pour une compagnie qui n'a pas ete creee en demo.
fn lire_la_demo(racine: &Path) -> Result<Option<FichierDemo>, String> {
    let p = racine.join(FICHIER_DEMO);
    if !p.is_file() {
        return Ok(None);
    }
    socle::forme::lire_fichier(&p, "La mention de démo de la compagnie (config/demo.json) ne se lit pas.").map(Some)
}

/// LA MENTION DE DEMO d'une compagnie gardee, telle que la page la montre : quand elle a ete creee et gardee, les mois
/// saisis en demo, et la phrase.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MentionDemo {
    creee_le: String,
    gardee_le: String,
    mois: Vec<String>,
    phrase: String,
}

/// La mention de demo de la compagnie ; `None` : une compagnie qui n'a pas ete creee en demo.
pub fn mention(racine: &Path) -> Result<Option<MentionDemo>, String> {
    let Some(d) = lire_la_demo(racine)? else { return Ok(None) };
    let jour = |t: &str| envois::etats::quand_lisible(&t.chars().take(10).collect::<String>());
    let (creee, gardee) = (jour(&d.creee_le), jour(&d.gardee_le));
    let depuis = if creee.is_empty() { String::new() } else { format!(" le {}", creee) };
    Ok(Some(MentionDemo {
        phrase: format!(
            "Compagnie créée en démo{}, gardée le {}. Les paies faites en démo n'ont été ni déclarées ni envoyées par PuppetPaie : aucune DPAE, aucun contrat envoyé à la signature, aucune DSN déposée, aucun document envoyé.",
            depuis, gardee
        ),
        creee_le: d.creee_le,
        gardee_le: d.gardee_le,
        mois: d.mois,
    }))
}

/// UN MOIS DE DEMO RETRAVAILLE apres que la compagnie a ete gardee (sa saisie enregistree, une paie validee ou modifiee :
/// `engagement::stock::poser_la_saisie`, `ecrire_la_carte`) est un mois qu'on entend
/// declarer : il sort de la mention (`config/demo.json`, `mois`), et la page ne le dit plus « fait en demo ». Rien a faire
/// pour une compagnie sans mention, ni pour un mois qui n'y est pas.
pub fn sortir_de_la_demo(racine: &Path, mois: &str) -> Result<(), String> {
    let Some(mut d) = lire_la_demo(racine)? else { return Ok(()) };
    let avant = d.mois.len();
    d.mois.retain(|m| m != mois);
    if d.mois.len() == avant {
        return Ok(());
    }
    socle::ecriture::ecrire_json(&racine.join(FICHIER_DEMO), &d, 2)
}
