//! Les routes du mois : `etat-par-engagement`, `etat-du-mois`, `mois-a-ouvrir`, et les lectures
//! qu'elles partagent (`bilan_du_mois`, `fichiers_deposables`) : l'etat de chaque engagement (sa validation, sa DPAE,
//! son contrat, ses documents, la DSN du mois), l'etat du mois, et l'annule-et-remplace.
//!
//! UN PROCESSUS PAR ROUTE, ET CHAQUE FICHIER LU UNE FOIS : le journal des
//! depots, la saisie de chaque mois et le Contexte ne se relisent pas d'un
//! etat a l'autre. Les etats du journal sont ceux de `etats.rs`, appeles
//! directement sur les lignes deja lues.
//!
//! AUCUN CALCUL D'UN ENGAGEMENT. La saisie se charge sans calculer (`charger_saisie` : les engagements, le contrat qui
//! deborde, les numeros attribues en memoire).
//!
//! LES FORMES. Chaque charge est une structure lue une fois a l'entree (`Charge::lire`) ; chaque reponse, une
//! structure : l'etat d'un engagement (`Ligne`, dont la page lit `LigneDePage`), le bilan du mois (`Bilan`, ses `Etape`
//! et leurs `Groupe`), les carres d'un engagement (`Carre`), le geste d'un clic (`Action`). Ce que ce module lit d'un
//! autre domaine se lit tel qu'il est : le journal des envois (`envois::etats` : les DPAE, les signatures, les avenants,
//! les depots ; `envois::journal`), l'engagement ecrit et sa saisie (`engagement` : l'identite, la frise, l'objet brut),
//! la fiche d'un spectacle (son titre), le dossier d'un contrat signe ailleurs (`gestes::signature`), le journal des
//! ecritures (`compagnie::pieces_ecrites`) et un mois importe (`import::mois`).

use droit::fiche::Lue as _;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use droit::echeances::Echeance;
use socle::date::Jour;
use socle::mois::{Graphie, Mois};
use engagement;
use engagement::etats::ARefaire;
use engagement::numeros::NumeroContrat;
use envois::etats::{self, quand_lisible, DOCUMENTS, DPAE, DSN, SIGNATURE};
use socle::echec::{Classe, R};
use socle::echec::erreur;
use socle::fichiers::sha256_hex;
use socle::echec::Echec;
use crate::catalogue::{Acte, Fiche};
use crate::catalogue::Route;
use crate::catalogue::Ecran;
use crate::SansCharge;
use socle::ecran::couleur_de_l_etat;
use crate::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use crate::depots::{Designation, EmbaucheDesignee, Quoi};
use crate::gestes::signature::ContratSigneDit;

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Lecture;

/// La route de ce module, et sa fiche ; les deux etats d'un mois se servent de l'etat garde (`etat_garde::ROUTES`).
pub const ROUTES: &[Fiche] = &[Fiche {
    route: Route::MoisAOuvrir,
    geste: "Dire quel mois ouvrir : le plus ancien dont une étape reste à faire.",
    charge: &[],
    exemple: "{}",
    rend: "mois (celui à ouvrir), courant, reste (les étapes qui restent)",
    refus: &[crate::catalogue::ILLISIBLE],
    acte: Acte::Lit,
    phrase: 0,
    ecran: ECRAN,
}];

pub(crate) type Paire = (String, String);

// ------------------------------------------------------------------ les charges

/// La charge des etats d'un mois (`etat-par-engagement`, `etat-du-mois`, `mois` : `etat_garde`) : le mois.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeMois {
    pub(crate) mois: Mois,
}

// ------------------------------------------------------------------ les reponses

/// Une etape du mois, et le carre d'un engagement qui la porte : sa cle, celle du journal des envois (`envois::etats`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuelleEtape {
    Dpae,
    Signature,
    Dsn,
    Documents,
}

impl QuelleEtape {
    pub(crate) fn cle(self) -> &'static str {
        match self {
            QuelleEtape::Dpae => DPAE,
            QuelleEtape::Signature => SIGNATURE,
            QuelleEtape::Dsn => DSN,
            QuelleEtape::Documents => DOCUMENTS,
        }
    }

    fn nom(self) -> &'static str {
        match self {
            QuelleEtape::Dpae => "DPAE",
            QuelleEtape::Signature => "Contrats",
            QuelleEtape::Dsn => "DSN",
            QuelleEtape::Documents => "Documents",
        }
    }
}

impl Serialize for QuelleEtape {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.cle())
    }
}

/// L'etat d'une case, d'un carre, d'un choix : `fait`, `a_faire`, `en_attente`, `refuse` ; `rien` (un mois sans
/// engagement), `information` (un mois importe). Sa couleur est celle de tout l'ecran (`couleur_de_l_etat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EtatCase {
    Fait,
    AFaire,
    EnAttente,
    Refuse,
    Rien,
    Information,
}

impl EtatCase {
    fn nom(self) -> &'static str {
        match self {
            EtatCase::Fait => "fait",
            EtatCase::AFaire => "a_faire",
            EtatCase::EnAttente => "en_attente",
            EtatCase::Refuse => "refuse",
            EtatCase::Rien => "rien",
            EtatCase::Information => "information",
        }
    }

    fn couleur(self) -> &'static str {
        couleur_de_l_etat(self.nom())
    }
}

/// LE GESTE D'UN CLIC, en routes du catalogue, celles que l'ecran et le LLM appellent : la route qu'il ouvre (`route`,
/// un apercu le plus souvent), sa charge (`charge`, en sa forme), la route de l'acte qui suit, s'il y en a un (`puis`), et
/// ce que dit le bouton (`libelle`). La page ouvre sa boite d'apres la route. `tester` : le depot d'une DSN, dont le test
/// part des qu'elle est ecrite.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Action {
    route: Route,
    charge: ChargeDuGeste,
    puis: Option<Route>,
    libelle: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    tester: bool,
}

/// LA CHARGE D'UN GESTE : celle de la route qu'il ouvre, en sa forme, telle que la route la lit.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum ChargeDuGeste {
    Mois(ChargeMois),
    Depot(Designation),
    Recu(crate::contrats::ChargeRecu),
    Avenant(crate::contrats::ChargeAvenant),
    FinContrat(crate::imprevus::ChargeFinContratSignaler),
    Arret(crate::imprevus::ChargeArretSignaler),
    ObligationDArret(ChargeObligationDArret),
    Annee(ChargeAnnee),
    Revue(crate::rappels::ChargeRevue),
}

/// La charge du geste d'une obligation d'un arret (`accident-declare`, `attestation-declare`) : le salarie et son dernier
/// jour travaille, qui designent l'arret.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChargeObligationDArret {
    pub(crate) salarie: String,
    pub(crate) dernier_jour: String,
}

/// La charge du geste d'une echeance de l'annee (`cloture`, `export-compta`) : l'annee.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChargeAnnee {
    pub(crate) annee: String,
}

macro_rules! charge_du_geste {
    ($($t:ty => $v:ident),* $(,)?) => {
        $(impl From<$t> for ChargeDuGeste {
            fn from(c: $t) -> ChargeDuGeste {
                ChargeDuGeste::$v(c)
            }
        })*
    };
}

charge_du_geste! {
    ChargeMois => Mois,
    Designation => Depot,
    crate::contrats::ChargeRecu => Recu,
    crate::contrats::ChargeAvenant => Avenant,
    crate::imprevus::ChargeFinContratSignaler => FinContrat,
    crate::imprevus::ChargeArretSignaler => Arret,
    ChargeObligationDArret => ObligationDArret,
    ChargeAnnee => Annee,
    crate::rappels::ChargeRevue => Revue,
}

/// L'`Action` de `route` sur `charge`, la charge de la route en sa forme.
pub(crate) fn action(route: Route, charge: impl Into<ChargeDuGeste>, puis: Option<Route>, libelle: String) -> Action {
    Action { route, charge: charge.into(), puis, libelle, tester: false }
}

/// Le compte d'une case : un nombre, ou ce qui est revenu sur ce qui est parti.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum Compte {
    Nombre(usize),
    Recus { recus: usize, partis: usize },
}

/// UN GROUPE D'UNE ETAPE, une case du panneau : son etat, son compte, ce qu'elle ecrit (`compte_texte` en gras : « 2 »,
/// « 1/2 », « Lyon », ou rien ; `quoi` : « à déposer », « reçu »), sa phrase entiere (`phrase_du_groupe`), son echeance,
/// le geste qu'ouvre un clic, les colonnes qu'elle couvre, sa couleur, et si elle se dessine coupee (une attente : la
/// moitie faite, la moitie qui attend le retour).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Groupe {
    pub(crate) etat: EtatCase,
    compte: Option<Compte>,
    compte_texte: String,
    quoi: String,
    phrase: Option<String>,
    echeance: Option<Echeance>,
    action: Option<Action>,
    colonnes: Vec<String>,
    couleur: &'static str,
    coupee: bool,
}

impl Groupe {
    /// Un groupe : son etat, son compte, ce qu'il ecrit, son echeance et son geste ; sa phrase, les deux joints.
    fn de(etat: EtatCase, compte: Option<Compte>, compte_texte: String, quoi: String, echeance: Option<Echeance>, action: Option<Action>) -> Groupe {
        let phrase = if compte_texte.is_empty() { quoi.clone() } else { format!("{} {}", compte_texte, quoi) };
        Groupe { etat, compte, compte_texte, quoi, phrase: Some(phrase), echeance, action, colonnes: Vec::new(), couleur: etat.couleur(), coupee: etat == EtatCase::EnAttente }
    }

    /// Ce qui est fait : le nom de l'etape et la coche, rien d'autre.
    fn fait() -> Groupe {
        Groupe { phrase: None, ..Groupe::de(EtatCase::Fait, None, String::new(), String::new(), None, None) }
    }

    /// Un mois sans aucun engagement : l'etape n'a rien a faire, elle n'est pas faite pour autant. Elle se dit en gris.
    fn rien_ce_mois(colonnes: Vec<String>) -> Groupe {
        Groupe { colonnes, ..Groupe::de(EtatCase::Rien, None, String::new(), "rien ce mois-ci".into(), None, None) }
    }

    fn sous(mut self, colonnes: &[String]) -> Groupe {
        self.colonnes = colonnes.to_vec();
        self
    }
}

/// UNE ETAPE DU MOIS : sa cle, son nom, ses groupes, et si elle est la suivante (la premiere qui a quelque chose a faire
/// de notre cote). Un mois importe dit sa phrase (`import::mois::etapes`) ; les documents nomment ce qui part.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Etape {
    pub(crate) cle: QuelleEtape,
    nom: &'static str,
    pub(crate) groupes: Vec<Groupe>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phrase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    suivante: bool,
}

impl Etape {
    fn de(cle: QuelleEtape, groupes: Vec<Groupe>) -> Etape {
        Etape { cle, nom: cle.nom(), groupes, phrase: None, detail: None, suivante: false }
    }
}

/// LE BILAN DU MOIS (`bilan_du_mois`), ce que rend `etat-du-mois` : sa phrase, ses colonnes (G d'abord), ses etapes.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Bilan {
    mois: Mois,
    phrase: String,
    colonnes: Vec<String>,
    pub(crate) etapes: Vec<Etape>,
}

/// `mois-a-ouvrir` : le mois a ouvrir, celui du jour, et les etapes qui restent au premier.
#[derive(Debug, Serialize)]
struct MoisAOuvrir {
    phrase: String,
    mois: Mois,
    courant: Mois,
    reste: Vec<QuelleEtape>,
}

/// `etat-par-engagement` : la phrase du mois, celle de chaque engagement (ses carres et sa frise), les lignes que la page
/// lit, et les engagements non actifs du mois.
#[derive(Debug, Serialize)]
struct EtatParEngagement<'a> {
    phrase: String,
    phrases: Vec<String>,
    mois: Mois,
    engagements: Vec<LigneDePage<'a>>,
    non_actifs: Vec<NonActif>,
}

impl Dite for EtatParEngagement<'_> {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Bilan {
    /// La phrase du bilan, celle de l'ecran du mois.
    pub(crate) fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// La phrase d'un engagement dans `etat-par-engagement` : qui, son contrat, puis ce que disent ses carres et sa frise.
fn phrase_de_la_ligne(l: &LigneDePage) -> String {
    let qui = match (l.nom, l.numero) {
        (Some(n), Some(c)) => format!("{} ({})", n, c),
        (Some(n), None) => n.to_string(),
        (None, Some(c)) => format!("contrat {}", c),
        (None, None) => "engagement à compléter".to_string(),
    };
    let dit: Vec<&str> = l.carres.iter().map(|c| c.phrase.as_str()).chain(l.versions.map(|f| f.phrase.as_str())).collect();
    format!("{} : {}", qui, dit.join(" ; "))
}

/// Un engagement non actif du mois (`non_actifs`).
#[derive(Debug, Serialize)]
struct NonActif {
    salarie: Option<String>,
    nom: Option<String>,
    numero: Option<String>,
    debut: String,
    fin: Option<String>,
    non_actif: String,
    non_actif_dit: String,
    dit: String,
    a_faire: Vec<String>,
}

// ------------------------------------------------------------ les groupes

/// Les quatre etapes du mois, dans l'ordre de l'habitude (aucun texte n'impose d'ordre).
const ORDRE: [QuelleEtape; 4] = [QuelleEtape::Dpae, QuelleEtape::Signature, QuelleEtape::Dsn, QuelleEtape::Documents];

/// L'echeance d'un depot de DSN : `mots` le `jour` a `heure`, d'apres `source`.
fn echeance_dsn(jour: Jour, heure: &'static str, source: &str) -> Echeance {
    Echeance { jour, mots: "avant le", heure, source: source.into() }
}

/// Faire signer les contrats du mois : l'apercu, puis la demande.
fn faire_signer(mois: Mois) -> Action {
    action(Route::SignatureApercu, ChargeMois { mois }, Some(Route::DemanderSignature), "Demander la signature…".into())
}

/// Les contrats partis du mois, recus ou a relancer : l'apercu, puis noter chacun recu.
fn contrats_en_attente(mois: Mois) -> Action {
    action(Route::SignatureApercu, ChargeMois { mois }, Some(Route::ContratRecu), "Reçus ou à relancer…".into())
}

/// Les documents du mois : l'apercu, puis l'envoi.
fn envoyer_documents(mois: Mois, libelle: String) -> Action {
    action(Route::EnvoiApercu, ChargeMois { mois }, Some(Route::EnvoyerDocuments), libelle)
}

/// Ce que les retours du mois ont dit (le bilan d'un refus).
fn historique(mois: Mois) -> Action {
    action(Route::HistoriqueMois, ChargeMois { mois }, None, "Voir le bilan…".into())
}

/// Les DPAE du mois a ecrire : l'apercu du depot (qui les ecrit), puis le depot.
fn declarer_embauches(mois: Mois) -> Action {
    action(Route::DepotApercu, Designation::du_mois(Quoi::Dpae, mois), Some(Route::Deposer), "Envoyer les DPAE…".into())
}

/// La DSN du mois a ecrire, ou a reecrire : l'apercu du depot (qui l'ecrit), puis le depot, dont le test part des
/// qu'elle est ecrite (`tester`).
pub(crate) fn deposer_dsn(mois: Mois, nic: Option<&str>, libelle: String) -> Action {
    let mut charge = Designation::du_mois(Quoi::Dsn, mois);
    charge.etablissement = nic.map(String::from);
    Action { tester: true, ..action(Route::DepotApercu, charge, Some(Route::Deposer), libelle) }
}

/// Le geste d'un carre Contrat en attente : noter revenu signe (`recu`) le contrat de l'engagement `p`, ou l'avenant
/// `avenant` (son numero) ; la route `contrat-recu` rend la phrase qui le dit et de quoi l'annuler. Rien pour un
/// engagement qui ne dit pas encore son salarie ou son premier jour.
fn noter_recu(p: &Vue, mois: Mois, libelle: &str, avenant: Option<i64>, recu: bool) -> Option<Action> {
    let charge = crate::contrats::ChargeRecu { mois, salarie: p.salarie.clone()?, debut: p.debut?, numero: p.numero.as_ref().map(|n| n.to_string()), avenant, recu };
    Some(action(Route::ContratRecu, charge, None, libelle.into()))
}

/// L'avenant du contrat de l'engagement `p` : l'apercu, puis sa demande de signature. Rien pour un engagement qui ne
/// dit pas encore son salarie ou son premier jour.
fn avenant(p: &Vue, mois: Mois) -> Option<Action> {
    let charge = crate::contrats::ChargeAvenant { salarie: p.salarie.clone()?, debut: p.debut?, numero: p.numero.as_ref().map(|n| n.to_string()), mois: Some(mois) };
    Some(action(Route::AvenantApercu, charge, Some(Route::DemanderSignature), "Avenant…".into()))
}

/// Une etape qui n'a rien a faire ici pour les engagements de sa case (un contrat commence avant le logiciel) : faite, et
/// elle dit pourquoi (`quoi`).
fn rien_ici(quoi: &str) -> Groupe {
    Groupe::de(EtatCase::Fait, None, String::new(), quoi.into(), None, None)
}

/// `n` engagements auxquels il manque un fait (`A_COMPLETER`) : ce qui reste a faire, en orange, sans geste ; la carte
/// de chacun dit ce qui lui manque.
fn a_completer(n: usize) -> Groupe {
    Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à compléter".into(), None, None)
}

/// `n` engagements complets pas encore valides (`A_VALIDER`) : ce qui reste a faire, en orange, sans geste ; la carte de
/// chacun porte « Valider l'engagement ».
fn a_valider(n: usize) -> Groupe {
    Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à valider".into(), None, None)
}

/// Marque `suivante` la premiere etape qui a quelque chose a faire de notre cote (a faire, ou refuse), et donne a chaque
/// groupe sa phrase entiere (`phrase_du_groupe`).
fn suivante(mut etapes: Vec<Etape>) -> Vec<Etape> {
    let i = etapes.iter().position(|e| e.groupes.iter().any(|x| matches!(x.etat, EtatCase::AFaire | EtatCase::Refuse)));
    for (j, e) in etapes.iter_mut().enumerate() {
        e.suivante = Some(j) == i;
        let nom = e.nom;
        for g in &mut e.groupes {
            g.phrase = phrase_du_groupe(nom, g);
        }
    }
    etapes
}

