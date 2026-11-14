//! L'heure locale du poste : Europe/Paris, sans lire le fuseau du systeme.
//!
//! Les dates et heures que le logiciel ecrit sont celles de Paris, quel que soit le fuseau du poste. Le moteur n'a pas
//! de fuseau : il applique la regle de l'heure d'ete de l'Union europeenne
//! (directive 2000/84/CE, art. 2 et 3, en vigueur a compter de 2002 ;
//! `atelier/reference/sources/union-europeenne/eurlex-directive-2000-84-ce-heure-d-ete-20010202.txt`) :
//! UTC+2 du dernier dimanche de mars (1 h UTC) au dernier dimanche d'octobre
//! (1 h UTC), UTC+1 le reste de l'annee. Avant 2002, d'autres directives
//! reglaient la periode ; aucune date de fichier du logiciel n'y remonte.
//!
//! UN INSTANT EST UN `Moment` : des secondes depuis 1970, dont le jour, en UTC comme a Paris, est un jour du calendrier
//! (`Jour` : annees 1 a 9999). Il ne se construit que par ses constructeurs, qui le verifient a sa source (l'horloge,
//! un jour pose, un instant ecrit dans un fichier, la date d'un fichier) : les fonctions d'ici sont totales.

use crate::date::Jour;

/// Le premier instant : le 01/01/0001 a 0 h UTC.
const PREMIER: i64 = (1 - 719_163) * 86_400;
/// Le dernier instant dont l'heure de Paris, arrondie a la seconde, tombe encore le 31/12/9999 : deux heures et une
/// seconde avant sa fin en UTC (l'heure d'ete ajoute deux heures).
const DERNIER: i64 = (3_652_059 + 1 - 719_163) * 86_400 - 7_201;

/// Les nanosecondes d'une seconde.
const NANOS: i128 = 1_000_000_000;

/// UN INSTANT, en nanosecondes depuis le 01/01/1970 (UTC), dans le calendrier (le module) : un entier, exact. La montre
/// le donne a la microseconde ; un instant ecrit dans un fichier se lit tel qu'il est ecrit, a la nanoseconde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Moment(i128);

impl Moment {
    /// L'instant de `ns` nanosecondes depuis 1970 ; `None` hors du calendrier.
    fn de_nanos(ns: i128) -> Option<Moment> {
        (ns >= PREMIER as i128 * NANOS && ns <= DERNIER as i128 * NANOS).then_some(Moment(ns))
    }

    /// L'instant de `t` secondes entieres depuis 1970 ; `None` hors du calendrier.
    pub fn de_secondes(t: i64) -> Option<Moment> {
        Moment::de_nanos(t as i128 * NANOS)
    }

    /// L'instant d'une duree depuis 1970 (la montre, la date d'un fichier), a la nanoseconde ; `avant_1970` : avant.
    pub fn de_duree(d: std::time::Duration, avant_1970: bool) -> Option<Moment> {
        let ns = d.as_nanos() as i128;
        Moment::de_nanos(if avant_1970 { -ns } else { ns })
    }

    /// L'instant de `jour` a `heure:minute:seconde`, heure de Paris ; `None` hors du calendrier.
    pub fn a_paris(jour: Jour, heure: i64, minute: i64, seconde: i64) -> Option<Moment> {
        let local = (jour.ordinal() - 719_163) * 86_400 + heure * 3_600;
        // Le decalage de Paris a cet instant : celui de l'instant local lu comme UTC, corrige une fois (un changement
        // d'heure ne tombe jamais a l'heure d'un pas).
        let approx = local - decalage_brut(local)?;
        Moment::de_secondes(local - decalage_brut(approx)? + minute * 60 + seconde)
    }

    /// Les secondes entieres depuis 1970 (l'instant arrondi a la seconde d'avant).
    pub fn entieres(self) -> i64 {
        self.0.div_euclid(NANOS) as i64
    }

    /// Les nanosecondes passees depuis la seconde entiere (`entieres`), de 0 a 999 999 999.
    pub fn nanosecondes(self) -> u32 {
        self.0.rem_euclid(NANOS) as u32
    }

    /// L'instant arrondi a la microseconde la plus proche (au pair a egalite) : la montre ne dit pas plus fin.
    /// Les bornes du calendrier sont des secondes entieres : l'arrondi y reste.
    pub fn a_la_microseconde(self) -> Moment {
        let us = self.0.div_euclid(1_000);
        let reste = self.0.rem_euclid(1_000);
        let us = if reste > 500 || (reste == 500 && us % 2 != 0) { us + 1 } else { us };
        Moment(us * 1_000)
    }

    /// `secondes` plus tard (moins tot : negatives) ; `None` hors du calendrier.
    pub fn plus(self, secondes: i64) -> Option<Moment> {
        Moment::de_nanos(self.0 + secondes as i128 * NANOS)
    }

    /// `d` plus tard, a la nanoseconde ; `None` hors du calendrier.
    pub fn plus_duree(self, d: std::time::Duration) -> Option<Moment> {
        Moment::de_nanos(self.0 + d.as_nanos() as i128)
    }

    /// La duree de `avant` a cet instant ; `None` si `avant` est apres.
    pub fn depuis(self, avant: Moment) -> Option<std::time::Duration> {
        let ns = self.0.checked_sub(avant.0).filter(|ns| *ns >= 0)?;
        Some(std::time::Duration::new((ns / NANOS) as u64, (ns % NANOS) as u32))
    }

