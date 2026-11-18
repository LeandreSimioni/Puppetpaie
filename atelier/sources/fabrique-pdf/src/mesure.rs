//! UNE MESURE D'UN PDF, en nombre exact : une position, une largeur, une taille de lettre en points PDF (1/72 de
//! pouce), ou un rapport sans unité (un interligne, une fraction de largeur, un coefficient de matrice), en millionièmes
//! dans un entier. C'est le seul nombre à virgule de la fabrique et de la lecture des PDF : aucun flottant.
//!
//! Un PDF écrit ses nombres en texte (`12.5 0 Td`) : une mesure s'écrit (`Display`, `{:.4}` compris) et se lit
//! (`Mesure::lue`, `FromStr`) en texte, sans passer par un flottant. Un produit ou un quotient s'arrondit au millionième
//! le plus proche, la moitié loin de zéro : une position peut différer d'une fraction de millième de point, invisible.
//!
//! Les bibliothèques qui la rendent en flottant (la lecture, `lopdf` ; le dessin d'un SVG, `usvg`) la donnent par
//! son texte : `Mesure::de_l_objet`, `Mesure::par_son_texte`.

use std::fmt;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// Un millionième : l'échelle d'une `Mesure`.
const UN: i64 = 1_000_000;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mesure(i64);

/// Une mesure écrite dans le code, en décimal : `mesure!("7.5")`, `mesure!("-0.25")` (un signe, des chiffres, au plus
/// six décimales), lue à la compilation (`MesureEcrite::MESURE`) : une écriture fausse ne compile pas.
#[macro_export]
macro_rules! mesure {
    ($texte:literal) => {{
        enum Ecrite {}
        impl $crate::mesure::MesureEcrite for Ecrite {
            const TEXTE: &'static str = $texte;
        }
        const LUE: $crate::mesure::Mesure = <Ecrite as $crate::mesure::MesureEcrite>::MESURE;
        LUE
    }};
}

/// Le texte d'une mesure écrite dans le code (`mesure!`), qu'une constante porte : la mesure se lit à la compilation, et
/// aucun texte lu à l'exécution ne devient une mesure par ce chemin.
pub trait MesureEcrite {
    const TEXTE: &'static str;
    const MESURE: Mesure = ecrite(Self::TEXTE);
}

const fn ecrite(t: &str) -> Mesure {
    let o = t.as_bytes();
    let (negatif, mut i) = if !o.is_empty() && o[0] == b'-' { (true, 1) } else { (false, 0) };
    let mut v: i64 = 0;
    let mut decimales: i32 = -1;
    let mut chiffres = 0;
    while i < o.len() {
        let c = o[i];
        if c == b'.' && decimales < 0 {
            decimales = 0;
        } else if c >= b'0' && c <= b'9' {
            v = v * 10 + (c - b'0') as i64;
            chiffres += 1;
            if decimales >= 0 {
                decimales += 1;
            }
        } else {
            panic!("une mesure s'écrit en décimal");
        }
        i += 1;
    }
    if chiffres == 0 || decimales > 6 {
        panic!("une mesure s'écrit en décimal, six décimales au plus");
    }
    let mut d = if decimales < 0 { 0 } else { decimales };
    while d < 6 {
        v *= 10;
        d += 1;
    }
    Mesure(if negatif { -v } else { v })
}

/// `n / d`, arrondi au plus proche, la moitié loin de zéro (`d` non nul).
fn quotient_arrondi(n: i128, d: i128) -> i128 {
    let (q, r) = (n / d, n % d);
    if 2 * r.abs() >= d.abs() {
        q + if (n < 0) == (d < 0) { 1 } else { -1 }
    } else {
        q
    }
}

/// `quotient_arrondi` en 64 bits : le même quotient, ou `None` quand un pas du calcul déborde d'un `i64` (`n / d` pour
/// `i64::MIN / -1`, l'arrondi qui passe `i64::MAX`), ou que `d` est nul. `2|r| >= |d|` s'écrit `|r| >= |d| - |r|` en non
/// signé : `|r| < |d|`, la différence ne déborde pas.
fn quotient_arrondi_64(n: i64, d: i64) -> Option<i64> {
    let (q, r) = (n.checked_div(d)?, n.checked_rem(d)?);
    if r.unsigned_abs() >= d.unsigned_abs() - r.unsigned_abs() {
        q.checked_add(if (n < 0) == (d < 0) { 1 } else { -1 })
    } else {
        Some(q)
    }
}

