//! Le « Plafond Urssaf » du bas de bulletin : le plafond de securite sociale d'une paie (L. 241-3 CSS), celui sur
//! lequel s'assoient la vieillesse plafonnee, le FNAL et le chomage d'un intermittent (`calcul::calculer`, 7.6).
//!
//! ```text
//! artiste, periode d'engagement continu de moins de cinq jours (du debut a la fin du contrat, jours calendaires) :
//!     plafond = PSS horaire x 12 x journees de travail accomplies dans la paie (dates distinctes)
//! autre intermittent (artiste engage cinq jours ou plus, technicien) :
//!     plafond = au centime( somme, pour chaque jour ayant donne lieu a remuneration dans la paie (dates distinctes),
//!                           de PSS mensuel de son annee / jours de son mois )
//! permanent (mensualise) :
//!     plafond = au centime( PSS mensuel x jours de la periode de paie couverts par le contrat / jours du mois )
//! ```
//!
//! Textes (`atelier/reference/sources/`) :
//! - arrete du 24/01/1975, art. 3, al. 2 (`arrete-1975-artistes-du-spectacle/articles-20261010.md`) : « Pour les
//!   periodes d'engagement continu inferieures a cinq jours, le plafond [...] applicable a chaque journee de travail
//!   accomplie par un artiste du spectacle pour un meme employeur est egal a douze fois le plafond horaire, quels que
//!   soient le nombre d'heures et la nature du travail effectues dans ladite journee. » Une journee, pas un cachet ;
//!   « est egal », pas un plancher ; la duree de l'engagement, pas une suite de jours travailles. L'arrete ne vise que
//!   les artistes (art. 1 : 15° de L. 311-3 CSS) ;
//! - BOSS, assiette generale, version du 01/08/2026, § 1090 (`boss/boss-assiette-generale-20260801.txt`,
//!   l. 1472-1483) : les artistes du spectacle « ne sont pas eligibles a la reduction du plafond » ; « Toutefois, il est
//!   admis que : pour les salaries intermittents, le plafond soit proratise pour ne tenir compte que des jours ayant
//!   donne lieu a remuneration ». Tolerance appliquee (le BOSS est opposable) a tout
//!   intermittent hors de l'arrete de 1975 ; le prorata se fait comme au § 700 (« valeur mens. du plafond x (nombre de
//!   jours de la periode d'emploi / nombre de jours calendaires du mois) »), les jours de la periode d'emploi etant
//!   ici les jours remuneres de la paie. Un jour remunere se lit sur les lignes datees ; il ne se devine pas ;
//! - CSS R. 242-2, I, al. 2 et 3 (`code-de-la-securite-sociale/articles-20261010.md`) : le plafond « retenu pour
//!   chaque paie », « reduit[s] a due proportion du nombre de jours de la periode pendant laquelle les personnes sont
//!   employees » : la regle d'un permanent, sur la periode de sa paie ;
//! - CSS D. 242-19 : la valeur horaire (dans le bareme, `plafond_securite_sociale`).
//!
//! Les plafonds (PSS) ne sont pas dans le code : le module recoit un resolveur `annee -> Pss`
//! (`Contexte::plafond_securite_sociale` en production, qui les lit en euros entiers). Un refus du resolveur passe tel
//! quel.

use socle::date::{Duree, EnJours, Jour, Periode};
use socle::echec::R;
use socle::grandeurs::ecrit::texte::dit;
use socle::grandeurs::{nulle, Coefficient, Heures, Jours, Montant, MontantUnitaire};
use std::collections::BTreeSet;

/// LE PLAFOND DE LA SECURITE SOCIALE D'UNE ANNEE (CSS L. 241-3, D. 242-17 a D. 242-19) : sa valeur mensuelle, et sa
/// valeur horaire, des euros par heure, en euros entiers (`en_euros_entiers`, le seul chemin qui en fait un).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pss {
    mensuel: Montant,
    horaire: MontantUnitaire,
}

