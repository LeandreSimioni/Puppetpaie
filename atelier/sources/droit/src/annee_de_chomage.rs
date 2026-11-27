//! L'ANNEE D'INDEMNISATION D'UN INTERMITTENT, ESTIMEE MOIS PAR MOIS (annexes VIII et X au reglement d'assurance chomage,
//! `atelier/reference/sources/plafond-intermittents/unedic-annexe8-convention-20241115.txt`, `…annexe10…` ; le guide de
//! France Travail, `chomage-intermittents/francetravail-guide-intermittents-20261020.txt`, p. 11 a 17) : l'annee qui suit
//! la fin de contrat de l'ouverture des droits, jusqu'a sa date anniversaire (art. 9), sans travail ou avec le travail que
//! l'appelant dit, mois par mois.
//!
//! - L'ALLOCATION DU JOUR (`AllocationDuJour`) : l'allocation retenue (art. 14 a 16), moins la participation de 0,93 % du
//!   salaire journalier moyen (art. 19), sans passer sous l'allocation minimale ; moins la CSG et la CRDS, sur l'allocation
//!   brute abattue de 1,75 %, sans faire passer l'allocation versee sous 62 € (circulaire Unedic 2026-06, fiche 2, § 7 ;
//!   `chomage-permanents/unedic-circulaire-2026-06-reglementation-20260901.txt`). Le taux de CSG est celui de la personne
//!   (`TauxDeCsg`) : inconnu, l'allocation nette ne se dit pas.
//! - LES FRANCHISES ET LE DELAI D'ATTENTE (art. 21 a 23), dans l'ordre du texte : le delai d'attente, la franchise de
//!   conges payes, puis celle de salaires, chacun sur les jours indemnisables du mois, ce qu'un mois n'a pas pris passant
//!   au suivant. La franchise de conges compte les jours travailles de l'AEM ; celle de salaires, les salaires de la
//!   periode de reference et le SMIC de son dernier jour. Ce qui manque pour l'une ou l'autre ne devient pas zero : elle
//!   ne se deduit pas, et l'annee le dit (`Manque`) : son chomage est alors « au plus ».
//! - UN MOIS TRAVAILLE (art. 30 ; `activite_du_mois`) : ses heures font des jours de travail, qui retirent chacun un
//!   coefficient de jours indemnisables ; au seuil de l'annexe, rien n'est verse ; le salaire et l'allocation brute du mois
//!   ne passent pas 1,18 fois le plafond mensuel de la Securite sociale, et l'allocation est alors la difference, ses jours
//!   arrondis a l'entier superieur. Le plafond du cumul se compte apres les franchises : le guide (p. 16) le verifie sur
//!   « l'ARE a verser » ; aucun texte depose ne dit cet ordre en toutes lettres.
//!
//! Les jours de travail, les jours retires et les jours payes d'un mois ne s'arrondissent pas : seuls le plafond du cumul
//! (art. 30) et les franchises (leur total, leur part du mois) disent un arrondi. Les valeurs et leurs textes sont dans
//! `allocation-intermittents.json` (`droit::allocation`).

use crate::allocation::{ActiviteReprise, Allocation};
use crate::emplois::AnnexeChomage;
use socle::date::{Duree, Jour, Periode};
use socle::echec::R;
use socle::grandeurs::{Heures, Jours, Montant, MontantUnitaire};
use socle::mois::{Annee, Mois};

/// LE TAUX DE CSG DE LA PERSONNE sur son allocation (circulaire Unedic 2026-06, fiche 2, § 7) : le taux plein, le taux
/// reduit, ou l'exoneration totale (de la CSG et de la CRDS), selon son revenu fiscal de reference ; seule la personne le
/// sait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TauxDeCsg {
    Plein,
    Reduit,
    Exonere,
}

/// Le taux de CSG est une liste fermee : la charge l'ecrit par son code (`plein`…), l'ecran la fait choisir.
impl socle::liste::ListeFermee for TauxDeCsg {
    const TOUTES: &'static [TauxDeCsg] = &[TauxDeCsg::Plein, TauxDeCsg::Reduit, TauxDeCsg::Exonere];

    fn code(&self) -> R<String> {
        socle::liste::code_ecrit(self)
    }

