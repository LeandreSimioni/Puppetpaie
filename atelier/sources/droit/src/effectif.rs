//! L'EFFECTIF SALARIE ANNUEL, COMME L. 130-1 ET R. 130-1 DU CODE DE LA SECURITE SOCIALE LE COMPTENT
//! (`atelier/reference/sources/code-de-la-securite-sociale/articles-20261110.md`, « L'effectif au sens de la securite
//! sociale ») : la seule regle, que lisent l'Urssaf simulee des fictives (`envois::organismes_simules`, depuis les DSN
//! deposees) et l'effectif propose d'apres les paies reprises d'un ancien logiciel (`engagement::effectif_repris`).
//! Chacun dit ses contrats dans la forme qu'elle lit (`ContratDansLEffectif`) ; elle ne va rien chercher.

use std::collections::BTreeMap;

use socle::date::{Jour, Periode, PeriodeOuverte};
use socle::echec::R;
use socle::grandeurs::{Coefficient, Effectif};
use socle::mois::Mois;

use crate::natures::{DispositifDePolitiquePublique, MotifDeRecours, QuotiteDuContrat};

/// LE TEMPS DE TRAVAIL D'UN CONTRAT, tel que l'effectif le compte (R. 130-1, II, troisieme alinea) : la part d'une
/// personne qu'un jour de ce contrat compte. A temps plein, entierement ; a temps partiel au sens de L. 3123-1 du code
/// du travail, au rapport de ses horaires a la duree legale ou conventionnelle (dernier alinea de L. 1111-2 du code du
/// travail : la quotite du contrat, S21.G00.40.013, sur celle de reference, S21.G00.40.012), au plus une. La part ne se
/// fabrique que par `PLEIN` et `partiel`, qui refuse une reference nulle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempsDansLEffectif(Coefficient);

impl TempsDansLEffectif {
    /// Un contrat a temps plein : la personne compte entiere.
    pub const PLEIN: TempsDansLEffectif = TempsDansLEffectif(Coefficient::UN);

    /// Un contrat a temps partiel : `quotite` sur `reference`, au plus une. Refus : une reference nulle ne rapporte rien,
    /// et la personne ne se compte pas sans elle.
    pub fn partiel(quotite: QuotiteDuContrat, reference: QuotiteDuContrat) -> R<TempsDansLEffectif> {
        match quotite.rapport(reference)? {
            Some(r) => Ok(TempsDansLEffectif(r.min(Coefficient::UN))),
            None => Err(socle::refus!(
                "la quotité de référence du contrat à temps partiel est nulle : c'est la durée collective du travail de l'entreprise (S21.G00.40.012), sans laquelle son temps partiel ne se compte pas dans l'effectif"
            )),
        }
    }
}

/// UN CONTRAT TEL QUE L'EFFECTIF LE COMPTE : la personne qui en est titulaire (`qui` : son NIR ou son NTT dans une DSN,
/// sa fiche dans les paies reprises), ses dates (un contrat sans fin court jusqu'au dernier jour du mois), et la part
/// qu'un jour du contrat compte, aucune quand le contrat est exclu du decompte. Il ne se fabrique que par `de`, qui
/// ecarte ce qui ne compte pas avant de lire le temps de travail.
#[derive(Debug, Clone)]
pub struct ContratDansLEffectif<K> {
    qui: K,
    periode: PeriodeOuverte,
    poids: Option<Coefficient>,
}

/// LES DISPOSITIFS QUE L. 1111-3 DU CODE DU TRAVAIL EXCLUT DE L'EFFECTIF, auquel R. 130-1, III renvoie (1°, 2°, 4° et
/// 6° : `atelier/reference/sources/code-du-travail/legifrance-code-du-travail-L1111-3-20261118.pdf`), tels que la DSN
/// les declare (S21.G00.40.008, `norme-dsn/texte/bloc-S21.G00.40.txt`) : les apprentis (64, 65, 81), le contrat
/// initiative-emploi (21), le contrat d'accompagnement dans l'emploi (41, « Parcours Emploi Compétences / CUI - CAE »),
/// le contrat de professionnalisation (61). INFERENCE : le texte n'exclut le CIE et le CAE que « pendant la durée
/// d'attribution de l'aide financière », et le contrat de professionnalisation a duree indeterminee que jusqu'a la fin de
/// l'action de professionnalisation ; la DSN ne declare ni l'une ni l'autre, et un contrat qui porte le dispositif est
/// tenu exclu tout entier. Le contrat d'acces a l'emploi des DOM (42) n'est pas un de ceux que L. 1111-3 nomme : il compte.
fn exclu_par_l_1111_3(d: DispositifDePolitiquePublique) -> bool {
    use DispositifDePolitiquePublique as D;
    matches!(d, D::ApprentissageArtisanal | D::ApprentissageNonInscrit | D::ApprentissageSecteurPublic | D::CuiInitiativeEmploi | D::ParcoursEmploiCompetences | D::Professionnalisation)
}

