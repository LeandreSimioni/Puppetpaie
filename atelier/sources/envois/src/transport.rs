//! Le transport vers net-entreprises et l'Urssaf : le client HTTP, l'authentification, les consultations, et l'envoi du
//! corps d'un depot (DSN, DPAE).
//!
//! CE MODULE NE CHERCHE JAMAIS UN SECRET : le mot de passe arrive de l'appelant, sert le temps de l'authentification
//! et n'est ecrit nulle part ; il n'ouvre pas le coffre et n'en parle pas. Le jeton, lui, vaut une session : c'est
//! l'appelant qui le garde en memoire et le repasse.
//!
//! Les pannes : `Organisme` (message et `code`) ; un delai de lecture (`DelaiDepasse`), une reponse coupee, une connexion
//! rompue (`Reseau`), une reponse mal formee, gzip ou protocole (`Illisible`, `mal_formee`) portent leur `classe`, que
//! l'appelant releve.

use droit::fiche::Lue as _;
use socle::echec::{Classe, Echec};
use socle::garde_reseau::Url;
use socle::grandeurs::Octets;
use std::io::Read;
use std::sync::mpsc;
use std::time::Duration;

use ureq::http;

pub const USER_AGENT: &str = "Client-DSN (PuppetPaie/1.0; PuppetPaie)";

/// Les messages de l'API par statut HTTP (guide, section 8).
pub const MESSAGES: [(http::StatusCode, &str); 7] = [
    (http::StatusCode::UNAUTHORIZED, "Authentification refusee, ou declarant non inscrit au service DSN."),
    (
        http::StatusCode::FORBIDDEN,
        "Compte temporairement bloque (trop de tentatives), ou tentative d'acces a des donnees deposees par quelqu'un d'autre.",
    ),
    (http::StatusCode::NOT_FOUND, "Service ou identifiant inconnu. Peut aussi signaler une interruption de service."),
    (http::StatusCode::NOT_ACCEPTABLE, "Format de reponse demande non supporte."),
    (http::StatusCode::UNPROCESSABLE_ENTITY, "Parametres invalides."),
    (
        http::StatusCode::TOO_MANY_REQUESTS,
        "Contraintes de consultation non respectees : trop d'appels, ou plage de recherche trop large. Attendre et relancer.",
    ),
    (http::StatusCode::SERVICE_UNAVAILABLE, "Service indisponible."),
];

const DELAI_APPEL: Duration = Duration::from_secs(60);
pub const DELAI_DEPOT: Duration = Duration::from_secs(120);
const MAX_REDIRECTIONS: usize = 10;

/// Ce que le guide dit d'un statut HTTP, s'il en dit quelque chose.
pub fn message_du_statut(code: http::StatusCode) -> Option<&'static str> {
    MESSAGES.iter().find(|(c, _)| *c == code).map(|(_, m)| *m)
}

/// CE QU'ON FAISAIT quand un tiers a mal repondu : le conseil de sa phrase en depend (`panne_dite`). Un meme statut ne
/// dit pas la meme chose selon le service appele (guide de l'API DSN v6.9, § 9.1.1 « Dépôt et retours », l. 5191-5232,
/// `atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.9-20260904.txt`) : un 403 est un
/// compte bloque a l'authentification, une declaration deposee par un autre a la consultation ; un 422 est un parametre
/// invalide a l'authentification, l'avis de rejet au depot. Seule l'identification renvoie la compagnie a ses mots de
/// passe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeQuOnFaisait {
    /// S'identifier aupres de l'organisme du canal (l'authentification du guide).
    Identifier(crate::releve_retours::CanalDesRetours),
    /// Relever ce que l'organisme sert : la liste des retours ou des depots, un retour a telecharger.
    Relever(crate::releve_retours::CanalDesRetours),
    /// Deposer la declaration du canal (une DSN, une DPAE).
    Deposer(crate::releve_retours::CanalDesRetours),
    /// Se servir du Drive (sauvegarder, restaurer, voir le compte lie), et le jeton d'acces qu'il demande.
    Sauvegarder,
    /// Lier le compte Google (l'autorisation).
    LierGoogle,
}

impl CeQuOnFaisait {
    /// Le tiers, tel qu'une phrase le nomme en tete.
    pub fn qui(self) -> &'static str {
        match self {
            CeQuOnFaisait::Identifier(c) | CeQuOnFaisait::Relever(c) | CeQuOnFaisait::Deposer(c) => c.organisme(),
            CeQuOnFaisait::Sauvegarder | CeQuOnFaisait::LierGoogle => "Google",
        }
    }
}

