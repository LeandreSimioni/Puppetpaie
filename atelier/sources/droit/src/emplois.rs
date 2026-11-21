//! Les metiers du spectacle et la fiche telle qu'elle vaut pour un engagement (`emploi_de`, `emploi_du_mois`).
//!
//! Deux fichiers livres avec le code, chacun lu par sa forme, une fois tant qu'il ne change pas : `emplois-base.json`
//! (le catalogue, `BaseEmplois`) et `nomenclature-emplois-ft.json` (les codes France Travail, `Nomenclature`). Un
//! metier se lit par `metier`, qui rend un `Metier` : celui du catalogue, sinon celui de la nomenclature.
//!
//! De l'interface (l'accueil, `routes::ouverture`) : `catalogue`, `familles`,
//! `noms_des_conventions` et `controler_fiche`.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use socle::fichiers::{forme_en_cache, repr, CheminRelatif, FormeGardee};

/// Le catalogue livre des emplois, sous `donnees/`.
pub const BASE: CheminRelatif = CheminRelatif::du_moteur("emplois-base.json");

/// La nomenclature des emplois de France Travail, livree sous `donnees/`.
const NOMENCLATURE: CheminRelatif = CheminRelatif::du_moteur("nomenclature-emplois-ft.json");
use socle::echec::R;
use socle::date::{Jour, Periode};

use crate::contexte::{Contexte, IdSalarie};
use crate::fiche::{AnnexeConvention, CaseDeFiche, CaseSalarie, CodeEmploiFt, CodePcsEse, FicheSalarie, Idcc, Lue, Statut, StatutCategorielRc, StatutConventionnel};
use crate::minima::CleDeGrille;

/// `emplois-base.json` : le catalogue des metiers, dans l'ordre ou le deroulant les montre, et les noms courts des
/// conventions. Sa `note` dit le perimetre, les sources et les regles de chaque champ.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseEmplois {
    pub quoi: String,
    pub note: String,
    /// IDCC -> le nom court de la convention.
    pub conventions: BTreeMap<Idcc, String>,
    #[serde(deserialize_with = "socle::forme::en_ordre")]
    pub emplois: Vec<(CodeEmploi, FicheDuCatalogue)>,
}

/// L'ANNEXE DU REGLEMENT D'ASSURANCE CHOMAGE dont releve un intermittent : la VIII (les ouvriers et techniciens), la X
/// (les artistes) (`atelier/reference/sources/plafond-intermittents/unedic-annexe8-convention-20241115.txt`,
/// `…annexe10…`, art. 1er). Une liste du texte ; trois ecritures, dites par la forme qui la porte : son nom (`X`,
/// `VIII` : une rubrique de `rubriques-base.json`, une source), son numero (`8`, `10` : `emplois-base.json`, la
/// nomenclature de France Travail, ce que la page lit d'un metier : `en_nombre`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnnexeChomage {
    #[serde(rename = "VIII")]
    Viii,
    #[serde(rename = "X")]
    X,
}

impl AnnexeChomage {
    /// Les deux annexes, dans l'ordre ou un conseil les cite : X (artistes), VIII (techniciens).
    pub const LES_DEUX: [AnnexeChomage; 2] = [AnnexeChomage::X, AnnexeChomage::Viii];

    /// Son nom, tel qu'une rubrique ou une source le porte : `X`, `VIII`.
    pub fn nom(self) -> &'static str {
        match self {
            AnnexeChomage::Viii => "VIII",
            AnnexeChomage::X => "X",
        }
    }

    /// Son numero, tel que le catalogue des metiers et la nomenclature l'ecrivent : 8, 10.
    pub fn numero(self) -> u8 {
        match self {
            AnnexeChomage::Viii => 8,
            AnnexeChomage::X => 10,
        }
    }

    /// L'annexe de numero `n`, s'il en est un.
    fn du_numero(n: u8) -> Option<AnnexeChomage> {
        match n {
            8 => Some(AnnexeChomage::Viii),
            10 => Some(AnnexeChomage::X),
            _ => None,
        }
    }
}

/// L'ANNEXE ECRITE PAR SON NUMERO (`#[serde(with = "droit::emplois::annexe_en_nombre")]`) : `emplois-base.json`, la
/// nomenclature, un metier rendu ; `option` pour un `Option<AnnexeChomage>`. Un autre numero se refuse a la lecture.
pub mod annexe_en_nombre {
    use super::AnnexeChomage;

