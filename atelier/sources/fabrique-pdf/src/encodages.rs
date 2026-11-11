//! Les encodages prédéfinis (ISO 32000-1, annexe D) : le nom du glyphe de chaque code de StandardEncoding,
//! MacRomanEncoding, WinAnsiEncoding (D.2), MacExpertEncoding (D.4), des encodages propres de Symbol (D.5) et de
//! ZapfDingbats (D.6), et le caractère de chaque code de PDFDocEncoding (D.3). Les tables sont celles de la
//! transcription déposée (`atelier/reference/sources/iso-32000/adobe-iso32000-1-annexe-d-20261111.txt`), lue telle
//! quelle, une fois par processus.

use std::sync::OnceLock;

use crate::description::{defaut, Refus};

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
    let mut a = Annexe { standard: vide(), mac_roman: vide(), win_ansi: vide(), mac_expert: vide(), symbol: vide(), zapf: vide(), pdfdoc: vec![None; 256] };
    let mut section = "";
    let mut vus_pdfdoc = 0;
    for ligne in ANNEXE_D.lines() {
        if let Some(titre) = ligne.strip_prefix("## ") {
            // « D.2 », ou « D.2, » pour ses notes.
            section = titre.split_once(' ').map_or(titre, |(s, _)| s);
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
                let (hex, unicode) = match mots.as_slice() {
                    [hex, _, unicode] | [hex, _, unicode, _] => (*hex, *unicode),
                    _ => return defaut(format!("l'annexe D déposée a, en D.3, la ligne « {} »", ligne)),
                };
                let code = hex.strip_prefix("0x").and_then(|h| usize::from_str_radix(h, 16).ok()).filter(|c| *c < 256);
                let Some(code) = code else {
                    return defaut(format!("l'annexe D déposée a, en D.3, le code {:?}", hex));
                };
                a.pdfdoc[code] = match unicode {
                    "Undefined" => None,
                    u => match u.strip_prefix("U+").and_then(|h| u32::from_str_radix(h, 16).ok()).and_then(char::from_u32) {
                        Some(c) => Some(c),
                        None => return defaut(format!("l'annexe D déposée a, en D.3, le caractère {:?}", u)),
                    },
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

/// Ce que montre un code d'un encodage : le nom d'un glyphe, ou un caractère que l'annexe D dit sans nom.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Code {
    Nom(&'static str),
    Caractere(char),
}

/// Ce que montre chaque code de `e` (`None` : non encodé). Les notes de D.2 s'y appliquent : WinAnsiEncoding, « all
/// unused codes greater than 40 map to the bullet character » (3), « The hyphen character is also encoded as 255 [...]
/// The meaning of this duplicate code shall be “soft hyphen” » (5) ; WinAnsiEncoding 240 et MacRomanEncoding 312, « This
/// duplicate code shall signify a nonbreaking space » (6).
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
            v[0o255] = Some(Code::Caractere('\u{AD}'));
            v[0o240] = Some(Code::Caractere('\u{A0}'));
            for c in v.iter_mut().skip(0o41) {
                if c.is_none() {
                    *c = Some(Code::Nom("bullet"));
                }
            }
        }
        Encodage::MacRoman => v[0o312] = Some(Code::Caractere('\u{A0}')),
        Encodage::Standard | Encodage::MacExpert | Encodage::Symbol | Encodage::ZapfDingbats => {}
    }
    Ok(v)
}

/// Le caractère d'un octet en PDFDocEncoding (D.3) ; `None` pour un code que la table dit « Undefined ».
pub(crate) fn pdfdoc(b: u8) -> Result<Option<char>, Refus> {
    Ok(annexe()?.pdfdoc[b as usize])
}
