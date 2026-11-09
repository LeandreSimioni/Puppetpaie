//! LES ROUTES : la seule porte du moteur. L'ecran les appelle par `/api/<route>` (la fenetre les sert dans son processus,
//! `dans_la_fenetre`), le LLM par `moteur route` (`commande`, une par processus) ; l'un et l'autre lisent la meme reponse.
//!
//! L'ENVELOPPE (`enveloppe`) dit la forme de ce qu'une route recoit, rend et refuse : la demande et sa charge
//! (`Demande`, `Charge`), le rendu (`Rendu`, ecrit depuis une forme `Dite` qui porte sa phrase), le refus (`Refus` : une
//! classe en mots du logiciel, `socle::echec::Classe`, sa phrase, ce que la fiche dit de corriger, ce qu'il porte a part)
//! et la reponse (`Reponse`). Le JSON ne se touche que la : sous la porte, une route lit sa charge en sa forme et rend la
//! sienne.
//!
//! UN MODULE PAR PLACE DE L'ECRAN (`catalogue::Place`) : la Paie (`mois`, `etapes`, `engagement`, `contrats`,
//! `documents`, `depots`, `imprevus`, `rappels`), la Configuration (`fiches`, `metiers`, `identite`, `bulletin`,
//! `modeles_contrat`, `coffre`, `import`), l'Historique (`historique`), les Exports (`archives`, `dossier_salarie`),
//! « Un probleme, une idee ? » (`filet`), l'accueil des compagnies (`accueil` : les routes de l'installation), et les
//! lectures que les ecrans partagent (`ouverture`). Chacun dit ses routes et leurs fiches (`ROUTES`, `catalogue::FICHES`) ;
//! l'aiguillage (`servir`) ne fait rien d'autre que les servir.
//!
//! BRANCHER UN MODULE SUR L'ENVELOPPE :
//! 1. sa fonction `route` prend la porte et la charge et rend un rendu :
//!    `pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu>` ;
//! 2. chaque bras lit la charge de sa route en sa forme et finit par l'ecrire :
//!    `Route::X => en_json(faire(porte.ctx, charge.lire()?)?)` (`enveloppe::en_json`) ; une route qui ne prend rien lit
//!    `SansCharge`. La forme de la charge est une structure `serde` (`deny_unknown_fields`) ; le mot de passe du logiciel
//!    n'y est pas : `charge.mot_de_passe()` ;
//! 3. ce que la route rend est une structure `serde` qui porte sa phrase et la dit (`impl Dite`) ;
//! 4. un refus est un `Echec` dont la classe est un mot du logiciel (`erreur(Classe::Manque, …)`), ce qui n'est pas pour
//!    l'ecran a part (`socle::echec::avec`) ; la fiche dit quoi corriger pour chaque classe qu'elle refuse ;
//! 5. dans `servir`, son bras passe de `brute(m::route(o, &ctx, route, charge.brute()))` a `m::route(&porte, charge)`.
//!
//! Ce que les routes partagent (le salarie d'une charge, l'encart, la pastille, `INTERFACE`) est ici.

use std::collections::HashMap;
use std::path::Path;

use socle::echec::{erreur, Classe, Echec, R};
use crate::service::contexte;
use catalogue::Route;
use enveloppe::{Charge, Demande, Porte, Refus, Rendu, Reponse};
use brut::brute;

pub use enveloppe::{phrases_rendues, plus_longue_phrase};
pub(crate) use brut::{brut, cle_du_salarie, en_json, encart, jour_en_clair, lire_charge, lire_texte, mot, nom_a_l_ecran, nombre, pastille, periode_en_clair, phrase_en_tete, salarie};

mod brut;
pub mod enveloppe;
pub mod ouverture;
pub mod depots;
pub mod archives;
pub mod libelles_courts;
pub mod historique;
pub mod documents;
pub mod dossier_salarie;
pub mod coffre;
pub mod engagement;
pub mod contrats;
pub mod modeles_contrat;
pub mod fiches;
pub mod identite;
pub mod metiers;
pub mod bulletin;
pub mod etablissements;
pub mod atmp;
pub mod import;
pub mod imprevus;
pub mod filet;
pub mod etapes;
pub mod rappels;
pub mod mois;
pub mod accueil;
pub mod catalogue;
pub mod affiche;

