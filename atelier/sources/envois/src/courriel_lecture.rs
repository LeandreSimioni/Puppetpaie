//! Lire un message RECU : le texte propre, l'adresse de l'expediteur, la date.
//!
//! Ce que la personne a ECRIT elle-meme dans sa reponse (la citation du message d'origine
//! exclue), l'adresse dont elle vient, la date a laquelle elle l'a ecrite : la releve des fiches s'en sert
//! (`routes::fiches`), celle des signatures s'en sert aussi. Lecture MIME par `mailparse`
//! (charset inconnu, en-tete `Date` sans fuseau, entites HTML hors du latin courant : voir les fonctions concernees).
//!
//! La citation commence a la premiere ligne qui ressemble a l'une des formes de `_DEBUT_CITATION` : `>`,
//! « Le ... a ecrit : », « On ... wrote: », une ligne de tirets, « De : », « From: », une ligne de soulignes.
//! Le `\r` final d'une ligne ne compte pas : un message qui sort d'IMAP a des fins de ligne CRLF, et
//! « -----Original Message----- » doit encore etre reconnu.

use mailparse::{MailHeaderMap, ParsedMail};

use crate::simulation_courriels::{feuilles, nom_de_fichier, texte_de};

/// L'adresse seule, en minuscules ; `None` quand le message ne dit pas d'ou il vient.
pub fn adresse_de(m: &ParsedMail) -> Option<String> {
    let h = m.headers.get_first_header("From")?;
    let liste = match mailparse::addrparse_header(h) {
        Ok(liste) => liste,
        // Un `From` qui n'est pas une adresse : le message ne dit pas d'ou il vient.
        Err(pas_une_adresse) => {
            drop(pas_une_adresse);
            return None;
        }
    };
    for a in liste.iter() {
        match a {
            mailparse::MailAddr::Single(i) => return Some(socle::texte::strip(&i.addr).to_lowercase()),
            mailparse::MailAddr::Group(g) => {
                if let Some(i) = g.addrs.first() {
                    return Some(socle::texte::strip(&i.addr).to_lowercase());
                }
            }
        }
    }
    None
}

/// Un message se lit toujours : quand le bloc d'en-tetes se rompt (une ligne qui n'est ni `Nom: valeur`,
/// ni une suite de ligne, ni `From `), les en-tetes s'arretent AVANT cette ligne et tout le reste est le corps ;
/// `mailparse` prendrait cette ligne pour un en-tete (corps vide) ou refuserait le message. Ce qu'il faut lire est le message ainsi recoupe
/// (les en-tetes valides, une ligne vide, le reste). `Borrowed` quand les en-tetes sont sains.
pub fn recouper_les_entetes(brut: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    let est_entete = |l: &[u8]| -> bool {
        l.starts_with(b"From ")
            || matches!(l.first(), Some(b' ') | Some(b'\t'))
            || l.iter().position(|c| *c == b':').is_some_and(|i| l[..i].iter().all(|c| (0x21..=0x39).contains(c) || (0x3b..=0x7e).contains(c)))
    };
    let mut fin_des_entetes = 0;
    let mut ix = 0;
    let mut rompu = false;
    while ix < brut.len() {
        let suite = &brut[ix..];
        let longueur = match suite.iter().position(|c| *c == b'\n') {
            Some(i) => i + 1,
            None => suite.len(),
        };
        let ligne = sans_fin_de_ligne(&suite[..longueur]);
        if ligne.is_empty() {
            break;
        }
        if !est_entete(ligne) {
            rompu = true;
            break;
        }
        ix += longueur;
        fin_des_entetes = ix;
    }
    if !rompu {
        return std::borrow::Cow::Borrowed(brut);
    }
    let mut refait = brut[..fin_des_entetes].to_vec();
    if !refait.is_empty() && !refait.ends_with(b"\n") {
        refait.push(b'\n');
    }
    refait.push(b'\n');
    refait.extend_from_slice(&brut[fin_des_entetes..]);
    std::borrow::Cow::Owned(refait)
}

/// Une ligne sans sa fin (`\n`, `\r\n`).
fn sans_fin_de_ligne(l: &[u8]) -> &[u8] {
    let l = match l.strip_suffix(b"\n") {
        Some(sans) => sans,
        None => l,
    };
    match l.strip_suffix(b"\r") {
        Some(sans) => sans,
        None => l,
    }
}

