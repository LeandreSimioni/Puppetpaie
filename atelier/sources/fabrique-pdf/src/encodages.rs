//! Les encodages prédéfinis (ISO 32000-1, annexe D) : le nom du glyphe de chaque code de StandardEncoding,
//! MacRomanEncoding, WinAnsiEncoding (D.2), MacExpertEncoding (D.4), des encodages propres de Symbol (D.5) et de
//! ZapfDingbats (D.6), et le caractère de chaque code de PDFDocEncoding (D.3), qui avec UTF-16BE fait les chaînes de
//! texte (7.9.2.2), lues et écrites ici seulement. Les tables sont celles de la
//! transcription déposée (`atelier/reference/sources/iso-32000/adobe-iso32000-1-annexe-d-20261111.txt`), lue telle
//! quelle, une fois par processus.

use std::sync::OnceLock;

use crate::description::{defaut, illisible, Refus};

const ANNEXE_D: &str = include_str!("../../../reference/sources/iso-32000/adobe-iso32000-1-annexe-d-20261111.txt");

/// Un encodage dont l'annexe D donne le nom du glyphe de chaque code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Encodage {
    Standard,
    MacRoman,
    WinAnsi,
    MacExpert,
    /// L'encodage propre de la police Symbol (D.5).
    Symbol,
    /// L'encodage propre de la police ZapfDingbats (D.6).
    ZapfDingbats,
}

/// Le nom du glyphe de chaque code (`None` : non encodé).
type Noms = Vec<Option<&'static str>>;

struct Annexe {
    standard: Noms,
    mac_roman: Noms,
    win_ansi: Noms,
    mac_expert: Noms,
    symbol: Noms,
    zapf: Noms,
    /// Le caractère de chaque code de PDFDocEncoding (`None` : « Undefined »).
    pdfdoc: Vec<Option<char>>,
    /// Les codes de PDFDocEncoding que la table ne note pas « U » (« Undefined code point in PDFDocEncoding ») : les
    /// seuls qu'on écrit.
    pdfdoc_defini: Vec<bool>,
}

static ANNEXE: OnceLock<Result<Annexe, Refus>> = OnceLock::new();

/// Un code octal de la transcription : `None` pour « — » (non encodé).
fn octal(t: &str, ligne: &str) -> Result<Option<usize>, Refus> {
    if t == "—" {
        return Ok(None);
    }
    match usize::from_str_radix(t, 8) {
        Ok(c) if c < 256 => Ok(Some(c)),
        Ok(c) => defaut(format!("l'annexe D déposée donne le code {} (octal {}) : « {} »", c, t, ligne)),
        Err(e) => defaut(format!("l'annexe D déposée a le code {:?} ({}) : « {} »", t, e, ligne)),
    }
}

/// Un nombre hexadécimal de la transcription, derrière son préfixe (« 0x », « U+ »).
fn hexadecimal(t: &str, prefixe: &str, ligne: &str) -> Result<u32, Refus> {
    let Some(h) = t.strip_prefix(prefixe) else {
        return defaut(format!("l'annexe D déposée a {:?} sans son préfixe {} : « {} »", t, prefixe, ligne));
    };
    u32::from_str_radix(h, 16).map_err(|e| Refus::defaut(format!("l'annexe D déposée a {:?} ({}) : « {} »", t, e, ligne)))
}

/// Pose `nom` au code `code` de `table` : deux noms pour un code sont un défaut de la transcription livrée.
fn poser(table: &mut Noms, code: Option<usize>, nom: &'static str, ligne: &str) -> Result<(), Refus> {
    let Some(c) = code else {
        return Ok(());
    };
    match table[c] {
        Some(autre) => defaut(format!("l'annexe D déposée donne au code {:o} les noms {} et {} : « {} »", c, autre, nom, ligne)),
        None => {
            table[c] = Some(nom);
            Ok(())
        }
    }
}

