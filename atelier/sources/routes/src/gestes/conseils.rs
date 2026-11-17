//! LES CONSEILS « Le saviez-vous ? ». Une option de la compagnie (fiche Entreprise, `conseils`, decochee
//! par defaut : sans elle, rien ne s'affiche nulle part) ouvre trois familles, qui parlent a l'employeur :
//!
//! - le conseil du demarrage (`conseil-du-jour`) : un conseil general de la banque (`quand` = « toujours »), a
//!   l'accueil. Il tourne sans rien ecrire : le rang du jour est `jour x pas mod N`, ou `pas` est premier avec `N` (le
//!   nombre de conseils affichables) ; N jours de suite parcourent donc toute la banque avant de repasser par le meme,
//!   et « un autre » (`decalage`) avance d'un cran a partir du conseil du jour. Rien a garder, rien a synchroniser entre
//!   postes, et le meme conseil pour tout le monde le meme jour ;
//! - les conseils de saisie (`de_saisie`) : sous la carte d'engagement de l'apercu, au plus un par carte, quand un fait
//!   de la saisie l'appelle (`saisie:<condition>`, dans l'ordre de `ordre_saisie`). Ils ne vivent QUE dans la route
//!   `apercu` (champ `conseil` de la vue d'une paie) : ni dans les avertissements de la paie, ni sur le bulletin, ni
//!   dans `produire`. Une phrase courte (`texte`), et le detail (`detail` : les deux faces, les chiffres) que l'ecran
//!   deplie derriere « pourquoi ? » avec la reference ;
//! - les conseils sur l'historique (`conseils-salarie`, `conseils-resume`) : ce que la compagnie a reellement fait pour
//!   un salarie, sur la fenetre de ses droits (depuis la date anniversaire que porte sa fiche, sinon 12 mois glissants).
//!
//! Le texte, ses chiffres dates et sa source vivent dans `donnees/conseils.json` (sa forme : `Banque`) : un conseil
//! s'ajoute sans toucher au moteur, tant qu'il ne lit que les valeurs livrees que la forme d'un gabarit connait
//! (`Livree`). Ici, seulement les conditions et les valeurs qu'elles fournissent. Ce qui est sur se chiffre : les heures
//! comptees chez nous, leur part des 507, ce que les conseils auraient change en heures et en jours d'allocation (annexe
//! X ou VIII, art. 30). JAMAIS d'euros d'allocation : on ne connait pas les autres employeurs. Le seul cas de
//! certitude : nos contrats font a eux seuls au moins 507 heures sur la fenetre ; alors ses droits sont ouverts, et le
//! conseil « plus d'heures » cede la place a « intense plutot que dilue ».
//!
//! Le salarie se nomme par son seul prenom (`{prenom}`), jamais par un pronom qui le genre.

use droit::fiche::Lue as _;
use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, OnceLock};

use droit::source::Source;
use serde::{Deserialize, Serialize};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use droit::cumul::{self, Constantes, Declare};
use droit::forme_bareme::Pile;
use socle::date::{Jour, Periode};
use droit::emplois;
use droit::emplois::AnnexeChomage as Annexe;
use droit::minima::Par;

use socle::grandeurs::{Cachets, Coefficient, Grandeur, Heures, HeuresParUnite, Jours, Montant, MontantUnitaire, NonNul, Services, Taux};
use socle::echec::R;
use socle::texte::Phrase;

use crate::catalogue::{Acte, Affiche, Champ, Forme, Fiche, OPERATION, SALARIE};
use crate::catalogue::Route;
use crate::catalogue::{Ecran, Place};
use crate::enveloppe::{en_json, route_inconnue, Charge, Dite, Porte, Rendu};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de lecture des conseils, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::ConseilDuJour,
        geste: "Dire le conseil du jour (« Le saviez-vous ? »).",
        charge: &[Champ { nom: "decalage", libelle: Affiche::new("Autre conseil"), forme: Forme::Nombre, sens: "un autre conseil : 1 le suivant, 2 celui d'après", avance: false }],
        exemple: r#"{}"#,
        rend: "le conseil (titre, texte, ce qu'il ouvre)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ConseilsSalarie,
        geste: "Dire les conseils qui valent pour un salarié.",
        charge: &[SALARIE],
        exemple: r#"{"salarie":"FULLER"}"#,
        rend: "conseils",
        refus: &[OPERATION],
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Fiches),
    },
    Fiche {
        route: Route::ConseilsResume,
        geste: "Dire les conseils qui valent pour la compagnie, en résumé.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "conseils",
        refus: &[OPERATION],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
];

// ---------------------------------------------------------------- la banque

/// Le refus d'une banque qui ne se lit pas.
const ILLISIBLE: &str = "La banque des conseils ne se lit pas : le signaler au développeur.";

/// LA BANQUE DES CONSEILS, `donnees/conseils.json`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Banque {
    #[serde(default, deserialize_with = "socle::forme::non_lue")]
    quoi: socle::forme::NonLue,
    #[serde(default, deserialize_with = "socle::forme::non_lue")]
    note: socle::forme::NonLue,
    /// Le titre lisible de chaque texte depose qu'une source cite, par son chemin.
    documents: BTreeMap<droit::source::CheminDepose, String>,
    annexes: Annexes,
    fenetres: Fenetres,
    /// L'ordre dans lequel les conditions de saisie se demandent : la premiere qui parle donne le conseil de la carte.
    ordre_saisie: Vec<Quand>,
    conseils: Vec<Conseil>,
}

/// Ce que l'article 30 des annexes dit d'un mois travaille pendant l'indemnisation, par annexe.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Annexes {
    #[serde(default, deserialize_with = "socle::forme::non_lue")]
    quoi: socle::forme::NonLue,
    #[serde(default, deserialize_with = "socle::forme::non_lue")]
    note: socle::forme::NonLue,
    #[serde(rename = "X")]
    x: Art30,
    #[serde(rename = "VIII")]
    viii: Art30Techniciens,
}

/// Les constantes de l'article 30 pour une annexe.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Art30 {
    heures_par_jour: HeuresParUnite,
    coefficient: Coefficient,
    jours_sans_allocation: Jours,
}

/// L'article 30 de l'annexe VIII, et les heures retenues par mois d'un technicien pour ses 507 heures (208, 250 avec
/// plusieurs employeurs).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Art30Techniciens {
    heures_par_jour: HeuresParUnite,
    coefficient: Coefficient,
    jours_sans_allocation: Jours,
    heures_retenues_par_mois: Heures,
    heures_retenues_plusieurs_employeurs: Heures,
}

/// La phrase de la fenetre des droits que la phrase nomme : `{debut}` et `{fin}`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fenetres {
    #[serde(default, deserialize_with = "socle::forme::non_lue")]
    quoi: socle::forme::NonLue,
    anniversaire: String,
    glissante: String,
}

/// QUAND UN CONSEIL PARLE : toujours (le conseil du jour), quand un fait de la saisie l'appelle, ou sur l'historique d'un
/// salarie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum Quand {
    #[serde(rename = "toujours")]
    Toujours,
    #[serde(rename = "saisie:technicien-plus-de-208-heures")]
    TechnicienPlusDe208Heures,
    #[serde(rename = "saisie:plus-de-28-cachets")]
    PlusDe28Cachets,
    #[serde(rename = "saisie:mois-sature")]
    MoisSature,
    #[serde(rename = "saisie:507-assurees")]
    HeuresAssurees,
    #[serde(rename = "saisie:repetition-en-cachet")]
    RepetitionEnCachet,
    #[serde(rename = "saisie:heures-pour-la-meme-somme")]
    HeuresPourLaMemeSomme,
    #[serde(rename = "saisie:enseignement")]
    Enseignement,
    #[serde(rename = "saisie:suspension")]
    Suspension,
    #[serde(rename = "saisie:cachets-et-heures")]
    CachetsEtHeures,
    #[serde(rename = "historique:droits-ouverts")]
    DroitsOuverts,
    #[serde(rename = "historique:intense")]
    Intense,
    #[serde(rename = "historique:heures-comptees")]
    HeuresComptees,
    #[serde(rename = "historique:gain-des-conseils")]
    GainDesConseils,
    #[serde(rename = "historique:cachets-perdus")]
    CachetsPerdus,
    #[serde(rename = "historique:enseignement-perdu")]
    EnseignementPerdu,
    #[serde(rename = "historique:resume")]
    Resume,
}

