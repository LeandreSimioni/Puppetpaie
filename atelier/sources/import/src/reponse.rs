//! LA REPONSE DE L'IMPORT (`importation-apercu`, `importer`) : ce que l'apercu reprendrait, ou ce que l'import a repris,
//! telle que la page la lit -- le dossier fichier par fichier (`TriVu`), les declarations, les bulletins, les certificats
//! et les AEM en PDF lus, chaque fiche que l'import cree, complete ou laisse (`FicheImportee`), et leurs phrases
//! (`Importation::dire`, une fois tout ecrit).
//!
//! Ce que porte une fiche, et la case d'un ecart, sont des saisies de leur fiche (`declarations::champs::Saisie`,
//! `Valeur`) ; ses anomalies, celles de sa verification (`declarations::champs::Anomalie`).

use serde::Serialize;

use declarations::champs::{Anomalie, Saisie, Valeur};
use droit::fiche::{CaseDeFiche, FicheNommee, NomDeFiche};

use socle::ecran::{couleur_de_l_etat, EtatDEcran, Pastille};
use engagement::importe::{ContratLu, PaieLue};
use socle::date::Jour;
use socle::fichiers::NomDeFichier;
use socle::mois::{Graphie, Mois};
use std::path::PathBuf;

// ------------------------------------------------------------------ les sorts

/// LE SORT D'UN FICHIER OU D'UNE PAGE DU DOSSIER (`import::bulletins`, la table de ses sorts).
///
/// | sort | ce qu'il veut dire |
/// |---|---|
/// | `repris` | ce que le fichier porte entre dans l'apercu |
/// | `deja_repris` | tout ce qu'il porte est deja dans l'historique (ou dans un autre fichier du dossier) |
/// | `libelle_inconnu` | repris, mais une ligne au moins a un libelle que la table ne connait pas : gardee telle quelle |
/// | `deja_saisi` | un engagement saisi paie ce salarie ce mois-la, et aucune paie declaree n'est a completer : non repris |
/// | `incoherent` | une DSN ou une AEM d'une autre entreprise (rien n'en est lu) ; la page contredit le reste (bulletin d'une autre entreprise, NIR qui contredit sa cle, plusieurs paies possibles, deux bulletins qui different) : non repris -- une page qui ne retombe pas sur ses propres totaux est reprise (le bulletin fait foi) |
/// | `incomplet` | ce qui identifie le bulletin ne se lit pas : non repris |
/// | `sans_texte` | aucun mot (lettres en traces, scan) : non repris |
/// | `sans_entetes` | le rang d'en-tetes de son tableau (« Libellé », « Base », les parts) ne se trouve pas : non repris |
/// | `ambigu` | la page se lit de deux facons (deux en-tetes, une colonne sans sa part, deux adresses de salarie) : non repris |
/// | `annexe` | l'annexe d'un bulletin, qui en detaille les cotisations : rien a reprendre |
/// | `non_reconnu` | ni DSN, ni AEM, ni PDF lisible, ni archive |
/// | `doublon` | la copie exacte d'un autre fichier du dossier |
/// | `archive` | un zip, ouvert : son contenu a son propre sort |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    Repris,
    DejaRepris,
    LibelleInconnu,
    DejaSaisi,
    Incoherent,
    Incomplet,
    SansTexte,
    SansEntetes,
    Ambigu,
    Annexe,
    NonReconnu,
    Doublon,
    Archive,
}

impl Sort {
    /// Les sorts d'une page, du plus grave au moins grave : un PDF de plusieurs pages prend le plus grave.
    pub const GRAVITE: [Sort; 10] = [
        Sort::Incoherent,
        Sort::Incomplet,
        Sort::Ambigu,
        Sort::SansEntetes,
        Sort::SansTexte,
        Sort::LibelleInconnu,
        Sort::Repris,
        Sort::DejaSaisi,
        Sort::DejaRepris,
        Sort::Annexe,
    ];

    /// Les sorts qui laissent quelque chose a voir (l'editeur, la table de lecture) : le fichier n'est pas repris.
    pub fn non_repris(self) -> bool {
        matches!(self, Sort::Incoherent | Sort::Incomplet | Sort::Ambigu | Sort::SansEntetes | Sort::SansTexte | Sort::NonReconnu)
    }

