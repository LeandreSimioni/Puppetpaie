//! Le bouton « Un probleme, une idee ? » : l'apercu de ce qui partira, l'envoi chez
//! l'editeur, la boite des signalements faits depuis ce poste, la releve des
//! reponses de l'editeur et leur lecture.
//!
//! Ce qui part (`Corps`, la forme que le serveur lit, `retourner` de `infra/filet/`) : le texte (caviarde, mis en forme
//! par `diagnostic`), la version, l'ecran, la machine (en entier, `diagnostic::machine`), le fil des trois minutes
//! d'avant (`diagnostic::fil`), les dix derniers incidents (chemin du dossier et adresses retires) et cinq captures
//! PNG ou JPEG au plus. `journal/signalements.json` garde ce que ce poste a signale (`Signalement`), du plus ancien au
//! plus recent ; la page le lit du plus recent au plus ancien.
//!
//! L'ENVOI SE FAIT EN FOND : « Envoyer » enregistre le
//! message (`envoyer` : sa ligne `en-attente` dans la boite, son corps exact dans
//! `journal/signalements/<id>.json`) et rend la main ; `partir` le fait partir,
//! lance aussitot par l'interface et relance par la veille tant qu'il attend --
//! le modele des incidents (`en-attente`, `depose`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::diagnostic::{Ecriture, Machine};
use crate::editeur::Retour;
use crate::incidents::{self, Canal, EtatEnvoi, PourLEditeur, DEPOSE, EN_ATTENTE};
use socle::ecriture::ecrire_json;
use socle::echec::{erreur, Classe, R};
use socle::verrou::Verrou;

const TEXTE_MAX: usize = 5000;
/// 1,5 Mo decodes.
const CAPTURE_MAX: usize = 1536 * 1024;
const CAPTURE_TYPES: [&str; 2] = ["image/png", "image/jpeg"];
pub const CAPTURES_MAX: usize = 5;
const INCIDENTS_JOINTS: usize = 10;
/// « Un probleme, une idee ? », sous la racine de la compagnie.
pub const SIGNALEMENTS: &str = "journal/signalements.json";

fn tete(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn refus(m: &str) -> socle::echec::Echec {
    erreur(Classe::Illisible, m)
}

/// Le base64 d'une capture, lu strictement : seulement des caracteres de
/// l'alphabet, le remplissage qu'il faut et rien apres.
pub fn base64_strict(t: &str) -> Option<Vec<u8>> {
    let o = t.as_bytes();
    let utile = match o.iter().position(|&c| c == b'=') {
        Some(i) => i,
        None => o.len(),
    };
    let (donnees, reste) = o.split_at(utile);
    let valeur = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let n = donnees.len();
    // Au plus deux `=`, ignores quand les donnees finissent sur un bloc complet ; sinon ils
    // doivent completer le bloc.
    if reste.len() > 2 || !reste.iter().all(|&c| c == b'=') {
        return None;
    }
    match n % 4 {
        0 => {}
        1 => return None,
        2 if reste.len() < 2 => return None,
        3 if reste.len() != 1 => return None,
        _ => {}
    }
    let mut sortie = Vec::with_capacity(n / 4 * 3 + 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &c in donnees {
        acc = (acc << 6) | valeur(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            sortie.push((acc >> bits & 0xff) as u8);
        }
    }
    Some(sortie)
}

/// UNE CAPTURE DE LA PAGE : son type d'image, ses donnees en base64 (avec ou sans le prefixe `data:image/...;base64,`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureDeLaPage {
    #[serde(rename = "type")]
    pub genre: String,
    pub donnees: String,
}

/// Une capture qui part : son type, ses donnees en base64 sans prefixe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureQuiPart {
    #[serde(rename = "type")]
    pub genre: String,
    pub donnees: String,
}

/// Une capture lue : ce qui part, et sa taille decodee.
struct Capture {
    part: CaptureQuiPart,
    octets: usize,
}