    pub fn serialize<S: serde::Serializer>(a: &AnnexeChomage, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(a.numero())
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<AnnexeChomage, D::Error> {
        lue(<u8 as serde::Deserialize>::deserialize(d)?)
    }

    /// L'annexe de numero `n` ; un autre numero se refuse en le nommant.
    fn lue<E: serde::de::Error>(n: u8) -> Result<AnnexeChomage, E> {
        match AnnexeChomage::du_numero(n) {
            Some(a) => Ok(a),
            None => Err(E::custom(format!("annexe {} inconnue : l'assurance chômage n'en a que deux, la 8 (technicien) et la 10 (artiste)", n))),
        }
    }

    pub mod option {
        use super::AnnexeChomage;

        pub fn serialize<S: serde::Serializer>(a: &Option<AnnexeChomage>, s: S) -> Result<S::Ok, S::Error> {
            match a {
                Some(a) => s.serialize_some(&a.numero()),
                None => s.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<AnnexeChomage>, D::Error> {
            match <Option<u8> as serde::Deserialize>::deserialize(d)? {
                Some(n) => super::lue(n).map(Some),
                None => Ok(None),
            }
        }
    }
}

/// LE CODE D'UN METIER DU SPECTACLE : un code de la nomenclature des emplois du spectacle de France Travail
/// (`nomenclature-emplois-ft.json`, ses 1 236 codes relevés de
/// `atelier/reference/sources/francetravail-nomenclature-emplois-spectacle-20161212.pdf`, « ACC010 ACCESSOIRISTE,
/// ART120 ARTISTE DRAMATIQUE... » : `logiciel/donnees/sources.json`, `francetravail-nomenclature-emplois-2016`). Chaque
/// metier du catalogue (`emplois-base.json`) a le sien, et un metier hors catalogue se choisit dans la nomenclature
/// (sa `note`) : le metier qu'une carte choisit, la cle du catalogue, celui d'un deroulant. Un metier que la compagnie a
/// cree (Configuration › Métiers, `MetierCree`) a le sien, `CIE-` et son numero : il ne s'ecrit jamais comme un code de
/// la nomenclature (ses codes n'ont que des lettres et des chiffres), et ne masque donc aucun metier du catalogue. Hors
/// de ces deux formes, un code se refuse. Saisi, il se range sans ses blancs de bord et en capitales.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CodeEmploi(String);

/// Le debut du code d'un metier que la compagnie a cree (`CodeEmploi::cree`).
const PREFIXE_CREE: &str = "CIE-";

impl CodeEmploi {
    /// Le code `texte`, s'il est de la nomenclature, ou celui d'un metier cree (`CIE-` et son numero).
    pub fn lire(texte: &str) -> R<CodeEmploi> {
        if CodeEmploi::numero_de(texte).is_some() {
            return Ok(CodeEmploi(texte.to_string()));
        }
        match CodeEmploi::de_la_nomenclature(texte)? {
            Some(c) => Ok(c),
            None => Err(socle::refus!("Métier {} inconnu : ni dans le catalogue des emplois, ni dans la nomenclature de France Travail.", repr(texte))),
        }
    }

    /// Le code du metier cree de numero `numero` : `CIE-1`, `CIE-2`...
    pub fn cree(numero: u32) -> CodeEmploi {
        CodeEmploi(format!("{}{}", PREFIXE_CREE, numero))
    }

    /// Le numero d'un code de metier cree (`CIE-` et un entier sans zero de tete), ou `None`.
    fn numero_de(texte: &str) -> Option<u32> {
        let n = texte.strip_prefix(PREFIXE_CREE)?;
        if n.starts_with('0') || !n.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        n.parse::<u32>().ok()
    }

    /// Le numero du metier, s'il est un metier que la compagnie a cree.
    pub fn numero_cree(&self) -> Option<u32> {
        CodeEmploi::numero_de(&self.0)
    }

    /// Le code `texte`, s'il est de la nomenclature ; `None` sinon.
    fn de_la_nomenclature(texte: &str) -> R<Option<CodeEmploi>> {
        let code = CodeEmploi(texte.to_string());
        Ok(nomenclature()?.emplois.contains_key(&code).then_some(code))
    }

    /// LE METIER QUE NOMME UN CODE EMPLOI DECLARE (S21.G00.40.005, `CodeEmploiFt`) : celui de la nomenclature de meme
    /// code, quand il y est (un code de la table ART) ; `None` pour un code qui n'en nomme aucun (« 999SPT », un autre
    /// code de la norme).
    pub fn du_code_declare(c: &crate::fiche::CodeEmploiFt) -> R<Option<CodeEmploi>> {
        CodeEmploi::de_la_nomenclature(c.as_str())
    }

    /// Le texte saisi `texte` dans sa forme, avant d'etre lu : sans ses blancs de bord, en capitales.
    pub fn rangee(texte: &str) -> String {
        socle::texte::en_capitales(texte)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// SON METIER, le seul chemin : celui que la compagnie a cree (`config/metiers-crees.json` sous `racine`), sinon celui
    /// du catalogue, sinon celui de la nomenclature, qui a tous les codes. Un metier cree qui n'est pas dans les metiers
    /// de la compagnie se refuse en disant ou le creer.
    pub fn metier(&self, racine: &Path) -> R<Metier> {
        if self.numero_cree().is_none() {
            return self.du_catalogue_ou_de_la_nomenclature();
        }
        let crees = metiers_crees(racine)?;
        let Some(m) = crees.iter().find(|m| m.code == *self) else {
            return Err(socle::refus!("Le métier {} n'est pas dans les métiers de la compagnie : créez-le d'abord dans Configuration › Métiers.", self));
        };
        // Le calcul qui lit un metier cree depend de ce qu'il dit : son fichier se note dans l'empreinte de la paie.
        socle::fichiers::noter_si_besoin(&METIERS_CREES.dans(racine));
        Ok(m.metier())
    }

    /// Le metier d'un code de la nomenclature : celui du catalogue, sinon celui de la nomenclature.
    fn du_catalogue_ou_de_la_nomenclature(&self) -> R<Metier> {
        if let Some((_, f)) = base()?.emplois.iter().find(|(c, _)| c == self) {
            return Ok(Metier { code: self.clone(), code_declare: self.clone(), fiche: FicheDuMetier::Catalogue(f.clone()) });
        }
        let n = nomenclature()?.emplois.get(self).cloned().ok_or_else(|| socle::echec::livree(&NOMENCLATURE, &self.0))?;
        Ok(Metier { code: self.clone(), code_declare: self.clone(), fiche: FicheDuMetier::Nomenclature(n) })
    }
}

impl std::fmt::Display for CodeEmploi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CodeEmploi {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        CodeEmploi::lire(&socle::forme::range_si_saisi(String::deserialize(d)?, CodeEmploi::rangee)).map_err(|e| serde::de::Error::custom(e.phrase))
    }
}

socle::noms_fermes! {
    /// LA FAMILLE D'UN METIER DU CATALOGUE (`emplois-base.json`), qui groupe le deroulant.
    FamilleDeMetier, "Famille de métier" {
        Artistes = "Artistes",
        AudiovisuelDansLeSpectacle = "Audiovisuel dans le spectacle",
        CirqueEtRue = "Cirque et rue",
        CostumeCoiffureMaquillage = "Costume, coiffure, maquillage",
        CreationEtEvenement = "Création et événement",
        DecorEtConstruction = "Décor et construction",
        EditionPhonographique = "Édition phonographique",
        FilmsDAnimation = "Films d'animation",
        Lumiere = "Lumière",
        MiseEnSceneEtCreation = "Mise en scène et création",
        Musique = "Musique",
        ProductionAudiovisuelle = "Production audiovisuelle",
        ProductionCinematographique = "Production cinématographique",
        ProductionEtAdministration = "Production et administration",
        Radiodiffusion = "Radiodiffusion",
        RegieEtPlateau = "Régie et plateau",
        Son = "Son",
        TechniquesDuSpectacle = "Techniques du spectacle",
        Telediffusion = "Télédiffusion",
    }
}

/// Un metier du catalogue : ce que les declarations demandent (le libelle et sa forme declaree S21.G00.40.006, la
/// PCS-ESE 40.004, le statut artiste ou technicien, le statut conventionnel 40.002, cadre par defaut 40.003), sa famille,
/// les conventions dont le deroulant le montre, et la ligne de minima et de classification de chaque convention qui
/// le nomme (la `note` de `emplois-base.json`).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FicheDuCatalogue {
    pub libelle: String,
    pub feminin: String,
    pub pcs_ese: CodePcsEse,
    pub statut: Statut,
    pub famille: FamilleDeMetier,
    pub cadre_par_defaut: bool,
    /// Le defaut cadre d'une convention qui le dit autrement (le dramaturge, cadre dans la 1285).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cadre_selon_la_convention: Option<BTreeMap<Idcc, bool>>,
    pub statut_conventionnel: StatutConventionnel,
    pub minima: Minima,
    pub emploi: String,
    #[serde(with = "annexe_en_nombre")]
    pub annexe: AnnexeChomage,
    /// L'annexe que la nomenclature France Travail donne au code, quand le catalogue en retient une autre.
    #[serde(default, with = "annexe_en_nombre::option", skip_serializing_if = "Option::is_none")]
    pub annexe_nomenclature: Option<AnnexeChomage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conventions: Option<Vec<Idcc>>,
    /// Les conventions dont le deroulant le montre sans que leur liste de l'annexe 8 le nomme.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conventions_hors_liste: Option<Vec<Idcc>>,
    /// IDCC -> le classement conventionnel du metier dans cette convention.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classement: Option<BTreeMap<Idcc, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification_1285: Option<Classification1285>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification_2642: Option<Fonction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification_3090: Option<Classification3090>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification_3097: Option<Classification3097>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// La ligne de minima de chaque convention qui nomme le metier : la 3090 par annexe (annexe -> ligne), les autres en
/// une ligne.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Minima {
    #[serde(rename = "3090", skip_serializing_if = "Option::is_none")]
    pub ccn_3090: Option<BTreeMap<AnnexeConvention, CleDeGrille>>,
    #[serde(rename = "1285", skip_serializing_if = "Option::is_none")]
    pub ccn_1285: Option<CleDeGrille>,
    #[serde(rename = "2642", skip_serializing_if = "Option::is_none")]
    pub ccn_2642: Option<CleDeGrille>,
    #[serde(rename = "3097", skip_serializing_if = "Option::is_none")]
    pub ccn_3097: Option<CleDeGrille>,
}

impl Minima {
    /// La ligne de minima d'une convention qui n'en a qu'une (1285, 2642, 3097), ou `None`.
    pub fn ligne(&self, idcc: &Idcc) -> Option<&CleDeGrille> {
        if *idcc == crate::clauses::IDCC_CCNEAC {
            self.ccn_1285.as_ref()
        } else if *idcc == crate::fiche::IDCC_2642 {
            self.ccn_2642.as_ref()
        } else if *idcc == crate::fiche::IDCC_3097 {
            self.ccn_3097.as_ref()
        } else {
            None
        }
    }
}

/// La classification de la CCNEAC (1285) : le groupe de la grille des emplois autres qu'artistiques, et l'emploi
/// repere de l'article XI.3.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Classification1285 {
    pub groupe: Option<u32>,
    pub emploi_xi3: String,
}

/// La classification de la 3090 : la grille (technique, administratif) et le niveau.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Classification3090 {
    pub grille: CleDeGrille,
    pub niveau: CleDeGrille,
}

