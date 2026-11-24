//! Les routes du mois a l'ecran : `saisie`, `apercu`, `enregistrer-saisie`, `ajuster`, et ce que la saisie
//! consulte (`contrats-en-cours`, `contrats-passes`, `emploi`).
//!
//! `saisie` : le mois tel que l'ecran l'ouvre -- la saisie que ses engagements montrent (`engagement::stock::vue_du_mois`)
//! ou, pour un mois neuf, un squelette vide au dernier taux saisi --, avec les noms resolus en cles de fiche et les lignes
//! de paie telles que le moteur les comprend. Aucun calcul pour l'ecran : la saisie se charge jusqu'au contrat qui
//! deborde, sans chiffrer. Les engagements valides que l'ecran montre se lisent par `engagement::etats::du_mois`, qui les
//! recalcule s'ils ne sont plus justes.
//!
//! `apercu` : le mois pour l'ecran, rien d'ecrit. Un engagement valide montre la paie qu'il a ecrite ; un engagement en
//! saisie se calcule en memoire, tel que son brouillon est enregistre (`paies_de_l_ecran`) ; une modification pas encore
//! ecrite d'un engagement valide se calcule de ses changements (`mois_designe`) ; les numeros se reservent sans rien
//! ecrire.
//!
//! Chaque charge est une structure (`ChargeCase`, `ChargeApercu`, `ChargeAjuster`, `ChargeContratsPasses`,
//! et `etapes::ChargeMois`), lue une fois a la porte (`Charge::lire`) ; chaque reponse en est une aussi,
//! qui porte sa phrase (`Dite`).
//!
//! La carte d'un engagement et la vue du mois sont `engagement::saisie::{Carte, VueDuMois}`. Les cartes importees
//! (`import::mois::cartes`) se composent en JSON dans leur module, et passent ici telles quelles ; le conseil de saisie
//! est la forme de son module (`gestes::conseils::ConseilVu`).

use droit::fiche::Lue as _;
use droit::fiche::NumeroObjetSpectacle;
use engagement::numeros::NumeroContrat;
use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::fiches::refus;
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use crate::etapes::ChargeMois;
use engagement::calcul::Paie;
use droit::natures::NatureDeContrat;
use engagement::carte::{Brouillon, Changements};
use engagement::saisie::{Carte as SaisieCarte, RangDansLeMois, VueDuMois};
use engagement::stock::Identite;
use engagement::viser::Cible;
use droit::catalogue;
use droit::contexte::{Contexte, IdSalarie};
use droit::emplois;
use engagement::{self, LignePaie, PlaceDuMois};
use socle::date::{Jour, Periode, PeriodeOuverte};
use socle::echec::{erreur, Classe, Echec, R};
use socle::mois::{Graphie, Mois};
use socle::grandeurs::{Cachets, Coefficient, Heures, Jours, Montant, MontantUnitaire};

use crate::catalogue::{Acte, Affiche, Forme, Fiche, FORME, ILLISIBLE, MANQUE, OPERATION};
use crate::catalogue::Route;
use crate::declaration::{charge, dit, legendes, Legende, ParSaForme, ValeurDeLaCharge};
use crate::engagement::{CHANGEMENTS, IDENTITE, SAISIE_INCOMPLETE};
use crate::catalogue::{Ecran, Place};

const MODIFICATIONS: Legende = dit(
    "Modifications en cours",
    "les engagements validés ouverts pour être modifiés, pas encore écrits : [{engagement (son identité), changements (comme modifier-engagement)}] ; [] sans aucun. Une carte en saisie se lit telle qu'elle est enregistrée",
);

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);


/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Saisie,
        geste: "Lire les engagements d'un mois, leurs identités et le cachet par défaut.",
        charge: &crate::etapes::ChargeMois::CHARGE,
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "saisie (les engagements du mois), identites, a_completer (ce qui manque à chacun pour se lire, ou null), importes (chaque paie importée telle qu'elle se lit : son résumé et ses pastilles, ce que l'import n'a pas lu, ses cases en lecture, les lignes de son bulletin), ferme_aux_neufs, nouvelle_carte (les natures qu'une carte neuve du mois peut prendre, la case de la nature quand plus d'une se choisit et ses choix, nature par nature les rubriques qu'elle propose avant de nommer son salarié, et prerempli.lignes : ce qu'une ligne neuve prend, le taux du cachet par défaut), phrase (ce que l'écran dit de la saisie), vide_dit (ce que dit le mois quand l'aperçu n'a aucune paie d'ici), demo (la mention d'un mois saisi en démo)",
        refus: &[MANQUE, ILLISIBLE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Apercu,
        geste: "Calculer sans rien écrire les paies du mois, avec les modifications en cours.",
        charge: &ChargeApercu::CHARGE,
        exemple: r#"{"mois":"2026-11","modifications":[]}"#,
        rend: "paies (une par engagement, ses bornes debut et fin, ou ce qui lui manque : a_completer ; carte : les cases que sa carte ouvre, chacune sa place (base, avance, imprevu : la fin du contrat, rendue : montrée, pas saisie), ce qu'offrent ses cases à choix qui dépendent de lui (motif_rupture, modele_contrat), ce que le moteur prend pour une case vide (prerempli, dont le métier que la paie applique, et lignes : ce qu'une ligne neuve prend), et faute quand un fait ne se lit pas ; contrat_dit : ses bornes et ses jours travaillés, lus sous ses lignes ; contrat.rupture : sa fin, la porte qui l'ouvre et si elle se signale, ou illisible ; a_dire : la case de fiche qu'il appelle, comme « Pour démarrer » ; propose : les rubriques que sa carte propose, d'après sa nature et le statut de son métier, et celle d'une ligne neuve ; cases : celles qu'une ligne de frais appelle, avec leur aide ; avertissements et informations : chacun son texte, et le lien vers le texte qu'il renvoie à vérifier, à part : libellé, adresse), les trois nombres de chaque carte, le mois, ses deux nombres (total_net, total_cout) et ce qu'ils disent (total_net_dit, total_cout_dit)",
        refus: &[SAISIE_INCOMPLETE, MANQUE, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ContratsEnCours,
        geste: "Lire les contrats d'un mois passé qui courent encore ce mois.",
        charge: &crate::etapes::ChargeMois::CHARGE,
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "contrats : chacun avec l'engagement qui le reprend",
        refus: &[MANQUE, ILLISIBLE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ContratsPasses,
        geste: "Lire les contrats déclarés d'un mois importé.",
        charge: &ChargeContratsPasses::CHARGE,
        exemple: r#"{"mois":"2026-11","salarie":null}"#,
        rend: "contrats : ceux que les engagements importés attestent ; non_lus : les contrats repris qui ne se lisent pas",
        refus: &[MANQUE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Paie),
    },
    Fiche {
        route: Route::EnregistrerSaisie,
        geste: "Changer des cases du brouillon d'une carte, et l'écrire.",
        charge: &ChargeCase::CHARGE,
        exemple: r#"{"mois":"2026-11","carte":{"neuve":"cddu_spectacle"},"changements":[{"case":"salarie","valeur":"FULLER"},{"case":"spectacle","valeur":"le-banc-des-menteurs"},{"case":"lignes","valeur":[{"rubrique":"cachet","quantite":{"cachets":1},"taux":150,"lieu":null,"dates":["2026-11-20"],"planning":[]}]}]}"#,
        rend: "engagement (l'identité écrite, à passer à valider-engagement), brouillon (chacune des cases de sa nature, null tant qu'elle n'est pas remplie ; chaque grandeur écrite par elle-même), a_completer (ce qui lui manque pour être validé, ou null), enregistre_a",
        refus: &[OPERATION, FORME, MANQUE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::CopierEngagement,
        geste: "Copier un engagement du mois pour un autre salarié, sans rien écrire.",
        charge: &ChargeCarteProposee::CHARGE,
        exemple: r#"{"mois":"2026-11","engagement":{"salarie":"FULLER","debut":"2026-11-27","rang":0}}"#,
        rend: "phrase, brouillon (la carte neuve proposée : le spectacle, les lignes et leurs dates, le métier, les termes du contrat et la date du paiement ; vides : le salarié, le numéro, ce que la carte rend sans le faire saisir, ce que le salarié a fait ou pris ce mois, la fin du contrat, le justificatif des frais), enregistrer (le geste qui l'écrit : enregistrer-saisie sur une carte neuve, avec les changements qui la font ; s'y ajoutent ceux qu'on y fait) ; rien n'est écrit",
        refus: &[OPERATION, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 200,
        // `etat-par-engagement` la rend en geste sur la ligne de chaque engagement (`copier`) ; le menu « … » de la
        // ligne la montre, et la page de la paie l'appelle et dessine la carte proposée (`js/carte.js`,
        // `copierLEngagement`), comme une carte neuve.
        ecran: Ecran::Dessine(Place::Paie),
    },
    Fiche {
        route: Route::Ajuster,
        geste: "Trouver le taux qui donne un net, un brut ou un coût voulu.",
        charge: &ChargeAjuster::CHARGE,
        exemple: r#"{"mois":"2026-11","modifications":[],"engagement":{"salarie":"FULLER","debut":"2026-11-27","rang":0},"cible":"brut","montant":200}"#,
        rend: "phrase (ce qui est atteint, et le taux qu'il faudrait), vise, atteint, ecart, engagements (les taux qui changent), paies ; rien n'est écrit",
        refus: &[OPERATION, FORME, SAISIE_INCOMPLETE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 200,
        ecran: Ecran::DessineAvecBouton(Place::Paie, Affiche::new("Calculer")),
    },
];

// ------------------------------------------------------------------ les charges

/// UNE MODIFICATION PAS ENCORE ECRITE D'UN ENGAGEMENT VALIDE (`apercu`, `ajuster`) : la carte ouverte de l'engagement
/// `engagement` (son identite), et ce qu'on y a change, comme `modifier-engagement` l'ecrira (`changements`). Elle ne
/// s'ecrit qu'a « Modifier l'engagement » : l'apercu et le montant vise la calculent en memoire.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModificationEnCours {
    pub(crate) engagement: Identite,
    pub(crate) changements: Changements,
}

/// LA CARTE QU'UN CHANGEMENT VISE : une carte neuve, nee dans sa nature (`nouvelle_carte` de la route saisie, ou la
/// carte proposee de `copier-engagement`), ou celle d'un engagement ecrit, par son identite.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CarteVisee {
    Neuve(NatureDeContrat),
    Ecrite(Identite),
}

/// Dans une charge, une modification en cours et la carte visee se disent d'un objet.
impl ValeurDeLaCharge<ParSaForme> for ModificationEnCours {
    const FORME: Forme = Forme::Objet;
}
impl ValeurDeLaCharge<ParSaForme> for CarteVisee {
    const FORME: Forme = Forme::Objet;
}

charge! {
    /// LA CHARGE DES CHANGEMENTS D'UNE CARTE (`enregistrer-saisie`) : le mois de l'ecran, la carte visee, et ses
    /// changements, dans l'ordre. La carte proposee a un autre salarie la rend (`copier-engagement`, `enregistrer`).
    #[derive(Debug, Clone, Serialize)]
    pub(crate) struct ChargeCase {
        pub(crate) mois: Mois = legendes::MOIS,
        pub(crate) carte: CarteVisee = dit("Carte", "{\"neuve\": sa nature (l'une des `nouvelle_carte.natures` de la route saisie)} pour une carte neuve, ou {\"ecrite\": son identité, celle que le moteur a rendue}"),
        pub(crate) changements: Changements = CHANGEMENTS,
    }
}

charge! {
    /// La charge de la carte proposee a un autre salarie (`copier-engagement`) : le mois de l'ecran, et l'identite de
    /// l'engagement copie. `etat-par-engagement` la rend dans le geste de chaque ligne (`copier`).
    #[derive(Debug, Clone, Serialize)]
    pub(crate) struct ChargeCarteProposee {
        pub(crate) mois: Mois = legendes::MOIS,
        pub(crate) engagement: Identite = IDENTITE,
    }
}

charge! {
    /// La charge de l'apercu : le mois, et les modifications pas encore ecrites d'engagements valides (`mois_designe`).
    #[derive(Debug)]
    struct ChargeApercu {
        mois: Mois = legendes::MOIS,
        modifications: Vec<ModificationEnCours> = MODIFICATIONS,
    }
}

charge! {
    /// La charge du montant vise : le mois et ses modifications pas encore ecrites, l'engagement a ajuster (son
    /// identite), ce qui se vise et combien.
    #[derive(Debug)]
    struct ChargeAjuster {
        mois: Mois = legendes::MOIS,
        modifications: Vec<ModificationEnCours> = MODIFICATIONS,
        engagement: Identite = IDENTITE,
        cible: Cible = dit("Ce qui se vise", "le montant que la paie atteint ; les indemnités de frais sont les repas, nuitées et défraiements, et l'acompte reste retiré du net"),
        /// Le montant vise, un nombre, ou tel qu'il est tape (« 1 850,00 € »), converti a la porte (`Charge::lire`).
        montant: Montant = dit("Montant voulu", "le montant voulu, en euros"),
    }
}

charge! {
    /// La charge des contrats d'un mois passe : le mois, et le salarie dont ne lire que les siens (absent : tous).
    #[derive(Debug)]
    struct ChargeContratsPasses {
        mois: Mois = legendes::MOIS,
        salarie: Option<IdSalarie> = dit("Salarié", "la clé d'un salarié, pour ne lire que les siens ; null : tous").avancee(),
    }
}

// ------------------------------------------------------------------ les reponses

/// UNE CARTE NEUVE DU MOIS S'OUVRE EN CDDU, sa nature ecrite des l'ouverture : le logiciel paie surtout des
/// intermittents, un permanent s'ajoute rarement. Les natures qu'elle peut prendre sont celles que le mois propose
/// (`engagement::permanents::Permanents::natures_neuves`) : un CDI ou un CDD neuf seulement quand la fiche Entreprise dit
/// employer des permanents ce mois-la ; sinon rien ne se choisit, et la carte n'a pas de case nature. Une carte qui a
/// deja sa nature la garde. Changer de nature change la carte de variante (`engagement::carte::CarteValide`).
#[derive(Debug, Serialize)]
struct NouvelleCarte {
    nature: NatureDeContrat,
    natures: Vec<NatureDeContrat>,
    /// Les cases qu'une carte neuve montre en plus de celles de sa nature : la nature, quand plus d'une se choisit ce mois.
    cases: Vec<engagement::carte::CaseDecrite<crate::catalogue::Forme>>,
    /// Ce qu'elles offrent, sous la cle de leur case : les natures qu'une carte neuve peut prendre ce mois.
    choix: std::collections::BTreeMap<&'static str, Vec<crate::ouverture::Choix>>,
    /// L'aide d'un mois vide (`ouverture::AIDES`) : celle qui nomme le salaire mensuel d'un permanent quand le mois en
    /// propose ou en paie un.
    aide_vide: &'static str,
    /// Les rubriques qu'une carte neuve propose avant de nommer son salarie, nature par nature (`natures`) : celles de sa
    /// nature (`engagement::proposees`).
    propose: std::collections::BTreeMap<NatureDeContrat, engagement::proposees::Proposees>,
    /// Ce qu'une carte neuve pre-remplit avant sa premiere paie (`Prerempli`) : ses lignes neuves.
    prerempli: Prerempli,
}

/// LE MOIS TEL QUE L'ECRAN L'OUVRE (`saisie`).
#[derive(Debug, Serialize)]
struct SaisieDuMois {
    mois: Mois,
    phrase: Phrase,
    /// Les cartes importees (`import::mois::cartes`).
    importes: Vec<import::mois::CarteImportee>,
    importes_titre: &'static str,
    importes_dit: String,
    ferme_aux_neufs: Option<String>,
    /// Ce que dit le mois quand l'apercu n'a aucune paie d'ici : que ses engagements viennent de l'ancien logiciel, ou
    /// l'aide d'un mois vide (`NouvelleCarte::aide_vide`).
    vide_dit: &'static str,
    /// La mention d'un mois saisi en demo, dans une compagnie gardee (`accueil::suppression::mention`) ; aucune sinon.
    #[serde(skip_serializing_if = "Option::is_none")]
    demo: Option<String>,
    nouvelle_carte: NouvelleCarte,
    /// La vue du mois (`engagement::stock::vue_du_mois`), les lignes des permanents calculees (`avec_les_lignes_calculees_des_permanents`).
    saisie: VueDuMois,
    /// L'identite de chaque engagement de `saisie.engagements`, dans son ordre : ce par quoi sa carte le designe.
    identites: Vec<Identite>,
    /// Ce qui manque a chaque engagement de `saisie.engagements` pour se lire, dans son ordre (`null` : rien) : la case
    /// de la fiche de son salarie, une saisie pas finie (`avec_les_lignes_calculees_des_permanents`).
    a_completer: Vec<Option<String>>,
}

/// Une carte ecrite (`enregistrer-saisie`).
#[derive(Debug, Serialize)]
struct CarteEcrite {
    phrase: Phrase,
    engagement: Identite,
    /// Le brouillon tel qu'il est ecrit, chacune de ses cases (`null` : pas encore remplie).
    brouillon: Brouillon,
    a_completer: Option<String>,
    #[serde(with = "socle::paris::ecrit::seconde")]
    enregistre_a: socle::paris::Moment,
    enregistre_dit: String,
}

/// LA CARTE PROPOSEE A UN AUTRE SALARIE, copiee d'un engagement du mois (`copier-engagement`,
/// `Brouillon::proposee_a_un_autre_salarie`) : la carte neuve telle qu'elle s'ouvre, et le geste qui l'ecrit
/// (`enregistrer-saisie` sur une carte neuve, avec les changements qui la font, ceux-la memes dont la carte montree est
/// faite). Rien n'est ecrit : elle s'ecrit a son premier changement, qui s'ajoute aux siens.
#[derive(Debug, Serialize)]
struct CarteProposee {
    phrase: Phrase,
    brouillon: Brouillon,
    enregistrer: crate::etapes::Action,
}

/// UN CONTRAT D'USAGE REPRIS D'UN AUTRE LOGICIEL QUI COURT SUR LE MOIS, et l'engagement qui le reprend.
#[derive(Debug, Serialize)]
struct EnCours {
    salarie: IdSalarie,
    nom: String,
    spectacle: Option<droit::contexte::IdSpectacle>,
    contrat: Periode,
    numero: NumeroContrat,
    depuis: Option<Mois>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    taux_cachet: Option<MontantUnitaire>,
    deja_repris: bool,
    engagement: Reprise,
}

/// L'ENGAGEMENT D'UN CONTRAT EN COURS, pour le fichier du mois : le contrat tel qu'il a ete signe, et une ligne de
/// cachets a dater.
#[derive(Debug, Serialize)]
struct Reprise {
    nature: NatureDeContrat,
    salarie: IdSalarie,
    #[serde(skip_serializing_if = "Option::is_none")]
    spectacle: Option<droit::contexte::IdSpectacle>,
    debut: Jour,
    fin: Jour,
    numero: NumeroContrat,
    lignes: [LigneAReprendre; 1],
}

/// La ligne de cachets d'un contrat repris : au taux du contrat ou au dernier saisi, sans quantite (aucun cachet n'est
/// encore dit : la case est vide, pas zero).
#[derive(Debug, Serialize)]
struct LigneAReprendre {
    rubrique: &'static str,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    taux: Option<MontantUnitaire>,
    dates: [Jour; 0],
}

#[derive(Debug, Serialize)]
struct ContratsEnCours {
    phrase: Phrase,
    mois: Mois,
    contrats: Vec<EnCours>,
    reprise: Option<String>,
}

/// UN CONTRAT QUI TOUCHE UN MOIS PASSE, tel que le calendrier le montre. LECTURE SEULE, ET UN PLANCHER : c'est ce que la
/// compagnie a declare, pas la position du salarie.
#[derive(Debug, Serialize)]
struct Passe {
    dit: String,
    salarie: IdSalarie,
    nom: String,
    periode: Periode,
    /// Des cachets entiers ; des heures et des jours tels que les pieces les disent (la DSN : `151.67`), en nombre JSON,
    /// entier quand ils le sont.
    #[serde(serialize_with = "socle::grandeurs::ecrit::entier::option::serialize")]
    cachets: Option<Cachets>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    heures: Option<Heures>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    jours: Option<Jours>,
    titre: Option<String>,
    numero_objet: Option<NumeroObjetSpectacle>,
    spectacle: Option<droit::contexte::IdSpectacle>,
    dates: Vec<Jour>,
    source: SourceDuContratPasse,
}

/// D'OU LE CALENDRIER TIENT UN CONTRAT PASSE : un engagement du logiciel (`paie`), ou l'historique de l'ancien logiciel
/// (`historique`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SourceDuContratPasse {
    Paie,
    Historique,
}

#[derive(Debug, Serialize)]
struct ContratsPasses {
    phrase: Phrase,
    mois: Mois,
    contrats: Vec<Passe>,
    /// Les contrats repris qui ne se lisent pas, une phrase chacun : ils ne ferment pas le reste.
    non_lus: Vec<String>,
    rien_dit: &'static str,
}

/// Une ligne de la carte dont le taux change, pour le montant vise. TRAVAIL A FAIRE (chaine A) : les taux et la rubrique
/// se lisent de la saisie, telle que son fichier l'ecrit.
#[derive(Debug, Serialize)]
struct TauxQuiChange {
    rang: engagement::ligne::RangDeLigne,
    rubrique: catalogue::CodeDeRubrique,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    taux_avant: Option<MontantUnitaire>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    taux: Option<MontantUnitaire>,
}

/// Ce qui a change, pour l'engagement ajuste : son salaire du mois s'il change, ses lignes dont le taux change.
#[derive(Debug, Serialize)]
struct CeQuiChange {
    index: RangDansLeMois,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "salaire_qui_change")]
    salaire_mensuel: Option<Option<MontantUnitaire>>,
    lignes: Vec<TauxQuiChange>,
}