fn capture_valide(c: &CaptureDeLaPage) -> R<Capture> {
    if !CAPTURE_TYPES.contains(&c.genre.as_str()) {
        return Err(refus("Une capture doit être une image PNG ou JPEG."));
    }
    let donnees = match c.donnees.strip_prefix("data:") {
        Some(d) => match d.split_once(',') {
            Some((_, r)) => r.to_string(),
            None => return Err(refus("Une capture est illisible.")),
        },
        None => c.donnees.clone(),
    };
    let Some(brut) = base64_strict(&donnees) else { return Err(refus("Une capture est illisible.")) };
    if brut.is_empty() {
        return Err(refus("Une capture est vide."));
    }
    if brut.len() > CAPTURE_MAX {
        return Err(refus("Une capture dépasse 1,5 Mo."));
    }
    Ok(Capture { part: CaptureQuiPart { genre: c.genre.clone(), donnees }, octets: brut.len() })
}

fn captures_valides(captures: &[CaptureDeLaPage]) -> R<Vec<Capture>> {
    if captures.len() > CAPTURES_MAX {
        return Err(refus(&format!("{} captures au plus.", CAPTURES_MAX)));
    }
    captures.iter().map(capture_valide).collect()
}

/// Ce que le message dit de son contexte (`contexte` du serveur).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contexte {
    /// Ou l'on etait, quand la page ou le LLM le dit.
    pub ecran: Option<String>,
    /// Le nom du champ que le serveur de l'editeur attend (son contrat, `infra/filet/`) : le retour vient d'une compagnie
    /// fictive.
    pub demonstration: bool,
    /// Le dossier d'une compagnie fictive ; une compagnie reelle n'est pas nommee.
    pub compagnie: Option<String>,
    pub systeme: String,
    /// LA MACHINE EN ENTIER, et ce que la page a vu (`diagnostic::machine`) : de quoi deboguer sans redemander.
    pub machine: Machine,
}

/// LE CORPS EXACT D'UN MESSAGE AU DEVELOPPEUR (`POST /retour`), le numero du poste a part (`editeur::Client`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corps<C> {
    pub texte: String,
    pub version: String,
    pub contexte: Contexte,
    /// LES TROIS MINUTES D'AVANT : chaque geste et chaque route, une ligne par action.
    pub fil: Vec<String>,
    /// Et, entieres (mises en forme), les ecritures et les refus de la page dans ces trois minutes : de quoi les rejouer.
    pub ecritures: Vec<Ecriture>,
    pub incidents: Vec<PourLEditeur>,
    pub captures: Vec<C>,
}

/// Une capture telle que l'apercu la montre : sa taille, pas ses octets.
#[derive(Debug, Clone, Serialize)]
pub struct CaptureMontree {
    #[serde(rename = "type")]
    pub genre: String,
    pub donnees: String,
}

/// Le corps exact qui part, si le caviardage a retire quelque chose, et les captures lues.
fn corps_signalement(racine: &Path, texte: &str, ecran: Option<&str>, captures: &[CaptureDeLaPage]) -> R<(Corps<CaptureQuiPart>, bool, Vec<usize>)> {
    let texte = texte.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)).to_string();
    if texte.is_empty() {
        return Err(refus("Écrivez votre message avant d'envoyer."));
    }
    if texte.chars().count() > TEXTE_MAX {
        return Err(refus(&format!("Le texte dépasse {} caractères.", TEXTE_MAX)));
    }
    let caps = captures_valides(captures)?;
    let (texte, trouve) = incidents::caviarder(&texte);
    // La frontiere (`diagnostic`) vaut aussi pour ce que la personne ecrit : un salarie nomme devient son numero.
    let texte = crate::diagnostic::texte_en_forme(Some(racine), &texte, TEXTE_MAX + 200);
    let fictive = crate::installation::est_fictive(racine)?;
    let compagnie = match (fictive, racine.file_name()) {
        (true, Some(n)) => Some(n.to_string_lossy().into_owned()),
        (true, None) | (false, _) => None,
    };
    let lues = incidents::lire(racine, None, false)?;
    let debut = lues.len().saturating_sub(INCIDENTS_JOINTS);
    let maintenant = socle::temps::maintenant_ms();
    let corps = Corps {
        texte,
        version: incidents::version(),
        contexte: Contexte {
            ecran: ecran.map(|e| tete(e, 200)),
            demonstration: fictive,
            compagnie,
            systeme: incidents::machine(),
            machine: crate::diagnostic::machine(racine),
        },
        fil: crate::diagnostic::fil(maintenant),
        ecritures: crate::diagnostic::ecritures(maintenant, 12),
        incidents: lues[debut..].iter().map(|l| incidents::pour_l_editeur(racine, l)).collect(),
        captures: caps.iter().map(|c| c.part.clone()).collect(),
    };
    Ok((corps, !trouve.is_empty(), caps.iter().map(|c| c.octets).collect()))
}

