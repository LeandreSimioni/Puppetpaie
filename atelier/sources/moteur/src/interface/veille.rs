//! La veille de la fenetre : relever les retours, les signatures et les fiches, puis
//! sauvegarder, a l'ouverture puis toutes les `periode`. En arriere-plan, jamais dans une requete (sauf sans fil, dans le navigateur : `suivie`) ; un tour a la fois.
//! Elle ne depose rien : elle appelle la route `ouverture`, qui ne connait que la releve et la sauvegarde. Ce qu'elle
//! dit a la page (son etat, le bandeau) : `routes::veille`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use socle::memoire::tenu;

use super::api::route;
use routes::gestes::ouverture::ChargeOuverture;
use routes::veille::{acces_a_poser, dit_du_bandeau, souci_de_la_sauvegarde, EtatDeLaVeille};
use routes::envois_en_fond::EnvoisEnFond;
use routes::etat_garde::FileDesEtats;
use routes::catalogue::Route;
use routes::enveloppe::{Charge, Refus};

pub struct Veille {
    racine: PathBuf,
    /// Le temps entre deux tours de la boucle ; zero : aucune boucle.
    pub periode: Duration,
    jours: Duration,
    erreur: Mutex<Option<String>>,
    en_cours: AtomicBool,
    sauvegarde_en_cours: AtomicBool,
    tour_unique: Mutex<()>,
    arretee: AtomicBool,
    /// Sans fil (`routes::SANS_FIL`), le premier tour reste a faire : qui suit la veille le fait (`suivie`).
    du: AtomicBool,
    /// Quand partira le prochain tour de la boucle (« prochain relevé dans … minutes »).
    prochain: Mutex<Option<Instant>>,
    /// Les etats des mois, a verifier apres chaque tour : un retour releve, une signature notee changent un mois.
    etats: Option<Arc<FileDesEtats>>,
    /// Les envois de la compagnie partis en fond (`EnvoisEnFond`) : le bandeau les dit, et le passage les suit.
    envois: Arc<EnvoisEnFond>,
}

/// OU EN EST LE PASSAGE (`Veille::etape`) : la releve ou la sauvegarde qui tournent, et l'etape que l'ouverture ecrit.
#[derive(Debug, Clone, Serialize)]
pub struct EtapeDeLaVeille {
    pub en_cours: bool,
    pub sauvegarde_en_cours: bool,
    pub etape: Option<String>,
    /// Un envoi parti en fond n'est pas au bout : la page suit le bandeau jusqu'a ce qu'il le soit.
    pub envoi_en_cours: bool,
}

impl Veille {
    pub fn nouvelle(racine: &Path, periode: Duration, etats: Option<Arc<FileDesEtats>>, envois: Arc<EnvoisEnFond>) -> Arc<Veille> {
        Arc::new(Veille {
            racine: racine.to_path_buf(),
            periode,
            jours: routes::gestes::ouverture::QUATRE_VINGT_DIX_JOURS,
            erreur: Mutex::new(None),
            en_cours: AtomicBool::new(false),
            sauvegarde_en_cours: AtomicBool::new(false),
            tour_unique: Mutex::new(()),
            arretee: AtomicBool::new(false),
            du: AtomicBool::new(false),
            prochain: Mutex::new(None),
            etats,
            envois,
        })
    }

    /// Ou en est le passage, sans rien lancer (`veille-etape`) : une lecture de drapeaux et d'un petit fichier, que la page
    /// fait toutes les secondes le temps d'un passage. L'etape est celle que l'ouverture ecrit en y entrant
    /// (`ouverture::etape_en_cours`) ; aucun passage en cours, aucune etape.
    pub fn etape(&self) -> EtapeDeLaVeille {
        let en_cours = self.en_cours.load(Ordering::SeqCst);
        let sauvegarde_en_cours = self.sauvegarde_en_cours.load(Ordering::SeqCst);
        let etape = if en_cours || sauvegarde_en_cours {
            match routes::gestes::ouverture::etape_en_cours(&self.racine) {
                Ok(e) => e,
                Err(e) => Some(e.phrase),
            }
        } else {
            None
        };
        EtapeDeLaVeille { en_cours, sauvegarde_en_cours, etape, envoi_en_cours: self.envois.en_cours() }
    }

