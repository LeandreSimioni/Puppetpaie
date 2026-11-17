//! Le texte : le blanc, les accents, la cle de recherche.

/// UNE PHRASE DITE AUX GENS : ce que le moteur compose pour qu'une personne le lise (la phrase que rend une route, un
/// conseil, un avis). Elle se compose par `socle::phrase!` (comme `format!`), ou d'un texte fixe (`fixe`) ; un nombre y entre par l'ecriture de sa grandeur (`euros`, `virgule`, `par_milliers`…). Elle s'ecrit en
/// texte, telle quelle (serde, `Display`), et ne se rouvre pas : une phrase en cite une autre par `Display`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Phrase(String);

impl Phrase {
    /// Une phrase ecrite telle quelle dans le moteur.
    pub fn fixe(t: &'static str) -> Phrase {
        Phrase(t.to_string())
    }

    /// Une phrase composee : la porte de `socle::phrase!`, qui seule l'appelle (`format_args!`).
    #[doc(hidden)]
    pub fn de_format(a: std::fmt::Arguments<'_>) -> Phrase {
        Phrase(a.to_string())
    }

    /// Ne dit-elle rien (vide, ou des blancs seuls) ?
    pub fn est_vide(&self) -> bool {
        self.0.trim().is_empty()
    }

    /// LES PHRASES `phrases` MISES BOUT A BOUT, separees par `separateur` : une phrase en suit une autre sans se rouvrir.
    pub fn jointes(phrases: impl IntoIterator<Item = Phrase>, separateur: &str) -> Phrase {
        Phrase(phrases.into_iter().map(|p| p.0).collect::<Vec<_>>().join(separateur))
    }
}

