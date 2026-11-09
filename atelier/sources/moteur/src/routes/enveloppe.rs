//! L'ENVELOPPE D'UNE ROUTE : la forme, declaree une fois, de ce qu'une route recoit (`Demande`, `Charge`), de ce qu'elle
//! rend (`Rendu`, que sa forme `Dite` ecrit) et de ce qu'elle refuse (`Refus`), et la reponse que l'ecran
//! (`/api/<route>`) et le LLM (`moteur route`) lisent, la meme (`Reponse`).
//!
//! Le JSON ne se touche qu'ici, a la porte : la demande se lit en sa forme, la charge en celle de sa route
//! (`Charge::lire`), ce que la route rend s'ecrit une fois (`Rendu::de`), la reponse une fois (`Reponse::en_json`).
//!
//! LA REPONSE :
//! - rendue : `{"ok": true, "ecrit": …, "resultat": {"phrase": "…", …}}` ;
//! - refusee : `{"ok": false, "ecrit": …, "refus": {"classe": "Manque", "phrase": "…", "corriger": "…", "a_part": {…}}}`.
//!
//! `ecrit` dit que la route ecrit ou engage (l'acte de sa fiche) : l'ecran oublie alors ce qu'il a lu, meme apres un
//! refus. La `classe` d'un refus est un mot du logiciel (`socle::echec::Classe`), absente pour un refus ordinaire ;
//! `corriger`, ce que la fiche de la route dit de corriger pour cette classe, quand elle le dit ; `a_part`, ce que le
//! refus porte hors de sa phrase (un chemin, une cle, l'etat d'un mois, les anomalies d'une fiche : `socle::echec::avec`).
//!
//! C'EST LE BORD DU MOTEUR : le seul module des routes ou le JSON se touche (lire la demande, ecrire la reponse, lire les
//! phrases d'un rendu pour les controler a la porte : `Rendu::phrases`). Ailleurs, une route lit sa charge en sa forme
//! et rend la sienne.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use socle::echec::{erreur, Classe, Echec, R};

use super::catalogue::{self, Route};

/// La cle de la charge ou la page et le LLM portent le mot de passe du logiciel tape pour la route. Il est a
/// l'enveloppe, pas a la route : la route le lit par `Charge::mot_de_passe`, sa forme ne le declare pas.
const MOT_DE_PASSE: &str = "motdepasse";

// ------------------------------------------------------------------------------------------------- la demande

/// LA DEMANDE DE `moteur route`, sur l'entree standard : la route, par son nom, et sa charge (un objet, `{}` pour une
/// route qui ne prend rien). La page, elle, nomme la route dans l'adresse (`/api/<route>`) et envoie la charge seule.
pub struct Demande {
    pub route: Route,
    pub charge: Charge,
}

/// La demande telle qu'elle s'ecrit : la route en texte, lue ensuite (`Route::lire`), pour que son refus soit le sien.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DemandeEcrite {
    route: String,
    charge: Charge,
}

impl Demande {
    /// La demande ecrite `texte` : un refus si ce n'est pas du JSON (`DemandeIllisible`), si elle n'a pas sa forme
    /// (`MalFormee`), ou si elle nomme une route qui n'existe pas (`RouteInconnue`, le nom a part).
    pub fn lire(texte: &str) -> R<Demande> {
        let v: Value = serde_json::from_str(texte).map_err(|e| {
            erreur(Classe::DemandeIllisible, socle::echec::avec("La demande ne se lit pas : ce n'est pas du JSON.", socle::echec::APart { detail: Some(e.to_string()), ..Default::default() }))
        })?;
        let d: DemandeEcrite = socle::forme::lire(&v, "La demande est mal formée : elle porte sa route et sa charge.").map_err(|m| erreur(Classe::MalFormee, m))?;
        Ok(Demande { route: route_nommee(&d.route)?, charge: d.charge })
    }
}

/// La route nommee `nom`, ou le refus d'un nom qui n'est celui d'aucune route (`route_inconnue`).
pub fn route_nommee(nom: &str) -> R<Route> {
    Route::lire(nom).ok_or_else(|| route_inconnue(nom))
}

/// Le refus d'un nom qui n'est celui d'aucune route servie ici : le nom va a part.
pub fn route_inconnue(nom: &str) -> Echec {
    erreur(Classe::RouteInconnue, socle::echec::avec(catalogue::ROUTE_INCONNUE, Nomme { route: nom }))
}

