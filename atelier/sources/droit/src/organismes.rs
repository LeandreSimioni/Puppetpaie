//! LES ORGANISMES ET LES CONTRATS DE LA DSN d'une compagnie, composes a une date (`Contexte::organismes`) : ce que la
//! DSN, les signalements, l'amorcage et le contrat ecrit lisent des organismes.
//!
//! Tout se compose a la date lue, depuis les textes livres et la configuration de cette date :
//! - la partie normative (organismes payes, CTP, bloc 08) vient de `donnees/organismes-base.json` (`Base`), la meme
//!   pour toutes ;
//! - les contrats attendus aussi (`contrats` : ceux de toutes les compagnies, puis ceux de la convention en vigueur a
//!   cette date), plus la CASC-SVP (`casc-svp-base.json`, `adhesions::casc_svp`) et les contributions de branche en DSN
//!   (`contributions_dsn` du bareme de cette date) ;
//! - de la compagnie, seulement ce qui est a elle (`config/organismes.json`, ecrit par Configuration › Organismes et
//!   contrats) : la reference de ses contrats chez l'organisme quand aucun texte ne la donne, l'organisme quand la
//!   branche n'en designe pas, et les identifiants d'adhesion deja attribues.
//!
//! Une compagnie qui n'a rien a saisir (les intermittents d'une 3090 : tous leurs contrats sont donnes par les textes)
//! n'a donc rien a enregistrer. Ce qui manque ne se devine pas : un contrat des permanents sans reference laisse les
//! references des permanents vides (`ParNature`), et la DSN refuse le permanent (`dsn::affiliations`), pas la compagnie.
//!
//! `config/organismes.json` est DATE comme toute la configuration (`fiche::dater`) : sa cle
//! `adhesions_prevoyance` est une liste `{depuis, valeur}`. Les identifiants d'adhesion (S21.G00.15.005) ne bougent
//! jamais : relus dans le fichier, et un identifiant nouveau n'en reprend aucun qu'une autre periode porte.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use socle::date::Jour;
use socle::fichiers::{forme_en_cache, FormeGardee};
use socle::echec::R;

use crate::contexte::{Contexte, ORGANISMES};
use crate::fiche::{Datee, Idcc, Lue, Piece};
use crate::source::{forme_notee, Fondement};

/// Audiens, delegataire des contrats de branche du spectacle (S21.G00.15.002).
pub const AUDIENS: IdentifiantDOrganisme = IdentifiantDOrganisme::du_moteur("P0983");
/// Le premier identifiant d'adhesion libre : ceux d'en dessous sont ceux que les textes donnent (`reference` des
/// contrats de `organismes-base.json`, `identifiant` de la CASC-SVP).
const PREMIER_LIBRE: u32 = 104;

// ------------------------------------------------------------------------------------------ la partie normative

forme_notee! {
    /// `donnees/organismes-base.json`, entier.
    pub struct Base {
        pub versements: Vec<Versement>,
        pub sans_individu: SansIndividu,
        pub ctp_urssaf: TableCtp,
        pub contrats: Contrats,
    }
}

forme_notee! {
    /// UN ORGANISME QUE LA DSN PAIE (bloc S21.G00.20) : son identifiant (20.001), son libelle, son nom propre, ce que
    /// couvre son versement, le jour du mois suivant ou il preleve quand un texte le dit, ses cles de totalisation (les
    /// codes 81 qu'il recoit), et son adresse quand le contrat ecrit la nomme.
    pub struct Versement {
        pub siret: IdentifiantDOrganisme,
        pub libelle: String,
        pub nom: String,
        pub objet: String,
        pub jour_de_paiement: Option<u32>,
        pub codes81: Vec<crate::code_cotisation::CleDeTotalisation>,
        pub adresse: Option<String>,
    }
}

impl Versement {
    /// L'Urssaf : l'organisme du bordereau et des CTP, reconnu a son libelle.
    pub fn est_urssaf(&self) -> bool {
        self.libelle == "URSSAF"
    }
}

forme_notee! {
    /// Le bloc S20.G00.08 d'un mois sans individu : le code caisse des organismes qui n'ont plus d'individu rattache.
    pub struct SansIndividu {
        pub absence_de_rattachement: Vec<CodeCaisse>,
    }
}

forme_notee! {
    /// La table des CTP du bordereau Urssaf (bloc S21.G00.23).
    pub struct TableCtp {
        pub lignes: Vec<Ctp>,
    }
}

/// Le taux AT/MP d'une ligne du bordereau : celui du bareme du mois declare.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TauxAt {
    Bareme,
}

/// UNE RUBRIQUE DE NATURE `X [min,max]` du cahier technique DSN : de `min` a `max` caracteres (comptes en caracteres,
/// non en octets), sans blanc au debut ni a la fin ; l'espace et les lettres accentuees de la table 3.5 sont des
/// caracteres permis (`dsn-cahier-technique-2026.1-formats.txt` l. 6630-6633, 7432-7440). Rien de plus (regle 7).
pub(crate) const fn de_nature_x(texte: &str, min: usize, max: usize) -> bool {
    let o = texte.as_bytes();
    if o.is_empty() || o[0] == b' ' || o[o.len() - 1] == b' ' {
        return false;
    }
    let mut caracteres = 0;
    let mut i = 0;
    while i < o.len() {
        if o[i] & 0xC0 != 0x80 {
            caracteres += 1;
        }
        i += 1;
    }
    caracteres >= min && caracteres <= max
}

/// LE CODE TYPE DE PERSONNEL D'UNE LIGNE DU BORDEREAU URSSAF (S21.G00.23.001) : « X [3,3] Table des Codes Type de
/// Personnel » (cahier technique DSN 2026.1 p. 168, `atelier/reference/norme-dsn/texte/bloc-S21.G00.23.txt` l. 23-27).
/// La table est celle de l'Urssaf (`donnees/urssaf-codes-types-personnel.csv`), datee : qu'un code y soit au jour du
/// bordereau, `documents::bordereau` le juge. La forme est celle du cahier, trois caracteres (`X`), et rien de plus :
/// que la table n'ait que des chiffres ne fonde pas un controle (regle 7).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct CodeCtp(std::borrow::Cow<'static, str>);