/// La pile d'un fil qui sert des routes dans la fenetre : 8 Mo, celle du fil principal d'un `moteur route` sous Linux (un
/// fil neuf n'en a que 2). Un calcul qui remonte les mois d'avant s'y empile ; un debordement de pile ne se rattrape pas
/// (`catch_unwind` n'y peut rien), il fermerait la fenetre.
pub const PILE_D_UNE_ROUTE: usize = 8 * 1024 * 1024;

/// Un fil qui sert des routes dans la fenetre (une connexion, la file des etats, la veille, un passage en fond), avec
/// la pile qu'il leur faut (`PILE_D_UNE_ROUTE`).
pub fn fil_de_routes<F: FnOnce() + Send + 'static>(f: F) {
    if let Err(e) = std::thread::Builder::new().stack_size(PILE_D_UNE_ROUTE).spawn(f) {
        eprintln!("Un fil du moteur ne demarre pas : {}.", e);
    }
}

/// Le fil remis propre apres une route servie dans la fenetre (`dans_la_fenetre`, `etat_garde::FileDesEtats`) : plus
/// de mot de passe tape, les nombres a leur ecriture normale, aucun registre de lectures ouvert.
pub(crate) fn remettre_le_fil() {
    crate::compagnie::coffre::oublier_mot_de_passe_tape();
    socle::num::ECHELLE_GARDEE.with(|f| f.set(false));
    socle::fichiers::fermer_tout();
}