/// Ce que le refus d'une route inconnue porte a part : le nom demande.
#[derive(Serialize)]
struct Nomme<'a> {
    route: &'a str,
}

// ------------------------------------------------------------------------------------------------- la charge

/// LA CHARGE D'UNE ROUTE, telle qu'elle arrive a la porte : un objet, qui ne se lit qu'en la forme de sa route
/// (`lire`). Le mot de passe du logiciel qu'elle porte est a l'enveloppe (`mot_de_passe`). Elle s'ecrit telle qu'elle
/// est arrivee (l'exemple d'une fiche, au catalogue).
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct Charge(Value);

impl<'de> serde::Deserialize<'de> for Charge {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Charge, D::Error> {
        Ok(Charge(Value::Object(Map::deserialize(d)?)))
    }
}

impl Charge {
    /// La charge d'une route qui ne prend rien : `{}`.
    pub fn vide() -> Charge {
        Charge(Value::Object(Map::new()))
    }

    /// La charge ecrite en JSON `v` (le corps d'une demande de la page) : un objet, sinon un refus (`MalFormee`).
    fn de(v: Value) -> R<Charge> {
        match v {
            Value::Object(_) => Ok(Charge(v)),
            _ => Err(erreur(Classe::MalFormee, "La demande est mal formée : sa charge est un objet {…}.")),
        }
    }

    /// La charge ecrite depuis sa forme `t` : ce qu'une route du moteur demande a une autre (la veille a l'ouverture).
    pub fn de_forme<T: Serialize>(t: &T) -> R<Charge> {
        Charge::de(socle::ecriture::en_valeur(t))
    }

    /// La charge ecrite en texte (le corps d'une demande de la page ; vide : `{}`), ou le refus de la demande illisible.
    pub fn du_texte(corps: &[u8]) -> R<Charge> {
        if corps.is_empty() {
            return Ok(Charge::vide());
        }
        let texte = std::str::from_utf8(corps).map_err(|e| erreur(Classe::DemandeIllisible, socle::echec::avec("La demande ne se lit pas : ce n'est pas du JSON.", socle::echec::APart { detail: Some(e.to_string()), ..Default::default() })))?;
        Charge::de(socle::forme::lire_texte(texte, "La demande ne se lit pas : ce n'est pas du JSON.").map_err(|m| erreur(Classe::DemandeIllisible, m))?)
    }

    /// LA CHARGE LUE EN SA FORME `T` (`serde`, `deny_unknown_fields`), sans le mot de passe de l'enveloppe : une cle que
    /// la forme ne connait pas, une cle qui manque, une valeur d'une autre forme la refusent (`MalFormee`), la cle
    /// fautive a part ; rien ne s'ecrit.
    pub fn lire<T: DeserializeOwned>(&self) -> R<T> {
        let mut v = self.0.clone();
        if let Value::Object(m) = &mut v {
            m.remove(MOT_DE_PASSE);
        }
        socle::forme::lire(&v, "La demande est mal formée.").map_err(|m| erreur(Classe::MalFormee, m))
    }

    /// Le mot de passe du logiciel tape pour cette route, s'il est donne (un texte non vide).
    pub fn mot_de_passe(&self) -> Option<&str> {
        self.0.get(MOT_DE_PASSE).and_then(Value::as_str).filter(|m| !m.is_empty())
    }

    /// LE MOIS DE LA CHARGE, s'il est donne : une route qui porte un mois se sert sous la configuration de ce mois
    /// (`Contexte::du_mois`). Un mois qui ne se lit pas refuse la route (`Illisible`), rien ne s'ecrit.
    pub fn mois(&self) -> R<Option<socle::mois::Mois>> {
        match self.0.get("mois").and_then(Value::as_str).filter(|m| !m.is_empty()) {
            Some(m) => socle::mois::Mois::lire(m)
                .map(Some)
                .map_err(|e| erreur(Classe::Illisible, socle::echec::a_la_cle("Le mois de la demande ne se lit pas : il s'écrit AAAA-MM, de 01 à 12.", "mois", e))),
            None => Ok(None),
        }
    }

    /// LE TEXTE QUE LA CHARGE PORTE A `cle`, s'il y en a un : un dossier ou un fichier qu'une route lit (le banc d'essai le
    /// recopie dans son scenario, `atelier::scenario`).
    pub fn texte(&self, cle: &str) -> Option<&str> {
        self.0.get(cle).and_then(Value::as_str)
    }