/// UN CONSEIL DE LA BANQUE : son texte et son detail (des gabarits), ses sources, et ce qui le range (`domaine`,
/// `public`, `ciblage`, `certitude`, pour qui l'ecrit).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Conseil {
    id: String,
    quand: Quand,
    texte: Gabarit,
    detail: Option<Gabarit>,
    source: Vec<Source>,
    #[serde(deserialize_with = "socle::forme::non_lue")]
    domaine: socle::forme::NonLue,
    #[serde(deserialize_with = "socle::forme::non_lue")]
    public: socle::forme::NonLue,
    #[serde(deserialize_with = "socle::forme::non_lue")]
    ciblage: socle::forme::NonLue,
    #[serde(deserialize_with = "socle::forme::non_lue")]
    certitude: socle::forme::NonLue,
}

/// La banque, lue en sa forme une fois par contenu.
fn banque() -> R<Arc<Banque>> {
    static CACHE: OnceLock<socle::fichiers::FormeGardee<Banque>> = OnceLock::new();
    socle::fichiers::forme_en_cache(&CACHE, &const { socle::fichiers::CheminRelatif::du_moteur("conseils.json") }, ILLISIBLE, Ok)
}

impl Banque {
    /// La reference lisible d'une source : le titre du document, l'article. Un texte que la banque cite sans titre est
    /// une banque mal ecrite.
    fn reference_de(&self, s: &Source) -> R<Phrase> {
        let titre = self.documents.get(&s.fichier).ok_or_else(|| {
            socle::echec::refus_avec(ILLISIBLE, socle::echec::Lieu { cle: Some(format!("documents/{}", s.fichier.texte())), detail: Some("le texte cité n'a pas de titre".to_string()), ..Default::default() })
        })?;
        Ok(match (s.article.as_deref().filter(|a| !a.is_empty()), s.fichier.code()) {
            (None, _) => socle::phrase!("{}", titre),
            (Some(a), Some(_)) => socle::phrase!("{}, art. {}", titre, a),
            (Some(a), None) => socle::phrase!("{}, {}", titre, a),
        })
    }

    /// L'article 30 d'une annexe.
    fn art30(&self, annexe: Annexe) -> Art30 {
        match annexe {
            Annexe::X => Art30 { heures_par_jour: self.annexes.x.heures_par_jour, coefficient: self.annexes.x.coefficient, jours_sans_allocation: self.annexes.x.jours_sans_allocation },
            Annexe::Viii => Art30 { heures_par_jour: self.annexes.viii.heures_par_jour, coefficient: self.annexes.viii.coefficient, jours_sans_allocation: self.annexes.viii.jours_sans_allocation },
        }
    }
}

/// Les annexes dont parle un conseil sur ce salarie : la sienne, lue de son metier ; les deux quand son metier ne la
/// dit pas (rien ne se devine : la regle citee vaut alors pour l'une ou l'autre).
fn annexes_en_jeu(annexe: Option<Annexe>) -> Vec<Annexe> {
    match annexe {
        Some(a) => vec![a],
        None => Annexe::LES_DEUX.to_vec(),
    }
}

/// L'option de la fiche Entreprise : absente vaut NON.
fn actifs(entreprise: &droit::fiche::ChampsEntreprise) -> bool {
    entreprise.conseils.oui()
}

// ---------------------------------------------------------------- gabarits

/// UN GABARIT : le texte d'un conseil, en morceaux. Dans le fichier, une phrase ou `{annee}` (l'annee du bareme lu, celle
/// du jour), `{bareme:chemin}` et `{minima:chemin}` (une valeur livree, `Livree`, puis ses operations `*n` ou `/n` et ses
/// decimales apres la virgule finale), `{variable}` (une valeur que la condition fournit), `{variable|singulier|pluriel}`
/// (le nombre et le mot accorde), `{variable?singulier|pluriel}` (le mot seul). Un gabarit qui ne se lit pas refuse la
/// banque ; un gabarit dont une valeur manque tait le conseil.
struct Gabarit(Vec<Morceau>);

enum Morceau {
    Texte(String),
    Annee,
    Donnee(Donnee),
    Variable { nom: String, mots: Option<(String, String)>, avec_nombre: bool },
}

/// Une valeur livree, ses operations et ses decimales.
struct Donnee {
    valeur: Livree,
    operations: Vec<Operation>,
    decimales: Option<u32>,
}

/// Une operation d'un gabarit sur la valeur livree : la mettre a l'echelle d'un coefficient (`*12`, `*100`), ou la
/// diviser par un coefficient non nul (`/31`).
enum Operation {
    Fois(Coefficient),
    DivisePar(NonNul<Coefficient>),
}

/// UNE VALEUR LIVREE QU'UN GABARIT CITE, dans sa grandeur.
#[derive(Clone, Copy)]
enum ValeurLivree {
    Montant(Montant),
    MontantUnitaire(MontantUnitaire),
    Taux(Taux),
    Coefficient(Coefficient),
}

/// LES VALEURS LIVREES QU'UN GABARIT LIT, chacune un acces a sa forme : le socle livre de l'annee
/// (`droit::forme_bareme::Pile`) ou les minima conventionnels (`droit::minima::Table`).
enum Livree {
    /// `bareme:abattement_csg`
    AbattementCsg,
    /// `bareme:plafond_securite_sociale/mensuel`
    PlafondMensuel,
    /// `bareme:plafond_securite_sociale/horaire`
    PlafondHoraire,
    /// `bareme:deduction_forfaitaire/plafond_annuel`
    PlafondDeduction,
    /// `bareme:deduction_forfaitaire/taux/<taux>`
    TauxDeduction(droit::forme_bareme::CategorieDfs),
    /// `bareme:rubriques_bulletin/id=<rubrique>/taux_pat`
    TauxPatronal(droit::forme_bareme::IdDeCotisation),
    /// `minima:conventions/<idcc>/annexes/<annexe>/repetition/cachet`
    CachetDeRepetition { idcc: droit::fiche::Idcc, annexe: droit::fiche::AnnexeConvention },
}

impl<'de> Deserialize<'de> for Gabarit {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Gabarit, D::Error> {
        lire_gabarit(&String::deserialize(d)?).map_err(|e| serde::de::Error::custom(e.phrase))
    }
}

fn lire_gabarit(t: &str) -> R<Gabarit> {
    let mut morceaux = Vec::new();
    let mut reste = t;
    while let Some(i) = reste.find('{') {
        if i > 0 {
            morceaux.push(Morceau::Texte(reste[..i].to_string()));
        }
        let apres = &reste[i + 1..];
        let j = apres.find('}').ok_or_else(|| socle::refus!("une accolade s'ouvre sans se fermer avant : {}", apres))?;
        morceaux.push(lire_morceau(&apres[..j])?);
        reste = &apres[j + 1..];
    }
    if !reste.is_empty() {
        morceaux.push(Morceau::Texte(reste.to_string()));
    }
    Ok(Gabarit(morceaux))
}

