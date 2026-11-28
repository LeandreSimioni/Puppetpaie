//! L'ANNEE D'INDEMNISATION D'UN INTERMITTENT, ESTIMEE MOIS PAR MOIS (annexes VIII et X au reglement d'assurance chomage,
//! `atelier/reference/sources/plafond-intermittents/unedic-annexe8-convention-20241115.txt`, `…annexe10…` ; le guide de
//! France Travail, `chomage-intermittents/francetravail-guide-intermittents-20261020.txt`, p. 11 a 17) : l'annee qui suit
//! la fin de contrat de l'ouverture des droits, jusqu'a sa date anniversaire (art. 9), sans travail ou avec le travail que
//! l'appelant dit, mois par mois.
//!
//! - L'ANNEE : du lendemain de la fin de contrat au terme des douze mois qui la suivent (art. 9 §1 c). A une readmission,
//!   elle court au plus tot au lendemain de la date anniversaire precedente (art. 23 §1 ; guide, exemple 9 : « ouverture
//!   de droit […] sur la FCT du 15/05/23 pour une période d'indemnisation du 13/09/23 au 15/05/24 »).
//! - L'ALLOCATION DU JOUR (`AllocationDuJour`) : l'allocation retenue (art. 14 a 16), moins la participation de 0,93 % du
//!   salaire journalier moyen (art. 19), sans passer sous l'allocation minimale ; moins la CSG et la CRDS, chacune sur
//!   l'allocation brute abattue de 1,75 %, sans faire passer sous 62 € l'allocation brute diminuee de ce qui est deja pris
//!   (circulaire Unedic 2026-06, fiche 2, § 7 et exemple 20 ;
//!   `chomage-permanents/unedic-circulaire-2026-06-reglementation-20260901.txt`). Le taux est celui de la personne
//!   (`TauxDeCsg`), qui ne joue que si l'allocation brute passe le plancher ; l'appelant qui ne le connait pas compte
//!   l'annee aux deux bouts de `TauxDeCsg::FOURCHETTE`. Seuls s'arrondissent, au centime, l'assiette abattue et
//!   chaque contribution, comme l'ecrit l'exemple 20 (« Assiette = 106,90 € (108,80 € x 98,25 %) », « Montant CSG =
//!   6,63 € ») ; le salaire journalier moyen et la participation restent exacts : aucun exemple depose ne les arrondit.
//! - UN MOIS (le guide, p. 16-17, etapes 2 a 6, dans cet ordre) : ses heures font des jours de travail (art. 30), qui
//!   retirent chacun un coefficient de jours indemnisables ; au seuil de l'annexe, rien n'est verse (etapes 2 et 3) ;
//!   l'allocation a verser est l'allocation journaliere fois ces jours (etape 4) ; le salaire et cette allocation ne
//!   passent pas 1,18 fois le plafond mensuel de la Securite sociale, et l'allocation est alors la difference, ses jours
//!   arrondis a l'entier superieur (etape 5 ; art. 30) ; enfin « votre nombre de jours indemnisables obtenu après la prise
//!   en compte d'une activité professionnelle » est reduit du delai d'attente, de la franchise de conges payes du mois, de
//!   celle de salaires du mois, puis du reliquat de l'une et de l'autre (etape 6 ; art. 23 §1 : le delai d'attente court
//!   « après application des dispositions de l'article 30 »). Ce que verse le mois : ses jours payes fois l'allocation
//!   journaliere, sans passer la difference du plafond du cumul quand il a joue (INFERENCE : le guide ne dit pas le montant
//!   d'un mois plafonne dont une franchise prend des jours).
//! - LE DELAI D'ATTENTE (art. 22) s'applique « dès lors qu'il n'excède pas sept jours sur une même période de douze mois » :
//!   a la premiere ouverture (aucune date anniversaire) ; pas a la readmission a la date anniversaire (exemple 9 : « Pas
//!   d'application du délai d'attente ») ; a une readmission plus de douze mois apres la date anniversaire, aucun delai
//!   precedent ne tombe dans les douze mois. Entre les deux, les textes ne disent rien que la personne ait dit : il ne se
//!   compte pas (`ManqueDeLAnnee::DelaiDAttente`).
//! - LES FRANCHISES (art. 21, art. 23 §2) : la franchise de conges compte les jours travailles de la periode ; celle de
//!   salaires, ses salaires et le SMIC de son dernier jour. Ce qu'un mois n'a pas pris passe au suivant. Ce qui manque
//!   pour l'une ou l'autre ne devient pas zero : elle ne se deduit pas, et l'annee le dit (`ManqueDeLAnnee`) : son chomage
//!   est alors « au plus ».
//! - LE PLAFOND DE LA SECURITE SOCIALE d'une annee que le bareme livre ne porte pas encore : le dernier qu'il porte
//!   (Léandre, 28/11/2026 : « on prend le dernier en date 2027 arrive bientôt » ; `plafond_mensuel_livre`), et l'annee le
//!   dit (`AnneeDeChomage::plafonds_repris`).
//!
//! Les jours de travail, les jours retires et les jours payes d'un mois ne s'arrondissent pas : seuls le plafond du cumul
//! (art. 30) et les franchises (leur total, leur part du mois) disent un arrondi. Les valeurs et leurs textes sont dans
//! `allocation-intermittents.json` (`droit::allocation`).