    /// La charge qui porte le texte `texte` a `cle` (le rejeu d'un scenario y nomme la piece qu'il a posee).
    pub fn poser_texte(&mut self, cle: &str, texte: String) {
        if let Value::Object(m) = &mut self.0 {
            m.insert(cle.to_string(), Value::String(texte));
        }
    }

    /// LA CHARGE SANS AUCUN MOT DE PASSE TAPE : la valeur de toute cle `motdepasse…` qui porte un texte, a toute
    /// profondeur, vide. Un scenario du banc la garde ainsi.
    pub fn sans_les_mots_de_passe(&self) -> Charge {
        fn vider(v: &Value) -> Value {
            match v {
                Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), if k.starts_with(MOT_DE_PASSE) && x.is_string() { Value::String(String::new()) } else { vider(x) })).collect()),
                Value::Array(a) => Value::Array(a.iter().map(vider).collect()),
                x => x.clone(),
            }
        }
        Charge(vider(&self.0))
    }

    /// LA CHARGE D'UN PROGRAMME (`moteur route` : un LLM, un script) SANS SES SECRETS : aucun secret ne s'y lit. Le mot
    /// de passe du logiciel de toute route, et ceux d'une route du coffre (`routes::coffre::SECRETS`), en sont retires ;
    /// la route les demande alors elle-meme a un humain.
    pub fn sans_les_secrets(&mut self, route: Route) {
        if let Value::Object(m) = &mut self.0 {
            m.remove(MOT_DE_PASSE);
            if catalogue::dans(super::coffre::ROUTES, route).is_some() {
                for s in super::coffre::SECRETS {
                    m.remove(s);
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------- la porte

/// CE QU'UNE ROUTE BRANCHEE RECOIT DE LA PORTE, a cote de sa charge : la route servie, la compagnie sous la
/// configuration du mois de la charge (`Charge::mois`), et les options de la commande (`--racine`, `--date`).
pub struct Porte<'a> {
    pub route: Route,
    pub ctx: &'a droit::contexte::Contexte,
    pub options: &'a Options,
}

/// LES OPTIONS D'UNE COMMANDE, a cote de sa demande : la compagnie (`--racine`) et l'aujourd'hui de la route (`--date`).
/// La fenetre pose toujours la compagnie ; `moteur route` les lit de sa ligne de commande (`service::options`).
#[derive(Clone, Debug)]
pub struct Options {
    /// La compagnie nommee. Sans elle, `PUPPETPAIE_DONNEES` (`moteur route` lance a la main ou par un test), sinon le
    /// dossier du code (`service::racine_de`) ; une route de l'installation se sert alors sur celle du programme.
    pub racine: Option<std::path::PathBuf>,
    /// Le jour de la route : celui du contexte, et celui de l'horloge qui date ce qu'elle ecrit. Sans lui, aujourd'hui.
    pub date: Option<socle::date::Jour>,
}

impl Options {
    /// Les options d'une route servie sans compagnie (l'accueil, un message au developpeur depuis l'accueil).
    pub fn sans_compagnie() -> Options {
        Options { racine: None, date: None }
    }

    /// Les options d'une route servie sur la compagnie `racine`, au jour de l'horloge.
    pub fn de_la_compagnie(racine: &std::path::Path) -> Options {
        Options { racine: Some(racine.to_path_buf()), date: None }
    }
}

// ------------------------------------------------------------------------------------------------- ce qu'une route rend

/// CE QU'UNE ROUTE BRANCHEE REND : une forme `serde`, qui porte la phrase que l'ecran affiche et que
/// `moteur route --texte` imprime. Chaque reponse en a une ; la forme la dit (`phrase`), l'enveloppe la verifie.
pub trait Dite: Serialize {
    fn phrase(&self) -> &str;
}

/// LE RENDU D'UNE ROUTE, ecrit en JSON une fois, a la porte : un objet dont la `phrase` est en tete.
#[derive(Clone, Debug)]
pub struct Rendu(Value);

impl Rendu {
    /// Ce qu'une route branchee rend (`Dite`), ecrit. Une phrase vide est un defaut du logiciel : la route le refuse.
    pub fn de<T: Dite>(t: &T) -> R<Rendu> {
        if t.phrase().trim().is_empty() {
            return Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : la route n'a pas rendu de phrase."));
        }
        match serde_json::to_value(t) {
            Ok(v @ Value::Object(_)) => Ok(Rendu(v)),
            Ok(_) => Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : la route a rendu autre chose qu'un objet.")),
            Err(e) => Err(erreur(Classe::DefautDuLogiciel, socle::echec::avec("Défaut du logiciel : la réponse de la route ne s'écrit pas.", socle::echec::APart { detail: Some(e.to_string()), ..Default::default() }))),
        }
    }

    /// A-t-il sa phrase, un texte non vide ?
    pub fn a_sa_phrase(&self) -> bool {
        matches!(self.0.get("phrase"), Some(Value::String(p)) if !p.trim().is_empty())
    }

    /// Les cles qu'il porte, pour dire au developpeur ce qu'une reponse sans phrase rendait.
    pub fn cles(&self) -> Vec<&str> {
        match &self.0 {
            Value::Object(o) => o.keys().map(String::as_str).collect(),
            _ => Vec::new(),
        }
    }

    /// Les phrases qu'il porte, a toute profondeur (`phrases_rendues`).
    pub fn phrases(&self) -> Vec<&str> {
        phrases_rendues(&self.0)
    }

    /// La plus longue de ses phrases, en caracteres (`plus_longue_phrase`).
    pub fn plus_longue_phrase(&self) -> usize {
        plus_longue_phrase(&self.0)
    }

    /// Le rendu en JSON, pris.
    fn en_valeur(self) -> Value {
        self.0
    }
}

/// LA PLUS LONGUE DES PHRASES QUE REND UNE ROUTE, en caracteres : la sienne, et celle de chaque ligne qu'elle porte (la
/// `phrase` d'une carte, d'un carre, de la frise d'un engagement), a toute profondeur. Chacune s'affiche dans la place
/// de la route (`Fiche::phrase`).
pub fn plus_longue_phrase(v: &Value) -> usize {
    match v {
        Value::Object(o) => o
            .iter()
            .map(|(k, x)| match (k.as_str(), x) {
                ("phrase", Value::String(p)) => p.chars().count(),
                _ => plus_longue_phrase(x),
            })
            .fold(0, usize::max),
        Value::Array(a) => a.iter().map(plus_longue_phrase).fold(0, usize::max),
        _ => 0,
    }
}

/// LES PHRASES QUE REND UNE ROUTE, celles que l'ecran lit : la `phrase`, les `phrases` et les `aides` de la reponse et de
/// chaque ligne qu'elle porte, a toute profondeur.
pub fn phrases_rendues(v: &Value) -> Vec<&str> {
    let mut out = Vec::new();
    match v {
        Value::Object(o) => {
            for (k, x) in o {
                match (k.as_str(), x) {
                    ("phrase", Value::String(p)) => out.push(p.as_str()),
                    ("phrases", Value::Array(l)) => out.extend(l.iter().filter_map(Value::as_str)),
                    ("aides", Value::Object(a)) => out.extend(a.values().filter_map(Value::as_str)),
                    _ => out.extend(phrases_rendues(x)),
                }
            }
        }
        Value::Array(a) => out.extend(a.iter().flat_map(phrases_rendues)),
        _ => {}
    }
    out
}

/// Ce qu'une route branchee rend, ecrit (`Rendu::de`) : la fin de chaque bras de son module.
pub fn en_json<T: Dite>(t: T) -> R<Rendu> {
    Rendu::de(&t)
}

// ------------------------------------------------------------------------------------------------- ce qu'elle refuse

/// LE REFUS D'UNE ROUTE, tel que l'ecran et le LLM le lisent : sa classe (absente pour un refus ordinaire), sa phrase
/// en mots, ce que la fiche de la route dit de corriger pour cette classe, et ce qu'il porte a part.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classe: Option<Classe>,
    pub phrase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corriger: Option<String>,
    /// Absent quand le refus ne porte rien a part.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a_part: Option<APart>,
}

/// CE QU'UN REFUS PORTE A PART : ce que la couche qui refuse a mis hors de la phrase, ecrit avec elle
/// (`socle::echec::avec`), en sa forme : celle du socle (`socle::echec::APart` : un chemin, un fichier, une cle, un
/// detail), ou celle de la couche qui refuse. Cette forme-ci les reunit toutes, chaque case une fois : la porte la lit
/// entiere, la rend, la fenetre l'ouvre, le LLM la lit. Une cle qu'aucune couche n'ecrit la refuse : c'est un defaut du
/// logiciel (`Refus::de`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct APart {
    /// Un chemin : un fichier ou un dossier de la compagnie ou du poste (le socle, `engagement::stock`, l'administrateur,
    /// le modele d'un contrat).
    #[serde(skip_serializing_if = "Option::is_none")]
    chemin: Option<String>,
    /// Un fichier livre, ou celui que le refus vise (le socle, la mise a jour, le plan comptable, les natures).
    #[serde(skip_serializing_if = "Option::is_none")]
    fichier: Option<String>,
    /// La cle d'une donnee (le socle, le calcul d'un engagement).
    #[serde(skip_serializing_if = "Option::is_none")]
    cle: Option<String>,
    /// Ce qui a ete lu, ou ce que le systeme a dit.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    /// Le nom d'une route qu'aucune fiche ne porte (`route_inconnue`).
    #[serde(skip_serializing_if = "Option::is_none")]
    route: Option<String>,
    /// Le rang d'un engagement dans son fichier (`engagement::stock`).
    #[serde(skip_serializing_if = "Option::is_none")]
    rang: Option<usize>,
    /// L'installation qu'un dossier n'est pas (`compagnie::administrateur`).
    #[serde(skip_serializing_if = "Option::is_none")]
    installation: Option<String>,
    /// Le condensat d'une archive alteree, et celui qu'annonce la version publiee (`compagnie::maj`).
    #[serde(skip_serializing_if = "Option::is_none")]
    condensat: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attendu: Option<String>,
    /// La zone d'un export comptable trop longue (`documents::etats_compta`).
    #[serde(skip_serializing_if = "Option::is_none")]
    zone: Option<String>,
    /// Le format des libelles courts a poser, et combien (`documents::etats_compta`) : la page y mene.
    #[serde(skip_serializing_if = "Option::is_none")]
    libelles_courts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nombre: Option<usize>,
    /// Le marqueur d'un modele de contrat en cause, ce qu'il faut ecrire, ou apres quoi (`documents::contrat`).
    #[serde(skip_serializing_if = "Option::is_none")]
    marqueur: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    a_ecrire: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apres: Option<String>,
    /// Les anomalies d'une fiche refusee, et leur nombre en mots (`routes::fiches`) : la page les montre.
    #[serde(skip_serializing_if = "Option::is_none")]
    anomalies: Option<Vec<declarations::champs::Anomalie>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    erreurs_dit: Option<String>,
    /// Ou en est ce qui refuse un depot (`compagnie::depot_acte`) : la DSN du mois, ou le test du fichier.
    #[serde(skip_serializing_if = "Option::is_none")]
    etat: Option<EtatAPart>,
}

/// L'ETAT QU'UN REFUS DE DEPOT PORTE A PART : celui de la DSN du mois (une seconde DSN normale), ou celui du test du
/// fichier (un depot reel sans test conforme). Chacun se reconnait a ses cases.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum EtatAPart {
    Dsn(crate::compagnie::etat_dsn::EtatDsn),
    Test(Box<envois::etats::Repetition>),
}

