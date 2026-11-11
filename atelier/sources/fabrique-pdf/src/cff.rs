//! L'encodage propre d'un programme compact incorporé (`FontFile3` de sous-type `Type1C`, ISO 32000-1, table 126) :
//! pour chaque code, le nom de son glyphe, lu dans le programme selon « The Compact Font Format Specification »
//! (Adobe Technical Note #5176, `atelier/reference/sources/adobe-polices/adobe-cff-extraits-20261111.md`) et ses
//! tables (annexes A, B et C, `adobe-cff-tables-20261111.txt`, lues telles quelles). Rien n'est complété : un code que
//! l'encodage du programme ne donne pas n'a pas de glyphe (« glyphs that are not specified by custom or predefined
//! encodings are unencoded », section 12).

use std::sync::OnceLock;

use crate::description::{defaut, illisible, Refus};

const TABLES: &str = include_str!("../../../reference/sources/adobe-polices/adobe-cff-tables-20261111.txt");

/// Les tables des annexes : les chaînes standard (A), les encodages prédéfinis (B : code -> SID), les jeux de caractères
/// prédéfinis (C : GID 1, 2… -> SID).
struct Tables {
    chaines: Vec<&'static str>,
    standard: Vec<Option<u16>>,
    expert: Vec<Option<u16>>,
    iso_adobe: Vec<u16>,
    jeu_expert: Vec<u16>,
    jeu_expert_reduit: Vec<u16>,
}

static LUES: OnceLock<Result<Tables, Refus>> = OnceLock::new();

fn lire_tables() -> Result<Tables, Refus> {
    let mut t = Tables { chaines: Vec::new(), standard: vec![None; 256], expert: vec![None; 256], iso_adobe: Vec::new(), jeu_expert: Vec::new(), jeu_expert_reduit: Vec::new() };
    let mut section = "";
    let nombre = |m: &str, ligne: &str| -> Result<u16, Refus> {
        m.parse().map_err(|e| Refus::defaut(format!("les tables CFF déposées ont la ligne « {} » ({})", ligne, e)))
    };
    for ligne in TABLES.lines() {
        if let Some(titre) = ligne.strip_prefix("## ") {
            section = titre.split(" : ").next().unwrap_or(titre);
            continue;
        }
        if ligne.starts_with('#') || ligne.trim().is_empty() {
            continue;
        }
        let mots: Vec<&'static str> = ligne.split(' ').collect();
        match (section, mots.as_slice()) {
            ("A Standard Strings", [sid, nom]) => {
                if nombre(sid, ligne)? as usize != t.chaines.len() {
                    return defaut(format!("les chaînes standard déposées ne se suivent pas : « {} »", ligne));
                }
                t.chaines.push(nom);
            }
            ("B Standard Encoding" | "B Expert Encoding", [code, sid, _]) => {
                let (code, sid) = (nombre(code, ligne)? as usize, nombre(sid, ligne)?);
                let table = if section == "B Standard Encoding" { &mut t.standard } else { &mut t.expert };
                // Le SID 0 est .notdef : le code n'est pas encodé.
                match table.get_mut(code) {
                    Some(c) => *c = (sid != 0).then_some(sid),
                    None => return defaut(format!("les tables CFF déposées ont le code {} : « {} »", code, ligne)),
                }
            }
            ("C ISOAdobe" | "C Expert" | "C ExpertSubset", [sid, _]) => {
                let sid = nombre(sid, ligne)?;
                match section {
                    "C ISOAdobe" => t.iso_adobe.push(sid),
                    "C Expert" => t.jeu_expert.push(sid),
                    _ => t.jeu_expert_reduit.push(sid),
                }
            }
            _ => return defaut(format!("les tables CFF déposées ont, en {:?}, la ligne « {} »", section, ligne)),
        }
    }
    Ok(t)
}

fn tables() -> Result<&'static Tables, Refus> {
    match LUES.get_or_init(lire_tables) {
        Ok(t) => Ok(t),
        Err(r) => Err(r.clone()),
    }
}

/// Une lecture dans les octets du programme : ce qui dépasse se refuse.
struct Octets<'a>(&'a [u8]);

