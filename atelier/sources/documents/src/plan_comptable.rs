//! Le plan comptable et les tables des exports : le plan livre (`plan-comptable-base.json`) recouvert par
//! `config/plan-comptable.json`, le sous-compte 421 d'un salarie (le modele `net.par_salarie`, un
//! modèle à champs nommés que ce module rend ou refuse), le contrôle du plan avant un export vers le
//! logiciel du comptable, et les tables fixes des exports (`exports_tables.json`).
//!
//! Un modele de compte individuel qui emploie une forme que ce module ne rend pas -- groupement des milliers avec
//! zeros
//! (`{code:010,}`), types flottants (`{code:e}`, `f`, `g`, `%`), methodes ou attributs de `str`/`int` autres que
//! `real`, `imag`, `numerator`, `denominator` -- est refuse (`Refuse`) en le disant.

use droit::fiche::Lue as _;
use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

use serde::{Deserialize, Deserializer, Serialize};

use droit::contexte::Contexte;
use droit::forme_bareme::IdDeCotisation;
use engagement::ecarts::RegroupementDeComparaison;
use socle::echec::{erreur, Classe, Echec, R};

use crate::ecritures_comptables::{CodeJournal, NumeroDeCompte, PosteAnalytique};
use socle::fichiers::repr;

// ------------------------------------------------------------------ les listes des exports

/// UNE LISTE DU LOGICIEL que les tables des exports (`exports_tables.json`), la charge d'`exporter` et ses lecteurs
/// nomment : chaque valeur s'ecrit, se lit et se trie par son texte ; une autre se refuse a sa lecture.
macro_rules! liste_des_exports {
    ($(#[$m:meta])* pub enum $nom:ident, $quoi:literal { $( $var:ident = $txt:literal ),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $nom { $( $var ),* }

        impl $nom {
            /// Son texte.
            pub fn as_str(self) -> &'static str {
                match self { $( $nom::$var => $txt ),* }
            }

            /// Celle qu'ecrit `texte`, ou la phrase qui le refuse.
            pub fn lire(texte: &str) -> R<$nom> {
                match texte {
                    $( $txt => Ok($nom::$var), )*
                    _ => Err(socle::refus!(concat!($quoi, " : « {} »."), texte)),
                }
            }
        }

        impl std::fmt::Display for $nom {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl Ord for $nom {
            fn cmp(&self, autre: &Self) -> std::cmp::Ordering {
                self.as_str().cmp(autre.as_str())
            }
        }

        impl PartialOrd for $nom {
            fn partial_cmp(&self, autre: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(autre))
            }
        }

        impl Serialize for $nom {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $nom {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let texte = String::deserialize(d)?;
                $nom::lire(&texte).map_err(|e| serde::de::Error::custom(e.phrase))
            }
        }
    };
}

liste_des_exports! {
    /// UN ETAT QUE LA PAGE DES EXPORTS PROPOSE (`etats` et `au_mois` des tables) : ce qu'`exporter` fait, ce qu'une
    /// convention ou la taxe sur les salaires reserve, l'etat d'un tableau rendu.
    pub enum EtatDExport, "État inconnu" {
        LivreDePaie = "livre-de-paie",
        LivreDePaieDetaille = "livre-de-paie-detaille",
        RecapitulatifAnnuel = "recapitulatif-annuel",
        MasseSalariale = "masse-salariale",
        EtatDesVirements = "etat-des-virements",
        PrelevementALaSource = "prelevement-a-la-source",
        CotisationsParOrganisme = "cotisations-par-organisme",
        VersementsOrganismes = "versements-organismes",
        BordereauUrssaf = "bordereau-urssaf",
        ReductionGenerale = "reduction-generale",
        Assiettes = "assiettes",
        RegularisationCascSvp = "regularisation-casc-svp",
        FcapSvp = "fcap-svp",
        Afdas2642 = "afdas-2642",
        ParitarismeBranche = "paritarisme-branche",
        EcrituresComptables = "ecritures-comptables",
        GrandLivrePaie = "grand-livre-paie",
        BalancePaie = "balance-paie",
        TaxeSurLesSalaires = "taxe-sur-les-salaires",
        CoutParSpectacle = "cout-par-spectacle",
        HeuresEtCachets = "heures-et-cachets",
        Contrats = "contrats",
        RegistreDuPersonnel = "registre-du-personnel",
        FichierDuPersonnel = "fichier-du-personnel",
        JournalDesDeclarations = "journal-des-declarations",
        ProvisionConges = "provision-conges",
    }
}

liste_des_exports! {
    /// LE FORMAT D'UN EXPORT : les six de la machinerie des tableaux (`tableaux_export::FORMATS_TABLEAU`), puis ceux des
    /// logiciels du comptable (`etats_compta::FORMATS_ECRITURES`).
    pub enum FormatDExport, "Format inconnu" {
        Csv = "csv",
        Tsv = "tsv",
        Xlsx = "xlsx",
        Pdf = "pdf",
        Html = "html",
        Json = "json",
        Fec = "fec",
        Ximport = "ximport",
        Oxygene = "oxygene",
        Sage = "sage",
        Quadra = "quadra",
        Ebp = "ebp",
        Pennylane = "pennylane",
    }
}

liste_des_exports! {
    /// LE REGROUPEMENT D'UN ETAT : les groupes du livre de paie (par salarie, par mois, par spectacle), ceux des
    /// cotisations (par organisme, par mois).
    pub enum RegroupementDEtat, "Regroupement inconnu" {
        Mois = "mois",
        Organisme = "organisme",
        Salarie = "salarie",
        Spectacle = "spectacle",
    }
}

liste_des_exports! {
    /// LE LOGICIEL DU COMPTABLE (`logiciels` des tables) : il fixe le format des ecritures comptables ; `config/compta.json`
    /// garde celui que la compagnie a retenu.
    pub enum LogicielComptable, "Logiciel inconnu" {
        Oxygene = "oxygene",
        Sage = "sage",
        Quadra = "quadra",
        Ciel = "ciel",
        Ebp = "ebp",
        Pennylane = "pennylane",
        Paheko = "paheko",
        Acd = "acd",
        Loop = "loop",
        Isacompta = "isacompta",
        Autre = "autre",
    }
}