use crate::allocation::{ActiviteReprise, Allocation};
use crate::emplois::AnnexeChomage;
use socle::date::{Duree, Jour, Periode};
use socle::echec::R;
use socle::grandeurs::{Heures, Jours, Montant, MontantUnitaire, Taux};
use socle::mois::{Annee, Mois};

/// CE QUE LA PERSONNE PAIE DE CSG ET DE CRDS sur son allocation (circulaire Unedic 2026-06, fiche 2, § 7), selon son
/// revenu fiscal de reference ; seule la personne le sait. La circulaire dit l'exoneration de la CSG (totale, ou le taux
/// reduit) et, dans un paragraphe a part, que « certains allocataires peuvent bénéficier d'une exonération totale de la
/// CRDS » : elle ne dit pas que l'une emporte l'autre, donc l'exoneration de la CSG se choisit avec ou sans la CRDS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TauxDeCsg {
    Plein,
    Reduit,
    ExonereDeCsg,
    ExonereDeCsgEtDeCrds,
}

impl TauxDeCsg {
    /// Les deux bouts de ce qu'une allocation peut verser quand la personne ne dit pas son taux : le taux plein, qui
    /// prend le plus, puis l'exoneration de l'une et de l'autre, qui ne prend rien.
    pub const FOURCHETTE: [TauxDeCsg; 2] = [TauxDeCsg::Plein, TauxDeCsg::ExonereDeCsgEtDeCrds];

    /// Le taux de CSG que la personne paie (aucun : elle en est exoneree), lu dans `c`.
    fn csg(&self, c: &crate::allocation::ContributionsSurLAllocation) -> Option<Taux> {
        match self {
            TauxDeCsg::Plein => Some(c.csg),
            TauxDeCsg::Reduit => Some(c.csg_reduite),
            TauxDeCsg::ExonereDeCsg | TauxDeCsg::ExonereDeCsgEtDeCrds => None,
        }
    }

    /// Le taux de CRDS que la personne paie (aucun : elle en est exoneree), lu dans `c`.
    fn crds(&self, c: &crate::allocation::ContributionsSurLAllocation) -> Option<Taux> {
        match self {
            TauxDeCsg::Plein | TauxDeCsg::Reduit | TauxDeCsg::ExonereDeCsg => Some(c.crds),
            TauxDeCsg::ExonereDeCsgEtDeCrds => None,
        }
    }
}

/// Le taux de CSG est une liste fermee : la charge l'ecrit par son code (`plein`…), l'ecran la fait choisir.
impl socle::liste::ListeFermee for TauxDeCsg {
    const TOUTES: &'static [TauxDeCsg] = &[TauxDeCsg::Plein, TauxDeCsg::Reduit, TauxDeCsg::ExonereDeCsg, TauxDeCsg::ExonereDeCsgEtDeCrds];

    fn code(&self) -> R<String> {
        socle::liste::code_ecrit(self)
    }

    fn libelle(&self) -> R<&'static str> {
        Ok(match self {
            TauxDeCsg::Plein => "Taux plein (imposable)",
            TauxDeCsg::Reduit => "Taux réduit",
            TauxDeCsg::ExonereDeCsg => "Exonéré de CSG, la CRDS due",
            TauxDeCsg::ExonereDeCsgEtDeCrds => "Exonéré de CSG et de CRDS",
        })
    }
}

