//! La mise a jour d'une installation : verifier, appliquer, revenir. La publication, par le flux « Publier » de GitHub,
//! est dans `moteur::publication`.
//!
//! UNE MISE A JOUR PAR INSTALLATION. Il n'y a qu'un logiciel par installation, et c'est lui qui se met a jour, pas une
//! compagnie : tout se passe dans `<installation>/maj/` (a cote de `logiciel/`, que la bascule ne touche pas), sans
//! qu'aucune compagnie soit ouverte. Toute installation recoit. Le bridage n'y joue aucun
//! role.
//!
//!   verifier   au lancement, en fond (`chercher_en_fond` ; lance sans reseau, il reprend de lui-meme). Lit
//!              `derniere.json` au depot ; une version plus recente est
//!              prise, son condensat et la SIGNATURE de l'editeur (Ed25519, `cles_de_l_editeur`) verifies, deballee dans
//!              `maj/<version>/`, CONTROLEE (son programme lit la configuration de chaque compagnie de l'installation),
//!              puis `maj/prete.json` est pose.
//!   appliquer  au demarrage suivant. Ce qui est dans `logiciel/` va dans `logiciel/.avant/`, la version prete prend sa
//!              place ; si ca casse en route, tout revient.
//!
//! UNE ARCHIVE PAR SYSTEME, ET UNE TOUT-SYSTEME. `logiciel/` porte les programmes des trois systemes : `puppetpaie.exe` et
//! `WebView2Loader.dll` a la racine (Windows, le seul suivi par Git), `linux/puppetpaie` et `linux/moteur`,
//! `mac/puppetpaie` et `mac/moteur` (poses par le flux « Publier », universels Intel et Apple pour le Mac) : Linux et Mac
//! ont tous deux un `puppetpaie`, chacun son dossier (`dossier_programmes`). Une version a une archive par systeme (les
//! fichiers communs et ceux de ce systeme, `systeme_du_fichier`) et une qui porte tout (`CleDArchive::DeTous` : un dossier
//! portable pour plusieurs machines, et le repli), chacune signee ; `derniere.json` les nomme sous `archives`. Une
//! installation prend celle de chacun de ses systemes (`systemes_poses` : le sien, et ceux d'un dossier portable), ou la
//! tout-systeme si elle les porte tous (`telecharger`). Seules les archives fabriquees sont nommees, la tout-systeme dit
//! sous `systemes` ceux qu'elle porte, et une installation dont l'un des systemes n'est pas publie garde sa version et le
//! dit. Un dossier portable recoit les systemes qui lui manquent par la route `portable-completer` (`completer_le_portable`).
//!
//! ====================================================================================================================
//! LA FORME DE L'ARCHIVE DE MISE A JOUR EST UN CONTRAT AVEC TOUTES LES INSTALLATIONS POSEES. ELLE NE CHANGE PLUS.
//! ====================================================================================================================
//! L'archive que `derniere.json` nomme sous `archives` (`puppetpaie-<version>-<systeme>.zip`) porte LE CONTENU DE
//! `logiciel/` A SA RACINE (`VERSION`, `donnees/`, `interface/`, les programmes), et rien d'autre : ni `logiciel/` en tete,
//! ni `fictives/`. C'est le code DEJA INSTALLE chez chaque compagnie qui la lit, pas celui qu'elle apporte : il la deballe
//! telle quelle dans `maj/<version>/logiciel/`, y lit `VERSION`, et refuse si ce n'est pas la version annoncee. Changer
//! cette forme, c'est couper toutes les installations posees de leurs mises a jour, sans retour possible : leur code ne se
//! change que par une mise a jour.
//! De meme pour `derniere.json` (`Publiee`) : sa forme est stricte, chaque installation posee refuse un `derniere.json`
//! qui porte une cle qu'elle ne connait pas. On ne peut donc ni ajouter, ni retirer, ni changer une cle sans couper
//! toutes les installations posees. `archive`, `sha256` et `signature` a la racine, que lit une installation qui ne
//! connait qu'une archive, sont ceux de l'archive de Windows.
//! `tests/fichiers/mise_a_jour.rs` deballe l'archive comme le fait le code des installations posees.
//!
//! L'ARCHIVE D'INSTALLATION, a cote (`puppetpaie-<version>-<systeme>-installation.zip`) : ce que l'installeur Windows
//! pose, pour Linux, Mac et le dossier portable (la tout-systeme) : `logiciel/` et, a cote, `fictives/` (les compagnies
//! fictives livrees). Decompressee dans un dossier neuf, elle s'ouvre bridee sur ses fictives. Elle ne sert qu'a
//! installer : aucune installation ne la telecharge, `derniere.json` ne la nomme pas, et elle n'est pas signee
//! (l'installeur Windows non plus) ; la Release la porte a cote des archives de mise a jour. Celle de tous les systemes
//! porte aussi `PORTABLE.json` a sa racine : decompressee sur une cle USB, elle est portable.
//!
//! LES FICTIVES VOYAGENT A PART : `puppetpaie-<version>-fictives.zip`, signee, nommee dans `derniere.json` sous `fictives`
//! avec l'EMPREINTE de leur contenu. Une installation ne
//! la telecharge que si l'empreinte n'est pas celle qu'elle retient (`fictives/.livraison.json`) : une Release aux memes
//! fictives ne coute rien. Quand elle a change, la livraison REMPLACE les fictives du poste au lancement suivant, jouees
//! ou non, photos d'origine comprises (`preparer_les_fictives`, `poser_les_fictives_pretes`). La mise a jour du logiciel,
//! elle, ne touche jamais `fictives/`.
//!
//! LE RELAIS, AU LANCEMENT, que personne ne voit. Sous Windows, un programme ne se remplace pas pendant qu'il tourne. Le
//! programme qu'on lance regarde donc, AVANT TOUTE FENETRE, si l'installation a une version prete (`au_lancement`) : il
//! copie le programme NEUF hors de `logiciel/` (`maj/relais-<version>`), le lance avec `maj-relais`, et s'arrete sans rien
//! avoir montre. Le relais attend la fin de ce processus (par son numero, pas par un port), bascule, lance la fenetre
//! neuve, et s'arrete : on voit s'ouvrir directement la neuve.
//!
//! UNE VERSION S'INSTALLE DES QU'ELLE EST PRETE : au lancement suivant, ou au clic « Redemarrer pour mettre a jour ».
//! Rien ne la retient. Sa phrase `calcul` (la case de « Publier ») dit a la compagnie ce qu'elle change d'un calcul de
//! paie : l'indicateur la montre avec la version prete, puis, une fois, au lancement qui suit son installation
//! (`dire_l_installation`) ; la route `maj-etat` aussi. Une version EXIGEE bloque tout depot reel tant qu'elle n'est pas
//! installee (`depot_acte::exigee_en_attente`) ; l'indicateur le dit, sans survol (`Recherche::etat`, `exigee`).
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use socle::date::Jour;
use socle::echec::{refus, refus_avec, APart, Echec, Lieu, R};
use socle::fichiers::{Empreinte, NomDeFichier};
use socle::num::Decimal;
use socle::paris::Moment;
use socle::garde_reseau::Url;


/// La cle publique d'un poste d'editeur (Ed25519, 32 octets en base64), que les installations posees acceptent encore
/// (`cles_de_l_editeur`) : plus rien ici ne fabrique sa cle privee, seule la cle de GitHub signe.
pub const CLE_PUBLIQUE: &str = "sDj5+46oBt7Yy1x00VJ1ThJNOwf5jT5+bttXEN/yAIU=";
/// LA CLE PUBLIQUE DE GITHUB : le flux « Publier » du depot public signe l'archive avec une cle dediee, dont la graine est
/// le secret `PUPPETPAIE_CLE_SIGNATURE` de l'environnement `publication` (approuve par l'editeur). Fabriquee par
/// `moteur maj --cle-github` (`generer_cle_github`), qui ecrit la cle publique dans un fichier a reporter
/// ICI. Vide, aucune archive ne serait acceptee par elle et le flux refuserait de publier (`publier_depuis_l_environnement`).
pub const CLE_PUBLIQUE_GITHUB: &str = "48dC1Gb82e2Zh7z7NiUH14EbXX7IvXTp83Xaui/31wg=";
/// Le depot des mises a jour QUAND LE REGLAGE DE L'INSTALLATION NE DIT RIEN : les fichiers de la DERNIERE RELEASE du
/// depot public. GitHub repond a
/// `releases/latest/download/<fichier>` par une redirection vers la Release la plus recente, puis vers son stockage :
/// `lire_depot` les suit (ureq, dix au plus). `derniere.json` y nomme les archives, servies a cote. Un `depot` pose dans le
/// reglage le remplace toujours. Il ne sert qu'a RECEVOIR (`verifier`) : `publier` ecrit dans un dossier.
pub const DEPOT_PAR_DEFAUT: Url = Url::du_moteur("https://github.com/LeandreSimioni/PuppetPaie/releases/latest/download");
pub const DERNIERE: &str = "derniere.json";
const AGENT: &str = "PuppetPaie (mise a jour)";
const NE_PAS_TOUCHER: [&str; 3] = [".git", ".avant", ".apres"];
/// Ce qui, dans `logiciel/`, dit l'INSTALLATION et non la version : il ne part jamais dans une archive (`archiver`), et la
/// bascule le laisse EXACTEMENT comme il etait -- remis s'il etait la (copie depuis `.avant/`, qui garde l'image entiere de
/// la version precedente pour `revenir`), absent s'il n'y etait pas, meme si l'archive en porte un. Une installation
/// debloquee reste debloquee ; bridee, elle reste bridee. Ce fichier est celui du PAQUET SOUDE (`"edition": "bride"`,
/// `bridage.rs`) ; le geste, `.edition.json`, vit a cote de `logiciel/` : la bascule n'y touche pas.
pub const PROPRES_A_L_INSTALLATION: [&str; 1] = ["EDITION.json"];

/// UN SYSTEME, dont une archive porte les programmes : les trois, chacun son archive. Son texte (`windows`, `linux`,
/// `mac`) est le nom du dossier des programmes de ce systeme dans `logiciel/` (sauf Windows, dont les programmes sont a la
/// racine : `dossier_programmes_de`) et la cle de son archive dans `derniere.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Systeme {
    Windows,
    Linux,
    Mac,
}

impl Systeme {
    /// Les trois, dans l'ordre des archives.
    pub const TOUS: [Systeme; 3] = [Systeme::Windows, Systeme::Linux, Systeme::Mac];

    /// Le systeme de ce programme, tel qu'il a ete compile. Ce qui n'est ni Windows ni macOS est lu comme Linux : le flux
    /// ne compile pour aucun autre, et un programme Linux qui ne se lance pas ailleurs est refuse par `controler`.
    pub const DE_CE_PROGRAMME: Systeme = if cfg!(windows) {
        Systeme::Windows
    } else if cfg!(target_os = "macos") {
        Systeme::Mac
    } else {
        Systeme::Linux
    };

    /// Son texte : le dossier de ses programmes, la cle de son archive.
    pub fn texte(self) -> &'static str {
        match self {
            Systeme::Windows => "windows",
            Systeme::Linux => "linux",
            Systeme::Mac => "mac",
        }
    }

    /// Son nom tel qu'on le lit.
    pub fn lu(self) -> &'static str {
        match self {
            Systeme::Windows => "Windows",
            Systeme::Linux => "Linux",
            Systeme::Mac => "macOS",
        }
    }
}

impl std::fmt::Display for Systeme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.texte())
    }
}

/// LA CLE D'UNE ARCHIVE publiee dans `derniere.json` : celle d'un systeme, ou celle qui porte les trois a la fois
/// (`tout-systeme` : un dossier portable qui passe d'une machine a l'autre, et le repli d'une installation dont le
/// systeme n'a pas d'archive a lui). Elle s'ecrit par son texte (`prete.json`, les noms des archives).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum CleDArchive {
    Du(Systeme),
    DeTous,
}

impl CleDArchive {
    /// Son texte : celui du systeme, ou `tout-systeme`.
    pub fn texte(self) -> &'static str {
        match self {
            CleDArchive::Du(s) => s.texte(),
            CleDArchive::DeTous => "tout-systeme",
        }
    }

    /// La cle ecrite `t` ; une autre se refuse.
    pub fn lire(t: &str) -> R<CleDArchive> {
        match Systeme::TOUS.into_iter().find(|s| s.texte() == t) {
            Some(s) => Ok(CleDArchive::Du(s)),
            None if t == CleDArchive::DeTous.texte() => Ok(CleDArchive::DeTous),
            None => Err(refus_avec("Une clé d'archive ne se lit pas : un système (windows, linux, mac) ou tout-systeme.", Lieu { detail: Some(t.to_string()), ..Default::default() })),
        }
    }
}

impl std::fmt::Display for CleDArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.texte())
    }
}

impl TryFrom<String> for CleDArchive {
    type Error = Echec;
    fn try_from(s: String) -> R<CleDArchive> {
        CleDArchive::lire(&s)
    }
}

impl From<CleDArchive> for String {
    fn from(c: CleDArchive) -> String {
        c.texte().to_string()
    }
}

/// Ce qui, a la racine de `logiciel/`, n'est que de Windows : la fenetre (suivie par Git), le chargeur de WebView2 qu'elle
/// charge au demarrage, et le moteur s'il y est pose.
const PROPRES_A_WINDOWS: [&str; 3] = ["puppetpaie.exe", "WebView2Loader.dll", "moteur.exe"];

/// Le dossier du logiciel (`logiciel/`), resolu.
pub fn code() -> PathBuf {
    crate::installation::logiciel()
}

/// Le dossier des programmes de CE systeme dans `logiciel/` (`dossier_programmes_de`).
pub fn dossier_programmes(code: &Path) -> PathBuf {
    dossier_programmes_de(code, Systeme::DE_CE_PROGRAMME)
}

/// Le dossier des programmes d'un systeme : `logiciel/` lui-meme pour Windows, ou l'exe est pose a cote de `donnees/` et
/// `interface/` (`logiciel/` est exactement ce que les gens recoivent ; le raccourci de l'installeur et `PuppetPaie.cmd`
/// le lancent la) ; `logiciel/linux/` et `logiciel/mac/` pour les deux autres, qui ont chacun un `puppetpaie` (l'archive
/// de tous les systemes les porte ensemble). Un programme pose dans un
/// sous-dossier trouve ses `donnees/` en remontant (`installation::fixer_le_code`).
pub fn dossier_programmes_de(code: &Path, systeme: Systeme) -> PathBuf {
    match systeme {
        Systeme::Windows => code.to_path_buf(),
        Systeme::Linux | Systeme::Mac => code.join(systeme.texte()),
    }
}

/// Le programme `nom` de CE systeme dans `logiciel/`.
pub fn programme(code: &Path, nom: &str) -> PathBuf {
    programme_de(code, Systeme::DE_CE_PROGRAMME, nom)
}

/// Le programme `nom` d'un systeme dans `logiciel/` : `puppetpaie.exe`, `linux/puppetpaie`, `mac/puppetpaie`.
pub fn programme_de(code: &Path, systeme: Systeme, nom: &str) -> PathBuf {
    dossier_programmes_de(code, systeme).join(if systeme == Systeme::Windows { format!("{}.exe", nom) } else { nom.to_string() })
}