/// LA PHRASE D'UNE CASE, entiere, telle qu'on la lit hors de la case (son titre, la ligne de commande) : l'etape, ce
/// qu'elle ecrit et son echeance (« DPAE : 2 à déposer, avant le 19/01/2026 »). Une etape qui n'a rien a faire dans un
/// mois sans engagement n'en a pas : la phrase du mois le dit une fois (`bilan_du_mois`).
fn phrase_du_groupe(nom: &str, g: &Groupe) -> Option<String> {
    if g.etat == EtatCase::Rien {
        return None;
    }
    let ecrit = [g.compte_texte.as_str(), g.quoi.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
    let ecrit = if ecrit.is_empty() && g.etat == EtatCase::Fait { "fait".to_string() } else { ecrit };
    match g.echeance.as_ref().map(Echeance::texte).filter(|t| !t.is_empty()) {
        Some(t) => Some(format!("{} : {}, {}", nom, ecrit, t)),
        None => Some(format!("{} : {}", nom, ecrit)),
    }
}

/// LA PHRASE DU MOIS : ce qu'en disent ses cases, l'une apres l'autre (`phrase_du_groupe`) ; un mois sans engagement le
/// dit une fois en tete (`debut`), sans redire ses etapes qui n'ont rien a faire.
fn phrase_du_mois(debut: Option<String>, etapes: &[Etape]) -> String {
    let cases = etapes.iter().flat_map(|e| &e.groupes).filter_map(|g| g.phrase.clone());
    let morceaux: Vec<String> = debut.into_iter().chain(cases).collect();
    format!("{}.", morceaux.join(". "))
}

// ------------------------------------------------------------ les colonnes
//
// Les cases des quatre lignes s'alignent sur les memes colonnes. G, la grande, porte ce qui est le plus avance ; une
// colonne de cote s'ouvre par vague de ce qui est en cours (des embauches nouvelles, un contrat qui change, un
// etablissement dont la DSN diverge), et une vague qui en est au meme point que G y rentre : on ne separe que le temps
// d'un changement. Chaque ligne fond les colonnes voisines qui en sont au meme point en une seule case.

/// Ou en est un engagement, ligne par ligne. Une ligne qui ne le concerne pas (la DPAE d'un contrat qui a commence un
/// mois d'avant, la signature d'un salarie au mois) est faite pour lui. `dsn` : la classe de son etablissement
/// (`groupes_dsn`) ; aucune pour une carte qui ne nomme pas encore son salarie, dans une compagnie de plusieurs
/// etablissements (sa DSN est a completer).
/// Le point d'un engagement auquel il manque un fait, sur chacune de ses lignes.
const A_COMPLETER: &str = "a_completer";

/// Le point d'un engagement complet que personne n'a encore valide, sur chacune de ses lignes : rien ne part d'un
/// engagement en saisie (`routes/depots.rs`, `routes/documents.rs`).
const A_VALIDER: &str = "a_valider";

/// Le point, sur la ligne DPAE et sur la ligne Contrats, d'un contrat commence avant le logiciel : rien a faire ici.
const RIEN_A_DECLARER: &str = "rien_a_declarer";
const RIEN_A_SIGNER: &str = "rien_a_signer";

#[derive(Clone, Copy)]
struct Point {
    dpae: &'static str,
    contrat: &'static str,
    documents: &'static str,
    dsn: Option<usize>,
}

impl Point {
    fn de(p: &Vue, dsn: Option<usize>) -> Point {
        let embauche = p.contrat.premier_mois();
        // UN CONTRAT COMMENCE AVANT LE LOGICIEL (`engagement::stock::commence_avant_le_logiciel`) : sa DPAE et son contrat
        // se sont faits ailleurs ; ici, rien a declarer ni a signer.
        let anterieur = p.contrat.anterieur();
        // UN ENGAGEMENT A COMPLETER, jamais valide (`ligne_a_completer`) : rien n'est parti de lui, et tout ce qui partira
        // attend qu'il soit complet. Il reste a faire, en orange, pour qu'on ne l'oublie pas.
        if p.a_completer.is_some() {
            let spectacle = p.spectacle.is_some();
            return Point {
                dpae: if anterieur { RIEN_A_DECLARER } else if embauche { A_COMPLETER } else { "fait" },
                contrat: if anterieur { RIEN_A_SIGNER } else if embauche && spectacle { A_COMPLETER } else { "fait" },
                documents: A_COMPLETER,
                dsn,
            };
        }
        // UN ENGAGEMENT EN SAISIE, complet : rien ne part de lui tant qu'il n'est pas valide ; chaque ligne qui
        // l'attend le dit.
        let ecrit = p.contrat.ecrit();
        let ailleurs = p.contrat.ailleurs();
        if !p.valide() {
            return Point {
                dpae: if anterieur { RIEN_A_DECLARER } else if embauche { A_VALIDER } else { "fait" },
                contrat: if anterieur || ailleurs { RIEN_A_SIGNER } else if embauche && ecrit { A_VALIDER } else { "fait" },
                documents: A_VALIDER,
                dsn,
            };
        }
        // LA DPAE A DEPOSER est celle que dit la liste des DPAE a faire (`Moteur::embauches_a_declarer`, `a_declarer`).
        let d = &p.etats.dpae;
        let dpae = if anterieur {
            RIEN_A_DECLARER
        } else if !embauche {
            "fait"
        } else if d.refusee {
            "refuse"
        } else if d.a_declarer {
            "a_faire"
        } else {
            "fait"
        };
        let s = &p.etats.signature;
        // Le contrat se fait signer au mois de l'embauche ; un avenant, a chaque mois du contrat : tant qu'il n'est pas
        // revenu signe, chacun de ses mois le dit.
        // Un contrat signe hors du logiciel ne se fait pas signer ici, ses avenants si.
        let contrat = if (ecrit && !embauche || ailleurs) && s.caduc {
            "a_refaire"
        } else if (ecrit && !embauche || ailleurs) && s.avenant_attendu {
            "en_attente"
        } else if anterieur || ailleurs {
            RIEN_A_SIGNER
        } else if !(embauche && ecrit) {
            "fait"
        } else if !s.parti() {
            "a_envoyer"
        } else if s.caduc {
            "a_refaire"
        } else if s.recu.is_none() || s.avenant_attendu {
            "en_attente"
        } else {
            "fait"
        };
        let o = &p.etats.documents;
        let documents = if o.caduc {
            "a_renvoyer"
        } else if o.parti() {
            "fait"
        } else {
            "a_envoyer"
        };
        Point { dpae, contrat, documents, dsn }
    }

    /// Sa vague : ce qui reste a faire de notre cote. Une DPAE partie l'est, revenue refusee ou non ; un contrat parti
    /// l'est, signe ou non.
    fn vague(&self) -> (bool, &'static str, &'static str, Option<usize>) {
        let contrat = if matches!(self.contrat, "a_envoyer" | "a_refaire") { self.contrat } else { "parti" };
        (self.dpae == "a_faire", contrat, self.documents, self.dsn)
    }

    /// Combien de ses lignes sont faites de notre cote : l'ordre des colonnes, la plus avancee en G.
    fn avance(&self, dsn_fait: &[bool]) -> usize {
        [!matches!(self.dpae, "a_faire" | A_COMPLETER | A_VALIDER), matches!(self.contrat, "fait" | "en_attente" | RIEN_A_SIGNER), self.documents == "fait", self.dsn.is_some_and(|k| dsn_fait.get(k) == Some(&true))]
            .iter()
            .filter(|x| **x)
            .count()
    }
}

/// Une colonne : son nom, la classe DSN de ses etablissements, et ses engagements (rangs dans `etat_par_engagement`).
struct Colonne {
    nom: String,
    dsn: Option<usize>,
    membres: Vec<usize>,
}

/// LES COLONNES DU MOIS. Les engagements d'une meme vague font une colonne ; la plus avancee est G. Une DPAE revenue
/// refusee ne reste pas dans G, qui dit ce qui est fait : elle sort a cote. Un etablissement sans engagement du mois
/// dont la DSN diverge a sa colonne. `dsn_fait` : par classe DSN (`groupes_dsn`, une au moins), si elle est faite.
fn colonnes(points: &[Point], dsn_fait: &[bool]) -> Vec<Colonne> {
    let mut vagues: Vec<((bool, &'static str, &'static str, Option<usize>), Vec<usize>)> = Vec::new();
    for (i, p) in points.iter().enumerate() {
        match vagues.iter_mut().find(|(v, _)| *v == p.vague()) {
            Some((_, m)) => m.push(i),
            None => vagues.push((p.vague(), vec![i])),
        }
    }
    let mut cols: Vec<(Option<usize>, Vec<usize>)> = vagues.into_iter().map(|(v, m)| (v.3, m)).collect();
    cols.sort_by_key(|(_, m)| std::cmp::Reverse(points[m[0]].avance(dsn_fait)));
    if let Some((dsn, g)) = cols.first_mut() {
        let (refusees, reste): (Vec<usize>, Vec<usize>) = g.iter().partition(|i| points[**i].dpae == "refuse");
        if !refusees.is_empty() && !reste.is_empty() {
            let dsn = *dsn;
            *g = reste;
            cols.insert(1, (dsn, refusees));
        }
    }
    for c in 0..dsn_fait.len() {
        if !cols.iter().any(|(d, _)| *d == Some(c)) {
            cols.push((Some(c), Vec::new()));
        }
    }
    const COTE: &str = "ABCDEFHIJKLMNOPQRSTUVWXYZ";
    cols.into_iter()
        .enumerate()
        .map(|(k, (dsn, membres))| {
            let nom = match k {
                0 => "G".to_string(),
                k => match COTE.chars().nth(k - 1) {
                    Some(c) => c.to_string(),
                    None => format!("C{}", k),
                },
            };
            Colonne { nom, dsn, membres }
        })
        .collect()
}

/// Une ligne en cases : les colonnes voisines au meme point (`point`) se fondent. Chaque case : son point, ses
/// engagements, ses colonnes, et la classe DSN de sa premiere colonne.
fn cases_de_la_ligne(cols: &[Colonne], point: impl Fn(&Colonne) -> String) -> Vec<(String, Vec<usize>, Vec<String>, Option<usize>)> {
    let mut out: Vec<(String, Vec<usize>, Vec<String>, Option<usize>)> = Vec::new();
    for c in cols {
        let p = point(c);
        match out.last_mut() {
            Some((q, m, n, _)) if *q == p => {
                m.extend(&c.membres);
                n.push(c.nom.clone());
            }
            _ => out.push((p, c.membres.clone(), vec![c.nom.clone()], c.dsn)),
        }
    }
    out
}

/// Le point d'une colonne sur une ligne : le premier etat de `ordre` qu'un de ses engagements porte ; sinon `fait`, ou le
/// point que tous ses engagements partagent (rien a faire ici pour chacun).
fn point_de(c: &Colonne, points: &[Point], ligne: fn(&Point) -> &'static str, ordre: &[&'static str]) -> String {
    if let Some(o) = ordre.iter().find(|o| c.membres.iter().any(|i| ligne(&points[*i]) == **o)) {
        return o.to_string();
    }
    let mut tous = c.membres.iter().map(|i| ligne(&points[*i]));
    match tous.next() {
        Some(p) if tous.all(|q| q == p) => p.to_string(),
        _ => "fait".to_string(),
    }
}

// ------------------------------------------------------------ petits outils

/// Le nom d'un etablissement dans une case : sa commune (celle de la fiche entreprise pour le principal), sinon son NIC.
fn nom_de_l_etablissement(ent: &droit::fiche::ChampsEntreprise, nic: &str) -> String {
    let commune = if Some(nic) == ent.nic_principal() {
        ent.commune_affichage.remplie().or(ent.commune.portee()).map(|c| c.trim().to_string())
    } else {
        ent.secondaires().into_iter().find(|e| e.nic.portee() == Some(nic)).and_then(|e| e.commune_affichage.remplie().or(e.commune.portee()).map(|c| c.trim().to_string()))
    };
    match commune {
        Some(c) => c,
        None => format!("NIC {}", nic),
    }
}

/// « du 02/11/2026 au 30/11/2026 », « depuis le 02/11/2026 » sans fin : une periode telle qu'une phrase la dit.
fn periode_dite(debut: Jour, fin: Option<Jour>) -> String {
    match fin {
        Some(f) => format!("du {} au {}", debut.jj_mm_aaaa(), f.jj_mm_aaaa()),
        None => format!("depuis le {}", debut.jj_mm_aaaa()),
    }
}

/// Ce qu'ecrit une case (`etat`), et ce qu'un test refuse en a dit, s'il en a dit quelque chose.
fn avec_refus(etat: &str, refus: Option<String>) -> String {
    match refus {
        Some(r) => format!("{} · {}", etat, r),
        None => etat.to_string(),
    }
}

fn plur(k: usize, un: &'static str, des: &'static str) -> &'static str {
    if k <= 1 {
        un
    } else {
        des
    }
}

/// Un refus du chargement d'une saisie. Les refus que l'ecran ne tient pas
/// pour un defaut du logiciel -- le spectacle pas encore choisi, les jours ou
/// les heures pas encore saisis (`SaisieIncomplete`, leur liste est
/// `engagement::saisie_incomplete`) -- gardent leur classe ; tout autre est
/// celui du moteur.
pub(crate) fn classer_saisie(e: Echec) -> Echec {
    let incomplete = engagement::saisie_incomplete(&e.phrase);
    Echec { classe: if incomplete { Some(Classe::SaisieIncomplete) } else { None }, ..e }
}

/// Quand le contrat (ou son dernier avenant) de l'engagement `numero` du salarie `cle` est parti (`departs`) ; rien s'il
/// n'est pas parti.
pub(crate) fn contrat_parti_le(departs: &engagement::departs::Departs, cle: &str, numero: Option<&str>) -> Option<String> {
    let n = numero?;
    departs.de(cle, n).iter().rev().find(|d| d.envoi == engagement::departs::Envoi::Contrat).map(|d| d.le.clone())
}

/// Ce qui est a refaire pour l'engagement `e` : le present compare a la version sur laquelle chaque envoi est parti
/// (`engagement::etats::a_refaire`) ; rien pour un engagement en saisie.
pub(crate) fn a_refaire_de(ctx: &Contexte, e: &engagement::etats::Etat, departs: &engagement::departs::Departs) -> R<Vec<ARefaire>> {
    match e.valide() {
        Some(v) => engagement::etats::a_refaire(ctx, v, departs),
        None => Ok(Vec::new()),
    }
}

/// Les contrats de `mois` dont l'avenant est a faire signer : `(cle du salarie, numero)` des engagements dont les clauses
/// ont change depuis la version sur laquelle le contrat est parti (`a_refaire_parmi`, les contrats seuls).
pub(crate) fn avenants_a_faire(ctx: &Contexte, mois: Mois) -> R<HashSet<(String, String)>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let etats = engagement::etats::du_mois(ctx, mois, &departs)?;
    let avenant = |s: engagement::departs::Envoi| s == engagement::departs::Envoi::Contrat;
    let mut out = HashSet::new();
    for e in &etats {
        let Some(v) = e.valide() else { continue };
        if !engagement::etats::a_refaire_parmi(ctx, v, &departs, &avenant)?.iter().any(|a| matches!(a, ARefaire::Avenant { .. })) {
            continue;
        }
        let b = &*e.objet().brut;
        if let (Some(cle), Some(numero)) = (&b.saisie.carte.salarie, b.saisie.carte.numero.clone()) {
            out.insert((cle.to_string(), numero));
        }
    }
    Ok(out)
}

/// La date d'embauche que la DPAE partie a declaree, quand elle n'est plus celle de l'engagement (`ARefaire::Dpae`).
pub(crate) fn debut_declare(refaire: &[ARefaire]) -> Option<String> {
    refaire.iter().find_map(|a| match a {
        ARefaire::Dpae { debut_declare, .. } => Some(debut_declare.clone()),
        _ => None,
    })
}

/// L'etablissement (NIC) dont releve l'engagement `e` : celui de la fiche de son salarie, ou celui que l'engagement dit
/// (`etablissement`) ; aucun pour une carte qui ne nomme pas encore son salarie. Une fiche qui manque, un etablissement
/// qui ne se rattache pas, se refusent.
pub(crate) fn nic_de_l_engagement(ctx: &Contexte, e: &engagement::etats::Etat) -> R<Option<String>> {
    nic_de_l_engagement_ecrit(ctx, &e.objet().brut)
}

/// `nic_de_l_engagement`, sur l'engagement tel qu'il est ecrit.
pub(crate) fn nic_de_l_engagement_ecrit(ctx: &Contexte, brut: &engagement::stock::Ecrit) -> R<Option<String>> {
    let Some(cle) = brut.saisie.carte.salarie.as_ref() else { return Ok(None) };
    let mut fiche = droit::fiche::SalarieDuContrat::de(ctx.salarie(cle.as_ref()).map_err(|m| m.reclasser(Classe::Manque))?);
    if let Some(n) = brut.saisie.carte.etablissement.as_deref().filter(|n| !n.trim().is_empty()) {
        fiche.etablissement = Some(n.to_string());
    }
    Ok(Some(droit::bareme::etablissement_de(&ctx.entreprise, &fiche)?))
}

/// La DSN de `mois` est-elle a refaire ? Une DSN est celle d'un etablissement (`derniere` : sa ligne du journal, son NIC
/// en `detail.etablissement`, le principal s'il est tu) : seuls comptent les engagements qui en relevent
/// (`nic_de_l_engagement`). La paie de l'un d'eux a change depuis la version sur laquelle elle est partie
/// (`refaire` : `Moteur::a_refaire_du_mois`), ou ils ne sont plus ceux qu'elle a emportes (`versions`) : un engagement ajoute, ou retire.
pub(crate) fn dsn_a_refaire(ctx: &Contexte, mois: Mois, derniere: &envois::journal::Ligne, etats: &[engagement::etats::Etat], refaire: &mut dyn FnMut(&engagement::etats::Etat) -> R<Rc<Vec<ARefaire>>>) -> R<bool> {
    // Une ligne qui ne dit pas ce qu'elle a emporte ne se compare a rien.
    let Some(emportes) = derniere.versions() else { return Ok(false) };
    let nic = match derniere.etablissement() {
        Some(n) => n.to_string(),
        None => ctx.entreprise.nic.ecrite("nic")?.to_string(),
    };
    let mut siens: Vec<&engagement::etats::Etat> = Vec::new();
    for e in etats {
        if nic_de_l_engagement(ctx, e)?.as_deref() == Some(nic.as_str()) {
            siens.push(e);
        }
    }
    for e in &siens {
        if refaire(e)?.iter().any(|a| matches!(a, ARefaire::Dsn { mois: m, .. } if *m == mois)) {
            return Ok(true);
        }
    }
    // Les corrections de mois passes que la ligne porte ne sont pas des engagements de ce mois.
    let mut emportes: Vec<(String, String)> = emportes.iter().filter(|v| v.mois.is_none()).map(|v| (v.salarie.to_string(), v.numero.to_string())).collect();
    let mut presents: Vec<(String, String)> = siens
        .iter()
        .filter(|e| e.valide().is_some())
        .filter_map(|e| {
            let b = &*e.objet().brut;
            Some((b.saisie.carte.salarie.as_ref()?.to_string(), b.saisie.carte.numero.clone()?))
        })
        .collect();
    emportes.sort();
    presents.sort();
    Ok(emportes != presents)
}

/// UNE CORRECTION PORTEE, telle que l'empreinte de la saisie la retient (`Moteur::portees`) : le mois corrige, le
/// salarie, le contrat et la version. Elle s'ecrit en liste.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Portee(Mois, String, Option<NumeroContrat>, u64);

/// Ce dont l'empreinte d'une saisie qui porte des corrections se calcule (`Moteur::empreinte_saisie_avec`).
#[derive(Serialize)]
struct SaisieEtCorrections<'a> {
    saisie: &'a str,
    corrections: &'a [Portee],
}