/// CE QUE L'OUVERTURE DES DROITS DONNE A L'ANNEE : l'annexe, l'allocation retenue (art. 14 a 16), le salaire de reference
/// et les heures travaillees qui l'ont faite, la fin de la periode de reference ; la date anniversaire de la periode
/// precedente (aucune : la personne n'en a pas dit) ; les salaires de toute la periode (aucun : une paie ne dit pas son
/// brut), ses jours travailles (aucun : une paie ne les dit pas), le SMIC horaire de son dernier jour (aucun : le bareme
/// livre ne le porte pas), et le taux de CSG de la personne (aucun : il ne joue pas, l'allocation brute ne passant pas le
/// plancher, `ContributionsSurLAllocation::jouent` ; au-dessus, l'annee ne se compte pas sans lui).
#[derive(Debug, Clone)]
pub struct OuvertureDesDroits {
    pub annexe: AnnexeChomage,
    pub allocation: Montant,
    pub salaire_de_reference: Montant,
    pub heures: Heures,
    pub fin: Jour,
    pub anniversaire: Option<Jour>,
    pub salaires_de_la_periode: Option<Montant>,
    pub jours_travailles: Option<Jours>,
    pub smic_horaire: Option<MontantUnitaire>,
    pub csg: Option<TauxDeCsg>,
}

/// LE TRAVAIL D'UN MOIS DE L'ANNEE (art. 30) : ses heures (un cachet pour les heures que le catalogue des rubriques lui
/// donne, l'enseignement compris), et son salaire brut (aucun : une paie du mois ne le dit pas).
#[derive(Debug, Clone)]
pub struct TravailDuMois {
    pub mois: Mois,
    pub heures: Heures,
    pub salaire: Option<Montant>,
}

/// L'ALLOCATION D'UN JOUR PAYE : la brute ; le salaire journalier moyen (art. 19) et la participation qu'il donne, exacts ;
/// ce qui reste apres elle ; la CSG et la CRDS ; et la nette, ce qu'un jour paye verse.
#[derive(Debug, Clone, PartialEq)]
pub struct AllocationDuJour {
    pub brute: Montant,
    pub salaire_journalier_moyen: MontantUnitaire,
    pub participation: Montant,
    pub apres_participation: Montant,
    pub contributions: Montant,
    pub nette: Montant,
}

impl AllocationDuJour {
    /// Ce que versent les jours payes `jours` (la nette), au centime : ce qu'un mois de ces jours touche, ou ce que des
    /// jours retires lui font perdre.
    pub fn pour_les_jours(&self, jours: Jours) -> Montant {
        pour(jours, self.nette).au_centime()
    }
}

/// UNE FRANCHISE, quand elle se compte (art. 21, art. 23 §2) : ses jours, et ce qu'elle prend par mois.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FranchiseComptee {
    pub jours: Jours,
    pub par_mois: Jours,
}

/// LA FRANCHISE DE SALAIRES, comptee : ses jours et sa part du mois, et le SMIC mensuel et journalier qui l'ont faite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FranchiseDeSalairesComptee {
    pub franchise: FranchiseComptee,
    pub smic_mensuel: Montant,
    pub smic_journalier: Montant,
}

/// CE QUI MANQUE A L'ANNEE pour se compter entiere : les jours travailles de la periode (la franchise de conges ne se
/// compte pas), ses salaires ou le SMIC (la franchise de salaires ne se compte pas), ce que les textes ne tirent pas de la
/// date anniversaire (le delai d'attente ne se compte pas), le salaire d'un mois travaille ou tout plafond de la Securite
/// sociale jusqu'a une annee (le plafond du cumul ne se compte pas ce mois-la).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManqueDeLAnnee {
    JoursTravailles,
    SalairesDeLaPeriode,
    Smic,
    DelaiDAttente,
    SalaireDuMois(Mois),
    PlafondDeLAnnee(Annee),
}

