//! La sauvegarde du dossier prive dans le Drive, par differences : ce qu'elle sauve, l'inventaire, l'index, ce qui
//! persiste. Le passage lui-meme (`sauvegarder`, le client Drive du poste) est `gestes::sauvegarde`.
//!
//! Ce sont des FONCTIONS, pas des routes : leurs appelants sont l'ouverture et la veille, la ligne de commande, la
//! restauration, et l'interface qui lit `derniere` et `rien_n_a_bouge`.
//!
//! OU ELLE VA : UN DOSSIER PAR COMPAGNIE, sous la racine du Drive (`PuppetPaie/`) : nomme comme la compagnie, et marque
//! d'une identite qui ne depend pas du nom (`IdentiteSurLeDrive`, posee en `appProperties` du dossier et gardee dans la
//! compagnie, `config/drive.json`, que la sauvegarde envoie). La sauvegarde retrouve son dossier par cette identite,
//! jamais par le nom ; deux compagnies d'un meme compte Google ne se melent plus. Dedans : `sauvegarde/` (l'arbre de la
//! compagnie) et `archives/<mois>` (`routes::gestes::archivage`). L'ancien dossier commun (`PuppetPaie/sauvegarde/`, ou
//! toutes les compagnies du compte ecrivaient) ne se recopie pas : la premiere sauvegarde remplit le dossier neuf.
//!
//! CE QU'ELLE SAUVE : tout le dossier des donnees (`config/`, `engagements/`, `sorties/`, `retours/`,
//! `gabarits/`, `journal/`, `reference/`), coffre compris (`coffre.bin`, scelle par le mot de passe du logiciel, qui
//! l'ouvre sur un poste neuf apres une restauration), sauf ce qui est propre au poste (`chemins_exclus`, par leur chemin : le
//! jeton Google, que la cle du poste scelle ; l'identite du poste chez l'editeur ; les etats et caches de la machine ; le
//! journal de la fenetre et celui des incidents du poste ; la marque d'une reprise pas finie) et le rebut.
//!
//! ELLE NE SUPPRIME JAMAIS RIEN dans le Drive. Un fichier efface ici reste la-bas ; seul le geste « Supprimer » d'un
//! dossier (`routes::drive`) met un dossier a la corbeille.
//!
//! COMMENT ELLE SAIT CE QUI A CHANGE : par l'empreinte MD5 que Google publie
//! (`md5Checksum`). L'index local (`config/.sauvegarde.json`) ne sert qu'a repondre SANS RESEAU
//! (`rien_n_a_bouge`) et a garder les identifiants des dossiers Drive ; il n'est jamais cru pour
//! DECIDER d'un televersement.

use std::collections::BTreeMap;
use socle::echec::Classe;
use socle::fichiers::CheminRelatif;
use socle::grandeurs::Octets;
use socle::paris::Moment;
use std::path::{Path, PathBuf};


use envois::drive::{Drive, ErreurGoogle, IdDrive, MarqueDuDossier};

pub const RACINE_DRIVE: &str = "PuppetPaie";
/// Sous le dossier d'une compagnie, l'arbre de sa sauvegarde ; c'etait aussi le nom de l'ancien dossier commun, sous la
/// racine.
pub const SAUVEGARDE: &str = "sauvegarde";
/// Sous le dossier d'une compagnie, les mois archives (`routes::gestes::archivage`).
pub const ARCHIVES: &str = "archives";
/// L'index local de la sauvegarde, sous la compagnie (`chemin_index`).
pub const INDEX: CheminRelatif = CheminRelatif::du_moteur("config/.sauvegarde.json");
/// L'identite de la compagnie sur le Drive (`IdentiteSurLeDrive`), sous la compagnie : elle part avec la sauvegarde, et
/// revient avec une restauration.
pub const FICHIER_DU_DRIVE: CheminRelatif = CheminRelatif::du_moteur("config/drive.json");
/// Ce qui commence l'identite d'une compagnie sur le Drive (`IdentiteSurLeDrive`).
const PREFIXE_IDENTITE: &str = "cie:";