/// L'ORDRE DES FICHIERS DEPOSABLES D'UN MOIS (`fichiers_deposables`), le meme partout : plus jamais la date de
/// modification du fichier, que deux machines -- ou deux rejeux -- ne donnent pas pareille.
/// - Les DSN : D'ABORD LA MENSUELLE (nature 01 ; un etablissement par fichier, le principal `DSN_<Mois>.dsn` avant les
///   `DSN_<Mois>_<NIC>.dsn`, par le nom) : c'est « la DSN du mois » que l'ecran et l'echeance attendent. D'un meme
///   etablissement, LA DERNIERE ECRITE D'ABORD : un annule-et-remplace s'ecrit a cote de la DSN deja
///   partie, sous `DSN_<Mois>_AR<ordre>.dsn` (`depart::fabriquer_dsn`), et c'est lui qui part ; il se reconnait a son numero
///   d'ordre (S20.G00.05.004, ou `_AR<n>` du nom), le plus grand d'abord. Rangees par le seul nom, la DSN deja deposee
///   (`DSN_Janvier2025.dsn`) passait avant son annule-et-remplace (`DSN_Janvier2025_AR2.dsn`) : la boite « Deposer » et
///   l'etape DSN montraient, testaient et auraient redepose l'ancienne. PUIS LES SIGNALEMENTS (arret, fin de contrat,
///   amorcage...), dans l'ordre de leur numero d'ordre (S20.G00.05.004) : unique et jamais remis a zero, il dit l'ordre ou
///   le moteur les a produits (`ordre_signalements`, cahier technique DSN 2026.1 p. 136), et chacun part a son tour.
/// - Les DPAE : la plus recemment produite d'abord, lue dans son nom (`DPAE_<Mois>_<AAAAMMJJ-HHMM>.txt` apres
///   `DPAE_<Mois>.txt` : l'ordre inverse des noms).
/// Une DSN qui ne se lit pas, ou dont le numero d'ordre n'est pas un nombre, se refuse.
pub(crate) fn ordre_de_depot(nom: &str, chemin: &Path) -> R<OrdreDeDepot> {
    use std::cmp::Reverse;
    if suffixe(nom).to_lowercase() != ".dsn" {
        return Ok((0, None, String::new(), Reverse(None), Reverse(nom.to_string())));
    }
    let apercu = declarations::dsn_lue::lire(&crate::depots::lire_octets(&chemin.to_string_lossy())?, nom)?;
    let lu = match apercu.ordre.as_deref().map(str::trim) {
        Some(t) => Some(t.parse::<i64>().map_err(|e| socle::echec::a_la_cle("Le numéro d'ordre d'une DSN écrite ne se lit pas.", "S20.G00.05.004", e).reclasser(Classe::Illisible))?),
        None => None,
    };
    if apercu.est_signalement() {
        return Ok((1, lu, nom.to_string(), Reverse(None), Reverse(String::new())));
    }
    let (etablissement, du_nom) = sans_annule_et_remplace(nom);
    Ok((0, None, etablissement, Reverse(lu.or(du_nom)), Reverse(String::new())))
}

/// La cle de rangement d'un fichier deposable (`ordre_de_depot`) : mensuelle ou signalement, l'ordre d'un signalement,
/// l'etablissement, l'ordre d'une mensuelle (le plus grand d'abord), le nom d'une DPAE (le plus grand d'abord).
pub(crate) type OrdreDeDepot = (u8, Option<i64>, String, std::cmp::Reverse<Option<i64>>, std::cmp::Reverse<String>);

/// `DSN_<Mois>[_<NIC>]_AR<n>.dsn` -> (`DSN_<Mois>[_<NIC>].dsn`, Some(n)) ; un autre nom reste tel quel.
fn sans_annule_et_remplace(nom: &str) -> (String, Option<i64>) {
    if let Some(corps) = nom.strip_suffix(".dsn").or_else(|| nom.strip_suffix(".DSN")) {
        if let Some(i) = corps.rfind("_AR") {
            if let Ok(n) = corps[i + 3..].parse::<i64>() {
                return (format!("{}.dsn", &corps[..i]), Some(n));
            }
        }
    }
    (nom.to_string(), None)
}

/// L'extension d'un nom de fichier.
fn suffixe(nom: &str) -> &str {
    match nom.rfind('.') {
        Some(i) if i > 0 && i + 1 < nom.len() => &nom[i..],
        _ => "",
    }
}

/// Le nom d'un fichier sans sa dernière extension.
fn radical(nom: &str) -> &str {
    match nom.rfind('.') {
        Some(i) if i > 0 && i + 1 < nom.len() => &nom[..i],
        _ => nom,
    }
}

/// Ce qu'une saisie chargee sans calcul porte : une « embauche » par engagement (le salarie, le spectacle, les dates,
/// la nature et le numero du contrat ; aucun montant).
pub(crate) struct SaisieMois {
    pub(crate) paies: Vec<Paie>,
    /// L'identite de chaque engagement de `paies` (`engagement::stock::identites`, sur tous les engagements du mois).
    pub(crate) identites: Vec<engagement::stock::Identite>,
    /// Ce qui manque a chaque engagement du mois (`None` : rien), dans l'ordre de `stock::objets_du_mois` :
    /// `paies` ne porte que ceux qui ne manquent de rien.
    pub(crate) manques: Vec<Option<socle::echec::Echec>>,
}

/// Les etats du journal et de la saisie, calcules une fois par processus : ce que le journal des envois dit de chaque
/// contrat (`envois::etats`), lu tel qu'il est.
pub(crate) struct Globales {
    pub(crate) dpae: HashMap<etats::Paire, etats::EtatEmbauche>,
    /// Le jour ou chaque contrat est revenu signe.
    pub(crate) recus: HashMap<etats::Paire, String>,
    pub(crate) renvoyes: HashSet<etats::Paire>,
    pub(crate) avenants: HashMap<etats::Paire, Vec<envois::journal::AvenantParti>>,
    /// Comment chaque contrat est parti la derniere fois : `imprime` ou `courriel`.
    pub(crate) par: HashMap<etats::Paire, &'static str>,
}

// ------------------------------------------------------------ l'etat d'un engagement

/// Les echeances d'un engagement : sa DPAE (L. 1221-10) et la remise de son contrat (L. 1242-13, ou la couche de la
/// convention), `droit::echeances`.
#[derive(Debug, Clone)]
struct Echeances {
    dpae: Echeance,
    contrat: Echeance,
}

/// Ce que la carte d'un engagement dit de son contrat : entier pour un engagement qui ne manque de rien, son premier
/// mois et s'il commence avant le logiciel pour celui auquel il manque un fait.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum ContratDit {
    Complet(ContratComplet),
    ACompleter(ContratACompleter),
}

/// `chevrons` : `routes::mois::chevrons_dits` ; `dossier` : celui d'un contrat signe ailleurs
/// (`signature::contrat_signe_dit`), lus tels qu'ils sont.
#[derive(Debug, Clone, Serialize)]
struct ContratComplet {
    premier_mois: bool,
    dernier_mois: bool,
    chevrons: super::mois::Chevrons,
    depuis: Mois,
    jusqu_a: Mois,
    repris: bool,
    ecrit: bool,
    ailleurs: bool,
    dossier: Option<ContratSigneDit>,
    anterieur: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ContratACompleter {
    premier_mois: bool,
    repris: bool,
    anterieur: bool,
}

impl ContratDit {
    fn premier_mois(&self) -> bool {
        match self {
            ContratDit::Complet(c) => c.premier_mois,
            ContratDit::ACompleter(c) => c.premier_mois,
        }
    }

    fn anterieur(&self) -> bool {
        match self {
            ContratDit::Complet(c) => c.anterieur,
            ContratDit::ACompleter(c) => c.anterieur,
        }
    }

    fn ecrit(&self) -> bool {
        matches!(self, ContratDit::Complet(c) if c.ecrit)
    }

    fn ailleurs(&self) -> bool {
        matches!(self, ContratDit::Complet(c) if c.ailleurs)
    }

    /// « novembre 2026 » : le premier mois du contrat ; rien pour un contrat a completer.
    fn depuis(&self) -> String {
        match self {
            ContratDit::Complet(c) => c.depuis.libelle(Graphie::Accentuee),
            ContratDit::ACompleter(_) => String::new(),
        }
    }

    fn dossier(&self) -> Option<&ContratSigneDit> {
        match self {
            ContratDit::Complet(c) => c.dossier.as_ref(),
            ContratDit::ACompleter(_) => None,
        }
    }
}

/// Une date d'embauche changee depuis la version sur laquelle la DPAE est partie : l'ancienne, si sa DPAE vaut, et la
/// phrase qui le dit (`regularisation::dpae_d_une_date_changee`).
#[derive(Debug, Clone, Serialize)]
struct DateChangee {
    avant: String,
    vaut: bool,
    phrase: String,
}

/// LA DPAE D'UN ENGAGEMENT : a declarer (la liste des DPAE a faire), partie (`fait` : quand, ou quand elle a ete
/// refusee), et ce que le journal des envois en dit (`envois::etats::etat_des_embauches`, lu tel qu'il est).
#[derive(Debug, Clone, Default)]
struct EtatDpae {
    date_changee: Option<DateChangee>,
    a_declarer: bool,
    fait: Option<String>,
    passee: bool,
    refusee: bool,
    anomalies: Option<Vec<String>>,
    dossier: Option<String>,
    deja_existante: bool,
}

/// Un CDI en cours, d'un mois d'apres son embauche : le mois de sa signature.
#[derive(Debug, Clone)]
pub(crate) struct CdiEnCours {
    signe_en: Mois,
}

/// LA PASTILLE CONTRAT D'UN ENGAGEMENT (`etat_signature`). `fait` : quand le contrat (ou son dernier avenant) est parti ;
/// `par`, `recu`, `avenant` : ce que le journal des envois en dit (`envois::etats`), lu tel qu'il est.
#[derive(Debug, Clone, Default)]
pub(crate) struct Signature {
    pub(crate) fait: Option<String>,
    pub(crate) par: Option<&'static str>,
    pub(crate) recu: Option<String>,
    pub(crate) caduc: bool,
    pub(crate) pourquoi: &'static str,
    pub(crate) avenant: Option<envois::journal::AvenantParti>,
    pub(crate) avenant_attendu: bool,
    cdi_en_cours: Option<CdiEnCours>,
}

impl Signature {
    /// Le contrat est parti.
    pub(crate) fn parti(&self) -> bool {
        self.fait.as_deref().is_some_and(|f| !f.is_empty())
    }
}

/// LA PASTILLE DOCUMENTS d'un engagement pour un mois (`etat_documents`).
#[derive(Debug, Clone, Default)]
struct EtatDocuments {
    fait: Option<String>,
    caduc: bool,
    pourquoi: &'static str,
}

impl EtatDocuments {
    fn parti(&self) -> bool {
        self.fait.as_deref().is_some_and(|f| !f.is_empty())
    }
}

/// Les etats des envois d'un engagement ; rien, pour un engagement auquel il manque un fait.
#[derive(Debug, Clone, Default)]
struct Etats {
    dpae: EtatDpae,
    signature: Signature,
    documents: EtatDocuments,
}

/// CE QUE LES ETAPES LISENT D'UN ENGAGEMENT DU MOIS : qui, quoi, quand, son contrat, ses echeances, son etablissement,
/// ce qu'il recoit (`pieces`), les etats de ses envois. `a_completer` : ce qui lui manque, s'il lui manque un fait (rien
/// ne se calcule sur lui ; son salarie, sa nature, son premier jour peuvent n'etre pas encore dits). `spectacle` : le
/// titre que sa fiche dit.
#[derive(Debug, Clone)]
pub(crate) struct Vue {
    salarie: Option<String>,
    nom: Option<String>,
    spectacle: Option<String>,
    /// Le libelle de sa nature de contrat, quand elle est dite.
    libelle_nature: Option<String>,
    numero: Option<NumeroContrat>,
    debut: Option<Jour>,
    fin: Option<Jour>,
    etablissement: Option<String>,
    echeances: Option<Echeances>,
    valide: Option<String>,
    recalcul: Option<String>,
    contrat: ContratDit,
    pieces: Vec<&'static str>,
    etats: Etats,
    a_completer: Option<String>,
}

impl Vue {
    fn valide(&self) -> bool {
        self.valide.as_deref().is_some_and(|v| !v.is_empty())
    }
}

/// LA LIGNE D'UN ENGAGEMENT (`etat_par_engagement`) : ce que les etapes lisent (`vue`), ses carres et le choix de sa
/// DPAE, son identite (`engagement::stock::identites`) et sa frise (`engagement::versions::frise`), lues telles qu'elles
/// sont.
#[derive(Debug, Clone)]
pub(crate) struct Ligne {
    vue: Vue,
    carres: Vec<Carre>,
    dpae_choix: Choix,
    versions: Option<engagement::versions::Frise>,
    engagement: engagement::stock::Identite,
}

impl Ligne {
    fn de(vue: Vue, mois: Mois, versions: Option<engagement::versions::Frise>, engagement: engagement::stock::Identite) -> R<Ligne> {
        Ok(Ligne { carres: carres(&vue, mois)?, dpae_choix: choix_dpae(&vue)?, vue, versions, engagement })
    }
}

/// La ligne d'un engagement telle que la page la lit : son identite, sa carte fermee (qui, quand, pour quoi, le contrat,
/// la validation, le recalcul, la frise), ses carres et le choix de sa DPAE, la phrase d'une date d'embauche changee, et
/// ce que la carte, la boite des DPAE et le detail d'une piece en disent (`periode_dit`, `valide_dit`, `validee_dit`,
/// `repris_dit`, `embauche_dit`, `numero_dit`). Le reste (echeances, pieces, etablissement, etats des envois) ne sert
/// qu'aux etapes du mois.
#[derive(Debug, Serialize)]
struct LigneDePage<'a> {
    engagement: &'a engagement::stock::Identite,
    salarie: Option<&'a str>,
    nom: Option<&'a str>,
    spectacle: Option<&'a str>,
    nature: Option<&'a str>,
    numero: Option<&'a str>,
    debut: Option<Jour>,
    fin: Option<Jour>,
    valide: Option<&'a str>,
    recalcul: Option<&'a str>,
    contrat: &'a ContratDit,
    versions: Option<&'a engagement::versions::Frise>,
    carres: &'a [Carre],
    dpae_choix: &'a Choix,
    etats: EtatsDePage<'a>,
    periode_dit: Option<String>,
    valide_dit: Option<String>,
    validee_dit: Option<String>,
    repris_dit: Option<&'static str>,
    embauche_dit: Option<String>,
    numero_dit: Option<String>,
}

#[derive(Debug, Serialize)]
struct EtatsDePage<'a> {
    dpae: DpaeDePage<'a>,
}

#[derive(Debug, Serialize)]
struct DpaeDePage<'a> {
    date_changee: Option<&'a DateChangee>,
}

impl<'a> LigneDePage<'a> {
    fn de(l: &'a Ligne) -> LigneDePage<'a> {
        let v = &l.vue;
        let valide = v.valide.as_deref().filter(|t| !t.is_empty()).map(socle::ecran::quand);
        let embauche = v.debut.map(|d| format!("embauche le {}", d.jj_mm_aaaa()));
        LigneDePage {
            periode_dit: v.debut.map(|d| periode_dite(d, v.fin)),
            valide_dit: valide.as_ref().map(|q| format!("le {}", q)),
            validee_dit: valide.as_ref().map(|q| format!("validée le {}", q)),
            repris_dit: matches!(&v.contrat, ContratDit::Complet(c) if c.repris).then_some("contrat repris"),
            embauche_dit: match (&v.spectacle, embauche) {
                (Some(s), Some(e)) => Some(format!("{}, {}", s, e)),
                (_, e) => e,
            },
            numero_dit: v.numero.as_ref().map(|n| format!("contrat n° {}", n)),
            engagement: &l.engagement,
            salarie: v.salarie.as_deref(),
            nom: v.nom.as_deref(),
            spectacle: v.spectacle.as_deref(),
            nature: v.libelle_nature.as_deref(),
            numero: v.numero.as_deref(),
            debut: v.debut,
            fin: v.fin,
            valide: v.valide.as_deref(),
            recalcul: v.recalcul.as_deref(),
            contrat: &v.contrat,
            versions: l.versions.as_ref(),
            carres: &l.carres,
            dpae_choix: &l.dpae_choix,
            etats: EtatsDePage { dpae: DpaeDePage { date_changee: v.etats.dpae.date_changee.as_ref() } },
        }
    }
}

/// UN CARRE DE LA LIGNE D'UN ENGAGEMENT (`carres`) : son nom, son etat, sa phrase (l'infobulle), le geste qu'ouvre un
/// clic, ce que montre son detail (`pieces`, DPAE et Contrat), sa couleur, ce qui reste en un mot, le lien vers ce qui est
/// parti, et le titre de sa boite. Un carre qui attend qu'on complete ou valide l'engagement n'a ni detail, ni coupure, ni
/// lien.
#[derive(Debug, Clone, Serialize)]
struct Carre {
    cle: QuelleEtape,
    nom: String,
    etat: EtatCase,
    phrase: String,
    action: Option<Geste>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pieces: Option<Pieces>,
    couleur: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    coupee: Option<bool>,
    reste: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    lien: Option<Option<&'static str>>,
    titre: &'static str,
}

/// Le geste d'un carre : une route du catalogue, ou le dossier d'un contrat signe hors du logiciel (`geste_du_dossier`).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum Geste {
    Route(Action),
    Dossier(GesteDuDossier),
}

/// Le dossier d'un contrat signe hors du logiciel : la fenetre l'ouvre et le cree ; `chemin` et `phrase` sont ceux que
/// `signature::contrat_signe_dit` rend, lus tels qu'ils sont.
#[derive(Debug, Clone, Serialize)]
struct GesteDuDossier {
    libelle: &'static str,
    chemin: String,
    phrase: String,
}

/// Ce que montre le detail d'un carre (`pieces_du_contrat`).
#[derive(Debug, Clone, Serialize)]
struct Pieces {
    vide: Option<String>,
    encart: Option<EncartDePiece>,
    gestes: Vec<GesteDePiece>,
}

#[derive(Debug, Clone, Serialize)]
struct EncartDePiece {
    titre: &'static str,
    texte: String,
}

#[derive(Debug, Clone, Serialize)]
struct GesteDePiece {
    libelle: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    titre: Option<&'static str>,
    action: Option<Geste>,
    principal: bool,
}

/// CE QUE LA BOITE DES DPAE DIT D'UN ENGAGEMENT (`choix_dpae`).
#[derive(Debug, Clone, Serialize)]
struct Choix {
    cochable: bool,
    cochee: bool,
    etat: EtatCase,
    couleur: &'static str,
    phrase: String,
}

/// UNE DPAE A FAIRE (`embauches_a_declarer`) : le mois, le contrat, qui, pour quoi, ses bornes, l'empreinte de ce qu'elle
/// declare, et pourquoi quand une date d'embauche a change. `spectacle` : le titre que sa fiche dit (aucun pour un
/// salarie au mois).
#[derive(Debug, Clone, Serialize)]
pub struct DpaeDue {
    pub mois: Mois,
    pub salarie: String,
    pub numero: NumeroContrat,
    pub nom: String,
    pub spectacle: Option<String>,
    pub debut: String,
    pub fin: Option<String>,
    pub empreinte: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pourquoi: Option<String>,
}

/// UN FICHIER DEPOSABLE D'UN MOIS (`fichiers_deposables`) : son chemin.
#[derive(Debug, Clone)]
pub(crate) struct Deposable {
    pub(crate) chemin: String,
}

/// Les fichiers deposables d'un mois, par acte, le dernier ecrit en tete.
#[derive(Debug, Clone, Default)]
pub(crate) struct Deposables {
    pub(crate) dsn: Vec<Deposable>,
    dpae: Vec<Deposable>,
}

impl Deposables {
    fn de(&self, quoi: &str) -> &[Deposable] {
        if quoi == DSN { &self.dsn } else { &self.dpae }
    }
}