    /// L'etat d'un sort (sa couleur) et ses mots, tels qu'on les lit : sur la pastille et dans le resume d'un PDF. Repris,
    /// fait ; repris sous reserve (un libelle que la table ne connait pas), a voir ; non repris parce que quelque chose
    /// manque ou se contredit (`non_repris`), refuse ; laisse expres (deja saisi, annexe, doublon, archive ouverte), une
    /// information.
    pub fn en_mots(self) -> (EtatDEcran, &'static str) {
        match self {
            Sort::Repris => (EtatDEcran::Fait, "repris"),
            Sort::DejaRepris => (EtatDEcran::Fait, "déjà repris"),
            Sort::LibelleInconnu => (EtatDEcran::AFaire, "libellé inconnu"),
            Sort::DejaSaisi => (EtatDEcran::Information, "déjà saisi"),
            Sort::Incoherent => (EtatDEcran::Refuse, "incohérent"),
            Sort::Incomplet => (EtatDEcran::Refuse, "incomplet"),
            Sort::SansTexte => (EtatDEcran::Refuse, "sans texte"),
            Sort::SansEntetes => (EtatDEcran::Refuse, "tableau introuvable"),
            Sort::Ambigu => (EtatDEcran::Refuse, "ambigu"),
            Sort::Annexe => (EtatDEcran::Information, "annexe"),
            Sort::NonReconnu => (EtatDEcran::Refuse, "non reconnu"),
            Sort::Doublon => (EtatDEcran::Information, "doublon"),
            Sort::Archive => (EtatDEcran::Information, "archive"),
        }
    }
}

/// Une valeur par sort (un compte, une pastille), dans l'ordre des sorts.
#[derive(Debug, Clone, Serialize)]
pub struct ParSort<T> {
    pub repris: T,
    pub deja_repris: T,
    pub libelle_inconnu: T,
    pub deja_saisi: T,
    pub incoherent: T,
    pub incomplet: T,
    pub sans_texte: T,
    pub sans_entetes: T,
    pub ambigu: T,
    pub annexe: T,
    pub non_reconnu: T,
    pub doublon: T,
    pub archive: T,
}

impl<T> ParSort<T> {
    pub fn de(&mut self, s: Sort) -> &mut T {
        match s {
            Sort::Repris => &mut self.repris,
            Sort::DejaRepris => &mut self.deja_repris,
            Sort::LibelleInconnu => &mut self.libelle_inconnu,
            Sort::DejaSaisi => &mut self.deja_saisi,
            Sort::Incoherent => &mut self.incoherent,
            Sort::Incomplet => &mut self.incomplet,
            Sort::SansTexte => &mut self.sans_texte,
            Sort::SansEntetes => &mut self.sans_entetes,
            Sort::Ambigu => &mut self.ambigu,
            Sort::Annexe => &mut self.annexe,
            Sort::NonReconnu => &mut self.non_reconnu,
            Sort::Doublon => &mut self.doublon,
            Sort::Archive => &mut self.archive,
        }
    }
}

impl ParSort<usize> {
    /// Aucun fichier compte, de chaque sort.
    pub fn a_zero() -> Self {
        ParSort { repris: 0, deja_repris: 0, libelle_inconnu: 0, deja_saisi: 0, incoherent: 0, incomplet: 0, sans_texte: 0, sans_entetes: 0, ambigu: 0, annexe: 0, non_reconnu: 0, doublon: 0, archive: 0 }
    }
}

/// LA PASTILLE DE CHAQUE SORT, telle que la page la dessine (`Sort::en_mots`).
pub fn pastilles_des_sorts() -> ParSort<Pastille> {
    let p = |s: Sort| Pastille::de(s.en_mots().0, s.en_mots().1);
    ParSort {
        repris: p(Sort::Repris),
        deja_repris: p(Sort::DejaRepris),
        libelle_inconnu: p(Sort::LibelleInconnu),
        deja_saisi: p(Sort::DejaSaisi),
        incoherent: p(Sort::Incoherent),
        incomplet: p(Sort::Incomplet),
        sans_texte: p(Sort::SansTexte),
        sans_entetes: p(Sort::SansEntetes),
        ambigu: p(Sort::Ambigu),
        annexe: p(Sort::Annexe),
        non_reconnu: p(Sort::NonReconnu),
        doublon: p(Sort::Doublon),
        archive: p(Sort::Archive),
    }
}

// ------------------------------------------------------------------ d'ou vient une valeur

/// La piece d'ou vient une valeur reprise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Source {
    #[serde(rename = "dsn")]
    Dsn,
    #[serde(rename = "bulletin")]
    Bulletin,
    #[serde(rename = "certificat")]
    Certificat,
    #[serde(rename = "AEM")]
    Aem,
}

/// D'ou vient une valeur reprise : la DSN d'un mois, un bulletin (fichier, page), un certificat Congés Spectacles ou une
/// AEM (fichier, page, mois).
#[derive(Debug, Clone, Serialize)]
pub struct Origine {
    pub source: Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fichier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mois: Option<Mois>,
}

impl Origine {
    pub fn dsn(mois: Option<Mois>) -> Origine {
        Origine { source: Source::Dsn, fichier: None, page: None, mois }
    }

    /// Un bulletin : son fichier et sa page, quand on les sait.
    pub fn bulletin(fichier: Option<String>, page: Option<usize>) -> Origine {
        Origine { source: Source::Bulletin, fichier, page, mois: None }
    }

    /// Une piece : ce qu'elle est, son fichier, sa page, son mois.
    pub fn piece(source: Source, fichier: String, page: usize, mois: Option<Mois>) -> Origine {
        Origine { source, fichier: Some(fichier), page: Some(page), mois }
    }