/// CE QUI NE MONTE PAS, PARCE QU'IL EST PROPRE AU POSTE, par son chemin depuis la racine de la compagnie
/// (`chemins_exclus` : un fichier du meme nom ailleurs, sous `import/` par exemple, monte comme les autres) :
/// - le jeton Google, scelle par la cle de ce poste (`socle::fichiers::JETON_GOOGLE`) ;
/// - l'identite du poste chez l'editeur, son numero et sa cle (`incidents::DIAGNOSTIC`) ;
/// - le journal des incidents de ce poste, dont chaque ligne porte la machine qui l'a ecrite (`incidents::INCIDENTS`) ;
/// - le journal de la fenetre de ce poste (`installation::JOURNAL_INTERFACE`) ;
/// - la marque d'une reprise du Drive pas finie sur ce poste (`restauration::REPRISE_EN_COURS`) ;
/// - les etats de la machine : l'index de la sauvegarde, la trace et l'etape de l'ouverture, l'etat garde de l'ouverture
///   (ses chemins absolus), le tableau garde de l'administrateur de l'installation, les verrous.
fn chemins_exclus() -> [CheminRelatif; 11] {
    use socle::fichiers::{ETAPE_DE_L_OUVERTURE, ETAT_DE_L_OUVERTURE, JETON_GOOGLE, TABLEAU_GARDE, TRACE_DE_L_OUVERTURE};
    [
        JETON_GOOGLE,
        crate::incidents::DIAGNOSTIC,
        crate::incidents::INCIDENTS,
        crate::installation::JOURNAL_INTERFACE,
        crate::restauration::REPRISE_EN_COURS,
        socle::verrou::COMPTEURS,
        INDEX,
        TRACE_DE_L_OUVERTURE,
        ETAPE_DE_L_OUVERTURE,
        CheminRelatif::from(ETAT_DE_L_OUVERTURE),
        TABLEAU_GARDE,
    ]
}
const SUFFIXES_EXCLUS: [&str; 5] = [".pyc", ".tmp", ".lock", ".part", ".verrou"];
const DOSSIERS_EXCLUS: [&str; 2] = ["__pycache__", ".git"];

/// Les signes d'une absence de reseau, SANS `timed out` : un delai de LECTURE depasse (un serveur qui repond trop
/// lentement, un dossier de Drive qui ne repond pas) n'arrete pas TOUTE la sauvegarde (« Hors connexion : sauvegarde au
/// prochain passage ») quand le reseau est la. Un delai depasse est un incident du dossier ou du fichier ; l'absence de reseau reste « injoignable »,
/// `getaddrinfo`, `Network is unreachable`, `Name or service not known`... (un delai de CONNEXION dit « Reseau injoignable :
/// timed out » : il garde son « injoignable »).
const SIGNES_HORS_LIGNE: [&str; 10] = [
    "injoignable",
    "getaddrinfo",
    "errno 11001",
    "errno 11004",
    "winerror 10051",
    "winerror 10060",
    "winerror 10065",
    "network is unreachable",
    "name or service not known",
    "temporary failure in name resolution",
];

/// `hors_ligne` : ce message dit-il que le reseau manque, plutot qu'un vrai refus ?
pub fn hors_ligne(erreur: &str) -> bool {
    let texte = erreur.to_lowercase();
    SIGNES_HORS_LIGNE.iter().any(|s| texte.contains(s))
}

/// L'etat d'un fichier a l'envoi (`Fichier.etat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatFichier {
    Neuf,
    Modifie,
}

impl EtatFichier {
    pub fn texte(self) -> &'static str {
        match self {
            EtatFichier::Neuf => "neuf",
            EtatFichier::Modifie => "modifie",
        }
    }
}

/// Un fichier depose (`Fichier`).
#[derive(Debug, Clone, PartialEq)]
pub struct Fichier {
    pub relatif: CheminRelatif,
    pub octets: Octets,
    pub etat: EtatFichier,
}

/// Le compte rendu d'un passage (`Sauvegarde`).
#[derive(Debug, Clone, PartialEq)]
pub struct Sauvegarde {
    pub racine_drive: String,
    pub compte: Option<String>,
    pub deposes: Vec<Fichier>,
    pub identiques: usize,
    pub octets_deposes: Octets,
    pub exclus: usize,
    /// Purges entre l'inventaire et l'envoi : pas un incident.
    pub disparus: usize,
    /// Le detail brut de ce qui n'est pas passe (`dossier : erreur`), pour la ligne de commande. La fenetre ne le montre
    /// pas : un dossier reporte au passage suivant n'est pas un incident.
    pub incidents: Vec<String>,
    /// Ce qui n'a pas pu monter a ce passage, reporte au suivant.
    pub reportes: Vec<Reporte>,
    /// La phrase de ce qui PERSISTE (`souci`), apres ce passage : `None` tant que ce n'est que passager.
    pub persistant: Option<String>,
    /// Pas de reseau : arretee au premier dossier.
    pub hors_ligne: bool,
}