fn lire_morceau(dedans: &str) -> R<Morceau> {
    if dedans == "annee" {
        return Ok(Morceau::Annee);
    }
    if let Some(expr) = dedans.strip_prefix("bareme:") {
        return lire_donnee(expr, true).map(Morceau::Donnee);
    }
    if let Some(expr) = dedans.strip_prefix("minima:") {
        return lire_donnee(expr, false).map(Morceau::Donnee);
    }
    // {var|singulier|pluriel} : le nombre et le mot ; {var?singulier|pluriel} : le mot seul.
    let (nom, mots, avec_nombre) = match (dedans.find('|'), dedans.find('?')) {
        (_, Some(q)) => (&dedans[..q], Some(&dedans[q + 1..]), false),
        (Some(p), None) => (&dedans[..p], Some(&dedans[p + 1..]), true),
        (None, None) => (dedans, None, true),
    };
    let mots = match mots {
        Some(m) => Some(m.split_once('|').map(|(s, p)| (s.to_string(), p.to_string())).ok_or_else(|| socle::refus!("{} : le singulier et le pluriel s'écrivent séparés par une barre verticale", dedans))?),
        None => None,
    };
    Ok(Morceau::Variable { nom: nom.to_string(), mots, avec_nombre })
}

/// `chemin*n/m,2` : la valeur au chemin, puis les operations (elles commencent au premier « * » : un « / » avant lui
/// separe le chemin), avec ses decimales apres la virgule finale.
fn lire_donnee(expr: &str, bareme: bool) -> R<Donnee> {
    let (expr, decimales) = match expr.rsplit_once(',') {
        Some((e, d)) => (e, Some(d.parse::<u32>().map_err(|e| socle::refus!("{} : les décimales s'écrivent en chiffres ({})", d, e))?)),
        None => (expr, None),
    };
    let (chemin, ops) = match expr.find('*') {
        Some(i) => expr.split_at(i),
        None => (expr, ""),
    };
    let mut operations = Vec::new();
    let mut reste = ops;
    while let Some(op) = reste.chars().next() {
        let suite = &reste[op.len_utf8()..];
        let fin = match suite.find(['*', '/']) {
            Some(n) => n,
            None => suite.len(),
        };
        let n: Coefficient = socle::grandeurs::lu::ecrite(&suite[..fin]).map_err(|e| socle::refus!("{} : un nombre attendu après « {} » ({})", &suite[..fin], op, e))?;
        operations.push(match (op, NonNul::de(n)) {
            ('*', _) => Operation::Fois(n),
            ('/', Some(n)) => Operation::DivisePar(n),
            _ => return Err(socle::refus!("{} : une valeur se multiplie ou se divise par un nombre non nul", ops)),
        });
        reste = &suite[fin..];
    }
    Ok(Donnee { valeur: lire_livree(chemin, bareme)?, operations, decimales })
}

fn lire_livree(chemin: &str, bareme: bool) -> R<Livree> {
    let segments: Vec<&str> = chemin.split('/').collect();
    let inconnue = || socle::refus!("{} : une valeur livrée que la banque des conseils ne sait pas lire", chemin);
    Ok(match (bareme, segments.as_slice()) {
        (true, ["abattement_csg"]) => Livree::AbattementCsg,
        (true, ["plafond_securite_sociale", "mensuel"]) => Livree::PlafondMensuel,
        (true, ["plafond_securite_sociale", "horaire"]) => Livree::PlafondHoraire,
        (true, ["deduction_forfaitaire", "plafond_annuel"]) => Livree::PlafondDeduction,
        (true, ["deduction_forfaitaire", "taux", taux]) => Livree::TauxDeduction(droit::forme_bareme::CategorieDfs::lire(taux)?),
        (true, ["rubriques_bulletin", id, "taux_pat"]) => match id.strip_prefix("id=") {
            Some(id) => Livree::TauxPatronal(droit::forme_bareme::IdDeCotisation::lire(id)?),
            None => return Err(inconnue()),
        },
        (false, ["conventions", idcc, "annexes", annexe, "repetition", "cachet"]) => match droit::fiche::AnnexeConvention::du_code(annexe) {
            Some(annexe) => Livree::CachetDeRepetition { idcc: droit::fiche::Idcc::lire(idcc)?, annexe },
            None => return Err(inconnue()),
        },
        _ => return Err(inconnue()),
    })
}

impl Donnee {
    /// La valeur livree, ses operations faites.
    fn calculer(&self, x: ValeurLivree) -> ValeurLivree {
        self.operations.iter().fold(x, |x, op| match x {
            ValeurLivree::Montant(g) => ValeurLivree::Montant(operer(g, op)),
            ValeurLivree::MontantUnitaire(g) => ValeurLivree::MontantUnitaire(operer(g, op)),
            ValeurLivree::Taux(g) => ValeurLivree::Taux(operer(g, op)),
            ValeurLivree::Coefficient(g) => ValeurLivree::Coefficient(operer(g, op)),
        })
    }
}

/// Une operation d'un gabarit sur une grandeur.
fn operer<G>(g: G, op: &Operation) -> G
where
    G: std::ops::Mul<Coefficient, Output = G> + std::ops::Div<socle::grandeurs::NonNul<Coefficient>, Output = G>,
{
    match op {
        Operation::Fois(c) => g * *c,
        Operation::DivisePar(c) => g / *c,
    }
}

impl ValeurLivree {
    /// Telle que la phrase la cite : un montant en euros, par la seule ecriture d'un montant (`socle::texte::euros`, son
    /// « € » compris) ; une autre grandeur, `decimales` ecrites, sinon au centieme sans zero inutile (`socle::texte`).
    fn dite(self, decimales: Option<u32>) -> String {
        fn dite<G: Grandeur>(g: G, decimales: Option<u32>) -> String {
            match decimales {
                Some(d) => socle::texte::par_milliers_a(g, d),
                None => socle::texte::par_milliers(g),
            }
        }
        match self {
            ValeurLivree::Montant(g) => socle::texte::euros(g),
            ValeurLivree::MontantUnitaire(g) => socle::texte::euros(g),
            ValeurLivree::Taux(g) => dite(g, decimales),
            ValeurLivree::Coefficient(g) => dite(g, decimales),
        }
    }
}

/// Une valeur que fournit une condition : son texte, et si le mot qui la suit s'accorde au pluriel.
#[derive(Debug, Clone)]
struct Valeur {
    texte: String,
    pluriel: bool,
}

type Valeurs = BTreeMap<String, Valeur>;

fn val_texte(t: impl Into<String>) -> Valeur {
    Valeur { texte: t.into(), pluriel: false }
}

/// Une grandeur que la phrase cite (des heures, des jours, des cachets, un coefficient, un pourcentage) : sa porte.
fn val_grandeur<G: Grandeur>(g: G) -> Valeur {
    Valeur { texte: socle::texte::par_milliers(g), pluriel: socle::texte::au_pluriel(g) }
}

/// Un compte que la phrase cite (les salaries d'un groupe).
fn val_compte(n: usize) -> Valeur {
    Valeur { texte: socle::texte::compte(n), pluriel: n >= 2 }
}

/// Un montant unitaire que la phrase cite, en euros (`socle::texte::euros`) : sa porte.
fn val_unitaire(m: MontantUnitaire) -> Valeur {
    val_texte(socle::texte::euros(m))
}

/// Un montant que la phrase cite, en euros (`socle::texte::euros`) : sa porte.
fn val_euros(m: Montant) -> Valeur {
    val_texte(socle::texte::euros(m))
}