/// La fonction du metier dans la grille de la 2642.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Fonction {
    pub fonction: CleDeGrille,
}

/// La fonction du metier dans la grille de la 3097 (titre II), et l'emploi repere du titre IV qui donne le salaire d'un
/// permanent (`minima-conventionnels.json`, `3097._permanents` : seuls les metiers qui le disent sont compares).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Classification3097 {
    pub fonction: CleDeGrille,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titre_iv: Option<CleDeGrille>,
}

/// `nomenclature-emplois-ft.json` : les codes emploi France Travail (S21.G00.40.005), leur libelle et leur annexe.
/// Ses cles sont les codes des metiers : la nomenclature est la liste des `CodeEmploi`, elle les fait (`lue`).
#[derive(Debug, Clone)]
pub struct Nomenclature {
    pub quoi: String,
    pub note: String,
    pub emplois: BTreeMap<CodeEmploi, EmploiFt>,
}

/// `nomenclature-emplois-ft.json` tel qu'ecrit : ses codes, avant d'etre ceux des metiers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NomenclatureLue {
    quoi: String,
    note: String,
    emplois: BTreeMap<String, EmploiFt>,
}

impl NomenclatureLue {
    /// La nomenclature : chacun de ses codes est un `CodeEmploi`.
    fn lue(self) -> R<Nomenclature> {
        Ok(Nomenclature { quoi: self.quoi, note: self.note, emplois: self.emplois.into_iter().map(|(code, e)| (CodeEmploi(code), e)).collect() })
    }
}

/// Un code de la nomenclature : son libelle et son annexe (8 technicien, 10 artiste).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EmploiFt {
    pub libelle: String,
    #[serde(with = "annexe_en_nombre")]
    pub annexe: AnnexeChomage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evolution_2016: Option<String>,
}

const CATALOGUE_ILLISIBLE: &str = "Le catalogue des métiers livré avec le logiciel ne se lit pas : le signaler au développeur.";

/// LA SEULE LECTURE D'`emplois-base.json`.
pub fn base() -> R<Arc<BaseEmplois>> {
    static CACHE: OnceLock<FormeGardee<BaseEmplois>> = OnceLock::new();
    forme_en_cache(&CACHE, &BASE, CATALOGUE_ILLISIBLE, Ok)
}

/// LA SEULE LECTURE DE `nomenclature-emplois-ft.json`.
pub fn nomenclature() -> R<Arc<Nomenclature>> {
    static CACHE: OnceLock<FormeGardee<Nomenclature>> = OnceLock::new();
    forme_en_cache(&CACHE, &NOMENCLATURE, "La nomenclature des emplois France Travail livrée avec le logiciel ne se lit pas : le signaler au développeur.", NomenclatureLue::lue)
}

/// Premiere lettre en majuscule, le reste en minuscules.
fn capitaliser(s: &str) -> String {
    s.chars().take(1).flat_map(char::to_uppercase).chain(s.chars().skip(1).flat_map(char::to_lowercase)).collect()
}

/// Un metier, par son code : celui du catalogue, ou, hors catalogue, celui de la nomenclature France Travail (un code
/// et une annexe ; ni PCS, ni niveau, ni minima), ou celui que la compagnie a cree (`MetierCree` : sa fiche est une
/// fiche du catalogue, copiee puis modifiee).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Metier {
    pub code: CodeEmploi,
    /// Le code de la nomenclature qu'il fait declarer (S21.G00.40.005) : le sien, sauf pour un metier cree.
    #[serde(skip)]
    code_declare: CodeEmploi,
    #[serde(flatten)]
    pub fiche: FicheDuMetier,
}

/// D'ou vient un metier : le catalogue, ou la nomenclature seule.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum FicheDuMetier {
    Catalogue(FicheDuCatalogue),
    Nomenclature(EmploiFt),
}

impl Metier {
    /// LE CODE EMPLOI QU'IL FAIT DECLARER (S21.G00.40.005) : un code de la nomenclature des emplois du spectacle, la
    /// table ART (CCH-18, `fiche::CodeEmploiFt`), le sien ou, pour un metier cree, celui que la compagnie lui a donne.
    pub fn code_emploi_ft(&self) -> CodeEmploiFt {
        CodeEmploiFt::du_metier(&self.code_declare)
    }

    /// Sa fiche du catalogue, ou `None` hors catalogue.
    pub fn au_catalogue(&self) -> Option<&FicheDuCatalogue> {
        match &self.fiche {
            FicheDuMetier::Catalogue(f) => Some(f),
            FicheDuMetier::Nomenclature(_) => None,
        }
    }

    /// Le libelle lisible : celui du catalogue, ou celui de la nomenclature en casse ordinaire.
    pub fn libelle(&self) -> String {
        match &self.fiche {
            FicheDuMetier::Catalogue(f) => f.libelle.clone(),
            FicheDuMetier::Nomenclature(n) => capitaliser(&n.libelle),
        }
    }

    /// Le libelle au feminin (hors catalogue, le meme).
    pub fn feminin(&self) -> String {
        match &self.fiche {
            FicheDuMetier::Catalogue(f) => f.feminin.clone(),
            FicheDuMetier::Nomenclature(n) => capitaliser(&n.libelle),
        }
    }

