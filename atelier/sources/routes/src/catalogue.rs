//! LA FICHE D'UNE ROUTE ET LE CATALOGUE (`moteur routes`). Chaque route porte sa fiche dans la liste `ROUTES` de son
//! module : le geste qu'elle sert, sa charge et un exemple, ce qu'elle rend, ses refus et quoi corriger, si elle lit,
//! ecrit ou engage, si elle est « avancee », et la place des phrases qu'elle rend. La page et le LLM passent par les memes
//! routes ; le catalogue est leur documentation, tiree du code : celles d'une compagnie et celles de l'installation
//! (`accueil`). Les routes de l'atelier (l'horloge du banc, la route d'essai qui tombe) n'y sont pas.

use engagement::carte::{CarteValide, Exige, NatureDecrite};
use socle::echec::{Classe, R};

use super::enveloppe::{en_json, Charge, Dite, Rendu};

pub use crate::affiche::Affiche;

/// LES ROUTES, chacune une fois, par son nom : celui de `/api/<route>` et de `moteur route`. Une variante par fiche du
/// catalogue (`au_catalogue`), et celles de l'atelier, qui n'en ont pas : l'horloge du banc, la route d'essai qui tombe.
/// Le nom d'une route se lit une fois, a l'entree (`Route::lire`) ; le moteur ne compare plus de noms en texte, et une
/// route mal orthographiee dans le code ne compile pas.
macro_rules! routes {
    (catalogue { $($v:ident = $n:literal,)* } atelier { $($a:ident = $an:literal,)* }) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
        pub enum Route {
            $($v,)*
            $($a,)*
        }

        impl Route {
            /// Toutes les routes, celles du catalogue d'abord.
            pub const TOUTES: &'static [Route] = &[$(Route::$v,)* $(Route::$a,)*];

            /// Le nom de la route, tel que la page et la ligne de commande l'ecrivent.
            pub const fn nom(self) -> &'static str {
                match self {
                    $(Route::$v => $n,)*
                    $(Route::$a => $an,)*
                }
            }

            /// La route a-t-elle sa fiche au catalogue ? Celles de l'atelier n'en ont pas.
            pub const fn au_catalogue(self) -> bool {
                match self {
                    $(Route::$v => true,)*
                    $(Route::$a => false,)*
                }
            }
        }
    };
}

