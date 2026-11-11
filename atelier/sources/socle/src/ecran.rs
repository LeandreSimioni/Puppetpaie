//! CE QUE LA PAGE DESSINE D'UN ETAT : sa couleur (`couleur_de_l_etat`, la charte des etats) et la pastille qui le dit
//! (`Pastille`) ; et une tournure que l'ecran partage, un nombre de choses (`compte`). Le
//! moteur les rend avec ses reponses ; la page ne les compose jamais.

/// LA COULEUR D'UN ETAT : fait, vert ; a faire, en attente, en retard, orange ; refuse, rouge ; le reste, gris.
pub fn couleur_de_l_etat(etat: EtatDEcran) -> &'static str {
    match etat {
        EtatDEcran::Fait => "vert",
        EtatDEcran::AFaire | EtatDEcran::EnAttente | EtatDEcran::EnRetard => "orange",
        EtatDEcran::Refuse => "rouge",
        EtatDEcran::Information | EtatDEcran::Rien => "gris",
    }
}

/// UN ETAT TEL QUE L'ECRAN LE COLORE (`couleur_de_l_etat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatDEcran {
    Fait,
    AFaire,
    EnAttente,
    EnRetard,
    Refuse,
    Information,
    Rien,
}

/// LA PASTILLE D'UN ETAT, telle que la page la dessine : sa couleur (`couleur_de_l_etat`) et ses mots.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Pastille {
    pub couleur: &'static str,
    pub mots: String,
}

impl Pastille {
    pub fn de(etat: EtatDEcran, mots: &str) -> Pastille {
        Pastille { couleur: couleur_de_l_etat(etat), mots: mots.to_string() }
    }
}

/// « 1 salarié », « 3 salariés » : `n` et le nom, au pluriel quand il le faut (un `s` de plus, ou `pluriel`).
pub fn compte(n: usize, un: &str, pluriel: &str) -> String {
    format!("{} {}", n, if n > 1 { pluriel } else { un })
}
