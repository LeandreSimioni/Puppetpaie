//! LES HEURES EN PLUS D'UN SALARIE PAYE AU MOIS : les heures supplementaires d'un temps plein (C. trav. L3121-28 : toute
//! heure au-dela de la duree legale), les heures complementaires d'un temps partiel (L3123-20). Leur majoration se lit
//! dans la convention de la compagnie (`clauses-conventionnelles-base.json`, `heures_supplementaires`,
//! `heures_complementaires`), a defaut dans la loi (`permanents-base.json`, `heures_en_plus`) : L3121-33, I, 1° et
//! L3123-21 donnent la main a l'accord, L3121-36 et L3123-29 disent le taux « a defaut ». Ce qui suit les heures (la
//! reduction de cotisations salariales de L241-17, la deduction patronale de L241-18 et L241-18-1, l'exoneration
//! d'impot de 81 quater) se lit ici aussi ; l'engagement en fait les lignes et les montants (`engagement::heures_en_plus`).
//! Textes : `atelier/reference/sources/code-du-travail/articles-20261102.md`,
//! `code-de-la-securite-sociale/articles-20261102.md`, `code-general-des-impots/articles-20261102.md`,
//! `kali-1285-titre-vi-duree-du-travail-20261102.md`, `kali-3090-titre-viii-duree-du-travail-20261102.md`,
//! `boss/boss-heures-supplementaires-20260601.txt`.

use socle::echec::R;
use socle::grandeurs::{Heures, Montant, MontantUnitaire, Taux};
use socle::texte::virgule;

/// Des heures a une majoration.
#[derive(Debug, Clone, PartialEq)]
pub struct Palier {
    pub heures: Heures,
    pub majoration: Taux,
}

/// Les heures du mois reparties par majoration, et ce qu'il y a a en dire (un contingent, un plafond depasse).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Reparties {
    pub paliers: Vec<Palier>,
    pub avis: Vec<String>,
}

/// La loi, a defaut d'accord (`permanents-base.json`, `heures_en_plus`).
fn loi() -> R<crate::permanents::HeuresEnPlus> {
    Ok(crate::permanents::regles()?.heures_en_plus.clone())
}

/// Ce que la convention `idcc` dit des heures supplementaires, s'il est releve.
fn supplementaires_de_la_convention(idcc: Option<&crate::fiche::Idcc>) -> R<Option<crate::clauses::HeuresSupplementaires>> {
    Ok(crate::clauses::releve(idcc)?.and_then(|c| c.heures_supplementaires))
}

/// LA CONVENTION FIXE-T-ELLE LES PALIERS DES HEURES SUPPLEMENTAIRES ? Sinon la loi : 25 % pour les huit premieres
/// heures de la semaine, 50 % au-dela (L3121-36), et le mois ne dit pas lesquelles sont au-dela de la huitieme d'une
/// semaine (elles se decomptent par semaine, L3121-29) : la carte le demande (`heures_supplementaires_a_50`).
pub fn paliers_conventionnels(idcc: Option<&crate::fiche::Idcc>) -> R<bool> {
    Ok(supplementaires_de_la_convention(idcc)?.is_some_and(|c| c.paliers_annuels.is_some()))
}

/// LES HEURES SUPPLEMENTAIRES DU MOIS, par majoration. `heures` : celles du mois ; `a_50` : celles d'entre elles qui
/// passent la huitieme d'une semaine (la loi seule les lit) ; `deja` : celles des mois d'avant dans l'annee, que les
/// paliers annuels d'une convention et son contingent comptent (1285, VI.9 et VI.12).
pub fn supplementaires(idcc: Option<&crate::fiche::Idcc>, heures: Heures, a_50: Heures, deja: Heures) -> R<Reparties> {
    let mut r = Reparties::default();
    if heures <= Heures::ZERO {
        return Ok(r);
    }
    let conv = supplementaires_de_la_convention(idcc)?;
    match conv.as_ref().and_then(|c| c.paliers_annuels.as_ref().map(|p| (c, p))) {
        Some((c, paliers)) => {
            // Les paliers annuels de la convention : la n-ieme heure de l'annee prend la majoration de son palier ; au-dela
            // du dernier palier (le contingent), `au_dela_du_contingent`.
            let mut debut = Heures::ZERO;
            let mut reste = heures;
            let mut rang = deja;
            for p in paliers.iter() {
                let fin = debut + p.heures;
                let ici = (fin - rang).max(Heures::ZERO).min(reste);
                if ici > Heures::ZERO {
                    ajouter(&mut r.paliers, ici, p.majoration);
                    reste -= ici;
                    rang += ici;
                }
                debut = fin;
            }
            if reste > Heures::ZERO {
                let au_dela = c.au_dela_du_contingent.ok_or_else(|| socle::echec::livree(&crate::clauses::FICHIER, "heures_supplementaires.au_dela_du_contingent"))?;
                ajouter(&mut r.paliers, reste, au_dela);
            }
        }
        None => {
            if a_50 > heures {
                return Err(socle::refus!(
                    "{} heures supplémentaires au-delà de la huitième d'une semaine, pour {} heures supplémentaires dans le mois : \
                     les premières sont une part des secondes. Rien n'est calculé.",
                    virgule(a_50), virgule(heures)
                ));
            }
            let (premier, second) = loi()?.heures_supplementaires.paliers_hebdomadaires;
            ajouter(&mut r.paliers, heures - a_50, premier.majoration);
            ajouter(&mut r.paliers, a_50, second.majoration);
        }
    }
    if let Some(contingent) = conv.as_ref().and_then(|c| c.contingent_annuel).filter(|c| !c.est_nul()) {
        if deja + heures > contingent {
            r.avis.push(format!(
                "Heures supplémentaires de l'année : {} avec ce mois, au-delà du contingent annuel de {} heures de la convention. \
                 Les heures au-delà ouvrent droit à une contrepartie obligatoire en repos (C. trav. L3121-30), à poser hors de la paie.",
                virgule(deja + heures),
                virgule(contingent)
            ));
        }
    }
    Ok(r)
}