/// LE MONTANT VISE (`ajuster`) : ce qui est atteint, le taux qu'il faudrait ; rien n'est ecrit.
#[derive(Debug, Serialize)]
struct Ajuste {
    phrase: Phrase,
    cible: Cible,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    vise: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    atteint: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    depart: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    ecart: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    facteur: Coefficient,
    tours: usize,
    engagements: [CeQuiChange; 1],
    paies: [VuePaie; 1],
}

// ------------------------------------------------------------------ les routes

/// Une route de ce module.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::Saisie => en_json(saisie(ctx, charge.lire::<ChargeMois>()?.mois)?),
        Route::Apercu => en_json(apercu(ctx, charge.lire()?)?),
        Route::ContratsEnCours => en_json(contrats_en_cours_route(ctx, charge.lire::<ChargeMois>()?.mois)?),
        Route::ContratsPasses => en_json(contrats_passes_route(ctx, charge.lire()?)?),
        Route::EnregistrerSaisie => en_json(enregistrer_saisie(ctx, charge.lire()?)?),
        Route::CopierEngagement => en_json(copier_engagement(ctx, charge.lire()?)?),
        Route::Ajuster => en_json(ajuster(ctx, charge.lire()?)?),
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

macro_rules! dites {
    ($($t:ty),* $(,)?) => {
        $(impl Dite for $t {
            fn phrase(&self) -> &Phrase {
                &self.phrase
            }
        })*
    };
}

dites!(SaisieDuMois, ContratsEnCours, ContratsPasses, CarteEcrite, CarteProposee, Ajuste, Apercu);

// ------------------------------------------------------------------ outils



// ============================================================ la saisie

/// Un mois neuf s'ouvre VIDE, avec le
/// dernier taux saisi -- `null` quand le dossier n'a encore aucun cachet (R12 :
/// aucun texte ne fixe un taux de cachet, la compagnie le saisit).
pub(crate) fn squelette(ctx: &Contexte, mois: Mois) -> R<VueDuMois> {
    Ok(VueDuMois { taux_cachet: dernier_taux(ctx)?.map(socle::grandeurs::courte), ..VueDuMois::vide(mois) })
}


/// Le taux du cachet le plus recemment saisi : une ligne de cachet d'abord, le `taux_cachet` du mois a defaut ; `None` si
/// aucun mois n'en dit rien -- aucun texte ne fixe un taux de cachet (regle 1, R12), la compagnie saisit le sien.
/// Il se lit une fois par geste, dans la lecture du geste (`socle::forme::une_fois_par_geste`, sur le dossier de la
/// compagnie) : le geste qui ecrit une carte le refait.
pub(crate) fn dernier_taux(ctx: &Contexte) -> R<Option<MontantUnitaire>> {
    /// Le dernier taux de cachet du dossier, tel que le geste l'a lu.
    struct DernierTaux(Option<MontantUnitaire>);
    let lu = socle::forme::une_fois_par_geste(&ctx.racine, || -> R<DernierTaux> {
        let mut mois = engagement::stock::mois_saisis(&ctx.racine)?;
        mois.reverse();
        for m in mois {
            let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, m)? else { continue };
            if let Some(t) = taux_du_mois(ctx, &data)? {
                return Ok(DernierTaux(Some(t)));
            }
        }
        Ok(DernierTaux(None))
    })?;
    Ok(lu.0)
}

/// Le taux du dernier cachet de la saisie `data`, sinon son `taux_cachet` (un taux nul ne dit rien).
fn taux_du_mois(ctx: &Contexte, data: &VueDuMois) -> R<Option<MontantUnitaire>> {
    for e in data.engagements.iter().rev() {
        for l in e.lignes().iter().rev() {
            if let Some(t) = taux_d_une_ligne(ctx, l)? {
                return Ok(Some(t));
            }
        }
    }
    Ok(data.taux_cachet.filter(|t| !t.est_nul()))
}

/// Le taux d'une ligne de cachet ; rien pour une autre ligne, ou un cachet sans taux.
fn taux_d_une_ligne(ctx: &Contexte, l: &LignePaie) -> R<Option<MontantUnitaire>> {
    if droit::catalogue::rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?.unite != droit::catalogue::Unite::Cachet {
        return Ok(None);
    }
    Ok(l.taux().and_then(engagement::ligne::TauxDeLigne::par_unite).filter(|t| !t.est_nul()))
}

/// LES ENGAGEMENTS DU MOIS TELS QUE L'ECRAN LES OUVRE, chacun lu seul (`engagement::lue_sans_paie` : une paie ne lit
/// que son engagement et la fiche de son salarie) : ses lignes telles que le moteur les a comprises (`lignes_affichees`),
/// et, dans le meme ordre, ce qui lui manque pour se lire (`None` : rien). Un engagement qui ne se lit pas encore (une
/// saisie pas finie, une case de la fiche de son salarie) se rend tel qu'il est ecrit, avec sa phrase : ouvrir le mois
/// n'est pas payer, et le refus vient au calcul de sa paie (`apercu`, `valider-engagement`), pas aux autres.
fn avec_les_lignes_calculees_des_permanents(ctx: &Contexte, saisie: VueDuMois) -> R<(VueDuMois, Vec<Option<String>>)> {
    let sans_engagement = VueDuMois { engagements: Vec::new(), ..saisie.clone() };
    let mut engagements = Vec::new();
    let mut a_completer = Vec::new();
    for e in &saisie.engagements {
        let seul = VueDuMois { engagements: vec![e.clone()], ..sans_engagement.clone() };
        let mut resolu = e.clone();
        match engagement::lue_sans_paie(ctx, &seul) {
            Ok(paies) => {
                for p in &paies {
                    // Les lignes d'un permanent sont CALCULEES, pas saisies.
                    if !p.engagement.nat()?.mensuelle() {
                        resolu.lignes = Some(lignes_affichees(p, e.lignes())?);
                    }
                }
                a_completer.push(None);
            }
            Err(m) if m.dit_un_manque() => a_completer.push(Some(m.phrase)),
            // Une panne (le disque, un fichier livre) n'est pas un manque de l'engagement : elle remonte.
            Err(m) => return Err(m),
        }
        engagements.push(resolu);
    }
    Ok((VueDuMois { engagements, ..saisie }, a_completer))
}

/// Ce que dit, a l'ecran, la case d'une ligne de frais (`engagement::ligne::Marques::cases`).
fn libelle_de_case(case: engagement::CaseDeFrais) -> &'static str {
    match case {
        engagement::CaseDeFrais::Paris => "Paris ou petite couronne",
        engagement::CaseDeFrais::Restaurant => "au restaurant",
        engagement::CaseDeFrais::Loge => "hôtel payé par la compagnie",
        engagement::CaseDeFrais::Justificatif => "justificatif conservé",
    }
}

/// LES LIGNES DE PAIE TELLES QUE LE MOTEUR LES A COMPRISES. `remuneration` suit les lignes du fichier une a une
/// (`calcul::lignes_brut`) ; la part d'un frais au-dela de sa limite, que le moteur pose apres elles
/// (`engagement::frais_au_dela`), ne se montre pas en ligne de saisie, et la ligne garde le taux saisi
/// (`ligne::Marques::taux_saisi`). Une ligne en unites ou de frais prend la quantite et le taux que le calcul a lus (le
/// tarif de la convention d'un repas qui ne dit pas le sien) ; le reste de la ligne du fichier se garde (ses heures, ses
/// cases de frais, ses jours et son lieu ; une ligne en pourcentage, au montant ou en avantage en nature telle quelle) :
/// la page le relit pour le remettre dans ses champs.
fn lignes_affichees(p: &engagement::calcul::Paie, brutes: &[LignePaie]) -> R<Vec<LignePaie>> {
    let mut out = Vec::new();
    for (i, (l, src)) in p.remuneration.iter().zip(brutes).enumerate() {
        let saisi = p.engagement.lignes.get(i).and_then(|x| x.marques()).and_then(|m| m.taux_saisi);
        let quantite = l.paiement.quantite();
        let taux = match saisi {
            Some(t) => Some(t),
            None => l.paiement.taux().par_unite(),
        };
        out.push(match src.clone() {
            LignePaie::Unites(x) => LignePaie::Unites(engagement::ligne::Unites { quantite, taux, ..x }),
            LignePaie::UnitesEtHeures(x) => LignePaie::UnitesEtHeures(engagement::ligne::UnitesEtHeures { quantite, taux, ..x }),
            LignePaie::Frais(x) => LignePaie::Frais(engagement::ligne::Frais { quantite, taux, ..x }),
            autre => autre,
        });
    }
    Ok(out)
}

/// La route `saisie`.
fn saisie(ctx: &Contexte, mois: Mois) -> R<SaisieDuMois> {
    let existe = engagement::stock::a_une_saisie(&ctx.racine, mois)?;
    // Un mois neuf s'ouvre VIDE, avec le dernier taux saisi (`squelette`) ; un mois que des engagements touchent, tel qu'ils
    // le montrent.
    // Ce qui ne se pre-remplit pas ne ferme pas le mois : la phrase de la route le dit (`ne_se_preremplit`).
    let mut ne_se_preremplit = Vec::new();
    let saisie = match engagement::stock::vue_du_mois(&ctx.racine, mois)? {
        Some(vue) => vue,
        None => match squelette(ctx, mois) {
            Ok(vue) => vue,
            Err(e) => {
                ne_se_preremplit.push(e.phrase);
                VueDuMois::vide(mois)
            }
        },
    };
    let lignes_neuves = lignes_ou_vide(&lignes_preremplies(ctx, &saisie), &mut ne_se_preremplit);
    // LES ENGAGEMENTS IMPORTES QUI TOUCHENT LE MOIS, en cartes fermees : ils se lisent, rien ne les calcule
    // (`import::mois`). Un mois qui en porte ne recoit pas d'engagement neuf : `ferme_aux_neufs` est la phrase qui le
    // dit (`null` : le mois en recoit).
    let importes = import::mois::cartes(ctx, mois)?;
    let ferme = ferme_aux_neufs(ctx, mois)?;
    // CE QUE L'ECRAN DIT DE LA SAISIE : pour un mois importe, `ferme_aux_neufs`.
    let phrase = match (&ferme, existe) {
        (Some(f), _) => socle::phrase!("{}", f),
        (None, true) => Phrase::fixe("Saisie chargée. L'enregistrement est automatique."),
        (None, false) => Phrase::fixe("Nouveau mois — l'enregistrement est automatique dès la première saisie."),
    };
    let phrase = match ne_se_preremplit.is_empty() {
        true => phrase,
        false => Phrase::jointes([phrase, socle::phrase!("Le taux d'un cachet neuf ne se pré-remplit pas : {}", ne_se_preremplit.join(" "))], " "),
    };
    // CE QUE LE MOIS PROPOSE D'UNE CARTE NEUVE : ses natures, lues sur le mois (`engagement::permanents`).
    let p = engagement::permanents::Permanents::de(ctx, &socle::date::Periode::du_mois(mois))?;
    let mut neuves = Vec::new();
    let mut offertes = Vec::new();
    let mut propose = std::collections::BTreeMap::new();
    for n in p.natures_neuves()?.iter() {
        neuves.push(n.cle);
        offertes.push(crate::ouverture::Choix::de(n.cle, &n.libelle));
        propose.insert(n.cle, engagement::proposees::proposees(ctx, Some(n.mensuelle()), None, None, &mois.premier_jour())?);
    }
    // La case de la nature, quand plus d'une se choisit ce mois : de base, en tete de la carte.
    let au_choix = neuves.len() > 1;
    let nature = engagement::saisie::Champ::Nature;
    let nouvelle_carte = NouvelleCarte {
        nature: NatureDeContrat::CdduSpectacle,
        cases: if au_choix {
            vec![engagement::carte::CaseDecrite {
                cle: nature.nom(),
                libelle: engagement::carte::libelle(nature),
                exige: engagement::carte::Exige::Obligatoire,
                forme: crate::catalogue::Forme::de_la_case(nature),
                ouverte: engagement::carte::Ouverture::Toujours,
                place: engagement::carte::PlaceDeLaCase::Base,
            }]
        } else {
            Vec::new()
        },
        choix: if au_choix { std::collections::BTreeMap::from([(nature.nom(), offertes)]) } else { std::collections::BTreeMap::new() },
        natures: neuves,
        aide_vide: if p.ouverts() { "apercu_vide_permanents" } else { "apercu_vide" },
        propose,
        // Une carte neuve n'a pas encore de paie ni de carte rendue : seules ses lignes neuves se pre-remplissent.
        prerempli: Prerempli { lignes: lignes_neuves, ..Prerempli::default() },
    };
    // L'en-tete des cartes importees : son titre, et combien d'engagements viennent de l'ancien logiciel.
    let n_importes = importes.len();
    let vide_dit = crate::ouverture::aide(if n_importes > 0 { "importes" } else { nouvelle_carte.aide_vide })?;
    let demo = crate::accueil::suppression::mention(&ctx.racine)?.and_then(|m| m.du_mois(mois).map(str::to_string));
    let (saisie, a_completer) = avec_les_lignes_calculees_des_permanents(ctx, saisie)?;
    Ok(SaisieDuMois {
        mois,
        phrase,
        importes_titre: if n_importes > 1 { "Importés" } else { "Importé" },
        importes_dit: format!("{} de l'ancien logiciel", socle::ecran::compte(n_importes, "engagement", "engagements")),
        importes,
        ferme_aux_neufs: ferme,
        vide_dit,
        demo,
        nouvelle_carte,
        saisie,
        identites: engagement::stock::identites(&engagement::stock::objets_du_mois(&ctx.racine, mois)?),
        a_completer,
    })
}

// ------------------------------------------------------- les jours d'un engagement

/// Les jours travailles d'un engagement brut, AAAA-MM-JJ, tries : ceux de ses lignes de travail
/// (`engagement::dates_engagement`).
fn jours_joues(e: &SaisieCarte) -> Vec<Jour> {
    let mut jours: Vec<Jour> = e.dates().iter().map(|j| j.jour()).collect();
    jours.sort();
    jours
}

// ------------------------------------------------------------ les fichiers de mois

/// Les engagements que le mois montre (`engagement::stock::vue_du_mois`), avec la cle du salarie ; vide si aucun
/// engagement ne le touche. Une carte qui ne nomme pas son salarie est sautee.
pub(crate) fn engagements_du_mois(ctx: &Contexte, mois: Mois) -> R<Vec<(IdSalarie, SaisieCarte)>> {
    // Un mois sans saisie n'a aucun engagement.
    Ok(engagement::stock::vue_du_mois(&ctx.racine, mois)?.into_iter().flat_map(|data| data.engagements).filter_map(|e| Some((e.salarie.clone()?, e))).collect())
}

// ------------------------------------------------------------- contrats en cours