/// Ce qu'un message de refus porte a part, case par case : la premiere ecrite garde la case (une phrase qu'on prefixe
/// garde ce qu'elle porte).
macro_rules! reunir {
    ($a:ident, $b:ident, $($c:ident),*) => {
        $( if $a.$c.is_none() { $a.$c = $b.$c; } )*
    };
}

impl APart {
    /// Le refus ne porte-t-il rien a part ?
    pub fn est_vide(&self) -> bool {
        socle::ecriture::json_en_ligne(self) == "{}"
    }

    /// Ce que porte aussi `autre`, la ou rien n'est deja dit.
    fn reunir(&mut self, autre: APart) {
        reunir!(self, autre, chemin, fichier, cle, detail, route, rang, installation, condensat, attendu, zone, libelles_courts, nombre, marqueur, a_ecrire, apres, anomalies, erreurs_dit, etat);
    }
}

/// LE MESSAGE D'UN REFUS SEPARE : sa phrase, et ce qu'il porte a part, lu en sa forme (`socle::echec::avec` ecrit l'un
/// apres l'autre, un caractere `A_PART` entre eux). Ce qui ne se lit pas en sa forme se dit (le refus de la lecture).
fn separer(message: &str) -> (String, Result<APart, String>) {
    let mut morceaux = message.split(socle::echec::A_PART);
    let mut phrase = String::new();
    let mut a_part = Ok(APart::default());
    if let Some(tete) = morceaux.next() {
        phrase.push_str(tete);
    }
    for m in morceaux {
        match socle::forme::objet_en_tete(m) {
            Some(n) => {
                let lu = socle::forme::lire_texte::<APart>(&m[..n], "Ce qu'un refus porte à part ne se lit pas.");
                a_part = match (a_part, lu) {
                    (Ok(mut a), Ok(b)) => {
                        a.reunir(b);
                        Ok(a)
                    }
                    (Err(e), _) | (Ok(_), Err(e)) => Err(e),
                };
                phrase.push_str(&m[n..]);
            }
            None => phrase.push_str(m),
        }
    }
    (phrase, a_part)
}

