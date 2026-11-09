//! L'IMPORT AU NIVEAU DES ROUTES : ce que l'import compose et ecrit pour les routes de « Reprendre l'historique »
//! (`routes::import`, `routes::archives`), sur ce que l'import lit des pieces (`crate::import`). L'apercu du dossier
//! (`bulletins`), ce qui s'ecrit (`ecrite` : les engagements importes ; `fiches` : les fiches creees ou completees par
//! le chemin des fiches ; `completer` : ce qu'une fiche qui existe recoit), la nature des contrats que les bulletins
//! n'ont pas dite (`nature`), ce que les DSN reprises disent de la configuration (`parametrage`), la regularisation
//! CASC-SVP des mois importes (`casc`) et l'incident d'un document accepte sans etre lu (`incident`). La reponse de
//! l'import (`import::reponse::Importation`) se dit ici (`Dite`).

pub mod bulletins;
pub mod casc;
pub mod completer;
pub mod ecrite;
pub mod fiches;
pub mod incident;
pub mod nature;
pub mod parametrage;

impl crate::routes::enveloppe::Dite for crate::import::reponse::Importation {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}
