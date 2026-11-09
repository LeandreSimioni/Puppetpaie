//! `/api/<route>` : ce que la page demande, rendu dans l'enveloppe des routes (`routes::enveloppe::Reponse`), la meme
//! que `moteur route`.
//!
//! LES ROUTES TOURNENT DANS LE PROCESSUS DE LA FENETRE :
//! `route`, et pourquoi rien ne reste a part. Rien d'autre ne passe ici que les routes du catalogue : les gestes du poste
//! (ouvrir, choisir un dossier) et la veille sont ceux de la fenetre (`poste.rs`, `Fenetre::lanceur`).

use std::path::Path;

use serde_json::{json, Map, Value};

use socle::echec::Classe;

use super::veille::Veille;
use crate::etat_garde::FileDesEtats;
use crate::routes::catalogue::Route;
use crate::routes::enveloppe::{Charge, Refus, Rendu, Reponse};

/// Les refus qui ne sont pas des defauts du logiciel : une saisie en cours, un mot de passe mal tape, un mois sans
/// paie, et ceux que la page traite elle-meme (une fiche refusee, un depot deja fait...). On ne les compte pas en incidents.
const PAS_DES_INCIDENTS: [Classe; 7] = [
    Classe::SaisieIncomplete,
    Classe::PreuveInsuffisante,
    Classe::AucunDocument,
    Classe::FicheRefusee,
    Classe::DsnDejaDeposee,
    Classe::RepetitionManquante,
    Classe::ModificationNonDeclarable,
];

/// Une route du moteur, servie ICI, dans le processus de la fenetre :
/// sous Windows, un processus par route coutait 120 a 260 ms de plus a chaque appel (l'antivirus inspecte chaque
/// lancement et chaque lecture d'un processus neuf), et les baremes, referentiels et catalogues se relisaient a chaque
/// fois ; ici ils restent en memoire tant que leur fichier ne change pas (`fichiers::json_en_cache` et les caches qui
/// s'en servent). Rend son `resultat`, ou son refus.
///
/// Rien ne reste dans un processus a part. Ce que le processus separe protegeait est tenu ici : une panique rend un
/// refus et la fenetre reste (`routes::dans_la_fenetre`, `panic = "unwind"` en `release`) ; le mot de passe tape est
/// oublie a la fin de sa route, quoi qu'il arrive ; la compagnie vient de `racine`, jamais d'une variable du processus
/// (deux compagnies peuvent etre servies en meme temps par deux fils) ; les sorties reseau passent par `garde_reseau`
/// aussi. Seul `maj::controler` lance encore un processus : il eprouve le NOUVEAU programme, qui n'est pas celui-ci.
pub fn route(racine: &Path, nom: Route, charge: &Charge) -> Result<Value, Refus> {
    reponse(racine, nom, charge).en_resultat()
}

/// La reponse entiere d'une route servie ici (`route`).
fn reponse(racine: &Path, nom: Route, charge: &Charge) -> Reponse {
    // LE FIL (`diagnostic`) : une route que la page n'a pas demandee (la veille, le filet, le fond) s'y note ici ; celle de
    // la page se note dans `repondre`, avec ce que la page a fait avant.
    if DANS_LA_PAGE.with(std::cell::Cell::get) {
        return route_seule(racine, nom, charge);
    }
    let debut = std::time::Instant::now();
    let r = route_seule(racine, nom, charge);
    crate::compagnie::diagnostic::noter_route(racine, "fond", nom, charge.brute(), debut.elapsed(), issue(&r));
    r
}

/// L'issue d'une reponse telle que le fil la note : rien, ou la classe et la phrase du refus.
fn issue(r: &Reponse) -> Result<(), (&str, &str)> {
    match r.refus() {
        Some(x) => Err((socle::echec::classe_notee(x.classe), x.phrase.as_str())),
        None => Ok(()),
    }
}

