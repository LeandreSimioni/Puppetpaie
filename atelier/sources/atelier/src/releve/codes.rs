//! `moteur-atelier releve-codes moissonner|rapport|comparer <sources> [code] [--reconstruire]` : les articles de loi dont la paie a
//! besoin, releves chez la DILA. Un outil de l'atelier : `<sources>` est `atelier/reference/sources` du depot.
//!
//! POURQUOI. Legifrance refuse les robots (403) : un releve A LA MAIN, article par article,
//! marche une fois et se perime en silence. La base LEGI est publique en open data (`echanges.dila.gouv.fr/OPENDATA/LEGI/`),
//! exactement comme KALI pour les conventions collectives : une archive globale et une livraison par soir. Le releve
//! devient donc RE-VERIFIABLE automatiquement, comme la table des parametres du BOSS et celle des CTP de l'Urssaf.
//!
//! PLUSIEURS TEXTES, UN SEUL PASSAGE. Le code du travail dit ce qu'une indemnite de rupture VAUT ; le code de
//! la securite sociale dit ce qu'elle SUPPORTE, et le code general des impots ce qu'elle coute a celui qui la recoit. Tous
//! se lisent dans la meme archive : ce qui coute, c'est de faire defiler 1,1 Go, pas de reconnaitre un article de plus au
//! passage. Le dossier donne en argument est celui des SOURCES ; chaque texte y ecrit dans le sien (`textes::TEXTES`).
//!
//! `moissonner` TIENT UN ENTREPOT A JOUR : `legi-entrepot/` dans le dossier des sources garde TOUS les
//! articles en vigueur des textes suivis et la liste des livraisons deja appliquees (`entrepot_dila`). La premiere fois
//! (ou avec `--reconstruire`), l'archive globale defile -- 1,1 Go, un quart d'heure --, puis les livraisons depuis sa date ;
//! ensuite, seules les livraisons nouvelles se telechargent (quelques secondes pour une semaine). Ajouter un article a
//! `TEXTES` ne demande plus aucun telechargement : il est deja dans l'entrepot. `rapport` ecrit le document lisible a
//! partir de ce qui a ete moissonne.
//!
//! `comparer` REPOND A LA QUESTION QUE `moissonner` POSE. « Huit articles ont change de version » ne dit pas
//! si le DROIT a change : un article dont un alinea etranger a la paie est reecrit recoit un identifiant neuf sans que rien
//! de ce qui nous concerne bouge. Il faut donc relire chaque article contre sa version d'AVANT -- et cette version-la ne se
//! trouve nulle part ailleurs que dans l'archive globale. Le releve ne garde que la version en vigueur ; l'historique Git
//! ne l'a pas non plus, parce qu'un releve precedent portait deja, lui aussi, la version d'apres. Comptez un quart
//! d'heure : `comparer` s'arrete des qu'il les a toutes.
//!
//! LE PIEGE PROPRE A `comparer` : on cherche par IDENTIFIANT, jamais par numero. L'archive globale porte plusieurs
//! versions du meme article ; demander « L242-1 » y rapporte un paquet de redactions dont on ne saurait laquelle etait en
//! vigueur. Ce qu'on veut n'est pas « une vieille version », c'est CELLE que le releve precedent tenait pour en vigueur --
//! celle que le releve precedent nomme sous `avant` (`avant`, plus bas).
//!
//! TROIS PIEGES, payes tous les trois (chacun est rappele la ou le code l'evite, dans `passer`) :
//!
//!   1. le dossier du dump s'appelle `code_en_vigueur`, mais il porte AUSSI les versions anterieures de chaque article.
//!      La version qui vaut est celle dont `<ETAT>` est VIGUEUR et `<DATE_FIN>` 2999-01-01 -- et `etat` existe aussi
//!      comme ATTRIBUT, sur d'autres balises, ce qui fait attraper n'importe quoi. Premiere version du moissonneur : sept
//!      articles sur cinquante-cinq gardes dans une version perimee, dont L3141-3 (les 2,5 jours de conges) et R1234-2
//!      (l'indemnite de licenciement) ;
//!   2. un article modifie recoit un NOUVEL identifiant de version. On range donc l'entrepot par NUMERO d'article, pas
//!      par identifiant ;
//!   3. un numero d'article n'appartient a aucun code par lui-meme. `L242-1` est un article du code de la securite
//!      sociale, et rien n'interdit a un autre code d'en porter un du meme nom. Ce qui tranche, c'est le LEGITEXT dans le
//!      chemin du fichier -- pas le numero.
//!
//! Les documents ecrits nomment la commande (`moteur releve-codes`) ; l'entrepot est une archive compressee
//! (`entrepot_dila`) ; la date du jour se prend dans
//! `PUPPETPAIE_AUJOURDHUI` quand elle est posee (`contexte::aujourdhui_sans_note`, en temps universel sinon).

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::releve::entrepot_dila::{self as dila, dire, Entrepot, LecteurTar, Source};
use crate::EnOrdre;
use socle::date::Jour;

mod difflib;
pub mod textes;

pub use textes::{Domaine, Texte, TEXTES};

use socle::echec::{Echec, R};

pub const BASE: &str = "https://echanges.dila.gouv.fr/OPENDATA/LEGI/";
pub const GLOBAL: &str = "Freemium_legi_global_20250713-140000.tar.gz";
pub const DEPUIS: &str = "20250713";
/// Le sous-dossier des pieces justificatives.
pub const XML: &str = "xml";
/// Les versions d'avant, a cote des pieces (`comparer`).
pub const ANTERIEUR: &str = "xml-anterieur";
/// L'entrepot des textes suivis, a cote des releves.
pub const ENTREPOT: &str = "legi-entrepot";

const USAGE: &str = "usage : moteur-atelier releve-codes moissonner <sources> [--reconstruire]
       moteur-atelier releve-codes rapport <sources> [code]
       moteur-atelier releve-codes comparer <sources>

