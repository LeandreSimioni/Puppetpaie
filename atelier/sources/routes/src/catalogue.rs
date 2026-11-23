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
        ConventionDuMetier = "convention-du-metier",
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
        ImportationIdentite = "importation-identite",
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
        Fiche = "fiche",
        TaxeSurLesSalairesRegime = "taxe-sur-les-salaires-regime",
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
        IdentiteImage = "identite-image",
        PoserIdentite = "poser-identite",
        RetirerIdentite = "retirer-identite",
        MetiersCatalogue = "metiers-catalogue",
        MetierRetenu = "metier-retenu",
        MetierCree = "metier-cree",
        MetierModifie = "metier-modifie",
        BulletinModeleApercu = "bulletin-modele-apercu",
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
        ValeursIllisibles = "valeurs-illisibles",
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
        Basculer = "basculer",
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

/// CE QUE FAIT LA ROUTE PENDANT QU'UNE VALEUR D'UNE FICHE ATTEND D'ETRE SAISIE DE NOUVEAU (`crate::redire`) : tant que la
/// compagnie n'a pas repondu, rien ne se calcule, ne s'ecrit de soi-meme ni ne part. Le barrage (`redire::barrage`) ne lit
/// que ceci : chaque fiche le dit, une route neuve ne compile pas sans le dire.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PendantLaDemande {
    /// Elle se sert : elle lit l'etat de la compagnie sans rien calculer, elle est la demande ou sa reponse, ou elle ne
    /// porte sur aucune compagnie (l'accueil, le catalogue, ecrire au developpeur).
    Servie,
    /// Elle refuse en renvoyant a la demande (`Classe::ValeurARedire`).
    Refusee,
}