    /// LA GRAINE DES ALEAS EN SUITE FIXE (`temps::aleas`) : les 64 bits du nombre binaire IEEE 754 (double precision) le
    /// plus proche de ses secondes (au pair a egalite), calcules en entiers. Les scenarios enregistres et les fictives
    /// tirent leurs aleas de cette graine : un nom de courriel joue, un code de signature, en dependent.
    pub fn graine(self) -> u64 {
        if self.0 == 0 {
            return 0;
        }
        let signe = if self.0 < 0 { 1u64 << 63 } else { 0 };
        let a = self.0.unsigned_abs();
        // `a << k` est entre 2^82 et 2^83 : son quotient par 10^9 tient entre 2^52 et 2^53,6, la mantisse a peu pres.
        let mut k = 83 - (128 - a.leading_zeros()) as i32;
        let decale = |k: i32| if k >= 0 { a << k } else { a >> -k };
        let mut q = decale(k) / NANOS as u128;
        if q >= 1 << 53 {
            k -= 1;
            q = decale(k) / NANOS as u128;
        }
        // `a` tient sous 2^69 (l'an 9999) : `k` reste positif, rien ne se perd au decalage.
        let reste = decale(k) % NANOS as u128;
        if 2 * reste > NANOS as u128 || (2 * reste == NANOS as u128 && q % 2 == 1) {
            q += 1;
        }
        if q == 1 << 53 {
            q = 1 << 52;
            k -= 1;
        }
        let exposant = (52 - k + 1023) as u64;
        signe | exposant << 52 | (q as u64 - (1 << 52))
    }

    /// L'instant tel qu'un fichier l'ecrit (`lu` le relit) : ses secondes depuis 1970, en nombre a virgule (`1767600000.0`,
    /// `1793606401.2339187`), sans zero en queue de la fraction.
    pub fn ecrit(self) -> String {
        let fraction = format!("{:09}", self.nanosecondes());
        let fraction = fraction.trim_end_matches('0');
        format!("{}.{}", self.entieres(), if fraction.is_empty() { "0" } else { fraction })
    }

    /// L'instant d'un nombre de secondes ecrit (`1767600000`, `1793606401.2339187`, `1.7676e9`), exact, arrondi a la
    /// nanoseconde (au pair a egalite) ; `None` s'il ne se lit pas ou sort du calendrier.
    pub(crate) fn lu(texte: &str) -> Option<Moment> {
        let d = crate::ecriture::decouper(texte)?;
        let moins = d.moins;
        let chiffres = d.chiffres.trim_start_matches('0');
        // La valeur vaut `chiffres` x 10^puissance nanosecondes ; 10^39 ne tient plus sur 128 bits.
        let puissance = d.exposant.checked_sub((d.chiffres.len() - d.entiers) as i64)?.checked_add(9)?;
        if puissance.unsigned_abs() > 38 {
            return None;
        }
        let mut n: i128 = 0;
        for c in chiffres.bytes() {
            n = n.checked_mul(10)?.checked_add(i128::from(c - b'0'))?;
        }
        let ns = if puissance >= 0 {
            n.checked_mul(10i128.pow(puissance as u32))?
        } else {
            let diviseur = 10i128.pow(puissance.unsigned_abs() as u32);
            let (q, r) = (n / diviseur, n % diviseur);
            if r > diviseur - r || (r == diviseur - r && q % 2 == 1) { q + 1 } else { q }
        };
        Moment::de_nanos(if moins { -ns } else { ns })
    }

    /// Le jour de Paris de cet instant (l'instant arrondi a la seconde d'avant).
    pub fn jour_de_paris(self) -> Jour {
        dans_le_calendrier((self.entieres() + decalage(self)).div_euclid(86_400))
    }

    /// Le jour UTC de cet instant.
    pub fn jour_utc(self) -> Jour {
        dans_le_calendrier(self.entieres().div_euclid(86_400))
    }


    /// L'instant a la seconde : arrondi a la microseconde (la montre ne dit pas plus fin), puis tronque a la seconde. Le
    /// decalage de Paris est d'heures entieres : la seconde de Paris tombe sur la seconde UTC.
    pub fn a_la_seconde(self) -> Moment {
        Moment(self.a_la_microseconde().entieres() as i128 * NANOS)
    }

    /// L'instant a la minute : comme `a_la_seconde`, puis tronque a la minute (de Paris comme d'UTC : le decalage est
    /// d'heures entieres).
    pub fn a_la_minute(self) -> Moment {
        let s = self.a_la_microseconde().entieres();
        Moment((s - s.rem_euclid(60)) as i128 * NANOS)
    }

    /// « 12/09/2026 à 18:18 » : le jour et l'heure de Paris, a la minute, tels que l'ecran les dit.
    pub fn dit(self) -> String {
        let (jour, reste) = ecrit::local(self);
        format!("{} à {:02}:{:02}", jour.jj_mm_aaaa(), reste / 3_600, (reste % 3_600) / 60)
    }

    /// Les secondes passees depuis minuit de Paris, ce jour-la (l'instant arrondi a la seconde d'avant, comme
    /// `jour_de_paris`).
    pub fn seconde_du_jour_de_paris(self) -> i64 {
        (self.entieres() + decalage(self)).rem_euclid(86_400)
    }
}

