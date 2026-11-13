//! LA LIGNE DU JOURNAL DES ENVOIS, telle que `journal` l'ecrit et la relit : une structure par sorte d'envoi (`Ligne`,
//! etiquetee par `quoi`), qui porte ce que toutes partagent (`Envoi` : quand, mode, accepte, flux, versions...), le
//! detail de sa sorte et ce qui en revient (`retours`). Chaque forme se lit strictement (`deny_unknown_fields`) : une
//! ligne qui porte une cle inconnue ne se lit pas. Le numero d'un contrat y est un `NumeroContrat`, le mois un `Mois`,
//! le salarie un `IdSalarie`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use droit::contexte::IdSalarie;
use documents::fiche_questionnaire::CodeDeDemande;
use droit::fiche::{Courriel, Nic};
use droit::nature_declaration::NatureDeLaDeclaration;
use droit::type_declaration::TypeDeLaDeclaration;
use engagement::numeros::NumeroContrat;
use socle::fichiers::{CheminRelatif, EmpreinteCourte, NomDeFichier};
use socle::mois::Mois;

use crate::envoi::IdDeCourriel;
use crate::retours::{IdDeFlux, StatutDuRetour, Verdict};

/// Le mode d'un envoi : reel (il engage), ou test (un depot de test, qui ne conserve rien).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Reel,
    Test,
}

impl Mode {
    pub fn de(reel: bool) -> Mode {
        if reel {
            Mode::Reel
        } else {
            Mode::Test
        }
    }

    /// Son nom, tel que la ligne l'ecrit.
    pub fn nom(self) -> &'static str {
        match self {
            Mode::Reel => "reel",
            Mode::Test => "test",
        }
    }
}

/// La sorte d'un envoi, telle que `quoi` l'ecrit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Acte {
    Dsn,
    Dpae,
    Documents,
    ContratSignature,
    ContratContresigne,
    RegularisationCasc,
    DossierSalarie,
    FicheDemande,
}

impl Acte {
    /// Son nom, tel que la ligne l'ecrit.
    pub fn nom(self) -> &'static str {
        match self {
            Acte::Dsn => "dsn",
            Acte::Dpae => "dpae",
            Acte::Documents => "documents",
            Acte::ContratSignature => "contrat-signature",
            Acte::ContratContresigne => "contrat-contresigne",
            Acte::RegularisationCasc => "regularisation-casc",
            Acte::DossierSalarie => "dossier-salarie",
            Acte::FicheDemande => "fiche-demande",
        }
    }
}

/// Ce qui ne revient jamais d'un envoi (des documents, un contrat contresigne...) : sa liste `retours` reste vide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Aucun {}

/// LA VERSION D'UN ENGAGEMENT QU'UN ENVOI EMPORTE (`journal::versions_du_mois`) : la cle du salarie, le numero du contrat
/// et la version ; `mois`, pour la correction d'un mois passe que porte une DSN (`produire_regularisations`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    pub salarie: IdSalarie,
    pub numero: NumeroContrat,
    pub version: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mois: Option<Mois>,
}

/// CE QUE TOUTE LIGNE PORTE, et le detail (`D`) et les retours (`R`) de sa sorte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envoi<D, R> {
    /// L'heure de l'envoi (a la seconde), ou celle qu'une reprise lit dans sa piece (`journal::noter`).
    pub(crate) quand: socle::paris::InstantOuJour,
    /// D'ou vient une heure qui n'est pas celle de l'acte (une reprise, l'horloge d'une fictive) ; vide sinon.
    pub(crate) origine: String,
    pub mois: Option<Mois>,
    /// Le nom du fichier parti, sans son dossier.
    pub fichier: Option<NomDeFichier>,
    pub empreinte: Option<EmpreinteCourte>,
    pub mode: Mode,
    pub qui: String,
    pub accepte: bool,
    pub destinataire: String,
    /// Le flux que la plate-forme a donne au depot ; `""` au fichier quand elle n'en a pas donne.
    #[serde(with = "socle::forme::vide_si_aucun")]
    pub flux: Option<IdDeFlux>,
    pub retours: Vec<R>,
    pub reponse: String,
    /// La reponse de l'organisme archivee, relative a la compagnie.
    pub trace: Option<CheminRelatif>,
    pub detail: D,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versions: Option<Vec<Version>>,
}