impl Pss {
    /// Le plafond que dit un texte (le bareme, la table des parametres du BOSS), ses deux valeurs tronquees a l'euro.
    pub fn en_euros_entiers(mensuel: Montant, horaire: MontantUnitaire) -> Pss {
        Pss { mensuel: mensuel.partie_entiere(), horaire: horaire.partie_entiere() }
    }

    /// Sa valeur mensuelle.
    pub fn mensuel(self) -> Montant {
        self.mensuel
    }

    /// Sa valeur horaire.
    pub fn horaire(self) -> MontantUnitaire {
        self.horaire
    }
}

/// Arrete du 24/01/1975, art. 3, al. 2 : « periodes d'engagement continu inferieures a cinq jours ».
pub const JOURS_ENGAGEMENT_CONTINU: Duree<EnJours> = Duree::jours(5);

/// Arrete du 24/01/1975, art. 3, al. 2 : « douze fois le plafond horaire » par journee de travail.
pub const HEURES_PAR_JOURNEE: Heures = Heures::fixes(12);

// ---------------------------------------------------------------- les dates

use socle::mois::Annee;

// ----------------------------------------------------------------- le calcul

/// PSS mensuel x jours calendaires de la `periode` / jours du mois, mois par mois,
/// somme. Les deux bornes sont incluses.
pub fn prorata_calendaire(periode: &Periode, pss: &dyn Fn(Annee) -> R<Pss>) -> R<Montant> {
    let mut total: Montant = nulle();
    for mois in periode.mois() {
        let Some(commun) = periode.croise(&Periode::du_mois(mois)) else { continue };
        let jours = Jours::de_dates(commun.jours().count());
        total += pss(mois.annee())?.mensuel.prorata(jours, Jours::fixes(mois.jours()));
    }
    Ok(total)
}

/// Vrai quand l'arrete du 24/01/1975, art. 3, al. 2 s'applique : un artiste engage moins de cinq jours continus (les
/// jours calendaires du `contrat`, bornes incluses : du 24 au 24 fait un jour).
pub fn engagement_court(contrat: &Periode, artiste: bool) -> bool {
    artiste && contrat.duree() < JOURS_ENGAGEMENT_CONTINU
}

/// PSS horaire x 12 par journee de travail accomplie (dates distinctes : deux cachets le meme jour font une
/// journee), au PSS horaire de l'annee de chaque journee. Arrete du 24/01/1975, art. 3, al. 2.
pub fn plafond_journees(dates: &[Jour], pss: &dyn Fn(Annee) -> R<Pss>) -> R<Montant> {
    let journees: BTreeSet<Jour> = dates.iter().copied().collect();
    let mut total: Montant = nulle();
    for j in journees {
        total += HEURES_PAR_JOURNEE * pss(j.annee())?.horaire;
    }
    Ok(total)
}

/// BOSS, assiette generale, § 1090 (tolerance) et § 700 (la formule) : pour chaque jour ayant donne lieu a
/// remuneration (dates distinctes), mois par mois : PSS mensuel de l'annee x jours remuneres du mois / jours du mois ;
/// la somme, non arrondie (la forme de `prorata_calendaire`).
pub fn plafond_jours_remuneres(dates: &[Jour], pss: &dyn Fn(Annee) -> R<Pss>) -> R<Montant> {
    let jours: BTreeSet<Jour> = dates.iter().copied().collect();
    let mut par_mois: std::collections::BTreeMap<socle::mois::Mois, usize> = Default::default();
    for j in jours {
        *par_mois.entry(j.mois_civil()).or_default() += 1;
    }
    let mut total: Montant = nulle();
    for (mois, n) in par_mois {
        total += pss(mois.annee())?.mensuel.prorata(Jours::de_dates(n), Jours::fixes(mois.jours()));
    }
    Ok(total)
}

