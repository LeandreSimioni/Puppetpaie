//! CE QUE LA VEILLE DE LA FENETRE DIT A LA PAGE : son etat (`EtatDeLaVeille`), tel que le bandeau le dessine
//! (`js/veille.js`) et que l'etat garde le rend au demarrage (`etat_garde::ACote`), et les phrases du bandeau
//! (`DitDuBandeau`). La veille qui tourne (ses tours, sa boucle) : `interface::veille`.

use serde::Serialize;

use crate::gestes::ouverture::Trace;

/// L'ETAT DE LA VEILLE, tel que le bandeau le dessine (`js/veille.js`) : ce qu'il dit (`dit`), le passage en cours, la
/// boucle (`minutes`), la trace du dernier passage (`dernier`), et ce qui l'a empeche (`erreur`).
#[derive(Debug, Clone, Serialize)]
pub struct EtatDeLaVeille {
    pub dit: DitDuBandeau,
    pub en_cours: bool,
    pub sauvegarde_en_cours: bool,
    pub etape: Option<String>,
    pub minutes: u64,
    pub dernier: Option<Trace>,
    pub erreur: Option<String>,
}

/// « à 16:28 » le jour du passage, « le 23/09 à 16:28 » un autre jour : l'heure d'un passage, au jour de l'horloge de
/// Paris (celle d'une fictive au banc comprise).
fn heure_du_passage(quand: socle::paris::MinuteDite) -> String {
    let (jour, s) = (quand.naif().jour(), quand.naif().seconde_du_jour());
    let heure = format!("{:02}:{:02}", s / 3_600, s / 60 % 60);
    if socle::paris::InstantEcrit::maintenant().jour() == jour {
        format!("à {}", heure)
    } else {
        format!("le {:02}/{:02} à {}", jour.jour, jour.mois, heure)
    }
}

/// Ce que le bandeau dit d'un acces qui manque, par service (`a_poser` de la trace) : la phrase, et le bouton qui y mene.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct APoser {
    #[serde(rename = "net-entreprises")]
    net_entreprises: [&'static str; 2],
    urssaf: [&'static str; 2],
    google: [&'static str; 2],
}

/// CE QUE LE BANDEAU DIT (`dit` de l'etat de la veille) : l'etat en une phrase, un service muet, ce qu'il y a a poser et
/// ou, les retours nouveaux, ce que l'horloge d'une fictive a fait, et ce que dit un passage en cours ou fini. La page
/// n'en ecrit rien.
#[derive(Debug, Clone, Serialize)]
pub struct DitDuBandeau {
    etat: String,
    muets: Option<String>,
    a_poser: APoser,
    retours: Option<String>,
    fictive: Option<&'static str>,
    horloge: Option<String>,
    horloge_titre: &'static str,
    horloge_intro: &'static str,
    en_cours: &'static str,
    sauvegarde: &'static str,
    termine: &'static str,
}

pub fn dit_du_bandeau(d: Option<&Trace>, dans: Option<u64>, fictive: bool) -> DitDuBandeau {
    // Aucun passage encore : le releve est a venir.
    let quand = match d {
        Some(t) => heure_du_passage(t.quand),
        None => "à venir".to_string(),
    };
    let hors_ligne = d.is_some_and(|t| t.hors_ligne);
    let retours = d.and_then(|t| t.retours.as_deref()).is_some_and(|r| !r.is_empty());
    let etat = match (hors_ligne && !retours, dans) {
        (true, Some(n)) => format!("Pas de réponse au relevé {} : vérifier la connexion ; nouvel essai dans {} min", quand, n),
        (true, None) => format!("Pas de réponse au relevé {} : vérifier la connexion", quand),
        (false, Some(n)) => format!("À jour, relevé {} · prochain relevé dans {} min", quand, n),
        (false, None) => format!("À jour, relevé {}", quand),
    };
    let n_muets = d.map(|t| t.injoignables.len());
    let muets = match (hors_ligne && retours, n_muets) {
        (true, Some(n)) if n > 1 => Some(format!("{} services n'ont pas répondu (détail au survol) : nouvel essai au prochain relevé", n)),
        (true, _) => Some("un service n'a pas répondu (détail au survol) : nouvel essai au prochain relevé".to_string()),
        (false, _) => None,
    };
    let nouveaux = d.map(|t| t.nouveaux).filter(|n| *n > 0);
    let horloge = d.map(|t| t.horloge.len()).filter(|n| fictive && *n > 0);
    DitDuBandeau {
        etat,
        muets,
        a_poser: APoser {
            net_entreprises: ["Les retours de DSN ne se relèvent pas seuls : le mot de passe net-entreprises n'est pas posé.", "Poser le mot de passe"],
            urssaf: ["Les retours de DPAE ne se relèvent pas seuls : le mot de passe Urssaf (DPAE) n'est pas posé.", "Poser le mot de passe"],
            google: ["La sauvegarde ne part pas : aucun Drive n'est lié.", "Lier le Drive"],
        },
        retours: nouveaux.map(|n| format!("{} retour{} nouveau{} : les lire dans l'Historique", n, if n > 1 { "s" } else { "" }, if n > 1 { "x" } else { "" })),
        fictive: fictive.then_some("compagnie fictive : pas de sauvegarde, pourquoi ?"),
        horloge: horloge.map(|n| format!("pendant votre absence, la compagnie a travaillé ({}) : voir", n)),
        horloge_titre: "Pendant votre absence",
        horloge_intro: "L'administrateur fictif a fait ce qui tombait, chacun à sa date limite : le 1er du mois, le mois s'écrit ; le jour d'une embauche, sa DPAE part ; à l'échéance, la DSN. Tout est joué, rien n'a quitté le poste, et le journal de chaque mois le montre.",
        en_cours: "Relevé des retours…",
        sauvegarde: "Sauvegarde…",
        termine: "Relevé terminé.",
    }
}