/// LES ECRITURES D'UN INSTANT : une facon d'ecrire n'est pas un type. Chaque forme de fichier dit la sienne par un
/// attribut serde (`#[serde(with = "socle::paris::ecrit::seconde")]`, `…::option` pour un `Option<Moment>` : absent ou
/// `null`, `None`), ou l'ecrit par `ecrit::<forme>::texte`. Une heure de Paris se lit champ a champ, l'instant se
/// construit (`Moment::a_paris`), puis se reecrit : s'il ne redonne pas le texte lu, la lecture rend `None`. Une heure
/// qui n'existe pas a Paris (l'heure sautee du passage a l'heure d'ete, directive 2000/84/CE, art. 2) se refuse ainsi ;
/// l'heure doublee du passage a l'heure d'hiver (art. 3) se relit a l'identique.
pub mod ecrit {
    use super::{local_arrondi, Moment};
    use crate::date::Jour;

    /// Le jour et la seconde du jour de l'heure de Paris d'un instant (arrondi a la microseconde, puis tronque).
    pub(super) fn local(m: Moment) -> (Jour, i64) {
        let l = local_arrondi(m);
        (super::dans_le_calendrier(l.div_euclid(86_400)), l.rem_euclid(86_400))
    }

    /// Le jour et la seconde du jour UTC d'un instant (arrondi a la microseconde, puis tronque).
    fn utc(m: Moment) -> (Jour, i64) {
        let s = m.a_la_microseconde().entieres();
        (super::dans_le_calendrier(s.div_euclid(86_400)), s.rem_euclid(86_400))
    }

    /// Des chiffres ASCII seuls, au plus quatre : leur valeur se compte, sans lecture qui puisse echouer.
    fn chiffres(t: &str) -> Option<i64> {
        (!t.is_empty() && t.len() <= 4 && t.bytes().all(|b| b.is_ascii_digit())).then(|| t.bytes().fold(0i64, |n, b| n * 10 + i64::from(b - b'0')))
    }

    /// L'instant d'une heure de Paris ecrite champ a champ (`an`, `mois`, `jour`, `heure`, `minute`, `seconde`), relu par
    /// `texte` : `None` si un champ ne se lit pas, si l'heure n'existe pas a Paris, ou si l'ecriture ne redonne pas `lu`.
    fn de_paris(champs: [&str; 6], texte: fn(Moment) -> String, lu: &str) -> Option<Moment> {
        let [an, mois, jour, heure, minute, seconde] = champs;
        let jour = Jour::nouveau(chiffres(an)? as i32, chiffres(mois)? as u32, chiffres(jour)? as u32)?;
        let m = Moment::a_paris(jour, chiffres(heure)?, chiffres(minute)?, chiffres(seconde)?)?;
        (texte(m) == lu).then_some(m)
    }

    /// Les morceaux `[a..b]` d'un texte ASCII de `n` octets ; `None` sinon.
    fn morceaux<const K: usize>(t: &str, n: usize, bornes: [(usize, usize); K]) -> Option<[&str; K]> {
        if t.len() != n || !t.is_ascii() {
            return None;
        }
        Some(bornes.map(|(a, b)| &t[a..b]))
    }

