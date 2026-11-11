//! CE QUI PART POUR DEBOGUER : le plus possible sur la machine, rien sur les salaries -- c'est le pacte avec
//! l'utilisateur. Avec chaque envoi part l'historique des actions des trois dernieres minutes.
//!
//! Un incident `{"route": "mois"}` seul ne dit ni la charge, ni ce que la page faisait, ni l'endroit du code. Ce module
//! tient, en UN SEUL ENDROIT, tout ce qu'un retour (« Un probleme, une idee ? ») et chaque incident portent de plus :
//!
//! - LA FRONTIERE (`en_forme`, `texte_en_forme`) : rien qui designe une personne ne passe. Noms, prenoms, NIR, adresses,
//!   IBAN, BIC, naissances, courriels, telephones et secrets sont REMPLACES PAR LEUR FORME (« salarie n°2 », « NIR
//!   present, cle valide », « texte de 12 car. ») ; la compagnie n'est pas nommee (« [compagnie] »). Les montants, dates,
//!   nombres, codes, natures, motifs, spectacles et lieux, qui servent a reproduire, RESTENT. Deux serrures : la cle du
//!   champ (`genre_de_cle`) et, dans tout texte, les personnes que la compagnie connait (`config/salaries.json`,
//!   `config/entreprise.json`) et les motifs (NIR, IBAN, BIC, courriel, telephone, jeton). C'est ICI que la page et le
//!   moteur sont mis en forme : la page envoie ce qu'elle a vu, le moteur decide (regle 2). La frontiere passe sur toute
//!   forme (`serde::Serialize` : une charge de route, ce qu'un refus porte a part) au moment ou elle s'ecrit
//!   (`EnForme`) : ce qui en sort est un texte, la trace de ce qui est arrive, que le moteur ne relit jamais.
//! - LE FIL (`noter_route`, `noter_gestes`, `noter_moteur`, `fil`) : les trois dernieres minutes de la fenetre, en memoire
//!   seulement (rien ne s'ecrit sur le disque au-dela du journal des incidents, qui l'emporte) -- chaque geste de la page
//!   (clic, champ modifie, boite ouverte, console), chaque route (sa charge mise en forme, sa duree, son issue), ce que la
//!   veille et le fond ont joue. Une ligne par action, l'heure relative (« -2:41 ») ; les repetitions se comptent.
//! - LA TRACE D'UN REFUS (`noter_echec`, `trace_du_refus`) : l'endroit du code ou chaque echec a ete construit
//!   (`#[track_caller]` sur `socle::echec::erreur` et sur les conversions de `socle::echec::Echec`), et les paniques ; un refus de route
//!   retrouve la chaine des echecs construits pendant la route, et celui qui porte son message.
//! - LA MACHINE (`machine`) : version et build exacts, systeme, architecture, coeurs, memoire, disque libre, installation
//!   (installee, portable, atelier ; chemin sans nom de session), edition (bride, debloque), compagnie fictive ou reelle
//!   (non nommee), duree depuis le lancement, dernieres mises a jour, et ce que la page a vu (ecran, echelle, langue,
//!   fuseau, moteur de rendu).
//! - L'ETAT D'UN MOIS (`etat_du_mois`) : ses engagements et ses depots.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use regex::Regex;
use serde::ser::{self, Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, Serializer};
use serde::Deserialize;

use socle::echec::{Echoue, R, FENETRE_MS};
use socle::temps::maintenant_ms;
/// Au plus tant d'actions gardees en memoire, et rendues (les plus recentes).
const FIL_MAX: usize = 600;
const FIL_RENDU: usize = 150;
/// Une ligne du fil, au plus.
const LIGNE_MAX: usize = 400;
/// Une charge, dans une ligne du fil.
const CHARGE_LIGNE_MAX: usize = 260;
/// Un texte libre mis en forme, au plus (au-dela : « … (N car.) »).
const TEXTE_MAX: usize = 300;
/// Une repetition (meme action, meme issue) se compte dans la ligne d'avant si elle arrive dans ce delai.
const REPETITION_MS: u64 = 60_000;
/// Une forme mise en forme ne descend pas plus bas : au-dela, « … ».
const PROFONDEUR_MAX: usize = 14;
/// Une liste mise en forme garde ses premiers elements ; le reste se compte.
const LISTE_MAX: usize = 60;

/// Le verrou d'une memoire du fil. Un fil tombe en le tenant (une panique) le laisse empoisonne : ce qu'il garde reste
/// une liste de lignes entieres, qui se lit.
fn verrou<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match m.lock() {
        Ok(g) => g,
        Err(empoisonne) => empoisonne.into_inner(),
    }
}

fn demarrage() -> &'static Instant {
    static D: OnceLock<Instant> = OnceLock::new();
    D.get_or_init(Instant::now)
}

/// Au lancement de la fenetre : l'heure du lancement, et les paniques qui se notent (leur endroit) avant d'etre dites.
pub fn installer() {
    demarrage();
    static FAIT: OnceLock<()> = OnceLock::new();
    FAIT.get_or_init(|| {
        let avant = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = match (info.payload().downcast_ref::<&str>(), info.payload().downcast_ref::<String>()) {
                (Some(s), _) => s.to_string(),
                (None, Some(s)) => s.clone(),
                (None, None) => "(une panique sans message)".to_string(),
            };
            // La bibliotheque standard dit toujours l'endroit d'une panique (`PanicHookInfo::location`) ; s'il manquait,
            // la ligne le dirait.
            let ou = if let Some(l) = info.location() { format!("{}:{}", l.file(), l.line()) } else { "(endroit inconnu)".to_string() };
            socle::echec::noter_echec_a(&ou, Some("panique"), &message);
            avant(info);
        }));
    });
}

// =================================================================================================== la frontiere

/// Ce qu'une cle de champ dit de sa valeur.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Genre {
    Secret,
    Nir,
    Iban,
    Bic,
    Naissance,
    Nom,
    Adresse,
    CodePostal,
    Courriel,
    Telephone,
    Compagnie,
    Salarie,
}

/// Les mots d'une cle, en minuscules sans accents : `nomAffichage` -> [nom, affichage] ; `date_naissance` -> [date, naissance].
fn mots_de_cle(k: &str) -> Vec<String> {
    let mut mots = Vec::new();
    let mut cur = String::new();
    let mut prec_min = false;
    for c in declarations::champs::sans_accents(k).chars() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                mots.push(std::mem::take(&mut cur));
            }
            prec_min = false;
            continue;
        }
        if c.is_uppercase() && prec_min && !cur.is_empty() {
            mots.push(std::mem::take(&mut cur));
        }
        prec_min = c.is_lowercase() || c.is_ascii_digit();
        cur.extend(c.to_lowercase());
    }
    if !cur.is_empty() {
        mots.push(cur);
    }
    mots
}

/// Le genre d'une cle, s'il dit une personne, la compagnie ou un secret. Les autres cles laissent passer leur valeur
/// (mise en forme comme tout texte : `texte_en_forme`).
fn genre_de_cle(k: &str) -> Option<Genre> {
    let mots = mots_de_cle(k);
    let joint = mots.concat();
    let a = |m: &str| mots.iter().any(|x| x == m);
    let contient = |m: &str| joint.contains(m);
    if contient("motdepasse") || contient("password") || a("mdp") || contient("jeton") || contient("token") || contient("secret") || a("authorization") {
        return Some(Genre::Secret);
    }
    if a("nir") || a("ntt") || contient("securitesociale") || contient("numerosecu") {
        return Some(Genre::Nir);
    }
    if a("iban") || a("rib") {
        return Some(Genre::Iban);
    }
    if a("bic") {
        return Some(Genre::Bic);
    }
    if mots.iter().any(|m| m.contains("naissance")) {
        return Some(Genre::Naissance);
    }
    if a("email") || a("courriel") || a("mail") || a("mel") || contient("courriel") {
        return Some(Genre::Courriel);
    }
    if a("telephone") || a("tel") || a("portable") || a("mobile") || a("fax") || a("gsm") {
        return Some(Genre::Telephone);
    }
    if a("siren") || a("siret") || a("nic") || contient("raisonsociale") || a("enseigne") || contient("urssafcompte") || a("licence") {
        return Some(Genre::Compagnie);
    }
    if (a("code") && a("postal")) || a("cp") || contient("codepostal") {
        return Some(Genre::CodePostal);
    }
    if a("adresse") || a("rue") || a("voie") || a("complement") || a("batiment") {
        return Some(Genre::Adresse);
    }
    // Un salarie se designe par sa cle (son nom) : `salarie`, `salaries`, `cle_salarie`.
    if mots.iter().any(|m| m == "salarie" || m == "salaries") && !a("nombre") && !a("code") && !a("statut") && !a("categorie") {
        return Some(Genre::Salarie);
    }
    // Un nom de personne ; pas celui d'un fichier, d'un spectacle, d'un metier, d'un organisme...
    const PAS_UNE_PERSONNE: [&str; 14] = [
        "fichier", "spectacle", "metier", "emploi", "organisme", "rubrique", "convention", "modele", "dossier", "route", "piece",
        "caisse", "champ", "onglet",
    ];
    let personne = ["nom", "noms", "prenom", "prenoms", "signataire", "destinataire", "beneficiaire", "titulaire", "conjoint", "representant"];
    if mots.iter().any(|m| personne.contains(&m.as_str())) && !mots.iter().any(|m| PAS_UNE_PERSONNE.contains(&m.as_str())) {
        return Some(Genre::Nom);
    }
    None
}

