//! LA LECTURE D'UNE FORME : une donnee lue (un fichier, un bareme, la charge d'une route) est une structure derivee
//! (`serde::Deserialize`), dont chaque type porte `#[serde(deny_unknown_fields)]` (garde par
//! `moteur/tests/fichiers/structure.rs`) : une cle que la forme ne connait pas est refusee. Seule la reponse d'un tiers
//! (`ReponseDUnTiers`, Google) peut porter en plus des cles qu'on ignore a la porte (`lire_reponse_tiers`). `lire` est la
//! seule fonction qui la lit ; son refus est une phrase du moteur, la cle fautive (son chemin dans la donnee) et ce qui ne
//! va pas a part (`echec::Lieu`).
//!
//! C'est un des deux modules du BORD (avec `ecriture`, qui ecrit) : les seuls endroits ou le JSON se manipule sans forme
//! (seul `socle` depend de `serde_json`, `une_crate_ne_prend_qu_a_celles_du_dessous`) ; tout autre module passe par eux.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::echec::{refus_avec, Lieu, Echec, R};
use serde::de::DeserializeOwned;
use serde_json::Value;

// Un refus de lecture se note la ou la forme se lit (`#[track_caller]`), pas ici : aucune fermeture ne le construit.

/// La donnee `v` lue en `T`. `phrase` est celle du refus, pour l'ecran (« Le barème livré ne se lit pas : le signaler au
/// développeur. ») ; le chemin de la cle et le detail vont a part. Elle se lit dans son texte (`lire_texte`) : un nombre
/// n'y passe jamais par un flottant, que la lecture d'une `Value` lui donnerait.
#[track_caller]
pub fn lire<T: DeserializeOwned>(v: &Value, phrase: &str) -> R<T> {
    lire_texte(&v.to_string(), phrase)
}

