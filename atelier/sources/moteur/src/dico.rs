//! LES LECTURES D'UNE VALEUR JSON QUE LE MOTEUR FAIT A LA MAIN : `est`, `items`, et `Dico`, une table a cle composee
//! de valeurs, dans l'ordre d'insertion. Les couches fermees n'en ont pas : elles ne lisent que des formes.

use std::borrow::Cow;
use std::collections::HashMap;

use serde_json::Value;
use socle::echec::{erreur, R};

/// La valeur `v` est le texte `s`.
pub fn est(v: &Value, s: &str) -> bool {
    v.as_str() == Some(s)
}

/// `for x in v` : une liste, les cles d'un dictionnaire, les caracteres d'un texte.
pub fn items(v: &Value) -> R<Vec<Cow<'_, Value>>> {
    match v {
        Value::Array(a) => Ok(a.iter().map(Cow::Borrowed).collect()),
        Value::Object(o) => Ok(o.keys().map(|k| Cow::Owned(Value::String(k.clone()))).collect()),
        Value::String(s) => Ok(s.chars().map(|c| Cow::Owned(Value::String(c.to_string()))).collect()),
        _ => Err(erreur(socle::echec::Classe::MalFormee, socle::fichiers::attendu("une liste", crate::valeur::nom_type(v)))),
    }
}

/// La cle de hachage d'une valeur : `1`, `1.0` et `true` ne font qu'une ; une liste ou un dictionnaire ne se hache pas.
fn cle_de_hachage(v: &Value) -> R<String> {
    match v {
        Value::Null => Ok("N".into()),
        Value::String(t) => Ok(format!("s{}:{}", t.len(), t)),
        Value::Array(_) => Err(erreur(socle::echec::Classe::MalFormee, socle::fichiers::pas_une_cle("list"))),
        Value::Object(_) => Err(erreur(socle::echec::Classe::MalFormee, socle::fichiers::pas_une_cle("dict"))),
        autre => {
            let x = crate::valeur::nombre(autre).unwrap_or(0.0);
            Ok(format!("n{:?}", if x == 0.0 { 0.0 } else { x }))
        }
    }
}

/// Une table (ou un ensemble) a cle composee, dans l'ordre d'insertion : reposer
/// une cle change la valeur et garde la place.
pub struct Dico<V> {
    pub index: HashMap<String, usize>,
    pub items: Vec<(Vec<Value>, V)>,
}

impl<V> Dico<V> {
    pub fn nouveau() -> Self {
        Dico { index: HashMap::new(), items: Vec::new() }
    }

    pub fn cle(k: &[Value]) -> R<String> {
        let mut t = String::new();
        for x in k {
            t.push_str(&cle_de_hachage(x)?);
            t.push('\u{1f}');
        }
        Ok(t)
    }

    pub fn get(&self, k: &[Value]) -> R<Option<&V>> {
        Ok(self.index.get(&Self::cle(k)?).map(|&i| &self.items[i].1))
    }

    pub fn get_mut(&mut self, k: &[Value]) -> R<Option<&mut V>> {
        let i = self.index.get(&Self::cle(k)?).copied();
        Ok(i.map(move |i| &mut self.items[i].1))
    }

    pub fn contient(&self, k: &[Value]) -> R<bool> {
        Ok(self.index.contains_key(&Self::cle(k)?))
    }

    pub fn poser(&mut self, k: Vec<Value>, v: V) -> R<()> {
        let c = Self::cle(&k)?;
        match self.index.get(&c) {
            Some(&i) => self.items[i].1 = v,
            None => {
                self.index.insert(c, self.items.len());
                self.items.push((k, v));
            }
        }
        Ok(())
    }

    pub fn sortie(&self, f: impl Fn(&V) -> Value) -> Value {
        Value::Array(
            self.items
                .iter()
                .map(|(k, v)| {
                    let mut l = k.clone();
                    l.push(f(v));
                    Value::Array(l)
                })
                .collect(),
        )
    }

    pub fn cles(&self) -> Value {
        Value::Array(self.items.iter().map(|(k, _)| Value::Array(k.clone())).collect())
    }
}