impl<'a> Octets<'a> {
    fn tranche(&self, debut: usize, n: usize, quoi: &str) -> Result<&'a [u8], Refus> {
        match debut.checked_add(n).and_then(|fin| self.0.get(debut..fin)) {
            Some(t) => Ok(t),
            None => illisible(format!("le programme compact s'arrête dans {} (octet {})", quoi, debut)),
        }
    }
    fn card8(&self, i: usize, quoi: &str) -> Result<usize, Refus> {
        Ok(self.tranche(i, 1, quoi)?[0] as usize)
    }
    fn card16(&self, i: usize, quoi: &str) -> Result<usize, Refus> {
        let t = self.tranche(i, 2, quoi)?;
        Ok((t[0] as usize) << 8 | t[1] as usize)
    }
    /// Un décalage de `taille` octets (1 à 4, table 2), poids fort d'abord.
    fn decalage(&self, i: usize, taille: usize, quoi: &str) -> Result<usize, Refus> {
        if !(1..=4).contains(&taille) {
            return illisible(format!("le programme compact donne à {} des décalages de {} octets (table 2 : 1 à 4)", quoi, taille));
        }
        Ok(self.tranche(i, taille, quoi)?.iter().fold(0usize, |a, b| a << 8 | *b as usize))
    }

    /// Un INDEX (table 7) qui commence en `i` : ses objets, et l'octet qui le suit.
    fn index(&self, i: usize, quoi: &str) -> Result<(Vec<&'a [u8]>, usize), Refus> {
        let n = self.card16(i, quoi)?;
        if n == 0 {
            return Ok((Vec::new(), i + 2));
        }
        let taille = self.card8(i + 2, quoi)?;
        let table = i + 3;
        // « Offsets in the offset array are relative to the byte that precedes the object data. »
        let donnees = table + (n + 1) * taille - 1;
        let mut decalages = Vec::with_capacity(n + 1);
        for k in 0..=n {
            decalages.push(self.decalage(table + k * taille, taille, quoi)?);
        }
        let mut objets = Vec::with_capacity(n);
        for k in 0..n {
            let (a, b) = (decalages[k], decalages[k + 1]);
            if b < a {
                return illisible(format!("l'INDEX {} du programme compact a des décalages qui reculent", quoi));
            }
            objets.push(self.tranche(donnees + a, b - a, quoi)?);
        }
        Ok((objets, donnees + decalages[n]))
    }
}

/// Les opérateurs d'un DICT (section 4) : (opérateur, opérandes entiers). Un réel (30) se lit et se saute : aucun des
/// opérateurs qu'on lit n'en prend.
fn dict(d: &[u8]) -> Result<Vec<(u16, Vec<i64>)>, Refus> {
    let mut sortie = Vec::new();
    let mut operandes: Vec<i64> = Vec::new();
    let mut i = 0;
    let octet = |k: usize| -> Result<i64, Refus> {
        match d.get(k) {
            Some(b) => Ok(*b as i64),
            None => illisible("un DICT du programme compact s'arrête dans un opérande (section 4)"),
        }
    };
    while i < d.len() {
        let b0 = d[i] as i64;
        match b0 {
            0..=21 => {
                let op = if b0 == 12 {
                    i += 1;
                    1200 + octet(i)? as u16
                } else {
                    b0 as u16
                };
                sortie.push((op, std::mem::take(&mut operandes)));
                i += 1;
            }
            28 => {
                operandes.push((((octet(i + 1)? << 8) | octet(i + 2)?) as u16 as i16) as i64);
                i += 3;
            }
            29 => {
                let v = (octet(i + 1)? << 24) | (octet(i + 2)? << 16) | (octet(i + 3)? << 8) | octet(i + 4)?;
                operandes.push(v as u32 as i32 as i64);
                i += 5;
            }
            30 => {
                // Les quartets jusqu'au quartet f (table 5).
                i += 1;
                loop {
                    let b = octet(i)?;
                    i += 1;
                    if b >> 4 == 0xf || b & 0xf == 0xf {
                        break;
                    }
                }
                operandes.push(0);
            }
            32..=246 => {
                operandes.push(b0 - 139);
                i += 1;
            }
            247..=250 => {
                operandes.push((b0 - 247) * 256 + octet(i + 1)? + 108);
                i += 2;
            }
            251..=254 => {
                operandes.push(-(b0 - 251) * 256 - octet(i + 1)? - 108);
                i += 2;
            }
            _ => return illisible(format!("un DICT du programme compact porte l'octet réservé {} (section 4)", b0)),
        }
    }
    Ok(sortie)
}