/// LES HEURES COMPLEMENTAIRES DU MOIS, par majoration, sur un contrat de `contrat` heures par mois. La loi (L3123-29) :
/// 10 % dans le dixieme des heures du contrat, 25 % jusqu'au tiers ; une convention qui ne fixe pas d'autre taux la
/// laisse (1285, VI.13 bis : 25 % au-dela du dixieme, rien dans le dixieme). Le plafond : le dixieme sans accord
/// (L3123-28), le tiers quand la convention le permet ; au-dela, un avis.
pub fn complementaires(idcc: Option<&crate::fiche::Idcc>, heures: Heures, contrat: Heures) -> R<Reparties> {
    let mut r = Reparties::default();
    if heures <= Heures::ZERO {
        return Ok(r);
    }
    let hc = loi()?.heures_complementaires;
    let mut debut = Heures::ZERO;
    let mut reste = heures;
    for p in hc.paliers.iter() {
        let fin = p.jusqu_au.de(contrat);
        let ici = (fin - debut).max(Heures::ZERO).min(reste);
        if ici > Heures::ZERO {
            ajouter(&mut r.paliers, ici, p.majoration);
            reste -= ici;
        }
        debut = fin;
    }
    if reste > Heures::ZERO {
        // Au-dela du dernier palier, sa majoration.
        ajouter(&mut r.paliers, reste, hc.paliers.dernier().majoration);
    }
    // Le plafond de la convention quand elle en fixe un, celui de la loi (L. 3123-28) sinon.
    let plafond = match crate::clauses::releve(idcc)?.and_then(|c| c.heures_complementaires) {
        Some(c) => c.plafond,
        None => hc.plafond_sans_accord,
    };
    let limite = plafond.de(contrat);
    if heures > limite {
        r.avis.push(format!(
            "{} heures complémentaires pour un contrat de {} heures par mois : au-delà du {} de la durée du contrat que la loi ou la \
             convention permettent (C. trav. L3123-20, L3123-28). Le logiciel les paie ; au-delà, la durée du contrat se revoit.",
            virgule(heures),
            virgule(contrat.a_deux_decimales()),
            plafond.en_mots()
        ));
    }
    Ok(r)
}

fn ajouter(v: &mut Vec<Palier>, heures: Heures, majoration: Taux) {
    if heures <= Heures::ZERO {
        return;
    }
    match v.iter_mut().find(|p| p.majoration == majoration) {
        Some(p) => p.heures += heures,
        None => v.push(Palier { heures, majoration }),
    }
}

/// Le taux maximum de la reduction de cotisations salariales (D241-21 : 11,31 %).
pub fn taux_maximum_de_reduction() -> R<Taux> {
    Ok(loi()?.reduction_salariale.taux_maximum)
}

/// La deduction patronale par heure supplementaire (D241-24) : 1,50 EUR sous vingt salaries, 0,50 EUR a partir de vingt.
pub fn deduction_par_heure(au_moins_20: bool) -> R<MontantUnitaire> {
    let d = loi()?.deduction_patronale;
    Ok(if au_moins_20 { d.au_moins_20_salaries } else { d.moins_de_20_salaries })
}

/// La limite annuelle de l'exoneration d'impot sur le revenu (CGI 81 quater : 7 500 EUR).
pub fn limite_de_l_exoneration() -> R<Montant> {
    Ok(loi()?.exoneration_impot.limite_annuelle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `n` heures.
    fn h(n: usize) -> Heures {
        Heures::UNE.fois(n)
    }

    #[test]
    fn la_loi_majore_de_25_les_huit_premieres_heures_d_une_semaine_et_de_50_les_suivantes() {
        // L3121-36, sans convention relevee (une compagnie sans IDCC).
        let r = supplementaires(None, h(12), h(3), Heures::ZERO).unwrap();
        assert_eq!(r.paliers, vec![Palier { heures: h(9), majoration: Taux::points(25) }, Palier { heures: h(3), majoration: Taux::points(50) }]);
    }

    #[test]
    fn la_1285_majore_les_80_premieres_heures_de_l_annee_de_25_puis_de_50() {
        // CCNEAC VI.12 : 75 heures deja faites dans l'annee, 10 ce mois : 5 a 25 %, 5 a 50 % ; la semaine ne compte pas.
        let r = supplementaires(Some(&crate::clauses::IDCC_CCNEAC), h(10), h(4), h(75)).unwrap();
        assert_eq!(r.paliers, vec![Palier { heures: h(5), majoration: Taux::points(25) }, Palier { heures: h(5), majoration: Taux::points(50) }]);
    }

    #[test]
    fn la_loi_majore_de_10_les_heures_complementaires_du_dixieme_et_de_25_les_suivantes() {
        // L3123-29 : un contrat de 100 heures par mois, 14 heures complementaires : 10 a 10 %, 4 a 25 %.
        let r = complementaires(None, h(14), h(100)).unwrap();
        assert_eq!(r.paliers, vec![Palier { heures: h(10), majoration: Taux::points(10) }, Palier { heures: h(4), majoration: Taux::points(25) }]);
    }
}