/// `a × b / d`, arrondi au plus proche, la moitié loin de zéro : en 64 bits quand le produit et le quotient y tiennent,
/// sinon en 128 bits ; le même nombre dans les deux cas (la division tronquée et son reste sont ceux des 128 bits quand
/// les opérandes tiennent en 64).
fn produit_sur(a: i64, b: i64, d: i64) -> i128 {
    match a.checked_mul(b).and_then(|p| quotient_arrondi_64(p, d)) {
        Some(q) => q as i128,
        None => quotient_arrondi(a as i128 * b as i128, d as i128),
    }
}

/// Un `i128` ramené à une mesure, borné aux mesures possibles.
fn borne(n: i128) -> Mesure {
    Mesure(n.clamp(i64::MIN as i128, i64::MAX as i128) as i64)
}

impl Mesure {
    pub const ZERO: Mesure = Mesure(0);
    pub const UN: Mesure = Mesure(UN);
    pub const MAX: Mesure = Mesure(i64::MAX);
    pub const MIN: Mesure = Mesure(i64::MIN);

    /// `n` points (ou `n`, un rapport) : un entier de la norme PDF ou d'une police, dans la fabrique ; hors d'elle, une
    /// mesure se dit par son texte (`mesure!("40")`).
    pub(crate) const fn entier(n: i64) -> Mesure {
        Mesure(n * UN)
    }

    /// `n` millièmes : `Mesure::milliemes(595_280)` vaut 595,28.
    pub(crate) const fn milliemes(n: i64) -> Mesure {
        Mesure(n * 1000)
    }

    /// `n` millionièmes.
    pub(crate) const fn millioniemes(n: i64) -> Mesure {
        Mesure(n)
    }

    /// Le nombre de millionièmes.
    pub(crate) const fn en_millioniemes(self) -> i64 {
        self.0
    }

    /// Le nombre entier le plus proche, la moitié loin de zéro.
    pub(crate) fn arrondi(self) -> i64 {
        produit_sur(self.0, 1, UN) as i64
    }

    /// Le plus petit entier qui n'est pas en dessous.
    pub(crate) fn plafond(self) -> i64 {
        self.0.div_euclid(UN) + if self.0.rem_euclid(UN) == 0 { 0 } else { 1 }
    }

    /// La partie entière, vers zéro.
    pub(crate) fn tronque(self) -> i64 {
        self.0 / UN
    }
    /// Arrondie à `chiffres` décimales (au plus six), la moitié loin de zéro.
    pub fn arrondie(self, chiffres: u32) -> Mesure {
        let pas = 10i64.pow(6 - chiffres.min(6));
        Mesure((produit_sur(self.0, 1, pas) * pas as i128) as i64)
    }

    /// A six chiffres significatifs, la moitié au pair (`12.345678` : `12.3457`) : une coordonnée d'un dessin SVG,
    /// écrite ensuite sans zéro inutile (`Display`).
    pub fn a_six_chiffres(self) -> Mesure {
        let a = self.0.unsigned_abs();
        let chiffres = if a == 0 { 1 } else { a.ilog10() + 1 };
        if chiffres <= 6 {
            return self;
        }
        let pas = 10u64.pow(chiffres - 6);
        let (q, r) = (a / pas, a % pas);
        let q = if 2 * r > pas || (2 * r == pas && q % 2 == 1) { q + 1 } else { q };
        let v = (q * pas) as i64;
        Mesure(if self.0 < 0 { -v } else { v })
    }

    pub fn abs(self) -> Mesure {
        Mesure(self.0.saturating_abs())
    }

    pub fn est_nulle(self) -> bool {
        self.0 == 0
    }

    /// `self` sur `autre`, arrondi au millionième ; `None` si `autre` est nul.
    pub fn sur(self, autre: Mesure) -> Option<Mesure> {
        (autre.0 != 0).then(|| borne(produit_sur(self.0, UN, autre.0)))
    }

