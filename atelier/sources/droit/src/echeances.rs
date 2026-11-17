//! LES ECHEANCES D'UN ENGAGEMENT, tirees des textes deposes : la DPAE et la remise du contrat. Celle
//! de la DSN est dans `declarations::echeance_dsn`.
//!
//! - LA DPAE : « L'embauche d'un salarié ne peut intervenir qu'après déclaration nominative » (code du travail, L. 1221-10,
//!   `atelier/reference/sources/code-du-travail/articles-20261028.md`, `xml/L1221-10.xml`) : elle se fait avant le
//!   premier jour de l'engagement.
//! - LE CONTRAT : « Le contrat de travail est transmis au salarié, au plus tard, dans les deux jours ouvrables suivant
//!   l'embauche » (L. 1242-13, `atelier/reference/sources/code-du-travail/xml/L1242-13.xml`) ; une convention plus
//!   exigeante l'avance (`clauses-conventionnelles-base.json`, `remise_du_contrat` de sa couche : la 1285, titre XV).
//!
//! INFERENCE : un jour ouvrable est un jour du lundi au samedi qui n'est pas un ferie chome de la compagnie
//! (`socle::date::JoursOuvrables`, sur `feries::chomes` ; L. 3141-3) ; « dans les deux jours ouvrables suivant l'embauche » se compte a partir du
//! lendemain de l'embauche.

use serde::Serialize;

use socle::date::{Jour, JoursOuvrables};
use socle::echec::R;
use socle::paris::ecrit::jour_et_heure;
use socle::paris::InstantOuJour;

/// Une echeance : son moment (`quand` : un jour seul, ou l'instant limite, dont l'heure se dit « à midi », « à
/// minuit »), ce qui le precede (`Tournure`), et le texte depose qui la donne. Elle se rend dans sa forme
/// (`EcheanceRendue`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(into = "EcheanceRendue")]
pub struct Echeance {
    pub quand: InstantOuJour,
    pub mots: Tournure,
    pub source: String,
}

/// CE QUI PRECEDE LE JOUR D'UNE ECHEANCE : le jour lui-meme exclu, ou inclus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tournure {
    /// « avant le » : le jour exclu.
    AvantLe,
    /// « au plus tard le » : le jour inclus.
    AuPlusTardLe,
}

impl Tournure {
    /// Avec l'annee : « avant le », « au plus tard le ».
    fn longue(self) -> &'static str {
        match self {
            Tournure::AvantLe => "avant le",
            Tournure::AuPlusTardLe => "au plus tard le",
        }
    }

    /// Sans l'annee : « au plus tard le » dit « jusqu'au » (meme jour limite, inclus).
    fn courte(self) -> &'static str {
        match self {
            Tournure::AvantLe => "avant le",
            Tournure::AuPlusTardLe => "jusqu'au",
        }
    }
}

impl Echeance {
    /// Une echeance d'un jour seul, sans heure.
    fn du_jour(jour: Jour, mots: Tournure, source: String) -> Echeance {
        Echeance { quand: InstantOuJour::Jour(jour), mots, source }
    }

    /// Le jour qu'elle dit : celui de son moment ; un instant a minuit est la fin du jour d'avant
    /// (`jour_et_heure::jour_limite` : « avant le 14/12/2026 à minuit »).
    pub fn jour(&self) -> Jour {
        jour_et_heure::jour_limite(self.quand)
    }

    /// « avant le 14/11/2026 », « avant le 15/12/2026 à midi ».
    pub fn texte(&self) -> String {
        self.dire(self.mots.longue(), &self.jour().jj_mm_aaaa())
    }

    /// Sans l'annee, dans sa tournure courte : « avant le 14/11 », « avant le 15/12 à midi », « jusqu'au 24/11 ». C'est
    /// ce qu'ecrit une case du panneau.
    pub fn court(&self) -> String {
        self.dire(self.mots.courte(), &self.jour().jj_mm())
    }

    fn dire(&self, mots: &str, jour_dit: &str) -> String {
        match jour_et_heure::heure(self.quand) {
            None => format!("{mots} {jour_dit}"),
            Some(h) => format!("{mots} {jour_dit} {}", h.dite()),
        }
    }
}

/// Une echeance telle qu'une case la rend : son jour, son texte, son texte court, sa source.
#[derive(Serialize)]
struct EcheanceRendue {
    jour: Jour,
    texte: String,
    court: String,
    source: String,
}

impl From<Echeance> for EcheanceRendue {
    fn from(e: Echeance) -> EcheanceRendue {
        EcheanceRendue { jour: e.jour(), texte: e.texte(), court: e.court(), source: e.source }
    }
}

/// La remise du contrat selon la loi : « dans les deux jours ouvrables suivant l'embauche » (L. 1242-13).
const REMISE_SELON_LA_LOI: JoursOuvrables = JoursOuvrables::de(2);

/// La DPAE d'un engagement qui commence le `debut` : avant ce jour (L. 1221-10).
pub fn dpae(debut: &Jour) -> Echeance {
    Echeance::du_jour(*debut, Tournure::AvantLe, "Code du travail, art. L. 1221-10 (atelier/reference/sources/code-du-travail/xml/L1221-10.xml)".into())
}

/// La remise du contrat a duree determinee d'un engagement qui commence le `debut` : deux jours ouvrables apres
/// l'embauche (L. 1242-13), ou plus tot si la couche de la convention `idcc` le dit pour ce metier (`code_emploi`, quand
/// l'engagement ou la fiche le dit).
pub fn remise_du_contrat(debut: &Jour, idcc: Option<&crate::fiche::Idcc>, code_emploi: Option<&crate::fiche::CodeEmploiFt>, chomes: Option<&str>) -> R<Echeance> {
    // Les feries de la compagnie (sa fiche) : ceux qu'elle chome ne sont pas ouvrables (L. 3141-3).
    let feries = |a: socle::mois::Annee| socle::feries::chomes(a, chomes);
    let loi = REMISE_SELON_LA_LOI.apres(*debut, feries)?;
    let selon_la_loi = || Echeance::du_jour(loi, Tournure::AuPlusTardLe, "Code du travail, art. L. 1242-13 (atelier/reference/sources/code-du-travail/xml/L1242-13.xml)".into());
    let Some(r) = crate::clauses::releve(idcc)?.and_then(|c| c.remise_du_contrat) else { return Ok(selon_la_loi()) };
    if code_emploi.is_some_and(|c| r.metiers.iter().any(|m| crate::fiche::CodeEmploiFt::from(m) == *c)) {
        let jour = r.jours_ouvrables_avant.avant(*debut, feries)?;
        if jour < loi {
            return Ok(Echeance::du_jour(jour, Tournure::AuPlusTardLe, format!("{}, art. {}", crate::clauses::nom(idcc), r.article)));
        }
    }
    Ok(selon_la_loi())
}
