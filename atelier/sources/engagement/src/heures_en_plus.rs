//! LES HEURES EN PLUS D'UN SALARIE PAYE AU MOIS : ses heures supplementaires (temps plein) ou complementaires (temps
//! partiel) du mois, des faits de sa carte (`HEURES_SUPPLEMENTAIRES`, `HEURES_SUPPLEMENTAIRES_A_50`,
//! `HEURES_COMPLEMENTAIRES`), dont le moteur fait les lignes du bulletin (`lignes`, une par majoration :
//! `droit::heures_en_plus`) et ce qui les suit au calcul (`apres_les_cotisations`) : la reduction de cotisations
//! salariales (CSS L241-17, D241-21), la deduction forfaitaire patronale des heures supplementaires (L241-18, L241-18-1,
//! D241-24) et l'exoneration d'impot sur le revenu (CGI 81 quater) qui sort leur montant net de la remuneration nette
//! fiscale. Un contrat d'usage n'en a pas : ses heures sont ses lignes. Textes : `logiciel/donnees/permanents-base.json`,
//! `heures_en_plus`, qui les cite.

use droit::fiche::Lue;
use socle::forme::NombreLu;

use crate::calcul::{BaseDeCotisation, LigneBrut, LigneCotisation};
use crate::conges::{avertir, info, qui};
use crate::ligne::QuantiteDeLigne;
use crate::saisie::Carte;
use crate::Avis;
use droit::catalogue::CodeDeRubrique;
use droit::contexte::Contexte;
use droit::forme_bareme::{IdDeCotisation, CSG_CRDS_ND, CSG_DED, MUTUELLE, VIEIL_DEPLAF, VIEIL_PLAF};
use socle::texte::euros;
use socle::echec::{refus, R};
use socle::mois::Mois;
use socle::grandeurs::{Heures, Montant, MontantUnitaire, Taux};
use socle::num::{court, Decimal};

/// Les faits du mois, sur la carte d'un salarie paye au mois.
pub const HEURES_SUPPLEMENTAIRES: &str = "heures_supplementaires";
pub const HEURES_SUPPLEMENTAIRES_A_50: &str = "heures_supplementaires_a_50";
pub const HEURES_COMPLEMENTAIRES: &str = "heures_complementaires";

/// Les rubriques que le moteur compose (`rubriques-base.json`).
pub const HEURE_SUPPLEMENTAIRE: CodeDeRubrique = CodeDeRubrique::du_moteur("heure_supplementaire");
pub const HEURE_COMPLEMENTAIRE: CodeDeRubrique = CodeDeRubrique::du_moteur("heure_complementaire");

/// Les lignes du bulletin qui portent la reduction salariale et la deduction patronale.
pub const REDUCTION: IdDeCotisation = IdDeCotisation::du_moteur("reduction_heures_en_plus");
pub const DEDUCTION: IdDeCotisation = IdDeCotisation::du_moteur("deduction_heures_supplementaires");

/// LES FAITS QU'APPELLE UN CONTRAT PAYE AU MOIS, ceux que sa carte montre sous « Avancé » : un temps partiel ses heures
/// complementaires ; un temps plein ses heures supplementaires, et, quand la convention ne fixe pas leurs paliers (la
/// loi les decompte par semaine, C. trav. L3121-29 et L3121-36), celles qui passent la huitieme d'une semaine.
pub fn faits_appeles(ctx: &Contexte, partiel: bool) -> R<Vec<&'static str>> {
    if partiel {
        return Ok(vec![HEURES_COMPLEMENTAIRES]);
    }
    let idcc = ctx.entreprise.idcc.dite();
    Ok(if droit::heures_en_plus::paliers_conventionnels(idcc)? {
        vec![HEURES_SUPPLEMENTAIRES]
    } else {
        vec![HEURES_SUPPLEMENTAIRES, HEURES_SUPPLEMENTAIRES_A_50]
    })
}