    /// La forme declaree de l'emploi (S21.G00.40.006).
    pub fn emploi(&self) -> &str {
        match &self.fiche {
            FicheDuMetier::Catalogue(f) => &f.emploi,
            FicheDuMetier::Nomenclature(n) => &n.libelle,
        }
    }

    /// L'annexe de l'assurance chomage : VIII (technicien) ou X (artiste).
    pub fn annexe(&self) -> AnnexeChomage {
        match &self.fiche {
            FicheDuMetier::Catalogue(f) => f.annexe,
            FicheDuMetier::Nomenclature(n) => n.annexe,
        }
    }

    /// Le statut : celui du catalogue ; hors catalogue, artiste pour l'annexe 10, technicien sinon.
    pub fn statut(&self) -> Statut {
        match &self.fiche {
            FicheDuMetier::Catalogue(f) => f.statut,
            FicheDuMetier::Nomenclature(n) if n.annexe == AnnexeChomage::X => Statut::Artiste,
            FicheDuMetier::Nomenclature(_) => Statut::Technicien,
        }
    }

    /// La famille du catalogue ; aucune hors du catalogue.
    pub fn famille(&self) -> Option<FamilleDeMetier> {
        self.au_catalogue().map(|f| f.famille)
    }

    /// La PCS-ESE du catalogue (hors catalogue : aucune).
    pub fn pcs_ese(&self) -> Option<&CodePcsEse> {
        self.au_catalogue().map(|f| &f.pcs_ese)
    }

    /// La classification 3090 du catalogue, ou `None`.
    pub fn classification_3090(&self) -> Option<&Classification3090> {
        self.au_catalogue().and_then(|f| f.classification_3090.as_ref())
    }

    /// La liste de l'annexe 8 de la convention `idcc` nomme-t-elle ce metier ? Ce n'est pas le deroulant : un metier
    /// porte dans `conventions` les conventions dont le deroulant le montre, dont celles qui le portent sans que leur liste
    /// le nomme (`conventions_hors_liste`, et la regle de lecture des listes 6 et 7 dans la `note` de `emplois-base.json`).
    /// Un metier hors catalogue ne dit pas sur quelle liste il est : non.
    pub fn nomme_par_l_annexe8(&self, idcc: &Idcc) -> bool {
        self.au_catalogue().is_some_and(|f| f.conventions.iter().flatten().any(|c| c == idcc) && !f.conventions_hors_liste.iter().flatten().any(|c| c == idcc))
    }

    /// Le metier est-il cadre, faute d'engagement qui le dise, dans la convention `idcc` ? `cadre_selon_la_convention`
    /// l'emporte pour la convention qu'il nomme (le dramaturge, cadre dans la 1285, art. XI.2.1, que la grille 3090 ne
    /// nomme pas : `cadre_par_defaut` dans la `note` de `emplois-base.json`) ; sinon `cadre_par_defaut`. Hors catalogue :
    /// `None`, seule la compagnie le sait (la fiche le dit).
    pub fn cadre_par_defaut(&self, idcc: Option<&Idcc>) -> Option<bool> {
        self.au_catalogue().map(|f| match f.cadre_selon_la_convention.as_ref().zip(idcc).and_then(|(c, i)| c.get(i)) {
            Some(cadre) => *cadre,
            None => f.cadre_par_defaut,
        })
    }

    /// Le defaut cadre de ce metier change-t-il d'une convention a l'autre (`cadre_selon_la_convention`) ? Une carte qui le
    /// dit garde alors ce qu'elle dit (`engagement::stock::en_forme`).
    pub fn cadre_selon_la_convention(&self) -> bool {
        self.au_catalogue().is_some_and(|f| f.cadre_selon_la_convention.as_ref().is_some_and(|c| !c.is_empty()))
    }

    /// Le statut conventionnel DSN (40.002) du metier declare NON cadre. L'artiste-interprete : 05, par inference
    /// (`statut_conventionnel` dans la `note` de `emplois-base.json` : « Non-cadres » de la 3090, art. 6.3, hors de la categorie
    /// cadre de la 1285, art. XI.2.1 ; 05 par l'exemple de la fiche net-entreprises 3344, l. 364-366).
    /// Un niveau 3090 de categorie cadre ou agent de maitrise (la categorie de chaque niveau :
    /// `categories.par_niveau_3090` de la 3090 dans `clauses-conventionnelles-base.json`) se declare agent de maitrise ;
    /// un niveau que cette table ne range pas se refuse.
    pub fn statut_non_cadre(&self) -> R<Option<StatutConventionnel>> {
        if let Some(st) = self.au_catalogue().map(|f| f.statut_conventionnel).filter(|s| *s != StatutConventionnel::Cadre04) {
            return Ok(Some(st));
        }
        if self.statut() == Statut::Artiste {
            return Ok(Some(StatutConventionnel::AgentDeMaitrise));
        }
        let Some(c) = self.classification_3090() else { return Ok(None) };
        let t = crate::clauses::table()?;
        let categorie = t
            .convention(Some(&crate::fiche::IDCC_3090))
            .and_then(|conv| conv.categories.as_ref())
            .and_then(|s| s.par_niveau_3090.as_ref())
            .and_then(|par| par.get(&c.niveau))
            .ok_or_else(|| socle::echec::livree(&crate::clauses::FICHIER, &format!("conventions.3090.categories.par_niveau_3090.{}", c.niveau)))?;
        if [crate::fiche::CategorieConventionnelle::Cadre, crate::fiche::CategorieConventionnelle::AgentDeMaitrise].contains(categorie) {
            return Ok(Some(StatutConventionnel::AgentDeMaitrise));
        }
        Ok(Some(if c.grille == crate::minima::ADMINISTRATIF { StatutConventionnel::Employe06 } else { StatutConventionnel::Employe07 }))
    }
}

/// Le metier que nomme le code emploi declare `code` (S21.G00.40.005), tel qu'il est ecrit, s'il est un code de la
/// nomenclature (`CodeEmploi::metier`). `None` sinon : un emploi permanent (« 999SPT »), un code d'une autre table.
pub fn metier(code: &CodeEmploiFt) -> R<Option<Metier>> {
    match CodeEmploi::de_la_nomenclature(code.as_ref())? {
        Some(c) => c.du_catalogue_ou_de_la_nomenclature().map(Some),
        None => Ok(None),
    }
}

/// LES SEPT CHAMPS DECLARES POUR UN METIER, tels que la fiche du salarie les ecrit : l'emploi declare et son libelle,
/// les codes PCS-ESE et France Travail, le statut, les statuts conventionnel (S21.G00.40.002) et categoriel de retraite
/// complementaire (S21.G00.40.003).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ChampsDuMetier {
    pub emploi: String,
    pub emploi_affichage: String,
    pub pcs_ese: Option<CodePcsEse>,
    pub code_emploi_ft: Option<CodeEmploiFt>,
    pub statut: Statut,
    pub statut_conventionnel: Option<StatutConventionnel>,
    pub statut_categoriel_rc: StatutCategorielRc,
}

