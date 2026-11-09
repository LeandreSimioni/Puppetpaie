//! `moteur guet` : le guet de l'orchestrateur, combien de contexte le sous-agent a-t-il consomme ?
//! Un outil d'atelier.
//!
//! Lit toutes les 30 s le dernier `usage` du transcript du sous-agent (et de celui de l'orchestrateur), jamais un
//! transcript entier : la fin du fichier seulement. Il SORT quand il faut reveiller l'orchestrateur, qui le lance en
//! arriere-plan avec le sous-agent :
//!
//! - le sous-agent passe `--seuil` (300 000 par defaut) ;
//! - avec `--commits`, un nouveau commit apparait sur HEAD (le moment propre pour un arret) ;
//! - l'orchestrateur passe 300 000 ;
//! - le transcript du sous-agent n'a plus bouge depuis 20 minutes (fini ou bloque).
//!
//! OU SONT LES TRANSCRIPTS : sous `~/.claude/projects/<dossier ou la seance a ete lancee, encode>/`, le fichier
//! `<seance>.jsonl` pour l'orchestrateur, `<seance>/subagents/*.jsonl` pour ses sous-agents. L'encodage remplace `/`
//! par `-`, et aussi tout autre caractere hors `[A-Za-z0-9]` (un `\`, un `:`, un point), par inference : c'est ce
//! qu'on voit sur les dossiers deja crees, pas un texte lu. La seance peut etre lancee depuis le depot ou depuis un de
//! ses parents : le guet essaie le depot puis chacun de ses parents, et garde celui des
//! dossiers dont le dernier `.jsonl` est le plus recent. `--transcript` force le fichier du sous-agent.
//!
//! ```text
//! moteur guet                              le dernier sous-agent lance, seuil 300 000
//! moteur guet --seuil 330000 --commits
//! moteur guet --transcript CHEMIN.jsonl
//! ```
//!
//! Ne l'arreter jamais par `pkill -f guet` : le motif tue le shell qui le lance.

use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde::Deserialize;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Le seuil de l'orchestrateur, fixe, et celui du sous-agent par defaut.
const SEUIL: u64 = 300_000;
/// Ce qu'on lit de la fin d'un transcript : assez pour y trouver le dernier tour.
const FIN_LUE: u64 = 400_000;
/// Le pas de la veille.
const PAS: Duration = Duration::from_secs(30);
/// Un sous-agent dont le transcript n'a pas bouge depuis ce temps est fini ou bloque.
const IMMOBILE: Duration = Duration::from_secs(1200);

const AIDE: &str = "usage : moteur guet [--seuil N] [--commits] [--transcript CHEMIN.jsonl]\n\
    \x20 sort quand le sous-agent passe le seuil (300 000 par defaut), quand l'orchestrateur passe 300 000,\n\
    \x20 au prochain commit sur HEAD (--commits), ou apres 20 minutes sans que le transcript du sous-agent bouge.\n\
    \x20 --transcript : le transcript du sous-agent (par defaut : le plus recent).";

struct Options {
    seuil: u64,
    commits: bool,
    transcript: Option<PathBuf>,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut o = Options { seuil: SEUIL, commits: false, transcript: None };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--commits" => o.commits = true,
            "--seuil" | "--transcript" => {
                let v = args.get(i + 1).ok_or_else(|| format!("{} attend une valeur", args[i]))?;
                if args[i] == "--seuil" {
                    o.seuil = v.replace('_', "").parse().map_err(|_| format!("--seuil : nombre attendu, pas {:?}", v))?;
                } else {
                    o.transcript = Some(PathBuf::from(v));
                }
                i += 1;
            }
            a => return Err(format!("option inconnue : {}", a)),
        }
        i += 1;
    }
    Ok(o)
}

