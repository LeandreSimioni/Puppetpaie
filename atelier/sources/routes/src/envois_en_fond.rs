//! LES ENVOIS QUI PARTENT EN FOND (#683) : un envoi a la signature, les documents du mois, un depot reel, une demande de
//! fiche, un dossier ou la regularisation CASC-SVP font attendre le serveur de courriel ou celui de l'organisme, des
//! dizaines de secondes sur un poste. Dans la fenetre, la page n'attend plus que la verification.
//!
//! UN SEUL CHEMIN : la route est la meme, servie par la meme fonction, de bout en bout. La fenetre la fait sur un fil a
//! elle (`EnvoisEnFond::servir`) et attend ce que ce fil lui dit : un refus, ou la reponse entiere, se rend comme avant ;
//! mais quand la route arrive au point ou tout est verifie, le mot de passe compris, et ou seul le transport reste
//! (`partir`, juste avant le premier octet qui quitte le poste), la page recoit aussitot « en cours d'envoi » et la route
//! continue sur son fil. Ce qu'elle rend ensuite, partie ou refusee, se dit dans le bandeau (`EnvoisEnFond::dits`, que
//! la veille rend a la page) et a sa place (`PlaceDeLEnvoi` : l'etape du mois, la fiche d'un salarie neuf), avec le
//! geste qui la refait quand elle a echoue. Tant qu'il est en cours, sa place le dit a la place de son geste : un second
//! clic ne le refait pas partir. En ligne de commande, dans la veille, dans l'horloge d'une fictive et dans le navigateur
//! (`SANS_FIL`), aucun fil n'attend ce point : `partir` ne fait rien, et la route va au bout avant de rendre ce qu'elle
//! rend.
//!
//! UN ENVOI A LA FOIS PAR COMPAGNIE, dans l'ordre des clics (`un_a_la_fois`) : le second attend que le premier soit au
//! bout avant meme de se verifier, et lit donc le journal que le premier a ecrit ; rien ne part deux fois. Fermer le
//! logiciel ou lacher la compagnie attend les envois en vol (`attendre`) et refuse ceux qui arrivent pendant l'attente :
//! un envoi n'est jamais coupe au milieu.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};

use droit::contexte::Contexte;
use serde::Serialize;
use socle::echec::{Classe, R};
use socle::memoire::{repris, tenu};
use socle::mois::Mois;
use socle::texte::Phrase;

use crate::catalogue::Route;
use crate::enveloppe::{en_json, Dite, Refus, Reponse};
use crate::etapes::QuelleEtape;

/// OU UN ENVOI SE DIT, en plus du bandeau : l'etape du mois d'ou il part (`etat-du-mois` la dit « en cours d'envoi » a
/// la place de son geste, puis son issue dans le cadre du mois), la demande de fiche a la personne d'adresse `adresse`
/// (la fiche d'un salarie neuf la dit en cours, a cote de son geste : une demande a une autre personne reste a faire),
/// ou ailleurs (un dossier, la regularisation CASC-SVP depuis les Exports, un signalement), ou son geste reste a sa place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlaceDeLEnvoi {
    Etape { mois: Mois, etape: QuelleEtape },
    Fiche { adresse: String },
    Ailleurs,
}