/// Les sept champs declares pour le metier `m` (`emploi_affichage` compris), lu par `CodeEmploi::metier` (un metier cree
/// comme un autre). `cadre` a `None` prend celui du metier dans la convention `idcc` (`Metier::cadre_par_defaut`) ;
/// seule la cle gardee d'une carte ecrite avant le dit autrement (`saisie::Carte.cadre`).
pub fn resoudre(m: &Metier, cadre: Option<bool>, feminin: bool, idcc: Option<&Idcc>) -> R<ChampsDuMetier> {
    let est_cadre = match cadre.or_else(|| m.cadre_par_defaut(idcc)) {
        Some(c) => c,
        None => return Err(socle::refus!("Le métier {} est hors catalogue : la fiche doit dire s'il est cadre.", repr(m.code.as_str()))),
    };
    let libelle = if feminin { m.feminin() } else { m.libelle() };
    Ok(ChampsDuMetier {
        emploi: m.emploi().to_string(),
        emploi_affichage: libelle,
        pcs_ese: m.pcs_ese().cloned(),
        code_emploi_ft: Some(m.code_emploi_ft()),
        statut: m.statut(),
        statut_conventionnel: if est_cadre { Some(StatutConventionnel::Cadre04) } else { m.statut_non_cadre()? },
        statut_categoriel_rc: if est_cadre { StatutCategorielRc::Cadre } else { StatutCategorielRc::NonCadre },
    })
}

/// Les cases de `UNE_VALEUR_PAR_MOIS` d'une fiche, dans son ordre, chacune telle qu'ecrite : `None` quand elle est vide.
fn valeurs_par_mois(f: &FicheSalarie) -> [(CaseSalarie, Option<String>); 5] {
    fn portee<T: std::fmt::Display>(c: &Option<T>) -> Option<String> {
        c.as_ref().map(T::to_string)
    }
    [
        (UNE_VALEUR_PAR_MOIS[0], portee(&f.statut_categoriel_rc)),
        (UNE_VALEUR_PAR_MOIS[1], portee(&f.statut_conventionnel)),
        (UNE_VALEUR_PAR_MOIS[2], portee(&f.statut)),
        (UNE_VALEUR_PAR_MOIS[3], portee(&f.code_emploi_ft)),
        (UNE_VALEUR_PAR_MOIS[4], portee(&f.pcs_ese)),
    ]
}

/// Ce qu'un contrat paye au mois ne laisse pas changer en cours de mois : ce qui choisit la pile du bareme (le statut
/// artiste / technicien, la categorie cadre, le metier qui la dit a defaut de la fiche) et ce que le bloc 40 porte avec
/// une rubrique en bloc 41 (40.002, 40.003, 40.004). Le libelle de l'emploi n'en est pas : la DSN declare « la derniere
/// valeur connue » du mois (cahier technique 2026.1, § 2.1.2.4.2, p. 22).
const UNE_VALEUR_PAR_MOIS: [CaseSalarie; 5] = [CaseSalarie::StatutCategorielRc, CaseSalarie::StatutConventionnel, CaseSalarie::Statut, CaseSalarie::CodeEmploiFt, CaseSalarie::PcsEse];

/// La fiche salarie `cle` d'un contrat PAYE AU MOIS (CDI, CDD de droit commun), telle qu'elle vaut pour la periode payee
/// (`periode`) : lue a sa fin. Un permanent passe cadre, change de poste, obtient ou perd sa RQTH en
/// cours de contrat : la paie et la DSN du mois ou cela prend effet le disent.
///
/// UN CHANGEMENT DANS LA PERIODE REFUSE (`UNE_VALEUR_PAR_MOIS`) : une paie se calcule sur une seule pile de bareme
/// (regle 6). La paie d'un mois ou ces valeurs changent se decoupe en periodes elementaires aux jours du changement
/// (`jours_de_changement`, `periodes::bornes`, norme § 2.1.3.2, p. 28), et chaque periode lit sa fiche ici, sur ses
/// bornes : le refus ne garde que l'appel sur une periode qui n'aurait pas ete coupee. Le statut BOETH, qui ne change
/// pas la paie, peut changer en cours de mois (40.072 se renseigne « pour le contrat concerne dans la periode
/// declaree », p. 211 : celui de la fin).
pub fn emploi_du_mois(ctx: &Contexte, cle: &IdSalarie, periode: &Periode) -> R<FicheSalarie> {
    let au_debut = ctx.salarie_au(cle, &periode.debut())?;
    let a_la_fin = ctx.salarie_au(cle, &periode.fin())?;
    for ((c, avant), (_, apres)) in valeurs_par_mois(&au_debut).into_iter().zip(valeurs_par_mois(&a_la_fin)) {
        if avant == apres {
            continue;
        }
        let c = CaseDeFiche::from(c);
        let le = ctx.salarie_brut(cle).into_iter().flat_map(|b| b.depuis(c)).flatten().filter(|j| *j > periode.debut() && periode.contient(j)).last();
        let mois = periode.debut().mois_civil();
        let (premier, suivant) = (mois.premier_jour(), mois.suivant().premier_jour());
        let texte = |v: &Option<String>| match v {
            Some(v) => repr(v),
            None => "(vide)".to_string(),
        };
        let qui = [a_la_fin.prenom.portee(), a_la_fin.nom.portee()].into_iter().flatten().filter(|s| !s.trim().is_empty()).collect::<Vec<_>>().join(" ");
        return Err(socle::echec::a_la_cle(&format!(
            "{} : la fiche change une case {}({} -> {}), dans une periode de paie qui n'est pas coupee a ce jour-la : une \
periode se calcule sur une seule pile de bareme. Dater le changement au premier jour d'un mois sur la fiche : le {} (il \
vaut pour tout ce mois-ci) ou le {} (il vaut a partir du mois suivant).",
            if qui.is_empty() { "Ce salarie".to_string() } else { qui },
            match le {
                Some(j) => format!("le {} ", j.jj_mm_aaaa()),
                None => "en cours de periode ".to_string(),
            },
            texte(&avant),
            texte(&apres),
            premier.jj_mm_aaaa(),
            suivant.jj_mm_aaaa()
        ), c.nom(), ""));
    }
    Ok(a_la_fin)
}

/// Les jours de `periode`, apres son premier (la periode payee d'un contrat paye au mois), ou la fiche `cle` change
/// une valeur qui choisit la pile du bareme ou que le bloc 40 porte avec une rubrique en bloc 41 (`UNE_VALEUR_PAR_MOIS`) :
/// le `depuis` d'une de ces valeurs, quand la fiche de la veille et celle du jour different. C'est la que la paie du mois
/// se decoupe en periodes elementaires (`periodes::bornes` ; norme DSN 2026.1, § 2.1.3.2, p. 28 : « passage de non
/// cadre a cadre en cours de periode de paie »). Vide : le mois se paie d'un seul tenant.
pub fn jours_de_changement(ctx: &Contexte, cle: &IdSalarie, periode: &Periode) -> R<Vec<Jour>> {
    let Some(brute) = ctx.salarie_brut(cle) else {
        return Err(socle::refus!("La fiche salarié « {} » n'existe pas : sa paie au mois ne se découpe pas.", cle));
    };
    let mut jours: Vec<Jour> = Vec::new();
    for c in UNE_VALEUR_PAR_MOIS {
        jours.extend(brute.depuis(c.into()).into_iter().flatten().filter(|j| *j > periode.debut() && periode.contient(j)));
    }
    jours.sort();
    jours.dedup();
    let mut out = Vec::new();
    for j in jours {
        let (avant, apres) = (brute.au(&j.avant(socle::date::Duree::jours(1))?), brute.au(&j));
        if !avant.memes_cases(&apres, &UNE_VALEUR_PAR_MOIS) {
            out.push(j);
        }
    }
    Ok(out)
}

