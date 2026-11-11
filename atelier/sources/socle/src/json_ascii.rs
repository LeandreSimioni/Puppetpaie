//! Le JSON ASCII et compact qu'on scelle ou qu'on envoie a un service : `\uXXXX` hors ASCII, cles triees a la demande,
//! separateurs `, ` et `: ` (ou `,` et `:` serres) ; les nombres a virgule comme les fichiers les ecrivent
//! (`ecriture::a_virgule`). Les octets qu'un coffre deja scelle ou qu'une signature couvre ne changent pas d'une version
//! a l'autre.
//!
//! Un des trois modules du BORD (`forme` lit ; `ecriture` et `json_ascii` ecrivent) : ce qui s'ecrit est une forme.

use serde_json::Value;

/// Le JSON en ASCII (tout autre caractere en `\uXXXX`), cles triees si `trie`. Sans
/// `indent`, les separateurs sont `, ` et `: ` (ou `,` et `:` si `serre`).
pub fn dumps<T: serde::Serialize + ?Sized>(v: &T, indent: Option<usize>, serre: bool, trie: bool) -> String {
    let mut s = String::new();
    ecrire(&crate::ecriture::en_valeur(v), indent, serre, trie, 0, &mut s);
    s
}

fn chaine_json(t: &str, s: &mut String) {
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            '\u{8}' => s.push_str("\\b"),
            '\u{c}' => s.push_str("\\f"),
            ' '..='~' => s.push(c),
            c => {
                let mut tampon = [0u16; 2];
                for u in c.encode_utf16(&mut tampon) {
                    s.push_str(&format!("\\u{:04x}", u));
                }
            }
        }
    }
    s.push('"');
}

fn ecrire(v: &Value, indent: Option<usize>, serre: bool, trie: bool, niveau: usize, s: &mut String) {
    let (sep_item, sep_cle) = match (indent, serre) {
        (Some(_), _) => (",", ": "),
        (None, true) => (",", ":"),
        (None, false) => (", ", ": "),
    };
    let retrait = |n: usize| -> String { indent.into_iter().map(|i| format!("\n{}", " ".repeat(i * n))).collect() };
    match v {
        Value::Null => s.push_str("null"),
        Value::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => s.push_str(&crate::ecriture::nombre_ecrit(n)),
        Value::String(t) => chaine_json(t, s),
        Value::Array(a) => {
            if a.is_empty() {
                s.push_str("[]");
                return;
            }
            s.push('[');
            s.push_str(&retrait(niveau + 1));
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push_str(sep_item);
                    s.push_str(&retrait(niveau + 1));
                }
                ecrire(x, indent, serre, trie, niveau + 1, s);
            }
            s.push_str(&retrait(niveau));
            s.push(']');
        }
        Value::Object(o) => {
            if o.is_empty() {
                s.push_str("{}");
                return;
            }
            let mut entrees: Vec<(&String, &Value)> = o.iter().collect();
            if trie {
                entrees.sort_by(|a, b| a.0.cmp(b.0));
            }
            s.push('{');
            s.push_str(&retrait(niveau + 1));
            for (i, (k, x)) in entrees.into_iter().enumerate() {
                if i > 0 {
                    s.push_str(sep_item);
                    s.push_str(&retrait(niveau + 1));
                }
                chaine_json(k, s);
                s.push_str(sep_cle);
                ecrire(x, indent, serre, trie, niveau + 1, s);
            }
            s.push_str(&retrait(niveau));
            s.push('}');
        }
    }
}
