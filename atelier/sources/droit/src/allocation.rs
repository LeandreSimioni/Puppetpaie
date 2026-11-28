//! LES DROITS D'UN INTERMITTENT, SANS COMPAGNIE (annexes VIII et X au reglement d'assurance chomage ;
//! `atelier/reference/sources/plafond-intermittents/unedic-annexe8-convention-20241115.txt`, `…annexe10…`) :
//!
//! - L'AFFILIATION (art. 3, art. 9 §1 a et b) : 507 heures sur les 365 jours qui precedent la fin de contrat ; a la
//!   readmission, 42 heures de plus par periode de 30 jours au-dela du 365e jour, jamais avant la fin de contrat qui a
//!   ouvert le droit precedent (`affiliation`) ; et le jour ou, sans contrat de plus, une ligne sort des 365 jours et il
//!   en faudra davantage (`butoir`).
//! - L'ALLOCATION JOURNALIERE, ESTIMEE (art. 14 et 16) : AJ = A + B + C, A du salaire de reference, B des heures
//!   travaillees, C de l'allocation journaliere minimale ; pas moins que le plancher de l'annexe, pas plus que le plafond ;
//!   sur les 365 jours, avec les diviseurs de l'annexe.
//! - LA LIMITE DU SALAIRE DE REFERENCE (art. 11 §3) : chaque paie n'y compte qu'a quatre fois son plafond
//!   (`salaire_limite`, `facteur_vise`).
//!
//! Les valeurs et leurs textes sont dans `allocation-intermittents.json` ; ce module n'a que les formules, et les formes
//! de ce que l'annee d'indemnisation lit (art. 19 a 30 : `droit::annee_de_chomage`). Ni le salaire annuel de reference
//! (art. 11 §2), ni la clause de rattrapage. Chaque partie
//! se tronque au centime, puis s'ajoute : c'est ce qu'ecrit l'exemple 6 du guide de France Travail
//! (`chomage-intermittents/francetravail-guide-intermittents-20261020.txt` : un technicien, 800 heures, 18 000 € ; la
//! partie A, 31,96 x 6 228 / 5 000 = 39,809…, s'y ecrit 39,80 € ; « 39,80 + 12,20 + 12,78 = 64,78 € ») ; aucun article
//! des annexes ne dit l'arrondi.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use crate::contexte::IdSalarie;
use crate::cumul::{derniere_fin, fenetre_allongee, fenetre_douze_mois, position, Constantes, Declare, Position};
use crate::emplois::AnnexeChomage;
use crate::source::forme_notee;
use socle::date::{Duree, Jour};
use socle::echec::R;
use socle::fichiers::{forme_en_cache, CheminRelatif, FormeGardee};
use socle::grandeurs::{Coefficient, Heures, HeuresParUnite, Jours, Montant, Taux};

forme_notee! {
    /// `allocation-intermittents.json`, entier.
    pub struct Allocation {
        pub aj_minimale: AjMinimale,
        #[serde(rename = "VIII")]
        pub viii: Formule,
        #[serde(rename = "X")]
        pub x: Formule,
        pub readmission: Readmission,
        pub limite_du_salaire: LimiteDuSalaire,
        pub plafond: PlafondDeLAllocation,
        pub activite_reprise: ActivitesReprises,
        pub participation: Participation,
        pub delai_d_attente: DelaiDAttente,
        pub franchise_conges: FranchiseDeConges,
        pub franchise_salaires: FranchiseDeSalaires,
        pub contributions: ContributionsSurLAllocation,
    }
}

forme_notee! {
    /// Un mois travaille pendant l'indemnisation (art. 30) : le plafond du cumul du salaire et de l'allocation (en
    /// plafonds mensuels de la Securite sociale), et ce que dit chaque annexe. Les heures d'un cachet sont celles du
    /// catalogue des rubriques (`droit::cumul::Constantes::heures_par_cachet`).
    pub struct ActivitesReprises {
        pub plafond_du_cumul: Coefficient,
        #[serde(rename = "VIII")]
        pub viii: ActiviteReprise,
        #[serde(rename = "X")]
        pub x: ActiviteReprise,
    }
}