/// L'identifiant du message (`Message-ID`), sans blancs aux bords ; `None` s'il manque.
pub fn identifiant_de(m: &ParsedMail) -> Option<String> {
    identifiant_des_entetes(&m.headers)
}

/// L'identifiant (`Message-ID`) d'en-tetes lus seuls (`mailparse::parse_headers`), s'ils le portent.
pub fn identifiant_des_entetes(entetes: &[mailparse::MailHeader]) -> Option<String> {
    entetes.get_first_value("Message-ID").map(|v| socle::texte::strip(&v).to_string())
}

/// La date du message, en heure locale (Paris), ISO a la seconde -- le format du journal. Sans en-tete
/// `Date` lisible, l'heure de la releve (`maintenant`, deja en ISO).
///
/// `-0000` est une heure UT, comme `+0000` : la RFC 5322, § 3.3 (`atelier/reference/sources/ietf/LISEZ-MOI.md`), ne lui
/// retire que l'information sur le fuseau LOCAL de l'expediteur ; un nom de fuseau inconnu (`CEST`) et une zone
/// militaire valent `-0000` (§ 4.3). Un en-tete sans fuseau se lit comme une heure de Paris : une INFERENCE du moteur,
/// pas un texte. La RFC exige la zone (`time = time-of-day zone`, l. 808 ; `obs-zone`, l. 1838) et laisse a
/// l'implementation ce qu'elle fait d'une forme qu'elle ne decrit pas, pourvu qu'elle ne perde pas le message (§ 4,
/// l. 1711-1718 : « It is up to the implementation to deal with messages robustly »). Un champ hors
/// limites (31 fevrier, 24 h, 60 s, decalage de 24 h) est illisible.
pub fn date_de(m: &ParsedMail, maintenant: socle::paris::InstantEcrit) -> socle::paris::InstantEcrit {
    match m.headers.get_first_value("Date").as_deref().and_then(date_lue) {
        Some(local) => socle::paris::InstantEcrit::du_naif(local),
        None => maintenant,
    }
}

/// Les mois en anglais (les abreges, puis les noms entiers).
const MOIS: [&str; 24] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec", "january", "february", "march", "april", "may", "june",
    "july", "august", "september", "october", "november", "december",
];

/// Un entier signe en chiffres ASCII (ni autres ecritures de chiffres, ni `_`).
fn entier(t: &str) -> Option<i64> {
    let (signe, chiffres) = if let Some(c) = t.strip_prefix('-') {
        (-1, c)
    } else if let Some(c) = t.strip_prefix('+') {
        (1, c)
    } else {
        (1, t)
    };
    if chiffres.is_empty() || !chiffres.bytes().all(|b| b.is_ascii_digit()) || chiffres.len() > 15 {
        return None;
    }
    // Quinze chiffres au plus : la valeur tient dans un i64.
    Some(signe * chiffres.bytes().fold(0i64, |n, b| n * 10 + i64::from(b - b'0')))
}