/// `moteur guet ...` : le code de sortie (0 quand il sonne, 1 sans transcript, 2 sur une option fausse).
pub fn commande(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--aide" || a == "-h" || a == "--help") {
        dire!("{}", AIDE);
        return 0;
    }
    let o = match options(args) {
        Ok(o) => o,
        Err(m) => {
            eprintln!("{}\n{}", m, AIDE);
            return 2;
        }
    };
    match guetter(&o) {
        Ok(code) => code,
        Err(m) => {
            eprintln!("{}", m);
            1
        }
    }
}

/// Un usage tel que le guet l'affiche : rien tant que le transcript n'en porte aucun.
fn dit(u: Option<u64>) -> String {
    match u {
        Some(n) => n.to_string(),
        None => "aucun usage".to_string(),
    }
}

fn guetter(o: &Options) -> Result<i32, String> {
    let depot = depot()?;
    let projets = match dossier_maison() {
        Some(m) => m.join(".claude").join("projects"),
        None => return Err("ni HOME ni USERPROFILE : pas de dossier ~/.claude/projects".to_string()),
    };
    let projet = dossier_des_transcripts(&projets, &depot)?;
    let moi = match &projet {
        Some(p) => plus_recent(jsonl_de(p)?)?,
        None => None,
    };
    let sous_agent = match (&o.transcript, &projet) {
        (Some(t), _) => t.clone(),
        (None, Some(p)) => match plus_recent(jsonl_des_sous_agents(p)?)? {
            Some(s) => s,
            None => return Err(format!("aucun transcript de sous-agent sous {}", p.display())),
        },
        (None, None) => return Err(format!("aucun transcript sous {} pour {} ni ses parents", projets.display(), depot.display())),
    };
    dire!("guet : {}, seuil {}{}", sous_agent.display(), o.seuil, if o.commits { ", et au prochain commit" } else { "" });

    let depart = head(&depot)?;
    loop {
        let s = dernier_usage(&sous_agent)?;
        if let Some(n) = s.filter(|n| *n >= o.seuil) {
            dire!("SEUIL : sous-agent a {} (>= {})", n, o.seuil);
            return Ok(0);
        }
        if let Some(moi) = &moi {
            if let Some(m) = dernier_usage(moi)?.filter(|m| *m >= SEUIL) {
                dire!("SEUIL : orchestrateur a {}", m);
                return Ok(0);
            }
        }
        if o.commits {
            let h = head(&depot)?;
            if h != depart {
                let court = match h.get(..8) {
                    Some(c) => c,
                    None => &h,
                };
                dire!("COMMIT {} : sous-agent a {}", court, dit(s));
                return Ok(0);
            }
        }
        let bouge = date(&sous_agent)?;
        match SystemTime::now().duration_since(bouge) {
            Ok(d) if d > IMMOBILE => {
                dire!("IMMOBILE depuis 20 minutes : sous-agent a {}", dit(s));
                return Ok(0);
            }
            Ok(_) => {}
            // Le transcript date d'apres l'horloge : il vient de bouger.
            Err(apres) => drop(apres),
        }
        std::thread::sleep(PAS);
    }
}

/// Le depot ou l'on est (`git rev-parse --show-toplevel`) ; hors d'un depot, le dossier courant (la seance peut etre
/// lancee d'un de ses parents).
fn depot() -> Result<PathBuf, String> {
    let ici = std::env::current_dir().map_err(|e| format!("le dossier courant ne se lit pas ({})", e))?;
    let sortie = std::process::Command::new("git").args(["rev-parse", "--show-toplevel"]).current_dir(&ici).output().map_err(|e| format!("git ne se lance pas ({})", e))?;
    let lu = String::from_utf8_lossy(&sortie.stdout).trim().to_string();
    Ok(if sortie.status.success() && !lu.is_empty() { PathBuf::from(lu) } else { ici })
}

