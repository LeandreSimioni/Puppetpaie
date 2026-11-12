//! LES RUBRIQUES QU'UNE CARTE PROPOSE : celles que le calcul paie a cet engagement. Les faits qui les decident sont ceux
//! que le calcul lit : la nature du contrat (la forme de paie : un contrat d'usage paie son activite, une carte au mois ne
//! porte que ses frais, `a_une_limite` ; `Rubrique::reservee_a`), le statut que son metier lui donne (le service et le
//! cachet, unites des seuls artistes : `Rubrique::reservee_au_statut`, que `unites_du_statut` refuse sinon), et, pour la
//! prime d'anciennete, la convention et l'annexe de la compagnie et le metier de la carte
//! (`droit::clauses::prime_anciennete_possible`, les conditions de `droit::clauses::prime_anciennete`). Ce que la carte
//! ne dit pas encore ne retire rien. La rubrique d'une ligne neuve est la premiere de salaire de ces rubriques, sur une
//! carte au mois son premier frais, dans l'ordre du catalogue.

use droit::catalogue::{catalogue, Rubrique};
use droit::clauses;
use droit::contexte::Contexte;
use droit::fiche::Lue as _;
use socle::date::Jour;
use socle::echec::R;

use crate::saisie::Carte;

/// Ce que la carte d'un engagement propose : les codes de ses rubriques, dans l'ordre du catalogue, et celle d'une ligne
/// neuve.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Proposees {
    pub rubriques: Vec<droit::catalogue::CodeDeRubrique>,
    pub par_defaut: Option<droit::catalogue::CodeDeRubrique>,
}

/// Les rubriques que la nature (`mensuelle` : payee au mois ; `None` : pas encore dite), le statut et le metier (`None` :
/// pas encore dits) d'un engagement prennent, au catalogue de la compagnie a `jour`.
pub fn proposees(ctx: &Contexte, mensuelle: Option<bool>, statut: Option<droit::fiche::Statut>, metier: Option<&str>, jour: &Jour) -> R<Proposees> {
    let cat = catalogue(&ctx.racine, jour)?;
    let ent = &ctx.entreprise;
    let prime = clauses::prime_anciennete_possible(ent.idcc.portee(), ent.annexe_convention.code(), metier, statut)?;
    let prend = |r: &&Rubrique| {
        let forme = mensuelle.is_none_or(|m| if m { crate::a_une_limite(&r.code) } else { r.reservee_a() != Some("mensuelle") });
        // Une rubrique de la piece d'un salarie au mois ne se saisit pas : son calcul la compose (`rubrique_a_saisir`).
        !r.saisie.is_empty()
            && r.piece.is_none()
            && forme
            && statut.is_none_or(|s| r.reservee_au_statut().is_none_or(|v| v == s))
            && (prime || r.code != clauses::RUBRIQUE_PRIME_ANCIENNETE)
    };
    let siennes: Vec<&Rubrique> = cat.iter().filter(prend).collect();
    let par_defaut = match mensuelle {
        Some(true) => siennes.first(),
        _ => siennes.iter().find(|r| r.nature == droit::catalogue::NatureDeRubrique::Salaire),
    };
    Ok(Proposees { par_defaut: par_defaut.map(|r| r.code.clone()), rubriques: siennes.iter().map(|r| r.code.clone()).collect() })
}

/// Les rubriques d'une carte qui ne se calcule pas encore, lues sur sa saisie comme la paie les lira : sa nature si elle
/// la dit ; son metier, sinon celui de la fiche de son salarie ; son statut ecrit, sinon celui de son metier, sinon
/// celui de la fiche de son salarie.
pub fn de_la_carte(ctx: &Contexte, carte: &Carte, jour: &Jour) -> R<Proposees> {
    let mensuelle = match carte.nature {
        Some(n) => Some(n.proprietes()?.mensuelle()),
        None => None,
    };
    let fiche = carte.salarie.as_ref().and_then(|c| ctx.salaries.get(c.as_str()));
    let metier = carte.metier.as_ref().map(|m| m.to_string()).or_else(|| fiche.and_then(|f| f.code_emploi_ft.remplie()).map(str::to_string));
    let du_metier = match &carte.metier {
        Some(code) => Some(code.metier()?.statut()),
        None => None,
    };
    let de_la_fiche = fiche.and_then(|f| f.statut.dite()).copied();
    let statut = carte.statut.or(du_metier).or(de_la_fiche);
    proposees(ctx, mensuelle, statut, metier.as_deref(), jour)
}