routes! {
    catalogue {
        ValiderEngagement = "valider-engagement",
        ModifierEngagement = "modifier-engagement",
        RetirerEngagement = "retirer-engagement",
        Mois = "mois",
        Demarrage = "demarrage",
        EtatParEngagement = "etat-par-engagement",
        EtatDuMois = "etat-du-mois",
        Saisie = "saisie",
        Apercu = "apercu",
        ContratsEnCours = "contrats-en-cours",
        ContratsPasses = "contrats-passes",
        Emploi = "emploi",
        EnregistrerSaisie = "enregistrer-saisie",
        Ajuster = "ajuster",
        MoisAOuvrir = "mois-a-ouvrir",
        Imprevus = "imprevus",
        Arrets = "arrets",
        FinsDeContrat = "fins-de-contrat",
        ArretContrats = "arret-contrats",
        ArretEnregistrer = "arret-enregistrer",
        ArretProlonger = "arret-prolonger",
        ArretReprise = "arret-reprise",
        ArretIndemnites = "arret-indemnites",
        ArretSignaler = "arret-signaler",
        ArretOublier = "arret-oublier",
        SuspensionEnregistrer = "suspension-enregistrer",
        SuspensionModifier = "suspension-modifier",
        SuspensionOublier = "suspension-oublier",
        FinContratSignaler = "fin-contrat-signaler",
        FinContratDocuments = "fin-contrat-documents",
        DossierSalarie = "dossier-salarie",
        SignatureApercu = "signature-apercu",
        DemanderSignature = "demander-signature",
        PiecesParties = "pieces-parties",
        AvenantApercu = "avenant-apercu",
        ContratsImprimer = "contrats-imprimer",
        ContratRecu = "contrat-recu",
        PoserCourriel = "poser-courriel",
        ModelesContrat = "modeles-contrat",
        ModeleContrat = "modele-contrat",
        CopierModeleContrat = "copier-modele-contrat",
        RenommerModeleContrat = "renommer-modele-contrat",
        EcrireParagrapheContrat = "ecrire-paragraphe-contrat",
        AjouterParagrapheContrat = "ajouter-paragraphe-contrat",
        DeplacerParagrapheContrat = "deplacer-paragraphe-contrat",
        SupprimerParagrapheContrat = "supprimer-paragraphe-contrat",
        ApercuModeleContrat = "apercu-modele-contrat",
        SupprimerModeleContrat = "supprimer-modele-contrat",
        ModeleContratParDefaut = "modele-contrat-par-defaut",
        DepotApercu = "depot-apercu",
        Deposer = "deposer",
        SuivreDepot = "suivre-depot",
        BilanRepetition = "bilan-repetition",
        EnvoiApercu = "envoi-apercu",
        EnvoyerDocuments = "envoyer-documents",
        DocumentsImprimer = "documents-imprimer",
        PiecePartie = "piece-partie",
        CoffreEtat = "coffre-etat",
        GoogleEtat = "google-etat",
        CoffrePoser = "coffre-poser",
        CoffreVerifier = "coffre-verifier",
        CoffreOublier = "coffre-oublier",
        MotDePasseChanger = "mot-de-passe-changer",
        MotDePasseRetirer = "mot-de-passe-retirer",
        GoogleAutoriser = "google-autoriser",
        GoogleOublier = "google-oublier",
        AdministrateurEtat = "administrateur-etat",
        AdministrateurRattacher = "administrateur-rattacher",
        AdministrateurDetacher = "administrateur-detacher",
        ImportationEtat = "importation-etat",
        ImportationApercu = "importation-apercu",
        Importer = "importer",
        ImportationNature = "importation-nature",
        DossierFormes = "dossier-formes",
        ExportsPeriodes = "exports-periodes",
        RetenirLogiciel = "retenir-logiciel",
        LibellesCourts = "libelles-courts",
        LibellesCourtsEnregistrer = "libelles-courts-enregistrer",
        CascRegularise = "casc-regularise",
        Exporter = "exporter",
        DossierEnvoyer = "dossier-envoyer",
        Droits = "droits",
        CascEnvoyer = "casc-envoyer",
        Exercices = "exercices",
        Regularisations = "regularisations",
        Cloture = "cloture",
        ExportCompta = "export-compta",
        Archiver = "archiver",
        JournalReprise = "journal-reprise",
        Permanents = "permanents",
        TaxeSurLesSalairesRegime = "taxe-sur-les-salaires-regime",
        VerifierFiche = "verifier-fiche",
        EnregistrerFiche = "enregistrer-fiche",
        ActiverFiche = "activer-fiche",
        FicheDemander = "fiche-demander",
        FichesRelever = "fiches-relever",
        FicheOublierDemande = "fiche-oublier-demande",
        TauxAtmp = "taux-atmp",
        Effectif = "effectif",
        Couts = "couts",
        SauvegardeMaintenant = "sauvegarde-maintenant",
        CompagnieSupprimer = "compagnie-supprimer",
        Identite = "identite",
        PoserIdentite = "poser-identite",
        RetirerIdentite = "retirer-identite",
        MetiersCatalogue = "metiers-catalogue",
        MetierRetenu = "metier-retenu",
        BulletinModeles = "bulletin-modeles",
        BulletinModeleApercu = "bulletin-modele-apercu",
        BulletinModeleChoisir = "bulletin-modele-choisir",
        OrganismesContrats = "organismes-contrats",
        OrganismesEnregistrer = "organismes-enregistrer",
        SignalementApercu = "signalement-apercu",
        SignalementEnvoyer = "signalement-envoyer",
        Signalements = "signalements",
        SignalementsLus = "signalements-lus",
        SignalementsPartir = "signalements-partir",
        IncidentNavigateur = "incident-navigateur",
        DiagnosticEtat = "diagnostic-etat",
        DiagnosticEnvoyer = "diagnostic-envoyer",
        DiagnosticRegler = "diagnostic-regler",
        IncidentsLire = "incidents-lire",
        IncidentsRegrouper = "incidents-regrouper",
        IncidentsEnvoyer = "incidents-envoyer",
        IncidentsOublier = "incidents-oublier",
        HistoriqueMois = "historique-mois",
        Etat = "etat",
        Configuration = "configuration",
        MajEtat = "maj-etat",
        CourrielsFictifs = "courriels-fictifs",
        Rappels = "rappels",
        Echeancier = "echeancier",
        RevueSpectacles = "revue-spectacles",
        PourDemarrer = "pour-demarrer",
        EcheanceFaite = "echeance-faite",
        ConseilDuJour = "conseil-du-jour",
        ConseilsSalarie = "conseils-salarie",
        ConseilsResume = "conseils-resume",
        Ouverture = "ouverture",
        ArretObligations = "arret-obligations",
        ArretConnaissance = "arret-connaissance",
        AccidentDeclare = "accident-declare",
        AttestationDeclare = "attestation-declare",
        Compagnies = "compagnies",
        CompagnieAjouter = "compagnie-ajouter",
        CompagnieCouleur = "compagnie-couleur",
        CompagnieRemettreAZero = "compagnie-remettre-a-zero",
        CompagniesGarder = "compagnies-garder",
        AdministrateurTableau = "administrateur-tableau",
        AdministrateurDeposer = "administrateur-deposer",
        AdministrateurSuivre = "administrateur-suivre",
        PortableCompleter = "portable-completer",
        Routes = "routes",
    }
    atelier {
        HorlogeRattraper = "horloge-rattraper",
        EssaiDefautDuLogiciel = "essai-defaut-du-logiciel",
    }
}

