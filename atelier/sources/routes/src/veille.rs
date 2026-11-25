//! CE QUE LA VEILLE DE LA FENETRE DIT A LA PAGE : son etat (`EtatDeLaVeille`), tel que le bandeau le dessine
//! (`js/veille.js`) et que l'etat garde le rend au demarrage (`etat_garde::ACote`), et les phrases du bandeau
//! (`DitDuBandeau`). La veille qui tourne (ses tours, sa boucle) : `interface::veille`.

use serde::{Deserialize, Serialize};

use crate::catalogue::Route;
use crate::enveloppe::{Charge, Refus, Reponse};
use crate::gestes::ouverture::Trace;

/// LA SAUVEGARDE QUI SUIT UN GESTE (le Drive lie : `google-autoriser`), que la reponse dit en `ensuite` : sa route et sa
/// charge, les memes pour qui les lit. En ligne de commande, l'appelant l'appelle ; la fenetre la fait par sa veille, sur
/// son fil (`interface::veille::Veille::faire_suivre`), et le bandeau en suit l'avancement. Une sauvegarde seule
/// (`sauvegarde-maintenant`), sans relevé. Pas un geste qu'on clique : celui-la, une reponse le rend en `suite`
/// (`etapes::Action`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SauvegardeQuiSuit {
    route: Route,
    charge: crate::SansCharge,
}

impl SauvegardeQuiSuit {
    /// La sauvegarde du Drive, seule.
    pub(crate) fn du_drive() -> SauvegardeQuiSuit {
        SauvegardeQuiSuit { route: Route::SauvegardeMaintenant, charge: crate::SansCharge {} }
    }

    /// Celle que la reponse `r` dit suivre (`ensuite`) ; aucune quand elle n'en dit pas, ou qu'elle a refuse. Une cle
    /// `ensuite` qui n'a pas sa forme est un defaut du logiciel : le refus le dit.
    pub fn dite_par(r: &Reponse) -> Result<Option<SauvegardeQuiSuit>, Refus> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Ensuite {
            ensuite: Option<SauvegardeQuiSuit>,
        }
        if !r.est_rendue() {
            return Ok(None);
        }
        r.lire_cles::<Ensuite>(&["ensuite"]).map(|e| e.ensuite)
    }

    /// Sa route.
    pub fn route(&self) -> Route {
        self.route
    }

    /// Sa charge, telle que la route la lit.
    pub fn charge(&self) -> socle::echec::R<Charge> {
        Charge::de_forme(&self.charge)
    }
}

/// L'ETAT DE LA VEILLE, tel que le bandeau le dessine (`js/veille.js`) : ce qu'il dit (`dit`), le passage en cours, la
/// boucle (`minutes`), la trace du dernier passage (`dernier`), et ce qui l'a empeche (`erreur`).
#[derive(Debug, Clone, Serialize)]
pub struct EtatDeLaVeille {
    pub dit: DitDuBandeau,
    pub en_cours: bool,
    pub sauvegarde_en_cours: bool,
    pub etape: Option<String>,
    /// Le temps entre deux passages de la boucle, en minutes ; zero : aucune boucle.
    #[serde(serialize_with = "socle::temps::ecrit::minutes::serialize")]
    pub minutes: std::time::Duration,
    pub dernier: Option<Trace>,
    pub erreur: Option<String>,
}

/// « à 16:28 » le jour du passage, « le 23/09 à 16:28 » un autre jour : l'heure d'un passage, au jour de l'horloge de
/// Paris (celle d'une fictive au banc comprise).
fn heure_du_passage(quand: socle::paris::Moment) -> String {
    let jour = quand.jour_de_paris();
    let heure = socle::paris::ecrit::heure_minute(quand, ":");
    if socle::temps::secondes().a_la_seconde().jour_de_paris() == jour {
        format!("à {}", heure)
    } else {
        format!("le {} à {}", jour.jj_mm(), heure)
    }
}

/// UN ACCES QUI MANQUE, tel que le bandeau le dit : le service (`net-entreprises`, `urssaf`, `google`, ou mene le
/// bouton), la phrase, et le bouton.
#[derive(Debug, Clone, Serialize)]
pub struct APoser {
    service: String,
    phrase: &'static str,
    geste: &'static str,
}

