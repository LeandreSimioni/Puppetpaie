//! Le journal des incidents : ce que le logiciel n'a pas su faire.
//!
//! Le journal des envois (`envois::journal`) note ce qu'un fichier produit est DEVENU ; celui-ci note les
//! moments ou le logiciel s'est arrete, a refuse, ou s'est fait rejeter. Il
//! porte le diagnostic a distance : tout s'ecrit ICI, toujours, et une ligne
//! porte son etat d'envoi (`en-attente`, `depose`, `sans-objet`), c'est tout
//! ce que l'envoi ajoute. `journal/incidents.json` : une liste de lignes (`Ligne`), on
//! n'ajoute qu'a la fin.
//!
//! Quatre natures (`Nature`) ; les deux premieres se
//! regroupent par empreinte, et le meme incident revu dans les dix minutes se
//! compte dans sa ligne (`fois`, `derniere`). UNE LIGNE PORTE DE QUOI DEBOGUER :
//! le detail (la charge de la route, l'etat du mois : une trace, ecrite en texte par la frontiere au moment ou l'incident
//! se note, que le moteur ne relit jamais), la trace, le fil des trois minutes d'avant, les ecritures et la machine
//! (`diagnostic`). RIEN QUI DESIGNE UNE PERSONNE : `noter` passe tout par la frontiere de
//! `diagnostic` (une personne devient sa forme, « salarie n°2 ») puis CAVIARDE
//! ce qui a la forme d'un NIR, d'un IBAN, d'un BIC ou du jeton de session.
//!
//! Ce journal est appele depuis le gestionnaire de plantage : `noter` rend son refus (un disque qui refuse, un journal
//! illisible) a l'appelant, qui le dit sans remplacer l'erreur qu'on cherche a comprendre ; `envoyer` ne leve jamais, son
//! bilan dit ce qui n'a pas marche.
//!
//! Le format sur le disque : `socle::ecriture::ecrire_json`, indent 2.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::diagnostic::{Ecriture, Machine};
use crate::editeur::{Client, ErreurFilet};
use socle::ecriture::ecrire_json;
use socle::echec::R;
use socle::echec::Classe;
use socle::fichiers::dossier_du_code;
use socle::paris::Moment;
use regex::Regex;
use socle::verrou::Verrou;

/// La nature d'un incident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Nature {
    /// Un defaut du logiciel.
    Logiciel,
    /// Un refus d'usage.
    Refus,
    /// Une DPAE rejetee.
    Dpae,
    /// Une DSN rejetee.
    Dsn,
}

pub const LOGICIEL: Nature = Nature::Logiciel;
pub const REFUS: Nature = Nature::Refus;
pub const DPAE: Nature = Nature::Dpae;
pub const DSN: Nature = Nature::Dsn;

impl Nature {
    /// Le nom de la nature, tel qu'il s'ecrit.
    pub fn nom(self) -> &'static str {
        match self {
            Nature::Logiciel => "logiciel",
            Nature::Refus => "refus",
            Nature::Dpae => "dpae",
            Nature::Dsn => "dsn",
        }
    }

    /// Un defaut et un refus se regroupent par empreinte ; un rejet de declaration fait une ligne a lui.
    pub fn se_regroupe(self) -> bool {
        matches!(self, Nature::Logiciel | Nature::Refus)
    }
}

/// Ou en est l'envoi d'une ligne chez l'editeur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EtatEnvoi {
    EnAttente,
    Depose,
    SansObjet,
}

pub const EN_ATTENTE: EtatEnvoi = EtatEnvoi::EnAttente;
pub const DEPOSE: EtatEnvoi = EtatEnvoi::Depose;
pub const SANS_OBJET: EtatEnvoi = EtatEnvoi::SansObjet;

/// L'envoi d'une ligne : son etat, et quand elle est partie.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envoi {
    pub etat: EtatEnvoi,
    #[serde(with = "socle::paris::ecrit::seconde::option")]
    pub quand: Option<Moment>,
}

/// UNE LIGNE DU JOURNAL DES INCIDENTS, telle qu'elle s'ecrit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ligne {
    pub quoi: Nature,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub quand: Moment,
    pub version: String,
    pub empreinte: String,
    pub ou: String,
    pub genre: String,
    pub resume: String,
    /// La trace de ce que l'incident portait (la charge de la route, l'etat du mois), ecrite par la frontiere.
    pub detail: String,
    pub trace: Option<String>,
    pub caviarde: Vec<String>,
    /// Combien de fois il a ete vu, et la derniere.
    pub fois: u64,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub derniere: Moment,
    pub fil: Vec<String>,
    pub ecritures: Vec<Ecriture>,
    pub machine_detail: Machine,
    pub envoi: Envoi,
}

/// Au-dela, on tronque : une trace d'exception peut faire des pages.
const TRACE_MAX: usize = 4000;
const TEXTE_MAX: usize = 600;
const JS_PILE_MAX: usize = 2000;

/// La borne du serveur ; `MAX_PAR_PASSAGE` : au-dela, le
/// passage suivant reprend.
const LOT: usize = 20;
const MAX_PAR_PASSAGE: usize = 200;
/// Un lot, en octets de JSON : le serveur doit en admettre davantage (la limite du corps d'un rapport, 1 Mio, `infra/filet/`
/// du site).
const LOT_OCTETS: usize = 400_000;

/// L'identite du poste chez l'editeur, sous la racine de la compagnie.
pub const DIAGNOSTIC: &str = "config/diagnostic.json";
/// Le journal des incidents, sous la racine de la compagnie.
pub const INCIDENTS: &str = "journal/incidents.json";
const FICHIER_EDITEUR: &str = "EDITEUR.json";

// ------------------------------------------------------------------- les motifs

