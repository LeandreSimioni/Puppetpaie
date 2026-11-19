//! LA RELEVE DES RETOURS, du cote de la compagnie : le branchement de la compagnie (`bridage::branchement`), le client
//! de la consultation (`compagnie::consultation`), puis les suites (`suites::faire` : les incidents des retours, le
//! rangement des attestations France Travail).

use droit::contexte::Contexte;
use envois::ordre_signalements::OrdreConnu;
use envois::releve_retours::{CanalDesRetours, Client, EnvironnementNetEntreprises, PlageDuReleve, Releve};
use envois::suites::{ADire, Suites};
use socle::echec::R;
use compagnie::consultation::{client_dpae, client_dsn};

/// Fait les suites d'une releve : la releve, et tout ce qu'elle a a dire, ce qui a empeche de lire la plate-forme en
/// tete.
fn avec_les_suites(ctx: &Contexte, r: R<Releve>, s: Suites) -> R<(Releve, ADire)> {
    let faites = crate::gestes::suites::faire(ctx, s);
    let releve = r?;
    let mut dit = ADire { incidents: releve.plateforme_non_lue.clone(), a_faire: Vec::new() };
    dit.etendre(faites?);
    Ok((releve, dit))
}

/// Releve les retours du canal `canal` (la DPAE avec un client Urssaf donne) : `client` donne, ou celui de la
/// consultation net-entreprises.
pub fn relever_retours(ctx: &Contexte, plage: PlageDuReleve, tout: bool, environnement: EnvironnementNetEntreprises, client: Option<Client>, canal: CanalDesRetours) -> R<(Releve, ADire)> {
    let b = compagnie::bridage::branchement(&ctx.racine)?;
    let client = match client {
        Some(c) => Some(c),
        None => client_dsn(ctx, b, environnement)?,
    };
    let mut s = Suites::default();
    let r = envois::releve_retours::relever_retours(ctx, b, plage, tout, client, canal, &mut s);
    avec_les_suites(ctx, r, s)
}

/// Releve les retours de la DPAE : par flux s'il est connu, sinon par plage.
pub fn relever_retours_dpae(ctx: &Contexte, plage: PlageDuReleve, tout: bool, flux: Option<&envois::retours::IdDeFlux>) -> R<(Releve, ADire)> {
    let b = compagnie::bridage::branchement(&ctx.racine)?;
    let client = client_dpae(ctx, b)?;
    let mut s = Suites::default();
    let r = envois::releve_retours::relever_retours_dpae(ctx, b, plage, tout, flux, client, &mut s);
    avec_les_suites(ctx, r, s)
}

/// Le simulateur rend les retours des depots joues qui les attendent : le nombre de depots completes, et ce que leurs
/// suites ont a dire (une attestation que la paie ne confirme pas, un nom qu'aucune fiche ne porte, un rangement qui
/// echoue).
pub fn rattacher_simulation(ctx: &Contexte) -> R<(usize, ADire)> {
    let mut s = Suites::default();
    let n = envois::releve_retours::rattacher_simulation(ctx, &mut s);
    let faites = crate::gestes::suites::faire(ctx, s)?;
    Ok((n?, faites))
}

/// La plate-forme relevee avant l'ordre d'un essai de mensuelle (`envois::ordre_signalements::relever_la_plateforme`), ses
/// bilans publies depuis le premier jour de `depuis` (`envois::ordre_signalements::depuis_le_mois`), par le client de la
/// consultation de `environnement` ; les suites vont dans `suites`, que l'appelant fait avec celles du depot. Ce qui a
/// empeche de la lire, en une phrase.
pub(crate) fn relever_avant_l_ordre(ctx: &Contexte, branchement: envois::branchement::Branchement, environnement: EnvironnementNetEntreprises, depuis: socle::mois::Mois, suites: &mut Suites) -> R<Option<String>> {
    let debut = envois::ordre_signalements::depuis_le_mois(depuis)?;
    let client = client_dsn(ctx, branchement, environnement)?;
    Ok(envois::ordre_signalements::relever_la_plateforme(ctx, branchement, client, debut, suites))
}

/// Le plus grand ordre de signalement connu (`envois::ordre_signalements::dernier_ordre_connu`) : `client` donne, ou celui
/// de la consultation net-entreprises. Ce que la releve et ses suites ont a dire part avec ses avertissements.
pub(crate) fn dernier_ordre_connu(ctx: &Contexte, client: Option<Client>) -> R<OrdreConnu> {
    let b = compagnie::bridage::branchement(&ctx.racine)?;
    let client = match client {
        Some(c) => Some(c),
        None => client_dsn(ctx, b, EnvironnementNetEntreprises::Production)?,
    };
    let mut s = Suites::default();
    let r = envois::ordre_signalements::dernier_ordre_connu(ctx, b, client, &mut s);
    let faites = crate::gestes::suites::faire(ctx, s)?;
    let mut connu = r?;
    connu.avertissements.extend(faites.phrases());
    Ok(connu)
}