thread_local! {
    /// Vrai le temps de lire une saisie (`lire_saisie`).
    static EN_SAISIE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// LA PHRASE D'UNE CASE TAPEE QUI SE REFUSE, la seule : la case, nommee comme l'ecran la montre, et pourquoi
/// (« Taux AT/MP : « 1 5 » ne se lit pas : on attend un taux en pourcent… »).
pub fn case_refusee(case: &str, raison: &str) -> String {
    format!("{} : {}", case, raison)
}

/// LA CASE OU UNE SAISIE `v` SE REFUSE AU CHEMIN `chemin` : une faute qui a un chemin se dit par sa case, a tout niveau
/// (une valeur qui ne se lit pas, une cle inconnue ou qui manque dans une case) : la case du changement qui la porte
/// (`{"case": …, "valeur": …}`, celui de la carte), sinon la premiere cle du chemin (une cle inconnue de la charge est
/// au chemin de son nom). Aucune pour une faute a la racine de la saisie (une cle qui manque a la charge).
fn case_au_chemin(v: &Value, chemin: &serde_path_to_error::Path) -> Option<crate::echec::CaseDeLaSaisie> {
    use crate::echec::CaseDeLaSaisie;
    use serde_path_to_error::Segment;
    let mut ici = Some(v);
    let mut premiere: Option<String> = None;
    let mut case: Option<String> = None;
    for s in chemin.iter() {
        if let Some(Value::Object(o)) = ici {
            if let Some(Value::String(c)) = o.get("case") {
                case = Some(c.clone());
            }
        }
        ici = match s {
            Segment::Map { key } => {
                if premiere.is_none() {
                    premiere = Some(key.clone());
                }
                ici.and_then(|x| x.get(key.as_str()))
            }
            Segment::Seq { index } => ici.and_then(|x| x.get(*index)),
            Segment::Enum { .. } | Segment::Unknown => ici,
        };
    }
    match (case, premiere) {
        (Some(c), _) => Some(CaseDeLaSaisie::DUnChangement(c)),
        (None, Some(c)) => Some(CaseDeLaSaisie::Cle(c)),
        (None, None) => None,
    }
}

/// UNE SAISIE LUE (la charge d'une route) : la donnee `v` lue en `T` comme `lire`, le texte de chaque notion qui range sa
/// saisie (`droit::fiche::FormeDeCase::RANGER`) d'abord range dans sa forme (`range_si_saisi`), comme la saisie d'une
/// fiche. Un fichier se lit par `lire`, tel qu'il est ecrit. Une case dont la valeur ne se lit pas (un texte tape qui ne
/// se convertit pas, un nombre hors de sa forme) refuse la saisie en portant sa case, son chemin et la raison
/// (`echec::CaseRefusee`), dont la route fait sa phrase ; une cle exigee qui manque a la racine, de meme, a sa case
/// (`cle_qui_manque`) ; une autre faute de la saisie elle-meme, par `phrase`.
#[track_caller]
pub fn lire_saisie<T: DeserializeOwned>(v: &Value, phrase: &str) -> R<T> {
    let _remise = EnSaisie::ouverte();
    let texte = v.to_string();
    let mut d = serde_json::Deserializer::from_str(&texte);
    let e = match serde_path_to_error::deserialize(&mut d) {
        Ok(t) => return Ok(t),
        Err(e) => e,
    };
    let detail = en_clair(&e.inner().to_string());
    match (case_au_chemin(v, e.path()), cle_qui_manque(&e.inner().to_string())) {
        (Some(case), _) => Err(case_refusee_a(phrase, case, e.path().to_string(), detail)),
        (None, Some(cle)) => Err(case_refusee_a(phrase, crate::echec::CaseDeLaSaisie::Cle(cle.clone()), cle, detail)),
        (None, None) => Err(refus(phrase, None, &e)),
    }
}

/// LA CLE EXIGEE QUI MANQUE A LA RACINE D'UNE SAISIE (« missing field `quoi` ») : la case fautive est elle, que le chemin
/// de la faute (la racine) ne nomme pas.
fn cle_qui_manque(m: &str) -> Option<String> {
    let m = match m.split_once(" at line ") {
        Some((avant, _)) => avant,
        None => m,
    };
    m.strip_prefix("missing field `").and_then(|r| r.strip_suffix('`')).map(str::to_string)
}

/// Le temps d'une lecture d'une saisie (`EN_SAISIE`) : l'etat d'avant revient en sortant, panique comprise ; une lecture
/// de fichier qui suit n'est jamais rangee.
struct EnSaisie(bool);

impl EnSaisie {
    fn ouverte() -> EnSaisie {
        EnSaisie(EN_SAISIE.replace(true))
    }
}

impl Drop for EnSaisie {
    fn drop(&mut self) {
        EN_SAISIE.set(self.0);
    }
}

/// LE REFUS D'UNE SAISIE A SA CASE `case` (au chemin `cle`), pour la raison `detail` : la route en fait sa phrase
/// (`echec::CaseRefusee`), et c'est elle que la memoire des echecs note, la ou la route la fait
/// (`routes::case_nommee`) ; `phrase` est celle d'un appel qui ne la nomme pas.
fn case_refusee_a(phrase: &str, case: crate::echec::CaseDeLaSaisie, cle: String, detail: String) -> Echec {
    Echec { classe: None, phrase: phrase.to_string(), a_part: Some(crate::echec::APart::Case(crate::echec::CaseRefusee { case, cle, detail })) }
}

/// Lit-on une saisie (`lire_saisie`) ?
pub(crate) fn en_saisie() -> bool {
    EN_SAISIE.get()
}

/// LE TEXTE `texte` D'UNE NOTION QUI RANGE SA SAISIE, a sa lecture : range par `ranger` (son `FormeDeCase::RANGER`) dans
/// une saisie (`lire_saisie`), tel quel ailleurs.
pub fn range_si_saisi(texte: String, ranger: fn(&str) -> String) -> String {
    if EN_SAISIE.get() {
        ranger(&texte)
    } else {
        texte
    }
}

/// Le texte JSON `texte` lu en `T` (`lire`, sans passer par une `Value`).
#[track_caller]
pub fn lire_texte<T: DeserializeOwned>(texte: &str, phrase: &str) -> R<T> {
    let mut d = serde_json::Deserializer::from_str(texte);
    match serde_path_to_error::deserialize(&mut d) { Ok(t) => Ok(t), Err(e) => Err(refus(phrase, None, &e)) }
}

/// UNE FORME QUI EST LA REPONSE D'UN TIERS (Google, qui ajoute des cles a ses reponses sans prevenir) : elle se declare
/// telle dans son code, a cote de sa structure, et n'a pas `deny_unknown_fields`. Elle ne se lit que par
/// `lire_reponse_tiers`. Nos formes et les retours de la norme (net-entreprises, Urssaf) ne la portent jamais.
pub trait ReponseDUnTiers: DeserializeOwned {}

/// LA PORTE DES TIERS : le texte JSON `texte`, reponse d'un tiers, lu en `T`, comme `lire_texte`. On n'y lit que des
/// cles connues et typees : une cle exigee qui manque, ou d'un autre type, refuse la reponse. Une cle que le tiers ajoute
/// et que `T` ne declare pas est ignoree a la porte : ni lue, ni gardee, ni transmise. Sans cela, une cle neuve de Google
/// couperait la connexion et le Drive jusqu'a une nouvelle version.
#[track_caller]
pub fn lire_reponse_tiers<T: ReponseDUnTiers>(texte: &str, phrase: &str) -> R<T> {
    lire_texte(texte, phrase)
}

/// UNE FORME ECRITE (`Serialize`) RELUE EN `T` : ce qu'elle ecrit, lu par `lire`.
#[track_caller]
pub fn relire<T: DeserializeOwned>(t: &(impl serde::Serialize + ?Sized), phrase: &str) -> R<T> {
    lire(&crate::ecriture::en_valeur(t), phrase)
}

/// UN OBJET JSON QUI PASSE UNE PORTE AVANT D'ETRE LU EN SA FORME : la charge d'une route, que sa route lit
/// (`routes::enveloppe::Charge`), ce qu'une route rend, que la porte controle et que le moteur relit
/// (`routes::enveloppe::Rendu`). Il ne se lit qu'en une forme (`lire`) ; ce que l'enveloppe en voit sans forme (le mot de
/// passe qu'elle porte, le mois d'une charge) se lit aussi en une forme, sur les seules cles que l'enveloppe nomme
/// (`lire_cles`). Son JSON ne se touche qu'ici et dans `ecriture` (`ecriture::en_objet`).
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct Objet(pub(crate) serde_json::Map<String, Value>);

/// Un objet, et rien d'autre.
impl<'de> serde::Deserialize<'de> for Objet {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Objet, D::Error> {
        serde_json::Map::deserialize(d).map(Objet)
    }
}

impl Objet {
    /// L'objet sans aucune cle, `{}` : la charge d'une route qui ne prend rien.
    pub fn sans_cle() -> Objet {
        Objet(serde_json::Map::new())
    }

    /// LE TEXTE JSON `texte`, UN OBJET : un texte qui n'est pas du JSON se refuse par `illisible`
    /// (`Classe::DemandeIllisible`) ; une autre valeur qu'un objet, par `pas_un_objet` (`Classe::MalFormee`).
    #[track_caller]
    pub fn du_texte(texte: &str, illisible: &str, pas_un_objet: &str) -> R<Objet> {
        match lire_texte::<Value>(texte, illisible).map_err(|e| e.reclasser(crate::echec::Classe::DemandeIllisible))? {
            Value::Object(m) => Ok(Objet(m)),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => Err(crate::echec::erreur(crate::echec::Classe::MalFormee, pas_un_objet)),
        }
    }

    /// L'OBJET LU EN SA FORME `T`, sans ses cles `sans` (ce que l'enveloppe porte a cote de la forme) : comme `lire`.
    #[track_caller]
    pub fn lire<T: DeserializeOwned>(&self, sans: &[&str], phrase: &str) -> R<T> {
        let mut m = self.0.clone();
        for c in sans {
            m.remove(*c);
        }
        lire(&Value::Object(m), phrase)
    }

    /// L'objet, une saisie (la charge d'une route), sans ses cles `sans`, lu en `T` comme `lire`, chaque texte range
    /// dans la forme de sa notion (`lire_saisie`).
    #[track_caller]
    pub fn lire_saisie<T: DeserializeOwned>(&self, sans: &[&str], phrase: &str) -> R<T> {
        let mut m = self.0.clone();
        for c in sans {
            m.remove(*c);
        }
        lire_saisie(&Value::Object(m), phrase)
    }

    /// LES SEULES CLES `cles` DE L'OBJET (celles qu'il porte), LUES EN LEUR FORME `T` : ce que l'enveloppe lit a cote de
    /// la forme de la route (le mot de passe, le mois d'une charge ; ce qu'un rendu porte pour le banc). Strict comme
    /// `lire` : une valeur d'une autre forme la refuse.
    #[track_caller]
    pub fn lire_cles<T: DeserializeOwned>(&self, cles: &[&str], phrase: &str) -> R<T> {
        let m: serde_json::Map<String, Value> = self.0.iter().filter(|(k, _)| cles.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
        lire(&Value::Object(m), phrase)
    }

    /// LE JOUR DU MOIS `mois` QUE CETTE SAISIE DIT A `cle` (le numero du jour, tape ou en nombre, lu par la lecture du
    /// nombre tape), lu une fois en `Jour` : le numero d'un jour n'existe qu'ecrit. Absent ou nul, aucun. Un numero qui
    /// ne se lit pas, ou qui n'est pas un jour de ce mois, refuse la saisie a sa case (`echec::CaseRefusee`), `phrase` pour
    /// qui ne la nomme pas.
    #[track_caller]
    pub fn jour_du_mois(&self, cle: &str, mois: crate::mois::Mois, phrase: &str) -> R<Option<crate::date::Jour>> {
        let Some(v) = self.0.get(cle).filter(|v| !v.is_null()) else { return Ok(None) };
        let refusee = |detail: String| case_refusee_a(phrase, crate::echec::CaseDeLaSaisie::Cle(cle.to_string()), cle.to_string(), detail);
        // Relu de son texte, comme `lire_saisie` : un nombre JSON (`3.0`) se lit comme le meme nombre tape (« 3,0 »).
        let n = {
            let _remise = EnSaisie::ouverte();
            let texte = v.to_string();
            let mut d = serde_json::Deserializer::from_str(&texte);
            crate::grandeurs::lu::entier(&mut d, "un jour du mois, de 1 à 31").map_err(|e| refusee(en_clair(&e.to_string())))?
        };
        match u32::try_from(n).ok().and_then(|n| mois.jour(n)) {
            Some(j) => Ok(Some(j)),
            None => Err(refusee(format!("« {} » n'est pas un jour de {}", n, mois.libelle(crate::mois::Graphie::Accentuee)))),
        }
    }

    /// Ses cles, dans l'ordre ecrit.
    pub fn cles(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }

    /// LA VALEUR DE LA SEULE CLE `cle`, LUE EN SA FORME `T` comme une saisie (`lire_saisie`) ; absente, aucune.
    #[track_caller]
    pub fn lire_une<T: DeserializeOwned>(&self, cle: &str, phrase: &str) -> R<Option<T>> {
        match self.0.get(cle) {
            Some(v) => lire_saisie(v, phrase).map(Some),
            None => Ok(None),
        }
    }

    /// CHAQUE ENTREE DONT LA CLE EST GARDEE (`garder`), LUE SEULE EN SA FORME `T`, comme l'objet
    /// `{nom_cle: cle, nom_valeur: valeur}` d'une saisie (`lire_saisie`) : une entree fautive ne fait pas taire les
    /// suivantes. Sa faute : le chemin sous la valeur (vide pour la valeur elle-meme) et ce qui ne va pas.
    pub fn chacune<T: DeserializeOwned>(&self, garder: impl Fn(&str) -> bool, nom_cle: &str, nom_valeur: &str) -> Vec<(String, Result<T, (String, String)>)> {
        let mut out = Vec::new();
        for (cle, valeur) in self.0.iter().filter(|(k, _)| garder(k)) {
            let mut entree = serde_json::Map::new();
            entree.insert(nom_cle.to_string(), Value::String(cle.clone()));
            entree.insert(nom_valeur.to_string(), valeur.clone());
            out.push((cle.clone(), une_entree(&Value::Object(entree), nom_valeur)));
        }
        out
    }

    /// L'OBJET D'UN FORMULAIRE, SES CASES BLANCHES VIDES : une case que la page laisse blanche (`""`, des blancs) n'y dit
    /// rien, a toute profondeur (une case d'une structure, d'une ligne) ; elle s'ecrit nulle (`null`).
    pub fn sans_textes_blancs(&self) -> Objet {
        fn blanchir(v: &Value) -> Value {
            match v {
                Value::String(t) if t.trim().is_empty() => Value::Null,
                Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), blanchir(x))).collect()),
                Value::Array(a) => Value::Array(a.iter().map(blanchir).collect()),
                x => x.clone(),
            }
        }
        Objet(self.0.iter().map(|(k, x)| (k.clone(), blanchir(x))).collect())
    }

    /// L'objet sans sa cle `cle`.
    pub fn retirer(&mut self, cle: &str) {
        self.0.remove(cle);
    }

    /// L'objet qui porte le texte `texte` a `cle`.
    pub fn poser_texte(&mut self, cle: &str, texte: String) {
        self.0.insert(cle.to_string(), Value::String(texte));
    }

    /// L'OBJET SANS LES TEXTES DES CLES QUI COMMENCENT PAR `prefixe`, a toute profondeur : chacun vide.
    pub fn vider_les_textes(&self, prefixe: &str) -> Objet {
        fn vider(v: &Value, prefixe: &str) -> Value {
            match v {
                Value::Object(o) => Value::Object(vider_l_objet(o, prefixe)),
                Value::Array(a) => Value::Array(a.iter().map(|x| vider(x, prefixe)).collect()),
                x => x.clone(),
            }
        }
        fn vider_l_objet(o: &serde_json::Map<String, Value>, prefixe: &str) -> serde_json::Map<String, Value> {
            o.iter().map(|(k, x)| (k.clone(), if k.starts_with(prefixe) && x.is_string() { Value::String(String::new()) } else { vider(x, prefixe) })).collect()
        }
        Objet(vider_l_objet(&self.0, prefixe))
    }
}

