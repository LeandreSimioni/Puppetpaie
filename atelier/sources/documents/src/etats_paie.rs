//! Les etats de paie de l'onglet Exports : la periode, le livre de paie et ses trois ecrivains (CSV, XLSX, PDF), le livre
//! detaille, le recapitulatif annuel, la masse salariale, les virements, le prelevement a la source, la reduction
//! generale, les assiettes, les heures et cachets, le cout par spectacle.
//!
//! UN ETAT LIT LES PAIES DU MOIS (`engagement::etats::paies_du_mois`), par un seul chemin : les paies ecrites des
//! engagements valides, et celles des engagements importes, dans les mots du moteur (ce que les pieces de l'ancien
//! logiciel en disent : `engagement::importe::Importe::paie`). Un engagement en saisie n'est pas paye, il n'y entre pas ;
//! la periode couvre les mois que des engagements touchent. Le PAS se lit par `prelevement::des_paies` ; aucun numero
//! n'est consomme. Un montant que la paie ne dit pas (`PaieDuMois`) est VIDE, pas zero, et le total ne le compte pas.

use droit::assiette::Assiette;
use droit::fiche::Lue as _;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use socle::fichiers::NomDeFichier;
use std::path::PathBuf;
use std::rc::Rc;

use crate::classeur::{Feuille, Style, Vertical, CORPS_DU_CLASSEUR};
use crate::tableaux_export::Rangee;
use serde::Serialize;

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::echec::{erreur, Classe};
use socle::date::Jour;
use socle::mois::{Annee, Mois, PeriodeDemandee};
use socle::grandeurs::{nulle, Grandeur, Montant};
use engagement::ligne::PaiementDeLigne;
use crate::prelevement::Prelevement;
use crate::plan_comptable::{EtatDExport, FormatDExport, LogicielComptable, RegroupementDEtat};
use engagement::etats::{PaieDuMois, RemunerationDuMois};
use crate::tableaux_export::{
    ecrire as ecrire_tableau, format_possible, jj, libelle_mois, qui, refus, sans_mois, section_du_mois, sections_de, totaliser, colonne as C, ColonneDEtat, ColonneDeMontant, ColonneDeTexte, Ligne,
    Section, Tableau, FORMATS_TABLEAU, R, TOTAL_GENERAL,
};

/// D'OU VIENNENT LES PAIES D'UN MOIS DES ETATS. Elle ne s'ecrit nulle part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrigineDuMois {
    /// Un mois que la saisie touche.
    Saisie,
    /// Un mois que seuls des engagements importes touchent : l'ancien logiciel l'a paye.
    Importe,
}

/// Ce qu'un etat demande : la periode (une seule des trois), le format, ce qui ne sort que sur demande.
#[derive(Clone)]
pub struct Demande {
    pub mois: Option<Mois>,
    pub annee: Option<Annee>,
    pub toutes: bool,
    /// Le regroupement, quand l'etat en demande un.
    pub regroupement: Option<RegroupementDEtat>,
    /// Le format, quand l'etat en a.
    pub format: Option<FormatDExport>,
    pub coordonnees: bool,
    pub ecrire: bool,
    pub logiciel: Option<LogicielComptable>,
    pub analytique: Option<bool>,
    /// Ce qui est parti, que le moteur lit (`envois::journal::departs`) : le prelevement d'une paie declaree se lit dans la
    /// paie que la DSN a declaree (`prelevement::des_paies`).
    pub departs: engagement::departs::Departs,
}


// ------------------------------------------------------------------ ou ca s'ecrit

/// `exports/`, et le sous-dossier de l'annee quand il y en a une.
pub fn dossier_exports(ctx: &Contexte, annee: Option<Annee>) -> PathBuf {
    let d = crate::DossierDeTravail::Exports.sous(&ctx.racine);
    match annee {
        Some(a) => d.join(a.to_string()),
        None => d,
    }
}

// ------------------------------------------------------------------ la periode

/// Le dossier des exports de la periode `quand` : `exports/` (toutes les annees) ou `exports/AAAA/`.
fn dossier_de(ctx: &Contexte, quand: &PeriodeDemandee) -> PathBuf {
    match quand {
        PeriodeDemandee::Toutes => dossier_exports(ctx, None),
        PeriodeDemandee::Mois(m) => dossier_exports(ctx, Some(Annee::du_mois(*m))),
        PeriodeDemandee::Annee(a) => dossier_exports(ctx, Some(*a)),
    }
}

const UNE_PERIODE: &str = "Un état se demande pour UNE période : un mois (AAAA-MM), une année (AAAA), ou toutes les années (toutes).";

/// Ce qu'on a demande, les mois qu'on a, et d'ou ils viennent.
pub struct Periode {
    pub dispo: BTreeMap<Mois, OrigineDuMois>,
    pub quand: PeriodeDemandee,
    pub mois: Vec<Mois>,
    pub libelle: String,
}

/// Les mois qu'il y a matiere a exporter : ceux que la saisie touche (`Saisie`), et ceux ou seuls des engagements
/// importes ont ete payes (`Importe`).
pub fn mois_disponibles(ctx: &Contexte) -> R<BTreeMap<Mois, OrigineDuMois>> {
    let mois = engagement::stock::mois_saisis(&ctx.racine).map_err(|m| m.reclasser(Classe::Illisible))?;
    let mut dispo: BTreeMap<Mois, OrigineDuMois> = mois_importes(ctx)?.into_iter().map(|m| (m, OrigineDuMois::Importe)).collect();
    dispo.extend(mois.into_iter().map(|m| (m, OrigineDuMois::Saisie)));
    Ok(dispo)
}

/// Les mois ou un engagement importe a ete paye : ce que l'import a lu de ses paies (`engagement::importe`).
pub fn mois_importes(ctx: &Contexte) -> R<BTreeSet<Mois>> {
    let tous = engagement::importe::tous(&ctx.racine).map_err(|m| m.reclasser(Classe::Illisible))?;
    Ok(tous.iter().flat_map(|i| i.paies().keys().copied()).collect())
}

/// Le libelle de la periode d'un etat : un mois, une annee, ou toutes les annees (de la premiere a la derniere de
/// `mois`, quand il y en a).
fn libelle_periode(mois: &[Mois], quand: &PeriodeDemandee) -> String {
    match quand {
        PeriodeDemandee::Mois(m) => libelle_mois(*m),
        PeriodeDemandee::Annee(a) => a.to_string(),
        PeriodeDemandee::Toutes => {
            let annees: BTreeSet<Annee> = mois.iter().map(|m| m.annee()).collect();
            match (annees.first(), annees.last()) {
                (Some(p), Some(d)) if p != d => format!("toutes les années ({} à {})", p, d),
                (Some(p), _) => format!("toutes les années ({})", p),
                _ => "toutes les années".to_string(),
            }
        }
    }
}

impl Periode {
    /// La periode d'un etat : un mois, une annee, ou toutes les annees. `obligatoire` : ses mois sont ceux que la compagnie
    /// a (`mois_pris`), sinon ceux que la demande nomme.
    pub fn nouvelle(ctx: &Contexte, mois: Option<Mois>, annee: Option<Annee>, toutes: bool, obligatoire: bool) -> R<Periode> {
        Periode::pour(ctx, PeriodeDemandee::des_trois(mois, annee, toutes, UNE_PERIODE)?, obligatoire)
    }

    /// La periode `quand`, et les mois qu'il y a matiere a exporter.
    pub fn pour(ctx: &Contexte, quand: PeriodeDemandee, obligatoire: bool) -> R<Periode> {
        let dispo = mois_disponibles(ctx)?;
        let pris: Vec<Mois> = if obligatoire {
            mois_pris(&dispo, &quand)?
        } else {
            match &quand {
                PeriodeDemandee::Mois(m) => vec![*m],
                autre => dispo.keys().copied().filter(|m| autre.contient(*m)).collect(),
            }
        };
        let libelle = libelle_periode(&pris, &quand);
        Ok(Periode { dispo, quand, mois: pris, libelle })
    }

    /// La periode d'une demande (`mois`, `annee`, `toutes`).
    pub fn de(ctx: &Contexte, d: &Demande, obligatoire: bool) -> R<Periode> {
        Periode::nouvelle(ctx, d.mois, d.annee, d.toutes, obligatoire)
    }

    pub fn saisis(&self) -> Vec<Mois> {
        self.mois.iter().copied().filter(|m| self.dispo.get(m) == Some(&OrigineDuMois::Saisie)).collect()
    }

    /// Les mois de la periode que seul l'import a payes.
    pub fn importes(&self) -> Vec<Mois> {
        self.mois.iter().copied().filter(|m| self.dispo.get(m) == Some(&OrigineDuMois::Importe)).collect()
    }

    pub fn contient(&self, m: Mois) -> bool {
        self.quand.contient(m)
    }

    /// `Periode.nom` : ce qui entre dans le nom du fichier.
    pub fn nom(&self) -> NomDeFichier {
        self.quand.nom()
    }

    /// `Periode.dossier` : `exports/` (toutes les annees) ou `exports/AAAA/`.
    pub fn dossier(&self, ctx: &Contexte) -> PathBuf {
        dossier_de(ctx, &self.quand)
    }
}

/// Les mois demandes qu'on sait exporter -- et le refus qui dit lequel manque.
fn mois_pris(dispo: &BTreeMap<Mois, OrigineDuMois>, quand: &PeriodeDemandee) -> R<Vec<Mois>> {
    match quand {
        PeriodeDemandee::Toutes => {
            if dispo.is_empty() {
                return Err(refus("Aucun engagement : il n'y a pas de livre de paie à faire."));
            }
            Ok(dispo.keys().copied().collect())
        }
        PeriodeDemandee::Mois(m) => {
            if !dispo.contains_key(m) {
                return Err(refus(format!("Aucune paie en {} : aucun engagement ne touche ce mois.", m)));
            }
            Ok(vec![*m])
        }
        PeriodeDemandee::Annee(_) => {
            let pris: Vec<Mois> = dispo.keys().copied().filter(|m| quand.contient(*m)).collect();
            if pris.is_empty() {
                return Err(refus(format!("Aucune paie en {} : aucun engagement ne touche cette année.", quand.nom())));
            }
            Ok(pris)
        }
    }
}

