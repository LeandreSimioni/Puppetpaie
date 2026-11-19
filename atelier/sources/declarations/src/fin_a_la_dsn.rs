//! CE QUE LA DSN SAIT DE LA FIN D'UN CONTRAT, DECIDE UNE FOIS (`droit::natures::ConnuDeLaDsn`) : infra DSN mensuelle ou
//! deja declare, d'apres son debut et le mois (`ConnuDeLaDsn::du_contrat`) ; et, pour un contrat d'usage deja declare
//! arrive a son terme, ce qui est parti : la DSN mensuelle deposee du mois de la fin a-t-elle deja declare cette fin par
//! le circuit derogatoire (S21.G00.62.017 = 01) ? Alors le choix « précédemment transmis » se reporte (fiche 1902,
//! `atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche1902-circuit-derogatoire-cddu-d-
//! 20250512.txt`), aucun signalement n'est attendu (fiche 2062 : « Aucun signalement de type FCTU n'est attendu quand
//! la dérogation est appliquée »), l'attestation est celle de l'archive de la mensuelle, et rien ne se refait (regle 4).
//!
//! Tous ceux qui ecrivent ou attendent la fin la lisent ici : le bloc 62 de la mensuelle (`dsn::fabriquer`), le FCTU
//! (`fctu::verifier`), les attestations attendues (`documents::aer::due`, `attestation_fctu`), la liste des fins a
//! signaler et la carte d'un engagement (`routes`). Ce qui est parti se lit dans les departs (`Departs::de`, le seul
//! filtre de ce qui est parti) et dans le fichier depose ; il ne se deduit ni d'une date de bascule, ni d'une liste.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use droit::contexte::IdSalarie;
use droit::natures::{ConnuDeLaDsn, ModaliteDeDeclarationDeFin, MotifDeRupture, Nature};
use engagement::departs::{Departs, Envoi};
use engagement::numeros::NumeroContrat;
use socle::date::Jour;
use socle::echec::{Classe, R};
use socle::mois::{Graphie, Mois};

/// La fin d'un contrat telle qu'une DSN deposee la declare : son numero (S21.G00.40.009), sa date (62.001) et sa
/// modalite (62.017).
#[derive(Debug, Clone)]
struct FinLue {
    numero: Option<NumeroContrat>,
    fin: Option<Jour>,
    modalite: Option<ModaliteDeDeclarationDeFin>,
}

/// LES DSN DEPOSEES RELUES PENDANT UN GESTE : chaque fichier ne se lit qu'une fois, quel que soit le nombre de contrats
/// et de mois qui le demandent. Un geste en tient une (`DsnDeposees::de`) et la passe a chaque decision.
pub struct DsnDeposees {
    racine: PathBuf,
    /// Par chemin : les fins que le fichier declare, ou `None` s'il manque ou ne se relit pas.
    lues: RefCell<HashMap<PathBuf, Option<Rc<Vec<FinLue>>>>>,
}

impl DsnDeposees {
    /// Aucune encore relue, pour la compagnie de `racine`.
    pub fn de(racine: &Path) -> DsnDeposees {
        DsnDeposees { racine: racine.to_path_buf(), lues: RefCell::new(HashMap::new()) }
    }

    /// Les fins que declare la DSN deposee `fichier` du mois `mois` ; `None` : le fichier manque, ou ne se relit pas
    /// (une restauration sans `sorties`, une DSN d'une autre version). Une panne du disque remonte.
    fn fins(&self, mois: Mois, fichier: &socle::fichiers::NomDeFichier) -> R<Option<Rc<Vec<FinLue>>>> {
        let chemin = documents::fichier_depose(&self.racine, mois, fichier.texte());
        if let Some(l) = self.lues.borrow().get(&chemin) {
            return Ok(l.clone());
        }
        let lue = match std::fs::read(&chemin) {
            Ok(octets) => match crate::dsn_lue::lire(&octets, Some(fichier)) {
                Ok(dsn) => Some(Rc::new(dsn.contrats.iter().map(|c| FinLue { numero: c.numero.clone(), fin: c.fin_declaree, modalite: c.modalite_fin }).collect())),
                Err(_) => None,
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(socle::echec::du_disque(&chemin, &e)),
        };
        self.lues.borrow_mut().insert(chemin, lue.clone());
        Ok(lue)
    }
}

/// CE QUE LA DSN SAIT DE LA FIN, ou ce qui empeche de le dire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinALaDsn {
    /// Ce que la DSN sait du contrat.
    Connue(ConnuDeLaDsn),
    /// Une DSN mensuelle reelle et acceptee du mois `mois` a porte ce contrat, mais son fichier depose manque ou ne se
    /// relit pas : on ne sait pas si elle a declare la fin par le circuit derogatoire.
    FichierIllisible { mois: Mois },
}

