//! Un fichier de donnees JSON s'ecrit toujours aux memes octets, sur toute machine : c'est ce qui permet de comparer
//! les fichiers a l'octet dans le Drive et dans git, et ceux deja ecrits ne changent pas.
//!
//! La forme : une indentation de 2 (`", "`, `": "`, `[]` et `{}` vides), du texte non echappe hors ASCII, une fin de
//! ligne LF ; les flottants sous leur forme la plus courte qui revient au meme nombre, l'exposant signe sur deux
//! chiffres (`1e+16`, `1e-05`) et la decimale gardee (`100.0`). Un entier lu hors de 64 bits devient un flottant a la
//! lecture (pas d'`arbitrary_precision`) : il s'ecrit alors comme le flottant qu'il est devenu.
//!
//! C'est un des deux modules du BORD (`forme` lit ; `ecriture` ecrit) : ce qui s'ecrit est une forme
//! (`serde::Serialize`), que le module seul met en JSON (le JSON ASCII aussi, que `json_ascii` demande ici).

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::echec::R;

/// La forme `v` en valeur JSON, pour l'ecrire. Toute forme du logiciel s'ecrit : ses `Serialize` sont ceux de serde
/// (derives), et les cles de ses tables sont des textes, des nombres ou des variantes unitaires. Une forme que serde
/// n'ecrirait pas est un defaut du logiciel : le moteur s'arrete en la nommant.
pub fn en_valeur<T: Serialize + ?Sized>(v: &T) -> Value {
    match serde_json::to_value(v) {
        Ok(valeur) => valeur,
        Err(e) => unreachable!("une forme du logiciel ne s'écrit pas en JSON ({}) : {}", std::any::type_name::<T>(), e),
    }
}

/// CE QU'UNE CASE ECRIT, tel que son fichier l'ecrit : rien (nulle), un texte, un nombre (son ecriture), oui ou non, une
/// structure (un objet ou une liste) qui porte quelque chose, ou une structure sans rien dedans (`{}`, `[]` : une liste de
/// jours qui n'en coche aucun). C'est la valeur typee dans son ecriture, pas un texte tape : ce que lisent les regles de
/// `champs-base.json`, ecrites dans la meme ecriture (`declarations::champs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotEcrit {
    Vide,
    Mot(String),
    /// Un nombre, tel qu'il s'ecrit : les regles de forme d'un texte (`champs-base.json`) ne s'y appliquent pas.
    Nombre(String),
    OuiNon(bool),
    Structure,
    StructureVide,
}

impl MotEcrit {
    /// Le mot ecrit, s'il en est un (un texte ou un nombre).
    pub fn mot(&self) -> Option<&str> {
        match self {
            MotEcrit::Mot(m) | MotEcrit::Nombre(m) => Some(m),
            MotEcrit::Vide | MotEcrit::OuiNon(_) | MotEcrit::Structure | MotEcrit::StructureVide => None,
        }
    }
}

/// CE QU'ECRIT LA VALEUR `v` (`MotEcrit`), par la seule ecriture du logiciel (`en_valeur`).
pub fn mot<T: Serialize + ?Sized>(v: &T) -> MotEcrit {
    mot_de_la_valeur(&en_valeur(v))
}

/// CE QU'ECRIT LA CASE AU BOUT DE `chemin` DANS LA VALEUR `v` (les cles des structures qui y menent) : vide si une
/// structure du chemin ne la porte pas.
pub fn mot_au_chemin<T: Serialize + ?Sized>(v: &T, chemin: &[&str]) -> MotEcrit {
    let valeur = en_valeur(v);
    let mut ici = &valeur;
    for cle in chemin {
        match ici.get(*cle) {
            Some(x) => ici = x,
            None => return MotEcrit::Vide,
        }
    }
    mot_de_la_valeur(ici)
}

/// LA VALEUR D'UNE CASE D'UNE FICHE, ecrite comme son fichier l'ecrit (la valeur typee, rien de tape) : ce que la boite
/// « Ce qui change » et l'historique d'une fiche montrent.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ValeurDeCase(Value);