    /// Le `serialize` et le `deserialize` d'une ecriture en texte (`texte`, `lu`), et ceux de son `Option` (`option`).
    macro_rules! en_texte {
        ($message:literal) => {
            /// L'instant ecrit dans cette forme.
            pub fn serialize<S: serde::Serializer>(m: &Moment, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&texte(*m))
            }

            /// L'instant lu dans cette forme, et elle seule.
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Moment, D::Error> {
                let s = <String as serde::Deserialize>::deserialize(d)?;
                lu(&s).ok_or_else(|| serde::de::Error::custom(format!($message, crate::date::repr_texte(&s))))
            }

            /// Un `Option<Moment>` dans cette forme : `null`, `None` ; une cle absente ne se lit `None` que si le champ porte
            /// `#[serde(default)]` (un champ omis quand il est vide).
            pub mod option {
                use super::Moment;

                pub fn serialize<S: serde::Serializer>(m: &Option<Moment>, s: S) -> Result<S::Ok, S::Error> {
                    match m {
                        Some(m) => s.serialize_some(&super::texte(*m)),
                        None => s.serialize_none(),
                    }
                }

                pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Moment>, D::Error> {
                    match <Option<String> as serde::Deserialize>::deserialize(d)? {
                        Some(s) => super::lu(&s).map(Some).ok_or_else(|| serde::de::Error::custom(format!($message, crate::date::repr_texte(&s)))),
                        None => Ok(None),
                    }
                }
            }
        };
    }

    /// `AAAA-MM-JJTHH:MM:SS`, heure de Paris : l'instant d'un geste (la saisie d'un imprevu, la validation d'une
    /// version, le depart d'un envoi, une ligne de journal).
    pub mod seconde {
        use super::Moment;

        pub fn texte(m: Moment) -> String {
            let (j, r) = super::local(m);
            format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", j.an, j.mois, j.jour, r / 3_600, (r % 3_600) / 60, r % 60)
        }

        pub(crate) fn lu(t: &str) -> Option<Moment> {
            let c = super::morceaux(t, 19, [(0, 4), (5, 7), (8, 10), (11, 13), (14, 16), (17, 19)])?;
            super::de_paris(c, texte, t)
        }

        en_texte!("{} n'est pas un instant AAAA-MM-JJTHH:MM:SS");
    }

    /// `AAAA-MM-JJTHH:MM`, heure de Paris : la pose d'un secret dans le coffre, la fin d'une seance.
    pub mod minute {
        use super::Moment;

        pub fn texte(m: Moment) -> String {
            let (j, r) = super::local(m);
            format!("{:04}-{:02}-{:02}T{:02}:{:02}", j.an, j.mois, j.jour, r / 3_600, (r % 3_600) / 60)
        }

        pub(crate) fn lu(t: &str) -> Option<Moment> {
            let [a, m, j, h, mi] = super::morceaux(t, 16, [(0, 4), (5, 7), (8, 10), (11, 13), (14, 16)])?;
            super::de_paris([a, m, j, h, mi, "0"], texte, t)
        }

        en_texte!("{} n'est pas un instant AAAA-MM-JJTHH:MM");
    }

    /// `AAAA-MM-JJ HH:MM`, heure de Paris, une espace entre le jour et l'heure : la saisie d'un imprevu, de la
    /// connaissance d'un arret.
    pub mod saisie {
        use super::Moment;

        pub fn texte(m: Moment) -> String {
            let (j, r) = super::local(m);
            format!("{:04}-{:02}-{:02} {:02}:{:02}", j.an, j.mois, j.jour, r / 3_600, (r % 3_600) / 60)
        }

        pub(crate) fn lu(t: &str) -> Option<Moment> {
            let [a, m, j, h, mi] = super::morceaux(t, 16, [(0, 4), (5, 7), (8, 10), (11, 13), (14, 16)])?;
            super::de_paris([a, m, j, h, mi, "0"], texte, t)
        }

        en_texte!("{} n'est pas une heure AAAA-MM-JJ HH:MM");
    }

    /// `JJ/MM/AAAA HH:MM`, heure de Paris : l'heure d'un passage de l'ouverture, que le bandeau montre.
    pub mod francaise {
        use super::Moment;

        pub fn texte(m: Moment) -> String {
            let (j, r) = super::local(m);
            format!("{} {:02}:{:02}", j.jj_mm_aaaa(), r / 3_600, (r % 3_600) / 60)
        }

        pub(crate) fn lu(t: &str) -> Option<Moment> {
            let [j, m, a, h, mi] = super::morceaux(t, 16, [(0, 2), (3, 5), (6, 10), (11, 13), (14, 16)])?;
            super::de_paris([a, m, j, h, mi, "0"], texte, t)
        }

        en_texte!("{} n'est pas une heure JJ/MM/AAAA HH:MM");
    }

    /// `AAAA-MM-JJTHH:MMZ`, en UTC : l'heure ou le serveur du filet a releve la reponse a un retour (`iso` de
    /// `infra/filet/filet_serveur.py`, `FORMAT = "%Y-%m-%dT%H:%M"` suivi de `Z`).
    pub mod minute_utc {
        use super::Moment;

        pub fn texte(m: Moment) -> String {
            let (j, r) = super::utc(m);
            format!("{:04}-{:02}-{:02}T{:02}:{:02}Z", j.an, j.mois, j.jour, r / 3_600, (r % 3_600) / 60)
        }

        pub(crate) fn lu(t: &str) -> Option<Moment> {
            let [a, m, j, h, mi] = super::morceaux(t, 17, [(0, 4), (5, 7), (8, 10), (11, 13), (14, 16)])?;
            let jour = crate::date::Jour::nouveau(super::chiffres(a)? as i32, super::chiffres(m)? as u32, super::chiffres(j)? as u32)?;
            let instant = Moment::de_secondes((jour.ordinal() - 719_163) * 86_400 + super::chiffres(h)? * 3_600 + super::chiffres(mi)? * 60)?;
            (texte(instant) == t).then_some(instant)
        }

        en_texte!("{} n'est pas une heure AAAA-MM-JJTHH:MMZ");

        /// L'heure d'une reponse, telle que le serveur du filet l'ecrit : `""` tant que l'editeur n'a pas repondu (`None`),
        /// sinon une minute UTC. Le `""` est un contrat avec les versions installees, qui le lisent en texte strict : le
        /// serveur ne change pas, et il se lit ici, a la porte, comme le cas qu'il nomme. `null` se refuse.
        pub fn vide_ou<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Moment>, D::Error> {
            let s = <String as serde::Deserialize>::deserialize(d)?;
            if s.is_empty() {
                return Ok(None);
            }
            lu(&s).map(Some).ok_or_else(|| serde::de::Error::custom(format!("{} n'est pas une heure AAAA-MM-JJTHH:MMZ", crate::date::repr_texte(&s))))
        }
    }

    /// `AAAAMMJJHHMMSS` : la date d'un depot que la plate-forme DSN liste (guide d'implementation de l'API DSN v6.8,
    /// « Date du dépôt », `depot.date`, format `yyyyMMddhhmmss`, [14,14] ;
    /// `atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`). Le guide ne
    /// dit pas le fuseau ; lue comme l'heure de Paris (une inference).
    pub mod compacte {
        use super::Moment;

        pub fn texte(m: Moment) -> String {
            let (j, r) = super::local(m);
            format!("{:04}{:02}{:02}{:02}{:02}{:02}", j.an, j.mois, j.jour, r / 3_600, (r % 3_600) / 60, r % 60)
        }

        pub(crate) fn lu(t: &str) -> Option<Moment> {
            let c = super::morceaux(t, 14, [(0, 4), (4, 6), (6, 8), (8, 10), (10, 12), (12, 14)])?;
            super::de_paris(c, texte, t)
        }

        en_texte!("{} n'est pas un instant AAAAMMJJHHMMSS");
    }

    /// Un entier : les secondes de l'heure de Paris lue comme si c'etait de l'UTC (l'enregistrement d'un message).
    pub mod secondes_locales {
        use super::Moment;

        /// Les secondes locales de Paris de cet instant (arrondi a la microseconde, puis tronque).
        pub fn texte(m: Moment) -> String {
            nombre(m).to_string()
        }

        fn nombre(m: Moment) -> i64 {
            let (j, r) = super::local(m);
            (j.ordinal() - 719_163) * 86_400 + r
        }

        pub(crate) fn lu(local: i64) -> Option<Moment> {
            let jour = crate::date::Jour::depuis_jours_unix(local.div_euclid(86_400))?;
            let r = local.rem_euclid(86_400);
            let m = Moment::a_paris(jour, r / 3_600, (r % 3_600) / 60, r % 60)?;
            (nombre(m) == local).then_some(m)
        }

        pub fn serialize<S: serde::Serializer>(m: &Moment, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_i64(nombre(*m))
        }

        pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Moment, D::Error> {
            let t = <i64 as serde::Deserialize>::deserialize(d)?;
            lu(t).ok_or_else(|| serde::de::Error::custom(format!("{} s n'est pas une heure du calendrier (années 1 à 9999)", t)))
        }
    }

    /// UN JOUR ET SON HEURE EN DEUX CHAMPS : le jour `AAAA-MM-JJ` et l'heure de Paris `HH:MM`, facultative ; ensemble, un
    /// `InstantOuJour` (l'instant a la minute, ou le jour seul quand l'heure manque). La fiche de suivi des obligations
    /// d'un arret les ecrit ainsi (le jour et l'heure ou une obligation hors DSN a ete faite, ou la compagnie a connu
    /// l'arret ou l'accident), et ses routes les rendent ainsi. La forme qui les porte ecrit l'heure en `HeureEcrite`, et
    /// passe de ses deux champs a l'instant par `quand`, de l'instant a ses deux champs par `champs`.
    pub mod jour_et_heure {
        use super::super::{InstantOuJour, Moment};
        use crate::date::Jour;

        /// L'heure `HH:MM` qu'une forme ecrit a cote de son jour : elle ne se lit que de ce texte, et ne sert qu'a faire
        /// l'instant (`quand`).
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct HeureEcrite {
            heure: i64,
            minute: i64,
        }

        /// L'instant de `jour` a l'heure `heure`, ou le jour seul sans heure ; `None` si cette heure n'existe pas a Paris
        /// ce jour-la (l'heure sautee du passage a l'heure d'ete, directive 2000/84/CE, art. 2).
        pub fn quand(jour: Jour, heure: Option<HeureEcrite>) -> Option<InstantOuJour> {
            match heure {
                None => Some(InstantOuJour::Jour(jour)),
                Some(h) => a_la_minute(jour, h.heure * 60 + h.minute),
            }
        }

        /// L'instant de `jour` a `minute` (comptee depuis minuit, heure de Paris) ; `None` pour une minute hors du jour,
        /// ou une heure qui n'existe pas a Paris ce jour-la.
        pub fn a_la_minute(jour: Jour, minute: i64) -> Option<InstantOuJour> {
            if !(0..1_440).contains(&minute) {
                return None;
            }
            let m = Moment::a_paris(jour, minute / 60, minute % 60, 0)?;
            (super::local(m) == (jour, minute * 60)).then_some(InstantOuJour::Instant(m))
        }

        /// La minute du jour (comptee depuis minuit, heure de Paris) d'un instant ; `None` pour un jour seul.
        pub fn minute(q: InstantOuJour) -> Option<u32> {
            match q {
                InstantOuJour::Jour(_) => None,
                // La seconde du jour est de 0 a 86 399 : sa minute tient dans un `u32`.
                InstantOuJour::Instant(m) => Some((super::local(m).1 / 60) as u32),
            }
        }

        /// Les deux champs d'un instant ou d'un jour : son jour (de Paris) et, pour un instant, son heure a la minute.
        pub fn champs(q: InstantOuJour) -> (Jour, Option<HeureEcrite>) {
            match q {
                InstantOuJour::Jour(j) => (j, None),
                InstantOuJour::Instant(m) => {
                    let (j, r) = super::local(m);
                    (j, Some(HeureEcrite { heure: r / 3_600, minute: (r % 3_600) / 60 }))
                }
            }
        }

        impl HeureEcrite {
            /// L'HEURE QU'UNE CHARGE SAISIT, sa seule lecture : `H:M`, un ou deux chiffres de chaque cote, sans ses blancs
            /// autour, de 00:00 a 23:59 ; `None` sinon.
            fn saisie(texte: &str) -> Option<HeureEcrite> {
                let (a, b) = texte.trim().split_once(':')?;
                let nombre = |t: &str| (!t.is_empty() && t.len() <= 2 && t.bytes().all(|c| c.is_ascii_digit())).then(|| t.bytes().fold(0i64, |n, c| n * 10 + i64::from(c - b'0')));
                let (heure, minute) = (nombre(a)?, nombre(b)?);
                (heure < 24 && minute < 60).then_some(HeureEcrite { heure, minute })
            }
        }

        impl std::fmt::Display for HeureEcrite {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:02}:{:02}", self.heure, self.minute)
            }
        }

        /// L'HEURE FACULTATIVE D'UNE CHARGE (`#[serde(default, deserialize_with = "…::saisie")]`) : absente ou nulle, aucune ;
        /// `""` ne dit rien et se refuse, comme tout texte d'une charge ; sinon `HeureEcrite::saisie`, ou le refus.
        pub fn saisie<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<HeureEcrite>, D::Error> {
            let Some(t) = <Option<String> as serde::Deserialize>::deserialize(d)? else { return Ok(None) };
            if t.is_empty() {
                return Err(serde::de::Error::custom("un texte vide ne dit rien : le vide s'écrit absent ou null"));
            }
            match HeureEcrite::saisie(&t) {
                Some(h) => Ok(Some(h)),
                None => Err(serde::de::Error::custom(format!("Heure {} illisible : HH:MM, de 00:00 a 23:59.", crate::date::repr_texte(&t)))),
            }
        }

        impl serde::Serialize for HeureEcrite {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.to_string())
            }
        }

        impl<'de> serde::Deserialize<'de> for HeureEcrite {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<HeureEcrite, D::Error> {
                let t = <String as serde::Deserialize>::deserialize(d)?;
                let lue = match super::morceaux(&t, 5, [(0, 2), (2, 3), (3, 5)]) {
                    Some([h, ":", m]) => match (super::chiffres(h), super::chiffres(m)) {
                        (Some(heure), Some(minute)) if heure < 24 && minute < 60 => Some(HeureEcrite { heure, minute }),
                        _ => None,
                    },
                    _ => None,
                };
                lue.ok_or_else(|| serde::de::Error::custom(format!("{} n'est pas une heure HH:MM", crate::date::repr_texte(&t))))
            }
        }
    }
}