/// LES CONTRATS D'USAGE REPRIS D'UN AUTRE LOGICIEL QUI COURENT SUR `mois`. Un engagement d'ici commence un mois precedent
/// est deja a l'ecran de `mois` (un seul objet, lu dans le fichier du mois ou il commence : `engagement::stock`), comme
/// le contrat repris d'un permanent, qui continue dans son engagement (`engagement::importe`) ; ne restent que les
/// contrats d'usage que l'historique importe porte. Chacun se dit repris (`deja_repris`) quand un engagement de l'ecran
/// a son salarie, son debut et son numero. Jamais lu dans la DSN.
///
/// LE CONTRAT D'USAGE COMMENCE DANS L'ANCIEN LOGICIEL ET CONTINUE ICI (`engagement::importe::Importe::continue_apres`) :
/// ses mois verses sont importes, les suivants se paient ici, sous le numero et les dates que la DSN deposee lui a
/// donnes (regle 4). Le spectacle est celui de son numero d'objet, le cachet celui que l'ancien logiciel a paye ; son
/// engagement, pour le fichier du mois, est le contrat tel qu'il a ete signe, et une ligne de cachets a dater, au taux du
/// contrat, sinon au dernier saisi (la page ne choisit pas de taux ; un dossier qu'on ne sait pas relire, ou qui n'a
/// encore aucun cachet, R12, n'en propose aucun : la ligne reste a completer).
fn contrats_en_cours(ctx: &Contexte, mois: Mois) -> R<Vec<EnCours>> {
    let mut out: Vec<EnCours> = Vec::new();
    let mut vus: HashSet<(IdSalarie, Jour, NumeroContrat)> = HashSet::new();
    for i in engagement::importe::qui_continuent(&ctx.racine, mois)? {
        let b = i.objet().carte();
        // Un contrat qui continue ici porte le salarie, les bornes et le numero que la DSN deposee lui a donnes.
        let (Some(salarie), Some(contrat), Some(numero)) = (b.salarie.clone(), b.contrat()?.and_then(|c| c.fermee()), b.numero.clone()) else {
            return Err(erreur(Classe::Illisible, "Un contrat repris d'un autre logiciel ne dit pas son salarié, ses bornes ou son numéro : le reprendre à l'import."));
        };
        if !vus.insert((salarie.clone(), contrat.debut(), numero.clone())) {
            continue;
        }
        let fiche = ctx.fiche_de(&salarie)?;
        let derniere = i.paies().values().last();
        let numero_objet = match i.contrat().and_then(|c| c.numero_objet.as_ref()) {
            Some(n) => Some(n),
            None => derniere.and_then(|p| p.numero_objet.as_ref()),
        };
        let spectacle = numero_objet.and_then(|no| ctx.spectacles.iter().find(|(_, s)| s.numero_objet.dite().is_some_and(|n| n.meme(no))).map(|(k, _)| k.clone()));
        let taux_cachet = derniere
            .into_iter()
            .flat_map(|p| p.lignes.iter())
            .find(|l| l.rubrique.as_ref() == Some(&droit::catalogue::CACHET))
            .and_then(|l| match l.paiement {
                Some(engagement::ligne::PaiementDeLigne::Cachets(_, t)) => Some(t),
                _ => None,
            });
        let taux = match taux_cachet.filter(|t| !t.est_nul()) {
            Some(t) => Some(t),
            None => dernier_taux(ctx)?,
        };
        let engagement = Reprise {
            // Un contrat qui continue ici est un contrat d'usage (`importe::Importe::continue_apres` : jamais un permanent).
            nature: NatureDeContrat::CdduSpectacle,
            salarie: fiche.id.clone(),
            spectacle: spectacle.clone(),
            // La carte du mois les ecrit en deux cases.
            debut: contrat.debut(),
            fin: contrat.fin(),
            numero: numero.clone(),
            lignes: [LigneAReprendre { rubrique: "cachet", taux, dates: [] }],
        };
        out.push(EnCours {
            nom: ctx.nomme(&salarie),
            salarie: fiche.id,
            spectacle,
            contrat,
            numero,
            depuis: i.continue_apres(),
            taux_cachet,
            deja_repris: false,
            engagement,
        });
    }
    let mut deja: HashSet<(IdSalarie, Option<Jour>, Option<NumeroContrat>)> = HashSet::new();
    for (c, e) in engagements_du_mois(ctx, mois)? {
        deja.insert((c, e.debut_lu(), e.numero.clone()));
    }
    for c in out.iter_mut() {
        c.deja_repris = deja.contains(&(c.salarie.clone(), Some(c.contrat.debut()), Some(c.numero.clone())));
    }
    Ok(out)
}

fn contrats_en_cours_route(ctx: &Contexte, mois: Mois) -> R<ContratsEnCours> {
    let contrats = contrats_en_cours(ctx, mois)?;
    // Ce que l'ecran dit en posant les contrats qui ne sont pas encore repris : qui, et que ses jours restent a poser.
    let de_mois = socle::texte::de(&mois.libelle(Graphie::Accentuee));
    let noms: Vec<&str> = contrats.iter().filter(|c| !c.deja_repris).map(|c| c.nom.as_str()).collect();
    let reprise = match noms.len() {
        0 => None,
        1 => Some(format!("Contrat en cours repris : {} — poser ses jours {}.", noms[0], de_mois)),
        n => Some(format!("{} contrats en cours repris ({}) — poser leurs jours {}.", n, noms.join(", "), de_mois)),
    };
    Ok(ContratsEnCours { phrase: socle::phrase!("Contrats d'un mois passé qui courent encore ce mois : {}.", contrats.len()), mois, contrats, reprise })
}

// ------------------------------------------------------------- contrats passes


const TITRE_A_COMPLETER: &str = "A COMPLETER";
/// Les spectacles retrouves par leur cle ou par leur numero d'objet.
struct Spectacles<'a> {
    ctx: &'a Contexte,
    par_objet: HashMap<&'a NumeroObjetSpectacle, (&'a droit::contexte::IdSpectacle, &'a droit::fiche::ChampsSpectacle)>,
}

impl<'a> Spectacles<'a> {
    fn nouveau(ctx: &'a Contexte) -> Spectacles<'a> {
        let mut par_objet = HashMap::new();
        for (k, v) in &ctx.spectacles {
            if let Some(n) = v.numero_objet.dite() {
                par_objet.entry(n).or_insert((k, v));
            }
        }
        Spectacles { ctx, par_objet }
    }

    /// Le spectacle de cle `x`, sinon de numero d'objet `x` (le texte qu'un engagement ecrit de son spectacle) : sa cle et
    /// sa fiche.
    fn trouver(&self, x: &str) -> Option<(&'a droit::contexte::IdSpectacle, &'a droit::fiche::ChampsSpectacle)> {
        self.ctx.spectacles.get_key_value(x).or_else(|| self.par_objet.iter().find_map(|(n, v)| (***n == *x).then_some(*v)))
    }
}

/// Le titre de la fiche, sinon le numero d'objet ; rien si ni l'un ni l'autre n'est dit.
fn titre_de(spec: Option<&droit::fiche::ChampsSpectacle>, numero: Option<&str>) -> Option<String> {
    match spec.and_then(|s| s.titre.remplie()).map(str::trim).filter(|t| !t.is_empty() && *t != TITRE_A_COMPLETER) {
        Some(t) => Some(t.to_string()),
        None => numero.map(String::from),
    }
}

/// Le nombre de cachets d'un engagement : la somme des quantites de ses lignes dont la rubrique est un cachet (une case
/// vide n'en compte aucun).
fn cachets_de(ctx: &Contexte, e: &SaisieCarte) -> R<Cachets> {
    let mut cachets = Cachets::ZERO;
    for l in e.lignes() {
        let r = catalogue::rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?;
        if let Some(engagement::ligne::QuantiteDeLigne::Cachets(q)) = l.quantite(r.unite) {
            cachets += q;
        }
    }
    Ok(socle::grandeurs::sans_zero(cachets))
}

/// Ce qu'une ligne de la liste dit d'un contrat passe : ses bornes, ses cachets et ses heures, quand ils sont dits.
fn dit_du_contrat(periode: &Periode, cachets: Option<Cachets>, heures: Option<Heures>) -> String {
    use socle::grandeurs::ecrit::sans_zero::dit as nombre;
    let mut dit = format!("{}→{}", periode.debut().jj_mm(), periode.fin().jj_mm());
    if let Some(c) = cachets {
        let c = c.max(Cachets::ZERO);
        dit.push_str(&format!(" · {} {}", nombre(c), if c > Cachets::UN { "cachets" } else { "cachet" }));
    }
    if let Some(h) = heures.filter(|h| *h > Heures::ZERO) {
        dit.push_str(&format!(" + {} h", nombre(h)));
    }
    dit
}

/// UNE LIGNE DU CALENDRIER : un mois verse d'un engagement importe, aux dates du mois dans le contrat, comme le registre
/// de l'import l'atteste (`Importe::mois_par_mois`), ou le contrat entier que rien ne paie ; avec l'activite que les
/// pieces disent du mois (`Importe::activite_du_mois`). Ce que les pieces ne disent pas reste vide.
struct Atteste {
    salarie: IdSalarie,
    periode: Periode,
    numero_objet: Option<NumeroObjetSpectacle>,
    cachets: Option<Cachets>,
    heures: Option<Heures>,
    jours: Option<Jours>,
}

/// Les contrats que les engagements importes attestent ; un contrat sans ses bornes ne se place dans aucun mois. LE
/// CALENDRIER SE LIT TOUJOURS : un engagement importe dont la carte ne se lit pas ne ferme que lui, et se dit (la
/// seconde liste : une phrase par engagement) ; une panne remonte.
fn historique(ctx: &Contexte) -> R<(Vec<Atteste>, Vec<String>)> {
    let (mut out, mut non_lus) = (Vec::new(), Vec::new());
    for i in engagement::importe::tous(&ctx.racine)? {
        let Some(salarie) = i.objet().carte().salarie.clone() else { continue };
        match attestes_par(&i, &salarie) {
            Ok(a) => out.extend(a),
            Err(e) if e.dit_un_manque() => non_lus.push(format!("Un contrat repris de {} ne se lit pas : {}", ctx.nomme(&salarie), e.phrase)),
            Err(e) => return Err(e),
        }
    }
    Ok((out, non_lus))
}

/// Ce que l'engagement importe `i` du salarie `salarie` atteste, pour le calendrier : chaque mois verse, aux dates du
/// mois dans le contrat (`Importe::mois_par_mois`) ; un mois verse que les dates du contrat ne touchent pas (la DSN de
/// fevrier paie un CDDU de fin janvier : le mois d'une paie est celui de sa DSN), et un contrat que rien ne paie (une
/// AEM, un certificat), le contrat entier. L'activite est celle que les pieces
/// disent du mois (`Importe::activite_du_mois`), la seule regle, que les etats lisent aussi.
fn attestes_par(i: &engagement::importe::Importe, salarie: &IdSalarie) -> R<Vec<Atteste>> {
    let objet = i.contrat().and_then(|c| c.numero_objet.clone());
    let atteste = |periode: Periode, mois: Mois, numero_objet: Option<NumeroObjetSpectacle>| {
        let a = i.activite_du_mois(mois);
        Atteste { salarie: salarie.clone(), periode, numero_objet, cachets: a.cachets, heures: a.heures, jours: a.jours }
    };
    let entier = i.objet().carte().contrat()?.and_then(|o| o.fermee());
    if i.paies().is_empty() {
        return Ok(entier.map(|p| atteste(p, p.debut().mois_civil(), objet)).into_iter().collect());
    }
    Ok(i.mois_par_mois()?
        .into_iter()
        .filter_map(|(mois, periode, p)| {
            let objet = objet.clone().or_else(|| p.numero_objet.clone());
            match periode {
                Some(periode) => Some(atteste(periode, mois, objet)),
                // Verse un mois que ses dates ne touchent pas (la DSN du mois suivant le paie) : le contrat entier.
                None => entier.map(|e| atteste(e, mois, objet)),
            }
        })
        .collect())
}

/// LES CONTRATS QUI TOUCHENT `mois`, pour le calendrier des mois passes. Deux sources : un mois qui a son fichier de paie
/// est lu la (avec ses jours exacts), l'historique importe parle pour les autres. Trie par salarie puis debut.
/// `salarie` : ses seuls contrats (absent : ceux de tous).
/// Rend aussi ce qui ne s'est pas lu de l'historique (`historique`).
fn contrats_passes(ctx: &Contexte, mois: Mois, salarie: Option<&IdSalarie>) -> R<(Vec<Passe>, Vec<String>)> {
    let le_mois = Periode::du_mois(mois);
    let voulu = |cle: &IdSalarie| salarie.is_none_or(|s| s == cle);
    let spectacles = Spectacles::nouveau(ctx);

    let mut couverts: HashSet<Mois> = HashSet::new();
    // Un contrat sur plusieurs mois est dans chacun de ses fichiers de mois, avec les memes bornes : on le garde une
    // fois, de preference tel que le fichier du mois regarde l'ecrit (c'est lui qui a les jours joues).
    let mut vus: HashMap<(IdSalarie, Periode), usize> = HashMap::new();
    let mut out: Vec<Passe> = Vec::new();
    let mut connus = engagement::stock::mois_saisis(&ctx.racine)?;
    connus.reverse();
    for &mc in &connus {
        couverts.insert(mc);
        // Les jours de chaque contrat sont ceux de son engagement (`Engagement::jours`) : une seule regle.
        let mut du_mois: Vec<(IdSalarie, SaisieCarte, Jours)> = Vec::new();
        if let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, mc)? {
            let engagements = engagement::engagements_de_la_saisie(ctx, &data)?;
            for (e, eng) in data.engagements.into_iter().zip(engagements) {
                if let Some(cle) = e.salarie.clone() {
                    du_mois.push((cle, e, eng.jours));
                }
            }
        }
        for (cle, e, jours) in du_mois {
            let Some(contrat) = e.contrat()? else { continue };
            // Un contrat sans jour finit le jour ou il commence.
            let periode = contrat.fermee_au(contrat.debut())?;
            if periode.croise(&le_mois).is_none() {
                continue;
            }
            if !voulu(&cle) {
                continue;
            }
            let trouve = e.spectacle.as_ref().and_then(|r| spectacles.trouver(r));
            let spec = trouve.map(|(_, s)| s);
            let numero_objet = spec.and_then(|s| s.numero_objet.dite()).cloned();
            // Le titre d'une fiche sans titre se dit par son numero d'objet, sinon par le spectacle que l'engagement nomme.
            let titre = match &numero_objet {
                Some(n) => titre_de(spec, Some(&n.to_string())),
                None => titre_de(spec, e.spectacle.as_deref()),
            };
            let cachets = cachets_de(ctx, &e)?;
            let dates = jours_joues(&e);
            let ident = (cle.clone(), periode);
            let deja = vus.get(&ident).copied();
            if deja.is_some() && mc != mois {
                continue;
            }
            let contrat = Passe {
                dit: dit_du_contrat(&periode, Some(cachets), None),
                nom: ctx.nomme(&cle),
                salarie: cle.clone(),
                periode,
                cachets: Some(cachets),
                heures: None,
                jours: Some(jours),
                titre,
                numero_objet,
                spectacle: e.spectacle.clone(),
                dates: dates.into_iter().filter(|j| j.mois_civil() == mois).collect(),
                source: SourceDuContratPasse::Paie,
            };
            match deja {
                Some(i) => out[i] = contrat,
                None => {
                    vus.insert(ident, out.len());
                    out.push(contrat);
                }
            }
        }
    }
    let (attestes, non_lus) = historique(ctx)?;
    for a in attestes {
        if a.periode.croise(&le_mois).is_none() {
            continue;
        }
        // Un mois produit ici se tait dans l'historique : ce sont les memes contrats, declares deux fois pendant le
        // double run.
        if couverts.contains(&a.periode.fin().mois_civil()) || !voulu(&a.salarie) {
            continue;
        }
        let trouve = a.numero_objet.as_ref().and_then(|n| spectacles.par_objet.get(n).copied());
        let spec = trouve.map(|(_, s)| s);
        out.push(Passe {
            dit: dit_du_contrat(&a.periode, a.cachets, a.heures),
            nom: ctx.nomme(&a.salarie),
            titre: titre_de(spec, a.numero_objet.as_deref()),
            spectacle: trouve.map(|(k, _)| k.clone()),
            salarie: a.salarie,
            periode: a.periode,
            cachets: a.cachets,
            heures: a.heures,
            jours: a.jours,
            numero_objet: a.numero_objet,
            dates: Vec::new(),
            source: SourceDuContratPasse::Historique,
        });
    }
    out.sort_by(|a, b| (&a.salarie, &a.periode).cmp(&(&b.salarie, &b.periode)));
    Ok((out, non_lus))
}

fn contrats_passes_route(ctx: &Contexte, c: ChargeContratsPasses) -> R<ContratsPasses> {
    let (contrats, non_lus) = contrats_passes(ctx, c.mois, c.salarie.as_ref())?;
    Ok(ContratsPasses {
        phrase: socle::phrase!("Contrats déclarés dans ce mois importé : {}.", contrats.len()),
        mois: c.mois,
        contrats,
        non_lus,
        // Ce que la liste dit quand elle est vide : rien de declare pour ce salarie, ou aucun salarie choisi.
        rien_dit: if c.salarie.is_some() { "aucun contrat déclaré" } else { "aucun salarié coché" },
    })
}

// ---------------------------------------------------------------- rubriques cochees


/// `refuser_si_clos` : un exercice cloture ne bouge plus.
pub(crate) fn refuser_si_clos(ctx: &Contexte, mois: Mois) -> R<()> {
    let annee = mois.annee();
    if let Some(clos) = crate::rappels::exercice_clos(ctx, annee)? {
        let le = clos.le.jour_de_paris().iso();
        return Err(refus(format!(
            "L'exercice {} est cloture depuis le {} : {} ne se modifie plus. Rouvrir l'exercice (Historique > L'exercice) \
             avant de toucher a ce mois.",
            annee, le, mois
        )));
    }
    Ok(())
}

/// UN MOIS QUI PORTE DES PAIES IMPORTEES NE RECOIT PAS D'ENGAGEMENT NEUF : ses paies ont ete calculees et declarees par
/// l'ancien logiciel (`engagement::importe`), et sa DSN avec elles ; un engagement de plus la changerait, et une
/// declaration deja faite ne se regenere pas (regle 4). Ce mois se finit dans l'ancien logiciel. Rien ne passe outre.
/// Rend la phrase qui le dit (`None` : le mois recoit des engagements) ; la route `saisie` la rend a la page, les
/// ecritures et l'apercu d'une carte neuve refusent avec elle.
fn ferme_aux_neufs(ctx: &Contexte, mois: Mois) -> R<Option<String>> {
    let importes = engagement::importe::du_mois(&ctx.racine, mois)?;
    Ok(importes.iter().any(|i| i.paies().contains_key(&mois)).then(|| {
        format!(
            "Le mois de {} porte des paies importées de votre ancien logiciel : il ne reçoit pas d'engagement neuf. Finissez ce mois \
             dans l'ancien logiciel.",
            mois.libelle(Graphie::Accentuee)
        )
    }))
}

