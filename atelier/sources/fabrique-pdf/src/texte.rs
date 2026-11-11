//! L'extraction du texte d'un PDF (`fabrique-pdf --texte`) : le texte de chaque page, dans
//! l'ordre où le PDF l'écrit, au plus près de `page.get_text()` de PyMuPDF (MuPDF).
//!
//! Trois étages :
//!
//! 1. un interpréteur du flux de contenu (états graphiques, matrices, opérateurs de texte,
//!    objets formulaire, apparences des annotations — MuPDF dessine les champs de formulaire, et
//!    leur texte sort avec celui de la page) qui rend des caractères positionnés ;
//! 2. les polices, dans l'ordre d'ISO 32000-1, 9.10.2 : la table `ToUnicode` quand la police en a
//!    une ; sinon les noms de glyphes de l'encodage d'une police simple (un encodage prédéfini, les
//!    `/Differences`, l'encodage propre d'un programme Type 1 incorporé) lus par la liste des glyphes
//!    d'Adobe ; sinon le programme TrueType incorporé (sa « cmap » mène au glyphe, qui dit son
//!    caractère par son nom ou par la « cmap » Unicode du programme). Un glyphe que rien ne dit est
//!    un signe inconnu du texte (`Inconnu` : la police, le code), à sa place ; seul le lecteur qui en
//!    a besoin le refuse. Les largeurs viennent du PDF, ou de Liberation pour les polices standard
//!    sans `/Widths` ;
//! 3. la mise en lignes : un caractère prolonge la ligne courante s'il part du bout du précédent
//!    (à un espace près) sur la même ligne de base ; sinon il ouvre une ligne. C'est la règle de
//!    MuPDF, pas un tri géométrique : le texte sort dans l'ordre du flux.
//!
//! Le résultat est le texte de chaque page (`TexteDePage`), les lignes séparées par `\n`, un `\n` final.

use std::collections::HashMap;
use std::rc::Rc;

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::description::{defaut, illisible, Classe, Refus};
use crate::encodages::{self, Code, Encodage};
use crate::glyphes;
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

/// Ce qu'un code (ou un glyphe) d'une police montre, en texte.
#[derive(Clone)]
enum Lu {
    /// Ses caractères.
    Texte(String),
    /// Aucun glyphe (`.notdef`) : aucun caractère.
    Rien,
    /// Un glyphe dont ni le PDF ni la norme ne disent le caractère : pourquoi. Il se refuse quand il se montre
    /// (ISO 32000-1, 9.10.2 : « there is no way to determine what the character code represents »).
    Inconnu(String),
}

/// Un caractère que rien ne dit (ISO 32000-1, 9.10.2 : « there is no way to determine what the character code
/// represents ») : la police qui le montre (son `/BaseFont`), son code, et pourquoi rien ne le dit. Il est un signe du
/// texte, à sa place ; seul le lecteur qui a besoin de ce caractère refuse, en le nommant.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Inconnu {
    pub police: String,
    pub code: u32,
    pub pourquoi: String,
}

impl Inconnu {
    /// La phrase qui le nomme, sur la page `page`.
    pub fn phrase(&self, page: u32) -> String {
        format!(
            "page {} : la police {} montre le code {} sans /ToUnicode : {} ; rien ne dit son caractère (ISO 32000-1, 9.10.2)",
            page, self.police, self.code, self.pourquoi
        )
    }
}

/// Un signe du texte : un caractère, ou un caractère que rien ne dit.
#[derive(Clone, Debug, PartialEq)]
pub enum Signe {
    Caractere(char),
    Inconnu(Inconnu),
}

/// Un morceau du texte d'une page : des caractères, ou un caractère que rien ne dit, tel qu'il est.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum Morceau {
    Texte(String),
    Inconnu(Inconnu),
}

/// Le texte d'une page : ses lignes, chacune suivie de `\n`, en morceaux.
#[derive(Clone, Debug, PartialEq)]
pub struct TexteDePage {
    /// Son numéro dans le document, à partir de 1.
    pub page: u32,
    pub morceaux: Vec<Morceau>,
}

impl TexteDePage {
    /// Les morceaux d'une page, à partir de ses lignes.
    fn de_lignes(page: u32, lignes: Vec<Vec<Signe>>) -> TexteDePage {
        let mut morceaux = Vec::new();
        let mut courant = String::new();
        for ligne in lignes {
            for s in ligne {
                match s {
                    Signe::Caractere(c) => courant.push(c),
                    Signe::Inconnu(i) => {
                        if !courant.is_empty() {
                            morceaux.push(Morceau::Texte(std::mem::take(&mut courant)));
                        }
                        morceaux.push(Morceau::Inconnu(i));
                    }
                }
            }
            courant.push('\n');
        }
        if !courant.is_empty() {
            morceaux.push(Morceau::Texte(courant));
        }
        TexteDePage { page, morceaux }
    }

    /// Le texte de la page à montrer à quelqu'un : chaque caractère que rien ne dit, tel qu'il est
    /// (« ⟦code C de la police X⟧ »).
    pub fn montre(&self) -> String {
        let mut t = String::new();
        for m in &self.morceaux {
            match m {
                Morceau::Texte(s) => t.push_str(s),
                Morceau::Inconnu(i) => t.push_str(&format!("⟦code {} de la police {}⟧", i.code, i.police)),
            }
        }
        t
    }

    /// Le texte de la page pour un lecteur qui en a besoin en entier : un caractère que rien ne dit le refuse, nommé
    /// (la page, la police, le code).
    pub fn lu(&self) -> Result<String, Refus> {
        let mut t = String::new();
        for m in &self.morceaux {
            match m {
                Morceau::Texte(s) => t.push_str(s),
                Morceau::Inconnu(i) => return illisible(i.phrase(self.page)),
            }
        }
        Ok(t)
    }
}

/// Le texte d'une ligne, quand chacun de ses signes est un caractère.
pub(crate) fn caracteres_de(ligne: &[Signe]) -> Option<String> {
    ligne
        .iter()
        .map(|s| match s {
            Signe::Caractere(c) => Some(*c),
            Signe::Inconnu(_) => None,
        })
        .collect()
}

