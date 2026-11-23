//! LES LISTES FERMEES : un type dont les valeurs se comptent (une `enum`, une liste du logiciel) dit lui-meme chacune,
//! ce qu'une charge en ecrit (`code`) et ce que l'ecran en montre (`libelle`). La fiche d'une route qui lit ce type en
//! tire la forme de son champ (`routes::catalogue::ListeDeChoix::de`) : une liste deroulante a l'ecran, la meme liste
//! en ligne de commande ; jamais une seconde liste ecrite a cote du type.

use serde::Serialize;

use crate::echec::{erreur, Classe, R};

/// UNE LISTE FERMEE : toutes ses valeurs, dans l'ordre ou l'ecran les propose, le code que la charge ecrit et le libelle
/// que l'ecran montre de chacune.
pub trait ListeFermee: Sized + 'static {
    /// Chacune, dans l'ordre de la liste.
    const TOUTES: &'static [Self];
    /// Ce que la charge ecrit : celui que le type lit (`code_ecrit` pour un type que serde ecrit en un mot).
    fn code(&self) -> R<String>;
    /// Ce que l'ecran en montre.
    fn libelle(&self) -> R<&'static str>;
}

/// LE REFUS D'UNE VALEUR QU'UNE LISTE FERMEE N'A PAS, a sa lecture : la meme phrase que la porte dit d'une `enum` que
/// serde lit (`forme::en_clair`), precedee de ce que la liste nomme (`quoi`), et suivie des valeurs qu'elle a.
pub fn pas_de_la_liste(quoi: &str, valeur: &str, connues: &[&str]) -> String {
    format!("{} : la valeur {} n'est pas connue ici (attendues : {})", quoi, crate::fichiers::repr(valeur), connues.join(", "))
}

/// LE CODE D'UNE VALEUR QUE SERDE ECRIT EN UN MOT : celui-la meme que sa lecture attend (`socle::ecriture::mot`).
pub fn code_ecrit<T: Serialize>(v: &T) -> R<String> {
    match crate::ecriture::mot(v).mot() {
        Some(m) => Ok(m.to_string()),
        None => Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : une valeur d'une liste fermée ne s'écrit pas en un mot.")),
    }
}
