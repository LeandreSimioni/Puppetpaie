//! CE QUE LES MODULES PAS ENCORE BRANCHES SUR L'ENVELOPPE LISENT ET ECRIVENT EN JSON : leur charge (`lire_charge`), leur
//! reponse (`en_json`), les morceaux qu'ils y composent (un encart, une pastille, une phrase en tete) et les mots qu'ils
//! tirent d'une valeur. Une route branchee n'en prend rien : elle lit sa charge en sa forme (`Charge::lire`) et rend la
//! sienne (`enveloppe::Dite`). Ce module s'efface avec le dernier module qui s'en sert.

use std::path::Path;

use serde_json::{json, Map, Value};

use crate::valeur::valeur_en_texte;
use socle::echec::{erreur, Classe, Echec, R};

use super::Encart;

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

/// `Encart::de`, en JSON.
pub(crate) fn encart(etat: &str, titre: impl Into<String>, phrase: impl Into<String>) -> Value {
    json!(Encart::de(etat, titre, phrase))
}

/// La phrase d'une route, en tete de ce qu'elle rend ; une phrase deja posee par la route est remplacee.
pub(crate) fn phrase_en_tete(v: Value, phrase: impl Into<String>) -> Value {
    let mut m = Map::new();
    m.insert("phrase".into(), Value::String(phrase.into()));
    if let Value::Object(o) = v {
        m.extend(o.into_iter().filter(|(k, _)| k != "phrase"));
    }
    Value::Object(m)
}

/// Une valeur rendue, en mots dans une phrase : rien pour une valeur absente (jamais « None »).
pub(crate) fn mot(v: &Value) -> String {
    if v.is_null() { String::new() } else { valeur_en_texte(v) }
}

/// Un jour rendu (`2026-11-02`), tel qu'une phrase le dit : « 02/11/2026 » ; ce qui n'en est pas un, tel quel.
pub(crate) fn jour_en_clair(v: &Value) -> String {
    let t = mot(v);
    socle::date::Jour::lire(&t).map(|j| j.jj_mm_aaaa()).unwrap_or(t)
}


/// Le nom d'un salarie tel qu'un ecran le montre (`ChampsSalarie::nomme`), vide pour une fiche sans nom.
pub(crate) fn nom_a_l_ecran(sal: &droit::fiche::ChampsSalarie) -> String {
    sal.nomme().unwrap_or_default()
}

/// Un fichier lu en UTF-8 : un refus du disque sous sa classe (`FichierAbsent`, `Disque`), un texte qui n'est pas de
/// l'UTF-8 `Illisible`.
pub(crate) fn lire_texte(p: &Path) -> R<String> {
    let octets = std::fs::read(p).map_err(|e| erreur(Classe::du_disque(&e), socle::echec::du_disque(p, &e)))?;
    String::from_utf8(octets).map_err(|e| erreur(Classe::Illisible, socle::echec::du_disque(&p, &e)))
}
