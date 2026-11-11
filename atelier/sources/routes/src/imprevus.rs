//! Les routes des IMPREVUS d'un salarie : la liste (`imprevus`, `arrets`, `fins-de-contrat`, `arret-contrats`), l'arret
//! saisi, repris, prolonge, signale ou oublie (`arret-enregistrer`, `arret-reprise`, `arret-prolonger`,
//! `arret-signaler`, `arret-oublier`), ses indemnites journalieres (`arret-indemnites`), la suspension du bloc 65
//! (`suspension-enregistrer`, `-modifier`, `-oublier`), et la fin d'un contrat signalee, avec ses documents
//! (`fin-contrat-signaler`, `fin-contrat-documents`).
//!
//! RIEN NE PART D'ICI : un imprevu s'ecrit dans `imprevus/AAAA-MM.json`, et c'est la production du mois qui en tire la
//! ligne de paie et le bloc de la DSN ; un signalement s'ecrit en fichier, il ne s'envoie pas. Les refus sont des
//! `Refuse` (la page les montre tels quels) ; un refus n'ecrit rien.
//!
//! CE QUI S'Y SAISIT, ET CE QUI L'APPELLE (rien de remplissable s'il n'y a pas a le remplir). Le moteur dit, sur chaque
//! imprevu resume (`resume`), ce que ses faits ouvrent : ses `gestes` (`GesteDImprevu`), ceux seulement que l'imprevu
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

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::enveloppe::{en_json, route_inconnue, Charge, Dite, Porte, Rendu};
use compagnie::{maintenant, INTERFACE};
use crate::SansCharge;
use socle::ecran::Pastille;
use declarations::signalement;
use droit::contexte::{Contexte, IdSalarie};
use droit::nature_declaration::NatureDeLaDeclaration;
use engagement::calcul::Paie;
use droit::arrets::{Bloc60, CodeDuBloc60};
use engagement::imprevus::{self, ArretSaisi, GenreSuspension, Imprevu, Prolongation, RepriseNotee, Suspend, SuspensionSaisie, ARRET, SUSPENSION};
use engagement::numeros::NumeroContrat;
use engagement::saisie::{Carte, Preavis, VueDuMois};
use engagement::Chiffre;
use socle::date::Jour;
use socle::echec::{erreur, Classe, Echec, R};
use socle::mois::{Graphie, Mois};
use socle::num::Decimal;

use crate::catalogue::{Acte, Affiche, Champ, Forme, Fiche, FORME, ILLISIBLE, MANQUE, MOIS, OPERATION, SALARIE};
use crate::catalogue::Route;
use crate::catalogue::{Ecran, Place};
use crate::metier::imprevus::{arret_a_ecrire, magasin, nomenclatures, tous_les_arrets, ArretDit, ContratAuJour};

const DERNIER_JOUR: Champ = Champ { nom: "dernier_jour", libelle: Affiche::new("Dernier jour travaillé"), forme: Forme::Date, sens: "le dernier jour travaillé avant l'arrêt, qui le désigne", avance: false };
const DEBUT: Champ = Champ { nom: "debut", libelle: Affiche::new("Premier jour d'absence"), forme: Forme::Date, sens: "le premier jour de l'absence, qui la désigne", avance: false };
const UN_ARRET: &str = r#"{"salarie":"FULLER","dernier_jour":"2026-11-27"}"#;

/// Une fiche de ce module : `rend` et les refus que toutes partagent.
const fn fiche(route: Route, geste: &'static str, charge: &'static [Champ], exemple: &'static str, rend: &'static str, acte: Acte, ecran: Ecran) -> Fiche {
    Fiche { route, geste, charge, exemple, rend, refus: &[OPERATION, MANQUE, ILLISIBLE, FORME], acte, phrase: 0, ecran }
}

const ORDRE: Champ = Champ { nom: "ordre", libelle: Affiche::new("Numéro d'ordre"), forme: Forme::Nombre, sens: "son numéro d'ordre ; sinon le suivant de ceux déjà partis", avance: true };
const REMPLACE: Champ = Champ { nom: "remplace", libelle: Affiche::new("Remplace"), forme: Forme::Texte, sens: "le signalement qu'il remplace", avance: true };
const TEST: Champ = Champ { nom: "test", libelle: Affiche::new("Fichier d'essai"), forme: Forme::OuiNon, sens: "vrai : un fichier d'essai, marqué comme tel", avance: true };

/// Ou ces routes se voient a l'ecran (une place par module) : sur le chemin simple, ou hors de lui.
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);
const AVANCE: Ecran = Ecran::DessineAvance(Place::Paie);
/// Les routes qu'un imprevu offre en gestes (`GesteDImprevu`) : la page les atteint par le geste, dans une boite tiree
/// de leur fiche.
const GESTE: Ecran = Ecran::Geste(Place::Paie);
const GESTE_AVANCE: Ecran = Ecran::GesteAvance(Place::Paie);

// Les cases que la boite d'un geste fait saisir : celles de la fiche de sa route, nommees par le geste (`cases`).
const FIN_PRESCRITE: Champ = Champ { nom: "fin", libelle: Affiche::new("Nouvelle fin prescrite"), forme: Forme::Date, sens: "la nouvelle fin prescrite", avance: false };
const REPRISE: Champ = Champ { nom: "reprise", libelle: Affiche::new("Reprise le"), forme: Forme::Date, sens: "le jour de la reprise ; nul l'efface", avance: false };
const MOTIF_REPRISE: Champ = Champ { nom: "motif_reprise", libelle: Affiche::new("Motif de la reprise"), forme: Forme::Texte, sens: "le motif de la reprise", avance: true };
const IJ_BRUTE: Champ = Champ { nom: "ij_brute", libelle: Affiche::new("Indemnité brute d'un jour"), forme: Forme::Texte, sens: "l'indemnité d'un jour, avant CSG et CRDS (relevé de la CPAM), en texte (« 40.89 ») ; absente, elle ne change pas", avance: false };
const IJ_NETTE: Champ = Champ { nom: "ij_nette", libelle: Affiche::new("Indemnité nette d'un jour"), forme: Forme::Texte, sens: "la même, nette de CSG et CRDS", avance: false };
const IJ_BRUTE_MAJOREE: Champ = Champ { nom: "ij_brute_majoree", libelle: Affiche::new("Indemnité brute majorée"), forme: Forme::Texte, sens: "accident du travail, dès le 29e jour : brute", avance: true };
const IJ_NETTE_MAJOREE: Champ = Champ { nom: "ij_nette_majoree", libelle: Affiche::new("Indemnité nette majorée"), forme: Forme::Texte, sens: "la même, nette", avance: true };
const JOURS_DEJA_INDEMNISES: Champ = Champ { nom: "jours_deja_indemnises", libelle: Affiche::new("Jours déjà indemnisés"), forme: Forme::Nombre, sens: "les jours indemnisés dans les douze mois qu'aucun arrêt saisi ne porte", avance: true };
const NOUVELLE_FIN: Champ = Champ { nom: "fin", libelle: Affiche::new("Nouvelle fin"), forme: Forme::Date, sens: "sa nouvelle fin", avance: false };

