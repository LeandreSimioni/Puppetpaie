//! LA CARTE D'UN ENGAGEMENT : ce que la personne tape pour un mois, et ce que l'engagement dit de ce mois (sa vue,
//! `stock::Objet::vue`). Une seule forme, declaree ici : la charge d'une route qui ecrit ou calcule une carte, la vue que
//! l'ecran ouvre et que le calcul lit, chacune lue par `socle::forme` ; une cle inconnue refuse en se nommant.
//!
//! Chaque champ appartient a un groupe (`Groupe`) : ce que tous les mois de l'engagement partagent, ses termes, les faits
//! d'un mois, sa fin, ce que la vue deduit, ce que la vue dit de lui. Le stock compose et ecrit les cartes par leurs
//! groupes et par leurs champs (`Champ`), jamais par le nom d'une cle.

use serde::{Deserialize, Serialize};
use socle::date::Jour;
use socle::echec::R;
use socle::forme::NombreLu;

use crate::ligne::{DateDeLigne, LignePaie};

/// LES GROUPES DES CHAMPS D'UNE CARTE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Groupe {
    /// Ce que tous les mois de l'engagement partagent : le salarie, les bornes du contrat, son numero.
    Partage,
    /// LES TERMES DU CONTRAT, qui valent jusqu'a ce qu'ils changent (`stock::Avenant`) : sa nature, son motif, sa duree du
    /// travail, son salaire, son emploi, ce que son contrat ecrit dit en plus (`ContratEcrit`), son
    /// spectacle, son lieu, son etablissement et son metier.
    Terme,
    /// Ce qui n'appartient qu'a un mois : ses lignes (celles d'un contrat paye au mois se composent chaque mois), ses
    /// conges pris, ses primes, ses heures en plus, la date de son paiement.
    Fait,
    /// CE QUE LA FIN DU CONTRAT DIT D'ELLE : son motif, ses dates (notifiee, engagee, signee), son preavis, la
    /// transaction, le dernier jour paye, et ce que le motif appelle. Ce sont les faits du mois ou le contrat finit : la
    /// paie de ce mois-la et le signalement de fin de contrat les lisent (S21.G00.62, S21.G00.63), aucun autre mois.
    Fin,
    /// Ce que la vue d'un mois porte et que le moteur deduit, qui ne s'ecrit pas : les jours du contrat entier, que la DSN
    /// declare chaque mois (S21.G00.40.013) ; la date de l'avenant du mois et les termes d'avant (`stock::Objet::vue`).
    Deduit,
    /// Ce que la vue dit de l'engagement ecrit : qu'il est valide, qu'il est importe.
    Vue,
}

