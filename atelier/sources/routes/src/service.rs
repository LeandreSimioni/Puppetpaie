//! `moteur route` : la demande sur l'entree standard, la reponse de l'enveloppe des routes (`routes::enveloppe::Reponse`)
//! en une ligne de JSON sur la sortie standard (`--texte` : ses phrases) ; un refus sort en code 1. Et ce dont les routes
//! ont besoin pour lire la compagnie (`contexte`).

use std::io::Read;
use std::path::PathBuf;

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::{erreur, Classe, R};
use socle::fichiers::dossier_du_code;
use socle::num::Resultat;

use crate::catalogue::Route;
use crate::enveloppe::{Options, Refus, Reponse};

/// Les options de `moteur route` (`--racine R`, `--date D`), en leur forme. `None` : la commande est mal dite (usage) ;
/// une date qui ne se lit pas refuse la route (`Illisible`), rien ne s'ecrit.
pub(crate) fn options(args: &[String]) -> Option<R<Options>> {
    let mut o = Options::sans_compagnie();
    let mut i = 0;
    while i < args.len() {
        let valeur = args.get(i + 1)?;
        match args[i].as_str() {
            "--racine" => o.racine = Some(PathBuf::from(valeur)).filter(|r| !r.as_os_str().is_empty()),
            "--date" => match Jour::lire(valeur) {
                Ok(d) => o.date = Some(d),
                Err(m) => return Some(Err(erreur(Classe::Illisible, m))),
            },
            _ => return None,
        }
        i += 2;
    }
    Some(Ok(o))
}

/// La compagnie d'une commande : `--racine` (toujours pose par la fenetre et la file des etats), sinon, a la ligne de
/// commande seulement, `PUPPETPAIE_DONNEES` (`moteur route` lance a la main ou par un test).
pub(crate) fn racine_de(o: &Options) -> PathBuf {
    match (&o.racine, std::env::var("PUPPETPAIE_DONNEES")) {
        (Some(r), _) => r.clone(),
        (None, Ok(e)) if !e.is_empty() => PathBuf::from(e),
        (None, _) => dossier_du_code(),
    }
}

/// La route nomme-t-elle une compagnie (`--racine`, ou `PUPPETPAIE_DONNEES`) ?
pub(crate) fn compagnie_nommee(o: &Options) -> bool {
    o.racine.is_some() || matches!(std::env::var("PUPPETPAIE_DONNEES"), Ok(e) if !e.is_empty())
}

/// La compagnie de la route, au jour de la route.
pub(crate) fn contexte(o: &Options) -> Resultat<Contexte> {
    Contexte::nouveau(&racine_de(o), o.date).map_err(String::from)
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
    let lues = options(&args)?;
    // Les referentiels : ceux de l'installation du programme, comme pour toute autre entree.
    compagnie::installation::fixer_le_code();
    let mut entree = String::new();
    let lue = std::io::stdin().read_to_string(&mut entree);
    let o = match lues {
        Ok(o) => o,
        Err(e) => {
            // Une date qui ne se lit pas : la route ne se sert pas, sa reponse est le refus qui le dit.
            let refusee = Reponse::refusee(Refus::de(None, e));
            if texte_seul {
                dire_sans_ligne!("{}", refusee.en_texte(""));
            } else {
                dire!("{}", refusee.en_json());
            }
            return Some(1);
        }
    };
    // `--date D` est l'aujourd'hui de la route : le jour du contexte, et celui de l'horloge (`socle::temps`, le jour pose)
    // qui date ce qu'elle ecrit. Une seule horloge : chaque ligne du journal porte le jour de la route.
    if let Some(d) = o.date {
        std::env::set_var("PUPPETPAIE_AUJOURDHUI", d.iso());
    }
    let _canaux = compagnie::demande::installer(compagnie::demande::canal_de_la_route(&racine_de(&o)));
    let reponse = match lue {
        Ok(_) => crate::commande(&o, entree.trim_start_matches('\u{feff}')),
        Err(e) => Reponse::refusee(Refus::nouveau(Classe::DemandeIllisible, format!("L'entrée standard ne se lit pas : {}.", e))),
    };
    // Une demande qui ne se lit pas n'a pas de route : sa reponse est deja le refus qui le dit.
    let nom = match crate::enveloppe::Demande::lire(entree.trim_start_matches('\u{feff}')) {
        Ok(d) => Some(d.route),
        Err(refusee) => {
            drop(refusee);
            None
        }
    };
    // LE MESSAGE AU DEVELOPPEUR PART AUSSITOT, comme la fenetre le fait partir en fond (`interface::api`) : le processus
    // attend son depart, sans changer ce que la route a rendu.
    if nom == Some(Route::SignalementEnvoyer) && reponse.est_rendue() {
        let _ = crate::servir_la_route(&o, Route::SignalementsPartir, &crate::enveloppe::Charge::vide());
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