/// (an, mois, jour, heure, minute, seconde, decalage en secondes ou `None`).
fn parsedate_tz(donnees: &str) -> Option<(i64, i64, i64, i64, i64, i64, Option<i64>)> {
    let mut d: Vec<String> = donnees.split(socle::texte::blanc).filter(|x| !x.is_empty()).map(String::from).collect();
    if d.is_empty() {
        return None;
    }
    const JOURS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
    if d[0].ends_with(',') || JOURS.contains(&d[0].to_lowercase().as_str()) {
        d.remove(0);
    } else if let Some(i) = d[0].rfind(',') {
        d[0] = d[0][i + 1..].to_string();
    }
    if d.len() == 3 {
        let morceaux: Vec<String> = d[0].split('-').map(String::from).collect();
        if morceaux.len() == 3 {
            let reste: Vec<String> = d[1..].to_vec();
            d = morceaux.into_iter().chain(reste).collect();
        }
    }
    if d.len() == 4 {
        let s = d[3].clone();
        let i = s.find('+').or_else(|| s.find('-'));
        match i {
            Some(i) if i > 0 => {
                d.truncate(3);
                d.push(s[..i].to_string());
                d.push(s[i..].to_string());
            }
            _ => d.push(String::new()),
        }
    }
    if d.len() < 5 {
        return None;
    }
    d.truncate(5);
    let (mut dd, mut mm, mut yy, mut tm, mut tz) = (d[0].clone(), d[1].clone(), d[2].clone(), d[3].clone(), d[4].clone());
    if dd.is_empty() || mm.is_empty() || yy.is_empty() {
        return None;
    }
    mm = mm.to_lowercase();
    if !MOIS.contains(&mm.as_str()) {
        let ancien = dd.clone();
        dd = mm.clone();
        mm = ancien.to_lowercase();
        if !MOIS.contains(&mm.as_str()) {
            return None;
        }
    }
    let mut mois = MOIS.iter().position(|m| *m == mm).unwrap() as i64 + 1;
    if mois > 12 {
        mois -= 12;
    }
    if dd.ends_with(',') {
        dd.pop();
    }
    if yy.find(':').is_some_and(|i| i > 0) {
        std::mem::swap(&mut yy, &mut tm);
    }
    if yy.ends_with(',') {
        yy.pop();
        if yy.is_empty() {
            return None;
        }
    }
    if !yy.chars().next().is_some_and(|c| declarations::champs::est_chiffre(c)) {
        std::mem::swap(&mut yy, &mut tz);
    }
    if tm.ends_with(',') {
        tm.pop();
    }
    let t: Vec<&str> = tm.split(':').collect();
    let (hh, mi, ss): (String, String, String) = match t.len() {
        2 => (t[0].into(), t[1].into(), "0".into()),
        3 => (t[0].into(), t[1].into(), t[2].into()),
        1 if t[0].contains('.') => {
            let p: Vec<&str> = t[0].split('.').collect();
            match p.len() {
                2 => (p[0].into(), p[1].into(), "0".into()),
                3 => (p[0].into(), p[1].into(), p[2].into()),
                _ => return None,
            }
        }
        _ => return None,
    };
    let (mut an, jour, heure, minute, seconde) = (entier(&yy)?, entier(&dd)?, entier(&hh)?, entier(&mi)?, entier(&ss)?);
    if an < 100 {
        an += if an > 68 { 1900 } else { 2000 };
    }
    let tz = tz.to_uppercase();
    let nommes: [(&str, i64); 14] = [
        ("UT", 0), ("UTC", 0), ("GMT", 0), ("Z", 0), ("AST", -400), ("ADT", -300), ("EST", -500), ("EDT", -400), ("CST", -600), ("CDT", -500), ("MST", -700),
        ("MDT", -600), ("PST", -800), ("PDT", -700),
    ];
    // Un nom de fuseau que cette table ne connait pas (« CEST »), ou une zone militaire d'une lettre : RFC 5322, § 4.3,
    // « SHOULD be considered equivalent to "-0000" » -- une heure UT (§ 3.3). Sans fuseau, ou avec un commentaire seul
    // (« (CEST) »), `None` : l'heure de Paris, une inference du moteur que la RFC laisse a l'implementation (§ 4,
    // l. 1711-1718 ; `atelier/reference/sources/ietf/LISEZ-MOI.md`).
    let decalage: Option<i64> = match nommes.iter().find(|(n, _)| *n == tz) {
        Some((_, v)) => Some(*v),
        None if !tz.is_empty() && tz.bytes().all(|b| b.is_ascii_alphabetic()) => Some(0),
        None => entier(&tz),
    };
    let decalage = decalage.map(|v| {
        if v == 0 {
            0
        } else {
            let (signe, v) = if v < 0 { (-1, -v) } else { (1, v) };
            signe * ((v / 100) * 3600 + (v % 100) * 60)
        }
    });
    Some((an, mois, jour, heure, minute, seconde, decalage))
}

/// L'en-tete `Date` lu, converti en heure locale naive, ou `None` quand il est illisible (ou hors du calendrier).
fn date_lue(brut: &str) -> Option<socle::paris::Naif> {
    let (an, mois, jour, h, mi, s, decalage) = parsedate_tz(brut)?;
    let (an, mois, jour) = match (i32::try_from(an), u32::try_from(mois), u32::try_from(jour)) {
        (Ok(a), Ok(m), Ok(j)) => (a, m, j),
        // Un champ hors de ce qu'une date peut porter : l'en-tete est illisible.
        _ => return None,
    };
    let j = socle::date::Jour::nouveau(an, mois, jour)?;
    if !(0..24).contains(&h) || !(0..60).contains(&mi) || !(0..60).contains(&s) {
        return None;
    }
    let naif = socle::paris::Naif::du_jour(j, h, mi, s)?;
    match decalage {
        None => Some(naif),
        Some(d) => {
            // `timezone(timedelta(seconds=d))` : strictement entre -24 h et 24 h.
            if d.abs() >= 86_400 {
                return None;
            }
            let t = (j.ordinal() - 719_163) * 86_400 + h * 3_600 + mi * 60 + s - d;
            Some(socle::paris::Moment::de_secondes(t)?.local_naif())
        }
    }
}