/// UNE ENTREE LUE SEULE (`Objet::chacune`, `Liste::chacun`) comme une saisie : sa faute, le chemin sous `sous` (vide pour la
/// valeur elle-meme) et ce qui ne va pas.
fn une_entree<T: DeserializeOwned>(v: &Value, sous: &str) -> Result<T, (String, String)> {
    let _remise = EnSaisie::ouverte();
    let texte = v.to_string();
    let mut d = serde_json::Deserializer::from_str(&texte);
    match serde_path_to_error::deserialize(&mut d) {
        Ok(t) => Ok(t),
        Err(e) => {
            let chemin = e.path().to_string();
            let chemin = match chemin.strip_prefix(sous) {
                Some(reste) => reste.trim_start_matches('.').to_string(),
                None if chemin == "." => String::new(),
                None => chemin,
            };
            Err((chemin, en_clair(&e.inner().to_string())))
        }
    }
}

/// LE TEXTE `texte` QUE LE MOTEUR POSE DANS UNE CASE (une forme declaree qu'il fabrique de la lisible), lu dans le type de
/// sa case comme un changement `{nom_cle: cle, nom_valeur: texte}` qui se saisit : sa faute, le chemin et ce qui ne va pas.
pub fn un_texte_pose<T: DeserializeOwned>(nom_cle: &str, cle: &str, nom_valeur: &str, texte: &str) -> Result<T, (String, String)> {
    let mut entree = serde_json::Map::new();
    entree.insert(nom_cle.to_string(), Value::String(cle.to_string()));
    entree.insert(nom_valeur.to_string(), Value::String(texte.to_string()));
    une_entree(&Value::Object(entree), nom_valeur)
}

/// UNE LISTE QUE SA FORME LIT ELEMENT PAR ELEMENT (les changements d'une fiche, `droit::fiche::Changements`) : chacun lu
/// seul, une faute ne faisant pas taire les suivants (`chacun`). Vide, elle ne dit aucun changement.
#[derive(Clone, Debug, PartialEq)]
pub struct Liste(Vec<Value>);

impl<'de> serde::Deserialize<'de> for Liste {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Liste, D::Error> {
        Vec::<Value>::deserialize(d).map(Liste)
    }
}

/// Elle s'ecrit comme elle se lit : ses elements, tels quels (la charge d'un geste qu'une route rend).
impl serde::Serialize for Liste {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl Liste {

    /// CHAQUE ELEMENT LU SEUL EN SA FORME `T`, comme une saisie (`lire_saisie`) ; sa faute : le chemin sous `sous` et ce qui
    /// ne va pas.
    pub fn chacun<T: DeserializeOwned>(&self, sous: &str) -> Vec<Result<T, (String, String)>> {
        self.0.iter().map(|v| une_entree(v, sous)).collect()
    }
}

/// UNE CLE CONNUE QUE LE LOGICIEL NE LIT PAS : une note d'un fichier (`quoi`, `note`, `source` : atelier/AGENTS.md,
/// « Chaque forme est declaree une fois »), ou une valeur que le fichier porte pour qui l'ecrit. La forme la nomme, pour
/// refuser toute cle inconnue (`deny_unknown_fields`), et n'en garde rien : son type ne porte aucune valeur
/// (`PhantomData`, que le compilateur sait vide). Elle se lit par `non_lue`, exigee :
/// `#[serde(deserialize_with = "socle::forme::non_lue")]` ; une note qui peut manquer ajoute `default`, qui ne fabrique
/// aucune valeur.
pub type NonLue = std::marker::PhantomData<Rien>;

/// Ce qu'une cle non lue garde : rien (aucune valeur de ce type n'existe).
pub enum Rien {}

/// La valeur d'une cle non lue (`NonLue`) : n'importe quelle valeur JSON, parcourue et jetee.
pub fn non_lue<'de, D: serde::Deserializer<'de>>(d: D) -> Result<NonLue, D::Error> {
    <serde::de::IgnoredAny as serde::Deserialize>::deserialize(d).map(|_| std::marker::PhantomData)
}

// ------------------------------------------------------------------ le fichier d'une installation posee

thread_local! {
    /// Vrai le temps de lire le fichier d'une installation posee (`lire_fichier_pose_en_phrase`).
    static EN_FICHIER_POSE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Le temps d'une lecture du fichier d'une installation posee (`EN_FICHIER_POSE`) : l'etat d'avant revient en sortant,
/// panique comprise.
struct EnFichierPose(bool);

impl EnFichierPose {
    fn ouvert() -> EnFichierPose {
        EnFichierPose(EN_FICHIER_POSE.replace(true))
    }
}

impl Drop for EnFichierPose {
    fn drop(&mut self) {
        EN_FICHIER_POSE.set(self.0);
    }
}

/// LE FICHIER D'UNE INSTALLATION POSEE (les fiches de la compagnie : `droit::fiche::lire_fiche_posee`), lu comme
/// `lire_fichier_en_phrase`, a ceci pres : la valeur d'une case qui n'a plus la forme de son type se garde telle
/// quelle et ne dit rien (`LueOuIllisible`), une cle que la forme ne connait pas se garde et ne dit rien
/// (`ClesInconnues`). Le logiciel est doux : une valeur qu'une version precedente ou une main a ecrite ne ferme pas la
/// compagnie. Un fichier qui n'est pas du JSON, ou dont la structure d'ensemble est fausse, se refuse comme ailleurs. Les
/// donnees livrees et les charges des routes ne passent jamais par ici : elles restent strictes.
#[track_caller]
pub fn lire_fichier_pose_en_phrase<T: DeserializeOwned>(fichier: &Path, phrase: impl Fn(&str, &str) -> String) -> R<T> {
    let _remise = EnFichierPose::ouvert();
    lire_fichier_en_phrase(fichier, phrase)
}

/// Lit-on le fichier d'une installation posee (`lire_fichier_pose_en_phrase`) ? Ce qu'une installation a pu ecrire et
/// que la forme neuve ne prend plus (le texte d'avant d'une case a choix, `droit::versement`) ne se lit que la.
pub fn en_fichier_pose() -> bool {
    EN_FICHIER_POSE.get()
}

/// UNE VALEUR ILLISIBLE : la valeur qu'une case d'un fichier pose porte et que son type ne lit pas, gardee telle
/// qu'ecrite pour se reecrire telle quelle (rien ne s'ecrit par-dessus sans geste) ; elle ne dit rien au calcul.
#[derive(Debug, Clone, PartialEq)]
pub struct ValeurIllisible(Value);

impl ValeurIllisible {
    /// LA VALEUR TELLE QU'UN ADMINISTRATEUR LA LIT : un texte tel quel, un nombre ou un oui-non tel qu'il s'ecrit, une
    /// liste ou un objet par leurs valeurs, separees d'un point median (les cles d'un objet sont des noms internes : elles
    /// ne se montrent pas) ; rien d'ecrit, un tiret.
    pub fn ecrite(&self) -> String {
        fn feuilles(v: &Value, out: &mut Vec<String>) {
            match v {
                Value::Null => {}
                Value::String(t) => out.push(t.clone()),
                Value::Array(l) => l.iter().for_each(|x| feuilles(x, out)),
                Value::Object(o) => o.values().for_each(|x| feuilles(x, out)),
                Value::Bool(_) | Value::Number(_) => out.push(v.to_string()),
            }
        }
        let mut out = Vec::new();
        feuilles(&self.0, &mut out);
        if out.is_empty() {
            return "—".to_string();
        }
        out.join(" · ")
    }

    /// Combien elle porte d'elements, quand c'est une liste.
    pub fn elements(&self) -> Option<usize> {
        match &self.0 {
            Value::Array(l) => Some(l.len()),
            _ => None,
        }
    }

    /// Est-ce un objet, des cases ecrites ensemble ?
    pub fn est_un_objet(&self) -> bool {
        matches!(self.0, Value::Object(_))
    }
}

/// Elle se reecrit telle qu'elle a ete lue.
impl serde::Serialize for ValeurIllisible {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

/// UNE VALEUR LUE EN `T`, OU ILLISIBLE : dans un fichier pose (`en_fichier_pose`), une valeur que `T` ne lit pas se
/// garde (`ValeurIllisible`) ; partout ailleurs, elle refuse la lecture comme `T` le ferait. Absente, elle vaut ce que
/// `T` vaut par defaut (une case absente : vide).
#[derive(Debug, Clone, PartialEq)]
pub enum LueOuIllisible<T> {
    Lue(T),
    Illisible(ValeurIllisible),
}

impl<T: Default> Default for LueOuIllisible<T> {
    fn default() -> Self {
        LueOuIllisible::Lue(T::default())
    }
}

impl<'de, T: DeserializeOwned> serde::Deserialize<'de> for LueOuIllisible<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        if !EN_FICHIER_POSE.get() {
            return T::deserialize(d).map(LueOuIllisible::Lue);
        }
        let v = <Value as serde::Deserialize>::deserialize(d)?;
        // Relue dans son texte : un nombre n'y passe jamais par un flottant (`lire`).
        let texte = v.to_string();
        let mut lu = serde_json::Deserializer::from_str(&texte);
        match T::deserialize(&mut lu) {
            Ok(t) => Ok(LueOuIllisible::Lue(t)),
            Err(_) => Ok(LueOuIllisible::Illisible(ValeurIllisible(v))),
        }
    }
}

/// LES CLES QU'UNE FORME NE CONNAIT PAS, dans un fichier pose : chacune gardee telle qu'ecrite, pour se reecrire telle
/// quelle (`#[serde(flatten)]` a l'ecriture) ; elles ne disent rien au calcul. Partout ailleurs, une cle inconnue
/// refuse la lecture (`garder`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClesInconnues(Vec<(String, Value)>);

impl ClesInconnues {
    /// LA CLE `cle` QUE LA FORME NE CONNAIT PAS (ses cles : `connues`), sa valeur au bout de `m` : gardee dans un fichier
    /// pose (`en_fichier_pose`), refusee partout ailleurs (« unknown field », comme `deny_unknown_fields`).
    pub fn garder<'de, A: serde::de::MapAccess<'de>>(&mut self, cle: String, m: &mut A, connues: &'static [&'static str]) -> Result<(), A::Error> {
        if !EN_FICHIER_POSE.get() {
            return Err(<A::Error as serde::de::Error>::unknown_field(&cle, connues));
        }
        let v: Value = m.next_value()?;
        self.0.push((cle, v));
        Ok(())
    }