/// Le systeme d'un fichier de `logiciel/` (chemin relatif, `/` comme separateur), ou `None` s'il est commun a tous
/// (`VERSION`, `donnees/`, `interface/`) : ce qui decide de ce que porte l'archive d'un systeme.
pub fn systeme_du_fichier(rel: &str) -> Option<Systeme> {
    match rel.split_once('/') {
        Some((tete, _)) => Systeme::TOUS.into_iter().find(|s| *s != Systeme::Windows && s.texte() == tete),
        None => PROPRES_A_WINDOWS.contains(&rel).then_some(Systeme::Windows),
    }
}

/// Le nom de l'archive d'une version pour un systeme (ou celle de tous) : `puppetpaie-<version>-<cle>.zip`.
pub fn nom_d_archive(version: &VersionDuLogiciel, cle: CleDArchive) -> R<NomDeFichier> {
    NomDeFichier::lire(&format!("puppetpaie-{}-{}.zip", version, cle))
}

/// Le nom de l'archive D'INSTALLATION d'une version (`archiver_installation`) : `puppetpaie-<version>-<systeme>-installation.zip`.
/// Il n'y en a que pour Linux, Mac et tous les systemes : Windows a son installeur.
pub fn nom_d_installation(version: &VersionDuLogiciel, cle: CleDArchive) -> R<NomDeFichier> {
    NomDeFichier::lire(&format!("puppetpaie-{}-{}-installation.zip", version, cle))
}

/// LA SEULE LECTURE DE `VERSION` : la version d'un logiciel. Un logiciel qui ne la dit pas, ou qui dit autre chose
/// qu'une version, se refuse.
pub fn courante(code: &Path) -> R<VersionDuLogiciel> {
    let p = code.join("VERSION");
    let t = socle::fichiers::lire_texte(&p).map_err(|e| socle::echec::du_disque(&p, &e))?;
    VersionDuLogiciel::lire(&t)
}

/// Le dossier du logiciel (`logiciel/`) qui porte ce programme : le parent du dossier du code.
pub fn dossier_installe() -> R<PathBuf> {
    let code = socle::fichiers::dossier_du_code();
    code.parent().map(Path::to_path_buf).ok_or_else(|| refus_avec("Le dossier du logiciel ne se trouve pas.", a_part_chemin(&code)))
}

/// UNE VERSION DU LOGICIEL : des nombres separes de points (`AAAA.MM.JJ`, `AAAA.MM.JJ.N`). Elle GARDE LE TEXTE LU (sans
/// ses blancs autour) et s'ecrit par lui, octet pour octet (`VERSION`, `derniere.json`, `prete.json`, les noms des
/// archives, `maj/<version>/`, ce que signe l'editeur) : `2026.11.01` reste `2026.11.01`. Deux versions sont la meme si
/// elles s'ecrivent pareil ; l'ordre des versions est celui de leurs nombres, de gauche a droite (`plus_recente_que`).
/// Elle ne se lit que par `lire`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VersionDuLogiciel {
    texte: String,
    nombres: Vec<u64>,
}

impl VersionDuLogiciel {
    /// La version ecrite `v` ; une version qui n'en est pas une se refuse.
    pub fn lire(v: &str) -> R<VersionDuLogiciel> {
        let texte = v.trim();
        let nombres = texte
            .split('.')
            .map(|x| x.parse::<u64>().map_err(|_| refus_avec("Une version ne se lit pas : elle s'écrit en nombres séparés de points.", Lieu { detail: Some(v.to_string()), ..Default::default() })))
            .collect::<R<Vec<u64>>>()?;
        Ok(VersionDuLogiciel { texte: texte.to_string(), nombres })
    }

    /// Le texte de la version, tel qu'il a ete lu.
    pub fn texte(&self) -> &str {
        &self.texte
    }

    /// Est-elle plus recente que `autre` ? Les nombres, de gauche a droite.
    pub fn plus_recente_que(&self, autre: &VersionDuLogiciel) -> bool {
        self.nombres > autre.nombres
    }

    /// LA VERSION TELLE QU'UN HUMAIN LA LIT : `JJ.MM.AAAA`, l'annee en tete embrouille.
    /// L'IDENTIFIANT, lui, reste `AAAA.MM.JJ` (`texte`) : l'ordre compare les nombres de gauche a droite, et `01.11.2026`
    /// passerait pour plus ancienne que `14.10.2026`. Seul l'affichage change : tout ce qui montre une version passe par
    /// ici. Une version qui n'a pas la forme d'une date (`0.0.0`) se dit par son texte.
    pub fn lue(&self) -> String {
        let p: Vec<&str> = self.texte.split('.').collect();
        let chiffres = |x: &str, n: usize| x.len() == n && x.bytes().all(|c| c.is_ascii_digit());
        if p.len() == 3 && chiffres(p[0], 4) && chiffres(p[1], 2) && chiffres(p[2], 2) {
            format!("{}.{}.{}", p[2], p[1], p[0])
        } else if p.len() == 4 && chiffres(p[0], 4) && chiffres(p[1], 2) && chiffres(p[2], 2) && !p[3].is_empty() && p[3].bytes().all(|c| c.is_ascii_digit()) {
            // La deuxieme version d'un meme jour (`AAAA.MM.JJ.N`, posee par le flux « Publier ») : « JJ.MM.AAAA (N) ».
            format!("{}.{}.{} ({})", p[2], p[1], p[0], p[3])
        } else {
            self.texte.clone()
        }
    }
}

impl PartialEq for VersionDuLogiciel {
    fn eq(&self, autre: &VersionDuLogiciel) -> bool {
        self.texte == autre.texte
    }
}

impl Eq for VersionDuLogiciel {}

impl std::fmt::Display for VersionDuLogiciel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.texte)
    }
}

impl TryFrom<String> for VersionDuLogiciel {
    type Error = Echec;
    fn try_from(s: String) -> R<VersionDuLogiciel> {
        VersionDuLogiciel::lire(&s)
    }
}

impl From<VersionDuLogiciel> for String {
    fn from(v: VersionDuLogiciel) -> String {
        v.texte
    }
}

/// LA SEULE LECTURE D'UN FICHIER DE `maj/` (`reglage.json`, `prete.json`, `appliquee.json`) : sa forme, stricte (une cle
/// qu'elle ne connait pas refuse) ; `None` s'il n'existe pas. `phrase` est celle du refus, qui dit quoi corriger ; le
/// chemin du fichier va a part.
fn lire_forme<T: serde::de::DeserializeOwned>(p: &Path, phrase: &str) -> R<Option<T>> {
    if !p.exists() {
        return Ok(None);
    }
    socle::forme::lire_fichier(p, phrase).map(Some)
}

pub fn ecrire_json<T: Serialize + ?Sized>(p: &Path, v: &T) -> R<()> {
    socle::ecriture::ecrire_json(p, v, 1)
}

/// L'INSTALLATION : le dossier qui porte `logiciel/`, et a cote les compagnies (`prive/`, `compagnies/`, les fictives) et
/// `maj/`. Il n'y a qu'un logiciel par installation, et c'est lui qui se met a jour, pas une compagnie : la mise a jour se cherche, se range et
/// s'applique ici, sans qu'aucune compagnie soit ouverte. Un dossier portable est une installation comme une autre.
pub fn installation() -> PathBuf {
    crate::installation::base()
}

/// `<installation>/maj/` : le reglage, la version prete, ce qui a ete applique, le journal. A cote de `logiciel/`, que la
/// bascule ne touche pas.
pub fn dossier_maj(inst: &Path) -> PathBuf {
    inst.join("maj")
}

/// Le dossier du logiciel dans une installation, et dans une archive.
pub const LOGICIEL: &str = "logiciel";

/// Le logiciel de l'installation.
fn logiciel_de(inst: &Path) -> PathBuf {
    inst.join(LOGICIEL)
}

const REGLAGE: &str = "reglage.json";

/// LE DEPOT DES MISES A JOUR : une adresse (`http://` ou `https://`, le depot public, un faux depot local), ou un dossier
/// du poste (ce que `publier` ecrit). Lu une fois, par son debut ; il s'ecrit et se dit tel qu'on l'a lu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum DepotDesMisesAJour {
    Adresse(Url),
    Dossier(PathBuf),
}

impl DepotDesMisesAJour {
    /// Le depot `texte` : une adresse s'il commence par `http://` ou `https://`, sinon un dossier ; vide, il se refuse.
    pub fn lire(texte: &str) -> R<DepotDesMisesAJour> {
        if texte.is_empty() {
            return Err(socle::refus!("Un dépôt des mises à jour vide : ni une adresse, ni un dossier."));
        }
        if texte.starts_with("http://") || texte.starts_with("https://") {
            return Ok(DepotDesMisesAJour::Adresse(Url::lire(texte)?));
        }
        Ok(DepotDesMisesAJour::Dossier(PathBuf::from(texte)))
    }
}

impl std::fmt::Display for DepotDesMisesAJour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DepotDesMisesAJour::Adresse(u) => f.write_str(u.texte()),
            DepotDesMisesAJour::Dossier(p) => f.write_str(&p.to_string_lossy()),
        }
    }
}

impl TryFrom<String> for DepotDesMisesAJour {
    type Error = Echec;
    fn try_from(s: String) -> R<DepotDesMisesAJour> {
        DepotDesMisesAJour::lire(&s)
    }
}

impl From<DepotDesMisesAJour> for String {
    fn from(d: DepotDesMisesAJour) -> String {
        d.to_string()
    }
}

/// `maj/reglage.json`, pose a la main : le depot d'ou l'installation recoit.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reglage {
    depot: Option<DepotDesMisesAJour>,
}

/// Le depot que pose le reglage de l'installation, `maj/reglage.json`, ou rien de pose s'il n'existe pas.
pub fn reglage(inst: &Path) -> R<Option<DepotDesMisesAJour>> {
    let r: Option<Reglage> = lire_forme(&dossier_maj(inst).join(REGLAGE), "Le réglage de la mise à jour ne se lit pas : il ne dit que le dépôt d'où l'installation reçoit.")?;
    Ok(r.and_then(|r| r.depot))
}

/// Le depot d'ou l'installation recoit : celui qui est pose, sinon celui du depot public (`DEPOT_PAR_DEFAUT`).
pub fn depot(pose: Option<DepotDesMisesAJour>) -> DepotDesMisesAJour {
    match pose {
        Some(d) => d,
        None => DepotDesMisesAJour::Adresse(DEPOT_PAR_DEFAUT),
    }
}

/// LA VERSION PRETE (`maj/prete.json`, que `telecharger` ecrit) : sa version, quand elle a ete prise et par quelles
/// archives, ses notes, sa phrase `calcul` (ce qu'elle change d'un calcul de paie, vide : rien) et sa raison `exigee`
/// (vide : elle n'est pas indispensable avant de declarer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prete {
    pub version: VersionDuLogiciel,
    #[serde(with = "socle::paris::ecrit::minute")]
    pub quand: Moment,
    pub archives: Vec<CleDArchive>,
    pub notes: String,
    pub calcul: String,
    pub exigee: String,
}

const PRETE: &str = "prete.json";

/// LA SEULE LECTURE DE `prete.json`, dans le dossier `maj` : la version prete dont le logiciel est la, ou `None`. Sa forme
/// est stricte, la seule qu'ecrit `telecharger` : le relais d'une version neuve (`appliquer_depuis`) lit ce qu'a ecrit le
/// code de l'installation posee, et une autre forme refuse la bascule (l'ancienne version reste en place et le dit).
fn prete_dans(maj: &Path) -> R<Option<Prete>> {
    let p: Option<Prete> = lire_forme(&maj.join(PRETE), "La mise à jour prête ne se lit pas : effacez son fichier, elle se téléchargera de nouveau.")?;
    Ok(p.filter(|p| maj.join(p.version.texte()).join(LOGICIEL).join("donnees").is_dir()))
}

/// La version telechargee, controlee, a appliquer.
pub fn prete(inst: &Path) -> R<Option<Prete>> {
    prete_dans(&dossier_maj(inst))
}

pub fn lire_depot(depot: &DepotDesMisesAJour, nom: &NomDeFichier) -> R<Vec<u8>> {
    lire_depot_suivi(depot, nom, &|_, _| {}).map_err(|(_, e)| e)
}

/// CE QUI A FAIT ECHOUER UNE RECHERCHE DE MISE A JOUR, dit en un mot a l'ecran, sans alarme (`mot`) ; la phrase entiere
/// va au journal de la mise a jour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EchecDeMiseAJour {
    HorsLigne,
    Introuvable,
    DepotIllisible,
    SignatureRefusee,
    ArchiveAlteree,
    RefuseeAuControle,
    ReglageIllisible,
}

impl EchecDeMiseAJour {
    /// Le mot que l'ecran en dit.
    pub fn mot(self) -> &'static str {
        match self {
            EchecDeMiseAJour::HorsLigne => "hors ligne",
            EchecDeMiseAJour::Introuvable => "introuvable",
            EchecDeMiseAJour::DepotIllisible => "dépôt illisible",
            EchecDeMiseAJour::SignatureRefusee => "signature refusée",
            EchecDeMiseAJour::ArchiveAlteree => "archive altérée",
            EchecDeMiseAJour::RefuseeAuControle => "refusée au contrôle",
            EchecDeMiseAJour::ReglageIllisible => "réglage illisible",
        }
    }
}

/// Un echec de la mise a jour : ce qui l'a fait echouer, et le refus.
type Rate = (EchecDeMiseAJour, Echec);

/// `lire_depot`, en disant a `suivre` ce qui est recu (octets recus, octets attendus si le depot le dit) : la barre de
/// l'indicateur (`Recherche`). Un echec dit s'il est injoignable (`HorsLigne`) ou absent (`Introuvable`).
fn lire_depot_suivi(depot: &DepotDesMisesAJour, nom: &NomDeFichier, suivre: &dyn Fn(u64, Option<u64>)) -> Result<Vec<u8>, Rate> {
    const LIMITE: u64 = 512 * 1024 * 1024;
    let lire_tout = |lecteur: &mut dyn Read, attendus: Option<u64>| -> std::io::Result<Vec<u8>> {
        let mut o = Vec::new();
        if let Some(n) = attendus {
            o.reserve(n.min(LIMITE) as usize);
        }
        let mut tampon = vec![0u8; 64 * 1024];
        suivre(0, attendus);
        loop {
            let n = lecteur.read(&mut tampon)?;
            if n == 0 {
                return Ok(o);
            }
            o.extend_from_slice(&tampon[..n]);
            suivre(o.len() as u64, attendus);
        }
    };
    let adresse = match depot {
        DepotDesMisesAJour::Adresse(a) => a,
        DepotDesMisesAJour::Dossier(dossier) => {
            let p = dossier.join(nom);
            let dit = |e: std::io::Error| (EchecDeMiseAJour::Introuvable, socle::echec::du_disque(&p, &e));
            let mut f = std::fs::File::open(&p).map_err(dit)?;
            let attendus = f.metadata().map_err(dit)?.len();
            return lire_tout(&mut f, Some(attendus)).map_err(dit);
        }
    };
    let url = Url::lire(&format!("{}/{}", adresse.texte().trim_end_matches('/'), nom)).map_err(|e| (EchecDeMiseAJour::HorsLigne, e))?;
    let mut c = ureq::Agent::config_builder().user_agent(AGENT).timeout_global(Some(std::time::Duration::from_secs(120)));
    if url.texte().starts_with("https:") {
        c = c.tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build());
    }
    // En essai, le garde-fou reseau : rien ne sort de la machine (une panne de reseau, comme hors ligne).
    let agent: ureq::Agent = socle::garde_reseau::regler(c, &url).map_err(|e| (EchecDeMiseAJour::HorsLigne, e.reformuler(|m| format!("{} : {}", url, m))))?.build().into();
    let mut r = agent.get(url.texte()).call().map_err(|e| {
        let mot = if matches!(e, ureq::Error::StatusCode(_)) { EchecDeMiseAJour::Introuvable } else { EchecDeMiseAJour::HorsLigne };
        (mot, socle::refus!("{} : {}", url, e))
    })?;
    let attendus = r.body().content_length();
    let mut lecteur = r.body_mut().with_config().limit(LIMITE).reader();
    lire_tout(&mut lecteur, attendus).map_err(|e| (EchecDeMiseAJour::HorsLigne,socle::refus!("{} : {}", url, e)))
}

