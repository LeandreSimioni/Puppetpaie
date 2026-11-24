//! LES ENVOIS QUI PARTENT EN FOND (#683) : un envoi a la signature, les documents du mois, un depot reel, une demande de
//! fiche, un dossier ou la regularisation CASC-SVP font attendre le serveur de courriel ou celui de l'organisme, des
//! dizaines de secondes sur un poste. Dans la fenetre, la page n'attend plus que la verification.
//!
//! UN SEUL CHEMIN : la route est la meme, servie par la meme fonction, de bout en bout. La fenetre la fait sur un fil a
//! elle (`EnvoisEnFond::servir`) et attend ce que ce fil lui dit : un refus, ou la reponse entiere, se rend comme avant ;
//! mais quand la route arrive au point ou tout est verifie, le mot de passe compris, et ou seul le transport reste
//! (`partir`, juste avant le premier octet qui quitte le poste), la page recoit aussitot « en cours d'envoi » et la route
//! continue sur son fil. Ce qu'elle rend ensuite, partie ou refusee, se dit dans le bandeau (`EnvoisEnFond::dits`, que
//! la veille rend a la page), avec le geste qui la refait quand elle a echoue. En ligne de commande, dans la veille, dans
//! l'horloge d'une fictive et dans le navigateur (`SANS_FIL`), aucun fil n'attend ce point : `partir` ne fait rien, et la
//! route va au bout avant de rendre ce qu'elle rend.
//!
//! UN ENVOI A LA FOIS PAR COMPAGNIE, dans l'ordre des clics (`un_a_la_fois`) : le second attend que le premier soit au
//! bout avant meme de se verifier, et lit donc le journal que le premier a ecrit ; rien ne part deux fois. Fermer le
//! logiciel ou lacher la compagnie attend les envois en vol (`attendre`) : un envoi n'est jamais coupe au milieu.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};

use droit::contexte::Contexte;
use serde::{Deserialize, Serialize};
use socle::echec::{Classe, R};
use socle::memoire::{repris, tenu};
use socle::mois::Mois;
use socle::texte::Phrase;

use crate::catalogue::Route;
use crate::enveloppe::{en_json, Dite, Refus, Reponse};

/// CE QUI PART, tel qu'un envoi le dit au moment de partir (`partir`) : ce qui part, en mots (« 2 contrats », « DSN de
/// novembre 2026 »), et le geste qui le refait s'il n'arrive pas au bout (la boite d'ou il est parti) ; aucun quand ce
/// geste est a sa place, sur l'ecran d'ou l'envoi est parti.
pub(crate) struct EnvoiQuiPart {
    pub(crate) quoi: String,
    /// Le mois dont il part : le cadre de ce mois le dit (`etat-du-mois`) ; aucun pour ce qui ne part d'aucun mois (une
    /// demande de fiche, un dossier, la regularisation CASC-SVP).
    pub(crate) mois: Option<Mois>,
    pub(crate) refaire: Option<crate::etapes::Action>,
}

/// LE NUMERO D'UN ENVOI PARTI EN FOND, dans la fenetre : le premier est 1 ; la page suit l'envoi par lui.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct NumeroDEnvoi(u64);

/// Ce que le fil d'une route dit a la requete qui l'attend : l'envoi est parti en fond (son numero, sa phrase), ou la route
/// a fini avant (sa reponse).
enum Nouvelle {
    Parti { numero: NumeroDEnvoi, phrase: Phrase },
    Fini(Reponse),
}

/// LE FIL D'UNE ROUTE QUE LA FENETRE FAIT EN FOND : a qui dire que l'envoi part, et sous quel numero il est parti.
struct Attente {
    registre: Arc<EnvoisEnFond>,
    route: Route,
    dire: mpsc::Sender<Nouvelle>,
    numero: Option<NumeroDEnvoi>,
    suivre: Option<crate::depots::ChargeSuivre>,
}

thread_local! {
    /// Pose sur le fil d'une route que la fenetre sert en fond (`EnvoisEnFond::servir`), et seulement la.
    static ATTENTE: RefCell<Option<Attente>> = const { RefCell::new(None) };
}

