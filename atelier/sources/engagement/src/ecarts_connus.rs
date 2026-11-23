//! LES ECARTS CONNUS DES ANCIENS LOGICIELS (`ecarts-connus-base.json`) : ce qu'un ancien logiciel a applique autrement
//! que le texte, que ce moteur a raison de ne pas reproduire, et que l'editeur a deja releve (Léandre, 23/11/2026 :
//! « quand quelqu'un arrive avec un défaut Coolpaie, on ne relève plus les défauts déjà relevés »). Chaque ligne dit le
//! logiciel, la cotisation, la part dont le taux differe (`ecarts::CeQuiDiffere`, un taux), la periode, les deux taux
//! exacts (ce qu'il a applique, ce que dit le texte, avec la `source` qui fonde ce dernier) et l'explication, en une
//! phrase, pour la compagnie.
//!
//! UNE COMPARAISON QUI RETOMBE EXACTEMENT SUR UNE LIGNE N'EST PAS UN ECART (`ecarts::confronter`) : le taux que le
//! bulletin imprime est celui que l'ancien logiciel a applique quand notre ligne porte celui du texte, et son montant est
//! le notre, chaque ligne au taux du texte refaite au taux applique, a l'arrondi pres. La paie se refait alors en
//! memoire, ces lignes-la au taux applique (`droit::contexte::TauxApplique`), et se compare tout de nouveau : ce qui en
//! retombe (l'assiette de la CSG, les nets, les totaux) ne sonne pas (`ecarts::comparer`). Elle ne fait pas d'incident,
//! et la carte de la paie importee dit l'explication et l'ecart chiffre (`import::mois`). Une comparaison qui ne colle pas
//! exactement reste un ecart : une ligne trop large cacherait un vrai defaut.

use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use socle::echec::R;
use socle::fichiers::{forme_en_cache, CheminRelatif, EmpreinteCourte, FormeGardee};
use socle::grandeurs::{Montant, Taux};
use socle::mois::Mois;

use crate::ecarts::{CeQuiDiffere, RegroupementDeComparaison};
use crate::importe::LogicielDUnBulletin;

/// UN ECART CONNU D'UN ANCIEN LOGICIEL : une ligne de `ecarts-connus-base.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EcartConnu {
    pub logiciel: LogicielDUnBulletin,
    pub cotisation: RegroupementDeComparaison,
    /// Le taux qui differe, et dont la ligne explique aussi le montant de la part : `taux_employeur` ou `taux_salarie`
    /// (`taux_de_la_part`, la porte).
    pub grandeur: CeQuiDiffere,
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

/// LA PORTE D'UNE LIGNE : sa grandeur est le taux d'une part (`CeQuiDiffere::TauxEmployeur`, `TauxSalarie`) ; une autre
/// se refuse, la table entiere avec elle.
fn taux_de_la_part(e: &EcartConnu) -> R<()> {
    match e.grandeur {
        CeQuiDiffere::TauxEmployeur | CeQuiDiffere::TauxSalarie => Ok(()),
        g => Err(socle::refus!("La table livrée des écarts connus nomme une grandeur qui n'est pas le taux d'une part ({}) : le signaler au développeur.", g.code())),
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
    forme_en_cache(&CACHE, &const { CheminRelatif::du_moteur("ecarts-connus-base.json") }, "La table livrée des écarts connus des anciens logiciels ne se lit pas : le signaler au développeur.", |f: Fichier| {
        for e in &f.ecarts {
            taux_de_la_part(e)?;
        }
        Ok(f.ecarts)
    })
}

/// Les ecarts connus du logiciel `logiciel` pour la paie du mois `mois` sur la cotisation `cotisation`.
pub fn de(logiciel: Option<&LogicielDUnBulletin>, mois: Mois, cotisation: &RegroupementDeComparaison) -> R<Vec<EcartConnu>> {
    Ok(livres()?.iter().filter(|e| e.vaut(logiciel, mois, cotisation)).cloned().collect())
}

/// UN ECART CONNU CONSTATE sur une paie importee, et sa note dans `journal/ecarts-connus.json` (`import::ecarts`, la
/// seule lecture) : la paie (son empreinte, `ecarts::empreinte`), ce que la ligne connue dit, et les deux montants de sa
/// part, le notre (au taux du texte) et celui du bulletin (au taux applique). Ce que la compagnie lit de son historique ;
/// rien n'en part chez l'editeur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EcartConnuConstate {
    pub paie: EmpreinteCourte,
    pub logiciel: LogicielDUnBulletin,
    pub cotisation: RegroupementDeComparaison,
    pub grandeur: CeQuiDiffere,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub applique: Taux,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub texte: Taux,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub notre: Montant,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub leur: Montant,
    pub explication: String,
}

impl EcartConnuConstate {
    /// Celui que la ligne `connu` constate sur la paie `paie`, aux montants `notre` et `leur`.
    pub fn de(paie: &EmpreinteCourte, connu: &EcartConnu, notre: Montant, leur: Montant) -> EcartConnuConstate {
        EcartConnuConstate {
            paie: paie.clone(),
            logiciel: connu.logiciel.clone(),
            cotisation: connu.cotisation.clone(),
            grandeur: connu.grandeur,
            applique: connu.applique,
            texte: connu.texte,
            notre,
            leur,
            explication: connu.explication.clone(),
        }
    }

    /// Ce que la carte de la paie importee en dit : l'explication, et l'ecart chiffre de la part.
    pub fn phrase(&self) -> String {
        let part = match self.grandeur {
            CeQuiDiffere::TauxSalarie => "part salariale",
            _ => "part employeur",
        };
        format!(
            "{} Sur ce bulletin ({}) : {} au lieu de {}, soit {} d'écart.",
            self.explication,
            part,
            socle::texte::euros(self.leur),
            socle::texte::euros(self.notre),
            socle::texte::euros((self.leur - self.notre).abs())
        )
    }
}
