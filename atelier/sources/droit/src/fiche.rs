//! LES FICHES DE LA COMPAGNIE, DECLAREES UNE FOIS : celle d'un salarie (`config/salaries.json`) et celle de l'entreprise
//! avec ses autres etablissements (`config/entreprise.json`). La structure est la forme : chaque champ y a son type, une
//! cle qu'elle ne connait pas se refuse a la lecture (`lire_fichier`, la seule qui les lit), et
//! l'ecriture passe par elle (`socle::ecriture`). `donnees/champs-base.json` en garde la
//! presentation (libelles, aide, conditions d'affichage, ordre) : chacune de ses cles est un champ d'ici, et chaque champ
//! d'ici y a sa ligne (garde par `moteur/tests/fichiers/structure.rs`). La ligne `historique` (forme `histoire`) n'est
//! pas une donnee : la route de la fiche la compose des listes datees.
//!
//! Le fichier est date (`FicheDatee`) : chaque champ y est une liste `[{"depuis"?, "valeur", "note"?}]`
//! (`Datee`) ; la fiche a un jour (`ChampsSalarie`, `ChampsEntreprise`) en est la valeur en vigueur. Chaque case est un
//! `Option` : absente ou nulle, elle est vide (`Lue` la lit). Une valeur s'ecrit nulle quand la fiche ne disait rien
//! avant elle (`dater`).

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use socle::date::Jour;
use socle::mois::Mois;
use socle::echec::R;
use socle::num::Decimal;

use crate::contexte::{IdSalarie, IdSpectacle};

// ------------------------------------------------------------------ une case

/// LA LECTURE D'UNE CASE D'UNE FICHE, un `Option` ordinaire : absente ou nulle, elle est vide. Ce qu'un acte exige
/// d'une case vide se refuse en la nommant (`exigee`, `ecrite`).
pub trait Lue<T> {
    /// La valeur dite.
    fn dite(&self) -> Option<&T>;

    fn est_dite(&self) -> bool {
        self.dite().is_some()
    }

    /// La valeur dite, que l'acte exige : sinon le refus nomme la case (`cle`).
    fn exigee(&self, cle: &str) -> R<&T> {
        self.dite().ok_or_else(|| socle::fichiers::absent(cle))
    }

    /// La meme case, son texte en `String` : pour une table de cases qui les ecrit toutes de meme.
    fn en_texte(&self) -> Option<String>
    where
        T: AsRef<str>,
    {
        self.dite().map(|t| t.as_ref().to_string())
    }

    /// Le texte dit, s'il n'est pas vide.
    fn remplie<'a>(&'a self) -> Option<&'a str>
    where
        T: AsRef<str> + 'a,
    {
        self.dite().map(AsRef::as_ref).filter(|t| !t.is_empty())
    }

    /// Le texte dit.
    fn portee<'a>(&'a self) -> Option<&'a str>
    where
        T: AsRef<str> + 'a,
    {
        self.dite().map(AsRef::as_ref)
    }

    /// Le texte d'une case que l'acte exige : vide, le refus la nomme (`cle`).
    fn ecrite<'a>(&'a self, cle: &str) -> R<&'a str>
    where
        T: AsRef<str> + 'a,
    {
        self.exigee(cle).map(AsRef::as_ref)
    }

    /// Un choix dit, tel qu'il s'ecrit.
    fn code(&self) -> Option<&'static str>
    where
        T: Choix,
    {
        self.dite().map(Choix::code)
    }

    /// Cochee : `true` dit ; ni vide, ni `false`.
    fn oui(&self) -> bool
    where
        T: std::borrow::Borrow<bool>,
    {
        self.dite().is_some_and(|b| *b.borrow())
    }
}

impl<T> Lue<T> for Option<T> {
    fn dite(&self) -> Option<&T> {
        self.as_ref()
    }
}

/// Un choix (`choix!`) : sa valeur ecrite.
pub trait Choix {
    fn code(&self) -> &'static str;
}

/// LA VALEUR D'UN CHANGEMENT DE CASE (`fiche!`) : `null` vide la case ; sinon elle se lit dans le type de la case, chaque
/// texte saisi range d'abord dans la forme du type de sa case (`Declaree::rangee`). `""` n'est pas le vide et se refuse
/// (`socle::forme::Remplie`).
pub fn nulle_ou_dite<'de, D: Deserializer<'de>, T: DeserializeOwned + FormeDeCase>(d: D) -> Result<Option<T>, D::Error> {
    let case = declaree::<T>("");
    let lue = if case.range() {
        socle::forme::rangee_puis_lue::<D, T>(d, &|chemin, texte| rangee_au_chemin(&case, chemin, texte))?
    } else {
        socle::forme::Remplie::<T>::deserialize(d)?
    };
    match lue {
        socle::forme::Remplie::Nulle => Ok(None),
        socle::forme::Remplie::Vide => Err(serde::de::Error::custom("un texte vide ne dit rien : le vide s'écrit null")),
        socle::forme::Remplie::Dite(v) => Ok(Some(v)),
    }
}

/// LES CHANGEMENTS D'UN APPEL (`changements` de la route `enregistrer-fiche`, quelle que soit la fiche), comme ceux de
/// la carte : au moins un, dans l'ordre.
#[derive(Debug, Clone, PartialEq)]
pub struct Changements<C>(pub Vec<C>);

/// La case d'un changement, lue seule : ce que nomme un changement dont la valeur est fautive.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseSeule {
    case: CaseDeFiche,
    #[serde(default, rename = "valeur", deserialize_with = "socle::forme::non_lue")]
    _valeur: socle::forme::NonLue,
}

impl<C: DeserializeOwned> Changements<C> {
    /// LES CHANGEMENTS TAPES `liste`, CHACUN LU SEUL dans le type de sa case : ceux qui se lisent, et la faute de chacun
    /// des autres, toutes ensemble (`FauteDeCase`). Une liste vide, ou un changement qui ne nomme pas une case d'une
    /// fiche, refuse la charge.
    pub fn lire(liste: &socle::forme::Liste) -> R<(Changements<C>, Vec<FauteDeCase>)> {
        if liste.est_vide() {
            return Err(socle::echec::refus("`changements` : au moins un changement, {\"case\": …, \"valeur\": …}"));
        }
        let mut lus = Vec::new();
        let mut fautes = Vec::new();
        for (case, lu) in liste.chacun::<CaseSeule>("valeur").into_iter().zip(liste.chacun::<C>("valeur")) {
            let case = case.map_err(|(_, detail)| socle::echec::refus(format!("`changements` : {}", detail)))?.case;
            match lu {
                Ok(ch) => lus.push(ch),
                Err((chemin, phrase)) => fautes.push(FauteDeCase::de(case, chemin, phrase)),
            }
        }
        Ok((Changements(lus), fautes))
    }
}

/// UNE FAUTE D'UNE CASE SAISIE : la case, le chemin sous elle dans une structure (`[1].siret`, `email`), et la phrase de
/// ce que le type de la case refuse. La case reste vide dans la fiche typee ; la faute se dit a cote, avec toutes les
/// autres du formulaire (`declarations::champs::des_fautes`), et ne se passe pas outre.
#[derive(Debug, Clone, PartialEq)]
pub struct FauteDeCase {
    pub case: CaseDeFiche,
    pub chemin: Option<String>,
    pub phrase: String,
}

impl FauteDeCase {
    /// La faute de la case `case`, au chemin `chemin` sous elle (vide : la case elle-meme).
    fn de(case: CaseDeFiche, chemin: String, phrase: String) -> FauteDeCase {
        let chemin = match chemin.as_str() {
            "" | "?" => None,
            _ => Some(chemin),
        };
        FauteDeCase { case, chemin, phrase }
    }
}

impl FauteDeCase {
    /// LE TEXTE `texte` QUE LE MOTEUR POSE DANS LA CASE `case` (une forme declaree fabriquee de la lisible), lu dans le type
    /// de la case comme un changement `C` : ce que le type refuse est la faute de la case.
    pub fn pose<C: DeserializeOwned>(case: CaseDeFiche, texte: &str) -> Result<C, FauteDeCase> {
        match socle::forme::un_texte_pose::<C>("case", case.nom(), "valeur", texte) {
            Ok(ch) => Ok(ch),
            Err((chemin, phrase)) => Err(FauteDeCase::de(case, chemin, phrase)),
        }
    }
}

pub use socle::ecriture::{MotEcrit, ValeurDeCase};

/// UNE FICHE QUE LA TABLE DES CASES JUGE (`declarations::champs`) : l'entreprise, un salarie, un spectacle, typee. Ce
/// qu'une case ecrit (`ecrite`), si elle est remplie (`dite`), et sa seule porte depuis un formulaire (`lire_cases`).
pub trait FicheSaisie: Sized + Default + Clone {
    const NOM: NomDeFiche;

    /// Ce qu'ecrit la case au bout de `chemin` sous la case `k` (une structure : les cles qui y menent).
    fn ecrite_au_chemin(&self, k: CaseDeFiche, chemin: &[&str]) -> MotEcrit;

    /// LA FICHE TAPEE `o` (un formulaire), case par case (`lire_cases` de sa forme) : la fiche typee et la faute de
    /// chaque case que son type refuse.
    fn lire_cases(o: &socle::forme::Objet) -> R<(Self, Vec<FauteDeCase>)>;

    /// La valeur de la case `k`, ecrite comme le fichier l'ecrit ; vide, aucune.
    fn valeur_ecrite(&self, k: CaseDeFiche) -> Option<ValeurDeCase>;

    /// Aucune case n'y est ecrite (ses notes ne comptent pas).
    fn est_vide(&self) -> bool;

    /// Les cases et les notes que `autre` dit remplacent les siennes.
    fn fondre(&mut self, autre: &Self);

    /// Les cases et les notes que `autre` dit et qu'elle ne dit pas.
    fn completer_de(&mut self, autre: &Self);

    /// La case `k` telle que `autre` la dit.
    fn prendre(&mut self, autre: &Self, k: CaseDeFiche);

    /// Les cases qu'elle dit, dans l'ordre de la forme.
    fn cases_dites(&self) -> Vec<CaseDeFiche>;

    /// Les notes qu'elle dit : leur cle et leur texte.
    fn notes_dites(&self) -> Vec<(&'static str, &str)>;

    /// La note `cle` telle que `autre` la dit.
    fn prendre_la_note(&mut self, autre: &Self, cle: &str);

    /// Ce qu'ecrit la case `k`.
    fn ecrite(&self, k: CaseDeFiche) -> MotEcrit {
        self.ecrite_au_chemin(k, &[])
    }

    /// LA CASE `k` EST-ELLE REMPLIE : une valeur, sauf une case a cocher non cochee ; une structure, une de ses cases
    /// remplie. Zero est une valeur.
    fn dite(&self, k: CaseDeFiche) -> bool {
        dit(&self.ecrite(k))
    }
}

/// Ce mot ecrit dit-il quelque chose (`FicheSaisie::dite`) ?
pub fn dit(m: &MotEcrit) -> bool {
    match m {
        MotEcrit::Vide | MotEcrit::StructureVide => false,
        MotEcrit::Mot(t) | MotEcrit::Nombre(t) => !t.is_empty(),
        MotEcrit::OuiNon(b) => *b,
        MotEcrit::Structure => true,
    }
}

/// UN MONTANT OU UN NOMBRE D'UNE FICHE : un nombre JSON, ou un texte qui se lit en decimal (`12,5` compris), garde tel
/// qu'il est ecrit (`socle::forme::NombreEcrit`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "socle::forme::NombreEcrit")]
pub enum Montant {
    Entier(i64),
    Naturel(u64),
    /// Un nombre ecrit avec sa virgule, dans sa forme la plus courte (`socle::forme::NombreEcrit::Flottant`).
    Flottant(Decimal),
    /// Un texte qui se lit en decimal (`socle::num::decimal_lu`).
    Texte(String),
}

impl Montant {
    /// Le decimal ; un texte se lit en decimal (`12,5` compris). Un texte illisible se refuse.
    pub fn decimal(&self) -> R<Decimal> {
        match self {
            Montant::Entier(i) => Ok(Decimal::from(*i)),
            Montant::Naturel(u) => Ok(Decimal::from(*u)),
            Montant::Flottant(x) => Ok(*x),
            Montant::Texte(s) => socle::num::decimal_lu(s).ok_or_else(|| socle::echec::refus(socle::num::illisible(s))),
        }
    }

    /// Le decimal, un nombre a virgule dans sa forme la plus courte, son echelle telle qu'elle s'ecrit (`130.0`) ; un texte
    /// se lit sans virgule.
    pub fn dec(&self) -> R<Decimal> {
        match self {
            Montant::Texte(s) => socle::num::dec_texte(s),
            autre => autre.decimal(),
        }
    }

    /// Le taux, la quotite ou le coefficient qu'il dit, dans sa forme la plus courte (`socle::num::court`) ; un texte se
    /// lit sans virgule.
    pub fn court(&self) -> R<Decimal> {
        self.dec().map(socle::num::court)
    }

    /// L'entier ecrit : un nombre entier, ou un texte qui se lit en entier ; `None` pour un nombre qui n'est pas un
    /// entier ecrit (un naturel hors des entiers signes, un flottant). Un texte illisible se refuse.
    pub fn entier(&self) -> R<Option<i64>> {
        match self {
            Montant::Entier(i) => Ok(Some(*i)),
            Montant::Texte(s) => s.trim().parse().map(Some).map_err(|_| socle::echec::refus(socle::num::illisible(s))),
            Montant::Naturel(_) | Montant::Flottant(_) => Ok(None),
        }
    }

    /// Tel qu'il est ecrit, pour un message : un texte tel quel, un nombre sous sa forme ecrite.
    pub fn ecrit(&self) -> String {
        match self {
            Montant::Entier(i) => i.to_string(),
            Montant::Naturel(u) => u.to_string(),
            Montant::Flottant(x) => x.to_string(),
            Montant::Texte(s) => s.clone(),
        }
    }
}

impl Serialize for Montant {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Montant::Entier(i) => s.serialize_i64(*i),
            Montant::Naturel(u) => s.serialize_u64(*u),
            Montant::Flottant(x) => socle::forme::ecrire_a_virgule(x, s),
            Montant::Texte(t) => s.serialize_str(t),
        }
    }
}

impl From<socle::forme::NombreEcrit> for Montant {
    fn from(n: socle::forme::NombreEcrit) -> Montant {
        match n {
            socle::forme::NombreEcrit::Entier(i) => Montant::Entier(i),
            socle::forme::NombreEcrit::Naturel(u) => Montant::Naturel(u),
            socle::forme::NombreEcrit::Flottant(x) => Montant::Flottant(x),
            socle::forme::NombreEcrit::Texte(s) => Montant::Texte(s),
        }
    }
}

// ------------------------------------------------------- les textes que les normes bornent

/// La borne d'un texte declare : au plus, ou exactement, sa longueur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Borne {
    AuPlus,
    Exactement,
}

impl Borne {
    fn hors(self, n: usize, longueur: usize) -> bool {
        match self {
            Borne::AuPlus => n > longueur,
            Borne::Exactement => n != longueur,
        }
    }
}

/// Le nombre de caracteres d'un texte.
fn caracteres(t: &str) -> usize {
    t.chars().count()
}

/// Le nombre de chiffres d'un texte (`socle::texte::est_chiffre`).
fn chiffres(t: &str) -> usize {
    t.chars().filter(|c| socle::texte::est_chiffre(*c)).count()
}