/// Ce que rend l'apercu d'un message.
#[derive(Debug, Serialize)]
pub struct Apercu {
    /// Ce qui partira, au mot pres (les octets des captures mis a part).
    pub corps: Corps<CaptureMontree>,
    pub caviarde: bool,
    pub serveur: String,
    pub captures: usize,
    pub incidents: usize,
}

/// `signalement_apercu` : ce qui partira, au mot pres (les octets des captures
/// mis a part). On peut envoyer sans l'avoir regarde.
pub fn apercu(racine: &Path, texte: &str, ecran: Option<&str>, captures: &[CaptureDeLaPage]) -> R<Apercu> {
    let (corps, caviarde, tailles) = corps_signalement(racine, texte, ecran, captures)?;
    let (serveur, _) = incidents::configuration(racine)?;
    let incidents = corps.incidents.len();
    let montrees = corps
        .captures
        .iter()
        .zip(&tailles)
        .map(|(c, n)| CaptureMontree { genre: c.genre.clone(), donnees: format!("({} Ko, non affichés)", n.div_ceil(1024)) })
        .collect();
    let corps = Corps {
        texte: corps.texte,
        version: corps.version,
        contexte: corps.contexte,
        fil: corps.fil,
        ecritures: corps.ecritures,
        incidents: corps.incidents,
        captures: montrees,
    };
    Ok(Apercu { corps, caviarde, serveur, captures: tailles.len(), incidents })
}

// ------------------------------------------------------------ la boite des messages

/// L'envoi d'un message : son etat, quand il est parti, son dernier essai, et pourquoi il n'est pas parti.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvoiDuMessage {
    pub etat: EtatEnvoi,
    pub quand: Option<String>,
    pub essai: Option<String>,
    pub erreur: Option<String>,
}

/// UN MESSAGE DE LA BOITE (`journal/signalements.json`) : ce qu'on a ecrit, son envoi, et ce que le serveur en a dit
/// (son numero, ou l'editeur le suit, sa reponse ; vides tant qu'il n'a rien dit).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signalement {
    pub id: String,
    pub quand: String,
    pub texte: String,
    pub captures: usize,
    pub incidents: usize,
    pub empreinte: String,
    /// L'heure ou il s'est enregistre, en secondes (les doublons).
    pub enregistre: i64,
    pub envoi: EnvoiDuMessage,
    pub numero: Option<String>,
    pub ticket: Option<String>,
    pub ticket_numero: Option<i64>,
    pub reponse: Option<String>,
    pub repondu_le: Option<String>,
    pub lu: Option<bool>,
}

/// Ou en est un message, pour la page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EtatDuMessage {
    /// Arrive chez l'editeur.
    Arrive,
    /// Il part, ou partira tout de suite.
    EnCours,
    /// Un essai a manque.
    PasParti,
}

/// Un message, tel que la page le lit : ce que la boite garde, ou il en est, et pourquoi il n'est pas parti.
#[derive(Debug, Clone, Serialize)]
pub struct SignalementVu {
    #[serde(flatten)]
    pub garde: Signalement,
    pub etat: EtatDuMessage,
    pub raison: Option<String>,
}

