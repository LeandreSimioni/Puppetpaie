//! L'ALLOCATION JOURNALIERE D'UN INTERMITTENT, ESTIMEE (annexes VIII et X au reglement d'assurance chomage, art. 14 et
//! 16 ; `atelier/reference/sources/plafond-intermittents/unedic-annexe8-convention-20241115.txt`, `…annexe10…`) :
//! AJ = A + B + C, A du salaire de reference, B des heures travaillees, C de l'allocation journaliere minimale ; pas moins
//! que le plancher de l'annexe, pas plus que le plafond (34,4 % de 1/365e de quatre plafonds annuels de la Securite
//! sociale). Les valeurs et leurs textes sont dans `allocation-intermittents.json` ; ce module n'a que la formule.
//!
//! Une estimation : ni la readmission (art. 9), ni le salaire annuel de reference (art. 11 §2), ni la limite du salaire
//! de reference (art. 11 §3), ni la participation de 0,93 % (art. 19). Chaque partie s'arrondit au centime, puis
//! s'ajoute : INFERENCE depuis l'exemple 6 du guide de France Travail
//! (`chomage-intermittents/francetravail-guide-intermittents-20261020.txt` : « 39,80 + 12,20 + 12,78 = 64,78 € »).

use std::sync::{Arc, OnceLock};

use crate::emplois::AnnexeChomage;
use crate::source::forme_notee;
use socle::echec::R;
use socle::fichiers::{forme_en_cache, CheminRelatif, FormeGardee};
use socle::grandeurs::{Coefficient, Heures, Montant, Taux};

forme_notee! {
    /// `allocation-intermittents.json`, entier.
    pub struct Allocation {
        pub aj_minimale: AjMinimale,
        #[serde(rename = "VIII")]
        pub viii: Formule,
        #[serde(rename = "X")]
        pub x: Formule,
        pub plafond: PlafondDeLAllocation,
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

    /// Le plafond de l'allocation (art. 16), sous le plafond annuel de la Securite sociale `pss_annuel`, au centime.
    pub fn plafond(&self, pss_annuel: Montant) -> Montant {
        let p = &self.plafond;
        (pss_annuel * p.plafonds_de_la_securite_sociale).fois_taux(p.part).reparti_sur(p.jours as usize).au_centime()
    }

    /// LA PARTIE A : l'allocation minimale fois la part du salaire de reference `sr`, jusqu'au seuil puis au-dela, sur le
    /// diviseur.
    fn partie_a(&self, f: &Formule, sr: Montant) -> R<Montant> {
        let (bas, haut) = (sr.min(f.seuil_du_salaire), (sr - f.seuil_du_salaire).max(Montant::ZERO));
        let part = |m: Montant| m.rapport(f.diviseur_du_salaire).ok_or_else(|| socle::fichiers::absent("diviseur_du_salaire"));
        Ok((self.aj_minimale.montant * (part(bas)? * f.part_du_salaire + part(haut)? * f.part_du_salaire_au_dela)).au_centime())
    }

    /// LA PARTIE B : l'allocation minimale fois la part des heures travaillees `nht`, jusqu'au seuil puis au-dela, sur le
    /// diviseur.
    fn partie_b(&self, f: &Formule, nht: Heures) -> R<Montant> {
        let (bas, haut) = (nht.min(f.seuil_des_heures), (nht - f.seuil_des_heures).max(Heures::ZERO));
        let part = |h: Heures| h.rapport(f.diviseur_des_heures).ok_or_else(|| socle::fichiers::absent("diviseur_des_heures"));
        Ok((self.aj_minimale.montant * (part(bas)? * f.part_des_heures + part(haut)? * f.part_des_heures_au_dela)).au_centime())
    }

    /// L'ALLOCATION DE L'ANNEXE `annexe` pour le salaire de reference `sr` et les heures travaillees `nht`, sous le plafond
    /// `plafond` (`Allocation::plafond`).
    pub fn estimer(&self, annexe: AnnexeChomage, sr: Montant, nht: Heures, plafond: Montant) -> R<Estimation> {
        let f = self.formule(annexe);
        let (a, b) = (self.partie_a(f, sr)?, self.partie_b(f, nht)?);
        let c = (self.aj_minimale.montant * f.part_fixe).au_centime();
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
        let c = (self.aj_minimale.montant * f.part_fixe).au_centime();
        let a = visee - b - c;
        if a <= Montant::ZERO {
            return Ok(Visee::SansSalaire);
        }
        let aj = self.aj_minimale.montant;
        let du_seuil = self.partie_a(f, f.seuil_du_salaire)?;
        // La part du diviseur qu'une partie A demande, a la part `p` du salaire.
        let salaire = |reste: Montant, p: Coefficient| -> R<Montant> {
            let fois = reste.rapport(aj).and_then(|r| r.sur(p)).ok_or_else(|| socle::fichiers::absent("part_du_salaire"))?;
            Ok(f.diviseur_du_salaire * fois)
        };
        let sr = if a <= du_seuil { salaire(a, f.part_du_salaire)? } else { f.seuil_du_salaire + salaire(a - du_seuil, f.part_du_salaire_au_dela)? };
        Ok(Visee::Salaire(sr.au_centime()))
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