/// UN INSTANT ECRIT DANS UN FICHIER (un scenario) : ses secondes depuis 1970, un nombre a virgule (`ecrit`), lues
/// contre le calendrier (`lu`).
impl serde::Serialize for Moment {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        crate::forme::nombre_du_texte(&self.ecrit()).serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for Moment {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Moment, D::Error> {
        let t = crate::forme::nombre_en_texte(d)?;
        Moment::lu(&t).ok_or_else(|| serde::de::Error::custom(format!("{} s n'est pas un instant du calendrier (années 1 à 9999)", t)))
    }
}

/// L'INSTANT OU LE JOUR : l'instant d'un geste (un envoi, un pas de l'horloge, la reprise d'un certificat DPAE ou d'un
/// bilan qui dit son heure), ou le jour seul (la reprise d'un bilan qui ne dit que sa date de reception, un geste de
/// scenario dit d'un jour). Il s'ecrit `AAAA-MM-JJTHH:MM:SS` (l'heure de Paris, `ecrit::seconde`) ou `AAAA-MM-JJ`, et se
/// lit dans l'une de ces deux formes. L'ordre : par jour de Paris ; un jour seul avant les instants de ce jour ; puis par
/// instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstantOuJour {
    Instant(Moment),
    Jour(Jour),
}

