//! Les metiers du spectacle et la fiche telle qu'elle vaut pour un engagement (`emploi_de`, `emploi_du_mois`).
//!
//! Deux fichiers livres avec le code, chacun lu par sa forme, une fois tant qu'il ne change pas : `emplois-base.json`
//! (le catalogue, `BaseEmplois`) et `nomenclature-emplois-ft.json` (les codes France Travail, `Nomenclature`). Un
//! metier se lit par `metier`, qui rend un `Metier` : celui du catalogue, sinon celui de la nomenclature.
//!
//! De l'interface (l'accueil, `routes::ouverture`) : `catalogue`, `familles`,
//! `noms_des_conventions` et `controler_fiche`.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use socle::fichiers::{forme_en_cache, repr, FormeGardee};
use socle::echec::R;
use socle::date::Jour;

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
/// (sa `note`) : le metier qu'une carte choisit, la cle du catalogue, celui d'un deroulant. La liste est la nomenclature
/// livree : un code qu'elle n'a pas se refuse. Saisi, il se range sans ses blancs de bord et en capitales.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CodeEmploi(String);

impl CodeEmploi {
    /// Le code `texte`, s'il est de la nomenclature.
    pub fn lire(texte: &str) -> R<CodeEmploi> {
        if nomenclature()?.emplois.contains_key(texte) {
            return Ok(CodeEmploi(texte.to_string()));
        }
        Err(socle::refus!("Métier {} inconnu : ni dans le catalogue des emplois, ni dans la nomenclature de France Travail.", repr(texte)))
    }

