//! `coffre-verifier`.
//!
//! Le secret pose ouvre-t-il une session chez l'organisme ? UNE SEULE TENTATIVE,
//! JAMAIS DE BOUCLE : au-dela de quelques echecs, net-entreprises bloque le compte (403). Trois services :
//! net-entreprises (la DSN), l'Urssaf (la DPAE : un autre compte, un autre mot de passe) et la boite courriel (une
//! connexion IMAP). Le verdict est rendu avec la premiere ligne de l'erreur, qui ne contient jamais le mot de passe.
//!
//! Le mot de passe est lu au coffre pour une CONSULTATION (l'environnement d'abord) et va directement au client : il
//! ne sort jamais d'ici, ni dans la reponse ni dans une erreur.
//!
//! `verifier_secret_avec` prend de quoi authentifier et de quoi ouvrir la boite : les tests y passent un faux, sans
//! reseau. `verifier_secret` ouvre les vraies connexions.

use socle::echec::Classe;
use socle::ecran::Pastille;

use crate::compagnie::boite::Ouverture;
use crate::compagnie::coffre::Coffre;
use droit::contexte::Contexte;
use socle::echec::erreur;
use crate::compagnie::consultation::mot_de_passe_de_consultation;
use envois::releve_retours::{Client, Panne};
use socle::echec::Echec;


type R<T> = Result<T, Echec>;

pub const NET_ENTREPRISES: &str = "net-entreprises";
pub const URSSAF: &str = "urssaf";
pub const COURRIEL: &str = "courriel";

/// De quoi ouvrir une session chez net-entreprises ou l'Urssaf : le service, et son mot de passe (`None` quand rien
/// n'est pose ; la variable d'environnement a deja ete consultee).
pub type Authentifier<'a> = &'a mut dyn FnMut(&Contexte, &str, Option<String>) -> Result<(), Panne>;

/// De quoi ouvrir la boite : `courriel.verifier` (une seule connexion IMAP).
pub type OuvrirBoite<'a> = &'a mut dyn FnMut(&Contexte) -> Result<Ouverture, envois::courriel::ErreurCourriel>;

/// LE VERDICT D'UNE TENTATIVE : le service essaye, si la session s'ouvre, la premiere ligne de ce qu'il a repondu, ce
/// qu'il faut en faire quand la reponse le dit, et sa pastille (la session s'ouvre, fait ; l'organisme refuse, refuse).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Verdict {
    pub service: &'static str,
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conseil: Option<&'static str>,
    pub pastille: Pastille,
}

impl Verdict {
    fn de(service: &'static str, ok: bool, message: String, conseil: Option<&'static str>) -> Verdict {
        let pastille = if ok { Pastille::de("fait", "OK") } else { Pastille::de("refuse", "refusé") };
        Verdict { service, ok, message, conseil, pastille }
    }
}

/// La route `coffre-verifier` : le secret pose du service `service` ouvre-t-il une session.
pub fn route(ctx: &Contexte, service: &str) -> R<Verdict> {
    verifier_secret(ctx, service)
}

/// Le secret pose ouvre-t-il une session : `verifier_secret_avec`, avec les vraies connexions.
pub fn verifier_secret(ctx: &Contexte, service: &str) -> R<Verdict> {
    verifier_secret_avec(ctx, service, &mut authentifier_pour_de_vrai, &mut |c| crate::compagnie::boite::verifier(&c.racine))
}

/// L'authentification d'un service : le client de la DSN, ou celui de la DPAE (un autre compte).
fn authentifier_pour_de_vrai(ctx: &Contexte, service: &str, mdp: Option<String>) -> Result<(), Panne> {
    let mut client = if service == NET_ENTREPRISES { Client::dsn(ctx, "production", mdp)? } else { Client::dpae(ctx, false, mdp) };
    client.authentifier()
}

/// Le secret pose d'un service ouvre-t-il une session ? Pour la boite, ce que `courriel.verifier` en dit.
pub fn verifier_secret_avec(ctx: &Contexte, service: &str, authentifier: Authentifier, boite: OuvrirBoite) -> R<Verdict> {
    let nom = match [NET_ENTREPRISES, URSSAF, COURRIEL].into_iter().find(|s| *s == service) {
        Some(s) => s,
        None => {
            return Err(erreur(Classe::Refuse, format!("Service inconnu : « {} ». Connus : {}, {}, {}.", service, NET_ENTREPRISES, URSSAF, COURRIEL)));
        }
    };
    let coffre = Coffre::du_poste(&ctx.racine, "");
    if !coffre.disponible(nom).map_err(crate::compagnie::coffre::vers_route)? {
        return Err(erreur(Classe::Refuse, format!("Aucun mot de passe {} : rien à vérifier.", nom)));
    }
    if nom == COURRIEL {
        // La boite : une connexion IMAP, une seule (`courriel.verifier`).
        let o = boite(ctx).map_err(|e| erreur(Classe::Courriel, e.0))?;
        return Ok(Verdict::de(nom, o.ok, o.message, o.conseil));
    }
    let mdp = mot_de_passe_de_consultation(ctx, nom)?;
    match authentifier(ctx, nom, mdp) {
        Ok(()) => Ok(Verdict::de(nom, true, "session ouverte".into(), None)),
        Err(p) if p.classe == Classe::Organisme => {
            let premiere: String = socle::texte::lignes_du_texte(&p.message).first().map(|l| l.to_string()).into_iter().collect();
            let conseil = if premiere.contains("422") {
                Some("Le service répond 422 (paramètres invalides) : le plus souvent un mot de passe tronqué à la saisie, pas un compte fautif. Reposez-le.")
            } else if premiere.contains("403") {
                Some("Le service répond 403 : le compte est bloqué un temps (trop de tentatives). N'essayez plus, attendez.")
            } else {
                None
            };
            Ok(Verdict::de(nom, false, premiere, conseil))
        }
        Err(autre) => Err(autre.en_echec()),
    }
}