Les articles de loi dont la paie a besoin, releves dans la base LEGI de la DILA (open data). <sources> est le dossier des
sources du depot (atelier/reference/sources) ; chaque texte y ecrit dans le sien. `moissonner` tient `legi-entrepot/` a
jour (seules les livraisons nouvelles se telechargent ; `--reconstruire` refait defiler l'archive globale, 1,1 Go) puis
ecrit `releve.json` et `xml/` ; `rapport` ecrit `articles-<date>.md` ; `comparer` relit chaque article change contre sa
version d'avant, dans l'archive globale.";

/// UN ARTICLE DE LOI, tel que l'entrepot le garde (une ligne de `articles.jsonl.gz`) : sa cle (texte, numero), son
/// identifiant de version, le jour depuis lequel elle vaut (quand le XML le dit), si elle est en vigueur, et son XML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticleLegi {
    cle: Vec<String>,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    debut: Option<String>,
    en_vigueur: bool,
    xml: String,
}

impl dila::Article for ArticleLegi {
    fn cle(&self) -> &[String] {
        &self.cle
    }
}

/// L'ETAT DE L'ENTREPOT LEGI (`etat.json`, cles triees) : les changements de version notes par les livraisons, le jour
/// de sa construction, l'archive globale lue, les livraisons appliquees, le jour de la derniere sauvegarde.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EtatLegi {
    changes: Vec<ChangeLegi>,
    construit_le: String,
    global: String,
    lues: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mis_a_jour_le: Option<String>,
}

impl dila::Etat for EtatLegi {
    fn lues(&self) -> &[String] {
        &self.lues
    }
    fn lues_mut(&mut self) -> &mut Vec<String> {
        &mut self.lues
    }
    fn dater(&mut self, aujourdhui: &str) {
        self.mis_a_jour_le = Some(aujourdhui.to_string());
    }
}

/// Une livraison qui remplace la version d'un article : laquelle (`avant` -> `apres`), depuis quand, et quand (champs
/// dans l'ordre alphabetique : `etat.json` et `releve.json` les ecrivent ainsi).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangeLegi {
    apres: String,
    article: String,
    avant: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    depuis: Option<String>,
    livraison: String,
    texte: String,
}

type EntrepotLegi = Entrepot<ArticleLegi, EtatLegi>;

/// LE RELEVE D'UN TEXTE (`<texte>/releve.json`) : d'ou il vient, ou en est la lecture des livraisons, ce qui a change
/// depuis l'archive globale, et ses articles voulus.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleveDUnTexte {
    source: String,
    global: String,
    texte: String,
    legitext: String,
    livraisons_lues: usize,
    livraisons_totales: usize,
    /// Ce qui reste a lire quand un passage s'interrompt : le document le dira, plutot que de se donner pour complet.
    complet: bool,
    derniere_livraison: Option<String>,
    changes_depuis_le_global: Vec<ChangeLegi>,
    releve_le: String,
    articles: EnOrdre<ArticleReleve>,
}

/// Un article d'un releve : sa version, le jour depuis lequel elle vaut, si elle est en vigueur (son XML est a cote).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArticleReleve {
    #[serde(skip_serializing_if = "Option::is_none")]
    debut: Option<String>,
    en_vigueur: bool,
    id: String,
}

fn texte_de(cle: &str) -> Option<&'static Texte> {
    TEXTES.iter().find(|t| t.cle == cle)
}

/// LEGITEXT -> cle du texte.
fn par_legitext() -> Vec<(&'static str, &'static str)> {
    TEXTES.iter().map(|t| (t.legitext, t.cle)).collect()
}

/// Cle du texte -> les numeros qu'on y cherche.
fn tous() -> &'static HashMap<&'static str, HashSet<&'static str>> {
    static T: OnceLock<HashMap<&'static str, HashSet<&'static str>>> = OnceLock::new();
    T.get_or_init(|| TEXTES.iter().map(|t| (t.cle, t.domaines.iter().flat_map(|d| d.numeros.iter().copied()).collect())).collect())
}

fn voulu(texte: &str, num: &str) -> bool {
    tous().get(texte).is_some_and(|n| n.contains(num))
}

fn motif(m: &'static OnceLock<Regex>, s: &str) -> &'static Regex {
    m.get_or_init(|| Regex::new(s).expect("motif"))
}

fn re_num() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    motif(&M, r"<NUM>([^<]*)</NUM>")
}
fn re_etat() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    motif(&M, r"<ETAT>([^<]*)</ETAT>")
}
fn re_debut() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    motif(&M, r"<DATE_DEBUT>([^<]*)</DATE_DEBUT>")
}
fn re_fin() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    motif(&M, r"<DATE_FIN>([^<]*)</DATE_FIN>")
}
fn re_id() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    motif(&M, r"(LEGIARTI\d+)\.xml$")
}
fn re_texte() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    motif(&M, r"(?s)<BLOC_TEXTUEL>(.*?)</BLOC_TEXTUEL>")
}

fn groupe<'a>(m: &Regex, t: &'a str) -> Option<&'a str> {
    m.captures(t).and_then(|c| c.get(1)).map(|g| g.as_str())
}

// ------------------------------------------------------------------ la ligne de commande

/// `moteur-atelier releve-codes ...` : rend le code de sortie.
pub fn commande(args: &[String]) -> i32 {
    if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help" || a == "--aide") {
        eprintln!("{}", USAGE);
        return 2;
    }
    let racine = Path::new(&args[1]);
    let aujourdhui = match droit::contexte::aujourdhui_sans_note() {
        Ok(j) => j,
        Err(m) => {
            eprintln!("{}", m.en_lignes());
            return 1;
        }
    };
    let r = match args[0].as_str() {
        "moissonner" => moissonner(racine, args.iter().any(|a| a == "--reconstruire"), &dila::Reseau, &aujourdhui),
        "rapport" => rapport(racine, args.get(2).map(String::as_str), &aujourdhui),
        "comparer" => comparer(racine, &dila::Reseau, &aujourdhui),
        v => {
            eprintln!("Verbe inconnu : {}\n\n{}", v, USAGE);
            return 2;
        }
    };
    match r {
        Ok(()) => 0,
        Err(m) => {
            eprintln!("{}", m.en_lignes());
            1
        }
    }
}