    /// La part de `self` sur un compte de `n` (une moyenne, une colonne parmi `n`), arrondie au millionième ; `None` si
    /// `n` est nul.
    pub fn part_sur(self, n: usize) -> Option<Mesure> {
        (n != 0).then(|| {
            borne(match i64::try_from(n) {
                Ok(n) => produit_sur(self.0, 1, n),
                Err(_) => quotient_arrondi(self.0 as i128, n as i128),
            })
        })
    }

    /// `self` fois le rapport de deux mesures, `n / d`, en un seul arrondi ; `None` si `d` est nulle.
    pub fn fois_rapport(self, n: Mesure, d: Mesure) -> Option<Mesure> {
        (d.0 != 0).then(|| borne(produit_sur(self.0, n.0, d.0)))
    }

    /// `self` fois l'entier `n` (de la norme PDF, d'une police), bornée aux mesures possibles.
    pub(crate) fn fois_entier(self, n: i64) -> Mesure {
        match self.0.checked_mul(n) {
            Some(p) => Mesure(p),
            None => borne(self.0 as i128 * n as i128),
        }
    }

    /// Le quotient par l'entier non nul `n` (de la norme PDF, d'une police), arrondi au millionième.
    pub(crate) fn divisee_par(self, n: i64) -> Mesure {
        borne(produit_sur(self.0, 1, n))
    }

    /// `self` fois `n / d` en un seul arrondi, `d` non nul par son type.
    pub(crate) fn fois(self, n: i64, d: std::num::NonZeroI64) -> Mesure {
        borne(produit_sur(self.0, n, d.get()))
    }

    /// La racine carrée (d'une mesure positive ou nulle ; une mesure négative n'en a pas : `None`), arrondie au
    /// millionième le plus proche.
    pub fn racine(self) -> Option<Mesure> {
        if self.0 < 0 {
            return None;
        }
        // La racine de `self.0 / UN`, en millionièmes : la racine entière de `self.0 * UN`.
        if let Some(n) = (self.0 as u64).checked_mul(UN as u64) {
            return Some(Mesure(racine_arrondie_64(n) as i64));
        }
        let n = self.0 as u128 * UN as u128;
        let mut r = racine_entiere(n);
        // `r` est la racine par défaut ; on arrondit au plus proche : r + 1/2 au carré, soit r² + r + 1/4, comparé à n.
        if n > r * r + r {
            r += 1;
        }
        Some(Mesure(r as i64))
    }

    /// Un nombre d'un objet PDF lu par `lopdf` : un entier, ou un réel, que `lopdf` garde en flottant et qui repasse
    /// par son texte (sa forme la plus courte, celle que le PDF écrit). `None` pour un autre objet.
    pub fn de_l_objet(o: &lopdf::Object) -> Option<Mesure> {
        match o {
            lopdf::Object::Integer(i) => Some(Mesure(i.checked_mul(UN)?)),
            lopdf::Object::Real(r) => Mesure::lue(&r.to_string()),
            _ => None,
        }
    }

    /// Un nombre qu'une bibliothèque rend en flottant (le dessin d'un SVG, `usvg`), lu par son texte (sa forme la
    /// plus courte) ; `None` s'il sort des mesures possibles.
    pub fn par_son_texte(x: impl fmt::Display) -> Option<Mesure> {
        Mesure::lue(&x.to_string())
    }

    /// L'objet PDF qui l'écrit, pour `lopdf` : un entier s'il l'est, sinon un réel, que `lopdf` garde en flottant et
    /// qui se lit depuis son texte.
    pub fn en_objet(self) -> Result<lopdf::Object, crate::description::Refus> {
        if self.0 % UN == 0 {
            Ok(lopdf::Object::Integer(self.0 / UN))
        } else {
            let texte = self.to_string();
            texte
                .parse()
                .map(lopdf::Object::Real)
                .map_err(|e| crate::description::Refus::defaut(format!("la mesure {} ne s'écrit pas en réel PDF ({})", texte, e)))
        }
    }