/// Le refus d'un intermittent dont les journees de travail ne se lisent pas.
#[track_caller]
fn journees_illisibles(contrat: &Periode, court: bool, dites: Jours, datees: usize) -> socle::echec::Echec {
    let regle = if court {
        "le plafond d'un artiste engage moins de cinq jours se compte par journee de travail accomplie, repetitions \
         comprises (arrete du 24/01/1975, art. 3, al. 2)"
    } else {
        "le plafond d'un intermittent se proratise sur les jours ayant donne lieu a remuneration (BOSS, assiette \
         generale, § 1090)"
    };
    socle::refus!(
        "Contrat du {} au {} : {}, et la saisie dit {} jour(s) travaille(s) pour {} date(s). Dater chaque journee de \
         travail sur les lignes de paie : elle ne se devine pas.",
        contrat.debut().iso(), contrat.fin().iso(), regle, dit(dites), datees)
}

/// Le plafond d'un contrat d'intermittent, arrondi au centime, pour les journees `dates` (celles d'une paie, ou de
/// tout le contrat). `artiste` : le statut (l'arrete de 1975 ne vise que les artistes). Artiste engage moins de cinq
/// jours : 12 x PSS horaire par journee (arrete de 1975). Sinon : le prorata du PSS mensuel sur les jours remuneres
/// (BOSS § 1090). Sans date, ou avec une date hors du contrat : refus.
pub fn plafond_urssaf(contrat: &Periode, artiste: bool, pss: &dyn Fn(Annee) -> R<Pss>, dates: &[Jour]) -> R<Montant> {
    let court = engagement_court(contrat, artiste);
    if dates.is_empty() {
        return Err(journees_illisibles(contrat, court, nulle(), 0));
    }
    if let Some(j) = dates.iter().find(|j| !contrat.contient(j)) {
        return Err(socle::refus!("journee de travail le {} hors du contrat du {} au {}", j.iso(), contrat.debut().iso(), contrat.fin().iso()));
    }
    if court {
        Ok(plafond_journees(dates, pss)?.au_centime())
    } else {
        Ok(plafond_jours_remuneres(dates, pss)?.au_centime())
    }
}

/// Le plafond d'un permanent (mensualise) pour sa periode de paie, arrondi au centime : le prorata calendaire de
/// R. 242-2, I, al. 3 (`prorata_calendaire`), et, quand l'employeur l'a choisi pour un salarie a temps partiel, la
/// correction du dernier alinea du meme I : « l'employeur est en droit de corriger le plafond, sans pouvoir augmenter sa
/// valeur mensuelle, a due proportion de la duree de travail inscrite a leur contrat de travail [...], majoree du
/// nombre d'heures complementaires [...], rapportee a celle correspondant a la plus courte des durees mentionnees au 1°
/// de l'article L. 3123-1 du code du travail » (`code-de-la-securite-sociale/articles-20261010.md`, R242-2 ; BOSS,
/// assiette generale, § 760-770, `boss/boss-assiette-generale-20260801.txt` l. 1033-1072 : « valeur mens. du plafond x
/// (duree contractuelle + heures complementaires) / duree legale du travail », et l'exemple « Plafond mensuel x
/// (121,33 h / 151,67 h) x (12 jours / 31 jours) »). `temps_partiel` : (duree du contrat, duree de reference), `None`
/// sans correction. Le rapport ne depasse jamais 1. Textes : `atelier/reference/sources/plafond-temps-partiel/`.
pub fn plafond_permanent(periode: &Periode, temps_partiel: Option<(Heures, Heures)>, pss: &dyn Fn(Annee) -> R<Pss>) -> R<Montant> {
    let prorata = prorata_calendaire(periode, pss)?;
    match temps_partiel {
        Some((contrat, reference)) if contrat < reference => {
            if reference <= Heures::ZERO {
                return Err(socle::refus!("duree de reference du temps partiel nulle ou negative : {}", dit(reference)));
            }
            Ok(prorata.prorata(contrat, reference).au_centime())
        }
        _ => Ok(prorata.au_centime()),
    }
}

