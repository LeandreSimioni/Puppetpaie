//! LES FICHES QUE L'IMPORT PROPOSE : chacune une fiche typee de sa sorte (`FicheProposee`), lue par la seule porte d'une
//! fiche saisie (`droit::fiche::FicheSaisie::lire_cases`) depuis les formes que l'import lit (la DSN, le bulletin,
//! l'AEM, le certificat : `de`). Une valeur qu'une piece porte et que le type de sa case refuse laisse la case vide :
//! sa faute (`FauteDeCase`) se garde a cote, se dit, et la case se redemande ; la piece ne se refuse pas pour elle.
//! L'import l'ecrit ensuite par le seul chemin des fiches (`importation::fiches`).

use serde::Serialize;

use declarations::champs::FicheAJuger;
use droit::fiche::{CaseDeFiche, ChampsEntreprise, ChampsSalarie, ChampsSpectacle, FauteDeCase, FicheSaisie, NomDeFiche, ValeurDeCase};
use socle::echec::R;

/// Le titre que l'import donne a un spectacle dont il ne connait que le numero d'objet.
pub const A_COMPLETER: &str = "A COMPLETER";

/// LA FICHE `P` QUE DIT UNE FORME LUE (`t` : ses champs nommes comme les cases de la fiche), lue case par case par sa
/// porte (`FicheSaisie::lire_cases`) : la fiche typee, et la faute de chaque case que son type refuse (la case reste
/// vide). Une cle qui n'est pas une case de la fiche refuse : la forme du moteur est fausse.
pub fn de<P: FicheSaisie, T: Serialize + ?Sized>(t: &T) -> R<(P, Vec<FauteDeCase>)> {
    match socle::ecriture::en_objet(t) {
        Some(o) => P::lire_cases(&o),
        None => Err(socle::echec::refus(format!("La fiche {} que l'import compose ne s'écrit pas en objet : le signaler au développeur.", P::NOM))),
    }
}

/// UNE FICHE QUE L'IMPORT PROPOSE : celle de l'entreprise, d'un salarie ou d'un spectacle, typee. Elle s'ecrit comme sa
/// fiche, a plat, ses cases dans l'ordre de sa forme puis ses notes.
#[derive(Debug, Clone, PartialEq)]
pub enum FicheProposee {
    Entreprise(ChampsEntreprise),
    Salarie(ChampsSalarie),
    Spectacle(ChampsSpectacle),
}

impl Serialize for FicheProposee {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            FicheProposee::Entreprise(f) => f.serialize(s),
            FicheProposee::Salarie(f) => f.serialize(s),
            FicheProposee::Spectacle(f) => f.serialize(s),
        }
    }
}

impl FicheProposee {
    /// La fiche `nom`, vide.
    pub fn vide(nom: NomDeFiche) -> FicheProposee {
        match nom {
            NomDeFiche::Entreprise => FicheProposee::Entreprise(ChampsEntreprise::default()),
            NomDeFiche::Salarie => FicheProposee::Salarie(ChampsSalarie::default()),
            NomDeFiche::Spectacle => FicheProposee::Spectacle(ChampsSpectacle::default()),
        }
    }

    /// Son nom.
    pub fn nom(&self) -> NomDeFiche {
        self.a_juger().nom()
    }

    /// La fiche, a juger par la table (`declarations::champs::verifier_fiche`).
    pub fn a_juger(&self) -> FicheAJuger<'_> {
        match self {
            FicheProposee::Entreprise(f) => FicheAJuger::Entreprise(f),
            FicheProposee::Salarie(f) => FicheAJuger::Salarie(f),
            FicheProposee::Spectacle(f) => FicheAJuger::Spectacle(f),
        }
    }

    /// La fiche d'un salarie ; une autre n'en est pas une.
    pub fn salarie(&self) -> Option<&ChampsSalarie> {
        match self {
            FicheProposee::Salarie(f) => Some(f),
            FicheProposee::Entreprise(_) | FicheProposee::Spectacle(_) => None,
        }
    }

    /// La fiche d'un spectacle ; une autre n'en est pas une.
    pub fn spectacle(&self) -> Option<&ChampsSpectacle> {
        match self {
            FicheProposee::Spectacle(f) => Some(f),
            FicheProposee::Entreprise(_) | FicheProposee::Salarie(_) => None,
        }
    }

    /// La valeur de la case `k`, ecrite comme le fichier l'ecrit ; vide, aucune.
    pub fn valeur_ecrite(&self, k: CaseDeFiche) -> Option<ValeurDeCase> {
        match self {
            FicheProposee::Entreprise(f) => f.valeur_ecrite(k),
            FicheProposee::Salarie(f) => f.valeur_ecrite(k),
            FicheProposee::Spectacle(f) => f.valeur_ecrite(k),
        }
    }

    /// Les cases qu'elle dit, dans l'ordre de sa forme, chacune avec sa valeur.
    pub fn valeurs_dites(&self) -> Vec<(CaseDeFiche, ValeurDeCase)> {
        let cases = match self {
            FicheProposee::Entreprise(f) => f.cases_dites(),
            FicheProposee::Salarie(f) => f.cases_dites(),
            FicheProposee::Spectacle(f) => f.cases_dites(),
        };
        cases.into_iter().filter_map(|k| self.valeur_ecrite(k).map(|v| (k, v))).collect()
    }

    /// Ses cases, sans ses notes.
    pub fn sans_notes(&self) -> FicheProposee {
        match self {
            FicheProposee::Entreprise(f) => FicheProposee::Entreprise(f.clone().sans_notes()),
            FicheProposee::Salarie(f) => FicheProposee::Salarie(f.clone().sans_notes()),
            FicheProposee::Spectacle(f) => FicheProposee::Spectacle(f.clone().sans_notes()),
        }
    }

    /// Dit-elle une case ou une note ?
    pub fn est_dite(&self) -> bool {
        match self {
            FicheProposee::Entreprise(f) => !f.est_vide() || !f.notes_dites().is_empty(),
            FicheProposee::Salarie(f) => !f.est_vide() || !f.notes_dites().is_empty(),
            FicheProposee::Spectacle(f) => !f.est_vide() || !f.notes_dites().is_empty(),
        }
    }

    /// Les cases et les notes de `autre` qu'elle ne dit pas encore, posees : la premiere piece qui en dit une la garde.
    /// Les deux sont de la meme sorte ; deux sortes, une faute du logiciel.
    pub fn completer_de(&mut self, autre: &FicheProposee) -> R<()> {
        match (self, autre) {
            (FicheProposee::Entreprise(f), FicheProposee::Entreprise(a)) => f.completer_de(a),
            (FicheProposee::Salarie(f), FicheProposee::Salarie(a)) => f.completer_de(a),
            (FicheProposee::Spectacle(f), FicheProposee::Spectacle(a)) => f.completer_de(a),
            (f, a) => return Err(deux_sortes(f.nom(), a.nom())),
        }
        Ok(())
    }
}

/// Deux fiches proposees de sortes differentes, qu'on voulait reunir : une faute du logiciel, qui se dit.
pub fn deux_sortes(a: NomDeFiche, b: NomDeFiche) -> socle::echec::Echec {
    socle::echec::refus(format!("L'import réunit une fiche {} et une fiche {} : le signaler au développeur.", a, b))
}