/// OU EN EST LE MOIS VIS-A-VIS DE L'ANNULE-ET-REMPLACE (`etat_remplacement`) : deposee, son dernier ordre et le prochain,
/// la limite et si elle est passee, si la paie a change depuis (`saisie_modifiee`, rien si rien ne se compare), si un
/// annule-et-remplace est a envoyer, si la DSN suivante porte la correction, et la phrase qui le dit.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Remplacement {
    mois: Mois,
    pub(crate) deposee: bool,
    pub(crate) dernier_ordre: Option<i64>,
    prochain_ordre: i64,
    pub(crate) limite: Jour,
    admis: bool,
    saisie_modifiee: Option<bool>,
    pub(crate) a_envoyer: bool,
    pub(crate) phrase: String,
    corrigee_par_la_suivante: bool,
}

/// Tout ce qu'une route lit, lu une fois.
pub struct Moteur<'a> {
    ctx: &'a Contexte,
    globales: Option<Rc<Globales>>,
    fichiers: HashMap<Mois, Rc<Deposables>>,
    /// LES ENGAGEMENTS TELS QU'ILS SONT ECRITS, sans recalcul d'eux-memes (`engagement::etats::tels_qu_ecrits`) : ce que
    /// lit `mois_a_ouvrir`, qui regarde chaque mois passe a l'ouverture et ne calcule rien.
    ecrit: bool,
    /// CE QU'IL A LU D'UN MOIS, une fois pour sa vie (une route, l'etat garde d'un mois) : la saisie chargee
    /// (`charger_saisie`), ce qui est a refaire de chaque engagement valide (`a_refaire_du_mois`, par mois, salarie et
    /// numero) et l'etat de chaque engagement (`etat_par_engagement`).
    saisies: HashMap<Mois, Rc<SaisieMois>>,
    refaire: HashMap<(Mois, String, String), Rc<Vec<ARefaire>>>,
    lignes: HashMap<Mois, Rc<Vec<Ligne>>>,
}

