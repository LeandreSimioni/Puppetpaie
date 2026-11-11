//! Le client Drive (`google.Client`) : les dossiers, le listage, le televersement.
//!
//! Les appels passent par `transport::envoyer`. Le client recoit ce que l'appelant a lu
//! sur le poste (`Autorisation` : les identifiants de l'application, le jeton scelle,
//! l'adresse du compte ; `routes::coffre::client_drive`) : il n'ouvre jamais le coffre.
//!
//! LE TRAIT `Drive` : la sauvegarde et l'archivage ne connaissent que lui (comme
//! `Courriel` pour la messagerie). Le vrai `Client` l'implemente ; la preuve met a sa
//! place un faux, ou le vrai `Client` pointe sur un serveur local (`Adresses`).
//!
//! Ce module ne parle jamais a Google tout seul : `Client::nouveau` ne fait aucun appel
//! reseau ; le premier est `jeton_acces`, au premier
//! `appel`.
//!
//! `drive.file` ne laisse voir que ce que l'application a cree : la recherche d'un dossier
//! ne retrouvera jamais un dossier fait a la main, meme du meme nom. C'est voulu.

use std::collections::BTreeMap;
use std::path::Path;

use socle::echec::{Classe, Echec};

use serde::{Deserialize, Serialize};

use crate::google::{encoder, jeton_par_rafraichissement, lue, phrase_de_panne, Application, JETONS};
use crate::transport::{envoyer, Requete};

/// UN FICHIER (OU UN DOSSIER) TEL QUE DRIVE LE REND, des seuls champs demandes (`fields`) : un element d'un listage, un
/// dossier cree, un fichier depose. Une reponse de Google : une cle qu'il ajoute est ignoree a la porte
/// (`socle::forme::lire_reponse_tiers`).
#[derive(Debug, Deserialize)]
struct FichierDrive {
    id: Option<String>,
    name: Option<String>,
    #[serde(rename = "md5Checksum")]
    md5: Option<String>,
    /// La taille, que Drive ecrit en texte ; absente pour un fichier Google Docs.
    size: Option<String>,
    #[serde(rename = "mimeType")]
    type_mime: Option<String>,
    #[serde(rename = "webViewLink")]
    lien: Option<String>,
}
impl socle::forme::ReponseDUnTiers for FichierDrive {}

impl FichierDrive {
    /// Son identifiant, que Drive donne toujours a ce qu'il rend.
    fn id(&self) -> Result<String, ErreurGoogle> {
        self.id.clone().ok_or_else(|| ErreurGoogle::from(socle::fichiers::absent("id").reclasser(Classe::Manque)))
    }

    /// Son nom, que le listage demande.
    fn nom(&self) -> Result<String, ErreurGoogle> {
        self.name.clone().ok_or_else(|| ErreurGoogle::from(socle::fichiers::absent("name").reclasser(Classe::Manque)))
    }
}

/// UN LISTAGE DE DRIVE (`files.list`) : ses fichiers (Drive ecrit la liste, vide s'il n'y en a pas), et la page suivante
/// s'il y en a une. Une reponse de Google : une cle qu'il ajoute est ignoree a la porte.
#[derive(Debug, Deserialize)]
struct Listage {
    files: Vec<FichierDrive>,
    #[serde(rename = "nextPageToken")]
    page_suivante: Option<String>,
}
impl socle::forme::ReponseDUnTiers for Listage {}

impl Listage {
    /// La page suivante, quand Drive en annonce une.
    fn suite(&self) -> Option<String> {
        self.page_suivante.clone().filter(|p| !p.is_empty())
    }
}

/// Ce qui part pour creer un dossier (`files.create`) : son nom, son type, son parent.
#[derive(Serialize)]
struct DossierACreer<'a> {
    name: &'a str,
    #[serde(rename = "mimeType")]
    type_mime: &'a str,
    parents: [&'a str; 1],
}

/// Les metadonnees d'un premier depot (`files.create`, `multipart`) : le nom et le parent.
#[derive(Serialize)]
struct FichierACreer<'a> {
    name: &'a str,
    parents: [&'a str; 1],
}

/// `google.DRIVE_FICHIERS`.
pub const DRIVE_FICHIERS: &str = "https://www.googleapis.com/drive/v3/files";
/// `google.DRIVE_TELEVERSEMENT`.
pub const DRIVE_TELEVERSEMENT: &str = "https://www.googleapis.com/upload/drive/v3/files";
/// Le type d'un dossier chez Drive.
pub const DOSSIER_DRIVE: &str = "application/vnd.google-apps.folder";

