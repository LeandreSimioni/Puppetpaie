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
use socle::fichiers::NomDeFichier;
use socle::mois::{Graphie, Mois};

use crate::dsn_lue::ContratDeclare;

/// LES DSN DEPOSEES RELUES PENDANT UN GESTE : chaque fichier ne se lit qu'une fois, quel que soit le nombre de contrats
/// et de mois qui le demandent. Un geste en tient une (`DsnDeposees::de`) et la passe a chaque decision.
pub struct DsnDeposees {
    racine: PathBuf,
    /// Par chemin : les contrats que le fichier declare (`dsn_lue::ContratDeclare`), ou `None` s'il manque ou si sa
    /// forme ne se lit pas.
    lues: RefCell<HashMap<PathBuf, Option<Rc<Vec<ContratDeclare>>>>>,
}

impl DsnDeposees {
    /// Aucune encore relue, pour la compagnie de `racine`.
    pub fn de(racine: &Path) -> DsnDeposees {
        DsnDeposees { racine: racine.to_path_buf(), lues: RefCell::new(HashMap::new()) }
    }

    /// Les contrats que declare la DSN deposee `fichier` du mois `mois` ; `None` : le fichier manque, ou sa forme ne se
    /// relit pas (`dsn_lue::lire` le refuse : une DSN d'une autre version, un fichier abime). Toute autre erreur (le
    /// disque, une donnee livree qui ne se lit pas) remonte telle quelle : une panne n'est pas un fichier a remettre.
    fn contrats(&self, mois: Mois, fichier: &NomDeFichier) -> R<Option<Rc<Vec<ContratDeclare>>>> {
        let chemin = documents::fichier_depose(&self.racine, mois, fichier.texte());
        if let Some(l) = self.lues.borrow().get(&chemin) {
            return Ok(l.clone());
        }
        let lue = match std::fs::read(&chemin) {
            Ok(octets) => match crate::dsn_lue::lire(&octets, Some(fichier)) {
                Ok(dsn) => Some(Rc::new(dsn.contrats)),
                Err(e) if e.classe == Some(Classe::Refuse) => None,
                Err(e) => return Err(e),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(socle::echec::du_disque(&chemin, &e)),
        };
        self.lues.borrow_mut().insert(chemin, lue.clone());
        Ok(lue)
    }
}

/// CE QUE LA DSN SAIT DE LA FIN, ou ce qui empeche de le dire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinALaDsn {
    /// Ce que la DSN sait du contrat.
    Connue(ConnuDeLaDsn),
    /// Une DSN mensuelle reelle et acceptee du mois `mois` a porte ce contrat, mais son fichier depose (`fichier` : son
    /// nom, que le journal dit ; `None` : le journal ne le dit pas) manque ou ne se relit pas : on ne sait pas si elle a
    /// declare la fin par le circuit derogatoire.
    FichierIllisible { mois: Mois, fichier: Option<NomDeFichier> },
}

impl FinALaDsn {
    /// La phrase d'une fin dont le fichier depose ne se relit pas : quel fichier remettre, et ou.
    pub fn phrase_illisible(mois: Mois, fichier: Option<&NomDeFichier>) -> String {
        let dossier = documents::FichierDuJournal::Depose.dossier().chemin(mois);
        let quoi = match fichier {
            Some(f) => format!("Le fichier déposé de la DSN de {} ({})", mois.libelle(Graphie::Accentuee), f.texte()),
            None => format!("Le fichier déposé de la DSN de {}", mois.libelle(Graphie::Accentuee)),
        };
        format!(
            "{} manque ou ne se relit pas : on ne sait pas si elle a déjà déclaré cette fin par le circuit dérogatoire. \
Remettez ce fichier dans {} (une sauvegarde qui le contient).",
            quoi,
            dossier
        )
    }

    /// La phrase de ce qui empeche de dire la fin ; `None` : elle est connue.
    pub fn illisible(&self) -> Option<String> {
        match self {
            FinALaDsn::Connue(_) => None,
            FinALaDsn::FichierIllisible { mois, fichier } => Some(FinALaDsn::phrase_illisible(*mois, fichier.as_ref())),
        }
    }