liste_des_exports! {
    /// LA PERIODE QU'UN ETAT PROPOSE (`periodes` d'un etat) : une annee, un mois, toutes les annees.
    pub enum PeriodeDExport, "Période inconnue" {
        Annee = "annee",
        Mois = "mois",
        Toutes = "toutes",
    }
}

liste_des_exports! {
    /// LA COLONNE D'UN ETAT que le livre de paie totalise (`etats_paie::CUMULABLES`) : sa cle, sous laquelle le total
    /// rendu la nomme, et sous laquelle se compte ce qu'un bulletin ne declare pas.
    pub enum ColonneDEtat, "Colonne inconnue" {
        Brut = "brut",
        CotisationsSalariales = "cotisations_salariales",
        CotisationsPatronales = "cotisations_patronales",
        NetImposable = "net_imposable",
        NetAPayer = "net_a_payer",
        Pas = "pas",
        CoutEmployeur = "cout_employeur",
    }
}

// ------------------------------------------------------------------ les tables des exports

/// LES TABLES FIXES DES EXPORTS ET DES ECRITURES COMPTABLES (`exports_tables.json`) : les etats que la page propose,
/// ceux qu'une convention ou des permanents reservent, les formats, les regroupements, les logiciels du comptable et
/// les largeurs de leurs zones.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tables {
    pub quoi: String,
    etats: Vec<EtatLivre>,
    /// LA PIECE D'UN SALARIE AU MOIS (`droit::fiche::Piece::Permanent`) : les etats qui ne lisent que ses paies (la dette
    /// de conges payes : `engagement::conges::provision_conges_de`), ranges a part. Leur place dit leur piece.
    au_mois: Vec<EtatLivre>,
    /// L'etat -> les conventions (IDCC) qui le demandent.
    pub si_convention: BTreeMap<EtatDExport, Vec<droit::fiche::Idcc>>,
    /// Les etats qui ne servent qu'a une compagnie qui doit la taxe sur les salaires une des annees a exporter.
    pub si_taxe_sur_les_salaires: Vec<EtatDExport>,
    pub formats: Vec<FormatLivre>,
    pub regroupements: Vec<RegroupementLivre>,
    pub logiciels: Vec<Logiciel>,
    /// Le format -> les largeurs de ses zones.
    pub limites: BTreeMap<FormatDExport, Limite>,
}

impl Tables {
    /// Chaque etat livre, dans l'ordre, avec sa piece : celle d'un salarie au mois pour ceux de `au_mois`.
    pub fn etats(&self) -> impl Iterator<Item = (&EtatLivre, Option<droit::fiche::Piece>)> {
        self.etats.iter().map(|e| (e, None)).chain(self.au_mois.iter().map(|e| (e, Some(droit::fiche::Piece::Permanent))))
    }
}

/// Un etat que la page propose : ce qu'il demande (`questions`), ses formats, ses regroupements, ses periodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EtatLivre {
    pub etat: EtatDExport,
    pub quoi: String,
    pub famille: String,
    pub questions: Vec<String>,
    pub bouton: String,
    pub apercu: bool,
    pub formats: Vec<FormatDExport>,
    pub regroupements: Vec<RegroupementDEtat>,
    pub periodes: Vec<PeriodeDExport>,
}

/// Un format de fichier, et ce qu'il est.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatLivre {
    pub format: FormatDExport,
    pub quoi: String,
}

/// Un regroupement du livre de paie, et ce qu'il est.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegroupementLivre {
    pub regroupement: RegroupementDEtat,
    pub quoi: String,
}

/// Un logiciel du comptable : son nom, le format de ses ecritures et pourquoi, ce format en clair, et s'il porte
/// l'analytique.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Logiciel {
    pub logiciel: LogicielComptable,
    pub nom: String,
    pub format: FormatDExport,
    pub pourquoi: String,
    pub format_lisible: String,
    pub analytique: bool,
}

/// Les largeurs des zones d'un format : le compte (absent : la notice ne le borne pas) et le journal.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limite {
    pub nom: String,
    pub compte: Option<usize>,
    pub journal: usize,
}

/// Les tables fixes des exports et des ecritures comptables.
pub fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| match socle::forme::lire_texte(include_str!("exports_tables.json"), "Les tables des exports livrées ne se lisent pas : le signaler au développeur.") {
        Ok(t) => t,
        Err(m) => unreachable!("exports_tables.json : {}", m.phrase),
    })
}

/// Les logiciels du comptable que les ecritures comptables connaissent.
pub fn logiciels_connus() -> Vec<LogicielComptable> {
    tables().logiciels.iter().map(|l| l.logiciel).collect()
}


/// Le plan comptable propre a la compagnie, sous sa racine.
pub const PLAN_COMPTABLE: &str = "config/plan-comptable.json";

/// UN OBJET DONT L'ORDRE COMPTE, lu dans l'ordre du fichier (`socle::forme::en_ordre`) : les lignes, les tiers et les
/// retenues du plan, ou la premiere ligne qui porte un compte lui donne son libelle. `K` : sa cle, un texte (une
/// rubrique), un compte (`NumeroDeCompte`), la cotisation d'une ligne du bulletin (`IdDeCotisation`) ou son
/// regroupement (`RegroupementDeComparaison`), lue a la porte.
#[derive(Debug, Clone)]
pub struct Ordonne<T, K = String>(pub Vec<(K, T)>);

impl<'de, T: Deserialize<'de>, K: Deserialize<'de> + PartialEq + std::fmt::Display> Deserialize<'de> for Ordonne<T, K> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        socle::forme::en_ordre(d).map(Ordonne)
    }
}

impl<T, K: PartialEq> Ordonne<T, K> {
    pub fn get<Q: PartialEq + ?Sized>(&self, cle: &Q) -> Option<&T>
    where
        K: std::borrow::Borrow<Q>,
    {
        self.0.iter().find(|(k, _)| k.borrow() == cle).map(|(_, v)| v)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &T)> {
        self.0.iter().map(|(k, v)| (k, v))
    }