/// UN TEXTE DECLARE QUE LES NORMES BORNENT : il en a `LONGUEUR` (`Borne`), comptes par sa mesure, la longueur de la plus
/// courte des normes qui l'ecrivent. Hors de la borne, il se refuse a la lecture (`lire`, que la lecture du fichier
/// appelle) et a la saisie (`FormeDeCase::FAUTE`, par `declarations::champs::fautes_des_types`), en une phrase qui dit
/// quoi faire (`{texte}`, `{n}` sa mesure, `{max}` la borne) : rien ne le couperait. Un texte vide n'est pas le sien :
/// la case vide s'ecrit nulle.
macro_rules! texte_borne {
    (@ranger) => {
        None
    };
    (@ranger $ranger:path) => {
        Some($ranger)
    };
    (@commun $nom:ident, $unite:literal $(, rangee par $ranger:path)?) => {
        impl $nom {
            /// Le texte `texte`, ou ce qui le refuse ; vide, il n'est pas une valeur (la case vide s'ecrit nulle).
            pub fn lire(texte: &str) -> R<$nom> {
                if texte.is_empty() {
                    return Err(socle::echec::refus("Texte vide : une case vide s'écrit nulle (absente), jamais en texte vide."));
                }
                match Self::faute(texte) {
                    Some(f) => Err(socle::echec::refus(f)),
                    None => Ok($nom(texte.to_string())),
                }
            }
        }

        impl AsRef<str> for $nom {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::ops::Deref for $nom {
            type Target = str;
            fn deref(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $nom {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $nom {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let texte = String::deserialize(d)?;
                // Une saisie se range d'abord dans la forme du type (`FormeDeCase::RANGER`), s'il en a une.
                let texte = match <$nom as FormeDeCase>::RANGER {
                    Some(ranger) => socle::forme::range_si_saisi(texte, ranger),
                    None => texte,
                };
                $nom::lire(&texte).map_err(|e| serde::de::Error::custom(e.phrase))
            }
        }

        impl FormeDeCase for $nom {
            const FAUTE: Option<fn(&str) -> Option<String>> = Some($nom::faute);
            const LONGUEUR: Option<(usize, &'static str)> = Some(($nom::LONGUEUR, $unite));
            const RANGER: Option<fn(&str) -> String> = texte_borne!(@ranger $($ranger)?);
        }
    };
    ($(#[$m:meta])* pub struct $nom:ident : $borne:ident $max:literal $unite:literal par $mesure:ident $(rangee par $ranger:path)?, $phrase:literal;) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $nom(String);

        impl $nom {
            pub const LONGUEUR: usize = $max;

            /// Ce qui refuse `texte`, en une phrase qui dit quoi faire ; rien pour un texte vide.
            pub fn faute(texte: &str) -> Option<String> {
                let n = $mesure(texte);
                (!texte.is_empty() && Borne::$borne.hors(n, Self::LONGUEUR)).then(|| format!($phrase, texte = texte, n = n, max = Self::LONGUEUR))
            }
        }

        texte_borne!(@commun $nom, $unite $(, rangee par $ranger)?);
    };
}

texte_borne! {
    /// UNE LIGNE D'ADRESSE DECLAREE de l'entreprise ou d'un etablissement (la voie, puis son complement) : 32 caracteres au
    /// plus. La DPAE en ecrit deux de 32 (L_ADR_EMP_1, positions 188-219, et L_ADR_EMP_2, 220-251 :
    /// `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 262-276) ; la DSN, 50 (S21.G00.06.004 et .007,
    /// S21.G00.11.003 et .006, S10.G00.01.004 et .009, X [1,50] : `atelier/reference/norme-dsn/texte/bloc-S21.G00.06.txt`,
    /// `bloc-S21.G00.11.txt`, `bloc-S10.G00.01.txt`).
    pub struct LigneAdresse : AuPlus 32 "caractères" par caracteres,
        "« {texte} » a {n} caractères : une ligne d'adresse en a {max} au plus, ce que la DPAE écrit (norme DPAE v5.3, \
zone « Adresse de l'établissement »). Passez la suite à la ligne suivante, « Complément d'adresse », ou abrégez";
}

texte_borne! {
    /// LA COMMUNE DECLAREE de l'entreprise ou d'un etablissement : 27 caracteres au plus. La DPAE en ecrit 27
    /// (L_BUR_DIST_EMP, positions 257-283 : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 284-292) ; la DSN,
    /// 50 (S10.G00.01.006, S21.G00.06.006, S21.G00.11.005, X [1,50] : `logiciel/donnees/formats-dsn-base.json`).
    pub struct CommuneEmployeur : AuPlus 27 "caractères" par caracteres,
        "« {texte} » a {n} caractères : la commune d'un établissement en a {max} au plus, ce que la DPAE écrit (norme DPAE \
v5.3, zone « Commune de l'établissement »). Abrégez-la";
}

texte_borne! {
    /// LE CODE POSTAL de l'entreprise ou d'un etablissement : 5 caracteres. La DPAE en ecrit 5 (C_POSTAL_EMP, positions
    /// 252-256 : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 277-283) ; la DSN aussi (S10.G00.01.005,
    /// S21.G00.06.005, S21.G00.11.004, X [5,5] : `logiciel/donnees/formats-dsn-base.json`).
    pub struct CodePostal : Exactement 5 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un code postal en a {max}, ce que la DPAE et la DSN écrivent (norme DPAE v5.3, \
zone « Code postal de l'établissement » ; DSN, S21.G00.11.004). Corrigez-le";
}

texte_borne! {
    /// LE CODE APE (NAF) de l'entreprise : 5 caracteres. La DPAE en ecrit 5 (C_NAF_NN, positions 661-665 :
    /// `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 397-403) ; la DSN aussi (S21.G00.06.003, S21.G00.11.002,
    /// X [5,5] : `logiciel/donnees/formats-dsn-base.json`).
    pub struct CodeNaf : Exactement 5 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un code APE en a {max}, quatre chiffres et une lettre, sans point, ce que la DPAE et \
la DSN écrivent (norme DPAE v5.3, zone « Code NAF » ; DSN, S21.G00.06.003). Corrigez-le";
}

impl CodePostal {
    /// LE CODE POSTAL QU'UNE PIECE IMPORTEE IMPRIME (un bulletin, une AEM, un certificat d'un autre logiciel), ses blancs
    /// de bord retires : hors de sa forme, la case reste vide et se redemande (`Nir::d_une_piece`).
    pub fn d_une_piece(texte: &str) -> Option<CodePostal> {
        match CodePostal::lire(texte.trim()) {
            Ok(c) => Some(c),
            Err(_) => None,
        }
    }
}

impl CodeNaf {
    /// LE CODE APE QU'UNE PIECE IMPORTEE IMPRIME, ses blancs de bord retires, et sans le point de la notation de l'INSEE
    /// (« 90.01Z », les deux chiffres de la division puis le point : `atelier/reference/sources/insee-naf-2025/`, ses
    /// classeurs et son `LISEZ-MOI.md`) : la DPAE et la DSN l'ecrivent sans lui (« 9001Z »). Hors de sa forme, la case
    /// reste vide et se redemande (`Nir::d_une_piece`).
    pub fn d_une_piece(texte: &str) -> Option<CodeNaf> {
        let t = texte.trim();
        let sans_point = match (t.get(..2), t.get(2..3), t.get(3..)) {
            (Some(division), Some("."), Some(reste)) => format!("{}{}", division, reste),
            _ => t.to_string(),
        };
        match CodeNaf::lire(&sans_point) {
            Ok(c) => Some(c),
            Err(_) => None,
        }
    }
}

texte_borne! {
    /// LE CODE PCS-ESE d'un emploi (S21.G00.40.004) : « Table PCSESE - Code profession et catégorie socioprofessionnelle
    /// PCS-ESE », « X [4,4] » (cahier technique DSN 2026.1, p. 183, `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`
    /// l. 200-215). La table est celle de l'Insee (`atelier/reference/sources/insee-pcs-ese-2017-liste.xls`), non livree :
    /// la forme est celle du cahier, quatre caracteres, et rien de plus (regle 7). La fiche du salarie, la carte,
    /// l'engagement et le catalogue des metiers le portent.
    pub struct CodePcsEse : Exactement 4 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un code PCS-ESE en a {max} (465b), ce que la DSN écrit \
(S21.G00.40.004). Corrigez-le";
}

impl CodePcsEse {
    /// UN EMPLOI D'ARTISTE DU SPECTACLE : un code du groupe 354. Les rubriques deposees de ce groupe sont des artistes
    /// « dans le cadre d'un spectacle vivant ou audiovisuel » (Insee, PCS-ESE 2017,
    /// `atelier/reference/sources/insee-pcs-ese-2017-fiches/354b.txt`, `354c.txt`, `354e.txt`, l. 34-35), et les
    /// « Artistes du cirque et des spectacles divers » (`354f.txt`, l. 34). INFERENCE : le groupe entier se lit ainsi ;
    /// ses autres rubriques ne sont pas deposees.
    pub fn est_un_artiste_du_spectacle(&self) -> bool {
        self.0.starts_with("354")
    }
}

/// LE CODE EMPLOI DECLARE d'un emploi (S21.G00.40.005, « Code complément PCS-ESE ») : « X [2,6] », d'une des tables
/// « CCP - Code Complément PCS-ESE, NEHMED [...], NEHNMED [...], ART - Code détaillé des professions du spectacle
/// (complément PCS-ESE), NET, NNE et FESAIEG [...], CMET » (cahier technique DSN 2026.1, p. 183-185,
/// `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt` l. 217-354). Pour le spectacle, CCH-18 (l. 325-329) : « une
/// valeur présente dans la table de nomenclature des emplois du spectacle ou "999SPT" pour les emplois permanents du
/// spectacle ou "50" pour les mannequins ou "51" pour les artistes de corrida ». Ce n'est donc pas toujours le code d'un
/// metier (`emplois::CodeEmploi`) : il peut n'en nommer aucun. Les autres tables ne sont pas deposees : la forme est
/// celle du cahier, de 2 a 6 caracteres (`X`), et rien de plus (regle 7). La fiche du salarie, la carte, l'engagement et
/// sa paie ecrite le portent. Saisi, il se range sans ses blancs de bord et en capitales.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct CodeEmploiFt(String);

impl CodeEmploiFt {
    /// Le code `texte`, ou la phrase qui le refuse.
    pub fn lire(texte: &str) -> R<CodeEmploiFt> {
        if crate::organismes::de_nature_x(texte, 2, 6) {
            Ok(CodeEmploiFt(texte.to_string()))
        } else {
            Err(socle::refus!(
                "Code emploi {} : le code emploi déclaré (complément PCS-ESE, S21.G00.40.005) s'écrit de 2 à 6 caractères, sans blanc au début ni à la fin. Corrigez-le",
                socle::fichiers::repr(texte)
            ))
        }
    }

    /// Le code d'un emploi permanent du spectacle, « 999SPT » (CCH-18, ci-dessus) : il ne nomme aucun metier.
    pub fn est_permanent(&self) -> bool {
        self.0 == "999SPT"
    }
}

/// Le code d'un metier est un code de la nomenclature des emplois du spectacle, la table ART de 40.005 (CCH-18,
/// ci-dessus) : il se declare tel quel. Ses codes ont six caracteres (`nomenclature-emplois-ft.json`).
impl From<&crate::emplois::CodeEmploi> for CodeEmploiFt {
    fn from(c: &crate::emplois::CodeEmploi) -> CodeEmploiFt {
        CodeEmploiFt(c.as_str().to_string())
    }
}

/// LE CODE DU SERVICE DE SANTE AU TRAVAIL, celui de la DPAE : « MT » et trois chiffres, 5 caracteres. La norme DPAE
/// (C_MT_DCL, positions 938-947 : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 485-497) : « Il est
/// attendu ici un code du type MT+ 3 positions numériques. Ex: MT542 [...] Si vous disposez d'un centre de médecine
/// interne à l'établissement, renseignez MT999 » ; la zone en a 10, cadree a droite. La DSN 2027 ecrit la medecine
/// interne « MT01 - MEDECINE INTERNE ETABLISSEMENT » en S21.G00.11.025 (`declarations::dsn::spst_ecrit`, qui la tire de
/// `MT999`, `INTERNE`) ; l'identifiant d'un service externe y est defini par une fiche consigne qui n'est pas deposee.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CodeMedecine(String);

impl CodeMedecine {
    pub const LONGUEUR: usize = 5;
    /// La medecine interne a l'etablissement, dans le code de la DPAE (l. 496-497).
    pub const INTERNE: &'static str = "MT999";

    /// Ce qui refuse `texte`, en une phrase qui dit quoi faire ; rien pour un texte vide.
    pub fn faute(texte: &str) -> Option<String> {
        let c: Vec<char> = texte.chars().collect();
        let bon = c.len() == Self::LONGUEUR && c[0] == 'M' && c[1] == 'T' && c[2..].iter().all(|x| x.is_ascii_digit());
        (!texte.is_empty() && !bon).then(|| {
            format!(
                "« {} » n'est pas un code de service de santé au travail : la DPAE l'attend en MT suivi de trois chiffres, \
MT542 par exemple, et MT999 pour une médecine interne à l'établissement (norme DPAE v5.3, zone « Code centre de médecine \
du travail »). Il se lit sur l'attestation d'adhésion à ce service",
                texte
            )
        })
    }

    /// La compagnie a-t-elle sa propre medecine du travail (`INTERNE`) ?
    pub fn interne(&self) -> bool {
        self.0 == Self::INTERNE
    }
}

texte_borne!(@commun CodeMedecine, "caractères");

texte_borne! {
    /// LE TELEPHONE de l'employeur (le contact DSN, le signataire) : 11 chiffres au plus. La DPAE en ecrit les chiffres, 11
    /// au plus (N_TEL_EMP, positions 542-552 : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 380-385) ; la
    /// DSN, le texte, 20 caracteres au plus (S10.G00.02.005, S20.G00.07.002, X [10,20] :
    /// `logiciel/donnees/formats-dsn-base.json`), que son controle des formats tient.
    pub struct Telephone : AuPlus 11 "chiffres" par chiffres,
        "« {texte} » a {n} chiffres : un téléphone en a {max} au plus, ce que la DPAE écrit (norme DPAE v5.3, zone « Numéro de téléphone »). \
Écrivez-le sans l'indicatif international, en dix chiffres (0102030405)";
}

texte_borne! {
    /// LE NOM DECLARE d'un salarie : 32 caracteres au plus. La DPAE en ecrit 32 (L_NOM_PATRO_SAL, positions 284-315 :
    /// `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 293-300) ; la DSN, 80 (S21.G00.30.002, X [1,80] :
    /// `logiciel/donnees/formats-dsn-base.json`).
    pub struct NomDeclare : AuPlus 32 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un nom déclaré en a {max} au plus, ce que la DPAE écrit (norme DPAE v5.3, \
zone « Nom du salarié »). Abrégez-le";
}

texte_borne! {
    /// LE PRENOM DECLARE d'un salarie : 32 caracteres au plus. La DPAE en ecrit 32 (L_PRENOMS_SAL, positions 316-347 :
    /// `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 301-308) ; la DSN, 80 (S21.G00.30.004, X [1,80] :
    /// `logiciel/donnees/formats-dsn-base.json`).
    pub struct PrenomDeclare : AuPlus 32 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un prénom déclaré en a {max} au plus, ce que la DPAE écrit (norme DPAE v5.3, \
zone « Prénom du salarié »). Abrégez-le";
}

texte_borne! {
    /// LE LIEU DE NAISSANCE DECLARE d'un salarie (la commune ou le pays) : 24 caracteres au plus. La DPAE en ecrit 24
    /// (L_LIEU_NAISS_SAL, positions 369-392 : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 323-332) ; la
    /// DSN, 30 (S21.G00.30.007, X [1,30] : `logiciel/donnees/formats-dsn-base.json`).
    pub struct LieuNaissance : AuPlus 24 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un lieu de naissance déclaré en a {max} au plus, ce que la DPAE écrit (norme DPAE \
v5.3, zone « Lieu de naissance du salarié »). Abrégez-le";
}

texte_borne! {
    /// LE DEPARTEMENT DE NAISSANCE d'un salarie : 2 caracteres. La DPAE en ecrit 2 (C_DEPT_NAISS_SAL, positions 828-829 :
    /// `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 427-433) ; la DSN aussi (S21.G00.30.014, X [2,2] :
    /// `logiciel/donnees/formats-dsn-base.json`).
    pub struct DepartementNaissance : Exactement 2 "caractères" par caracteres,
        "« {texte} » a {n} caractères : un département de naissance en a {max} (75, 2A, 99 pour une naissance à \
l'étranger), ce que la DPAE et la DSN écrivent (norme DPAE v5.3, zone « Code du département de naissance » ; DSN, S21.G00.30.014). Corrigez-le";
}

/// S21.G00.30.014 (`atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.30.txt`, l. 254-262) : « Pour les personnes nées
/// à l'étranger, renseigner 99 » ; « La valeur 20 n'est acceptée que pour les personnes nées avant 1976 », l'ancien code
/// de la Corse (2A et 2B depuis).
impl DepartementNaissance {
    /// 99 : une naissance a l'etranger.
    pub fn est_etranger(&self) -> bool {
        self.0 == "99"
    }

    /// 20 : l'ancien code de la Corse, avant 1976.
    pub fn est_l_ancienne_corse(&self) -> bool {
        self.0 == "20"
    }
}

texte_borne! {
    /// LE NUMERO DE SECURITE SOCIALE d'un salarie, sans sa cle : 13 caracteres. La DPAE en ecrit 13 (N_SECU_SOC, positions
    /// 348-360 : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 309-315) ; la DSN aussi (S21.G00.30.001,
    /// X [13,13] : `logiciel/donnees/formats-dsn-base.json`). Saisi, il se range sans aucun blanc et en capitales
    /// (`Nir::rangee`) : un departement corse saisi « 2a » s'ecrit « 2A ».
    pub struct Nir : Exactement 13 "caractères" par caracteres rangee par Nir::rangee,
        "« {texte} » a {n} caractères : un numéro de sécurité sociale en a {max}, sans sa clé, ce que la DPAE et la DSN \
écrivent (norme DPAE v5.3, zone « Numéro de sécurité sociale » ; DSN, S21.G00.30.001). Les deux derniers chiffres vont dans la case « Clé »";
}

impl Nir {
    /// Le texte saisi `texte` dans sa forme, avant d'etre lu (`FormeDeCase::RANGER`) : sans aucun blanc, en capitales.
    pub fn rangee(texte: &str) -> String {
        sans_blancs(texte).to_uppercase()
    }

    /// Le sexe que dit son premier chiffre (`Sexe::du_nir`).
    pub fn sexe(&self) -> Option<Sexe> {
        Sexe::du_nir(&self.0)
    }

    /// UN NIR IMPRIME AVEC OU SANS SA CLE, deja compacte (`texte`) : treize caracteres, ou quinze dont les deux derniers
    /// sont la cle (INSEE, schema NIR) ; la seule coupe « 13 + cle ». Une autre longueur : aucun.
    pub fn et_sa_cle(texte: &str) -> Option<(&str, Option<&str>)> {
        match texte.chars().count() {
            13 => Some((texte, None)),
            15 => {
                let (i, _) = texte.char_indices().nth(13)?;
                Some((&texte[..i], Some(&texte[i..])))
            }
            _ => None,
        }
    }

    /// LE NIR QU'UNE PIECE IMPORTEE PORTE (une DSN lue, une AEM, une AER, un certificat, un bulletin d'autrui) : hors de sa
    /// forme, la case reste vide, comme un code hors de nos listes ; la piece d'un tiers se lit, elle ne se refuse pas
    /// pour lui. Les retours de la norme le lisent strictement (`lire`).
    pub fn d_une_piece(texte: &str) -> Option<Nir> {
        match Nir::lire(texte) {
            Ok(n) => Some(n),
            Err(_) => None,
        }
    }
}

/// LE MATRICULE d'un salarie, son numero dans la compagnie (S21.G00.30.019, « Matricule de l'individu dans
/// l'entreprise ») : « Le choix de la valeur que prend le matricule revient à l'employeur », « X [1,30] » (cahier
/// technique DSN 2026.1, `atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt` l. 327-331 ;
/// `logiciel/donnees/formats-dsn-base.json`, S21.G00.30.019). De 1 a 30 caracteres, et rien de plus (regle 7) ; il
/// s'ecrit tel qu'il a ete saisi. La fiche du salarie le porte, la DSN et le signalement le declarent, la DSN partie le
/// garde (`declarations::changements_individu::Declaree`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Matricule(String);

impl Matricule {
    pub const LONGUEUR: usize = 30;

    /// Le matricule `texte`, ou la phrase qui le refuse.
    pub fn lire(texte: &str) -> R<Matricule> {
        let n = caracteres(texte);
        if (1..=Self::LONGUEUR).contains(&n) {
            Ok(Matricule(texte.to_string()))
        } else {
            Err(socle::refus!(
                "Matricule {} : il a {} caractères, la DSN en écrit de 1 à {} (S21.G00.30.019). Corrigez-le",
                socle::fichiers::repr(texte),
                n,
                Self::LONGUEUR
            ))
        }
    }
}

/// Le matricule saisi se garde tel quel : le choix de sa valeur revient a l'employeur.
fn tel_quel(texte: &str) -> String {
    texte.to_string()
}

/// CE QU'UNE NOTION LUE PORTE AVEC SON TEXTE : elle se lit de son seul lecteur (`lire`, a la lecture du fichier comme a
/// la porte) et se refuse a la saisie par sa faute (`FormeDeCase::FAUTE`) ; sa saisie se range d'abord dans sa forme
/// (`$ranger`, `FormeDeCase::RANGER`), qu'elle soit la case d'une fiche ou la valeur d'une charge
/// (`socle::forme::range_si_saisi`) ; elle s'ecrit telle qu'elle a ete lue.
macro_rules! notion_lue {
    ($nom:ident rangee par $ranger:path, faute a part) => {
        impl AsRef<str> for $nom {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::ops::Deref for $nom {
            type Target = str;
            fn deref(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $nom {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $nom {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                $nom::lire(&socle::forme::range_si_saisi(String::deserialize(d)?, $nom::rangee)).map_err(|e| serde::de::Error::custom(e.phrase))
            }
        }

        impl FormeDeCase for $nom {
            const FAUTE: Option<fn(&str) -> Option<String>> = Some($nom::faute);
            const RANGER: Option<fn(&str) -> String> = Some($nom::rangee);
        }

        impl $nom {
            /// Le texte saisi `texte` dans sa forme, avant d'etre lu (`FormeDeCase::RANGER`).
            pub fn rangee(texte: &str) -> String {
                $ranger(texte)
            }
        }
    };
    ($nom:ident rangee par $ranger:path) => {
        notion_lue!($nom rangee par $ranger, faute a part);

        impl $nom {
            /// Ce qui refuse `texte` a la saisie, en une phrase qui dit quoi faire ; rien pour un texte vide (la case vide
            /// s'ecrit nulle).
            pub fn faute(texte: &str) -> Option<String> {
                if texte.is_empty() {
                    return None;
                }
                $nom::lire(texte).err().map(|e| e.phrase)
            }
        }
    };
}

/// UNE SAISIE SANS SES BLANCS DE BORD (`socle::texte::blanc`) : la forme rangee d'un texte que son type lit tel quel.
pub fn sans_blancs_de_bord(texte: &str) -> String {
    texte.trim_matches(socle::texte::blanc).to_string()
}

/// UNE SAISIE DE CHIFFRES SANS AUCUN BLANC : la forme rangee d'un identifiant fait de chiffres (`identifiant_de_chiffres!`),
/// que ses blancs, autour ou entre ses groupes (« 123 456 789 00012 »), ne changent pas.
pub fn sans_blancs(texte: &str) -> String {
    texte.chars().filter(|c| !socle::texte::blanc(*c)).collect()
}

/// Le texte fait d'exactement `n` chiffres ASCII.
fn de_chiffres(texte: &str, n: usize) -> bool {
    texte.len() == n && texte.bytes().all(|b| b.is_ascii_digit())
}

/// UN IDENTIFIANT FAIT D'UN NOMBRE FIXE DE CHIFFRES : il en a exactement `LONGUEUR`, sinon il se refuse a la lecture en
/// une phrase qui dit quoi faire (`{texte}`, `{max}` sa longueur).
macro_rules! identifiant_de_chiffres {
    ($(#[$m:meta])* pub struct $nom:ident : $n:literal, $phrase:literal;) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $nom(String);

        impl $nom {
            pub const LONGUEUR: usize = $n;

            /// L'identifiant `texte`, ou la phrase qui le refuse.
            pub fn lire(texte: &str) -> R<$nom> {
                if de_chiffres(texte, Self::LONGUEUR) {
                    Ok($nom(texte.to_string()))
                } else {
                    Err(socle::echec::refus(format!($phrase, texte = texte, max = Self::LONGUEUR)))
                }
            }
        }

        notion_lue!($nom rangee par sans_blancs);
    };
}

/// LA CLE DE LUHN D'UN SIREN OU D'UN SIRET : « on parcourt les chiffres [...], on multiplie par 2 les chiffres de rang
/// impair et on fait ensuite la somme de tous les chiffres obtenus [...] Le contrôle de parité est correct si le
/// résultat obtenu est un multiple de 10 » (INSEE, schema SIRET, 8.1 : `atelier/reference/sources/insee-nir-siret/`).
/// Le texte dit que ce controle « peut être utilisé pour réduire les risques d'erreurs de frappe » : il n'entre pas dans
/// le lecteur, et une cle fausse se dit a la saisie sans refuser la fiche (`declarations::champs`).
fn cle_de_luhn_juste(chiffres: &str) -> bool {
    let total: u32 = chiffres
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, b)| {
            let n = u32::from(b - b'0');
            if i % 2 == 1 {
                let d = n * 2;
                if d > 9 {
                    d - 9
                } else {
                    d
                }
            } else {
                n
            }
        })
        .sum();
    total % 10 == 0
}

identifiant_de_chiffres! {
    /// LE SIREN d'une entreprise : 9 chiffres (INSEE, schema SIRET, « SIRENType », et la DSN, S21.G00.06.001, X [9,9] :
    /// `atelier/reference/sources/insee-nir-siret/`, `logiciel/donnees/formats-dsn-base.json`).
    pub struct Siren : 9,
        "« {texte} » n'est pas un SIREN : il a {max} chiffres, sans espace (INSEE ; DSN, S21.G00.06.001). Corrigez-le";
}

identifiant_de_chiffres! {
    /// LE NIC d'un etablissement : « 5 chiffres ajoutés au numéro SIREN de l'entreprise pour identifier un
    /// établissement » (INSEE, schema SIRET, 4.1 ; DSN, S21.G00.11.001, X [5,5]).
    pub struct Nic : 5,
        "« {texte} » n'est pas un NIC : il a {max} chiffres, ceux qui suivent le SIREN dans le SIRET (INSEE ; DSN, \
S21.G00.11.001). Corrigez-le";
}

identifiant_de_chiffres! {
    /// LE SIRET d'un etablissement : « un identifiant numérique de 14 chiffres composé du SIREN (9 chiffres) et d'un
    /// numéro interne de classement de 5 chiffres (NIC) » (INSEE, schema SIRET, 5.1 :
    /// `atelier/reference/sources/insee-nir-siret/`).
    pub struct Siret : 14,
        "« {texte} » n'est pas un SIRET : il a {max} chiffres, le SIREN puis le NIC, sans espace (INSEE). Corrigez-le";
}

/// L'IDCC, L'IDENTIFIANT D'UNE CONVENTION COLLECTIVE : 4 chiffres (DSN, « Code convention collective applicable »,
/// S21.G00.40.017, et « Code convention collective principale », S21.G00.11.022, X [4,4] :
/// `logiciel/donnees/formats-dsn-base.json`). C'est la cle des tables par convention : il s'ecrit et se trie
/// comme son texte (quatre chiffres : l'ordre du texte est celui des nombres), et une table qu'il indexe s'ecrit dans
/// l'ordre de ses cles telles qu'ecrites. Une convention que le moteur nomme lui-meme est une constante (`du_moteur`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Idcc(std::borrow::Cow<'static, str>);

impl Idcc {
    pub const LONGUEUR: usize = 4;

    /// L'identifiant `texte`, ou la phrase qui le refuse.
    pub fn lire(texte: &str) -> R<Idcc> {
        if de_chiffres(texte, Self::LONGUEUR) {
            Ok(Idcc(std::borrow::Cow::Owned(texte.to_string())))
        } else {
            Err(socle::echec::refus(format!(
                "« {texte} » n'est pas un IDCC : une convention collective se dit en {max} chiffres, 3090 par exemple \
(DSN, S21.G00.40.017). Corrigez-le",
                texte = texte,
                max = Self::LONGUEUR
            )))
        }
    }

    /// UNE CONVENTION QUE LE MOTEUR NOMME, ecrite en Rust : une constante (`const X: Idcc = Idcc::du_moteur("…")`),
    /// dont le compilateur refuse un texte qui n'est pas fait de quatre chiffres.
    pub const fn du_moteur(texte: &'static str) -> Idcc {
        let octets = texte.as_bytes();
        assert!(octets.len() == Self::LONGUEUR, "un IDCC a quatre chiffres");
        let mut i = 0;
        while i < octets.len() {
            assert!(octets[i].is_ascii_digit(), "un IDCC a quatre chiffres");
            i += 1;
        }
        Idcc(std::borrow::Cow::Borrowed(texte))
    }
}

notion_lue!(Idcc rangee par sans_blancs);

/// Les conventions que le moteur nomme lui-meme (une regle propre a l'une d'elles), par leur IDCC ; la CCNEAC :
/// `clauses::IDCC_CCNEAC`.
pub const IDCC_1734: Idcc = Idcc::du_moteur("1734");
pub const IDCC_1790: Idcc = Idcc::du_moteur("1790");
pub const IDCC_2642: Idcc = Idcc::du_moteur("2642");
pub const IDCC_3090: Idcc = Idcc::du_moteur("3090");
pub const IDCC_3097: Idcc = Idcc::du_moteur("3097");
pub const IDCC_3241: Idcc = Idcc::du_moteur("3241");

impl Siren {
    /// Sa cle de Luhn est-elle juste (`cle_de_luhn_juste`) ?
    pub fn cle_juste(&self) -> bool {
        cle_de_luhn_juste(&self.0)
    }
}

impl Siret {
    /// Le SIRET de l'etablissement `nic` de l'entreprise `siren` : l'un puis l'autre (INSEE, 5.1).
    pub fn de(siren: &Siren, nic: &Nic) -> Siret {
        Siret(format!("{}{}", siren.0, nic.0))
    }

    /// Son SIREN, ses 9 premiers chiffres.
    pub fn siren(&self) -> Siren {
        Siren(self.0[..Siren::LONGUEUR].to_string())
    }

    /// Son NIC, ses 5 derniers chiffres.
    pub fn nic(&self) -> Nic {
        Nic(self.0[Siren::LONGUEUR..].to_string())
    }

    /// Sa cle de Luhn est-elle juste (`cle_de_luhn_juste`) ?
    pub fn cle_juste(&self) -> bool {
        cle_de_luhn_juste(&self.0)
    }

    /// LE SIRET QU'UNE PIECE IMPORTEE IMPRIME (un bulletin d'un autre logiciel), ses chiffres : hors de sa forme, la case
    /// reste vide et se redemande ; la piece d'un tiers se lit, elle ne se refuse pas pour lui (`Nir::d_une_piece`).
    pub fn d_une_piece(texte: &str) -> Option<Siret> {
        match Siret::lire(texte) {
            Ok(s) => Some(s),
            Err(_) => None,
        }
    }
}

/// UN IBAN (ISO 13616) : sa forme et sa cle sont celles de `socle::iban::faute_iban` (S21.G00.20.004 de la DSN, et le
/// communique IBAN de la Banque centrale de Turquie : `atelier/reference/sources/iban/`), qui le lit compacte (sans
/// blancs, en majuscules : `socle::iban::iban_compact`, l'etape preliminaire de la validation, annexe 3). Il se garde
/// tel qu'il est ecrit ; sa saisie s'ecrit compacte (`FormeDeCase::RANGER`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Iban(String);

impl Iban {
    /// L'IBAN `texte`, ou la phrase qui le refuse.
    pub fn lire(texte: &str) -> R<Iban> {
        match socle::iban::faute_iban(texte) {
            Some(f) => Err(socle::refus!("« {} » n'est pas un IBAN : {}", texte, f)),
            None => Ok(Iban(texte.to_string())),
        }
    }
}

notion_lue!(Iban rangee par socle::iban::iban_compact);

/// UN BIC (ISO 9362) : sa forme est celle de `socle::iban::faute_bic` (S21.G00.20.003 de la DSN), qui le lit compacte.
/// Il se garde tel qu'il est ecrit ; sa saisie s'ecrit compacte (`FormeDeCase::RANGER`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Bic(String);

impl Bic {
    /// Le BIC `texte`, ou la phrase qui le refuse.
    pub fn lire(texte: &str) -> R<Bic> {
        match socle::iban::faute_bic(texte) {
            Some(f) => Err(socle::refus!("« {} » n'est pas un BIC : {}", texte, f)),
            None => Ok(Bic(texte.to_string())),
        }
    }
}

notion_lue!(Bic rangee par socle::iban::iban_compact);

/// UNE ADRESSE DE COURRIEL : de 6 a 100 caracteres, ce que dit la norme DSN, et rien d'autre (« Adresse mél »,
/// S21.G00.30.018, X [6,100] : `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.30.txt`, l. 294-300 ; « Adresse mél
/// du contact », S20.G00.07.003, X [6,100] : `bloc-S20.G00.05.txt`, l. 312-319). Aucun texte depose ne dit la forme d'une
/// adresse. Elle se garde telle qu'elle est ecrite, capitales comprises (RFC 5321, § 2.4 :
/// `ietf/ietf-rfc5321-simple-mail-transfer-protocol-20081001.txt`, l. 869).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Courriel(String);

impl Courriel {
    /// Sa longueur, en caracteres, au moins et au plus (X [6,100]).
    pub const LONGUEUR: (usize, usize) = (6, 100);

    /// L'adresse `texte`, ou la phrase qui la refuse.
    pub fn lire(texte: &str) -> R<Courriel> {
        let n = texte.chars().count();
        let (min, max) = Self::LONGUEUR;
        if (min..=max).contains(&n) {
            Ok(Courriel(texte.to_string()))
        } else {
            Err(socle::refus!(
                "« {} » a {} caractères : une adresse de courriel en a de {} à {}, ce que la DSN écrit (S21.G00.30.018, S20.G00.07.003). Corrigez-la",
                texte,
                n,
                min,
                max
            ))
        }
    }
}

notion_lue!(Courriel rangee par sans_blancs_de_bord);

/// LA CLE D'UN NIR, a cote de lui (`Nir`) : « le complément à 97 du reste de la division par 97 » du NIR, deux chiffres,
/// de 01 a 97 (INSEE, schema NIR, 4.1 : `atelier/reference/sources/insee-nir-siret/`). Qu'elle soit celle de son NIR se
/// dit a la saisie (`declarations::champs::cle_nir`), sans refuser la fiche.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct CleNir(String);

impl CleNir {
    /// La cle `texte`, ou la phrase qui la refuse.
    pub fn lire(texte: &str) -> R<CleNir> {
        let valeur = if de_chiffres(texte, 2) { texte.parse::<u8>().ok() } else { None };
        match valeur {
            Some(1..=97) => Ok(CleNir(texte.to_string())),
            _ => Err(socle::refus!(
                "« {} » n'est pas la clé d'un numéro de sécurité sociale : deux chiffres, de 01 à 97, ceux qui suivent les 13 autres (INSEE)",
                texte
            )),
        }
    }

    /// La cle du nombre `nombre` qu'ecrit un NIR : le complement a 97 de son reste par 97, de 01 a 97 (INSEE, 4.1).
    fn du_nombre(nombre: u64) -> CleNir {
        CleNir(format!("{:02}", 97 - nombre % 97))
    }

    /// LA CLE DU NIR `nir` : 97 - (NIR mod 97), sur deux chiffres (`atelier/reference/sources/insee-nir-siret/`, INSEE,
    /// schema NIR, 4.1). En Corse, 2A et 2B au 6e-7e rang valent 19 et 18 (equivalent, par inference, au zero moins
    /// 1 000 000 ou 2 000 000 du texte). Aucune si un de ses caracteres n'est pas un chiffre.
    pub fn du_nir(nir: &Nir) -> Option<CleNir> {
        CleNir::des_treize(&nir.0)
    }

    /// La cle de treize caracteres `n` deja ranges (en capitales), la seule arithmetique de la cle : celle d'un `Nir`
    /// (`du_nir`), et celle d'un NIR qu'une saisie en cours ecrit (`declarations::champs::cle_nir`).
    pub fn des_treize(n: &str) -> Option<CleNir> {
        let rang_corse: String = n.chars().skip(5).take(2).collect();
        let n = match rang_corse.as_str() {
            "2A" => n.replacen("2A", "19", 1),
            "2B" => n.replacen("2B", "18", 1),
            _ => n.to_string(),
        };
        // Les chiffres decimaux de toutes les ecritures comptent comme des chiffres.
        let mut x: u64 = 0;
        for c in n.chars() {
            x = x * 10 + u64::from(valeur_decimale(c)?);
        }
        Some(CleNir::du_nombre(x))
    }
}

/// La valeur d'un chiffre decimal Unicode (categorie Nd).
fn valeur_decimale(c: char) -> Option<u32> {
    if let Some(d) = c.to_digit(10) {
        return Some(d);
    }
    // Le zero de chaque suite de dix chiffres decimaux (Unicode 15).
    const ZEROS: [u32; 65] = [
        0x660, 0x6F0, 0x7C0, 0x966, 0x9E6, 0xA66, 0xAE6, 0xB66, 0xBE6, 0xC66, 0xCE6, 0xD66, 0xDE6, 0xE50, 0xED0, 0xF20, 0x1040, 0x1090, 0x17E0,
        0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90, 0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10,
        0x104A0, 0x10D30, 0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0, 0x11650, 0x116C0, 0x11730, 0x118E0, 0x11950, 0x11C50,
        0x11D50, 0x11DA0, 0x16A60, 0x16AC0, 0x16B50, 0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0, 0x1E4F0, 0x1E950,
    ];
    let p = c as u32;
    ZEROS.iter().find(|z| (**z..**z + 10).contains(&p)).map(|z| p - z).or_else(|| (0x1FBF0..0x1FBFA).contains(&p).then(|| p - 0x1FBF0))
}

notion_lue!(CleNir rangee par sans_blancs_de_bord);

/// LE NUMERO D'OBJET D'UN SPECTACLE : « le numéro unique constitué de 12 caractères alphanumériques qui est attribué par le
/// CNCS pour chaque spectacle ou chaque production » (S21.G00.40.050, X [12,12] :
/// `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.40.txt`, l. 1430-1441). Qu'il ne se declare qu'en natures 02 et
/// 10 (CCH-11, regle 6 d'AGENTS) tient au contrat, pas au numero : la DSN le dit.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct NumeroObjetSpectacle(String);

impl NumeroObjetSpectacle {
    pub const LONGUEUR: usize = 12;

    /// Le numero `texte`, ou la phrase qui le refuse.
    pub fn lire(texte: &str) -> R<NumeroObjetSpectacle> {
        if texte.len() == Self::LONGUEUR && texte.bytes().all(|b| b.is_ascii_alphanumeric()) {
            Ok(NumeroObjetSpectacle(texte.to_string()))
        } else {
            Err(socle::refus!(
                "« {} » n'est pas un numéro d'objet : il a {} lettres ou chiffres, sans espace, tel que le CNCS l'attribue (DSN, S21.G00.40.050)",
                texte,
                Self::LONGUEUR
            ))
        }
    }

    /// LE NUMERO D'OBJET QU'UNE PIECE IMPORTEE PORTE (une AEM, une AER, un bulletin d'autrui) : sans ses
    /// blancs de bord ; a blanc ou hors de sa forme, la case reste vide, comme un code hors de nos listes (la piece d'un
    /// tiers se lit, elle ne se refuse pas pour lui).
    pub fn d_une_piece(texte: &str) -> Option<NumeroObjetSpectacle> {
        match NumeroObjetSpectacle::lire(&NumeroObjetSpectacle::rangee(texte)) {
            Ok(n) => Some(n),
            Err(_) => None,
        }
    }

    /// Le meme numero que `autre`, sans regarder la casse : une piece et une fiche l'ecrivent en capitales ou non.
    pub fn meme(&self, autre: &NumeroObjetSpectacle) -> bool {
        self.0.eq_ignore_ascii_case(&autre.0)
    }

    /// Le meme numero, en capitales : la cle sous laquelle l'import regroupe ce que les pieces en impriment.
    pub fn en_capitales(&self) -> NumeroObjetSpectacle {
        NumeroObjetSpectacle(self.0.to_ascii_uppercase())
    }

    /// L'ANNEE OU L'EMPLOYEUR A DEMANDE LE NUMERO : ses deux premiers caracteres (circulaire Unedic 2016-25, fiche 2,
    /// § 1.1.3.2 « Composition du numéro d'objet », `atelier/reference/sources/unedic-circulaire-2016-25-annexes-8-10.txt`,
    /// l. 5681-5700 : « 2 caractères pour l'année (année de la demande de numéro par l'employeur) »). `None` quand ce ne
    /// sont pas deux chiffres : le texte dit deux caractères, pas deux chiffres, et un numero qui ne les a pas ne dit pas
    /// son annee. INFERENCE : le siecle (`2000 +`) ; le texte ne donne que deux caracteres, et le numero n'est exige sur
    /// l'AEM qu'au-dela du 31 mars 2008 (meme fichier, l. 5705).
    pub fn annee_de_la_demande(&self) -> Option<i32> {
        match self.0.as_bytes() {
            [d @ b'0'..=b'9', u @ b'0'..=b'9', ..] => Some(2000 + i32::from(d - b'0') * 10 + i32::from(u - b'0')),
            _ => None,
        }
    }
}

notion_lue!(NumeroObjetSpectacle rangee par sans_blancs_de_bord);

/// LE CODE INSEE D'UNE COMMUNE : cinq caracteres, ce que dit la norme DSN, et rien d'autre (« Code INSEE commune »,
/// « Nomenclature INSEE des communes de France », S21.G00.81.005, X [5,5] :
/// `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.81.txt`, l. 458-496).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct CodeInsee(String);

impl CodeInsee {
    pub const LONGUEUR: usize = 5;

    /// Le code `texte`, ou la phrase qui le refuse.
    pub fn lire(texte: &str) -> R<CodeInsee> {
        let n = texte.chars().count();
        if n == Self::LONGUEUR {
            Ok(CodeInsee(texte.to_string()))
        } else {
            Err(socle::refus!(
                "« {} » a {} caractères : un code INSEE de commune en a {}, 37261 par exemple, ce que la DSN écrit (S21.G00.81.005). Corrigez-le",
                texte,
                n,
                Self::LONGUEUR
            ))
        }
    }
}

notion_lue!(CodeInsee rangee par socle::texte::en_capitales);

notion_lue!(CodeEmploiFt rangee par socle::texte::en_capitales);
notion_lue!(Matricule rangee par tel_quel);

/// L'ECHELON D'UN EMPLOI AUTRE QU'ARTISTIQUE DE LA CCNEAC : un entier de 1 a 12, « échelon 1 : coefficient 100 [...]
/// échelon 12 : coefficient 133 », « attribué par l'employeur » (art. X.4.1,
/// `atelier/reference/sources/kali-1285-titres-ii-iii-x-xi-xii-20260923.md`). Hors de 1 a 12, il se refuse a la lecture
/// (`lire`) et a la saisie (`FormeDeCase::FAUTE`), en une phrase qui dit quoi faire. Il se lit d'un nombre ou d'un texte
/// de chiffres (la case de la page envoie son texte) ; il s'ecrit en nombre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct EchelonCcneac(u8);

impl EchelonCcneac {
    /// Son rang dans la grille, de 1 a 12.
    pub fn rang(self) -> usize {
        usize::from(self.0)
    }

    /// L'echelon ecrit `texte`, ou la phrase qui le refuse.
    pub fn lire(texte: &str) -> R<EchelonCcneac> {
        match socle::num::entier_de_chiffres(texte.trim()).filter(|n| (1..=12).contains(n)) {
            Some(n) => Ok(EchelonCcneac(n as u8)),
            None => Err(socle::refus!(
                "« {} » n'est pas un échelon de la CCNEAC : la convention en compte douze, de 1 à 12 (art. X.4.1). Écrivez celui que l'employeur a attribué",
                texte
            )),
        }
    }

    /// Ce qui refuse `texte` ; rien pour un texte vide (la case vide s'ecrit nulle).
    pub fn faute(texte: &str) -> Option<String> {
        if texte.is_empty() {
            return None;
        }
        Self::lire(texte).err().map(|e| e.phrase)
    }
}

impl std::fmt::Display for EchelonCcneac {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for EchelonCcneac {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let ecrit = match socle::forme::NombreEcrit::deserialize(d)? {
            socle::forme::NombreEcrit::Entier(i) => i.to_string(),
            socle::forme::NombreEcrit::Naturel(u) => u.to_string(),
            socle::forme::NombreEcrit::Flottant(x) => x.to_string(),
            socle::forme::NombreEcrit::Texte(t) => t,
        };
        EchelonCcneac::lire(&ecrit).map_err(|e| serde::de::Error::custom(e.phrase))
    }
}

impl FormeDeCase for EchelonCcneac {
    const FAUTE: Option<fn(&str) -> Option<String>> = Some(EchelonCcneac::faute);
}

/// LA RAISON SOCIALE DECLAREE : 60 caracteres au plus, en deux lignes de 32 coupees entre deux mots. La DSN en ecrit 60
/// (S10.G00.01.003, X [1,60] : `logiciel/donnees/formats-dsn-base.json`) ; la DPAE, deux lignes de 32 (L_RAISON_SOC_1,
/// positions 152-183, et L_RAISON_SOC_2, « Nom ou raison sociale (ligne 2) », 510-541 :
/// `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`, l. 249-256 et 374-379).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RaisonSociale(String);

impl RaisonSociale {
    pub const LONGUEUR: usize = 60;
    /// Une ligne de la DPAE.
    pub const LIGNE: usize = 32;

    /// Ses deux lignes de la DPAE (`LIGNE` chacune au plus) : la premiere aussi longue que les mots le permettent, la
    /// seconde le reste, vide quand tout tient sur la premiere ; sinon, la phrase qui dit quoi faire.
    pub fn en_deux_lignes(texte: &str) -> R<(String, String)> {
        if caracteres(texte) <= Self::LIGNE {
            return Ok((texte.to_string(), String::new()));
        }
        let c: Vec<char> = texte.chars().collect();
        let deux = (1..=Self::LIGNE).rev().find(|&i| c[i] == ' ').map(|i| (c[..i].iter().collect::<String>(), c[i + 1..].iter().collect::<String>()));
        match deux {
            Some((premiere, seconde)) if caracteres(&seconde) <= Self::LIGNE => Ok((premiere, seconde)),
            _ => Err(socle::refus!(
                "« {} » ne tient pas en deux lignes de {} caractères coupées entre deux mots, ce que la DPAE écrit (norme DPAE \
v5.3, zone « Nom ou raison sociale », lignes 1 et 2). Abrégez le mot le plus long, ou la fin",
                texte,
                Self::LIGNE
            )),
        }
    }

    /// Ce qui refuse `texte`, en une phrase qui dit quoi faire ; rien pour un texte vide.
    pub fn faute(texte: &str) -> Option<String> {
        let n = caracteres(texte);
        if n > Self::LONGUEUR {
            return Some(format!(
                "« {} » a {} caractères : une raison sociale déclarée en a {} au plus, ce que la DSN écrit (S10.G00.01.003). \
Abrégez-la",
                texte,
                n,
                Self::LONGUEUR
            ));
        }
        Self::en_deux_lignes(texte).err().map(|e| e.phrase)
    }
}

texte_borne!(@commun RaisonSociale, "caractères");

/// L'adresse d'un document en une ligne : la voie, puis son complement quand la fiche le dit (`LigneAdresse`).
pub fn avec_complement(adresse: String, complement: Option<&str>) -> String {
    match complement.filter(|c| !c.trim().is_empty()) {
        Some(c) => format!("{}, {}", adresse, c),
        None => adresse,
    }
}

// ------------------------------------------------------------ la forme declaree

/// LES TROIS FICHES : celle de l'entreprise, d'un salarie, d'un spectacle. Une charge ou une table qui nomme une autre
/// fiche se refuse a la porte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NomDeFiche {
    Entreprise,
    Salarie,
    Spectacle,
}

impl NomDeFiche {
    /// Son nom, tel que les charges et `champs-base.json` l'ecrivent.
    pub fn nom(self) -> &'static str {
        match self {
            NomDeFiche::Entreprise => "entreprise",
            NomDeFiche::Salarie => "salarie",
            NomDeFiche::Spectacle => "spectacle",
        }
    }
}

impl std::fmt::Display for NomDeFiche {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.nom())
    }
}

/// UNE FICHE NOMMEE : l'entreprise (le fichier entier, sans cle), ou un salarie, un spectacle par la cle de sa fiche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FicheNommee {
    Entreprise,
    Salarie(IdSalarie),
    Spectacle(IdSpectacle),
}

impl FicheNommee {
    /// Le nom de la fiche.
    pub fn nom(&self) -> NomDeFiche {
        match self {
            FicheNommee::Entreprise => NomDeFiche::Entreprise,
            FicheNommee::Salarie(_) => NomDeFiche::Salarie,
            FicheNommee::Spectacle(_) => NomDeFiche::Spectacle,
        }
    }
}

/// LE NOM D'UNE CASE DECLAREE HORS DES PIECES : un champ d'une fiche ou une case d'une structure, tel que sa forme
/// l'ecrit. Il ne se fabrique que de la forme (`Declaree`, les cles d'une fiche : `CaseSalarie`, `CaseEntreprise`...) ;
/// un texte ne devient un nom de case qu'a la porte, contre la forme (`NomDeCase::lire`), et un nom que la forme ne
/// declare pas s'y refuse. UNE CASE D'UNE PIECE N'EN EST PAS UNE : elle est une `CaseDePiece`, que `fiche!` separe a la
/// declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NomDeCase(&'static str);

impl NomDeCase {
    /// LA LIGNE DE L'HISTOIRE D'UNE FICHE (`historique`, forme `histoire` de `champs-base.json`) : ce que la route compose
    /// des listes datees ; elle ne se saisit pas et n'est pas une case de la forme.
    pub const HISTORIQUE: NomDeCase = NomDeCase("historique");

    /// Le nom, tel que le fichier et la page l'ecrivent.
    pub fn nom(self) -> &'static str {
        self.0
    }

    /// LE NOM `texte` parmi les cases `declarees` (un niveau de la forme), s'il y est.
    pub fn parmi(declarees: &[Declaree], texte: &str) -> Option<NomDeCase> {
        declarees.iter().map(|d| d.cle).find(|c| c.0 == texte)
    }

    /// LE NOM `texte`, s'il est celui d'une case qu'une fiche declare hors de ses pieces (a tout niveau) ou la ligne de
    /// l'histoire : la seule lecture d'un nom de case. Le nom d'une case d'une piece n'en est pas un (`CaseDePiece::lire`).
    pub fn lire(texte: &str) -> Option<NomDeCase> {
        fn dans(d: &[Declaree], texte: &str) -> Option<NomDeCase> {
            NomDeCase::parmi(d, texte).or_else(|| d.iter().find_map(|x| dans(&x.cases, texte)))
        }
        if texte == NomDeCase::HISTORIQUE.0 {
            return Some(NomDeCase::HISTORIQUE);
        }
        [ChampsEntreprise::cases(), ChampsSalarie::cases(), ChampsSpectacle::cases()].iter().flatten().find_map(|d| match d.cle {
            CaseDeFiche::Case(n) if n.0 == texte => Some(n),
            CaseDeFiche::Case(_) | CaseDeFiche::Piece(_) => dans(&d.cases, texte),
        })
    }
}

/// UNE CASE D'UNE PIECE (`PieceFermee`), telle que `fiche!` la separe a la declaration : elle n'est pas un `NomDeCase`.
/// Sa piece est celle que sa forme lui donne (`CaseDePiece::piece`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CaseDePiece {
    Salarie(CasePieceSalarie),
    Entreprise(CasePieceEntreprise),
}

impl CaseDePiece {
    /// Son nom, tel que le fichier et la page l'ecrivent.
    pub fn nom(self) -> &'static str {
        match self {
            CaseDePiece::Salarie(c) => c.nom(),
            CaseDePiece::Entreprise(c) => c.nom(),
        }
    }

    /// La piece ou sa forme la range.
    pub fn piece(self) -> Piece {
        match self {
            CaseDePiece::Salarie(c) => c.piece(),
            CaseDePiece::Entreprise(c) => c.piece(),
        }
    }

    /// LE NOM `texte`, s'il est celui d'une case d'une piece : la seule lecture d'une case de piece d'un texte.
    pub fn lire(texte: &str) -> Option<CaseDePiece> {
        let salarie = CasePieceSalarie::TOUTES.iter().map(|c| CaseDePiece::Salarie(*c));
        let entreprise = CasePieceEntreprise::TOUTES.iter().map(|c| CaseDePiece::Entreprise(*c));
        salarie.chain(entreprise).find(|c| c.nom() == texte)
    }
}