/// L'ENVOI PART : tout est verifie, la preuve comprise, et seul le transport reste. Sur le fil d'une route que la fenetre
/// sert en fond, la page recoit maintenant « en cours d'envoi » (`ce_qui_part` le dit) et la route continue ici ;
/// partout ailleurs, rien. Une compagnie aux envois simules ne part jamais en fond : rien ne quitte le poste, rien ne
/// se fait attendre. Une seule fois par route : un second envoi de la meme route (les avenants apres les contrats, la
/// demande de taux apres la DPAE) part sur le meme fil, sous le meme numero.
pub(crate) fn partir(ctx: &Contexte, ce_qui_part: impl FnOnce() -> R<EnvoiQuiPart>) -> R<()> {
    let attendu = ATTENTE.with(|a| a.borrow().as_ref().is_some_and(|x| x.numero.is_none()));
    if !attendu || compagnie::bridage::branchement(&ctx.racine)?.joue() {
        return Ok(());
    }
    let c = ce_qui_part()?;
    ATTENTE.with(|a| {
        if let Some(x) = a.borrow_mut().as_mut() {
            let phrase = en_cours(&c.quoi);
            let numero = x.registre.inscrire(x.route, c);
            x.numero = Some(numero);
            // La requete attend sur l'autre bout : elle ne s'en va qu'en lisant cette nouvelle ou la reponse.
            if x.dire.send(Nouvelle::Parti { numero, phrase }).is_err() {
                compagnie::diagnostic::noter_moteur("Un envoi parti en fond n'a pu le dire à la page qui l'attendait.");
            }
        }
    });
    Ok(())
}

/// CE QUI SE SUIT APRES UN DEPOT REEL PARTI EN FOND : ses retours, que la page releve comme apres un depot qu'elle a
/// attendu (`suivre-depot`). Hors d'un fil que la fenetre sert en fond, rien.
pub(crate) fn a_suivre(charge: crate::depots::ChargeSuivre) {
    ATTENTE.with(|a| {
        if let Some(x) = a.borrow_mut().as_mut().filter(|x| x.numero.is_some()) {
            x.suivre = Some(charge);
        }
    });
}

/// « 2 contrats en cours d'envoi… »
fn en_cours(quoi: &str) -> Phrase {
    socle::phrase!("{} en cours d'envoi…", quoi)
}

/// LES ROUTES QUI PARTENT EN FOND DANS LA FENETRE : celles qui engagent (leur fiche, `Acte::Engage`). Une route qui
/// engage sans transport long n'atteint jamais `partir` : elle rend comme avant.
pub fn part_en_fond(route: Route) -> bool {
    crate::catalogue::FICHES.iter().any(|l| crate::catalogue::dans(l, route).is_some_and(|f| f.acte == crate::catalogue::Acte::Engage))
}

/// CE QUE LA PAGE RECOIT D'UN ENVOI PARTI EN FOND : sa phrase, et son numero (`en_fond`), que le bandeau suit.
#[derive(Serialize)]
struct PartiEnFond {
    phrase: Phrase,
    en_fond: NumeroDEnvoi,
}

impl Dite for PartiEnFond {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// Ce qu'un envoi a rendu, une fois au bout : la phrase de sa route, ou celle de son refus.
#[derive(Debug, Clone)]
enum Issue {
    Parti(String),
    Echoue(String),
}

/// UN ENVOI PARTI EN FOND : son numero, sa route, ce qui part, le geste qui le refait, ce qui se suit apres lui, et son
/// issue (aucune tant qu'il est en cours).
#[derive(Debug, Clone)]
struct EnvoiEnFond {
    numero: NumeroDEnvoi,
    route: Route,
    quoi: String,
    mois: Option<Mois>,
    refaire: Option<crate::etapes::Action>,
    suivre: Option<crate::depots::ChargeSuivre>,
    issue: Option<Issue>,
    /// Au bout, il etait deja dit quand un passage de la veille est venu (`oublier_les_partis`).
    dit_au_passage: bool,
}

/// OU EN EST UN ENVOI, tel que le bandeau le dessine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EtatDUnEnvoi {
    EnCours,
    Parti,
    Echoue,
}