/// Ce que l'auteur de la reponse a ecrit lui-meme, citation exclue. La partie texte ; a defaut,
/// le HTML debarrasse de ses balises (et de sa citation Gmail). Puis on coupe au premier signe de citation.
pub fn texte_propre(m: &ParsedMail) -> String {
    let mut fs = Vec::new();
    feuilles(m, &mut fs);
    let (mut texte, mut html) = (String::new(), String::new());
    for f in fs {
        // Une feuille qui n'est pas du texte (ou dont le type ne se lit pas), ou qui porte un nom de fichier, est une
        // pièce jointe.
        let Some(("text", sous)) = f.ctype.mimetype.split_once('/') else { continue };
        if nom_de_fichier(f).is_some_and(|n| !socle::texte::strip(&n).is_empty()) {
            continue;
        }
        let contenu = texte_de(f);
        if sous == "plain" && texte.is_empty() {
            texte = contenu;
        } else if sous == "html" && html.is_empty() {
            html = contenu;
        }
    }
    if texte.is_empty() && !html.is_empty() {
        let coupe = ["<blockquote", "class=\"gmail_quote"].iter().filter_map(|b| html.find(b)).min();
        let html = match coupe {
            Some(c) => &html[..c],
            None => &html[..],
        };
        texte = html_desechapper(&sans_balises(html));
    }
    match debut_de_citation(&texte) {
        Some(i) => texte[..i].to_string(),
        None => texte,
    }
}

/// Le HTML, chaque balise remplacée par un espace.
fn sans_balises(html: &str) -> String {
    let c: Vec<char> = html.chars().collect();
    let mut sortie = String::with_capacity(html.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] == '<' {
            // `[^>]+` : au moins un signe, puis `>`.
            if let Some(fin) = c[i + 1..].iter().position(|x| *x == '>').filter(|f| *f >= 1) {
                sortie.push(' ');
                i += fin + 2;
                continue;
            }
        }
        sortie.push(c[i]);
        i += 1;
    }
    sortie
}

// ------------------------------------------------------------------ la citation

/// L'indice (en octets) de la premiere ligne qui commence une citation. Les lignes
/// se coupent sur `\n` seulement.
fn debut_de_citation(texte: &str) -> Option<usize> {
    let mut debut = 0;
    for ligne in texte.split('\n') {
        if commence_une_citation(ligne) {
            return Some(debut);
        }
        debut += ligne.len() + 1;
    }
    None
}

fn commence_une_citation(ligne: &str) -> bool {
    ligne.starts_with('>')
        || forme_de_citation(ligne, "Le ", " a écrit", true)
        || forme_de_citation(ligne, "On ", " wrote:", false)
        || ligne_de_tirets(ligne)
        || ligne.starts_with("De:")
        || ligne.starts_with("De :")
        || ligne.starts_with("From:")
        || ligne.starts_with("________________")
}

/// `<tete>.{3,120}<milieu>` puis, si `deux_points`, ` ?:`.
fn forme_de_citation(ligne: &str, tete: &str, milieu: &str, deux_points: bool) -> bool {
    let Some(reste) = ligne.strip_prefix(tete) else { return false };
    let c: Vec<char> = reste.chars().collect();
    let milieu: Vec<char> = milieu.chars().collect();
    (3..=120.min(c.len())).any(|i| {
        let suite = &c[i..];
        if !suite.starts_with(&milieu) {
            return false;
        }
        if !deux_points {
            return true;
        }
        match suite.get(milieu.len()) {
            Some(':') => true,
            Some(' ') => suite.get(milieu.len() + 1) == Some(&':'),
            _ => false,
        }
    })
}

