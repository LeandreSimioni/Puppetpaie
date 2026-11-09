//! LA CONSULTATION DES ORGANISMES, du cote de la compagnie : le mot de passe de consultation pris au coffre (regle 3 :
//! la releve d'`envois` le recoit), et les clients net-entreprises et Urssaf qu'il ouvre, dans le monde seulement.

use droit::contexte::Contexte;
use envois::branchement::Branchement;
use envois::releve_retours::Client;
use socle::echec::R;

/// Le mot de passe d'un service, pour une CONSULTATION : l'environnement d'abord, puis le coffre ; `None`
/// quand rien n'est pose. Un coffre qui ne s'ouvre pas est un refus, pas une absence.
pub fn mot_de_passe_de_consultation(ctx: &Contexte, service: &str) -> R<Option<String>> {
    let coffre = crate::compagnie::coffre::Coffre::du_poste(&ctx.racine, "");
    coffre.lire(service, crate::compagnie::coffre::CONSULTATION, false, None).map_err(crate::routes::vers_route)
}

/// Le client net-entreprises de la consultation, dans le monde seulement : `None` quand aucun mot de passe n'est pose.
pub fn client_dsn(ctx: &Contexte, branchement: Branchement, environnement: &str) -> R<Option<Client>> {
    if branchement != Branchement::Monde {
        return Ok(None);
    }
    let Some(mdp) = mot_de_passe_de_consultation(ctx, "net-entreprises")? else { return Ok(None) };
    Client::dsn(ctx, environnement, Some(mdp)).map(Some).map_err(|p| p.en_echec())
}

/// Le client Urssaf de la consultation, dans le monde seulement : `None` quand aucun mot de passe n'est pose.
pub fn client_dpae(ctx: &Contexte, branchement: Branchement) -> R<Option<Client>> {
    if branchement != Branchement::Monde {
        return Ok(None);
    }
    Ok(mot_de_passe_de_consultation(ctx, "urssaf")?.map(|mdp| Client::dpae(ctx, false, Some(mdp))))
}