/// Le refus d'un nom qui n'est celui d'aucune route ; le nom va a part.
pub(crate) const ROUTE_INCONNUE: &str = "Cette route n'existe pas : la liste des routes le dit.";

impl Route {
    /// La route nommee `nom`, s'il en est une.
    pub fn lire(nom: &str) -> Option<Route> {
        Route::TOUTES.iter().copied().find(|r| r.nom() == nom)
    }
}

impl std::fmt::Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.nom())
    }
}

impl serde::Serialize for Route {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nom())
    }
}

impl<'de> serde::Deserialize<'de> for Route {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Route, D::Error> {
        let nom = String::deserialize(d)?;
        Route::lire(&nom).ok_or_else(|| serde::de::Error::custom(ROUTE_INCONNUE))
    }
}

/// Ce que fait la route.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Acte {
    /// Elle lit : rien ne s'ecrit.
    Lit,
    /// Elle ecrit dans la compagnie ; rien ne part.
    Ecrit,
    /// Un acte reel (un envoi, un depot, une signature) : le mot de passe du logiciel, tape par un humain.
    Engage,
}

impl Acte {
    pub fn mot(self) -> &'static str {
        match self {
            Acte::Lit => "lit",
            Acte::Ecrit => "écrit",
            Acte::Engage => "engage (le mot de passe du logiciel, tapé par un humain)",
        }
    }
}

/// LES PLACES DE L'ECRAN, telles que la page les nomme : l'onglet, et le sous-onglet de la Configuration. Une place se
/// nomme d'une seule facon, ici ; une fiche ne peut nommer qu'une de ces places.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    Paie,
    Fiches,
    Metiers,
    Organismes,
    Bulletin,
    ModelesContrat,
    Identite,
    Coffre,
    Import,
    Historique,
    Exports,
    Signaler,
    Bandeau,
    Accueil,
}

impl Place {
    /// Le nom que l'ecran montre (la page le porte en `data-place`).
    pub fn nom(self) -> &'static str {
        match self {
            Place::Paie => "Paie",
            Place::Fiches => "Configuration › Entreprise, Salariés, Spectacles",
            Place::Metiers => "Configuration › Métiers",
            Place::Organismes => "Configuration › Organismes et contrats",
            Place::Bulletin => "Configuration › Modèle de bulletin",
            Place::ModelesContrat => "Configuration › Modèles de contrat",
            Place::Identite => "Configuration › Signature & tampon",
            Place::Coffre => "Configuration › Mots de passe",
            Place::Import => "Configuration › Reprendre l'historique",
            Place::Historique => "Historique",
            Place::Exports => "Exports",
            Place::Signaler => "Un problème, une idée ?",
            Place::Bandeau => "Le bandeau",
            Place::Accueil => "L'accueil des compagnies",
        }
    }
}

/// OU LA ROUTE SE VOIT A L'ECRAN, et si elle est « avancee » : hors du chemin simple (le catalogue sans `avance` ne la
/// dit pas ; la porte qui l'ouvre va dans le groupe « Avancé » de sa barre, `js/tuyau.js`). L'ecran seul le dit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ecran {
    /// Le chemin simple : la page de cette place l'appelle (son ecran est dessine).
    Dessine(Place),
    /// Avancee : la page de cette place l'appelle (son ecran est dessine), hors du chemin simple.
    DessineAvance(Place),
    /// Le chemin simple : une autre route de cette place la rend en geste (sa route, sa charge, son libelle, les cases
    /// a saisir) ; la page le dessine et fait saisir ces cases depuis sa fiche (`js/avance.js`), sans la nommer.
    Geste(Place),
    /// Avancee : rendue en geste, comme `Geste`, hors du chemin simple.
    GesteAvance(Place),
    /// Avancee, sans ecran dessine : sous « Avancé » de sa place, la page l'affiche depuis sa fiche (`js/avance.js`),
    /// avec ce bouton (le `geste` est pour le LLM).
    Auto(Place, Affiche),
    /// Le chemin simple : une lecture qui nourrit les ecrans (l'etat, la configuration, le mois) ; elle n'a pas de place.
    Lecture,
    /// Avancee : une lecture qui nourrit les ecrans, hors du chemin simple.
    LectureAvancee,
}

