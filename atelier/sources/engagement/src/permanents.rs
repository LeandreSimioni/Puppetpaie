//! CE QUE LE MOTEUR PROPOSE DES PERMANENTS, ET CE QUI LEUR EST DU, sur une periode : la seule reponse que lisent les
//! natures d'une carte, les modeles de contrat, les cases des fiches, les contrats des organismes, « Pour demarrer » et
//! les etats d'export.
//!
//! La case « emploie des permanents » de la fiche Entreprise (datee par son historique,
//! `Contexte::emploie_des_permanents_pendant`) ne regle que ce qui se PROPOSE : un CDI ou un CDD neuf. Ce qui est du a un
//! permanent engage quand elle etait cochee se montre toujours : sa paie, sa fin, ses documents, la portabilite de ses
//! garanties (CSS L. 911-8, `atelier/reference/sources/code-de-la-securite-sociale/xml/L911-8.xml`), les cases de sa
//! fiche et les contrats de prevoyance et de sante qui le couvrent.

use std::sync::Arc;

use droit::contexte::{Contexte, IdSalarie};
use droit::natures::Nature;
use socle::date::Jour;
use socle::num::Resultat;

use crate::stock::Objet;

/// Un engagement paye au mois qui court sur la periode : son salarie, sa nature, le jour ou il est paye au mois, sa fin
/// s'il en a une.
#[derive(Debug, Clone)]
pub struct Du {
    pub salarie: IdSalarie,
    pub nature: Arc<Nature>,
    pub depuis: Jour,
    pub fin: Option<Jour>,
}

/// La reponse sur une periode (`Permanents::de`).
#[derive(Debug, Clone)]
pub struct Permanents {
    /// La case de la fiche Entreprise, cochee a un jour au moins de la periode : un CDI ou un CDD neuf s'y propose.
    pub proposes: bool,
    /// Les engagements payes au mois qui courent sur la periode (commences au plus tard a sa fin, pas finis avant son
    /// debut) et qui ont commence la case cochee : ce qui leur est du se montre, la case cochee ou non aujourd'hui.
    pub dus: Vec<Du>,
}

impl Permanents {
    /// LA REPONSE SUR `debut..=fin` : la case sur la periode, et les engagements payes au mois qui y courent.
    pub fn de(ctx: &Contexte, debut: &Jour, fin: &Jour) -> Resultat<Permanents> {
        let mut dus = Vec::new();
        for o in crate::stock::objets(&ctx.racine)? {
            if o.est_non_actif() || o.importe_seul() {
                continue;
            }
            let Some((depuis, nature)) = o.au_mois_depuis()? else { continue };
            let fin_dite = o.carte().fin.as_deref().filter(|f| !f.is_empty()).map(Jour::lire).transpose()?;
            if depuis > *fin || fin_dite.is_some_and(|f| f < *debut) {
                continue;
            }
            // Un engagement au mois commence sans la case ne se calcule pas (`engagements`) : il n'est du a personne.
            if !ctx.emploie_des_permanents_pendant(&depuis, &depuis) {
                continue;
            }
            let Some(salarie) = o.carte().salarie.clone() else { continue };
            dus.push(Du { salarie, nature, depuis, fin: fin_dite });
        }
        Ok(Permanents { proposes: ctx.emploie_des_permanents_pendant(debut, fin), dus })
    }

    /// Ce qui ne vaut que pour les permanents se montre-t-il (les blocs de la fiche Entreprise, les contrats de prevoyance
    /// et de sante des permanents, les etats qui ne concernent qu'eux, « Pour demarrer ») ? Un permanent neuf s'y
    /// propose, ou un permanent y est du.
    pub fn ouverts(&self) -> bool {
        self.proposes || !self.dus.is_empty()
    }

    /// Les cases d'un permanent se montrent-elles sur la fiche du salarie `salarie` ? Un permanent neuf se propose, ou
    /// ce salarie est du.
    pub fn pour(&self, salarie: &IdSalarie) -> bool {
        self.proposes || self.dus.iter().any(|d| d.salarie == *salarie)
    }

    /// LE FILTRE UNIQUE DES NATURES : celles d'un contrat d'usage toujours ; une nature payee au mois quand un permanent
    /// neuf se propose, ou qu'un engagement du l'a. Dans l'ordre de `contrats-base.json`.
    pub fn natures(&self) -> Resultat<Vec<Arc<Nature>>> {
        Ok(droit::natures::natures()?
            .iter()
            .filter(|n| !n.mensuelle() || self.proposes || self.dus.iter().any(|d| d.nature.cle == n.cle))
            .cloned()
            .collect())
    }

    /// Les natures qu'une carte NEUVE peut prendre : celles d'un contrat d'usage, et celles payees au mois quand un
    /// permanent neuf se propose.
    pub fn natures_neuves(&self) -> Resultat<Vec<Arc<Nature>>> {
        Ok(droit::natures::natures()?.iter().filter(|n| !n.mensuelle() || self.proposes).cloned().collect())
    }
}

impl Objet {
    /// LE JOUR OU L'ENGAGEMENT EST PAYE AU MOIS, et cette nature : son debut si sa premiere carte l'est, sinon le jour de
    /// l'avenant qui lui donne une nature payee au mois ; `None` s'il ne l'est jamais, ou ne dit pas encore son debut ou
    /// sa nature (un contrat repris dont la nature est a dire, `routes::importation::nature` : rien ne le paie encore).
    pub fn au_mois_depuis(&self) -> Resultat<Option<(Jour, Arc<Nature>)>> {
        if self.carte().nature.as_deref().is_none_or(|n| n.trim().is_empty()) {
            return Ok(None);
        }
        let premiere = self.carte().nature_lue()?;
        if premiere.mensuelle() {
            let debut = crate::debut_brut(&self.brut);
            if debut.is_empty() {
                return Ok(None);
            }
            return Ok(Some((Jour::lire(&debut)?, premiere)));
        }
        for a in self.avenants() {
            if let Some(n) = a.termes.nature.as_deref().filter(|n| !n.trim().is_empty()) {
                let n = droit::natures::nature(n)?;
                if n.mensuelle() {
                    return Ok(Some((a.depuis, n)));
                }
            }
        }
        Ok(None)
    }
}