    /// « la DSN de mars 2025 », « le bulletin (x.pdf, page 2) », « l'AEM de mars 2025 (x.pdf) ».
    fn en_mots(&self) -> String {
        let fichier: String = self
            .fichier
            .iter()
            .map(|f| match self.page {
                Some(p) => format!(" ({}, page {})", f, p),
                None => format!(" ({})", f),
            })
            .collect();
        let de_mois: String = self.mois.iter().map(|m| format!(" {}", socle::texte::de(&m.libelle(Graphie::Accentuee)))).collect();
        match self.source {
            Source::Aem => format!("l'AEM{}{}", de_mois, fichier),
            Source::Certificat => format!("le certificat Congés Spectacles{}{}", de_mois, fichier),
            Source::Bulletin => format!("le bulletin{}", fichier),
            Source::Dsn => format!("la DSN{}", de_mois),
        }
    }
}

/// UNE CASE, EN MOTS DANS UNE PHRASE : un texte ou un nombre tel qu'il s'ecrit, oui ou non, des mots a la suite, une
/// structure (`cle valeur`), ses lignes (chacune ses valeurs).
pub fn en_mots(v: &Valeur) -> String {
    let valeurs = |s: &Saisie| s.cases().iter().map(|(_, x)| en_mots(x)).collect::<Vec<_>>().join(" ");
    match v {
        Valeur::Texte(t) => t.clone(),
        Valeur::Nombre(n) => n.to_string(),
        Valeur::OuiNon(true) => "oui".to_string(),
        Valeur::OuiNon(false) => "non".to_string(),
        Valeur::Mots(l) => l.join(", "),
        Valeur::Bloc(b) => b.cases().iter().map(|(k, x)| format!("{} {}", k, en_mots(x))).collect::<Vec<_>>().join(", "),
        Valeur::Lignes(l) => l.iter().map(valeurs).collect::<Vec<_>>().join(", "),
    }
}

// ------------------------------------------------------------------ une fiche

/// Ou en est une fiche que l'import reprend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EtatFiche {
    ACreer,
    Existante,
    Creee,
    Completee,
}

/// Une case vide d'une fiche qu'une piece remplit : sa valeur et d'ou elle vient.
#[derive(Debug, Clone, Serialize)]
pub struct Completee {
    pub champ: CaseDeFiche,
    pub valeur: Valeur,
    #[serde(flatten)]
    pub origine: Origine,
    pub dit: String,
}

/// Ce qu'une piece dit d'une case autrement que la fiche, et ou.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum DitPar {
    Dsn { dsn: Valeur, mois: Mois },
    Bulletin { bulletin: String, fichier: String, page: usize },
    Certificat { certificat: String, fichier: String, page: usize },
    Aem { aem: String, fichier: String, page: usize },
}

/// UN ECART : une case que la fiche dit, qu'une piece disait autrement ; la fiche fait foi.
#[derive(Debug, Clone, Serialize)]
pub struct Ecart {
    pub champ: CaseDeFiche,
    /// La case de la fiche, telle qu'elle est (un texte, ou une structure : la banque).
    pub fiche: Valeur,
    #[serde(flatten)]
    pub par: DitPar,
    pub dit: String,
}

impl Ecart {
    pub fn new(champ: impl Into<CaseDeFiche>, fiche: Valeur, par: DitPar) -> Ecart {
        let (ou, lu) = match &par {
            DitPar::Certificat { certificat, fichier, page } => (format!("le certificat ({}, page {}) disait", fichier, page), certificat.clone()),
            DitPar::Aem { aem, fichier, page } => (format!("l'AEM ({}, page {}) disait", fichier, page), aem.clone()),
            DitPar::Bulletin { bulletin, fichier, page } => (format!("le bulletin ({}, page {}) disait", fichier, page), bulletin.clone()),
            DitPar::Dsn { dsn, mois } => (format!("la DSN {} disait", socle::texte::de(&mois.libelle(socle::mois::Graphie::Accentuee))), en_mots(dsn)),
        };
        let dit = format!("{} « {} », la fiche dit « {} » : la fiche fait foi", ou, lu, en_mots(&fiche));
        Ecart { champ: champ.into(), fiche, par, dit }
    }
}

/// LA FICHE QUE L'IMPORT VISE : une fiche nommee (qui existe, ou que l'import vient d'ecrire), ou la sorte d'une fiche
/// encore a creer, qui n'a pas de cle. Elle s'ecrit en deux champs, `nom` et `cle` (la cle de la fiche nommee, `null`
/// pour l'entreprise et pour une fiche a creer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FicheVisee {
    Nommee(FicheNommee),
    ACreer(NomDeFiche),
}

impl FicheVisee {
    /// Le nom de la fiche.
    pub fn nom(&self) -> NomDeFiche {
        match self {
            FicheVisee::Nommee(f) => f.nom(),
            FicheVisee::ACreer(n) => *n,
        }
    }

