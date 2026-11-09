//! CE QUE LES MODULES PAS ENCORE BRANCHES SUR L'ENVELOPPE LISENT ET ECRIVENT EN JSON : leur charge (`lire_charge`), leur
//! reponse (`en_json`) et les mots qu'ils
//! tirent d'une valeur. Une route branchee n'en prend rien : elle lit sa charge en sa forme (`Charge::lire`) et rend la
//! sienne (`enveloppe::Dite`). Ce module s'efface avec le dernier module qui s'en sert.

use serde_json::Value;

use crate::valeur::valeur_en_texte;
use socle::echec::{erreur, Classe, Echec, R};

/// La charge d'une route pas encore branchee, lue en sa forme comme `Charge::lire` : une cle que la forme ne connait
/// pas, une cle qui manque, une valeur d'une autre forme la refusent (`MalFormee`), la cle fautive a part ; rien ne
/// s'ecrit.
pub(crate) fn lire_charge<T: serde::de::DeserializeOwned>(charge: &Value) -> R<T> {
    socle::forme::lire(charge, "La demande est mal formée.").map_err(|m| erreur(Classe::MalFormee, m))
}

/// La reponse d'une route pas encore branchee, en JSON : chacune est une structure de son module, que `serde_json`
/// ecrit toujours.
pub(crate) fn en_json(v: impl serde::Serialize) -> R<Value> {
    serde_json::to_value(v).map_err(|e| Echec::from(e.to_string()))
}

/// Ce qu'une route pas encore branchee a rendu, pour la porte.
pub(super) fn brute(rendu: R<Value>) -> R<super::Rendu> {
    rendu.map(super::Rendu::brut)
}

/// Une valeur rendue, en mots dans une phrase : rien pour une valeur absente (jamais « None »).
pub(crate) fn mot(v: &Value) -> String {
    if v.is_null() { String::new() } else { valeur_en_texte(v) }
}


/// Le nom d'un salarie tel qu'un ecran le montre (`ChampsSalarie::nomme`), vide pour une fiche sans nom.
pub(crate) fn nom_a_l_ecran(sal: &droit::fiche::ChampsSalarie) -> String {
    sal.nomme().unwrap_or_default()
}