/// UNE PANNE D'UN TIERS, DITE A L'ADMINISTRATEUR -- le seul endroit ou la reponse d'un organisme (net-entreprises,
/// l'Urssaf) ou de Google devient une phrase que la compagnie lit. La phrase dit qui (`CeQuOnFaisait::qui`), ce que
/// cela peut etre pour ce qu'on faisait, et quoi faire ; elle ne porte ni adresse, ni code de reponse, ni corps, sauf
/// les motifs d'un avis de rejet, ecrits pour etre lus (`motifs_du_rejet`). Le detail technique (le statut, l'adresse
/// sans ses parametres, le debut du corps, ce que le systeme ou la bibliotheque a dit) va a part (`Lieu::detail`), que
/// le journal des incidents porte a l'editeur ; il est aussi note dans la memoire des echecs
/// (`socle::echec::noter_echec`). La classe est celle du statut (`classe_du_statut` : `Organisme`, `DepotRefuse` ou
/// `Google`), ou celle de la panne brute (un delai depasse reste `DelaiDepasse`) : rien ne change de ce qui est tente,
/// accepte ou refuse.
#[track_caller]
pub fn panne_dite(faisait: CeQuOnFaisait, classe_du_statut: Classe, url: &Url, p: Panne) -> EchecApi {
    let qui = faisait.qui();
    let (classe, phrase, detail, code) = match p {
        Panne::Statut { code, corps } => {
            let motifs = match (faisait, &corps) {
                (CeQuOnFaisait::Deposer(_), Ok(c)) if code == http::StatusCode::UNPROCESSABLE_ENTITY => motifs_du_rejet(c),
                _ => Vec::new(),
            };
            let phrase = phrase_du_statut_dite(faisait, code, &motifs);
            (Some(classe_du_statut), phrase, phrase_du_statut(code, &format!("sur {}", sans_secret(url.texte())), Some(debut_du_corps(corps, 800))), Some(code))
        }
        // « injoignable » : le mot que `sauvegarde::hors_ligne` lit pour dire que le reseau manque.
        Panne::Reseau(r) => {
            let phrase = match faisait {
                // La connexion a pu se rompre apres l'envoi (`panne_de` : ureq ne dit pas a quel moment).
                CeQuOnFaisait::Deposer(c) => format!(
                    "{} injoignable pendant le dépôt : vérifier la connexion internet. {} a pu partir avant la coupure : regarder ce que le relevé des retours en dit avant toute autre chose.",
                    qui,
                    majuscule(c.declaration())
                ),
                _ => format!("{} injoignable : vérifier la connexion internet ; nouvel essai plus tard.", qui),
            };
            (Some(classe_du_statut), phrase, format!("Reseau injoignable pour {} : {}", sans_secret(url.texte()), r), None)
        }
        Panne::Brute(e) => {
            let phrase = match (faisait, e.classe) {
                (CeQuOnFaisait::Deposer(c), Some(Classe::DelaiDepasse)) => peut_etre_recue(qui, "n'a pas répondu à temps au dépôt", c),
                (CeQuOnFaisait::Deposer(c), Some(Classe::Reseau)) => peut_etre_recue(qui, "a coupé sa réponse au dépôt", c),
                (CeQuOnFaisait::Deposer(c), _) => peut_etre_recue(qui, "a rendu au dépôt une réponse que le logiciel ne lit pas", c),
                (_, Some(Classe::DelaiDepasse)) => format!("{} n'a pas répondu à temps : nouvel essai plus tard.", qui),
                (_, Some(Classe::Reseau)) => format!("{} a coupé sa réponse en chemin : nouvel essai plus tard.", qui),
                _ => format!("{} a rendu une réponse que le logiciel ne lit pas : nouvel essai plus tard.", qui),
            };
            (e.classe, phrase, format!("{} : {}", sans_secret(url.texte()), e.phrase), None)
        }
    };
    socle::echec::noter_echec(classe, &detail);
    let a_part = socle::echec::Lieu { detail: Some(detail), ..socle::echec::Lieu::default() };
    EchecApi { echec: Echec { classe, phrase, a_part: Some(a_part.into()) }, code }
}

/// La phrase d'un statut hors 2xx, pour ce qu'on faisait. Les sens viennent du guide de l'API DSN v6.9, § 9.1.1
/// (l. 5191-5232), que l'API DPAE reprend (`deposer-dsn`, guide d'implementation de l'API DPAE) ; pour Google, de
/// « Resolve errors » (`atelier/reference/sources/google-drive-api/google-drive-api-gerer-les-erreurs-20261020.txt` :
/// 401, l. 549-573 : rafraichir le jeton, sinon refaire l'autorisation).
fn phrase_du_statut_dite(faisait: CeQuOnFaisait, code: http::StatusCode, motifs: &[String]) -> String {
    use http::StatusCode as S;
    let qui = faisait.qui();
    let espacer = || format!("{} demande d'espacer les appels : nouvel essai plus tard.", qui);
    match faisait {
        // Authentification : 401 erreur d'authentification, 403 compte bloque apres des tentatives infructueuses, 422
        // parametres invalides (dont les contraintes de format de net-entreprises).
        CeQuOnFaisait::Identifier(_) => match code {
            S::UNAUTHORIZED => format!("{} a refusé l'identification : vérifier les accès dans Configuration › Mots de passe.", qui),
            S::FORBIDDEN => format!("{} refuse l'accès : le compte est peut-être bloqué un temps après trop d'essais. Attendre, puis réessayer.", qui),
            S::UNPROCESSABLE_ENTITY => format!("{} refuse les identifiants envoyés : vérifier le compte (SIRET, nom, prénom sans accent) et son mot de passe dans Configuration › Mots de passe.", qui),
            S::TOO_MANY_REQUESTS => espacer(),
            _ => format!("{} n'a pas répondu comme attendu : le service est peut-être indisponible, ou les accès ne conviennent plus. Réessayer plus tard ; si cela dure, vérifier les accès dans Configuration › Mots de passe.", qui),
        },
        // Consultation et telechargement : 401 jeton invalide, ou declarant pas encore inscrit (« attendre J+1 ») ; 403
        // une declaration que ce compte n'a pas deposee ; 422 parametres invalides ; 429 les contraintes de consultation.
        CeQuOnFaisait::Relever(_) => match code {
            S::UNAUTHORIZED => format!("{} a refusé la consultation : la session n'est plus valable, ou le compte n'est inscrit à ce service que depuis aujourd'hui (il l'est le lendemain). Nouvel essai plus tard.", qui),
            S::FORBIDDEN => format!("{} refuse l'accès : ce qui est demandé porte sur une déclaration que ce compte n'a pas déposée.", qui),
            S::UNPROCESSABLE_ENTITY => format!("{} juge la demande de consultation invalide : nouvel essai plus tard.", qui),
            S::TOO_MANY_REQUESTS => espacer(),
            _ => format!("{} n'a pas répondu comme attendu : le service est peut-être indisponible. Nouvel essai plus tard.", qui),
        },
        // Depot : 401 jeton invalide, ou declarant pas encore inscrit ; 422 l'avis de rejet (§ 3.2.2, l. 1336-1365) ;
        // une erreur du service (5xx) ne dit pas si la declaration a ete recue.
        CeQuOnFaisait::Deposer(c) => match code {
            S::UNPROCESSABLE_ENTITY if motifs.is_empty() => format!("{} a rejeté {} (avis de rejet) : {}", qui, c.declaration(), c.ou_se_relit_l_avis_de_rejet()),
            S::UNPROCESSABLE_ENTITY => format!("{} a rejeté {} (avis de rejet) : {}", qui, c.declaration(), motifs.join(" ; ")),
            S::UNAUTHORIZED => format!("{} a refusé le dépôt : la session n'est plus valable, ou le compte n'est inscrit à ce service que depuis aujourd'hui (il l'est le lendemain). Nouvel essai plus tard.", qui),
            serveur if serveur.is_server_error() => peut_etre_recue(qui, "a répondu au dépôt par une erreur de son service", c),
            _ => format!("{} a refusé le dépôt : le service est peut-être indisponible. Nouvel essai plus tard.", qui),
        },
        CeQuOnFaisait::Sauvegarder => match code {
            S::UNAUTHORIZED => "Google ne reconnaît plus l'autorisation du logiciel : lier de nouveau le Drive (Configuration › Mots de passe).".to_string(),
            S::FORBIDDEN => "Google refuse cette action sur le Drive : nouvel essai plus tard ; si cela dure, lier de nouveau le Drive (Configuration › Mots de passe).".to_string(),
            S::TOO_MANY_REQUESTS => espacer(),
            _ => "Google n'a pas répondu comme attendu : nouvel essai plus tard.".to_string(),
        },
        CeQuOnFaisait::LierGoogle => "Google n'a pas accepté la liaison du compte : recommencer plus tard.".to_string(),
    }
}

