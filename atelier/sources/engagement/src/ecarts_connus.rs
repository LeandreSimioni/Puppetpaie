//! LES ECARTS CONNUS DES ANCIENS LOGICIELS (`ecarts-connus-base.json`) : ce qu'un ancien logiciel a applique autrement
//! que le texte, que ce moteur a raison de ne pas reproduire, et que l'editeur a deja releve (Léandre, 23/11/2026 :
//! « quand quelqu'un arrive avec un défaut Coolpaie, on ne relève plus les défauts déjà relevés »). Chaque ligne dit le
//! logiciel, la cotisation, ce qu'elle constate (`Constat` : le taux d'une part, deux taux exacts, ce qu'il a applique et
//! ce que dit le texte ; ou une cotisation que le bulletin imprime a tort et que ce moteur ne calcule pas), la periode,
//! le statut de la paie quand elle n'en vise qu'un, la `source` qui fonde le texte, et ce que la carte de la paie
//! importee dit a la compagnie (`pour`, `explication`).
//!
//! UNE COMPARAISON QUI RETOMBE EXACTEMENT SUR UNE LIGNE N'EST PAS UN ECART (`ecarts::confronter`). Un taux : celui que le
//! bulletin imprime est celui que l'ancien logiciel a applique quand notre ligne porte celui du texte, et son montant est
//! le notre, chaque ligne au taux du texte refaite au taux applique, a l'arrondi pres ; la paie se refait alors en
//! memoire, ces lignes-la au taux applique (`droit::contexte::TauxApplique`), et se compare tout de nouveau : ce qui en
//! retombe (l'assiette de la CSG, les nets, les totaux) ne sonne pas (`ecarts::comparer`). Une cotisation imprimee a
//! tort : le bulletin l'imprime, ce moteur ne la calcule pas, et le total des cotisations de sa part la compte. Elle ne
//! fait pas d'incident, et la carte de la paie importee dit ce que la compagnie a cotise de moins (ou de plus) que le
//! texte, et pour quoi (`EcartConnuConstate::phrase`, `import::mois`). Une comparaison qui ne colle pas exactement reste
//! un ecart : une ligne trop large cacherait un vrai defaut.

use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use socle::echec::R;
use socle::fichiers::{forme_en_cache, CheminRelatif, FormeGardee};
use socle::grandeurs::{Montant, Taux};
use socle::mois::Mois;

use crate::ecarts::{CeQuiDiffere, RegroupementDeComparaison};
use crate::importe::LogicielDUnBulletin;

/// CE QU'UNE LIGNE DES ECARTS CONNUS CONSTATE : le taux d'une part (`grandeur`, `CeQuiDiffere::TauxEmployeur` ou
/// `TauxSalarie`), celui que l'ancien logiciel a applique et celui du texte ; ou une cotisation que le bulletin imprime a
/// tort et que ce moteur ne calcule pas (`CeQuiDiffere::AbsenteDuCalcul`, sans taux), avec ce qu'elle fait a ce que la
/// paie a cotise (`effet`). Il s'ecrit `{"grandeur", "applique", "texte"}`, les taux seulement pour un taux, ou
/// `{"grandeur", "effet"}` (`ConstatEcrit`, la porte).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ConstatEcrit", into = "ConstatEcrit")]
pub enum Constat {
    Taux { grandeur: CeQuiDiffere, applique: Taux, texte: Taux },
    AbsenteDuCalcul { effet: EffetSurLaPaie },
}

/// CE QU'UNE COTISATION IMPRIMEE A TORT FAIT A CE QUE LA PAIE A COTISE, que la ligne connue dit et fonde (sa `source`) :
/// une reduction ou une exoneration (`Deduction` : la mention « Exonérations et allègements de cotisations » de
/// l'arrete du 25/02/2016, art. 4, la somme des reductions, deduite de ce qui est du). Ce que le bulletin imprime
/// compte de moins, quel que soit le signe que l'ancien logiciel lui donne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffetSurLaPaie {
    Deduction,
}

