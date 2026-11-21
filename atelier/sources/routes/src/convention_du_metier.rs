//! Ce que la convention de la compagnie fixe a un metier, rubrique par rubrique (`convention-du-metier`) : la bulle d'une
//! ligne de paie de la carte. Une lecture : elle repond toujours, et un metier dont rien n'est releve, ou un code
//! qu'aucune liste n'a (une frappe en cours), rend une liste vide.

use serde::{Deserialize, Serialize};
use socle::echec::Classe;
use socle::forme::TexteDit;
use socle::mois::Mois;

use crate::catalogue::{Acte, Affiche, Champ, Ecran, Fiche, Forme, Refus, Route, FORME, MOIS};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use droit::contexte::Contexte;
use droit::emplois::CodeEmploi;
use engagement::minima::{ce_que_fixe_la_convention, CeQueFixe};
use socle::echec::R;

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[Fiche {
    route: Route::ConventionDuMetier,
    geste: "Dire ce que la convention fixe au métier, rubrique par rubrique.",
    charge: &[MOIS, Champ { nom: "metier", libelle: Affiche::new("Métier"), forme: Forme::Texte, sens: "le code du métier (MAR010…)", avance: false }],
    exemple: r#"{"mois":"2026-11","metier":"MAR010"}"#,
    rend: "phrase : le nom que la convention donne au métier (la tête de la bulle) ; rubriques : pour chacune (rubrique), les montants et ce que chacun paie (condition, montant), le texte qui les fixe",
    refus: &[FORME, Refus { classe: Classe::Disque, corriger: "le fichier que la phrase dit illisible" }],
    acte: Acte::Lit,
    phrase: 80,
    ecran: Ecran::Lecture,
}];

/// La charge : le mois de la carte, et le code de son metier, tel que la carte l'ecrit.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeDuMetier {
    mois: Mois,
    metier: TexteDit,
}

/// Ce que la route rend : sa phrase (le nom que la convention donne au metier, la tete de la bulle ; ou qu'elle ne fixe
/// rien), et ce que la convention fixe pour chaque rubrique.
#[derive(Debug, Serialize)]
struct ConventionDuMetier {
    phrase: Phrase,
    rubriques: Vec<CeQueFixe>,
}

impl Dite for ConventionDuMetier {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::ConventionDuMetier => en_json(convention_du_metier(porte.ctx, charge.lire()?)?),
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

/// `convention-du-metier` : ce qui vaut au premier jour du mois de la carte.
fn convention_du_metier(ctx: &Contexte, c: ChargeDuMetier) -> R<ConventionDuMetier> {
    let fixe = match CodeEmploi::du_texte(&c.metier)? {
        Some(code) => ce_que_fixe_la_convention(ctx, &code, c.mois.premier_jour())?,
        None => None,
    };
    Ok(match fixe {
        Some(f) if !f.rubriques.is_empty() => ConventionDuMetier { phrase: socle::phrase!("{}", f.nom), rubriques: f.rubriques },
        _ => ConventionDuMetier { phrase: Phrase::fixe("La convention ne fixe rien de relevé pour ce métier."), rubriques: Vec::new() },
    })
}