    /// L'etat de la veille : la trace du dernier passage relue (une trace qui ne se lit pas se dit dans `erreur`).
    pub fn etat(&self) -> EtatDeLaVeille {
        let (dernier, illisible) = match routes::gestes::ouverture::derniere_ouverture(&self.racine) {
            Ok(t) => (t, None),
            Err(e) => (None, Some(e.phrase)),
        };
        // Un marqueur qui ne se lit pas ne dit pas si les envois sont joues : le bandeau les dit vrais, et l'erreur le dit.
        let (fictive, marqueur) = match compagnie::bridage::branchement(&self.racine) {
            Ok(b) => (b.joue(), None),
            Err(e) => (false, Some(e.phrase)),
        };
        // Ce qui manque, dit tel que le coffre le voit maintenant : un coffre qui ne se lit pas le dit dans l'erreur.
        let (a_poser, coffre) = match acces_a_poser(&self.racine, dernier.as_ref()) {
            Ok(a) => (a, None),
            Err(e) => (Vec::new(), Some(e.phrase)),
        };
        // Ce qui persiste de la sauvegarde, vrai maintenant : un coffre ou un jeton dont la date ne se lit pas le dit dans
        // l'erreur.
        let (souci, jeton) = match souci_de_la_sauvegarde(&self.racine, dernier.as_ref()) {
            Ok(s) => (s, None),
            Err(e) => (None, Some(e.phrase)),
        };
        let erreur = [tenu(&self.erreur).clone(), illisible, marqueur, coffre, jeton].into_iter().flatten().next();
        let EtapeDeLaVeille { en_cours, sauvegarde_en_cours, etape, .. } = self.etape();
        EtatDeLaVeille {
            dit: dit_du_bandeau(dernier.as_ref(), a_poser, souci, self.envois.dits(), self.prochain_dans(), fictive),
            // Sans fil, le tour qui reste du est un passage en cours pour la page : elle le suit, et sa demande le fait
            // (`suivie`).
            en_cours: en_cours || self.du.load(Ordering::SeqCst),
            sauvegarde_en_cours,
            etape,
            minutes: self.periode,
            dernier,
            erreur,
        }
    }

    /// L'ETAT DE LA VEILLE POUR LA PAGE QUI LA SUIT. Sans fil, le tour qui reste du (`demarrer`) se fait ici, sur place :
    /// la page recoit l'etat d'apres ce tour.
    /// Le tour du attend la mise a jour (`tour`) : en cours ailleurs, il reste du, et la page qui suit la veille le
    /// redemande.
    pub fn suivie(&self) -> EtatDeLaVeille {
        if self.du.load(Ordering::SeqCst) {
            if let Some(f) = &self.etats {
                if let Err(e) = f.attendre_la_version() {
                    *tenu(&self.erreur) = Some(e.phrase);
                    return self.etat();
                }
            }
            if self.du.swap(false, Ordering::SeqCst) {
                return self.tour(false, true);
            }
        }
        self.etat()
    }

    /// Dans combien de temps part le prochain tour, une minute au moins ; aucun sans boucle.
    fn prochain_dans(&self) -> Option<Duration> {
        let p = *tenu(&self.prochain);
        match p {
            Some(t) if !self.periode.is_zero() => Some(t.saturating_duration_since(Instant::now()).max(Duration::from_secs(60))),
            Some(_) | None => None,
        }
    }

    /// La route `ouverture`, servie pour la veille sur `charge` ; son refus est l'erreur du passage.
    fn passage(&self, charge: &ChargeOuverture) -> Result<(), Refus> {
        let c = Charge::de_forme(charge).map_err(|e| Refus::de(Some(Route::Ouverture), e))?;
        route(&self.racine, Route::Ouverture, &c).en_refus()
    }