impl<'a> Moteur<'a> {
    pub fn nouveau(ctx: &'a Contexte) -> Moteur<'a> {
        Moteur { ctx, globales: None, fichiers: HashMap::new(), ecrit: false, saisies: HashMap::new(), refaire: HashMap::new(), lignes: HashMap::new() }
    }

    /// Ce qui est parti (`envois::journal::lu`, tire une fois par contenu du journal) : ce que le moteur passe a
    /// l'engagement, qui ne lit pas le journal.
    pub(crate) fn departs(&self) -> R<engagement::departs::Departs> {
        envois::journal::lu(&self.ctx.racine).map(|j| j.departs)
    }

    /// Les engagements de `mois`, dans l'ordre de l'ecran : recalcules d'eux-memes s'il le faut (`du_mois`), ou tels
    /// qu'ils sont ecrits (`ecrit`).
    pub(crate) fn etats_du_mois(&self, mois: Mois) -> R<Vec<engagement::etats::Etat>> {
        let departs = self.departs()?;
        if self.ecrit {
            return engagement::etats::tels_qu_ecrits(self.ctx, mois, &departs);
        }
        engagement::etats::du_mois(self.ctx, mois, &departs)
    }

    fn racine(&self) -> &Path {
        &self.ctx.racine
    }

    /// Les lignes de `journal/depots.jsonl` (`envois::journal::lu`, la lecture qui donne aussi `departs`).
    pub(crate) fn lignes(&self) -> R<std::sync::Arc<Vec<envois::journal::Ligne>>> {
        envois::journal::lu(self.racine()).map(|j| j.lignes)
    }

    /// Le NIC de l'etablissement principal, s'il est dit : une compagnie neuve ne l'a pas encore, et n'a que lui (la DSN,
    /// elle, l'exige : « Pour demarrer » le demande).
    fn nic(&self) -> Option<String> {
        self.ctx.entreprise.nic_principal().map(str::to_string)
    }

    // ------------------------------------------------------ la saisie du mois

    /// La saisie d'un mois, chargee sans calculer la paie : ses engagements qui ne manquent de rien
    /// (`mois::manques_de_la_saisie`), et ce qui manque a chacun de ses engagements, dans son ordre.
    fn charger_saisie(&mut self, mois: Mois) -> R<Rc<SaisieMois>> {
        if let Some(s) = self.saisies.get(&mois) {
            return Ok(s.clone());
        }
        let s = Rc::new(self.saisie_chargee(mois)?);
        self.saisies.insert(mois, s.clone());
        Ok(s)
    }

    /// CE QUI EST A REFAIRE de l'engagement `e` de `mois` (`a_refaire_de`), compare une fois.
    fn a_refaire_du_mois(&mut self, mois: Mois, e: &engagement::etats::Etat, departs: &engagement::departs::Departs) -> R<Rc<Vec<ARefaire>>> {
        let carte = &e.objet().brut.saisie.carte;
        // Rien n'est parti d'un engagement en saisie : rien n'est a refaire.
        let (Some(_), Some(salarie), Some(numero)) = (e.valide(), carte.salarie.as_ref(), carte.numero.as_deref().filter(|n| !n.is_empty())) else {
            return Ok(Rc::new(Vec::new()));
        };
        let cle = (mois, salarie.to_string(), numero.to_string());
        if let Some(r) = self.refaire.get(&cle) {
            return Ok(r.clone());
        }
        let r = Rc::new(a_refaire_de(self.ctx, e, departs)?);
        self.refaire.insert(cle, r.clone());
        Ok(r)
    }

    fn saisie_chargee(&mut self, lu: Mois) -> R<SaisieMois> {
        let mut data = engagement::vue_requise(self.racine(), lu).map_err(classer_saisie)?;
        let manques = super::mois::manques_de_la_saisie(self.ctx, &data);
        let objets = engagement::stock::objets_du_mois(self.racine(), lu)?;
        let mut identites = engagement::stock::identites(&objets);
        let mut m = manques.iter();
        identites.retain(|_| m.next().is_some_and(Option::is_none));
        let mut m = manques.iter();
        data.engagements.retain(|_| m.next().is_some_and(Option::is_none));
        let mut paies = engagement::lue_sans_paie(self.ctx, &data).map_err(classer_saisie)?;
        engagement::numeros::numeroter_les_contrats(self.ctx, &mut paies, &self.departs()?)?;
        Ok(SaisieMois { paies, identites, manques })
    }

    // --------------------------------------------- ce que le journal et la saisie disent

    pub(crate) fn globales(&mut self) -> R<Rc<Globales>> {
        if let Some(g) = &self.globales {
            return Ok(g.clone());
        }
        let lignes = self.lignes()?;
        let dpae = etats::etat_des_embauches(&lignes);
        let recus = etats::contrats_recus(&lignes);
        let renvoyes = etats::renvoyes_depuis_la_signature(&lignes);
        let avenants = etats::avenants_par_contrat(&lignes);
        let par = etats::contrats_partis_par(&lignes);
        let g = Rc::new(Globales { dpae, recus, renvoyes, avenants, par });
        self.globales = Some(g.clone());
        Ok(g)
    }

    // ------------------------------------------------------------ etat_par_engagement

    /// `etat_par_engagement` : ou en est CHAQUE engagement du mois (ce que les etapes lisent ; la route n'en rend que ce
    /// que la page lit, `LigneDePage`).
    pub(crate) fn etat_par_engagement(&mut self, le_mois: Mois) -> R<Rc<Vec<Ligne>>> {
        if let Some(l) = self.lignes.get(&le_mois) {
            return Ok(l.clone());
        }
        let l = Rc::new(self.lignes_du_mois(le_mois)?);
        self.lignes.insert(le_mois, l.clone());
        Ok(l)
    }

    fn lignes_du_mois(&mut self, le_mois: Mois) -> R<Vec<Ligne>> {
        let mois = le_mois;
        // UN MOIS SANS ENGAGEMENT N'EST PAS UNE ERREUR.
        if !engagement::stock::a_une_saisie(self.racine(), le_mois)? {
            return Ok(Vec::new());
        }
        let s = self.charger_saisie(mois)?;
        // Valide, ou parti : la date de la validation (`engagement::etats`), engagement par engagement ; chacun retrouve son
        // embauche par son identite (`engagement::stock::identites`).
        let etats_eng = self.etats_du_mois(le_mois)?;
        let ids = engagement::stock::identites(&etats_eng.iter().map(|e| e.objet().clone()).collect::<Vec<_>>());
        let g = self.globales()?;
        let departs = self.departs()?;
        // Ce qui est a refaire de chacun, compare une fois : la liste des DPAE, la ligne et la DSN du mois le lisent.
        for e in &etats_eng {
            self.a_refaire_du_mois(mois, e, &departs)?;
        }
        // LES DPAE A DEPOSER DU MOIS : la seule liste (`embauches_a_declarer`), que lisent aussi les rappels et le depot.
        let a_declarer: HashSet<Paire> = self.embauches_a_declarer(Some(le_mois))?.into_iter().map(|d| (d.salarie, d.numero.to_string())).collect();

        // Chaque ligne porte `engagement` : l'identite par laquelle sa carte le designe. Un engagement auquel il manque un
        // fait a sa ligne a lui (`ligne_a_completer`) ; les autres retrouvent leur paie parmi ceux qui ne manquent de rien.
        let mut out = Vec::new();
        for (j, (e, id)) in etats_eng.iter().zip(&ids).enumerate() {
            if let Some(Some(manque)) = s.manques.get(j) {
                let vue = self.ligne_a_completer(mois, e.objet(), &manque.phrase)?;
                out.push(Ligne::de(vue, le_mois, None, id.clone())?);
                continue;
            }
            let Some(k) = s.identites.iter().position(|x| x == id) else { continue };
            let p = &s.paies[k];
            let cle = p.salarie.id.to_string();
            let debut = p.engagement.debut.iso();
            // CE QUI EST A REFAIRE : le present compare a la version sur laquelle chaque envoi est parti.
            let refaire = self.a_refaire_du_mois(mois, e, &departs)?;
            let sal = &p.salarie;
            let nat = p.engagement.nat()?;
            let spectacle = match &p.spectacle {
                Some(s) => match s.titre.dite() {
                    Some(t) => Some(t.clone()),
                    None => return Err(socle::fichiers::absent("titre").reclasser(Classe::Manque)),
                },
                None => None,
            };
            let mois_du_contrat = p.engagement.mois_du_contrat();
            let (Some(&depuis), Some(&jusqu_a)) = (mois_du_contrat.first(), mois_du_contrat.last()) else {
                return Err(erreur(Classe::Manque, socle::fichiers::trop_courte("les mois du contrat")));
            };
            let premier_mois = p.engagement.premier_mois();
            let dernier_mois = p.engagement.dernier_mois();
            // Son etablissement (NIC) : la DSN qui le porte.
            let etablissement = nic_de_l_engagement(self.ctx, e)?;
            // LES ECHEANCES de l'engagement : sa DPAE (L. 1221-10) et la remise de son contrat (L. 1242-13, ou la couche
            // de la convention), `droit::echeances`. Les documents n'en ont pas : chaque compagnie les envoie quand elle
            // veut, souvent apres la DSN qui fait remonter l'AER. Un mois passe lu a l'ouverture (`ecrit`) ne les demande pas.
            let echeances = if self.ecrit {
                None
            } else {
                let code_emploi = p.engagement.code_emploi_ft.as_deref().or(sal.code_emploi_ft.portee());
                let ent = &self.ctx.entreprise;
                let remise = droit::echeances::remise_du_contrat(&p.engagement.debut, ent.idcc.portee(), code_emploi, ent.feries_chomes())?;
                Some(Echeances { dpae: droit::echeances::dpae(&p.engagement.debut), contrat: remise })
            };
            // RECALCULEE D'ELLE-MEME (`engagement::etats::du_mois`) : pourquoi, et quand.
            let recalcul = match e.valide() {
                Some(v) => v.recalcul(le_mois)?,
                None => None,
            };
            let recalcul = recalcul.as_ref().and_then(engagement::etats::phrase_du_recalcul);
            // REPRIS D'UN ANCIEN LOGICIEL : la carte le dit (`engagement::importe::vient_d_un_ancien_logiciel`).
            let repris = engagement::importe::vient_d_un_ancien_logiciel(&self.ctx.racine, &p.engagement.salarie, &p.engagement.debut.iso(), p.engagement.numero.as_deref())?;
            // SIGNE AILLEURS : son contrat ecrit ne se fait pas signer ici, ses avenants si (`engagement::signe_ailleurs`).
            let ailleurs = engagement::signe_ailleurs(&self.ctx.racine, &p.engagement)?;
            // ECRIT : le contrat de cet engagement se fait signer ici (`engagement::contrat_a_signer`).
            let contrat = ContratDit::Complet(ContratComplet {
                premier_mois,
                dernier_mois,
                chevrons: super::mois::chevrons_dits(&p.engagement)?,
                depuis,
                jusqu_a,
                repris,
                ecrit: engagement::contrat_a_signer(&self.ctx.racine, &p.engagement)?,
                ailleurs,
                dossier: ailleurs.then(|| crate::gestes::signature::contrat_signe_dit(self.ctx, &cle, &p.engagement.debut.iso())),
                anterieur: engagement::stock::commence_avant_le_logiciel(&self.ctx.racine, &p.engagement.debut.iso(), le_mois)?,
            });
            // CE QUE CE SALARIE RECOIT POUR CETTE PAIE : le bulletin s'il y a une paie, le certificat Conges Spectacles
            // du contrat declare a la caisse qui finit (`crate::dossier_salarie::certificats_dus`), l'AER Cinema
            // Spectacle qui lui est due (`aer::due`). L'etape « Documents » du mois les nomme d'apres ceci, et rien d'autre.
            let mut pieces: Vec<&'static str> = Vec::new();
            if !p.engagement.sans_paie() {
                pieces.push("bulletin");
            }
            if dernier_mois && p.declaree_a_la_caisse_des_conges()? {
                pieces.push("certificat_cs");
            }
            if documents::aer::due(p)? {
                pieces.push("aer");
            }
            // LA DPAE DE CET ENGAGEMENT : partie (`departs`), et ce qui en est revenu (la ligne qui la porte, par le numero du
            // contrat). Refusee au bilan, elle n'est pas partie ; la ligne dit quand elle a ete refusee.
            let numero = p.engagement.numero.as_deref();
            // Le contrat tel que le journal le designe (`envois::etats::Paire`).
            let k: etats::Paire = (cle.clone(), numero.map(str::to_string));
            let mut dpae_v = g.dpae.get(&k);
            let mut partie_le = match numero {
                Some(n) => departs.de(&cle, n).iter().filter(|d| d.envoi == engagement::departs::Envoi::Dpae).map(|d| d.le.clone()).max(),
                None => None,
            };
            // UNE DATE D'EMBAUCHE CHANGEE depuis la version sur laquelle la DPAE est partie (`ARefaire::Dpae`) : la DPAE de
            // l'ancienne date vaut-elle (`crate::regularisation::dpae_d_une_date_changee`) ?
            let mut date_changee = None;
            if let Some(avant) = debut_declare(&refaire) {
                let (vaut, phrase) = crate::regularisation::dpae_d_une_date_changee(&avant, &debut)?;
                if !vaut {
                    dpae_v = None;
                    partie_le = None;
                }
                date_changee = Some(DateChangee { avant, vaut, phrase });
            }
            let refusee = dpae_v.is_some_and(|d| d.refusee);
            let fait = match partie_le {
                Some(le) => Some(le),
                None if refusee => dpae_v.map(|d| d.quand.clone()),
                None => None,
            };
            let dpae = EtatDpae {
                date_changee,
                a_declarer: numero.is_some_and(|n| a_declarer.contains(&(cle.clone(), n.to_string()))),
                fait,
                passee: dpae_v.is_some_and(|d| d.passee),
                refusee,
                anomalies: dpae_v.map(|d| d.anomalies.clone()),
                dossier: dpae_v.and_then(|d| d.dossier.clone()),
                deja_existante: dpae_v.is_some_and(|d| d.deja_existante),
            };
            let avenant = refaire.iter().any(|a| matches!(a, ARefaire::Avenant { .. }));
            let mut signature = etat_signature(
                contrat_parti_le(&departs, &cle, p.engagement.numero.as_deref()),
                g.par.get(&k).copied(),
                g.recus.get(&k),
                g.renvoyes.contains(&k),
                g.avenants.get(&k),
                avenant,
                ailleurs,
            );
            // UN CDI EN COURS N'A PAS D'ETAT DE CONTRAT MENSUEL.
            if nat.mensuelle() && nat.duree_indeterminee && !premier_mois {
                let recu = g.recus.get(&k);
                let signe_en = match recu {
                    Some(r) => Mois::lire(&r.chars().take(7).collect::<String>())?,
                    None => depuis,
                };
                signature.cdi_en_cours = Some(CdiEnCours { signe_en });
            }
            // LES DOCUMENTS DE CET ENGAGEMENT, pour ce mois : son dernier envoi (`departs`), et son bulletin a refaire.
            let documents_le = p.engagement.numero.as_deref().and_then(|n| {
                departs.de(&cle, n).iter().filter(|d| d.envoi == engagement::departs::Envoi::Documents && d.mois == Some(le_mois) && d.par.is_none()).map(|d| d.le.clone()).max()
            });
            let bulletin = refaire.iter().any(|a| matches!(a, ARefaire::Bulletin { mois: m, .. } if *m == mois));
            let documents = etat_documents(documents_le, bulletin);
            let vue = Vue {
                salarie: Some(cle.clone()),
                nom: Some(super::nom_a_l_ecran(sal, &cle.to_string())),
                spectacle,
                libelle_nature: Some(nat.libelle.clone()),
                numero: p.engagement.numero.clone(),
                debut: Some(p.engagement.debut),
                // Un CDI n'a pas de fin : rien, comme `Engagement::canon`.
                fin: p.engagement.fin,
                etablissement,
                echeances,
                valide: e.valide().map(|v| v.le().to_string()),
                recalcul,
                contrat,
                pieces,
                etats: Etats { dpae, signature, documents },
                a_completer: None,
            };
            // LA FRISE : un engagement modifie apres un envoi dit quand, et ce qui a change de version en version.
            let frise = match e.valide() {
                Some(v) => engagement::versions::frise(self.ctx, v, &departs, le_mois)?,
                None => None,
            };
            out.push(Ligne::de(vue, le_mois, frise, id.clone())?);
        }
        Ok(out)
    }

    /// LA LIGNE D'UN ENGAGEMENT JAMAIS VALIDE AUQUEL IL MANQUE UN FAIT : rien ne se calcule sur lui, rien n'est parti ;
    /// il dit qui, quand, et ce qui lui manque. Ses carres disent « à compléter » (`Point::de`). L'engagement se lit tel
    /// que sa saisie l'ecrit (`Objet::vue`).
    fn ligne_a_completer(&self, lu: Mois, o: &engagement::stock::Objet, manque: &str) -> R<Vue> {
        let Some(e) = o.vue(lu)? else {
            return Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : un engagement du mois ne se lit pas dans ce mois."));
        };
        let cle = e.salarie.as_ref().map(ToString::to_string);
        // Un salarie qu'aucune fiche ne porte se nomme par sa cle.
        let nom = cle.as_deref().map(|c| match self.ctx.salaries.get(c) {
            Some(f) => super::nom_a_l_ecran(f, c),
            None => c.to_string(),
        });
        let spectacle = match e.spectacle.as_deref().filter(|s| !s.is_empty()) {
            Some(s) => match self.ctx.spectacle(s)?.titre.dite() {
                Some(t) => Some(t.clone()),
                None => Some(s.to_string()),
            },
            None => None,
        };
        let libelle_nature = match e.nature {
            Some(n) => Some(n.proprietes()?.libelle.clone()),
            None => None,
        };
        let premier_mois = o.premier_mois() == lu;
        let debut = match e.debut_lu() {
            d if d.is_empty() => None,
            d => Some(Jour::lire(&d)?),
        };
        let anterieur = if let Some(d) = debut { engagement::stock::commence_avant_le_logiciel(&self.ctx.racine, &d.iso(), lu)? } else { false };
        let numero = match e.numero_dit() {
            Some(n) => Some(NumeroContrat::lire(n)?),
            None => None,
        };
        let fin = match e.fin.as_deref().filter(|f| !f.is_empty()) {
            Some(f) => Some(Jour::lire(f)?),
            None => None,
        };
        Ok(Vue {
            salarie: cle,
            nom,
            spectacle,
            libelle_nature,
            numero,
            debut,
            fin,
            etablissement: None,
            echeances: None,
            valide: None,
            recalcul: None,
            contrat: ContratDit::ACompleter(ContratACompleter { premier_mois, repris: false, anterieur }),
            pieces: Vec::new(),
            etats: Etats::default(),
            a_completer: Some(manque.to_string()),
        })
    }

    // ------------------------------------------------------ les fichiers deposables

    /// `fichiers_deposables` : les fichiers d'un mois qu'on peut deposer, par
    /// acte, le dernier ecrit en tete.
    pub(crate) fn fichiers_deposables(&mut self, mois: Mois) -> R<Rc<Deposables>> {
        if let Some(v) = self.fichiers.get(&mois) {
            return Ok(v.clone());
        }
        let dossier = self.racine().join("sorties").join(mois.to_string());
        let rendu = if !dossier.is_dir() {
            Deposables::default()
        } else {
            let mut tous: Vec<(String, PathBuf)> = Vec::new();
            let rd = std::fs::read_dir(&dossier).map_err(|e| socle::echec::du_disque(&dossier, &e).reclasser(Classe::Disque))?;
            for e in rd {
                let e = e.map_err(|x| socle::echec::du_disque(&dossier, &x).reclasser(Classe::Disque))?;
                let chemin = e.path();
                if chemin.is_file() {
                    tous.push((e.file_name().to_string_lossy().into_owned(), chemin));
                }
            }
            // L'ORDRE NE DEPEND PAS DE L'HEURE D'UN FICHIER (deux rejeux, deux machines, le meme ordre) : il se lit
            // dans ce que le moteur sait de chacun (`ordre_de_depot`).
            let ranger = |v: Vec<&(String, PathBuf)>| -> R<Vec<Deposable>> {
                let mut ranges = v.into_iter().map(|f| Ok((ordre_de_depot(&f.0, &f.1)?, f))).collect::<R<Vec<_>>>()?;
                ranges.sort_by(|a, b| a.0.cmp(&b.0));
                Ok(ranges.into_iter().map(|(_, (_, chemin))| Deposable { chemin: chemin.to_string_lossy().into_owned() }).collect())
            };
            let dsn: Vec<_> = tous.iter().filter(|f| suffixe(&f.0).to_lowercase() == ".dsn").collect();
            let dpae: Vec<_> = tous
                .iter()
                .filter(|f| f.0.starts_with("DPAE_") && suffixe(&f.0).to_lowercase() == ".txt" && !radical(&f.0).ends_with("_recapitulatif"))
                .collect();
            Deposables { dsn: ranger(dsn)?, dpae: ranger(dpae)? }
        };
        let rendu = Rc::new(rendu);
        self.fichiers.insert(mois, rendu.clone());
        Ok(rendu)
    }

    // ----------------------------------------------------- l'annule-et-remplace

    /// `empreinte_saisie` : ce que la DSN du mois declare, resume en 16 caracteres -- les paies validees du mois
    /// (`engagement::etats::empreinte_des_paies_validees` : un brouillon n'y est pas, la DSN ne le declare pas), et la
    /// version de chaque engagement d'un mois passe dont elle porte la correction
    /// (`produire_regularisations::regularisations_pour`) : une correction validee apres l'ecriture du fichier le rend
    /// perime comme une paie du mois qui change. `None` si rien ne se lit ni ne se corrige.
    pub(crate) fn empreinte_saisie(&self, mois: Mois) -> R<Option<String>> {
        let departs = self.departs()?;
        self.empreinte_saisie_avec(mois, || declarations::produire_regularisations::regularisations_pour(self.ctx, mois, &departs).map(|r| Self::portees(&r)))
    }

    /// `empreinte_saisie`, sur les corrections portees que `portees` lit (`Moteur::portees`) : la fabrication de la DSN,
    /// qui les a deja lues, les passe.
    pub(crate) fn empreinte_saisie_avec(&self, mois: Mois, portees: impl FnOnce() -> R<Vec<Portee>>) -> R<Option<String>> {
        let vue = engagement::etats::empreinte_des_paies_validees(self.ctx, mois, None)?;
        let portees = portees()?;
        if portees.is_empty() {
            return Ok(Some(vue.chars().take(16).collect()));
        }
        let ecrite = socle::ecriture::json_trie(&SaisieEtCorrections { saisie: &vue, corrections: &portees });
        Ok(Some(sha256_hex(ecrite.as_bytes())[..16].to_string()))
    }

    /// Ce que l'empreinte de la saisie retient de chaque correction portee (`Portee`).
    pub(crate) fn portees(regs: &[declarations::produire_regularisations::Reg]) -> Vec<Portee> {
        regs.iter().map(|r| Portee(r.mois, r.apres.salarie.id.to_string(), r.apres.engagement.numero.clone(), r.version)).collect()
    }

    /// `limite` : le dernier jour ou un annule-et-remplace est admis, jusqu'a minuit : la veille de l'echeance de depot
    /// (`echeance_dsn::Echeance::veille`, reportee comme elle).
    pub(crate) fn limite(&self, mois: Mois) -> R<Jour> {
        let e = declarations::echeance_dsn::de_la_compagnie(self.ctx, mois).map_err(|m| m.reclasser(Classe::Illisible))?;
        Ok(e.veille())
    }

    /// Le refus d'un annule-et-remplace du mois apres sa `limite` (cahier technique DSN §1.4.2) ; `Ok` jusqu'a elle.
    pub(crate) fn refuser_si_hors_delai(&self, mois: Mois) -> R<()> {
        let limite = self.limite(mois)?;
        if self.ctx.aujourdhui <= limite {
            return Ok(());
        }
        Err(erreur(
            Classe::AnnuleRemplaceHorsDelai,
            format!(
                "La DSN de {} est deja partie, et l'annule-et-remplace n'est plus admis depuis le {} a minuit (veille de l'echeance, cahier technique DSN §1.4.2). Ce qui a change dans ce mois part dans la DSN d'un mois suivant, en correction. Rien ne part.",
                mois,
                limite.jj_mm_aaaa()
            ),
        ))
    }

    /// Le plus haut ordre porte par ces depots reels du mois (`depots_reels`) ; `None` s'il n'y en a pas.
    pub(crate) fn dernier_ordre(&self, mois: Mois, faits: &[envois::journal::Ligne]) -> R<Option<i64>> {
        Ok(faits.iter().map(|l| self.ordre_de_la_ligne(mois, l)).collect::<R<Vec<i64>>>()?.into_iter().max())
    }

    /// Le rang d'un depot reel du journal : l'ordre que sa ligne porte (S20.G00.05.004 du fichier parti, noté au depot).
    pub(crate) fn ordre_de_la_ligne(&self, mois: Mois, l: &envois::journal::Ligne) -> R<i64> {
        let refus = || Echec {
            a_part: Some(socle::echec::Lieu { fichier: Some(envois::journal::DEPOTS.to_string()), cle: Some("ordre".to_string()), ..Default::default() }.into()),
            ..erreur(
                Classe::Illisible,
                format!("Un dépôt réel de la DSN de {} ne porte pas son ordre (S20.G00.05.004) dans le journal des envois : le rang d'un annule-et-remplace ne se devine pas ; le signaler au développeur.", mois),
            )
        };
        let Some(t) = l.dsn().and_then(|e| e.detail.ordre.as_deref()) else { return Err(refus()) };
        t.trim().parse::<i64>().map_err(|_| refus())
    }

    /// `depots_reels` : les depots REELS de DSN mensuelle du mois que la
    /// plate-forme a pris et qu'aucun bilan n'a refuses (les lignes du journal des envois, telles qu'elles sont).
    pub(crate) fn depots_reels(&mut self, mois: Mois, nic: Option<&str>) -> R<Vec<envois::journal::Ligne>> {
        let lignes = self.lignes()?;
        let reels = envois::journal::filtrer(&lignes, Some(envois::journal::Acte::Dsn), Some(mois), true);
        let mut out = Vec::new();
        for l in etats::de_l_etablissement(reels, nic, self.nic().as_deref()) {
            if !l.accepte() || l.signalement() || etats::refuse_par_un_retour(l) {
                continue;
            }
            // Le type de la declaration (S20.G00.05.002), que le depot note : une ligne qui ne le dit pas se refuse.
            let Some(t) = l.dsn().and_then(|e| e.detail.type_declaration.as_deref()) else {
                return Err(Echec {
                    a_part: Some(socle::echec::Lieu { fichier: Some(envois::journal::DEPOTS.to_string()), cle: Some("type_declaration".to_string()), ..Default::default() }.into()),
                    ..erreur(Classe::Illisible, format!("Un dépôt réel de la DSN de {} ne dit pas son type (S20.G00.05.002) dans le journal des envois ; le signaler au développeur.", mois))
                });
            };
            if matches!(t, "01" | "02" | "03" | "05") {
                out.push(l.clone());
            }
        }
        Ok(out)
    }

    /// Ou en est le mois vis-a-vis de
    /// l'annule-et-remplace.
    /// `nic` : l'etablissement dont on parle (`Null` : le principal).
    pub(crate) fn etat_remplacement(&mut self, le_mois: Mois, nic: Option<&str>) -> R<Remplacement> {
        let faits = self.depots_reels(le_mois, nic)?;
        let lim = self.limite(le_mois)?;
        let mois = le_mois;
        let admis = self.ctx.aujourdhui <= lim;
        let mut out = Remplacement {
            mois: le_mois,
            deposee: !faits.is_empty(),
            dernier_ordre: None,
            prochain_ordre: 1,
            limite: lim,
            admis,
            saisie_modifiee: None,
            a_envoyer: false,
            phrase: String::new(),
            // Apres la limite : la DSN suivante corrige chaque contrat dont la paie validee a change.
            corrigee_par_la_suivante: false,
        };
        let (Some(derniere), Some(dernier)) = (faits.last(), self.dernier_ordre(le_mois, &faits)?) else {
            return Ok(out);
        };
        out.dernier_ordre = Some(dernier);
        out.prochain_ordre = dernier + 1;
        // Ce que la DSN a emporte, compare au present des engagements du mois (`dsn_a_refaire`).
        let mut modifiee = false;
        if derniere.versions().is_some() {
            let etats_eng = self.etats_du_mois(le_mois)?;
            let (ctx, departs) = (self.ctx, self.departs()?);
            modifiee = dsn_a_refaire(ctx, mois, derniere, &etats_eng, &mut |e| self.a_refaire_du_mois(mois, e, &departs))?;
            out.saisie_modifiee = Some(modifiee);
        }
        // Apres la limite : les contrats dont la paie validee n'est plus celle que la DSN a declaree ; la DSN suivante
        // porte leur correction d'elle-meme (`engagement::etats::correction_due`, comme
        // `produire_regularisations::corrections_dues`), ceux de cet etablissement seuls.
        let corriges = if admis {
            0
        } else {
            let departs = self.departs()?;
            let plusieurs = !self.ctx.entreprise_du_mois(le_mois).secondaires().is_empty();
            let voulu = nic.map(str::to_string).or_else(|| self.nic());
            let mut n = 0;
            for e in self.etats_du_mois(le_mois)? {
                let Some(v) = e.valide() else { continue };
                if plusieurs && nic_de_l_engagement(self.ctx, &e)?.as_deref() != voulu.as_deref() {
                    continue;
                }
                if engagement::etats::correction_due(self.ctx, v, &departs, le_mois, None)?.is_some() {
                    n += 1;
                }
            }
            n
        };
        if corriges > 0 {
            modifiee = true;
            out.saisie_modifiee = Some(true);
        }
        let quand = quand_lisible(derniere.quand());
        let jour = lim.jj_mm_aaaa();
        if corriges > 0 {
            let pluriel = if corriges > 1 { "s" } else { "" };
            out.corrigee_par_la_suivante = true;
            out.phrase = format!(
                "Paie modifiée depuis la DSN du {}, après la limite de l'annule-et-remplace ({} à minuit) — {} contrat{} corrigé{} par la DSN suivante",
                quand, jour, corriges, pluriel, pluriel
            );
            return Ok(out);
        }
        if modifiee {
            if admis {
                out.a_envoyer = true;
                out.phrase = format!("Paie modifiée depuis la DSN du {} — annule-et-remplace à envoyer, jusqu'au {} à minuit", quand, jour);
            } else {
                out.phrase = format!(
                    "Paie modifiée depuis la DSN du {}, après la limite de l'annule-et-remplace ({} à minuit) — la correction part dans la DSN suivante",
                    quand, jour
                );
            }
        } else if admis {
            out.phrase = format!("Une modification reste possible : la DSN refaite partira en annule-et-remplace (ordre {}), jusqu'au {} à minuit", dernier + 1, jour);
        }
        Ok(out)
    }

    /// `paie_changee_depuis_l_ecriture` : `None` si le fichier declare la paie
    /// d'aujourd'hui, sinon POURQUOI. Le journal des ecritures (`compagnie::pieces_ecrites`) se lit tel qu'il est.
    pub(crate) fn paie_changee_depuis_l_ecriture(&self, le_mois: Mois, fichier: &Path) -> R<Option<String>> {
        use compagnie::pieces_ecrites::{piece_notee, EmpreinteNotee};
        let note = piece_notee(self.racine(), fichier)?;
        if let Some((n, EmpreinteNotee::Saisie(notee))) = note.as_ref().and_then(|n| Some((n, n.empreinte_saisie.as_ref()?))) {
            if self.empreinte_saisie(le_mois)?.as_deref() != Some(notee.as_str()) {
                return Ok(Some(format!("l'engagement a changé depuis que ce fichier a été écrit (le {})", quand_lisible(&n.quand))));
            }
            return Ok(None);
        }
        // SANS EMPREINTE NOTEE, LE MOTEUR NE SAIT PAS : il ne compare plus les dates des fichiers. Une fictive
        // livree, decompressee chez un visiteur, a des dates de fichier qui ne disent rien de l'ordre ou elle a ete ecrite,
        // et chacun de ses mois se disait « paie changee ».
        Ok(None)
    }

    // ----------------------------------------------------------- les groupes

    /// LE CHEMIN DU FICHIER A DEPOSER (`quoi` : DPAE ou DSN) : ou il en est (ecrite, en test, testee, test refuse,
    /// paie changee), le fichier, et ce qu'un test refuse a dit ; `None` sans fichier a deposer, ou s'il est deja parti.
    /// `nic` : pour la DSN d'une compagnie a plusieurs etablissements, celle de cet etablissement (son en-tete,
    /// S21.G00.11.001) ; aucun : le premier fichier, quel qu'il soit.
    fn chemin_de_depot(&mut self, le_mois: Mois, quoi: &str, nic: Option<&str>) -> R<Option<(&'static str, Deposable, Option<String>)>> {
        // « Ecrite » pour une DSN : le meme critere que l'horloge, l'accueil et les rappels (`depart::mois_ecrit`,
        // la DSN mensuelle notee au journal des ecritures), pas un fichier pose dans `sorties/<mois>/`. Un mois passe lu a
        // l'ouverture (`ecrit`) ne demande que ce qui reste, pas le geste qui l'ouvre.
        if self.ecrit || (quoi == DSN && !crate::gestes::depart::mois_ecrit(self.racine(), le_mois)?) {
            return Ok(None);
        }
        let fichiers = self.fichiers_deposables(le_mois)?;
        let mut trouve = None;
        for f in fichiers.de(quoi) {
            let de_l_etablissement = match (quoi == DSN, nic) {
                (true, Some(n)) => declarations::dsn_lue::lire(&crate::depots::lire_octets(&f.chemin)?, "")?.etablissement() == Some(n),
                _ => true,
            };
            if de_l_etablissement {
                trouve = Some(f.clone());
                break;
            }
        }
        let Some(premier) = trouve else {
            return Ok(None);
        };
        let chemin = PathBuf::from(&premier.chemin);
        let lignes = self.lignes()?;
        let empreinte = envois::journal::empreinte_fichier(&chemin).map_err(|m| m.reclasser(Classe::FichierAbsent))?;
        let acte = if quoi == DSN { envois::journal::Acte::Dsn } else { envois::journal::Acte::Dpae };
        if etats::deja_depose(&lignes, acte, &empreinte).is_some() {
            return Ok(None);
        }
        let nom = socle::fichiers::nom_du(&chemin);
        let r = etats::repetition(&lignes, acte, &empreinte, &nom);
        let etape = if quoi == DSN && self.paie_changee_depuis_l_ecriture(le_mois, &chemin)?.is_some() {
            "paie_changee"
        } else {
            r.etat.nom()
        };
        // UN TEST REFUSE SE DIT AVEC SES ANOMALIES, la ou l'on propose d'envoyer : le code et le message du bilan.
        let refus = r.motif.as_deref().filter(|_| etape == "test_refuse").map(|m| m.trim_end_matches('.').to_string());
        Ok(Some((etape, premier, refus)))
    }

    /// LE GESTE d'une case orange qui depose (DPAE, DSN), une fois sa declaration ecrite : la charge du depot qui la
    /// designe (`depots::designee` : le mois, l'etablissement, les embauches que le fichier ecrit declare) ; sinon `defaut`.
    fn action_de_depot(&mut self, le_mois: Mois, quoi: &str, nic: Option<&str>, defaut: Option<Action>) -> R<(Option<Action>, Option<String>)> {
        let Some((etape, fichier, refus)) = self.chemin_de_depot(le_mois, quoi, nic)? else { return Ok((defaut, None)) };
        let mut charge = Designation::du_mois(if quoi == DSN { Quoi::Dsn } else { Quoi::Dpae }, le_mois);
        if quoi == DSN {
            charge.etablissement = nic.map(String::from);
        }
        if quoi == DPAE {
            let a = crate::depots::apercu_depot_dpae(self.ctx, &fichier.chemin)?;
            // La DPAE s'ecrit depuis des engagements valides : chacune de ses embauches a sa fiche et son contrat.
            let designee = |e: crate::depots::EmbaucheDuFichier| match (e.salarie, e.numero) {
                (Some(salarie), Some(numero)) => Ok(EmbaucheDesignee { salarie, numero }),
                _ => Err(erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : une embauche de la DPAE écrite ({}) n'a pas son engagement.", e.nom))),
            };
            charge.engagements = Some(a.embauches.into_iter().map(designee).collect::<R<Vec<_>>>()?);
        }
        let nom = if quoi == DSN { "la DSN" } else { "la DPAE" };
        let deposer = |libelle: String| action(Route::DepotApercu, charge.clone(), Some(Route::Deposer), libelle);
        let action = match etape {
            // Le fichier ecrit ne declare plus la paie : la DSN se refait depuis les engagements valides, et repasse par
            // le test.
            "paie_changee" => deposer_dsn(le_mois, nic, format!("Réécrire {} et la tester…", nom)),
            "ecrite" => deposer(format!("Tester {}…", nom)),
            "en_test" => deposer("Voir le test en cours…".into()),
            "testee" => deposer(format!("Envoyer {} testée…", nom)),
            _ => deposer("Voir le refus du test…".into()),
        };
        Ok((Some(action), refus.map(|r| format!("test refusé : {}", r))))
    }

    /// `bilan_du_mois` : LES ETAPES DU MOIS, dans l'ordre de l'habitude (DPAE, Contrats, DSN, Documents), chacune
    /// avec les groupes que le panneau dessine (`Groupe`) : un groupe `fait` n'a pas de compte ; un groupe qui reste compte
    /// en nombre simple, avec son echeance (`texte`, et `court` sans l'annee, que la case ecrit) et le geste qu'ouvre un
    /// clic. `colonnes` : celles du mois (`colonnes`, G d'abord) ; chaque groupe nomme celles qu'il couvre, les memes d'une
    /// etape a l'autre. L'etape `suivante` est la premiere qui a quelque chose a faire de notre cote. Aucun montant : le
    /// brut n'est jamais lu.
    pub(crate) fn bilan_du_mois(&mut self, le_mois: Mois) -> R<Bilan> {
        // UN MOIS TENU AVANT CE LOGICIEL : ses etapes se sont faites ailleurs ; chaque case le dit (« mois importé »),
        // sans geste. Ce que l'import en dit (`import::mois::etapes`) se lit tel qu'il est.
        if let Some(vue) = import::mois::vue(self.ctx, le_mois)? {
            use import::mois::Teinte;
            let b = import::mois::etapes(&vue);
            let colonnes = vec!["G".to_string()];
            let etapes = ORDRE
                .iter()
                .map(|cle| {
                    let x = match cle {
                        QuelleEtape::Dpae => &b.dpae,
                        QuelleEtape::Signature => &b.signature,
                        QuelleEtape::Dsn => &b.dsn,
                        QuelleEtape::Documents => &b.documents,
                    };
                    let etat = match x.teinte {
                        Teinte::Faite => EtatCase::Fait,
                        Teinte::Refusee => EtatCase::Refuse,
                        Teinte::SansTrace => EtatCase::Information,
                    };
                    let quoi = match (&x.courte, x.teinte) {
                        (Some(c), Teinte::Refusee | Teinte::SansTrace) => format!("mois importé · {}", c),
                        _ => "mois importé".to_string(),
                    };
                    let g = Groupe::de(etat, None, String::new(), quoi, None, None).sous(&colonnes);
                    Etape { phrase: x.phrase.clone(), ..Etape::de(*cle, vec![g]) }
                })
                .collect();
            let etapes = suivante(etapes);
            return Ok(Bilan { mois: le_mois, phrase: phrase_du_mois(None, &etapes), colonnes, etapes });
        }
        let paies = self.etat_par_engagement(le_mois)?;
        let echeance_de = |ps: &[&Vue], dpae: bool| -> Option<Echeance> {
            ps.iter().filter_map(|p| p.echeances.as_ref()).map(|e| if dpae { &e.dpae } else { &e.contrat }).min_by_key(|e| e.jour).cloned()
        };

        // --- Les colonnes : chaque engagement a son point, la DSN celui de son etablissement.
        let classes = self.groupes_dsn(le_mois)?;
        // La classe d'un engagement : celle de son etablissement. Une compagnie d'un seul etablissement n'a que lui. Une
        // carte qui ne nomme pas encore son salarie, dans une compagnie de plusieurs etablissements, n'en a pas : sa DSN
        // est a completer. Un etablissement qui n'est pas de la compagnie se refuse.
        let classe_de = |p: &Vue| -> R<Option<usize>> {
            match (p.etablissement.as_deref(), classes.len()) {
                (_, 1) => Ok(Some(0)),
                (None, _) => Ok(None),
                (Some(nic), _) => match classes.iter().position(|(nics, _)| nics.iter().any(|n| n == nic)) {
                    Some(k) => Ok(Some(k)),
                    None => Err(erreur(Classe::Illisible, format!("L'établissement {} d'un engagement de {} n'est pas un établissement de la compagnie : corrigez la fiche du salarié.", nic, le_mois.libelle(socle::mois::Graphie::Accentuee)))),
                },
            }
        };
        let points: Vec<Point> = paies.iter().map(|l| Ok(Point::de(&l.vue, classe_de(&l.vue)?))).collect::<R<Vec<Point>>>()?;
        let dsn_fait: Vec<bool> = classes.iter().map(|(_, g)| g.etat == EtatCase::Fait).collect();
        let cols = colonnes(&points, &dsn_fait);
        let de = |m: &[usize]| -> Vec<&Vue> { m.iter().map(|i| &paies[*i].vue).collect() };

        // --- DPAE : par embauche du mois. Partie, elle est faite tant qu'aucun retour ne revient KO.
        let mut dpae = Vec::new();
        for (point, m, noms, _) in cases_de_la_ligne(&cols, |c| point_de(c, &points, |p| p.dpae, &["a_faire", "refuse", A_COMPLETER, A_VALIDER])) {
            let siens: Vec<&Vue> = de(&m).into_iter().zip(m.iter()).filter(|(_, i)| points[**i].dpae == point).map(|(p, _)| p).collect();
            let n = siens.len();
            let g = match point.as_str() {
                "refuse" => Groupe::de(EtatCase::Refuse, Some(Compte::Nombre(n)), n.to_string(), format!("refusée{} · l'éditeur est prévenu", plur(n, "", "s")), echeance_de(&siens, true), Some(historique(le_mois))),
                "a_faire" => {
                    let (a, refus) = self.action_de_depot(le_mois, DPAE, None, Some(declarer_embauches(le_mois)))?;
                    // Le jour de l'embauche passe, la DPAE qui devait le preceder (L. 1221-10) est en retard ; elle se depose
                    // toujours.
                    let echeance = echeance_de(&siens, true);
                    let en_retard = siens.iter().filter(|p| p.echeances.as_ref().is_some_and(|e| e.dpae.jour < self.ctx.aujourdhui)).count();
                    let etat = match en_retard {
                        0 => "à déposer".to_string(),
                        r if r == n => "à déposer, en retard".to_string(),
                        r => format!("à déposer, dont {} en retard", r),
                    };
                    let quoi = avec_refus(&etat, refus);
                    Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), quoi, echeance, a)
                }
                A_COMPLETER => a_completer(n),
                A_VALIDER => a_valider(n),
                RIEN_A_DECLARER => rien_ici("rien à déclarer"),
                _ => Groupe::fait(),
            };
            dpae.push(g.sous(&noms));
        }

        // --- Contrats : un par embauche dont la nature a un contrat ecrit (`engagement::contrat_a_signer`).
        let mut contrats = Vec::new();
        for (point, m, noms, _) in cases_de_la_ligne(&cols, |c| point_de(c, &points, |p| p.contrat, &["a_envoyer", "a_refaire", "en_attente", A_COMPLETER, A_VALIDER])) {
            let siens: Vec<&Vue> = de(&m).into_iter().zip(m.iter()).filter(|(_, i)| points[**i].contrat == point).map(|(p, _)| p).collect();
            let n = siens.len();
            let g = match point.as_str() {
                "en_attente" => {
                    // CE QUI EST REVENU SUR CE QUI VIENT DE PARTIR, et sur cela seul : les contrats de la case partis le
                    // jour du dernier envoi encore attendu.
                    let jour = |p: &Vue| p.etats.signature.fait.as_deref().map(|f| f.chars().take(10).collect::<String>());
                    let dernier = siens.iter().map(|p| jour(p)).max().flatten();
                    let partis: Vec<&Vue> = de(&m)
                        .into_iter()
                        .zip(m.iter())
                        .filter(|(p, i)| matches!(points[**i].contrat, "fait" | "en_attente") && p.etats.signature.parti() && jour(p) == dernier)
                        .map(|(p, _)| p)
                        .collect();
                    let recus = partis.iter().filter(|p| p.etats.signature.recu.is_some() && !p.etats.signature.avenant_attendu).count();
                    // L'echeance est celle des contrats attendus (L. 1242-13) ; un avenant attendu n'en a pas (aucun texte
                    // depose ne date sa remise).
                    let contrats_attendus: Vec<&Vue> = siens.iter().copied().filter(|p| !p.etats.signature.avenant_attendu).collect();
                    Groupe::de(
                        EtatCase::EnAttente,
                        Some(Compte::Recus { recus, partis: partis.len() }),
                        format!("{}/{}", recus, partis.len()),
                        format!("reçu{}", plur(recus, "", "s")),
                        echeance_de(&contrats_attendus, false),
                        Some(contrats_en_attente(le_mois)),
                    )
                }
                // L'AVENANT n'a pas d'echeance : L. 1242-13 date la remise du contrat a l'embauche, aucun texte depose ne
                // date celle d'un avenant (regle 1 : rien ne se devine).
                "a_refaire" => Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à refaire signer".into(), None, Some(faire_signer(le_mois))),
                "a_envoyer" => Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à envoyer".into(), echeance_de(&siens, false), Some(faire_signer(le_mois))),
                A_COMPLETER => a_completer(n),
                A_VALIDER => a_valider(n),
                RIEN_A_SIGNER => rien_ici("rien à signer"),
                _ => Groupe::fait(),
            };
            contrats.push(g.sous(&noms));
        }

        // --- DSN : par etablissement (`groupes_dsn`), chaque colonne sous la DSN de ses etablissements ; celle des cartes
        // qui ne nomment pas encore leur salarie est a completer.
        let dsn: Vec<Groupe> = cases_de_la_ligne(&cols, |c| format!("{:?}", c.dsn))
            .into_iter()
            .map(|(_, m, noms, k)| match k {
                Some(k) => classes[k].1.clone().sous(&noms),
                None => a_completer(m.len()).sous(&noms),
            })
            .collect();

        // --- Documents : par salarie du mois, des que son engagement est valide, sans echeance : ils partent quand on veut.
        let mut documents = Vec::new();
        for (point, m, noms, _) in cases_de_la_ligne(&cols, |c| point_de(c, &points, |p| p.documents, &["a_renvoyer", "a_envoyer", A_COMPLETER, A_VALIDER])) {
            let siens: Vec<&Vue> = de(&m).into_iter().zip(m.iter()).filter(|(_, i)| points[**i].documents == point).map(|(p, _)| p).collect();
            let n = siens.iter().map(|p| p.salarie.as_deref()).collect::<BTreeSet<Option<&str>>>().len();
            let g = match point.as_str() {
                "a_renvoyer" => Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à renvoyer".into(), None, Some(envoyer_documents(le_mois, format!("Renvoyer les documents corrigés ({})…", n)))),
                "a_envoyer" => Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à envoyer".into(), None, Some(envoyer_documents(le_mois, "Envoyer les documents…".into()))),
                // Ce qui reste a completer se compte en engagements, comme a la DPAE.
                A_COMPLETER => a_completer(siens.len()),
                A_VALIDER => a_valider(siens.len()),
                _ => Groupe::fait(),
            };
            documents.push(g.sous(&noms));
        }
        // CE QUI PART CE MOIS-CI, NOMME PAR LE MOTEUR : les pieces des paies du mois, dans l'ordre de l'envoi.
        let noms_pieces = [("bulletin", "bulletin"), ("certificat_cs", "certificat Congés Spectacles"), ("aer", "AER")];
        let presentes: Vec<&str> = noms_pieces.iter().filter(|(c, _)| paies.iter().any(|p| p.vue.pieces.contains(c))).map(|(_, n)| *n).collect();

        // UN MOIS SANS ENGAGEMENT n'a rien de fait : ce qui n'a rien a faire s'y dit « rien ce mois-ci », en gris. Ce que le
        // moteur demande quand meme (une DSN due sans salarie) garde son etat.
        let vide = paies.is_empty();
        // La DSN d'un mois sans engagement, partie sans individu, est faite ; pas due, elle n'a rien a faire.
        let dsn_partie = vide && compagnie::etat_dsn::du_mois(self.ctx, le_mois, None)?.partie();
        let rien = |gs: Vec<Groupe>| -> Vec<Groupe> { gs.into_iter().map(|g| if vide && g.etat == EtatCase::Fait { Groupe::rien_ce_mois(g.colonnes) } else { g }).collect() };
        let dsn = if dsn_partie { dsn } else { rien(dsn) };
        // Un mois sans engagement se dit une fois : ce qui reste (sa DSN) se dit dans sa case.
        let debut = match (vide, dsn.iter().all(|g| g.etat == EtatCase::Rien)) {
            (false, _) => None,
            (true, rien_du_tout) => Some(format!(
                "Aucun engagement en {} : {}",
                le_mois.libelle(Graphie::Accentuee),
                if rien_du_tout { "rien à faire" } else { "ni DPAE, ni contrat, ni document" }
            )),
        };
        let etapes = vec![
            Etape::de(QuelleEtape::Dpae, rien(dpae)),
            Etape::de(QuelleEtape::Signature, rien(contrats)),
            Etape::de(QuelleEtape::Dsn, dsn),
            Etape { detail: Some(presentes.join(", ")), ..Etape::de(QuelleEtape::Documents, rien(documents)) },
        ];
        let noms: Vec<String> = cols.iter().map(|c| c.nom.clone()).collect();
        let etapes = suivante(etapes);
        Ok(Bilan { mois: le_mois, phrase: phrase_du_mois(debut, &etapes), colonnes: noms, etapes })
    }

