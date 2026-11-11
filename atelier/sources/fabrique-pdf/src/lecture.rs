//! La lecture des PDF existants : le nombre de pages (`--pages`) et le texte (`--texte`), et le
//! chargement commun à tous les modes qui relisent un PDF (`charger`).

use std::collections::BTreeMap;

use lopdf::{Dictionary, Document, Object, ObjectId};

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
pub fn texte(chemin: &str) -> Result<(Vec<crate::texte::TexteDePage>, Vec<String>), Refus> {
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

/// Un élément de tableau (ou un opérande), désigné et lu par `lire` : exigé, un refus qui dit `quoi` quand il est nul
/// ou d'un autre type.
pub fn element<'a, T>(doc: &'a Document, o: &'a Object, quoi: &str, lire: impl Fn(&'a Object) -> Result<T, &'static str>) -> Result<T, Refus> {
    match designe(doc, o)? {
        Some(x) => lire(x).map_err(|attendu| Refus::illisible(format!("{} n'est pas {}", quoi, attendu))),
        None => illisible(format!("{} est nul", quoi)),
    }
}

/// Un tableau de nombres (`quoi` le nomme dans un refus).
pub fn nombres_de(doc: &Document, o: &Object, quoi: &str) -> Result<Vec<Mesure>, Refus> {
    let v = tableau(o).map_err(|attendu| Refus::illisible(format!("{} n'est pas {}", quoi, attendu)))?;
    v.iter().map(|x| element(doc, x, &format!("un élément de {}", quoi), nombre)).collect()
}

/// Le tableau de nombres de `cle` : `None` absent, un refus quand un élément n'est pas un nombre.
pub fn nombres(doc: &Document, d: &Dictionary, cle: &[u8]) -> Result<Option<Vec<Mesure>>, Refus> {
    match valeur(doc, d, cle)? {
        Some(o) => Ok(Some(nombres_de(doc, o, &nom_de(cle))?)),
        None => Ok(None),
    }
}

/// Un rectangle (ISO 32000-1, 7.9.5 : quatre nombres, deux coins opposés), ses coins remis dans l'ordre (x0, y0, x1,
/// y1) ; un refus quand ce ne sont pas quatre nombres.
pub fn rectangle_de(doc: &Document, o: &Object, quoi: &str) -> Result<[Mesure; 4], Refus> {
    match nombres_de(doc, o, quoi)?.as_slice() {
        &[a, b, c, e] => Ok([a.min(c), b.min(e), a.max(c), b.max(e)]),
        v => illisible(format!("{} a {} nombres, un rectangle en a quatre", quoi, v.len())),
    }
}

/// Le rectangle de `cle` : `None` absent.
pub fn rectangle(doc: &Document, d: &Dictionary, cle: &[u8]) -> Result<Option<[Mesure; 4]>, Refus> {
    match valeur(doc, d, cle)? {
        Some(o) => Ok(Some(rectangle_de(doc, o, &nom_de(cle))?)),
        None => Ok(None),
    }
}

/// La matrice de `cle` (six nombres, ISO 32000-1, 8.3.4) : `None` absente, un refus quand ce ne sont pas six nombres.
pub fn matrice(doc: &Document, d: &Dictionary, cle: &[u8]) -> Result<Option<[Mesure; 6]>, Refus> {
    match nombres(doc, d, cle)?.as_deref() {
        Some(&[a, b, c, e, f, g]) => Ok(Some([a, b, c, e, f, g])),
        Some(v) => illisible(format!("{} a {} nombres, une matrice en a six", nom_de(cle), v.len())),
        None => Ok(None),
    }
}

/// Les filtres de compression que lopdf défait.
const FILTRES_DE_LOPDF: [&[u8]; 2] = [b"FlateDecode", b"LZWDecode"];
/// Les filtres d'écriture que la fabrique défait elle-même (ISO 32000-1, 7.4.2, 7.4.3, 7.4.5).
const FILTRES_ECRITS: [&[u8]; 3] = [b"ASCIIHexDecode", b"ASCII85Decode", b"RunLengthDecode"];

