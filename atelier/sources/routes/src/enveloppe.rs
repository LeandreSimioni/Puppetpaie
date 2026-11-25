//! L'ENVELOPPE D'UNE ROUTE : la forme, declaree une fois, de ce qu'une route recoit (`Demande`, `Charge`), de ce qu'elle
//! rend (`Rendu`, que sa forme `Dite` ecrit) et de ce qu'elle refuse (`Refus`), et la reponse que l'ecran
//! (`/api/<route>`) et le LLM (`moteur route`) lisent, la meme (`Reponse`).
//!
//! A la porte : la demande se lit en sa forme, la charge en celle de sa route (`Charge::lire`), ce que la route rend
//! s'ecrit une fois (`Rendu::de`), la reponse une fois (`Reponse::en_json`).
//!
//! LA REPONSE :
//! - rendue : `{"ok": true, "ecrit": …, "resultat": {"phrase": "…", …}}` ;
//! - refusee : `{"ok": false, "ecrit": …, "refus": {"classe": "Manque", "phrase": "…", "corriger": "…", "a_part": {…}}}`.
//!
//! `ecrit` dit que la route a pu changer la compagnie et que la page doit relire : toute route dont la fiche ecrit ou
//! engage, quelle que soit son issue ; et une lecture qui a ecrit au journal des envois (un releve qui range des
//! retours). L'ecran oublie alors ce qu'il a lu. La `classe` d'un refus est un mot du logiciel (`socle::echec::Classe`), absente pour un refus ordinaire ;
//! `corriger`, ce que la fiche de la route dit de corriger pour cette classe, quand elle le dit ; `a_part`, ce que le
//! refus porte hors de sa phrase (un chemin, une cle, l'etat d'un mois, les anomalies d'une fiche : `socle::echec::APart`).
//!
//! LA PORTE DU MOTEUR, SANS JSON A ELLE : la demande, la charge et le rendu passent la porte en objets du socle
//! (`socle::forme::Objet`), qui ne se lisent qu'en une forme ; ce que l'enveloppe en lit a cote de la forme de la route
//! (le mot de passe et le mois d'une charge, la phrase d'un rendu et les phrases qu'il porte, pour les controler a la
//! porte : `Rendu::phrases`) se lit aussi en sa forme. Une route lit sa charge en sa forme et rend la sienne.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use socle::echec::{erreur, Classe, Echec, Lieu, R};
use socle::forme::{Objet, PhrasesRendues};
pub use socle::texte::Phrase;

use super::catalogue::{self, Route};

/// La cle de la charge ou la page et le LLM portent le mot de passe du logiciel tape pour la route. Il est a
/// l'enveloppe, pas a la route : la route le lit par `Charge::mot_de_passe`, sa structure ne le porte pas ; sa
/// declaration le dit (`charge! { motdepasse = … ; … }`).
pub(crate) const MOT_DE_PASSE: &str = "motdepasse";

/// Le mot de passe tape, tel que l'enveloppe le lit : un texte, ou rien.
pub(crate) type MotDePasseTape = Option<socle::secret::Secret>;

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
        const MAL_FORMEE: &str = "La demande est mal formée : elle porte sa route et sa charge.";
        let o = Objet::du_texte(texte, "La demande ne se lit pas : ce n'est pas du JSON.", MAL_FORMEE)?;
        let d: DemandeEcrite = o.lire(&[], MAL_FORMEE).map_err(|m| m.reclasser(Classe::MalFormee))?;
        Ok(Demande { route: route_nommee(&d.route)?, charge: d.charge })
    }
}

/// La route nommee `nom`, ou le refus d'un nom qui n'est celui d'aucune route (`route_inconnue`).
pub fn route_nommee(nom: &str) -> R<Route> {
    Route::lire(nom).ok_or_else(|| route_inconnue(nom))
}

/// Le refus d'une demande qui n'est pas du JSON : ce que le lecteur a dit va a part.
#[track_caller]
fn illisible(detail: String) -> Echec {
    Echec { a_part: Some(Lieu { detail: Some(detail), ..Lieu::default() }.into()), ..erreur(Classe::DemandeIllisible, "La demande ne se lit pas : ce n'est pas du JSON.") }
}