/// Les personnes et la compagnie que la compagnie connait, et ce par quoi on les remplace.
struct Personnes {
    /// Une expression par personne (tout ce qui la nomme), et son remplacement : « salarie n°2 », « compagnie ».
    une: Vec<(Regex, String)>,
    /// Toutes a la fois : un texte qui ne s'y accroche pas n'a rien a remplacer.
    toutes: Option<Regex>,
    /// Pourquoi les personnes de la compagnie ne se connaissent pas (des fiches qui ne se lisent pas) : la frontiere ne
    /// sait plus qui elle doit taire, et aucun texte ne passe.
    inconnues: Option<String>,
}

/// Une lettre, sans regarder ses accents ni sa casse.
fn classe_de(c: char) -> String {
    let variantes = match c {
        'a' => "aàâäáãå",
        'e' => "eéèêë",
        'i' => "iîïíì",
        'o' => "oôöóòõ",
        'u' => "uùûüú",
        'c' => "cç",
        'y' => "yÿý",
        'n' => "nñ",
        _ => return regex::escape(&c.to_string()),
    };
    format!("[{}]", variantes)
}

/// L'expression d'un nom : sans accents ni casse, les separateurs (espace, tiret, apostrophe) au choix.
fn motif_de(nom: &str) -> Option<String> {
    let base = declarations::champs::sans_accents(nom.trim()).to_lowercase();
    let mut s = String::new();
    let mut sep = false;
    for c in base.chars() {
        if c.is_whitespace() || c == '-' || c == '\'' || c == '\u{2019}' || c == '_' {
            sep = true;
            continue;
        }
        if sep && !s.is_empty() {
            s.push_str(r"[\s\-'’_]+");
        }
        sep = false;
        s.push_str(&classe_de(c));
    }
    // Deux caracteres au moins, et pas un nombre seul (un code salarie « 1 » ne se cherche pas dans les montants).
    let lettres = base.chars().filter(|c| c.is_alphabetic()).count();
    let chiffres = base.chars().filter(|c| c.is_ascii_digit()).count();
    if base.chars().filter(|c| c.is_alphanumeric()).count() < 2 || (lettres == 0 && chiffres < 9) {
        return None;
    }
    Some(s)
}

/// La date et la taille d'un fichier, pour savoir s'il a change ; `None` quand le systeme ne les donne pas (absent : il
/// se relira).
fn signature(p: &Path) -> Option<(SystemTime, u64)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// Les textes qu'une fiche datee a portes, champ par champ (`droit::fiche::VoirListe`) : tout ce qui peut nommer une
/// personne, a quelque date que ce soit.
struct TextesDeFiche(Vec<(&'static str, Vec<String>)>);

impl droit::fiche::VoirListe for TextesDeFiche {
    fn liste<T: Serialize>(&mut self, cle: &'static str, liste: &[droit::fiche::Datee<T>]) {
        let textes: Vec<String> = liste.iter().filter_map(|e| e.valeur.as_ref()).filter_map(texte_de).collect();
        if !textes.is_empty() {
            self.0.push((cle, textes));
        }
    }
}

impl TextesDeFiche {
    fn de<F: droit::fiche::FicheDatee>(f: &F) -> TextesDeFiche {
        let mut t = TextesDeFiche(Vec::new());
        f.voir_listes(&mut t);
        t
    }

    /// Tous les textes que le champ `cle` a portes.
    fn tous<'a>(&'a self, cle: &'a str) -> impl Iterator<Item = &'a String> + 'a {
        self.0.iter().filter(move |(c, _)| *c == cle).flat_map(|(_, l)| l.iter())
    }

    /// Le dernier texte du champ `cle`, sans ses blancs de bord, s'il en a porte un qui dit quelque chose.
    fn dernier(&self, cle: &str) -> Option<String> {
        self.tous(cle).last().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
    }
}

/// Le numero d'un salarie : son `code_salarie` (celui des bulletins), sinon son rang dans les fiches.
fn numero_de(fiche: &TextesDeFiche, rang: usize) -> String {
    match fiche.dernier("code_salarie") {
        Some(code) => code,
        None => format!("{}", rang + 1),
    }
}

/// Les champs d'une fiche qui nomment la personne : ceux-la se cherchent dans tout texte.
const CHAMPS_QUI_NOMMENT: [&str; 13] = [
    "nom", "prenom", "prenoms", "nom_affichage", "prenom_affichage", "nom_usage", "nom_naissance", "email", "courriel",
    "adresse", "adresse_affichage", "telephone", "nir",
];
const CHAMPS_DE_LA_COMPAGNIE: [&str; 11] = [
    "raison_sociale", "nom_affichage", "enseigne", "siren", "siret", "adresse", "adresse_affichage", "urssaf_compte",
    "licence_spectacle", "email", "telephone",
];

fn construire(racine: &Path) -> Personnes {
    let mut une: Vec<(Regex, String)> = Vec::new();
    let mut tous: Vec<String> = Vec::new();
    let mut inconnues: Option<String> = None;
    let mut ajouter = |noms: Vec<String>, quoi: String, inconnues: &mut Option<String>| {
        let mut motifs: Vec<String> = noms.iter().filter_map(|n| motif_de(n)).collect();
        motifs.sort_by_key(|m| std::cmp::Reverse(m.len()));
        motifs.dedup();
        if motifs.is_empty() {
            return;
        }
        match Regex::new(&format!(r"(?i)\b(?:{})\b", motifs.join("|"))) {
            Ok(r) => {
                tous.extend(motifs);
                une.push((r, quoi));
            }
            Err(e) => *inconnues = Some(format!("les noms de {} ne se cherchent pas ({})", quoi, e)),
        }
    };
    let salaries = racine.join(droit::contexte::SALARIES);
    if salaries.is_file() {
        match droit::fiche::lire_fichier::<droit::fiche::Salaries>(&salaries, droit::contexte::SALARIES) {
            Ok(fiches) => {
                for (rang, (cle, datee)) in fiches.fiches.iter().enumerate() {
                    let fiche = TextesDeFiche::de(datee);
                    let mut noms = vec![cle.to_string()];
                    for c in CHAMPS_QUI_NOMMENT {
                        noms.extend(fiche.tous(c).cloned());
                    }
                    // Le prenom et le nom ensemble, dans les deux ordres : « Fanchon Brioche » devient un seul « [salarie n°1] ».
                    for (p, n) in [("prenom", "nom"), ("prenom_affichage", "nom_affichage"), ("prenom_affichage", "nom_usage")] {
                        if let (Some(p), Some(n)) = (fiche.dernier(p), fiche.dernier(n)) {
                            noms.push(format!("{} {}", p, n));
                            noms.push(format!("{} {}", n, p));
                        }
                    }
                    ajouter(noms, format!("salarié n°{}", numero_de(&fiche, rang)), &mut inconnues);
                }
            }
            Err(e) => inconnues = Some(format!("les fiches des salariés ne se lisent pas ({})", e.phrase)),
        }
    }
    let entreprise = racine.join(droit::contexte::ENTREPRISE);
    if entreprise.is_file() {
        match droit::fiche::lire_fichier::<droit::fiche::FicheEntrepriseDatee>(&entreprise, droit::contexte::ENTREPRISE) {
            Ok(e) => {
                let fiche = TextesDeFiche::de(&e);
                let mut noms = Vec::new();
                for c in CHAMPS_DE_LA_COMPAGNIE {
                    for s in fiche.tous(c) {
                        noms.push(s.clone());
                        // « Compagnie du Fil de Fer (démonstration) » : le nom sans sa parenthese aussi.
                        if let Some((avant, _)) = s.split_once(" (") {
                            noms.push(avant.to_string());
                        }
                    }
                }
                ajouter(noms, "compagnie".to_string(), &mut inconnues);
            }
            Err(e) => inconnues = Some(format!("la fiche de l'entreprise ne se lit pas ({})", e.phrase)),
        }
    }
    tous.sort_by_key(|m| std::cmp::Reverse(m.len()));
    let toutes = if tous.is_empty() {
        None
    } else {
        match Regex::new(&format!(r"(?i)\b(?:{})\b", tous.join("|"))) {
            Ok(r) => Some(r),
            Err(e) => {
                inconnues = Some(format!("les noms de la compagnie ne se cherchent pas ({})", e));
                None
            }
        }
    };
    Personnes { une, toutes, inconnues }
}

type Signatures = (Option<(SystemTime, u64)>, Option<(SystemTime, u64)>);

/// Les personnes de la compagnie `racine`, relues quand ses fiches changent. Sans compagnie, personne n'est connu.
fn personnes(racine: Option<&Path>) -> Arc<Personnes> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, (Signatures, Arc<Personnes>)>>> = OnceLock::new();
    let Some(racine) = racine else { return Arc::new(Personnes { une: Vec::new(), toutes: None, inconnues: None }) };
    let sig = (signature(&racine.join(droit::contexte::SALARIES)), signature(&racine.join(droit::contexte::ENTREPRISE)));
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((s, p)) = verrou(cache).get(racine) {
        if *s == sig {
            return p.clone();
        }
    }
    let p = Arc::new(construire(racine));
    verrou(cache).insert(racine.to_path_buf(), (sig, p.clone()));
    p
}

