//! L'extraction du texte d'un PDF (`fabrique-pdf --texte`) : le texte de chaque page, dans
//! l'ordre où le PDF l'écrit, au plus près de `page.get_text()` de PyMuPDF (MuPDF).
//!
//! Trois étages :
//!
//! 1. un interpréteur du flux de contenu (états graphiques, matrices, opérateurs de texte,
//!    objets formulaire, apparences des annotations — MuPDF dessine les champs de formulaire, et
//!    leur texte sort avec celui de la page) qui rend des caractères positionnés ;
//! 2. les polices : table `ToUnicode` quand la police en a une (elle l'emporte, comme dans
//!    MuPDF), sinon l'encodage de la police simple (WinAnsi, MacRoman, Standard, `/Differences`) ;
//!    les largeurs viennent du PDF, ou de Liberation pour les polices standard sans `/Widths` ;
//! 3. la mise en lignes : un caractère prolonge la ligne courante s'il part du bout du précédent
//!    (à un espace près) sur la même ligne de base ; sinon il ouvre une ligne. C'est la règle de
//!    MuPDF, pas un tri géométrique : le texte sort dans l'ordre du flux.
//!
//! Le résultat est le texte de chaque page, les lignes séparées par `\n`, un `\n` final.

use std::collections::HashMap;
use std::rc::Rc;

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::description::{defaut, illisible, Refus};
use crate::glyphes::GLYPHES;
use crate::lecture;
use crate::mesure::{mesure, Mesure};

type Mat = [Mesure; 6];
const ID: Mat = [Mesure::UN, Mesure::ZERO, Mesure::ZERO, Mesure::UN, Mesure::ZERO, Mesure::ZERO];
const Z: Mesure = Mesure::ZERO;
/// Le CID d'un code que le CMap ne nomme pas (ISO 32000-1, 9.7.6.3).
const CID_SANS_CORRESPONDANCE: u32 = 0;
/// La largeur d'un glyphe d'une police CID que `W` ne dit pas, sans `DW` (ISO 32000-1, table 117).
const LARGEUR_CID_PAR_DEFAUT: Mesure = Mesure::entier(1000);
/// La largeur d'un code d'une police simple que `Widths` ne dit pas, sans `MissingWidth` (ISO 32000-1, table 122).
const LARGEUR_MANQUANTE_PAR_DEFAUT: Mesure = Mesure::ZERO;

/// Un nombre lu, en entier positif (un code, un numéro de caractère) : sa partie entière, bornée.
fn entier_u32(m: Mesure) -> u32 {
    m.tronque().clamp(0, u32::MAX as i64) as u32
}

/// `a` puis `b` (convention PDF des vecteurs lignes).
fn mul(a: &Mat, b: &Mat) -> Mat {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

/// La translation de (`x`, `y`).
fn translation(x: Mesure, y: Mesure) -> Mat {
    [Mesure::UN, Z, Z, Mesure::UN, x, y]
}

fn point(m: &Mat, x: Mesure, y: Mesure) -> (Mesure, Mesure) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

// ---------------------------------------------------------------------------------------------
// Lecture des objets (`lecture.rs`, strictement).
// ---------------------------------------------------------------------------------------------

/// La matrice d'un objet formulaire : `Matrix`, « Default value: the identity matrix » (ISO 32000-1, table 95).
fn matrice_de_formulaire(doc: &Document, d: &Dictionary) -> Result<Mat, Refus> {
    match lecture::matrice(doc, d, b"Matrix")? {
        Some(m) => Ok(m),
        None => Ok(ID),
    }
}

/// Le dictionnaire de `cle`, exigé : un refus qui dit `quoi` quand il manque.
fn exige<'a, T>(doc: &'a Document, d: &'a Dictionary, cle: &[u8], quoi: &str, lire: impl Fn(&'a Object) -> Result<T, &'static str>) -> Result<T, Refus> {
    match lecture::lue(doc, d, cle, lire)? {
        Some(v) => Ok(v),
        None => illisible(format!("{} n'a pas de {}, que la norme exige", quoi, lecture::nom_de(cle))),
    }
}

// ---------------------------------------------------------------------------------------------
// Les tables CMap : ToUnicode, et codes des polices composées.
// ---------------------------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum Jeton {
    Hex(Vec<u8>),
    Mot(String),
    Ouvre,
    Ferme,
}

