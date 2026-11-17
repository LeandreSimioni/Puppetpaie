//! UNE FRACTION EXACTE : la forme interne d'une anciennete (`date::Anciennete`) et de mois de salaire
//! (`grandeurs::MoisDeSalaire`), qui comptent des jours sur la longueur de leur mois (1/30, 15/31) et des annees de douze
//! mois : un decimal les couperait au vingt-huitieme chiffre, et un montant exactement sur un demi-centime tomberait d'un
//! cote ou de l'autre selon cette coupure. Un signe, un numerateur et un denominateur entiers, toujours reduits ; elle
//! s'ajoute, se soustrait, se multiplie, se divise et se compare exactement, et ne devient un decimal qu'a sa porte :
//! l'arrondi d'un montant (`arrondi`), l'ecriture d'une phrase (`en_decimal`). Elle n'est pas une notion : hors du
//! socle, elle ne se nomme pas.

use crate::num::Decimal;
use std::cmp::Ordering;

/// Les mots de 64 bits d'un entier : 1 024 bits. Une anciennete lue d'un decimal tient en 200 bits, des mois de salaire
/// qui en sont faits (fois une part de mois par annee, une majoration) en 400 ; leurs produits, le temps d'une
/// comparaison ou d'un montant, en 800.
const MOTS: usize = 16;

/// Un entier naturel d'au plus 1 024 bits, ses mots du plus faible au plus fort.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Naturel([u64; MOTS]);

/// Ce qui deborde les 1 024 bits n'est pas un nombre de l'anciennete ni des mois de salaire (voir `MOTS`) : une faute du
/// code, pas une donnee.
fn deborde() -> ! {
    panic!("une fraction exacte deborde ses 1 024 bits : un nombre de l'anciennete ou des mois de salaire ne les atteint pas")
}

impl Naturel {
    const ZERO: Naturel = Naturel([0; MOTS]);

    fn de(n: u128) -> Naturel {
        let mut m = [0; MOTS];
        m[0] = n as u64;
        m[1] = (n >> 64) as u64;
        Naturel(m)
    }

    /// Ses mots utiles : sans les zeros du haut.
    fn mots(&self) -> usize {
        MOTS - self.0.iter().rev().take_while(|m| **m == 0).count()
    }

    fn nul(&self) -> bool {
        self.mots() == 0
    }

    fn en_u128(&self) -> Option<u128> {
        (self.mots() <= 2).then(|| u128::from(self.0[0]) | (u128::from(self.0[1]) << 64))
    }

    /// Son nombre de bits, le plus fort a un compris.
    fn bits(&self) -> u32 {
        match self.mots() {
            0 => 0,
            n => (n as u32 - 1) * 64 + (64 - self.0[n - 1].leading_zeros()),
        }
    }

    fn impair(&self) -> bool {
        self.0[0] & 1 == 1
    }

    fn plus(self, o: Naturel) -> Naturel {
        let n = self.mots().max(o.mots());
        let mut r = self;
        let mut retenue = 0u128;
        for i in 0..n {
            let t = u128::from(self.0[i]) + u128::from(o.0[i]) + retenue;
            r.0[i] = t as u64;
            retenue = t >> 64;
        }
        if retenue != 0 {
            match r.0.get_mut(n) {
                Some(m) => *m = retenue as u64,
                None => deborde(),
            }
        }
        r
    }

    /// `self - o`, `o` ne le depassant pas.
    fn moins(self, o: Naturel) -> Naturel {
        let mut r = self;
        let mut emprunt = false;
        for i in 0..self.mots() {
            let (t, e1) = self.0[i].overflowing_sub(o.0[i]);
            let (t, e2) = t.overflowing_sub(u64::from(emprunt));
            r.0[i] = t;
            emprunt = e1 || e2;
        }
        r
    }