impl ValeurDeCase {
    /// La valeur `v` ecrite ; nulle, aucune.
    pub fn de<T: Serialize + ?Sized>(v: &T) -> Option<ValeurDeCase> {
        let valeur = en_valeur(v);
        (!valeur.is_null()).then_some(ValeurDeCase(valeur))
    }

    /// Un texte ecrit tel quel (une ligne d'un autre etablissement, dite en mots).
    pub fn texte(t: String) -> ValeurDeCase {
        ValeurDeCase(Value::String(t))
    }

    /// Ce qu'elle ecrit (`MotEcrit`).
    pub fn ecrite(&self) -> MotEcrit {
        mot_de_la_valeur(&self.0)
    }

    /// Les cases d'une structure, chacune avec sa valeur (une case nulle n'en a pas) ; autre chose qu'une structure, aucune.
    pub fn cases(&self) -> Option<Vec<(String, Option<ValeurDeCase>)>> {
        match &self.0 {
            Value::Object(o) => Some(o.iter().map(|(k, x)| (k.clone(), (!x.is_null()).then(|| ValeurDeCase(x.clone())))).collect()),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => None,
        }
    }

    /// Une liste : chacun de ses elements.
    pub fn elements(&self) -> Option<Vec<ValeurDeCase>> {
        match &self.0 {
            Value::Array(l) => Some(l.iter().cloned().map(ValeurDeCase).collect()),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_) => None,
        }
    }

    /// Le nombre qu'elle dit : un nombre, ou un texte qui en ecrit un (`num::dec_texte`).
    pub fn nombre(&self) -> Option<crate::num::Decimal> {
        let ecrit = match &self.0 {
            Value::Number(n) => nombre_ecrit(n),
            Value::String(t) => t.clone(),
            Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => return None,
        };
        match crate::num::dec_texte(&ecrit) {
            Ok(d) => Some(d),
            Err(_) => None,
        }
    }

    /// LA VALEUR EN MOTS, pour une phrase : un texte, un nombre ecrit, oui ou non, des mots separes, les cases d'une
    /// structure (« cle valeur »), le nombre de lignes d'une liste de structures.
    pub fn en_mots(&self) -> String {
        fn mots(v: &Value) -> String {
            match v {
                Value::Null => "vide".to_string(),
                Value::String(t) if t.trim().is_empty() => "vide".to_string(),
                Value::String(t) => t.clone(),
                Value::Number(n) => nombre_ecrit(n),
                Value::Bool(b) => (if *b { "oui" } else { "non" }).to_string(),
                Value::Array(l) if l.iter().any(Value::is_object) => format!("{} lignes", l.len()),
                Value::Array(l) => l.iter().map(mots).collect::<Vec<_>>().join(", "),
                Value::Object(o) => o.iter().map(|(k, x)| format!("{} {}", k, mots(x))).collect::<Vec<_>>().join(", "),
            }
        }
        mots(&self.0)
    }
}

fn mot_de_la_valeur(v: &Value) -> MotEcrit {
    match v {
        Value::Null => MotEcrit::Vide,
        Value::String(t) => MotEcrit::Mot(t.clone()),
        Value::Number(n) => MotEcrit::Nombre(nombre_ecrit(n)),
        Value::Bool(b) => MotEcrit::OuiNon(*b),
        Value::Array(l) if l.is_empty() => MotEcrit::StructureVide,
        Value::Object(o) if o.is_empty() => MotEcrit::StructureVide,
        Value::Array(_) | Value::Object(_) => MotEcrit::Structure,
    }
}

/// LA FORME `t` ECRITE EN OBJET (`forme::Objet`) : `None` si elle s'ecrit autre chose qu'un objet.
pub fn en_objet<T: Serialize + ?Sized>(t: &T) -> Option<crate::forme::Objet> {
    match en_valeur(t) {
        Value::Object(m) => Some(crate::forme::Objet(m)),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => None,
    }
}