fn motif_courriel() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| Regex::new(r#"[^\s<>()\[\]"',;:]+@[^\s<>()\[\]"',;:]+\.[A-Za-z]{2,}"#).expect("motif"))
}

/// Un IBAN en clair (lettres comprises, avec ou sans espaces) : le caviardage des incidents ne voit que les IBAN francais
/// tout en chiffres.
fn motif_iban() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| Regex::new(r"\b[A-Z]{2}\d{2}(?: ?[A-Z0-9]{4}){2,7}(?: ?[A-Z0-9]{1,4})?\b").expect("motif"))
}

fn motif_telephone() -> &'static Regex {
    static M: OnceLock<Regex> = OnceLock::new();
    M.get_or_init(|| Regex::new(r"(?:\+33[\s.\-]?|\b0)[1-9](?:[\s.\-]?\d{2}){4}\b").expect("motif"))
}

/// « 12 car. », au singulier comme au pluriel.
fn car(n: usize) -> String {
    format!("{} car.", n)
}

fn nb_car(s: &str) -> usize {
    s.chars().count()
}

/// Un texte libre, mis en forme : les donnees encodees (une capture, un fichier) deviennent leur taille ; NIR, IBAN, BIC,
/// jeton, courriel et telephone deviennent ce qu'ils sont ; toute personne et la compagnie connues deviennent leur numero ;
/// le reste passe, coupe a `limite` caracteres.
pub fn texte_en_forme(racine: Option<&Path>, s: &str, limite: usize) -> String {
    texte_avec(&personnes(racine), s, limite)
}

fn texte_avec(p: &Personnes, s: &str, limite: usize) -> String {
    let n = nb_car(s);
    if let Some(pourquoi) = &p.inconnues {
        return format!("(texte de {} retiré : {})", car(n), pourquoi);
    }
    let tete: String = s.chars().take(200).collect();
    if n > 200 && !tete.chars().any(char::is_whitespace) {
        return format!("(données de {})", car(n));
    }
    let (t, _) = crate::incidents::caviarder(s);
    let t = motif_courriel().replace_all(&t, "[adresse courriel]").into_owned();
    let t = motif_telephone().replace_all(&t, "[téléphone]").into_owned();
    let mut t = motif_iban().replace_all(&t, "[IBAN]").into_owned();
    if p.toutes.as_ref().is_some_and(|r| r.is_match(&t)) {
        for (r, quoi) in &p.une {
            t = r.replace_all(&t, format!("[{}]", quoi).as_str()).into_owned();
        }
    }
    let n = nb_car(&t);
    if n > limite {
        let coupe: String = t.chars().take(limite).collect();
        return format!("{}… ({})", coupe, car(n));
    }
    t
}

/// La cle de controle d'un NIR (13 chiffres, Corse 2A/2B comprise) : 97 - (NIR mod 97).
fn cle_nir(n13: &str) -> socle::echec::R<u64> {
    let corse = match &n13[5..7] {
        "2A" | "2a" => Some(1_000_000u64),
        "2B" | "2b" => Some(2_000_000u64),
        _ => None,
    };
    let chiffres: String = n13.chars().map(|c| if c.is_ascii_alphabetic() { '0' } else { c }).collect();
    let v: u64 = chiffres.parse().map_err(|e: std::num::ParseIntError| socle::echec::refus(e.to_string()))?;
    let v = match corse {
        Some(m) => v.checked_sub(m).ok_or_else(|| socle::echec::refus("département corse hors des nombres"))?,
        None => v,
    };
    Ok(97 - v % 97)
}

/// La forme d'un NIR : present, sa cle valide ou non (quinze caracteres), sans cle (treize).
fn forme_nir(s: &str) -> String {
    let c: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if c.is_empty() {
        return String::new();
    }
    let lisible = |x: &str| x.len() == 13 && x.chars().enumerate().all(|(i, ch)| ch.is_ascii_digit() || ((i == 5 || i == 6) && "ABab".contains(ch)));
    if c.is_ascii() && c.len() == 15 && lisible(&c[..13]) {
        return match (cle_nir(&c[..13]), c[13..].parse::<u64>()) {
            (Ok(attendue), Ok(k)) if attendue == k => "NIR présent, clé valide".to_string(),
            (Ok(_), Ok(_)) => "NIR présent, clé invalide".to_string(),
            (Ok(_), Err(e)) => format!("NIR présent, clé illisible ({})", e),
            (Err(e), _) => format!("NIR présent, illisible ({})", e.phrase),
        };
    }
    if lisible(&c) {
        return match cle_nir(&c) {
            Ok(_) => "NIR présent (13 car.), sans clé".to_string(),
            Err(e) => format!("NIR présent, illisible ({})", e.phrase),
        };
    }
    if c.len() <= 2 && c.chars().all(|x| x.is_ascii_digit()) {
        return "clé de NIR présente".to_string();
    }
    format!("NIR de forme inattendue ({})", car(nb_car(&c)))
}

fn forme_iban(s: &str) -> String {
    let c: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    let inattendu = || format!("IBAN de forme inattendue ({})", car(nb_car(&c)));
    if c.len() < 5 || !c.chars().all(|x| x.is_ascii_alphanumeric()) {
        return inattendu();
    }
    let tourne = format!("{}{}", &c[4..], &c[..4]);
    let mut reste: u64 = 0;
    for ch in tourne.chars() {
        let v = match ch.to_digit(36) {
            Some(v) => v as u64,
            None => return inattendu(),
        };
        reste = if v >= 10 { (reste * 100 + v) % 97 } else { (reste * 10 + v) % 97 };
    }
    format!("IBAN {} de {}, clé {}", &c[..2], car(c.len()), if reste == 1 { "valide" } else { "invalide" })
}

fn forme_date(s: &str) -> String {
    let b = s.as_bytes();
    let iso = b.len() == 10 && b[4] == b'-' && b[7] == b'-';
    let fr = b.len() == 10 && b[2] == b'/' && b[5] == b'/';
    if iso || fr {
        "date présente (bien formée)".to_string()
    } else {
        format!("texte de {}", car(nb_car(s)))
    }
}

fn forme_courriel(s: &str) -> String {
    let t = s.trim();
    match t.split_once('@') {
        Some((a, d)) if !a.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.') && !t.contains(char::is_whitespace) => {
            "adresse courriel bien formée".to_string()
        }
        _ => format!("adresse courriel mal formée ({})", car(nb_car(t))),
    }
}

/// Un salarie designe par sa cle (son nom) : son numero, ou qu'il n'est pas dans les fiches -- ce qu'un refus
/// « salarie introuvable » demande de savoir.
fn forme_salarie(p: &Personnes, s: &str) -> Option<String> {
    for (r, quoi) in &p.une {
        if let Some(m) = r.find(s.trim()) {
            if m.start() == 0 && m.end() == s.trim().len() {
                return Some(quoi.clone());
            }
        }
    }
    None
}

/// Les sortes de fiches de la page (`js/fiche.js`, `FICHE_ACTIVE`).
const SORTES_DE_FICHE: [&str; 12] = [
    "salarie", "salaries", "entreprise", "spectacle", "spectacles", "organismes", "metiers", "bulletin", "coffre", "identite", "importer", "permanents",
];