    fn fois(self, o: Naturel) -> Naturel {
        let (na, nb) = (self.mots(), o.mots());
        if na <= 1 && nb <= 1 {
            return Naturel::de(u128::from(self.0[0]) * u128::from(o.0[0]));
        }
        let mut r = [0u64; 2 * MOTS];
        for i in 0..na {
            let mut retenue = 0u128;
            for j in 0..nb {
                let t = u128::from(self.0[i]) * u128::from(o.0[j]) + u128::from(r[i + j]) + retenue;
                r[i + j] = t as u64;
                retenue = t >> 64;
            }
            r[i + nb] = retenue as u64;
        }
        if r[MOTS..].iter().any(|m| *m != 0) {
            deborde();
        }
        let mut m = [0; MOTS];
        m.copy_from_slice(&r[..MOTS]);
        Naturel(m)
    }

    fn decale_a_gauche(self, k: u32) -> Naturel {
        if self.nul() {
            return self;
        }
        if self.bits() + k > 64 * MOTS as u32 {
            deborde();
        }
        let (mots, bits) = ((k / 64) as usize, k % 64);
        let mut r = [0; MOTS];
        for i in (mots..MOTS).rev() {
            let bas = self.0[i - mots];
            let avant = if bits > 0 && i > mots { self.0[i - mots - 1] >> (64 - bits) } else { 0 };
            r[i] = (bas << bits) | avant;
        }
        Naturel(r)
    }

    fn decale_a_droite(self, k: u32) -> Naturel {
        let (mots, bits) = ((k / 64) as usize, k % 64);
        let mut r = [0; MOTS];
        for (i, m) in r.iter_mut().enumerate().take(self.mots().saturating_sub(mots)) {
            let haut = self.0[i + mots];
            let apres = if bits > 0 && i + mots + 1 < MOTS { self.0[i + mots + 1] << (64 - bits) } else { 0 };
            *m = (haut >> bits) | apres;
        }
        Naturel(r)
    }

    /// Ses zeros de poids faible ; il n'est pas nul.
    fn zeros_de_fin(&self) -> u32 {
        let i = self.0.iter().take_while(|m| **m == 0).count();
        i as u32 * 64 + self.0[i].trailing_zeros()
    }

    /// Le quotient et le reste par `d`, non nul.
    fn divise(self, d: Naturel) -> (Naturel, Naturel) {
        if let (Some(a), Some(b)) = (self.en_u128(), d.en_u128()) {
            return (Naturel::de(a / b), Naturel::de(a % b));
        }
        if self < d {
            return (Naturel::ZERO, self);
        }
        let s = self.bits() - d.bits();
        let (mut q, mut r, mut dd) = (Naturel::ZERO, self, d.decale_a_gauche(s));
        for i in (0..=s).rev() {
            if r >= dd {
                r = r.moins(dd);
                q.0[(i / 64) as usize] |= 1 << (i % 64);
            }
            dd = dd.decale_a_droite(1);
        }
        (q, r)
    }

    /// Le plus grand diviseur commun (l'algorithme binaire), l'un ou l'autre non nul.
    fn pgcd(a: Naturel, b: Naturel) -> Naturel {
        if let (Some(a), Some(b)) = (a.en_u128(), b.en_u128()) {
            return Naturel::de(pgcd_u128(a, b));
        }
        if a.nul() {
            return b;
        }
        if b.nul() {
            return a;
        }
        let k = a.zeros_de_fin().min(b.zeros_de_fin());
        let (mut a, mut b) = (a.decale_a_droite(a.zeros_de_fin()), b);
        loop {
            b = b.decale_a_droite(b.zeros_de_fin());
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            b = b.moins(a);
            if b.nul() {
                return a.decale_a_gauche(k);
            }
        }
    }
}

fn pgcd_u128(mut a: u128, mut b: u128) -> u128 {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    let k = (a | b).trailing_zeros();
    a >>= a.trailing_zeros();
    loop {
        b >>= b.trailing_zeros();
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        b -= a;
        if b == 0 {
            return a << k;
        }
    }
}

