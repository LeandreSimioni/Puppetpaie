//! Le « Plafond Urssaf » du bas de bulletin : le plafond de securite sociale d'une paie (L. 241-3 CSS), celui sur
//! lequel s'assoient la vieillesse plafonnee, le FNAL et le chomage d'un intermittent (`calcul::calculer`, 7.6).
//!
//! ```text
//! artiste, periode d'engagement continu de moins de cinq jours (du debut a la fin du contrat, jours calendaires) :
//!     plafond = PSS horaire x 12 x journees de travail accomplies dans la paie (dates distinctes)
//! autre intermittent (artiste engage cinq jours ou plus, technicien) :
//!     plafond = au centime( PSS mensuel x jours calendaires du contrat dans la periode de paie / jours du mois )
//! permanent (mensualise) :
//!     plafond = au centime( PSS mensuel x jours de la periode de paie couverts par le contrat, hors absence non
//!                           remuneree / jours du mois )
//! ```
//!
//! Textes (`atelier/reference/sources/`) :
//! - arrete du 24/01/1975, art. 3, al. 2 (`arrete-1975-artistes-du-spectacle/articles-20261010.md`) : « Pour les
//!   periodes d'engagement continu inferieures a cinq jours, le plafond [...] applicable a chaque journee de travail
//!   accomplie par un artiste du spectacle pour un meme employeur est egal a douze fois le plafond horaire, quels que
//!   soient le nombre d'heures et la nature du travail effectues dans ladite journee. » Une journee, pas un cachet ;
//!   « est egal », pas un plancher ; la duree de l'engagement, pas une suite de jours travailles. L'arrete ne vise que
//!   les artistes (art. 1 : 15° de L. 311-3 CSS) ;
//! - CSS R. 242-2, I, al. 2 et 3 (`code-de-la-securite-sociale/articles-20261010.md`) : le plafond « retenu pour
//!   chaque paie », « reduit[s] a due proportion du nombre de jours de la periode pendant laquelle les personnes sont
//!   employees » ; BOSS, assiette generale, version du 01/08/2026, § 690-750 (`boss/boss-assiette-generale-20260801.txt`,
//!   l. 987-1025) : « a proportion des jours couverts par le contrat de travail au cours de cette periode », la formule
//!   du § 700, « valeur mens. du plafond x (nombre de jours de la periode d'emploi / nombre de jours calendaires du
//!   mois) », et le § 740, « Pour les salaries non mensualises, le plafond retenu est ajuste en fonction de la periode
//!   d'emploi a laquelle se rattache la paie ». La regle de tout salarie hors de l'arrete de 1975, permanent ou
//!   intermittent : les jours de la periode d'emploi sont les jours calendaires du contrat dans la periode de la paie,
//!   les deux bornes incluses, quels que soient les jours travailles ;
//! - BOSS, assiette generale, § 1090 (l. 1472-1483), premiere phrase : les artistes du spectacle « ne sont pas eligibles
//!   a la reduction du plafond : pour ces salaries, s'ils ne sont pas mensualises, le plafond applicable depend uniquement
//!   de la periodicite de la paie ». Aucun jour d'absence ne sort donc du prorata d'un artiste. La tolerance du meme
//!   paragraphe (« il est admis que [...] le plafond soit proratise pour ne tenir compte que des jours ayant donne lieu a
//!   remuneration ») n'est pas appliquee : la loi, sans la tolerance (Leandre, 24/11/2026) ;
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

/// Le refus d'un artiste engage moins de cinq jours dont les journees de travail ne se lisent pas : l'arrete de 1975
/// compte son plafond par journee, et une journee ne se devine pas.
#[track_caller]
fn journees_illisibles(contrat: &Periode, dites: Jours, datees: usize) -> socle::echec::Echec {
    socle::refus!(
        "Contrat du {} au {} : le plafond d'un artiste engage moins de cinq jours se compte par journee de travail \
         accomplie, repetitions comprises (arrete du 24/01/1975, art. 3, al. 2), et la saisie dit {} jour(s) \
         travaille(s) pour {} date(s). Dater chaque journee de travail sur les lignes de paie : elle ne se devine pas.",
        contrat.debut().iso(), contrat.fin().iso(), dit(dites), datees)
}