/// `derniere.json`, LA VERSION PUBLIEE : ce qu'ecrit `moteur::publication` et que lit chaque installation, une
/// seule forme, stricte (une cle qu'elle ne connait pas refuse tout le fichier : une cle ajoutee a la publication coupe
/// donc les installations posees de leurs mises a jour). `archive`, `sha256` et `signature` a la racine sont ceux de
/// l'archive de Windows, pour les installations qui ne lisent qu'une archive ; `compagnie::maj` ne les lit pas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publiee {
    pub version: VersionDuLogiciel,
    pub publiee_le: Jour,
    pub notes: String,
    pub calcul: String,
    pub exigee: String,
    pub archives: Archives,
    pub fictives: FictivesPubliees,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive: Option<NomDeFichier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<Empreinte>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// Les archives de mise a jour publiees, une par systeme fabrique (`Systeme`) et celle de tous (`CleDArchive::DeTous`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Archives {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows: Option<ArchivePubliee>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux: Option<ArchivePubliee>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<ArchivePubliee>,
    #[serde(rename = "tout-systeme", skip_serializing_if = "Option::is_none")]
    pub tout_systeme: Option<ArchivePubliee>,
}

/// Une archive signee : le nom du fichier, a cote, son condensat et la signature de l'editeur. Celle de tous les systemes
/// dit sous `systemes` ceux qu'elle porte (ceux qui ont ete fabriques) ; celle d'un systeme n'en dit aucun.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchivePubliee {
    pub archive: NomDeFichier,
    pub sha256: Empreinte,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub systemes: Option<Vec<Systeme>>,
}

/// Les fictives, signees comme une archive, et l'empreinte de leur contenu.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FictivesPubliees {
    pub archive: NomDeFichier,
    pub sha256: Empreinte,
    pub signature: String,
    pub empreinte: Empreinte,
}

impl ArchivePubliee {
    /// Ce que `archive_verifiee` verifie : le nom, le condensat, la signature.
    fn signee(&self) -> (&NomDeFichier, &Empreinte, &str) {
        (&self.archive, &self.sha256, &self.signature)
    }
}

impl FictivesPubliees {
    /// Ce que `archive_verifiee` verifie : le nom, le condensat, la signature.
    fn signee(&self) -> (&NomDeFichier, &Empreinte, &str) {
        (&self.archive, &self.sha256, &self.signature)
    }
}

impl Archives {
    /// L'archive publiee sous la cle `cle` (un systeme, ou celle de tous).
    pub fn de(&self, cle: CleDArchive) -> Option<&ArchivePubliee> {
        match cle {
            CleDArchive::Du(Systeme::Windows) => self.windows.as_ref(),
            CleDArchive::Du(Systeme::Linux) => self.linux.as_ref(),
            CleDArchive::Du(Systeme::Mac) => self.mac.as_ref(),
            CleDArchive::DeTous => self.tout_systeme.as_ref(),
        }
    }

    /// Chaque archive publiee, sous sa cle, dans l'ordre des systemes puis celle de tous.
    pub fn toutes(&self) -> impl Iterator<Item = (CleDArchive, &ArchivePubliee)> {
        Systeme::TOUS.into_iter().map(CleDArchive::Du).chain([CleDArchive::DeTous]).filter_map(|k| self.de(k).map(|a| (k, a)))
    }

    /// Pose l'archive de la cle `cle`.
    pub fn poser(&mut self, cle: CleDArchive, a: ArchivePubliee) {
        match cle {
            CleDArchive::Du(Systeme::Windows) => self.windows = Some(a),
            CleDArchive::Du(Systeme::Linux) => self.linux = Some(a),
            CleDArchive::Du(Systeme::Mac) => self.mac = Some(a),
            CleDArchive::DeTous => self.tout_systeme = Some(a),
        }
    }
}

/// `derniere.json` du depot (`Publiee`). Toute autre forme est refusee.
pub fn derniere(depot: &DepotDesMisesAJour) -> R<Publiee> {
    derniere_dite(depot).map_err(|(_, e)| e)
}

/// `derniere`, l'echec avec son mot : le depot ne repond pas, ou il repond quelque chose qui n'est pas un `derniere.json`.
fn derniere_dite(depot: &DepotDesMisesAJour) -> Result<Publiee, Rate> {
    let octets = lire_depot_suivi(depot, &const { NomDeFichier::du_moteur(DERNIERE) }, &|_, _| {})?;
    derniere_lue(&octets).map_err(|e| (EchecDeMiseAJour::DepotIllisible, e))
}

const PUBLIEE_ILLISIBLE: &str = "La version publiée ne se lit pas : la mise à jour se refera à la prochaine.";

/// La version publiee (`derniere.json`) qui ne se lit pas : la phrase en mots, la cle a part.
#[track_caller]
fn publiee_illisible(cle: &str, detail: impl std::fmt::Display) -> Echec {
    refus_avec(PUBLIEE_ILLISIBLE, Lieu { fichier: Some(DERNIERE.to_string()), cle: Some(cle.to_string()), detail: Some(detail.to_string()), ..Default::default() })
}

/// LA SEULE LECTURE DE `derniere.json` : sa forme (`Publiee`, dont la version), puis ce qu'elle ne dit pas d'elle-meme :
/// au moins une archive, aucun champ vide, et la liste des systemes de celle de tous.
pub fn derniere_lue(octets: &[u8]) -> R<Publiee> {
    let texte = std::str::from_utf8(octets).map_err(|e| publiee_illisible("", e))?;
    let d: Publiee = socle::forme::lire_texte(texte, PUBLIEE_ILLISIBLE)?;
    if d.archives.toutes().next().is_none() {
        return Err(publiee_illisible("archives", ""));
    }
    for (systeme, a) in d.archives.toutes() {
        if a.signature.is_empty() {
            return Err(publiee_illisible(&format!("archives.{}.signature", systeme), ""));
        }
        // Celle de tous les systemes dit ceux qu'elle porte (au moins un) ; celle d'un systeme n'en dit aucun.
        let dit_ses_systemes = a.systemes.as_ref().is_some_and(|s| !s.is_empty());
        if (systeme == CleDArchive::DeTous) != dit_ses_systemes {
            return Err(publiee_illisible(&format!("archives.{}.systemes", systeme), ""));
        }
    }
    if d.fictives.signature.is_empty() {
        return Err(publiee_illisible("fictives.signature", ""));
    }
    Ok(d)
}

pub fn b64(t: &str) -> R<Vec<u8>> {
    socle::base64::lire(t)
}

/// Les cles publiques dont une archive est acceptee : celle du poste de l'editeur (`CLE_PUBLIQUE`) et celle du flux de
/// GitHub (`CLE_PUBLIQUE_GITHUB`), quand elle est reportee. Une signature de toute autre cle est refusee.
pub fn cles_de_l_editeur() -> Vec<&'static str> {
    let v: Vec<&'static str> = [CLE_PUBLIQUE, CLE_PUBLIQUE_GITHUB].into_iter().filter(|c| !c.is_empty()).collect();
    #[cfg(feature = "atelier")]
    let v = avec_la_cle_de_l_atelier(v);
    v
}

/// LE BINAIRE DE L'ATELIER (le banc, jamais livre) accepte aussi la cle d'un faux depot local, que nomme
/// `PUPPETPAIE_ATELIER_CLE_MAJ` (une cle publique Ed25519 en base64) : la mise a jour et le dossier portable s'y
/// jouent de bout en bout par `moteur route`. Le binaire livre n'en porte rien (`tests/atelier.rs`).
#[cfg(feature = "atelier")]
fn avec_la_cle_de_l_atelier(mut v: Vec<&'static str>) -> Vec<&'static str> {
    if let Some(c) = std::env::var_os("PUPPETPAIE_ATELIER_CLE_MAJ") {
        let c = c.to_string_lossy();
        if !c.trim().is_empty() {
            v.push(Box::leak(c.trim().to_string().into_boxed_str()));
        }
    }
    v
}

/// La signature est-elle celle de l'une des `cles` ?
pub fn signee_par_l_une(octets: &[u8], signature: &str, cles: &[&str]) -> bool {
    cles.iter().any(|c| signature_valide(octets, signature, c))
}

/// La signature `signature` des `octets` est-elle celle de `cle_publique` ? Une cle ou une signature qui ne se lit pas,
/// une signature d'une autre cle ou d'autres octets : non.
pub fn signature_valide(octets: &[u8], signature: &str, cle_publique: &str) -> bool {
    matches!(verifier_la_signature(octets, signature, cle_publique), Ok(()))
}

/// `signature_valide`, la raison du non dite.
fn verifier_la_signature(octets: &[u8], signature: &str, cle_publique: &str) -> R<()> {
    let cle = <[u8; 32]>::try_from(b64(cle_publique)?.as_slice()).map_err(|e| refus(e.to_string()))?;
    let sig = <[u8; 64]>::try_from(b64(signature)?.as_slice()).map_err(|e| refus(e.to_string()))?;
    let v = ed25519_dalek::VerifyingKey::from_bytes(&cle).map_err(|e| refus(e.to_string()))?;
    v.verify_strict(octets, &ed25519_dalek::Signature::from_bytes(&sig)).map_err(|e| refus(e.to_string()))
}

/// Les compagnies de l'installation : la compagnie du poste (`prive/`), les autres (`compagnies/*`), les fictives ; pas
/// une copie de developpement.
pub fn compagnies_de(inst: &Path) -> R<Vec<PathBuf>> {
    let mut v = vec![inst.join("prive")];
    let groupes: [&str; 2] = ["compagnies", crate::installation::DOSSIER_FICTIVES];
    for groupe in groupes {
        let mut autres: Vec<PathBuf> = socle::fichiers::lister(&inst.join(groupe))?.into_iter().flatten().collect();
        autres.sort();
        v.extend(autres);
    }
    let mut gardees = Vec::new();
    for r in v {
        if !crate::installation::est_cache(&r) && r.join(droit::contexte::ENTREPRISE).is_file() && (!crate::installation::est_copie(&r) || crate::installation::est_fictive(&r)?) {
            gardees.push(r);
        }
    }
    Ok(gardees)
}

/// L'ARCHIVE DE MISE A JOUR (`archiver` : le contenu de `logiciel/` a sa racine, le contrat de la tete de ce fichier),
/// deballee telle quelle dans `neuf` (`maj/<version>/logiciel/`). Rien n'en sort de `neuf` (`enclosed_name`) : les
/// compagnies du poste, les fictives jouees et leur photo d'origine (`fictives/.origine/`) ne sont jamais touchees, et
/// l'archive n'en porte aucune. Une archive D'INSTALLATION (`archiver_installation` : `logiciel/` et `fictives/` en tete)
/// n'est pas une mise a jour : refusee avant d'ecrire quoi que ce soit.
pub fn deballer(octets: &[u8], neuf: &Path) -> R<()> {
    deballer_ce_qui(octets, neuf, &|_| true)
}

/// `deballer`, seulement les fichiers dont le nom (chemin relatif, `/`) passe `garder` : les programmes d'un systeme
/// qu'un dossier portable ajoute (`completer_le_portable`).
fn deballer_ce_qui(octets: &[u8], neuf: &Path, garder: &dyn Fn(&str) -> bool) -> R<()> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(octets)).map_err(|e| socle::refus!("archive illisible : {}", e))?;
    if z.file_names().any(|n| n == format!("{}/VERSION", LOGICIEL) || n.starts_with(&format!("{}/", crate::installation::DOSSIER_FICTIVES))) {
        return Err(refus("C'est une archive d'installation, pas une archive de mise à jour."));
    }
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| refus(e.to_string()))?;
        let Some(rel) = f.enclosed_name() else { return Err(socle::refus!("archive suspecte : {}", f.name())) };
        if !garder(&rel.to_string_lossy().replace('\\', "/")) {
            continue;
        }
        let chemin = neuf.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&chemin).map_err(|e| refus(e.to_string()))?;
            continue;
        }
        if let Some(p) = chemin.parent() {
            std::fs::create_dir_all(p).map_err(|e| refus(e.to_string()))?;
        }
        let mut o = Vec::new();
        f.read_to_end(&mut o).map_err(|e| refus(e.to_string()))?;
        std::fs::write(&chemin, o).map_err(|e| refus(e.to_string()))?;
        // Un programme d'un systeme Unix : `archiver` l'a marque executable, il le redevient ici.
        #[cfg(unix)]
        if f.unix_mode().is_some_and(|m| m & 0o111 != 0) {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&chemin, std::fs::Permissions::from_mode(0o755)).map_err(|e| socle::echec::du_disque(&chemin, &e))?;
        }
    }
    Ok(())
}

/// Ce qu'un refus porte a part : le chemin `p`.
fn a_part_chemin(p: &Path) -> Lieu {
    Lieu { chemin: Some(p.to_string_lossy().into_owned()), ..Default::default() }
}

