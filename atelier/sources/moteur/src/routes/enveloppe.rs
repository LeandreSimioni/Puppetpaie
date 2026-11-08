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

use serde::de::DeserializeOwned;
use serde::Serialize;
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
/// (`lire`). Le mot de passe du logiciel qu'elle porte est a l'enveloppe (`mot_de_passe`).
#[derive(Clone, Debug)]
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
    pub fn de(v: Value) -> R<Charge> {
        match v {
            Value::Object(_) => Ok(Charge(v)),
            _ => Err(erreur(Classe::MalFormee, "La demande est mal formée : sa charge est un objet {…}.")),
        }
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

    /// LA CHARGE EN JSON, pour une route qui n'est pas encore branchee sur l'enveloppe (son module la lit lui-meme).
    pub fn brute(&self) -> &Value {
        &self.0
    }
}

// ------------------------------------------------------------------------------------------------- la porte

/// CE QU'UNE ROUTE BRANCHEE RECOIT DE LA PORTE, a cote de sa charge : la route servie, et la compagnie sous la
/// configuration du mois de la charge (`Charge::mois`).
pub struct Porte<'a> {
    pub route: Route,
    pub ctx: &'a droit::contexte::Contexte,
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

    /// Ce qu'une route qui n'est pas encore branchee a rendu, en JSON.
    pub fn brut(v: Value) -> Rendu {
        Rendu(v)
    }

    /// Le rendu en JSON, tel que l'ecran et le LLM le lisent.
    pub fn valeur(&self) -> &Value {
        &self.0
    }

    /// Le rendu en JSON, pris.
    pub fn en_valeur(self) -> Value {
        self.0
    }
}

/// Ce qu'une route branchee rend, ecrit (`Rendu::de`) : la fin de chaque bras de son module.
pub fn en_json<T: Dite>(t: T) -> R<Rendu> {
    Rendu::de(&t)
}

// ------------------------------------------------------------------------------------------------- ce qu'elle refuse

/// LE REFUS D'UNE ROUTE, tel que l'ecran et le LLM le lisent : sa classe (absente pour un refus ordinaire), sa phrase
/// en mots, ce que la fiche de la route dit de corriger pour cette classe, et ce qu'il porte a part.
#[derive(Clone, Debug, Serialize)]
pub struct Refus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classe: Option<Classe>,
    pub phrase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corriger: Option<&'static str>,
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub a_part: Map<String, Value>,
}

impl Refus {
    /// Le refus `e` de la route `route` (aucune : la demande ne s'est pas lue) : sa phrase, separee de ce qu'elle porte a
    /// part (`socle::echec::avec`), et ce que la fiche de la route dit de corriger pour sa classe.
    pub fn de(route: Option<Route>, e: Echec) -> Refus {
        let (phrase, a_part) = crate::valeur::separer(&e.message);
        let corriger = match (route, e.classe) {
            (Some(r), Some(c)) => catalogue::catalogue().iter().filter(|f| f.route == r).flat_map(|f| f.refus.iter()).find(|x| x.classe == c).map(|x| x.corriger),
            _ => None,
        };
        Refus { classe: e.classe, phrase, corriger, a_part }
    }

    /// Un refus de classe `classe`, en une phrase, sans rien a part : celui de la fenetre, avant ou autour d'une route.
    pub fn nouveau(classe: Classe, phrase: impl Into<String>) -> Refus {
        Refus { classe: Some(classe), phrase: phrase.into(), corriger: None, a_part: Map::new() }
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
        ReponseEcrite { ok: self.issue.is_ok(), ecrit: self.ecrit, resultat: self.issue.as_ref().ok(), refus: self.issue.as_ref().err() }.serialize(s)
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
    pub fn resultat(&self) -> Option<&Value> {
        self.issue.as_ref().ok()
    }

    /// Le refus, s'il y a lieu.
    pub fn refus(&self) -> Option<&Refus> {
        self.issue.as_ref().err()
    }

    /// Le rendu ou le refus, pris.
    pub fn en_resultat(self) -> Result<Value, Refus> {
        self.issue
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
                let corriger = r.corriger.map(|c| format!("À corriger : {}\n", c)).unwrap_or_default();
                return format!("Refus : {}\n{}", r.phrase, corriger);
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