/// `urlopen(..., timeout=120)` des appels Drive.
const DELAI_APPEL: u64 = 120;

/// LES NOUVELLES TENTATIVES (un « HTTP 500 : Internal Error » passager ne doit pas remonter a la personne). Google le dit pour l'API Drive
/// (`atelier/reference/sources/google-drive-api/`, LISEZ-MOI.md) : sur 500, 502, 503, 504 et 429, « use exponential backoff to
/// retry the request » (`google-drive-api-gerer-les-erreurs-20261020.txt`, « 429 errors », « 500, 502, 503, 504 errors ») ;
/// sur un 403 de limite de debit, « retry the upload » (`google-drive-api-televerser-20261020.txt`, « Handle media upload
/// errors ») ; et l'algorithme : attendre 1 s, puis 2 s, puis 4 s, chaque fois plus un hasard d'au plus 1 000 ms, « up to some
/// maximum number of retries » (`google-drive-api-limites-et-quotas-20261020.txt`, « Example algorithm »). Trois nouvelles
/// tentatives au plus : un appel qui echoue encore apres sept secondes d'attente est rendu, et la sauvegarde le reporte au
/// passage suivant (`sauvegarde.rs`).
pub const NOUVELLES_TENTATIVES: u32 = 3;

/// L'attente avant la premiere nouvelle tentative, en millisecondes (1 s, puis le double) ; le hasard ajoute en vaut au plus
/// autant.
const ATTENTE_DE_BASE_MS: u64 = 1000;

/// Une erreur que Google dit passagere : a retenter apres une attente (voir `NOUVELLES_TENTATIVES`).
pub fn passagere(e: &ErreurGoogle) -> bool {
    // Un 403 n'est passager que s'il dit une limite de debit.
    let limite = || {
        let m = e.echec.phrase.to_lowercase();
        m.contains("rate limit") || m.contains("ratelimitexceeded")
    };
    matches!(e.statut, Some(500 | 502 | 503 | 504 | 429)) || (e.statut == Some(403) && limite())
}

/// Une erreur de l'API ou de la configuration, avec sa classe (`Google` pour ce qui est deja traduit en francais ;
/// `CopieDeDeveloppement`, `FichierAbsent`, `SortieAbsente`... pour le reste ; aucune pour un refus ordinaire). La phrase
/// est ecrite pour etre lue.
#[derive(Debug, Clone)]
pub struct ErreurGoogle {
    pub echec: Echec,
    /// Le statut HTTP d'une reponse d'erreur de Google (rien d'autre n'en porte) : de quoi decider d'une nouvelle tentative.
    pub statut: Option<u16>,
}

impl ErreurGoogle {
    #[track_caller]
    pub fn nouvelle(classe: Classe, message: impl Into<String>) -> ErreurGoogle {
        ErreurGoogle::from(socle::echec::erreur(classe, message))
    }

    /// Un refus de Google, traduit.
    #[track_caller]
    pub fn google(message: impl Into<String>) -> ErreurGoogle {
        ErreurGoogle::nouvelle(Classe::Google, message)
    }

    /// Un fichier local qui manque, ou ne se lit pas.
    #[track_caller]
    pub fn fichier(e: &std::io::Error, chemin: &Path) -> ErreurGoogle {
        ErreurGoogle::nouvelle(Classe::du_disque(e), socle::fichiers::panne_fichier(e, chemin))
    }
}

impl From<Echec> for ErreurGoogle {
    fn from(echec: Echec) -> Self {
        ErreurGoogle { echec, statut: None }
    }
}

/// Un fichier deja depose dans un dossier Drive (`Client.lister`).
#[derive(Debug, Clone, PartialEq)]
pub struct Distant {
    pub id: String,
    /// `md5Checksum`, calcule par Google ; absent pour un fichier Google Docs.
    pub md5: Option<String>,
    /// Sa taille ; absente pour un fichier Google Docs.
    pub octets: Option<u64>,
}

/// Ce que Drive rend d'un depot (`{id, name, webViewLink}`), plus `remplace`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Depose {
    pub id: Option<String>,
    pub nom: Option<String>,
    pub lien: Option<String>,
    /// Vrai quand le contenu d'un fichier de meme nom a ete remplace au lieu d'un second depot.
    pub remplace: bool,
}

