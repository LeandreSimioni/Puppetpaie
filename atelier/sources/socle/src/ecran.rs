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

/// « 1 salarié », « 3 salariés » : `n` (`texte::compte`) et le nom, au pluriel quand il le faut (un `s` de plus, ou
/// `pluriel`).
pub fn compte(n: usize, un: &str, pluriel: &str) -> String {
    format!("{} {}", crate::texte::compte(n), if n > 1 { pluriel } else { un })
}

/// UNE COLONNE D'UN TABLEAU RENDU, telle que la page la dessine : son en-tete, a droite (un nombre, un montant), discrete
/// (ce qui precise la ligne : le spectacle d'une ligne du livre, le nombre de bulletins d'un total).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ColonneRendue {
    pub libelle: &'static str,
    pub a_droite: bool,
    pub discrete: bool,
}

impl ColonneRendue {
    /// Une colonne de texte.
    pub fn de_texte(libelle: &'static str) -> ColonneRendue {
        ColonneRendue { libelle, a_droite: false, discrete: false }
    }

    /// Une colonne de nombres, a droite.
    pub fn de_nombres(libelle: &'static str) -> ColonneRendue {
        ColonneRendue { libelle, a_droite: true, discrete: false }
    }

    /// Une colonne qui precise la ligne, en retrait.
    pub fn discrete(libelle: &'static str) -> ColonneRendue {
        ColonneRendue { libelle, a_droite: false, discrete: true }
    }
}

/// UNE RANGEE D'UN TABLEAU RENDU : ce qu'elle est (`SorteDeRangee`) et ses cellules deja dites, une par colonne.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RangeeRendue {
    pub sorte: SorteDeRangee,
    pub cellules: Vec<String>,
}

/// Ce qu'est une rangee d'un tableau rendu : une ligne, le sous-total d'un groupe, ou le total general (la page les dit en
/// gras).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SorteDeRangee {
    Ligne,
    SousTotal,
    TotalGeneral,
}

/// QUAND, SUR UNE CARTE FERMEE (celle d'un engagement, celle d'une paie importee) : ses jours en clair et au plus court
/// (`des_jours` ; une periode quand ils ne sont pas lus), s'ils sont epars (des jours qui ne se suivent pas, ou une
/// periode a cheval sur un autre mois), ce qu'ils sont au survol, et le mois, abrege.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QuandDeLaCarte {
    pub jours: String,
    pub epars: bool,
    pub titre: String,
    pub mois: &'static str,
}

impl QuandDeLaCarte {
    /// Les jours de `jours` qui tombent dans `mois`, en clair : « 6 », « 21–23 », « 14 · 15 · 17 · 18 » (des jours qui se
    /// suivent font une plage) ; au survol, chacun en entier.
    pub fn des_jours(jours: &[crate::date::Jour], mois: crate::mois::Mois) -> crate::echec::R<QuandDeLaCarte> {
        let mut jours: Vec<crate::date::Jour> = jours.iter().filter(|j| mois.contient(j)).copied().collect();
        jours.sort();
        jours.dedup();
        let mut suivis = true;
        for w in jours.windows(2) {
            suivis &= w[0].apres(crate::date::Duree::jours(1))? == w[1];
        }
        let en_clair = match (jours.first(), jours.last()) {
            (None, _) | (_, None) => String::new(),
            (Some(a), Some(b)) if a == b => a.quantieme_ecrit(),
            (Some(a), Some(b)) if suivis => format!("{}–{}", a.quantieme_ecrit(), b.quantieme_ecrit()),
            _ => jours.iter().map(crate::date::Jour::quantieme_ecrit).collect::<Vec<_>>().join(" · "),
        };
        Ok(QuandDeLaCarte {
            jours: en_clair,
            epars: !suivis,
            titre: jours.iter().map(crate::date::Jour::jj_mm_aaaa).collect::<Vec<_>>().join(", "),
            mois: mois.nom(crate::mois::Graphie::Abregee),
        })
    }
}