    /// Un passage : la releve d'abord (qui ecrit la trace, et envoie les incidents a l'editeur), puis la sauvegarde seule.
    /// Un passage deja en cours : l'etat, tel quel.
    ///
    /// APRES LA MISE A JOUR : la file des etats la prend en compte d'abord (`FileDesEtats::attendre_la_version`), et le
    /// passage ne part qu'ensuite ; rien de la veille ne croise la reecriture des paies. L'attente se fait hors du tour
    /// (`tour_unique`) : la compagnie qui s'en va n'attend pas la mise a jour pour arreter sa veille, et le tour qui
    /// suit l'attente ne part pas pour une veille arretee. Sans fil, une mise a jour en cours ailleurs est l'erreur du
    /// tour, que le tour suivant reprend.
    pub fn tour(&self, veille: bool, sauvegarder: bool) -> EtatDeLaVeille {
        if let Some(f) = &self.etats {
            if let Err(e) = f.attendre_la_version() {
                *tenu(&self.erreur) = Some(e.phrase);
                return self.etat();
            }
        }
        let _un_seul = match self.tour_unique.try_lock() {
            Ok(g) => g,
            Err(std::sync::TryLockError::WouldBlock) => return self.etat(),
            Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        };
        if self.arretee.load(Ordering::SeqCst) {
            return self.etat();
        }
        self.en_cours.store(true, Ordering::SeqCst);
        *tenu(&self.erreur) = None;
        // Un envoi au bout s'est dit depuis le passage d'avant : il ne se redit plus.
        self.envois.oublier_les_partis();
        let releve = ChargeOuverture { jours: Some(self.jours), veille: Some(veille), incidents_editeur: Some(true), sauvegarder: Some(false), ..Default::default() };
        let mut erreur = self.passage(&releve).err();
        self.en_cours.store(false, Ordering::SeqCst);
        if erreur.is_none() && sauvegarder {
            self.sauvegarde_en_cours.store(true, Ordering::SeqCst);
            let sauvegarde = ChargeOuverture {
                jours: Some(self.jours),
                veille: Some(veille),
                incidents_editeur: Some(false),
                sauvegarder: Some(true),
                sauvegarde_seule: Some(true),
                ..Default::default()
            };
            erreur = self.passage(&sauvegarde).err();
        }
        *tenu(&self.erreur) = erreur.map(|r| r.phrase);
        self.en_cours.store(false, Ordering::SeqCst);
        self.sauvegarde_en_cours.store(false, Ordering::SeqCst);
        if let Some(f) = &self.etats {
            f.tout_verifier();
        }
        self.etat()
    }

    /// Le premier tour, complet, puis un tour toutes les `periode` tant que la veille n'est pas arretee. Le premier tour
    /// attend que la file ait pris en compte la mise a jour (`tour`), sur le fil de la veille : la requete qui ouvre la
    /// compagnie n'attend pas, la page voit « Prise en compte de la mise à jour… » (`demarrage`).
    /// SANS FIL (le navigateur), aucune boucle : le premier tour est du, et la page qui suit la veille le fait
    /// (`suivie`) ; ensuite, un tour se demande (« relever maintenant »).
    pub fn demarrer(self: &Arc<Self>) {
        if routes::SANS_FIL {
            self.du.store(true, Ordering::SeqCst);
            return;
        }
        let v = self.clone();
        routes::fil_de_routes(move || {
            // La mise a jour ne se cherche pas ici, a l'ouverture d'une compagnie : c'est le logiciel qui la cherche pour
            // lui-meme, au lancement (`maj::chercher_en_fond`).
            v.tour(false, true);
            while !v.periode.is_zero() {
                *tenu(&v.prochain) = Some(Instant::now() + v.periode);
                for _ in 0..v.periode.as_secs() {
                    if v.arretee.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
                v.tour(true, true);
            }
        });
    }

    pub fn arreter(&self) {
        self.arretee.store(true, Ordering::SeqCst);
    }

    /// Un passage tourne-t-il (la releve, ou la sauvegarde) ?
    pub fn occupee(&self) -> bool {
        self.en_cours.load(Ordering::SeqCst) || self.sauvegarde_en_cours.load(Ordering::SeqCst)
    }

    /// LA COMPAGNIE S'EN VA (`accueil::suppression`) : plus de tour, et celui qui aurait commence se finit avant qu'on
    /// rende la main -- rien ne s'ecrit plus dans son dossier ensuite.
    pub fn arreter_et_attendre(&self) {
        self.arreter();
        let _fini = tenu(&self.tour_unique);
    }
}