impl EffetSurLaPaie {
    /// Ce que les montants `imprimes` ont fait cotiser de plus (positif) ou de moins (negatif) : leur somme, signes
    /// compris (une regularisation imprimee en sens contraire s'en retranche), puis son effet.
    pub fn ecart(self, imprimes: &mut dyn Iterator<Item = Montant>) -> Montant {
        let somme = imprimes.fold(Montant::ZERO, |a, m| a + m);
        match self {
            EffetSurLaPaie::Deduction => -somme.abs(),
        }
    }
}

/// La forme ecrite d'un `Constat`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstatEcrit {
    grandeur: CeQuiDiffere,
    /// Le taux qu'il a applique, en pourcent.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "socle::grandeurs::ecrit::nombre::option")]
    applique: Option<Taux>,
    /// Le taux que dit le texte (`source`), en pourcent.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "socle::grandeurs::ecrit::nombre::option")]
    texte: Option<Taux>,
    /// Ce que la cotisation imprimee a tort fait a la paie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    effet: Option<EffetSurLaPaie>,
}

impl TryFrom<ConstatEcrit> for Constat {
    type Error = String;
    fn try_from(e: ConstatEcrit) -> Result<Constat, String> {
        match (e.grandeur, e.applique, e.texte, e.effet) {
            (CeQuiDiffere::TauxEmployeur | CeQuiDiffere::TauxSalarie, Some(applique), Some(texte), None) => Ok(Constat::Taux { grandeur: e.grandeur, applique, texte }),
            (CeQuiDiffere::AbsenteDuCalcul, None, None, Some(effet)) => Ok(Constat::AbsenteDuCalcul { effet }),
            (CeQuiDiffere::TauxEmployeur | CeQuiDiffere::TauxSalarie, _, _, _) => Err(format!("un écart connu sur le {} dit le taux appliqué et celui du texte, et rien d'autre", e.grandeur.dite())),
            (CeQuiDiffere::AbsenteDuCalcul, _, _, _) => Err("un écart connu sur une cotisation absente du calcul dit son effet (une déduction), sans taux".to_string()),
            (g, _, _, _) => Err(format!("un écart connu constate un taux, ou une cotisation absente du calcul, pas « {} »", g.code())),
        }
    }
}

impl From<Constat> for ConstatEcrit {
    fn from(c: Constat) -> ConstatEcrit {
        match c {
            Constat::Taux { grandeur, applique, texte } => ConstatEcrit { grandeur, applique: Some(applique), texte: Some(texte), effet: None },
            Constat::AbsenteDuCalcul { effet } => ConstatEcrit { grandeur: CeQuiDiffere::AbsenteDuCalcul, applique: None, texte: None, effet: Some(effet) },
        }
    }
}

/// UN ECART CONNU D'UN ANCIEN LOGICIEL : une ligne de `ecarts-connus-base.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EcartConnu {
    pub logiciel: LogicielDUnBulletin,
    pub cotisation: RegroupementDeComparaison,
    pub constate: Constat,
    /// Le premier et le dernier mois de paie ou il l'a applique.
    pub de: Mois,
    pub a: Mois,
    /// Le statut de la paie que la ligne vise (la reduction generale, que la loi n'ouvre pas aux artistes) ; vide : toute
    /// paie.
    pub statut: Option<droit::fiche::Statut>,
    /// Ce que la cotisation finance, tel que la carte le dit (« la prévoyance de ses salariés »).
    pub pour: String,
    /// Pourquoi l'ancien logiciel l'a fait, en une phrase courte, pour la carte de la paie importee.
    pub explication: String,
    pub note: Option<String>,
    pub source: droit::source::Fondement,
}

