//! LES MARQUEURS D'UN MODELE DE DOCUMENT (`{{employeur.siret}}`), DECLARES UNE FOIS : chacun une variante de `Marqueur`,
//! et son nom texte, celui que le modele porte, ici seulement. Ils sont dans `droit` pour que la table livree des
//! natures (`contrats-base.json`, `mentions_obligatoires`) les lise a sa porte (`natures::Mention`) ; la lecture d'un
//! modele (`documents::marqueurs::ModeleLu`) et les valeurs d'un document restent dans `documents`.
//!
//! L'ordre de la declaration ne compte pas : un marqueur se trie par son nom.

macro_rules! marqueurs {
    ($($v:ident = $nom:literal),* $(,)?) => {
        /// UN MARQUEUR qu'un modele peut citer. Il s'ecrit, se lit et se trie par son nom (`nom`) : les clauses d'un
        /// contrat parti, au journal des envois, se rangent dans l'ordre de leurs noms.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Marqueur { $($v),* }

        impl Marqueur {
            /// Son nom dans le modele, sans les accolades.
            pub fn nom(self) -> &'static str {
                match self { $(Marqueur::$v => $nom),* }
            }

            /// Le marqueur de ce nom ; `None` : le logiciel ne le connait pas.
            pub fn lire(nom: &str) -> Option<Marqueur> {
                match nom { $($nom => Some(Marqueur::$v),)* _ => None }
            }
        }
    };
}

marqueurs! {
    EmployeurRaisonSociale = "employeur.raison_sociale",
    EmployeurAdresse = "employeur.adresse",
    EmployeurCodePostal = "employeur.code_postal",
    EmployeurCommune = "employeur.commune",
    EmployeurSiret = "employeur.siret",
    EmployeurNaf = "employeur.naf",
    EmployeurLicence = "employeur.licence",
    EmployeurIdcc = "employeur.idcc",
    EmployeurConvention = "employeur.convention",
    EmployeurCongesspectacles = "employeur.congesspectacles",
    EmployeurSignataire = "employeur.signataire",
    EmployeurSignataireFonction = "employeur.signataire_fonction",
    EmployeurQualiteDe = "employeur.qualite_de",
    SalarieNom = "salarie.nom",
    SalariePrenom = "salarie.prenom",
    SalarieNomComplet = "salarie.nom_complet",
    SalarieDateNaissance = "salarie.date_naissance",
    SalarieLieuNaissance = "salarie.lieu_naissance",
    SalarieAdresse = "salarie.adresse",
    SalarieCodePostal = "salarie.code_postal",
    SalarieCommune = "salarie.commune",
    SalarieNir = "salarie.nir",
    SalarieEmploi = "salarie.emploi",
    SalarieQualiteDe = "salarie.qualite_de",
    ContratNumero = "contrat.numero",
    ContratDebut = "contrat.debut",
    ContratFin = "contrat.fin",
    ContratDatesTravail = "contrat.dates_travail",
    ContratLieu = "contrat.lieu",
    ContratDatePaiement = "contrat.date_paiement",
    RemunerationDetail = "remuneration.detail",
    RemunerationPeriode = "remuneration.periode",
    RemunerationCachets = "remuneration.cachets",
    RemunerationTauxCachet = "remuneration.taux_cachet",
    RemunerationBrut = "remuneration.brut",
    RemunerationNetEstime = "remuneration.net_estime",
    SpectacleTitre = "spectacle.titre",
    SpectacleNumeroObjet = "spectacle.numero_objet",
    CertificatDate = "certificat.date",
    CertificatPortabilite = "certificat.portabilite",
    ContratClausesParticulieres = "contrat.clauses_particulieres",
    ContratDureeTravail = "contrat.duree_travail",
    ContratEmploi = "contrat.emploi",
    ContratMotif = "contrat.motif",
    ContratNature = "contrat.nature",
    ContratPaiement = "contrat.paiement",
    ContratPeriodeEssai = "contrat.periode_essai",
    ContratPersonneRemplacee = "contrat.personne_remplacee",
    MentionsConges = "mentions.conges",
    MentionsFormation = "mentions.formation",
    MentionsProtectionSociale = "mentions.protection_sociale",
    MentionsRupture = "mentions.rupture",
    ModificationApres = "modification.apres",
    ModificationArticle = "modification.article",
    ModificationAvant = "modification.avant",
    ModificationNumero = "modification.numero",
    ModificationPhrase = "modification.phrase",
    OrganismesPrevoyance = "organismes.prevoyance",
    OrganismesRetraite = "organismes.retraite",
    RecuDate = "recu.date",
    RemplaceDate = "remplace.date",
    RemplaceNumero = "remplace.numero",
    RemunerationHorsBrut = "remuneration.hors_brut",
    SalarieCaseSignature = "salarie.case_signature",
    SalarieCategorie = "salarie.categorie",
    SoldeInventaire = "solde.inventaire",
    SoldeNet = "solde.net",
    AvenantNumero = "avenant.numero",
    AvenantDateContrat = "avenant.date_contrat",
    AvenantReference = "avenant.reference",
    AvenantEffet = "avenant.effet",
    AvenantDate = "avenant.date",
}

impl Ord for Marqueur {
    fn cmp(&self, autre: &Self) -> std::cmp::Ordering {
        self.nom().cmp(autre.nom())
    }
}

impl PartialOrd for Marqueur {
    fn partial_cmp(&self, autre: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(autre))
    }
}

impl serde::Serialize for Marqueur {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nom())
    }
}

impl<'de> serde::Deserialize<'de> for Marqueur {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let nom = <String as serde::Deserialize>::deserialize(d)?;
        Marqueur::lire(&nom).ok_or_else(|| serde::de::Error::custom(format!("« {} » n'est pas un champ que le logiciel connaît", nom)))
    }
}

impl std::fmt::Display for Marqueur {
    /// Tel que le modele l'ecrit : `{{employeur.siret}}`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{{{}}}}}", self.nom())
    }
}