/// Le refus d'un nom qui n'est celui d'aucune route servie ici : le nom va a part.
pub fn route_inconnue(nom: &str) -> Echec {
    Echec { a_part: Some(socle::echec::APart::forme(Nomme { route: nom.to_string() })), ..erreur(Classe::RouteInconnue, catalogue::ROUTE_INCONNUE) }
}

/// Ce que le refus d'une route inconnue porte a part : le nom demande.
#[derive(Serialize)]
struct Nomme {
    route: String,
}

// ------------------------------------------------------------------------------------------------- la charge

/// LA CHARGE D'UNE ROUTE, telle qu'elle arrive a la porte : un objet, qui ne se lit qu'en la forme de sa route
/// (`lire`). Le mot de passe du logiciel qu'elle porte est a l'enveloppe (`mot_de_passe`). Elle s'ecrit telle qu'elle
/// est arrivee (l'exemple d'une fiche, au catalogue).
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct Charge {
    objet: Objet,
    /// Le mot de passe que l'objet porte, lu une fois en sa forme a l'entree (`MotDePasseDeLaCharge`).
    #[serde(skip)]
    mot_de_passe: Option<socle::secret::Secret>,
}

/// LE MOT DE PASSE DE L'ENVELOPPE, seul, lu en sa forme : un texte, ou rien. Un texte vide n'est pas un mot de passe : la
/// boite de la page l'envoie vide quand personne n'a tape.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MotDePasseDeLaCharge {
    motdepasse: MotDePasseTape,
}

impl<'de> Deserialize<'de> for Charge {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Charge, D::Error> {
        Charge::de_l_objet(Objet::deserialize(d)?).map_err(|e| serde::de::Error::custom(e.phrase))
    }
}

/// Le refus d'une charge qui n'est pas un objet.
const CHARGE_MAL_FORMEE: &str = "La demande est mal formée : sa charge est un objet {…}.";

impl Charge {
    /// LA CHARGE QU'EST L'OBJET `objet` : son mot de passe lu en sa forme (`MotDePasseDeLaCharge`), puis retire de l'objet
    /// (la charge s'ecrit et se montre sans lui : il ne vit plus que dans `mot_de_passe`) ; une autre valeur
    /// qu'un texte a `motdepasse` la refuse (`MalFormee`).
    #[track_caller]
    fn de_l_objet(mut objet: Objet) -> R<Charge> {
        let lu: MotDePasseDeLaCharge = objet.lire_cles(&[MOT_DE_PASSE], "La demande est mal formée : le mot de passe est un texte.").map_err(|m| m.reclasser(Classe::MalFormee))?;
        objet.retirer(MOT_DE_PASSE);
        Ok(Charge { objet, mot_de_passe: lu.motdepasse.filter(|m| !m.devoiler().is_empty()) })
    }

    /// La charge d'une route qui ne prend rien : `{}`.
    pub fn vide() -> Charge {
        Charge { objet: Objet::sans_cle(), mot_de_passe: None }
    }

    /// La charge ecrite depuis sa forme `t` : ce qu'une route du moteur demande a une autre (la veille a l'ouverture).
    pub fn de_forme<T: Serialize>(t: &T) -> R<Charge> {
        Charge::de_l_objet(socle::ecriture::en_objet(t).ok_or_else(|| erreur(Classe::MalFormee, CHARGE_MAL_FORMEE))?)
    }

    /// La charge ecrite en texte (le corps d'une demande de la page ; vide : `{}`), ou le refus de la demande illisible.
    pub fn du_texte(corps: &[u8]) -> R<Charge> {
        if corps.is_empty() {
            return Ok(Charge::vide());
        }
        let texte = std::str::from_utf8(corps).map_err(|e| illisible(e.to_string()))?;
        Charge::de_l_objet(Objet::du_texte(texte, "La demande ne se lit pas : ce n'est pas du JSON.", CHARGE_MAL_FORMEE)?)
    }

    /// LA CHARGE LUE EN SA FORME `T` (`serde`, `deny_unknown_fields`), sans le mot de passe de l'enveloppe : une cle que
    /// la forme ne connait pas, une cle qui manque, une valeur qui ne se lit pas la refusent (`MalFormee`), la cle
    /// fautive a part ; une valeur qui ne se lit pas porte sa case et la raison (`socle::echec::CaseRefusee`), dont la
    /// route fait sa phrase (`case_nommee`) ; rien ne s'ecrit. Une charge est une saisie : le texte de chaque notion se
    /// range d'abord dans sa forme, comme la saisie d'une fiche (`socle::forme::lire_saisie`).
    pub fn lire<T: DeserializeOwned>(&self) -> R<T> {
        self.objet.lire_saisie(&[MOT_DE_PASSE], "La demande est mal formée.").map_err(|m| m.reclasser(Classe::MalFormee))
    }

