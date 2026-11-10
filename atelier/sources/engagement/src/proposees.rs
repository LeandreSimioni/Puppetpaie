//! LES RUBRIQUES QU'UNE CARTE PROPOSE : celles que le calcul paie a cet engagement. Deux faits les decident, ceux que le
//! calcul lit : la nature du contrat (la forme de paie : un contrat d'usage paie son activite, une carte au mois ne porte
//! que ses frais, `a_une_limite` ; `Rubrique::reservee_a`), et le statut que son metier lui donne (le service et le
//! cachet, unites des seuls artistes : `Rubrique::reservee_au_statut`, que `unites_du_statut` refuse sinon). Ce que la
//! carte ne dit pas encore ne retire rien. La rubrique d'une ligne neuve est la premiere de salaire de ces rubriques,
//! sur une carte au mois son premier frais, dans l'ordre du catalogue.

use droit::catalogue::{catalogue, Rubrique};
use droit::contexte::Contexte;
use droit::emplois;
use droit::fiche::Lue as _;
use socle::date::Jour;
use socle::num::Resultat;

use crate::saisie::Carte;

/// Ce que la carte d'un engagement propose : les codes de ses rubriques, dans l'ordre du catalogue, et celle d'une ligne
/// neuve.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Proposees {
    pub rubriques: Vec<String>,
    pub par_defaut: Option<String>,
}

/// Les rubriques que la nature (`mensuelle` : payee au mois ; `None` : pas encore dite) et le statut (`None` : pas encore
/// dit) d'un engagement prennent, au catalogue de la compagnie a `jour`.
pub fn proposees(ctx: &Contexte, mensuelle: Option<bool>, statut: Option<&str>, jour: &Jour) -> Resultat<Proposees> {
    let cat = catalogue(&ctx.racine, jour)?;
    let prend = |r: &&Rubrique| {
        let forme = match mensuelle {
            None => true,
            Some(true) => crate::a_une_limite(&r.code),
            Some(false) => r.reservee_a() != Some("mensuelle"),
        };
        !r.saisie.is_empty() && forme && statut.is_none_or(|s| r.reservee_au_statut().is_none_or(|v| v == s))
    };
    let siennes: Vec<&Rubrique> = cat.iter().filter(prend).collect();
    let par_defaut = match mensuelle {
        Some(true) => siennes.first(),
        _ => siennes.iter().find(|r| r.nature == "salaire"),
    };
    Ok(Proposees { par_defaut: par_defaut.map(|r| r.code.clone()), rubriques: siennes.iter().map(|r| r.code.clone()).collect() })
}

/// Les rubriques d'une carte qui ne se calcule pas encore, lues sur sa saisie comme la paie les lira : sa nature si elle
/// la dit ; son statut ecrit, sinon celui de son metier, sinon celui de la fiche de son salarie.
pub fn de_la_carte(ctx: &Contexte, carte: &Carte, jour: &Jour) -> Resultat<Proposees> {
    let dit = |x: &Option<String>| x.as_deref().map(str::trim).filter(|x| !x.is_empty()).map(str::to_string);
    let mensuelle = match dit(&carte.nature) {
        Some(n) => Some(droit::natures::nature(&n)?.mensuelle()),
        None => None,
    };
    let du_metier = match dit(&carte.metier) {
        Some(code) => emplois::metier(&code)?.map(|m| m.statut().as_str().to_string()),
        None => None,
    };
    let de_la_fiche = carte.salarie.as_ref().and_then(|c| ctx.salaries.get(c.as_str())).and_then(|f| f.statut.dite()).map(|s| s.as_str().to_string());
    let statut = dit(&carte.statut).or(du_metier).or(de_la_fiche);
    proposees(ctx, mensuelle, statut.as_deref(), jour)
}