    /// Les entrees de `autre` remplacent les siennes a leur place ; les nouvelles s'ajoutent a la fin.
    fn recouvrir(&mut self, autre: Ordonne<T, K>) {
        for (k, v) in autre.0 {
            match self.0.iter_mut().find(|(x, _)| *x == k) {
                Some(place) => place.1 = v,
                None => self.0.push((k, v)),
            }
        }
    }
}

/// Une cle de `lignes` qui n'est pas une cotisation (un regroupement ou un total d'un bulletin), en une phrase.
fn hors_des_cotisations(lignes: &Ordonne<RegleLigne, IdDeCotisation>) -> Option<String> {
    lignes.iter().find_map(|(k, _)| match RegroupementDeComparaison::lire(k.as_str()) {
        Ok(RegroupementDeComparaison::Cotisation(_)) => None,
        Ok(_) => Some(format!("« {} » nomme un regroupement ou un total d'un bulletin, pas une cotisation : un regroupement a sa place sous « regroupements », pas sous « lignes ».", k)),
        Err(e) => Some(e.phrase),
    })
}

/// Une cle de `regroupements` qui n'est pas un regroupement des lignes d'un bulletin importe, en une phrase.
fn hors_des_regroupements(regroupements: &Ordonne<RegleLigne, RegroupementDeComparaison>) -> Option<String> {
    regroupements.iter().find_map(|(k, _)| match k {
        RegroupementDeComparaison::Autres | RegroupementDeComparaison::Convention | RegroupementDeComparaison::Apec => None,
        RegroupementDeComparaison::Cotisation(_) | RegroupementDeComparaison::Total(_) => Some(format!(
            "« {} » n'est pas un regroupement des lignes d'un bulletin importé (autres, convention, apec) : une cotisation a sa place sous « lignes », pas sous « regroupements ».",
            k
        )),
    })
}

/// `lignes` du plan : des cotisations seules.
fn cotisations_seules<'de, D: Deserializer<'de>>(d: D) -> Result<Ordonne<RegleLigne, IdDeCotisation>, D::Error> {
    let lignes = Ordonne::<RegleLigne, IdDeCotisation>::deserialize(d)?;
    match hors_des_cotisations(&lignes) {
        Some(faute) => Err(serde::de::Error::custom(faute)),
        None => Ok(lignes),
    }
}

/// `lignes` d'un plan propre, quand il les dit : des cotisations seules.
fn cotisations_seules_dites<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Ordonne<RegleLigne, IdDeCotisation>>, D::Error> {
    let lignes = Option::<Ordonne<RegleLigne, IdDeCotisation>>::deserialize(d)?;
    match lignes.as_ref().and_then(hors_des_cotisations) {
        Some(faute) => Err(serde::de::Error::custom(faute)),
        None => Ok(lignes),
    }
}

/// `regroupements` du plan : des regroupements seuls.
fn regroupements_seuls<'de, D: Deserializer<'de>>(d: D) -> Result<Ordonne<RegleLigne, RegroupementDeComparaison>, D::Error> {
    let regroupements = Ordonne::<RegleLigne, RegroupementDeComparaison>::deserialize(d)?;
    match hors_des_regroupements(&regroupements) {
        Some(faute) => Err(serde::de::Error::custom(faute)),
        None => Ok(regroupements),
    }
}

/// `regroupements` d'un plan propre, quand il les dit : des regroupements seuls.
fn regroupements_seuls_dits<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Ordonne<RegleLigne, RegroupementDeComparaison>>, D::Error> {
    let regroupements = Option::<Ordonne<RegleLigne, RegroupementDeComparaison>>::deserialize(d)?;
    match regroupements.as_ref().and_then(hors_des_regroupements) {
        Some(faute) => Err(serde::de::Error::custom(faute)),
        None => Ok(regroupements),
    }
}

/// La cle du plan qui dit les comptes d'une ligne de cotisation (`Plan::regle`) : `lignes.<cotisation>` ou
/// `regroupements.<regroupement>`.
pub fn cle_dans_le_plan(id: &RegroupementDeComparaison) -> String {
    match id {
        RegroupementDeComparaison::Cotisation(c) => format!("lignes.{}", c),
        autre => format!("regroupements.{}", autre),
    }
}

/// Un compte et son libelle.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compte {
    pub compte: NumeroDeCompte,
    pub libelle: String,
}

/// LE NET : le compte 421, son libelle, et le modele du sous-compte individuel (`par_salarie`), absent quand le
/// comptable tient un seul 421. Le modele n'est pas un compte : il en fait un par salarie (`compte_du_salarie`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompteNet {
    pub compte: NumeroDeCompte,
    pub libelle: String,
    pub par_salarie: Option<String>,
}

/// Une retenue (acompte, avance, saisie) : son compte, et son libelle (absent : celui de la rubrique).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retenue {
    pub compte: NumeroDeCompte,
    pub libelle: Option<String>,
}

/// Une ligne de bulletin : son compte de charge, son compte de tiers, et son libelle (absent : celui du bulletin).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegleLigne {
    pub charge: NumeroDeCompte,
    pub tiers: NumeroDeCompte,
    pub libelle: Option<String>,
}

/// L'ecriture de la provision pour conges payes.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisionConges {
    pub conges: Compte,
    pub dette: Compte,
    pub charges: Compte,
    pub dette_charges: Compte,
}

/// LE PLAN COMPTABLE de la compagnie : le plan livre (`plan-comptable-base.json`), recouvert par le sien. Les codes
/// analytiques des spectacles (`sections`) et le poste analytique d'Oxygene (`poste_analytique`) ne sont dits que par la
/// compagnie.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub quoi: Option<String>,
    pub note: Option<String>,
    pub source: Option<String>,
    pub journal: CodeJournal,
    /// Le libelle du journal, au FEC (`JournalLib`).
    pub journal_libelle: String,
    pub brut: Compte,
    pub net: CompteNet,
    pub frais: Compte,
    pub pas: Compte,
    pub retenues: Ordonne<Retenue>,
    /// Le compte d'un tiers -> son nom.
    pub tiers: Ordonne<String, NumeroDeCompte>,
    /// La cotisation d'une ligne du bulletin -> ses comptes.
    #[serde(deserialize_with = "cotisations_seules")]
    pub lignes: Ordonne<RegleLigne, IdDeCotisation>,
    /// LE REGROUPEMENT d'une ligne qu'un bulletin importe imprime pour plusieurs cotisations (`autres`, `convention`,
    /// `apec` : `engagement::ecarts::RegroupementDeComparaison`) -> ses comptes. A part des cotisations : un regroupement
    /// n'est pas une ligne du calcul.
    #[serde(deserialize_with = "regroupements_seuls")]
    pub regroupements: Ordonne<RegleLigne, RegroupementDeComparaison>,
    pub provision_conges: ProvisionConges,
    pub sections: Option<BTreeMap<String, String>>,
    pub poste_analytique: Option<PosteAnalytique>,
}

