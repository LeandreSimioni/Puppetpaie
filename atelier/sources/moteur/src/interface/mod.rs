//! La fenetre de PuppetPaie : le serveur local qui sert la page, l'accueil des compagnies et l'instance unique du
//! poste.
//!
//! UN SERVEUR LOCAL, TROIS ETATS. Le serveur (127.0.0.1, un jeton exige a chaque appel) sert `/` selon l'etat :
//!   - l'ACCUEIL : les tuiles des compagnies ;
//!   - le MOT DE PASSE de la compagnie choisie (ou son choix, a la premiere ouverture) ; une fictive n'en a pas ;
//!   - la COMPAGNIE OUVERTE : la page du logiciel (`moteur/interface/`), ses appels `/api/<route>` (`api.rs`) et la veille.
//! Les pages de l'accueil appellent `window.pywebview.api.<nom>(...)`, comme dans la fenetre pywebview : une cale
//! (`CALE`) poste ces appels a `/lanceur/<nom>`. La page ne sait pas dans quoi elle tourne.
//!
//! LA FENETRE. Tauri (`fenetre/`, le binaire `puppetpaie`) montre `adresse()` dans une vraie fenetre ; `moteur interface`
//! l'ouvre dans un Chromium en mode application, sinon dans le navigateur par defaut (`navigateur.rs`).

pub mod api;
pub mod http;
pub mod navigateur;
pub mod veille;

use std::collections::HashMap;
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use http::{Reponse, Requete};
use routes::accueil::suppression;
use compagnie::installation::{self, JOURNAL_INTERFACE};
use compagnie::instance::{self, Instance, Prise};
use compagnie::poste;
use routes::etat_garde::{Fil, FileDesEtats};
use routes::veille::EtatDeLaVeille;
use veille::Veille;
use routes::catalogue::Route;
use routes::enveloppe::{Charge, Refus, Rendu, Reponse as RouteReponse};
use socle::echec::{Classe, R};
use socle::memoire::tenu;

pub const MINUTES_VEILLE: u64 = 20;

const MOT_DE_PASSE: &str = include_str!("pages/mot_de_passe.html");
const CHOIX_MOT_DE_PASSE: &str = include_str!("pages/choix_mot_de_passe.html");
const ACCUEIL: &str = include_str!("pages/accueil.html");
const CRAYON: &str = include_str!("pages/crayon.html");
const ZERO: &str = include_str!("pages/zero.html");
const TUILE_AJOUTER: &str = include_str!("pages/tuile_ajouter.html");
const AVIS_AJOUTER: &str = include_str!("pages/avis_ajouter.html");
const GARDER: &str = include_str!("pages/garder.html");

/// Les appels `window.pywebview.api.<nom>(...)` de l'accueil, postes au serveur. Une reponse qui dit `recharger`
/// fait revenir la fenetre sur `/` : c'est le serveur qui sait quelle page montrer ensuite.
const CALE: &str = r#"<script>
(function () {
  const JETON = "{{JETON}}";
  // Une reponse se lit toujours en `{ok, resultat}` ou `{ok: false, refus: {phrase}}`, comme le moteur la rend : un corps
  // qui n'est pas du JSON ou un serveur parti ne levent pas dans la page.
  const lire = (r) => r.text().then((t) => {
    try { return JSON.parse(t); }
    catch (e) { return { ok: false, refus: { phrase: "PuppetPaie a répondu " + r.status + " sans dire pourquoi." } }; }
  });
  const parti = () => ({ ok: false, refus: { phrase: "PuppetPaie ne répond plus : relancez le logiciel." } });
  const appel = (nom, args) => fetch("/lanceur/" + nom, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Jeton": JETON },
      body: JSON.stringify(args),
    }).then(lire, parti)
      .then((d) => {
        if (d && d.recharger) location.replace("/");
        return d;
      });
  const api = {};
  for (const nom of {{NOMS}}) api[nom] = (...args) => appel(nom, args);
  window.pywebview = { api };
  // UNE ROUTE DU CATALOGUE, par la porte de toute la page (`/api/<route>`) : la reponse du moteur, telle quelle.
  window.appel = (nom, charge) => fetch("/api/" + nom, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Jeton": JETON },
      body: JSON.stringify(charge),
    }).then(lire, parti);
  document.addEventListener("DOMContentLoaded", () => window.dispatchEvent(new Event("pywebviewready")));
})();
</script>
"#;

/// LES GESTES DE LA FENETRE ELLE-MEME (`window.pywebview.api.<nom>`, servis par `Fenetre::lanceur`) : aucun n'est un geste
/// du logiciel, chacun dit ce que la fenetre montre ou fait d'elle-meme. Tout le reste passe par la porte de la page, les
/// routes du catalogue (`window.appel`, `/api/<route>`). A l'accueil et aux pages du mot de passe :
/// - `choisir` : montrer la compagnie d'une tuile (sa page, ou d'abord son mot de passe) ; `verifier` : la deverrouiller dans
///   cette fenetre par son mot de passe (en ligne de commande, `--racine` et la seance du coffre) ; `accueil` : revenir
///   aux tuiles ;
/// - `fermer`, `redemarrer` : fermer la fenetre, la rouvrir sur la version prete ; `maj` : ou en est la recherche que la
///   fenetre mene en fond (la version, elle, est la route `maj-etat`) ; `site` : ouvrir dans le navigateur du poste le site
///   de l'editeur (le lien de l'auteur, `poste::ouvrir_site`, qui refuse toute autre adresse).
const NOMS_ACCUEIL: &str = r#"["choisir", "verifier", "accueil", "fermer", "redemarrer", "maj", "site"]"#;
/// Dans la page d'une compagnie : revenir aux compagnies, la mise a jour (`js/mise-a-jour.js`), ouvrir ce qu'une route a
/// rendu et choisir un dossier (`poste.rs`), le site de l'editeur (`site`), la veille de la fenetre (`js/veille.js`).
const NOMS_LOGICIEL: &str = r#"["accueil", "maj", "redemarrer", "site", "ouvrir", "choisir_dossier", "veille", "veille_etape", "veille_maintenant"]"#;

/// UN ARGUMENT D'UN GESTE DE LA FENETRE (`window.pywebview.api.<nom>(...)`, poste en liste) : un texte, ou un oui-non.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Argument {
    Texte(String),
    OuiNon(bool),
}

/// Le texte `i` des arguments d'un geste, ou le refus du geste qui ne le porte pas.
fn texte(args: &[Argument], i: usize) -> R<&str> {
    match args.get(i) {
        Some(Argument::Texte(t)) => Ok(t),
        Some(Argument::OuiNon(_)) | None => Err(socle::refus!("Ce geste de la fenêtre attend un texte en argument {}.", i + 1)),
    }
}

/// Le oui-non `i` des arguments d'un geste, ou le refus du geste qui ne le porte pas.
fn oui_non(args: &[Argument], i: usize) -> R<bool> {
    match args.get(i) {
        Some(Argument::OuiNon(b)) => Ok(*b),
        Some(Argument::Texte(_)) | None => Err(socle::refus!("Ce geste de la fenêtre attend un oui ou un non en argument {}.", i + 1)),
    }
}

/// CE QUE REND UN GESTE DE LA FENETRE (`Fenetre::lanceur`), lu par la page comme une route : `ok` et ce que le geste dit,
/// ou son refus.
#[derive(Serialize)]
#[serde(untagged)]
enum Geste {
    /// Fait ; `recharger` : la fenetre revient sur `/`, le serveur sait quelle page montrer.
    Fait { ok: bool, recharger: bool },
    Dit { ok: bool, phrase: String },
    /// Le dossier choisi ; aucun quand la boite a ete fermee.
    Dossier { ok: bool, dossier: Option<String> },
    Veille { ok: bool, etat: Box<EtatDeLaVeille> },
    Etape { ok: bool, etat: veille::EtapeDeLaVeille },
    Maj { ok: bool, etat: compagnie::maj::EtatDeLaRecherche },
    Refuse { ok: bool, refus: Refus },
}

/// CE QUE REND UN GESTE DU BANC D'ESSAI (`Fenetre::atelier`).
#[cfg(feature = "atelier")]
#[derive(Serialize)]
#[serde(untagged)]
enum Atelier {
    Etat {
        ok: bool,
        simule: bool,
        jour: socle::date::Jour,
        heure: socle::paris::InstantEcrit,
        secondes: socle::paris::Moment,
        scenario: Option<String>,
        compagnie: Option<String>,
        simulee: bool,
        ajoutee: bool,
    },
    Jour {
        ok: bool,
        jour: socle::date::Jour,
        #[serde(skip_serializing_if = "Option::is_none")]
        veille: Option<EtatDeLaVeille>,
    },
    Fait { ok: bool },
    Refuse { ok: bool, refus: Refus },
}

/// La charge du geste `jour` du banc.
#[cfg(feature = "atelier")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeJour {
    jour: socle::date::Jour,
}

#[derive(Clone)]
enum Phase {
    Accueil,
    MotDePasse { racine: PathBuf, premiere: bool },
    Ouverte { racine: PathBuf, veille: Arc<Veille>, etats: Arc<FileDesEtats> },
}

