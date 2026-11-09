//! Le refus du moteur, et la memoire de l'endroit ou il se construit.
//!
//! Un `Echec` porte sa phrase et, s'il y a lieu, sa classe (`Classe`) que les routes rendent a la page et au LLM. Chaque
//! echec construit se note ici (l'endroit dans le code, le fil, l'instant) : la trace d'un refus de route
//! (`diagnostic::trace_du_refus`) retrouve ou il est ne et ce qui l'a precede sur le meme fil.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, OnceLock};
use crate::temps::maintenant_ms;

/// LES CLASSES D'UN REFUS, chacune une fois, par son nom : celui que la reponse d'une route ecrit (`classe`), que la fiche
/// d'une route nomme pour dire quoi corriger, que le journal et les incidents notent.
macro_rules! classes {
    ($($(#[doc = $d:literal])* $v:ident,)*) => {
        /// LA CLASSE D'UN REFUS, en mots du logiciel. Le code qui refuse la choisit ; celui qui rattrape un refus la
        /// compare, jamais sa phrase.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
        pub enum Classe {
            $($(#[doc = $d])* $v,)*
        }

        impl Classe {
            /// Le nom de la classe, tel que la reponse d'une route et le journal l'ecrivent.
            pub const fn nom(self) -> &'static str {
                match self {
                    $(Classe::$v => stringify!($v),)*
                }
            }
        }
    };
}

classes! {
    /// Le geste est refuse : ce que la phrase dit ; rien n'est ecrit.
    Refuse,
    /// Une donnee manque : le champ de la demande, la fiche ou la ligne que la phrase nomme.
    Manque,
    /// Une valeur ne se lit pas : la date, le nombre ou la donnee que la phrase dit illisible.
    Illisible,
    /// La demande est mal formee : un champ qu'elle ne connait pas, ou d'une autre forme.
    MalFormee,
    /// Une forme que le logiciel ne sait pas remplir (le modele d'un compte du plan comptable).
    FormeNonPortee,
    /// Un calcul ne se fait pas (un nombre hors de ses bornes, une division par zero).
    CalculImpossible,
    /// Un fichier attendu n'est pas sur le disque.
    FichierAbsent,
    /// Le disque refuse un fichier (interdit, un dossier a sa place, plein).
    Disque,
    /// Un XML (un retour d'organisme) ne se lit pas.
    XmlIllisible,
    /// Une archive zip ne se lit pas : ce n'en est pas une.
    ArchiveIllisible,
    /// Une archive zip que le logiciel ne lit pas (sa compression, son chiffrement, sa fin coupee).
    ArchiveNonLue,
    /// Le reseau ne repond pas, ou coupe la reponse.
    Reseau,
    /// Le reseau n'a pas repondu a temps.
    DelaiDepasse,
    /// Le coffre des mots de passe ne s'ouvre pas, ou n'a pas ce qu'on lui demande.
    Coffre,
    /// Le coffre est abime.
    CoffreIllisible,
    /// Un organisme (net-entreprises, l'Urssaf) refuse la demande.
    Organisme,
    /// Un organisme refuse le depot.
    DepotRefuse,
    /// Google refuse (le Drive, l'autorisation).
    Google,
    /// La boite courriel refuse.
    Courriel,
    /// Un courriel est parti, avec un avertissement a lire.
    CourrielAvecAvertissement,
    /// Le serveur de l'editeur (le filet) refuse.
    Filet,
    /// L'acte reel demande le mot de passe du logiciel, tape par un humain.
    PreuveInsuffisante,
    /// La saisie n'est pas finie.
    SaisieIncomplete,
    /// Aucun document a envoyer ou a imprimer.
    AucunDocument,
    /// La fiche porte des anomalies.
    FicheRefusee,
    /// La DSN de ce mois est deja deposee.
    DsnDejaDeposee,
    /// La repetition d'un depot manque.
    RepetitionManquante,
    /// Une modification que la DSN ne declare pas.
    ModificationNonDeclarable,
    /// Un mois qui ne se regularise pas.
    MoisNonRegularisable,
    /// La fabrique des documents refuse.
    FabriqueRefus,
    /// Une version plus recente du logiciel est exigee.
    VersionExigee,
    /// Une DSN faite par l'ancien logiciel.
    DsnDeLAncienLogiciel,
    /// Les compteurs sont verrouilles.
    CompteursVerrouilles,
    /// L'amorcage ne se fait pas.
    AmorcageImpossible,
    /// L'acte reel est refuse ici.
    ActeReelRefuse,
    /// Un modele de contrat incomplet.
    ModeleIncomplet,
    /// Un envoi refuse a une fictive ajoutee (la demo).
    EnvoiBride,
    /// La copie de developpement ne fait pas d'acte reel.
    CopieDeDeveloppement,
    /// L'edition est bridee (la demo).
    EditionBridee,
    /// Aucune boite courriel n'est posee.
    BoiteAbsente,
    /// Le contrat n'a pas de sortie.
    SortieAbsente,
    /// Le numero d'ordre est trop petit.
    OrdreTropPetit,
    /// L'annule et remplace est hors delai.
    AnnuleRemplaceHorsDelai,
    /// L'exercice est clos.
    ExerciceClos,
    /// Plusieurs sauvegardes conviennent.
    SauvegardeAmbigue,
    /// Aucune sauvegarde.
    SauvegardeAbsente,
    /// Le dossier n'est pas vide.
    DossierNonVide,
    /// Un engagement est en saisie.
    EngagementEnSaisie,
    /// Le recalcul d'un mois est refuse.
    RecalculRefuse,
    /// Le logiciel s'est arrete sur la route : un defaut, a signaler.
    DefautDuLogiciel,
    /// Le nom n'est celui d'aucune route.
    RouteInconnue,
    /// La demande ne se lit pas : ce n'est pas du JSON.
    DemandeIllisible,
    /// La page n'est pas reconnue (son jeton).
    NonAutorise,
    /// Le serveur de la fenetre refuse la requete (trop grosse, coupee).
    RequeteRefusee,
}

impl Classe {
    /// La classe d'un fichier que le disque refuse : absent, ou refuse.
    pub fn du_disque(e: &std::io::Error) -> Classe {
        match e.kind() {
            std::io::ErrorKind::NotFound => Classe::FichierAbsent,
            _ => Classe::Disque,
        }
    }
}

impl std::fmt::Display for Classe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.nom())
    }
}