/// Les donnees que les gabarits lisent : le socle livre de l'annee (lu au 1er janvier, a la premiere valeur qu'un
/// gabarit lui demande), les minima.
struct Donnees {
    annee: socle::mois::Annee,
    bareme: OnceCell<Option<Pile>>,
}

impl Donnees {
    fn de_l_annee(annee: socle::mois::Annee) -> Donnees {
        Donnees { annee, bareme: OnceCell::new() }
    }

    /// Le socle livre de l'annee ; `None` : il n'est pas livre.
    fn bareme(&self) -> R<Option<&Pile>> {
        if let Some(b) = self.bareme.get() {
            return Ok(b.as_ref());
        }
        let lu = droit::contexte::socle_livre_lu(self.annee)?;
        Ok(self.bareme.get_or_init(|| lu).as_ref())
    }

    /// La valeur livree ; `None` : la donnee ne la porte pas.
    fn valeur(&self, l: &Livree) -> R<Option<ValeurLivree>> {
        let b = match l {
            Livree::CachetDeRepetition { idcc, annexe } => {
                let t = droit::minima::table()?;
                let a = t.conventions.get(idcc).and_then(|c| c.annexes.as_ref()).and_then(|a| a.get(annexe));
                return Ok(a.and_then(|a| a.repetition.as_ref()).and_then(|r| r.cachet).map(ValeurLivree::MontantUnitaire));
            }
            _ => match self.bareme()? {
                Some(b) => b,
                None => return Ok(None),
            },
        };
        Ok(match l {
            Livree::AbattementCsg => Some(ValeurLivree::Coefficient(b.abattement_csg)),
            Livree::PlafondMensuel => b.plafond_securite_sociale.as_ref().map(|p| ValeurLivree::Montant(p.mensuel)),
            Livree::PlafondHoraire => b.plafond_securite_sociale.as_ref().map(|p| ValeurLivree::MontantUnitaire(p.horaire)),
            Livree::PlafondDeduction => b.deduction_forfaitaire.as_ref().map(|d| ValeurLivree::Montant(d.plafond_annuel)),
            Livree::TauxDeduction(t) => b.deduction_forfaitaire.as_ref().and_then(|d| d.taux.get(t)).map(|n| ValeurLivree::Taux(*n)),
            Livree::TauxPatronal(id) => match b.rubrique(id).and_then(|r| r.taux_pat) {
                Some(t) => Some(ValeurLivree::Taux(t.0)),
                None => None,
            },
            Livree::CachetDeRepetition { .. } => None,
        })
    }
}

/// Le texte d'un conseil, gabarits resolus ; `None` quand l'un d'eux ne se resout pas (le conseil se tait).
fn rendre(g: &Gabarit, d: &Donnees, vals: &Valeurs) -> R<Option<Phrase>> {
    let mut morceaux = Vec::new();
    for m in &g.0 {
        morceaux.push(match m {
            Morceau::Texte(t) => socle::phrase!("{}", t),
            Morceau::Annee => match d.bareme()? {
                Some(_) => socle::phrase!("{}", d.annee),
                None => return Ok(None),
            },
            Morceau::Donnee(x) => match d.valeur(&x.valeur)? {
                Some(v) => socle::phrase!("{}", x.calculer(v).dite(x.decimales)),
                None => return Ok(None),
            },
            Morceau::Variable { nom, mots, avec_nombre } => {
                let Some(v) = vals.get(nom) else { return Ok(None) };
                match mots {
                    None => socle::phrase!("{}", v.texte),
                    Some((sing, plur)) => {
                        let mot = if v.pluriel { plur } else { sing };
                        if *avec_nombre {
                            socle::phrase!("{} {}", v.texte, mot)
                        } else {
                            socle::phrase!("{}", mot)
                        }
                    }
                }
            }
        });
    }
    Ok(Some(Phrase::jointes(morceaux, "")))
}

/// LE CONSEIL RENDU POUR L'ECRAN : son identifiant, son texte (la phrase courte d'un conseil de saisie), son detail
/// quand la banque en a un, sa reference (celle de chaque source citee).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ConseilVu {
    id: String,
    texte: Phrase,
    reference: Phrase,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<Phrase>,
}

/// Le conseil rendu ; rien si l'un de ses gabarits ne se resout pas. Une source tiree d'une annexe (`annexe` : « X » ou
/// « VIII ») n'est citee que si cette annexe est en jeu (`annexes` : celles des salaries dont le conseil parle) ; une
/// source sans annexe l'est toujours.
fn vue(b: &Banque, c: &Conseil, d: &Donnees, vals: &Valeurs, annexes: &[Annexe]) -> R<Option<ConseilVu>> {
    let mut references = Vec::new();
    for s in &c.source {
        let d_une_autre_annexe = matches!(s.annexe, Some(a) if !annexes.contains(&a));
        if !d_une_autre_annexe {
            references.push(b.reference_de(s)?);
        }
    }
    let Some(texte) = rendre(&c.texte, d, vals)? else { return Ok(None) };
    let detail = match &c.detail {
        Some(g) => match rendre(g, d, vals)? {
            Some(t) => Some(t),
            None => return Ok(None),
        },
        None => None,
    };
    Ok(Some(ConseilVu { id: c.id.clone(), texte, reference: Phrase::jointes(references, " ; "), detail }))
}

/// Le premier conseil de la banque pour cette condition (`quand`), rendu avec ces valeurs : un conseil dont un gabarit
/// ne se resout pas cede la place au suivant ; rien s'il n'en reste aucun.
fn conseil(b: &Banque, d: &Donnees, quand: Quand, vals: &Valeurs, annexes: &[Annexe]) -> R<Option<ConseilVu>> {
    for c in b.conseils.iter().filter(|c| c.quand == quand) {
        if let Some(v) = vue(b, c, d, vals, annexes)? {
            return Ok(Some(v));
        }
    }
    Ok(None)
}

/// Le prenom du salarie, tel que sa fiche l'affiche (`prenom_affichage`, sinon `prenom`, en capitales initiales s'il
/// est tout en majuscules) ; sans prenom, son nom, sinon sa cle.
fn prenom(sal: &droit::fiche::ChampsSalarie, cle: &droit::contexte::IdSalarie) -> String {
    fn lu<T: AsRef<str>>(c: &Option<T>) -> Option<String> {
        c.portee().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
    }
    if let Some(p) = lu(&sal.prenom_affichage) {
        return p;
    }
    if let Some(p) = lu(&sal.prenom).or_else(|| lu(&sal.nom_affichage)).or_else(|| lu(&sal.nom)) {
        if p.chars().any(char::is_lowercase) {
            return p;
        }
        // FANCHON -> Fanchon, JEAN-LOUP -> Jean-Loup.
        let mut out = String::with_capacity(p.len());
        let mut debut = true;
        for ch in p.chars() {
            if debut {
                out.extend(ch.to_uppercase());
            } else {
                out.extend(ch.to_lowercase());
            }
            debut = !ch.is_alphabetic();
        }
        return out;
    }
    cle.to_string()
}

// ------------------------------------------------------------ le demarrage

fn pgcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        pgcd(b, a % b)
    }
}

/// Le pas de la rotation : le premier entier a partir de N x 0,618 qui est premier avec N. Deux jours voisins ne
/// tombent pas sur deux conseils voisins de la banque (rangee par domaine), et N jours de suite les montrent tous.
fn pas(n: usize) -> usize {
    let mut s = (n * 618).div_ceil(1000).max(1);
    while pgcd(s, n) != 1 {
        s += 1;
    }
    s
}

