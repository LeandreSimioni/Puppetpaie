//! `moteur-atelier releve-conventions mettre-a-jour|article|chercher` : le texte complet des conventions collectives suivies, tenu a
//! jour en local (`kali-entrepot/`). Un outil de l'atelier : `<sources>` est `atelier/reference/sources` du depot.
//!
//! POURQUOI. La veille des conventions (`kali.rs`, `moteur-atelier sources --verifier`) est incrementale, mais elle ne garde que
//! la LISTE des textes de chaque convention -- pas leurs articles ; sans entrepot, relever une clause demanderait de
//! retelecharger la base KALI entiere (180 Mo) et toutes ses livraisons, puis de filtrer a la main. L'entrepot garde, dans `kali-entrepot/`, TOUS les articles des
//! conventions citees par `donnees/sources.json` -- clauses communes, annexes, avenants, toutes versions --, et la liste
//! des livraisons deja appliquees : un article se lit en une seconde, et une mise a jour ne telecharge que les livraisons
//! nouvelles.
//!
//! Un article se rend avec son identifiant KALI, son etat (VIGUEUR_ETEN, VIGUEUR_NON_ETEN, ABROGE...), ses dates et le
//! texte auquel il appartient : c'est ce qu'une transcription dans `atelier/reference/sources/` recopie. Un element que
//! le XML de l'article ne porte pas reste vide (`None`) ; un element present et vide (un chapeau sans numero) est vide.
//!
//! La base entiere ne se relit que pour construire l'entrepot, ou avec `--reconstruire` (une convention nouvelle a suivre,
//! par exemple -- elle se relit d'elle-meme quand le manifeste en cite une que l'entrepot ne tient pas).

use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::releve::entrepot_dila::{self as dila, dire, Entrepot, EtatDuTexteDila, IdDila, LecteurTar, LivraisonDila, Source};
use crate::sources::Manifeste;
use crate::EnOrdre;
use droit::fiche::Idcc;
use socle::date::Jour;

use socle::echec::{Echec, R};

pub const BASE: &str = crate::releve::kali::INDEX;
pub const DOSSIER: &str = "kali-entrepot";

const USAGE: &str = "usage : moteur-atelier releve-conventions mettre-a-jour <sources> [--reconstruire]
       moteur-atelier releve-conventions article <sources> <IDCC> <numero> [...]
       moteur-atelier releve-conventions chercher <sources> <IDCC> <mot>

Le texte complet des conventions collectives suivies (celles que cite logiciel/donnees/sources.json), tenu a jour dans
<sources>/kali-entrepot/ depuis la base KALI de la DILA (open data). `mettre-a-jour` n'applique que les livraisons
nouvelles (`--reconstruire` relit la base entiere, 180 Mo) ; `article` rend les articles en vigueur d'un numero ;
`chercher` liste les articles en vigueur dont le texte porte le mot.";

/// UN ARTICLE D'UNE CONVENTION, tel que l'entrepot le garde (une ligne de `articles.jsonl.gz`) : sa cle (IDCC,
/// identifiant KALIARTI), son numero, son etat, ses dates, son conteneur, le texte auquel il appartient, son chemin de
/// titres et son contenu.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticleKali {
    cle: Vec<String>,
    id: IdDila,
    #[serde(skip_serializing_if = "Option::is_none")]
    num: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    etat: Option<EtatDuTexteDila>,
    #[serde(skip_serializing_if = "Option::is_none")]
    debut: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fin: Option<Jour>,
    conteneur: IdDila,
    #[serde(skip_serializing_if = "Option::is_none")]
    texte: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    titre_texte: Option<String>,
    chemin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    contenu: Option<String>,
}

impl dila::Article for ArticleKali {
    fn cle(&self) -> &[String] {
        &self.cle
    }
}

/// L'ETAT DE L'ENTREPOT KALI (`etat.json`, cles triees) : les changements d'etat ou de texte notes par les livraisons,
/// le jour de sa construction, les conventions tenues (conteneur -> IDCC), l'archive globale lue, les livraisons
/// appliquees, le jour de la derniere sauvegarde.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EtatKali {
    changes: Vec<ChangeKali>,
    construit_le: Jour,
    conteneurs: BTreeMap<String, String>,
    global: String,
    lues: Vec<LivraisonDila>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mis_a_jour_le: Option<Jour>,
}