forme_notee! {
    /// L'activite reprise sous une annexe (art. 30, et art. 19 pour les heures d'un jour) : les heures qui font un jour de
    /// travail, les jours indemnisables qu'un jour de travail retire, et les jours de travail a partir desquels rien n'est
    /// verse dans le mois.
    pub struct ActiviteReprise {
        pub heures_par_jour: HeuresParUnite,
        pub coefficient: Coefficient,
        pub jours_sans_allocation: Jours,
    }
}

forme_notee! {
    /// La participation au financement des retraites complementaires (art. 19), en points du salaire journalier moyen.
    pub struct Participation {
        pub taux: Taux,
    }
}

forme_notee! {
    /// Le delai d'attente (art. 22).
    pub struct DelaiDAttente {
        pub jours: Jours,
    }
}

forme_notee! {
    /// La franchise de conges payes (art. 21 §1 a, art. 23 §2) : `jours_de_conges` par `par_jours_travailles`, au plus
    /// `plafond` ; `forfait` jours par mois jusqu'a `seuil_du_forfait` jours de franchise, `forfait_au_dela` au-dessus.
    pub struct FranchiseDeConges {
        pub jours_de_conges: Jours,
        pub par_jours_travailles: Jours,
        pub plafond: Jours,
        pub forfait: Jours,
        pub forfait_au_dela: Jours,
        pub seuil_du_forfait: Jours,
    }
}

forme_notee! {
    /// La franchise de salaires (art. 21 §1 b, art. 23 §2) : les jours deduits, le SMIC mensuel et journalier sur
    /// `heures_par_semaine` (mensuel : x `semaines_par_an` / `mois_par_an` ; journalier : / `jours_par_semaine`), les
    /// SMIC journaliers du diviseur, et les premiers mois sur lesquels elle se repartit.
    pub struct FranchiseDeSalaires {
        pub jours_deduits: Jours,
        pub heures_par_semaine: Heures,
        pub semaines_par_an: u32,
        pub mois_par_an: u32,
        pub jours_par_semaine: u32,
        pub smic_journaliers: Coefficient,
        pub mois_de_repartition: u32,
    }
}

forme_notee! {
    /// La CSG et la CRDS sur l'allocation (circulaire Unedic 2026-06, fiche 2, § 7) : l'abattement de l'assiette, les
    /// taux, en points, et l'allocation brute sous laquelle elles ne la font pas passer.
    pub struct ContributionsSurLAllocation {
        pub abattement: Taux,
        pub csg: Taux,
        pub csg_reduite: Taux,
        pub crds: Taux,
        pub plancher: Montant,
    }
}

impl ContributionsSurLAllocation {
    /// Si la CSG et la CRDS peuvent prendre quelque chose a l'allocation brute `brute` d'un jour : seulement quand elle
    /// passe le plancher (circulaire Unedic 2026-06, fiche 2, § 7, exemple 20).
    pub fn jouent(&self, brute: Montant) -> bool {
        brute > self.plancher
    }
}

forme_notee! {
    /// L'allocation journaliere minimale des annexes (art. 14).
    pub struct AjMinimale {
        pub montant: Montant,
    }
}

forme_notee! {
    /// La formule d'une annexe (art. 14) : ses seuils, ses parts, ses diviseurs, et son plancher.
    pub struct Formule {
        pub seuil_du_salaire: Montant,
        pub part_du_salaire: Coefficient,
        pub part_du_salaire_au_dela: Coefficient,
        pub diviseur_du_salaire: Montant,
        pub seuil_des_heures: Heures,
        pub part_des_heures: Coefficient,
        pub part_des_heures_au_dela: Coefficient,
        pub diviseur_des_heures: Heures,
        pub part_fixe: Coefficient,
        pub plancher: Montant,
    }
}

forme_notee! {
    /// La readmission (art. 9 §1 b) : les heures de plus par periode de jours au-dela de la fenetre.
    pub struct Readmission {
        pub heures_par_periode: Heures,
        pub jours_par_periode: u32,
    }
}

forme_notee! {
    /// La limite du salaire de reference (art. 11 §3, art. 49) : les plafonds d'une paie qu'il compte au plus.
    pub struct LimiteDuSalaire {
        pub plafonds_par_paie: Coefficient,
    }
}

forme_notee! {
    /// Le plafond de l'allocation (art. 16) : `part` de 1/`jours` de `plafonds_de_la_securite_sociale` plafonds annuels.
    pub struct PlafondDeLAllocation {
        pub part: Taux,
        pub plafonds_de_la_securite_sociale: Coefficient,
        pub jours: u32,
    }
}