// ------------------------------------------------------------------ moissonner

/// Une duree en secondes entieres, la moitie au pair, pour le dire.
pub(crate) fn secondes(d: std::time::Duration) -> String {
    match socle::num::quotient(d.as_nanos(), 1_000_000_000, 0) {
        Some(t) => t,
        None => unreachable!("une seconde n'est pas nulle"),
    }
}

/// Fait defiler une archive et garde les articles en vigueur ; rend (fichiers d'article des textes suivis, duree).
///
/// `tar` : une archive ouverte (en flux pour la globale, en memoire pour une livraison). `tous` : garder TOUS les articles
/// des textes suivis -- c'est ce que fait l'entrepot --, et non les seuls voulus.
///
/// `changes` : chaque fois qu'une livraison remplace la version d'un article, on note laquelle et quand. C'est la seule
/// chose qui compte au deuxieme passage -- « aucun des articles n'a bouge » est une phrase qui doit se prouver, pas se
/// supposer.
///
/// `seuls` : ne regarder que ces textes-la (des cles de `TEXTES`) -- c'est ce que fait `completer` pour un texte ajoute
/// apres la construction de l'entrepot.
fn passer<L: Read>(
    tar: &mut LecteurTar<L>,
    articles: &mut EnOrdre<ArticleLegi>,
    journal: &mut Vec<String>,
    mut changes: Option<&mut Vec<ChangeLegi>>,
    livraison: &str,
    tous_les_articles: bool,
    seuls: Option<&[&str]>,
) -> R<(usize, std::time::Duration)> {
    let suivis: Vec<(&str, &str)> = par_legitext().into_iter().filter(|(_, cle)| seuls.is_none_or(|s| s.contains(cle))).collect();
    let (mut vus, t0) = (0usize, Instant::now());
    while let Some(m) = tar.suivant()? {
        if !m.fichier {
            continue;
        }
        if m.nom.ends_with(".dat") && m.nom.contains("suppression") {
            // CE QUE LA LIVRAISON RETIRE : un article dont la version gardee est dans la liste sort de l'entrepot.
            let liste = String::from_utf8_lossy(&tar.lire()?).into_owned();
            let retires: HashSet<&str> = socle::texte::lignes_du_texte(&liste)
                .into_iter()
                .map(dila::sans_blancs)
                .filter(|l| !l.is_empty())
                .map(|l| match l.rsplit_once('/') {
                    Some((_, fin)) => fin,
                    None => l,
                })
                .collect();
            for (_, a) in articles.0.iter().filter(|(_, a)| retires.contains(a.id.as_str())) {
                journal.push(format!("retire par {} : {}", livraison, a.cle.join(" ")));
            }
            articles.0.retain(|(_, a)| !retires.contains(a.id.as_str()));
            continue;
        }
        if !m.nom.contains("/article/") {
            continue;
        }
        // LE NUMERO NE SUFFIT PAS A DIRE DE QUEL CODE IL EST (piege 3) : "L242-1" existe au code de la securite sociale, et
        // rien n'interdit qu'il existe ailleurs. C'est le chemin du fichier, qui porte le LEGITEXT, qui tranche -- et il
        // tranche AVANT qu'on lise le contenu.
        let Some(texte) = suivis.iter().find(|(lt, _)| m.nom.contains(lt)).map(|(_, cle)| *cle) else {
            continue;
        };
        vus += 1;
        let txt = String::from_utf8_lossy(&tar.lire()?).into_owned();
        let Some(num) = groupe(re_num(), &txt).map(|n| dila::sans_blancs(n).to_string()) else {
            continue;
        };
        if !(tous_les_articles || voulu(texte, &num)) {
            continue;
        }
        // PIEGE 1 : `code_en_vigueur` porte aussi les versions anterieures. La version qui vaut a `<ETAT>` VIGUEUR et
        // `<DATE_FIN>` 2999-01-01 -- la BALISE, pas l'attribut `etat` d'autres balises.
        let en_vigueur = groupe(re_etat(), &txt) == Some("VIGUEUR") && groupe(re_fin(), &txt) == Some("2999-01-01");
        let debut = groupe(re_debut(), &txt).map(str::to_string);
        // PIEGE 2 : l'entrepot range par (texte, NUMERO) ; une version neuve remplace l'ancienne sous la meme cle.
        let k = dila::cle(&[texte, &num]);
        let ancien = articles.valeur(&k);
        if ancien.is_some_and(|a| a.en_vigueur) && !en_vigueur {
            continue;
        }
        let id = groupe(re_id(), &m.nom).ok_or_else(|| refus!("{} : un article dont le nom ne porte pas son identifiant LEGIARTI", m.nom))?.to_string();
        if let (Some(ch), Some(a)) = (changes.as_deref_mut(), ancien) {
            if a.id != id {
                ch.push(ChangeLegi { texte: texte.to_string(), article: num.clone(), avant: a.id.clone(), apres: id.clone(), depuis: debut.clone(), livraison: livraison.to_string() });
            }
        }
        articles.poser(k, ArticleLegi { cle: vec![texte.to_string(), num], id, debut, en_vigueur, xml: txt });
    }
    Ok((vus, t0.elapsed()))
}

/// Les livraisons de l'index posterieures a l'archive globale. Un index qui n'en liste aucune n'est plus celui qu'on
/// lit : la page a change.
fn livraisons(source: &dyn Source) -> R<Vec<String>> {
    let page = dila::page(source, BASE)?;
    let toutes = dila::livraisons(&page, "LEGI");
    if toutes.is_empty() {
        return Err(refus!("l'index LEGI ({}) ne liste aucune livraison : la page a change ?", BASE));
    }
    Ok(toutes.into_iter().filter(|n| n[..8] > *DEPUIS).collect())
}