/// LES PLACES DE L'ECRAN, telles que la page les nomme : l'onglet, et le sous-onglet de la Configuration. Une place se
/// nomme d'une seule facon, ici ; une fiche ne peut nommer qu'une de ces places.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    Paie,
    Fiches,
    Metiers,
    Organismes,
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
    /// Le chemin simple : la lecture qui rend l'ecran de cette place tel qu'on le voit et le saisit
    /// (`ecran::EcranRendu` : ce qu'il dit en tete, ses rangs, ses cases et le geste qui les enregistre) ; la page le
    /// pose depuis sa reponse (`js/avance.js`, `poserLEcranRendu`), sans la nommer.
    Rendu(Place),
    /// Le chemin simple : la page de cette place l'appelle (son ecran est dessine), par ce bouton (viser un montant
    /// sur la carte).
    DessineAvecBouton(Place, Affiche),
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
            Ecran::Dessine(p) | Ecran::Rendu(p) | Ecran::DessineAvecBouton(p, _) | Ecran::DessineAvance(p) | Ecran::Geste(p) | Ecran::GesteAvance(p) | Ecran::Auto(p, _) => Some(p),
            Ecran::Lecture | Ecran::LectureAvancee => None,
        }
    }

    /// La route est-elle « avancee » (hors du chemin simple) ?
    pub fn avance(self) -> bool {
        match self {
            Ecran::Dessine(_) | Ecran::Rendu(_) | Ecran::DessineAvecBouton(..) | Ecran::Geste(_) | Ecran::Lecture => false,
            Ecran::DessineAvance(_) | Ecran::GesteAvance(_) | Ecran::Auto(..) | Ecran::LectureAvancee => true,
        }
    }

    fn mot(self) -> &'static str {
        match self {
            Ecran::Dessine(_) | Ecran::DessineAvecBouton(..) | Ecran::DessineAvance(_) => "dessine",
            Ecran::Rendu(_) => "rendu",
            Ecran::Geste(_) | Ecran::GesteAvance(_) => "geste",
            Ecran::Auto(..) => "auto",
            Ecran::Lecture | Ecran::LectureAvancee => "lecture",
        }
    }

    /// Ce que `moteur routes` en dit.
    pub fn texte(self) -> String {
        match self {
            Ecran::Rendu(p) => format!("{} (l'écran que la route rend)", p.nom()),
            Ecran::Dessine(p) | Ecran::DessineAvecBouton(p, _) | Ecran::DessineAvance(p) => p.nom().to_string(),
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
    /// UN MONTANT (des euros, ou des euros par unite : le taux d'une ligne de paie) : il se montre comme la page ecrit un
    /// montant, et se tape avec une virgule ou un point (`socle::grandeurs::lu::tapee`).
    Montant,
    /// AAAA-MM.
    Mois,
    /// AAAA-MM-JJ : l'ecran la fait choisir dans son calendrier (`js/calendrier.js`), d'un jour.
    Date,
    /// Des jours, chacun AAAA-MM-JJ : l'ecran les fait cocher dans le meme calendrier, plusieurs a la fois (les jours
    /// d'une ligne de paie).
    Jours,
    /// Une liste de textes.
    Liste,
    /// UNE QUANTITE D'UNE LIGNE DE PAIE, dans l'unite de sa rubrique : `{"cachets": 2}`, la cle que dit `quantite` de la
    /// rubrique (route etat) ; la grandeur s'ecrit elle-meme (des cachets en entier).
    Quantite,
    /// DES LIGNES DE PAIE : chacune dit les cles de sa rubrique (`rubriques[].cles` de la route etat), chaque cle sa forme ;
    /// l'ecran les montre en lignes, une colonne par cle.
    Lignes,
    /// Une valeur composee (un objet, une liste d'objets) : le `sens` la decrit ; l'ecran ne la saisit pas d'une case.
    Objet,
    /// UNE VALEUR D'UNE LISTE FERMEE : ses valeurs et leurs libelles, tires du type qui la lit (`ListeDeChoix::de`) ;
    /// l'ecran la fait choisir dans une liste deroulante, la ligne de commande la lit telle quelle.
    Choix(ListeDeChoix),
    /// Une valeur d'une liste fermee que la route rend a cote, tiree des donnees, sous la cle de la case : les salaries,
    /// les spectacles, les metiers et les etablissements d'une carte (`choix` de la route etat), le genre d'un avantage en
    /// nature et le lieu d'une ligne de frais (`rubriques[].choix`), la rubrique d'une ligne (celles que la carte propose,
    /// `propose`) ; l'ecran la fait choisir.
    ChoixRendu,
    /// UNE VALEUR FAITE DE PARTIES, chacune sa cle, son libelle et sa forme simple (`Partie`) : la route etat les rend
    /// sous la cle de la case (`parties`) ; la partie qui se choisit offre ce que la route rend sous la meme cle
    /// (`choix`), et chaque valeur offerte dit les parties qu'elle ouvre (`ouvre`) : une partie qui n'est pas de toujours
    /// ne se saisit que si la valeur choisie l'ouvre. Le preavis d'une fin de contrat : son type, puis ses dates quand
    /// le type en a.
    Parties(&'static [Partie]),
    /// DES RANGEES D'UNE FICHE : une liste d'objets, chacun les memes parties (`routes::fiches::PartieDeLaFiche`, rendues
    /// a cote de la case), qu'on ajoute et retire (les autres etablissements de l'entreprise).
    Rangees,
    /// Du HTML colle (la signature des courriels) : le moteur le nettoie et en dit le poids (`champs::Forme::Html`).
    Html,
    /// UN FICHIER QUE LA PERSONNE CHOISIT SUR SON POSTE, d'un des types de media que la fiche admet (`image/png`…) : la
    /// charge porte ses octets en base64 (une adresse data: aussi), ce que la ligne de commande donne pour le meme geste.
    Fichier(&'static [&'static str]),
    /// UN MOT DE PASSE QU'UN HUMAIN TAPE : la case le masque, et ni la page ni le journal de ses gestes ne le gardent ; il
    /// part dans la charge, ce que la ligne de commande ne donne jamais (le moteur le demande lui-meme a un humain).
    MotDePasse,
}

/// UNE PARTIE D'UNE VALEUR FAITE DE PARTIES (`Forme::Parties`) : sa cle dans la valeur, son libelle, sa forme, et si elle
/// est de toujours (sinon, la valeur choisie d'une autre partie l'ouvre).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Partie {
    pub cle: &'static str,
    pub libelle: Affiche,
    pub forme: Forme,
    pub toujours: bool,
}

/// Une partie, telle que la route etat l'ecrit.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct PartieEcrite {
    cle: &'static str,
    libelle: &'static str,
    forme: Forme,
    toujours: bool,
}

impl Partie {
    pub(crate) fn ecrite(&self) -> PartieEcrite {
        PartieEcrite { cle: self.cle, libelle: self.libelle.texte(), forme: self.forme, toujours: self.toujours }
    }
}