/// Une livraison qui change l'etat ou le texte d'un article deja tenu (champs dans l'ordre alphabetique).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangeKali {
    #[serde(skip_serializing_if = "Option::is_none")]
    apres: Option<EtatDuTexteDila>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avant: Option<EtatDuTexteDila>,
    id: IdDila,
    idcc: Idcc,
    livraison: LivraisonDila,
    #[serde(skip_serializing_if = "Option::is_none")]
    num: Option<String>,
    texte_change: bool,
}

impl dila::Etat for EtatKali {
    fn lues(&self) -> &[LivraisonDila] {
        &self.lues
    }
    fn lues_mut(&mut self) -> &mut Vec<LivraisonDila> {
        &mut self.lues
    }
    fn dater(&mut self, aujourdhui: Jour) {
        self.mis_a_jour_le = Some(aujourdhui);
    }
}

/// L'entrepot des conventions.
pub type EntrepotKali = Entrepot<ArticleKali, EtatKali>;

/// `{conteneur KALICONT: IDCC}` -- les conventions que `sources.json` cite, et celles qu'il met de cote a relever. Dans
/// l'ordre du manifeste : c'est lui qui decide a quel conteneur un article se rattache quand il en cite plusieurs.
pub fn suivies(manifeste: &Manifeste) -> EnOrdre<String> {
    let mut out = EnOrdre::new();
    for (_, s) in &manifeste.sources.0 {
        if let Some(k) = &s.kali {
            out.poser(k.conteneur.texte().to_string(), k.idcc.to_string());
        }
    }
    // Les conventions que le moteur ne porte pas encore, gardees pour etre relues avant d'ecrire leur couche.
    for (idcc, c) in &manifeste.conventions_a_relever.0 {
        out.poser(c.conteneur.texte().to_string(), idcc.clone());
    }
    out
}

fn re_champ(k: &str) -> &'static Regex {
    static M: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    let t = M.get_or_init(|| {
        ["ID", "NUM", "ETAT", "DATE_DEBUT", "DATE_FIN", "CONTENU"]
            .into_iter()
            .map(|k| (k, Regex::new(&format!(r"(?s)<{}>(.*?)</{}>", k, k)).expect("motif")))
            .collect()
    });
    &t.iter().find(|(n, _)| *n == k).expect("champ connu").1
}