/// L'archive globale en flux, rejouee trois fois : elle fait 1,1 Go, et une coupure de transport n'est pas une reponse.
fn passer_le_global(
    ent: &mut EntrepotLegi,
    journal: &mut Vec<String>,
    source: &dyn Source,
    seuls: Option<&[&str]>,
) -> R<(usize, std::time::Duration)> {
    let mut essai = 1;
    loop {
        let r = source.flux(&format!("{}{}", BASE, GLOBAL)).and_then(|f| passer(&mut dila::tar_gz(f), &mut ent.articles, journal, None, "", true, seuls));
        match r {
            Ok(v) => return Ok(v),
            Err(e) => {
                dire(&format!("  archive globale, essai {} : {}", essai, e.phrase));
                if essai == 3 {
                    return Err(e);
                }
                // Ce qui a ete garde de ce passage-la ne vaut rien : on le retire avant de rejouer.
                match seuls {
                    None => ent.articles.0.clear(),
                    Some(s) => ent.articles.0.retain(|(_, a)| !s.contains(&a.cle[0].as_str())),
                }
                source.pause(10 * essai);
                essai += 1;
            }
        }
    }
}

/// Tenir l'entrepot a jour, puis en tirer les articles voulus. `racine` est le dossier des sources ; chaque texte y ecrit
/// dans le sien.
///
/// LA PREMIERE FOIS SEULEMENT (ou avec `reconstruire`), l'archive globale defile -- 1,1 Go, un quart d'heure -- et
/// l'entrepot garde TOUS les articles en vigueur des textes suivis, pas seulement les voulus. Ensuite, seules les
/// livraisons pas encore appliquees se telechargent, plusieurs a la fois, et s'appliquent dans l'ordre. Ajouter un article
/// a `TEXTES` ne coute plus rien : il est deja dans l'entrepot.
pub fn moissonner(racine: &Path, reconstruire: bool, source: &dyn Source, aujourdhui: &Jour) -> R<()> {
    let jour = aujourdhui.iso();
    std::fs::create_dir_all(racine).map_err(|e| refus!("{} : {}", racine.display(), e))?;
    let dossier = racine.join(ENTREPOT);
    let mut journal: Vec<String> = Vec::new();
    let mut ent: EntrepotLegi = if dila::existe(&dossier) && !reconstruire {
        let mut ent: EntrepotLegi = Entrepot::charger(&dossier)?;
        dire(&format!("== entrepot : {} articles, {} livraisons deja appliquees ==", ent.articles.len(), ent.lues().len()));
        // UN TEXTE AJOUTE A `TEXTES` APRES LA CONSTRUCTION (l'arrete du 4 septembre 2025 sur les frais
        // professionnels) n'a aucun article dans l'entrepot : l'archive globale ne l'a pas garde. On la refait defiler pour
        // lui seul, puis les livraisons deja lues.
        let presents: HashSet<String> = ent.articles.0.iter().map(|(_, a)| a.cle[0].clone()).collect();
        let absents: Vec<&str> = TEXTES.iter().map(|t| t.cle).filter(|c| !presents.contains(*c)).collect();
        if !absents.is_empty() {
            completer(&mut ent, &absents, &mut journal, source, &jour)?;
        }
        ent
    } else {
        dire("== archive globale (construction de l'entrepot) ==");
        // L'ARCHIVE GLOBALE EST LE SOCLE : sans elle il n'y a rien a comparer.
        let etat = EtatLegi { changes: Vec::new(), construit_le: jour.clone(), global: GLOBAL.to_string(), lues: Vec::new(), mis_a_jour_le: None };
        let mut ent: EntrepotLegi = Entrepot::neuf(&dossier, etat);
        let (vus, sec) = passer_le_global(&mut ent, &mut journal, source, None)?;
        dire(&format!("  {} fichiers d'article lus en {}s ; {} articles gardes", vus, secondes(sec), ent.articles.len()));
        ent.sauver(&jour)?;
        ent
    };
    let toutes = livraisons(source)?;
    let lues = ent.lues();
    let neuves: Vec<String> = toutes.iter().filter(|n| !lues.contains(*n)).cloned().collect();
    dire(&format!("== {} livraison(s) nouvelle(s) sur {} ==", neuves.len(), toutes.len()));
    let urls: Vec<String> = neuves.iter().map(|n| format!("{}LEGI_{}.tar.gz", BASE, n)).collect();
    let mut changes: Vec<ChangeLegi> = Vec::new();
    let mut panne: Option<Echec> = None;
    dila::en_ordre(source, &urls, 8, |i, octets| {
        let nom = &neuves[i];
        let octets = match octets {
            Ok(o) => o,
            Err(e) => {
                // UNE LIVRAISON MANQUEE ARRETE LA SUITE : appliquer celle du lendemain par-dessus un trou ferait un
                // entrepot faux sans le dire. La prochaine moisson reprendra ici.
                journal.push(format!("livraison {} : {} -- arret, reprise au prochain passage", nom, e.phrase));
                return false;
            }
        };
        if let Err(e) = passer(&mut dila::archive(&octets), &mut ent.articles, &mut journal, Some(&mut changes), nom, true, None) {
            panne = Some(e.reformuler(|p| format!("livraison {} : {}", nom, p)));
            return false;
        }
        ent.noter_lue(nom);
        // ON ECRIT EN CHEMIN, toutes les vingt-cinq. Un releve qui ne dit pas ou il s'est arrete est un releve qu'on croit
        // complet.
        if (i + 1) % 25 == 0 {
            ent.etat.changes.append(&mut changes);
            if let Err(e) = ent.sauver(&jour) {
                panne = Some(e);
                return false;
            }
            dire(&format!("  [{}/{}] {}", i + 1, neuves.len(), nom));
        }
        true
    });
    if let Some(e) = panne {
        return Err(e);
    }
    ent.etat.changes.append(&mut changes);
    ent.sauver(&jour)?;
    let articles: Vec<&ArticleLegi> = ent.articles.0.iter().map(|(_, a)| a).filter(|a| voulu(&a.cle[0], &a.cle[1])).collect();
    let changes_voulus: Vec<&ChangeLegi> = ent.etat.changes.iter().filter(|c| voulu(&c.texte, &c.article)).collect();
    let mut lues: Vec<String> = ent.lues().into_iter().collect();
    lues.sort();
    ecrire(racine, &articles, &lues, toutes.len(), &changes_voulus, &jour)?;
    let gardes: HashSet<(&str, &str)> = articles.iter().map(|a| (a.cle[0].as_str(), a.cle[1].as_str())).collect();
    let mut manque: Vec<String> = TEXTES
        .iter()
        .flat_map(|t| t.domaines.iter().flat_map(|d| d.numeros.iter()).map(move |n| (t.cle, *n)))
        .filter(|cn| !gardes.contains(cn))
        .map(|(c, n)| format!("{} {}", c, n))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    manque.sort();
    let mut perimes: Vec<String> = articles.iter().filter(|a| !a.en_vigueur).map(|a| a.cle.join(" ")).collect();
    perimes.sort();
    let mut vus_changes: Vec<String> = changes_voulus.iter().map(|c| format!("{} ({})", c.article, c.livraison)).collect::<HashSet<_>>().into_iter().collect();
    vus_changes.sort();
    let toutes_ens: HashSet<&String> = toutes.iter().collect();
    dire("== fini ==");
    dire(&format!("  livraisons : {} appliquees sur {}", ent.lues().iter().filter(|l| toutes_ens.contains(l)).count(), toutes.len()));
    dire(&format!("  manquants : {}", if manque.is_empty() { "aucun".to_string() } else { manque.join(" ") }));
    // LE GARDE-FOU : un article garde dans une version perimee ferait appliquer un droit abroge sans que rien ne le signale.
    dire(&format!("  perimes   : {}", if perimes.is_empty() { "aucun".to_string() } else { perimes.join(" ") }));
    dire(&format!("  changes   : {}", if vus_changes.is_empty() { "aucun".to_string() } else { vus_changes.join(" ") }));
    for l in journal.iter().take(20) {
        dire(&format!("  ! {}", l));
    }
    Ok(())
}