/// UN ENVOI PARTI EN FOND, TEL QUE LE BANDEAU LE DIT (`veille::DitDuBandeau`) : son numero, ou il en est, sa phrase, le
/// geste qui le refait (echoue, quand il en a un), et ce qui se suit apres lui (un depot reel parti : `suivre-depot`).
#[derive(Debug, Clone, Serialize)]
pub struct EnvoiDit {
    numero: NumeroDEnvoi,
    etat: EtatDUnEnvoi,
    phrase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    refaire: Option<crate::etapes::Action>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suivre: Option<crate::depots::ChargeSuivre>,
}

/// LES ENVOIS PARTIS EN FOND D'UNE COMPAGNIE, dans la fenetre : ceux en cours, et ceux qui sont au bout tant que le bandeau
/// les dit (`oublier_les_partis`). Un par compagnie, que la fenetre garde quand la compagnie se ferme et se rouvre.
#[derive(Default)]
pub struct EnvoisEnFond {
    envois: Mutex<Vec<EnvoiEnFond>>,
    /// Les routes servies en fond qui ne sont pas au bout, parties ou non encore (`attendre`).
    en_vol: Mutex<usize>,
    au_bout: Condvar,
    /// Un envoi a la fois (`servir`).
    un_a_la_fois: Mutex<()>,
    dernier: AtomicU64,
}

/// La phrase d'un rendu, seule, lue en sa forme.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PhraseRendue {
    phrase: String,
}

/// LES ENVOIS PARTIS EN FOND DE CHAQUE COMPAGNIE DE CE PROCESSUS, par dossier : la fenetre les sert (`servir`) et les
/// attend (`tous`, `lacher`), le cadre d'un mois les dit (`etat-du-mois`). En ligne de commande, rien n'y entre.
static PAR_COMPAGNIE: Mutex<Vec<(std::path::PathBuf, Arc<EnvoisEnFond>)>> = Mutex::new(Vec::new());

/// La cle d'une compagnie : son dossier resolu ; un dossier qui ne se resout plus (efface, deplace) se nomme par son
/// chemin, qui le designe encore ; un dossier qui ne se resout pas pour une autre raison aussi, et cela se note.
fn cle(racine: &std::path::Path) -> std::path::PathBuf {
    match socle::fichiers::resolu(racine) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => racine.to_path_buf(),
        Err(e) => {
            compagnie::diagnostic::noter_moteur(&socle::echec::du_disque(&racine, &e).en_lignes());
            racine.to_path_buf()
        }
    }
}

/// Les envois partis en fond de la compagnie `racine`, nes a son premier envoi ou a sa premiere lecture.
pub fn de_la_compagnie(racine: &std::path::Path) -> Arc<EnvoisEnFond> {
    let c = cle(racine);
    let mut toutes = tenu(&PAR_COMPAGNIE);
    match toutes.iter().find(|(k, _)| *k == c) {
        Some((_, e)) => e.clone(),
        None => {
            let e = EnvoisEnFond::nouveaux();
            toutes.push((c, e.clone()));
            e
        }
    }
}

/// Ceux de toutes les compagnies (fermer le logiciel les attend).
pub fn tous() -> Vec<Arc<EnvoisEnFond>> {
    tenu(&PAR_COMPAGNIE).iter().map(|(_, e)| e.clone()).collect()
}

/// Le logiciel se ferme (`fermer_apres_les_envois`) : le bandeau dit qu'il attend la fin d'un envoi.
static FERMETURE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// FERMER LE LOGICIEL, OU REDEMARRER POUR LA MISE A JOUR, par son bouton ou par la croix de la fenetre : un envoi parti
/// en fond va d'abord au bout, jamais coupe au milieu ; le bandeau le dit tant qu'il attend (`DitDuBandeau::fermeture`).
/// Chaque transport a son delai : l'attente a une fin.
pub fn fermer_apres_les_envois() {
    FERMETURE.store(true, Ordering::SeqCst);
    for e in tous() {
        e.attendre();
    }
}