/// Les routes de ce module, et leurs fiches, dans l'ordre d'un arrêt : il s'enregistre, se suit, se signale, s'oublie.
pub const ROUTES: &[Fiche] = &[
    fiche(Route::Imprevus, "Lire les imprévus d'un mois ou d'un salarié, et ce qu'on peut en saisir.",
        &[MOIS, Champ { nom: "salarie", libelle: Affiche::new("Salarié"), forme: Forme::Texte, sens: "à la place du mois : les imprévus de ce salarié", avance: true }],
        r#"{"mois":"2026-11"}"#, "imprevus (chacun résumé, un arrêt avec ses obligations), nomenclatures", Acte::Lit, ECRAN),
    fiche(Route::Arrets, "Lire tous les arrêts de la compagnie.", &[], "{}", "phrase, phrases (qui, quand, le motif, ce qui reste à signaler), arrets : chacun résumé", Acte::Lit, Ecran::Auto(Place::Exports, Affiche::new("Lire tous les arrêts"))),
    fiche(Route::FinsDeContrat, "Lire les fins de contrat dues ce mois, et ce qui manque à chacune.", &[MOIS],
        r#"{"mois":"2026-11"}"#, "fins : chacune avec ce qui est fait et ce qui manque", Acte::Lit, ECRAN),
    fiche(Route::ArretContrats, "Lire les contrats qu'un arrêt suspendrait à ce jour.",
        &[SALARIE, Champ { nom: "jour", libelle: Affiche::new("Dernier jour travaillé"), forme: Forme::Date, sens: "le dernier jour travaillé", avance: false }],
        r#"{"salarie":"FULLER","jour":"2026-11-27"}"#, "contrats : ceux du salarié qui couvrent ce jour", Acte::Lit, ECRAN),
    Fiche { phrase: 400, ..fiche(Route::ArretEnregistrer, "Enregistrer un arrêt de travail.",
        &[
            SALARIE, DERNIER_JOUR,
            Champ { nom: "debut", libelle: Affiche::new("Début de l'arrêt"), forme: Forme::Date, sens: "le premier jour que l'avis prescrit ; le dernier jour travaillé s'il a été prescrit ce jour-là", avance: false },
            Champ { nom: "fin", libelle: Affiche::new("Fin prescrite"), forme: Forme::Date, sens: "la fin prescrite", avance: false },
            Champ { nom: "motif", libelle: Affiche::new("Motif"), forme: Forme::Texte, sens: "le code du motif d'arrêt (nomenclatures de imprevus)", avance: false },
            Champ { nom: "date_accident", libelle: Affiche::new("Jour de l'accident"), forme: Forme::Date, sens: "le jour de l'accident, quand le motif l'appelle", avance: true },
            Champ { nom: "connu", libelle: Affiche::new("Su le"), forme: Forme::Objet, sens: "{jour, heure} où la compagnie a su l'accident, quand le motif l'appelle", avance: true },
            Champ { nom: "contrat", libelle: Affiche::new("Contrat"), forme: Forme::Texte, sens: "le contrat suspendu, quand plusieurs couvrent le jour", avance: true },
            Champ { nom: "jours_deja_indemnises", libelle: Affiche::new("Jours déjà indemnisés"), forme: Forme::Nombre, sens: "pour le maintien d'un permanent", avance: true },
            Champ { nom: "note", libelle: Affiche::new("Note"), forme: Forme::Texte, sens: "une note libre", avance: true },
        ],
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","debut":"2026-11-30","fin":"2026-11-30","motif":"01"}"#,
        "arret (l'arrêt résumé), phrase", Acte::Ecrit, ECRAN) },
    Fiche { phrase: 700, ..fiche(Route::ArretProlonger, "Prolonger un arrêt : sa nouvelle fin.",
        &[SALARIE, DERNIER_JOUR, FIN_PRESCRITE],
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","fin":"2026-12-04"}"#,
        "arret (l'arrêt résumé), phrase", Acte::Ecrit, GESTE) },
    Fiche { phrase: 400, ..fiche(Route::ArretReprise, "Poser ou effacer la reprise d'un arrêt.",
        &[SALARIE, DERNIER_JOUR, REPRISE, MOTIF_REPRISE],
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","reprise":"2026-12-03"}"#, "arret (l'arrêt résumé), phrase", Acte::Ecrit, GESTE) },
    Fiche { phrase: 400, ..fiche(Route::ArretIndemnites, "Dire les indemnités journalières d'un arrêt, par jour.",
        &[SALARIE, DERNIER_JOUR, IJ_BRUTE, IJ_NETTE, IJ_BRUTE_MAJOREE, IJ_NETTE_MAJOREE, JOURS_DEJA_INDEMNISES],
        r#"{"salarie":"SVOBODA","dernier_jour":"2026-11-02","ij_brute":"40.89","ij_nette":"38.15"}"#, "arret (l'arrêt résumé), phrase", Acte::Ecrit, GESTE) },
    Fiche { phrase: 600, ..fiche(Route::ArretSignaler, "Fabriquer le signalement d'un arrêt, sans le déposer.",
        &[SALARIE, DERNIER_JOUR, Champ { nom: "nature", libelle: Affiche::new("Nature"), forme: Forme::Texte, sens: "04 l'arrêt, ou 05 la reprise", avance: false }, REMPLACE, ORDRE, TEST,
            Champ { nom: "le", libelle: Affiche::new("Constitué le"), forme: Forme::Date, sens: "le jour de constitution ; sinon aujourd'hui", avance: true }],
        r#"{"salarie":"FULLER","dernier_jour":"2026-11-27","nature":"04"}"#, "fichier (écrit dans sorties/AAAA-MM/), phrase, nature, type, ordre, avertissements", Acte::Ecrit, ECRAN) },
    Fiche { phrase: 400, ..fiche(Route::ArretOublier, "Retirer un arrêt qui n'a pas été déclaré.", &[SALARIE, DERNIER_JOUR],
        UN_ARRET, "arret (l'arrêt retiré, résumé), phrase", Acte::Ecrit, GESTE) },
    Fiche { phrase: 400, ..fiche(Route::SuspensionEnregistrer, "Enregistrer une absence qui suspend un contrat.",
        &[SALARIE, DEBUT, Champ { nom: "fin", libelle: Affiche::new("Dernier jour"), forme: Forme::Date, sens: "son dernier jour", avance: false }, Champ { nom: "motif", libelle: Affiche::new("Motif"), forme: Forme::Texte, sens: "le code du motif (nomenclatures de imprevus)", avance: false }, Champ { nom: "contrat", libelle: Affiche::new("Contrat"), forme: Forme::Texte, sens: "le contrat, quand plusieurs couvrent le jour", avance: true }, Champ { nom: "note", libelle: Affiche::new("Note"), forme: Forme::Texte, sens: "une note libre", avance: true }],
        r#"{"salarie":"FULLER","debut":"2026-11-27","fin":"2026-11-27","motif":"501"}"#, "suspension (l'absence résumée), phrase", Acte::Ecrit, AVANCE) },
    Fiche { phrase: 400, ..fiche(Route::SuspensionModifier, "Changer la fin ou le motif d'une absence.",
        &[SALARIE, DEBUT, NOUVELLE_FIN, Champ { nom: "motif", libelle: Affiche::new("Nouveau motif"), forme: Forme::Texte, sens: "son nouveau motif", avance: true }],
        r#"{"salarie":"FULLER","debut":"2026-11-27","fin":"2026-11-27"}"#, "suspension (l'absence résumée), phrase", Acte::Ecrit, GESTE_AVANCE) },
    Fiche { phrase: 400, ..fiche(Route::SuspensionOublier, "Retirer une absence qui n'a pas été déclarée.", &[SALARIE, DEBUT],
        r#"{"salarie":"FULLER","debut":"2026-11-27"}"#, "suspension (l'absence retirée, résumée), phrase", Acte::Ecrit, GESTE_AVANCE) },
    Fiche { phrase: 600, ..fiche(Route::FinContratSignaler, "Fabriquer le signalement d'une fin de contrat, sans le déposer.",
        &[MOIS, SALARIE, REMPLACE, ORDRE, TEST, Champ { nom: "le", libelle: Affiche::new("Constitué le"), forme: Forme::Date, sens: "le jour de constitution ; sinon aujourd'hui", avance: true }],
        r#"{"mois":"2026-11","salarie":"FULLER"}"#, "fichier (écrit dans sorties/AAAA-MM/), phrase, type, ordre, preavis, avertissements", Acte::Ecrit, AVANCE) },
    Fiche { phrase: 160, ..fiche(Route::FinContratDocuments, "Fabriquer le certificat de travail et le reçu pour solde de tout compte.",
        &[MOIS, SALARIE, Champ { nom: "le", libelle: Affiche::new("Établis le"), forme: Forme::Date, sens: "le jour où ils s'établissent ; sinon aujourd'hui", avance: true }],
        r#"{"mois":"2026-11","salarie":"FULLER"}"#, "phrase, certificat et recu (leurs chemins), dossier, avertissements (ce qu'il faut savoir en les remettant)", Acte::Ecrit, ECRAN) },
];

// ------------------------------------------------------------------ les charges
//
// `salarie` y est toujours ce que la personne en dit : la cle de la fiche, ou son nom (`Contexte::cle_du_salarie`), resolu
// a l'entree de la route en `IdSalarie`. Ce qu'elles ecrivent, la fenetre l'ecrit (`compagnie::INTERFACE`).

/// La charge de `imprevus` : les imprevus d'un mois, ou d'un salarie ; tous, sans l'un ni l'autre.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeImprevus {
    mois: Option<Mois>,
    salarie: Option<String>,
}

/// La charge de `fins-de-contrat` : le mois.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeMois {
    mois: Mois,
}

/// La charge de `arret-contrats` : le salarie et son dernier jour travaille.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArretContrats {
    salarie: String,
    jour: Jour,
}

/// La charge de `arret-enregistrer`. `connu` est la connaissance de l'accident, en la forme que lit
/// `obligations::connaissance_saisie` (`{jour, heure}`, celle de la route `arret-connaissance`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArretEnregistrer {
    salarie: String,
    dernier_jour: Jour,
    debut: Jour,
    fin: Jour,
    motif: String,
    date_accident: Option<Jour>,
    connu: Option<crate::obligations::ConnaissanceDite>,
    contrat: Option<NumeroContrat>,
    jours_deja_indemnises: Option<u32>,
    note: Option<String>,
}

/// La charge de `arret-prolonger` : l'arret (son salarie, son dernier jour travaille) et sa nouvelle fin prescrite.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArretProlonger {
    salarie: String,
    dernier_jour: Jour,
    fin: Jour,
}

/// La charge de `arret-reprise` : la reprise (nulle ou absente, elle s'efface) et son motif.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArretReprise {
    salarie: String,
    dernier_jour: Jour,
    reprise: Option<Jour>,
    motif_reprise: Option<String>,
}

/// La charge de `arret-indemnites` : chaque montant, un decimal en texte (« 40.89 », tel que le releve l'ecrit et que
/// l'arret le garde), et les jours deja indemnises ; absent, rien ne change.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArretIndemnites {
    salarie: String,
    dernier_jour: Jour,
    ij_brute: Option<String>,
    ij_nette: Option<String>,
    ij_brute_majoree: Option<String>,
    ij_nette_majoree: Option<String>,
    jours_deja_indemnises: Option<u32>,
}

pub(crate) use declarations::signalement::NatureSignalement;

/// La charge de `arret-signaler`. `remplace` : l'identifiant du signalement qu'il annule et remplace ; `ordre` : son
/// numero d'ordre (absent : le suivant) ; `le` : son jour de constitution (absent : aujourd'hui). Le geste qu'offre le
/// rappel d'un signalement l'ecrit aussi (`ChargeArretSignaler::de`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeArretSignaler {
    salarie: String,
    dernier_jour: Jour,
    nature: NatureSignalement,
    #[serde(skip_serializing_if = "Option::is_none")]
    remplace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ordre: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    test: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    le: Option<Jour>,
}

impl ChargeArretSignaler {
    /// Le signalement de l'arret de `salarie` qui suit `dernier_jour`, de sa `nature` : le suivant, constitue aujourd'hui.
    pub(crate) fn de(salarie: String, dernier_jour: Jour, nature: NatureSignalement) -> ChargeArretSignaler {
        ChargeArretSignaler { salarie, dernier_jour, nature, remplace: None, ordre: None, test: None, le: None }
    }

    /// Le signalement parti, refait pour se lire (`gestes::depart::refaire_le_signalement`) : son numero d'ordre et son
    /// jour de constitution, tels que le journal les dit.
    pub(crate) fn refait(salarie: String, dernier_jour: Jour, nature: NatureSignalement, ordre: i64, le: Jour) -> ChargeArretSignaler {
        ChargeArretSignaler { ordre: Some(ordre), le: Some(le), ..ChargeArretSignaler::de(salarie, dernier_jour, nature) }
    }
}

/// La charge de `arret-oublier`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArretOublier {
    salarie: String,
    dernier_jour: Jour,
}

/// La charge de `suspension-enregistrer`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeSuspensionEnregistrer {
    salarie: String,
    debut: Jour,
    fin: Jour,
    motif: String,
    contrat: Option<NumeroContrat>,
    note: Option<String>,
}

/// La charge de `suspension-modifier` : le debut la designe ; la fin et le motif, s'ils se disent, changent.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeSuspensionModifier {
    salarie: String,
    debut: Jour,
    fin: Option<Jour>,
    motif: Option<String>,
}

/// La charge de `suspension-oublier`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeSuspensionOublier {
    salarie: String,
    debut: Jour,
}

/// La charge de `fin-contrat-signaler` (`remplace`, `ordre`, `test`, `le` : comme `arret-signaler`). Le geste qu'offre
/// le rappel d'une fin de contrat l'ecrit aussi (`ChargeFinContratSignaler::de`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeFinContratSignaler {
    mois: Mois,
    salarie: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    remplace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ordre: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    test: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    le: Option<Jour>,
}

impl ChargeFinContratSignaler {
    /// Le signalement de la fin du contrat de `salarie` en `mois` : le suivant, constitue aujourd'hui.
    pub(crate) fn de(mois: Mois, salarie: String) -> ChargeFinContratSignaler {
        ChargeFinContratSignaler { mois, salarie, remplace: None, ordre: None, test: None, le: None }
    }

    /// Le signalement parti, refait pour se lire (`gestes::depart::refaire_le_signalement`).
    pub(crate) fn refait(mois: Mois, salarie: String, ordre: i64, le: Jour) -> ChargeFinContratSignaler {
        ChargeFinContratSignaler { ordre: Some(ordre), le: Some(le), ..ChargeFinContratSignaler::de(mois, salarie) }
    }
}

/// La charge de `fin-contrat-documents`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeFinContratDocuments {
    mois: Mois,
    salarie: String,
    le: Option<Jour>,
}

/// L'identifiant du signalement qu'un annule-et-remplace remplace, s'il se dit ; dit vide, il ne designe rien et se refuse.
fn remplace_dit(r: &Option<String>) -> R<Option<String>> {
    match r.as_deref().map(str::trim) {
        Some("") => Err(erreur(Classe::Manque, "Le signalement remplacé n'est pas dit : donner son identifiant, ou ne rien remplacer.")),
        autre => Ok(autre.map(str::to_string)),
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
fn ordre_du_signalement(ctx: &Contexte, ordre: Option<i64>) -> R<(i64, Vec<String>)> {
    match ordre {
        Some(n) => Ok((n, Vec::new())),
        None => prochain_ordre(ctx),
    }
}

// ------------------------------------------------------------------ les reponses

/// `imprevus` : les imprevus resumes, les nomenclatures pour en saisir, et ce que l'ecran lit de chacun.
#[derive(Debug, Serialize)]
struct ReponseImprevus {
    phrase: String,
    imprevus: Vec<Resume>,
    motifs_arret: Vec<CodeDuBloc60>,
    motifs_reprise: Vec<CodeDuBloc60>,
    motifs_suspension: Vec<MotifSuspension>,
    phrases: Vec<String>,
}

/// `arrets` : tous les arrets resumes, ceux qui restent a signaler, et les nomenclatures du bloc 60.
#[derive(Debug, Serialize)]
struct ReponseArrets {
    phrase: String,
    phrases: Vec<String>,
    arrets: Vec<Resume>,
    a_signaler: Vec<Resume>,
    motifs: Vec<CodeDuBloc60>,
    motifs_reprise: Vec<CodeDuBloc60>,
    subrogation: Vec<CodeDuBloc60>,
}

/// `fins-de-contrat`.
#[derive(Debug, Serialize)]
struct ReponseFins {
    phrase: String,
    mois: Mois,
    fins: Vec<FinASignaler>,
}

/// `arret-contrats`.
#[derive(Debug, Serialize)]
struct ReponseContrats {
    phrase: String,
    contrats: Vec<ContratAuJour>,
}

/// Ce que rend un geste sur un arret : l'arret resume, et ce qui se lit du geste.
#[derive(Debug, Serialize)]
struct ReponseArret {
    arret: Resume,
    phrase: String,
}

/// Ce que rend un geste sur une absence.
#[derive(Debug, Serialize)]
struct ReponseSuspension {
    suspension: Resume,
    phrase: String,
}

/// `arret-signaler` : le fichier ecrit.
#[derive(Debug, Serialize)]
pub(crate) struct ReponseSignalement {
    fichier: String,
    phrase: String,
    nature: NatureSignalement,
    #[serde(rename = "type")]
    type_declaration: &'static str,
    ordre: i64,
    avertissements: Vec<String>,
}

/// `fin-contrat-signaler` : le fichier ecrit.
#[derive(Debug, Serialize)]
pub(crate) struct ReponseSignalementFin {
    fichier: String,
    phrase: String,
    #[serde(rename = "type")]
    type_declaration: &'static str,
    ordre: i64,
    preavis: String,
    avertissements: Vec<String>,
}

/// `fin-contrat-documents` : les deux papiers ecrits.
#[derive(Debug, Serialize)]
struct ReponsePapiers {
    phrase: String,
    certificat: String,
    recu: String,
    dossier: String,
    avertissements: &'static [&'static str],
}

/// Ce que chaque route de ce module rend porte sa phrase.
macro_rules! dites {
    ($($t:ty),*) => {$(
        impl Dite for $t {
            fn phrase(&self) -> &str {
                &self.phrase
            }
        }
    )*};
}
dites!(ReponseImprevus, ReponseArrets, ReponseFins, ReponseContrats, ReponseArret, ReponseSuspension, ReponseSignalement, ReponseSignalementFin, ReponsePapiers);

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
        Route::ArretContrats => en_json(arret_contrats(ctx, charge.lire()?)?),
        Route::ArretEnregistrer => {
            let c: ChargeArretEnregistrer = charge.lire()?;
            let mut r = arret_enregistrer(ctx, &c)?;
            regulariser(ctx, &c.salarie, &mut r.phrase)?;
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
fn regulariser(ctx: &Contexte, nom: &str, phrase: &mut String) -> R<()> {
    use engagement::etats::Revalidation;
    let cle = cle_du_nom(ctx, nom)?;
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
        *phrase = format!("{} {}", phrase, dit.join(" "));
    }
    Ok(())
}

// ------------------------------------------------------------------ outils

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}


/// Un numero de contrat tel qu'une phrase le dit.
fn numero_dit(n: &Option<String>) -> String {
    match n {
        Some(n) => n.clone(),
        None => "sans numéro".to_string(),
    }
}

/// La cle de la fiche que `nom` designe, comme partout dans le moteur (`Contexte::cle_du_salarie` : la casse, les accents,
/// le prenom, le NIR ne comptent pas). Un nom qui ne designe personne se refuse avec sa phrase.
fn cle_du_nom(ctx: &Contexte, nom: &str) -> R<IdSalarie> {
    ctx.cle_du_salarie(nom).map_err(|m| m.reclasser(Classe::Manque))
}

/// LE CONTRAT QU'UN IMPREVU SUSPEND : parmi ceux du salarie qui couvrent `jour` (`contrats_au`), celui que la charge
/// nomme (`voulu`, son numero), sinon le seul. Un imprevu designe son contrat par son numero et sa date de debut
/// (S21.G00.40.009 et .001) ; sans contrat, il n'y a rien a suspendre, et a deux, le cahier technique laisse le choix au
/// declarant : on le lui demande.
pub(crate) fn contrat_suspendu(ctx: &Contexte, cle: &str, jour: &Jour, voulu: Option<&str>) -> R<ContratAuJour> {
    let candidats = contrats_au(ctx, cle, *jour)?;
    if let Some(voulu) = voulu {
        return match candidats.iter().find(|c| c.numero.as_deref() == Some(voulu)) {
            Some(c) => Ok(c.clone()),
            None => {
                let liste: Vec<String> = candidats.iter().map(|c| numero_dit(&c.numero)).collect();
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
                .map(|c| match &c.fin {
                    Some(f) => format!("{} du {} au {}", numero_dit(&c.numero), c.debut.iso(), f.iso()),
                    None => format!("{} depuis le {}", numero_dit(&c.numero), c.debut.iso()),
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
    let (p, dj) = (magasin(imprevus::premier_jour(nouveau))?, magasin(imprevus::dernier_jour(nouveau))?);
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
        let (ap, ad) = (magasin(imprevus::premier_jour(&autre))?, magasin(imprevus::dernier_jour(&autre))?);
        if ap <= dj && ad >= p {
            return Err(refus(format!(
                "{} : du {} au {}, cet imprevu chevauche {} du {} au {} sur le meme contrat ({}). Un jour d'absence ne se \
declare et ne se retire qu'une fois : corriger les dates, ou reprendre l'autre imprevu.",
                cle,
                p.jj_mm_aaaa(),
                dj.jj_mm_aaaa(),
                if imprevus::est_un_arret(&autre) { "l'arret de travail" } else { "la suspension" },
                ap.jj_mm_aaaa(),
                ad.jj_mm_aaaa(),
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
fn trouver_arret(ctx: &Contexte, cle: &str, dernier: &Jour) -> R<Option<(usize, Vec<Imprevu>, ArretSaisi)>> {
    let liste = le_fichier(ctx, dernier)?;
    let mut trouve = None;
    for (i, x) in liste.iter().enumerate() {
        if let Imprevu::Arret(a) = x {
            if a.salarie == cle && a.dernier_jour_travaille == *dernier {
                trouve = Some((i, a.clone()));
                break;
            }
        }
    }
    Ok(trouve.map(|(i, a)| (i, liste, a)))
}

/// La suspension de ce salarie qui commence ce jour : son indice, le fichier de son mois, et la suspension.
fn trouver_suspension(ctx: &Contexte, cle: &str, debut: &Jour) -> R<Option<(usize, Vec<Imprevu>, SuspensionSaisie)>> {
    let liste = le_fichier(ctx, debut)?;
    let mut trouve = None;
    for (i, x) in liste.iter().enumerate() {
        if let Imprevu::Suspension(s) = x {
            if s.salarie == cle && s.debut == *debut {
                trouve = Some((i, s.clone()));
                break;
            }
        }
    }
    Ok(trouve.map(|(i, s)| (i, liste, s)))
}

/// L'arret de ce salarie a ce dernier jour travaille, ou le refus qui dit ou les lire.
fn l_arret(ctx: &Contexte, cle: &str, dernier: &Jour) -> R<(usize, Vec<Imprevu>, ArretSaisi)> {
    trouver_arret(ctx, cle, dernier)?.ok_or_else(|| {
        refus(format!("Aucun arret de {} dont le dernier jour travaille soit le {}. La route `arrets` les liste.", cle, dernier.jj_mm_aaaa()))
    })
}

/// La suspension de ce salarie qui commence ce jour, ou le refus.
fn la_suspension(ctx: &Contexte, cle: &str, debut: &Jour) -> R<(usize, Vec<Imprevu>, SuspensionSaisie)> {
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
    Ok(envois.iter().filter(|x| x.nature_declaration == nature.nature() && x.type_declaration.as_deref() == Some(signalement::NORMALE)).map(|x| x.quand).min().map(|q| q.jour()))
}

/// Le nom d'un signalement : `<base>.dsn` ; un annule-et-remplace, `<base>_AR<ordre>.dsn`, a cote du signalement parti
/// qu'il remplace, qui reste tel qu'il est parti (regle 4).
fn nom_du_signalement(base: &str, annule_et_remplace: bool, ordre: i64) -> String {
    if annule_et_remplace {
        format!("{}_AR{}.dsn", base, ordre)
    } else {
        format!("{}.dsn", base)
    }
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
    salarie: String,
    contrat: Option<String>,
    contrat_debut: Jour,
    motif: String,
    pub motif_libelle: String,
    dernier_jour_travaille: Jour,
    debut: Jour,
    fin_previsionnelle: Jour,
    subrogation: Option<String>,
    subrogation_debut: Option<Jour>,
    subrogation_fin: Option<Jour>,
    date_accident: Option<Jour>,
    reprise: Option<Jour>,
    reprise_motif: Option<String>,
    note: Option<String>,
    saisi_le: socle::paris::HeureDeSaisie,
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

/// UN ARRET TEL QUE LA PAGE LE LIT : le resume de toujours (`ArretResume`), plus `genre`, `premier_jour`, `dernier_jour`,
/// `jours_deja_indemnises`, les indemnites par jour (`ij_brute`, `ij_nette`, `ij_brute_majoree`, `ij_nette_majoree`),
/// `origine`, `cachets`, ses `gestes` et `a_dire` (ce qu'il ouvre a la page), ses phrases (`Dit`), puis, dans les listes,
/// ses `obligations` et ce qui en est `parti`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResumeDArret {
    #[serde(flatten)]
    base: ArretResume,
    genre: &'static str,
    premier_jour: Jour,
    dernier_jour: Jour,
    jours_deja_indemnises: Option<Chiffre>,
    ij_brute: Option<Chiffre>,
    ij_nette: Option<Chiffre>,
    ij_brute_majoree: Option<Chiffre>,
    ij_nette_majoree: Option<Chiffre>,
    origine: Option<droit::maintien::Origine>,
    cachets: Vec<Cachet>,
    prolongations: Vec<Prolongation>,
    gestes: Vec<GesteDImprevu>,
    /// Ce que l'arret ecrit appelle encore : ses indemnites journalieres, quand il ouvre le maintien (le meme geste que
    /// dans `gestes`).
    a_dire: Option<GesteDImprevu>,
    #[serde(flatten)]
    dit: Dit,
    #[serde(skip_serializing_if = "Option::is_none")]
    obligations: Option<Vec<crate::obligations::Obligation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parti: Option<Vec<Parti>>,
}

impl ResumeDArret {
    /// Le salarie de l'arret (la cle de sa fiche).
    pub(crate) fn salarie(&self) -> &str {
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
        let obligations = match &self.obligations {
            Some(o) => o.as_slice(),
            None => &[],
        };
        self.parti = Some(ce_qui_est_parti(&self.prolongations, &self.base.signalements, &self.base.reporte_par_dit, obligations)?);
        Ok(())
    }
}

/// UNE ABSENCE TELLE QUE LA PAGE LA LIT : `genre`, `salarie`, `contrat`, `contrat_debut`, `motif`, `motif_libelle`,
/// `remunere`, `debut`, `fin`, `premier_jour`, `dernier_jour`, `note`, `saisi_le`, `reporte_par`, `cachets`, ses phrases
/// (`Dit`), puis, dans la liste, ce qui en est `parti`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResumeDeSuspension {
    genre: &'static str,
    salarie: String,
    contrat: Option<String>,
    contrat_debut: Jour,
    motif: String,
    motif_libelle: String,
    remunere: bool,
    debut: Jour,
    fin: Jour,
    premier_jour: Jour,
    dernier_jour: Jour,
    note: Option<String>,
    saisi_le: socle::paris::HeureDeSaisie,
    reporte_par_dit: Vec<String>,
    cachets: Vec<Cachet>,
    gestes: Vec<GesteDImprevu>,
    #[serde(flatten)]
    dit: Dit,
    #[serde(skip_serializing_if = "Option::is_none")]
    parti: Option<Vec<Parti>>,
}

/// UN GESTE D'UN IMPREVU, que la page dessine en bouton et branche : sa route, sa charge (ce qui designe l'imprevu), son
/// libelle, et ce que sa boite fait saisir : les cases de la fiche de sa route qu'elle montre (`cases` ; aucune : le geste
/// part tel quel), ce qu'elles montrent d'abord (`valeurs`), les choix d'une case qui se choisit (`choix`) et ce que la
/// boite dit au-dessus (`phrases`). Le moteur ne rend que ceux que l'imprevu ouvre.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct GesteDImprevu {
    route: Route,
    charge: Designe,
    libelle: &'static str,
    cases: Vec<&'static str>,
    valeurs: ValeursDeDepart,
    choix: ChoixDesCases,
    phrases: Vec<String>,
}

/// Ce que les cases d'un geste montrent d'abord : ce que l'imprevu dit deja, ou ce que le moteur propose.
#[derive(Debug, Clone, Default, Serialize)]
struct ValeursDeDepart {
    #[serde(skip_serializing_if = "Option::is_none")]
    fin: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reprise: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    motif_reprise: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ij_brute: Option<Chiffre>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ij_nette: Option<Chiffre>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ij_brute_majoree: Option<Chiffre>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ij_nette_majoree: Option<Chiffre>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jours_deja_indemnises: Option<Chiffre>,
}

/// Les choix d'une case d'un geste qui se choisit dans une liste : le motif d'une reprise (nomenclature du bloc 60).
#[derive(Debug, Clone, Default, Serialize)]
struct ChoixDesCases {
    #[serde(skip_serializing_if = "Option::is_none")]
    motif_reprise: Option<Vec<Choisi>>,
}

/// Un choix : le code que la charge porte, le libelle que la liste montre.
#[derive(Debug, Clone, Serialize)]
struct Choisi {
    code: String,
    libelle: String,
}

/// Ce qui designe un imprevu aux routes : un arret, son salarie et son dernier jour travaille ; une absence, son salarie
/// et son premier jour.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum Designe {
    Arret { salarie: String, dernier_jour: Jour },
    Absence { salarie: String, debut: Jour },
}

// Les libelles des gestes, en constantes : un texte fautif ne compile pas (`Affiche::new`).
const PROLONGER: Affiche = Affiche::new("Prolonger l'arrêt");
const REPRISE_PLUS_TOT: Affiche = Affiche::new("Reprise plus tôt");
const CORRIGER_LA_REPRISE: Affiche = Affiche::new("Corriger la reprise");
const INDEMNITES_DITES: Affiche = Affiche::new("Indemnités journalières");
const INDEMNITES_A_DIRE: Affiche = Affiche::new("Indemnités journalières…");
const CHANGER_LA_FIN: Affiche = Affiche::new("Changer la fin");
const RETIRER: Affiche = Affiche::new("Retirer");

impl Designe {
    /// Le geste, sans case a saisir : il part avec sa charge.
    fn geste(&self, route: Route, libelle: Affiche) -> GesteDImprevu {
        GesteDImprevu {
            route,
            charge: self.clone(),
            libelle: libelle.texte(),
            cases: Vec::new(),
            valeurs: ValeursDeDepart::default(),
            choix: ChoixDesCases::default(),
            phrases: Vec::new(),
        }
    }
}

/// La reprise d'un arret, a saisir : son jour et son motif, ceux que l'arret dit deja.
fn geste_de_reprise(d: &Designe, a: &ArretSaisi, n: &Bloc60, libelle: Affiche) -> GesteDImprevu {
    let motifs = n.motifs_reprise.saisissables().into_iter().map(|m| Choisi { code: m.code, libelle: m.libelle }).collect();
    GesteDImprevu {
        cases: vec![REPRISE.nom, MOTIF_REPRISE.nom],
        valeurs: ValeursDeDepart { reprise: a.reprise.clone(), motif_reprise: a.reprise_motif.clone(), ..ValeursDeDepart::default() },
        choix: ChoixDesCases { motif_reprise: Some(motifs) },
        ..d.geste(Route::ArretReprise, libelle)
    }
}

/// LES INDEMNITES JOURNALIERES D'UN ARRET QUI OUVRE LE MAINTIEN, a dire : les cases que le moteur attend (les majorees,
/// pour un accident du travail au-dela du vingt-huitieme jour, CSS R433-3), remplies de ce que l'arret dit, sinon du montant de principe
/// (`engagement::maintien::ij_de_principe`), que la personne garde ou corrige ; sinon, la phrase dit ou le lire.
fn geste_des_indemnites(ctx: &Contexte, d: &Designe, a: &ArretSaisi, majoree: bool) -> R<GesteDImprevu> {
    let mut propose = ValeursDeDepart::default();
    let mut dit = None;
    if !a.ij_brute.as_ref().is_some_and(Chiffre::vrai) {
        use engagement::maintien::IjDePrincipe;
        match magasin(engagement::maintien::ij_de_principe(ctx, a))? {
            IjDePrincipe::Calculee(ij) => {
                propose.ij_brute = Some(Chiffre::Texte(ij.brute.to_string()));
                propose.ij_nette = Some(Chiffre::Texte(ij.nette.to_string()));
                propose.ij_brute_majoree = ij.majorees.map(|(b, _)| Chiffre::Texte(b.to_string()));
                propose.ij_nette_majoree = ij.majorees.map(|(_, n)| Chiffre::Texte(n.to_string()));
                dit = Some(ij.phrase);
            }
            IjDePrincipe::AuReleve(phrase) => dit = Some(phrase),
        }
    }
    let mut cases = vec![IJ_BRUTE.nom, IJ_NETTE.nom];
    if majoree {
        cases.extend([IJ_BRUTE_MAJOREE.nom, IJ_NETTE_MAJOREE.nom]);
    }
    cases.push(JOURS_DEJA_INDEMNISES.nom);
    // Des indemnites deja dites se reprennent ; pas encore dites, elles sont a dire.
    let libelle = if a.ij_brute.is_some() { INDEMNITES_DITES } else { INDEMNITES_A_DIRE };
    let valeurs = ValeursDeDepart {
        ij_brute: a.ij_brute.clone().or(propose.ij_brute),
        ij_nette: a.ij_nette.clone().or(propose.ij_nette),
        ij_brute_majoree: a.ij_brute_majoree.clone().or(propose.ij_brute_majoree),
        ij_nette_majoree: a.ij_nette_majoree.clone().or(propose.ij_nette_majoree),
        jours_deja_indemnises: a.jours_deja_indemnises.clone(),
        ..ValeursDeDepart::default()
    };
    let mut phrases = vec![crate::ouverture::aide("indemnites_journalieres")?.to_string()];
    if let Some(dit) = dit {
        phrases.push(dit);
    }
    phrases.push(crate::ouverture::aide("jours_deja_indemnises")?.to_string());
    Ok(GesteDImprevu { cases, valeurs, phrases, ..d.geste(Route::ArretIndemnites, libelle) })
}

/// CE QU'UN ARRET OUVRE (rien de remplissable s'il n'y a pas a le remplir) : en cours, il se prolonge ou se reprend plus
/// tot ; une reprise saisie se corrige tant que son signalement n'est pas parti ; ses indemnites journalieres se disent
/// s'il ouvre le maintien (`indemnites`) ; il se retire tant que rien n'en est parti (ni signalement, ni DSN qui le
/// reporte). Une reprise a la date prevue ne se saisit pas : la DSN la porte.
fn gestes_d_un_arret(n: &Bloc60, a: &ArretSaisi, base: &ArretResume, cours: bool, reprise_corrigeable: bool, indemnites: Option<&GesteDImprevu>) -> R<Vec<GesteDImprevu>> {
    let d = Designe::Arret { salarie: a.salarie.clone(), dernier_jour: a.dernier_jour_travaille.clone() };
    let mut g = Vec::new();
    if cours {
        g.push(GesteDImprevu {
            cases: vec![FIN_PRESCRITE.nom],
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
fn gestes_d_une_absence(s: &SuspensionSaisie, reportee: &[Mois]) -> Vec<GesteDImprevu> {
    let d = Designe::Absence { salarie: s.salarie.clone(), debut: s.debut.clone() };
    let mut g = vec![GesteDImprevu {
        cases: vec![NOUVELLE_FIN.nom],
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
    signaler_titre: SignalerTitre,
    deja_signale: DejaSignale,
}

#[derive(Debug, Clone, Serialize)]
struct SignalerTitre {
    #[serde(rename = "04")]
    arret: &'static str,
    #[serde(rename = "05")]
    reprise: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct DejaSignale {
    #[serde(rename = "04", skip_serializing_if = "Option::is_none")]
    arret: Option<String>,
    #[serde(rename = "05", skip_serializing_if = "Option::is_none")]
    reprise: Option<String>,
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
    contrat: Option<&'a str>,
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
            signaler_titre: SignalerTitre { arret: "Signaler l'arrêt", reprise: "Signaler la reprise" },
            deja_signale: DejaSignale { arret: deja(NatureSignalement::Arret), reprise: deja(NatureSignalement::Reprise) },
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
        let quoi = if s.nature_declaration == NatureDeLaDeclaration::SignalementReprise { "reprise signalée" } else if s.type_declaration.as_deref() == Some("03") { "signalement rectifié" } else { "signalé" };
        parti.push(dite(format!("{} le {}", quoi, s.quand.dit())));
    }
    if !reporte_par_dit.is_empty() {
        parti.push(dite(format!("déclaré par la DSN {}", reporte_par_dit.join(", "))));
    }
    for o in obligations {
        let libelle = minuscule(o.libelle.clone());
        match &o.fait {
            Some(f) if matches!(o.ou, Some(crate::obligations::Ou::Lieux(_))) => {
                parti.push(dite(format!("{}, faite le {}", libelle, socle::ecran::quand(&f.le().iso()))));
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
            a.obligations = Some(obligations);
        }
    }

    /// Ce qui est parti de l'imprevu (`ce_qui_est_parti`), une fois ses obligations posees (un arret sans obligations
    /// posees n'en dit aucune).
    fn dire_ce_qui_est_parti(&mut self) -> R<()> {
        match self {
            Resume::Arret(a) => a.dire_ce_qui_est_parti()?,
            Resume::Suspension(s) => s.parti = Some(ce_qui_est_parti(&[], &[], &s.reporte_par_dit, &[])?),
        }
        Ok(())
    }

    /// L'imprevu en lignes, tel que `imprevus` le dit : qui, quel contrat, quoi, quand, ou en est son signalement, puis
    /// chacune de ses obligations.
    fn en_lignes(&self) -> Vec<String> {
        let (salarie, contrat, libelle, premier, dernier, etat, obligations) = match self {
            Resume::Arret(a) => (&a.base.salarie, &a.base.contrat, &a.base.motif_libelle, &a.premier_jour, &a.dernier_jour, a.base.etat.dit(), a.obligations.as_deref()),
            Resume::Suspension(s) => (&s.salarie, &s.contrat, &s.motif_libelle, &s.premier_jour, &s.dernier_jour, "", None),
        };
        let mut l = vec![format!("{} ({}) : {}, du {} au {} — {}", salarie, numero_dit(contrat), libelle, premier.jj_mm_aaaa(), dernier.jj_mm_aaaa(), etat)];
        if let Some(o) = obligations {
            l.extend(crate::obligations::en_phrases(o).into_iter().map(|o| format!("  {}", o)));
        }
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
    let motif = magasin(droit::arrets::motif_d_arret(&a.motif))?;
    let origine = motif.and_then(|m| m.origine);
    let ouvert = ouvre_le_maintien(ctx, a)?;
    let majoree = ouvert && origine.is_some_and(|o| o.professionnelle() || o == droit::maintien::Origine::Trajet) && dernier.moins(&premier) >= 28;
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
        contrat: a.contrat_numero.as_deref(),
        signalements: &base.signalements,
    })?;
    Ok(ResumeDArret {
        base,
        genre: ARRET,
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
        obligations: None,
        parti: None,
    })
}

/// Une absence telle que la page la lit (`ResumeDeSuspension`). Son motif est connu de la norme : un motif inconnu ne
/// s'enregistre pas (`verifier_suspension`), il se refuse s'il est dans le magasin.
fn resume_de_suspension(ctx: &Contexte, s: &SuspensionSaisie, cachets: Vec<Cachet>, premier: Jour, dernier: Jour) -> R<ResumeDeSuspension> {
    let reportee = reporte_par(&ses_envois(ctx, &Imprevu::Suspension(s.clone()))?);
    let motif = magasin(imprevus::motif_de_suspension(&s.motif))?.ok_or_else(|| {
        erreur(Classe::Illisible, format!("La suspension de {} du {} a un motif que la norme ne connaît pas ({}) : la corriger.", s.salarie, s.debut.iso(), s.motif))
    })?;
    let dit = Dit::de(ADire {
        arret: false,
        libelle: &motif.libelle,
        reprise: None,
        premier,
        dernier,
        contrat: s.contrat_numero.as_deref(),
        signalements: &[],
    })?;
    Ok(ResumeDeSuspension {
        genre: SUSPENSION,
        salarie: s.salarie.clone(),
        contrat: s.contrat_numero.clone(),
        contrat_debut: s.contrat_debut,
        motif: s.motif.clone(),
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
        parti: None,
    })
}

/// Un motif d'absence qui se declare, tel que la page le propose.
#[derive(Debug, Serialize)]
struct MotifSuspension {
    code: String,
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
            MotifSuspension { code: m.code.clone(), libelle: m.libelle.clone(), libelle_dit, remunere: m.remunere }
        })
        .collect())
}

// ------------------------------------------------------------------ imprevus

/// La route `imprevus` : les imprevus (d'un salarie, qui croisent un mois), resumes -- un arret avec ses `obligations` --,
/// et les nomenclatures pour en saisir.
fn imprevus(ctx: &Contexte, c: ChargeImprevus) -> R<ReponseImprevus> {
    let cle = c.salarie.as_deref().map(|nom| cle_du_nom(ctx, nom)).transpose()?;
    let bornes = c.mois.map(|m| (m.premier_jour(), m.dernier_jour()));
    let n = nomenclatures()?;
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    let mut out = Vec::new();
    for lu in magasin(imprevus::tous(&ctx.racine))? {
        if cle.as_ref().is_some_and(|c| lu.salarie() != c.as_str()) {
            continue;
        }
        if let Some((debut, fin)) = &bornes {
            if !magasin(imprevus::croise(&lu, debut, fin))? {
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
        out.push(r);
    }
    Ok(ReponseImprevus {
        phrase: format!("Imprévus : {}.", out.len()),
        phrases: out.iter().flat_map(Resume::en_lignes).collect(),
        imprevus: out,
        motifs_arret: n.motifs_arret.saisissables(),
        motifs_reprise: n.motifs_reprise.saisissables(),
        motifs_suspension: motifs_de_suspension()?,
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
    let montant = |k: &str, t: &Option<String>| -> R<Option<Chiffre>> {
        let Some(t) = t else { return Ok(None) };
        let x = Chiffre::Texte(t.trim().to_string()).dec_avec_echelle().map_err(|e| e.reformuler(|p| format!("Indemnité journalière ({}) : {}", k, p)).reclasser(Classe::Refuse))?;
        if x <= Decimal::ZERO {
            return Err(refus("Une indemnité journalière est un montant par jour, positif : rien n'est changé."));
        }
        Ok(Some(Chiffre::Texte(socle::num::eur2(x).to_string())))
    };
    if let Some(x) = montant("ij_brute", &c.ij_brute)? {
        a.ij_brute = Some(x);
    }
    if let Some(x) = montant("ij_nette", &c.ij_nette)? {
        a.ij_nette = Some(x);
    }
    if let Some(x) = montant("ij_brute_majoree", &c.ij_brute_majoree)? {
        a.ij_brute_majoree = Some(x);
    }
    if let Some(x) = montant("ij_nette_majoree", &c.ij_nette_majoree)? {
        a.ij_nette_majoree = Some(x);
    }
    if let Some(j) = c.jours_deja_indemnises {
        a.jours_deja_indemnises = Some(Chiffre::entier(i64::from(j)));
    }
    let nouveau = Imprevu::Arret(a.clone());
    imprevus::verifier(&nouveau).map_err(|e| e.reclasser(Classe::Refuse))?;
    liste[i] = nouveau;
    ecrire(ctx, &dernier, liste)?;
    reponse_arret(ctx, &a, "Indemnités journalières notées.")
}

/// Le contrat qu'un imprevu suspend, lu dans la saisie qui le porte ; `None` s'il n'y est plus.
fn contrat_de(ctx: &Contexte, imp: &impl Suspend) -> R<Option<ContratAuJour>> {
    let contrats = contrats_au(ctx, imp.salarie(), imp.contrat_debut())?;

    Ok(contrats.into_iter().find(|c| c.debut == imp.contrat_debut()))
}

/// La nature du contrat qu'un arret suspend ; `None` s'il n'y est plus.
fn nature_du_contrat(ctx: &Contexte, a: &ArretSaisi) -> R<Option<std::sync::Arc<droit::natures::Nature>>> {
    let Some(c) = contrat_de(ctx, a)? else { return Ok(None) };
    // Une nature se dit, elle ne se devine pas (regle 6) : celle d'un contrat repris se dit apres l'import (`import::nature`).
    let Some(n) = c.nature else {
        return Err(refus(format!("{} : la nature de son contrat du {} n'est pas encore dite.", c.nom, c.debut.iso())));
    };
    Ok(Some(n.proprietes().map_err(|e| e.reclasser(Classe::Refuse))?))
}

/// L'arret ouvre-t-il le maintien de salaire, par la loi ou la convention (`engagement::maintien::droits`) ? Un arret dont
/// le contrat n'est plus dans la saisie n'en ouvre pas.
fn ouvre_le_maintien(ctx: &Contexte, a: &ArretSaisi) -> R<bool> {
    let Some(nat) = nature_du_contrat(ctx, a)? else { return Ok(false) };
    Ok(magasin(engagement::maintien::droits(ctx, a, &nat))?.is_some_and(|d| d.du()))
}

/// LA SUBROGATION D'UN ARRET, telle que le moteur la tire de la convention et du choix de la compagnie
/// (`engagement::maintien::subrogation`) : ses dates, ou rien (pas de subrogation).
fn subrogation_tiree(ctx: &Contexte, a: &ArretSaisi) -> R<Option<(Jour, Jour)>> {
    match nature_du_contrat(ctx, a)? {
        Some(nat) => magasin(engagement::maintien::subrogation(ctx, a, &nat)),
        None => Ok(None),
    }
}

/// La subrogation tiree du droit (`subrogation_tiree`), posee sur l'arret : `01` et ses dates, ou `02`.
pub(crate) fn poser_la_subrogation(ctx: &Contexte, a: &mut ArretSaisi) -> R<()> {
    match subrogation_tiree(ctx, a)? {
        Some((d, f)) => {
            a.subrogation = Some("01".to_string());
            a.subrogation_debut = Some(d);
            a.subrogation_fin = Some(f);
        }
        None => {
            a.subrogation = Some("02".to_string());
            a.subrogation_debut = None;
            a.subrogation_fin = None;
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
        if let Some((_, sub_fin)) = subrogation_tiree(ctx, &a)? {
            a.subrogation_fin = Some(sub_fin);
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
        subrogation_fin_avant: avant.subrogation_fin,
        subrogation_fin: a.subrogation_fin,
    });
    liste[i] = Imprevu::Arret(a.clone());
    ecrire(ctx, &dernier, liste)?;

    let mut avertissements: Vec<String> = Vec::new();
    avertissements.push(format!(
        "Prolongé jusqu'au {}. Rien ne part : pas de nouveau signalement d'arrêt ; la DSN mensuelle de chaque mois que \
l'arrêt traverse porte la nouvelle fin prévisionnelle (S21.G00.60.003, fiche net-entreprises 1983).",
        f.jj_mm_aaaa()
    ));
    // Les jours ajoutes qui tombent dans un mois dont la DSN reelle est deja partie : elle portait l'ancienne fin.
    let declares = reporte_par(&envois_de_l_arret(ctx, &avant)?);
    for m in Mois::entre(&fin_avant.plus_jours(1).map_err(|e| e.reclasser(Classe::Illisible))?, &f) {
        if declares.contains(&m) {
            avertissements.push(format!(
                "La DSN de {} est déjà partie avec l'ancienne fin (le {}) : la paie de ce mois, où les jours ajoutés \
tombent, est à corriger.",
                m.libelle(Graphie::Accentuee),
                fin_avant.jj_mm_aaaa()
            ));
        }
    }
    if a.subrogation_fin != avant.subrogation_fin && envoye(ctx, &avant, NatureSignalement::Arret)? {
        avertissements.push(
            "Le signalement d'arrêt est déjà parti avec l'ancienne fin de subrogation : un signalement d'arrêt annule et \
remplace porte la nouvelle, le plus vite possible (fiche net-entreprises 1983). Il paraît dans ce qui est dû."
                .to_string(),
        );
    }
    reponse_arret(ctx, &a, &avertissements.join("  ·  "))
}

/// L'arret est-il subroge (S21.G00.60.004 = oui) ?
fn signalement_subroge(a: &ArretSaisi) -> R<bool> {
    declarations::signalement::avec_subrogation(&declarations::dsn::Arret { saisi: a.clone() }).map_err(|m| m.reclasser(Classe::Illisible))
}

// ------------------------------------------------------------------ les suspensions

/// Ce qu'une suspension doit tenir avant de s'ecrire : son motif se declare (`imprevus::verifier_motif_de_suspension` :
/// connu, pas le 200), elle finit apres avoir commence, et ne deborde pas de son contrat.
fn verifier_suspension(s: &SuspensionSaisie, contrat_fin: Option<Jour>) -> R<()> {
    imprevus::verifier_motif_de_suspension(&s.motif, &s.salarie).map_err(|e| e.reclasser(Classe::Refuse))?;
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
            numero_dit(&s.contrat_numero),
            contrat_fin.jj_mm_aaaa()
        )));
    }
    Ok(())
}

/// `suspension-enregistrer`.
fn suspension_enregistrer(ctx: &Contexte, c: &ChargeSuspensionEnregistrer) -> R<ReponseSuspension> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let debut = c.debut;
    let contrat = contrat_suspendu(ctx, &cle, &debut, c.contrat.as_ref().map(NumeroContrat::as_str))?;
    let s = SuspensionSaisie {
        genre: GenreSuspension::Suspension,
        salarie: cle.to_string(),
        contrat_debut: contrat.debut.clone(),
        contrat_numero: contrat.numero.clone(),
        motif: c.motif.clone(),
        debut,
        fin: c.fin,
        saisi_le: maintenant(),
        qui: INTERFACE.to_string(),
        note: c.note.clone(),
    };
    verifier_suspension(&s, contrat.fin)?;
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
    reponse_suspension(ctx, &s, "Absence enregistrée. Chaque mois qu'elle traverse la déclarera.")
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
    if let Some(m) = &c.motif {
        s.motif = m.clone();
    }
    let fin_du_contrat = contrat_de(ctx, &s)?.and_then(|c| c.fin);
    verifier_suspension(&s, fin_du_contrat)?;
    sans_chevauchement(ctx, &Imprevu::Suspension(s.clone()), Some(&Imprevu::Suspension(avant)))?;
    liste[i] = Imprevu::Suspension(s.clone());
    ecrire(ctx, &debut, liste)?;
    reponse_suspension(ctx, &s, "Absence changée.")
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
    reponse_suspension(ctx, &s, "Absence retirée : aucune déclaration ne l'avait portée.")
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
pub(crate) fn dernier_jour_saisi(ctx: &Contexte, salarie: &str, declare: Jour) -> R<Jour> {
    for a in tous_les_arrets(ctx)? {
        if a.salarie == salarie && magasin(imprevus::dernier_jour_declare(&a))? == declare {
            return Ok(a.dernier_jour_travaille);
        }
    }
    Ok(declare)
}

/// `{"arret": resume, "phrase"}` : l'arret tel que la page le lit (`resume`, avec les cachets qui tombent dedans), et ce
/// qui se lit du geste.
fn reponse_arret(ctx: &Contexte, arret: &ArretSaisi, phrase: &str) -> R<ReponseArret> {
    Ok(ReponseArret { arret: resume(ctx, &nomenclatures()?, &Imprevu::Arret(arret.clone()))?, phrase: phrase.to_string() })
}

/// `{"suspension": resume, "phrase"}`.
fn reponse_suspension(ctx: &Contexte, s: &SuspensionSaisie, phrase: &str) -> R<ReponseSuspension> {
    Ok(ReponseSuspension { suspension: resume(ctx, &nomenclatures()?, &Imprevu::Suspension(s.clone()))?, phrase: phrase.to_string() })
}

/// `arret-enregistrer`. Les controles de la norme sont ceux du signalement (`signalement::verifier`, nature 04), avant
/// toute ecriture ; un arret ne chevauche pas un autre imprevu du meme contrat (`sans_chevauchement`).
fn arret_enregistrer(ctx: &Contexte, c: &ChargeArretEnregistrer) -> R<ReponseArret> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let dernier = c.dernier_jour;
    let choisi = contrat_suspendu(ctx, &cle, &dernier, c.contrat.as_ref().map(NumeroContrat::as_str))?;
    let mut arret = arret_a_ecrire(ArretDit {
        cle: &cle,
        contrat: &choisi,
        motif: &c.motif,
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
    reponse_arret(ctx, &arret, "Arrêt enregistré.")
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
    reponse_arret(ctx, &a, if c.reprise.is_some() { "Reprise notée." } else { "Reprise effacée." })
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
    reponse_arret(ctx, &a, "Arrêt retiré : aucune déclaration ne l'avait porté.")
}

// ------------------------------------------------------------------ la fin d'un contrat

/// Ce qui peut partir du mois : la saisie de ses engagements valides (`engagement::etats::validees_du_mois`), et leurs
/// paies ecrites ; refus si aucun engagement ne le touche.
fn validees(ctx: &Contexte, mois: Mois) -> R<(VueDuMois, Vec<Paie>)> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    engagement::etats::validees_du_mois(ctx, mois, &departs)?.ok_or_else(|| refus(format!("Aucune saisie pour {}.", mois)))
}

/// L'engagement avec ses bornes : un contrat aux cachets ne les ecrit pas, il commence son premier jour travaille et
/// finit son dernier (`engagement::debut_brut`, `engagement::dates_engagement`).
fn avec_ses_bornes(e: &Carte) -> Carte {
    let mut e = e.clone();
    if e.debut.is_none() {
        e.debut = e.debut_lu();
    }
    if e.fin.is_none() {
        e.fin = e.dates().iter().map(|j| j.jour()).max();
    }
    e
}

/// L'engagement de ce salarie (sa cle) qui se termine ce mois, avec ses bornes (`avec_ses_bornes`), et son rang dans la
/// saisie. `nom` : comme on l'a dit, pour la phrase. Un contrat qui a un motif de rupture passe avant les autres. Un
/// salarie sans engagement ce mois-la n'a pas de paie : la phrase le dit, avec le mois en toutes lettres.
fn engagement_qui_finit(saisie: &VueDuMois, cle: &IdSalarie, nom: &str, mois: Mois) -> R<(usize, Carte)> {
    let mut siens: Vec<(usize, Carte)> =
        saisie.engagements.iter().enumerate().filter(|(_, x)| x.salarie.as_ref() == Some(cle)).map(|(i, x)| (i, avec_ses_bornes(x))).collect();
    if siens.is_empty() {
        return Err(refus(format!("{} n'a aucun engagement en {} : aucun contrat ne s'y termine.", nom, mois.libelle(Graphie::Accentuee))));
    }
    let finit = |x: &Carte| x.fin.is_some_and(|f| f.mois_civil() == mois);
    let rompu = siens.iter().position(|(_, x)| finit(x) && x.motif_rupture.is_some());
    let i = rompu.or_else(|| siens.iter().position(|(_, x)| finit(x))).ok_or_else(|| {
        refus(format!("{} n'a aucun contrat qui se termine en {}.", nom, mois.libelle(Graphie::Accentuee)))
    })?;
    Ok(siens.swap_remove(i))
}

/// La paie validee de l'engagement de rang `rang` (`paies` : celles de `validees`, au meme rang que sa saisie).
fn paie_de_l_engagement(paies: &[Paie], rang: usize, cle: &IdSalarie, nom: &str, mois: Mois) -> R<Paie> {
    paies
        .get(rang)
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
    let (rang, _) = engagement_qui_finit(&saisie, &cle, &c.salarie, mois)?;
    let p = paie_de_l_engagement(&paies, rang, &cle, &c.salarie, mois)?;
    let date = constitue_le(ctx, c.le);
    let dossier = ctx.racine.join("sorties").join(mois.to_string());
    let [certificat, recu] = documents::fin_de_contrat::ecrire(ctx, &p, date, &dossier)?;
    Ok(ReponsePapiers {
        phrase: format!("Écrits pour {} : le certificat de travail et le reçu pour solde de tout compte.", crate::nom_a_l_ecran(&p.salarie, &cle.to_string())),
        certificat: certificat.to_string_lossy().into_owned(),
        recu: recu.to_string_lossy().into_owned(),
        dossier: dossier.to_string_lossy().into_owned(),
        avertissements: documents::fin_de_contrat::AVIS,
    })
}

/// Un jour que le signalement exige, lu dans la carte ; absent, il se refuse.
fn exige(x: Option<Jour>, cle: &str) -> R<Jour> {
    x.ok_or_else(|| socle::fichiers::absent(cle).reclasser(Classe::Manque))
}

/// `fctu.FinDeContrat` que le signalement declarera, batie sur la saisie (`fin-contrat-signaler`).
fn fin_de_contrat_declaree(ctx: &Contexte, e: &Carte, mois: Mois) -> R<declarations::fctu::FinDeContrat> {
    let manque = ce_qui_manque(ctx, e)?;
    let e = &fin_au_terme(e)?;
    if !manque.is_empty() {
        let liste: Vec<String> = manque.iter().map(phrase_du_manque).collect();
        return Err(refus(format!(
            "Contrat {} : le signalement de fin de contrat ne s'écrit pas encore ; il manque, dans la carte de l'engagement, sous « Contrat » :\n  {}",
            numero_dit(&e.numero),
            liste.join("\n  ")
        )));
    }
    let preavis = e.preavis.as_ref().ok_or_else(|| socle::fichiers::absent("preavis").reclasser(Classe::Manque))?;
    Ok(declarations::fctu::FinDeContrat {
        fin: exige(e.fin, "fin")?,
        motif: e.motif_rupture.ok_or_else(|| socle::fichiers::absent("motif_rupture").reclasser(Classe::Manque))?,
        mois_rattachement: mois,
        type_preavis: preavis.genre.clone(),
        preavis_debut: preavis.debut,
        preavis_fin: preavis.fin,
        notification: e.notification_rupture,
        signature_convention: e.signature_convention,
        engagement_licenciement: e.engagement_licenciement,
        transaction: e.transaction.clone(),
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
    let remplace = remplace_dit(&c.remplace)?;
    let envoi_test = c.test == Some(true);

    let (saisie, paies) = validees(ctx, mois)?;
    let cle = cle_du_nom(ctx, salarie)?;
    let (rang, e) = engagement_qui_finit(&saisie, &cle, salarie, mois)?;
    let fin = fin_de_contrat_declaree(ctx, &e, mois)?;
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
    let type_declaration = if remplace.is_some() { declarations::fctu::ANNULE_ET_REMPLACE } else { declarations::fctu::NORMALE };
    let mut avertissements = declarations::fctu::verifier(&fin, &paie.engagement, type_declaration).map_err(echec_fctu)?;
    // LE NUMERO D'ORDRE : celui qu'on donne, sinon le suivant de tous les signalements deja partis (p. 136), lu une fois la
    // saisie verifiee : la plate-forme ne se consulte pas pour un refus.
    let (ordre, plate_forme) = ordre_du_signalement(ctx, c.ordre)?;
    avertissements.extend(plate_forme);
    let nom = nom_du_signalement(&format!("SignalementFinContrat_{}_{}", cle, fin.fin.aaaammjj()), remplace.is_some(), ordre);
    let cible = ctx.racine.join("sorties").join(mois.to_string()).join(&nom);
    compagnie::pieces_ecrites::refuser_si_partie(&ctx.racine, &cible)?;
    let options = declarations::fctu::Options {
        type_declaration: type_declaration.to_string(),
        ordre,
        remplace: remplace.clone(),
        envoi_test,
        date_constitution: c.le,
    };
    // Les arrets du contrat qui croisent la periode du signalement : le FCTU les reporte (bloc 60), comme la mensuelle,
    // tires du magasin des imprevus ; ses suspensions (bloc 65) en viennent deja (`imprevus::suspensions_du_mois`).
    let (debut_p, fin_p) = (paie.engagement.debut_periode(), paie.engagement.fin_periode());
    let arrets: Vec<declarations::dsn::Arret> = magasin(imprevus::arrets_du_contrat(ctx, &paie, &debut_p, &fin_p))?
        .into_iter()
        .map(|saisi| declarations::dsn::Arret { saisi })
        .collect();
    let texte = declarations::fctu::generer(ctx, &paie, &fin, &options, &arrets, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?).map_err(echec_fctu)?;
    declarations::dsn::ecrire(&texte, &cible)?;
    compagnie::pieces_ecrites::noter_piece_ecrite(&ctx.racine, &compagnie::pieces_ecrites::cle_de_piece(&cible), None, None)?;
    let preavis = declarations::fctu::libelle_preavis(&fin.type_preavis)?;
    Ok(ReponseSignalementFin {
        fichier: cible.to_string_lossy().into_owned(),
        phrase: phrase_du_signalement(&format!("Signalement de fin de contrat écrit (préavis : {}). Rien n'est parti.", preavis), &avertissements),
        type_declaration,
        ordre,
        preavis,
        avertissements,
    })
}

/// Ce qui se lit d'un signalement écrit : ce qui est fait, puis ses avis, sur la même ligne.
fn phrase_du_signalement(fait: &str, avertissements: &[String]) -> String {
    std::iter::once(fait.to_string()).chain(avertissements.iter().cloned()).collect::<Vec<_>>().join("  ·  ")
}

/// Une saisie d'un arret qui se defait : un avis de prolongation, ou une reprise notee.
enum Trace<'a> {
    Prolongation(&'a Prolongation),
    Reprise(&'a RepriseNotee),
}

impl Trace<'_> {
    fn quand(&self) -> socle::paris::HeureDeSaisie {
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
                a.subrogation_fin = p.subrogation_fin_avant.clone();
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
pub(crate) fn etats_d_alors(ctx: &Contexte, salarie: &str, dernier_jour: &Jour, instant: socle::paris::HeureDeSaisie) -> R<Vec<ArretSaisi>> {
    let (_, _, lu) = l_arret(ctx, &cle_du_nom(ctx, salarie)?, dernier_jour)?;
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
fn prochain_ordre(ctx: &Contexte) -> R<(i64, Vec<String>)> {
    let connu = crate::gestes::releve::dernier_ordre_connu(ctx, None)?;
    Ok((connu.dernier + 1, connu.avertissements))
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
    let remplace = remplace_dit(&c.remplace)?;
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
    let type_declaration = if remplace.is_some() { signalement::ANNULE_ET_REMPLACE } else { signalement::NORMALE };
    let constitution = constitue_le(ctx, c.le);
    let dsn_arret = declarations::dsn::Arret { saisi: arret.clone() };
    let mut avertissements = signalement::verifier(&dsn_arret, nature, Some(&constitution)).map_err(echec_fctu)?;
    let (ordre, plate_forme) = ordre_du_signalement(ctx, c.ordre)?;
    avertissements.extend(plate_forme);

    let dernier = arret.dernier_jour_travaille;
    let nom = nom_du_signalement(
        &format!("Signalement{}_{}_{}", if c.nature == NatureSignalement::Arret { "Arret" } else { "Reprise" }, arret.salarie, dernier.aaaammjj()),
        type_declaration == signalement::ANNULE_ET_REMPLACE,
        ordre,
    );
    let cible = ctx.racine.join("sorties").join(dernier.mois_civil().to_string()).join(&nom);
    compagnie::pieces_ecrites::refuser_si_partie(&ctx.racine, &cible)?;
    let options = signalement::Options {
        nature_declaration: nature,
        type_declaration: type_declaration.to_string(),
        ordre,
        remplace,
        envoi_test: c.test == Some(true),
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
        fichier: cible.to_string_lossy().into_owned(),
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
    match n.motifs_arret.code(&a.motif) {
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
    let signalement_du = crate::obligations::signalement_du(&a.motif)?;
    Ok(ArretResume {
        salarie: a.salarie.clone(),
        contrat: a.contrat_numero.clone(),
        contrat_debut: a.contrat_debut,
        motif: a.motif.clone(),
        motif_libelle,
        dernier_jour_travaille: a.dernier_jour_travaille,
        debut: a.debut,
        fin_previsionnelle: a.fin_previsionnelle,
        subrogation: a.subrogation.clone(),
        subrogation_debut: a.subrogation_debut,
        subrogation_fin: a.subrogation_fin,
        date_accident: a.date_accident,
        reprise: a.reprise,
        reprise_motif: a.reprise_motif.clone(),
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
        if let Some(o) = &r.obligations {
            phrases.extend(crate::obligations::en_phrases(o).into_iter().map(|o| format!("  {}", o)));
        }
        resumes.push(Resume::Arret(Box::new(r)));
    }
    let a_signaler: Vec<Resume> = resumes.iter().filter(|r| matches!(r, Resume::Arret(x) if !x.base.signale && x.base.signalement_du)).cloned().collect();
    Ok(ReponseArrets {
        phrase: format!("{}, {} à signaler.", socle::ecran::compte(resumes.len(), "arrêt de travail", "arrêts de travail"), a_signaler.len()),
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
    r.obligations = Some(crate::obligations::de_l_arret(ctx, a, suivi, jour)?);
    r.dire_ce_qui_est_parti()?;
    Ok(r)
}

// ----------------------------------------------------------- fins de contrat

/// La nature d'un engagement : dite (regle 6) ; sinon `Manque`.
fn nature_de(e: &Carte) -> R<std::sync::Arc<droit::natures::Nature>> {
    let Some(n) = e.nature else {
        return Err(socle::fichiers::absent("nature").reclasser(Classe::Manque));
    };
    n.proprietes().map_err(|m| m.reclasser(Classe::Manque))
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
const SANS_PREAVIS: &str = "90";

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
            e.preavis = Some(Preavis { genre: SANS_PREAVIS.to_string(), debut: None, fin: None });
        }
    }
    Ok(e)
}

/// La phrase d'un manque (`ce_qui_manque`) : sa case, son nom dans la saisie de la carte, et ce qu'on y met.
pub(crate) fn phrase_du_manque(m: &Manque) -> String {
    format!("« {} » (`{}`) : {}", m.case, m.champ, m.dire)
}

/// Ce que dit la loi du preavis d'une rupture, et la convention a cote (`moteur preavis`), pour la case « Préavis ».
fn preavis_dit(ctx: &Contexte, e: &Carte) -> R<String> {
    let (debut, fin) = (exige(e.debut, "debut")?, exige(e.fin, "fin")?);
    let anciennete = droit::permanents::anciennete_mois(&debut, &fin);
    let motif = e.motif_rupture;
    let cdd = !nature_de(e)?.duree_indeterminee;
    let duree = droit::permanents::preavis_mois(anciennete, motif, cdd)?;
    let regle = droit::permanents::regle_du_preavis(motif, cdd)?;
    let mut dit = match (duree, &regle) {
        (Some(m), _) if m != Decimal::ZERO => format!("La loi en donne {} mois (L1234-1).", m),
        (Some(_), _) => match droit::permanents::raison_sans_preavis(motif, cdd)? {
            Some(r) => format!("La loi ne donne aucun préavis pour ce motif : {}", r),
            None => "La loi ne donne aucun préavis pour ce motif.".to_string(),
        },
        (None, Some(r)) => format!("La loi en donne un que le moteur ne compte pas : {}", r),
        (None, None) => "Sous six mois d'ancienneté, la loi renvoie à la convention ou aux usages (L1234-1) : elle ne dit pas de durée.".to_string(),
    };
    // Ce que dit la convention, a cote, par le moteur (le calcul de `moteur preavis`) ; pas pour un motif sans preavis
    // ni pour la rupture d'un CDD par le salarie (L1243-2), que la convention du licenciement ne regle pas.
    if duree != Some(Decimal::ZERO) && regle.is_none() {
        let tout = engagement::preavis(ctx, e, anciennete, motif)?;
        if let (Some(conv), true) = (tout.convention, tout.qui != "la loi (L1234-1)") {
            dit.push_str(&format!(" {} en donne {} : c'est elle qui s'applique.", tout.qui, conv));
        } else if tout.mois.is_none() {
            if let Some(pourquoi) = &tout.pourquoi {
                dit.push_str(&format!(" Et {}.", pourquoi.trim_end_matches('.')));
            }
        }
    }
    Ok(dit)
}

/// Ce qu'un signalement exige et que la saisie ne dit pas encore, un manque par case de la carte (`Manque`).
fn ce_qui_manque(ctx: &Contexte, e: &Carte) -> R<Vec<Manque>> {
    let e = &fin_au_terme(e)?;
    let mut manques = Vec::new();
    match e.motif_rupture {
        None => manques.push(manque(
            "motif_rupture",
            "Motif de la rupture",
            "pourquoi le contrat a pris fin.",
            "Le motif de la rupture (S21.G00.62.002) est obligatoire, et un CDI n'en a aucun par défaut.",
        )),
        // CCH-12 de S21.G00.62.003 : la date de notification, obligatoire pour ces motifs (`fctu::notification_exigee`).
        Some(motif) if e.notification_rupture.is_none() && declarations::fctu::notification_exigee(motif)? => manques.push(manque(
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
            format!("le préavis de la rupture, ses dates s'il y en a un, ou « pas de clause de préavis ». {}", preavis_dit(ctx, e)?),
            "Le bloc S21.G00.63 est obligatoire dans un signalement de fin de contrat (cardinalité 1,*) : même sans préavis, il \
se déclare, avec le type « 90 - pas de clause de préavis applicable ».",
        )),
        Some(p) if declarations::fctu::preavis_non_effectue(&p.genre)? && e.dernier_jour_paye.is_none() => manques.push(manque(
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
/// `debut`, `numero`, `fin`, `motif_rupture`, `preavis`, tels que la saisie les ecrit), sa nature, ce qui manque pour la
/// signaler (`a_saisir`), son obligation (`obligations::de_la_fin`) et sa pastille ; la route `fins-de-contrat` y ajoute
/// ses phrases (`qui_dit`, `contrat_dit`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct FinASignaler {
    pub salarie: IdSalarie,
    pub mois: Mois,
    debut: Option<Jour>,
    pub en_retard: bool,
    pub numero: Option<String>,
    pub fin: Jour,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    contrat_dit: Option<String>,
}

/// Les contrats qui finissent dans `mois` et dont la fin SE SIGNALE, chacun avec
/// son obligation (`obligations::de_la_fin`) comptee au jour `aujourdhui`. UN SEUL ENDROIT : la route `fins-de-contrat` (la
/// colonne du mois) et les rappels de l'ouverture (`rappels`) la lisent.
pub(crate) fn fins_a_signaler(ctx: &Contexte, mois: Mois, aujourdhui: Jour) -> R<Vec<FinASignaler>> {
    // Un signalement de fin ne se doit que pour un engagement valide, lu tel qu'il est ecrit (`tels_qu_ecrits`) : un
    // brouillon ne part pas, et ce qui ne fait que lire ne recalcule rien. Un mois sans saisie n'a aucune fin.
    let Some(saisie) = engagement::stock::vue_du_mois(&ctx.racine, mois)? else {
        return Ok(Vec::new());
    };
    let journal = envois::journal::lu(&ctx.racine)?;
    let etats = engagement::etats::tels_qu_ecrits(ctx, mois, &journal.departs)?;
    let mut out = Vec::new();
    for (i, carte) in saisie.engagements.iter().enumerate() {
        if etats.get(i).and_then(|x| x.valide()).is_none() {
            continue;
        }
        let e = avec_ses_bornes(carte);
        let nat = nature_de(&e)?;
        // La regle est celle de la nature et du motif (`Nature::signalement_fctu`) : un CDDU rompu avant son terme se signale.
        if !nat.signalement_fctu(e.motif_rupture) {
            continue;
        }
        let Some(fin) = e.fin else { continue };
        if !(mois.premier_jour() <= fin && fin <= mois.dernier_jour()) {
            continue;
        }
        let salarie = e.salarie.clone().ok_or_else(|| socle::fichiers::absent("salarie").reclasser(Classe::Manque))?;
        let a_saisir = ce_qui_manque(ctx, &e)?;
        // LE DELAI ET OU ON EN EST (`obligations::de_la_fin`).
        let obligation = crate::obligations::de_la_fin(ctx, Some(salarie.as_str()), fin, &journal.lignes, aujourdhui)?;
        // Due : il manque de quoi signaler, ou le delai court. Sa pastille : celle du delai passe
        // (« en retard » l'emporte : la date butoir ne se tait pas parce qu'une case manque), sinon ce qui manque, sinon
        // celle du delai.
        let incomplet = !a_saisir.is_empty();
        let du = incomplet || obligation.du;
        let en_retard = obligation.etat == crate::obligations::EN_RETARD;
        let pastille = if incomplet && !en_retard { Pastille::de("a_faire", "à compléter") } else { obligation.pastille.clone() };
        out.push(FinASignaler {
            salarie,
            mois,
            debut: e.debut,
            en_retard,
            numero: e.numero.clone(),
            fin,
            libelle_nature: nat.libelle.clone(),
            motif_rupture: e.motif_rupture,
            preavis: e.preavis.clone(),
            a_saisir,
            du,
            pastille,
            obligation,
            qui_dit: None,
            contrat_dit: None,
        });
    }
    Ok(out)
}

/// La route `fins-de-contrat`.
fn fins_de_contrat(ctx: &Contexte, c: ChargeMois) -> R<ReponseFins> {
    let mut fins = fins_en_retard_avant(ctx, c.mois, ctx.aujourdhui)?;
    fins.extend(fins_a_signaler(ctx, c.mois, ctx.aujourdhui)?);
    // Ce que l'ecran lit de chaque fin : qui, quelle nature, quel jour (et le mois d'une fin d'un mois passe), et son
    // contrat.
    for f in &mut fins {
        let fin = f.fin.jj_mm_aaaa();
        let ailleurs = if f.mois != c.mois { format!(" ({})", f.mois.libelle(Graphie::Accentuee)) } else { String::new() };
        f.qui_dit = Some(format!("{}, {}, fin le {}{}", ctx.nomme(f.salarie.as_str()), f.libelle_nature, fin, ailleurs));
        f.contrat_dit = Some(format!("contrat {}, fin le {}", numero_dit(&f.numero), fin));
    }
    Ok(ReponseFins { phrase: format!("Fins de contrat du mois : {}.", fins.len()), mois: c.mois, fins })
}

/// Combien de mois en arriere un signalement de fin en retard se rappelle : une annee de paies.
const MOIS_EN_ARRIERE: u32 = 12;

/// LES FINS D'AVANT, EN RETARD : celles des mois qui precedent `mois` (sur `MOIS_EN_ARRIERE`) dont le signalement est du
/// et sa date butoir passee. Une echeance manquee reste due -- le signalement se fait « dans les cinq jours ouvres qui
/// suivent la survenance de la fin du contrat » (cahier technique DSN 2026.1, p. 107, `fctu-base.json`, sa `note`) -- et
/// elle se dit sur le mois qu'on regarde, pas seulement sur celui ou le contrat a fini.
pub(crate) fn fins_en_retard_avant(ctx: &Contexte, mois: Mois, aujourdhui: Jour) -> R<Vec<FinASignaler>> {
    let mut out = Vec::new();
    let mut m = mois;
    for _ in 0..MOIS_EN_ARRIERE {
        m = m.precedent();
        let mut celles: Vec<FinASignaler> = fins_a_signaler(ctx, m, aujourdhui)?.into_iter().filter(|f| f.du && f.en_retard).collect();
        celles.extend(out);
        out = celles;
    }
    Ok(out)
}

// ------------------------------------------------------------ les contrats d'un jour


/// `arret-contrats` : les contrats d'un salarie qui couvrent un jour.
fn arret_contrats(ctx: &Contexte, c: ChargeArretContrats) -> R<ReponseContrats> {
    let cle = cle_du_nom(ctx, &c.salarie)?;
    let mut contrats = contrats_au(ctx, &cle, c.jour)?;
    let phrase = match contrats.len() {
        0 => format!("Aucun contrat de {} ne couvre le {} : il n'y a rien à suspendre.", ctx.nomme(&cle), c.jour.jj_mm_aaaa()),
        n => format!("Contrats qu'un arrêt peut toucher : {}.", n),
    };
    // Chaque contrat se dit en une ligne : son numero et sa periode.
    for k in &mut contrats {
        let periode = match &k.fin {
            Some(f) => format!("du {} au {}", k.debut.jj_mm_aaaa(), f.jj_mm_aaaa()),
            None => format!("depuis le {}", k.debut.jj_mm_aaaa()),
        };
        k.dit = Some(format!("{}, {}", numero_dit(&k.numero), periode));
    }
    Ok(ReponseContrats { phrase, contrats })
}

/// Les contrats de la fiche `cle` qui couvrent `jour`, lus dans les saisies, du plus ancien debut au plus recent.
fn contrats_au(ctx: &Contexte, cle: &str, jour: Jour) -> R<Vec<ContratAuJour>> {
    let mut sorties: Vec<ContratAuJour> = Vec::new();
    let mut vus: BTreeSet<(Jour, Option<String>)> = BTreeSet::new();
    let mut mois = engagement::stock::mois_saisis(&ctx.racine)?;
    mois.reverse();
    for m in mois {
        for (c, e) in crate::mois::engagements_du_mois(ctx, m)? {
            if c.as_str() != cle {
                continue;
            }
            let (Some(debut), fin) = crate::mois::bornes_brutes(&e) else { continue };
            // Un contrat paye au mois sans fin ecrite (un CDI qui court) couvre tout jour depuis son debut : c'est sur lui
            // qu'un arret ou une suspension se pose. Un contrat dont la nature n'est pas encore dite n'est pas tenu pour
            // paye au mois : il ne couvre que ses bornes. Le mois le plus recent qui le saisit fait foi (une fin ecrite
            // plus tard l'emporte) : il est retenu, couvrant ou non, avant les mois plus anciens.
            let mensuel = e.nature.map(droit::natures::NatureDeContrat::proprietes).transpose().map_err(|e| e.reclasser(Classe::Refuse))?.is_some_and(|n| n.mensuelle());
            if fin.is_none() && !mensuel {
                continue;
            }
            if !vus.insert((debut, e.numero.clone())) {
                continue;
            }
            if !(debut <= jour && fin.is_none_or(|f| jour <= f)) {
                continue;
            }
            sorties.push(ContratAuJour {
                nom: ctx.nomme(cle),
                salarie: c,
                numero: e.numero.clone(),
                debut,
                fin,
                mois: m,
                spectacle: e.spectacle.as_ref().map(ToString::to_string),
                // Paye au mois (`Nature::mensuelle`) : son mois de paie se tire du moteur, et un arret y appelle le maintien
                // de salaire (L1226-1 : pas pour un intermittent).
                mensuel,
                nature: e.nature,
                dit: None,
            });
        }
    }
    sorties.sort_by(|a, b| a.debut.cmp(&b.debut));
    Ok(sorties)
}

/// Les mois des DSN qui ont porte un imprevu, tels que l'ecran les lit (« de novembre 2026 »).
fn mois_de_la_dsn(mois: &[Mois]) -> Vec<String> {
    mois.iter().map(|&m| crate::regularisation::de_mois_en_clair(m)).collect()
}
