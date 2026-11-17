//! Les aides des tests du droit : les references typees, lues par la porte du socle (`typee`) ; le jour et le mois que
//! le test ecrit, lus par leur forme.

pub mod typee;

/// Le jour que le test ecrit (`AAAA-MM-JJ`), lu par sa forme.
pub fn le_jour(ecrit: &str) -> socle::date::Jour {
    socle::forme::lire_texte(&format!("\"{}\"", ecrit), "le jour du test").unwrap()
}

/// L'annee que le test ecrit (`AAAA`), lue par sa forme.
pub fn l_annee(ecrit: &str) -> socle::mois::Annee {
    socle::forme::lire_texte(ecrit, "l'annee du test").unwrap()
}

/// Le mois que le test ecrit (`AAAA-MM`), lu par sa forme.
pub fn le_mois(ecrit: &str) -> socle::mois::Mois {
    socle::forme::lire_texte(&format!("\"{}\"", ecrit), "le mois du test").unwrap()
}