/// UNE LIGNE DU JOURNAL : une sorte d'envoi, son detail, ses retours.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "quoi", rename_all = "kebab-case")]
pub enum Ligne {
    Dsn(Envoi<DetailDsn, RetourOrganisme>),
    Dpae(Envoi<DetailDpae, RetourOrganisme>),
    Documents(Envoi<DetailDocuments, Aucun>),
    ContratSignature(Envoi<DetailSignature, RetourContrat>),
    ContratContresigne(Envoi<DetailContresigne, Aucun>),
    RegularisationCasc(Envoi<DetailCasc, Aucun>),
    DossierSalarie(Envoi<DetailDossier, Aucun>),
    FicheDemande(Envoi<DetailFiche, RetourFiche>),
}

/// `$x` sur la part commune (`Envoi`) de la ligne `$l`, quelle que soit sa sorte.
macro_rules! commun {
    ($l:expr, $e:ident => $x:expr) => {
        match $l {
            Ligne::Dsn($e) => $x,
            Ligne::Dpae($e) => $x,
            Ligne::Documents($e) => $x,
            Ligne::ContratSignature($e) => $x,
            Ligne::ContratContresigne($e) => $x,
            Ligne::RegularisationCasc($e) => $x,
            Ligne::DossierSalarie($e) => $x,
            Ligne::FicheDemande($e) => $x,
        }
    };
}

impl Ligne {
    pub fn acte(&self) -> Acte {
        match self {
            Ligne::Dsn(_) => Acte::Dsn,
            Ligne::Dpae(_) => Acte::Dpae,
            Ligne::Documents(_) => Acte::Documents,
            Ligne::ContratSignature(_) => Acte::ContratSignature,
            Ligne::ContratContresigne(_) => Acte::ContratContresigne,
            Ligne::RegularisationCasc(_) => Acte::RegularisationCasc,
            Ligne::DossierSalarie(_) => Acte::DossierSalarie,
            Ligne::FicheDemande(_) => Acte::FicheDemande,
        }
    }

    pub fn quand(&self) -> socle::paris::InstantOuJour {
        commun!(self, e => e.quand)
    }

    pub fn origine(&self) -> &str {
        commun!(self, e => &e.origine)
    }

    pub fn mois(&self) -> Option<Mois> {
        commun!(self, e => e.mois)
    }

    pub fn fichier(&self) -> Option<&NomDeFichier> {
        commun!(self, e => e.fichier.as_ref())
    }

    pub fn empreinte(&self) -> Option<&EmpreinteCourte> {
        commun!(self, e => e.empreinte.as_ref())
    }

    pub fn mode(&self) -> Mode {
        commun!(self, e => e.mode)
    }

    pub fn reel(&self) -> bool {
        self.mode() == Mode::Reel
    }

    pub fn qui(&self) -> &str {
        commun!(self, e => &e.qui)
    }

    pub fn accepte(&self) -> bool {
        commun!(self, e => e.accepte)
    }

    pub fn destinataire(&self) -> &str {
        commun!(self, e => &e.destinataire)
    }

    pub fn flux(&self) -> Option<&IdDeFlux> {
        commun!(self, e => e.flux.as_ref())
    }

    pub fn reponse(&self) -> &str {
        commun!(self, e => &e.reponse)
    }

    pub fn trace(&self) -> Option<&CheminRelatif> {
        commun!(self, e => e.trace.as_ref())
    }

    pub fn versions(&self) -> Option<&[Version]> {
        commun!(self, e => e.versions.as_deref())
    }

    /// Le nombre de retours rattaches.
    pub fn nombre_de_retours(&self) -> usize {
        commun!(self, e => e.retours.len())
    }

    /// Les retours des organismes d'une DSN ou d'une DPAE ; aucun pour un autre envoi.
    pub fn retours_organisme(&self) -> &[RetourOrganisme] {
        match self {
            Ligne::Dsn(e) => &e.retours,
            Ligne::Dpae(e) => &e.retours,
            _ => &[],
        }
    }

    /// Le detail d'une DSN.
    pub fn dsn(&self) -> Option<&Envoi<DetailDsn, RetourOrganisme>> {
        match self {
            Ligne::Dsn(e) => Some(e),
            _ => None,
        }
    }