// ------------------------------------------------------------ la regularisation sur l'annee

/// LES ASSIETTES QUI SE REGULARISENT SUR L'ANNEE CIVILE, d'une paie ou d'une somme de paies (CSS R. 242-2, II : « Les
/// cotisations calculees dans la limite d'un plafond annuel sont regularisees chaque mois ») :
/// - `t1` : la remuneration dans la limite du plafond (la vieillesse plafonnee, le FNAL, la tranche 1 de la retraite
///   complementaire et de la prevoyance : R. 242-2, II ; BOSS, assiette generale, § 1220-1240 ; lettre circulaire Acoss
///   2013-019, annexe 1, « Regularisation progressive ») ;
/// - `t2` : au-dela du plafond, dans la limite de huit plafonds ; `t1_t2` : dans la limite de huit plafonds ; `cet` :
///   la remuneration dans la limite de huit plafonds quand elle depasse le plafond (la retraite complementaire : cahier
///   Agirc-Arrco CT2026, « la regularisation annuelle du plafond de securite sociale est remplacee par la regularisation
///   progressive au mois le mois ») ;
/// - `t2_conventionnelle` : au-dela du plafond, dans la limite de quatre plafonds (la prevoyance : FAQ DSN des organismes
///   complementaires, la tranche B « pouvant etre negative par effet de regularisation progressive ») ;
/// - `abattue_csg` : la part de la remuneration que l'abattement de la CSG abat, dans la limite de `limite_abattement_csg`
///   plafonds (CSS L. 136-2, I ; BOSS, assiette generale, § 290 : le « plafond annuel », calcule comme celui de la
///   vieillesse, « voir le chapitre 6 », sa section 3, la regularisation, comprise).
/// Textes : `atelier/reference/sources/` (le barème, `regularisation_du_plafond`, les cite). Une assiette regularisee
/// peut etre negative (la FAQ des organismes complementaires le dit de la tranche B) : la DSN la declare signee.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Regularisees {
    pub t1: Montant,
    pub t2: Montant,
    pub t2_conventionnelle: Montant,
    pub t1_t2: Montant,
    pub cet: Montant,
    pub abattue_csg: Montant,
}

impl Regularisees {
    /// Aucune.
    pub const ZERO: Regularisees = Regularisees {
        t1: Montant::ZERO,
        t2: Montant::ZERO,
        t2_conventionnelle: Montant::ZERO,
        t1_t2: Montant::ZERO,
        cet: Montant::ZERO,
        abattue_csg: Montant::ZERO,
    };

    /// Celles d'une remuneration `remuneration` sous un plafond `plafond` (d'une paie, ou de l'annee : deux sommes),
    /// `limite_csg` le nombre de plafonds sous lequel la CSG abat.
    fn sous(remuneration: Montant, plafond: Montant, limite_csg: Coefficient) -> Regularisees {
        let zero = Montant::ZERO;
        let huit = plafond.fois(8);
        Regularisees {
            t1: remuneration.min(plafond),
            t2: zero.max(remuneration.min(huit) - plafond),
            t2_conventionnelle: zero.max(remuneration.min(plafond.fois(4)) - plafond),
            t1_t2: remuneration.min(huit),
            cet: if remuneration > plafond { remuneration.min(huit) } else { zero },
            abattue_csg: remuneration.min(plafond * limite_csg),
        }
    }
}

impl std::ops::Add for Regularisees {
    type Output = Regularisees;
    fn add(self, o: Regularisees) -> Regularisees {
        Regularisees {
            t1: self.t1 + o.t1,
            t2: self.t2 + o.t2,
            t2_conventionnelle: self.t2_conventionnelle + o.t2_conventionnelle,
            t1_t2: self.t1_t2 + o.t1_t2,
            cet: self.cet + o.cet,
            abattue_csg: self.abattue_csg + o.abattue_csg,
        }
    }
}