    /// Leurs noms, dans l'ordre du fichier.
    pub fn noms(&self) -> Vec<&str> {
        self.0.iter().map(|(k, _)| k.as_str()).collect()
    }
}

/// Elles se reecrivent telles qu'elles ont ete lues, a cote des cles connues (`#[serde(flatten)]`).
impl serde::Serialize for ClesInconnues {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

/// Le fichier JSON `fichier` lu en `T` (`lire_texte`, sa lecture notee au registre d'un calcul suivi :
/// `fichiers::lire_texte`). Le disque qui refuse se dit par `echec::du_disque` ; une forme fausse par `phrase`, le
/// chemin du fichier, la cle et le detail a part.
#[track_caller]
pub fn lire_fichier<T: DeserializeOwned>(fichier: &Path, phrase: &str) -> R<T> {
    let texte = match crate::fichiers::lire_texte(fichier) { Ok(t) => t, Err(e) => return Err(crate::echec::du_disque(fichier, &e)) };
    let mut d = serde_json::Deserializer::from_str(&texte);
    match serde_path_to_error::deserialize(&mut d) { Ok(t) => Ok(t), Err(e) => Err(refus(phrase, Some(fichier), &e)) }
}

/// UNE DONNEE DONT LES VALEURS NE S'AFFICHENT JAMAIS (l'identite d'une vraie compagnie, `compagnie::essai`) : le
/// texte JSON `texte` lu en `T`, comme `lire_texte`, jusqu'a sa fin. Son refus ne dit que le chemin de la cle fautive, et
/// ce qui ne va pas sans la valeur : la cle qui manque, la cle inconnue (son nom), la forme attendue, la ligne et la
/// colonne d'un texte qui n'est pas du JSON. Le message de serde, qui peut citer la valeur, ne sort pas.
#[track_caller]
pub fn lire_texte_sans_valeur<T: DeserializeOwned>(texte: &str, phrase: &str) -> R<T> {
    let mut d = serde_json::Deserializer::from_str(texte);
    let t = match serde_path_to_error::deserialize(&mut d) { Ok(t) => t, Err(e) => return Err(refus_sans_valeur(phrase, &e.path().to_string(), e.inner())) };
    match d.end() { Ok(()) => Ok(t), Err(e) => Err(refus_sans_valeur(phrase, ".", &e)) }
}

#[track_caller]
fn refus_sans_valeur(phrase: &str, chemin: &str, e: &serde_json::Error) -> Echec {
    let detail = match e.classify() {
        serde_json::error::Category::Data => {
            let m = e.to_string();
            let m = match m.split_once(" at line ") {
                Some((avant, _)) => avant.to_string(),
                None => m,
            };
            if let Some(r) = m.strip_prefix("unknown field ") {
                let cle = match r.split_once(',') {
                    Some((cle, _)) => cle,
                    None => r,
                };
                Some(format!("clé inconnue : {}", cle))
            } else if let Some(cle) = m.strip_prefix("missing field ") {
                Some(format!("la clé {} manque", cle))
            } else {
                m.split_once(", expected ").map(|(_, attendu)| format!("la valeur n'a pas la forme attendue : {}", attendu))
            }
        }
        _ => Some(format!("ce n'est pas un JSON lisible (ligne {}, colonne {})", e.line(), e.column())),
    };
    refus_avec(phrase, Lieu { cle: Some(if chemin == "." { String::new() } else { chemin.to_string() }), detail, ..Lieu::default() })
}

#[track_caller]
fn refus(phrase: &str, fichier: Option<&Path>, e: &serde_path_to_error::Error<serde_json::Error>) -> Echec {
    let cle = e.path().to_string();
    let detail = en_clair(&e.inner().to_string());
    refus_avec(
        phrase,
        Lieu {
            chemin: fichier.map(|f| f.to_string_lossy().into_owned()),
            cle: Some(if cle == "." { String::new() } else { cle }),
            detail: Some(detail).filter(|d| !d.is_empty()),
            ..Lieu::default()
        },
    )
}

/// Le message de serde, dit en francais pour ce qu'il a de courant ; le reste tel quel.
fn en_clair(m: &str) -> String {
    let m = match m.split_once(" at line ") {
        Some((avant, _)) => avant,
        None => m,
    };
    if let Some(r) = m.strip_prefix("unknown field ") {
        let cle = match r.split_once(',') {
            Some((cle, _)) => cle,
            None => r,
        };
        return match r.split_once("expected ") {
            Some((_, a)) => format!("la clé {} n'est pas connue ici (attendues : {})", cle, a.replace("one of ", "")),
            None => format!("la clé {} n'est pas connue ici : aucune n'est attendue", cle),
        };
    }
    if let Some(r) = m.strip_prefix("missing field ") {
        return format!("la clé {} manque", r);
    }
    if let Some(r) = m.strip_prefix("invalid type: ") {
        return format!("la valeur n'a pas la forme attendue : {}", r.replace(", expected ", ", attendu "));
    }
    if let Some(r) = m.strip_prefix("invalid value: ") {
        return match r.split_once(", expected ") {
            Some((valeur, attendu)) => format!("la valeur {} n'est pas admise (attendu : {})", valeur, attendu),
            None => format!("la valeur {} n'est pas admise", r),
        };
    }
    if let Some(r) = m.strip_prefix("unknown variant ") {
        return format!("la valeur {} n'est pas connue ici", r.replace(", expected one of ", " (attendues : ").replace(", expected ", " (attendue : ").replace("` or `", "` ou `") + ")");
    }
    m.to_string()
}

/// LE DECIMAL D'UN NOMBRE A VIRGULE D'UN FICHIER, lu exactement dans son texte (le lecteur JSON le garde en texte :
/// `arbitrary_precision`) : sans zero en queue, une decimale au moins sous 10^16 (`1.0` reste `1.0`, `1.50` est
/// `1.5`, `1e5` est `100000.0`).
fn decimal_ecrit(t: &str) -> crate::echec::R<crate::num::Decimal> {
    use std::str::FromStr;
    let d = match crate::num::Decimal::from_str(t).or_else(|_| crate::num::Decimal::from_scientific(t)) {
        Ok(d) => d.normalize(),
        Err(e) => {
            return Err(crate::echec::refus_avec(
                format!("nombre hors de portee : {}", t),
                crate::echec::Lieu { detail: Some(e.to_string()), ..Default::default() },
            ))
        }
    };
    let mut d = d;
    if d.scale() == 0 && d.abs() < crate::num::Decimal::from(10_000_000_000_000_000i64) {
        d.rescale(1);
    }
    Ok(d)
}

/// UN NOMBRE LU TEL QU'IL EST ECRIT (un instant en secondes, `paris::Moment`) : son texte, sans passer par un flottant.
pub fn nombre_en_texte<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    use serde::Deserialize;
    serde_json::Number::deserialize(d).map(|n| n.to_string())
}

/// LE TEXTE D'UN NOMBRE A VIRGULE QUE LE LECTEUR JSON REND : `arbitrary_precision` le passe en objet a cle privee, que
/// `serde_json::Number` sait lire.
pub(crate) fn texte_a_virgule<'de, A: serde::de::MapAccess<'de>>(m: A) -> Result<String, A::Error> {
    use serde::Deserialize;
    serde_json::Number::deserialize(serde::de::value::MapAccessDeserializer::new(m)).map(|n| n.to_string())
}