/// UN MOIS DE L'ANNEE (art. 30) : la part du mois que l'annee couvre ; les jours de travail que ses heures font, et ceux
/// qu'ils retirent ; s'il passe le seuil ; les jours indemnisables qui restent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiviteIndemnisee {
    pub jours_du_mois: Jours,
    pub heures: Heures,
    pub jours_de_travail: Jours,
    pub retires: Jours,
    pub au_dela_du_seuil: bool,
    pub indemnisables: Jours,
}

/// LE PLAFOND DU CUMUL D'UN MOIS (art. 30) : il ne joue pas (aucune activite, ou rien a verser) ; sinon le plafond, et
/// s'il a ramene l'allocation ; ou il ne se connait pas (le salaire du mois ou tout plafond de l'annee manquent).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlafondDuCumul {
    NeJouePas,
    SousLePlafond(Montant),
    Plafonne(Montant),
    Inconnu,
}

/// UN MOIS DE L'ANNEE D'INDEMNISATION : son activite, le plafond du cumul, ce que le delai d'attente et les franchises y
/// prennent, les jours payes, l'allocation brute, la participation et les contributions qu'elle retient, ce
/// qu'elle verse, et son salaire.
#[derive(Debug, Clone, PartialEq)]
pub struct MoisDeChomage {
    pub mois: Mois,
    pub activite: ActiviteIndemnisee,
    pub delai: Jours,
    pub conges: Jours,
    pub franchise_salaires: Jours,
    pub jours_payes: Jours,
    pub brute: Montant,
    pub participation: Montant,
    pub contributions: Montant,
    pub versee: Montant,
    pub cumul: PlafondDuCumul,
    pub salaire: Option<Montant>,
}

/// L'ANNEE D'INDEMNISATION : sa periode, l'allocation du jour, le delai d'attente et les franchises (aucun : il ou elle
/// ne se compte pas), ses mois, leurs totaux, ce qui lui manque (alors elle est « au plus »), et les plafonds de la
/// Securite sociale qu'elle a pris a une autre annee (l'annee du mois, celle du plafond pris).
#[derive(Debug, Clone, PartialEq)]
pub struct AnneeDeChomage {
    pub periode: Periode,
    pub jour: AllocationDuJour,
    pub delai: Option<Jours>,
    pub conges: Option<FranchiseComptee>,
    pub salaires: Option<FranchiseDeSalairesComptee>,
    pub mois: Vec<MoisDeChomage>,
    pub jours_payes: Jours,
    pub brute: Montant,
    pub participation: Montant,
    pub contributions: Montant,
    pub versee: Montant,
    pub manques: Vec<ManqueDeLAnnee>,
    pub plafonds_repris: Vec<(Annee, Annee)>,
}

impl AnneeDeChomage {
    /// Les jours de l'annee.
    pub fn jours(&self) -> Jours {
        Jours::de_dates(self.periode.jours().count())
    }

    /// Le salaire brut de ses mois, quand chaque mois travaille le dit.
    pub fn salaires(&self) -> Option<Montant> {
        self.mois.iter().try_fold(Montant::ZERO, |total, m| match (m.activite.heures.est_nul(), m.salaire) {
            (true, _) => Some(total),
            (false, Some(s)) => Some(total + s),
            (false, None) => None,
        })
    }
}

/// Des jours fois un montant par jour : un montant, exact.
fn pour(jours: Jours, par_jour: Montant) -> Montant {
    jours * par_jour.pour_une_unite()
}

/// Des jours, a l'entier inferieur.
fn entiers(j: Jours) -> R<Jours> {
    Ok(Jours::UN * j.rapport(Jours::UN).ok_or_else(|| socle::fichiers::absent("un jour"))?.entier())
}

/// LE PLAFOND MENSUEL DE LA SECURITE SOCIALE QUE L'ANNEE `an` PREND (art. 30) : le sien, quand le bareme livre le porte ;
/// sinon le dernier que le bareme livre porte avant elle (Léandre, 28/11/2026 : « on prend le dernier en date 2027 arrive
/// bientôt ») ; avec l'annee dont il est. Aucun : le bareme livre n'en porte aucun jusqu'a `an`.
pub fn plafond_mensuel_livre(an: Annee) -> R<Option<(Annee, Montant)>> {
    let livrees = crate::contexte::annees_du_socle(&socle::fichiers::dossier_du_code().join("baremes"))?;
    for a in livrees.into_iter().rev().filter(|a| *a <= an) {
        if let Some(p) = crate::contexte::socle_livre_lu(a)?.and_then(|p| p.plafond_securite_sociale) {
            return Ok(Some((a, p.mensuel)));
        }
    }
    Ok(None)
}