/// LA COMPAGNIE S'EN VA (supprimee, deplacee) : ses envois, retires, pour qu'on les attende.
pub fn lacher(racine: &std::path::Path) -> Option<Arc<EnvoisEnFond>> {
    let c = cle(racine);
    let mut toutes = tenu(&PAR_COMPAGNIE);
    let i = toutes.iter().position(|(k, _)| *k == c)?;
    Some(toutes.remove(i).1)
}

/// Une route servie en fond n'est plus en vol, quoi qu'il lui soit arrive.
struct EnVol(Arc<EnvoisEnFond>);

impl Drop for EnVol {
    fn drop(&mut self) {
        *tenu(&self.0.en_vol) -= 1;
        self.0.au_bout.notify_all();
    }
}

impl EnvoisEnFond {
    pub fn nouveaux() -> Arc<EnvoisEnFond> {
        Arc::new(EnvoisEnFond::default())
    }

    /// SERT `route` POUR LA PAGE, la route faite par `faire` (la meme qu'en ligne de commande, `interface::api::repondre`)
    /// sur un fil a elle. Rend ce que ce fil dit d'abord : la reponse entiere (un refus, ou une route allee au bout sans
    /// rien faire partir), ou, des que l'envoi part (`partir`), « en cours d'envoi » ; la route continue alors en fond, et
    /// son issue se range ici. Sans fil (`SANS_FIL`), la route se fait sur place.
    pub fn servir(self: &Arc<Self>, route: Route, faire: impl FnOnce() -> Reponse + Send + 'static) -> Reponse {
        if crate::SANS_FIL {
            return faire();
        }
        let (dire, ecouter) = mpsc::channel::<Nouvelle>();
        *tenu(&self.en_vol) += 1;
        let en_vol = EnVol(self.clone());
        let moi = self.clone();
        let demarre = crate::fil_de_routes(move || {
            let _en_vol = en_vol;
            let _un_seul = tenu(&moi.un_a_la_fois);
            ATTENTE.with(|a| *a.borrow_mut() = Some(Attente { registre: moi.clone(), route, dire: dire.clone(), numero: None, suivre: None }));
            let fait = std::panic::catch_unwind(std::panic::AssertUnwindSafe(faire));
            let attente = ATTENTE.with(|a| a.borrow_mut().take());
            let reponse = match fait {
                Ok(r) => r,
                Err(_) => Reponse::refusee_par(route, Refus::nouveau(Classe::DefautDuLogiciel, format!("Défaut du logiciel : la route {} s'est arrêtée.", route))),
            };
            match attente.and_then(|a| a.numero.map(|n| (n, a.suivre))) {
                Some((numero, suivre)) => moi.finir(numero, &reponse, suivre),
                None => {
                    if dire.send(Nouvelle::Fini(reponse)).is_err() {
                        compagnie::diagnostic::noter_moteur("Une route servie en fond a fini sans personne pour lire sa réponse.");
                    }
                }
            }
        });
        if !demarre {
            return Reponse::refusee_par(route, Refus::nouveau(Classe::DefautDuLogiciel, "Défaut du logiciel : le fil de l'envoi ne démarre pas. Rien n'est parti."));
        }
        match ecouter.recv() {
            Ok(Nouvelle::Fini(r)) => r,
            Ok(Nouvelle::Parti { numero, phrase }) => Reponse::du_geste(route, en_json(PartiEnFond { phrase, en_fond: numero }), true),
            Err(_) => Reponse::refusee_par(route, Refus::nouveau(Classe::DefautDuLogiciel, "Défaut du logiciel : le fil de l'envoi s'est arrêté sans rien dire.")),
        }
    }

    /// L'envoi part : il prend son numero, et remplace ce que le bandeau disait d'un envoi de la meme route au bout.
    fn inscrire(&self, route: Route, c: EnvoiQuiPart) -> NumeroDEnvoi {
        let numero = NumeroDEnvoi(self.dernier.fetch_add(1, Ordering::SeqCst) + 1);
        let mut envois = tenu(&self.envois);
        envois.retain(|e| e.route != route || e.issue.is_none());
        envois.push(EnvoiEnFond { numero, route, quoi: c.quoi, mois: c.mois, refaire: c.refaire, suivre: None, issue: None, dit_au_passage: false });
        numero
    }