/// L'EMPLOI ECRIT : les six champs de l'emploi qu'une carte ecrit a la main, ou qu'un bulletin importe imprime, chacun
/// seulement quand il est dit. Un emploi qui ne dit rien n'est pas ecrit : sa case est vide (`EmploiEcrit::dit`). Son
/// libelle lisible n'en est pas un champ : l'engagement le resout (`EmploiEngagement`).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmploiEcrit {
    pub emploi: Option<String>,
    pub pcs_ese: Option<CodePcsEse>,
    pub code_emploi_ft: Option<CodeEmploiFt>,
    pub statut: Option<Statut>,
    pub statut_conventionnel: Option<StatutConventionnel>,
    pub statut_categoriel_rc: Option<StatutCategorielRc>,
}

impl EmploiEcrit {
    /// La case d'un emploi ecrit, lue (`deserialize_with`) : `null`, absente, ou un objet qui ne dit aucun champ, elle
    /// est vide.
    pub fn dit<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<EmploiEcrit>, D::Error> {
        Ok(Option::<EmploiEcrit>::deserialize(d)?.filter(|e| *e != EmploiEcrit::default()))
    }
}

/// L'EMPLOI D'UN ENGAGEMENT : ce qu'il dit de l'emploi qu'il fait declarer, le metier de sa carte resolu (`resoudre`)
/// sous ce que la carte ecrit a la main (`engagement::engagements`), ou ce qu'un bulletin importe imprime (`ecrit`) ; et
/// le libelle lisible de son metier, que l'engagement resout (`emploi_affichage`). Chaque champ seulement quand il est
/// dit, le reste vient de la fiche du salarie (`emploi_de`). Ce n'est pas un metier du catalogue (`Metier`). Il s'ecrit
/// en un objet a plat, dans la paie ecrite et ce dont un engagement se fabrique (`EmploiEngagementEcrit`).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(from = "EmploiEngagementEcrit", into = "EmploiEngagementEcrit")]
pub struct EmploiEngagement {
    pub ecrit: EmploiEcrit,
    /// Le libelle lisible que l'engagement a resolu.
    pub emploi_affichage: Option<String>,
}

/// La forme ecrite d'un `EmploiEngagement` : ses sept champs a plat.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EmploiEngagementEcrit {
    emploi: Option<String>,
    emploi_affichage: Option<String>,
    pcs_ese: Option<CodePcsEse>,
    code_emploi_ft: Option<CodeEmploiFt>,
    statut: Option<Statut>,
    statut_conventionnel: Option<StatutConventionnel>,
    statut_categoriel_rc: Option<StatutCategorielRc>,
}

impl From<EmploiEngagementEcrit> for EmploiEngagement {
    fn from(e: EmploiEngagementEcrit) -> EmploiEngagement {
        let EmploiEngagementEcrit { emploi, emploi_affichage, pcs_ese, code_emploi_ft, statut, statut_conventionnel, statut_categoriel_rc } = e;
        EmploiEngagement { ecrit: EmploiEcrit { emploi, pcs_ese, code_emploi_ft, statut, statut_conventionnel, statut_categoriel_rc }, emploi_affichage }
    }
}

impl From<EmploiEngagement> for EmploiEngagementEcrit {
    fn from(e: EmploiEngagement) -> EmploiEngagementEcrit {
        let EmploiEngagement { ecrit: EmploiEcrit { emploi, pcs_ese, code_emploi_ft, statut, statut_conventionnel, statut_categoriel_rc }, emploi_affichage } = e;
        EmploiEngagementEcrit { emploi, emploi_affichage, pcs_ese, code_emploi_ft, statut, statut_conventionnel, statut_categoriel_rc }
    }
}

/// La fiche salarie `sal` (lue a la date de la periode) telle qu'elle vaut POUR CET ENGAGEMENT : ce que l'engagement
/// dit l'emporte ; le reste vient de la fiche.
pub fn emploi_de(sal: &FicheSalarie, emploi: &EmploiEngagement) -> R<FicheSalarie> {
    let mut out = sal.clone();
    let dit = |v: &Option<String>| v.as_ref().filter(|s| !s.is_empty()).cloned();
    let eng = &emploi.ecrit;
    let pcs_ese = eng.pcs_ese.clone();
    let affichage = dit(&emploi.emploi_affichage);
    let statuts_dits = eng.statut.is_some() || eng.statut_conventionnel.is_some() || eng.statut_categoriel_rc.is_some();
    if dit(&eng.emploi).is_none() && eng.code_emploi_ft.is_none() && pcs_ese.is_none() && !statuts_dits && affichage.is_none() {
        return Ok(out);
    }
    if let Some(v) = dit(&eng.emploi) {
        out.emploi = Some(v);
    }
    if let Some(v) = pcs_ese {
        out.pcs_ese = Some(v);
    }
    if let Some(v) = &eng.code_emploi_ft {
        out.code_emploi_ft = Some(v.clone());
    }
    if let Some(v) = eng.statut {
        out.statut = Some(v);
    }
    if let Some(v) = eng.statut_conventionnel {
        out.statut_conventionnel = Some(v);
    }
    if let Some(v) = eng.statut_categoriel_rc {
        out.statut_categoriel_rc = Some(v);
    }
    // Un metier choisi sur la ligne : son libelle lisible aussi, tel que
    // l'engagement l'a resolu (au feminin si la fiche le dit) ; a defaut, le
    // libelle du catalogue.
    if let Some(a) = affichage {
        out.emploi_affichage = Some(a);
    } else if let Some(code) = &eng.code_emploi_ft {
        if sal.code_emploi_ft.dite() != Some(code) {
            let m = metier(code)?.ok_or_else(|| socle::refus!("Le métier {} de l'engagement n'est ni au catalogue ni dans la nomenclature France Travail.", repr(code)))?;
            out.emploi_affichage = Some(m.libelle());
        }
    }
    Ok(out)
}

// ------------------------------------------------------- les metiers crees

/// Les metiers que la compagnie a crees, sous sa racine (Configuration › Métiers).
pub const METIERS_CREES: CheminRelatif = CheminRelatif::du_moteur("config/metiers-crees.json");

/// `config/metiers-crees.json` : les metiers que la compagnie a crees, chacun copie d'un metier puis modifie.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetiersCrees {
    pub quoi: String,
    pub metiers: Vec<MetierCree>,
}

