//! `fabrique-pdf --inspecter a.pdf` : ce qu'un test veut savoir d'un PDF (la page, les mots
//! cherchés, les dessins, les images, les polices, les métadonnées, les permissions),
//! en une ligne de JSON. Le relevé est fait par l'interpréteur de `texte.rs` (le même que pour
//! `--texte` : mêmes lignes, mêmes mots), ici on le met en forme dans le repère de PyMuPDF
//! (origine en haut à gauche de la page visible, y vers le bas). Voir FORMAT.md, « Inspecter un PDF ».

use lopdf::{Document, Object};
use serde::Serialize;

use crate::description::{illisible, Refus};
use crate::lecture;
use crate::mesure::Mesure;
use crate::texte::{inspecter_page, Boite};

#[derive(Serialize)]
struct Chiffrement {
    filtre: String,
    #[serde(rename = "V")]
    v: i64,
    #[serde(rename = "R")]
    r: i64,
    longueur: i64,
    #[serde(rename = "P")]
    p: i64,
    methode: String,
}

#[derive(Serialize)]
struct Mot {
    texte: String,
    rect: Boite,
    ligne: usize,
}

#[derive(Serialize)]
struct Page {
    largeur: Mesure,
    hauteur: Mesure,
    rotation: i64,
    polices: Vec<String>,
    images: Vec<Boite>,
    traits: Vec<Boite>,
    mots: Vec<Mot>,
}

#[derive(Serialize)]
struct Inspection {
    chiffrement: Option<Chiffrement>,
    metadonnees: std::collections::BTreeMap<String, String>,
    pages: Vec<Page>,
}

/// Arrondi à 2 décimales.
fn arrondi(x: Mesure) -> Mesure {
    x.arrondie(2)
}

/// Le passage du repère du PDF (origine en bas à gauche) à celui de PyMuPDF : origine en haut à
/// gauche de la page visible, y vers le bas. Comme PyMuPDF, les **boîtes** (mots, images, traits)
/// restent dans le repère de la page **non tournée**, alors que `largeur` et `hauteur` (`page.rect`)
/// sont ceux de la page tournée de `/Rotate`.
struct Repere {
    cadre: [Mesure; 4],
    rotation: i64,
}

impl Repere {
    fn boite(&self, b: &Boite) -> Boite {
        let [x0, _, _, y1] = self.cadre;
        [arrondi(b[0].min(b[2]) - x0), arrondi(y1 - b[1].max(b[3])), arrondi(b[0].max(b[2]) - x0), arrondi(y1 - b[1].min(b[3]))]
    }

    fn dimensions(&self) -> (Mesure, Mesure) {
        let (l, h) = (self.cadre[2] - self.cadre[0], self.cadre[3] - self.cadre[1]);
        if self.rotation % 180 == 90 {
            (h, l)
        } else {
            (l, h)
        }
    }
}

/// Les mots d'une ligne : les suites de caractères séparées par des espaces, avec leur boîte.
fn mots_de(ligne: usize, texte: &str, boites: &[Option<Boite>], repere: &Repere, sortie: &mut Vec<Mot>) {
    let mut mot = String::new();
    let mut boite: Option<Boite> = None;
    let mut finir = |mot: &mut String, boite: &mut Option<Boite>| {
        if let (false, Some(b)) = (mot.is_empty(), *boite) {
            sortie.push(Mot { texte: std::mem::take(mot), rect: repere.boite(&b), ligne });
        }
        mot.clear();
        *boite = None;
    };
    for (c, b) in texte.chars().zip(boites.iter()) {
        if c.is_whitespace() {
            finir(&mut mot, &mut boite);
            continue;
        }
        mot.push(c);
        if let Some(b) = b {
            boite = Some(match boite {
                Some(u) => [u[0].min(b[0]), u[1].min(b[1]), u[2].max(b[2]), u[3].max(b[3])],
                None => *b,
            });
        }
    }
    finir(&mut mot, &mut boite);
}