    /// La longueur du vecteur (`self`, `autre`) : la racine de la somme des carrés, au millionième le plus proche.
    pub fn hypot(self, autre: Mesure) -> Mesure {
        let (a, b) = (self.0.unsigned_abs(), autre.0.unsigned_abs());
        if let Some(n) = a.checked_mul(a).zip(b.checked_mul(b)).and_then(|(a2, b2)| a2.checked_add(b2)) {
            return Mesure(racine_arrondie_64(n).min(i64::MAX as u64) as i64);
        }
        let (a, b) = (a as u128, b as u128);
        let n = a * a + b * b;
        let mut r = racine_entiere(n);
        if n > r * r + r {
            r += 1;
        }
        Mesure(r.min(i64::MAX as u128) as i64)
    }

    /// Lit un nombre écrit en texte : un signe facultatif, des chiffres, une virgule (`.`) facultative, et un exposant
    /// facultatif (`1e-7`, qu'un JSON peut écrire). Au-delà du millionième, il s'arrondit au plus proche, la moitié loin
    /// de zéro. `None` si le texte n'est pas un nombre, ou sort des mesures possibles.
    pub fn lue(texte: &str) -> Option<Mesure> {
        let t = texte.trim();
        let (negatif, t) = match t.as_bytes().first()? {
            b'-' => (true, &t[1..]),
            b'+' => (false, &t[1..]),
            _ => (false, t),
        };
        let (mantisse, exposant) = match t.find(['e', 'E']) {
            Some(i) => (&t[..i], exposant(&t[i + 1..])?),
            None => (t, 0),
        };
        let (entiere, fraction) = match mantisse.find('.') {
            Some(i) => (&mantisse[..i], &mantisse[i + 1..]),
            None => (mantisse, ""),
        };
        if entiere.is_empty() && fraction.is_empty() {
            return None;
        }
        if !entiere.bytes().chain(fraction.bytes()).all(|b| b.is_ascii_digit()) {
            return None;
        }
        // Les chiffres, sans les zéros de tête, et la position de la virgule après `exposant`.
        let chiffres: String = entiere.chars().chain(fraction.chars()).collect();
        let chiffres = chiffres.trim_start_matches('0');
        let decimales = fraction.len() as i64 - exposant as i64;
        if chiffres.is_empty() {
            return Some(Mesure::ZERO);
        }
        // La valeur vaut `chiffres × 10^(6 - decimales)` millionièmes.
        let decale = 6 - decimales;
        let valeur: i128 = if decale >= 0 {
            if chiffres.len() as i64 + decale > 30 {
                return None;
            }
            entier_de(chiffres) * 10i128.pow(decale as u32)
        } else {
            // On garde les chiffres jusqu'au millionième, plus un pour l'arrondi.
            let garde = chiffres.len() as i64 + decale;
            if garde < 0 {
                return Some(Mesure::ZERO);
            }
            if garde > 30 {
                return None;
            }
            let (tete, reste) = chiffres.split_at(garde as usize);
            entier_de(tete) + if reste.as_bytes()[0] >= b'5' { 1 } else { 0 }
        };
        let valeur = if negatif { -valeur } else { valeur };
        if valeur > i64::MAX as i128 || valeur < i64::MIN as i128 {
            return None;
        }
        Some(Mesure(valeur as i64))
    }
}

/// Des chiffres décimaux (vérifiés, 30 au plus : ils tiennent dans un i128) en entier ; aucun chiffre vaut 0.
fn entier_de(chiffres: &str) -> i128 {
    chiffres.bytes().fold(0, |n, c| n * 10 + (c - b'0') as i128)
}