/// UNE ROUTE SERVIE DANS LA FENETRE, ET CE QU'ELLE LAISSE SUR SON FIL. La fenetre sert ses routes dans son
/// processus (`interface::api::route`, la file des etats), et le fil d'une connexion en sert d'autres ensuite. Cette
/// fonction fait la route (`servir_la_route`), gardee d'une panique (une route qui tombe rend un refus `DefautDuLogiciel`
/// et le fil continue), puis remet le fil propre, quoi qu'il soit arrive : le mot de passe tape oublie (`coffre::TAPE` :
/// il ne survit jamais a sa route), les nombres a leur ecriture normale (`num::ECHELLE_GARDEE`), le registre des lectures
/// qu'un calcul aurait laisse ouvert en tombant ferme (`fichiers::suivre`).
///
/// `moteur route` et la ligne de commande (`commandes::Cadre`) appellent `commande` directement : le processus meurt avec
/// sa commande, et le mot de passe qu'elle a fait taper (`demande::autoriser_acte`) sert a toute la commande.
pub fn dans_la_fenetre(o: &HashMap<String, String>, route: Route, charge: &Charge) -> Reponse {
    let rendu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| servir_la_route(o, route, charge)));
    remettre_le_fil();
    match rendu {
        Ok(rep) => rep,
        Err(panique) => {
            let pourquoi = match (panique.downcast_ref::<&str>(), panique.downcast_ref::<String>()) {
                (Some(s), _) => format!(" ({})", s),
                (None, Some(s)) => format!(" ({})", s),
                (None, None) => String::new(),
            };
            Reponse::de(route, Err(erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : la route {} s'est arrêtée{}.", route, pourquoi))))
        }
    }
}

/// `moteur route [--racine R] [--date D]` : la demande lue en sa forme (`Demande::lire`), sans ses secrets
/// (`Charge::sans_les_secrets` : l'entree standard est celle d'un programme), puis servie.
pub fn commande(o: &HashMap<String, String>, entree: &str) -> Reponse {
    match Demande::lire(entree) {
        Ok(mut d) => {
            d.charge.sans_les_secrets(d.route);
            servir_la_route(o, d.route, &d.charge)
        }
        Err(e) => Reponse::refusee(Refus::de(None, e)),
    }
}

/// La route `route` servie sur `charge`, et ce que l'ecran et le LLM en lisent (`Reponse`).
pub(crate) fn servir_la_route(o: &HashMap<String, String>, route: Route, charge: &Charge) -> Reponse {
    // Le code livre ne bouge pas le temps de la route : ses signatures se lisent une fois (`fichiers::code_fige`).
    let _code = socle::fichiers::code_fige();
    // Ce que la compagnie porte (les engagements, leurs paies, les imprevus) se lit une fois par geste (`socle::forme::geste`).
    let _geste = socle::forme::geste();
    crate::compagnie::incidents::brancher_l_engagement();
    let rendu = servir(o, route, charge);
    // Ce que la route rend, reussite ou refus, est ce que l'ecran lit : ses phrases passent ici (`phrase_a_sa_place`).
    let racine = crate::service::compagnie_nommee(o).then(|| crate::service::racine_de(o));
    if let (Ok(v), Some(r)) = (&rendu, &racine) {
        if catalogue::dans(accueil::ROUTES, route).is_none() && catalogue::dans(catalogue::ROUTES, route).is_none() {
            phrase_a_sa_place(r, route, v);
        }
    }
    let reponse = Reponse::de(route, rendu);
    if let (Some(refus), Some(r)) = (reponse.refus(), &racine) {
        refus_a_sa_place(r, route.nom(), &refus.phrase);
    }
    reponse
}

/// LA ROUTE `route` SERVIE : ce qu'elle rend, ou son refus. L'aiguillage, rien d'autre.
fn servir(o: &HashMap<String, String>, route: Route, charge: &Charge) -> R<Rendu> {
    // Le mot de passe du logiciel tape pour cette route ouvre aussi le coffre (`coffre::retenir_mot_de_passe_tape`) ;
    // dans la fenetre, le temps de cette route seulement (`dans_la_fenetre`).
    if let Some(m) = charge.mot_de_passe() {
        crate::compagnie::coffre::retenir_mot_de_passe_tape(m);
    }
    // UNE ROUTE QUI TOMBE, pour les essais seulement (`garde_reseau::en_essai`, que Cargo pose) : la fenetre doit rendre
    // un refus et servir la suivante (`dans_la_fenetre`, `tests/routes/`). Le fil est laisse sale
    // expres, comme un calcul qui tombe le laisserait. Hors des essais, elle n'existe pas.
    if route == Route::EssaiDefautDuLogiciel {
        if !socle::garde_reseau::en_essai() {
            return Err(enveloppe::route_inconnue(route.nom()));
        }
        socle::num::ECHELLE_GARDEE.with(|f| f.set(true));
        panic!("la route d'essai tombe expres");
    }
    // UNE ROUTE DE L'INSTALLATION (l'accueil des compagnies) ne porte sur aucune compagnie : elle se sert sans contexte.
    if catalogue::dans(accueil::ROUTES, route).is_some() {
        return accueil::route(o, route, charge);
    }
    // ECRIRE AU DEVELOPPEUR, ET CE QUI S'Y RATTACHE, SE SERT AUSSI SANS COMPAGNIE : depuis l'accueil, ou en ligne de
    // commande sans `--racine` (`filet::sans_compagnie`).
    if catalogue::dans(filet::ROUTES, route).is_some() && !crate::service::compagnie_nommee(o) {
        return filet::route_sans_compagnie(o, route, charge);
    }
    // Le catalogue lui-meme, ce que `moteur routes --json` imprime : il ne porte sur aucune compagnie non plus.
    if route == Route::Routes {
        return catalogue::route(charge);
    }
    // UNE ROUTE QUI PORTE UN MOIS LIT LA CONFIGURATION DE CE MOIS (`Contexte::du_mois`) : sa paie, sa DSN et ses documents
    // se font sous les valeurs qui valaient alors, jamais sous celles d'aujourd'hui. Un mois qui ne se lit pas refuse la
    // route, rien ne s'ecrit.
    let ctx = match charge.mois()? {
        Some(m) => contexte(o)?.du_mois(&m.to_string())?,
        None => contexte(o)?,
    };
    let porte = Porte { route, ctx: &ctx, options: o };
    let dans = |l: &[catalogue::Fiche]| catalogue::dans(l, route).is_some();
    let c = charge.brute();
    let rendu: R<Rendu> = match route {
        // Les modules branches sur l'enveloppe : leur charge lue en sa forme, leur rendu ecrit depuis la sienne.
        _ if dans(bulletin::ROUTES) => bulletin::route(&porte, charge),
        _ if dans(metiers::ROUTES) => metiers::route(&porte, charge),
        _ if dans(libelles_courts::ROUTES) => libelles_courts::route(&porte, charge),
        // L'etat d'un mois : l'etat garde, ecrit apres chaque ecriture, lu tel quel (`etat_garde`).
        _ if dans(crate::etat_garde::ROUTES) => brute(crate::etat_garde::route(o, &ctx, route, c)),
        // Un geste de l'ecran, une route : un module par geste.
        _ if dans(mois::ROUTES) => brute(mois::route(o, &ctx, route, c)),
        _ if dans(etapes::ROUTES) => brute(etapes::route(o, &ctx, route, c)),
        _ if dans(engagement::ROUTES) => brute(engagement::route(o, &ctx, route, c)),
        _ if dans(contrats::ROUTES) => brute(contrats::route(o, &ctx, route, c)),
        _ if dans(modeles_contrat::ROUTES) => modeles_contrat::route(&porte, charge),
        _ if dans(documents::ROUTES) => brute(documents::route(o, &ctx, route, c)),
        _ if dans(depots::ROUTES) => brute(depots::route(o, &ctx, route, c)),
        _ if dans(imprevus::ROUTES) => brute(imprevus::route(o, &ctx, route, c)),
        // Les obligations d'un arret : le jour ou la compagnie l'a su, la declaration d'accident faite.
        _ if dans(crate::obligations::ROUTES) => brute(crate::obligations::route(&ctx, route, c)),
        _ if dans(fiches::ROUTES) => brute(fiches::route(o, &ctx, route, c)),
        _ if dans(identite::ROUTES) => identite::route(&porte, charge),
        // Les organismes et les contrats de la DSN : lus, et composes quand la compagnie les enregistre (`organismes`).
        _ if dans(crate::compagnie::organismes::ROUTES) => brute(crate::compagnie::organismes::route(&ctx, route, c)),
        _ if dans(coffre::ROUTES) => coffre::route(&porte, charge),
        _ if dans(filet::ROUTES) => filet::route(&porte, charge),
        _ if dans(import::ROUTES) => brute(import::route(o, &ctx, route, c)),
        _ if dans(archives::ROUTES) => brute(archives::route(o, &ctx, route, c)),
        _ if dans(historique::ROUTES) => brute(historique::route(o, &ctx, route, c)),
        // L'Historique d'un mois : ses documents, ses envois et leurs retours, lus (lecture seule).
        Route::HistoriqueMois => brute(crate::historique_mois::route(&ctx, c)),
        _ if dans(dossier_salarie::ROUTES) => brute(dossier_salarie::route(o, &ctx, route, c)),
        _ if dans(ouverture::ROUTES) => brute(ouverture::route(o, &ctx, route, c)),
        _ if dans(rappels::ROUTES) => brute(rappels::route(o, &ctx, route, c)),
        // Les conseils « Le saviez-vous ? » : le conseil du jour, ceux du dossier d'un salarie, le resume de l'accueil.
        _ if dans(crate::compagnie::conseils::ROUTES) => brute(crate::compagnie::conseils::route(&ctx, route, c)),
        // L'ouverture du logiciel et la veille : sauvegarder, relever les retours, les signatures et les fiches, sans
        // jamais echouer. ATTENTION : hors d'une compagnie fictive, cette route va au reseau (net-entreprises, Urssaf,
        // boite courriel, Drive, serveur des incidents).
        _ if dans(crate::compagnie::ouverture::ROUTES) => brute(crate::compagnie::ouverture::route(o, &ctx, route, c)),
        // Le banc : l'horloge de la compagnie fictive (rattraper ce qui etait du, ce que le bandeau « pendant votre
        // absence » lit ; `horloge::HorlogeFictive`).
        Route::HorlogeRattraper => brute(crate::horloge::route(o, &ctx, c)),
        autre => return Err(enveloppe::route_inconnue(autre.nom())),
    };
    // UN ACTE REFUSE A UNE COMPAGNIE QUI N'A PAS FINI DE DEMARRER dit tout ce qui lui manque, pas un manque a la fois.
    // Les mots de passe et les comptes (`routes::coffre`), le filet et les messages au developpeur (`routes::filet`) ne
    // dependent d'aucun de ces manques : leur refus se dit seul.
    match rendu {
        Err(Echec { classe, message }) if est_une_ecriture(route) && !dans(coffre::ROUTES) && !dans(filet::ROUTES) && classe != Some(Classe::EnvoiBride) => {
            // Un refus qui porte une donnee a part (l'etat d'un mois, les anomalies d'une fiche, un chemin) se dit seul.
            let message = if message.contains(socle::echec::A_PART) {
                message
            } else {
                match declarations::demarrage::a_la_suite(&ctx, &message) {
                    Ok(Some(suite)) => message + &suite,
                    Ok(None) => message,
                    Err(e) => format!("{} {}", message, e),
                }
            };
            Err(Echec { classe, message })
        }
        autre => autre,
    }
}
/// Ce qu'un incident silencieux de la porte dit au developpeur d'une phrase : la route, et selon le cas le defaut, les
/// cles d'une reponse sans phrase, la place et la longueur. Jamais la phrase, qui peut nommer un salarie.
#[derive(serde::Serialize)]
struct PhraseNotee<'a> {
    route: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    defaut: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refus: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cles: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    place: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    longueur: Option<usize>,
}