/// UN DEPOT QUI N'A PAS ABOUTI SANS REFUS (un delai depasse, une reponse coupee, une erreur du service) : la declaration
/// a peut-etre ete recue, et ne se redepose pas a l'aveugle (regle de fond 4). L'accuse d'enregistrement ou l'avis de
/// rejet « sont téléchargeables à nouveau par les services de gestion des retours » (guide de l'API DSN v6.9, § 3.2.2,
/// l. 1338-1340) ; pour la DPAE, ils sont « mis à disposition sur le tableau de bord du déclarant » et une seconde DPAE
/// de la meme embauche est rejetee en doublon (`atelier/reference/norme-dpae/5491-API-DPAE-Fonctionnement.pdf`, p. 6 et
/// 9). Ce que le releve dira d'un depot sans reponse, aucun texte ne l'ecrit : INFERENCE, de ce que ces retours se
/// relisent.
fn peut_etre_recue(qui: &str, quoi: &str, c: crate::releve_retours::CanalDesRetours) -> String {
    format!("{} {} : {} a peut-être été reçue. Attendre ce que le relevé des retours en dit avant toute autre chose.", qui, quoi, c.declaration())
}

/// Le texte avec sa premiere lettre en capitale.
fn majuscule(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// LES MOTIFS D'UN AVIS DE REJET, tels que l'organisme les ecrit : le corps d'un 422 au depot est l'avis de rejet (guide
/// de l'API DSN v6.9, § 3.2.2, l. 1338-1365), au format des bilans, dont chaque anomalie porte son « message », le
/// « Message d'erreur » a afficher (`gip-mds-specifications-schema-bilans-vt2.2-20201118.txt`, l. 251-252). Un corps qui
/// ne se lit pas en bilan n'en donne aucun : la phrase dit alors ou l'avis se relit, et le corps entier part a part.
fn motifs_du_rejet(corps: &[u8]) -> Vec<String> {
    let lu = if corps.starts_with(&[0x1f, 0x8b]) {
        match gunzip(corps) {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        }
    } else {
        corps.to_vec()
    };
    let mut motifs: Vec<String> = Vec::new();
    for a in crate::retours::anomalies_codees_d_octets(&lu).into_iter().flatten() {
        let m = socle::texte::strip(&a.message).to_string();
        if !m.is_empty() && !motifs.contains(&m) {
            motifs.push(m);
        }
    }
    motifs
}

/// `HTTP <code> <ou>`, puis ce que le guide dit du statut et le debut du corps de la reponse, chacun sur sa ligne.
fn phrase_du_statut(code: http::StatusCode, ou: &str, corps: Option<String>) -> String {
    let mut m = format!("HTTP {} {}", code.as_str(), ou);
    for ligne in message_du_statut(code).map(String::from).into_iter().chain(corps.filter(|c| !socle::texte::strip(c).is_empty())) {
        m.push_str("\n  ");
        m.push_str(&ligne);
    }
    m
}

// ----------------------------------------------------------------- les pannes

/// Ce qui peut mal tourner, en trois formes.
#[derive(Debug)]
pub enum Panne {
    /// Un statut HTTP hors 2xx ; le corps, lu en entier (ou la panne de sa lecture).
    Statut { code: http::StatusCode, corps: Result<Vec<u8>, Box<Panne>> },
    /// La connexion n'a pas abouti : la raison.
    Reseau(String),
    /// Une panne après l'envoi : son refus, né avec sa classe.
    Brute(Echec),
}

impl Panne {
    #[track_caller]
    fn brute(classe: Classe, message: impl Into<String>) -> Panne {
        Panne::Brute(socle::echec::erreur(classe, message))
    }
}

/// UNE PANNE D'UN ORGANISME, telle que la releve la note : son refus, de classe `Organisme`, `DepotRefuse`, ou celle
/// d'une panne brute, et le statut HTTP quand l'organisme a repondu.
#[derive(Debug, Clone)]
pub struct EchecApi {
    pub echec: Echec,
    pub code: Option<http::StatusCode>,
}

impl EchecApi {
    #[track_caller]
    fn nouveau(classe: Classe, erreur: String, code: Option<http::StatusCode>) -> EchecApi {
        EchecApi { echec: socle::echec::erreur(classe, erreur), code }
    }

    /// Un refus de la plate-forme elle-meme (`Organisme`, `DepotRefuse`) : la releve le note et continue.
    pub fn est_api(&self) -> bool {
        matches!(self.echec.classe, Some(Classe::Organisme | Classe::DepotRefuse))
    }

    /// L'organisme refuse ce qu'on lui a envoye (les identifiants, le compte bloque, des appels trop rapproches : 401, 403,
    /// 422, 429) : une situation d'usage, pas un defaut, pour le journal des incidents.
    pub fn dit_un_refus(&self) -> bool {
        matches!(
            self.code,
            Some(http::StatusCode::UNAUTHORIZED | http::StatusCode::FORBIDDEN | http::StatusCode::UNPROCESSABLE_ENTITY | http::StatusCode::TOO_MANY_REQUESTS)
        )
    }

    /// La panne comme un refus de route.
    pub fn en_echec(self) -> Echec {
        self.echec
    }
}

impl From<Echec> for EchecApi {
    fn from(echec: Echec) -> Self {
        EchecApi { echec, code: None }
    }
}

fn echec_brut(p: Panne) -> EchecApi {
    match p {
        Panne::Brute(e) => EchecApi::from(e),
        // Les appelants traduisent les deux autres avant d'arriver ici.
        Panne::Reseau(r) => EchecApi::nouveau(Classe::Reseau, r, None),
        Panne::Statut { code, .. } => EchecApi::nouveau(Classe::Reseau, format!("HTTP {}", code.as_str()), Some(code)),
    }
}

// ------------------------------------------------------- les petits outils

use socle::texte::blanc as blanc;

/// Les mots d'un texte, separes par du blanc.
fn mots(s: &str) -> Vec<&str> {
    s.split(blanc).filter(|m| !m.is_empty()).collect()
}

/// `s[:n]` sur des points de code.
fn debut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Des octets lus en UTF-8, un caractère de remplacement pour ce qui ne se lit pas.
fn utf8_remplace(o: &[u8]) -> String {
    String::from_utf8_lossy(o).into_owned()
}

/// Jamais un identifiant dans un message.
pub fn sans_secret(url: &str) -> &str {
    avant(url, &['?'])
}

/// Ce qui precede le premier des `signes` ; aucun : tout le texte.
fn avant<'a>(t: &'a str, signes: &[char]) -> &'a str {
    match t.find(signes) {
        Some(i) => &t[..i],
        None => t,
    }
}