/// LE JSON ASCII DE `v` (`json_ascii::dumps`) : tout autre caractere en `\uXXXX`, cles triees si `trie` ; sans `indent`,
/// les separateurs sont `, ` et `: ` (ou `,` et `:` si `serre`).
pub(crate) fn en_ascii<T: Serialize + ?Sized>(v: &T, indent: Option<usize>, serre: bool, trie: bool) -> String {
    let mut s = String::new();
    ecrire_ascii(&en_valeur(v), indent, serre, trie, 0, &mut s);
    s
}

fn chaine_ascii(t: &str, s: &mut String) {
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

fn ecrire_ascii(v: &Value, indent: Option<usize>, serre: bool, trie: bool, niveau: usize, s: &mut String) {
    let (sep_item, sep_cle) = match (indent, serre) {
        (Some(_), _) => (",", ": "),
        (None, true) => (",", ":"),
        (None, false) => (", ", ": "),
    };
    let retrait = |n: usize| -> String { indent.into_iter().map(|i| format!("\n{}", " ".repeat(i * n))).collect() };
    match v {
        Value::Null => s.push_str("null"),
        Value::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => s.push_str(&nombre_ecrit(n)),
        Value::String(t) => chaine_ascii(t, s),
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
                ecrire_ascii(x, indent, serre, trie, niveau + 1, s);
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
                chaine_ascii(k, s);
                s.push_str(sep_cle);
                ecrire_ascii(x, indent, serre, trie, niveau + 1, s);
            }
            s.push_str(&retrait(niveau));
            s.push('}');
        }
    }
}

/// Le JSON de `v`, indente de `indent`, sous la forme du module.
pub fn json_indente<T: Serialize + ?Sized>(v: &T, indent: usize) -> String {
    let mut s = String::new();
    ecrire(&en_valeur(v), indent, 0, &mut s);
    s
}

/// UN OBJET DONT L'ORDRE COMPTE, ecrit de ses paires `(cle, valeur)` dans leur ordre : le pendant de `forme::en_ordre`,
/// `#[serde(serialize_with = "socle::ecriture::en_ordre")]` sur un `Vec<(K, T)>`.
pub fn en_ordre<S: serde::Serializer, K: Serialize, T: Serialize>(paires: &[(K, T)], s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    let mut m = s.serialize_map(Some(paires.len()))?;
    for (k, v) in paires {
        m.serialize_entry(k, v)?;
    }
    m.end()
}

/// UN DECIMAL QU'UNE REPONSE REND EN TEXTE (un montant au centime, des heures) : son texte tel quel (`12.50`, `-3`),
/// `#[serde(serialize_with = "socle::ecriture::decimal_en_texte::serialize")]`. Le decimal arrive deja arrondi ou
/// normalise : l'ecriture ne change pas son echelle. `option` : `None` s'ecrit `null`.
pub mod decimal_en_texte {
    pub fn serialize<S: serde::Serializer>(d: &crate::num::Decimal, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(d)
    }

    pub mod option {
        pub fn serialize<S: serde::Serializer>(d: &Option<crate::num::Decimal>, s: S) -> Result<S::Ok, S::Error> {
            match d {
                Some(d) => super::serialize(d, s),
                None => s.serialize_none(),
            }
        }
    }
}

/// Le JSON de `v` sur une ligne : separateurs `, ` et
/// `: `, cles triees (par point de code), non-ASCII laisse tel quel.
pub fn json_trie<T: Serialize + ?Sized>(v: &T) -> String {
    let mut s = String::new();
    ecrire_trie(&en_valeur(v), &mut s);
    s
}

/// Le JSON de `v` sur une ligne, serre, tel que serde l'ecrit : une ligne d'un journal (JSONL), ou ce qu'un refus porte
/// a part (`Echec::en_lignes`).
pub fn json_en_ligne<T: Serialize + ?Sized>(v: &T) -> String {
    // Toute forme du logiciel s'ecrit (`en_valeur`).
    match serde_json::to_string(v) {
        Ok(texte) => texte,
        Err(e) => unreachable!("une forme du logiciel ne s'écrit pas en JSON ({}) : {}", std::any::type_name::<T>(), e),
    }
}