/// La table PDFDocEncoding au-delà de Latin-1 (ISO 32000-1, annexe D.2).
fn pdfdoc(b: u8) -> char {
    const BAS: [char; 8] = ['\u{2D8}', '\u{2C7}', '\u{2C6}', '\u{2D9}', '\u{2DD}', '\u{2DB}', '\u{2DA}', '\u{2DC}'];
    const HAUT: [char; 33] = [
        '\u{2022}', '\u{2020}', '\u{2021}', '\u{2026}', '\u{2014}', '\u{2013}', '\u{192}', '\u{2044}', '\u{2039}', '\u{203A}',
        '\u{2212}', '\u{2030}', '\u{201E}', '\u{201C}', '\u{201D}', '\u{2018}', '\u{2019}', '\u{201A}', '\u{2122}', '\u{FB01}',
        '\u{FB02}', '\u{141}', '\u{152}', '\u{160}', '\u{178}', '\u{17D}', '\u{131}', '\u{142}', '\u{153}', '\u{161}', '\u{17E}',
        '\u{FFFD}', '\u{20AC}',
    ];
    match b {
        0x18..=0x1F => BAS[(b - 0x18) as usize],
        0x80..=0xA0 => HAUT[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// Une chaîne de texte PDF : UTF-16BE ou UTF-16LE avec marque, UTF-8 avec marque, sinon PDFDocEncoding.
fn chaine_pdf(s: &[u8]) -> Result<String, Refus> {
    let utf16 = |o: &[u8], be: bool| -> Result<String, Refus> {
        if o.len() % 2 == 1 {
            return illisible(format!("une chaîne en UTF-16 a un nombre impair d'octets ({})", o.len()));
        }
        let u: Vec<u16> = o
            .chunks(2)
            .map(|p| {
                let (a, b) = (p[0] as u16, p[1] as u16);
                if be { (a << 8) | b } else { (b << 8) | a }
            })
            .collect();
        Ok(String::from_utf16_lossy(&u))
    };
    let utf8 = s.utf8_chunks().all(|morceau| morceau.invalid().is_empty());
    if s.starts_with(&[0xFE, 0xFF]) {
        utf16(&s[2..], true)
    } else if s.starts_with(&[0xFF, 0xFE]) {
        utf16(&s[2..], false)
    } else if s.starts_with(&[0xEF, 0xBB, 0xBF]) {
        Ok(String::from_utf8_lossy(&s[3..]).into_owned())
    } else if s.iter().any(|b| *b >= 0x80) && utf8 {
        // Des octets qui forment de l'UTF-8 valide (sans marque) : MuPDF
        // les lit comme tels, et un texte en PDFDocEncoding n'a presque jamais cette forme.
        Ok(String::from_utf8_lossy(s).into_owned())
    } else {
        Ok(s.iter().map(|b| pdfdoc(*b)).collect())
    }
}

/// Un nom lu, en texte.
fn nom_texte(n: &[u8]) -> String {
    String::from_utf8_lossy(n).into_owned()
}

/// Le dictionnaire `/Encrypt` du fichier (à lire AVANT de déchiffrer : le déchiffrement l'efface). ISO 32000-1,
/// tables 20, 21, 25 et 26 : `Filter`, `R` et `P` sont exigés ; `V` absent vaut 0 ; `Length` (pour `V` 2 ou 3) absent
/// vaut 40 ; `StmF` absent vaut `Identity` ; `CFM` absent vaut `None`.
fn chiffrement(doc: &Document) -> Result<Option<Chiffrement>, Refus> {
    let Some(e) = lecture::lue(doc, &doc.trailer, b"Encrypt", lecture::dictionnaire)? else {
        return Ok(None);
    };
    let exige = |cle: &[u8]| -> Result<i64, Refus> {
        match lecture::lue(doc, e, cle, lecture::entier)? {
        Some(v) => Ok(v),
        None => illisible(format!("le chiffrement n'a pas de {}, que la norme exige", lecture::nom_de(cle))),
        }
    };
    let v = match lecture::lue(doc, e, b"V", lecture::entier)? {
        Some(v) => v,
        None => V_PAR_DEFAUT,
    };
    let r = exige(b"R")?;
    let longueur = match v {
        5 => 256,
        4 => 128,
        _ => match lecture::lue(doc, e, b"Length", lecture::entier)? {
            Some(l) => l,
            None => LONGUEUR_PAR_DEFAUT,
        },
    };
    let mut p = exige(b"P")?;
    if p > i32::MAX as i64 {
        p -= 1 << 32;
    }
    let methode = if v >= 4 {
        match lecture::lue(doc, e, b"StmF", lecture::nom)? {
            None | Some(b"Identity") => IDENTITE.to_string(),
            Some(filtre) => {
                let Some(cf) = lecture::lue(doc, e, b"CF", lecture::dictionnaire)? else {
                    return illisible(format!("/StmF nomme /{}, et /CF manque", nom_texte(filtre)));
                };
                let Some(f) = lecture::lue(doc, cf, filtre, lecture::dictionnaire)? else {
                    return illisible(format!("/StmF nomme /{}, que /CF n'a pas", nom_texte(filtre)));
                };
                match lecture::lue(doc, f, b"CFM", lecture::nom)? {
                    Some(m) => nom_texte(m),
                    None => CFM_PAR_DEFAUT.to_string(),
                }
            }
        }
    } else {
        "RC4".to_string()
    };
    let filtre = match lecture::lue(doc, e, b"Filter", lecture::nom)? {
        Some(f) => nom_texte(f),
        None => return illisible("le chiffrement n'a pas de /Filter, que la norme exige"),
    };
    Ok(Some(Chiffrement { filtre, v, r, longueur, p, methode }))
}

/// ISO 32000-1, table 20 : `V`, « The default value if this entry is omitted shall be 0 ».
const V_PAR_DEFAUT: i64 = 0;
/// Table 20 : `Length`, « Default value: 40 ».
const LONGUEUR_PAR_DEFAUT: i64 = 40;
/// Tables 20 et 26 : `StmF`, « Default value: Identity ».
const IDENTITE: &str = "Identity";
/// Table 25 : `CFM`, « Default value: None ».
const CFM_PAR_DEFAUT: &str = "None";

/// Le dictionnaire `/Info`, chaînes décodées (`{}` s'il n'y en a pas). Une entrée qui ne se lit pas n'empêche pas de
/// lire le reste du fichier (le texte n'en a pas besoin) : elle manque, et un avertissement dit pourquoi.
fn metadonnees(doc: &Document, avertissements: &mut Vec<String>) -> Result<std::collections::BTreeMap<String, String>, Refus> {
    let mut m = std::collections::BTreeMap::new();
    let Some(info) = lecture::lue(doc, &doc.trailer, b"Info", lecture::dictionnaire)? else {
        return Ok(m);
    };
    for (cle, valeur) in info.iter() {
        let texte = match lecture::designe(doc, valeur)? {
            Some(Object::String(s, _)) => match chaine_pdf(s) {
                Ok(t) => t,
                Err(r) => {
                    avertissements.push(format!("métadonnées : {} : {}", lecture::nom_de(cle), r.phrase));
                    continue;
                }
            },
            Some(Object::Name(n)) => nom_texte(n),
            Some(Object::Integer(i)) => i.to_string(),
            Some(o @ Object::Real(_)) => match Mesure::de_l_objet(o) {
                Some(r) => r.to_string(),
                None => {
                    avertissements.push(format!("métadonnées : {} n'est pas un nombre que la fabrique lit", lecture::nom_de(cle)));
                    continue;
                }
            },
            Some(Object::Boolean(b)) => b.to_string(),
            // Un tableau, un dictionnaire, un flux : rien qui se dise en une chaîne ; une clé nulle est absente.
            Some(Object::Array(_) | Object::Dictionary(_) | Object::Stream(_) | Object::Reference(_) | Object::Null) | None => continue,
        };
        m.insert(nom_texte(cle), texte);
    }
    Ok(m)
}

fn page_de(doc: &Document, numero: u32, id: lopdf::ObjectId, avertissements: &mut Vec<String>) -> Result<Page, Refus> {
    let cadre = lecture::cadre_de(doc, id).map_err(|r| r.reformuler(|m| format!("page {} : {}", numero, m)))?;
    let rotation = lecture::rotation_de(doc, id).map_err(|r| r.reformuler(|m| format!("page {} : {}", numero, m)))?;
    let repere = Repere { cadre, rotation };
    let (largeur, hauteur) = repere.dimensions();
    let mut page = Page {
        largeur: arrondi(largeur),
        hauteur: arrondi(hauteur),
        rotation,
        polices: Vec::new(),
        images: Vec::new(),
        traits: Vec::new(),
        mots: Vec::new(),
    };
    match inspecter_page(doc, id) {
        Ok(lue) => {
            page.polices = lue.polices;
            page.images = lue.images.iter().map(|b| repere.boite(b)).collect();
            page.traits = lue.traits.iter().map(|b| repere.boite(b)).collect();
            for (k, (texte, boites)) in lue.lignes.iter().enumerate() {
                mots_de(k, texte, boites, &repere, &mut page.mots);
            }
        }
        Err(r) => avertissements.push(format!("page {} : {}", numero, r.phrase)),
    }
    Ok(page)
}

/// L'inspection de `chemin` : la ligne de JSON, et les avertissements (pages illisibles).
pub fn inspecter(chemin: &str) -> Result<(String, Vec<String>), Refus> {
    let octets = std::fs::read(chemin).map_err(|e| Refus::illisible(format!("{} illisible ({})", chemin, e)))?;
    let mut doc = crate::lecture::charger(&octets).map_err(|r| r.reformuler(|p| format!("{} {}", chemin, p)))?;
    let dans = |r: Refus| r.reformuler(|p| format!("{} : {}", chemin, p));
    let chiffre = chiffrement(&doc).map_err(dans)?;
    crate::chiffrement::dechiffrer(&mut doc, b"").map_err(dans)?;
    let mut avertissements = Vec::new();
    let pages = doc.get_pages().into_iter().map(|(n, id)| page_de(&doc, n, id, &mut avertissements)).collect::<Result<_, _>>().map_err(dans)?;
    let inspection = Inspection { chiffrement: chiffre, metadonnees: metadonnees(&doc, &mut avertissements).map_err(dans)?, pages };
    let json = serde_json::to_string(&inspection).map_err(|e| Refus::defaut(format!("{} : {}", chemin, e)))?;
    Ok((json, avertissements))
}