/// Les trois caracteres que le HTML reserve (`&`, `<`, `>`) echappes.
pub fn echapper(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `AAAAMMJJhhmmss` d'un instant a l'heure de Paris, la forme des plages de consultation.
fn horodatage(m: socle::paris::Moment) -> String {
    socle::paris::ecrit::compacte::texte(m)
}

// ------------------------------------------------------------------- gzip

/// UNE REPONSE MAL FORMEE (une compression fausse, un protocole que le client ne lit pas) : le corps est arrive, mais ne
/// se lit pas. Ce n'est pas une coupure (`Classe::Reseau`) : la phrase le dit (`panne_dite` : « a rendu une réponse que
/// le logiciel ne lit pas »), et ce n'est pas une panne de passage (`suites::panne_de_passage`) : le detail part chez
/// l'editeur.
#[track_caller]
fn mal_formee(message: impl Into<String>) -> Panne {
    Panne::brute(Classe::Illisible, message)
}

/// Decompresse un corps gzip ; les refus (tronque, en-tete invalide, somme de controle) sont dits en francais. Le corps
/// est arrive en entier (`lire_corps`) : un gzip tronque est une reponse mal formee.
pub fn gunzip(donnees: &[u8]) -> Result<Vec<u8>, Panne> {
    let eof = || mal_formee("réponse gzip tronquée");
    let mut sortie = Vec::new();
    let mut reste = donnees;
    loop {
        if reste.is_empty() {
            return Ok(sortie);
        }
        if reste.len() < 2 || reste[0] != 0x1f || reste[1] != 0x8b {
            let debut: Vec<String> = reste.iter().take(2).map(|o| format!("{:02x}", o)).collect();
            return Err(mal_formee(format!("réponse attendue en gzip, elle commence par {}", debut.join(" "))));
        }
        let mut pos = 2;
        let lire = |pos: &mut usize, n: usize| -> Result<(), Panne> {
            if reste.len() < *pos + n {
                return Err(eof());
            }
            *pos += n;
            Ok(())
        };
        if reste.len() < 10 {
            return Err(eof());
        }
        let (methode, drapeaux) = (reste[2], reste[3]);
        pos += 8;
        if methode != 8 {
            return Err(mal_formee("réponse gzip : méthode de compression inconnue"));
        }
        if drapeaux & 4 != 0 {
            if reste.len() < pos + 2 {
                return Err(eof());
            }
            let n = u16::from_le_bytes([reste[pos], reste[pos + 1]]) as usize;
            pos += 2;
            lire(&mut pos, n)?;
        }
        for drapeau in [8u8, 16u8] {
            if drapeaux & drapeau != 0 {
                loop {
                    match reste.get(pos) {
                        None => break,
                        Some(0) => {
                            pos += 1;
                            break;
                        }
                        Some(_) => pos += 1,
                    }
                }
            }
        }
        if drapeaux & 2 != 0 {
            lire(&mut pos, 2)?;
        }
        let mut d = flate2::Decompress::new(false);
        let mut membre = Vec::new();
        let en_entree = &reste[pos..];
        loop {
            let avant_in = d.total_in() as usize;
            let avant_out = d.total_out();
            membre.reserve(32 * 1024);
            let st = d
                .decompress_vec(&en_entree[avant_in..], &mut membre, flate2::FlushDecompress::None)
                .map_err(|e| mal_formee(format!("réponse gzip illisible : {}", e)))?;
            if st == flate2::Status::StreamEnd {
                break;
            }
            if d.total_in() as usize == avant_in && d.total_out() == avant_out {
                return Err(eof());
            }
        }
        let utilises = d.total_in() as usize;
        let apres = &en_entree[utilises..];
        if apres.len() < 8 {
            return Err(eof());
        }
        let crc = u32::from_le_bytes([apres[0], apres[1], apres[2], apres[3]]);
        let longueur = u32::from_le_bytes([apres[4], apres[5], apres[6], apres[7]]);
        let mut h = flate2::Crc::new();
        h.update(&membre);
        if crc != h.sum() {
            return Err(mal_formee("réponse gzip : somme de contrôle fausse"));
        }
        if longueur != (membre.len() as u64 & 0xffff_ffff) as u32 {
            return Err(mal_formee("réponse gzip : longueur fausse"));
        }
        sortie.extend_from_slice(&membre);
        let mut suite = &apres[8..];
        while let Some((0, r)) = suite.split_first().map(|(a, r)| (*a, r)) {
            suite = r;
        }
        reste = suite;
    }
}

// ------------------------------------------------------------------- HTTP

pub type Entetes = Vec<(String, String)>;

/// `Content-Type` -> `Content-Type`, comme un serveur les ecrit.
fn nom_canonique(n: &str) -> String {
    n.split('-')
        .map(|m| {
            let mut c = m.chars();
            let premiere: String = c.next().map(|p| p.to_ascii_uppercase()).into_iter().collect();
            premiere + c.as_str()
        })
        .collect::<Vec<_>>()
        .join("-")
}

fn errno_texte(e: &std::io::Error) -> String {
    match e.raw_os_error() {
        Some(n) => {
            let t = e.to_string();
            let suffixe = format!(" (os error {})", n);
            let t = match t.strip_suffix(&suffixe) {
                Some(sans) => sans,
                None => &t,
            };
            format!("[Errno {}] {}", n, t)
        }
        None => e.to_string(),
    }
}

fn delai_expire(https: bool) -> &'static str {
    if https {
        "The read operation timed out"
    } else {
        "timed out"
    }
}