// ------------------------------------------------------------------ les lectures

/// La saisie de `mois` reduite a ses engagements valides, et leurs paies telles qu'elles ont ete validees : ce que
/// lisent les etats et les exports.
pub fn charger_mois(ctx: &Contexte, mois: Mois, departs: &engagement::departs::Departs) -> R<(engagement::saisie::VueDuMois, Vec<Paie>)> {
    engagement::etats::validees_du_mois(ctx, mois, departs)
        .map_err(|m| m.reclasser(Classe::Refuse))?
        .ok_or_else(|| erreur(Classe::Refuse, format!("{} : aucun engagement ne touche ce mois.", mois)))
}

/// Les paies validees d'un mois, lues une fois par export.
pub struct Lecture<'a> {
    ctx: &'a Contexte,
    departs: &'a engagement::departs::Departs,
    paies: HashMap<Mois, Rc<Vec<PaieDuMois>>>,
    pas: HashMap<Mois, Rc<Vec<Option<Prelevement>>>>,
    /// Les paies des mois lus mises de cote (`engagement::etats::PaieDeCote`), dans l'ordre lu.
    a_dire: Vec<engagement::etats::PaieDeCote>,
}

impl<'a> Lecture<'a> {
    pub fn nouvelle(ctx: &'a Contexte, departs: &'a engagement::departs::Departs) -> Self {
        Lecture { ctx, departs, paies: HashMap::new(), pas: HashMap::new(), a_dire: Vec::new() }
    }

    /// Les paies du mois, calculees ou importees (`engagement::etats::paies_du_mois`) ; celles qui ne se rangent pas se
    /// disent (`avertissements`).
    pub fn paies(&mut self, mois: Mois) -> R<Rc<Vec<PaieDuMois>>> {
        if !self.paies.contains_key(&mois) {
            let (paies, a_dire) = engagement::etats::paies_du_mois(self.ctx, mois, self.departs).map_err(|m| m.reclasser(Classe::Refuse))?;
            self.a_dire.extend(a_dire);
            self.paies.insert(mois, Rc::new(paies));
        }
        Ok(Rc::clone(&self.paies[&mois]))
    }

    /// CE QUE L'ETAT DIT DES PAIES QU'IL N'A PAS COMPTEES : une phrase par paie des mois lus mise de cote
    /// (`engagement::etats::PaieDeCote::phrase`). L'etat repond avec le reste.
    pub fn avertissements(&self) -> Vec<String> {
        self.a_dire().iter().map(engagement::etats::PaieDeCote::phrase).collect()
    }

    /// Les paies des mois lus mises de cote.
    pub fn a_dire(&self) -> &[engagement::etats::PaieDeCote] {
        &self.a_dire
    }

    /// LES PAIES DU SALARIE `cle` MISES DE COTE, de `du` a `au` (ces mois sont lus) : un calcul de ce
    /// salarie qui a besoin de ses paies de ces mois refuse pour lui seul, en les nommant ; vide : il les a toutes.
    pub fn a_dire_de(&mut self, cle: &droit::contexte::IdSalarie, du: Mois, au: Mois) -> R<Vec<engagement::etats::PaieDeCote>> {
        for m in du.jusqu_a(au) {
            self.paies(m)?;
        }
        Ok(self.a_dire.iter().filter(|a| a.salarie.as_ref() == Some(cle) && du <= a.mois && a.mois <= au).cloned().collect())
    }

    /// Le PAS de chaque paie du mois, par position dans `paies(mois)`.
    pub fn pas(&mut self, mois: Mois) -> R<Rc<Vec<Option<Prelevement>>>> {
        if !self.pas.contains_key(&mois) {
            let paies = self.paies(mois)?;
            let v = crate::prelevement::du_mois(self.ctx, &paies, self.departs).map_err(|m| m.reclasser(Classe::Refuse))?;
            self.pas.insert(mois, Rc::new(v));
        }
        Ok(Rc::clone(&self.pas[&mois]))
    }
}


/// Le titre du spectacle d'une paie, quand elle en a un.
pub fn titre_spectacle(p: &PaieDuMois) -> Option<String> {
    p.spectacle().and_then(|s| s.titre.dite()).cloned()
}

fn nat_libelle(p: &PaieDuMois) -> R<String> {
    Ok(p.engagement().nat().map_err(|m| m.reclasser(Classe::Refuse))?.libelle.clone())
}

// ------------------------------------------------------------------ le livre de paie

/// Les montants du livre de paie, dans son ordre : la colonne sous laquelle chacun s'ecrit et se compte quand il manque,
/// ce que chaque ligne en dit, et sa somme dans un cumul.
const CUMULABLES: [(ColonneDeMontant, fn(&LigneLivre) -> Option<Montant>, fn(&CumulDuLivre) -> SommeDuLivre); 7] = [
    (C::Brut, |l| l.brut, |c| c.brut),
    (C::CotisationsSalariales, |l| l.cotisations_salariales, |c| c.cotisations_salariales),
    (C::CotisationsPatronales, |l| l.cotisations_patronales, |c| c.cotisations_patronales),
    (C::NetImposable, |l| l.net_imposable, |c| c.net_imposable),
    (C::NetAPayer, |l| l.net_a_payer, |c| c.net_a_payer),
    (C::Pas, |l| l.pas, |c| c.pas),
    (C::CoutEmployeur, |l| l.cout_employeur, |c| c.cout_employeur),
];

/// LA SOMME D'UN MONTANT DU LIVRE SUR DES LIGNES : ce que portent celles qui le disent, et combien le disent (les pieces
/// d'une paie importee peuvent le taire ; une somme que nulle ligne ne dit est zero, dite par aucune).
#[derive(Clone, Copy, Debug)]
pub struct SommeDuLivre {
    pub montant: Montant,
    pub dite: usize,
}

impl SommeDuLivre {
    const RIEN: SommeDuLivre = SommeDuLivre { montant: Montant::ZERO, dite: 0 };

    fn ajouter(&mut self, m: Option<Montant>) {
        if let Some(m) = m {
            self.montant += m;
            self.dite += 1;
        }
    }

    /// La somme, quand une ligne au moins la dit.
    pub fn si_dite(self) -> Option<Montant> {
        (self.dite > 0).then_some(self.montant)
    }
}

/// LE CUMUL DE LIGNES DU LIVRE DE PAIE : combien de bulletins, et la somme de chacun de ses montants (`CUMULABLES`). Le
/// seul cumul des etats : le livre, le recapitulatif annuel, la masse salariale, le cout par spectacle, le cout de
/// l'annee d'une fiche, la masse des contributions de branche, le taux patronal reel ; ce qui les distingue (la cle,
/// les lignes retenues) se dit a leur porte.
#[derive(Clone, Debug)]
pub struct CumulDuLivre {
    pub bulletins: usize,
    pub brut: SommeDuLivre,
    pub cotisations_salariales: SommeDuLivre,
    pub cotisations_patronales: SommeDuLivre,
    pub net_imposable: SommeDuLivre,
    pub net_a_payer: SommeDuLivre,
    pub pas: SommeDuLivre,
    pub cout_employeur: SommeDuLivre,
}

impl CumulDuLivre {
    /// Le cumul d'aucune ligne.
    pub const VIDE: CumulDuLivre = CumulDuLivre {
        bulletins: 0,
        brut: SommeDuLivre::RIEN,
        cotisations_salariales: SommeDuLivre::RIEN,
        cotisations_patronales: SommeDuLivre::RIEN,
        net_imposable: SommeDuLivre::RIEN,
        net_a_payer: SommeDuLivre::RIEN,
        pas: SommeDuLivre::RIEN,
        cout_employeur: SommeDuLivre::RIEN,
    };

    fn ajouter(&mut self, l: &LigneLivre) {
        self.bulletins += 1;
        self.brut.ajouter(l.brut);
        self.cotisations_salariales.ajouter(l.cotisations_salariales);
        self.cotisations_patronales.ajouter(l.cotisations_patronales);
        self.net_imposable.ajouter(l.net_imposable);
        self.net_a_payer.ajouter(l.net_a_payer);
        self.pas.ajouter(l.pas);
        self.cout_employeur.ajouter(l.cout_employeur);
    }

    /// Le cumul des `lignes`.
    pub fn des<'l>(lignes: impl IntoIterator<Item = &'l LigneLivre>) -> CumulDuLivre {
        let mut c = CumulDuLivre::VIDE;
        for l in lignes {
            c.ajouter(l);
        }
        c
    }
}

/// Le cumul des `lignes` sous chaque cle que `cle` leur donne, dans l'ordre ou les lignes les rencontrent ; une ligne sans
/// cle (`None`) n'entre dans aucun.
pub fn cumuls_par<'l, K: PartialEq>(lignes: impl IntoIterator<Item = &'l LigneLivre>, cle: impl Fn(&LigneLivre) -> Option<K>) -> Vec<(K, CumulDuLivre)> {
    let mut out: Vec<(K, CumulDuLivre)> = Vec::new();
    for l in lignes {
        let Some(k) = cle(l) else { continue };
        match out.iter_mut().find(|(x, _)| *x == k) {
            Some((_, c)) => c.ajouter(l),
            None => {
                let mut c = CumulDuLivre::VIDE;
                c.ajouter(l);
                out.push((k, c));
            }
        }
    }
    out
}

/// Le cumul sous la cle `k` parmi `cumuls` ; celui d'aucune ligne quand aucune ne la porte.
pub fn cumul_de<K: PartialEq>(cumuls: &[(K, CumulDuLivre)], k: &K) -> CumulDuLivre {
    match cumuls.iter().find(|(x, _)| x == k) {
        Some((_, c)) => c.clone(),
        None => CumulDuLivre::VIDE,
    }
}