pub struct Fenetre {
    jeton: String,
    /// Le port de la page : le cookie du jeton porte son nom (`jeton-<port>`), car un cookie ne connait pas le port --
    /// deux installations ouvertes dans le meme navigateur ne s'ecrasent pas leur jeton.
    port: u16,
    /// Le dossier de cette fenetre sur le poste (`navigateur::dossier_local`) : le jeton, le profil du navigateur, le
    /// journal d'avant toute compagnie.
    local: PathBuf,
    /// LE FIL DES CALCULS EN FOND (`etat_garde::Fil`) et la file de chaque compagnie qu'il tient, par
    /// dossier : nees au lancement pour les compagnies de l'installation (`prechauffer`), ou a l'ouverture d'une autre ;
    /// reprises, jamais refaites, quand la compagnie s'ouvre, se ferme et se rouvre.
    fil: Arc<Fil>,
    files: Mutex<HashMap<PathBuf, Arc<FileDesEtats>>>,
    phase: Mutex<Phase>,
    /// LE MOT DE PASSE DE L'ADMINISTRATEUR, tape une fois dans cette fenetre : il ouvre ses autres
    /// compagnies sans le redemander tant que la fenetre vit. Il OUVRE, il n'autorise rien : un acte reel redemande
    /// toujours le mot de passe au moment de l'acte. Il ne va dans aucune route, aucun fichier, aucun journal.
    administrateur: Mutex<Option<String>>,
    temoin: Mutex<Instant>,
    journal: Mutex<Option<PathBuf>>,
    minutes_veille: u64,
    /// Ce que fait « fermer » : arreter le processus (le navigateur), ou fermer la fenetre Tauri.
    pub fermer: Mutex<Option<Box<dyn Fn() + Send>>>,
}

/// La cle d'une compagnie dans la fenetre : son dossier resolu ; un dossier qui ne se resout plus (efface, deplace) se
/// nomme par son chemin, qui le designe encore.
fn cle_du_dossier(p: &Path) -> PathBuf {
    match std::fs::canonicalize(p) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => p.to_path_buf(),
        Err(e) => {
            eprintln!("{}", socle::echec::du_disque(&p, &e).en_lignes());
            p.to_path_buf()
        }
    }
}

fn echapper(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#x27;")
}

/// La version telle que la fenetre la montre : `JJ.MM.AAAA` (`maj::version_lue`) ; le fichier garde l'identifiant.
/// Le binaire de l'atelier n'est pas une version publiee : il dit de quelles sources il est fait (le commit, `build.rs`).
#[cfg(feature = "atelier")]
fn version() -> String {
    let commit = env!("PAIE_COMMIT");
    if commit.is_empty() {
        "de développement".to_string()
    } else {
        format!("de développement, {}", commit)
    }
}

#[cfg(not(feature = "atelier"))]
fn version() -> String {
    let p = installation::logiciel().join("VERSION");
    match std::fs::read_to_string(&p) {
        Ok(v) => compagnie::maj::version_lue(&v),
        Err(e) => format!("illisible ({})", socle::echec::du_disque(&p, &e).phrase),
    }
}

fn ressources() -> PathBuf {
    installation::logiciel().join("interface")
}

/// Douze caracteres qui changent des qu'une feuille ou un script change : la fenetre ne garde rien de perime.
/// Tous les `.css` et `.js` de `interface/` et de `interface/js/`, tries par nom : la meme empreinte d'un
/// lancement a l'autre. Le nom entre dans le condensat avec le contenu.
fn empreinte() -> String {
    use sha2::{Digest, Sha256};
    use std::sync::OnceLock;
    /// La date et la taille d'un fichier, ou ce qui l'empeche de se lire.
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
    enum Signature {
        Lue(Option<(u128, u64)>),
        Illisible(String),
    }
    let mut noms: Vec<(String, Signature)> = Vec::new();
    // Un dossier qui ne se lit pas entre dans l'empreinte par son erreur : la page se refera quand il se lira.
    const DOSSIERS: [&str; 2] = ["", "js"];
    for dossier in DOSSIERS {
        let it = match std::fs::read_dir(ressources().join(dossier)) {
            Ok(it) => it,
            Err(e) => {
                noms.push((format!("{}/", dossier), Signature::Illisible(e.to_string())));
                continue;
            }
        };
        for e in it.flatten() {
            let nom = e.file_name().to_string_lossy().into_owned();
            if matches!(e.file_type(), Ok(t) if t.is_file()) && (nom.ends_with(".css") || nom.ends_with(".js")) {
                let nom = if dossier.is_empty() { nom } else { format!("{}/{}", dossier, nom) };
                let sig = match socle::fichiers::signature(&ressources().join(&nom)) {
                    Ok(s) => Signature::Lue(s),
                    Err(e) => Signature::Illisible(e.en_lignes()),
                };
                noms.push((nom, sig));
            }
        }
    }
    noms.sort();
    // LE CONTENU NE SE RELIT QUE S'IL A CHANGE : la date et la taille de chaque fichier suffisent a savoir
    // que rien n'a bouge depuis la derniere page -- la relecture et le condensat de 800 Ko a chaque `GET /` coutaient.
    static DERNIERE: OnceLock<Mutex<Option<(Vec<(String, Signature)>, String)>>> = OnceLock::new();
    let memo = DERNIERE.get_or_init(|| Mutex::new(None));
    if let Some((vus, e)) = tenu(memo).as_ref() {
        if *vus == noms {
            return e.clone();
        }
    }
    let mut h = Sha256::new();
    for (nom, _) in &noms {
        h.update(nom.as_bytes());
        h.update([0u8]);
        match std::fs::read(ressources().join(nom)) {
            Ok(o) => {
                h.update((o.len() as u64).to_le_bytes());
                h.update(&o);
            }
            Err(e) => h.update(e.to_string().as_bytes()),
        }
    }
    let e: String = h.finalize().iter().take(6).map(|b| format!("{:02x}", b)).collect();
    *tenu(memo) = Some((noms, e.clone()));
    e
}

/// Un fichier de la page (`interface/`), servi tel quel. Le chemin arrive decode : il n'est pris que s'il est fait de
/// noms ordinaires (ni `..`, ni `.`, ni racine, ni lecteur, ni `\`, ni `:`), d'une extension connue, et si, resolu,
/// il est un fichier SOUS `interface/`. Tout le reste est un 404 (`Ok(None)`) ; un fichier qui est la mais ne se lit
/// pas, l'erreur du disque.
fn fichier_de_la_page(chemin: &str) -> R<Option<(Vec<u8>, &'static str)>> {
    use std::path::Component;
    let Some(rel) = chemin.strip_prefix('/') else { return Ok(None) };
    if rel.is_empty() || rel.contains('\\') || rel.contains(':') || rel.contains('\0') {
        return Ok(None);
    }
    if rel.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return Ok(None);
    }
    let rel = Path::new(rel);
    if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return Ok(None);
    }
    let mime = match rel.extension().and_then(|e| e.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("html") => "text/html; charset=utf-8",
        _ => return Ok(None),
    };
    let present = |p: &Path| match std::fs::canonicalize(p) {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(socle::echec::du_disque(&p, &e)),
    };
    let Some(racine) = present(&ressources())? else { return Ok(None) };
    let Some(vrai) = present(&racine.join(rel))? else { return Ok(None) };
    if !vrai.starts_with(&racine) || !vrai.is_file() {
        return Ok(None);
    }
    std::fs::read(&vrai).map(|o| Some((o, mime))).map_err(|e| socle::echec::du_disque(&vrai, &e))
}