/// La forme d'un texte (ou d'un nombre ecrit) sous une cle d'un genre donne. Un texte vide reste vide.
fn forme_de_genre(p: &Personnes, g: Genre, s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    match g {
        Genre::Secret => "[secret retiré]".to_string(),
        Genre::Nir => forme_nir(s),
        Genre::Iban => forme_iban(s),
        Genre::Bic => format!("BIC de {}", car(nb_car(s.trim()))),
        Genre::Naissance => forme_date(s),
        // `nom` nomme aussi la sorte d'une fiche (`enregistrer-fiche` : « salaries », « entreprise ») : ces mots passent.
        Genre::Nom if SORTES_DE_FICHE.contains(&s) => s.to_string(),
        Genre::Nom => match forme_salarie(p, s) {
            Some(connu) => connu,
            None => format!("nom de {}", car(nb_car(s))),
        },
        Genre::Adresse => format!("adresse de {}", car(nb_car(s))),
        Genre::CodePostal => format!("code postal de {}", car(nb_car(s.trim()))),
        Genre::Courriel => forme_courriel(s),
        Genre::Telephone => format!("téléphone de {} chiffres", s.chars().filter(char::is_ascii_digit).count()),
        Genre::Compagnie => format!("identifiant de la compagnie ({})", car(nb_car(s.trim()))),
        Genre::Salarie => match forme_salarie(p, s) {
            Some(connu) => connu,
            None => format!("salarié absent des fiches ({})", car(nb_car(s))),
        },
    }
}

/// LA FORME D'UNE DONNEE (une charge de route, ce qu'un refus porte a part, l'etat d'un mois) : la frontiere, en un seul
/// endroit, ecrite en une ligne. C'est une trace : le moteur ne la relit jamais.
pub fn en_forme<T: Serialize + ?Sized>(racine: Option<&Path>, v: &T) -> String {
    let p = personnes(racine);
    socle::ecriture::json_en_ligne(&EnForme { p: &p, v, genre: None, prof: 0 })
}

/// La valeur d'un champ de la page, telle que le fil la dit : sa forme sous le nom du champ (`nom` : son `name`, son
/// `id`), comme sous la cle `nom`.
pub fn champ_en_forme(racine: Option<&Path>, nom: &str, v: &ValeurDeChamp) -> String {
    let p = personnes(racine);
    match (v, genre_de_cle(nom)) {
        (ValeurDeChamp::Coche(true), _) => "coché".to_string(),
        (ValeurDeChamp::Coche(false), _) => "décoché".to_string(),
        (ValeurDeChamp::Nombre(n), None) => n.to_string(),
        (ValeurDeChamp::Nombre(n), Some(g)) => forme_de_genre(&p, g, &n.to_string()),
        (ValeurDeChamp::Texte(s), None) => compact(&socle::ecriture::json_en_ligne(&texte_avec(&p, s, TEXTE_MAX)), 120),
        (ValeurDeChamp::Texte(s), Some(g)) => compact(&socle::ecriture::json_en_ligne(&forme_de_genre(&p, g, s)), 120),
    }
}

/// Chaque texte deja mis en forme (une ligne du journal des incidents, au moment ou elle part), repasse sans etre coupe :
/// une ligne notee avant qu'un salarie entre dans les fiches ne part pas avec son nom.
pub fn texte_repasse(racine: Option<&Path>, s: &str) -> String {
    texte_avec(&personnes(racine), s, usize::MAX)
}

/// Un texte en une ligne, coupe a `limite` caracteres.
pub fn compact(t: &str, limite: usize) -> String {
    let n = nb_car(t);
    if n <= limite {
        t.to_string()
    } else {
        format!("{}… ({})", t.chars().take(limite).collect::<String>(), car(n))
    }
}

/// UNE FORME QUI S'ECRIT A TRAVERS LA FRONTIERE : elle s'ecrit comme `v`, chaque texte, chaque nombre sous une cle qui
/// dit une personne, et chaque cle mis en forme ; une liste garde ses `LISTE_MAX` premiers elements ; au-dela de
/// `PROFONDEUR_MAX`, « … ».
struct EnForme<'a, T: ?Sized> {
    p: &'a Personnes,
    v: &'a T,
    /// Le genre de la cle sous laquelle la forme est rangee : il vaut pour tout ce qu'elle porte.
    genre: Option<Genre>,
    prof: usize,
}

impl<T: Serialize + ?Sized> Serialize for EnForme<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.prof > PROFONDEUR_MAX {
            return s.serialize_str("…");
        }
        self.v.serialize(Frontiere { s, p: self.p, genre: self.genre, prof: self.prof })
    }
}

/// L'ecrivain de la frontiere : il passe a `s` ce que la forme ecrit, mis en forme.
struct Frontiere<'a, S> {
    s: S,
    p: &'a Personnes,
    genre: Option<Genre>,
    prof: usize,
}

/// Une forme composee (liste, table, structure) qui s'ecrit a travers la frontiere.
struct Composee<'a, C> {
    c: C,
    p: &'a Personnes,
    genre: Option<Genre>,
    prof: usize,
    /// Les elements ecrits, et ceux qui depassent `LISTE_MAX` (une liste).
    ecrits: usize,
    reste: usize,
    /// Le genre de la cle dont la valeur suit (une table).
    cle: Option<Genre>,
}

impl<'a, C> Composee<'a, C> {
    fn de(c: C, p: &'a Personnes, genre: Option<Genre>, prof: usize) -> Composee<'a, C> {
        Composee { c, p, genre, prof, ecrits: 0, reste: 0, cle: None }
    }
}

macro_rules! nombres_en_forme {
    ($($f:ident : $t:ty),*) => {
        $(fn $f(self, v: $t) -> Result<S::Ok, S::Error> {
            match self.genre {
                Some(g) => self.s.serialize_str(&forme_de_genre(self.p, g, &v.to_string())),
                None => self.s.$f(v),
            }
        })*
    };
}

impl<'a, S: Serializer> Serializer for Frontiere<'a, S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = Composee<'a, S::SerializeSeq>;
    type SerializeTuple = Composee<'a, S::SerializeTuple>;
    type SerializeTupleStruct = Composee<'a, S::SerializeTupleStruct>;
    type SerializeTupleVariant = Composee<'a, S::SerializeTupleVariant>;
    type SerializeMap = Composee<'a, S::SerializeMap>;
    type SerializeStruct = Composee<'a, S::SerializeStruct>;
    type SerializeStructVariant = Composee<'a, S::SerializeStructVariant>;

    fn serialize_bool(self, v: bool) -> Result<S::Ok, S::Error> {
        self.s.serialize_bool(v)
    }

    nombres_en_forme!(serialize_i8: i8, serialize_i16: i16, serialize_i32: i32, serialize_i64: i64, serialize_i128: i128,
        serialize_u8: u8, serialize_u16: u16, serialize_u32: u32, serialize_u64: u64, serialize_u128: u128,
        serialize_f32: f32, serialize_f64: f64);

    fn serialize_char(self, v: char) -> Result<S::Ok, S::Error> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<S::Ok, S::Error> {
        match self.genre {
            Some(g) => self.s.serialize_str(&forme_de_genre(self.p, g, v)),
            None => self.s.serialize_str(&texte_avec(self.p, v, TEXTE_MAX)),
        }
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<S::Ok, S::Error> {
        self.s.serialize_str(&format!("(données de {} octets)", v.len()))
    }

    fn serialize_none(self) -> Result<S::Ok, S::Error> {
        self.s.serialize_none()
    }

    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<S::Ok, S::Error> {
        v.serialize(self)
    }

    fn serialize_unit(self) -> Result<S::Ok, S::Error> {
        self.s.serialize_unit()
    }

    fn serialize_unit_struct(self, nom: &'static str) -> Result<S::Ok, S::Error> {
        self.s.serialize_unit_struct(nom)
    }

    fn serialize_unit_variant(self, nom: &'static str, rang: u32, variante: &'static str) -> Result<S::Ok, S::Error> {
        self.s.serialize_unit_variant(nom, rang, variante)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _nom: &'static str, v: &T) -> Result<S::Ok, S::Error> {
        v.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(self, nom: &'static str, rang: u32, variante: &'static str, v: &T) -> Result<S::Ok, S::Error> {
        self.s.serialize_newtype_variant(nom, rang, variante, &EnForme { p: self.p, v, genre: self.genre, prof: self.prof + 1 })
    }

    fn serialize_seq(self, n: Option<usize>) -> Result<Self::SerializeSeq, S::Error> {
        Ok(Composee::de(self.s.serialize_seq(n)?, self.p, self.genre, self.prof))
    }

    fn serialize_tuple(self, n: usize) -> Result<Self::SerializeTuple, S::Error> {
        Ok(Composee::de(self.s.serialize_tuple(n)?, self.p, self.genre, self.prof))
    }

    fn serialize_tuple_struct(self, nom: &'static str, n: usize) -> Result<Self::SerializeTupleStruct, S::Error> {
        Ok(Composee::de(self.s.serialize_tuple_struct(nom, n)?, self.p, self.genre, self.prof))
    }

    fn serialize_tuple_variant(self, nom: &'static str, rang: u32, variante: &'static str, n: usize) -> Result<Self::SerializeTupleVariant, S::Error> {
        Ok(Composee::de(self.s.serialize_tuple_variant(nom, rang, variante, n)?, self.p, self.genre, self.prof))
    }

    fn serialize_map(self, n: Option<usize>) -> Result<Self::SerializeMap, S::Error> {
        Ok(Composee::de(self.s.serialize_map(n)?, self.p, self.genre, self.prof))
    }

    fn serialize_struct(self, nom: &'static str, n: usize) -> Result<Self::SerializeStruct, S::Error> {
        Ok(Composee::de(self.s.serialize_struct(nom, n)?, self.p, self.genre, self.prof))
    }

    fn serialize_struct_variant(self, nom: &'static str, rang: u32, variante: &'static str, n: usize) -> Result<Self::SerializeStructVariant, S::Error> {
        Ok(Composee::de(self.s.serialize_struct_variant(nom, rang, variante, n)?, self.p, self.genre, self.prof))
    }

    fn is_human_readable(&self) -> bool {
        self.s.is_human_readable()
    }
}

impl<C: SerializeSeq> SerializeSeq for Composee<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), C::Error> {
        if self.ecrits >= LISTE_MAX {
            self.reste += 1;
            return Ok(());
        }
        self.ecrits += 1;
        let e = EnForme { p: self.p, v, genre: self.genre, prof: self.prof + 1 };
        self.c.serialize_element(&e)
    }