impl CodeCtp {
    /// Le code ecrit `texte`.
    pub fn lire(texte: &str) -> R<CodeCtp> {
        if !CodeCtp::a_sa_forme(texte) {
            return Err(socle::refus!("CTP {} : un code type de personnel s'écrit de trois caractères.", socle::fichiers::repr(texte)));
        }
        Ok(CodeCtp(std::borrow::Cow::Owned(texte.to_string())))
    }

    const fn a_sa_forme(texte: &str) -> bool {
        de_nature_x(texte, 3, 3)
    }

    /// LE CODE QUE LE MOTEUR NOMME, ecrit en Rust : une constante, dont le compilateur refuse une forme fausse.
    pub const fn du_moteur(code: &'static str) -> CodeCtp {
        assert!(CodeCtp::a_sa_forme(code), "code type de personnel mal forme");
        CodeCtp(std::borrow::Cow::Borrowed(code))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CodeCtp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CodeCtp {
    type Error = socle::echec::Echec;
    fn try_from(s: String) -> R<CodeCtp> {
        CodeCtp::lire(&s)
    }
}

impl From<CodeCtp> for String {
    fn from(c: CodeCtp) -> String {
        c.0.into_owned()
    }
}

/// LE FORMAT D'UN CTP dans la table de l'Urssaf (colonne « Format » de `donnees/urssaf-codes-types-personnel.csv`) :
/// il dit si la ligne 23 porte une assiette (S21.G00.23.004) ou un montant (23.005), et comment elle se valorise. Le
/// guide Urssaf de la regularisation, § 1.3 (`atelier/reference/sources/editeur-dsn/guide-urssaf-regularisation.txt`,
/// l. 351-407) : « A, D et K : Taux AT à renseigner » ; « E : Pas de taux AT à renseigner » ; F, « Format utilisé pour
/// déclarer une déduction de cotisation », et R, « Format propre à la déclaration d'une réduction de cotisations sur
/// heures supplémentaires », « ont la particularité de porter un signe négatif » ; « Y : Ce format s'applique uniquement
/// pour le CTP 901 » ; « Z : [...] notamment pour les lignes mobilité (hors VMRR) » ; « V : Taux à renseigner hors taux
/// AT ». B et C sont dans la table, que le tableau du guide ne decrit pas (version 5.2 non plus,
/// `editeur-dsn/guide-urssaf-regularisation-v5.2-20261019.txt`, l. 345-386) ; le meme guide dit ou leur montant se
/// porte (l. 387-394) : « le montant associé au code type devra systématiquement être porté dans la rubrique 23.004
/// (montant d'assiette), sauf si le CTP [...] est de format "R" ou "F" » : une assiette, comme A. La liste est celle de
/// la table livree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatDeCtp {
    A,
    B,
    C,
    D,
    E,
    F,
    K,
    R,
    V,
    Y,
    Z,
}

impl FormatDeCtp {
    /// Le format ecrit `texte` dans la table, ou la phrase qui le refuse.
    pub fn lire(texte: &str) -> R<FormatDeCtp> {
        Ok(match texte {
            "A" => FormatDeCtp::A,
            "B" => FormatDeCtp::B,
            "C" => FormatDeCtp::C,
            "D" => FormatDeCtp::D,
            "E" => FormatDeCtp::E,
            "F" => FormatDeCtp::F,
            "K" => FormatDeCtp::K,
            "R" => FormatDeCtp::R,
            "V" => FormatDeCtp::V,
            "Y" => FormatDeCtp::Y,
            "Z" => FormatDeCtp::Z,
            _ => return Err(socle::refus!("Format de CTP {} inconnu : la table de l'Urssaf écrit A, B, C, D, E, F, K, R, V, Y ou Z.", socle::fichiers::repr(texte))),
        })
    }

    /// Son nom dans la table.
    pub fn as_str(self) -> &'static str {
        match self {
            FormatDeCtp::A => "A",
            FormatDeCtp::B => "B",
            FormatDeCtp::C => "C",
            FormatDeCtp::D => "D",
            FormatDeCtp::E => "E",
            FormatDeCtp::F => "F",
            FormatDeCtp::K => "K",
            FormatDeCtp::R => "R",
            FormatDeCtp::V => "V",
            FormatDeCtp::Y => "Y",
            FormatDeCtp::Z => "Z",
        }
    }
}

impl std::fmt::Display for FormatDeCtp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// L'IDENTIFIANT D'UN ORGANISME DE PROTECTION SOCIALE : celui que la DSN paie (S21.G00.20.001), celui d'une cotisation
/// individuelle (S21.G00.81.002), le code de l'organisme d'une adhesion de prevoyance (S21.G00.15.002). Le cahier
/// technique DSN 2026.1 nomme les deux premiers « Identifiant Organisme de Protection Sociale », « X [2,14] » (p. 154-155,
/// `atelier/reference/norme-dsn/texte/bloc-S21.G00.20.txt` l. 25-80 ; p. 306-307, `bloc-S21.G00.81.txt` l. 359-404) :
/// le SIRET de l'Urssaf, de France Travail, de la caisse Congés Spectacles, ou pour un organisme complementaire « code de
/// l'institution de prévoyance, de la mutuelle, de la société d'assurance » (tables PREV, MUT, ASSU, OCI), c'est-a-dire
/// le code de 15.002 (« X [5,9] », p. 151, `bloc-S21.G00.15.txt` l. 45-55), que CCH-13 de 20.001 compare a lui. Une
/// seule notion ; la longueur propre a 15.002 se juge la ou l'adhesion se compose (`composer`). Les tables IVO, ICI,
/// PREV ne sont pas deposees : la forme est celle du cahier, de 2 a 14 caracteres (`X`), et rien de plus (regle 7).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct IdentifiantDOrganisme(std::borrow::Cow<'static, str>);