impl<K> ContratDansLEffectif<K> {
    /// LE CONTRAT DE `qui`, du `periode`. Ne compte pas (R. 130-1, II, premier alinea, et III) : un CDD qui remplace un
    /// salarie absent (motif de recours 01 : la deuxieme phrase du 2° de L. 1111-2 du code du travail), un contrat d'un
    /// dispositif que L. 1111-3 exclut (`exclu_par_l_1111_3`) ; son temps de travail ne se lit pas (`temps` n'est pas
    /// appele). Un autre compte selon `temps`, dont le refus remonte.
    pub fn de(
        qui: K,
        periode: PeriodeOuverte,
        motif_recours: Option<MotifDeRecours>,
        dispositif: Option<DispositifDePolitiquePublique>,
        temps: impl FnOnce() -> R<TempsDansLEffectif>,
    ) -> R<ContratDansLEffectif<K>> {
        let exclu = motif_recours == Some(MotifDeRecours::Remplacement) || dispositif.is_some_and(exclu_par_l_1111_3);
        let poids = match exclu {
            true => None,
            false => Some(temps()?.0),
        };
        Ok(ContratDansLEffectif { qui, periode, poids })
    }
}

/// Chaque personne, la part qu'elle compte chaque jour de `periode` ou elle est employee, au plus fort de ses contrats
/// de ce jour ; et si un salarie, meme exclu du decompte, y est employe.
fn par_personne<K: Ord + Clone>(periode: &Periode, contrats: &[ContratDansLEffectif<K>]) -> (BTreeMap<K, BTreeMap<Jour, Coefficient>>, bool) {
    let mut par_jour: BTreeMap<K, BTreeMap<Jour, Coefficient>> = BTreeMap::new();
    let mut employe = false;
    for k in contrats {
        let Some(presence) = k.periode.croise(periode) else { continue };
        employe = true;
        let Some(poids) = k.poids else { continue };
        let jours = par_jour.entry(k.qui.clone()).or_default();
        for j in presence.jours() {
            let p = jours.entry(j).or_insert(Coefficient::ZERO);
            *p = (*p).max(poids);
        }
    }
    (par_jour, employe)
}

/// L'EFFECTIF D'UN MOIS (R. 130-1, II) : chaque personne titulaire d'un contrat de travail compte a due proportion des
/// jours du mois ou elle est employee (quatrieme alinea), selon son temps de travail (`TempsDansLEffectif`) ; ce qui
/// est exclu du decompte ne compte pas (`ContratDansLEffectif::de`). Une personne compte au plus une fois par jour, au
/// plus fort de ses contrats de ce jour. `None` : aucun salarie n'est employe ce mois-la, ni compte ni exclu du decompte
/// (R. 130-1, I : « les mois au cours desquels aucun salarié n'est employé » ; un mois ou seuls travaillent des salaries
/// exclus du decompte en a, et compte zero).
pub fn effectif_du_mois<K: Ord + Clone>(mois: Mois, contrats: &[ContratDansLEffectif<K>]) -> Option<Effectif> {
    let le_mois = Periode::du_mois(mois);
    let n = le_mois.jours().count();
    let (par_jour, employe) = par_personne(&le_mois, contrats);
    let une_personne = Effectif::personnes(1);
    employe.then(|| par_jour.values().map(|jours| (une_personne * jours.values().copied().sum::<Coefficient>()).reparti_sur(n)).sum())
}

/// L'EFFECTIF PRESENT LE JOUR `jour` : chaque personne, la part que son contrat de ce jour compte (`TempsDansLEffectif`),
/// au plus fort de ses contrats. C'est celui de l'annee de la premiere embauche, au dernier jour du mois de cette
/// embauche (L. 130-1, I, troisieme alinea : « l'effectif présent le dernier jour du mois au cours duquel a été
/// réalisée cette première embauche »). Les categories et le decompte sont ceux de R. 130-1, II (le quatrieme alinea,
/// le prorata des jours du mois, n'a pas d'objet pour un jour : INFERENCE).
pub fn effectif_au_jour<K: Ord + Clone>(jour: Jour, contrats: &[ContratDansLEffectif<K>]) -> Effectif {
    let (par_jour, _) = par_personne(&Periode::du_jour(jour), contrats);
    let une_personne = Effectif::personnes(1);
    par_jour.values().map(|jours| une_personne * jours.values().copied().sum::<Coefficient>()).sum()
}

/// L'EFFECTIF SALARIE ANNUEL : la moyenne des effectifs des mois, les mois sans salarie employe ecartes (`None`,
/// R. 130-1, I, premier alinea), tronquee au centieme (second alinea : « il n'est pas tenu compte de la fraction
/// d'effectif au-delà de la deuxième décimale ») : un effectif, pas un montant. `None` quand aucun mois n'a de salarie.
pub fn effectif_annuel(mois: impl IntoIterator<Item = Option<Effectif>>) -> Option<Effectif> {
    let mois: Vec<Effectif> = mois.into_iter().flatten().collect();
    if mois.is_empty() {
        return None;
    }
    let moyenne = mois.iter().copied().sum::<Effectif>().reparti_sur(mois.len());
    // Le milliardieme ajoute avant de tronquer : un quotient qui tombe juste sur un centieme (un tiers fois trois)
    // s'ecrit en decimal une unite du dernier chiffre en dessous, et ne doit pas perdre ce centieme.
    Some((moyenne + Effectif::UN_MILLIARDIEME).tronque_au_centieme())
}
