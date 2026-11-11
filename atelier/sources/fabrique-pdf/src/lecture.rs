//! La lecture des PDF existants : le nombre de pages (`--pages`) et le texte (`--texte`), et le
//! chargement commun à tous les modes qui relisent un PDF (`charger`).

use std::collections::BTreeMap;

use lopdf::{Dictionary, Document, Object};

use crate::description::{illisible, Refus};
use crate::mesure::Mesure;

/// Lit un PDF en mémoire. Un fichier dont la table de références est fausse (« Microsoft Print To
/// PDF » écrit des positions décalées d'un octet, que lopdf ne retrouve pas) est réparé : on
/// reconstruit la table en cherchant les objets (`N G obj`) dans le fichier, comme font les
/// lecteurs de PDF.
pub fn charger(octets: &[u8]) -> Result<Document, Refus> {
    Ok(charger_avec_octets(octets)?.0)
}

/// `charger`, et les octets que la lecture a lus : ceux d'entrée, ou ceux que la réparation a complétés
/// (une mise à jour incrémentale s'écrit à leur suite).
pub fn charger_avec_octets(octets: &[u8]) -> Result<(Document, Vec<u8>), Refus> {
    match Document::load_mem(octets) {
        Ok(doc) if !doc.get_pages().is_empty() => Ok((doc, octets.to_vec())),
        premier => {
            let repare = reparer(octets).map(|r| (Document::load_mem(&r), r));
            match (premier, repare) {
                (_, Some((Ok(doc), r))) if !doc.get_pages().is_empty() => Ok((doc, r)),
                // Lu, sans page, et que la réparation ne rend pas lisible avec des pages : le fichier tel qu'il est.
                (Ok(doc), _) => Ok((doc, octets.to_vec())),
                (Err(e), None | Some((Ok(_), _))) => illisible(format!("n'est pas un PDF lisible ({})", e)),
                (Err(e), Some((Err(e2), _))) => {
                    illisible(format!("n'est pas un PDF lisible ({}), ni avec sa table de références refaite ({})", e, e2))
                }
            }
        }
    }
}

/// Les nombres décimaux d'un début de ligne : `(n, g)` de « n g obj », et la position du mot `obj`.
fn debut_d_objet(b: &[u8], i: usize) -> Option<(u32, u16)> {
    let chiffres = |mut j: usize| {
        let debut = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j == debut || j - debut > 10 {
            return None;
        }
        // Dix chiffres au plus : ils tiennent dans un u64.
        let n = b[debut..j].iter().fold(0u64, |n, c| n * 10 + (c - b'0') as u64);
        Some((n, j))
    };
    let (n, j) = chiffres(i)?;
    if b.get(j) != Some(&b' ') {
        return None;
    }
    let (g, k) = chiffres(j + 1)?;
    if b.get(k) != Some(&b' ') || b.get(k + 1..k + 4) != Some(b"obj") {
        return None;
    }
    // « obj » est suivi d'un blanc ou d'un délimiteur (et non de « ect » d'un autre mot).
    // Un numéro ou une génération plus grands que ceux d'un PDF (u32, u16) ne sont pas un début d'objet.
    match b.get(k + 4) {
        Some(c) if c.is_ascii_alphanumeric() => None,
        Some(_) | None if n > u32::MAX as u64 || g > u16::MAX as u64 => None,
        Some(_) | None => Some((n as u32, g as u16)),
    }
}