/// CE QUI PART, tel qu'un envoi le dit au moment de partir (`partir`) : ce qui part, en mots (« 2 contrats », « DSN de
/// novembre 2026 »), sa place, le geste qui le refait s'il n'arrive pas au bout (la boite d'ou il est parti ; aucun
/// quand ce geste est a sa place, sur l'ecran d'ou l'envoi est parti), et ce que l'envoi avertit deja (les salaries sans
/// courriel, la boite qui envoie).
pub(crate) struct EnvoiQuiPart {
    pub(crate) quoi: String,
    pub(crate) place: PlaceDeLEnvoi,
    pub(crate) refaire: Option<crate::etapes::Action>,
    pub(crate) avertissements: Vec<String>,
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

/// OU EN EST UN ENVOI, et la phrase qui le dit : en cours (« 2 contrats en cours d'envoi… »), parti (la phrase de sa
/// route), ou echoue (ce qui n'est pas alle au bout, et le refus). S'ecrit `etat` et `phrase`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "etat", content = "phrase", rename_all = "snake_case")]
pub enum EtatDUnEnvoi {
    EnCours(String),
    Parti(String),
    Echoue(String),
}

/// UN ENVOI PARTI EN FOND : son numero, sa route, ce qui part, sa place, le geste qui le refait, ce qu'il avertit, ce qui
/// se suit apres lui, et ou il en est.
#[derive(Debug, Clone)]
struct EnvoiEnFond {
    numero: NumeroDEnvoi,
    route: Route,
    quoi: String,
    place: PlaceDeLEnvoi,
    refaire: Option<crate::etapes::Action>,
    avertissements: Vec<String>,
    suivre: Option<crate::depots::ChargeSuivre>,
    etat: EtatDUnEnvoi,
    /// Au bout, il etait deja dit quand un passage de la veille est venu (`oublier_les_partis`).
    dit_au_passage: bool,
}

impl EtatDUnEnvoi {
    /// La couleur de l'encart qui le dit (`couleur_de_l_etat`) : en cours, une information ; parti, fait ; echoue, a
    /// refaire.
    fn couleur(&self) -> &'static str {
        socle::ecran::couleur_de_l_etat(match self {
            EtatDUnEnvoi::EnCours(_) => socle::ecran::EtatDEcran::Information,
            EtatDUnEnvoi::Parti(_) => socle::ecran::EtatDEcran::Fait,
            EtatDUnEnvoi::Echoue(_) => socle::ecran::EtatDEcran::AFaire,
        })
    }
}

/// UN ENVOI PARTI EN FOND, TEL QUE LE BANDEAU ET LE CADRE DU MOIS LE DISENT (`veille::DitDuBandeau`,
/// `etat_garde::DuMois`) : son numero, ou il en est et sa phrase, la couleur de son encart, ce qu'il avertit (au bout
/// seulement), le geste qui le refait (echoue, quand il en a un), et ce qui se suit apres lui (un depot reel parti :
/// `suivre-depot`).
#[derive(Debug, Clone, Serialize)]
pub struct EnvoiDit {
    numero: NumeroDEnvoi,
    couleur: &'static str,
    #[serde(flatten)]
    etat: EtatDUnEnvoi,
    avertissements: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refaire: Option<crate::etapes::Action>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suivre: Option<crate::depots::ChargeSuivre>,
}

/// Les routes servies en fond qui ne sont pas au bout, et si la compagnie s'en va (`lacher`) : rien de neuf ne part plus.
#[derive(Default)]
struct EnVolEtLache {
    en_vol: usize,
    lache: bool,
}

/// LES ENVOIS PARTIS EN FOND D'UNE COMPAGNIE, dans la fenetre : ceux en cours, et ceux qui sont au bout tant que le bandeau
/// les dit (`oublier_les_partis`). Un par compagnie, que la fenetre garde quand la compagnie se ferme et se rouvre.
#[derive(Default)]
pub struct EnvoisEnFond {
    envois: Mutex<Vec<EnvoiEnFond>>,
    vol: Mutex<EnVolEtLache>,
    au_bout: Condvar,
    /// Un envoi a la fois (`servir`).
    un_a_la_fois: Mutex<()>,
    dernier: AtomicU64,
}

/// LES ENVOIS PARTIS EN FOND DE CHAQUE COMPAGNIE DE CE PROCESSUS, par dossier : la fenetre les sert (`servir`) et les
/// attend (`fermer_apres_les_envois`, `lacher`), le cadre d'un mois et la fiche d'un salarie neuf les disent
/// (`existants`). En ligne de commande, rien n'y entre.
static PAR_COMPAGNIE: Mutex<Vec<(PathBuf, Arc<EnvoisEnFond>)>> = Mutex::new(Vec::new());