/// Les conseils generaux affichables cette annee, dans l'ordre de la banque, deja rendus.
fn generaux(b: &Banque, d: &Donnees) -> R<Vec<ConseilVu>> {
    let mut out = Vec::new();
    for c in b.conseils.iter().filter(|c| c.quand == Quand::Toujours) {
        if let Some(v) = vue(b, c, d, &Valeurs::new(), &Annexe::LES_DEUX)? {
            out.push(v);
        }
    }
    Ok(out)
}

/// Le conseil du jour, decale de `decalage` (« un autre » l'augmente d'un) ; rien quand l'option est decochee.
fn du_jour(ctx: &Contexte, decalage: Option<usize>) -> R<Option<ConseilVu>> {
    if !actifs(&ctx.entreprise) {
        return Ok(None);
    }
    let b = banque()?;
    let jour = ctx.aujourdhui;
    let d = Donnees::de_l_annee(jour.annee());
    let liste = generaux(&b, &d)?;
    let n = liste.len();
    if n == 0 {
        return Ok(None);
    }
    let rang_du_jour = (jour.ordinal() as i128) * (pas(n) as i128);
    let rang = match decalage {
        Some(x) => rang_du_jour + x as i128,
        None => rang_du_jour,
    };
    Ok(liste.into_iter().nth(rang.rem_euclid(n as i128) as usize))
}

// ------------------------------------------------------- les faits d'une paie

/// L'annexe du salarie de cette paie, lue du metier de sa fiche : X pour un artiste, VIII pour un technicien ; rien
/// sinon (rien ne se devine).
fn annexe_de(sal: &droit::fiche::ChampsSalarie) -> R<Option<Annexe>> {
    let Some(code) = sal.code_emploi_ft.dite() else { return Ok(None) };
    let Some(m) = emplois::metier(code)? else { return Ok(None) };
    Ok(Some(match m.statut() {
        droit::fiche::Statut::Artiste => Annexe::X,
        droit::fiche::Statut::Technicien => Annexe::Viii,
    }))
}

/// Les jours d'allocation qu'un nombre d'heures retire un mois indemnise (art. 30 : heures / heures par jour x
/// coefficient), sans arrondi : une somme sur plusieurs paies s'arrondit une fois (`environ`).
fn jours_bruts(heures: Heures, a: &Art30) -> R<Jours> {
    match heures.en_jours(a.heures_par_jour) {
        Some(j) => Ok(j * a.coefficient),
        None => Err(socle::refus!("la banque des conseils dit des jours de travail de zéro heure : le signaler au développeur")),
    }
}

/// `jours_perdus`, seulement quand le cout arrondi n'est pas nul : « environ », au demi-jour le plus proche (8 heures en
/// annexe X, 1,04 jour, font « environ 1 jour ») ; zero quand le cout est de moins d'un quart de jour : la condition ne
/// fournit alors pas `jours_perdus`, et la variante « -minime » de la banque dit « moins d'un demi-jour » au lieu d'une
/// perte.
fn avec_jours(v: &mut Vec<(&str, Valeur)>, jours: Jours) {
    let j = jours.au_demi();
    if j > Jours::ZERO {
        let dit = val_grandeur(j);
        let t = if j == Jours::DEMI { "½".to_string() } else { dit.texte };
        v.push(("jours_perdus", Valeur { texte: t, pluriel: dit.pluriel }));
    }
}

/// Ce qu'une paie gagnerait en heures a suivre le conseil « cachet de repetition » : (heures avant, gain, jours,
/// somme), ou rien. La convention 3090 fixe un « cachet de repetition pour 1 ou 2 services de repetition dans la meme
/// journee » (`minima::cachet_de_repetition`) ; un cachet compte 12 heures (annexe X, art. 3). Seulement les lignes
/// « Service de repetition » a dates connues, de 1 ou 2 services par jour, de moins de 12 heures par jour, dont la
/// somme par jour couvre le cachet de repetition : sous le minimum, rien.
struct GainRepetition {
    heures_avant: Heures,
    gain: Heures,
    jours: Jours,
    montant: Montant,
    minimum: droit::minima::Minimum,
}

fn gain_repetition(ctx: &Contexte, p: &Paie, heures_cachet: HeuresParUnite) -> R<Option<GainRepetition>> {
    let Some(min) = p.cachet_de_repetition(ctx)? else { return Ok(None) };
    // Le minimum du cachet de repetition : le prix d'un jour.
    let minimum_du_jour = min.montant;
    let saisies = p.saisies();
    let (mut heures_avant, mut gain, mut jours, mut montant) = (Heures::ZERO, Heures::ZERO, Jours::ZERO, Montant::ZERO);
    for (l, s) in p.remuneration.iter().zip(saisies) {
        if l.code != droit::catalogue::REPETITION || l.heures_annexes.est_nul() {
            continue;
        }
        let Some(engagement::ligne::QuantiteDeLigne::Services(services)) = l.paiement.quantite() else { continue };
        // Une ligne sans saisie n'a pas de dates connues.
        let Some(s) = s else { continue };
        let dates: BTreeSet<Jour> = s.dates().iter().map(|j| j.jour()).collect();
        if dates.is_empty() {
            continue;
        }
        // Par jour : au plus deux services, moins que les heures d'un cachet, au moins le minimum.
        let n = Jours::de_dates(dates.len());
        if services > Services::comptes(2 * dates.len()) || l.heures_annexes >= n * heures_cachet || l.montant < n * minimum_du_jour {
            continue;
        }
        heures_avant += l.heures_annexes;
        gain += n * heures_cachet - l.heures_annexes;
        jours += n;
        montant += l.montant;
    }
    if gain <= Heures::ZERO {
        return Ok(None);
    }
    Ok(Some(GainRepetition { heures_avant, gain, jours, montant, minimum: min }))
}

/// Ce qu'une paie gagnerait en heures a couvrir, pour la meme somme, autant d'heures que le minimum horaire de la
/// convention le permet (`minima::minimum_de_la_ligne`, un minimum a l'heure) : la ligne au plus grand gain, ou rien.
/// Sous le minimum, rien.
///
/// LE SEUIL : un gain d'au moins `seuil` heures, le poids d'un cachet (12 heures, annexe X art. 3 §1) : un conseil
/// ne se donne que si l'ecart est important. Pourquoi lui : deux simulations (le modele
/// mensuel des annexes VIII et X, rejoue sur les exemples chiffres du guide France Travail) montrent que des heures de
/// plus pour la meme somme ne valent que pres du seuil des 507 heures, ou elles peuvent valoir d'un mois a une annee de
/// droits ; ailleurs, un mois indemnise, chaque heure coute 0,13 jour d'allocation (X ; 0,175 en VIII) et rapporte
/// moins l'annee suivante (2 heures de plus : de 0 a -60 EUR sur 24 mois ; dix fois 2 heures : -60 a -200 EUR). Le conseil ne
/// gagne que si le salarie est a moins de `gain` heures des 507 : sa chance de servir croit avec le gain, et sous le
/// poids d'un cachet elle est trop mince pour une ligne sous chaque carte.
struct GainHeures {
    heures: Heures,
    montant: Montant,
    minimum: MontantUnitaire,
    heures_max: Heures,
    gain: Heures,
}

fn gain_heures(ctx: &Contexte, p: &Paie, seuil: Heures) -> R<Option<GainHeures>> {
    let mut meilleur: Option<GainHeures> = None;
    for (i, l) in p.remuneration.iter().enumerate() {
        let (heures, montant) = (l.heures_annexes, l.montant);
        if heures <= Heures::ZERO || montant <= Montant::ZERO {
            continue;
        }
        let Some(m) = p.minimum_de_la_ligne(ctx, i)? else { continue };
        if m.par != Par::Heure || m.montant <= MontantUnitaire::ZERO {
            continue;
        }
        let minimum = m.montant;
        let Some(heures_max) = montant.en_heures(minimum) else { continue };
        let heures_max = heures_max.entieres_en_dessous();
        let gain = (heures_max - heures).entieres_en_dessous();
        if gain < seuil.max(Heures::UNE) {
            continue;
        }
        if !matches!(&meilleur, Some(g) if gain <= g.gain) {
            meilleur = Some(GainHeures { heures, montant, minimum, heures_max, gain });
        }
    }
    Ok(meilleur)
}