/// Le fichier avec une table de références neuve, ajoutée à la fin, bâtie sur la position réelle
/// des objets ; `None` quand il n'y a pas de queue classique (`trailer`) à reprendre.
fn reparer(b: &[u8]) -> Option<Vec<u8>> {
    let mut objets: BTreeMap<u32, (usize, u16)> = BTreeMap::new();
    let mut i = 0;
    while i < b.len() {
        if i == 0 || b[i - 1] == b'\n' || b[i - 1] == b'\r' {
            if let Some((n, g)) = debut_d_objet(b, i) {
                objets.insert(n, (i, g));
            }
        }
        i += 1;
    }
    if objets.is_empty() {
        return None;
    }
    // La dernière queue du fichier : son dictionnaire, jusqu'à `startxref`.
    let t = b.windows(7).rposition(|w| w == b"trailer")?;
    // Elle va jusqu'à `startxref`, ou jusqu'à la fin d'un fichier qui ne l'écrit pas.
    let fin = match b[t..].windows(9).position(|w| w == b"startxref") {
        Some(p) => t + p,
        None => b.len(),
    };
    let queue = &b[t..fin];
    let racine = queue.windows(5).any(|w| w == b"/Root");
    if !racine {
        return None;
    }
    let plus_grand = *objets.keys().last()?;
    let mut sortie = b.to_vec();
    sortie.push(b'\n');
    let debut_table = sortie.len();
    sortie.extend_from_slice(format!("xref\n0 {}\n", plus_grand + 1).as_bytes());
    sortie.extend_from_slice(b"0000000000 65535 f \n");
    for n in 1..=plus_grand {
        match objets.get(&n) {
            Some((pos, g)) => sortie.extend_from_slice(format!("{:010} {:05} n \n", pos, g).as_bytes()),
            None => sortie.extend_from_slice(b"0000000000 65535 f \n"),
        }
    }
    sortie.extend_from_slice(queue);
    sortie.extend_from_slice(format!("\nstartxref\n{}\n%%EOF\n", debut_table).as_bytes());
    Some(sortie)
}

/// Ouvre un PDF, et dit s'il était verrouillé. Un fichier chiffré s'ouvre avec le mot de passe vide
/// quand c'est possible (`dechiffrer` : `true`) ; sans le déchiffrer, le nombre de pages se lit
/// quand même.
pub fn ouvrir(chemin: &str, dechiffrer: bool) -> Result<(Document, bool), Refus> {
    let octets = std::fs::read(chemin).map_err(|e| Refus::illisible(format!("{} illisible ({})", chemin, e)))?;
    let mut doc = charger(&octets).map_err(|r| r.reformuler(|p| format!("{} {}", chemin, p)))?;
    let verrouille = doc.is_encrypted();
    if dechiffrer {
        crate::chiffrement::dechiffrer(&mut doc, b"").map_err(|r| r.reformuler(|p| format!("{} : {}", chemin, p)))?;
    }
    Ok((doc, verrouille))
}

/// Le nombre de pages de chaque fichier, dans l'ordre.
pub fn pages(chemins: &[String]) -> Result<Vec<usize>, Refus> {
    chemins.iter().map(|c| Ok(ouvrir(c, false)?.0.get_pages().len())).collect()
}

/// Le texte de chaque page de `chemin`, et les avertissements (pages illisibles).
pub fn texte(chemin: &str) -> Result<(Vec<String>, Vec<String>), Refus> {
    let (doc, _) = ouvrir(chemin, true)?;
    Ok(crate::texte::texte_des_pages(&doc))
}

// ---------------------------------------------------------------------------------------------
// Lire les objets d'un PDF, strictement (ISO 32000-1, 7.3 ; atelier/reference/sources/iso-32000/). Une clé absente est
// un état du fichier (`None`) ; une clé présente d'un autre type que celui que la norme lui donne est une pièce que la
// fabrique ne lit pas : elle se refuse en la nommant.
// ---------------------------------------------------------------------------------------------

/// Au-delà, une chaîne de références qui ne finit pas (un fichier abîmé) se refuse.
const REFERENCES_MAX: usize = 32;

/// L'objet que `o` désigne : lui-même, ou celui de sa référence. `None` pour l'objet nul, et pour une référence à un
/// objet que le fichier n'a pas : ISO 32000-1, 7.3.9 et 7.3.10, « An indirect reference to an undefined object [...]
/// shall be treated as a reference to the null object », et l'objet nul comme valeur d'une clé « shall be equivalent
/// to omitting the entry entirely ».
pub fn designe<'a>(doc: &'a Document, o: &'a Object) -> Result<Option<&'a Object>, Refus> {
    let mut o = o;
    for _ in 0..REFERENCES_MAX {
        match o {
            Object::Null => return Ok(None),
            Object::Reference(id) => match doc.objects.get(id) {
                Some(x) => o = x,
                None => return Ok(None),
            },
            autre => return Ok(Some(autre)),
        }
    }
    illisible(format!("des références en boucle (au-delà de {})", REFERENCES_MAX))
}