fn refuser_si_importe(ctx: &Contexte, mois: Mois) -> R<()> {
    match ferme_aux_neufs(ctx, mois)? {
        Some(phrase) => Err(refus(format!("{} Rien n'est écrit.", phrase))),
        None => Ok(()),
    }
}

/// UN MOIS D'AVANT LA PREMIERE EMBAUCHE NE SE SAISIT PAS : la DSN est due par l'employeur pour les salaries qu'il
/// emploie, et le premier mois du est celui de la premiere embauche (`premiere_embauche` de la fiche Entreprise, la
/// borne que l'echeancier, les rappels et le panneau lisent : `compagnie::etat_dsn::premier_mois_du`), une fois l'echeance
/// de sa DSN passee (`echeance_dsn`). Avant elle, une compagnie qui commence saisit ce mois, et sa DSN se depose a temps.
/// Un mois qui a deja des engagements se modifie.
pub(crate) fn refuser_si_avant_la_premiere_embauche(ctx: &Contexte, mois: Mois) -> R<()> {
    // Sans premiere embauche dite, aucun mois ne la precede.
    let Some(embauche) = ctx.entreprise.premiere_embauche.dite() else { return Ok(()) };
    if mois < Mois::du_jour(&embauche)
        && declarations::echeance_dsn::de_la_compagnie(ctx, mois)?.passee_au(&ctx.aujourdhui)
        && !engagement::stock::a_une_saisie(&ctx.racine, mois)?
    {
        return Err(refus(format!(
            "{} precede la premiere embauche de la compagnie ({}) : aucune DSN n'y est due, rien ne s'y saisit. Si la \
             compagnie employait deja, corriger la date de premiere embauche (Configuration › Entreprise). Rien n'est ecrit.",
            mois,
            embauche.jj_mm_aaaa()
        )));
    }
    Ok(())
}


/// LES CHANGEMENTS D'UNE CARTE, ECRITS : le brouillon de la carte visee (neuf, ou celui que son engagement ecrit pour ce
/// mois, `engagement::stock::Objet::vue`) prend chaque changement dans l'ordre, tout ou rien (`Changements::appliquer`),
/// puis s'ecrit entier (`ecrire_une_carte`). Rend l'identite de l'engagement ecrit, le brouillon tel qu'il est ecrit, et
/// ce qui lui manque pour etre valide.
/// LES METIERS QU'UNE CARTE CHOISIT : ceux de la compagnie (Configuration › Métiers, `metiers::metiers`), la liste que
/// la route etat rend (`choix.metier`).
pub(crate) fn metiers_de_la_compagnie(ctx: &Contexte) -> R<Vec<emplois::CodeEmploi>> {
    Ok(crate::metiers::metiers(ctx)?.into_iter().map(|m| m.code).collect())
}

/// LE METIER QU'UNE CARTE DE CDDU ECRIT QUAND ELLE NOMME LE SALARIE `s` (`engagement::carte::Brouillon::preremplir_le_metier`) :
/// celui de son dernier contrat (`engagement::carte::metier_du_dernier_contrat`, sans l'engagement `id` de la carte,
/// deja ecrit) ; seulement un metier de la compagnie (`metiers` : ceux que la carte offre au choix). Sans dernier
/// contrat qui en dise un, RIEN NE S'ECRIT : la paie applique l'emploi de la fiche du salarie, tel que la fiche le dit
/// (ses statuts, sa PCS-ESE), et la case le montre (`Prerempli::metier`). Ecrire a sa place le metier du catalogue que
/// nomme son code changerait la paie : le rejeu des fictives le montre. L'erreur dit pourquoi rien ne se pre-remplit :
/// elle ne refuse rien.
fn metier_a_preremplir(ctx: &Contexte, mois: Mois, id: Option<&Identite>, s: &IdSalarie, metiers: &[emplois::CodeEmploi]) -> R<Option<emplois::CodeEmploi>> {
    let ecrit = match id {
        Some(i) => Some(engagement::stock::designe(&ctx.racine, mois, i)?.1),
        None => None,
    };
    let offert = |m: &emplois::CodeEmploi| metiers.contains(m);
    Ok(engagement::carte::metier_du_dernier_contrat(&ctx.racine, s, ecrit.as_ref())?.filter(offert))
}

fn enregistrer_saisie(ctx: &Contexte, c: ChargeCase) -> R<CarteEcrite> {
    let mois = c.mois;
    let _verrou = avant_d_ecrire(ctx, mois)?;
    let (mut brouillon, id) = match c.carte {
        CarteVisee::Neuve(n) => (Brouillon::neuf(n), None),
        CarteVisee::Ecrite(id) => (brouillon_ecrit(ctx, mois, &id)?, Some(id)),
    };
    let nomme = c.changements.nomment_le_salarie();
    let metiers = metiers_de_la_compagnie(ctx)?;
    c.changements.appliquer(ctx, mois, &mut brouillon, &metiers, |b| qui_de_la_carte(ctx, &b.en_carte())).map_err(|m| m.reclasser(Classe::Refuse))?;
    // Une carte qui nomme son salarie ecrit son metier, si elle n'en dit pas (`metier_a_preremplir`). Un pre-remplissage
    // ne refuse jamais l'ecriture d'une carte : s'il ne se lit pas, la carte s'ecrit sans metier, et sa carte rendue dit
    // pourquoi (`faute`, `carte_lue`).
    if nomme {
        if let Some(s) = brouillon.salarie_sans_metier().cloned() {
            if let Ok(m) = metier_a_preremplir(ctx, mois, id.as_ref(), &s, &metiers) {
                brouillon.preremplir_le_metier(m);
            }
        }
    }
    let id = ecrire_une_carte(ctx, mois, id.as_ref(), &brouillon.en_carte())?;
    let brouillon = brouillon_ecrit(ctx, mois, &id)?;
    let (k, _) = engagement::stock::designe(&ctx.racine, mois, &id)?;
    let vue = engagement::stock::vue_du_mois(&ctx.racine, mois)?.ok_or_else(|| refus(format!("{} : la carte écrite ne s'y lit pas.", mois.libelle(Graphie::Accentuee))))?;
    let a_completer = manque_de(ctx, &vue, k).map(|m| m.phrase);
    // « Enregistré à HH:MM » : l'heure du moteur (`temps.rs`), pas celle du navigateur (au banc, le temps est simule).
    let a = socle::temps::secondes().a_la_seconde();
    Ok(CarteEcrite {
        phrase: match &a_completer {
            Some(m) => socle::phrase!("Carte enregistrée ; à compléter avant de valider : {}", m),
            None => Phrase::fixe("Carte enregistrée."),
        },
        engagement: id,
        brouillon,
        a_completer,
        enregistre_dit: format!("Enregistré à {}", socle::paris::ecrit::heure_minute(a, ":")),
        enregistre_a: a,
    })
}

/// LE BROUILLON QU'ECRIT L'ENGAGEMENT `id` de l'ecran de `mois` : sa vue de ce mois (`engagement::stock::Objet::vue`),
/// lue en brouillon.
pub(crate) fn brouillon_ecrit(ctx: &Contexte, mois: Mois, id: &Identite) -> R<Brouillon> {
    Ok(brouillon_et_qui(ctx, mois, id)?.0)
}

/// Le brouillon qu'ecrit l'engagement `id` de l'ecran de `mois` (`brouillon_ecrit`), et son salarie tel qu'un refus le
/// nomme.
fn brouillon_et_qui(ctx: &Contexte, mois: Mois, id: &Identite) -> R<(Brouillon, String)> {
    let (_, o) = engagement::stock::designe(&ctx.racine, mois, id).map_err(|m| m.reclasser(Classe::Refuse))?;
    let qui = engagement::etats::qui_de(ctx, &o);
    let vue = o.vue(mois)?.ok_or_else(|| refus(format!("{} : l'écran ne montre pas cet engagement ; rechargez le mois. Rien n'est écrit.", qui)))?;
    let b = Brouillon::de_la_carte(&vue, &qui).map_err(|m| m.reclasser(Classe::Refuse))?;
    Ok((b, qui))
}

/// `copier-engagement` : la carte proposee a un autre salarie, copiee de l'engagement `c.engagement`
/// (`Brouillon::proposee_a_un_autre_salarie`) ; rien n'est ecrit. La carte montree est faite des changements que son
/// geste ecrira, appliques a une carte neuve par le seul chemin d'une carte (`Changements::appliquer`) : ce qui s'ecrit
/// est ce qu'elle montrait, et un refus nomme la carte neuve (`qui_de_la_carte`).
fn copier_engagement(ctx: &Contexte, c: ChargeCarteProposee) -> R<CarteProposee> {
    let mois = c.mois;
    let (source, qui) = brouillon_et_qui(ctx, mois, &c.engagement)?;
    let n = source.nature();
    let changements = source.proposee_a_un_autre_salarie();
    let mut brouillon = Brouillon::neuf(n);
    let metiers = metiers_de_la_compagnie(ctx)?;
    changements.clone().appliquer(ctx, mois, &mut brouillon, &metiers, |b| qui_de_la_carte(ctx, &b.en_carte())).map_err(|m| m.reclasser(Classe::Refuse))?;
    Ok(CarteProposee {
        phrase: socle::phrase!("Copie de l'engagement de {}, sans son salarié : choisissez-le. La carte s'écrit à sa première case changée.", qui),
        brouillon,
        enregistrer: crate::etapes::action(
            Route::EnregistrerSaisie,
            ChargeCase { mois, carte: CarteVisee::Neuve(n), changements },
            None,
            "Enregistrer".into(),
        ),
    })
}

/// UNE ECRITURE A LA FOIS, celle d'un engagement (`avant_d_ecrire`) ou des metiers de la compagnie (`metiers`) : la
/// fenetre sert ses routes en parallele, et deux ecritures qui relisent puis recrivent le meme fichier en perdraient
/// une. Le verrou rendu se garde jusqu'a la fin de l'ecriture.
pub(crate) fn une_ecriture_a_la_fois() -> std::sync::MutexGuard<'static, ()> {
    static ECRITURE_EN_COURS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // Une ecriture tombee en tenant le verrou ne le garde pas : il se reprend.
    match ECRITURE_EN_COURS.lock() {
        Ok(v) => v,
        Err(tombe) => tombe.into_inner(),
    }
}

/// AVANT TOUTE ECRITURE D'UN ENGAGEMENT (saisir, valider, modifier, retirer) : une ecriture a la fois (la fenetre sert
/// ses routes en parallele, et la page enregistre une seconde apres la derniere frappe, meme si l'enregistrement d'avant
/// n'a pas fini), puis les trois refus du mois : des paies importees (`refuser_si_importe`), un exercice clos
/// (`refuser_si_clos`), un mois d'avant la premiere embauche (`refuser_si_avant_la_premiere_embauche`). Le verrou rendu
/// se garde jusqu'a la fin de l'ecriture.
pub(crate) fn avant_d_ecrire(ctx: &Contexte, mois: Mois) -> R<std::sync::MutexGuard<'static, ()>> {
    let verrou = une_ecriture_a_la_fois();
    refuser_si_importe(ctx, mois)?;
    refuser_si_clos(ctx, mois)?;
    refuser_si_avant_la_premiere_embauche(ctx, mois)?;
    Ok(verrou)
}

/// LA CARTE D'UN ENGAGEMENT EN SAISIE, ECRITE (`engagement::stock::ecrire_la_carte`) : `saisie`, et `id`, son identite
/// (absente : un engagement neuf). Un mois sans fichier le prend avec ce qu'il dit par defaut (`squelette` : le cachet et
/// la date de paiement du mois). Une carte a laquelle il manque un fait s'ecrit aussi : seule la validation refuse. Rend
/// l'identite de l'engagement ecrit. L'appelant tient le verrou (`avant_d_ecrire`).
pub(crate) fn ecrire_une_carte(ctx: &Contexte, lu: Mois, id: Option<&Identite>, saisie: &SaisieCarte) -> R<Identite> {
    // UN ENGAGEMENT NEUF, OU LE BROUILLON QUI DEMENAGE, S'ECRIT DANS SON PREMIER MOIS (`engagement::stock::ou_s_ecrit`) : ce
    // mois-la passe les memes refus que celui de la carte, et donne ce qu'il dit par defaut.
    let fichier = engagement::stock::ou_s_ecrit(&ctx.racine, lu, id, saisie).map_err(|m| m.reclasser(Classe::Refuse))?;
    let ecrit_dans = match id {
        Some(id) => Some(engagement::stock::designe(&ctx.racine, lu, id).map_err(|m| m.reclasser(Classe::Refuse))?.1.fichier),
        None => None,
    };
    if fichier != lu && ecrit_dans != Some(fichier) {
        refuser_si_importe(ctx, fichier)?;
        refuser_si_clos(ctx, fichier)?;
    }
    let id = engagement::stock::ecrire_la_carte(ctx, lu, id, saisie, &squelette(ctx, fichier)?).map_err(|m| m.reclasser(Classe::Refuse))?;
    // UN MOIS FAIT EN DEMO, retravaille dans la compagnie gardee : il cesse d'etre « de demo » (`suppression::sortir_de_la_demo`).
    crate::accueil::suppression::sortir_de_la_demo(&ctx.racine, lu).map_err(|m| m.reclasser(Classe::Disque))?;
    Ok(id)
}

/// CE QUI MANQUE A CHAQUE ENGAGEMENT DE LA SAISIE `data` pour se calculer, dans son ordre (`None` : rien). Une saisie
/// qui se calcule entiere n'a rien qui manque. Sinon, chaque engagement se calcule a la suite de ceux d'avant qui se
/// calculent (le calcul d'un engagement lit ceux qui le precedent) : celui qui refuse porte le refus, et les suivants se
/// calculent sans lui. Un engagement valide ne manque de rien, sa validation l'a calcule, sauf les indemnites journalieres
/// d'un arret venu apres elle (`engagement::maintien::indemnites_a_dire`).
pub(crate) fn manques_de_la_saisie(ctx: &Contexte, data: &VueDuMois) -> Vec<Option<Echec>> {
    let tous = &data.engagements;
    let mois = data.mois;
    let a_dire = |e: &SaisieCarte| -> Option<Echec> {
        if !e.est_validee() {
            return None;
        }
        // Ce qui empeche de les lire manque aussi a l'engagement.
        match engagement::maintien::indemnites_a_dire(ctx, e, mois) {
            Ok(manque) => manque.map(socle::echec::refus),
            Err(m) => Some(m),
        }
    };
    let valides: Vec<Option<Echec>> = tous.iter().map(a_dire).collect();
    // Le refus de la saisie entiere ; la suite le refait engagement par engagement, et chacun dit le sien a son rang.
    let refus_d_ensemble = || -> Option<Echec> {
        match engagement::pour_l_ecran(ctx, data) {
            Ok(_) => None,
            Err(m) => Some(m),
        }
    };
    if valides.iter().all(Option::is_none) && (tous.iter().all(SaisieCarte::est_validee) || refus_d_ensemble().is_none()) {
        return vec![None; tous.len()];
    }
    let mut gardes: Vec<SaisieCarte> = Vec::new();
    let mut manques = Vec::new();
    for (e, manque) in tous.iter().zip(valides) {
        if e.est_validee() {
            if manque.is_none() {
                gardes.push(e.clone());
            }
            manques.push(manque);
            continue;
        }
        let essai = VueDuMois { engagements: gardes.iter().cloned().chain(std::iter::once(e.clone())).collect(), ..data.clone() };
        match engagement::pour_l_ecran(ctx, &essai) {
            Ok(_) => {
                gardes.push(e.clone());
                manques.push(None);
            }
            Err(m) => manques.push(Some(m)),
        }
    }
    manques
}

/// CE QUI MANQUE A L'ENGAGEMENT `i` DE LA SAISIE `data` (`manques_de_la_saisie`), sur ce que son calcul lit : les
/// engagements de son salarie (une paie ne lit que les siens, `engagement::etats`) et ceux qui portent son numero de
/// contrat (un numero est celui d'un seul salarie). Les autres salaries du mois ne se calculent pas pour lui.
pub(crate) fn manque_de(ctx: &Contexte, data: &VueDuMois, i: RangDansLeMois) -> Option<Echec> {
    let e = i.dans(&data.engagements)?;
    let cle = &e.salarie;
    let numero = e.numero.as_ref();
    let mut rang = 0;
    let mut siens = Vec::new();
    for (j, x) in RangDansLeMois::de_chacun(&data.engagements) {
        let meme_salarie = cle.is_some() && x.salarie == *cle;
        let meme_numero = numero.is_some() && x.numero.as_ref() == numero;
        if j == i {
            rang = siens.len();
        }
        if j == i || meme_salarie || meme_numero {
            siens.push(x.clone());
        }
    }
    manques_de_la_saisie(ctx, &VueDuMois { engagements: siens, ..data.clone() }).swap_remove(rang)
}


// ------------------------------------------------------------------ le montant vise


/// Ce qui a change, de l'engagement `avant` a l'engagement `apres` (la saisie que la recherche a ajustee).
fn ce_qui_a_change(avant: &SaisieCarte, apres: &SaisieCarte, index: RangDansLeMois) -> CeQuiChange {
    let mut lignes = Vec::new();
    for (rang, (a, b)) in engagement::ligne::RangDeLigne::de_chacun(avant.lignes().iter().zip(apres.lignes().iter())) {
        let par_unite = |l: &LignePaie| l.taux().and_then(engagement::ligne::TauxDeLigne::par_unite);
        let (ta, tb) = (par_unite(a), par_unite(b));
        if ta.is_none() && tb.is_none_or(|t| t.est_nul()) {
            continue; // restee sans taux : rien a ecrire
        }
        if ta != tb {
            lignes.push(TauxQuiChange { rang, rubrique: b.rubrique().clone(), taux_avant: ta, taux: tb });
        }
    }
    let change = avant.salaire_mensuel != apres.salaire_mensuel;
    CeQuiChange { index, salaire_mensuel: change.then_some(apres.salaire_mensuel), lignes }
}

/// Le salaire du mois qui change, ecrit en nombre (`null` : il n'en dit plus).
fn salaire_qui_change<S: serde::Serializer>(s: &Option<Option<MontantUnitaire>>, ser: S) -> Result<S::Ok, S::Error> {
    match s {
        Some(x) => socle::grandeurs::ecrit::nombre_json::option::serialize(x, ser),
        None => ser.serialize_none(),
    }
}

/// Ce que la phrase du montant vise dit de la cible.
fn cible_dite(c: Cible) -> &'static str {
    match c {
        Cible::Net => "Net salarié, indemnités comprises",
        Cible::NetSansFrais => "Net salarié, sans les indemnités",
        Cible::Brut => "Brut",
        Cible::Cout => "Coût entreprise, sans les indemnités",
        Cible::CoutAvecFrais => "Coût entreprise, indemnités comprises",
    }
}

