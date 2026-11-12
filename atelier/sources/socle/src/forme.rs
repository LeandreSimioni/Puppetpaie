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
/// passe qu'elle porte, les phrases qu'une reponse rend) se dit par ses textes, a une cle que l'enveloppe nomme. Son
/// JSON ne se touche qu'ici et dans `ecriture` (`ecriture::en_objet`).
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(transparent)]
pub struct Objet(pub(crate) serde_json::Map<String, Value>);

/// Un objet, et rien d'autre.
impl<'de> serde::Deserialize<'de> for Objet {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Objet, D::Error> {
        serde_json::Map::deserialize(d).map(Objet)
    }
}

/// LA FORME D'UN TEXTE QU'UN OBJET PORTE A UNE CLE (`Objet::textes_sous`) : un texte, une liste de textes, une table
/// dont les valeurs sont des textes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormeDesTextes {
    Texte,
    Liste,
    Table,
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

    /// L'objet reduit a sa cle `cle`, si elle porte un texte non vide (une cle que l'enveloppe lit en sa forme, seule).
    pub fn reduit_au_texte(&self, cle: &str) -> Option<Objet> {
        let v = self.0.get(cle).filter(|v| v.as_str().is_some_and(|t| !t.is_empty()))?;
        Some(Objet(serde_json::Map::from_iter([(cle.to_string(), v.clone())])))
    }

    /// Le texte que l'objet porte a `cle`, s'il en porte un.
    pub fn texte(&self, cle: &str) -> Option<&str> {
        self.0.get(cle).and_then(Value::as_str)
    }

    /// Les textes de la liste que l'objet porte a `cle` ; aucun s'il n'y porte pas de liste.
    pub fn textes(&self, cle: &str) -> Vec<&str> {
        self.0.get(cle).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect()
    }

    /// LES TEXTES QUE L'OBJET PORTE AUX CLES `cles`, A TOUTE PROFONDEUR : a chaque cle nommee qui porte un texte de la
    /// forme dite, ce texte (ou ceux de sa liste, ou les valeurs de sa table) ; toute autre valeur se parcourt.
    pub fn textes_sous(&self, cles: &[(&str, FormeDesTextes)]) -> Vec<&str> {
        fn de_la_valeur<'a>(v: &'a Value, cles: &[(&str, FormeDesTextes)], out: &mut Vec<&'a str>) {
            match v {
                Value::Object(o) => de_l_objet(o, cles, out),
                Value::Array(a) => a.iter().for_each(|x| de_la_valeur(x, cles, out)),
                // Une valeur seule n'est le texte de rien.
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
            }
        }
        fn de_l_objet<'a>(o: &'a serde_json::Map<String, Value>, cles: &[(&str, FormeDesTextes)], out: &mut Vec<&'a str>) {
            for (k, x) in o {
                let forme = cles.iter().find(|(c, _)| c == k).map(|(_, f)| *f);
                match (forme, x) {
                    (Some(FormeDesTextes::Texte), Value::String(t)) => out.push(t.as_str()),
                    (Some(FormeDesTextes::Liste), Value::Array(l)) => out.extend(l.iter().filter_map(Value::as_str)),
                    (Some(FormeDesTextes::Table), Value::Object(t)) => out.extend(t.values().filter_map(Value::as_str)),
                    (Some(_) | None, x) => de_la_valeur(x, cles, out),
                }
            }
        }
        let mut out = Vec::new();
        de_l_objet(&self.0, cles, &mut out);
        out
    }

    /// Ses cles, dans leur ordre.
    pub fn cles(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
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

/// UNE CLE CONNUE QUE LE LOGICIEL NE LIT PAS : une note d'un fichier (`quoi`, `note`, `source` : atelier/AGENTS.md,
/// « Chaque forme est declaree une fois »), ou une valeur que le fichier porte pour qui l'ecrit. La forme la nomme, pour
/// refuser toute cle inconnue (`deny_unknown_fields`), et n'en garde rien : son type ne porte aucune valeur
/// (`PhantomData`, que le compilateur sait vide). Elle se lit par `non_lue`, exigee :
/// `#[serde(deserialize_with = "socle::forme::non_lue")]` ; une note qui peut manquer ajoute `default`, qui ne fabrique
/// aucune valeur (`aucun_repli_ne_fabrique_une_valeur`, `moteur/tests/fichiers/structure.rs`).
pub type NonLue = std::marker::PhantomData<Rien>;

/// Ce qu'une cle non lue garde : rien (aucune valeur de ce type n'existe).
pub enum Rien {}

/// La valeur d'une cle non lue (`NonLue`) : n'importe quelle valeur JSON, parcourue et jetee.
pub fn non_lue<'de, D: serde::Deserializer<'de>>(d: D) -> Result<NonLue, D::Error> {
    <serde::de::IgnoredAny as serde::Deserialize>::deserialize(d).map(|_| std::marker::PhantomData)
}

/// UNE FORME QUE SA TABLE TYPE (`DeserializeSeed` : une saisie de fiche, que la table des cases lit case par case selon
/// la forme de chacune, `declarations::champs::Lecture`) : la donnee `v` lue selon `table`, dans son texte, comme `lire`.
#[track_caller]
fn lire_selon<S, U>(v: &Value, table: S, phrase: &str) -> R<U>
where
    S: for<'de> serde::de::DeserializeSeed<'de, Value = U>,
{
    let texte = v.to_string();
    let mut d = serde_json::Deserializer::from_str(&texte);
    let mut suivi = serde_path_to_error::Track::new();
    match table.deserialize(serde_path_to_error::Deserializer::new(&mut d, &mut suivi)) { Ok(u) => Ok(u), Err(e) => Err(refus(phrase, None, &serde_path_to_error::Error::new(suivi.path(), e))) }
}

/// UNE FORME ECRITE (`Serialize`, une fiche typee) RELUE SELON UNE TABLE : ce qu'elle ecrit, lu par `lire_selon`.
#[track_caller]
pub fn relire_selon<T, S, U>(t: &T, table: S, phrase: &str) -> R<U>
where
    T: serde::Serialize + ?Sized,
    S: for<'de> serde::de::DeserializeSeed<'de, Value = U>,
{
    lire_selon(&crate::ecriture::en_valeur(t), table, phrase)
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
fn texte_a_virgule<'de, A: serde::de::MapAccess<'de>>(m: A) -> Result<String, A::Error> {
    use serde::Deserialize;
    serde_json::Number::deserialize(serde::de::value::MapAccessDeserializer::new(m)).map(|n| n.to_string())
}

/// La cle privee sous laquelle le lecteur JSON passe un nombre a virgule a un `visit_map` (`serde_json`,
/// `arbitrary_precision`).
const CLE_DU_NOMBRE: &str = "$serde_json::private::Number";

/// CE QU'UN `visit_map` RECOIT D'UNE CASE QUI PEUT ETRE UN NOMBRE OU UN OBJET (`declarations::champs`) : un nombre a
/// virgule, ou un vrai objet, rendu entier a son lecteur (`Table`, dont la premiere cle se relit).
pub enum TableOuNombre<A> {
    Nombre(NombreLu),
    Table(Table<A>),
}

/// Un objet dont la premiere cle a deja ete lue (`table_ou_nombre`) : il la rend d'abord, puis la suite.
pub struct Table<A> {
    premiere: Option<String>,
    reste: A,
}

/// `m` est-il un nombre a virgule (sa seule cle est la cle privee du lecteur JSON), ou un objet ?
pub fn table_ou_nombre<'de, A: serde::de::MapAccess<'de>>(mut m: A) -> Result<TableOuNombre<A>, A::Error> {
    let premiere: Option<String> = m.next_key()?;
    if premiere.as_deref() == Some(CLE_DU_NOMBRE) {
        let texte: String = m.next_value()?;
        return match NombreLu::du_texte(&texte) {
            Ok(n) => Ok(TableOuNombre::Nombre(n)),
            Err(e) => Err(serde::de::Error::custom(format!("{} n'est pas un nombre ({})", texte, e))),
        };
    }
    Ok(TableOuNombre::Table(Table { premiere, reste: m }))
}

impl<'de, A: serde::de::MapAccess<'de>> serde::de::MapAccess<'de> for Table<A> {
    type Error = A::Error;
    fn next_key_seed<K: serde::de::DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, A::Error> {
        match self.premiere.take() {
            Some(cle) => seed.deserialize(serde::de::value::StringDeserializer::<A::Error>::new(cle)).map(Some),
            None => self.reste.next_key_seed(seed),
        }
    }
    fn next_value_seed<V: serde::de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, A::Error> {
        self.reste.next_value_seed(seed)
    }
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
pub fn decimal<'de, D: serde::Deserializer<'de>>(d: D) -> Result<crate::num::Decimal, D::Error> {
    struct EnDecimal;
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
            decimal_ecrit(&texte_a_virgule(m)?).map_err(|e| serde::de::Error::custom(e.phrase))
        }
        fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
            crate::num::dec_texte(s).map_err(|e| E::custom(e.phrase))
        }
    }
    d.deserialize_any(EnDecimal)
}