/// Au plus tant de messages par passage : chacun peut attendre le serveur 60 s, et le verrou d'envoi se tient pour
/// abandonne passe `verrou::DELAI_VERROU` (10 minutes).
const PAR_PASSAGE: usize = 8;
const DOSSIER_ATTENTE: &str = "signalements";

fn chemin_signalements(racine: &Path) -> PathBuf {
    // A cote du journal des incidents, qu'un essai deplace (`incidents::chemin`).
    incidents::chemin(racine).with_file_name(Path::new(SIGNALEMENTS).file_name().expect("un nom de fichier"))
}

/// `journal/signalements/` : le corps exact de chaque message qui attend son depart, un fichier par message.
fn dossier_attente(racine: &Path) -> PathBuf {
    chemin_signalements(racine).with_file_name(DOSSIER_ATTENTE)
}

fn chemin_corps(racine: &Path, id: &str) -> PathBuf {
    dossier_attente(racine).join(format!("{}.json", id))
}

/// Ecrit en entier ou pas du tout : un fichier a cote, puis renomme. Un lecteur ne voit jamais un fichier a moitie.
fn ecrire_entier<T: Serialize + ?Sized>(p: &Path, v: &T) -> R<()> {
    let provisoire = socle::ecriture::provisoire(p);
    ecrire_json(&provisoire, v, 2).map_err(|m| erreur(Classe::Disque, m))?;
    std::fs::rename(&provisoire, p).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&p, &e)))
}

/// `signalements` : ceux faits depuis ce poste, du plus recent au plus ancien, chacun avec ce que la page en dit.
pub fn signalements(racine: &Path) -> R<Vec<SignalementVu>> {
    let mut a: Vec<SignalementVu> = en_ordre(racine)?.into_iter().map(pour_la_page).collect();
    a.reverse();
    Ok(a)
}

fn pour_la_page(l: Signalement) -> SignalementVu {
    let (etat, raison) = match (en_attente(&l), &l.envoi.erreur) {
        (true, Some(e)) => (EtatDuMessage::PasParti, Some(e.clone())),
        (true, None) => (EtatDuMessage::EnCours, None),
        (false, _) => (EtatDuMessage::Arrive, None),
    };
    SignalementVu { garde: l, etat, raison }
}

/// La boite telle qu'elle est, du plus ancien au plus recent : vide sans fichier ; un fichier qui ne se lit pas se
/// refuse -- reecrire par-dessus perdrait tout ce qu'il garde.
fn en_ordre(racine: &Path) -> R<Vec<Signalement>> {
    let p = chemin_signalements(racine);
    if !p.is_file() {
        return Ok(Vec::new());
    }
    Ok(socle::forme::lire_fichier(&p, "La boîte des messages au développeur ne se lit pas : rien n'y est écrit, pour ne rien perdre de ce qu'elle garde.")?)
}

/// Un message qui n'est pas encore arrive chez l'editeur.
fn en_attente(l: &Signalement) -> bool {
    l.envoi.etat == EN_ATTENTE
}

/// Le verrou de la boite : chaque ecriture le tient le temps de relire et de reecrire le fichier, jamais le temps d'un
/// appel au serveur. Tenu par un autre, on attend un peu (dix secondes) ; passe ce delai, `None`.
fn verrouiller(racine: &Path) -> R<Option<Verrou>> {
    let p = chemin_signalements(racine).with_file_name(".signalements.verrou");
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&d, &e)))?;
    }
    let marque = socle::verrou::marque("signalements", std::process::id(), &socle::paris::maintenant());
    let fin = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match Verrou::prendre(&p, &marque, socle::verrou::DELAI_VERROU).map_err(|m| erreur(Classe::Disque, m))? {
            Ok(v) => return Ok(Some(v)),
            Err(socle::verrou::Verrouille { .. }) if std::time::Instant::now() >= fin => return Ok(None),
            Err(socle::verrou::Verrouille { .. }) => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
}