/// L'ALLOCATION D'UN JOUR PAYE (art. 19 ; circulaire Unedic 2026-06, fiche 2, § 7, exemple 20) : voir le module. La CSG
/// puis la CRDS se comparent au plancher sur l'allocation brute, diminuee de ce qui est deja pris (exemple 20 : « Base
/// d'exonération = 108,80 € », puis « Base d'exonération = ARE – CSG = 102,17 € »), et ne la font pas passer dessous.
pub fn allocation_du_jour(t: &Allocation, o: &OuvertureDesDroits) -> R<AllocationDuJour> {
    let a = t.activite(o.annexe);
    let jours = o.heures.en_jours(a.heures_par_jour).ok_or_else(|| socle::fichiers::absent("heures_par_jour"))?;
    let sjm = match o.salaire_de_reference.par_jour(jours) {
        Some(s) => s,
        None => return Err(socle::refus!("le salaire journalier moyen ne se calcule pas : aucune heure travaillée")),
    };
    let participation = sjm.fois_taux(t.participation.taux).fois(1);
    let apres_participation = (o.allocation - participation).max(t.aj_minimale.montant).min(o.allocation);
    let c = &t.contributions;
    let assiette = (o.allocation - o.allocation.fois_taux(c.abattement)).au_centime();
    // Ce qu'une contribution prend : son montant sur l'assiette, au plus ce qui depasse le plancher de la base.
    let prise = |taux: Option<Taux>, base: Montant| -> Montant {
        match taux {
            Some(x) => assiette.fois_taux(x).au_centime().min((base - c.plancher).max(Montant::ZERO)),
            None => Montant::ZERO,
        }
    };
    let contributions = match (o.csg, c.jouent(o.allocation)) {
        (_, false) => Montant::ZERO,
        (Some(taux), true) => {
            let csg = prise(taux.csg(c), o.allocation);
            csg + prise(taux.crds(c), o.allocation - csg)
        }
        (None, true) => return Err(socle::fichiers::absent("le taux de CSG de la personne")),
    };
    Ok(AllocationDuJour {
        brute: o.allocation,
        salaire_journalier_moyen: sjm,
        participation,
        apres_participation,
        contributions,
        nette: apres_participation - contributions,
    })
}

/// LES JOURS DE TRAVAIL QUE FONT LES HEURES `heures` sous l'activite `a` (art. 30).
fn jours_de_travail(a: &ActiviteReprise, heures: Heures) -> R<Jours> {
    heures.en_jours(a.heures_par_jour).ok_or_else(|| socle::fichiers::absent("heures_par_jour"))
}

/// LES JOURS INDEMNISABLES QUE RETIRENT LES HEURES `heures` d'un mois, sous l'annexe `annexe` (art. 30), sous le seuil.
pub fn retires(t: &Allocation, annexe: AnnexeChomage, heures: Heures) -> R<Jours> {
    let a = t.activite(annexe);
    Ok(jours_de_travail(a, heures)? * a.coefficient)
}

/// UN MOIS DE `jours_du_mois` JOURS (la part que l'annee en couvre) OU L'ON TRAVAILLE `heures` (art. 30) : voir
/// `ActiviteIndemnisee`.
pub fn activite_du_mois(t: &Allocation, annexe: AnnexeChomage, jours_du_mois: Jours, heures: Heures) -> R<ActiviteIndemnisee> {
    let a = t.activite(annexe);
    let jt = jours_de_travail(a, heures)?;
    let retires = jt * a.coefficient;
    let au_dela_du_seuil = jt >= a.jours_sans_allocation;
    let indemnisables = match au_dela_du_seuil {
        true => Jours::ZERO,
        false => (jours_du_mois - retires).max(Jours::ZERO),
    };
    Ok(ActiviteIndemnisee { jours_du_mois, heures, jours_de_travail: jt, retires, au_dela_du_seuil, indemnisables })
}

