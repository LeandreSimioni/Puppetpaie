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

use droit::fiche::{Lue as _, Nic};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde::Serialize;

use engagement::calcul::Paie;
use droit::contexte::{Contexte, IdSalarie};
use droit::echeances::Echeance;
use droit::natures::NumeroDOrdre;
use crate::imprevus::RangDuGeste;
use socle::date::{Jour, PeriodeOuverte};
use socle::mois::{Graphie, Mois};
use socle::paris::{InstantOuJour, Moment};
use engagement;
use engagement::etats::ARefaire;
use engagement::numeros::NumeroContrat;
use envois::etats::{self, DOCUMENTS, DPAE, DSN, SIGNATURE};
use socle::echec::{Classe, R};
use socle::echec::erreur;
use socle::fichiers::{Empreinte, EmpreinteCourte, NomDeFichier};
use socle::echec::Echec;
use crate::catalogue::{Acte, Fiche};
use crate::catalogue::Route;
use crate::catalogue::Ecran;
use crate::SansCharge;
use crate::declaration::{charge, legendes};
use socle::ecran::couleur_de_l_etat;
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use crate::depots::{Designation, EmbaucheDesignee, Quoi};
use crate::gestes::signature::ContratSigneDit;

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Lecture;

/// La route de ce module, et sa fiche ; les deux etats d'un mois se servent de l'etat garde (`etat_garde::ROUTES`).
pub const ROUTES: &[Fiche] = &[Fiche {
    route: Route::MoisAOuvrir,
    geste: "Dire quel mois ouvrir : le plus ancien dont une étape reste à faire.",
    charge: &SansCharge::CHARGE,
    exemple: "{}",
    rend: "mois (celui à ouvrir), courant, reste (les étapes qui restent)",
    refus: &[crate::catalogue::ILLISIBLE],
    pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
    acte: Acte::Lit,
    phrase: 0,
    ecran: ECRAN,
}];

pub(crate) type Paire = (IdSalarie, NumeroContrat);

// ------------------------------------------------------------------ les charges

charge! {
    /// La charge des etats d'un mois (`etat-par-engagement`, `etat-du-mois`, `mois` : `etat_garde`) : le mois.
    #[derive(Debug, Clone, Serialize)]
    pub(crate) struct ChargeMois {
        pub(crate) mois: Mois = legendes::MOIS,
    }
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
    fn couleur(self) -> &'static str {
        couleur_de_l_etat(match self {
            EtatCase::Fait => socle::ecran::EtatDEcran::Fait,
            EtatCase::AFaire => socle::ecran::EtatDEcran::AFaire,
            EtatCase::EnAttente => socle::ecran::EtatDEcran::EnAttente,
            EtatCase::Refuse => socle::ecran::EtatDEcran::Refuse,
            EtatCase::Rien => socle::ecran::EtatDEcran::Rien,
            EtatCase::Information => socle::ecran::EtatDEcran::Information,
        })
    }
}

/// LE GESTE D'UN CLIC, en routes du catalogue, celles que l'ecran et le LLM appellent : la route qu'il ouvre (`route`,
/// un apercu le plus souvent), sa charge (`charge`, en sa forme), la route de l'acte qui suit, s'il y en a un (`puis`), et
/// ce que dit le bouton (`libelle`), et son rang dans son ecran (`rang`, `RangDuGeste` : la page en tire la forme du
/// bouton). La page ouvre sa boite d'apres la route. `tester` : le depot d'une DSN, dont le test part des qu'elle est
/// ecrite.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Action {
    route: Route,
    charge: ChargeDeLAction,
    puis: Option<Route>,
    libelle: String,
    rang: RangDuGeste,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    tester: bool,
    /// La vue qu'il ouvre, quand sa route en a deux (`VueDuGeste`) ; aucune pour une route qui n'en a qu'une.
    #[serde(skip_serializing_if = "Option::is_none")]
    vue: Option<VueDuGeste>,
}

/// LA VUE QU'OUVRE UN GESTE DONT LA ROUTE EN A DEUX : l'apercu du depot (`depot-apercu`) se montre pour choisir les
/// embauches dont la DPAE s'ecrit, ou pour deposer ce qu'il designe ; celui des contrats (`signature-apercu`), pour
/// faire partir les contrats a envoyer, ou pour suivre ceux qui attendent leur signature. Chacune nomme sa route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum VueDuGeste {
    EmbauchesAChoisir,
    Depot,
    ContratsAEnvoyer,
    ContratsEnAttente,
}

impl VueDuGeste {
    /// La route dont elle est une vue.
    fn route(self) -> Route {
        match self {
            VueDuGeste::EmbauchesAChoisir | VueDuGeste::Depot => Route::DepotApercu,
            VueDuGeste::ContratsAEnvoyer | VueDuGeste::ContratsEnAttente => Route::SignatureApercu,
        }
    }
}

/// LA CHARGE D'UNE ACTION : celle d'une route qui n'a qu'une vue, ou celle d'une route a deux vues, que seul `vers` donne.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum ChargeDeLAction {
    Route(ChargeDuGeste),
    Vue(ChargeDUneVue),
}

/// LA CHARGE D'UNE ROUTE A DEUX VUES (`VueDuGeste`) : l'apercu du depot, celui des contrats. Elle n'entre dans une
/// `Action` que par `vers`, qui dit la vue ; `action` ne la prend pas.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum ChargeDUneVue {
    Signature(crate::contrats::ChargeApercuSignature),
    Depot(Designation),
}

impl From<crate::contrats::ChargeApercuSignature> for ChargeDUneVue {
    fn from(c: crate::contrats::ChargeApercuSignature) -> ChargeDUneVue {
        ChargeDUneVue::Signature(c)
    }
}

impl From<Designation> for ChargeDUneVue {
    fn from(c: Designation) -> ChargeDUneVue {
        ChargeDUneVue::Depot(c)
    }
}

/// LA CHARGE D'UN GESTE : celle de la route qu'il ouvre, en sa forme, telle que la route la lit ; celle d'une route a deux
/// vues n'en est pas (`ChargeDUneVue`).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum ChargeDuGeste {
    Mois(ChargeMois),
    Recu(crate::contrats::ChargeRecu),
    Avenant(crate::contrats::ChargeAvenant),
    FinContrat(crate::imprevus::ChargeFinContratSignaler),
    Arret(crate::imprevus::ChargeArretSignaler),
    ObligationDArret(ChargeObligationDArret),
    Connaissance(crate::obligations::ChargeConnaissance),
    Papiers(crate::imprevus::ChargeFinContratDocuments),
    Annee(ChargeAnnee),
    Faite(crate::rappels::ChargeFaite),
    Piece(crate::documents::ChargePiecePartie),
    Proposee(crate::mois::ChargeCarteProposee),
    Carte(crate::mois::ChargeCase),
    Valider(crate::engagement::ChargeValider),
    Modifier(crate::engagement::ChargeModifier),
    Retirer(crate::engagement::ChargeRetirer),
}

/// La charge du geste d'une obligation d'un arret (`accident-declare`, `attestation-declare`) : le salarie et son dernier
/// jour travaille, qui designent l'arret.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChargeObligationDArret {
    pub(crate) salarie: IdSalarie,
    pub(crate) dernier_jour: socle::date::Jour,
}

/// La charge du geste d'une echeance de l'annee (`cloture`, `export-compta`) : l'annee.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChargeAnnee {
    pub(crate) annee: socle::mois::Annee,
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
    crate::contrats::ChargeRecu => Recu,
    crate::contrats::ChargeAvenant => Avenant,
    crate::imprevus::ChargeFinContratSignaler => FinContrat,
    crate::imprevus::ChargeArretSignaler => Arret,
    ChargeObligationDArret => ObligationDArret,
    crate::obligations::ChargeConnaissance => Connaissance,
    crate::imprevus::ChargeFinContratDocuments => Papiers,
    ChargeAnnee => Annee,
    crate::rappels::ChargeFaite => Faite,
    crate::documents::ChargePiecePartie => Piece,
    crate::mois::ChargeCarteProposee => Proposee,
    crate::mois::ChargeCase => Carte,
    crate::engagement::ChargeValider => Valider,
    crate::engagement::ChargeModifier => Modifier,
    crate::engagement::ChargeRetirer => Retirer,
}

/// L'`Action` de `route` sur `charge`, la charge de la route en sa forme. Une route a deux vues n'a pas sa charge ici
/// (`ChargeDUneVue`) : son action se fait par `vers`.
pub(crate) fn action(route: Route, charge: impl Into<ChargeDuGeste>, puis: Option<Route>, libelle: String) -> Action {
    Action { route, charge: ChargeDeLAction::Route(charge.into()), puis, libelle, rang: RangDuGeste::Second, tester: false, vue: None }
}

impl Action {
    /// La meme, le geste principal de son ecran (celui d'un rappel).
    pub(crate) fn principale(self) -> Action {
        Action { rang: RangDuGeste::Principal, ..self }
    }

    /// Envoie-t-elle quelque chose ? Sa route, ou celle qu'elle ouvre ensuite, engage (`envois_en_fond::part_en_fond`) ;
    /// l'Historique, une boite qui ne fait que montrer, n'envoient rien.
    fn envoie(&self) -> bool {
        crate::envois_en_fond::part_en_fond(self.route) || self.puis.is_some_and(crate::envois_en_fond::part_en_fond)
    }
}

/// L'`Action` qui ouvre la vue `vue` de sa route, sur `charge`.
pub(crate) fn vers(vue: VueDuGeste, charge: impl Into<ChargeDUneVue>, puis: Option<Route>, libelle: String) -> Action {
    Action { route: vue.route(), charge: ChargeDeLAction::Vue(charge.into()), puis, libelle, rang: RangDuGeste::Second, tester: false, vue: Some(vue) }
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
/// moitie faite, la moitie qui attend le retour). Un envoi de son etape en vol prend son geste (`Groupe::pendant_les_envois`).
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
        Groupe {
            etat,
            compte,
            compte_texte,
            quoi,
            phrase: Some(phrase),
            echeance,
            action,
            colonnes: Vec::new(),
            couleur: etat.couleur(),
            coupee: etat == EtatCase::EnAttente,
        }
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

    /// UN ENVOI DE SON ETAPE EN VOL CE MOIS (`envois`) : le geste du groupe couvre l'etape entiere (la boite du mois, ou
    /// tout ce qui reste se coche d'abord), et le refaire attendrait cet envoi puis renverrait ce qui en est parti ; le
    /// groupe dit donc « 1 contrat en cours d'envoi… » a la place de son geste, en entier, quoi que l'envoi emporte. Un
    /// geste qui n'envoie rien (l'Historique) reste. Rend ce qui part (« 1 contrat ») quand l'envoi a pris le geste.
    fn pendant_les_envois(&mut self, cle: QuelleEtape, envois: &crate::envois_en_fond::EnvoisDuMois) -> Option<String> {
        if self.etat == EtatCase::Fait || !self.action.as_ref().is_some_and(Action::envoie) {
            return None;
        }
        let quoi = envois.de_l_etape(cle)?;
        let en_cours = crate::envois_en_fond::phrase_en_cours(&quoi);
        let colonnes = std::mem::take(&mut self.colonnes);
        *self = Groupe { colonnes, ..Groupe::de(EtatCase::EnAttente, None, String::new(), en_cours, self.echeance.clone(), None) };
        Some(quoi)
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
    phrase: Phrase,
    colonnes: Vec<String>,
    pub(crate) etapes: Vec<Etape>,
    /// Le cadre sous le mois : son dernier fait et sa prochaine etape (`OuEnEstLeMois`).
    ou_en_est: OuEnEstLeMois,
}

/// OU EN EST LE MOIS, le cadre que la page pose sous lui : son titre, le dernier fait du mois tel que l'Historique le dit
/// (la derniere ligne du journal de ce mois, son acte et ce qui en est revenu : `historique_mois::fait_du_journal`), et
/// la prochaine etape, telle que la colonne des etapes la montre (la premiere case a faire
/// de l'etape suivante, sinon la premiere qui attend : `phrase_du_groupe`). Lu de l'etat du mois, jamais de la page : il
/// est juste apres reouverture, et le meme en ligne de commande. Ce qui part en fond s'y ajoute a la lecture
/// (`etat_garde::DuMois::envois`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct OuEnEstLeMois {
    titre: String,
    /// Le dernier fait, dans le ton de ce qui en est revenu (`historique_mois::fait_du_journal`).
    #[serde(skip_serializing_if = "Option::is_none")]
    dernier: Option<crate::Encart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prochaine: Option<String>,
    /// La case d'ou vient `prochaine` (son etape, son rang) : un envoi en cours qui prend son geste dit la prochaine
    /// etape a sa place (`Bilan::pendant_les_envois`).
    #[serde(skip)]
    prochaine_de: Option<(QuelleEtape, usize)>,
}

impl OuEnEstLeMois {
    /// Le cadre de `mois` : `lignes`, le journal (`envois::journal::lu`), ses etapes deja faites.
    fn de(ctx: &Contexte, mois: Mois, lignes: &[envois::journal::Ligne], etapes: &[Etape]) -> OuEnEstLeMois {
        let du_mois: Vec<&envois::journal::Ligne> = envois::journal::filtrer(lignes, None, Some(mois), None).collect();
        // Le plus recent ; a la meme heure, le reel devant son test (l'ordre de l'Historique).
        let dernier = du_mois.iter().max_by(|a, b| (a.quand(), a.mode()).cmp(&(b.quand(), b.mode()))).map(|l| crate::historique_mois::fait_du_journal(ctx, l, lignes));
        let groupes = || etapes.iter().flat_map(|e| e.groupes.iter().enumerate().map(move |(i, g)| (e, i, g)));
        let a_faire = groupes().find(|(e, _, g)| e.suivante && matches!(g.etat, EtatCase::AFaire | EtatCase::Refuse)).and_then(|(e, i, g)| g.phrase.clone().map(|p| ((e.cle, i), p)));
        let attend = groupes().find(|(_, _, g)| g.etat == EtatCase::EnAttente).and_then(|(e, i, g)| g.phrase.clone().map(|p| ((e.cle, i), p)));
        let (prochaine, prochaine_de) = match (a_faire, attend) {
            (Some((e, p)), _) => (Some(format!("Prochaine étape — {}.", p)), Some(e)),
            (None, Some((e, p))) => (Some(format!("En attente — {}.", p)), Some(e)),
            (None, None) => (None, None),
        };
        OuEnEstLeMois { titre: format!("Où en est {}", mois.libelle(Graphie::Accentuee)), dernier, prochaine, prochaine_de }
    }
}

/// `mois-a-ouvrir` : le mois a ouvrir, celui du jour, et les etapes qui restent au premier.
#[derive(Debug, Serialize)]
struct MoisAOuvrir {
    phrase: Phrase,
    mois: Mois,
    courant: Mois,
    reste: Vec<QuelleEtape>,
}

/// `etat-par-engagement` : la phrase du mois, celle de chaque engagement (ses carres et sa frise), les lignes que la page
/// lit, et les engagements non actifs du mois. LA BOITE DES DPAE : son titre, ses deux notes (`dpae_notes` : ce qui est
/// coche d'avance ; l'heure, et le test avant le reel), le geste qui envoie les embauches cochees (`dpae_envoyer` :
/// l'apercu du depot, dont le test part tout de suite ; sa case `engagements` prend la `part` de chaque engagement coche),
/// aucun quand rien ne se coche, et ce que son bouton dit quand rien n'est coche (`dpae_rien`).
#[derive(Debug, Serialize)]
pub(crate) struct EtatParEngagement<'a> {
    phrase: Phrase,
    phrases: Vec<Phrase>,
    mois: Mois,
    engagements: Vec<LigneDePage<'a>>,
    non_actifs: &'a [NonActif],
    dpae_titre: &'static str,
    dpae_notes: [&'static str; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    dpae_envoyer: Option<Action>,
    dpae_rien: &'static str,
}

