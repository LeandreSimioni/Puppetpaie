//! LES ECARTS CONNUS DES ANCIENS LOGICIELS (`ecarts-connus-base.json`) : ce qu'un ancien logiciel a applique autrement
//! que le texte, que ce moteur a raison de ne pas reproduire, et que l'editeur a deja releve (Léandre, 23/11/2026 :
//! « quand quelqu'un arrive avec un défaut Coolpaie, on ne relève plus les défauts déjà relevés »). Chaque ligne dit le
//! logiciel, la cotisation, la part dont le taux differe, la periode, les deux taux exacts (ce qu'il a applique, ce que
//! dit le texte, avec la `source` qui fonde ce dernier) et l'explication, en une phrase, pour la compagnie.
//!
//! UNE COMPARAISON QUI RETOMBE EXACTEMENT SUR UNE LIGNE N'EST PAS UN ECART (`ecarts::confronter`) : le taux que le
//! bulletin imprime est celui que l'ancien logiciel a applique quand notre ligne porte celui du texte, et son montant est
//! le notre, chaque ligne au taux du texte refaite au taux applique, a l'arrondi pres. Elle ne fait pas d'incident, et la
//! carte de la paie importee dit l'explication et l'ecart chiffre (`import::mois`). Une comparaison qui ne colle pas
//! exactement reste un ecart : une ligne trop large cacherait un vrai defaut.

use std::sync::{Arc, OnceLock};

use serde::Deserialize;
use socle::echec::R;
use socle::fichiers::{forme_en_cache, CheminRelatif, FormeGardee};
use socle::grandeurs::{Montant, Taux};
use socle::mois::Mois;

use crate::ecarts::RegroupementDeComparaison;
use crate::importe::LogicielDUnBulletin;

/// LA PART D'UNE COTISATION DONT UN ECART CONNU DIT LE TAUX : celle de l'employeur ou celle du salarie. Elle s'ecrit
/// comme la grandeur du taux qui differe (`ecarts::CeQuiDiffere::code`) ; l'ecart connu explique aussi le montant de
/// cette part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
pub enum PartDuTaux {
    #[serde(rename = "taux_employeur")]
    Employeur,
    #[serde(rename = "taux_salarie")]
    Salarie,
}

/// UN ECART CONNU D'UN ANCIEN LOGICIEL : une ligne de `ecarts-connus-base.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EcartConnu {
    pub logiciel: LogicielDUnBulletin,
    pub cotisation: RegroupementDeComparaison,
    pub grandeur: PartDuTaux,
    /// Le premier et le dernier mois de paie ou il l'a applique.
    pub de: Mois,
    pub a: Mois,
    /// Le taux qu'il a applique, en pourcent.
    #[serde(deserialize_with = "socle::grandeurs::lu::court::deserialize")]
    pub applique: Taux,
    /// Le taux que dit le texte (`source`), en pourcent.
    #[serde(deserialize_with = "socle::grandeurs::lu::court::deserialize")]
    pub texte: Taux,
    /// Ce que la carte de la paie importee dit a la compagnie, en une phrase.
    pub explication: String,
    pub note: Option<String>,
    pub source: droit::source::Fondement,
}

impl EcartConnu {
    /// La ligne vaut-elle pour le bulletin du logiciel `logiciel`, de la paie du mois `mois`, sur la cotisation
    /// `cotisation` ?
    fn vaut(&self, logiciel: Option<&LogicielDUnBulletin>, mois: Mois, cotisation: &RegroupementDeComparaison) -> bool {
        logiciel == Some(&self.logiciel) && self.de <= mois && mois <= self.a && self.cotisation == *cotisation
    }
}

/// `ecarts-connus-base.json`, entier.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fichier {
    #[serde(deserialize_with = "socle::forme::non_lue")]
    quoi: socle::forme::NonLue,
    ecarts: Vec<EcartConnu>,
}

/// LA LECTURE DE `ecarts-connus-base.json`, la seule, une fois tant que le fichier ne change pas.
fn livres() -> R<Arc<Vec<EcartConnu>>> {
    static CACHE: OnceLock<FormeGardee<Vec<EcartConnu>>> = OnceLock::new();
    forme_en_cache(&CACHE, &const { CheminRelatif::du_moteur("ecarts-connus-base.json") }, "La table livrée des écarts connus des anciens logiciels ne se lit pas : le signaler au développeur.", |f: Fichier| Ok(f.ecarts))
}

/// Les ecarts connus du logiciel `logiciel` pour la paie du mois `mois` sur la cotisation `cotisation`.
pub fn de(logiciel: Option<&LogicielDUnBulletin>, mois: Mois, cotisation: &RegroupementDeComparaison) -> R<Vec<EcartConnu>> {
    Ok(livres()?.iter().filter(|e| e.vaut(logiciel, mois, cotisation)).cloned().collect())
}

/// UN ECART CONNU CONSTATE sur une paie importee : la ligne, et les deux montants de sa part (le notre, au taux du texte ;
/// celui du bulletin, au taux applique). Ce que la compagnie lit de son historique ; rien n'en part chez l'editeur.
#[derive(Debug, Clone)]
pub struct EcartConnuConstate {
    pub connu: EcartConnu,
    pub notre: Montant,
    pub leur: Montant,
}