fn motifs_a_caviarder() -> &'static [(Regex, &'static str)] {
    static M: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    M.get_or_init(|| {
        vec![
            // Avec ou sans les espaces de la carte Vitale (1 84 12 76 451 089 46).
            (motif(r"\b[12] ?[0-9]{2} ?(?:0[1-9]|1[0-2]) ?[0-9]{2} ?[0-9]{3} ?[0-9]{3}(?: ?[0-9]{1,2})?\b"), "NIR"),
            (motif(r"\bFR\d{2} ?(?:\d{4} ?){5}\d{3}\b"), "IBAN"),
            (motif(r"\b[A-Z]{4}FR[A-Z0-9]{2}[0-9][A-Z0-9]{0,2}\b"), "BIC"),
            // Le jeton de session de l'interface : le secret du serveur local.
            (motif(r#"(?i)(jeton=)[^&\s\"'\\]+"#), "JETON"),
        ]
    })
}

/// Les motifs sont ecrits ici, une fois : un motif faux est un defaut du programme.
fn motif(source: &str) -> Regex {
    Regex::new(source).expect("motif du journal des incidents")
}

/// Le chemin jusqu'au dossier du logiciel . `regex` ne
/// connait pas `(?=...)` : le dossier qui suit est capture (groupe 1), et la
/// recherche suivante repart de lui, comme apres une anticipation.
fn motif_chemin() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| {
        motif(r#"(?:\b[A-Za-z]:)?[\\/][^"'\n<>|;]*?[\\/]((?:logiciel|atelier|moteur|prive|prive-dev|fictives|demo|Lib|lib|site-packages)[\\/])"#)
    })
}

fn motif_adresse() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| motif(r#"[^\s<>()\[\]"',;:]+@[^\s<>()\[\]"',;:]+\.[A-Za-z]{2,}"#))
}

pub(crate) fn motif_maison() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| motif(r#"(?i)([A-Z]:\\+Users\\+|/home/|/Users/)[^\\/"'\s]+"#))
}

/// Chaque chemin jusqu'au dossier du logiciel devient `…/` ; le dossier lui-meme reste et la recherche repart de lui.
fn sans_chemin(s: &str) -> String {
    let re = motif_chemin();
    let (mut out, mut dernier, mut pos) = (String::new(), 0, 0);
    while let Some(c) = re.captures_at(s, pos) {
        let debut = c.get(0).expect("une capture a toujours son groupe 0").start();
        let suite = c.get(1).expect("le groupe 1 du motif n'est pas facultatif").start();
        out.push_str(&s[dernier..debut]);
        out.push_str("…/");
        dernier = suite;
        pos = suite;
    }
    out.push_str(&s[dernier..]);
    out
}

// ---------------------------------------------------------------------- le chemin

/// Ou vit le journal. `PUPPETPAIE_INCIDENTS` le deplace
/// (les essais).
pub fn chemin(racine: &Path) -> PathBuf {
    match std::env::var_os("PUPPETPAIE_INCIDENTS") {
        Some(autre) if !autre.is_empty() => PathBuf::from(autre).join("incidents.json"),
        Some(_) | None => racine.join(INCIDENTS),
    }
}

/// Le texte sans ses blancs de bord.
fn sans_blancs_de_bord(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
}

/// `s[:n]` sur des points de code.
fn tete(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

// -------------------------------------------------------------------- caviardage

/// Le texte nettoye, et ce qui a ete trouve. Le
/// marqueur dit CE QUI a ete retire (`[NIR]`), pas ce que c'etait.
pub fn caviarder(texte: &str) -> (String, Vec<&'static str>) {
    let mut s = texte.to_string();
    let mut trouve = Vec::new();
    for (motif, quoi) in motifs_a_caviarder() {
        let n = motif.find_iter(&s).count();
        if n > 0 {
            s = motif.replace_all(&s, format!("[{}]", quoi).as_str()).into_owned();
        }
        for _ in 0..n {
            trouve.push(*quoi);
        }
    }
    (s, trouve)
}

/// Un texte caviarde puis coupe a `limite` points de code ; ce que le caviardage a trouve s'ajoute a `trouve`.
fn nettoyer(s: &str, trouve: &mut Vec<&'static str>, limite: usize) -> String {
    let (propre, vus) = caviarder(s);
    trouve.extend(vus);
    tete(&propre, limite)
}

/// Ce qui fait que deux incidents sont LE MEME -- la
/// nature, l'endroit et le genre, pas le message.
pub fn empreinte(nature: Nature, ou: &str, quoi: &str) -> String {
    use sha2::{Digest, Sha256};
    let graine = format!("{}|{}|{}", nature.nom(), ou, quoi);
    let h = Sha256::digest(graine.as_bytes());
    h.iter().map(|o| format!("{:02x}", o)).collect::<String>()[..12].to_string()
}

// ------------------------------------------------------------------------ ecrire

/// La version du logiciel : `VERSION`, a cote du code ; un fichier qui ne se lit pas le dit.
pub fn version() -> String {
    let p = dossier_du_code().join("..").join("VERSION");
    match std::fs::read_to_string(&p) {
        Ok(t) => sans_blancs_de_bord(&t).to_string(),
        Err(e) => format!("(VERSION ne se lit pas : {})", e),
    }
}

/// Ce que `noter` recoit. `detail` est la forme de ce que l'incident porte (la charge de la route, l'etat du mois) : il
/// s'ecrit en texte, a travers la frontiere (`diagnostic::en_forme`).
pub struct Note<'a, D: Serialize> {
    pub nature: Nature,
    pub resume: &'a str,
    /// L'endroit : « calcul.rs:614 », « depot-dsn », « fiche/nir ».
    pub ou: &'a str,
    /// Le genre : le type d'exception, le code de controle, le motif du refus.
    pub quoi: &'a str,
    pub detail: D,
    pub trace: Option<&'a str>,
    /// Sans le fil des trois minutes d'avant ni les ecritures : un rapport qui ne vient d'aucun geste a deboguer, et dont
    /// les charges de routes (des saisies, leurs montants) n'ont rien a faire chez l'editeur.
    pub sans_contexte: bool,
}

/// Les lignes du journal : aucune sans journal ; un journal qui ne se lit pas se refuse.
fn lignes(p: &Path) -> R<Vec<Ligne>> {
    if !p.is_file() {
        return Ok(Vec::new());
    }
    Ok(socle::forme::lire_fichier(p, "Le journal des incidents ne se lit pas.")?)
}

fn ecrire_lignes(p: &Path, toutes: &[Ligne]) -> R<()> {
    ecrire_json(p, toutes, 2).map_err(|e| e.reclasser(Classe::Disque))
}

/// `noter` : ajoute une ligne, et la rend ; `None` quand rien n'est a noter (les faux envois d'une fictive).
pub fn noter<D: Serialize>(racine: &Path, n: &Note<D>) -> R<Option<Ligne>> {
    if (n.nature == DPAE || n.nature == DSN) && crate::bridage::branchement(racine)?.joue() {
        // LES FAUX ENVOIS NE SONT PAS DES INCIDENTS : une fictive joue ses depots.
        return Ok(None);
    }
    let mut trouve = Vec::new();
    // LA FRONTIERE (`diagnostic`) : un message peut nommer un salarie ; il devient son numero, puis le caviardage. L'alarme
    // (`caviarde`) se leve sur ce qui est ARRIVE : ce que la frontiere a deja remplace se dit quand meme.
    let texte = |s: &str, limite: usize, trouve: &mut Vec<&'static str>| {
        nettoyer(s, trouve, limite);
        nettoyer(&crate::diagnostic::texte_en_forme(Some(racine), s, limite), trouve, limite + 40)
    };
    let ou = texte(n.ou, 200, &mut trouve);
    let genre = texte(n.quoi, 200, &mut trouve);
    let resume = texte(n.resume, TEXTE_MAX, &mut trouve);
    nettoyer(&socle::ecriture::json_en_ligne(&n.detail), &mut trouve, usize::MAX);
    let detail = nettoyer(&crate::diagnostic::en_forme(Some(racine), &n.detail), &mut trouve, usize::MAX);
    let trace = match n.trace {
        Some(t) if !t.is_empty() => Some(texte(t, TRACE_MAX, &mut trouve)),
        Some(_) | None => None,
    };
    trouve.sort_unstable();
    trouve.dedup();
    let quand = socle::temps::secondes().a_la_seconde();
    let empreinte = empreinte(n.nature, n.ou, n.quoi);
    let p = chemin(racine);
    let mut toutes = lignes(&p)?;
    // LES DOUBLONS : le meme incident (meme empreinte, meme resume), encore en attente et vu il y a moins de dix minutes,
    // ne fait pas une ligne de plus -- il se compte (`fois`, `derniere`). La ligne garde le fil et la machine de la
    // premiere fois : ce qui a mene au refus.
    let debut = toutes.len().saturating_sub(50);
    let mut doublon = None;
    for (i, l) in toutes.iter().enumerate().skip(debut).rev() {
        if l.empreinte == empreinte && l.resume == resume && l.envoi.etat == EN_ATTENTE && quand.depuis(l.derniere).is_some_and(|d| d.as_secs() < DOUBLON_SECONDES as u64) {
            doublon = Some(i);
            break;
        }
    }
    if let Some(i) = doublon {
        toutes[i].fois += 1;
        toutes[i].derniere = quand;
        let ligne = toutes[i].clone();
        ecrire_lignes(&p, &toutes)?;
        return Ok(Some(ligne));
    }
    let maintenant = socle::temps::maintenant_ms();
    let (fil, ecritures) = if n.sans_contexte {
        (Vec::new(), Vec::new())
    } else {
        let fil = crate::diagnostic::fil(maintenant);
        (fil[fil.len().saturating_sub(FIL_PAR_INCIDENT)..].to_vec(), crate::diagnostic::ecritures(maintenant, ECRITURES_PAR_INCIDENT))
    };
    let ligne = Ligne {
        quoi: n.nature,
        quand: quand.clone(),
        version: version(),
        empreinte,
        ou,
        genre,
        resume,
        detail,
        trace,
        caviarde: trouve.iter().map(|t| t.to_string()).collect(),
        fois: 1,
        derniere: quand,
        fil,
        ecritures,
        machine_detail: crate::diagnostic::machine(racine),
        envoi: Envoi { etat: EN_ATTENTE, quand: None },
    };
    toutes.push(ligne.clone());
    ecrire_lignes(&p, &toutes)?;
    Ok(Some(ligne))
}