/// UN NOMBRE JSON DE SON TEXTE (un decimal, un instant) : il s'ecrit tel quel, exact (`ecriture::nombre_ecrit`).
pub fn nombre_du_texte(t: &str) -> serde_json::Number {
    match t.parse::<serde_json::Number>() {
        Ok(n) => n,
        Err(e) => unreachable!("un nombre s'ecrit en chiffres : {} ({})", t, e),
    }
}

/// UN MONTANT OU UN TAUX EN DECIMAL : un entier garde son echelle nulle, un nombre a virgule sa forme la plus courte
/// (`1.0` reste `1.0`, `decimal_ecrit`), un nombre ecrit en texte se lit (`num::dec_texte`).
/// `#[serde(deserialize_with = "socle::forme::decimal")]`.
pub(crate) fn decimal<'de, D: serde::Deserializer<'de>>(d: D) -> Result<crate::num::Decimal, D::Error> {
    d.deserialize_any(EnDecimal { texte: TexteLu::Ecrit })
}

/// LA LECTURE D'UNE GRANDEUR (`grandeurs`) : un nombre JSON ; dans une saisie (`lire_saisie`, la charge d'une route), ce
/// qu'une personne a tape, converti par `tapee` (`grandeurs::lu::tapee`, la seule conversion d'une saisie) ; dans un
/// fichier, jamais un texte.
pub(crate) fn decimal_en_nombre<'de, D: serde::Deserializer<'de>>(d: D, tapee: fn(&str) -> Result<crate::num::Decimal, String>) -> Result<crate::num::Decimal, D::Error> {
    d.deserialize_any(EnDecimal { texte: if EN_SAISIE.get() { TexteLu::Tape(tapee) } else { TexteLu::Refuse } })
}

/// `decimal` d'une grandeur qu'un fichier ecrit en texte (une ligne de paie) : dans une saisie, ce qu'une personne a tape,
/// converti par `tapee`, comme `decimal_en_nombre`.
pub(crate) fn decimal_ou_tape<'de, D: serde::Deserializer<'de>>(d: D, tapee: fn(&str) -> Result<crate::num::Decimal, String>) -> Result<crate::num::Decimal, D::Error> {
    d.deserialize_any(EnDecimal { texte: if EN_SAISIE.get() { TexteLu::Tape(tapee) } else { TexteLu::Ecrit } })
}

/// Ce qu'un nombre ecrit en texte devient : refuse, lu tel qu'un fichier l'ecrit (`num::dec_texte`), ou converti comme
/// une personne l'a tape.
enum TexteLu {
    Refuse,
    Ecrit,
    Tape(fn(&str) -> Result<crate::num::Decimal, String>),
}

/// Le visiteur de `decimal` ; `texte` : ce qu'un nombre ecrit en texte devient.
struct EnDecimal {
    texte: TexteLu,
}

impl<'de> serde::de::Visitor<'de> for EnDecimal {
    type Value = crate::num::Decimal;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("un nombre")
    }
    fn visit_i64<E: serde::de::Error>(self, i: i64) -> Result<Self::Value, E> {
        Ok(i.into())
    }
    fn visit_u64<E: serde::de::Error>(self, u: u64) -> Result<Self::Value, E> {
        Ok(u.into())
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(self, m: A) -> Result<Self::Value, A::Error> {
        let texte = texte_a_virgule(m)?;
        match self.texte {
            // Dans une saisie, un nombre se lit comme le meme nombre tape (`1e3` se refuse comme « 1e3 »).
            TexteLu::Tape(tapee) => tapee(&texte).map_err(serde::de::Error::custom),
            TexteLu::Refuse | TexteLu::Ecrit => decimal_ecrit(&texte).map_err(|e| serde::de::Error::custom(e.phrase)),
        }
    }
    fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
        match self.texte {
            TexteLu::Refuse => Err(E::custom(format!("« {} » est écrit en texte : écrire le nombre sans guillemets", s))),
            TexteLu::Ecrit => crate::num::dec_texte(s).map_err(|e| E::custom(e.phrase)),
            TexteLu::Tape(tapee) => tapee(s).map_err(E::custom),
        }
    }
}

/// `decimal`, quand la valeur peut manquer ou etre nulle (avec `#[serde(default)]`).
pub(crate) fn decimal_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<crate::num::Decimal>, D::Error> {
    <Option<Nombre> as serde::Deserialize>::deserialize(d).map(|n| n.map(|n| n.0))
}

/// UN TEXTE DIT : jamais vide. `Option<TexteDit>` est la case facultative d'une charge : absente ou nulle, elle est vide
/// (`None`) ; un texte vide `""` ne dit rien et se refuse a la porte.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct TexteDit(String);

impl TexteDit {
    /// Le texte `t`, dit ; vide, il se refuse.
    pub fn lire(t: &str) -> R<TexteDit> {
        if t.is_empty() {
            return Err(crate::refus!("un texte vide ne dit rien : le vide s'écrit absent ou null"));
        }
        Ok(TexteDit(t.to_string()))
    }
}

impl<'de> serde::Deserialize<'de> for TexteDit {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let t = <String as serde::Deserialize>::deserialize(d)?;
        TexteDit::lire(&t).map_err(|e| serde::de::Error::custom(e.phrase))
    }
}

impl std::ops::Deref for TexteDit {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TexteDit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// UN OBJET DONT L'ORDRE COMPTE (un catalogue montre dans l'ordre du fichier), lu en paires `(cle, valeur)` :
/// `#[serde(deserialize_with = "socle::forme::en_ordre")]` sur un `Vec<(K, T)>` (`K` : un texte, ou un type qui se lit
/// d'un texte). Une cle repetee refuse.
pub fn en_ordre<'de, D, K, T>(d: D) -> Result<Vec<(K, T)>, D::Error>
where
    D: serde::Deserializer<'de>,
    K: serde::Deserialize<'de> + PartialEq + std::fmt::Display,
    T: serde::Deserialize<'de>,
{
    struct Visiteur<K, T>(std::marker::PhantomData<(K, T)>);
    impl<'de, K: serde::Deserialize<'de> + PartialEq + std::fmt::Display, T: serde::Deserialize<'de>> serde::de::Visitor<'de> for Visiteur<K, T> {
        type Value = Vec<(K, T)>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("un objet")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Self::Value, A::Error> {
            let mut out: Vec<(K, T)> = Vec::new();
            while let Some((k, v)) = m.next_entry::<K, T>()? {
                if out.iter().any(|(x, _)| *x == k) {
                    return Err(serde::de::Error::custom(format!("la clé {} est répétée", k)));
                }
                out.push((k, v));
            }
            Ok(out)
        }
    }
    d.deserialize_map(Visiteur(std::marker::PhantomData))
}

/// UN TAUX, UNE QUOTITE, UN COEFFICIENT, UNE QUANTITE QUE LE CALCUL LIT : un nombre (jamais un texte), en decimal, dans
/// sa forme la plus courte, une decimale au moins (`num::court` : `24` se lit `24.0`, `0.10` se lit `0.1`).
/// `#[serde(deserialize_with = "socle::forme::court")]`.
pub(crate) fn court<'de, D: serde::Deserializer<'de>>(d: D) -> Result<crate::num::Decimal, D::Error> {
    struct EnCourt;
    impl<'de> serde::de::Visitor<'de> for EnCourt {
        type Value = crate::num::Decimal;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("un nombre")
        }
        fn visit_i64<E: serde::de::Error>(self, i: i64) -> Result<Self::Value, E> {
            Ok(crate::num::court(i.into()))
        }
        fn visit_u64<E: serde::de::Error>(self, u: u64) -> Result<Self::Value, E> {
            Ok(crate::num::court(u.into()))
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, m: A) -> Result<Self::Value, A::Error> {
            decimal_ecrit(&texte_a_virgule(m)?).map(crate::num::court).map_err(|e| serde::de::Error::custom(e.phrase))
        }
    }
    d.deserialize_any(EnCourt)
}

/// Un nombre lu comme `court`, la ou un champ ne se pose pas (une valeur de table, une liste datee), ou la ou il peut
/// manquer (`Option<Court>` : absent ou nul, il est vide).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Court(pub(crate) crate::num::Decimal);