/// Le nouveau code demarre-t-il, et lit-il NOS donnees ? Son programme, sur la racine de cette compagnie.
fn controler(neuf: &Path, racine: &Path) -> R<()> {
    // La fenetre sert aussi les routes (`puppetpaie route`) : c'est elle qu'on livre, pas la ligne de commande.
    let exe = programme(neuf, "puppetpaie");
    if !exe.is_file() {
        return Err(refus_avec("Le programme de ce système manque dans l'archive.", a_part_chemin(&exe)));
    }
    let mut c = std::process::Command::new(&exe);
    c.args(["route", "--racine"])
        .arg(racine)
        .env("PUPPETPAIE_CODE", neuf.join("donnees"))
        .env("PUPPETPAIE_DONNEES", racine)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let mut p = c.spawn().map_err(|e| socle::refus!("le nouveau programme ne se lance pas : {}", e))?;
    // Un programme qui n'a pas recu sa demande le dit par son issue : son refus de la lire passe d'abord.
    let demande = match p.stdin.take() {
        Some(mut e) => e.write_all(br#"{"route": "configuration", "charge": {}}"#),
        None => Err(std::io::Error::other("son entree n'est pas ouverte")),
    };
    let s = p.wait_with_output().map_err(|e| refus(e.to_string()))?;
    if s.status.success() {
        if let Err(e) = demande {
            return Err(socle::refus!("il n'a pas reçu sa demande ({})", e));
        }
    }
    let sortie = String::from_utf8_lossy(&s.stdout);
    let erreurs = || -> String { String::from_utf8_lossy(&s.stderr).trim().chars().rev().take(800).collect::<String>().chars().rev().collect() };
    // L'issue est le code de sortie de `moteur route` (0 : la route a repondu ; un refus sort en 1, `service::commande`) :
    // le programme d'aujourd'hui ne lit pas la reponse de celui de demain, dont la forme peut avoir change. Un echec se
    // dit avec la derniere ligne qu'il a ecrite, en texte, et ce qu'il a dit sur sa sortie d'erreur.
    if s.status.success() {
        return Ok(());
    }
    let derniere = sortie.lines().rev().find(|l| !l.trim().is_empty()).map(str::trim).map(String::from);
    let dit: Vec<String> = derniere.into_iter().chain(Some(erreurs()).filter(|e| !e.is_empty())).collect();
    Err(socle::refus!("il ne lit pas la configuration ({})", dit.join(" ; ")))
}

/// UNE ARCHIVE SIGNEE, recue et verifiee : son entree de `derniere.json` (`archive`, `sha256`, `signature`) lue au depot,
/// son condensat et la signature de l'editeur (l'une des `cles`) verifies. Le seul chemin par ou une archive de l'editeur
/// entre dans une installation : la mise a jour, les fictives, les systemes qu'un dossier portable ajoute
/// (`completer_le_portable`). Un echec de lecture porte `HorsLigne` ou `Introuvable` ; une archive qui n'est pas la
/// notre, `ArchiveAlteree` ou `SignatureRefusee`.
fn archive_verifiee(depot: &DepotDesMisesAJour, (nom, attendu, signature): (&NomDeFichier, &Empreinte, &str), cles: &[&str], suivre: &dyn Fn(u64, Option<u64>)) -> Result<Vec<u8>, Rate> {
    let octets = lire_depot_suivi(depot, nom, suivre)?;
    let sha = Empreinte::des_octets(&octets);
    if sha != *attendu {
        return Err((EchecDeMiseAJour::ArchiveAlteree, refus_avec("Une archive téléchargée est altérée : elle se télécharge de nouveau à la prochaine mise à jour.", APart::forme(Alteree { fichier: nom.to_string(), condensat: sha.to_string(), attendu: attendu.to_string() }))));
    }
    if !signee_par_l_une(&octets, signature, cles) {
        return Err((EchecDeMiseAJour::SignatureRefusee, socle::refus!("la signature de {} n'est pas celle de l'editeur", nom)));
    }
    Ok(octets)
}

/// Ce que le refus d'une archive alteree porte a part : le fichier, son condensat, celui qu'annonce `derniere.json`.
#[derive(Serialize)]
struct Alteree {
    fichier: String,
    condensat: String,
    attendu: String,
}

/// LES SYSTEMES D'UNE INSTALLATION : celui de ce programme, puis ceux dont la fenetre est posee dans `logiciel/`
/// (`programme_de`). Une installation ordinaire n'en a qu'un ; un dossier portable en porte plusieurs, et la mise a jour
/// les tient tous a jour.
pub fn systemes_poses(code: &Path) -> Vec<Systeme> {
    let mut v = vec![Systeme::DE_CE_PROGRAMME];
    v.extend(Systeme::TOUS.into_iter().filter(|s| *s != Systeme::DE_CE_PROGRAMME && programme_de(code, *s, "puppetpaie").is_file()));
    v
}

/// Les textes de systemes ou de cles, joints : ce que le journal de la mise a jour en dit.
fn joints<T: std::fmt::Display>(l: &[T]) -> String {
    l.iter().map(T::to_string).collect::<Vec<_>>().join(", ")
}

/// Les archives qui portent `systemes`, chacune en un jeu a prendre en entier : l'archive de chaque systeme, et celle de
/// tous les systemes SI ELLE LES PORTE TOUS (sa liste `systemes`) ; leur cle dans `derniere.json` et leur entree, dans
/// l'ordre ou les essayer (pour un systeme seul, la sienne d'abord ; pour plusieurs, celle de tous, une seule archive).
/// Vide : la version est publiee sans l'un d'eux (« Publier » choisit Windows seul, Windows et Linux, ou les trois),
/// l'installation ne telecharge rien.
fn jeux_a_essayer<'a>(d: &'a Publiee, systemes: &[Systeme]) -> Vec<Vec<(CleDArchive, &'a ArchivePubliee)>> {
    let porte = |a: &ArchivePubliee| systemes.iter().all(|s| a.systemes.iter().flatten().any(|x| x == s));
    let siens: Option<Vec<(CleDArchive, &ArchivePubliee)>> = systemes.iter().map(|s| d.archives.de(CleDArchive::Du(*s)).map(|a| (CleDArchive::Du(*s), a))).collect();
    let tous = d.archives.de(CleDArchive::DeTous).filter(|a| porte(a)).map(|a| vec![(CleDArchive::DeTous, a)]);
    let v: Vec<_> = if systemes.len() > 1 { [tous, siens] } else { [siens, tous] }.into_iter().flatten().collect();
    v
}

/// Telecharge les archives qui portent les systemes de l'installation (`systemes_poses`, `jeux_a_essayer` : un jeu qui ne
/// se telecharge pas laisse essayer le suivant), les verifie (`archive_verifiee`), les deballe l'une sur l'autre (les
/// fichiers communs sont les memes), la controle et la pose prete. Rend les cles des archives prises. Une archive
/// telechargee dont le condensat ou la signature ne vaut pas est refusee sans essayer l'autre : ce n'est pas qu'elle
/// manque, c'est qu'elle n'est pas la notre.
fn telecharger(inst: &Path, depot: &DepotDesMisesAJour, d: &Publiee, cles: &[&str], suivi: &Recherche) -> Result<Prete, Rate> {
    let version = &d.version;
    let systemes = systemes_poses(&logiciel_de(inst));
    let jeux = jeux_a_essayer(d, &systemes);
    if jeux.is_empty() {
        let phrase = format!("La version publiée n'a pas d'archive pour ce système ({}).", joints(&systemes));
        return Err((EchecDeMiseAJour::Introuvable, refus_avec(phrase, Lieu { fichier: Some(DERNIERE.to_string()), ..Default::default() })));
    }
    let mut manquees = Vec::new();
    let mut mot = EchecDeMiseAJour::Introuvable;
    let mut prises = None;
    'jeux: for jeu in jeux {
        let mut recues = Vec::new();
        for (k, a) in jeu {
            let suivre = |recus: u64, attendus: Option<u64>| suivi.poser(Etape::Telechargement, Some(version), |p| (p.recus, p.attendus) = (recus, attendus));
            match archive_verifiee(depot, a.signee(), cles, &suivre) {
                Ok(o) => recues.push((k, o)),
                Err((m @ (EchecDeMiseAJour::HorsLigne | EchecDeMiseAJour::Introuvable), e)) => {
                    mot = m;
                    manquees.push(e);
                    continue 'jeux;
                }
                Err(e) => return Err(e),
            }
        }
        prises = Some(recues);
        break;
    }
    // Les phrases de chaque jeu manque, ensemble : le mot de l'ecran est celui du dernier.
    let prises = prises.ok_or_else(|| (mot, refus(manquees.iter().map(|e: &Echec| e.phrase.as_str()).collect::<Vec<_>>().join(" ; "))))?;
    suivi.poser(Etape::Preparation, Some(version), |_| {});
    let au_controle = |e: Echec| (EchecDeMiseAJour::RefuseeAuControle, e);
    let cible = dossier_maj(inst).join(version.texte());
    socle::fichiers::effacer(&cible).map_err(au_controle)?;
    let neuf = logiciel_de(&cible);
    std::fs::create_dir_all(&neuf).map_err(|e| au_controle(refus(e.to_string())))?;
    for (_, octets) in &prises {
        deballer(octets, &neuf).map_err(au_controle)?;
    }
    let dite = courante(&neuf).map_err(au_controle)?;
    if dite != *version {
        return Err(au_controle(socle::refus!("l'archive dit {}, l'annonce de la dernière version dit {}", dite.lue(), version.lue())));
    }
    // Le programme de chaque systeme doit y etre, meme sans compagnie a controler.
    if let Some(s) = systemes.iter().copied().find(|s| !programme_de(&neuf, *s, "puppetpaie").is_file()) {
        return Err(au_controle(socle::fichiers::effacer_apres(
            socle::echec::refus_avec(
                format!("le programme de {} manque dans l'archive", s),
                socle::echec::Lieu { chemin: Some(programme_de(Path::new(""), s, "puppetpaie").to_string_lossy().into_owned()), ..Default::default() },
            ),
            &cible,
        )));
    }
    // Le programme neuf lit la configuration de chaque compagnie de l'installation (sans compagnie, il n'y a rien a lire).
    for racine in compagnies_de(inst).map_err(au_controle)? {
        if let Err(p) = controler(&neuf, &racine) {
            let p = p.reformuler(|m| format!("le nouveau code ne demarre pas sur {} : {}", racine.display(), m));
            return Err(au_controle(socle::fichiers::effacer_apres(p, &cible)));
        }
    }
    let prete = Prete {
        version: version.clone(),
        quand: socle::temps::secondes().a_la_minute(),
        archives: prises.iter().map(|(k, _)| *k).collect(),
        notes: d.notes.clone(),
        calcul: d.calcul.clone(),
        exigee: d.exigee.clone(),
    };
    ecrire_prete(&dossier_maj(inst), &prete).map_err(au_controle)?;
    Ok(prete)
}

/// LA SEULE ECRITURE DE `prete.json` (`Prete`).
fn ecrire_prete(maj: &Path, p: &Prete) -> R<()> {
    ecrire_json(&maj.join(PRETE), p)
}

/// Le depot qui porte la version `version` : la Release de cette version quand le reglage ne dit rien
/// (`DEPOT_PAR_DEFAUT` nomme la derniere ; GitHub sert les fichiers d'une Release sous `releases/download/v<version>`,
/// le nom que lui donne le flux « Publier »). Un depot pose dans le reglage est pris tel quel.
fn depot_de_la_version(depot: &DepotDesMisesAJour, version: &VersionDuLogiciel) -> R<DepotDesMisesAJour> {
    match depot {
        DepotDesMisesAJour::Adresse(a) if *a == DEPOT_PAR_DEFAUT => match a.texte().strip_suffix("/latest/download") {
            Some(tete) => Ok(DepotDesMisesAJour::Adresse(Url::lire(&format!("{}/download/v{}", tete, version))?)),
            None => Ok(depot.clone()),
        },
        _ => Ok(depot.clone()),
    }
}

/// LE DOSSIER PORTABLE COMPLETE : une installation recoit les programmes des systemes qui lui manquent, POUR LA VERSION
/// INSTALLEE, et s'ouvre ensuite sous Windows, Linux et macOS. Les archives de cette version (`derniere.json` de son
/// depot, `depot_de_la_version`) : celle de chaque systeme manquant, sinon celle de tous qui le porte. Toutes sont
/// recues et verifiees (`archive_verifiee`) avant que rien ne s'ecrive ; seuls les fichiers de ces systemes
/// (`systeme_du_fichier`) vont dans `logiciel/`. `portable` pose aussi `PORTABLE.json` (`coffre::MARQUEUR_PORTABLE`).
/// La mise a jour tient ensuite tous ces systemes a jour (`systemes_poses`). Ce que le telechargement recoit se dit
/// sur la sortie d'erreur (l'installeur Windows le montre ; la sortie standard reste la reponse de la route).
pub fn completer_le_portable(inst: &Path, portable: bool, cles: &[&str]) -> R<PortableComplete> {
    let rien = "Rien n'est posé.";
    let code = logiciel_de(inst);
    if !code.join("VERSION").is_file() {
        return Err(refus_avec(format!("Ce dossier n'est pas une installation de PuppetPaie. {}", rien), a_part_chemin(inst)));
    }
    let version = courante(&code)?;
    if a_installer(inst)?.is_some() {
        return Err(socle::refus!("Une mise à jour attend le redémarrage : redémarrez PuppetPaie, puis recommencez. {}", rien));
    }
    let manquants: Vec<Systeme> = Systeme::TOUS.into_iter().filter(|s| !programme_de(&code, *s, "puppetpaie").is_file()).collect();
    let mut ajoutes: Vec<Systeme> = Vec::new();
    let mut absents: Vec<Systeme> = Vec::new();
    if !manquants.is_empty() {
        let hors_ligne = |e: Echec| e.reformuler(|m| format!("Le dépôt ne répond pas ({}). {} Reconnectez-vous, puis recommencez.", m, rien));
        let depot = depot_de_la_version(&depot(reglage(inst)?), &version)?;
        let d = derniere_dite(&depot).map_err(|(mot, e)| {
            if mot == EchecDeMiseAJour::HorsLigne {
                hors_ligne(e)
            } else {
                e.reformuler(|m| format!("Le dépôt ne donne pas la version {} ({}). {}", version.lue(), m, rien))
            }
        })?;
        if d.version != version {
            return Err(socle::refus!("Le dépôt n'a plus la version installée ({}) : mettez PuppetPaie à jour, puis recommencez. {}", version.lue(), rien));
        }
        // Chaque systeme manquant, son archive : la sienne, sinon celle de tous qui le porte (une seule fois pour plusieurs).
        let mut a_prendre: Vec<(&ArchivePubliee, Vec<Systeme>)> = Vec::new();
        for s in manquants.iter().copied() {
            match jeux_a_essayer(&d, &[s]).into_iter().next().and_then(|j| j.into_iter().next()) {
                Some((CleDArchive::DeTous, a)) => match a_prendre.iter_mut().find(|(x, _)| std::ptr::eq(*x, a)) {
                    Some((_, l)) => l.push(s),
                    None => a_prendre.push((a, vec![s])),
                },
                Some((CleDArchive::Du(_), a)) => a_prendre.push((a, vec![s])),
                None => absents.push(s),
            }
        }
        let mut recues = Vec::new();
        for (a, systemes) in a_prendre {
            let qui = systemes.iter().map(|s| s.lu()).collect::<Vec<_>>().join(" et ");
            let dit = std::cell::Cell::new(None);
            let suivre = |recus: u64, attendus: Option<u64>| {
                let quart = attendus.filter(|a| *a > 0).map(|a| recus * 4 / a);
                if quart.is_some() && quart != dit.get() {
                    dit.set(quart);
                    let mo = |o: u64| crate::sauvegarde::en_mo(o, ',');
                    if let Some(a) = attendus {
                        eprintln!("{} : {} reçus sur {}", qui, mo(recus), mo(a));
                    }
                }
            };
            let de_qui = |e: Echec| e.reformuler(|m| format!("{} : {}. {}", qui, m, rien));
            let octets = archive_verifiee(&depot, a.signee(), cles, &suivre).map_err(|(mot, e)| if mot == EchecDeMiseAJour::HorsLigne { hors_ligne(e) } else { de_qui(e) })?;
            let dite = version_de_l_archive(&octets).map_err(de_qui)?;
            if dite != version {
                return Err(socle::refus!("{} : l'archive n'est pas de la version {}. {}", qui, version.lue(), rien));
            }
            recues.push((octets, systemes));
        }
        for (octets, systemes) in &recues {
            deballer_ce_qui(octets, &code, &|rel| systeme_du_fichier(rel).is_some_and(|s| systemes.contains(&s)))?;
            ajoutes.extend(systemes.iter().copied().filter(|s| programme_de(&code, *s, "puppetpaie").is_file()));
        }
    }
    if portable && !inst.join(crate::coffre::MARQUEUR_PORTABLE).is_file() {
        std::fs::write(inst.join(crate::coffre::MARQUEUR_PORTABLE), crate::coffre::CONTENU_PORTABLE).map_err(|e| socle::echec::du_disque(&inst, &e))?;
    }
    let noms = |l: &[Systeme]| l.iter().map(|s| s.lu()).collect::<Vec<_>>().join(", ");
    let mut phrase = if ajoutes.is_empty() && absents.is_empty() {
        "Les trois systèmes sont déjà là.".to_string()
    } else if ajoutes.is_empty() {
        String::new()
    } else {
        format!("Ajoutés : {}.", noms(&ajoutes))
    };
    if !absents.is_empty() {
        phrase.push_str(&format!("{}La {} n'est pas publiée pour {}.", if phrase.is_empty() { "" } else { " " }, version.lue(), noms(&absents)));
    }
    if inst.join(crate::coffre::MARQUEUR_PORTABLE).is_file() {
        phrase.push_str(" Le dossier est portable.");
    }
    Ok(PortableComplete { ajoutes, absents, version, portable: inst.join(crate::coffre::MARQUEUR_PORTABLE).is_file(), phrase })
}