    /// LA DSN DU MOIS, par etablissement. Elle « est mensuelle et produite par établissement employeur » (cahier technique
    /// DSN 2026.1, § 2.4, `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`) : chaque
    /// etablissement a la sienne (`groupes_dsn_de`). Les etablissements au meme point (meme etat, phrase et echeance)
    /// forment une classe, une case ; quand ils ne sont pas tous au meme point, la case d'une classe qui n'est pas faite
    /// nomme ses etablissements (« Lyon refusée · l'éditeur est prévenu »). Rend les classes (leurs NIC, leur case), le
    /// principal dans la premiere.
    fn groupes_dsn(&mut self, mois: Mois) -> R<Vec<(Vec<String>, Groupe)>> {
        let ent = self.ctx.entreprise_du_mois(mois);
        if ent.secondaires().is_empty() {
            let g = self.groupes_dsn_de(mois, None)?;
            // Une fiche qui ne dit pas encore son NIC n'a que son siege : sa classe ne le nomme pas.
            return Ok(vec![(ent.nic_principal().map(str::to_string).into_iter().collect(), g)]);
        }
        let point = |g: &Groupe| (g.etat, g.phrase.clone(), g.echeance.as_ref().map(|e| e.jour));
        let mut classes: Vec<(Vec<String>, Groupe)> = Vec::new();
        for nic in ent.nics()? {
            let g = self.groupes_dsn_de(mois, Some(&nic))?;
            match classes.iter_mut().find(|(_, x)| point(x) == point(&g)) {
                Some((nics, _)) => nics.push(nic),
                None => classes.push((vec![nic], g)),
            }
        }
        if classes.len() > 1 {
            for (nics, g) in classes.iter_mut().filter(|(_, g)| g.etat != EtatCase::Fait) {
                let noms: Vec<String> = nics.iter().map(|n| nom_de_l_etablissement(&ent, n)).collect();
                g.compte_texte = noms.join(", ");
                g.phrase = Some(format!("{} {}", noms.join(", "), g.quoi));
            }
        }
        Ok(classes)
    }

    /// LES GROUPES DE LA DSN de l'etablissement `nic` (`Null` : la compagnie n'en a qu'un). Une DSN partie ne se refait que
    /// si ce qu'elle a emporte est a corriger. L'echeance : celle du depot (`echeance_dsn`, le jour meme a midi) ; la veille a minuit pour un
    /// annule-et-remplace ; apres un refus, celle du depot qui la reprend.
    ///
    /// LA DSN REFUSEE n'est pas integree : celle qui la reprend est un depot initial (cahier technique DSN 2026.1,
    /// § 1.4.2, `atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt` : le depot initial va jusqu'au jour de
    /// l'echeance a midi, l'annule-et-remplace jusqu'a la veille a minuit). INFERENCE : un depot refuse n'ouvre pas
    /// l'annule-et-remplace, puisqu'il n'y a rien a annuler ; l'echeance de la reprise est celle du depot initial.
    fn groupes_dsn_de(&mut self, le_mois: Mois, nic: Option<&str>) -> R<Groupe> {
        let echeance = declarations::echeance_dsn::de_la_compagnie(self.ctx, le_mois).map_err(|m| m.reclasser(Classe::Illisible))?;
        let au_depot = echeance_dsn(echeance.jour, "à midi", "Cahier technique DSN 2026.1, § 1.4.2 ; CSS R. 243-6 et R. 133-14 (declarations::echeance_dsn)");
        let e = compagnie::etat_dsn::du_mois(self.ctx, le_mois, nic)?;
        let partie = e.deposee();
        if !partie {
            // Due chaque mois, sans salarie aussi (`etat_dsn::due`, ce que l'echeancier et les rappels lisent) ; un mois
            // sans rien a declarer porte encore la correction d'un mois passe (`dsn_a_declarer`).
            let a_declarer = self.dsn_a_declarer(le_mois, nic)?;
            if !a_declarer && !compagnie::etat_dsn::due(self.ctx, le_mois, nic)? {
                return Ok(Groupe::fait());
            }
            // Avant son premier jour de depot, la case reste a deposer : c'est le depot qui dit le jour
            // (`gestes::depart::fabriquer_dsn`).
            let (a, refus) = self.action_de_depot(le_mois, DSN, nic, Some(deposer_dsn(le_mois, nic, "Déposer la DSN…".into())))?;
            let etat = if a_declarer { "à déposer" } else { "à déposer, sans salarié" };
            let quoi = avec_refus(etat, refus);
            return Ok(Groupe::de(EtatCase::AFaire, None, String::new(), quoi, Some(au_depot), a));
        }
        if e.refusee() {
            return Ok(Groupe::de(EtatCase::Refuse, None, String::new(), "refusée · l'éditeur est prévenu".into(), Some(au_depot), Some(historique(le_mois))));
        }
        // Partie : faite, tant qu'aucun retour ne revient KO. Ce qui la rouvre : une paie modifiee depuis.
        let r = self.etat_remplacement(le_mois, nic)?;
        let libelle = format!("Refaire la DSN (annule-et-remplace, ordre {})…", r.prochain_ordre);
        let veille = echeance_dsn(r.limite, "à minuit", "Cahier technique DSN 2026.1, § 1.4.2 (atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt)");
        let modifiee = r.saisie_modifiee == Some(true);
        if modifiee && r.a_envoyer {
            // Un annule-et-remplace deja ecrit montre son chemin ; sinon, le refaire.
            let ecrit = self.chemin_de_depot(le_mois, DSN, nic)?.is_some();
            let (a, refus) = if ecrit { self.action_de_depot(le_mois, DSN, nic, None)? } else { (Some(deposer_dsn(le_mois, nic, libelle)), None) };
            let quoi = avec_refus("à refaire", refus);
            return Ok(Groupe::de(EtatCase::AFaire, None, String::new(), quoi, Some(veille), a));
        }
        if modifiee {
            // Apres la veille : la correction part dans la DSN qui porte (la premiere d'un mois suivant qui n'est pas
            // partie), celle que l'avertissement de « Modifier l'engagement » nomme aussi.
            let suivant = compagnie::etat_dsn::dsn_qui_porte(self.ctx, le_mois)?;
            let e2 = declarations::echeance_dsn::de_la_compagnie(self.ctx, suivant).map_err(|m| m.reclasser(Classe::Illisible))?;
            let a = deposer_dsn(suivant, nic, format!("Déposer la DSN de {}…", suivant.libelle(Graphie::Accentuee)));
            return Ok(Groupe::de(
                EtatCase::AFaire,
                None,
                String::new(),
                format!("à corriger dans la DSN de {}", suivant.libelle(Graphie::Accentuee)),
                Some(echeance_dsn(e2.jour, "à midi", "Cahier technique DSN 2026.1, § 1.4.2 (declarations::echeance_dsn)")),
                Some(a),
            ));
        }
        Ok(Groupe::fait())
    }

