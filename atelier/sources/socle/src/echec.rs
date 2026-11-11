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
    /// Le taux AT/MP de l'annee manque : la notification de la CARSAT le dit, la fiche Entreprise le porte.
    TauxAtmpAbsent,
    /// L'effectif d'une annee manque : celui que l'Urssaf a calcule, ou la date de la premiere embauche.
    EffectifAbsent,
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
    /// Un code d'une liste de la norme DSN que la version du fichier n'a pas dans sa liste.
    HorsDeLaVersion,
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

/// UN REFUS : sa classe (`None` pour un refus ordinaire), sa phrase, en mots, pour l'ecran, et ce qu'il porte a part
/// (`APart` : un lieu, ou la forme de la couche qui refuse), dans sa forme, jamais dans la phrase. Il ne
/// se construit que par ses constructeurs (`refus`, `refus_avec`, `erreur`, `noter_et`, ceux de ce module), qui notent
/// l'endroit ou il nait (`noter_echec`).
#[derive(Debug, Clone)]
pub struct Echec {
    pub classe: Option<Classe>,
    pub phrase: String,
    pub a_part: Option<APart>,
}

/// Le resultat d'une fonction qui peut refuser.
pub type R<T> = Result<T, Echec>;

/// Un refus se dit par sa phrase : c'est elle qu'une lecture de serde (`try_from`) rend dans son propre refus.
impl std::fmt::Display for Echec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.phrase)
    }
}

impl Echec {
    /// La classe du refus telle que le journal et les incidents la notent : un refus ordinaire (`None`) s'y nomme
    /// `Exception`.
    pub fn classe_notee(&self) -> &'static str {
        classe_notee(self.classe)
    }

    /// Le meme refus, sa phrase recomposee par `f` (« <ce qu'on faisait> : <la phrase> ») : sa classe et ce qu'il porte a
    /// part restent, l'endroit ou il est ne aussi.
    pub fn reformuler(self, f: impl FnOnce(&str) -> String) -> Echec {
        Echec { phrase: f(&self.phrase), ..self }
    }

    /// Le meme refus, rendu dans la classe `classe` : sa phrase, ce qu'il porte a part et l'endroit ou il est ne restent.
    pub fn reclasser(self, classe: Classe) -> Echec {
        Echec { classe: Some(classe), ..self }
    }

    /// Le refus tel qu'un terminal le dit : sa phrase, puis, sur la ligne suivante, ce qu'il porte a part, en objet.
    pub fn en_lignes(&self) -> String {
        match &self.a_part {
            Some(a) => format!("{}\n{}", self.phrase, crate::ecriture::json_en_ligne(a)),
            None => self.phrase.clone(),
        }
    }

    /// Le meme refus, survenu en lisant le fichier `chemin` : son chemin va a part, s'il n'en porte pas deja un. Un refus
    /// qui porte la forme de sa couche (`APart::Forme`) la garde telle quelle : elle dit ce que sa couche veut dire.
    pub fn au_fichier(mut self, chemin: &std::path::Path) -> Echec {
        let chemin = || Some(chemin.to_string_lossy().into_owned());
        match &mut self.a_part {
            None => self.a_part = Some(APart::Lieu(Lieu { chemin: chemin(), ..Lieu::default() })),
            Some(APart::Lieu(a)) => a.chemin = a.chemin.take().or_else(chemin),
            Some(APart::Forme(_)) => {}
        }
        self
    }
}

/// UN REFUS ORDINAIRE, en une phrase, son endroit note.
#[track_caller]
pub fn refus(phrase: impl Into<String>) -> Echec {
    noter_et(None, phrase)
}