/// LA FORME `v` ECRITE SANS CE QUI NE CHANGE RIEN A CE QU'ELLE DIT, pour la comparer ou en tirer une empreinte : les cles
/// triees, un nombre lu en decimal exact et ecrit dans sa seule forme (`decimal_canonique` : `180` et `180.0` sont le
/// meme taux, la page ecrit l'un, un fichier l'autre ; `0.1000000000000000001` n'est pas `0.1`), un nombre ecrit en
/// texte comme ce nombre.
pub fn canonique<T: Serialize + ?Sized>(v: &T) -> String {
    canonique_de(&en_valeur(v))
}

/// `canonique`, sans les listes vides : une ligne sans `planning` et une ligne au planning vide disent la meme chose.
pub fn comparable<T: Serialize + ?Sized>(v: &T) -> String {
    fn sans_vides(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(m.iter().filter(|(_, x)| !x.as_array().is_some_and(Vec::is_empty)).map(|(k, x)| (k.clone(), sans_vides(x))).collect()),
            Value::Array(a) => Value::Array(a.iter().map(sans_vides).collect()),
            autre => autre.clone(),
        }
    }
    canonique_de(&sans_vides(&en_valeur(v)))
}

/// Un texte qui ecrit un nombre decimal : un `-` au plus, en tete, un `.` au plus, des chiffres, au moins un.
fn nombre_en_texte(t: &str) -> bool {
    let corps = match t.strip_prefix('-') {
        Some(c) => c,
        None => t,
    };
    corps.bytes().any(|c| c.is_ascii_digit()) && corps.bytes().all(|c| c.is_ascii_digit() || c == b'.') && corps.bytes().filter(|c| *c == b'.').count() <= 1
}

fn canonique_de(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let mut cles: Vec<&String> = m.keys().collect();
            cles.sort();
            let corps: Vec<String> = cles.iter().map(|k| format!("{}:{}", Value::String((*k).clone()), canonique_de(&m[*k]))).collect();
            format!("{{{}}}", corps.join(","))
        }
        Value::Array(a) => format!("[{}]", a.iter().map(canonique_de).collect::<Vec<_>>().join(",")),
        // Un nombre, tel qu'il est ecrit : le JSON le garde en texte exact.
        Value::Number(n) => decimal_canonique(&n.to_string()),
        // Un nombre ecrit en texte : un signe au plus, en tete, un point au plus, et au moins un chiffre.
        Value::String(t) if nombre_en_texte(t) => decimal_canonique(t),
        autre => autre.to_string(),
    }
}

/// LA SEULE FORME D'UN DECIMAL ECRIT (`-`, des chiffres, un `.`, un exposant `e` signe ou non, comme `nombre_en_texte`, un
/// entier ou `{:e}` l'ecrivent), lu exactement, chiffre a chiffre, sans flottant ni borne : `<signe><chiffres>e<exposant>`,
/// sans zero en tete ni en queue des chiffres (`180.0` et `1.8e2` : `18e1` ; `-0.050` : `-5e-2`) ; zero, de tout signe,
/// s'ecrit `0`.
fn decimal_canonique(t: &str) -> String {
    match chiffres_et_puissance(t) {
        Some((_, chiffres, _)) if chiffres.is_empty() => "0".to_string(),
        Some((moins, chiffres, puissance)) => format!("{}{}e{}", if moins { "-" } else { "" }, chiffres, puissance - chiffres.len() as i64),
        None => unreachable!("{} : un decimal ecrit se lit en chiffres", t),
    }
}

fn ecrire_trie(v: &Value, s: &mut String) {
    match v {
        Value::Array(a) => {
            s.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                ecrire_trie(x, s);
            }
            s.push(']');
        }
        Value::Object(o) => {
            let mut entrees: Vec<(&String, &Value)> = o.iter().collect();
            entrees.sort_by(|a, b| a.0.cmp(b.0));
            s.push('{');
            for (i, (k, x)) in entrees.into_iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                chaine(k, s);
                s.push_str(": ");
                ecrire_trie(x, s);
            }
            s.push('}');
        }
        autre => ecrire(autre, 0, 0, s),
    }
}