/// `decimal`, quand la valeur peut manquer ou etre nulle (avec `#[serde(default)]`).
pub fn decimal_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<crate::num::Decimal>, D::Error> {
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
pub fn court<'de, D: serde::Deserializer<'de>>(d: D) -> Result<crate::num::Decimal, D::Error> {
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
pub struct Court(pub crate::num::Decimal);

impl<'de> serde::Deserialize<'de> for Court {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        court(d).map(Court)
    }
}

/// Un nombre decimal, lu comme `decimal`, la ou un champ ne se pose pas (une valeur de table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nombre(pub crate::num::Decimal);

impl<'de> serde::Deserialize<'de> for Nombre {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        decimal(d).map(Nombre)
    }
}

/// UNE VALEUR QUI PEUT NE RIEN DIRE : nulle (`null`), vide (`""`), ou dite, lue en `T` sans passer par une valeur
/// intermediaire (le chemin d'un refus va jusqu'a la cle fautive de `T`). C'est la lecture d'un changement de case de
/// fiche (`droit::fiche::nulle_ou_dite`), qui refuse `""`.
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

/// UN NOMBRE TEL QU'IL EST ECRIT : un entier, un flottant, ou un texte qui se lit en decimal (`num::decimal_lu`, `12,5`
/// compris), garde tel quel pour se reecrire de meme (`droit::fiche::Montant`). Un nombre ecrit avec sa virgule se garde
/// en decimal, dans sa forme la plus courte (`num::court` : `130.0` reste `130.0`), et se reecrit en nombre a virgule
/// (`ecrire_a_virgule`).
#[derive(Debug, Clone, PartialEq)]
pub enum NombreEcrit {
    Entier(i64),
    Naturel(u64),
    Flottant(crate::num::Decimal),
    Texte(String),
}