/// Un refus : sa phrase, et sa classe ; `None` pour un refus ordinaire.
#[derive(Debug)]
pub struct Echec {
    pub classe: Option<Classe>,
    pub message: String,
}

pub type R<T> = Result<T, Echec>;

impl Echec {
    /// La classe du refus telle que le journal et les incidents la notent : un refus ordinaire (`None`) s'y nomme
    /// `Exception`.
    pub fn classe_notee(&self) -> &'static str {
        classe_notee(self.classe)
    }
}

/// La classe d'un refus telle que le journal et les incidents la notent : un refus ordinaire (`None`) s'y nomme
/// `Exception`.
pub fn classe_notee(classe: Option<Classe>) -> &'static str {
    match classe {
        Some(c) => c.nom(),
        None => "Exception",
    }
}

impl From<String> for Echec {
    #[track_caller]
    fn from(message: String) -> Self {
        noter_echec(None, &message);
        Echec { classe: None, message }
    }
}

impl From<&str> for Echec {
    #[track_caller]
    fn from(m: &str) -> Self {
        noter_echec(None, m);
        Echec { classe: None, message: m.to_string() }
    }
}

/// Un echec de classe `classe`, son endroit note.
#[track_caller]
pub fn erreur(classe: Classe, message: impl Into<String>) -> Echec {
    let message = message.into();
    noter_echec(Some(classe), &message);
    Echec { classe: Some(classe), message }
}

/// Un echec de classe quelconque (`None` : un refus ordinaire), son endroit note comme pour `erreur`.
#[track_caller]
pub fn noter_et(classe: Option<Classe>, message: impl Into<String>) -> Echec {
    let message = message.into();
    noter_echec(classe, &message);
    Echec { classe, message }
}

// ===================================================================================================== la memoire

/// La fenetre de la memoire : les trois minutes d'avant.
pub const FENETRE_MS: u64 = 180_000;
/// Au plus tant d'echecs gardes.
const ECHECS_MAX: usize = 80;