/// Un dossier (tout ce qu'il porte) ou un fichier qui n'a pas pu monter a un passage : il reste hors de l'index, et le
/// passage suivant le reprend.
#[derive(Debug, Clone, PartialEq)]
pub struct Reporte {
    /// Le chemin relatif du dossier (`.` pour la racine) ou du fichier.
    pub quoi: String,
    pub dossier: bool,
    /// La cause, en clair (`cause_de`).
    pub cause: String,
}

impl Reporte {
    /// La cle de l'index (`echecs`) : un dossier finit par `/`.
    pub fn cle(&self) -> String {
        if self.dossier {
            format!("{}/", self.quoi)
        } else {
            self.quoi.clone()
        }
    }
}

/// Les causes, en clair. La phrase de chaque echec reste dans `incidents`, pour la ligne de commande ; le detail
/// technique d'un echec qui n'est pas une panne de Google part chez l'editeur, au premier passage ou il apparait
/// (`routes::gestes::sauvegarde`).
pub const GOOGLE_INDISPONIBLE: &str = "Google indisponible";
const GOOGLE_MUET: &str = "Google ne répond pas";
const GOOGLE_REFUSE: &str = "Google refuse l'envoi";
/// Le Drive plein dit son remede dans la phrase de ce qui persiste : reessayer n'y suffit pas.
const DRIVE_PLEIN: &str = "le Drive du compte Google lié est plein : y faire de la place";
const FICHIER_ILLISIBLE: &str = "fichier illisible sur ce poste";
/// Deux fichiers du meme nom dans un dossier du Drive (`FichiersDuDossier::le_seul`) : le remede est dans le Drive.
const HOMONYMES: &str = "deux fichiers de ce nom dans le Drive : n'en garder qu'un";

/// La cause d'une erreur, en clair : passagere chez Google (`drive::passagere`, deja retentee par le client), un delai
/// depasse, le Drive plein (`RaisonDeGoogle::DrivePlein`), deux fichiers du meme nom dans le Drive, un refus de Google,
/// ou un fichier du poste qui ne se lit pas.
pub fn cause_de(e: &ErreurGoogle) -> &'static str {
    if envois::drive::passagere(e) {
        GOOGLE_INDISPONIBLE
    } else if e.echec.classe == Some(Classe::DelaiDepasse) || e.echec.phrase.to_lowercase().contains("timed out") {
        GOOGLE_MUET
    } else if e.raison == Some(envois::drive::RaisonDeGoogle::DrivePlein) {
        DRIVE_PLEIN
    } else if e.echec.classe == Some(Classe::SauvegardeAmbigue) {
        HOMONYMES
    } else if e.echec.classe == Some(Classe::Disque) {
        FICHIER_ILLISIBLE
    } else {
        GOOGLE_REFUSE
    }
}

impl Sauvegarde {
    /// La ligne de la sauvegarde. Ce qui est reporte au prochain passage s'y dit en passant, sans alarme : c'est
    /// `persistant` (la trace, `sauvegarde_souci`) qui dit ce qui dure.
    pub fn resume(&self) -> String {
        if self.hors_ligne {
            return "Hors connexion : sauvegarde au prochain passage.".into();
        }
        let reporte = if self.reportes.is_empty() { String::new() } else { format!(" ; {} au prochain passage", combien(&self.reportes, "reporte", "reportes")) };
        if self.deposes.is_empty() {
            if reporte.is_empty() {
                return format!("Sauvegarde a jour ({} fichiers).", self.identiques);
            }
            return format!("Sauvegarde : {} fichier(s) inchange(s){}.", self.identiques, reporte);
        }
        format!("Sauvegarde : {} fichier(s) envoye(s), {}, {} inchange(s){}.", self.deposes.len(), self.octets_deposes, self.identiques, reporte)
    }
}

/// « 1 dossier reporte », « 2 fichiers reportes », « 1 dossier et 2 fichiers reportes ».
fn combien(reportes: &[Reporte], un: &str, plusieurs: &str) -> String {
    let (nd, nf) = (reportes.iter().filter(|r| r.dossier).count(), reportes.iter().filter(|r| !r.dossier).count());
    let nom = |n: usize, quoi: &str| format!("{} {}{}", n, quoi, if n > 1 { "s" } else { "" });
    let (texte, pluriel) = match (nd, nf) {
        (0, f) => (nom(f, "fichier"), f > 1),
        (d, 0) => (nom(d, "dossier"), d > 1),
        (d, f) => (format!("{} et {}", nom(d, "dossier"), nom(f, "fichier")), true),
    };
    format!("{} {}", texte, if pluriel { plusieurs } else { un })
}