    /// LA DSN DE `mois` A-T-ELLE QUELQUE CHOSE A DECLARER pour l'etablissement `nic` (`Null` : le seul) ? Elle est a
    /// deposer des qu'un engagement valide touche le mois (« le dépôt initial de la DSN "normale" peut
    /// être émis jusqu'à 1 mois à l'avance et 3 mois en retard », cahier technique DSN 2026.1, § 1.4.2,
    /// `atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt`), ou qu'elle porte la correction d'un mois passe
    /// dont l'annule-et-remplace n'est plus admis (§ 2.1.2.1, `chapitre-2-principes-de-constitution.txt` : « Une
    /// correction est un évènement connu après la date de clôture de la paie. Il donne lieu à la déclaration d'une
    /// correction dans la DSN suivante » ; `produire_regularisations::regularisations_pour`). Sinon, rien n'est a faire.
    /// Ne lit que ce qui est ecrit.
    fn dsn_a_declarer(&mut self, le_mois: Mois, nic: Option<&str>) -> R<bool> {
        let plusieurs = !self.ctx.entreprise_du_mois(le_mois).secondaires().is_empty();
        let voulu = nic.map(str::to_string).or_else(|| self.nic());
        let de_l_etablissement = |ctx: &Contexte, brut: &engagement::stock::Ecrit| -> R<bool> { Ok(!plusieurs || nic_de_l_engagement_ecrit(ctx, brut)?.as_deref() == voulu.as_deref()) };
        for e in self.etats_du_mois(le_mois)? {
            if e.valide().is_some() && de_l_etablissement(self.ctx, &e.objet().brut)? {
                return Ok(true);
            }
        }
        let departs = self.departs()?;
        for mm in engagement::stock::mois_saisis(self.racine())? {
            if mm >= le_mois {
                break;
            }
            if !declarations::produire_regularisations::se_corrige_ailleurs(self.ctx, &departs, mm)? {
                continue;
            }
            for e in self.etats_du_mois(mm)? {
                let Some(v) = e.valide() else { continue };
                if de_l_etablissement(self.ctx, &e.objet().brut)? && engagement::etats::correction_due(self.ctx, v, &departs, mm, Some(le_mois))?.is_some() {
                    return Ok(true);
                }
            }
            for n in engagement::etats::non_actifs_du_mois(self.ctx, mm)? {
                if de_l_etablissement(self.ctx, &n.objet().brut)? && n.correction_due(self.ctx, &departs, mm, le_mois)?.is_some() {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    // ------------------------------------------------------- les autres routes

    /// `exercice_clos` : l'annee `annee` est-elle close (`gestes::exercice_ecrit::lire_exercices`) ?
    pub(crate) fn exercice_clos(&self, annee: i32) -> R<bool> {
        Ok(crate::gestes::exercice_ecrit::lire_exercices(self.racine())?.de(&annee.to_string()).is_some())
    }

    /// `mois_a_ouvrir` : le plus ancien mois passe dont une etape n'est pas
    /// verte, sinon le mois du calendrier.
    fn mois_a_ouvrir(&mut self) -> R<MoisAOuvrir> {
        let courant = self.ctx.aujourdhui.mois_civil();
        // A L'OUVERTURE, RIEN NE SE CALCULE : les groupes d'un mois passe se tirent de ce qui est ecrit.
        self.ecrit = true;
        // Un mois passe sans engagement dont la DSN est due (ce que les rappels reclament) s'ouvre aussi.
        for m in crate::rappels::mois_a_parcourir(self.ctx, self.ctx.aujourdhui)? {
            if m >= courant || self.exercice_clos(m.annee())? {
                continue;
            }
            let reste = self.etapes_restantes(m)?;
            if !reste.is_empty() {
                let noms: Vec<&str> = ORDRE.iter().filter(|c| reste.contains(c)).map(|c| c.nom()).collect();
                let phrase = format!("À ouvrir : {}, où il reste {}.", m.libelle(socle::mois::Graphie::Accentuee), noms.join(", "));
                return Ok(MoisAOuvrir { phrase, mois: m, courant, reste });
            }
        }
        let phrase = format!("À ouvrir : {}, le mois du jour ; rien ne reste aux mois d'avant.", courant.libelle(Graphie::Accentuee));
        Ok(MoisAOuvrir { phrase, mois: courant, courant, reste: Vec::new() })
    }

    /// LES ETAPES QUI RESTENT A UN MOIS PASSE : celles dont un groupe n'est pas fait (`bilan_du_mois`).
    pub(crate) fn etapes_restantes(&mut self, mois: Mois) -> R<Vec<QuelleEtape>> {
        let b = self.bilan_du_mois(mois)?;
        Ok(b.etapes.iter().filter(|e| e.groupes.iter().any(|x| !matches!(x.etat, EtatCase::Fait | EtatCase::Rien))).map(|e| e.cle).collect())
    }

    /// `embauches_a_declarer` : les contrats des engagements valides dont la DPAE n'est pas encore partie, chacun avec
    /// l'empreinte de ce que sa DPAE declare. La seule liste des DPAE a faire : le panneau, les rappels, le filet,
    /// l'horloge la lisent.
    pub fn embauches_a_declarer(&mut self, mois: Option<Mois>) -> R<Vec<DpaeDue>> {
        // UNE DERIVATION POUR TOUS LES MOIS : le bilan de chaque mois charge sa saisie, et les conges d'un
        // permanent y lisent les paies validees des mois d'avant (`conges::paies_du_mois`) une fois pour toute la route.
        // Rien ne s'ecrit entre deux mois de cette lecture : le memo vaut pour toute la route.
        let _derivation = engagement::conges::Derivation::ouvrir();
        let departs = self.departs()?;
        let mois: Vec<Mois> = match mois {
            Some(m) => vec![m],
            None => {
                let mut v = engagement::stock::mois_saisis(self.racine())?;
                v.reverse();
                v
            }
        };
        let mut attente = Vec::new();
        for le_mois in mois {
            if !engagement::stock::a_une_saisie(self.racine(), le_mois)? {
                continue;
            }
            let s = self.charger_saisie(le_mois)?;
            let etats_m = engagement::etats::tels_qu_ecrits(self.ctx, le_mois, &departs)?;
            // La DPAE part de l'engagement valide : un engagement en saisie n'a rien a declarer. Chacun retrouve son embauche
            // par son identite (salarie, numero du contrat).
            for e in &etats_m {
                let Some(v) = e.valide() else { continue };
                let brut = &*v.objet().brut;
                let (Some(cle), Some(numero)) = (&brut.saisie.carte.salarie, brut.saisie.carte.numero.as_deref()) else { continue };
                let Some(p) = s.paies.iter().find(|p| p.salarie.id == *cle && p.engagement.numero.as_deref() == Some(numero)) else {
                    continue;
                };
                // Seule la DPAE partie se compare (`debut_declare`) : les paies parties ne se relisent pas ici, sauf
                // quand le mois a deja compare tout ce qui est a refaire (`a_refaire_du_mois`).
                let refaire: Vec<ARefaire> = match self.refaire.get(&(le_mois, cle.to_string(), numero.to_string())) {
                    Some(r) => r.iter().filter(|a| matches!(a, ARefaire::Dpae { .. })).cloned().collect(),
                    None => engagement::etats::a_refaire_parmi(self.ctx, v, &departs, &|s| s == engagement::departs::Envoi::Dpae)?,
                };
                let debut = p.engagement.debut.iso();
                // UNE DPAE PAR EMBAUCHE, reconnue a sa date : « L'embauche d'un salarié ne peut intervenir qu'après
                // déclaration nominative » (code du travail, L. 1221-10), et la DPAE porte la « Date et heure
                // d'embauche » (R. 1221-1, 3°) ; `atelier/reference/sources/code-du-travail/xml/L1221-10.xml` et
                // `R1221-1.xml`. INFERENCE : un cachet ajoute a une date nouvelle d'un engagement deja declare prolonge le
                // meme engagement (un avenant) ; l'embauche et sa date ne changent pas, aucune DPAE ne ressort -- aucun texte
                // depose n'en demande une nouvelle quand la duree (R. 1221-1, 4°) s'allonge. Une date d'embauche avancee,
                // elle, en appelle une (`dpae_d_une_date_changee`, plus bas).
                // La DPAE de cet engagement est partie (`departs`) ; sa date d'embauche n'a pas change depuis (`ARefaire::Dpae`).
                let partie = departs.de(&cle, numero).iter().any(|d| d.envoi == engagement::departs::Envoi::Dpae);
                let ancienne = debut_declare(&refaire);
                if partie && ancienne.is_none() {
                    continue;
                }
                // Un contrat en cours se declare avec son premier mois.
                if !p.engagement.premier_mois() {
                    continue;
                }
                // UNE DATE D'EMBAUCHE CHANGEE depuis la version sur laquelle la DPAE est partie : reculee, la DPAE de l'ancienne
                // date vaut ; avancee, une DPAE est a deposer, et la phrase dit pourquoi (`dpae_d_une_date_changee`). Seulement
                // quand une date d'embauche a change : le reste du temps, rien a dire.
                let pourquoi = match ancienne {
                    Some(a) => {
                        let (vaut, phrase) = crate::regularisation::dpae_d_une_date_changee(&a, &debut)?;
                        if vaut {
                            continue;
                        }
                        Some(phrase)
                    }
                    None => None,
                };
                let titre = p.spectacle.as_ref().and_then(|s| s.titre.dite()).filter(|t| !t.is_empty()).cloned();
                let Some(numero) = p.engagement.numero.clone() else { continue };
                attente.push(DpaeDue {
                    mois: le_mois,
                    salarie: cle.to_string(),
                    numero,
                    nom: super::nom_a_l_ecran(&p.salarie, &cle.to_string()),
                    spectacle: titre,
                    debut,
                    fin: p.engagement.fin.map(|f| f.iso()),
                    empreinte: empreinte_de_l_embauche(&p.engagement, &p.salarie),
                    pourquoi,
                });
            }
        }
        Ok(attente)
    }
}

/// « le 05/11/2026 » d'un instant du journal.
fn le_jour(t: &str) -> R<String> {
    let tete = match t.get(..10) {
        Some(x) => x,
        None => t,
    };
    let jour = Jour::lire(tete).map_err(|e| socle::echec::a_la_cle("Un instant du journal des envois ne se lit pas.", "quand", e.phrase).reclasser(Classe::Illisible))?;
    Ok(format!("le {}", jour.jj_mm_aaaa()))
}

/// `le_jour` de l'instant d'un envoi que l'etat dit parti : sans instant, c'est un defaut du logiciel.
fn le_jour_de(t: &Option<String>) -> R<String> {
    match t {
        Some(x) => le_jour(x),
        None => Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : un envoi parti ne dit pas quand.")),
    }
}

/// Un carre, son etat et sa phrase.
fn carre(cle: QuelleEtape, nom: impl Into<String>, etat: EtatCase, phrase: String, action: Option<Geste>, reste: &'static str) -> Carre {
    Carre { cle, nom: nom.into(), etat, phrase, action, pieces: None, couleur: etat.couleur(), coupee: None, reste, lien: None, titre: if cle == QuelleEtape::Dpae { "DPAE" } else { "Contrat" } }
}

/// LES TROIS CARRES DE LA LIGNE D'UN ENGAGEMENT VALIDE (DPAE, Contrat, Documents), lus comme les cases du panneau
/// (`Point::de`) : chacun son nom, son etat (`fait`, `a_faire`, `en_attente`, `refuse`), sa phrase (l'infobulle), le
/// geste qu'ouvre un clic, et le titre de sa boite. Le contrat ou l'avenant parti et pas revenu se note signe d'un clic
/// (`contrat-recu`).
fn carres(p: &Vue, mois: Mois) -> R<Vec<Carre>> {
    // Les carres ne lisent pas la classe DSN du point (la DSN n'a pas de carre).
    let pt = Point::de(p, None);
    // A COMPLETER, OU A VALIDER : rien n'est parti de lui ; chaque carre qui l'attend dit ce qui reste, sans geste.
    if p.a_completer.is_some() || !p.valide() {
        let attend = |cle: QuelleEtape, nom: &str, point: &str| match (point, p.a_completer.as_deref()) {
            (A_COMPLETER, Some(manque)) => carre(cle, nom, EtatCase::AFaire, format!("À compléter avant de valider : {}", manque), None, "à compléter"),
            (A_VALIDER, _) => carre(cle, nom, EtatCase::AFaire, format!("{} après la validation", nom), None, "à valider"),
            _ => carre(cle, nom, EtatCase::Fait, "Rien à faire ici pour cet engagement".into(), None, ""),
        };
        return Ok(vec![attend(QuelleEtape::Dpae, "DPAE", pt.dpae), attend(QuelleEtape::Signature, "Contrat", pt.contrat), attend(QuelleEtape::Documents, "Documents", pt.documents)]);
    }
    let d = &p.etats.dpae;
    let s = &p.etats.signature;
    let o = &p.etats.documents;
    let embauche = p.contrat.premier_mois();
    // Un engagement valide dit son premier jour.
    let Some(debut) = p.debut else {
        return Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : un engagement valide ne dit pas son premier jour."));
    };
    let debut_dit = debut.jj_mm_aaaa();
    let depuis = p.contrat.depuis();
    let route = |a: Action| Some(Geste::Route(a));

    let (etat, phrase, geste) = match pt.dpae {
        "refuse" => {
            let motif = motif_dpae(d);
            (EtatCase::Refuse, format!("DPAE refusée {} : {}", le_jour_de(&d.fait)?, motif), route(historique(mois)))
        }
        "a_faire" => (EtatCase::AFaire, "DPAE à déposer".to_string(), route(declarer_embauches(mois))),
        RIEN_A_DECLARER => (EtatCase::Fait, format!("Contrat commencé le {}, avant PuppetPaie : rien à déclarer ici", debut_dit), None),
        _ if !embauche => (EtatCase::Fait, format!("DPAE faite en {}, à l'embauche", depuis), None),
        _ if d.passee => (EtatCase::Fait, format!("DPAE déposée {}, passée", le_jour_de(&d.fait)?), None),
        _ => (EtatCase::Fait, format!("DPAE déposée {}, retour pas encore arrivé", le_jour_de(&d.fait)?), None),
    };
    let pieces = Pieces { vide: Some("Aucune DPAE déposée pour cette embauche.".into()), encart: None, gestes: Vec::new() };
    let dpae = Carre { pieces: Some(pieces), ..carre(QuelleEtape::Dpae, "DPAE", etat, phrase, geste, "") };

    let cdi = &s.cdi_en_cours;
    let ailleurs = p.contrat.ailleurs();
    let dossier = p.contrat.dossier();
    let nom_contrat = match cdi {
        Some(c) if !ailleurs => format!("CDI, signé en {}", c.signe_en.libelle(Graphie::Accentuee)),
        _ => "Contrat".to_string(),
    };
    let par_dit = if s.par == Some("imprime") { "imprimé" } else { "envoyé" };
    let (etat, phrase, geste) = match pt.contrat {
        "a_envoyer" => (EtatCase::AFaire, "Contrat à envoyer".to_string(), route(faire_signer(mois))),
        "a_refaire" if !s.parti() => (EtatCase::AFaire, majuscule_initiale(s.pourquoi), route(faire_signer(mois))),
        "a_refaire" => (EtatCase::AFaire, format!("Contrat parti {} : {}", le_jour_de(&s.fait)?, s.pourquoi), route(faire_signer(mois))),
        // Un avenant parti, imprime ou par courriel, se note signe d'un clic, comme le contrat ; la relance est dans la
        // boite des contrats en attente. `par` est celui du dernier depart, l'avenant attendu.
        "en_attente" if s.avenant_attendu => (
            EtatCase::EnAttente,
            match s.avenant.as_ref().and_then(|a| a.numero) {
                Some(n) => format!("Avenant n° {} {} {}", n, par_dit, le_jour_de(&s.avenant.as_ref().map(|a| a.quand.clone()))?),
                None => format!("Avenant {} {}", par_dit, le_jour_de(&s.avenant.as_ref().map(|a| a.quand.clone()))?),
            },
            noter_recu(p, mois, "Noter l'avenant signé", s.avenant.as_ref().and_then(|a| a.numero), true).and_then(&route),
        ),
        "en_attente" => (EtatCase::EnAttente, format!("Contrat {} {}", par_dit, le_jour_de(&s.fait)?), noter_recu(p, mois, "Noter le contrat signé", None, true).and_then(&route)),
        RIEN_A_SIGNER if ailleurs && !p.contrat.anterieur() => (EtatCase::Fait, "Contrat signé hors de PuppetPaie : rien à signer ici".to_string(), geste_du_dossier(dossier)),
        RIEN_A_SIGNER => (EtatCase::Fait, format!("Contrat commencé le {}, avant PuppetPaie : rien à signer ici", debut_dit), geste_du_dossier(dossier)),
        _ if cdi.is_some() => (EtatCase::Fait, format!("CDI en cours depuis {}, sans terme : rien à envoyer", depuis), None),
        _ if !embauche => (EtatCase::Fait, format!("Contrat signé en {}", depuis), None),
        _ if !p.contrat.ecrit() => (EtatCase::Fait, "Pas de contrat à faire signer ici".to_string(), None),
        _ => (EtatCase::Fait, format!("Contrat signé {}", le_jour_de(&s.recu)?), None),
    };
    let contrat = Carre { pieces: Some(pieces_du_contrat(p, mois, s, dossier)), ..carre(QuelleEtape::Signature, nom_contrat, etat, phrase, geste, "") };

    let (etat, phrase, geste) = match pt.documents {
        "a_renvoyer" => (EtatCase::AFaire, format!("Documents à renvoyer : {}", o.pourquoi), route(envoyer_documents(mois, "Renvoyer les documents corrigés…".into()))),
        "a_envoyer" => (EtatCase::AFaire, "Documents à envoyer".to_string(), route(envoyer_documents(mois, "Envoyer les documents…".into()))),
        _ => (EtatCase::Fait, format!("Documents partis {}", le_jour_de(&o.fait)?), None),
    };
    let documents = carre(QuelleEtape::Documents, "Documents", etat, phrase, geste, "");
    // CE QUI RESTE, EN UN MOT, sur le carre meme (la phrase entiere est l'infobulle).
    let restes = [
        match pt.dpae { "refuse" => "refusée", "a_faire" => "à déposer", _ => "" },
        match pt.contrat { "a_envoyer" => "à envoyer", "a_refaire" => "à refaire signer", "en_attente" => "signature attendue", _ => "" },
        match pt.documents { "a_renvoyer" => "à renvoyer", "a_envoyer" => "à envoyer", _ => "" },
    ];
    // LE LIEN DU DETAIL vers ce qui est parti : seulement quand une piece est passee par le logiciel, ou vers le contrat
    // signe hors du logiciel, que son dossier garde.
    let liens = [
        d.fait.is_some().then_some("La DPAE partie"),
        if ailleurs { Some("Le contrat signé") } else { s.parti().then_some("Le contrat parti") },
        None,
    ];
    let mut tous = vec![dpae, contrat, documents];
    for ((c, reste), lien) in tous.iter_mut().zip(restes).zip(liens) {
        c.coupee = Some(c.etat == EtatCase::EnAttente);
        c.reste = reste;
        c.lien = Some(lien);
    }
    Ok(tous)
}

/// CE QUE MONTRE LE DETAIL DU CONTRAT d'un engagement (les pieces parties, ouvertes depuis la ligne) : la phrase quand
/// aucune piece n'est passee par le logiciel, l'encart d'un contrat qui n'est plus le bon, et les gestes, chacun son
/// libelle, son geste et s'il est le principal. Un contrat parti ou recu se note signe (ou ne l'est plus) ; un contrat
/// parti se modifie par un avenant, signe ou non, commence ou non ; un contrat jamais parti part a la signature. Un
/// contrat signe hors du logiciel (`dossier` : son dossier, `signature::contrat_signe_dit`) ne part pas : son PDF se range
/// dans son dossier, et il se modifie par un avenant.
fn pieces_du_contrat(p: &Vue, mois: Mois, s: &Signature, dossier: Option<&ContratSigneDit>) -> Pieces {
    let (fait, recu, caduc) = (s.parti(), s.recu.is_some(), s.caduc);
    let ailleurs = dossier.is_some();
    // Un contrat ou un avenant parti (`fait`, imprime ou par courriel) est une piece : le detail la nomme, il n'a pas de
    // phrase « sans piece ».
    let vide = match (fait, recu) {
        _ if ailleurs => dossier.map(|d| d.phrase.clone()),
        (true, _) => None,
        (false, true) => Some("Le contrat a été noté signé à la main : aucun fichier n'est passé par le logiciel.".into()),
        (false, false) => Some("Aucun contrat n'est parti à la signature pour cet engagement.".into()),
    };
    let encart = caduc.then(|| {
        let pourquoi = majuscule_initiale(s.pourquoi);
        let pourquoi = if pourquoi.is_empty() { "L'engagement a changé depuis qu'il est parti.".to_string() } else { format!("{}.", pourquoi) };
        EncartDePiece { titre: "Ce contrat n'est plus le bon", texte: format!("{} Il se modifie par un avenant, que le salarié signe à son tour.", pourquoi) }
    });
    let route = |a: Action| Some(Geste::Route(a));
    let mut gestes = Vec::new();
    if ailleurs {
        gestes.push(GesteDePiece { libelle: "Ranger le contrat signé…", titre: None, action: geste_du_dossier(dossier), principal: !caduc });
        if caduc || fait {
            gestes.push(GesteDePiece { libelle: "Avenant…", titre: Some("Modifier le contrat signé hors de PuppetPaie : ce qui change, en clair"), action: avenant(p, mois).and_then(&route), principal: caduc });
        }
        return Pieces { vide, encart, gestes };
    }
    if fait || recu {
        let libelle = if recu { "Retirer la mention « signé »" } else { "Noter le contrat signé reçu" };
        gestes.push(GesteDePiece { libelle, titre: None, action: noter_recu(p, mois, libelle, None, !recu).and_then(&route), principal: false });
    }
    if fait {
        gestes.push(GesteDePiece { libelle: "Avenant…", titre: Some("Modifier le contrat parti : l'ancien texte et le nouveau, article par article"), action: avenant(p, mois).and_then(&route), principal: caduc });
    }
    if !fait {
        gestes.push(GesteDePiece { libelle: "Envoyer à la signature…", titre: None, action: route(faire_signer(mois)), principal: true });
    }
    Pieces { vide, encart, gestes }
}

/// Le geste qui ouvre le dossier d'un contrat signe hors du logiciel (`signature::contrat_signe_dit`) : la fenetre l'ouvre
/// et le cree ; la phrase dit ce qu'on y depose. Rien pour un autre contrat.
fn geste_du_dossier(dossier: Option<&ContratSigneDit>) -> Option<Geste> {
    let d = dossier?;
    Some(Geste::Dossier(GesteDuDossier { libelle: "Ranger le contrat signé…", chemin: d.chemin.clone(), phrase: d.phrase.clone() }))
}

/// « Un terme… » d'« un terme… ».
fn majuscule_initiale(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => t.to_string(),
    }
}

/// CE QUE LA BOITE DES DPAE DIT D'UN ENGAGEMENT : s'il se coche (une embauche du mois), s'il est coche d'avance (pas de
/// DPAE, ou une DPAE refusee : c'est la seule raison de redeposer ; une DPAE passee ne se redeclare pas), et son etat
/// (`fait`, `a_faire`, `en_attente`, `refuse`) avec sa phrase.
fn choix_dpae(p: &Vue) -> R<Choix> {
    let d = &p.etats.dpae;
    let choix = |cochable: bool, cochee: bool, etat: EtatCase, phrase: String| -> R<Choix> { Ok(Choix { cochable, cochee, etat, couleur: etat.couleur(), phrase }) };
    // Il lui manque un fait : sa DPAE attend qu'il soit complet.
    if let Some(manque) = &p.a_completer {
        return choix(false, false, EtatCase::AFaire, format!("à compléter avant de valider : {}", manque));
    }
    if p.contrat.anterieur() {
        return choix(false, false, EtatCase::Fait, "contrat commencé avant PuppetPaie : rien à déclarer ici".into());
    }
    if !p.contrat.premier_mois() {
        let depuis = p.contrat.depuis();
        return choix(false, false, EtatCase::Fait, format!("contrat en cours depuis {} — sa DPAE a été faite ce mois-là", depuis));
    }
    // RIEN NE PART D'UN ENGAGEMENT EN SAISIE : le depot le refuserait, et les autres avec lui.
    if !p.valide() {
        return choix(false, false, EtatCase::AFaire, "à valider avant sa DPAE".into());
    }
    // CE QUI SE COCHE EST CE QUE LE DEPOT PREND : la liste des DPAE a faire (`a_declarer`).
    if d.a_declarer && !d.refusee {
        return choix(true, true, EtatCase::AFaire, "à déclarer".into());
    }
    let le = le_jour_de(&d.fait)?;
    if d.refusee {
        let motif = motif_dpae(d);
        return choix(true, true, EtatCase::Refuse, format!("refusée {} — {} — à redéclarer", le, motif));
    }
    if d.passee {
        let dossier = if d.deja_existante {
            " (déjà existante à l'Urssaf, rien de créé)".to_string()
        } else if let Some(x) = d.dossier.as_deref().filter(|x| !x.is_empty()) {
            format!(" (dossier {})", x)
        } else {
            String::new()
        };
        return choix(true, false, EtatCase::Fait, format!("déclarée {} — passée{}", le, dossier));
    }
    choix(true, false, EtatCase::EnAttente, format!("déclarée {} — retour pas encore arrivé : on attend, on ne renvoie pas", le))
}

/// Les motifs d'un refus de DPAE, lus des anomalies du retour : « E99 : anomalie fictive », ou « motif non lu ».
fn motif_dpae(d: &EtatDpae) -> String {
    match d.anomalies.as_deref() {
        Some(a) if !a.is_empty() => a.join(" ; "),
        _ => "motif non lu".to_string(),
    }
}

/// Les documents d'un salarie partis pour un mois (`documents_partis`) : quand, et ce que leur envoi en a resume
/// (`envois::etats::resumes_des_documents`, lu tel qu'il est).
#[derive(Debug, Clone)]
pub(crate) struct DocumentsPartis {
    pub(crate) quand: String,
    pub(crate) resume: Option<envois::envoi::ResumeDePaie>,
}

/// LES DOCUMENTS PARTIS DE `mois`, par salarie : le dernier envoi de ceux dont tous les engagements valides du mois sont
/// partis.
pub(crate) fn documents_partis(ctx: &Contexte, mois: Mois, etats: &[engagement::etats::Etat], lignes: &[envois::journal::Ligne]) -> R<HashMap<String, DocumentsPartis>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let resumes = etats::resumes_des_documents(lignes, mois);
    let mut out: HashMap<String, DocumentsPartis> = HashMap::new();
    // Les documents d'un salarie partent ensemble, pour tous ses engagements valides du mois : un engagement valide dont
    // ils ne sont pas partis (valide apres leur envoi) les rend a envoyer, avec son bulletin.
    let a_envoyer: HashSet<String> = etats
        .iter()
        .filter(|e| matches!(e, engagement::etats::Etat::Valide(_)))
        .filter_map(|e| e.objet().brut.saisie.carte.salarie.as_ref().map(ToString::to_string))
        .collect();
    for e in etats {
        if !matches!(e, engagement::etats::Etat::Partis(_)) {
            continue;
        }
        let brut = &*e.objet().brut;
        let (Some(cle), Some(numero)) = (brut.saisie.carte.salarie.as_ref().map(ToString::to_string), brut.saisie.carte.numero_dit().map(String::from)) else { continue };
        let dernier = departs
            .de(&cle, &numero)
            .iter()
            .filter(|d| d.envoi == engagement::departs::Envoi::Documents && d.mois.is_some_and(|m| m == mois) && d.par.is_none())
            .map(|d| d.le.clone())
            .max();
        let Some(quand) = dernier else { continue };
        if a_envoyer.contains(&cle) {
            continue;
        }
        let plus_recent = out.get(&cle).is_some_and(|v| v.quand >= quand);
        if !plus_recent {
            out.insert(cle.clone(), DocumentsPartis { resume: resumes.get(&cle).cloned().flatten(), quand });
        }
    }
    Ok(out)
}
/// La phrase de la pastille Documents d'un salarie dont la paie a change depuis l'envoi.
pub(crate) const DOCUMENTS_CADUCS: &str = "la paie a changé depuis l'envoi";

/// LA PASTILLE DOCUMENTS d'un engagement pour un mois. `parti_le` : le dernier envoi de ses documents ; `caduc` : ils
/// sont partis, et sa paie a change depuis la version sur laquelle ils sont partis (`engagement::etats::a_refaire`) --
/// ils ne sont plus les bons, ils sont a renvoyer.
fn etat_documents(parti_le: Option<String>, caduc: bool) -> EtatDocuments {
    let d = EtatDocuments { fait: parti_le, caduc: false, pourquoi: "" };
    let caduc = d.parti() && caduc;
    EtatDocuments { caduc, pourquoi: if caduc { DOCUMENTS_CADUCS } else { "" }, ..d }
}

/// `DSN_[^\W\d_]+\d{4}(_AR\d+)?\.dsn` : le nom d'une DSN mensuelle.
pub(crate) fn est_une_dsn_mensuelle(nom: &str) -> bool {
    let Some(reste) = nom.strip_prefix("DSN_") else { return false };
    let Some(reste) = reste.strip_suffix(".dsn") else { return false };
    let c: Vec<char> = reste.chars().collect();
    let lettres = c.iter().take_while(|x| etats::lettre_ou_chiffre(**x) && !x.is_ascii_digit() && **x != '_' && !x.is_numeric()).count();
    if lettres == 0 {
        return false;
    }
    let suite = &c[lettres..];
    if suite.len() < 4 || !suite[..4].iter().all(char::is_ascii_digit) {
        return false;
    }
    let fin = &suite[4..];
    fin.is_empty() || (fin.len() > 3 && fin[..3] == ['_', 'A', 'R'] && fin[3..].iter().all(char::is_ascii_digit))
}

/// La phrase de la pastille Contrat d'un engagement qui a change depuis la version sur laquelle son contrat est parti.
pub(crate) const CONTRAT_A_REFAIRE: &str = "l'engagement a changé depuis l'envoi du contrat : un avenant est à faire signer";

/// La meme, pour un contrat signe hors du logiciel (`engagement::signe_ailleurs`) dont un terme a change.
pub(crate) const CONTRAT_AILLEURS_A_MODIFIER: &str = "un terme du contrat signé hors de PuppetPaie a changé : un avenant est à faire signer";

/// La pastille Contrat d'un engagement.
///
/// `caduc` : le contrat parti -- signe ou non -- ne dit plus ce que dit l'engagement : ses clauses ont change depuis la
/// version sur laquelle il est parti (`a_refaire_signer`, `engagement::etats::a_refaire`), ou il porte un autre numero
/// que celui que la DSN declare. Un contrat renvoye attend une NOUVELLE signature : celle de l'ancien ne compte plus.
#[allow(clippy::too_many_arguments)]
pub(crate) fn etat_signature(
    fait: Option<String>, par: Option<&'static str>, recu: Option<&String>, renvoye: bool, avenants: Option<&Vec<envois::journal::AvenantParti>>,
    a_refaire_signer: bool, ailleurs: bool,
) -> Signature {
    let recu = if renvoye { None } else { recu.cloned() };
    let s = Signature { fait, ..Signature::default() };
    let parti = s.parti();
    // Un contrat signe hors du logiciel (`ailleurs`) vaut un contrat parti : un terme qui change s'y dit par un avenant.
    let caduc = (parti || ailleurs) && a_refaire_signer;
    // L'AVENANT : le dernier parti. Pas encore revenu, il laisse la pastille orange.
    let dernier = avenants.and_then(|a| a.last()).cloned();
    let attendu = dernier.as_ref().is_some_and(|d| d.signe.is_none());
    let pourquoi = match (caduc, ailleurs) {
        (false, _) => "",
        (true, false) => CONTRAT_A_REFAIRE,
        (true, true) => CONTRAT_AILLEURS_A_MODIFIER,
    };
    Signature {
        par: if parti { par } else { None },
        recu,
        caduc,
        pourquoi,
        avenant: dernier,
        avenant_attendu: attendu,
        ..s
    }
}

// ----------------------------------------------------------- l'aiguillage

/// LES ENGAGEMENTS NON ACTIFS que `mois` montrerait (`engagement::etats::non_actifs_du_mois`), pour qu'ils se lisent :
/// le salarie, son nom, le contrat, depuis quand, et ce que leur desactivation rend a faire (`a_faire`). L'engagement se
/// lit tel qu'il est ecrit (`brut`).
fn non_actifs(ctx: &Contexte, mois: Mois) -> R<Vec<NonActif>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let mut out = Vec::new();
    for n in engagement::etats::non_actifs_du_mois(ctx, mois)? {
        let b = &*n.objet().brut;
        let cle = b.saisie.carte.salarie.as_ref().map(ToString::to_string);
        // Un salarie qu'aucune fiche ne porte se nomme par sa cle.
        let nom = cle.as_deref().map(|c| match ctx.salaries.get(c) {
            Some(f) => super::nom_a_l_ecran(f, c).trim().to_string(),
            None => c.to_string(),
        });
        let debut = Jour::lire(&engagement::debut_brut(b))?;
        let fin_ecrite = match b.saisie.carte.fin.as_deref().filter(|f| !f.is_empty()) {
            Some(f) => Some(Jour::lire(f)?),
            None => None,
        };
        let fin = match fin_ecrite {
            None => b.saisie.carte.dates().iter().map(|j| j.jour().to_string()).max(),
            Some(f) => Some(f.iso()),
        };
        let dit = match b.saisie.carte.numero_dit() {
            Some(n) => format!("contrat {}, {}", n, periode_dite(debut, fin_ecrite)),
            None => format!("contrat sans numéro, {}", periode_dite(debut, fin_ecrite)),
        };
        out.push(NonActif {
            salarie: cle,
            nom,
            numero: b.saisie.carte.numero.clone(),
            dit,
            debut: debut.iso(),
            fin,
            non_actif: n.le().to_string(),
            non_actif_dit: format!("non actif depuis le {}", socle::ecran::quand(n.le())),
            a_faire: crate::engagement::a_faire_en_phrases(&n.a_refaire(&departs)),
        });
    }
    Ok(out)
}

/// La route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::MoisAOuvrir => {
            let SansCharge {} = charge.lire()?;
            en_json(Moteur::nouveau(porte.ctx).mois_a_ouvrir()?)
        }
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