/// Une panne de `ureq` en refus du moteur : `Panne::Reseau` avant la reponse, une panne brute apres l'envoi.
fn panne_de(e: ureq::Error, https: bool) -> Panne {
    use std::io::ErrorKind as K;
    use ureq::{Error, Timeout};
    match e {
        Error::Timeout(Timeout::Connect) | Error::Timeout(Timeout::Resolve) => Panne::Reseau("timed out".into()),
        Error::Timeout(_) => Panne::brute(Classe::DelaiDepasse, delai_expire(https)),
        Error::HostNotFound => Panne::Reseau("[Errno -2] Name or service not known".into()),
        Error::Io(io) => match io.kind() {
            K::UnexpectedEof => Panne::brute(Classe::Reseau, "Remote end closed connection without response"),
            // Observe sur le service reel : la connexion est rompue pendant
            // l'etablissement (mandataire, TLS) : la connexion n'a pas abouti.
            // ureq ne dit pas a quel moment.
            K::ConnectionReset | K::ConnectionAborted => Panne::Reseau(errno_texte(&io)),
            K::TimedOut if io.raw_os_error().is_none() => Panne::Reseau("timed out".into()),
            _ => Panne::Reseau(errno_texte(&io)),
        },
        Error::Protocol(p) => mal_formee(p.to_string()),
        Error::Rustls(r) => Panne::Reseau(format!("[SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: {}", r)),
        Error::Tls(t) => Panne::Reseau(format!("[SSL] {}", t)),
        Error::BadUri(u) => Panne::brute(Classe::Illisible, format!("unknown url type: {:?}", u)),
        autre => Panne::Reseau(autre.to_string()),
    }
}

/// Le mandataire de l'environnement : `https_proxy` pour
/// une adresse https, `http_proxy` pour une adresse http, et `no_proxy`
/// (noms, suffixes de domaine, `*`) qui l'ecarte. La machine elle-meme (les faux
/// serveurs de l'atelier et des essais) ne passe jamais par un mandataire.
fn mandataire(url: &Url) -> Result<Option<ureq::Proxy>, Panne> {
    let Some((schema, reste)) = url.texte().split_once("://") else { return Ok(None) };
    // La variable en minuscules, sinon en capitales ; posee mais vide, ou hors d'Unicode, elle ne dit rien.
    let lue = |n: &str| match std::env::var(n) {
        Ok(v) => Some(v),
        Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(..)) => None,
    };
    let var = |n: &str| lue(&n.to_lowercase()).or_else(|| lue(&n.to_uppercase())).filter(|v| !v.is_empty());
    let Some(valeur) = var(&format!("{}_proxy", schema.to_lowercase())) else { return Ok(None) };
    let autorite = avant(reste, &['/', '?', '#']);
    let hote = match autorite.rsplit_once('@') {
        Some((_, h)) => h,
        None => autorite,
    };
    let nom = match hote.strip_prefix('[') {
        Some(entre_crochets) => avant(entre_crochets, &[']']),
        None => avant(hote, &[':']),
    };
    if matches!(nom.to_lowercase().as_str(), "127.0.0.1" | "localhost" | "::1") {
        return Ok(None);
    }
    if let Some(np) = var("no_proxy") {
        for e in np.split(',') {
            let e = socle::texte::strip(e).trim_start_matches('.').to_lowercase();
            if e == "*" {
                return Ok(None);
            }
            if e.is_empty() {
                continue;
            }
            let (h, a) = (nom.to_lowercase(), hote.to_lowercase());
            let correspond = |x: &str| x == e || x.ends_with(&format!(".{}", e));
            if correspond(&h) || correspond(&a) {
                return Ok(None);
            }
        }
    }
    ureq::Proxy::new(&valeur).map(Some).map_err(|e| Panne::Reseau(format!("mandataire illisible ({}_proxy) : {}", schema.to_lowercase(), e)))
}

/// L'agent d'une demande. En essai, le garde-fou reseau (`garde_reseau`) le regle : rien ne sort de la machine.
fn agent(delai: Duration, url: &Url) -> Result<ureq::Agent, Panne> {
    let tls = url.texte().starts_with("https:");
    let d = Some(delai);
    let mut c = ureq::Agent::config_builder()
        .proxy(mandataire(url)?)
        .http_status_as_error(false)
        .max_redirects(0)
        .max_redirects_will_error(false)
        .user_agent(USER_AGENT)
        .accept("")
        .timeout_resolve(d)
        .timeout_connect(d)
        .timeout_send_request(d)
        .timeout_send_body(d)
        .timeout_recv_response(d);
    if tls {
        c = c.tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build());
    }
    let c = socle::garde_reseau::regler(c, url).map_err(|e| Panne::Reseau(e.phrase))?;
    Ok(c.build().into())
}