/// Les engagements que nos fichiers declarent, ceux du mois en cours remplaces par les paies de l'ecran ; lus une
/// fois par apercu, et seulement si une condition en a besoin.
struct Declares<'a> {
    ctx: &'a Contexte,
    paies: &'a [Paie],
    departs: &'a engagement::departs::Departs,
    lus: Option<Vec<Declare>>,
}

impl<'a> Declares<'a> {
    fn tous(&mut self) -> R<&[Declare]> {
        let lus = match self.lus.take() {
            Some(l) => l,
            None => engagement::cumul::lire_engagements(self.ctx, Some(self.paies), self.departs)?,
        };
        Ok(self.lus.insert(lus))
    }
}

/// La phrase de la fenetre (`conseils.json`, `fenetres`).
fn phrase_fenetre(b: &Banque, f: &cumul::Position, anniversaire: bool) -> String {
    let g = if anniversaire { &b.fenetres.anniversaire } else { &b.fenetres.glissante };
    g.replace("{debut}", &f.periode.debut().jj_mm_aaaa()).replace("{fin}", &f.periode.fin().jj_mm_aaaa())
}

/// La position du salarie sur la fenetre de ses droits : depuis sa date anniversaire si sa fiche la porte, sinon les
/// 12 mois jusqu'a `a_la_date`.
fn position(ctx: &Contexte, cle: &droit::contexte::IdSalarie, engs: &[Declare], a_la_date: Jour) -> R<(cumul::Position, bool)> {
    let anniv = cumul::anniversaire_configure(ctx, cle)?;
    let p = cumul::position_droits(ctx, cle, engs, anniv, Some(a_la_date))?;
    Ok((p, anniv.is_some()))
}

// ------------------------------------------------------------- la saisie

/// LES CONSEILS DE SAISIE : pour chaque paie de l'apercu, le premier conseil (dans l'ordre `ordre_saisie`) qu'un fait
/// de la saisie appelle, ou rien. Rien du tout quand l'option est decochee. Un permanent (paye au mois), un mois sans
/// paie, une paie dont le metier ne dit pas l'annexe : rien.
pub(crate) fn de_saisie(ctx: &Contexte, paies: &[Paie]) -> R<Vec<Option<ConseilVu>>> {
    let mut out = vec![None; paies.len()];
    if !actifs(&ctx.entreprise) {
        return Ok(out);
    }
    let b = banque()?;
    let d = Donnees::de_l_annee(ctx.aujourdhui.annee());
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    // Le mois de chaque salarie, chez nous : ses heures (cachets a 12 h) et ses cachets, toutes ses paies de l'ecran.
    let mut du_mois: HashMap<String, (Heures, Cachets)> = HashMap::new();
    for p in paies {
        let e = &p.engagement;
        if e.sans_paie() || e.nat()?.mensuelle() {
            continue;
        }
        let x = du_mois.entry(p.salarie.id.to_string()).or_insert((Heures::ZERO, Cachets::ZERO));
        if let Some(h) = e.heures {
            x.0 += h.entieres();
        }
        x.0 += e.cachets * c.heures_par_cachet;
        x.1 += e.cachets;
    }
    let heures_cachet = c.heures_par_cachet;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let mut declares = Declares { ctx, paies, departs: &departs, lus: None };
    let mut assurees: HashMap<String, Option<(Heures, String)>> = HashMap::new();
    for (i, p) in paies.iter().enumerate() {
        let e = &p.engagement;
        if e.sans_paie() || e.nat()?.mensuelle() {
            continue;
        }
        let Some(annexe) = annexe_de(&p.salarie)? else { continue };
        let a30 = b.art30(annexe);
        let cle = p.salarie.id.to_string();
        // Chaque paie comptee ici l'a ete au mois de son salarie, juste avant.
        let Some(&(heures_mois, cachets_mois)) = du_mois.get(&cle) else { continue };
        let qui = prenom(&p.salarie, &p.salarie.id);
        let mois = e.mois.libelle(socle::mois::Graphie::Accentuee);
        let base = |extra: Vec<(&str, Valeur)>| -> Valeurs {
            let mut v = Valeurs::new();
            v.insert("prenom".into(), val_texte(qui.clone()));
            v.insert("mois".into(), val_texte(mois.clone()));
            v.insert("heures_cachet".into(), val_grandeur(heures_cachet));
            for (k, x) in extra {
                v.insert(k.into(), x);
            }
            v
        };
        let seuil_mois = a30.jours_sans_allocation * a30.heures_par_jour;
        let sature = heures_mois >= seuil_mois;
        // Les deux conseils « plus d'heures », et ce qui les remplace quand nos contrats assurent les 507 heures. Le seuil
        // d'un gain d'heures : le poids d'un cachet (`gain_heures`).
        let rep = if annexe == Annexe::X { gain_repetition(ctx, p, heures_cachet)? } else { None };
        let hrs = gain_heures(ctx, p, Cachets::UN * heures_cachet)?;
        // Chaque gain, ses jours d'allocation, et ce qu'il paie par jour ou par heure (un gain a des jours et des heures).
        let impossible = || socle::refus!("un gain des conseils sans jour ni heure : le signaler au développeur");
        let rep = match rep {
            Some(g) => Some((jours_bruts(g.gain, &a30)?, g.montant.par_jour(g.jours).ok_or_else(impossible)?, g)),
            None => None,
        };
        let hrs = match hrs {
            Some(g) => Some((jours_bruts(g.gain, &a30)?, g.montant.par_heure(g.heures).ok_or_else(impossible)?, g)),
            None => None,
        };
        let assure = if (rep.is_some() || hrs.is_some()) && !sature {
            if !assurees.contains_key(&cle) {
                let (pos, anniv) = position(ctx, &p.salarie.id, declares.tous()?, e.periode_payee()?.fin())?;
                let h = pos.heures_ouverture()?;
                let r = (h >= c.seuil_heures).then(|| (h, phrase_fenetre(&b, &pos, anniv)));
                assurees.insert(cle.clone(), r);
            }
            assurees.get(&cle).cloned().flatten()
        } else {
            None
        };
        for &quand in &b.ordre_saisie {
            let vals: Option<Valeurs> = match quand {
                Quand::TechnicienPlusDe208Heures => {
                    let t = &b.annexes.viii;
                    (annexe == Annexe::Viii && heures_mois > t.heures_retenues_par_mois).then(|| {
                        base(vec![
                            ("heures", val_grandeur(heures_mois)),
                            ("plafond", val_grandeur(t.heures_retenues_par_mois)),
                            ("plafond_majore", val_grandeur(t.heures_retenues_plusieurs_employeurs)),
                        ])
                    })
                }
                Quand::MoisSature if sature => Some(base(vec![
                    ("heures", val_grandeur(heures_mois)),
                    ("seuil", val_grandeur(seuil_mois)),
                    ("jours", val_grandeur(a30.jours_sans_allocation)),
                    ("heures_par_jour", val_grandeur(a30.heures_par_jour)),
                ])),
                Quand::PlusDe28Cachets if annexe == Annexe::X && cachets_mois > c.cachets_par_mois => Some(base(vec![
                    ("cachets", val_grandeur(cachets_mois)),
                    ("plafond", val_grandeur(c.cachets_par_mois)),
                    ("perdus", val_grandeur(cachets_mois - c.cachets_par_mois)),
                ])),
                Quand::HeuresAssurees => assure.as_ref().map(|(h, fenetre)| {
                    base(vec![
                        ("heures", val_grandeur(*h)),
                        ("fenetre", val_texte(fenetre.clone())),
                        ("heures_par_jour", val_grandeur(a30.heures_par_jour)),
                        ("coefficient", val_grandeur(a30.coefficient)),
                    ])
                }),
                Quand::RepetitionEnCachet if assure.is_none() && !sature => rep.as_ref().map(|(jours, par_jour, g)| {
                    let mut v = vec![
                        ("heures_avant", val_grandeur(g.heures_avant)),
                        ("nb_jours", val_grandeur(g.jours)),
                        ("montant_jour", val_unitaire(*par_jour)),
                        ("minimum", val_unitaire(g.minimum.montant.au_centime())),
                        ("gain", val_grandeur(g.gain)),
                    ];
                    avec_jours(&mut v, *jours);
                    base(v)
                }),
                Quand::HeuresPourLaMemeSomme if assure.is_none() && !sature => hrs.as_ref().map(|(jours, par_heure, g)| {
                    let mut v = vec![
                        ("heures", val_grandeur(g.heures)),
                        ("montant", val_euros(g.montant)),
                        ("taux", val_unitaire(*par_heure)),
                        ("minimum", val_unitaire(g.minimum)),
                        ("heures_max", val_grandeur(g.heures_max)),
                        ("gain", val_grandeur(g.gain)),
                    ];
                    avec_jours(&mut v, *jours);
                    base(v)
                }),
                Quand::Enseignement => {
                    let mut trouve = None;
                    for l in &p.remuneration {
                        let r = droit::catalogue::rubrique(&ctx.racine, &l.code, &ctx.lue_au)?;
                        if r.compte_pour != droit::catalogue::ComptePour::OuvertureDesDroits {
                            continue;
                        }
                        if let Some(pl) = r.plafond_annuel_heures {
                            trouve = Some(base(vec![("plafond", val_grandeur(pl.moins_de_50_ans)), ("plafond_50", val_grandeur(pl.cinquante_ans_et_plus))]));
                        }
                        break;
                    }
                    trouve
                }
                Quand::Suspension if !e.suspensions.is_empty() => Some(base(vec![])),
                Quand::CachetsEtHeures if annexe == Annexe::X && e.cachets > Cachets::ZERO && e.heures.is_some_and(|h| h > Heures::ZERO) => Some(base(vec![])),
                _ => None,
            };
            if let Some(v) = vals {
                if let Some(x) = conseil(&b, &d, quand, &v, &[annexe])? {
                    out[i] = Some(x);
                    break;
                }
            }
        }
    }
    Ok(out)
}