impl Dite for MoisAOuvrir {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// LES DEUX ETATS DE `mois`, servis par une meme lecture (`Moteur`) : celui de chaque engagement (`etat-par-engagement`,
/// rendu) et le bilan du mois, tels que l'etat garde d'un mois les garde (`etat_garde`).
pub(crate) fn les_deux_etats(ctx: &Contexte, mois: Mois) -> R<(Rendu, Bilan)> {
    let mut m = Moteur::nouveau(ctx);
    let lignes = m.etat_par_engagement(mois)?;
    let engagements: Vec<LigneDePage> = lignes.iter().map(LigneDePage::de).collect();
    let phrase = match engagements.len() {
        0 => format!("Aucun engagement enregistré en {}.", mois.libelle(socle::mois::Graphie::Accentuee)),
        n => format!("Engagements du mois : {}.", n),
    };
    let phrases = engagements.iter().map(phrase_de_la_ligne).collect();
    let par = EtatParEngagement { phrase, phrases, mois, engagements, non_actifs: non_actifs(ctx, mois)? };
    let bilan = m.bilan_du_mois(mois)?;
    Ok((Rendu::de(&par)?, bilan))
}

/// LE MOIS A OUVRIR (`mois-a-ouvrir`), tel que l'etat garde de l'ouverture le garde : le mois, et la reponse rendue.
pub(crate) fn le_mois_a_ouvrir(ctx: &Contexte) -> R<(Mois, Rendu)> {
    let m = Moteur::nouveau(ctx).mois_a_ouvrir()?;
    Ok((m.mois, Rendu::de(&m)?))
}

/// Ce que la DPAE d'une embauche declare, en 16 caracteres : l'identite du salarie telle que l'engagement la lit, les
/// bornes du contrat, le spectacle, la nature ; pas les taux ni les cachets. Un test de DPAE fait sur une autre empreinte
/// ne vaut plus.
fn empreinte_de_l_embauche(e: &engagement::Engagement, sal: &droit::fiche::ChampsSalarie) -> String {
    let r = |t: &str| socle::fichiers::repr(t);
    // Une case vide s'ecrit `None`, comme la fin et le spectacle.
    let o = |t: Option<&str>| match t {
        Some(x) => r(x),
        None => "None".to_string(),
    };
    // La derniere place est celle d'une case `nationalite` que la fiche n'a pas : vide, comme l'empreinte l'a toujours ecrite.
    let ident: Vec<String> = vec![
        o(sal.nir.portee()), o(sal.nom.portee()), o(sal.prenom.portee()), o(sal.date_naissance.dite().map(|j| j.iso()).as_deref()),
        o(sal.lieu_naissance.portee()), o(sal.departement_naissance.portee()), o(sal.sexe.code()), r(""),
    ];
    let fin = match e.fin {
        Some(f) => f.iso(),
        None => "None".to_string(),
    };
    let spectacle = match &e.spectacle {
        Some(s) => s.to_string(),
        None => "None".to_string(),
    };
    let brut = format!(
        "({}, {}, {}, ({}), {})",
        socle::fichiers::repr(&e.debut.iso()),
        socle::fichiers::repr(&fin),
        socle::fichiers::repr(&spectacle),
        ident.join(", "),
        socle::fichiers::repr(e.nature.cle())
    );
    sha256_hex(brut.as_bytes())[..16].to_string()
}