impl InstantOuJour {
    /// `AAAA-MM-JJTHH:MM:SS` ou `AAAA-MM-JJ`, et rien d'autre ; `None` sinon.
    pub(crate) fn lire(texte: &str) -> Option<InstantOuJour> {
        match ecrit::seconde::lu(texte) {
            Some(i) => Some(InstantOuJour::Instant(i)),
            None => crate::date::jour_iso(texte).filter(|j| j.iso() == texte).map(InstantOuJour::Jour),
        }
    }

    /// Son jour (de Paris).
    pub fn jour(self) -> Jour {
        match self {
            InstantOuJour::Instant(i) => i.jour_de_paris(),
            InstantOuJour::Jour(j) => j,
        }
    }

    /// Sa forme ecrite.
    pub fn iso(self) -> String {
        match self {
            InstantOuJour::Instant(i) => ecrit::seconde::texte(i),
            InstantOuJour::Jour(j) => j.iso(),
        }
    }

    /// Tel que l'ecran le dit : « 12/09/2026 à 18:18 », ou « 12/09/2026 » pour un jour seul.
    pub fn dit(self) -> String {
        match self {
            InstantOuJour::Instant(i) => i.dit(),
            InstantOuJour::Jour(j) => j.jj_mm_aaaa(),
        }
    }
}

impl From<Moment> for InstantOuJour {
    fn from(i: Moment) -> InstantOuJour {
        InstantOuJour::Instant(i)
    }
}

impl PartialOrd for InstantOuJour {
    fn partial_cmp(&self, autre: &InstantOuJour) -> Option<std::cmp::Ordering> {
        Some(self.cmp(autre))
    }
}

impl Ord for InstantOuJour {
    fn cmp(&self, autre: &InstantOuJour) -> std::cmp::Ordering {
        use InstantOuJour::{Instant, Jour};
        self.jour().cmp(&autre.jour()).then_with(|| match (self, autre) {
            (Jour(_), Jour(_)) => std::cmp::Ordering::Equal,
            (Jour(_), Instant(_)) => std::cmp::Ordering::Less,
            (Instant(_), Jour(_)) => std::cmp::Ordering::Greater,
            (Instant(a), Instant(b)) => a.cmp(b),
        })
    }
}

impl std::fmt::Display for InstantOuJour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.iso())
    }
}

impl serde::Serialize for InstantOuJour {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.iso())
    }
}

impl<'de> serde::Deserialize<'de> for InstantOuJour {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<InstantOuJour, D::Error> {
        let s = String::deserialize(d)?;
        InstantOuJour::lire(&s).ok_or_else(|| serde::de::Error::custom(format!("{} n'est ni un instant AAAA-MM-JJTHH:MM:SS ni un jour AAAA-MM-JJ", crate::date::repr_texte(&s))))
    }
}

/// LE FUSEAU QU'UNE DATE OU UNE HEURE DE XML SCHEMA PEUT PORTER (XML Schema Part 2, § 3.2.7.3 : `Z`, ou `(+|-)hh:mm`,
/// `hh` de 00 a 14, `mm` de 00 a 59, et 14 seulement avec `:00`). Il garde ce qui est ecrit : son signe et ses chiffres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FuseauXsd {
    Z,
    Decalage { moins: bool, heures: u8, minutes: u8 },
}

