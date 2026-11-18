//! L'EFFECTIF SALARIE ANNUEL, COMME R. 130-1 DU CODE DE LA SECURITE SOCIALE LE COMPTE
//! (`atelier/reference/sources/code-de-la-securite-sociale/articles-20261110.md`, « L'effectif au sens de la securite
//! sociale ») : la seule regle, que lisent l'Urssaf simulee des fictives (`envois::organismes_simules`, depuis les DSN
//! deposees) et l'effectif propose d'apres les paies reprises d'un ancien logiciel (`engagement::effectif_repris`).
//! Chacun dit ses contrats dans la forme qu'elle lit (`ContratDansLEffectif`) ; elle ne va rien chercher.

use std::collections::BTreeMap;

use socle::date::{Jour, Periode, PeriodeOuverte};
use socle::echec::R;
use socle::grandeurs::{Coefficient, Effectif};
use socle::mois::Mois;

use crate::natures::{MotifDeRecours, QuotiteDuContrat};

/// LE TEMPS DE TRAVAIL D'UN CONTRAT, tel que l'effectif le compte (R. 130-1, II, troisieme alinea) : a temps plein,
/// entierement (un contrat qui n'est pas a temps partiel au sens de L. 3123-1 du code du travail, le CDDU compris, que la
/// DSN dit « non concerné », S21.G00.40.014 = 99) ; a temps partiel, au rapport de ses horaires a la duree legale ou
/// conventionnelle (dernier alinea de L. 1111-2 du code du travail : la quotite du contrat, S21.G00.40.013, sur celle
/// de reference, S21.G00.40.012).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TempsDansLEffectif {
    Plein,
    Partiel { quotite: QuotiteDuContrat, reference: QuotiteDuContrat },
}

impl TempsDansLEffectif {
    /// La part d'une personne qu'un jour de ce contrat compte : une entiere a temps plein ; a temps partiel, le rapport de
    /// ses quotites, au plus une (une reference nulle ne rapporte rien : la personne compte entiere).
    fn poids(self) -> R<Coefficient> {
        Ok(match self {
            TempsDansLEffectif::Plein => Coefficient::UN,
            TempsDansLEffectif::Partiel { quotite, reference } => match quotite.rapport(reference)? {
                Some(r) => r.min(Coefficient::UN),
                None => Coefficient::UN,
            },
        })
    }
}

/// UN CONTRAT TEL QUE L'EFFECTIF LE COMPTE : la personne qui en est titulaire (`qui` : son NIR ou son NTT dans une DSN,
/// sa fiche dans les paies reprises), ses dates (un contrat sans fin court jusqu'au dernier jour du mois), son motif de
/// recours, et son temps de travail.
#[derive(Debug, Clone)]
pub struct ContratDansLEffectif<K> {
    pub qui: K,
    pub periode: PeriodeOuverte,
    pub motif_recours: Option<MotifDeRecours>,
    pub temps: TempsDansLEffectif,
}

/// L'EFFECTIF D'UN MOIS (R. 130-1, II) : chaque personne titulaire d'un contrat de travail compte a due proportion des
/// jours du mois ou elle est employee (quatrieme alinea), selon son temps de travail (`TempsDansLEffectif`) ; un CDD qui
/// remplace un salarie absent (motif de recours 01) ne compte pas (premier alinea, qui renvoie a la deuxieme phrase du 2°
/// de L. 1111-2 du code du travail). Une personne compte au plus une fois par jour, au plus fort de ses contrats de ce
/// jour.
pub fn effectif_du_mois<K: Ord + Clone>(mois: Mois, contrats: &[ContratDansLEffectif<K>]) -> R<Effectif> {
    let le_mois = Periode::du_mois(mois);
    let n = le_mois.jours().count();
    let mut par_jour: BTreeMap<K, BTreeMap<Jour, Coefficient>> = BTreeMap::new();
    for k in contrats {
        if k.motif_recours == Some(MotifDeRecours::Remplacement) {
            continue;
        }
        let Some(presence) = k.periode.croise(&le_mois) else { continue };
        let poids = k.temps.poids()?;
        let jours = par_jour.entry(k.qui.clone()).or_default();
        for j in presence.jours() {
            let p = jours.entry(j).or_insert(Coefficient::ZERO);
            *p = (*p).max(poids);
        }
    }
    let une_personne = Effectif::personnes(1);
    Ok(par_jour.values().map(|jours| (une_personne * jours.values().copied().sum::<Coefficient>()).reparti_sur(n)).sum())
}

/// L'EFFECTIF SALARIE ANNUEL : la moyenne des effectifs des mois, les mois sans salarie ecartes (R. 130-1, I, premier
/// alinea), tronquee au centieme (second alinea : « il n'est pas tenu compte de la fraction d'effectif au-delà de la
/// deuxième décimale ») : un effectif, pas un montant. `None` quand aucun mois n'a de salarie.
pub fn effectif_annuel(mois: impl IntoIterator<Item = Effectif>) -> Option<Effectif> {
    let mois: Vec<Effectif> = mois.into_iter().filter(|e| *e > Effectif::ZERO).collect();
    if mois.is_empty() {
        return None;
    }
    let moyenne = mois.iter().copied().sum::<Effectif>().reparti_sur(mois.len());
    // Le milliardieme ajoute avant de tronquer : un quotient qui tombe juste sur un centieme (un tiers fois trois)
    // s'ecrit en decimal une unite du dernier chiffre en dessous, et ne doit pas perdre ce centieme.
    Some((moyenne + Effectif::UN_MILLIARDIEME).tronque_au_centieme())
}