impl Ecran {
    pub fn place(self) -> Option<Place> {
        match self {
            Ecran::Dessine(p) | Ecran::DessineAvance(p) | Ecran::Geste(p) | Ecran::GesteAvance(p) | Ecran::Auto(p, _) => Some(p),
            Ecran::Lecture | Ecran::LectureAvancee => None,
        }
    }

    /// La route est-elle « avancee » (hors du chemin simple) ?
    pub fn avance(self) -> bool {
        match self {
            Ecran::Dessine(_) | Ecran::Geste(_) | Ecran::Lecture => false,
            Ecran::DessineAvance(_) | Ecran::GesteAvance(_) | Ecran::Auto(..) | Ecran::LectureAvancee => true,
        }
    }

    fn mot(self) -> &'static str {
        match self {
            Ecran::Dessine(_) | Ecran::DessineAvance(_) => "dessine",
            Ecran::Geste(_) | Ecran::GesteAvance(_) => "geste",
            Ecran::Auto(..) => "auto",
            Ecran::Lecture | Ecran::LectureAvancee => "lecture",
        }
    }

    /// Ce que `moteur routes` en dit.
    pub fn texte(self) -> String {
        match self {
            Ecran::Dessine(p) | Ecran::DessineAvance(p) => p.nom().to_string(),
            Ecran::Geste(p) | Ecran::GesteAvance(p) => format!("{} (un geste qu'une route rend)", p.nom()),
            Ecran::Auto(p, _) => format!("{} › Avancé", p.nom()),
            Ecran::Lecture | Ecran::LectureAvancee => "(une lecture des écrans)".to_string(),
        }
    }
}

/// LA FORME D'UNE VALEUR DE LA CHARGE : ce que l'ecran saisit (une case, un nombre, un mois…) et ce que le LLM ecrit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Forme {
    Texte,
    OuiNon,
    Nombre,
    /// AAAA-MM.
    Mois,
    /// AAAA-MM-JJ.
    Date,
    /// Une liste de textes.
    Liste,
    /// Une valeur composee (un objet, une liste d'objets) : le `sens` la decrit ; l'ecran ne la saisit pas d'une case.
    Objet,
}

impl Forme {
    pub fn mot(self) -> &'static str {
        match self {
            Forme::Texte => "texte",
            Forme::OuiNon => "oui_non",
            Forme::Nombre => "nombre",
            Forme::Mois => "mois",
            Forme::Date => "date",
            Forme::Liste => "liste",
            Forme::Objet => "objet",
        }
    }
}

/// Un champ de la charge : son nom, ce que l'ecran en montre (`libelle`) et sa forme, son sens pour le LLM, et s'il est
/// « avance » (l'ecran le range sous Avancé, ou seul un fait l'appelle).
#[derive(Clone, Copy, Debug)]
pub struct Champ {
    pub nom: &'static str,
    pub libelle: Affiche,
    pub forme: Forme,
    pub sens: &'static str,
    pub avance: bool,
}

/// Un champ de la charge, tel que le catalogue l'ecrit.
#[derive(serde::Serialize)]
pub struct ChampEcrit {
    nom: &'static str,
    libelle: &'static str,
    forme: &'static str,
    sens: &'static str,
    avance: bool,
}

impl Champ {
    pub fn ecrit(&self) -> ChampEcrit {
        ChampEcrit { nom: self.nom, libelle: self.libelle.texte(), forme: self.forme.mot(), sens: self.sens, avance: self.avance }
    }
}