/// LES JOURS TRAVAILLES D'UN MOIS QU'AUCUNE AEM NE DIT, ENVIRON (Léandre, 28/11/2026 : « si les gens mettent les bulletins
/// on a tout sinon on fait approx ») : ses heures `heures` sous l'annexe `annexe`, a huit ou dix heures par jour, au
/// nombre entier obtenu, comme le guide compte les jours travailles d'une periode qu'aucune AEM n'atteste (p. 14,
/// legendes : « Pour l'annexe 8 : heures mensuelles travaillées de la période/8 », « Le résultat est arrondi au nombre
/// entier obtenu »). Le guide le dit d'un projet de transition professionnelle et d'une periode attestee par un U1 : pour
/// une autre periode, c'est une approximation, et l'appelant la dit telle.
pub fn jours_travailles_environ(t: &Allocation, annexe: AnnexeChomage, heures: Heures) -> R<Jours> {
    entiers(jours_de_travail(t.activite(annexe), heures)?)
}

/// LA FRANCHISE DE CONGES PAYES (art. 21 §1 a, art. 23 §2) pour `jours_travailles` jours de la periode.
fn franchise_de_conges(t: &Allocation, jours_travailles: Jours) -> R<FranchiseComptee> {
    let f = &t.franchise_conges;
    let part = jours_travailles.rapport(f.par_jours_travailles).ok_or_else(|| socle::fichiers::absent("par_jours_travailles"))?;
    let jours = entiers(f.jours_de_conges * part)?.min(f.plafond);
    let par_mois = if jours <= f.seuil_du_forfait { f.forfait } else { f.forfait_au_dela };
    Ok(FranchiseComptee { jours, par_mois })
}

/// LA FRANCHISE DE SALAIRES (art. 21 §1 b, art. 23 §2) : les salaires `salaires` de la periode, le salaire journalier
/// moyen `sjm`, le SMIC horaire `smic` du dernier jour de la periode, sur `mois` mois civils d'indemnisation (le guide,
/// p. 15 : « Franchise salaires totale / nombre de mois civils de la période d'indemnisation, limité à 8 mois ») ; une
/// annee sans mois civil ne se reparti pas : l'absence remonte.
fn franchise_de_salaires(t: &Allocation, salaires: Montant, sjm: MontantUnitaire, smic: MontantUnitaire, mois: usize) -> R<FranchiseDeSalairesComptee> {
    let f = &t.franchise_salaires;
    let semaine = f.heures_par_semaine * smic;
    let smic_mensuel = semaine.fois(f.semaines_par_an as usize).reparti_sur(f.mois_par_an as usize).au_centime();
    let smic_journalier = semaine.reparti_sur(f.jours_par_semaine as usize).au_centime();
    let absent = || socle::fichiers::absent("le SMIC de la franchise de salaires");
    let des_salaires = salaires.rapport(smic_mensuel).ok_or_else(absent)?;
    let du_sjm = sjm.fois(1).rapport(smic_journalier * f.smic_journaliers).ok_or_else(absent)?;
    let jours = entiers((Jours::UN * (des_salaires * du_sjm) - f.jours_deduits).max(Jours::ZERO))?;
    let sur = mois.min(f.mois_de_repartition as usize);
    if sur == 0 {
        return Err(socle::fichiers::absent("les mois civils de l'indemnisation"));
    }
    let par_mois = jours.reparties_sur(sur).a_l_entier_superieur();
    Ok(FranchiseDeSalairesComptee { franchise: FranchiseComptee { jours, par_mois }, smic_mensuel, smic_journalier })
}

/// CE QUE PREND UNE DEDUCTION de jours `du` sur les jours `j` qui restent au mois : au plus ce qu'il en reste ; `j` en
/// est diminue.
fn prendre(j: &mut Jours, du: Jours) -> Jours {
    let pris = (*j).min(du);
    *j -= pris;
    pris
}