// --------------------------------------------------------------- l'historique

/// Ce que les conseils de saisie auraient change aux paies de ce salarie sur la fenetre : (heures, jours d'allocation,
/// avant arrondi). Chaque mois de la fenetre se lit dans les paies ecrites de ses engagements valides
/// (`engagement::etats::validees_du_mois`) : rien ne se recalcule ; un mois sans engagement valide n'en a pas.
fn gains_sur_la_fenetre(ctx: &Contexte, cle: &droit::contexte::IdSalarie, f: &Periode, a30: &Art30, annexe: Annexe, departs: &engagement::departs::Departs) -> R<(Heures, Jours)> {
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let heures_cachet = c.heures_par_cachet;
    let mut gain = Heures::ZERO;
    let mut jours = Jours::ZERO;
    for mois in f.mois() {
        let Some((_, paies)) = engagement::etats::validees_du_mois(ctx, mois, departs)? else { continue };
        for p in &paies {
            let e = &p.engagement;
            if p.salarie.id != *cle || e.sans_paie() || e.nat()?.mensuelle() {
                continue;
            }
            if !f.contient(&e.periode_payee()?.fin()) {
                continue;
            }
            let repetition = match annexe {
                Annexe::X => gain_repetition(ctx, p, heures_cachet)?,
                Annexe::Viii => None,
            };
            let g = match repetition {
                Some(r) => Some(r.gain),
                None => gain_heures(ctx, p, Cachets::UN * heures_cachet)?.map(|h| h.gain),
            };
            if let Some(g) = g.filter(|g| *g > Heures::ZERO) {
                gain += g;
                jours += jours_bruts(g, a30)?;
            }
        }
    }
    Ok((gain, jours))
}

/// `conseils-salarie` : ce que la compagnie a fait pour ce salarie, sur la fenetre de ses droits. Un salarie qu'aucune
/// fiche ne porte n'en a pas.
fn du_salarie(ctx: &Contexte, cle: &droit::contexte::IdSalarie) -> R<Vec<ConseilVu>> {
    if !actifs(&ctx.entreprise) {
        return Ok(Vec::new());
    }
    Ok(ctx.salaries.get(cle).map(|fiche| conseils_de(ctx, &fiche.id, fiche)).transpose()?.into_iter().flatten().collect())
}

fn conseils_de(ctx: &Contexte, cle: &droit::contexte::IdSalarie, fiche: &droit::fiche::FicheSalarie) -> R<Vec<ConseilVu>> {
    let b = banque()?;
    let jour = ctx.aujourdhui;
    let d = Donnees::de_l_annee(jour.annee());
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let engs = engagement::cumul::lire_engagements(ctx, None, &departs)?;
    let (pos, anniv) = position(ctx, cle, &engs, jour)?;
    if pos.engagements.is_empty() {
        return Ok(Vec::new());
    }
    let qui = prenom(fiche, cle);
    let fenetre = phrase_fenetre(&b, &pos, anniv);
    let annexe = annexe_de(fiche)?;
    let a30 = annexe.map(|a| b.art30(a));
    let heures = pos.heures_ouverture()?;
    let base = |extra: Vec<(&str, Valeur)>| -> Valeurs {
        let mut v = Valeurs::new();
        v.insert("prenom".into(), val_texte(qui.clone()));
        v.insert("fenetre".into(), val_texte(fenetre.clone()));
        v.insert("seuil".into(), val_grandeur(c.seuil_heures));
        for (k, x) in extra {
            v.insert(k.into(), x);
        }
        v
    };
    let en_jeu = annexes_en_jeu(annexe);
    let mut a_dire: Vec<(Quand, Valeurs)> = Vec::new();
    // La part du seuil que des heures font, en pourcent : la phrase la dit.
    let pourcentage = |h: Heures| h.rapport(c.seuil_heures).map(Coefficient::en_taux).ok_or_else(|| socle::refus!("le seuil des droits est de zéro heure : le signaler au développeur"));
    if heures >= c.seuil_heures {
        a_dire.push((Quand::DroitsOuverts, base(vec![("heures", val_grandeur(heures))])));
        if let Some(a) = &a30 {
            a_dire.push((Quand::Intense, base(vec![("heures_par_jour", val_grandeur(a.heures_par_jour)), ("coefficient", val_grandeur(a.coefficient))])));
        }
    } else {
        let pct = pourcentage(heures)?.au_pair(0);
        a_dire.push((Quand::HeuresComptees, base(vec![("heures", val_grandeur(heures)), ("pourcentage", val_grandeur(pct))])));
        if let (Some(a), Some(ax)) = (&a30, annexe) {
            let (gain, jours) = gains_sur_la_fenetre(ctx, cle, &pos.periode, a, ax, &departs)?;
            if gain > Heures::ZERO {
                let pct = pourcentage(gain)?.au_pair(1);
                let mut v = vec![("gain", val_grandeur(gain)), ("pourcentage", val_grandeur(pct))];
                avec_jours(&mut v, jours);
                a_dire.push((Quand::GainDesConseils, base(v)));
            }
        }
    }
    // Le plafond de 28 cachets est de l'annexe X (art. 3 §1) : rien a dire d'un technicien.
    if annexe != Some(Annexe::Viii) {
        let par_mois = pos.cachets_par_mois();
        for m in pos.mois_plafonnes() {
            // Un mois plafonne est un mois de `cachets_par_mois`.
            let Some(&n) = par_mois.get(&m) else { continue };
            a_dire.push((
                Quand::CachetsPerdus,
                base(vec![
                    ("mois", val_texte(m.libelle(socle::mois::Graphie::Accentuee))),
                    ("cachets", val_grandeur(n)),
                    ("plafond", val_grandeur(c.cachets_par_mois)),
                    ("perdus", val_grandeur(n - c.cachets_par_mois)),
                ]),
            ));
        }
    }
    let perdu = pos.enseignement_perdu()?;
    if perdu > Heures::ZERO {
        a_dire.push((
            Quand::EnseignementPerdu,
            base(vec![("heures", val_grandeur(pos.heures_enseignement())), ("plafond", val_grandeur(pos.plafond_enseignement()?)), ("perdus", val_grandeur(perdu))]),
        ));
    }
    let mut out = Vec::new();
    for (quand, v) in a_dire {
        if let Some(x) = conseil(&b, &d, quand, &v, &en_jeu)? {
            out.push(x);
        }
    }
    Ok(out)
}