/// Le taux qu'il faudrait, pour que l'engagement `engagement` (son identite ; son brouillon enregistre, ou sa modification
/// pas encore ecrite, `mois_designe`) coute CE montant. N'ecrit rien.
fn ajuster(ctx: &Contexte, c: ChargeAjuster) -> R<Ajuste> {
    let vise = socle::grandeurs::courte(c.montant).au_centime();
    if vise <= Montant::ZERO {
        return Err(refus(format!("Un montant vise se dit en euros et il est positif (recu : {}).", socle::grandeurs::ecrit::texte::dit(vise))));
    }
    let texte = c.mois.libelle(Graphie::Accentuee);
    let (data, ids) = mois_designe(ctx, c.mois, c.modifications)?;
    let i = RangDansLeMois::du_premier(&ids, |x| *x == c.engagement).ok_or_else(|| {
        refus(format!("{} : l'écran ne montre pas cet engagement ; rechargez le mois.", texte))
    })?;
    let v = engagement::viser::viser(ctx, &data, &[i], c.cible, vise).map_err(|r| match r {
        engagement::viser::RefusVisee::Refus(m) => m,
        engagement::viser::RefusVisee::Calcul(m) => refus_du_calcul(ctx, &data, m),
    })?;
    // Le calcul rend une paie par engagement de la saisie visee, dans son ordre.
    let apres_et_paie = RangDansLeMois::de_chacun(&v.saisie.engagements).zip(&v.paies).find(|((j, _), _)| *j == i).map(|((_, e), p)| (e, p));
    let (avant, apres, paie) = match (i.dans(&data.engagements), apres_et_paie) {
        (Some(a), Some((b, p))) => (a, b, p),
        _ => return Err(socle::refus!("{} : l'engagement visé n'a pas de paie calculée.", texte)),
    };
    let vue = vue_paie(ctx, paie, apres, &v.saisie, &lignes_preremplies(ctx, &v.saisie))?;
    let atteint = v.atteint.au_centime();
    let change = ce_qui_a_change(avant, apres, i);
    // Le taux qu'il faudrait : chaque ligne qui change, puis le salaire du mois s'il change.
    let mut faut: Vec<String> = change
        .lignes
        .iter()
        .map(|l| {
            let taux = match l.taux {
                Some(t) => format!("{} à {}", l.rubrique, socle::texte::euros(t)),
                None => l.rubrique.to_string(),
            };
            match l.taux_avant {
                Some(a) => format!("{} au lieu de {}", taux, socle::texte::euros(a)),
                None => taux,
            }
        })
        .collect();
    if let Some(Some(s)) = &change.salaire_mensuel {
        faut.push(format!("salaire du mois à {}", socle::texte::euros(*s)));
    }
    let avec = if faut.is_empty() { String::new() } else { format!(" ; {}", faut.join(", ")) };
    let ecart = atteint - vise;
    let pres = if ecart.est_nul() {
        " au centime près".to_string()
    } else {
        format!(" ({}{} : un taux se paie au centime)", if ecart > Montant::ZERO { "+" } else { "" }, socle::texte::euros(ecart))
    };
    Ok(Ajuste {
        phrase: socle::phrase!("{} : visé {} → {}{}{}.", cible_dite(c.cible), socle::texte::euros(vise), socle::texte::euros(atteint), pres, avec),
        cible: c.cible,
        vise,
        atteint,
        depart: v.depart.au_centime(),
        ecart,
        facteur: v.facteur.a_decimales(6),
        tours: v.tours,
        engagements: [change],
        paies: [vue],
    })
}

// ============================================================== l'apercu

/// Le refus du calcul d'une saisie : le moteur refuse avec sa phrase, mais
/// quand la saisie elle-meme est encore incomplete (un spectacle pas encore
/// choisi, des jours ou des heures pas encore saisis :
/// `engagement::saisie_incomplete`) c'est cette phrase-la, sous son nom
/// (`SaisieIncomplete`), qui monte. La page les
/// dit sobrement ; l'ecran enregistre a chaque frappe, donc ce ne sont pas des
/// defauts du logiciel.
pub(crate) fn refus_du_calcul(ctx: &Contexte, data: &VueDuMois, e: Echec) -> Echec {
    if engagement::saisie_incomplete(&e) {
        return e;
    }
    match engagement::refus_de_saisie_incomplete(ctx, data) {
        Some(m) => m,
        None => e,
    }
}

/// LE MOIS QUE L'APERCU ET LE MONTANT VISE CALCULENT : ses engagements tels qu'ils sont ecrits (le brouillon enregistre
/// de chaque carte en saisie, la validation de chaque engagement valide), dans l'ordre de l'ecran (`vue_du_mois`), et
/// chaque modification pas encore ecrite d'un engagement valide (`modifications`) : son brouillon (`Brouillon::de_la_carte`
/// de sa vue) prend ses changements comme `modifier-engagement` les prendra, et se calcule a sa place, en saisie. Rend
/// ce mois et l'identite de chacun de ses engagements, dans son ordre. Rien ne s'ecrit.
fn mois_designe(ctx: &Contexte, mois: Mois, modifications: Vec<ModificationEnCours>) -> R<(VueDuMois, Vec<Identite>)> {
    let texte = mois.libelle(Graphie::Accentuee);
    // Un mois qu'aucun engagement ne touche encore se calcule vide.
    let mut v = match engagement::stock::vue_du_mois(&ctx.racine, mois)? {
        Some(v) => v,
        None => VueDuMois::vide(mois),
    };
    let ids = engagement::stock::identites(&engagement::stock::objets_du_mois(&ctx.racine, mois)?);
    if ids.len() != v.engagements.len() {
        return Err(socle::refus!("{} : {} identités pour {} engagements.", texte, ids.len(), v.engagements.len()));
    }
    for m in modifications {
        let ecran = || refus(format!("{} : l'écran ne montre pas cet engagement ; rechargez le mois.", texte));
        let i = RangDansLeMois::du_premier(&ids, |x| *x == m.engagement).ok_or_else(ecran)?;
        let ecrit = i.dans(&v.engagements).ok_or_else(ecran)?.clone();
        if !ecrit.est_validee() {
            return Err(refus(format!(
                "{} : cet engagement n'est pas validé ; ses changements s'enregistrent (enregistrer-saisie), l'aperçu lit son brouillon.",
                qui_de_la_carte(ctx, &ecrit)
            )));
        }
        let mut b = Brouillon::de_la_carte(&ecrit, &qui_de_la_carte(ctx, &ecrit)).map_err(|e| e.reclasser(Classe::Refuse))?;
        m.changements.appliquer(ctx, mois, &mut b, &metiers_de_la_compagnie(ctx)?, |b| qui_de_la_carte(ctx, &b.en_carte())).map_err(|e| e.reclasser(Classe::Refuse))?;
        let saisie = engagement::stock::en_forme(&b.en_carte()).map_err(|e| e.reclasser(Classe::Refuse))?;
        // L'avenant du mois se deduit des termes dates de l'engagement ecrit (`stock::Objet::vue`) : le brouillon ne le
        // porte pas.
        *i.dans_mut(&mut v.engagements).ok_or_else(ecran)? = SaisieCarte { date_modification: ecrit.date_modification.clone(), avant: ecrit.avant.clone(), ..saisie };
    }
    Ok((v, ids))
}

/// Le salarie qu'une carte nomme, pour un refus (« Cette carte » sans lui).
fn qui_de_la_carte(ctx: &Contexte, c: &SaisieCarte) -> String {
    match &c.salarie {
        Some(s) => ctx.nomme(s),
        None => "Cette carte".to_string(),
    }
}

/// Une paie que l'ecran montre, et pourquoi elle s'est recalculee d'elle-meme (`{"le", "raison"}`), s'il y a lieu.
pub(crate) struct PaieMontree {
    pub(crate) paie: Paie,
    pub(crate) recalcul: Option<engagement::paie_ecrite::Recalcul>,
}


/// LES PAIES QUE L'ECRAN MONTRE POUR LA SAISIE `data` : un engagement valide se lit (sa paie ecrite, `etats::Valide::paie`),
/// un engagement en saisie se calcule, sans rien ecrire, comme un engagement valide d'un mois d'apres le present, qui n'a
/// pas encore de paie. Un mois tout valide et present ne calcule rien.
///
/// L'ecran rend chaque engagement valide tel que le moteur le lui a montre (`valide` pose) : il retrouve le sien, sur le
/// disque, par son salarie et son numero de contrat (ecrit a la validation).
pub(crate) fn paies_de_l_ecran(ctx: &Contexte, data: &VueDuMois, mois: Mois) -> R<Vec<PaieMontree>> {
    let disque = engagement::stock::vue_du_mois(&ctx.racine, mois)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let etats = if disque.is_some() { engagement::etats::du_mois(ctx, mois, &departs)? } else { Vec::new() };
    // TRAVAIL A FAIRE (chaine A) : l'engagement ecrit (`stock::Objet::champs`) se lit par ses cles.
    let qui = |b: &engagement::saisie::Carte| (b.salarie.clone(), b.numero.clone());
    let mut lues: Vec<Option<PaieMontree>> = Vec::new();
    for e in &data.engagements {
        let mut lue = None;
        if e.est_validee() {
            let sien = (e.salarie.clone(), e.numero.clone());
            // Un mois d'apres le present n'a pas encore de paie ecrite : l'engagement valide s'y calcule pour l'ecran.
            let apres_le_present = mois > engagement::etats::mois_present(ctx);
            let mut trouve = None;
            for (j, x) in RangDansLeMois::de_chacun(&etats) {
                let a_venir = apres_le_present && x.paie_ecrite(mois)?.is_none();
                if x.valide().is_some() && !a_venir && sien.1.is_some() && qui(x.objet().carte()) == sien {
                    trouve = Some((j, x));
                    break;
                }
            }
            if let (Some((j, x)), Some(d)) = (trouve, disque.as_ref()) {
                let v = x.valide().ok_or_else(|| socle::echec::refus("engagement en saisie"))?;
                lue = Some(PaieMontree { paie: v.paie(ctx, d, j)?, recalcul: v.recalcul(mois)? });
            }
        }
        lues.push(lue);
    }
    let calculees = if lues.iter().any(Option::is_none) {
        engagement::pour_l_ecran(ctx, data).map_err(|m| refus_du_calcul(ctx, data, m))?.paies
    } else {
        Vec::new()
    };
    let mut out = Vec::new();
    for (i, l) in lues.into_iter().enumerate() {
        match l {
            Some(l) => out.push(l),
            None => out.push(PaieMontree { paie: calculees.get(i).cloned().ok_or_else(|| socle::echec::refus("le calcul du mois n'a pas cette paie"))?, recalcul: None }),
        }
    }
    Ok(out)
}

/// LA RUPTURE d'un engagement, telle que la page la propose (« Un imprévu… ») : le libelle de la porte et la fin du
/// contrat (celle qu'il dit, ou son dernier jour travaille quand il est aux cachets) ; si cette fin se signale, au motif
/// que dit la carte (au motif par defaut sans lui ; `Nature::signalement_fctu`, avec ce que la DSN sait du contrat : un
/// CDDU a son terme part dans la mensuelle s'il est infra DSN mensuelle, il se signale s'il est rompu avant ou deja
/// declare, au calendrier : `ConnuDeLaDsn::du_contrat`) ; et le choix vide des motifs (le defaut d'une fin qui ne se
/// signale pas), que la carte offre en tete de ses motifs (`CarteRendue`). La page ne decide rien.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Rupture {
    porte: &'static str,
    fin: Option<Jour>,
    signalee: bool,
    #[serde(skip)]
    motif_vide_dit: String,
}

/// LA FIN DU CONTRAT TELLE QUE LA ROUTE LA REND (`contrat.rupture`) : aucune (`null` : elle n'a pas de sens ici), la
/// rupture lue, ou une rupture qui ne se lit pas (`illisible`, sa phrase). Celle-la n'est jamais rendue comme « pas de
/// rupture » : la carte dit aussi ce qui ne se lit pas (`CarteRendue::faute`), et la page n'ouvre aucune fin qu'elle ne
/// sait pas dire ; l'apercu des autres reste.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum FinDuContrat {
    Lue(Option<Rupture>),
    Illisible { illisible: String },
}

impl FinDuContrat {
    fn de(r: R<Option<Rupture>>) -> FinDuContrat {
        match r {
            Ok(r) => FinDuContrat::Lue(r),
            Err(e) => FinDuContrat::Illisible { illisible: e.phrase },
        }
    }
}

/// LA RUPTURE de cet engagement : `None` quand elle n'a pas de sens ici -- une nature sans motif de rupture
/// (S21.G00.62.002, CCH-11 : `Nature::motifs_rupture`), ou un contrat a terme qui ne finit pas ce mois-ci. Les motifs et
/// ce que chacun demande viennent de `natures`.
fn rupture_de(e: &engagement::Engagement, nat: &droit::natures::Nature) -> R<Option<Rupture>> {
    if nat.motifs_rupture().is_empty() {
        return Ok(None);
    }
    let connu = droit::natures::ConnuDeLaDsn::du_contrat(e.contrat().debut(), e.mois);
    // Un contrat paye au mois (CDI, CDD) dit sa fin dans sa case : elle se tape, le mois ou elle a lieu.
    if nat.duree_indeterminee || nat.mensuelle() {
        return Ok(Some(rupture_pour(nat, connu, e.contrat().fin(), e.motif_rupture)));
    }
    // Un contrat aux cachets finit son dernier jour travaille ; le bloc 62 s'ecrit ce mois-la et lui seul (fiche n° 3344,
    // `fin_de_contrat` de `contrats-base.json`) : sa rupture se dit ce mois-la.
    if let PlaceDuMois::Premier | PlaceDuMois::Milieu = e.place_du_mois()? {
        return Ok(None);
    }
    let fin = match e.contrat().fin() {
        Some(f) => f,
        None => e.periode_payee()?.fin(),
    };
    Ok(Some(rupture_pour(nat, connu, Some(fin), e.motif_rupture)))
}

/// LA RUPTURE D'UNE CARTE QUI NE SE CALCULE PAS ENCORE (`a_completer`), lue sur sa saisie `e` du mois `mois` : le bloc de
/// fin se dessine avant que la paie se calcule, pour que ce qui lui manque (une date de notification, un preavis) se tape
/// ou il se demande. La meme que `rupture_de`, depuis ce que la carte dit : sa nature, son debut lu et sa fin lue
/// (`Carte::debut_lu`, `Carte::fin_lue`). `None` sans nature ni debut (rien ne dit encore le contrat), pour une nature
/// sans motif de rupture, ou pour un contrat aux cachets qui finit apres ce mois.
fn rupture_en_saisie(e: &SaisieCarte, mois: Mois) -> R<Option<Rupture>> {
    let (Some(_), Some(debut)) = (e.nature, e.debut_lu()) else { return Ok(None) };
    let nat = e.nature_lue()?;
    if nat.motifs_rupture().is_empty() {
        return Ok(None);
    }
    let connu = droit::natures::ConnuDeLaDsn::du_contrat(debut, mois);
    if nat.duree_indeterminee || nat.mensuelle() {
        return Ok(Some(rupture_pour(&nat, connu, e.fin, e.motif_rupture)));
    }
    match e.fin_lue() {
        Some(f) if f.mois_civil() > mois => Ok(None),
        fin => Ok(Some(rupture_pour(&nat, connu, fin, e.motif_rupture))),
    }
}

/// La rupture d'un contrat de nature `nat`, que la DSN connait comme `connu`, qui finit le `fin` au motif `motif` : si
/// cette fin se signale (`Nature::signalement_fctu`), et le choix vide des motifs.
fn rupture_pour(nat: &droit::natures::Nature, connu: droit::natures::ConnuDeLaDsn, fin: Option<Jour>, motif: Option<droit::natures::MotifDeRupture>) -> Rupture {
    let porte = if nat.duree_indeterminee { "La fin du contrat (licenciement, démission…)" } else { "Une rupture avant terme" };
    let motif_vide_dit = match nat.motif_rupture_par_defaut() {
        Some(d) if !nat.signalement_fctu(None, connu) => format!("— {}, à son terme —", d),
        _ => "— à choisir —".to_string(),
    };
    Rupture { porte, fin, signalee: nat.signalement_fctu(motif, connu), motif_vide_dit }
}


/// CE QUE LE CONTRAT ECRIT DE CET ENGAGEMENT APPELLE SUR SA CARTE, les faits qui en ouvrent les cases
/// (`engagement::carte::FaitQuiOuvre::{ContratEcrit, ModelesAuChoix, NumeroADire}`) :
/// - `ecrit` : le contrat se fait signer ici (`engagement::contrat_a_signer`) ; sa carte dit sa periode d'essai et ses
///   clauses particulieres (`engagement::ContratEcrit`) ;
/// - `modeles` : la compagnie a des modeles speciaux de sa nature (`modeles_contrat::au_choix`) ;
/// - `numero` : un contrat commence avant le logiciel (`engagement::stock::commence_avant_le_logiciel`), signe ailleurs,
///   dont aucune DSN importee ne dit le numero (`engagement::importe::numero_declare_du_contrat`) : un numero ne se
///   recalcule pas, il se lit (regle 4) ; celui que l'ancien logiciel a declare (S21.G00.40.009) se dit dans sa case
///   (`engagement::numeros::a_la_validation` le garde). Vide, le logiciel en donne un.
#[derive(Debug, Clone, Copy, Default)]
struct ContratAppelle {
    ecrit: bool,
    modeles: bool,
    numero: bool,
}

/// Ce qu'appelle le contrat d'un contrat commence avant le logiciel : la case de son numero, s'il n'est pas deja lu.
fn numero_a_dire(ctx: &Contexte, salarie: &IdSalarie, debut: Jour) -> R<ContratAppelle> {
    Ok(ContratAppelle { numero: !engagement::importe::numero_declare_du_contrat(&ctx.racine, salarie, debut)?, ..ContratAppelle::default() })
}

/// `ContratAppelle` d'un engagement calcule `e`, de nature `nat`.
fn contrat_appelle(ctx: &Contexte, e: &engagement::Engagement, nat: &droit::natures::Nature) -> R<ContratAppelle> {
    if engagement::stock::commence_avant_le_logiciel(&ctx.racine, e.contrat().debut(), e.mois)? {
        return numero_a_dire(ctx, &e.salarie, e.contrat().debut());
    }
    if !engagement::contrat_a_signer(&ctx.racine, e)? {
        return Ok(ContratAppelle::default());
    }
    Ok(ContratAppelle { ecrit: true, modeles: !crate::modeles_contrat::au_choix(ctx, nat.cle)?.is_empty(), numero: false })
}