/// LA CARTE ET SES CHAMPS, declares une fois : la structure telle qu'elle s'ecrit, et, de chaque champ marque
/// `#[champ(Groupe, Champ)]`, son nom (`Champ`), son groupe, et ce qui le lit et l'ecrit sans passer par son nom de cle.
macro_rules! carte {
    (
        $(#[$sattr:meta])*
        pub struct $carte:ident {
            $( #[champ($groupe:ident, $variante:ident)] $(#[$attr:meta])* pub $champ:ident : Option<$t:ty>, )*
        }
    ) => {
        $(#[$sattr])*
        pub struct $carte {
            $( $(#[$attr])* pub $champ: Option<$t>, )*
        }

        /// UN CHAMP DE LA CARTE, tel qu'un avenant le retire (`stock::Avenant::retires`) ; il s'ecrit par son nom de cle.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Champ { $( $variante, )* }

        impl Champ {
            /// Tous les champs, dans l'ordre de la carte.
            pub const TOUS: &'static [Champ] = &[ $( Champ::$variante, )* ];

            /// Son nom de cle.
            pub fn nom(self) -> &'static str {
                match self { $( Champ::$variante => stringify!($champ), )* }
            }

            /// Son groupe.
            pub fn groupe(self) -> Groupe {
                match self { $( Champ::$variante => Groupe::$groupe, )* }
            }
        }

        impl $carte {
            /// Le champ `c` de `de` pose sur la carte, tel quel (dit ou non).
            pub fn prendre(&mut self, de: &$carte, c: Champ) {
                match c { $( Champ::$variante => self.$champ = de.$champ.clone(), )* }
            }

            /// Le champ `c` ote.
            pub fn ote(&mut self, c: Champ) {
                match c { $( Champ::$variante => self.$champ = None, )* }
            }

            /// Le champ `c` est-il dit ?
            pub fn dit(&self, c: Champ) -> bool {
                match c { $( Champ::$variante => self.$champ.is_some(), )* }
            }

            /// Le champ `c` tel qu'il se compare (`socle::ecriture::comparable`).
            pub fn comparable_du(&self, c: Champ) -> String {
                match c { $( Champ::$variante => socle::ecriture::comparable(&self.$champ), )* }
            }

            /// La valeur du champ `c` quand elle est un texte, un nombre ou un oui-non (un terme, une clause) ; `None`
            /// sinon, ou s'il n'est pas dit.
            pub fn valeur_du(&self, c: Champ) -> Option<Valeur> {
                match c { $( Champ::$variante => self.$champ.as_ref().and_then(Simple::simple), )* }
            }
        }
    };
}

carte! {
    /// UNE CARTE. Absent et `null` se confondent : rien n'est dit. Chaque champ dit son groupe et son nom de champ
    /// (`#[champ(Groupe, Champ)]`).
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Carte {
        #[champ(Partage, Salarie)]
        pub salarie: Option<droit::contexte::IdSalarie>,
        #[champ(Terme, Spectacle)]
        pub spectacle: Option<droit::contexte::IdSpectacle>,
        #[champ(Terme, Nature)]
        pub nature: Option<droit::natures::NatureDeContrat>,
        #[champ(Partage, Debut)]
        pub debut: Option<Jour>,
        #[champ(Partage, Fin)]
        pub fin: Option<Jour>,
        #[champ(Partage, Numero)]
        pub numero: Option<crate::numeros::NumeroContrat>,
        #[champ(Terme, Etablissement)]
        pub etablissement: Option<String>,
        #[champ(Terme, Metier)]
        pub metier: Option<String>,
        #[champ(Terme, Cadre)]
        pub cadre: Option<bool>,
        #[champ(Terme, Lieu)]
        pub lieu: Option<String>,
        #[champ(Terme, MotifRecours)]
        pub motif_recours: Option<droit::natures::MotifDeRecours>,
        #[champ(Terme, QuotiteContrat)]
        pub quotite_contrat: Option<NombreLu>,
        #[champ(Terme, QuotiteReference)]
        pub quotite_reference: Option<NombreLu>,
        #[champ(Terme, ModaliteTempsTravail)]
        pub modalite_temps_travail: Option<droit::natures::ModaliteDuTempsDeTravail>,
        #[champ(Terme, SalaireMensuel)]
        pub salaire_mensuel: Option<NombreLu>,
        #[champ(Terme, Emploi)]
        pub emploi: Option<String>,
        #[champ(Terme, PcsEse)]
        pub pcs_ese: Option<String>,
        #[champ(Terme, CodeEmploiFt)]
        pub code_emploi_ft: Option<String>,
        #[champ(Terme, Statut)]
        pub statut: Option<droit::fiche::Statut>,
        #[champ(Terme, StatutConventionnel)]
        pub statut_conventionnel: Option<droit::fiche::StatutConventionnel>,
        #[champ(Terme, StatutCategorielRc)]
        pub statut_categoriel_rc: Option<droit::fiche::StatutCategorielRc>,
        #[champ(Terme, PeriodeEssaiJours)]
        pub periode_essai_jours: Option<NombreLu>,
        #[champ(Terme, PersonneRemplacee)]
        pub personne_remplacee: Option<String>,
        #[champ(Terme, RepartitionHoraire)]
        pub repartition_horaire: Option<String>,
        #[champ(Terme, ClausesParticulieres)]
        pub clauses_particulieres: Option<String>,
        #[champ(Terme, ModeleContrat)]
        pub modele_contrat: Option<String>,
        #[champ(Fait, Lignes)]
        /// Les lignes de paie de la carte (`ligne::LignePaie`).
        pub lignes: Option<Vec<LignePaie>>,
        #[champ(Fait, CongesPris)]
        pub conges_pris: Option<Vec<CongePris>>,
        #[champ(Fait, Primes)]
        pub primes: Option<Vec<Prime>>,
        #[champ(Fait, DatePaiement)]
        pub date_paiement: Option<Jour>,
        #[champ(Fait, Jours)]
        pub jours: Option<NombreLu>,
        #[champ(Fait, TauxCachet)]
        pub taux_cachet: Option<NombreLu>,
        #[champ(Fait, HeuresSupplementaires)]
        pub heures_supplementaires: Option<NombreLu>,
        #[champ(Fait, HeuresSupplementairesA50)]
        pub heures_supplementaires_a_50: Option<NombreLu>,
        #[champ(Fait, HeuresComplementaires)]
        pub heures_complementaires: Option<NombreLu>,
        #[champ(Fait, IndemniteLicenciement)]
        pub indemnite_licenciement: Option<NombreLu>,
        #[champ(Fait, FauteGrave)]
        pub faute_grave: Option<bool>,
        #[champ(Fin, MotifRupture)]
        pub motif_rupture: Option<droit::natures::MotifDeRupture>,
        #[champ(Fin, NotificationRupture)]
        pub notification_rupture: Option<Jour>,
        #[champ(Fin, EngagementLicenciement)]
        pub engagement_licenciement: Option<Jour>,
        #[champ(Fin, SignatureConvention)]
        pub signature_convention: Option<Jour>,
        #[champ(Fin, Preavis)]
        pub preavis: Option<Preavis>,
        #[champ(Fin, Transaction)]
        pub transaction: Option<String>,
        #[champ(Fin, DernierJourPaye)]
        pub dernier_jour_paye: Option<Jour>,
        #[champ(Fin, EnDroitALaRetraite)]
        pub en_droit_a_la_retraite: Option<bool>,
        #[champ(Fin, PreavisNonEffectue)]
        pub preavis_non_effectue: Option<bool>,
        #[champ(Fin, PreavisMois)]
        pub preavis_mois: Option<NombreLu>,
        #[champ(Fin, PrecariteDue)]
        pub precarite_due: Option<bool>,
        #[champ(Fin, PrecaritePourquoi)]
        pub precarite_pourquoi: Option<String>,
        #[champ(Deduit, JoursContrat)]
        pub jours_contrat: Option<NombreLu>,
        #[champ(Deduit, DateModification)]
        pub date_modification: Option<Jour>,
        #[champ(Deduit, Avant)]
        pub avant: Option<Avant>,
        #[champ(Vue, Valide)]
        /// Dans la vue : l'engagement est valide, et depuis quand.
        pub valide: Option<Validee>,
        #[champ(Vue, Importe)]
        /// La paie d'un bulletin de l'ancien logiciel, recalculee pour s'y comparer (`ecarts`) : ce qu'il a verse est un fait,
        /// rien ne s'y juge (le jour du versement, les unites du statut).
        pub importe: Option<bool>,
    }
}

impl Champ {
    /// Les champs des groupes `g`, dans l'ordre de la carte.
    pub fn des(g: &[Groupe]) -> impl Iterator<Item = Champ> + '_ {
        Champ::TOUS.iter().copied().filter(move |c| g.contains(&c.groupe()))
    }
}

/// LA VALEUR SIMPLE D'UN CHAMP (`Carte::valeur_du`) : un texte, un nombre, un oui-non ; elle s'ecrit telle quelle.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Valeur {
    Nature(droit::natures::NatureDeContrat),
    Texte(String),
    Nombre(NombreLu),
    Oui(bool),
}

/// Ce qu'un champ a de simple (`Valeur`).
trait Simple {
    fn simple(&self) -> Option<Valeur>;
}

impl Simple for String {
    fn simple(&self) -> Option<Valeur> {
        Some(Valeur::Texte(self.clone()))
    }
}

impl Simple for Jour {
    fn simple(&self) -> Option<Valeur> {
        Some(Valeur::Texte(self.iso()))
    }
}

impl Simple for droit::contexte::IdSalarie {
    fn simple(&self) -> Option<Valeur> {
        Some(Valeur::Texte(self.to_string()))
    }
}

impl Simple for droit::contexte::IdSpectacle {
    fn simple(&self) -> Option<Valeur> {
        Some(Valeur::Texte(self.to_string()))
    }
}

impl Simple for droit::natures::NatureDeContrat {
    fn simple(&self) -> Option<Valeur> {
        Some(Valeur::Nature(*self))
    }
}

macro_rules! choix_simple {
    ($($t:ty),*) => { $( impl Simple for $t { fn simple(&self) -> Option<Valeur> { Some(Valeur::Texte(self.as_str().to_string())) } } )* };
}
choix_simple!(droit::fiche::Statut, droit::fiche::StatutConventionnel, droit::fiche::StatutCategorielRc, crate::numeros::NumeroContrat);

macro_rules! code_simple {
    ($($t:ty),*) => { $( impl Simple for $t { fn simple(&self) -> Option<Valeur> { Some(Valeur::Texte(self.code().to_string())) } } )* };
}
code_simple!(droit::natures::MotifDeRecours, droit::natures::MotifDeRupture, droit::natures::ModaliteDuTempsDeTravail);

impl Simple for NombreLu {
    fn simple(&self) -> Option<Valeur> {
        Some(Valeur::Nombre(self.clone()))
    }
}

impl Simple for bool {
    fn simple(&self) -> Option<Valeur> {
        Some(Valeur::Oui(*self))
    }
}

macro_rules! rien_de_simple {
    ($($t:ty),*) => { $( impl Simple for $t { fn simple(&self) -> Option<Valeur> { None } } )* };
}
rien_de_simple!(Vec<LignePaie>, Vec<CongePris>, Vec<Prime>, Preavis, Avant, Validee);

impl Serialize for Champ {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nom())
    }
}