/// Le jeton de cette fenetre, tire une fois et garde dans son dossier sur le poste (`navigateur::dossier_local`).
/// Un jeton garde d'une autre forme se tire a neuf. UN TIRAGE QUI ECHOUE REFUSE : la fenetre ne demarre pas avec un jeton
/// previsible.
fn jeton(local: &Path) -> R<String> {
    let p = local.join(".jeton-interface");
    match std::fs::read_to_string(&p) {
        Ok(j) => {
            let j = j.trim();
            if j.len() >= 24 && j.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return Ok(j.to_string());
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(socle::echec::du_disque(&p, &e)),
    }
    let mut o = [0u8; 24];
    getrandom::fill(&mut o).map_err(|e| socle::refus!("Le jeton de la fenetre ne se tire pas au hasard : {}", e))?;
    let j: String = o.iter().map(|b| format!("{:02x}", b)).collect();
    std::fs::create_dir_all(local).map_err(|e| socle::echec::du_disque(local, &e))?;
    std::fs::write(&p, &j).map_err(|e| socle::echec::du_disque(&p, &e))?;
    Ok(j)
}

/// Un JSON pose dans la page, dans un `<script type="application/json">` : rien ne peut y fermer la balise.
fn json_dans_la_page(v: &impl serde::Serialize) -> String {
    socle::ecriture::json_en_ligne(v).replace('<', "\\u003c").replace('>', "\\u003e").replace('&', "\\u0026")
}

fn page(corps: &str, jeton: &str, noms: &str) -> String {
    // Ce que l'accueil et les pages du mot de passe disent : les phrases de la fenetre (`{{P:...}}`), la page n'en ecrit aucune.
    let corps = PHRASES_DE_L_ACCUEIL.iter().fold(corps.to_string(), |c, (cle, phrase)| c.replace(&format!("{{{{P:{}}}}}", cle), phrase));
    format!(
        "<!doctype html>\n<html lang=\"fr\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>PuppetPaie</title>\n{}</head><body>\n{}\n</body></html>",
        CALE.replace("{{JETON}}", jeton).replace("{{NOMS}}", noms),
        corps
    )
}

/// Le logo d'une tuile, pose dans la page ; un logo qui ne se lit pas laisse l'initiale, et le dit sur la sortie d'erreur.
fn logo_en_ligne(logo: &Path) -> Option<String> {
    let mime = if logo.extension().is_some_and(|e| e == "svg") { "image/svg+xml" } else { "image/png" };
    match std::fs::read(logo) {
        Ok(o) => Some(format!("data:{};base64,{}", mime, socle::base64::ecrire(&o))),
        Err(e) => {
            eprintln!("Un logo de l'accueil ne se lit pas : {}", socle::echec::du_disque(&logo, &e).en_lignes());
            None
        }
    }
}

/// L'accueil : les tuiles des compagnies, telles que la route `compagnies` les dit (`routes::accueil::liste`).
fn page_accueil() -> R<String> {
    let r = routes::accueil::liste(&installation::base())?;
    let mut tuiles = Vec::new();
    for c in &r.compagnies {
        let medaillon = match c.logo.as_deref().map(Path::new).and_then(logo_en_ligne) {
            Some(logo) => format!("<img src=\"{}\" alt=\"\">", logo),
            None => format!("<span class=\"initiale\">{}</span>", echapper(&c.nom.chars().take(1).collect::<String>())),
        };
        let marque = if c.reelle {
            CRAYON.replace("%s", &c.couleur)
        } else if c.remise_a_zero {
            ZERO.to_string()
        } else {
            String::new()
        };
        tuiles.push((
            c.reelle,
            format!(
                "<div class=\"tuile{}\" data-id=\"{}\"{}><div class=\"tete\"><div class=\"medaillon\">{}</div>{}</div><div class=\"corps\"><span class=\"nom\">{}</span><span class=\"lieu\">{}</span></div></div>",
                if c.reelle { "" } else { " fictive" },
                echapper(&c.id),
                if c.reelle { format!(" style=\"--cie:{}\"", c.couleur) } else { String::new() },
                medaillon,
                marque,
                echapper(&c.nom),
                echapper(&c.lieu)
            ),
        ));
    }
    let bride = r.bride;
    let grille = if bride {
        // BRIDE, LE « + » AUSSI : il ajoute une fictive (la route `compagnie-ajouter`), apres le message.
        format!("<div class=\"grille\">{}{}</div>{}", tuiles.iter().map(|(_, t)| t.as_str()).collect::<String>(), TUILE_AJOUTER, AVIS_AJOUTER)
    } else {
        let reelles: String = tuiles.iter().filter(|(r, _)| *r).map(|(_, t)| t.as_str()).collect::<String>() + TUILE_AJOUTER;
        let fictives: Vec<&str> = tuiles.iter().filter(|(r, _)| !*r).map(|(_, t)| t.as_str()).collect();
        let mut g = format!("<div class=\"grille\">{}</div>", reelles);
        if !fictives.is_empty() {
            g += &format!(
                "<details class=\"fictives\"><summary>Compagnies fictives ({}) &mdash; pour essayer, rien ne part</summary><div class=\"grille\">{}</div></details>",
                fictives.len(),
                fictives.concat()
            );
        }
        g
    };
    // AU DEBLOCAGE : tant qu'il reste des compagnies creees en demo, la question se pose a chaque accueil.
    let grille = if r.a_garder.is_empty() {
        grille
    } else {
        let liste: String = r
            .a_garder
            .iter()
            .map(|c| format!("<li><label><input type=\"checkbox\" value=\"{}\" checked> {}</label></li>", echapper(&c.id), echapper(&c.nom)))
            .collect();
        format!("{}{}", grille, GARDER.replace("{{LISTE}}", &liste))
    };
    // La version, sous le nom, comme dans la page d'une compagnie ; les tuiles en dernier : un nom de
    // compagnie n'est jamais lu comme une marque a remplacer.
    Ok(ACCUEIL
        .replace("{{VERSION_DITE}}", &format!("version {}", version()))
        .replace("{{AUTEUR}}", &json_dans_la_page(&r.auteur))
        .replace("{{EMPREINTE}}", &empreinte())
        .replace(
            "{{SOUS_TITRE}}",
            if bride { "des compagnies fictives, et rien ne part" } else { "quelle compagnie ouvrir ?" },
        )
        .replace("{{TUILES}}", &grille))
}

/// La page du logiciel, telle que la fenetre la recoit (publique pour `tests/interface_fichiers.rs`).
pub fn page_logiciel(racine: &Path, jeton: &str) -> R<String> {
    let brut = std::fs::read_to_string(ressources().join("index.html")).map_err(|e| socle::echec::du_disque(&ressources().join("index.html"), &e))?;
    let copie = installation::est_copie(racine);
    let mut p = brut
        .replace("{{JETON}}", jeton)
        .replace("{{EMPREINTE}}", &empreinte())
        .replace("{{COPIE}}", if copie { "1" } else { "" })
        .replace("{{COPIE_TEXTE}}", &installation::texte_copie(racine)?)
        .replace(
            "{{STYLE_COMPAGNIE}}",
            &if copie { String::new() } else { format!("<style>:root {{ --compagnie: {}; }}</style>", installation::couleur(racine)?) },
        )
        .replace("{{PHRASES}}", &socle::ecriture::json_en_ligne(&PHRASES_DE_LA_FENETRE))
        .replace("{{VERSION_DITE}}", &format!("version {}{}", version(), if compagnie::bridage::est_bride() { " &middot; d&eacute;mo" } else { "" }));
    let cale = CALE.replace("{{JETON}}", jeton).replace("{{NOMS}}", NOMS_LOGICIEL);
    // Avant le PREMIER script de la page (`/js/tuyau.js`, qui definit `api`) : la cale doit exister avant lui.
    p = p.replacen("<script src=\"/js/", &format!("{}<script src=\"/js/", cale), 1);
    Ok(p)
}

impl Fenetre {
    pub fn nouvelle(minutes_veille: u64, local: PathBuf, port: u16) -> R<Arc<Fenetre>> {
        Ok(Arc::new(Fenetre {
            jeton: jeton(&local)?,
            port,
            local,
            fil: Fil::nouveau(),
            files: Mutex::new(HashMap::new()),
            phase: Mutex::new(Phase::Accueil),
            administrateur: Mutex::new(None),
            temoin: Mutex::new(Instant::now()),
            journal: Mutex::new(None),
            minutes_veille,
            fermer: Mutex::new(None),
        }))
    }

    pub fn ecrire(&self, texte: &str) {
        // Aucune compagnie ouverte encore : le journal de la fenetre est dans son dossier sur le poste.
        let chemin = match tenu(&self.journal).clone() {
            Some(j) => j,
            None => self.local.join("interface.log"),
        };
        // Le journal de la fenetre n'a personne a qui dire qu'il ne s'ecrit pas : l'echec est note a sa naissance
        // (`echec::noter_echec`, que les incidents lisent), et la fenetre continue.
        if let Err(note) = ajouter_au_journal(&chemin, texte) {
            drop(note);
        }
    }

    fn phase(&self) -> Phase {
        tenu(&self.phase).clone()
    }

    /// La phase change. La compagnie qu'on quitte arrete sa veille ; sa file reste au fil (elle se rouvrira sans attendre),
    /// et ne passe plus devant les autres. Revenir a l'accueil : aucune compagnie devant.
    fn poser_phase(&self, p: Phase) {
        let accueil = matches!(p, Phase::Accueil);
        let avant = std::mem::replace(&mut *tenu(&self.phase), p);
        if let Phase::Ouverte { veille, .. } = avant {
            veille.arreter();
        }
        if accueil {
            self.fil.devant(None);
        }
    }

    /// Le dossier de cette fenetre sur le poste (`navigateur::dossier_local`).
    pub fn dossier_local(&self) -> &Path {
        &self.local
    }

    /// La file de la compagnie `racine` sur le fil de la fenetre : celle qu'il tient deja, sinon une neuve.
    fn file_de(&self, racine: &Path) -> Arc<FileDesEtats> {
        let cle = cle_du_dossier(racine);
        let mut files = tenu(&self.files);
        files.entry(cle).or_insert_with(|| self.fil.file(racine, None)).clone()
    }

    /// LES COMPAGNIES DE L'INSTALLATION, TENUES A JOUR DES LE LANCEMENT : n'importe quelle compagnie s'ouvre
    /// instantanement, meme a la premiere ouverture du jour. Chaque vraie compagnie de l'installation -- celles de
    /// l'administrateur, et celles qui ont garde leur mot de passe : toutes sont sur ce disque, en clair -- recoit sa file
    /// sur le fil de la fenetre, qui verifie et refait en fond son etat d'ouverture, son tableau (une compagnie de
    /// l'administrateur), puis ses mois. La derniere ouverte d'abord (la date de son journal de la fenetre, que chaque
    /// ouverture recommence), puis chacune son tour.
    ///
    /// AVANT TOUT MOT DE PASSE : du calcul local seulement. Le fil ne sort pas et n'ouvre aucun coffre (`fil_de_fond`) ;
    /// la veille (le reseau, le coffre, la releve, la sauvegarde) ne demarre qu'a l'ouverture d'une compagnie, apres son
    /// mot de passe (`ouvrir`). Les donnees sont en clair sur le disque : ce calcul ne revele rien que le disque ne montre
    /// deja. Les compagnies fictives ne sont pas prechauffees : elles s'ouvrent sur leur dernier etat garde, et le fil les
    /// prend a leur ouverture. Bride, rien.
    pub fn prechauffer(self: &Arc<Self>) {
        if compagnie::bridage::est_bride() {
            return;
        }
        let f = self.clone();
        routes::fil_de_routes(move || {
            let reelles = match installation::a_tenir_a_jour(&installation::base()) {
                Ok(r) => r,
                Err(e) => {
                    f.ecrire(&format!("Calcul en fond non lance : {}\n", e.phrase));
                    return;
                }
            };
            for r in &reelles {
                f.file_de(r).tout_verifier();
            }
            let noms: Vec<String> = reelles.iter().map(|r| r.display().to_string()).collect();
            f.ecrire(&format!("Calcul en fond, sans rien ouvrir : {} compagnie(s) a tenir a jour ({}).\n", reelles.len(), noms.join(", ")));
        });
    }

    /// Le nom du cookie du jeton : un par port (un cookie ne distingue pas les ports d'un meme hote).
    fn nom_du_cookie(&self) -> String {
        format!("jeton-{}", self.port)
    }

    /// Les secondes depuis la derniere requete autorisee (le guet du navigateur : plus de page, plus de serveur).
    pub fn silence(&self) -> Duration {
        tenu(&self.temoin).elapsed()
    }

    /// La compagnie choisie devient celle de la fenetre ; une fictive s'ouvre sans mot de passe.
    fn prendre(&self, racine: PathBuf) -> R<()> {
        if let Err(e) = installation::figer_origine(&racine) {
            self.ecrire(&format!("Photo d'origine non prise : {}\n", e.en_lignes()));
        }
        // Aucune variable du processus ne dit la compagnie : chaque route la recoit (`--racine`, `o["racine"]`,
        // le `Contexte`), et la fenetre pourra en servir plusieurs a la fois.
        let journal = racine.join(JOURNAL_INTERFACE);
        if let Some(d) = journal.parent() {
            std::fs::create_dir_all(d).map_err(|e| socle::echec::du_disque(d, &e))?;
        }
        std::fs::write(&journal, "").map_err(|e| socle::echec::du_disque(&journal, &e))?;
        *tenu(&self.journal) = Some(journal);
        self.ecrire(&format!("Compagnie : {}\n", racine.display()));
        // La compagnie choisie passe devant les autres sur le fil des le choix : son calcul local se fait pendant qu'on
        // tape son mot de passe.
        let etats = self.file_de(&racine);
        self.fil.devant(Some(&etats));
        etats.tout_verifier();
        if compagnie::bridage::est_bride() || installation::est_fictive(&racine)? {
            self.ouvrir(racine);
            return Ok(());
        }
        let exigee = compagnie::coffre::Coffre::du_poste(&racine).ouverture_exigee()?;
        // UNE COMPAGNIE DE L'ADMINISTRATEUR, quand son mot de passe a deja ete tape dans cette fenetre : elle s'ouvre sans
        // le redemander (il ouvre encore son coffre : c'est verifie, pas suppose).
        if exigee && self.administrateur_ouvre(&racine) {
            self.ecrire("Ouverte par le mot de passe de l'administrateur, deja tape dans cette fenetre.\n");
            self.ouvrir(racine);
            return Ok(());
        }
        self.poser_phase(Phase::MotDePasse { racine, premiere: !exigee });
        Ok(())
    }

    /// LA FENETRE LACHE LA COMPAGNIE `racine`, qui va etre supprimee ou deplacee (`suppression.rs`) : ouverte, sa veille
    /// s'arrete (le tour en cours se finit) et l'on revient a l'accueil, son journal n'est plus celui de la fenetre ; sa
    /// file quitte le fil des calculs (le calcul en cours se finit). Ensuite rien de la fenetre n'ecrit plus dans son
    /// dossier, et rien ne la refait apparaitre.
    fn lacher(&self, racine: &Path) {
        let cle = cle_du_dossier(racine);
        if let Phase::Ouverte { racine: r, veille, .. } = self.phase() {
            if cle_du_dossier(&r) == cle {
                veille.arreter_et_attendre();
                self.poser_phase(Phase::Accueil);
            }
        }
        // Le journal de la fenetre est celui de la derniere compagnie ouverte, meme revenu a l'accueil : il ne doit plus
        // recreer son dossier.
        let mut journal = tenu(&self.journal);
        if journal.as_ref().is_some_and(|j| j.starts_with(racine) || j.starts_with(&cle)) {
            *journal = None;
        }
        drop(journal);
        let file = tenu(&self.files).remove(&cle);
        if let Some(f) = file {
            f.quitter();
        }
    }

    /// Le mot de passe de l'administrateur, tape dans cette fenetre, ouvre-t-il `racine`, compagnie rattachee ?
    fn administrateur_ouvre(&self, racine: &Path) -> bool {
        let inst = installation::base();
        // Un registre ou un coffre qui ne se lit pas n'ouvre rien : le mot de passe se demande, et son refus le dit.
        if !matches!(routes::gestes::administrateur::est_rattachee(&inst, racine), Ok(true)) {
            return false;
        }
        let garde = tenu(&self.administrateur).clone();
        garde.is_some_and(|m| matches!(routes::gestes::administrateur::mot_de_passe_ouvre(&routes::gestes::administrateur::Poste::de_ce_poste(), racine, &m), Ok(true)))
    }

    /// Retient le mot de passe de l'administrateur pour la fenetre, s'il ouvre une compagnie rattachee.
    fn retenir_administrateur(&self, racine: &Path, motdepasse: &str) {
        if matches!(routes::gestes::administrateur::est_rattachee(&installation::base(), racine), Ok(true)) {
            *tenu(&self.administrateur) = Some(motdepasse.to_string());
        }
    }

    fn ouvrir(&self, racine: PathBuf) {
        // La mise a jour ne se fait pas ici : le logiciel la cherche au lancement (`maj::chercher_en_fond`) et l'applique
        // au lancement suivant, avant toute fenetre (`maj::au_lancement`), sans qu'aucune compagnie soit ouverte.
        // Les etats des mois se refont en fond, dans ce processus, s'ils ne sont plus justes (`etat_garde::FileDesEtats`) :
        // a l'ouverture, apres chaque ecriture et chaque tour de veille. Un mois s'affiche ensuite sans calcul. La file est
        // celle que le fil de la fenetre tient deja pour cette compagnie (`prechauffer`) : elle passe devant.
        let etats = self.file_de(&racine);
        self.fil.devant(Some(&etats));
        etats.tout_verifier();
        let veille = Veille::nouvelle(&racine, self.minutes_veille, Some(etats.clone()));
        veille.demarrer();
        self.poser_phase(Phase::Ouverte { racine, veille, etats });
    }

    fn autorise(&self, r: &Requete) -> bool {
        let fourni = r.param("jeton").map(String::from).or_else(|| r.entete("x-jeton").map(String::from)).or_else(|| r.cookie(&self.nom_du_cookie()));
        let ok = fourni.is_some_and(|f| egal_en_temps_constant(f.as_bytes(), self.jeton.as_bytes()));
        if ok {
            *tenu(&self.temoin) = Instant::now();
        }
        ok
    }

    fn fermer_le_logiciel(&self) {
        if let Some(f) = tenu(&self.fermer).as_ref() {
            f();
        }
    }

    /// LES GESTES DU BANC D'ESSAI (l'atelier seulement ; `atelier/outils/banc/`) : `/atelier/<geste>`, avec le jeton.
    /// - `etat` : le jour simule, le scenario, la compagnie ouverte ;
    /// - `jour` `{"jour": "AAAA-MM-JJ"}` : le calendrier avance jusqu'a ce jour, 9 h (Paris) ; puis, si une compagnie est
    ///   ouverte, un passage de la veille, comme a l'ouverture : l'horloge d'une fictive aux envois simules joue ce qui
    ///   etait du, les retours des depots joues reviennent. Rend ce que la veille a fait ;
    /// - `simuler` : la compagnie ouverte, une fictive ajoutee par le « + », devient une fictive aux envois simules
    ///   (`scenario::rendre_simulee`).
    #[cfg(feature = "atelier")]
    fn atelier(&self, geste: &str, corps: &[u8]) -> R<Atelier> {
        let phase = self.phase();
        let ouverte = match &phase {
            Phase::Ouverte { racine, .. } => Some(racine.clone()),
            _ => None,
        };
        let enr = routes::scenario::enregistreur_actif();
        match geste {
            "etat" => Ok(Atelier::Etat {
                ok: true,
                simule: socle::temps::simule(),
                jour: socle::temps::jour_de_paris(),
                heure: socle::paris::InstantEcrit::maintenant(),
                secondes: socle::temps::secondes(),
                scenario: enr.map(|e| e.dossier().display().to_string()),
                compagnie: ouverte.as_ref().map(|r| r.display().to_string()),
                simulee: ouverte.as_deref().map(compagnie::bridage::branchement).transpose()?.is_some_and(|b| b.joue()),
                ajoutee: ouverte.as_deref().map(installation::est_ajoutee).transpose()?.is_some_and(|a| a),
            }),
            "jour" => {
                if !socle::temps::simule() {
                    return Err(socle::echec::refus("Le temps n'est pas simulé : relancer la fenêtre en lui donnant un jour."));
                }
                let texte = std::str::from_utf8(corps).map_err(|e| socle::echec::refus(e.to_string()))?;
                let ChargeJour { jour } = socle::forme::lire_texte(texte, "Il faut {\"jour\": \"AAAA-MM-JJ\"}.")?;
                let instant = socle::paris::Moment::a_paris(jour, 9, 0, 0).ok_or_else(|| socle::echec::refus(socle::date::HORS_CALENDRIER))?;
                if instant <= socle::temps::secondes() {
                    return Err(socle::refus!("Le {} n'est pas apres l'instant simule ({}) : le calendrier n'avance que vers demain.", jour.iso(), socle::paris::iso_secondes(socle::temps::secondes())));
                }
                socle::temps::couler_depuis(instant);
                if let Some(e) = enr {
                    // La compagnie ouverte passe sa veille : l'horloge joue dans la route `ouverture` qui suit, et
                    // seulement la (`scenario::rejouer`).
                    use routes::scenario::{Genre, Geste};
                    e.geste(|| Ok(((), Geste { jour: Some(jour), veille: ouverte.is_some().then_some(true), ..Geste::du_banc(Genre::Jour) })))?;
                }
                self.ecrire(&format!("Banc d'essai : le calendrier avance au {}.\n", jour.iso()));
                let veille = match phase {
                    Phase::Ouverte { veille, etats, .. } => {
                        let v = veille.tour(false, false);
                        etats.tout_verifier();
                        Some(v)
                    }
                    Phase::Accueil | Phase::MotDePasse { .. } => None,
                };
                Ok(Atelier::Jour { ok: true, jour, veille })
            }
            "simuler" => {
                let racine = ouverte.ok_or_else(|| socle::echec::refus("Aucune compagnie n'est ouverte."))?;
                use routes::scenario::{Genre, Geste};
                match enr {
                    Some(e) => e.geste(|| {
                        routes::scenario::rendre_simulee(&racine)?;
                        Ok(((), Geste { compagnie: Some(e.compagnie(&racine)?), ..Geste::du_banc(Genre::Simuler) }))
                    })?,
                    None => routes::scenario::rendre_simulee(&racine)?,
                }
                self.ecrire("Banc d'essai : la compagnie devient une fictive aux envois simules.\n");
                Ok(Atelier::Fait { ok: true })
            }
            _ => Err(socle::refus!("Geste d'atelier inconnu : {}.", geste)),
        }
    }

    /// UNE ROUTE DE L'INSTALLATION, demandee par la page (`routes::accueil`). La fenetre n'y ajoute que ce qui la regarde :
    /// avant que les compagnies creees en demo soient gardees ou supprimees, elle lache ce qu'elle en tient (`lacher`) ;
    /// le tableau de l'administrateur se lit, comme l'ecran d'un mois, dans ce que son fil a deja refait (`prechauffer`),
    /// et ne releve les comptes rendus (le reseau, le coffre) qu'une fois le mot de passe de l'administrateur tape ici.
    fn route_de_l_installation(&self, nom: Route, charge: &Charge) -> RouteReponse {
        let inst = installation::base();
        if nom == Route::CompagniesGarder {
            match suppression::a_garder(&inst) {
                Ok(cies) => cies.iter().for_each(|c| self.lacher(&c.racine)),
                Err(e) => return RouteReponse::de(nom, Err(e.reclasser(Classe::Refuse))),
            }
        }
        if nom == Route::AdministrateurTableau {
            let c: routes::accueil::ChargeTableau = match charge.lire() {
                Ok(c) => c,
                Err(e) => return RouteReponse::de(nom, Err(e)),
            };
            if c.garde == Some(true) || c.relever == Some(true) {
                return api::route_de_l_installation(nom, charge);
            }
            if tenu(&self.administrateur).is_some() {
                return match Charge::de_forme(&routes::accueil::ChargeTableau { relever: Some(true), garde: None }) {
                    Ok(c) => api::route_de_l_installation(nom, &c),
                    Err(e) => RouteReponse::de(nom, Err(e)),
                };
            }
            let tableau = routes::gestes::administrateur::rattachees(&inst).and_then(|rattachees| {
                let par = rattachees.iter().map(|r| (r.clone(), self.file_de(r).tableau())).collect();
                routes::gestes::administrateur::tableau_de(&inst, par)
            });
            return RouteReponse::de(nom, tableau.and_then(|t| Rendu::de(&t)));
        }
        api::route_de_l_installation(nom, charge)
    }

    /// Les gestes de la fenetre elle-meme (`window.pywebview.api.<nom>(...)`, `NOMS_ACCUEIL`, `NOMS_LOGICIEL`).
    fn lanceur(&self, nom: &str, args: &[Argument]) -> Geste {
        match self.geste(nom, args) {
            Ok(g) => g,
            Err(e) => Geste::Refuse { ok: false, refus: Refus::de(None, e.reclasser(Classe::Refuse)) },
        }
    }

    fn geste(&self, nom: &str, args: &[Argument]) -> R<Geste> {
        let fait = |recharger: bool| Geste::Fait { ok: true, recharger };
        match (nom, self.phase()) {
            ("choisir", Phase::Accueil) => {
                let r = installation::racine_choisie(&installation::base(), texte(args, 0)?)?;
                self.prendre(r)?;
                Ok(fait(true))
            }
            ("verifier", Phase::MotDePasse { racine, .. }) => {
                let motdepasse = texte(args, 0)?;
                if !compagnie::coffre::Coffre::du_poste(&racine).verifier_ouverture(motdepasse)? {
                    return Err(socle::echec::refus("Mot de passe refusé."));
                }
                self.retenir_administrateur(&racine, motdepasse);
                self.ouvrir(racine);
                Ok(fait(true))
            }
            ("accueil", _) => {
                self.ecrire("Retour a l'accueil.\n");
                self.poser_phase(Phase::Accueil);
                Ok(fait(true))
            }
            ("fermer", _) => {
                self.fermer_le_logiciel();
                Ok(fait(false))
            }
            // LES GESTES DU POSTE (`poste.rs`) : ouvrir ce qu'une route a rendu (un chemin), choisir un dossier.
            ("ouvrir", Phase::Ouverte { racine, .. }) => {
                let phrase = poste::ouvrir_pour_la_page(&racine, texte(args, 0)?, oui_non(args, 1)?)?;
                Ok(Geste::Dit { ok: true, phrase })
            }
            ("choisir_dossier", Phase::Ouverte { .. }) => Ok(Geste::Dossier { ok: true, dossier: poste::choisir_dossier(texte(args, 0)?)? }),
            // LA VEILLE DE LA FENETRE : son etat, l'etape du passage en cours, un passage tout de suite (un passage appelle
            // la route `ouverture`, qui releve et sauvegarde).
            ("veille", Phase::Ouverte { veille, .. }) => Ok(Geste::Veille { ok: true, etat: Box::new(veille.etat()) }),
            ("veille_etape", Phase::Ouverte { veille, .. }) => Ok(Geste::Etape { ok: true, etat: veille.etape() }),
            ("veille_maintenant", Phase::Ouverte { veille, .. }) => Ok(Geste::Veille { ok: true, etat: Box::new(veille.tour(false, false)) }),
            // Ou en est la recherche de la mise a jour : l'accueil et la page d'une compagnie l'affichent.
            ("maj", _) => Ok(Geste::Maj { ok: true, etat: compagnie::maj::RECHERCHE.etat() }),
            // Le site de l'editeur, dans le navigateur du poste : la fenetre ne quitte jamais sa page.
            ("site", _) => {
                poste::ouvrir_site(texte(args, 0)?)?;
                Ok(fait(false))
            }
            // « Redemarrer pour mettre a jour » : la version prete part au relais, puis le logiciel se ferme comme par sa
            // croix (la veille s'arrete) ; le relais attend sa fin, installe et rouvre la fenetre neuve. Rien de pret :
            // refuse, et rien ne se ferme.
            ("redemarrer", _) => {
                let v = compagnie::maj::redemarrer_pour_mettre_a_jour(&compagnie::maj::installation(), &compagnie::maj::RECHERCHE)?;
                self.ecrire(&format!("Redemarrage pour la mise a jour {}.\n", compagnie::maj::version_lue(&v)));
                self.poser_phase(Phase::Accueil);
                self.fermer_le_logiciel();
                Ok(fait(false))
            }
            _ => Err(socle::refus!("Geste inconnu ici : {}.", nom)),
        }
    }

    /// Une requete de la fenetre.
    pub fn traiter(&self, r: Requete) -> Reponse {
        let texte = |code: u16| Reponse::nouvelle(code, "text/plain; charset=utf-8", Vec::new());
        if r.methode == "POST" {
            // Un refus du serveur, avant toute route ou tout geste, se dit comme le refus d'une route (`refus`), que la page
            // lit de meme pour un geste de la fenetre.
            let refus = |code: u16, classe: Classe, message: String| Reponse::json(code, &RouteReponse::refusee(Refus::nouveau(classe, message)));
            if !self.autorise(&r) {
                return refus(403, Classe::NonAutorise, "Cette page n'est plus reconnue par PuppetPaie (jeton absent) : fermez la fenêtre et relancez le logiciel.".into());
            }
            if let Some(nom) = r.chemin.strip_prefix("/lanceur/") {
                let texte = match std::str::from_utf8(&r.corps) {
                    Ok(t) => t,
                    Err(e) => return refus(400, Classe::DemandeIllisible, format!("La demande ne se lit pas : ce n'est pas du texte ({}).", e)),
                };
                return match socle::forme::lire_texte::<Vec<Argument>>(texte, "Les arguments d'un geste de la fenêtre sont une liste de textes et de oui-non.") {
                    Ok(args) => Reponse::json(200, &self.lanceur(nom, &args)),
                    Err(m) => Reponse::json(400, &RouteReponse::refusee(Refus::de(None, m.reclasser(Classe::DemandeIllisible)))),
                };
            }
            #[cfg(feature = "atelier")]
            if let Some(nom) = r.chemin.strip_prefix("/atelier/") {
                return match self.atelier(nom, &r.corps) {
                    Ok(a) => Reponse::json(200, &a),
                    Err(e) => Reponse::json(200, &Atelier::Refuse { ok: false, refus: Refus::de(None, e.reclasser(Classe::Refuse)) }),
                };
            }
            if let Some(texte) = r.chemin.strip_prefix("/api/") {
                // LE NOM DE LA ROUTE SE LIT ICI, UNE FOIS : un nom qui n'est celui d'aucune route se refuse ; une charge qui
                // n'est pas un objet aussi.
                let nom = match routes::enveloppe::route_nommee(texte) {
                    Ok(n) => n,
                    Err(e) => return Reponse::json(200, &RouteReponse::refusee(Refus::de(None, e))),
                };
                let charge = match Charge::du_texte(&r.corps) {
                    Ok(c) => c,
                    Err(e) => return Reponse::json(200, &RouteReponse::de(nom, Err(e))),
                };
                // LA SEULE PORTE DE LA PAGE VERS LE MOTEUR : une route du catalogue. Celle de l'installation se sert a tout
                // moment ; celle d'une compagnie, quand la compagnie est ouverte -- ou quand elle n'a pas encore de mot de
                // passe (rien a deverrouiller : la page du premier mot de passe le pose par sa route).
                // Ecrire au developpeur se sert aussi a l'accueil, aucune compagnie ouverte (`routes::filet::sans_compagnie`).
                // Le catalogue (`routes`) ne porte sur aucune compagnie : l'accueil le lit pour son « Avancé » (`js/avance.js`).
                let au_developpeur = routes::catalogue::dans(routes::filet::ROUTES, nom).is_some() && !matches!(self.phase(), Phase::Ouverte { .. });
                let catalogue = routes::catalogue::dans(routes::catalogue::ROUTES, nom).is_some();
                if routes::catalogue::dans(routes::accueil::ROUTES, nom).is_some() || au_developpeur || catalogue {
                    let debut = Instant::now();
                    let rendu = self.route_de_l_installation(nom, &charge);
                    self.ecrire(&ligne_de_route(nom, debut.elapsed(), &rendu));
                    return Reponse::json(200, &rendu);
                }
                let (racine, veille, etats) = match self.phase() {
                    Phase::Ouverte { racine, veille, etats } => (racine, Some(veille), Some(etats)),
                    Phase::MotDePasse { racine, premiere: true } => (racine, None, None),
                    _ => return Reponse::json(200, &RouteReponse::refusee_par(nom, Refus::nouveau(Classe::Refuse, "Aucune compagnie n'est ouverte."))),
                };
                // LE JOURNAL DE L'INTERFACE note chaque route et sa duree (de quoi mesurer l'ouverture sur le poste) :
                // l'heure, le nom, les millisecondes, et la classe d'un refus. Jamais la charge : elle peut porter un mot
                // de passe.
                // CE QUE LA PAGE A FAIT depuis sa derniere demande (`X-Gestes`, js/tuyau.js : clics, champs, boites, console),
                // note dans le fil AVANT la route qu'il a declenchee (`diagnostic::noter_gestes`, qui le met en forme).
                if let Some(g) = r.entete("x-gestes").filter(|g| !g.is_empty()) {
                    compagnie::diagnostic::noter_gestes(&racine, &http::decoder_pourcent(g));
                }
                // SUPPRIMER LA COMPAGNIE OUVERTE (`compagnie-supprimer` confirmee) : la route decide tout
                // (`suppression::route`) ; la fenetre lache d'abord ce qu'elle tient de la compagnie (sa veille, sa file
                // des etats, son journal), et ne note rien apres dans un dossier qui n'est plus. Un passage de la veille en
                // cours ecrit dans ce dossier : on attend qu'il finisse.
                if nom == Route::CompagnieSupprimer {
                    let c: suppression::ChargeSupprimer = match charge.lire() {
                        Ok(c) => c,
                        Err(e) => return Reponse::json(200, &RouteReponse::de(nom, Err(e))),
                    };
                    if c.confirme == Some(true) {
                        let rendu = if veille.as_ref().is_some_and(|v| v.occupee()) {
                            RouteReponse::refusee_par(nom, Refus::nouveau(Classe::Refuse, "Un relevé ou une sauvegarde de cette compagnie est en cours : attendez qu'il se termine, puis recommencez. Rien n'a été supprimé."))
                        } else {
                            match suppression::route(&racine, c, charge.mot_de_passe(), &|r| self.lacher(r)) {
                                Ok(s) => RouteReponse::de(nom, routes::enveloppe::en_json(s)),
                                Err(m) => RouteReponse::refusee_par(nom, Refus::de(None, m.reclasser(Classe::Refuse))),
                            }
                        };
                        if rendu.est_rendue() {
                            self.ecrire(&format!("Compagnie supprimee : {}.\n", racine.display()));
                        }
                        return Reponse::json(200, &rendu);
                    }
                }
                let debut = Instant::now();
                let rendu = api::repondre(&racine, nom, &charge, veille.as_deref(), etats.as_deref());
                self.ecrire(&ligne_de_route(nom, debut.elapsed(), &rendu));
                return Reponse::json(200, &rendu);
            }
            return refus(404, Classe::RouteInconnue, "Cette adresse n'est ni une route ni un geste de la fenêtre.".into());
        }
        match r.chemin.as_str() {
            "/" | "/index.html" => {
                if !self.autorise(&r) {
                    return Reponse::nouvelle(403, "text/plain; charset=utf-8", "jeton absent ou invalide");
                }
                if r.params.contains_key("jeton") && r.chemin == "/" {
                    // Le jeton passe en cookie, et l'adresse redevient propre.
                    return Reponse::nouvelle(303, "text/plain", Vec::new())
                        .avec("Location", "/")
                        .avec("Set-Cookie", format!("{}={}; Path=/; HttpOnly; SameSite=Strict", self.nom_du_cookie(), self.jeton));
                }
                let html = match self.phase() {
                    Phase::Accueil => match page_accueil() {
                        Ok(p) => page(&p, &self.jeton, NOMS_ACCUEIL),
                        Err(e) => return Reponse::nouvelle(500, "text/plain; charset=utf-8", e.phrase),
                    },
                    Phase::MotDePasse { racine, premiere } => {
                        // Ce que l'administrateur est pour cette compagnie (la route `administrateur-etat`) : la page le lit,
                        // sans rien decider.
                        let etat = match routes::coffre::administrateur_etat(&racine) {
                            Ok(e) => e,
                            Err(e) => return Reponse::nouvelle(500, "text/plain; charset=utf-8", e.phrase),
                        };
                        let gabarit = if premiere { CHOIX_MOT_DE_PASSE } else { MOT_DE_PASSE };
                        page(&gabarit.replace("{{ADMINISTRATEUR}}", &json_dans_la_page(&etat)), &self.jeton, NOMS_ACCUEIL)
                    }
                    Phase::Ouverte { racine, .. } => match page_logiciel(&racine, &self.jeton) {
                        Ok(p) => p,
                        Err(e) => return Reponse::nouvelle(500, "text/plain; charset=utf-8", e.phrase),
                    },
                };
                Reponse::nouvelle(200, "text/html; charset=utf-8", html)
            }
            "/fictive/page" => self.piece_fictive(&r),
            "/apercu/bulletin" => self.apercu_bulletin(&r),
            c if c.starts_with("/identite/") => self.identite(&r, c.trim_start_matches("/identite/")),
            c => match fichier_de_la_page(c) {
                // UNE FEUILLE OU UN SCRIPT NOMME PAR SON EMPREINTE (`?v=`, `empreinte`) se garde : la page
                // d'une compagnie charge vingt-cinq fichiers, qu'elle ne redemande plus a chaque ouverture. Ce qui change
                // change d'empreinte, donc d'adresse : rien de perime ne se garde.
                Ok(Some((o, mime))) if r.params.contains_key("v") && (c.ends_with(".js") || c.ends_with(".css")) => {
                    Reponse::nouvelle(200, mime, o).avec("Cache-Control", "max-age=31536000, immutable")
                }
                Ok(Some((o, mime))) => Reponse::nouvelle(200, mime, o),
                Ok(None) => texte(404),
                Err(e) => {
                    self.ecrire(&format!("{}\n", e.en_lignes()));
                    texte(404)
                }
            },
        }
    }

    /// Le PDF joint a un courriel joue d'une compagnie fictive, tel quel (le lecteur de la fenetre l'affiche).
    fn piece_fictive(&self, r: &Requete) -> Reponse {
        let interdit = Reponse::nouvelle(403, "text/plain", Vec::new());
        let Phase::Ouverte { racine, .. } = self.phase() else { return interdit };
        if !self.autorise(r) {
            return interdit;
        }
        // Une piece fictive ne se sert que d'une compagnie dont les envois sont joues ; un marqueur illisible ne le dit pas.
        let joue = match compagnie::bridage::branchement(&racine) {
            Ok(b) => b.joue(),
            Err(e) => {
                self.ecrire(&format!("{}\n", e.phrase));
                return interdit;
            }
        };
        if !joue {
            return interdit;
        }
        let rien = || Reponse::nouvelle(404, "text/plain", Vec::new());
        let panne = |m: String| Reponse::nouvelle(500, "text/plain; charset=utf-8", m.into_bytes());
        // `c` : le courriel joue ; `p` : le rang de sa piece jointe.
        let (Some(nom), Some(rang)) = (r.param("c"), r.param("p")) else { return rien() };
        let rang: usize = match rang.parse() {
            Ok(n) => n,
            Err(e) => return panne(format!("Le rang de la pièce ne se lit pas : {}.", e)),
        };
        let joues = match envois::simulation_courriels::courriels(&racine) {
            Ok(j) => j,
            Err(e) => return panne(e.phrase),
        };
        let Some(brut) = joues.into_iter().find(|(n, _)| n == nom).map(|(_, o)| o) else { return rien() };
        let m = match mailparse::parse_mail(&brut) {
            Ok(m) => m,
            Err(e) => return panne(format!("Le courriel joué ne se lit pas : {}.", e)),
        };
        let mut feuilles = Vec::new();
        envois::simulation_courriels::feuilles(&m, &mut feuilles);
        let pieces: Vec<_> = feuilles.into_iter().filter_map(|f| envois::simulation_courriels::nom_de_fichier(f).map(|n| (f, n))).collect();
        let Some((piece, nom_pdf)) = pieces.get(rang) else { return rien() };
        let octets = match piece.get_body_raw() {
            Ok(o) => o,
            Err(e) => return panne(format!("La pièce jointe ne se lit pas : {}.", e)),
        };
        if piece.ctype.mimetype != "application/pdf" || !octets.starts_with(b"%PDF") {
            return rien();
        }
        let sur: String = nom_pdf.chars().map(|c| if c.is_alphanumeric() || "._-".contains(c) { c } else { '_' }).collect();
        Reponse::nouvelle(200, "application/pdf", octets)
            .avec("Content-Disposition", format!("inline; filename=\"{}\"", sur))
            .avec("Content-Security-Policy", "default-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'self'")
    }

    /// L'apercu d'un modele de bulletin : le PDF d'une paie fictive, que la route `bulletin-modele-apercu` fait dans le
    /// processus de la fenetre, comme toute route ; le lecteur de la fenetre l'affiche (Configuration).
    fn apercu_bulletin(&self, r: &Requete) -> Reponse {
        let interdit = Reponse::nouvelle(403, "text/plain", Vec::new());
        let Phase::Ouverte { racine, .. } = self.phase() else { return interdit };
        if !self.autorise(r) {
            return interdit;
        }
        let Some(modele) = r.param("modele").and_then(routes::bulletin::modele_de_l_adresse) else {
            return Reponse::nouvelle(404, "text/plain", Vec::new());
        };
        // La route, comme la page l'appellerait : sa charge et sa reponse lues par leur forme.
        let lire = || -> R<Vec<u8>> {
            let charge = Charge::de_forme(&routes::bulletin::ChargeModele { modele: modele.cle().to_string() })?;
            let a: routes::bulletin::ApercuModele = api::route(&racine, Route::BulletinModeleApercu, &charge).lire().map_err(|x| socle::echec::refus(x.phrase))?;
            socle::base64::lire(&a.pdf).map_err(|e| e.reformuler(|p| format!("L'aperçu du bulletin ne se lit pas : {}.", p)))
        };
        match lire() {
            Ok(octets) if octets.starts_with(b"%PDF") => Reponse::nouvelle(200, "application/pdf", octets)
                .avec("Content-Disposition", format!("inline; filename=\"apercu-bulletin-{}.pdf\"", if modele == documents::bulletin::Modele::Complet { "complet" } else { "simplifie" }))
                .avec("Content-Security-Policy", "default-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'self'"),
            Ok(_) => Reponse::nouvelle(500, "text/plain; charset=utf-8", "L'aperçu du bulletin n'est pas un PDF."),
            Err(e) => Reponse::nouvelle(500, "text/plain; charset=utf-8", e.phrase),
        }
    }

    /// L'image posee pour la signature, le logo ou le cachet ; ou le cachet que le logiciel dessine.
    fn identite(&self, r: &Requete, quoi: &str) -> Reponse {
        let rien = || Reponse::nouvelle(404, "text/plain", Vec::new());
        if !self.autorise(r) {
            return Reponse::nouvelle(403, "text/plain", Vec::new());
        }
        let Phase::Ouverte { racine, .. } = self.phase() else { return rien() };
        if quoi == "cachet-automatique" {
            let ent = match droit::contexte::Contexte::nouveau(&racine, None) {
                Ok(c) => c.entreprise,
                Err(e) => return Reponse::nouvelle(500, "text/plain; charset=utf-8", e.phrase),
            };
            return match documents::cachet::svg(&ent) {
                Some(d) => Reponse::nouvelle(200, "image/svg+xml; charset=utf-8", d),
                None => rien(),
            };
        }
        if !["cachet", "logo", "signature"].contains(&quoi) {
            return rien();
        }
        let dossier = racine.join("gabarits").join("identite");
        for (ext, mime) in [(".png", "image/png"), (".jpg", "image/jpeg"), (".svg", "image/svg+xml")] {
            let f = dossier.join(format!("{}{}", quoi, ext));
            match std::fs::read(&f) {
                Ok(o) => return Reponse::nouvelle(200, mime, o),
                // Pas d'image dans ce format : le suivant.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Reponse::nouvelle(500, "text/plain; charset=utf-8", socle::echec::du_disque(&f, &e).phrase),
            }
        }
        rien()
    }

    /// L'adresse que la fenetre charge (avec le jeton, qui passe aussitot en cookie).
    pub fn adresse(&self, hote: &str, port: u16) -> String {
        format!("http://{}:{}/?jeton={}", hote, port, self.jeton)
    }
}

/// Ajoute `texte` au journal `chemin` (cree avec son dossier) ; le disque qui refuse se dit.
fn ajouter_au_journal(chemin: &Path, texte: &str) -> R<()> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).map_err(|e| socle::echec::du_disque(d, &e))?;
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(chemin).map_err(|e| socle::echec::du_disque(chemin, &e))?;
    f.write_all(texte.as_bytes()).map_err(|e| socle::echec::du_disque(chemin, &e))
}