/// `config/plan-comptable.json` : ce que la compagnie change au plan livre. Chaque cle dite remplace la sienne ;
/// `lignes`, `tiers` et `retenues` se recouvrent entree par entree.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanPropre {
    quoi: Option<String>,
    note: Option<String>,
    source: Option<String>,
    journal: Option<CodeJournal>,
    journal_libelle: Option<String>,
    brut: Option<Compte>,
    net: Option<CompteNet>,
    frais: Option<Compte>,
    pas: Option<Compte>,
    retenues: Option<Ordonne<Retenue>>,
    tiers: Option<Ordonne<String, NumeroDeCompte>>,
    #[serde(default, deserialize_with = "cotisations_seules_dites")]
    lignes: Option<Ordonne<RegleLigne, IdDeCotisation>>,
    #[serde(default, deserialize_with = "regroupements_seuls_dits")]
    regroupements: Option<Ordonne<RegleLigne, RegroupementDeComparaison>>,
    provision_conges: Option<ProvisionConges>,
    sections: Option<BTreeMap<String, String>>,
    poste_analytique: Option<PosteAnalytique>,
}

impl Plan {
    fn recouvrir(&mut self, p: PlanPropre) {
        fn poser<T>(cible: &mut T, v: Option<T>) {
            if let Some(v) = v {
                *cible = v;
            }
        }
        fn poser_dit<T>(cible: &mut Option<T>, v: Option<T>) {
            if v.is_some() {
                *cible = v;
            }
        }
        poser_dit(&mut self.quoi, p.quoi);
        poser_dit(&mut self.note, p.note);
        poser_dit(&mut self.source, p.source);
        poser(&mut self.journal, p.journal);
        poser(&mut self.journal_libelle, p.journal_libelle);
        poser(&mut self.brut, p.brut);
        poser(&mut self.net, p.net);
        poser(&mut self.frais, p.frais);
        poser(&mut self.pas, p.pas);
        poser(&mut self.provision_conges, p.provision_conges);
        poser_dit(&mut self.sections, p.sections);
        poser_dit(&mut self.poste_analytique, p.poste_analytique);
        if let Some(r) = p.retenues {
            self.retenues.recouvrir(r);
        }
        if let Some(t) = p.tiers {
            self.tiers.recouvrir(t);
        }
        if let Some(l) = p.lignes {
            self.lignes.recouvrir(l);
        }
        if let Some(r) = p.regroupements {
            self.regroupements.recouvrir(r);
        }
    }

    /// Les comptes d'une ligne de cotisation d'une paie du mois : ceux de sa cotisation (`lignes`), ou ceux de son
    /// regroupement (`regroupements`) ; aucun si le plan ne la nomme pas.
    pub fn regle(&self, id: &RegroupementDeComparaison) -> Option<&RegleLigne> {
        match id {
            RegroupementDeComparaison::Cotisation(c) => self.lignes.get(c),
            autre => self.regroupements.get(autre),
        }
    }

    /// Le libelle du tiers `compte`, s'il est dit (et non vide).
    pub fn nom_du_tiers(&self, compte: &NumeroDeCompte) -> Option<&str> {
        self.tiers.get(compte).map(String::as_str).filter(|n| !n.is_empty())
    }
}

/// Le plan comptable de la compagnie, et d'ou il vient (`config/plan-comptable.json` ou le plan livre).
pub fn plan(ctx: &Contexte) -> R<(Plan, &'static str)> {
    let p = plan_comptable(ctx)?;
    let origine = if ctx.racine.join(PLAN_COMPTABLE).exists() { PLAN_COMPTABLE } else { "plan livré avec le logiciel (plan-comptable-base.json)" };
    Ok((p, origine))
}

/// Le plan livre, recouvert par `config/plan-comptable.json`.
pub fn plan_comptable(ctx: &Contexte) -> R<Plan> {
    let mut plan: Plan = socle::forme::lire_fichier(&ctx.code.join("plan-comptable-base.json"), "Le plan comptable livré ne se lit pas : le signaler au développeur.")
        .map_err(|m| m.reclasser(Classe::Illisible))?;
    let propre = ctx.racine.join(PLAN_COMPTABLE);
    if propre.exists() {
        let p: PlanPropre = socle::forme::lire_fichier(&propre, "Le plan comptable de la compagnie (config/plan-comptable.json) ne se lit pas : le corriger.")
            .map_err(|m| m.reclasser(Classe::Illisible))?;
        plan.recouvrir(p);
    }
    Ok(plan)
}

/// Ce qui, dans le plan comptable, ne
/// passerait pas dans ce format -- avant l'export.
pub fn controler_plan(ctx: &Contexte, format: FormatDExport) -> R<Vec<String>> {
    // Un format dont les tables ne bornent pas les zones n'a rien qui depasse.
    Ok(tables().limites.get(&format).map(|lim| controler_contre(ctx, format, lim)).transpose()?.into_iter().flatten().collect())
}

