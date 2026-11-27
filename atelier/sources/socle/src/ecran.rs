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
    pub libelle: std::borrow::Cow<'static, str>,
    pub a_droite: bool,
    pub discrete: bool,
    /// Dans un tableau qui se saisit (`SaisieDuTableau`), les parties de la case que la colonne tient, dans l'ordre (la
    /// fin d'une ligne prevue) ; aucune : elle ne se saisit pas.
    #[serde(skip_serializing_if = "<[&str]>::is_empty")]
    pub parties: &'static [&'static str],
}

impl ColonneRendue {
    /// Une colonne de texte.
    pub fn de_texte(libelle: &'static str) -> ColonneRendue {
        ColonneRendue { libelle: libelle.into(), a_droite: false, discrete: false, parties: &[] }
    }

    /// Une colonne de nombres, a droite.
    pub fn de_nombres(libelle: &'static str) -> ColonneRendue {
        ColonneRendue { libelle: libelle.into(), a_droite: true, discrete: false, parties: &[] }
    }

    /// Une colonne de nombres dont l'en-tete se compose (« Pour 100 € / jour », la valeur que la charge dit).
    pub fn de_nombres_dite(libelle: String) -> ColonneRendue {
        ColonneRendue { libelle: libelle.into(), a_droite: true, discrete: false, parties: &[] }
    }

    /// Une colonne qui precise la ligne, en retrait.
    pub fn discrete(libelle: &'static str) -> ColonneRendue {
        ColonneRendue { libelle: libelle.into(), a_droite: false, discrete: true, parties: &[] }
    }

    /// La meme, qui tient les parties `parties` d'une rangee qui se saisit.
    pub fn tenant(self, parties: &'static [&'static str]) -> ColonneRendue {
        ColonneRendue { parties, ..self }
    }
}

/// UN TABLEAU RENDU, tel que la page le dessine et que la ligne de commande le lit : son titre (aucun : le tableau n'en a
/// pas), ses colonnes, ses rangees, chacune ses cellules deja dites, et, quand des rangees s'y saisissent, la case
/// qu'elles portent (`saisie`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TableauRendu {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titre: Option<&'static str>,
    pub colonnes: Vec<ColonneRendue>,
    pub rangees: Vec<RangeeRendue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saisie: Option<SaisieDuTableau>,
}

/// LES RANGEES QUI SE SAISISSENT DANS UN TABLEAU (les lignes prevues dans le tableau des paies de `droits-des-pieces`) :
/// la case de la charge dont elles sont les rangees, et le bouton qui en retire une ; une rangee vide attend toujours en
/// bas, et se remplit pour en ajouter une. Chaque colonne dit les parties qu'elle tient (`ColonneRendue::parties`), chaque rangee son rang dans la case
/// (`RangeeRendue::saisie`) ; une rangee de la case que le tableau ne place pas (vide, ou qui ne se compte pas) se pose
/// apres ses rangees.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SaisieDuTableau {
    pub case: &'static str,
    pub retirer: &'static str,
}

impl TableauRendu {
    /// Un tableau sans titre.
    pub fn de(colonnes: Vec<ColonneRendue>, rangees: Vec<RangeeRendue>) -> TableauRendu {
        TableauRendu { titre: None, colonnes, rangees, saisie: None }
    }

    /// Le meme, sous le titre `titre`.
    pub fn intitule(self, titre: &'static str) -> TableauRendu {
        TableauRendu { titre: Some(titre), ..self }
    }

    /// Le meme, ou les rangees de la case `saisie` se saisissent.
    pub fn saisissant(self, saisie: SaisieDuTableau) -> TableauRendu {
        TableauRendu { saisie: Some(saisie), ..self }
    }

    /// Des lignes, chacune ses cellules dites, une par colonne.
    pub fn de_lignes(colonnes: Vec<ColonneRendue>, lignes: impl IntoIterator<Item = Vec<Option<String>>>) -> TableauRendu {
        TableauRendu::de(colonnes, lignes.into_iter().map(|cellules| RangeeRendue::de(SorteDeRangee::Ligne, cellules)).collect())
    }
}

/// UNE RANGEE D'UN TABLEAU RENDU : ce qu'elle est (`SorteDeRangee`), ses cellules deja dites, une par colonne (une
/// cellule qui ne dit rien est `None` : la page n'y pose rien), et, dans un tableau qui se saisit, son rang dans la case
/// (`saisie` : ses parties s'y saisissent, sous les colonnes qui les tiennent ; aucun : elle ne se saisit pas).
#[derive(Debug, Clone, serde::Serialize)]
pub struct RangeeRendue {
    pub sorte: SorteDeRangee,
    pub cellules: Vec<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saisie: Option<usize>,
}

impl RangeeRendue {
    /// Une rangee qui ne se saisit pas.
    pub fn de(sorte: SorteDeRangee, cellules: Vec<Option<String>>) -> RangeeRendue {
        RangeeRendue { sorte, cellules, saisie: None }
    }

    /// Une ligne, la rangee `rang` de la case du tableau.
    pub fn saisie(cellules: Vec<Option<String>>, rang: usize) -> RangeeRendue {
        RangeeRendue { sorte: SorteDeRangee::Ligne, cellules, saisie: Some(rang) }
    }
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