impl Refus {
    /// Le refus `e` de la route `route` (aucune : la demande ne s'est pas lue) : sa phrase, separee de ce qu'elle porte a
    /// part (`socle::echec::avec`), et ce que la fiche de la route dit de corriger pour sa classe.
    ///
    /// Ce qu'il porte a part et qui ne se lit pas en sa forme (`APart`) est un defaut du logiciel : le refus le dit, sa
    /// phrase gardee, la raison a part.
    pub fn de(route: Option<Route>, e: Echec) -> Refus {
        let (phrase, a_part) = separer(&e.message);
        let (classe, phrase, a_part) = match a_part {
            Ok(a) => (e.classe, phrase, Some(a).filter(|a| !a.est_vide())),
            Err(illisible) => (
                Some(Classe::DefautDuLogiciel),
                format!("Défaut du logiciel : ce que ce refus porte à part ne se lit pas. {}", phrase),
                Some(APart { detail: Some(socle::echec::phrase_de(&illisible)), ..APart::default() }),
            ),
        };
        let corriger = match (route, classe) {
            (Some(r), Some(c)) => catalogue::catalogue().iter().filter(|f| f.route == r).flat_map(|f| f.refus.iter()).find(|x| x.classe == c).map(|x| x.corriger.to_string()),
            _ => None,
        };
        Refus { classe, phrase, corriger, a_part }
    }