/// Ce que la sauvegarde et l'archivage demandent a Drive.
pub trait Drive {
    /// `Client.compte` : l'adresse du compte autorise, lue sans reseau.
    fn compte(&self) -> Option<String>;
    /// `Client.chemin` : cree au besoin toute une suite de dossiers et rend le dernier.
    fn chemin(&mut self, noms: &[&str]) -> Result<String, ErreurGoogle>;
    /// `Client.lister` : les FICHIERS deja deposes dans un dossier, par nom ; les sous-dossiers
    /// sont ecartes.
    fn lister(&mut self, parent: &str) -> Result<BTreeMap<String, Distant>, ErreurGoogle>;
    /// `Client.televerser` : depose un fichier. `connu` : le fichier de meme nom que `lister` a trouve dans ce dossier ;
    /// s'il y en a un, son contenu est REMPLACE (`Depose::remplace`, meme identifiant, meme lien), sinon le fichier est
    /// CREE. Le client ne cherche pas lui-meme (une recherche par nom seul confondrait un dossier et un fichier, et
    /// ne tiendrait pas compte de ce que `lister` vient de dire) : la decision est celle de `lister`, la meme que
    /// celle de la comparaison des empreintes.
    /// Un fichier disparu entre-temps est un `FichierAbsent`.
    fn televerser(&mut self, fichier: &Path, parent: &str, nom: Option<&str>, connu: Option<&Distant>) -> Result<Depose, ErreurGoogle>;
    /// Les SOUS-DOSSIERS d'un dossier, `(nom, identifiant)`, dans l'ordre ou Drive les rend, homonymes compris (la
    /// restauration les refuse plutot que d'en choisir un). Ne cree rien : c'est ce qui distingue la recherche de la
    /// restauration de `chemin`, qui cree ce qui manque.
    fn sous_dossiers(&mut self, parent: &str) -> Result<Vec<(String, String)>, ErreurGoogle>;
    /// Le contenu d'un fichier, tel que Drive le sert (`alt=media`). La restauration le compare a `md5Checksum` avant
    /// d'ecrire quoi que ce soit.
    fn telecharger(&mut self, id: &str) -> Result<Vec<u8>, ErreurGoogle>;
}

/// Les trois adresses que le client appelle. Les vraies par defaut ; la preuve pointe sur un
/// serveur local (`http://127.0.0.1:port/...`).
#[derive(Debug, Clone)]
pub struct Adresses {
    pub jetons: String,
    pub fichiers: String,
    pub televersement: String,
}

impl Adresses {
    /// Les adresses de Google.
    pub fn de_google() -> Adresses {
        Adresses { jetons: JETONS.into(), fichiers: DRIVE_FICHIERS.into(), televersement: DRIVE_TELEVERSEMENT.into() }
    }

    /// Les adresses d'un Google qui repond a `base` (un serveur local).
    pub fn de_la_base(base: &str) -> Adresses {
        Adresses { jetons: format!("{}/token", base), fichiers: format!("{}/drive/v3/files", base), televersement: format!("{}/upload/drive/v3/files", base) }
    }
}

/// CE QUE LE CLIENT RECOIT de l'appelant, qui l'a lu sur le poste (le coffre, la cle du poste) : l'application, le jeton
/// de rafraichissement scelle (aucun : le compte n'est pas lie ; ou la phrase qui dit pourquoi il ne se lit pas),
/// l'adresse du compte. Ce module n'ouvre jamais le coffre.
pub struct Autorisation {
    pub application: Application,
    pub rafraichissement: Result<Option<String>, ErreurGoogle>,
    pub compte: Option<String>,
}

/// Le jeton d'acces ne vit qu'en memoire.
pub struct Client {
    application: Application,
    rafraichissement: Result<Option<String>, ErreurGoogle>,
    compte: Option<String>,
    acces: Option<String>,
    adresses: Adresses,
}

impl Client {
    /// Le client du Drive, avec ce que l'appelant a lu (`Autorisation`). Aucun appel reseau.
    pub fn nouveau(autorisation: Autorisation, adresses: Adresses) -> Client {
        Client {
            application: autorisation.application,
            rafraichissement: autorisation.rafraichissement,
            compte: autorisation.compte,
            acces: None,
            adresses,
        }
    }

    // ------------------------------------------------------------- transport