/// Note les incidents qu'un rattachement des retours a releves (`envois::etats::AvecIncidents`), meme quand il s'est
/// arrete sur une erreur, puis rend son resultat. Un incident qui ne s'ecrit pas se refuse.
pub fn noter_ceux_des_retours<T>(racine: &Path, r: envois::etats::AvecIncidents<T>) -> R<T> {
    for i in &r.incidents {
        let canal = if i.canal == envois::etats::DPAE { DPAE } else { DSN };
        noter(racine, &Note { nature: canal, ou: &i.ou, quoi: &i.quoi, resume: &i.resume, detail: &i.detail, trace: None, sans_contexte: false })?;
    }
    r.resultat
}

/// Le meme incident revu dans ce delai se compte dans sa ligne (`noter`).
const DOUBLON_SECONDES: i64 = 600;
/// Les dernieres lignes du fil qu'un incident emporte.
const FIL_PAR_INCIDENT: usize = 100;
/// Les dernieres ecritures (charges entieres) de la page qu'un incident emporte : de quoi le rejouer.
const ECRITURES_PAR_INCIDENT: usize = 3;

/// Les classes qui disent un refus, pas un defaut (`nature_de`). Un coffre ferme ou un refus de Google y sont : une
/// situation d'usage, pas un defaut du logiciel.
const REFUS_PAR_CLASSE: [Classe; 15] = [
    Classe::Refuse, Classe::SaisieIncomplete, Classe::PreuveInsuffisante, Classe::FabriqueRefus, Classe::FicheRefusee,
    Classe::DsnDejaDeposee, Classe::RepetitionManquante, Classe::VersionExigee, Classe::CompteursVerrouilles,
    Classe::Manque, Classe::Illisible, Classe::FichierAbsent,
    Classe::Coffre, Classe::Google, Classe::ModificationNonDeclarable,
];