    fn libelle(&self) -> R<&'static str> {
        Ok(match self {
            TauxDeCsg::Plein => "Taux plein (imposable)",
            TauxDeCsg::Reduit => "Taux réduit",
            TauxDeCsg::Exonere => "Exonéré",
        })
    }
}

/// CE QUE L'OUVERTURE DES DROITS DONNE A L'ANNEE : l'annexe, l'allocation retenue (art. 14 a 16), le salaire de reference
/// et les heures travaillees qui l'ont faite, la fin de la periode de reference ; les salaires de toute la periode
/// (aucun : une paie ne dit pas son brut), ses jours travailles (aucun : une paie ne les dit pas), le SMIC horaire de son
/// dernier jour (aucun : le bareme livre ne le porte pas), et le taux de CSG de la personne (aucun : elle ne l'a pas dit).
#[derive(Debug, Clone)]
pub struct OuvertureDesDroits {
    pub annexe: AnnexeChomage,
    pub allocation: Montant,
    pub salaire_de_reference: Montant,
    pub heures: Heures,
    pub fin: Jour,
    pub salaires_de_la_periode: Option<Montant>,
    pub jours_travailles: Option<Jours>,
    pub smic_horaire: Option<MontantUnitaire>,
    pub csg: Option<TauxDeCsg>,
}

/// LE TRAVAIL D'UN MOIS DE L'ANNEE (art. 30) : ses heures (un cachet pour les heures que la table lui donne, l'enseignement
/// compris), et son salaire brut (aucun : une paie du mois ne le dit pas).
#[derive(Debug, Clone)]
pub struct TravailDuMois {
    pub mois: Mois,
    pub heures: Heures,
    pub salaire: Option<Montant>,
}

/// L'ALLOCATION D'UN JOUR PAYE : la brute ; le salaire journalier moyen (art. 19) et la participation qu'il donne ; ce qui
/// reste apres elle ; la CSG et la CRDS (aucune : le taux de la personne ne se connait pas et elles pourraient jouer) ; et
/// la nette.
#[derive(Debug, Clone, PartialEq)]
pub struct AllocationDuJour {
    pub brute: Montant,
    pub salaire_journalier_moyen: MontantUnitaire,
    pub participation: Montant,
    pub apres_participation: Montant,
    pub contributions: Option<Montant>,
    pub nette: Option<Montant>,
}

impl AllocationDuJour {
    /// Ce qu'un jour paye verse : la nette quand elle se connait, sinon ce qui reste apres la participation ; et si c'est
    /// la nette.
    pub fn versee(&self) -> (Montant, bool) {
        match self.nette {
            Some(n) => (n, true),
            None => (self.apres_participation, false),
        }
    }

    /// Ce que versent les jours payes `jours` (`versee`), au centime : ce qu'un mois de ces jours touche, ou ce que des
    /// jours retires lui font perdre.
    pub fn pour_les_jours(&self, jours: Jours) -> Montant {
        pour(jours, self.versee().0).au_centime()
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
/// compte pas), ses salaires ou le SMIC (la franchise de salaires ne se compte pas), le salaire d'un mois travaille ou le
/// plafond de la Securite sociale d'une annee (le plafond du cumul ne se compte pas ce mois-la).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Manque {
    JoursTravailles,
    SalairesDeLaPeriode,
    Smic,
    SalaireDuMois(Mois),
    PlafondDeLAnnee(Annee),
}

/// UN MOIS DE L'ANNEE (art. 30) : la part du mois que l'annee couvre ; les jours de travail que ses heures font, et ceux
/// qu'ils retirent ; s'il passe le seuil ; les jours indemnisables qui restent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiviteDuMois {
    pub jours_du_mois: Jours,
    pub heures: Heures,
    pub jours_de_travail: Jours,
    pub retires: Jours,
    pub au_dela_du_seuil: bool,
    pub indemnisables: Jours,
}

/// LE PLAFOND DU CUMUL D'UN MOIS (art. 30) : il ne joue pas (aucune activite, ou rien a verser) ; sinon le plafond, et
/// s'il a ramene l'allocation ; ou il ne se connait pas (le salaire du mois ou le plafond de l'annee manquent).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cumul {
    NeJouePas,
    SousLePlafond(Montant),
    Plafonne(Montant),
    Inconnu,
}