impl FinALaDsn {
    /// La phrase d'une fin dont le fichier depose ne se relit pas.
    pub fn phrase_illisible(mois: Mois) -> String {
        format!(
            "Le fichier déposé de la DSN de {} manque ou ne se relit pas : on ne sait pas si elle a déjà déclaré cette fin \
par le circuit dérogatoire. Remettez ce fichier dans les sorties du mois (une sauvegarde qui les contient).",
            mois.libelle(Graphie::Accentuee)
        )
    }

    /// Ce que la DSN sait, pour un acte qui en a besoin : le fichier qui ne se relit pas le refuse, en le disant.
    pub fn connue(self) -> R<ConnuDeLaDsn> {
        match self {
            FinALaDsn::Connue(c) => Ok(c),
            FinALaDsn::FichierIllisible { mois } => Err(socle::echec::erreur(Classe::Manque, FinALaDsn::phrase_illisible(mois))),
        }
    }
}

/// LA DECISION, ET SON SEUL ENDROIT, pour le contrat de nature `nat` du salarie `cle`, au numero `numero` (l'un ou
/// l'autre `None` : rien n'est parti sous eux, le numero est la cle de ce qui est parti), qui commence le `debut` et finit le `fin` (`None` : pas de fin dite), au motif `motif`
/// (`None` : le defaut de la nature), vu du mois `mois`. Un fichier ne se relit que pour un contrat d'usage deja declare
/// dont le motif est celui du circuit (`Nature::a_son_terme_hors_du_circuit`), qui finit ce mois, et dont une DSN
/// mensuelle de ce mois est partie (`Departs::de` : reelle, acceptee, non refusee au bilan).
#[allow(clippy::too_many_arguments)]
pub fn du_contrat(lues: &DsnDeposees, departs: &Departs, cle: Option<&IdSalarie>, numero: Option<&NumeroContrat>, nat: &Nature, motif: Option<MotifDeRupture>, debut: Jour, fin: Option<Jour>, mois: Mois) -> R<FinALaDsn> {
    let calendrier = ConnuDeLaDsn::du_contrat(debut, mois);
    if !nat.a_son_terme_hors_du_circuit(motif, calendrier) {
        return Ok(FinALaDsn::Connue(calendrier));
    }
    let (Some(cle), Some(numero), Some(fin)) = (cle, numero, fin.filter(|f| f.mois_civil() == mois)) else {
        return Ok(FinALaDsn::Connue(calendrier));
    };
    // La derniere DSN mensuelle de ce mois partie pour ce contrat (pas la correction qu'en porte un mois suivant).
    let partie = departs.de(cle, numero).iter().filter(|d| d.envoi == Envoi::Dsn && d.mois == Some(mois) && d.par.is_none()).max_by_key(|d| d.le.clone());
    let Some(partie) = partie else { return Ok(FinALaDsn::Connue(calendrier)) };
    let Some(fichier) = &partie.fichier else { return Ok(FinALaDsn::FichierIllisible { mois }) };
    let Some(fins) = lues.fins(mois, fichier)? else { return Ok(FinALaDsn::FichierIllisible { mois }) };
    let au_circuit = fins.iter().any(|c| c.numero.as_ref() == Some(numero) && c.fin == Some(fin) && c.modalite == Some(ModaliteDeDeclarationDeFin::CircuitDerogatoire));
    Ok(FinALaDsn::Connue(if au_circuit { ConnuDeLaDsn::FinPartieAuCircuitDerogatoire } else { calendrier }))
}

/// `du_contrat` pour l'engagement `e`, vu de son mois.
pub fn de_l_engagement(lues: &DsnDeposees, departs: &Departs, e: &engagement::Engagement) -> R<FinALaDsn> {
    du_contrat(lues, departs, Some(&e.salarie), e.numero.as_ref(), &*e.nat()?, e.motif_rupture, e.contrat().debut(), e.contrat().fin(), e.mois)
}