    fn end(mut self) -> Result<C::Ok, C::Error> {
        if self.reste > 0 {
            self.c.serialize_element(&format!("… et {} de plus", self.reste))?;
        }
        self.c.end()
    }
}

impl<C: SerializeTuple> SerializeTuple for Composee<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), C::Error> {
        let e = EnForme { p: self.p, v, genre: self.genre, prof: self.prof + 1 };
        self.c.serialize_element(&e)
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.c.end()
    }
}

impl<C: SerializeTupleStruct> SerializeTupleStruct for Composee<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), C::Error> {
        let e = EnForme { p: self.p, v, genre: self.genre, prof: self.prof + 1 };
        self.c.serialize_field(&e)
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.c.end()
    }
}

impl<C: SerializeTupleVariant> SerializeTupleVariant for Composee<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), C::Error> {
        let e = EnForme { p: self.p, v, genre: self.genre, prof: self.prof + 1 };
        self.c.serialize_field(&e)
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.c.end()
    }
}

impl<C: SerializeMap> SerializeMap for Composee<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    /// Une cle peut etre un nom (des fiches rangees par salarie) : elle se met en forme comme un texte, et dit le genre
    /// de sa valeur.
    fn serialize_key<T: Serialize + ?Sized>(&mut self, k: &T) -> Result<(), C::Error> {
        let Some(cle) = texte_de(k) else { return Err(ser::Error::custom("une clé de table qui n'est pas un texte")) };
        self.cle = match self.genre {
            Some(g) => Some(g),
            None => genre_de_cle(&cle),
        };
        let forme = texte_avec(self.p, &cle, 80);
        self.c.serialize_key(&forme)
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), C::Error> {
        let e = EnForme { p: self.p, v, genre: self.cle, prof: self.prof + 1 };
        self.c.serialize_value(&e)
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.c.end()
    }
}

impl<C: SerializeStruct> SerializeStruct for Composee<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, cle: &'static str, v: &T) -> Result<(), C::Error> {
        let genre = match self.genre {
            Some(g) => Some(g),
            None => genre_de_cle(cle),
        };
        let e = EnForme { p: self.p, v, genre, prof: self.prof + 1 };
        self.c.serialize_field(cle, &e)
    }

    fn skip_field(&mut self, cle: &'static str) -> Result<(), C::Error> {
        self.c.skip_field(cle)
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.c.end()
    }
}

impl<C: SerializeStructVariant> SerializeStructVariant for Composee<'_, C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, cle: &'static str, v: &T) -> Result<(), C::Error> {
        let genre = match self.genre {
            Some(g) => Some(g),
            None => genre_de_cle(cle),
        };
        let e = EnForme { p: self.p, v, genre, prof: self.prof + 1 };
        self.c.serialize_field(cle, &e)
    }

    fn skip_field(&mut self, cle: &'static str) -> Result<(), C::Error> {
        self.c.skip_field(cle)
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.c.end()
    }
}

/// Le texte d'une valeur simple (un texte, un nombre, un booleen, une variante sans donnee), ou `None` pour une forme
/// composee ou nulle : la cle d'une table, la valeur d'un champ de fiche.
fn texte_de<T: Serialize + ?Sized>(v: &T) -> Option<String> {
    match v.serialize(EnTexte) {
        Ok(t) => t,
        Err(PasUnTexte) => None,
    }
}

/// Ce que `EnTexte` refuse : une forme composee.
#[derive(Debug)]
struct PasUnTexte;

impl std::fmt::Display for PasUnTexte {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("pas une valeur simple")
    }
}

impl std::error::Error for PasUnTexte {}

impl ser::Error for PasUnTexte {
    fn custom<T: std::fmt::Display>(_pourquoi: T) -> PasUnTexte {
        PasUnTexte
    }
}

/// L'ecrivain d'une valeur simple en texte (`texte_de`).
struct EnTexte;

macro_rules! simples_en_texte {
    ($($f:ident : $t:ty),*) => {
        $(fn $f(self, v: $t) -> Result<Option<String>, PasUnTexte> {
            Ok(Some(v.to_string()))
        })*
    };
}

impl Serializer for EnTexte {
    type Ok = Option<String>;
    type Error = PasUnTexte;
    type SerializeSeq = ser::Impossible<Option<String>, PasUnTexte>;
    type SerializeTuple = ser::Impossible<Option<String>, PasUnTexte>;
    type SerializeTupleStruct = ser::Impossible<Option<String>, PasUnTexte>;
    type SerializeTupleVariant = ser::Impossible<Option<String>, PasUnTexte>;
    type SerializeMap = ser::Impossible<Option<String>, PasUnTexte>;
    type SerializeStruct = ser::Impossible<Option<String>, PasUnTexte>;
    type SerializeStructVariant = ser::Impossible<Option<String>, PasUnTexte>;

    simples_en_texte!(serialize_bool: bool, serialize_i8: i8, serialize_i16: i16, serialize_i32: i32, serialize_i64: i64,
        serialize_i128: i128, serialize_u8: u8, serialize_u16: u16, serialize_u32: u32, serialize_u64: u64,
        serialize_u128: u128, serialize_f32: f32, serialize_f64: f64, serialize_char: char, serialize_str: &str);