/// L'exposant d'un nombre écrit (`-7` de `1e-7`) : un signe facultatif et des chiffres ; `None` s'il n'en est pas un,
/// ou sort de ce qu'un nombre de mesure peut porter.
fn exposant(t: &str) -> Option<i32> {
    let (negatif, chiffres) = match t.as_bytes().first()? {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    if chiffres.is_empty() || chiffres.len() > 9 || !chiffres.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = entier_de(chiffres) as i32;
    Some(if negatif { -n } else { n })
}

/// La racine carrée de `n`, arrondie à l'entier le plus proche, quand `n` tient en 64 bits : `isqrt` rend la racine par
/// défaut, la même que `racine_entiere` ; `r ≤ 2³² - 1`, donc `r² + r` ne déborde pas.
fn racine_arrondie_64(n: u64) -> u64 {
    let r = n.isqrt();
    if n > r * r + r {
        r + 1
    } else {
        r
    }
}

/// La racine carrée entière de `n` (par défaut), par la méthode de Newton, partie d'au-dessus de la racine.
fn racine_entiere(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = 1u128 << ((128 - n.leading_zeros()) / 2 + 1).min(127);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

impl std::str::FromStr for Mesure {
    type Err = ();
    fn from_str(s: &str) -> Result<Mesure, ()> {
        Mesure::lue(s).ok_or(())
    }
}

/// Écrite en décimal : sa forme la plus courte (`12`, `12.5`, `-0.25`) ; avec une précision (`{:.4}`), arrondie à ce
/// nombre de décimales (au plus six) et écrite avec toutes.
impl fmt::Display for Mesure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (v, chiffres) = match f.precision() {
            Some(p) => (self.arrondie(p as u32), Some(p.min(6))),
            None => (*self, None),
        };
        let signe = if v.0 < 0 { "-" } else { "" };
        let a = v.0.unsigned_abs();
        let (entiere, fraction) = (a / UN as u64, a % UN as u64);
        let fraction = format!("{:06}", fraction);
        let fraction = match chiffres {
            Some(p) => fraction[..p].to_string(),
            None => fraction.trim_end_matches('0').to_string(),
        };
        if fraction.is_empty() {
            write!(f, "{}{}", signe, entiere)
        } else {
            write!(f, "{}{}.{}", signe, entiere, fraction)
        }
    }
}

impl fmt::Debug for Mesure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Add for Mesure {
    type Output = Mesure;
    fn add(self, o: Mesure) -> Mesure {
        Mesure(self.0.saturating_add(o.0))
    }
}

impl Sub for Mesure {
    type Output = Mesure;
    fn sub(self, o: Mesure) -> Mesure {
        Mesure(self.0.saturating_sub(o.0))
    }
}

impl AddAssign for Mesure {
    fn add_assign(&mut self, o: Mesure) {
        *self = *self + o;
    }
}

impl SubAssign for Mesure {
    fn sub_assign(&mut self, o: Mesure) {
        *self = *self - o;
    }
}

impl Neg for Mesure {
    type Output = Mesure;
    fn neg(self) -> Mesure {
        Mesure(self.0.saturating_neg())
    }
}

/// Le produit de deux mesures (une longueur par un rapport, deux coefficients de matrice), arrondi au millionième.
impl Mul for Mesure {
    type Output = Mesure;
    fn mul(self, o: Mesure) -> Mesure {
        borne(produit_sur(self.0, o.0, UN))
    }
}

/// Un compte de fois une mesure (des lignes, des colonnes, des caractères : `n * hauteur`) : ce qu'un appelant compte ne
/// passe pas en entier signé.
impl Mul<Mesure> for usize {
    type Output = Mesure;
    fn mul(self, m: Mesure) -> Mesure {
        match i64::try_from(self).ok().and_then(|n| m.0.checked_mul(n)) {
            Some(p) => Mesure(p),
            None => borne(m.0 as i128 * self as i128),
        }
    }
}

impl std::iter::Sum for Mesure {
    fn sum<I: Iterator<Item = Mesure>>(it: I) -> Mesure {
        it.fold(Mesure::ZERO, |a, b| a + b)
    }
}

impl<'a> std::iter::Sum<&'a Mesure> for Mesure {
    fn sum<I: Iterator<Item = &'a Mesure>>(it: I) -> Mesure {
        it.fold(Mesure::ZERO, |a, b| a + *b)
    }
}

/// Un nombre d'un JSON (une description, FORMAT.md) : lu par son texte, celui que le lecteur JSON redonne
/// (`serde_json::Number`, sa forme la plus courte), jamais par un flottant de notre code.
impl<'de> serde::Deserialize<'de> for Mesure {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Mesure, D::Error> {
        let n = serde_json::Number::deserialize(d)?;
        Mesure::lue(&n.to_string()).ok_or_else(|| serde::de::Error::custom(format!("{} : hors des mesures possibles", n)))
    }
}

/// Écrite en nombre JSON, par son texte.
impl serde::Serialize for Mesure {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let n: serde_json::Number = self.to_string().parse().map_err(serde::ser::Error::custom)?;
        n.serialize(s)
    }
}