/// LA LECTURE DE `allocation-intermittents.json`, la seule, une fois tant que le fichier ne change pas.
pub fn allocation() -> R<Arc<Allocation>> {
    static CACHE: OnceLock<FormeGardee<Allocation>> = OnceLock::new();
    forme_en_cache(&CACHE, &const { CheminRelatif::du_moteur("allocation-intermittents.json") }, "La table livrée de l'allocation des intermittents ne se lit pas : le signaler au développeur.", Ok)
}

impl Allocation {
    /// La formule de l'annexe `annexe`.
    pub fn formule(&self, annexe: AnnexeChomage) -> &Formule {
        match annexe {
            AnnexeChomage::Viii => &self.viii,
            AnnexeChomage::X => &self.x,
        }
    }

    /// L'activite reprise sous l'annexe `annexe` (art. 30).
    pub fn activite(&self, annexe: AnnexeChomage) -> &ActiviteReprise {
        match annexe {
            AnnexeChomage::Viii => &self.activite_reprise.viii,
            AnnexeChomage::X => &self.activite_reprise.x,
        }
    }

    /// Le plafond de l'allocation (art. 16), sous le plafond annuel de la Securite sociale `pss_annuel`, au centime.
    pub fn plafond(&self, pss_annuel: Montant) -> Montant {
        let p = &self.plafond;
        (pss_annuel * p.plafonds_de_la_securite_sociale).fois_taux(p.part).reparti_sur(p.jours as usize).au_centime()
    }

    /// LA PARTIE A : l'allocation minimale fois la part du salaire de reference `sr`, jusqu'au seuil puis au-dela, sur le
    /// diviseur de l'annexe.
    fn partie_a(&self, f: &Formule, sr: Montant) -> R<Montant> {
        let diviseur = f.diviseur_du_salaire;
        let (bas, haut) = (sr.min(f.seuil_du_salaire), (sr - f.seuil_du_salaire).max(Montant::ZERO));
        let part = |m: Montant| m.rapport(diviseur).ok_or_else(|| socle::fichiers::absent("diviseur_du_salaire"));
        Ok((self.aj_minimale.montant * (part(bas)? * f.part_du_salaire + part(haut)? * f.part_du_salaire_au_dela)).au_centime_vers_zero())
    }

    /// LA PARTIE B : l'allocation minimale fois la part des heures travaillees `nht`, jusqu'au seuil puis au-dela, sur le
    /// diviseur de l'annexe.
    fn partie_b(&self, f: &Formule, nht: Heures) -> R<Montant> {
        let diviseur = f.diviseur_des_heures;
        let (bas, haut) = (nht.min(f.seuil_des_heures), (nht - f.seuil_des_heures).max(Heures::ZERO));
        let part = |h: Heures| h.rapport(diviseur).ok_or_else(|| socle::fichiers::absent("diviseur_des_heures"));
        Ok((self.aj_minimale.montant * (part(bas)? * f.part_des_heures + part(haut)? * f.part_des_heures_au_dela)).au_centime_vers_zero())
    }

    /// LA PARTIE C : l'allocation minimale fois la part fixe.
    fn partie_c(&self, f: &Formule) -> Montant {
        (self.aj_minimale.montant * f.part_fixe).au_centime_vers_zero()
    }

    /// L'ALLOCATION DE L'ANNEXE `annexe` pour le salaire de reference `sr` et les heures travaillees `nht`, sous le plafond
    /// `plafond` (`Allocation::plafond`).
    pub fn estimer(&self, annexe: AnnexeChomage, sr: Montant, nht: Heures, plafond: Montant) -> R<Estimation> {
        let f = self.formule(annexe);
        let (a, b) = (self.partie_a(f, sr)?, self.partie_b(f, nht)?);
        let c = self.partie_c(f);
        let calculee = a + b + c;
        let (retenue, borne) = if calculee < f.plancher {
            (f.plancher, Some(Borne::Plancher))
        } else if calculee > plafond {
            (plafond, Some(Borne::Plafond))
        } else {
            (calculee, None)
        };
        Ok(Estimation { annexe, a, b, c, calculee, plancher: f.plancher, plafond, retenue, borne })
    }