// ---------------------------------------------------------------- ce qui persiste

/// CE QUI PERSISTE, ET SEULEMENT CELA. Un dossier reporte au prochain passage n'est pas un
/// incident a montrer : Google rend parfois un 500 que le passage suivant ne voit plus. Ce qui se dit, c'est ce qui dure :
/// le meme dossier (ou fichier) en echec sur `PASSAGES_QUI_DURENT` passages de suite (une heure, a vingt minutes d'une veille a
/// l'autre), ou depuis `UNE_JOURNEE`, ou une journee entiere sans sauvegarde complete.
pub const PASSAGES_QUI_DURENT: usize = 3;
pub const UNE_JOURNEE: std::time::Duration = std::time::Duration::from_secs(86_400);

/// « un dossier n'a pas pu être envoyé », « 2 fichiers n'ont pas pu être envoyés ».
fn ce_qui_manque(nd: usize, nf: usize) -> String {
    let nom = |n: usize, quoi: &str| if n == 1 { format!("un {}", quoi) } else { format!("{} {}s", n, quoi) };
    let (texte, pluriel) = match (nd, nf) {
        (0, f) => (nom(f, "fichier"), f > 1),
        (d, 0) => (nom(d, "dossier"), d > 1),
        (d, f) => (format!("{} et {}", nom(d, "dossier"), nom(f, "fichier")), true),
    };
    if pluriel {
        format!("{} n'ont pas pu être envoyés", texte)
    } else {
        format!("{} n'a pas pu être envoyé", texte)
    }
}

/// « depuis 09:51 » le jour meme, « depuis hier », « depuis le 18/10 ».
fn depuis_quand(depuis: Moment, maintenant: Moment) -> String {
    let (jour, aujourdhui) = (depuis.jour_de_paris(), maintenant.jour_de_paris());
    // Les jours ecoules d'un jour qui n'est pas apres aujourd'hui.
    let ecoules = (jour <= aujourdhui).then(|| socle::date::Duree::ecoulee(&jour, &aujourdhui));
    match ecoules {
        Some(d) if d == socle::date::Duree::jours(0) => format!("depuis {}", depuis.heure_de_paris()),
        Some(d) if d == socle::date::Duree::jours(1) => "depuis hier".to_string(),
        Some(_) | None => format!("depuis le {}", jour.jj_mm()),
    }
}

/// `maintenant` est-il une journee au moins apres `depuis` ?
fn une_journee_apres(maintenant: Moment, depuis: Moment) -> bool {
    maintenant.depuis(depuis).is_some_and(|d| d >= UNE_JOURNEE)
}

/// La phrase de ce qui persiste dans l'index (`echecs`, `derniere_complete`), a l'instant `maintenant`, ou `None`.
pub fn phrase_persistante(index: &Index, maintenant: Moment) -> Result<Option<String>, ErreurGoogle> {
    let Some(echecs) = index.echecs.as_ref().filter(|e| !e.is_empty()) else { return Ok(None) };
    // (cle, depuis, cause) de chaque echec, et ceux qui durent.
    let mut tous: Vec<(&String, Moment, &str, bool)> = Vec::new();
    for (cle, e) in echecs {
        let depuis = e.depuis;
        tous.push((cle, depuis, e.cause.as_str(), e.passages >= PASSAGES_QUI_DURENT || une_journee_apres(maintenant, depuis)));
    }
    let complete = index.derniere_complete;
    let durent: Vec<_> = tous.iter().filter(|t| t.3).collect();
    let (retenus, par_l_item): (Vec<_>, bool) = if !durent.is_empty() {
        (durent, true)
    } else {
        match complete.filter(|c| une_journee_apres(maintenant, *c)) {
            Some(_) => (tous.iter().collect(), false),
            None => return Ok(None),
        }
    };
    let nd = retenus.iter().filter(|t| t.0.ends_with('/')).count();
    let nf = retenus.len() - nd;
    // Le plus ancien : sa cause, et depuis quand (le plus ancien qui dure, ou la derniere sauvegarde complete).
    let Some(ancien) = retenus.iter().min_by_key(|t| t.1) else { return Ok(None) };
    let depuis = match (par_l_item, complete) {
        (false, Some(c)) => c,
        _ => ancien.1,
    };
    let quand = depuis_quand(depuis, maintenant);
    Ok(Some(if par_l_item {
        format!("Sauvegarde : {} {} ({}) ; le logiciel réessaie.", ce_qui_manque(nd, nf), quand, ancien.2)
    } else {
        format!("Sauvegarde : {} ({}), pas de sauvegarde complète {} ; le logiciel réessaie.", ce_qui_manque(nd, nf), ancien.2, quand)
    }))
}