/// Les textes de `absents`, lus dans l'archive globale puis dans chaque livraison deja appliquee a l'entrepot, dans
/// l'ordre : l'entrepot les tient ensuite comme s'il les avait gardes des le depart. Le cout est celui d'une construction
/// (l'archive globale en flux, puis les livraisons), mais une fois par texte ajoute.
fn completer(ent: &mut EntrepotLegi, absents: &[&str], journal: &mut Vec<String>, source: &dyn Source, jour: &str) -> R<()> {
    dire(&format!("== textes ajoutes depuis la construction : {} ==", absents.join(", ")));
    let (vus, sec) = passer_le_global(ent, journal, source, Some(absents))?;
    dire(&format!("  {} fichiers d'article de ces textes lus en {}s", vus, secondes(sec)));
    let mut lues: Vec<String> = ent.lues().into_iter().collect();
    lues.sort();
    let urls: Vec<String> = lues.iter().map(|n| format!("{}LEGI_{}.tar.gz", BASE, n)).collect();
    let mut panne: Option<Echec> = None;
    dila::en_ordre(source, &urls, 8, |i, octets| {
        let r = octets.and_then(|o| passer(&mut dila::archive(&o), &mut ent.articles, journal, None, &lues[i], true, Some(absents)));
        if let Err(e) = r {
            // Une livraison manquee laisserait ces textes dans une version ancienne sans le dire : on s'arrete, et on
            // n'ecrit rien.
            panne = Some(e.reformuler(|p| format!("livraison {} : {} -- les textes {} ne sont pas completes", lues[i], p, absents.join(", "))));
            return false;
        }
        true
    });
    if let Some(e) = panne {
        return Err(e);
    }
    ent.sauver(jour)?;
    dire(&format!("  {} livraisons rejouees pour ces textes", lues.len()));
    Ok(())
}

/// Un releve par texte, chacun dans son dossier.
///
/// CHAQUE DOSSIER SE LIT SEUL : son `releve.json` ne parle que de ses articles, et le nombre de livraisons lues y est
/// repete. Un texte dont le releve ne dirait ou il en est que par le dossier d'a cote serait un releve qu'on croit complet.
fn ecrire(racine: &Path, articles: &[&ArticleLegi], lues: &[String], totales: usize, changes: &[&ChangeLegi], jour: &str) -> R<()> {
    for texte in TEXTES {
        let dossier = racine.join(texte.dossier);
        // Le XML de chaque article dans `xml/`, le releve a cote : c'est la piece justificative qui reste a cote du compte
        // rendu.
        let px = dossier.join(XML);
        std::fs::create_dir_all(&px).map_err(|e| refus!("{} : {}", px.display(), e))?;
        let mut siens = EnOrdre::new();
        for a in articles.iter().filter(|a| a.cle[0] == texte.cle) {
            let num = &a.cle[1];
            let f = px.join(format!("{}.xml", fichier(num)));
            std::fs::write(&f, &a.xml).map_err(|e| refus!("{} : {}", f.display(), e))?;
            siens.poser(num.clone(), ArticleReleve { debut: a.debut.clone(), en_vigueur: a.en_vigueur, id: a.id.clone() });
        }
        let releve = ReleveDUnTexte {
            source: BASE.to_string(),
            global: GLOBAL.to_string(),
            texte: texte.nom.to_string(),
            legitext: texte.legitext.to_string(),
            livraisons_lues: lues.len(),
            livraisons_totales: totales,
            complet: lues.len() == totales,
            derniere_livraison: lues.last().cloned(),
            changes_depuis_le_global: changes.iter().filter(|c| c.texte == texte.cle).map(|c| (*c).clone()).collect(),
            releve_le: jour.to_string(),
            articles: siens,
        };
        let p = dossier.join("releve.json");
        std::fs::write(&p, socle::ecriture::json_indente(&releve, 1)).map_err(|e| refus!("{} : {}", p.display(), e))?;
    }
    Ok(())
}