/// La nature d'un refus d'apres sa classe : un refus d'usage, ou un defaut du logiciel (un refus sans classe aussi).
pub fn nature_de(classe: Option<Classe>) -> Nature {
    match classe {
        Some(c) if REFUS_PAR_CLASSE.contains(&c) => REFUS,
        Some(_) | None => LOGICIEL,
    }
}

/// `depuis_exception` : note un refus (sa classe, l'endroit, sa trace).
/// `nature` absente : elle se deduit de la classe. Les copies de developpement ne
/// notent rien (« rien ne part d'ici », le moteur tient sa porte).
pub fn depuis_exception<D: Serialize>(racine: &Path, classe: Option<Classe>, message: &str, nature: Option<Nature>, ou: &str, detail: D, trace: Option<&str>) -> R<Option<Ligne>> {
    if matches!(classe, Some(Classe::CopieDeDeveloppement | Classe::EditionBridee | Classe::EnvoiBride)) {
        return Ok(None);
    }
    let nature = match nature {
        Some(n) => n,
        None => nature_de(classe),
    };
    let classe = socle::echec::classe_notee(classe);
    noter(
        racine,
        &Note {
            nature,
            ou: if ou.is_empty() { "(endroit inconnu)" } else { ou },
            quoi: classe,
            resume: &format!("{}: {}", classe, message),
            detail,
            trace,
            sans_contexte: false,
        },
    )
}

/// Ce que porte un recalcul refuse : son mois, et ce que le refus porte a part.
#[derive(Serialize)]
struct DuMois<'a> {
    mois: socle::mois::Mois,
    #[serde(skip_serializing_if = "Option::is_none")]
    a_part: Option<&'a socle::echec::APart>,
}

/// UN RECALCUL QUE LE MOTEUR REFUSE A LA LECTURE D'UN ENGAGEMENT VALIDE (`engagement::etats::du_mois`) : l'engagement garde
/// sa paie validee, l'utilisateur n'en voit rien, et l'editeur le recoit comme un incident. Un incident qui ne s'ecrit pas
/// se dit sur la sortie d'erreur.
fn recalcul_refuse(racine: &Path, mois: socle::mois::Mois, refus: &socle::echec::Echec) {
    let detail = DuMois { mois, a_part: refus.a_part.as_ref() };
    if let Err(e) = depuis_exception(racine, Some(Classe::RecalculRefuse), &refus.phrase, None, "engagement::etats::du_mois", detail, None) {
        eprintln!("Un recalcul refusé ne se note pas au journal des incidents : {}", e.phrase);
    }
}

/// Branche l'engagement sur ce qui l'entoure, a l'entree de toute commande du moteur : le temoin des recalculs refuses
/// (`recalcul_refuse`).
pub fn brancher_l_engagement() {
    engagement::etats::poser_temoin(recalcul_refuse);
}

/// « http://hote:7331/interface.js?v=3 » -> « interface.js ».
/// L'hote et le port varient ; la requete porte le jeton, elle ne se garde jamais.
fn fichier_court(source: &str) -> String {
    let s = match source.split_once('?') {
        Some((avant, _)) => avant,
        None => source,
    };
    let s = match s.split_once('#') {
        Some((avant, _)) => avant,
        None => s,
    };
    let s = s.trim_end_matches('/');
    let s = match s.rsplit_once('/') {
        Some((_, nom)) => nom,
        None => s,
    };
    tete(s, 80)
}

/// CE QUE LA PAGE A VU CASSER (`noterIncident`, js/tuyau.js) : ce qu'une erreur, un rejet ou une ressource absente de la
/// page dit. La page dit toujours le genre, le message, le canal et l'ecran ; le fichier et la ligne, quand elle les
/// sait ; une ressource absente n'a ni pile ni colonne.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RapportNavigateur {
    pub genre: String,
    pub message: String,
    pub fichier: Option<String>,
    pub ligne: Option<u64>,
    pub colonne: Option<u64>,
    pub pile: Option<String>,
    pub canal: String,
    pub ou_dans_la_page: String,
}

/// Ce que porte un incident de la page.
#[derive(Serialize)]
struct DetailDeLaPage {
    face: &'static str,
    canal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    colonne: Option<u64>,
    ou_dans_la_page: String,
}

/// `depuis_navigateur` : note ce qui a casse dans l'interface. La page n'envoie
/// que ce qu'elle a vu ; la nature, l'endroit qui regroupe et le caviardage se
/// decident ICI. La nature est TOUJOURS `logiciel`. Un endroit que la page ne sait pas (sans fichier, sans ligne)
/// s'ecrit « interface:? ».
pub fn depuis_navigateur(racine: &Path, rapport: &RapportNavigateur) -> R<Option<Ligne>> {
    let genre = tete(&rapport.genre, 80);
    let fichier = match rapport.fichier.as_deref().map(fichier_court) {
        Some(f) if !f.is_empty() => f,
        _ => "interface".to_string(),
    };
    let ou = format!(
        "{}:{}",
        fichier,
        match rapport.ligne {
            Some(l) => l.to_string(),
            None => "?".to_string(),
        }
    );
    let detail = DetailDeLaPage { face: "interface", canal: tete(&rapport.canal, 20), colonne: rapport.colonne, ou_dans_la_page: tete(&rapport.ou_dans_la_page, 120) };
    let resume = format!("{}: {}", genre, rapport.message);
    let trace = rapport.pile.as_deref().filter(|p| !p.is_empty()).map(|p| tete(p, JS_PILE_MAX));
    noter(racine, &Note { nature: LOGICIEL, ou: &ou, quoi: &genre, resume: &resume, detail, trace: trace.as_deref(), sans_contexte: false })
}