impl std::ops::Sub for Regularisees {
    type Output = Regularisees;
    fn sub(self, o: Regularisees) -> Regularisees {
        Regularisees {
            t1: self.t1 - o.t1,
            t2: self.t2 - o.t2,
            t2_conventionnelle: self.t2_conventionnelle - o.t2_conventionnelle,
            t1_t2: self.t1_t2 - o.t1_t2,
            cet: self.cet - o.cet,
            abattue_csg: self.abattue_csg - o.abattue_csg,
        }
    }
}

/// CE QUI PRECEDE UNE PAIE DANS L'ANNEE CIVILE, chez la compagnie (CSS R. 242-2, II : l'employeur « fait masse [...] des
/// sommes dues au titre des periodes de travail comprises entre cette echeance et le premier jour de l'annee ou le jour
/// de l'embauche » ; BOSS, assiette generale, § 1230 : « la somme des remunerations dues depuis le debut de l'annee »,
/// « la somme des plafonds applicables », « le montant des assiettes plafonnees appliquees sur chaque echeance de paie
/// precedente ») : la somme des remunerations, celle des plafonds de ces paies, et celle des assiettes qu'elles ont
/// appliquees. Elle part de rien (`debut`), et chaque paie qui se regularise s'y ajoute (`ajouter`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnneeAnterieure {
    remunerations: Montant,
    plafonds: Montant,
    appliquees: Regularisees,
}

impl AnneeAnterieure {
    /// Rien avant : le premier jour de l'annee, ou de l'embauche.
    pub fn debut() -> AnneeAnterieure {
        AnneeAnterieure { remunerations: Montant::ZERO, plafonds: Montant::ZERO, appliquees: Regularisees::ZERO }
    }

    /// Une paie anterieure qui se regularise : sa remuneration, son plafond, et les assiettes qu'elle a appliquees, telles
    /// qu'elle les a ecrites (celles d'une paie corrigee depuis comprises : la suivante rattrape l'ecart).
    pub fn ajouter(&mut self, remuneration: Montant, plafond: Montant, appliquees: Regularisees) {
        self.remunerations += remuneration;
        self.plafonds += plafond;
        self.appliquees = self.appliquees + appliquees;
    }
}

/// COMMENT LE PLAFOND D'UNE PAIE TIENT COMPTE DE L'ANNEE : il se regularise sur ce qui la precede (`Regularise`, CSS
/// R. 242-2, II), ou il est celui de la paie seule (`ParPaie`) : BOSS, assiette generale, § 1250, « Les salaries
/// beneficiant de taux, d'assiettes ou de montants specifiques ou forfaitaires de cotisations [...] ne sont pas eligibles
/// au principe de regularisation des cotisations plafonnees : pour ces salaries, l'assiette plafonnee applicable lors de
/// chaque paie ne prend pas en compte la ou les assiettes plafonnees appliquees lors des echeances precedentes » ; qui
/// l'est, le bareme le dit (`forme_bareme::RegularisationDuPlafond::regularise`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Plafonnement {
    ParPaie,
    Regularise(AnneeAnterieure),
}

/// LES ASSIETTES PLAFONNEES D'UNE PAIE : celles qui se regularisent (`regularisees`), et le brut dans la limite de quatre
/// plafonds de la paie (`quatre_plafonds` : le chomage, l'AGS, l'Apec d'un permanent), qui ne se regularise pas : aucun
/// texte depose ne dit la regularisation de ces contributions (la fiche Unedic ne dit que l'identite de leur assiette avec
/// celle de la securite sociale), et seul se regularise ce qu'un texte depose fonde (Leandre, 19/11/2026).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssiettesPlafonnees {
    pub regularisees: Regularisees,
    pub quatre_plafonds: Montant,
}