    /// Le detail d'une DPAE.
    pub fn dpae(&self) -> Option<&Envoi<DetailDpae, RetourOrganisme>> {
        match self {
            Ligne::Dpae(e) => Some(e),
            _ => None,
        }
    }

    pub fn signature(&self) -> Option<&Envoi<DetailSignature, RetourContrat>> {
        match self {
            Ligne::ContratSignature(e) => Some(e),
            _ => None,
        }
    }

    pub fn documents(&self) -> Option<&Envoi<DetailDocuments, Aucun>> {
        match self {
            Ligne::Documents(e) => Some(e),
            _ => None,
        }
    }

    pub fn fiche(&self) -> Option<&Envoi<DetailFiche, RetourFiche>> {
        match self {
            Ligne::FicheDemande(e) => Some(e),
            _ => None,
        }
    }

    /// Un envoi remis en main propre (des contrats ou des documents imprimes).
    pub fn main_propre(&self) -> bool {
        match self {
            Ligne::ContratSignature(e) => e.detail.main_propre,
            Ligne::Documents(e) => e.detail.main_propre,
            Ligne::Dsn(_) | Ligne::Dpae(_) | Ligne::ContratContresigne(_) | Ligne::RegularisationCasc(_) | Ligne::DossierSalarie(_) | Ligne::FicheDemande(_) => false,
        }
    }

    /// L'etablissement (son NIC) qu'une DSN declare ; `None` : l'etablissement principal.
    pub fn etablissement(&self) -> Option<&Nic> {
        self.dsn().and_then(|e| e.detail.etablissement.as_ref())
    }

    /// La nature d'une DSN (`01` mensuelle, `04` `05` `07` `08` un signalement) ; `None` si la ligne ne la dit pas.
    pub fn nature_dsn(&self) -> Option<NatureDeLaDeclaration> {
        self.dsn().and_then(|e| e.detail.nature_declaration)
    }

    /// Une DSN de signalement (arret, reprise, fin de contrat, amorcage).
    pub fn signalement(&self) -> bool {
        self.nature_dsn().is_some_and(NatureDeLaDeclaration::signalement_envoye)
    }
}

// ------------------------------------------------------------------ la DSN et la DPAE

/// CE QUI A OUVERT UN DEPOT REEL (`routes::gestes::depot_acte::exiger_repetition`) : le test conforme de ce contenu
/// (son heure, son flux, son compte rendu), ou le motif ecrit d'un reel parti sans test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ouverture {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flux: Option<IdDeFlux>,
    pub empreinte: EmpreinteCourte,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compte_rendu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contournement: Option<String>,
}

/// LE DETAIL D'UNE DSN : ce que le fichier deposé declare (sa nature, son type, son ordre, son etablissement, ses
/// versements), ce qui l'a ouverte, ce qui l'a fait echouer (`erreur`) ; pour un depot repris des retours
/// (`reconstruit`), ce que son bilan en dit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailDsn {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preuve: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub erreur: Option<String>,
    pub reconstruit: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logiciel: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_declaration: Option<TypeDeLaDeclaration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fraction: Option<droit::natures::Fraction>,
    #[serde(rename = "nature", skip_serializing_if = "Option::is_none")]
    pub nature_declaration: Option<NatureDeLaDeclaration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordre: Option<droit::natures::NumeroDOrdre>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versements: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salaries: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrats: Option<String>,
    /// Ce qui a ete preleve sur la grille, et non sur le taux du salarie.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pas_bareme: Option<usize>,
    pub repetition: Option<Ouverture>,
    /// Ecrit en nombre, `null` quand il n'y en a pas ; une cle absente se lit vide, comme un `Option` ordinaire.
    #[serde(default, with = "droit::natures::en_nombre::option")]
    pub ordre_test: Option<droit::natures::NumeroDOrdre>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etablissement: Option<Nic>,
    /// Le jour de l'evenement qu'un signalement declare (le dernier jour travaille d'un arret, la fin d'un contrat...) ; la
    /// ligne porte le mois de ce jour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evenement: Option<socle::date::Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etat_envoi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etat_declaration: Option<Verdict>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict_connu: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pieces: Option<Vec<NomDeFichier>>,
}