    /// `Client.jeton_acces` : le jeton d'acces, demande a Google au premier appel.
    pub fn jeton_acces(&mut self) -> Result<String, ErreurGoogle> {
        if let Some(a) = self.acces.as_ref().filter(|a| !a.is_empty()) {
            return Ok(a.clone());
        }
        let Some(rafraichissement) = self.rafraichissement.clone()?.filter(|r| !r.is_empty()) else {
            return Err(ErreurGoogle::google(
                "Compte Google pas encore lie : Configuration > Mots de passe > Lier le Drive.",
            ));
        };
        let acces = jeton_par_rafraichissement(&self.adresses.jetons, &self.application, &rafraichissement, DELAI_APPEL.min(60))??;
        self.acces = Some(acces.clone());
        Ok(acces)
    }

    /// `Client.appel` : GET, ou POST si des donnees sont fournies, ou `methode`. Rend la reponse lue en sa forme `T`.
    ///
    /// UN JETON D'ACCES NE VIT QU'UNE HEURE (3600 s) : garde pour la vie du client, une sauvegarde de plus
    /// d'une heure recevrait des 401 pour chaque fichier restant. Un 401 fait donc oublier le jeton, en demande
    /// un autre (rafraichissement) et recommence UNE fois ; un second 401 est rendu comme une erreur ordinaire.
    fn appel<T: socle::forme::ReponseDUnTiers>(&mut self, url: &str, donnees: Option<&[u8]>, content_type: Option<&str>, methode: Option<&str>) -> Result<T, ErreurGoogle> {
        let brut = self.appel_brut(url, donnees, content_type, methode)?;
        lue(brut, "Google Drive a rendu une réponse illisible.").map_err(|m| ErreurGoogle::from(m.reclasser(Classe::Google)))
    }

    /// `appel` sans lecture du JSON : les octets de la reponse, tels quels (le contenu d'un fichier, `telecharger`). Le
    /// meme 401 rattrape une fois ; une erreur passagere (5xx, 429, limite de debit) se retente jusqu'a
    /// `NOUVELLES_TENTATIVES` fois, apres 1 s, 2 s puis 4 s, plus un hasard (`passagere`, la page de Google citee la).
    pub fn appel_brut(&mut self, url: &str, donnees: Option<&[u8]>, content_type: Option<&str>, methode: Option<&str>) -> Result<Vec<u8>, ErreurGoogle> {
        let mut tentative = 0;
        loop {
            let r = match self.appel_une_fois(url, donnees, content_type, methode) {
                Err(e) if e.statut == Some(401) => {
                    self.acces = None;
                    self.appel_une_fois(url, donnees, content_type, methode)
                }
                autre => autre,
            };
            match r {
                Err(e) if passagere(&e) && tentative < NOUVELLES_TENTATIVES => {
                    std::thread::sleep(std::time::Duration::from_millis(attente(ATTENTE_DE_BASE_MS, tentative)?));
                    tentative += 1;
                }
                autre => return autre,
            }
        }
    }

    fn appel_une_fois(&mut self, url: &str, donnees: Option<&[u8]>, content_type: Option<&str>, methode: Option<&str>) -> Result<Vec<u8>, ErreurGoogle> {
        let autorisation = format!("Bearer {}", self.jeton_acces()?);
        let entetes = [("Authorization", autorisation.as_str())];
        let requete = Requete {
            url,
            jeton: None,
            donnees,
            content_type,
            delai: DELAI_APPEL,
            entetes: &entetes,
            methode,
            ..Default::default()
        };
        match envoyer(&requete) {
            Ok((brut, _)) => Ok(brut),
            Err(panne) => {
                let statut = match &panne {
                    crate::transport::Panne::Statut { code, .. } => Some(*code),
                    _ => None,
                };
                let mut e = match phrase_de_panne(panne) {
                    Ok(phrase) => ErreurGoogle::google(phrase),
                    Err(e) => ErreurGoogle::from(e),
                };
                e.statut = statut;
                Err(e)
            }
        }
    }

    // ----------------------------------------------------------------- Drive

    /// `Client.dossier` : l'identifiant du dossier `nom` sous `parent`, cree s'il n'existe pas.
    pub fn dossier(&mut self, nom: &str, parent: &str) -> Result<String, ErreurGoogle> {
        let requete = format!(
            "name = '{}' and mimeType = '{}' and '{}' in parents and trashed = false",
            echapper(nom),
            DOSSIER_DRIVE,
            parent
        );
        let url = format!(
            "{}?{}",
            self.adresses.fichiers,
            encoder(&[("q", requete), ("fields", "files(id,name)".into()), ("pageSize", "10".into())])
        );
        let trouves: Listage = self.appel(&url, None, None, None)?;
        if let Some(premier) = trouves.files.first() {
            return premier.id();
        }
        let corps = socle::json_ascii::dumps(&DossierACreer { name: nom, type_mime: DOSSIER_DRIVE, parents: [parent] }, None, false, false);
        let url = self.adresses.fichiers.clone();
        let cree: FichierDrive = self.appel(&url, Some(corps.as_bytes()), Some("application/json"), None)?;
        cree.id()
    }
}