/// `conseils-resume` : a l'accueil, les salaries dont nos seuls contrats font au moins 507 heures sur la fenetre de
/// leurs droits, la meme que leur fiche (`position`) : les 12 mois avant leur date anniversaire si la fiche la porte,
/// sinon les 12 derniers mois. La phrase dit l'une, l'autre, ou les deux quand la liste les mele : la banque essaie
/// `historique-resume-melange`, puis `historique-resume-anniversaire`, puis `historique-resume`, et le premier dont
/// les deux groupes de noms sont la parle. Vide quand il n'y en a pas.
fn resume(ctx: &Contexte) -> R<Vec<ConseilVu>> {
    if !actifs(&ctx.entreprise) {
        return Ok(Vec::new());
    }
    let b = banque()?;
    let jour = ctx.aujourdhui;
    let d = Donnees::de_l_annee(jour.annee());
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let engs = engagement::cumul::lire_engagements(ctx, None, &departs)?;
    let cles: BTreeSet<&droit::contexte::IdSalarie> = engs.iter().map(|e| &e.salarie).collect();
    // Les salaries par leur prenom (sans fiche, par leur cle) ; deux du meme prenom, par leur prenom et leur nom (sinon
    // on ne saurait lequel).
    let mut fiches = Vec::new();
    for cle in cles {
        let (p, anniv) = position(ctx, cle, &engs, jour)?;
        if p.heures_ouverture()? >= c.seuil_heures {
            let fiche = ctx.salaries.get(cle).map(|f| &f.champs);
            let nom = match fiche {
                Some(f) => prenom(f, cle),
                None => cle.to_string(),
            };
            fiches.push((nom, fiche, anniv));
        }
    }
    if fiches.is_empty() {
        return Ok(Vec::new());
    }
    // Les annexes de ceux que la phrase nomme : la reference cite X pour un artiste, VIII pour un technicien, les deux
    // quand la liste les mele.
    let mut en_jeu: Vec<Annexe> = Vec::new();
    for (_, fiche, _) in &fiches {
        let annexe = match fiche {
            Some(f) => annexe_de(f)?,
            None => None,
        };
        for a in annexes_en_jeu(annexe) {
            if !en_jeu.contains(&a) {
                en_jeu.push(a);
            }
        }
    }
    let noms: Vec<(String, bool)> = fiches
        .iter()
        .map(|(p, fiche, anniv)| {
            let homonyme = fiches.iter().filter(|(q, _, _)| q == p).count() > 1;
            let nom = fiche.and_then(|f| f.nom_affichage.remplie().or(f.nom.portee())).map(str::trim);
            let n = match (homonyme, nom) {
                (true, Some(nom)) => format!("{} {}", p, nom),
                _ => p.clone(),
            };
            (n, *anniv)
        })
        .collect();
    let mut v = Valeurs::new();
    v.insert("seuil".into(), val_grandeur(c.seuil_heures));
    // `{noms_anniversaire}` (et `{n_anniversaire}`) : ceux dont la fiche porte la date anniversaire ;
    // `{noms_glissants}` : les autres. Un groupe vide ne se pose pas : les conseils qui le nomment se taisent.
    for (groupe, anniv) in [("anniversaire", true), ("glissants", false)] {
        let qui: Vec<String> = noms.iter().filter(|(_, a)| *a == anniv).map(|(n, _)| n.clone()).collect();
        if !qui.is_empty() {
            v.insert(format!("n_{}", groupe), val_compte(qui.len()));
            v.insert(format!("noms_{}", groupe), val_texte(et(&qui)));
        }
    }
    Ok(conseil(&b, &d, Quand::Resume, &v, &en_jeu)?.into_iter().collect())
}

/// « Anastasia », « Anastasia et Valentin », « Anastasia, Fanchon et Valentin ».
fn et(noms: &[String]) -> String {
    match noms {
        [] => String::new(),
        [seul] => seul.clone(),
        [tete @ .., dernier] => format!("{} et {}", tete.join(", "), dernier),
    }
}

// ------------------------------------------------------------------ routes

/// La charge de `conseil-du-jour` : le decalage d'« un autre » ; sans lui, le conseil du jour.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeConseilDuJour {
    decalage: Option<usize>,
}

/// La charge de `conseils-salarie` : la cle du salarie.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeConseilsSalarie {
    salarie: droit::contexte::IdSalarie,
}

/// Ce que rend `conseil-du-jour` : le conseil, ou rien quand l'option est decochee ; sa phrase est son texte.
#[derive(Serialize)]
struct ConseilDuJour {
    phrase: Phrase,
    #[serde(skip_serializing_if = "Option::is_none")]
    conseil: Option<ConseilVu>,
}

/// Ce que rendent `conseils-salarie` et `conseils-resume` : les conseils, et le texte de chacun, une phrase par conseil.
#[derive(Serialize)]
struct Conseils {
    phrase: Phrase,
    phrases: Vec<Phrase>,
    conseils: Vec<ConseilVu>,
}

impl Dite for ConseilDuJour {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for Conseils {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Conseils {
    fn de(conseils: Vec<ConseilVu>) -> Conseils {
        Conseils { phrase: socle::phrase!("Conseils : {}.", conseils.len()), phrases: conseils.iter().map(|c| c.texte.clone()).collect(), conseils }
    }
}

/// L'aiguillage des trois routes de lecture.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::ConseilDuJour => {
            let ChargeConseilDuJour { decalage } = charge.lire()?;
            let conseil = du_jour(ctx, decalage)?;
            let phrase = match &conseil {
                Some(c) => c.texte.clone(),
                None => Phrase::fixe("Pas de conseil aujourd'hui."),
            };
            en_json(ConseilDuJour { phrase, conseil })
        }
        Route::ConseilsSalarie => {
            let ChargeConseilsSalarie { salarie } = charge.lire()?;
            let rendus: Conseils = Conseils::de(du_salarie(ctx, &salarie)?);
            en_json(rendus)
        }
        Route::ConseilsResume => {
            let crate::SansCharge {} = charge.lire()?;
            let rendus: Conseils = Conseils::de(resume(ctx)?);
            en_json(rendus)
        }
        autre => Err(route_inconnue(autre.nom())),
    }
}