impl DetailDsn {
    /// Le detail d'un depot de ce logiciel (`reconstruit` faux) ou repris des retours, dont rien d'autre n'est encore dit.
    pub fn nouveau(reconstruit: bool) -> DetailDsn {
        DetailDsn {
            preuve: None,
            erreur: None,
            reconstruit,
            logiciel: None,
            type_declaration: None,
            fraction: None,
            nature_declaration: None,
            ordre: None,
            versements: None,
            salaries: None,
            contrats: None,
            pas_bareme: None,
            repetition: None,
            ordre_test: None,
            etablissement: None,
            evenement: None,
            etat_envoi: None,
            etat_declaration: None,
            verdict_connu: None,
            pieces: None,
        }
    }
}

/// Une embauche qu'une DPAE declare : le salarie, le numero du contrat de son engagement, son premier et son dernier
/// jour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbaucheDeclaree {
    pub salarie: IdSalarie,
    pub numero: Option<NumeroContrat>,
    pub debut: socle::date::Jour,
    pub fin: Option<socle::date::Jour>,
}

/// LE DETAIL D'UNE DPAE : ce qui l'a ouverte, ses embauches, ce qui l'a fait echouer ; pour une DPAE reprise des
/// certificats (`reconstruit`), les embauches qu'ils attestent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailDpae {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preuve: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub erreur: Option<String>,
    pub reconstruit: bool,
    pub repetition: Option<Ouverture>,
    pub embauches: Vec<EmbaucheDeclaree>,
    pub certificat: bool,
}

impl DetailDpae {
    /// Le detail d'un depot de ce logiciel (`reconstruit` faux) ou repris des certificats, sans embauche encore dite.
    pub fn nouveau(reconstruit: bool, certificat: bool) -> DetailDpae {
        DetailDpae { preuve: None, erreur: None, reconstruit, repetition: None, embauches: Vec::new(), certificat }
    }
}

/// Une personne du certificat de conformite d'une DPAE (nature 41, `declarations::dpae::lire_certificat`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonneCertifiee {
    pub nom: String,
    pub prenom: String,
    /// Le premier jour que le certificat dit ; aucun : sa zone est a blanc (le journal l'ecrit `""`).
    #[serde(with = "socle::forme::vide_si_aucun")]
    pub embauche: Option<socle::date::Jour>,
    pub dossier: String,
    /// C_RETOUR_AR.
    pub code: declarations::dpae::CodeRetourDpae,
    /// Le libelle de son code (Format 120).
    pub libelle: String,
    pub enregistree: bool,
    pub deja_existante: bool,
    pub formalites: Vec<declarations::dpae::FormaliteDpae>,
    /// Ce que la page en montre (`routes::depots`) : pose sur ce qu'une route rend, jamais ecrit ni lu au journal.
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub pastille: Option<socle::ecran::Pastille>,
}

impl PersonneCertifiee {
    /// Refusee : un code retour qui ne l'enregistre pas et ne la dit pas deja existante.
    pub fn refusee(&self) -> bool {
        !self.enregistree && !self.deja_existante
    }

    /// Sa pastille : enregistree, faite ; deja existante, une information ; refusee, refusee.
    pub fn pastille_due(&self) -> socle::ecran::Pastille {
        use socle::ecran::{EtatDEcran, Pastille};
        if self.enregistree {
            Pastille::de(EtatDEcran::Fait, &if self.dossier.is_empty() { "enregistrée".to_string() } else { format!("enregistrée — dossier {}", self.dossier) })
        } else if self.deja_existante {
            Pastille::de(EtatDEcran::Information, "déjà existante")
        } else {
            Pastille::de(EtatDEcran::Refuse, self.code.libelle())
        }
    }
}

/// CE QU'UN COMPTE RENDU DIT, lu dans sa piece (`etats::verdict_du_retour`) : son verdict, ses anomalies en phrases, et
/// pour le certificat d'une DPAE, chaque personne.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerdictDeLaPiece {
    pub verdict: Verdict,
    pub anomalies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dpae: Option<Vec<PersonneCertifiee>>,
}