/// LES CASES DE LA CARTE D'UN CONTRAT PAYE AU MOIS, cases de la charge `changements` (les faits du mois et ses termes). La marque
/// « avancé » range l'ecran et le catalogue a la fois : la page met sous « Avancé » de la carte les cases marquees, et
/// seulement celles que l'engagement appelle (`faits_avances` de la route apercu, `engagement::heures_en_plus::faits_appeles`).
pub(crate) const CASES_D_UN_MOIS_PAYE: &[Champ] = &[
    Champ { nom: "salaire_mensuel", libelle: Affiche::new("Salaire mensuel brut"), forme: Forme::Nombre, sens: "le salaire mensuel brut du contrat", avance: false },
    Champ { nom: "quotite_contrat", libelle: Affiche::new("Heures par mois"), forme: Forme::Nombre, sens: "ses heures par mois", avance: false },
    Champ { nom: "conges_pris", libelle: Affiche::new("Congés pris"), forme: Forme::Objet, sens: "les congés pris du mois : [{debut, fin}]", avance: false },
    Champ { nom: "primes", libelle: Affiche::new("Primes"), forme: Forme::Objet, sens: "les primes du mois : [{montant, rubrique, libelle}]", avance: false },
    Champ {
        nom: "lignes", libelle: Affiche::new("Repas et nuitées"), forme: Forme::Objet,
        sens: "ses repas et nuitées du mois, comme les lignes d'un intermittent : [{rubrique « repas » ou « nuitee », quantite, taux, dates}], aux mêmes limites",
        avance: false,
    },
    Champ { nom: "heures_supplementaires", libelle: Affiche::new("Heures supplémentaires"), forme: Forme::Nombre, sens: "un temps plein : ses heures supplémentaires du mois, que le moteur majore", avance: true },
    Champ {
        nom: "heures_supplementaires_a_50", libelle: Affiche::new("Dont à 50 %"), forme: Forme::Nombre,
        sens: "dont celles au-delà de la huitième d'une semaine (majorées de 50 % quand la convention ne fixe pas les paliers)",
        avance: true,
    },
    Champ { nom: "heures_complementaires", libelle: Affiche::new("Heures complémentaires"), forme: Forme::Nombre, sens: "un temps partiel : ses heures complémentaires du mois, que le moteur majore", avance: true },
];

/// LES CASES DE LA FIN DU CONTRAT, cases de la charge `changements` : les faits du mois ou il finit (`engagement::stock::DE_LA_FIN`),
/// la ou le moteur les ecrit quel que soit le mois de la carte. Ce que chaque motif appelle : `faits_de_rupture` des
/// `natures` de la route etat.
pub(crate) const CASES_DE_LA_FIN: &[Champ] = &[
    Champ { nom: "fin", libelle: Affiche::new("Dernier jour du contrat"), forme: Forme::Date, sens: "le dernier jour du contrat, AAAA-MM-JJ", avance: false },
    Champ { nom: "motif_rupture", libelle: Affiche::new("Motif de la fin"), forme: Forme::Texte, sens: "le code du motif, \"059\" pour une démission : `motifs_rupture` des `natures` de la route etat", avance: false },
    Champ { nom: "notification_rupture", libelle: Affiche::new("Notifiée le"), forme: Forme::Date, sens: "le jour où la rupture a été notifiée, AAAA-MM-JJ", avance: false },
    Champ { nom: "preavis", libelle: Affiche::new("Préavis"), forme: Forme::Objet, sens: "{type, debut, fin} : type de `types_preavis` de la route etat (\"90\" sans préavis), ses dates s'il en a", avance: false },
    Champ { nom: "dernier_jour_paye", libelle: Affiche::new("Dernier jour payé"), forme: Forme::Date, sens: "le dernier jour payé, s'il n'est pas la fin", avance: false },
];

/// La charge de la fiche porte-t-elle les changements d'une carte (`changements`) ?
fn porte_une_carte(charge: &[Champ]) -> bool {
    charge.iter().any(|c| c.nom == "changements")
}

/// Les cases d'une carte que porte la charge de la fiche, s'il y en a.
fn cases(charge: &[Champ]) -> &'static [Champ] {
    if porte_une_carte(charge) { CASES_D_UN_MOIS_PAYE } else { &[] }
}

/// Les cases de la fin du contrat que porte la charge de la fiche, s'il y en a.
fn cases_de_la_fin(charge: &[Champ]) -> &'static [Champ] {
    if porte_une_carte(charge) { CASES_DE_LA_FIN } else { &[] }
}

/// LES NATURES D'UNE CARTE ET LEURS CASES, tirees du type (`engagement::carte::CarteValide::natures`), pour une fiche dont
/// la charge porte une carte ; aucune sinon.
fn natures_de_la_carte(charge: &[Champ]) -> Vec<NatureDecrite> {
    if porte_une_carte(charge) { CarteValide::natures() } else { Vec::new() }
}

/// Un refus : sa classe, et ce qu'il faut corriger.
#[derive(Clone, Copy, Debug)]
pub struct Refus {
    pub classe: Classe,
    pub corriger: &'static str,
}

/// Un refus de la fiche, tel que le catalogue l'ecrit.
#[derive(serde::Serialize)]
pub(crate) struct RefusEcrit {
    classe: Classe,
    corriger: &'static str,
}