/// L'apostrophe se protege dans la requete `q`.
///
/// La barre oblique inverse se protege AUSSI (`\\`), comme la syntaxe de `q` l'exige : sans cela, un nom
/// qui en porte une donnerait une requete que Drive refuse.
fn echapper(nom: &str) -> String {
    nom.replace('\\', "\\\\").replace('\'', "\\'")
}

/// La taille d'un fichier (`size`, que Drive ecrit en texte).
fn octets_de(t: &str) -> Result<u64, ErreurGoogle> {
    t.trim().parse::<u64>().map_err(|e| ErreurGoogle::from(socle::echec::refus_avec("Google Drive a rendu une taille illisible.", socle::echec::Lieu { detail: Some(format!("{} : {}", t, e)), ..socle::echec::Lieu::default() }).reclasser(Classe::Illisible)))
}

/// Des aleas du systeme ; un systeme qui n'en donne pas refuse.
fn aleas(o: &mut [u8]) -> Result<(), ErreurGoogle> {
    getrandom::fill(o).map_err(|e| ErreurGoogle::nouvelle(Classe::Disque, format!("Le systeme ne donne pas d'aleas : {}.", e)))
}

/// L'attente avant la nouvelle tentative `n` (0 pour la premiere) : `base * 2^n` plus un hasard d'au plus `base`
/// (« wait 1 + random_number_milliseconds », puis 2, puis 4 : `google-drive-api-limites-et-quotas-20261020.txt`).
fn attente(base: u64, n: u32) -> Result<u64, ErreurGoogle> {
    let mut o = [0u8; 2];
    aleas(&mut o)?;
    let hasard = u64::from(u16::from_le_bytes(o)) * base / u64::from(u16::MAX);
    Ok(base.saturating_mul(1 << n.min(6)) + hasard)
}

/// Seize octets tirés au hasard, en hexadécimal.
fn hasard_hex() -> Result<String, ErreurGoogle> {
    let mut o = [0u8; 16];
    aleas(&mut o)?;
    Ok(o.iter().map(|b| format!("{:02x}", b)).collect())
}

/// Le corps `multipart/related` d'un premier depot (`Client.televerser`) : les metadonnees JSON,
/// puis le contenu. Rend (corps, frontiere).
pub fn corps_multipart(nom: &str, parent: &str, type_mime: &str, contenu: &[u8], frontiere: &str) -> Vec<u8> {
    let meta = socle::json_ascii::dumps(&FichierACreer { name: nom, parents: [parent] }, None, false, false);
    let mut corps = Vec::with_capacity(contenu.len() + 512);
    corps.extend_from_slice(b"--");
    corps.extend_from_slice(frontiere.as_bytes());
    corps.extend_from_slice(b"\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n");
    corps.extend_from_slice(meta.as_bytes());
    corps.extend_from_slice(b"\r\n--");
    corps.extend_from_slice(frontiere.as_bytes());
    corps.extend_from_slice(format!("\r\nContent-Type: {}\r\n\r\n", type_mime).as_bytes());
    corps.extend_from_slice(contenu);
    corps.extend_from_slice(b"\r\n--");
    corps.extend_from_slice(frontiere.as_bytes());
    corps.extend_from_slice(b"--\r\n");
    corps
}

impl Drive for Client {
    fn compte(&self) -> Option<String> {
        self.compte.clone()
    }

    fn chemin(&mut self, noms: &[&str]) -> Result<String, ErreurGoogle> {
        let mut parent = "root".to_string();
        for nom in noms {
            parent = self.dossier(nom, &parent)?;
        }
        Ok(parent)
    }