    /// La fiche nommee ; une fiche a creer n'en est pas une.
    pub fn nommee(&self) -> Option<&FicheNommee> {
        match self {
            FicheVisee::Nommee(f) => Some(f),
            FicheVisee::ACreer(_) => None,
        }
    }
}

/// L'ecriture d'une `FicheVisee` : son nom, et la cle d'une fiche nommee.
#[derive(Serialize)]
struct FicheViseeEcrite<'a> {
    nom: NomDeFiche,
    cle: Option<&'a str>,
}

impl Serialize for FicheVisee {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let cle = match self {
            FicheVisee::Nommee(FicheNommee::Salarie(k)) => Some(k.as_str()),
            FicheVisee::Nommee(FicheNommee::Spectacle(k)) => Some(k.as_str()),
            FicheVisee::Nommee(FicheNommee::Entreprise) | FicheVisee::ACreer(_) => None,
        };
        FicheViseeEcrite { nom: self.nom(), cle }.serialize(s)
    }
}

/// UNE FICHE QUE L'IMPORT CREE, COMPLETE OU LAISSE, ou sur laquelle il a a redire.
#[derive(Debug, Clone, Serialize)]
pub struct FicheImportee {
    #[serde(flatten)]
    pub fiche: FicheVisee,
    pub libelle: String,
    pub etat: EtatFiche,
    pub ecarts: Vec<Ecart>,
    pub anomalies: Vec<Anomalie>,
    /// Ce que porte une fiche a creer.
    pub donnees: Saisie,
    pub completer: Vec<Completee>,
    /// La piece d'ou vient une fiche a creer, quand ce n'est ni une DSN ni un bulletin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lu_dans: Option<Origine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lu_dans_dit: Option<String>,
    /// Creee, completee ou deja la, faite ; a creer ou a completer, a faire.
    pub pastille: Pastille,
}

impl FicheImportee {
    pub fn new(fiche: FicheVisee, libelle: &str, etat: EtatFiche, ecarts: Vec<Ecart>, anomalies: Vec<Anomalie>, donnees: &Saisie) -> FicheImportee {
        let mut f = FicheImportee {
            fiche,
            libelle: libelle.to_string(),
            etat,
            ecarts: Vec::new(),
            anomalies: Vec::new(),
            donnees: Saisie::default(),
            completer: Vec::new(),
            lu_dans: None,
            lu_dans_dit: None,
            pastille: Pastille::de(EtatDEcran::AFaire, "à créer"),
        };
        f.ajouter_ecarts(ecarts);
        f.poser(donnees, anomalies);
        f
    }

    /// Ce que porte la fiche a creer (ses cases, sans ses notes), et ses anomalies.
    pub fn poser(&mut self, donnees: &Saisie, anomalies: Vec<Anomalie>) {
        let mut cases = Saisie::default();
        crate::proposee::fondre(&mut cases, donnees);
        self.donnees = cases;
        self.anomalies = anomalies;
    }

    /// Les ecarts d'une piece de plus : une case deja notee garde le sien (la DSN passe avant le bulletin).
    pub fn ajouter_ecarts(&mut self, ecarts: Vec<Ecart>) {
        for e in ecarts {
            if !self.ecarts.iter().any(|x| x.champ == e.champ) {
                self.ecarts.push(e);
            }
        }
    }

    /// Note ce qui serait complete et d'ou ca vient. Une case deja notee garde sa premiere origine
    /// (`completer::ajouter`).
    pub fn noter(&mut self, champs: &Saisie, origine: &Origine) {
        for (k, v) in champs.cases() {
            if !self.completer.iter().any(|c| c.champ == *k) {
                self.completer.push(Completee { champ: *k, valeur: v.clone(), origine: origine.clone(), dit: String::new() });
            }
        }
    }

    /// Le numero d'objet d'un spectacle a creer. L'import l'y pose d'un `NumeroObjetSpectacle` (une AEM, un bulletin) : un
    /// texte qui ne se lit pas en numero d'objet est une faute du logiciel, qui se dit.
    pub fn numero_objet(&self) -> socle::echec::R<Option<droit::fiche::NumeroObjetSpectacle>> {
        self.donnees.mot(droit::fiche::CaseSpectacle::NumeroObjet).map(|n| droit::fiche::NumeroObjetSpectacle::lire(&n)).transpose()
    }

    /// La fiche s'ecrit-elle : a creer, ou existante avec des cases a completer.
    fn a_ecrire(&self) -> bool {
        self.etat == EtatFiche::ACreer || (self.etat == EtatFiche::Existante && !self.completer.is_empty())
    }