/// La ligne du journal de l'interface pour une route servie : `HH:MM:SS route NOM N ms`, et ` refus CLASSE` quand elle a
/// refuse. Ni la charge ni la reponse : seulement de quoi mesurer.
pub fn ligne_de_route(nom: Route, duree: Duration, rendu: &RouteReponse) -> String {
    let heure = socle::paris::maintenant_naif().iso();
    // L'heure seule (`HH:MM:SS`), apres la date de l'horodatage.
    let a_l_heure = match heure.get(11..) {
        Some(h) => h,
        None => heure.as_str(),
    };
    let mut ligne = format!("{} route {} {} ms", a_l_heure, nom, duree.as_millis());
    if let Some(r) = rendu.refus() {
        ligne.push_str(&format!(" refus {}", socle::echec::classe_notee(r.classe)));
    }
    ligne.push('\n');
    ligne
}

fn egal_en_temps_constant(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// AU LANCEMENT, avant tout : la livraison de fictives prete (`maj::poser_les_fictives_pretes`), et le reste cache d'une
/// compagnie supprimee (`suppression::effacer_les_restes`). Ce qui s'est passe va dans le journal de l'installation
/// (`maj::noter`).
pub fn preparer_l_installation() {
    let inst = compagnie::maj::installation();
    // Une livraison de compagnies fictives prete (`maj::preparer_les_fictives`) remplace celles du poste, avant toute fenetre.
    if let Some(t) = compagnie::maj::poser_les_fictives_pretes(&inst) {
        compagnie::maj::noter(&inst, &t);
    }
    // Une compagnie supprimee dont un fichier resistait (`suppression::effacer`) : son reste cache part maintenant.
    match suppression::effacer_les_restes(&inst) {
        Ok(restes) => {
            if !restes.partis.is_empty() {
                let noms: Vec<String> = restes.partis.iter().map(|p| p.display().to_string()).collect();
                compagnie::maj::noter(&inst, &format!("Restes de compagnies supprimees effaces : {}.", noms.join(", ")));
            }
            for phrase in &restes.resistent {
                compagnie::maj::noter(&inst, &format!("Un reste de compagnie supprimee resiste, il s'effacera au lancement suivant : {}", phrase));
            }
        }
        Err(e) => compagnie::maj::noter(&inst, &format!("Les restes de compagnies supprimees ne se cherchent pas : {}", e.phrase)),
    }
}

/// LE VERROU DE CETTE INSTALLATION (`compagnie::instance`) : un creneau par installation -- son verrou et le port de sa page.
/// La meme installation relancee : `Devant` (sa fenetre est revenue devant) ; une autre installation ouverte sur le poste
/// ne change rien, chacune a son creneau.
pub fn prendre_instance(devant: impl Fn() + Send + 'static) -> socle::echec::R<Instance> {
    instance::prendre(devant)
}

/// Demarre le serveur de la fenetre dans un fil, sur l'ecoute de la page que le verrou a prise (`Prise`, que l'appelant
/// garde tant que la fenetre vit : elle porte le verrou). Rend la fenetre et son port.
pub fn demarrer(minutes_veille: u64, prise: &Prise) -> std::io::Result<(Arc<Fenetre>, u16)> {
    let page = prise.page.try_clone()?;
    instance::sans_heritage(&page);
    servir_sur(minutes_veille, page, prise.port)
}

/// Demarre le serveur de la fenetre dans un fil, au port `port` s'il est libre (sinon un port que le systeme donne : la
/// ligne « la fenetre ecoute sur » dit lequel). Rend la fenetre et le port. Hors du verrou : `moteur interface --port`, et
/// les essais.
pub fn demarrer_sur(minutes_veille: u64, port: u16) -> std::io::Result<(Arc<Fenetre>, u16)> {
    let (ecouteur, port) = http::ecouter(port)?;
    servir_sur(minutes_veille, ecouteur, port)
}

fn servir_sur(minutes_veille: u64, ecouteur: TcpListener, port: u16) -> std::io::Result<(Arc<Fenetre>, u16)> {
    // L'heure du lancement et les paniques notees (`diagnostic`) : ce qu'un retour et un incident disent de la fenetre.
    compagnie::diagnostic::installer();
    let f = Fenetre::nouvelle(minutes_veille, navigateur::dossier_local(&installation::base(), port), port).map_err(|e| std::io::Error::other(e.en_lignes()))?;
    let servie = f.clone();
    std::thread::spawn(move || http::servir(ecouteur, Arc::new(move |r| servie.traiter(r))));
    f.ecrire(&format!("PuppetPaie {} : la fenetre ecoute sur 127.0.0.1:{}.\n", version(), port));
    Ok((f, port))
}

/// Ce que `moteur interface` a lu de sa ligne de commande. `port` : celui qu'on impose ; sans lui, celui de l'installation
/// (`compagnie::instance`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub port: Option<u16>,
    pub minutes_veille: u64,
    pub sans_navigateur: bool,
}

