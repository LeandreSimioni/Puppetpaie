//! La CASC-SVP que lit la DSN : les mois regularises hors DSN (sa base livree : `droit::adhesions::casc_svp_base`).

use serde::{Deserialize, Serialize};

use droit::contexte::Contexte;
use socle::num::Resultat;

/// Ce que la compagnie dit de ses mois Conges Spectacles regularises hors DSN, sous sa racine.
pub const CASC_SVP: &str = "config/casc-svp.json";

/// `config/casc-svp.json` : les annees regularisees hors DSN, dans l'ordre ou elles ont ete notees.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CascSvp {
    pub quoi: String,
    pub regularisations: Vec<RegularisationNotee>,
}

/// Une annee de CASC-SVP notee regularisee : ses mois, le montant vire et le montant calcule, le jour du virement, le
/// jour de la note et qui l'a notee.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegularisationNotee {
    pub annee: String,
    pub mois: Vec<String>,
    pub montant: String,
    pub calcule: String,
    pub vire_le: String,
    pub note_le: String,
    pub qui: String,
}

/// `config/casc-svp.json` de la compagnie, lu par sa forme ; `None` s'il n'existe pas (aucune annee notee).
pub fn lue(ctx: &Contexte) -> Resultat<Option<CascSvp>> {
    let p = ctx.racine.join(CASC_SVP);
    if !p.exists() {
        return Ok(None);
    }
    socle::forme::lire_fichier(&p, "Le fichier de la CASC-SVP de la compagnie (config/casc-svp.json) ne se lit pas : le signaler au développeur.").map(Some)
}

/// `mois_regularises` : les mois de `config/casc-svp.json` regularises hors DSN, tries (un fichier absent n'en
/// regularise aucun).
pub fn mois_regularises(ctx: &Contexte) -> Resultat<Vec<String>> {
    let mut mois: Vec<String> = vec![];
    for reg in lue(ctx)?.iter().flat_map(|d| &d.regularisations) {
        for m in &reg.mois {
            if !mois.contains(m) {
                mois.push(m.clone());
            }
        }
    }
    mois.sort();
    Ok(mois)
}