/// `-{3,}.*-{3,}$` : trois tirets au moins en tete, trois au moins en queue, sans se recouvrir (le `\r` final ne compte pas).
fn ligne_de_tirets(ligne: &str) -> bool {
    let ligne = match ligne.strip_suffix('\r') {
        Some(sans) => sans,
        None => ligne,
    };
    let c: Vec<char> = ligne.chars().collect();
    c.len() >= 6 && c[..3].iter().all(|x| *x == '-') && c[c.len() - 3..].iter().all(|x| *x == '-')
}

// ------------------------------------------------------------------ html.unescape

/// Les noms d'entite qu'`html.unescape` reconnait SANS point-virgule (la liste historique de HTML : le latin-1, plus
/// `amp`, `lt`, `gt`, `quot`) : le nom et ce qu'il vaut.
const ENTITES_HISTORIQUES: &[(&str, char)] = &[
    ("AElig", 'Æ'), ("AMP", '&'), ("Aacute", 'Á'), ("Acirc", 'Â'), ("Agrave", 'À'), ("Aring", 'Å'), ("Atilde", 'Ã'),
    ("Auml", 'Ä'), ("COPY", '©'), ("Ccedil", 'Ç'), ("ETH", 'Ð'), ("Eacute", 'É'), ("Ecirc", 'Ê'), ("Egrave", 'È'),
    ("Euml", 'Ë'), ("GT", '>'), ("Iacute", 'Í'), ("Icirc", 'Î'), ("Igrave", 'Ì'), ("Iuml", 'Ï'), ("LT", '<'),
    ("Ntilde", 'Ñ'), ("Oacute", 'Ó'), ("Ocirc", 'Ô'), ("Ograve", 'Ò'), ("Oslash", 'Ø'), ("Otilde", 'Õ'), ("Ouml", 'Ö'),
    ("QUOT", '"'), ("REG", '®'), ("THORN", 'Þ'), ("Uacute", 'Ú'), ("Ucirc", 'Û'), ("Ugrave", 'Ù'), ("Uuml", 'Ü'),
    ("Yacute", 'Ý'), ("aacute", 'á'), ("acirc", 'â'), ("acute", '´'), ("aelig", 'æ'), ("agrave", 'à'), ("amp", '&'),
    ("aring", 'å'), ("atilde", 'ã'), ("auml", 'ä'), ("brvbar", '¦'), ("ccedil", 'ç'), ("cedil", '¸'), ("cent", '¢'),
    ("copy", '©'), ("curren", '¤'), ("deg", '°'), ("divide", '÷'), ("eacute", 'é'), ("ecirc", 'ê'), ("egrave", 'è'),
    ("eth", 'ð'), ("euml", 'ë'), ("frac12", '½'), ("frac14", '¼'), ("frac34", '¾'), ("gt", '>'), ("iacute", 'í'),
    ("icirc", 'î'), ("iexcl", '¡'), ("igrave", 'ì'), ("iquest", '¿'), ("iuml", 'ï'), ("laquo", '«'), ("lt", '<'),
    ("macr", '¯'), ("micro", 'µ'), ("middot", '·'), ("nbsp", '\u{a0}'), ("not", '¬'), ("ntilde", 'ñ'), ("oacute", 'ó'),
    ("ocirc", 'ô'), ("ograve", 'ò'), ("ordf", 'ª'), ("ordm", 'º'), ("oslash", 'ø'), ("otilde", 'õ'), ("ouml", 'ö'),
    ("para", '¶'), ("plusmn", '±'), ("pound", '£'), ("quot", '"'), ("raquo", '»'), ("reg", '®'), ("sect", '§'),
    ("shy", '\u{ad}'), ("sup1", '¹'), ("sup2", '²'), ("sup3", '³'), ("szlig", 'ß'), ("thorn", 'þ'), ("times", '×'),
    ("uacute", 'ú'), ("ucirc", 'û'), ("ugrave", 'ù'), ("uml", '¨'), ("uuml", 'ü'), ("yacute", 'ý'), ("yen", '¥'),
    ("yuml", 'ÿ'),
];

/// Les noms qui ne valent qu'avec leur point-virgule : les plus courants d'un courriel en francais.
const ENTITES_AVEC_POINT_VIRGULE: &[(&str, char)] = &[
    ("apos", '\''), ("euro", '€'), ("hellip", '…'), ("lsquo", '\u{2018}'), ("rsquo", '\u{2019}'), ("ldquo", '\u{201c}'),
    ("rdquo", '\u{201d}'), ("ndash", '\u{2013}'), ("mdash", '\u{2014}'), ("oelig", 'œ'), ("OElig", 'Œ'), ("bull", '•'),
    ("trade", '™'), ("larr", '←'), ("rarr", '→'), ("thinsp", '\u{2009}'),
];