    /// Un refus de classe `classe`, en une phrase, sans rien a part : celui de la fenetre, avant ou autour d'une route.
    pub fn nouveau(classe: Classe, phrase: impl Into<String>) -> Refus {
        Refus { classe: Some(classe), phrase: phrase.into(), corriger: None, a_part: None }
    }
}

// ------------------------------------------------------------------------------------------------- la reponse

/// LA REPONSE D'UNE ROUTE, la meme pour l'ecran et le LLM : son rendu ou son refus, et si la route ecrit.
#[derive(Clone, Debug)]
pub struct Reponse {
    ecrit: bool,
    issue: Result<Value, Refus>,
}

/// La reponse telle qu'elle s'ecrit (`Reponse::en_json`).
#[derive(Serialize)]
struct ReponseEcrite<'a> {
    ok: bool,
    ecrit: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    resultat: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refus: Option<&'a Refus>,
}

impl Serialize for Reponse {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        ReponseEcrite { ok: self.issue.is_ok(), ecrit: self.ecrit, resultat: self.resultat(), refus: self.issue.as_ref().err() }.serialize(s)
    }
}

/// La reponse telle qu'elle se relit (une reponse gardee, `etat_garde`) : rendue, elle porte son resultat ; refusee, son
/// refus ; jamais les deux.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReponseLue {
    ok: bool,
    ecrit: bool,
    resultat: Option<Value>,
    refus: Option<Refus>,
}

impl<'de> Deserialize<'de> for Reponse {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Reponse, D::Error> {
        let r = ReponseLue::deserialize(d)?;
        let issue = match (r.ok, r.resultat, r.refus) {
            (true, Some(v @ Value::Object(_)), None) => Ok(v),
            (false, None, Some(refus)) => Err(refus),
            _ => return Err(serde::de::Error::custom("une réponse rendue porte son résultat (un objet), une réponse refusée son refus, et rien d'autre")),
        };
        Ok(Reponse { ecrit: r.ecrit, issue })
    }
}