/// `ContratAppelle` d'un engagement qui ne se calcule pas encore (`a_completer`), lu sur sa saisie `e` de `mois` : la
/// periode d'essai et les clauses se disent avant que le reste soit complet. Une carte qui ne nomme pas encore son
/// salarie ou sa nature n'appelle encore rien ; un contrat repris d'un ancien logiciel (sa date de debut dite) ne se
/// refait pas.
fn contrat_appelle_en_saisie(ctx: &Contexte, e: &SaisieCarte, mois: Mois) -> R<ContratAppelle> {
    let Some(cle) = e.salarie.as_ref() else { return Ok(ContratAppelle::default()) };
    let debut = e.debut_lu();
    if let Some(d) = debut {
        if engagement::stock::commence_avant_le_logiciel(&ctx.racine, d, mois)? {
            return numero_a_dire(ctx, cle, d);
        }
    }
    let Some(n) = e.nature else { return Ok(ContratAppelle::default()) };
    if n.proprietes()?.modele_contrat().is_none() {
        return Ok(ContratAppelle::default());
    }
    if let Some(d) = debut {
        if engagement::importe::vient_d_un_ancien_logiciel(&ctx.racine, cle, d, e.numero.as_ref())? {
            return Ok(ContratAppelle::default());
        }
    }
    Ok(ContratAppelle { ecrit: true, modeles: !crate::modeles_contrat::au_choix(ctx, n)?.is_empty(), numero: false })
}

/// LA CARTE D'UN ENGAGEMENT TELLE QUE LA ROUTE LA REND : les cases ouvertes pour lui, chacune sa place
/// (`engagement::carte::CarteValide::cases_ouvertes`), ce qu'offrent celles de ses cases a choix qui dependent de lui
/// (le motif de la fin, le choix vide dit en tete ; les modeles de contrat), sous la cle de leur case, et ce qui
/// pre-remplit une case qu'il ne dit pas (`Prerempli`). Un fait qui ne se lit pas pour lui ne ferme ni sa carte ni
/// l'apercu des autres (« le logiciel est doux ») : sa carte montre ses cases de toujours et dit ce qui ne se lit pas
/// (`faute`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CarteRendue {
    cases: Vec<engagement::carte::CaseOuverte>,
    choix: BTreeMap<&'static str, Vec<crate::ouverture::Choix>>,
    prerempli: Prerempli,
    #[serde(skip_serializing_if = "Option::is_none")]
    faute: Option<String>,
}

/// CE QUE LA ROUTE POSE DANS UNE CASE QUE LA CARTE NE DIT PAS, le seul pre-remplissage d'une carte : ce que le moteur
/// prend pour elle quand elle est vide. La carte le montre, il ne s'ecrit pas tant qu'on ne le change pas ; `None` : la
/// carte le dit, ou le moteur ne le sait pas (encore).
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct Prerempli {
    /// Le jour du versement que dit la configuration (`Engagement::date_du_versement`).
    date_paiement: Option<Jour>,
    /// Le debut du contrat, que ses jours coches disent (`Engagement::contrat` ; a completer, `debut_lu`) : la porte
    /// « Imprévu… » le lit (le debut d'un contrat dont la fin se dit). La fin et les jours se lisent dans `contrat_dit`.
    debut: Option<Jour>,
    /// Le metier que la paie applique quand la carte n'en dit pas : celui que nomme le code emploi de la fiche du
    /// salarie (`droit::emplois::CodeEmploi::du_code_declare`, `engagement::lib` : sans metier sur la carte, l'emploi est
    /// celui de la fiche). Montre, il ne s'ecrit que si on le choisit.
    metier: Option<droit::emplois::CodeEmploi>,
    /// CE QU'UNE LIGNE NEUVE DE CHAQUE RUBRIQUE PREND, sous son code (`lignes_preremplies`) : une ligne de cachet, le
    /// taux du cachet du mois ou du dossier.
    lignes: LignesPreremplies,
}

/// Ce qu'une ligne neuve de chaque rubrique prend, sous son code (`Prerempli::lignes`).
pub(crate) type LignesPreremplies = BTreeMap<droit::catalogue::CodeDeRubrique, LignePreremplie>;

/// Ce qu'une ligne neuve d'une rubrique prend (`Prerempli::lignes`), sous la cle de la ligne : son taux.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct LignePreremplie {
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    taux: MontantUnitaire,
}

/// LE TAUX D'UNE LIGNE DE CACHET NEUVE, un pre-remplissage : celui du dernier cachet du mois `data`, sinon le dernier
/// taux saisi du dossier (`dernier_taux`) ; rien quand aucun cachet n'en dit (regle 1 : aucun texte ne fixe un taux de
/// cachet, la compagnie saisit le sien). Chaque rubrique de cachet du catalogue de la compagnie le prend. Un mois du
/// dossier qui ne se lit pas fait la faute de chaque carte (`CarteRendue::faute`, par `Prerempli`), pas de l'apercu.
pub(crate) fn lignes_preremplies(ctx: &Contexte, data: &VueDuMois) -> R<LignesPreremplies> {
    let taux = match taux_du_mois(ctx, data)? {
        Some(t) => t,
        None => match dernier_taux(ctx)? {
            Some(t) => t,
            None => return Ok(BTreeMap::new()),
        },
    };
    Ok(droit::catalogue::catalogue(&ctx.racine, &data.mois.premier_jour())?
        .iter()
        .filter(|r| r.unite == droit::catalogue::Unite::Cachet)
        .map(|r| (r.code.clone(), LignePreremplie { taux }))
        .collect())
}

/// LES BORNES ET LES JOURS DU CONTRAT, tels qu'ils se lisent une fois, sous les lignes de la carte (`contrat_dit`) :
/// « Du 04/12/2026 au 05/12/2026 · 2 jours travaillés ». Ce sont les cases que la carte rend sans les faire saisir
/// (`PlaceDeLaCase::Rendue` : le debut, la fin, les jours travailles), lues la et nulle part ailleurs. Un contrat sans
/// fin se dit par son debut, et `sans_terme` (un contrat paye au mois) le dit ; `jours` : ceux que la DSN declare, quand
/// la paie les connait.
fn bornes_dites(debut: Jour, fin: Option<Jour>, sans_terme: bool, jours: Option<Jours>) -> String {
    let mut t = match fin {
        Some(f) => format!("Du {} au {}", debut.jj_mm_aaaa(), f.jj_mm_aaaa()),
        None if sans_terme => format!("Du {}, sans terme", debut.jj_mm_aaaa()),
        None => format!("Du {}", debut.jj_mm_aaaa()),
    };
    if let Some(j) = jours {
        let mot = if j > Jours::de_dates(1) { "jours travaillés" } else { "jour travaillé" };
        t.push_str(&format!(" · {} {}", socle::texte::virgule(j), mot));
    }
    t
}

/// Le metier que nomme le code emploi `code` d'une fiche (`Prerempli::metier`).
fn metier_de_la_fiche(code: Option<&droit::fiche::CodeEmploiFt>) -> R<Option<droit::emplois::CodeEmploi>> {
    match code {
        Some(c) => droit::emplois::CodeEmploi::du_code_declare(c),
        None => Ok(None),
    }
}

/// CE QUE LE MOTEUR SAIT D'UN ENGAGEMENT ET QUE SA CARTE LIT (`carte_rendue`), chaque chose telle qu'elle se lit : une qui
/// ne se lit pas fait la `faute` de sa carte, et d'elle seule.
struct CeQueLeMoteurSait<'a> {
    contrat: R<ContratAppelle>,
    rupture: &'a R<Option<Rupture>>,
    /// La modalite du temps de travail d'un contrat paye au mois, qui ouvre ses heures en plus
    /// (`FaitQuiOuvre::{TempsPlein, TempsPartiel}`) ; `None` pour un autre.
    temps: R<Option<droit::natures::ModaliteDuTempsDeTravail>>,
    /// Son pre-remplissage, chaque case le sien : un qui ne se lit pas laisse sa case vide (`ou_vide`).
    prerempli: Prerempli,
    /// Pourquoi une case ne se pre-remplit pas : la `faute` de la carte le dit, et aucune case ne se ferme pour lui.
    ne_se_preremplit: Vec<String>,
}

/// UN PRE-REMPLISSAGE QUI NE SE LIT PAS LAISSE SA CASE VIDE, et rien d'autre ne bouge : sa raison va a `fautes`
/// (`CeQueLeMoteurSait::ne_se_preremplit`, que la carte dit dans sa `faute`).
fn ou_vide<T>(r: R<Option<T>>, fautes: &mut Vec<String>) -> Option<T> {
    match r {
        Ok(v) => v,
        Err(e) => {
            fautes.push(e.phrase);
            None
        }
    }
}

/// Ce qu'une ligne neuve prend, pour une carte (`ou_vide`) : rien quand il ne se lit pas, et sa raison va a `fautes`.
fn lignes_ou_vide(lignes: &R<LignesPreremplies>, fautes: &mut Vec<String>) -> LignesPreremplies {
    match lignes {
        Ok(l) => l.clone(),
        Err(e) => {
            fautes.push(e.phrase.clone());
            BTreeMap::new()
        }
    }
}

/// POURQUOI UNE CARTE DE CDDU QUI NE DIT PAS SON METIER N'A PAS RECU CELUI DU DERNIER CONTRAT DE SON SALARIE, quand il ne
/// se lit pas (`metier_a_preremplir`, qui n'ecrit alors rien et ne refuse rien) : la carte le dit dans sa `faute`.
fn metier_du_dernier_contrat_illisible(ctx: &Contexte, e: &SaisieCarte, fautes: &mut Vec<String>) {
    let (Some(NatureDeContrat::CdduSpectacle), None, Some(s)) = (e.nature, &e.metier, &e.salarie) else { return };
    if let Err(m) = engagement::carte::metier_du_dernier_contrat(&ctx.racine, s, None) {
        fautes.push(format!("Le métier du dernier contrat ne se lit pas, la carte ne le reprend pas : {}", m.phrase));
    }
}

/// LA CARTE RENDUE DE LA SAISIE `e` (`CarteRendue`). Ses cases se lisent des faits ; un fait qui ne se lit pas ne laisse
/// que les cases de toute carte, et le dit (`faute`), avec ce qui ne se pre-remplit pas.
fn carte_rendue(ctx: &Contexte, e: &SaisieCarte, sait: CeQueLeMoteurSait) -> CarteRendue {
    let Some(n) = e.nature else {
        return CarteRendue { cases: Vec::new(), choix: BTreeMap::new(), prerempli: Prerempli::default(), faute: None };
    };
    let CeQueLeMoteurSait { contrat, rupture, temps, prerempli, ne_se_preremplit } = sait;
    let dite = |avant: Option<String>| -> Option<String> {
        let toutes: Vec<String> = avant.into_iter().chain(ne_se_preremplit.iter().cloned()).collect();
        (!toutes.is_empty()).then(|| toutes.join(" "))
    };
    match carte_lue(ctx, e, n, contrat, rupture, temps) {
        Ok((cases, choix)) => CarteRendue { cases, choix, prerempli, faute: dite(None) },
        Err(faute) => CarteRendue {
            cases: engagement::carte::CarteValide::cases_toujours_ouvertes(n),
            choix: BTreeMap::new(),
            prerempli,
            faute: dite(Some(faute.phrase)),
        },
    }
}

/// Les choix des cases d'une carte qui dependent d'elle, sous la cle de leur case.
type ChoixDeLaCarte = BTreeMap<&'static str, Vec<crate::ouverture::Choix>>;

/// Les cases ouvertes de la carte `e` de nature `n`, lues des faits que le moteur sait d'elle
/// (`engagement::carte::FaitQuiOuvre`) : la configuration (ses etablissements), ce que son contrat ecrit appelle, sa fin
/// ce mois (`rupture`), son motif et son preavis, sa duree du travail, son metier ecrit, son origine (une paie
/// importee) ; et les choix qui dependent d'elle.
fn carte_lue(
    ctx: &Contexte,
    e: &SaisieCarte,
    n: droit::natures::NatureDeContrat,
    contrat: R<ContratAppelle>,
    rupture: &R<Option<Rupture>>,
    temps: R<Option<droit::natures::ModaliteDuTempsDeTravail>>,
) -> R<(Vec<engagement::carte::CaseOuverte>, ChoixDeLaCarte)> {
    use engagement::carte::FaitQuiOuvre as F;
    let nat = n.proprietes()?;
    let rupture = rupture.as_ref().map_err(Clone::clone)?.as_ref();
    let contrat = contrat?;
    let temps = temps?;
    let fin_ce_mois = rupture.is_some();
    let faits = droit::permanents::faits_de_rupture(&nat, e.motif_rupture)?;
    let plusieurs = crate::ouverture::choix_d_etablissement(ctx)?.is_some();
    let non_effectue = match &e.preavis {
        Some(p) => declarations::fctu::preavis_non_effectue(p.genre)?,
        None => false,
    };
    // Le temps partiel (modalite 20) dit des heures complementaires (L. 3123-20) ; les autres modalites, des heures
    // supplementaires, comme le calcul les lit (`engagement::heures_en_plus::lignes`).
    use droit::natures::ModaliteDuTempsDeTravail::TempsPartiel;
    let plein = temps.is_some_and(|m| m != TempsPartiel);
    let partiel = temps == Some(TempsPartiel);
    let par_semaine = plein && engagement::heures_en_plus::decomptees_par_semaine(ctx)?;
    let vrai = |f: F| -> R<bool> {
        Ok(match f {
            F::PlusieursEtablissements => plusieurs,
            F::ContratEcrit => contrat.ecrit,
            F::ModelesAuChoix => contrat.modeles,
            F::NumeroADire => contrat.numero,
            F::FinCeMois => fin_ce_mois,
            F::MotifAvantTerme => fin_ce_mois && e.motif_rupture.is_some_and(|m| Some(m) != nat.motif_rupture_par_defaut()),
            F::DuMotif(fait) => fin_ce_mois && faits.contains(&fait),
            F::PreavisNonEffectue => fin_ce_mois && non_effectue,
            F::Importee => e.importe == Some(true),
            F::TempsPlein => plein,
            F::HeuresDecompteesParSemaine => par_semaine,
            F::TempsPartiel => partiel,
        })
    };
    let cases = engagement::carte::CarteValide::cases_ouvertes(n, &vrai)?;
    let ouverte = |c: engagement::saisie::Champ| cases.iter().any(|x| x.cle == c);
    let mut choix = BTreeMap::new();
    if let (true, Some(r)) = (ouverte(engagement::saisie::Champ::MotifRupture), rupture) {
        let vide = crate::ouverture::Choix::de("", &r.motif_vide_dit);
        let motifs = nat.motifs_rupture().iter().map(|m| crate::ouverture::Choix::de(m.code, &m.libelle).disant(m.dit.clone()));
        choix.insert(engagement::saisie::Champ::MotifRupture.nom(), std::iter::once(vide).chain(motifs).collect());
    }
    if ouverte(engagement::saisie::Champ::ModeleContrat) {
        choix.insert(engagement::saisie::Champ::ModeleContrat.nom(), crate::modeles_contrat::au_choix(ctx, n)?.iter().map(|m| m.en_choix()).collect());
    }
    Ok((cases, choix))
}

/// Le salarie d'une paie a l'ecran : sa cle, son nom, son prenom (celui d'usage s'il en a un).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SalarieVu {
    cle: IdSalarie,
    nom: Option<String>,
    prenom: Option<String>,
}

/// Le spectacle d'une paie : son titre et son numero d'objet, s'il est dit.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SpectacleVu {
    titre: String,
    numero_objet: Option<droit::fiche::NumeroObjetSpectacle>,
}

/// La nature du contrat : sa cle, son libelle, et s'il se paie au mois.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct NatureVue {
    cle: NatureDeContrat,
    libelle: String,
    mensuelle: bool,
}

/// LES CHEVRONS D'UN CONTRAT SUR PLUSIEURS MOIS, tels qu'on les lit au survol, de chaque cote ou il deborde du mois.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Chevrons {
    avant: Option<String>,
    apres: Option<String>,
}

/// Le contrat par rapport au mois : un contrat qui deborde n'a pas de DPAE ni de contrat imprime hors de son premier mois.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ContratVu {
    place: PlaceDuMois,
    chevrons: Chevrons,
    sans_paie: bool,
    depuis: Mois,
    jusqu_a: Mois,
    /// La periode de paie du contrat dans le mois.
    periode: socle::date::Periode,
    rupture: FinDuContrat,
}

/// Une case qu'une ligne de frais appelle : le rang de la ligne dans la saisie, la case, ce qu'elle dit, son aide.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CaseDeFrais {
    ligne: engagement::ligne::RangDeLigne,
    case: engagement::CaseDeFrais,
    libelle: &'static str,
    aide: Option<String>,
}

/// Une ligne de la remuneration, telle que la paie la calcule.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct LigneVue {
    rubrique: droit::catalogue::CodeDeRubrique,
    designation: String,
    /// Sa quantite (la base d'une ligne en pourcentage) et son taux, dans l'unite de sa rubrique, en texte.
    #[serde(serialize_with = "engagement::ligne::en_texte::quantite")]
    quantite: engagement::ligne::PaiementDeLigne,
    #[serde(serialize_with = "engagement::ligne::en_texte::taux")]
    taux: engagement::ligne::TauxDeLigne,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    montant: Montant,
}

/// L'emploi du salarie, tel que sa fiche le dit.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EmploiVu {
    libelle: Option<String>,
    code_emploi_ft: Option<droit::fiche::CodeEmploiFt>,
    statut: Option<droit::fiche::Statut>,
    cadre: bool,
}

/// UN AVIS DE LA PAIE TEL QUE LA CARTE LE MONTRE (`engagement::Avis`) : sa phrase, en mots, et a cote d'elle le lien vers
/// le texte qu'elle renvoie a verifier, quand elle en a un.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct AvisVu {
    texte: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lien: Option<LienDuTexte>,
}

impl AvisVu {
    /// Une phrase que la vue ajoute, sans lien.
    fn phrase(texte: String) -> AvisVu {
        AvisVu { texte, lien: None }
    }

    fn de(a: &engagement::Avis) -> AvisVu {
        AvisVu { texte: a.texte.clone(), lien: a.convention.clone().map(LienDuTexte::de_la_convention) }
    }
}