impl Court {
    /// Le nombre tel qu'une phrase le cite, le fichier qui l'écrit nommé à côté (« 1 500 », « 12,5 »).
    pub fn dit(&self) -> String {
        crate::texte::nombre_dit_par_milliers(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Court {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        court(d).map(Court)
    }
}

/// DEUX TEXTES QUI ECRIVENT LE MEME NOMBRE, a 1e-25 pres (`"2.50"` et `"2.5"` ; un oracle relu, un decimal a 28 chiffres
/// d'un cote) ; un texte qui n'est pas un nombre ecrit au point decimal n'en ecrit aucun.
pub fn memes_nombres_ecrits(x: &str, y: &str) -> bool {
    use std::str::FromStr;
    use crate::num::Decimal;
    match (Decimal::from_str(x), Decimal::from_str(y)) {
        (Ok(x), Ok(y)) => (x - y).abs() < Decimal::from_parts(1, 0, 0, false, 25),
        _ => false,
    }
}

/// Le refus d'un nombre qui ne se lit pas, avec le texte en cause.
pub fn illisible(t: &str) -> String {
    format!("nombre illisible : {}", crate::fichiers::repr(t))
}

/// Un nombre decimal, lu comme `decimal`, la ou un champ ne se pose pas (une valeur de table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nombre(pub(crate) crate::num::Decimal);

impl Nombre {
    /// LE NOMBRE QU'UNE RUBRIQUE DE LA NORME ECRIT (une DSN : `151.67`, `-2`), comme `num::decimal_lu` le lit, son
    /// echelle gardee : la seule lecture d'un nombre de la norme, avant ou apres que sa place dise sa grandeur
    /// (`grandeurs::lu::norme`) ; `None` : le texte n'est pas un nombre.
    pub fn de_la_norme(t: &str) -> Option<Nombre> {
        crate::num::decimal_lu(t).map(Nombre)
    }

    /// LE NOMBRE QU'UNE PIECE IMPRIME, a la francaise ou non (`num::decimal_a_virgule` : « 1 512,40 », « 0.000 », un
    /// signe en tete), son echelle gardee : la seule lecture d'un nombre imprime, avant ou apres que sa place dise sa
    /// grandeur (`grandeurs::lu::texte`) ; `None` : le texte n'est pas un nombre.
    pub fn a_virgule(t: &str) -> Option<Nombre> {
        crate::num::decimal_a_virgule(t).map(Nombre)
    }
}

impl<'de> serde::Deserialize<'de> for Nombre {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        decimal(d).map(Nombre)
    }
}

/// Il se reecrit en nombre JSON, sa valeur gardee : un entier tel quel (`3`), sinon a virgule dans sa forme la plus
/// courte (`86.67` ; `12.50` s'ecrit `12.5` : `ecrire_a_virgule`) ; une donnee relue puis reecrite (une description de
/// l'atelier) garde la valeur de ses nombres, pas leurs zeros de fin.
impl serde::Serialize for Nombre {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match (self.0.scale(), i64::try_from(self.0.mantissa())) {
            (0, Ok(n)) => s.serialize_i64(n),
            _ => ecrire_a_virgule(&self.0, s),
        }
    }
}

/// Le nombre tel qu'il est lu, son echelle gardee (`12.50`) : ce qu'un avertissement de lecture cite.
impl std::fmt::Display for Nombre {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// UNE LISTE QU'UNE FORME EXIGE NON VIDE (les paliers d'une regle, dont le dernier vaut au-dela) : lue d'un tableau,
/// un tableau vide se refuse a la lecture ; son dernier element se lit sans refus.
#[derive(Debug, Clone, PartialEq)]
pub struct AuMoinsUn<T> {
    avant: Vec<T>,
    dernier: T,
}

impl<T> AuMoinsUn<T> {
    /// Les elements de `liste`, dans l'ordre ; `None` : elle est vide (la forme qui la lit le refuse en ses mots).
    pub fn de(mut liste: Vec<T>) -> Option<AuMoinsUn<T>> {
        let dernier = liste.pop()?;
        Some(AuMoinsUn { avant: liste, dernier })
    }

    /// Ses elements, dans l'ordre.
    pub fn iter(&self) -> impl Iterator<Item = &T> + Clone {
        self.avant.iter().chain(std::iter::once(&self.dernier))
    }

    /// Son dernier element.
    pub fn dernier(&self) -> &T {
        &self.dernier
    }
}

/// Ses elements, dans l'ordre.
impl<T> IntoIterator for AuMoinsUn<T> {
    type Item = T;
    type IntoIter = std::iter::Chain<std::vec::IntoIter<T>, std::iter::Once<T>>;
    fn into_iter(self) -> Self::IntoIter {
        self.avant.into_iter().chain(std::iter::once(self.dernier))
    }
}

impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for AuMoinsUn<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        AuMoinsUn::de(Vec::<T>::deserialize(d)?).ok_or_else(|| serde::de::Error::invalid_length(0, &"au moins un élément"))
    }
}

/// UNE VALEUR QUI PEUT NE RIEN DIRE : nulle (`null`), vide (`""`), ou dite, lue en `T` sans passer par une valeur
/// intermediaire (le chemin d'un refus va jusqu'a la cle fautive de `T`). C'est la lecture d'un changement de case de
/// fiche (`droit::fiche::nulle_ou_dite`), qui refuse `""`, quand son type ne range pas sa saisie (`rangee_puis_lue`).
#[derive(Debug, Clone, PartialEq)]
pub enum Remplie<T> {
    Nulle,
    Vide,
    Dite(T),
}

impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Remplie<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::value::{BoolDeserializer, I64Deserializer, MapAccessDeserializer, SeqAccessDeserializer, StrDeserializer, U64Deserializer};
        struct Lue<T>(std::marker::PhantomData<T>);
        impl<'de, T: serde::Deserialize<'de>> serde::de::Visitor<'de> for Lue<T> {
            type Value = Remplie<T>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("une valeur, nulle ou vide")
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(Remplie::Nulle)
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(Remplie::Nulle)
            }
            fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
                d.deserialize_any(self)
            }
            fn visit_bool<E: serde::de::Error>(self, b: bool) -> Result<Self::Value, E> {
                T::deserialize(BoolDeserializer::<E>::new(b)).map(Remplie::Dite)
            }
            fn visit_i64<E: serde::de::Error>(self, i: i64) -> Result<Self::Value, E> {
                T::deserialize(I64Deserializer::<E>::new(i)).map(Remplie::Dite)
            }
            fn visit_u64<E: serde::de::Error>(self, u: u64) -> Result<Self::Value, E> {
                T::deserialize(U64Deserializer::<E>::new(u)).map(Remplie::Dite)
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
                if s.is_empty() {
                    return Ok(Remplie::Vide);
                }
                T::deserialize(StrDeserializer::<E>::new(s)).map(Remplie::Dite)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
                T::deserialize(SeqAccessDeserializer::new(a)).map(Remplie::Dite)
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(a)).map(Remplie::Dite)
            }
        }
        d.deserialize_any(Lue(std::marker::PhantomData))
    }
}

/// UNE VALEUR SAISIE RANGEE PUIS LUE (`Remplie`) : chaque texte qu'elle porte prend la forme que `rangee` lui donne
/// selon son chemin (les cles des objets qui y menent, `[]` pour la valeur elle-meme) avant d'etre lu en `T`. C'est la
/// lecture d'un changement de case de fiche dont le type range sa saisie (`droit::fiche::nulle_ou_dite`) : elle passe
/// par une valeur intermediaire, et le refus d'un texte range dit sa phrase sans le chemin sous la case.
pub fn rangee_puis_lue<'de, D, T>(d: D, rangee: &dyn Fn(&[String], &str) -> Option<String>) -> Result<Remplie<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: DeserializeOwned,
{
    use serde::Deserialize;
    let mut v = match Remplie::<Value>::deserialize(d)? {
        Remplie::Nulle => return Ok(Remplie::Nulle),
        Remplie::Vide => return Ok(Remplie::Vide),
        Remplie::Dite(v) => v,
    };
    ranger_les_textes(&mut v, &mut Vec::new(), rangee);
    // Relue depuis son texte JSON : un nombre a virgule y reste le texte ecrit (`arbitrary_precision`) ;
    // lue de la `Value` meme, il arriverait en flottant.
    let texte = serde_json::to_string(&v).map_err(serde::de::Error::custom)?;
    serde_json::from_str::<T>(&texte).map(Remplie::Dite).map_err(serde::de::Error::custom)
}