    fn lister(&mut self, parent: &str) -> Result<BTreeMap<String, Distant>, ErreurGoogle> {
        let mut fichiers = BTreeMap::new();
        let mut page: Option<String> = None;
        loop {
            let mut champs = vec![
                ("q", format!("'{}' in parents and trashed = false", parent)),
                ("fields", "nextPageToken,files(id,name,md5Checksum,size,mimeType)".to_string()),
                ("pageSize", "200".to_string()),
            ];
            if let Some(p) = &page {
                champs.push(("pageToken", p.clone()));
            }
            let url = format!("{}?{}", self.adresses.fichiers, encoder(&champs));
            let r: Listage = self.appel(&url, None, None, None)?;
            for f in &r.files {
                if f.type_mime.as_deref() == Some(DOSSIER_DRIVE) {
                    continue;
                }
                let octets = f.size.as_deref().filter(|t| !t.is_empty()).map(octets_de).transpose()?;
                fichiers.insert(f.nom()?, Distant { id: f.id()?, md5: f.md5.clone(), octets });
            }
            page = r.suite();
            if page.is_none() {
                return Ok(fichiers);
            }
        }
    }

    fn televerser(&mut self, fichier: &Path, parent: &str, nom: Option<&str>, connu: Option<&Distant>) -> Result<Depose, ErreurGoogle> {
        let nom: String = match nom.filter(|n| !n.is_empty()) {
            Some(n) => n.to_string(),
            None => fichier
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .ok_or_else(|| ErreurGoogle::from(socle::echec::refus_avec("Un fichier à déposer n'a pas de nom.", socle::echec::Lieu { chemin: Some(fichier.to_string_lossy().into_owned()), ..socle::echec::Lieu::default() }).reclasser(Classe::Illisible)))?,
        };
        // Un type que l'extension ne dit pas : des octets (RFC 2046, `application/octet-stream`).
        let type_mime = match crate::envoi::type_devine(&nom) {
            Some(t) => t,
            None => "application/octet-stream",
        };

        if let Some(distant) = connu {
            let url = format!(
                "{}/{}?{}",
                self.adresses.televersement,
                distant.id,
                encoder(&[("uploadType", "media".into()), ("fields", "id,name,webViewLink".into())])
            );
            let contenu = std::fs::read(fichier).map_err(|e| ErreurGoogle::fichier(&e, fichier))?;
            let r: FichierDrive = self.appel(&url, Some(&contenu), Some(type_mime), Some("PATCH"))?;
            let mut depose = depose_de(r);
            depose.remplace = true;
            return Ok(depose);
        }

        let frontiere = format!("----{}", hasard_hex()?);
        let contenu = std::fs::read(fichier).map_err(|e| ErreurGoogle::fichier(&e, fichier))?;
        let corps = corps_multipart(&nom, parent, type_mime, &contenu, &frontiere);
        let url = format!(
            "{}?{}",
            self.adresses.televersement,
            encoder(&[("uploadType", "multipart".into()), ("fields", "id,name,webViewLink".into())])
        );
        let r: FichierDrive = self.appel(&url, Some(&corps), Some(&format!("multipart/related; boundary={}", frontiere)), None)?;
        Ok(depose_de(r))
    }

    fn sous_dossiers(&mut self, parent: &str) -> Result<Vec<(String, String)>, ErreurGoogle> {
        let mut dossiers = Vec::new();
        let mut page: Option<String> = None;
        loop {
            let mut champs = vec![
                ("q", format!("'{}' in parents and mimeType = '{}' and trashed = false", parent, DOSSIER_DRIVE)),
                ("fields", "nextPageToken,files(id,name)".to_string()),
                ("pageSize", "200".to_string()),
            ];
            if let Some(p) = &page {
                champs.push(("pageToken", p.clone()));
            }
            let url = format!("{}?{}", self.adresses.fichiers, encoder(&champs));
            let r: Listage = self.appel(&url, None, None, None)?;
            for f in &r.files {
                dossiers.push((f.nom()?, f.id()?));
            }
            page = r.suite();
            if page.is_none() {
                return Ok(dossiers);
            }
        }
    }

    fn telecharger(&mut self, id: &str) -> Result<Vec<u8>, ErreurGoogle> {
        // Un identifiant Drive ne porte que des lettres, des chiffres, `-` et `_` ; il s'encode quand meme, on ne
        // fabrique pas un chemin d'URL avec ce qu'un serveur a rendu.
        let url = format!("{}/{}?{}", self.adresses.fichiers, socle::html::pourcent(id, ""), encoder(&[("alt", "media".into())]));
        self.appel_brut(&url, None, None, None)
    }
}

fn depose_de(r: FichierDrive) -> Depose {
    Depose { id: r.id, nom: r.name, lien: r.lien, remplace: false }
}