fn jetons(octets: &[u8]) -> Result<Vec<Jeton>, Refus> {
    let mut v = Vec::new();
    let mut i = 0;
    while i < octets.len() {
        let c = octets[i];
        match c {
            b'%' => {
                while i < octets.len() && octets[i] != b'\n' && octets[i] != b'\r' {
                    i += 1;
                }
            }
            b'<' if octets.get(i + 1) == Some(&b'<') => i += 2,
            b'>' if octets.get(i + 1) == Some(&b'>') => i += 2,
            b'>' => i += 1,
            b'<' => {
                // ISO 32000-1, 7.3.4.3 : les blancs s'ignorent ; « If the final digit of a hexadecimal string is
                // missing [...] the final digit shall be assumed to be 0 ».
                let mut chiffres: Vec<u8> = Vec::new();
                i += 1;
                while i < octets.len() && octets[i] != b'>' {
                    match octets[i] {
                        c @ b'0'..=b'9' => chiffres.push(c - b'0'),
                        c @ b'a'..=b'f' => chiffres.push(c - b'a' + 10),
                        c @ b'A'..=b'F' => chiffres.push(c - b'A' + 10),
                        c if c.is_ascii_whitespace() => {}
                        c => return illisible(format!("une table CMap a le caractère {:?} dans une chaîne hexadécimale", c as char)),
                    }
                    i += 1;
                }
                i += 1;
                if chiffres.len() % 2 == 1 {
                    chiffres.push(0);
                }
                v.push(Jeton::Hex(chiffres.chunks(2).map(|p| p[0] * 16 + p[1]).collect()));
            }
            b'[' => {
                v.push(Jeton::Ouvre);
                i += 1;
            }
            b']' => {
                v.push(Jeton::Ferme);
                i += 1;
            }
            b'(' => {
                // Une chaîne littérale : ignorée (noms de CMap, registre).
                let mut prof = 0;
                while i < octets.len() {
                    match octets[i] {
                        b'\\' => i += 1,
                        b'(' => prof += 1,
                        b')' => {
                            prof -= 1;
                            if prof == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                i += 1;
            }
            c if c.is_ascii_whitespace() => i += 1,
            _ => {
                let debut = i;
                while i < octets.len()
                    && !octets[i].is_ascii_whitespace()
                    && !matches!(octets[i], b'<' | b'>' | b'[' | b']' | b'(' | b'%')
                {
                    i += 1;
                }
                v.push(Jeton::Mot(String::from_utf8_lossy(&octets[debut..i]).into_owned()));
            }
        }
    }
    Ok(v)
}

fn entier(octets: &[u8]) -> u32 {
    octets.iter().take(4).fold(0u32, |a, b| (a << 8) | *b as u32)
}

/// UTF-16BE (ou un seul octet : Latin-1) d'une destination de `ToUnicode`, ou d'une chaîne marquée UTF-16BE. Un
/// nombre impair d'octets n'est pas de l'UTF-16 : il se refuse.
fn utf16(octets: &[u8]) -> Result<String, Refus> {
    if octets.len() == 1 {
        return Ok((octets[0] as char).to_string());
    }
    if octets.len() % 2 == 1 {
        return illisible(format!("un texte en UTF-16 a un nombre impair d'octets ({})", octets.len()));
    }
    let unites: Vec<u16> = octets.chunks(2).map(|p| ((p[0] as u16) << 8) | p[1] as u16).collect();
    Ok(String::from_utf16_lossy(&unites))
}

/// Une table CMap lue : espaces de codes, `ToUnicode` (code -> texte) et `cid` (code -> CID).
#[derive(Default)]
struct Cmap {
    espaces: Vec<(usize, u32, u32)>,
    unicode: HashMap<u32, String>,
    cid: HashMap<u32, u32>,
    plages_cid: Vec<(u32, u32, u32)>,
}

/// Un CID écrit en décimal dans une table CMap.
fn cid_ecrit(n: &str) -> Result<u32, Refus> {
    n.parse().map_err(|e| Refus::illisible(format!("une table CMap donne le CID {:?}, qui n'est pas un nombre ({})", n, e)))
}

fn lire_cmap(octets: &[u8]) -> Result<Cmap, Refus> {
    let t = jetons(octets)?;
    let mut c = Cmap::default();
    let mut i = 0;
    while i < t.len() {
        let mot = match &t[i] {
            Jeton::Mot(m) => m.as_str(),
            _ => {
                i += 1;
                continue;
            }
        };
        i += 1;
        match mot {
            "begincodespacerange" => {
                while let (Some(Jeton::Hex(a)), Some(Jeton::Hex(b))) = (t.get(i), t.get(i + 1)) {
                    c.espaces.push((a.len(), entier(a), entier(b)));
                    i += 2;
                }
            }
            "beginbfchar" => {
                while let (Some(Jeton::Hex(a)), Some(Jeton::Hex(b))) = (t.get(i), t.get(i + 1)) {
                    c.unicode.insert(entier(a), utf16(b)?);
                    i += 2;
                }
            }
            "beginbfrange" => {
                while let (Some(Jeton::Hex(a)), Some(Jeton::Hex(b))) = (t.get(i), t.get(i + 1)) {
                    let (debut, fin) = (entier(a), entier(b));
                    i += 2;
                    match t.get(i) {
                        Some(Jeton::Hex(d)) => {
                            i += 1;
                            // La destination est celle du premier code ; chacun des suivants
                            // avance son dernier caractère d'autant.
                            let chars: Vec<char> = utf16(d)?.chars().collect();
                            if let Some((dernier, tete)) = chars.split_last() {
                                for k in 0..=fin.saturating_sub(debut).min(0xFFFF) {
                                    let n = (*dernier as u32).wrapping_add(k);
                                    let Some(avance) = char::from_u32(n) else {
                                        return illisible(format!("une plage ToUnicode mène hors d'Unicode ({:X})", n));
                                    };
                                    c.unicode.insert(debut + k, tete.iter().chain([&avance]).collect());
                                }
                            }
                        }
                        Some(Jeton::Ouvre) => {
                            i += 1;
                            let mut k = 0;
                            while let Some(Jeton::Hex(d)) = t.get(i) {
                                c.unicode.insert(debut + k, utf16(d)?);
                                k += 1;
                                i += 1;
                            }
                            if t.get(i) == Some(&Jeton::Ferme) {
                                i += 1;
                            }
                        }
                        _ => break,
                    }
                }
            }
            "begincidchar" => {
                while let (Some(Jeton::Hex(a)), Some(Jeton::Mot(n))) = (t.get(i), t.get(i + 1)) {
                    c.cid.insert(entier(a), cid_ecrit(n)?);
                    i += 2;
                }
            }
            "begincidrange" => {
                while let (Some(Jeton::Hex(a)), Some(Jeton::Hex(b)), Some(Jeton::Mot(n))) =
                    (t.get(i), t.get(i + 1), t.get(i + 2))
                {
                    c.plages_cid.push((entier(a), entier(b), cid_ecrit(n)?));
                    i += 3;
                }
            }
            _ => {}
        }
    }
    Ok(c)
}

// ---------------------------------------------------------------------------------------------
// Les polices.
// ---------------------------------------------------------------------------------------------

/// Ce que l'interpréteur sait d'une police : décoder une chaîne en codes, et dire pour chaque code
/// son texte et sa largeur (millièmes d'em).
struct Police {
    composee: bool,
    /// Polices composées : les espaces de codes (longueur en octets, plus petit, plus grand), et
    /// la table des CID d'un CMap incorporé (`None` : Identity).
    espaces: Vec<(usize, u32, u32)>,
    cmap_cid: Option<Cmap>,
    unicode: HashMap<u32, String>,
    /// Polices simples : le caractère de chaque code d'après l'encodage.
    encodage: Vec<Option<char>>,
    /// Une TrueType lue par la « cmap » de son programme (ISO 32000-1, 9.6.6.4) : pour chaque code, s'il montre un
    /// glyphe. Son texte ne vient que de `ToUnicode` (9.10.2).
    par_cmap: Option<Vec<bool>>,
    largeurs: HashMap<u32, Mesure>,
    largeur_defaut: Mesure,
    /// Largeurs tirées de Liberation pour une police standard sans `/Widths`.
    standard: Option<crate::polices::Metrique>,
    vertical: bool,
    /// Ascendant et descendant (fractions du corps), qui bornent la boîte d'un glyphe pour
    /// `--inspecter` : ceux du fichier de police incorporé, sinon (polices standard) ceux de MuPDF,
    /// sinon ceux du `/FontDescriptor`, sinon 0,8 et 0,2. Laissés à 0,8 et 0,2 pour `--texte`.
    asc: Mesure,
    desc: Mesure,
}

/// Une boîte (x0, y0, x1, y1), dans le repère de la page du PDF (origine en bas à gauche).
pub(crate) type Boite = [Mesure; 4];

/// La boîte englobante du rectangle (x0, y0)-(x1, y1) transformé par `m`.
fn englobante(m: &Mat, x0: Mesure, y0: Mesure, x1: Mesure, y1: Mesure) -> Boite {
    let p = [point(m, x0, y0), point(m, x1, y0), point(m, x0, y1), point(m, x1, y1)];
    let mut b = [Mesure::MAX, Mesure::MAX, Mesure::MIN, Mesure::MIN];
    for (x, y) in p {
        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
    }
    b
}

/// L'ascendant et le descendant d'une police (fractions du corps). Comme MuPDF : ceux du fichier de
/// police incorporé (`hhea`) ; pour une police standard non incorporée, ceux des polices de
/// remplacement de MuPDF (mesurés) ; sinon le `/FontDescriptor` ; sinon 0,8 et 0,2.
fn ascendant_descendant(doc: &Document, fd: Option<&Dictionary>, base_font: Option<&str>) -> Result<(Mesure, Mesure), Refus> {
    let mut programme = None;
    if let Some(d) = fd {
        for cle in [b"FontFile2".as_slice(), b"FontFile3", b"FontFile"] {
            if let Some(s) = lecture::lue(doc, d, cle, lecture::flux)? {
                programme = Some((cle, s));
                break;
            }
        }
    }
    match programme {
        Some((cle, s)) => {
            // `ttf_parser` lit un programme TrueType (`FontFile2`) ou OpenType (`FontFile3` de sous-type `OpenType`,
            // ISO 32000-1, table 126) ; un programme Type 1 ou CFF nu ne se lit pas ici : le descripteur dit ses mesures.
            let opentype = cle == b"FontFile3" && lecture::lue(doc, &s.dict, b"Subtype", lecture::nom)? == Some(b"OpenType".as_slice());
            if cle == b"FontFile2" || opentype {
                let octets = lecture::contenu(doc, s)?;
                let face = ttf_parser::Face::parse(&octets, 0)
                    .map_err(|e| Refus::illisible(format!("le programme incorporé d'une police ({}) ne se lit pas ({})", lecture::nom_de(cle), e)))?;
                let em = face.units_per_em() as i64;
                let (a, d) = (Mesure::entier(face.ascender() as i64) / em, Mesure::entier(-(face.descender() as i64)) / em);
                if a > Z && a <= Mesure::entier(2) && (Z..=Mesure::UN).contains(&d) {
                    return Ok((a, d));
                }
            }
        }
        None => {
            if let Some(base) = base_font {
                let minuscules = base.to_ascii_lowercase();
                // Le nom sans le préfixe d'un sous-ensemble (« ABCDEF+ », ISO 32000-1, 9.6.4).
                let n = match minuscules.rsplit_once('+') {
                    Some((_, apres)) => apres,
                    None => minuscules.as_str(),
                };
                let gras = n.contains("bold");
                if n.contains("helvetica") || n == "arial" || n == "arialmt" || n.starts_with("arial,") || n.starts_with("arial-") {
                    return Ok(if gras { (mesure("1.07"), mesure("0.307")) } else { (mesure("1.075"), mesure("0.299")) });
                }
                if n.contains("times") {
                    return Ok(if gras { (mesure("1.044"), mesure("0.341")) } else { (mesure("1.053"), mesure("0.281")) });
                }
                if n.contains("courier") {
                    return Ok((mesure("0.932"), mesure("0.317")));
                }
            }
        }
    }
    let (ascent, descent) = match fd {
        Some(d) => (lecture::lue(doc, d, b"Ascent", lecture::nombre)?, lecture::lue(doc, d, b"Descent", lecture::nombre)?),
        None => (None, None),
    };
    Ok(match (ascent, descent) {
        (Some(a), Some(d)) if a > Z && a <= Mesure::entier(1500) && d.abs() <= Mesure::entier(1000) => (a / 1000, d.abs() / 1000),
        _ => (mesure("0.8"), mesure("0.2")),
    })
}

impl Police {
    /// Les codes d'une chaîne : (code, nombre d'octets).
    fn codes(&self, s: &[u8]) -> Result<Vec<(u32, usize)>, Refus> {
        if !self.composee {
            return Ok(s.iter().map(|b| (*b as u32, 1)).collect());
        }
        let Some(plus_court) = self.espaces.iter().map(|e| e.0).min() else {
            return illisible("une police composée n'a aucun espace de codes");
        };
        let mut v = Vec::new();
        let mut i = 0;
        while i < s.len() {
            let mut pris = None;
            for n in 1..=4usize.min(s.len() - i) {
                let code = entier(&s[i..i + n]);
                if self.espaces.iter().any(|(l, a, b)| *l == n && code >= *a && code <= *b) {
                    pris = Some((code, n));
                    break;
                }
            }
            // Un code hors de tout espace de codes se lit sur la longueur de l'espace le plus court (ISO 32000-1,
            // 9.7.6.3, a).
            let (code, n) = match pris {
                Some(p) => p,
                None => {
                    let n = plus_court.min(s.len() - i);
                    (entier(&s[i..i + n]), n)
                }
            };
            v.push((code, n));
            i += n;
        }
        Ok(v)
    }

    fn cid(&self, code: u32) -> u32 {
        match &self.cmap_cid {
            None => code,
            Some(c) => {
                if let Some(v) = c.cid.get(&code) {
                    return *v;
                }
                match c.plages_cid.iter().find(|(a, b, _)| code >= *a && code <= *b) {
                    Some((a, _, n)) => n + (code - a),
                    // ISO 32000-1, 9.7.6.3 : « If the CMap does not contain either a character mapping or a notdef
                    // mapping for the code, [...] the glyph for CID 0 shall be substituted ».
                    None => CID_SANS_CORRESPONDANCE,
                }
            }
        }
    }

    /// Le texte d'un code ; `None` quand l'encodage d'une police simple ne lui donne aucun caractère. Le glyphe qu'une
    /// TrueType lue par sa « cmap » montre sans `ToUnicode` se refuse : « there is no way to determine what the
    /// character code represents » (ISO 32000-1, 9.10.2).
    fn texte(&self, code: u32) -> Result<Option<String>, Refus> {
        if let Some(t) = self.unicode.get(&code) {
            return Ok(Some(t.clone()));
        }
        if self.composee {
            return Ok(Some("\u{FFFD}".to_string()));
        }
        if let Some(glyphes) = &self.par_cmap {
            return match glyphes.get(code as usize) {
                Some(true) => illisible(format!(
                    "le code {} d'une police TrueType symbolique ou sans encodage montre un glyphe que rien ne dit (ni /ToUnicode, ni encodage : ISO 32000-1, 9.10.2)",
                    code
                )),
                Some(false) | None => Ok(None),
            };
        }
        Ok(self.encodage.get(code as usize).copied().flatten().map(|c| c.to_string()))
    }

    /// La largeur du code, en fraction d'em.
    fn largeur(&self, code: u32) -> Mesure {
        let cle = if self.composee { self.cid(code) } else { code };
        if let Some(w) = self.largeurs.get(&cle) {
            return *w / 1000;
        }
        if let Some(m) = &self.standard {
            if let Some(c) = self.encodage.get(code as usize).copied().flatten() {
                if let Some(w) = m.largeur(c) {
                    return w;
                }
            }
        }
        self.largeur_defaut / 1000
    }
}

/// Des chiffres hexadécimaux, leur valeur ; `None` quand un caractère n'en est pas un (le nom n'est pas de cette forme).
fn hexadecimal(h: &str) -> Option<u32> {
    h.chars().try_fold(0u32, |v, c| Some(v * 16 + c.to_digit(16)?))
}

/// Le caractère d'un nom de glyphe : la liste de glyphes d'Adobe, ou un nom `uniXXXX` / `uXXXX[XX]` ; `None` pour un
/// nom qu'aucune de ces formes ne dit.
fn nom_de_glyphe(n: &str) -> Option<char> {
    let i = GLYPHES.partition_point(|(g, _)| *g < n);
    if let Some((g, u)) = GLYPHES.get(i) {
        if *g == n {
            return char::from_u32(*u as u32);
        }
    }
    if let Some(h) = n.strip_prefix("uni") {
        if h.len() >= 4 && h.is_char_boundary(4) {
            if let Some(v) = hexadecimal(&h[..4]) {
                return char::from_u32(v);
            }
        }
    }
    if let Some(h) = n.strip_prefix('u') {
        if (4..=6).contains(&h.len()) {
            if let Some(v) = hexadecimal(h) {
                return char::from_u32(v);
            }
        }
    }
    None
}

fn base_encodage(nom: &str) -> Vec<Option<char>> {
    (0..=255u8)
        .map(|b| {
            // WinAnsi met un trait d'union au code 0xAD ; MuPDF, lui, y lit le trait d'union
            // conditionnel (U+00AD) de Windows-1252, et c'est ce qui se retrouve dans son texte.
            if b == 0xAD && nom == "WinAnsiEncoding" {
                return Some('\u{AD}');
            }
            let s = Document::decode_text(Some(nom), &[b]);
            s.chars().next()
        })
        .collect()
}

/// Le `/BaseFont` d'une police ; `None` pour une police qui n'en a pas (Type 3, ISO 32000-1, table 112).
pub(crate) fn nom_de_base(doc: &Document, d: &Dictionary) -> Result<Option<String>, Refus> {
    Ok(lecture::lue(doc, d, b"BaseFont", lecture::nom)?.map(|n| String::from_utf8_lossy(n).into_owned()))
}

/// Le descripteur d'une police (`FontDescriptor`), s'il y en a un.
fn descripteur<'a>(doc: &'a Document, d: &'a Dictionary) -> Result<Option<&'a Dictionary>, Refus> {
    lecture::lue(doc, d, b"FontDescriptor", lecture::dictionnaire)
}

/// Un nombre d'un tableau de largeurs (`W`, `Widths`).
fn largeur_lue(doc: &Document, o: &Object, cle: &str) -> Result<Mesure, Refus> {
    lecture::element(doc, o, &format!("un élément de {}", cle), lecture::nombre)
}

fn charger_police(doc: &Document, d: &Dictionary, inspecter: bool) -> Result<Police, Refus> {
    let sous_type = exige(doc, d, b"Subtype", "une police", lecture::nom)?;
    let composee = sous_type == b"Type0";
    let mut p = Police {
        composee,
        espaces: vec![(2, 0, 0xFFFF)],
        cmap_cid: None,
        unicode: HashMap::new(),
        encodage: Vec::new(),
        par_cmap: None,
        largeurs: HashMap::new(),
        largeur_defaut: Z,
        standard: None,
        vertical: false,
        asc: mesure("0.8"),
        desc: mesure("0.2"),
    };
    if let Some(s) = lecture::lue(doc, d, b"ToUnicode", lecture::flux)? {
        p.unicode = lire_cmap(&lecture::contenu(doc, s)?)?.unicode;
    }
    let base_nom = nom_de_base(doc, d)?;
    if composee {
        // ISO 32000-1, table 121 : `Encoding` (un CMap prédéfini nommé, ou un flux) et `DescendantFonts` (un tableau
        // d'un élément) sont exigés.
        match lecture::valeur(doc, d, b"Encoding")? {
            Some(Object::Name(n)) => p.vertical = n.ends_with(b"-V"),
            Some(Object::Stream(s)) => {
                let c = lire_cmap(&lecture::contenu(doc, s)?)?;
                if !c.espaces.is_empty() {
                    p.espaces = c.espaces.clone();
                }
                p.cmap_cid = Some(c);
            }
            Some(_) => return illisible("/Encoding d'une police composée n'est ni un nom ni un flux"),
            None => return illisible("une police composée n'a pas d'/Encoding, que la norme exige (ISO 32000-1, table 121)"),
        }
        let descendantes = exige(doc, d, b"DescendantFonts", "une police composée", lecture::tableau)?;
        let desc = match descendantes {
            [une] => lecture::element(doc, une, "la police de /DescendantFonts", lecture::dictionnaire)?,
            v => return illisible(format!("/DescendantFonts a {} éléments, la norme en veut un (ISO 32000-1, table 121)", v.len())),
        };
        if inspecter {
            (p.asc, p.desc) = ascendant_descendant(doc, descripteur(doc, desc)?, base_nom.as_deref())?;
        }
        // ISO 32000-1, table 117 : `DW`, « Default value: 1000 ».
        p.largeur_defaut = match lecture::lue(doc, desc, b"DW", lecture::nombre)? {
            Some(dw) => dw,
            None => LARGEUR_CID_PAR_DEFAUT,
        };
        if let Some(w) = lecture::lue(doc, desc, b"W", lecture::tableau)? {
            // ISO 32000-1, 9.7.4.3 : « c [w1 w2 ... wn] » ou « cfirst clast w ».
            let mut i = 0;
            while i < w.len() {
                let premier = entier_u32(largeur_lue(doc, &w[i], "/W")?);
                match w.get(i + 1).map(|o| lecture::designe(doc, o)).transpose()?.flatten() {
                    Some(Object::Array(liste)) => {
                        for (k, x) in liste.iter().enumerate() {
                            p.largeurs.insert(premier + k as u32, largeur_lue(doc, x, "/W")?);
                        }
                        i += 2;
                    }
                    Some(o) => {
                        let dernier = entier_u32(largeur_lue(doc, o, "/W")?);
                        let Some(x) = w.get(i + 2) else {
                            return illisible("/W s'arrête après un « cfirst clast » sans largeur");
                        };
                        let v = largeur_lue(doc, x, "/W")?;
                        for c in premier..=dernier.min(premier + 0xFFFF) {
                            p.largeurs.insert(c, v);
                        }
                        i += 3;
                    }
                    None => return illisible("/W s'arrête après un premier code, sans ses largeurs"),
                }
            }
        }
        return Ok(p);
    }

    // Police simple : l'encodage.
    let fd = descripteur(doc, d)?;
    if inspecter {
        (p.asc, p.desc) = ascendant_descendant(doc, fd, base_nom.as_deref())?;
    }
    let encoding = lecture::valeur(doc, d, b"Encoding")?;
    if sous_type == b"TrueType" {
        p.encodage = encodage_truetype(doc, fd, encoding, &mut p.par_cmap)?;
    } else {
        p.encodage = encodage_simple(doc, encoding)?;
    }

    // Les largeurs : `Widths`, à partir de `FirstChar` (ISO 32000-1, table 111 : tous deux exigés, sauf pour les 14
    // polices standard, où ils sont présents ou absents ensemble).
    match lecture::lue(doc, d, b"Widths", lecture::tableau)? {
        Some(w) => {
            let premier = entier_u32(exige(doc, d, b"FirstChar", "une police qui a des /Widths", lecture::nombre)?);
            for (k, x) in w.iter().enumerate() {
                p.largeurs.insert(premier + k as u32, largeur_lue(doc, x, "/Widths")?);
            }
        }
        // Sans `Widths`, une des 14 polices standard, que son `BaseFont` nomme.
        None => match base_nom.as_deref() {
            Some(n) => p.standard = crate::polices::Metrique::pour(n)?,
            None => return illisible("une police simple n'a ni /Widths ni /BaseFont (ISO 32000-1, table 111)"),
        },
    }
    // ISO 32000-1, table 122 : `MissingWidth`, « Default value: 0 » ; une police sans descripteur ne la dit pas.
    p.largeur_defaut = match fd {
        Some(fd) => match lecture::lue(doc, fd, b"MissingWidth", lecture::nombre)? {
            Some(m) => m,
            None => LARGEUR_MANQUANTE_PAR_DEFAUT,
        },
        None => LARGEUR_MANQUANTE_PAR_DEFAUT,
    };
    Ok(p)
}

/// Les `/Differences` d'un dictionnaire d'encodage : (code, caractère) (ISO 32000-1, 9.6.6.1 : un code, puis les noms
/// des codes qui le suivent ; un nom que la liste de glyphes ne connaît pas ne donne pas de caractère).
fn differences_de(doc: &Document, e: &Dictionary) -> Result<Vec<(usize, char)>, Refus> {
    let mut differences = Vec::new();
    if let Some(v) = lecture::lue(doc, e, b"Differences", lecture::tableau)? {
        let mut code = 0usize;
        for x in v {
            match lecture::designe(doc, x)? {
                Some(Object::Integer(c)) => {
                    code = match usize::try_from(*c) {
                        Ok(c) => c,
                        Err(e) => return illisible(format!("/Differences donne le code {} ({})", c, e)),
                    }
                }
                Some(Object::Name(g)) => {
                    if let Some(c) = nom_de_glyphe(&String::from_utf8_lossy(g)) {
                        differences.push((code, c));
                    }
                    code += 1;
                }
                Some(_) | None => return illisible("un élément de /Differences n'est ni un code ni un nom de glyphe"),
            }
        }
    }
    Ok(differences)
}

/// `table` mise à jour par `differences`.
fn appliquer(mut table: Vec<Option<char>>, differences: Vec<(usize, char)>) -> Vec<Option<char>> {
    for (code, c) in differences {
        if code < 256 {
            table[code] = Some(c);
        }
    }
    table
}

/// Le bit `n` (à partir de 1) des drapeaux d'un descripteur (ISO 32000-1, 9.8.2).
fn drapeau(drapeaux: i64, n: u32) -> bool {
    drapeaux & (1 << (n - 1)) != 0
}

/// L'encodage d'une police TrueType simple, selon ISO 32000-1, 9.6.6.4 « Encodings for TrueType Fonts ».
///
/// Une police au `Encoding` nommé MacRomanEncoding ou WinAnsiEncoding, ou dont le descripteur lève le drapeau
/// Nonsymbolic, se lit par une table code → nom de glyphe : l'encodage nommé (annexe D), ou celui du dictionnaire,
/// « initialized with the entries from the dictionary's BaseEncoding entry », mis à jour par `Differences`, et dont
/// « any undefined entries in the table shall be filled using StandardEncoding ». Une police sans `Encoding`, ou au
/// drapeau Symbolic levé (`Encoding` alors ignoré), se lit par la table « cmap » de son programme : la sous-table (3, 0),
/// sinon (1, 0) ; ses codes n'ont de caractère que par `ToUnicode` (9.10.2), `par_cmap` dit lesquels montrent un glyphe.
/// Ce que la norme ne permet pas de lire se refuse.
fn encodage_truetype(doc: &Document, fd: Option<&Dictionary>, encoding: Option<&Object>, par_cmap: &mut Option<Vec<bool>>) -> Result<Vec<Option<char>>, Refus> {
    // ISO 32000-1, table 111 : `FontDescriptor`, exigé hors des 14 polices standard ; table 122 : `Flags`, exigé.
    let Some(fd) = fd else {
        return illisible("une police TrueType n'a pas de /FontDescriptor, que la norme exige (ISO 32000-1, table 111)");
    };
    let drapeaux = exige(doc, fd, b"Flags", "le descripteur d'une police TrueType", lecture::entier)?;
    // Table 123 : Symbolic (bit 3) et Nonsymbolic (bit 6) « shall not both be set or both be clear ».
    let symbolique = match (drapeau(drapeaux, 3), drapeau(drapeaux, 6)) {
        (true, false) => true,
        (false, true) => false,
        (true, true) | (false, false) => {
            return illisible(format!("les drapeaux {} d'une police TrueType lèvent Symbolic et Nonsymbolic ensemble, ou aucun des deux (ISO 32000-1, table 123)", drapeaux))
        }
    };
    match (symbolique, encoding) {
        (false, Some(Object::Name(n))) => match n.as_slice() {
            b"MacRomanEncoding" | b"WinAnsiEncoding" => Ok(base_encodage(&String::from_utf8_lossy(n))),
            autre => illisible(format!("une police TrueType non symbolique a l'encodage {}, dont 9.6.6.4 d'ISO 32000-1 ne dit pas la lecture", lecture::nom_de(autre))),
        },
        (false, Some(Object::Dictionary(e))) => {
            let table = match lecture::lue(doc, e, b"BaseEncoding", lecture::nom)? {
                Some(n) => base_encodage(&String::from_utf8_lossy(n)),
                None => vec![None; 256],
            };
            let standard = base_encodage("StandardEncoding");
            Ok(appliquer(table, differences_de(doc, e)?).into_iter().zip(standard).map(|(c, s)| c.or(s)).collect())
        }
        (false, Some(_)) => illisible("/Encoding d'une police simple n'est ni un nom ni un dictionnaire"),
        (true, _) | (false, None) => {
            *par_cmap = Some(glyphes_par_cmap(doc, fd)?);
            Ok(vec![None; 256])
        }
    }
}

/// Pour chaque code d'un octet, s'il montre un glyphe par la table « cmap » du programme incorporé (ISO 32000-1,
/// 9.6.6.4) : la sous-table (3, 0), dont les codes sont dans 0x0000-0x00FF, 0xF000-0xF0FF, 0xF100-0xF1FF ou
/// 0xF200-0xF2FF, chaque octet précédé de l'octet de tête de sa plage ; sinon la sous-table (1, 0), octet par octet.
fn glyphes_par_cmap(doc: &Document, fd: &Dictionary) -> Result<Vec<bool>, Refus> {
    let Some(programme) = lecture::lue(doc, fd, b"FontFile2", lecture::flux)? else {
        return illisible("une police TrueType sans encodage lisible n'incorpore pas son programme : la norme lit ses codes par la table « cmap » du programme (ISO 32000-1, 9.6.6.4)");
    };
    let octets = lecture::contenu(doc, programme)?;
    let face = ttf_parser::Face::parse(&octets, 0).map_err(|e| Refus::illisible(format!("le programme incorporé d'une police TrueType ne se lit pas ({})", e)))?;
    let Some(cmap) = face.tables().cmap else {
        return illisible("le programme incorporé d'une police TrueType n'a pas de table « cmap » (ISO 32000-1, 9.6.6.4)");
    };
    let sous_table = |plateforme: ttf_parser::PlatformId, codage: u16| cmap.subtables.into_iter().find(|s| s.platform_id == plateforme && s.encoding_id == codage);
    let montre = |s: &ttf_parser::cmap::Subtable, code: u32| s.glyph_index(code).is_some_and(|g| g.0 != 0);
    if let Some(s) = sous_table(ttf_parser::PlatformId::Windows, 0) {
        let plages: Vec<u32> = [0x00u32, 0xF0, 0xF1, 0xF2].into_iter().filter(|h| (0..=255u32).any(|c| montre(&s, h << 8 | c))).collect();
        return match plages.as_slice() {
            [h] => Ok((0..=255u32).map(|c| montre(&s, h << 8 | c)).collect()),
            [] => Ok(vec![false; 256]),
            _ => illisible("la sous-table (3, 0) d'une police TrueType couvre plusieurs plages de codes, la norme n'en lit qu'une (ISO 32000-1, 9.6.6.4)"),
        };
    }
    match sous_table(ttf_parser::PlatformId::Macintosh, 0) {
        Some(s) => Ok((0..=255u32).map(|c| montre(&s, c)).collect()),
        None => illisible("le programme incorporé d'une police TrueType n'a ni sous-table « cmap » (3, 0) ni (1, 0) (ISO 32000-1, 9.6.6.4)"),
    }
}

/// L'encodage d'une police simple qui n'est pas TrueType : l'encodage nommé, ou le dictionnaire (`BaseEncoding`, sinon
/// StandardEncoding) et ses `Differences`. Table 114 : StandardEncoding est l'encodage implicite d'une police non
/// symbolique non incorporée ; celui d'un programme incorporé est son encodage propre, que la fabrique ne lit pas pour
/// une police Type 1 : elle y lit StandardEncoding aussi.
fn encodage_simple(doc: &Document, encoding: Option<&Object>) -> Result<Vec<Option<char>>, Refus> {
    let (table, differences) = match encoding {
        Some(Object::Name(n)) => (base_encodage(&String::from_utf8_lossy(n)), Vec::new()),
        Some(Object::Dictionary(e)) => {
            let base = match lecture::lue(doc, e, b"BaseEncoding", lecture::nom)? {
                Some(n) => String::from_utf8_lossy(n).into_owned(),
                None => "StandardEncoding".to_string(),
            };
            (base_encodage(&base), differences_de(doc, e)?)
        }
        Some(_) => return illisible("/Encoding d'une police simple n'est ni un nom ni un dictionnaire"),
        None => (base_encodage("StandardEncoding"), Vec::new()),
    };
    Ok(appliquer(table, differences))
}

// ---------------------------------------------------------------------------------------------
// La mise en lignes (la règle de MuPDF).
// ---------------------------------------------------------------------------------------------

// Les seuils sont ceux de MuPDF, retrouvés sur 370 000 couples de caractères consécutifs de nos
// PDF (aucun couple mal classé) : un écart se mesure en corps de la police du caractère posé.

/// Au-dessous de cette avance, le caractère suivant colle au précédent.
const ESPACE_MIN: Mesure = mesure("0.15");
/// Entre `ESPACE_MIN` et celle-ci, un espace est inséré ; au-delà, c'est une autre ligne. Un recul
/// de plus que cela (caractère posé avant le bout du précédent) ouvre aussi une ligne.
const ESPACE_MAX: Mesure = mesure("0.8");
/// Écart de ligne de base toléré (en corps) pour rester sur la même ligne : les exposants
/// (« 1er ») et les indices restent dans la ligne.
const DECALAGE_MAX: Mesure = mesure("0.8");

struct LigneCourante {
    dir: (Mesure, Mesure),
    texte: String,
    /// Une boîte par caractère de `texte` (`None` : un espace ajouté), si `Mise::garder`.
    boites: Vec<Option<Boite>>,
}

#[derive(Default)]
struct Mise {
    lignes: Vec<String>,
    /// Garder la boîte de chaque caractère (`--inspecter`) : `boites[k]` est celle de `lignes[k]`.
    garder: bool,
    boites: Vec<Vec<Option<Boite>>>,
    courante: Option<LigneCourante>,
    /// Le bout du dernier caractère posé.
    stylo: (Mesure, Mesure),
}

impl Mise {
    fn fermer(&mut self) {
        if let Some(l) = self.courante.take() {
            self.lignes.push(l.texte);
            if self.garder {
                self.boites.push(l.boites);
            }
        }
    }

    /// Pose un caractère : origine `p`, vecteur de direction `dir` (déjà à l'échelle du corps),
    /// avance `adv` (fraction d'em).
    fn caractere(&mut self, c: char, p: (Mesure, Mesure), dir: (Mesure, Mesure), taille: Mesure, adv: Option<Mesure>, boite: Boite) {
        let longueur = dir.0.hypot(dir.1);
        let (Some(nx), Some(ny)) = (dir.0.sur(longueur), dir.1.sur(longueur)) else {
            return;
        };
        if taille.est_nulle() {
            return;
        }
        let nd = (nx, ny);
        let mut nouvelle = true;
        let mut espace = false;
        if let Some(l) = &self.courante {
            if (nd.0 - l.dir.0).abs() + (nd.1 - l.dir.1).abs() < mesure("0.05") {
                let d = (p.0 - self.stylo.0, p.1 - self.stylo.1);
                let le_long = d.0 * l.dir.0 + d.1 * l.dir.1;
                let en_travers = -d.0 * l.dir.1 + d.1 * l.dir.0;
                // Le corps n'est pas nul (plus haut) : le quotient existe.
                let Some(ecart) = le_long.sur(taille) else {
                    return;
                };
                if en_travers.abs() <= taille * DECALAGE_MAX {
                    if ecart >= -ESPACE_MAX && ecart < ESPACE_MIN {
                        nouvelle = false;
                    } else if ecart >= ESPACE_MIN && ecart < ESPACE_MAX {
                        nouvelle = false;
                        espace = true;
                    }
                }
            }
        }
        if nouvelle {
            self.fermer();
            self.courante = Some(LigneCourante { dir: nd, texte: String::new(), boites: Vec::new() });
        }
        let garder = self.garder;
        let l = self.courante.as_mut().unwrap();
        if espace && !l.texte.ends_with(' ') && c != ' ' {
            l.texte.push(' ');
            if garder {
                l.boites.push(None);
            }
        }
        l.texte.push(c);
        if garder {
            l.boites.push(Some(boite));
        }
        // Le 2e caractère d'un glyphe qui en vaut plusieurs (« fi ») n'avance pas le stylo.
        if let Some(adv) = adv {
            self.stylo = (p.0 + adv * dir.0, p.1 + adv * dir.1);
        }
    }

    /// Les lignes avec la boîte de chacun de leurs caractères (`Mise::garder`).
    fn finir_avec_boites(mut self) -> Vec<(String, Vec<Option<Boite>>)> {
        self.fermer();
        self.lignes.into_iter().zip(self.boites).collect()
    }

    fn finir(mut self) -> String {
        self.fermer();
        let mut t = String::new();
        for l in self.lignes {
            t.push_str(&l);
            t.push('\n');
        }
        t
    }
}

// ---------------------------------------------------------------------------------------------
// L'interpréteur.
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
struct Etat {
    ctm: Mat,
    tc: Mesure,
    tw: Mesure,
    th: Mesure,
    tl: Mesure,
    police: Option<Rc<Police>>,
    taille: Mesure,
    rendu: i64,
    rise: Mesure,
}

struct Interprete<'a> {
    doc: &'a Document,
    polices: HashMap<ObjectId, Rc<Police>>,
    mise: Mise,
    /// Le cadre de la page (x0, y0, x1, y1) : le texte tout entier hors de lui n'est pas lu.
    cadre: [Mesure; 4],
    /// Le `/ActualText` en cours : son texte, le premier glyphe (origine, direction, corps) et
    /// l'avance totale des glyphes qu'il remplace.
    actuel: Option<Actuel>,
    /// Ce que `--inspecter` relève en plus (images, chemins peints, polices) ; `None` pour `--texte`.
    releve: Option<Releve>,
    /// Vrai pendant la lecture des apparences d'annotations (leurs polices ne sont pas relevées).
    annotation: bool,
}

/// Ce que l'interpréteur relève pour `--inspecter`, dans le repère de la page du PDF.
#[derive(Default)]
pub(crate) struct Releve {
    pub images: Vec<Boite>,
    pub traits: Vec<Boite>,
    pub polices: Vec<String>,
    /// Les points du dernier chemin peint s'il n'a été que rempli (voir `peindre`).
    dernier_rempli: Option<Vec<(Mesure, Mesure)>>,
}

impl Releve {
    /// Un chemin peint : sa boîte (épaisseur du trait non comptée). Comme PyMuPDF, le tracé d'un
    /// chemin que l'opération de dessin précédente vient de remplir (même points) n'ajoute rien : les
    /// deux ne font qu'un seul dessin (« fs »).
    fn peindre(&mut self, points: &[(Mesure, Mesure)], trace: bool, rempli: bool) {
        let precedent = self.dernier_rempli.take();
        if points.len() < 2 {
            return;
        }
        if trace && precedent.as_deref() == Some(points) {
            return;
        }
        let mut b = [points[0].0, points[0].1, points[0].0, points[0].1];
        for p in points {
            b = [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)];
        }
        self.traits.push(b);
        if rempli {
            self.dernier_rempli = Some(points.to_vec());
        }
    }
}

struct Actuel {
    texte: String,
    premier: Option<((Mesure, Mesure), (Mesure, Mesure), Mesure)>,
    adv: Mesure,
    /// La matrice de rendu du premier glyphe, son ascendant et son descendant.
    trm: Mat,
    asc: Mesure,
    desc: Mesure,
}

/// Remplace chaque image en ligne (`BI … ID données EI`) par l'opérateur `PPBI`, que l'analyseur
/// de lopdf (qui ne connaît pas les données binaires) lit sans broncher. Sans `BI`, le flux est
/// rendu tel quel.
fn remplacer_images_en_ligne(flux: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    if !flux.windows(2).any(|w| w == b"BI") {
        return std::borrow::Cow::Borrowed(flux);
    }
    let blanc = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0c | 0);
    let delim = |c: u8| matches!(c, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%');
    let mut sortie = Vec::with_capacity(flux.len());
    let mut i = 0;
    let mut copie = 0; // ce qui précède a été recopié
    while i < flux.len() {
        match flux[i] {
            b'%' => {
                while i < flux.len() && flux[i] != b'\n' && flux[i] != b'\r' {
                    i += 1;
                }
            }
            b'(' => {
                let mut prof = 0;
                while i < flux.len() {
                    match flux[i] {
                        b'\\' => i += 1,
                        b'(' => prof += 1,
                        b')' => {
                            prof -= 1;
                            if prof == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                i += 1;
            }
            c if blanc(c) || delim(c) => i += 1,
            _ => {
                let debut = i;
                while i < flux.len() && !blanc(flux[i]) && !delim(flux[i]) {
                    i += 1;
                }
                if &flux[debut..i] != b"BI" {
                    continue;
                }
                // Les données commencent après « ID » et un blanc ; elles finissent à « EI ».
                let id = flux[i..].windows(3).position(|w| blanc(w[0]) && w[1] == b'I' && w[2] == b'D').map(|p| i + p + 3);
                let donnees = match id {
                    Some(d) => d + 1,
                    None => continue,
                };
                let fin = (donnees..flux.len().saturating_sub(1)).find(|&k| {
                    flux[k] == b'E'
                        && flux[k + 1] == b'I'
                        && (k == 0 || blanc(flux[k - 1]))
                        && !matches!(flux.get(k + 2), Some(c) if !blanc(*c) && !delim(*c))
                });
                if let Some(k) = fin {
                    sortie.extend_from_slice(&flux[copie..debut]);
                    sortie.extend_from_slice(b" PPBI ");
                    i = k + 2;
                    copie = i;
                }
            }
        }
    }
    sortie.extend_from_slice(&flux[copie.min(flux.len())..]);
    std::borrow::Cow::Owned(sortie)
}

/// Le texte d'une chaîne PDF : UTF-16BE s'il commence par la marque, sinon Latin-1 (approchant
/// PDFDocEncoding).
fn texte_pdf(s: &[u8]) -> Result<String, Refus> {
    if s.starts_with(&[0xFE, 0xFF]) {
        utf16(&s[2..])
    } else {
        Ok(s.iter().map(|b| *b as char).collect())
    }
}

/// Au-delà, des objets formulaire imbriqués (qui se citent l'un l'autre) se refusent.
const PROFONDEUR_MAX: usize = 12;

/// La police du texte : « There is no initial value for either font or size; they shall be specified explicitly by
/// using Tf before any text is shown » (ISO 32000-1, 9.3.1).
fn police_choisie(e: &Etat) -> Result<&Rc<Police>, Refus> {
    match &e.police {
        Some(p) => Ok(p),
        None => illisible("du texte se montre avant tout Tf, sans police"),
    }
}

impl<'a> Interprete<'a> {
    /// Pose le texte réel d'un `/ActualText` refermé, réparti sur la place des glyphes remplacés.
    fn finir_actuel(&mut self) {
        let a = match self.actuel.take() {
            Some(a) => a,
            None => return,
        };
        let ((x, y), dir, taille) = match a.premier {
            Some(p) => p,
            None => return,
        };
        let n = a.texte.chars().count().max(1) as i64;
        for (k, c) in a.texte.chars().enumerate() {
            let decalage = a.adv * k as i64 / n;
            let boite = englobante(&a.trm, decalage, -a.desc, decalage + a.adv / n, a.asc);
            self.mise.caractere(c, (x + decalage * dir.0, y + decalage * dir.1), dir, taille, Some(a.adv / n), boite);
        }
    }

    /// La police que `Tf` nomme : « the name of a font resource in the Font subdictionary of the current resource
    /// dictionary » (ISO 32000-1, 9.3.1) ; une police que les ressources n'ont pas se refuse.
    fn police(&mut self, res: &Dictionary, nom_police: &[u8]) -> Result<Rc<Police>, Refus> {
        let doc = self.doc;
        let Some(polices) = lecture::lue(doc, res, b"Font", lecture::dictionnaire)? else {
            return illisible(format!("Tf nomme la police {}, et les ressources n'ont pas de /Font", lecture::nom_de(nom_police)));
        };
        let Some(entree) = polices.as_hashmap().get(nom_police) else {
            return illisible(format!("Tf nomme la police {}, que les ressources n'ont pas", lecture::nom_de(nom_police)));
        };
        let quoi = format!("la police {}", lecture::nom_de(nom_police));
        if let Object::Reference(id) = entree {
            if let Some(p) = self.polices.get(id) {
                return Ok(p.clone());
            }
            let d = lecture::element(doc, entree, &quoi, lecture::dictionnaire)?;
            let p = Rc::new(charger_police(doc, d, self.releve.is_some()).map_err(|r| r.reformuler(|m| format!("{} : {}", quoi, m)))?);
            self.polices.insert(*id, p.clone());
            return Ok(p);
        }
        let d = lecture::element(doc, entree, &quoi, lecture::dictionnaire)?;
        Ok(Rc::new(charger_police(doc, d, self.releve.is_some()).map_err(|r| r.reformuler(|m| format!("{} : {}", quoi, m)))?))
    }

    /// Relève les `/BaseFont` des polices d'un dictionnaire de ressources (page ou formulaire suivi),
    /// dans l'ordre, sans doublon.
    fn relever_polices(&mut self, res: &Dictionary) -> Result<(), Refus> {
        if self.annotation {
            return Ok(());
        }
        let doc = self.doc;
        let Some(r) = self.releve.as_mut() else {
            return Ok(());
        };
        let Some(polices) = lecture::lue(doc, res, b"Font", lecture::dictionnaire)? else {
            return Ok(());
        };
        for (cle, entree) in polices.iter() {
            let d = lecture::element(doc, entree, &format!("la police {}", lecture::nom_de(cle)), lecture::dictionnaire)?;
            if let Some(nom) = nom_de_base(doc, d)? {
                if !nom.is_empty() && !r.polices.contains(&nom) {
                    r.polices.push(nom);
                }
            }
        }
        Ok(())
    }

    /// Le flux tel que l'analyseur le lira : pour `--inspecter`, les images en ligne remplacées.
    fn preparer<'f>(&self, flux: &'f [u8]) -> std::borrow::Cow<'f, [u8]> {
        if self.releve.is_some() {
            remplacer_images_en_ligne(flux)
        } else {
            std::borrow::Cow::Borrowed(flux)
        }
    }

    fn executer(&mut self, flux: &[u8], res: &Dictionary, ctm: Mat, profondeur: usize) -> Result<(), Refus> {
        let doc = self.doc;
        let flux = self.preparer(flux);
        let contenu = Content::decode(&flux).map_err(|e| Refus::illisible(format!("un flux de contenu ne se lit pas ({})", e)))?;
        let mut e = Etat { ctm, tc: Z, tw: Z, th: Mesure::UN, tl: Z, police: None, taille: Z, rendu: 0, rise: Z };
        let mut pile: Vec<Etat> = Vec::new();
        let mut tm = ID;
        let mut tlm = ID;
        // Pour chaque contenu marqué ouvert : a-t-il ouvert un `/ActualText` ?
        let mut marques: Vec<bool> = Vec::new();
        // Le chemin en construction (ses points, dans le repère de la page), pour `--inspecter`.
        let mut points: Vec<(Mesure, Mesure)> = Vec::new();
        for op in &contenu.operations {
            let a = &op.operands;
            let num = |i: usize| -> Result<Mesure, Refus> {
                match a.get(i) {
                    Some(o) => lecture::element(doc, o, &format!("l'opérande {} de {}", i + 1, op.operator), lecture::nombre),
                    None => illisible(format!("{} n'a pas son opérande {}", op.operator, i + 1)),
                }
            };
            match op.operator.as_str() {
                "q" => pile.push(e.clone()),
                "Q" => {
                    if let Some(s) = pile.pop() {
                        e = s;
                    }
                }
                "cm" => e.ctm = mul(&[num(0)?, num(1)?, num(2)?, num(3)?, num(4)?, num(5)?], &e.ctm),
                "BT" => {
                    tm = ID;
                    tlm = ID;
                }
                "Tc" => e.tc = num(0)?,
                "Tw" => e.tw = num(0)?,
                "Tz" => e.th = num(0)? / 100,
                "TL" => e.tl = num(0)?,
                "Tr" => e.rendu = num(0)?.tronque(),
                "Ts" => e.rise = num(0)?,
                "Tf" => {
                    match a.first() {
                        Some(Object::Name(n)) => e.police = Some(self.police(res, n)?),
                        Some(_) | None => return illisible("Tf ne nomme pas de police"),
                    }
                    e.taille = num(1)?;
                }
                "Td" => {
                    tlm = mul(&translation(num(0)?, num(1)?), &tlm);
                    tm = tlm;
                }
                "TD" => {
                    e.tl = -num(1)?;
                    tlm = mul(&translation(num(0)?, num(1)?), &tlm);
                    tm = tlm;
                }
                "Tm" => {
                    tlm = [num(0)?, num(1)?, num(2)?, num(3)?, num(4)?, num(5)?];
                    tm = tlm;
                }
                "T*" => {
                    tlm = mul(&translation(Z, -e.tl), &tlm);
                    tm = tlm;
                }
                "Tj" | "'" | "\"" => {
                    if op.operator != "Tj" {
                        if op.operator == "\"" {
                            e.tw = num(0)?;
                            e.tc = num(1)?;
                        }
                        tlm = mul(&translation(Z, -e.tl), &tlm);
                        tm = tlm;
                    }
                    match a.last() {
                        Some(Object::String(s, _)) => self.montrer(&e, &mut tm, s)?,
                        Some(_) | None => return illisible(format!("{} ne montre pas une chaîne", op.operator)),
                    }
                }
                "TJ" => {
                    let Some(Object::Array(v)) = a.first() else {
                        return illisible("TJ ne montre pas un tableau");
                    };
                    for x in v {
                        match x {
                            Object::String(s, _) => self.montrer(&e, &mut tm, s)?,
                            Object::Integer(_) | Object::Real(_) => {
                                let n = lecture::element(doc, x, "un décalage de TJ", lecture::nombre)?;
                                let vertical = police_choisie(&e)?.vertical;
                                let t = -(n * e.taille) / 1000;
                                let d = if vertical { translation(Z, t) } else { translation(t * e.th, Z) };
                                tm = mul(&d, &tm);
                            }
                            _ => return illisible("un élément de TJ n'est ni une chaîne ni un nombre"),
                        }
                    }
                }
                // Le contenu marqué : un `/ActualText` remplace les glyphes qu'il enveloppe (MuPDF
                // fait de même : Word écrit ainsi ses espaces, par exemple).
                "BMC" => marques.push(false),
                "BDC" => {
                    // ISO 32000-1, 14.6.2 : les propriétés, un dictionnaire en ligne ou le nom d'une ressource de
                    // `/Properties`.
                    let propres = match a.get(1) {
                        Some(Object::Name(n)) => {
                            let Some(p) = lecture::lue(doc, res, b"Properties", lecture::dictionnaire)? else {
                                return illisible(format!("BDC nomme {}, et les ressources n'ont pas de /Properties", lecture::nom_de(n)));
                            };
                            match lecture::lue(doc, p, n, lecture::dictionnaire)? {
                                Some(d) => d,
                                None => return illisible(format!("BDC nomme {}, que /Properties n'a pas", lecture::nom_de(n))),
                            }
                        }
                        Some(o) => lecture::element(doc, o, "les propriétés de BDC", lecture::dictionnaire)?,
                        None => return illisible("BDC n'a pas ses propriétés"),
                    };
                    let reel = match lecture::lue(doc, propres, b"ActualText", lecture::chaine)? {
                        Some(s) => Some(texte_pdf(s)?),
                        None => None,
                    };
                    match reel {
                        Some(t) if self.actuel.is_none() => {
                            self.actuel = Some(Actuel { texte: t, premier: None, adv: Z, trm: ID, asc: mesure("0.8"), desc: mesure("0.2") });
                            marques.push(true);
                        }
                        _ => marques.push(false),
                    }
                }
                "EMC" => {
                    if marques.pop() == Some(true) {
                        self.finir_actuel();
                    }
                }
                // Les chemins : leur boîte, relevée quand ils sont peints.
                "m" | "l" => points.push(point(&e.ctm, num(0)?, num(1)?)),
                "c" => {
                    for k in 0..3 {
                        points.push(point(&e.ctm, num(2 * k)?, num(2 * k + 1)?));
                    }
                }
                "v" | "y" => {
                    for k in 0..2 {
                        points.push(point(&e.ctm, num(2 * k)?, num(2 * k + 1)?));
                    }
                }
                "re" => {
                    let (x, y, w, h) = (num(0)?, num(1)?, num(2)?, num(3)?);
                    for (px, py) in [(x, y), (x + w, y), (x + w, y + h), (x, y + h)] {
                        points.push(point(&e.ctm, px, py));
                    }
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    if let Some(r) = self.releve.as_mut() {
                        r.peindre(&points, matches!(op.operator.as_str(), "S" | "s"), matches!(op.operator.as_str(), "f" | "F" | "f*"));
                    }
                    points.clear();
                }
                "n" => points.clear(),
                // Une image en ligne (voir `remplacer_images_en_ligne`) : le carré unité sous la CTM.
                "PPBI" => {
                    if let Some(r) = self.releve.as_mut() {
                        r.images.push(englobante(&e.ctm, Z, Z, Mesure::UN, Mesure::UN));
                    }
                }
                "Do" => {
                    if profondeur >= PROFONDEUR_MAX {
                        return illisible(format!("des objets formulaire imbriqués au-delà de {}", PROFONDEUR_MAX));
                    }
                    match a.first() {
                        Some(Object::Name(n)) => self.objet(res, n, &e.ctm, profondeur)?,
                        Some(_) | None => return illisible("Do ne nomme pas d'objet"),
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Un objet extérieur (`Do`) : une image (sa boîte), ou un objet formulaire (son flux, sous sa matrice, avec ses
    /// ressources).
    fn objet(&mut self, res: &Dictionary, nom_obj: &[u8], ctm: &Mat, profondeur: usize) -> Result<(), Refus> {
        let doc = self.doc;
        let quoi = format!("l'objet {}", lecture::nom_de(nom_obj));
        let Some(xo) = lecture::lue(doc, res, b"XObject", lecture::dictionnaire)? else {
            return illisible(format!("Do nomme {}, et les ressources n'ont pas de /XObject", quoi));
        };
        let s = match lecture::lue(doc, xo, nom_obj, lecture::flux)? {
            Some(s) => s,
            None => return illisible(format!("Do nomme {}, que /XObject n'a pas", quoi)),
        };
        // ISO 32000-1, 8.8 : `Subtype` (exigé) est `Image`, `Form` ou `PS` ; un objet PostScript n'a « no effect
        // either when viewing the document on-screen or when printing it to a non-PostScript device » (8.8.2).
        match exige(doc, &s.dict, b"Subtype", &quoi, lecture::nom)? {
            b"Image" => {
                if let Some(r) = self.releve.as_mut() {
                    r.images.push(englobante(ctm, Z, Z, Mesure::UN, Mesure::UN));
                }
                Ok(())
            }
            b"PS" => Ok(()),
            b"Form" => {
                let m = matrice_de_formulaire(doc, &s.dict)?;
                // ISO 32000-1, table 95 : sans ressources propres, un objet formulaire prend celles du contenu où il est
                // posé (« all named resources used in the form XObject shall be included in the resource dictionary of
                // each page object on which the form XObject appears », PDF 1.1).
                let res_form = match lecture::lue(doc, &s.dict, b"Resources", lecture::dictionnaire)? {
                    Some(r) => {
                        self.relever_polices(r)?;
                        r
                    }
                    None => res,
                };
                let flux = lecture::contenu(doc, s)?;
                self.executer(&flux, res_form, mul(&m, ctm), profondeur + 1)
            }
            autre => illisible(format!("{} est de sous-type {}, que la norme ne connaît pas", quoi, lecture::nom_de(autre))),
        }
    }

    /// Montre une chaîne : un caractère par code, le texte avance de la largeur de chaque glyphe.
    fn montrer(&mut self, e: &Etat, tm: &mut Mat, chaine: &[u8]) -> Result<(), Refus> {
        let police = police_choisie(e)?.clone();
        for (code, n) in police.codes(chaine)? {
            let w0 = police.largeur(code);
            let trm = mul(&[e.taille * e.th, Z, Z, e.taille, Z, e.rise], &mul(tm, &e.ctm));
            let texte = police.texte(code)?;
            let visible = self.dans_le_cadre(&trm, w0);
            let boite = englobante(&trm, Z, -police.desc, w0, police.asc);
            // Le corps rendu : la racine du déterminant de la matrice de rendu (positive : sa valeur absolue).
            let taille = (trm[0] * trm[3] - trm[1] * trm[2]).abs().racine().ok_or_else(|| Refus::defaut("une valeur absolue sans racine"))?;
            if let Some(a) = &mut self.actuel {
                // Sous un `/ActualText` : on ne garde que la position du premier glyphe et
                // l'avance totale ; le texte réel se pose à la fermeture.
                if a.premier.is_none() {
                    a.premier = Some(((trm[4], trm[5]), (trm[0], trm[1]), taille));
                    (a.trm, a.asc, a.desc) = (trm, police.asc, police.desc);
                }
                a.adv += w0;
            } else if visible {
                let dir = if police.vertical { (trm[2], trm[3]) } else { (trm[0], trm[1]) };
                for (k, c) in texte.iter().flat_map(|t| t.chars()).enumerate() {
                    self.mise.caractere(c, (trm[4], trm[5]), dir, taille, if k == 0 { Some(w0) } else { None }, boite);
                }
            }
            let mut tx = w0 * e.taille + e.tc;
            if n == 1 && code == 32 {
                tx += e.tw;
            }
            *tm = if police.vertical {
                mul(&translation(Z, -tx.abs()), tm)
            } else {
                mul(&translation(tx * e.th, Z), tm)
            };
        }
        Ok(())
    }

    fn dans_le_cadre(&self, trm: &Mat, w0: Mesure) -> bool {
        let c = &self.cadre;
        let p = (trm[4], trm[5]);
        let q = (p.0 + w0 * trm[0], p.1 + w0 * trm[1]);
        let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
        let (y0, y1) = (p.1.min(q.1) - trm[3].abs() * mesure("0.25"), p.1.max(q.1) + trm[3].abs() * mesure("0.75"));
        !(x1 < c[0] || x0 > c[2] || y1 < c[1] || y0 > c[3])
    }
}

// ---------------------------------------------------------------------------------------------
// Les pages.
// ---------------------------------------------------------------------------------------------

/// Les drapeaux d'une annotation qui ne se montre pas : Hidden (2) et NoView (32) (ISO 32000-1, 12.5.3, table 165).
const CACHEE: i64 = 2 | 32;
/// Les drapeaux d'une annotation qui n'en dit pas (ISO 32000-1, table 164 : « Default value: 0 »).
const DRAPEAUX_PAR_DEFAUT: i64 = 0;

/// L'échelle d'un côté de l'apparence : ce qui l'amène sur le rectangle de l'annotation (ISO 32000-1, 12.5.5). Une
/// apparence sans épaisseur de ce côté n'a rien à étirer.
fn echelle(rect: (Mesure, Mesure), boite: (Mesure, Mesure)) -> Mesure {
    match (rect.1 - rect.0).sur(boite.1 - boite.0) {
        Some(s) if boite.1 > boite.0 => s,
        Some(_) | None => Mesure::UN,
    }
}

/// Lit une page : son contenu, puis l'apparence de ses annotations (champs de formulaire). Avec
/// `inspecter`, l'interpréteur garde en plus la boîte des caractères, les images, les chemins
/// peints et les polices.
fn lire_la_page(doc: &Document, id: ObjectId, inspecter: bool) -> Result<Interprete<'_>, Refus> {
    let res = lecture::ressources_de_page(doc, id)?;
    let mise = Mise { garder: inspecter, ..Mise::default() };
    let releve = if inspecter { Some(Releve::default()) } else { None };
    let mut it = Interprete { doc, polices: HashMap::new(), mise, cadre: lecture::cadre_de(doc, id)?, actuel: None, releve, annotation: false };
    it.relever_polices(res)?;
    let flux = lecture::contenu_de_page(doc, id)?;
    if !flux.is_empty() {
        it.executer(&flux, res, ID, 0)?;
    }
    // Les annotations (Popup, cachées ou sans apparence exclues).
    let annots = match lecture::lue(doc, lecture::dictionnaire_de(doc, id)?, b"Annots", lecture::tableau)? {
        Some(v) => v,
        None => &[],
    };
    it.annotation = true;
    for a in annots {
        let d = lecture::element(doc, a, "une annotation de /Annots", lecture::dictionnaire)?;
        // ISO 32000-1, table 164 : `F`, « Default value: 0 » ; `Subtype` est exigé.
        let drapeaux = match lecture::lue(doc, d, b"F", lecture::entier)? {
            Some(f) => f,
            None => DRAPEAUX_PAR_DEFAUT,
        };
        if drapeaux & CACHEE != 0 || exige(doc, d, b"Subtype", "une annotation", lecture::nom)? == b"Popup" {
            continue;
        }
        // Table 168 : `N` est exigé dans `AP`, un flux ou un dictionnaire d'états, que `AS` choisit (table 164 :
        // exigé quand `AP` a des sous-dictionnaires). Un état que `N` n'a pas ne se dessine pas.
        let Some(ap) = lecture::lue(doc, d, b"AP", lecture::dictionnaire)? else {
            continue;
        };
        let s = match lecture::valeur(doc, ap, b"N")? {
            Some(Object::Stream(s)) => s,
            Some(Object::Dictionary(etats)) => {
                let etat = exige(doc, d, b"AS", "une annotation dont l'apparence a des états", lecture::nom)?;
                match lecture::lue(doc, etats, etat, lecture::flux)? {
                    Some(s) => s,
                    None => continue,
                }
            }
            Some(_) => return illisible("/N de l'apparence d'une annotation n'est ni un flux ni un dictionnaire"),
            None => return illisible("l'apparence d'une annotation n'a pas de /N, que la norme exige (ISO 32000-1, table 168)"),
        };
        let rect = exige(doc, d, b"Rect", "une annotation", |o| Ok(o))?;
        let rect = lecture::rectangle_de(doc, rect, "/Rect")?;
        let bbox = match lecture::rectangle(doc, &s.dict, b"BBox")? {
            Some(b) => b,
            None => return illisible("l'apparence d'une annotation n'a pas de /BBox, que la norme exige (ISO 32000-1, table 95)"),
        };
        let m = matrice_de_formulaire(doc, &s.dict)?;
        let coins = [point(&m, bbox[0], bbox[1]), point(&m, bbox[2], bbox[1]), point(&m, bbox[0], bbox[3]), point(&m, bbox[2], bbox[3])];
        let (bx0, bx1) = coins.iter().fold((Mesure::MAX, Mesure::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
        let (by0, by1) = coins.iter().fold((Mesure::MAX, Mesure::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
        let (rx0, rx1) = (rect[0], rect[2]);
        let (ry0, ry1) = (rect[1], rect[3]);
        let sx = echelle((rx0, rx1), (bx0, bx1));
        let sy = echelle((ry0, ry1), (by0, by1));
        let a_m = [sx, Z, Z, sy, rx0 - bx0 * sx, ry0 - by0 * sy];
        // Sans ressources propres, celles de la page (table 95, comme un objet formulaire).
        let res_ap = match lecture::lue(doc, &s.dict, b"Resources", lecture::dictionnaire)? {
            Some(r) => r,
            None => res,
        };
        let contenu = lecture::contenu(doc, s)?;
        it.executer(&contenu, res_ap, mul(&m, &a_m), 1)?;
    }
    Ok(it)
}

/// Le texte d'une page : son contenu, puis l'apparence de ses annotations (champs de formulaire).
fn texte_de_page(doc: &Document, id: ObjectId) -> Result<String, Refus> {
    Ok(lire_la_page(doc, id, false)?.mise.finir())
}

/// Ce que `--inspecter` relève d'une page, dans le repère du PDF (origine en bas à gauche).
pub(crate) struct PageLue {
    /// Les lignes de texte (celles de `--texte`) avec la boîte de chacun de leurs caractères.
    pub lignes: Vec<(String, Vec<Option<Boite>>)>,
    pub images: Vec<Boite>,
    pub traits: Vec<Boite>,
    pub polices: Vec<String>,
}

pub(crate) fn inspecter_page(doc: &Document, id: ObjectId) -> Result<PageLue, Refus> {
    let it = lire_la_page(doc, id, true)?;
    let Some(releve) = it.releve else {
        return defaut("une page inspectée sans son relevé");
    };
    Ok(PageLue { lignes: it.mise.finir_avec_boites(), images: releve.images, traits: releve.traits, polices: releve.polices })
}

/// Le texte de chaque page du document. Une page qui ne se lit pas donne un texte vide et un avertissement qui dit
/// pourquoi (rendu à l'appelant, qui en décide) : le reste du document se lit.
pub fn texte_des_pages(doc: &Document) -> (Vec<String>, Vec<String>) {
    let mut textes = Vec::new();
    let mut avertissements = Vec::new();
    for (n, id) in doc.get_pages() {
        match texte_de_page(doc, id) {
            Ok(t) => textes.push(t),
            Err(r) => {
                avertissements.push(format!("page {} : {}", n, r.phrase));
                textes.push(String::new());
            }
        }
    }
    (textes, avertissements)
}