impl<'a> PhraseNotee<'a> {
    fn de(route: &'a str) -> PhraseNotee<'a> {
        PhraseNotee { route, defaut: None, refus: None, cles: None, place: None, longueur: None }
    }

    /// L'incident silencieux `quoi`, resume par `resume`.
    fn noter(&self, racine: &Path, quoi: &str, resume: &str) {
        // Un incident qui ne s'ecrit pas ne retient pas la route : `noter` ne leve jamais.
        crate::compagnie::incidents::noter(racine, &crate::compagnie::incidents::Note {
            nature: crate::compagnie::incidents::LOGICIEL,
            resume,
            ou: self.route,
            quoi,
            detail: socle::ecriture::en_valeur(self),
            sans_contexte: true,
            ..Default::default()
        });
    }
}

/// UNE PHRASE DE REFUS QUI N'EST PAS FAITE POUR L'ECRAN (un chemin, un nom de fichier, un terme de code :
/// `affiche::defaut`) se lit quand meme, et note un incident silencieux au developpeur : la route et le defaut, jamais la
/// phrase. On la corrige a la cause : la phrase en mots, ce qui n'est pas pour l'ecran a part (`socle::echec::avec`).
fn refus_a_sa_place(racine: &Path, route: &str, phrase: &str) {
    let Some(d) = affiche::defaut(phrase, usize::MAX, false) else { return };
    let resume = format!("Un refus de la route {} n'est pas fait pour l'écran : {}.", route, d.corriger());
    PhraseNotee { defaut: Some(format!("{:?}", d)), refus: Some(true), ..PhraseNotee::de(route) }.noter(racine, "PhrasePasFaitePourLEcran", &resume);
}

/// UNE PHRASE PLUS LONGUE QUE SA PLACE (la `phrase` de la fiche ; `Rendu::plus_longue_phrase`) se lit entiere, et note un
/// incident silencieux au developpeur : la route, la place, la longueur ; jamais la phrase, qui peut nommer un salarie. On
/// la corrige a la cause, en l'ecrivant plus breve.
fn phrase_a_sa_place(racine: &Path, route: Route, rendu: &Rendu) {
    // UNE REPONSE SANS PHRASE d'une route du catalogue (un avertissement, une variante que l'exemple de sa fiche ne joue
    // pas) : ni l'ecran ni `moteur route --texte` n'ont rien a lire. Le meme incident silencieux, pour la reponse qui l'a
    // oubliee. Une route branchee ne rend pas sans phrase (`enveloppe::Dite`). (L'horloge du banc, hors catalogue, rend
    // ses phrases en liste.)
    if route.au_catalogue() && !rendu.a_sa_phrase() {
        let resume = format!("La route {} a rendu une réponse sans phrase.", route);
        PhraseNotee { cles: Some(rendu.cles()), ..PhraseNotee::de(route.nom()) }.noter(racine, "PhraseManquante", &resume);
        return;
    }
    // UNE PHRASE QUI N'EST PAS FAITE POUR L'ECRAN (un nom de fichier, un terme de code : `affiche::defaut`) : le meme
    // incident silencieux, sans la phrase.
    if let Some(d) = rendu.phrases().into_iter().find_map(|p| affiche::defaut(p, usize::MAX, false)) {
        let resume = format!("Une phrase de la route {} n'est pas faite pour l'écran : {}.", route, d.corriger());
        PhraseNotee { defaut: Some(format!("{:?}", d)), ..PhraseNotee::de(route.nom()) }.noter(racine, "PhrasePasFaitePourLEcran", &resume);
    }
    let Some(place) = catalogue::catalogue().iter().find(|f| f.route == route).map(|f| f.phrase).filter(|p| *p > 0) else { return };
    let longueur = rendu.plus_longue_phrase();
    if longueur <= place {
        return;
    }
    let resume = format!("La phrase de la route {} passe sa place : {} caractères pour {}.", route, longueur, place);
    PhraseNotee { place: Some(place), longueur: Some(longueur), ..PhraseNotee::de(route.nom()) }.noter(racine, "PhraseTropLongue", &resume);
}

/// Les routes qui ecrivent ou engagent, d'apres l'acte de leur fiche : apres elles, la fenetre verifie les etats des mois
/// (`interface/api.rs`), et la reponse le dit (`Reponse::ecrit`).
pub fn est_une_ecriture(route: Route) -> bool {
    catalogue::FICHES.iter().any(|l| catalogue::dans(l, route).is_some_and(|f| f.acte != catalogue::Acte::Lit))
}

/// `socle::paris::maintenant` a la minute, `AAAA-MM-JJ HH:MM` : l'heure d'une saisie.
pub(crate) fn maintenant() -> String {
    socle::paris::maintenant()[..16].replacen('T', " ", 1)
}

/// QUI ECRIT : ce que les routes inscrivent, la fenetre comme la ligne de commande (`moteur route`, qui sert les memes).
pub(crate) const INTERFACE: &str = "interface";

/// La charge d'une route qui ne prend rien : `{}`.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SansCharge {}

/// LA COULEUR D'UN ETAT, la meme sur tout l'ecran : `vert` ce qui est fait, `orange` ce qui reste a faire et que la
/// compagnie peut faire (une signature attendue se relance), `rouge` ce qui est refuse, `gris` ce qui n'est qu'une
/// information (rien a faire). La page dessine la couleur qu'on lui donne ; elle n'en choisit aucune. Sa pastille :
/// `Pastille`.
pub(crate) use socle::ecran::{couleur_de_l_etat, Pastille};

/// UN ENCART, tel que la page le dessine (`encart` de js/tuyau.js) : un titre en gras, sa phrase, la couleur de l'etat.
/// La page n'ecrit aucune phrase : ce qu'une boite dit d'un cas, la route le rend ainsi.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Encart {
    pub couleur: &'static str,
    pub titre: String,
    pub phrase: String,
}