/// Lit le corps par morceaux : un silence de `delai`
/// secondes entre deux morceaux, pas une duree totale.
fn lire_corps(corps: ureq::Body, delai: Duration, attendu: Option<Octets>, https: bool) -> Result<Vec<u8>, Panne> {
    let (tx, rx) = mpsc::channel::<Result<Vec<u8>, std::io::Error>>();
    std::thread::spawn(move || {
        let mut r = corps.into_reader();
        let mut tampon = vec![0u8; 64 * 1024];
        loop {
            // Un morceau vide dit la fin du corps ; une erreur aussi.
            let (morceau, dernier) = match r.read(&mut tampon) {
                Ok(0) => (Ok(Vec::new()), true),
                Ok(n) => (Ok(tampon[..n].to_vec()), false),
                Err(e) => (Err(e), true),
            };
            match tx.send(morceau) {
                Ok(()) => {}
                // `SendError` ne dit qu'une chose : le lecteur est parti, apres avoir rendu la panne du delai depasse ;
                // ce morceau ne sert plus a personne.
                Err(mpsc::SendError(_)) => return,
            }
            if dernier {
                return;
            }
        }
    });
    let mut tout = Vec::new();
    loop {
        match rx.recv_timeout(delai) {
            Ok(Ok(m)) if m.is_empty() => return Ok(tout),
            Ok(Ok(m)) => tout.extend_from_slice(&m),
            Ok(Err(e)) => {
                if matches!(e.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted) {
                    return Err(Panne::brute(Classe::Reseau, errno_texte(&e)));
                }
                return Err(coupee(tout.len(), attendu));
            }
            // Le silence a dure le delai (ou le fil de lecture s'est arrete sans rien dire).
            Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => return Err(Panne::brute(Classe::DelaiDepasse, delai_expire(https))),
        }
    }
}

/// N octets lus, M attendus de plus.
fn coupee(lus: usize, attendu: Option<Octets>) -> Panne {
    let message = match attendu {
        Some(a) if a > Octets::de_longueur(lus) => format!("IncompleteRead({} bytes read, {} more expected)", lus, a.en_usize() - lus),
        _ => format!("IncompleteRead({} bytes read)", lus),
    };
    Panne::brute(Classe::Reseau, message)
}

/// L'adresse d'un `Location` de redirection (une reponse du tiers), lue depuis l'adresse demandée ; une adresse qui ne
/// commence pas par `http://` ou `https://` se refuse.
fn joindre(base: &Url, loc: &str) -> socle::echec::R<Url> {
    let loc = loc.replace(' ', "%20");
    let jointe = 'j: {
        if let Some(i) = loc.find("://") {
            if loc[..i].chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
                break 'j loc;
            }
        }
        let (schema, reste) = match base.texte().split_once("://") {
            Some((s, r)) => (s, r),
            None => break 'j loc,
        };
        if let Some(l) = loc.strip_prefix("//") {
            break 'j format!("{}://{}", schema, l);
        }
        let (autorite, chemin) = match reste.find(['/', '?']) {
            Some(i) => (&reste[..i], &reste[i..]),
            None => (reste, ""),
        };
        if loc.starts_with('/') {
            break 'j format!("{}://{}{}", schema, autorite, loc);
        }
        let chemin = avant(chemin, &['?', '#']);
        let dossier = match chemin.rfind('/') {
            Some(i) => &chemin[..=i],
            None => "/",
        };
        format!("{}://{}{}{}", schema, autorite, dossier, loc)
    };
    Url::lire(&jointe)
}

fn en_tetes_de(parts: &http::response::Parts) -> Entetes {
    let mut v: Entetes = Vec::new();
    for (n, val) in parts.headers.iter() {
        let nom = nom_canonique(n.as_str());
        let val = String::from_utf8_lossy(val.as_bytes()).into_owned();
        // Un en-tête répété : le premier l'emporte.
        if !v.iter().any(|(k, _)| *k == nom) {
            v.push((nom, val));
        }
    }
    v
}

pub fn entete<'a>(e: &'a Entetes, nom: &str) -> Option<&'a str> {
    e.iter().find(|(k, _)| k.eq_ignore_ascii_case(nom)).map(|(_, v)| v.as_str())
}

/// Ce qu'on envoie : une requete du guide.
pub struct Requete<'a> {
    pub url: &'a Url,
    pub jeton: Option<&'a socle::secret::Secret>,
    pub donnees: Option<&'a [u8]>,
    pub content_type: Option<&'a str>,
    /// L'en-tete `Content-Encoding: gzip` d'un depot.
    pub corps_gzip: bool,
    /// Un depot part toujours en POST, meme vide.
    pub post: bool,
    /// L'attente de la reponse : la connexion, et chaque silence de la lecture du corps.
    pub delai: Duration,
    /// Des en-tetes en plus (le canal de l'editeur : `User-Agent`, `X-Filet-Cle`).
    pub entetes: &'a [(&'a str, &'a str)],
    /// Ne pas demander `gzip` (le canal de l'editeur ne le demande pas).
    pub sans_gzip: bool,
    /// Une autre methode que GET ou POST (`PATCH` du Drive) ; elle part avec son corps, meme vide.
    pub methode: Option<&'a str>,
}

