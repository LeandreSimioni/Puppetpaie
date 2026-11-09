//! CE QUE LA PAGE DESSINE D'UN ETAT : sa couleur (`couleur_de_l_etat`, la charte des etats) et la pastille qui le dit
//! (`Pastille`) ; et deux tournures que l'ecran partage, un horodatage (`quand`) et un nombre de choses (`compte`). Le
//! moteur les rend avec ses reponses ; la page ne les compose jamais.

/// LA COULEUR D'UN ETAT : fait, vert ; a faire, en attente, en retard, orange ; refuse, rouge ; le reste, gris.
pub fn couleur_de_l_etat(etat: &str) -> &'static str {
    match etat {
        "fait" => "vert",
        "a_faire" | "en_attente" | "en_retard" => "orange",
        "refuse" => "rouge",
        _ => "gris",
    }
}

/// LA PASTILLE D'UN ETAT, telle que la page la dessine : sa couleur (`couleur_de_l_etat`) et ses mots.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Pastille {
    pub couleur: &'static str,
    pub mots: String,
}

impl Pastille {
    pub fn de(etat: &str, mots: &str) -> Pastille {
        Pastille { couleur: couleur_de_l_etat(etat), mots: mots.to_string() }
    }
}

/// « 12/09/2026 à 18:18 » : un horodatage ISO tel que l'ecran le dit ; ce qui n'en est pas un, tel quel.
pub fn quand(iso: &str) -> String {
    let b = iso.as_bytes();
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return iso.to_string();
    }
    let jour = format!("{}/{}/{}", &iso[8..10], &iso[5..7], &iso[..4]);
    match iso.get(11..16) {
        Some(h) => format!("{} à {}", jour, h),
        None => jour,
    }
}

/// « 1 salarié », « 3 salariés » : `n` et le nom, au pluriel quand il le faut (un `s` de plus, ou `pluriel`).
pub fn compte(n: usize, un: &str, pluriel: &str) -> String {
    format!("{} {}", n, if n > 1 { pluriel } else { un })
}