/// La valeur de `cle` dans `d`, désignée (`designe`) ; `None` quand `d` ne la porte pas, ou porte l'objet nul.
pub fn valeur<'a>(doc: &'a Document, d: &'a Dictionary, cle: &[u8]) -> Result<Option<&'a Object>, Refus> {
    match d.as_hashmap().get(cle) {
        Some(o) => designe(doc, o).map_err(|r| r.reformuler(|p| format!("{} : {}", nom_de(cle), p))),
        None => Ok(None),
    }
}

/// La valeur de `cle`, directe (sans suivre une référence) : là où la norme la veut directe, ou avant que le fichier
/// soit déchiffré.
pub fn directe<'a>(d: &'a Dictionary, cle: &[u8]) -> Option<&'a Object> {
    match d.as_hashmap().get(cle) {
        Some(Object::Null) | None => None,
        Some(o) => Some(o),
    }
}

/// La valeur de `cle`, lue par `lire` (`entier`, `nom`, `dictionnaire`...) : `None` absente, un refus qui la nomme
/// quand elle n'est pas du type que `lire` attend.
pub fn lue<'a, T>(doc: &'a Document, d: &'a Dictionary, cle: &[u8], lire: impl Fn(&'a Object) -> Result<T, &'static str>) -> Result<Option<T>, Refus> {
    match valeur(doc, d, cle)? {
        Some(o) => lire(o).map(Some).map_err(|attendu| Refus::illisible(format!("{} n'est pas {}", nom_de(cle), attendu))),
        None => Ok(None),
    }
}

/// La clé telle qu'un PDF l'écrit (`/Widths`), pour une phrase.
pub fn nom_de(cle: &[u8]) -> String {
    format!("/{}", String::from_utf8_lossy(cle))
}

/// Un entier.
pub fn entier(o: &Object) -> Result<i64, &'static str> {
    match o {
        Object::Integer(i) => Ok(*i),
        _ => Err("un entier"),
    }
}

/// Un nombre, entier ou réel (ISO 32000-1, 7.3.3), en mesure.
pub fn nombre(o: &Object) -> Result<Mesure, &'static str> {
    match o {
        Object::Integer(_) | Object::Real(_) => Mesure::de_l_objet(o).ok_or("un nombre dans les mesures de la fabrique"),
        _ => Err("un nombre"),
    }
}

/// Un booléen.
pub fn booleen(o: &Object) -> Result<bool, &'static str> {
    match o {
        Object::Boolean(b) => Ok(*b),
        _ => Err("un booléen"),
    }
}

/// Un nom (`/AESV3`), sans sa barre.
pub fn nom(o: &Object) -> Result<&[u8], &'static str> {
    match o {
        Object::Name(n) => Ok(n),
        _ => Err("un nom"),
    }
}

/// Une chaîne, ses octets.
pub fn chaine(o: &Object) -> Result<&[u8], &'static str> {
    match o {
        Object::String(s, _) => Ok(s),
        _ => Err("une chaîne"),
    }
}

/// Un tableau.
pub fn tableau(o: &Object) -> Result<&[Object], &'static str> {
    match o {
        Object::Array(a) => Ok(a),
        _ => Err("un tableau"),
    }
}

/// Un dictionnaire (celui d'un flux compris : ISO 32000-1, 7.3.8.1).
pub fn dictionnaire(o: &Object) -> Result<&Dictionary, &'static str> {
    match o {
        Object::Dictionary(d) => Ok(d),
        Object::Stream(s) => Ok(&s.dict),
        _ => Err("un dictionnaire"),
    }
}

/// Un flux.
pub fn flux(o: &Object) -> Result<&lopdf::Stream, &'static str> {
    match o {
        Object::Stream(s) => Ok(s),
        _ => Err("un flux"),
    }
}
