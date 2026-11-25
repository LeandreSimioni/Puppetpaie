//! Les routes des IMPREVUS d'un salarie : la liste (`imprevus`, `arrets`, `fins-de-contrat`), l'arret saisi, repris,
//! prolonge, signale ou oublie (`arret-enregistrer`, `arret-reprise`, `arret-prolonger`, `arret-signaler`,
//! `arret-oublier`), ses indemnites journalieres (`arret-indemnites`), la suspension du bloc 65 (sa boite,
//! `suspension-saisie` ; `suspension-enregistrer`, `-modifier`, `-oublier`), et la fin d'un contrat signalee, avec ses
//! documents (`fin-contrat-signaler`, `fin-contrat-documents`). La boite qui saisit un imprevu
//! (`SaisieDUnImprevu`) est rendue par une lecture : `suspension-saisie` ici, `arret-obligations` pour un arret.
//!
//! RIEN NE PART D'ICI : un imprevu s'ecrit dans `imprevus/AAAA-MM.json`, et c'est la production du mois qui en tire la
//! ligne de paie et le bloc de la DSN ; un signalement s'ecrit en fichier, il ne s'envoie pas. Les refus sont des
//! `Refuse` (la page les montre tels quels) ; un refus n'ecrit rien.
//!
//! CE QUI S'Y SAISIT, ET CE QUI L'APPELLE (rien de remplissable s'il n'y a pas a le remplir). Le moteur dit, sur chaque
//! imprevu resume (`resume`), ce que ses faits ouvrent : ses `gestes` (`GesteDeRoute<Designe>`), ceux seulement que l'imprevu
//! ouvre, chacun sa route, sa charge, son libelle et les cases que sa boite fait saisir ; et `a_dire`, ce qu'un arret
//! ecrit appelle encore (ses indemnites journalieres, quand il ouvre le maintien de salaire, par la loi ou la
//! convention). La page les dessine et les branche, sans choisir ni nommer de route.
//!
//! LES FORMES. Chaque charge est une structure lue une fois a la porte (`enveloppe::Charge::lire`) ; chaque reponse, une
//! structure qui porte sa phrase. Ce que ce module lit d'un autre domaine se lit en sa forme : l'imprevu ecrit
//! (`engagement::imprevus` : `ArretSaisi`, `SuspensionSaisie`, `Imprevu`), la saisie des engagements
//! (`engagement::saisie::Carte`), le journal des envois (`envois::journal`), les obligations d'un arret
//! (`crate::obligations`), les nomenclatures livrees du bloc 60 (`droit::arrets::Bloc60`) et des suspensions
//! (`droit::permanents`).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::enveloppe::{en_json, route_inconnue, Charge, Dite, Phrase, Porte, Rendu};
use compagnie::{maintenant, INTERFACE};
use crate::SansCharge;
use socle::ecran::Pastille;
use declarations::signalement;
use droit::contexte::{Contexte, IdSalarie, SalarieDesigne};
use droit::nature_declaration::NatureDeLaDeclaration;
use droit::natures::{IdentifiantDeDeclaration, NumeroDOrdre};
use droit::type_declaration::TypeDeLaDeclaration;
use engagement::calcul::Paie;
use droit::arrets::{Bloc60, CodeDuBloc60, MotifDArret, MotifDeReprise, SubrogationDeLArret};
use droit::permanents::MotifDeSuspension;
use engagement::imprevus::{self, ArretSaisi, Imprevu, Prolongation, RepriseNotee, Suspend, SuspensionSaisie, GenreDImprevu};
use engagement::numeros::{numero_dit, NumeroContrat};
use engagement::saisie::{Carte, Preavis, RangDansLeMois, VueDuMois};
use socle::date::{Duree, EnJours, Jour, Periode};
use socle::echec::{erreur, Classe, Echec, R};
use socle::fichiers::NomDeFichier;
use socle::mode::Mode;
use socle::grandeurs::MontantUnitaire;
use socle::mois::{Graphie, Mois};

use crate::catalogue::{Acte, Affiche, Fiche, FORME, ILLISIBLE, MANQUE, OPERATION};
use crate::catalogue::Route;
use crate::catalogue::{Ecran, Place};
use crate::declaration::{charge, cle, dit, forme_du_type, legendes, ChargeDuType, Legende};
use crate::ecran::EnregistrerLEcran;
use crate::fiches::CaseDeLaFiche;
use crate::ouverture::Choix;
use droit::fiche::ValeurDeCase;
use crate::rappels::{ChoixDeLEcheance, GesteDEcheance, ValeursDeLEcheance};
use crate::metier::imprevus::{arret_a_ecrire, magasin, nomenclatures, tous_les_arrets, ArretDit, ContratAuJour};

/// Le dernier jour travaille avant l'arret, qui le designe.
const DERNIER_JOUR_TRAVAILLE: Legende = dit("Dernier jour travaillé", "le dernier jour travaillé avant l'arrêt, qui le désigne");
/// Le premier jour d'une absence, qui la designe.
const DEBUT: Legende = dit("Premier jour d'absence", "le premier jour de l'absence, qui la désigne");
const UN_ARRET: &str = r#"{"salarie":"FULLER","dernier_jour":"2026-11-27"}"#;

/// Une fiche de ce module : `rend` et les refus que toutes partagent.
const fn fiche(route: Route, geste: &'static str, charge: &'static ChargeDuType, exemple: &'static str, rend: &'static str, acte: Acte, ecran: Ecran) -> Fiche {
    Fiche { route, geste, charge, exemple, rend, refus: &[OPERATION, MANQUE, ILLISIBLE, FORME], acte, phrase: 0, ecran, pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee }
}

const ORDRE: Legende = dit("Numéro d'ordre", "son numéro d'ordre ; sinon le suivant de ceux déjà partis").avancee();
const REMPLACE: Legende = dit("Remplace", "le signalement qu'il remplace").avancee();
const MODE: Legende = dit("Test ou réel", "un test écrit un fichier d'essai, marqué comme tel").avancee();
const CONSTITUE_LE: Legende = dit("Constitué le", "le jour de constitution ; sinon aujourd'hui").avancee();

/// La nature d'un signalement d'arret (`declarations::signalement::NatureSignalement`) : 04 l'arret, 05 la reprise.

/// Ou ces routes se voient a l'ecran (une place par module) : sur le chemin simple, ou hors de lui.
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);
const AVANCE: Ecran = Ecran::DessineAvance(Place::Paie);
/// Les routes qu'un imprevu offre en gestes (`GesteDeRoute<Designe>`) : la page les atteint par le geste, dans une boite tiree
/// de leur fiche.
const GESTE: Ecran = Ecran::Geste(Place::Paie);
const GESTE_AVANCE: Ecran = Ecran::GesteAvance(Place::Paie);