impl<'a> Requete<'a> {
    /// Un GET de `url`, sans jeton, sans corps ni en-tete de plus, qui demande `gzip`.
    pub fn get(url: &'a Url, delai: Duration) -> Requete<'a> {
        Requete { url, jeton: None, donnees: None, content_type: None, corps_gzip: false, post: false, delai, entetes: &[], sans_gzip: false, methode: None }
    }
}

/// Le corps de la reponse (decompresse si `Content-Encoding: gzip`) et ses
/// en-tetes ; une panne sinon (`Panne::Statut` pour un statut hors 2xx).
pub fn envoyer(r: &Requete) -> Result<(Vec<u8>, Entetes), Panne> {
    let https = r.url.texte().starts_with("https:");
    let mut url = r.url.clone();
    let mut donnees = r.donnees;
    let mut methode = r.methode;
    let mut post = r.post || methode.is_some() || donnees.is_some_and(|d| !d.is_empty());
    let mut avec_entetes_du_corps = true;
    let mut sauts = 0;
    let (parts, corps) = loop {
        let verbe = match methode {
            Some(m) => m,
            None if post => "POST",
            None => "GET",
        };
        let mut b = http::Request::builder().method(verbe).uri(url.texte());
        if !r.sans_gzip {
            b = b.header("Accept-Encoding", "gzip");
        }
        for (nom, valeur) in r.entetes {
            b = b.header(*nom, *valeur);
        }
        if avec_entetes_du_corps {
            // Un POST sans type de contenu part en formulaire.
            match r.content_type {
                Some(ct) => b = b.header("Content-Type", ct),
                None if post => b = b.header("Content-Type", "application/x-www-form-urlencoded"),
                None => {}
            }
            if r.corps_gzip {
                b = b.header("Content-Encoding", "gzip");
            }
        }
        // Le jeton ne suit une redirection que vers le meme hote.
        if let Some(j) = r.jeton {
            if sauts == 0 || meme_hote(r.url.texte(), url.texte()) {
                b = b.header("Authorization", format!("DSNLogin jeton={}", j.devoiler()));
            }
        }
        let ag = agent(r.delai, &url)?;
        let rep = if post {
            // Un POST sans donnees part avec un corps vide.
            let corps: Vec<u8> = donnees.into_iter().flatten().copied().collect();
            ag.run(b.body(corps).map_err(|e| Panne::brute(Classe::Illisible, e.to_string()))?)
        } else {
            ag.run(b.body(()).map_err(|e| Panne::brute(Classe::Illisible, e.to_string()))?)
        }
        .map_err(|e| panne_de(e, https))?;
        let (parts, corps) = rep.into_parts();
        let code = parts.status.as_u16();
        let suite = matches!(code, 301 | 302 | 303 | 307 | 308) && (!post || code <= 303) && sauts < MAX_REDIRECTIONS;
        // Une redirection dont l'adresse n'est pas de l'ASCII : le tiers a mal repondu.
        let loc = match (suite, parts.headers.get(http::header::LOCATION)) {
            (true, Some(v)) => Some(
                v.to_str().map_err(|e| Panne::brute(Classe::Illisible, format!("HTTP {} : adresse de redirection illisible ({})", code, e)))?.to_string(),
            ),
            _ => None,
        };
        match (suite, loc) {
            (true, Some(l)) => {
                url = joindre(&url, &l).map_err(|e| Panne::brute(Classe::Illisible, format!("HTTP {} : adresse de redirection illisible ({})", code, e.phrase)))?;
                if post {
                    post = false;
                    methode = None;
                    donnees = None;
                    avec_entetes_du_corps = false;
                }
                sauts += 1;
                continue;
            }
            _ => break (parts, corps),
        }
    };
    let code = parts.status;
    let entetes = en_tetes_de(&parts);
    // Une longueur annoncee qui ne se lit pas n'annonce rien.
    let attendu = entete(&entetes, "Content-Length").and_then(|v| Octets::ecrite(v.trim()));
    let brut = lire_corps(corps, r.delai, attendu, https);
    if !code.is_success() {
        return Err(Panne::Statut { code, corps: brut.map_err(Box::new) });
    }
    let mut brut = brut?;
    if entete(&entetes, "Content-Encoding") == Some("gzip") {
        brut = gunzip(&brut)?;
    }
    Ok((brut, entetes))
}

fn meme_hote(a: &str, b: &str) -> bool {
    let hote = |u: &str| u.split_once("://").map(|(_, r)| avant(r, &['/', '?', '#']).to_string());
    hote(a) == hote(b)
}

// ------------------------------------------------- l'API des organismes

/// Les `n` premiers caracteres d'une reponse d'un tiers, lue en UTF-8 : ce que le detail d'une panne en garde.
pub fn debut_lisible(o: &[u8], n: usize) -> String {
    debut(&utf8_remplace(o), n)
}

/// Le debut lisible du corps d'une reponse d'erreur, ou la panne de sa lecture.
fn debut_du_corps(corps: Result<Vec<u8>, Box<Panne>>, n: usize) -> String {
    match corps {
        Ok(c) => debut_lisible(&c, n),
        Err(illisible) => format!("Le corps de la reponse ne s'est pas lu : {}", echec_brut(*illisible).echec.phrase),
    }
}

/// Une panne d'appel a un organisme pendant ce qu'on faisait, dite a l'administrateur (`panne_dite`) : un statut ou un
/// reseau injoignable en `Organisme`, une panne brute dans sa classe.
#[track_caller]
fn en_erreur_api(faisait: CeQuOnFaisait, url: &Url, p: Panne) -> EchecApi {
    panne_dite(faisait, Classe::Organisme, url, p)
}

/// Un GET sous le jeton de la session, a l'organisme du canal `canal`, pour relever ce qu'il sert : le corps et les
/// en-tetes ; une panne en `Organisme`.
pub fn appeler(canal: crate::releve_retours::CanalDesRetours, url: &Url, jeton: Option<&socle::secret::Secret>) -> Result<(Vec<u8>, Entetes), EchecApi> {
    let r = Requete { jeton, ..Requete::get(url, DELAI_APPEL) };
    envoyer(&r).map_err(|p| en_erreur_api(CeQuOnFaisait::Relever(canal), url, p))
}

/// La consultation d'une plage : l'adresse de base, puis `debut[/fin]` en `AAAAMMJJhhmmss` (des heures de Paris).
pub fn consulter(canal: crate::releve_retours::CanalDesRetours, base: &Url, jeton: Option<&socle::secret::Secret>, debut: socle::paris::Moment, fin: Option<socle::paris::Moment>) -> Result<Vec<u8>, EchecApi> {
    let mut suite = horodatage(debut);
    if let Some(fin) = fin {
        suite.push('/');
        suite.push_str(&horodatage(fin));
    }
    appeler(canal, &base.joindre(&suite), jeton).map(|(corps, _)| corps)
}

/// LE DECLARANT QUI S'AUTHENTIFIE : le SIRET, le nom et le prenom du compte ouvert pour le service (vides : la fiche ne
/// les dit pas).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Identite {
    pub siret: Option<droit::fiche::Siret>,
    pub nom: Option<String>,
    pub prenom: Option<String>,
}