/// Le nom de fichier d'un article. « 80 duodecies » en est un aussi.
pub fn fichier(num: &str) -> String {
    num.replace(' ', "_").replace('/', "-")
}

// ------------------------------------------------------------------ rapport

/// Le texte d'un article (son `BLOC_TEXTUEL`, s'il en a un), balises en sauts de ligne, entites decodees, lignes vides
/// retirees.
fn contenu(xml: &str) -> Option<String> {
    static BALISE: OnceLock<Regex> = OnceLock::new();
    static VIDES: OnceLock<Regex> = OnceLock::new();
    let t = groupe(re_texte(), xml)?;
    let t = motif(&BALISE, r"<[^>]+>").replace_all(t, "\n");
    let t = dila::unescape_html(&t);
    // Une ligne vide : rien que des blancs d'Unicode et des caracteres \x1c a \x1f.
    let t = motif(&VIDES, r"\n[\s\x1C-\x1F]*\n+").replace_all(&t, "\n");
    Some(dila::sans_blancs(&t).to_string())
}

/// 20250713 -> 13/07/2025. Une date se lit, elle ne se dechiffre pas.
fn jour_lisible(aaaammjj: &str) -> String {
    let c: Vec<char> = aaaammjj.chars().collect();
    if c.len() < 8 || !c[..8].iter().all(|x| x.is_ascii_digit()) {
        return aaaammjj.to_string(); // pas une date : on la rend telle quelle
    }
    let s = |a: usize, b: usize| c[a..b].iter().collect::<String>();
    format!("{}/{}/{}", s(6, 8), s(4, 6), s(0, 4))
}

/// La seule phrase qui compte au deuxieme passage.
///
/// « Aucun n'a change » n'a de valeur que dit par une machine qui a reellement passe les livraisons -- et il faut alors
/// dire combien.
fn ce_qui_a_change(meta: &ReleveDUnTexte) -> String {
    if meta.changes_depuis_le_global.is_empty() {
        return format!("**aucun de ces {} articles n'a change de version** depuis l'archive globale", meta.articles.len());
    }
    format!(
        "**{} article(s) ont change de version** : {}",
        meta.changes_depuis_le_global.len(),
        meta.changes_depuis_le_global
            .iter()
            .map(|c| format!("{} ({} -> {}, livraison du {})", c.article, c.avant, c.apres, jour_lisible(dila::premiers_caracteres(&c.livraison, 8))))
            .collect::<Vec<_>>()
            .join(" ; ")
    )
}

/// Le document lisible de chaque texte : un titre par domaine, un article par entree. `quel` limite a un seul texte.
pub fn rapport(racine: &Path, quel: Option<&str>, aujourdhui: &Jour) -> R<()> {
    for texte in TEXTES {
        if quel.is_some_and(|q| texte.cle != q) {
            continue;
        }
        let dossier = racine.join(texte.dossier);
        let cible = dossier.join(format!("articles-{}.md", aujourdhui.aaaammjj()));
        rapport_d_un_texte(&dossier, &cible, texte)?;
    }
    Ok(())
}

/// Un fichier texte en UTF-8, fins de ligne universelles (`\r\n` et `\r` deviennent `\n`).
fn lire_texte(p: &Path) -> R<String> {
    let t = std::fs::read_to_string(p).map_err(|e| refus!("{} : {}", p.display(), e))?;
    Ok(if t.contains('\r') { t.replace("\r\n", "\n").replace('\r', "\n") } else { t })
}

/// Le releve d'un texte (`releve.json`), en sa forme.
fn lire_releve(p: &Path) -> R<ReleveDUnTexte> {
    socle::forme::lire_texte(&lire_texte(p)?, &format!("{} ne se lit pas.", p.display()))
}