/// CE QUI MANQUE MAINTENANT : les acces que le dernier passage a trouves absents (`a_poser` de la trace : ceux dont ce
/// passage avait besoin), moins ceux que le coffre dit poses a l'instant ou la route rend (`ouverture::acces_pose`, sans
/// rien ouvrir). Lier le Drive ou poser un mot de passe fait donc disparaitre sa ligne au prochain appel, sans attendre
/// le releve. `Err` : le coffre ne se lit pas.
pub fn acces_a_poser(racine: &std::path::Path, d: Option<&Trace>) -> socle::echec::R<Vec<APoser>> {
    // Aucun passage encore : rien n'a ete trouve absent.
    let notes: &[String] = match d {
        Some(t) => &t.a_poser,
        None => &[],
    };
    let mut a_poser = Vec::new();
    for service in notes {
        if crate::gestes::ouverture::acces_pose(racine, service)? {
            continue;
        }
        let (phrase, geste) = match service.as_str() {
            "net-entreprises" => ("Les retours de DSN ne se relèvent pas seuls : le mot de passe net-entreprises n'est pas posé.", "Poser le mot de passe"),
            "urssaf" => ("Les retours de DPAE ne se relèvent pas seuls : le mot de passe Urssaf (DPAE) n'est pas posé.", "Poser le mot de passe"),
            "google" => ("La sauvegarde ne part pas : aucun Drive n'est lié.", "Lier le Drive"),
            autre => return Err(socle::refus!("La trace de la dernière ouverture nomme un accès inconnu : {}.", autre)),
        };
        a_poser.push(APoser { service: service.clone(), phrase, geste });
    }
    Ok(a_poser)
}

/// CE QUI PERSISTE DE LA SAUVEGARDE, MAINTENANT : ce que le dernier passage en a dit (`sauvegarde_souci` de la trace,
/// `compagnie::sauvegarde::souci`), tant que c'est encore vrai a l'instant ou la route rend. Le Drive lie apres ce
/// passage (`coffre::lie_le`), ou qui ne l'est plus (`ouverture::acces_pose`), le passage ne dit plus rien de la
/// sauvegarde qui partira : la phrase se tait jusqu'au passage suivant, qui dira ce qu'il trouve. `Err` : le coffre ou
/// la date du jeton ne se lit pas.
pub fn souci_de_la_sauvegarde(racine: &std::path::Path, d: Option<&Trace>) -> socle::echec::R<Option<String>> {
    let Some((souci, passage)) = d.and_then(|t| t.sauvegarde_souci.clone().map(|s| (s, t.quand))) else { return Ok(None) };
    if !crate::gestes::ouverture::acces_pose(racine, "google")? {
        return Ok(None);
    }
    Ok(match crate::coffre::lie_le(racine)? {
        Some(lie) if lie > passage => None,
        _ => Some(souci),
    })
}

/// CE QUE LE BANDEAU DIT (`dit` de l'etat de la veille) : l'etat en une phrase, un service muet, ce qu'il y a a poser et
/// ou, les retours nouveaux, ce que l'horloge d'une fictive a fait, et ce que dit un passage en cours ou fini. La page
/// n'en ecrit rien.
#[derive(Debug, Clone, Serialize)]
pub struct DitDuBandeau {
    etat: String,
    muets: Option<String>,
    /// Ce qui manque maintenant (`acces_a_poser`), une ligne par acces.
    a_poser: Vec<APoser>,
    /// Ce qui persiste de la sauvegarde, vrai maintenant (`souci_de_la_sauvegarde`).
    sauvegarde_souci: Option<String>,
    /// Les envois partis en fond (`envois_en_fond::EnvoisEnFond::dits`) : en cours, au bout, ou echoues avec le geste
    /// qui les refait.
    envois: Vec<crate::envois_en_fond::EnvoiDit>,
    /// Le logiciel se ferme des qu'un envoi en cours est au bout (`envois_en_fond::fermer_apres_les_envois`).
    #[serde(skip_serializing_if = "Option::is_none")]
    fermeture: Option<&'static str>,
    retours: Option<String>,
    fictive: Option<&'static str>,
    horloge: Option<String>,
    horloge_titre: &'static str,
    horloge_intro: &'static str,
    en_cours: &'static str,
    sauvegarde: &'static str,
    termine: &'static str,
}

pub fn dit_du_bandeau(
    d: Option<&Trace>,
    a_poser: Vec<APoser>,
    sauvegarde_souci: Option<String>,
    envois: Vec<crate::envois_en_fond::EnvoiDit>,
    fermeture: bool,
    dans: Option<std::time::Duration>,
    fictive: bool,
) -> DitDuBandeau {
    let dans = dans.map(socle::temps::ecrit::minutes_commencees_dites);
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
        a_poser,
        sauvegarde_souci,
        envois,
        fermeture: fermeture.then_some("PuppetPaie se fermera dès que l'envoi en cours sera parti."),
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

