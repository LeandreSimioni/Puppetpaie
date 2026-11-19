//! LES SUITES D'UN ENVOI : ce qu'un envoi releve en chemin et que `moteur` fait ensuite, parce que cela touche a la
//! compagnie et non au transport. Un envoi les remplit au fil de l'eau, meme quand il echoue ; l'appelant les fait
//! toutes, que l'envoi ait abouti ou non.

use std::path::PathBuf;

use crate::etats::IncidentDeRetour;
use crate::retours::Annonce;

/// Un depot refuse, a noter au journal des incidents (test comme reel).
#[derive(Clone, Debug)]
pub struct DepotRefuse {
    /// Une DSN ou une DPAE.
    pub quoi: crate::journal::Acte,
    pub mode: socle::mode::Mode,
    pub classe: Option<socle::echec::Classe>,
    pub message: String,
    /// Ce qui a ouvert le depot reel (`None` pour un test).
    pub repetition: Option<crate::journal::Ouverture>,
}

/// CE QU'UNE RELEVE A A DIRE, en phrases : ce qui n'a pas pu se faire (`incidents`), et ce qu'un retour laisse a faire a
/// la compagnie (`a_faire` : un taux a reporter, la conduite apres un signalement refuse). Le releve le remplit dans ses
/// suites (`Suites::dit`), qui y ajoutent le leur (`routes::gestes::suites::faire`) ; chaque route qui releve le rend en
/// entier.
#[derive(Default, Debug, Clone, serde::Serialize)]
pub struct ADire {
    pub incidents: Vec<String>,
    pub a_faire: Vec<String>,
}

impl ADire {
    /// Ajoute ce que `autre` a a dire.
    pub fn etendre(&mut self, autre: ADire) {
        self.incidents.extend(autre.incidents);
        self.a_faire.extend(autre.a_faire);
    }

    /// Toutes ses phrases : les incidents, puis ce qu'il reste a faire.
    pub fn phrases(self) -> Vec<String> {
        self.incidents.into_iter().chain(self.a_faire).collect()
    }
}

/// Les suites d'un envoi.
#[derive(Default, Debug)]
pub struct Suites {
    /// Les incidents que le rattachement des retours a releves (`etats::AvecIncidents`).
    pub incidents_des_retours: Vec<IncidentDeRetour>,
    /// Les attestations France Travail archivees, a ranger.
    pub aer: Vec<PathBuf>,
    /// Les comptes rendus DGFiP de nature 94 archives (le retour tel que la liste le decrit, l'archive) : leurs taux
    /// s'ecrivent dans la fiche de leur salarie.
    pub taux_pas: Vec<(Annonce, PathBuf)>,
    /// Les notifications du taux AT/MP de nature 34 archivees (le retour, l'archive) : leur code et leur taux se posent
    /// dans la surcouche de l'annee.
    pub taux_atmp: Vec<(Annonce, PathBuf)>,
    /// Un depot refuse.
    pub depot_refuse: Option<DepotRefuse>,
    /// Les pieces de retour que leur lecteur n'a pas lues : a noter chez l'editeur.
    pub illisibles: Vec<crate::retours::PieceIllisible>,
    /// Les retours du canal DSN que les suites lisent (`taux_pas`, `taux_atmp`) : ils se notent vus une fois lus
    /// (`releve_retours::noter_vus`) ; une piece que son lecteur ne lit pas reste neuve, et le releve suivant la relit.
    pub vus_une_fois_lus: Vec<Annonce>,
    /// Les retours que l'organisme a retires (HTTP 410 au telechargement) : leur nature, l'adresse de telechargement qui
    /// les identifie (guide de l'API DSN v6.9, § 3.4.4, l. 1756-1771 : l'identifiant n'est la « que pour
    /// rétrocompatibilité », l'URL le remplace) et le refus qui l'a dit. Notes vus, ils ne reviendront pas ; ils se notent
    /// chez l'editeur.
    pub retires: Vec<(crate::retours::NatureDuRetour, socle::garde_reseau::Url, socle::echec::Echec)>,
    /// Ce que l'envoi a deja a dire (les incidents de sa releve, ce que ses retours laissent a faire) : les suites y
    /// ajoutent le leur.
    pub dit: ADire,
}

impl Suites {
    /// Le resultat d'un `AvecIncidents`, ses incidents et ses pieces illisibles gardes pour l'appelant.
    pub fn garder<T>(&mut self, r: crate::etats::AvecIncidents<T>) -> socle::echec::R<T> {
        self.incidents_des_retours.extend(r.incidents);
        self.illisibles.extend(r.illisibles);
        r.resultat
    }
}