impl Dite for EtatParEngagement<'_> {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Bilan {
    /// La phrase du bilan, celle de l'ecran du mois.
    pub(crate) fn phrase(&self) -> &Phrase {
        &self.phrase
    }

    /// DES ENVOIS DE CE MOIS SONT EN COURS (`envois_en_fond`, la fenetre) : chaque case d'une etape dont un envoi est en
    /// vol dit « 1 contrat en cours d'envoi… » a la place de son geste, et la prochaine etape aussi quand c'est elle
    /// (`Groupe::pendant_les_envois`). Un second clic ne refait pas partir ce qui part.
    fn pendant_les_envois(&mut self, envois: &crate::envois_en_fond::EnvoisDuMois) {
        for e in self.etapes.iter_mut() {
            let cle = e.cle;
            for (i, g) in e.groupes.iter_mut().enumerate() {
                let Some(quoi) = g.pendant_les_envois(cle, envois) else { continue };
                if self.ou_en_est.prochaine_de == Some((cle, i)) {
                    self.ou_en_est.prochaine = Some(format!("En cours d'envoi — {}.", quoi));
                }
            }
        }
    }
}

/// La phrase d'un engagement dans `etat-par-engagement` : qui, son contrat, puis ce que disent ses carres et sa frise.
fn phrase_de_la_ligne(l: &LigneDePage) -> Phrase {
    let qui = match (l.nom, l.numero) {
        (Some(n), Some(c)) => format!("{} ({})", n, c),
        (Some(n), None) => n.to_string(),
        (None, Some(c)) => format!("contrat {}", c),
        (None, None) => "engagement à compléter".to_string(),
    };
    let dit: Vec<&str> = l.carres.iter().map(|c| c.phrase.as_str()).chain(l.versions.map(|f| f.phrase.as_str())).collect();
    socle::phrase!("{} : {}", qui, dit.join(" ; "))
}

/// Un engagement non actif du mois (`non_actifs`).
#[derive(Debug, Serialize)]
struct NonActif {
    salarie: Option<IdSalarie>,
    nom: Option<String>,
    numero: Option<NumeroContrat>,
    contrat: PeriodeOuverte,
    non_actif: String,
    non_actif_dit: String,
    /// Le mot que sa carte porte (`non_actif_dit` en dit le jour).
    mot: &'static str,
    dit: String,
    a_faire: Vec<String>,
}

// ------------------------------------------------------------ les groupes

/// Les quatre etapes du mois, dans l'ordre de l'habitude (aucun texte n'impose d'ordre).
const ORDRE: [QuelleEtape; 4] = [QuelleEtape::Dpae, QuelleEtape::Signature, QuelleEtape::Dsn, QuelleEtape::Documents];

/// L'echeance d'un depot de DSN : avant l'instant `limite` (midi du jour de l'echeance ; minuit qui finit sa veille pour
/// un annule-et-remplace), d'apres `source`.
fn echeance_dsn(limite: Moment, source: &str) -> Echeance {
    Echeance { quand: InstantOuJour::Instant(limite), mots: droit::echeances::Tournure::AvantLe, source: source.into() }
}

/// Faire signer les contrats du mois : l'apercu, puis la demande. `salaries` : ceux dont les contrats se cochent d'abord
/// (le detail d'un engagement) ; aucun, tous ceux a envoyer. `signee` : la signature de l'employeur est posee
/// (`contrats::signature_posee`) ; sans elle, rien ne part se faire signer, et le libelle dit ce qui est a faire.
fn faire_signer(mois: Mois, salaries: Option<Vec<IdSalarie>>, signee: bool) -> Action {
    let libelle = if signee { "Demander la signature…" } else { SIGNATURE_A_POSER };
    vers(VueDuGeste::ContratsAEnvoyer, crate::contrats::ChargeApercuSignature { mois, salaries }, Some(Route::DemanderSignature), libelle.into())
}

/// Le geste qui fait signer, sans signature de l'employeur posee : ce qui est a faire d'abord ; sa boite dit ou la poser
/// et offre d'imprimer.
const SIGNATURE_A_POSER: &str = "Signature de l'employeur à poser…";

/// LES GESTES QUI REFONT UN ENVOI PARTI EN FOND QUI N'EST PAS ALLE AU BOUT (`envois_en_fond::EnvoiQuiPart`) : la boite d'ou
/// il est parti, rouverte ; ce qui n'est pas parti y est encore a faire partir.
pub(crate) fn refaire_la_signature(mois: Mois) -> Action {
    vers(VueDuGeste::ContratsAEnvoyer, crate::contrats::ChargeApercuSignature { mois, salaries: None }, Some(Route::DemanderSignature), REFAIRE_L_ENVOI.into())
}

/// Les documents du mois, rouverts pour un envoi qui n'est pas alle au bout.
pub(crate) fn refaire_les_documents(mois: Mois) -> Action {
    envoyer_documents(mois, REFAIRE_L_ENVOI.into())
}

/// Le depot designe, rouvert pour un depot qui n'est pas alle au bout.
pub(crate) fn refaire_le_depot(d: Designation) -> Action {
    vers(VueDuGeste::Depot, d, Some(Route::Deposer), "Refaire le dépôt…".into())
}

const REFAIRE_L_ENVOI: &str = "Refaire l'envoi…";

/// Les contrats partis du mois, recus ou a relancer : l'apercu, puis noter chacun recu.
fn contrats_en_attente(mois: Mois) -> Action {
    vers(VueDuGeste::ContratsEnAttente, crate::contrats::ChargeApercuSignature { mois, salaries: None }, Some(Route::ContratRecu), "Reçus ou à relancer…".into())
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
    vers(VueDuGeste::EmbauchesAChoisir, Designation::du_mois(Quoi::Dpae, mois), Some(Route::Deposer), "Envoyer les DPAE…".into())
}

/// La DSN du mois a ecrire, ou a reecrire : l'apercu du depot (qui l'ecrit), puis le depot, dont le test part des
/// qu'elle est ecrite (`tester`).
pub(crate) fn deposer_dsn(mois: Mois, nic: Option<&Nic>, libelle: String) -> Action {
    let mut charge = Designation::du_mois(Quoi::Dsn, mois);
    charge.etablissement = nic.cloned();
    Action { tester: true, ..vers(VueDuGeste::Depot, charge, Some(Route::Deposer), libelle) }
}

/// Le geste d'un carre Contrat en attente : noter revenu signe (`recu`) le contrat de l'engagement `p`, ou l'avenant
/// `avenant` (son numero) ; la route `contrat-recu` rend la phrase qui le dit et de quoi l'annuler. Rien pour un
/// engagement qui ne dit pas encore son salarie ou son premier jour.
fn noter_recu(p: &Vue, mois: Mois, libelle: &str, avenant: Option<engagement::numeros::NumeroDAvenant>, recu: bool) -> Option<Action> {
    let charge = crate::contrats::ChargeRecu { mois, salarie: p.salarie.clone()?, debut: p.bornes?.debut(), numero: p.numero.clone(), avenant, recu };
    Some(action(Route::ContratRecu, charge, None, libelle.into()))
}

/// L'avenant du contrat de l'engagement `p` : l'apercu, puis sa demande de signature. Rien pour un engagement qui ne
/// dit pas encore son salarie ou son premier jour.
fn avenant(p: &Vue, mois: Mois) -> Option<Action> {
    let charge = crate::contrats::ChargeAvenant { salarie: p.salarie.clone()?, debut: p.bornes?.debut(), numero: p.numero.clone(), mois: Some(mois) };
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
/// dit une fois en tete (`tete`), sans redire ses etapes qui n'ont rien a faire.
fn phrase_du_mois(tete: Option<String>, etapes: &[Etape]) -> Phrase {
    let cases = etapes.iter().flat_map(|e| &e.groupes).filter_map(|g| g.phrase.clone());
    let morceaux: Vec<String> = tete.into_iter().chain(cases).collect();
    socle::phrase!("{}.", morceaux.join(". "))
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
/// CE QU'UNE LIGNE DIT D'UN ENGAGEMENT QUI N'EST PAS VALIDE : il lui manque un fait (`ACompleter`), il est complet et
/// personne ne l'a encore valide (`AValider` : rien ne part d'un engagement en saisie, `routes/depots.rs`,
/// `routes/documents.rs`), ou la ligne n'a rien a faire pour lui.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Attente {
    ACompleter,
    AValider,
    Aucune,
}

/// OU EN EST LE DEPOT D'UNE DECLARATION ECRITE (`chemin_de_depot`) : la paie a change depuis son ecriture (la DSN se
/// refait), ou l'etat de son test (`envois::etats::EtatDuTest`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EtapeDuDepot {
    PaieChangee,
    Test(etats::EtatDuTest),
}

/// Le point d'un engagement sur la ligne DPAE. `RienADeclarer` : un contrat commence avant le logiciel.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PointDpae {
    Fait,
    AFaire,
    Refuse,
    ACompleter,
    AValider,
    RienADeclarer,
}

/// Le point d'un engagement sur la ligne Contrats. `RienASigner` : un contrat commence avant le logiciel, ou signe
/// ailleurs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PointContrat {
    Fait,
    AEnvoyer,
    ARefaire,
    EnAttente,
    ACompleter,
    AValider,
    RienASigner,
}

/// Le point d'un engagement sur la ligne Documents.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PointDocuments {
    Fait,
    AEnvoyer,
    ARenvoyer,
    ACompleter,
    AValider,
}

impl PointDpae {
    fn attente(self) -> Attente {
        match self {
            PointDpae::ACompleter => Attente::ACompleter,
            PointDpae::AValider => Attente::AValider,
            PointDpae::Fait | PointDpae::AFaire | PointDpae::Refuse | PointDpae::RienADeclarer => Attente::Aucune,
        }
    }
}

impl PointContrat {
    fn attente(self) -> Attente {
        match self {
            PointContrat::ACompleter => Attente::ACompleter,
            PointContrat::AValider => Attente::AValider,
            PointContrat::Fait | PointContrat::AEnvoyer | PointContrat::ARefaire | PointContrat::EnAttente | PointContrat::RienASigner => Attente::Aucune,
        }
    }
}

impl PointDocuments {
    fn attente(self) -> Attente {
        match self {
            PointDocuments::ACompleter => Attente::ACompleter,
            PointDocuments::AValider => Attente::AValider,
            PointDocuments::Fait | PointDocuments::AEnvoyer | PointDocuments::ARenvoyer => Attente::Aucune,
        }
    }
}

#[derive(Clone, Copy)]
struct Point {
    dpae: PointDpae,
    contrat: PointContrat,
    documents: PointDocuments,
    dsn: Option<usize>,
}

impl Point {
    fn de(p: &Vue, dsn: Option<usize>) -> Point {
        let debut = p.contrat.debut();
        // UN CONTRAT COMMENCE AVANT LE LOGICIEL (`engagement::stock::commence_avant_le_logiciel`) : sa DPAE et son contrat
        // se sont faits ailleurs ; ici, rien a declarer ni a signer.
        let anterieur = p.contrat.anterieur();
        // UN ENGAGEMENT A COMPLETER, jamais valide (`ligne_a_completer`) : rien n'est parti de lui, et tout ce qui partira
        // attend qu'il soit complet. Il reste a faire, en orange, pour qu'on ne l'oublie pas.
        if p.a_completer.is_some() {
            let spectacle = p.spectacle.is_some();
            return Point {
                dpae: match debut {
                    _ if anterieur => PointDpae::RienADeclarer,
                    DebutDansLeMois::CeMois => PointDpae::ACompleter,
                    DebutDansLeMois::Avant => PointDpae::Fait,
                },
                contrat: match debut {
                    _ if anterieur => PointContrat::RienASigner,
                    DebutDansLeMois::CeMois if spectacle => PointContrat::ACompleter,
                    DebutDansLeMois::CeMois | DebutDansLeMois::Avant => PointContrat::Fait,
                },
                documents: PointDocuments::ACompleter,
                dsn,
            };
        }
        // UN ENGAGEMENT EN SAISIE, complet : rien ne part de lui tant qu'il n'est pas valide ; chaque ligne qui
        // l'attend le dit.
        let ecrit = p.contrat.ecrit();
        let ailleurs = p.contrat.ailleurs();
        if !p.valide() {
            return Point {
                dpae: match debut {
                    _ if anterieur => PointDpae::RienADeclarer,
                    DebutDansLeMois::CeMois => PointDpae::AValider,
                    DebutDansLeMois::Avant => PointDpae::Fait,
                },
                contrat: match debut {
                    _ if anterieur || ailleurs => PointContrat::RienASigner,
                    DebutDansLeMois::CeMois if ecrit => PointContrat::AValider,
                    DebutDansLeMois::CeMois | DebutDansLeMois::Avant => PointContrat::Fait,
                },
                documents: PointDocuments::AValider,
                dsn,
            };
        }
        // LA DPAE A DEPOSER est celle que dit la liste des DPAE a faire (`Moteur::embauches_a_declarer`, `a_declarer`).
        let d = &p.etats.dpae;
        let dpae = match debut {
            _ if anterieur => PointDpae::RienADeclarer,
            DebutDansLeMois::Avant => PointDpae::Fait,
            DebutDansLeMois::CeMois if d.refusee => PointDpae::Refuse,
            DebutDansLeMois::CeMois if d.a_declarer => PointDpae::AFaire,
            DebutDansLeMois::CeMois => PointDpae::Fait,
        };
        let s = &p.etats.signature;
        // Le contrat se fait signer au mois de l'embauche ; un avenant, a chaque mois du contrat : tant qu'il n'est pas
        // revenu signe, chacun de ses mois le dit.
        // Un contrat signe hors du logiciel ne se fait pas signer ici, ses avenants si.
        let contrat = match debut {
            DebutDansLeMois::Avant if (ecrit || ailleurs) && s.caduc => PointContrat::ARefaire,
            DebutDansLeMois::CeMois if ailleurs && s.caduc => PointContrat::ARefaire,
            DebutDansLeMois::Avant if (ecrit || ailleurs) && s.avenant_attendu => PointContrat::EnAttente,
            DebutDansLeMois::CeMois if ailleurs && s.avenant_attendu => PointContrat::EnAttente,
            _ if anterieur || ailleurs => PointContrat::RienASigner,
            DebutDansLeMois::Avant => PointContrat::Fait,
            DebutDansLeMois::CeMois if !ecrit => PointContrat::Fait,
            DebutDansLeMois::CeMois if !s.parti() => PointContrat::AEnvoyer,
            DebutDansLeMois::CeMois if s.caduc => PointContrat::ARefaire,
            DebutDansLeMois::CeMois if s.recu.is_none() || s.avenant_attendu => PointContrat::EnAttente,
            DebutDansLeMois::CeMois => PointContrat::Fait,
        };
        let o = &p.etats.documents;
        let documents = if o.caduc {
            PointDocuments::ARenvoyer
        } else if o.parti() {
            PointDocuments::Fait
        } else {
            PointDocuments::AEnvoyer
        };
        Point { dpae, contrat, documents, dsn }
    }