    /// Le texte saisi `texte` dans sa forme, avant d'etre lu : sans ses blancs de bord, en capitales.
    pub fn rangee(texte: &str) -> String {
        socle::texte::en_capitales(texte)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// SON METIER : celui du catalogue, sinon celui de la nomenclature, qui a tous les codes.
    pub fn metier(&self) -> R<Metier> {
        if let Some((_, f)) = base()?.emplois.iter().find(|(c, _)| c == self) {
            return Ok(Metier { code: self.clone(), fiche: FicheDuMetier::Catalogue(f.clone()) });
        }
        let n = nomenclature()?.emplois.get(&self.0).cloned().ok_or_else(|| socle::echec::livree("nomenclature-emplois-ft.json", &self.0))?;
        Ok(Metier { code: self.clone(), fiche: FicheDuMetier::Nomenclature(n) })
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
    pub famille: String,
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
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nomenclature {
    pub quoi: String,
    pub note: String,
    pub emplois: BTreeMap<String, EmploiFt>,
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
    forme_en_cache(&CACHE, "emplois-base.json", CATALOGUE_ILLISIBLE, Ok)
}

/// LA SEULE LECTURE DE `nomenclature-emplois-ft.json`.
pub fn nomenclature() -> R<Arc<Nomenclature>> {
    static CACHE: OnceLock<FormeGardee<Nomenclature>> = OnceLock::new();
    forme_en_cache(&CACHE, "nomenclature-emplois-ft.json", "La nomenclature des emplois France Travail livrée avec le logiciel ne se lit pas : le signaler au développeur.", Ok)
}

/// Premiere lettre en majuscule, le reste en minuscules.
fn capitaliser(s: &str) -> String {
    s.chars().take(1).flat_map(char::to_uppercase).chain(s.chars().skip(1).flat_map(char::to_lowercase)).collect()
}

/// Un metier, par son code : celui du catalogue, ou, hors catalogue, celui de la nomenclature France Travail (un code
/// et une annexe ; ni PCS, ni niveau, ni minima).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Metier {
    pub code: CodeEmploi,
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

    /// La famille du catalogue ; « Hors catalogue » sinon.
    pub fn famille(&self) -> &str {
        match self.au_catalogue() {
            Some(f) => &f.famille,
            None => "Hors catalogue",
        }
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
            .ok_or_else(|| socle::echec::livree("clauses-conventionnelles-base.json", &format!("conventions.3090.categories.par_niveau_3090.{}", c.niveau)))?;
        if [crate::clauses::CADRE, crate::clauses::AGENT_DE_MAITRISE].contains(categorie) {
            return Ok(Some(StatutConventionnel::AgentDeMaitrise));
        }
        Ok(Some(if c.grille == crate::minima::ADMINISTRATIF { StatutConventionnel::Employe06 } else { StatutConventionnel::Employe07 }))
    }
}

/// Le metier que nomme le code emploi declare `code` (S21.G00.40.005), tel qu'il est ecrit, s'il est un code de la
/// nomenclature (`CodeEmploi::metier`). `None` sinon : un emploi permanent (« 999SPT »), un code d'une autre table.
pub fn metier(code: &CodeEmploiFt) -> R<Option<Metier>> {
    if !nomenclature()?.emplois.contains_key(code.as_ref()) {
        return Ok(None);
    }
    CodeEmploi::lire(code)?.metier().map(Some)
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

/// Les sept champs declares pour ce metier (`emploi_affichage` compris).
/// `cadre` a `None` prend le defaut du catalogue dans la convention `idcc` (`Metier::cadre_par_defaut`).
pub fn resoudre(code: &CodeEmploi, cadre: Option<bool>, feminin: bool, idcc: Option<&Idcc>) -> R<ChampsDuMetier> {
    let m = code.metier()?;
    let est_cadre = match cadre.or_else(|| m.cadre_par_defaut(idcc)) {
        Some(c) => c,
        None => return Err(socle::refus!("Le métier {} est hors catalogue : la fiche doit dire s'il est cadre.", repr(code.as_str()))),
    };
    let libelle = if feminin { m.feminin() } else { m.libelle() };
    Ok(ChampsDuMetier {
        emploi: m.emploi().to_string(),
        emploi_affichage: libelle,
        pcs_ese: m.pcs_ese().cloned(),
        code_emploi_ft: Some(CodeEmploiFt::from(&m.code)),
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
/// (`debut`, `fin`) : lue a la fin de la periode. Un permanent passe cadre, change de poste, obtient ou perd sa RQTH en
/// cours de contrat : la paie et la DSN du mois ou cela prend effet le disent.
///
/// UN CHANGEMENT DANS LA PERIODE REFUSE (`UNE_VALEUR_PAR_MOIS`) : une paie se calcule sur une seule pile de bareme
/// (regle 6). La paie d'un mois ou ces valeurs changent se decoupe en periodes elementaires aux jours du changement
/// (`jours_de_changement`, `periodes::bornes`, norme § 2.1.3.2, p. 28), et chaque periode lit sa fiche ici, sur ses
/// bornes : le refus ne garde que l'appel sur une periode qui n'aurait pas ete coupee. Le statut BOETH, qui ne change
/// pas la paie, peut changer en cours de mois (40.072 se renseigne « pour le contrat concerne dans la periode
/// declaree », p. 211 : celui de la fin).
pub fn emploi_du_mois(ctx: &Contexte, cle: &IdSalarie, debut: &Jour, fin: &Jour) -> R<FicheSalarie> {
    let au_debut = ctx.salarie_au(cle, debut)?;
    let a_la_fin = ctx.salarie_au(cle, fin)?;
    for ((c, avant), (_, apres)) in valeurs_par_mois(&au_debut).into_iter().zip(valeurs_par_mois(&a_la_fin)) {
        if avant == apres {
            continue;
        }
        let c = CaseDeFiche::from(c);
        let le = ctx.salarie_brut(cle).into_iter().flat_map(|b| b.depuis(c)).flatten().filter(|j| j > debut && j <= fin).last();
        let premier = Jour { jour: 1, ..*debut };
        let suivant = Jour { jour: 1, ..premier.plus_jours(31)? };
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

/// Les jours, apres `debut` et au plus tard `fin` (la periode payee d'un contrat paye au mois), ou la fiche `cle` change
/// une valeur qui choisit la pile du bareme ou que le bloc 40 porte avec une rubrique en bloc 41 (`UNE_VALEUR_PAR_MOIS`) :
/// le `depuis` d'une de ces valeurs, quand la fiche de la veille et celle du jour different. C'est la que la paie du mois
/// se decoupe en periodes elementaires (`periodes::bornes` ; norme DSN 2026.1, § 2.1.3.2, p. 28 : « passage de non
/// cadre a cadre en cours de periode de paie »). Vide : le mois se paie d'un seul tenant.
pub fn jours_de_changement(ctx: &Contexte, cle: &IdSalarie, debut: &Jour, fin: &Jour) -> R<Vec<Jour>> {
    let Some(brute) = ctx.salarie_brut(cle) else {
        return Err(socle::refus!("La fiche salarié « {} » n'existe pas : sa paie au mois ne se découpe pas.", cle));
    };
    let mut jours: Vec<Jour> = Vec::new();
    for c in UNE_VALEUR_PAR_MOIS {
        jours.extend(brute.depuis(c.into()).into_iter().flatten().filter(|j| j > debut && j <= fin));
    }
    jours.sort();
    jours.dedup();
    let mut out = Vec::new();
    for j in jours {
        let (avant, apres) = (brute.au(&j.plus_jours(-1)?), brute.au(&j));
        if !avant.memes_cases(&apres, &UNE_VALEUR_PAR_MOIS) {
            out.push(j);
        }
    }
    Ok(out)
}

/// Ce que `emploi_de` lit d'un engagement.
#[derive(Debug, Clone, Default)]
pub struct EmploiEngagement {
    pub emploi: Option<String>,
    /// Le libelle lisible que l'engagement a resolu.
    pub emploi_affichage: Option<String>,
    pub pcs_ese: Option<CodePcsEse>,
    pub code_emploi_ft: Option<CodeEmploiFt>,
    pub statut: Option<Statut>,
    pub statut_conventionnel: Option<StatutConventionnel>,
    pub statut_categoriel_rc: Option<StatutCategorielRc>,
}

/// La fiche salarie `sal` (lue a la date de la periode) telle qu'elle vaut POUR CET ENGAGEMENT : ce que l'engagement
/// dit l'emporte ; le reste vient de la fiche.
pub fn emploi_de(sal: &FicheSalarie, eng: &EmploiEngagement) -> R<FicheSalarie> {
    let mut out = sal.clone();
    let dit = |v: &Option<String>| v.as_ref().filter(|s| !s.is_empty()).cloned();
    let pcs_ese = eng.pcs_ese.clone();
    let affichage = dit(&eng.emploi_affichage);
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

// ------------------------------------------------------------- l'interface

/// Un metier du catalogue tel qu'un deroulant le montre : son code, ses libelles, sa famille, son statut et son annexe,
/// sa PCS-ESE, s'il est cadre par defaut dans la convention de la compagnie, son niveau 3090, et les conventions dont le
/// deroulant le montre.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetierDuDeroulant {
    pub code: CodeEmploi,
    pub libelle: String,
    pub feminin: String,
    pub famille: String,
    pub statut: &'static str,
    #[serde(with = "annexe_en_nombre")]
    pub annexe: AnnexeChomage,
    pub pcs_ese: CodePcsEse,
    pub cadre_par_defaut: bool,
    pub niveau_3090: Option<CleDeGrille>,
    pub conventions: Vec<Idcc>,
}

/// Le catalogue du spectacle vivant, le defaut cadre de chaque metier lu dans la convention `idcc`, pour un
/// deroulant, dans l'ordre du fichier.
pub fn catalogue(idcc: Option<&Idcc>) -> R<Vec<MetierDuDeroulant>> {
    base()?
        .emplois
        .iter()
        .map(|(code, f)| {
            let m = Metier { code: code.clone(), fiche: FicheDuMetier::Catalogue(f.clone()) };
            Ok(MetierDuDeroulant {
                cadre_par_defaut: m.cadre_par_defaut(idcc).ok_or_else(|| socle::refus!("Metier {} : le catalogue ne dit pas s'il est cadre.", repr(code.as_str())))?,
                code: code.clone(),
                libelle: f.libelle.clone(),
                feminin: f.feminin.clone(),
                famille: f.famille.clone(),
                statut: f.statut.as_str(),
                annexe: f.annexe,
                pcs_ese: f.pcs_ese.clone(),
                niveau_3090: f.classification_3090.as_ref().map(|c| c.niveau.clone()),
                conventions: f.conventions.iter().flatten().cloned().collect(),
            })
        })
        .collect()
}

/// Le metier du catalogue qu'un intitule imprime nomme (un bulletin d'un logiciel precedent : « MUSICIEN »), compare
/// par `norme` (sans casse ni accents) a son libelle, a son feminin et a sa forme declaree. `(code, feminin)` quand un
/// seul metier porte cet intitule ; `None` quand aucun, ou plusieurs (« SCRIPTE ») : on ne choisit pas entre eux.
pub fn par_intitule(intitule: &str, norme: impl Fn(&str) -> String) -> R<Option<(CodeEmploi, bool)>> {
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
    Ok(if trouves.len() == 1 { trouves.pop() } else { None })
}

/// IDCC -> le nom court de la convention.
pub fn noms_des_conventions() -> R<BTreeMap<Idcc, String>> {
    Ok(base()?.conventions.clone())
}

/// Les familles du catalogue, dans l'ordre ou les
/// montrer : les artistes d'abord, parce que c'est le cas de presque toutes
/// les paies.
pub fn familles() -> R<Vec<String>> {
    const ORDRE: [&str; 11] = [
        "Artistes", "Musique", "Cirque et rue", "Mise en scène et création", "Régie et plateau", "Son", "Lumière",
        "Décor et construction", "Costume, coiffure, maquillage", "Production et administration", "Audiovisuel dans le spectacle",
    ];
    let vues: std::collections::BTreeSet<String> = base()?.emplois.iter().map(|(_, f)| f.famille.clone()).collect();
    let mut out: Vec<String> = ORDRE.iter().filter(|f| vues.contains(**f)).map(|f| f.to_string()).collect();
    out.extend(vues.iter().filter(|f| !ORDRE.contains(&f.as_str())).cloned());
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