/// La transcription lue : chaque section par son titre (`## D.2`, `## D.3`…), chaque entrée de la forme que le titre
/// dit. Une ligne d'une autre forme est un défaut du logiciel : la transcription est livrée avec lui.
fn lire() -> Result<Annexe, Refus> {
    let vide = || vec![None; 256];
    let mut a = Annexe { standard: vide(), mac_roman: vide(), win_ansi: vide(), mac_expert: vide(), symbol: vide(), zapf: vide(), pdfdoc: vec![None; 256], pdfdoc_defini: vec![false; 256] };
    let mut section = "";
    let mut vus_pdfdoc = 0;
    for ligne in ANNEXE_D.lines() {
        if let Some(titre) = ligne.strip_prefix("## ") {
            // « D.2 », ou « D.2, » pour ses notes.
            section = match titre.split_once(' ') {
                Some((s, _)) => s,
                None => titre,
            };
            continue;
        }
        if ligne.starts_with('#') || ligne.trim().is_empty() {
            continue;
        }
        let mots: Vec<&'static str> = ligne.split(' ').collect();
        match section {
            "D.2" => {
                // NAME [note] STD MAC WIN PDF
                let (nom, codes) = match mots.as_slice() {
                    [nom, _, s, m, w, p] | [nom, s, m, w, p] if !nom.is_empty() => (*nom, [*s, *m, *w, *p]),
                    _ => return defaut(format!("l'annexe D déposée a, en D.2, la ligne « {} »", ligne)),
                };
                poser(&mut a.standard, octal(codes[0], ligne)?, nom, ligne)?;
                poser(&mut a.mac_roman, octal(codes[1], ligne)?, nom, ligne)?;
                poser(&mut a.win_ansi, octal(codes[2], ligne)?, nom, ligne)?;
                // La colonne PDF (PDFDocEncoding) : D.3 en donne les caractères.
            }
            // Les notes de D.2, mot pour mot : `codes` applique les notes 3, 5 et 6.
            "D.2," => {}
            "D.3" => {
                // Hex Octal Unicode [Notes]
                let (hex, unicode, note) = match mots.as_slice() {
                    [hex, _, unicode] => (*hex, *unicode, None),
                    [hex, _, unicode, note] => (*hex, *unicode, Some(*note)),
                    _ => return defaut(format!("l'annexe D déposée a, en D.3, la ligne « {} »", ligne)),
                };
                let code = match hexadecimal(hex, "0x", ligne)? {
                    c if c < 256 => c as usize,
                    c => return defaut(format!("l'annexe D déposée a, en D.3, le code {} : « {} »", c, ligne)),
                };
                a.pdfdoc[code] = match unicode {
                    "Undefined" => None,
                    u => match char::from_u32(hexadecimal(u, "U+", ligne)?) {
                        Some(c) => Some(c),
                        None => return defaut(format!("l'annexe D déposée a, en D.3, le caractère {:?}", u)),
                    },
                };
                a.pdfdoc_defini[code] = match note {
                    None | Some("SR") => a.pdfdoc[code].is_some(),
                    Some("U") => false,
                    Some(autre) => return defaut(format!("l'annexe D déposée a, en D.3, la note {:?} : « {} »", autre, ligne)),
                };
                vus_pdfdoc += 1;
            }
            "D.4" | "D.5" | "D.6" => {
                let [nom, code] = mots.as_slice() else {
                    return defaut(format!("l'annexe D déposée a, en {}, la ligne « {} »", section, ligne));
                };
                let table = match section {
                    "D.4" => &mut a.mac_expert,
                    "D.5" => &mut a.symbol,
                    _ => &mut a.zapf,
                };
                poser(table, octal(code, ligne)?, nom, ligne)?;
            }
            autre => return defaut(format!("l'annexe D déposée a la ligne « {} » hors d'une table (section {:?})", ligne, autre)),
        }
    }
    if vus_pdfdoc != 256 {
        return defaut(format!("l'annexe D déposée donne {} codes de PDFDocEncoding, la table D.3 en a 256", vus_pdfdoc));
    }
    Ok(a)
}

fn annexe() -> Result<&'static Annexe, Refus> {
    match ANNEXE.get_or_init(lire) {
        Ok(a) => Ok(a),
        Err(r) => Err(r.clone()),
    }
}

