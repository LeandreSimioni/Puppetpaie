//! Les petits outils des commandes : les classes d'un refus, les garde-fous du binaire de l'atelier.

use socle::echec::{Classe, Echec};

/// Les classes d'un refus metier : le geste refuse, et ses cas nommes.
const OPERATION: [Classe; 17] = [
    Classe::Refuse, Classe::SaisieIncomplete, Classe::PreuveInsuffisante, Classe::FicheRefusee, Classe::CompteursVerrouilles,
    Classe::VersionExigee, Classe::DsnDejaDeposee, Classe::RepetitionManquante, Classe::OrdreTropPetit, Classe::AnnuleRemplaceHorsDelai,
    Classe::MoisNonRegularisable, Classe::ExerciceClos, Classe::SortieAbsente, Classe::BoiteAbsente, Classe::AucunDocument, Classe::ActeReelRefuse,
    Classe::ModificationNonDeclarable,
];

/// Cette classe est-elle celle d'un refus metier ?
pub fn est_operation(classe: Option<Classe>) -> bool {
    classe.is_some_and(|c| OPERATION.contains(&c))
}

/// Un refus que la commande rend en une phrase : un refus metier, ou une valeur illisible (la route rend ainsi ce que
/// la norme refuse, une date qui ne se lit pas).
pub fn est_refus(classe: Option<Classe>) -> bool {
    est_operation(classe) || classe == Some(Classe::Illisible)
}

// ------------------------------------------------------------------ le binaire de l'atelier

/// Ce que la redirection de l'atelier tient : net-entreprises, l'Urssaf et Google (`envois::redirection`). Le reste -- la boite courriel --
/// n'est jamais redirige.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Organisme {
    Dsn,
    Dpae,
    Google,
}

/// Dans `moteur-atelier`, HORS COMPAGNIE FICTIVE : l'organisme n'est-il PAS redirige vers un faux serveur local ? Alors une commande qui lui
/// parlerait irait au vrai, avec les vrais mots de passe du poste : elle doit s'arreter. Dans `moteur` : jamais (c'est le vrai
/// serveur, et c'est son role ; les garde-fous des actes reels tiennent seuls).
#[cfg(feature = "atelier")]
pub fn pas_redirige(cadre: &super::Cadre, organisme: Organisme) -> bool {
    let variable = match organisme {
        Organisme::Dsn => envois::redirection::VARIABLE_DSN,
        Organisme::Dpae => envois::redirection::VARIABLE_DPAE,
        Organisme::Google => envois::redirection::VARIABLE_GOOGLE,
    };
    envois::redirection::base(variable).is_none() && !matches!(compagnie::bridage::branchement(&cadre.racine), Ok(b) if b.joue())
}

#[cfg(not(feature = "atelier"))]
pub fn pas_redirige(_cadre: &super::Cadre, _organisme: Organisme) -> bool {
    false
}

/// Le refus de l'atelier quand `organisme` n'y est pas redirige (`pas_redirige`) ; `quoi` : ce qui irait au vrai serveur.
#[cfg(feature = "atelier")]
pub fn refuser_si_pas_redirige(cadre: &super::Cadre, organisme: Organisme, quoi: &str) -> Result<(), Echec> {
    if !pas_redirige(cadre, organisme) {
        return Ok(());
    }
    let variable = match organisme {
        Organisme::Dsn => "PUPPETPAIE_ATELIER_DSN",
        Organisme::Dpae => "PUPPETPAIE_ATELIER_DPAE",
        Organisme::Google => "PUPPETPAIE_ATELIER_GOOGLE",
    };
    Err(Echec {
        classe: Some(Classe::ActeReelRefuse),
        message: format!(
            "Ceci est le binaire de l'atelier (moteur-atelier) : {} irait au vrai serveur, que l'atelier ne tient pas. Poser {} sur un faux serveur local. Rien n'a ete modifie.",
            quoi, variable
        ),
    })
}

/// Dans `moteur` : rien n'est redirige, rien n'est refuse ici (le texte de l'atelier n'est pas compile dans le binaire livre).
#[cfg(not(feature = "atelier"))]
pub fn refuser_si_pas_redirige(_cadre: &super::Cadre, _organisme: Organisme, _quoi: &str) -> Result<(), Echec> {
    Ok(())
}