/// CE QUE REND `portable-completer` (`completer_le_portable`) : les systemes ajoutes, ceux que la version ne publie pas, la
/// version, si le dossier est portable, et la phrase.
#[derive(Debug, Serialize)]
pub struct PortableComplete {
    pub ajoutes: Vec<Systeme>,
    pub absents: Vec<Systeme>,
    pub version: VersionDuLogiciel,
    pub portable: bool,
    pub phrase: String,
}


/// La version que porte une archive de mise a jour (son fichier `VERSION`) : une archive qui ne la dit pas se refuse.
fn version_de_l_archive(octets: &[u8]) -> R<VersionDuLogiciel> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(octets)).map_err(|e| socle::refus!("archive illisible : {}", e))?;
    let mut f = z.by_name("VERSION").map_err(|e| socle::refus!("l'archive ne dit pas sa version : {}", e))?;
    let mut t = String::new();
    f.read_to_string(&mut t).map_err(|e| socle::refus!("l'archive ne dit pas sa version : {}", e))?;
    VersionDuLogiciel::lire(&t)
}

/// LES FICTIVES PRETES : la livraison telechargee et verifiee (`preparer_les_fictives`), qui attend le prochain lancement
/// dans `maj/fictives-pretes/fictives/` ; `poser_les_fictives_pretes` la met en place.
const FICTIVES_PRETES: &str = "fictives-pretes";

/// LA LIVRAISON DE FICTIVES (`fictives/.livraison.json`, que l'archive des fictives porte et que la pose recrit) : son
/// empreinte, et celle de chaque fictive qu'elle a posee, par nom. Une pose restee en partie (un dossier qui ne s'est pas
/// deplace) sur un poste qui n'avait aucune livraison ne retient pas d'empreinte : la suivante se retelecharge.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Livraison {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empreinte: Option<Empreinte>,
    pub posees: std::collections::BTreeMap<NomDeFichier, Empreinte>,
}

/// LA MARQUE D'UNE FICTIVE LIVREE (`fictives/<nom>/.livree.json`) : l'empreinte de son contenu a la livraison.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Livree {
    pub empreinte: Empreinte,
}

/// Ce qu'un dossier `fictives/` retient de sa livraison (`LIVRAISON`), ou `None` (une installation qui n'a recu
/// aucune livraison, ou posee par l'installeur Windows, qui ne la porte pas). Une livraison qui ne se lit pas se refuse.
fn livraison_retenue(fictives: &Path) -> R<Option<Livraison>> {
    lire_forme(&fictives.join(LIVRAISON), "La livraison des compagnies fictives ne se lit pas.")
}

/// L'empreinte de la livraison de fictives que retient un dossier `fictives/`, ou `None`.
pub fn empreinte_retenue(fictives: &Path) -> R<Option<Empreinte>> {
    Ok(livraison_retenue(fictives)?.and_then(|l| l.empreinte))
}

fn dossier_des_fictives(inst: &Path) -> PathBuf {
    inst.join(crate::installation::DOSSIER_FICTIVES)
}

/// L'archive des fictives deballee dans `cible` (`cible/fictives/...`). Tout ce qu'elle porte est sous `fictives/` et y
/// reste (`enclosed_name`) ; autre chose, et elle est refusee.
fn deballer_les_fictives(octets: &[u8], cible: &Path) -> R<()> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(octets)).map_err(|e| socle::refus!("archive des fictives illisible : {}", e))?;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| refus(e.to_string()))?;
        let rel = f
            .enclosed_name()
            .filter(|r| r.starts_with(crate::installation::DOSSIER_FICTIVES))
            .ok_or_else(|| socle::refus!("archive des fictives suspecte : {}", f.name()))?;
        let chemin = cible.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&chemin).map_err(|e| refus(e.to_string()))?;
            continue;
        }
        if let Some(p) = chemin.parent() {
            std::fs::create_dir_all(p).map_err(|e| refus(e.to_string()))?;
        }
        let mut o = Vec::new();
        f.read_to_end(&mut o).map_err(|e| refus(e.to_string()))?;
        std::fs::write(&chemin, o).map_err(|e| refus(e.to_string()))?;
    }
    Ok(())
}

/// LES FICTIVES SE METTENT A JOUR A PART DU LOGICIEL. `derniere.json` les nomme sous `fictives` (`archive`, `sha256`,
/// `signature`, `empreinte`). Une installation ne les telecharge QUE si leur empreinte n'est pas celle qu'elle retient
/// (`fictives/.livraison.json`) ni celle qui l'attend deja prete : une Release aux memes fictives ne coute rien. Recues,
/// condensat et signature verifies comme une archive du logiciel, et leur `.livraison.json` disant l'empreinte annoncee,
/// elles attendent dans `maj/fictives-pretes/` le prochain lancement (`poser_les_fictives_pretes`). Un DEPOT DE TRAVAIL
/// (l'installation porte `.git` : un clone du depot, ou `fictives/` est suivi par Git et d'ou partent les
/// livraisons) ne les recoit pas. Rend ce qui s'est passe, ou `None` s'il n'y avait rien a faire.
pub fn preparer_les_fictives(inst: &Path, depot: &DepotDesMisesAJour, d: &Publiee, cles: &[&str]) -> Option<String> {
    let f = &d.fictives;
    let empreinte = &f.empreinte;
    let maj = dossier_maj(inst);
    let pretes = maj.join(FICTIVES_PRETES);
    let vise = pretes.join(crate::installation::DOSSIER_FICTIVES);
    if inst.join(".git").exists() {
        return None;
    }
    let court = &empreinte.texte()[..12];
    let deja = || -> R<bool> { Ok(empreinte_retenue(&dossier_des_fictives(inst))?.as_ref() == Some(empreinte) || empreinte_retenue(&vise)?.as_ref() == Some(empreinte)) };
    match deja() {
        Ok(true) => return None,
        Ok(false) => {}
        Err(e) => return Some(format!("Compagnies fictives {} non preparees : {}", court, e.phrase)),
    }
    let essai = || -> R<()> {
        let octets = archive_verifiee(depot, f.signee(), cles, &|_, _| {}).map_err(|(_, e)| e)?;
        let part = maj.join(format!("{}.part", FICTIVES_PRETES));
        socle::fichiers::effacer(&part)?;
        deballer_les_fictives(&octets, &part)?;
        if empreinte_retenue(&part.join(crate::installation::DOSSIER_FICTIVES))?.as_ref() != Some(empreinte) {
            return Err(socle::fichiers::effacer_apres(refus("l'archive des fictives ne dit pas l'empreinte annoncee"), &part));
        }
        socle::fichiers::effacer(&pretes)?;
        std::fs::rename(&part, &pretes).map_err(|e| refus(e.to_string()))
    };
    Some(match essai() {
        Ok(()) => format!("Compagnies fictives {} telechargees, controlees, pretes : elles se poseront au prochain demarrage.", court),
        Err(e) => format!("Compagnies fictives {} non preparees : {}", court, e.phrase),
    })
}

/// LE MARQUEUR D'UNE FICTIVE LIVREE, pour une installation qui ne retient aucune livraison (qui n'en a recu aucune,
/// ou posee par l'installeur Windows) : son `COPIE-DE-DEVELOPPEMENT.json` dit `copie_de` = « compagnie fictive <son nom> »,
/// exactement ce que porte chaque fictive livree (`fictives/<nom>/COPIE-DE-DEVELOPPEMENT.json`). Une fictive copiee et
/// renommee sur le poste dit encore le nom de celle qu'elle copie : ce n'est pas le sien.
fn marquee_comme_livree(dossier: &Path, nom: &str) -> R<bool> {
    Ok(crate::installation::copie_de(dossier)? == Some(format!("{} {}", crate::installation::MARQUE_FICTIVE, nom)))
}

/// AU LANCEMENT, avant toute fenetre (`interface::preparer_l_installation`) : la livraison de fictives prete se pose.
/// Elle ne touche QUE CE QU'ELLE A POSE ELLE-MEME (une fictive faite sur le poste porte les donnees de quelqu'un, elle ne
/// se supprime pas par megarde) : un dossier de `fictives/` est a elle si son nom est dans
/// `posees` de la livraison retenue ET que sa marque (`.livree.json`) dit l'empreinte qu'on y a retenue ; sans livraison
/// retenue, si son nom est celui d'une fictive livree et qu'il porte le marqueur d'une fictive livree
/// (`marquee_comme_livree`). Ce qui est a elle est REMPLACE, joue ou non, photo d'origine (`fictives/.origine/<nom>`)
/// comprise -- elle se reprend a la prochaine ouverture, sur la fictive neuve --
/// ou RETIRE si la nouvelle livraison ne le porte plus. Une fictive nouvelle apparait. Une fictive livree dont le nom est
/// pris par un dossier qui n'est pas a la livraison n'est PAS posee : le dossier reste intact, le journal de la mise a
/// jour le dit, et elle n'entre pas dans `posees`. Tout autre dossier (une copie renommee, une fictive faite sur le
/// poste) et sa photo d'origine ne sont jamais touches, ni rien hors de `fictives/`. Un dossier qui ne se deplace pas (un
/// fichier ouvert) reste tel quel et a la livraison ; l'empreinte retenue reste alors l'ancienne, et le lancement
/// suivant reessaie. Rend ce qui s'est passe, ou `None` s'il n'y avait rien de pret.
pub fn poser_les_fictives_pretes(inst: &Path) -> Option<String> {
    let pretes = dossier_maj(inst).join(FICTIVES_PRETES);
    let neuves = pretes.join(crate::installation::DOSSIER_FICTIVES);
    let poste = dossier_des_fictives(inst);
    let (nouvelle, ancienne) = match (livraison_retenue(&neuves), livraison_retenue(&poste)) {
        (Ok(None), _) => return None,
        (Ok(Some(n)), Ok(a)) => (n, a),
        (Err(e), _) | (_, Err(e)) => return Some(format!("Compagnies fictives non posees : {}", e.phrase)),
    };
    let Some(empreinte) = nouvelle.empreinte.clone() else {
        return Some("Compagnies fictives non posees : la livraison prete ne dit pas son empreinte.".to_string());
    };
    let origines = poste.join(".origine");
    let avant = dossier_maj(inst).join("fictives-avant");
    if let Err(e) = socle::fichiers::effacer(&avant).and_then(|()| std::fs::create_dir_all(&poste).map_err(|e| socle::echec::du_disque(&poste, &e))) {
        return Some(format!("Compagnies fictives non posees : {}", e.phrase));
    }
    let empreinte_de = |a: &Livraison, nom: &NomDeFichier| a.posees.get(nom).cloned();
    // Un dossier est a la livraison si sa marque dit l'empreinte retenue ; une marque qui ne se lit pas se dit, et le
    // dossier reste tel quel.
    let a_la_livraison = |nom: &NomDeFichier| -> R<bool> {
        let d = poste.join(nom);
        Ok(match &ancienne {
            Some(a) => {
                let marque: Option<Livree> = lire_forme(&d.join(LIVREE), "La marque d'une fictive livrée ne se lit pas.")?;
                empreinte_de(a, nom).is_some() && marque.map(|m| m.empreinte) == empreinte_de(a, nom)
            }
            None => marquee_comme_livree(&d, nom.texte())?,
        })
    };
    let ecarter = |nom: &NomDeFichier| -> R<()> {
        let d = poste.join(nom);
        if d.exists() {
            std::fs::create_dir_all(&avant).map_err(|e| refus(e.to_string()))?;
            std::fs::rename(&d, avant.join(nom)).map_err(|e| socle::echec::du_disque(&d, &e))?;
        }
        socle::fichiers::effacer(&origines.join(nom))
    };
    let mut posees = std::collections::BTreeMap::new();
    let mut dit = Vec::new();
    let mut manque = false;
    let noms_neufs: Vec<NomDeFichier> = nouvelle.posees.keys().cloned().collect();
    for nom in &noms_neufs {
        let d = poste.join(nom);
        if d.exists() {
            match a_la_livraison(nom) {
                Ok(true) => {}
                Ok(false) => {
                    dit.push(format!("{} non posee : un dossier de ce nom, que la livraison n'a pas pose, reste tel quel", nom));
                    continue;
                }
                Err(e) => {
                    dit.push(format!("{} non posee : {}", nom, e.phrase));
                    continue;
                }
            }
        }
        let pose = ecarter(nom).and_then(|_| std::fs::rename(neuves.join(nom), &d).map_err(|e| socle::echec::du_disque(&d, &e)));
        match pose {
            Ok(()) => {
                if let Some(e) = nouvelle.posees.get(nom) {
                    posees.insert(nom.clone(), e.clone());
                }
            }
            Err(e) => {
                manque = true;
                dit.push(format!("{} non remplacee ({})", nom, e.phrase));
                if let Some(e) = ancienne.as_ref().and_then(|a| empreinte_de(a, nom)) {
                    posees.insert(nom.clone(), e);
                }
            }
        }
    }
    // Celles que l'ancienne livraison avait posees et que la nouvelle ne porte plus : retirees, si elles sont a elle.
    let anciennes: Vec<NomDeFichier> = ancienne.iter().flat_map(|a| a.posees.keys().cloned()).collect();
    for nom in anciennes.iter().filter(|n| !noms_neufs.contains(n)) {
        if !poste.join(nom).exists() {
            continue;
        }
        match a_la_livraison(nom) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) => {
                dit.push(format!("{} non retiree : {}", nom, e.phrase));
                continue;
            }
        }
        if let Err(e) = ecarter(nom) {
            manque = true;
            dit.push(format!("{} non retiree ({})", nom, e.phrase));
            if let Some(e) = ancienne.as_ref().and_then(|a| empreinte_de(a, nom)) {
                posees.insert(nom.clone(), e);
            }
        } else {
            dit.push(format!("{} retiree", nom));
        }
    }
    // Les fichiers de `fictives/` livres a la racine (`LISEZ-MOI.md`).
    match std::fs::read_dir(&neuves) {
        Ok(entrees) => {
            for e in entrees {
                let copie = e.and_then(|e| {
                    let nom = e.file_name().to_string_lossy().into_owned();
                    if e.file_type()?.is_file() && nom != LIVRAISON {
                        std::fs::copy(e.path(), poste.join(&nom))?;
                    }
                    Ok(())
                });
                if let Err(e) = copie {
                    dit.push(format!("un fichier de la livraison n'est pas copie ({})", e));
                }
            }
        }
        Err(e) => dit.push(format!("les fichiers de la livraison ne se lisent pas ({})", e)),
    }
    let retenue = if manque { ancienne.as_ref().and_then(|a| a.empreinte.clone()) } else { Some(empreinte.clone()) };
    if let Err(e) = ecrire_json(&poste.join(LIVRAISON), &Livraison { empreinte: retenue, posees: posees.clone() }) {
        dit.push(format!("la livraison retenue ne s'ecrit pas ({}) : le lancement suivant la reposera", e.phrase));
    }
    for reste in [&avant, &pretes] {
        if let Err(e) = socle::fichiers::effacer(reste) {
            dit.push(format!("{} ne s'efface pas ({})", reste.display(), e.phrase));
        }
    }
    Some(format!(
        "Compagnies fictives : livraison {} posee ({}){}.",
        &empreinte.texte()[..12],
        posees.keys().map(NomDeFichier::texte).collect::<Vec<_>>().join(", "),
        if dit.is_empty() { String::new() } else { format!(" ; {}", dit.join(" ; ")) }
    ))
}