/// Chaque texte de `v`, a son chemin `chemin`, range par `rangee`.
fn ranger_les_textes(v: &mut Value, chemin: &mut Vec<String>, rangee: &dyn Fn(&[String], &str) -> Option<String>) {
    match v {
        Value::String(t) => {
            if let Some(r) = rangee(chemin, t) {
                *t = r;
            }
        }
        Value::Array(a) => {
            for x in a {
                ranger_les_textes(x, chemin, rangee);
            }
        }
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                chemin.push(k.clone());
                ranger_les_textes(x, chemin, rangee);
                chemin.pop();
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Ecrit un decimal en nombre a virgule (une grandeur en nombre JSON, un taux rendu), exact, comme les fichiers l'ecrivent
/// (`ecriture::a_virgule` : `130.0`).
pub(crate) fn ecrire_a_virgule<S: serde::Serializer>(d: &crate::num::Decimal, s: S) -> Result<S::Ok, S::Error> {
    use serde::Serialize;
    match crate::ecriture::a_virgule(&d.to_string()) {
        Some(t) => nombre_du_texte(&t).serialize(s),
        None => unreachable!("un décimal s'écrit en chiffres : {}", d),
    }
}

/// `ecrire_a_virgule`, quand la valeur peut manquer (nulle, elle s'ecrit `null`).
pub(crate) fn ecrire_a_virgule_opt<S: serde::Serializer>(d: &Option<crate::num::Decimal>, s: S) -> Result<S::Ok, S::Error> {
    match d {
        Some(d) => ecrire_a_virgule(d, s),
        None => s.serialize_none(),
    }
}

/// UNE ABSENCE QU'UN AUTRE QUE NOUS ECRIT `""` : la liste des retours de l'API de net-entreprises (un `id`, une `url`
/// non renseignes, comme `<url/>` dans sa liste XML : guide de l'API DSN v6.8,
/// `atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`), le serveur du
/// filet (le `ticket` d'un retour qui n'en a pas encore). `#[serde(default, deserialize_with =
/// "socle::forme::absent_si_vide")]` : `""` ou `null` se lit `None` ; tout autre texte se lit dans la forme de la valeur.
/// Nos propres formes n'ecrivent jamais `""` pour une absence : `null`, ou rien.
pub fn absent_si_vide<'de, T: serde::de::DeserializeOwned, D: serde::Deserializer<'de>>(d: D) -> Result<Option<T>, D::Error> {
    let Some(texte) = <Option<String> as serde::Deserialize>::deserialize(d)?.filter(|t| !t.is_empty()) else {
        return Ok(None);
    };
    T::deserialize(serde::de::IntoDeserializer::<D::Error>::into_deserializer(texte)).map(Some)
}

/// UN NOMBRE D'UNE DONNEE, garde tel qu'il s'ecrit (`130.0` reste `130.0`, `12` reste `12`) : il se relit et se
/// reecrit a l'identique. Un texte n'en est pas un.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "serde_json::Number", into = "serde_json::Number")]
pub struct NombreLu(serde_json::Number);

impl TryFrom<serde_json::Number> for NombreLu {
    type Error = std::convert::Infallible;
    fn try_from(n: serde_json::Number) -> Result<NombreLu, Self::Error> {
        Ok(NombreLu(n))
    }
}

impl From<NombreLu> for serde_json::Number {
    fn from(n: NombreLu) -> serde_json::Number {
        n.0
    }
}

impl NombreLu {
    /// Le decimal du nombre : un entier tel quel, un nombre a virgule par sa forme la plus courte (`decimal_ecrit`).
    pub(crate) fn decimal(&self) -> R<crate::num::Decimal> {
        match (self.0.as_i64(), self.0.as_u64()) {
            (Some(i), _) => Ok(i.into()),
            (_, Some(u)) => Ok(u.into()),
            (None, None) => decimal_ecrit(&self.0.to_string()),
        }
    }

    /// L'ENTIER QU'IL VAUT EXACTEMENT, ECRIT en chiffres (`12` et `12.0` : « 12 »), comme un entier lu s'ecrit (la cle
    /// d'une case d'un classeur, qui confond `12` et `12.0`) ; `None` : il n'est pas entier, ou ne tient pas en 64 bits
    /// signes ; un nombre hors de portee d'un decimal se refuse.
    pub fn entier_ecrit(&self) -> R<Option<String>> {
        use rust_decimal::prelude::ToPrimitive;
        Ok(Some(self.decimal()?).filter(|d| d.fract().is_zero()).and_then(|d| d.to_i64()).map(|n| n.to_string()))
    }

    /// SA PARTIE ENTIERE, vers zero, en compte (une longueur ecrite `12.7` : 12) ; `None` : elle est negative, ou ne
    /// tient pas en memoire ; un nombre hors de portee d'un decimal se refuse.
    pub fn compte_tronque(&self) -> R<Option<usize>> {
        use rust_decimal::prelude::ToPrimitive;
        Ok(self.decimal()?.trunc().to_usize())
    }

    /// Le nombre dans sa forme la plus courte (`12.50` : `12.5` ; « -0 » s'ecrit « 0 » : `num::nombre`) : la forme
    /// canonique d'un nombre d'un oracle ; un nombre hors de portee d'un decimal se refuse.
    pub fn sans_zero(&self) -> R<String> {
        self.decimal().map(crate::num::nombre)
    }

    /// Le nombre est-il nul (`0`, `0.0`) ?
    pub fn est_nul(&self) -> bool {
        matches!(self.decimal(), Ok(d) if d.is_zero())
    }

    /// Le nombre tel qu'une donnee l'ecrit : un entier tel quel, un nombre a virgule sous sa forme ecrite (`130.0`,
    /// `ecriture::a_virgule`).
    pub fn ecrit(&self) -> String {
        crate::ecriture::nombre_ecrit(&self.0)
    }

    /// Le nombre qu'un texte ecrit en JSON (`12`, `1.50`, `1e-5`), exact ; l'erreur dit pourquoi ce n'en est pas un.
    pub fn du_texte(t: &str) -> Result<NombreLu, serde_json::Error> {
        t.parse::<serde_json::Number>().map(NombreLu)
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.0.as_u64()
    }

    /// Un entier lu en chiffres (la case entiere d'un classeur, `lecture_xlsx`), qui s'ecrit tel quel.
    pub(crate) fn de_l_entier(n: i64) -> NombreLu {
        NombreLu(n.into())
    }
}

impl From<u64> for NombreLu {
    fn from(n: u64) -> NombreLu {
        NombreLu(n.into())
    }
}

impl std::fmt::Display for NombreLu {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Les trois cles qui expliquent un objet d'un fichier, sans rien dire au calcul : `quoi` (ce qu'il est), `note` (une
/// remarque, un pourquoi, une regle d'emploi), `source` (le texte depose qui le fonde).
pub const NOTES: [&str; 3] = ["quoi", "note", "source"];

/// UN OBJET DE FICHES ET SA NOTE (`config/salaries.json`, `config/spectacles.json`) : la note (`note`), et les fiches dans
/// l'ordre du fichier, chacune sous sa cle lue en `K`. Une cle repetee refuse. `#[serde(deserialize_with)]` ne s'y pose
/// pas : la forme l'appelle de son `Deserialize`.
pub fn fiches_et_note<'de, D, K, F>(d: D, note: &'static str) -> Result<(Option<String>, Vec<(K, F)>), D::Error>
where
    D: serde::Deserializer<'de>,
    K: serde::Deserialize<'de>,
    F: serde::Deserialize<'de>,
{
    use serde::de::IntoDeserializer;
    struct Lues<K, F>(&'static str, std::marker::PhantomData<(K, F)>);
    impl<'de, K: serde::Deserialize<'de>, F: serde::Deserialize<'de>> serde::de::Visitor<'de> for Lues<K, F> {
        type Value = (Option<String>, Vec<(K, F)>);
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("un objet de fiches")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Self::Value, A::Error> {
            let (mut note, mut fiches, mut vues) = (None, Vec::new(), std::collections::HashSet::new());
            while let Some(k) = m.next_key::<String>()? {
                if k == self.0 {
                    note = Some(m.next_value()?);
                } else if !vues.insert(k.clone()) {
                    return Err(serde::de::Error::custom(format!("la clé {} est répétée", k)));
                } else {
                    let cle = K::deserialize(k.as_str().into_deserializer())?;
                    fiches.push((cle, m.next_value()?));
                }
            }
            Ok((note, fiches))
        }
    }
    d.deserialize_map(Lues(note, std::marker::PhantomData))
}

/// `lire_en_phrase`, du fichier JSON `fichier` (sa lecture notee au registre d'un calcul suivi, `fichiers::lire_texte`) ;
/// le disque qui refuse se dit par `echec::du_disque`.
#[track_caller]
pub fn lire_fichier_en_phrase<T: DeserializeOwned>(fichier: &Path, phrase: impl Fn(&str, &str) -> String) -> R<T> {
    let texte = match crate::fichiers::lire_texte(fichier) { Ok(t) => t, Err(e) => return Err(crate::echec::du_disque(fichier, &e)) };
    let mut d = serde_json::Deserializer::from_str(&texte);
    match serde_path_to_error::deserialize(&mut d) { Ok(t) => Ok(t), Err(e) => Err(refus_en_phrase(&phrase, &e)) }
}

#[track_caller]
fn refus_en_phrase(phrase: &impl Fn(&str, &str) -> String, e: &serde_path_to_error::Error<serde_json::Error>) -> Echec {
    let chemin = e.path().to_string();
    let chemin = if chemin == "." { String::new() } else { chemin };
    let detail = en_clair(&e.inner().to_string());
    refus_avec(phrase(&chemin, &detail), Lieu { cle: Some(chemin), detail: Some(detail).filter(|d| !d.is_empty()), ..Lieu::default() })
}

// ------------------------------------------------------------------ les formes partagees

/// Une forme lue, rangee par son type.
type Forme = Arc<dyn Any + Send + Sync>;

/// Ce que chaque fichier lu a donne, par son chemin et le type de sa forme : ses octets d'alors, et sa forme.
type CacheDesFormes = Mutex<HashMap<(PathBuf, TypeId), (Arc<Vec<u8>>, Forme)>>;

/// UN FICHIER LU EN SA FORME `T`, UNE FOIS PAR CONTENU : hors d'un geste, le fichier se relit a chaque fois (ses octets),
/// et la forme qu'on en avait tiree ne sert que si ses octets sont exactement ceux d'alors. Rien ne se lit perime, quel
/// que soit qui l'a ecrit et quand (une autre fenetre, une restauration, une cle USB aux dates a deux secondes pres) :
/// seule la lecture s'epargne. PENDANT UN GESTE (`geste`), il se lit une fois : la lecture du geste le garde, et ce que
/// le geste ecrit ou efface (`ecriture::ecrire_json`, `ecriture::effacer`) se relit. `None` s'il n'existe pas. Rien ne
/// se note au registre : l'appelant dit ce qu'il lit (les engagements se comptent par leur saisie, `engagement::stock`).
/// Partage : on ne la modifie pas ; deux lectures du meme contenu rendent la meme forme (`Arc::ptr_eq`). Un JSON
/// illisible se dit par `echec::du_disque`, une forme fausse par `phrase`, la cle et le detail a part (`lire`).
#[track_caller]
pub fn partagee<T: DeserializeOwned + Send + Sync + 'static>(chemin: &Path, phrase: &str) -> R<Option<Arc<T>>> {
    static CACHE: OnceLock<CacheDesFormes> = OnceLock::new();
    let cle = (chemin.to_path_buf(), TypeId::of::<T>());
    let en_t = |f: Forme| f.downcast::<T>().map_err(|_| crate::echec::refus("Une forme partagée n'a pas le type de sa lecture : le signaler au développeur."));
    if let Some(lu) = GESTE.with(|g| g.borrow().as_ref().and_then(|l| l.get(&cle).cloned())) {
        return lu.map(en_t).transpose();
    }
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let octets = match std::fs::read(chemin) {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            garder_dans_le_geste(cle, None);
            return Ok(None);
        }
        Err(e) => return Err(crate::echec::refus(crate::fichiers::panne_fichier(&e, chemin))),
    };
    let deja = crate::memoire::tenu(cache).get(&cle).filter(|(vus, _)| **vus == octets).map(|(_, f)| f.clone());
    let f = match deja {
        Some(f) => f,
        None => {
            let f: Forme = Arc::new(des_octets::<T>(chemin, &octets, phrase)?);
            crate::fichiers::ranger(cache, cle.clone(), (Arc::new(octets), f.clone()));
            f
        }
    };
    garder_dans_le_geste(cle, Some(f.clone()));
    en_t(f).map(Some)
}

/// Les octets JSON du fichier `chemin` lus en `T` : un JSON illisible se dit par `echec::du_disque`, une forme fausse par
/// `phrase`, la cle et le detail a part.
#[track_caller]
fn des_octets<T: DeserializeOwned>(chemin: &Path, octets: &[u8], phrase: &str) -> R<T> {
    let mut d = serde_json::Deserializer::from_slice(octets);
    let t = match serde_path_to_error::deserialize(&mut d) {
        Ok(t) => t,
        Err(e) if e.inner().classify() == serde_json::error::Category::Data => return Err(refus(phrase, None, &e)),
        Err(e) => return Err(crate::echec::du_disque(&chemin, e.inner())),
    };
    match d.end() { Ok(()) => Ok(t), Err(e) => Err(crate::echec::du_disque(&chemin, &e)) }
}

thread_local! {
    /// LA LECTURE DU GESTE EN COURS (`geste`) : chaque forme que `partagee` a servie, par son chemin et son type (`None` :
    /// absent).
    static GESTE: RefCell<Option<HashMap<(PathBuf, TypeId), Option<Forme>>>> = const { RefCell::new(None) };
}

fn garder_dans_le_geste(cle: (PathBuf, TypeId), f: Option<Forme>) {
    GESTE.with(|g| {
        if let Some(l) = g.borrow_mut().as_mut() {
            l.insert(cle, f);
        }
    });
}

/// Le geste vient d'ecrire ou d'effacer `chemin` (`ecriture::ecrire_json`, `ecriture::effacer`) : ce qu'il en avait lu
/// ne vaut plus, il le relit ; un calcul sur un dossier qui le contient (`une_fois_par_geste`) se refait.
pub(crate) fn change_pendant_le_geste(chemin: &Path) {
    GESTE.with(|g| {
        if let Some(l) = g.borrow_mut().as_mut() {
            l.retain(|(p, _), _| p != chemin && !chemin.starts_with(p));
        }
    });
}

/// UN CALCUL SUR CE QUE PORTE LE DOSSIER `dossier`, FAIT UNE FOIS PAR GESTE (`geste`), rangé par son type `T` : le
/// premier appel du geste le fait, les suivants rendent le meme ; ce que le geste ecrit ou efface sous `dossier` le
/// defait (`change_pendant_le_geste`). Hors d'un geste, il se fait a chaque appel. Un calcul qui echoue ne se garde pas.
pub fn une_fois_par_geste<T: Send + Sync + 'static>(dossier: &Path, calcul: impl FnOnce() -> R<T>) -> R<Arc<T>> {
    let cle = (dossier.to_path_buf(), TypeId::of::<T>());
    if let Some(Some(f)) = GESTE.with(|g| g.borrow().as_ref().and_then(|l| l.get(&cle).cloned())) {
        return f.downcast::<T>().map_err(|_| crate::echec::refus("Un calcul du geste n'a pas le type de sa lecture : le signaler au développeur."));
    }
    let t = Arc::new(calcul()?);
    garder_dans_le_geste(cle, Some(t.clone()));
    Ok(t)
}