const USAGE: &str = "Usage : moteur interface [--port N] [--sans-navigateur] [--veille MINUTES]";

/// Les options de `moteur interface`. Une valeur qui ne se lit pas est refusee, jamais remplacee par le defaut.
pub fn lire_options(args: &[String]) -> R<Options> {
    let mut o = Options { port: None, minutes_veille: MINUTES_VEILLE, sans_navigateur: false };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--sans-navigateur" => {
                o.sans_navigateur = true;
                i += 1;
            }
            "--port" if i + 1 < args.len() => {
                let port: u16 = args[i + 1].parse().map_err(|e| socle::refus!("Port illisible : {} ({}). {}", args[i + 1], e, USAGE))?;
                if port == 0 {
                    return Err(socle::refus!("Port illisible : 0. {}", USAGE));
                }
                o.port = Some(port);
                i += 2;
            }
            "--veille" if i + 1 < args.len() => {
                o.minutes_veille = args[i + 1].parse().map_err(|_| socle::refus!("Duree de veille illisible : {}. {}", args[i + 1], USAGE))?;
                i += 2;
            }
            autre => return Err(socle::refus!("Option inconnue : {}. {}", autre, USAGE)),
        }
    }
    Ok(o)
}

/// LE BANC D'ESSAI (l'atelier seulement ; `atelier/outils/banc/`) : `moteur-atelier interface ... [--enregistrer
/// DOSSIER [--lectures]] [--jour AAAA-MM-JJ]`. `--jour` : le temps du moteur coule depuis ce jour, 9 h a Paris (`temps`) ;
/// `--enregistrer` : chaque geste qui ecrit entre dans le scenario `DOSSIER` (`scenario::Enregistreur`), les lectures
/// aussi avec `--lectures`. Un scenario deja commence se poursuit : sans `--jour`, le temps reprend une minute apres son dernier geste ; un `--jour` avant lui est
/// refuse. Rend les autres options, pour `lire_options`.
#[cfg(feature = "atelier")]
fn poser_le_banc(args: &[String]) -> R<Vec<String>> {
    let (mut reste, mut dossier, mut jour, mut lectures) = (Vec::new(), None, None, false);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--enregistrer" if i + 1 < args.len() => {
                dossier = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--lectures" => {
                lectures = true;
                i += 1;
            }
            "--jour" if i + 1 < args.len() => {
                jour = Some(socle::ligne::Argument::tape(&args[i + 1]).jour().map_err(|_| socle::refus!("Jour illisible : {} (AAAA-MM-JJ).", args[i + 1]))?);
                i += 2;
            }
            a => {
                reste.push(a.to_string());
                i += 1;
            }
        }
    }
    let enr = match &dossier {
        Some(d) => Some(routes::scenario::Enregistreur::demarrer(d, &installation::base(), lectures)?),
        None => None,
    };
    let dernier = match enr {
        Some(e) => e.dernier_instant()?,
        None => None,
    };
    let hors_calendrier = || socle::echec::refus(socle::date::HORS_CALENDRIER);
    let depart = match (jour, dernier) {
        (Some(j), dernier) => {
            let neuf_heures = socle::paris::Moment::a_paris(j, 9, 0, 0).ok_or_else(hors_calendrier)?;
            if let Some(t) = dernier.filter(|t| neuf_heures <= *t) {
                return Err(socle::echec::refus_avec(
                    format!("Le scénario va déjà jusqu'au {} : ce jour le ferait reculer.", socle::paris::iso_secondes(t)),
                    socle::echec::Lieu { detail: Some(j.iso()), ..Default::default() },
                ));
            }
            Some(neuf_heures)
        }
        (None, Some(t)) => Some(t.plus(60).ok_or_else(hors_calendrier)?),
        (None, None) => None,
    };
    if let Some(t) = depart {
        socle::temps::couler_depuis(t);
    }
    if let Some(e) = enr {
        // LES ALEAS DU BANC sont ceux du rejeu (`temps::aleas`) : la compagnie du banc et son rejeu s'ecrivent pareil, et
        // `scenario::verifier` le controle. Le banc n'est jamais un poste.
        socle::temps::aleas_fixes();
        use routes::scenario::{Genre, Geste};
        e.geste(|| Ok(((), Geste { jour: Some(socle::temps::jour_de_paris()), logiciel: Some(version()), format: Some(routes::scenario::FORMAT), ..Geste::du_banc(Genre::Debut) })))?;
    }
    Ok(reste)
}