/// LES LIGNES DES HEURES EN PLUS DU MOIS de la saisie `e` d'un contrat paye au mois de `salaire` pour `quotite` heures
/// par mois, a temps `partiel` ou non : une ligne par majoration, au taux horaire du contrat majore. INFERENCE, ecrite
/// comme telle : le taux horaire est le salaire mensuel du contrat sur ses heures par mois ; le BOSS (§ 310) y ajoute les
/// primes qui sont la contrepartie du travail, que la carte ne distingue pas des autres.
pub(crate) fn lignes(ctx: &Contexte, e: &Carte, mois: Mois, salaire: MontantUnitaire, quotite: Heures, partiel: bool) -> R<(Vec<crate::LignePaie>, Vec<Avis>)> {
    let (salaire, quotite) = (salaire.decimal(), quotite.decimal());
    let heures = |x: &Option<NombreLu>, nom: &str| crate::conges::heures_dites(ctx, e, x, nom);
    let (sup, a_50, comp) = (
        heures(&e.heures_supplementaires, "Heures supplémentaires")?,
        heures(&e.heures_supplementaires_a_50, "Heures supplémentaires à 50 %")?,
        heures(&e.heures_complementaires, "Heures complémentaires")?,
    );
    if sup.is_zero() && comp.is_zero() && a_50.is_zero() {
        return Ok((Vec::new(), Vec::new()));
    }
    if partiel && !(sup.is_zero() && a_50.is_zero()) {
        return Err(socle::refus!(
            "{} : son contrat est à temps partiel ({} h par mois) ; les heures au-delà de son contrat sont des heures \
             complémentaires (C. trav. L3123-20). Les dire dans « Heures complémentaires ». Rien n'est calculé.",
            qui(ctx, e),
            quotite.round_dp(2).normalize().to_string().replace('.', ",")
        ));
    }
    if !partiel && !comp.is_zero() {
        return Err(socle::refus!(
            "{} : son contrat est à temps plein ; les heures au-delà de la durée légale sont des heures supplémentaires (C. trav. \
             L3121-28). Les dire dans « Heures supplémentaires ». Rien n'est calculé.",
            qui(ctx, e)
        ));
    }
    if quotite.is_zero() {
        return Err(socle::refus!("{} : ses heures par mois sont nulles ; le taux horaire de ses heures en plus ne se calcule pas.", qui(ctx, e)));
    }
    let idcc = ctx.entreprise.idcc.dite();
    let (rubrique, reparties) = if partiel {
        (HEURE_COMPLEMENTAIRE, droit::heures_en_plus::complementaires(idcc, comp, quotite)?)
    } else {
        let salarie = crate::conges::salarie_de(e)?;
        let deja = if droit::heures_en_plus::paliers_conventionnels(idcc)? || droit::clauses::releve(idcc)?.is_some_and(|c| c.heures_supplementaires.is_some()) {
            crate::conges::heures_supplementaires_des_mois_anterieurs(ctx, mois, &salarie)?
        } else {
            Decimal::ZERO
        };
        (HEURE_SUPPLEMENTAIRE, droit::heures_en_plus::supplementaires(idcc, sup, a_50, deja)?)
    };
    let horaire = salaire / quotite;
    let mut out = Vec::new();
    for p in &reparties.paliers {
        let taux = (horaire * (Decimal::ONE_HUNDRED + p.majoration) / Decimal::ONE_HUNDRED).round_dp(4);
        out.push(crate::LignePaie::Composee(crate::ligne::Composee {
            rubrique: rubrique.clone(),
            quantite: socle::forme::Nombre(court(p.heures)),
            taux: socle::grandeurs::MontantUnitaire::du_decimal(court(taux)),
            heures: None,
            precision: Some(format!("majorées de {} %", p.majoration.normalize().to_string().replace('.', ","))),
            dates: Vec::new(),
        }));
    }
    let mut avis = vec![info(format!(
        "{} : {} heures {} au taux horaire de son contrat ({}, le salaire mensuel sur ses {} h), {}.",
        qui(ctx, e),
        reparties.paliers.iter().fold(Decimal::ZERO, |a, p| a + p.heures).normalize().to_string().replace('.', ","),
        if partiel { "complémentaires" } else { "supplémentaires" },
        euros(Montant::du_decimal(horaire)),
        quotite.round_dp(2).normalize().to_string().replace('.', ","),
        reparties
            .paliers
            .iter()
            .map(|p| format!("{} h majorées de {} %", p.heures.normalize().to_string().replace('.', ","), p.majoration.normalize().to_string().replace('.', ",")))
            .collect::<Vec<_>>()
            .join(", ")
    ))];
    avis.extend(reparties.avis.into_iter().map(avertir));
    Ok((out, avis))
}