/// La cle d'une compagnie : son dossier resolu ; un dossier qui ne se resout plus (efface, deplace) se nomme par son
/// chemin, qui le designe encore. Un dossier qui ne se resout pas pour une autre raison se refuse en le disant.
fn cle(racine: &Path) -> R<PathBuf> {
    match socle::fichiers::resolu(racine) {
        Ok(c) => Ok(c),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(racine.to_path_buf()),
        Err(e) => Err(socle::echec::du_disque(&racine, &e)),
    }
}

/// Les envois partis en fond de la compagnie `racine`, nes a son premier envoi servi par la fenetre ou a son ouverture.
pub fn de_la_compagnie(racine: &Path) -> R<Arc<EnvoisEnFond>> {
    let c = cle(racine)?;
    let mut toutes = tenu(&PAR_COMPAGNIE);
    match toutes.iter().find(|(k, _)| *k == c) {
        Some((_, e)) => Ok(e.clone()),
        None => {
            let e = Arc::new(EnvoisEnFond::default());
            toutes.push((c, e.clone()));
            Ok(e)
        }
    }
}

/// CEUX DE LA COMPAGNIE `racine` S'IL Y EN A : une lecture (`etat-du-mois`, la fiche) ne cree rien ; aucun en ligne de
/// commande.
pub fn existants(racine: &Path) -> R<Option<Arc<EnvoisEnFond>>> {
    let c = cle(racine)?;
    Ok(tenu(&PAR_COMPAGNIE).iter().find(|(k, _)| *k == c).map(|(_, e)| e.clone()))
}

/// Ceux de `racine` pour une lecture : un registre qui ne se trouve pas se note, et la lecture se rend sans lui (une
/// lecture ne refuse pas en bloc).
fn pour_une_lecture(racine: &Path) -> Option<Arc<EnvoisEnFond>> {
    match existants(racine) {
        Ok(e) => e,
        Err(e) => {
            compagnie::diagnostic::noter_moteur(&format!("Les envois en fond ne se lisent pas ; la lecture se rend sans eux ({}).", e.en_lignes()));
            None
        }
    }
}

/// CE QUE LES ENVOIS EN FOND DISENT D'UN MOIS, a sa lecture (`etat-du-mois`, `etat-par-engagement`) : les etapes d'ou un
/// envoi est en cours et ce qui en part (que `etapes::LesDeuxEtats` dit a la place de leurs gestes), et les envois du
/// mois tels que le cadre les dit. Aucun en ligne de commande.
pub(crate) struct EnvoisDuMois {
    en_cours: Vec<(QuelleEtape, String)>,
    pub(crate) dits: Vec<EnvoiDit>,
}

impl EnvoisDuMois {
    /// Ceux du mois `mois` de la compagnie `racine`.
    pub(crate) fn de(racine: &Path, mois: Mois) -> EnvoisDuMois {
        let Some(e) = pour_une_lecture(racine) else { return EnvoisDuMois { en_cours: Vec::new(), dits: Vec::new() } };
        let en_cours = e
            .en_cours_ou()
            .into_iter()
            .filter_map(|(p, quoi)| match p {
                PlaceDeLEnvoi::Etape { mois: m, etape } if m == mois => Some((etape, quoi)),
                PlaceDeLEnvoi::Etape { .. } | PlaceDeLEnvoi::Fiche { .. } | PlaceDeLEnvoi::Ailleurs => None,
            })
            .collect();
        EnvoisDuMois { en_cours, dits: e.dits_du_mois(mois) }
    }

    /// Ce qui part en cours de l'etape `etape` (« 2 contrats »), s'il y en a.
    pub(crate) fn de_l_etape(&self, etape: QuelleEtape) -> Option<&str> {
        self.en_cours.iter().find(|(e, _)| *e == etape).map(|(_, quoi)| quoi.as_str())
    }

    /// Aucun envoi en cours ce mois : la lecture est celle que l'etat garde.
    pub(crate) fn aucun_en_cours(&self) -> bool {
        self.en_cours.is_empty()
    }
}

/// « 2 contrats en cours d'envoi… », telle que la case d'une etape la dit a la place de son geste.
pub(crate) fn phrase_en_cours(quoi: &str) -> String {
    en_cours(quoi).to_string()
}

