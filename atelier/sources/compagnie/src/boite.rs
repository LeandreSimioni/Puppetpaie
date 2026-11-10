//! LA BOITE COURRIEL DE LA COMPAGNIE : son adresse et son mot de passe d'application se lisent au coffre (service
//! `courriel`, en CONSULTATION : un courriel n'est pas une declaration, le garde-fou d'un envoi est le mot de passe du
//! logiciel exige avant l'envoi), et le client d'`envois::courriel` les recoit (regle 3 : le transport n'ouvre jamais le
//! coffre).

use std::path::Path;

use crate::coffre::{Coffre, CONSULTATION};
use droit::contexte::Contexte;
use envois::courriel::Client;
use socle::echec::{Classe, Echec, R};

/// Le service du coffre qui porte le mot de passe de la boite et son adresse.
pub const SERVICE: &str = "courriel";

fn coffre_de(racine: &Path) -> Coffre {
    Coffre::du_poste(racine, "")
}

/// La boite ne s'ouvre pas : un refus de classe `Courriel`.
#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    socle::echec::erreur(Classe::Courriel, m)
}

/// L'adresse de la boite, posee avec le mot de passe (`COURRIEL_COMPTE`, puis le coffre), en minuscules ; `None` si
/// aucune n'est posee.
pub fn adresse(racine: &Path) -> R<Option<String>> {
    match std::env::var("COURRIEL_COMPTE") {
        Ok(v) if !v.is_empty() => return Ok(Some(v.trim().to_lowercase())),
        Ok(_) | Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(_)) => {}
    }
    let du_coffre = coffre_de(racine).compte_pose(SERVICE).map_err(crate::en(Classe::Courriel))?;
    Ok(du_coffre.map(|a| a.trim().to_lowercase()).filter(|a| !a.is_empty()))
}

/// Peut-on ouvrir la boite sans rien demander ? Adresse ET secret ; ce qui ne se lit pas le refuse.
pub fn disponible(racine: &Path) -> R<bool> {
    match adresse(racine)? {
        Some(_) => coffre_de(racine).disponible(SERVICE).map_err(crate::en(Classe::Courriel)),
        None => Ok(false),
    }
}

/// Le mot de passe, lu en CONSULTATION.
pub fn mot_de_passe(racine: &Path) -> R<String> {
    let lu = coffre_de(racine).lire(SERVICE, CONSULTATION, false, None).map_err(crate::en(Classe::Courriel))?;
    match lu {
        Some(m) if !m.is_empty() => Ok(m.replace(' ', "")),
        _ => Err(refus("Aucun mot de passe de boite courriel dans le coffre.\n  route coffre-poser, service courriel, compte adresse@...")),
    }
}

/// Le vrai client de la boite, adresse et mot de passe lus au coffre.
pub fn client(racine: &Path) -> R<Client> {
    let Some(adresse) = adresse(racine)? else {
        return Err(refus("Aucune adresse de boite courriel posee avec le mot de passe.\n  route coffre-poser, service courriel, compte adresse@..."));
    };
    Client::nouveau(racine, &adresse, &mot_de_passe(racine)?)
}

/// Le vrai client de la boite de la compagnie.
pub fn client_du_contexte(ctx: &Contexte) -> R<Client> {
    client(&ctx.racine)
}

/// CE QU'UNE TENTATIVE D'OUVRIR LA BOITE DIT : si elle s'ouvre, la premiere ligne de ce qu'elle a repondu, et ce qu'il
/// faut faire quand rien n'est pose ; jamais le mot de passe.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Ouverture {
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conseil: Option<&'static str>,
}

/// La boite s'ouvre-t-elle avec ce qui est pose ? UNE tentative IMAP (`envois::courriel::verifier`).
pub fn verifier(racine: &Path) -> R<Ouverture> {
    if adresse(racine)?.is_none() {
        return Ok(Ouverture { ok: false, message: "Aucune adresse posee avec le mot de passe.".into(), conseil: Some("route coffre-poser, service courriel, compte adresse@...") });
    }
    match client(racine).and_then(envois::courriel::verifier) {
        Ok(message) => Ok(Ouverture { ok: true, message, conseil: None }),
        Err(e) => Ok(Ouverture { ok: false, message: socle::texte::lignes_du_texte(&e.phrase).first().map(|l| l.to_string()).into_iter().collect(), conseil: None }),
    }
}