    /// LA CHARGE LUE EN SA FORME `T` comme `lire`, sauf sa cle `cle_du_jour`, et le jour du mois que celle-ci dit, lu une
    /// fois dans le mois que la forme porte (`mois_de`) : le numero d'un jour n'existe qu'ecrit
    /// (`socle::forme::Objet::jour_du_mois`). Absent ou nul, aucun jour ; un jour qui n'est pas de ce mois se refuse a sa
    /// case, comme toute valeur qui ne se lit pas.
    pub fn lire_avec_le_jour<T: DeserializeOwned>(&self, cle_du_jour: &str, mois_de: impl Fn(&T) -> socle::mois::Mois) -> R<(T, Option<socle::date::Jour>)> {
        let t: T = self.objet.lire_saisie(&[MOT_DE_PASSE, cle_du_jour], "La demande est mal formée.").map_err(|m| m.reclasser(Classe::MalFormee))?;
        let jour = self.objet.jour_du_mois(cle_du_jour, mois_de(&t), "La demande est mal formée.").map_err(|m| m.reclasser(Classe::MalFormee))?;
        Ok((t, jour))
    }

    /// Le mot de passe du logiciel tape pour cette route, s'il est donne (un texte non vide).
    pub fn mot_de_passe(&self) -> Option<&socle::secret::Secret> {
        self.mot_de_passe.as_ref()
    }

    /// LE MOIS DE LA CHARGE, s'il est donne : une route qui porte un mois se sert sous la configuration de ce mois
    /// (`Contexte::du_mois`). Un mois qui ne se lit pas refuse la route (`Illisible`), rien ne s'ecrit.
    pub fn mois(&self) -> R<Option<socle::mois::Mois>> {
        /// Le mois de la charge, seul, lu par sa forme : absent ou `null`, la charge n'en porte pas.
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct MoisDeLaCharge {
            mois: Option<socle::mois::Mois>,
        }
        self.objet
            .lire_cles::<MoisDeLaCharge>(&["mois"], "Le mois de la demande ne se lit pas : il s'écrit AAAA-MM, de 01 à 12.")
            .map(|l| l.mois)
            .map_err(|e| e.reclasser(Classe::Illisible))
    }

    /// LES SEULES CLES `cles` DE LA CHARGE, LUES EN LEUR FORME `T` (`socle::forme::Objet::lire_cles`) : ce que le banc
    /// d'essai lit d'une charge, quelle que soit sa route (les pieces qu'elle nomme, `scenario`).
    #[cfg(feature = "atelier")]
    pub(crate) fn lire_cles<T: DeserializeOwned>(&self, cles: &[&str]) -> R<T> {
        self.objet.lire_cles(cles, "La demande est mal formée.").map_err(|m| m.reclasser(Classe::MalFormee))
    }

    /// La charge qui porte le texte `texte` a `cle` (le rejeu d'un scenario y nomme la piece qu'il a posee).
    pub(crate) fn poser_texte(&mut self, cle: &str, texte: String) {
        self.objet.poser_texte(cle, texte);
    }

    /// LA CHARGE SANS AUCUN MOT DE PASSE TAPE : la valeur de toute cle `motdepasse…` qui porte un texte, a toute
    /// profondeur, vide. Un scenario du banc la garde ainsi (l'enregistreur, dans l'atelier seulement).
    #[cfg(feature = "atelier")]
    pub(crate) fn sans_les_mots_de_passe(&self) -> Charge {
        Charge { objet: self.objet.vider_les_textes(MOT_DE_PASSE), mot_de_passe: None }
    }