fn controler_contre(ctx: &Contexte, format: FormatDExport, lim: &Limite) -> R<Vec<String>> {
    let plan = plan_comptable(ctx)?;
    let (lim_journal, nom_format) = (lim.journal, lim.nom.as_str());
    let mut comptes: HashSet<NumeroDeCompte> = HashSet::new();
    for c in [&plan.brut.compte, &plan.net.compte, &plan.pas.compte] {
        comptes.insert(c.clone());
    }
    for regle in plan.lignes.iter().map(|(_, r)| r).chain(plan.regroupements.iter().map(|(_, r)| r)) {
        for c in [&regle.charge, &regle.tiers] {
            comptes.insert(c.clone());
        }
    }
    comptes.extend(plan.tiers.iter().map(|(k, _)| k.clone()));
    for (_, regle) in plan.retenues.iter() {
        comptes.insert(regle.compte.clone());
    }
    let mut msgs: Vec<String> = Vec::new();
    let mut fiches: Vec<(&droit::contexte::IdSalarie, &droit::fiche::FicheSalarie)> = ctx.salaries.iter().collect();
    fiches.sort_by(|a, b| a.0.cmp(b.0));
    for (cle, s) in fiches {
        match compte_du_salarie(&plan, s, cle) {
            Ok(Some(c)) => {
                comptes.insert(c);
            }
            Ok(None) => {}
            Err(Refus::Operation(m)) => msgs.push(m),
            Err(Refus::Autre(e)) => return Err(e),
        }
    }
    let journal = plan.journal.as_str();
    let mut tries: Vec<&NumeroDeCompte> = comptes.iter().collect();
    tries.sort();
    for c in tries.into_iter().map(NumeroDeCompte::as_str) {
        if let Some(lim_compte) = lim.compte.filter(|l| c.chars().count() > *l) {
            msgs.push(format!("{} : le compte « {} » a {} caractères, la zone en prend {}.", nom_format, c, c.chars().count(), lim_compte));
        } else if format == FormatDExport::Quadra && !c.chars().all(|x| x.is_ascii_digit()) {
            msgs.push(format!(
                "Quadra : le compte « {} » n'est pas numérique ; Quadra range les comptes de lettres parmi les tiers (clients, fournisseurs).",
                c
            ));
        }
    }
    if journal.chars().count() > lim_journal {
        msgs.push(format!("{} : le journal « {} » a {} caractères, la zone en prend {}.", nom_format, journal, journal.chars().count(), lim_journal));
    }
    if format == FormatDExport::Pennylane && !journal.chars().all(char::is_alphabetic) {
        msgs.push(format!("Pennylane : le code journal « {} » ne doit porter que des lettres.", journal));
    }
    Ok(msgs)
}

pub enum Refus {
    /// Un refus `Refuse` : `controler_plan` le rend en phrase.
    Operation(String),
    Autre(Echec),
}

/// Le sous-compte 421 d'un salarie, ou `None` si le plan n'en tient pas (`net.par_salarie` absent) ou que son modele se
/// remplit a vide (aucun sous-compte).
pub fn compte_du_salarie(plan: &Plan, salarie: &droit::fiche::ChampsSalarie, cle: &droit::contexte::IdSalarie) -> Result<Option<NumeroDeCompte>, Refus> {
    let Some(modele) = &plan.net.par_salarie else { return Ok(None) };
    if modele.is_empty() {
        return Err(Refus::Operation(
            "Le modèle du compte individuel (net.par_salarie du plan comptable) est vide : le retirer pour un seul 421, ou l'écrire (« 421{code:03} », « 421{cle} »).".to_string(),
        ));
    }
    let code = match salarie.code_salarie.portee() {
        Some(t) => entier_de(t.trim()).map_err(|e| {
            Refus::Operation(format!("Le code salarié de {} : « {} » n'est pas un entier que le modèle sait écrire ({}).", cle, t.trim(), e))
        })?,
        None => None,
    };
    let rempli = formater(modele, code, cle.as_str(), 0).map_err(|(classe, message)| {
        // `compte_du_salarie` ne rattrape qu'une valeur illisible ou qui manque ; le reste traverse.
        match classe {
            Classe::Illisible | Classe::Manque => {}
            Classe::FormeNonPortee => {
                return Refus::Autre(Echec {
                    a_part: Some(socle::echec::Lieu { cle: Some("net.par_salarie".to_string()), detail: Some(message), ..socle::echec::Lieu::default() }.into()),
                    ..erreur(
                        Classe::Refuse,
                        format!(
                            "Le modèle du compte individuel « {} » emploie une forme que le logiciel ne sait pas remplir : l'écrire autrement dans le plan comptable.",
                            modele
                        ),
                    )
                })
            }
            autre => return Refus::Autre(erreur(autre, message)),
        }
        Refus::Operation(format!(
            "Le modèle du compte individuel « {} » (net.par_salarie du plan comptable) ne se remplit pas pour {} : {}. Modèles possibles : « 421{{code:03}} », « 421{{cle}} ».",
            modele,
            cle,
            message
        ))
    })?;
    if rempli.is_empty() {
        return Ok(None);
    }
    match NumeroDeCompte::lire(&rempli) {
        Ok(c) => Ok(Some(c)),
        Err(e) => Err(Refus::Operation(format!(
            "Le modèle du compte individuel « {} » (net.par_salarie du plan comptable) donne pour {} le compte « {} », que le logiciel ne prend pas : {}",
            modele, cle, rempli, e.phrase
        ))),
    }
}

/// Un entier écrit : le signe, les chiffres, un tiret bas entre deux chiffres ; `None` pour un code qui n'est pas un
/// entier (il se lit alors en texte). Un entier trop grand se refuse.
fn entier_de(t: &str) -> Result<Option<i64>, std::num::ParseIntError> {
    let (neg, reste) = if let Some(r) = t.strip_prefix('-') {
        (true, r)
    } else if let Some(r) = t.strip_prefix('+') {
        (false, r)
    } else {
        (false, t)
    };
    let bords_sans_tiret = reste.chars().next().is_some_and(|c| c != '_') && reste.chars().last().is_some_and(|c| c != '_');
    if !bords_sans_tiret || reste.contains("__") {
        return Ok(None);
    }
    let chiffres: String = reste.chars().filter(|c| *c != '_').collect();
    if !chiffres.chars().all(|c| c.is_ascii_digit()) {
        return Ok(None);
    }
    chiffres.parse::<i64>().map(|n| Some(if neg { -n } else { n }))
}

// ------------------------------------------------------------- str.format

/// `FormeNonPortee` : une forme que ce module refuse.
type Faute = (Classe, String);