impl<'de> Deserialize<'de> for Champ {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Champ, D::Error> {
        let nom = String::deserialize(d)?;
        Champ::TOUS.iter().copied().find(|c| c.nom() == nom).ok_or_else(|| serde::de::Error::custom(format!("« {} » n'est pas un champ d'une carte", nom)))
    }
}

impl Carte {
    /// La carte des seuls champs des groupes `g`.
    pub fn du_groupe(&self, g: &[Groupe]) -> Carte {
        let mut c = Carte::default();
        for x in Champ::des(g) {
            c.prendre(self, x);
        }
        c
    }

    /// La carte sans les champs des groupes `g`.
    pub fn sans(&self, g: &[Groupe]) -> Carte {
        let mut c = self.clone();
        for x in Champ::des(g) {
            c.ote(x);
        }
        c
    }

    /// Chaque champ que `de` dit se pose sur la carte ; les autres restent ce qu'ils sont.
    pub fn recouvrir(&mut self, de: &Carte) {
        for x in Champ::TOUS.iter().copied().filter(|x| de.dit(*x)) {
            self.prendre(de, x);
        }
    }

    /// Une carte qui ne dit rien (une liste vide ne dit rien : `socle::ecriture::comparable`) ?
    pub fn muette(&self) -> bool {
        socle::ecriture::comparable(self) == socle::ecriture::comparable(&Carte::default())
    }