impl std::fmt::Display for Phrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl serde::Serialize for Phrase {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Phrase {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Phrase, D::Error> {
        <String as serde::Deserialize>::deserialize(d).map(Phrase)
    }
}

/// Le blanc d'Unicode (`char::is_whitespace`), plus `\x1c` a `\x1f` : ce qu'on retire autour d'une valeur lue et ce qui
/// separe les mots.
pub fn blanc(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | '\u{1c}'..='\u{20}' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

/// UNE SAISIE SANS SES BLANCS DE BORD, EN CAPITALES : la forme rangee d'un code que son texte ecrit en capitales (un code
/// pays ISO 3166, `pays::CodePays::rangee` ; un code INSEE de commune : 2A, 2B).
pub fn en_capitales(texte: &str) -> String {
    texte.trim_matches(blanc).to_uppercase()
}

/// Un chiffre decimal d'Unicode (`str.isdigit()` : la table `nfkd_ascii::CHIFFRES`).
pub fn est_chiffre(c: char) -> bool {
    dans_les_plages(crate::nfkd_ascii::CHIFFRES, c)
}

/// `c` est-il dans une des plages `[a, b]` de `table`, triee, ses plages disjointes ? La premiere plage qui ne finit pas
/// avant lui le contient, ou aucune.
pub fn dans_les_plages(table: &[(u32, u32)], c: char) -> bool {
    let n = c as u32;
    table.get(table.partition_point(|&(_, b)| b < n)).is_some_and(|&(a, _)| a <= n)
}

/// Le texte sans le blanc autour (`blanc`).
pub fn strip(s: &str) -> &str {
    s.trim_matches(blanc)
}

/// Les lettres accentuees du latin (Latin-1 et Latin etendu A) que
/// `unicodedata.normalize("NFD")` decompose en lettre de base + diacritique.
/// Pas de crate de normalisation : la table suffit aux noms de personnes.
const ACCENTS: [(&str, char); 38] = [
    ("àáâãäåāăą", 'a'),
    ("ÀÁÂÃÄÅĀĂĄ", 'A'),
    ("çćĉċč", 'c'),
    ("ÇĆĈĊČ", 'C'),
    ("ď", 'd'),
    ("Ď", 'D'),
    ("èéêëēĕėęě", 'e'),
    ("ÈÉÊËĒĔĖĘĚ", 'E'),
    ("ĝğġģ", 'g'),
    ("ĜĞĠĢ", 'G'),
    ("ĥ", 'h'),
    ("Ĥ", 'H'),
    ("ìíîïĩīĭį", 'i'),
    ("ÌÍÎÏĨĪĬĮİ", 'I'),
    ("ĵ", 'j'),
    ("Ĵ", 'J'),
    ("ķ", 'k'),
    ("Ķ", 'K'),
    ("ĺļľ", 'l'),
    ("ĹĻĽ", 'L'),
    ("ñńņň", 'n'),
    ("ÑŃŅŇ", 'N'),
    ("òóôõöōŏő", 'o'),
    ("ÒÓÔÕÖŌŎŐ", 'O'),
    ("ŕŗř", 'r'),
    ("ŔŖŘ", 'R'),
    ("śŝşš", 's'),
    ("ŚŜŞŠ", 'S'),
    ("ţť", 't'),
    ("ŢŤ", 'T'),
    ("ùúûüũūŭůűų", 'u'),
    ("ÙÚÛÜŨŪŬŮŰŲ", 'U'),
    ("ŵ", 'w'),
    ("Ŵ", 'W'),
    ("ŷÿý", 'y'),
    ("ŶŸÝ", 'Y'),
    ("źżž", 'z'),
    ("ŹŻŽ", 'Z'),
];

/// La lettre de base d'un caractere accentue (NFD sans les marques), ou lui-meme.
fn sans_diacritique(c: char) -> char {
    match ACCENTS.iter().find(|(lettres, _)| lettres.contains(c)) {
        Some((_, base)) => *base,
        None => c,
    }
}

/// Sans les accents (NFD, marques retirees). Les lettres
/// hors du latin courant sont gardees telles quelles.
pub fn sans_accent(s: &str) -> String {
    s.chars().filter(|c| !('\u{300}'..='\u{36f}').contains(c)).map(sans_diacritique).collect()
}

/// La cle de recherche : minuscules, sans accent, sans ponctuation, espaces simples.
pub fn cle_recherche(s: &str) -> String {
    let bas: String = sans_accent(s).chars().flat_map(char::to_lowercase).collect();
    let net: String = bas.chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    net.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Le sigle : « Scène Ensemble » -> « SCENE ENSEMBLE » (NFKD, ASCII seul, majuscules).
pub fn sigle(s: &str) -> String {
    let ascii: String = sans_accent(s).chars().filter(char::is_ascii).collect();
    ascii.to_uppercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Les lignes d'un texte, coupées à toute fin de ligne d'Unicode (le numéro qu'on affiche les compte ainsi) : `\n`,
/// `\r`, `\r\n`, et les séparateurs `\x0b`, `\x0c`, `\x1c`-`\x1e`, `\u{85}`, `\u{2028}`, `\u{2029}` ; une fin de ligne
/// finale ne fait pas de ligne vide. La seule copie du moteur (DSN, retours, DILA, garde avant commit).
pub fn lignes_du_texte(t: &str) -> Vec<&str> {
    let mut v = Vec::new();
    let mut debut = 0;
    let mut it = t.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}') {
            v.push(&t[debut..i]);
            debut = i + c.len_utf8();
            if c == '\r' && it.peek().is_some_and(|&(_, d)| d == '\n') {
                it.next();
                debut += 1;
            }
        }
    }
    if debut < t.len() {
        v.push(&t[debut..]);
    }
    v
}

/// Des octets lus en Latin-1 (ISO 8859-1) : chaque octet est le caractere de meme code (la DSN, les retours).
pub fn texte_latin1(o: &[u8]) -> String {
    o.iter().map(|&b| b as char).collect()
}


/// `unicodedata.normalize("NFKD", v).encode("ascii", "ignore").decode("ascii")`.
pub fn ascii_nfkd(v: &str) -> String {
    let mut ascii = String::new();
    for c in v.chars() {
        if c.is_ascii() {
            ascii.push(c);
            continue;
        }
        let cp = c as u32;
        // La plage du caractere (la table est triee, ses plages disjointes : la premiere qui ne finit pas avant lui, si elle
        // le contient). Hors de la table, le caractere n'a pas d'ASCII : il tombe (`encode("ascii", "ignore")`).
        let table = &crate::nfkd_ascii::TABLE;
        let (a, t, avance) = match table.get(table.partition_point(|(_, b, _, _)| *b < cp)) {
            Some(&(a, _, t, avance)) if a <= cp => (a, t, avance),
            _ => continue,
        };
        if avance {
            // Une plage qui avance (`a` a `b` vers `t`, `t+1`...) : le caractere de meme rang depuis le premier de `t`.
            let decale = t.chars().next().and_then(|base| char::from_u32(base as u32 + (cp - a)));
            ascii.push(match decale {
                Some(c) => c,
                None => ' ',
            });
        } else {
            ascii.push_str(t);
        }
    }
    ascii
}
/// La première lettre en capitale, le reste en minuscules.
pub fn capitaliser(t: &str) -> String {
    let mut c = t.chars();
    let tete: String = c.next().into_iter().flat_map(char::to_uppercase).collect();
    tete + &c.as_str().to_lowercase()
}

/// « de » devant `mot`, elide devant une voyelle : « de mars 2026 », « d'avril 2026 », « d'absence non rémunérée ».
pub fn de(mot: &str) -> String {
    let voyelle = mot.chars().next().is_some_and(|c| "aàâeéèêëiîïoôuùûyAÀÂEÉÈÊIÎOÔUÛY".contains(c));
    format!("{}{}", if voyelle { "d'" } else { "de " }, mot)
}

/// LE SALARIE TEL QUE L'ECRAN LE NOMME, partout ou le moteur parle de lui (refus, avis, listes, phrases d'un envoi) :
/// « NOM Prénom » comme la page le montre : le nom avec ses accents (`nom_affichage`, sinon `nom`) en capitales, puis le
/// prénom d'usage (`prenom_affichage`, sinon `prenom`). `None` si la fiche ne porte ni nom ni prénom.
pub fn salarie_nomme(nom: Option<&str>, nom_affichage: Option<&str>, prenom: Option<&str>, prenom_affichage: Option<&str>) -> Option<String> {
    fn l_un_ou_l_autre<'a>(usage: Option<&'a str>, etat_civil: Option<&'a str>) -> Option<&'a str> {
        [usage, etat_civil].into_iter().flatten().map(strip).find(|t| !t.is_empty())
    }
    let parts: Vec<String> = [l_un_ou_l_autre(nom_affichage, nom).map(str::to_uppercase), l_un_ou_l_autre(prenom_affichage, prenom).map(str::to_string)]
        .into_iter()
        .flatten()
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// UN MONTANT TEL QU'ON LE LIT, et le seul endroit qui en écrit un (une phrase du moteur, une ligne d'écran, un
/// document imprimé) : « 1 234,50 € », au centime (`Montant::au_centime`), la virgule décimale, les milliers et l'euro
/// séparés par une espace insécable (le montant ne se coupe pas en fin de ligne ; U+00A0, que toute police d'un PDF
/// porte). Un fichier déclaré (DSN, bordereau, format d'un organisme) écrit ses montants dans la forme que sa norme impose.
pub fn euros<E: EnEuros>(x: E) -> String {
    // Le signe est celui du montant au centime : ce qui s'arrondit a zero n'en a plus.
    let c = au_centime(x.decimal_ecrit());
    format!("{}\u{a0}€", aux_centimes(c.is_sign_negative(), c.abs(), '\u{a0}'))
}

/// UN MONTANT TEL QUE LE BULLETIN ET LES DOCUMENTS DU CONTRAT L'IMPRIMENT, dans leurs colonnes et leurs marqueurs :
/// « 1 234,50 », au centime (`Montant::au_centime`), la virgule décimale, les milliers séparés par une espace ordinaire,
/// sans « € » ; le signe est celui de l'original (`-0,00` pour un retrait qui s'arrondit à zéro).
pub fn chiffres_en_euros<E: EnEuros>(x: E) -> String {
    chiffres(x.decimal_ecrit())
}

/// DES HEURES DANS UNE COLONNE DE MONTANTS DU BULLETIN (la base d'une ligne de cotisation qui compte des heures), écrites
/// comme `chiffres_en_euros` : « 1 234,50 », au centième, sans unité.
pub fn chiffres_d_heures(h: crate::grandeurs::Heures) -> String {
    chiffres(h.decimal())
}

/// UN TAUX DANS UNE COLONNE DE MONTANTS D'UN BULLETIN (le taux personnalisé du prélèvement à la source), écrit comme
/// `chiffres_en_euros` : « 7,50 », au centième, sans « % ».
pub fn chiffres_de_taux(t: crate::grandeurs::Taux) -> String {
    chiffres(t.decimal())
}

/// UN TAUX TEL QUE LE BULLETIN L'IMPRIME dans sa colonne de taux : trois décimales écrites, la moitié au pair (aucun
/// texte déposé ne dit l'arrondi d'un taux imprimé), la virgule décimale, le signe de l'original (« 9,100 », « -0,500 »).
pub fn taux_au_millieme(t: crate::grandeurs::Taux) -> String {
    let x = t.decimal();
    let mut r = x.abs().round_dp_with_strategy(3, rust_decimal::RoundingStrategy::MidpointNearestEven);
    r.rescale(3);
    format!("{}{}", if x.is_sign_negative() { "-" } else { "" }, r.to_string().replace('.', ","))
}

/// L'écriture de `chiffres_en_euros` et de `chiffres_d_heures` : au centième, le signe de l'original, les milliers
/// séparés par une espace ordinaire.
fn chiffres(d: rust_decimal::Decimal) -> String {
    aux_centimes(d.is_sign_negative(), au_centime(d.abs()), ' ')
}

/// Le decimal `d` au centime, par l'arrondi d'un montant (`Montant::au_centime`), ses deux decimales ecrites.
fn au_centime(d: rust_decimal::Decimal) -> rust_decimal::Decimal {
    crate::grandeurs::Montant::du_decimal(d).au_centime().decimal()
}

/// Les chiffres d'un montant au centime `c`, positif, precedes de « - » si `negatif`, les milliers separes par
/// `separateur`, la virgule decimale : l'ecriture de `euros` et de `chiffres_en_euros`.
fn aux_centimes(negatif: bool, c: rust_decimal::Decimal, separateur: char) -> String {
    let t = c.to_string();
    let (entier, centimes) = match t.split_once('.') {
        Some(e_c) => e_c,
        None => (t.as_str(), "00"),
    };
    format!("{}{},{}", if negatif { "-" } else { "" }, milliers(entier, separateur), centimes)
}

/// CE QUI SE DIT EN EUROS (`euros`, `chiffres_en_euros`) : un montant, ou un montant unitaire (le SMIC horaire, le
/// plafond horaire, le taux d'un cachet), et rien d'autre ; scelle (`scelle::EnEuros` ne se nomme pas dehors).
pub trait EnEuros: scelle::EnEuros {}

mod scelle {
    /// Son decimal, que seuls `euros` et `chiffres_en_euros` ecrivent.
    pub trait EnEuros {
        fn decimal_ecrit(self) -> rust_decimal::Decimal;
    }

    impl EnEuros for crate::grandeurs::Montant {
        fn decimal_ecrit(self) -> rust_decimal::Decimal {
            self.decimal()
        }
    }

    impl EnEuros for crate::grandeurs::MontantUnitaire {
        fn decimal_ecrit(self) -> rust_decimal::Decimal {
            self.decimal()
        }
    }
}

impl EnEuros for crate::grandeurs::Montant {}
impl EnEuros for crate::grandeurs::MontantUnitaire {}

/// UN POURCENTAGE TEL QU'ON LE LIT, et le seul endroit qui en écrit un dans une phrase : « 1,50 % », à deux décimales (la
/// moitié au pair), la virgule décimale.
pub fn pourcent(t: crate::grandeurs::Taux) -> String {
    let mut t = t.decimal().round_dp(2);
    t.rescale(2);
    format!("{} %", t.to_string().replace('.', ","))
}

/// Des chiffres, les milliers separes par `separateur` (« 1 500 »).
fn milliers(entier: &str, separateur: char) -> String {
    let mut dit = String::new();
    for (i, c) in entier.chars().enumerate() {
        if i > 0 && (entier.len() - i) % 3 == 0 {
            dit.push(separateur);
        }
        dit.push(c);
    }
    dit
}

/// UNE GRANDEUR TELLE QU'ON LA LIT dans une phrase (des heures, des jours, un effectif, un taux) : la virgule décimale,
/// sans zéro inutile (« 151,67 », « 8 », « 2,5 »).
pub fn virgule<G: crate::grandeurs::Grandeur>(g: G) -> String {
    nombre_dit(g.en_decimal())
}

/// DES HEURES DITES dans une phrase, au centieme, la moitie au pair (`Heures::a_deux_decimales`), sans zero inutile :
/// « 151,67 h », « 11 h ».
pub fn heures(h: crate::grandeurs::Heures) -> String {
    format!("{} h", virgule(h.a_deux_decimales()))
}

/// DES HEURES DITES EN HEURES ET MINUTES dans une phrase (151,67 h : « 151 h 40 »), a la minute la plus proche, la
/// moitie vers le haut ; des heures entieres, sans minutes (« 35 h »).
pub fn heures_et_minutes(h: crate::grandeurs::Heures) -> String {
    use rust_decimal::{Decimal, RoundingStrategy};
    let soixante = Decimal::from(60);
    let minutes = (h.decimal() * soixante).round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    let heures = (minutes / soixante).trunc();
    let reste = minutes - heures * soixante;
    match reste.is_zero() {
        true => format!("{} h", heures.normalize()),
        false => format!("{} h {:0>2}", heures.normalize(), reste.normalize().to_string()),
    }
}

/// UNE QUANTITE SE DIT AU PLURIEL dès deux (« 1,5 journée », « 2 cachets »).
pub fn au_pluriel<G: crate::grandeurs::Grandeur>(g: G) -> bool {
    g.en_decimal() >= rust_decimal::Decimal::TWO
}

/// UN NOMBRE TEL QU'ON LE LIT dans une phrase, et le seul endroit qui l'écrit ainsi : la virgule décimale, sans zéro
/// inutile (« 151,67 », « 8 », « 2,5 »).
pub fn nombre_dit(x: rust_decimal::Decimal) -> String {
    x.normalize().to_string().replace('.', ",")
}

/// UNE GRANDEUR TELLE QU'ELLE S'ECRIT, LA VIRGULE DECIMALE, son échelle gardée (`ecrit::texte` : « 12,50 », « 1,5 ») :
/// un taux, un abattement cité dans une phrase, la case d'un état ou d'un CSV à la française.
pub fn a_virgule<G: crate::grandeurs::Grandeur>(g: G) -> String {
    crate::grandeurs::ecrit::texte::dit(g).replace('.', ",")
}

/// UNE GRANDEUR CITÉE PAR UN CONSEIL (« 1 234,5 », « 15,5 ») : au centième, la moitié loin de zéro, sans zéro inutile, les
/// milliers séparés par une espace insécable, la virgule décimale. Son arrondi est celui d'un affichage.
pub fn par_milliers<G: crate::grandeurs::Grandeur>(g: G) -> String {
    nombre_par_milliers(g.en_decimal(), None)
}

/// La même, `decimales` décimales écrites (« 0,90 », « 60 ») : un taux, un coefficient qu'un conseil cite (un montant
/// s'écrit par `euros`).
pub fn par_milliers_a<G: crate::grandeurs::Grandeur>(g: G, decimales: u32) -> String {
    nombre_par_milliers(g.en_decimal(), Some(decimales))
}

/// UN COMPTE DIT DANS UNE PHRASE (« 3 », « 1 200 ») : les milliers séparés par une espace insécable, comme
/// `par_milliers`.
pub fn compte(n: usize) -> String {
    milliers(&n.to_string(), '\u{a0}')
}

/// L'écriture de `par_milliers` et `par_milliers_a` : `decimales` fixe, sinon au centième et les zéros de queue tombent.
fn nombre_par_milliers(x: rust_decimal::Decimal, decimales: Option<u32>) -> String {
    let x = match decimales {
        Some(d) => x.round_dp_with_strategy(d, rust_decimal::RoundingStrategy::MidpointAwayFromZero),
        None => x.round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero).normalize(),
    };
    let s = match decimales {
        Some(d) => format!("{:.*}", d as usize, x),
        None => x.to_string(),
    };
    let (signe, s) = match s.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", s.as_str()),
    };
    match s.split_once('.') {
        Some((e, f)) if !f.is_empty() => format!("{}{},{}", signe, milliers(e, '\u{a0}'), f),
        Some((e, _)) => format!("{}{}", signe, milliers(e, '\u{a0}')),
        None => format!("{}{}", signe, milliers(s, '\u{a0}')),
    }
}

/// Le même, les milliers séparés par une espace insécable, dans une phrase qui cite une valeur d'un fichier (« 1 500 »,
/// « 12,5 ») : `forme::Court::dit`.
pub(crate) fn nombre_dit_par_milliers(x: rust_decimal::Decimal) -> String {
    let t = nombre_dit(x);
    let (signe, t) = match t.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", t.as_str()),
    };
    let (entier, decimales) = match t.split_once(',') {
        Some((e, d)) => (e, format!(",{}", d)),
        None => (t, String::new()),
    };
    format!("{}{}{}", signe, milliers(entier, '\u{a0}'), decimales)
}