// ------------------------------------------------------------------------- lire

/// `lire` : les lignes, les plus recentes en dernier ; d'une seule nature, ou celles qui attendent seulement.
pub fn lire(racine: &Path, nature: Option<Nature>, en_attente_seulement: bool) -> R<Vec<Ligne>> {
    Ok(lignes(&chemin(racine))?
        .into_iter()
        .filter(|l| nature.is_none_or(|n| l.quoi == n))
        .filter(|l| !en_attente_seulement || l.envoi.etat == EN_ATTENTE)
        .collect())
}

/// Un paquet d'incidents semblables (`regrouper`).
#[derive(Debug, Clone, Serialize)]
pub struct Paquet {
    pub empreinte: String,
    pub quoi: Nature,
    pub ou: String,
    pub genre: String,
    pub resume: String,
    pub combien: u64,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub premiere: Moment,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub derniere: Moment,
    pub versions: Vec<String>,
    pub caviarde: Vec<String>,
}

/// `regrouper` : les incidents regroupes par empreinte, le plus frequent en
/// tete. Les natures qui ne se regroupent pas rendent une entree par ligne.
pub fn regrouper(racine: &Path, nature: Option<Nature>) -> R<Vec<Paquet>> {
    // (cle de regroupement, paquet) dans l'ordre d'insertion.
    let mut paquets: Vec<(Option<String>, Paquet)> = Vec::new();
    for l in lire(racine, nature, false)? {
        let cle = l.quoi.se_regroupe().then(|| l.empreinte.clone());
        let existant = match &cle {
            Some(c) => paquets.iter().position(|(k, _)| k.as_ref() == Some(c)),
            None => None,
        };
        match existant {
            None => paquets.push((
                cle,
                Paquet {
                    empreinte: l.empreinte.clone(),
                    quoi: l.quoi,
                    ou: l.ou.clone(),
                    genre: l.genre.clone(),
                    resume: l.resume.clone(),
                    combien: l.fois,
                    premiere: l.quand,
                    derniere: l.quand,
                    versions: vec![l.version.clone()],
                    caviarde: l.caviarde.clone(),
                },
            )),
            Some(i) => {
                let p = &mut paquets[i].1;
                p.combien += l.fois;
                p.derniere = l.quand;
                if !p.versions.contains(&l.version) {
                    p.versions.push(l.version.clone());
                }
                for c in &l.caviarde {
                    if !p.caviarde.contains(c) {
                        p.caviarde.push(c.clone());
                    }
                }
            }
        }
    }
    // Les plus fréquents d'abord, puis les plus récents ; tri stable.
    let mut v: Vec<Paquet> = paquets.into_iter().map(|(_, p)| p).collect();
    v.sort_by(|a, b| (b.combien, &b.derniere).cmp(&(a.combien, &a.derniere)));
    Ok(v)
}

/// `oublier` : vide le journal, en entier ou d'une nature. Rend le nombre retire.
pub fn oublier(racine: &Path, nature: Option<Nature>) -> R<usize> {
    let p = chemin(racine);
    let toutes = lignes(&p)?;
    let n = toutes.len();
    // Sans nature, tout s'efface ; avec, ce qui est d'une autre nature reste.
    let gardees: Vec<Ligne> = toutes.into_iter().filter(|l| nature.is_some_and(|q| l.quoi != q)).collect();
    ecrire_lignes(&p, &gardees)?;
    Ok(n - gardees.len())
}

// -------------------------------------------------------- ce qui part chez l'editeur

/// Ni le chemin du dossier de l'utilisateur (qui porte son nom de session), ni une adresse courriel ; puis la frontiere
/// encore, au depart (`diagnostic::texte_repasse`) : une ligne notee avant qu'un salarie entre dans les fiches ne part
/// pas avec son nom.
fn pour_texte(racine: &Path, s: &str) -> String {
    let s = sans_chemin(s);
    let s = motif_maison().replace_all(&s, "${1}[session]");
    let s = motif_adresse().replace_all(&s, "[adresse]");
    crate::diagnostic::texte_repasse(Some(racine), &s)
}

/// UN INCIDENT TEL QU'IL PART CHEZ L'EDITEUR (`POST /rapport`, `_ligne_de_rapport` du serveur) : la ligne moins son etat
/// d'envoi, plus la machine qui l'envoie en une ligne ; chaque texte repasse (`pour_texte`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PourLEditeur {
    pub quoi: Nature,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub quand: Moment,
    pub version: String,
    pub empreinte: String,
    pub ou: String,
    pub genre: String,
    pub resume: String,
    pub detail: String,
    pub trace: Option<String>,
    pub caviarde: Vec<String>,
    pub machine: String,
    pub fois: u64,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub derniere: Moment,
    pub fil: Vec<String>,
    pub ecritures: Vec<Ecriture>,
    pub machine_detail: Machine,
}

/// `pour_l_editeur` : ce qui part d'une ligne.
pub fn pour_l_editeur(racine: &Path, l: &Ligne) -> PourLEditeur {
    let t = |s: &str| pour_texte(racine, s);
    let m = &l.machine_detail;
    let page = m.page.clone().map(|p| crate::diagnostic::PageDite {
        ecran: t(&p.ecran),
        fenetre: t(&p.fenetre),
        langue: t(&p.langue),
        langues: t(&p.langues),
        fuseau: t(&p.fuseau),
        agent: t(&p.agent),
        ..p
    });
    PourLEditeur {
        quoi: l.quoi,
        quand: l.quand,
        version: t(&l.version),
        empreinte: l.empreinte.clone(),
        ou: t(&l.ou),
        genre: t(&l.genre),
        resume: t(&l.resume),
        detail: t(&l.detail),
        trace: l.trace.as_deref().map(t),
        caviarde: l.caviarde.clone(),
        machine: machine(),
        fois: l.fois,
        derniere: l.derniere,
        fil: l.fil.iter().map(|x| t(x)).collect(),
        ecritures: l.ecritures.iter().map(|e| Ecriture { il_y_a: e.il_y_a.clone(), route: e.route.clone(), charge: t(&e.charge), issue: t(&e.issue) }).collect(),
        machine_detail: Machine {
            systeme: t(&m.systeme),
            chemin: t(&m.chemin),
            mises_a_jour: m.mises_a_jour.iter().map(|x| t(x)).collect(),
            langue_systeme: m.langue_systeme.as_deref().map(t),
            page,
            ..m.clone()
        },
    }
}

