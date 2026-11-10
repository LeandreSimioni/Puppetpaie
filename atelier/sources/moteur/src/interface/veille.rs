//! La veille de la fenetre : relever les retours, les signatures et les fiches, puis
//! sauvegarder, a l'ouverture puis toutes les `minutes`. En arriere-plan, jamais dans une requete ; un tour a la fois.
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
use routes::veille::{dit_du_bandeau, EtatDeLaVeille};
use routes::etat_garde::FileDesEtats;
use routes::catalogue::Route;
use routes::enveloppe::{Charge, Refus};

pub struct Veille {
    racine: PathBuf,
    pub minutes: u64,
    jours: i64,
    erreur: Mutex<Option<String>>,
    en_cours: AtomicBool,
    sauvegarde_en_cours: AtomicBool,
    tour_unique: Mutex<()>,
    arretee: AtomicBool,
    /// Quand partira le prochain tour de la boucle (« prochain relevé dans … minutes »).
    prochain: Mutex<Option<Instant>>,
    /// Les etats des mois, a verifier apres chaque tour : un retour releve, une signature notee changent un mois.
    etats: Option<Arc<FileDesEtats>>,
}

/// OU EN EST LE PASSAGE (`Veille::etape`) : la releve ou la sauvegarde qui tournent, et l'etape que l'ouverture ecrit.
#[derive(Debug, Clone, Serialize)]
pub struct EtapeDeLaVeille {
    pub en_cours: bool,
    pub sauvegarde_en_cours: bool,
    pub etape: Option<String>,
}

impl Veille {
    pub fn nouvelle(racine: &Path, minutes: u64, etats: Option<Arc<FileDesEtats>>) -> Arc<Veille> {
        Arc::new(Veille {
            racine: racine.to_path_buf(),
            minutes,
            jours: 30,
            erreur: Mutex::new(None),
            en_cours: AtomicBool::new(false),
            sauvegarde_en_cours: AtomicBool::new(false),
            tour_unique: Mutex::new(()),
            arretee: AtomicBool::new(false),
            prochain: Mutex::new(None),
            etats,
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
                Err(e) => Some(e.into()),
            }
        } else {
            None
        };
        EtapeDeLaVeille { en_cours, sauvegarde_en_cours, etape }
    }

    /// L'etat de la veille : la trace du dernier passage relue (une trace qui ne se lit pas se dit dans `erreur`).
    pub fn etat(&self) -> EtatDeLaVeille {
        let (dernier, illisible) = match routes::gestes::ouverture::derniere_ouverture(&self.racine) {
            Ok(t) => (t, None),
            Err(e) => (None, Some(e.into())),
        };
        // Un marqueur qui ne se lit pas ne dit pas si les envois sont joues : le bandeau les dit vrais, et l'erreur le dit.
        let (fictive, marqueur) = match compagnie::bridage::branchement(&self.racine) {
            Ok(b) => (b.joue(), None),
            Err(e) => (false, Some(socle::echec::phrase_de(&e))),
        };
        let erreur = match (tenu(&self.erreur).clone(), illisible, marqueur) {
            (Some(e), _, _) | (None, Some(e), _) | (None, None, Some(e)) => Some(e),
            (None, None, None) => None,
        };
        let EtapeDeLaVeille { en_cours, sauvegarde_en_cours, etape } = self.etape();
        EtatDeLaVeille {
            dit: dit_du_bandeau(dernier.as_ref(), self.prochain_dans(), fictive),
            en_cours,
            sauvegarde_en_cours,
            etape,
            minutes: self.minutes,
            dernier,
            erreur,
        }
    }

    /// Dans combien de minutes part le prochain tour, arrondi a la minute superieure (au moins 1) ; aucun sans boucle.
    fn prochain_dans(&self) -> Option<u64> {
        let p = *tenu(&self.prochain);
        match p {
            Some(t) if self.minutes > 0 => Some(t.saturating_duration_since(Instant::now()).as_secs().div_ceil(60).max(1)),
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
    pub fn tour(&self, veille: bool, sauvegarder: bool) -> EtatDeLaVeille {
        let _un_seul = match self.tour_unique.try_lock() {
            Ok(g) => g,
            Err(std::sync::TryLockError::WouldBlock) => return self.etat(),
            Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        };
        self.en_cours.store(true, Ordering::SeqCst);
        *tenu(&self.erreur) = None;
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

    /// Le premier tour, complet, puis un tour toutes les `minutes` tant que la veille n'est pas arretee.
    pub fn demarrer(self: &Arc<Self>) {
        let v = self.clone();
        routes::fil_de_routes(move || {
            // La mise a jour ne se cherche pas ici, a l'ouverture d'une compagnie : c'est le logiciel qui la cherche pour
            // lui-meme, au lancement (`maj::chercher_en_fond`).
            v.tour(false, true);
            while v.minutes > 0 {
                *tenu(&v.prochain) = Some(Instant::now() + Duration::from_secs(v.minutes * 60));
                for _ in 0..v.minutes * 60 {
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