impl Identite {
    /// Ce qui lui manque pour s'authentifier, dit en mots.
    pub fn manquants(&self) -> Vec<&'static str> {
        [(self.siret.is_none(), "le SIRET"), (self.nom.is_none(), "le nom"), (self.prenom.is_none(), "le prénom")].into_iter().filter(|(manque, _)| *manque).map(|(_, m)| m).collect()
    }
}

/// L'identite du declarant inscrit au service DSN, telle que l'authentification l'envoie : celle que la fiche dit
/// (`declarant`), sinon le SIRET et le contact DSN coupe au premier espace (le nom, puis le prenom).
pub fn declarant(ent: &droit::fiche::ChampsEntreprise) -> Identite {
    if let Some(d) = ent.declarant.dite().filter(|d| !d.est_vide()) {
        return Identite { siret: d.siret.dite().cloned(), nom: d.nom.en_texte(), prenom: d.prenom.en_texte() };
    }
    let mut i = Identite { siret: ent.siret.dite().cloned(), nom: None, prenom: None };
    if let Some(brut) = ent.declarant_dsn.dite().and_then(|d| d.nom_prenom.portee()) {
        let morceaux = mots(socle::texte::strip(brut));
        if morceaux.len() >= 2 {
            i.nom = Some(morceaux[0].to_string());
            i.prenom = Some(morceaux[1..].join(" "));
        }
    }
    i
}

/// Ce qu'une authentification envoie : le declarant inscrit au service, le mot de passe deja prepare par l'appelant, le
/// service, et le jeton d'une session deja ouverte.
pub struct Authentification<'a> {
    /// Le canal de l'organisme qui authentifie : sa panne le nomme.
    pub canal: crate::releve_retours::CanalDesRetours,
    pub url: &'a Url,
    pub declarant: &'a Identite,
    pub motdepasse: Option<&'a socle::secret::Secret>,
    pub service: Option<&'a str>,
    pub jeton: Option<&'a socle::secret::Secret>,
}

/// UNE SESSION OUVERTE : son jeton.
#[derive(Debug, Clone)]
pub struct Session {
    pub jeton: socle::secret::Secret,
}

/// L'authentification du guide de l'API DSN : la session ouverte. Sans mot de passe ou sans declarant complet, rien ne
/// part. Le mot de passe ne survit pas a cet appel et n'entre dans aucun message.
pub fn authentifier(a: &Authentification) -> Result<Session, EchecApi> {
    let mdp = match a.motdepasse {
        Some(m) if !m.devoiler().is_empty() => m,
        _ => {
            return Err(EchecApi::nouveau(
                Classe::Organisme,
                format!("Le mot de passe de {} n'est pas posé : le poser dans Configuration › Mots de passe. Rien n'est parti.", a.canal.service_du_mot_de_passe()),
                None,
            ))
        }
    };
    let decl = a.declarant;
    let (Some(siret), Some(nom), Some(prenom)) = (&decl.siret, &decl.nom, &decl.prenom) else {
        return Err(EchecApi::nouveau(
            Classe::Organisme,
            format!(
                "Le compte qui se connecte est incomplet, il manque {} : ceux du compte ouvert pour ce service, sans accent. Rien n'est parti.",
                decl.manquants().join(", ")
            ),
            None,
        ));
    };
    let service = a.service.filter(|s| !s.is_empty());
    let corps = format!(
        "<identifiants><siret>{}</siret><nom>{}</nom><prenom>{}</prenom><motdepasse>{}</motdepasse>{}</identifiants>",
        AsRef::<str>::as_ref(siret),
        nom,
        prenom,
        echapper(mdp.devoiler()),
        // Le service ne s'ecrit que s'il est dit.
        service.iter().map(|s| format!("<service>{}</service>", s)).collect::<String>()
    );
    let url = a.url;
    let r = Requete {
        jeton: a.jeton,
        donnees: Some(corps.as_bytes()),
        content_type: Some("application/xml"),
        ..Requete::get(url, DELAI_APPEL)
    };
    let (brut, _) = envoyer(&r).map_err(|p| en_erreur_api(CeQuOnFaisait::Identifier(a.canal), url, p))?;
    let jeton: String = brut.iter().map(|&b| if b < 0x80 { b as char } else { '\u{fffd}' }).collect();
    Ok(Session { jeton: socle::secret::Secret::nouveau(socle::texte::strip(&jeton).to_string()) })
}

/// L'envoi du corps d'un depot (deja prepare : octets gzip) : POST `text/plain`,
/// `Content-Encoding: gzip`, jeton DSNLogin. `forme` dit comment une panne se
/// raconte, `dsn` (`DepotRefuse`) ou `dpae` (`Organisme`). `_au_jour` : la porte du depot l'a ouverte le jour reel de
/// la machine (`branchement::Branchement::ouvrir_le_depot`) ; sans lui, rien ne part.
pub fn depot(_au_jour: &crate::branchement::AuJourDuJour, url: &Url, jeton: &socle::secret::Secret, corps_gzip: &[u8], forme: &str, delai: Duration) -> Result<(Vec<u8>, Entetes), EchecApi> {
    let r = Requete {
        jeton: Some(jeton),
        donnees: Some(corps_gzip),
        content_type: Some("text/plain"),
        corps_gzip: true,
        post: true,
        ..Requete::get(url, delai)
    };
    envoyer(&r).map_err(|p| {
        let (canal, classe) = match forme {
            "dsn" => (crate::releve_retours::CanalDesRetours::Dsn, Classe::DepotRefuse),
            _ => (crate::releve_retours::CanalDesRetours::Dpae, Classe::Organisme),
        };
        let p = match (forme, p) {
            // Le refus d'un depot DSN dont le corps ne se lit pas : la panne de sa lecture, dans sa classe.
            ("dsn", Panne::Statut { corps: Err(illisible), .. }) => *illisible,
            (_, p) => p,
        };
        // Un depot ne rend pas le statut : il se juge a sa classe.
        EchecApi { code: None, ..panne_dite(CeQuOnFaisait::Deposer(canal), classe, url, p) }
    })
}
