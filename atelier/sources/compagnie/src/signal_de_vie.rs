//! LE SIGNAL DE VIE DU POSTE, ENVOYE A L'EDITEUR CHAQUE JOUR D'OUVERTURE : pour qu'il sache combien de postes se servent
//! du logiciel (Leandre, 26/11/2026 : « un petit compteur […] pour savoir combien de gens ont installe le logiciel »,
//! « on pourrait l'afficher sur la page du site » ; et « il y a quand meme un signal pour l'enlever si les gens veulent
//! dans le logiciel »). Le serveur compte les postes vus dans les 30 derniers jours.
//!
//! CE QUI PART : le numero du poste que portent deja les incidents, et rien d'autre : ni version, ni compagnie, ni
//! machine, ni nombre.
//!
//! QUAND : a l'ouverture ou au passage de la veille (`ouverture::ouverture`, avec les incidents et le nombre de paies),
//! au plus une fois par jour (l'horloge du moteur), par numero et par processus du moteur : le numero est celui de la
//! compagnie ouverte, et un poste qui en tient plusieurs envoie celui de chacune ; ce qui est parti se retient dans le
//! processus, jamais dans un fichier de la compagnie. Un envoi qui echoue (hors ligne, serveur en panne) ne retient
//! rien : le passage suivant le refait.
//!
//! PAR OU : le canal des incidents (`incidents::canal`, l'identite du poste de `config/diagnostic.json`, la cle dans
//! l'en-tete), action `vie` du serveur de l'editeur ; SOUS LE MEME INTERRUPTEUR (`incidents::envoi`) : coupe, rien ne
//! part. L'ecran « Ce qui est parti chez l'editeur » le dit (`routes::filet::diagnostic_etat`).
//!
//! JAMAIS : depuis une compagnie fictive ni depuis la copie de developpement, ni depuis le binaire de l'atelier
//! (`nombre_de_paies::empechement`, le meme empechement). Un logiciel bride, lui, envoie : une installation en demo est
//! une installation.

use std::sync::Mutex;

use droit::contexte::Contexte;
use socle::date::Jour;

use crate::incidents::{self, Canal, NumeroDePoste};

/// Le jour, et les numeros dont le signal est parti ce jour depuis ce processus : le numero est celui de la compagnie
/// ouverte (`config/diagnostic.json`), et un poste qui en tient plusieurs envoie celui de chacune.
static PARTIS: Mutex<Option<(Jour, Vec<NumeroDePoste>)>> = Mutex::new(None);

/// Ce que l'ecran « Ce qui est parti chez l'editeur » dit du signal de vie de la compagnie de `racine`, selon ce qui
/// l'empeche (`nombre_de_paies::empechement`, le meme qu'a l'envoi) : l'interrupteur coupe, ou une compagnie qui ne
/// compte pas (fictive, copie de developpement).
pub fn ce_qui_part(racine: &std::path::Path) -> socle::echec::R<&'static str> {
    Ok(match crate::nombre_de_paies::empechement(racine)? {
        None => "Chaque jour d'ouverture : le numéro de ce poste, pour que l'éditeur sache combien de postes se servent du logiciel.",
        Some(crate::nombre_de_paies::ENVOI_COUPE) => {
            "Chaque jour d'ouverture, le numéro de ce poste partirait, pour que l'éditeur sache combien de postes se servent du logiciel : l'envoi est coupé, rien ne part."
        }
        Some(_) => "Depuis cette compagnie d'essai, rien ne part : seule une compagnie réelle est comptée par l'éditeur.",
    })
}

/// A l'ouverture et a chaque passage de la veille : envoie le signal de vie du poste s'il n'est pas deja parti ce jour
/// depuis ce processus. Ne leve jamais (l'ouverture n'echoue pas) : rend ce qui s'est passe -- `erreur` non vide quand le
/// passage suivant recommencera.
pub fn envoyer_si_du(ctx: &Contexte) -> BilanDeVie {
    let bilan = |envoye: bool, raison: &str, erreur: &str| BilanDeVie {
        envoye,
        raison: (!raison.is_empty()).then(|| raison.to_string()),
        erreur: (!erreur.is_empty()).then(|| erreur.to_string()),
    };
    match crate::nombre_de_paies::empechement(&ctx.racine) {
        Ok(Some(r)) => return bilan(false, r, ""),
        Ok(None) => {}
        Err(e) => return bilan(false, "", &e.phrase),
    }
    // Le verrou est tenu jusqu'a la fin : deux passages du meme jour ne partent pas ensemble. Un verrou empoisonne garde
    // sa valeur : ce qui est retenu reste juste.
    let mut partis = match PARTIS.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if partis.as_ref().is_some_and(|(jour, _)| *jour != ctx.aujourdhui) {
        *partis = None;
    }
    let client = match incidents::canal(&ctx.racine) {
        Ok(Canal::Pret(c)) => c,
        Ok(Canal::Panne(e)) => return bilan(false, "", &e.dite()),
        Err(e) => return bilan(false, "", &e.phrase),
    };
    let numero = client.poste.0.clone();
    if partis.as_ref().is_some_and(|(_, v)| v.contains(&numero)) {
        return bilan(false, "deja parti ce jour", "");
    }
    match client.signaler_la_vie() {
        Ok(crate::editeur::Vie { ok: true }) => {
            partis.get_or_insert_with(|| (ctx.aujourdhui, Vec::new())).1.push(numero);
            bilan(true, "", "")
        }
        Ok(crate::editeur::Vie { ok: false }) => bilan(false, "", "le serveur n'a pas noté le signal"),
        Err(e) => bilan(false, "", &e.dite()),
    }
}

/// Ce que rend un passage du signal de vie (`envoyer_si_du`) : s'il est parti, pourquoi rien n'est parti, et l'erreur
/// quand le passage suivant recommencera.
#[derive(Debug, Clone)]
pub struct BilanDeVie {
    pub envoye: bool,
    pub raison: Option<String>,
    pub erreur: Option<String>,
}