fn entite_historique(nom: &str) -> Option<char> {
    ENTITES_HISTORIQUES.iter().find(|(n, _)| *n == nom).map(|(_, c)| *c)
}

/// Un nom d'entite suivi de son point-virgule.
fn entite_connue(nom: &str) -> Option<char> {
    entite_historique(nom).or_else(|| ENTITES_AVEC_POINT_VIRGULE.iter().find(|(n, _)| *n == nom).map(|(_, c)| *c))
}

/// `html._replace_charref` pour un nombre : le caractere qu'il vaut, ou rien (un caractere de controle ou un
/// non-caractere s'efface).
fn caractere_numerique(n: u32) -> Option<char> {
    const CP1252: [char; 32] = [
        '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{2c6}', '\u{2030}', '\u{160}',
        '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}', '\u{8f}', '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}',
        '\u{2013}', '\u{2014}', '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}', '\u{178}',
    ];
    match n {
        0 => Some('\u{fffd}'),
        0x0d => Some('\r'),
        0x80..=0x9f => Some(CP1252[(n - 0x80) as usize]),
        0xd800..=0xdfff => Some('\u{fffd}'),
        n if n > 0x10ffff => Some('\u{fffd}'),
        1..=8 | 0x0b | 0x0e..=0x1f | 0x7f | 0xfdd0..=0xfdef => None,
        n if n & 0xfffe == 0xfffe => None,
        // Ni substitut ni hors d'Unicode : un caractere.
        n => char::from_u32(n),
    }
}

/// Le texte HTML rendu en clair, pour ce qu'un courriel en francais porte (voir les tables) : `&nbsp;`, `&eacute;`, `&#233;`...
fn html_desechapper(t: &str) -> String {
    let c: Vec<char> = t.chars().collect();
    let mut sortie = String::with_capacity(t.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] != '&' {
            sortie.push(c[i]);
            i += 1;
            continue;
        }
        let suite = &c[i + 1..];
        // `#[0-9]+;?` et `#[xX][0-9a-fA-F]+;?`.
        if suite.first() == Some(&'#') {
            let hexa = matches!(suite.get(1), Some('x') | Some('X'));
            let debut = if hexa { 2 } else { 1 };
            let chiffres: String = suite[debut..].iter().take_while(|x| if hexa { x.is_ascii_hexdigit() } else { x.is_ascii_digit() }).collect();
            if !chiffres.is_empty() {
                let base = if hexa { 16 } else { 10 };
                let n = chiffres.chars().try_fold(0u32, |n, x| n.checked_mul(base)?.checked_add(x.to_digit(base)?));
                // Un nombre trop long pour un code est hors d'Unicode : le caractere de remplacement.
                let caractere = match n {
                    Some(n) => caractere_numerique(n),
                    None => Some('\u{fffd}'),
                };
                sortie.extend(caractere);
                let mut fin = debut + chiffres.chars().count();
                if suite.get(fin) == Some(&';') {
                    fin += 1;
                }
                i += 1 + fin;
                continue;
            }
        }
        // `[^\t\n\f <&#;]{1,32};?`.
        let nom: String = suite.iter().take_while(|x| !matches!(x, '\t' | '\n' | '\x0c' | ' ' | '<' | '&' | '#' | ';')).take(32).collect();
        if nom.is_empty() {
            sortie.push('&');
            i += 1;
            continue;
        }
        let long = nom.chars().count();
        let point_virgule = suite.get(long) == Some(&';');
        if point_virgule {
            if let Some(ch) = entite_connue(&nom) {
                sortie.push(ch);
                i += 1 + long + 1;
                continue;
            }
        } else if let Some(ch) = entite_historique(&nom) {
            sortie.push(ch);
            i += 1 + long;
            continue;
        }
        // Le plus long prefixe qui est un nom historique : `&eacutex` -> `éx`.
        let mut trouve = false;
        for x in (2..long).rev() {
            let prefixe: String = nom.chars().take(x).collect();
            if let Some(ch) = entite_historique(&prefixe) {
                sortie.push(ch);
                i += 1 + x;
                trouve = true;
                break;
            }
        }
        if !trouve {
            sortie.push('&');
            i += 1;
        }
    }
    sortie
}