/// LA FICHE D'UNE ROUTE TELLE QUE LE CATALOGUE L'ECRIT (`moteur routes --json`, la route `routes`) : ce que le LLM et
/// `js/avance.js` en lisent.
#[derive(serde::Serialize)]
pub struct FicheEcrite {
    nom: &'static str,
    geste: &'static str,
    charge: Vec<ChampEcrit>,
    cases: Vec<ChampEcrit>,
    cases_de_la_fin: Vec<ChampEcrit>,
    natures: Vec<NatureDecrite>,
    exemple: Charge,
    rend: &'static str,
    refus: Vec<RefusEcrit>,
    acte: &'static str,
    avance: bool,
    phrase: usize,
    ecran: Option<&'static str>,
    affichage: &'static str,
    bouton: Option<&'static str>,
}

/// Le catalogue entier, tel que `moteur routes --json` l'imprime.
#[derive(serde::Serialize)]
struct CatalogueEcrit {
    routes: Vec<FicheEcrite>,
}

/// LA FICHE D'UNE ROUTE.
#[derive(Clone, Copy, Debug)]
pub struct Fiche {
    pub route: Route,
    /// Le geste, en une phrase breve.
    pub geste: &'static str,
    pub charge: &'static [Champ],
    /// Une charge que la route accepte sur une compagnie de test (`tests/fichiers/routes.rs`), en JSON.
    pub exemple: &'static str,
    pub rend: &'static str,
    pub refus: &'static [Refus],
    pub acte: Acte,
    /// La place, en caracteres, de la phrase que la route rend pour l'affichage (`phrase`) ; 0 : elle n'en rend pas.
    pub phrase: usize,
    /// Ou la route se voit a l'ecran, et si elle est avancee.
    pub ecran: Ecran,
}

/// Ce que bien des fiches partagent : le mois de l'ecran, et les refus de la forme de la charge.
pub const MOIS: Champ = Champ { nom: "mois", libelle: Affiche::new("Mois"), forme: Forme::Mois, sens: "le mois de l'écran, AAAA-MM", avance: false };
pub const SALARIE: Champ = Champ { nom: "salarie", libelle: Affiche::new("Salarié"), forme: Forme::Texte, sens: "la clé de la fiche du salarié (ou son nom)", avance: false };
pub const MANQUE: Refus = Refus { classe: Classe::Manque, corriger: "le champ de la charge, ou la fiche, que la phrase nomme" };
pub(crate) const ILLISIBLE: Refus = Refus { classe: Classe::Illisible, corriger: "la date ou la valeur que la phrase dit illisible" };
pub const FORME: Refus = Refus { classe: Classe::MalFormee, corriger: "le champ de la charge que la phrase nomme" };
pub const OPERATION: Refus = Refus { classe: Classe::Refuse, corriger: "ce que la phrase dit ; rien n'est écrit" };