/// Les demandes de fiche en cours de la compagnie `racine` (`EnvoisEnFond::fiches_en_cours`), pour la fiche d'un
/// salarie neuf.
pub(crate) fn fiches_en_cours(racine: &Path) -> Vec<String> {
    match pour_une_lecture(racine) {
        Some(e) => e.fiches_en_cours(),
        None => Vec::new(),
    }
}

/// LES FERMETURES EN COURS (`fermer_apres_les_envois`) : tant qu'il y en a une, aucun envoi neuf ne part, et le bandeau
/// dit qu'on attend la fin de ceux en vol. Un compte, pour qu'une fermeture qui n'a pas eu lieu ne leve pas celle d'une
/// autre (la croix et le bouton en meme temps).
static FERMETURES: AtomicUsize = AtomicUsize::new(0);

/// Le logiciel se ferme-t-il ?
fn se_ferme() -> bool {
    FERMETURES.load(Ordering::SeqCst) > 0
}

/// CE QUI EMPECHE UN ENVOI DE PARTIR, s'il y a lieu : le logiciel qui se ferme, la compagnie qu'on lache. Regarde a
/// l'entree de `servir`, puis de nouveau quand vient son tour (`un_a_la_fois`), avant toute verification.
fn empeche(lache: bool) -> Option<&'static str> {
    if se_ferme() {
        Some("PuppetPaie se ferme : rien n'est parti. Rouvrez-le, puis recommencez.")
    } else if lache {
        Some("Cette compagnie se ferme sur ce poste : rien n'est parti.")
    } else {
        None
    }
}

/// FERMER LE LOGICIEL, OU REDEMARRER POUR LA MISE A JOUR, par son bouton ou par la croix de la fenetre : des maintenant,
/// un envoi neuf se refuse ; ceux en vol, de toutes les compagnies, vont d'abord au bout, jamais coupes au milieu, et le
/// bandeau le dit tant qu'il attend (`DitDuBandeau::fermeture`). Puis `fermer`, qui dit si la fermeture a eu lieu ; si
/// elle n'a pas eu lieu, elle ne compte plus, et sans autre fermeture en cours les envois repartent comme avant. Chaque
/// transport a son delai : l'attente a une fin.
pub fn fermer_apres_les_envois(fermer: impl FnOnce() -> bool) {
    FERMETURES.fetch_add(1, Ordering::SeqCst);
    // Une compagnie dont le registre nait pendant l'attente y entre : on ne s'arrete qu'a un tour ou rien n'est en vol.
    loop {
        let toutes: Vec<Arc<EnvoisEnFond>> = tenu(&PAR_COMPAGNIE).iter().map(|(_, e)| e.clone()).collect();
        if !toutes.iter().any(|e| e.en_vol()) {
            break;
        }
        for e in toutes {
            e.attendre();
        }
    }
    if !fermer() {
        FERMETURES.fetch_sub(1, Ordering::SeqCst);
    }
}

/// LA COMPAGNIE S'EN VA (supprimee, deplacee, gardee hors de la demo) : ce qui arrive pendant qu'on la lache se refuse,
/// ses envois en vol vont au bout, puis son registre s'en va. Le registre reste a sa place jusque-la : une requete qui
/// arrive entre-temps le trouve, et il la refuse.
pub fn lacher(racine: &Path) -> R<()> {
    let c = cle(racine)?;
    let Some(e) = tenu(&PAR_COMPAGNIE).iter().find(|(k, _)| *k == c).map(|(_, e)| e.clone()) else { return Ok(()) };
    tenu(&e.vol).lache = true;
    e.attendre();
    tenu(&PAR_COMPAGNIE).retain(|(_, x)| !Arc::ptr_eq(x, &e));
    Ok(())
}

/// Une route servie en fond n'est plus en vol, quoi qu'il lui soit arrive.
struct EnVol(Arc<EnvoisEnFond>);

impl Drop for EnVol {
    fn drop(&mut self) {
        tenu(&self.0.vol).en_vol -= 1;
        self.0.au_bout.notify_all();
    }
}