/// Le commit de HEAD du depot.
fn head(depot: &Path) -> Result<String, String> {
    let sortie = std::process::Command::new("git").arg("-C").arg(depot).args(["rev-parse", "HEAD"]).output().map_err(|e| format!("git ne se lance pas ({})", e))?;
    if !sortie.status.success() {
        return Err(format!("{} : git rev-parse HEAD refuse ({})", depot.display(), String::from_utf8_lossy(&sortie.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&sortie.stdout).trim().to_string())
}

fn dossier_maison() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// Le nom du dossier de transcripts d'un dossier de lancement : tout caractere hors `[A-Za-z0-9]` devient `-`
/// (`/home/user/PuppetPaie` donne `-home-user-PuppetPaie`).
fn nom_du_dossier(chemin: &Path) -> String {
    chemin.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// Le dossier de transcripts de la seance : celui du depot ou d'un de ses parents, le plus recent qui porte des `.jsonl`.
fn dossier_des_transcripts(projets: &Path, depot: &Path) -> Result<Option<PathBuf>, String> {
    let mut retenu: Option<(SystemTime, PathBuf)> = None;
    for p in depot.ancestors().map(|d| projets.join(nom_du_dossier(d))) {
        let mut dates = Vec::new();
        for f in jsonl_de(&p)? {
            dates.push(date(&f)?);
        }
        if let Some(t) = dates.into_iter().max() {
            if retenu.as_ref().is_none_or(|(r, _)| t > *r) {
                retenu = Some((t, p));
            }
        }
    }
    Ok(retenu.map(|(_, p)| p))
}

fn date(f: &Path) -> Result<SystemTime, String> {
    std::fs::metadata(f).and_then(|m| m.modified()).map_err(|e| socle::echec::du_disque(f, &e))
}

fn plus_recent(fichiers: Vec<PathBuf>) -> Result<Option<PathBuf>, String> {
    let mut dates = Vec::new();
    for f in fichiers {
        dates.push((date(&f)?, f));
    }
    Ok(dates.into_iter().max_by_key(|(t, _)| *t).map(|(_, f)| f))
}

/// Les entrees d'un dossier ; un dossier qui n'existe pas n'en a aucune.
fn entrees(dossier: &Path) -> Result<Vec<PathBuf>, String> {
    if !dossier.is_dir() {
        return Ok(Vec::new());
    }
    let mut v = Vec::new();
    for e in std::fs::read_dir(dossier).map_err(|e| socle::echec::du_disque(dossier, &e))? {
        v.push(e.map_err(|e| socle::echec::du_disque(dossier, &e))?.path());
    }
    Ok(v)
}

/// Les `*.jsonl` d'un dossier (pas ses sous-dossiers).
fn jsonl_de(dossier: &Path) -> Result<Vec<PathBuf>, String> {
    Ok(entrees(dossier)?.into_iter().filter(|p| p.extension().is_some_and(|x| x == "jsonl") && p.is_file()).collect())
}

/// Les `*/subagents/*.jsonl` d'un dossier de transcripts.
fn jsonl_des_sous_agents(dossier: &Path) -> Result<Vec<PathBuf>, String> {
    let mut v = Vec::new();
    for p in entrees(dossier)?.into_iter().filter(|p| p.is_dir()) {
        v.extend(jsonl_de(&p.join("subagents"))?);
    }
    Ok(v)
}

/// Le contexte du dernier tour d'un transcript, s'il en porte un : lit la fin du fichier seulement.
fn dernier_usage(chemin: &Path) -> Result<Option<u64>, String> {
    let lire = || -> std::io::Result<(Vec<u8>, bool)> {
        let mut f = std::fs::File::open(chemin)?;
        let taille = f.seek(SeekFrom::End(0))?;
        let debut = taille.saturating_sub(FIN_LUE);
        f.seek(SeekFrom::Start(debut))?;
        let mut octets = Vec::new();
        f.read_to_end(&mut octets)?;
        Ok((octets, debut > 0))
    };
    let (octets, coupee) = lire().map_err(|e| socle::echec::du_disque(chemin, &e))?;
    dernier_usage_d_octets(&octets, coupee, chemin)
}

/// UNE LIGNE DE TRANSCRIPT, telle que Claude Code l'ecrit : le guet n'en lit que `message.usage`. Le transcript n'est
/// pas une forme du logiciel ; ses autres cles (et celles de `message`) changent d'une version de Claude Code a
/// l'autre : elles se passent, nommees nulle part.
struct LigneDeTranscript {
    usage: Option<Usage>,
}

/// L'usage d'un tour : les jetons d'entree, et ceux du cache qu'il a crees ou lus. Ses autres cles (les jetons de
/// sortie, le detail du cache...) se passent.
#[derive(Default)]
struct Usage {
    entree: Option<u64>,
    cache_cree: Option<u64>,
    cache_lu: Option<u64>,
}

impl Usage {
    /// Le contexte du tour : ce que le tour a lu, les jetons qu'il ne dit pas ne comptant pas.
    fn contexte(&self) -> u64 {
        [self.entree, self.cache_cree, self.cache_lu].into_iter().flatten().sum()
    }
}

/// La seule cle `cle` d'un objet, lue en `T` ; les autres se passent.
fn une_cle<'de, A: MapAccess<'de>, T: Deserialize<'de>>(mut m: A, cle: &str) -> Result<Option<T>, A::Error> {
    let mut lu = None;
    while let Some(k) = m.next_key::<String>()? {
        if k == cle {
            lu = Some(m.next_value::<T>()?);
        } else {
            m.next_value::<IgnoredAny>()?;
        }
    }
    Ok(lu)
}

/// `message`, dont le guet ne lit que `usage`.
struct Message(Option<Usage>);

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Message, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Message;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("un message")
            }
            fn visit_map<A: MapAccess<'de>>(self, m: A) -> Result<Message, A::Error> {
                Ok(Message(une_cle(m, "usage")?))
            }
        }
        d.deserialize_map(V)
    }
}

