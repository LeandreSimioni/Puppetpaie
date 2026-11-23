//! ACTIVE OU ARCHIVEE : la fiche que l'import cree (un salarie, un spectacle) est active si une piece reprise la porte
//! dans les vingt-quatre derniers mois (`RECENTE`), archivee sinon (Léandre, 21/11/2026 : « tous les salariés qui
//! viennent des années précédentes [...] ils doivent être mis comme archivés [...] Si jamais on doit les embaucher à
//! nouveau, là on demandera » ; 23/11/2026, la duree : « 24 mois », pour les spectacles et pour les salaries). Une fiche archivee garde tout son historique ; seule « Pour démarrer » ne lui demande rien. Une fiche qui
//! existe deja n'est jamais rangee par l'import (`routes::importation::completer`).

use socle::date::{Duree, EnMois, Jour};
use socle::echec::R;
use socle::mois::Mois;

/// LA DERNIERE FOIS QUE LES PIECES REPRISES PORTENT UN SALARIE : le dernier mois qu'elles datent (un bulletin, la fin de
/// la periode de sa paie ; une DSN, son mois, et la fin du contrat quand elle la dit ; une AEM, son mois, et la fin du
/// contrat quand elle la dit ; un certificat, la fin de sa periode). Un contrat sans fin ne date rien de plus : tant
/// qu'il court, chaque DSN mensuelle le declare, et c'est son mois qui date le salarie (Léandre, 21/11/2026 : « Un CDI,
/// il est toujours présent dans les DSN »). `Aucune` : aucune piece ne le date.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DernierePiece {
    #[default]
    Aucune,
    Mois(Mois),
}

impl DernierePiece {
    /// Un mois qu'une piece porte : le plus recent l'emporte.
    pub fn porte(&mut self, m: Mois) {
        *self = match *self {
            DernierePiece::Mois(d) if d >= m => DernierePiece::Mois(d),
            _ => DernierePiece::Mois(m),
        };
    }

    /// La fin d'un contrat qu'une piece dit : le mois ou il finit (a venir, il date encore le salarie).
    pub fn contrat(&mut self, fin: Jour) {
        self.porte(fin.mois_civil());
    }

    /// Ce que d'autres pieces disent du meme salarie.
    pub fn suivre(&mut self, autre: DernierePiece) {
        match autre {
            DernierePiece::Aucune => {}
            DernierePiece::Mois(m) => self.porte(m),
        }
    }

    /// La fiche que l'import cree est-elle active au jour `aujourdhui` ? Rien quand aucune piece ne la date : la case
    /// reste vide (une absence ne devient ni active ni archivee).
    pub fn active(&self, aujourdhui: Jour) -> R<Option<bool>> {
        match *self {
            DernierePiece::Aucune => Ok(None),
            DernierePiece::Mois(m) => recente(m, aujourdhui).map(Some),
        }
    }
}

/// LE TEMPS APRES LEQUEL UNE FICHE QUE RIEN NE PORTE SE RANGE PARMI LES ARCHIVEES : une decision de l'editeur, pas un
/// texte (Léandre, 23/11/2026 : « 24 mois », pour les spectacles, puis « 24 mois salariés aussi »).
pub const RECENTE: Duree<EnMois> = Duree::mois(24);

/// LE SEUIL, le seul : un mois porte par une piece est recent s'il est au plus `RECENTE` avant le mois de
/// `aujourdhui` (ou apres lui : la fin a venir d'un contrat).
pub fn recente(m: Mois, aujourdhui: Jour) -> R<bool> {
    Ok(m >= aujourdhui.mois_civil().avant(RECENTE)?)
}