/// UN REFUS ORDINAIRE QUI PORTE QUELQUE CHOSE A PART : sa phrase se lit a l'ecran, en mots ; ce qui n'est pas fait pour
/// l'ecran (le chemin d'un fichier, la cle d'une donnee, le nom d'un fichier livre, ce que le systeme a dit) va dans
/// `a_part` (un `Lieu`, ou la forme de la couche qui refuse, `APart::forme`), que la reponse de la route rend a cote de
/// la phrase.
#[track_caller]
pub fn refus_avec(phrase: impl Into<String>, a_part: impl Into<APart>) -> Echec {
    Echec { a_part: Some(a_part.into()), ..noter_et(None, phrase) }
}

/// La classe d'un refus telle que le journal et les incidents la notent : un refus ordinaire (`None`) s'y nomme
/// `Exception`.
pub fn classe_notee(classe: Option<Classe>) -> &'static str {
    match classe {
        Some(c) => c.nom(),
        None => "Exception",
    }
}

/// Un echec de classe `classe`, son endroit note.
#[track_caller]
pub fn erreur(classe: Classe, phrase: impl Into<String>) -> Echec {
    noter_et(Some(classe), phrase)
}

/// Un echec de classe quelconque (`None` : un refus ordinaire), son endroit note comme pour `erreur`.
#[track_caller]
pub fn noter_et(classe: Option<Classe>, phrase: impl Into<String>) -> Echec {
    let phrase = phrase.into();
    noter_echec(classe, &phrase);
    Echec { classe, phrase, a_part: None }
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

/// CE QU'UN REFUS PORTE A PART, ecrit sans etiquette : l'objet rendu est celui de la variante. Un `Lieu` (un chemin, un
/// fichier, une cle, ce qui a ete lu), ou la forme que declare la couche qui refuse (l'etat d'un depot, les anomalies
/// d'une fiche, l'archive alteree), gardee typee jusqu'a la porte, qui l'ecrit une fois ; personne ne la relit.
#[derive(Clone, serde::Serialize)]
#[serde(untagged)]
pub enum APart {
    Lieu(Lieu),
    Forme(#[serde(serialize_with = "forme_ecrite")] std::sync::Arc<dyn erased_serde::Serialize + Send + Sync>),
}

impl APart {
    /// Ce que porte a part la couche qui refuse, dans sa forme declaree.
    pub fn forme(f: impl serde::Serialize + Send + Sync + 'static) -> APart {
        APart::Forme(std::sync::Arc::new(f))
    }

    /// Le lieu que le refus porte a part, s'il en porte un.
    pub fn lieu(&self) -> Option<&Lieu> {
        match self {
            APart::Lieu(l) => Some(l),
            APart::Forme(_) => None,
        }
    }
}

impl From<Lieu> for APart {
    fn from(l: Lieu) -> APart {
        APart::Lieu(l)
    }
}

/// Une forme d'une couche, ecrite telle qu'elle se declare.
fn forme_ecrite<S: serde::Serializer>(f: &std::sync::Arc<dyn erased_serde::Serialize + Send + Sync>, s: S) -> Result<S::Ok, S::Error> {
    erased_serde::serialize(f.as_ref(), s)
}

impl std::fmt::Debug for APart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            APart::Lieu(l) => f.debug_tuple("Lieu").field(l).finish(),
            APart::Forme(_) => f.debug_tuple("Forme").field(&crate::ecriture::json_en_ligne(self)).finish(),
        }
    }
}