/// CE QU'UN ORGANISME A RENDU d'une DSN ou d'une DPAE : sa nature, son libelle, son statut, sa publication, et ce que sa
/// piece en dit quand elle s'est lue (`verdict`, `anomalies`, `dpae`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetourOrganisme {
    /// La nature du retour (un compte rendu, un bilan, un certificat...).
    #[serde(rename = "nature")]
    pub nature_retour: crate::retours::NatureDuRetour,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<Verdict>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomalies: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dpae: Option<Vec<PersonneCertifiee>>,
    pub libelle: String,
    pub statut: Option<StatutDuRetour>,
    /// L'heure ou la plate-forme l'a mis a disposition (`retours::Annonce`).
    #[serde(with = "socle::paris::ecrit::compacte::option")]
    pub publication: Option<socle::paris::Moment>,
}

impl RetourOrganisme {
    pub fn verdict(&self) -> Option<Verdict> {
        self.verdict
    }

    pub fn ko(&self) -> bool {
        self.verdict == Some(Verdict::Ko)
    }

    pub fn anomalies(&self) -> Option<&[String]> {
        self.anomalies.as_deref()
    }

    pub fn personnes(&self) -> Option<&[PersonneCertifiee]> {
        self.dpae.as_deref()
    }

    /// Le meme compte rendu (sa nature et sa publication).
    pub fn meme(&self, autre: &RetourOrganisme) -> bool {
        self.nature_retour == autre.nature_retour && self.publication == autre.publication
    }

    /// Ce que sa piece en dit, pose sur lui.
    pub fn poser(&mut self, v: VerdictDeLaPiece) {
        self.verdict = Some(v.verdict);
        self.anomalies = Some(v.anomalies);
        if v.dpae.is_some() {
            self.dpae = v.dpae;
        }
    }

    /// Ce qu'un autre poste en savait de plus (un verdict), sans rien retirer.
    pub fn completer(&mut self, autre: &RetourOrganisme) {
        if self.verdict.is_none() {
            self.verdict = autre.verdict;
        }
        if self.anomalies.is_none() {
            self.anomalies = autre.anomalies.clone();
        }
        if self.dpae.is_none() {
            self.dpae = autre.dpae.clone();
        }
    }
}

// ------------------------------------------------------------------ les envois au salarie

/// Les durees d'un envoi de documents, en millisecondes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Durees {
    pub preparation: u64,
    pub mot_de_passe: u64,
    pub envoi: u64,
}

/// Un salarie que des documents ont servi : ses pieces, le resume de sa paie, et si c'etait un renvoi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Servi {
    pub cle: IdSalarie,
    pub nom: Option<String>,
    pub pieces: Vec<NomDeFichier>,
    pub resume: Option<crate::envoi::ResumeDePaie>,
    pub renvoi: bool,
}

/// LE DETAIL D'UN ENVOI DE DOCUMENTS : les salaries servis, imprimes (`main_propre`) ou partis par courriel (de
/// `expediteur`) ; un envoi coupe en route dit ceux qui ont recu (`partis`) et l'adresse ou il a echoue (`echec_a`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailDocuments {
    pub salaries: Vec<Servi>,
    pub main_propre: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expediteur: Option<Courriel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durees_ms: Option<Durees>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partis: Option<Vec<Courriel>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echec_a: Option<Courriel>,
}

/// UN CONTRAT (OU UN AVENANT) PARTI A LA SIGNATURE : son engagement (salarie, bornes, numero), sa piece, ce que le
/// contrat dit (`clauses`), et ce qu'il faut pour reconnaitre la reponse (le code, l'adresse, le message). Un avenant
/// (`acte`) dit son numero, son effet et ses modifications ; un contrat refait, celui qu'il remplace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContratParti {
    pub salarie: IdSalarie,
    pub debut: socle::date::Jour,
    pub fin: Option<socle::date::Jour>,
    pub numero: Option<NumeroContrat>,
    pub fichier: NomDeFichier,
    pub spectacle: Option<String>,
    pub mois: Mois,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acte: Option<ActeDuContrat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avenant: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remplace: Option<NumeroContrat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effet: Option<socle::date::Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifie: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifications: Option<Vec<ArticleModifie>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clauses: Option<BTreeMap<documents::marqueurs::Marqueur, Option<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeDeDemande>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adresse: Option<Courriel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relance: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empreinte: Option<EmpreinteCourte>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<IdDeCourriel>,
    pub ailleurs: bool,
    /// L'empreinte du modele avec lequel ce contrat est parti (son texte et celui de son avenant), fige sous ce nom avec la compagnie
    /// (`routes::modeles_contrat::figer`) ; un avenant ne la porte pas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modele: Option<EmpreinteCourte>,
}