    /// Les lignes de la carte (aucune si elle n'en dit pas).
    pub fn lignes(&self) -> &[LignePaie] {
        match &self.lignes {
            Some(l) => l,
            None => &[],
        }
    }

    /// Le debut, comme `Engagement` le deduirait : celui qu'elle dit, sinon son premier jour joue ; `None` sans aucun.
    pub fn debut_lu(&self) -> Option<Jour> {
        crate::debut_de(self.debut, self.lignes())
    }

    /// Les jours joues de ses lignes de travail (`crate::jours_des_lignes`).
    pub fn dates(&self) -> Vec<DateDeLigne> {
        crate::jours_des_lignes(self.lignes(), false)
    }

    /// Les jours que ses lignes de travail mettent au planning.
    pub fn planning(&self) -> Vec<DateDeLigne> {
        crate::jours_des_lignes(self.lignes(), true)
    }

    /// Les jours du contrat entier que la carte connait.
    pub fn jours_du_contrat(&self) -> usize {
        crate::jours_de_lignes(self.lignes())
    }

    /// La nature de son contrat ; une carte qui ne la dit pas se refuse (`crate::PAS_DE_NATURE`).
    pub fn nature_lue(&self) -> R<std::sync::Arc<droit::natures::Nature>> {
        match self.nature {
            Some(n) => n.proprietes(),
            None => Err(match &self.salarie {
                Some(s) => crate::a_finir(format!("{}{}", s, crate::PAS_DE_NATURE)),
                None => crate::a_finir(format!("Cet engagement{}", crate::PAS_DE_NATURE)),
            }),
        }
    }