/// UN GESTE (une route, l'etat d'un mois refait) LIT UNE FOIS ce que la compagnie porte et que `partagee` sert (les
/// engagements, leurs paies, les imprevus) : rendu par `geste`, la lecture vit tant que son garde vit, et tombe avec
/// lui ; un geste ouvert depuis un autre lit dans la lecture de celui-ci.
pub struct Geste(bool);

#[must_use = "la lecture du geste ne vit que tant que son garde vit"]
pub fn geste() -> Geste {
    Geste(GESTE.with(|g| {
        let mut g = g.borrow_mut();
        if g.is_some() {
            return false;
        }
        *g = Some(HashMap::new());
        true
    }))
}

impl Drop for Geste {
    fn drop(&mut self) {
        if self.0 {
            GESTE.with(|g| *g.borrow_mut() = None);
        }
    }
}

/// LES PHRASES QUE REND UNE ROUTE, celles que l'ecran lit : la `phrase` (un texte), les `phrases` (une liste de textes)
/// et les `aides` (une table de textes) de la reponse et de chaque ligne qu'elle porte, a toute profondeur. Elles se
/// lisent en leur forme : une `phrase` qui n'est pas un texte refuse la lecture ; absente ou `null`, il n'y en a pas. Ce
/// qui porte un autre nom est la forme de la route, deja ecrite : il se parcourt, sans s'y lire. Le parcours d'un JSON
/// brut est une porte : il vit ici (`routes::enveloppe::Rendu` le lit).
#[derive(Clone, Debug, Default)]
pub struct PhrasesRendues {
    /// Chaque `phrase` : la sienne, celle de chaque ligne (une carte, un carre, la frise d'un engagement).
    phrase: Vec<String>,
    /// Les lignes des `phrases` et les textes des `aides`.
    autres: Vec<String>,
}

impl PhrasesRendues {
    /// Toutes, chaque `phrase` d'abord.
    pub fn toutes(&self) -> impl Iterator<Item = &str> {
        self.phrase.iter().chain(&self.autres).map(String::as_str)
    }

    /// LA PLUS LONGUE DES `phrase`, en caracteres : chacune s'affiche dans la place de la route (`Fiche::phrase`). Aucune :
    /// zero caractere.
    pub fn plus_longue(&self) -> usize {
        self.phrase.iter().map(|p| p.chars().count()).max().into_iter().sum()
    }
}

impl<'de> serde::Deserialize<'de> for PhrasesRendues {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<PhrasesRendues, D::Error> {
        let mut p = PhrasesRendues::default();
        serde::de::DeserializeSeed::deserialize(Parcours(&mut p), d)?;
        Ok(p)
    }
}

/// Le parcours d'une valeur du rendu, qui range ses phrases dans `PhrasesRendues`.
struct Parcours<'a>(&'a mut PhrasesRendues);

impl<'de> serde::de::DeserializeSeed<'de> for Parcours<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> serde::de::Visitor<'de> for Parcours<'_> {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("ce qu'une route rend")
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<(), A::Error> {
        while let Some(cle) = m.next_key::<String>()? {
            match cle.as_str() {
                "phrase" => self.0.phrase.extend(m.next_value::<Option<String>>()?),
                "phrases" => self.0.autres.extend(m.next_value::<Option<Vec<String>>>()?.into_iter().flatten()),
                "aides" => self.0.autres.extend(m.next_value::<Option<std::collections::BTreeMap<String, String>>>()?.into_iter().flatten().map(|(_, t)| t)),
                _ => m.next_value_seed(Parcours(&mut *self.0))?,
            }
        }
        Ok(())
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut s: A) -> Result<(), A::Error> {
        while s.next_element_seed(Parcours(&mut *self.0))?.is_some() {}
        Ok(())
    }

    fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_any(self)
    }

    // Une valeur seule n'est la phrase de rien.
    fn visit_none<E: serde::de::Error>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
}