/// `code` est l'entier de la fiche, ou "" s'il
/// n'en a pas. Refuse, sous la classe de l'erreur (`Illisible`, `Manque`...) et une phrase en francais, en suivant `MarkupIterator_next` et
/// `parse_field`.
fn formater(modele: &str, code: Option<i64>, cle: &str, profondeur: u32) -> Result<String, Faute> {
    if profondeur >= 2 {
        return Err((Classe::Illisible, "mise en forme imbriquée trop profondément".into()));
    }
    let c: Vec<char> = modele.chars().collect();
    let mut sortie = String::new();
    let mut i = 0;
    let mut auto: Option<bool> = None; // Some(true) : numerotation automatique, Some(false) : manuelle
    let mut prochain = 0usize;
    while i < c.len() {
        let x = c[i];
        i += 1;
        match x {
            '{' | '}' => {
                let au_bout = i >= c.len();
                if x == '}' && (au_bout || c[i] != '}') {
                    return Err((Classe::Illisible, "« } » seule".into()));
                }
                if au_bout && x == '{' {
                    return Err((Classe::Illisible, "« { » seule".into()));
                }
                if c[i] == x {
                    sortie.push(x);
                    i += 1;
                    continue;
                }
                let (nom, spec, conversion, a_developper) = lire_champ(&c, &mut i)?;
                sortie.push_str(&remplir(&nom, &spec, conversion, a_developper, code, cle, profondeur, &mut auto, &mut prochain)?);
            }
            x => sortie.push(x),
        }
    }
    Ok(sortie)
}

/// `parse_field` : le nom, la specification, la conversion, et si la specification
/// porte des champs a developper. `i` est apres l'accolade ouvrante.
fn lire_champ(c: &[char], i: &mut usize) -> Result<(String, String, Option<char>, bool), Faute> {
    let debut = *i;
    let mut dernier = '\0';
    while *i < c.len() {
        dernier = c[*i];
        *i += 1;
        match dernier {
            '{' => return Err((Classe::Illisible, "« { » inattendue dans un nom de champ".into())),
            '[' => {
                while *i < c.len() && c[*i] != ']' {
                    *i += 1;
                }
                continue;
            }
            '}' | ':' | '!' => break,
            _ => continue,
        }
    }
    let nom: String = c[debut..(*i).saturating_sub(1).max(debut)].iter().collect();
    if dernier == '!' || dernier == ':' {
        let mut conversion = None;
        if dernier == '!' {
            if *i >= c.len() {
                return Err((Classe::Illisible, "conversion inachevée après « ! »".into()));
            }
            conversion = Some(c[*i]);
            *i += 1;
            if *i < c.len() {
                let x = c[*i];
                *i += 1;
                if x == '}' {
                    return Ok((nom, String::new(), conversion, false));
                }
                if x != ':' {
                    return Err((Classe::Illisible, "« : » attendu après la conversion".into()));
                }
            }
        }
        let debut_spec = *i;
        let (mut niveau, mut a_developper) = (1, false);
        while *i < c.len() {
            let x = c[*i];
            *i += 1;
            match x {
                '{' => {
                    a_developper = true;
                    niveau += 1;
                }
                '}' => {
                    niveau -= 1;
                    if niveau == 0 {
                        return Ok((nom, c[debut_spec..*i - 1].iter().collect(), conversion, a_developper));
                    }
                }
                _ => {}
            }
        }
        return Err((Classe::Illisible, "« { » sans « } » dans la mise en forme".into()));
    }
    if dernier != '}' {
        return Err((Classe::Illisible, "« } » attendue avant la fin".into()));
    }
    Ok((nom, String::new(), None, false))
}

enum Objet {
    Entier(i64),
    Texte(String),
}

/// Les attributs que l'entier et le texte portent et que ce modele ne sait
/// pas rendre (une methode se montre par son adresse en memoire).
const METHODES_ENTIER: [&str; 9] = ["as_integer_ratio", "bit_count", "bit_length", "conjugate", "from_bytes", "to_bytes", "is_integer", "__class__", "__doc__"];
const METHODES_TEXTE: [&str; 47] = [
    "capitalize", "casefold", "center", "count", "encode", "endswith", "expandtabs", "find", "format", "format_map", "index", "isalnum", "isalpha", "isascii",
    "isdecimal", "isdigit", "isidentifier", "islower", "isnumeric", "isprintable", "isspace", "istitle", "isupper", "join", "ljust", "lower", "lstrip",
    "maketrans", "partition", "removeprefix", "removesuffix", "replace", "rfind", "rindex", "rjust", "rpartition", "rsplit", "rstrip", "split", "splitlines",
    "startswith", "strip", "swapcase", "title", "translate", "upper", "zfill",
];