    /// LE SALAIRE DE REFERENCE QU'UNE ALLOCATION VISEE `visee` DEMANDE, sous l'annexe `annexe`, les heures `nht` restant
    /// ce qu'elles sont : la partie A qu'il faut (`visee` moins B et C), ramenee a son salaire, la formule lue a l'envers.
    pub fn salaire_vise(&self, annexe: AnnexeChomage, visee: Montant, nht: Heures, plafond: Montant) -> R<Visee> {
        let f = self.formule(annexe);
        if visee > plafond {
            return Ok(Visee::AuDelaDuPlafond);
        }
        if visee <= f.plancher {
            return Ok(Visee::DesLePlancher);
        }
        let b = self.partie_b(f, nht)?;
        let c = self.partie_c(f);
        let a = visee - b - c;
        if a <= Montant::ZERO {
            return Ok(Visee::SansSalaire);
        }
        let aj = self.aj_minimale.montant;
        let du_seuil = self.partie_a(f, f.seuil_du_salaire)?;
        let diviseur = f.diviseur_du_salaire;
        // Le salaire qu'une partie A demande, a la part `p` du salaire.
        let salaire = |reste: Montant, p: Coefficient| -> R<Montant> {
            let fois = reste.rapport(aj).and_then(|r| r.sur(p)).ok_or_else(|| socle::fichiers::absent("part_du_salaire"))?;
            Ok(diviseur * fois)
        };
        let sr = if a <= du_seuil { salaire(a, f.part_du_salaire)? } else { f.seuil_du_salaire + salaire(a - du_seuil, f.part_du_salaire_au_dela)? };
        Ok(Visee::Salaire(sr.au_centime()))
    }

    /// LA LIMITE D'UNE PAIE dans le salaire de reference (art. 11 §3) : quatre fois son plafond.
    pub fn limite_de_la_paie(&self, plafond_de_la_paie: Montant) -> Montant {
        (plafond_de_la_paie * self.limite_du_salaire.plafonds_par_paie).au_centime()
    }
}

/// CE QU'UNE ANNEXE DONNE : ses trois parties, leur somme, et l'allocation retenue entre son plancher et le plafond.
#[derive(Debug, Clone, PartialEq)]
pub struct Estimation {
    pub annexe: AnnexeChomage,
    pub a: Montant,
    pub b: Montant,
    pub c: Montant,
    /// A + B + C.
    pub calculee: Montant,
    pub plancher: Montant,
    pub plafond: Montant,
    /// La calculee, ramenee au plancher ou au plafond.
    pub retenue: Montant,
    /// Ce qui l'a ramenee, s'il y a lieu.
    pub borne: Option<Borne>,
}

/// La borne qui fixe l'allocation retenue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Borne {
    Plancher,
    Plafond,
}

/// CE QU'UNE ALLOCATION VISEE DEMANDE : un salaire de reference ; ou rien, le plancher la donne deja, ou les parties B et
/// C a elles seules ; ou elle passe le plafond, qu'aucun salaire n'atteint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visee {
    Salaire(Montant),
    DesLePlancher,
    SansSalaire,
    AuDelaDuPlafond,
}

// ------------------------------------------------------------------ la limite du salaire de reference

/// LE SALAIRE DE REFERENCE QUE COMPTENT LES PAIES `paies` (chacune son brut, et sa limite, `Allocation::limite_de_la_paie` ;
/// aucune : son plafond ne se connait pas, et elle compte entiere, l'appelant disant ce qu'il suppose), chacune augmentee
/// du facteur `k` : la somme, chaque paie sous sa limite (art. 11 §3).
pub fn salaire_limite(paies: &[(Montant, Option<Montant>)], k: Coefficient) -> Montant {
    paies
        .iter()
        .map(|(brut, limite)| {
            let b = *brut * k;
            match limite {
                Some(l) => b.min(*l),
                None => b,
            }
        })
        .fold(Montant::ZERO, |a, b| a + b)
}

/// LE PLUS HAUT SALAIRE DE REFERENCE QUE LES PAIES PEUVENT COMPTER : la somme de leurs limites ; aucun quand une paie qui a
/// un brut n'a pas de limite connue (elle monte sans fin).
pub fn salaire_au_plus(paies: &[(Montant, Option<Montant>)]) -> Option<Montant> {
    let mut total = Montant::ZERO;
    for (_, limite) in paies.iter().filter(|(b, _)| *b > Montant::ZERO) {
        match limite {
            Some(l) => total += *l,
            None => return None,
        }
    }
    Some(total)
}