    /// Valide, dans la vue de son mois (un engagement que le calcul compte seulement comme valide ne l'est pas).
    pub fn est_validee(&self) -> bool {
        self.valide.as_ref().is_some_and(|v| v.le.is_some())
    }

    /// Un contrat paye au mois ?
    pub fn mensuelle(&self) -> R<bool> {
        Ok(self.nature_lue()?.mensuelle())
    }
}

/// Des conges pris dans le mois, d'un jour a l'autre (AAAA-MM-JJ). La page envoie une ligne commencee telle quelle, le
/// jour qu'elle ne dit pas encore `null` : le calcul le refuse en le nommant (`CongePris::jours`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CongePris {
    pub debut: Option<Jour>,
    pub fin: Option<Jour>,
}

impl CongePris {
    /// Son premier et son dernier jour, lus ; un jour qui n'est pas dit se refuse en se nommant.
    pub fn jours(&self) -> R<(Jour, Jour)> {
        let lu = |x: Option<Jour>, cle: &str| x.ok_or_else(|| socle::echec::cle("Des congés pris disent leur premier et leur dernier jour.", cle));
        Ok((lu(self.debut, "debut")?, lu(self.fin, "fin")?))
    }
}

/// Une prime du mois : son montant, sa rubrique, son libelle (`null` : celui de sa rubrique).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prime {
    pub montant: NombreLu,
    /// Sa rubrique au catalogue, que la carte choisit ; `null` tant qu'elle ne la dit pas (le calcul la demande).
    pub rubrique: Option<droit::catalogue::CodeDeRubrique>,
    pub libelle: Option<String>,
}

/// Le preavis de la fin du contrat : son genre (`droit::preavis`), et ses jours quand ils sont dits (`null` sinon).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preavis {
    #[serde(rename = "type")]
    pub genre: String,
    pub debut: Option<Jour>,
    pub fin: Option<Jour>,
}

/// Les termes d'avant l'avenant qui change en cours de mois (`droit::periodes::TERMES_D_AVANT`), chacun `null` quand il
/// ne change pas.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Avant {
    pub salaire_mensuel: Option<NombreLu>,
    pub quotite_contrat: Option<NombreLu>,
}

/// Un engagement valide, dans la vue de son mois : le jour de sa validation (`null` : compte comme valide par le calcul
/// qui l'ecrit, `etats::calculer_les_rangs`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Validee {
    #[serde(with = "socle::paris::ecrit::seconde::option")]
    pub le: Option<socle::paris::Moment>,
}

/// Ce que la saisie d'un mois pose pour tous ses engagements (le cachet par defaut), tel que la paie ecrite le garde
/// (`etats::MOIS_LU`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CeQueLeMoisDit {
    pub taux_cachet: Option<NombreLu>,
}

/// LE MOIS DE L'ECRAN (`stock::vue_du_mois`) : le mois, le cachet qu'il dit par defaut, et la vue de chaque engagement qui
/// le touche, dans l'ordre de l'ecran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VueDuMois {
    pub mois: socle::mois::Mois,
    pub taux_cachet: Option<NombreLu>,
    pub engagements: Vec<Carte>,
}

impl VueDuMois {
    /// Un mois sans engagement.
    pub fn vide(mois: socle::mois::Mois) -> VueDuMois {
        VueDuMois { mois, taux_cachet: None, engagements: Vec::new() }
    }
}