fn marge(s: &mut String, indent: usize, niveau: usize) {
    s.push('\n');
    for _ in 0..indent * niveau {
        s.push(' ');
    }
}

fn ecrire(v: &Value, indent: usize, niveau: usize, s: &mut String) {
    match v {
        Value::Null => s.push_str("null"),
        Value::Bool(true) => s.push_str("true"),
        Value::Bool(false) => s.push_str("false"),
        Value::Number(n) => s.push_str(&nombre_ecrit(n)),
        Value::String(t) => chaine(t, s),
        Value::Array(a) if a.is_empty() => s.push_str("[]"),
        Value::Object(o) if o.is_empty() => s.push_str("{}"),
        Value::Array(a) => {
            s.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                marge(s, indent, niveau + 1);
                ecrire(x, indent, niveau + 1, s);
            }
            marge(s, indent, niveau);
            s.push(']');
        }
        Value::Object(o) => {
            s.push('{');
            for (i, (k, x)) in o.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                marge(s, indent, niveau + 1);
                chaine(k, s);
                s.push_str(": ");
                ecrire(x, indent, niveau + 1, s);
            }
            marge(s, indent, niveau);
            s.push('}');
        }
    }
}

/// Une chaine JSON, non-ASCII laisse tel quel : `"` `\` `\b` `\f` `\n` `\r`
/// `\t` nommes, les autres controles en `\u00xx`.
fn chaine(t: &str, s: &mut String) {
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\u{8}' => s.push_str("\\b"),
            '\u{c}' => s.push_str("\\f"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if (c as u32) < 0x20 => s.push_str(&format!("\\u{:04x}", c as u32)),
            c => s.push(c),
        }
    }
    s.push('"');
}

/// UN NOMBRE DECIMAL ECRIT EN TEXTE (`-`, des chiffres, un `.`, un exposant `e` signe ou non), lu exactement : son
/// signe, ses chiffres significatifs (sans zero en tete ni en queue, vides pour zero) et la puissance de dix de leur
/// premier chiffre plus un (`12.5` : `125`, 2 ; `0.05` : `5`, -1). `None` : le texte n'ecrit pas un nombre.
fn chiffres_et_puissance(t: &str) -> Option<(bool, String, i64)> {
    let d = decouper(t)?;
    let tete = d.chiffres.len() - d.chiffres.trim_start_matches('0').len();
    let significatifs = d.chiffres.trim_matches('0').to_string();
    let puissance = d.exposant + d.entiers as i64 - tete as i64;
    Some((d.moins, significatifs, puissance))
}

/// UN NOMBRE DECIMAL ECRIT EN TEXTE, decoupe (`decouper`) : son signe, tous ses chiffres a la suite (ceux d'avant la
/// virgule puis ceux d'apres), combien sont avant la virgule, et l'exposant de dix ecrit apres `e` (aucun : 0).
pub(crate) struct Decoupe {
    pub(crate) moins: bool,
    pub(crate) chiffres: String,
    pub(crate) entiers: usize,
    pub(crate) exposant: i64,
}

/// LE SEUL LECTEUR D'UN NOMBRE DECIMAL ECRIT EN TEXTE (`-` ou `+`, des chiffres, un `.` au plus, un exposant `e` signe
/// ou non), exact, chiffre a chiffre : ce qu'en font l'ecriture d'un nombre (`chiffres_et_puissance`) et un instant
/// (`paris::Moment::lu`). `None` : le texte n'ecrit pas un nombre (aucun chiffre, un autre signe, un exposant qui ne
/// tient pas sur 64 bits).
pub(crate) fn decouper(t: &str) -> Option<Decoupe> {
    let (moins, corps) = match t.strip_prefix('-') {
        Some(c) => (true, c),
        None => (false, sans_plus(t)),
    };
    let (mantisse, exposant) = match corps.split_once(['e', 'E']) {
        Some((m, e)) => (m, exposant_ecrit(e)?),
        None => (corps, 0),
    };
    let chiffres: String = mantisse.chars().filter(|c| *c != '.').collect();
    if chiffres.is_empty() || mantisse.matches('.').count() > 1 || !chiffres.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // Sans virgule, tous les chiffres sont avant elle.
    let entiers = match mantisse.find('.') {
        Some(virgule) => virgule,
        None => mantisse.len(),
    };
    Some(Decoupe { moins, chiffres, entiers, exposant })
}