/// Le plafond d'un contrat d'intermittent entier, arrondi au centime. `artiste` : le statut (l'arrete de 1975 ne vise
/// que les artistes). Artiste engage moins de cinq jours : 12 x PSS horaire par journee `dates` (arrete de 1975) ; sans
/// date, ou avec une date hors du contrat, refus. Sinon : le prorata calendaire du contrat (`plafond_de_la_periode`,
/// R. 242-2, I, al. 3), sans jour retire, quels que soient les jours travailles. Le plafond d'une paie :
/// `pour_paie`.
pub fn plafond_urssaf(contrat: &Periode, artiste: bool, pss: &dyn Fn(Annee) -> R<Pss>, dates: &[Jour]) -> R<Montant> {
    if !engagement_court(contrat, artiste) {
        return plafond_de_la_periode(contrat, &JoursSansRemuneration::aucun(), None, pss);
    }
    if dates.is_empty() {
        return Err(journees_illisibles(contrat, nulle(), 0));
    }
    if let Some(j) = dates.iter().find(|j| !contrat.contient(j)) {
        return Err(socle::refus!("journee de travail le {} hors du contrat du {} au {}", j.iso(), contrat.debut().iso(), contrat.fin().iso()));
    }
    Ok(plafond_journees(dates, pss)?.au_centime())
}

