//! `/api/<route>` : ce que la page demande, rendu dans l'enveloppe des routes (`routes::enveloppe::Reponse`), la meme
//! que `moteur route`.
//!
//! LES ROUTES TOURNENT DANS LE PROCESSUS DE LA FENETRE :
//! `route`, et pourquoi rien ne reste a part. Rien d'autre ne passe ici que les routes du catalogue : les gestes du poste
//! (ouvrir, choisir un dossier) et la veille sont ceux de la fenetre (`poste.rs`, `Fenetre::lanceur`).

use std::path::Path;

use socle::echec::Classe;

use super::veille::Veille;
use crate::etat_garde::{ACote, FileDesEtats};
use crate::routes::catalogue::Route;
use crate::routes::enveloppe::{Charge, Reponse};

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
/// s'en servent). Rend sa reponse entiere.
///
/// Rien ne reste dans un processus a part. Ce que le processus separe protegeait est tenu ici : une panique rend un
/// refus et la fenetre reste (`routes::dans_la_fenetre`, `panic = "unwind"` en `release`) ; le mot de passe tape est
/// oublie a la fin de sa route, quoi qu'il arrive ; la compagnie vient de `racine`, jamais d'une variable du processus
/// (deux compagnies peuvent etre servies en meme temps par deux fils) ; les sorties reseau passent par `garde_reseau`
/// aussi. Seul `maj::controler` lance encore un processus : il eprouve le NOUVEAU programme, qui n'est pas celui-ci.
pub fn route(racine: &Path, nom: Route, charge: &Charge) -> Reponse {
    // LE FIL (`diagnostic`) : une route que la page n'a pas demandee (la veille, le filet, le fond) s'y note ici ; celle de
    // la page se note dans `repondre`, avec ce que la page a fait avant.
    if DANS_LA_PAGE.with(std::cell::Cell::get) {
        return route_seule(racine, nom, charge);
    }
    let debut = std::time::Instant::now();
    let r = route_seule(racine, nom, charge);
    crate::compagnie::diagnostic::noter_route(racine, "fond", nom, charge, debut.elapsed(), issue(&r));
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
    let o = crate::routes::enveloppe::Options::de_la_compagnie(racine);
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
/// rendu la main ; `signalements-partir` le fait partir, le temps du serveur de l'editeur, sans que la page attende. Ce
/// qui ne part pas attend sa ligne, et la veille suivante reessaie.
fn signalements_en_fond(racine: &Path) {
    let r = racine.to_path_buf();
    crate::routes::fil_de_routes(move || {
        if let Some(x) = route(&r, Route::SignalementsPartir, &Charge::vide()).refus() {
            crate::compagnie::diagnostic::noter_moteur(&format!("Les messages au développeur ne partent pas : {}", x.phrase));
        }
    });
}

/// LA PAGE D'UNE COMPAGNIE OUVERTE. Deux routes se lisent dans la memoire de la fenetre, sans processus : l'ecran d'un
/// mois (`mois` : le dernier etat connu, tout de suite, meme perime -- marque `perime`, et refait en fond ; `attendre` :
/// l'etat frais) et ce que la page lit en s'ouvrant (`demarrage` : l'etat garde de l'ouverture, et, lus a cote, la veille
/// de la fenetre, la mise a jour et les signalements). Sans la fenetre, la porte du moteur les sert (`etat_garde::route`).
fn traiter(racine: &Path, nom: Route, charge: &Charge, veille: Option<&Veille>, etats: Option<&FileDesEtats>) -> Reponse {
    if let Some(f) = etats {
        if nom == Route::Mois {
            return Reponse::de(nom, f.ecran(charge));
        }
        if nom == Route::Demarrage {
            return Reponse::de(nom, f.demarrage(charge, || ACote::de(racine, veille.map(Veille::etat))));
        }
    }
    let r = route(racine, nom, charge);
    if nom == Route::SignalementEnvoyer && r.est_rendue() {
        signalements_en_fond(racine);
    }
    r
}

/// CE QUE PORTE L'INCIDENT D'UN REFUS DE LA PAGE, de quoi rejouer la route : sa charge (mise en forme par la frontiere
/// quand l'incident se note), sa duree, l'etat du mois qu'elle touche (ou pourquoi il ne se lit pas), ce que le refus
/// portait pour la page.
#[derive(serde::Serialize)]
struct RefusDeLaPage<'a> {
    route: Route,
    charge: &'a Charge,
    duree_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    mois: Option<crate::compagnie::diagnostic::EtatDuMois>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mois_illisible: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refus: Option<&'a crate::routes::enveloppe::APart>,
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
        let dit = match charge.lire::<crate::compagnie::incidents::RapportNavigateur>() {
            Ok(r) => {
                let texte = |s: &str, limite: usize| crate::compagnie::diagnostic::texte_en_forme(Some(racine), s, limite);
                format!("erreur JS {} : {} ({}:{})", texte(&r.genre, 80), texte(&r.message, 200), texte(&r.fichier, 80), r.ligne)
            }
            Err(e) => format!("erreur JS, son rapport ne se lit pas : {}", socle::echec::phrase_de(&e.message)),
        };
        crate::compagnie::diagnostic::noter_moteur(&dit);
    } else {
        crate::compagnie::diagnostic::noter_route(racine, "page", nom, charge, duree, issue(&rendu));
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
            let (mois, mois_illisible) = match charge.mois() {
                Ok(Some(m)) => match crate::compagnie::diagnostic::etat_du_mois(racine, &m.to_string()) {
                    Ok(etat) => (etat, None),
                    Err(e) => (None, Some(e.message)),
                },
                Ok(None) => (None, None),
                Err(e) => (None, Some(e.message)),
            };
            let detail = RefusDeLaPage {
                route: nom,
                charge,
                duree_ms: duree.as_millis() as u64,
                mois,
                mois_illisible,
                refus: (!r.a_part.est_vide()).then_some(&r.a_part),
            };
            let trace = crate::compagnie::diagnostic::trace_du_refus(racine, &r.phrase, debut_ms);
            if let Err(e) = crate::compagnie::incidents::depuis_exception(racine, r.classe, &r.phrase, None, nom.nom(), detail, trace.as_deref()) {
                crate::compagnie::diagnostic::noter_moteur(&format!("Le refus de {} ne se note pas au journal des incidents : {}", nom, socle::echec::phrase_de(&e.message)));
            }
        }
    }
    rendu
}