/// UN MOIS DE L'ANNEE D'INDEMNISATION : son activite, ce que le delai d'attente et les franchises y prennent, les jours
/// payes, l'allocation brute, la participation et les contributions qu'elle retient (aucune : le taux de CSG ne se
/// connait pas), ce qu'elle verse (`AllocationDuJour::versee`), le plafond du cumul, et son salaire.
#[derive(Debug, Clone, PartialEq)]
pub struct MoisDeChomage {
    pub mois: Mois,
    pub activite: ActiviteDuMois,
    pub delai: Jours,
    pub conges: Jours,
    pub franchise_salaires: Jours,
    pub jours_payes: Jours,
    pub brute: Montant,
    pub participation: Montant,
    pub contributions: Option<Montant>,
    pub versee: Montant,
    pub cumul: Cumul,
    pub salaire: Option<Montant>,
}

/// L'ANNEE D'INDEMNISATION : sa periode, l'allocation du jour, le delai d'attente et les franchises (aucune : elle ne se
/// compte pas), ses mois, leurs totaux, et ce qui lui manque (alors elle est « au plus »).
#[derive(Debug, Clone, PartialEq)]
pub struct AnneeDeChomage {
    pub periode: Periode,
    pub jour: AllocationDuJour,
    pub delai: Jours,
    pub conges: Option<FranchiseComptee>,
    pub salaires: Option<FranchiseDeSalairesComptee>,
    pub mois: Vec<MoisDeChomage>,
    pub jours_payes: Jours,
    pub brute: Montant,
    pub participation: Montant,
    pub contributions: Option<Montant>,
    pub versee: Montant,
    pub manques: Vec<Manque>,
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

/// L'ALLOCATION D'UN JOUR PAYE (art. 19 ; circulaire Unedic 2026-06, fiche 2, § 7) : voir le module.
pub fn allocation_du_jour(t: &Allocation, o: &OuvertureDesDroits) -> R<AllocationDuJour> {
    let a = t.activite(o.annexe);
    let jours = o.heures.en_jours(a.heures_par_jour).ok_or_else(|| socle::fichiers::absent("heures_par_jour"))?;
    let sjm = match o.salaire_de_reference.par_jour(jours) {
        Some(s) => s.au_centime(),
        None => return Err(socle::refus!("le salaire journalier moyen ne se calcule pas : aucune heure travaillée")),
    };
    let participation = sjm.fois_taux(t.participation.taux).au_centime().fois(1);
    let apres_participation = (o.allocation - participation).max(t.aj_minimale.montant).min(o.allocation);
    let c = &t.contributions;
    // Ce que la CSG et la CRDS peuvent prendre au plus : ce qui depasse le plancher de l'allocation versee.
    let au_plus = (apres_participation - c.plancher).max(Montant::ZERO);
    let assiette = o.allocation - o.allocation.fois_taux(c.abattement);
    let prises = |taux_csg: Option<socle::grandeurs::Taux>| -> Montant {
        match taux_csg {
            Some(csg) => (assiette.fois_taux(csg).au_centime() + assiette.fois_taux(c.crds).au_centime()).min(au_plus),
            None => Montant::ZERO,
        }
    };
    let contributions = match (o.csg, au_plus.est_nul()) {
        (_, true) => Some(Montant::ZERO),
        (Some(TauxDeCsg::Plein), false) => Some(prises(Some(c.csg))),
        (Some(TauxDeCsg::Reduit), false) => Some(prises(Some(c.csg_reduite))),
        (Some(TauxDeCsg::Exonere), false) => Some(prises(None)),
        (None, false) => None,
    };
    Ok(AllocationDuJour {
        brute: o.allocation,
        salaire_journalier_moyen: sjm,
        participation,
        apres_participation,
        contributions,
        nette: contributions.map(|x| apres_participation - x),
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
/// `ActiviteDuMois`.
pub fn activite_du_mois(t: &Allocation, annexe: AnnexeChomage, jours_du_mois: Jours, heures: Heures) -> R<ActiviteDuMois> {
    let a = t.activite(annexe);
    let jt = jours_de_travail(a, heures)?;
    let retires = jt * a.coefficient;
    let au_dela_du_seuil = jt >= a.jours_sans_allocation;
    let indemnisables = match au_dela_du_seuil {
        true => Jours::ZERO,
        false => (jours_du_mois - retires).max(Jours::ZERO),
    };
    Ok(ActiviteDuMois { jours_du_mois, heures, jours_de_travail: jt, retires, au_dela_du_seuil, indemnisables })
}

/// LA FRANCHISE DE CONGES PAYES (art. 21 §1 a, art. 23 §2) pour `jours_travailles` jours de l'AEM.
fn franchise_de_conges(t: &Allocation, jours_travailles: Jours) -> R<FranchiseComptee> {
    let f = &t.franchise_conges;
    let part = jours_travailles.rapport(f.par_jours_travailles).ok_or_else(|| socle::fichiers::absent("par_jours_travailles"))?;
    let jours = entiers(f.jours_de_conges * part)?.min(f.plafond);
    let par_mois = if jours <= f.seuil_du_forfait { f.forfait } else { f.forfait_au_dela };
    Ok(FranchiseComptee { jours, par_mois })
}

/// LA FRANCHISE DE SALAIRES (art. 21 §1 b, art. 23 §2) : les salaires `salaires` de la periode, le salaire journalier
/// moyen `sjm`, le SMIC horaire `smic` du dernier jour de la periode, sur `mois` mois civils d'indemnisation.
fn franchise_de_salaires(t: &Allocation, salaires: Montant, sjm: MontantUnitaire, smic: MontantUnitaire, mois: usize) -> R<FranchiseDeSalairesComptee> {
    let f = &t.franchise_salaires;
    let semaine = f.heures_par_semaine * smic;
    let smic_mensuel = semaine.fois(f.semaines_par_an as usize).reparti_sur(f.mois_par_an as usize).au_centime();
    let smic_journalier = semaine.reparti_sur(f.jours_par_semaine as usize).au_centime();
    let absent = || socle::fichiers::absent("le SMIC de la franchise de salaires");
    let des_salaires = salaires.rapport(smic_mensuel).ok_or_else(absent)?;
    let du_sjm = sjm.fois(1).rapport(smic_journalier * f.smic_journaliers).ok_or_else(absent)?;
    let jours = entiers((Jours::UN * (des_salaires * du_sjm) - f.jours_deduits).max(Jours::ZERO))?;
    let sur = mois.min(f.mois_de_repartition as usize).max(1);
    let par_mois = jours.reparties_sur(sur).a_l_entier_superieur();
    Ok(FranchiseDeSalairesComptee { franchise: FranchiseComptee { jours, par_mois }, smic_mensuel, smic_journalier })
}

/// CE QU'UNE FRANCHISE A ENCORE A PRENDRE : ce qui en reste, et ce qui est du ce mois-ci (sa part, et ce que les mois
/// precedents n'ont pas pris).
struct AConsommer {
    reste: Jours,
    du: Jours,
}

impl AConsommer {
    /// Ce qu'elle prend des jours `indemnisables` du mois, sa part `part` y ajoutee : ce qu'elle doit, au plus ce qui
    /// reste.
    fn prendre(&mut self, part: Jours, indemnisables: Jours) -> Jours {
        self.du = (self.du + part).min(self.reste);
        let pris = indemnisables.min(self.du);
        self.du -= pris;
        self.reste -= pris;
        pris
    }
}

/// L'ANNEE D'INDEMNISATION de l'ouverture `o` (voir le module) : du lendemain de sa fin de contrat a sa date anniversaire,
/// avec le travail `travail` (vide : sans travail), mois par mois ; `pmss` dit le plafond mensuel de la Securite sociale
/// d'une annee (aucun : le bareme livre ne le porte pas).
pub fn annee(t: &Allocation, o: &OuvertureDesDroits, travail: &[TravailDuMois], pmss: &dyn Fn(Annee) -> R<Option<Montant>>) -> R<AnneeDeChomage> {
    let periode = Periode::du_au(o.fin.apres(Duree::jours(1))?, o.fin.apres(Duree::mois(12))?)?;
    let mois = periode.mois();
    let jour = allocation_du_jour(t, o)?;
    let mut manques = Vec::new();
    let conges = match o.jours_travailles {
        Some(j) => Some(franchise_de_conges(t, j)?),
        None => {
            manques.push(Manque::JoursTravailles);
            None
        }
    };
    let salaires = match (o.salaires_de_la_periode, o.smic_horaire) {
        (Some(s), Some(smic)) => Some(franchise_de_salaires(t, s, jour.salaire_journalier_moyen, smic, mois.len())?),
        (None, _) => {
            manques.push(Manque::SalairesDeLaPeriode);
            None
        }
        (Some(_), None) => {
            manques.push(Manque::Smic);
            None
        }
    };
    let delai_total = t.delai_d_attente.jours;
    let mut delai = AConsommer { reste: delai_total, du: Jours::ZERO };
    let mut cp = conges.map(|f| (AConsommer { reste: f.jours, du: Jours::ZERO }, f.par_mois));
    let mut fs = salaires.map(|f| (AConsommer { reste: f.franchise.jours, du: Jours::ZERO }, f.franchise.par_mois));
    let mut out = Vec::new();
    for (k, m) in mois.iter().enumerate() {
        let dans = match Periode::du_mois(*m).croise(&periode) {
            Some(p) => p,
            None => continue,
        };
        let du_mois: Vec<&TravailDuMois> = travail.iter().filter(|x| x.mois == *m).collect();
        let heures: Heures = du_mois.iter().map(|x| x.heures).sum();
        let salaire = du_mois.iter().try_fold(Montant::ZERO, |total, x| x.salaire.map(|s| total + s));
        let activite = activite_du_mois(t, o.annexe, Jours::de_dates(dans.jours().count()), heures)?;
        let mut indemnisables = activite.indemnisables;
        let pris_delai = delai.prendre(delai_total, indemnisables);
        indemnisables -= pris_delai;
        let pris_cp = match &mut cp {
            Some((c, part)) => c.prendre(*part, indemnisables),
            None => Jours::ZERO,
        };
        indemnisables -= pris_cp;
        let pris_fs = match &mut fs {
            // Apres les premiers mois de la repartition, ce qui reste de la franchise est du entier.
            Some((c, part)) => match k < t.franchise_salaires.mois_de_repartition as usize {
                true => c.prendre(*part, indemnisables),
                false => {
                    let reste = c.reste;
                    c.prendre(reste, indemnisables)
                }
            },
            None => Jours::ZERO,
        };
        indemnisables -= pris_fs;
        let mut jours_payes = indemnisables;
        let mut brute = pour(indemnisables, jour.brute).au_centime();
        let cumul = match (heures.est_nul() || brute.est_nul(), salaire) {
            (true, _) => Cumul::NeJouePas,
            (false, None) => {
                manques.push(Manque::SalaireDuMois(*m));
                Cumul::Inconnu
            }
            (false, Some(s)) => match pmss(m.annee())? {
                None => {
                    if !manques.contains(&Manque::PlafondDeLAnnee(m.annee())) {
                        manques.push(Manque::PlafondDeLAnnee(m.annee()));
                    }
                    Cumul::Inconnu
                }
                Some(p) => {
                    let plafond = (p * t.activite_reprise.plafond_du_cumul).au_centime();
                    match s + brute > plafond {
                        false => Cumul::SousLePlafond(plafond),
                        true => {
                            brute = (plafond - s).max(Montant::ZERO);
                            let part = brute.rapport(jour.brute).ok_or_else(|| socle::fichiers::absent("l'allocation journalière"))?;
                            jours_payes = (Jours::UN * part).a_l_entier_superieur();
                            Cumul::Plafonne(plafond)
                        }
                    }
                }
            },
        };
        // Ce que chaque jour paye retient, jamais plus que ce que le mois verse.
        let participation = pour(jours_payes, jour.participation).au_centime().min(brute);
        let apres = brute - participation;
        let contributions = jour.contributions.map(|x| pour(jours_payes, x).au_centime().min(apres));
        let versee = match contributions {
            Some(x) => apres - x,
            None => apres,
        };
        out.push(MoisDeChomage {
            mois: *m,
            activite,
            delai: pris_delai,
            conges: pris_cp,
            franchise_salaires: pris_fs,
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
    Ok(AnneeDeChomage { periode, jour, delai: delai_total, conges, salaires, mois: out, jours_payes, brute, participation, contributions, versee, manques })
}