impl Reponse {
    /// La reponse de la route `route` : ce qu'elle a rendu, ou son refus.
    pub fn de(route: Route, rendu: R<Rendu>) -> Reponse {
        Reponse { ecrit: super::est_une_ecriture(route), issue: rendu.map(Rendu::en_valeur).map_err(|e| Refus::de(Some(route), e)) }
    }

    /// Le refus d'une demande qui n'a pas atteint de route (illisible, mal formee, une route inconnue) : rien ne s'est
    /// ecrit.
    pub fn refusee(refus: Refus) -> Reponse {
        Reponse { ecrit: false, issue: Err(refus) }
    }

    /// Le refus `refus` de la route `route`, deja dit (la fenetre autour de la route, un refus qu'elle rend tel quel).
    pub fn refusee_par(route: Route, refus: Refus) -> Reponse {
        Reponse { ecrit: super::est_une_ecriture(route), issue: Err(refus) }
    }

    /// La route a-t-elle rendu (et non refuse) ?
    pub fn est_rendue(&self) -> bool {
        self.issue.is_ok()
    }

    /// Ce que la route a rendu, s'il y a lieu.
    fn resultat(&self) -> Option<&Value> {
        match &self.issue {
            Ok(v) => Some(v),
            Err(Refus { .. }) => None,
        }
    }

    /// Le refus, s'il y a lieu.
    pub fn refus(&self) -> Option<&Refus> {
        self.issue.as_ref().err()
    }

    /// LE RENDU LU EN SA FORME `T` (ce qu'une route rend, relu par le moteur qui l'a demandee : la fenetre, le banc), ou
    /// le refus. Un rendu qui n'a pas la forme attendue est un defaut du logiciel.
    pub fn lire<T: DeserializeOwned>(&self) -> Result<T, Refus> {
        match &self.issue {
            Ok(v) => socle::forme::lire(v, "Défaut du logiciel : la réponse d'une route ne se relit pas.").map_err(|m| Refus::de(None, erreur(Classe::DefautDuLogiciel, m))),
            Err(r) => Err(r.clone()),
        }
    }

    /// LES TEXTES DE LA LISTE QUE LE RENDU PORTE A `cle` (les pas que l'horloge d'une fictive a joues : `phrases` de
    /// `horloge-rattraper`, `horloge` de `ouverture`) ; aucun pour un refus.
    pub fn textes(&self, cle: &str) -> Vec<String> {
        match &self.issue {
            Ok(v) => v.get(cle).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(String::from).collect(),
            Err(Refus { .. }) => Vec::new(),
        }
    }

    /// Le refus, s'il y a lieu, pris ; le rendu ne se lit pas ici (`lire`).
    pub fn en_refus(self) -> Result<(), Refus> {
        match self.issue {
            Ok(_) => Ok(()),
            Err(r) => Err(r),
        }
    }

    /// La reponse en JSON : ce que `moteur route` imprime et ce que `/api/<route>` rend. Un rendu et un refus sont des
    /// objets et des textes : leur ecriture ne refuse pas.
    pub fn en_json(&self) -> Value {
        match serde_json::to_value(self) {
            Ok(v) => v,
            Err(e) => serde_json::json!({"ok": false, "ecrit": self.ecrit, "refus": {"classe": Classe::DefautDuLogiciel, "phrase": format!("Défaut du logiciel : la réponse ne s'écrit pas ({}).", e)}}),
        }
    }

    /// `moteur route --texte` : ce que l'ecran lit de la reponse, sans la composer, et jamais du JSON. Rendue : la phrase de
    /// la route, puis les lignes de sa liste `phrases` ; refusee : sa phrase, et ce qu'il faut corriger.
    pub fn en_texte(&self, route: &str) -> String {
        let r = match &self.issue {
            Err(r) => {
                return match &r.corriger {
                    Some(c) => format!("Refus : {}\nÀ corriger : {}\n", r.phrase, c),
                    None => format!("Refus : {}\n", r.phrase),
                };
            }
            Ok(r) => r,
        };
        let mut lignes: Vec<String> = r.get("phrase").and_then(Value::as_str).filter(|p| !p.is_empty()).map(String::from).into_iter().collect();
        lignes.extend(r.get("phrases").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(String::from));
        if lignes.is_empty() {
            lignes.push(format!("La route {} n'a pas rendu de phrase.", route));
        }
        lignes.iter().map(|p| format!("{}\n", p)).collect()
    }
}