/// CE QUE LE SOCLE MET A PART D'UN REFUS : le chemin d'un fichier, le nom d'un fichier livre, la cle d'une donnee, ce
/// qui a ete lu ou ce que le systeme a dit. Ce qui n'est pas dit ne s'ecrit pas.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Lieu {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chemin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fichier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// UN FICHIER QUE LE DISQUE REFUSE (absent, interdit, illisible, mal forme) : la phrase dit ce qui arrive, en mots ; le
/// chemin et la raison du systeme vont a part (`chemin`, `detail`). Un refus du moteur sur ce que le fichier porte (une
/// phrase, `String`) garde sa phrase, et le chemin va a part.
#[track_caller]
pub fn du_disque<E: std::fmt::Display + 'static>(chemin: &std::path::Path, e: &E) -> Echec {
    let a_part = |detail: Option<String>| Lieu { chemin: Some(chemin.to_string_lossy().into_owned()), detail, ..Lieu::default() };
    let e_ = e as &dyn std::any::Any;
    if let Some(phrase) = e_.downcast_ref::<String>() {
        return refus_avec(phrase.as_str(), a_part(None));
    }
    let phrase = match e_.downcast_ref::<std::io::Error>().map(std::io::Error::kind) {
        Some(std::io::ErrorKind::NotFound) => "Un fichier de la compagnie manque.",
        Some(std::io::ErrorKind::PermissionDenied) => "Le poste refuse l'accès à un fichier de la compagnie : un autre programme le tient peut-être ouvert.",
        Some(std::io::ErrorKind::StorageFull) => "Le disque est plein : un fichier de la compagnie ne s'écrit pas.",
        Some(_) => "Le disque refuse un fichier de la compagnie.",
        None => "Un fichier de la compagnie ne se lit pas : il est abîmé.",
    };
    refus_avec(phrase, a_part(Some(e.to_string())))
}

/// UNE DONNEE QUI NE SE LIT PAS LA OU ELLE DEVRAIT : `quoi` la dit en mots (« Une paie écrite ne se relit pas. ») ; la cle
/// et ce qui a ete lu vont a part (`cle`, `detail`).
#[track_caller]
pub fn a_la_cle(quoi: &str, cle: &str, detail: impl std::fmt::Display) -> Echec {
    let detail = detail.to_string();
    refus_avec(quoi, Lieu { cle: Some(cle.to_string()), detail: Some(detail).filter(|d| !d.is_empty()), ..Lieu::default() })
}

/// UNE DONNEE LIVREE AVEC LE LOGICIEL QUI MANQUE OU NE SE LIT PAS (un referentiel de `donnees/`) : rien que la compagnie
/// puisse corriger ; le fichier et la cle vont a part.
#[track_caller]
pub fn livree(fichier: &str, cle: &str) -> Echec {
    refus_avec(
        "Une donnée livrée avec le logiciel manque ou ne se lit pas : le signaler au développeur.",
        Lieu { fichier: Some(fichier.to_string()), cle: Some(cle.to_string()), ..Lieu::default() },
    )
}

/// UNE DEMANDE MAL FORMEE : la charge d'une route n'a pas, a `cle`, ce qu'il faut (`attendu` : « un texte », « une
/// liste »). La cle va a part.
#[track_caller]
pub fn charge(cle: &str, attendu: &str) -> Echec {
    self::cle(format!("La demande est mal formée : {} est attendu.", attendu), cle)
}

/// Un refus en mots, et la cle de la donnee qu'il vise, a part.
#[track_caller]
pub fn cle(phrase: impl Into<String>, cle: &str) -> Echec {
    refus_avec(phrase, Lieu { cle: Some(cle.to_string()), ..Lieu::default() })
}

/// Une paie ecrite (ce que la validation d'un engagement a garde) qui ne se relit pas : la cle va a part (`a_la_cle`).
pub const PAIE_ILLISIBLE: &str = "Une paie écrite ne se relit pas : revalider l'engagement.";

/// UN FICHIER DE LA COMPAGNIE QUI NE PORTE PAS CE QU'IL DEVRAIT (une forme inattendue) : la phrase en mots, le chemin et ce
/// qui ne va pas a part.
#[track_caller]
pub fn abime(chemin: &std::path::Path, detail: impl std::fmt::Display) -> Echec {
    refus_avec(
        "Un fichier de la compagnie ne se lit pas : il est abîmé.",
        Lieu { chemin: Some(chemin.to_string_lossy().into_owned()), detail: Some(detail.to_string()), ..Lieu::default() },
    )
}