/// Au lancement, en fond. Ne leve jamais : rend une phrase (ce qui s'est passe), pour le journal de la mise a jour.
pub fn verifier(telecharger_aussi: bool) -> String {
    verifier_avec(&installation(), telecharger_aussi, &cles_de_l_editeur())
}

/// `verifier`, l'installation et les cles publiques acceptees donnees (le moteur livre passe toujours `installation()` et
/// `cles_de_l_editeur()` ; les essais de bout en bout, une installation fictive et des cles d'essai :
/// `tests/fichiers/mise_a_jour.rs`).
pub fn verifier_avec(inst: &Path, telecharger_aussi: bool, cles: &[&str]) -> String {
    verifier_suivi(inst, telecharger_aussi, cles, &Recherche::nouvelle())
}

/// `verifier_avec`, en tenant `suivi` a jour a chaque etape (ce que l'indicateur montre). Ce que la recherche
/// rend (la phrase du journal) et ce qu'elle laisse dans `suivi` disent la meme chose.
pub fn verifier_suivi(inst: &Path, telecharger_aussi: bool, cles: &[&str], suivi: &Recherche) -> String {
    verifier_en(inst, telecharger_aussi, cles, suivi, false)
}

/// `verifier_suivi` ; `calme` : un tour de fond (`chercher_tant_qu_il_tourne`), qui ne change ce que l'indicateur montre
/// que si quelque chose change. Il ne passe pas par « Recherche de mise a jour… », et un depot qui ne repond pas, le temps
/// d'un tour, laisse l'etat tel qu'il etait (« Logiciel a jour ») : le tour suivant redemandera.
/// Une version plus recente trouvee se dit comme au lancement (« Mise a jour en cours », puis prete ou l'echec).
fn verifier_en(inst: &Path, telecharger_aussi: bool, cles: &[&str], suivi: &Recherche, calme: bool) -> String {
    if !calme {
        suivi.poser(Etape::Recherche, None, |_| {});
    }
    let c = logiciel_de(inst);
    let echec = |mot: EchecDeMiseAJour, version: Option<&VersionDuLogiciel>, phrase: String| {
        suivi.poser(Etape::Echec, version, |p| p.mot = Some(mot));
        phrase
    };
    let courante_ici = match courante(&c) {
        Ok(v) => v,
        Err(e) => return echec(EchecDeMiseAJour::Introuvable, None, format!("version installee illisible ({})", e.phrase)),
    };
    let cour = courante_ici.lue();
    let depot = match reglage(inst) {
        Ok(d) => self::depot(d),
        Err(e) if calme => return format!("version {} ({} ; nouvel essai au tour suivant)", cour, e.phrase),
        Err(e) => return echec(EchecDeMiseAJour::ReglageIllisible, None, format!("version {} ({})", cour, e.phrase)),
    };
    let d = match derniere_dite(&depot) {
        Ok(d) => d,
        Err((_, e)) if calme => return format!("version {} (depot injoignable : {} ; nouvel essai au tour suivant)", cour, e.en_lignes()),
        Err((mot, e)) => return echec(mot, None, format!("version {} (depot injoignable : {})", cour, e.en_lignes())),
    };
    // Les fictives, a part du logiciel : seulement si leur livraison a change, a jour ou non (`preparer_les_fictives`).
    if telecharger_aussi {
        if let Some(t) = preparer_les_fictives(inst, &depot, &d, cles) {
            noter(inst, &t);
        }
    }
    let v = d.version.clone();
    if !v.plus_recente_que(&courante_ici) {
        suivi.poser(Etape::AJour, Some(&courante_ici), |p| p.sans_ce_systeme = None);
        return format!("version {}, a jour", cour);
    }
    let lue = v.lue();
    // Publiee sans l'un des systemes de l'installation : rien a telecharger, la version en place reste (une tout-systeme
    // qui ne le porte pas ne se lancerait pas la ; un dossier portable ne perd pas l'un de ses systemes). Pour cette
    // installation, il n'y a pas de mise a jour.
    let systemes = systemes_poses(&c);
    if jeux_a_essayer(&d, &systemes).is_empty() {
        suivi.poser(Etape::AJour, Some(&courante_ici), |p| p.sans_ce_systeme = Some(v.clone()));
        return format!("version {} -- la {} est publiee sans l'un de ses systemes ({}) : cette installation garde sa version", cour, lue, joints(&systemes));
    }
    // Un `prete.json` qui ne se lit pas se remplace : le telechargement qui suit le recrit.
    match prete(inst) {
        Ok(Some(p)) if p.version == v => {
            suivi.prete(&p);
            return format!("version {} -- la {} est prete", cour, lue);
        }
        Ok(_) => {}
        Err(e) => noter(inst, &e.phrase),
    }
    // `moteur maj --sans-telecharger` (le seul appel sans telechargement, sur un suivi que personne ne lit) : la version
    // existe, rien n'est recu. On le dit tel quel, sans pretendre « a jour » ni « prete ».
    if !telecharger_aussi {
        return format!("version {} -- la {} est publiee, pas encore telechargee (`moteur maj` la prepare)", cour, lue);
    }
    match telecharger(inst, &depot, &d, cles, suivi) {
        Ok(p) => {
            suivi.prete(&p);
            let prise = joints(&p.archives);
            format!(
                "version {} -- la {} est telechargee, controlee, prete : elle s'appliquera au prochain demarrage{}",
                cour,
                lue,
                if p.archives == [CleDArchive::Du(Systeme::DE_CE_PROGRAMME)] { String::new() } else { format!(" (archive {})", prise) }
            )
        }
        Err((mot, e)) => echec(mot, Some(&v), format!("version {} -- la {} existe mais n'a pas pu etre preparee : {}", cour, lue, e.en_lignes())),
    }
}

/// LA RECHERCHE EN FOND, DITE A L'ECRAN : en bas a gauche, des l'accueil, « Recherche de mise a jour », « Aucune mise a
/// jour », « Mise a jour en cours » (avec une barre de progression), « Redemarrer pour mettre a jour ». Le moteur tient ou en est la recherche de CE processus (`chercher_en_fond` pose `RECHERCHE`) ;
/// l'accueil et la page d'une compagnie la lisent par le geste `maj` du lanceur (`interface::Fenetre::lanceur`) et
/// n'affichent que ce que `etat` leur dit : la phrase, la barre, et s'il faut relire (`en_cours`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Etape {
    /// Aucune recherche dans ce processus (un `moteur route`, un essai) : rien a montrer.
    Aucune,
    Recherche,
    /// Rien de plus recent pour cette installation.
    AJour,
    /// L'archive arrive : `recus` sur `attendus`.
    Telechargement,
    /// Recue : condensat, signature, deballage, controle sur chaque compagnie.
    Preparation,
    /// Prete : elle s'applique au redemarrage.
    Prete,
    /// Quelque chose n'a pas marche ; `mot` le dit.
    Echec,
}

#[derive(Debug)]
struct Point {
    etape: Etape,
    version: Option<VersionDuLogiciel>,
    recus: u64,
    attendus: Option<u64>,
    /// Ce qui a fait echouer la recherche, a l'etape `Echec` ; vide a toute autre.
    mot: Option<EchecDeMiseAJour>,
    /// La phrase `calcul` de la version prete : ce qu'elle change d'un calcul de paie (vide : rien).
    calcul: String,
    /// La raison `exigee` de la version prete : elle est indispensable avant de declarer (vide : elle ne l'est pas).
    exigee: String,
    /// Ce que la version en place, installee juste avant ce lancement, change d'un calcul de paie
    /// (`dire_l_installation`) : vide, rien a dire. Il tient tout le lancement, quelle que soit l'etape.
    installee: String,
    sans_ce_systeme: Option<VersionDuLogiciel>,
    /// Un echec `HorsLigne` que la recherche reprendra d'elle-meme : quand (`chercher_tant_qu_il_tourne`).
    nouvel_essai: Option<std::time::Instant>,
}

impl Point {
    const fn vide() -> Point {
        Point {
            etape: Etape::Aucune,
            version: None,
            recus: 0,
            attendus: None,
            mot: None,
            calcul: String::new(),
            exigee: String::new(),
            installee: String::new(),
            sans_ce_systeme: None,
            nouvel_essai: None,
        }
    }
}

/// Ce que la page attend entre deux lectures de l'indicateur : pendant une recherche, pendant qu'un nouvel essai est
/// prevu (le reseau peut revenir), et pendant que la recherche de fond tourne (« a jour » : une version
/// trouvee entre deux tours doit se voir sans recharger la page, `RYTHME_A_JOUR`). Une lecture ne fait que lire `RECHERCHE` :
/// aucune recherche, aucun fil. Une version prete, ou un echec definitif : plus rien ne change, elle ne relit pas.
const RELIRE_EN_COURS_MS: u64 = 700;
const RELIRE_EN_ATTENTE_MS: u64 = 10_000;
const RELIRE_EN_FOND_MS: u64 = 30_000;

/// LE RYTHME DE LA RECHERCHE quand le logiciel est a jour : toutes les 5 minutes, pour qu'une version publiee pendant
/// que le logiciel tourne s'y voie.
/// Un tour ne coute qu'une demande de `derniere.json` ; le fil dort entre deux (`chercher_tant_qu_il_tourne`).
pub const RYTHME_A_JOUR: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// LE RYTHME DES NOUVEAUX ESSAIS quand le depot ne repond pas (un logiciel qui demarre
/// sans reseau ne reste pas « hors ligne » une fois le poste connecte). Apres le n-ieme echec, la recherche attend la
/// n-ieme duree, puis la derniere a chaque fois : un essai par 5 minutes au plus lent, jamais plus d'un par 30 secondes.
/// Un essai hors ligne ne coute qu'une demande de `derniere.json` qui echoue ; le fil dort entre deux.
pub const ATTENTES_HORS_LIGNE: [std::time::Duration; 4] = [
    std::time::Duration::from_secs(30),
    std::time::Duration::from_secs(60),
    std::time::Duration::from_secs(120),
    std::time::Duration::from_secs(300),
];

/// L'etat d'une recherche (celle du lancement : `RECHERCHE`).
#[derive(Debug)]
pub struct Recherche(std::sync::Mutex<Point>);

/// La recherche du lancement, celle que l'indicateur montre.
pub static RECHERCHE: Recherche = Recherche::nouvelle();

impl Recherche {
    pub const fn nouvelle() -> Recherche {
        Recherche(std::sync::Mutex::new(Point::vide()))
    }

    /// Passe a `etape` (ce qui tient a l'etape d'avant s'efface, sauf la version si `version` n'en dit pas), puis `plus`.
    fn poser(&self, etape: Etape, version: Option<&VersionDuLogiciel>, plus: impl FnOnce(&mut Point)) {
        let mut p = self.tenu();
        if p.etape != etape {
            let garde = version.cloned().or_else(|| if etape == Etape::AJour { None } else { p.version.take() });
            let installee = std::mem::take(&mut p.installee);
            *p = Point { etape, version: garde, installee, ..Point::vide() };
        } else if let Some(v) = version {
            p.version = Some(v.clone());
        }
        plus(&mut p);
    }

    /// La version `v` est prete (`maj/prete.json`) : elle s'applique au redemarrage ; sa phrase `calcul` et sa raison
    /// `exigee` se gardent pour l'indicateur.
    fn prete(&self, v: &Prete) {
        self.poser(Etape::Prete, Some(&v.version), |p| {
            p.calcul = v.calcul.trim().to_string();
            p.exigee = v.exigee.trim().to_string();
        });
    }

    /// Ce que la version en place, installee juste avant ce lancement, change d'un calcul de paie (`dire_l_installation`).
    fn installee(&self, phrase: String) {
        self.tenu().installee = phrase;
    }

    pub fn etape(&self) -> Etape {
        self.tenu().etape
    }

    /// La recherche a echoue faute de reseau (`HorsLigne`) : c'est le seul echec qu'un nouvel essai peut lever.
    fn hors_ligne(&self) -> bool {
        let p = self.tenu();
        p.etape == Etape::Echec && p.mot == Some(EchecDeMiseAJour::HorsLigne)
    }

    /// Ce que l'indicateur montre, tout decide ici : `etape`, `texte` (vide : rien a montrer), `detail` (la bulle),
    /// `progression` (0 a 1, ou rien : pas de barre ; une barre sans fin connue est `null` avec `barre`), `en_cours`
    /// (une recherche se fait), `relire_ms` (dans combien la page relit : pendant une recherche, pendant qu'un nouvel essai
    /// est prevu, et tant que la recherche de fond tourne ; `null`, elle se tait), `redemarrer` (le clic est offert),
    /// `exigee` (la version prete est indispensable avant de declarer : la phrase, ou `null`), `installee` (ce que la
    /// version en place, installee juste avant ce lancement, change d'un calcul de paie : la phrase, ou `null`).
    /// Tout se dit positivement, par ce qui est vrai (« Logiciel a jour » plutot qu'« Aucune mise a jour »).
    pub fn etat(&self) -> EtatDeLaRecherche {
        let p = self.tenu();
        let lue = p.version.as_ref().map(VersionDuLogiciel::lue);
        let nommee = |t: &str| lue.as_ref().map(|v| format!("PuppetPaie {} {}", v, t));
        let mo = |o: u64| crate::sauvegarde::en_mo(o, ',');
        let essai = p.nouvel_essai.map(|t| {
            let s = t.saturating_duration_since(std::time::Instant::now()).as_secs();
            match s {
                0 => "nouvel essai maintenant".to_string(),
                1..=59 => "nouvel essai dans moins d'une minute".to_string(),
                _ => format!("nouvel essai dans {} min", s.div_ceil(60)),
            }
        });
        let (texte, detail, barre, progression): (&str, Option<String>, bool, Option<Decimal>) = match p.etape {
            Etape::Aucune => ("", None, false, None),
            Etape::Recherche => ("Recherche de mise à jour…", None, false, None),
            Etape::AJour => (
                "Logiciel à jour",
                Some(match (&lue, &p.sans_ce_systeme) {
                    (Some(c), Some(v)) => format!("PuppetPaie {} est la dernière version pour ce système ; la {} est publiée pour les autres.", c, v.lue()),
                    (Some(c), None) => format!("PuppetPaie {} est la dernière version.", c),
                    (None, _) => "Cette version est la dernière.".into(),
                }),
                false,
                None,
            ),
            Etape::Telechargement => (
                "Mise à jour en cours",
                match p.attendus.filter(|a| *a > 0) {
                    Some(a) => nommee(&format!(": {} sur {}", mo(p.recus), mo(a))),
                    None => nommee(&format!(": {} reçus", mo(p.recus))),
                },
                true,
                // La part recue, au millionieme : la page la montre au millieme.
                p.attendus.filter(|a| *a > 0).map(|a| (Decimal::from(p.recus) / Decimal::from(a)).min(Decimal::ONE).round_dp(6)),
            ),
            Etape::Preparation => ("Mise à jour en cours", nommee(": vérification"), true, Some(Decimal::ONE)),
            Etape::Prete => (
                "Redémarrer pour mettre à jour",
                if p.calcul.is_empty() {
                    nommee("est prête : elle s'installe au redémarrage.")
                } else {
                    nommee(&format!("est prête : elle s'installe au redémarrage. {}", phrase_du_calcul(&p.calcul)))
                },
                false,
                None,
            ),
            Etape::Echec => (
                "",
                Some(match (&lue, &essai) {
                    (Some(v), Some(e)) => format!("Cette version reste en service ; PuppetPaie {} se prépare dès que le réseau revient : {}.", v, e),
                    (None, Some(e)) => format!("Cette version reste en service ; la recherche reprend dès que le réseau revient : {}.", e),
                    (Some(v), None) => format!("Cette version reste en service ; PuppetPaie {} se prépare de nouveau au prochain lancement.", v),
                    (None, None) => "Cette version reste en service ; la recherche reprend au prochain lancement.".into(),
                }),
                false,
                None,
            ),
        };
        let texte = match p.mot {
            Some(m) if p.etape == Etape::Echec => format!("Mise à jour : {}", m.mot()),
            _ => texte.to_string(),
        };
        let en_cours = matches!(p.etape, Etape::Recherche | Etape::Telechargement | Etape::Preparation);
        let relire_ms = if en_cours {
            Some(RELIRE_EN_COURS_MS)
        } else if p.etape == Etape::Echec && p.nouvel_essai.is_some() {
            Some(RELIRE_EN_ATTENTE_MS)
        } else if p.etape == Etape::AJour {
            Some(RELIRE_EN_FOND_MS)
        } else {
            None
        };
        EtatDeLaRecherche {
            etape: p.etape,
            texte,
            detail,
            barre,
            progression,
            en_cours,
            relire_ms,
            redemarrer: p.etape == Etape::Prete,
            version: lue,
            // Une version prete EXIGEE : les depots reels la refusent tant qu'elle n'est pas installee
            // (`depot_acte::exigee_en_attente`, qui lit le meme `prete.json`). La phrase se lit sans survol.
            exigee: (p.etape == Etape::Prete && !p.exigee.is_empty())
                .then(|| format!("Elle est indispensable avant de déclarer ({}) : aucun dépôt réel ne part tant qu'elle n'est pas installée.", p.exigee)),
            installee: (!p.installee.is_empty()).then(|| p.installee.clone()),
        }
    }