/// Ce que les heures en plus d'une paie font apres ses cotisations.
pub(crate) struct Suites {
    pub lignes: Vec<LigneCotisation>,
    pub reduction: Montant,
    pub deduction: Montant,
    pub remuneration: Montant,
    pub heures: Heures,
    pub exonere: Montant,
    pub avis: Vec<String>,
}

/// LES SUITES DES HEURES EN PLUS d'une paie de `brut`, ses lignes de remuneration `lb` et ses cotisations `lignes` (la
/// reduction generale comprise), `tot_pat` ses cotisations patronales ; `None` sans heure en plus.
/// - LA REDUCTION DE COTISATIONS SALARIALES (L241-17, II ; D241-21 ; BOSS § 350-370) : la remuneration des heures,
///   majorations comprises, fois le taux moyen des cotisations salariales d'assurance vieillesse (la famille RETRAITE du
///   bulletin : vieillesse, Agirc-Arrco, CET ; pas l'Apec) sur le brut, dans la limite de 11,31 % ; imputee sur la
///   vieillesse salariale de base (L241-3) et dans sa limite (L241-17, II ; BOSS § 450-480).
/// - LA DEDUCTION PATRONALE (L241-18 et L241-18-1 ; D241-24) : par heure supplementaire (pas complementaire, BOSS § 570),
///   1,50 EUR sous vingt salaries, 0,50 EUR a partir de vingt (l'effectif de L130-1), dans la limite des cotisations
///   patronales du salarie.
/// - L'EXONERATION D'IMPOT (CGI 81 quater) : le montant net des heures sort de la remuneration nette fiscale et se declare
///   a part (fiche net-entreprises 2066, S21.G00.58.003 = 01). INFERENCE, ecrite comme telle : le net est la remuneration
///   des heures moins la CSG deductible qui la frappe (l'exemple de la fiche : 400 EUR, 373,28 EUR) et moins ce que ses
///   autres cotisations salariales, a leur taux moyen sur le brut, laissent apres la reduction ; la limite annuelle de
///   7 500 EUR se compte sur ce net, dans l'annee civile, chez l'employeur (`deja`).
pub(crate) fn apres_les_cotisations(
    ctx: &Contexte, bar: &droit::forme_bareme::Pile, lb: &[LigneBrut], lignes: &[LigneCotisation], brut: Montant, tot_pat: Montant, deja: impl FnOnce() -> R<Montant>,
) -> R<Option<Suites>> {
    let siennes: Vec<&LigneBrut> = lb.iter().filter(|l| l.code == HEURE_SUPPLEMENTAIRE || l.code == HEURE_COMPLEMENTAIRE).collect();
    let remuneration = siennes.iter().fold(Montant::ZERO, |a, l| a + l.montant);
    if remuneration <= Montant::ZERO || brut <= Montant::ZERO {
        return Ok(None);
    }
    // Leurs rubriques comptent des heures (`rubriques-base.json`) ; une autre grandeur se refuse.
    let heures_de = |l: &LigneBrut| match l.quantite {
        QuantiteDeLigne::Heures(h) => Ok(h),
        _ => Err(socle::refus!("La ligne {} des heures en plus ne compte pas des heures : le signaler au développeur.", l.code)),
    };
    let heures = siennes.iter().try_fold(Heures::ZERO, |a, l| heures_de(l).map(|h| a + h))?;
    let supplementaires = siennes.iter().filter(|l| l.code == HEURE_SUPPLEMENTAIRE).try_fold(Heures::ZERO, |a, l| heures_de(l).map(|h| a + h))?;
    let sal = |f: &dyn Fn(&LigneCotisation) -> bool| lignes.iter().filter(|l| f(l)).fold(Montant::ZERO, |a, l| a + l.montant_sal);
    let mut avis = Vec::new();

    // La reduction salariale.
    let vieillesse = sal(&|l| l.famille == droit::forme_bareme::FAMILLE_RETRAITE);
    let taux = Taux::du_decimal(vieillesse.decimal() / brut.decimal() * Decimal::ONE_HUNDRED)
        .min(Taux::du_decimal(droit::heures_en_plus::taux_maximum_de_reduction()?))
        .a_deux_decimales();
    let de_base = sal(&|l| l.est(&VIEIL_PLAF) || l.est(&VIEIL_DEPLAF));
    let reduction = remuneration.fois_taux(taux).au_centime().min(de_base).max(Montant::ZERO);
    let genre = if supplementaires.est_nul() { "complémentaires" } else { "supplémentaires" };
    let mut out = vec![LigneCotisation {
        id: REDUCTION,
        famille: droit::forme_bareme::FAMILLE_ALLEGEMENTS,
        libelle: format!("Réduction de cotisations salariales sur heures {}", genre),
        base: BaseDeCotisation::Montant(remuneration),
        taux_pat: None,
        montant_pat: Montant::ZERO,
        taux_sal: Some(socle::grandeurs::courte(taux)),
        montant_sal: -reduction,
        masquer_base_et_taux: false,
    }];

    // La deduction patronale, des heures supplementaires seules.
    let mut deduction = Montant::ZERO;
    if !supplementaires.est_nul() {
        let annee = ctx.lue_au.annee();
        let au_moins_20 = ctx.effectif()?.atteint(socle::grandeurs::Effectif::personnes(20), annee).map_err(|e| {
            e.reformuler(|p| format!("La déduction patronale des heures supplémentaires dépend de l'effectif (CSS L241-18, L241-18-1 : moins de vingt salariés ou au moins vingt). {}", p))
        })?;
        let par_heure = MontantUnitaire::du_decimal(droit::heures_en_plus::deduction_par_heure(au_moins_20)?);
        deduction = (supplementaires * par_heure).au_centime().min(tot_pat.max(Montant::ZERO));
        if !deduction.est_nul() {
            out.push(LigneCotisation {
                id: DEDUCTION,
                famille: droit::forme_bareme::FAMILLE_ALLEGEMENTS,
                libelle: "Déduction patronale sur heures supplémentaires".into(),
                // Sa base compte des heures : la liste des bases en heures le dit (`calcul::BASES_EN_HEURES`).
                base: BaseDeCotisation::Heures(supplementaires),
                taux_pat: None,
                montant_pat: -deduction,
                taux_sal: None,
                montant_sal: Montant::ZERO,
                masquer_base_et_taux: true,
            });
        }
    }

    // L'exoneration d'impot : le net des heures.
    let abattement = bar.abattement_csg;
    // La CSG deductible que la paie retient (une paie sans CSG n'en deduit rien) ; sa ligne porte son taux.
    let taux_csg = lignes
        .iter()
        .filter(|l| l.est(&CSG_DED))
        .take(1)
        .map(|l| l.taux_sal.ok_or_else(|| refus("la ligne de CSG déductible n'a pas de taux salarial")))
        .sum::<R<Taux>>()?;
    let csg_deductible = (remuneration * abattement).fois_taux(taux_csg).au_centime();
    let autres = sal(&|l| !(l.est(&CSG_DED) || l.est(&CSG_CRDS_ND) || l.est(&MUTUELLE)));
    let leurs_cotisations = Montant::du_decimal(autres.decimal() * remuneration.decimal() / brut.decimal()).au_centime();
    let net = (remuneration - csg_deductible - (leurs_cotisations - reduction).max(Montant::ZERO)).max(Montant::ZERO);
    let deja = deja()?;
    let limite = Montant::du_decimal(droit::heures_en_plus::limite_de_l_exoneration()?);
    let reste = (limite - deja).max(Montant::ZERO);
    let exonere = net.min(reste);
    if exonere < net {
        avis.push(format!(
            "Heures en plus : {} nets exonérés d'impôt cette année avant ce mois ; la limite de {} (CGI 81 quater) laisse {} \
             exonérés ce mois, le reste ({}) est imposable.",
            euros(deja),
            euros(limite),
            euros(exonere),
            euros(net - exonere)
        ));
    }
    Ok(Some(Suites { lignes: out, reduction, deduction, remuneration, heures, exonere, avis }))
}