/// LES PARTIES DU PREAVIS D'UNE FIN DE CONTRAT (`engagement::saisie::Preavis`, S21.G00.62.006 a 008) : son type, au
/// choix parmi ceux qui se saisissent (`droit::preavis::TypePreavis`), puis son debut et sa fin quand le type en a.
pub const PARTIES_DU_PREAVIS: &[Partie] = &[
    Partie { cle: "type", libelle: Affiche::new("Type"), forme: Forme::ChoixRendu, toujours: true },
    Partie { cle: "debut", libelle: Affiche::new("Du"), forme: Forme::Date, toujours: false },
    Partie { cle: "fin", libelle: Affiche::new("Au"), forme: Forme::Date, toujours: false },
];

/// UNE FORME S'ECRIT PAR SON MOT (`Forme::mot`) : la page et le LLM la lisent ainsi ; les valeurs d'une liste fermee
/// s'ecrivent a cote (`ChampEcrit.choix`).
impl serde::Serialize for Forme {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.mot())
    }
}

/// LES VALEURS D'UNE LISTE FERMEE (`Forme::Choix`), tirees du type qui lit le champ (`socle::liste::ListeFermee`) : elle
/// ne se fabrique que par lui (`de`), jamais d'une liste ecrite a cote.
#[derive(Clone, Copy)]
pub struct ListeDeChoix(fn() -> R<Vec<ChoixEcrit>>);

impl ListeDeChoix {
    /// Les valeurs du type `T`, chacune son code et son libelle.
    pub const fn de<T: socle::liste::ListeFermee>() -> ListeDeChoix {
        ListeDeChoix(ecrits::<T>)
    }

    /// Les valeurs, ecrites.
    pub fn ecrites(self) -> R<Vec<ChoixEcrit>> {
        (self.0)()
    }
}

/// Deux listes sont la meme si elles viennent du meme type.
impl PartialEq for ListeDeChoix {
    fn eq(&self, autre: &ListeDeChoix) -> bool {
        std::ptr::fn_addr_eq(self.0, autre.0)
    }
}

impl Eq for ListeDeChoix {}

impl std::fmt::Debug for ListeDeChoix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ListeDeChoix")
    }
}

/// Les valeurs du type `T`, ecrites.
fn ecrits<T: socle::liste::ListeFermee>() -> R<Vec<ChoixEcrit>> {
    T::TOUTES.iter().map(|v| Ok(ChoixEcrit { code: v.code()?, libelle: v.libelle()? })).collect()
}

/// Une valeur d'une liste fermee, telle que le catalogue l'ecrit : ce que la charge ecrit (`code`) et ce que l'ecran en
/// montre.
#[derive(serde::Serialize)]
pub struct ChoixEcrit {
    code: String,
    libelle: &'static str,
}