    fn serialize_bytes(self, _v: &[u8]) -> Result<Option<String>, PasUnTexte> {
        Err(PasUnTexte)
    }
    fn serialize_none(self) -> Result<Option<String>, PasUnTexte> {
        Ok(None)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<Option<String>, PasUnTexte> {
        v.serialize(self)
    }
    fn serialize_unit(self) -> Result<Option<String>, PasUnTexte> {
        Ok(None)
    }
    fn serialize_unit_struct(self, _nom: &'static str) -> Result<Option<String>, PasUnTexte> {
        Ok(None)
    }
    fn serialize_unit_variant(self, _nom: &'static str, _rang: u32, variante: &'static str) -> Result<Option<String>, PasUnTexte> {
        Ok(Some(variante.to_string()))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _nom: &'static str, v: &T) -> Result<Option<String>, PasUnTexte> {
        v.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(self, _nom: &'static str, _rang: u32, _variante: &'static str, _v: &T) -> Result<Option<String>, PasUnTexte> {
        Err(PasUnTexte)
    }
    fn serialize_seq(self, _n: Option<usize>) -> Result<Self::SerializeSeq, PasUnTexte> {
        Err(PasUnTexte)
    }
    fn serialize_tuple(self, _n: usize) -> Result<Self::SerializeTuple, PasUnTexte> {
        Err(PasUnTexte)
    }
    fn serialize_tuple_struct(self, _nom: &'static str, _n: usize) -> Result<Self::SerializeTupleStruct, PasUnTexte> {
        Err(PasUnTexte)
    }
    fn serialize_tuple_variant(self, _nom: &'static str, _rang: u32, _variante: &'static str, _n: usize) -> Result<Self::SerializeTupleVariant, PasUnTexte> {
        Err(PasUnTexte)
    }
    fn serialize_map(self, _n: Option<usize>) -> Result<Self::SerializeMap, PasUnTexte> {
        Err(PasUnTexte)
    }
    fn serialize_struct(self, _nom: &'static str, _n: usize) -> Result<Self::SerializeStruct, PasUnTexte> {
        Err(PasUnTexte)
    }
    fn serialize_struct_variant(self, _nom: &'static str, _rang: u32, _variante: &'static str, _n: usize) -> Result<Self::SerializeStructVariant, PasUnTexte> {
        Err(PasUnTexte)
    }
}

// ========================================================================================================== le fil

struct Entree {
    premier: u64,
    dernier: u64,
    fois: u32,
    texte: String,
    /// Ce qui fait qu'une action est la meme qu'une autre (une lecture qui se repete toutes les secondes).
    cle: String,
}

fn le_fil() -> &'static Mutex<VecDeque<Entree>> {
    static F: OnceLock<Mutex<VecDeque<Entree>>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Ajoute une action au fil, a son heure (`t`, en ms) : une repetition recente se compte dans sa ligne.
fn ajouter(t: u64, texte: String, cle: String) {
    let texte = if nb_car(&texte) > LIGNE_MAX { format!("{}…", texte.chars().take(LIGNE_MAX).collect::<String>()) } else { texte };
    let mut f = verrou(le_fil());
    let limite = maintenant_ms().saturating_sub(FENETRE_MS);
    while f.front().is_some_and(|e| e.dernier < limite) || f.len() >= FIL_MAX {
        f.pop_front();
    }
    let n = f.len();
    for e in f.iter_mut().skip(n.saturating_sub(3)) {
        if e.cle == cle && t >= e.dernier && t - e.dernier < REPETITION_MS {
            e.fois += 1;
            e.dernier = t;
            return;
        }
    }
    // A sa place dans le temps : un geste de la page arrive avec la route qu'il a declenchee, mais il est d'avant elle.
    let i = f.len() - f.iter().rev().take_while(|e| e.premier > t).count();
    f.insert(i, Entree { premier: t, dernier: t, fois: 1, texte, cle });
}

/// « -2:41 » : il y a deux minutes et quarante et une secondes.
fn relatif(maintenant: u64, t: u64) -> String {
    let s = maintenant.saturating_sub(t) / 1000;
    format!("-{}:{:02}", s / 60, s % 60)
}

/// Le fil des trois dernieres minutes, une ligne par action, la plus ancienne d'abord ; vide hors de la fenetre.
pub fn fil(maintenant: u64) -> Vec<String> {
    let f = verrou(le_fil());
    let limite = maintenant.saturating_sub(FENETRE_MS);
    let gardees: Vec<&Entree> = f.iter().filter(|e| e.dernier >= limite && e.premier <= maintenant + 5_000).collect();
    let debut = gardees.len().saturating_sub(FIL_RENDU);
    gardees[debut..]
        .iter()
        .map(|e| {
            let mut l = format!("{}  {}", relatif(maintenant, e.premier), e.texte);
            if e.fois > 1 {
                l.push_str(&format!("  (×{}, la dernière à {})", e.fois, relatif(maintenant, e.dernier)));
            }
            l
        })
        .collect()
}

/// UNE ECRITURE DE LA PAGE (ou un refus), gardee entiere : de quoi la rejouer sur une fictive. L'heure relative, la route,
/// sa charge mise en forme (`en_forme`, une trace), son issue.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ecriture {
    pub il_y_a: String,
    pub route: String,
    pub charge: String,
    pub issue: String,
}

/// Une route servie par la fenetre : `source` « page » (la page l'a demandee) ou « fond » (la veille, le filet, l'etat des
/// mois...) ; son nom, si elle ecrit (`ecriture` : l'acte de sa fiche, `routes::est_une_ecriture`), sa charge mise en
/// forme, sa duree, son issue.
pub fn noter_route<C: Serialize + ?Sized>(racine: &Path, source: &str, nom: &str, ecriture: bool, charge: &C, duree: Duration, issue: Result<(), (&str, &str)>) {
    let forme = en_forme(Some(racine), charge);
    let ch = if forme == "{}" || forme == "null" { String::new() } else { format!(" {}", compact(&forme, CHARGE_LIGNE_MAX)) };
    let ms = duree.as_millis();
    let (fin, cle_issue, refusee) = match issue {
        Ok(()) => (format!("→ ok, {} ms", ms), "ok".to_string(), false),
        Err((classe, message)) => {
            let m = texte_en_forme(Some(racine), message, 200);
            (format!("→ refus {} « {} », {} ms", classe, m, ms), format!("refus {} {}", classe, m), true)
        }
    };
    let quoi = if source == "page" { "route" } else { "fond, route" };
    let t = maintenant_ms().saturating_sub(ms as u64);
    // DE QUOI REJOUER : la ligne du fil coupe la charge ; une ecriture de la page (ou un refus) la garde entiere, mise en
    // forme -- la saisie qui a mene au refus se rejoue sur une fictive (`ecritures`).
    if source == "page" && (ecriture || refusee) {
        let mut e = verrou(les_ecritures());
        while e.len() >= ECRITURES_MAX || e.front().is_some_and(|x| x.t + FENETRE_MS < t) {
            e.pop_front();
        }
        let issue_texte = match fin.rsplit_once(", ") {
            Some((a, _)) => a.to_string(),
            None => fin.clone(),
        };
        // La meme route, la meme charge, la meme issue que la derniere : le fil la compte, elle ne se garde qu'une fois.
        match e.back_mut() {
            Some(d) if d.route == nom && d.charge == forme && d.issue == issue_texte => d.t = t,
            _ => e.push_back(EcritureGardee { t, route: nom.to_string(), charge: forme.clone(), issue: issue_texte }),
        }
    }
    ajouter(t, format!("{} {}{} {}", quoi, nom, ch, fin), format!("{}|{}|{}|{}", source, nom, ch, cle_issue));
}

/// Les ecritures (et les refus) de la page gardees entieres : au plus tant, dans la fenetre.
const ECRITURES_MAX: usize = 12;

struct EcritureGardee {
    t: u64,
    route: String,
    charge: String,
    issue: String,
}

fn les_ecritures() -> &'static Mutex<VecDeque<EcritureGardee>> {
    static E: OnceLock<Mutex<VecDeque<EcritureGardee>>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Les `n` dernieres ecritures (et refus) de la page dans les trois minutes, la plus ancienne d'abord.
pub fn ecritures(maintenant: u64, n: usize) -> Vec<Ecriture> {
    let e = verrou(les_ecritures());
    let gardees: Vec<&EcritureGardee> = e.iter().filter(|x| x.t + FENETRE_MS >= maintenant).collect();
    gardees[gardees.len().saturating_sub(n)..]
        .iter()
        .map(|x| Ecriture { il_y_a: relatif(maintenant, x.t), route: x.route.clone(), charge: x.charge.clone(), issue: x.issue.clone() })
        .collect()
}

/// Une action du moteur qui n'est pas une route (la veille qui commence un tour, un releve...).
pub fn noter_moteur(texte: &str) {
    ajouter(maintenant_ms(), texte.to_string(), format!("moteur|{}", texte));
}

/// CE QUE LA PAGE DIT D'ELLE-MEME a son chargement (`ditDeLaPage`, js/tuyau.js) : l'ecran, la fenetre, l'echelle, la
/// langue, le fuseau, le moteur de rendu ; ses textes mis en forme ici.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageDite {
    pub ecran: String,
    pub fenetre: String,
    pub echelle: f64,
    pub langue: String,
    pub langues: String,
    pub fuseau: String,
    pub decalage_minutes: i64,
    pub agent: String,
    pub memoire_go: Option<f64>,
    pub coeurs: Option<u64>,
}

/// CE QUE LA PAGE A FAIT depuis sa derniere demande (`X-Gestes`, js/tuyau.js : `enteteGestes`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LotDeGestes {
    gestes: Vec<Geste>,
    page: Option<PageDite>,
    perdus: Option<u64>,
}

/// Un geste de la page (`noterGeste`) : `t` son heure (celle de la page, sur la meme machine, en ms).
#[derive(Debug, Deserialize)]
#[serde(tag = "quoi", rename_all = "lowercase", deny_unknown_fields)]
enum Geste {
    /// Un clic, sur quoi (`cibleDuClic`), et l'ecran ouvert.
    Clic { t: u64, cible: Cible, vue: String },
    /// Un champ modifie : son nom, son type, sa valeur (un mot de passe : seulement qu'il a ete tape), ce qu'une case coche.
    Champ {
        t: u64,
        nom: String,
        #[serde(rename = "type")]
        genre: String,
        secret: Option<bool>,
        valeur: Option<ValeurDeChamp>,
        de: Option<String>,
    },
    /// Une boite qui s'ouvre : son titre.
    Boite { t: u64, titre: String },
    /// Ce que la console dit d'une erreur ou d'un avertissement.
    Console { t: u64, niveau: String, message: String },
}