    /// Sa vague : ce qui reste a faire de notre cote. Une DPAE partie l'est, revenue refusee ou non ; un contrat parti
    /// l'est, signe ou non (`None`).
    fn vague(&self) -> (bool, Option<PointContrat>, PointDocuments, Option<usize>) {
        let contrat = matches!(self.contrat, PointContrat::AEnvoyer | PointContrat::ARefaire).then_some(self.contrat);
        (self.dpae == PointDpae::AFaire, contrat, self.documents, self.dsn)
    }

    /// Combien de ses lignes sont faites de notre cote : l'ordre des colonnes, la plus avancee en G.
    fn avance(&self, dsn_fait: &[bool]) -> usize {
        [
            !matches!(self.dpae, PointDpae::AFaire | PointDpae::ACompleter | PointDpae::AValider),
            matches!(self.contrat, PointContrat::Fait | PointContrat::EnAttente | PointContrat::RienASigner),
            self.documents == PointDocuments::Fait,
            self.dsn.is_some_and(|k| dsn_fait.get(k) == Some(&true)),
        ]
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
    let mut vagues: Vec<((bool, Option<PointContrat>, PointDocuments, Option<usize>), Vec<usize>)> = Vec::new();
    for (i, p) in points.iter().enumerate() {
        match vagues.iter_mut().find(|(v, _)| *v == p.vague()) {
            Some((_, m)) => m.push(i),
            None => vagues.push((p.vague(), vec![i])),
        }
    }
    let mut cols: Vec<(Option<usize>, Vec<usize>)> = vagues.into_iter().map(|(v, m)| (v.3, m)).collect();
    cols.sort_by_key(|(_, m)| std::cmp::Reverse(points[m[0]].avance(dsn_fait)));
    if let Some((dsn, g)) = cols.first_mut() {
        let (refusees, reste): (Vec<usize>, Vec<usize>) = g.iter().partition(|i| points[**i].dpae == PointDpae::Refuse);
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
fn cases_de_la_ligne<P: PartialEq>(cols: &[Colonne], point: impl Fn(&Colonne) -> P) -> Vec<(P, Vec<usize>, Vec<String>, Option<usize>)> {
    let mut out: Vec<(P, Vec<usize>, Vec<String>, Option<usize>)> = Vec::new();
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

/// Le point d'une colonne sur une ligne : le premier etat de `ordre` qu'un de ses engagements porte ; sinon le point que
/// tous ses engagements partagent (rien a faire ici pour chacun) ; sinon, ses engagements ne partageant rien qui reste a
/// faire, ou la colonne n'en ayant aucun, `fait`.
fn point_de<P: Copy + PartialEq>(c: &Colonne, points: &[Point], ligne: fn(&Point) -> P, ordre: &[P], fait: P) -> P {
    if let Some(o) = ordre.iter().find(|o| c.membres.iter().any(|i| ligne(&points[*i]) == **o)) {
        return *o;
    }
    let mut tous = c.membres.iter().map(|i| ligne(&points[*i]));
    match tous.next() {
        Some(p) if tous.all(|q| q == p) => p,
        Some(_) | None => fait,
    }
}

// ------------------------------------------------------------ petits outils

/// Le nom d'un etablissement dans une case : sa commune (celle de la fiche entreprise pour le principal), sinon son NIC.
fn nom_de_l_etablissement(ent: &droit::fiche::ChampsEntreprise, nic: &Nic) -> String {
    let commune = if Some(nic) == ent.nic_principal() {
        ent.commune_affichage.remplie().or(ent.commune.portee()).map(|c| c.trim().to_string())
    } else {
        ent.secondaires().into_iter().find(|e| e.nic.dite() == Some(nic)).and_then(|e| e.commune_affichage.remplie().or(e.commune.portee()).map(|c| c.trim().to_string()))
    };
    match commune {
        Some(c) => c,
        None => format!("NIC {}", nic),
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
    let incomplete = engagement::saisie_incomplete(&e);
    Echec { classe: if incomplete { Some(Classe::SaisieIncomplete) } else { None }, ..e }
}

/// Quand le contrat (ou son dernier avenant) de l'engagement `numero` du salarie `cle` est parti (`departs`) ; rien s'il
/// n'est pas parti.
pub(crate) fn contrat_parti_le(departs: &engagement::departs::Departs, cle: &droit::contexte::IdSalarie, numero: Option<&engagement::numeros::NumeroContrat>) -> Option<InstantOuJour> {
    let n = numero?;
    departs.de(cle, n).iter().rev().find(|d| d.envoi == engagement::departs::Envoi::Contrat).map(|d| d.le)
}

/// Ce qui est a refaire pour l'engagement `e` : le present compare a la version sur laquelle chaque envoi est parti
/// (`engagement::etats::a_refaire`) ; rien pour un engagement en saisie.
pub(crate) fn a_refaire_de(ctx: &Contexte, e: &engagement::etats::Etat, departs: &engagement::departs::Departs) -> R<Vec<ARefaire>> {
    Ok(e.valide().map(|v| engagement::etats::a_refaire(ctx, v, departs)).transpose()?.into_iter().flatten().collect())
}

/// Les contrats de `mois` dont l'avenant est a faire signer : `(cle du salarie, numero)` des engagements dont les clauses
/// ont change depuis la version sur laquelle le contrat est parti (`a_refaire_parmi`, les contrats seuls).
pub(crate) fn avenants_a_faire(ctx: &Contexte, mois: Mois) -> R<HashSet<(droit::contexte::IdSalarie, engagement::numeros::NumeroContrat)>> {
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
        if let (Some(cle), Some(numero)) = (&b.saisie.carte.salarie, &b.saisie.carte.numero) {
            out.insert((cle.clone(), numero.clone()));
        }
    }
    Ok(out)
}

/// La date d'embauche que la DPAE partie a declaree, quand elle n'est plus celle de l'engagement (`ARefaire::Dpae`).
pub(crate) fn debut_declare(refaire: &[ARefaire]) -> Option<Jour> {
    refaire.iter().find_map(|a| match a {
        ARefaire::Dpae { debut_declare, .. } => *debut_declare,
        _ => None,
    })
}

/// L'etablissement (NIC) dont releve l'engagement `e` : celui de la fiche de son salarie, ou celui que l'engagement dit
/// (`etablissement`) ; aucun pour une carte qui ne nomme pas encore son salarie. Une fiche qui manque, un etablissement
/// qui ne se rattache pas, se refusent.
pub(crate) fn nic_de_l_engagement(ctx: &Contexte, e: &engagement::etats::Etat) -> R<Option<Nic>> {
    nic_de_l_engagement_ecrit(ctx, &e.objet().brut)
}

/// `nic_de_l_engagement`, sur l'engagement tel qu'il est ecrit.
pub(crate) fn nic_de_l_engagement_ecrit(ctx: &Contexte, brut: &engagement::stock::Ecrit) -> R<Option<Nic>> {
    let Some(cle) = brut.saisie.carte.salarie.as_ref() else { return Ok(None) };
    let mut fiche = droit::fiche::SalarieDuContrat::de(ctx.fiche_de(cle)?);
    if let Some(n) = &brut.saisie.carte.etablissement {
        fiche.etablissement = Some(n.clone());
    }
    Ok(Some(droit::bareme::etablissement_de(&ctx.entreprise, &fiche)?))
}

/// La DSN de `mois` est-elle a refaire ? Une DSN est celle d'un etablissement (`derniere` : sa ligne du journal, son NIC
/// en `detail.etablissement`, le principal s'il est tu) : seuls comptent les engagements qui en relevent
/// (`nic_de_l_engagement`). La paie de l'un d'eux a change depuis la version sur laquelle elle est partie
/// (`refaire` : `Moteur::a_refaire_du_mois`), ou ils ne sont plus ceux qu'elle a emportes (`versions`) : un engagement ajoute, ou retire.
pub(crate) fn dsn_a_refaire(ctx: &Contexte, mois: Mois, derniere: &envois::journal::Ligne, etats: &[engagement::etats::Etat], refaire: &mut dyn FnMut(&engagement::etats::Etat) -> R<Rc<Vec<ARefaire>>>) -> R<bool> {
    // Une ligne qui ne dit pas ce qu'elle a emporte ne se compare a rien.
    Ok(derniere.versions().map(|emportes| emportes_a_refaire(ctx, mois, derniere, emportes, etats, refaire)).transpose()?.is_some_and(|oui| oui))
}

fn emportes_a_refaire(
    ctx: &Contexte,
    mois: Mois,
    derniere: &envois::journal::Ligne,
    emportes: &[envois::journal::Version],
    etats: &[engagement::etats::Etat],
    refaire: &mut dyn FnMut(&engagement::etats::Etat) -> R<Rc<Vec<ARefaire>>>,
) -> R<bool> {
    let nic = match derniere.etablissement() {
        Some(n) => n.clone(),
        None => ctx.entreprise.nic.exigee("nic")?.clone(),
    };
    let mut siens: Vec<&engagement::etats::Etat> = Vec::new();
    for e in etats {
        if nic_de_l_engagement(ctx, e)?.as_ref() == Some(&nic) {
            siens.push(e);
        }
    }
    for e in &siens {
        if refaire(e)?.iter().any(|a| matches!(a, ARefaire::Dsn { mois: m, .. } if *m == mois)) {
            return Ok(true);
        }
    }
    // Les corrections de mois passes que la ligne porte ne sont pas des engagements de ce mois.
    let mut emportes: Vec<(&IdSalarie, &NumeroContrat)> = emportes.iter().filter(|v| v.mois.is_none()).map(|v| (&v.salarie, &v.numero)).collect();
    let mut presents: Vec<(&IdSalarie, &NumeroContrat)> = siens
        .iter()
        .filter(|e| e.valide().is_some())
        .filter_map(|e| {
            let b = &*e.objet().brut;
            Some((b.saisie.carte.salarie.as_ref()?, b.saisie.carte.numero.as_ref()?))
        })
        .collect();
    emportes.sort();
    presents.sort();
    Ok(emportes != presents)
}

/// UNE CORRECTION PORTEE, telle que l'empreinte de la saisie la retient (`Moteur::portees`) : le mois corrige, le
/// salarie, le contrat et la version. Elle s'ecrit en liste.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Portee(Mois, String, Option<NumeroContrat>, engagement::stock::VersionDeLEngagement);

/// Ce dont l'empreinte d'une saisie qui porte des corrections se calcule (`Moteur::empreinte_saisie_avec`).
#[derive(Serialize)]
struct SaisieEtCorrections<'a> {
    saisie: &'a Empreinte,
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
/// Une DSN qui ne se lit pas se refuse ; un numero d'ordre hors de sa forme, la lecture de la DSN le laisse vide.
pub(crate) fn ordre_de_depot(nom: &str, chemin: &Path) -> R<OrdreDeDepot> {
    use std::cmp::Reverse;
    if !a_l_extension(nom, ".dsn") {
        return Ok((0, None, String::new(), Reverse(None), Reverse(nom.to_string())));
    }
    let apercu = declarations::dsn_lue::lire(&crate::depots::lire_octets(chemin)?, Some(&NomDeFichier::du(chemin)?))?;
    let lu = apercu.ordre;
    if apercu.est_signalement() {
        return Ok((1, lu, nom.to_string(), Reverse(None), Reverse(String::new())));
    }
    let (etablissement, du_nom) = sans_annule_et_remplace(nom);
    Ok((0, None, etablissement, Reverse(lu.or(du_nom)), Reverse(String::new())))
}

/// La cle de rangement d'un fichier deposable (`ordre_de_depot`) : mensuelle ou signalement, l'ordre d'un signalement,
/// l'etablissement, l'ordre d'une mensuelle (le plus grand d'abord), le nom d'une DPAE (le plus grand d'abord).
pub(crate) type OrdreDeDepot = (u8, Option<NumeroDOrdre>, String, std::cmp::Reverse<Option<NumeroDOrdre>>, std::cmp::Reverse<String>);

/// `DSN_<Mois>[_<NIC>]_AR<n>.dsn` -> (`DSN_<Mois>[_<NIC>].dsn`, Some(n)) ; un autre nom reste tel quel.
fn sans_annule_et_remplace(nom: &str) -> (String, Option<NumeroDOrdre>) {
    if let Some(corps) = nom.strip_suffix(".dsn").or_else(|| nom.strip_suffix(".DSN")) {
        if let Some(i) = corps.rfind("_AR") {
            // Ce qui suit `_AR` n'est pas un numero d'ordre : le nom n'est pas celui d'un annule-et-remplace.
            if let Some(n) = NumeroDOrdre::lu(&corps[i + 3..]) {
                return (format!("{}.dsn", &corps[..i]), Some(n));
            }
        }
    }
    (nom.to_string(), None)
}

/// Le nom de fichier porte-t-il l'extension `ext` (`.dsn`), a la casse pres ? Un nom sans extension n'en porte aucune.
fn a_l_extension(nom: &str, ext: &str) -> bool {
    nom.rfind('.').filter(|&i| i > 0 && i + 1 < nom.len()).is_some_and(|i| nom[i..].eq_ignore_ascii_case(ext))
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
    /// L'instant ou chaque contrat est revenu signe.
    pub(crate) recus: HashMap<etats::Paire, Moment>,
    pub(crate) renvoyes: HashSet<etats::Paire>,
    pub(crate) avenants: HashMap<etats::Paire, Vec<envois::journal::AvenantParti>>,
    /// Comment chaque contrat est parti la derniere fois.
    pub(crate) par: HashMap<etats::Paire, documents::ModeDeRemise>,
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
    place: engagement::PlaceDuMois,
    chevrons: super::mois::Chevrons,
    depuis: Mois,
    jusqu_a: Mois,
    repris: bool,
    ecrit: bool,
    ailleurs: bool,
    dossier: Option<ContratSigneDit>,
    anterieur: bool,
}

/// Un engagement a completer ne dit que s'il commence ce mois-ci (`DebutDansLeMois`) : sa fin n'est pas sure. Ses dates
/// sont les cases de sa saisie, chacune facultative (`engagement::Assemble`) : son debut lu (`Carte::debut_lu`) et sa
/// fin ecrite ; une carte a completer peut ecrire une fin avant son debut, c'est alors ce qui lui manque.
#[derive(Debug, Clone, Serialize)]
struct ContratACompleter {
    debut: DebutDansLeMois,
    debut_lu: Option<Jour>,
    fin_ecrite: Option<Jour>,
    repris: bool,
    anterieur: bool,
}

/// OU COMMENCE LE CONTRAT, AU MOIS DE L'ETAPE : ce mois-ci (son embauche : la DPAE, le contrat a signer), ou avant. Ce
/// qu'en sait un engagement a completer, dont la fin n'est pas sure ; un engagement complet le tire de sa place
/// (`PlaceDuMois`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DebutDansLeMois {
    CeMois,
    Avant,
}

impl ContratDit {
    /// Ou commence le contrat, au mois de l'etape : sa place dans le mois (`PlaceDuMois`), ou ce qu'en dit un engagement
    /// a completer, qui n'a pas de place sure.
    fn debut(&self) -> DebutDansLeMois {
        match self {
            ContratDit::Complet(c) => match c.place {
                engagement::PlaceDuMois::Seul | engagement::PlaceDuMois::Premier => DebutDansLeMois::CeMois,
                engagement::PlaceDuMois::Dernier | engagement::PlaceDuMois::Milieu => DebutDansLeMois::Avant,
            },
            ContratDit::ACompleter(c) => c.debut,
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
    avant: Jour,
    vaut: bool,
    phrase: String,
}

/// LA DPAE D'UN ENGAGEMENT : a declarer (la liste des DPAE a faire), partie (`fait` : quand, ou quand elle a ete
/// refusee), et ce que le journal des envois en dit (`envois::etats::etat_des_embauches`, lu tel qu'il est).
#[derive(Debug, Clone)]
struct EtatDpae {
    date_changee: Option<DateChangee>,
    a_declarer: bool,
    fait: Option<InstantOuJour>,
    passee: bool,
    refusee: bool,
    anomalies: Option<Vec<String>>,
    /// Ce que le certificat dit de sa personne (`envois::etats::EtatEmbauche::certifiee`).
    certifiee: Option<declarations::dpae::EtatDeLaDpae>,
}

/// Un CDI en cours, d'un mois d'apres son embauche : le mois de sa signature.
#[derive(Debug, Clone)]
pub(crate) struct CdiEnCours {
    signe_en: Mois,
}

/// LA PASTILLE CONTRAT D'UN ENGAGEMENT (`etat_signature`). `fait` : quand le contrat (ou son dernier avenant) est parti ;
/// `par`, `recu`, `avenant` : ce que le journal des envois en dit (`envois::etats`), lu tel qu'il est.
#[derive(Debug, Clone)]
pub(crate) struct Signature {
    pub(crate) fait: Option<InstantOuJour>,
    pub(crate) par: Option<documents::ModeDeRemise>,
    pub(crate) recu: Option<Moment>,
    pub(crate) caduc: bool,
    pub(crate) pourquoi: &'static str,
    pub(crate) avenant: Option<envois::journal::AvenantParti>,
    pub(crate) avenant_attendu: bool,
    cdi_en_cours: Option<CdiEnCours>,
}

impl Signature {
    /// Le contrat est parti.
    pub(crate) fn parti(&self) -> bool {
        self.fait.is_some()
    }
}

/// LA PASTILLE DOCUMENTS d'un engagement pour un mois (`etat_documents`).
#[derive(Debug, Clone)]
struct EtatDocuments {
    fait: Option<InstantOuJour>,
    caduc: bool,
    pourquoi: &'static str,
}

impl EtatDocuments {
    fn parti(&self) -> bool {
        self.fait.is_some()
    }
}

/// Les etats des envois d'un engagement ; rien, pour un engagement auquel il manque un fait.
#[derive(Debug, Clone)]
struct Etats {
    dpae: EtatDpae,
    signature: Signature,
    documents: EtatDocuments,
}

impl Etats {
    /// Ceux d'un engagement auquel il manque un fait : rien n'est a declarer, rien n'est parti.
    fn sans_envoi() -> Etats {
        Etats {
            dpae: EtatDpae { date_changee: None, a_declarer: false, fait: None, passee: false, refusee: false, anomalies: None, certifiee: None },
            signature: Signature { fait: None, par: None, recu: None, caduc: false, pourquoi: "", avenant: None, avenant_attendu: false, cdi_en_cours: None },
            documents: EtatDocuments { fait: None, caduc: false, pourquoi: "" },
        }
    }
}

/// CE QUE LES ETAPES LISENT D'UN ENGAGEMENT DU MOIS : qui, quoi, quand, son contrat, ses echeances, son etablissement,
/// ce qu'il recoit (`pieces`), les etats de ses envois. `a_completer` : ce qui lui manque, s'il lui manque un fait (rien
/// ne se calcule sur lui ; son salarie, sa nature, son premier jour peuvent n'etre pas encore dits). `spectacle` : le
/// titre que sa fiche dit.
#[derive(Debug, Clone)]
pub(crate) struct Vue {
    salarie: Option<IdSalarie>,
    nom: Option<String>,
    spectacle: Option<String>,
    /// Le libelle de sa nature de contrat, quand elle est dite.
    libelle_nature: Option<String>,
    numero: Option<NumeroContrat>,
    /// La periode du contrat d'un engagement complet ; aucune pour un engagement a completer, dont les dates sont les
    /// cases de sa saisie (`ContratACompleter`).
    bornes: Option<PeriodeOuverte>,
    etablissement: Option<Nic>,
    echeances: Option<Echeances>,
    valide: Option<Moment>,
    recalcul: Option<String>,
    contrat: ContratDit,
    pieces: Vec<&'static str>,
    etats: Etats,
    a_completer: Option<String>,
}

impl Vue {
    fn valide(&self) -> bool {
        self.valide.is_some()
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
    fn de(vue: Vue, mois: Mois, versions: Option<engagement::versions::Frise>, engagement: engagement::stock::Identite, signee: bool) -> R<Ligne> {
        Ok(Ligne { carres: carres(&vue, mois, signee)?, dpae_choix: choix_dpae(&vue)?, vue, versions, engagement })
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
    salarie: Option<&'a IdSalarie>,
    nom: Option<&'a str>,
    spectacle: Option<&'a str>,
    nature: Option<&'a str>,
    numero: Option<&'a engagement::numeros::NumeroContrat>,
    bornes: Option<PeriodeOuverte>,
    #[serde(with = "socle::paris::ecrit::seconde::option")]
    valide: Option<Moment>,
    recalcul: Option<&'a str>,
    contrat: &'a ContratDit,
    versions: Option<&'a engagement::versions::Frise>,
    carres: Vec<Carre>,
    dpae_choix: Choix,
    etats: EtatsDePage<'a>,
    periode_dit: Option<String>,
    valide_dit: Option<String>,
    validee_dit: Option<String>,
    repris_dit: Option<&'static str>,
    embauche_dit: Option<String>,
    numero_dit: Option<String>,
    /// Le geste qui copie l'engagement pour un autre salarie (`copier-engagement`, puis `enregistrer-saisie`).
    copier: Action,
    /// Les gestes de sa carte ouverte (`GestesDeLaCarte`) : la page en dessine les boutons, et n'en decide aucun.
    gestes: crate::engagement::GestesDeLaCarte,
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
    fn de(l: &'a Ligne, mois: Mois, envois: &crate::envois_en_fond::EnvoisDuMois) -> LigneDePage<'a> {
        let v = &l.vue;
        let valide = v.valide.map(Moment::dit);
        let debut = match (&v.bornes, &v.contrat) {
            (Some(b), _) => Some(b.debut()),
            (None, ContratDit::ACompleter(c)) => c.debut_lu,
            (None, ContratDit::Complet(_)) => None,
        };
        let embauche = debut.map(|d| format!("embauche le {}", d.jj_mm_aaaa()));
        LigneDePage {
            periode_dit: v.bornes.as_ref().map(crate::contrats::periode_dite),
            valide_dit: valide.as_ref().map(|q| format!("le {}", q)),
            validee_dit: valide.as_ref().map(|q| format!("validée le {}", q)),
            repris_dit: matches!(&v.contrat, ContratDit::Complet(c) if c.repris).then_some("contrat repris"),
            embauche_dit: match (&v.spectacle, embauche) {
                (Some(s), Some(e)) => Some(format!("{}, {}", s, e)),
                (_, e) => e,
            },
            numero_dit: v.numero.as_ref().map(|n| format!("contrat n° {}", n)),
            gestes: crate::engagement::GestesDeLaCarte::de(mois, &l.engagement, v.valide.is_some()),
            copier: action(
                Route::CopierEngagement,
                crate::mois::ChargeCarteProposee { mois, engagement: l.engagement.clone() },
                Some(Route::EnregistrerSaisie),
                "Copier pour un autre salarié".into(),
            ),
            engagement: &l.engagement,
            salarie: v.salarie.as_ref(),
            nom: v.nom.as_deref(),
            spectacle: v.spectacle.as_deref(),
            nature: v.libelle_nature.as_deref(),
            numero: v.numero.as_ref(),
            bornes: v.bornes,
            valide: v.valide,
            recalcul: v.recalcul.as_deref(),
            contrat: &v.contrat,
            versions: l.versions.as_ref(),
            carres: l.carres.iter().map(|c| c.pendant_les_envois(envois, v.salarie.as_ref(), v.numero.as_ref())).collect(),
            dpae_choix: l.dpae_choix.pendant_les_envois(envois, v.salarie.as_ref(), v.numero.as_ref()),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    reste: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lien: Option<Option<&'static str>>,
    titre: &'static str,
}

impl Carre {
    /// Le carre de l'engagement du salarie `salarie`, contrat `numero`, tel que la page le lit : un envoi en cours de son
    /// etape qui emporte cet engagement le dit a la place de son geste (`LesDeuxEtats`, la meme surcouche que les cases
    /// du bilan) ; un geste qui n'envoie rien (le dossier d'un contrat signe ailleurs, l'Historique) reste.
    fn pendant_les_envois(&self, envois: &crate::envois_en_fond::EnvoisDuMois, salarie: Option<&IdSalarie>, numero: Option<&NumeroContrat>) -> Carre {
        if !Geste::envoie(self.action.as_ref()) || self.etat == EtatCase::Fait {
            return self.clone();
        }
        match envois.ce_qui_part(self.cle, |p| p.emporte(salarie, numero)) {
            // Son detail perd aussi les gestes qui le referaient partir ; ceux qui n'envoient rien restent.
            Some(quoi) => Carre {
                etat: EtatCase::EnAttente,
                couleur: EtatCase::EnAttente.couleur(),
                phrase: crate::envois_en_fond::phrase_en_cours(&quoi),
                action: None,
                pieces: self.pieces.clone().map(|p| Pieces { gestes: p.gestes.into_iter().filter(|g| !Geste::envoie(g.action.as_ref())).collect(), ..p }),
                coupee: None,
                reste: None,
                ..self.clone()
            },
            None => self.clone(),
        }
    }
}

impl Geste {
    /// Envoie-t-il quelque chose (`Action::envoie`) ? Le dossier d'un contrat signe ailleurs, ou aucun geste, non.
    fn envoie(geste: Option<&Geste>) -> bool {
        match geste {
            Some(Geste::Route(a)) => a.envoie(),
            Some(Geste::Dossier(_)) | None => false,
        }
    }
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
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    chemin: PathBuf,
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
    rang: RangDuGeste,
}

/// CE QUE LA BOITE DES DPAE DIT D'UN ENGAGEMENT (`choix_dpae`) ; `part` : l'embauche qu'il ajoute, coche, a la case
/// `engagements` du geste de la boite (`EtatParEngagement::dpae_envoyer`), quand il se coche.
#[derive(Debug, Clone, Serialize)]
struct Choix {
    cochable: bool,
    cochee: bool,
    etat: EtatCase,
    couleur: &'static str,
    phrase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    part: Option<EmbaucheDesignee>,
}

impl Choix {
    /// Le choix de l'engagement du salarie `salarie`, contrat `numero`, tel que la boite des DPAE le lit : une DPAE en
    /// cours d'envoi qui emporte son embauche, ce qui se cochait le dit, et ne se coche plus ; les autres se cochent.
    fn pendant_les_envois(&self, envois: &crate::envois_en_fond::EnvoisDuMois, salarie: Option<&IdSalarie>, numero: Option<&NumeroContrat>) -> Choix {
        if !self.cochable {
            return self.clone();
        }
        match envois.ce_qui_part(QuelleEtape::Dpae, |p| p.emporte(salarie, numero)) {
            Some(quoi) => Choix {
                cochable: false,
                cochee: false,
                etat: EtatCase::EnAttente,
                couleur: EtatCase::EnAttente.couleur(),
                phrase: crate::envois_en_fond::phrase_en_cours(&quoi),
                part: None,
            },
            None => self.clone(),
        }
    }
}

/// UNE DPAE A FAIRE (`embauches_a_declarer`) : le mois, le contrat, qui, pour quoi, ses bornes, l'empreinte de ce qu'elle
/// declare, et pourquoi quand une date d'embauche a change. `spectacle` : le titre que sa fiche dit (aucun pour un
/// salarie au mois).
#[derive(Debug, Clone, Serialize)]
pub struct DpaeDue {
    pub mois: Mois,
    pub salarie: IdSalarie,
    pub numero: NumeroContrat,
    pub nom: String,
    pub spectacle: Option<String>,
    pub contrat: PeriodeOuverte,
    pub empreinte: EmpreinteCourte,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pourquoi: Option<String>,
}

/// UN FICHIER DEPOSABLE D'UN MOIS (`fichiers_deposables`) : son chemin.
#[derive(Debug, Clone)]
pub(crate) struct Deposable {
    pub(crate) chemin: PathBuf,
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
    #[serde(with = "droit::natures::en_nombre::option")]
    pub(crate) dernier_ordre: Option<NumeroDOrdre>,
    #[serde(with = "droit::natures::en_nombre")]
    prochain_ordre: NumeroDOrdre,
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
    refaire: HashMap<(Mois, IdSalarie, NumeroContrat), Rc<Vec<ARefaire>>>,
    /// Partagees (`Arc`) : l'etat garde d'un mois les garde telles quelles (`LesDeuxEtats`), sans les recopier.
    lignes: HashMap<Mois, std::sync::Arc<Vec<Ligne>>>,
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
    fn nic(&self) -> Option<Nic> {
        self.ctx.entreprise.nic_principal().cloned()
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
        let (Some(_), Some(salarie), Some(numero)) = (e.valide(), carte.salarie.as_ref(), carte.numero.as_ref()) else {
            return Ok(Rc::new(Vec::new()));
        };
        let cle = (mois, salarie.clone(), numero.clone());
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
    pub(crate) fn etat_par_engagement(&mut self, le_mois: Mois) -> R<std::sync::Arc<Vec<Ligne>>> {
        if let Some(l) = self.lignes.get(&le_mois) {
            return Ok(l.clone());
        }
        let l = std::sync::Arc::new(self.lignes_du_mois(le_mois)?);
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
        let a_declarer: HashSet<Paire> = self.embauches_a_declarer(Some(le_mois))?.into_iter().map(|d| (d.salarie, d.numero)).collect();
        let signee = crate::contrats::signature_posee(self.ctx)?;

        // Chaque ligne porte `engagement` : l'identite par laquelle sa carte le designe. Un engagement auquel il manque un
        // fait a sa ligne a lui (`ligne_a_completer`) ; les autres retrouvent leur paie parmi ceux qui ne manquent de rien.
        let mut out = Vec::new();
        for (j, (e, id)) in etats_eng.iter().zip(&ids).enumerate() {
            if let Some(Some(manque)) = s.manques.get(j) {
                let vue = self.ligne_a_completer(mois, e.objet(), &manque.phrase)?;
                out.push(Ligne::de(vue, le_mois, None, id.clone(), signee)?);
                continue;
            }
            // Un engagement ecrit qui ne manque de rien a sa paie dans la saisie du mois (`saisie_chargee`, les memes
            // engagements dans le meme ordre) : ne pas la trouver est un defaut du logiciel, qui se dit.
            let Some(k) = s.identites.iter().position(|x| x == id) else {
                return Err(erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : l'engagement de {} n'a pas sa paie dans la saisie du mois.", engagement::etats::qui_de(self.ctx, e.objet()))));
            };
            let p = &s.paies[k];
            let cle = p.salarie.id.clone();
            // CE QUI EST A REFAIRE : le present compare a la version sur laquelle chaque envoi est parti.
            let refaire = self.a_refaire_du_mois(mois, e, &departs)?;
            let sal = &p.salarie;
            let nat = p.engagement.nat()?;
            let spectacle = match &p.spectacle {
                Some(s) => match s.titre.dite() {
                    Some(t) => Some(t.clone()),
                    None => return Err(socle::fichiers::manque("titre")),
                },
                None => None,
            };
            let mois_du_contrat = p.engagement.mois_du_contrat()?;
            let (Some(&depuis), Some(&jusqu_a)) = (mois_du_contrat.first(), mois_du_contrat.last()) else {
                return Err(erreur(Classe::Manque, socle::fichiers::trop_courte("les mois du contrat")));
            };
            let place = p.engagement.place_du_mois()?;
            // Son etablissement (NIC) : la DSN qui le porte.
            let etablissement = nic_de_l_engagement(self.ctx, e)?;
            // LES ECHEANCES de l'engagement : sa DPAE (L. 1221-10) et la remise de son contrat (L. 1242-13, ou la couche
            // de la convention), `droit::echeances`. Les documents n'en ont pas : chaque compagnie les envoie quand elle
            // veut, souvent apres la DSN qui fait remonter l'AER. Un mois passe lu a l'ouverture (`ecrit`) ne les demande pas.
            let echeances = if self.ecrit {
                None
            } else {
                let code_emploi = p.engagement.emploi.ecrit.code_emploi_ft.as_ref().or(sal.code_emploi_ft.dite());
                let ent = &self.ctx.entreprise;
                let remise = droit::echeances::remise_du_contrat(&p.engagement.contrat().debut(), ent.idcc.dite(), code_emploi, ent.feries_chomes())?;
                Some(Echeances { dpae: droit::echeances::dpae(&p.engagement.contrat().debut()), contrat: remise })
            };
            // RECALCULEE D'ELLE-MEME (`engagement::etats::du_mois`) : pourquoi, et quand.
            let recalcul = match e.valide() {
                Some(v) => v.recalcul(le_mois)?,
                None => None,
            };
            let recalcul = recalcul.as_ref().and_then(engagement::etats::phrase_du_recalcul);
            // REPRIS D'UN ANCIEN LOGICIEL : la carte le dit (`engagement::importe::vient_d_un_ancien_logiciel`).
            let repris = engagement::importe::vient_d_un_ancien_logiciel(&self.ctx.racine, &p.engagement.salarie, p.engagement.contrat().debut(), p.engagement.numero.as_ref())?;
            // SIGNE AILLEURS : son contrat ecrit ne se fait pas signer ici, ses avenants si (`engagement::signe_ailleurs`).
            let ailleurs = engagement::signe_ailleurs(&self.ctx.racine, &p.engagement)?;
            // ECRIT : le contrat de cet engagement se fait signer ici (`engagement::contrat_a_signer`).
            let contrat = ContratDit::Complet(ContratComplet {
                place,
                chevrons: super::mois::chevrons_dits(&p.engagement)?,
                depuis,
                jusqu_a,
                repris,
                ecrit: engagement::contrat_a_signer(&self.ctx.racine, &p.engagement)?,
                ailleurs,
                dossier: ailleurs.then(|| crate::gestes::signature::contrat_signe_dit(self.ctx, &cle, p.engagement.contrat().debut())).transpose()?,
                anterieur: engagement::stock::commence_avant_le_logiciel(&self.ctx.racine, p.engagement.contrat().debut(), le_mois)?,
            });
            // CE QUE CE SALARIE RECOIT POUR CETTE PAIE : le bulletin s'il y a une paie, le certificat Conges Spectacles
            // du contrat declare a la caisse qui finit (`crate::dossier_salarie::certificats_dus`), l'AER Cinema
            // Spectacle qui lui est due (`aer::due`). L'etape « Documents » du mois les nomme d'apres ceci, et rien d'autre.
            let mut pieces: Vec<&'static str> = Vec::new();
            if !p.engagement.sans_paie() {
                pieces.push("bulletin");
            }
            if matches!(place, engagement::PlaceDuMois::Seul | engagement::PlaceDuMois::Dernier) && p.declaree_a_la_caisse_des_conges()? {
                pieces.push("certificat_cs");
            }
            if documents::aer::due(p)? {
                pieces.push("aer");
            }
            // LA DPAE DE CET ENGAGEMENT : partie (`departs`), et ce qui en est revenu (la ligne qui la porte, par le numero du
            // contrat). Refusee au bilan, elle n'est pas partie ; la ligne dit quand elle a ete refusee.
            let numero = p.engagement.numero.as_ref();
            // Le contrat tel que le journal le designe (`envois::etats::Paire`).
            let k: etats::Paire = (cle.clone(), numero.cloned());
            let mut dpae_v = g.dpae.get(&k);
            let mut partie_le = match numero {
                Some(n) => departs.de(&p.salarie.id, n).iter().filter(|d| d.envoi == engagement::departs::Envoi::Dpae).map(|d| d.le.clone()).max(),
                None => None,
            };
            // UNE DATE D'EMBAUCHE CHANGEE depuis la version sur laquelle la DPAE est partie (`ARefaire::Dpae`) : la DPAE de
            // l'ancienne date vaut-elle (`crate::regularisation::dpae_d_une_date_changee`) ?
            let mut date_changee = None;
            if let Some(avant) = debut_declare(&refaire) {
                let (vaut, phrase) = crate::regularisation::dpae_d_une_date_changee(avant, p.engagement.contrat().debut())?;
                if !vaut {
                    dpae_v = None;
                    partie_le = None;
                }
                date_changee = Some(DateChangee { avant, vaut, phrase });
            }
            let refusee = dpae_v.is_some_and(|d| d.refusee);
            let fait = match partie_le {
                Some(le) => Some(le),
                None if refusee => dpae_v.map(|d| d.quand),
                None => None,
            };
            let dpae = EtatDpae {
                date_changee,
                a_declarer: numero.is_some_and(|n| a_declarer.contains(&(cle.clone(), n.clone()))),
                fait,
                passee: dpae_v.is_some_and(|d| d.passee),
                refusee,
                anomalies: dpae_v.map(|d| d.anomalies.clone()),
                certifiee: dpae_v.and_then(|d| d.certifiee.clone()),
            };
            let avenant = refaire.iter().any(|a| matches!(a, ARefaire::Avenant { .. }));
            let mut signature = etat_signature(
                contrat_parti_le(&departs, &p.salarie.id, p.engagement.numero.as_ref()),
                g.par.get(&k).copied(),
                g.recus.get(&k),
                g.renvoyes.contains(&k),
                g.avenants.get(&k),
                avenant,
                ailleurs,
            );
            // UN CDI EN COURS N'A PAS D'ETAT DE CONTRAT MENSUEL.
            if nat.mensuelle() && nat.duree_indeterminee && matches!(place, engagement::PlaceDuMois::Dernier | engagement::PlaceDuMois::Milieu) {
                let signe_en = match g.recus.get(&k) {
                    Some(r) => r.jour_de_paris().mois_civil(),
                    None => depuis,
                };
                signature.cdi_en_cours = Some(CdiEnCours { signe_en });
            }
            // LES DOCUMENTS DE CET ENGAGEMENT, pour ce mois : son dernier envoi (`departs`), et son bulletin a refaire.
            let documents_le = p.engagement.numero.as_ref().and_then(|n| {
                departs.de(&p.salarie.id, n).iter().filter(|d| d.envoi == engagement::departs::Envoi::Documents && d.mois == Some(le_mois) && d.par.is_none()).map(|d| d.le).max()
            });
            let bulletin = refaire.iter().any(|a| matches!(a, ARefaire::Bulletin { mois: m, .. } if *m == mois));
            let documents = etat_documents(documents_le, bulletin);
            let vue = Vue {
                salarie: Some(cle.clone()),
                nom: Some(super::nom_a_l_ecran(sal, &cle)),
                spectacle,
                libelle_nature: Some(nat.libelle.clone()),
                numero: p.engagement.numero.clone(),
                bornes: Some(p.engagement.contrat()),
                etablissement,
                echeances,
                valide: e.valide().map(|v| v.le()),
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
            out.push(Ligne::de(vue, le_mois, frise, id.clone(), signee)?);
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
        let cle = e.salarie.clone();
        // Un salarie qu'aucune fiche ne porte se nomme par sa cle.
        let nom = cle.as_ref().map(|c| match self.ctx.salaries.get(c) {
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
        let debut_du_contrat = match o.premier_mois() {
            m if m == lu => DebutDansLeMois::CeMois,
            _ => DebutDansLeMois::Avant,
        };
        let debut = e.debut_lu();
        let anterieur = if let Some(d) = debut { engagement::stock::commence_avant_le_logiciel(&self.ctx.racine, d, lu)? } else { false };
        let numero = e.numero.clone();
        Ok(Vue {
            salarie: cle,
            nom,
            spectacle,
            libelle_nature,
            numero,
            // Ses dates sont les cases de sa saisie (`ContratACompleter`), pas la periode d'un contrat.
            bornes: None,
            etablissement: None,
            echeances: None,
            valide: None,
            recalcul: None,
            contrat: ContratDit::ACompleter(ContratACompleter { debut: debut_du_contrat, debut_lu: debut, fin_ecrite: e.fin, repris: false, anterieur }),
            pieces: Vec::new(),
            etats: Etats::sans_envoi(),
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
        // Les declarations deposables (la DSN, la DPAE) se rangent dans les sorties du mois.
        let sorties = documents::sorties_du_mois::SortiesDuMois::du_mois(self.racine(), mois)?;
        let noms = sorties.noms(documents::DossierDuMois::Sorties)?;
        let rendu = if noms.is_empty() {
            Deposables::default()
        } else {
            let mut tous: Vec<(String, PathBuf)> = Vec::new();
            for n in noms {
                let chemin = sorties.fichier_liste(documents::DossierDuMois::Sorties, &n)?;
                tous.push((n.texte().to_string(), chemin));
            }
            // L'ORDRE NE DEPEND PAS DE L'HEURE D'UN FICHIER (deux rejeux, deux machines, le meme ordre) : il se lit
            // dans ce que le moteur sait de chacun (`ordre_de_depot`).
            let ranger = |v: Vec<&(String, PathBuf)>| -> R<Vec<Deposable>> {
                let mut ranges = v.into_iter().map(|f| Ok((ordre_de_depot(&f.0, &f.1)?, f))).collect::<R<Vec<_>>>()?;
                ranges.sort_by(|a, b| a.0.cmp(&b.0));
                Ok(ranges.into_iter().map(|(_, (_, chemin))| Deposable { chemin: chemin.clone() }).collect())
            };
            let dsn: Vec<_> = tous.iter().filter(|f| a_l_extension(&f.0, ".dsn")).collect();
            let dpae: Vec<_> = tous
                .iter()
                .filter(|f| {
                    matches!(documents::SorteDePieceProduite::du_nom(&f.0), Some((documents::SorteDePieceProduite::Dpae, _)))
                        && a_l_extension(&f.0, ".txt")
                        && !radical(&f.0).ends_with("_recapitulatif")
                })
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
    pub(crate) fn empreinte_saisie(&self, mois: Mois) -> R<Option<EmpreinteCourte>> {
        let departs = self.departs()?;
        self.empreinte_saisie_avec(mois, || declarations::produire_regularisations::regularisations_pour(self.ctx, mois, &departs).map(|r| Self::portees(&r)))
    }

    /// `empreinte_saisie`, sur les corrections portees que `portees` lit (`Moteur::portees`) : la fabrication de la DSN,
    /// qui les a deja lues, les passe.
    pub(crate) fn empreinte_saisie_avec(&self, mois: Mois, portees: impl FnOnce() -> R<Vec<Portee>>) -> R<Option<EmpreinteCourte>> {
        let vue = engagement::etats::empreinte_des_paies_validees(self.ctx, mois, None)?;
        let portees = portees()?;
        if portees.is_empty() {
            return Ok(Some(vue.courte()));
        }
        let ecrite = socle::ecriture::json_trie(&SaisieEtCorrections { saisie: &vue, corrections: &portees });
        Ok(Some(Empreinte::des_octets(ecrite.as_bytes()).courte()))
    }

    /// Ce que l'empreinte de la saisie retient de chaque correction portee (`Portee`).
    pub(crate) fn portees(regs: &[declarations::produire_regularisations::Reg]) -> Vec<Portee> {
        regs.iter().map(|r| Portee(r.mois, r.apres.salarie.id.to_string(), r.apres.engagement.numero.clone(), r.version)).collect()
    }

    /// `limite` : le dernier jour ou un annule-et-remplace est admis, jusqu'a minuit : la veille de l'echeance de depot
    /// (`echeance_dsn::Echeance::veille`, reportee comme elle).
    pub(crate) fn limite(&self, mois: Mois) -> R<Jour> {
        let e = declarations::echeance_dsn::de_la_compagnie(self.ctx, mois)?;
        e.veille()
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
    pub(crate) fn dernier_ordre(&self, mois: Mois, faits: &[envois::journal::Ligne]) -> R<Option<NumeroDOrdre>> {
        Ok(faits.iter().map(|l| self.ordre_de_la_ligne(mois, l)).collect::<R<Vec<NumeroDOrdre>>>()?.into_iter().max())
    }

    /// Le rang d'un depot reel du journal : l'ordre que sa ligne porte (S20.G00.05.004 du fichier parti, noté au depot).
    pub(crate) fn ordre_de_la_ligne(&self, mois: Mois, l: &envois::journal::Ligne) -> R<NumeroDOrdre> {
        let refus = || Echec {
            a_part: Some(socle::echec::Lieu { fichier: Some(envois::journal::DEPOTS), cle: Some("ordre".to_string()), ..Default::default() }.into()),
            ..erreur(
                Classe::Illisible,
                format!("Un dépôt réel de la DSN de {} ne porte pas son ordre (S20.G00.05.004) dans le journal des envois : le rang d'un annule-et-remplace ne se devine pas ; le signaler au développeur.", mois),
            )
        };
        l.dsn().and_then(|e| e.detail.ordre).ok_or_else(refus)
    }

    /// `depots_reels` : les depots REELS de DSN mensuelle du mois que la
    /// plate-forme a pris et qu'aucun bilan n'a refuses (les lignes du journal des envois, telles qu'elles sont).
    pub(crate) fn depots_reels(&mut self, mois: Mois, nic: Option<&Nic>) -> R<Vec<envois::journal::Ligne>> {
        let lignes = self.lignes()?;
        let reels = envois::journal::filtrer(&lignes, Some(documents::Acte::Dsn), Some(mois), Some(socle::mode::Mode::Reel));
        let mut out = Vec::new();
        for l in etats::de_l_etablissement(reels, nic, self.nic().as_ref()) {
            if !l.accepte() || l.signalement() || etats::refuse_par_un_retour(l) {
                continue;
            }
            // Le type de la declaration (S20.G00.05.002), que le depot note : une ligne qui ne le dit pas se refuse.
            let Some(t) = l.dsn().and_then(|e| e.detail.type_declaration) else {
                return Err(Echec {
                    a_part: Some(socle::echec::Lieu { fichier: Some(envois::journal::DEPOTS), cle: Some("type_declaration".to_string()), ..Default::default() }.into()),
                    ..erreur(Classe::Illisible, format!("Un dépôt réel de la DSN de {} ne dit pas son type (S20.G00.05.002) dans le journal des envois ; le signaler au développeur.", mois))
                });
            };
            if t != droit::type_declaration::TypeDeLaDeclaration::Annule {
                out.push(l.clone());
            }
        }
        Ok(out)
    }

    /// Ou en est le mois vis-a-vis de
    /// l'annule-et-remplace.
    /// `nic` : l'etablissement dont on parle (`Null` : le principal).
    pub(crate) fn etat_remplacement(&mut self, le_mois: Mois, nic: Option<&Nic>) -> R<Remplacement> {
        let faits = self.depots_reels(le_mois, nic)?;
        let lim = self.limite(le_mois)?;
        let mois = le_mois;
        let admis = self.ctx.aujourdhui <= lim;
        let mut out = Remplacement {
            mois: le_mois,
            deposee: !faits.is_empty(),
            dernier_ordre: None,
            prochain_ordre: NumeroDOrdre::PREMIER,
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
        out.prochain_ordre = NumeroDOrdre::suivant(Some(dernier))?;
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
            let voulu = nic.cloned().or_else(|| self.nic());
            let mut n = 0;
            for e in self.etats_du_mois(le_mois)? {
                let Some(v) = e.valide() else { continue };
                if plusieurs && nic_de_l_engagement(self.ctx, &e)? != voulu {
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
        let quand = derniere.quand().dit();
        // La limite, son jour et son heure (« le 14/12/2026 à minuit ») : l'heure dite par `HeureDuJour`.
        let jour = format!("{} {}", lim.jj_mm_aaaa(), socle::paris::HeureDuJour::MINUIT.dite());
        if corriges > 0 {
            let pluriel = if corriges > 1 { "s" } else { "" };
            out.corrigee_par_la_suivante = true;
            out.phrase = format!(
                "Paie modifiée depuis la DSN du {}, après la limite de l'annule-et-remplace ({}) — {} contrat{} corrigé{} par la DSN suivante",
                quand, jour, corriges, pluriel, pluriel
            );
            return Ok(out);
        }
        if modifiee {
            if admis {
                out.a_envoyer = true;
                out.phrase = format!("Paie modifiée depuis la DSN du {} — annule-et-remplace à envoyer, jusqu'au {}", quand, jour);
            } else {
                out.phrase = format!(
                    "Paie modifiée depuis la DSN du {}, après la limite de l'annule-et-remplace ({}) — la correction part dans la DSN suivante",
                    quand, jour
                );
            }
        } else if admis {
            out.phrase = format!("Une modification reste possible : la DSN refaite partira en annule-et-remplace (ordre {}), jusqu'au {}", out.prochain_ordre, jour);
        }
        Ok(out)
    }

    /// `paie_changee_depuis_l_ecriture` : `None` si le fichier declare la paie
    /// d'aujourd'hui, sinon POURQUOI. Le journal des ecritures (`compagnie::pieces_ecrites`) se lit tel qu'il est.
    pub(crate) fn paie_changee_depuis_l_ecriture(&self, le_mois: Mois, fichier: &Path) -> R<Option<String>> {
        use compagnie::pieces_ecrites::{piece_notee, EmpreinteNotee};
        let note = piece_notee(self.racine(), fichier)?;
        if let Some((n, EmpreinteNotee::Saisie(notee))) = note.as_ref().and_then(|n| Some((n, n.empreinte_saisie.as_ref()?))) {
            if self.empreinte_saisie(le_mois)?.as_ref() != Some(notee) {
                return Ok(Some(format!("l'engagement a changé depuis que ce fichier a été écrit (le {})", n.quand.dit())));
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
    fn chemin_de_depot(&mut self, le_mois: Mois, quoi: &str, nic: Option<&Nic>) -> R<Option<(EtapeDuDepot, Deposable, Option<String>)>> {
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
                (true, Some(n)) => declarations::dsn_lue::lire(&crate::depots::lire_octets(&f.chemin)?, None)?.etablissement() == Some(n),
                // Une DPAE, ou une DSN sans etablissement demande : le premier fichier, quel qu'il soit.
                (true, None) | (false, Some(_)) | (false, None) => true,
            };
            if de_l_etablissement {
                trouve = Some(f.clone());
                break;
            }
        }
        let Some(premier) = trouve else {
            return Ok(None);
        };
        let chemin = premier.chemin.clone();
        let lignes = self.lignes()?;
        let empreinte = envois::journal::empreinte_fichier(&chemin).map_err(|m| m.reclasser(Classe::FichierAbsent))?;
        let acte = if quoi == DSN { documents::Acte::Dsn } else { documents::Acte::Dpae };
        if etats::deja_depose(&lignes, acte, &empreinte).is_some() {
            return Ok(None);
        }
        let nom = NomDeFichier::du(&chemin)?;
        let r = etats::repetition(&lignes, acte, &empreinte, &nom);
        let etape = if quoi == DSN && self.paie_changee_depuis_l_ecriture(le_mois, &chemin)?.is_some() {
            EtapeDuDepot::PaieChangee
        } else {
            EtapeDuDepot::Test(r.etat)
        };
        // UN TEST REFUSE SE DIT AVEC SES ANOMALIES, la ou l'on propose d'envoyer : le code et le message du bilan.
        let refus = r.motif.as_deref().filter(|_| etape == EtapeDuDepot::Test(etats::EtatDuTest::TestRefuse)).map(|m| m.trim_end_matches('.').to_string());
        Ok(Some((etape, premier, refus)))
    }

    /// LE GESTE d'une case orange qui depose (DPAE, DSN), une fois sa declaration ecrite : la charge du depot qui la
    /// designe (`depots::designee` : le mois, l'etablissement, les embauches que le fichier ecrit declare) ; sinon `defaut`.
    fn action_de_depot(&mut self, le_mois: Mois, quoi: &str, nic: Option<&Nic>, defaut: Option<Action>) -> R<(Option<Action>, Option<String>)> {
        let Some((etape, fichier, refus)) = self.chemin_de_depot(le_mois, quoi, nic)? else { return Ok((defaut, None)) };
        let mut charge = Designation::du_mois(if quoi == DSN { Quoi::Dsn } else { Quoi::Dpae }, le_mois);
        if quoi == DSN {
            charge.etablissement = nic.cloned();
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
        let deposer = |libelle: String| vers(VueDuGeste::Depot, charge.clone(), Some(Route::Deposer), libelle);
        let action = match etape {
            // Le fichier ecrit ne declare plus la paie : la DSN se refait depuis les engagements valides, et repasse par
            // le test.
            EtapeDuDepot::PaieChangee => deposer_dsn(le_mois, nic, format!("Réécrire {} et la tester…", nom)),
            EtapeDuDepot::Test(etats::EtatDuTest::Ecrite) => deposer(format!("Tester {}…", nom)),
            EtapeDuDepot::Test(etats::EtatDuTest::EnTest) => deposer("Voir le test en cours…".into()),
            EtapeDuDepot::Test(etats::EtatDuTest::Testee) => deposer(format!("Envoyer {} testée…", nom)),
            EtapeDuDepot::Test(etats::EtatDuTest::TestRefuse) => deposer("Voir le refus du test…".into()),
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
            let lignes = envois::journal::lu(&self.ctx.racine)?.lignes;
            let ou_en_est = OuEnEstLeMois::de(self.ctx, le_mois, &lignes, &etapes);
            return Ok(Bilan { mois: le_mois, phrase: phrase_du_mois(None, &etapes), colonnes, etapes, ou_en_est });
        }
        let paies = self.etat_par_engagement(le_mois)?;
        let signee = crate::contrats::signature_posee(self.ctx)?;
        let echeance_de = |ps: &[&Vue], dpae: bool| -> Option<Echeance> {
            ps.iter().filter_map(|p| p.echeances.as_ref()).map(|e| if dpae { &e.dpae } else { &e.contrat }).min_by_key(|e| e.jour()).cloned()
        };

        // --- Les colonnes : chaque engagement a son point, la DSN celui de son etablissement.
        let classes = self.groupes_dsn(le_mois)?;
        // La classe d'un engagement : celle de son etablissement. Une compagnie d'un seul etablissement n'a que lui. Une
        // carte qui ne nomme pas encore son salarie, dans une compagnie de plusieurs etablissements, n'en a pas : sa DSN
        // est a completer. Un etablissement qui n'est pas de la compagnie se refuse.
        let classe_de = |p: &Vue| -> R<Option<usize>> {
            match (p.etablissement.as_ref(), classes.len()) {
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
        let ordre = [PointDpae::AFaire, PointDpae::Refuse, PointDpae::ACompleter, PointDpae::AValider];
        for (point, m, noms, _) in cases_de_la_ligne(&cols, |c| point_de(c, &points, |p| p.dpae, &ordre, PointDpae::Fait)) {
            let siens: Vec<&Vue> = de(&m).into_iter().zip(m.iter()).filter(|(_, i)| points[**i].dpae == point).map(|(p, _)| p).collect();
            let n = siens.len();
            let g = match point {
                PointDpae::Refuse =>Groupe::de(EtatCase::Refuse, Some(Compte::Nombre(n)), n.to_string(), format!("refusée{} · l'éditeur est prévenu", plur(n, "", "s")), echeance_de(&siens, true), Some(historique(le_mois))),
                PointDpae::AFaire => {
                    let (a, refus) = self.action_de_depot(le_mois, DPAE, None, Some(declarer_embauches(le_mois)))?;
                    // Le jour de l'embauche passe, la DPAE qui devait le preceder (L. 1221-10) est en retard ; elle se depose
                    // toujours.
                    let echeance = echeance_de(&siens, true);
                    let en_retard = siens.iter().filter(|p| p.echeances.as_ref().is_some_and(|e| e.dpae.jour() < self.ctx.aujourdhui)).count();
                    let etat = match en_retard {
                        0 => "à déposer".to_string(),
                        r if r == n => "à déposer, en retard".to_string(),
                        r => format!("à déposer, dont {} en retard", r),
                    };
                    let quoi = avec_refus(&etat, refus);
                    Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), quoi, echeance, a)
                }
                PointDpae::ACompleter => a_completer(n),
                PointDpae::AValider => a_valider(n),
                PointDpae::RienADeclarer => rien_ici("rien à déclarer"),
                PointDpae::Fait => Groupe::fait(),
            };
            dpae.push(g.sous(&noms));
        }

        // --- Contrats : un par embauche dont la nature a un contrat ecrit (`engagement::contrat_a_signer`).
        let mut contrats = Vec::new();
        let ordre = [PointContrat::AEnvoyer, PointContrat::ARefaire, PointContrat::EnAttente, PointContrat::ACompleter, PointContrat::AValider];
        for (point, m, noms, _) in cases_de_la_ligne(&cols, |c| point_de(c, &points, |p| p.contrat, &ordre, PointContrat::Fait)) {
            let siens: Vec<&Vue> = de(&m).into_iter().zip(m.iter()).filter(|(_, i)| points[**i].contrat == point).map(|(p, _)| p).collect();
            let n = siens.len();
            let g = match point {
                PointContrat::EnAttente => {
                    // CE QUI EST REVENU SUR CE QUI VIENT DE PARTIR, et sur cela seul : les contrats de la case partis le
                    // jour du dernier envoi encore attendu.
                    let jour = |p: &Vue| p.etats.signature.fait.map(InstantOuJour::jour);
                    let dernier = siens.iter().map(|p| jour(p)).max().flatten();
                    let partis: Vec<&Vue> = de(&m)
                        .into_iter()
                        .zip(m.iter())
                        .filter(|(p, i)| matches!(points[**i].contrat, PointContrat::Fait | PointContrat::EnAttente) && p.etats.signature.parti() && jour(p) == dernier)
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
                PointContrat::ARefaire => Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à refaire signer".into(), None, Some(faire_signer(le_mois, None, signee))),
                PointContrat::AEnvoyer => Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à envoyer".into(), echeance_de(&siens, false), Some(faire_signer(le_mois, None, signee))),
                PointContrat::ACompleter => a_completer(n),
                PointContrat::AValider => a_valider(n),
                PointContrat::RienASigner => rien_ici("rien à signer"),
                PointContrat::Fait => Groupe::fait(),
            };
            contrats.push(g.sous(&noms));
        }

        // --- DSN : par etablissement (`groupes_dsn`), chaque colonne sous la DSN de ses etablissements ; celle des cartes
        // qui ne nomment pas encore leur salarie est a completer.
        let dsn: Vec<Groupe> = cases_de_la_ligne(&cols, |c| c.dsn)
            .into_iter()
            .map(|(_, m, noms, k)| match k {
                Some(k) => classes[k].1.clone().sous(&noms),
                None => a_completer(m.len()).sous(&noms),
            })
            .collect();

        // --- Documents : par salarie du mois, des que son engagement est valide, sans echeance : ils partent quand on veut.
        let mut documents = Vec::new();
        let ordre = [PointDocuments::ARenvoyer, PointDocuments::AEnvoyer, PointDocuments::ACompleter, PointDocuments::AValider];
        for (point, m, noms, _) in cases_de_la_ligne(&cols, |c| point_de(c, &points, |p| p.documents, &ordre, PointDocuments::Fait)) {
            let siens: Vec<&Vue> = de(&m).into_iter().zip(m.iter()).filter(|(_, i)| points[**i].documents == point).map(|(p, _)| p).collect();
            let n = siens.iter().map(|p| p.salarie.as_ref()).collect::<BTreeSet<Option<&IdSalarie>>>().len();
            let g = match point {
                PointDocuments::ARenvoyer => Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à renvoyer".into(), None, Some(envoyer_documents(le_mois, format!("Renvoyer les documents corrigés ({})…", n)))),
                PointDocuments::AEnvoyer => Groupe::de(EtatCase::AFaire, Some(Compte::Nombre(n)), n.to_string(), "à envoyer".into(), None, Some(envoyer_documents(le_mois, "Envoyer les documents…".into()))),
                // Ce qui reste a completer se compte en engagements, comme a la DPAE.
                PointDocuments::ACompleter => a_completer(siens.len()),
                PointDocuments::AValider => a_valider(siens.len()),
                PointDocuments::Fait => Groupe::fait(),
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
        let lignes = envois::journal::lu(&self.ctx.racine)?.lignes;
        let ou_en_est = OuEnEstLeMois::de(self.ctx, le_mois, &lignes, &etapes);
        Ok(Bilan { mois: le_mois, phrase: phrase_du_mois(debut, &etapes), colonnes: noms, etapes, ou_en_est })
    }

    /// LA DSN DU MOIS, par etablissement. Elle « est mensuelle et produite par établissement employeur » (cahier technique
    /// DSN 2026.1, § 2.4, `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`) : chaque
    /// etablissement a la sienne (`groupes_dsn_de`). Les etablissements au meme point (meme etat, phrase et echeance)
    /// forment une classe, une case ; quand ils ne sont pas tous au meme point, la case d'une classe qui n'est pas faite
    /// nomme ses etablissements (« Lyon refusée · l'éditeur est prévenu »). Rend les classes (leurs NIC, leur case), le
    /// principal dans la premiere.
    fn groupes_dsn(&mut self, mois: Mois) -> R<Vec<(Vec<Nic>, Groupe)>> {
        let ent = self.ctx.entreprise_du_mois(mois);
        if ent.secondaires().is_empty() {
            let g = self.groupes_dsn_de(mois, None)?;
            // Une fiche qui ne dit pas encore son NIC n'a que son siege : sa classe ne le nomme pas.
            return Ok(vec![(ent.nic_principal().cloned().into_iter().collect(), g)]);
        }
        let point = |g: &Groupe| (g.etat, g.phrase.clone(), g.echeance.as_ref().map(Echeance::jour));
        let mut classes: Vec<(Vec<Nic>, Groupe)> = Vec::new();
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
    fn groupes_dsn_de(&mut self, le_mois: Mois, nic: Option<&Nic>) -> R<Groupe> {
        let echeance = declarations::echeance_dsn::de_la_compagnie(self.ctx, le_mois)?;
        let au_depot = echeance_dsn(echeance.limite()?, "Cahier technique DSN 2026.1, § 1.4.2 ; CSS R. 243-6 et R. 133-14 (declarations::echeance_dsn)");
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
        let veille = echeance_dsn(echeance.limite_de_l_annule_et_remplace()?, "Cahier technique DSN 2026.1, § 1.4.2 (atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt)");
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
            let e2 = declarations::echeance_dsn::de_la_compagnie(self.ctx, suivant)?;
            let a = deposer_dsn(suivant, nic, format!("Déposer la DSN de {}…", suivant.libelle(Graphie::Accentuee)));
            return Ok(Groupe::de(
                EtatCase::AFaire,
                None,
                String::new(),
                format!("à corriger dans la DSN de {}", suivant.libelle(Graphie::Accentuee)),
                Some(echeance_dsn(e2.limite()?, "Cahier technique DSN 2026.1, § 1.4.2 (declarations::echeance_dsn)")),
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
    fn dsn_a_declarer(&mut self, le_mois: Mois, nic: Option<&Nic>) -> R<bool> {
        let plusieurs = !self.ctx.entreprise_du_mois(le_mois).secondaires().is_empty();
        let voulu = nic.cloned().or_else(|| self.nic());
        let de_l_etablissement = |ctx: &Contexte, brut: &engagement::stock::Ecrit| -> R<bool> { Ok(!plusieurs || nic_de_l_engagement_ecrit(ctx, brut)? == voulu) };
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

    /// `exercice_clos` : l'annee `annee` est-elle close (`documents::sorties_du_mois::lire_exercices`) ?
    pub(crate) fn exercice_clos(&self, annee: socle::mois::Annee) -> R<bool> {
        Ok(documents::sorties_du_mois::lire_exercices(self.racine())?.de(annee).is_some())
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
                let phrase = socle::phrase!("À ouvrir : {}, où il reste {}.", m.libelle(socle::mois::Graphie::Accentuee), noms.join(", "));
                return Ok(MoisAOuvrir { phrase, mois: m, courant, reste });
            }
        }
        let phrase = socle::phrase!("À ouvrir : {}, le mois du jour ; rien ne reste aux mois d'avant.", courant.libelle(Graphie::Accentuee));
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
                let (Some(cle), Some(numero)) = (&brut.saisie.carte.salarie, &brut.saisie.carte.numero) else { continue };
                let Some(p) = s.paies.iter().find(|p| p.salarie.id == *cle && p.engagement.numero.as_ref() == Some(numero)) else {
                    continue;
                };
                // Seule la DPAE partie se compare (`debut_declare`) : les paies parties ne se relisent pas ici, sauf
                // quand le mois a deja compare tout ce qui est a refaire (`a_refaire_du_mois`).
                let refaire: Vec<ARefaire> = match self.refaire.get(&(le_mois, cle.clone(), numero.clone())) {
                    Some(r) => r.iter().filter(|a| matches!(a, ARefaire::Dpae { .. })).cloned().collect(),
                    None => engagement::etats::a_refaire_parmi(self.ctx, v, &departs, &|s| s == engagement::departs::Envoi::Dpae)?,
                };
                // UNE DPAE PAR EMBAUCHE, reconnue a sa date : « L'embauche d'un salarié ne peut intervenir qu'après
                // déclaration nominative » (code du travail, L. 1221-10), et la DPAE porte la « Date et heure
                // d'embauche » (R. 1221-1, 3°) ; `atelier/reference/sources/code-du-travail/xml/L1221-10.xml` et
                // `R1221-1.xml`. INFERENCE : un cachet ajoute a une date nouvelle d'un engagement deja declare prolonge le
                // meme engagement (un avenant) ; l'embauche et sa date ne changent pas, aucune DPAE ne ressort -- aucun texte
                // depose n'en demande une nouvelle quand la duree (R. 1221-1, 4°) s'allonge. Une date d'embauche avancee,
                // elle, en appelle une (`dpae_d_une_date_changee`, plus bas).
                // La DPAE de cet engagement est partie (`departs`) ; sa date d'embauche n'a pas change depuis (`ARefaire::Dpae`).
                let partie = departs.de(cle, numero).iter().any(|d| d.envoi == engagement::departs::Envoi::Dpae);
                let ancienne = debut_declare(&refaire);
                if partie && ancienne.is_none() {
                    continue;
                }
                // Un contrat en cours se declare avec son premier mois.
                if let engagement::PlaceDuMois::Dernier | engagement::PlaceDuMois::Milieu = p.engagement.place_du_mois()? {
                    continue;
                }
                // UNE DATE D'EMBAUCHE CHANGEE depuis la version sur laquelle la DPAE est partie : reculee, la DPAE de l'ancienne
                // date vaut ; avancee, une DPAE est a deposer, et la phrase dit pourquoi (`dpae_d_une_date_changee`). Seulement
                // quand une date d'embauche a change : le reste du temps, rien a dire.
                let pourquoi = match ancienne {
                    Some(a) => {
                        let (vaut, phrase) = crate::regularisation::dpae_d_une_date_changee(a, p.engagement.contrat().debut())?;
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
                    salarie: cle.clone(),
                    numero,
                    nom: super::nom_a_l_ecran(&p.salarie, cle),
                    spectacle: titre,
                    contrat: p.engagement.contrat(),
                    empreinte: empreinte_de_l_embauche(&p.engagement, &p.salarie),
                    pourquoi,
                });
            }
        }
        Ok(attente)
    }
}

/// « le 05/11/2026 » : le jour d'un envoi que l'etat dit parti ; sans jour, c'est un defaut du logiciel.
fn le_jour_de(t: Option<Jour>) -> R<String> {
    match t {
        Some(jour) => Ok(format!("le {}", jour.jj_mm_aaaa())),
        None => Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : un envoi parti ne dit pas quand.")),
    }
}

/// Un carre, son etat et sa phrase.
fn carre(cle: QuelleEtape, nom: impl Into<String>, etat: EtatCase, phrase: String, action: Option<Geste>, reste: Option<&'static str>) -> Carre {
    Carre { cle, nom: nom.into(), etat, phrase, action, pieces: None, couleur: etat.couleur(), coupee: None, reste, lien: None, titre: if cle == QuelleEtape::Dpae { "DPAE" } else { "Contrat" } }
}

/// LES TROIS CARRES DE LA LIGNE D'UN ENGAGEMENT VALIDE (DPAE, Contrat, Documents), lus comme les cases du panneau
/// (`Point::de`) : chacun son nom, son etat (`fait`, `a_faire`, `en_attente`, `refuse`), sa phrase (l'infobulle), le
/// geste qu'ouvre un clic, et le titre de sa boite. Le contrat ou l'avenant parti et pas revenu se note signe d'un clic
/// (`contrat-recu`).
fn carres(p: &Vue, mois: Mois, signee: bool) -> R<Vec<Carre>> {
    // Les carres ne lisent pas la classe DSN du point (la DSN n'a pas de carre).
    let pt = Point::de(p, None);
    // A COMPLETER, OU A VALIDER : rien n'est parti de lui ; chaque carre qui l'attend dit ce qui reste, sans geste.
    if p.a_completer.is_some() || !p.valide() {
        let attend = |cle: QuelleEtape, nom: &str, attente: Attente| match (attente, p.a_completer.as_deref()) {
            (Attente::ACompleter, Some(manque)) => carre(cle, nom, EtatCase::AFaire, format!("À compléter avant de valider : {}", manque), None, Some("à compléter")),
            (Attente::AValider, _) => carre(cle, nom, EtatCase::AFaire, format!("{} après la validation", nom), None, Some("à valider")),
            (Attente::ACompleter, None) | (Attente::Aucune, _) => carre(cle, nom, EtatCase::Fait, "Rien à faire ici pour cet engagement".into(), None, None),
        };
        return Ok(vec![
            attend(QuelleEtape::Dpae, "DPAE", pt.dpae.attente()),
            attend(QuelleEtape::Signature, "Contrat", pt.contrat.attente()),
            attend(QuelleEtape::Documents, "Documents", pt.documents.attente()),
        ]);
    }
    let d = &p.etats.dpae;
    let s = &p.etats.signature;
    let o = &p.etats.documents;
    let debut_du_contrat = p.contrat.debut();
    // Un engagement valide dit son premier jour.
    let Some(debut) = p.bornes.map(|b| b.debut()) else {
        return Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : un engagement valide ne dit pas son premier jour."));
    };
    let debut_dit = debut.jj_mm_aaaa();
    let depuis = p.contrat.depuis();
    let route = |a: Action| Some(Geste::Route(a));

    // Un engagement valide n'est ni a completer ni a valider : ses points sont ceux de ce qui part.
    let (etat, phrase, geste) = match (pt.dpae, debut_du_contrat) {
        (PointDpae::Refuse, _) => {
            let motif = motif_dpae(d);
            (EtatCase::Refuse, format!("DPAE refusée {} : {}", le_jour_de(d.fait.map(InstantOuJour::jour))?, motif), route(historique(mois)))
        }
        (PointDpae::AFaire, _) => (EtatCase::AFaire, "DPAE à déposer".to_string(), route(declarer_embauches(mois))),
        (PointDpae::RienADeclarer, _) => (EtatCase::Fait, format!("Contrat commencé le {}, avant PuppetPaie : rien à déclarer ici", debut_dit), None),
        (PointDpae::Fait | PointDpae::ACompleter | PointDpae::AValider, DebutDansLeMois::Avant) => (EtatCase::Fait, format!("DPAE faite en {}, à l'embauche", depuis), None),
        (PointDpae::Fait | PointDpae::ACompleter | PointDpae::AValider, DebutDansLeMois::CeMois) if d.passee => (EtatCase::Fait, format!("DPAE déposée {}, passée", le_jour_de(d.fait.map(InstantOuJour::jour))?), None),
        (PointDpae::Fait | PointDpae::ACompleter | PointDpae::AValider, DebutDansLeMois::CeMois) => (EtatCase::Fait, format!("DPAE déposée {}, retour pas encore arrivé", le_jour_de(d.fait.map(InstantOuJour::jour))?), None),
    };
    let pieces = Pieces { vide: Some("Aucune DPAE déposée pour cette embauche.".into()), encart: None, gestes: Vec::new() };
    let dpae = Carre { pieces: Some(pieces), ..carre(QuelleEtape::Dpae, "DPAE", etat, phrase, geste, None) };

    let cdi = &s.cdi_en_cours;
    let ailleurs = p.contrat.ailleurs();
    let dossier = p.contrat.dossier();
    let nom_contrat = match cdi {
        Some(c) if !ailleurs => format!("CDI, signé en {}", c.signe_en.libelle(Graphie::Accentuee)),
        _ => "Contrat".to_string(),
    };
    let par_dit = etats::mot_de_la_remise(s.par, etats::PlaceDeLaRemise::Phrase);
    // LE CARRE NE VISE QUE SON ENGAGEMENT : sa boite ne coche d'abord que son contrat, comme son detail
    // (`pieces_du_contrat`) ; pendant l'envoi d'autres contrats du mois, il ne les renvoie pas.
    let sien = p.salarie.clone().map(|s| vec![s]);
    let (etat, phrase, geste) = match (pt.contrat, debut_du_contrat) {
        (PointContrat::AEnvoyer, _) => (EtatCase::AFaire, "Contrat à envoyer".to_string(), route(faire_signer(mois, sien.clone(), signee))),
        (PointContrat::ARefaire, _) if !s.parti() => (EtatCase::AFaire, majuscule_initiale(s.pourquoi), route(faire_signer(mois, sien.clone(), signee))),
        (PointContrat::ARefaire, _) => (EtatCase::AFaire, format!("Contrat parti {} : {}", le_jour_de(s.fait.map(InstantOuJour::jour))?, s.pourquoi), route(faire_signer(mois, sien.clone(), signee))),
        // Un avenant parti, imprime ou par courriel, se note signe d'un clic, comme le contrat ; la relance est dans la
        // boite des contrats en attente. `par` est celui du dernier depart, l'avenant attendu.
        (PointContrat::EnAttente, _) if s.avenant_attendu => (
            EtatCase::EnAttente,
            match s.avenant.as_ref().and_then(|a| a.numero) {
                Some(n) => format!("Avenant n° {} {} {}", n, par_dit, le_jour_de(s.avenant.as_ref().map(|a| a.quand.jour()))?),
                None => format!("Avenant {} {}", par_dit, le_jour_de(s.avenant.as_ref().map(|a| a.quand.jour()))?),
            },
            noter_recu(p, mois, "Noter l'avenant signé", s.avenant.as_ref().and_then(|a| a.numero), true).and_then(&route),
        ),
        (PointContrat::EnAttente, _) => (EtatCase::EnAttente, format!("Contrat {} {}", par_dit, le_jour_de(s.fait.map(InstantOuJour::jour))?), noter_recu(p, mois, "Noter le contrat signé", None, true).and_then(&route)),
        (PointContrat::RienASigner, _) if ailleurs && !p.contrat.anterieur() => (EtatCase::Fait, "Contrat signé hors de PuppetPaie : rien à signer ici".to_string(), geste_du_dossier(dossier)),
        (PointContrat::RienASigner, _) => (EtatCase::Fait, format!("Contrat commencé le {}, avant PuppetPaie : rien à signer ici", debut_dit), geste_du_dossier(dossier)),
        (PointContrat::Fait | PointContrat::ACompleter | PointContrat::AValider, _) if cdi.is_some() => (EtatCase::Fait, format!("CDI en cours depuis {}, sans terme : rien à envoyer", depuis), None),
        (PointContrat::Fait | PointContrat::ACompleter | PointContrat::AValider, DebutDansLeMois::Avant) => (EtatCase::Fait, format!("Contrat signé en {}", depuis), None),
        (PointContrat::Fait | PointContrat::ACompleter | PointContrat::AValider, DebutDansLeMois::CeMois) if !p.contrat.ecrit() => (EtatCase::Fait, "Pas de contrat à faire signer ici".to_string(), None),
        (PointContrat::Fait | PointContrat::ACompleter | PointContrat::AValider, DebutDansLeMois::CeMois) => (EtatCase::Fait, format!("Contrat signé {}", le_jour_de(s.recu.map(Moment::jour_de_paris))?), None),
    };
    let contrat = Carre { pieces: Some(pieces_du_contrat(p, mois, s, dossier, signee)), ..carre(QuelleEtape::Signature, nom_contrat, etat, phrase, geste, None) };

    let (etat, phrase, geste) = match pt.documents {
        PointDocuments::ARenvoyer => (EtatCase::AFaire, format!("Documents à renvoyer : {}", o.pourquoi), route(envoyer_documents(mois, "Renvoyer les documents corrigés…".into()))),
        PointDocuments::AEnvoyer => (EtatCase::AFaire, "Documents à envoyer".to_string(), route(envoyer_documents(mois, "Envoyer les documents…".into()))),
        PointDocuments::Fait | PointDocuments::ACompleter | PointDocuments::AValider => (EtatCase::Fait, format!("Documents partis {}", le_jour_de(o.fait.map(InstantOuJour::jour))?), None),
    };
    let documents = carre(QuelleEtape::Documents, "Documents", etat, phrase, geste, None);
    // CE QUI RESTE, EN UN MOT, sur le carre meme (la phrase entiere est l'infobulle) ; un point fait n'a rien qui reste.
    let restes = [
        match pt.dpae {
            PointDpae::Refuse => Some("refusée"),
            PointDpae::AFaire => Some("à déposer"),
            PointDpae::Fait | PointDpae::ACompleter | PointDpae::AValider | PointDpae::RienADeclarer => None,
        },
        match pt.contrat {
            PointContrat::AEnvoyer => Some("à envoyer"),
            PointContrat::ARefaire => Some("à refaire signer"),
            PointContrat::EnAttente => Some("signature attendue"),
            PointContrat::Fait | PointContrat::ACompleter | PointContrat::AValider | PointContrat::RienASigner => None,
        },
        match pt.documents {
            PointDocuments::ARenvoyer => Some("à renvoyer"),
            PointDocuments::AEnvoyer => Some("à envoyer"),
            PointDocuments::Fait | PointDocuments::ACompleter | PointDocuments::AValider => None,
        },
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
fn pieces_du_contrat(p: &Vue, mois: Mois, s: &Signature, dossier: Option<&ContratSigneDit>, signee: bool) -> Pieces {
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
        gestes.push(GesteDePiece { libelle: "Ranger le contrat signé…", titre: None, action: geste_du_dossier(dossier), rang: RangDuGeste::principal_si(!caduc) });
        if caduc || fait {
            gestes.push(GesteDePiece { libelle: "Avenant…", titre: Some("Modifier le contrat signé hors de PuppetPaie : ce qui change, en clair"), action: avenant(p, mois).and_then(&route), rang: RangDuGeste::principal_si(caduc) });
        }
        return Pieces { vide, encart, gestes };
    }
    if fait || recu {
        let libelle = if recu { "Retirer la mention « signé »" } else { "Noter le contrat signé reçu" };
        gestes.push(GesteDePiece { libelle, titre: None, action: noter_recu(p, mois, libelle, None, !recu).and_then(&route), rang: RangDuGeste::Second });
    }
    if fait {
        gestes.push(GesteDePiece { libelle: "Avenant…", titre: Some("Modifier le contrat parti : l'ancien texte et le nouveau, article par article"), action: avenant(p, mois).and_then(&route), rang: RangDuGeste::principal_si(caduc) });
    }
    if !fait {
        // Depuis le detail de l'engagement, seul son contrat se coche d'abord.
        let siens = p.salarie.clone().map(|s| vec![s]);
        let libelle = if signee { "Envoyer à la signature…" } else { SIGNATURE_A_POSER };
        gestes.push(GesteDePiece { libelle, titre: None, action: route(faire_signer(mois, siens, signee)), rang: RangDuGeste::Principal });
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
    // Ce qui se coche est valide : sa fiche et son contrat sont dits.
    let choix = |cochable: bool, cochee: bool, etat: EtatCase, phrase: String| -> R<Choix> {
        let part = match (cochable, p.salarie.clone(), p.numero.clone()) {
            (false, _, _) => None,
            (true, Some(salarie), Some(numero)) => Some(EmbaucheDesignee { salarie, numero }),
            (true, _, _) => return Err(erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : une embauche à cocher n'a pas sa fiche ou son contrat.")),
        };
        Ok(Choix { cochable, cochee, etat, couleur: etat.couleur(), phrase, part })
    };
    // Il lui manque un fait : sa DPAE attend qu'il soit complet.
    if let Some(manque) = &p.a_completer {
        return choix(false, false, EtatCase::AFaire, format!("à compléter avant de valider : {}", manque));
    }
    if p.contrat.anterieur() {
        return choix(false, false, EtatCase::Fait, "contrat commencé avant PuppetPaie : rien à déclarer ici".into());
    }
    if let DebutDansLeMois::Avant = p.contrat.debut() {
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
    let le = le_jour_de(d.fait.map(InstantOuJour::jour))?;
    if d.refusee {
        let motif = motif_dpae(d);
        return choix(true, true, EtatCase::Refuse, format!("refusée {} — {} — à redéclarer", le, motif));
    }
    if d.passee {
        let dossier = match &d.certifiee {
            Some(declarations::dpae::EtatDeLaDpae::DejaExistante { .. }) => " (déjà existante à l'Urssaf, rien de créé)".to_string(),
            Some(declarations::dpae::EtatDeLaDpae::Enregistree { dossier }) => format!(" (dossier {})", dossier),
            Some(declarations::dpae::EtatDeLaDpae::Refusee { .. }) | None => String::new(),
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
    pub(crate) quand: InstantOuJour,
    pub(crate) resume: Option<envois::envoi::ResumeDePaie>,
}

/// LES DOCUMENTS PARTIS DE `mois`, par salarie : le dernier envoi de ceux dont tous les engagements valides du mois sont
/// partis.
pub(crate) fn documents_partis(ctx: &Contexte, mois: Mois, etats: &[engagement::etats::Etat], lignes: &[envois::journal::Ligne]) -> R<HashMap<IdSalarie, DocumentsPartis>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let resumes = etats::resumes_des_documents(lignes, mois);
    let mut out: HashMap<IdSalarie, DocumentsPartis> = HashMap::new();
    // Les documents d'un salarie partent ensemble, pour tous ses engagements valides du mois : un engagement valide dont
    // ils ne sont pas partis (valide apres leur envoi) les rend a envoyer, avec son bulletin.
    let a_envoyer: HashSet<&IdSalarie> = etats
        .iter()
        .filter(|e| matches!(e, engagement::etats::Etat::Valide(_)))
        .filter_map(|e| e.objet().brut.saisie.carte.salarie.as_ref())
        .collect();
    for e in etats {
        if !matches!(e, engagement::etats::Etat::Partis(_)) {
            continue;
        }
        let brut = &*e.objet().brut;
        let (Some(cle), Some(numero)) = (&brut.saisie.carte.salarie, &brut.saisie.carte.numero) else { continue };
        let dernier = departs
            .de(cle, numero)
            .iter()
            .filter(|d| d.envoi == engagement::departs::Envoi::Documents && d.mois.is_some_and(|m| m == mois) && d.par.is_none())
            .map(|d| d.le)
            .max();
        let Some(quand) = dernier else { continue };
        if a_envoyer.contains(cle) {
            continue;
        }
        let plus_recent = out.get(cle).is_some_and(|v| v.quand >= quand);
        if !plus_recent {
            out.insert(cle.clone(), DocumentsPartis { resume: resumes.get(cle).cloned(), quand });
        }
    }
    Ok(out)
}
/// La phrase de la pastille Documents d'un salarie dont la paie a change depuis l'envoi.
pub(crate) const DOCUMENTS_CADUCS: &str = "la paie a changé depuis l'envoi";

/// LA PASTILLE DOCUMENTS d'un engagement pour un mois. `parti_le` : le dernier envoi de ses documents ; `caduc` : ils
/// sont partis, et sa paie a change depuis la version sur laquelle ils sont partis (`engagement::etats::a_refaire`) --
/// ils ne sont plus les bons, ils sont a renvoyer.
fn etat_documents(parti_le: Option<InstantOuJour>, caduc: bool) -> EtatDocuments {
    let d = EtatDocuments { fait: parti_le, caduc: false, pourquoi: "" };
    let caduc = d.parti() && caduc;
    EtatDocuments { caduc, pourquoi: if caduc { DOCUMENTS_CADUCS } else { "" }, ..d }
}

/// `DSN_[^\W\d_]+\d{4}(_AR\d+)?\.dsn` : le nom d'une DSN mensuelle.
pub(crate) fn est_une_dsn_mensuelle(nom: &str) -> bool {
    match documents::SorteDePieceProduite::du_nom(nom) {
        Some((documents::SorteDePieceProduite::Dsn, reste)) => reste.strip_suffix(".dsn").is_some_and(nom_du_mois_et_rang),
        _ => false,
    }
}

/// `[^\W\d_]+\d{4}(_AR\d+)?` : ce que le nom d'une DSN mensuelle porte entre `DSN_` et `.dsn`.
fn nom_du_mois_et_rang(reste: &str) -> bool {
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
pub(crate) fn etat_signature(
    fait: Option<InstantOuJour>, par: Option<documents::ModeDeRemise>, recu: Option<&Moment>, renvoye: bool, avenants: Option<&Vec<envois::journal::AvenantParti>>,
    a_refaire_signer: bool, ailleurs: bool,
) -> Signature {
    let recu = if renvoye { None } else { recu.cloned() };
    let parti = fait.is_some();
    // Un contrat signe hors du logiciel (`ailleurs`) vaut un contrat parti : un terme qui change s'y dit par un avenant.
    let caduc = (parti || ailleurs) && a_refaire_signer;
    // L'AVENANT : le dernier parti. Pas encore revenu, il laisse la pastille orange.
    let dernier = avenants.and_then(|a| a.last()).cloned();
    let attendu = dernier.as_ref().is_some_and(|d| d.signe.is_none());
    let pourquoi = match (caduc, ailleurs) {
        (false, false) | (false, true) => "",
        (true, false) => CONTRAT_A_REFAIRE,
        (true, true) => CONTRAT_AILLEURS_A_MODIFIER,
    };
    Signature {
        fait,
        par: if parti { par } else { None },
        recu,
        caduc,
        pourquoi,
        avenant: dernier,
        avenant_attendu: attendu,
        cdi_en_cours: None,
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
        let cle = b.saisie.carte.salarie.clone();
        // Un salarie qu'aucune fiche ne porte se nomme par sa cle.
        let nom = cle.as_ref().map(|c| match ctx.salaries.get(c) {
            Some(f) => super::nom_a_l_ecran(f, c).trim().to_string(),
            None => c.to_string(),
        });
        let contrat = b.saisie.carte.contrat()?.ok_or_else(|| socle::fichiers::absent("debut"))?;
        let dit = match &b.saisie.carte.numero {
            Some(n) => format!("contrat {}, {}", n, crate::contrats::periode_dite(&contrat)),
            None => format!("contrat sans numéro, {}", crate::contrats::periode_dite(&contrat)),
        };
        out.push(NonActif {
            salarie: cle,
            nom,
            numero: b.saisie.carte.numero.clone(),
            dit,
            contrat,
            non_actif: socle::paris::ecrit::seconde::texte(n.le()),
            non_actif_dit: format!("non actif depuis le {}", n.le().dit()),
            mot: "non actif",
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
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// LES DEUX ETATS DE `mois`, servis par une meme lecture (`Moteur`) : les lignes de chaque engagement et ceux qui ne sont
/// pas actifs (ce que `etat-par-engagement` rend : `par_engagement`), et le bilan du mois (`etat-du-mois` : `bilan`), tels
/// que l'etat garde d'un mois les garde (`etat_garde`). A LA LECTURE, UNE SEULE SURCOUCHE : les envois du mois en cours
/// (`envois_en_fond::EnvoisDuMois`) se disent a la place des gestes qu'ils font, dans l'un comme dans l'autre.
pub(crate) struct LesDeuxEtats {
    mois: Mois,
    lignes: std::sync::Arc<Vec<Ligne>>,
    non_actifs: Vec<NonActif>,
    bilan: Bilan,
}

impl LesDeuxEtats {
    /// Le bilan du mois, tel que la page le lit : celui qui est garde, et les envois en cours a la place de leurs gestes.
    pub(crate) fn bilan(&self, envois: &crate::envois_en_fond::EnvoisDuMois) -> std::borrow::Cow<'_, Bilan> {
        if envois.aucun_en_cours() {
            return std::borrow::Cow::Borrowed(&self.bilan);
        }
        let mut b = self.bilan.clone();
        b.pendant_les_envois(envois);
        std::borrow::Cow::Owned(b)
    }

    /// Le bilan garde, sans les envois (ce que le filet emporte, la phrase de l'ecran du mois).
    pub(crate) fn bilan_garde(&self) -> &Bilan {
        &self.bilan
    }

    /// `etat-par-engagement` : la phrase du mois, et ce que la page lit de chaque engagement ; un envoi en cours de ce
    /// mois se dit a la place des gestes de son etape (les carres, la boite des DPAE).
    pub(crate) fn par_engagement(&self, envois: &crate::envois_en_fond::EnvoisDuMois) -> R<EtatParEngagement<'_>> {
        let engagements: Vec<LigneDePage> = self.lignes.iter().map(|l| LigneDePage::de(l, self.mois, envois)).collect();
        let phrase = match engagements.len() {
            0 => socle::phrase!("Aucun engagement enregistré en {}.", self.mois.libelle(socle::mois::Graphie::Accentuee)),
            n => socle::phrase!("Engagements du mois : {}.", n),
        };
        let phrases = engagements.iter().map(phrase_de_la_ligne).collect();
        // Les embauches cochees partent par l'apercu du depot, qui ecrit leur DPAE ; son test part tout de suite. Celles
        // dont la DPAE est en cours d'envoi ne se cochent pas.
        let dpae_envoyer = engagements.iter().any(|l| l.dpae_choix.cochable).then(|| {
            let mut charge = Designation::du_mois(Quoi::Dpae, self.mois);
            charge.engagements = Some(Vec::new());
            Action { tester: true, ..vers(VueDuGeste::Depot, charge, Some(Route::Deposer), "Envoyer le test".into()) }
        });
        Ok(EtatParEngagement {
            phrase,
            phrases,
            mois: self.mois,
            engagements,
            non_actifs: &self.non_actifs,
            dpae_titre: "Envoyer les DPAE",
            dpae_notes: [crate::ouverture::aide("dpae_cochees")?, crate::ouverture::aide("dpae_heure")?],
            dpae_envoyer,
            dpae_rien: "Rien de coché",
        })
    }
}

pub(crate) fn les_deux_etats(ctx: &Contexte, mois: Mois) -> R<LesDeuxEtats> {
    let mut m = Moteur::nouveau(ctx);
    let lignes = m.etat_par_engagement(mois)?;
    let non_actifs = non_actifs(ctx, mois)?;
    let bilan = m.bilan_du_mois(mois)?;
    Ok(LesDeuxEtats { mois, lignes, non_actifs, bilan })
}

/// LE MOIS A OUVRIR (`mois-a-ouvrir`), tel que l'etat garde de l'ouverture le garde : le mois, et la reponse rendue.
pub(crate) fn le_mois_a_ouvrir(ctx: &Contexte) -> R<(Mois, Rendu)> {
    let m = Moteur::nouveau(ctx).mois_a_ouvrir()?;
    Ok((m.mois, Rendu::de(&m)?))
}

/// Ce que la DPAE d'une embauche declare, en 16 caracteres : l'identite du salarie telle que l'engagement la lit, les
/// la periode du contrat, le spectacle, la nature ; pas les taux ni les cachets. Un test de DPAE fait sur une autre empreinte
/// ne vaut plus.
fn empreinte_de_l_embauche(e: &engagement::Engagement, sal: &droit::fiche::ChampsSalarie) -> EmpreinteCourte {
    /// Ce que l'empreinte couvre, ecrit en JSON : une case vide y est `null`.
    #[derive(serde::Serialize)]
    struct Embauche<'a> {
        contrat: PeriodeOuverte,
        spectacle: Option<&'a droit::contexte::IdSpectacle>,
        nir: Option<&'a droit::fiche::Nir>,
        nom: Option<&'a str>,
        prenom: Option<&'a str>,
        date_naissance: Option<Jour>,
        lieu_naissance: Option<&'a str>,
        departement_naissance: Option<&'a str>,
        sexe: Option<&'a str>,
        nature: &'a str,
    }
    let embauche = Embauche {
        contrat: e.contrat(),
        spectacle: e.spectacle.as_ref(),
        nir: sal.nir.dite(),
        nom: sal.nom.portee(),
        prenom: sal.prenom.portee(),
        date_naissance: sal.date_naissance.dite().copied(),
        lieu_naissance: sal.lieu_naissance.portee(),
        departement_naissance: sal.departement_naissance.portee(),
        sexe: sal.sexe.code(),
        nature: e.nature.cle(),
    };
    Empreinte::des_octets(socle::ecriture::json_en_ligne(&embauche).as_bytes()).courte()
}