impl IdentifiantDOrganisme {
    /// L'identifiant ecrit `texte`.
    pub fn lire(texte: &str) -> R<IdentifiantDOrganisme> {
        if !IdentifiantDOrganisme::a_sa_forme(texte) {
            return Err(socle::refus!(
                "Organisme {} : l'identifiant d'un organisme de protection sociale s'écrit de 2 à 14 caractères, sans blanc au début ni à la fin.",
                socle::fichiers::repr(texte)
            ));
        }
        Ok(IdentifiantDOrganisme(std::borrow::Cow::Owned(texte.to_string())))
    }

    const fn a_sa_forme(texte: &str) -> bool {
        de_nature_x(texte, 2, 14)
    }

    /// L'IDENTIFIANT QUE LE MOTEUR NOMME, ecrit en Rust : une constante, dont le compilateur refuse une forme fausse.
    pub const fn du_moteur(code: &'static str) -> IdentifiantDOrganisme {
        assert!(IdentifiantDOrganisme::a_sa_forme(code), "identifiant d'organisme mal forme");
        IdentifiantDOrganisme(std::borrow::Cow::Borrowed(code))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for IdentifiantDOrganisme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for IdentifiantDOrganisme {
    type Error = socle::echec::Echec;
    fn try_from(s: String) -> R<IdentifiantDOrganisme> {
        IdentifiantDOrganisme::lire(&s)
    }
}

impl From<IdentifiantDOrganisme> for String {
    fn from(c: IdentifiantDOrganisme) -> String {
        c.0.into_owned()
    }
}

/// LE CODE CAISSE D'UN ORGANISME QUI N'A PLUS D'INDIVIDU RATTACHE (S20.G00.08.001, « Code caisse ») : « Numéro de la
/// caisse destinataire de la DSN sans salarié affilié » (Agirc-Arrco 88, France Travail 93, Congés Spectacles 97…),
/// « Table ODRT », « X [2,6] » (cahier technique DSN 2026.1 p. 140, `atelier/reference/norme-dsn/texte/bloc-S20.G00.08.txt`).
/// Ce n'est pas l'identifiant de l'organisme (`IdentifiantDOrganisme`). La table ODRT n'est pas deposee : la forme est
/// celle du cahier, de 2 a 6 caracteres (`X`), et rien de plus (regle 7).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct CodeCaisse(String);

impl CodeCaisse {
    /// Le code ecrit `texte`.
    pub fn lire(texte: &str) -> R<CodeCaisse> {
        if !de_nature_x(texte, 2, 6) {
            return Err(socle::refus!(
                "Code caisse {} : le code d'une caisse sans individu rattaché (S20.G00.08.001) s'écrit de 2 à 6 caractères, sans blanc au début ni à la fin.",
                socle::fichiers::repr(texte)
            ));
        }
        Ok(CodeCaisse(texte.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CodeCaisse {
    type Error = socle::echec::Echec;
    fn try_from(s: String) -> R<CodeCaisse> {
        CodeCaisse::lire(&s)
    }
}

impl From<CodeCaisse> for String {
    fn from(c: CodeCaisse) -> String {
        c.0
    }
}

crate::natures::codes_fermes! {
    /// LE QUALIFIANT D'ASSIETTE D'UNE LIGNE DU BORDEREAU (S21.G00.23.002), la liste entiere de la norme (cahier
    /// technique DSN 2026.1, `atelier/reference/norme-dsn/texte/bloc-S21.G00.23.txt` l. 33-36 ; relevee dans
    /// `formats-dsn-*-base.json`). Celui de chaque CTP, `organismes-base.json` le dit (`Ctp::qualifiant`).
    QualifiantDAssiette, "Qualifiant d'assiette", "S21.G00.23.002" {
        AutreAssiette = "920",
        AssiettePlafonnee = "921",
    }
}

forme_notee! {
    /// UNE LIGNE DU BORDEREAU URSSAF : le CTP (23.001), son qualifiant d'assiette (23.002), ce qu'il lit (une assiette,
    /// ou un montant), son taux AT/MP, la population qui l'ecrit (`statut`, `nature`) et les seuils d'effectif.
    pub struct Ctp {
        pub code: CodeCtp,
        pub qualifiant: QualifiantDAssiette,
        pub assiette: Option<crate::bordereau::AssietteDuBordereau>,
        pub montant: Option<crate::bordereau::MontantDuBordereau>,
        pub taux_at: Option<TauxAt>,
        pub statut: Option<crate::fiche::Statut>,
        /// La population : `intermittent` ou `permanent` (`ParNature`).
        #[serde(rename = "nature", default, deserialize_with = "crate::natures::population::option::deserialize")]
        pub population: Option<crate::natures::Paie>,
        pub effectif_au_moins: Option<i64>,
        pub effectif_sous: Option<i64>,
    }
}

forme_notee! {
    /// Les contrats que les textes exigent : ceux de toute compagnie, puis ceux de chaque convention (par IDCC).
    pub struct Contrats {
        pub tous: Vec<Contrat>,
        pub conventions: BTreeMap<Idcc, Vec<Contrat>>,
    }
}

crate::cle_opaque! {
    /// LA CLE D'UN CONTRAT D'ORGANISME : celle d'un contrat de `organismes-base.json` (`prevoyance_permanents`,
    /// `tsintermittents`), de la CASC-SVP (`casc`), d'une contribution de branche (`contribution_fnas`). Que portent un
    /// contrat qu'un texte exige, une adhesion saisie (`config/organismes.json`), un contrat attendu, la ligne de l'ecran
    /// et la saisie d'`organismes-enregistrer` (`contrats`).
    pub struct CleDeContrat, "clé d'un contrat d'organisme";
}

impl CleDeContrat {
    /// La cle du contrat d'une contribution de branche : `contribution_` et son nom en minuscules.
    pub fn de_la_contribution(c: &crate::forme_bareme::ContributionDeBranche) -> CleDeContrat {
        CleDeContrat(std::borrow::Cow::Owned(format!("contribution_{}", c.as_str().to_lowercase())))
    }
}

/// La CASC-SVP (`adhesions::casc_svp`).
pub const CASC: CleDeContrat = CleDeContrat::du_moteur("casc");
/// Les contrats des intermittents, dans l'ordre de leur rattachement (S21.G00.70.013).
pub const TSINTERMITTENTS: CleDeContrat = CleDeContrat::du_moteur("tsintermittents");
pub const INTERBRANCHE: CleDeContrat = CleDeContrat::du_moteur("interbranche");
/// Les contrats des permanents (`organismes-base.json`), que le contrat ecrit nomme.
pub const PREVOYANCE_PERMANENTS: CleDeContrat = CleDeContrat::du_moteur("prevoyance_permanents");
pub const SANTE_PERMANENTS: CleDeContrat = CleDeContrat::du_moteur("sante_permanents");

/// LE ROLE D'UN CONTRAT D'ORGANISME pour un salarie paye au mois : sa prevoyance, sa sante (`role` de
/// `organismes-base.json`). Une liste du logiciel ; le rattachement d'un permanent les prend dans cet ordre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoleDuContrat {
    Prevoyance,
    Sante,
}

/// UN CONTRAT QU'UN TEXTE EXIGE (bloc S21.G00.15) : sa cle, son role pour un permanent (`prevoyance`, `sante`), sa
/// reference chez l'organisme quand le texte la donne (15.001), l'identifiant de l'adhesion quand il est fixe (15.005),
/// son type (15.003), ses dates, qui il couvre, l'organisme (15.002) et s'il est designe, s'il n'est exige qu'avec un
/// permanent, et les options que l'organisme demande. `quoi` le nomme a l'ecran, `note` s'y lit sous lui ; `source` le
/// fonde.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contrat {
    pub cle: CleDeContrat,
    pub role: Option<RoleDuContrat>,
    pub libelle: Option<String>,
    pub reference: Option<String>,
    pub r#type: String,
    pub depuis: Option<Jour>,
    pub jusqu_a: Option<Jour>,
    #[serde(deserialize_with = "crate::natures::population::au_pluriel::deserialize")]
    pub beneficiaires: crate::natures::Paie,
    pub quoi: String,
    pub organisme: Option<IdentifiantDOrganisme>,
    pub organisme_fixe: bool,
    pub options: Option<Options>,
    pub note: Option<String>,
    pub source: Option<Fondement>,
}

forme_notee! {
    /// Les codes option (S21.G00.70.004) que l'organisme demande sous ce contrat, par niveau de garantie ; `regime_local` :
    /// ils valent aussi pour le regime local d'Alsace-Moselle.
    pub struct Options {
        pub regime_local: bool,
        pub niveaux: BTreeMap<crate::forme_bareme::NiveauDeGarantie, Niveau>,
    }
}

/// Les codes option d'un niveau : seul (`isole`), ou avec un conjoint ou des enfants (`famille`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Niveau {
    pub isole: String,
    pub famille: String,
}

/// LA LECTURE DE `donnees/organismes-base.json`, la seule, une fois tant que le fichier ne change pas.
pub fn base() -> R<Arc<Base>> {
    static CACHE: OnceLock<FormeGardee<Base>> = OnceLock::new();
    forme_en_cache(&CACHE, "organismes-base.json", "La table livrée des organismes ne se lit pas : le signaler au développeur.", Ok)
}

impl Base {
    /// Tous les contrats que les textes donnent, toutes conventions confondues.
    fn tous_les_contrats(&self) -> impl Iterator<Item = &Contrat> {
        self.contrats.tous.iter().chain(self.contrats.conventions.values().flatten())
    }

    /// Les contrats de toute compagnie, puis ceux de la convention `idcc` quand la fiche la dit.
    pub fn contrats_de(&self, idcc: Option<&Idcc>) -> impl Iterator<Item = &Contrat> {
        self.contrats.tous.iter().chain(idcc.and_then(|i| self.contrats.conventions.get(i)).into_iter().flatten())
    }

    /// Les options que l'organisme demande sous le contrat de reference `libelle` (S21.G00.15.001), pour la convention
    /// `idcc` quand la fiche la dit.
    pub fn options(&self, idcc: Option<&Idcc>, libelle: &str) -> Option<&Options> {
        self.contrats_de(idcc).filter(|c| c.libelle.as_deref().map(str::trim) == Some(libelle)).find_map(|c| c.options.as_ref())
    }

    /// Le versement de l'organisme `siret` (S21.G00.20.001).
    pub fn versement(&self, siret: &IdentifiantDOrganisme) -> Option<&Versement> {
        self.versements.iter().find(|v| v.siret == *siret)
    }
}

// ------------------------------------------------------------------------------------------ ce que la compagnie saisit

/// `config/organismes.json` : ce que la compagnie a saisi (Configuration › Organismes et contrats), date. Ecrit par
/// `compagnie::organismes`, lu par `saisis`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FichierOrganismes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adhesions_prevoyance: Option<Vec<Datee<Vec<Adhesion>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoi: Option<String>,
}

/// UNE ADHESION DE PREVOYANCE OU DE SANTE (bloc S21.G00.15) : le contrat (`cle`), sa reference (15.001), l'organisme
/// (15.002), son type (15.003), son identifiant (15.005), et ses dates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adhesion {
    pub cle: CleDeContrat,
    pub libelle: String,
    pub delegataire: IdentifiantDOrganisme,
    pub r#type: String,
    pub reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contribution: Option<crate::forme_bareme::ContributionDeBranche>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depuis: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jusqu_a: Option<Jour>,
}

impl Adhesion {
    /// L'adhesion vaut-elle pour la periode `debut`..=`fin` (`adhesions::en_vigueur`) ?
    pub fn en_vigueur(&self, debut: &Jour, fin: &Jour) -> bool {
        crate::adhesions::en_vigueur(self.depuis, self.jusqu_a, debut, fin)
    }
}

crate::cle_opaque! {
    /// LA REFERENCE D'UNE ADHESION (S21.G00.15.005, l'« identifiant technique Adhésion », que le bloc 70 d'un salarie
    /// cite en S21.G00.70.013) : celle que le moteur attribue a une adhesion (`Adhesion.reference`) ou qu'une
    /// contribution de branche porte. Les relectures d'une DSN que le moteur ecrit la lisent (`declarations::dsn`).
    pub struct ReferenceDAdhesion, "référence d'adhésion";
}

/// LA REFERENCE D'UN CONTRAT CHEZ L'ORGANISME (S21.G00.15.001, la « référence contrat » de la fiche de parametrage DSN)
/// que la compagnie saisit : de 1 a 30 caracteres (le cahier technique), sans blancs de bord, en capitales.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceDeContrat(String);

impl ReferenceDeContrat {
    /// La reference saisie `texte`, sans ses blancs autour et en capitales ; des blancs seuls ne disent rien (`None`).
    /// Plus de 30 caracteres se refusent.
    pub fn saisie(texte: &str) -> R<Option<ReferenceDeContrat>> {
        let Some(t) = rangee(texte) else { return Ok(None) };
        if t.chars().count() > 30 {
            return Err(socle::refus!("La référence du contrat « {} » dépasse 30 caractères, ce que la DSN n'admet pas.", t));
        }
        Ok(Some(ReferenceDeContrat(t)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Une case saisie d'un contrat, sans ses blancs autour et en capitales ; des blancs seuls ne disent rien.
fn rangee(texte: &str) -> Option<String> {
    Some(texte.trim()).filter(|t| !t.is_empty()).map(str::to_uppercase)
}

/// Ce que l'ecran saisit d'un contrat (`organismes-enregistrer`, `contrats` : cle -> saisie) : sa reference chez
/// l'organisme et le code de l'organisme, chacun lu dans son type a la porte (`Saisie::lue`). Une case vide est absente
/// ou nulle ; `""` se refuse a la porte (`TexteDit`) ; des blancs seuls ne disent rien.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, try_from = "SaisieEcrite")]
pub struct Saisie {
    pub libelle: Option<ReferenceDeContrat>,
    pub organisme: Option<IdentifiantDOrganisme>,
}

/// La saisie d'un contrat telle que la charge l'ecrit.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaisieEcrite {
    libelle: Option<socle::forme::TexteDit>,
    organisme: Option<socle::forme::TexteDit>,
}

impl TryFrom<SaisieEcrite> for Saisie {
    type Error = socle::echec::Echec;
    fn try_from(s: SaisieEcrite) -> R<Saisie> {
        Saisie::lue(s.libelle.as_deref(), s.organisme.as_deref())
    }
}

impl Saisie {
    /// LA SAISIE D'UN CONTRAT, sa seule porte : la reference (`ReferenceDeContrat::saisie`) et le code de l'organisme,
    /// sans blancs autour et en capitales (`IdentifiantDOrganisme::lire`) ; des blancs seuls ne disent rien.
    pub fn lue(libelle: Option<&str>, organisme: Option<&str>) -> R<Saisie> {
        let libelle = match libelle {
            Some(l) => ReferenceDeContrat::saisie(l)?,
            None => None,
        };
        let organisme = match organisme.and_then(rangee) {
            Some(o) => Some(IdentifiantDOrganisme::lire(&o)?),
            None => None,
        };
        Ok(Saisie { libelle, organisme })
    }
}

/// LA LECTURE DE `config/organismes.json`, la seule ; absent, il ne dit rien.
pub fn fichier_organismes(ctx: &Contexte) -> R<FichierOrganismes> {
    let p = ctx.racine.join(socle::fichiers::CONFIG).join(ORGANISMES);
    if !p.is_file() {
        return Ok(FichierOrganismes::default());
    }
    crate::fiche::lire_fichier(&p, &format!("config/{}", ORGANISMES))
}

/// Le numero d'une reference d'adhesion (S21.G00.15.001) : une reference qui n'en est pas un se refuse.
fn numero_de_reference(reference: &str) -> R<u32> {
    reference.trim().parse::<u32>().map_err(|_| {
        socle::refus!("La référence d'adhésion « {} » (S21.G00.15.001) n'est pas un nombre : la corriger dans les organismes de la compagnie.", reference)
    })
}

/// Ce que la compagnie a saisi, a la date du contexte (rien s'il n'y a rien), et les identifiants d'adhesion qu'une
/// periode quelconque du fichier porte. Le fichier ne porte que ce qu'aucun texte ne donne : de lui ne se lisent que les
/// adhesions, et une adhesion qui recopie un contrat qu'un texte donne entier (`donnes_par_un_texte`) n'est pas une
/// saisie, sous quelque convention qu'on lise. Son identifiant reste pris : il a pu partir dans une DSN.
pub fn saisis(ctx: &Contexte) -> R<(Vec<Adhesion>, Vec<u32>)> {
    let fichier = fichier_organismes(ctx)?;
    let Some(liste) = fichier.adhesions_prevoyance.as_deref() else { return Ok((Vec::new(), Vec::new())) };
    let prises = liste
        .iter()
        .flat_map(|e| e.valeur.dite().into_iter().flatten())
        .map(|a| numero_de_reference(&a.reference))
        .collect::<R<Vec<u32>>>()?;
    let au = crate::fiche::valeur_au(liste,&ctx.lue_au);
    let textes = donnes_par_un_texte()?;
    let adhesions: Vec<Adhesion> = au
        .dite()
        .into_iter()
        .flatten()
        .filter(|a| {
            let libelle = a.libelle.trim().to_uppercase();
            !textes.contains(&libelle) && !a.contribution.as_ref().is_some_and(|c| c.est_le_libelle(&libelle))
        })
        .cloned()
        .collect();
    Ok((adhesions, prises))
}

/// Les references de contrat (S21.G00.15.001) qu'un texte livre donne entieres, toutes conventions confondues : les
/// `libelle` des contrats de `organismes-base.json` et celui de la CASC-SVP (`casc-svp-base.json`). Une contribution de
/// branche se reconnait a son nom (`saisis`).
fn donnes_par_un_texte() -> R<Vec<String>> {
    let base = base()?;
    let mut out: Vec<String> =
        base.tous_les_contrats().filter_map(|c| c.libelle.as_deref()).map(|l| l.trim().to_uppercase()).filter(|l| !l.is_empty()).collect();
    let l = crate::adhesions::casc_svp_base()?.dsn.contrat.trim().to_uppercase();
    if !l.is_empty() {
        out.push(l);
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------------ les contrats attendus

/// UN CONTRAT ATTENDU d'une compagnie a une date : un contrat qu'un texte exige (`Contrat`), la CASC-SVP, ou une
/// contribution de branche du bareme. `libelle_defaut` : la reference que l'ecran propose quand rien n'est saisi.
#[derive(Debug, Clone)]
pub struct Attendu {
    pub cle: CleDeContrat,
    pub role: Option<RoleDuContrat>,
    pub libelle: Option<String>,
    pub libelle_defaut: Option<String>,
    pub reference: Option<String>,
    pub r#type: String,
    pub depuis: Option<Jour>,
    pub jusqu_a: Option<Jour>,
    pub beneficiaires: String,
    pub quoi: String,
    pub organisme: Option<IdentifiantDOrganisme>,
    pub organisme_fixe: bool,
    pub contribution: Option<crate::forme_bareme::ContributionDeBranche>,
    pub note: Option<String>,
}

impl From<&Contrat> for Attendu {
    fn from(c: &Contrat) -> Attendu {
        Attendu {
            cle: c.cle.clone(),
            role: c.role,
            libelle: c.libelle.clone(),
            reference: c.reference.clone(),
            r#type: c.r#type.clone(),
            depuis: c.depuis,
            jusqu_a: c.jusqu_a,
            beneficiaires: c.beneficiaires.au_pluriel().to_string(),
            quoi: c.quoi.clone(),
            organisme: c.organisme.clone(),
            organisme_fixe: c.organisme_fixe,
            note: c.note.clone(),
            libelle_defaut: None,
            contribution: None,
        }
    }
}

/// Le texte d'un champ facultatif, sans ses blancs ; `None` s'il manque ou n'ecrit que des blancs.
fn net(s: &Option<String>) -> Option<String> {
    s.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_string)
}

impl Attendu {
    /// La reference donnee par le texte, sans ses blancs ; `None` s'il n'en donne pas.
    pub fn libelle_donne(&self) -> Option<String> {
        net(&self.libelle)
    }

    /// SA PIECE : un contrat qui a un role (`prevoyance`, `sante`) est une affiliation d'un salarie paye au mois, que
    /// seul son calcul rattache (`par_nature.permanent`, dans l'ordre de ses roles) : la piece d'un permanent. Son role la
    /// dit ; aucune etiquette.
    pub fn piece(&self) -> Option<Piece> {
        self.role.map(|_| Piece::Permanent)
    }

    /// Le contrat vaut-il pour la periode (`adhesions::en_vigueur`) ?
    pub fn couvre(&self, (debut, fin): &(Jour, Jour)) -> bool {
        crate::adhesions::en_vigueur(self.depuis, self.jusqu_a, debut, fin)
    }
}

/// Les contrats que cette compagnie declare a la date du contexte, dans l'ordre ou ils se presentent.
pub fn attendus(ctx: &Contexte) -> R<Vec<Attendu>> {
    let idcc = ctx.entreprise.idcc.dite();
    let base = base()?;
    let mut out: Vec<Attendu> = base.contrats.tous.iter().map(Attendu::from).collect();
    // L'exemption de la CASC-SVP se lit avec l'effectif de l'annee du contexte (le mois que l'on compose), pas du jour.
    if let Some(casc) = crate::adhesions::casc_svp(&ctx.entreprise, &ctx.effectif()?, ctx.lue_au.an)? {
        out.push(Attendu {
            cle: CASC,
            libelle: Some(casc.libelle),
            reference: Some(casc.reference),
            r#type: casc.r#type,
            beneficiaires: "tous les salariés".into(),
            quoi: "CASC-SVP (activités sociales et culturelles)".into(),
            organisme: Some(casc.delegataire),
            organisme_fixe: true,
            role: None,
            libelle_defaut: None,
            depuis: None,
            jusqu_a: None,
            contribution: None,
            note: None,
        });
    }
    // Les contributions de branche en DSN : la convention de la date dit lesquelles (`Contexte::contributions_dsn`).
    for c in ctx.contributions_dsn(&ctx.lue_au)? {
        let nom = c.contribution.clone();
        out.push(Attendu {
            cle: CleDeContrat::de_la_contribution(&nom),
            contribution: Some(nom.clone()),
            libelle_defaut: Some(nom.to_string()),
            r#type: "01".into(),
            beneficiaires: "selon le contrat".into(),
            quoi: format!("Contribution {} (en DSN depuis {})", nom, c.en_dsn_depuis),
            organisme: Some(AUDIENS),
            organisme_fixe: true,
            role: None,
            libelle: None,
            reference: None,
            depuis: None,
            jusqu_a: None,
            note: None,
        });
    }
    out.extend(idcc.and_then(|i| base.contrats.conventions.get(i)).into_iter().flatten().map(Attendu::from));
    Ok(out)
}

/// L'adhesion saisie qui tient ce contrat : par sa cle, son identifiant fixe, ou sa contribution.
fn en_place<'a>(spec: &Attendu, saisi: &'a [Adhesion]) -> Option<&'a Adhesion> {
    if let Some(a) = saisi.iter().find(|a| a.cle == spec.cle) {
        return Some(a);
    }
    if let Some(reference) = net(&spec.reference) {
        return saisi.iter().find(|a| a.reference.trim() == reference);
    }
    let contribution = spec.contribution.as_ref()?;
    saisi.iter().find(|a| a.contribution.as_ref().is_some_and(|c| c.est(contribution)) || contribution.est_le_libelle(&a.libelle))
}

/// UNE LIGNE DE L'ECRAN (`organismes-contrats`) : ce que le contrat couvre, et ce qui vaut (le texte quand il le donne,
/// sinon la saisie).
#[derive(Debug, Clone, Serialize)]
pub struct Ligne {
    pub cle: CleDeContrat,
    pub quoi: String,
    pub beneficiaires: String,
    /// `None` : ni le texte ni la compagnie ne la disent encore.
    pub libelle: Option<String>,
    pub libelle_fixe: bool,
    /// `None` : ni le texte ni la compagnie ne le disent encore.
    pub organisme: Option<IdentifiantDOrganisme>,
    pub organisme_fixe: bool,
    /// Sa piece (`Attendu::piece`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub piece: Option<Piece>,
    pub depuis: Option<socle::date::Jour>,
    pub jusqu_a: Option<socle::date::Jour>,
    pub note: Option<String>,
}

/// La ligne de l'ecran d'un contrat attendu, sous ce que la compagnie a saisi : ce que le texte donne, sinon ce qu'elle
/// a saisi, sinon ce que le contrat propose (`libelle_defaut`, l'organisme de la contribution).
pub fn ligne(spec: &Attendu, saisi: &[Adhesion]) -> Ligne {
    let place = en_place(spec, saisi);
    let saisie = |t: &str| Some(t.trim().to_string()).filter(|s| !s.is_empty());
    let libelle_fixe = spec.libelle_donne().is_some();
    let libelle = match spec.libelle_donne() {
        Some(donne) => Some(donne),
        None => place.and_then(|a| saisie(&a.libelle)).or_else(|| net(&spec.libelle_defaut)),
    };
    let organisme = if spec.organisme_fixe {
        spec.organisme.clone()
    } else {
        place.map(|a| a.delegataire.clone()).or_else(|| spec.organisme.clone())
    };
    Ligne {
        cle: spec.cle.clone(),
        quoi: spec.quoi.trim().to_string(),
        beneficiaires: spec.beneficiaires.trim().to_string(),
        libelle,
        libelle_fixe,
        organisme,
        organisme_fixe: spec.organisme_fixe,
        piece: spec.piece(),
        depuis: spec.depuis,
        jusqu_a: spec.jusqu_a,
        note: net(&spec.note),
    }
}

// ------------------------------------------------------------------------------------------ la composition

/// Ce qu'une nature de contrat rattache (S21.G00.70.013) : les identifiants de ses adhesions, dans l'ordre que le
/// socle attend (pour un permanent, la prevoyance puis la sante), ou pourquoi elles manquent.
#[derive(Debug, Clone, PartialEq)]
pub struct Rattachement {
    pub references: Vec<String>,
    pub a_confirmer: Option<&'static str>,
}

/// Les rattachements de chaque nature : les intermittents, les permanents.
#[derive(Debug, Clone, PartialEq)]
pub struct ParNature {
    pub intermittent: Rattachement,
    pub permanent: Rattachement,
}

impl ParNature {
    /// Les populations, dans l'ordre : l'intermittent (a l'activite), le permanent (au mois).
    pub fn natures(&self) -> [(crate::natures::Paie, &Rattachement); 2] {
        [(crate::natures::Paie::Activite, &self.intermittent), (crate::natures::Paie::Mensuelle, &self.permanent)]
    }

    /// Le rattachement de la section `section` (`Nature::section_des_organismes`).
    pub fn de(&self, section: crate::natures::Paie) -> &Rattachement {
        match section {
            crate::natures::Paie::Activite => &self.intermittent,
            crate::natures::Paie::Mensuelle => &self.permanent,
        }
    }
}

/// Pourquoi les permanents n'ont pas de rattachement.
const PERMANENTS_A_SAISIR: &str = "La prévoyance et la santé des permanents ne sont pas toutes deux saisies (Configuration › \
Organismes et contrats) : un permanent est refusé tant qu'elles manquent.";

/// LES ORGANISMES COMPOSES d'une compagnie a une date : la partie normative (`base`), ses adhesions (bloc S21.G00.15,
/// dans l'ordre de leurs identifiants) et ce que chaque nature rattache.
#[derive(Debug, Clone)]
pub struct Organismes {
    pub base: Arc<Base>,
    pub adhesions_prevoyance: Vec<Adhesion>,
    pub par_nature: ParNature,
}

impl Organismes {
    /// Les organismes que la DSN paie (bloc S21.G00.20).
    pub fn versements(&self) -> &[Versement] {
        &self.base.versements
    }

    /// L'Urssaf, l'organisme du bordereau, de la DOETH et de la CSA (S21.G00.82.005) : la table livrée des versements la
    /// dit, sinon rien ne se déclare.
    pub fn urssaf(&self) -> R<&Versement> {
        self.base.versements.iter().find(|v| v.est_urssaf()).ok_or_else(|| {
            socle::echec::cle(
                "La table livrée des versements (organismes-base.json) n'a pas l'Urssaf (un versement de libellé URSSAF) : la DSN ne déclare sans elle ni son bordereau (S21.G00.22), ni la DOETH, ni la CSA (S21.G00.82.005). Rien n'est écrit : le signaler au développeur.",
                "versements",
            )
        })
    }

    /// La table des CTP du bordereau.
    pub fn ctp_urssaf(&self) -> &[Ctp] {
        &self.base.ctp_urssaf.lignes
    }

    /// Les organismes du bloc S20.G00.08 d'un mois sans individu.
    pub fn absence_de_rattachement(&self) -> &[CodeCaisse] {
        &self.base.sans_individu.absence_de_rattachement
    }

    /// L'adhesion d'identifiant `reference` (S21.G00.15.005).
    pub fn adhesion(&self, reference: &str) -> Option<&Adhesion> {
        self.adhesions_prevoyance.iter().find(|a| a.reference == reference)
    }

    /// L'adhesion `reference` vaut-elle pour la periode `debut`..=`fin` ? Une reference qu'aucune adhesion ne porte (une
    /// contribution de branche du bareme) n'a pas de date.
    pub fn adhesion_en_vigueur(&self, reference: &str, debut: &Jour, fin: &Jour) -> bool {
        self.adhesion(reference).is_none_or(|a| a.en_vigueur(debut, fin))
    }
}

/// Compose les organismes a la date du contexte depuis la base, les contrats attendus, ce que la compagnie a saisi
/// (`saisi` : le fichier a cette date ; `prises` : les identifiants que le fichier porte) et ce qu'elle saisit
/// maintenant (`saisies` : cle -> saisie). Chaque contrat garde ses dates (`depuis`, `jusqu_a`) : la DSN de chaque mois
/// ne lit que ceux qui le couvrent.
///
/// `exiger` : la periode que l'ecran enregistre, et si les contrats des permanents s'y montrent
/// (`engagement::permanents::Permanents::ouverts`) ; un contrat qu'il propose pour elle et dont la reference manque se
/// refuse. Sans elle (la lecture), un contrat sans reference ne se declare pas, et ce qui en depend se refuse plus loin.
pub fn composer(
    ctx: &Contexte,
    saisi: &[Adhesion],
    mut prises: Vec<u32>,
    saisies: &BTreeMap<CleDeContrat, Saisie>,
    exiger: Option<(&(Jour, Jour), bool)>,
) -> R<Organismes> {
    // Chaque contrat retenu, avec sa reference quand le texte ou la saisie la disent.
    let mut retenus: Vec<(Attendu, Option<String>, String, IdentifiantDOrganisme)> = Vec::new();
    for spec in attendus(ctx)? {
        let l = ligne(&spec, saisi);
        let maintenant = saisies.get(&spec.cle);
        let libelle = match maintenant {
            Some(s) if !l.libelle_fixe => s.libelle.as_ref().map(|r| r.as_str().to_string()),
            _ => l.libelle.clone(),
        };
        let organisme = match maintenant {
            Some(s) if !l.organisme_fixe => s.organisme.clone(),
            _ => l.organisme.clone(),
        };
        let quoi = spec.quoi.trim().to_string();
        let Some(libelle) = libelle else {
            // Un contrat vide ne se reclame pas quand l'ecran ne l'a pas propose : un contrat des permanents a une
            // compagnie qui n'en emploie pas, un contrat que la periode regardee ne couvre pas.
            let Some((periode, permanents)) = exiger else { continue };
            if (spec.piece() == Some(Piece::Permanent) && !permanents) || !spec.couvre(periode) {
                continue;
            }
            return Err(socle::refus!(
                "{} : la référence du contrat manque. Elle se lit sur la fiche de paramétrage DSN de l'organisme \
(« référence contrat »).",
                quoi
            ));
        };
        // Les longueurs du cahier technique : S21.G00.15.001 de 1 a 30, S21.G00.15.002 de 5 a 9.
        if libelle.chars().count() > 30 {
            return Err(socle::refus!("{} : « {} » dépasse 30 caractères, ce que la DSN n'admet pas.", quoi, libelle));
        }
        let organisme = match organisme {
            Some(o) if (5..=9).contains(&o.as_str().chars().count()) && !o.as_str().contains(char::is_whitespace) => o,
            autre => {
                let dit = match autre {
                    Some(o) => format!("« {} »", o),
                    None => "vide".to_string(),
                };
                return Err(socle::refus!(
                    "{} : le code de l'organisme ({}) a de 5 à 9 caractères sans espace, comme P0983 pour Audiens. Il est \
sur le certificat d'adhésion et la fiche de paramétrage DSN.",
                    quoi, dit
                ));
            }
        };
        let reference = match net(&spec.reference) {
            Some(r) => Some(r),
            None => en_place(&spec, saisi).map(|a| a.reference.trim().to_string()).filter(|r| !r.is_empty()),
        };
        if let Some(r) = &reference {
            prises.push(numero_de_reference(r)?);
        }
        retenus.push((spec, reference, libelle, organisme));
    }
    // Les identifiants nouveaux, a la suite des pris : ceux deja attribues ne bougent pas.
    let mut libre = PREMIER_LIBRE;
    let retenus: Vec<(Attendu, Adhesion)> = retenus
        .into_iter()
        .map(|(spec, reference, libelle, organisme)| {
            let reference = match reference {
                Some(r) => r,
                None => {
                    while prises.contains(&libre) {
                        libre += 1;
                    }
                    prises.push(libre);
                    libre.to_string()
                }
            };
            let a = Adhesion {
                cle: spec.cle.clone(),
                libelle,
                delegataire: organisme,
                r#type: spec.r#type.trim().to_string(),
                reference,
                contribution: spec.contribution.clone(),
                depuis: spec.depuis,
                jusqu_a: spec.jusqu_a,
            };
            (spec, a)
        })
        .collect();
    let role = |r: RoleDuContrat| retenus.iter().find(|(s, _)| s.role == Some(r)).map(|(_, a)| a.reference.clone());
    let permanent = match (role(RoleDuContrat::Prevoyance), role(RoleDuContrat::Sante)) {
        (Some(p), Some(s)) => Rattachement { references: vec![p, s], a_confirmer: None },
        _ => Rattachement { references: Vec::new(), a_confirmer: Some(PERMANENTS_A_SAISIR) },
    };
    let intermittent: Vec<String> = [TSINTERMITTENTS, INTERBRANCHE]
        .iter()
        .filter_map(|c| retenus.iter().find(|(s, _)| s.cle == *c).map(|(_, a)| a.reference.clone()))
        .collect();
    let mut adhesions = retenus
        .into_iter()
        .map(|(_, a)| Ok((numero_de_reference(&a.reference)?, a)))
        .collect::<R<Vec<(u32, Adhesion)>>>()?;
    adhesions.sort_by_key(|(n, _)| *n);
    let adhesions: Vec<Adhesion> = adhesions.into_iter().map(|(_, a)| a).collect();
    Ok(Organismes {
        base: base()?,
        adhesions_prevoyance: adhesions,
        par_nature: ParNature { intermittent: Rattachement { references: intermittent, a_confirmer: None }, permanent },
    })
}