/// LE FACTEUR qui porte le salaire de reference des paies `paies` a `cible`, chaque paie sous sa limite
/// (`salaire_limite`) ; aucun quand les limites l'arretent avant (`salaire_au_plus`), ou qu'aucune paie n'a de brut. La
/// somme croit par morceaux droits : chaque paie monte jusqu'a sa limite, puis s'arrete ; on cherche le morceau ou elle
/// passe la cible.
pub fn facteur_vise(paies: &[(Montant, Option<Montant>)], cible: Montant) -> Option<Coefficient> {
    if salaire_au_plus(paies).is_some_and(|m| m < cible) {
        return None;
    }
    // Le facteur ou chaque paie touche sa limite (son coude), du plus bas au plus haut ; apres le dernier, plus rien
    // n'arrete les paies sans limite.
    let coude = |b: &Montant, l: &Option<Montant>| l.and_then(|l| l.rapport(*b));
    let mut coudes: Vec<Coefficient> = paies.iter().filter_map(|(b, l)| coude(b, l)).collect();
    coudes.sort();
    let mut bas = Coefficient::ZERO;
    for haut in coudes.into_iter().map(Some).chain(std::iter::once(None)) {
        // Sur le morceau de `bas` a `haut`, une paie dont le coude est passe compte sa limite ; les autres, leur brut fois
        // le facteur.
        let (mut fixe, mut pente) = (Montant::ZERO, Montant::ZERO);
        for (b, l) in paies {
            match (coude(b, l), l) {
                (Some(t), Some(limite)) if t <= bas => fixe += *limite,
                _ => pente += *b,
            }
        }
        if let Some(k) = (cible - fixe).rapport(pente) {
            if k >= bas && haut.is_none_or(|h| k <= h) {
                return Some(k);
            }
        }
        if let Some(h) = haut {
            bas = h;
        }
    }
    None
}

// ------------------------------------------------------------------ l'affiliation

/// L'AFFILIATION D'UNE PERSONNE A UNE FIN DE CONTRAT : sa position dans la fenetre retenue (365 jours, ou allongee), les
/// periodes de 30 jours de plus, les heures exigees, et ce qui manque.
#[derive(Debug, Clone)]
pub struct Affiliation {
    pub position: Position,
    pub periodes_en_plus: u32,
    pub heures_exigees: Heures,
    pub manque: Heures,
    /// A la readmission, la fin de contrat qui a ouvert le droit precedent ; aucune : une premiere ouverture.
    pub precedente: Option<Jour>,
}

/// L'AFFILIATION DES DROITS D'UN INTERMITTENT, le seul compte (la route `droits` d'une compagnie, ses conseils, la colonne
/// « Droits » du bulletin, et `droits-des-pieces`) : `affiliation` a la derniere fin des lignes `lignes` de `salarie`
/// (art. 3 : « au cours des douze mois qui précèdent la fin du contrat de travail »), celles qui finissent au plus tard
/// `jusqu_au` quand il est dit ; aucune ligne, a `jusqu_au`, sinon aujourd'hui. Une date anniversaire `anniversaire`
/// (notifiee par France Travail) dit la readmission : la fin de contrat qui a ouvert le droit precedent est douze mois
/// avant (art. 9 §1 c : « la date anniversaire correspondant au terme des douze mois suivant la fin du contrat de travail
/// retenue pour l'ouverture » ; INFERENCE, dite dans `allocation-intermittents.json`).
#[allow(clippy::too_many_arguments)]
pub fn affiliation_des_droits(racine: &Path, c: &Constantes, r: &Readmission, lignes: &[Declare], salarie: &IdSalarie, anniversaire: Option<Jour>, jusqu_au: Option<Jour>, naissance: Option<Jour>) -> R<Affiliation> {
    let siennes: Vec<Declare> = lignes.iter().filter(|l| l.salarie == *salarie && jusqu_au.is_none_or(|j| l.fin <= j)).cloned().collect();
    let fin = match (derniere_fin(&siennes, salarie), jusqu_au) {
        (Some(j), _) | (None, Some(j)) => j,
        (None, None) => crate::contexte::aujourdhui()?,
    };
    let precedente = match anniversaire {
        Some(a) => Some(a.avant(Duree::mois(12))?),
        None => None,
    };
    affiliation(racine, c, r, &siennes, salarie, fin, precedente, naissance)
}