    /// Ce que la DSN sait, pour un acte qui en a besoin : le fichier qui ne se relit pas le refuse, en le nommant.
    pub fn connue(self) -> R<ConnuDeLaDsn> {
        match self {
            FinALaDsn::Connue(c) => Ok(c),
            FinALaDsn::FichierIllisible { mois, fichier } => Err(socle::echec::erreur(Classe::Manque, FinALaDsn::phrase_illisible(mois, fichier.as_ref()))),
        }
    }
}

/// LA DECISION, ET SON SEUL ENDROIT, pour le contrat de nature `nat` du salarie `cle`, au numero `numero` (l'un ou
/// l'autre `None` : rien n'est parti sous eux, le numero est la cle de ce qui est parti), qui commence le `debut` et finit
/// le `fin` (`None` : pas de fin dite), au motif `motif` (`None` : le defaut de la nature), vu du mois `mois`. Un fichier
/// ne se relit que pour un contrat d'usage deja declare dont le motif est celui du circuit
/// (`Nature::a_son_terme_hors_du_circuit`), qui finit ce mois, et dont une DSN mensuelle de ce mois est partie
/// (`Departs::de` : reelle, acceptee, non refusee au bilan).
///
/// LA FIN PARTIE AU CIRCUIT se reconnait au numero du contrat et a 62.017 = 01, quelle que soit la date que ce bloc 62
/// declarait : la fiche 1902 dit qu'une correction, faite apres l'exigibilite, se reporte dans la DSN du mois suivant
/// « en appliquant le choix du déclarant concernant la valeur 01 ou 02 [...] précédemment transmis », et la date de fin
/// est l'une des donnees qu'une correction change. INFERENCE : seule la DSN du mois ou la fin tombe aujourd'hui se relit ;
/// une fin deplacee dans un autre mois (le contrat qui « n'est plus infra-DSN », dit la meme fiche) n'y est pas trouvee,
/// et suit le calendrier (62.017 = 02) : le choix precedent est dans la DSN d'un autre mois, que rien ne relit ici.
#[allow(clippy::too_many_arguments)]
pub fn du_contrat(lues: &DsnDeposees, departs: &Departs, cle: Option<&IdSalarie>, numero: Option<&NumeroContrat>, nat: &Nature, motif: Option<MotifDeRupture>, debut: Jour, fin: Option<Jour>, mois: Mois) -> R<FinALaDsn> {
    let calendrier = ConnuDeLaDsn::du_contrat(debut, mois);
    if !nat.a_son_terme_hors_du_circuit(motif, calendrier) {
        return Ok(FinALaDsn::Connue(calendrier));
    }
    let (Some(cle), Some(numero), Some(_)) = (cle, numero, fin.filter(|f| f.mois_civil() == mois)) else {
        return Ok(FinALaDsn::Connue(calendrier));
    };
    // La derniere DSN mensuelle de ce mois partie pour ce contrat (pas la correction qu'en porte un mois suivant).
    let partie = departs.de(cle, numero).iter().filter(|d| d.envoi == Envoi::Dsn && d.mois == Some(mois) && d.par.is_none()).max_by_key(|d| d.le.clone());
    let Some(partie) = partie else { return Ok(FinALaDsn::Connue(calendrier)) };
    let Some(fichier) = &partie.fichier else { return Ok(FinALaDsn::FichierIllisible { mois, fichier: None }) };
    let Some(contrats) = lues.contrats(mois, fichier)? else { return Ok(FinALaDsn::FichierIllisible { mois, fichier: Some(fichier.clone()) }) };
    let au_circuit = contrats.iter().any(|c| c.numero.as_ref() == Some(numero) && c.fin_declaree.is_some() && c.modalite_fin == Some(ModaliteDeDeclarationDeFin::CircuitDerogatoire));
    Ok(FinALaDsn::Connue(if au_circuit { ConnuDeLaDsn::FinPartieAuCircuitDerogatoire } else { calendrier }))
}

/// `du_contrat` pour l'engagement `e`, vu de son mois, a son motif.
pub fn de_l_engagement(lues: &DsnDeposees, departs: &Departs, e: &engagement::Engagement) -> R<FinALaDsn> {
    du_contrat(lues, departs, Some(&e.salarie), e.numero.as_ref(), &*e.nat()?, e.motif_rupture, e.contrat().debut(), e.contrat().fin(), e.mois)
}