/// Le nom de glyphe de chaque code (256) d'un programme compact (`FontFile3`, `Type1C`) : `None` pour un code non
/// encodé.
pub(crate) fn noms_des_codes(programme: &[u8]) -> Result<Vec<Option<String>>, Refus> {
    let t = tables()?;
    let o = Octets(programme);
    // Table 8 : la version majeure 1 est la seule que la spécification déposée décrit.
    let majeure = o.card8(0, "l'en-tête")?;
    if majeure != 1 {
        return illisible(format!("le programme compact est de version majeure {}, la spécification déposée décrit la 1 (section 6)", majeure));
    }
    let taille_entete = o.card8(2, "l'en-tête")?;
    let (_, apres_noms) = o.index(taille_entete, "des noms")?;
    let (dicts, apres_dicts) = o.index(apres_noms, "des Top DICT")?;
    let (chaines, _) = o.index(apres_dicts, "des chaînes")?;
    let Some(premier) = dicts.first() else {
        return illisible("le programme compact n'a pas de Top DICT (section 8)");
    };
    let mut charset = 0usize;
    let mut encodage = 0usize;
    let mut charstrings = None;
    for (op, v) in dict(premier)? {
        let un = |quoi: &str| -> Result<usize, Refus> {
            match v.as_slice() {
                [x] if *x >= 0 => Ok(*x as usize),
                _ => illisible(format!("l'opérateur {} du Top DICT n'a pas un opérande positif (table 9)", quoi)),
            }
        };
        match op {
            15 => charset = un("charset")?,
            16 => encodage = un("Encoding")?,
            17 => charstrings = Some(un("CharStrings")?),
            // Table 10 : « CIDFonts which specify no encoding » ; le Top DICT d'une CIDFont commence par ROS.
            1230 => return illisible("le programme compact est une CIDFont (ROS, table 10), sans encodage (section 12)"),
            _ => {}
        }
    }
    let Some(charstrings) = charstrings else {
        return illisible("le Top DICT du programme compact n'a pas de CharStrings (table 9)");
    };
    let n_glyphes = o.card16(charstrings, "les CharStrings")?;

    // Le jeu de caractères (section 13) : le SID de chaque GID ; le GID 0 est .notdef.
    let mut sids: Vec<u16> = vec![0];
    let predefini = |jeu: &[u16]| -> Result<Vec<u16>, Refus> {
        // « A font may use a predefined charset if it exactly matches in the first nGlyphs. »
        match jeu.get(..n_glyphes.saturating_sub(1)) {
            Some(j) => Ok(j.to_vec()),
            None => illisible(format!("le programme compact a {} glyphes, plus que son jeu de caractères prédéfini (section 13)", n_glyphes)),
        }
    };
    match charset {
        0 => sids.extend(predefini(&t.iso_adobe)?),
        1 => sids.extend(predefini(&t.jeu_expert)?),
        2 => sids.extend(predefini(&t.jeu_expert_reduit)?),
        debut => {
            let format = o.card8(debut, "le jeu de caractères")?;
            let mut i = debut + 1;
            while sids.len() < n_glyphes {
                match format {
                    0 => {
                        sids.push(o.card16(i, "le jeu de caractères")? as u16);
                        i += 2;
                    }
                    1 | 2 => {
                        let premier = o.card16(i, "le jeu de caractères")?;
                        let reste = if format == 1 { o.card8(i + 2, "le jeu de caractères")? } else { o.card16(i + 2, "le jeu de caractères")? };
                        i += if format == 1 { 3 } else { 4 };
                        for k in 0..=reste {
                            sids.push((premier + k) as u16);
                        }
                    }
                    f => return illisible(format!("le jeu de caractères du programme compact est de format {} (section 13 : 0, 1 ou 2)", f)),
                }
            }
            sids.truncate(n_glyphes);
        }
    }
    let nom = |sid: u16| -> Result<String, Refus> {
        let s = sid as usize;
        match t.chaines.get(s) {
            Some(n) => Ok(n.to_string()),
            None => match chaines.get(s - t.chaines.len()) {
                Some(octets) => Ok(String::from_utf8_lossy(octets).into_owned()),
                None => illisible(format!("le programme compact nomme le SID {}, que ses chaînes n'ont pas (section 10)", sid)),
            },
        }
    };

    // L'encodage (section 12) : le GID de chaque code ; les tableaux commencent au GID 1 (section 11).
    let mut codes: Vec<Option<String>> = vec![None; 256];
    match encodage {
        0 | 1 => {
            // Encodages prédéfinis : du code au SID (note 4) ; un glyphe que le programme n'a pas n'est pas montré.
            let table = if encodage == 0 { &t.standard } else { &t.expert };
            for (code, sid) in table.iter().enumerate() {
                if let Some(sid) = sid {
                    if sids.contains(sid) {
                        codes[code] = Some(nom(*sid)?);
                    }
                }
            }
        }
        debut => {
            let format = o.card8(debut, "l'encodage")?;
            let mut i = debut + 1;
            let mut gid = 1usize;
            let mut poser = |code: usize, gid: usize| -> Result<(), Refus> {
                if let Some(sid) = sids.get(gid) {
                    codes[code] = Some(nom(*sid)?);
                }
                Ok(())
            };
            match format & 0x7f {
                0 => {
                    let n = o.card8(i, "l'encodage")?;
                    for k in 0..n {
                        poser(o.card8(i + 1 + k, "l'encodage")?, gid)?;
                        gid += 1;
                    }
                    i += 1 + n;
                }
                1 => {
                    let n = o.card8(i, "l'encodage")?;
                    for k in 0..n {
                        let premier = o.card8(i + 1 + 2 * k, "l'encodage")?;
                        let reste = o.card8(i + 2 + 2 * k, "l'encodage")?;
                        for c in premier..=premier + reste {
                            if c > 255 {
                                return illisible("une plage de l'encodage du programme compact dépasse le code 255 (section 12)");
                            }
                            poser(c, gid)?;
                            gid += 1;
                        }
                    }
                    i += 1 + 2 * n;
                }
                f => return illisible(format!("l'encodage du programme compact est de format {} (section 12 : 0 ou 1)", f)),
            }
            // Table 14 : le bit de poids fort du format annonce des codes supplémentaires (code, SID).
            if format & 0x80 != 0 {
                let n = o.card8(i, "l'encodage")?;
                for k in 0..n {
                    let code = o.card8(i + 1 + 3 * k, "l'encodage")?;
                    let sid = o.card16(i + 2 + 3 * k, "l'encodage")? as u16;
                    codes[code] = Some(nom(sid)?);
                }
            }
        }
    }
    Ok(codes)
}