thread_local! {
    /// Vrai le temps d'une requete de la page (`repondre`) : ses routes internes ne se notent pas une a une dans le fil.
    static DANS_LA_PAGE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn route_seule(racine: &Path, nom: Route, charge: &Charge) -> Reponse {
    let mut o = std::collections::HashMap::new();
    o.insert("racine".to_string(), racine.to_string_lossy().into_owned());
    // LE BANC D'ESSAI (l'atelier seulement) : chaque route que la fenetre sert -- la page, la veille, le fil de fond --
    // passe par l'enregistreur, qui garde celles qu'il enregistre dans le scenario, avec leur charge et leur instant
    // (`scenario::Enregistreur::route`).
    #[cfg(feature = "atelier")]
    if let Some(e) = crate::atelier::scenario::enregistreur_actif() {
        return e.route(racine, nom, charge, || crate::routes::dans_la_fenetre(&o, nom, charge));
    }
    crate::routes::dans_la_fenetre(&o, nom, charge)
}

/// « Un probleme, une idee ? » part en fond : `signalement-envoyer` a enregistre le message et
/// rendu la main ; `signalements-partir` le fait partir, le temps du serveur de l'editeur, sans que la page attende.
fn signalements_en_fond(racine: &Path) {
    let r = racine.to_path_buf();
    crate::routes::fil_de_routes(move || {
        let _ = route(&r, Route::SignalementsPartir, &Charge::vide());
    });
}

/// Le texte d'une charge a la cle `cle`, ou le refus de la demande qui ne le porte pas.
fn texte(charge: &Charge, cle: &str) -> Result<String, Refus> {
    charge.brute().get(cle).and_then(Value::as_str).map(String::from).ok_or_else(|| Refus::nouveau(Classe::Manque, socle::fichiers::absent(cle)))
}

/// Un refus du moteur, tel que la page le lit, pour la route `nom`.
fn refus_de(nom: Route, e: socle::echec::Echec) -> Refus {
    Refus::de(Some(nom), e)
}

fn traiter(racine: &Path, nom: Route, charge: &Charge, veille: Option<&Veille>, etats: Option<&FileDesEtats>) -> Reponse {
    let rendu = |r: Result<Value, Refus>| match r {
        Ok(v) => Reponse::de(nom, Ok(Rendu::brut(v))),
        Err(x) => Reponse::refusee_par(nom, x),
    };
    match nom {
        // `xlsx` et `pdf` : deux faits du poste, que le moteur constate lui-meme.
        Route::ExportsPeriodes => {
            let mut c = match charge.brute() {
                Value::Object(m) => m.clone(),
                _ => Map::new(),
            };
            c.insert("xlsx".into(), Value::Bool(true));
            c.insert("pdf".into(), Value::Bool(true));
            match Charge::de(Value::Object(c)) {
                Ok(c) => reponse(racine, nom, &c),
                Err(e) => Reponse::de(nom, Err(e)),
            }
        }
        Route::SignalementEnvoyer => {
            let r = reponse(racine, nom, charge);
            if r.est_rendue() {
                signalements_en_fond(racine);
            }
            r
        }
        Route::Rappels => reponse(racine, nom, &Charge::vide()),
        // L'ECRAN D'UN MOIS SE LIT ICI, SANS PROCESSUS : le dernier etat connu du
        // mois, tout de suite, meme perime (marque `perime`, et refait en fond) ; `attendre` : l'etat frais.
        Route::Mois => match (texte(charge, "mois"), etats) {
            (Err(x), _) => Reponse::refusee_par(nom, x),
            (Ok(mois), Some(f)) => rendu(f.ecran(&mois, charge.brute().get("attendre").and_then(Value::as_bool) == Some(true)).map_err(|e| refus_de(nom, e))),
            (Ok(_), None) => reponse(racine, nom, charge),
        },
        // L'OUVERTURE D'UNE COMPAGNIE, D'UN COUP ET SANS PROCESSUS : le dernier etat connu de ce que la page lit
        // en s'ouvrant (`etat_garde::FileDesEtats::ouverture`), et, lus a chaque fois a cote, ce qui vit dans la memoire de
        // la fenetre (la veille, la mise a jour) et les signalements (que l'etat garde ne suit pas).
        Route::Ouverture => {
            let Some(f) = etats else {
                return Reponse::refusee_par(nom, Refus::nouveau(Classe::Refuse, "Pas d'ouverture sans la fenetre de PuppetPaie."));
            };
            let frais = charge.brute().get("frais").and_then(Value::as_bool) == Some(true);
            let patienter = std::time::Duration::from_millis(charge.brute().get("patienter").and_then(Value::as_u64).unwrap_or(400).min(5000));
            // La veille se lit pendant que l'etat garde se lit (elle demande au coffre ce qui est pose).
            let (lu, etat_veille) = std::thread::scope(|s| {
                let v = s.spawn(|| (!frais).then(|| veille.map(Veille::etat)).flatten());
                let r = f.ouverture(patienter, frais);
                (r, v.join().ok().flatten())
            });
            let mut v = match lu {
                Ok(v) => v,
                Err(e) => return Reponse::de(nom, Err(e)),
            };
            if v.get("pret").and_then(Value::as_bool) == Some(true) && !frais {
                v["veille"] = etat_veille.unwrap_or_else(|| json!({"minutes": 0, "dernier": {}, "en_cours": false}));
                v["maj"] = crate::compagnie::maj::RECHERCHE.etat();
                v["signalements"] = Reponse::de(Route::Signalements, crate::routes::filet::signalements_route(racine).and_then(crate::routes::enveloppe::en_json)).en_json();
            }
            rendu(Ok(v))
        }
        _ => reponse(racine, nom, charge),
    }
}

/// La reponse a `/api/<nom>`, dans l'enveloppe des routes.
pub fn repondre(racine: &Path, nom: Route, charge: &Charge, veille: Option<&Veille>, etats: Option<&FileDesEtats>) -> Reponse {
    let debut = std::time::Instant::now();
    let debut_ms = socle::temps::maintenant_ms();
    let avant = DANS_LA_PAGE.with(|d| d.replace(true));
    let rendu = traiter(racine, nom, charge, veille, etats);
    DANS_LA_PAGE.with(|d| d.set(avant));
    let duree = debut.elapsed();
    // LE FIL : la route de la page, sa charge mise en forme, sa duree, son issue. Une erreur de la page (`incident-navigateur`)
    // s'y dit comme telle.
    if nom == Route::IncidentNavigateur {
        let c = |k: &str| charge.brute().get(k).map(crate::valeur::valeur_en_texte).unwrap_or_default();
        crate::compagnie::diagnostic::noter_moteur(&format!(
            "erreur JS {} : {} ({}:{})",
            c("genre"),
            crate::compagnie::diagnostic::texte_en_forme(Some(racine), &c("message"), 200),
            crate::compagnie::diagnostic::texte_en_forme(Some(racine), &c("fichier"), 80),
            c("ligne")
        ));
    } else {
        crate::compagnie::diagnostic::noter_route(racine, "page", nom, charge.brute(), duree, issue(&rendu));
    }
    // APRES UNE ECRITURE, les etats de tous les mois se verifient en fond, le mois affiche
    // d'abord : une ecriture d'un mois peut changer les suivants (un contrat en cours, une numerotation, un cumul). Meme
    // refusee, une ecriture a pu toucher un fichier ; un mois deja juste ne coute que son empreinte. La reponse le dit
    // (`ecrit`) : la page oublie alors ce qu'elle a lu.
    if let Some(f) = etats {
        if crate::routes::est_une_ecriture(nom) {
            f.tout_verifier();
        }
    }
    if let Some(r) = rendu.refus() {
        if !r.classe.is_some_and(|c| PAS_DES_INCIDENTS.contains(&c)) && nom != Route::IncidentNavigateur {
            // DE QUOI REJOUER LA ROUTE : sa charge mise en forme,
            // sa duree, l'etat du mois qu'elle touche, ce que le refus portait pour la page ; et la trace -- l'endroit du
            // code et la chaine des echecs. Le fil et la machine s'y ajoutent dans `incidents::noter`.
            let mut detail = Map::new();
            detail.insert("route".into(), json!(nom));
            detail.insert("charge".into(), crate::compagnie::diagnostic::en_forme(Some(racine), charge.brute()));
            detail.insert("duree_ms".into(), json!(duree.as_millis() as u64));
            if let Some(m) = charge.brute().get("mois").and_then(Value::as_str) {
                detail.insert("mois".into(), crate::compagnie::diagnostic::etat_du_mois(racine, m));
            }
            if !r.a_part.est_vide() {
                detail.insert("refus".into(), crate::compagnie::diagnostic::en_forme(Some(racine), &socle::ecriture::en_valeur(&r.a_part)));
            }
            let trace = crate::compagnie::diagnostic::trace_du_refus(racine, &r.phrase, debut_ms);
            let _ = crate::compagnie::incidents::depuis_exception(racine, r.classe, &r.phrase, None, nom.nom(), Value::Object(detail), trace.as_deref());
        }
    }
    rendu
}

/// UNE ROUTE DE L'INSTALLATION (`routes::accueil` : l'accueil des compagnies), servie sans compagnie : l'installation est
/// celle du programme.
pub fn route_de_l_installation(nom: Route, charge: &Charge) -> Reponse {
    let o = std::collections::HashMap::new();
    // LE BANC D'ESSAI (l'atelier seulement) : une compagnie ajoutee entre dans le scenario comme le decor de depart que le
    // rejeu refait (`scenario::rejouer`, le geste `ajouter`).
    #[cfg(feature = "atelier")]
    if nom == Route::CompagnieAjouter {
        if let Some(e) = crate::atelier::scenario::enregistreur_actif() {
            let rendu = e.geste("ajouter", || {
                let v = crate::routes::dans_la_fenetre(&o, nom, charge).en_resultat().map_err(|r| r.phrase)?;
                let mut m = Map::new();
                m.insert("nom".into(), charge.brute().get("nom").cloned().unwrap_or(Value::Null));
                if let Some(venue) = charge.brute().get("venue").filter(|v| v.as_str() != Some("neuve")) {
                    m.insert("venue".into(), venue.clone());
                }
                let racine = std::path::PathBuf::from(v.get("chemin").and_then(Value::as_str).unwrap_or_default());
                m.insert("compagnie".into(), json!(e.compagnie(&racine)));
                Ok((v, m))
            });
            return match rendu {
                Ok(v) => Reponse::de(nom, Ok(Rendu::brut(v))),
                Err(m) => Reponse::refusee_par(nom, Refus::nouveau(Classe::Refuse, m)),
            };
        }
    }
    let rendu = crate::routes::dans_la_fenetre(&o, nom, charge);
    // Le message ecrit a l'accueil part en fond, comme celui d'une compagnie (`signalements_en_fond`).
    if nom == Route::SignalementEnvoyer && rendu.est_rendue() {
        crate::routes::fil_de_routes(move || {
            let _ = crate::routes::dans_la_fenetre(&std::collections::HashMap::new(), Route::SignalementsPartir, &Charge::vide());
        });
    }
    rendu
}