    /// Le point de la recherche, tenu (`socle::memoire::tenu` : empoisonne, il panique).
    fn tenu(&self) -> std::sync::MutexGuard<'_, Point> {
        socle::memoire::tenu(&self.0)
    }
}

/// CE QUE L'INDICATEUR DE MISE A JOUR MONTRE (`Recherche::etat`).
#[derive(Debug, Clone, Serialize)]
pub struct EtatDeLaRecherche {
    pub etape: Etape,
    pub texte: String,
    pub detail: Option<String>,
    pub barre: bool,
    #[serde(serialize_with = "socle::forme::ecrire_a_virgule_opt")]
    pub progression: Option<Decimal>,
    pub en_cours: bool,
    pub relire_ms: Option<u64>,
    pub redemarrer: bool,
    pub version: Option<String>,
    pub exigee: Option<String>,
    pub installee: Option<String>,
}

/// « Elle change un calcul de paie : … », la phrase `calcul` de « Publier » terminee par un point.
fn phrase_du_calcul(calcul: &str) -> String {
    let c = calcul.trim();
    let point = if c.ends_with(['.', '!', '?', '…']) { "" } else { "." };
    format!("Elle change un calcul de paie : {}{}", c, point)
}

/// LA VERSION QUI VIENT DE S'INSTALLER se dit au lancement qui suit, une fois. La bascule (`appliquer_depuis`) ecrit
/// `maj/appliquee.json` avec la phrase `calcul` et `vue: false` ; lu ici, il passe a `vue: true` et ne se redit plus.
/// Une version qui change un calcul de paie se dit pour tout ce lancement (`Point::installee`) : « PuppetPaie X est en
/// place. Elle change un calcul de paie : … » ; une version sans phrase `calcul` n'a rien de plus a dire que « Logiciel
/// a jour ». Si `vue` ne s'ecrit pas, la phrase se dit quand meme : mieux vaut la relire que ne jamais la lire.
fn dire_l_installation(inst: &Path, suivi: &Recherche) {
    let chemin = dossier_maj(inst).join(APPLIQUEE);
    let mut a = match lire_forme::<Appliquee>(&chemin, "La version installée ne se lit pas : elle ne se dit pas au lancement.") {
        Ok(Some(a)) if !a.vue => a,
        Ok(_) => return,
        Err(e) => return noter(inst, &e.phrase),
    };
    a.vue = true;
    if let Err(e) = ecrire_appliquee(&chemin, &a) {
        noter(inst, &e.phrase);
    }
    let calcul = a.calcul.trim();
    if calcul.is_empty() {
        return;
    }
    suivi.installee(format!("PuppetPaie {} est en place. {}", a.version.lue(), phrase_du_calcul(calcul)));
}

/// `maj/appliquee.json`, que la bascule ecrit : la version installee, quand, ses notes, sa phrase `calcul`, et si elle a
/// deja ete dite au lancement (`vue`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Appliquee {
    version: VersionDuLogiciel,
    #[serde(with = "socle::paris::ecrit::minute")]
    quand: Moment,
    notes: String,
    calcul: String,
    vue: bool,
}

const APPLIQUEE: &str = "appliquee.json";

/// LA SEULE ECRITURE D'`appliquee.json` (`Appliquee`).
fn ecrire_appliquee(chemin: &Path, a: &Appliquee) -> R<()> {
    ecrire_json(chemin, a)
}

/// Le geste « Redemarrer pour mettre a jour » : la version prete part au relais (comme au lancement, `au_lancement`) ; a
/// l'appelant de s'arreter aussitot, sans rien ouvrir (le relais attend sa fin, bascule et ouvre la neuve). Refuse si la
/// recherche de ce processus n'a rien de pret : rien n'est lance.
pub fn redemarrer_pour_mettre_a_jour(inst: &Path, suivi: &Recherche) -> R<VersionDuLogiciel> {
    let v = match a_installer(inst)? {
        Some(v) if suivi.etape() == Etape::Prete => v,
        _ => return Err(refus("Cette version reste en service jusqu'à ce qu'une nouvelle soit prête.")),
    };
    lancer_relais(inst, &v).map_err(|e| {
        noter(inst, &format!("Mise a jour {} non appliquee : {}", v.lue(), e.phrase));
        e.reformuler(|m| format!("La mise à jour ne s'installe pas : {}", m))
    })?;
    noter(inst, &format!("Redemarrage demande pour la mise a jour {}.", v.lue()));
    Ok(v)
}

/// La version prete et plus recente que celle en place : celle qui s'installe au lancement suivant ; ou `None`.
pub fn a_installer(inst: &Path) -> R<Option<VersionDuLogiciel>> {
    match prete(inst)? {
        Some(p) if p.version.plus_recente_que(&courante(&logiciel_de(inst))?) => Ok(Some(p.version)),
        Some(_) | None => Ok(None),
    }
}

fn deplacer(de: &Path, vers: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    if tests::echouer_sur(de) {
        return Err(std::io::Error::other("echec provoque par l'essai"));
    }
    std::fs::rename(de, vers)
}

fn copier(de: &Path, vers: &Path) -> std::io::Result<u64> {
    #[cfg(test)]
    if tests::echouer_sur(de) {
        return Err(std::io::Error::other("echec provoque par l'essai"));
    }
    std::fs::copy(de, vers)
}

/// La bascule : `logiciel/` -> `logiciel/.avant/`, la version prete a sa place ; tout revient si ca casse.
pub fn appliquer(inst: &Path) -> R<Basculee> {
    appliquer_depuis(&dossier_maj(inst), &logiciel_de(inst))
}

/// Ce qu'une bascule a fait : la version en place, ou la precedente est gardee, quand.
#[derive(Debug, Clone, Serialize)]
pub struct Basculee {
    pub version: VersionDuLogiciel,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub avant: PathBuf,
    #[serde(with = "socle::paris::ecrit::minute")]
    pub quand: Moment,
}

/// `appliquer`, le dossier de la version prete et le logiciel donnes : `<installation>/maj/`, ou, pour le relais lance avec
/// `--racine`, le `maj/` de la compagnie ou la version prete est rangee.
fn appliquer_depuis(maj: &Path, code: &Path) -> R<Basculee> {
    let inst = code.parent().map(Path::to_path_buf).ok_or_else(|| refus_avec("Le logiciel n'est dans aucune installation.", a_part_chemin(code)))?;
    let d = prete_dans(maj)?.ok_or_else(|| refus("aucune version prete a appliquer"))?;
    let version = d.version.clone();
    let neuf = maj.join(version.texte()).join("logiciel");
    let avant = code.join(".avant");
    socle::fichiers::effacer(&avant)?;
    std::fs::create_dir_all(&avant).map_err(|e| refus(e.to_string()))?;
    // Les entrees d'un dossier, par nom, sauf ce que la bascule ne touche pas.
    let entrees = |d: &Path| -> R<Vec<std::ffi::OsString>> {
        let mut v = Vec::new();
        for e in std::fs::read_dir(d).map_err(|e| socle::echec::du_disque(d, &e))? {
            let nom = e.map_err(|e| socle::echec::du_disque(d, &e))?.file_name();
            if !NE_PAS_TOUCHER.iter().any(|n| nom == *n) {
                v.push(nom);
            }
        }
        Ok(v)
    };
    let mut deplaces: Vec<std::ffi::OsString> = Vec::new();
    let mut arrives: Vec<std::ffi::OsString> = Vec::new();
    let mut faire = || -> R<()> {
        for nom in entrees(code)? {
            let p = code.join(&nom);
            deplacer(&p, &avant.join(&nom)).map_err(|e| socle::echec::du_disque(&p, &e))?;
            deplaces.push(nom);
        }
        for nom in entrees(&neuf)? {
            let p = neuf.join(&nom);
            deplacer(&p, &code.join(&nom)).map_err(|e| socle::echec::du_disque(&p, &e))?;
            arrives.push(nom);
        }
        for nom in PROPRES_A_L_INSTALLATION {
            if code.join(nom).is_file() {
                std::fs::remove_file(code.join(nom)).map_err(|e| socle::echec::du_disque(&code.join(nom), &e))?;
            }
            if avant.join(nom).is_file() {
                copier(&avant.join(nom), &code.join(nom)).map_err(|e| socle::echec::du_disque(&code.join(nom), &e))?;
            }
        }
        Ok(())
    };
    if let Err(e) = faire() {
        // Tout revient : ce qui est arrive repart, ce qui est parti revient. Ce qui est arrive se suit par son nom, pas par
        // difference avec ce qui est parti (un fichier de la version neuve qui porte le nom d'un fichier de l'ancienne --
        // VERSION, donnees -- resterait en place, et l'ancien ne reviendrait pas : un melange des deux).
        // Ce qui ne revient pas se dit avec l'echec : l'installation est alors melangee.
        let mut pas_revenus = Vec::new();
        for nom in &arrives {
            if let Err(x) = deplacer(&code.join(nom), &neuf.join(nom)) {
                pas_revenus.push(format!("{} ({})", nom.to_string_lossy(), x));
            }
        }
        for nom in &deplaces {
            if avant.join(nom).exists() && !code.join(nom).exists() {
                if let Err(x) = deplacer(&avant.join(nom), &code.join(nom)) {
                    pas_revenus.push(format!("{} ({})", nom.to_string_lossy(), x));
                }
            }
        }
        if !pas_revenus.is_empty() {
            return Err(e.reformuler(|m| format!("la bascule a echoue, et l'ancienne version n'est pas revenue entiere ({}) : {}", pas_revenus.join(", "), m)));
        }
        return Err(e.reformuler(|m| format!("la bascule a echoue, l'ancienne version reste en place : {}", m)));
    }
    let quand = socle::temps::secondes().a_la_minute();
    std::fs::create_dir_all(dossier_maj(&inst)).map_err(|e| refus(e.to_string()))?;
    ecrire_appliquee(&dossier_maj(&inst).join(APPLIQUEE), &Appliquee { version: version.clone(), quand, notes: d.notes, calcul: d.calcul, vue: false })?;
    socle::fichiers::effacer_apres_coup(&maj.join(PRETE));
    socle::fichiers::effacer_apres_coup(&maj.join(version.texte()));
    Ok(Basculee { version, avant, quand })
}

/// Reprendre la version d'avant, gardee dans `.avant/`. Une seule marche arriere. Rend la version reprise.
pub fn revenir(inst: &Path) -> R<VersionDuLogiciel> {
    let code = &logiciel_de(inst);
    let avant = code.join(".avant");
    if !avant.join("donnees").is_dir() {
        return Err(refus_avec("Cette installation n'a pas de version d'avant où revenir.", a_part_chemin(&avant)));
    }
    let version_avant = courante(&avant)?;
    // Ce qui part est DEPLACE dans `.apres/`, puis efface si possible : sous Windows, le programme qui lance `maj
    // --revenir` (`puppetpaie.exe`) ne peut pas s'effacer pendant qu'il tourne, mais il peut se deplacer.
    let apres = code.join(".apres");
    socle::fichiers::effacer(&apres)?;
    std::fs::create_dir_all(&apres).map_err(|e| refus(e.to_string()))?;
    for e in std::fs::read_dir(code).map_err(|e| socle::echec::du_disque(code, &e))? {
        let e = e.map_err(|e| socle::echec::du_disque(code, &e))?;
        let p = e.path();
        if NE_PAS_TOUCHER.iter().any(|n| e.file_name() == *n) {
            continue;
        }
        deplacer(&p, &apres.join(e.file_name())).map_err(|e| socle::echec::du_disque(&p, &e))?;
    }
    for e in std::fs::read_dir(&avant).map_err(|e| socle::echec::du_disque(&avant, &e))? {
        let e = e.map_err(|e| socle::echec::du_disque(&avant, &e))?;
        deplacer(&e.path(), &code.join(e.file_name())).map_err(|e| refus(e.to_string()))?;
    }
    // `.avant/` est vide a present ; `.apres/` porte, sous Windows, le programme qui tourne : il s'efface la fois suivante.
    socle::fichiers::effacer_apres_coup(&avant);
    socle::fichiers::effacer_apres_coup(&apres);
    socle::fichiers::effacer_apres_coup(&dossier_maj(inst).join(APPLIQUEE));
    Ok(version_avant)
}

/// Ce qu'une installation retient de la livraison de fictives qu'elle a : `fictives/.livraison.json`,
/// `{"empreinte": <celle de toute la livraison>, "posees": {<nom>: <empreinte de cette fictive livree>}}`. Un nom qui
/// commence par un point : ni une fictive, ni livre par `fictives_livrees`.
pub const LIVRAISON: &str = ".livraison.json";
/// LA MARQUE D'UNE FICTIVE POSEE PAR UNE LIVRAISON : `fictives/<nom>/.livree.json`, `{"empreinte": …}`, l'empreinte de
/// cette fictive telle que livree. Elle voyage avec la fictive (photo d'origine, remise a zero) ; une copie renommee la
/// porte aussi, mais sous un autre nom. Une livraison ne reconnait comme sien que le dossier dont le NOM est dans
/// `posees` ET dont la marque dit L'EMPREINTE qu'elle y a retenue (`poser_les_fictives_pretes`).
pub const LIVREE: &str = ".livree.json";

/// AU LANCEMENT, avant toute fenetre : une version prete part au relais. Rend `true` si le relais
/// est parti (le programme doit alors s'arreter sans rien ouvrir). Ce qui echoue s'ecrit dans le journal de la mise a
/// jour, et l'ancienne version s'ouvre : une mise a jour ne doit jamais empecher une paie.
pub fn au_lancement() -> bool {
    let inst = installation();
    let v = match a_installer(&inst) {
        Ok(Some(v)) => v,
        Ok(None) => return false,
        Err(e) => {
            noter(&inst, &format!("Mise a jour non appliquee : {}", e.phrase));
            return false;
        }
    };
    match lancer_relais(&inst, &v) {
        Ok(_) => true,
        Err(e) => {
            noter(&inst, &format!("Mise a jour {} non appliquee : {}", v.lue(), e.phrase));
            false
        }
    }
}