/// Le plafond d'une periode de paie, arrondi au centime : le prorata calendaire de R. 242-2, I, al. 3 (PSS mensuel x
/// jours calendaires de la periode / jours du mois, mois par mois, les deux bornes incluses ; BOSS, assiette generale,
/// § 700), celui de tout salarie hors de l'arrete de 1975, permanent ou intermittent, sur la part du contrat que la
/// periode `periode` couvre, et, quand l'employeur l'a choisi pour un salarie a temps partiel, la
/// correction du dernier alinea du meme I : « l'employeur est en droit de corriger le plafond, sans pouvoir augmenter sa
/// valeur mensuelle, a due proportion de la duree de travail inscrite a leur contrat de travail [...], majoree du
/// nombre d'heures complementaires [...], rapportee a celle correspondant a la plus courte des durees mentionnees au 1°
/// de l'article L. 3123-1 du code du travail » (`code-de-la-securite-sociale/articles-20261010.md`, R242-2 ; BOSS,
/// assiette generale, § 760-770, `boss/boss-assiette-generale-20260801.txt` l. 1033-1072 : « valeur mens. du plafond x
/// (duree contractuelle + heures complementaires) / duree legale du travail », et l'exemple « Plafond mensuel x
/// (121,33 h / 151,67 h) x (12 jours / 31 jours) »). `temps_partiel` : (duree du contrat, duree de reference), `None`
/// sans correction. Le rapport ne depasse jamais 1. Textes : `atelier/reference/sources/plafond-temps-partiel/`.
///
/// Les jours `sans_remuneration` (`JoursSansRemuneration`) sortent du prorata : CSS R. 242-2, I, al. 4, dernier tiret,
/// « Le plafond est egalement reduit [...] pour tenir compte de periodes d'absence n'ayant pas donne lieu a
/// remuneration » ; BOSS, assiette generale, § 870-880 (l. 1189-1195) : « valeur mensuelle du plafond x (nombre de jours
/// remuneres de la periode d'emploi / nombre de jours calendaires du mois) », les jours remuneres etant « le nombre de
/// jours calendaires desquels sont retires les jours d'absence ne donnant pas lieu a maintien de la remuneration ». La
/// correction du temps partiel s'applique ensuite (§ 910 ; l'exemple du § 770 multiplie les deux rapports).
pub fn plafond_de_la_periode(
    periode: &Periode, sans_remuneration: &JoursSansRemuneration, temps_partiel: Option<(Heures, Heures)>, pss: &dyn Fn(Annee) -> R<Pss>,
) -> R<Montant> {
    let mut prorata: Montant = nulle();
    for mois in periode.mois() {
        let Some(commun) = periode.croise(&Periode::du_mois(mois)) else { continue };
        let remuneres = commun.jours().filter(|j| !sans_remuneration.0.contains(j)).count();
        prorata += pss(mois.annee())?.mensuel.prorata(Jours::de_dates(remuneres), Jours::fixes(mois.jours()));
    }
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

/// LES JOURS D'ABSENCE SANS REMUNERATION D'UNE PAIE, qui reduisent son plafond (`plafond_de_la_periode`) : CSS R. 242-2, I,
/// al. 4, dernier tiret ; BOSS, assiette generale, § 820-900 (`boss/boss-assiette-generale-20260801.txt`, l. 1142-1203).
/// Un jour y entre quand l'absence de ce jour « ne donne pas lieu a un maintien de la remuneration par l'employeur »
/// (§ 840) ; une absence « partiellement ou totalement remuneree par l'employeur, sous forme de versement visant au
/// maintien de salaire » n'y entre pas (§ 820). Seules les journees d'absence totale comptent (§ 890), de date a date, les
/// jours de repos et feries compris (§ 900). Qui sont ces jours, la deduction du mois d'un permanent le dit
/// (`engagement::conges::lignes_du_mois`, avec les § 860 et 920) ; ce type ne fait que les porter.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JoursSansRemuneration(BTreeSet<Jour>);

impl JoursSansRemuneration {
    /// Aucun jour : une paie sans absence non remuneree.
    pub fn aucun() -> JoursSansRemuneration {
        JoursSansRemuneration(BTreeSet::new())
    }

    /// Ces jours.
    pub fn de(jours: BTreeSet<Jour>) -> JoursSansRemuneration {
        JoursSansRemuneration(jours)
    }

    /// Ces jours, sans `jours` : ceux que la paie maintient finalement (le complement d'un maintien garanti en net, que
    /// seul le calcul de la paie connait : `engagement::maintien::Reconstitue::maintenus`), ou ceux d'un arret pour le
    /// mois « s'il avait continue de travailler » (`engagement::maintien::Reconstitution::arrets`).
    pub fn hors(&self, jours: &BTreeSet<Jour>) -> JoursSansRemuneration {
        JoursSansRemuneration(self.0.difference(jours).copied().collect())
    }
}

// ------------------------------------------------------------ la regularisation sur l'annee

/// LES ASSIETTES QUI SE REGULARISENT SUR L'ANNEE CIVILE, d'une paie ou d'une somme de paies (CSS R. 242-2, II : « Les
/// cotisations calculees dans la limite d'un plafond annuel sont regularisees chaque mois ») :
/// - `t1` : la remuneration dans la limite du plafond (la vieillesse plafonnee, le FNAL, la tranche 1 de la retraite
///   complementaire et de la prevoyance : R. 242-2, II ; BOSS, assiette generale, § 1220-1240 ; lettre circulaire Acoss
///   2013-019, annexe 1, « Regularisation progressive ») ;
/// - `t2` : au-dela du plafond, dans la limite de `TranchesDuPlafond::t2` plafonds ; `t1_t2` : dans cette limite ; `cet` :
///   la remuneration dans cette limite quand elle depasse le plafond (la retraite complementaire : ANI Agirc-Arrco du
///   17/11/2017, art. 44, « Les versements de cotisations donnent lieu a une regularisation progressive telle que prevue
///   au II. de l'article R. 242-2 du code de la securite sociale », circulaire 2018-09-DC, l. 1642-1643 ; cahier
///   Agirc-Arrco CT2026). Pour la CET, sa base regularisee devient celle de toute l'annee (la remuneration dans la limite
///   de la tranche 2) des que la somme des remunerations depasse la somme des plafonds, et retombe a zero en dessous :
///   Audiens le dit, Parametres 2026, « Cas particulier : la CET », exemple 2
///   (`audiens-parametres-2026/Audiens-Parametres-2026.txt`, l. 97-101) : « Pas de CET sur les bulletins de janvier a
///   novembre (car le PSS n'est pas atteint) », « Puis, CET due des lors que le salaire atteint le PSS ➔ la CET sera
///   calculee sur la base de 50 600 € sur le bulletin de decembre » ; l'ANI (art. 37) la dit due « pour les participants
///   dont la remuneration excede le plafond », et la fiche net-entreprises 2556
///   (`net-entreprises-base-connaissances/net-entreprises-fiche2556-agirc-arrco-fiabilisation-20260708.txt`, l. 89) la
///   suppose regularisee, « y compris sous forme negative » ;
/// - `t2_conventionnelle` : au-dela du plafond, dans la limite de `TranchesDuPlafond::t2_conventionnelle` plafonds (la
///   prevoyance). INFERENCE : un seul emetteur, la FAQ DSN des organismes complementaires, suppose sa regularisation (la
///   tranche B « pouvant etre negative par effet de regularisation progressive ») sans la poser ; aucun accord de
///   prevoyance depose ne la dit ;
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
    /// `limite_csg` le nombre de plafonds sous lequel la CSG abat, `tranches` les bornes des tranches (le bareme).
    fn sous(remuneration: Montant, plafond: Montant, limite_csg: Coefficient, tranches: &TranchesDuPlafond) -> Regularisees {
        let zero = Montant::ZERO;
        let haut_t2 = plafond * tranches.t2;
        Regularisees {
            t1: remuneration.min(plafond),
            t2: zero.max(remuneration.min(haut_t2) - plafond),
            t2_conventionnelle: zero.max(remuneration.min(plafond * tranches.t2_conventionnelle) - plafond),
            t1_t2: remuneration.min(haut_t2),
            cet: if remuneration > plafond { remuneration.min(haut_t2) } else { zero },
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

    /// RIEN AVANT, SOUS LE PLAFOND DE L'ANNEE `plafond_annuel` : l'annee de la retraite complementaire d'un intermittent
    /// (`tranches_sous_le_plafond_annuel`). Le plafond est celui de l'annee entiere des le premier jour, « quelle que soit
    /// la duree du travail » (Audiens, Parametres 2026, l. 67-69) : une paie ne l'augmente pas (elle s'ajoute avec un
    /// plafond nul, `ajouter`).
    pub fn sous_le_plafond_annuel(plafond_annuel: Montant) -> AnneeAnterieure {
        AnneeAnterieure { remunerations: Montant::ZERO, plafonds: plafond_annuel, appliquees: Regularisees::ZERO }
    }

    /// Une paie anterieure qui se regularise : sa remuneration, son plafond, et les assiettes qu'elle a appliquees, telles
    /// qu'elle les a ecrites (celles d'une paie corrigee depuis comprises : la suivante rattrape l'ecart).
    pub fn ajouter(&mut self, remuneration: Montant, plafond: Montant, appliquees: Regularisees) {
        self.remunerations += remuneration;
        self.plafonds += plafond;
        self.appliquees = self.appliquees + appliquees;
    }

    /// La somme des plafonds a laquelle la remuneration de l'annee se compare (le plafond de l'annee, sous
    /// `sous_le_plafond_annuel`).
    pub fn plafonds(&self) -> Montant {
        self.plafonds
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

/// LES ASSIETTES PLAFONNEES D'UNE PAIE : celles qui se regularisent (`regularisees`), le brut dans la limite des
/// plafonds du chomage de la paie (`chomage` : le chomage et l'AGS d'un permanent, le chomage d'un intermittent ;
/// `TranchesDuPlafond::chomage`) et dans celle des plafonds de l'Apec d'un permanent (`apec`, `TranchesDuPlafond::apec`),
/// qui ne se regularisent pas : aucun texte depose ne dit la regularisation de ces contributions (la fiche Unedic ne dit
/// que l'identite de leur assiette avec celle de la securite sociale), et seul se regularise ce qu'un texte depose fonde
/// (Leandre, 19/11/2026).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssiettesPlafonnees {
    pub regularisees: Regularisees,
    pub chomage: Montant,
    pub apec: Montant,
}

/// LA PAIE OU LA CONTRIBUTION D'EQUILIBRE TECHNIQUE DE L'ANNEE BASCULE : le critere de `Regularisees::sous` (la somme des
/// remunerations de l'annee depasse la somme des plafonds) change avec elle. `Due` : la somme passe au-dessus, de
/// `depassement` ; la CET devient due sur toute la remuneration de l'annee. `Rendue` : elle repasse au plus a la somme
/// des plafonds, `ecart` en dessous ; la CET de l'annee se rend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BasculeDeLaCet {
    Due { depassement: Montant },
    Rendue { ecart: Montant },
}

impl BasculeDeLaCet {
    /// La bascule d'une paie (ou d'un mois decoupe en periodes), de l'annee qui la precede, `avant`, a l'annee jusqu'a
    /// elle comprise, `apres` ; aucune quand le critere est le meme des deux cotes.
    pub fn entre(avant: &AnneeAnterieure, apres: &AnneeAnterieure) -> Option<BasculeDeLaCet> {
        match (avant.remunerations > avant.plafonds, apres.remunerations > apres.plafonds) {
            (false, true) => Some(BasculeDeLaCet::Due { depassement: apres.remunerations - apres.plafonds }),
            (true, false) => Some(BasculeDeLaCet::Rendue { ecart: apres.plafonds - apres.remunerations }),
            _ => None,
        }
    }
}

/// LES ASSIETTES PLAFONNEES de la paie de remuneration `remuneration` (son brut soumis) et de plafond `plafond` (le
/// plafond de la paie, `pour_paie`), `limite_csg` le nombre de plafonds sous lequel la CSG abat (le bareme,
/// `limite_abattement_csg`), `tranches` les bornes des tranches en nombre de plafonds (le bareme, `tranches_du_plafond`) :
/// le seul endroit ou elles se calculent, et elles ne se calculent pas sans dire ce qui precede la paie dans l'annee
/// (`plafonnement`).
///
/// Regularisee : chacune est celle de la somme des remunerations sous la somme des plafonds, de l'annee jusqu'a cette
/// paie comprise, moins ce que les paies d'avant ont applique (BOSS, assiette generale, § 1230 : « si la somme des
/// remunerations est inferieure a la somme des plafonds, l'assiette plafonnee a retenir pour une echeance de paie donnee
/// est egale a la somme des remunerations, diminuee du montant des assiettes plafonnees appliquees sur chaque echeance de
/// paie precedente » ; au-dessus, « egale a la somme des plafonds » diminuee de meme). Elle peut etre negative, ou depasser
/// la remuneration de la paie.
pub fn assiettes_plafonnees(
    remuneration: Montant, plafond: Montant, limite_csg: Coefficient, tranches: &TranchesDuPlafond, plafonnement: &Plafonnement,
) -> AssiettesPlafonnees {
    let regularisees = match plafonnement {
        Plafonnement::ParPaie => Regularisees::sous(remuneration, plafond, limite_csg, tranches),
        Plafonnement::Regularise(a) => {
            Regularisees::sous(a.remunerations + remuneration, a.plafonds + plafond, limite_csg, tranches) - a.appliquees
        }
    };
    AssiettesPlafonnees { regularisees, chomage: remuneration.min(plafond * tranches.chomage), apec: remuneration.min(plafond * tranches.apec) }
}

/// LES TRANCHES DE LA RETRAITE COMPLEMENTAIRE D'UNE PAIE D'INTERMITTENT, SUR LE PLAFOND DE L'ANNEE : la tranche 1, la
/// tranche 2 et la CET (`Regularisees::t1`, `t2`, `cet` ; les autres champs n'ont pas de sens ici) de la remuneration
/// `remuneration` que la retraite compte, `annee` ce qui la precede dans l'annee chez la compagnie, sous le plafond annuel
/// (`AnneeAnterieure::sous_le_plafond_annuel`). Le meme calcul que celui d'une paie qui se regularise
/// (`assiettes_plafonnees`), une paie n'ajoutant aucun plafond a celui de l'annee. Textes, deux emetteurs :
/// - Audiens, Parametres 2026 (`audiens-parametres-2026/Audiens-Parametres-2026.txt`, l. 66-74) : « Non-cadres : Pour
///   determiner les tranches (T1 et T2), ce sont les plafonds annuels qui doivent etre retenus, quelle que soit la duree
///   du travail » ; « 100 jours de travail remuneres : 50 000 € ➔ T1 = 48 060 € et T2 = 1 940 € » ; et la CET (l. 103-105) :
///   « intermittent du spectacle non-cadre : 35 000 € de salaire brut sur l'annee ➔ CET non due », « 50 000 € [...] ➔ CET
///   due » ;
/// - la circulaire Agirc-Arrco 2018-09-DC, annexe A, art. 1, « Intermittents des professions du spectacle et mannequins »
///   (`agirc-arrco/agirc-arrco-circulaire-2018-09-dc-ani-20171117-20181219.txt`, l. 4246-4248) : « les remunerations
///   versees dans l'annee par chaque employeur sont traitees de facon distincte, dans les limites annuelles des assiettes
///   T1 et T2 telles que definies a l'article 32 ne tenant pas compte de la duree de chaque emploi ».
/// Le moment de la CET est celui d'une paie qui se regularise (`Regularisees`, l'exemple 2 d'Audiens) : rien tant que la
/// remuneration de l'annee reste sous le plafond, toute celle de l'annee a la paie qui le passe (`BasculeDeLaCet`).
pub fn tranches_sous_le_plafond_annuel(remuneration: Montant, tranches: &TranchesDuPlafond, annee: &AnneeAnterieure) -> Regularisees {
    assiettes_plafonnees(remuneration, Montant::ZERO, Coefficient::ZERO, tranches, &Plafonnement::Regularise(*annee)).regularisees
}

use crate::forme_bareme::TranchesDuPlafond;

/// Ce que `pour_paie` lit de l'engagement d'un intermittent.
#[derive(Debug, Clone)]
pub struct EngagementPlafond {
    /// Le statut du salarie est `artiste` (vide compris) ; `false` : `technicien`.
    pub artiste: bool,
    /// La periode de paie.
    pub periode: Periode,
    /// Le contrat, du debut a la fin.
    pub contrat: Periode,
    /// Les jours travailles de la paie, dates (les dates distinctes de toutes les lignes) : l'arrete de 1975 les compte.
    pub dates_travail: Vec<Jour>,
    /// Les jours travailles que la saisie dit (saisis, sinon les dates distinctes).
    pub jours: Jours,
}

/// Le plafond d'UNE paie d'intermittent (R. 242-2 : le plafond « retenu pour chaque paie » ; celui d'un permanent :
/// `plafond_de_la_periode`, avec ses absences et son temps partiel).
///
/// Artiste engage moins de cinq jours : 12 x horaire par journee datee de CETTE paie (arrete de 1975 ; aucune journee
/// dans le mois, aucun plafond) ; une journee dite et non datee refuse, une date hors de la periode de paie aussi.
///
/// Tout autre intermittent : le prorata calendaire de la part du contrat que couvre la periode de paie
/// (`plafond_de_la_periode`, R. 242-2, I, al. 3 ; BOSS § 700 et 740), quels que soient les jours travailles. Aucun jour
/// n'en sort : l'artiste « n'est pas eligible a la reduction du plafond » (BOSS § 1090), et le moteur ne connait aucun
/// jour d'absence sans remuneration d'un technicien intermittent (`Engagement::sans_remuneration` n'est deduit que du
/// mois d'un salarie au mois). Sans correction du temps partiel : le moteur ne la calcule que pour un contrat paye au
/// mois (`engagement::calcul::plafond_de_l_engagement`).
pub fn pour_paie(e: &EngagementPlafond, pss: &dyn Fn(Annee) -> R<Pss>) -> R<Montant> {
    if !engagement_court(&e.contrat, e.artiste) {
        let couverte = e.periode.croise(&e.contrat).ok_or_else(|| {
            socle::refus!(
                "la periode de paie du {} au {} ne touche pas le contrat du {} au {}",
                e.periode.debut().iso(), e.periode.fin().iso(), e.contrat.debut().iso(), e.contrat.fin().iso())
        })?;
        return plafond_de_la_periode(&couverte, &JoursSansRemuneration::aucun(), None, pss);
    }
    let datees = e.dates_travail.iter().collect::<BTreeSet<_>>().len();
    if e.jours != Jours::de_dates(datees) {
        return Err(journees_illisibles(&e.contrat, e.jours, datees));
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