    /// L'envoi `numero` est au bout : la phrase de sa route, ou son refus.
    fn finir(&self, numero: NumeroDEnvoi, r: &Reponse, suivre: Option<crate::depots::ChargeSuivre>) {
        let issue = match r.refus() {
            Some(x) => Issue::Echoue(x.phrase.clone()),
            None => match r.lire_cles::<PhraseRendue>(&["phrase"]) {
                Ok(p) => Issue::Parti(p.phrase),
                Err(x) => Issue::Echoue(x.phrase),
            },
        };
        if let Some(e) = tenu(&self.envois).iter_mut().find(|e| e.numero == numero) {
            e.issue = Some(issue);
            e.suivre = suivre;
        }
    }

    /// Un envoi est-il en cours (parti, pas au bout) ?
    pub fn en_cours(&self) -> bool {
        tenu(&self.envois).iter().any(|e| e.issue.is_none())
    }

    /// Une route servie en fond est-elle en vol (partie ou non encore) ?
    pub fn en_vol(&self) -> bool {
        *tenu(&self.en_vol) > 0
    }

    /// Le logiciel attend-il, pour se fermer, un envoi de cette compagnie (`fermer_apres_les_envois`) ?
    pub fn fermeture_attend(&self) -> bool {
        FERMETURE.load(Ordering::SeqCst) && self.en_vol()
    }

    /// ATTENDRE QUE TOUT SOIT AU BOUT : fermer le logiciel ou lacher la compagnie ne coupe aucun envoi au milieu. Chaque
    /// transport a son delai (`envois::courriel::DELAI`, celui des organismes) : l'attente a une fin.
    pub fn attendre(&self) {
        let mut n = tenu(&self.en_vol);
        while *n > 0 {
            n = repris(self.au_bout.wait(n));
        }
    }

    /// UN PASSAGE DE LA VEILLE (`Veille::tour`) : un envoi parti sans refus se dit jusqu'au passage qui suit celui-ci (un
    /// passage entier au moins), puis plus ; un envoi qui a echoue reste dit jusqu'a ce qu'un envoi de la meme route
    /// reparte.
    pub fn oublier_les_partis(&self) {
        let mut envois = tenu(&self.envois);
        envois.retain(|e| !(e.dit_au_passage && matches!(e.issue, Some(Issue::Parti(_)))));
        for e in envois.iter_mut().filter(|e| matches!(e.issue, Some(Issue::Parti(_)))) {
            e.dit_au_passage = true;
        }
    }

    /// CE QUE LE BANDEAU DIT DES ENVOIS : « 2 contrats en cours d'envoi… », puis la phrase de la route au bout, ou ce qui a
    /// echoue et le geste qui le refait.
    pub fn dits(&self) -> Vec<EnvoiDit> {
        self.dits_de(|_| true)
    }

    /// Ceux qui partent de `mois`, tels que le cadre du mois les dit (`etat-du-mois`).
    pub(crate) fn dits_du_mois(&self, mois: Mois) -> Vec<EnvoiDit> {
        self.dits_de(|e| e.mois == Some(mois))
    }

    fn dits_de(&self, garder: impl Fn(&EnvoiEnFond) -> bool) -> Vec<EnvoiDit> {
        tenu(&self.envois)
            .iter()
            .filter(|e| garder(e))
            .map(|e| {
                let (etat, phrase) = match &e.issue {
                    None => (EtatDUnEnvoi::EnCours, en_cours(&e.quoi).to_string()),
                    Some(Issue::Parti(p)) => (EtatDUnEnvoi::Parti, p.clone()),
                    Some(Issue::Echoue(p)) => (EtatDUnEnvoi::Echoue, format!("{} : l'envoi n'est pas allé au bout. {}", e.quoi, p)),
                };
                let echoue = etat == EtatDUnEnvoi::Echoue;
                EnvoiDit {
                    numero: e.numero,
                    etat,
                    phrase,
                    refaire: e.refaire.clone().filter(|_| echoue),
                    suivre: e.suivre.clone().filter(|_| etat == EtatDUnEnvoi::Parti),
                }
            })
            .collect()
    }
}