/// `souci` : ce qui persiste de la sauvegarde, dit en clair (« Sauvegarde : un dossier n'a pas pu être envoyé depuis hier
/// (Google indisponible) ; le logiciel réessaie. »), ou `None` si tout est passe, ou si ce n'est que passager, ou si
/// aucune sauvegarde n'est partie. SANS RESEAU : lu dans l'index. La trace de l'ouverture le porte (`sauvegarde_souci`),
/// la fenetre le montre a la place de la sauvegarde.
pub fn souci(racine: &Path) -> Result<Option<String>, ErreurGoogle> {
    match lire_index(racine)? {
        Some(index) => phrase_persistante(&index, socle::temps::secondes()),
        None => Ok(None),
    }
}

// ---------------------------------------------------------------- l'inventaire

/// La taille et la date de modification (a la seconde) d'un fichier (`inventaire`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Etat {
    pub octets: Octets,
    pub mtime: Moment,
}

/// Un chemin exclu (depuis la racine de la compagnie), un suffixe exclu, ou un dossier exclu n'importe ou dans le chemin
/// relatif (le nom du fichier compris). La restauration s'en sert aussi : elle ne redescend que ce que la sauvegarde
/// envoie.
pub(crate) fn ecarte(relatif: &CheminRelatif) -> bool {
    let nom = relatif.nom();
    chemins_exclus().contains(relatif) || SUFFIXES_EXCLUS.iter().any(|s| nom.texte().ends_with(s)) || relatif.texte().split('/').any(|n| DOSSIERS_EXCLUS.contains(&n))
}

/// Les entrees (fichiers et liens, pas les dossiers) sous `dossier`, relatives a `racine`. Un dossier lien n'est pas
/// suivi. Un dossier qui disparait pendant le parcours (`engagements/avant/` ne garde que les dernieres copies) n'a plus
/// rien a sauver ; un dossier qui ne se lit pas arrete l'inventaire.
fn parcourir(racine: &Path, dossier: &Path, sortie: &mut Vec<PathBuf>) -> Result<(), ErreurGoogle> {
    let entrees = match std::fs::read_dir(dossier) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(ErreurGoogle::fichier(&e, dossier)),
    };
    for e in entrees {
        let e = e.map_err(|x| ErreurGoogle::fichier(&x, dossier))?;
        let chemin = e.path();
        let genre = match e.file_type() {
            Ok(t) => t,
            Err(x) if x.kind() == std::io::ErrorKind::NotFound => continue,
            Err(x) => return Err(ErreurGoogle::fichier(&x, &chemin)),
        };
        if genre.is_dir() {
            parcourir(racine, &chemin, sortie)?;
        } else if let Some(rel) = socle::fichiers::sous(&chemin, racine) {
            sortie.push(rel.to_path_buf());
        }
    }
    Ok(())
}

/// `inventaire` : les fichiers triés par chemin (composante par
/// composante, pas en texte : `a/x` passe avant `a b/x`), et le nombre d'ecartes. Ne lit aucun
/// contenu. Un fichier qui disparait entre le parcours et sa lecture n'est plus a sauver.
pub fn inventaire(racine: &Path) -> Result<(Vec<(CheminRelatif, Etat)>, usize), ErreurGoogle> {
    let mut chemins = Vec::new();
    parcourir(racine, racine, &mut chemins)?;
    chemins.sort();
    let (mut trouves, mut ecartes) = (Vec::new(), 0);
    for rel in chemins {
        let complet = racine.join(&rel);
        if !complet.is_file() {
            continue;
        }
        let relatif = CheminRelatif::du(&rel)?;
        if ecarte(&relatif) {
            ecartes += 1;
            continue;
        }
        let m = match std::fs::metadata(&complet) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(ErreurGoogle::fichier(&e, &complet)),
        };
        // La date du fichier dit s'il a change depuis son dernier envoi : datee hors du calendrier, elle ne le dit pas, et
        // la sauvegarde s'arrete en le nommant plutot que de l'envoyer a l'aveugle.
        let Some(date) = socle::paris::mtime(&m) else {
            return Err(ErreurGoogle::nouvelle(
                Classe::Disque,
                format!("{} est daté hors du calendrier (années 1 à 9999) : régler la date du fichier.", socle::date::repr_texte(&complet.display().to_string())),
            ));
        };
        trouves.push((relatif, Etat { octets: Octets::du_disque(&m), mtime: date.a_la_seconde() }));
    }
    Ok((trouves, ecartes))
}