/// `moteur interface [--port N] [--sans-navigateur] [--veille MINUTES]` : la fenetre dans le navigateur du poste.
pub fn commande(args: &[String]) -> i32 {
    if let Err(e) = installation::fixer_le_code() {
        eprintln!("{}", e.en_lignes());
        return 1;
    }
    compagnie::incidents::brancher_l_engagement();
    #[cfg(feature = "atelier")]
    let args = &match poser_le_banc(args) {
        Ok(reste) => reste,
        Err(e) => {
            eprintln!("{}", e.en_lignes());
            return 2;
        }
    };
    let options = match lire_options(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{}", e.en_lignes());
            return 2;
        }
    };
    let minutes = options.minutes_veille;
    let devant: Arc<Mutex<Option<(String, PathBuf)>>> = Arc::new(Mutex::new(None));
    let pour_devant = devant.clone();
    let instance = prendre_instance(move || {
        if let Some((a, profil)) = tenu(&pour_devant).clone() {
            navigateur::ouvrir(&a, &profil, &|_| {});
        }
    });
    let prise = match instance {
        Ok(Instance::Tenue(p)) => p,
        Ok(Instance::Devant) => {
            dire!("PuppetPaie est deja ouvert depuis cette installation : sa fenetre revient devant.");
            return 0;
        }
        Err(e) => {
            eprintln!("La fenetre ne demarre pas : {}", e.phrase);
            return 1;
        }
    };
    // La mise a jour se cherche au lancement, pour l'installation (`maj::chercher_en_fond`) ; elle s'appliquera au
    // prochain lancement de la fenetre.
    preparer_l_installation();
    compagnie::maj::chercher_en_fond();
    let demarre = match options.port {
        Some(p) => demarrer_sur(minutes, p),
        None => demarrer(minutes, &prise),
    };
    let (f, port) = match demarre {
        Ok(x) => x,
        Err(e) => {
            eprintln!("La fenetre ne demarre pas : {}.", e);
            return 1;
        }
    };
    // Les compagnies de l'installation se tiennent a jour en fond, des maintenant, sans rien ouvrir (`prechauffer`).
    f.prechauffer();
    *tenu(&f.fermer) = Some(Box::new(|| {
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(200));
            std::process::exit(0);
        });
    }));
    let url = f.adresse("puppetpaie.localhost", port);
    *tenu(&devant) = Some((url.clone(), f.dossier_local().join("navigateur")));
    dire!("{}", url);
    let navigateur = if options.sans_navigateur { None } else { navigateur::ouvrir(&url, &f.dossier_local().join("navigateur"), &|t| f.ecrire(t)) };
    // Le navigateur a nous : sa fermeture arrete PuppetPaie. Sinon, plus aucune page depuis cinq minutes l'arrete
    // (la page interroge la veille chaque minute).
    if let Some(mut p) = navigateur {
        let debut = Instant::now();
        match p.wait() {
            Ok(_ferme) => {
                if debut.elapsed() > Duration::from_secs(5) {
                    return 0;
                }
            }
            // Le navigateur ne se laisse pas attendre : le guet du silence arretera la fenetre.
            Err(e) => f.ecrire(&format!("Le navigateur ne se laisse pas attendre ({}) : la fenetre s'arretera apres cinq minutes sans page.\n", e)),
        }
    }
    if options.sans_navigateur {
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    }
    loop {
        std::thread::sleep(Duration::from_secs(5));
        if f.silence() > Duration::from_secs(5 * 60) {
            return 0;
        }
    }
}