impl Fiche {
    /// Le nom de la route.
    pub const fn nom(&self) -> &'static str {
        self.route.nom()
    }

    /// La fiche telle que le catalogue l'ecrit. Un exemple qui n'est pas une charge est un defaut du logiciel.
    pub fn ecrite(&self) -> R<FicheEcrite> {
        let exemple = socle::forme::lire_texte(self.exemple, "Défaut du logiciel : l'exemple d'une route n'est pas une charge.")
            .map_err(|m| m.reclasser(Classe::DefautDuLogiciel))?;
        Ok(FicheEcrite {
            nom: self.nom(),
            geste: self.geste,
            charge: self.charge.iter().map(Champ::ecrit).collect(),
            cases: cases(self.charge).iter().map(Champ::ecrit).collect(),
            cases_de_la_fin: cases_de_la_fin(self.charge).iter().map(Champ::ecrit).collect(),
            natures: natures_de_la_carte(self.charge),
            exemple,
            rend: self.rend,
            refus: self.refus.iter().map(|r| RefusEcrit { classe: r.classe, corriger: r.corriger }).collect(),
            acte: match self.acte {
                Acte::Lit => "lit",
                Acte::Ecrit => "ecrit",
                Acte::Engage => "engage",
            },
            avance: self.ecran.avance(),
            phrase: self.phrase,
            ecran: self.ecran.place().map(Place::nom),
            affichage: self.ecran.mot(),
            bouton: match self.ecran {
                Ecran::Auto(_, b) => Some(b.texte()),
                Ecran::Dessine(_) | Ecran::DessineAvance(_) | Ecran::Geste(_) | Ecran::GesteAvance(_) | Ecran::Lecture | Ecran::LectureAvancee => None,
            },
        })
    }

    pub fn en_texte(&self) -> String {
        let mut t = format!("{}{} : {}\n  Acte : {}\n  Charge :\n", self.nom(), if self.ecran.avance() { " (avancé)" } else { "" }, self.geste, self.acte.mot());
        for c in self.charge {
            t.push_str(&format!("    {}{} : {}\n", c.nom, if c.avance { " (avancé)" } else { "" }, c.sens));
        }
        if !cases(self.charge).is_empty() {
            t.push_str("  Cases de la carte d'un contrat au mois (chacune une case de changements) :\n");
            for c in cases(self.charge) {
                t.push_str(&format!("    {}{} : {}\n", c.nom, if c.avance { " (avancé)" } else { "" }, c.sens));
            }
        }
        if !cases_de_la_fin(self.charge).is_empty() {
            t.push_str("  Cases de la fin du contrat (cases de changements, sur la carte de n'importe quel mois du contrat) :\n");
            for c in cases_de_la_fin(self.charge) {
                t.push_str(&format!("    {} : {}\n", c.nom, c.sens));
            }
        }
        for n in natures_de_la_carte(self.charge) {
            t.push_str(&format!("  Cases d'une carte {} (cases de changements, carte neuve \"{}\") :\n", n.nature, n.nature));
            for c in &n.cases {
                t.push_str(&format!("    {} ({}) : {}\n", c.cle, c.libelle, if c.exige == Exige::Obligatoire { "obligatoire" } else { "facultative, null si rien n'est dit" }));
            }
            for m in &n.motifs_recours {
                let en_plus: Vec<&str> = m.cases.iter().map(|c| c.cle).collect();
                t.push_str(&format!("    motif_recours \"{}\"{}\n", m.code, if en_plus.is_empty() { String::new() } else { format!(" : et {} (facultative)", en_plus.join(", ")) }));
            }
        }
        t.push_str(&format!("  Exemple : echo '{{\"route\":\"{}\",\"charge\":{}}}' | moteur route\n", self.nom(), self.exemple));
        t.push_str(&format!("  Rend : {}\n", self.rend));
        t.push_str(&format!("  Écran : {}\n", self.ecran.texte()));
        if self.phrase > 0 {
            t.push_str(&format!("  Phrase : au plus {} caractères\n", self.phrase));
        }
        t.push_str("  Refus :\n");
        for r in self.refus {
            t.push_str(&format!("    {} : {}\n", r.classe, r.corriger));
        }
        t
    }
}

/// La fiche de `nom` dans `fiches`, si elle y est.
pub fn dans(fiches: &[Fiche], route: Route) -> Option<Fiche> {
    fiches.iter().find(|f| f.route == route).copied()
}

/// La route du catalogue, pour la page (l'acte d'une route, ce qui est avance) : avec `avance`, ce que
/// `moteur routes --json` imprime.
pub const ROUTES: &[Fiche] = &[Fiche {
    route: Route::Routes,
    geste: "Dire les routes : leur fiche, le chemin simple d'abord.",
    charge: &[Champ { nom: "avance", libelle: Affiche::new("Toutes les routes"), forme: Forme::OuiNon, sens: "vrai : toutes les routes, les avancées comprises", avance: true }],
    exemple: r#"{"avance": true}"#,
    rend: "routes : la fiche de chaque route (nom, geste, charge, exemple, rend, refus, acte, avance, phrase)",
    refus: &[FORME],
    acte: Acte::Lit,
    phrase: 0,
    ecran: Ecran::LectureAvancee,
}];

/// La charge de `routes` : toutes les routes (`avance`), ou le chemin simple.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeDuCatalogue {
    avance: Option<bool>,
}

/// Ce que rend `routes` : la phrase, une ligne par route (« nom : geste »), et la fiche de chacune.
#[derive(serde::Serialize)]
struct LeCatalogue {
    phrase: String,
    phrases: Vec<String>,
    routes: Vec<FicheEcrite>,
}