/// LES ASSIETTES PLAFONNEES de la paie de remuneration `remuneration` (son brut soumis) et de plafond `plafond` (le
/// plafond de la paie, `pour_paie`), `limite_csg` le nombre de plafonds sous lequel la CSG abat (le bareme,
/// `limite_abattement_csg`) : le seul endroit ou elles se calculent, et elles ne se calculent pas sans dire ce qui
/// precede la paie dans l'annee (`plafonnement`).
///
/// Regularisee : chacune est celle de la somme des remunerations sous la somme des plafonds, de l'annee jusqu'a cette
/// paie comprise, moins ce que les paies d'avant ont applique (BOSS, assiette generale, § 1230 : « si la somme des
/// remunerations est inferieure a la somme des plafonds, l'assiette plafonnee a retenir pour une echeance de paie donnee
/// est egale a la somme des remunerations, diminuee du montant des assiettes plafonnees appliquees sur chaque echeance de
/// paie precedente » ; au-dessus, « egale a la somme des plafonds » diminuee de meme). Elle peut etre negative, ou depasser
/// la remuneration de la paie.
pub fn assiettes_plafonnees(remuneration: Montant, plafond: Montant, limite_csg: Coefficient, plafonnement: &Plafonnement) -> AssiettesPlafonnees {
    let regularisees = match plafonnement {
        Plafonnement::ParPaie => Regularisees::sous(remuneration, plafond, limite_csg),
        Plafonnement::Regularise(a) => {
            Regularisees::sous(a.remunerations + remuneration, a.plafonds + plafond, limite_csg) - a.appliquees
        }
    };
    AssiettesPlafonnees { regularisees, quatre_plafonds: remuneration.min(plafond.fois(4)) }
}

/// Ce que `pour_paie` lit de l'engagement.
#[derive(Debug, Clone)]
pub struct EngagementPlafond {
    /// La nature du contrat est payee au mois (`Nature::mensuelle`).
    pub mensuelle: bool,
    /// Le statut du salarie est `artiste` (vide compris) ; `false` : `technicien`.
    pub artiste: bool,
    /// La periode de paie.
    pub periode: Periode,
    /// Le contrat, du debut a la fin.
    pub contrat: Periode,
    /// Les jours travailles de la paie, dates (les dates distinctes de toutes les lignes).
    pub dates_travail: Vec<Jour>,
    /// Les jours travailles que la saisie dit (saisis, sinon les dates distinctes).
    pub jours: Jours,
}

/// Le plafond d'UNE paie (R. 242-2 : le plafond « retenu pour chaque paie »). Un permanent : le prorata de droit
/// commun sur la periode de paie du mois. Un intermittent : les journees datees de CETTE paie (aucune journee dans le
/// mois, aucun plafond) : 12 x horaire chacune pour un artiste engage moins de cinq jours, sinon le prorata mensuel
/// des jours remuneres (BOSS § 1090). Une journee dite et non datee refuse, une date hors de la periode de paie aussi.
pub fn pour_paie(e: &EngagementPlafond, pss: &dyn Fn(Annee) -> R<Pss>) -> R<Montant> {
    if e.mensuelle {
        return Ok(prorata_calendaire(&e.periode, pss)?.au_centime());
    }
    let court = engagement_court(&e.contrat, e.artiste);
    let datees = e.dates_travail.iter().collect::<BTreeSet<_>>().len();
    if e.jours != Jours::de_dates(datees) {
        return Err(journees_illisibles(&e.contrat, court, e.jours, datees));
    }
    if datees == 0 {
        return Ok(Montant::ZERO);
    }
    if let Some(j) = e.dates_travail.iter().find(|j| !e.periode.contient(j)) {
        return Err(socle::refus!(
            "journee de travail le {} hors de la periode de paie du {} au {} : le plafond se retient pour chaque paie \
             (R. 242-2 CSS), sur les jours de cette paie",
            j.iso(), e.periode.debut().iso(), e.periode.fin().iso()));
    }
    plafond_urssaf(&e.contrat, e.artiste, pss, &e.dates_travail)
}