impl FuseauXsd {
    /// `Z` ou `(+|-)hh:mm`, et rien d'autre ; `None` sinon.
    fn lire(texte: &str) -> Option<FuseauXsd> {
        if texte == "Z" {
            return Some(FuseauXsd::Z);
        }
        let b = texte.as_bytes();
        if b.len() != 6 || !matches!(b[0], b'+' | b'-') || b[3] != b':' {
            return None;
        }
        let (heures, minutes) = (deux_chiffres(&b[1..3])?, deux_chiffres(&b[4..6])?);
        (heures < 14 && minutes < 60 || heures == 14 && minutes == 0).then_some(FuseauXsd::Decalage { moins: b[0] == b'-', heures, minutes })
    }

    /// Son decalage sur UTC, en secondes (`Z` : zero).
    pub fn secondes(self) -> i64 {
        match self {
            FuseauXsd::Z => 0,
            FuseauXsd::Decalage { moins, heures, minutes } => (if moins { -1 } else { 1 }) * (i64::from(heures) * 3_600 + i64::from(minutes) * 60),
        }
    }
}

impl std::fmt::Display for FuseauXsd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FuseauXsd::Z => f.write_str("Z"),
            FuseauXsd::Decalage { moins, heures, minutes } => write!(f, "{}{:02}:{:02}", if *moins { '-' } else { '+' }, heures, minutes),
        }
    }
}

/// UNE DATE DE XML SCHEMA (`date`, XML Schema Part 2, § 3.2.9) : `AAAA-MM-JJ`, suivie ou non d'un fuseau (`FuseauXsd`) ;
/// la date de reception d'un bilan (`date_reception`, schema des bilans harmonises, l. 318-322 ;
/// `atelier/reference/sources/gip-mds-api-dsn/net-entreprises-dsn-bilans-v02r01-20230703.xsd`). Une annee hors de 1 a
/// 9999 (un signe moins, plus de quatre chiffres), que le schema permet, sort du calendrier du moteur : elle se refuse.
/// Rend le jour et le fuseau ecrits ; `None` pour autre chose.
pub(crate) fn date_xsd(texte: &str) -> Option<(Jour, Option<FuseauXsd>)> {
    let (j, f) = (texte.get(..10)?, texte.get(10..)?);
    let jour = crate::date::jour_iso(j).filter(|x| x.iso() == j)?;
    let fuseau = if f.is_empty() { None } else { Some(FuseauXsd::lire(f)?) };
    Some((jour, fuseau))
}

/// UNE HEURE DE XML SCHEMA (`time`, XML Schema Part 2, § 3.2.8) : `hh:mm:ss`, une fraction de seconde ou non (`.` et des
/// chiffres), un fuseau ou non (`FuseauXsd`) ; `24:00:00` est la fin du jour (XML Schema 1.1, § 3.3.8). L'heure de
/// reception d'un bilan (`heure_reception`, schema des bilans harmonises, l. 323-327 ; meme fichier que `date_xsd`).
/// Rend les secondes depuis minuit (de 0 a 86 400), si la fraction est nulle (absente, ou rien que des zeros : l'heure
/// tombe sur sa seconde), et le fuseau ecrit ; `None` pour autre chose.
pub(crate) fn heure_xsd(texte: &str) -> Option<(i64, bool, Option<FuseauXsd>)> {
    let b = texte.as_bytes();
    if b.len() < 8 || b[2] != b':' || b[5] != b':' {
        return None;
    }
    let (h, m, s) = (deux_chiffres(&b[..2])?, deux_chiffres(&b[3..5])?, deux_chiffres(&b[6..8])?);
    let reste = &texte[8..];
    let (fraction, reste) = match reste.strip_prefix('.') {
        Some(r) => {
            let n = r.bytes().take_while(u8::is_ascii_digit).count();
            if n == 0 {
                return None;
            }
            (Some(&r[..n]), &r[n..])
        }
        None => (None, reste),
    };
    let fuseau = if reste.is_empty() { None } else { Some(FuseauXsd::lire(reste)?) };
    let a_la_seconde = fraction.is_none_or(|f| f.bytes().all(|c| c == b'0'));
    let fin_du_jour = h == 24 && m == 0 && s == 0 && a_la_seconde;
    if !(h < 24 && m < 60 && s < 60 || fin_du_jour) {
        return None;
    }
    Some((i64::from(h) * 3_600 + i64::from(m) * 60 + i64::from(s), a_la_seconde, fuseau))
}

/// Deux chiffres ASCII, leur valeur ; `None` sinon.
fn deux_chiffres(b: &[u8]) -> Option<u8> {
    (b.len() == 2 && b.iter().all(u8::is_ascii_digit)).then(|| (b[0] - b'0') * 10 + (b[1] - b'0'))
}

/// Le jour d'un jour unix qu'un `Moment` porte : il est dans le calendrier, par sa construction.
fn dans_le_calendrier(jours: i64) -> Jour {
    Jour::depuis_jours_unix(jours).expect("un instant est dans le calendrier")
}

/// Le jour unix (nombre de jours depuis le 01/01/1970) du dernier dimanche du
/// mois `mois` de l'annee `an`.
fn dernier_dimanche(an: i32, mois: u32) -> i64 {
    let dernier = Jour::nouveau(an, mois, crate::date::jours_du_mois(an, mois)).expect("fin de mois");
    let n = dernier.ordinal() - 719_163;
    // Le 01/01/1970 etait un jeudi : (n + 4) % 7 vaut 0 pour un dimanche.
    n - (n + 4).rem_euclid(7)
}