/// Relit la boite sous son verrou, la passe a `f`, et la reecrit si `f` dit qu'elle a change. `None` : la boite est
/// restee occupee.
fn modifier<T>(racine: &Path, f: impl FnOnce(&mut Vec<Signalement>) -> R<(T, bool)>) -> R<Option<T>> {
    let Some(_verrou) = verrouiller(racine)? else { return Ok(None) };
    let mut lignes = en_ordre(racine)?;
    let (t, change) = f(&mut lignes)?;
    if change {
        ecrire_entier(&chemin_signalements(racine), &lignes)?;
    }
    Ok(Some(t))
}

/// Un identifiant de message, propre au poste : l'heure et six octets tires au hasard. Il nomme son fichier d'attente.
fn nouvel_id() -> R<String> {
    let mut alea = [0u8; 6];
    socle::temps::aleas(&mut alea).map_err(|e| erreur(Classe::Disque, e))?;
    let heure: String = socle::paris::maintenant().chars().filter(char::is_ascii_alphanumeric).collect();
    Ok(format!("{}-{}", heure, alea.iter().map(|o| format!("{:02x}", o)).collect::<String>()))
}

/// Ce que rend l'enregistrement d'un message : son identifiant, et s'il l'etait deja.
#[derive(Debug, Serialize)]
pub struct Enregistre {
    pub id: String,
    pub deja: bool,
}

/// `signalement-envoyer` : ENREGISTRE le message sur le poste, et rend la main tout de suite (le
/// bouton ne reste pas sur « Envoi… » le temps du serveur). Rien ne part d'ici : le depart est `partir`, que l'interface
/// lance aussitot en fond et que la veille relance tant que le message attend. Le corps exact qui partira (celui de
/// l'apercu, captures comprises) est fige maintenant, dans `journal/signalements/<id>.json` ; la boite
/// (`journal/signalements.json`) en garde la ligne, `en-attente`.
///
/// UN MEME MESSAGE NE S'ENREGISTRE QU'UNE FOIS (sinon il partirait deux fois, l'un « arrive », l'autre « pas encore
/// parti, le contenu n'est plus sur cet ordinateur »). Deux gardes :
/// - le meme texte avec les memes captures, enregistre depuis moins de `DOUBLON_SECONDES` (un double clic, une requete
///   rejouee par la fenetre), n'ajoute pas de ligne : la route rend celle qui existe (`deja`) ;
/// - le corps et sa ligne s'ecrivent sous le meme verrou, que l'effacement (`effacer_les_partis`) tient aussi : un
///   passage de `partir` lance par le premier envoi ne peut pas effacer le corps du second avant que sa ligne l'annonce.
pub fn envoyer(racine: &Path, texte: &str, ecran: Option<&str>, captures: &[CaptureDeLaPage]) -> R<Enregistre> {
    let (corps, _trouve, _tailles) = corps_signalement(racine, texte, ecran, captures)?;
    let empreinte = empreinte_du_message(&corps);
    let maintenant = socle::paris::maintenant_naif();
    let id = nouvel_id()?;
    let fichier = chemin_corps(racine, &id);
    let ligne = Signalement {
        id: id.clone(),
        quand: tete(&socle::paris::maintenant(), 16),
        texte: corps.texte.clone(),
        captures: corps.captures.len(),
        incidents: corps.incidents.len(),
        empreinte: empreinte.clone(),
        enregistre: maintenant,
        envoi: EnvoiDuMessage { etat: EN_ATTENTE, quand: None, essai: None, erreur: None },
        numero: None,
        ticket: None,
        ticket_numero: None,
        reponse: None,
        repondu_le: None,
        lu: None,
    };
    let ecrit = modifier(racine, |lignes| {
        if let Some(deja) = lignes.iter().rev().find(|l| l.empreinte == empreinte && (en_attente(l) || maintenant - l.enregistre < DOUBLON_SECONDES)) {
            return Ok((Some(deja.id.clone()), false));
        }
        if let Some(d) = fichier.parent() {
            std::fs::create_dir_all(d).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&d, &e)))?;
        }
        ecrire_entier(&fichier, &corps)?;
        lignes.push(ligne);
        Ok((None, true))
    });
    match ecrit {
        Ok(Some(Some(existant))) => Ok(Enregistre { id: existant, deja: true }),
        Ok(Some(None)) => Ok(Enregistre { id, deja: false }),
        Ok(None) => Err(erreur(Classe::Refuse, "La boîte des messages est occupée : réessayez dans un instant. Rien n'est enregistré.")),
        Err(e) => match std::fs::remove_file(&fichier) {
            Ok(()) => Err(e),
            Err(x) if x.kind() == std::io::ErrorKind::NotFound => Err(e),
            Err(x) => Err(socle::echec::Echec { classe: e.classe, phrase: format!("{} {}", String::from(e), String::from(socle::echec::du_disque(&fichier, &x))), a_part: None }),
        },
    }
}