impl Forme {
    pub fn mot(self) -> &'static str {
        match self {
            Forme::Texte => "texte",
            Forme::OuiNon => "oui_non",
            Forme::Nombre => "nombre",
            Forme::Montant => "montant",
            Forme::Mois => "mois",
            Forme::Date => "date",
            Forme::Jours => "jours",
            Forme::Liste => "liste",
            Forme::Quantite => "quantite",
            Forme::Lignes => "lignes",
            Forme::Objet => "objet",
            Forme::Choix(_) => "choix",
            Forme::ChoixRendu => "choix_rendu",
            Forme::Parties(_) => "parties",
            Forme::Rangees => "rangees",
            Forme::Html => "html",
            Forme::Fichier(_) => "fichier",
            Forme::MotDePasse => "mot_de_passe",
        }
    }

    /// Les valeurs d'une liste fermee, ecrites ; aucune pour une autre forme.
    fn choix(self) -> R<Option<Vec<ChoixEcrit>>> {
        match self {
            Forme::Choix(c) => Ok(Some(c.ecrites()?)),
            Forme::Texte | Forme::OuiNon | Forme::Nombre | Forme::Montant | Forme::Mois | Forme::Date | Forme::Jours | Forme::Liste | Forme::Quantite | Forme::Lignes | Forme::Objet | Forme::ChoixRendu | Forme::Parties(_) | Forme::Rangees | Forme::Html | Forme::Fichier(_) | Forme::MotDePasse => Ok(None),
        }
    }

    /// Les types de media qu'un fichier admet ; aucun pour une autre forme.
    fn admis(self) -> Option<&'static [&'static str]> {
        match self {
            Forme::Fichier(a) => Some(a),
            Forme::Texte | Forme::OuiNon | Forme::Nombre | Forme::Montant | Forme::Mois | Forme::Date | Forme::Jours | Forme::Liste | Forme::Quantite | Forme::Lignes | Forme::Objet | Forme::Choix(_) | Forme::ChoixRendu | Forme::Parties(_) | Forme::Rangees | Forme::Html | Forme::MotDePasse => None,
        }
    }

    /// LA FORME D'UNE CASE DE LA CARTE, tiree du type de son champ (`engagement::saisie::Carte`) : un jour se choisit
    /// dans le calendrier, un nombre se tape, un oui-non se coche, une fiche (salarie, spectacle, metier, etablissement)
    /// se choisit dans ce que la route etat rend (`choix`) ; les lignes sont des lignes (`rubriques[].cles` de la route
    /// etat) ; une autre valeur composee (les conges, le preavis) est un objet. Chaque champ y est classe : un
    /// champ neuf de la carte ne compile pas tant que sa forme n'est pas dite. C'est la seule declaration de la forme d'une
    /// case de la carte.
    pub const fn de_la_case(c: engagement::saisie::Champ) -> Forme {
        use engagement::saisie::Champ as C;
        match c {
            C::Debut | C::Fin | C::DatePaiement | C::NotificationRupture | C::EngagementLicenciement | C::SignatureConvention | C::DernierJourPaye | C::DateModification => Forme::Date,
            C::QuotiteContrat
            | C::QuotiteReference
            | C::PeriodeEssaiJours
            | C::Jours
            | C::HeuresSupplementaires
            | C::HeuresSupplementairesA50
            | C::HeuresComplementaires
            | C::PreavisMois
            | C::JoursContrat => Forme::Nombre,
            C::SalaireMensuel | C::IndemniteLicenciement => Forme::Montant,
            C::FauteGrave | C::EnDroitALaRetraite | C::PreavisNonEffectue | C::PrecariteDue | C::Importe => Forme::OuiNon,
            C::Salarie | C::Spectacle | C::Etablissement | C::Metier | C::MotifRupture | C::Transaction | C::ModeleContrat | C::Nature | C::MotifRecours => {
                Forme::ChoixRendu
            }
            C::Numero
            | C::Lieu
            | C::ModaliteTempsTravail
            | C::Emploi
            | C::PcsEse
            | C::CodeEmploiFt
            | C::Statut
            | C::StatutConventionnel
            | C::StatutCategorielRc
            | C::PersonneRemplacee
            | C::RepartitionHoraire
            | C::ClausesParticulieres
            | C::PrecaritePourquoi => Forme::Texte,
            C::Lignes => Forme::Lignes,
            C::Preavis => Forme::Parties(PARTIES_DU_PREAVIS),
            C::CongesPris | C::Primes | C::Avant | C::Valide => Forme::Objet,
        }
    }

    /// LA FORME D'UNE CLE D'UNE LIGNE DE PAIE (`engagement::ligne::Cle`) : ses jours (`dates`, et ceux des mois suivants,
    /// `planning`) se cochent dans le calendrier, ses nombres se tapent, ses cases de frais se cochent, son genre et son
    /// lieu hors de la metropole se choisissent dans ce que la route etat rend de sa rubrique.
    pub fn de_la_cle(c: engagement::ligne::Cle) -> Forme {
        use engagement::ligne::Cle as C;
        match c {
            C::Dates | C::Planning => Forme::Jours,
            C::Quantite => Forme::Quantite,
            C::Heures | C::TauxMajoration => Forme::Nombre,
            C::Taux | C::Base | C::Montant => Forme::Montant,
            C::Cases => Forme::Liste,
            C::Rubrique | C::Genre | C::Territoire => Forme::ChoixRendu,
            C::Lieu => Forme::Texte,
        }
    }
}

/// LES NATURES D'UNE CARTE ET LEURS CASES, chacune sa forme (`Forme::de_la_case`) : ce que le catalogue et la route etat
/// en disent.
pub fn natures_des_cartes() -> R<Vec<NatureDecrite<Forme>>> {
    CarteValide::natures(Forme::de_la_case)
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
    forme: Forme,
    #[serde(skip_serializing_if = "Option::is_none")]
    choix: Option<Vec<ChoixEcrit>>,
    /// Les types de media qu'un fichier admet (`Forme::Fichier`).
    #[serde(skip_serializing_if = "Option::is_none")]
    admis: Option<&'static [&'static str]>,
    sens: &'static str,
    avance: bool,
}

impl Champ {
    pub fn ecrit(&self) -> R<ChampEcrit> {
        Ok(ChampEcrit { nom: self.nom, libelle: self.libelle.texte(), forme: self.forme, choix: self.forme.choix()?, admis: self.forme.admis(), sens: self.sens, avance: self.avance })
    }
}