/// Le decalage de Paris sur UTC, en secondes, a `t` secondes unix ; `None` si son jour UTC n'est pas du calendrier.
fn decalage_brut(t: i64) -> Option<i64> {
    let an = Jour::depuis_jours_unix(t.div_euclid(86_400))?.an;
    let debut = dernier_dimanche(an, 3) * 86_400 + 3_600;
    let fin = dernier_dimanche(an, 10) * 86_400 + 3_600;
    Some(if t >= debut && t < fin { 7_200 } else { 3_600 })
}

/// Le decalage de Paris sur UTC, en secondes, a cet instant (arrondi a la seconde d'avant).
pub fn decalage(t: Moment) -> i64 {
    let s = t.entieres();
    match decalage_brut(s) {
        Some(d) => d,
        None => unreachable!("un instant est dans le calendrier : {} s", s),
    }
}

/// Les secondes locales de Paris d'un instant, apres l'arrondi a la microseconde (au pair le plus proche) ; elles restent
/// dans le calendrier (`DERNIER` en garde la marge).
fn local_arrondi(t: Moment) -> i64 {
    let utc = t.a_la_microseconde().entieres();
    match decalage_brut(utc) {
        Some(d) => utc + d,
        None => unreachable!("un instant est dans le calendrier : {} s", utc),
    }
}

/// LA DATE DE MODIFICATION D'UN FICHIER, une donnee du dehors : les secondes et la fraction en nanosecondes, lues
/// contre le calendrier ; `None` : le systeme date le fichier hors du calendrier. Son lecteur dit ce que l'absence veut
/// dire pour lui.
/// Hors Unix (Windows), la meme valeur par `modified()`.
#[cfg(unix)]
pub fn mtime(m: &std::fs::Metadata) -> Option<Moment> {
    use std::os::unix::fs::MetadataExt;
    Moment::de_nanos(m.mtime() as i128 * NANOS + m.mtime_nsec() as i128)
}

#[cfg(not(unix))]
pub fn mtime(m: &std::fs::Metadata) -> Option<Moment> {
    // Windows date tout fichier (`ftLastWriteTime`) ; une date d'avant 1970 compte en secondes negatives, comme `mtime` de
    // Unix.
    let date = match m.modified() {
        Ok(t) => t,
        Err(e) => unreachable!("Windows date tout fichier : {}", e),
    };
    match date.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => Moment::de_duree(d, false),
        Err(avant_1970) => Moment::de_duree(avant_1970.duration(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(t: i64) -> Moment {
        Moment::de_secondes(t).unwrap()
    }

    /// Directive 2000/84/CE, art. 2 : « la période de l'heure d'été commence […] à 1 heure du matin, temps universel, le
    /// dernier dimanche de mars » (`atelier/reference/sources/union-europeenne/eurlex-directive-2000-84-ce-heure-d-ete-20010202.txt`).
    #[test]
    fn l_heure_d_ete_commence_le_dernier_dimanche_de_mars_a_1_h_utc() {
        // 2026 : dimanche 29 mars. 00:59:59 UTC encore l'hiver, 01:00:00 l'ete.
        let t = Jour::nouveau(2026, 3, 29).unwrap().ordinal() - 719_163;
        assert_eq!(decalage(a(t * 86_400 + 3_599)), 3_600);
        assert_eq!(decalage(a(t * 86_400 + 3_600)), 7_200);
        assert_eq!(ecrit::minute::texte(a(t * 86_400 + 3_599)), "2026-03-29T01:59");
        assert_eq!(ecrit::minute::texte(a(t * 86_400 + 3_600)), "2026-03-29T03:00");
    }

    /// Directive 2000/84/CE, art. 3 : la période « se termine […] à 1 heure du matin, temps universel, le dernier dimanche
    /// d'octobre » (même fichier).
    #[test]
    fn l_heure_d_ete_finit_le_dernier_dimanche_d_octobre_a_1_h_utc() {
        // 2026 : dimanche 25 octobre.
        let t = Jour::nouveau(2026, 10, 25).unwrap().ordinal() - 719_163;
        assert_eq!(decalage(a(t * 86_400 + 3_599)), 7_200);
        assert_eq!(decalage(a(t * 86_400 + 3_600)), 3_600);
        assert_eq!(ecrit::minute::texte(a(t * 86_400 + 3_599)), "2026-10-25T02:59");
        assert_eq!(ecrit::minute::texte(a(t * 86_400 + 3_600)), "2026-10-25T02:00");
    }

    /// Directive 2000/84/CE, art. 2 (même fichier) : à 1 h UTC le dernier dimanche de mars, Paris passe de 2 h à 3 h ;
    /// l'heure de Paris `2026-03-29T02:30:00` n'existe pas, elle se refuse.
    #[test]
    fn l_heure_sautee_du_passage_a_l_ete_se_refuse() {
        assert_eq!(ecrit::seconde::lu("2026-03-29T02:30:00"), None);
    }

    /// Directive 2000/84/CE, art. 3 (même fichier) : à 1 h UTC le dernier dimanche d'octobre, Paris revient de 3 h à 2 h ;
    /// l'heure de Paris `2026-10-25T02:30:00` existe deux fois, elle se relit à l'identique.
    #[test]
    fn l_heure_doublee_du_passage_a_l_hiver_se_relit() {
        let m = ecrit::seconde::lu("2026-10-25T02:30:00").unwrap();
        assert_eq!(ecrit::seconde::texte(m), "2026-10-25T02:30:00");
    }

}