impl PartialOrd for Naturel {
    fn partial_cmp(&self, o: &Naturel) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for Naturel {
    fn cmp(&self, o: &Naturel) -> Ordering {
        let n = self.mots();
        n.cmp(&o.mots()).then_with(|| self.0[..n].iter().rev().cmp(o.0[..n].iter().rev()))
    }
}

/// La moitie, a l'arrondi : loin de zero (un montant au centime, `Montant::au_centime`) ou au pair (un nombre dit dans
/// une phrase au dixieme).
#[derive(Clone, Copy)]
pub(crate) enum Moitie {
    LoinDeZero,
    AuPair,
}

/// UNE FRACTION EXACTE, reduite, son denominateur jamais nul ; zero n'est pas negatif.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fraction {
    negative: bool,
    num: Naturel,
    den: Naturel,
}

impl Fraction {
    pub(crate) const ZERO: Fraction = Fraction { negative: false, num: Naturel::ZERO, den: Naturel({
        let mut m = [0; MOTS];
        m[0] = 1;
        m
    }) };

    /// `num / den` reduite ; `den` n'est pas nul.
    fn reduite(negative: bool, num: Naturel, den: Naturel) -> Fraction {
        if num.nul() {
            return Fraction::ZERO;
        }
        let g = Naturel::pgcd(num, den);
        match g == Naturel::de(1) {
            true => Fraction { negative, num, den },
            false => Fraction { negative, num: num.divise(g).0, den: den.divise(g).0 },
        }
    }

    /// L'entier `n`.
    pub(crate) fn entier(n: i128) -> Fraction {
        Fraction::reduite(n < 0, Naturel::de(n.unsigned_abs()), Naturel::de(1))
    }

    /// `n / d`, `d` non nul.
    pub(crate) fn de(n: i128, d: u128) -> Fraction {
        Fraction::reduite(n < 0, Naturel::de(n.unsigned_abs()), Naturel::de(d))
    }

    /// Le decimal `d`, exactement : sa mantisse sur dix a la puissance de son echelle.
    pub(crate) fn du_decimal(d: Decimal) -> Fraction {
        Fraction::de(d.mantissa(), 10u128.pow(d.scale()))
    }

    /// Le plus grand entier qui ne la depasse pas.
    pub(crate) fn plancher(self) -> Fraction {
        let (q, r) = self.num.divise(self.den);
        match (self.negative, r.nul()) {
            (false, _) | (true, true) => Fraction::reduite(self.negative, q, Naturel::de(1)),
            (true, false) => Fraction::reduite(true, q.plus(Naturel::de(1)), Naturel::de(1)),
        }
    }

    /// Le plus petit entier qui ne la precede pas.
    pub(crate) fn plafond(self) -> Fraction {
        -((-self).plancher())
    }

    /// L'entier qu'elle est, quand elle en est un.
    pub(crate) fn en_entier(self) -> Option<i128> {
        if self.den != Naturel::de(1) {
            return None;
        }
        let n = i128::try_from(self.num.en_u128()?).ok()?;
        Some(if self.negative { -n } else { n })
    }

    /// SON ECRITURE DECIMALE, a vingt-huit chiffres significatifs, la moitie au pair, puis a vingt-huit decimales au plus
    /// (ce que `num::div` rend d'un entier sur un entier, `d28::P::div`) : sans zero inutile quand elle tombe juste. Une
    /// porte : l'ecriture d'une phrase ou d'un oracle.
    pub(crate) fn en_decimal(self) -> Decimal {
        let (entiere, mut r) = self.num.divise(self.den);
        let Some(mut q) = entiere.en_u128().and_then(|q| i128::try_from(q).ok()) else { deborde() };
        let chiffres = |q: i128| if q == 0 { 0 } else { q.unsigned_abs().ilog10() + 1 };
        let mut s = 0;
        while !r.nul() && chiffres(q) < 28 {
            let (c, reste) = r.fois(Naturel::de(10)).divise(self.den);
            q = q * 10 + c.0[0] as i128;
            r = reste;
            s += 1;
        }
        if !r.nul() {
            let deux_r = r.fois(Naturel::de(2));
            if deux_r > self.den || (deux_r == self.den && q % 2 != 0) {
                q += 1;
                if q == 10i128.pow(28) {
                    q /= 10;
                    s -= 1;
                }
            }
        }
        crate::num::en_decimal(crate::d28::arrondi28(if self.negative { -q } else { q }, s))
    }