/// Un incident qui pese plus qu'un lot entier (une saisie de cent engagements dans ses ecritures) ne resterait jamais
/// sans partir : ses ecritures gardent les 2 000 premiers caracteres de leur charge, puis son fil ses quarante dernieres
/// lignes. Rend sa taille.
fn alleger(c: &mut PourLEditeur) -> usize {
    let taille = |c: &PourLEditeur| socle::ecriture::json_en_ligne(c).len();
    let mut n = taille(c);
    if n > LOT_OCTETS {
        for e in c.ecritures.iter_mut() {
            e.charge = crate::diagnostic::compact(&e.charge, 2_000);
        }
        n = taille(c);
    }
    if n > LOT_OCTETS {
        let debut = c.fil.len().saturating_sub(40);
        c.fil.drain(..debut);
        n = taille(c);
    }
    n
}

/// La machine, en une ligne -- « Linux Ubuntu 24.04 ·
/// x86_64 · moteur Rust · complete » : le systeme, l'architecture, le moteur et
/// l'edition. Rien qui designe la personne. Pas de virgule : le serveur les
/// regroupe par elle.
pub fn machine() -> String {
    static M: OnceLock<String> = OnceLock::new();
    M.get_or_init(|| {
        let morceaux = [systeme(), architecture(), "moteur Rust".to_string(), edition()];
        let m: Vec<String> = morceaux.iter().filter(|m| !m.is_empty()).map(|m| m.replace(',', " ")).collect();
        tete(&m.join(" · "), 120)
    })
    .clone()
}

fn architecture() -> String {
    let a = std::env::consts::ARCH;
    let nom = match (std::env::consts::OS, a) {
        ("windows", "x86_64") => "AMD64",
        ("windows", "aarch64") => "ARM64",
        ("macos", "aarch64") => "arm64",
        (_, a) => a,
    };
    nom.to_string()
}

#[cfg(unix)]
fn systeme() -> String {
    match std::env::consts::OS {
        "macos" => match std::process::Command::new("sw_vers").arg("-productVersion").output() {
            Ok(o) => format!("macOS {}", String::from_utf8_lossy(&o.stdout).trim()),
            Err(e) => format!("macOS (version illisible : {})", e),
        },
        _ => {
            let nomme = match std::fs::read_to_string("/etc/os-release") {
                Ok(t) => t.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim().trim_matches('"').to_string())),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => Some(format!("Linux (os-release illisible : {})", e)),
            };
            if let Some(nom) = nomme {
                return nom;
            }
            match std::fs::read_to_string("/proc/sys/kernel/osrelease") {
                Ok(t) => format!("Linux {}", t.trim()),
                Err(e) => format!("Linux (version illisible : {})", e),
            }
        }
    }
}

#[cfg(not(unix))]
fn systeme() -> String {
    use std::os::windows::process::CommandExt;
    // Sans console (CREATE_NO_WINDOW) : la route tourne sans console (`api::route`), et `cmd` en ouvrirait une, le temps
    // de dire sa version -- la boite noire qui clignote a l'envoi d'un message.
    match std::process::Command::new("cmd").args(["/C", "ver"]).creation_flags(0x0800_0000).stdin(std::process::Stdio::null()).output() {
        Ok(o) => windows_de_ver(&String::from_utf8_lossy(&o.stdout)),
        Err(e) => format!("Windows (version illisible : {})", e),
    }
}

/// Ce que `cmd /C ver` dit du systeme, en « Windows 11 (10.0.22631.4317) ». Le numero est entre crochets, et le mot qui
/// le precede depend de la langue : « [Version 10.0.22631.4317] » en anglais, « [version 10.0.19045.5011] » en
/// francais. On prend
/// donc le nombre pointe entre les crochets, sans lire le mot. Windows 11 garde le « 10.0 » de Windows 10 : c'est le build
/// (le troisieme nombre) qui les distingue, 22000 et plus pour Windows 11. Sans numero lisible, « Windows » seul : le
/// moteur ne devine pas lequel (regle 1).
#[cfg(not(unix))]
fn windows_de_ver(sortie: &str) -> String {
    let Some((entre, _)) = sortie.split_once('[').and_then(|(_, r)| r.split_once(']')) else { return "Windows".to_string() };
    let numero = |m: &&str| m.contains('.') && m.chars().all(|c| c.is_ascii_digit() || c == '.');
    let Some(version) = entre.split_whitespace().find(numero) else { return "Windows".to_string() };
    match version.split('.').nth(2).map(str::parse::<u32>) {
        Some(Ok(build)) => format!("Windows {} ({})", if build >= 22000 { "11" } else { "10" }, version),
        Some(Err(e)) => format!("Windows ({}, build illisible : {})", version, e),
        None => format!("Windows ({})", version),
    }
}

/// Le bridage du poste, `bride` ou `debloque` (`bridage::etat`).
pub fn edition() -> String {
    crate::bridage::etat().to_string()
}

// -------------------------------------------------------- l'identite du poste et l'interrupteur

/// UNE ANNEE DONT LE NOMBRE DE PAIES EST PARTI (`nombre_de_paies`) : combien, depuis quel poste, quand.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnneeRetenue {
    pub paies: u64,
    pub poste: String,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub parti_le: Moment,
}