/// UN LIEN VERS LE TEXTE QU'UNE PHRASE NOMME, rendu a cote d'elle et jamais dedans (`affiche::defaut`) : son libelle,
/// et l'adresse que la page ouvre dans le navigateur.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct LienDuTexte {
    libelle: &'static str,
    adresse: socle::garde_reseau::Url,
}

impl LienDuTexte {
    /// Le texte d'une convention collective (`engagement::Avis::convention`).
    fn de_la_convention(adresse: socle::garde_reseau::Url) -> LienDuTexte {
        LienDuTexte { libelle: const { Affiche::new("Voir la convention") }.texte(), adresse }
    }
}

/// UNE PAIE POUR L'ECRAN (`apercu`, `ajuster`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct VuePaie {
    /// Les cases que sa carte ouvre, et leur place (`carte_rendue`).
    carte: CarteRendue,
    resolu_dit: String,
    contrat_dit: String,
    /// Ce que la carte validee dit de la paie : ce qu'elle paye, en bref, son bulletin.
    paye_dit: String,
    bulletin_dit: Option<String>,
    numero_bulletin: Option<engagement::numeros::NumeroDeBulletin>,
    numero_contrat: Option<engagement::numeros::NumeroContrat>,
    salarie: SalarieVu,
    /// UN PERMANENT N'A PAS DE SPECTACLE, et `None` est la reponse juste.
    spectacle: Option<SpectacleVu>,
    nature: NatureVue,
    /// La periode du contrat (`Engagement::contrat`).
    bornes: PeriodeOuverte,
    contrat: ContratVu,
    dates_travail: Vec<Jour>,
    /// Ce que dit sa carte validee, fermee : ses jours travailles du mois en clair, et le mois abrege.
    quand: socle::ecran::QuandDeLaCarte,
    #[serde(serialize_with = "socle::grandeurs::ecrit::entier::serialize")]
    cachets: Cachets,
    /// Le taux de la premiere ligne de cachet ; sans cachet, celui que la carte ou son mois ecrit.
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    taux_cachet: Option<MontantUnitaire>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::a_virgule::option::serialize")]
    heures: Option<Heures>,
    /// Les rubriques que sa carte propose, et celle d'une ligne neuve (`engagement::proposees`).
    propose: engagement::proposees::Proposees,
    cases: Vec<CaseDeFrais>,
    remuneration: Vec<LigneVue>,
    /// Les jours de l'engagement (ceux que la carte declare, sinon ceux du calcul) ; le lieu qu'elle ecrit.
    #[serde(serialize_with = "socle::grandeurs::ecrit::entier::serialize")]
    jours: Jours,
    lieu: Option<String>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    brut: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    net_a_payer: Montant,
    /// Le net paye, connu quand le prelevement a la source l'est (`Paie::pas`, a la validation) ; vide avant.
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    net_paye: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    net_imposable: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    cout_employeur: Montant,
    avertissements: Vec<AvisVu>,
    informations: Vec<AvisVu>,
    emploi: EmploiVu,
    /// Le conseil que la saisie appelle (`gestes::conseils::de_saisie`).
    #[serde(skip_serializing_if = "Option::is_none")]
    conseil: Option<crate::gestes::conseils::ConseilVu>,
    /// La ligne de « Pour démarrer » qui demande les conges acquis avant le logiciel (`declarations::demarrage::conges_a_dire`).
    #[serde(skip_serializing_if = "Option::is_none")]
    a_dire: Option<[declarations::demarrage::Manque; 1]>,
}

/// Une paie pour l'ecran. `brut` : la carte de l'engagement ; `data` : la saisie du mois (son `taux_cachet`) ; `lignes` :
/// ce qu'une ligne neuve prend (`lignes_preremplies`), lu une fois pour le mois.
pub(crate) fn vue_paie(ctx: &Contexte, p: &Paie, brut: &SaisieCarte, data: &VueDuMois, lignes: &R<LignesPreremplies>) -> R<VuePaie> {
    let e = &p.engagement;
    let nat = e.nat()?;
    let s = &p.salarie;
    let exige = |c: &Option<String>, k: &str| if c.is_none() { Err(socle::fichiers::manque(k)) } else { Ok(c.clone()) };
    let salarie = SalarieVu {
        cle: s.id.clone(),
        nom: exige(&s.nom.en_texte(), "nom")?,
        prenom: match s.prenom_affichage {
            Some(_) => s.prenom_affichage.clone(),
            None => exige(&s.prenom.en_texte(), "prenom")?,
        },
    };
    let spectacle = match &p.spectacle {
        Some(s) => match s.titre.dite() {
            Some(titre) => Some(SpectacleVu { titre: titre.clone(), numero_objet: s.numero_objet.dite().cloned() }),
            None => return Err(socle::fichiers::manque("titre")),
        },
        None => None,
    };
    let mois_du_contrat = e.mois_du_contrat()?;
    // Le taux du cachet, tel que le contrat l'imprime : celui de la premiere ligne a l'unite « cachet » ; sans cachet, le
    // cachet par defaut du mois, et rien quand rien n'est ecrit (regle 1 : plus de 130 EUR).
    let mut premier_cachet = None;
    for l in &e.lignes {
        if droit::catalogue::rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?.unite == droit::catalogue::Unite::Cachet {
            premier_cachet = Some(l);
            break;
        }
    }
    let taux_cachet = match premier_cachet {
        Some(l) => Some(l.taux().and_then(engagement::ligne::TauxDeLigne::par_unite).ok_or_else(|| socle::fichiers::manque("taux"))?),
        None => data.taux_cachet,
    };
    let lieu = brut.lieu.clone();
    // Le net paye : le net a payer moins le prelevement a la source que la validation a calcule ; une paie que rien n'a
    // encore validee n'en a pas (`Paie::pas`) : son net paye n'est pas encore connu.
    let net_paye = p.pas.as_ref().map(|v| (p.net_a_payer - v.prelevement).au_centime());
    // CE QUE LE MOTEUR A COMPRIS DE LA CARTE, en une ligne : son bulletin (ou qu'il n'y en a pas), son contrat, ses
    // bornes (et la periode de paie d'un contrat a cheval), ce qu'elle paye, le metier, le brut.
    let resolu_dit = {
        let mut m = vec![match (&p.numero_bulletin, e.sans_paie()) {
            (_, true) => "aucun cachet ce mois-ci".to_string(),
            (Some(n), false) => format!("bulletin {}", n),
            (None, false) => "bulletin".to_string(),
        }];
        if let Some(n) = e.numero.as_ref() {
            m.push(format!("contrat {}", n));
        }
        let mut bornes = match e.contrat().fin() {
            Some(f) => format!("{} → {}", e.contrat().debut().jj_mm_aaaa(), f.jj_mm_aaaa()),
            None => format!("{} →", e.contrat().debut().jj_mm_aaaa()),
        };
        if e.place_du_mois()? != PlaceDuMois::Seul {
            let payee = e.periode_payee()?;
            bornes.push_str(&format!(" (paie du {} au {})", payee.debut().jj_mm_aaaa(), payee.fin().jj_mm_aaaa()));
        }
        m.push(bornes);
        let paye = p.paye_en_bref();
        m.push(if paye.is_empty() { "—".to_string() } else { paye });
        if let Some(emploi) = s.emploi.dite().filter(|x| !x.is_empty()) {
            let cadre = if droit::bareme::est_cadre(s) { " (cadre)" } else { "" };
            m.push(format!("{}{}", emploi.to_lowercase(), cadre));
        }
        m.push(format!("brut {}", socle::texte::euros(p.brut)));
        m.join(" · ")
    };
    // LA LIGNE DU CONTRAT, SOUS SES LIGNES DE PAIE : ses bornes et ses jours travailles (`bornes_dites`), son numero, et
    // ce qu'il fait par rapport au mois (en cours depuis un mois passe, se poursuit au mois suivant).
    let contrat_dit = {
        let debut = e.contrat().debut();
        let mut t = bornes_dites(debut, e.contrat().fin(), nat.mensuelle(), Some(e.jours));
        if let Some(n) = &e.numero {
            t.push_str(&format!(" · n° {}", n));
        }
        if e.contrat().fin().is_some() {
            let place = e.place_du_mois()?;
            if let PlaceDuMois::Dernier | PlaceDuMois::Milieu = place {
                t.push_str(&format!(" — en cours depuis {}", debut.mois_civil().libelle(Graphie::Accentuee)));
            }
            if let PlaceDuMois::Premier | PlaceDuMois::Milieu = place {
                t.push_str(&format!(" — se poursuit en {}", e.debut_mois().mois_civil().suivant().libelle(Graphie::Accentuee)));
            }
        }
        t
    };
    let trop_courte = || socle::echec::refus(socle::fichiers::trop_courte("les mois du contrat"));
    let rupture = rupture_de(e, &nat);
    let temps = || -> R<Option<droit::natures::ModaliteDuTempsDeTravail>> {
        if !nat.mensuelle() {
            return Ok(None);
        }
        Ok(Some(e.modalite_temps_travail()?))
    };
    let mut ne_se_preremplit = Vec::new();
    let f = &mut ne_se_preremplit;
    let prerempli = Prerempli {
        date_paiement: if brut.date_paiement.is_none() { ou_vide(e.date_du_versement().map(Some), f) } else { None },
        debut: brut.debut.is_none().then(|| e.contrat().debut()),
        // Sans metier sur la carte, la paie prend l'emploi de la fiche (`s`, qu'aucune case de la carte ne change).
        metier: if brut.metier.is_none() { ou_vide(metier_de_la_fiche(s.code_emploi_ft.dite()), f) } else { None },
        lignes: lignes_ou_vide(lignes, f),
    };
    metier_du_dernier_contrat_illisible(ctx, brut, &mut ne_se_preremplit);
    let sait = CeQueLeMoteurSait { contrat: contrat_appelle(ctx, e, &nat), rupture: &rupture, temps: temps(), prerempli, ne_se_preremplit };
    let carte = carte_rendue(ctx, brut, sait);
    let rupture = FinDuContrat::de(rupture);
    Ok(VuePaie {
        carte,
        resolu_dit,
        contrat_dit,
        paye_dit: p.paye_en_bref(),
        bulletin_dit: p.numero_bulletin.as_ref().map(|n| format!("bulletin {}", n)),
        numero_bulletin: p.numero_bulletin.clone(),
        numero_contrat: e.numero.clone(),
        salarie,
        spectacle,
        nature: NatureVue { cle: nat.cle, libelle: nat.libelle.clone(), mensuelle: nat.mensuelle() },
        bornes: e.contrat(),
        contrat: ContratVu {
            place: e.place_du_mois()?,
            chevrons: chevrons_dits(e)?,
            sans_paie: e.sans_paie(),
            depuis: mois_du_contrat.first().copied().ok_or_else(trop_courte)?,
            jusqu_a: mois_du_contrat.last().copied().ok_or_else(trop_courte)?,
            periode: e.periode_payee()?,
            rupture,
        },
        dates_travail: e.dates_travail.clone(),
        quand: socle::ecran::QuandDeLaCarte::des_jours(&e.dates_travail, data.mois)?,
        cachets: e.cachets,
        taux_cachet,
        heures: e.heures,
        // LES RUBRIQUES QUE SA CARTE PROPOSE : sa nature, et le statut et le metier de la fiche telle que la paie la lit
        // (le metier de la carte par-dessus celui du salarie).
        propose: engagement::proposees::proposees(ctx, Some(nat.mensuelle()), s.statut.dite().copied(), s.code_emploi_ft.dite(), &data.mois.premier_jour())?,
        // LES CASES QU'UNE LIGNE DE FRAIS APPELLE : le lieu d'une nuitée, le restaurant d'un repas, le justificatif d'un
        // frais au-delà de sa limite (`engagement::frais_au_dela`).
        cases: e
            .lignes
            .iter()
            .filter_map(|l| l.marques())
            .filter_map(|m| Some((m.rang?, m)))
            .flat_map(|(i, m)| {
                m.cases.iter().map(move |k| CaseDeFrais { ligne: i, case: *k, libelle: libelle_de_case(*k), aide: m.aides.get(k).cloned() })
            })
            .collect(),
        remuneration: p
            .remuneration
            .iter()
            .map(|l| LigneVue { rubrique: l.code.clone(), designation: l.designation(), quantite: l.paiement, taux: l.paiement.taux(), montant: l.montant.au_centime() })
            .collect(),
        jours: e.jours,
        lieu,
        brut: p.brut.au_centime(),
        net_a_payer: p.net_a_payer.au_centime(),
        // CE QUE LA COMPAGNIE VIRE : la somme effectivement recue par le salarie (C. trav. R. 3243-1, 10°,
        // `atelier/reference/sources/code-du-travail/articles-20261029.md`), le « MONTANT NET À PAYER » du bulletin :
        // le net a payer moins l'impot preleve a la source.
        net_paye,
        net_imposable: p.net_imposable.au_centime(),
        cout_employeur: p.cout_employeur.au_centime(),
        avertissements: p.avertissements.iter().filter(|a| !a.info).map(AvisVu::de).collect(),
        informations: p.avertissements.iter().filter(|a| a.info).map(AvisVu::de).collect(),
        emploi: EmploiVu { libelle: s.emploi.clone(), code_emploi_ft: s.code_emploi_ft.clone(), statut: s.statut.clone(), cadre: droit::bareme::est_cadre(s) },
        conseil: None,
        a_dire: None,
    })
}

/// LES CHEVRONS D'UN CONTRAT SUR PLUSIEURS MOIS : ses bornes, ce qu'il fait du cote ou il deborde du mois (commence
/// avant, se poursuit apres), et le mois voisin ou mene le clic. `None` du cote ou il ne deborde pas.
pub(crate) fn chevrons_dits(e: &engagement::Engagement) -> R<Chevrons> {
    let clair = |m: Mois| m.libelle(Graphie::Accentuee);
    let bornes = match e.contrat().fin() {
        Some(f) => format!("Contrat du {} au {}", e.contrat().debut().jj_mm_aaaa(), f.jj_mm_aaaa()),
        None => format!("Contrat du {}, sans date de fin", e.contrat().debut().jj_mm_aaaa()),
    };
    let mois = e.mois_du_contrat()?;
    let premier = e.debut_mois();
    let place = e.place_du_mois()?;
    let avant = match place {
        PlaceDuMois::Seul | PlaceDuMois::Premier => None,
        PlaceDuMois::Dernier | PlaceDuMois::Milieu => {
            let voisin = premier.avant(socle::date::Duree::jours(1))?.mois_civil();
            Some(match mois.first() {
                Some(&m) => format!("{} — commencé en {} · voir {}", bornes, clair(m), clair(voisin)),
                None => format!("{} — voir {}", bornes, clair(voisin)),
            })
        }
    };
    let apres = match place {
        PlaceDuMois::Seul | PlaceDuMois::Dernier => None,
        PlaceDuMois::Premier | PlaceDuMois::Milieu => {
            let suite = match (e.contrat().fin(), mois.last()) {
                (Some(_), Some(&j)) => format!("se poursuit jusqu'en {}", clair(j)),
                _ => "se poursuit".to_string(),
            };
            Some(format!("{} — {} · voir {}", bornes, suite, clair(premier.mois_civil().suivant())))
        }
    };
    Ok(Chevrons { avant, apres })
}

/// Ce qui, dans ce mois, porte sur une
/// fiche archivee. On n'INTERDIT pas : rejouer une paie passee, reimprimer un
/// bulletin perdu, cela doit marcher. Mais une paie NEUVE sur une fiche mise de
/// cote est presque toujours une erreur, et elle ne se verrait nulle part ailleurs.
pub(crate) fn archives_employees(paies: &[Paie]) -> R<Vec<String>> {
    let mut out = Vec::new();
    for p in paies {
        if p.salarie.archivee() {
            let qui = match p.salarie.nomme() {
                Some(n) => n,
                None => p.salarie.id.to_string(),
            };
            out.push(format!(
                "{} : sa fiche est archivee. Pour refaire une paie passee, rien a faire ; pour une nouvelle embauche, la \
                 reactiver (Configuration › Salaries, « Reactiver »).",
                qui
            ));
        }
        // Un permanent n'a pas de spectacle -- le numero d'objet lui est interdit
        // (CCH-11) -- donc rien a verifier de ce cote.
        if let Some(sp) = p.spectacle.as_ref().filter(|s| s.archivee()) {
            let titre = sp.titre.ecrite("titre")?;
            let lequel = match sp.numero_objet.portee() {
                Some(objet) => format!("« {} » (objet {})", titre, objet),
                None => format!("« {} »", titre),
            };
            out.push(format!("Le spectacle {} est archive : pour le jouer a nouveau, le reactiver (Configuration › Spectacles).", lequel));
        }
    }
    Ok(out)
}

/// Les trois nombres d'une carte (un engagement) ou d'un salarie (la colonne « Le mois »).
#[derive(Debug, Clone, Serialize)]
struct Trois {
    /// Vide tant que le net paye d'une de ces paies ne l'est (`VuePaie::net_paye`).
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    net_paye: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    brut: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    cout_employeur: Montant,
    engagements: usize,
}

/// Le net paye de `vues` : leur somme, quand chacun est connu ; vide sinon.
fn net_paye_de<'a>(vues: impl Iterator<Item = &'a VuePaie>) -> Option<Montant> {
    vues.map(|v| v.net_paye).sum::<Option<Montant>>().map(Montant::au_centime)
}

impl Trois {
    fn de(vues: &[&VuePaie]) -> Trois {
        let somme = |f: fn(&VuePaie) -> Montant| vues.iter().fold(Montant::ZERO, |t, v| t + f(v)).au_centime();
        Trois { net_paye: net_paye_de(vues.iter().copied()), brut: somme(|v| v.brut), cout_employeur: somme(|v| v.cout_employeur), engagements: vues.len() }
    }
}

/// La ligne d'un salarie dans la colonne « Le mois » : ses trois nombres, son nom, ses engagements, et au survol le cout
/// et le brut.
#[derive(Debug, Serialize)]
struct ParSalarie {
    #[serde(flatten)]
    trois: Trois,
    salarie: SalarieVu,
    nom: String,
    engagements_dit: String,
    survol_dit: String,
}

/// Un engagement de la saisie qui ne se calcule pas encore : ce qui lui manque, son salarie tel que la saisie l'ecrit,
/// ses bornes comme le moteur les lit de ses lignes de travail (la carte les montre avant tout calcul), les cases du
/// contrat ecrit qu'il appelle deja.
#[derive(Debug, Serialize)]
struct PaieACompleter {
    a_completer: String,
    salarie: CleEcrite,
    avertissements: [String; 0],
    informations: [String; 0],
    contrat: EcritsSeuls,
    /// Ses bornes, lues de ce que sa carte dit ou de ses jours coches (`bornes_dites`, sans ses jours travailles que la
    /// paie seule connait) ; rien tant qu'elle ne dit pas son debut.
    contrat_dit: Option<String>,
    /// Les cases que sa carte ouvre deja, lues sur sa saisie (`carte_rendue`) ; son debut et sa fin lus de ses jours
    /// (`prerempli`) : une carte a completer peut ecrire une fin avant son debut, c'est alors ce qui lui manque
    /// (`a_completer`).
    carte: CarteRendue,
    /// Les rubriques que sa carte propose deja, lues sur sa saisie (`engagement::proposees::de_la_carte`).
    propose: engagement::proposees::Proposees,
}