// --------------------------------------------------------------------- l'index

/// LE CHEMIN D'UN DOSSIER DE LA SAUVEGARDE DANS LE DRIVE, sous le dossier de la compagnie : `sauvegarde`, puis le
/// dossier de la compagnie qu'il tient (aucun pour sa racine). L'index y garde l'identifiant de chaque dossier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct DossierDuDrive(String);

impl DossierDuDrive {
    /// Le dossier du Drive qui tient `dossier` (aucun : la racine de la compagnie).
    pub fn de(dossier: Option<&CheminRelatif>) -> DossierDuDrive {
        DossierDuDrive(DossierDuDrive::segments(dossier).join("/"))
    }

    /// Ses dossiers, du dossier de la compagnie au sien, un par un : ce que le Drive parcourt.
    pub fn segments(dossier: Option<&CheminRelatif>) -> Vec<&str> {
        let mut segments = vec![SAUVEGARDE];
        if let Some(d) = dossier {
            segments.extend(d.texte().split('/'));
        }
        segments
    }
}

impl std::fmt::Display for DossierDuDrive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// `chemin_index` : `config/`, et pas `journal/` : de l'etat machine, pas un document. Il est
/// ecarte de la sauvegarde (`chemins_exclus`).
pub fn chemin_index(racine: &Path) -> PathBuf {
    INDEX.dans(racine)
}

/// L'INDEX DE LA SAUVEGARDE (`config/.sauvegarde.json`) : quand le dernier passage s'est fait, vers quelle racine du
/// Drive, le dossier de la compagnie (`compagnie`) et les identifiants de ses dossiers (par chemin `sauvegarde/…`),
/// l'etat de chaque fichier passe ; et, s'il reste un echec, la derniere sauvegarde complete et ce qui reste en echec
/// (`souci`). L'index d'une version qui sauvegardait dans le dossier commun n'a pas `compagnie` : ses dossiers sont ceux
/// du dossier commun, que la sauvegarde ne reprend pas (`dossiers_connus`), et rien n'est a jour (`rien_n_a_bouge`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    #[serde(with = "socle::paris::ecrit::saisie")]
    pub quand: socle::paris::Moment,
    pub racine_drive: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compagnie: Option<IdDrive>,
    pub dossiers: BTreeMap<DossierDuDrive, IdDrive>,
    pub fichiers: BTreeMap<CheminRelatif, Etat>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "socle::paris::ecrit::saisie::option")]
    pub derniere_complete: Option<socle::paris::Moment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echecs: Option<BTreeMap<String, EchecIndexe>>,
}

/// Ce qui reste en echec d'un passage a l'autre : depuis quand, combien de passages, la cause en clair (`cause_de`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EchecIndexe {
    #[serde(with = "socle::paris::ecrit::saisie")]
    pub depuis: socle::paris::Moment,
    pub passages: usize,
    pub cause: String,
}


/// L'index, ou `None` si aucune sauvegarde ne l'a encore ecrit. Un index qui ne se lit pas se refuse.
pub fn lire_index(racine: &Path) -> Result<Option<Index>, ErreurGoogle> {
    let p = chemin_index(racine);
    if !p.exists() {
        return Ok(None);
    }
    socle::forme::lire_fichier(&p, "L'index de la sauvegarde ne se lit pas : le mettre de côté, la sauvegarde suivante le refait.").map(Some).map_err(|m| ErreurGoogle::from(m.reclasser(Classe::Illisible)))
}

pub fn ecrire_index(racine: &Path, index: &Index) -> Result<(), ErreurGoogle> {
    socle::ecriture::ecrire_json(&chemin_index(racine), index, 1).map_err(|e| ErreurGoogle::from(e.reclasser(Classe::Disque)))
}