/// Un echec note : quand, sur quel fil, ou dans le code, sa classe et le debut de sa phrase.
#[derive(Clone, Debug)]
pub struct Echoue {
    pub t: u64,
    pub fil: String,
    pub ou: String,
    pub classe: String,
    pub message: String,
}

fn les_echecs() -> &'static Mutex<VecDeque<Echoue>> {
    static E: OnceLock<Mutex<VecDeque<Echoue>>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(VecDeque::new()))
}


/// Le fil courant, tel que la memoire le nomme.
pub fn fil_courant() -> String {
    let t = std::thread::current();
    match t.name() {
        Some(n) => format!("{} {:?}", n, t.id()),
        None => format!("{:?}", t.id()),
    }
}

/// Un echec vient d'etre construit : son endroit dans le code, garde en memoire (les 80 derniers, trois minutes).
#[track_caller]
pub fn noter_echec(classe: Option<Classe>, message: &str) {
    let l = std::panic::Location::caller();
    noter_echec_a(&format!("{}:{}", l.file(), l.line()), classe.map(Classe::nom), message);
}

/// Un echec ne a l'endroit `ou` (une panique donne le sien), garde en memoire comme `noter_echec`.
pub fn noter_echec_a(ou: &str, classe: Option<&str>, message: &str) {
    let e = Echoue {
        t: maintenant_ms(),
        fil: fil_courant(),
        ou: ou.to_string(),
        classe: match classe {
            Some(c) => c.to_string(),
            None => "-".to_string(),
        },
        message: message.chars().take(300).collect(),
    };
    let mut l = echecs_notes();
    while l.len() >= ECHECS_MAX || l.front().is_some_and(|x| x.t + FENETRE_MS < e.t) {
        l.pop_front();
    }
    l.push_back(e);
}

/// Les echecs notes, du plus ancien au plus recent.
pub fn echecs_notes() -> MutexGuard<'static, VecDeque<Echoue>> {
    crate::memoire::tenu(les_echecs())
}

// ============================================================================================ ce qui va a part

/// Entre la phrase d'un refus et ce qu'il porte a part : un chemin, une cle, un fichier, une donnee pour la page.
pub const A_PART: char = '\u{1e}';

/// CE QUE LE SOCLE MET A PART D'UN REFUS : le chemin d'un fichier, le nom d'un fichier livre, la cle d'une donnee, ce
/// qui a ete lu ou ce que le systeme a dit. Ce qui n'est pas dit ne s'ecrit pas. Une couche du dessus porte a part ce
/// qu'elle a d'autre (l'etat d'un mois, les anomalies d'une fiche) dans sa propre forme (`avec`).
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct APart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chemin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fichier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// UN REFUS QUI PORTE QUELQUE CHOSE A PART. Sa phrase se lit a l'ecran, en mots ; ce qui n'est pas fait pour l'ecran
/// (le chemin d'un fichier, la cle d'une donnee, le nom d'un fichier livre) ou ce que la page lit d'autre (l'etat d'un
/// mois, les anomalies d'une fiche) va dans `plus`, une forme (`APart`, ou celle de la couche qui refuse) qui s'ecrit en
/// objet, que la reponse de la route rend a cote de la phrase (`separer`) : la fenetre l'ouvre, le LLM le lit. Une
/// phrase qu'on prefixe garde ce qu'elle porte.
pub fn avec(phrase: impl Into<String>, plus: impl serde::Serialize) -> String {
    format!("{}{}{}", phrase.into(), A_PART, crate::ecriture::json_en_ligne(&plus))
}

/// La phrase d'un refus, sans ce qu'il porte a part (`avec`) : les morceaux du message, chacun sans l'objet qui
/// l'ouvre.
pub fn phrase_de(message: &str) -> String {
    let mut phrase = String::new();
    for (rang, m) in message.split(A_PART).enumerate() {
        let debut = match (rang, crate::forme::objet_en_tete(m)) {
            (0, _) | (_, None) => 0,
            (_, Some(n)) => n,
        };
        phrase.push_str(&m[debut..]);
    }
    phrase
}