/// `LigneLivre` : un bulletin, vu du livre de paie, tel que la route le rend.
#[derive(Clone, Debug, Serialize)]
pub struct LigneLivre {
    pub mois: Mois,
    pub salarie: String,
    pub cle_salarie: droit::contexte::IdSalarie,
    /// Le numero du contrat, quand il est ecrit (un mois produit l'a fige).
    pub contrat: Option<engagement::numeros::NumeroContrat>,
    /// Le libelle de sa nature de contrat.
    #[serde(rename = "nature")]
    pub libelle_nature: String,
    /// Le titre du spectacle ; vide (`""`) sans spectacle, tant que `fiche_recapitulative` le lit en texte.
    pub spectacle: String,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    pub brut: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    pub cotisations_salariales: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    pub cotisations_patronales: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    pub net_imposable: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    pub net_a_payer: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    pub pas: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    pub cout_employeur: Option<Montant>,
    /// Le jour du versement (`Engagement::versement_dit`) ; vide pour une paie importee dont les pieces ne le disent pas.
    pub verse_le: Option<Jour>,
    pub nir: Option<droit::fiche::Nir>,
    pub date_naissance: Option<Jour>,
    pub adresse: Option<String>,
}

/// LES TOTAUX D'UN GROUPE DU LIVRE, rendus dans leur forme (`TotauxRendus`) : le cumul de ses lignes, et les colonnes
/// que des lignes taisent, dans l'ordre ou on l'a vu.
#[derive(Clone, Debug, Serialize)]
#[serde(into = "TotauxRendus")]
pub struct Totaux {
    cumul: CumulDuLivre,
    taisent: Vec<(ColonneDeMontant, fn(&CumulDuLivre) -> SommeDuLivre)>,
}

impl Totaux {
    /// Les totaux des `lignes`.
    fn des(lignes: &[&LigneLivre]) -> Totaux {
        let mut taisent: Vec<(ColonneDeMontant, fn(&CumulDuLivre) -> SommeDuLivre)> = Vec::new();
        for l in lignes {
            for (c, montant, somme) in CUMULABLES {
                if montant(l).is_none() && !taisent.iter().any(|(k, _)| *k == c) {
                    taisent.push((c, somme));
                }
            }
        }
        Totaux { cumul: CumulDuLivre::des(lignes.iter().copied()), taisent }
    }

    /// Combien de bulletins.
    pub fn bulletins(&self) -> usize {
        self.cumul.bulletins
    }

    /// Combien de lignes taisent chaque colonne qu'une ligne au moins tait, dans l'ordre ou on l'a vu.
    pub fn inconnus(&self) -> Vec<(ColonneDEtat, usize)> {
        self.taisent.iter().map(|(c, somme)| ((*c).into(), self.cumul.bulletins - somme(&self.cumul).dite)).collect()
    }

    /// Le total de la somme `s` de ce cumul, ou `None` quand AUCUNE de ses lignes ne la dit.
    fn total(&self, s: SommeDuLivre) -> Option<Montant> {
        (s.dite > 0 || self.cumul.bulletins == 0).then_some(s.montant)
    }
}

/// Les totaux tels que la route les rend : les bulletins (et leur nombre dit), la somme de chaque colonne (a plat, sous
/// son nom), et ce que combien de lignes ne declarent pas.
#[derive(Serialize)]
struct TotauxRendus {
    bulletins: usize,
    bulletins_dit: String,
    #[serde(flatten, serialize_with = "socle::ecriture::en_ordre")]
    sommes: Vec<(ColonneDeMontant, String)>,
    #[serde(serialize_with = "socle::ecriture::en_ordre")]
    inconnus: Vec<(ColonneDEtat, usize)>,
}

impl Totaux {
    /// Le nombre de bulletins, dit (« 3 bulletins »).
    pub fn bulletins_dit(&self) -> String {
        let bulletins = self.bulletins();
        format!("{} {}", bulletins, if bulletins == 1 { "bulletin" } else { "bulletins" })
    }

    /// La somme de chaque montant du livre, dite, dans l'ordre de ses colonnes (`CUMULABLES`).
    pub fn sommes_dites(&self) -> Vec<String> {
        CUMULABLES.iter().map(|(_, _, somme)| socle::grandeurs::ecrit::texte::dit(somme(&self.cumul).montant)).collect()
    }
}

impl LigneLivre {
    /// Les montants de la ligne, dans l'ordre des colonnes du livre (`CUMULABLES`) ; vide, celui qu'elle ne dit pas.
    pub fn montants(&self) -> Vec<Option<Montant>> {
        CUMULABLES.iter().map(|(_, montant, _)| montant(self)).collect()
    }
}

impl From<Totaux> for TotauxRendus {
    fn from(t: Totaux) -> TotauxRendus {
        TotauxRendus {
            bulletins: t.bulletins(),
            bulletins_dit: t.bulletins_dit(),
            sommes: CUMULABLES.iter().map(|(c, _, somme)| (*c, socle::grandeurs::ecrit::texte::dit(somme(&t.cumul).montant))).collect(),
            inconnus: t.inconnus(),
        }
    }
}

/// Un groupe tel que la route le rend : son libelle, celui de son sous-total (`total_dit`), ses lignes, ses totaux.
#[derive(Debug, Serialize)]
pub struct Groupe {
    pub libelle: String,
    pub total_dit: String,
    pub lignes: Vec<LigneLivre>,
    pub totaux: Totaux,
}

/// `LivreDePaie`, tel que la route le rend : son fichier ne s'ecrit que par son nom.
#[derive(Debug, Serialize)]
pub struct LivreDePaie {
    pub periode: String,
    pub mois: Vec<Mois>,
    pub regroupement: RegroupementDEtat,
    pub groupes: Vec<Groupe>,
    pub totaux: Totaux,
    pub coordonnees: bool,
    pub format: FormatDExport,
    #[serde(serialize_with = "crate::tableaux_export::nom_du_fichier")]
    pub fichier: Option<PathBuf>,
    /// Ce que le livre dit, dont chaque paie qu'il ne compte pas (`engagement::etats::PaieDeCote::phrase`).
    pub avertissements: Vec<String>,
}

