//! ACTIVE OU ARCHIVEE : la fiche que l'import cree (un salarie, un spectacle) est active si une piece reprise la porte
//! dans les douze derniers mois, archivee sinon (Léandre, 21/11/2026 : « tous les salariés qui viennent des années
//! précédentes [...] ils doivent être mis comme archivés [...] Si jamais on doit les embaucher à nouveau, là on
//! demandera »). Une fiche archivee garde tout son historique ; seule « Pour démarrer » ne lui demande rien. Une fiche qui
//! existe deja n'est jamais rangee par l'import (`routes::importation::completer`).

use socle::date::{Duree, Jour};
use socle::echec::R;
use socle::mois::Mois;

/// LA DERNIERE FOIS QUE LES PIECES REPRISES PORTENT UN SALARIE : le dernier mois qu'elles datent (un bulletin, la fin de
/// la periode de sa paie ; une DSN, une AEM ou un certificat, la fin du contrat), ou un contrat qui court encore (sans
/// fin : un CDI, un contrat que l'AEM dit « en cours »). `Aucune` : aucune piece ne le date.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DernierePiece {
    #[default]
    Aucune,
    Mois(Mois),
    CourtEncore,
}

impl DernierePiece {
    /// Un mois qu'une piece porte : le plus recent l'emporte.
    pub fn porte(&mut self, m: Mois) {
        *self = match *self {
            DernierePiece::CourtEncore => DernierePiece::CourtEncore,
            DernierePiece::Mois(d) if d >= m => DernierePiece::Mois(d),
            _ => DernierePiece::Mois(m),
        };
    }

    /// Un contrat qu'une piece porte, par sa fin : le mois ou il finit ; sans fin, il court encore.
    pub fn contrat(&mut self, fin: Option<Jour>) {
        match fin {
            Some(f) => self.porte(f.mois_civil()),
            None => *self = DernierePiece::CourtEncore,
        }
    }

    /// Ce que d'autres pieces disent du meme salarie.
    pub fn suivre(&mut self, autre: DernierePiece) {
        match autre {
            DernierePiece::Aucune => {}
            DernierePiece::Mois(m) => self.porte(m),
            DernierePiece::CourtEncore => *self = DernierePiece::CourtEncore,
        }
    }

    /// La fiche que l'import cree est-elle active au jour `aujourdhui` ? Rien quand aucune piece ne la date : la case
    /// reste vide (une absence ne devient ni active ni archivee).
    pub fn active(&self, aujourdhui: Jour) -> R<Option<bool>> {
        match *self {
            DernierePiece::Aucune => Ok(None),
            DernierePiece::CourtEncore => Ok(Some(true)),
            DernierePiece::Mois(m) => recente(m, aujourdhui).map(Some),
        }
    }
}

/// LE SEUIL, le seul : un mois porte par une piece est recent s'il est au plus douze mois avant le mois de
/// `aujourdhui` (ou apres lui : la fin a venir d'un contrat).
pub fn recente(m: Mois, aujourdhui: Jour) -> R<bool> {
    Ok(m >= aujourdhui.mois_civil().avant(Duree::mois(12))?)
}