/// `rien_n_a_bouge` : le disque est-il identique au dernier passage ? SANS RESEAU. Taille et date
/// suffisent : on ne decide pas d'un televersement, seulement s'il vaut la peine d'aller voir. Au
/// moindre doute (index absent, fichier apparu ou disparu), la reponse est non.
///
/// `racine_drive` compte : un index ecrit pour une autre racine
/// du Drive (`--racine-drive`) laisserait la veille croire que tout est sauvegarde la ou on l'attend. Un index d'une autre
/// racine (ou qui n'en dit aucune) donne non.
pub fn rien_n_a_bouge(racine: &Path, racine_drive: &str) -> Result<bool, ErreurGoogle> {
    // Sans index, rien ne dit ce qui est sauvegarde.
    Ok(lire_index(racine)?.map(|index| comme_l_index(racine, racine_drive, index)).transpose()?.is_some_and(|oui| oui))
}

fn comme_l_index(racine: &Path, racine_drive: &str, index: Index) -> Result<bool, ErreurGoogle> {
    if index.racine_drive != racine_drive || index.compagnie.is_none() || index.fichiers.is_empty() {
        return Ok(false);
    }
    let connus = &index.fichiers;
    let (presents, _) = inventaire(racine)?;
    if presents.len() != connus.len() {
        return Ok(false);
    }
    Ok(presents.iter().all(|(relatif, etat)| connus.get(relatif) == Some(etat)))
}

/// Ce que dit l'index du dernier passage (`derniere`) : quand, combien, vers quelle racine du Drive. Pour l'affichage.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Derniere {
    #[serde(with = "socle::paris::ecrit::saisie")]
    pub quand: socle::paris::Moment,
    pub fichiers: usize,
    pub racine_drive: String,
}

/// Le dernier passage, ou `None` si aucune sauvegarde n'est encore partie.
pub fn derniere(racine: &Path) -> Result<Option<Derniere>, ErreurGoogle> {
    Ok(lire_index(racine)?.map(|i| Derniere { fichiers: i.fichiers.len(), quand: i.quand, racine_drive: i.racine_drive }))
}

// ------------------------------------------------------------ le dossier de la compagnie

/// L'IDENTITE D'UNE COMPAGNIE SUR LE DRIVE : ce qui marque son dossier (`appProperties`, `MarqueDuDossier::compagnie`)
/// et qui ne depend pas de son nom : un identifiant tire au hasard a la premiere sauvegarde (`cie:<32 caracteres
/// hexadecimaux>`), jamais une donnee de la compagnie (deux copies d'une meme compagnie, deux reprises du meme dossier
/// partageraient un SIRET) ; une fois ecrite (`config/drive.json`), elle ne change plus, et une compagnie reprise du Drive
/// garde celle de son dossier. Elle s'ecrit telle quelle ; lue, elle refuse une autre forme.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IdentiteSurLeDrive(String);

impl IdentiteSurLeDrive {
    /// L'identite `texte`, ou la phrase qui la refuse.
    pub fn lire(texte: &str) -> socle::echec::R<IdentiteSurLeDrive> {
        if !texte.strip_prefix(PREFIXE_IDENTITE).is_some_and(|c| socle::fichiers::est_hexa(c, 32)) {
            return Err(socle::echec::refus_avec(
                "Ce n'est pas la marque qu'écrit PuppetPaie : le dossier a peut-être été changé à la main, ou par une autre application.",
                socle::echec::Lieu { detail: Some(format!("{} : attendu cie:<32 caracteres hexadecimaux>", socle::fichiers::repr(texte))), ..socle::echec::Lieu::default() },
            ));
        }
        Ok(IdentiteSurLeDrive(texte.to_string()))
    }

    /// Une identite neuve, tiree au hasard.
    fn tiree() -> socle::echec::R<IdentiteSurLeDrive> {
        let mut o = [0u8; 16];
        socle::temps::aleas(&mut o)?;
        IdentiteSurLeDrive::lire(&format!("{}{}", PREFIXE_IDENTITE, socle::fichiers::hexa(&o)))
    }