/// Le salarie tel que la carte le nomme.
#[derive(Debug, Serialize)]
struct CleEcrite {
    cle: Option<IdSalarie>,
}

#[derive(Debug, Serialize)]
struct EcritsSeuls {
    /// Sa rupture, lue sur sa saisie (`rupture_en_saisie`) : le bloc de fin se dessine avant que la paie se calcule.
    rupture: FinDuContrat,
}

/// Une paie de l'apercu : calculee, ou a completer.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum PaieVue {
    Calculee(Box<VuePaie>),
    ACompleter(PaieACompleter),
}

impl PaieVue {
    fn textes(&self) -> impl Iterator<Item = &String> {
        match self {
            PaieVue::Calculee(v) => Some(v.avertissements.iter().chain(v.informations.iter()).map(|a| &a.texte)),
            PaieVue::ACompleter(_) => None,
        }
        .into_iter()
        .flatten()
    }
}

/// Une paie de l'apercu, et l'identite de son engagement : la carte retrouve la sienne.
#[derive(Debug, Serialize)]
struct PaieDeLApercu {
    #[serde(flatten)]
    paie: PaieVue,
    engagement: Identite,
}

/// Les paies de la saisie, et ce que le mois en compte.
struct Calcul {
    phrase_du_mois: Option<String>,
    mois: Mois,
    libelle: String,
    paies: Vec<PaieVue>,
    total_cout: Montant,
    /// Vide tant qu'un net paye du mois ne l'est (`VuePaie::net_paye`).
    total_net: Option<Montant>,
    blocs: Vec<Option<Trois>>,
    par_salarie: Vec<ParSalarie>,
    avertissements: Vec<String>,
    informations: Vec<String>,
}

/// L'APERCU (`apercu`) : le mois tel que l'ecran le montre, rien d'ecrit.
#[derive(Debug, Serialize)]
struct Apercu {
    phrase: Phrase,
    phrase_du_mois: Option<String>,
    mois: Mois,
    libelle: String,
    paies: Vec<PaieDeLApercu>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    total_cout: Montant,
    /// Vide tant qu'un net paye du mois ne l'est (`VuePaie::net_paye`).
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    total_net: Option<Montant>,
    /// Ce que disent les deux nombres du mois : le net (ce qu'on vire), le cout employeur.
    total_net_dit: &'static str,
    total_cout_dit: &'static str,
    blocs: Vec<Option<Trois>>,
    par_salarie: Vec<ParSalarie>,
    avertissements: Vec<String>,
    informations: Vec<String>,
    /// Ce que le moteur dit de chaque engagement (les encarts de sa carte), en phrases : le mois, puis ses paies.
    phrases: Vec<String>,
}

/// `apercu` : le mois tel que l'ecran le montre, rien d'ecrit -- ni fichier, ni compteur.
///
/// Un engagement valide montre la paie qu'il a ecrite, avec ses numeros du moment ; s'il s'est recalcule de lui-meme,
/// ses informations disent quand et pourquoi (`etats::phrase_du_recalcul`). Un engagement en saisie se calcule, et
/// porte ses conseils de saisie.
/// L'APERCU D'UNE SAISIE DONT DES ENGAGEMENTS NE SE CALCULENT PAS ENCORE (`manques_de_la_saisie`) : les autres se
/// calculent, et chacun de ceux-la tient sa place dans `paies` avec ce qui lui manque (`a_completer`), sans montant ;
/// la carte qui en porte un n'a pas de total. Une saisie dont aucun refus ne se rapporte a un engagement refuse.
fn apercu(ctx: &Contexte, c: ChargeApercu) -> R<Apercu> {
    let mois = c.mois;
    let (data, identites) = mois_designe(ctx, c.mois, c.modifications)?;
    let r = apercu_des_paies(ctx, &data)?;
    let mut paies = Vec::new();
    for (mut paie, engagement) in r.paies.into_iter().zip(identites) {
        if let PaieVue::Calculee(v) = &mut paie {
            dire_les_tarifs(ctx, mois, v)?;
        }
        paies.push(PaieDeLApercu { paie, engagement });
    }
    let phrase = match &r.phrase_du_mois {
        Some(p) => socle::phrase!("{}", p),
        None => socle::phrase!("{} : {}.", r.libelle, socle::ecran::compte(paies.len(), "paie", "paies")),
    };
    let mut phrases: Vec<String> = Vec::new();
    for t in r.avertissements.iter().chain(r.informations.iter()).chain(paies.iter().flat_map(|p| p.paie.textes())) {
        if !phrases.contains(t) {
            phrases.push(t.clone());
        }
    }
    Ok(Apercu {
        phrase,
        phrase_du_mois: r.phrase_du_mois,
        mois: r.mois,
        libelle: r.libelle,
        paies,
        total_cout: r.total_cout,
        total_net: r.total_net,
        total_net_dit: "Net à virer",
        total_cout_dit: "Coût employeur",
        blocs: r.blocs,
        par_salarie: r.par_salarie,
        avertissements: r.avertissements,
        informations: r.informations,
        phrases,
    })
}

/// LE MONTANT D'UN REPAS OU D'UNE NUITEE DIT D'OU IL VIENT : une ligne au tarif de la convention (`engagement::tarif_conventionnel`,
/// le barème de l'année, couche de la convention de la compagnie) le dit dans les informations de sa paie. Le tarif
/// est daté : il se lit au premier jour du mois, au premier et au dernier jour de la paie dans le mois.
fn dire_les_tarifs(ctx: &Contexte, mois: Mois, p: &mut VuePaie) -> R<()> {
    let premier = mois.premier_jour();
    let mut jours = vec![premier];
    for j in std::iter::once(p.bornes.debut()).chain(p.bornes.fin()) {
        if mois.contient(&j) && !jours.contains(&j) {
            jours.push(j);
        }
    }
    let mut dits: Vec<String> = Vec::new();
    for l in &p.remuneration {
        let mut tarif = None;
        for j in &jours {
            match engagement::tarif_conventionnel(ctx, &l.rubrique, j)? {
                Some(t) if Some(t) == l.taux.par_unite() => {
                    tarif = Some(t);
                    break;
                }
                _ => {}
            }
        }
        if let Some(tarif) = tarif {
            let dit = format!("{} à {}", l.designation.to_lowercase(), socle::texte::euros(tarif));
            if !dits.contains(&dit) {
                dits.push(dit);
            }
        }
    }
    if dits.is_empty() {
        return Ok(());
    }
    let idcc = ctx.entreprise.idcc.exigee("idcc")?;
    let quoi = dits.join(", ");
    let quoi = match nom_prenom(p.salarie.nom.portee(), p.salarie.prenom.portee()) {
        Some(qui) => format!("{} : {}", qui, quoi),
        None => quoi[..1].to_uppercase() + &quoi[1..],
    };
    p.informations.push(AvisVu::phrase(format!("{}, le tarif de la convention collective {}.", quoi, idcc)));
    Ok(())
}

/// « Nom Prenom », de ce que la fiche dit ; rien quand elle ne dit ni l'un ni l'autre.
fn nom_prenom(nom: Option<&str>, prenom: Option<&str>) -> Option<String> {
    match (nom, prenom) {
        (Some(n), Some(p)) => Some(format!("{} {}", n, p)),
        (Some(un), None) | (None, Some(un)) => Some(un.to_string()),
        (None, None) => None,
    }
}

/// UN CONTRAT COMMENCE AVANT LE LOGICIEL, SUR SA CARTE PAS ENCORE VALIDEE (`engagement::stock::commence_avant_le_logiciel`) :
/// la phrase qui dit ce qui se passe (ni DPAE ni contrat ici ; le numero sous lequel la DSN le declare, et qu'un numero
/// deja declare par l'ancien logiciel se dit dans sa case, `NUMERO_DIT`), et, pour un contrat paye au mois dont la fiche
/// ne dit pas ses conges a la reprise, la ligne de « Pour démarrer » qui y mene (`a_dire`,
/// `declarations::demarrage::conges_a_dire`) : le compteur de conges part du solde de l'ancien bulletin (L3141-3,
/// `engagement::conges::reprise_conges`). `brut` : l'engagement tel que la saisie l'ecrit ; `v` : sa vue (`vue_paie`).
fn dire_l_entree(ctx: &Contexte, mois: Mois, brut: &SaisieCarte, v: &mut VuePaie) -> R<()> {
    if brut.est_validee() || !engagement::stock::commence_avant_le_logiciel(&ctx.racine, v.bornes.debut(), mois)? {
        return Ok(());
    }
    let qui = match nom_prenom(v.salarie.nom.as_deref(), v.salarie.prenom.as_deref()) {
        Some(q) => q,
        None => v.salarie.cle.to_string(),
    };
    let le = v.bornes.debut().jj_mm_aaaa();
    // L'apercu numerote chaque paie qu'il montre (`ecrire_documents::numeroter`).
    let Some(numero) = v.numero_contrat.as_ref().map(|n| n.as_str().to_string()) else {
        return Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : une paie de l'aperçu n'a pas de numéro de contrat."));
    };
    let phrase = if brut.numero.is_some() {
        format!("{} : contrat commencé le {}, avant PuppetPaie : ni DPAE ni contrat à faire ici. La DSN le déclare sous le n° {}, celui que votre ancien logiciel a déclaré.", qui, le, numero)
    } else {
        format!(
            "{} : contrat commencé le {}, avant PuppetPaie : ni DPAE ni contrat à faire ici. La DSN le déclare sous le n° {} ; si votre ancien logiciel l'a déjà déclaré en DSN, dites son numéro (« Numéro en DSN »), sinon il compterait pour un second contrat.",
            qui, le, numero
        )
    };
    v.informations.insert(0, AvisVu::phrase(phrase));
    let au_mois = v.nature.cle.proprietes()?.au_mois();
    if au_mois.is_some_and(|au| !declarations::demarrage::conges_repris(ctx, &au, &v.salarie.cle)) {
        let depuis = mois.premier_jour();
        v.a_dire = Some([declarations::demarrage::conges_a_dire(&v.salarie.cle, &qui, depuis)]);
    }
    Ok(())
}

/// Les paies de l'apercu de `data`, une par engagement, dans son ordre.
fn apercu_des_paies(ctx: &Contexte, data: &VueDuMois) -> R<Calcul> {
    let lignes = &lignes_preremplies(ctx, data);
    let refus = match apercu_de(ctx, data, lignes) {
        Ok(c) => return Ok(c),
        Err(refus) => refus,
    };
    let manques = manques_de_la_saisie(ctx, data);
    if manques.iter().all(Option::is_none) {
        return Err(refus);
    }
    let tous = &data.engagements;
    let complete = VueDuMois { engagements: tous.iter().zip(&manques).filter(|(_, m)| m.is_none()).map(|(e, _)| e.clone()).collect(), ..data.clone() };
    let mut r = apercu_de(ctx, &complete, lignes)?;
    let mut calculees = std::mem::take(&mut r.paies).into_iter();
    let mut calcules = std::mem::take(&mut r.blocs).into_iter();
    let mois = data.mois;
    for (e, m) in tous.iter().zip(&manques) {
        match m {
            None => {
                r.paies.extend(calculees.next());
                r.blocs.push(calcules.next().flatten());
            }
            Some(m) => {
                let rupture = rupture_en_saisie(e, mois);
                // Rien ne pre-remplit le jour du versement tant que la paie ne se calcule pas : le debut est celui que
                // ses jours coches disent ; le metier, celui de la fiche du salarie qu'elle nomme.
                let mut ne_se_preremplit = Vec::new();
                let f = &mut ne_se_preremplit;
                let fiche = e.salarie.as_ref().and_then(|s| ctx.salaries.get(s));
                let prerempli = Prerempli {
                    debut: if e.debut.is_none() { e.debut_lu() } else { None },
                    metier: if e.metier.is_none() { ou_vide(metier_de_la_fiche(fiche.and_then(|x| x.code_emploi_ft.dite())), f) } else { None },
                    lignes: lignes_ou_vide(lignes, f),
                    ..Prerempli::default()
                };
                metier_du_dernier_contrat_illisible(ctx, e, &mut ne_se_preremplit);
                let sait = CeQueLeMoteurSait { contrat: contrat_appelle_en_saisie(ctx, e, mois), rupture: &rupture, temps: Ok(None), prerempli, ne_se_preremplit };
                let carte = carte_rendue(ctx, e, sait);
                r.paies.push(PaieVue::ACompleter(PaieACompleter {
                    carte,
                    a_completer: m.phrase.clone(),
                    salarie: CleEcrite { cle: e.salarie.clone() },
                    avertissements: [],
                    informations: [],
                    contrat: EcritsSeuls { rupture: FinDuContrat::de(rupture) },
                    // Sa nature n'est pas encore lue : un contrat sans fin s'y dit par son debut seul.
                    contrat_dit: e.debut_lu().map(|d| bornes_dites(d, e.fin_lue(), false, None)),
                    propose: engagement::proposees::de_la_carte(ctx, e, &data.mois.premier_jour())?,
                }));
                // Une carte a completer n'a pas de total.
                r.blocs.push(None);
            }
        }
    }
    r.phrase_du_mois = phrase_du_mois(&r.paies, r.par_salarie.is_empty());
    Ok(r)
}

/// CE QUE LES CHIFFRES DU MOIS NE COMPTENT PAS, en une phrase sous eux (`None` : rien) : un engagement a completer ne
/// se calcule pas, mais il est la ; un mois sans aucun engagement se dit aussi.
fn phrase_du_mois(paies: &[PaieVue], aucun_chiffre: bool) -> Option<String> {
    let attendent = paies.iter().filter(|v| matches!(v, PaieVue::ACompleter(_))).count();
    match (attendent, aucun_chiffre) {
        (0, true) => Some("Aucun engagement ce mois-ci.".to_string()),
        (0, false) => None,
        (1, vide) => Some(format!("1 engagement à compléter{} : il compte quand il est complet.", if vide { "" } else { ", pas encore compté" })),
        (k, vide) => Some(format!("{} engagements à compléter{} : ils comptent quand ils sont complets.", k, if vide { "" } else { ", pas encore comptés" })),
    }
}

/// L'apercu de la saisie `data` : chaque engagement se calcule. `lignes` : ce qu'une ligne neuve prend
/// (`lignes_preremplies`, de la saisie entiere).
fn apercu_de(ctx: &Contexte, data: &VueDuMois, lignes: &R<LignesPreremplies>) -> R<Calcul> {
    let mois = data.mois;
    let lu = data.mois;
    let montrees = paies_de_l_ecran(ctx, data, mois)?;
    let mut paies: Vec<Paie> = montrees.iter().map(|m| m.paie.clone()).collect();
    documents::ecrire_documents::numeroter(ctx, &mut paies, lu.annee(), &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)?;
    let tous = &data.engagements;
    // Chaque paie de l'apercu est celle d'une carte de la saisie, au meme rang.
    let carte = |i: usize| tous.get(i).ok_or_else(|| erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : une paie de l'aperçu n'a pas sa carte."));
    let mut vues = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        vues.push(vue_paie(ctx, p, carte(i)?, data, lignes)?);
    }
    // LES CONSEILS DE SAISIE : au plus un par carte, quand l'option de la compagnie est cochee, pour un
    // engagement en saisie seulement. Ils ne vivent qu'ici, dans la vue de l'ecran : ni dans les avertissements de la
    // paie, ni au bulletin, ni a `produire`.
    for (i, (v, c)) in vues.iter_mut().zip(crate::gestes::conseils::de_saisie(ctx, &paies)?).enumerate() {
        if !tous.get(i).is_some_and(SaisieCarte::est_validee) {
            v.conseil = c;
        }
    }
    for (i, v) in vues.iter_mut().enumerate() {
        dire_l_entree(ctx, mois, carte(i)?, v)?;
    }
    for (v, m) in vues.iter_mut().zip(&montrees) {
        if let Some(phrase) = m.recalcul.as_ref().and_then(engagement::etats::phrase_du_recalcul) {
            v.informations.insert(0, AvisVu::phrase(phrase));
        }
    }
    let total_cout = vues.iter().fold(Montant::ZERO, |t, v| t + v.cout_employeur).au_centime();
    let total_net = net_paye_de(vues.iter());
    // LES TROIS NOMBRES, sommes ici : par carte (une carte, un engagement) et par salarie (la colonne « Le mois »).
    let blocs = vues.iter().map(|v| Some(Trois::de(&[v]))).collect();
    let mut salaries: Vec<(String, String, Vec<&VuePaie>)> = Vec::new();
    for (v, p) in vues.iter().zip(&paies) {
        let cle = v.salarie.cle.to_string();
        match salaries.iter_mut().find(|(c, _, _)| *c == cle) {
            Some((_, _, vs)) => vs.push(v),
            None => salaries.push((cle, super::nom_a_l_ecran(&p.salarie, &p.salarie.id), vec![v])),
        }
    }
    let par_salarie = salaries
        .iter()
        .map(|(_, nom, vs)| {
            let trois = Trois::de(vs);
            ParSalarie {
                engagements_dit: socle::ecran::compte(vs.len(), "engagement", "engagements"),
                survol_dit: format!("coût employeur {} · brut {}", socle::texte::euros(trois.cout_employeur), socle::texte::euros(trois.brut)),
                salarie: vs[0].salarie.clone(),
                nom: nom.clone(),
                trois,
            }
        })
        .collect::<Vec<_>>();
    let mut avertissements = archives_employees(&paies)?;
    let mut informations = Vec::new();
    for v in &vues {
        avertissements.extend(v.avertissements.iter().map(|a| a.texte.clone()));
        informations.extend(v.informations.iter().map(|a| a.texte.clone()));
    }
    let paies: Vec<PaieVue> = vues.into_iter().map(|v| PaieVue::Calculee(Box::new(v))).collect();
    Ok(Calcul {
        phrase_du_mois: phrase_du_mois(&paies, par_salarie.is_empty()),
        mois: lu,
        libelle: lu.libelle(socle::mois::Graphie::Titre),
        paies,
        total_cout,
        total_net,
        blocs,
        par_salarie,
        avertissements,
        informations,
    })
}