impl ContratParti {
    /// Un avenant au contrat, pas le contrat.
    pub fn est_avenant(&self) -> bool {
        self.acte == Some(ActeDuContrat::Avenant)
    }
}

/// CE QU'EST UN CONTRAT PARTI QUI N'EST PAS LE CONTRAT LUI-MEME : un avenant (`avenant`), ou le contrat refait qui annule
/// et remplace celui qui est parti (`remplace`). Une liste du logiciel, qu'il ecrit lui-meme ; aucun acte : le contrat
/// lui-meme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActeDuContrat {
    Avenant,
    Remplace,
}

/// D'OU EST VENUE LA REPONSE A UN CONTRAT PARTI : un « Je signe » releve par courriel (`courriel`). Une liste du
/// logiciel, qu'il ecrit lui-meme ; aucune : la mention posee a la main.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrigineDuRetourDeContrat {
    Courriel,
}

/// CE QU'UN AVENANT CHANGE, un article ou un terme : l'article du modele de contrat et son numero, l'ancien texte et le
/// nouveau (`documents::avenant::modifications`, les paragraphes `blocs` de l'article) ; ou, pour un contrat signe hors du
/// logiciel, le terme dit en clair (`phrase`), sans numero, et le jour ou il change (`documents::avenant::changements_en_clair`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticleModifie {
    pub numero: Option<usize>,
    pub article: String,
    pub avant: Vec<String>,
    pub apres: Vec<String>,
    pub blocs: Vec<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phrase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jour: Option<socle::date::Jour>,
}

impl ArticleModifie {
    /// Un terme dit en clair (un contrat signe hors du logiciel), pas un article cite.
    pub fn en_clair(&self) -> bool {
        self.phrase.as_deref().is_some_and(|p| !p.is_empty())
    }
}

/// LE DETAIL D'UN ENVOI A LA SIGNATURE : ses contrats (ou ses avenants, `acte`), imprimes (`main_propre`) ou partis par
/// courriel (de `expediteur`, une relance ou non) ; un envoi coupe en route dit ceux qui ont recu et ou il a echoue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailSignature {
    pub contrats: Vec<ContratParti>,
    pub main_propre: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expediteur: Option<Courriel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relance: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acte: Option<ActeDuContrat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partis: Option<Vec<Courriel>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echec_a: Option<Courriel>,
}

/// CE QUI REVIENT D'UN CONTRAT PARTI (`routes::contrats::contrat_recu_avec`) : le contrat (salarie, debut,
/// numero, avenant), s'il est revenu signe (`recu`, ou la mention retiree), et, pour une reponse « Je signe » relevee
/// par courriel (`origine`), ce qui la prouve. `quand`, `qui`, `reponse` : la note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetourContrat {
    pub salarie: IdSalarie,
    pub debut: socle::date::Jour,
    pub recu: bool,
    pub numero: Option<NumeroContrat>,
    pub nom: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avenant: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origine: Option<OrigineDuRetourDeContrat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeDeDemande>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adresse: Option<Courriel>,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub quand: socle::paris::Moment,
    /// L'identifiant de la reponse dans la boite (son UID IMAP) : celui d'un message recu, pas un `Message-ID`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preuve: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fichier_signe: Option<NomDeFichier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mention: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acte: Option<ActeDuContrat>,
    pub qui: String,
    pub reponse: String,
}

impl RetourContrat {
    /// Une reponse « Je signe » relevee par courriel.
    pub fn par_courriel(&self) -> bool {
        self.origine == Some(OrigineDuRetourDeContrat::Courriel)
    }
}

/// UN AVENANT PARTI (`etats::avenants_par_contrat`) : son numero, quand et dans quel mois il est parti, sa piece, son code
/// de signature, le contrat qu'il modifie, son effet, ses modifications, le jour ou il est revenu signe, et si un
/// courriel a deja demande de signer cette piece (`par_courriel`) : la redemander par courriel est alors un rappel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AvenantParti {
    pub numero: Option<i64>,
    pub quand: socle::paris::InstantOuJour,
    pub mois: Mois,
    pub fichier: NomDeFichier,
    pub code: Option<CodeDeDemande>,
    pub contrat: Option<NumeroContrat>,
    pub effet: Option<socle::date::Jour>,
    pub modifications: Vec<ArticleModifie>,
    #[serde(with = "socle::paris::ecrit::seconde::option")]
    pub signe: Option<socle::paris::Moment>,
    pub par_courriel: bool,
}