/// AU LANCEMENT, apres `au_lancement` : la version qui vient de s'installer se dit (`dire_l_installation`), puis le
/// logiciel cherche pour lui-meme, en fond, sans attendre qu'une compagnie s'ouvre ; ce qu'il trouve s'installe au
/// lancement suivant (ou au clic « Redemarrer pour mettre a jour »). Ce qui s'est passe va dans le journal de la mise a
/// jour ; ou elle en est, dans `RECHERCHE` (l'indicateur). Elle continue tant que le logiciel tourne
/// (`chercher_tant_qu_il_tourne`). Un seul fil par processus : un second appel ne lance rien.
pub fn chercher_en_fond() {
    static LANCEE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if LANCEE.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let inst = installation();
    dire_l_installation(&inst, &RECHERCHE);
    RECHERCHE.poser(Etape::Recherche, None, |_| {});
    std::thread::spawn(move || chercher_tant_qu_il_tourne(&inst, &cles_de_l_editeur(), &RECHERCHE, &ATTENTES_HORS_LIGNE, RYTHME_A_JOUR));
}

/// LA RECHERCHE CONTINUE TANT QUE LE LOGICIEL TOURNE. `verifier_suivi`, puis, selon ce qu'elle laisse dans `suivi` :
/// - un echec `HorsLigne` (le depot ne repond pas, ou la reception a ete coupee) : un nouvel essai apres
///   `attentes[n]` (la derniere se repete : `ATTENTES_HORS_LIGNE`). Le moment du prochain essai est dans `suivi` : la
///   bulle le dit, et la page relit tant qu'il est prevu (`relire_ms`) ;
/// - « a jour » : un tour calme (`verifier_en`) toutes les `rythme` (`RYTHME_A_JOUR`). Une version publiee
///   entre-temps est trouvee, telechargee et preparee ; l'indicateur passe a « Redemarrer pour mettre a jour ». Rien ne
///   s'installe : elle attend le redemarrage ;
/// - « prete », ou un echec qu'un nouvel essai ne leverait pas (signature refusee, archive alteree, depot introuvable) :
///   la boucle s'arrete. Une version prete ne se remplace pas sous le bouton « Redemarrer » : le clic installe celle que
///   l'indicateur nomme, et le lancement d'apres recherche de nouveau (une plus recente s'y trouve en un tour).
///
/// Le journal de la mise a jour recoit chaque tour dont la phrase change (pas une ligne par tour). Bloquante : a lancer
/// dans un fil.
pub fn chercher_tant_qu_il_tourne(inst: &Path, cles: &[&str], suivi: &Recherche, attentes: &[std::time::Duration], rythme: std::time::Duration) {
    let mut n = 0usize;
    let mut calme = false;
    let mut notee = String::new();
    loop {
        let dit = verifier_en(inst, true, cles, suivi, calme);
        if dit != notee {
            noter(inst, &format!("Recherche : {}", dit));
            notee = dit;
        }
        let attente = match suivi.etape() {
            Etape::AJour => {
                n = 0;
                calme = true;
                rythme
            }
            _ if suivi.hors_ligne() => {
                let Some(a) = attentes.get(n).or(attentes.last()).copied() else { return };
                n += 1;
                calme = false;
                suivi.tenu().nouvel_essai = Some(std::time::Instant::now() + a);
                a
            }
            _ => return,
        };
        std::thread::sleep(attente);
    }
}

/// Le journal de la mise a jour (et de ce que le lancement range dans l'installation) : `<installation>/maj/journal.log`,
/// une ligne datee par evenement.
pub fn noter(inst: &Path, t: &str) {
    let d = dossier_maj(inst);
    let chemin = d.join("journal.log");
    let ecrit = std::fs::create_dir_all(&d)
        .and_then(|()| std::fs::OpenOptions::new().create(true).append(true).open(&chemin))
        .and_then(|mut f| writeln!(f, "{} {}", socle::paris::ecrit::minute::texte(socle::temps::secondes()), t));
    // Le journal de la mise a jour n'a personne a qui dire qu'il ne s'ecrit pas : son echec se note
    // (`echec::noter_echec`, que les incidents lisent).
    if let Err(e) = ecrit {
        socle::echec::noter_echec(Some(socle::echec::Classe::du_disque(&e)), &format!("Le journal de la mise à jour ne s'écrit pas ({}) : {}", chemin.display(), e));
    }
}

/// Le programme neuf, copie hors de `logiciel/`, prend le relais ; il attendra la fin de CE processus. Rend `true` si le
/// relais est parti.
pub fn lancer_relais(inst: &Path, version: &VersionDuLogiciel) -> R<bool> {
    let maj = dossier_maj(inst);
    let neuf = maj.join(version.texte()).join("logiciel");
    let source = programme(&neuf, "puppetpaie");
    if !source.is_file() {
        return Err(refus_avec(format!("Le programme de la fenêtre manque dans la version {}.", version), a_part_chemin(&source)));
    }
    let relais = maj.join(format!("relais-{}{}", version, if cfg!(windows) { ".exe" } else { "" }));
    std::fs::copy(&source, &relais).map_err(|e| refus(e.to_string()))?;
    // La fenetre Windows compilee hors MSVC charge WebView2Loader.dll au demarrage : elle doit suivre le relais.
    let chargeur = dossier_programmes(&neuf).join("WebView2Loader.dll");
    if cfg!(windows) && chargeur.is_file() {
        std::fs::copy(&chargeur, maj.join("WebView2Loader.dll")).map_err(|e| refus(e.to_string()))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&relais, std::fs::Permissions::from_mode(0o755)).map_err(|e| socle::echec::du_disque(&relais, &e))?;
    }
    let mut c = std::process::Command::new(&relais);
    c.arg("maj-relais").arg("--installation").arg(inst).arg("--code").arg(logiciel_de(inst)).arg("--attendre").arg(std::process::id().to_string());
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c.spawn().map_err(|e| socle::refus!("le relais ne se lance pas : {}", e))?;
    Ok(true)
}

/// Attend la fin du processus `pid` (60 s au plus). Vrai s'il est parti. Sous Windows, le processus lui-meme (son numero,
/// qui ne se confond avec aucun autre) ; le dossier d'installation n'y sert pas.
#[cfg(windows)]
fn attendre_fin(pid: u32, _inst: &Path) -> R<bool> {
    extern "system" {
        fn OpenProcess(acces: u32, heriter: i32, pid: u32) -> isize;
        fn WaitForSingleObject(objet: isize, millisecondes: u32) -> u32;
        fn CloseHandle(objet: isize) -> i32;
    }
    const SYNCHRONIZE: u32 = 0x0010_0000;
    // SAFETY : appels du noyau sur un numero de processus ; la poignee ouverte est refermee.
    unsafe {
        let h = OpenProcess(SYNCHRONIZE, 0, pid);
        if h == 0 {
            return Ok(true); // deja parti (ou jamais vu : rien a attendre)
        }
        let r = WaitForSingleObject(h, 60_000);
        CloseHandle(h);
        Ok(r == 0)
    }
}

/// Hors Windows, la prise du verrou n'est jamais heritee (CLOEXEC) : quand LA FENETRE DE CETTE INSTALLATION ne repond plus
/// a son creneau (`instance::ouverte`, qui compare le dossier de l'installation), elle est partie. Une autre
/// installation ouverte sur le poste a son propre creneau, et ne se confond jamais avec elle.
#[cfg(not(windows))]
fn attendre_fin(_pid: u32, inst: &Path) -> R<bool> {
    for _ in 0..120 {
        if !crate::instance::ouverte(inst, crate::instance::Plage::DU_POSTE)? {
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Ok(false)
}

/// Le code de sortie du relais appele sans `--code`, ou sans `--installation` ni `--racine` : il n'a rien a relayer.
const RELAIS_MAL_APPELE: i32 = 2;

/// Le relais (`puppetpaie maj-relais --installation I --code C --attendre PID`) : attend la fin du programme qui l'a
/// lance, bascule, et lance la fenetre (la neuve, ou l'ancienne si la bascule a echoue). Ecrit ce qui s'est passe dans le
/// journal de la mise a jour. `--racine R` (la compagnie dont `maj/` porte la version prete) au lieu de `--installation` :
/// c'est ainsi que le programme d'une installation posee qui ne connait pas `--installation` lance le relais de la
/// version neuve ; ses arguments sont un contrat avec les installations posees, comme la forme des archives.
pub fn relais(args: &[socle::ligne::Argument]) -> i32 {
    let args = socle::ligne::textes(args);
    let valeur = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let Some(code) = valeur("--code") else { return RELAIS_MAL_APPELE };
    let maj = match (valeur("--installation"), valeur("--racine")) {
        (Some(i), _) => dossier_maj(&i),
        (None, Some(r)) => dossier_maj(&r),
        (None, None) => return RELAIS_MAL_APPELE,
    };
    let Some(inst) = code.parent().map(Path::to_path_buf) else { return RELAIS_MAL_APPELE };
    let pid = valeur("--attendre").and_then(|p| p.to_str().map(str::parse::<u32>));
    let attendu = match pid {
        Some(Ok(p)) => match attendre_fin(p, &inst) {
            Ok(true) => true,
            Ok(false) => {
                noter(&inst, "Mise a jour non appliquee : le programme qui l'a lancee ne s'est pas arrete.");
                false
            }
            Err(e) => {
                noter(&inst, &format!("Mise a jour non appliquee : on ne sait pas si le programme qui l'a lancee s'est arrete ({}).", e.phrase));
                false
            }
        },
        Some(Err(e)) => {
            noter(&inst, &format!("Mise a jour non appliquee : le numero du programme a attendre ne se lit pas ({}).", e));
            false
        }
        None => {
            noter(&inst, "Mise a jour non appliquee : le relais ne dit pas quel programme attendre.");
            false
        }
    };
    if attendu {
        let journal = |t: &str| noter(&inst, t);
        match appliquer_depuis(&maj, &code) {
            Ok(r) => journal(&format!("Mise a jour appliquee : PuppetPaie {} (la precedente est gardee dans .avant/).", r.version.lue())),
            Err(e) => journal(&format!("Mise a jour non appliquee : {}", e.phrase)),
        }
    }
    if let Err(e) = std::process::Command::new(programme(&code, "puppetpaie")).spawn() {
        noter(&inst, &format!("La fenetre ne se relance pas apres la mise a jour ({}) : la rouvrir a la main.", e));
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    thread_local! {
        /// Le chemin dont `deplacer` ou `copier` echoue UNE fois, pour l'essai de la bascule qui casse en route.
        pub(super) static ECHOUER_SUR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
    }

    /// Vrai une seule fois pour le chemin pose dans `ECHOUER_SUR` (le retour en arriere, lui, doit pouvoir le deplacer).
    pub(super) fn echouer_sur(de: &Path) -> bool {
        ECHOUER_SUR.with(|e| {
            let oui = e.borrow().as_deref() == Some(de);
            if oui {
                e.borrow_mut().take();
            }
            oui
        })
    }

    fn arbre(d: &Path, version: &str) {
        std::fs::create_dir_all(d.join("donnees")).unwrap();
        std::fs::create_dir_all(d.join("interface")).unwrap();
        std::fs::write(d.join("VERSION"), format!("{}\n", version)).unwrap();
        std::fs::write(d.join("donnees").join("a.json"), version).unwrap();
        std::fs::write(d.join("interface").join("index.html"), version).unwrap();
    }

    /// La version 2026.11.01 prete dans `<racine>/maj/`, telle que `telecharger` l'ecrit.
    fn poser_la_prete(racine: &Path) {
        let p = Prete { version: VersionDuLogiciel::lire("2026.11.01").unwrap(), quand: Moment::a_paris(socle::date::Jour::nouveau(2026, 11, 1).unwrap(), 9, 0, 0).unwrap(), archives: vec![CleDArchive::Du(Systeme::DE_CE_PROGRAMME)], notes: String::new(), calcul: String::new(), exigee: String::new() };
        ecrire_prete(&racine.join("maj"), &p).unwrap();
    }

    /// Une archive qui porterait un `EDITION.json` ne change pas l'installation : un paquet soude
    /// garde son `EDITION.json` ; une installation sans `EDITION.json` n'en recoit pas (elle reste bridee ou debloquee par son
    /// `.edition.json`, a cote de `logiciel/`, que la bascule ne touche pas).
    #[test]
    fn la_bascule_laisse_l_edition_de_l_installation_comme_elle_etait() {
        for avant in [Some("{\"edition\": \"bride\"}"), None] {
            let base = std::env::temp_dir().join(format!("puppetpaie-maj-edition-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            let (code, racine) = (base.join("logiciel"), base.clone());
            arbre(&code, "2026.10.09");
            if let Some(t) = avant {
                std::fs::write(code.join("EDITION.json"), t).unwrap();
            }
            let neuf = racine.join("maj").join("2026.11.01").join("logiciel");
            arbre(&neuf, "2026.11.01");
            std::fs::write(neuf.join("EDITION.json"), "{\"edition\": \"bride\"}").unwrap();
            poser_la_prete(&racine);
            appliquer(&racine).unwrap();
            assert_eq!(courante(&code).unwrap().texte(), "2026.11.01");
            assert_eq!(std::fs::read_to_string(code.join("EDITION.json")).ok().as_deref(), avant);
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    /// La bascule casse en route, au dernier pas (toute la version neuve est arrivee, l'`EDITION.json` de l'installation ne
    /// se remet pas) : TOUT revient. L'ancienne version est entiere a sa
    /// place, la neuve entiere dans `prive/maj/<version>/` (rien de melange), et l'`EDITION.json` de l'installation est la.
    #[test]
    fn une_bascule_qui_casse_en_route_remet_l_ancienne_entiere() {
        let base = std::env::temp_dir().join(format!("puppetpaie-maj-bascule-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (code, racine) = (base.join("logiciel"), base.clone());
        arbre(&code, "2026.10.09");
        std::fs::write(code.join("EDITION.json"), "{\"edition\": \"bride\"}").unwrap();
        let neuf = racine.join("maj").join("2026.11.01").join("logiciel");
        arbre(&neuf, "2026.11.01");
        poser_la_prete(&racine);
        ECHOUER_SUR.with(|e| *e.borrow_mut() = Some(code.join(".avant").join("EDITION.json")));
        let r = appliquer(&racine);
        ECHOUER_SUR.with(|e| *e.borrow_mut() = None);
        assert!(r.unwrap_err().phrase.starts_with("la bascule a echoue"));
        assert_eq!(courante(&code).unwrap().texte(), "2026.10.09");
        assert_eq!(std::fs::read_to_string(code.join("donnees").join("a.json")).unwrap(), "2026.10.09");
        assert_eq!(std::fs::read_to_string(code.join("interface").join("index.html")).unwrap(), "2026.10.09");
        assert!(code.join("EDITION.json").is_file());
        assert_eq!(courante(&neuf).unwrap().texte(), "2026.11.01");
        assert_eq!(std::fs::read_to_string(neuf.join("donnees").join("a.json")).unwrap(), "2026.11.01");
        assert!(neuf.join("interface").join("index.html").is_file());
        assert!(prete(&racine).unwrap().is_some(), "la version reste prete, pour le prochain demarrage");
        let _ = std::fs::remove_dir_all(&base);
    }

}