/// Un message enregistre depuis moins de dix minutes, et identique, est le meme message.
const DOUBLON_SECONDES: i64 = 600;

/// L'empreinte de ce qu'on a ecrit : le texte (caviarde) et les captures, rien de ce qui change d'une seconde a l'autre
/// (incidents joints, version).
fn empreinte_du_message(corps: &Corps<CaptureQuiPart>) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(corps.texte.as_bytes());
    for c in &corps.captures {
        h.update(b"\n");
        h.update(c.donnees.as_bytes());
    }
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// LE BILAN D'UN PASSAGE des messages : partis, ce qui attend encore, la panne, et si un autre passage tenait la boite.
#[derive(Debug, Serialize)]
pub struct BilanMessages {
    pub partis: usize,
    pub reste: usize,
    pub erreur: Option<String>,
    pub occupe: bool,
}

/// `signalements-partir` : fait partir chez l'editeur ce qui attend, du plus ancien au plus recent, et ne leve jamais.
/// UN SEUL PASSAGE A LA FOIS (`journal/.signalements-envoi.verrou`) : un autre en cours, celui-ci rend `occupe` -- et
/// l'autre prend aussi ce qui s'est enregistre pendant qu'il tournait. Arrive, un message passe a `depose` avec le numero
/// que le serveur lui a donne, et son fichier d'attente s'efface (les captures ne restent pas sur le poste). Une panne
/// arrete le passage : tout ce qui attend garde sa ligne, et la raison ; la veille suivante reessaie.
pub fn partir(racine: &Path) -> BilanMessages {
    match passage(racine) {
        Ok(b) => b,
        Err(e) => {
            let reste = match en_ordre(racine) {
                Ok(l) => l.iter().filter(|l| en_attente(l)).count(),
                Err(f) => return BilanMessages { partis: 0, reste: 0, erreur: Some(format!("{} {}", String::from(e), String::from(f))), occupe: false },
            };
            BilanMessages { partis: 0, reste, erreur: Some(e.into()), occupe: false }
        }
    }
}