impl Encart {
    pub(crate) fn de(etat: &str, titre: impl Into<String>, phrase: impl Into<String>) -> Encart {
        Encart { couleur: couleur_de_l_etat(etat), titre: titre.into(), phrase: phrase.into() }
    }

    /// L'encart en une ligne pour `moteur route --texte` : « titre : phrase ».
    pub(crate) fn en_ligne(&self) -> String {
        [self.titre.as_str(), self.phrase.as_str()].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" : ")
    }
}

/// « 12/09/2026 à 18:18 » : un horodatage ISO tel que l'ecran le dit ; ce qui n'en est pas un, tel quel.
pub(crate) fn quand(iso: &str) -> String {
    let b = iso.as_bytes();
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return iso.to_string();
    }
    let jour = format!("{}/{}/{}", &iso[8..10], &iso[5..7], &iso[..4]);
    match iso.get(11..16) {
        Some(h) => format!("{} à {}", jour, h),
        None => jour,
    }
}

pub(crate) fn vers_route(x: crate::compagnie::coffre::Echec) -> Echec {
    Echec { classe: Some(x.classe), message: x.message }
}

/// « 1 salarié », « 3 salariés » : `n` et le nom, au pluriel quand il le faut (un `s` de plus, ou `pluriel`).
pub(crate) fn compte(n: usize, un: &str, pluriel: &str) -> String {
    format!("{} {}", n, if n > 1 { pluriel } else { un })
}