/// Le texte sans son `+` de tete, s'il en a un.
fn sans_plus(t: &str) -> &str {
    match t.strip_prefix('+') {
        Some(c) => c,
        None => t,
    }
}

/// L'exposant ecrit apres le `e` d'un nombre (`5`, `+5`, `-05`) ; `None` : ce n'est pas un entier de 64 bits.
fn exposant_ecrit(e: &str) -> Option<i64> {
    let (moins, chiffres) = match e.strip_prefix('-') {
        Some(c) => (true, c),
        None => (false, sans_plus(e)),
    };
    if chiffres.is_empty() || !chiffres.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n = chiffres.bytes().try_fold(0i64, |n, c| n.checked_mul(10)?.checked_add(i64::from(c - b'0')))?;
    Some(if moins { -n } else { n })
}

/// UN NOMBRE A VIRGULE TEL QUE LES FICHIERS L'ECRIVENT, du texte exact d'un nombre (`None` : ce n'en est pas un) : ses
/// chiffres significatifs, en notation positionnelle pour 1e-4 <= |x| < 1e16, une decimale au moins (`130.0`, `0.0001`),
/// sinon `d.ddde+XX` (exposant sur deux chiffres au moins : `1e-05`, `1.5e+16`).
pub fn a_virgule(t: &str) -> Option<String> {
    let (moins, chiffres, decpt) = chiffres_et_puissance(t)?;
    let signe = if moins { "-" } else { "" };
    if chiffres.is_empty() {
        return Some(format!("{}0.0", signe));
    }
    let n = chiffres.len() as i64;
    let exp = decpt - 1;
    let corps = if decpt <= -4 || decpt > 16 {
        let mut m = chiffres[..1].to_string();
        if n > 1 {
            m.push('.');
            m.push_str(&chiffres[1..]);
        }
        format!("{}e{}{:02}", m, if exp < 0 { '-' } else { '+' }, exp.abs())
    } else if decpt <= 0 {
        format!("0.{}{}", "0".repeat((-decpt) as usize), chiffres)
    } else if decpt >= n {
        format!("{}{}.0", chiffres, "0".repeat((decpt - n) as usize))
    } else {
        format!("{}.{}", &chiffres[..decpt as usize], &chiffres[decpt as usize..])
    };
    Some(format!("{}{}", signe, corps))
}

/// UNE FORME ECRITE, VUE EN ARBRE (ce que `en_valeur` en fait), pour qui la lit sans connaitre son type : la trace d'un
/// diagnostic, une case de fiche confrontee a une condition. Un nombre y est son texte ecrit (`nombre_ecrit`).
#[derive(Debug, Clone, PartialEq)]
pub enum Arbre {
    Nul,
    Booleen(bool),
    Nombre(String),
    Texte(String),
    Liste(Vec<Arbre>),
    Table(Vec<(String, Arbre)>),
}

impl Arbre {
    /// L'arbre de ce que `v` ecrit.
    pub fn de<T: Serialize + ?Sized>(v: &T) -> Arbre {
        fn de_valeur(v: Value) -> Arbre {
            match v {
                Value::Null => Arbre::Nul,
                Value::Bool(b) => Arbre::Booleen(b),
                Value::Number(n) => Arbre::Nombre(nombre_ecrit(&n)),
                Value::String(t) => Arbre::Texte(t),
                Value::Array(a) => Arbre::Liste(a.into_iter().map(de_valeur).collect()),
                Value::Object(o) => Arbre::Table(o.into_iter().map(|(k, x)| (k, de_valeur(x))).collect()),
            }
        }
        de_valeur(en_valeur(v))
    }
}