impl<'de> Deserialize<'de> for LigneDeTranscript {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<LigneDeTranscript, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = LigneDeTranscript;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("une ligne de transcript")
            }
            fn visit_map<A: MapAccess<'de>>(self, m: A) -> Result<LigneDeTranscript, A::Error> {
                let message: Option<Message> = une_cle(m, "message")?;
                Ok(LigneDeTranscript { usage: message.and_then(|m| m.0) })
            }
        }
        d.deserialize_map(V)
    }
}

impl<'de> Deserialize<'de> for Usage {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Usage, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Usage;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("un usage")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Usage, A::Error> {
                let mut u = Usage::default();
                while let Some(k) = m.next_key::<String>()? {
                    match k.as_str() {
                        "input_tokens" => u.entree = Some(m.next_value()?),
                        "cache_creation_input_tokens" => u.cache_cree = Some(m.next_value()?),
                        "cache_read_input_tokens" => u.cache_lu = Some(m.next_value()?),
                        _ => {
                            m.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(u)
            }
        }
        d.deserialize_map(V)
    }
}

/// Le contexte du dernier tour que portent des lignes JSON (une ligne sans usage n'est pas un tour). Quand la lecture
/// commence au milieu du fichier (`coupee_en_tete`), sa premiere ligne est coupee ; une derniere ligne sans fin de ligne
/// s'ecrit encore : elles ne se lisent pas, elles ne comptent pas.
fn dernier_usage_d_octets(octets: &[u8], coupee_en_tete: bool, chemin: &Path) -> Result<Option<u64>, String> {
    let lignes: Vec<&[u8]> = octets.split(|&b| b == b'\n' || b == b'\r').collect();
    let en_cours = !octets.ends_with(b"\n");
    for (i, ligne) in lignes.iter().enumerate().rev() {
        let incomplete = (i == 0 && coupee_en_tete) || (i + 1 == lignes.len() && en_cours);
        if incomplete || !ligne.windows(7).any(|w| w == b"\"usage\"") {
            continue;
        }
        let texte = String::from_utf8_lossy(ligne);
        let lue: LigneDeTranscript = socle::forme::lire_texte(&texte, &format!("{} : une ligne du transcript ne se lit pas.", chemin.display()))?;
        if let Some(u) = lue.usage {
            return Ok(Some(u.contexte()));
        }
    }
    Ok(None)
}