/// Les données d'un flux, ses filtres défaits. ISO 32000-1, table 5 : `Filter` est facultatif (un nom, ou un tableau de
/// zéro, un ou plusieurs noms, appliqués dans l'ordre) ; sans filtre, les données sont telles quelles. Flate et LZW
/// (lopdf) ; ASCIIHex, ASCII85 et RunLength (7.4.2, 7.4.3, 7.4.5). Un autre filtre se refuse en le nommant.
pub fn contenu(doc: &Document, s: &lopdf::Stream) -> Result<Vec<u8>, Refus> {
    let mut filtres: Vec<&[u8]> = Vec::new();
    match valeur(doc, &s.dict, b"Filter")? {
        Some(Object::Name(n)) => filtres.push(n.as_slice()),
        Some(Object::Array(v)) => {
            for x in v {
                filtres.push(element(doc, x, "un élément de /Filter", nom)?);
            }
        }
        Some(_) => return illisible("/Filter n'est ni un nom ni un tableau de noms"),
        None => {}
    }
    if let Some(f) = filtres.iter().find(|f| !FILTRES_DE_LOPDF.contains(f) && !FILTRES_ECRITS.contains(f)) {
        return illisible(format!("un flux est encodé par {}, que la fabrique ne défait pas", nom_de(f)));
    }
    if filtres.is_empty() {
        return Ok(s.content.clone());
    }
    if filtres.iter().all(|f| FILTRES_DE_LOPDF.contains(f)) {
        return s.decompressed_content().map_err(|e| Refus::illisible(format!("un flux ne se décompresse pas ({})", e)));
    }
    // Table 5 : `DecodeParms`, un dictionnaire pour un seul filtre, un tableau (un élément par filtre, nul pour un
    // filtre sans paramètre) pour plusieurs.
    let parametres: Vec<Option<&Dictionary>> = match (valeur(doc, &s.dict, b"DecodeParms")?, filtres.len()) {
        (None, n) => vec![None; n],
        (Some(o), 1) => vec![Some(dictionnaire(o).map_err(|attendu| Refus::illisible(format!("/DecodeParms d'un seul filtre n'est pas {}", attendu)))?)],
        (Some(Object::Array(v)), n) if v.len() == n => v
            .iter()
            .map(|x| match designe(doc, x)? {
                Some(p) => dictionnaire(p).map(Some).map_err(|attendu| Refus::illisible(format!("un élément de /DecodeParms n'est pas {}", attendu))),
                None => Ok(None),
            })
            .collect::<Result<_, _>>()?,
        (Some(_), n) => return illisible(format!("/DecodeParms n'est pas un tableau d'un élément par filtre ({} filtres)", n)),
    };
    let mut donnees = s.content.clone();
    for (f, p) in filtres.iter().zip(parametres) {
        donnees = match *f {
            b"ASCIIHexDecode" => ascii_hex(&donnees)?,
            b"ASCII85Decode" => ascii_85(&donnees)?,
            b"RunLengthDecode" => longueurs(&donnees)?,
            compression => {
                let mut d = Dictionary::new();
                d.set("Filter", Object::Name(compression.to_vec()));
                if let Some(p) = p {
                    d.set("DecodeParms", Object::Dictionary(p.clone()));
                }
                lopdf::Stream::new(d, donnees)
                    .decompressed_content()
                    .map_err(|e| Refus::illisible(format!("un flux ne se décompresse pas par {} ({})", nom_de(compression), e)))?
            }
        };
    }
    Ok(donnees)
}

/// Un blanc (ISO 32000-1, 7.2.2, table 1).
fn blanc(c: u8) -> bool {
    matches!(c, 0 | b'\t' | b'\n' | 0x0C | b'\r' | b' ')
}

/// ASCIIHexDecode (7.4.2) : deux chiffres par octet, les blancs ignorés, `>` pour fin ; un chiffre seul à la fin est
/// suivi d'un 0 ; un autre caractère est une erreur.
fn ascii_hex(o: &[u8]) -> Result<Vec<u8>, Refus> {
    let mut chiffres = Vec::new();
    for &c in o {
        match c {
            b'>' => break,
            b'0'..=b'9' => chiffres.push(c - b'0'),
            b'a'..=b'f' => chiffres.push(c - b'a' + 10),
            b'A'..=b'F' => chiffres.push(c - b'A' + 10),
            c if blanc(c) => {}
            c => return illisible(format!("un flux ASCIIHexDecode a le caractère {:?} (ISO 32000-1, 7.4.2)", c as char)),
        }
    }
    if chiffres.len() % 2 == 1 {
        chiffres.push(0);
    }
    Ok(chiffres.chunks(2).map(|p| p[0] * 16 + p[1]).collect())
}