    /// Ou elle en est (`pastille`), d'ou elle vient (`lu_dans_dit`), chaque case qu'une piece remplit (`completer.dit`).
    fn dire(&mut self) {
        self.pastille = match self.etat {
            EtatFiche::Completee => Pastille::de(EtatDEcran::Fait, "complétée"),
            EtatFiche::Existante if !self.completer.is_empty() => Pastille::de(EtatDEcran::AFaire, "à compléter"),
            EtatFiche::Existante => Pastille::de(EtatDEcran::Fait, "existe déjà"),
            EtatFiche::Creee => Pastille::de(EtatDEcran::Fait, "créée"),
            EtatFiche::ACreer => Pastille::de(EtatDEcran::AFaire, "à créer"),
        };
        self.lu_dans_dit = self.lu_dans.as_ref().map(|o| format!("Lu dans {}", o.en_mots()));
        let completee = self.etat == EtatFiche::Completee;
        for c in &mut self.completer {
            c.dit = format!("{} « {} », lu dans {} : la case était vide", if completee { "a reçu" } else { "recevra" }, en_mots(&c.valeur), c.origine.en_mots());
        }
    }
}

// ------------------------------------------------------------------ le dossier, fichier par fichier

/// Ou en est la paie qu'un bulletin complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EtatPaie {
    Deja,
    Nouvelle,
    Completee,
}

/// Ou en est le contrat qu'une piece atteste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EtatContrat {
    Inconnu,
    Deja,
    Ecart,
    Nouveau,
}

/// UN ECART ENTRE UN BULLETIN ET LA PAIE QU'IL COMPLETE : le bulletin l'emporte (a voir), ou l'ecart est note.
#[derive(Debug, Clone, Serialize)]
pub struct EcartPaie {
    pub champ: &'static str,
    /// D'ou vient la paie : `dsn`, `historique` ou `bulletin`.
    pub source: &'static str,
    pub valeur: String,
    pub bulletin: String,
    pub le_bulletin_l_emporte: bool,
    pub champ_dit: &'static str,
    pub pastille: Pastille,
}

impl EcartPaie {
    pub fn new(champ: &'static str, source: &'static str, valeur: String, gagne: bool, bulletin: String) -> EcartPaie {
        let champ_dit = match champ {
            "brut" => "brut",
            "net_imposable" => "net imposable",
            "net_a_payer" => "net à payer",
            "pas" => "prélèvement à la source",
            autre => autre,
        };
        let pastille = if gagne { Pastille::de(EtatDEcran::AFaire, "le bulletin l'emporte") } else { Pastille::de(EtatDEcran::Information, "noté") };
        EcartPaie { champ, source, valeur, bulletin, le_bulletin_l_emporte: gagne, champ_dit, pastille }
    }
}

/// Le salarie d'une page, ecrit comme une ligne lue l'ecrit ; une page qui n'en dit pas ne l'ecrit pas (`skip`).
fn ecrit_salarie_de_la_page<S: serde::Serializer>(salarie: &Option<Option<droit::contexte::IdSalarie>>, s: S) -> Result<S::Ok, S::Error> {
    match salarie {
        Some(cle) => engagement::importe::ecrit_salarie(cle, s),
        None => s.serialize_none(),
    }
}

/// Ce qu'une page dit d'elle-meme dans le compte rendu.
#[derive(Debug, Clone, Default, Serialize)]
pub struct InfosPage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logiciel: Option<engagement::importe::LogicielDUnBulletin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numero: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aem: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificat: Option<String>,
    /// Le salarie que la page paie, quand elle se rattache a un NIR : la cle de sa fiche, `None` tant qu'elle est a
    /// creer (`engagement::importe::ecrit_salarie`).
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "ecrit_salarie_de_la_page")]
    pub salarie: Option<Option<droit::contexte::IdSalarie>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debut: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fin: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mois: Option<Mois>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paie: Option<EtatPaie>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrat: Option<EtatContrat>,
    pub ecarts: Vec<EcartPaie>,
    /// Ce que le compte des heures, des cachets et des jours a releve.
    pub compteurs: Vec<String>,
    /// Ce que la page dit d'elle-meme qui ne se retrouve pas : le bulletin fait foi.
    pub avertissements: Vec<String>,
    /// Le contrat du bulletin, quand un autre contrat atteste aux memes dates est garde.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrat_bulletin: Option<ContratLu>,
}

/// Le compte rendu d'une page de PDF.
#[derive(Debug, Clone, Serialize)]
pub struct PageVu {
    pub page: usize,
    pub sort: Sort,
    pub raison: String,
    #[serde(flatten)]
    pub infos: InfosPage,
    /// « page 2 (mars 2025) ».
    pub page_dit: String,
    /// Ou se lit un ecart de la page : le fichier, et sa page s'il en a plusieurs.
    pub ou_dit: String,
}

impl PageVu {
    pub fn new(page: usize, sort: Sort, raison: String, infos: InfosPage) -> PageVu {
        PageVu { page, sort, raison, infos, page_dit: String::new(), ou_dit: String::new() }
    }
}