/// La charge de la fiche porte-t-elle les changements d'une carte (`changements`) ?
fn porte_une_carte(charge: &[Champ]) -> bool {
    charge.iter().any(|c| c.nom == "changements")
}

/// LES NATURES D'UNE CARTE ET LEURS CASES, tirees du type (`engagement::carte::CarteValide::natures`), pour une fiche dont
/// la charge porte une carte ; aucune sinon.
fn natures_de_la_carte(charge: &[Champ]) -> R<Vec<NatureDecrite<Forme>>> {
    if porte_une_carte(charge) { natures_des_cartes() } else { Ok(Vec::new()) }
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
    natures: Vec<NatureDecrite<Forme>>,
    exemple: Charge,
    rend: &'static str,
    refus: Vec<RefusEcrit>,
    acte: &'static str,
    avance: bool,
    /// La longueur permise de la phrase que la route rend (`Fiche::phrase`).
    longueur_de_phrase: usize,
    ecran: Option<&'static str>,
    affichage: &'static str,
    bouton: Option<&'static str>,
    /// Vrai : elle se sert pendant qu'une valeur attend d'etre saisie de nouveau (`Fiche::pendant_la_demande`).
    servie_pendant_la_demande: bool,
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
    /// Ce qu'elle fait pendant qu'une valeur d'une fiche attend d'etre saisie de nouveau.
    pub pendant_la_demande: PendantLaDemande,
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
            charge: self.charge.iter().map(Champ::ecrit).collect::<R<_>>()?,
            natures: natures_de_la_carte(self.charge)?,
            exemple,
            rend: self.rend,
            refus: self.refus.iter().map(|r| RefusEcrit { classe: r.classe, corriger: r.corriger }).collect(),
            acte: match self.acte {
                Acte::Lit => "lit",
                Acte::Ecrit => "ecrit",
                Acte::Engage => "engage",
            },
            avance: self.ecran.avance(),
            longueur_de_phrase: self.phrase,
            ecran: self.ecran.place().map(Place::nom),
            affichage: self.ecran.mot(),
            bouton: match self.ecran {
                Ecran::Auto(_, b) | Ecran::DessineAvecBouton(_, b) => Some(b.texte()),
                Ecran::Dessine(_) | Ecran::Rendu(_) | Ecran::DessineAvance(_) | Ecran::Geste(_) | Ecran::GesteAvance(_) | Ecran::Lecture | Ecran::LectureAvancee => None,
            },
            servie_pendant_la_demande: self.pendant_la_demande == PendantLaDemande::Servie,
        })
    }

    pub fn en_texte(&self) -> R<String> {
        let mut t = format!("{}{} : {}\n  Acte : {}\n  Charge :\n", self.nom(), if self.ecran.avance() { " (avancé)" } else { "" }, self.geste, self.acte.mot());
        for c in self.charge {
            let admis = match c.forme.admis() {
                Some(a) => format!(" (un fichier : {})", a.join(", ")),
                None => String::new(),
            };
            // Une liste fermee se lit telle que l'ecran la propose : chaque code et son libelle.
            let choix = match c.forme.choix()? {
                Some(v) => format!(" ; au choix : {}", v.iter().map(|x| format!("{} ({})", x.code, x.libelle)).collect::<Vec<_>>().join(", ")),
                None => String::new(),
            };
            t.push_str(&format!("    {}{} : {}{}{}\n", c.nom, if c.avance { " (avancé)" } else { "" }, c.sens, admis, choix));
        }
        for n in natures_de_la_carte(self.charge)? {
            t.push_str(&format!(
                "  Cases d'une carte {} (cases de changements, carte neuve \"{}\" ; celles qu'un fait ouvre, et la place de chacune : `carte.cases` de chaque paie de la route apercu) :\n",
                n.nature, n.nature
            ));
            for c in &n.cases {
                t.push_str(&format!("    {} ({}, {}) : {}\n", c.cle, c.libelle, c.forme.mot(), if c.exige == Exige::Obligatoire { "obligatoire" } else { "facultative, null si rien n'est dit" }));
            }
            for m in &n.motifs_recours {
                let en_plus: Vec<&str> = m.cases.iter().map(|c| c.cle).collect();
                t.push_str(&format!("    motif_recours \"{}\"{}\n", m.code, if en_plus.is_empty() { String::new() } else { format!(" : et {} (facultative)", en_plus.join(", ")) }));
            }
        }
        t.push_str(&format!("  Exemple : echo '{{\"route\":\"{}\",\"charge\":{}}}' | moteur route\n", self.nom(), self.exemple));
        t.push_str(&format!("  Rend : {}\n", self.rend));
        t.push_str(&format!("  Écran : {}\n", self.ecran.texte()));
        if self.pendant_la_demande == PendantLaDemande::Servie {
            t.push_str("  Servie aussi quand une valeur d'une fiche attend d'être saisie de nouveau\n");
        }
        if self.phrase > 0 {
            t.push_str(&format!("  Phrase : au plus {} caractères\n", self.phrase));
        }
        t.push_str("  Refus :\n");
        for r in self.refus {
            t.push_str(&format!("    {} : {}\n", r.classe, r.corriger));
        }
        Ok(t)
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
    rend: "routes : la fiche de chaque route (nom, geste, charge, exemple, rend, refus, acte, avance, longueur_de_phrase)",
    refus: &[FORME],
    acte: Acte::Lit,
    phrase: 0,
    ecran: Ecran::LectureAvancee,
    pendant_la_demande: PendantLaDemande::Servie,
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
    phrase: socle::texte::Phrase,
    phrases: Vec<socle::texte::Phrase>,
    routes: Vec<FicheEcrite>,
}