/// Ce que le scenario du banc retient d'une compagnie ajoutee (`scenario::rejouer`, le geste `ajouter`) : son nom, sa
/// venue quand elle vient d'un logiciel precedent, et son dossier.
#[cfg(feature = "atelier")]
#[derive(serde::Serialize)]
struct GesteAjouter {
    nom: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    venue: Option<&'static str>,
    compagnie: String,
}

/// UNE ROUTE DE L'INSTALLATION (`routes::accueil` : l'accueil des compagnies), servie sans compagnie : l'installation est
/// celle du programme.
pub fn route_de_l_installation(nom: Route, charge: &Charge) -> Reponse {
    let o = crate::routes::enveloppe::Options::sans_compagnie();
    // LE BANC D'ESSAI (l'atelier seulement) : une compagnie ajoutee entre dans le scenario comme le decor de depart que le
    // rejeu refait (`scenario::rejouer`, le geste `ajouter`).
    #[cfg(feature = "atelier")]
    if nom == Route::CompagnieAjouter {
        if let Some(e) = crate::atelier::scenario::enregistreur_actif() {
            let rendu = e.geste("ajouter", || {
                let a: crate::routes::accueil::ChargeAjouter = charge.lire().map_err(|x| x.phrase())?;
                let r = crate::routes::dans_la_fenetre(&o, nom, charge);
                let v = match r.refus() {
                    Some(x) => return Err(x.phrase.clone()),
                    None => r.resultat().ok_or("La compagnie ajoutée ne se dit pas.")?,
                };
                let ajoutee: crate::routes::accueil::Ajoutee = crate::routes::lire_charge(v).map_err(|x| x.phrase())?;
                let venue = (a.venue != crate::interface::compagnies::Venue::Neuve).then(|| a.venue.mot());
                let geste = GesteAjouter { nom: a.nom, venue, compagnie: e.compagnie(std::path::Path::new(&ajoutee.chemin)) };
                Ok((r, geste))
            });
            return match rendu {
                Ok(r) => r,
                Err(m) => Reponse::refusee_par(nom, crate::routes::enveloppe::Refus::nouveau(Classe::Refuse, m)),
            };
        }
    }
    let rendu = crate::routes::dans_la_fenetre(&o, nom, charge);
    // Le message ecrit a l'accueil part en fond, comme celui d'une compagnie (`signalements_en_fond`).
    if nom == Route::SignalementEnvoyer && rendu.est_rendue() {
        crate::routes::fil_de_routes(move || {
            if let Some(x) = crate::routes::dans_la_fenetre(&crate::routes::enveloppe::Options::sans_compagnie(),Route::SignalementsPartir, &Charge::vide()).refus() {
                crate::compagnie::diagnostic::noter_moteur(&format!("Les messages au développeur ne partent pas : {}", x.phrase));
            }
        });
    }
    rendu
}
