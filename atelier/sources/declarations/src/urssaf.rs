//! LES ORGANISMES DE RECOUVREMENT (`urssaf-dpae-base.json`, extrait par `moteur-atelier extraire urssaf`) : le code a trois
//! chiffres que la DPAE attend en C_URSSAF (positions 133-136, annexe II.1 de la norme), et l'organisme du territoire
//! de chaque departement. `base` est sa seule lecture.

use std::collections::BTreeMap;

use serde::Deserialize;

use std::sync::{Arc, OnceLock};

use socle::fichiers::{forme_en_cache, FormeGardee};
use socle::echec::R;

/// `urssaf-dpae-base.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseUrssaf {
    pub quoi: String,
    pub note: String,
    pub organismes: Vec<Organisme>,
    /// L'organisme de chaque ancienne region (2015) : ce d'ou l'extraction tire `par_departement`.
    #[serde(rename = "par_ancienne_region")]
    _par_ancienne_region: BTreeMap<String, String>,
    /// L'organisme du territoire de chaque departement (deux chiffres, trois outre-mer, 2A et 2B par 20). La cle est le
    /// departement tel que la table l'ecrit (« 971 », trois chiffres outre-mer), que `DepartementNaissance` ne porte pas.
    par_departement: BTreeMap<String, CodeUrssafDpae>,
}

/// Un organisme de recouvrement : son code, sa codification, son nom, son SIRET et son code emetteur.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Organisme {
    pub code: CodeUrssafDpae,
    pub codification: String,
    pub nom: String,
    pub siret: droit::fiche::Siret,
    pub emetteur: String,
}

/// LE CODE D'UN ORGANISME DE RECOUVREMENT, celui que la DPAE attend en C_URSSAF : trois chiffres (positions 133-136,
/// annexe II.1 du Format 120, `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`). Les codes qui existent sont ceux
/// de la table livree (`base`, `existe`). Il s'ecrit et se trie comme son texte.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CodeUrssafDpae(String);

impl CodeUrssafDpae {
    /// Le code `texte`, ou ce qui le refuse.
    pub fn lire(texte: &str) -> R<CodeUrssafDpae> {
        if texte.len() == 3 && texte.bytes().all(|b| b.is_ascii_digit()) {
            Ok(CodeUrssafDpae(texte.to_string()))
        } else {
            Err(socle::refus!("{} n'est pas un code Urssaf de la DPAE : trois chiffres (C_URSSAF, Format 120).", socle::fichiers::repr(texte)))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CodeUrssafDpae {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CodeUrssafDpae {
    type Error = String;
    fn try_from(texte: String) -> Result<CodeUrssafDpae, String> {
        CodeUrssafDpae::lire(&texte).map_err(|e| e.phrase)
    }
}

impl From<CodeUrssafDpae> for String {
    fn from(c: CodeUrssafDpae) -> String {
        c.0
    }
}

/// La table livree, lue par sa forme.
pub fn base() -> R<Arc<BaseUrssaf>> {
    static CACHE: OnceLock<FormeGardee<BaseUrssaf>> = OnceLock::new();
    forme_en_cache(&CACHE, "urssaf-dpae-base.json", "La table des Urssaf livrée avec le logiciel ne se lit pas : le signaler au développeur.", Ok)
}

impl BaseUrssaf {
    /// Ce code est-il celui d'un organisme connu ?
    pub fn existe(&self, code: &CodeUrssafDpae) -> bool {
        self.organismes.iter().any(|o| o.code == *code)
    }

    /// L'organisme du TERRITOIRE de ce departement, s'il est connu. Une suggestion, pas une decision.
    pub fn du_departement(&self, dep: &str) -> Option<CodeUrssafDpae> {
        let d = socle::texte::strip(dep).to_uppercase();
        let lu = |k: &str| self.par_departement.get(k).cloned();
        lu(&d).or_else(|| if d == "2A" || d == "2B" { lu("20") } else { None })
    }

    /// Idem, depuis un code postal francais. L'outre-mer se lit sur TROIS chiffres (97139 -> 971), la metropole sur deux.
    pub fn du_code_postal(&self, cp: &str) -> Option<CodeUrssafDpae> {
        let v: Vec<char> = cp.chars().filter(|c| c.is_ascii_digit()).collect();
        if v.len() < 2 {
            return None;
        }
        let n = if v[0] == '9' && (v[1] == '7' || v[1] == '8') { 3 } else { 2 };
        self.du_departement(&v.iter().take(n).collect::<String>())
    }
}