/// Le compte rendu d'un fichier du dossier.
#[derive(Debug, Clone, Serialize)]
pub struct FichierVu {
    pub chemin: String,
    /// La sorte du fichier, au tri (`tri::Sorte::nom`).
    pub sorte: &'static str,
    pub pages: Vec<PageVu>,
    pub sort: Sort,
    pub raison: String,
    /// Le nom de sa copie sous `historique/bulletins/`, s'il est repris.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copie: Option<NomDeFichier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doublon_de: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mois: Option<Mois>,
    /// D'ou il vient : une copie, une archive.
    pub de_dit: Option<String>,
    /// « 3 pages ».
    pub pages_dit: String,
}

impl FichierVu {
    /// Ses phrases : d'ou il vient, ses pages, ou se lit l'ecart d'une page.
    pub fn dire(&mut self) {
        self.de_dit = match (&self.doublon_de, &self.archive) {
            (Some(d), _) => Some(format!("(copie de {})", d)),
            (None, Some(a)) => Some(format!("(dans {})", a)),
            _ => None,
        };
        let nb = self.pages.len();
        self.pages_dit = socle::ecran::compte(nb, "page", "pages");
        for p in &mut self.pages {
            p.page_dit = match p.infos.mois {
                Some(m) => format!("page {} ({})", p.page, m.libelle(Graphie::Accentuee)),
                None => format!("page {}", p.page),
            };
            p.ou_dit = if nb > 1 { format!("{}, page {}", self.chemin, p.page) } else { self.chemin.clone() };
        }
    }
}

/// Un fichier qui n'est pas repris, pour l'encart.
#[derive(Debug, Clone, Serialize)]
pub struct NonReprisVu {
    pub chemin: String,
    pub sort: Sort,
    pub raison: String,
}

/// CE QUI N'EST PAS REPRIS : chaque fichier et sa raison ; un import qui en laisse se rejoue (`phrase`).
#[derive(Debug, Clone, Serialize)]
pub struct NonRepris {
    pub fichiers: Vec<NonReprisVu>,
    pub phrase: &'static str,
}

/// Le dossier lu, fichier par fichier.
#[derive(Debug, Clone, Serialize)]
pub struct TriVu {
    /// Le dossier lu ; vide tant qu'aucun ne l'est (`aucun`).
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub dossier: PathBuf,
    pub fichiers: Vec<FichierVu>,
    /// Les fichiers, par sort.
    pub comptes: ParSort<usize>,
    pub total: usize,
    pub non_repris: Option<NonRepris>,
    /// « 12 fichiers dans … ».
    pub dit: String,
}

impl TriVu {
    /// Aucun dossier lu encore.
    pub fn aucun() -> TriVu {
        TriVu { dossier: PathBuf::new(), fichiers: Vec::new(), comptes: ParSort::a_zero(), total: 0, non_repris: None, dit: String::new() }
    }
}

// ------------------------------------------------------------------ les pieces lues

/// Les bulletins lus : combien, de quand a quand, ce qu'ils ajoutent.
#[derive(Debug, Clone, Serialize)]
pub struct BlocBulletins {
    pub pages: usize,
    pub lus: usize,
    pub repris: usize,
    pub par_logiciel: std::collections::BTreeMap<String, usize>,
    pub du: Option<Jour>,
    pub au: Option<Jour>,
    pub fiches_a_creer: usize,
    pub contrats_nouveaux: usize,
    pub contrats: Vec<ContratLu>,
    pub paies_nouvelles: usize,
    pub paies_completees: usize,
    pub paies: Vec<PaieLue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dit: Option<String>,
}

impl BlocBulletins {
    /// Aucun bulletin lu encore.
    pub fn aucun() -> BlocBulletins {
        BlocBulletins {
            pages: 0,
            lus: 0,
            repris: 0,
            par_logiciel: std::collections::BTreeMap::new(),
            du: None,
            au: None,
            fiches_a_creer: 0,
            contrats_nouveaux: 0,
            contrats: Vec::new(),
            paies_nouvelles: 0,
            paies_completees: 0,
            paies: Vec::new(),
            dit: None,
        }
    }
}

/// Les certificats Congés Spectacles, ou les AEM en PDF, lus : combien, de quand a quand, ce qu'ils ajoutent, et ce qui
/// reste a saisir dans une fiche qu'ils creent.
#[derive(Debug, Clone, Serialize)]
pub struct BlocPieces {
    pub lus: usize,
    pub du: Option<Jour>,
    pub au: Option<Jour>,
    pub contrats_nouveaux: usize,
    pub contrats: Vec<ContratLu>,
    pub confirmes: usize,
    pub fiches_a_creer: usize,
    pub reste_a_saisir: Vec<&'static str>,
    pub dit: String,
}

/// La copie de ce que l'import reprend, dans `historique/bulletins/`.
#[derive(Debug, Clone, Serialize)]
pub struct Copie {
    pub copies: usize,
    pub identiques: usize,
    /// Les noms deja copies avec un autre contenu : la copie est gardee telle quelle.
    pub differents: Vec<String>,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub dossier: PathBuf,
}

// ------------------------------------------------------------------ la reponse