/// `get_field_object` : le champ `nom` (`code`, `cle`, un numero, avec ses `.attribut`
/// et ses `[indice]`).
fn resoudre(nom: &str, code: Option<i64>, cle: &str, auto: &mut Option<bool>, prochain: &mut usize) -> Result<Objet, Faute> {
    let c: Vec<char> = nom.chars().collect();
    let fin_premier = c.iter().take_while(|x| **x != '.' && **x != '[').count();
    let premier: String = c[..fin_premier].iter().collect();
    let mut objet = if premier.is_empty() || premier.chars().all(|x| x.is_ascii_digit()) {
        let n = if premier.is_empty() {
            if *auto == Some(false) {
                return Err((Classe::Illisible, "champs numérotés et non numérotés mêlés".into()));
            }
            *auto = Some(true);
            *prochain += 1;
            (*prochain - 1).to_string()
        } else {
            if *auto == Some(true) {
                return Err((Classe::Illisible, "champs numérotés et non numérotés mêlés".into()));
            }
            *auto = Some(false);
            match premier.trim_start_matches('0') {
                "" => "0".to_string(),
                n => n.to_string(),
            }
        };
        return Err((Classe::Manque, format!("champ {{{}}} sans valeur (seuls {{code}} et {{cle}} en ont une)", n)));
    } else if premier == "code" {
        match code {
            Some(n) => Objet::Entier(n),
            None => Objet::Texte(String::new()),
        }
    } else if premier == "cle" {
        Objet::Texte(cle.to_string())
    } else {
        return Err((Classe::Manque, format!("champ {} inconnu (seuls code et cle en ont un)", repr(&premier))));
    };
    let mut i = fin_premier;
    while i < c.len() {
        if c[i] == '.' {
            i += 1;
            let debut = i;
            while i < c.len() && c[i] != '.' && c[i] != '[' {
                i += 1;
            }
            let attribut: String = c[debut..i].iter().collect();
            if attribut.is_empty() {
                return Err((Classe::Illisible, "attribut vide".into()));
            }
            objet = match (&objet, attribut.as_str()) {
                (Objet::Entier(n), "real") | (Objet::Entier(n), "numerator") => Objet::Entier(*n),
                (Objet::Entier(_), "imag") => Objet::Entier(0),
                (Objet::Entier(_), "denominator") => Objet::Entier(1),
                (Objet::Entier(_), a) if METHODES_ENTIER.contains(&a) => return Err((Classe::FormeNonPortee, format!("l'attribut « {} » d'un entier", a))),
                (Objet::Texte(_), a) if METHODES_TEXTE.contains(&a) || a.starts_with("__") => {
                    return Err((Classe::FormeNonPortee, format!("l'attribut « {} » d'un texte", a)))
                }
                (Objet::Entier(_), a) => return Err((Classe::Illisible, format!("un nombre entier n'a pas d'attribut « {} »", a))),
                (Objet::Texte(_), a) => return Err((Classe::Illisible, format!("un texte n'a pas d'attribut « {} »", a))),
            };
        } else {
            // c[i] == '[' : `[indice]`
            i += 1;
            let debut = i;
            while i < c.len() && c[i] != ']' {
                i += 1;
            }
            if i >= c.len() {
                return Err((Classe::Illisible, "« ] » manquant".into()));
            }
            let indice: String = c[debut..i].iter().collect();
            i += 1;
            if indice.is_empty() {
                return Err((Classe::Illisible, "attribut vide".into()));
            }
            if i < c.len() && c[i] != '.' && c[i] != '[' {
                return Err((Classe::Illisible, "seuls « . » ou « [ » suivent « ] »".into()));
            }
            objet = match &objet {
                Objet::Entier(_) => return Err((Classe::MalFormee, "un nombre entier ne s'indexe pas".into())),
                Objet::Texte(t) => {
                    if !indice.chars().all(|x| x.is_ascii_digit()) {
                        return Err((Classe::MalFormee, "un texte ne s'indexe que par un nombre".into()));
                    }
                    match indice.parse::<usize>() {
                        Ok(n) => match t.chars().nth(n) {
                            Some(x) => Objet::Texte(x.to_string()),
                            None => return Err((Classe::Manque, format!("indice {} hors du texte", indice))),
                        },
                        Err(e) => return Err((Classe::Manque, format!("indice {} hors du texte ({})", indice, e))),
                    }
                }
            };
        }
    }
    Ok(objet)
}

/// `output_markup` : le champ resolu, converti, mis en forme.
fn remplir(
    nom: &str,
    spec: &str,
    conversion: Option<char>,
    a_developper: bool,
    code: Option<i64>,
    cle: &str,
    profondeur: u32,
    auto: &mut Option<bool>,
    prochain: &mut usize,
) -> Result<String, Faute> {
    let objet = resoudre(nom, code, cle, auto, prochain)?;
    let objet = match conversion {
        None => objet,
        Some('s') => Objet::Texte(match objet {
            Objet::Entier(n) => n.to_string(),
            Objet::Texte(t) => t,
        }),
        Some('r') | Some('a') => Objet::Texte(match objet {
            Objet::Entier(n) => n.to_string(),
            Objet::Texte(t) => {
                let r = repr(&t);
                if conversion == Some('a') {
                    r.chars()
                        .map(|x| match x as u32 {
                            0..=0x7f => x.to_string(),
                            0x80..=0xff => format!("\\x{:02x}", x as u32),
                            0x100..=0xffff => format!("\\u{:04x}", x as u32),
                            n => format!("\\U{:08x}", n),
                        })
                        .collect()
                } else {
                    r
                }
            }
        }),
        Some(x) => return Err((Classe::Illisible, format!("conversion « !{} » inconnue", x))),
    };
    let spec = if a_developper { formater(spec, code, cle, profondeur + 1)? } else { spec.to_string() };
    mettre_en_forme(&objet, &spec)
}

struct Specification {
    remplissage: char,
    alignement: Option<char>,
    signe: Option<char>,
    diese: bool,
    zero: bool,
    largeur: Option<usize>,
    groupe: Option<char>,
    precision: Option<usize>,
    genre: Option<char>,
}

fn lire_specification(spec: &str, nom_du_type: &str) -> Result<Specification, Faute> {
    let c: Vec<char> = spec.chars().collect();
    let invalide = || (Classe::Illisible, format!("mise en forme « {} » invalide pour {}", spec, socle::fichiers::nature(nom_du_type)));
    let est_align = |x: char| matches!(x, '<' | '>' | '=' | '^');
    let mut s = Specification { remplissage: ' ', alignement: None, signe: None, diese: false, zero: false, largeur: None, groupe: None, precision: None, genre: None };
    let mut i = 0;
    if c.len() >= 2 && est_align(c[1]) {
        s.remplissage = c[0];
        s.alignement = Some(c[1]);
        i = 2;
    } else if !c.is_empty() && est_align(c[0]) {
        s.alignement = Some(c[0]);
        i = 1;
    }
    if i < c.len() && matches!(c[i], '+' | '-' | ' ') {
        s.signe = Some(c[i]);
        i += 1;
    }
    if i < c.len() && c[i] == '#' {
        s.diese = true;
        i += 1;
    }
    if i < c.len() && c[i] == '0' {
        s.zero = true;
        i += 1;
    }
    let debut = i;
    while i < c.len() && c[i].is_ascii_digit() {
        i += 1;
    }
    if i > debut {
        s.largeur = Some(c[debut..i].iter().collect::<String>().parse().map_err(|_| (Classe::Illisible, "nombre trop grand dans la mise en forme".to_string()))?);
    }
    if i < c.len() && matches!(c[i], ',' | '_') {
        s.groupe = Some(c[i]);
        i += 1;
    }
    if i < c.len() && c[i] == '.' {
        i += 1;
        let debut = i;
        while i < c.len() && c[i].is_ascii_digit() {
            i += 1;
        }
        if i == debut {
            return Err((Classe::Illisible, "précision manquante après « . »".into()));
        }
        s.precision = Some(c[debut..i].iter().collect::<String>().parse().map_err(|_| (Classe::Illisible, "nombre trop grand dans la mise en forme".to_string()))?);
    }
    if i < c.len() {
        s.genre = Some(c[i]);
        i += 1;
    }
    if i < c.len() {
        return Err(invalide());
    }
    Ok(s)
}