impl Dite for LeCatalogue {
    fn phrase(&self) -> &socle::texte::Phrase {
        &self.phrase
    }
}

/// `routes` : le catalogue, ce que `moteur routes --json` imprime. Il ne porte sur aucune compagnie.
pub fn route(route: Route, charge: &Charge) -> R<Rendu> {
    match route {
        Route::Routes => {
            let catalogue: LeCatalogue = le_catalogue(charge.lire()?)?;
            en_json(catalogue)
        }
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

fn le_catalogue(c: ChargeDuCatalogue) -> R<LeCatalogue> {
    let fiches: Vec<Fiche> = catalogue().into_iter().filter(|f| c.avance == Some(true) || !f.ecran.avance()).collect();
    let phrases: Vec<socle::texte::Phrase> = fiches.iter().map(|f| socle::phrase!("{} : {}", f.nom(), f.geste)).collect();
    Ok(LeCatalogue { phrase: socle::phrase!("Routes : {} (la fiche de chacune : moteur routes NOM).", phrases.len()), phrases, routes: fiches.iter().map(Fiche::ecrite).collect::<R<_>>()? })
}

/// Les modules dont les routes portent leur fiche, dans l'ordre du catalogue : l'ordre ou leurs exemples se jouent.
pub const FICHES: [&[Fiche]; 31] = [
    crate::engagement::ROUTES,
    crate::etat_garde::ROUTES,
    crate::mois::ROUTES,
    crate::convention_du_metier::ROUTES,
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
    crate::redire::ROUTES,
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
pub fn commande(args: &[socle::ligne::Argument]) -> std::process::ExitCode {
    use std::process::ExitCode;
    let args = socle::ligne::textes(args);
    // Une option que `routes` ne prend pas se refuse, comme a `moteur route` (code 2) : sa valeur se lirait comme un nom.
    if let Some(o) = args.iter().find(|a| a.starts_with("--") && !["--json", "--avance"].contains(&a.as_str())) {
        eprintln!("option inconnue : {} (moteur routes [NOM] [--avance] [--json] ; --racine et --date sont de moteur route)", o);
        return ExitCode::from(2);
    }
    let json = args.iter().any(|a| a == "--json");
    let avance = args.iter().any(|a| a == "--avance");
    let nom = args.iter().find(|a| !a.starts_with("--"));
    let tout = catalogue();
    if let Some(nom) = nom {
        let Some(f) = Route::lire(nom).and_then(|r| dans(&tout, r)) else {
            eprintln!("route inconnue : {} (moteur routes --avance les dit toutes)", nom);
            return ExitCode::from(2);
        };
        let ecrite = if json { f.ecrite().map(|e| format!("{}\n", socle::ecriture::json_en_ligne(&e))) } else { f.en_texte() };
        match ecrite {
            Ok(t) => dire_sans_ligne!("{}", t),
            Err(e) => {
                eprintln!("{}", e.phrase);
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }
    if json {
        return match catalogue().iter().map(Fiche::ecrite).collect::<R<Vec<_>>>() {
            Ok(routes) => {
                dire!("{}", socle::ecriture::json_en_ligne(&CatalogueEcrit { routes }));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{}", e.phrase);
                ExitCode::FAILURE
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
    ExitCode::SUCCESS
}