/// CE QUE L'APERCU OU LA REPRISE D'UN DOSSIER REND (`importation-apercu`, `importer`).
#[derive(Debug, Clone, Serialize)]
pub struct Importation {
    pub phrase: String,
    pub source: String,
    /// Les DSN lues, les fichiers d'AEM lus.
    pub dsn: usize,
    pub aem: usize,
    pub premier_mois: Option<Mois>,
    pub dernier_mois: Option<Mois>,
    pub entreprise: Option<FicheImportee>,
    pub salaries: Vec<FicheImportee>,
    pub spectacles: Vec<FicheImportee>,
    /// Les contrats attestes par les AEM deposees.
    pub contrats: usize,
    pub contrats_nouveaux: usize,
    pub contrats_du: Option<Jour>,
    pub contrats_au: Option<Jour>,
    pub contrats_sans_fiche: usize,
    pub avertissements: Vec<String>,
    pub rien_a_faire: bool,
    pub ecrit: bool,
    /// Les fichiers que l'import a ecrits.
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub fichiers: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copie: Option<Copie>,
    pub tri: TriVu,
    pub bulletins: BlocBulletins,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificats: Option<BlocPieces>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aem_pdf: Option<BlocPieces>,
    /// Les depots DSN que les retours deja releves attestent, entres au journal (`importer`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depots_repris: Option<usize>,
    /// Ce que les DSN reprises ont pose dans la configuration (`importer`).
    pub parametrage: Vec<String>,
    pub declarations_dit: String,
    pub titre_dit: &'static str,
    /// Ce que le bouton reprendrait.
    pub reprendre: String,
    pub pastilles: ParSort<Pastille>,
    /// L'encart de ce qui est repris, ou qu'il n'y a rien a reprendre : fait.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encart: Option<Pastille>,
    /// La couleur de la phrase (`importer`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub couleur: Option<&'static str>,
}

/// « 3 bulletins » : `n` et le mot, au pluriel quand il le faut ; rien pour zero.
pub fn n_mots(n: usize, mot: &str) -> String {
    match n {
        0 => String::new(),
        1 => format!("1 {}", mot),
        n => format!("{} {}s", n, mot),
    }
}

impl Importation {
    /// Les fiches de la reponse : l'entreprise, les salaries, les spectacles.
    pub fn fiches_mut(&mut self) -> impl Iterator<Item = &mut FicheImportee> {
        self.entreprise.iter_mut().chain(self.salaries.iter_mut()).chain(self.spectacles.iter_mut())
    }