/// LE DEBUT DE L'ANNEE ET SON DELAI D'ATTENTE (art. 9, 22, 23 §1 ; guide, exemple 9 ; voir le module) : le lendemain de la
/// fin de contrat, ou de la date anniversaire precedente quand elle vient apres ; le delai (aucun : il ne se tire pas des
/// textes).
fn debut_et_delai(t: &Allocation, o: &OuvertureDesDroits) -> R<(Jour, Option<Jours>)> {
    let lendemain_de_la_fin = o.fin.apres(Duree::jours(1))?;
    Ok(match o.anniversaire {
        None => (lendemain_de_la_fin, Some(t.delai_d_attente.jours)),
        Some(da) if da >= o.fin => (da.apres(Duree::jours(1))?, Some(Jours::ZERO)),
        Some(da) => match da.apres(Duree::mois(12))? < lendemain_de_la_fin {
            true => (lendemain_de_la_fin, Some(t.delai_d_attente.jours)),
            false => (lendemain_de_la_fin, None),
        },
    })
}

/// L'ANNEE D'INDEMNISATION de l'ouverture `o` (voir le module) : de son debut au terme des douze mois qui suivent sa fin
/// de contrat, avec le travail `travail` (vide : sans travail), mois par mois ; `pmss` dit le plafond mensuel de la
/// Securite sociale qu'une annee prend, et l'annee dont il est (`plafond_mensuel_livre` ; aucun : le bareme livre n'en
/// porte pas). Aucune annee : la date anniversaire precedente tombe au terme de ces douze mois, ou apres.
pub fn annee(t: &Allocation, o: &OuvertureDesDroits, travail: &[TravailDuMois], pmss: &dyn Fn(Annee) -> R<Option<(Annee, Montant)>>) -> R<Option<AnneeDeChomage>> {
    let (debut, delai_total) = debut_et_delai(t, o)?;
    let fin = o.fin.apres(Duree::mois(12))?;
    if debut > fin {
        return Ok(None);
    }
    let periode = Periode::du_au(debut, fin)?;
    let mois = periode.mois();
    let jour = allocation_du_jour(t, o)?;
    let mut manques = Vec::new();
    let mut plafonds_repris = Vec::new();
    if delai_total.is_none() {
        manques.push(ManqueDeLAnnee::DelaiDAttente);
    }
    let conges = match o.jours_travailles {
        Some(j) => Some(franchise_de_conges(t, j)?),
        None => {
            manques.push(ManqueDeLAnnee::JoursTravailles);
            None
        }
    };
    let salaires = match (o.salaires_de_la_periode, o.smic_horaire) {
        (Some(s), Some(smic)) => Some(franchise_de_salaires(t, s, jour.salaire_journalier_moyen, smic, mois.len())?),
        (None, _) => {
            manques.push(ManqueDeLAnnee::SalairesDeLaPeriode);
            None
        }
        (Some(_), None) => {
            manques.push(ManqueDeLAnnee::Smic);
            None
        }
    };
    // Ce qui reste de chaque deduction, et pour une franchise ce que les mois precedents n'ont pas pris (son reliquat).
    let mut delai = delai_total;
    let mut cp = conges.map(|f| (f.jours, Jours::ZERO));
    let mut fs = salaires.map(|f| (f.franchise.jours, Jours::ZERO));
    let mut out = Vec::new();
    for (k, m) in mois.iter().enumerate() {
        let dans = match Periode::du_mois(*m).croise(&periode) {
            Some(p) => p,
            None => continue,
        };
        let du_mois: Vec<&TravailDuMois> = travail.iter().filter(|x| x.mois == *m).collect();
        let heures: Heures = du_mois.iter().map(|x| x.heures).sum();
        let salaire = du_mois.iter().try_fold(Montant::ZERO, |total, x| x.salaire.map(|s| total + s));
        // Etapes 2 a 4 : l'activite, et l'allocation a verser.
        let activite = activite_du_mois(t, o.annexe, Jours::de_dates(dans.jours().count()), heures)?;
        let mut jours = activite.indemnisables;
        let mut a_verser = pour(jours, jour.brute).au_centime();
        // Etape 5 : le plafond du cumul.
        let cumul = match (heures.est_nul() || a_verser.est_nul(), salaire) {
            (true, _) => PlafondDuCumul::NeJouePas,
            (false, None) => {
                manques.push(ManqueDeLAnnee::SalaireDuMois(*m));
                PlafondDuCumul::Inconnu
            }
            (false, Some(s)) => match pmss(m.annee())? {
                None => {
                    if !manques.contains(&ManqueDeLAnnee::PlafondDeLAnnee(m.annee())) {
                        manques.push(ManqueDeLAnnee::PlafondDeLAnnee(m.annee()));
                    }
                    PlafondDuCumul::Inconnu
                }
                Some((de, p)) => {
                    if de != m.annee() && !plafonds_repris.contains(&(m.annee(), de)) {
                        plafonds_repris.push((m.annee(), de));
                    }
                    let plafond = (p * t.activite_reprise.plafond_du_cumul).au_centime();
                    match s + a_verser > plafond {
                        false => PlafondDuCumul::SousLePlafond(plafond),
                        true => {
                            a_verser = (plafond - s).max(Montant::ZERO);
                            let part = a_verser.rapport(jour.brute).ok_or_else(|| socle::fichiers::absent("l'allocation journalière"))?;
                            jours = (Jours::UN * part).a_l_entier_superieur();
                            PlafondDuCumul::Plafonne(plafond)
                        }
                    }
                }
            },
        };
        // Etape 6 : le delai d'attente, la franchise de conges du mois, celle de salaires du mois, puis le reliquat de
        // l'une et de l'autre.
        let pris_delai = match &mut delai {
            Some(reste) => {
                let pris = prendre(&mut jours, *reste);
                *reste -= pris;
                pris
            }
            None => Jours::ZERO,
        };
        let forfait_cp = conges.map(|f| f.par_mois);
        let part_cp = match (&cp, forfait_cp) {
            (Some((reste, reliquat)), Some(forfait)) => forfait.min(*reste - *reliquat),
            _ => Jours::ZERO,
        };
        // Apres les premiers mois de la repartition, ce qui reste de la franchise de salaires est du entier.
        let part_fs = match (&fs, salaires) {
            (Some((reste, reliquat)), Some(f)) => match k < t.franchise_salaires.mois_de_repartition as usize {
                true => f.franchise.par_mois.min(*reste - *reliquat),
                false => *reste - *reliquat,
            },
            _ => Jours::ZERO,
        };
        let cp_du_mois = prendre(&mut jours, part_cp);
        let fs_du_mois = prendre(&mut jours, part_fs);
        // Une franchise qui ne se compte pas ne prend rien (elle est dans `manques`).
        let cp_reliquat = match cp {
            Some((_, reliquat)) => prendre(&mut jours, reliquat),
            None => Jours::ZERO,
        };
        let fs_reliquat = match fs {
            Some((_, reliquat)) => prendre(&mut jours, reliquat),
            None => Jours::ZERO,
        };
        if let Some((reste, reliquat)) = &mut cp {
            *reste -= cp_du_mois + cp_reliquat;
            *reliquat = *reliquat - cp_reliquat + (part_cp - cp_du_mois);
        }
        if let Some((reste, reliquat)) = &mut fs {
            *reste -= fs_du_mois + fs_reliquat;
            *reliquat = *reliquat - fs_reliquat + (part_fs - fs_du_mois);
        }
        let jours_payes = jours;
        let brute = pour(jours_payes, jour.brute).au_centime().min(a_verser);
        // Ce que chaque jour paye retient, jamais plus que ce que le mois verse.
        let participation = pour(jours_payes, jour.participation).au_centime().min(brute);
        let apres = brute - participation;
        let contributions = pour(jours_payes, jour.contributions).au_centime().min(apres);
        let versee = apres - contributions;
        out.push(MoisDeChomage {
            mois: *m,
            activite,
            delai: pris_delai,
            conges: cp_du_mois + cp_reliquat,
            franchise_salaires: fs_du_mois + fs_reliquat,
            jours_payes,
            brute,
            participation,
            contributions,
            versee,
            cumul,
            salaire: if heures.est_nul() { None } else { salaire },
        });
    }
    let jours_payes = out.iter().map(|m| m.jours_payes).sum();
    let brute = out.iter().map(|m| m.brute).sum();
    let participation = out.iter().map(|m| m.participation).sum();
    let contributions = out.iter().map(|m| m.contributions).sum();
    let versee = out.iter().map(|m| m.versee).sum();
    Ok(Some(AnneeDeChomage {
        periode,
        jour,
        delai: delai_total,
        conges,
        salaires,
        mois: out,
        jours_payes,
        brute,
        participation,
        contributions,
        versee,
        manques,
        plafonds_repris,
    }))
}
