//! Ce que la convention de la compagnie fixe a un metier, rubrique par rubrique (`convention-du-metier`) : la bulle d'une
//! ligne de paie de la carte. Une lecture : elle repond toujours, et un metier dont rien n'est releve rend une liste vide.

use serde::{Deserialize, Serialize};
use socle::mois::Mois;

use crate::catalogue::{Acte, Affiche, Champ, Ecran, Fiche, Forme, Route, FORME, MOIS};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use droit::contexte::Contexte;
use engagement::minima::{ce_que_fixe_la_convention, CeQueFixe};
use socle::echec::R;

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[Fiche {
    route: Route::ConventionDuMetier,
    geste: "Dire ce que la convention fixe au métier, rubrique par rubrique.",
    charge: &[MOIS, Champ { nom: "metier", libelle: Affiche::new("Métier"), forme: Forme::Texte, sens: "le code du métier (MAR010…)", avance: false }],
    exemple: r#"{"mois":"2026-11","metier":"MAR010"}"#,
    rend: "rubriques : pour chacune (rubrique), le nom que la convention donne au métier (nom), la situation, les montants et ce que chacun paie (condition, montant), le texte en clair",
    refus: &[FORME],
    acte: Acte::Lit,
    phrase: 0,
    ecran: Ecran::Lecture,
}];

/// La charge : le mois de la carte, et son metier.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeDuMetier {
    mois: Mois,
    metier: droit::emplois::CodeEmploi,
}

/// Ce que la route rend : sa phrase, et ce que la convention fixe pour chaque rubrique.
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
    let rubriques = ce_que_fixe_la_convention(ctx, &c.metier, c.mois.premier_jour())?;
    let phrase = match rubriques.len() {
        0 => Phrase::fixe("La convention ne fixe rien de relevé pour ce métier."),
        n => socle::phrase!("Ce que la convention fixe pour ce métier : {} rubrique{}.", n, if n > 1 { "s" } else { "" }),
    };
    Ok(ConventionDuMetier { phrase, rubriques })
}
