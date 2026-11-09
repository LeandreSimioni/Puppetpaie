//! LES COMMANDES DE L'EDITEUR : celles qui passent par le cadre des commandes (`commandes::executer` : `sources`, et
//! dans le binaire de l'atelier les outils sur une compagnie, `outils`), et celles que `main` lance par leur nom
//! (`lancer` : le releve des textes, les referentiels, les tables Unicode, le guet, le garde avant commit, les bulletins
//! fictifs).

pub mod publication;
pub mod sources;
#[cfg(feature = "atelier")]
pub mod outils;

use crate::commandes::Commande;

/// Les commandes de l'editeur qui passent par le cadre des commandes, dans l'ordre de l'aide.
pub fn commandes() -> Vec<Commande> {
    #[allow(unused_mut)]
    let mut v = vec![
        // L'outil de l'editeur, avant chaque publication : refuse ailleurs que sur son poste (le reglage de l'installation, `maj::reglage`).
        Commande { nom: "sources", aide: "les sources des donnees livrees, et leur veille (poste de l'editeur)", cachee: true, spec: &sources::SPEC, executer: sources::executer },
    ];
    // LES OUTILS DE L'ATELIER sur une compagnie (`outils`) : le binaire de l'atelier seulement.
    #[cfg(feature = "atelier")]
    {
        use outils::{aer, depot_dsn, journal, pas, retours};
        v.extend([
            Commande { nom: "pas", aide: "les taux de PAS connus ; --amorcer, --demander (atelier)", cachee: false, spec: &pas::SPEC, executer: pas::executer },
            Commande { nom: "aer", aide: "ranger de nouveau les AER recues ; --forcer (atelier)", cachee: false, spec: &aer::SPEC, executer: aer::executer },
            Commande { nom: "journal", aide: "--dpae : reprendre au journal les DPAE attestees (atelier)", cachee: false, spec: &journal::SPEC, executer: journal::executer },
            Commande { nom: "retours", aide: "l'enquete des retours : plage, flux, plate-forme (atelier)", cachee: false, spec: &retours::SPEC, executer: retours::executer },
            Commande { nom: "depot-dsn", aide: "un depot de test a un ordre, ou chez l'editeur (atelier)", cachee: false, spec: &depot_dsn::SPEC, executer: depot_dsn::executer },
        ]);
    }
    v
}

/// La commande de l'editeur `nom`, lancee sur `args` : son code de sortie ; `None` si `nom` n'en est pas une.
pub fn lancer(nom: &str, args: &[String]) -> Option<i32> {
    Some(match nom {
        // Les bulletins fictifs d'un logiciel precedent, pour une compagnie qui s'importe. L'atelier seulement.
        #[cfg(feature = "atelier")]
        "fabriquer-bulletins" => super::fabriquer_bulletins::commande(args),
        "generer-tables" => super::generer_tables::commande(args),
        "extraire" => super::extraire::commande(args),
        // Les codes (LEGI) et les conventions collectives (KALI) releves chez la DILA, dans `atelier/reference/sources`.
        // Les tables des missions a l'etranger de la DGFiP, relevees avant chaque publication.
        "releve-missions" => super::releve::missions::commande(args),
        "releve-codes" => super::releve::codes::commande(args),
        "releve-conventions" => super::releve::conventions::commande(args),
        // Outil d'atelier : la veille du contexte d'un sous-agent (`guet.rs`).
        "guet" => super::guet::commande(args),
        // Le garde avant commit (le crochet `atelier/.githooks/pre-commit`) : les donnees reelles dans ce que git emporterait.
        "verifier-avant-commit" => super::verifier_avant_commit::commande(args),
        _ => return None,
    })
}
