//! LA RELEVE DES RETOURS, du cote de la compagnie : le branchement de la compagnie (`bridage::branchement`), le client
//! de la consultation (`compagnie::consultation`), puis les suites (`suites::faire` : les incidents des retours, le
//! rangement des attestations France Travail).

use droit::contexte::Contexte;
use envois::ordre_signalements::OrdreConnu;
use envois::releve_retours::{Client, Releve};
use envois::suites::Suites;
use socle::echec::{Echec, R};
use compagnie::consultation::{client_dpae, client_dsn};

/// Fait les suites d'une releve et les reporte sur elle.
fn avec_les_suites(ctx: &Contexte, r: R<Releve>, s: Suites) -> R<Releve> {
    let faites = crate::gestes::suites::faire(ctx, s);
    let mut releve = r?;
    let faites = faites?;
    releve.a_faire.extend(faites.a_faire);
    releve.incidents.extend(faites.incidents);
    Ok(releve)
}

/// Releve les retours DSN (ou, `canal = Some("dpae")` avec un client Urssaf donne, ceux de la DPAE) : `client` donne, ou
/// celui de la consultation net-entreprises. `canal` : celui de la DSN s'il n'est pas dit.
pub fn relever_retours(ctx: &Contexte, debut: i64, fin: i64, tout: bool, environnement: &str, client: Option<Client>, canal: Option<&str>) -> R<Releve> {
    let canal = match canal {
        Some(c) => c,
        None => envois::releve_retours::CANAL_DSN,
    };    let b = compagnie::bridage::branchement(&ctx.racine)?;
    let client = match client {
        Some(c) => Some(c),
        None => client_dsn(ctx, b, environnement)?,
    };
    let mut s = Suites::default();
    let r = envois::releve_retours::relever_retours(ctx, b, debut, fin, tout, client, canal, &mut s);
    avec_les_suites(ctx, r, s)
}

/// Releve les retours de la DPAE : par flux s'il est connu, sinon par plage.
pub fn relever_retours_dpae(ctx: &Contexte, debut: i64, fin: i64, tout: bool, flux: Option<&str>) -> R<Releve> {
    let b = compagnie::bridage::branchement(&ctx.racine)?;
    let client = client_dpae(ctx, b)?;
    let mut s = Suites::default();
    let r = envois::releve_retours::relever_retours_dpae(ctx, b, debut, fin, tout, flux, client, &mut s);
    avec_les_suites(ctx, r, s)
}

/// Le simulateur rend les retours des depots joues qui les attendent : le nombre de depots
/// completes, et ce que le rangement des AER jouees laisse a faire (une attestation que la paie ne confirme pas, un nom
/// qu'aucune fiche ne porte). Un rangement qui echoue est un refus.
pub fn rattacher_simulation(ctx: &Contexte) -> R<(i64, Vec<String>)> {
    let mut s = Suites::default();
    let n = envois::releve_retours::rattacher_simulation(ctx, &mut s);
    let faites = crate::gestes::suites::faire(ctx, s)?;
    if let Some(i) = faites.incidents.into_iter().next() {
        return Err(Echec::from(i));
    }
    Ok((n?, faites.a_faire))
}

/// Le plus grand ordre de signalement connu (`envois::ordre_signalements::dernier_ordre_connu`) : `client` donne, ou celui
/// de la consultation net-entreprises.
pub(crate) fn dernier_ordre_connu(ctx: &Contexte, client: Option<Client>) -> R<OrdreConnu> {
    let b = compagnie::bridage::branchement(&ctx.racine)?;
    let client = match client {
        Some(c) => Some(c),
        None => client_dsn(ctx, b, "production")?,
    };
    let mut s = Suites::default();
    let r = envois::ordre_signalements::dernier_ordre_connu(ctx, b, client, &mut s);
    crate::gestes::suites::faire(ctx, s)?;
    r
}