fn passage(racine: &Path) -> R<BilanMessages> {
    let bilan = |partis: usize, reste: usize, erreur: Option<String>, occupe: bool| BilanMessages { partis, reste, erreur, occupe };
    let p = chemin_signalements(racine).with_file_name(".signalements-envoi.verrou");
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&d, &e)))?;
    }
    let marque = socle::verrou::marque("signalements-envoi", std::process::id(), &socle::paris::maintenant());
    let _envoi = match Verrou::prendre(&p, &marque, socle::verrou::DELAI_VERROU).map_err(|m| erreur(Classe::Disque, m))? {
        Ok(v) => v,
        Err(socle::verrou::Verrouille { .. }) => return Ok(bilan(0, en_ordre(racine)?.iter().filter(|l| en_attente(l)).count(), None, true)),
    };
    effacer_les_partis(racine)?;
    // Ce qui attend part maintenant : la boite le dit « en cours d'envoi ».
    let essai = socle::paris::maintenant();
    let attendent = modifier(racine, |lignes| {
        let mut n = 0;
        for l in lignes.iter_mut().filter(|l| en_attente(l)) {
            l.envoi.essai = Some(essai.clone());
            l.envoi.erreur = None;
            n += 1;
        }
        Ok((n, n > 0))
    })?;
    let Some(attendent) = attendent else { return Ok(bilan(0, 0, Some("La boîte des messages est occupée.".to_string()), false)) };
    if attendent == 0 {
        return Ok(bilan(0, 0, None, false));
    }
    let mut client = None;
    let (mut partis, mut tentes, mut panne) = (0, Vec::<String>::new(), None::<String>);
    while tentes.len() < PAR_PASSAGE {
        // Le plus ancien qui attend et n'a pas ete tente : un message enregistre pendant le passage part avec lui.
        let lignes = en_ordre(racine)?;
        let Some(id) = lignes.iter().filter(|l| en_attente(l)).map(|l| &l.id).find(|i| !tentes.contains(i)).cloned() else {
            break;
        };
        tentes.push(id.clone());
        let fichier = chemin_corps(racine, &id);
        if !fichier.is_file() {
            noter_raison(racine, &[id], "le contenu de ce message n'est plus sur cet ordinateur : il ne peut pas partir.")?;
            continue;
        }
        let corps: Corps<CaptureQuiPart> = socle::forme::lire_fichier(&fichier, "Le contenu d'un message au développeur ne se lit pas.")?;
        if client.is_none() {
            match incidents::canal(racine)? {
                Canal::Panne(e) => {
                    panne = Some(e.dite());
                    break;
                }
                Canal::Pret(c) => client = Some(c),
            }
        }
        let c = client.as_ref().expect("pose juste au-dessus");
        match c.signaler(&corps) {
            Ok(r) => {
                marquer_arrive(racine, &id, &r)?;
                socle::ecriture::effacer(&fichier).map_err(|m| erreur(Classe::Disque, m))?;
                partis += 1;
            }
            Err(e) => {
                panne = Some(e.dite());
                break;
            }
        }
    }
    // Ce qui attend encore, et que ce passage n'a pas fait partir, le dit : la panne si elle a eu lieu (elle vaut pour
    // tous), sinon qu'il partira au passage suivant.
    let reste = modifier(racine, |lignes| {
        let mut n = 0;
        let mut change = false;
        for l in lignes.iter_mut().filter(|l| en_attente(l)) {
            n += 1;
            match (&panne, &l.envoi.erreur) {
                (Some(p), _) => {
                    l.envoi.erreur = Some(format!("{} PuppetPaie le renverra de lui-même.", p));
                    change = true;
                }
                (None, None) => {
                    l.envoi.erreur = Some("il partira au prochain passage de la veille.".to_string());
                    change = true;
                }
                (None, Some(_)) => {}
            }
        }
        Ok((n, change))
    })?;
    match reste {
        Some(n) => Ok(bilan(partis, n, panne, false)),
        None => Ok(bilan(partis, attendent - partis, Some("La boîte des messages est occupée.".to_string()), false)),
    }
}

/// La ligne `id` est arrivee chez l'editeur : `depose`, avec ce que le serveur a rendu.
fn marquer_arrive(racine: &Path, id: &str, r: &Retour) -> R<()> {
    let quand = socle::paris::maintenant();
    let fait = modifier(racine, |lignes| {
        let mut change = false;
        for l in lignes.iter_mut().filter(|l| l.id == id) {
            l.envoi = EnvoiDuMessage { etat: DEPOSE, quand: Some(quand.clone()), essai: l.envoi.essai.clone(), erreur: None };
            l.numero = Some(r.numero.clone());
            l.ticket = Some(r.ticket.clone()).filter(|t| !t.is_empty());
            l.ticket_numero = r.ticket_numero;
            change = true;
        }
        Ok(((), change))
    })?;
    // La boite est restee occupee dix secondes : le message est parti, mais sa ligne le dirait en attente et la veille le
    // renverrait. On le dit, sans rien effacer.
    fait.ok_or_else(|| erreur(Classe::Refuse, format!("Le message {} est parti, mais la boîte des messages est restée occupée : il n'a pas pu être noté.", id)))
}