/// `config/diagnostic.json`, TEL QU'IL S'ECRIT : l'interrupteur de l'envoi (`envoi` : absent ou vrai, les incidents
/// partent), l'identite du poste chez l'editeur (`poste`, `cle`, et le `serveur` qui l'a donnee, ou que la compagnie a
/// pose), et les annees dont le nombre de paies est parti (`nombre_de_paies::CLE`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigDiagnostic {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envoi: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poste: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serveur: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paies_annuelles: Option<BTreeMap<String, AnneeRetenue>>,
}

/// `config/diagnostic.json` de `racine` : rien n'est pose sans le fichier ; un fichier qui ne se lit pas se refuse.
pub fn lire_config(racine: &Path) -> R<ConfigDiagnostic> {
    let p = racine.join(DIAGNOSTIC);
    if !p.is_file() {
        return Ok(ConfigDiagnostic::default());
    }
    Ok(socle::forme::lire_fichier(&p, "La configuration du signalement des dysfonctionnements ne se lit pas.")?)
}

/// Ecrit `config/diagnostic.json` de `racine`.
pub fn ecrire_config(racine: &Path, c: &ConfigDiagnostic) -> R<()> {
    ecrire_json(&racine.join(DIAGNOSTIC), c, 2).map_err(|e| e.reclasser(Classe::Disque))
}

/// L'ENVOI A L'EDITEUR SE COUPE (une condition de SignPath). `config/diagnostic.json`,
/// cle `envoi` : absente ou vraie, les incidents partent ; `false`, ils restent notes sur le poste et rien ne part.
pub fn envoi(racine: &Path) -> R<bool> {
    Ok(lire_config(racine)?.envoi != Some(false))
}

/// `envoi`, pour l'import, qui ne peut rien refuser (`import::incident`) : une configuration qui ne se lit pas coupe
/// l'envoi -- rien ne part tant que la compagnie ne peut pas dire qu'elle le veut --, et le fil le dit.
pub fn envoi_actif(racine: &Path) -> bool {
    match envoi(racine) {
        Ok(actif) => actif,
        Err(e) => {
            crate::diagnostic::noter_moteur(&format!("Envoi à l'éditeur coupé : {}", e.phrase));
            false
        }
    }
}

/// Active ou coupe l'envoi a l'editeur ; le numero du poste reste.
pub fn regler_envoi(racine: &Path, actif: bool) -> R<()> {
    let mut c = lire_config(racine)?;
    c.envoi = Some(actif);
    ecrire_config(racine, &c)
}