/// CE QUE LA PAGE DIT QUAND LE MOTEUR NE PEUT PLUS LE DIRE : le serveur qui ne repond plus, une reponse illisible. La
/// fenetre les pose dans la page qu'elle sert (`{{PHRASES}}` de `index.html`), la page n'en ecrit aucune.
#[derive(Serialize)]
struct PhrasesDeLaFenetre {
    perdu: &'static str,
    perdu_court: &'static str,
    illisible: &'static str,
    lecture_absente: &'static str,
}

const PHRASES_DE_LA_FENETRE: PhrasesDeLaFenetre = PhrasesDeLaFenetre {
    perdu: "Le serveur ne répond plus : la fenêtre PuppetPaie a été fermée, ou le terminal qui la faisait tourner. Rien de ce qui est à l'écran n'est enregistré. Relancer PuppetPaie, puis recharger cette page.",
    perdu_court: "Le serveur PuppetPaie ne répond plus. Rien n'a été enregistré.",
    illisible: "PuppetPaie a répondu sans dire pourquoi. Rien n'est sûr d'avoir été enregistré.",
    lecture_absente: "Lecture absente de l'état du mois.",
};

/// CE QUE DISENT L'ACCUEIL DES COMPAGNIES ET LES PAGES DU MOT DE PASSE, que la fenetre sert (`pages/`) : chacune a sa
/// place dans le gabarit (`{{P:cle}}`), y compris dans ses scripts (sans guillemet droit ni retour a la ligne).
const PHRASES_DE_L_ACCUEIL: &[(&str, &str)] = &[
    ("ouvre_pas", "La compagnie ne s'ouvre pas."),
    ("zero_confirmer", "remettre à zéro ? Tout ce qui a été fait dans cette compagnie fictive sera effacé."),
    ("toutes_faites", "Toutes les déclarations de vos compagnies sont faites."),
    ("tableau_illisible", "Le tableau de cette compagnie ne se lit pas"),
    ("administrateur_seul", "un seul mot de passe, celui de l'administrateur."),
    ("chaque_dsn", "Chaque DSN part avec ses contrôles et son bilan, compagnie par compagnie. Le mot de passe de l'administrateur le confirme."),
    ("mdp_administrateur", "le mot de passe de l'administrateur"),
    ("rien_parti", "Rien n'est parti."),
    ("rien_change", "Rien n'a changé."),
    ("avis_ajouter", "Vous pouvez ajouter votre compagnie et regarder le logiciel avec vos vraies données. Pour 2027, et afin d'offrir un service de qualité, le nombre d'utilisateurs est restreint. N'hésitez pas à contacter Léandre"),
    ("avis_ajouter_suite", "avec une courte présentation et votre motivation pour faire partie des premiers utilisateurs."),
    ("meme_administrateur", "Gérer cette compagnie avec le même administrateur ?"),
    ("ouvre_deja", "Le mot de passe qui ouvre déjà"),
    ("non_a_elle", "Non, un mot de passe à elle"),
    ("tapez_pour", "Le mot de passe que vous tapez pour"),
    ("ouvrira_aussi", "ouvrira aussi cette compagnie, et il servira aux actes sur toutes à la fois, comme déposer d'un coup toutes les DSN prêtes."),
    ("choisissez", "Choisissez le mot de passe du logiciel"),
    ("demande_ouverture", "Il sera demandé à chaque ouverture et à chaque dépôt réel. Ce n'est pas celui d'un organisme."),
    ("retour_choix", "Revenir au choix de la compagnie"),
    ("garder_titre", "Vous venez de débloquer le logiciel. Voulez-vous garder les compagnies créées ?"),
    ("garder_note", "Une compagnie gardée devient une compagnie réelle : son mot de passe du logiciel se choisit à sa première ouverture. Une compagnie décochée est supprimée, avec tout son dossier."),
    ("mdp_logiciel", "Mot de passe du logiciel"),
    ("mdp_redemande", "Celui du logiciel, pas celui d'un organisme. Il sera redemandé à chaque dépôt."),
    ("mdp_titre_admin", "Mot de passe de l'administrateur"),
    ("neuve", "Elle commence ses paies ici : vous la configurez, puis vous engagez vos salariés."),
    ("precedent", "Vous faisiez vos paies ailleurs : vous mettez dans un dossier ce que votre ancien logiciel vous a laissé (bulletins, déclarations), et PuppetPaie se paramètre avec."),
    ("zero", "Remettre à zéro : tout ce qui a été fait ici s'efface"),
];