impl Serialize for Arbre {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::{SerializeMap, SerializeSeq};
        match self {
            Arbre::Nul => s.serialize_unit(),
            Arbre::Booleen(b) => s.serialize_bool(*b),
            Arbre::Nombre(t) => crate::forme::nombre_du_texte(t).serialize(s),
            Arbre::Texte(t) => s.serialize_str(t),
            Arbre::Liste(a) => {
                let mut l = s.serialize_seq(Some(a.len()))?;
                for x in a {
                    l.serialize_element(x)?;
                }
                l.end()
            }
            Arbre::Table(o) => {
                let mut m = s.serialize_map(Some(o.len()))?;
                for (k, x) in o {
                    m.serialize_entry(k, x)?;
                }
                m.end()
            }
        }
    }
}

/// Le texte d'un nombre JSON tel que les fichiers l'ecrivent : un entier tel quel, un nombre a virgule par `a_virgule`.
pub fn nombre_ecrit(n: &serde_json::Number) -> String {
    let t = n.to_string();
    if n.is_i64() || n.is_u64() {
        return t;
    }
    match a_virgule(&t) {
        Some(e) => e,
        None => unreachable!("un nombre JSON s'ecrit en chiffres : {}", t),
    }
}

/// Ecrit `donnees` dans `chemin` : cree le dossier, ecrit en
/// LF, le JSON suivi d'un saut de ligne.
pub fn ecrire_json<T: Serialize + ?Sized>(chemin: &Path, donnees: &T, indent: usize) -> R<()> {
    if let Some(dossier) = chemin.parent() {
        std::fs::create_dir_all(dossier).map_err(|e| crate::echec::du_disque(&dossier, &e))?;
    }
    let donnees = en_valeur(donnees);
    let mut texte = String::new();
    ecrire(&donnees, indent, 0, &mut texte);
    texte.push('\n');
    // ATOMIQUE : le fichier s'ecrit a cote puis prend sa place d'un coup. `std::fs::write` tronque d'abord,
    // puis ecrit : un lecteur arrive entre les deux (l'etat du mois se refait dans le fil de la fenetre, `etat_garde::
    // FileDesEtats`, pendant qu'une route relit `journal/dsn-ecrites.json`) lirait un fichier vide, « EOF while
    // parsing a value ». Un journal JSON ne se voit jamais a moitie ecrit.
    // Le fichier d'a cote est a cette ecriture seule (le processus, un numero) : la fenetre sert ses routes dans son
    // processus, et deux fils qui ecrivent le meme fichier ne s'ecrivent jamais l'un sur l'autre. En `.tmp` :
    // ni l'empreinte de l'etat d'un mois ni la sauvegarde ne le voient le temps qu'il existe.
    let provisoire = provisoire(chemin);
    std::fs::write(&provisoire, texte).map_err(|e| crate::echec::du_disque(&provisoire, &e))?;
    std::fs::rename(&provisoire, chemin).map_err(|e| crate::echec::du_disque(&chemin, &e))?;
    crate::forme::change_pendant_le_geste(chemin);
    Ok(())
}

/// Le fichier provisoire d'une ecriture de `chemin` : a cote, `<nom>.<processus>-<numero>.tmp`, que nulle autre ecriture
/// ne prend (ni un autre fil, ni un autre processus).
pub fn provisoire(chemin: &Path) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let nom: String = chemin.file_name().iter().map(|n| n.to_string_lossy()).collect();
    chemin.with_file_name(format!("{}.{}-{}.tmp", nom, std::process::id(), n))
}


/// Efface `chemin` s'il existe : le geste qui l'avait lu le relit (`forme::change_pendant_le_geste`).
pub fn effacer(chemin: &Path) -> R<()> {
    match std::fs::remove_file(chemin) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(crate::echec::du_disque(&chemin, &e)),
        _ => {}
    }
    crate::forme::change_pendant_le_geste(chemin);
    Ok(())
}