/// Ce que montre un code d'un encodage : le nom d'un glyphe, ou un caractère que l'annexe D dit sans nom, avec le nom
/// du glyphe qui le dessine (« typographically the same as »).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Code {
    Nom(&'static str),
    Caractere(char, &'static str),
}

/// Ce que montre chaque code de `e` (`None` : non encodé). Les notes de D.2 s'y appliquent : WinAnsiEncoding, « all
/// unused codes greater than 40 map to the bullet character » (3), « The hyphen character is also encoded as 255 [...]
/// The meaning of this duplicate code shall be “soft hyphen,” but it shall be typographically the same as hyphen » (5) ;
/// WinAnsiEncoding 240 et MacRomanEncoding 312, « This duplicate code shall signify a nonbreaking space; it shall be
/// typographically the same as [...] SPACE » (6).
pub(crate) fn codes(e: Encodage) -> Result<Vec<Option<Code>>, Refus> {
    let a = annexe()?;
    let noms = match e {
        Encodage::Standard => &a.standard,
        Encodage::MacRoman => &a.mac_roman,
        Encodage::WinAnsi => &a.win_ansi,
        Encodage::MacExpert => &a.mac_expert,
        Encodage::Symbol => &a.symbol,
        Encodage::ZapfDingbats => &a.zapf,
    };
    let mut v: Vec<Option<Code>> = noms.iter().map(|n| n.map(Code::Nom)).collect();
    match e {
        Encodage::WinAnsi => {
            v[0o255] = Some(Code::Caractere('\u{AD}', "hyphen"));
            v[0o240] = Some(Code::Caractere('\u{A0}', "space"));
            for c in v.iter_mut().skip(0o41) {
                if c.is_none() {
                    *c = Some(Code::Nom("bullet"));
                }
            }
        }
        Encodage::MacRoman => v[0o312] = Some(Code::Caractere('\u{A0}', "space")),
        Encodage::Standard | Encodage::MacExpert | Encodage::Symbol | Encodage::ZapfDingbats => {}
    }
    Ok(v)
}

/// La marque d'une chaîne de texte en UTF-16BE (ISO 32000-1, 7.9.2.2 : « the first two bytes shall be 254 followed by
/// 255 »).
const MARQUE_UTF16BE: [u8; 2] = [254, 255];
/// Ce qui ouvre et ferme une séquence d'échappement de langue (7.9.2.2 : « The Unicode value U+001B »).
const ECHAPPEMENT: u16 = 0x1B;

/// Le texte d'une chaîne de texte (ISO 32000-1, 7.9.2.2, `/ActualText`, les métadonnées, un champ de formulaire) :
/// UTF-16BE quand elle commence par sa marque, PDFDocEncoding sinon (annexe D.3), seules formes que la norme connaît.
/// Ce qu'elle ne permet pas de lire se refuse : un code que D.3 dit « Undefined », de l'UTF-16 qui n'en est pas (un
/// nombre impair d'octets, une moitié de paire), une séquence d'échappement mal formée. Une séquence d'échappement de
/// langue (U+001B, deux octets de langue, deux de pays au choix, U+001B) n'est pas du texte : elle tombe.
pub(crate) fn chaine_de_texte(s: &[u8]) -> Result<String, Refus> {
    let Some(utf16) = s.strip_prefix(&MARQUE_UTF16BE) else {
        let a = annexe()?;
        return s
            .iter()
            .map(|b| match a.pdfdoc[*b as usize] {
                Some(c) => Ok(c),
                None => illisible(format!("une chaîne de texte en PDFDocEncoding porte le code {:#04x}, que la norme laisse sans caractère (ISO 32000-1, annexe D.3)", b)),
            })
            .collect();
    };
    if utf16.len() % 2 == 1 {
        return illisible(format!("une chaîne de texte en UTF-16BE a un nombre impair d'octets ({} ; ISO 32000-1, 7.9.2.2)", utf16.len()));
    }
    let unites: Vec<u16> = utf16.chunks(2).map(|p| u16::from_be_bytes([p[0], p[1]])).collect();
    let mut texte = Vec::with_capacity(unites.len());
    let mut i = 0;
    while i < unites.len() {
        if unites[i] != ECHAPPEMENT {
            texte.push(unites[i]);
            i += 1;
            continue;
        }
        // La langue tient une unité de deux octets, le pays une autre, au choix.
        match (unites.get(i + 2), unites.get(i + 3)) {
            (Some(&ECHAPPEMENT), _) => i += 3,
            (_, Some(&ECHAPPEMENT)) => i += 4,
            _ => return illisible("une chaîne de texte en UTF-16BE ouvre une séquence d'échappement de langue sans la fermer (ISO 32000-1, 7.9.2.2)"),
        }
    }
    String::from_utf16(&texte).map_err(|e| Refus::illisible(format!("une chaîne de texte marquée UTF-16BE n'en est pas ({} ; ISO 32000-1, 7.9.2.2)", e)))
}

/// Une chaîne de texte pour `t` (ISO 32000-1, 7.9.2.2) : PDFDocEncoding quand chacun de ses caractères y a un code que
/// D.3 ne note pas « U », UTF-16BE avec sa marque sinon.
pub(crate) fn vers_chaine_de_texte(t: &str) -> Result<Vec<u8>, Refus> {
    let a = annexe()?;
    let code = |c: char| (0..256).find(|&b| a.pdfdoc_defini[b] && a.pdfdoc[b] == Some(c)).map(|b| b as u8);
    match t.chars().map(code).collect::<Option<Vec<u8>>>() {
        Some(octets) => Ok(octets),
        None => Ok(vers_utf16be(t)),
    }
}

/// Une chaîne de texte en UTF-16BE, sa marque en tête (ISO 32000-1, 7.9.2.2).
pub(crate) fn vers_utf16be(t: &str) -> Vec<u8> {
    let mut u = MARQUE_UTF16BE.to_vec();
    for unite in t.encode_utf16() {
        u.extend_from_slice(&unite.to_be_bytes());
    }
    u
}