/// ASCII85Decode (7.4.3) : cinq caractères de `!` à `u` pour quatre octets, `z` pour quatre zéros, `~>` pour fin, les
/// blancs ignorés ; un dernier groupe de n + 1 caractères donne n octets. Un autre caractère, ou un groupe impossible,
/// est une erreur.
fn ascii_85(o: &[u8]) -> Result<Vec<u8>, Refus> {
    let mut sortie = Vec::new();
    let mut groupe: Vec<u32> = Vec::new();
    let mut i = 0;
    while i < o.len() {
        let c = o[i];
        i += 1;
        match c {
            b'~' => match o.get(i) {
                Some(b'>') => break,
                _ => return illisible("un flux ASCII85Decode a un « ~ » qui n'ouvre pas sa fin « ~> » (ISO 32000-1, 7.4.3)"),
            },
            b'z' if groupe.is_empty() => sortie.extend_from_slice(&[0, 0, 0, 0]),
            b'!'..=b'u' => {
                groupe.push((c - b'!') as u32);
                if groupe.len() == 5 {
                    let n = groupe.iter().try_fold(0u32, |v, d| v.checked_mul(85).and_then(|v| v.checked_add(*d)));
                    let Some(n) = n else {
                        return illisible("un flux ASCII85Decode a un groupe qui dépasse quatre octets (ISO 32000-1, 7.4.3)");
                    };
                    sortie.extend_from_slice(&n.to_be_bytes());
                    groupe.clear();
                }
            }
            c if blanc(c) => {}
            c => return illisible(format!("un flux ASCII85Decode a le caractère {:?} (ISO 32000-1, 7.4.3)", c as char)),
        }
    }
    match groupe.len() {
        0 => {}
        1 => return illisible("un flux ASCII85Decode finit par un groupe d'un seul caractère (ISO 32000-1, 7.4.3)"),
        n => {
            // Le groupe complété de `u` (84), le plus grand chiffre : ses n - 1 premiers octets sont ceux de l'encodeur.
            let complet = groupe.iter().copied().chain(std::iter::repeat(84)).take(5);
            let valeur = complet.fold(Some(0u32), |v, d| v.and_then(|v| v.checked_mul(85)).and_then(|v| v.checked_add(d)));
            let Some(valeur) = valeur else {
                return illisible("un flux ASCII85Decode a un dernier groupe qui dépasse quatre octets (ISO 32000-1, 7.4.3)");
            };
            sortie.extend_from_slice(&valeur.to_be_bytes()[..n - 1]);
        }
    }
    Ok(sortie)
}

/// RunLengthDecode (7.4.5) : une longueur de 0 à 127, puis longueur + 1 octets tels quels ; de 129 à 255, un octet
/// répété 257 - longueur fois ; 128, la fin.
fn longueurs(o: &[u8]) -> Result<Vec<u8>, Refus> {
    let mut sortie = Vec::new();
    let mut i = 0;
    while i < o.len() {
        let l = o[i] as usize;
        i += 1;
        match l {
            128 => break,
            0..=127 => {
                let Some(morceau) = o.get(i..i + l + 1) else {
                    return illisible("un flux RunLengthDecode s'arrête au milieu d'une suite (ISO 32000-1, 7.4.5)");
                };
                sortie.extend_from_slice(morceau);
                i += l + 1;
            }
            _ => {
                let Some(&octet) = o.get(i) else {
                    return illisible("un flux RunLengthDecode s'arrête avant l'octet à répéter (ISO 32000-1, 7.4.5)");
                };
                sortie.extend(std::iter::repeat(octet).take(257 - l));
                i += 1;
            }
        }
    }
    Ok(sortie)
}

/// Le dictionnaire de l'objet `id`.
pub fn dictionnaire_de(doc: &Document, id: ObjectId) -> Result<&Dictionary, Refus> {
    match doc.objects.get(&id) {
        Some(o) => dictionnaire(o).map_err(|attendu| Refus::illisible(format!("l'objet {} {} n'est pas {}", id.0, id.1, attendu))),
        None => illisible(format!("l'objet {} {} manque", id.0, id.1)),
    }
}

/// Au-delà, un arbre des pages qui ne remonte pas à sa racine (un fichier abîmé) se refuse.
const ANCETRES_MAX: usize = 64;

/// Un attribut héritable d'une page, désigné : le sien, sinon celui de son plus proche ancêtre (ISO 32000-1, 7.7.3.4 :
/// « If such an attribute is omitted from a page object, its value shall be inherited from an ancestor node in the
/// page tree ») ; `None` quand aucun ne le porte. `Parent` est une référence (table 30), absente à la racine.
pub fn herite<'a>(doc: &'a Document, page: ObjectId, cle: &[u8]) -> Result<Option<&'a Object>, Refus> {
    let mut courant = page;
    for _ in 0..ANCETRES_MAX {
        let d = dictionnaire_de(doc, courant)?;
        if let Some(v) = valeur(doc, d, cle)? {
            return Ok(Some(v));
        }
        courant = match directe(d, b"Parent") {
            Some(Object::Reference(id)) => *id,
            Some(_) => return illisible("/Parent d'un nœud des pages n'est pas une référence"),
            None => return Ok(None),
        };
    }
    illisible(format!("l'arbre des pages remonte au-delà de {} ancêtres", ANCETRES_MAX))
}