/// `format(objet, spec)`.
fn mettre_en_forme(objet: &Objet, spec: &str) -> Result<String, Faute> {
    match objet {
        Objet::Texte(t) => {
            if spec.is_empty() {
                return Ok(t.clone());
            }
            let s = lire_specification(spec, "str")?;
            if let Some(g) = s.genre.filter(|g| *g != 's') {
                return Err((Classe::Illisible, format!("type « {} » inconnu pour un texte", g)));
            }
            if s.signe.is_some() {
                return Err((Classe::Illisible, "pas de signe pour un texte".into()));
            }
            if s.diese {
                return Err((Classe::Illisible, "pas de forme « # » pour un texte".into()));
            }
            if s.alignement == Some('=') {
                return Err((Classe::Illisible, "pas d'alignement « = » pour un texte".into()));
            }
            if let Some(g) = s.groupe {
                return Err((Classe::Illisible, format!("pas de « {} » avec « s »", g)));
            }
            let mut texte: String = t.clone();
            if let Some(p) = s.precision {
                texte = texte.chars().take(p).collect();
            }
            // Sans alignement dit, un texte se cale a gauche (`format`).
            let (remplissage, alignement) = match s.alignement {
                Some(a) => (s.remplissage, a),
                None => (if s.zero { '0' } else { s.remplissage }, '<'),
            };
            Ok(remplir_largeur(&texte, s.largeur, remplissage, alignement))
        }
        Objet::Entier(n) => {
            if spec.is_empty() {
                return Ok(n.to_string());
            }
            let s = lire_specification(spec, "int")?;
            // Sans type dit, un entier s'ecrit en decimal (`d`).
            let genre = match s.genre {
                Some(g) => g,
                None => 'd',
            };
            if matches!(genre, 'e' | 'E' | 'f' | 'F' | 'g' | 'G' | '%') {
                return Err((Classe::FormeNonPortee, format!("le type « {} » appliqué à un entier", genre)));
            }
            if !matches!(genre, 'd' | 'n' | 'x' | 'X' | 'o' | 'b' | 'c') {
                return Err((Classe::Illisible, format!("type « {} » inconnu pour un nombre entier", genre)));
            }
            if s.precision.is_some() {
                return Err((Classe::Illisible, "pas de précision pour un nombre entier".into()));
            }
            if s.groupe.is_some() && (s.zero || s.alignement == Some('=')) {
                return Err((Classe::FormeNonPortee, "le groupement des milliers avec remplissage de zeros".into()));
            }
            let negatif = *n < 0;
            let valeur = n.unsigned_abs();
            let mut corps = match genre {
                'x' => format!("{:x}", valeur),
                'X' => format!("{:X}", valeur),
                'o' => format!("{:o}", valeur),
                'b' => format!("{:b}", valeur),
                'c' => {
                    return match u32::try_from(*n).map(char::from_u32) {
                        Ok(Some(x)) => Ok(x.to_string()),
                        _ => Err((Classe::Illisible, format!("{} n'est pas un caractère (%c)", n))),
                    }
                }
                _ => valeur.to_string(),
            };
            if let (Some(g), 'd') = (s.groupe, genre) {
                let chiffres: Vec<char> = corps.chars().collect();
                let mut groupes = String::new();
                for (k, ch) in chiffres.iter().enumerate() {
                    if k > 0 && (chiffres.len() - k) % 3 == 0 {
                        groupes.push(g);
                    }
                    groupes.push(*ch);
                }
                corps = groupes;
            }
            let prefixe = if s.diese {
                match genre {
                    'x' => "0x",
                    'X' => "0X",
                    'o' => "0o",
                    'b' => "0b",
                    'd' | 'n' | 'c' => "",
                    autre => return Err((Classe::Illisible, format!("type « {} » inconnu pour un nombre entier", autre))),
                }
            } else {
                ""
            };
            let signe = if negatif {
                "-"
            } else {
                match s.signe {
                    Some('+') => "+",
                    Some(' ') => " ",
                    None | Some('-') => "",
                    Some(autre) => return Err((Classe::Illisible, format!("signe « {} » inconnu", autre))),
                }
            };
            let entete = format!("{}{}", signe, prefixe);
            // Sans alignement dit, un entier se cale a droite, ou apres son signe quand il se complete de zeros.
            let (remplissage, alignement) = match s.alignement {
                Some(a) => (s.remplissage, a),
                None if s.zero => ('0', '='),
                None => (s.remplissage, '>'),
            };
            if alignement == '=' {
                let mut sortie = entete.clone();
                if let Some(largeur) = s.largeur {
                    let total = entete.chars().count() + corps.chars().count();
                    sortie.push_str(&remplissage.to_string().repeat(largeur.saturating_sub(total)));
                }
                sortie.push_str(&corps);
                return Ok(sortie);
            }
            Ok(remplir_largeur(&format!("{}{}", entete, corps), s.largeur, remplissage, alignement))
        }
    }
}

fn remplir_largeur(t: &str, largeur: Option<usize>, remplissage: char, alignement: char) -> String {
    let Some(largeur) = largeur else { return t.to_string() };
    let n = t.chars().count();
    let manque = largeur.saturating_sub(n);
    let bourre = |k: usize| remplissage.to_string().repeat(k);
    match alignement {
        '>' => format!("{}{}", bourre(manque), t),
        '^' => format!("{}{}{}", bourre(manque / 2), t, bourre(manque - manque / 2)),
        _ => format!("{}{}", t, bourre(manque)),
    }
}