/// L'AFFILIATION A LA FIN DE CONTRAT `fin` (art. 3 ; art. 9 §1 a et b) : les lignes `lignes` de `salarie` dont la fin
/// est dans les 365 jours qui la precedent, 507 heures exigees ; a la readmission (`precedente` : la fin de contrat qui a
/// ouvert le droit precedent), seules comptent les lignes qui finissent apres elle, et, faute des 507 heures, la fenetre
/// s'allonge de 30 jours en 30 jours (42 heures de plus chaque fois) tant qu'elle ne remonte pas a `precedente`. Rendue :
/// la fenetre ou il manque le moins, la plus courte a egalite.
#[allow(clippy::too_many_arguments)]
pub fn affiliation(racine: &Path, c: &Constantes, r: &Readmission, lignes: &[Declare], salarie: &IdSalarie, fin: Jour, precedente: Option<Jour>, naissance: Option<Jour>) -> R<Affiliation> {
    let siennes: Vec<Declare> = match precedente {
        Some(p) => lignes.iter().filter(|l| l.fin > p).cloned().collect(),
        None => lignes.to_vec(),
    };
    let mut meilleure: Option<Affiliation> = None;
    let mut k = 0u32;
    loop {
        let fenetre = match k {
            0 => fenetre_douze_mois(c, fin)?,
            _ => fenetre_allongee(c, fin, Duree::jours(r.jours_par_periode).fois(k))?,
        };
        if k > 0 && !precedente.is_some_and(|p| fenetre.0.debut() > p) {
            break;
        }
        let exigees = c.seuil_heures + r.heures_par_periode.fois(k as usize);
        let p = position(racine, c, &siennes, salarie, &fenetre, naissance);
        let manque = (exigees - p.heures_ouverture()?).max(Heures::ZERO);
        let atteinte = manque.est_nul();
        if meilleure.as_ref().is_none_or(|m| manque < m.manque) {
            meilleure = Some(Affiliation { position: p, periodes_en_plus: k, heures_exigees: exigees, manque, precedente });
        }
        if atteinte {
            break;
        }
        k += 1;
    }
    meilleure.ok_or_else(|| socle::fichiers::absent("la fenêtre des 365 jours"))
}

/// LE JOUR BUTOIR : le dernier jour ou, sans contrat de plus, l'affiliation a une fin de contrat demande ce qu'elle
/// demande a `fin` ; le lendemain, une ligne sort des 365 jours (art. 3) et il en faut davantage. Ce n'est pas une
/// echeance de France Travail. Rendu : ce jour, et l'affiliation du lendemain ; aucun quand il ne manque jamais davantage
/// (rien ne se compte : il manque deja toutes les heures).
#[derive(Debug, Clone)]
pub struct Butoir {
    pub jour: Jour,
    pub lendemain: Affiliation,
}

/// Le jour butoir apres la fin de `a_la_fin` (voir `Butoir`), sur les 365 jours, sans readmission ni allongement (la
/// page des droits compte sur un an, `droits-des-pieces`) : les jours ou une ligne sort de la fenetre, du plus proche au
/// plus lointain, jusqu'au premier ou il manque davantage qu'a cette fin. Une ligne qui finit le jour L est dans la
/// fenetre jusqu'a L + 364 (`cumul::fenetre_douze_mois`).
pub fn butoir(racine: &Path, c: &Constantes, r: &Readmission, lignes: &[Declare], salarie: &IdSalarie, naissance: Option<Jour>, a_la_fin: &Affiliation) -> R<Option<Butoir>> {
    let fin = a_la_fin.position.periode.fin();
    let mut jours: Vec<Jour> = Vec::new();
    for l in lignes {
        jours.push(l.fin.apres(c.fenetre_jours)?);
    }
    jours.retain(|j| *j > fin);
    jours.sort();
    jours.dedup();
    for j in jours {
        let a = affiliation(racine, c, r, lignes, salarie, j, None, naissance)?;
        if a.manque > a_la_fin.manque {
            return Ok(Some(Butoir { jour: j.avant(Duree::jours(1))?, lendemain: a }));
        }
    }
    Ok(None)
}
