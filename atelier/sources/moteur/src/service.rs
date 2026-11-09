//! `moteur route` : la demande sur l'entree standard, la reponse de l'enveloppe des routes (`routes::enveloppe::Reponse`)
//! en une ligne de JSON sur la sortie standard (`--texte` : ses phrases) ; un refus sort en code 1. Et ce dont les routes
//! ont besoin pour lire la compagnie (`contexte`).

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::Classe;
use socle::fichiers::dossier_du_code;
use socle::num::Resultat;

use crate::routes::catalogue::Route;
use crate::routes::enveloppe::{Refus, Reponse};

pub(crate) fn options(args: &[String]) -> Option<HashMap<String, String>> {
    let mut o = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let cle = args[i].strip_prefix("--")?;
        if !["racine", "date"].contains(&cle) {
            return None;
        }
        o.insert(cle.to_string(), args.get(i + 1)?.clone());
        i += 2;
    }
    Some(o)
}

/// La compagnie d'une commande : `--racine` (`o["racine"]`, toujours pose par la fenetre et la file des etats), sinon,
/// a la ligne de commande seulement, `PUPPETPAIE_DONNEES` (`moteur route` lance a la main ou par un test).
pub(crate) fn racine_de(o: &HashMap<String, String>) -> PathBuf {
    match (o.get("racine"), std::env::var("PUPPETPAIE_DONNEES")) {
        (Some(r), _) if !r.is_empty() => PathBuf::from(r),
        (_, Ok(e)) if !e.is_empty() => PathBuf::from(e),
        _ => dossier_du_code(),
    }
}

/// La route nomme-t-elle une compagnie (`--racine`, ou `PUPPETPAIE_DONNEES`) ?
pub(crate) fn compagnie_nommee(o: &HashMap<String, String>) -> bool {
    o.get("racine").is_some_and(|r| !r.is_empty()) || matches!(std::env::var("PUPPETPAIE_DONNEES"), Ok(e) if !e.is_empty())
}

pub(crate) fn contexte(o: &HashMap<String, String>) -> Resultat<Contexte> {
    let date = o.get("date").map(|d| Jour::lire(d)).transpose()?;
    Contexte::nouveau(&racine_de(o), date)
}

/// `moteur route [--racine R] [--date D] [--texte]` : la demande sur l'entree standard. `None` : usage.
///
/// UN ACTE REEL PAR `moteur route` : la demande du mot de passe du logiciel est celle du moteur (`demande::Reel` : sa
/// fenetre, sinon le terminal de l'humain, sans echo, sinon un refus). L'entree standard est celle d'un programme (un LLM,
/// un script) : aucun secret ne s'y lit, ils sont retires de la charge (`Charge::sans_les_secrets`) et la route les demande a un humain.
pub fn commande(quoi: &str, args: &[String]) -> Option<i32> {
    if quoi != "route" {
        return None;
    }
    let texte_seul = args.iter().any(|a| a == "--texte");
    let args: Vec<String> = args.iter().filter(|a| *a != "--texte").cloned().collect();
    let o = options(&args)?;
    // Les referentiels : ceux de l'installation du programme, comme pour toute autre entree.
    crate::commandes::compagnie::fixer_le_code();
    // `--date D` est l'aujourd'hui de la route : le jour du contexte, et celui de l'horloge (`socle::temps`, le jour pose)
    // qui date ce qu'elle ecrit. Une seule horloge : chaque ligne du journal porte le jour de la route.
    // Une date qui ne se lit pas ne pose rien : le contexte de la route la refuse (`contexte`), en le disant.
    if let Some(Ok(d)) = o.get("date").map(|d| Jour::lire(d)) {
        std::env::set_var("PUPPETPAIE_AUJOURDHUI", d.iso());
    }
    let _canaux = crate::compagnie::demande::installer(crate::compagnie::demande::canal_de_la_route(&racine_de(&o)));
    let mut entree = String::new();
    let reponse = match std::io::stdin().read_to_string(&mut entree) {
        Ok(_) => crate::routes::commande(&o, entree.trim_start_matches('\u{feff}')),
        Err(e) => Reponse::refusee(Refus::nouveau(Classe::DemandeIllisible, format!("L'entrée standard ne se lit pas : {}.", e))),
    };
    // Une demande qui ne se lit pas n'a pas de route : sa reponse est deja le refus qui le dit.
    let nom = match crate::routes::enveloppe::Demande::lire(entree.trim_start_matches('\u{feff}')) {
        Ok(d) => Some(d.route),
        Err(refusee) => {
            drop(refusee);
            None
        }
    };
    // LE MESSAGE AU DEVELOPPEUR PART AUSSITOT, comme la fenetre le fait partir en fond (`interface::api`) : le processus
    // attend son depart, sans changer ce que la route a rendu.
    if nom == Some(Route::SignalementEnvoyer) && reponse.est_rendue() {
        let _ = crate::routes::servir_la_route(&o, Route::SignalementsPartir, &crate::routes::enveloppe::Charge::vide());
    }
    if texte_seul {
        // Une demande sans route lisible : son texte ne nomme aucune route.
        const SANS_ROUTE: &str = "";
        dire_sans_ligne!("{}", reponse.en_texte(match nom {
            Some(r) => r.nom(),
            None => SANS_ROUTE,
        }));
    } else {
        dire!("{}", reponse.en_json());
    }
    // UN REFUS SORT EN 1 (la reponse reste la meme) : qui se fie au code de sortie ne croit pas le geste fait. 2 : la
    // commande elle-meme est mal dite (`usage`).
    Some(if reponse.est_rendue() { 0 } else { 1 })
}