fn rapport_d_un_texte(dossier: &Path, cible: &Path, texte: &Texte) -> R<()> {
    let meta = lire_releve(&dossier.join("releve.json"))?;
    let n_articles = meta.articles.len();
    let (lues, totales) = (meta.livraisons_lues, meta.livraisons_totales);
    let mut lignes: Vec<String> = vec![
        format!("# {}", texte.titre),
        String::new(),
        "**Releve automatiquement**, et c'est l'interet : un article lu a la main sur Legifrance -- qui refuse les robots \
         (403) -- se perime en silence. Ceux-ci viennent de la base **LEGI** de la DILA, en open data, par \
         `moteur releve-codes` -- la meme voie que KALI pour les conventions collectives (§29). Le releve se REFAIT, et \
         c'est tout le sujet : il dit ce qui a bouge depuis la derniere fois."
            .to_string(),
        String::new(),
        "| | |".into(),
        "|---|---|".into(),
        format!("| source | `{}` |", meta.source),
        format!("| archive globale | `{}` |", meta.global),
        format!("| code | {} (`{}`) |", texte.nom, texte.legitext),
        format!("| livraisons lues | **{} sur {}**, dans l'ordre depuis le {} |", lues, totales, jour_lisible(DEPUIS)),
        format!("| ce qu'elles disent | {} |", ce_qui_a_change(&meta)),
        format!("| releve le | {} |", meta.releve_le),
        format!("| articles | {} |", n_articles),
        String::new(),
    ];
    // CE QUI RESTE A LIRE, quand un passage s'est interrompu. Un releve qui tairait la difference se donnerait pour
    // complet, et c'est precisement ce qu'on ne veut pas d'une source.
    if !meta.complet {
        let reste = totales as i64 - lues as i64;
        let au = match &meta.derniere_livraison {
            Some(l) => format!("au {}", jour_lisible(dila::premiers_caracteres(l, 8))),
            None => "avant toute livraison".to_string(),
        };
        lignes.push(format!(
            "**PASSAGE INCOMPLET.** Il reste **{} livraisons** a passer -- les plus recentes. Ce qui suit est donc vrai {} \
             pour les {} premieres ; `moteur releve-codes moissonner` le refait en entier, et c'est tout l'interet : un \
             releve a la main ne se refait pas.",
            reste, au, lues
        ));
        lignes.push(String::new());
    }
    lignes.push(
        "**ET LA VERSION EST VERIFIEE.** Le dossier du dump s'appelle `code_en_vigueur`, mais il porte AUSSI les versions \
         anterieures de chaque article : la premiere version du moissonneur en a garde sept sur cinquante-cinq dans une \
         version PERIMEE, dont L3141-3 du code du travail (les 2,5 jours de conges) et R1234-2 (l'indemnite de \
         licenciement). Chaque article ci-dessous est verifie en vigueur -- `<ETAT>` a VIGUEUR et `<DATE_FIN>` a \
         2999-01-01 --, et celui qui ne le serait pas est signale a sa ligne."
            .into(),
    );
    lignes.push(String::new());
    lignes.push(
        "Chaque article porte l'identifiant de SA VERSION (`LEGIARTI...`) et la date depuis laquelle elle est en vigueur : \
         c'est ce qui permet de dire, au prochain relevé, si quelque chose a bouge."
            .into(),
    );
    lignes.push(String::new());
    for domaine in texte.domaines {
        lignes.extend(["---".to_string(), String::new(), format!("## {}", domaine.titre), String::new()]);
        for num in domaine.numeros {
            let Some(a) = meta.articles.valeur(num) else {
                lignes.extend([format!("### {} -- ABSENT DU RELEVE", num), String::new()]);
                continue;
            };
            let xml = lire_texte(&dossier.join(XML).join(format!("{}.xml", fichier(num))))?;
            let etat = if a.en_vigueur { "" } else { "  **VERSION PERIMEE**" };
            match &a.debut {
                Some(d) => lignes.push(format!("### {} (`{}`, en vigueur depuis le {}){}", num, a.id, d, etat)),
                None => lignes.push(format!("### {} (`{}`){}", num, a.id, etat)),
            }
            lignes.push(String::new());
            match contenu(&xml) {
                Some(t) => lignes.push(format!("> {}", t.replace('\n', "\n> "))),
                None => lignes.push("> (l'article n'a pas de bloc textuel)".to_string()),
            }
            lignes.push(String::new());
        }
    }
    let mut t = lignes.join("\n");
    t.push('\n');
    std::fs::write(cible, t).map_err(|e| refus!("{} : {}", cible.display(), e))?;
    dire(&format!("{} articles -> {}", n_articles, cible.display()));
    Ok(())
}

// ------------------------------------------------------------------ comparer

/// Tous les changements de version notes par le dernier `moissonner`, tous textes confondus. C'est le releve qui dit ce
/// qu'il y a a comparer : on ne redecide pas ici quels articles ont bouge.
fn changements(racine: &Path) -> R<Vec<ChangeLegi>> {
    let mut tout = Vec::new();
    for texte in TEXTES {
        let p = racine.join(texte.dossier).join("releve.json");
        if !p.exists() {
            continue;
        }
        tout.extend(lire_releve(&p)?.changes_depuis_le_global);
    }
    Ok(tout)
}

/// Va chercher dans l'archive globale les versions d'AVANT, par leur IDENTIFIANT (le piege propre a `comparer`). Rend
/// {(texte, article): xml} et ecrit chaque piece a cote des autres, dans `<texte>/xml-anterieur/`.
///
/// ON S'ARRETE DES QU'ON LES A TOUTES. L'archive porte 1,75 million d'articles et les versions cherchees ne sont qu'une
/// poignee : sortir au dernier trouve economise un cinquieme du defilement.
fn avant(racine: &Path, changements: &[ChangeLegi], source: &dyn Source) -> R<HashMap<(String, String), String>> {
    let mut vise: HashMap<String, (String, String)> = HashMap::new();
    for c in changements {
        vise.insert(c.avant.clone(), (c.texte.clone(), c.article.clone()));
    }
    let (mut trouves, mut vus, t0) = (HashMap::new(), 0usize, Instant::now());
    let mut tar = dila::tar_gz(source.flux(&format!("{}{}", BASE, GLOBAL))?);
    while let Some(m) = tar.suivant()? {
        if !m.fichier || !m.nom.contains("/article/") {
            continue;
        }
        vus += 1;
        let Some(id) = groupe(re_id(), &m.nom).map(String::from) else {
            continue;
        };
        let Some((cle, num)) = vise.get(&id).cloned() else {
            continue;
        };
        let xml = String::from_utf8_lossy(&tar.lire()?).into_owned();
        let dossier = racine.join(texte_de(&cle).map(|t| t.dossier).ok_or_else(|| refus!("texte inconnu : {}", cle))?).join(ANTERIEUR);
        std::fs::create_dir_all(&dossier).map_err(|e| refus!("{} : {}", dossier.display(), e))?;
        let f = dossier.join(format!("{}.xml", fichier(&num)));
        std::fs::write(&f, &xml).map_err(|e| refus!("{} : {}", f.display(), e))?;
        trouves.insert((cle, num.clone()), xml);
        dire(&format!("  {} {} ({}/{})", num, id, trouves.len(), vise.len()));
        if trouves.len() == vise.len() {
            break;
        }
    }
    dire(&format!(
        "  {} fichiers parcourus en {}s ; {}/{} versions anterieures retrouvees",
        vus,
        secondes(t0.elapsed()),
        trouves.len(),
        vise.len()
    ));
    Ok(trouves)
}