fn motif(m: &'static OnceLock<Regex>, s: &str) -> &'static Regex {
    m.get_or_init(|| Regex::new(s).expect("motif"))
}

/// Un fragment de XML rendu lisible : `<br/>` et `</p>` en sauts de ligne, les autres balises retirees, les entites
/// decodees, les lignes vides ramenees a une.
pub fn nettoyer(x: &str) -> String {
    static SAUTS: OnceLock<Regex> = OnceLock::new();
    static BALISES: OnceLock<Regex> = OnceLock::new();
    static VIDES: OnceLock<Regex> = OnceLock::new();
    let x = motif(&SAUTS, r"<br[\s\x1C-\x1F]*/?>|</p>").replace_all(x, "\n");
    let x = dila::unescape_html(&motif(&BALISES, r"<[^>]+>").replace_all(&x, ""));
    let x = motif(&VIDES, r"\n[\s\x1C-\x1F]*\n+").replace_all(&x, "\n\n");
    dila::sans_blancs(&x).to_string()
}

/// (cle, article) d'un fichier d'article, ou `None` s'il n'est d'aucune convention suivie (ou n'a pas d'identifiant).
/// La cle est (IDCC, identifiant KALIARTI). L'identifiant, l'etat et les dates se lisent ici, a la porte du XML : un
/// element vide n'en dit aucun ; un element qui n'en a pas la forme refuse l'article.
fn lire(xml: &str, conteneurs: &EnOrdre<String>) -> R<Option<(String, ArticleKali)>> {
    static TXT: OnceLock<Regex> = OnceLock::new();
    static TM: OnceLock<Regex> = OnceLock::new();
    let Some((cont, idcc)) = conteneurs.0.iter().find(|(c, _)| xml.contains(c.as_str())) else {
        return Ok(None);
    };
    let g = |k: &str| re_champ(k).captures(xml).and_then(|c| c.get(1)).map(|m| dila::sans_blancs(m.as_str()).to_string());
    let txt = motif(&TXT, r#"(?s)<TITRE_TXT[^>]*id_txt="([^"]*)"[^>]*>(.*?)</TITRE_TXT>"#).captures(xml);
    let Some(ident) = g("ID").filter(|i| !i.is_empty()) else {
        return Ok(None);
    };
    let jour = |k: &str| -> R<Option<Jour>> {
        match g(k).filter(|d| !d.is_empty()) {
            Some(d) => match socle::piece::Zone::lue(&d, socle::piece::format::JourIso).valeur() {
                Some(j) => Ok(Some(j)),
                None => Err(refus!("{} : {} « {} » n'est pas un jour AAAA-MM-JJ", ident, k, d)),
            },
            None => Ok(None),
        }
    };
    let chemin: Vec<String> = motif(&TM, r"(?s)<TITRE_TM[^>]*>(.*?)</TITRE_TM>").captures_iter(xml).map(|c| nettoyer(&c[1])).collect();
    let a = ArticleKali {
        cle: vec![idcc.clone(), ident.clone()],
        id: IdDila::lire(&ident)?,
        num: g("NUM"),
        etat: g("ETAT").filter(|e| !e.is_empty()).map(|e| EtatDuTexteDila::lire(&e)).transpose()?,
        debut: jour("DATE_DEBUT")?,
        fin: jour("DATE_FIN")?,
        conteneur: IdDila::lire(cont)?,
        texte: txt.as_ref().map(|c| c[1].to_string()),
        titre_texte: txt.as_ref().map(|c| nettoyer(&c[2])),
        chemin: chemin.join(" > "),
        contenu: g("CONTENU").map(|c| nettoyer(&c)),
    };
    Ok(Some((dila::cle_de(&a.cle), a)))
}

/// Le dernier segment d'un chemin de la liste des suppressions (l'identifiant retire).
fn dernier_segment(l: &str) -> &str {
    match l.rsplit_once('/') {
        Some((_, fin)) => fin,
        None => l,
    }
}

/// Fait defiler une archive (la base entiere, ou une livraison quand `livraison` est nommee) et range les articles des
/// conventions suivies ; rend le nombre d'articles gardes. Une livraison qui change l'etat ou le texte d'un article deja
/// tenu le note dans `changes`.
fn passer<L: Read>(tar: &mut LecteurTar<L>, articles: &mut EnOrdre<ArticleKali>, changes: &mut Vec<ChangeKali>, conteneurs: &EnOrdre<String>, livraison: Option<&LivraisonDila>) -> R<usize> {
    let mut n = 0;
    while let Some(m) = tar.suivant()? {
        if !m.fichier {
            continue;
        }
        if m.nom.ends_with(".dat") && m.nom.contains("suppression") {
            let liste = String::from_utf8_lossy(&tar.lire()?).into_owned();
            let retires: HashSet<&str> = socle::texte::lignes_du_texte(&liste).into_iter().map(dila::sans_blancs).filter(|l| !l.is_empty()).map(dernier_segment).collect();
            articles.0.retain(|(_, a)| !a.cle.get(1).is_some_and(|id| retires.contains(id.as_str())));
            continue;
        }
        if !m.nom.contains("/article/") {
            continue;
        }
        let xml = String::from_utf8_lossy(&tar.lire()?).into_owned();
        let Some((k, a)) = lire(&xml, conteneurs)? else {
            continue;
        };
        if let (Some(livraison), Some(ancien)) = (livraison, articles.valeur(&k)) {
            if (&ancien.etat, &ancien.contenu) != (&a.etat, &a.contenu) {
                changes.push(ChangeKali {
                    idcc: Idcc::lire(&a.cle[0])?,
                    id: a.id.clone(),
                    num: a.num.clone(),
                    livraison: livraison.clone(),
                    avant: ancien.etat,
                    apres: a.etat,
                    texte_change: ancien.contenu != a.contenu,
                });
            }
        }
        articles.poser(k, a);
        n += 1;
    }
    Ok(n)
}

/// L'archive globale la plus recente de l'index.
fn globale(source: &dyn Source) -> R<String> {
    let page = dila::page(source, BASE)?;
    let m = Regex::new(r"(Freemium_kali_global_\d{8}-\d{6}\.tar\.gz)").expect("motif");
    let mut noms: Vec<String> = m.captures_iter(&page).map(|c| c[1].to_string()).collect();
    noms.sort();
    noms.dedup();
    noms.pop().ok_or_else(|| refus!("aucune archive globale KALI a {}", BASE))
}

/// Tient l'entrepot a jour : la base entiere la premiere fois (ou une convention nouvelle a suivre, ou `reconstruire`),
/// puis les livraisons pas encore appliquees, dans l'ordre.
pub fn mettre_a_jour(racine: &Path, reconstruire: bool, manifeste: &Manifeste, source: &dyn Source, aujourdhui: &Jour) -> R<EntrepotKali> {
    let jour = *aujourdhui;
    let conteneurs = suivies(manifeste);
    let dossier = racine.join(DOSSIER);
    let charge: Option<EntrepotKali> = if dila::existe(&dossier) && !reconstruire { Some(Entrepot::charger(&dossier)?) } else { None };
    let mut ent = match charge {
        Some(e) if conteneurs.0.iter().all(|(c, _)| e.etat.conteneurs.contains_key(c)) => e,
        _ => {
            // UNE CONVENTION NOUVELLE A SUIVRE, ou la premiere fois : la base entiere defile une fois (180 Mo), puis les
            // livraisons depuis sa date.
            let glob = globale(source)?;
            dire(&format!("== base KALI entiere : {} ==", glob));
            let etat = EtatKali {
                changes: Vec::new(),
                construit_le: jour,
                conteneurs: conteneurs.0.iter().cloned().collect(),
                global: glob.clone(),
                lues: Vec::new(),
                mis_a_jour_le: None,
            };
            let mut ent: EntrepotKali = Entrepot::neuf(&dossier, etat);
            let t0 = Instant::now();
            let n = passer(&mut dila::tar_gz(source.flux(&format!("{}{}", BASE, glob))?), &mut ent.articles, &mut ent.etat.changes, &conteneurs, None)?;
            dire(&format!("  {} articles gardes en {}s", n, crate::releve::codes::secondes(t0.elapsed())));
            ent.sauver(jour)?;
            ent
        }
    };
    let date_globale = Regex::new(r"(\d{8})")
        .expect("motif")
        .captures(&ent.etat.global)
        .map(|c| c[1].to_string())
        .ok_or_else(|| refus!("{}/etat.json : « global » sans date ({})", DOSSIER, ent.etat.global))?;
    let toutes: Vec<LivraisonDila> = dila::livraisons(&dila::page(source, BASE)?, "KALI")?.into_iter().filter(|l| l.jour().aaaammjj() >= date_globale).collect();
    let lues = ent.lues();
    let neuves: Vec<LivraisonDila> = toutes.iter().filter(|l| !lues.contains(*l)).cloned().collect();
    dire(&format!("== {} livraison(s) nouvelle(s) sur {} ==", neuves.len(), toutes.len()));
    let urls: Vec<String> = neuves.iter().map(|l| format!("{}KALI_{}.tar.gz", BASE, l)).collect();
    let mut panne: Option<Echec> = None;
    dila::en_ordre(source, &urls, 8, |i, octets| {
        let nom = &neuves[i];
        let octets = match octets {
            Ok(o) => o,
            Err(e) => {
                // Une livraison manquee arrete la suite : la suivante ne s'applique pas par-dessus un trou.
                dire(&format!("  ! livraison {} : {} -- arret, reprise au prochain passage", nom, e.phrase));
                return false;
            }
        };
        if let Err(e) = passer(&mut dila::archive(&octets), &mut ent.articles, &mut ent.etat.changes, &conteneurs, Some(nom)) {
            panne = Some(e.reformuler(|p| format!("livraison {} : {}", nom, p)));
            return false;
        }
        ent.noter_lue(nom);
        if (i + 1) % 50 == 0 {
            if let Err(e) = ent.sauver(jour) {
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
    ent.sauver(jour)?;
    dire(&format!("== {} articles, {} livraisons appliquees ==", ent.articles.len(), ent.lues().len()));
    Ok(ent)
}

/// L'entrepot, qui doit exister.
pub fn charger(racine: &Path) -> R<EntrepotKali> {
    let dossier = racine.join(DOSSIER);
    if !dila::existe(&dossier) {
        return Err(refus!(
            "Pas d'entrepot KALI dans {} : lancer d'abord `moteur-atelier releve-conventions mettre-a-jour`.",
            racine.display()
        ));
    }
    Entrepot::charger(&dossier)
}

/// Les articles d'une convention, d'un numero donne s'il y en a un ; les versions en vigueur seulement, par defaut. Ranges
/// par texte, numero, date de debut.
pub fn articles<'a>(ent: &'a EntrepotKali, idcc: &Idcc, num: Option<&str>, en_vigueur: bool) -> Vec<&'a ArticleKali> {
    let mut out: Vec<&ArticleKali> = ent
        .articles
        .0
        .iter()
        .map(|(_, a)| a)
        .filter(|a| a.cle[0] == **idcc && num.is_none_or(|n| a.num.as_deref() == Some(n)) && (!en_vigueur || a.etat.is_some_and(EtatDuTexteDila::de_vigueur)))
        .collect();
    out.sort_by(|a, b| (&a.texte, &a.num, &a.debut).cmp(&(&b.texte, &b.num, &b.debut)));
    out
}

/// Un element de l'article tel que la transcription l'ecrit.
fn dit(t: &Option<String>) -> &str {
    match t {
        Some(t) => t,
        None => "(non dit)",
    }
}

/// L'etat de l'article tel que la transcription l'ecrit.
fn etat_dit(e: Option<EtatDuTexteDila>) -> &'static str {
    match e {
        Some(e) => e.nom(),
        None => "(non dit)",
    }
}

/// Une date de l'article telle que la transcription l'ecrit.
fn jour_dit(j: Option<Jour>) -> String {
    match j {
        Some(j) => j.iso(),
        None => "(non dit)".to_string(),
    }
}

/// Un article comme les transcriptions de `atelier/reference/sources/` le recopient.
pub fn transcrire(a: &ArticleKali) -> String {
    let num = match a.num.as_deref().filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => "(chapeau)".to_string(),
    };
    format!(
        "{}\n{} -- article {}\n{} -- {} du {} au {}\ntexte : {} ({})\n{}\n{}\n",
        "=".repeat(78),
        dila::premiers_caracteres(&a.chemin, 120),
        num,
        a.id,
        etat_dit(a.etat),
        jour_dit(a.debut),
        jour_dit(a.fin),
        dit(&a.titre_texte),
        dit(&a.texte),
        "-".repeat(78),
        dit(&a.contenu)
    )
}

/// `article <sources> <IDCC> <numero>...` : ce que la commande affiche.
pub fn rendre_articles(ent: &EntrepotKali, idcc: &Idcc, nums: &[String]) -> String {
    let mut out = String::new();
    for num in nums {
        let trouves = articles(ent, idcc, Some(num), true);
        if trouves.is_empty() {
            out.push_str(&format!("(aucun article {} en vigueur pour l'IDCC {})\n", num, idcc));
        }
        for a in trouves {
            out.push_str(&transcrire(a));
            out.push('\n');
        }
    }
    out
}

/// `chercher <sources> <IDCC> <mot>` : une ligne par article en vigueur dont le texte porte le mot (sans egard a la casse).
pub fn rendre_recherche(ent: &EntrepotKali, idcc: &Idcc, mot: &str) -> String {
    let mot = mot.to_lowercase();
    let mut out = String::new();
    for a in articles(ent, idcc, None, true) {
        if a.contenu.as_deref().is_some_and(|c| c.to_lowercase().contains(&mot)) {
            out.push_str(&format!("{}  {:<8} {:<22} {}\n", a.id, dit(&a.num), etat_dit(a.etat), dila::premiers_caracteres(&a.chemin, 90)));
        }
    }
    out
}

/// `moteur-atelier releve-conventions ...` : rend le code de sortie.
pub fn commande(args: &[socle::ligne::Argument]) -> i32 {
    let args: &[String] = &socle::ligne::textes(args);
    if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help" || a == "--aide") {
        eprintln!("{}", USAGE);
        return 2;
    }
    let racine = Path::new(&args[1]);
    let sortie = |r: R<String>| match r {
        Ok(t) => {
            use std::io::Write;
            let mut o = std::io::stdout().lock();
            // Un tuyau ferme n'est pas une erreur : le lecteur a pris ce qu'il voulait.
            if let Err(e) = o.write_all(t.as_bytes()).and_then(|_| o.flush()) {
                if e.kind() != std::io::ErrorKind::BrokenPipe {
                    eprintln!("{}", e);
                    return 1;
                }
            }
            0
        }
        Err(m) => {
            eprintln!("{}", m.en_lignes());
            1
        }
    };
    match args[0].as_str() {
        "mettre-a-jour" => {
            let r = compagnie::installation::fixer_le_code().and_then(|()| crate::sources::manifeste(&socle::fichiers::dossier_du_code())).and_then(|m| {
                let jour = droit::contexte::aujourdhui_sans_note()?;
                mettre_a_jour(racine, args.iter().any(|a| a == "--reconstruire"), &m, &dila::Reseau, &jour).map(|_| String::new())
            });
            sortie(r)
        }
        "article" if args.len() >= 4 => sortie(Idcc::lire(&args[2]).and_then(|i| charger(racine).map(|e| rendre_articles(&e, &i, &args[3..])))),
        "chercher" if args.len() >= 4 => sortie(Idcc::lire(&args[2]).and_then(|i| charger(racine).map(|e| rendre_recherche(&e, &i, &args[3])))),
        _ => {
            eprintln!("{}", USAGE);
            2
        }
    }
}
