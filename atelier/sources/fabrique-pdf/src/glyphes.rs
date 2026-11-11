//! Les noms de glyphes d'Adobe : le caractère qu'un nom de glyphe dit (ISO 32000-1, 9.10.2 : « Look up the character
//! name in the Adobe Glyph List »). Les listes sont celles déposées dans `atelier/reference/sources/adobe-polices/`,
//! lues telles quelles ; un nom se lit selon « 2. The mapping » de la spécification déposée à côté
//! (`adobe-agl-specification-20261111.md`).

use std::sync::OnceLock;

use crate::description::{defaut, Refus};

/// L'Adobe Glyph List 2.0.
const AGL: &str = include_str!("../../../reference/sources/adobe-polices/adobe-glyphlist-20261111.txt");
/// L'ITC Zapf Dingbats Glyph List.
const ZAPF: &str = include_str!("../../../reference/sources/adobe-polices/adobe-zapfdingbats-20261111.txt");

/// Une liste lue : (nom, caractères), triée par nom.
type Liste = Vec<(&'static str, String)>;

static LISTE_AGL: OnceLock<Result<Liste, Refus>> = OnceLock::new();
static LISTE_ZAPF: OnceLock<Result<Liste, Refus>> = OnceLock::new();

/// Une liste de la forme « nom;XXXX[ XXXX…] » (les lignes `#` sont des commentaires). Une ligne d'une autre forme est
/// un défaut du logiciel : la liste est livrée avec lui.
fn lire(texte: &'static str, quoi: &str) -> Result<Liste, Refus> {
    let mut v = Vec::new();
    for ligne in texte.lines() {
        if ligne.starts_with('#') {
            continue;
        }
        let Some((nom, valeurs)) = ligne.split_once(';') else {
            return defaut(format!("la liste {} a la ligne {:?}, sans « ; »", quoi, ligne));
        };
        let mut caracteres = String::new();
        for h in valeurs.split(' ') {
            let n = match u32::from_str_radix(h, 16) {
                Ok(n) => n,
                Err(e) => return defaut(format!("la liste {} donne à {} la valeur {:?} ({})", quoi, nom, h, e)),
            };
            let Some(c) = char::from_u32(n) else {
                return defaut(format!("la liste {} donne à {} la valeur {:X}, hors d'Unicode", quoi, nom, n));
            };
            caracteres.push(c);
        }
        v.push((nom, caracteres));
    }
    v.sort_by(|a, b| a.0.cmp(b.0));
    Ok(v)
}

fn liste(cellule: &'static OnceLock<Result<Liste, Refus>>, texte: &'static str, quoi: &str) -> Result<&'static Liste, Refus> {
    match cellule.get_or_init(|| lire(texte, quoi)) {
        Ok(l) => Ok(l),
        Err(r) => Err(r.clone()),
    }
}

fn chercher<'a>(l: &'a Liste, nom: &str) -> Option<&'a str> {
    let i = l.partition_point(|(g, _)| *g < nom);
    match l.get(i) {
        Some((g, c)) if *g == nom => Some(c.as_str()),
        Some(_) | None => None,
    }
}

/// La valeur de chiffres hexadécimaux **capitales** (0–9, A–F) ; `None` pour un autre caractère.
fn capitales_hexadecimales(h: &str) -> Option<u32> {
    h.chars().try_fold(0u32, |v, c| match c {
        '0'..='9' | 'A'..='F' => c.to_digit(16).map(|d| v * 16 + d),
        _ => None,
    })
}

/// Un composant (spécification, 2, étape 3) : la liste Zapf Dingbats pour cette police, la liste, `uniXXXX…` (des
/// groupes de quatre chiffres capitales, hors D800–DFFF), `uXXXX` à `uXXXXXX` (hors D800–DFFF, jusqu'à 10FFFF) ; sinon
/// rien.
fn composant(c: &str, zapf: bool, sortie: &mut String) -> Result<(), Refus> {
    if zapf {
        if let Some(t) = chercher(liste(&LISTE_ZAPF, ZAPF, "Zapf Dingbats")?, c) {
            sortie.push_str(t);
            return Ok(());
        }
    }
    if let Some(t) = chercher(liste(&LISTE_AGL, AGL, "des glyphes d'Adobe")?, c) {
        sortie.push_str(t);
        return Ok(());
    }
    if let Some(h) = c.strip_prefix("uni") {
        if !h.is_empty() && h.len() % 4 == 0 && h.is_ascii() {
            let valeurs: Option<Vec<char>> = (0..h.len() / 4)
                .map(|k| capitales_hexadecimales(&h[4 * k..4 * k + 4]).filter(|v| !(0xD800..=0xDFFF).contains(v)).and_then(char::from_u32))
                .collect();
            if let Some(v) = valeurs {
                sortie.extend(v);
                return Ok(());
            }
        }
    }
    if let Some(h) = c.strip_prefix('u') {
        if (4..=6).contains(&h.len()) {
            if let Some(v) = capitales_hexadecimales(h).filter(|v| !(0xD800..=0xDFFF).contains(v)).and_then(char::from_u32) {
                sortie.push(v);
            }
        }
    }
    Ok(())
}

/// Les caractères qu'un nom de glyphe dit (spécification, 2) : le nom coupé au premier point, ses composants séparés
/// par `_`, chacun lu, bout à bout ; `None` quand ils ne disent rien. `zapf` : la police est ZapfDingbats.
pub(crate) fn caracteres(nom: &str, zapf: bool) -> Result<Option<String>, Refus> {
    let tete = match nom.split_once('.') {
        Some((avant, _)) => avant,
        None => nom,
    };
    let mut sortie = String::new();
    for c in tete.split('_') {
        composant(c, zapf, &mut sortie)?;
    }
    Ok(Some(sortie).filter(|s| !s.is_empty()))
}