impl EnvoisEnFond {
    /// SERT `route` POUR LA PAGE, la route faite par `faire` (la meme qu'en ligne de commande, `interface::api::repondre`)
    /// sur un fil a elle. Rend ce que ce fil dit d'abord : la reponse entiere (un refus, ou une route allee au bout sans
    /// rien faire partir), ou, des que l'envoi part (`partir`), « en cours d'envoi » ; la route continue alors en fond, et
    /// son issue se range ici. Sans fil (`SANS_FIL`), la route se fait sur place. Le logiciel qui se ferme, ou la
    /// compagnie qu'on lache, refuse l'envoi, a son arrivee comme a son tour : rien ne part.
    pub fn servir(self: &Arc<Self>, route: Route, faire: impl FnOnce() -> Reponse + Send + 'static) -> Reponse {
        if crate::SANS_FIL {
            return faire();
        }
        {
            let mut vol = tenu(&self.vol);
            if let Some(pourquoi) = empeche(vol.lache) {
                return Reponse::refusee_par(route, Refus::nouveau(Classe::Refuse, pourquoi));
            }
            vol.en_vol += 1;
        }
        let (dire, ecouter) = mpsc::channel::<Nouvelle>();
        let en_vol = EnVol(self.clone());
        let moi = self.clone();
        let demarre = crate::fil_de_routes(move || {
            let _en_vol = en_vol;
            let _un_seul = tenu(&moi.un_a_la_fois);
            // Son tour venu, derriere un autre envoi : la fermeture ou le lacher demandes pendant l'attente le refusent.
            let lache = tenu(&moi.vol).lache;
            if let Some(pourquoi) = empeche(lache) {
                if dire.send(Nouvelle::Fini(Reponse::refusee_par(route, Refus::nouveau(Classe::Refuse, pourquoi)))).is_err() {
                    compagnie::diagnostic::noter_moteur("Un envoi refusé à son tour n'a pu le dire à la page qui l'attendait.");
                }
                return;
            }
            ATTENTE.with(|a| *a.borrow_mut() = Some(Attente { registre: moi.clone(), route, dire: dire.clone(), numero: None, suivre: None }));
            let fait = std::panic::catch_unwind(std::panic::AssertUnwindSafe(faire));
            let attente = ATTENTE.with(|a| a.borrow_mut().take());
            let reponse = match fait {
                Ok(r) => r,
                Err(panique) => Reponse::de(route, Err(crate::route_tombee(route, panique.as_ref()))),
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

    /// L'envoi part : il prend son numero, et remplace ce que le bandeau disait d'un envoi au bout de la meme route et de
    /// la meme place (le meme mois).
    fn inscrire(&self, route: Route, c: EnvoiQuiPart) -> NumeroDEnvoi {
        let numero = NumeroDEnvoi(self.dernier.fetch_add(1, Ordering::SeqCst) + 1);
        let mut envois = tenu(&self.envois);
        envois.retain(|e| e.route != route || e.place != c.place || matches!(e.etat, EtatDUnEnvoi::EnCours(_)));
        let etat = EtatDUnEnvoi::EnCours(en_cours(&c.quoi).to_string());
        envois.push(EnvoiEnFond { numero, route, quoi: c.quoi, place: c.place, refaire: c.refaire, avertissements: c.avertissements, suivre: None, etat, dit_au_passage: false });
        numero
    }

    /// L'envoi `numero` est au bout : la phrase de sa route, ou son refus.
    fn finir(&self, numero: NumeroDEnvoi, r: &Reponse, suivre: Option<crate::depots::ChargeSuivre>) {
        if let Some(e) = tenu(&self.envois).iter_mut().find(|e| e.numero == numero) {
            e.etat = match r.issue_dite() {
                Ok(p) => EtatDUnEnvoi::Parti(p.to_string()),
                Err(x) => EtatDUnEnvoi::Echoue(format!("{} : l'envoi n'est pas allé au bout. {}", e.quoi, x.phrase)),
            };
            e.suivre = suivre;
        }
    }

    /// Un envoi est-il en cours (parti, pas au bout) ?
    pub fn en_cours(&self) -> bool {
        tenu(&self.envois).iter().any(|e| matches!(e.etat, EtatDUnEnvoi::EnCours(_)))
    }

    /// LES ENVOIS EN COURS : leur place, et ce qui part (`quoi`).
    fn en_cours_ou(&self) -> Vec<(PlaceDeLEnvoi, String)> {
        tenu(&self.envois)
            .iter()
            .filter(|e| matches!(e.etat, EtatDUnEnvoi::EnCours(_)))
            .map(|e| (e.place.clone(), e.quoi.clone()))
            .collect()
    }

    /// Les demandes de fiche en cours, chacune sa phrase (« Demande de fiche à … en cours d'envoi… ») : la fiche d'un
    /// salarie neuf les dit a cote de son geste.
    pub(crate) fn fiches_en_cours(&self) -> Vec<String> {
        self.en_cours_ou().into_iter().filter(|(p, _)| matches!(p, PlaceDeLEnvoi::Fiche { .. })).map(|(_, quoi)| en_cours(&quoi).to_string()).collect()
    }

    /// Une route servie en fond est-elle en vol (partie ou non encore) ?
    pub fn en_vol(&self) -> bool {
        tenu(&self.vol).en_vol > 0
    }

    /// Le logiciel attend-il, pour se fermer, un envoi de cette compagnie (`fermer_apres_les_envois`) ?
    pub fn fermeture_attend(&self) -> bool {
        se_ferme() && self.en_vol()
    }

    /// ATTENDRE QUE TOUT SOIT AU BOUT : fermer le logiciel ou lacher la compagnie ne coupe aucun envoi au milieu. Chaque
    /// transport a son delai (`envois::courriel::DELAI`, celui des organismes) : l'attente a une fin.
    fn attendre(&self) {
        let mut v = tenu(&self.vol);
        while v.en_vol > 0 {
            v = repris(self.au_bout.wait(v));
        }
    }

    /// UN PASSAGE DE LA VEILLE (`Veille::tour`) : un envoi parti sans refus se dit jusqu'au passage qui suit celui-ci (un
    /// passage entier au moins), puis plus ; un envoi qui a echoue reste dit jusqu'a ce qu'un envoi de la meme route et
    /// du meme mois reparte.
    pub fn oublier_les_partis(&self) {
        let mut envois = tenu(&self.envois);
        envois.retain(|e| !(e.dit_au_passage && matches!(e.etat, EtatDUnEnvoi::Parti(_))));
        for e in envois.iter_mut().filter(|e| matches!(e.etat, EtatDUnEnvoi::Parti(_))) {
            e.dit_au_passage = true;
        }
    }

    /// CE QUE LE BANDEAU DIT DES ENVOIS : « 2 contrats en cours d'envoi… », puis la phrase de la route au bout et ce qu'il
    /// avertit, ou ce qui a echoue et le geste qui le refait.
    pub fn dits(&self) -> Vec<EnvoiDit> {
        self.dits_de(|_| true)
    }

    /// Ceux qui partent de `mois`, tels que le cadre du mois les dit (`etat-du-mois`).
    fn dits_du_mois(&self, mois: Mois) -> Vec<EnvoiDit> {
        self.dits_de(|e| matches!(e.place, PlaceDeLEnvoi::Etape { mois: m, .. } if m == mois))
    }

    fn dits_de(&self, garder: impl Fn(&EnvoiEnFond) -> bool) -> Vec<EnvoiDit> {
        tenu(&self.envois)
            .iter()
            .filter(|e| garder(e))
            .map(|e| {
                // Ce qu'il avertit (un salarie sans courriel) se dit quand l'envoi est au bout, avec ce qui est parti.
                let (refaire, suivre, avertissements) = match e.etat {
                    EtatDUnEnvoi::Echoue(_) => (e.refaire.clone(), None, e.avertissements.clone()),
                    EtatDUnEnvoi::Parti(_) => (None, e.suivre.clone(), e.avertissements.clone()),
                    EtatDUnEnvoi::EnCours(_) => (None, None, Vec::new()),
                };
                EnvoiDit { numero: e.numero, couleur: e.etat.couleur(), etat: e.etat.clone(), avertissements, refaire, suivre }
            })
            .collect()
    }
}