    /// Elle telle qu'ecrite.
    pub fn texte(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for IdentiteSurLeDrive {
    type Error = socle::echec::Echec;
    fn try_from(s: String) -> socle::echec::R<IdentiteSurLeDrive> {
        IdentiteSurLeDrive::lire(&s)
    }
}

impl From<IdentiteSurLeDrive> for String {
    fn from(i: IdentiteSurLeDrive) -> String {
        i.0
    }
}

/// CE QUE `config/drive.json` PORTE : l'identite de la compagnie sur le Drive, et ce qu'elle est.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FicheDuDrive {
    identite: IdentiteSurLeDrive,
    quoi: String,
}

/// L'IDENTITE DE LA COMPAGNIE `racine` SUR LE DRIVE : celle que `config/drive.json` garde ; a la premiere sauvegarde, un
/// identifiant tire au hasard, ecrit alors dans `config/drive.json`. Elle ne lit rien d'autre de la compagnie : une
/// fiche abimee n'arrete pas sa sauvegarde. Un `drive.json` qui ne se lit pas se refuse, et rien ne s'ecrit par-dessus.
pub fn identite(racine: &Path) -> socle::echec::R<IdentiteSurLeDrive> {
    if let Some(i) = identite_ecrite(racine)? {
        return Ok(i);
    }
    let p = FICHIER_DU_DRIVE.dans(racine);
    let identite = IdentiteSurLeDrive::tiree()?;
    let fiche = FicheDuDrive {
        identite: identite.clone(),
        quoi: "L'identité de la compagnie sur le Drive : elle marque son dossier, que la sauvegarde retrouve par elle et non par son nom. Elle ne change plus.".to_string(),
    };
    socle::ecriture::ecrire_json(&p, &fiche, 2)?;
    Ok(identite)
}

/// L'identite que `config/drive.json` garde, sans rien ecrire ; `None` : aucune sauvegarde ne l'a encore posee.
pub fn identite_ecrite(racine: &Path) -> socle::echec::R<Option<IdentiteSurLeDrive>> {
    let p = FICHIER_DU_DRIVE.dans(racine);
    if !p.is_file() {
        return Ok(None);
    }
    let f: FicheDuDrive = socle::forme::lire_fichier(&p, "L'identité de la compagnie sur le Drive ne se lit pas : son fichier a été changé à la main. Le remettre tel qu'il était, ou le mettre de côté : la sauvegarde suivante écrira dans un nouveau dossier.")?;
    Ok(Some(f.identite))
}

/// LA MARQUE DU DOSSIER DE LA COMPAGNIE `identite`, apres une sauvegarde qui a envoye quelque chose a `quand` : ce que la
/// compagnie sauvegarde pese alors (`octets`).
pub fn marque(identite: &IdentiteSurLeDrive, quand: socle::paris::Moment, octets: Octets) -> MarqueDuDossier {
    MarqueDuDossier {
        compagnie: Some(identite.texte().to_string()),
        derniere: Some(socle::paris::ecrit::minute::texte(quand)),
        octets: Some(socle::ecriture::json_en_ligne(&octets)),
    }
}

/// LE DOSSIER DE LA COMPAGNIE `identite` DANS LE DRIVE, sous la racine `racine_drive` : `connu` (celui que l'index garde)
/// s'il est encore en place ; sinon celui que sa marque designe sous la racine (le premier, s'il y en a deux : la vue du
/// Drive les montre tous) ; sinon un dossier neuf, nomme `nom` et marque. Un dossier mis a la corbeille n'est plus en
/// place : la sauvegarde suivante renvoie tout dans un dossier neuf.
pub fn dossier_de_la_compagnie(client: &mut dyn Drive, racine_drive: &str, identite: &IdentiteSurLeDrive, nom: &str, connu: Option<&IdDrive>) -> Result<IdDrive, ErreurGoogle> {
    if let Some(id) = connu {
        if client.en_place(id)? {
            return Ok(id.clone());
        }
    }
    let racine = client.chemin(&IdDrive::RACINE, &[racine_drive])?;
    if let Some(d) = client.sous_dossiers(&racine)?.into_iter().find(|d| d.marque.as_ref().and_then(|m| m.compagnie.as_deref()) == Some(identite.texte())) {
        return Ok(d.id);
    }
    let marque = MarqueDuDossier { compagnie: Some(identite.texte().to_string()), derniere: None, octets: None };
    client.creer_dossier(nom, &racine, &marque)
}

/// LES DOSSIERS QUE L'INDEX CONNAIT DANS LE DOSSIER `compagnie` : aucun quand l'index est celui d'un autre dossier (le
/// dossier commun d'avant, un dossier mis a la corbeille) ; ils se cherchent alors dans le Drive.
pub fn dossiers_connus(index: Option<&Index>, compagnie: &IdDrive) -> BTreeMap<DossierDuDrive, IdDrive> {
    match index {
        Some(i) if i.compagnie.as_ref() == Some(compagnie) => i.dossiers.clone(),
        Some(_) | None => BTreeMap::new(),
    }
}