/// Ce sur quoi on a clique (`cibleDuClic`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cible {
    balise: String,
    id: String,
    classe: String,
    texte: String,
}

/// La valeur d'un champ de la page : une case cochee ou non, un nombre de fichiers choisis, un texte.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ValeurDeChamp {
    Coche(bool),
    Nombre(u64),
    Texte(String),
}

fn page_vue() -> &'static Mutex<Option<PageDite>> {
    static P: OnceLock<Mutex<Option<PageDite>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(None))
}

/// Ce que la page a vu et fait depuis sa derniere demande (`X-Gestes`, le texte de l'en-tete decode) : chaque geste au
/// fil, mis en forme ici. Un lot qui ne se lit pas se dit au fil.
pub fn noter_gestes(racine: &Path, texte: &str) {
    let lot: LotDeGestes = match socle::forme::lire_texte(texte, "Les gestes de la page ne se lisent pas.") {
        Ok(l) => l,
        Err(e) => return noter_moteur(&format!("Les gestes de la page ne se lisent pas : {} {:?}", e.phrase, e.a_part)),
    };
    let forme = |s: &str, limite: usize| texte_en_forme(Some(racine), s.trim(), limite);
    if let Some(p) = lot.page {
        let p = PageDite {
            ecran: forme(&p.ecran, 300),
            fenetre: forme(&p.fenetre, 300),
            langue: forme(&p.langue, 300),
            langues: forme(&p.langues, 300),
            fuseau: forme(&p.fuseau, 300),
            agent: forme(&p.agent, 300),
            ..p
        };
        *verrou(page_vue()) = Some(p);
        ajouter(maintenant_ms(), "page chargée".to_string(), format!("page|{}", maintenant_ms()));
    }
    if let Some(n) = lot.perdus.filter(|n| *n > 0) {
        ajouter(maintenant_ms(), format!("({} gestes de la page perdus : trop nombreux)", n), "perdus".into());
    }
    let maintenant = maintenant_ms();
    for g in lot.gestes.iter().take(200) {
        let (t, texte) = match g {
            Geste::Clic { t, cible, vue } => {
                let mut ou = forme(&cible.balise, 20);
                let id = forme(&cible.id, 60);
                if !id.is_empty() {
                    ou.push('#');
                    ou.push_str(&id);
                }
                let classe = forme(&cible.classe, 60);
                if !classe.is_empty() {
                    ou.push('.');
                    ou.push_str(&classe);
                }
                let vue = forme(vue, 30);
                (*t, format!("clic « {} » ({}){}", forme(&cible.texte, 80), ou, if vue.is_empty() { String::new() } else { format!(" — écran {}", vue) }))
            }
            Geste::Champ { t, nom, genre, secret, valeur, de } => {
                let nom = forme(nom, 60);
                let v = match (secret, valeur) {
                    (Some(true), _) => "[secret retiré]".to_string(),
                    (_, Some(v)) => champ_en_forme(Some(racine), &nom, v),
                    (_, None) => "(sans valeur)".to_string(),
                };
                let mut ligne = format!("champ {}", nom);
                // Une case dit ce qu'elle coche (un salarie -- qui devient son numero --, un jour, une rubrique).
                if let Some(d) = de.as_deref().map(|d| forme(d, 80)).filter(|d| !d.is_empty() && d != "on") {
                    ligne.push_str(&format!(" « {} »", d));
                }
                (*t, format!("{} ({}) = {}", ligne, forme(genre, 20), v))
            }
            Geste::Boite { t, titre } => (*t, format!("boîte « {} »", forme(titre, 100))),
            Geste::Console { t, niveau, message } => (*t, format!("console.{} : {}", forme(niveau, 10), forme(message, 240))),
        };
        // L'heure du geste ; dans le futur, maintenant. Plus vieux que la fenetre : il n'a rien a faire dans le fil.
        let t = if t > maintenant + 5_000 { maintenant } else { t };
        if t + FENETRE_MS < maintenant {
            continue;
        }
        ajouter(t, texte.clone(), format!("geste|{}", texte));
    }
}

/// Ce que la page a dit d'elle-meme a son chargement (ecran, echelle, langue, fuseau, moteur de rendu) ; rien tant
/// qu'elle ne l'a pas dit.
pub fn page() -> Option<PageDite> {
    verrou(page_vue()).clone()
}

// ===================================================================================================== les echecs

/// La trace d'un refus de route : l'endroit ou son echec a ete construit (celui qui porte son message, sur quelque fil
/// que ce soit : un etat de mois se calcule dans le fil de fond), et la chaine des echecs construits avant lui sur le
/// meme fil, depuis `debut` (le debut de la route) ou cinq secondes avant lui. `None` : aucun echec note ne s'y rattache.
pub fn trace_du_refus(racine: &Path, message: &str, debut: u64) -> Option<String> {
    let l = socle::echec::echecs_notes();
    let tete: String = message.chars().take(300).collect();
    let porte = |e: &Echoue| !e.message.is_empty() && (e.message == tete || tete.contains(&e.message) || e.message.contains(tete.as_str()));
    let ici = socle::echec::fil_courant();
    let (fil, jusqu_a, depuis, endroit) = match l.iter().rev().find(|e| porte(e)) {
        Some(e) => (e.fil.clone(), e.t, if e.fil == ici { debut.min(e.t) } else { e.t.saturating_sub(5_000) }, Some(e)),
        None => (ici, u64::MAX, debut, None),
    };
    let chaine: Vec<&Echoue> = l.iter().filter(|e| e.fil == fil && e.t >= depuis && e.t <= jusqu_a).collect();
    if chaine.is_empty() && endroit.is_none() {
        return None;
    }
    let debut_chaine = chaine.len().saturating_sub(12);
    let mut t = String::new();
    match endroit {
        Some(e) => t.push_str(&format!("endroit : {} ({}), fil « {} »\n", e.ou, e.classe, e.fil)),
        None => t.push_str("endroit : aucun echec note ne porte ce message (construit sans `erreur`)\n"),
    }
    t.push_str("echecs construits avant lui sur ce fil, du plus ancien au plus recent :\n");
    for e in &chaine[debut_chaine..] {
        t.push_str(&format!("  {}  {}  {}\n", e.ou, e.classe, texte_en_forme(Some(racine), &e.message, 200)));
    }
    Some(t)
}

// ===================================================================================================== la machine

/// Ce que le systeme dit d'une grandeur de la machine, ou pourquoi il ne la dit pas.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
#[serde(untagged)]
pub enum Lu<T> {
    Dit(T),
    PasDit(String),
}

/// La memoire du poste, en Mo : (totale, disponible).
#[cfg(target_os = "linux")]
fn memoire() -> (Lu<u64>, Lu<u64>) {
    let t = match std::fs::read_to_string("/proc/meminfo") {
        Ok(t) => t,
        Err(e) => return (Lu::PasDit(e.to_string()), Lu::PasDit(e.to_string())),
    };
    let lire = |cle: &str| match t.lines().find_map(|l| l.strip_prefix(cle)).and_then(|r| r.split_whitespace().next()).map(str::parse::<u64>) {
        Some(Ok(k)) => Lu::Dit(k / 1024),
        Some(Err(e)) => Lu::PasDit(format!("{} {}", cle, e)),
        None => Lu::PasDit(format!("/proc/meminfo ne dit pas {}", cle)),
    };
    (lire("MemTotal:"), lire("MemAvailable:"))
}

#[cfg(target_os = "macos")]
fn memoire() -> (Lu<u64>, Lu<u64>) {
    let total = match std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
        Ok(o) => match String::from_utf8_lossy(&o.stdout).trim().parse::<u64>() {
            Ok(n) => Lu::Dit(n / (1024 * 1024)),
            Err(e) => Lu::PasDit(format!("sysctl hw.memsize : {}", e)),
        },
        Err(e) => Lu::PasDit(format!("sysctl : {}", e)),
    };
    (total, Lu::PasDit("macOS ne la dit pas à sysctl".to_string()))
}