/// Le texte d'un article, alinea par alinea, espaces normalises.
///
/// On compare le BLOC_TEXTUEL et lui seul : ni les visas, ni les liens, ni les notes. Un changement qui n'y figurerait pas
/// ECHAPPE a cette lecture, et le document le dit plutot que de le taire.
fn alineas(xml: &str) -> Vec<String> {
    static ESPACES: OnceLock<Regex> = OnceLock::new();
    contenu(xml)
        .iter()
        .flat_map(|t| t.split('\n'))
        .filter(|l| !dila::sans_blancs(l).is_empty())
        .map(|l| dila::sans_blancs(&motif(&ESPACES, r"[\s\x1C-\x1F]+").replace_all(l, " ")).to_string())
        .collect()
}

/// Relit chaque article qui a change contre sa version d'avant, et ecrit le document des differences.
pub fn comparer(racine: &Path, source: &dyn Source, aujourdhui: &Jour) -> R<()> {
    let changements = changements(racine)?;
    let cible = racine.join(format!("changements-de-version-{}.md", aujourdhui.aaaammjj()));
    if changements.is_empty() {
        // RIEN A COMPARER N'EST PAS RIEN A DIRE : le document existe quand meme, et il date le passage qui n'a rien trouve.
        let t = format!(
            "# Les changements de version : aucun\n\nLe dernier `moissonner` n'a note **aucun** changement de version depuis \
             l'archive globale `{}`. Il n'y a donc rien a relire.\n\n*Etabli le {}.*\n",
            GLOBAL,
            aujourdhui.jj_mm_aaaa()
        );
        std::fs::write(&cible, t).map_err(|e| refus!("{} : {}", cible.display(), e))?;
        dire(&format!("aucun changement -> {}", cible.display()));
        return Ok(());
    }
    dire(&format!("== {} article(s) a relire contre leur version d'avant ==", changements.len()));
    let mut essai = 1;
    let anterieures = loop {
        match avant(racine, &changements, source) {
            Ok(a) => break a,
            Err(e) => {
                dire(&format!("  archive globale, essai {} : {}", essai, e.phrase));
                if essai == 3 {
                    return Err(e);
                }
                source.pause(10 * essai);
                essai += 1;
            }
        }
    };
    ecrire_comparaison(racine, &cible, &changements, &anterieures, aujourdhui)
}

fn ecrire_comparaison(
    racine: &Path,
    cible: &Path,
    changements: &[ChangeLegi],
    anterieures: &HashMap<(String, String), String>,
    aujourdhui: &Jour,
) -> R<()> {
    let mut lignes: Vec<String> = vec![
        "# Ce qui a change DANS LE TEXTE des articles qui ont change de version".into(),
        String::new(),
        format!("*Etabli le {} par `moteur releve-codes comparer`.*", aujourdhui.jj_mm_aaaa()),
        String::new(),
        "Changer de version n'est pas changer de sens : un article dont un alinea etranger a la paie est reecrit recoit un \
         identifiant neuf sans que rien de ce qui nous concerne bouge. Ce document montre, article par article, **ce qui a \
         reellement change dans le texte** -- il ne dit pas ce qu'il faut en faire, ce qui demande une lecture."
            .into(),
        String::new(),
        format!(
            "Les versions d'avant viennent de l'archive globale `{}`, cherchees **par leur identifiant** et non par leur \
             numero : on ne voulait pas une vieille version, on voulait celle que le relevé precedent tenait pour en \
             vigueur. Elles sont gardees dans `<code>/{}/`, a cote des pieces justificatives.",
            GLOBAL, ANTERIEUR
        ),
        String::new(),
        "La comparaison porte sur le `BLOC_TEXTUEL` seul -- ni visas, ni liens, ni notes. **Un changement qui n'y \
         figurerait pas echappe a cette lecture.**"
            .into(),
        String::new(),
    ];
    let mut tries: Vec<&ChangeLegi> = changements.iter().collect();
    tries.sort_by(|a, b| (&a.texte, &a.article).cmp(&(&b.texte, &b.article)));
    for c in tries {
        let (cle, article) = (c.texte.clone(), c.article.clone());
        let texte = texte_de(&cle).ok_or_else(|| refus!("texte inconnu : {}", cle))?;
        let livraison = jour_lisible(dila::premiers_caracteres(&c.livraison, 8));
        let quand = match &c.depuis {
            Some(depuis) => format!("en vigueur depuis le {}, livraison du {}", jour_lisible(dila::premiers_caracteres(&depuis.replace('-', ""), 8)), livraison),
            None => format!("livraison du {}", livraison),
        };
        lignes.extend(["---".to_string(), String::new(), format!("## {} ({})", article, texte.nom), String::new(), format!("`{}` -> `{}`, {}.", c.avant, c.apres, quand), String::new()]);
        let Some(xml_a) = anterieures.get(&(cle.clone(), article.clone())) else {
            // UNE VERSION INTROUVABLE SE DIT. La taire laisserait croire que l'article a ete relu, alors qu'il ne l'a pas ete.
            lignes.extend([
                "**Version anterieure INTROUVABLE dans l'archive globale.** Cet article n'a donc PAS ete relu contre son passe."
                    .to_string(),
                String::new(),
            ]);
            continue;
        };
        let xml_b = lire_texte(&racine.join(texte.dossier).join(XML).join(format!("{}.xml", fichier(&article))))?;
        let (a, b) = (alineas(xml_a), alineas(&xml_b));
        if a == b {
            lignes.extend([format!("**Texte identique** : {} alineas de part et d'autre. Seule la version a change.", a.len()), String::new()]);
            continue;
        }
        lignes.extend([format!("{} alineas -> {}. Ce qui differe :", a.len(), b.len()), String::new(), "```diff".into()]);
        for l in difflib::unifie(&a, &b, "avant", "apres", 1) {
            if !(l.starts_with("---") || l.starts_with("+++") || l.starts_with("@@")) {
                lignes.push(l);
            }
        }
        lignes.extend(["```".to_string(), String::new()]);
    }
    let mut t = lignes.join("\n");
    t.push('\n');
    std::fs::write(cible, t).map_err(|e| refus!("{} : {}", cible.display(), e))?;
    dire(&format!("{} article(s) -> {}", changements.len(), cible.display()));
    Ok(())
}