/// Les ressources d'une page recopiées, et leur sous-dictionnaire `cle` (`Font`, `XObject`) recopié à part, pour y
/// poser une ressource : des ressources qui n'ont pas `cle` n'ont encore aucune ressource de cette sorte.
/// `Resources` est héritable (ISO 32000-1, table 30 : « (Required; inheritable) ») : il se lit chez la page, sinon
/// chez son plus proche ancêtre. Quand aucun ne le porte, la page n'a aucune ressource : le dictionnaire part vide,
/// et celui qui y pose la sienne l'écrit dans la page.
pub fn ressources_et(doc: &Document, page: ObjectId, cle: &[u8]) -> Result<(Dictionary, Dictionary), Refus> {
    let res = match herite(doc, page, b"Resources")? {
        Some(o) => dictionnaire(o).map_err(|attendu| Refus::illisible(format!("/Resources de la page n'est pas {}", attendu)))?.clone(),
        None => Dictionary::new(),
    };
    let mut sorte = Dictionary::new();
    if let Some(p) = lue(doc, &res, cle, dictionnaire)? {
        sorte.extend(p);
    }
    Ok((res, sorte))
}

/// Le rectangle héritable `cle` d'une page.
fn rectangle_herite(doc: &Document, page: ObjectId, cle: &[u8]) -> Result<Option<[Mesure; 4]>, Refus> {
    match herite(doc, page, cle)? {
        Some(o) => Ok(Some(rectangle_de(doc, o, &nom_de(cle))?)),
        None => Ok(None),
    }
}

/// La partie visible d'une page (x0, y0, x1, y1) : sa `MediaBox`, exigée et héritable, coupée par sa `CropBox`, qui
/// vaut la `MediaBox` quand elle manque (ISO 32000-1, table 30). Une page sans surface se refuse.
pub fn cadre_de(doc: &Document, page: ObjectId) -> Result<[Mesure; 4], Refus> {
    let m = match rectangle_herite(doc, page, b"MediaBox")? {
        Some(m) => m,
        None => return illisible("la page n'a pas de /MediaBox, que la norme exige (ISO 32000-1, table 30)"),
    };
    let c = match rectangle_herite(doc, page, b"CropBox")? {
        Some(r) => [m[0].max(r[0]), m[1].max(r[1]), m[2].min(r[2]), m[3].min(r[3])],
        None => m,
    };
    match c[2] > c[0] && c[3] > c[1] {
        true => Ok(c),
        false => illisible("la partie visible de la page (/MediaBox, /CropBox) n'a pas de surface"),
    }
}

/// La rotation d'une page, en degrés, entre 0 et 270 : `Rotate`, héritable, « shall be a multiple of 90. Default
/// value: 0 » (ISO 32000-1, table 30) ; une autre valeur se refuse.
pub fn rotation_de(doc: &Document, page: ObjectId) -> Result<i64, Refus> {
    match herite(doc, page, b"Rotate")? {
        Some(o) => match entier(o) {
            Ok(r) if r % 90 == 0 => Ok(r.rem_euclid(360)),
            Ok(r) => illisible(format!("/Rotate vaut {}, qui n'est pas un multiple de 90", r)),
            Err(attendu) => illisible(format!("/Rotate n'est pas {}", attendu)),
        },
        None => Ok(ROTATION_PAR_DEFAUT),
    }
}

/// ISO 32000-1, table 30 : `Rotate`, « Default value: 0 ».
const ROTATION_PAR_DEFAUT: i64 = 0;

/// Au-delà, des tableaux de contenus imbriqués se refusent.
const IMBRICATION_MAX: usize = 4;

/// Les flux de contenu d'une page, bout à bout (un saut de ligne entre deux : un opérateur ne s'étend jamais sur deux
/// flux). `Contents` est un flux ou un tableau de flux ; absent, la page est vide (ISO 32000-1, table 30).
pub fn contenu_de_page(doc: &Document, page: ObjectId) -> Result<Vec<u8>, Refus> {
    fn ramasser(doc: &Document, o: &Object, sortie: &mut Vec<u8>, prof: usize) -> Result<(), Refus> {
        match designe(doc, o)? {
            Some(Object::Stream(s)) => {
                sortie.extend_from_slice(&contenu(doc, s)?);
                sortie.push(b'\n');
                Ok(())
            }
            Some(Object::Array(v)) if prof < IMBRICATION_MAX => v.iter().try_for_each(|x| ramasser(doc, x, sortie, prof + 1)),
            Some(Object::Array(_)) => illisible(format!("/Contents imbrique plus de {} tableaux", IMBRICATION_MAX)),
            Some(_) => illisible("/Contents n'est ni un flux ni un tableau de flux"),
            None => illisible("un élément de /Contents est nul"),
        }
    }
    let mut sortie = Vec::new();
    if let Some(c) = valeur(doc, dictionnaire_de(doc, page)?, b"Contents")? {
        ramasser(doc, c, &mut sortie, 0)?;
    }
    Ok(sortie)
}