/// `REGROUPEMENTS`.
/// Ce qu'une demande dit, dans la phrase de son refus : son texte, rien quand elle ne dit rien (« « » »).
fn ecrit_ou_vide<T: std::fmt::Display>(v: Option<T>) -> String {
    match v {
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

/// Le refus d'un regroupement que le livre de paie n'a pas.
fn regroupement_inconnu(r: Option<RegroupementDEtat>) -> socle::echec::Echec {
    refus(format!("Regroupement inconnu : « {} ». Les regroupements possibles sont mois, salarie, spectacle.", ecrit_ou_vide(r)))
}

fn regroupement_lisible(r: RegroupementDEtat) -> &'static str {
    match r {
        RegroupementDEtat::Salarie => "par salarié",
        RegroupementDEtat::Mois => "par mois",
        RegroupementDEtat::Spectacle => "par spectacle",
        RegroupementDEtat::Organisme => "par organisme",
    }
}

/// Le NIR, la date de naissance et l'adresse (la forme lisible quand la fiche la porte, la declaree sinon) d'une fiche,
/// quand le livre porte les coordonnees ; chacune vide quand la fiche ne la dit pas.
fn coordonnees_de(s: &droit::fiche::ChampsSalarie, coordonnees: bool) -> (Option<droit::fiche::Nir>, Option<Jour>, Option<String>) {
    if !coordonnees {
        return (None, None, None);
    }
    (s.nir.dite().cloned(), s.date_naissance.dite().copied(), s.adresse_affichage.clone().or_else(|| s.adresse.en_texte()))
}

/// Les lignes du livre pour un mois (`engagement::etats::paies_du_mois` : les paies ecrites des engagements valides, puis
/// celles que l'import a lues), ce qu'il a fallu taire, si ses numeros de contrat sont tous ecrits, et les paies
/// qu'il ne compte pas (`engagement::etats::PaieDeCote`, en phrases). Un montant que les pieces d'une paie importee ne
/// disent pas reste vide (`PaieDuMois`) ; aucun numero n'est invente.
fn lignes_du_mois(ctx: &Contexte, mois: Mois, coordonnees: bool, departs: &engagement::departs::Departs) -> R<(Vec<LigneLivre>, Vec<String>, bool, Vec<engagement::etats::PaieDeCote>)> {
    if engagement::stock::vue_du_mois(&ctx.racine, mois)?.is_none() && !mois_importes(ctx)?.contains(&mois) {
        return Err(erreur(Classe::Refuse, format!("{} : aucun engagement ne touche ce mois.", mois)));
    }
    let (paies, a_dire) = engagement::etats::paies_du_mois(ctx, mois, departs).map_err(|m| m.reclasser(Classe::Refuse))?;
    let pas = crate::prelevement::du_mois(ctx, &paies, departs).map_err(|m| m.reclasser(Classe::Refuse))?;
    let (mut lignes, mut sans_pas) = (Vec::new(), Vec::new());
    for (p, r) in paies.iter().zip(pas.iter()) {
        if r.is_none() && p.validee().is_some() {
            sans_pas.push(qui(p.salarie())?);
        }
        let s = p.salarie();
        let (nir, date_naissance, adresse) = coordonnees_de(s, coordonnees);
        let e = p.engagement();
        lignes.push(LigneLivre {
            mois,
            salarie: qui(s)?,
            cle_salarie: s.id.clone(),
            contrat: e.numero.clone(),
            libelle_nature: nat_libelle(p)?,
            // `fiche_recapitulative` lit encore le titre en texte : vide sans spectacle.
            spectacle: titre_spectacle(p).into_iter().collect(),
            brut: p.brut().map(Montant::au_centime),
            cotisations_salariales: p.total_sal().map(Montant::au_centime),
            cotisations_patronales: p.total_pat().map(Montant::au_centime),
            net_imposable: p.net_imposable().map(Montant::au_centime),
            net_a_payer: p.net_a_payer().map(Montant::au_centime),
            pas: r.as_ref().map(|v| v.prelevement().au_centime()),
            cout_employeur: p.cout_employeur().map(Montant::au_centime),
            // Une paie importee : ce que ses pieces disent ; une paie du logiciel a toujours son jour.
            verse_le: match p.importee() {
                Some(_) => e.versement_dit()?,
                None => Some(e.date_du_versement()?),
            },
            nir,
            date_naissance,
            adresse,
        });
    }
    // Ses numeros de contrat sont ecrits quand chaque engagement est valide, ou que la DSN de l'ancien logiciel les a donnes.
    let figes = paies.iter().all(|p| p.engagement().numero.is_some());
    Ok((lignes, sans_pas, figes, a_dire))
}

/// Les lignes en groupes, chacun avec son sous-total. L'ordre DANS un groupe est chronologique puis
/// alphabetique : deux editions de la meme periode donnent le meme fichier.
fn grouper(lignes: &[LigneLivre], regroupement: RegroupementDEtat) -> R<Vec<Groupe>> {
    // Le livre ne se regroupe pas par organisme (`livre_de_paie` le refuse avant).
    if regroupement == RegroupementDEtat::Organisme {
        return Err(regroupement_inconnu(Some(regroupement)));
    }
    // Par mois, la cle est le mois ; sinon, le salarie ou le spectacle.
    let cle = |l: &LigneLivre| -> (Option<Mois>, String) {
        match regroupement {
            RegroupementDEtat::Salarie => (None, l.salarie.clone()),
            RegroupementDEtat::Mois => (Some(l.mois), String::new()),
            RegroupementDEtat::Spectacle | RegroupementDEtat::Organisme => {
                if l.spectacle.is_empty() {
                    (None, "(sans spectacle)".to_string())
                } else {
                    (None, l.spectacle.clone())
                }
            }
        }
    };
    let dedans = |l: &LigneLivre| -> (String, String, Option<engagement::numeros::NumeroContrat>) {
        match regroupement {
            RegroupementDEtat::Salarie => (l.mois.to_string(), l.spectacle.clone(), l.contrat.clone()),
            RegroupementDEtat::Mois => (l.salarie.clone(), l.spectacle.clone(), l.contrat.clone()),
            RegroupementDEtat::Spectacle | RegroupementDEtat::Organisme => (l.mois.to_string(), l.salarie.clone(), l.contrat.clone()),
        }
    };
    let mut paquets: BTreeMap<(Option<Mois>, String), Vec<LigneLivre>> = BTreeMap::new();
    for l in lignes {
        paquets.entry(cle(l)).or_default().push(l.clone());
    }
    let mut out = Vec::new();
    for ((m, k), mut v) in paquets {
        v.sort_by_cached_key(|l| dedans(l));
        let refs: Vec<&LigneLivre> = v.iter().collect();
        let totaux = Totaux::des(&refs);
        // Groupe par mois : il se nomme par son mois ; sinon par sa cle (le salarie, le spectacle).
        let libelle = match m {
            Some(m) => libelle_mois(m),
            None => k,
        };
        out.push(Groupe { total_dit: format!("Total {}", libelle), libelle, lignes: v, totaux });
    }
    Ok(out)
}

/// Le livre de paie d'UNE periode (un mois, une annee ou toutes), au format et dans le regroupement demandes.
pub fn livre_de_paie(ctx: &Contexte, d: &Demande) -> R<LivreDePaie> {
    Ok(livre_lu(ctx, d)?.0)
}

/// Le livre de paie (`livre_de_paie`), et les paies qu'il ne compte pas (`engagement::etats::PaieDeCote`,
/// que ses avertissements disent chacune).
fn livre_lu(ctx: &Contexte, d: &Demande) -> R<(LivreDePaie, Vec<engagement::etats::PaieDeCote>)> {
    let quand = PeriodeDemandee::des_trois(
        d.mois,
        d.annee,
        d.toutes,
        "Un livre de paie se demande pour UNE période : un mois (AAAA-MM), une année (AAAA), ou toutes les années (toutes).",
    )?;
    let format = match d.format {
        Some(f @ (FormatDExport::Csv | FormatDExport::Xlsx | FormatDExport::Pdf)) => f,
        autre => return Err(refus(format!("Format inconnu : « {} ». Les formats possibles sont csv, pdf, xlsx.", ecrit_ou_vide(autre)))),
    };
    let regroupement = match d.regroupement {
        Some(r @ (RegroupementDEtat::Salarie | RegroupementDEtat::Mois | RegroupementDEtat::Spectacle)) => r,
        autre => return Err(regroupement_inconnu(autre)),
    };
    let p = Periode::pour(ctx, quand, true)?;
    let (mut lignes, mut avert, mut sans_pas, mut sans_numeros) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut a_dire: Vec<engagement::etats::PaieDeCote> = Vec::new();
    for &m in &p.mois {
        let (l_m, sans, figes, non_comptees) = lignes_du_mois(ctx, m, d.coordonnees, &d.departs)?;
        lignes.extend(l_m);
        sans_pas.extend(sans);
        a_dire.extend(non_comptees);
        if !figes {
            sans_numeros.push(m.to_string());
        }
    }
    // Des paies que le livre ne compte pas encore (`PaieDeCote`) : il repond, vide s'il le faut, et dit pourquoi.
    if lignes.is_empty() && a_dire.is_empty() {
        return Err(refus("Aucun bulletin sur la période : rien à porter au livre de paie."));
    }
    avert.extend(a_dire.iter().map(engagement::etats::PaieDeCote::phrase));
    if !sans_numeros.is_empty() {
        avert.push(format!(
            "Numéros de contrat absents pour {} : ces mois n'ont pas été produits par PuppetPaie, rien n'y a été figé. La colonne Contrat reste vide plutôt que de porter un numéro qui n'a jamais été déclaré.",
            sans_numeros.join(", ")
        ));
    }
    if !sans_pas.is_empty() {
        let uniques: BTreeSet<&String> = sans_pas.iter().collect();
        avert.push(format!(
            "Prélèvement à la source inconnu pour {} bulletin(s) ({}) : aucun taux DGFiP reçu et le barème de l'année ne porte pas la grille par défaut. La colonne reste vide et le total du PAS ne les compte pas.",
            sans_pas.len(),
            uniques.into_iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    if d.coordonnees {
        avert.push("Ce fichier porte le NIR, la date de naissance et l'adresse des salariés : il reste dans le dossier de la compagnie, et ne s'envoie qu'à qui en a besoin.".to_string());
    }
    let groupes = grouper(&lignes, regroupement)?;
    let refs: Vec<&LigneLivre> = lignes.iter().collect();
    let mut livre = LivreDePaie {
        periode: p.libelle.clone(),
        mois: p.mois.clone(),
        regroupement,
        groupes,
        totaux: Totaux::des(&refs),
        coordonnees: d.coordonnees,
        format,
        fichier: None,
        avertissements: avert,
    };
    if d.ecrire {
        livre.fichier = Some(ecrire_livre(ctx, &livre, &p.quand)?);
    }
    Ok((livre, a_dire))
}

/// LE NOM D'UN FICHIER D'EXPORT, ecrit en Rust : autour de la periode (`Periode::nom`, `PeriodeDemandee::nom`, une annee), ou fixe
/// (`COMPTES.TXT`, qu'EBP cherche sous ce nom-la, a cote des ecritures).
pub enum NomDExport {
    Autour(NomDeFichier, NomDeFichier),
    Fixe(NomDeFichier),
}

impl NomDExport {
    /// Le nom autour de la periode : `avant<periode>apres`.
    pub const fn autour(avant: &'static str, apres: &'static str) -> NomDExport {
        NomDExport::Autour(NomDeFichier::du_moteur(avant), NomDeFichier::du_moteur(apres))
    }

    /// Le nom du fichier de la periode `periode`.
    pub fn de(&self, periode: &NomDeFichier) -> NomDeFichier {
        match self {
            NomDExport::Autour(avant, apres) => avant.suivi_de(periode).suivi_de(apres),
            NomDExport::Fixe(nom) => nom.clone(),
        }
    }
}

/// Ecrit le livre dans `exports/AAAA/livre-de-paie-<periode>-<regroupement>[-coordonnees].<ext>`.
fn ecrire_livre(ctx: &Contexte, livre: &LivreDePaie, quand: &PeriodeDemandee) -> R<PathBuf> {
    let dossier = dossier_de(ctx, quand);
    std::fs::create_dir_all(&dossier).map_err(|e| socle::echec::du_disque(&dossier, &e).reclasser(Classe::Disque))?;
    let octets = match livre.format {
        FormatDExport::Csv => csv_livre(livre),
        FormatDExport::Xlsx => xlsx_livre(livre)?,
        FormatDExport::Pdf => pdf_livre(ctx, livre)?,
        autre => return Err(refus(format!("Format inconnu : « {} ». Les formats possibles sont csv, pdf, xlsx.", autre))),
    };
    let groupe = const { NomDeFichier::du_moteur("-") }.suivi_de(&livre.regroupement.nom());
    let groupe = match livre.coordonnees {
        true => groupe.suivi_de(&const { NomDeFichier::du_moteur("-coordonnees") }),
        false => groupe,
    };
    let nom = NomDExport::Autour(const { NomDeFichier::du_moteur("livre-de-paie-") }, groupe.avec_l_extension(&livre.format.nom())).de(&quand.nom());
    let chemin = dossier.join(nom);
    std::fs::write(&chemin, octets).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque))?;
    Ok(chemin)
}

// --------------------------------------------------- les ecrivains du livre de paie

/// Les colonnes qui portent un montant : elles s'alignent a droite au PDF et se formatent en euros au classeur.
const MONTANTS: [&str; 7] = ["Brut", "Cotisations salariales", "Cotisations patronales", "Net imposable", "Net a payer", "PAS", "Cout employeur"];

/// `colonnes(livre)` : l'en-tete, dans l'ordre.
fn colonnes_livre(livre: &LivreDePaie) -> Vec<&'static str> {
    let mut c = vec!["Mois", "Salarie"];
    if livre.coordonnees {
        c.extend(["NIR", "Date de naissance", "Adresse"]);
    }
    c.extend(["Contrat", "Nature", "Spectacle", "Brut", "Cotisations salariales", "Cotisations patronales", "Net imposable", "Net a payer", "PAS", "Cout employeur"]);
    c
}

/// Une cellule du livre : un texte (ou son absence), un montant (ou son absence).
#[derive(Clone)]
enum CelL {
    T(String),
    Vide,
    M(Option<Montant>),
}

fn cellules(l: &LigneLivre, coordonnees: bool) -> Vec<CelL> {
    let t = |s: &str| CelL::T(s.to_string());
    let o = |s: &Option<String>| match s {
        Some(x) => CelL::T(x.clone()),
        None => CelL::Vide,
    };
    let mut c = vec![t(&l.mois.to_string()), t(&l.salarie)];
    if coordonnees {
        c.extend([o(&l.nir.as_ref().map(ToString::to_string)),o(&l.date_naissance.map(|j| j.iso())), o(&l.adresse)]);
    }
    c.extend([o(&l.contrat.as_ref().map(ToString::to_string)), t(&l.libelle_nature), t(&l.spectacle)]);
    c.extend(CUMULABLES.iter().map(|(_, montant, _)| CelL::M(montant(l))));
    c
}

fn cellules_totaux(tot: &Totaux, libelle: &str, coordonnees: bool) -> Vec<CelL> {
    let t = |s: &str| CelL::T(s.to_string());
    let mut c = vec![t(""), t(libelle)];
    if coordonnees {
        c.extend([t(""), t(""), t("")]);
    }
    c.extend([CelL::T(format!("{} bulletin(s)", tot.bulletins())), t(""), t("")]);
    c.extend(CUMULABLES.iter().map(|(_, _, somme)| CelL::M(tot.total(somme(&tot.cumul)))));
    c
}

/// Les lignes du livre dans l'ordre d'ecriture : (cellules, ce qu'est la rangee).
fn rangees_livre(livre: &LivreDePaie) -> Vec<(Vec<CelL>, Rangee)> {
    let mut out = Vec::new();
    for g in &livre.groupes {
        for l in &g.lignes {
            out.push((cellules(l, livre.coordonnees), Rangee::Ligne));
        }
        out.push((cellules_totaux(&g.totaux, &format!("Total {}", g.libelle), livre.coordonnees), Rangee::Total));
    }
    out.push((cellules_totaux(&livre.totaux, "TOTAL GENERAL", livre.coordonnees), Rangee::General));
    out
}

fn titre_livre(livre: &LivreDePaie) -> String {
    format!("Livre de paie -- {}, {}", livre.periode, regroupement_lisible(livre.regroupement))
}

/// `csv_livre` : point-virgule, virgule decimale, UTF-8 avec BOM.
pub fn csv_livre(livre: &LivreDePaie) -> Vec<u8> {
    let mut s = String::from("\u{feff}");
    let ligne = |s: &mut String, c: &[String]| s.push_str(&socle::csv::ligne(c, ';'));
    let texte = |c: &CelL| -> String {
        match c {
            CelL::T(t) => t.clone(),
            CelL::Vide | CelL::M(None) => String::new(),
            CelL::M(Some(d)) => socle::texte::a_virgule(*d),
        }
    };
    ligne(&mut s, &[titre_livre(livre)]);
    ligne(&mut s, &[]);
    ligne(&mut s, &colonnes_livre(livre).iter().map(|c| c.to_string()).collect::<Vec<_>>());
    for g in &livre.groupes {
        for l in &g.lignes {
            ligne(&mut s, &cellules(l, livre.coordonnees).iter().map(texte).collect::<Vec<_>>());
        }
        ligne(&mut s, &cellules_totaux(&g.totaux, &format!("Total {}", g.libelle), livre.coordonnees).iter().map(texte).collect::<Vec<_>>());
        ligne(&mut s, &[]);
    }
    ligne(&mut s, &cellules_totaux(&livre.totaux, "TOTAL GENERAL", livre.coordonnees).iter().map(texte).collect::<Vec<_>>());
    for a in &livre.avertissements {
        ligne(&mut s, &[]);
        ligne(&mut s, &[format!("! {}", a)]);
    }
    s.into_bytes()
}

/// `xlsx_livre` : une feuille, les colonnes en place, les montants en nombres (le comptable les additionne).
pub fn xlsx_livre(livre: &LivreDePaie) -> R<Vec<u8>> {
    let mut ws = Feuille::nouvelle("Livre de paie");
    ws.texte(0, 0, titre_livre(livre), &Style::gras(13));
    let cl = colonnes_livre(livre);
    let entete = Style { gras: true, retour_a_la_ligne: true, vertical: Some(Vertical::Centre), ..Style::CLASSEUR };
    for (i, c) in cl.iter().enumerate() {
        ws.texte(2, i as u16, *c, &entete);
    }
    let mut ligne: u32 = 3;
    let ecrire_rang = |ws: &mut Feuille, cel: &[CelL], gras: bool, corps: u32, ligne: u32| {
        for (i, c) in cel.iter().enumerate() {
            let style = Style { gras, corps, ..Style::CLASSEUR };
            match c {
                CelL::T(t) => {
                    if !t.is_empty() {
                        ws.texte(ligne, i as u16, t.clone(), &style);
                    }
                }
                CelL::Vide | CelL::M(None) => {}
                CelL::M(Some(d)) => {
                    let style = Style { format: MONTANTS.contains(&cl[i]).then_some("# ##0.00 \u{20ac}"), ..style };
                    ws.nombre(ligne, i as u16, *d, &style);
                }
            }
        }
    };
    for g in &livre.groupes {
        for l in &g.lignes {
            ecrire_rang(&mut ws, &cellules(l, livre.coordonnees), false, CORPS_DU_CLASSEUR, ligne);
            ligne += 1;
        }
        ecrire_rang(&mut ws, &cellules_totaux(&g.totaux, &format!("Total {}", g.libelle), livre.coordonnees), true, CORPS_DU_CLASSEUR, ligne);
        ligne += 2;
    }
    ecrire_rang(&mut ws, &cellules_totaux(&livre.totaux, "TOTAL GENERAL", livre.coordonnees), true, 12, ligne);
    ligne += 1;
    for a in &livre.avertissements {
        ligne += 1;
        ws.texte(ligne, 0, format!("! {}", a), &Style::CLASSEUR);
        ligne += 1;
    }
    for (i, nom) in cl.iter().enumerate() {
        ws.largeur(i as u16, i as u16, if ["Salarie", "Adresse", "Spectacle"].contains(nom) { 34 } else { 16 });
    }
    ws.figer(3);
    ws.octets()
}

/// `pdf_livre` : A4 paysage, l'en-tete des colonnes a chaque page, un pied qui numerote.
pub fn pdf_livre(ctx: &Contexte, livre: &LivreDePaie) -> R<Vec<u8>> {
    let cel = |c: &CelL| -> String {
        match c {
            CelL::T(t) => t.clone(),
            CelL::Vide => String::new(),
            CelL::M(None) => "—".to_string(),
            CelL::M(Some(d)) => socle::texte::a_virgule(*d),
        }
    };
    let rangs: Vec<(Vec<String>, Rangee)> = rangees_livre(livre).iter().map(|(c, g)| (c.iter().map(cel).collect(), *g)).collect();
    let etiquette = |c: &str| -> String {
        match c {
            "Salarie" => "Salarié",
            "Net a payer" => "Net à payer",
            "Cout employeur" => "Coût employeur",
            autre => autre,
        }
        .to_string()
    };
    let entetes: Vec<(String, bool)> = colonnes_livre(livre).iter().map(|c| (etiquette(c), MONTANTS.contains(c))).collect();
    // Le nom d'usage (pas la raison sociale).
    let nom = ctx.entreprise.nom_affichage.remplie();
    let sous = format!("{} — {}{}", livre.periode, regroupement_lisible(livre.regroupement), if livre.coordonnees { " — avec les coordonnées des salariés" } else { "" });
    let d = crate::tableaux_export::description_pdf(
        "Livre de paie",
        &livre.periode,
        nom,
        &sous,
        &format!("Livre de paie — {}", livre.periode),
        &entetes,
        &rangs,
        &livre.avertissements,
        &[],
    );
    crate::tableaux_export::pdf_fabrique(&d)
}

// ------------------------------------------------------------------ _finir, _sections

/// Le format, les totaux, puis -- si on ecrit -- le fichier `<nom>-<periode>.<ext>` dans le dossier
/// de la periode (celui d'`exports/` sans periode).
pub fn finir(ctx: &Contexte, t: &mut Tableau, p: Option<&Periode>, nom: &str, format: Option<FormatDExport>, ecrire: bool) -> R<()> {
    t.format = format_possible(format, &FORMATS_TABLEAU)?;
    totaliser(t, TOTAL_GENERAL);
    if ecrire {
        ecrire_fini(ctx, t, p, nom)?;
    }
    Ok(())
}

/// L'ecriture d'un tableau fini : `tb.ecrire(t, dossier, "<nom>-<periode>" ou "<nom>", ctx)`.
pub fn ecrire_fini(ctx: &Contexte, t: &mut Tableau, p: Option<&Periode>, nom: &str) -> R<()> {
    let (dossier, fichier) = match p {
        Some(p) => (p.dossier(ctx), format!("{}-{}", nom, p.nom())),
        None => (dossier_exports(ctx, None), nom.to_string()),
    };
    ecrire_tableau(t, &dossier, &fichier, ctx)?;
    Ok(())
}

/// Les lignes du livre de paie, et ses avertissements -- les etats de synthese se
/// construisent dessus, pour tomber sur le livre.
pub fn lignes_livre(ctx: &Contexte, p: &Periode, avert: &mut Vec<String>, departs: &engagement::departs::Departs) -> R<Vec<LigneLivre>> {
    let (livre, _) = livre_des_etats(ctx, p, departs)?;
    avert.extend(livre.avertissements.iter().filter(|a| !a.starts_with("Numéros de contrat")).cloned());
    Ok(livre.groupes.into_iter().flat_map(|g| g.lignes).collect())
}

/// Le livre de paie de la periode `p` que lisent les etats de synthese (`lignes_livre`) et le cout de l'annee : regroupe
/// par salarie, sans coordonnees, sans fichier ; et les paies reprises qu'il ne compte pas (ses avertissements les disent).
pub fn livre_des_etats(ctx: &Contexte, p: &Periode, departs: &engagement::departs::Departs) -> R<(LivreDePaie, Vec<engagement::etats::PaieDeCote>)> {
    let (mois, annee, toutes) = p.quand.en_trois();
    let d = Demande {
        mois,
        annee,
        toutes,
        regroupement: Some(RegroupementDEtat::Salarie),
        format: Some(FormatDExport::Csv),
        departs: departs.clone(),
        coordonnees: false,
        ecrire: false,
        logiciel: None,
        analytique: None,
    };
    livre_lu(ctx, &d)
}

/// Le salarie tel que le dit la premiere des `lignes` d'un cumul (celles que `du_cumul` retient).
fn premier_nom(lignes: &[LigneLivre], du_cumul: impl Fn(&LigneLivre) -> bool) -> R<String> {
    match lignes.iter().find(|l| du_cumul(l)) {
        Some(l) => Ok(l.salarie.clone()),
        None => Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : un cumul du livre de paie n'a aucune ligne.")),
    }
}

/// L'annee du mois, sur quatre chiffres.
fn annee_de(m: Mois) -> String {
    format!("{:04}", m.annee())
}

/// Les cles tri : un tuple de textes.
fn cle_tri(l: &Ligne, cles: &[ColonneDeTexte]) -> Vec<String> {
    cles.iter().map(|c| l.texte(*c)).collect()
}

// =========================================================== 1. le livre de paie detaille

/// Chaque bulletin ligne a ligne -- la remuneration, puis chaque cotisation,
/// base, taux et montants.
pub fn livre_detaille(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let avert = Vec::new();
    let mut lecture = Lecture::nouvelle(ctx, &d.departs);
    let mut sections = Vec::new();
    for m in p.mois.clone() {
        let paies = lecture.paies(m)?;
        let mut ordre: Vec<(String, Option<engagement::numeros::NumeroContrat>, usize)> = Vec::new();
        for (i, paie) in paies.iter().enumerate() {
            ordre.push((qui(paie.salarie())?, paie.engagement().numero.clone(), i));
        }
        ordre.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        for (nom, numero, i) in ordre {
            let paie = &paies[i];
            let titre = titre_spectacle(paie);
            let socle = |l: &mut Ligne, genre: &str| {
                l.m(C::Mois, m).t(C::Salarie, nom.clone()).ot(C::Contrat, numero.clone()).t(C::Genre, genre);
            };
            let mut lignes = Vec::new();
            for r in paie.remuneration() {
                // Une ligne rattachee a sa rubrique : son code et son libelle au catalogue ; une ligne d'un bulletin
                // importe qui ne l'est pas : ce qu'il imprime. Ce qu'elle paie, quand elle le dit (une ligne calculee le
                // dit toujours ; une ligne importee, quand le bulletin l'imprime).
                let (code, libelle, paiement, montant) = match r {
                    RemunerationDuMois::Calculee(r) => (Some(r.code.clone()), r.libelle.clone(), Some(r.paiement), Some(r.montant)),
                    RemunerationDuMois::Rattachee(r) => (Some(r.code.clone()), r.libelle.clone(), r.paiement, Some(r.montant)),
                    RemunerationDuMois::Imprimee(i) => (None, i.libelle.clone(), None, i.montant),
                };
                let mut l = Ligne::nouvelle();
                socle(&mut l, "Rémunération");
                l.ot(C::Code, code).t(C::Libelle, libelle);
                // La colonne « Base ou quantité » : la base en euros d'une ligne en pourcentage, la quantité d'une autre.
                match paiement {
                    Some(PaiementDeLigne::Base(b, _)) => l.om(C::Base, Some(b)),
                    p => l.grandeur_ou_rien(C::Base, p.and_then(PaiementDeLigne::quantite)),
                };
                l.grandeur_ou_rien(C::TauxSal, paiement.map(PaiementDeLigne::taux)).om(C::Gain, montant.map(Montant::au_centime));
                lignes.push(l);
            }
            for c in paie.cotisations() {
                let mut l = Ligne::nouvelle();
                socle(&mut l, "Cotisation");
                l.ot(C::Code, c.cotisation().map(|i| i.as_str().to_string()));
                l.t(C::Libelle, c.libelle()).base(C::Base, c.base().map(|x| x.au_centime()));
                l.taux(C::TauxSal, c.taux_salarie()).om(C::PartSal, c.montant_salarie().map(Montant::au_centime));
                l.taux(C::TauxPat, c.taux_employeur()).om(C::PartPat, c.montant_employeur().map(Montant::au_centime));
                lignes.push(l);
            }
            for (lib, v) in [("Net imposable", paie.net_imposable()), ("Net à payer avant impôt", paie.net_a_payer())] {
                let mut l = Ligne::nouvelle();
                socle(&mut l, "Résultat");
                l.t(C::Libelle, lib).om(C::NetDuBulletin, v.map(Montant::au_centime));
                lignes.push(l);
            }
            let sec = format!("{} — {}{}", nom, libelle_mois(m), titre.filter(|t| !t.is_empty()).map(|t| format!(" — {}", t)).into_iter().collect::<String>());
            sections.push(Section::nouvelle(sec, lignes));
        }
    }
    let mut t = Tableau::nouveau(EtatDExport::LivreDePaieDetaille, "Livre de paie détaillé", p.libelle.clone());
    t.mois = p.mois.clone();
    t.sections = sections;
    t.avertissements = avert;
    t.avertissements.extend(lecture.avertissements());
    t.colonnes = vec![
        C::Mois.colonne(),
        C::Salarie.colonne(),
        C::Contrat.colonne(),
        C::Genre.colonne(),
        C::Code.colonne(),
        C::Libelle.colonne(),
        C::Base.titree("Base ou quantité"),
        C::TauxSal.titree("Taux (sal.) ou unitaire"),
        C::Gain.colonne(),
        C::PartSal.colonne(),
        C::TauxPat.colonne(),
        C::PartPat.colonne(),
        C::NetDuBulletin.colonne(),
    ];
    t.sources = vec!["le bulletin de paie tel que PuppetPaie le calcule et l'imprime (art. R3243-1 du Code du travail pour ses mentions)".to_string()];
    // Le total du Net additionnerait net imposable et net a payer : sa colonne n'en a pas (`NetDuBulletin`).
    finir(ctx, &mut t, Some(&p), "livre-de-paie-detaille", d.format,false)?;
    if d.ecrire {
        ecrire_fini(ctx, &mut t, Some(&p), "livre-de-paie-detaille")?;
    }
    Ok(t)
}

// ===================================================== 2. le recapitulatif annuel par salarie

/// Le recapitulatif annuel : une ligne par salarie et par annee (pas pour un mois).
pub fn recapitulatif_annuel(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    if d.mois.is_some() {
        return Err(refus("Le récapitulatif annuel se demande pour une année, ou pour toutes les années — pas pour un mois."));
    }
    let p = Periode::nouvelle(ctx, None, d.annee, d.toutes, true)?;
    let mut avert = Vec::new();
    let lignes = lignes_livre(ctx, &p, &mut avert, &d.departs)?;
    // Le cumul de chaque salarie par annee, et celui de chacun de ses mois.
    let par_mois = cumuls_par(&lignes, |l| Some((annee_de(l.mois), l.cle_salarie.clone(), l.mois.de_l_annee())));
    let mut a_trier = Vec::new();
    for ((annee, cle), c) in cumuls_par(&lignes, |l| Some((annee_de(l.mois), l.cle_salarie.clone()))) {
        let mut ligne = Ligne::nouvelle();
        ligne.t(C::Annee, annee.clone()).t(C::Salarie, premier_nom(&lignes, |l| annee_de(l.mois) == annee && l.cle_salarie == cle)?).t(C::CleSalarie, cle.to_string()).compte(C::Bulletins, c.bulletins);
        // Le brut de chaque mois : ce que les bulletins du mois en disent ; un mois sans brut dit reste vide.
        for m in socle::mois::MoisDeLAnnee::TOUS {
            if let Some(brut) = cumul_de(&par_mois, &(annee.clone(), cle.clone(), m)).brut.si_dite() {
                ligne.montant(C::DuMois(m), brut);
            }
        }
        // Une somme qu'aucun bulletin ne dit reste vide.
        for (champ, s) in [
            (C::Brut, c.brut),
            (C::NetImposable, c.net_imposable),
            (C::NetAPayer, c.net_a_payer),
            (C::Pas, c.pas),
            (C::CotisationsPatronales, c.cotisations_patronales),
            (C::CoutEmployeur, c.cout_employeur),
        ] {
            if let Some(v) = s.si_dite() {
                ligne.montant(champ, v);
            }
        }
        a_trier.push(ligne);
    }
    let mut cols = vec![C::Salarie.colonne()];
    // Une colonne par mois de l'annee (l'annee n'y paraît pas).
    cols.extend(socle::mois::MoisDeLAnnee::TOUS.map(|m| C::DuMois(m).colonne()));
    cols.extend([
        C::Brut.titree("Brut annuel"),
        C::NetImposable.colonne(),
        C::Pas.colonne(),
        C::NetAPayer.titree("Net payé"),
        C::CoutEmployeur.colonne(),
        C::Bulletins.colonne(),
    ]);
    let mut t = Tableau::nouveau(EtatDExport::RecapitulatifAnnuel, "Récapitulatif annuel par salarié", p.libelle.clone());
    t.mois = p.mois.clone();
    t.colonnes = cols;
    t.avertissements = avert;
    t.sections = sections_de(a_trier, |l| l.texte(C::Annee), |k| Ok(k.clone()), Some(&|l| cle_tri(l, &[C::Salarie, C::CleSalarie])))?;
    t.sources = vec!["cumuls des bulletins de l'année ; le net imposable est celui que la DSN transmet à la DGFiP pour la déclaration de revenus".to_string()];
    finir(ctx, &mut t, Some(&p), "recapitulatif-annuel", d.format,d.ecrire)?;
    Ok(t)
}

// ================================================== 3. la masse salariale par mois

/// Un mois par ligne -- ce que le comptable rapproche du compte 641.
pub fn masse_salariale(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let lignes = lignes_livre(ctx, &p, &mut avert, &d.departs)?;
    let mut a_trier = Vec::new();
    for (m, c) in cumuls_par(&lignes, |l| Some(l.mois)) {
        let mut ligne = Ligne::nouvelle();
        ligne.m(C::Mois, m).t(C::Libelle, libelle_mois(m)).compte(C::Bulletins, c.bulletins);
        // Une somme qu'aucun bulletin du mois ne dit reste vide.
        for (champ, _, somme) in CUMULABLES {
            if let Some(v) = somme(&c).si_dite() {
                ligne.montant(champ, v);
            }
        }
        // Les salaries distincts du mois : un compte, pas une somme.
        let salaries: BTreeSet<&droit::contexte::IdSalarie> = lignes.iter().filter(|l| l.mois == m).map(|l| &l.cle_salarie).collect();
        ligne.compte(C::Salaries, salaries.len());
        a_trier.push(ligne);
    }
    let mut t = Tableau::nouveau(EtatDExport::MasseSalariale, "Masse salariale par mois", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(a_trier, |l| l.mois(C::Mois).map(annee_de), |k| k.clone().ok_or_else(sans_mois), Some(&|l| cle_tri(l, &[C::Mois])))?;
    t.colonnes = vec![
        C::Libelle.titree("Mois"),
        C::Salaries.colonne(),
        C::Bulletins.colonne(),
        C::Brut.colonne(),
        C::CotisationsSalariales.colonne(),
        C::CotisationsPatronales.colonne(),
        C::NetImposable.colonne(),
        C::Pas.colonne(),
        C::NetAPayer.colonne(),
        C::CoutEmployeur.colonne(),
    ];
    t.sources = vec!["cumuls des bulletins du mois".to_string()];
    finir(ctx, &mut t, Some(&p), "masse-salariale", d.format,false)?;
    if d.ecrire {
        ecrire_fini(ctx, &mut t, Some(&p), "masse-salariale")?;
    }
    Ok(t)
}

// ===================================================== 4. l'etat des virements

/// Le net a verser, salarie par salarie et mois par mois.
pub fn etat_des_virements(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut lignes = Vec::new();
    for l in lignes_livre(ctx, &p, &mut avert, &d.departs)? {
        let mut x = Ligne::nouvelle();
        x.m(C::Mois, l.mois).t(C::Salarie, l.salarie.clone()).ot(C::Contrat, l.contrat.clone()).ot(C::Spectacle, Some(l.spectacle.clone()).filter(|s| !s.is_empty()));
        // Ce qui part au salarie : le net a payer du bulletin, son prelevement a la source deduit quand il en porte un.
        let verse = l.net_a_payer.map(|n| match l.pas {
            Some(p) => n - p,
            None => n,
        });
        x.om(C::NetAPayer, verse).ot(C::VerseLe, l.verse_le.map(|j| j.jj_mm_aaaa()));
        lignes.push(x);
    }
    let mut t = Tableau::nouveau(EtatDExport::EtatDesVirements, "État des virements de salaires", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(lignes, |l| l.mois(C::Mois), section_du_mois, Some(&|l| cle_tri(l, &[C::Salarie, C::Contrat])))?;
    t.colonnes = vec![
        C::Mois.colonne(),
        C::Salarie.colonne(),
        C::Contrat.colonne(),
        C::Spectacle.colonne(),
        C::NetAPayer.titree("Net versé"),
        C::VerseLe.colonne(),
    ];
    t.sources = vec!["net à payer du bulletin, prélèvement à la source déduit, et date de paiement de la paie validée (la date de versement que la DSN déclare, S21.G00.50.001)".to_string()];
    finir(ctx, &mut t, Some(&p), "etat-des-virements", d.format,d.ecrire)?;
    Ok(t)
}

// ================================================ 5. le prelevement a la source

/// Ce qui a ete retenu au titre de l'impot sur le revenu.
pub fn prelevement_a_la_source(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let (mut avert, mut lignes, mut sans) = (Vec::new(), Vec::new(), Vec::new());
    let mut lecture = Lecture::nouvelle(ctx, &d.departs);
    let coord = d.coordonnees;
    for m in p.mois.clone() {
        let r_pas = lecture.pas(m)?;
        let paies = lecture.paies(m)?;
        for (paie, r) in paies.iter().zip(r_pas.iter()) {
            if r.is_none() && paie.validee().is_some() {
                sans.push(qui(paie.salarie())?);
            }
            let mut l = Ligne::nouvelle();
            l.m(C::Mois, m).t(C::Salarie, qui(paie.salarie())?).ot(C::Contrat, paie.engagement().numero.clone());
            l.ot(C::Nir, paie.salarie().nir.portee().filter(|_| coord));
            l.om(C::NetImposable, paie.net_imposable().map(Montant::au_centime));
            match r {
                Some(v) => {
                    // Ce que la paie ne dit pas (une paie importee dont les pieces ne l'impriment pas) reste vide.
                    l.om(C::Abattement, v.abattement()).om(C::AssiettePas, v.montant_soumis());
                    l.ot(C::Type, v.type_taux().map(|t| format!("{} — {}", t, t.libelle())));
                    l.taux_de_pas(C::Taux, v.taux());
                    l.montant(C::Prelevement, v.prelevement().au_centime());
                }
                None => {
                    l.rien(C::Abattement).rien(C::AssiettePas).rien(C::Type).rien(C::Taux).rien(C::Prelevement);
                }
            }
            lignes.push(l);
        }
    }
    if !sans.is_empty() {
        let uniques: BTreeSet<&String> = sans.iter().collect();
        avert.push(format!(
            "Prélèvement inconnu pour {} : aucun taux DGFiP reçu et le barème ne porte pas la grille par défaut. La ligne reste vide.",
            uniques.into_iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    let mut cols = vec![C::Mois.colonne(), C::Salarie.colonne(), C::Contrat.colonne()];
    if coord {
        cols.push(C::Nir.colonne());
    }
    cols.extend([
        C::NetImposable.colonne(),
        C::Abattement.colonne(),
        C::AssiettePas.colonne(),
        C::Type.colonne(),
        C::Taux.colonne(),
        C::Prelevement.colonne(),
    ]);
    let mut t = Tableau::nouveau(EtatDExport::PrelevementALaSource, "Prélèvement à la source", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.avertissements.extend(lecture.avertissements());
    t.colonnes = cols;
    t.donnees_personnelles = coord;
    t.sections = sections_de(lignes, |l| l.mois(C::Mois), section_du_mois, Some(&|l| cle_tri(l, &[C::Salarie, C::Contrat])))?;
    t.sources = vec!["art. 204 A et suivants du CGI ; abattement des contrats courts : BOI-IR-PAS-20-20-30-10 ; types de taux : cahier technique DSN 2026.1, rubrique S21.G00.50.007".to_string()];
    finir(ctx, &mut t, Some(&p), "prelevement-a-la-source", d.format,d.ecrire)?;
    Ok(t)
}

// ================================================ 9. la reduction generale

/// Une grandeur de la reduction generale, un zero ecrit `0` (son echelle nulle).
fn ou_zero<G: Grandeur + PartialEq>(g: G) -> G {
    if g == nulle() {
        nulle()
    } else {
        g
    }
}

/// La reduction generale de chaque paie de la periode : brut, heures, SMIC de reference, coefficient.
pub fn reduction_generale(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let (mut avert, mut lignes) = (Vec::new(), Vec::new());
    let mut lecture = Lecture::nouvelle(ctx, &d.departs);
    for m in p.mois.clone() {
        // Une paie importee ne porte pas de reduction generale : rien n'y est calcule.
        for paie in lecture.paies(m)?.iter().filter_map(PaieDuMois::validee) {
            let Some(rg) = &paie.reduction_generale else { continue };
            let mut l = Ligne::nouvelle();
            l.m(C::Mois, m).t(C::Salarie, qui(&paie.salarie)?).ot(C::Contrat, paie.engagement.numero.clone());
            // Les cases du tableau : leur porte.
            l.montant(C::Brut, paie.brut.au_centime()).grandeur(C::Heures, ou_zero(rg.heures)).montant(C::Smic, rg.smic_reference.au_centime()).grandeur(C::Coefficient, ou_zero(rg.coefficient));
            l.montant(C::Montant, rg.montant.au_centime()).montant(C::Urssaf, rg.urssaf.au_centime()).montant(C::Chomage, rg.chomage.au_centime()).montant(C::Retraite, rg.agirc_arrco.au_centime());
            lignes.push(l);
        }
    }
    if lignes.is_empty() {
        avert.push("Aucun bulletin de la période ne porte la réduction générale (elle ne s'applique pas aux artistes du spectacle).".to_string());
    }
    // Les paies reprises qui ne se rangent pas : l'etat ne les lit pas, et le dit.
    avert.extend(lecture.avertissements());
    let mut t = Tableau::nouveau(EtatDExport::ReductionGenerale, "Réduction générale des cotisations", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(lignes, |l| l.mois(C::Mois), section_du_mois, Some(&|l| cle_tri(l, &[C::Salarie])))?;
    t.colonnes = vec![
        C::Mois.colonne(),
        C::Salarie.colonne(),
        C::Contrat.colonne(),
        C::Brut.colonne(),
        C::Heures.colonne(),
        C::Smic.colonne(),
        C::Coefficient.colonne(),
        C::Montant.titree("Réduction"),
        C::Urssaf.colonne(),
        C::Chomage.colonne(),
        C::Retraite.colonne(),
    ];
    t.sources = vec!["art. L241-13 et D241-7 du Code de la sécurité sociale ; le calcul est celui du bulletin".to_string()];
    finir(ctx, &mut t, Some(&p), "reduction-generale", d.format,false)?;
    // Un SMIC de reference ne s'additionne pas.
    for tot in t.sections.iter_mut().map(|s| &mut s.total).chain(std::iter::once(&mut t.total)) {
        if let Some(x) = tot.as_mut() {
            x.rien(C::Smic);
        }
    }
    if d.ecrire {
        ecrire_fini(ctx, &mut t, Some(&p), "reduction-generale")?;
    }
    Ok(t)
}

// ===================================================== 10. les assiettes

/// Les assiettes que l'etat nomme, dans son ordre ; les autres suivent sous leur nom.
const ASSIETTES: [(Assiette, &str); 8] = [
    (Assiette::Brut, "Brut"),
    (Assiette::BrutCs, "Assiette Congés Spectacles"),
    (Assiette::Csg, "Assiette CSG/CRDS"),
    (Assiette::BrutX115, "Brut majoré 115 %"),
    (Assiette::BrutX110, "Brut majoré 110 %"),
    (Assiette::Apec, "Assiette APEC"),
    (Assiette::Cet, "Assiette CET"),
    (Assiette::BrutDfs, "Brut après DFS"),
];

/// Les assiettes de cotisation de chaque paie de la periode, et son plafond.
pub fn assiettes(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let (avert, mut lignes) = (Vec::new(), Vec::new());
    let mut lecture = Lecture::nouvelle(ctx, &d.departs);
    let mut vues: BTreeSet<Assiette> = BTreeSet::new();
    for m in p.mois.clone() {
        for paie in lecture.paies(m)?.iter() {
            let mut l = Ligne::nouvelle();
            l.m(C::Mois, m).t(C::Salarie, qui(paie.salarie())?).ot(C::Contrat, paie.engagement().numero.clone());
            l.om(C::Plafond, paie.plafond().map(Montant::au_centime));
            for (k, v) in paie.bases() {
                l.assiette(*k, Some(v.au_centime()))?;
                vues.insert(*k);
            }
            if !paie.bases().contains_key(&Assiette::Csg) {
                l.assiette(Assiette::Csg, paie.base_csg()?.map(|m| engagement::calcul::BaseDeCotisation::Montant(m.au_centime())))?;
                vues.insert(Assiette::Csg);
            }
            lignes.push(l);
        }
    }
    let mut cols = vec![C::Mois.colonne(), C::Salarie.colonne(), C::Contrat.colonne()];
    for (k, lib) in ASSIETTES {
        if vues.contains(&k) {
            cols.push(C::DeLAssiette(k).titree(lib));
        }
    }
    for k in &vues {
        if !ASSIETTES.iter().any(|(a, _)| a == k) {
            cols.push(C::DeLAssiette(*k).colonne());
        }
    }
    cols.push(C::Plafond.colonne());
    let mut t = Tableau::nouveau(EtatDExport::Assiettes, "Assiettes de cotisation", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.avertissements.extend(lecture.avertissements());
    t.colonnes = cols;
    t.sections = sections_de(lignes, |l| l.mois(C::Mois), section_du_mois, Some(&|l| cle_tri(l, &[C::Salarie, C::Contrat])))?;
    t.sources = vec![
        "assiettes du calcul, déclarées telles quelles en DSN (S21.G00.78) ; plafond : arrêté annuel du plafond de la sécurité sociale, proratisé comme le bulletin l'applique".to_string(),
    ];
    finir(ctx, &mut t, Some(&p), "assiettes", d.format,d.ecrire)?;
    Ok(t)
}

// =============================================== 11. les heures et les cachets

/// Contrat par contrat, cachets, heures retenues pour les annexes 8 et 10,
/// jours travailles, brut, lus dans les paies ecrites des engagements valides.
pub fn heures_et_cachets(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, false)?;
    let mut lignes = Vec::new();
    let mut lecture = Lecture::nouvelle(ctx, &d.departs);
    let avert = Vec::new();
    for m in p.mois.clone() {
        for paie in lecture.paies(m)?.iter() {
            let e = paie.engagement();
            // Les cachets et les jours que la paie compte (`PaieDuMois::activite` : pour une paie importee, ce que les
            // pieces disent du mois, une mesure tue restant vide) ; les heures pour les annexes, celles de ses lignes
            // (une paie importee dont aucune ligne n'est rattachee a une rubrique ne les dit pas).
            let (activite, heures) = (paie.activite(), paie.heures_annexes());
            let emploi = e.emploi.ecrit.emploi.clone().filter(|x| !x.is_empty()).or_else(|| paie.salarie().emploi.en_texte());
            let debut = e.contrat().debut().iso();
            // La fin du contrat, sinon son dernier jour travaille.
            let fin = match e.contrat().fin() {
                Some(f) => Some(f.iso()),
                None => e.dates_travail.last().map(|j| j.iso()),
            };
            let mut l = Ligne::nouvelle();
            l.m(C::Mois, m).t(C::Salarie, qui(paie.salarie())?).t(C::CleSalarie, paie.salarie().id.to_string());
            l.ot(C::Contrat, e.numero.clone()).ot(C::Spectacle, titre_spectacle(paie)).ot(C::Emploi, emploi).t(C::Debut, jj(&debut)).ot(C::Fin, fin.as_deref().map(jj));
            l.grandeur_ou_rien(C::Cachets, activite.cachets).grandeur_ou_rien(C::Heures, heures).grandeur_ou_rien(C::Jours, activite.jours).om(C::Brut, paie.brut().map(Montant::au_centime));
            lignes.push(l);
        }
    }
    // Des paies que l'etat ne compte pas encore (`PaieDeCote`) : il repond, vide s'il le faut, et dit pourquoi.
    if lignes.is_empty() && lecture.avertissements().is_empty() {
        return Err(refus(format!("Aucun contrat validé sur la période ({}).", p.libelle)));
    }
    let mut t = Tableau::nouveau(EtatDExport::HeuresEtCachets, "Heures et cachets (annexes 8 et 10)", p.libelle.clone());
    t.mois = lignes.iter().filter_map(|l| l.mois(C::Mois)).collect::<BTreeSet<_>>().into_iter().collect();
    t.sections = sections_de(lignes, |l| (l.texte(C::Salarie), l.texte(C::CleSalarie)), |k| Ok(k.0.clone()), Some(&|l| cle_tri(l, &[C::Mois, C::Debut, C::Contrat])))?;
    t.colonnes = vec![
        C::Mois.colonne(),
        C::Salarie.colonne(),
        C::Contrat.colonne(),
        C::Spectacle.colonne(),
        C::Emploi.colonne(),
        C::Debut.colonne(),
        C::Fin.colonne(),
        C::Cachets.colonne(),
        C::Heures.titree("Heures (annexes)"),
        C::Jours.colonne(),
        C::Brut.colonne(),
    ];
    t.sources = vec![
        "règlement d'assurance chômage, annexes VIII et X (décret n° 2019-797 modifié) : un cachet compte pour les heures que la rubrique déclare (heures_annexes) ; AEM : attestation employeur mensuelle déposée à France Travail".to_string(),
    ];
    t.avertissements = avert;
    t.avertissements.extend(lecture.avertissements());
    finir(ctx, &mut t, Some(&p), "heures-et-cachets", d.format,d.ecrire)?;
    Ok(t)
}

// =============================================== 12. le cout par spectacle

/// Chaque spectacle, salarie par salarie : deux homonymes, chacun par la cle de sa fiche, font deux lignes.
pub fn cout_par_spectacle(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let lignes = lignes_livre(ctx, &p, &mut avert, &d.departs)?;
    let spectacle_de = |l: &LigneLivre| if l.spectacle.is_empty() { "(sans spectacle)".to_string() } else { l.spectacle.clone() };
    let mut a_trier = Vec::new();
    for ((sp, cle), c) in cumuls_par(&lignes, |l| Some((spectacle_de(l), l.cle_salarie.clone()))) {
        let mut ligne = Ligne::nouvelle();
        let nom = premier_nom(&lignes, |l| spectacle_de(l) == sp && l.cle_salarie == cle)?;
        ligne.t(C::Spectacle, sp).t(C::Salarie, nom).t(C::CleSalarie, cle.to_string()).compte(C::Bulletins, c.bulletins);
        // Une somme qu'aucun bulletin ne dit reste vide.
        for (champ, s) in [(C::Brut, c.brut), (C::CotisationsPatronales, c.cotisations_patronales), (C::CoutEmployeur, c.cout_employeur), (C::NetAPayer, c.net_a_payer)] {
            if let Some(v) = s.si_dite() {
                ligne.montant(champ, v);
            }
        }
        a_trier.push(ligne);
    }
    let mut t = Tableau::nouveau(EtatDExport::CoutParSpectacle, "Coût employeur par spectacle", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(a_trier, |l| l.texte(C::Spectacle), |k| Ok(k.clone()), Some(&|l| cle_tri(l, &[C::Salarie, C::CleSalarie])))?;
    t.colonnes = vec![
        C::Spectacle.colonne(),
        C::Salarie.colonne(),
        C::Bulletins.colonne(),
        C::Brut.colonne(),
        C::CotisationsPatronales.titree("Charges patronales"),
        C::CoutEmployeur.colonne(),
        C::NetAPayer.titree("Net versé"),
    ];
    t.sources = vec!["bulletins rattachés au spectacle de leur engagement".to_string()];
    finir(ctx, &mut t, Some(&p), "cout-par-spectacle", d.format,d.ecrire)?;
    Ok(t)
}