impl EcartConnu {
    /// La ligne vaut-elle pour le bulletin du logiciel `logiciel`, de la paie du mois `mois` et du statut `statut`, sur la
    /// cotisation `cotisation` ?
    fn vaut(&self, logiciel: Option<&LogicielDUnBulletin>, mois: Mois, statut: Option<droit::fiche::Statut>, cotisation: &RegroupementDeComparaison) -> bool {
        logiciel == Some(&self.logiciel) && self.de <= mois && mois <= self.a && self.statut.is_none_or(|s| statut == Some(s)) && self.cotisation == *cotisation
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

/// Les ecarts connus du logiciel `logiciel` pour la paie du mois `mois`, du statut `statut`, sur la cotisation
/// `cotisation`.
pub fn de(logiciel: Option<&LogicielDUnBulletin>, mois: Mois, statut: Option<droit::fiche::Statut>, cotisation: &RegroupementDeComparaison) -> R<Vec<EcartConnu>> {
    Ok(livres()?.iter().filter(|e| e.vaut(logiciel, mois, statut, cotisation)).cloned().collect())
}

/// UN ECART CONNU CONSTATE sur une paie importee, et sa note dans `journal/ecarts-connus.json` (`import::ecarts`, la
/// seule lecture), rangee sous l'empreinte de sa paie (`ecarts::empreinte`) : ce que la ligne connue dit, et ce que la
/// compagnie et le salarie ont cotise de plus (positif) ou de moins (negatif) que le texte, en euros, sur ce bulletin.
/// Ce que la compagnie lit de son historique ; rien n'en part chez l'editeur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EcartConnuConstate {
    pub logiciel: LogicielDUnBulletin,
    pub cotisation: RegroupementDeComparaison,
    pub constate: Constat,
    pub pour: String,
    pub explication: String,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub ecart_employeur: Montant,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub ecart_salarie: Montant,
}

impl EcartConnuConstate {
    /// Celui que la ligne `connu` constate sur une paie : ce que la compagnie (`employeur`) et le salarie (`salarie`) ont
    /// cotise de plus ou de moins que le texte.
    pub fn de(connu: &EcartConnu, employeur: Montant, salarie: Montant) -> EcartConnuConstate {
        EcartConnuConstate {
            logiciel: connu.logiciel.clone(),
            cotisation: connu.cotisation.clone(),
            constate: connu.constate,
            pour: connu.pour.clone(),
            explication: connu.explication.clone(),
            ecart_employeur: employeur,
            ecart_salarie: salarie,
        }
    }

    /// CE QUE LA CARTE DE LA PAIE IMPORTEE EN DIT, en une phrase, chiffres a l'appui (AGENTS, « Le logiciel est neutre ») :
    /// qui a cotise combien de moins (ou de plus) que le texte, pour quoi, aux deux taux pour un taux, et pourquoi.
    /// « La compagnie a cotisé 3,12 € de moins que le texte pour la prévoyance de ses salariés (Audiens, non cadres), à
    /// 1,04 % au lieu de 1,11 % : … »
    pub fn phrase(&self) -> String {
        let taux = match self.constate {
            Constat::Taux { applique, texte, .. } => format!(", à {} % au lieu de {} %", socle::texte::virgule(applique), socle::texte::virgule(texte)),
            Constat::AbsenteDuCalcul { .. } => String::new(),
        };
        let dit = |qui: &str, x: Montant| {
            let sens = if x < Montant::ZERO { "moins" } else { "plus" };
            format!("{} a cotisé {} de {} que le texte", qui, socle::texte::euros(x.abs()), sens)
        };
        let qui: Vec<String> = [("La compagnie", self.ecart_employeur), ("Le salarié", self.ecart_salarie)].into_iter().filter(|(_, x)| !x.est_nul()).map(|(q, x)| dit(q, x)).collect();
        match qui.is_empty() {
            true => self.explication.clone(),
            false => format!("{} pour {}{} : {}", qui.join(", et "), self.pour, taux, self.explication),
        }
    }
}