/// Un numero de poste tel que le serveur les donne : 32 hexadecimaux.
fn numero_de_poste(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Une cle de poste telle que le serveur les donne : 64 hexadecimaux.
fn cle_de_poste(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Le numero de ce poste chez l'editeur, tel que le serveur l'a donne (`config/diagnostic.json`, dans la compagnie dont
/// vient la configuration) ; aucun tant qu'aucune identite n'a ete demandee. Ne demande rien : c'est `client_du_poste`
/// qui le fait, a l'envoi.
pub fn poste(racine: &Path) -> R<Option<String>> {
    let (_, ou) = configuration(racine)?;
    Ok(lire_config(&ou)?.poste.filter(|n| numero_de_poste(n)))
}

/// `client_du_poste` : le client des actions de l'editeur chez `serveur`, avec
/// l'identite de ce poste. Elle se lit dans `config/diagnostic.json` de `ou`
/// (`poste`, `cle`, `serveur`) ; absente, incomplete, ou donnee par un autre
/// serveur, elle se DEMANDE au serveur (`POST /poste`) et s'ecrit, une fois pour
/// toutes. Une panne du serveur (reseau, 429) se rend
/// telle quelle : l'appelant laisse tout en attente.
pub fn client_du_poste(ou: &Path, serveur: &str) -> R<Result<Client, ErreurFilet>> {
    let mut c = lire_config(ou)?;
    let serveur = serveur.trim_end_matches('/');
    let connue = match (&c.poste, &c.cle, &c.serveur) {
        (Some(n), Some(k), Some(s)) if numero_de_poste(n) && cle_de_poste(k) && s.trim_end_matches('/') == serveur => Some((n.clone(), k.clone())),
        _ => None,
    };
    let (numero, cle) = match connue {
        Some(id) => id,
        None => {
            let r = match Client::demander_poste(serveur) {
                Ok(r) => r,
                Err(e) => return Ok(Err(e)),
            };
            if !numero_de_poste(&r.poste) || !cle_de_poste(&r.cle) {
                return Ok(Err(ErreurFilet::filet(format!("le serveur de l'éditeur a répondu : {}/poste n'a pas rendu une identité de poste.", serveur), 0)));
            }
            c.poste = Some(r.poste.clone());
            c.cle = Some(r.cle.clone());
            c.serveur = Some(serveur.to_string());
            ecrire_config(ou, &c)?;
            (r.poste, r.cle)
        }
    };
    Ok(Ok(Client::du_poste(serveur, &numero, &cle)))
}

/// `EDITEUR.json`, que le paquet livre a cote du code : le serveur de l'editeur.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Editeur {
    serveur: String,
}

/// Le serveur de l'editeur, pour les incidents, les
/// retours et leurs reponses, et la compagnie qui porte l'identite du poste
/// (son `config/diagnostic.json`) -- celui de la compagnie ouverte ; sinon celui
/// de la vraie compagnie du poste (depuis une FICTIVE seulement : un poste, une
/// identite) ; sinon celui que le paquet livre (`EDITEUR.json`), l'identite
/// restant dans la compagnie ouverte ; sinon celui du moteur
/// (`editeur::SERVEUR_PAR_DEFAUT`, que `EDITEUR.json` remplace quand le paquet le livre).
pub fn configuration(racine: &Path) -> R<(String, PathBuf)> {
    if let Some(s) = lire_config(racine)?.serveur.filter(|s| !s.is_empty()) {
        return Ok((s, racine.to_path_buf()));
    }
    let code = dossier_du_code();
    let logiciel = code.parent().ok_or_else(|| socle::echec::refus_avec("Le dossier du code n'a pas de parent.", socle::echec::Lieu { chemin: Some(code.to_string_lossy().into_owned()), ..Default::default() }).reclasser(Classe::DefautDuLogiciel))?;
    let voisin = match logiciel.parent() {
        Some(r) => Some(r.join("prive")),
        None => None,
    };
    let reel = match voisin {
        Some(v) if v.join(socle::fichiers::CONFIG).is_dir() => v,
        Some(_) | None => logiciel.to_path_buf(),
    };
    if crate::installation::est_fictive(racine)? && reel != racine && reel.join(socle::fichiers::CONFIG).is_dir() {
        if let Some(s) = lire_config(&reel)?.serveur.filter(|s| !s.is_empty()) {
            return Ok((s, reel));
        }
    }
    let editeur = logiciel.join(FICHIER_EDITEUR);
    if editeur.is_file() {
        let e: Editeur = socle::forme::lire_fichier(&editeur, "Le serveur de l'éditeur livré avec le logiciel ne se lit pas.")?;
        return Ok((e.serveur, racine.to_path_buf()));
    }
    Ok((crate::editeur::SERVEUR_PAR_DEFAUT.to_string(), racine.to_path_buf()))
}

/// Le canal vers l'editeur, pret a envoyer.
pub enum Canal {
    /// Le client, avec l'identite du poste.
    Pret(Client),
    /// L'identite n'a pas pu etre obtenue (reseau, 429).
    Panne(ErreurFilet),
}

/// `canal` : le client des actions de l'editeur pour cette compagnie.
pub fn canal(racine: &Path) -> R<Canal> {
    let (serveur, ou) = configuration(racine)?;
    Ok(match client_du_poste(&ou, &serveur)? {
        Ok(c) => Canal::Pret(c),
        Err(e) => Canal::Panne(e),
    })
}

/// Passe a `depose` les lignes nommees par (empreinte, quand).
fn marquer(racine: &Path, cles: &[(String, Moment)]) -> R<usize> {
    if cles.is_empty() {
        return Ok(0);
    }
    let p = chemin(racine);
    let mut toutes = lignes(&p)?;
    let quand = socle::temps::secondes().a_la_seconde();
    let mut n = 0;
    for l in toutes.iter_mut() {
        if l.envoi.etat == EN_ATTENTE && cles.iter().any(|(e, w)| *e == l.empreinte && *w == l.quand) {
            l.envoi = Envoi { etat: DEPOSE, quand: Some(quand) };
            n += 1;
        }
    }
    if n > 0 {
        ecrire_lignes(&p, &toutes)?;
    }
    Ok(n)
}

/// LE BILAN D'UN PASSAGE D'ENVOI DES INCIDENTS : combien sont partis, combien attendent, ce qui n'a pas marche, et si un
/// autre passage tenait deja le journal.
#[derive(Debug, Serialize)]
pub struct BilanIncidents {
    pub envoyes: usize,
    pub reste: usize,
    pub erreur: Option<String>,
    pub occupe: bool,
}

fn passage(racine: &Path, bilan: &mut BilanIncidents) -> R<()> {
    let attente = lire(racine, None, true)?;
    bilan.reste = attente.len();
    if attente.is_empty() {
        return Ok(());
    }
    let client = match canal(racine)? {
        Canal::Panne(e) => {
            bilan.erreur = Some(e.dite());
            return Ok(());
        }
        Canal::Pret(c) => c,
    };
    let a_faire = &attente[..attente.len().min(MAX_PAR_PASSAGE)];
    // Des lots de `LOT` incidents au plus, et de `LOT_OCTETS` : un incident porte son fil et sa machine.
    let mut lots: Vec<Vec<PourLEditeur>> = Vec::new();
    let mut taille = 0;
    for l in a_faire {
        let mut c = pour_l_editeur(racine, l);
        let n = alleger(&mut c);
        match lots.last_mut() {
            Some(lot) if lot.len() < LOT && taille + n <= LOT_OCTETS => {
                lot.push(c);
                taille += n;
            }
            _ => {
                lots.push(vec![c]);
                taille = n;
            }
        }
    }
    for contenu in lots {
        let r = match client.rapporter(&contenu) {
            Ok(r) => r,
            Err(e) => {
                bilan.erreur = Some(e.dite());
                break;
            }
        };
        let cles: Vec<(String, Moment)> = r.recus.into_iter().chain(r.ecartes).collect();
        bilan.envoyes += marquer(racine, &cles)?;
    }
    bilan.reste = lire(racine, None, true)?.len();
    Ok(())
}

/// `envoyer` : depose chez l'editeur ce qui attend. Rend un bilan ; ne leve
/// jamais. Une panne laisse les lignes `en-attente` : le passage suivant reessaie.
pub fn envoyer(racine: &Path) -> BilanIncidents {
    // Avant le passage : rien d'envoye, rien qui attende, aucun autre passage vu.
    let mut bilan = BilanIncidents { envoyes: 0, reste: 0, erreur: None, occupe: false };
    let p = chemin(racine);
    if !p.exists() {
        return bilan;
    }
    match envoi(racine) {
        Ok(true) => {}
        Ok(false) => return bilan,
        Err(e) => {
            bilan.erreur = Some(e.phrase);
            return bilan;
        }
    }
    // Un autre passage est en cours : celui-ci n'a rien a faire.
    let marque = socle::verrou::marque("incidents", std::process::id(), &socle::paris::ecrit::seconde::texte(socle::temps::secondes()));
    let _verrou = match Verrou::prendre(&p.with_file_name(".envoi.verrou"), &marque, socle::verrou::DELAI_VERROU) {
        Ok(Ok(v)) => v,
        Ok(Err(socle::verrou::Verrouille { .. })) => {
            bilan.occupe = true;
            return bilan;
        }
        Err(m) => {
            bilan.erreur = Some(m.phrase);
            return bilan;
        }
    };
    if let Err(e) = passage(racine, &mut bilan) {
        bilan.erreur = Some(e.phrase);
    }
    bilan
}
