//! CE QUE LES ETATS LISENT ENCORE PAR CLE (`etats_paie`, `etats_compta`, `etats_personnel`, `plan_comptable`,
//! `tableaux_export`, `ecritures_comptables`) : leurs seuls lecteurs. Les pieces ne lisent que des formes ; la lecture
//! par cle des couches du dessus est `declarations::valeur`. Ce module tombe avec le dernier JSON des etats.

use std::path::Path;
use std::sync::OnceLock;

use rust_decimal::prelude::*;
use serde_json::{Map, Value};

use socle::date::{repr_flottant, repr_texte};
use socle::echec::{erreur, Echec, R};
use socle::fichiers::{attendu, lire_texte_echec, repr};
use socle::num::{dec_f64, dec_texte, Resultat};

/// Le flottant d'un nombre JSON : tout nombre JSON en a un.
fn en_f64(n: &serde_json::Number) -> Resultat<f64> {
    n.as_f64().ok_or_else(|| format!("nombre illisible : {}", n))
}

/// La forme ecrite d'une valeur dans un message : objet, liste, texte cite, nombre, `True`, `None`.
pub fn forme_ecrite(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => i.to_string(),
            (_, Some(u), _) => u.to_string(),
            (_, _, Some(x)) => repr_flottant(x),
            _ => n.to_string(),
        },
        Value::String(s) => repr_texte(s),
        Value::Array(a) => format!("[{}]", a.iter().map(forme_ecrite).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter().map(|(k, v)| format!("{}: {}", repr_texte(k), forme_ecrite(v))).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// Un fichier JSON (`lire_texte_echec`, puis `JSONDecodeError` s'il ne se lit pas : la phrase en mots, le chemin et ce
/// qui ne va pas a part, `echec::du_disque`).
pub fn lire_json_echec(p: &Path) -> Result<Value, Echec> {
    serde_json::from_str(&lire_texte_echec(p)?).map_err(|e| erreur("JSONDecodeError", socle::echec::du_disque(p, &e)))
}

/// Le decimal d'un nombre JSON (entier ou flottant, par sa forme ecrite la plus courte) ou d'une chaine.
pub fn dec(v: &Value) -> Resultat<Decimal> {
    match v {
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => Ok(Decimal::from(i)),
            (_, Some(u)) => Ok(Decimal::from(u)),
            _ => dec_f64(en_f64(n)?),
        },
        Value::String(s) => dec_texte(s),
        autre => Err(format!("nombre attendu, lu {}", autre)),
    }
}

/// Une valeur est-elle vraie ? Faux : `null`, `false`, zero, un texte, une liste ou un objet vides.
pub fn vrai(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_none_or(|x| x != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Le nom du type d'une valeur, tel que les phrases de refus le disent (`socle::fichiers::attendu`).
pub fn nom_type(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

/// La forme ecrite d'une valeur : `None`, `True`, un texte entre apostrophes, une liste entre crochets.
fn valeur_ecrite(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => i.to_string(),
            (_, Some(u), _) => u.to_string(),
            (_, _, Some(x)) => socle::ecriture::repr_flottant(x),
            _ => n.to_string(),
        },
        Value::String(s) => repr(s),
        Value::Array(a) => format!("[{}]", a.iter().map(valeur_ecrite).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter().map(|(k, v)| format!("{}: {}", repr(k), valeur_ecrite(v))).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// Une valeur en texte : un texte tel quel, le reste sous sa forme ecrite (`valeur_ecrite`).
pub fn valeur_en_texte(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        autre => valeur_ecrite(autre),
    }
}

/// `{}`, a rendre par reference.
pub fn objet_vide() -> &'static Value {
    static V: OnceLock<Value> = OnceLock::new();
    V.get_or_init(|| Value::Object(Map::new()))
}

/// `v.get(k)`, ou `defaut` ; sur autre chose qu'un dictionnaire, `AttributeError`.
pub fn getd<'a>(v: &'a Value, k: &str, defaut: &'a Value) -> R<&'a Value> {
    match v {
        Value::Object(o) => Ok(o.get(k).unwrap_or(defaut)),
        _ => Err(erreur("AttributeError", attendu("un objet", nom_type(v)))),
    }
}

/// `v or defaut`.
pub fn ou<'a>(v: &'a Value, defaut: &'a Value) -> &'a Value {
    if vrai(v) {
        v
    } else {
        defaut
    }
}