impl From<CasePieceSalarie> for CaseDePiece {
    fn from(c: CasePieceSalarie) -> CaseDePiece {
        CaseDePiece::Salarie(c)
    }
}

impl From<CasePieceEntreprise> for CaseDePiece {
    fn from(c: CasePieceEntreprise) -> CaseDePiece {
        CaseDePiece::Entreprise(c)
    }
}

impl From<CasePieceSalarie> for CaseDeFiche {
    fn from(c: CasePieceSalarie) -> CaseDeFiche {
        CaseDeFiche::Piece(c.into())
    }
}

impl From<CasePieceEntreprise> for CaseDeFiche {
    fn from(c: CasePieceEntreprise) -> CaseDeFiche {
        CaseDeFiche::Piece(c.into())
    }
}

impl std::fmt::Display for CaseDePiece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.nom())
    }
}

/// UNE CASE D'UNE FICHE, de ses pieces ou non : ce que la mecanique de toutes les cases (la table `champs-base.json`, la
/// saisie case par case, l'ecriture datee, l'historique) nomme sans la lire par son nom. Une lecture qui nomme sa case
/// prend un `NomDeCase`, ou une `CaseDePiece`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CaseDeFiche {
    Case(NomDeCase),
    Piece(CaseDePiece),
}

impl CaseDeFiche {
    /// Son nom, tel que le fichier et la page l'ecrivent.
    pub fn nom(self) -> &'static str {
        match self {
            CaseDeFiche::Case(n) => n.nom(),
            CaseDeFiche::Piece(p) => p.nom(),
        }
    }

    /// Sa piece ; `None` : elle n'est dans aucune, tout calcul la lit.
    pub fn piece(self) -> Option<Piece> {
        match self {
            CaseDeFiche::Case(_) => None,
            CaseDeFiche::Piece(p) => Some(p.piece()),
        }
    }

    /// LE NOM `texte`, s'il est celui d'une case qu'une fiche declare, d'une piece ou non.
    pub fn lire(texte: &str) -> Option<CaseDeFiche> {
        NomDeCase::lire(texte).map(CaseDeFiche::Case).or_else(|| CaseDePiece::lire(texte).map(CaseDeFiche::Piece))
    }
}