fn noter_raison(racine: &Path, ids: &[String], raison: &str) -> R<()> {
    modifier(racine, |lignes| {
        let mut change = false;
        for l in lignes.iter_mut().filter(|l| en_attente(l) && ids.contains(&l.id)) {
            l.envoi.erreur = Some(raison.to_string());
            change = true;
        }
        Ok(((), change))
    })?;
    Ok(())
}

/// Les fichiers d'attente dont le message n'attend plus (parti, ou que la boite ne connait pas : son enregistrement n'a
/// pas abouti, et la page l'a dit) s'effacent.
///
/// SOUS LE VERROU DE LA BOITE : `envoyer` y ecrit le corps et sa ligne ensemble ; un corps sans ligne, vu
/// d'ici, n'est jamais celui d'un message en train de s'enregistrer. Boite occupee : rien ne s'efface ce passage-ci.
fn effacer_les_partis(racine: &Path) -> R<()> {
    modifier(racine, |lignes| {
        let Some(fichiers) = socle::fichiers::lister(&dossier_attente(racine))? else { return Ok(((), false)) };
        for f in fichiers {
            let nom = match f.file_name() {
                Some(n) => n.to_string_lossy().into_owned(),
                None => continue,
            };
            let Some(id) = nom.strip_suffix(".json") else { continue };
            if !lignes.iter().any(|l| en_attente(l) && l.id == id) {
                socle::ecriture::effacer(&f).map_err(|m| erreur(Classe::Disque, m))?;
            }
        }
        Ok(((), false))
    })?;
    Ok(())
}

/// `relever_reponses` : rend le nombre de reponses nouvelles. Rien a demander,
/// rien ne part que les numeros des signalements qui n'en ont pas encore.
pub fn relever_reponses(racine: &Path) -> R<usize> {
    let attendent: Vec<String> = en_ordre(racine)?.into_iter().filter(|l| l.reponse.is_none()).filter_map(|l| l.numero).collect();
    if attendent.is_empty() {
        return Ok(0);
    }
    let client = match incidents::canal(racine)? {
        Canal::Panne(e) if e.est_filet() => return Ok(0), // la veille suivante reessaie
        Canal::Panne(e) => return Err(e.en_echec()),
        Canal::Pret(c) => c,
    };
    let debut = attendent.len().saturating_sub(200);
    let r = match client.reponses(&attendent[debut..]) {
        Ok(r) => r,
        Err(e) if e.est_filet() => return Ok(0), // la veille suivante reessaie
        Err(e) => return Err(e.en_echec()),
    };
    // Une reponse vide : l'editeur n'a pas encore repondu.
    let recues: Vec<_> = r.retours.into_iter().filter(|x| !x.reponse.is_empty()).collect();
    // La boite se relit sous son verrou apres l'appel : un message enregistre ou parti pendant ce temps reste.
    let n = modifier(racine, |lignes| {
        let mut n = 0;
        for l in lignes.iter_mut().filter(|l| l.reponse.is_none()) {
            let Some(x) = recues.iter().rev().find(|x| l.numero.as_deref() == Some(x.numero.as_str())) else { continue };
            l.reponse = Some(tete(&x.reponse, 2000));
            l.repondu_le = Some(x.repondu_le.clone()).filter(|q| !q.is_empty());
            l.lu = Some(false);
            n += 1;
        }
        Ok((n, n > 0))
    })?;
    n.ok_or_else(|| erreur(Classe::Refuse, "La boîte des messages est occupée : les réponses seront relevées au prochain passage."))
}

/// `signalements_lus` : la boite est ouverte, ce qu'elle montre est lu. Rend le
/// nombre marque.
pub fn signalements_lus(racine: &Path) -> R<usize> {
    let n = modifier(racine, |lignes| {
        let mut n = 0;
        for l in lignes.iter_mut().filter(|l| l.reponse.is_some()) {
            if l.lu != Some(true) {
                n += 1;
            }
            l.lu = Some(true);
        }
        Ok((n, n > 0))
    })?;
    n.ok_or_else(|| erreur(Classe::Refuse, "La boîte des messages est occupée : réessayez dans un instant."))
}