    /// ARRONDIE A `decimales` DECIMALES, la moitie selon `moitie`, de sa valeur exacte, une seule fois : un montant au
    /// centime, un nombre dit au dixieme.
    pub(crate) fn arrondi(self, decimales: u32, moitie: Moitie) -> Decimal {
        let (q, r) = self.num.fois(Naturel::de(10u128.pow(decimales))).divise(self.den);
        let deux_r = r.fois(Naturel::de(2));
        let haut = match deux_r.cmp(&self.den) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => match moitie {
                Moitie::LoinDeZero => true,
                Moitie::AuPair => q.impair(),
            },
        };
        let q = if haut { q.plus(Naturel::de(1)) } else { q };
        let Some(q) = q.en_u128().and_then(|q| i128::try_from(q).ok()) else { deborde() };
        match Decimal::try_from_i128_with_scale(if self.negative { -q } else { q }, decimales) {
            Ok(d) => d,
            Err(_) => deborde(),
        }
    }
}

impl std::fmt::Debug for Fraction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.en_decimal(), f)
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(&self, o: &Fraction) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for Fraction {
    fn cmp(&self, o: &Fraction) -> Ordering {
        let grandeurs = |a: &Fraction, b: &Fraction| match a.den == b.den {
            true => a.num.cmp(&b.num),
            false => a.num.fois(b.den).cmp(&b.num.fois(a.den)),
        };
        match (self.negative, o.negative) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => grandeurs(self, o),
            (true, true) => grandeurs(o, self),
        }
    }
}

impl std::ops::Neg for Fraction {
    type Output = Fraction;
    fn neg(self) -> Fraction {
        match self.num.nul() {
            true => self,
            false => Fraction { negative: !self.negative, ..self },
        }
    }
}

impl std::ops::Add for Fraction {
    type Output = Fraction;
    fn add(self, o: Fraction) -> Fraction {
        let (x, y, den) = match self.den == o.den {
            true => (self.num, o.num, self.den),
            false => (self.num.fois(o.den), o.num.fois(self.den), self.den.fois(o.den)),
        };
        match (self.negative == o.negative, x.cmp(&y)) {
            (true, _) => Fraction::reduite(self.negative, x.plus(y), den),
            (false, Ordering::Less) => Fraction::reduite(o.negative, y.moins(x), den),
            (false, Ordering::Greater | Ordering::Equal) => Fraction::reduite(self.negative, x.moins(y), den),
        }
    }
}

impl std::ops::Sub for Fraction {
    type Output = Fraction;
    fn sub(self, o: Fraction) -> Fraction {
        self + (-o)
    }
}

impl std::ops::Mul for Fraction {
    type Output = Fraction;
    fn mul(self, o: Fraction) -> Fraction {
        Fraction::reduite(self.negative != o.negative, self.num.fois(o.num), self.den.fois(o.den))
    }
}

/// `self / o`, `o` non nul : le diviseur, chez chaque appelant, est un texte (douze mois, deux demi-mois) ou se refuse nul
/// avant (`Anciennete::fois_entieres`).
impl std::ops::Div for Fraction {
    type Output = Fraction;
    fn div(self, o: Fraction) -> Fraction {
        if o.num.nul() {
            panic!("une fraction exacte ne se divise pas par zero");
        }
        Fraction::reduite(self.negative != o.negative, self.num.fois(o.den), self.den.fois(o.num))
    }
}