#[cfg(windows)]
fn memoire() -> (Lu<u64>, Lu<u64>) {
    #[repr(C)]
    struct EtatMemoire {
        longueur: u32,
        charge: u32,
        physique_totale: u64,
        physique_libre: u64,
        pagination_totale: u64,
        pagination_libre: u64,
        virtuelle_totale: u64,
        virtuelle_libre: u64,
        etendue_libre: u64,
    }
    extern "system" {
        fn GlobalMemoryStatusEx(etat: *mut EtatMemoire) -> i32;
    }
    let mut e = EtatMemoire {
        longueur: std::mem::size_of::<EtatMemoire>() as u32,
        charge: 0,
        physique_totale: 0,
        physique_libre: 0,
        pagination_totale: 0,
        pagination_libre: 0,
        virtuelle_totale: 0,
        virtuelle_libre: 0,
        etendue_libre: 0,
    };
    // SAFETY : la structure est celle de MEMORYSTATUSEX, sa longueur posee comme l'appel l'exige.
    let ok = unsafe { GlobalMemoryStatusEx(&mut e) } != 0;
    if ok {
        (Lu::Dit(e.physique_totale / (1024 * 1024)), Lu::Dit(e.physique_libre / (1024 * 1024)))
    } else {
        let pourquoi = std::io::Error::last_os_error().to_string();
        (Lu::PasDit(pourquoi.clone()), Lu::PasDit(pourquoi))
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn memoire() -> (Lu<u64>, Lu<u64>) {
    let pourquoi = format!("{} ne la dit pas au moteur", std::env::consts::OS);
    (Lu::PasDit(pourquoi.clone()), Lu::PasDit(pourquoi))
}

/// Le disque libre la ou vit `p`, en Mo.
#[cfg(windows)]
fn disque_libre(p: &Path) -> Lu<u64> {
    use std::os::windows::ffi::OsStrExt;
    extern "system" {
        fn GetDiskFreeSpaceExW(dossier: *const u16, libre_appelant: *mut u64, total: *mut u64, libre_total: *mut u64) -> i32;
    }
    let large: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let (mut libre, mut total, mut libre_total) = (0u64, 0u64, 0u64);
    // SAFETY : un chemin termine par zero, trois sorties vivantes.
    let ok = unsafe { GetDiskFreeSpaceExW(large.as_ptr(), &mut libre, &mut total, &mut libre_total) } != 0;
    if ok {
        Lu::Dit(libre / (1024 * 1024))
    } else {
        Lu::PasDit(std::io::Error::last_os_error().to_string())
    }
}

#[cfg(not(windows))]
fn disque_libre(p: &Path) -> Lu<u64> {
    let o = match std::process::Command::new("df").arg("-Pk").arg(p).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output() {
        Ok(o) => o,
        Err(e) => return Lu::PasDit(format!("df : {}", e)),
    };
    let t = String::from_utf8_lossy(&o.stdout).into_owned();
    match t.lines().nth(1).and_then(|l| l.split_whitespace().nth(3)).map(str::parse::<u64>) {
        Some(Ok(k)) => Lu::Dit(k / 1024),
        Some(Err(e)) => Lu::PasDit(format!("df : {}", e)),
        None => Lu::PasDit("df ne dit pas le disque libre".to_string()),
    }
}

/// « 1 h 12 min 5 s ».
fn duree_lisible(d: Duration) -> String {
    let s = d.as_secs();
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{} h {} min {} s", h, m, s)
    } else if m > 0 {
        format!("{} min {} s", m, s)
    } else {
        format!("{} s", s)
    }
}

/// Un chemin du poste, sans le nom de la session : le dossier qui suit `Users` ou `home` devient « [session] ».
fn chemin_sans_session(p: &Path) -> String {
    let s = p.to_string_lossy().into_owned();
    crate::incidents::motif_maison().replace_all(&s, "${1}[session]").into_owned()
}

/// La memoire du poste, en Mo.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memoire {
    pub totale: Lu<u64>,
    pub disponible: Lu<u64>,
}

/// LA MACHINE, EN ENTIER : tout ce qui aide a reproduire, rien qui designe la personne. Ce que le systeme ne dit pas
/// reste vide.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Machine {
    pub version: String,
    pub moteur: String,
    pub profil: String,
    pub systeme: String,
    pub os: String,
    pub architecture: String,
    pub coeurs: Lu<usize>,
    pub memoire_mo: Memoire,
    pub disque_libre_mo: Lu<u64>,
    pub installation: String,
    pub chemin: String,
    pub edition: String,
    pub compagnie: String,
    pub langue_systeme: Option<String>,
    /// Depuis combien de temps le moteur tourne, en mots.
    #[serde(rename = "lance_depuis")]
    pub duree_lance: String,
    pub mises_a_jour: Vec<String>,
    pub page: Option<PageDite>,
}

/// La machine, en entier.
pub fn machine(racine: &Path) -> Machine {
    let base = crate::installation::base();
    let (totale, libre) = memoire();
    let installation = if base.join(crate::coffre::MARQUEUR_PORTABLE).is_file() {
        "portable"
    } else if base.join("atelier").is_dir() {
        "atelier (dépôt de développement)"
    } else {
        "installée"
    };
    // Les dernieres lignes du journal des mises a jour ; sans journal, aucune n'a eu lieu.
    let journal = crate::maj::dossier_maj(&base).join("journal.log");
    let mises_a_jour: Vec<String> = match std::fs::read_to_string(&journal) {
        Ok(t) => {
            let l: Vec<&str> = t.lines().filter(|l| !l.trim().is_empty()).collect();
            l[l.len().saturating_sub(6)..].iter().map(|x| texte_en_forme(None, &chemin_sans_session(Path::new(x)), 200)).collect()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => vec![format!("(le journal des mises à jour ne se lit pas : {})", e)],
    };
    let langue = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|v| match std::env::var(v) {
        Ok(x) if !x.is_empty() => Some(x),
        Ok(_) | Err(std::env::VarError::NotPresent) | Err(std::env::VarError::NotUnicode(_)) => None,
    });
    Machine {
        version: crate::incidents::version(),
        moteur: env!("PAIE_SOURCES").to_string(),
        profil: if cfg!(debug_assertions) { "debug" } else { "release" }.to_string(),
        systeme: crate::incidents::machine(),
        os: std::env::consts::OS.to_string(),
        architecture: std::env::consts::ARCH.to_string(),
        coeurs: match std::thread::available_parallelism() {
            Ok(n) => Lu::Dit(n.get()),
            Err(e) => Lu::PasDit(e.to_string()),
        },
        memoire_mo: Memoire { totale, disponible: libre },
        disque_libre_mo: disque_libre(racine),
        installation: installation.to_string(),
        chemin: chemin_sans_session(&base),
        edition: crate::bridage::etat().to_string(),
        compagnie: match crate::installation::est_fictive(racine) {
            Ok(true) => "fictive".to_string(),
            Ok(false) => "réelle (non nommée)".to_string(),
            Err(e) => e.phrase,
        },
        langue_systeme: langue,
        duree_lance: duree_lisible(demarrage().elapsed()),
        mises_a_jour,
        page: page(),
    }
}

// ===================================================================================================== l'etat d'un mois

/// Un depot du mois, tel qu'un incident le dit : l'acte, quand, le mode, accepte ou non, et les verdicts des organismes.
#[derive(Debug, serde::Serialize)]
pub struct DepotDit {
    quoi: String,
    quand: socle::paris::HeureDuJournal,
    mode: String,
    accepte: bool,
    retours: Vec<String>,
}

/// L'ETAT UTILE D'UN MOIS POUR UN REFUS : ses engagements (combien, combien valides) et ses huit derniers depots.
#[derive(Debug, serde::Serialize)]
pub struct EtatDuMois {
    saisie: bool,
    engagements: usize,
    valides: usize,
    depots: Vec<DepotDit>,
}

/// L'etat de `mois`. Un engagement ou un journal qui ne se lit pas se refuse.
pub fn etat_du_mois(racine: &Path, mois: socle::mois::Mois) -> R<EtatDuMois> {
    let objets = engagement::stock::objets_du_mois(racine, mois)?;
    let journal = envois::journal::lu(racine)?;
    let du_mois: Vec<DepotDit> = journal
        .lignes
        .iter()
        .filter(|d| d.mois() == Some(mois))
        .map(|d| DepotDit {
            quoi: d.acte().nom().to_string(),
            quand: d.quand(),
            mode: d.mode().nom().to_string(),
            accepte: d.accepte(),
            retours: d
                .retours_organisme()
                .iter()
                .map(|r| match &r.verdict {
                    Some(v) => format!("{} {}", r.nature_retour, v),
                    None => format!("{} sans verdict", r.nature_retour),
                })
                .collect(),
        })
        .collect();
    let n = du_mois.len();
    Ok(EtatDuMois {
        saisie: !objets.is_empty(),
        engagements: objets.len(),
        valides: objets.iter().filter(|o| o.est_valide()).count(),
        depots: du_mois.into_iter().skip(n.saturating_sub(8)).collect(),
    })
}