/// Les routes de ce module, et leurs fiches, dans l'ordre d'un arrêt : il s'enregistre, se suit, se signale, s'oublie.
pub const ROUTES: &[Fiche] = &[
    fiche(Route::Imprevus, "Lire les imprévus d'un mois ou d'un salarié, et ce qu'on peut en saisir.",
        &ChargeImprevus::CHARGE,
        r#"{"mois":"2026-11"}"#, "imprevus (chacun résumé avec le nom de son salarié, qui ; un arrêt avec ses obligations), nomenclatures, porte (ce qui s'ouvre pour un salarié : chaque choix, la route qui l'enregistre en code, son libellé et ce qu'il couvre) et porte_dit ; pour un salarié, le plus récent d'abord, sous titre ; pour un mois, chaque imprévu ne dit que les jours payés de ce mois (cachets) ; pour un mois, dus et dus_titre : ce qui est dû de ses arrêts, le plus pressé d'abord (rang), chaque obligation avec qui et ses gestes (route, charge, libellé, cases)", Acte::Lit, ECRAN),
    fiche(Route::Arrets, "Lire tous les arrêts de la compagnie.", &SansCharge::CHARGE, "{}", "phrase, phrases (qui, quand, le motif, ce qui reste à signaler), arrets : chacun résumé", Acte::Lit, Ecran::Auto(Place::Exports, Affiche::new("Lire tous les arrêts"))),
    fiche(Route::FinsDeContrat, "Lire les fins de contrat dues ce mois, et ce qui manque à chacune.", &ChargeMois::CHARGE,
        r#"{"mois":"2026-11"}"#, "fins : chacune lue (lecture « lue » : le nom de son salarié, qui ; ce qui est fait et ce qui manque) ou sur une carte à corriger (lecture « a_corriger » : quoi corriger, et où) ; dus et dus_titre : celles qui sont dues, dans leur rang (rang), une fin due avec ses gestes (route, charge, libellé, cases)", Acte::Lit, ECRAN),
    fiche(Route::SuspensionSaisie, "Lire la boîte qui saisit une absence, pour ce qui est déjà dit.",
        &ChargeSuspensionSaisie::CHARGE,
        r#"{"salarie":"FULLER","debut":"2026-11-27"}"#, SAISIE_REND, Acte::Lit, ECRAN),
    Fiche { phrase: 400, ..fiche(Route::ArretEnregistrer, "Enregistrer un arrêt de travail.",
        &ChargeArretEnregistrer::CHARGE,
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","debut":"2026-11-30","fin":"2026-11-30","motif":"01"}"#,
        "arret (l'arrêt résumé), phrase ; appelle_encore et plus_tard, quand il appelle encore quelque chose (le geste de ses indemnités, a_dire ; les jours payés pendant lui, cachets)", Acte::Ecrit, ECRAN) },
    Fiche { phrase: 700, ..fiche(Route::ArretProlonger, "Prolonger un arrêt : sa nouvelle fin.",
        &ChargeArretProlonger::CHARGE,
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","fin":"2026-12-04"}"#,
        "arret (l'arrêt résumé), phrase", Acte::Ecrit, GESTE) },
    Fiche { phrase: 400, ..fiche(Route::ArretReprise, "Poser ou effacer la reprise d'un arrêt.",
        &ChargeArretReprise::CHARGE,
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","reprise":"2026-12-03"}"#, "arret (l'arrêt résumé), phrase", Acte::Ecrit, GESTE) },
    Fiche { phrase: 400, ..fiche(Route::ArretIndemnites, "Dire les indemnités journalières d'un arrêt, par jour.",
        &ChargeArretIndemnites::CHARGE,
        r#"{"salarie":"SVOBODA","dernier_jour":"2026-11-02","ij_brute":"40.89","ij_nette":"38.15"}"#, "arret (l'arrêt résumé), phrase", Acte::Ecrit, GESTE) },
    Fiche { phrase: 600, ..fiche(Route::ArretSignaler, "Fabriquer le signalement d'un arrêt, sans le déposer.",
        &ChargeArretSignaler::CHARGE,
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","nature":"04"}"#, "fichier (écrit dans sorties/AAAA-MM/), phrase, nature, type, ordre, avertissements, suite (le geste qui le dépose)", Acte::Ecrit, GESTE) },
    Fiche { phrase: 400, ..fiche(Route::ArretOublier, "Retirer un arrêt qui n'a pas été déclaré.", &ChargeArretOublier::CHARGE,
        UN_ARRET, "arret (l'arrêt retiré, résumé), phrase", Acte::Ecrit, GESTE) },
    Fiche { phrase: 400, ..fiche(Route::SuspensionEnregistrer, "Enregistrer une absence qui suspend un contrat.",
        &ChargeSuspensionEnregistrer::CHARGE,
        r#"{"salarie":"FULLER","debut":"2026-11-27","fin":"2026-11-27","motif":"501"}"#, "suspension (l'absence résumée), phrase ; appelle_encore et plus_tard, quand des jours sont payés pendant elle (cachets)", Acte::Ecrit, AVANCE) },
    Fiche { phrase: 400, ..fiche(Route::SuspensionModifier, "Changer la fin ou le motif d'une absence.",
        &ChargeSuspensionModifier::CHARGE,
        r#"{"salarie":"FULLER","debut":"2026-11-27","fin":"2026-11-27"}"#, "suspension (l'absence résumée), phrase", Acte::Ecrit, GESTE_AVANCE) },
    Fiche { phrase: 400, ..fiche(Route::SuspensionOublier, "Retirer une absence qui n'a pas été déclarée.", &ChargeSuspensionOublier::CHARGE,
        r#"{"salarie":"FULLER","debut":"2026-11-27"}"#, "suspension (l'absence retirée, résumée), phrase", Acte::Ecrit, GESTE_AVANCE) },
    Fiche { phrase: 600, ..fiche(Route::FinContratSignaler, "Fabriquer le signalement d'une fin de contrat, sans le déposer.",
        &ChargeFinContratSignaler::CHARGE,
        r#"{"mois":"2026-11","salarie":"FULLER"}"#, "fichier (écrit dans sorties/AAAA-MM/), phrase, type, ordre, preavis, avertissements, suite (le geste qui le dépose)", Acte::Ecrit, GESTE) },
    Fiche { phrase: 160, ..fiche(Route::FinContratDocuments, "Fabriquer le certificat de travail et le reçu pour solde de tout compte.",
        &ChargeFinContratDocuments::CHARGE,
        r#"{"mois":"2026-11","salarie":"FULLER"}"#, "phrase, certificat et recu (leurs chemins), dossier, avertissements (ce qu'il faut savoir en les remettant)", Acte::Ecrit, GESTE) },
];

// ------------------------------------------------------------------ les charges
//
// `salarie` y est toujours ce que la personne en dit, un `SalarieDesigne` : la cle de la fiche, ou son nom, resolu a
// l'entree de la route en `IdSalarie` (`Contexte::cle_du_salarie`). Ce qu'elles ecrivent, la fenetre l'ecrit (`compagnie::INTERFACE`).

charge! {
    /// La charge de `imprevus` : les imprevus d'un mois, ou d'un salarie ; tous, sans l'un ni l'autre.
    #[derive(Debug)]
    struct ChargeImprevus {
        mois: Option<Mois> = legendes::MOIS,
        salarie: Option<SalarieDesigne> = dit("Salarié", "à la place du mois : les imprévus de ce salarié").avancee(),
    }
}

charge! {
    /// La charge de `fins-de-contrat` : le mois.
    #[derive(Debug)]
    struct ChargeMois {
        mois: Mois = legendes::MOIS,
    }
}

charge! {
    /// La charge de `arret-enregistrer`. `connu` est la connaissance de l'accident, en la forme que lit
    /// `obligations::connaissance_saisie` (`{jour, heure}`, celle de la route `arret-connaissance`). La boite qui saisit
    /// un arret (`arret-obligations`) nomme ses cases par ses champs (`cle!`).
    #[derive(Debug)]
    pub(crate) struct ChargeArretEnregistrer {
        pub(crate) salarie: SalarieDesigne = legendes::SALARIE,
        pub(crate) dernier_jour: Jour = DERNIER_JOUR_TRAVAILLE,
        pub(crate) debut: Jour = dit("Début de l'arrêt", "le premier jour que l'avis prescrit ; le dernier jour travaillé s'il a été prescrit ce jour-là"),
        pub(crate) fin: Jour = dit("Fin prescrite", "la fin prescrite"),
        pub(crate) motif: MotifDArret = dit("Motif", "le motif d'arrêt (motifs_arret des nomenclatures de imprevus)"),
        pub(crate) date_accident: Option<Jour> = dit("Jour de l'accident", "le jour de l'accident, quand le motif l'appelle").avancee(),
        pub(crate) connu: Option<crate::obligations::ConnaissanceDite> = dit("Su le", "{jour, heure} où la compagnie a su l'accident, quand le motif l'appelle").avancee(),
        pub(crate) contrat: Option<NumeroContrat> = dit("Contrat", "le contrat suspendu, quand plusieurs couvrent le jour").avancee(),
        #[serde(default, deserialize_with = "socle::date::Duree::en_jours_opt")]
        pub(crate) jours_deja_indemnises: Option<Duree<EnJours>> = dit("Jours déjà indemnisés", "pour le maintien d'un permanent").avancee(),
        pub(crate) note: Option<String> = dit("Note", "une note libre").avancee(),
    }
}

charge! {
    /// La charge de `arret-prolonger` : l'arret (son salarie, son dernier jour travaille) et sa nouvelle fin prescrite.
    #[derive(Debug)]
    struct ChargeArretProlonger {
        salarie: SalarieDesigne = legendes::SALARIE,
        dernier_jour: Jour = DERNIER_JOUR_TRAVAILLE,
        fin: Jour = dit("Nouvelle fin prescrite", "la nouvelle fin prescrite"),
    }
}

charge! {
    /// La charge de `arret-reprise` : la reprise (nulle ou absente, elle s'efface) et son motif.
    #[derive(Debug)]
    struct ChargeArretReprise {
        salarie: SalarieDesigne = legendes::SALARIE,
        dernier_jour: Jour = DERNIER_JOUR_TRAVAILLE,
        reprise: Option<Jour> = dit("Reprise le", "le jour de la reprise ; nul l'efface"),
        motif_reprise: Option<MotifDeReprise> = dit("Motif de la reprise", "le motif de la reprise").avancee(),
    }
}

charge! {
    /// La charge de `arret-indemnites` : chaque montant par jour (40.89 en nombre, ou « 40,89 » tel que le releve l'ecrit,
    /// converti a la porte : `Charge::lire` ; l'arret le garde au centime), et les jours deja indemnises ; absent, rien
    /// ne change.
    #[derive(Debug)]
    struct ChargeArretIndemnites {
        salarie: SalarieDesigne = legendes::SALARIE,
        dernier_jour: Jour = DERNIER_JOUR_TRAVAILLE,
        ij_brute: Option<MontantUnitaire> = dit("Indemnité brute d'un jour", "l'indemnité d'un jour, avant CSG et CRDS (relevé de la CPAM) ; absente, elle ne change pas"),
        ij_nette: Option<MontantUnitaire> = dit("Indemnité nette d'un jour", "la même, nette de CSG et CRDS"),
        ij_brute_majoree: Option<MontantUnitaire> = dit("Indemnité brute majorée", "accident du travail, dès le 29e jour : brute").avancee(),
        ij_nette_majoree: Option<MontantUnitaire> = dit("Indemnité nette majorée", "la même, nette").avancee(),
        #[serde(default, deserialize_with = "socle::date::Duree::en_jours_opt")]
        jours_deja_indemnises: Option<Duree<EnJours>> = dit("Jours déjà indemnisés", "les jours indemnisés dans les douze mois qu'aucun arrêt saisi ne porte").avancee(),
    }
}

pub(crate) use declarations::signalement::NatureSignalement;

charge! {
    /// La charge de `arret-signaler`. `remplace` : l'identifiant du signalement qu'il annule et remplace ; `ordre` : son
    /// numero d'ordre (absent : le suivant) ; `le` : son jour de constitution (absent : aujourd'hui). Le geste qu'offre le
    /// rappel d'un signalement l'ecrit aussi (`ChargeArretSignaler::de`).
    #[derive(Debug, Clone, Serialize)]
    pub(crate) struct ChargeArretSignaler {
        salarie: SalarieDesigne = legendes::SALARIE,
        dernier_jour: Jour = DERNIER_JOUR_TRAVAILLE,
        nature: NatureSignalement = dit("Nature", "ce que le signalement déclare"),
        #[serde(skip_serializing_if = "Option::is_none")]
        remplace: Option<IdentifiantDeDeclaration> = REMPLACE,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "droit::natures::en_nombre::option")]
        ordre: Option<NumeroDOrdre> = ORDRE,
        #[serde(skip_serializing_if = "Option::is_none")]
        mode: Option<Mode> = MODE,
        #[serde(skip_serializing_if = "Option::is_none")]
        le: Option<Jour> = CONSTITUE_LE,
    }
}

impl ChargeArretSignaler {
    /// Le signalement de l'arret de `salarie` qui suit `dernier_jour`, de sa `nature` : le suivant, constitue aujourd'hui.
    pub(crate) fn de(salarie: IdSalarie, dernier_jour: Jour, nature: NatureSignalement) -> ChargeArretSignaler {
        ChargeArretSignaler { salarie: salarie.into(), dernier_jour, nature, remplace: None, ordre: None, mode: None, le: None }
    }

    /// Le signalement parti, refait pour se lire (`gestes::depart::refaire_le_signalement`) : son numero d'ordre et son
    /// jour de constitution, tels que le journal les dit.
    pub(crate) fn refait(salarie: IdSalarie, dernier_jour: Jour, nature: NatureSignalement, ordre: NumeroDOrdre, le: Jour) -> ChargeArretSignaler {
        ChargeArretSignaler { ordre: Some(ordre), le: Some(le), ..ChargeArretSignaler::de(salarie, dernier_jour, nature) }
    }
}

/// Le mode qu'un signalement ecrit (S10.G00.00.005) : celui que la charge dit (`test`, le fichier d'essai) ; une charge
/// qui ne demande pas d'essai ecrit le fichier reel, que son depot de test bascule en memoire.
fn mode_du_fichier(dit: Option<Mode>) -> Mode {
    match dit {
        Some(m) => m,
        None => Mode::Reel,
    }
}

charge! {
    /// La charge de `arret-oublier`.
    #[derive(Debug)]
    struct ChargeArretOublier {
        salarie: SalarieDesigne = legendes::SALARIE,
        dernier_jour: Jour = DERNIER_JOUR_TRAVAILLE,
    }
}

charge! {
    /// La charge de `suspension-enregistrer`.
    #[derive(Debug)]
    struct ChargeSuspensionEnregistrer {
        salarie: SalarieDesigne = legendes::SALARIE,
        debut: Jour = DEBUT,
        fin: Jour = dit("Dernier jour", "son dernier jour"),
        motif: MotifDeSuspension = dit("Motif", "le code du motif (nomenclatures de imprevus)"),
        contrat: Option<NumeroContrat> = dit("Contrat", "le contrat, quand plusieurs couvrent le jour").avancee(),
        note: Option<String> = dit("Note", "une note libre").avancee(),
    }
}

charge! {
    /// La charge de `suspension-saisie` : l'absence telle qu'elle se saisit, chaque champ vide tant qu'il n'est pas dit
    /// (les champs de `suspension-enregistrer`, que ses cases portent), et la carte d'ou l'on vient (le numero et le debut
    /// de son contrat : c'est le sien qu'elle suspend).
    #[derive(Debug)]
    struct ChargeSuspensionSaisie {
        salarie: Option<SalarieDesigne> = dit("Salarié", "le salarié absent"),
        debut: Option<Jour> = dit("Premier jour d'absence", "le premier jour de l'absence"),
        fin: Option<Jour> = dit("Dernier jour", "son dernier jour"),
        motif: Option<MotifDeSuspension> = dit("Motif", "le code du motif"),
        contrat: Option<NumeroContrat> = dit("Contrat", "le contrat, quand plusieurs couvrent le jour").avancee(),
        note: Option<String> = dit("Note", "une note libre").avancee(),
        carte_numero: Option<NumeroContrat> = CARTE_NUMERO,
        carte_debut: Option<Jour> = CARTE_DEBUT,
    }
}

/// La carte d'ou l'on vient, quand une boite de saisie s'ouvre depuis elle : le numero de son contrat, son premier jour.
pub(crate) const CARTE_NUMERO: Legende = dit("Contrat de la carte", "le numéro du contrat de la carte d'où l'on vient").avancee();
pub(crate) const CARTE_DEBUT: Legende = dit("Début de la carte", "le premier jour du contrat de la carte d'où l'on vient").avancee();

charge! {
    /// La charge de `suspension-modifier` : le debut la designe ; la fin et le motif, s'ils se disent, changent.
    #[derive(Debug)]
    struct ChargeSuspensionModifier {
        salarie: SalarieDesigne = legendes::SALARIE,
        debut: Jour = DEBUT,
        fin: Option<Jour> = dit("Nouvelle fin", "sa nouvelle fin"),
        motif: Option<MotifDeSuspension> = dit("Nouveau motif", "son nouveau motif").avancee(),
    }
}

charge! {
    /// La charge de `suspension-oublier`.
    #[derive(Debug)]
    struct ChargeSuspensionOublier {
        salarie: SalarieDesigne = legendes::SALARIE,
        debut: Jour = DEBUT,
    }
}

charge! {
    /// La charge de `fin-contrat-signaler` (`remplace`, `ordre`, `test`, `le` : comme `arret-signaler`). Le geste
    /// qu'offre le rappel d'une fin de contrat l'ecrit aussi (`ChargeFinContratSignaler::de`).
    #[derive(Debug, Clone, Serialize)]
    pub(crate) struct ChargeFinContratSignaler {
        mois: Mois = legendes::MOIS,
        salarie: SalarieDesigne = legendes::SALARIE,
        #[serde(skip_serializing_if = "Option::is_none")]
        remplace: Option<IdentifiantDeDeclaration> = REMPLACE,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "droit::natures::en_nombre::option")]
        ordre: Option<NumeroDOrdre> = ORDRE,
        #[serde(skip_serializing_if = "Option::is_none")]
        mode: Option<Mode> = MODE,
        #[serde(skip_serializing_if = "Option::is_none")]
        le: Option<Jour> = CONSTITUE_LE,
    }
}

impl ChargeFinContratSignaler {
    /// Le signalement de la fin du contrat de `salarie` en `mois` : le suivant, constitue aujourd'hui.
    pub(crate) fn de(mois: Mois, salarie: IdSalarie) -> ChargeFinContratSignaler {
        ChargeFinContratSignaler { mois, salarie: salarie.into(), remplace: None, ordre: None, mode: None, le: None }
    }

    /// Le signalement parti, refait pour se lire (`gestes::depart::refaire_le_signalement`).
    pub(crate) fn refait(mois: Mois, salarie: IdSalarie, ordre: NumeroDOrdre, le: Jour) -> ChargeFinContratSignaler {
        ChargeFinContratSignaler { ordre: Some(ordre), le: Some(le), ..ChargeFinContratSignaler::de(mois, salarie) }
    }
}

charge! {
    /// La charge de `fin-contrat-documents`. Le geste « Papiers de fin » d'une fin due l'ecrit aussi (`gestes_d_une_fin`).
    #[derive(Debug, Clone, Serialize)]
    pub(crate) struct ChargeFinContratDocuments {
        mois: Mois = legendes::MOIS,
        salarie: SalarieDesigne = legendes::SALARIE,
        #[serde(skip_serializing_if = "Option::is_none")]
        le: Option<Jour> = dit("Établis le", "le jour où ils s'établissent ; sinon aujourd'hui").avancee(),
    }
}

/// Le jour de constitution d'un signalement ou d'un document : celui que la charge dit ; absent, aujourd'hui (la fiche de
/// la route le dit).
fn constitue_le(ctx: &Contexte, le: Option<Jour>) -> Jour {
    match le {
        Some(j) => j,
        None => ctx.aujourdhui,
    }
}

/// Le numero d'ordre d'un signalement : celui que la charge dit ; absent, le suivant de tous ceux deja partis
/// (`prochain_ordre`), avec ce que la plate-forme en dit.
fn ordre_du_signalement(ctx: &Contexte, ordre: Option<NumeroDOrdre>) -> R<(NumeroDOrdre, Vec<String>)> {
    match ordre {
        Some(n) => Ok((n, Vec::new())),
        None => prochain_ordre(ctx),
    }
}

// ------------------------------------------------------------------ les reponses

/// `imprevus` : les imprevus resumes, les nomenclatures pour en saisir, et ce que l'ecran lit de chacun ; et CE QUI EST
/// DU de leurs arrets (`dus`, le plus presse d'abord, chacune ses gestes ; `dus_titre`, quand il y en a).
#[derive(Debug, Serialize)]
struct ReponseImprevus {
    phrase: Phrase,
    #[serde(skip_serializing_if = "Option::is_none")]
    dus_titre: Option<&'static str>,
    dus: Vec<ObligationDue>,
    imprevus: Vec<Resume>,
    motifs_arret: Vec<CodeDuBloc60<MotifDArret>>,
    motifs_reprise: Vec<CodeDuBloc60<MotifDeReprise>>,
    motifs_suspension: Vec<MotifSuspension>,
    porte: Vec<Choix>,
    /// Le mot du bouton qui ouvre la porte, dans le dossier d'un salarie.
    porte_dit: &'static str,
    /// Le titre des imprevus d'un salarie (son dossier), quand il en a.
    #[serde(skip_serializing_if = "Option::is_none")]
    titre: Option<&'static str>,
    phrases: Vec<String>,
}

/// Le mot de la porte, et le titre des imprevus d'un salarie.
const PORTE_DIT: &str = "Imprévu…";
const SES_IMPREVUS: &str = "Ses imprévus";

/// LA PORTE D'UN IMPREVU, sur la carte d'un engagement et dans le dossier d'un salarie : ce qui s'y ouvre, chaque choix
/// par la lecture qui rend sa boite de saisie (son code : `arret-obligations`, `suspension-saisie`, `SaisieDUnImprevu`),
/// son libelle et ce qu'il couvre (`dit`).
fn porte() -> Vec<Choix> {
    vec![
        Choix::de(Route::ArretObligations.nom(), "Un arrêt de travail").disant(Some("maladie, accident, maternité".to_string())),
        Choix::de(Route::SuspensionSaisie.nom(), "Une absence").disant(Some("congé parental, sans solde".to_string())),
    ]
}

/// `arrets` : tous les arrets resumes, ceux qui restent a signaler, et les nomenclatures du bloc 60.
#[derive(Debug, Serialize)]
struct ReponseArrets {
    phrase: Phrase,
    phrases: Vec<String>,
    arrets: Vec<Resume>,
    a_signaler: Vec<Resume>,
    motifs: Vec<CodeDuBloc60<MotifDArret>>,
    motifs_reprise: Vec<CodeDuBloc60<MotifDeReprise>>,
    subrogation: Vec<CodeDuBloc60<SubrogationDeLArret>>,
}

/// `fins-de-contrat` : les fins, et celles qui sont DUES (`dus` : une carte a corriger d'abord, puis le plus presse ;
/// `dus_titre`, quand il y en a).
#[derive(Debug, Serialize)]
struct ReponseFins {
    phrase: Phrase,
    mois: Mois,
    fins: Vec<FinASignaler>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dus_titre: Option<&'static str>,
    dus: Vec<FinASignaler>,
}

/// Le titre de ce qui est du, au-dessus des obligations dues des arrets et des fins de contrat.
const CE_QUI_EST_DU: &str = "Ce qui est dû";

/// Le rang d'une carte a corriger parmi ce qui est du : devant toute obligation (`Obligation::rang`).
const RANG_D_UNE_CARTE_A_CORRIGER: &str = "0";

/// UNE OBLIGATION DUE D'UN ARRET, telle que la colonne « Ce qui est dû » la montre : pour qui (`qui`), son rang parmi ce
/// qui est du (`rang`, qui se range en texte avec celui des fins de contrat), l'obligation, et les gestes qui la font
/// (signaler, la dire faite, dire le jour ou on l'a su).
#[derive(Debug, Serialize)]
struct ObligationDue {
    qui: String,
    rang: String,
    obligation: crate::obligations::Obligation,
    gestes: Vec<GesteDEcheance>,
}

/// Les libelles des gestes qui signalent.
const SIGNALER_L_ARRET: Affiche = Affiche::new("Signaler l'arrêt");
const SIGNALER_LA_REPRISE: Affiche = Affiche::new("Signaler la reprise");
const SIGNALER_LA_FIN: Affiche = Affiche::new("Signaler la fin");
const PAPIERS_DE_FIN: Affiche = Affiche::new("Papiers de fin");

/// LE GESTE QUI FAIT UNE OBLIGATION DUE D'UN ARRET : le signalement de l'arret (ou son rectificatif), de sa reprise, qui
/// s'ecrit puis se depose (sa reponse dit la `suite`) ; une obligation hors DSN, qui se dit faite
/// (`Obligation::geste_qui_la_dit_faite`). Aucun pour une autre. Un signalement deja parti fait saisir celui qu'il
/// remplace.
pub(crate) fn geste_qui_fait(a: &ResumeDArret, o: &crate::obligations::Obligation) -> R<Option<GesteDEcheance>> {
    use crate::obligations::CleDObligation as C;
    let (nature, libelle) = match o.cle {
        C::SignalementArret | C::SignalementArretRectificatif => (NatureSignalement::Arret, SIGNALER_L_ARRET),
        C::SignalementReprise => (NatureSignalement::Reprise, SIGNALER_LA_REPRISE),
        _ => return o.geste_qui_la_dit_faite(a.salarie(), a.dernier_jour_travaille()),
    };
    let mut ce_que_c_est = vec![a.dit.quoi.clone(), a.dit.periode.clone()];
    if nature == NatureSignalement::Reprise {
        ce_que_c_est.extend(a.dit.reprise_dit.clone());
    }
    let mut phrases = vec![ce_que_c_est.join(", "), crate::ouverture::aide("signalement_dsn")?.to_string()];
    let mut cases = Vec::new();
    if let Some(deja) = a.dit.deja_signale.get(&nature) {
        phrases.push(format!("{}. {}", deja, crate::ouverture::aide("deja_signale")?));
        cases.push(cle!(ChargeArretSignaler, remplace));
    }
    let charge = ChargeArretSignaler::de(a.salarie().clone(), a.dernier_jour_travaille(), nature);
    Ok(Some(GesteDeRoute::avec_cases(Route::ArretSignaler, charge.into(), libelle, cases, ValeursDeLEcheance::default(), ChoixDeLEcheance::default()).disant(phrases)))
}

/// LE SIGNALEMENT D'UNE FIN DUE, qui s'ecrit puis se depose (sa reponse dit la `suite`) ; aucun tant qu'il lui manque une
/// case (`a_saisir`) : elle se dit d'abord dans la carte.
pub(crate) fn geste_du_signalement_de_fin(f: &FinLue) -> R<Option<GesteDEcheance>> {
    if !f.a_saisir.is_empty() {
        return Ok(None);
    }
    let charge = ChargeFinContratSignaler::de(f.mois, f.salarie.clone());
    let phrases = vec![format!("{} — {}", f.libelle_nature, f.contrat_dit), crate::ouverture::aide("signalement_fin")?.to_string()];
    let cases = vec![cle!(ChargeFinContratSignaler, remplace)];
    Ok(Some(GesteDeRoute::avec_cases(Route::FinContratSignaler, charge.into(), SIGNALER_LA_FIN, cases, ValeursDeLEcheance::default(), ChoixDeLEcheance::default()).disant(phrases)))
}

/// LES GESTES D'UNE FIN DUE : la signaler (`geste_du_signalement_de_fin`), et ecrire les deux papiers qu'on remet au
/// salarie qui part (sa boite dit d'abord ce qu'il faut savoir en les remettant).
fn gestes_d_une_fin(f: &FinLue) -> R<Vec<GesteDEcheance>> {
    let mut gestes: Vec<GesteDEcheance> = geste_du_signalement_de_fin(f)?.into_iter().collect();
    let papiers = ChargeFinContratDocuments { mois: f.mois, salarie: f.salarie.clone().into(), le: None };
    let avis = documents::fin_de_contrat::AVIS.iter().map(|a| a.to_string()).collect();
    gestes.push(GesteDeRoute::avec_cases(Route::FinContratDocuments, papiers.into(), PAPIERS_DE_FIN, Vec::new(), ValeursDeLEcheance::default(), ChoixDeLEcheance::default()).disant(avis));
    Ok(gestes)
}

/// Ce que rend une boite de saisie (`suspension-saisie`, `arret-obligations`).
pub(crate) const SAISIE_REND: &str = "titre, cases (chacune sa clé, celle d'un champ du geste qui enregistre, son libellé, sa forme, ses choix, sa valeur, ce que le moteur propose, prerempli, et ce qu'il dit sous elle), dit (ce que la boîte dit sous ses cases), enregistrer (sa route, sa charge déjà remplie, où chaque case va sous sa clé, et son libellé ; aucun quand il ne pourrait que refuser : aucun contrat ce jour-là, le salarié illisible, et dit dit pourquoi)";

/// LA BOITE QUI SAISIT UN IMPREVU, telle qu'une lecture la rend pour les faits deja dits (`suspension-saisie` pour une
/// absence, `arret-obligations` pour un arret) : son titre, ses cases ouvertes dans l'ordre ou elle les montre (chacune
/// sous la cle du champ de la charge du geste qui l'enregistre, avec sa forme, son libelle, son aide, ses choix, sa
/// valeur, ce que le moteur propose et ce qu'il dit sous elle), ce qu'elle dit sous ses cases (`dit`), et le geste qui
/// l'enregistre (sa route, sa charge deja remplie de ce qui est sur, son libelle ; les cases y vont chacune sous sa cle).
/// La page la redemande a chaque case changee, avec ce qui est saisi, et la redessine (`js/imprevus.js`).
#[derive(Debug, Serialize)]
pub(crate) struct SaisieDUnImprevu {
    titre: String,
    cases: Vec<CaseDeLaFiche<&'static str>>,
    dit: Vec<String>,
    /// Aucun quand il ne pourrait que refuser : aucun contrat ne couvre le jour, ou le salarie ne se lit pas (`dit` dit
    /// pourquoi, et la boite n'a qu'« Annuler »).
    #[serde(skip_serializing_if = "Option::is_none")]
    enregistrer: Option<EnregistrerLEcran<ChargeDeLaSaisie>>,
}

impl SaisieDUnImprevu {
    /// La boite titree `titre`, de ces cases, disant `dit`, enregistree par `route` avec la charge que `salarie` et `contrat`
    /// remplissent deja ; sans geste quand le salarie ne se lit pas (`salarie_illisible`) ou qu'aucun contrat ne couvre
    /// le jour.
    pub(crate) fn de(titre: String, cases: Vec<CaseDeLaFiche<&'static str>>, dit: Vec<String>, route: Route, salarie: Option<SalarieDesigne>, salarie_illisible: bool, contrat: &ContratDeLaSaisie) -> SaisieDUnImprevu {
        let ne_peut_que_refuser = salarie_illisible || matches!(contrat, ContratDeLaSaisie::Aucun(_));
        let charge = ChargeDeLaSaisie { salarie, contrat: contrat.porte() };
        let enregistrer = (!ne_peut_que_refuser).then(|| EnregistrerLEcran::a_plat(route, charge, ENREGISTRER));
        SaisieDUnImprevu { titre, cases, dit, enregistrer }
    }
}

/// Le libelle du geste qui enregistre une boite de saisie.
const ENREGISTRER: Affiche = Affiche::new("Enregistrer");

/// Ce que le geste qui enregistre un imprevu porte deja : le salarie, et le contrat suspendu quand la boite ne le demande
/// pas et qu'il se sait (celui de la carte d'ou l'on vient).
#[derive(Debug, Serialize)]
pub(crate) struct ChargeDeLaSaisie {
    #[serde(skip_serializing_if = "Option::is_none")]
    salarie: Option<SalarieDesigne>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contrat: Option<NumeroContrat>,
}

/// LE CONTRAT QU'UN IMPREVU EN SAISIE SUSPEND, tel que sa boite le traite (`contrat_de_la_saisie`).
pub(crate) enum ContratDeLaSaisie {
    /// Le jour n'est pas encore dit, ou un seul contrat le couvre : rien ne se demande, et le geste n'en porte pas (la
    /// route qui enregistre prend le seul, `contrat_suspendu`).
    RienADemander,
    /// Le contrat de la carte d'ou l'on vient, parmi ceux qui couvrent le jour : rien ne se demande, le geste le porte.
    LeSien(NumeroContrat),
    /// Plusieurs contrats couvrent le jour : la case qui le fait choisir, chacun son numero et ce qu'il dit (aucune case
    /// quand aucun n'a de numero) ; et, pour chaque contrat qui n'a pas encore de numero, la phrase qui le dit.
    AChoisir { choix: Vec<Choix>, sans_numero: Vec<String> },
    /// Aucun contrat ne couvre le jour : la phrase qui le dit (il n'y a rien a suspendre).
    Aucun(String),
}

/// La case qui fait choisir le contrat, son libelle.
const QUEL_CONTRAT: Affiche = Affiche::new("Quel contrat");

impl ContratDeLaSaisie {
    /// Ce que le geste qui enregistre porte deja : le sien.
    fn porte(&self) -> Option<NumeroContrat> {
        match self {
            ContratDeLaSaisie::LeSien(n) => Some(n.clone()),
            ContratDeLaSaisie::RienADemander | ContratDeLaSaisie::AChoisir { .. } | ContratDeLaSaisie::Aucun(_) => None,
        }
    }

    /// Le contrat que l'imprevu suspendrait, pour ce qui se lit avant l'enregistrement : le sien, ou celui qu'on a choisi
    /// (`choisi`) quand la boite le demande ; aucun sinon.
    pub(crate) fn voulu(&self, choisi: Option<&NumeroContrat>) -> Option<NumeroContrat> {
        match self {
            ContratDeLaSaisie::LeSien(n) => Some(n.clone()),
            ContratDeLaSaisie::AChoisir { .. } => choisi.cloned(),
            ContratDeLaSaisie::RienADemander | ContratDeLaSaisie::Aucun(_) => None,
        }
    }

    /// La case qui le fait choisir (sous la cle `cle`, montrant `choisi`), quand la boite le demande et qu'un contrat a
    /// de quoi se choisir (son numero).
    pub(crate) fn case(&self, cle: &'static str, choisi: Option<&NumeroContrat>) -> Option<CaseDeLaFiche<&'static str>> {
        match self {
            ContratDeLaSaisie::AChoisir { choix, .. } if !choix.is_empty() => {
                Some(CaseDeLaFiche::de_la_boite(cle, QUEL_CONTRAT, crate::catalogue::Forme::ChoixRendu, "", choisi.and_then(ValeurDeCase::de)).offrant(choix.clone()))
            }
            ContratDeLaSaisie::AChoisir { .. } | ContratDeLaSaisie::RienADemander | ContratDeLaSaisie::LeSien(_) | ContratDeLaSaisie::Aucun(_) => None,
        }
    }

    /// Ce que la boite en dit : qu'aucun contrat ne couvre le jour ; qu'un contrat a choisir n'a pas encore de numero.
    pub(crate) fn dit(&self) -> Option<String> {
        match self {
            ContratDeLaSaisie::Aucun(p) => Some(p.clone()),
            ContratDeLaSaisie::AChoisir { sans_numero, .. } if !sans_numero.is_empty() => Some(sans_numero.join(" ")),
            ContratDeLaSaisie::AChoisir { .. } | ContratDeLaSaisie::RienADemander | ContratDeLaSaisie::LeSien(_) => None,
        }
    }
}

/// LE CONTRAT QUE SUSPENDRAIT L'IMPREVU DU SALARIE `cle` QUI COMMENCE (ou dont le dernier jour travaille est) `jour`,
/// parmi ceux qui couvrent ce jour (`contrats_au`) : un seul, rien a demander ; celui de la carte d'ou l'on vient
/// (`carte_numero`, sinon son premier jour `carte_debut`) quand il a son numero, le sien ; plusieurs, a choisir ; aucun,
/// la phrase qui le dit. Sans salarie ni jour, rien a demander encore.
pub(crate) fn contrat_de_la_saisie(ctx: &Contexte, cle: Option<&IdSalarie>, jour: Option<Jour>, carte_numero: Option<&NumeroContrat>, carte_debut: Option<Jour>) -> R<ContratDeLaSaisie> {
    let (Some(cle), Some(jour)) = (cle, jour) else { return Ok(ContratDeLaSaisie::RienADemander) };
    let contrats = contrats_au(ctx, cle, jour)?;
    if contrats.is_empty() {
        return Ok(ContratDeLaSaisie::Aucun(format!("Aucun contrat de {} ne couvre le {} : il n'y a rien à suspendre.", ctx.nomme(cle), jour.jj_mm_aaaa())));
    }
    if contrats.len() == 1 {
        return Ok(ContratDeLaSaisie::RienADemander);
    }
    let sien = contrats.iter().find(|c| (carte_numero.is_some() && c.numero.as_ref() == carte_numero) || Some(c.periode.debut()) == carte_debut);
    if let Some(n) = sien.and_then(|c| c.numero.clone()) {
        return Ok(ContratDeLaSaisie::LeSien(n));
    }
    // Chaque contrat se dit en une ligne : son numero et sa periode ; un contrat sans numero ne se designe pas (la charge
    // le nomme par son numero), il ne s'offre pas : la boite dit qu'il recoit son numero a la validation de son
    // engagement (`engagement::numeros::a_la_validation`).
    let (mut choix, mut sans_numero) = (Vec::new(), Vec::new());
    for k in &contrats {
        let periode = match k.periode.fin() {
            Some(f) => format!("du {} au {}", k.periode.debut().jj_mm_aaaa(), f.jj_mm_aaaa()),
            None => format!("depuis le {}", k.periode.debut().jj_mm_aaaa()),
        };
        match &k.numero {
            Some(numero) => choix.push(Choix::de(numero, format!("{}, {}", numero, periode))),
            None => sans_numero.push(format!(
                "Le contrat de {} {} n'a pas encore de numéro : il le reçoit à la validation de son engagement. Validez-le, puis choisissez-le ici.",
                ctx.nomme(cle),
                periode
            )),
        }
    }
    Ok(ContratDeLaSaisie::AChoisir { choix, sans_numero })
}

/// Le titre d'une boite de saisie : ce qu'elle saisit, et pour qui (le nom du salarie, quand il se lit).
pub(crate) fn titre_de_la_saisie(ctx: &Contexte, quoi: &str, cle: Option<&IdSalarie>) -> String {
    match cle {
        Some(c) => format!("{} — {}", quoi, ctx.nomme(c)),
        None => quoi.to_string(),
    }
}

/// Le salarie d'une boite de saisie, lu : sa cle, ou la phrase qui dit pourquoi il ne se lit pas (la boite la dit, et
/// c'est la route qui enregistre qui le refusera).
pub(crate) fn salarie_de_la_saisie(ctx: &Contexte, salarie: Option<&SalarieDesigne>) -> (Option<IdSalarie>, Option<String>) {
    match salarie.map(|s| ctx.cle_du_salarie(s)) {
        Some(Ok(cle)) => (Some(cle), None),
        Some(Err(m)) => (None, Some(m.phrase)),
        None => (None, None),
    }
}

/// La note d'une boite de saisie : une case de texte, sous « Avancé », qui dit ce qu'elle est.
pub(crate) fn case_de_la_note(cle: &'static str, note: Option<&String>) -> CaseDeLaFiche<&'static str> {
    CaseDeLaFiche::de_la_boite(cle, NOTE, forme_du_type::<_, String>(), "", note.and_then(ValeurDeCase::de))
        .avancee()
        .disant(vec!["Une note pour vous : elle ne part nulle part.".to_string()])
}

const NOTE: Affiche = Affiche::new("Note");

/// `suspension-saisie` : la boite qui saisit une absence, et sa phrase.
#[derive(Debug, Serialize)]
struct ReponseSaisieDUneAbsence {
    phrase: Phrase,
    #[serde(flatten)]
    boite: SaisieDUnImprevu,
}

/// `suspension-saisie` : la boite qui saisit une absence, pour ce qui est deja dit. Ses cases : du, au, le contrat quand
/// plusieurs couvrent le premier jour (`contrat_de_la_saisie`), le motif (ceux qui se declarent, `motifs_de_suspension`),
/// la note ; ce qu'elle dit : qu'aucun contrat ne couvre le jour, et ce que fait une absence saisie. Rien n'est ecrit,
/// rien ne se refuse de l'absence elle-meme (c'est `suspension-enregistrer` qui refuse).
fn suspension_saisie(ctx: &Contexte, c: ChargeSuspensionSaisie) -> R<ReponseSaisieDUneAbsence> {
    let (cle, illisible) = salarie_de_la_saisie(ctx, c.salarie.as_ref());
    let contrat = contrat_de_la_saisie(ctx, cle.as_ref(), c.debut, c.carte_numero.as_ref(), c.carte_debut)?;
    let motifs = motifs_de_suspension()?.into_iter().map(|m| Choix::de(m.code, m.libelle_dit)).collect();
    let jour = |cle, libelle, v: Option<Jour>| CaseDeLaFiche::de_la_boite(cle, libelle, forme_du_type::<_, Jour>(), "", v.as_ref().and_then(ValeurDeCase::de));
    let mut cases = vec![
        jour(cle!(ChargeSuspensionEnregistrer, debut), Affiche::new("Du"), c.debut),
        jour(cle!(ChargeSuspensionEnregistrer, fin), Affiche::new("Au"), c.fin),
    ];
    cases.extend(contrat.case(cle!(ChargeSuspensionEnregistrer, contrat), c.contrat.as_ref()));
    cases.push(
        CaseDeLaFiche::de_la_boite(cle!(ChargeSuspensionEnregistrer, motif), Affiche::new("Motif"), forme_du_type::<_, MotifDeSuspension>(), "", c.motif.as_ref().and_then(ValeurDeCase::de))
            .offrant(motifs),
    );
    cases.push(case_de_la_note(cle!(ChargeSuspensionEnregistrer, note), c.note.as_ref()));
    let salarie_illisible = illisible.is_some();
    let dit: Vec<String> = [illisible, contrat.dit(), Some("Saisie une fois : chaque mois qu'elle traverse la déclare et, si elle n'est pas payée, en retire le salaire.".to_string())]
        .into_iter()
        .flatten()
        .collect();
    let titre = titre_de_la_saisie(ctx, "Une absence", cle.as_ref());
    let phrase = socle::phrase!("{} : {} cases.", titre, cases.len());
    Ok(ReponseSaisieDUneAbsence { phrase, boite: SaisieDUnImprevu::de(titre, cases, dit, Route::SuspensionEnregistrer, c.salarie, salarie_illisible, &contrat) })
}

/// Ce que rend un geste sur un arret : l'arret resume, et ce qui se lit du geste ; un arret qu'on vient d'enregistrer, ce
/// qu'il appelle encore (`EncoreAppele`).
#[derive(Debug, Serialize)]
struct ReponseArret {
    arret: Resume,
    phrase: Phrase,
    #[serde(flatten)]
    encore: EncoreAppele,
}

/// Ce que rend un geste sur une absence ; une absence qu'on vient d'enregistrer, ce qu'elle appelle encore.
#[derive(Debug, Serialize)]
struct ReponseSuspension {
    suspension: Resume,
    phrase: Phrase,
    #[serde(flatten)]
    encore: EncoreAppele,
}

/// CE QU'UN IMPREVU QU'ON VIENT D'ENREGISTRER APPELLE ENCORE, quand il appelle quelque chose (le geste de ses indemnites
/// journalieres, `a_dire` ; les jours que la saisie paie pendant lui, `cachets`) : le titre de la page qui le dit
/// (`appelle_encore`), ce qu'elle dit au-dessus des jours payes, quand il y en a (`jours_dit`), le mot qui la laisse
/// pour plus tard (`plus_tard`) et celui qui enregistre ce qu'on y a dit (`enregistrer`) ; rien n'est obligatoire. Rien
/// d'appele : aucun, et rien ne suit.
#[derive(Debug, Default, Serialize)]
struct EncoreAppele {
    #[serde(skip_serializing_if = "Option::is_none")]
    appelle_encore: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jours_dit: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plus_tard: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enregistrer: Option<&'static str>,
}

impl EncoreAppele {
    /// Ce que l'imprevu `r`, qu'on vient d'enregistrer, appelle encore.
    fn de(r: &Resume) -> EncoreAppele {
        let (a_dire, cachets) = match r {
            Resume::Arret(a) => (a.a_dire.is_some(), !a.cachets.is_empty()),
            Resume::Suspension(s) => (false, !s.cachets.is_empty()),
        };
        if !a_dire && !cachets {
            return EncoreAppele::default();
        }
        EncoreAppele {
            appelle_encore: Some("Ce qu'il appelle encore"),
            jours_dit: cachets.then_some("Des jours sont payés pendant l'imprévu : cocher ceux à retirer de l'engagement."),
            plus_tard: Some("Plus tard"),
            enregistrer: Some(ENREGISTRER.texte()),
        }
    }
}

/// `arret-signaler` : le fichier ecrit, et son depot (`suite`).
#[derive(Debug, Serialize)]
pub(crate) struct ReponseSignalement {
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    fichier: std::path::PathBuf,
    phrase: Phrase,
    nature: NatureSignalement,
    #[serde(rename = "type")]
    type_declaration: TypeDeLaDeclaration,
    #[serde(with = "droit::natures::en_nombre")]
    ordre: NumeroDOrdre,
    avertissements: Vec<String>,
    suite: crate::etapes::Action,
}

/// CE QUI SUIT L'ECRITURE D'UN SIGNALEMENT : son depot (l'apercu du depot du fichier ecrit, puis le depot), le geste que
/// la reponse rend en `suite`. Ecrire un fichier n'engage rien ; le deposer engage tout : deux gestes.
fn le_depot(fichier: &std::path::Path) -> crate::etapes::Action {
    crate::etapes::vers(crate::etapes::VueDuGeste::Depot, crate::depots::Designation::du_fichier(fichier.to_path_buf()), Some(Route::Deposer), "Déposer".into())
}

/// `fin-contrat-signaler` : le fichier ecrit, et son depot (`suite`).
#[derive(Debug, Serialize)]
pub(crate) struct ReponseSignalementFin {
    suite: crate::etapes::Action,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    fichier: std::path::PathBuf,
    phrase: Phrase,
    #[serde(rename = "type")]
    type_declaration: TypeDeLaDeclaration,
    #[serde(with = "droit::natures::en_nombre")]
    ordre: NumeroDOrdre,
    preavis: String,
    avertissements: Vec<String>,
}

/// `fin-contrat-documents` : les deux papiers ecrits.
#[derive(Debug, Serialize)]
struct ReponsePapiers {
    phrase: Phrase,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    certificat: std::path::PathBuf,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    recu: std::path::PathBuf,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    dossier: std::path::PathBuf,
    avertissements: &'static [&'static str],
}

/// Ce que chaque route de ce module rend porte sa phrase.
macro_rules! dites {
    ($($t:ty),*) => {$(
        impl Dite for $t {
            fn phrase(&self) -> &Phrase {
                &self.phrase
            }
        }
    )*};
}
dites!(ReponseImprevus, ReponseArrets, ReponseFins, ReponseSaisieDUneAbsence, ReponseArret, ReponseSuspension, ReponseSignalement, ReponseSignalementFin, ReponsePapiers);

/// Une route de ce module.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::Imprevus => en_json(imprevus(ctx, charge.lire()?)?),
        Route::Arrets => {
            let SansCharge {} = charge.lire()?;
            en_json(arrets(ctx)?)
        }
        Route::FinsDeContrat => en_json(fins_de_contrat(ctx, charge.lire()?)?),
        Route::SuspensionSaisie => en_json(suspension_saisie(ctx, charge.lire()?)?),
        Route::ArretEnregistrer => {
            let c: ChargeArretEnregistrer = charge.lire()?;
            let mut r = arret_enregistrer(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            r.encore = EncoreAppele::de(&r.arret);
            en_json(r)
        }
        Route::ArretReprise => {
            let c: ChargeArretReprise = charge.lire()?;
            let mut r = arret_reprise(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::ArretOublier => {
            let c: ChargeArretOublier = charge.lire()?;
            let mut r = arret_oublier(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::ArretIndemnites => {
            let c: ChargeArretIndemnites = charge.lire()?;
            let mut r = arret_indemnites(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::ArretProlonger => {
            let c: ChargeArretProlonger = charge.lire()?;
            let mut r = arret_prolonger(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::ArretSignaler => en_json(signaler_l_arret(ctx, &charge.lire()?, None)?),
        Route::SuspensionEnregistrer => {
            let c: ChargeSuspensionEnregistrer = charge.lire()?;
            let mut r = suspension_enregistrer(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            r.encore = EncoreAppele::de(&r.suspension);
            en_json(r)
        }
        Route::SuspensionModifier => {
            let c: ChargeSuspensionModifier = charge.lire()?;
            let mut r = suspension_modifier(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::SuspensionOublier => {
            let c: ChargeSuspensionOublier = charge.lire()?;
            let mut r = suspension_oublier(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
            en_json(r)
        }
        Route::FinContratSignaler => en_json(signaler_la_fin(ctx, &charge.lire()?, false)?),
        Route::FinContratDocuments => en_json(fin_contrat_documents(ctx, charge.lire()?)?),
        autre => Err(route_inconnue(autre.nom())),
    }
}

/// APRES L'ECRITURE D'UN IMPREVU : les engagements du salarie dont des documents sont partis se revalident
/// (`engagement::etats::apres_un_imprevu`) ; une paie partie qui change suit le chemin de toute correction d'un mois parti,
/// et la phrase dit ce qu'il entraine, avec les mots de « Modifier l'engagement »
/// (`crate::engagement::ce_que_le_changement_entraine`).
fn regulariser(ctx: &Contexte, salarie: &SalarieDesigne, phrase: &mut Phrase) -> R<()> {
    use engagement::etats::Revalidation;
    let cle = cle_du_nom(ctx, salarie)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let mut dit = Vec::new();
    for x in engagement::etats::apres_un_imprevu(ctx, &cle, &departs)? {
        match x {
            Revalidation::Changee(v) => {
                let lignes = crate::engagement::ce_que_le_changement_entraine(ctx, &v, &departs)?;
                if !lignes.is_empty() {
                    dit.push(format!("La paie partie change : {}.", lignes.join(", ")));
                }
            }
            Revalidation::Attend(_, refus) => dit.push(format!("La paie partie attend : {}", refus.phrase)),
        }
    }
    if !dit.is_empty() {
        *phrase = socle::phrase!("{} {}", phrase, dit.join(" "));
    }
    Ok(())
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}


/// Ce qu'une phrase dit d'un contrat qui n'a pas de numero (`engagement::numeros::numero_dit`).
const SANS_NUMERO: &str = "sans numéro";

/// La cle de la fiche que `salarie` designe, comme partout dans le moteur (`Contexte::cle_du_salarie` : la casse, les
/// accents, le prenom, le NIR ne comptent pas). Un nom qui ne designe personne se refuse avec sa phrase.
fn cle_du_nom(ctx: &Contexte, salarie: &SalarieDesigne) -> R<IdSalarie> {
    ctx.cle_du_salarie(salarie).map_err(|m| m.reclasser(Classe::Manque))
}

/// LE CONTRAT QU'UN IMPREVU SUSPEND : parmi ceux du salarie qui couvrent `jour` (`contrats_au`), celui que la charge
/// nomme (`voulu`, son numero), sinon le seul. Un imprevu designe son contrat par son numero et sa date de debut
/// (S21.G00.40.009 et .001) ; sans contrat, il n'y a rien a suspendre, et a deux, le cahier technique laisse le choix au
/// declarant : on le lui demande.
pub(crate) fn contrat_suspendu(ctx: &Contexte, cle: &IdSalarie, jour: &Jour, voulu: Option<&NumeroContrat>) -> R<ContratAuJour> {
    let candidats = contrats_au(ctx, cle, *jour)?;
    if let Some(voulu) = voulu {
        return match candidats.iter().find(|c| c.numero.as_ref() == Some(voulu)) {
            Some(c) => Ok(c.clone()),
            None => {
                let liste: Vec<String> = candidats.iter().map(|c| numero_dit(c.numero.as_ref(), "", "", SANS_NUMERO)).collect();
                Err(refus(format!(
                    "Contrat {} introuvable parmi ceux de {} qui couvrent le {} : {}",
                    voulu,
                    cle,
                    jour.jj_mm_aaaa(),
                    if liste.is_empty() { "aucun".to_string() } else { liste.join(", ") }
                )))
            }
        };
    }
    match candidats.len() {
        1 => Ok(candidats[0].clone()),
        0 => Err(refus(format!(
            "Aucun contrat de {} ne couvre le {}. Un arret ou une suspension designe le contrat suspendu par son numero et sa \
date de debut (S21.G00.40.009 et .001) : sans contrat declare, il n'y a rien a suspendre. Verifier la date, ou saisir le \
contrat.",
            cle,
            jour.jj_mm_aaaa()
        ))),
        _ => {
            let liste: Vec<String> = candidats
                .iter()
                .map(|c| match c.periode.fin() {
                    Some(f) => format!("{} du {} au {}", numero_dit(c.numero.as_ref(), "", "", SANS_NUMERO), c.periode.debut().iso(), f.iso()),
                    None => format!("{} depuis le {}", numero_dit(c.numero.as_ref(), "", "", SANS_NUMERO), c.periode.debut().iso()),
                })
                .collect();
            Err(refus(format!(
                "Deux contrats de {} couvrent le {} ({}). Le cahier technique laisse le choix au declarant : dire lequel.",
                cle,
                jour.jj_mm_aaaa(),
                liste.join(", ")
            )))
        }
    }
}

/// UN IMPREVU NE CHEVAUCHE PAS UN AUTRE IMPREVU DU MEME CONTRAT : un jour d'absence ne se retire qu'une fois du salaire,
/// et deux periodes qui se recouvrent sur le meme contrat sont deux declarations qui se contredisent (un salarie ne
/// travaille pas le dernier jour travaille d'un arret qui court deja). `sauf` : l'imprevu lui-meme, quand il change.
fn sans_chevauchement(ctx: &Contexte, nouveau: &Imprevu, sauf: Option<&Imprevu>) -> R<()> {
    // Un imprevu sans jour d'absence n'en retire aucun.
    let Some(le_sien) = magasin(imprevus::absence(nouveau))? else { return Ok(()) };
    let cle = nouveau.salarie();
    let debut = nouveau.contrat_debut();
    let numero = nouveau.contrat_numero();
    for autre in magasin(imprevus::tous(&ctx.racine))? {
        let lui_meme = sauf.map(|s| meme_imprevu(s, &autre)).transpose()?.is_some_and(|m| m);
        if lui_meme || meme_imprevu(nouveau, &autre)? {
            continue;
        }
        if !magasin(imprevus::du_contrat(&autre, cle, numero, &debut))? {
            continue;
        }
        let Some(l_autre) = magasin(imprevus::absence(&autre))? else { continue };
        if le_sien.croise(&l_autre).is_some() {
            return Err(refus(format!(
                "{} : du {} au {}, cet imprevu chevauche {} du {} au {} sur le meme contrat ({}). Un jour d'absence ne se \
declare et ne se retire qu'une fois : corriger les dates, ou reprendre l'autre imprevu.",
                cle,
                le_sien.debut().jj_mm_aaaa(),
                le_sien.fin().jj_mm_aaaa(),
                if imprevus::est_un_arret(&autre) { "l'arret de travail" } else { "la suspension" },
                l_autre.debut().jj_mm_aaaa(),
                l_autre.fin().jj_mm_aaaa(),
                match numero {
                    Some(n) => n.to_string(),
                    None => debut.jj_mm_aaaa(),
                }
            )));
        }
    }
    Ok(())
}

/// Le meme imprevu : meme genre, meme salarie, meme jour de rangement.
fn meme_imprevu(a: &Imprevu, b: &Imprevu) -> R<bool> {
    Ok(a.genre() == b.genre() && a.salarie() == b.salarie() && magasin(imprevus::jour_de_rangement(a))? == magasin(imprevus::jour_de_rangement(b))?)
}

/// Le fichier du magasin du mois de `jour` : tous les imprevus qu'il range.
fn le_fichier(ctx: &Contexte, jour: &Jour) -> R<Vec<Imprevu>> {
    magasin(imprevus::du_fichier(&ctx.racine, jour.mois_civil()))
}

/// L'arret de ce salarie a ce dernier jour travaille : son indice, le fichier de son mois (tous imprevus compris), et
/// l'arret.
fn trouver_arret(ctx: &Contexte, cle: &IdSalarie, dernier: &Jour) -> R<Option<(usize, Vec<Imprevu>, ArretSaisi)>> {
    let liste = le_fichier(ctx, dernier)?;
    let mut trouve = None;
    for (i, x) in liste.iter().enumerate() {
        if let Imprevu::Arret(a) = x {
            if a.salarie == *cle && a.dernier_jour_travaille == *dernier {
                trouve = Some((i, a.clone()));
                break;
            }
        }
    }
    Ok(trouve.map(|(i, a)| (i, liste, a)))
}

/// La suspension de ce salarie qui commence ce jour : son indice, le fichier de son mois, et la suspension.
fn trouver_suspension(ctx: &Contexte, cle: &IdSalarie, debut: &Jour) -> R<Option<(usize, Vec<Imprevu>, SuspensionSaisie)>> {
    let liste = le_fichier(ctx, debut)?;
    let mut trouve = None;
    for (i, x) in liste.iter().enumerate() {
        if let Imprevu::Suspension(s) = x {
            if s.salarie == *cle && s.debut == *debut {
                trouve = Some((i, s.clone()));
                break;
            }
        }
    }
    Ok(trouve.map(|(i, s)| (i, liste, s)))
}

/// L'arret de ce salarie a ce dernier jour travaille, ou le refus qui dit ou les lire.
fn l_arret(ctx: &Contexte, cle: &IdSalarie, dernier: &Jour) -> R<(usize, Vec<Imprevu>, ArretSaisi)> {
    trouver_arret(ctx, cle, dernier)?.ok_or_else(|| {
        refus(format!("Aucun arret de {} dont le dernier jour travaille soit le {} : choisir un arret de la liste.", cle, dernier.jj_mm_aaaa()))
    })
}

/// La suspension de ce salarie qui commence ce jour, ou le refus.
fn la_suspension(ctx: &Contexte, cle: &IdSalarie, debut: &Jour) -> R<(usize, Vec<Imprevu>, SuspensionSaisie)> {
    trouver_suspension(ctx, cle, debut)?.ok_or_else(|| refus(format!("Aucune suspension de {} qui commence le {}.", cle, debut.jj_mm_aaaa())))
}

/// Ecrit le fichier du mois de `jour` du magasin des imprevus (`imprevus/AAAA-MM.json`) : les arrets ET les suspensions
/// qu'il range, tels que la liste les donne.
fn ecrire(ctx: &Contexte, jour: &Jour, liste: Vec<Imprevu>) -> R<()> {
    imprevus::ecrire(&ctx.racine, jour.mois_civil(), liste).map_err(|m| m.reclasser(Classe::Disque))
}

/// Ce qui est parti d'un imprevu (`obligations::ce_qui_est_parti` : les envois du journal, tels qu'il les lit).
fn ses_envois(ctx: &Contexte, imp: &Imprevu) -> R<Vec<crate::obligations::EnvoiDeLArret>> {
    crate::obligations::ce_qui_est_parti(ctx, imp)
}

/// Ce qui est parti d'un arret.
fn envois_de_l_arret(ctx: &Contexte, a: &ArretSaisi) -> R<Vec<crate::obligations::EnvoiDeLArret>> {
    ses_envois(ctx, &Imprevu::Arret(a.clone()))
}

/// Les mois des DSN mensuelles qui ont reporte l'imprevu (nature 01).
fn reporte_par(envois: &[crate::obligations::EnvoiDeLArret]) -> Vec<Mois> {
    envois.iter().filter(|x| x.nature_declaration == NatureDeLaDeclaration::Mensuelle).filter_map(|x| x.mois).collect()
}

/// Un signalement de cette nature (04, 05) est-il parti pour cet arret ?
fn envoye(ctx: &Contexte, a: &ArretSaisi, nature: NatureSignalement) -> R<bool> {
    Ok(envois_de_l_arret(ctx, a)?.iter().any(|x| x.nature_declaration == nature.nature()))
}

/// Le jour ou le signalement NORMAL (type 01) de cette nature (04, 05) est parti pour cet arret, s'il l'est
/// (`obligations::ce_qui_est_parti` : un depot reel accepte, que le bilan n'a pas refuse).
fn parti_le(ctx: &Contexte, a: &ArretSaisi, nature: NatureSignalement) -> R<Option<Jour>> {
    let envois = envois_de_l_arret(ctx, a)?;
    Ok(envois.iter().filter(|x| x.nature_declaration == nature.nature() && x.type_declaration == Some(TypeDeLaDeclaration::Normale)).map(|x| x.quand).min().map(|q| q.jour()))
}

/// Le nom d'un signalement : `<base>.dsn` ; un annule-et-remplace, `<base>_AR<ordre>.dsn`, a cote du signalement parti
/// qu'il remplace, qui reste tel qu'il est parti (regle 4).
fn nom_du_signalement(base: &str, annule_et_remplace: bool, ordre: NumeroDOrdre) -> R<NomDeFichier> {
    NomDeFichier::lire(&if annule_et_remplace { format!("{}_AR{}.dsn", base, ordre) } else { format!("{}.dsn", base) })
}

/// UN ARRET EN COURS : celui dont on peut encore apprendre qu'il se prolonge ou que le salarie a repris plus
/// tot. Aucune reprise n'est saisie, et :
///
/// - sa fin previsionnelle n'est pas passee : le salarie est arrete aujourd'hui ;
/// - ou elle est passee, mais la DSN mensuelle du mois ou elle tombe n'est pas encore partie : l'avis de prolongation peut
///   arriver apres la fin prescrite, et une reprise anticipee se signale encore. Une fois cette DSN partie, ou la DSN
///   suivante qui reporte un arret saisi apres elle (`reporte_par`),
///   l'arret y est declare fini a sa fin previsionnelle -- « la reprise du travail de votre salarie a la date prevue doit
///   etre signalee dans la DSN mensuelle » (ameli, `atelier/reference/sources/ameli-arret-de-travail/`) --, et il n'est
///   plus en cours.
///
/// Les routes elles-memes ne s'en servent pas pour refuser : une prolongation se saisit tant qu'aucune reprise ne l'est
/// (`arret_prolonger`). C'est ce que la page offre, pas ce que le moteur admet.
fn en_cours(ctx: &Contexte, a: &ArretSaisi) -> R<bool> {
    if a.reprise.is_some() {
        return Ok(false);
    }
    let fin = a.fin_previsionnelle;
    if fin >= ctx.aujourdhui {
        return Ok(true);
    }
    Ok(!reporte_par(&envois_de_l_arret(ctx, a)?).iter().any(|m| *m >= fin.mois_civil()))
}

// ------------------------------------------------------------------ le resume

/// UN IMPREVU TEL QUE LA PAGE LE LIT : un arret (`ResumeDArret`) ou une absence (`ResumeDeSuspension`).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum Resume {
    Arret(Box<ResumeDArret>),
    Suspension(Box<ResumeDeSuspension>),
}

/// UN ARRET RESUME, tel que le lisent la liste des imprevus et la DSN qui le reporte (`gestes::depart`) : ses champs
/// tels que l'arret les ecrit, ce qui en est parti (`signalements` : 04 et 05 ; `reporte_par` : les mois des DSN
/// mensuelles), et l'etat de son signalement.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ArretResume {
    salarie: IdSalarie,
    contrat: Option<engagement::numeros::NumeroContrat>,
    contrat_debut: Jour,
    motif: MotifDArret,
    pub motif_libelle: String,
    dernier_jour_travaille: Jour,
    debut: Jour,
    fin_previsionnelle: Jour,
    subrogation: Option<SubrogationDeLArret>,
    periode_de_subrogation: Option<Periode>,
    date_accident: Option<Jour>,
    reprise: Option<Jour>,
    reprise_motif: Option<MotifDeReprise>,
    note: Option<String>,
    #[serde(with = "socle::paris::ecrit::saisie")]
    saisi_le: socle::paris::Moment,
    /// Un signalement en est parti : `etat` le dit a la page.
    #[serde(skip)]
    pub signale: bool,
    signalement_du: bool,
    signalements: Vec<crate::obligations::EnvoiDeLArret>,
    reporte_par_dit: Vec<String>,
    /// Les mois des DSN qui l'ont reporte : `reporte_par_dit` les dit a la page.
    #[serde(skip)]
    reporte_par: Vec<Mois>,
    etat: EtatDuSignalement,
}

/// Ou en est le signalement d'un arret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum EtatDuSignalement {
    #[serde(rename = "signale")]
    Signale,
    #[serde(rename = "a signaler")]
    ASignaler,
    #[serde(rename = "sans signalement")]
    SansSignalement,
}

impl EtatDuSignalement {
    /// Tel que l'ecran le dit.
    fn dit(self) -> &'static str {
        match self {
            EtatDuSignalement::Signale => "signalé",
            EtatDuSignalement::ASignaler => "à signaler",
            EtatDuSignalement::SansSignalement => "sans signalement",
        }
    }
}

/// UN ARRET TEL QUE LA PAGE LE LIT : le resume de toujours (`ArretResume`), plus le nom de son salarie (`qui`), `genre`,
/// `premier_jour`, `dernier_jour`, `jours_deja_indemnises`, les indemnites par jour (`ij_brute`, `ij_nette`,
/// `ij_brute_majoree`, `ij_nette_majoree`), `origine`, `cachets`, ses `gestes` et `a_dire` (ce qu'il ouvre a la page), ses
/// phrases (`Dit`), puis, dans les listes, ses `obligations` et ce qui en est `parti`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResumeDArret {
    #[serde(flatten)]
    base: ArretResume,
    qui: String,
    genre: GenreDImprevu,
    premier_jour: Jour,
    dernier_jour: Jour,
    #[serde(serialize_with = "socle::date::Duree::ecrite_en_jours_opt")]
    jours_deja_indemnises: Option<Duree<EnJours>>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    ij_brute: Option<MontantUnitaire>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    ij_nette: Option<MontantUnitaire>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    ij_brute_majoree: Option<MontantUnitaire>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    ij_nette_majoree: Option<MontantUnitaire>,
    origine: Option<droit::maintien::Origine>,
    cachets: Vec<Cachet>,
    prolongations: Vec<Prolongation>,
    gestes: Vec<GesteDeRoute<Designe>>,
    /// Ce que l'arret ecrit appelle encore : ses indemnites journalieres, quand il ouvre le maintien (le meme geste que
    /// dans `gestes`).
    a_dire: Option<GesteDeRoute<Designe>>,
    #[serde(flatten)]
    dit: Dit,
    /// Ses obligations, une fois posees (`Resume::poser_obligations`) ; aucune avant.
    obligations: Vec<crate::obligations::Obligation>,
    /// Ce qui en est parti, une fois dit (`Resume::dire_ce_qui_est_parti`) ; rien avant.
    parti: Vec<Parti>,
}

impl ResumeDArret {
    /// Le salarie de l'arret (la cle de sa fiche).
    pub(crate) fn salarie(&self) -> &IdSalarie {
        &self.base.salarie
    }

    /// Le dernier jour travaille saisi, qui designe l'arret aux routes.
    pub(crate) fn dernier_jour_travaille(&self) -> Jour {
        self.base.dernier_jour_travaille
    }

    /// « Arrêt de travail (accident du travail), du 02/11/2026 au 13/11/2026 » : ce qu'est l'arret, avec le libelle de son
    /// motif, et sa periode.
    pub(crate) fn en_clair(&self) -> String {
        format!("Arrêt de travail ({}), {}", self.base.motif_libelle.to_lowercase(), self.dit.periode)
    }

    /// Ce qui est parti de l'arret (`ce_qui_est_parti`), une fois ses obligations posees (un arret sans obligations
    /// posees n'en dit aucune).
    fn dire_ce_qui_est_parti(&mut self) -> R<()> {
        self.parti = ce_qui_est_parti(&self.prolongations, &self.base.signalements, &self.base.reporte_par_dit, &self.obligations)?;
        Ok(())
    }
}

/// UNE ABSENCE TELLE QUE LA PAGE LA LIT : `genre`, `salarie` et son nom (`qui`), `contrat`, `contrat_debut`, `motif`,
/// `motif_libelle`, `remunere`, `debut`, `fin`, `premier_jour`, `dernier_jour`, `note`, `saisi_le`, `reporte_par`,
/// `cachets`, ses phrases (`Dit`), puis, dans la liste, ce qui en est `parti`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResumeDeSuspension {
    genre: GenreDImprevu,
    salarie: IdSalarie,
    qui: String,
    contrat: Option<engagement::numeros::NumeroContrat>,
    contrat_debut: Jour,
    motif: MotifDeSuspension,
    motif_libelle: String,
    remunere: bool,
    debut: Jour,
    fin: Jour,
    premier_jour: Jour,
    dernier_jour: Jour,
    note: Option<String>,
    #[serde(with = "socle::paris::ecrit::saisie")]
    saisi_le: socle::paris::Moment,
    reporte_par_dit: Vec<String>,
    cachets: Vec<Cachet>,
    gestes: Vec<GesteDeRoute<Designe>>,
    #[serde(flatten)]
    dit: Dit,
    /// Ce qui en est parti, une fois dit (`Resume::dire_ce_qui_est_parti`) ; rien avant.
    parti: Vec<Parti>,
}

/// UN GESTE QU'UNE ROUTE REND, que la page dessine en bouton et branche (`js/avance.js`, `brancherLesGestes`) : sa
/// route, sa charge (`C` : ce qui designe un imprevu, une question d'identite de l'import...), son libelle, et ce que sa
/// boite fait saisir : les cases de la fiche de sa route qu'elle montre (`cases` ; aucune : le geste part tel quel), ce
/// qu'elles montrent d'abord (`valeurs`, `V` : celles d'un imprevu par defaut), les choix d'une case qui se choisit
/// (`choix`, `X`) et ce que la boite dit au-dessus (`phrases` ; un geste sans case qui en dit ouvre sa boite, pour les
/// dire avant de partir : une confirmation). `demande` : le mot de passe du logiciel que le geste exige, que la page fait
/// taper dans sa boite du mot de passe avant de partir (`js/boites.js`, `demanderMotDePasse`) ; aucune : il n'en exige
/// pas. `rang` : s'il est le geste principal de son ecran, que la page dessine en bouton principal (`RangDuGeste`). Le
/// moteur ne rend que ceux qu'il ouvre.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct GesteDeRoute<C, V = ValeursDeDepart, X = ChoixDesCases> {
    route: Route,
    charge: C,
    libelle: &'static str,
    rang: RangDuGeste,
    cases: Vec<&'static str>,
    valeurs: V,
    choix: X,
    phrases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    demande: Option<compagnie::demande::DemandeDuMotDePasse>,
    /// Les parties d'une case faite de parties (`catalogue::Forme::Parties`) que la boite fait saisir, sous la cle de sa
    /// case, quand la route les rend avec le geste (une date par annee de la regularisation CASC-SVP) ; aucune : rien.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    parties: BTreeMap<&'static str, Vec<crate::fiches::PartieDeLaFiche>>,
    /// LA LECTURE QUI MONTRE CE QUE LE GESTE FERAIT, tant que ses cases se saisissent (sa fiche : `Ecran::Lecture`) :
    /// elle recoit la charge en cours, celle que le geste enverrait, et rend sa phrase et l'image qu'elle montre ; la
    /// boite la pose sous les cases a chaque case changee (`js/avance.js`, `montrerLApercu`). Aucune : rien a montrer.
    #[serde(skip_serializing_if = "Option::is_none")]
    apercu: Option<Route>,
}

impl<C, V, X> GesteDeRoute<C, V, X> {
    /// Le geste dont la boite fait saisir les cases `cases` de la fiche de sa route, montrant d'abord `valeurs`, et
    /// offrant `choix` aux cases qui se choisissent.
    pub(crate) fn avec_cases(route: Route, charge: C, libelle: Affiche, cases: Vec<&'static str>, valeurs: V, choix: X) -> GesteDeRoute<C, V, X> {
        GesteDeRoute { route, charge, libelle: libelle.texte(), rang: RangDuGeste::Second, cases, valeurs, choix, phrases: Vec::new(), demande: None, parties: BTreeMap::new(), apercu: None }
    }

    /// Le meme geste, sa boite montrant sous ses cases ce que la lecture `apercu` rend de la charge en cours.
    pub(crate) fn montrant(self, apercu: Route) -> GesteDeRoute<C, V, X> {
        GesteDeRoute { apercu: Some(apercu), ..self }
    }

    /// Le meme geste, le geste principal de son ecran (« Importer »).
    pub(crate) fn principal(self) -> GesteDeRoute<C, V, X> {
        GesteDeRoute { rang: RangDuGeste::Principal, ..self }
    }

    /// Le meme geste, sa case `cle` faite des parties `parties`.
    pub(crate) fn en_parties(mut self, cle: &'static str, parties: Vec<crate::fiches::PartieDeLaFiche>) -> GesteDeRoute<C, V, X> {
        self.parties.insert(cle, parties);
        self
    }

    /// Le meme geste, sa boite disant `phrases` au-dessus de ses cases.
    pub(crate) fn disant(self, phrases: Vec<String>) -> GesteDeRoute<C, V, X> {
        GesteDeRoute { phrases, ..self }
    }

    /// Le meme geste, exigeant le mot de passe du logiciel que `demande` dit (aucune : il n'en exige pas).
    pub(crate) fn exigeant(self, demande: Option<compagnie::demande::DemandeDuMotDePasse>) -> GesteDeRoute<C, V, X> {
        GesteDeRoute { demande, ..self }
    }
}

/// LE RANG D'UN GESTE DANS SON ECRAN : le geste principal (« Importer », « Reprendre »), que la page dessine en bouton
/// principal ; un geste second, en lien ; ou un geste second dessine en bouton secondaire, a cote du principal
/// (« Annuler » de la carte d'un engagement ouverte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RangDuGeste {
    Principal,
    Second,
    Secondaire,
}

impl RangDuGeste {
    /// Le geste principal si `principal`, un geste second sinon.
    pub(crate) fn principal_si(principal: bool) -> RangDuGeste {
        if principal { RangDuGeste::Principal } else { RangDuGeste::Second }
    }
}

/// Ce que les cases d'un geste montrent d'abord : ce que l'imprevu dit deja, ou ce que le moteur propose.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct ValeursDeDepart {
    #[serde(skip_serializing_if = "Option::is_none")]
    fin: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reprise: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    motif_reprise: Option<MotifDeReprise>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    ij_brute: Option<MontantUnitaire>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    ij_nette: Option<MontantUnitaire>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    ij_brute_majoree: Option<MontantUnitaire>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    ij_nette_majoree: Option<MontantUnitaire>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::date::Duree::ecrite_en_jours_opt")]
    jours_deja_indemnises: Option<Duree<EnJours>>,
}

/// Les choix d'une case d'un geste qui se choisit dans une liste : le motif d'une reprise (nomenclature du bloc 60).
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct ChoixDesCases {
    #[serde(skip_serializing_if = "Option::is_none")]
    motif_reprise: Option<Vec<Choisi>>,
}

/// Un choix : le code que la charge porte, le libelle que la liste montre.
#[derive(Debug, Clone, Serialize)]
struct Choisi {
    code: MotifDeReprise,
    libelle: String,
}

/// Ce qui designe un imprevu aux routes : un arret, son salarie et son dernier jour travaille ; une absence, son salarie
/// et son premier jour.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum Designe {
    Arret { salarie: IdSalarie, dernier_jour: Jour },
    Absence { salarie: IdSalarie, debut: Jour },
}

// Les libelles des gestes, en constantes : un texte fautif ne compile pas (`Affiche::new`).
const PROLONGER: Affiche = Affiche::new("Prolonger l'arrêt");
const REPRISE_PLUS_TOT: Affiche = Affiche::new("Reprise plus tôt");
const CORRIGER_LA_REPRISE: Affiche = Affiche::new("Corriger la reprise");
const INDEMNITES_DITES: Affiche = Affiche::new("Indemnités journalières");
const INDEMNITES_A_DIRE: Affiche = Affiche::new("Indemnités journalières…");
const CHANGER_LA_FIN: Affiche = Affiche::new("Changer la fin");
const RETIRER: Affiche = Affiche::new("Retirer");

impl<C> GesteDeRoute<C> {
    /// Le geste, sans case a saisir : il part avec sa charge.
    pub(crate) fn sans_cases(route: Route, charge: C, libelle: Affiche) -> GesteDeRoute<C> {
        GesteDeRoute {
            route,
            charge,
            libelle: libelle.texte(),
            rang: RangDuGeste::Second,
            cases: Vec::new(),
            valeurs: ValeursDeDepart::default(),
            choix: ChoixDesCases::default(),
            phrases: Vec::new(),
            demande: None,
            parties: BTreeMap::new(),
            apercu: None,
        }
    }
}

impl Designe {
    /// Le geste, sans case a saisir : il part avec sa charge.
    fn geste(&self, route: Route, libelle: Affiche) -> GesteDeRoute<Designe> {
        GesteDeRoute::sans_cases(route, self.clone(), libelle)
    }
}

/// La reprise d'un arret, a saisir : son jour et son motif, ceux que l'arret dit deja.
fn geste_de_reprise(d: &Designe, a: &ArretSaisi, n: &Bloc60, libelle: Affiche) -> GesteDeRoute<Designe> {
    let motifs = n.motifs_reprise.saisissables().into_iter().map(|m| Choisi { code: m.code, libelle: m.libelle }).collect();
    GesteDeRoute {
        cases: vec![cle!(ChargeArretReprise, reprise), cle!(ChargeArretReprise, motif_reprise)],
        valeurs: ValeursDeDepart { reprise: a.reprise.clone(), motif_reprise: a.reprise_motif, ..ValeursDeDepart::default() },
        choix: ChoixDesCases { motif_reprise: Some(motifs) },
        ..d.geste(Route::ArretReprise, libelle)
    }
}

/// LES INDEMNITES JOURNALIERES D'UN ARRET QUI OUVRE LE MAINTIEN, a dire : les cases que le moteur attend (les majorees,
/// pour un accident du travail au-dela du vingt-huitieme jour, CSS R433-3), remplies de ce que l'arret dit, sinon du montant de principe
/// (`engagement::maintien::ij_de_principe`), que la personne garde ou corrige ; sinon, la phrase dit ou le lire.
fn geste_des_indemnites(ctx: &Contexte, d: &Designe, a: &ArretSaisi, majoree: bool) -> R<GesteDeRoute<Designe>> {
    let mut propose = ValeursDeDepart::default();
    let mut dit = None;
    if !a.ij_brute.is_some_and(|x| !x.est_nul()) {
        use engagement::maintien::IjDePrincipe;
        match magasin(engagement::maintien::ij_de_principe(ctx, a))? {
            IjDePrincipe::Calculee(ij) => {
                propose.ij_brute = Some(ij.brute);
                propose.ij_nette = Some(ij.nette);
                propose.ij_brute_majoree = ij.majorees.map(|(b, _)| b);
                propose.ij_nette_majoree = ij.majorees.map(|(_, n)| n);
                dit = Some(ij.phrase);
            }
            IjDePrincipe::AuReleve(phrase) => dit = Some(phrase),
        }
    }
    let mut cases = vec![cle!(ChargeArretIndemnites, ij_brute), cle!(ChargeArretIndemnites, ij_nette)];
    if majoree {
        cases.extend([cle!(ChargeArretIndemnites, ij_brute_majoree), cle!(ChargeArretIndemnites, ij_nette_majoree)]);
    }
    cases.push(cle!(ChargeArretIndemnites, jours_deja_indemnises));
    // Des indemnites deja dites se reprennent ; pas encore dites, elles sont a dire.
    let libelle = if a.ij_brute.is_some() { INDEMNITES_DITES } else { INDEMNITES_A_DIRE };
    let valeurs = ValeursDeDepart {
        ij_brute: a.ij_brute.or(propose.ij_brute),
        ij_nette: a.ij_nette.or(propose.ij_nette),
        ij_brute_majoree: a.ij_brute_majoree.or(propose.ij_brute_majoree),
        ij_nette_majoree: a.ij_nette_majoree.or(propose.ij_nette_majoree),
        jours_deja_indemnises: a.jours_deja_indemnises,
        ..ValeursDeDepart::default()
    };
    let mut phrases = vec![crate::ouverture::aide("indemnites_journalieres")?.to_string()];
    if let Some(dit) = dit {
        phrases.push(dit);
    }
    phrases.push(crate::ouverture::aide("jours_deja_indemnises")?.to_string());
    Ok(GesteDeRoute { cases, valeurs, phrases, ..d.geste(Route::ArretIndemnites, libelle) })
}

/// CE QU'UN ARRET OUVRE (rien de remplissable s'il n'y a pas a le remplir) : en cours, il se prolonge ou se reprend plus
/// tot ; une reprise saisie se corrige tant que son signalement n'est pas parti ; ses indemnites journalieres se disent
/// s'il ouvre le maintien (`indemnites`) ; il se retire tant que rien n'en est parti (ni signalement, ni DSN qui le
/// reporte). Une reprise a la date prevue ne se saisit pas : la DSN la porte.
fn gestes_d_un_arret(n: &Bloc60, a: &ArretSaisi, base: &ArretResume, cours: bool, reprise_corrigeable: bool, indemnites: Option<&GesteDeRoute<Designe>>) -> R<Vec<GesteDeRoute<Designe>>> {
    let d = Designe::Arret { salarie: a.salarie.clone(), dernier_jour: a.dernier_jour_travaille.clone() };
    let mut g = Vec::new();
    if cours {
        g.push(GesteDeRoute {
            cases: vec![cle!(ChargeArretProlonger, fin)],
            phrases: vec![crate::ouverture::aide("prolongation")?.to_string()],
            ..d.geste(Route::ArretProlonger, PROLONGER)
        });
        g.push(geste_de_reprise(&d, a, n, REPRISE_PLUS_TOT));
    }
    if reprise_corrigeable {
        g.push(geste_de_reprise(&d, a, n, CORRIGER_LA_REPRISE));
    }
    g.extend(indemnites.cloned());
    if !base.signale && base.reporte_par.is_empty() {
        g.push(d.geste(Route::ArretOublier, RETIRER));
    }
    Ok(g)
}

/// Ce qu'une absence ouvre : sa fin change ; elle se retire tant qu'aucune DSN ne l'a reportee.
fn gestes_d_une_absence(s: &SuspensionSaisie, reportee: &[Mois]) -> Vec<GesteDeRoute<Designe>> {
    let d = Designe::Absence { salarie: s.salarie.clone(), debut: s.debut.clone() };
    let mut g = vec![GesteDeRoute {
        cases: vec![cle!(ChargeSuspensionModifier, fin)],
        valeurs: ValeursDeDepart { fin: Some(s.fin.clone()), ..ValeursDeDepart::default() },
        ..d.geste(Route::SuspensionModifier, CHANGER_LA_FIN)
    }];
    if reportee.is_empty() {
        g.push(d.geste(Route::SuspensionOublier, RETIRER));
    }
    g
}

/// Un jour paye pendant l'imprevu (`imprevus::cachets_dans`, tel qu'il le rend), et chacun de ses jours en phrases.
#[derive(Debug, Clone, Serialize)]
struct Cachet {
    #[serde(flatten)]
    lu: imprevus::CachetPaye,
    jours: Vec<JourPaye>,
}

#[derive(Debug, Clone, Serialize)]
struct JourPaye {
    iso: Jour,
    phrase: String,
    phrase_du_mois: String,
}

/// CE QUE L'ECRAN LIT D'UN IMPREVU, en phrases que la page ne compose pas : ce qu'il est (`quoi`), sa periode
/// (`periode` ; un arret sans reprise dit sa fin prescrite), son contrat (`contrat_dit`), sa reprise (`reprise_dit`), ce
/// qui est paye pendant (`paye_pendant`), et les signalements deja partis (`deja_signale`, par nature).
#[derive(Debug, Clone, Serialize)]
struct Dit {
    quoi: String,
    periode: String,
    contrat_dit: Option<String>,
    reprise_dit: Option<String>,
    paye_pendant: &'static str,
    /// Combien de fois chaque nature est deja partie, pour celles qui le sont.
    deja_signale: BTreeMap<NatureSignalement, String>,
}

/// Un morceau de ce qui est parti d'un imprevu, ou de ce qui en reste du (`pastille`).
#[derive(Debug, Clone, Serialize)]
struct Parti {
    phrase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pastille: Option<Pastille>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discret: Option<bool>,
}

/// Ce que `Dit` lit d'un imprevu.
struct ADire<'a> {
    arret: bool,
    libelle: &'a str,
    reprise: Option<Jour>,
    premier: Jour,
    dernier: Jour,
    contrat: Option<&'a NumeroContrat>,
    signalements: &'a [crate::obligations::EnvoiDeLArret],
}

impl Dit {
    fn de(x: ADire) -> R<Dit> {
        let deja = |nature: NatureSignalement| {
            let n = x.signalements.iter().filter(|s| s.nature_declaration == nature.nature()).count();
            (n > 0).then(|| format!("Déjà signalé {} fois", n))
        };
        Ok(Dit {
            quoi: if x.arret { format!("Arrêt de travail ({})", x.libelle) } else { x.libelle.to_string() },
            periode: if x.arret && x.reprise.is_none() {
                format!("depuis le {}, prescrit jusqu'au {}", x.premier.jj_mm_aaaa(), x.dernier.jj_mm_aaaa())
            } else {
                format!("du {} au {}", x.premier.jj_mm_aaaa(), x.dernier.jj_mm_aaaa())
            },
            contrat_dit: x.contrat.map(|c| format!("contrat {}", c)),
            reprise_dit: x.reprise.map(|r| format!("reprise le {}", r.jj_mm_aaaa())),
            paye_pendant: if x.arret { "Payé pendant l'arrêt" } else { "Payé pendant l'absence" },
            deja_signale: NatureSignalement::TOUS.into_iter().filter_map(|n| deja(n).map(|d| (n, d))).collect(),
        })
    }
}

/// Un cachet paye pendant l'imprevu, et chacun de ses jours, dit dans son mois (« de novembre 2026 »).
fn cachet_dit(c: imprevus::CachetPaye) -> R<Cachet> {
    let mois_dit = crate::regularisation::de_mois_en_clair(c.mois);
    let mut jours = Vec::new();
    for j in &c.dates {
        let le = format!("{} le {}", c.libelle, j.jj_mm_aaaa());
        jours.push(JourPaye { iso: *j, phrase_du_mois: format!("{}, {}", le, mois_dit), phrase: le });
    }
    Ok(Cachet { lu: c, jours })
}

/// CE QUI EST PARTI D'UN IMPREVU, ET CE QUI EN RESTE DU : une ligne courte par morceau, dans l'ordre ou l'ecran les lit --
/// les prolongations, les signalements, la DSN qui l'a declare, puis chaque obligation, faite ou due (avec sa pastille) ;
/// « rien n'est encore parti » sinon.
fn ce_qui_est_parti(prolongations: &[Prolongation], signalements: &[crate::obligations::EnvoiDeLArret], reporte_par_dit: &[String], obligations: &[crate::obligations::Obligation]) -> R<Vec<Parti>> {
    let minuscule = |t: String| -> String { t.chars().take(1).flat_map(char::to_lowercase).chain(t.chars().skip(1)).collect() };
    let dite = |phrase: String| Parti { phrase, pastille: None, discret: None };
    let mut parti = Vec::new();
    for p in prolongations.iter().filter(|p| p.fin != p.fin_avant) {
        parti.push(dite(format!("prolongé jusqu'au {} (avis du {})", p.fin.jj_mm_aaaa(), p.le.jj_mm_aaaa())));
    }
    for s in signalements {
        let quoi = if s.nature_declaration == NatureDeLaDeclaration::SignalementReprise { "reprise signalée" } else if s.type_declaration == Some(TypeDeLaDeclaration::AnnuleEtRemplace) { "signalement rectifié" } else { "signalé" };
        parti.push(dite(format!("{} le {}", quoi, s.quand.dit())));
    }
    if !reporte_par_dit.is_empty() {
        parti.push(dite(format!("déclaré par la DSN {}", reporte_par_dit.join(", "))));
    }
    for o in obligations {
        let libelle = minuscule(o.libelle.clone());
        match &o.fait {
            Some(f) if matches!(o.ou, Some(crate::obligations::Ou::Lieux(_))) => {
                parti.push(dite(format!("{}, faite le {}", libelle, f.le().jj_mm_aaaa())));
            }
            // Une obligation due sans date butoir (son delai ne se compte pas) dit son etat, pas une butoir absente.
            _ if o.du => {
                let quand = match &o.butoir_dit {
                    Some(b) => b.clone(),
                    None => o.pastille.mots.clone(),
                };
                parti.push(Parti { pastille: Some(o.pastille.clone()), ..dite(format!("{}, {}", libelle, quand)) });
            }
            _ => {}
        }
    }
    if parti.is_empty() {
        parti.push(Parti { discret: Some(true), ..dite("rien n'est encore parti".to_string()) });
    }
    Ok(parti)
}

impl Resume {
    /// Les obligations d'un arret (`obligations::de_l_arret`), posees sur lui ; une absence n'en a pas.
    pub(crate) fn poser_obligations(&mut self, obligations: Vec<crate::obligations::Obligation>) {
        if let Resume::Arret(a) = self {
            a.obligations = obligations;
        }
    }

    /// Ce qui est parti de l'imprevu (`ce_qui_est_parti`), une fois ses obligations posees (un arret sans obligations
    /// posees n'en dit aucune).
    fn dire_ce_qui_est_parti(&mut self) -> R<()> {
        match self {
            Resume::Arret(a) => a.dire_ce_qui_est_parti()?,
            Resume::Suspension(s) => s.parti = ce_qui_est_parti(&[], &[], &s.reporte_par_dit, &[])?,
        }
        Ok(())
    }

    /// Les jours payes pendant l'imprevu qui sont de `mois`, eux seuls : ceux que la carte de ce mois retire.
    fn garder_les_cachets_de(&mut self, mois: Mois) {
        let cachets = match self {
            Resume::Arret(a) => &mut a.cachets,
            Resume::Suspension(s) => &mut s.cachets,
        };
        cachets.retain(|c| c.lu.mois == mois);
    }

    /// Son premier jour.
    fn premier_jour(&self) -> Jour {
        match self {
            Resume::Arret(a) => a.premier_jour,
            Resume::Suspension(s) => s.premier_jour,
        }
    }

    /// L'imprevu en lignes, tel que `imprevus` le dit : qui, quel contrat, quoi, quand, ou en est son signalement, puis
    /// chacune de ses obligations.
    fn en_lignes(&self) -> Vec<String> {
        let (salarie, contrat, libelle, premier, dernier, etat, obligations) = match self {
            Resume::Arret(a) => (&a.base.salarie, &a.base.contrat, &a.base.motif_libelle, &a.premier_jour, &a.dernier_jour, a.base.etat.dit(), a.obligations.as_slice()),
            Resume::Suspension(s) => (&s.salarie, &s.contrat, &s.motif_libelle, &s.premier_jour, &s.dernier_jour, "", &[][..]),
        };
        let mut l = vec![format!("{} ({}) : {}, du {} au {} — {}", salarie, numero_dit(contrat.as_ref(), "", "", SANS_NUMERO), libelle, premier.jj_mm_aaaa(), dernier.jj_mm_aaaa(), etat)];
        l.extend(crate::obligations::en_phrases(obligations).into_iter().map(|o| format!("  {}", o)));
        l
    }
}

/// UN IMPREVU TEL QUE LA PAGE LE LIT (`Resume`), avec les cachets qui tombent dedans.
pub(crate) fn resume(ctx: &Contexte, n: &Bloc60, imp: &Imprevu) -> R<Resume> {
    let cachets = cachets_de(ctx, imp)?;
    let premier = magasin(imprevus::premier_jour(imp))?;
    let dernier = magasin(imprevus::dernier_jour(imp))?;
    match imp {
        Imprevu::Arret(a) => Ok(Resume::Arret(Box::new(resume_d_arret(ctx, n, a, cachets, premier, dernier)?))),
        Imprevu::Suspension(s) => Ok(Resume::Suspension(Box::new(resume_de_suspension(ctx, s, cachets, premier, dernier)?))),
    }
}

/// Les jours payes pendant l'imprevu (`imprevus::cachets_dans`), dits.
fn cachets_de(ctx: &Contexte, imp: &Imprevu) -> R<Vec<Cachet>> {
    magasin(imprevus::cachets_dans(ctx, imp))?.into_iter().map(cachet_dit).collect()
}

/// Un arret tel que la page le lit (`ResumeDArret`).
fn resume_d_arret(ctx: &Contexte, n: &Bloc60, a: &ArretSaisi, cachets: Vec<Cachet>, premier: Jour, dernier: Jour) -> R<ResumeDArret> {
    let base = resume_arret(ctx, n, a)?;
    let motif = magasin(droit::arrets::motif_d_arret(a.motif))?;
    let origine = motif.and_then(|m| m.origine);
    let ouvert = ouvre_le_maintien(ctx, a)?;
    let majoree = ouvert && origine.is_some_and(|o| o.professionnelle() || o == droit::maintien::Origine::Trajet) && socle::date::Duree::du_au(&premier, &dernier) > socle::date::Duree::jours(28);
    let designe = Designe::Arret { salarie: a.salarie.clone(), dernier_jour: a.dernier_jour_travaille };
    let a_dire = if ouvert { Some(geste_des_indemnites(ctx, &designe, a, majoree)?) } else { None };
    let cours = en_cours(ctx, a)?;
    let reprise_corrigeable = a.reprise.is_some() && !envoye(ctx, a, NatureSignalement::Reprise)?;
    let gestes = gestes_d_un_arret(n, a, &base, cours, reprise_corrigeable, a_dire.as_ref())?;
    let dit = Dit::de(ADire {
        arret: true,
        libelle: &base.motif_libelle,
        reprise: a.reprise,
        premier,
        dernier,
        contrat: a.contrat_numero.as_ref(),
        signalements: &base.signalements,
    })?;
    Ok(ResumeDArret {
        qui: ctx.nomme(&a.salarie),
        base,
        genre: GenreDImprevu::Arret,
        premier_jour: premier,
        dernier_jour: dernier,
        jours_deja_indemnises: a.jours_deja_indemnises.clone(),
        ij_brute: a.ij_brute.clone(),
        ij_nette: a.ij_nette.clone(),
        ij_brute_majoree: a.ij_brute_majoree.clone(),
        ij_nette_majoree: a.ij_nette_majoree.clone(),
        origine,
        cachets,
        prolongations: a.prolongations.clone(),
        gestes,
        a_dire,
        dit,
        obligations: Vec::new(),
        parti: Vec::new(),
    })
}

/// Une absence telle que la page la lit (`ResumeDeSuspension`). Son motif est connu de la norme : un motif inconnu ne
/// s'enregistre pas (`verifier_suspension`), il se refuse s'il est dans le magasin.
fn resume_de_suspension(ctx: &Contexte, s: &SuspensionSaisie, cachets: Vec<Cachet>, premier: Jour, dernier: Jour) -> R<ResumeDeSuspension> {
    let reportee = reporte_par(&ses_envois(ctx, &Imprevu::Suspension(s.clone()))?);
    let motif = magasin(imprevus::motif_de_suspension(s.motif))?.ok_or_else(|| {
        erreur(Classe::Illisible, format!("La suspension de {} du {} a un motif que la norme ne connaît pas ({}) : la corriger.", s.salarie, s.debut.iso(), s.motif))
    })?;
    let dit = Dit::de(ADire {
        arret: false,
        libelle: &motif.libelle,
        reprise: None,
        premier,
        dernier,
        contrat: s.contrat_numero.as_ref(),
        signalements: &[],
    })?;
    Ok(ResumeDeSuspension {
        genre: GenreDImprevu::Suspension,
        qui: ctx.nomme(&s.salarie),
        salarie: s.salarie.clone(),
        contrat: s.contrat_numero.clone(),
        contrat_debut: s.contrat_debut,
        motif: s.motif,
        motif_libelle: motif.libelle.clone(),
        remunere: motif.remunere,
        debut: s.debut,
        fin: s.fin,
        premier_jour: premier,
        dernier_jour: dernier,
        note: s.note.clone(),
        saisi_le: s.saisi_le,
        reporte_par_dit: mois_de_la_dsn(&reportee),
        gestes: gestes_d_une_absence(s, &reportee),
        cachets,
        dit,
        parti: Vec::new(),
    })
}

/// Un motif d'absence qui se declare, tel que la page le propose.
#[derive(Debug, Serialize)]
struct MotifSuspension {
    code: MotifDeSuspension,
    libelle: String,
    libelle_dit: String,
    remunere: bool,
}

/// Les motifs d'une suspension qui se DECLARENT (`permanents-base.json`, section `suspensions`, sans le 200).
fn motifs_de_suspension() -> R<Vec<MotifSuspension>> {
    let regles = magasin(droit::permanents::regles())?;
    Ok(regles
        .suspensions
        .motifs
        .iter()
        .filter(|m| m.ne_se_declare_pas.is_none())
        .map(|m| {
            let libelle_dit = format!("{}{}", m.libelle, if m.remunere { " (payé)" } else { "" });
            MotifSuspension { code: m.code, libelle: m.libelle.clone(), libelle_dit, remunere: m.remunere }
        })
        .collect())
}

// ------------------------------------------------------------------ imprevus

/// La route `imprevus` : les imprevus (d'un salarie, qui croisent un mois), resumes -- un arret avec ses `obligations` --,
/// et les nomenclatures pour en saisir.
fn imprevus(ctx: &Contexte, c: ChargeImprevus) -> R<ReponseImprevus> {
    let cle = c.salarie.as_ref().map(|s| cle_du_nom(ctx, s)).transpose()?;
    let bornes = c.mois.map(Periode::du_mois);
    let n = nomenclatures()?;
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    let mut out = Vec::new();
    for lu in magasin(imprevus::tous(&ctx.racine))? {
        if cle.as_ref().is_some_and(|c| lu.salarie() != c) {
            continue;
        }
        if let Some(mois) = &bornes {
            if !magasin(imprevus::croise(&lu, mois))? {
                continue;
            }
        }
        let mut r = resume(ctx, &n, &lu)?;
        // CE QU'UN ARRET FAIT NAITRE, SUR L'ARRET LUI-MEME : la page des imprevus (la carte d'un salarie, la
        // colonne du mois, son dossier) lit une seule liste, et chaque arret y porte ses obligations, calculees au meme
        // endroit que pour la route `arrets` (`obligations::de_l_arret`).
        if let Imprevu::Arret(a) = &lu {
            r.poser_obligations(crate::obligations::de_l_arret(ctx, a, &suivi, ctx.aujourdhui)?);
        }
        r.dire_ce_qui_est_parti()?;
        // Les imprevus d'un mois disent les jours payes de ce mois (la carte les retire un par un).
        if let Some(mois) = c.mois {
            r.garder_les_cachets_de(mois);
        }
        out.push(r);
    }
    // LES IMPREVUS D'UN SALARIE, son dossier : le plus recent d'abord, sous leur titre.
    let titre = match &c.salarie {
        Some(_) => {
            out.sort_by(|x, y| y.premier_jour().cmp(&x.premier_jour()));
            (!out.is_empty()).then_some(SES_IMPREVUS)
        }
        None => None,
    };
    // CE QUI EST DU DES ARRETS DU MOIS, le plus presse d'abord (`Obligation::rang`), chaque obligation avec ses gestes.
    let mut dus = Vec::new();
    if c.mois.is_some() {
        for r in &out {
            let Resume::Arret(a) = r else { continue };
            for o in a.obligations.iter().filter(|o| o.du) {
                let mut gestes: Vec<GesteDEcheance> = geste_qui_fait(a, o)?.into_iter().collect();
                gestes.extend(o.geste_de_la_connaissance(a.salarie(), a.dernier_jour_travaille()));
                dus.push(ObligationDue { qui: ctx.nomme(a.salarie()), rang: o.rang(), obligation: o.clone(), gestes });
            }
        }
    }
    dus.sort_by(|x, y| x.rang.cmp(&y.rang));
    Ok(ReponseImprevus {
        phrase: socle::phrase!("Imprévus : {}.", out.len()),
        dus_titre: (!dus.is_empty()).then_some(CE_QUI_EST_DU),
        dus,
        phrases: out.iter().flat_map(Resume::en_lignes).collect(),
        imprevus: out,
        motifs_arret: n.motifs_arret.saisissables(),
        motifs_reprise: n.motifs_reprise.saisissables(),
        motifs_suspension: motifs_de_suspension()?,
        porte: porte(),
        porte_dit: PORTE_DIT,
        titre,
    })
}

// ------------------------------------------------------------------ le maintien

/// `arret-indemnites`. Les indemnites journalieres d'un arret, par jour, telles que les ecrit le releve de la caisse ou
/// son bordereau de paiement (BPIJ : le prix unitaire de l'« I.J. normale » ou « majoree », puis les retenues CSG et RDS,
/// `atelier/reference/sources/subrogation-indemnites-journalieres/bpij-edi/`) : brutes, et nettes de CSG et CRDS. Le
/// maintien de salaire s'en deduit (`engagement::maintien`) ; avant le releve, le resume en propose le montant de principe
/// (`geste_des_indemnites`), qui remplit les cases du geste. Un champ absent ne change pas. Le geste ne s'offre que pour
/// un arret qui ouvre le maintien ; ailleurs, elles ne se deduisent de rien.
fn arret_indemnites(ctx: &Contexte, c: &ChargeArretIndemnites) -> R<ReponseArret> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let dernier = c.dernier_jour;
    let (i, mut liste, mut a) = l_arret(ctx, &cle, &dernier)?;
    if [&c.ij_brute, &c.ij_nette, &c.ij_brute_majoree, &c.ij_nette_majoree].iter().all(|v| v.is_none()) && c.jours_deja_indemnises.is_none() {
        return Err(refus("Dire les indemnités journalières (brute et nette, par jour) ou les jours déjà indemnisés : rien n'est changé."));
    }
    // L'arret garde chaque montant au centime.
    let montant = |t: Option<MontantUnitaire>| -> R<Option<MontantUnitaire>> {
        let Some(x) = t else { return Ok(None) };
        if x <= MontantUnitaire::ZERO {
            return Err(refus("Une indemnité journalière est un montant par jour, positif : rien n'est changé."));
        }
        Ok(Some(x.au_centime()))
    };
    if let Some(x) = montant(c.ij_brute)? {
        a.ij_brute = Some(x);
    }
    if let Some(x) = montant(c.ij_nette)? {
        a.ij_nette = Some(x);
    }
    if let Some(x) = montant(c.ij_brute_majoree)? {
        a.ij_brute_majoree = Some(x);
    }
    if let Some(x) = montant(c.ij_nette_majoree)? {
        a.ij_nette_majoree = Some(x);
    }
    if let Some(j) = c.jours_deja_indemnises {
        a.jours_deja_indemnises = Some(j);
    }
    let nouveau = Imprevu::Arret(a.clone());
    imprevus::verifier(&nouveau).map_err(|e| e.reclasser(Classe::Refuse))?;
    liste[i] = nouveau;
    ecrire(ctx, &dernier, liste)?;
    reponse_arret(ctx, &a, Phrase::fixe("Indemnités journalières notées."))
}

/// Le contrat qu'un imprevu suspend, lu dans la saisie qui le porte ; `None` s'il n'y est plus.
fn contrat_de(ctx: &Contexte, imp: &impl Suspend) -> R<Option<ContratAuJour>> {
    let contrats = contrats_au(ctx, imp.salarie(), imp.contrat_debut())?;

    Ok(contrats.into_iter().find(|c| c.periode.debut() == imp.contrat_debut()))
}

/// La nature du contrat qu'un arret suspend ; `None` s'il n'y est plus.
fn nature_du_contrat(ctx: &Contexte, a: &ArretSaisi) -> R<Option<std::sync::Arc<droit::natures::Nature>>> {
    let Some(c) = contrat_de(ctx, a)? else { return Ok(None) };
    // Une nature se dit, elle ne se devine pas (regle 6) : celle d'un contrat repris se dit apres l'import (`import::nature`).
    let Some(n) = c.nature else {
        return Err(refus(format!("{} : la nature de son contrat du {} n'est pas encore dite.", c.nom, c.periode.debut().iso())));
    };
    Ok(Some(n.proprietes().map_err(|e| e.reclasser(Classe::Refuse))?))
}

/// L'arret ouvre-t-il le maintien de salaire, par la loi ou la convention (`engagement::maintien::droits`) ? Un arret dont
/// le contrat n'est plus dans la saisie n'en ouvre pas.
fn ouvre_le_maintien(ctx: &Contexte, a: &ArretSaisi) -> R<bool> {
    let ouvre = |nat: std::sync::Arc<droit::natures::Nature>| -> R<bool> { Ok(magasin(engagement::maintien::droits(ctx, a, &nat))?.is_some_and(|d| d.du())) };
    Ok(nature_du_contrat(ctx, a)?.map(ouvre).transpose()?.is_some_and(|oui| oui))
}

/// LA SUBROGATION D'UN ARRET, telle que le moteur la tire de la convention et du choix de la compagnie
/// (`engagement::maintien::subrogation`) : ses dates, ou rien (pas de subrogation).
fn subrogation_tiree(ctx: &Contexte, a: &ArretSaisi) -> R<Option<socle::date::Periode>> {
    match nature_du_contrat(ctx, a)? {
        Some(nat) => magasin(engagement::maintien::subrogation(ctx, a, &nat)),
        None => Ok(None),
    }
}

/// La subrogation tiree du droit (`subrogation_tiree`), posee sur l'arret : `01` et ses dates, ou `02`.
pub(crate) fn poser_la_subrogation(ctx: &Contexte, a: &mut ArretSaisi) -> R<()> {
    match subrogation_tiree(ctx, a)? {
        Some(p) => {
            a.subrogation = Some(SubrogationDeLArret::Oui);
            a.periode_de_subrogation = Some(p);
        }
        None => {
            a.subrogation = Some(SubrogationDeLArret::Non);
            a.periode_de_subrogation = None;
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ la prolongation

/// `arret-prolonger` : L'AVIS DE PROLONGATION D'UN ARRET EN COURS.
///
/// CE QUI PART, ET CE QUI NE PART PAS. « En cas de prolongation d'arret, il ne faut pas envoyer de nouveau signalement de
/// nature " arret de travail ". Il suffit de declarer la nouvelle date de fin de l'arret en DSN mensuelle, au niveau de la
/// rubrique " Date de fin previsionnelle - S21.G00.60.003 " » (fiche net-entreprises 1983, `atelier/reference/sources/
/// net-entreprises-base-connaissances/net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.txt`, l.
/// 13) ; ameli dit de meme a l'employeur en DSN : « modifier la date de fin previsionnelle de l'arret de travail dans votre
/// logiciel de paie » (`atelier/reference/sources/ameli-arret-de-travail/`). La route ne change donc que
/// `fin_previsionnelle` : le bloc S21.G00.60 de chaque DSN mensuelle que l'arret traverse la porte (`dsn::bloc_60`,
/// `imprevus::du_mois`, qui suit la nouvelle fin), la paie d'un permanent retire les jours ajoutes et le maintien s'ouvre
/// aux mois ajoutes (`imprevus::lignes_du_mois`, `maintien`), les cachets payes dans la prolongation s'avertissent
/// (`imprevus::cachets_dans`) -- par le mecanisme de toujours, sans second. Un signalement d'arret pas encore parti partira
/// avec la nouvelle fin.
///
/// LA SUBROGATION. Elle n'est portee que par le signalement d'arret (60.004 a 60.008 : C en 01, mais le moteur ne l'ecrit
/// qu'en 04 -- `dsn::bloc_60`, tableau des usages) ; sa fin « maximale prevue dans le cadre de la convention collective ou
/// de l'accord de branche » se declare d'emblee (fiche 1983, l. 25). Si elle est « insuffisante ou erronee », un nouveau
/// signalement d'arret de type annule et remplace la porte (l. 37), « le plus rapidement possible des connaissance de la
/// prolongation » (l. 66) ; si elle « court toujours », rien (l. 110). Le moteur tire ce maximum du droit
/// (`engagement::maintien::subrogation`) : il ne bouge qu'avec une subrogation qui dure tant que la caisse indemnise (1285,
/// X.4.7), et suit alors la nouvelle fin. Une fin de subrogation changee apres le
/// depot du signalement d'arret fait naitre le signalement rectificatif (`obligations::de_l_arret`,
/// `signalement_arret_rectificatif`), d'ou la trace de chaque avis dans `prolongations`.
///
/// CE QUI REFUSE, sans rien ecrire : un arret deja repris (« se prolonge sans interruption », ameli : un arret apres la
/// reprise est un nouvel arret) ; une fin qui n'est pas apres la fin prescrite (une fin plus tot est une reprise
/// anticipee, qui se signale : `arret-reprise`) ; ce que la norme refuse au signalement d'arret (`verifier`) ; un
/// chevauchement.
fn arret_prolonger(ctx: &Contexte, c: &ChargeArretProlonger) -> R<ReponseArret> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let dernier = c.dernier_jour;
    let (i, mut liste, avant) = l_arret(ctx, &cle, &dernier)?;
    if let Some(reprise) = &avant.reprise {
        return Err(refus(format!(
            "L'arrêt de {} a pris fin : le salarié a repris le {}. Une prolongation continue un arrêt en cours, sans \
interruption ; un arrêt qui commence après la reprise est un nouvel arrêt, qui se saisit et se signale comme tel. Rien \
n'est changé.",
            cle,
            reprise.jj_mm_aaaa()
        )));
    }
    let f = c.fin;
    let fin_avant = avant.fin_previsionnelle;
    if f < fin_avant {
        return Err(refus(format!(
            "Le {} est avant la fin prescrite de l'arrêt (le {}) : ce n'est pas une prolongation. Si le salarié a repris \
plus tôt, c'est une reprise anticipée, qui se saisit comme telle et se signale (nature 05). Rien n'est changé.",
            f.jj_mm_aaaa(),
            fin_avant.jj_mm_aaaa()
        )));
    }
    if f == fin_avant {
        return Err(refus(format!("L'arrêt est déjà prescrit jusqu'au {} : rien n'est changé.", f.jj_mm_aaaa())));
    }
    let mut a = avant.clone();
    a.fin_previsionnelle = f;
    // Une subrogation qui dure tant que la caisse indemnise (1285, X.4.7) suit la fin de l'arret ; les autres vont deja
    // jusqu'au maximum du maintien (`engagement::maintien::subrogation`).
    if signalement_subroge(&a)? {
        if let Some(sub) = subrogation_tiree(ctx, &a)? {
            a.periode_de_subrogation = Some(sub);
        }
    }
    verifier(&a)?;
    sans_chevauchement(ctx, &Imprevu::Arret(a.clone()), Some(&Imprevu::Arret(avant.clone())))?;

    // La trace de l'avis : le jour ou la compagnie l'a su (le jour du moteur), l'heure de la saisie, et ce qui a change.
    a.prolongations.push(Prolongation {
        le: ctx.aujourdhui,
        saisi_le: maintenant(),
        qui: INTERFACE.to_string(),
        fin_avant: avant.fin_previsionnelle,
        fin: a.fin_previsionnelle,
        periode_de_subrogation_avant: avant.periode_de_subrogation,
        periode_de_subrogation: a.periode_de_subrogation,
    });
    liste[i] = Imprevu::Arret(a.clone());
    ecrire(ctx, &dernier, liste)?;

    let mut avertissements: Vec<Phrase> = Vec::new();
    avertissements.push(socle::phrase!(
        "Prolongé jusqu'au {}. Rien ne part : pas de nouveau signalement d'arrêt ; la DSN mensuelle de chaque mois que \
l'arrêt traverse porte la nouvelle fin prévisionnelle (S21.G00.60.003, fiche net-entreprises 1983).",
        f.jj_mm_aaaa()
    ));
    // Les jours ajoutes qui tombent dans un mois dont la DSN reelle est deja partie : elle portait l'ancienne fin.
    let declares = reporte_par(&envois_de_l_arret(ctx, &avant)?);
    // Apres la fin d'avant : la prolongation la recule (refus plus haut sinon).
    let ajoutes = Periode::du_au(fin_avant.apres(Duree::jours(1)).map_err(|e| e.reclasser(Classe::Illisible))?, f)?;
    for m in ajoutes.mois() {
        if declares.contains(&m) {
            avertissements.push(socle::phrase!(
                "La DSN de {} est déjà partie avec l'ancienne fin (le {}) : la paie de ce mois, où les jours ajoutés \
tombent, est à corriger.",
                m.libelle(Graphie::Accentuee),
                fin_avant.jj_mm_aaaa()
            ));
        }
    }
    if a.periode_de_subrogation != avant.periode_de_subrogation && envoye(ctx, &avant, NatureSignalement::Arret)? {
        avertissements.push(Phrase::fixe(
            "Le signalement d'arrêt est déjà parti avec l'ancienne fin de subrogation : un signalement d'arrêt annule et \
remplace porte la nouvelle, le plus vite possible (fiche net-entreprises 1983). Il paraît dans ce qui est dû.",
        ));
    }
    reponse_arret(ctx, &a, Phrase::jointes(avertissements, "  ·  "))
}

/// L'arret est-il subroge (S21.G00.60.004 = oui) ?
fn signalement_subroge(a: &ArretSaisi) -> R<bool> {
    declarations::signalement::avec_subrogation(&declarations::dsn::Arret { saisi: a.clone() }).map_err(|m| m.reclasser(Classe::Illisible))
}

// ------------------------------------------------------------------ les suspensions

/// Ce qu'une suspension doit tenir avant de s'ecrire : son motif se declare (`imprevus::verifier_motif_de_suspension` :
/// connu, pas le 200), elle finit apres avoir commence, et ne deborde pas de son contrat.
fn verifier_suspension(s: &SuspensionSaisie, contrat_fin: Option<Jour>) -> R<()> {
    imprevus::verifier_motif_de_suspension(s.motif, s.salarie.as_str()).map_err(|e| e.reclasser(Classe::Refuse))?;
    let (debut, fin) = (s.debut, s.fin);
    if fin < debut {
        return Err(refus(format!(
            "Suspension du {} au {} : elle finit avant de commencer (S21.G00.65.002 et .003).",
            debut.jj_mm_aaaa(),
            fin.jj_mm_aaaa()
        )));
    }
    if let Some(contrat_fin) = contrat_fin.filter(|f| fin > *f) {
        return Err(refus(format!(
            "Suspension jusqu'au {}, mais le contrat {} finit le {} : une suspension ne survit pas a son contrat.",
            fin.jj_mm_aaaa(),
            numero_dit(s.contrat_numero.as_ref(), "", "", SANS_NUMERO),
            contrat_fin.jj_mm_aaaa()
        )));
    }
    Ok(())
}

/// `suspension-enregistrer`.
fn suspension_enregistrer(ctx: &Contexte, c: &ChargeSuspensionEnregistrer) -> R<ReponseSuspension> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let debut = c.debut;
    let contrat = contrat_suspendu(ctx, &cle, &debut, c.contrat.as_ref())?;
    let s = SuspensionSaisie {
        salarie: cle.clone(),
        contrat_debut: contrat.periode.debut(),
        contrat_numero: contrat.numero.clone(),
        motif: c.motif,
        debut,
        fin: c.fin,
        saisi_le: maintenant(),
        qui: INTERFACE.to_string(),
        note: c.note.clone(),
    };
    verifier_suspension(&s, contrat.periode.fin())?;
    if trouver_suspension(ctx, &cle, &debut)?.is_some() {
        return Err(refus(format!(
            "Une suspension de {} au {} est deja enregistree : la modifier plutot que d'en saisir une seconde.",
            cle,
            debut.jj_mm_aaaa()
        )));
    }
    sans_chevauchement(ctx, &Imprevu::Suspension(s.clone()), None)?;
    let mut liste = le_fichier(ctx, &debut)?;
    liste.push(Imprevu::Suspension(s.clone()));
    ecrire(ctx, &debut, liste)?;
    reponse_suspension(ctx, &s, Phrase::fixe("Absence enregistrée. Chaque mois qu'elle traverse la déclarera."))
}

/// `suspension-modifier`. Le debut designe la suspension et ne change pas : une suspension qui commence un autre jour
/// s'oublie et se saisit de nouveau.
fn suspension_modifier(ctx: &Contexte, c: &ChargeSuspensionModifier) -> R<ReponseSuspension> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let debut = c.debut;
    let (i, mut liste, avant) = la_suspension(ctx, &cle, &debut)?;
    let mut s = avant.clone();
    if let Some(f) = c.fin {
        s.fin = f;
    }
    if let Some(m) = c.motif {
        s.motif = m;
    }
    let fin_du_contrat = contrat_de(ctx, &s)?.and_then(|c| c.periode.fin());
    verifier_suspension(&s, fin_du_contrat)?;
    sans_chevauchement(ctx, &Imprevu::Suspension(s.clone()), Some(&Imprevu::Suspension(avant)))?;
    liste[i] = Imprevu::Suspension(s.clone());
    ecrire(ctx, &debut, liste)?;
    reponse_suspension(ctx, &s, Phrase::fixe("Absence changée."))
}

/// `suspension-oublier`. Une suspension qu'une DSN reelle a deja declaree ne se retire pas : le magasin garderait moins
/// que ce qui est parti.
fn suspension_oublier(ctx: &Contexte, c: &ChargeSuspensionOublier) -> R<ReponseSuspension> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let debut = c.debut;
    let (i, mut liste, s) = la_suspension(ctx, &cle, &debut)?;
    let envois = ses_envois(ctx, &liste[i])?.len();
    if envois > 0 {
        return Err(refus(format!(
            "Cette suspension a deja ete declaree ({} DSN). La retirer du magasin n'annule rien aupres des organismes : la \
reprendre (une fin plus tot), ou corriger les DSN deposees.",
            envois
        )));
    }
    liste.remove(i);
    ecrire(ctx, &debut, liste)?;
    reponse_suspension(ctx, &s, Phrase::fixe("Absence retirée : aucune déclaration ne l'avait portée."))
}

/// Ce que la norme refuse (`SignalementImpossible`, une valeur `Illisible`) ; les avertissements ne servent pas ici.
fn verifier(arret: &ArretSaisi) -> R<()> {
    match signalement::verifier(&declarations::dsn::Arret { saisi: arret.clone() }, NatureSignalement::Arret, None) {
        Ok(_) => Ok(()),
        Err(signalement::Echec::Impossible(m)) => Err(m.reclasser(Classe::Illisible)),
        Err(signalement::Echec::Autre(e)) => Err(e),
    }
}

/// Le dernier jour travaille saisi de l'arret de ce salarie dont un signalement a declare `declare` (S21.G00.60.002,
/// `imprevus::dernier_jour_declare`) : ce qui designe l'arret aux routes ; `declare` lui-meme si aucun arret ne le porte.
pub(crate) fn dernier_jour_saisi(ctx: &Contexte, salarie: &IdSalarie, declare: Jour) -> R<Jour> {
    for a in tous_les_arrets(ctx)? {
        if a.salarie == *salarie && magasin(imprevus::dernier_jour_declare(&a))? == declare {
            return Ok(a.dernier_jour_travaille);
        }
    }
    Ok(declare)
}

/// `{"arret": resume, "phrase"}` : l'arret tel que la page le lit (`resume`, avec les cachets qui tombent dedans), et ce
/// qui se lit du geste.
fn reponse_arret(ctx: &Contexte, arret: &ArretSaisi, phrase: Phrase) -> R<ReponseArret> {
    Ok(ReponseArret { arret: resume(ctx, &nomenclatures()?, &Imprevu::Arret(arret.clone()))?, phrase, encore: EncoreAppele::default() })
}

/// `{"suspension": resume, "phrase"}`.
fn reponse_suspension(ctx: &Contexte, s: &SuspensionSaisie, phrase: Phrase) -> R<ReponseSuspension> {
    Ok(ReponseSuspension { suspension: resume(ctx, &nomenclatures()?, &Imprevu::Suspension(s.clone()))?, phrase, encore: EncoreAppele::default() })
}

/// `arret-enregistrer`. Les controles de la norme sont ceux du signalement (`signalement::verifier`, nature 04), avant
/// toute ecriture ; un arret ne chevauche pas un autre imprevu du meme contrat (`sans_chevauchement`).
fn arret_enregistrer(ctx: &Contexte, c: &ChargeArretEnregistrer) -> R<ReponseArret> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let dernier = c.dernier_jour;
    let choisi = contrat_suspendu(ctx, &cle, &dernier, c.contrat.as_ref())?;
    let mut arret = arret_a_ecrire(ArretDit {
        cle: &cle,
        contrat: &choisi,
        motif: c.motif,
        dernier_jour: dernier,
        debut: c.debut,
        fin: c.fin,
        date_accident: c.date_accident,
        jours_deja_indemnises: c.jours_deja_indemnises,
        note: c.note.clone(),
    });
    // Les jours deja indemnises (D1226-4) ne se disent que pour un arret qui ouvre le maintien de salaire.
    if c.jours_deja_indemnises.is_some() && !ouvre_le_maintien(ctx, &arret)? {
        return Err(refus(
            "Les jours deja indemnises (D1226-4) ne servent qu'au maintien de salaire, que cet arret n'ouvre pas. Rien n'est ecrit.",
        ));
    }
    // LA SUBROGATION SE TIRE DU DROIT : la convention qui la donne, ou le choix de la compagnie, et la duree maximale du
    // maintien pour ses dates (`engagement::maintien::subrogation`).
    poser_la_subrogation(ctx, &mut arret)?;
    // On verifie AVANT d'ecrire, avec les regles de la nature 04 -- celles qui partiront.
    verifier(&arret)?;
    // Le jour où la compagnie a su l'accident, point de départ des 48 heures de la déclaration (R441-3) : contrôlé ici,
    // écrit avec l'arrêt.
    let connu = match &c.connu {
        Some(dite) => Some(crate::obligations::connaissance_saisie(ctx, &arret, crate::obligations::DeQuoi::Accident, dite)?),
        None => None,
    };
    if trouver_arret(ctx, &cle, &dernier)?.is_some() {
        return Err(refus(format!(
            "Un arret de {} au {} est deja enregistre. Le modifier plutot que d'en saisir un second : deux arrets au meme dernier jour travaille, ce sont deux declarations qui se contredisent.",
            cle,
            dernier.jj_mm_aaaa()
        )));
    }
    sans_chevauchement(ctx, &Imprevu::Arret(arret.clone()), None)?;
    let mut liste = le_fichier(ctx, &dernier)?;
    liste.push(Imprevu::Arret(arret.clone()));
    ecrire(ctx, &dernier, liste)?;
    if let Some(saisie) = connu {
        crate::obligations::connaissance_ecrire(ctx, &arret, crate::obligations::DeQuoi::Accident, saisie)?;
    }
    reponse_arret(ctx, &arret, Phrase::fixe("Arrêt enregistré."))
}

/// `arret-reprise` : une reprise nulle l'efface, et son motif avec elle.
fn arret_reprise(ctx: &Contexte, c: &ChargeArretReprise) -> R<ReponseArret> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let (i, mut liste, mut a) = l_arret(ctx, &cle, &c.dernier_jour)?;
    let reprise = c.reprise;
    let motif = match reprise {
        Some(_) => c.motif_reprise.clone(),
        None => None,
    };
    // La trace de la saisie : quand, et la reprise d'avant et d'apres (`etats_d_alors` la defait).
    a.reprises.push(RepriseNotee {
        le: ctx.aujourdhui,
        saisi_le: maintenant(),
        qui: INTERFACE.to_string(),
        reprise_avant: a.reprise,
        reprise_motif_avant: a.reprise_motif.clone(),
        reprise,
        reprise_motif: motif.clone(),
    });
    a.reprise = reprise;
    a.reprise_motif = motif;
    verifier(&a)?;
    liste[i] = Imprevu::Arret(a.clone());
    ecrire(ctx, &c.dernier_jour, liste)?;
    reponse_arret(ctx, &a, Phrase::fixe(if c.reprise.is_some() { "Reprise notée." } else { "Reprise effacée." }))
}

/// `arret-oublier`. Un arret deja declare ne se retire pas.
fn arret_oublier(ctx: &Contexte, c: &ChargeArretOublier) -> R<ReponseArret> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let (i, mut liste, a) = l_arret(ctx, &cle, &c.dernier_jour)?;
    let envois = envois_de_l_arret(ctx, &a)?.len();
    if envois > 0 {
        return Err(refus(format!(
            "Cet arret a deja ete declare ({} envoi(s)). Le retirer du magasin n'annule rien aupres des organismes et ferait disparaitre la trace de ce qui est parti : emettre un annule-et-remplace.",
            envois
        )));
    }
    liste.remove(i);
    ecrire(ctx, &c.dernier_jour, liste)?;
    reponse_arret(ctx, &a, Phrase::fixe("Arrêt retiré : aucune déclaration ne l'avait porté."))
}

// ------------------------------------------------------------------ la fin d'un contrat

/// Ce qui peut partir du mois : la saisie de ses engagements valides (`engagement::etats::validees_du_mois`), et leurs
/// paies ecrites ; refus si aucun engagement ne le touche.
fn validees(ctx: &Contexte, mois: Mois) -> R<(VueDuMois, Vec<Paie>)> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    engagement::etats::validees_du_mois(ctx, mois, &departs)?.ok_or_else(|| refus(format!("Aucune saisie pour {}.", mois)))
}

/// La periode du contrat d'une carte quand sa fin se lit (`Carte::contrat` : un contrat aux cachets ne l'ecrit pas, il
/// finit son dernier jour).
fn contrat_fini(e: &Carte) -> R<Option<Periode>> {
    Ok(e.contrat()?.and_then(|c| c.fermee()))
}

/// L'engagement de ce salarie (sa cle) qui se termine ce mois, la periode de son contrat (`contrat_fini`), et son rang
/// dans la saisie. `nom` : comme on l'a dit, pour la phrase. Un contrat qui a un motif de rupture passe avant les
/// autres. Un salarie sans engagement ce mois-la n'a pas de paie : la phrase le dit, avec le mois en toutes lettres.
fn engagement_qui_finit(saisie: &VueDuMois, cle: &IdSalarie, nom: &str, mois: Mois) -> R<(RangDansLeMois, Carte, Periode)> {
    let siens: Vec<(RangDansLeMois, &Carte)> = RangDansLeMois::de_chacun(&saisie.engagements).filter(|(_, x)| x.salarie.as_ref() == Some(cle)).collect();
    if siens.is_empty() {
        return Err(refus(format!("{} n'a aucun engagement en {} : aucun contrat ne s'y termine.", nom, mois.libelle(Graphie::Accentuee))));
    }
    let mut finissent: Vec<(RangDansLeMois, Carte, Periode)> = Vec::new();
    for (i, x) in siens {
        if let Some(contrat) = contrat_fini(x)?.filter(|c| c.fin().mois_civil() == mois) {
            finissent.push((i, x.clone(), contrat));
        }
    }
    // Un tri stable : les rompus d'abord, chacun a son rang.
    finissent.sort_by_key(|(_, x, _)| x.motif_rupture.is_none());
    finissent.into_iter().next().ok_or_else(|| refus(format!("{} n'a aucun contrat qui se termine en {}.", nom, mois.libelle(Graphie::Accentuee))))
}

/// La paie validee de l'engagement de rang `rang` (`paies` : celles de `validees`, au meme rang que sa saisie).
fn paie_de_l_engagement(paies: &[Paie], rang: RangDansLeMois, cle: &IdSalarie, nom: &str, mois: Mois) -> R<Paie> {
    rang.dans(paies)
        .filter(|p| p.salarie.id == *cle)
        .cloned()
        .ok_or_else(|| refus(format!("{} n'a aucune paie validee en {} pour ce contrat.", nom, mois.libelle(Graphie::Accentuee))))
}

/// `fin-contrat-documents` : le certificat de travail et le recu pour solde de tout compte (`documents::fin_de_contrat`)
/// de l'engagement du salarie qui finit ce mois.
fn fin_contrat_documents(ctx: &Contexte, c: ChargeFinContratDocuments) -> R<ReponsePapiers> {
    let mois = c.mois;
    let (saisie, paies) = validees(ctx, mois)?;
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let (rang, _, _) = engagement_qui_finit(&saisie, &cle, c.salarie.as_str(), mois)?;
    let p = paie_de_l_engagement(&paies, rang, &cle, c.salarie.as_str(), mois)?;
    let date = constitue_le(ctx, c.le);
    let dossier = documents::DossierDuMois::Sorties.dans(&ctx.racine, mois);
    // Les sorties du mois : chaque papier s'y range dans le dossier de sa sorte (`fin_de_contrat::ecrire`).
    let [certificat, recu] = documents::fin_de_contrat::ecrire(ctx, &p, date, &dossier)?;
    Ok(ReponsePapiers {
        phrase: socle::phrase!("Écrits pour {} : le certificat de travail et le reçu pour solde de tout compte.", crate::nom_a_l_ecran(&p.salarie, &cle)),
        certificat,
        recu,
        dossier,
        avertissements: documents::fin_de_contrat::AVIS,
    })
}

/// `fctu.FinDeContrat` que le signalement declarera, batie sur la saisie (`fin-contrat-signaler`).
fn fin_de_contrat_declaree(ctx: &Contexte, e: &Carte, contrat: &Periode, mois: Mois) -> R<declarations::fctu::FinDeContrat> {
    let manque = ce_qui_manque(ctx, e, contrat)?;
    let e = &fin_au_terme(e)?;
    if !manque.is_empty() {
        let liste: Vec<String> = manque.iter().map(phrase_du_manque).collect();
        return Err(refus(format!(
            "Contrat {} : le signalement de fin de contrat ne s'écrit pas encore ; il manque, dans la carte de l'engagement, sous « Contrat » :\n  {}",
            numero_dit(e.numero.as_ref(), "", "", SANS_NUMERO),
            liste.join("\n  ")
        )));
    }
    let preavis = e.preavis.as_ref().ok_or_else(|| socle::fichiers::manque("preavis"))?;
    Ok(declarations::fctu::FinDeContrat {
        fin: contrat.fin(),
        motif: e.motif_rupture.ok_or_else(|| socle::fichiers::manque("motif_rupture"))?,
        mois_rattachement: mois,
        type_preavis: preavis.genre,
        preavis_debut: preavis.debut,
        preavis_fin: preavis.fin,
        notification: e.notification_rupture,
        signature_convention: e.signature_convention,
        engagement_licenciement: e.engagement_licenciement,
        transaction: e.transaction,
    })
}

/// L'echec d'un signalement (fin de contrat, arret, reprise) : une regle de la norme qui s'oppose
/// (`FctuImpossible`, `SignalementImpossible`, une valeur `Illisible`) ou autre chose.
fn echec_fctu(e: declarations::fctu::Echec) -> Echec {
    e.en_classe(Classe::Illisible)
}

/// LE SIGNALEMENT DE FIN DE CONTRAT, ecrit dans `sorties/AAAA-MM/`. Rien n'est depose : le depot est un acte a part. Un
/// signalement normal deja parti pour ce contrat ne se refait pas : toute modification posterieure « doit faire l'objet
/// d'un signalement « Annule et remplace » ou « Annule » » (cahier technique DSN, `norme-dsn/texte/chapitre-5-modeles.txt`,
/// 5.2) ; la route le dit et n'ecrit rien. `refaire` : la piece partie, refaite pour se lire
/// (`gestes::depart::refaire_le_signalement`). L'annule-et-remplace a son nom (`_AR<ordre>`) : le fichier parti reste
/// tel qu'il est parti.
pub(crate) fn signaler_la_fin(ctx: &Contexte, c: &ChargeFinContratSignaler, refaire: bool) -> R<ReponseSignalementFin> {
    let mois = c.mois;
    let salarie = c.salarie.as_str();
    let remplace = c.remplace;
    let mode = mode_du_fichier(c.mode);

    let (saisie, paies) = validees(ctx, mois)?;
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let (rang, e, contrat) = engagement_qui_finit(&saisie, &cle, salarie, mois)?;
    let fin = fin_de_contrat_declaree(ctx, &e, &contrat, mois)?;
    if remplace.is_none() && !refaire {
        let journal = envois::journal::lu(&ctx.racine)?;
        if let Some(le) = crate::obligations::fin_signalee_le(&journal.lignes, &cle, &fin.fin)? {
            return Err(refus(format!(
                "Le signalement de fin de contrat de {} est parti le {} : il ne se refait pas. Le corriger : un annule et remplace (champ « remplace »). Rien n'est écrit.",
                cle,
                le.jj_mm_aaaa()
            )));
        }
    }
    let paie = paie_de_l_engagement(&paies, rang, &cle, salarie, mois)?;
    let type_declaration = if remplace.is_some() { TypeDeLaDeclaration::AnnuleEtRemplace } else { TypeDeLaDeclaration::Normale };
    let mut avertissements = declarations::fctu::verifier(&fin, &paie.engagement, type_declaration).map_err(echec_fctu)?;
    // LE NUMERO D'ORDRE : celui qu'on donne, sinon le suivant de tous les signalements deja partis (p. 136), lu une fois la
    // saisie verifiee : la plate-forme ne se consulte pas pour un refus.
    let (ordre, plate_forme) = ordre_du_signalement(ctx, c.ordre)?;
    avertissements.extend(plate_forme);
    let nom = nom_du_signalement(&format!("{}{}_{}", documents::SorteDePieceProduite::SignalementFinContrat.prefixe(), cle, fin.fin.aaaammjj()), remplace.is_some(), ordre)?;
    let cible = documents::PieceDuMois::SignalementFinContrat.dossier().dans(&ctx.racine, mois).join(&nom);
    compagnie::pieces_ecrites::refuser_si_partie(&ctx.racine, &cible)?;
    let options = declarations::fctu::Options {
        type_declaration,
        ordre,
        remplace,
        mode,
        date_constitution: c.le,
    };
    // Les arrets du contrat qui croisent la periode du signalement : le FCTU les reporte (bloc 60), comme la mensuelle,
    // tires du magasin des imprevus ; ses suspensions (bloc 65) en viennent deja (`imprevus::suspensions_du_mois`).
    let payee = paie.engagement.periode_payee()?;
    let arrets: Vec<declarations::dsn::Arret> = magasin(imprevus::arrets_du_contrat(ctx, &paie, &payee))?
        .into_iter()
        .map(|saisi| declarations::dsn::Arret { saisi })
        .collect();
    let texte = declarations::fctu::generer(ctx, &paie, &fin, &options, &arrets, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?).map_err(echec_fctu)?;
    declarations::dsn::ecrire(&texte, &cible)?;
    compagnie::pieces_ecrites::noter_piece_ecrite(&ctx.racine, &compagnie::pieces_ecrites::cle_de_piece(&cible), None, None)?;
    let preavis = declarations::fctu::libelle_preavis(fin.type_preavis)?;
    Ok(ReponseSignalementFin {
        suite: le_depot(&cible),
        fichier: cible,
        phrase: phrase_du_signalement(&format!("Signalement de fin de contrat écrit (préavis : {}). Rien n'est parti.", preavis), &avertissements),
        type_declaration,
        ordre,
        preavis,
        avertissements,
    })
}

/// Ce qui se lit d'un signalement écrit : ce qui est fait, puis ses avis, sur la même ligne.
fn phrase_du_signalement(fait: &str, avertissements: &[String]) -> Phrase {
    socle::phrase!("{}", std::iter::once(fait.to_string()).chain(avertissements.iter().cloned()).collect::<Vec<_>>().join("  ·  "))
}

/// Une saisie d'un arret qui se defait : un avis de prolongation, ou une reprise notee.
enum Trace<'a> {
    Prolongation(&'a Prolongation),
    Reprise(&'a RepriseNotee),
}

impl Trace<'_> {
    fn quand(&self) -> socle::paris::Moment {
        match self {
            Trace::Prolongation(p) => p.saisi_le,
            Trace::Reprise(r) => r.saisi_le,
        }
    }

    /// L'arret tel qu'il etait avant cette saisie.
    fn defaire(&self, a: &mut ArretSaisi) {
        match self {
            Trace::Prolongation(p) => {
                a.fin_previsionnelle = p.fin_avant.clone();
                a.periode_de_subrogation = p.periode_de_subrogation_avant;
            }
            Trace::Reprise(r) => {
                a.reprise = r.reprise_avant.clone();
                a.reprise_motif = r.reprise_motif_avant.clone();
            }
        }
    }
}

/// CE QU'ETAIT L'ARRET A L'INSTANT `instant` (« AAAA-MM-JJ HH:MM », `compagnie::maintenant`) : chaque saisie d'apres se
/// defait par la trace qu'elle a laissee, de la plus recente a la plus ancienne -- une prolongation (`prolongations`) rend
/// la fin prescrite et la fin de subrogation d'avant, une reprise (`reprises`) la reprise et son motif d'avant. Une saisie
/// de la minute meme a pu preceder l'instant ou le suivre : chacun de ces etats est rendu, du plus recent au plus ancien ;
/// l'empreinte de ce qui est parti dit lequel c'etait (`gestes::depart::refaire_le_signalement`).
pub(crate) fn etats_d_alors(ctx: &Contexte, salarie: &IdSalarie, dernier_jour: &Jour, instant: socle::paris::Moment) -> R<Vec<ArretSaisi>> {
    let (_, _, lu) = l_arret(ctx, salarie, dernier_jour)?;
    let mut traces: Vec<Trace> = lu.prolongations.iter().map(Trace::Prolongation).chain(lu.reprises.iter().map(Trace::Reprise)).collect();
    traces.sort_by_key(|t| t.quand());
    let apres = traces.iter().filter(|t| t.quand() > instant).count();
    let memes = traces.iter().filter(|t| t.quand() == instant).count();
    let mut a = lu.clone();
    let mut etats = Vec::new();
    if apres == 0 {
        etats.push(a.clone());
    }
    for (n, t) in traces.iter().rev().take(apres + memes).enumerate() {
        t.defaire(&mut a);
        if n + 1 >= apres {
            etats.push(a.clone());
        }
    }
    Ok(etats)
}

/// Le numero d'ordre (`S20.G00.05.004`) du prochain signalement : un de plus que le plus grand de tous ceux que
/// la compagnie a deja envoyes, arrets, reprises, fins de contrat et amorcages confondus. Cahier technique
/// DSN, p. 136 : « Pour un signalement, ce numero ne doit pas etre remis a zero afin qu'il soit unique quel que
/// soit le mois d'emission ». LA PLATE-FORME FAIT FOI : `crate::gestes::releve::dernier_ordre_connu` la consulte ; ce
/// qu'il faut en dire (elle n'a pas repondu, elle connaissait plus que le journal) part avec les avertissements.
fn prochain_ordre(ctx: &Contexte) -> R<(NumeroDOrdre, Vec<String>)> {
    let connu = crate::gestes::releve::dernier_ordre_connu(ctx, None)?;
    Ok((NumeroDOrdre::suivant(connu.dernier)?, connu.avertissements))
}

/// LE SIGNALEMENT D'ARRET (04) OU DE REPRISE (05), ecrit dans `sorties/AAAA-MM/`. Rien n'est depose : le depot est un
/// acte a part. Un signalement normal de cette nature deja parti pour cet arret ne se refait pas : une prolongation n'en
/// appelle pas de nouveau, « il suffit de declarer la nouvelle date de fin de l'arret en DSN mensuelle » (fiche
/// net-entreprises 1983, `atelier/reference/sources/net-entreprises-base-connaissances/
/// net-entreprises-fiche1983-prolongation-arret-dates-de-subrogation-20250322.txt`, l. 13) ; ce qui le corrige est un
/// annule et remplace (l. 37). La route le dit et n'ecrit rien. `alors` : la piece partie, refaite pour se lire
/// (`gestes::depart::refaire_le_signalement`) depuis l'arret tel qu'il etait a son ecriture (`etats_d_alors`) : une
/// prolongation ou une reprise saisie depuis ne la change pas. L'annule-et-remplace a son nom (`_AR<ordre>`) : le
/// fichier parti reste tel qu'il est parti.
pub(crate) fn signaler_l_arret(ctx: &Contexte, c: &ChargeArretSignaler, alors: Option<&ArretSaisi>) -> R<ReponseSignalement> {
    let nature = c.nature;
    let remplace = c.remplace;
    let arret = match alors {
        Some(a) => a.clone(),
        None => l_arret(ctx, &cle_du_nom(ctx, &c.salarie)?, &c.dernier_jour)?.2,
    };
    if remplace.is_none() && alors.is_none() {
        if let Some(le) = parti_le(ctx, &arret, nature)? {
            let quoi = match c.nature {
                NatureSignalement::Arret => ("d'arrêt", " ; une prolongation ne se signale pas, la DSN mensuelle porte la nouvelle fin"),
                NatureSignalement::Reprise => ("de reprise", ""),
            };
            return Err(refus(format!(
                "Le signalement {} de {} est parti le {} : il ne se refait pas{}. Le corriger : un annule et remplace (champ « remplace »). Rien n'est écrit.",
                quoi.0,
                arret.salarie,
                le.jj_mm_aaaa(),
                quoi.1
            )));
        }
    }
    let type_declaration = if remplace.is_some() { TypeDeLaDeclaration::AnnuleEtRemplace } else { TypeDeLaDeclaration::Normale };
    let constitution = constitue_le(ctx, c.le);
    let dsn_arret = declarations::dsn::Arret { saisi: arret.clone() };
    let mut avertissements = signalement::verifier(&dsn_arret, nature, Some(&constitution)).map_err(echec_fctu)?;
    let (ordre, plate_forme) = ordre_du_signalement(ctx, c.ordre)?;
    avertissements.extend(plate_forme);

    let dernier = arret.dernier_jour_travaille;
    let piece = match c.nature {
        NatureSignalement::Arret => documents::PieceDuMois::SignalementArret,
        NatureSignalement::Reprise => documents::PieceDuMois::SignalementReprise,
    };
    let nom = nom_du_signalement(
        &format!("{}{}_{}", piece.sorte().prefixe(), arret.salarie, dernier.aaaammjj()),
        type_declaration == TypeDeLaDeclaration::AnnuleEtRemplace,
        ordre,
    )?;
    // Arret et reprise : deux pieces du mois, la sienne dit son dossier.
    let cible = piece.dossier().dans(&ctx.racine, dernier.mois_civil()).join(&nom);
    compagnie::pieces_ecrites::refuser_si_partie(&ctx.racine, &cible)?;
    let options = signalement::Options {
        nature_declaration: nature,
        type_declaration,
        ordre,
        remplace,
        mode: mode_du_fichier(c.mode),
        date_constitution: Some(constitution),
    };
    let texte = signalement::generer(ctx, &dsn_arret, &options, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?).map_err(echec_fctu)?;
    declarations::dsn::ecrire(&texte, &cible)?;
    compagnie::pieces_ecrites::noter_piece_ecrite(&ctx.racine, &compagnie::pieces_ecrites::cle_de_piece(&cible), None, None)?;
    let fait = match c.nature {
        NatureSignalement::Arret => "Signalement d'arrêt écrit. Rien n'est parti.",
        NatureSignalement::Reprise => "Signalement de reprise écrit. Rien n'est parti.",
    };
    Ok(ReponseSignalement {
        suite: le_depot(&cible),
        fichier: cible,
        phrase: phrase_du_signalement(fait, &avertissements),
        nature: c.nature,
        type_declaration,
        ordre,
        avertissements,
    })
}

// -------------------------------------------------------------------- arrets

/// Le libelle du motif d'arret dans les codes de la norme. Un motif inconnu ne s'enregistre pas (`verifier`) : dans le
/// magasin, il se refuse.
fn libelle_motif(n: &Bloc60, a: &ArretSaisi) -> R<String> {
    match n.motifs_arret.code(a.motif) {
        Some(c) => Ok(c.libelle.clone()),
        None => Err(erreur(
            Classe::Illisible,
            format!("L'arrêt de {} au {} a un motif que la norme ne connaît pas ({}) : le corriger.", a.salarie, a.dernier_jour_travaille.iso(), a.motif),
        )),
    }
}

/// Un arret resume (`ArretResume`).
pub(crate) fn resume_arret(ctx: &Contexte, n: &Bloc60, a: &ArretSaisi) -> R<ArretResume> {
    let envois = envois_de_l_arret(ctx, a)?;
    let (signalements, mois): (Vec<_>, Vec<_>) = envois.into_iter().filter(|e| NatureSignalement::de(e.nature_declaration).is_some() || e.nature_declaration == NatureDeLaDeclaration::Mensuelle)
        .partition(|e| e.nature_declaration != NatureDeLaDeclaration::Mensuelle);
    let mois: Vec<Mois> = mois.into_iter().filter_map(|e| e.mois).collect();
    let motif_libelle = libelle_motif(n, a)?;
    let signale = !signalements.is_empty();
    // UN SIGNALEMENT EST-IL DU ? Pas pour un motif que la DSN ne couvre pas (le deuil d'enfant : `obligations-base.json`,
    // `signalement_arret.sauf_motifs`) ; son obligation est alors l'attestation de salaire hors DSN.
    let signalement_du = crate::obligations::signalement_du(a.motif)?;
    Ok(ArretResume {
        salarie: a.salarie.clone(),
        contrat: a.contrat_numero.clone(),
        contrat_debut: a.contrat_debut,
        motif: a.motif,
        motif_libelle,
        dernier_jour_travaille: a.dernier_jour_travaille,
        debut: a.debut,
        fin_previsionnelle: a.fin_previsionnelle,
        subrogation: a.subrogation,
        periode_de_subrogation: a.periode_de_subrogation,
        date_accident: a.date_accident,
        reprise: a.reprise,
        reprise_motif: a.reprise_motif,
        note: a.note.clone(),
        saisi_le: a.saisi_le,
        signale,
        signalement_du,
        signalements,
        reporte_par_dit: mois_de_la_dsn(&mois),
        reporte_par: mois,
        etat: if signale {
            EtatDuSignalement::Signale
        } else if signalement_du {
            EtatDuSignalement::ASignaler
        } else {
            EtatDuSignalement::SansSignalement
        },
    })
}

/// La route `arrets` : les arrets de travail du magasin des imprevus, chacun resume comme `imprevus` le resume
/// (`resume`), avec ses obligations, ceux qu'aucun signalement n'a annonces, et les nomenclatures du bloc 60.
fn arrets(ctx: &Contexte) -> R<ReponseArrets> {
    let n = nomenclatures()?;
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    let mut resumes = Vec::new();
    let mut phrases = Vec::new();
    for a in tous_les_arrets(ctx)? {
        let r = resume_d_arret_complet(ctx, &n, &a, &suivi, ctx.aujourdhui)?;
        // Ce que l'ecran lit (Exports › Avancé) : qui, quand, le motif, ce qui reste a signaler, puis ses obligations, une
        // ligne chacune.
        let b = &r.base;
        phrases.push(format!(
            "{} : dernier jour travaillé le {}, fin prévue le {} — {} — {}",
            b.salarie,
            b.dernier_jour_travaille.jj_mm_aaaa(),
            b.fin_previsionnelle.jj_mm_aaaa(),
            b.motif_libelle,
            b.etat.dit()
        ));
        phrases.extend(crate::obligations::en_phrases(&r.obligations).into_iter().map(|o| format!("  {}", o)));
        resumes.push(Resume::Arret(Box::new(r)));
    }
    let a_signaler: Vec<Resume> = resumes.iter().filter(|r| matches!(r, Resume::Arret(x) if !x.base.signale && x.base.signalement_du)).cloned().collect();
    Ok(ReponseArrets {
        phrase: socle::phrase!("{}, {} à signaler.", socle::ecran::compte(resumes.len(), "arrêt de travail", "arrêts de travail"), a_signaler.len()),
        phrases,
        arrets: resumes,
        a_signaler,
        motifs: n.motifs_arret.saisissables(),
        motifs_reprise: n.motifs_reprise.saisissables(),
        subrogation: n.subrogation.codes,
    })
}

/// UN ARRET RESUME AVEC SES OBLIGATIONS, comptees au jour `jour` (`obligations::de_l_arret`), et ce qui en est parti : ce
/// que la route `arrets` et les rappels lisent.
pub(crate) fn resume_d_arret_complet(ctx: &Contexte, n: &Bloc60, a: &ArretSaisi, suivi: &crate::obligations::Suivi, jour: Jour) -> R<ResumeDArret> {
    let imp = Imprevu::Arret(a.clone());
    let mut r = resume_d_arret(ctx, n, a, cachets_de(ctx, &imp)?, magasin(imprevus::premier_jour(&imp))?, magasin(imprevus::dernier_jour(&imp))?)?;
    r.obligations = crate::obligations::de_l_arret(ctx, a, suivi, jour)?;
    r.dire_ce_qui_est_parti()?;
    Ok(r)
}

// ----------------------------------------------------------- fins de contrat

/// La nature d'un engagement : dite (regle 6) ; sinon `Manque`.
fn nature_de(e: &Carte) -> R<std::sync::Arc<droit::natures::Nature>> {
    let Some(n) = e.nature else {
        return Err(socle::fichiers::manque("nature"));
    };
    // Une nature dite qui ne se lit pas (les donnees livrees) est une panne : elle remonte telle quelle.
    n.proprietes()
}

/// CE QUI MANQUE A UN SIGNALEMENT, tel que l'ecran le dit : `champ` (la donnee de l'engagement, `data-fait` de sa carte),
/// `case` (le nom de la case, sous « Contrat »), `dire` (ce qu'on y met, en mots d'utilisateur, visible sans rien
/// deplier) et `pourquoi` (la regle de la norme).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Manque {
    champ: &'static str,
    case: &'static str,
    dire: String,
    pourquoi: String,
}

fn manque(champ: &'static str, case: &'static str, dire: impl Into<String>, pourquoi: impl Into<String>) -> Manque {
    Manque { champ, case, dire: dire.into(), pourquoi: pourquoi.into() }
}

/// Le type de preavis « 90 - pas de clause de preavis applicable » (`fctu-base.json`, S21.G00.63.001).
const SANS_PREAVIS: droit::preavis::TypeDePreavis = droit::preavis::TypeDePreavis::SansClause;

/// UN CONTRAT A DUREE DETERMINEE QUI VA A SON TERME n'a rien a dire de sa fin : son motif est celui de sa nature
/// (`contrats-base.json`, `motif_par_defaut` : 031, « la fin normale d'un CDD arrive a son terme »), et il n'a pas de
/// preavis -- il « cesse de plein droit a l'echeance du terme » (L1243-5, `permanents-base.json`, `motifs_sans_preavis`,
/// 031) --, ce que le bloc S21.G00.63 declare en type 90. L'engagement tel que le signalement le lit : sans motif de
/// rupture saisi, ces deux faits ; avec un motif saisi, tel quel.
fn fin_au_terme(e: &Carte) -> R<Carte> {
    let mut e = e.clone();
    if e.motif_rupture.is_some() {
        return Ok(e);
    }
    if let Some(m) = nature_de(&e)?.motif_rupture_par_defaut() {
        e.motif_rupture = Some(m);
        if e.preavis.is_none() {
            e.preavis = Some(Preavis { genre: SANS_PREAVIS, debut: None, fin: None });
        }
    }
    Ok(e)
}

/// La phrase d'un manque (`ce_qui_manque`) : sa case, son nom dans la saisie de la carte, et ce qu'on y met.
pub(crate) fn phrase_du_manque(m: &Manque) -> String {
    format!("« {} » (`{}`) : {}", m.case, m.champ, m.dire)
}

/// Ce que dit la loi du preavis d'une rupture, et la convention a cote (`moteur preavis`), pour la case « Préavis ».
fn preavis_dit(ctx: &Contexte, e: &Carte, contrat: &Periode) -> R<String> {
    let anciennete = contrat.anciennete();
    let motif = e.motif_rupture;
    let cdd = !nature_de(e)?.duree_indeterminee;
    let duree = droit::permanents::preavis_mois(anciennete, motif, cdd)?;
    let regle = droit::permanents::regle_du_preavis(motif, cdd)?;
    let mut dit = match (duree, &regle) {
        (Some(m), _) if !m.est_nul() => format!("La loi en donne {} mois (L1234-1).", socle::grandeurs::ecrit::texte::dit(m)),
        (Some(_), _) => match droit::permanents::raison_sans_preavis(motif, cdd)? {
            Some(r) => format!("La loi ne donne aucun préavis pour ce motif : {}", r),
            None => "La loi ne donne aucun préavis pour ce motif.".to_string(),
        },
        (None, Some(r)) => format!("La loi en donne un que le moteur ne compte pas : {}", r),
        (None, None) => "Sous six mois d'ancienneté, la loi renvoie à la convention ou aux usages (L1234-1) : elle ne dit pas de durée.".to_string(),
    };
    // Ce que dit la convention, a cote, par le moteur (le calcul de `moteur preavis`) ; pas pour un motif sans preavis
    // ni pour la rupture d'un CDD par le salarie (L1243-2), que la convention du licenciement ne regle pas.
    if !duree.is_some_and(|m| m.est_nul()) && regle.is_none() {
        let tout = engagement::preavis(ctx, e, contrat, motif)?;
        let hors_la_loi = tout.duree.as_ref().filter(|d| d.source != engagement::solde::SourceDuPreavis::Loi);
        if let (Some(conv), Some(d)) = (tout.convention, hors_la_loi) {
            dit.push_str(&format!(" {} en donne {} : c'est elle qui s'applique.", d.qui(), socle::grandeurs::ecrit::texte::dit(conv)));
        } else if tout.duree.is_none() {
            if let Some(pourquoi) = &tout.pourquoi {
                dit.push_str(&format!(" Et {}.", pourquoi.trim_end_matches('.')));
            }
        }
    }
    Ok(dit)
}

/// Ce qu'un signalement exige et que la saisie ne dit pas encore, un manque par case de la carte (`Manque`).
fn ce_qui_manque(ctx: &Contexte, e: &Carte, contrat: &Periode) -> R<Vec<Manque>> {
    let e = &fin_au_terme(e)?;
    let mut manques = Vec::new();
    match e.motif_rupture {
        None => manques.push(manque(
            "motif_rupture",
            "Motif de la rupture",
            "pourquoi le contrat a pris fin.",
            "Le motif de la rupture (S21.G00.62.002) est obligatoire, et un CDI n'en a aucun par défaut.",
        )),
        // CCH-12 de S21.G00.62.003 : la date de notification, obligatoire pour ces motifs (`preavis::notification_exigee`).
        Some(motif) if e.notification_rupture.is_none() && droit::preavis::notification_exigee(motif)? => manques.push(manque(
            "notification_rupture",
            "Notifiée le",
            "le jour où la rupture a été notifiée : la démission remise, la lettre de rupture ou de licenciement envoyée, la \
fin de la période d'essai notifiée.",
            format!(
                "La date de notification de la rupture (S21.G00.62.003) est obligatoire pour le motif {} (CCH-12, cahier \
technique DSN p. 275). Le signalement ne s'écrit pas sans elle.",
                motif
            ),
        )),
        Some(_) => {}
    }
    match &e.preavis {
        None => manques.push(manque(
            "preavis",
            "Préavis",
            format!("le préavis de la rupture, ses dates s'il y en a un, ou « pas de clause de préavis ». {}", preavis_dit(ctx, e, contrat)?),
            "Le bloc S21.G00.63 est obligatoire dans un signalement de fin de contrat (cardinalité 1,*) : même sans préavis, il \
se déclare, avec le type « 90 - pas de clause de préavis applicable ».",
        )),
        Some(p) if declarations::fctu::preavis_non_effectue(p.genre)? && e.dernier_jour_paye.is_none() => manques.push(manque(
            "dernier_jour_paye",
            "Dernier jour payé",
            "le dernier jour travaillé et payé au salaire habituel, avant le préavis qui n'est pas effectué.",
            "Sur un préavis non effectué, le dernier jour travaillé et payé au salaire habituel (S21.G00.62.006) n'est pas la fin \
du contrat, il précède le début du préavis (CCH-12, cahier technique DSN p. 275). La mensuelle et le signalement le \
déclarent tous deux.",
        )),
        Some(_) => {
            // Hors preavis non effectue, aucun jour de la paie ne vient apres le dernier jour paye.
            if let Some(dernier) = e.dernier_jour_paye {
                let apres: Vec<Jour> = e.dates().iter().map(|j| j.jour()).filter(|j| *j > dernier).collect();
                if !apres.is_empty() {
                    let dits: Vec<String> = apres.iter().map(|j| format!("le {}", j.jj_mm())).collect();
                    manques.push(manque(
                        "dernier_jour_paye",
                        "Dernier jour payé",
                        format!(
                            "{} {} après le dernier jour payé, le {} : retirez-le de la paie, ou dites un autre dernier jour payé.",
                            dits.join(", "),
                            if apres.len() > 1 { "sont" } else { "est" },
                            dernier.jj_mm()
                        ),
                        "Le dernier jour travaillé et payé (S21.G00.62.006) clôt la paie du contrat rompu : un jour payé après lui \
contredit le signalement.",
                    ));
                }
            }
        }
    }
    Ok(manques)
}

/// UNE FIN DE CONTRAT QUI SE SIGNALE, telle que la colonne du mois et les rappels la lisent : l'engagement (`salarie`,
/// `numero`, `motif_rupture`, `preavis`, tels que la saisie les ecrit, et la periode de son contrat, `contrat_fini`), sa
/// nature, ce qui manque pour la signaler (`a_saisir`, et ce que l'ecran en dit : `avant_signaler`, `la_dire`), son
/// obligation (`obligations::de_la_fin`), sa pastille, son contrat en clair (`contrat_dit`), son rang parmi ce qui est
/// du (`rang`) et ses gestes, le nom de son salarie (`qui`, le titre de la boite de ses gestes) ; la route
/// `fins-de-contrat` y ajoute qui, quoi et quand (`qui_dit`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FinLue {
    pub salarie: IdSalarie,
    qui: String,
    pub mois: Mois,
    pub contrat: Periode,
    pub en_retard: bool,
    pub numero: Option<NumeroContrat>,
    /// Le libelle de sa nature de contrat.
    #[serde(rename = "nature")]
    libelle_nature: String,
    motif_rupture: Option<droit::natures::MotifDeRupture>,
    preavis: Option<Preavis>,
    pub a_saisir: Vec<Manque>,
    pub du: bool,
    pastille: Pastille,
    pub obligation: crate::obligations::Obligation,
    #[serde(skip_serializing_if = "Option::is_none")]
    qui_dit: Option<String>,
    contrat_dit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    avant_signaler: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    la_dire: Option<VersLaCase>,
    rang: String,
    gestes: Vec<GesteDEcheance>,
}

/// LE MOT QUI MENE A LA CASE QUI MANQUE A UNE FIN (`a_saisir`), dans la carte de son contrat : son libelle et son rang
/// (`RangDuGeste`, la page en tire la forme du bouton).
#[derive(Debug, Clone, Serialize)]
struct VersLaCase {
    libelle: &'static str,
    rang: RangDuGeste,
}

/// Ce que l'ecran dit d'une fin a qui il manque une case, et le mot qui mene a la case, un geste second.
const LA_DIRE: VersLaCase = VersLaCase { libelle: "La dire", rang: RangDuGeste::Second };

/// LA FIN D'UNE CARTE VALIDEE QUI NE SE LIT PLUS (son contrat finit avant de commencer, son salarie n'est plus dit) : on
/// ne sait ni si elle finit ce mois-ci, ni si elle se signale. Elle ne porte que ce qu'il faut corriger (`a_corriger`)
/// et ou (la carte de son `mois`, son `numero`) ; aucun acte ne se propose sur elle. La route `fins-de-contrat` y ajoute
/// sa phrase (`qui_dit`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FinACorriger {
    pub mois: Mois,
    #[serde(skip_serializing_if = "Option::is_none")]
    salarie: Option<IdSalarie>,
    numero: Option<NumeroContrat>,
    a_corriger: Manque,
    pastille: Pastille,
    #[serde(skip_serializing_if = "Option::is_none")]
    qui_dit: Option<String>,
    rang: &'static str,
}

/// UNE FIN DE LA LISTE DES FINS A SIGNALER : lue (`FinLue`, son salarie et son contrat dits), ou sur une carte a corriger
/// (`FinACorriger`), qui reste listee, seule, et ne ferme ni la liste, ni les rappels, ni l'horloge.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "lecture", rename_all = "snake_case")]
pub(crate) enum FinASignaler {
    Lue(FinLue),
    ACorriger(FinACorriger),
}

impl FinASignaler {
    /// La fin lue ; `None` : une carte a corriger, sur qui rien ne se fait.
    pub(crate) fn lue(self) -> Option<FinLue> {
        match self {
            FinASignaler::Lue(f) => Some(f),
            FinASignaler::ACorriger(_) => None,
        }
    }

    /// Est-elle due (la colonne « Ce qui est dû » la montre) : une carte a corriger, ou une fin due.
    fn due(&self) -> bool {
        match self {
            FinASignaler::Lue(f) => f.du,
            FinASignaler::ACorriger(_) => true,
        }
    }

    /// Son rang parmi ce qui est du.
    fn rang(&self) -> &str {
        match self {
            FinASignaler::Lue(f) => &f.rang,
            FinASignaler::ACorriger(f) => f.rang,
        }
    }
}

/// Les contrats qui finissent dans `mois` et dont la fin SE SIGNALE, chacun avec
/// son obligation (`obligations::de_la_fin`) comptee au jour `aujourdhui`. UN SEUL ENDROIT : la route `fins-de-contrat` (la
/// colonne du mois) et les rappels de l'ouverture (`rappels`) la lisent.
pub(crate) fn fins_a_signaler(ctx: &Contexte, mois: Mois, aujourdhui: Jour) -> R<Vec<FinASignaler>> {
    // Un signalement de fin ne se doit que pour un engagement valide, lu tel qu'il est ecrit (`tels_qu_ecrits`) : un
    // brouillon ne part pas, et ce qui ne fait que lire ne recalcule rien. Un mois sans saisie n'a aucune fin.
    let saisie = engagement::stock::vue_du_mois(&ctx.racine, mois)?;
    let cartes: Vec<&engagement::saisie::Carte> = saisie.iter().flat_map(|s| s.engagements.iter()).collect();
    if cartes.is_empty() {
        return Ok(Vec::new());
    }
    let journal = envois::journal::lu(&ctx.racine)?;
    let etats = engagement::etats::tels_qu_ecrits(ctx, mois, &journal.departs)?;
    let mut out = Vec::new();
    for (i, carte) in cartes.into_iter().enumerate() {
        if etats.get(i).and_then(|x| x.valide()).is_none() {
            continue;
        }
        let e = carte;
        // UNE CARTE VALIDEE QUI NE SE LIT PLUS (son contrat finit avant de commencer, son salarie n'est plus dit) reste
        // dans la liste, seule, avec ce qu'il faut corriger (`FinACorriger`). Une nature qui manque est un manque : la fin
        // reste dans la liste, sur elle seule, avec ce qui lui manque. Une panne (le disque, une nature que les donnees
        // livrees ne lisent pas) remonte.
        let contrat = match contrat_fini(e) {
            Ok(c) => c,
            Err(err) if err.dit_un_manque() => {
                out.push(fin_a_corriger(e, mois, manque("debut", "Contrat", err.phrase.clone(), "Le contrat de la carte ne se lit plus : on ne sait pas s'il finit ce mois-ci, ni si sa fin se signale.")));
                continue;
            }
            Err(err) => return Err(err),
        };
        let Some(contrat) = contrat.filter(|c| c.fin().mois_civil() == mois) else { continue };
        let Some(salarie) = e.salarie.clone() else {
            out.push(fin_a_corriger(e, mois, manque("salarie", "Salarié", "le salarié de cette carte.", "Une carte validée dit son salarié : sans lui, sa fin ne se signale pas.")));
            continue;
        };
        let Some(n) = e.nature else {
            out.push(fin_a_corriger(
                e,
                mois,
                manque(
                    "nature",
                    "Nature",
                    "la nature du contrat.",
                    "La nature dit si la fin du contrat se signale (`Nature::signalement_fctu`) : sans elle, on ne sait pas.",
                ),
            ));
            continue;
        };
        let nat = n.proprietes()?;
        // La regle est celle de la nature, du motif et de ce que la DSN sait du contrat, au calendrier
        // (`Nature::signalement_fctu`, `ConnuDeLaDsn::du_contrat`) : un CDDU rompu avant son terme, ou deja declare par la
        // DSN d'un mois precedent, se signale.
        if !nat.signalement_fctu(e.motif_rupture, droit::natures::ConnuDeLaDsn::du_contrat(contrat.debut(), mois)) {
            continue;
        }
        let a_saisir = ce_qui_manque(ctx, e, &contrat)?;
        // LE DELAI ET OU ON EN EST (`obligations::de_la_fin`).
        let obligation = crate::obligations::de_la_fin(ctx, Some(&salarie), contrat.fin(), &journal.lignes, aujourdhui)?;
        // Due : il manque de quoi signaler, ou le delai court. Sa pastille : celle du delai passe
        // (« en retard » l'emporte : la date butoir ne se tait pas parce qu'une case manque), sinon ce qui manque, sinon
        // celle du delai.
        let incomplet = !a_saisir.is_empty();
        let du = incomplet || obligation.du;
        let en_retard = obligation.etat == crate::obligations::EtatDObligation::EnRetard;
        let pastille = if incomplet && !en_retard { Pastille::de(socle::ecran::EtatDEcran::AFaire, "à compléter") } else { obligation.pastille.clone() };
        let contrat_dit = format!("contrat {}, fin le {}", numero_dit(e.numero.as_ref(), "", "", SANS_NUMERO), contrat.fin().jj_mm_aaaa());
        let (avant_signaler, la_dire) = if incomplet { (Some(crate::ouverture::aide("a_dire_avant_signaler")?), Some(LA_DIRE)) } else { (None, None) };
        let mut fin = FinLue {
            qui: ctx.nomme(&salarie),
            salarie,
            mois,
            contrat,
            en_retard,
            numero: e.numero.clone(),
            libelle_nature: nat.libelle.clone(),
            motif_rupture: e.motif_rupture,
            preavis: e.preavis.clone(),
            a_saisir,
            du,
            pastille,
            rang: obligation.rang(),
            obligation,
            qui_dit: None,
            contrat_dit,
            avant_signaler,
            la_dire,
            gestes: Vec::new(),
        };
        // Une fin qui n'est pas due n'offre aucun geste.
        if du {
            fin.gestes = gestes_d_une_fin(&fin)?;
        }
        out.push(FinASignaler::Lue(fin));
    }
    Ok(out)
}

/// La fin d'une carte validee qui ne se lit plus (`fins_a_signaler`) : ce qu'il faut corriger, et ou.
fn fin_a_corriger(e: &Carte, mois: Mois, a_corriger: Manque) -> FinASignaler {
    FinASignaler::ACorriger(FinACorriger {
        mois,
        salarie: e.salarie.clone(),
        numero: e.numero.clone(),
        a_corriger,
        pastille: Pastille::de(socle::ecran::EtatDEcran::AFaire, "à corriger"),
        qui_dit: None,
        rang: RANG_D_UNE_CARTE_A_CORRIGER,
    })
}

/// La route `fins-de-contrat`.
fn fins_de_contrat(ctx: &Contexte, c: ChargeMois) -> R<ReponseFins> {
    let mut fins = fins_en_retard_avant(ctx, c.mois, ctx.aujourdhui)?;
    fins.extend(fins_a_signaler(ctx, c.mois, ctx.aujourdhui)?);
    // Ce que l'ecran lit de chaque fin : qui, quelle nature, quel jour (et le mois d'une fin d'un mois passe), et son
    // contrat ; d'une carte a corriger, qui et ou.
    for f in &mut fins {
        match f {
            FinASignaler::Lue(f) => {
                let fin = f.contrat.fin().jj_mm_aaaa();
                let ailleurs = if f.mois != c.mois { format!(" ({})", f.mois.libelle(Graphie::Accentuee)) } else { String::new() };
                f.qui_dit = Some(format!("{}, {}, fin le {}{}", ctx.nomme(&f.salarie), f.libelle_nature, fin, ailleurs));
            }
            FinASignaler::ACorriger(f) => {
                let qui = match f.salarie.as_ref() {
                    Some(s) => ctx.nomme(s),
                    None => "Une carte sans salarié".to_string(),
                };
                f.qui_dit = Some(format!(
                    "{}, contrat {} : à corriger dans la carte de {}",
                    qui,
                    numero_dit(f.numero.as_ref(), "", "", SANS_NUMERO),
                    f.mois.libelle(Graphie::Accentuee)
                ));
            }
        }
    }
    // Une carte a corriger compte parmi les fins : la phrase le dit.
    let a_corriger = fins.iter().filter(|f| matches!(f, FinASignaler::ACorriger(_))).count();
    let phrase = match a_corriger {
        0 => socle::phrase!("Fins de contrat du mois : {}.", fins.len()),
        n => socle::phrase!("Fins de contrat du mois : {}, dont {} à corriger.", fins.len(), n),
    };
    // CE QUI EST DU : une carte a corriger, une fin due ; dans leur rang.
    let mut dus: Vec<FinASignaler> = fins.iter().filter(|f| f.due()).cloned().collect();
    dus.sort_by(|x, y| x.rang().cmp(y.rang()));
    let dus_titre = (!dus.is_empty()).then_some(CE_QUI_EST_DU);
    Ok(ReponseFins { phrase, mois: c.mois, fins, dus_titre, dus })
}

/// Combien de mois en arriere un signalement de fin en retard se rappelle : une annee de paies.
const MOIS_EN_ARRIERE: socle::date::Duree<socle::date::EnMois> =socle::date::Duree::mois(12);

/// LES FINS D'AVANT, EN RETARD : celles des mois qui precedent `mois` (sur `MOIS_EN_ARRIERE`) dont le signalement est du
/// et sa date butoir passee. Une echeance manquee reste due -- le signalement se fait « dans les cinq jours ouvres qui
/// suivent la survenance de la fin du contrat » (`obligations::de_la_fin`, `obligations-base.json`) -- et elle se dit sur
/// le mois qu'on regarde, pas seulement sur celui ou le contrat a fini.
pub(crate) fn fins_en_retard_avant(ctx: &Contexte, mois: Mois, aujourdhui: Jour) -> R<Vec<FinASignaler>> {
    let mut out = Vec::new();
    // Du plus ancien au mois d'avant.
    for m in mois.avant(MOIS_EN_ARRIERE)?.jusqu_a(mois.precedent()) {
        out.extend(fins_a_signaler(ctx, m, aujourdhui)?.into_iter().filter(|f| matches!(f, FinASignaler::Lue(l) if l.du && l.en_retard)));
    }
    Ok(out)
}

// ------------------------------------------------------------ les contrats d'un jour

/// Les contrats de la fiche `cle` qui couvrent `jour`, lus dans les saisies, du plus ancien debut au plus recent.
fn contrats_au(ctx: &Contexte, cle: &IdSalarie, jour: Jour) -> R<Vec<ContratAuJour>> {
    let mut sorties: Vec<ContratAuJour> = Vec::new();
    let mut vus: BTreeSet<(Jour, Option<NumeroContrat>)> = BTreeSet::new();
    let mut mois = engagement::stock::mois_saisis(&ctx.racine)?;
    mois.reverse();
    for m in mois {
        for (c, e) in crate::mois::engagements_du_mois(ctx, m)? {
            if c != *cle {
                continue;
            }
            let Some(contrat) = e.contrat()? else { continue };
            // Un contrat paye au mois sans fin ecrite (un CDI qui court) couvre tout jour depuis son debut : c'est sur lui
            // qu'un arret ou une suspension se pose. Un contrat dont la nature n'est pas encore dite n'est pas tenu pour
            // paye au mois : il ne couvre que ses bornes. Le mois le plus recent qui le saisit fait foi (une fin ecrite
            // plus tard l'emporte) : il est retenu, couvrant ou non, avant les mois plus anciens.
            let mensuel = e.nature.map(droit::natures::NatureDeContrat::proprietes).transpose().map_err(|e| e.reclasser(Classe::Refuse))?.is_some_and(|n| n.mensuelle());
            if contrat.fin().is_none() && !mensuel {
                continue;
            }
            if !vus.insert((contrat.debut(), e.numero.clone())) {
                continue;
            }
            if !contrat.contient(&jour) {
                continue;
            }
            sorties.push(ContratAuJour {
                nom: ctx.nomme(cle),
                numero: e.numero.clone(),
                periode: contrat,
                nature: e.nature,
            });
        }
    }
    sorties.sort_by(|a, b| a.periode.debut().cmp(&b.periode.debut()));
    Ok(sorties)
}

/// Les mois des DSN qui ont porte un imprevu, tels que l'ecran les lit (« de novembre 2026 »).
fn mois_de_la_dsn(mois: &[Mois]) -> Vec<String> {
    mois.iter().map(|&m| crate::regularisation::de_mois_en_clair(m)).collect()
}
