//! LES TEMPS D'UNE CASE DATEE, tels qu'une route les rend sous la case : la derniere valeur d'avant et la prochaine a
//! venir, et toutes ses valeurs pour l'historique replie. Le calcul est `droit::fiche::temps_au`, seul ; ici, il se met
//! en phrases. Les fiches (`ouverture`), les autres etablissements (`ouverture`) et les contrats de la DSN
//! (`gestes::organismes`) le rendent ainsi.

use serde::Serialize;
use socle::date::Jour;
use socle::echec::Echec;

type R<T> = Result<T, Echec>;

/// CE QU'UNE CASE DATEE DIT A UN JOUR : sous la case, la derniere valeur d'avant et la prochaine a venir ; toutes ses
/// valeurs, passees et a venir, pour l'historique replie. Les cases d'une liste (les autres etablissements) ont les leurs,
/// ligne par ligne (`lignes`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct TempsDeLaCase<V> {
    pub(crate) champ: CaseDatee,
    #[serde(skip_serializing_if = "Option::is_none")]
    avant: Option<LigneDuTemps<V>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    a_venir: Option<LigneDuTemps<V>>,
    valeurs: Vec<LigneDuTemps<V>>,
    lignes: Vec<TempsDUneLigne<V>>,
}

/// CE QUI EST DATE : une case d'une fiche (ou d'une ligne d'une liste de la fiche), ou un contrat de prevoyance saisi
/// (`config/organismes.json`), par sa cle. Il s'ecrit en texte.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub(crate) enum CaseDatee {
    Case(droit::fiche::NomDeCase),
    Contrat(String),
}

/// Les temps des cases d'une ligne d'une liste datee, la ligne nommee par sa cle (le NIC d'un etablissement) et par ce
/// qui la dit (son libelle).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct TempsDUneLigne<V> {
    pub(crate) cle: String,
    pub(crate) dit: String,
    pub(crate) cases: Vec<TempsDeLaCase<V>>,
}

/// Une valeur d'une case datee : de quand a quand elle vaut, la phrase qui le dit, si elle est celle d'aujourd'hui, et
/// d'ou elle vient (la `note` de sa ligne datee ; sans note, rien n'est dit).
#[derive(Debug, Clone, Serialize)]
struct LigneDuTemps<V> {
    valeur: Option<V>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depuis: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jusqu_au: Option<Jour>,
    dit: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    en_vigueur: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// De quand a quand une valeur datee vaut, en mots.
fn quand_dit(depuis: Option<Jour>, jusqu_au: Option<Jour>) -> String {
    match (depuis, jusqu_au) {
        (None, None) => "de tout temps".to_string(),
        (None, Some(j)) => format!("jusqu'au {}", j.jj_mm_aaaa()),
        (Some(d), None) => format!("à partir du {}", d.jj_mm_aaaa()),
        (Some(d), Some(j)) => format!("du {} au {}", d.jj_mm_aaaa(), j.jj_mm_aaaa()),
    }
}

/// LES TEMPS DE LA CASE `champ` (sa liste datee `liste`) a `jour` (`droit::fiche::temps_au`, `meme` disant deux valeurs
/// pareilles) ; `None` si elle n'a qu'une valeur.
pub(crate) fn d_une_case<V: Clone>(champ: CaseDatee, liste: &[droit::fiche::Datee<V>], jour: &Jour, meme: impl Fn(&V, &V) -> bool) -> R<Option<TempsDeLaCase<V>>> {
    let temps = droit::fiche::temps_au(liste, jour, meme)?;
    if temps.toutes.len() < 2 {
        return Ok(None);
    }
    let avant = temps.avant.map(|a| LigneDuTemps { valeur: Some(a.valeur), depuis: None, jusqu_au: Some(a.jusqu_au), dit: quand_dit(None, Some(a.jusqu_au)), en_vigueur: false, note: a.note });
    let a_venir = temps.a_venir.map(|a| LigneDuTemps { valeur: a.valeur, depuis: Some(a.depuis), jusqu_au: None, dit: quand_dit(Some(a.depuis), None), en_vigueur: false, note: a.note });
    // Chaque date n'y est qu'une fois : la sienne nomme la valeur en vigueur.
    let en_vigueur = temps.en_vigueur.map(|e| e.depuis);
    let mut valeurs = Vec::new();
    for (i, e) in temps.toutes.iter().enumerate() {
        let jusqu_au = temps.toutes.get(i + 1).and_then(|s| s.depuis).map(|d| d.plus_jours(-1)).transpose()?;
        valeurs.push(LigneDuTemps { valeur: e.valeur.clone(), depuis: e.depuis, jusqu_au, dit: quand_dit(e.depuis, jusqu_au), en_vigueur: en_vigueur == Some(e.depuis), note: e.note.clone() });
    }
    Ok(Some(TempsDeLaCase { champ, avant, a_venir, valeurs, lignes: Vec::new() }))
}

/// LES TEMPS DES CASES DE CHAQUE LIGNE d'une liste datee (`lignes`, chacune nommee), sous la case `champ` de la liste ;
/// `None` si aucune case d'aucune ligne n'a plus d'une valeur.
pub(crate) fn d_une_liste<V>(champ: droit::fiche::NomDeCase, lignes: Vec<TempsDUneLigne<V>>) -> Option<TempsDeLaCase<V>> {
    let lignes: Vec<TempsDUneLigne<V>> = lignes.into_iter().filter(|l| !l.cases.is_empty()).collect();
    (!lignes.is_empty()).then(|| TempsDeLaCase { champ: CaseDatee::Case(champ), avant: None, a_venir: None, valeurs: Vec::new(), lignes })
}