    /// LA CHARGE D'UN PROGRAMME (`moteur route` : un LLM, un script) SANS SES SECRETS : aucun secret ne s'y lit. Le mot
    /// de passe du logiciel de toute route, et ceux d'une route du coffre (`routes::coffre::SECRETS`), en sont retires ;
    /// la route les demande alors elle-meme a un humain.
    pub(crate) fn sans_les_secrets(&mut self, route: Route) {
        self.mot_de_passe = None;
        if catalogue::dans(super::coffre::ROUTES, route).is_some() {
            for s in super::coffre::SECRETS {
                self.objet.retirer(s);
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
    fn phrase(&self) -> &Phrase;
}

/// LE RENDU D'UNE ROUTE, ecrit en JSON une fois, a la porte : un objet dont la `phrase` est en tete, et les phrases
/// qu'il porte, lues une fois en leur forme (`PhrasesRendues`).
#[derive(Clone, Debug)]
pub struct Rendu {
    objet: Objet,
    phrase: Phrase,
    phrases: PhrasesRendues,
}

/// La phrase d'un rendu, seule, lue en sa forme : un texte, exige.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PhraseDuRendu {
    phrase: Phrase,
}

impl Rendu {
    /// Ce qu'une route branchee rend (`Dite`), ecrit, puis relu : sa `phrase` (un texte non vide) et les phrases qu'il
    /// porte (`PhrasesRendues`). Une phrase vide ou absente, une phrase qui n'a pas sa forme sont un defaut du logiciel :
    /// la route le refuse.
    pub fn de<T: Dite>(t: &T) -> R<Rendu> {
        const SANS_PHRASE: &str = "Défaut du logiciel : la route n'a pas rendu de phrase.";
        if t.phrase().est_vide() {
            return Err(erreur(Classe::DefautDuLogiciel, SANS_PHRASE));
        }
        let objet = socle::ecriture::en_objet(t).ok_or_else(|| erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : la route a rendu autre chose qu'un objet."))?;
        let ecrite: PhraseDuRendu = objet.lire_cles(&["phrase"], SANS_PHRASE).map_err(|m| m.reclasser(Classe::DefautDuLogiciel))?;
        if ecrite.phrase.est_vide() {
            return Err(erreur(Classe::DefautDuLogiciel, SANS_PHRASE));
        }
        let phrases = objet.lire(&[], "Défaut du logiciel : une phrase que la route rend n'est pas un texte.").map_err(|m| m.reclasser(Classe::DefautDuLogiciel))?;
        Ok(Rendu { objet, phrase: ecrite.phrase, phrases })
    }

    /// Les phrases qu'il porte, a toute profondeur (`PhrasesRendues`).
    pub fn phrases(&self) -> &PhrasesRendues {
        &self.phrases
    }
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

/// CE QU'UN REFUS PORTE A PART : ce que la couche qui refuse a mis hors de la phrase, porte par le refus
/// (`socle::echec::APart`), en sa forme : celle du socle (`socle::echec::Lieu` : un chemin, un fichier, une cle, un
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
    /// L'installation qu'un dossier n'est pas (`gestes::administrateur`).
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
    /// Ou en est ce qui refuse un depot (`gestes::depot_acte`) : la DSN du mois, ou le test du fichier.
    #[serde(skip_serializing_if = "Option::is_none")]
    etat: Option<EtatAPart>,
}

/// L'ETAT QU'UN REFUS DE DEPOT PORTE A PART : celui de la DSN du mois (une seconde DSN normale), ou celui du test du
/// fichier (un depot reel sans test conforme). Chacun se reconnait a ses cases.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum EtatAPart {
    Dsn(compagnie::etat_dsn::EtatDsn),
    Test(Box<envois::etats::Repetition>),
}

impl APart {
    /// Le refus ne porte-t-il rien a part ?
    pub(crate) fn est_vide(&self) -> bool {
        socle::ecriture::json_en_ligne(self) == "{}"
    }

    /// CE QUE LE REFUS PORTE A PART, `a` (un `Lieu`, ou la forme de la couche qui refuse), lu en cette forme-ci : ecrit
    /// une fois, relu strict. Ce qui ne s'y lit pas est refuse.
    fn de(a: &socle::echec::APart) -> R<APart> {
        socle::forme::relire(a, "Ce qu'un refus porte à part ne se lit pas.")
    }
}

impl Refus {
    /// Le refus `e` de la route `route` (aucune : la demande ne s'est pas lue) : sa phrase, ce qu'il porte a part, lu en
    /// sa forme (`APart::de`), et ce que la fiche de la route dit de corriger pour sa classe.
    ///
    /// Ce qu'il porte a part et qui ne se lit pas en sa forme (`APart`) est un defaut du logiciel : le refus le dit, sa
    /// phrase gardee, la raison a part.
    pub fn de(route: Option<Route>, e: Echec) -> Refus {
        let (classe, phrase, a_part) = match e.a_part.as_ref().map(APart::de) {
            None => (e.classe, e.phrase, None),
            Some(Ok(a)) => (e.classe, e.phrase, Some(a).filter(|a| !a.est_vide())),
            Some(Err(illisible)) => (
                Some(Classe::DefautDuLogiciel),
                format!("Défaut du logiciel : ce que ce refus porte à part ne se lit pas. {}", e.phrase),
                Some(APart { detail: Some(illisible.phrase), ..APart::default() }),
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

/// LA REPONSE D'UNE ROUTE, la meme pour l'ecran et le LLM : son rendu ou son refus, et si la route a ecrit.
#[derive(Clone, Debug)]
pub struct Reponse {
    ecrit: bool,
    issue: Result<Rendue, Refus>,
}

/// CE QU'UNE REPONSE RENDUE PORTE : l'objet de la route, et sa phrase, lue une fois a la porte (`Rendu::de`, ou la
/// relecture d'une reponse gardee) ; on ne la relit plus ensuite.
#[derive(Clone, Debug)]
struct Rendue {
    objet: Objet,
    phrase: Phrase,
}

/// La reponse telle qu'elle s'ecrit (`Reponse::en_json`).
#[derive(Serialize)]
struct ReponseEcrite<'a> {
    ok: bool,
    ecrit: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    resultat: Option<&'a Objet>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refus: Option<&'a Refus>,
}

impl Serialize for Reponse {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        ReponseEcrite { ok: self.est_rendue(), ecrit: self.ecrit, resultat: self.resultat(), refus: self.issue.as_ref().err() }.serialize(s)
    }
}

/// La reponse telle qu'elle se relit (une reponse gardee, `etat_garde`) : rendue, elle porte son resultat ; refusee, son
/// refus ; jamais les deux.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReponseLue {
    ok: bool,
    ecrit: bool,
    resultat: Option<Objet>,
    refus: Option<Refus>,
}

impl<'de> Deserialize<'de> for Reponse {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Reponse, D::Error> {
        let r = ReponseLue::deserialize(d)?;
        let issue = match (r.ok, r.resultat, r.refus) {
            (true, Some(objet), None) => {
                let lue: PhraseDuRendu = objet.lire_cles(&["phrase"], "Une réponse rendue porte sa phrase.").map_err(|e| serde::de::Error::custom(e.phrase))?;
                Ok(Rendue { objet, phrase: lue.phrase })
            }
            (false, None, Some(refus)) => Err(refus),
            _ => return Err(serde::de::Error::custom("une réponse rendue porte son résultat (un objet), une réponse refusée son refus, et rien d'autre")),
        };
        Ok(Reponse { ecrit: r.ecrit, issue })
    }
}

impl Reponse {
    /// La reponse de la route `route` : ce qu'elle a rendu, ou son refus, sans compter le journal des envois (une
    /// lecture que le moteur garde, une route tombee).
    pub fn de(route: Route, rendu: R<Rendu>) -> Reponse {
        Reponse::du_geste(route, rendu, false)
    }

    /// La reponse de la route `route` servie : ce qu'elle a rendu, ou son refus ; `au_journal`, si elle a ecrit au
    /// journal des envois (`envois::journal::ecritures_du_fil`).
    pub fn du_geste(route: Route, rendu: R<Rendu>, au_journal: bool) -> Reponse {
        Reponse::issue_de(route, rendu.map(|r| Rendue { objet: r.objet, phrase: r.phrase }).map_err(|e| Refus::de(Some(route), e)), au_journal)
    }

    /// L'issue `issue` de la route `route`, et si elle a pu ecrire (`ecrit`) : la page oublie alors ce qu'elle garde de
    /// la compagnie, et c'est tout ce que cela coute. Toute route dont la fiche ecrit ou engage, quelle que soit son
    /// issue (elle a pu ecrire avant de refuser, ou de tomber) ; et une lecture qui a ecrit au journal des envois
    /// (`au_journal` : un releve qui range des retours).
    fn issue_de(route: Route, issue: Result<Rendue, Refus>, au_journal: bool) -> Reponse {
        Reponse { ecrit: au_journal || super::est_une_ecriture(route), issue }
    }

    /// Le refus d'une demande qui n'a pas atteint de route (illisible, mal formee, une route inconnue) : rien ne s'est
    /// ecrit.
    pub fn refusee(refus: Refus) -> Reponse {
        Reponse { ecrit: false, issue: Err(refus) }
    }

    /// Le refus `refus` de la route `route`, deja dit (la fenetre autour de la route, un refus qu'elle rend tel quel) :
    /// `ecrit` suit la fiche de la route, comme pour tout refus.
    pub fn refusee_par(route: Route, refus: Refus) -> Reponse {
        Reponse::issue_de(route, Err(refus), false)
    }

    /// La route a-t-elle rendu (et non refuse) ?
    pub fn est_rendue(&self) -> bool {
        self.refus().is_none()
    }

    /// Ce que la route a rendu, s'il y a lieu.
    fn resultat(&self) -> Option<&Objet> {
        match &self.issue {
            Ok(v) => Some(&v.objet),
            Err(Refus { .. }) => None,
        }
    }

    /// Le refus, s'il y a lieu.
    pub fn refus(&self) -> Option<&Refus> {
        self.issue.as_ref().err()
    }

    /// CE QUE LA REPONSE DIT, en une phrase : celle du rendu, ou le refus ; sans rien relire.
    pub(crate) fn issue_dite(&self) -> Result<&Phrase, &Refus> {
        self.issue.as_ref().map(|r| &r.phrase)
    }

    /// LE RENDU LU EN SA FORME `T` (ce qu'une route rend, relu par le moteur qui l'a demandee : la fenetre, le banc), ou
    /// le refus. Un rendu qui n'a pas la forme attendue est un defaut du logiciel.
    pub fn lire<T: DeserializeOwned>(&self) -> Result<T, Refus> {
        match &self.issue {
            Ok(o) => o.objet.lire(&[],"Défaut du logiciel : la réponse d'une route ne se relit pas.").map_err(|m| Refus::de(None, m.reclasser(Classe::DefautDuLogiciel))),
            Err(r) => Err(r.clone()),
        }
    }

    /// LES SEULES CLES `cles` DU RENDU, LUES EN LEUR FORME `T` (`socle::forme::Objet::lire_cles`), ou le refus : ce que
    /// le banc lit d'un rendu dont la forme entiere ne se relit pas (les pas que l'horloge d'une fictive a joues). Un
    /// rendu qui ne les porte pas en leur forme est un defaut du logiciel.
    pub(crate) fn lire_cles<T: DeserializeOwned>(&self, cles: &[&str]) -> Result<T, Refus> {
        match &self.issue {
            Ok(o) => o.objet.lire_cles(cles,"Défaut du logiciel : la réponse d'une route ne se relit pas.").map_err(|m| Refus::de(None, m.reclasser(Classe::DefautDuLogiciel))),
            Err(r) => Err(r.clone()),
        }
    }

    /// Le refus, s'il y a lieu, pris ; le rendu ne se lit pas ici (`lire`).
    pub fn en_refus(self) -> Result<(), Refus> {
        match self.issue {
            Ok(_) => Ok(()),
            Err(r) => Err(r),
        }
    }

    /// La reponse en JSON, sur une ligne : ce que `moteur route` imprime et ce que `/api/<route>` rend. Un rendu et un
    /// refus sont des objets et des textes : leur ecriture ne refuse pas.
    pub fn en_json(&self) -> String {
        socle::ecriture::json_en_ligne(self)
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
            Ok(r) => &r.objet,
        };
        /// Ce que `--texte` lit d'un rendu : sa phrase, et les lignes de sa liste `phrases` quand il en porte une.
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct EnTexte {
            phrase: String,
            phrases: Option<Vec<String>>,
            ensuite: Option<crate::veille::SauvegardeQuiSuit>,
        }
        match r.lire_cles::<EnTexte>(&["phrase", "phrases", "ensuite"], "Défaut du logiciel : la réponse d'une route ne se relit pas.") {
            Ok(t) => {
                // Ce que la reponse dit suivre, que la fenetre fait d'elle-meme : en texte, la route a appeler.
                let ensuite = t.ensuite.map(|s| format!("Ensuite : la route {}.", s.route().nom()));
                std::iter::once(t.phrase).chain(t.phrases.into_iter().flatten()).chain(ensuite).map(|p| format!("{}\n", p)).collect()
            }
            Err(e) => format!("Refus : {} (route {})\n", e.phrase, route),
        }
    }
}