/// D'une police composée sans `ToUnicode` pour un code, ce que son programme dit de chaque glyphe.
enum ParProgramme {
    /// Un programme TrueType (CIDFontType2) : le CID mène au glyphe par `CIDToGIDMap` (ses octets ; `None` : Identity,
    /// la valeur par défaut, table 117), et chaque glyphe dit ses caractères par son nom ou par la « cmap » Unicode du
    /// programme.
    Glyphes { cid_gid: Option<Vec<u8>>, lus: HashMap<u16, Lu> },
    /// Rien ne dit le caractère d'un glyphe : pourquoi.
    Aucun(String),
}

/// Une police simple, ou composée.
enum Sorte {
    /// Le texte de chaque code (256) d'après l'encodage.
    Simple { encodage: Vec<Lu> },
    /// Les espaces de codes (longueur en octets, plus petit, plus grand), la table des CID d'un CMap incorporé (`None` :
    /// Identity), et ce que le programme dit des glyphes.
    Composee { espaces: Vec<(usize, u32, u32)>, cmap_cid: Option<Cmap>, programme: ParProgramme },
}

/// Ce que l'interpréteur sait d'une police : décoder une chaîne en codes, et dire pour chaque code
/// son texte et sa largeur (millièmes d'em).
struct Police {
    /// Son `/BaseFont`, pour nommer la police dans un refus.
    nom: String,
    sorte: Sorte,
    unicode: HashMap<u32, String>,
    largeurs: HashMap<u32, Mesure>,
    largeur_defaut: Mesure,
    /// Largeurs tirées de Liberation pour une police standard sans `/Widths`.
    standard: Option<crate::polices::Metrique>,
    vertical: bool,
    /// Ascendant et descendant (fractions du corps), qui bornent la boîte d'un glyphe pour
    /// `--inspecter` : ceux du fichier de police incorporé, sinon (polices standard) ceux de leur fichier de métriques d'Adobe,
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

/// L'ascendant et le descendant d'une police (fractions du corps) : ceux du fichier de police incorporé (`hhea`) ;
/// pour une police standard non incorporée, ceux de son fichier de métriques d'Adobe ; sinon le `/FontDescriptor` ;
/// sinon 0,8 et 0,2. Un programme incorporé qui ne se
/// lit pas laisse la place au descripteur, qui dit les mêmes mesures (`Ascent`, `Descent`, table 122) ; sans lui, refus.
fn ascendant_descendant(doc: &Document, fd: Option<&Dictionary>, base_font: Option<&str>) -> Result<(Mesure, Mesure), Refus> {
    let mut programme = None;
    let mut programme_illisible = None;
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
                match ttf_parser::Face::parse(&octets, 0) {
                    Ok(face) => {
                        let em = face.units_per_em() as i64;
                        let (a, d) = (Mesure::entier(face.ascender() as i64) / em, Mesure::entier(-(face.descender() as i64)) / em);
                        if a > Z && a <= Mesure::entier(2) && (Z..=Mesure::UN).contains(&d) {
                            return Ok((a, d));
                        }
                    }
                    Err(e) => programme_illisible = Some(format!("le programme incorporé d'une police ({}) ne se lit pas ({}), et son descripteur ne dit pas ses mesures", lecture::nom_de(cle), e)),
                }
            }
        }
        None => {
            // Une police standard non incorporée (9.6.2.2), que son nom dit : les mesures de son fichier de métriques
            // d'Adobe (`afm.rs`), celles qu'un descripteur dirait (`Ascent`, `Descent`, table 122).
            if let Some(base) = base_font {
                if let Some((a, d)) = crate::afm::ascendant_descendant(base)? {
                    return Ok((a / 1000, d.abs() / 1000));
                }
            }
        }
    }
    let (ascent, descent) = match fd {
        Some(d) => (lecture::lue(doc, d, b"Ascent", lecture::nombre)?, lecture::lue(doc, d, b"Descent", lecture::nombre)?),
        None => (None, None),
    };
    match (ascent, descent, programme_illisible) {
        (Some(a), Some(d), _) if a > Z && a <= Mesure::entier(1500) && d.abs() <= Mesure::entier(1000) => Ok((a / 1000, d.abs() / 1000)),
        (_, _, Some(pourquoi)) => illisible(pourquoi),
        (_, _, None) => Ok((mesure("0.8"), mesure("0.2"))),
    }
}

