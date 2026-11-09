//! CE QUE LES MODULES PAS ENCORE BRANCHES SUR L'ENVELOPPE LISENT EN JSON : leur charge (`lire_charge`), et le nom d'un
//! salarie a l'ecran. Une route branchee n'en prend rien : elle lit sa charge en sa forme (`Charge::lire`) et rend la
//! sienne (`enveloppe::Dite`). Ce module s'efface avec le dernier module qui s'en sert.

use serde_json::Value;

use socle::echec::{erreur, Classe, R};

/// La charge d'une route pas encore branchee, lue en sa forme comme `Charge::lire` : une cle que la forme ne connait
/// pas, une cle qui manque, une valeur d'une autre forme la refusent (`MalFormee`), la cle fautive a part ; rien ne
/// s'ecrit.
pub(crate) fn lire_charge<T: serde::de::DeserializeOwned>(charge: &Value) -> R<T> {
    socle::forme::lire(charge, "La demande est mal formée.").map_err(|m| erreur(Classe::MalFormee, m))
}

/// Le nom d'un salarie tel qu'un ecran le montre (`ChampsSalarie::nomme`), vide pour une fiche sans nom.
pub(crate) fn nom_a_l_ecran(sal: &droit::fiche::ChampsSalarie) -> String {
    sal.nomme().unwrap_or_default()
}