/// UN CONTRAT QUE LA SIGNATURE ATTEND (`etats::signatures_attendues`), et que le contrat contresigne renvoie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContratAttendu {
    pub salarie: IdSalarie,
    pub debut: socle::date::Jour,
    pub fin: Option<socle::date::Jour>,
    pub numero: Option<NumeroContrat>,
    pub spectacle: Option<String>,
    pub fichier: NomDeFichier,
    pub avenant: Option<i64>,
    pub mois: Mois,
}


/// LE DETAIL DU CONTRAT CONTRESIGNE RENVOYE AU SALARIE : qui, sous quel code, ses contrats, les PDF contresignes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailContresigne {
    pub salarie: IdSalarie,
    pub nom: String,
    pub code: CodeDeDemande,
    pub contrats: Vec<ContratAttendu>,
    pub fichiers: Vec<NomDeFichier>,
}

/// Une annee de la regularisation CASC-SVP : son montant, et le jour du virement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnneeCasc {
    pub annee: socle::mois::Annee,
    pub vire_le: socle::date::Jour,
    pub montant: rust_decimal::Decimal,
}

/// LE DETAIL DE LA REGULARISATION CASC-SVP envoyee a Audiens et au CASC-SVP : ses annees, ses mois, son total, les
/// messages gardes (`eml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailCasc {
    pub annees: Vec<AnneeCasc>,
    pub mois: Vec<Mois>,
    pub total: rust_decimal::Decimal,
    pub expediteur: Courriel,
    pub eml: Vec<NomDeFichier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partis: Option<Vec<Courriel>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echec_a: Option<Courriel>,
}

/// Le dossier qu'un salarie a recu : sa piece.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DossierServi {
    pub cle: IdSalarie,
    pub piece: Option<NomDeFichier>,
}

/// LE DETAIL DE L'ENVOI DU DOSSIER AUX SALARIES : la periode, ce qu'il porte, qui l'a recu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailDossier {
    pub periode: String,
    pub categories: Vec<documents::CategorieDeDocument>,
    pub salaries: Vec<DossierServi>,
    pub expediteur: Courriel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partis: Option<Vec<Courriel>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echec_a: Option<Courriel>,
}

/// LE DETAIL D'UNE DEMANDE DE FICHE : son code, a qui, et l'identifiant du message parti.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailFiche {
    pub code: CodeDeDemande,
    pub adresse: Courriel,
    /// Le prenom pour lui ecrire, s'il a ete dit.
    pub prenom: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<IdDeCourriel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expediteur: Option<Courriel>,
}

/// CE QUI REVIENT D'UNE DEMANDE DE FICHE : la fiche recue (la cle de la fiche, illisible ou non, ses erreurs, le
/// message), ou la demande abandonnee.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetourFiche {
    pub code: CodeDeDemande,
    pub abandon: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salarie: Option<IdSalarie>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub illisible: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub erreurs: Option<usize>,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub quand: socle::paris::Moment,
    pub qui: String,
    /// L'adresse d'ou la reponse est venue (`courriel_lecture::Expediteur::Adresse`, la seule retenue).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub de: Option<Courriel>,
    /// L'identifiant de la reponse dans la boite (son UID IMAP) : celui d'un message recu, pas un `Message-ID`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub reponse: String,
}

/// UN ENVOI COUPE EN ROUTE (`ligne_d_envoi::noter_echec_d_envoi`) : son detail dit ceux qui ont recu et l'adresse ou il
/// a echoue.
pub trait EnRoute {
    fn coupe(&mut self, partis: Vec<Courriel>, echec_a: Courriel);
}

macro_rules! en_route {
    ($($t:ty),*) => {
        $(impl EnRoute for $t {
            fn coupe(&mut self, partis: Vec<Courriel>, echec_a: Courriel) {
                self.partis = Some(partis);
                self.echec_a = Some(echec_a);
            }
        })*
    };
}

en_route!(DetailDocuments, DetailSignature, DetailCasc, DetailDossier);