impl From<NomDeCase> for CaseDeFiche {
    fn from(n: NomDeCase) -> CaseDeFiche {
        CaseDeFiche::Case(n)
    }
}

impl From<&CaseDeFiche> for CaseDeFiche {
    fn from(c: &CaseDeFiche) -> CaseDeFiche {
        *c
    }
}

impl From<CaseDePiece> for CaseDeFiche {
    fn from(p: CaseDePiece) -> CaseDeFiche {
        CaseDeFiche::Piece(p)
    }
}

impl std::fmt::Display for CaseDeFiche {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.nom())
    }
}

impl Serialize for CaseDeFiche {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nom())
    }
}

/// A la porte : un nom que la forme ne declare pas se refuse.
impl<'de> Deserialize<'de> for CaseDeFiche {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let t = String::deserialize(d)?;
        CaseDeFiche::lire(&t).ok_or_else(|| serde::de::Error::custom(format!("la case « {} » n'existe dans aucune fiche", t)))
    }
}

impl From<&NomDeCase> for NomDeCase {
    fn from(n: &NomDeCase) -> NomDeCase {
        *n
    }
}

impl std::fmt::Display for NomDeCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl Serialize for NomDeCase {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.0)
    }
}

/// A la porte : un nom que la forme ne declare pas se refuse.
impl<'de> Deserialize<'de> for NomDeCase {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let t = String::deserialize(d)?;
        NomDeCase::lire(&t).ok_or_else(|| serde::de::Error::custom(format!("la case « {} » n'existe dans aucune fiche", t)))
    }
}

/// Le nom d'une cle de la forme : `r#type` s'ecrit `type`.
fn nom_ecrit(cle: &'static str) -> NomDeCase {
    match cle.strip_prefix("r#") {
        Some(reste) => NomDeCase(reste),
        None => NomDeCase(cle),
    }
}

/// LES CLES D'UNE FORME (`fiche!`, `cases!`) : une variante par case, qui donne son nom tel qu'il s'ecrit (`NomDeCase`).
macro_rules! cles {
    ($cles:ident { $( $var:ident = $ecrit:literal ),* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $cles {
            $( $var, )*
        }

        impl From<$cles> for NomDeCase {
            fn from(c: $cles) -> NomDeCase {
                match c {
                    $( $cles::$var => NomDeCase($ecrit), )*
                }
            }
        }

        impl From<$cles> for CaseDeFiche {
            fn from(c: $cles) -> CaseDeFiche {
                CaseDeFiche::Case(c.into())
            }
        }
    };
}

/// CE QU'UNE CASE DECLARE, pour le test qui la confronte a `champs-base.json` : ses valeurs (un choix) et ses cases (une
/// structure) ; pour la saisie, la faute que son type refuse (`FormeDeCase::FAUTE`). Sa cle : le nom d'une case d'une
/// structure, ou la case d'une fiche (`CaseDeFiche`, de ses pieces ou non).
#[derive(Debug, Clone)]
pub struct Declaree<K = NomDeCase> {
    pub cle: K,
    pub valeurs: &'static [&'static str],
    pub cases: Vec<Declaree>,
    pub faute: Option<fn(&str) -> Option<String>>,
    /// La borne de son texte et ce qu'elle compte (`texte_borne!`), que l'aide de sa ligne de `champs-base.json` dit.
    pub longueur: Option<(usize, &'static str)>,
    /// La forme que son type donne a un texte saisi (`FormeDeCase::RANGER`), que `rangee` applique.
    pub ranger: Option<fn(&str) -> String>,
}

impl<K> Declaree<K> {
    /// LE TEXTE SAISI `texte` RANGE DANS LA FORME DU TYPE DE LA CASE (`FormeDeCase::RANGER`, le seul endroit qui dit
    /// comment une saisie se range), avant tout controle et toute ecriture : ce qui s'ecrit est ce que son lecteur relit.
    /// La seule porte d'une case saisie l'applique, celle d'un changement de case (`nulle_ou_dite`, par laquelle
    /// `FicheSaisie::lire_cases` lit chaque case) ; rien pour un type qui lit son texte tel quel.
    pub fn rangee(&self, texte: &str) -> Option<String> {
        self.ranger.map(|r| r(texte))
    }

    /// Une case de son type, ou d'une structure qu'il porte, range-t-elle sa saisie ?
    fn range(&self) -> bool {
        self.ranger.is_some() || self.cases.iter().any(Declaree::range)
    }
}

/// LA SAISIE D'UN CHANGEMENT DE CASE DE TYPE `T`, RANGEE : le texte au chemin `chemin` (les cles qui y menent sous la
/// case) prend la forme du type de sa case (`Declaree::rangee`).
fn rangee_au_chemin(case: &Declaree, chemin: &[String], texte: &str) -> Option<String> {
    match chemin.split_first() {
        None => case.rangee(texte),
        Some((cle, reste)) => case.cases.iter().find(|d| d.cle.nom() == cle).and_then(|d| rangee_au_chemin(d, reste, texte)),
    }
}

/// La forme d'un type de case.
pub trait FormeDeCase {
    /// Les valeurs d'un choix ; aucune hors d'un choix.
    const VALEURS: &'static [&'static str] = &[];
    /// Ce que le type refuse d'un texte saisi, en une phrase qui dit quoi corriger (`LigneAdresse`) ; rien d'ordinaire.
    const FAUTE: Option<fn(&str) -> Option<String>> = None;
    /// La borne d'un texte declare (`texte_borne!`) et ce qu'elle compte ; aucune d'ordinaire.
    const LONGUEUR: Option<(usize, &'static str)> = None;
    /// La forme que le type donne a un texte saisi avant de le lire (`Declaree::rangee`) : sans ses blancs de bord, en
    /// capitales ; aucune d'ordinaire (le texte se lit tel quel).
    const RANGER: Option<fn(&str) -> String> = None;
    /// Les cases d'une structure ; aucune hors d'une structure.
    fn cases() -> Vec<Declaree> {
        Vec::new()
    }
}

fn declaree<T: FormeDeCase>(cle: &'static str) -> Declaree {
    Declaree { cle: nom_ecrit(cle), valeurs: T::VALEURS, cases: T::cases(), faute: T::FAUTE, longueur: T::LONGUEUR, ranger: T::RANGER }
}

/// La case d'une fiche, hors de ses pieces.
fn declaree_de_fiche<T: FormeDeCase>(cle: &'static str) -> Declaree<CaseDeFiche> {
    Declaree { cle: CaseDeFiche::Case(nom_ecrit(cle)), valeurs: T::VALEURS, cases: T::cases(), faute: T::FAUTE, longueur: T::LONGUEUR, ranger: T::RANGER }
}

/// La case d'une piece d'une fiche.
fn declaree_de_piece<T: FormeDeCase>(cle: CaseDePiece) -> Declaree<CaseDeFiche> {
    Declaree { cle: CaseDeFiche::Piece(cle), valeurs: T::VALEURS, cases: T::cases(), faute: T::FAUTE, longueur: T::LONGUEUR, ranger: T::RANGER }
}

impl FormeDeCase for String {}
impl FormeDeCase for bool {}
impl FormeDeCase for i64 {}
impl FormeDeCase for Jour {}
impl FormeDeCase for Mois {}
impl FormeDeCase for Montant {}
impl FormeDeCase for socle::paris::Moment {}
impl FormeDeCase for crate::forme_bareme::NiveauDeGarantie {}
impl FormeDeCase for crate::bareme::CodeRisqueAt {
    const FAUTE: Option<fn(&str) -> Option<String>> = Some(crate::bareme::CodeRisqueAt::faute);
    const RANGER: Option<fn(&str) -> String> = Some(crate::bareme::CodeRisqueAt::rangee);
}
impl FormeDeCase for socle::pays::CodePays {
    const FAUTE: Option<fn(&str) -> Option<String>> = Some(socle::pays::CodePays::faute);
    const RANGER: Option<fn(&str) -> String> = Some(socle::pays::CodePays::rangee);
}
impl<T: FormeDeCase> FormeDeCase for Vec<T> {
    const VALEURS: &'static [&'static str] = T::VALEURS;
    const RANGER: Option<fn(&str) -> String> = T::RANGER;
    fn cases() -> Vec<Declaree> {
        T::cases()
    }
}

/// UN CHOIX (`forme: "choix"` dans `champs-base.json`) : une valeur de la liste, ecrite telle quelle. La ligne vide d'une
/// liste est une case vide (nulle), pas une valeur.
macro_rules! choix {
    ($(#[$m:meta])* pub enum $nom:ident { $( $(#[$vm:meta])* $var:ident = $txt:literal ),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub enum $nom { $( $(#[$vm])* #[serde(rename = $txt)] $var ),* }
        impl $nom {
            pub fn as_str(&self) -> &'static str {
                match self { $( $nom::$var => $txt ),* }
            }
            /// Le choix dont le code est `code`, s'il est de la liste.
            pub fn du_code(code: &str) -> Option<$nom> {
                match code { $( $txt => Some($nom::$var), )* _ => None }
            }
        }
        impl std::fmt::Display for $nom {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
        impl AsRef<str> for $nom {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
        impl FormeDeCase for $nom {
            const VALEURS: &'static [&'static str] = &[$($txt),*];
        }
        impl Choix for $nom {
            fn code(&self) -> &'static str {
                self.as_str()
            }
        }
    };
}