impl Dite for LeCatalogue {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// `routes` : le catalogue, ce que `moteur routes --json` imprime. Il ne porte sur aucune compagnie.
pub fn route(charge: &Charge) -> R<Rendu> {
    en_json(le_catalogue(charge.lire()?)?)
}

fn le_catalogue(c: ChargeDuCatalogue) -> R<LeCatalogue> {
    let fiches: Vec<Fiche> = catalogue().into_iter().filter(|f| c.avance == Some(true) || !f.ecran.avance()).collect();
    let phrases: Vec<String> = fiches.iter().map(|f| format!("{} : {}", f.nom(), f.geste)).collect();
    Ok(LeCatalogue { phrase: format!("Routes : {} (la fiche de chacune : moteur routes NOM).", phrases.len()), phrases, routes: fiches.iter().map(Fiche::ecrite).collect::<R<_>>()? })
}

/// Les modules dont les routes portent leur fiche, dans l'ordre du catalogue : l'ordre ou leurs exemples se jouent.
pub const FICHES: [&[Fiche]; 29] = [
    crate::engagement::ROUTES,
    crate::etat_garde::ROUTES,
    crate::mois::ROUTES,
    crate::etapes::ROUTES,
    crate::imprevus::ROUTES,
    crate::dossier_salarie::ROUTES,
    crate::contrats::ROUTES,
    crate::modeles_contrat::ROUTES,
    crate::depots::ROUTES,
    crate::documents::ROUTES,
    crate::coffre::ROUTES,
    crate::import::ROUTES,
    crate::archives::ROUTES,
    crate::libelles_courts::ROUTES,
    crate::historique::ROUTES,
    crate::fiches::ROUTES,
    crate::identite::ROUTES,
    crate::metiers::ROUTES,
    crate::bulletin::ROUTES,
    crate::gestes::organismes::ROUTES,
    crate::filet::ROUTES,
    crate::historique_mois::ROUTES,
    crate::ouverture::ROUTES,
    crate::rappels::ROUTES,
    crate::gestes::conseils::ROUTES,
    crate::gestes::ouverture::ROUTES,
    crate::obligations::ROUTES,
    crate::accueil::ROUTES,
    ROUTES,
];

/// LE CATALOGUE : toutes les routes, d'une compagnie et de l'installation, chacune une fois, dans l'ordre des modules.
pub fn catalogue() -> Vec<Fiche> {
    FICHES.iter().flat_map(|l| l.iter().copied()).collect()
}

/// `moteur routes [NOM] [--avance] [--json]` : le catalogue en texte (le chemin simple ; toutes avec `--avance`), en JSON
/// (toutes : chaque fiche porte `avance`), ou la fiche entiere d'une route.
pub fn commande(args: &[String]) -> i32 {
    // Une option que `routes` ne prend pas se refuse, comme a `moteur route` (code 2) : sa valeur se lirait comme un nom.
    if let Some(o) = args.iter().find(|a| a.starts_with("--") && !["--json", "--avance"].contains(&a.as_str())) {
        eprintln!("option inconnue : {} (moteur routes [NOM] [--avance] [--json] ; --racine et --date sont de moteur route)", o);
        return 2;
    }
    let json = args.iter().any(|a| a == "--json");
    let avance = args.iter().any(|a| a == "--avance");
    let nom = args.iter().find(|a| !a.starts_with("--"));
    let tout = catalogue();
    if let Some(nom) = nom {
        let Some(f) = Route::lire(nom).and_then(|r| dans(&tout, r)) else {
            eprintln!("route inconnue : {} (moteur routes --avance les dit toutes)", nom);
            return 2;
        };
        if json {
            match f.ecrite() {
                Ok(e) => dire!("{}", socle::ecriture::json_en_ligne(&e)),
                Err(e) => {
                    eprintln!("{}", e.phrase);
                    return 1;
                }
            }
        } else {
            dire_sans_ligne!("{}", f.en_texte());
        }
        return 0;
    }
    if json {
        return match catalogue().iter().map(Fiche::ecrite).collect::<R<Vec<_>>>() {
            Ok(routes) => {
                dire!("{}", socle::ecriture::json_en_ligne(&CatalogueEcrit { routes }));
                0
            }
            Err(e) => {
                eprintln!("{}", e.phrase);
                1
            }
        };
    }
    let montrees: Vec<&Fiche> = tout.iter().filter(|f| avance || !f.ecran.avance()).collect();
    dire!("Les routes (echo '{{\"route\":NOM,\"charge\":{{…}}}}' | moteur route ; la fiche : moteur routes NOM ; toutes : --avance)\n");
    let large = montrees.iter().map(|f| f.nom().len()).fold(0, usize::max);
    for f in &montrees {
        dire!("  {:<large$}  {:<5}  {}  [{}]", f.nom(), match f.acte { Acte::Lit => "lit", Acte::Ecrit => "écrit", Acte::Engage => "engage" }, f.geste, f.ecran.texte());
    }
    let cachees = tout.len() - montrees.len();
    if cachees > 0 {
        dire!("\n{} routes avancées ne sont pas montrées : moteur routes --avance", cachees);
    }
    0
}