/// Ecrit un decimal en nombre a virgule (`NombreEcrit::Flottant`, un taux rendu), exact, comme les fichiers l'ecrivent
/// (`ecriture::a_virgule` : `130.0`).
pub fn ecrire_a_virgule<S: serde::Serializer>(d: &crate::num::Decimal, s: S) -> Result<S::Ok, S::Error> {
    use serde::Serialize;
    match crate::ecriture::a_virgule(&d.to_string()) {
        Some(t) => nombre_du_texte(&t).serialize(s),
        None => unreachable!("un décimal s'écrit en chiffres : {}", d),
    }
}

/// `ecrire_a_virgule`, quand la valeur peut manquer (nulle, elle s'ecrit `null`).
pub fn ecrire_a_virgule_opt<S: serde::Serializer>(d: &Option<crate::num::Decimal>, s: S) -> Result<S::Ok, S::Error> {
    match d {
        Some(d) => ecrire_a_virgule(d, s),
        None => s.serialize_none(),
    }
}

impl<'de> serde::Deserialize<'de> for NombreEcrit {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Lu;
        impl<'de> serde::de::Visitor<'de> for Lu {
            type Value = NombreEcrit;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("un nombre")
            }
            fn visit_i64<E: serde::de::Error>(self, i: i64) -> Result<NombreEcrit, E> {
                Ok(NombreEcrit::Entier(i))
            }
            fn visit_u64<E: serde::de::Error>(self, u: u64) -> Result<NombreEcrit, E> {
                // Un naturel que `i64` tient est un entier ; au-dela, il reste naturel.
                Ok(if u <= i64::MAX as u64 { NombreEcrit::Entier(u as i64) } else { NombreEcrit::Naturel(u) })
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, m: A) -> Result<NombreEcrit, A::Error> {
                decimal_ecrit(&texte_a_virgule(m)?).map(|d| NombreEcrit::Flottant(crate::num::court(d))).map_err(|e| serde::de::Error::custom(e.phrase))
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<NombreEcrit, E> {
                match crate::num::decimal_lu(s) {
                    Some(_) => Ok(NombreEcrit::Texte(s.to_string())),
                    None => Err(E::custom(format!("{} n'est pas un nombre", crate::ecriture::json_en_ligne(s)))),
                }
            }
            fn visit_bool<E: serde::de::Error>(self, b: bool) -> Result<NombreEcrit, E> {
                Err(E::custom(format!("{} n'est pas un nombre", b)))
            }
        }
        d.deserialize_any(Lu)
    }
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
    pub fn decimal(&self) -> R<crate::num::Decimal> {
        match (self.0.as_i64(), self.0.as_u64()) {
            (Some(i), _) => Ok(i.into()),
            (_, Some(u)) => Ok(u.into()),
            (None, None) => decimal_ecrit(&self.0.to_string()),
        }
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

    /// Un decimal en nombre a virgule : celui que sa forme la plus courte ecrit (`num::court` : `130` s'ecrit `130.0`).
    pub fn de_decimal(d: crate::num::Decimal) -> NombreLu {
        let t = crate::num::court(d).to_string();
        match t.parse::<serde_json::Number>() {
            Ok(n) => NombreLu(n),
            Err(e) => unreachable!("un décimal s'écrit en nombre : {} ({})", t, e),
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        self.0.as_i64()
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.0.as_u64()
    }
}

impl From<u64> for NombreLu {
    fn from(n: u64) -> NombreLu {
        NombreLu(n.into())
    }
}

impl From<i64> for NombreLu {
    fn from(n: i64) -> NombreLu {
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

/// UNE FORME ECRITE (`Serialize` : une charge, une fiche composee) RELUE EN `T`, quand la phrase du refus nomme
/// elle-meme la cle fautive : `phrase(chemin, detail)` la compose (le fichier d'une compagnie, qu'elle corrige :
/// `droit::fiche`).
#[track_caller]
pub fn relire_en_phrase<T: DeserializeOwned>(t: &(impl serde::Serialize + ?Sized), phrase: impl Fn(&str, &str) -> String) -> R<T> {
    let texte = crate::ecriture::en_valeur(t).to_string();
    let mut d = serde_json::Deserializer::from_str(&texte);
    match serde_path_to_error::deserialize(&mut d) { Ok(t) => Ok(t), Err(e) => Err(refus_en_phrase(&phrase, &e)) }
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
/// ne vaut plus, il le relit.
pub(crate) fn change_pendant_le_geste(chemin: &Path) {
    GESTE.with(|g| {
        if let Some(l) = g.borrow_mut().as_mut() {
            l.retain(|(p, _), _| p != chemin);
        }
    });
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