/// UNE STRUCTURE DE CASES : chacune un `Option` (`Lue`), une cle inconnue refusee. Ses `notes` (d'une fiche datee) ne sont pas des
/// cases : des textes qui ne se datent pas, que la fiche porte tels qu'ecrits.
///
/// AVEC SES CLES (`cles`) : chaque case dit sa variante, son type, son nom ecrit et son champ
/// (`Iban(String) = "iban" => iban`), comme les changements de la carte (`engagement::carte::Changement`).
macro_rules! cases {
    ($(#[$m:meta])* pub struct $nom:ident, cles $cles:ident { $( $(#[$fm:meta])* $var:ident ( $t:ty ) = $ecrit:literal => $champ:ident ),* $(,)? }) => {
        cles! { $cles { $( $var = $ecrit ),* } }
        cases! { @ecrites $(#[$m])* pub struct $nom { $( $(#[$fm])* $champ : $t = $ecrit ),* } }
    };
    (@ecrites $(#[$m:meta])* pub struct $nom:ident { $( $(#[$fm:meta])* $champ:ident : $t:ty = $ecrit:literal ),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $nom {
            $( $(#[$fm])* #[serde(rename = $ecrit, skip_serializing_if = "Option::is_none")] pub $champ: Option<$t>, )*
        }
        impl FormeDeCase for $nom {
            fn cases() -> Vec<Declaree> {
                vec![$(declaree::<$t>($ecrit)),*]
            }
        }
        impl $nom {
            /// Aucune case n'y est ecrite.
            pub fn est_vide(&self) -> bool {
                true $(&& self.$champ.is_none())*
            }

            /// Les cases que `autre` dit remplacent les siennes.
            pub fn par_dessus(&mut self, autre: &$nom) {
                $( if autre.$champ.is_some() {
                    self.$champ = autre.$champ.clone();
                } )*
            }
        }
    };
    ($(#[$m:meta])* pub struct $nom:ident { $( $(#[$fm:meta])* $champ:ident : $t:ty ),* $(,)? } $( notes { $( $(#[$nm:meta])* $note:ident ),* $(,)? } )?) => {
        $(#[$m])*
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $nom {
            $( $(#[$fm])* #[serde(skip_serializing_if = "Option::is_none")] pub $champ: Option<$t>, )*
            $( $( $(#[$nm])* #[serde(skip_serializing_if = "Option::is_none")] pub $note: Option<String>, )* )?
        }
        impl FormeDeCase for $nom {
            fn cases() -> Vec<Declaree> {
                vec![$(declaree::<$t>(stringify!($champ))),*]
            }
        }
        impl $nom {
            /// Les cles de ses notes : des textes qui ne sont pas des cases.
            pub const NOTES: &'static [&'static str] = &[$($( stringify!($note) ),*)?];

            /// Aucune case n'y est ecrite.
            pub fn est_vide(&self) -> bool {
                true $(&& self.$champ.is_none())*
            }

            /// Les cases que `autre` dit remplacent les siennes ; ses notes restent.
            pub fn par_dessus(&mut self, autre: &$nom) {
                $( if autre.$champ.is_some() {
                    self.$champ = autre.$champ.clone();
                } )*
            }

            /// Les memes cases, sans ses notes.
            pub fn sans_notes(self) -> $nom {
                $nom { $( $( $note: None, )* )? ..self }
            }
        }
    };
}

/// LES PIECES D'UNE FICHE : les cases qu'un seul genre de calcul lit, et que seul un calcul qui tient la cle de la piece
/// ouvre (`PieceFermee`). Ce que la fiche montre se deduit d'elles (`routes::fiches::montres`) : une piece que la
/// compagnie, ou le salarie, n'ouvre jamais ne pose aucune question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Piece {
    /// Les cases d'un salarie au mois, et celles de l'entreprise que seul son calcul lit : seul le calcul d'un engagement
    /// paye au mois les ouvre (`natures::AuMois`).
    Permanent,
    /// Les cases d'une compagnie qui emploie des permanents : seule la case « emploie des permanents » cochee les ouvre
    /// (`EmploieDesPermanents`).
    EmploieDesPermanents,
    /// Les cases que seule une grille qui vise le metier d'un engagement lit (`minima::ParLaGrille`).
    Metier,
    /// Les cases que seule la grille du metier d'un engagement paye au mois lit (`minima::ParLaGrille` et
    /// `natures::AuMois`) : la jauge de la salle de l'annexe I.
    MetierAuMois,
    /// La representation du personnel : seul un effectif qui laisse un comite exister l'ouvre
    /// (`contexte::UnCsePossible`).
    Cse,
    /// L'exemption de la CASC-SVP : seule une compagnie d'au moins cinquante salaries dotee d'un comite l'ouvre
    /// (`ExemptionCasc`).
    Casc,
}

/// LA CLE DE L'EXEMPTION DE LA CASC-SVP (`ChampsEntreprise::exemption_casc`) : « les entreprises d'au moins 50 salariés
/// qui disposent d'un comité social et économique » (3090, art. 4.22 B,
/// `atelier/reference/sources/kali-3090-titres-iv-v-sante-20260923.md`) ; seules les cles de l'effectif
/// (`contexte::CinquanteSalaries`, `contexte::UnCsePossible`) et un comite dit la donnent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExemptionCasc(());

/// LA CLE DES CASES D'UNE COMPAGNIE QUI EMPLOIE DES PERMANENTS (`ChampsEntreprise::avec_permanents`) : seule sa case
/// « emploie des permanents », cochee a la date lue, la donne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmploieDesPermanents(());

/// UNE PIECE FERMEE : ses cases (`C`) ne se lisent qu'avec sa cle (`K`), qu'un calcul ne tient que s'il est celui qui
/// les lit (`natures::AuMois` : la nature d'un engagement paye au mois). Une case rangee ici et lue par un calcul qui
/// n'a pas la cle ne compile pas.
pub struct PieceFermee<C, K> {
    cases: C,
    cle: std::marker::PhantomData<fn() -> K>,
}

impl<C, K> PieceFermee<C, K> {
    fn de(cases: C) -> Self {
        PieceFermee { cases, cle: std::marker::PhantomData }
    }

    /// Ses cases, a qui tient sa cle.
    pub fn ouvrir(&self, _cle: &K) -> &C {
        &self.cases
    }

    /// Ses cases a changer (la fiche que la paie lit perd une dispense hors de ses bornes), a qui tient sa cle.
    pub fn ouvrir_mut(&mut self, _cle: &K) -> &mut C {
        &mut self.cases
    }
}

impl<C: Default, K> Default for PieceFermee<C, K> {
    fn default() -> Self {
        PieceFermee::de(C::default())
    }
}

impl<C: Clone, K> Clone for PieceFermee<C, K> {
    fn clone(&self) -> Self {
        PieceFermee::de(self.cases.clone())
    }
}

impl<C: PartialEq, K> PartialEq for PieceFermee<C, K> {
    fn eq(&self, autre: &Self) -> bool {
        self.cases == autre.cases
    }
}

impl<C: std::fmt::Debug, K> std::fmt::Debug for PieceFermee<C, K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.cases.fmt(f)
    }
}

/// Une case vide ne s'ecrit pas.
fn vide<T>(case: &&Option<T>) -> bool {
    case.is_none()
}

/// UNE FICHE DATEE : ses champs a un jour (`$plate`), et la fiche telle qu'ecrite (`$datee`), chaque champ une liste
/// datee ; `au` lit l'une dans l'autre, la valeur en vigueur de chaque champ (`valeur_au`).
///
/// SES PIECES (`pieces`) : les cases que seul un calcul qui tient leur cle lit, rangees chacune dans sa piece
/// (`PieceFermee`), qui ne s'ouvre que par elle. Le fichier, lui, reste a plat : une case d'une piece s'y ecrit comme
/// toute autre, et la forme qui la nomme (`Changement`, `Cases`, la liste datee) aussi. Ce que la fiche montre se deduit
/// de la piece de chaque case (`CaseDeFiche::piece`).
///
/// SES CLES SONT SEPAREES : celles des cases hors des pieces (`cles`, un `NomDeCase` chacune), celles des cases des
/// pieces (`pieces $pcles`, une `CaseDePiece` chacune) ; aucune n'est les deux.
macro_rules! fiche {
    ($(#[$m:meta])* pub struct $plate:ident, datee $datee:ident, changement $ch:ident, cles $cles:ident { $( $(#[$fm:meta])* $var:ident ( $t:ty ) = $ecrit:literal => $champ:ident ),* $(,)? }
     pieces $pcles:ident { $( $(#[$pm:meta])* $piece:ident : $genre:ident, $cases:ident, $ouvre:ty { $( $(#[$pfm:meta])* $pvar:ident ( $pt:ty ) = $pecrit:literal => $pchamp:ident ),* $(,)? } )* }
     $( notes { $( $(#[$nm:meta])* $note:ident ),* $(,)? } )?) => {
        $( cases! { @ecrites $(#[$pm])* pub struct $cases { $( $(#[$pfm])* $pchamp : $pt = $pecrit ),* } } )*

        cles! { $cles { $( $var = $ecrit, )* } }

        /// LES CASES DE SES PIECES : une variante par case, qui n'est pas un `NomDeCase` (`CaseDePiece`).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $pcles {
            $( $( $pvar, )* )*
        }

        impl $pcles {
            /// Toutes, dans l'ordre de la forme.
            pub const TOUTES: &'static [$pcles] = &[$( $( $pcles::$pvar, )* )*];

            /// Son nom, tel que le fichier et la page l'ecrivent.
            pub fn nom(self) -> &'static str {
                match self {
                    $( $( $pcles::$pvar => $pecrit, )* )*
                }
            }

            /// La piece ou sa forme la range.
            pub fn piece(self) -> Piece {
                match self {
                    $( $( $pcles::$pvar => Piece::$genre, )* )*
                }
            }
        }

        $(#[$m])*
        #[derive(Debug, Clone, Default, PartialEq)]
        pub struct $plate {
            $( $(#[$fm])* pub $champ: Option<$t>, )*
            $( $(#[$pm])* pub $piece: PieceFermee<$cases, $ouvre>, )*
            $( $( $(#[$nm])* pub $note: Option<String>, )* )?
        }

        /// A plat, comme le fichier : les cases de ses pieces a cote des autres.
        impl Serialize for $plate {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                #[derive(Serialize)]
                struct APlat<'a> {
                    $( #[serde(rename = $ecrit, skip_serializing_if = "vide")] $champ: &'a Option<$t>, )*
                    $( $( #[serde(rename = $pecrit, skip_serializing_if = "vide")] $pchamp: &'a Option<$pt>, )* )*
                    $( $( #[serde(skip_serializing_if = "vide")] $note: &'a Option<String>, )* )?
                }
                APlat { $( $champ: &self.$champ, )* $( $( $pchamp: &self.$piece.cases.$pchamp, )* )* $( $( $note: &self.$note, )* )? }.serialize(s)
            }
        }

        /// A plat, une cle inconnue refusee.
        impl<'de> Deserialize<'de> for $plate {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct APlat {
                    $( #[serde(rename = $ecrit)] $champ: Option<$t>, )*
                    $( $( #[serde(rename = $pecrit)] $pchamp: Option<$pt>, )* )*
                    $( $( $note: Option<String>, )* )?
                }
                let p = APlat::deserialize(d)?;
                Ok($plate {
                    $( $champ: p.$champ, )*
                    $( $piece: PieceFermee::de($cases { $( $pchamp: p.$pchamp, )* }), )*
                    $( $( $note: p.$note, )* )?
                })
            }
        }

        impl $plate {
            /// Les cles de ses notes : des textes qui ne sont pas des cases.
            pub const NOTES: &'static [&'static str] = &[$($( stringify!($note) ),*)?];

            /// Ses cases, celles de ses pieces apres les autres.
            pub fn cases() -> Vec<Declaree<CaseDeFiche>> {
                vec![$(declaree_de_fiche::<$t>($ecrit),)* $( $( declaree_de_piece::<$pt>($pcles::$pvar.into()), )* )*]
            }

            /// Aucune case n'y est ecrite.
            pub fn est_vide(&self) -> bool {
                true $(&& self.$champ.is_none())* $( $(&& self.$piece.cases.$pchamp.is_none())* )*
            }

            /// Les cases `cles` sont-elles les memes dans `autre` ?
            pub fn memes_cases(&self, autre: &$plate, cles: &[$cles]) -> bool {
                $( if cles.contains(&$cles::$var) && self.$champ != autre.$champ {
                    return false;
                } )*
                true
            }

            /// Les cases que `autre` dit remplacent les siennes ; ses notes restent.
            pub fn par_dessus(&mut self, autre: &$plate) {
                $( if autre.$champ.is_some() {
                    self.$champ = autre.$champ.clone();
                } )*
                $( $( if autre.$piece.cases.$pchamp.is_some() {
                    self.$piece.cases.$pchamp = autre.$piece.cases.$pchamp.clone();
                } )* )*
            }

            /// Les cases dont la valeur differe dans `autre`, nommees comme `champs-base.json` les nomme ; ses notes n'en
            /// sont pas.
            pub fn cases_differentes(&self, autre: &$plate) -> Vec<CaseDeFiche> {
                let mut out = Vec::new();
                $( if self.$champ != autre.$champ {
                    out.push(CaseDeFiche::Case($cles::$var.into()));
                } )*
                $( $( if self.$piece.cases.$pchamp != autre.$piece.cases.$pchamp {
                    out.push(CaseDeFiche::Piece($pcles::$pvar.into()));
                } )* )*
                out
            }

            /// Les memes cases, sans ses notes.
            pub fn sans_notes(self) -> $plate {
                $plate { $( $( $note: None, )* )? ..self }
            }

            /// Les cases et les notes que `autre` dit remplacent les siennes.
            pub fn fondre(&mut self, autre: &$plate) {
                self.par_dessus(autre);
                $( $( if autre.$note.is_some() {
                    self.$note = autre.$note.clone();
                } )* )?
            }

            /// Les cases et les notes que `autre` dit et qu'elle ne dit pas : la premiere qui en dit une la garde.
            pub fn completer_de(&mut self, autre: &$plate) {
                $( if self.$champ.is_none() {
                    self.$champ = autre.$champ.clone();
                } )*
                $( $( if self.$piece.cases.$pchamp.is_none() {
                    self.$piece.cases.$pchamp = autre.$piece.cases.$pchamp.clone();
                } )* )*
                $( $( if self.$note.is_none() {
                    self.$note = autre.$note.clone();
                } )* )?
            }

            /// La case `k` telle que `autre` la dit (vide s'il ne la dit pas).
            pub fn prendre(&mut self, autre: &$plate, k: CaseDeFiche) {
                $( if k == CaseDeFiche::Case(NomDeCase($ecrit)) {
                    self.$champ = autre.$champ.clone();
                } )*
                $( $( if k == CaseDeFiche::Piece($pcles::$pvar.into()) {
                    self.$piece.cases.$pchamp = autre.$piece.cases.$pchamp.clone();
                } )* )*
            }

            /// Les cases qu'elle dit, dans l'ordre de la forme.
            pub fn cases_dites(&self) -> Vec<CaseDeFiche> {
                let mut out = Vec::new();
                $( if self.$champ.is_some() {
                    out.push(CaseDeFiche::Case($cles::$var.into()));
                } )*
                $( $( if self.$piece.cases.$pchamp.is_some() {
                    out.push(CaseDeFiche::Piece($pcles::$pvar.into()));
                } )* )*
                out
            }

            /// Les notes qu'elle dit : leur cle et leur texte.
            pub fn notes_dites(&self) -> Vec<(&'static str, &str)> {
                let toutes: Vec<(&'static str, Option<&str>)> = vec![$($( (stringify!($note), self.$note.as_deref()) ),*)?];
                toutes.into_iter().filter_map(|(k, t)| t.map(|t| (k, t))).collect()
            }

            /// La note `cle` telle que `autre` la dit ; une fiche sans notes n'a rien a y lire (`_autre`, `_cle`).
            pub fn prendre_la_note(&mut self, _autre: &$plate, _cle: &str) {
                $( $( if _cle == stringify!($note) {
                    self.$note = _autre.$note.clone();
                } )* )?
            }

            /// CE QU'ECRIT LA CASE au bout de `chemin` sous la case `k` (`socle::ecriture::mot_au_chemin`) ; une case
            /// que la forme ne declare pas n'ecrit rien.
            pub fn ecrite_au_chemin(&self, k: CaseDeFiche, chemin: &[&str]) -> MotEcrit {
                $( if k == CaseDeFiche::Case(NomDeCase($ecrit)) {
                    return socle::ecriture::mot_au_chemin(&self.$champ, chemin);
                } )*
                $( $( if k == CaseDeFiche::Piece($pcles::$pvar.into()) {
                    return socle::ecriture::mot_au_chemin(&self.$piece.cases.$pchamp, chemin);
                } )* )*
                MotEcrit::Vide
            }

            /// La valeur de la case `k`, ecrite comme le fichier l'ecrit ; vide, aucune.
            pub fn valeur_ecrite(&self, k: CaseDeFiche) -> Option<ValeurDeCase> {
                $( if k == CaseDeFiche::Case(NomDeCase($ecrit)) {
                    return ValeurDeCase::de(&self.$champ);
                } )*
                $( $( if k == CaseDeFiche::Piece($pcles::$pvar.into()) {
                    return ValeurDeCase::de(&self.$piece.cases.$pchamp);
                } )* )*
                None
            }

            /// LA FICHE TAPEE `o` (un formulaire de la fiche `fiche`), LA SEULE PORTE D'UNE FICHE SAISIE : chaque case
            /// lue seule dans son type (`$ch`, par `nulle_ou_dite` : son rangement, puis son type), une case blanche
            /// vide (`Objet::sans_textes_blancs`) ; la faute d'une case s'ajoute (`FauteDeCase`) et la lecture continue.
            /// Ses notes se lisent en texte ; la ligne de l'histoire (`historique`), que la page renvoie, ne se lit pas.
            /// Une cle qui n'est pas une case de la fiche refuse le formulaire.
            pub fn lire_cases_de_la_fiche(o: &socle::forme::Objet, fiche: &str) -> R<($plate, Vec<FauteDeCase>)> {
                let o = o.sans_textes_blancs();
                let cases = $plate::cases();
                let lue = |k: &str| k != NomDeCase::HISTORIQUE.nom() && !$plate::NOTES.contains(&k);
                let mut plate = $plate::default();
                let mut fautes = Vec::new();
                for (k, lu) in o.chacune::<$ch>(lue, "case", "valeur") {
                    let Some(d) = cases.iter().find(|d| d.cle.nom() == k) else {
                        let connues: Vec<&str> = cases.iter().map(|d| d.cle.nom()).collect();
                        return Err(socle::echec::refus(format!("la case « {} » n'existe pas dans la fiche {} ; ses cases sont : {}", k, fiche, connues.join(", "))));
                    };
                    match lu {
                        Ok(ch) => <$datee as FicheDatee>::appliquer(&mut plate, ch),
                        Err((chemin, phrase)) => fautes.push(FauteDeCase::de(d.cle, chemin, phrase)),
                    }
                }
                $( $( plate.$note = o.lire_une::<Option<String>>(stringify!($note), "Une note de la fiche se lit en texte.")?.flatten(); )* )?
                Ok((plate, fautes))
            }
        }

        /// UN CHANGEMENT D'UNE CASE DE LA FICHE, comme ceux de la carte (`engagement::carte::Changement`) :
        /// `{"case": "email", "valeur": "…"}` pose la case ; `null` la vide. Une case que la liste ne nomme pas ne change
        /// pas : la fiche elle-meme n'a pas a distinguer l'absent du nul.
        #[derive(Debug, Clone, PartialEq, Deserialize)]
        #[serde(tag = "case", content = "valeur", deny_unknown_fields)]
        pub enum $ch {
            $( #[serde(rename = $ecrit, deserialize_with = "nulle_ou_dite")] $var(Option<$t>), )*
            $( $( #[serde(rename = $pecrit, deserialize_with = "nulle_ou_dite")] $pvar(Option<$pt>), )* )*
        }

        impl $ch {
            /// La case qu'il change, nommee comme `champs-base.json` la nomme.
            pub fn case(&self) -> CaseDeFiche {
                match self {
                    $( $ch::$var(_) => CaseDeFiche::Case(NomDeCase($ecrit)), )*
                    $( $( $ch::$pvar(_) => CaseDeFiche::Piece($pcles::$pvar.into()), )* )*
                }
            }
        }

        /// La fiche telle qu'ecrite : chaque champ une liste datee ; absente ou nulle, la fiche ne dit rien de ce champ.
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $datee {
            $( #[serde(rename = $ecrit, skip_serializing_if = "Option::is_none")] pub $champ: Option<Vec<Datee<$t>>>, )*
            $( $( #[serde(rename = $pecrit, skip_serializing_if = "Option::is_none")] pub $pchamp: Option<Vec<Datee<$pt>>>, )* )*
            $( $( $(#[$nm])* #[serde(skip_serializing_if = "Option::is_none")] pub $note: Option<String>, )* )?
        }

        impl $datee {
            /// Ses champs en vigueur a `jour`, ses notes telles qu'ecrites.
            pub fn au(&self, jour: &Jour) -> $plate {
                $plate {
                    $( $champ: self.$champ.as_deref().and_then(|l| valeur_au(l, jour)), )*
                    $( $piece: PieceFermee::de($cases { $( $pchamp: self.$pchamp.as_deref().and_then(|l| valeur_au(l, jour)), )* }), )*
                    $( $( $note: self.$note.clone(), )* )?
                }
            }

            /// Les jours ou le champ `cle` prend une valeur, du plus ancien au plus recent (`None` : la valeur de tout
            /// temps, d'abord) ; aucun pour un champ que la fiche ne dit pas, ou que la forme ne declare pas.
            pub fn depuis(&self, cle: CaseDeFiche) -> Vec<Option<Jour>> {
                $( if CaseDeFiche::Case(NomDeCase($ecrit)) == cle {
                    let mut j: Vec<Option<Jour>> = self.$champ.iter().flatten().map(|e| e.depuis).collect();
                    j.sort();
                    return j;
                } )*
                $( $( if CaseDeFiche::Piece($pcles::$pvar.into()) == cle {
                    let mut j: Vec<Option<Jour>> = self.$pchamp.iter().flatten().map(|e| e.depuis).collect();
                    j.sort();
                    return j;
                } )* )*
                Vec::new()
            }

            /// Le `depuis` de la valeur en vigueur a `jour` de chaque champ ; un champ dont seule la valeur de tout temps
            /// s'applique n'en donne pas.
            pub fn dates_en_vigueur(&self, jour: &Jour) -> Vec<Jour> {
                let mut out = Vec::new();
                $( out.extend(self.$champ.iter().flatten().filter_map(|e| e.depuis).filter(|j| j <= jour).max()); )*
                $( $( out.extend(self.$pchamp.iter().flatten().filter_map(|e| e.depuis).filter(|j| j <= jour).max()); )* )*
                out
            }

            /// Les listes et les notes que `autre` porte remplacent les siennes, entieres ; les autres restent.
            pub fn par_dessus(&mut self, autre: &$datee) {
                $( if autre.$champ.is_some() {
                    self.$champ = autre.$champ.clone();
                } )*
                $( $( if autre.$pchamp.is_some() {
                    self.$pchamp = autre.$pchamp.clone();
                } )* )*
                $( $( if autre.$note.is_some() {
                    self.$note = autre.$note.clone();
                } )* )?
            }
        }

        impl FicheDatee for $datee {
            type Plate = $plate;
            type Changement = $ch;

            fn au(&self, jour: &Jour) -> $plate {
                $datee::au(self, jour)
            }

            fn depuis(&self, cle: CaseDeFiche) -> Vec<Option<Jour>> {
                $datee::depuis(self, cle)
            }

            fn changer(&mut self, ch: $ch, depuis: Option<&Jour>, premiere_datee: bool, note: Option<&str>) {
                match ch {
                    $( $ch::$var(x) => dater(&mut self.$champ, x, depuis, premiere_datee, note), )*
                    $( $( $ch::$pvar(x) => dater(&mut self.$pchamp, x, depuis, premiere_datee, note), )* )*
                }
            }

            fn appliquer(plate: &mut $plate, ch: $ch) {
                match ch {
                    $( $ch::$var(x) => plate.$champ = x, )*
                    $( $( $ch::$pvar(x) => plate.$piece.cases.$pchamp = x, )* )*
                }
            }

            fn changement(plate: &$plate, cle: CaseDeFiche) -> Option<$ch> {
                $( if CaseDeFiche::Case(NomDeCase($ecrit)) == cle {
                    return Some($ch::$var(plate.$champ.clone()));
                } )*
                $( $( if CaseDeFiche::Piece($pcles::$pvar.into()) == cle {
                    return Some($ch::$pvar(plate.$piece.cases.$pchamp.clone()));
                } )* )*
                None
            }

            fn case_changee(ch: &$ch) -> CaseDeFiche {
                ch.case()
            }

            // Une fiche sans notes n'a rien a y lire : `_plate`.
            fn noter(&mut self, _plate: &$plate) {
                $( $( if _plate.$note.is_some() {
                    self.$note = _plate.$note.clone();
                } )* )?
            }

            fn neuve(plate: &$plate) -> $datee {
                let mut f = $datee::default();
                $( if plate.$champ.is_some() {
                    dater(&mut f.$champ, plate.$champ.clone(), None, false, None);
                } )*
                $( $( if plate.$piece.cases.$pchamp.is_some() {
                    dater(&mut f.$pchamp, plate.$piece.cases.$pchamp.clone(), None, false, None);
                } )* )*
                f.noter(plate);
                f
            }

            fn voir_listes<V: VoirListe>(&self, v: &mut V) {
                $( if let Some(l) = &self.$champ {
                    v.liste(CaseDeFiche::Case(NomDeCase($ecrit)), l);
                } )*
                $( $( if let Some(l) = &self.$pchamp {
                    v.liste(CaseDeFiche::Piece($pcles::$pvar.into()), l);
                } )* )*
            }
        }
    };
}

/// UNE FICHE DATEE (`fiche!`) : la fiche telle qu'ecrite, chaque champ une liste datee, et sa fiche a un jour
/// (`Plate`). Elle s'ecrit par `changer` (une case, qui vaut a partir de `depuis`), ses notes par `noter`.
pub trait FicheDatee: Clone + Default + PartialEq + Serialize + DeserializeOwned {
    type Plate: Clone + Default + Serialize + DeserializeOwned;
    type Changement: DeserializeOwned;

    /// Ses champs en vigueur a `jour`, ses notes telles qu'ecrites.
    fn au(&self, jour: &Jour) -> Self::Plate;

    /// Les jours ou le champ `cle` prend une valeur, du plus ancien au plus recent (`None` d'abord) ; aucun pour un champ
    /// que la fiche ne dit pas, ou que la forme ne declare pas.
    fn depuis(&self, cle: CaseDeFiche) -> Vec<Option<Jour>>;

    /// LA CASE QUE `ch` CHANGE S'ECRIT DATEE (`dater`, `premiere_datee` compris) : sa valeur, ou vide.
    fn changer(&mut self, ch: Self::Changement, depuis: Option<&Jour>, premiere_datee: bool, note: Option<&str>);

    /// La case que `ch` change, posee dans la fiche a un jour `plate` : sa valeur, ou vide.
    fn appliquer(plate: &mut Self::Plate, ch: Self::Changement);

    /// La case `cle` de `plate`, comme un changement qui la poserait (`None` : la forme ne declare pas cette case).
    fn changement(plate: &Self::Plate, cle: CaseDeFiche) -> Option<Self::Changement>;

    /// La case que `ch` change, nommee comme `champs-base.json` la nomme.
    fn case_changee(ch: &Self::Changement) -> CaseDeFiche;

    /// Les notes que `plate` porte remplacent les siennes ; les autres restent.
    fn noter(&mut self, plate: &Self::Plate);

    /// Une fiche neuve : chaque case dite de `plate` vaut de tout temps, ses notes telles quelles.
    fn neuve(plate: &Self::Plate) -> Self;

    /// Chaque liste datee, sous son nom, a qui la regarde.
    fn voir_listes<V: VoirListe>(&self, v: &mut V);
}

/// CE QUI REGARDE LES LISTES DATEES D'UNE FICHE PAR LEUR NOM (l'historique que l'ecran montre, les gardes qui cherchent
/// ce qui nomme une personne) : chacune lui est donnee typee (`FicheDatee::voir_listes`).
pub trait VoirListe {
    fn liste<T: Serialize>(&mut self, cle: CaseDeFiche, liste: &[Datee<T>]);
}

/// UNE VALEUR DATEE : de tout temps sans `depuis`, ou a partir de ce jour ; `note` dit d'ou elle vient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T: DeserializeOwned"))]
pub struct Datee<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depuis: Option<Jour>,
    /// La valeur ; nulle, la case est vide a partir de `depuis`.
    pub valeur: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// La valeur en vigueur a `jour` : celle dont le `depuis` est le plus recent sans le depasser (a meme date, la derniere
/// ecrite ; sans `depuis`, de tout temps). Avant la premiere, la case est vide.
pub fn valeur_au<T: Clone>(liste: &[Datee<T>], jour: &Jour) -> Option<T> {
    let mut entrees: Vec<&Datee<T>> = liste.iter().collect();
    entrees.sort_by_key(|e| e.depuis);
    entrees.into_iter().filter(|e| e.depuis.is_none_or(|j| j <= *jour)).last().and_then(|e| e.valeur.clone())
}

/// LES TEMPS D'UNE CASE DATEE A UN JOUR (`temps_au`) : la valeur en vigueur, la derniere d'avant, la prochaine a venir,
/// et toutes, de la plus ancienne a la plus recente.
#[derive(Debug, Clone, PartialEq)]
pub struct Temps<T> {
    /// En vigueur ce jour ; `None` avant la premiere.
    pub en_vigueur: Option<Datee<T>>,
    pub avant: Option<Avant<T>>,
    pub a_venir: Option<AVenir<T>>,
    /// Toutes, chaque date une fois (a meme date, la derniere ecrite).
    pub toutes: Vec<Datee<T>>,
}

/// La derniere valeur d'avant qui differe de celle en vigueur, et son dernier jour : jamais vide.
#[derive(Debug, Clone, PartialEq)]
pub struct Avant<T> {
    pub valeur: T,
    pub jusqu_au: Jour,
    pub note: Option<String>,
}

/// La prochaine valeur, et son premier jour.
#[derive(Debug, Clone, PartialEq)]
pub struct AVenir<T> {
    pub valeur: Option<T>,
    pub depuis: Jour,
    pub note: Option<String>,
}

/// CE QU'UNE CASE DATEE DIT A `jour`, la seule fonction qui le dit : la valeur en vigueur (la meme que `valeur_au`), la
/// derniere d'avant, la prochaine a venir (la plus proche ; les suivantes sont dans `toutes`), chacune avec sa note. A
/// meme date, la derniere ecrite.
///
/// LA VALEUR D'AVANT DIT UN CHANGEMENT : celle qui precede la suite des valeurs que `meme` tient pour celle en vigueur
/// (une meme valeur reecrite, d'une autre provenance, n'en est pas une), jusqu'a la veille du debut de cette suite ; vide,
/// il n'y en a pas. Ses reecritures restent dans `toutes`.
pub fn temps_au<T: Clone>(liste: &[Datee<T>], jour: &Jour, meme: impl Fn(&T, &T) -> bool) -> R<Temps<T>> {
    let mut toutes: Vec<Datee<T>> = Vec::new();
    for e in valeurs_datees(liste) {
        if toutes.last().is_some_and(|d| d.depuis == e.depuis) {
            toutes.pop();
        }
        toutes.push(e.clone());
    }
    let ici = toutes.iter().rposition(|e| e.depuis.is_none_or(|j| j <= *jour));
    let en_vigueur = ici.map(|i| toutes[i].clone());
    let pareilles = |a: &Option<T>, b: &Option<T>| match (a, b) {
        (Some(x), Some(y)) => meme(x, y),
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    };
    let mut avant = None;
    if let Some(i) = ici {
        let mut debut = i;
        while debut > 0 && pareilles(&toutes[debut - 1].valeur, &toutes[i].valeur) {
            debut -= 1;
        }
        if let (Some(e), Some(depuis)) = (debut.checked_sub(1).map(|p| &toutes[p]), toutes[debut].depuis) {
            if let Some(valeur) = e.valeur.clone() {
                avant = Some(Avant { valeur, jusqu_au: depuis.plus_jours(-1)?, note: e.note.clone() });
            }
        }
    }
    let suivante = match ici {
        Some(i) => toutes.get(i + 1),
        None => toutes.first(),
    };
    let a_venir = suivante.and_then(|e| e.depuis.map(|depuis| AVenir { valeur: e.valeur.clone(), depuis, note: e.note.clone() }));
    Ok(Temps { en_vigueur, avant, a_venir, toutes })
}

/// LA CONFIGURATION DE LA COMPAGNIE EST DATEE, comme les baremes : chaque valeur d'une fiche (l'entreprise et sa
/// convention, ses etablissements, un salarie, un spectacle, les organismes, les rubriques de la compagnie) s'ecrit en
/// liste datee (`Datee`) et se lit a la date de la periode payee (`valeur_au`), jamais « telle qu'aujourd'hui » : une
/// valeur changee a une date ne touche pas ce qui precede. La premiere valeur d'un champ s'ecrit sans `depuis` : elle
/// vaut DE TOUT TEMPS, jusqu'a la suivante (un contrat commence avant que la fiche soit saisie, un mois repris d'un
/// autre logiciel trouvent la fiche). Un champ dont toutes les valeurs sont datees ne dit rien avant la premiere.
///
/// ECRIRE UNE VALEUR DATEE, la seule fonction qui le fait : `valeur` vaut a partir de `depuis` (`None` : de tout temps) ;
/// ce qui precede reste ce qu'il etait. La premiere valeur d'une liste vaut de tout temps, quel que soit `depuis` : avant
/// elle, la fiche ne disait rien d'autre. Sauf `premiere_datee` : une valeur qui, par nature, ne vaut qu'a partir d'un
/// jour (le taux de PAS recu, BOI-IR-PAS-30-10-50 § 20 ; une ligne du taux AT/MP, a la date de sa notification) ; dans une
/// liste vide, elle vaut a partir de `depuis`, et avant lui la case est vide. Une valeur deja ecrite au meme jour est
/// remplacee (une correction du meme jour d'effet). `note` dit pourquoi.
pub fn dater<T>(liste: &mut Option<Vec<Datee<T>>>, valeur: Option<T>, depuis: Option<&Jour>, premiere_datee: bool, note: Option<&str>) {
    let liste = liste.get_or_insert_with(Vec::new);
    if liste.is_empty() && premiere_datee && depuis.is_some() {
        liste.push(Datee { depuis: None, valeur: None, note: None });
    }
    let depuis = if liste.is_empty() { None } else { depuis.copied() };
    liste.retain(|e| e.depuis != depuis);
    liste.push(Datee { depuis, valeur, note: note.map(str::trim).filter(|n| !n.is_empty()).map(str::to_string) });
    liste.sort_by_key(|e| e.depuis); // stable : a meme date, la derniere ecrite gagne
}

/// LES VALEURS D'UNE LISTE DATEE, de la plus ancienne a la plus recente : `None` pour celle de tout temps (a meme date,
/// dans l'ordre ecrit).
pub fn valeurs_datees<T>(liste: &[Datee<T>]) -> Vec<&Datee<T>> {
    let mut out: Vec<&Datee<T>> = liste.iter().collect();
    out.sort_by_key(|e| e.depuis);
    out
}

// ------------------------------------------------------------------ les choix

choix! {
    /// Le sexe du NIR (S21.G00.30.005).
    pub enum Sexe { Masculin = "1", Feminin = "2" }
}

impl Sexe {
    /// Le sexe que dit le premier chiffre d'un NIR écrit `texte` (« S = sexe de la personne physique doit être égal à 1
    /// ou 2 » : `atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt`, l. 81-82), ses blancs de bord ôtés ; aucun pour
    /// un autre signe. La seule lecture du sexe d'un NIR : `Nir::sexe`, et le NIR qu'une saisie en cours écrit.
    pub fn du_nir(texte: &str) -> Option<Sexe> {
        match socle::texte::strip(texte).get(..1) {
            Some("1") => Some(Sexe::Masculin),
            Some("2") => Some(Sexe::Feminin),
            _ => None,
        }
    }
}

choix! {
    /// S21.G00.30.007.
    pub enum CodeNationalite { Francaise = "01", UnionEuropeenne = "02", HorsUnionEuropeenne = "03", Inconnue = "04" }
}

choix! {
    pub enum Statut { Artiste = "artiste", Technicien = "technicien" }
}

choix! {
    /// S21.G00.40.002.
    pub enum StatutConventionnel { Cadre03 = "03", Cadre04 = "04", AgentDeMaitrise = "05", Employe06 = "06", Employe07 = "07" }
}

choix! {
    /// S21.G00.40.003.
    pub enum StatutCategorielRc { Cadre = "01", Extension = "02", NonCadre = "04" }
}

choix! {
    /// La profession de la deduction forfaitaire specifique.
    pub enum ProfessionDfs {
        Musicien = "musicien",
        Choriste = "choriste",
        ChefOrchestre = "chef_orchestre",
        RegisseurTheatre = "regisseur_theatre",
        ArtisteDramatique = "artiste_dramatique",
        ArtisteLyrique = "artiste_lyrique",
        ArtisteChoregraphique = "artiste_choregraphique",
        ArtisteCinematographique = "artiste_cinematographique",
    }
}

choix! {
    pub enum AccordDfs { Contrat = "contrat", ConsentementAnnuel = "consentement_annuel", Refus = "refus" }
}

choix! {
    /// S21.G00.30.026 (le statut BOETH).
    pub enum StatutBoeth {
        S01 = "01", S02 = "02", S03 = "03", S04 = "04", S05 = "05", S06 = "06",
        S07 = "07", S08 = "08", S09 = "09", S10 = "10", S11 = "11", S12 = "12",
    }
}

choix! {
    pub enum StagiaireCcneac { Premiere = "1", Deuxieme = "2" }
}

/// LA CATEGORIE D'UN SALARIE DANS LA CONVENTION : celle que nomment les clauses d'une convention
/// (`clauses-conventionnelles-base.json` : `categories.valeurs`, le preavis par categorie, la categorie d'un niveau 3090),
/// le profil d'un salarie (`clauses::Profil`) et la case « Catégorie dans la convention » de la fiche. Une liste du
/// logiciel, ecrite par son texte. `NonCadre` (« non cadre » : la clause qui vaut pour tous ceux qui ne sont pas cadres)
/// ne se dit pas sur la fiche, ou la case cadre le dit : la fiche la refuse en nommant sa case (sa lecture, `DeLaFiche`),
/// les clauses la lisent (`de_la_convention`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(from = "CategorieDeLaFiche")]
pub enum CategorieConventionnelle {
    #[serde(rename = "artiste")]
    Artiste,
    #[serde(rename = "employe")]
    Employe,
    #[serde(rename = "technicien")]
    Technicien,
    #[serde(rename = "agent_de_maitrise")]
    AgentDeMaitrise,
    #[serde(rename = "cadre")]
    Cadre,
    #[serde(rename = "cadre_de_direction")]
    CadreDeDirection,
    #[serde(rename = "non_cadre")]
    NonCadre,
}

/// La lecture de la case « Catégorie dans la convention » de la fiche : les six qu'elle porte, une autre refusee
/// (serde nomme les six), et la case nommee par la faute de la saisie (`FauteDeCase`). Elle se nomme comme la notion dans
/// ce que serde en dit.
#[derive(Deserialize)]
#[serde(rename = "CategorieConventionnelle")]
enum CategorieDeLaFiche {
    #[serde(rename = "artiste")]
    Artiste,
    #[serde(rename = "employe")]
    Employe,
    #[serde(rename = "technicien")]
    Technicien,
    #[serde(rename = "agent_de_maitrise")]
    AgentDeMaitrise,
    #[serde(rename = "cadre")]
    Cadre,
    #[serde(rename = "cadre_de_direction")]
    CadreDeDirection,
}

impl From<CategorieDeLaFiche> for CategorieConventionnelle {
    fn from(c: CategorieDeLaFiche) -> CategorieConventionnelle {
        match c {
            CategorieDeLaFiche::Artiste => CategorieConventionnelle::Artiste,
            CategorieDeLaFiche::Employe => CategorieConventionnelle::Employe,
            CategorieDeLaFiche::Technicien => CategorieConventionnelle::Technicien,
            CategorieDeLaFiche::AgentDeMaitrise => CategorieConventionnelle::AgentDeMaitrise,
            CategorieDeLaFiche::Cadre => CategorieConventionnelle::Cadre,
            CategorieDeLaFiche::CadreDeDirection => CategorieConventionnelle::CadreDeDirection,
        }
    }
}

impl CategorieConventionnelle {
    /// Les sept, dans l'ordre de la liste.
    pub const TOUTES: [CategorieConventionnelle; 7] = [
        CategorieConventionnelle::Artiste,
        CategorieConventionnelle::Employe,
        CategorieConventionnelle::Technicien,
        CategorieConventionnelle::AgentDeMaitrise,
        CategorieConventionnelle::Cadre,
        CategorieConventionnelle::CadreDeDirection,
        CategorieConventionnelle::NonCadre,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            CategorieConventionnelle::Artiste => "artiste",
            CategorieConventionnelle::Employe => "employe",
            CategorieConventionnelle::Technicien => "technicien",
            CategorieConventionnelle::AgentDeMaitrise => "agent_de_maitrise",
            CategorieConventionnelle::Cadre => "cadre",
            CategorieConventionnelle::CadreDeDirection => "cadre_de_direction",
            CategorieConventionnelle::NonCadre => "non_cadre",
        }
    }

    /// Celle dont le code est `code`, si elle est des sept.
    pub fn du_code(code: &str) -> Option<CategorieConventionnelle> {
        CategorieConventionnelle::TOUTES.into_iter().find(|c| c.as_str() == code)
    }

    /// LA CATEGORIE QU'UNE CONVENTION NOMME dans ses clauses (`clauses-conventionnelles-base.json`) : l'une des sept,
    /// `non_cadre` compris ; une autre se refuse.
    pub fn de_la_convention(texte: &str) -> R<CategorieConventionnelle> {
        match CategorieConventionnelle::du_code(texte) {
            Some(c) => Ok(c),
            None => Err(socle::refus!(
                "Catégorie dans la convention : {} n'en est pas une ({}).",
                socle::fichiers::repr(texte),
                CategorieConventionnelle::TOUTES.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ")
            )),
        }
    }
}

impl std::fmt::Display for CategorieConventionnelle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for CategorieConventionnelle {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FormeDeCase for CategorieConventionnelle {
    /// Les six que la case de la fiche porte.
    const VALEURS: &'static [&'static str] = &["artiste", "employe", "technicien", "agent_de_maitrise", "cadre", "cadre_de_direction"];
}

impl Choix for CategorieConventionnelle {
    fn code(&self) -> &'static str {
        self.as_str()
    }
}

choix! {
    /// LES DEUX MODELES DU BULLETIN (`documents::bulletin`, ou chacun se decrit) : le simplifie, le modele de l'arrete
    /// du 25/02/2016, art. 1, I ; le complet, le meme bulletin suivi d'une page annexe qui detaille chaque cotisation.
    /// La case `modele_bulletin` de la fiche entreprise (la compagnie, pour tous) et de la fiche d'un salarie (qui
    /// l'emporte) ; la charge des routes du modele.
    pub enum ModeleBulletin { Simplifie = "simplifié", Complet = "complet" }
}

impl ModeleBulletin {
    pub const TOUS: [ModeleBulletin; 2] = [ModeleBulletin::Simplifie, ModeleBulletin::Complet];

    pub fn nom(self) -> &'static str {
        match self {
            ModeleBulletin::Simplifie => "Simplifié",
            ModeleBulletin::Complet => "Complet",
        }
    }

    /// Ce que la page en dit, sous l'apercu.
    pub fn description(self) -> &'static str {
        match self {
            ModeleBulletin::Simplifie => {
                "Le bulletin clarifié du modèle réglementaire (arrêté du 25 février 2016) : les cotisations regroupées \
                 sous les lignes du modèle, le taux de la part salariale, les contributions de l'employeur en une ligne."
            }
            ModeleBulletin::Complet => {
                "Le même bulletin, ligne pour ligne, suivi d'une page annexe qui détaille chaque cotisation et \
                 contribution sous la ligne du bulletin qui la regroupe : sa base, le taux et le montant de la part \
                 salariale et de la part employeur."
            }
        }
    }

    /// Son ecriture dans l'adresse d'un apercu : sans accent (`simplifie`, `complet`).
    pub fn dans_l_adresse(self) -> &'static str {
        match self {
            ModeleBulletin::Simplifie => "simplifie",
            ModeleBulletin::Complet => "complet",
        }
    }

    /// Le modele qu'ecrit l'adresse d'un apercu (sans accent, ou avec), s'il en est un.
    pub fn de_l_adresse(v: &str) -> Option<ModeleBulletin> {
        match v {
            "simplifie" | "simplifié" => Some(ModeleBulletin::Simplifie),
            "complet" => Some(ModeleBulletin::Complet),
            _ => None,
        }
    }
}

choix! {
    pub enum JourDeLaSemaine {
        Lundi = "lundi", Mardi = "mardi", Mercredi = "mercredi", Jeudi = "jeudi", Vendredi = "vendredi", Samedi = "samedi",
        Dimanche = "dimanche",
    }
}

choix! {
    pub enum AnnexeConvention { A1 = "1", A2 = "2", A3 = "3", A4 = "4", A5 = "5", A6 = "6" }
}

choix! {
    pub enum Secteur2121 { Phonographique = "phonographique", Livre = "livre", Musique = "musique" }
}

choix! {
    pub enum TypeRadio1922 { T1 = "1", T2 = "2", T3 = "3" }
}

choix! {
    pub enum JaugeAnnexe1 { JusquA200 = "<=200", De201A500 = "201-500", PlusDe500 = ">500" }
}

choix! {
    pub enum Subrogation { Oui = "oui" }
}

choix! {
    pub enum JoursFeriesChomes { Tous = "tous", PremierMaiSeulement = "1er mai seulement" }
}

choix! {
    pub enum PlafondTempsPartiel { Proratise = "proratisé", Entier = "entier" }
}

choix! {
    pub enum OptionDfs { AccordCollectif = "accord_collectif", AccordCse = "accord_cse", AccordIndividuel = "accord_individuel" }
}

choix! {
    pub enum RepresentationPersonnel { Aucune = "aucune", Csec = "csec", CseAdherentFnas = "cse_adherent_fnas", Cse = "cse" }
}

choix! {
    pub enum SanteSuspension { Suspendue = "suspendue", Maintenue = "maintenue" }
}

choix! {
    /// S21.G00.44 : le motif de non-assujettissement a la taxe d'apprentissage.
    pub enum MotifNonAssujettissement {
        M001 = "001", M002 = "002", M003 = "003", M004 = "004", M005 = "005", M006 = "006", M007 = "007", M008 = "008",
        M009 = "009", M010 = "010", M011 = "011", M012 = "012", M013 = "013", M014 = "014", M015 = "015", M016 = "016",
        M017 = "017", M018 = "018", M019 = "019",
    }
}

choix! {
    pub enum Tva { Assujettie = "assujettie", Exoneree = "exoneree" }
}

choix! {
    pub enum RegimePrevoyance { Contrat = "contrat" }
}

choix! {
    pub enum ExtensionT2 { Tous = "tous", Cadres = "cadres" }
}

choix! {
    /// S20.G00.07.004.
    pub enum Civilite { Monsieur = "01", Madame = "02" }
}

// ------------------------------------------------------------- les structures

cases! {
    /// Le compte bancaire (IBAN, BIC).
    pub struct Banque, cles CaseBanque { Iban(Iban) = "iban" => iban, Bic(Bic) = "bic" => bic }
}

cases! {
    /// La complementaire sante d'un permanent : le niveau souscrit et la composition couverte.
    pub struct SanteSvp { niveau: crate::forme_bareme::NiveauDeGarantie, conjoint: bool, enfants: Montant }
}

cases! {
    /// Les conges a la reprise, lus sur le dernier bulletin de l'ancien logiciel.
    pub struct CongesReprise {
        date: Jour,
        report: Montant,
        acquis_report: Montant,
        remuneration_report: Montant,
        acquis: Montant,
        pris: Montant,
        remuneration_periode: Montant,
    }
}

cases! {
    /// UN TAUX DE PAS RECU (`droit::pas`) : le taux, son type, l'identifiant et l'instant de sa mise a disposition
    /// (AAAAMMJJHHMMSS), et le mois de la DSN a laquelle il repond.
    pub struct TauxPas {
        taux: crate::pas::TauxDePas,
        r#type: crate::type_taux_pas::TypeDeTauxPas,
        identifiant: crate::pas::IdentifiantDuTauxPas,
        #[serde(default, with = "socle::paris::ecrit::compacte::option")]
        mis_a_disposition: socle::paris::Moment,
        en_retour_de: Mois,
    }
}

cases! {
    /// Le contact de la medecine du travail interne (S20.G00.07).
    pub struct ContactSanteTravail, cles CaseContactSanteTravail {
        NomPrenomAffichage(String) = "nom_prenom_affichage" => nom_prenom_affichage,
        NomPrenom(String) = "nom_prenom" => nom_prenom,
        Tel(String) = "tel" => tel,
        Email(Courriel) = "email" => email,
    }
}

cases! {
    pub struct TauxPrevoyance { t1_non_cadre: Montant, t1_cadre: Montant, part_patronale: Montant }
}

cases! {
    /// La prevoyance des permanents, quand un contrat d'entreprise la fixe.
    pub struct PrevoyancePermanents { regime: RegimePrevoyance, taux: TauxPrevoyance, extension_t2: ExtensionT2, adhesion: String }
}

cases! {
    pub struct SantePermanents { taux_contrat: Montant, part_patronale: Montant }
}

cases! {
    /// Le bloc S21.G00.44 : la compagnie doit-elle la taxe sur les salaires.
    pub struct TaxeSurLesSalaires { tva: Tva, part_tva: Montant, sous_franchise: bool, association_1679a: bool }
}

cases! {
    /// LA NOTIFICATION DE LA CARSAT d'ou viennent un code risque et un taux (retour DSN de nature 34,
    /// `routes::atmp::recue`) : le jour ou elle a paru, celui de son calcul, sa date d'effet.
    pub struct NotificationAtmp { le: Jour, calcul: Jour, effet: Jour }
}

choix! {
    /// D'OU VIENT UNE LIGNE DU TAUX AT/MP (`routes::atmp`) : la saisie, l'import des DSN reprises, la notification
    /// de la CARSAT recue avec les retours, le taux collectif de l'arrete de l'annee.
    pub enum ProvenanceAtmp { Saisie = "saisie", Import = "import", Notification = "notification", Arrete = "arrete" }
}

cases! {
    /// UNE LIGNE DU CODE RISQUE ET DU TAUX AT/MP du siege (`taux_atmp` de la fiche) ou d'un autre etablissement
    /// (`etablissements`), datee comme toute la configuration : le taux vaut a partir de `depuis` (la date d'effet de la
    /// notification), pour l'annee de cette date (`bareme::taux_atmp_de_l_annee`) ; `provenance` dit qui l'a ecrite,
    /// `notification` celle d'ou elle vient.
    pub struct TauxAtmp, cles CaseTauxAtmp {
        CodeRisque(crate::bareme::CodeRisqueAt) = "code_risque" => code_risque,
        Valeur(Montant) = "valeur" => valeur,
        Depuis(Jour) = "depuis" => depuis,
        Provenance(ProvenanceAtmp) = "provenance" => provenance,
        Notification(NotificationAtmp) = "notification" => notification,
    }
}

cases! {
    pub struct VersementMobilite, cles CaseVersementMobilite { Taux(Montant) = "taux" => taux, CodeInsee(CodeInsee) = "code_insee" => code_insee }
}

cases! {
    /// UN AUTRE ETABLISSEMENT que le siege : son SIRET (et le NIC qui s'en lit), son adresse (et la forme declaree qui se
    /// fabrique de la lisible), son ouverture, son taux AT/MP et son versement mobilite.
    pub struct Etablissement, cles CaseEtablissement {
        Libelle(String) = "libelle" => libelle,
        Siret(Siret) = "siret" => siret,
        Nic(Nic) = "nic" => nic,
        AdresseAffichage(String) = "adresse_affichage" => adresse_affichage,
        Adresse(LigneAdresse) = "adresse" => adresse,
        ComplementAdresseAffichage(String) = "complement_adresse_affichage" => complement_adresse_affichage,
        ComplementAdresse(LigneAdresse) = "complement_adresse" => complement_adresse,
        CodePostal(CodePostal) = "code_postal" => code_postal,
        CommuneAffichage(String) = "commune_affichage" => commune_affichage,
        Commune(CommuneEmployeur) = "commune" => commune,
        Ouverture(Jour) = "ouverture" => ouverture,
        TauxAtmp(TauxAtmp) = "taux_atmp" => taux_atmp,
        VersementMobilite(VersementMobilite) = "versement_mobilite" => versement_mobilite,
    }
}

cases! {
    pub struct Signataire, cles CaseSignataire {
        PrenomAffichage(String) = "prenom_affichage" => prenom_affichage,
        NomAffichage(String) = "nom_affichage" => nom_affichage,
        Prenom(String) = "prenom" => prenom,
        Nom(String) = "nom" => nom,
        Fonction(String) = "fonction" => fonction,
        Tel(Telephone) = "tel" => tel,
        Email(Courriel) = "email" => email,
    }
}

cases! {
    /// Le contact DSN (S20.G00.07).
    pub struct DeclarantDsn, cles CaseDeclarantDsn {
        Civilite(Civilite) = "civilite" => civilite,
        NomPrenomAffichage(String) = "nom_prenom_affichage" => nom_prenom_affichage,
        NomPrenom(String) = "nom_prenom" => nom_prenom,
        Tel(Telephone) = "tel" => tel,
        Email(Courriel) = "email" => email,
        Qualite(String) = "qualite" => qualite,
    }
}

cases! {
    /// Le declarant inscrit au service DSN de net-entreprises, tel qu'inscrit (`envois::transport::declarant`).
    pub struct Declarant { siret: Siret, nom: String, prenom: String }
}

cases! {
    /// Le compte DPAE d'urssaf.fr (`envois::releve_retours`) : son titulaire, son SIRET (celui de l'entreprise sinon), son
    /// service.
    pub struct ApiDpae { nom: String, prenom: String, siret: Siret, service: String }
}

// ------------------------------------------------------------------ les fiches

fiche! {
    /// LA FICHE D'UN SALARIE a un jour.
    pub struct ChampsSalarie, datee FicheSalarieDatee, changement ChangementSalarie, cles CaseSalarie {
        CodeSalarie(Matricule) = "code_salarie" => code_salarie,
        InitialesContrat(String) = "initiales_contrat" => initiales_contrat,
        NomAffichage(String) = "nom_affichage" => nom_affichage,
        Nom(NomDeclare) = "nom" => nom,
        PrenomAffichage(String) = "prenom_affichage" => prenom_affichage,
        Prenom(PrenomDeclare) = "prenom" => prenom,
        Pseudonyme(String) = "pseudonyme" => pseudonyme,
        Nir(Nir) = "nir" => nir,
        NirCle(CleNir) = "nir_cle" => nir_cle,
        Sexe(Sexe) = "sexe" => sexe,
        DateNaissance(Jour) = "date_naissance" => date_naissance,
        LieuNaissance(LieuNaissance) = "lieu_naissance" => lieu_naissance,
        LieuNaissanceAffichage(String) = "lieu_naissance_affichage" => lieu_naissance_affichage,
        DepartementNaissance(DepartementNaissance) = "departement_naissance" => departement_naissance,
        PaysNaissance(socle::pays::CodePays) = "pays_naissance" => pays_naissance,
        CodeNationaliteDsn(CodeNationalite) = "code_nationalite_dsn" => code_nationalite_dsn,
        PaysNationalite(socle::pays::CodePays) = "pays_nationalite" => pays_nationalite,
        PaysNationaliteHorsListe(String) = "pays_nationalite_hors_liste" => pays_nationalite_hors_liste,
        Adresse(String) = "adresse" => adresse,
        AdresseAffichage(String) = "adresse_affichage" => adresse_affichage,
        CodePostal(String) = "code_postal" => code_postal,
        Commune(String) = "commune" => commune,
        CommuneAffichage(String) = "commune_affichage" => commune_affichage,
        Email(Courriel) = "email" => email,
        Banque(Banque) = "banque" => banque,
        NumCongesSpectacles(String) = "num_conges_spectacles" => num_conges_spectacles,
        EmploiAffichage(String) = "emploi_affichage" => emploi_affichage,
        Emploi(String) = "emploi" => emploi,
        PcsEse(CodePcsEse) = "pcs_ese" => pcs_ese,
        CodeEmploiFt(CodeEmploiFt) = "code_emploi_ft" => code_emploi_ft,
        Statut(Statut) = "statut" => statut,
        StatutConventionnel(StatutConventionnel) = "statut_conventionnel" => statut_conventionnel,
        StatutCategorielRc(StatutCategorielRc) = "statut_categoriel_rc" => statut_categoriel_rc,
        DeductionForfaitaire(ProfessionDfs) = "deduction_forfaitaire" => deduction_forfaitaire,
        DeductionForfaitaireAccord(AccordDfs) = "deduction_forfaitaire_accord" => deduction_forfaitaire_accord,
        StatutBoeth(StatutBoeth) = "statut_boeth" => statut_boeth,
        ClassementConventionnel(String) = "classement_conventionnel" => classement_conventionnel,
        DateAnciennete(Jour) = "date_anciennete" => date_anciennete,
        Actif(bool) = "actif" => actif,
        AnniversaireDroits(Jour) = "anniversaire_droits" => anniversaire_droits,
        ModeleBulletin(ModeleBulletin) = "modele_bulletin" => modele_bulletin,
        TauxPas(TauxPas) = "taux_pas" => taux_pas,
    }
    pieces CasePieceSalarie {
        /// LES CASES D'UN SALARIE AU MOIS : sa complementaire sante, sa prevoyance, ses jours, sa categorie, ses conges
        /// repris. Seul le calcul d'un engagement paye au mois les ouvre (`natures::AuMois`).
        permanent: Permanent, CasesDuPermanent, crate::natures::AuMois {
            SanteSvp(SanteSvp) = "sante_svp" => sante_svp,
            JoursTravailles(Vec<JourDeLaSemaine>) = "jours_travailles" => jours_travailles,
            ForfaitJours(bool) = "forfait_jours" => forfait_jours,
            SanteDispense(String) = "sante_dispense" => sante_dispense,
            SanteDispenseDepuis(Jour) = "sante_dispense_depuis" => sante_dispense_depuis,
            SanteDispenseJusquAu(Jour) = "sante_dispense_jusqu_au" => sante_dispense_jusqu_au,
            PrevoyanceBrancheDepuis(Jour) = "prevoyance_branche_depuis" => prevoyance_branche_depuis,
            CategorieConventionnelle(CategorieConventionnelle) = "categorie_conventionnelle" => categorie_conventionnelle,
            CongesReprise(CongesReprise) = "conges_reprise" => conges_reprise,
        }
        /// LES CASES DU METIER : ce que seule une grille des minima qui vise le metier d'un engagement lit (l'echelon de
        /// la CCNEAC, le poste depuis lequel une anciennete se compte, l'artiste stagiaire, l'experience de la 3241).
        /// Seule une grille trouvee les ouvre (`minima::ParLaGrille`).
        metier: Metier, CasesDuMetier, crate::minima::ParLaGrille {
            EchelonCcneac(EchelonCcneac) = "echelon_ccneac" => echelon_ccneac,
            PosteDepuis(Jour) = "poste_depuis" => poste_depuis,
            StagiaireCcneac(StagiaireCcneac) = "stagiaire_ccneac" => stagiaire_ccneac,
            Experimente3241(bool) = "experimente_3241" => experimente_3241,
        }
    }
}

fiche! {
    /// LA FICHE DE L'ENTREPRISE a un jour (son siege et, en `etablissements`, les autres).
    pub struct ChampsEntreprise, datee FicheEntrepriseDatee, changement ChangementEntreprise, cles CaseEntreprise {
        NomAffichage(String) = "nom_affichage" => nom_affichage,
        RaisonSociale(RaisonSociale) = "raison_sociale" => raison_sociale,
        Siren(Siren) = "siren" => siren,
        Nic(Nic) = "nic" => nic,
        Siret(Siret) = "siret" => siret,
        Naf(CodeNaf) = "naf" => naf,
        CodeUrssaf(String) = "code_urssaf" => code_urssaf,
        Adresse(LigneAdresse) = "adresse" => adresse,
        AdresseAffichage(String) = "adresse_affichage" => adresse_affichage,
        ComplementAdresse(LigneAdresse) = "complement_adresse" => complement_adresse,
        ComplementAdresseAffichage(String) = "complement_adresse_affichage" => complement_adresse_affichage,
        CodePostal(CodePostal) = "code_postal" => code_postal,
        Commune(CommuneEmployeur) = "commune" => commune,
        CommuneAffichage(String) = "commune_affichage" => commune_affichage,
        Idcc(Idcc) = "idcc" => idcc,
        AnnexeConvention(AnnexeConvention) = "annexe_convention" => annexe_convention,
        Secteur2121(Secteur2121) = "secteur_2121" => secteur_2121,
        TypeRadio1922(TypeRadio1922) = "type_radio_1922" => type_radio_1922,
        CinquanteSalaries3241(bool) = "cinquante_salaries_3241" => cinquante_salaries_3241,
        UrssafCompte(String) = "urssaf_compte" => urssaf_compte,
        MedecineTravail(CodeMedecine) = "medecine_travail" => medecine_travail,
        ContactSanteTravail(ContactSanteTravail) = "contact_sante_travail" => contact_sante_travail,
        LicenceSpectacle(String) = "licence_spectacle" => licence_spectacle,
        FranceTravailDossierRecouvrement(String) = "france_travail_dossier_recouvrement" => france_travail_dossier_recouvrement,
        CongesspectaclesAdherent(String) = "congesspectacles_adherent" => congesspectacles_adherent,
        AffiliationFranceTravail(String) = "affiliation_france_travail" => affiliation_france_travail,
        RetraiteComplementaire(String) = "retraite_complementaire" => retraite_complementaire,
        OrganisationPatronale(crate::forme_bareme::OrganisationPatronale) = "organisation_patronale" => organisation_patronale,
        OrganisationPatronaleDepuis(Jour) = "organisation_patronale_depuis" => organisation_patronale_depuis,
        EmploieDesPermanents(bool) = "emploie_des_permanents" => emploie_des_permanents,
        DateDeVersementIntermittents(crate::versement::Versement) = "date_de_versement_intermittents" => date_de_versement_intermittents,
        JoursFeriesChomes(JoursFeriesChomes) = "jours_feries_chomes" => jours_feries_chomes,
        DeductionForfaitaireOption(OptionDfs) = "deduction_forfaitaire_option" => deduction_forfaitaire_option,
        PremiereEmbauche(Jour) = "premiere_embauche" => premiere_embauche,
        TaxeSurLesSalaires(TaxeSurLesSalaires) = "taxe_sur_les_salaires" => taxe_sur_les_salaires,
        TaxeApprentissageMotifNonAssujettissement(MotifNonAssujettissement) = "taxe_apprentissage_motif_non_assujettissement" => taxe_apprentissage_motif_non_assujettissement,
        TauxAtmp(TauxAtmp) = "taux_atmp" => taux_atmp,
        Etablissements(Vec<Etablissement>) = "etablissements" => etablissements,
        Signataire(Signataire) = "signataire" => signataire,
        DeclarantDsn(DeclarantDsn) = "declarant_dsn" => declarant_dsn,
        Declarant(Declarant) = "declarant" => declarant,
        ApiDpae(ApiDpae) = "api_dpae" => api_dpae,
        Banque(Banque) = "banque" => banque,
        CourrielNom(String) = "courriel_nom" => courriel_nom,
        CourrielSignature(String) = "courriel_signature" => courriel_signature,
        ModeleBulletin(ModeleBulletin) = "modele_bulletin" => modele_bulletin,
        Conseils(bool) = "conseils" => conseils,
    }
    pieces CasePieceEntreprise {
        /// LES CASES DE L'ENTREPRISE QUE SEUL LE CALCUL D'UN SALARIE AU MOIS LIT : la subrogation de son maintien, le
        /// jour de son versement, sa prevoyance et sa sante, son plafond a temps partiel, sa sante pendant une suspension
        /// non indemnisee (`natures::AuMois`).
        permanent: Permanent, CasesDesPermanents, crate::natures::AuMois {
            SubrogationIj(Subrogation) = "subrogation_ij" => subrogation_ij,
            DateDeVersementPermanents(crate::versement::Versement) = "date_de_versement_permanents" => date_de_versement_permanents,
            PrevoyancePermanents(PrevoyancePermanents) = "prevoyance_permanents" => prevoyance_permanents,
            PlafondTempsPartiel(PlafondTempsPartiel) = "plafond_temps_partiel" => plafond_temps_partiel,
            SantePermanents(SantePermanents) = "sante_permanents" => sante_permanents,
            SanteSuspensionNonIndemnisee(SanteSuspension) = "sante_suspension_non_indemnisee" => sante_suspension_non_indemnisee,
        }
        /// LES CASES D'UNE COMPAGNIE QUI EMPLOIE DES PERMANENTS : « au moins cinquante salaries », que seule la case
        /// « emploie des permanents » cochee ouvre (`ChampsEntreprise::avec_permanents`).
        avec_permanents: EmploieDesPermanents, CasesAvecPermanents, EmploieDesPermanents {
            CinquanteSalariesEtPlus(bool) = "cinquante_salaries_et_plus" => cinquante_salaries_et_plus,
        }
        /// LA JAUGE DE LA SALLE, que seule la prime d'anciennete d'un technicien de l'annexe I paye au mois lit
        /// (`clauses::prime_anciennete`, que seul le calcul d'un engagement au mois appelle) : la cle de sa grille
        /// (`minima::ParLaGrille`) et celle d'un salarie au mois (`natures::AuMois`).
        metier: MetierAuMois, CasesDuMetierEntreprise, (crate::minima::ParLaGrille, crate::natures::AuMois) {
            JaugeAnnexe1(JaugeAnnexe1) = "jauge_annexe_1" => jauge_annexe_1,
        }
        /// LA REPRESENTATION DU PERSONNEL : seul un effectif qui laisse un comite exister l'ouvre
        /// (`contexte::UnCsePossible`).
        cse: Cse, CasesDuCse, crate::contexte::UnCsePossible {
            RepresentationPersonnel(RepresentationPersonnel) = "representation_personnel" => representation_personnel,
        }
        /// L'EXEMPTION DE LA CASC-SVP (3090, art. 4.22 B) : seule une compagnie d'au moins cinquante salaries dotee d'un
        /// comite l'ouvre (`ChampsEntreprise::exemption_casc`).
        casc: Casc, CasesDeLaCasc, ExemptionCasc {
            CascSvpExemptee(bool) = "casc_svp_exemptee" => casc_svp_exemptee,
        }
    }
    notes {
        /// D'ou vient le fichier (une compagnie d'essai le dit).
        note,
    }
}

choix! {
    /// Le genre d'un programme audiovisuel (2642) : les minima de ses techniciens en dependent.
    pub enum GenreAudiovisuel { Fiction = "fiction", Flux = "flux", Autre = "autre" }
}

choix! {
    /// Le format d'un film (3097) : les minima de ses comediens en dependent.
    pub enum FormatCinema { Long = "long", Court = "court" }
}

fiche! {
    /// LA FICHE D'UN SPECTACLE a un jour (`config/spectacles.json`). Ses notes, ecrites par l'import, disent d'ou il
    /// vient et le titre que les bulletins de l'ancien logiciel proposent (`declarations::demarrage`).
    pub struct ChampsSpectacle, datee FicheSpectacleDatee, changement ChangementSpectacle, cles CaseSpectacle {
        Titre(String) = "titre" => titre,
        NumeroObjet(NumeroObjetSpectacle) = "numero_objet" => numero_objet,
        CodeAnalytique(String) = "code_analytique" => code_analytique,
        GenreAudiovisuel(GenreAudiovisuel) = "genre_audiovisuel" => genre_audiovisuel,
        EmissionTv1734(bool) = "emission_tv_1734" => emission_tv_1734,
        FormatCinema(FormatCinema) = "format_cinema" => format_cinema,
        DerogationAnnexeIii3097(bool) = "derogation_annexe_iii_3097" => derogation_annexe_iii_3097,
        Alias(Vec<String>) = "alias" => alias,
        Actif(bool) = "actif" => actif,
    }
    pieces CasePieceSpectacle {}
    notes {
        /// D'ou vient la fiche (ce que l'import a lu).
        note,
        titre_propose,
        titre_lu_dans,
    }
}

impl FormeDeCase for crate::catalogue::PlafondAnnuelHeures {}
impl FormeDeCase for crate::emplois::AnnexeChomage {}

fiche! {
    /// UNE RUBRIQUE TELLE QU'UN FICHIER L'ECRIT, case par case : livree (`rubriques-base.json`, de tout temps), ou la
    /// compagnie par-dessus, case par case, datee comme toute la configuration (`config/rubriques-compagnie.json`).
    /// `catalogue::construire` en fait la `Rubrique` du calcul.
    pub struct ChampsRubrique, datee RubriqueDatee, changement ChangementRubrique, cles CaseRubrique {
        Libelle(String) = "libelle" => libelle,
        Unite(crate::catalogue::Unite) = "unite" => unite,
        Nature(crate::catalogue::NatureDeRubrique) = "nature" => nature,
        Saisie(Vec<crate::catalogue::CaseSaisie>) = "saisie" => saisie,
        Soumis(bool) = "soumis" => soumis,
        Imposable(bool) = "imposable" => imposable,
        CongesSpectacles(bool) = "conges_spectacles" => conges_spectacles,
        Source(String) = "source" => source,
        Sens(crate::catalogue::Sens) = "sens" => sens,
        HeuresForfait(Montant) = "heures_forfait" => heures_forfait,
        Plafond(crate::catalogue::Unite) = "plafond" => plafond,
        Aem(crate::catalogue::Unite) = "aem" => aem,
        DsnType(String) = "dsn_type" => dsn_type,
        DsnAutresBruts(crate::autre_revenu_brut::AutreElementDeRevenuBrut) = "dsn_autres_bruts" => dsn_autres_bruts,
        Annexe(crate::emplois::AnnexeChomage) = "annexe" => annexe,
        PlafondMensuel(i64) = "plafond_mensuel" => plafond_mensuel,
        PlafondAnnuelHeures(crate::catalogue::PlafondAnnuelHeures) = "plafond_annuel_heures" => plafond_annuel_heures,
        ComptePour(crate::catalogue::ComptePour) = "compte_pour" => compte_pour,
        SalaireDeBase(bool) = "salaire_de_base" => salaire_de_base,
        Retabli(bool) = "retabli" => retabli,
        Suspension(bool) = "suspension" => suspension,
        ArretDeTravail(bool) = "arret_de_travail" => arret_de_travail,
        CongesPris(bool) = "conges_pris" => conges_pris,
        DsnBloc52(crate::type_de_prime::TypeDePrime) = "dsn_bloc52" => dsn_bloc52,
        Dit(String) = "dit" => dit,
    }
    pieces CasePieceRubrique {}
}

/// Les trois fiches que la table des cases juge (`FicheSaisie`).
macro_rules! fiche_saisie {
    ($($plate:ident = $nom:ident),*) => {
        $(
            impl FicheSaisie for $plate {
                const NOM: NomDeFiche = NomDeFiche::$nom;
                fn ecrite_au_chemin(&self, k: CaseDeFiche, chemin: &[&str]) -> MotEcrit {
                    $plate::ecrite_au_chemin(self, k, chemin)
                }
                fn lire_cases(o: &socle::forme::Objet) -> R<(Self, Vec<FauteDeCase>)> {
                    $plate::lire_cases_de_la_fiche(o, NomDeFiche::$nom.nom())
                }
                fn valeur_ecrite(&self, k: CaseDeFiche) -> Option<ValeurDeCase> {
                    $plate::valeur_ecrite(self, k)
                }
                fn est_vide(&self) -> bool {
                    $plate::est_vide(self)
                }
                fn fondre(&mut self, autre: &Self) {
                    $plate::fondre(self, autre)
                }
                fn completer_de(&mut self, autre: &Self) {
                    $plate::completer_de(self, autre)
                }
                fn prendre(&mut self, autre: &Self, k: CaseDeFiche) {
                    $plate::prendre(self, autre, k)
                }
                fn cases_dites(&self) -> Vec<CaseDeFiche> {
                    $plate::cases_dites(self)
                }
                fn notes_dites(&self) -> Vec<(&'static str, &str)> {
                    $plate::notes_dites(self)
                }
                fn prendre_la_note(&mut self, autre: &Self, cle: &str) {
                    $plate::prendre_la_note(self, autre, cle)
                }
            }
        )*
    };
}
fiche_saisie!(ChampsEntreprise = Entreprise, ChampsSalarie = Salarie, ChampsSpectacle = Spectacle);

impl ChampsSpectacle {
    /// Archivee : `actif: false`. L'absence vaut actif.
    pub fn archivee(&self) -> bool {
        self.actif.dite() == Some(&false)
    }
}

/// UN SPECTACLE TROUVE (`contexte::Contexte::spectacle`) : sa cle dans `config/spectacles.json`, et sa fiche au jour.
#[derive(Debug, Clone, PartialEq)]
pub struct Spectacle {
    pub cle: IdSpectacle,
    pub champs: ChampsSpectacle,
}

impl std::ops::Deref for Spectacle {
    type Target = ChampsSpectacle;
    fn deref(&self) -> &ChampsSpectacle {
        &self.champs
    }
}

impl ChampsSalarie {
    /// « NOM Prénom », tel que l'ecran le nomme (`socle::texte::salarie_nomme`) : le nom et le prenom d'usage, sinon ceux de
    /// l'etat civil. `None` : la fiche ne nomme personne.
    pub fn nomme(&self) -> Option<String> {
        socle::texte::salarie_nomme(self.nom.portee(), self.nom_affichage.portee(), self.prenom.portee(), self.prenom_affichage.portee())
    }

    /// LE SEXE DU SALARIE, la seule ecriture de cette regle : celui que dit son NIR (`Nir::sexe`), sinon la case `sexe` ;
    /// aucun si ni l'un ni l'autre ne le dit.
    pub fn sexe(&self) -> Option<Sexe> {
        match self.nir.dite().and_then(Nir::sexe) {
            Some(s) => Some(s),
            None => self.sexe,
        }
    }

    /// Ce que la fiche dit de la nationalite (`crate::nationalite`, S21.G00.30.013).
    pub fn nationalite(&self) -> crate::nationalite::Nationalite {
        crate::nationalite::Nationalite::nouvelle(
            self.code_nationalite_dsn.dite().copied(),
            self.pays_nationalite.dite().cloned(),
            self.pays_nationalite_hors_liste.portee(),
            self.nomme(),
        )
    }

    /// Le pays de naissance tel que la DSN le declare (S21.G00.30.015, `socle::pays::de_naissance`).
    pub fn pays_de_naissance(&self) -> R<Option<String>> {
        let qui = [self.prenom.portee(), self.nom.portee()].into_iter().flatten().collect::<Vec<_>>().join(" ").trim().to_string();
        self.pays_naissance.as_ref().map(|code| socle::pays::de_naissance(code, &qui)).transpose()
    }

    /// Archivee : `actif: false`. L'absence vaut actif.
    pub fn archivee(&self) -> bool {
        self.actif.dite() == Some(&false)
    }
}

impl ChampsEntreprise {
    /// Le NIC du siege, tel que la fiche le dit.
    pub fn nic_principal(&self) -> Option<&Nic> {
        self.nic.dite()
    }

    /// Les etablissements autres que le siege.
    pub fn secondaires(&self) -> Vec<&Etablissement> {
        let nic = self.nic.dite();
        self.etablissements.dite().into_iter().flatten().filter(|e| e.nic.dite() != nic).collect()
    }

    /// L'autre etablissement de ce NIC, `None` : le siege, ou un NIC que la fiche ne porte pas.
    pub fn etablissement(&self, nic: &Nic) -> Option<&Etablissement> {
        self.secondaires().into_iter().find(|e| e.nic.dite() == Some(nic))
    }

    /// Le NIC du siege, puis celui de chaque autre etablissement : une DSN par etablissement. Chacun s'exige : sans lui,
    /// la DSN de l'etablissement ne s'ecrit pas.
    pub fn nics(&self) -> R<Vec<Nic>> {
        std::iter::once(self.nic.exigee("nic")).chain(self.secondaires().iter().map(|e| e.nic.exigee("etablissements.nic"))).map(|n| n.cloned()).collect()
    }

    /// Les feries qu'elle chome, tels que `socle::feries::chomes` les lit ; `None` : la fiche n'en dit rien.
    pub fn feries_chomes(&self) -> Option<&'static str> {
        self.jours_feries_chomes.dite().map(Choix::code)
    }

    /// La cle des cases d'une compagnie qui emploie des permanents : sa case « emploie des permanents » cochee.
    pub fn avec_permanents(&self) -> Option<EmploieDesPermanents> {
        self.emploie_des_permanents.oui().then_some(EmploieDesPermanents(()))
    }

    /// LA CLE DE L'EXEMPTION DE LA CASC-SVP : la compagnie compte au moins cinquante salaries, un comite peut exister, et
    /// la fiche dit un CSE (`cse` ou `cse_adherent_fnas` ; le CSE conventionnel de la CCNEAC n'est pas celui de la 3090).
    pub fn exemption_casc(&self, _cinquante: &crate::contexte::CinquanteSalaries, cse: &crate::contexte::UnCsePossible) -> Option<ExemptionCasc> {
        matches!(self.cse.ouvrir(cse).representation_personnel.code(), Some("cse" | "cse_adherent_fnas")).then_some(ExemptionCasc(()))
    }

    /// Le nom de la compagnie tel que l'ecran le dit : le nom d'usage, sinon la raison sociale.
    pub fn nommee(&self) -> Option<&str> {
        self.nom_affichage.remplie().or_else(|| self.raison_sociale.portee())
    }
}

/// LA FICHE D'UN SALARIE A UN JOUR, avec sa cle : ce que lit tout ce qui le paie ou le declare.
#[derive(Debug, Clone, PartialEq)]
pub struct FicheSalarie {
    pub id: IdSalarie,
    pub champs: ChampsSalarie,
}

impl std::ops::Deref for FicheSalarie {
    type Target = ChampsSalarie;
    fn deref(&self) -> &ChampsSalarie {
        &self.champs
    }
}

impl std::ops::DerefMut for FicheSalarie {
    fn deref_mut(&mut self) -> &mut ChampsSalarie {
        &mut self.champs
    }
}

/// LA FICHE QUE LA PAIE D'UN CONTRAT LIT : celle du salarie a la date de la periode, telle que l'engagement la voit (son
/// emploi l'emporte, `emplois::emploi_de`), avec ce que le contrat y ajoute : la convention de l'employeur
/// (`caisse_conges::intermittent`), l'etablissement de rattachement (S21.G00.11.001), et la sante suspendue d'un mois
/// sans indemnisation (`bareme::sans_sante`). Elle s'ecrit a cote de la paie (`engagement::relire::FichesDuCalcul`)
/// par sa forme (`Figee`) et se relit telle quelle (`lire`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Figee", into = "Figee")]
pub struct SalarieDuContrat {
    pub fiche: FicheSalarie,
    pub idcc_de_l_employeur: Option<Idcc>,
    pub etablissement: Option<Nic>,
    pub sante_suspendue: Option<String>,
}

/// La forme ecrite de `SalarieDuContrat` : la cle du salarie, ses champs, et ce que le contrat y ajoute.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Figee {
    cle: IdSalarie,
    champs: ChampsSalarie,
    #[serde(skip_serializing_if = "Option::is_none")]
    idcc_de_l_employeur: Option<Idcc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    etablissement: Option<Nic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sante_suspendue: Option<String>,
}

impl From<Figee> for SalarieDuContrat {
    fn from(f: Figee) -> SalarieDuContrat {
        SalarieDuContrat { fiche: FicheSalarie { id: f.cle, champs: f.champs }, idcc_de_l_employeur: f.idcc_de_l_employeur, etablissement: f.etablissement, sante_suspendue: f.sante_suspendue }
    }
}

impl From<SalarieDuContrat> for Figee {
    fn from(s: SalarieDuContrat) -> Figee {
        Figee { cle: s.fiche.id, champs: s.fiche.champs, idcc_de_l_employeur: s.idcc_de_l_employeur, etablissement: s.etablissement, sante_suspendue: s.sante_suspendue }
    }
}

impl std::ops::Deref for SalarieDuContrat {
    type Target = FicheSalarie;
    fn deref(&self) -> &FicheSalarie {
        &self.fiche
    }
}

impl std::ops::DerefMut for SalarieDuContrat {
    fn deref_mut(&mut self) -> &mut FicheSalarie {
        &mut self.fiche
    }
}

impl SalarieDuContrat {
    /// La fiche, sans rien que le contrat ajoute.
    pub fn de(fiche: FicheSalarie) -> SalarieDuContrat {
        SalarieDuContrat { fiche, idcc_de_l_employeur: None, etablissement: None, sante_suspendue: None }
    }
}

// ------------------------------------------------------------- les fichiers

/// La note d'un fichier de fiches.
pub const NOTE_DU_FICHIER: &str = "note";

/// Cette cle est-elle une note d'une fiche de la configuration (`notes` de `ChampsEntreprise` et de `ChampsSpectacle`) :
/// un texte qui n'est pas une case, que l'ecran ne montre pas et qu'une ecriture garde.
pub fn est_une_note(cle: &str) -> bool {
    ChampsEntreprise::NOTES.contains(&cle) || ChampsSpectacle::NOTES.contains(&cle)
}

/// UN FICHIER DE FICHES (`config/salaries.json`, `config/spectacles.json`) : sa `note`, et ses fiches par cle (`K`), dans
/// l'ordre du fichier. Une cle repetee se refuse (`socle::forme::fiches_et_note`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FichierDeFiches<K: std::hash::Hash + Eq, F> {
    #[serde(rename = "note", skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(flatten)]
    pub fiches: IndexMap<K, F>,
}

impl<K: std::hash::Hash + Eq, F> Default for FichierDeFiches<K, F> {
    fn default() -> Self {
        FichierDeFiches { source: None, fiches: IndexMap::new() }
    }
}

impl<'de, K: std::hash::Hash + Eq + Deserialize<'de>, F: Deserialize<'de>> Deserialize<'de> for FichierDeFiches<K, F> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (source, fiches) = socle::forme::fiches_et_note(d, NOTE_DU_FICHIER)?;
        Ok(FichierDeFiches { source, fiches: fiches.into_iter().collect() })
    }
}

/// `config/salaries.json`.
pub type Salaries = FichierDeFiches<IdSalarie, FicheSalarieDatee>;

/// `config/spectacles.json`.
pub type Spectacles = FichierDeFiches<IdSpectacle, FicheSpectacleDatee>;

/// LA LECTURE D'UN FICHIER DE FICHES (`config/entreprise.json` en `FicheEntrepriseDatee`, `config/salaries.json` en
/// `Salaries`, `config/spectacles.json` en `Spectacles`), la seule : `fichier` le nomme dans les refus, qui disent la cle
/// fautive jusqu'a sa case et quoi faire.
pub fn lire_fichier<T: DeserializeOwned>(chemin: &std::path::Path, fichier: &str) -> R<T> {
    socle::forme::lire_fichier_en_phrase(chemin, |cle, detail| refus_de_fiche(fichier, cle, detail))
}

/// Le refus d'une fiche qui ne se lit pas : le fichier, le chemin de la valeur (la cle de la fiche d'un salarie ou d'un
/// spectacle, sa case, le rang de la valeur datee : « FULLER.nir_cle[0].valeur »), ce qui ne va pas (la phrase du type
/// qui la refuse), et quoi faire.
fn refus_de_fiche(fichier: &str, chemin: &str, detail: &str) -> String {
    let ou = if chemin.is_empty() { String::new() } else { format!(", à « {} »", chemin) };
    format!(
        "{}{} ne se lit pas : {}. Ce fichier a été écrit par une version précédente du logiciel ou à la main : ajouter la \
compagnie à nouveau (« Ajouter une compagnie », d'un logiciel précédent) ou corriger cette clé ou sa valeur dans le fichier.",
        fichier, ou, detail
    )
}

/// Le refus d'une ecriture de fiche que sa forme ne lit pas : rien ne s'ecrit, la valeur se corrige.
fn refus_d_ecriture(fichier: &str, chemin: &str, detail: &str) -> String {
    let ou = if chemin.is_empty() { String::new() } else { format!(", à « {} »", chemin) };
    format!("{}{} ne s'écrirait pas : {}. Rien n'est écrit : corriger cette valeur.", fichier, ou, detail)
}

/// CE QU'UNE ECRITURE VA POSER DANS LE FICHIER `fichier`, lu par sa forme avant de s'ecrire : ce qui ne se lit pas se
/// refuse en nommant la cle jusqu'a sa case, et rien n'est ecrit.
pub fn lire_saisie<T: DeserializeOwned>(v: &(impl Serialize + ?Sized), fichier: &str) -> R<T> {
    socle::forme::relire_en_phrase(v, |chemin, detail| refus_d_ecriture(fichier, chemin, detail))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_siret_se_controle_par_luhn() {
        // insee-nir-siret : la somme des chiffres (rang impair doubles) est un multiple de 10.
        assert!(Siret::lire("73282932000074").unwrap().cle_juste());
        assert!(!Siret::lire("73282932000075").unwrap().cle_juste());
    }

    /// Une fiche se relit et se reecrit telle quelle : les cases nulles et vides comprises.
    #[test]
    fn une_fiche_se_reecrit_telle_qu_elle_est_lue() {
        let texte = r#"{
            "note": "note",
            "FULLER": {
                "nom": [{"valeur": "FULLER", "note": "importe"}],
                "actif": [{"valeur": null}, {"depuis": "2026-11-02", "valeur": true}],
                "taux_pas": [{"valeur": null}, {"depuis": "2026-11-20", "valeur": {"taux": "1.90", "type": "01", "identifiant": "1", "mis_a_disposition": "20261120000000"}}],
                "conges_reprise": [{"valeur": {"date": "2026-01-01", "report": 30, "remuneration_report": 47321.04}}]
            }
        }"#;
        let s: Salaries = socle::forme::lire_texte(texte, "config/salaries.json").unwrap();
        let ecrit = socle::ecriture::json_en_ligne(&s);
        let relu: Salaries = socle::forme::lire_texte(&ecrit, "config/salaries.json").unwrap();
        assert_eq!(socle::ecriture::json_en_ligne(&relu), ecrit);
        for garde in [r#""actif":[{"valeur":null}"#, r#""taux_pas":[{"valeur":null}"#, r#""note":"importe""#, r#""remuneration_report":47321.04"#] {
            assert!(ecrit.contains(garde), "{} manque à la réécriture : {}", garde, ecrit);
        }
        let f = &s.fiches[&IdSalarie::lire("FULLER").unwrap()];
        assert_eq!(f.au(&Jour::nouveau(2026, 11, 1).unwrap()).actif, None);
        assert!(f.au(&Jour::nouveau(2026, 11, 2).unwrap()).actif.oui());
    }
}