    /// LES PHRASES DE CE QUI EST REPRIS : les declarations, les bulletins, les certificats et les AEM en PDF lus, le
    /// dossier, ce que le bouton reprendrait, et pour chaque fiche ce qu'elle recoit d'une piece ; la pastille de chaque
    /// sort et l'encart. Une fois tout ecrit.
    pub fn dire(&mut self) {
        let ecrit = self.ecrit;
        let s = |k: usize| if k > 1 { "s" } else { "" };
        let creer = |k: usize| if ecrit { if k > 1 { " créées" } else { " créée" } } else { " à créer" };
        // Les declarations.
        // Une periode ne se dit qu'avec ses deux bornes.
        let periode: String = self.premier_mois.zip(self.dernier_mois).map(|(p, d)| format!(" ({} → {})", p.libelle(Graphie::Accentuee), d.libelle(Graphie::Accentuee))).into_iter().collect();
        let contrats_du_au: String = self.contrats_du.zip(self.contrats_au).map(|(du, au)| format!(" du {} au {}", du.jj_mm_aaaa(), au.jj_mm_aaaa())).into_iter().collect();
        let contrats = if self.contrats > 0 {
            format!(
                "{} contrat(s) attesté(s){} : {} {} à l'historique, {} déjà connu(s){}",
                self.contrats,
                contrats_du_au,
                self.contrats_nouveaux,
                if ecrit { "ajouté(s)" } else { "à ajouter" },
                self.contrats.saturating_sub(self.contrats_nouveaux + self.contrats_sans_fiche),
                if self.contrats_sans_fiche > 0 { format!(", {} sans fiche", self.contrats_sans_fiche) } else { String::new() }
            )
        } else {
            "aucun contrat attesté par une AEM".to_string()
        };
        self.declarations_dit = if self.dsn + self.aem > 0 {
            format!("{} DSN{}, {} fichier(s) AEM : {}.", self.dsn, periode, self.aem, contrats)
        } else {
            "Aucune DSN ni fichier d'AEM déposé dans le dossier.".to_string()
        };
        // Les bulletins.
        let b = &mut self.bulletins;
        b.dit = (b.pages > 0).then(|| {
            let ajouts: Vec<String> = [
                if b.paies_nouvelles > 0 { format!("{} paie{} qu'aucune DSN ne déclare", b.paies_nouvelles, s(b.paies_nouvelles)) } else { String::new() },
                if b.paies_completees > 0 {
                    let k = b.paies_completees;
                    format!("{} paie{} déclarée{} complétée{} (cotisations, coût employeur, lignes)", k, s(k), s(k), s(k))
                } else {
                    String::new()
                },
                if b.contrats_nouveaux > 0 { format!("{} contrat{} attesté{}", b.contrats_nouveaux, s(b.contrats_nouveaux), s(b.contrats_nouveaux)) } else { String::new() },
                if b.fiches_a_creer > 0 { format!("{} fiche{}{}", b.fiches_a_creer, s(b.fiches_a_creer), creer(b.fiches_a_creer)) } else { String::new() },
            ]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect();
            let du: String = b.du.zip(b.au).map(|(d, a)| format!(" du {} au {}", d.jj_mm_aaaa(), a.jj_mm_aaaa())).into_iter().collect();
            format!(
                "{} bulletin{} lu{} sur {} page{}{} ; {} {}{}.",
                b.lus,
                s(b.lus),
                s(b.lus),
                b.pages,
                s(b.pages),
                du,
                b.repris,
                if ecrit { "repris" } else { "à reprendre" },
                if ajouts.is_empty() { String::new() } else { format!(" : {}", ajouts.join(", ")) }
            )
        });
        // Les certificats Congés Spectacles et les AEM en PDF.
        for (c, piece, pieces, aucune) in [
            (&mut self.certificats, "certificat", "certificats", "Aucun certificat ne porte"),
            (&mut self.aem_pdf, "attestation", "attestations", "Aucune AEM ne porte"),
        ] {
            let Some(c) = c.as_mut() else { continue };
            let ajouts: Vec<String> = [
                if c.contrats_nouveaux > 0 { format!("{} contrat{} attesté{}", c.contrats_nouveaux, s(c.contrats_nouveaux), s(c.contrats_nouveaux)) } else { String::new() },
                if c.confirmes > 0 { format!("{} déjà attesté{} par une autre pièce", c.confirmes, s(c.confirmes)) } else { String::new() },
                if c.fiches_a_creer > 0 { format!("{} fiche{}{}", c.fiches_a_creer, s(c.fiches_a_creer), creer(c.fiches_a_creer)) } else { String::new() },
            ]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect();
            let du: String = c.du.zip(c.au).map(|(d, a)| format!(" du {} au {}", d.jj_mm_aaaa(), a.jj_mm_aaaa())).into_iter().collect();
            c.dit = format!(
                "{} {} lu{}{}{}.{}",
                c.lus,
                if c.lus > 1 { pieces } else { piece },
                s(c.lus),
                du,
                if ajouts.is_empty() { String::new() } else { format!(" : {}", ajouts.join(", ")) },
                if c.reste_a_saisir.is_empty() { String::new() } else { format!(" {}, et reste à saisir dans chaque fiche : {}.", aucune, c.reste_a_saisir.join(" ; ")) }
            );
        }
        // Le dossier, fichier par fichier.
        self.tri.dit = format!("{} fichier{} dans {}", self.tri.total, s(self.tri.total), self.tri.dossier.display());
        self.tri.fichiers.iter_mut().for_each(FichierVu::dire);
        self.titre_dit = if ecrit { "Repris" } else { "Serait repris" };
        // Ce que le bouton reprendrait.
        let nc = self.contrats_nouveaux + self.bulletins.contrats_nouveaux;
        let np = self.bulletins.paies_nouvelles + self.bulletins.paies_completees;
        let compte = |l: &[FicheImportee]| l.iter().filter(|f| f.a_ecrire()).count();
        let quoi: Vec<String> = [
            if self.entreprise.as_ref().is_some_and(FicheImportee::a_ecrire) { "la fiche de la compagnie".to_string() } else { String::new() },
            n_mots(compte(&self.salaries), "salarié"),
            n_mots(compte(&self.spectacles), "spectacle"),
            n_mots(nc, "contrat"),
            n_mots(np, "bulletin"),
        ]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect();
        self.reprendre = quoi.join(", ");
        self.fiches_mut().for_each(FicheImportee::dire);
        self.encart = (self.ecrit || self.rien_a_faire).then(|| Pastille::de(EtatDEcran::Fait, "repris"));
    }

    /// La phrase de l'apercu : ce que le dossier lu donne ; rien n'est ecrit.
    pub fn dire_l_apercu(&mut self) {
        self.dire();
        let b = &self.bulletins;
        let periode: String = b.du.zip(b.au).map(|(du, au)| format!(" (du {} au {})", du.jj_mm_aaaa(), au.jj_mm_aaaa())).into_iter().collect();
        self.phrase = format!(
            "Dans le dossier lu : {} bulletins lus{}, {} à reprendre, fiches à créer : {} ; DSN : {} ; AEM : {}. Rien n'est écrit.",
            b.lus,
            periode,
            b.repris,
            b.fiches_a_creer,
            self.dsn,
            self.aem
        );
    }

    /// La couleur de la phrase d'un import fait.
    pub fn fait(&mut self) {
        self.couleur = Some(couleur_de_l_etat(EtatDEcran::Fait));
    }
}