/// Ce que dit le fichier des metiers crees (`MetiersCrees::quoi`).
pub const QUOI_METIERS_CREES: &str = "Les metiers que cette compagnie a crees dans Configuration > Metiers : chacun copie d'un metier (copie_de), puis modifie. Son code (CIE- et un numero) ne se declare pas : la DSN declare son code_declare, un code de la nomenclature des emplois du spectacle. Sa fiche a la forme d'un metier du catalogue (donnees/emplois-base.json).";

/// UN METIER QUE LA COMPAGNIE A CREE (Leandre, 20/11/2026 : « On copie et on modifie ») : son code (`CodeEmploi::cree`),
/// le metier dont il est la copie (du catalogue, ou deja cree), le code de la nomenclature qu'il fait declarer
/// (S21.G00.40.005, CCH-18 : un code de la table des emplois du spectacle), et sa fiche, de la forme d'un metier du
/// catalogue : le moteur le lit comme eux (`CodeEmploi::metier`), un seul chemin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetierCree {
    pub code: CodeEmploi,
    pub copie_de: CodeEmploi,
    /// Le code de la nomenclature qu'il fait declarer (S21.G00.40.005), qui n'est jamais un code cree.
    pub code_declare: CodeEmploi,
    pub fiche: FicheDuCatalogue,
}

impl MetierCree {
    /// Son metier, tel que le moteur le lit (`CodeEmploi::metier`).
    pub fn metier(&self) -> Metier {
        Metier { code: self.code.clone(), code_declare: self.code_declare.clone(), fiche: FicheDuMetier::Catalogue(self.fiche.clone()) }
    }

    /// UN METIER NON CADRE SE DECLARE AVEC UN CLASSEMENT NON CADRE (S21.G00.40.002) : celui que la DSN lit de lui
    /// (`Metier::statut_non_cadre`, comme pour un metier du catalogue) est 05, 06 ou 07. Sinon -- la copie non cadre d'un
    /// metier cadre, un classement 03 -- la DSN le refuserait ou le declarerait cadre dirigeant non cadre : il se refuse
    /// ici, en disant quoi choisir.
    pub fn classement_qui_se_declare(&self) -> R<()> {
        if self.fiche.cadre_par_defaut || self.metier().statut_non_cadre()?.is_some_and(|s| !s.cadre()) {
            return Ok(());
        }
        let choix: Vec<&str> = StatutConventionnel::NON_CADRES.iter().map(|s| s.as_str()).collect();
        Err(socle::refus!(
            "Le métier {} ({}) n'est pas cadre : son classement conventionnel (DSN) se choisit parmi {}, sous « Avancé » ; ou il est cadre. Rien n'est écrit.",
            self.fiche.libelle,
            self.code,
            choix.join(", ")
        ))
    }

    /// LA COPIE du metier `de` (du catalogue, ou deja cree) sous le code `code` : nommee `libelle` (au feminin aussi, qui
    /// se change ensuite), cadre ou non (`cadre`) dans toute convention ; le reste est celui de `de`, le code qu'il fait
    /// declarer compris. Un metier de la seule nomenclature n'a pas de fiche a copier.
    pub fn copie(code: CodeEmploi, de: &Metier, libelle: String, cadre: bool) -> R<MetierCree> {
        let Some(f) = de.au_catalogue() else {
            return Err(socle::refus!("Le métier {} n'est pas au catalogue : il ne se copie pas ; copiez un métier de la liste.", de.code));
        };
        let mut fiche = f.clone();
        fiche.feminin = libelle.clone();
        fiche.libelle = libelle;
        fiche.cadre_par_defaut = cadre;
        fiche.cadre_selon_la_convention = None;
        Ok(MetierCree { code, copie_de: de.code.clone(), code_declare: de.code_declare.clone(), fiche })
    }

    /// Cadre ou non, dans toute convention.
    pub fn tenir_pour_cadre(&mut self, cadre: bool) {
        self.fiche.cadre_par_defaut = cadre;
        self.fiche.cadre_selon_la_convention = None;
    }

    /// LE CODE QU'IL FAIT DECLARER (S21.G00.40.005) devient `code`, un code de la nomenclature des emplois du spectacle
    /// (CCH-18 : « une valeur présente dans la table de nomenclature des emplois du spectacle », `fiche::CodeEmploiFt`) :
    /// l'annexe, le statut et les conventions dont la liste le nomme suivent ce code, tels que le catalogue les dit, sinon
    /// la nomenclature (un code hors catalogue n'est sur aucune liste : `Metier::nomme_par_l_annexe8`). Le reste de sa
    /// fiche, et le rattachement aux minima qu'il tient de sa copie, ne changent pas.
    pub fn declarer(&mut self, code: CodeEmploi) -> R<()> {
        if code.numero_cree().is_some() {
            return Err(socle::refus!(
                "Le code {} est celui d'un métier créé : un métier se déclare par un code de la nomenclature des emplois du spectacle de France Travail (DSN, S21.G00.40.005).",
                code
            ));
        }
        let m = code.du_catalogue_ou_de_la_nomenclature()?;
        let f = &mut self.fiche;
        match m.au_catalogue() {
            Some(c) => {
                f.annexe = c.annexe;
                f.annexe_nomenclature = c.annexe_nomenclature;
                f.statut = c.statut;
                f.conventions = c.conventions.clone();
                f.conventions_hors_liste = c.conventions_hors_liste.clone();
            }
            None => {
                f.annexe = m.annexe();
                f.annexe_nomenclature = None;
                f.statut = m.statut();
                f.conventions = None;
                f.conventions_hors_liste = None;
            }
        }
        self.code_declare = code;
        Ok(())
    }
}

/// La faute d'un fichier des metiers crees qui ne se lit pas : une phrase de l'onglet Metiers (`routes::metiers`), sans
/// chemin ; le fichier, `METIERS_CREES`.
const CREES_ILLISIBLES: &str = "Les métiers créés par la compagnie ne se lisent pas : leur fichier a été changé à la main ; le remettre tel que le logiciel l'a écrit.";

/// LA SEULE LECTURE de `config/metiers-crees.json` sous `racine` : aucun sans le fichier. Chaque code est celui d'un
/// metier cree, une seule fois ; le code qu'il fait declarer est de la nomenclature. Un fichier qui ne se lit pas, ou
/// qui ne tient pas cela, est `Classe::Illisible` : une lecture le passe et le dit (`routes::metiers`).
pub fn metiers_crees(racine: &Path) -> R<Vec<MetierCree>> {
    let illisible = |e: socle::echec::Echec| e.reclasser(socle::echec::Classe::Illisible);
    let Some(lus) = socle::forme::partagee::<MetiersCrees>(&METIERS_CREES.dans(racine), CREES_ILLISIBLES).map_err(illisible)? else { return Ok(Vec::new()) };
    let mut vus = std::collections::BTreeSet::new();
    for m in &lus.metiers {
        if m.code.numero_cree().is_none() || m.code_declare.numero_cree().is_some() || !vus.insert(m.code.clone()) {
            return Err(illisible(socle::refus!("{} (métier {})", CREES_ILLISIBLES, m.code)));
        }
    }
    Ok(lus.metiers.clone())
}