impl Echec {
    /// La phrase du refus, sans ce qu'il porte a part (`phrase_de`).
    pub fn phrase(&self) -> String {
        phrase_de(&self.message)
    }
}

/// UN FICHIER QUE LE DISQUE REFUSE (absent, interdit, illisible, mal forme) : la phrase dit ce qui arrive, en mots ; le
/// chemin et la raison du systeme vont a part (`chemin`, `detail`). Un refus du moteur sur ce que le fichier porte (une
/// phrase, `String`) garde sa phrase, et le chemin va a part.
pub fn du_disque<E: std::fmt::Display + 'static>(chemin: &std::path::Path, e: &E) -> String {
    let a_part = |phrase: &str, detail: Option<String>| {
        avec(phrase, APart { chemin: Some(chemin.to_string_lossy().into_owned()), detail, ..APart::default() })
    };
    let e_ = e as &dyn std::any::Any;
    if let Some(phrase) = e_.downcast_ref::<String>() {
        return a_part(phrase, None);
    }
    let phrase = match e_.downcast_ref::<std::io::Error>().map(std::io::Error::kind) {
        Some(std::io::ErrorKind::NotFound) => "Un fichier de la compagnie manque.",
        Some(std::io::ErrorKind::PermissionDenied) => "Le poste refuse l'accès à un fichier de la compagnie : un autre programme le tient peut-être ouvert.",
        Some(std::io::ErrorKind::StorageFull) => "Le disque est plein : un fichier de la compagnie ne s'écrit pas.",
        Some(_) => "Le disque refuse un fichier de la compagnie.",
        None => "Un fichier de la compagnie ne se lit pas : il est abîmé.",
    };
    a_part(phrase, Some(e.to_string()))
}

/// UNE DONNEE QUI NE SE LIT PAS LA OU ELLE DEVRAIT : `quoi` la dit en mots (« Une paie écrite ne se relit pas. ») ; la cle
/// et ce qui a ete lu vont a part (`cle`, `detail`).
pub fn a_la_cle(quoi: &str, cle: &str, detail: impl std::fmt::Display) -> String {
    let detail = detail.to_string();
    avec(quoi, APart { cle: Some(cle.to_string()), detail: Some(detail).filter(|d| !d.is_empty()), ..APart::default() })
}

/// UNE DONNEE LIVREE AVEC LE LOGICIEL QUI MANQUE OU NE SE LIT PAS (un referentiel de `donnees/`) : rien que la compagnie
/// puisse corriger ; le fichier et la cle vont a part.
pub fn livree(fichier: &str, cle: &str) -> String {
    avec(
        "Une donnée livrée avec le logiciel manque ou ne se lit pas : le signaler au développeur.",
        APart { fichier: Some(fichier.to_string()), cle: Some(cle.to_string()), ..APart::default() },
    )
}

/// UNE DEMANDE MAL FORMEE : la charge d'une route n'a pas, a `cle`, ce qu'il faut (`attendu` : « un texte », « une
/// liste »). La cle va a part.
pub fn charge(cle: &str, attendu: &str) -> String {
    self::cle(format!("La demande est mal formée : {} est attendu.", attendu), cle)
}

/// Un refus en mots, et la cle de la donnee qu'il vise, a part.
pub fn cle(phrase: impl Into<String>, cle: &str) -> String {
    avec(phrase, APart { cle: Some(cle.to_string()), ..APart::default() })
}

/// Une paie ecrite (ce que la validation d'un engagement a garde) qui ne se relit pas : la cle va a part (`a_la_cle`).
pub const PAIE_ILLISIBLE: &str = "Une paie écrite ne se relit pas : revalider l'engagement.";

/// UN FICHIER DE LA COMPAGNIE QUI NE PORTE PAS CE QU'IL DEVRAIT (une forme inattendue) : la phrase en mots, le chemin et ce
/// qui ne va pas a part.
pub fn abime(chemin: &std::path::Path, detail: impl std::fmt::Display) -> String {
    avec(
        "Un fichier de la compagnie ne se lit pas : il est abîmé.",
        APart { chemin: Some(chemin.to_string_lossy().into_owned()), detail: Some(detail.to_string()), ..APart::default() },
    )
}