impl Police {
    /// Les codes d'une chaîne : (code, nombre d'octets).
    fn codes(&self, s: &[u8]) -> Result<Vec<(u32, usize)>, Refus> {
        let espaces = match &self.sorte {
            Sorte::Simple { .. } => return Ok(s.iter().map(|b| (*b as u32, 1)).collect()),
            Sorte::Composee { espaces, .. } => espaces,
        };
        let Some(plus_court) = espaces.iter().map(|e| e.0).min() else {
            return illisible("une police composée n'a aucun espace de codes");
        };
        let mut v = Vec::new();
        let mut i = 0;
        while i < s.len() {
            let mut pris = None;
            for n in 1..=4usize.min(s.len() - i) {
                let code = entier(&s[i..i + n]);
                if espaces.iter().any(|(l, a, b)| *l == n && code >= *a && code <= *b) {
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
        let cmap_cid = match &self.sorte {
            Sorte::Simple { .. } => return code,
            Sorte::Composee { cmap_cid, .. } => cmap_cid,
        };
        match cmap_cid {
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

    /// Les signes d'un code (ISO 32000-1, 9.10.2) : `ToUnicode` d'abord ; puis l'encodage d'une police simple, ou le
    /// programme d'une police composée. Aucun quand le code ne montre aucun glyphe ; un glyphe dont rien ne dit le
    /// caractère est un signe inconnu, qui nomme la police et le code.
    fn signes(&self, code: u32) -> Vec<Signe> {
        if let Some(t) = self.unicode.get(&code) {
            return t.chars().map(Signe::Caractere).collect();
        }
        let lu = match &self.sorte {
            Sorte::Simple { encodage } => encodage[code as usize & 0xFF].clone(),
            Sorte::Composee { programme: ParProgramme::Aucun(pourquoi), .. } => Lu::Inconnu(pourquoi.clone()),
            Sorte::Composee { programme: ParProgramme::Glyphes { cid_gid, lus }, .. } => {
                let cid = self.cid(code);
                match glyphe_du_cid(cid_gid.as_deref(), cid) {
                    Some(g) => match lus.get(&g) {
                        Some(l) => l.clone(),
                        None => Lu::Inconnu(format!("son CID {} mène au glyphe {}, que le programme n'a pas", cid, g)),
                    },
                    None => Lu::Inconnu(format!("son CID {} dépasse /CIDToGIDMap", cid)),
                }
            }
        };
        match lu {
            Lu::Texte(t) => t.chars().map(Signe::Caractere).collect(),
            Lu::Rien => Vec::new(),
            Lu::Inconnu(pourquoi) => vec![Signe::Inconnu(Inconnu { police: self.nom.clone(), code, pourquoi })],
        }
    }

    /// La largeur du code, en fraction d'em.
    fn largeur(&self, code: u32) -> Mesure {
        let cle = self.cid(code);
        if let Some(w) = self.largeurs.get(&cle) {
            return *w / 1000;
        }
        if let (Some(m), Sorte::Simple { encodage }) = (&self.standard, &self.sorte) {
            if let Lu::Texte(t) = &encodage[code as usize & 0xFF] {
                let mut c = t.chars();
                if let (Some(c), None) = (c.next(), c.next()) {
                    if let Some(w) = m.largeur(c) {
                        return w;
                    }
                }
            }
        }
        self.largeur_defaut / 1000
    }
}

/// Le glyphe d'un CID par `CIDToGIDMap` (table 117 : deux octets par CID, poids fort d'abord ; `None` : Identity) ;
/// `None` quand le CID dépasse la table.
fn glyphe_du_cid(table: Option<&[u8]>, cid: u32) -> Option<u16> {
    match table {
        None => (cid <= u16::MAX as u32).then_some(cid as u16),
        Some(t) => {
            let i = 2 * cid as usize;
            t.get(i..i + 2).map(|p| u16::from_be_bytes([p[0], p[1]]))
        }
    }
}

/// Un encodage prédéfini (ISO 32000-1, 9.6.6.1 et annexe D) : le texte de chaque code. Un autre nom se refuse.
fn predefini(nom: &[u8]) -> Result<Vec<Lu>, Refus> {
    let e = match nom {
        b"StandardEncoding" => Encodage::Standard,
        b"MacRomanEncoding" => Encodage::MacRoman,
        b"MacExpertEncoding" => Encodage::MacExpert,
        b"WinAnsiEncoding" => Encodage::WinAnsi,
        autre => return illisible(format!("l'encodage {} n'est pas un encodage prédéfini (ISO 32000-1, 9.6.6.1)", lecture::nom_de(autre))),
    };
    de_l_annexe(e)
}

/// Le texte de chaque code d'un encodage de l'annexe D (`encodages.rs`) : le nom de son glyphe lu par la liste des glyphes
/// d'Adobe (la liste Zapf Dingbats pour ZapfDingbats) ; un code non encodé ne montre rien (`.notdef`).
fn de_l_annexe(e: Encodage) -> Result<Vec<Lu>, Refus> {
    encodages::codes(e)?
        .into_iter()
        .map(|c| match c {
            Some(Code::Nom(n)) => lu_du_nom(n, e == Encodage::ZapfDingbats),
            Some(Code::Caractere(c)) => Ok(Lu::Texte(c.to_string())),
            None => Ok(Lu::Rien),
        })
        .collect()
}

/// Le texte d'un nom de glyphe (liste des glyphes d'Adobe, `glyphes.rs`) : `.notdef` ne montre rien (9.6.6.2) ; un nom
/// que la liste ne dit pas est inconnu.
fn lu_du_nom(nom: &str, zapf: bool) -> Result<Lu, Refus> {
    Ok(match glyphes::caracteres(nom, zapf)? {
        Some(t) => Lu::Texte(t),
        None if nom == ".notdef" => Lu::Rien,
        None => Lu::Inconnu(format!("son glyphe /{} n'a pas de nom que la liste des glyphes d'Adobe dise", nom)),
    })
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
    let base_nom = nom_de_base(doc, d)?;
    let nom = match &base_nom {
        Some(n) => n.clone(),
        None => format!("sans /BaseFont, de sous-type {}", lecture::nom_de(sous_type)),
    };
    // Le nom sans le préfixe d'un sous-ensemble (« ABCDEF+ », ISO 32000-1, 9.6.4).
    let nom_nu = match nom.split_once('+') {
        Some((_, apres)) => apres.to_string(),
        None => nom.clone(),
    };
    // La spécification de la liste des glyphes : la liste Zapf Dingbats vaut pour la police ZapfDingbats.
    let zapf = nom_nu == "ZapfDingbats";
    let mut p = Police {
        nom,
        sorte: Sorte::Simple { encodage: Vec::new() },
        unicode: HashMap::new(),
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
    if sous_type == b"Type0" {
        // ISO 32000-1, table 121 : `Encoding` (un CMap prédéfini nommé, ou un flux) et `DescendantFonts` (un tableau
        // d'un élément) sont exigés.
        let mut espaces = vec![(2, 0, 0xFFFF)];
        let mut cmap_cid = None;
        // Un CMap prédéfini autre qu'Identity (table 118) n'est pas déposé : ses CID ne se lisent pas, le programme ne
        // dit donc pas le caractère d'un code (seul `ToUnicode` le dit).
        let mut non_depose = None;
        match lecture::valeur(doc, d, b"Encoding")? {
            Some(Object::Name(n)) => {
                p.vertical = n.ends_with(b"-V");
                if n.as_slice() != b"Identity-H" && n.as_slice() != b"Identity-V" {
                    non_depose = Some(format!("le CMap prédéfini {} n'est pas déposé, rien ne dit le CID de ce code (ISO 32000-1, 9.10.2)", lecture::nom_de(n)));
                }
            }
            Some(Object::Stream(s)) => {
                let c = lire_cmap(&lecture::contenu(doc, s)?)?;
                if !c.espaces.is_empty() {
                    espaces = c.espaces.clone();
                }
                cmap_cid = Some(c);
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
        let programme = match (p.unicode.is_empty(), non_depose) {
            (_, Some(pourquoi)) => ParProgramme::Aucun(pourquoi),
            (true, None) => sinon_inconnu(programme_cid(doc, desc, zapf), ParProgramme::Aucun)?,
            (false, None) => ParProgramme::Aucun("/ToUnicode ne dit pas ce code".to_string()),
        };
        p.sorte = Sorte::Composee { espaces, cmap_cid, programme };
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
    let encodage = match sous_type {
        b"TrueType" => encodage_truetype(doc, fd, encoding, zapf)?,
        _ => encodage_type1(doc, fd, sous_type, &nom_nu, encoding, zapf)?,
    };
    p.sorte = Sorte::Simple { encodage };

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

/// Les `/Differences` d'un dictionnaire d'encodage : (code, texte du nom) (ISO 32000-1, 9.6.6.1 : un code, puis les
/// noms des codes qui le suivent ; 9.10.2 : le nom donne le caractère par la liste des glyphes d'Adobe).
fn differences_de(doc: &Document, e: &Dictionary, zapf: bool) -> Result<Vec<(usize, Lu)>, Refus> {
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
                    differences.push((code, lu_du_nom(&String::from_utf8_lossy(g), zapf)?));
                    code += 1;
                }
                Some(_) | None => return illisible("un élément de /Differences n'est ni un code ni un nom de glyphe"),
            }
        }
    }
    Ok(differences)
}

/// `table` mise à jour par `differences` (un code au-delà de 255 n'est pas d'une police simple).
fn appliquer(table: &mut [Lu], differences: Vec<(usize, Lu)>) {
    for (code, l) in differences {
        if code < 256 {
            table[code] = l;
        }
    }
}

/// Ce que le programme d'une police ne permet pas de lire ne se refuse que lorsqu'un code qui en dépend se montre : le
/// refus d'une pièce illisible devient la raison d'un glyphe inconnu (`inconnu`) ; un défaut du logiciel remonte.
fn sinon_inconnu<T>(r: Result<T, Refus>, inconnu: impl FnOnce(String) -> T) -> Result<T, Refus> {
    match r {
        Ok(v) => Ok(v),
        Err(Refus { classe: Classe::PieceIllisible, phrase }) => Ok(inconnu(phrase)),
        Err(autre) => Err(autre),
    }
}

/// 256 codes inconnus, pour la même raison.
fn tous_inconnus(pourquoi: String) -> Vec<Lu> {
    vec![Lu::Inconnu(pourquoi); 256]
}

/// Le bit `n` (à partir de 1) des drapeaux d'un descripteur (ISO 32000-1, 9.8.2).
fn drapeau(drapeaux: i64, n: u32) -> bool {
    drapeaux & (1 << (n - 1)) != 0
}

/// L'encodage d'une police TrueType simple. Le texte suit 9.10.2 (ISO 32000-1) : un encodage nommé (MacRoman,
/// MacExpert, WinAnsi) ou les noms des `Differences` donnent le caractère, quels que soient les drapeaux. Pour le reste,
/// 9.6.6.4 : un dictionnaire d'une police non symbolique part de son `BaseEncoding` (sinon d'une table vide), et « any
/// undefined entries in the table shall be filled using StandardEncoding » ; sans `Encoding`, ou ce que le dictionnaire
/// d'une police symbolique ne dit pas, le code mène au glyphe par la « cmap » du programme (`par_cmap`), et le glyphe
/// dit son caractère. Les drapeaux ne servent qu'à savoir si la police est non symbolique (table 123 : Nonsymbolic
/// levé, Symbolic baissé) ; absents ou contradictoires, elle ne l'est pas.
fn encodage_truetype(doc: &Document, fd: Option<&Dictionary>, encoding: Option<&Object>, zapf: bool) -> Result<Vec<Lu>, Refus> {
    let drapeaux = match fd {
        Some(fd) => lecture::lue(doc, fd, b"Flags", lecture::entier)?,
        None => None,
    };
    let non_symbolique = drapeaux.is_some_and(|f| drapeau(f, 6) && !drapeau(f, 3));
    match encoding {
        Some(Object::Name(n)) => predefini(n),
        Some(Object::Dictionary(e)) => {
            let mut table = match (lecture::lue(doc, e, b"BaseEncoding", lecture::nom)?, non_symbolique) {
                (Some(n), _) => predefini(n)?,
                (None, true) => vec![Lu::Rien; 256],
                (None, false) => par_cmap(doc, fd, zapf)?,
            };
            appliquer(&mut table, differences_de(doc, e, zapf)?);
            if non_symbolique {
                for (l, s) in table.iter_mut().zip(predefini(b"StandardEncoding")?) {
                    if matches!(l, Lu::Rien) {
                        *l = s;
                    }
                }
            }
            Ok(table)
        }
        Some(_) => illisible("/Encoding d'une police simple n'est ni un nom ni un dictionnaire"),
        None => par_cmap(doc, fd, zapf),
    }
}

/// Le texte de chaque code d'un octet lu par la « cmap » du programme TrueType incorporé (ISO 32000-1, 9.6.6.4) : la
/// sous-table (3, 0), dont les codes sont dans 0x0000-0x00FF, 0xF000-0xF0FF, 0xF100-0xF1FF ou 0xF200-0xF2FF, chaque
/// octet précédé de l'octet de tête de sa plage ; sinon la sous-table (1, 0), octet par octet. Le glyphe 0 ne montre
/// rien ; un autre dit son caractère (`glyphes_du_programme`). Ce que le programme ne permet pas de lire rend chaque code
/// inconnu.
fn par_cmap(doc: &Document, fd: Option<&Dictionary>, zapf: bool) -> Result<Vec<Lu>, Refus> {
    sinon_inconnu(lire_par_cmap(doc, fd, zapf), tous_inconnus)
}

fn lire_par_cmap(doc: &Document, fd: Option<&Dictionary>, zapf: bool) -> Result<Vec<Lu>, Refus> {
    let programme = match fd {
        Some(fd) => lecture::lue(doc, fd, b"FontFile2", lecture::flux)?,
        None => None,
    };
    let Some(programme) = programme else {
        return illisible("une police TrueType sans encodage lisible n'incorpore pas son programme, dont la « cmap » mène du code au glyphe (ISO 32000-1, 9.6.6.4)");
    };
    let octets = lecture::contenu(doc, programme)?;
    let face = ttf_parser::Face::parse(&octets, 0).map_err(|e| Refus::illisible(format!("le programme incorporé d'une police TrueType ne se lit pas ({})", e)))?;
    let Some(cmap) = face.tables().cmap else {
        return illisible("le programme incorporé d'une police TrueType n'a pas de table « cmap » (ISO 32000-1, 9.6.6.4)");
    };
    let sous_table = |plateforme: ttf_parser::PlatformId, codage: u16| cmap.subtables.into_iter().find(|s| s.platform_id == plateforme && s.encoding_id == codage);
    let glyphe = |s: &ttf_parser::cmap::Subtable, code: u32| s.glyph_index(code).map(|g| g.0).filter(|g| *g != 0);
    let glyphes: Vec<Option<u16>> = match (sous_table(ttf_parser::PlatformId::Windows, 0), sous_table(ttf_parser::PlatformId::Macintosh, 0)) {
        (Some(s), _) => {
            let plages: Vec<u32> = [0x00u32, 0xF0, 0xF1, 0xF2].into_iter().filter(|h| (0..=255u32).any(|c| glyphe(&s, h << 8 | c).is_some())).collect();
            match plages.as_slice() {
                [h] => (0..=255u32).map(|c| glyphe(&s, h << 8 | c)).collect(),
                [] => vec![None; 256],
                _ => return illisible("la sous-table (3, 0) d'une police TrueType couvre plusieurs plages de codes, la norme n'en lit qu'une (ISO 32000-1, 9.6.6.4)"),
            }
        }
        (None, Some(s)) => (0..=255u32).map(|c| glyphe(&s, c)).collect(),
        (None, None) => return illisible("le programme incorporé d'une police TrueType n'a ni sous-table « cmap » (3, 0) ni (1, 0) (ISO 32000-1, 9.6.6.4)"),
    };
    let unicode = unicode_du_programme(&face);
    glyphes
        .into_iter()
        .map(|g| match g {
            Some(g) => glyphe_du_programme(&face, &unicode, g, zapf),
            None => Ok(Lu::Rien),
        })
        .collect()
}

/// Les caractères que la « cmap » Unicode d'un programme ((0, x), (3, 1), (3, 10)) donne à chaque glyphe.
fn unicode_du_programme(face: &ttf_parser::Face) -> HashMap<u16, Vec<char>> {
    let mut par_glyphe: HashMap<u16, Vec<char>> = HashMap::new();
    if let Some(cmap) = face.tables().cmap {
        for s in cmap.subtables.into_iter().filter(|s| s.is_unicode()) {
            s.codepoints(|cp| {
                if let (Some(g), Some(c)) = (s.glyph_index(cp), char::from_u32(cp)) {
                    let v = par_glyphe.entry(g.0).or_default();
                    if !v.contains(&c) {
                        v.push(c);
                    }
                }
            });
        }
    }
    par_glyphe
}

/// Le texte d'un glyphe d'un programme incorporé : son nom (table « post », ou le jeu de caractères d'un programme
/// compact) lu par la liste des glyphes d'Adobe ; sinon le seul caractère que la « cmap » Unicode du programme lui
/// donne. Un glyphe sans nom lisible, sans caractère ou avec plusieurs est inconnu.
fn glyphe_du_programme(face: &ttf_parser::Face, unicode: &HashMap<u16, Vec<char>>, g: u16, zapf: bool) -> Result<Lu, Refus> {
    if let Some(nom) = face.glyph_name(ttf_parser::GlyphId(g)) {
        if let l @ (Lu::Texte(_) | Lu::Rien) = lu_du_nom(nom, zapf)? {
            return Ok(l);
        }
    }
    Ok(match unicode.get(&g).map(Vec::as_slice) {
        Some([c]) => Lu::Texte(c.to_string()),
        Some(plusieurs) if !plusieurs.is_empty() => Lu::Inconnu(format!(
            "son glyphe {} vaut {} caractères dans le programme ({})",
            g,
            plusieurs.len(),
            plusieurs.iter().map(|c| format!("U+{:04X}", *c as u32)).collect::<Vec<_>>().join(", ")
        )),
        Some(_) | None => Lu::Inconnu(format!("son glyphe {} n'a ni nom de la liste des glyphes d'Adobe ni caractère dans le programme", g)),
    })
}

/// Ce que le programme d'une police CID dit de ses glyphes : un programme TrueType (`CIDFontType2`, `FontFile2`), son
/// `CIDToGIDMap` (table 117 : un flux, ou Identity, la valeur par défaut). Une police CID Type 0 (programme compact)
/// ne nomme pas ses glyphes.
fn programme_cid(doc: &Document, desc: &Dictionary, zapf: bool) -> Result<ParProgramme, Refus> {
    let sous_type = exige(doc, desc, b"Subtype", "la police de /DescendantFonts", lecture::nom)?;
    if sous_type != b"CIDFontType2" {
        return illisible(format!("une police CID de sous-type {} n'a pas de noms de glyphes que la fabrique lise", lecture::nom_de(sous_type)));
    }
    let programme = match descripteur(doc, desc)? {
        Some(fd) => lecture::lue(doc, fd, b"FontFile2", lecture::flux)?,
        None => None,
    };
    let Some(programme) = programme else {
        return illisible("une police CID TrueType n'incorpore pas son programme");
    };
    let cid_gid = match lecture::valeur(doc, desc, b"CIDToGIDMap")? {
        Some(Object::Name(n)) if n.as_slice() == b"Identity" => None,
        Some(Object::Stream(s)) => Some(lecture::contenu(doc, s)?),
        Some(_) => return illisible("/CIDToGIDMap n'est ni un flux ni /Identity (ISO 32000-1, table 117)"),
        None => None,
    };
    let octets = lecture::contenu(doc, programme)?;
    let face = ttf_parser::Face::parse(&octets, 0).map_err(|e| Refus::illisible(format!("le programme incorporé d'une police CID TrueType ne se lit pas ({})", e)))?;
    let unicode = unicode_du_programme(&face);
    let mut lus = HashMap::new();
    lus.insert(0, Lu::Rien);
    for g in 1..face.number_of_glyphs() {
        lus.insert(g, glyphe_du_programme(&face, &unicode, g, zapf)?);
    }
    Ok(ParProgramme::Glyphes { cid_gid, lus })
}

/// L'encodage d'une police simple qui n'est pas TrueType (Type 1, MMType1, Type 3) : l'encodage nommé ; ou le
/// dictionnaire, son `BaseEncoding` ou l'encodage implicite, puis ses `Differences` ; sans `Encoding`, l'encodage
/// implicite. Table 114 : l'encodage implicite d'un programme incorporé est son encodage propre ; sinon StandardEncoding
/// pour une police non symbolique, et l'encodage propre de la police pour une symbolique.
fn encodage_type1(doc: &Document, fd: Option<&Dictionary>, sous_type: &[u8], nom_nu: &str, encoding: Option<&Object>, zapf: bool) -> Result<Vec<Lu>, Refus> {
    let implicite = || -> Result<Vec<Lu>, Refus> {
        // 9.6.6.3 : l'encodage d'une police Type 3 est tout entier dans son `Encoding`, exigé.
        if sous_type == b"Type3" {
            return Ok(match encoding {
                Some(_) => vec![Lu::Rien; 256],
                None => tous_inconnus("une police Type 3 n'a pas d'/Encoding, que la norme exige (ISO 32000-1, 9.6.6.3)".to_string()),
            });
        }
        let (programme, compact) = match fd {
            Some(fd) => (lecture::lue(doc, fd, b"FontFile", lecture::flux)?, lecture::lue(doc, fd, b"FontFile3", lecture::flux)?),
            None => (None, None),
        };
        if let Some(s) = programme {
            return sinon_inconnu(encodage_du_programme_type1(doc, s, zapf), tous_inconnus);
        }
        if let Some(s) = compact {
            // Table 126 : `FontFile3` de sous-type `Type1C` (un programme CFF nu) ou `OpenType`.
            return match lecture::lue(doc, &s.dict, b"Subtype", lecture::nom)? {
                Some(b"Type1C") => sinon_inconnu(encodage_cff(doc, s, zapf), tous_inconnus),
                autre => Ok(tous_inconnus(format!(
                    "l'encodage propre d'un programme incorporé /FontFile3 de sous-type {} n'est pas lu par la fabrique",
                    autre.map_or("absent".to_string(), lecture::nom_de)
                ))),
            };
        }
        // 9.6.6.1 : les deux polices standard symboliques sont Symbol et ZapfDingbats ; table 123 : le drapeau Symbolic.
        let drapeaux = match fd {
            Some(fd) => lecture::lue(doc, fd, b"Flags", lecture::entier)?,
            None => None,
        };
        let symbolique = match drapeaux {
            Some(f) => drapeau(f, 3),
            None => nom_nu == "Symbol" || nom_nu == "ZapfDingbats",
        };
        // Annexe D.5 et D.6 : les encodages propres des deux polices standard symboliques.
        match (symbolique, nom_nu) {
            (false, _) => predefini(b"StandardEncoding"),
            (true, "Symbol") => de_l_annexe(Encodage::Symbol),
            (true, "ZapfDingbats") => de_l_annexe(Encodage::ZapfDingbats),
            (true, _) => Ok(tous_inconnus(format!(
                "la police symbolique {} n'incorpore pas son programme, seul à dire son encodage propre (ISO 32000-1, table 114)",
                nom_nu
            ))),
        }
    };
    match encoding {
        Some(Object::Name(n)) => predefini(n),
        Some(Object::Dictionary(e)) => {
            let mut table = match lecture::lue(doc, e, b"BaseEncoding", lecture::nom)? {
                Some(n) => predefini(n)?,
                None => implicite()?,
            };
            appliquer(&mut table, differences_de(doc, e, zapf)?);
            Ok(table)
        }
        Some(_) => illisible("/Encoding d'une police simple n'est ni un nom ni un dictionnaire"),
        None => implicite(),
    }
}

/// L'encodage propre d'un programme Type 1 incorporé (`FontFile`), lu dans sa partie en clair (`Length1`, exigé, table
/// 127) : « The tokens following /Encoding may be StandardEncoding def », ou la suite des « dup index charactername
/// put », à partir du premier `dup` après `/Encoding`, « followed by an instance of the token def or readonly » (Adobe
/// Type 1 Font Format, 10.3). Un code qu'elle ne nomme pas est `.notdef`.
fn encodage_du_programme_type1(doc: &Document, s: &lopdf::Stream, zapf: bool) -> Result<Vec<Lu>, Refus> {
    let Some(longueur) = lecture::lue(doc, &s.dict, b"Length1", lecture::entier)? else {
        return illisible("le programme Type 1 incorporé n'a pas de /Length1, que la norme exige (ISO 32000-1, table 127)");
    };
    let octets = lecture::contenu(doc, s)?;
    let clair = match usize::try_from(longueur) {
        Ok(l) => octets.get(..l),
        Err(e) => return illisible(format!("/Length1 vaut {} ({})", longueur, e)),
    };
    let Some(clair) = clair else {
        return illisible(format!("/Length1 ({}) ne mesure pas la partie en clair du programme Type 1 ({} octets)", longueur, octets.len()));
    };
    let mots = mots_postscript(clair);
    let Some(debut) = mots.iter().position(|m| m == "/Encoding") else {
        return illisible("la partie en clair du programme Type 1 n'a pas d'/Encoding (Adobe Type 1 Font Format, 10.3)");
    };
    let suite = &mots[debut + 1..];
    if suite.first().map(String::as_str) == Some("StandardEncoding") {
        return predefini(b"StandardEncoding");
    }
    let Some(premier) = suite.iter().position(|m| m == "dup") else {
        return illisible("l'/Encoding du programme Type 1 n'a ni StandardEncoding ni « dup index charactername put » (Adobe Type 1 Font Format, 10.3)");
    };
    let mut table = vec![Lu::Rien; 256];
    let mut i = premier;
    loop {
        match &suite[i..] {
            [dup, index, nom, put, ..] if dup == "dup" && put == "put" => {
                let code = match index.parse::<usize>() {
                    Ok(c) => c,
                    Err(e) => return illisible(format!("l'/Encoding du programme Type 1 a « dup {} {} put » ({} ; Adobe Type 1 Font Format, 10.3)", index, nom, e)),
                };
                let Some(nom) = nom.strip_prefix('/') else {
                    return illisible(format!("l'/Encoding du programme Type 1 a « dup {} {} put », sans nom (Adobe Type 1 Font Format, 10.3)", index, nom));
                };
                if code < 256 {
                    table[code] = lu_du_nom(nom, zapf)?;
                }
                i += 4;
            }
            [fin, ..] if fin == "def" || fin == "readonly" => return Ok(table),
            reste => {
                return illisible(format!(
                    "l'/Encoding du programme Type 1 a {:?} dans sa suite de « dup index charactername put » (Adobe Type 1 Font Format, 10.3)",
                    reste.iter().take(4).collect::<Vec<_>>()
                ))
            }
        }
    }
}

/// L'encodage propre d'un programme compact incorporé (`FontFile3`, `Type1C`) : le nom du glyphe de chaque code, lu
/// dans le programme (`cff.rs`), par la liste des glyphes d'Adobe ; un code non encodé ne montre rien.
fn encodage_cff(doc: &Document, s: &lopdf::Stream, zapf: bool) -> Result<Vec<Lu>, Refus> {
    let octets = lecture::contenu(doc, s)?;
    crate::cff::noms_des_codes(&octets)?
        .into_iter()
        .map(|n| match n {
            Some(n) => lu_du_nom(&n, zapf),
            None => Ok(Lu::Rien),
        })
        .collect()
}

/// Les mots d'un texte PostScript : les noms avec leur `/`, les chaînes et les commentaires sautés, les délimiteurs
/// seuls.
fn mots_postscript(o: &[u8]) -> Vec<String> {
    let delimiteur = |c: u8| matches!(c, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%');
    let blanc = |c: u8| matches!(c, 0 | b'\t' | b'\n' | 0x0C | b'\r' | b' ');
    let mut mots = Vec::new();
    let mut i = 0;
    while i < o.len() {
        let c = o[i];
        if blanc(c) {
            i += 1;
        } else if c == b'%' {
            while i < o.len() && o[i] != b'\n' && o[i] != b'\r' {
                i += 1;
            }
        } else if c == b'(' {
            let mut prof = 0;
            while i < o.len() {
                match o[i] {
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
        } else if delimiteur(c) && c != b'/' {
            mots.push((c as char).to_string());
            i += 1;
        } else {
            let debut = i;
            i += 1;
            while i < o.len() && !blanc(o[i]) && !delimiteur(o[i]) {
                i += 1;
            }
            mots.push(String::from_utf8_lossy(&o[debut..i]).into_owned());
        }
    }
    mots
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
    texte: Vec<Signe>,
    /// Une boîte par signe de `texte` (`None` : un espace ajouté), si `Mise::garder`.
    boites: Vec<Option<Boite>>,
}

#[derive(Default)]
struct Mise {
    lignes: Vec<Vec<Signe>>,
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

    /// Pose un signe : origine `p`, vecteur de direction `dir` (déjà à l'échelle du corps),
    /// avance `adv` (fraction d'em).
    fn caractere(&mut self, c: Signe, p:(Mesure, Mesure), dir: (Mesure, Mesure), taille: Mesure, adv: Option<Mesure>, boite: Boite) {
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
            self.courante = Some(LigneCourante { dir: nd, texte: Vec::new(), boites: Vec::new() });
        }
        let garder = self.garder;
        let l = self.courante.as_mut().unwrap();
        let blanc = Signe::Caractere(' ');
        if espace && l.texte.last() != Some(&blanc) && c != blanc {
            l.texte.push(blanc);
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
    fn finir_avec_boites(mut self) -> Vec<(Vec<Signe>, Vec<Option<Boite>>)> {
        self.fermer();
        self.lignes.into_iter().zip(self.boites).collect()
    }

    fn finir(mut self) -> Vec<Vec<Signe>> {
        self.fermer();
        self.lignes
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
    /// Le cadre de la page (x0, y0, x1, y1) : le texte tout entier hors de lui n'est pas lu. `None` : la page ne dit
    /// pas son support (`MediaBox`), rien n'est hors de lui.
    cadre: Option<[Mesure; 4]>,
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

/// Le texte d'une chaîne PDF : UTF-16BE s'il commence par la marque, sinon PDFDocEncoding (annexe D.3) ; un
/// code que la table dit « Undefined » se refuse.
fn texte_pdf(s: &[u8]) -> Result<String, Refus> {
    if s.starts_with(&[0xFE, 0xFF]) {
        return utf16(&s[2..]);
    }
    s.iter()
        .map(|b| match encodages::pdfdoc(*b)? {
            Some(c) => Ok(c),
            None => illisible(format!("une chaîne en PDFDocEncoding porte le code {:#04x}, que la norme laisse sans caractère (ISO 32000-1, annexe D.3)", b)),
        })
        .collect()
}

/// Le sous-dictionnaire `cle` des ressources (`Font`, `XObject`, `Properties`) : `None` quand il manque, ou que le
/// contenu n'a pas de ressources.
fn ressource<'a>(doc: &'a Document, res: Option<&'a Dictionary>, cle: &[u8]) -> Result<Option<&'a Dictionary>, Refus> {
    match res {
        Some(r) => lecture::lue(doc, r, cle, lecture::dictionnaire),
        None => Ok(None),
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
            self.mise.caractere(Signe::Caractere(c), (x + decalage * dir.0, y + decalage * dir.1), dir, taille, Some(a.adv / n), boite);
        }
    }

    /// La police que `Tf` nomme : « the name of a font resource in the Font subdictionary of the current resource
    /// dictionary » (ISO 32000-1, 9.3.1) ; une police que les ressources n'ont pas se refuse.
    fn police(&mut self, res: Option<&Dictionary>, nom_police: &[u8]) -> Result<Rc<Police>, Refus> {
        let doc = self.doc;
        let Some(polices) = ressource(doc, res, b"Font")? else {
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
    fn relever_polices(&mut self, res: Option<&Dictionary>) -> Result<(), Refus> {
        if self.annotation {
            return Ok(());
        }
        let doc = self.doc;
        let Some(r) = self.releve.as_mut() else {
            return Ok(());
        };
        let Some(polices) = ressource(doc, res, b"Font")? else {
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

    fn executer(&mut self, flux: &[u8], res: Option<&Dictionary>, ctm: Mat, profondeur: usize) -> Result<(), Refus> {
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
                    // `/Properties`. Elles ne servent qu'à son `/ActualText` : des propriétés que le PDF ne donne pas
                    // n'en ont pas.
                    let propres = match a.get(1) {
                        Some(Object::Name(n)) => match ressource(doc, res, b"Properties")? {
                            Some(p) => lecture::lue(doc, p, n, lecture::dictionnaire)?,
                            None => None,
                        },
                        Some(o) => lecture::designe(doc, o)?.map(|o| lecture::element(doc, o, "les propriétés de BDC", lecture::dictionnaire)).transpose()?,
                        None => None,
                    };
                    let reel = match propres.map(|p| lecture::lue(doc, p, b"ActualText", lecture::chaine)).transpose()?.flatten() {
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
    fn objet(&mut self, res: Option<&Dictionary>, nom_obj: &[u8], ctm: &Mat, profondeur: usize) -> Result<(), Refus> {
        let doc = self.doc;
        let quoi = format!("l'objet {}", lecture::nom_de(nom_obj));
        let Some(xo) = ressource(doc, res, b"XObject")? else {
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
                        self.relever_polices(Some(r))?;
                        Some(r)
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
            let signes = police.signes(code);
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
                for (k, c) in signes.into_iter().enumerate() {
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
        let Some(c) = &self.cadre else {
            return true;
        };
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
///
/// Ce qui ne sert pas au texte ne se lit pas, et ne refuse donc rien : une page sans `Resources` n'a aucune ressource
/// (un `Tf` qui en nomme une se refuse alors) ; sans `MediaBox`, `--texte` ne coupe rien (`--inspecter`, qui place
/// chaque mot dans la page, l'exige : `inspection::page_de`).
fn lire_la_page(doc: &Document, id: ObjectId, inspecter: bool) -> Result<Interprete<'_>, Refus> {
    let res = match lecture::herite(doc, id, b"Resources")? {
        Some(o) => Some(lecture::element(doc, o, "/Resources de la page", lecture::dictionnaire)?),
        None => None,
    };
    let cadre = match lecture::herite(doc, id, b"MediaBox")? {
        Some(_) => Some(lecture::cadre_de(doc, id)?),
        None => None,
    };
    let mise = Mise { garder: inspecter, ..Mise::default() };
    let releve = if inspecter { Some(Releve::default()) } else { None };
    let mut it = Interprete { doc, polices: HashMap::new(), mise, cadre, actuel: None, releve, annotation: false };
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
        // Une annotation nulle (7.3.10) n'est pas là.
        let Some(d) = lecture::designe(doc, a)? else {
            continue;
        };
        let d = lecture::element(doc, d, "une annotation de /Annots", lecture::dictionnaire)?;
        // ISO 32000-1, table 164 : `F`, « Default value: 0 ». Le sous-type ne sert qu'à écarter une Popup.
        let drapeaux = match lecture::lue(doc, d, b"F", lecture::entier)? {
            Some(f) => f,
            None => DRAPEAUX_PAR_DEFAUT,
        };
        if drapeaux & CACHEE != 0 || lecture::lue(doc, d, b"Subtype", lecture::nom)? == Some(b"Popup".as_slice()) {
            continue;
        }
        // Table 168 : l'apparence normale `N`, un flux ou un dictionnaire d'états, que `AS` choisit (table 164). Une
        // apparence sans `N`, des états sans `AS`, un état que `N` n'a pas : rien n'est choisi, rien ne se dessine.
        let Some(ap) = lecture::lue(doc, d, b"AP", lecture::dictionnaire)? else {
            continue;
        };
        let s = match lecture::valeur(doc, ap, b"N")? {
            Some(Object::Stream(s)) => s,
            Some(Object::Dictionary(etats)) => {
                let Some(etat) = lecture::lue(doc, d, b"AS", lecture::nom)? else {
                    continue;
                };
                match lecture::lue(doc, etats, etat, lecture::flux)? {
                    Some(s) => s,
                    None => continue,
                }
            }
            Some(_) => return illisible("/N de l'apparence d'une annotation n'est ni un flux ni un dictionnaire"),
            None => continue,
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
            Some(r) => Some(r),
            None => res,
        };
        let contenu = lecture::contenu(doc, s)?;
        it.executer(&contenu, res_ap, mul(&m, &a_m), 1)?;
    }
    Ok(it)
}

/// Le texte d'une page : son contenu, puis l'apparence de ses annotations (champs de formulaire).
fn texte_de_page(doc: &Document, id: ObjectId) -> Result<Vec<Vec<Signe>>, Refus> {
    Ok(lire_la_page(doc, id, false)?.mise.finir())
}

/// Ce que `--inspecter` relève d'une page, dans le repère du PDF (origine en bas à gauche).
pub(crate) struct PageLue {
    /// Les lignes de texte (celles de `--texte`) avec la boîte de chacun de leurs signes.
    pub lignes: Vec<(Vec<Signe>, Vec<Option<Boite>>)>,
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
pub fn texte_des_pages(doc: &Document) -> (Vec<TexteDePage>, Vec<String>) {
    let mut textes = Vec::new();
    let mut avertissements = Vec::new();
    for (n, id) in doc.get_pages() {
        match texte_de_page(doc, id) {
            Ok(lignes) => textes.push(TexteDePage::de_lignes(n, lignes)),
            Err(r) => {
                avertissements.push(format!("page {} : {}", n, r.phrase));
                textes.push(TexteDePage { page: n, morceaux: Vec::new() });
            }
        }
    }
    (textes, avertissements)
}
