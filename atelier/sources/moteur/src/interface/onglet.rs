//! LE LOGICIEL DANS UN ONGLET DU NAVIGATEUR (la cible WASI, la demo de puppetpaie.fr) : la page parle au moteur sans
//! serveur. Une demande entre (`DemandeDeLOnglet`), la fenetre la traite comme une requete de son serveur
//! (`Fenetre::traiter`), une reponse sort (`ReponseALOnglet`) : les memes pages, la meme porte `/api/<route>`, les memes
//! routes. La fenetre nait une fois (`demarrage`), puis sert chaque demande (`traiter`). Les octets entrent et sortent en
//! JSON, le corps en base64 ; la crate `onglet` les passe a l'hote JavaScript (ses exports), sans rien y lire.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use super::http::{self, Reponse};
use super::{navigateur, Fenetre, VEILLE};
use routes::enveloppe::{Refus, Reponse as RouteReponse};
use socle::echec::{Classe, R};

/// Le port que la fenetre d'un onglet se donne : aucun serveur n'ecoute ; il nomme son dossier sur le poste
/// (`navigateur::dossier_local`) et le cookie de son jeton.
const PORT: u16 = 0;

/// La fenetre de l'onglet, nee une fois (`demarrer`).
static FENETRE: OnceLock<Arc<Fenetre>> = OnceLock::new();

/// UNE DEMANDE DE LA PAGE, telle que l'hote de l'onglet la passe au moteur.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemandeDeLOnglet {
    /// `GET` ou `POST`.
    methode: String,
    /// Le chemin et ses parametres : `/`, `/?jeton=…`, `/js/avance.js`, `/api/mois`.
    adresse: String,
    /// Les en-tetes que la page pose (`X-Jeton`, `X-Gestes`, `Cookie`) ; aucun : `{}`.
    entetes: BTreeMap<String, String>,
    /// Le corps, en base64 ; aucun : `""`.
    corps: String,
}

/// LA REPONSE DU MOTEUR, telle que la fenetre la sert : le code HTTP, le type, les en-tetes, le corps en base64.
#[derive(Serialize)]
pub struct ReponseALOnglet {
    code: u16,
    #[serde(rename = "type")]
    type_mime: String,
    entetes: Vec<(String, String)>,
    corps: String,
}

impl ReponseALOnglet {
    fn de(r: Reponse) -> ReponseALOnglet {
        ReponseALOnglet { code: r.code, type_mime: r.type_mime, entetes: r.entetes, corps: socle::base64::ecrire(&r.corps) }
    }
}

/// LA FENETRE DE L'ONGLET EST NEE : la premiere page que l'hote demande (`/?jeton=…`). Sinon, l'hote recoit le refus
/// que la page lit (`refus.phrase`, `RouteReponse::refusee`).
#[derive(Serialize)]
pub struct PremierePageDeLOnglet {
    page: String,
}

/// `demarrer`, en octets pour l'hote (JSON) : `PremierePageDeLOnglet`, ou le refus.
pub fn demarrage() -> Vec<u8> {
    match demarrer() {
        Ok(page) => socle::ecriture::json_en_ligne(&PremierePageDeLOnglet { page }),
        Err(e) => socle::ecriture::json_en_ligne(&RouteReponse::refusee(Refus::de(None, e))),
    }
    .into_bytes()
}

/// LA FENETRE DE L'ONGLET NAIT, une fois : rend la premiere page a demander (`/?jeton=…`). Un second appel rend celle de
/// la fenetre deja nee. Comme tout programme du logiciel, elle fixe d'abord le dossier du code (`fixer_le_code` :
/// `PUPPETPAIE_CODE`, que l'hote pose ; sans elle, le refus la nomme).
fn demarrer() -> R<String> {
    if let Some(f) = FENETRE.get() {
        return Ok(f.premiere_page());
    }
    compagnie::installation::fixer_le_code()?;
    compagnie::diagnostic::installer();
    let f = Fenetre::nouvelle(VEILLE, navigateur::dossier_local(&compagnie::installation::base(), PORT), PORT)?;
    let premiere = f.premiere_page();
    // Un seul fil dans l'onglet : personne n'a pu la poser entre-temps.
    if FENETRE.set(f).is_err() {
        return Err(socle::echec::erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : la fenêtre de l'onglet est née deux fois."));
    }
    Ok(premiere)
}

/// UNE DEMANDE ENTRE (`DemandeDeLOnglet`, en JSON), UNE REPONSE SORT (`ReponseALOnglet`, en JSON) : celle de
/// `Fenetre::traiter`. Une demande qui ne se lit pas, ou qui arrive avant `demarrer`, rend le refus que la page lit
/// (`d.refus.phrase`).
pub fn traiter(octets: &[u8]) -> Vec<u8> {
    let reponse = match lire(octets) {
        Ok(r) => r,
        Err((code, e)) => Reponse::json(code, &RouteReponse::refusee(Refus::de(None, e))),
    };
    socle::ecriture::json_en_ligne(&ReponseALOnglet::de(reponse)).into_bytes()
}

fn lire(octets: &[u8]) -> Result<Reponse, (u16, socle::echec::Echec)> {
    let illisible = |e: socle::echec::Echec| (400, e.reclasser(Classe::DemandeIllisible));
    let texte = std::str::from_utf8(octets).map_err(|e| illisible(socle::refus!("La demande de l'onglet n'est pas du texte ({}).", e)))?;
    let d: DemandeDeLOnglet = socle::forme::lire_texte(texte, "La demande de l'onglet ne se lit pas.").map_err(illisible)?;
    let corps = socle::base64::lire(&d.corps).map_err(illisible)?;
    let Some(fenetre) = FENETRE.get() else {
        return Err((503, socle::echec::erreur(Classe::Refuse, "Le logiciel n'est pas encore démarré dans cet onglet.")));
    };
    let entetes: HashMap<String, String> = d.entetes.into_iter().map(|(k, v)| (k.to_ascii_lowercase(), v)).collect();
    Ok(fenetre.traiter(http::requete(d.methode, &d.adresse, entetes, corps)))
}