// ------------------------------------------------------------- l'interface

/// Un metier du catalogue tel qu'un deroulant le montre : son code, ses libelles, sa famille, son statut et son annexe,
/// sa PCS-ESE, s'il est cadre par defaut dans la convention de la compagnie, son niveau 3090, et les conventions dont le
/// deroulant le montre.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetierDuDeroulant {
    pub code: CodeEmploi,
    pub libelle: String,
    pub feminin: String,
    pub famille: FamilleDeMetier,
    pub statut: &'static str,
    #[serde(with = "annexe_en_nombre")]
    pub annexe: AnnexeChomage,
    pub pcs_ese: CodePcsEse,
    pub cadre_par_defaut: bool,
    pub niveau_3090: Option<CleDeGrille>,
    /// Les conventions dont le deroulant le montre ; aucune pour un metier cree : celui de la compagnie se montre
    /// quelle que soit sa convention.
    pub conventions: Vec<Idcc>,
    /// Le metier dont il est la copie, pour un metier que la compagnie a cree.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copie_de: Option<CodeEmploi>,
    /// Le code qu'il fait declarer (S21.G00.40.005), pour un metier que la compagnie a cree.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_declare: Option<CodeEmploi>,
}

/// Le catalogue du spectacle vivant puis les metiers que la compagnie a crees (`crees`, lus par `metiers_crees`), le cadre
/// de chaque metier lu dans la convention `idcc`, pour un deroulant, dans l'ordre des fichiers.
pub fn catalogue(idcc: Option<&Idcc>, crees: &[MetierCree]) -> R<Vec<MetierDuDeroulant>> {
    let du_deroulant = |code: &CodeEmploi, f: &FicheDuCatalogue, cree: Option<&MetierCree>| -> R<MetierDuDeroulant> {
        let m = Metier { code: code.clone(), code_declare: code.clone(), fiche: FicheDuMetier::Catalogue(f.clone()) };
        Ok(MetierDuDeroulant {
            cadre_par_defaut: m.cadre_par_defaut(idcc).ok_or_else(|| socle::refus!("Metier {} : le catalogue ne dit pas s'il est cadre.", repr(code.as_str())))?,
            code: code.clone(),
            libelle: f.libelle.clone(),
            feminin: f.feminin.clone(),
            famille: f.famille,
            statut: f.statut.as_str(),
            annexe: f.annexe,
            pcs_ese: f.pcs_ese.clone(),
            niveau_3090: f.classification_3090.as_ref().map(|c| c.niveau.clone()),
            conventions: match cree {
                Some(_) => Vec::new(),
                None => f.conventions.iter().flatten().cloned().collect(),
            },
            copie_de: cree.map(|c| c.copie_de.clone()),
            code_declare: cree.map(|c| c.code_declare.clone()),
        })
    };
    let mut out = base()?.emplois.iter().map(|(code, f)| du_deroulant(code, f, None)).collect::<R<Vec<_>>>()?;
    for c in crees {
        out.push(du_deroulant(&c.code, &c.fiche, Some(c))?);
    }
    Ok(out)
}

/// Le metier du catalogue qu'un intitule imprime nomme (un bulletin d'un logiciel precedent : « MUSICIEN »), compare
/// par `norme` (sans casse ni accents) a son libelle, a son feminin et a sa forme declaree. `(metier, feminin)` quand un
/// seul metier porte cet intitule ; `None` quand aucun, ou plusieurs (« SCRIPTE ») : on ne choisit pas entre eux.
pub fn par_intitule(intitule: &str, norme: impl Fn(&str) -> String) -> R<Option<(Metier, bool)>> {
    let cherche = norme(intitule);
    if cherche.is_empty() {
        return Ok(None);
    }
    let mut trouves: Vec<(CodeEmploi, bool)> = Vec::new();
    for (code, f) in &base()?.emplois {
        let (libelle, feminin, emploi) = (norme(&f.libelle), norme(&f.feminin), norme(&f.emploi));
        if [&libelle, &feminin, &emploi].contains(&&cherche) {
            trouves.push((code.clone(), feminin == cherche && libelle != cherche));
        }
    }
    match trouves.pop() {
        Some((code, feminin)) if trouves.is_empty() => Ok(Some((code.du_catalogue_ou_de_la_nomenclature()?, feminin))),
        _ => Ok(None),
    }
}

/// IDCC -> le nom court de la convention.
pub fn noms_des_conventions() -> R<BTreeMap<Idcc, String>> {
    Ok(base()?.conventions.clone())
}

/// Les familles du catalogue, dans l'ordre ou les
/// montrer : les artistes d'abord, parce que c'est le cas de presque toutes
/// les paies.
pub fn familles() -> R<Vec<FamilleDeMetier>> {
    use FamilleDeMetier as F;
    const ORDRE: [FamilleDeMetier; 11] = [
        F::Artistes, F::Musique, F::CirqueEtRue, F::MiseEnSceneEtCreation, F::RegieEtPlateau, F::Son, F::Lumiere,
        F::DecorEtConstruction, F::CostumeCoiffureMaquillage, F::ProductionEtAdministration, F::AudiovisuelDansLeSpectacle,
    ];
    let vues: std::collections::BTreeSet<FamilleDeMetier> = base()?.emplois.iter().map(|(_, f)| f.famille).collect();
    let mut out: Vec<FamilleDeMetier> = ORDRE.iter().copied().filter(|f| vues.contains(f)).collect();
    out.extend(vues.iter().copied().filter(|f| !ORDRE.contains(f)));
    Ok(out)
}

/// Ce qu'une fiche salarie declare (son code emploi, son statut, sa PCS-ESE ; vide : elle ne le dit pas), confronte au
/// catalogue. Des phrases d'avertissement, jamais d'erreur : la fiche peut avoir raison contre le catalogue.
pub fn controler_fiche(code: &CodeEmploiFt, statut: &str, pcs_ese: &str) -> R<Vec<String>> {
    let mut avis = Vec::new();
    let Some(m) = metier(code)? else {
        avis.push(format!("Code emploi France Travail {} inconnu de la nomenclature (S21.G00.40.005).", repr(code)));
        return Ok(avis);
    };
    let libelle = m.libelle();
    if !statut.is_empty() && statut != m.statut().as_str() {
        avis.push(format!(
            "Le code emploi {} ({}) est de l'annexe {}, donc {} ; la fiche dit {}.",
            code, libelle, m.annexe().numero(), m.statut(), repr(statut)
        ));
    }
    if let Some(attendu) = m.pcs_ese().filter(|_| !pcs_ese.is_empty()) {
        if pcs_ese.to_lowercase() != attendu.to_lowercase() {
            avis.push(format!("PCS-ESE {} pour {} ; le catalogue attend {} (fiche Insee de la rubrique).", pcs_ese, libelle, attendu));
        }
    }
    Ok(avis)
}
