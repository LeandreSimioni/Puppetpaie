//! Les minima conventionnels : la table livree (`minima-conventionnels.json`, une grille par convention), lue par sa
//! forme (`Table`, la seule lecture : `table`), ses paliers, les accords de salaires qui la suivent et depuis quand ils
//! lient une compagnie, et ce qu'un minimum est. Les confronter a une paie, et l'avertir, est l'affaire de l'engagement
//! (`engagement::minima`).

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use serde::Deserialize;

use crate::fiche::Lue;
use socle::date::Jour;
use socle::fichiers::{forme_en_cache, FormeGardee};
use socle::forme::{Court, Nombre};
use socle::echec::R;
use socle::num::Decimal;

use crate::forme_bareme::OrganisationPatronale;

use crate::source::forme_notee;

// ------------------------------------------------------------ la forme du fichier

forme_notee! {
    /// `donnees/minima-conventionnels.json` : une grille par convention, par son IDCC.
    pub struct Table {
        pub conventions: BTreeMap<crate::fiche::Idcc, Convention>,
    }
}

forme_notee! {
    /// UNE CONVENTION, OU UN ACCORD DE SALAIRES QUI SUIT SA GRILLE (`accords`, de la meme forme) : le texte qui la donne
    /// (`texte`, sa premiere partie dans la phrase d'un avertissement), le lien vers la convention, et ses grilles. Chaque
    /// convention n'en porte qu'une partie : la 3090 par annexe (`annexes`, `clauses_communes`), la 1285 ses cachets, son
    /// service de repetition, ses emplois non artistiques et ses accords, la 2642 et la 3097 leurs tables propres, toutes
    /// des grilles (`grilles`, `grilles_complementaires`).
    pub struct Convention {
        /// L'accord, tel qu'une valeur du bareme le nomme (`accord`).
        pub id: Option<AccordDeSalaires>,
        pub libelle: Option<String>,
        pub texte: Option<String>,
        pub lien: Option<socle::garde_reseau::Url>,
        pub en_vigueur: Option<Jour>,
        pub etendu: Option<bool>,
        /// Le jour ou l'extension de l'accord prend effet (sa publication au JORF).
        pub etendu_le: Option<Jour>,
        /// Les organisations d'employeurs signataires de l'accord, ecrites par leur cle du bareme
        /// (`par_organisation_patronale`) ; le nom que l'accord leur donne est dans sa source.
        pub signataires: Option<Vec<OrganisationPatronale>>,
        pub annexes: Option<BTreeMap<crate::fiche::AnnexeConvention, Annexe>>,
        pub clauses_communes: Option<ClausesCommunes>,
        pub cachets: Option<BTreeMap<CleDeGrille, GrilleDeCachets>>,
        pub repetition: Option<RepetitionParService>,
        pub emplois_autres_qu_artistiques: Option<EmploisAutres>,
        pub accords: Option<Vec<Convention>>,
        pub grilles: Option<Vec<Grille>>,
        pub grilles_complementaires: Option<Vec<Grille>>,
        /// 2642 : le minimum d'un CDDU a la semaine de 35 h, par fonction.
        pub cddu_semaine_35h: Option<BTreeMap<CleDeGrille, Cddu2642>>,
        /// 2642 : le mensuel pour 35 h d'un permanent, par fonction.
        pub permanents_mensuel_35h: Option<BTreeMap<CleDeGrille, Nombre>>,
        /// 3097 : la semaine de 39 h d'un technicien, par fonction.
        pub techniciens_semaine_39h: Option<BTreeMap<CleDeGrille, Nombre>>,
        /// 3097 : les heures que la semaine de 39 h paie au taux de base (35 h + 4 h a 125 %).
        pub heures_payees_de_la_semaine: Option<Nombre>,
        pub avant_2026_05: Option<Avant2026>,
        pub annexe_iii: Option<AnnexeIii>,
        pub artistes: Option<Artistes3097>,
        pub complement: Option<Complement3097>,
        pub permanents_mensuel: Option<Permanents3097>,
    }
}

forme_notee! {
    /// Une annexe de la 3090 : ses cachets d'artistes, sa repetition, le taux horaire de ses techniciens.
    pub struct Annexe {
        pub libelle: String,
        pub cachets: Option<BTreeMap<CleDeGrille, GrilleDeCachets>>,
        pub repetition: Option<RepetitionDeLAnnexe>,
        pub techniciens: Option<ParNiveau<Nombre>>,
    }
}

forme_notee! {
    /// Les clauses communes de la 3090 : le taux horaire des techniciens, la grille mensuelle des administratifs.
    pub struct ClausesCommunes {
        pub techniciens: ParNiveau<Nombre>,
        pub administratifs_mensuel_echelons: ParNiveau<Vec<Court>>,
    }
}

/// UNE VALEUR PAR NIVEAU DE LA 3090 (`classification_3090.niveau` d'un metier), et ses notes.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T: Deserialize<'de>"))]
pub struct ParNiveau<T> {
    pub cadre_groupe_1: Option<T>,
    pub cadre_groupe_2: Option<T>,
    pub cadre_groupe_3: Option<T>,
    pub agent_de_maitrise: Option<T>,
    pub employe_qualifie_1: Option<T>,
    pub employe_qualifie_2: Option<T>,
    pub employe: Option<T>,
    pub quoi: Option<String>,
    pub note: Option<String>,
    pub source: Option<crate::source::Fondement>,
}

impl<T> ParNiveau<T> {
    /// La valeur du niveau `niveau`, si la grille en a une.
    pub fn du_niveau(&self, niveau: &CleDeGrille) -> Option<&T> {
        const NIVEAUX: [CleDeGrille; 7] = [
            CleDeGrille::du_moteur("cadre_groupe_1"),
            CleDeGrille::du_moteur("cadre_groupe_2"),
            CleDeGrille::du_moteur("cadre_groupe_3"),
            CleDeGrille::du_moteur("agent_de_maitrise"),
            CleDeGrille::du_moteur("employe_qualifie_1"),
            CleDeGrille::du_moteur("employe_qualifie_2"),
            CleDeGrille::du_moteur("employe"),
        ];
        let valeurs = [
            &self.cadre_groupe_1,
            &self.cadre_groupe_2,
            &self.cadre_groupe_3,
            &self.agent_de_maitrise,
            &self.employe_qualifie_1,
            &self.employe_qualifie_2,
            &self.employe,
        ];
        NIVEAUX.iter().zip(valeurs).find(|(n, _)| *n == niveau).and_then(|(_, v)| v.as_ref())
    }
}

forme_notee! {
    /// La grille des cachets d'un emploi : ses paliers sur le nombre de representations, et sa jauge quand elle en depend.
    pub struct GrilleDeCachets {
        pub libelle: String,
        pub periode: Option<CleDeGrille>,
        pub jauge: Option<CleDeGrille>,
        pub paliers: Vec<Palier>,
        pub autres_jauges: Option<BTreeMap<CleDeGrille, Vec<Nombre>>>,
    }
}

/// Un palier : le montant jusqu'a `jusqu_a` representations (`None` : au-dela).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palier {
    pub jusqu_a: Option<i64>,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub montant: Decimal,
}

forme_notee! {
    /// La repetition d'une annexe de la 3090 : le cachet de repetition, le taux horaire, le service d'un musicien, le
    /// service isole.
    pub struct RepetitionDeLAnnexe {
        pub cachet: Option<Nombre>,
        pub cachet_libelle: Option<String>,
        pub heure: Option<Nombre>,
        pub service_musicien: Option<Nombre>,
        pub service_isole: Option<Nombre>,
    }
}

forme_notee! {
    /// Le service de repetition de la 1285, et celui de chaque grille d'artiste qui en a un propre.
    pub struct RepetitionParService {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub service: Decimal,
        pub service_libelle: String,
        /// Par ligne de minima d'un metier de la 1285 (`emplois-base.json`, `minima.1285`).
        pub par_categorie: Option<BTreeMap<CleDeGrille, ServiceDeCategorie>>,
    }
}

/// Le service de repetition propre a une grille d'artiste.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceDeCategorie {
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub service: Decimal,
    pub libelle: String,
}

forme_notee! {
    /// Les emplois autres qu'artistiques de la 1285 : le mensuel de chaque echelon, par groupe, pour `heures_mensuelles`.
    pub struct EmploisAutres {
        #[serde(deserialize_with = "socle::forme::court")]
        pub heures_mensuelles: Decimal,
        pub groupes: BTreeMap<CleDeGrille, Vec<Court>>,
    }
}

/// Une fonction de la 2642 : sa filiere, son niveau, le minimum de la semaine par genre de programme.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cddu2642 {
    pub filiere: CleDeGrille,
    pub niveau: CleDeGrille,
    pub fiction: Option<Nombre>,
    pub flux: Option<Nombre>,
    pub autre: Option<Nombre>,
}

impl Cddu2642 {
    /// Les genres que la fonction connait, dans l'ordre fiction, flux, autre, avec leur minimum.
    pub fn genres(&self) -> Vec<(&'static str, Decimal)> {
        [("fiction", self.fiction), ("flux", self.flux), ("autre", self.autre)].into_iter().filter_map(|(g, m)| m.map(|m| (g, m.0))).collect()
    }
}

forme_notee! {
    /// 3097 : les deux fonctions reevaluees au 01/05/2026, a leur montant d'avant.
    pub struct Avant2026 {
        pub montants: BTreeMap<CleDeGrille, Nombre>,
    }
}

forme_notee! {
    /// 3097, titre II, annexe III : le socle de la semaine et la part payee au-dela.
    pub struct AnnexeIii {
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub socle: Decimal,
        #[serde(deserialize_with = "socle::forme::decimal")]
        pub part_au_dela: Decimal,
        pub depuis: Jour,
    }
}

forme_notee! {
    /// 3097, titre III : les artistes-interpretes d'un long et d'un court-metrage.
    pub struct Artistes3097 {
        pub long: Engagement3097,
        pub court: Engagement3097,
    }
}

/// Un engagement d'artiste de la 3097 : la journee, la semaine de 5 et de 6 jours, et les repetitions d'un
/// long-metrage.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Engagement3097 {
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub journee: Decimal,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub semaine_5j: Decimal,
    #[serde(deserialize_with = "socle::forme::decimal")]
    pub semaine_6j: Decimal,
    pub repetition_service_4h: Option<Nombre>,
    pub repetition_2_services_4h: Option<Nombre>,
    pub repetition_service_3h_danse_chant_musique: Option<Nombre>,
    pub repetition_2_services_3h_danse_chant_musique: Option<Nombre>,
}

forme_notee! {
    /// 3097, titre III, sous-titre II : les acteurs de complement d'un long-metrage, par role.
    pub struct Complement3097 {
        pub figuration: Engagement3097,
        pub silhouette_muette: Engagement3097,
        pub silhouette_parlante: Engagement3097,
        pub doublure_simple: Engagement3097,
        pub doublure_polyvalente: Engagement3097,
    }
}

impl Complement3097 {
    /// Le role `role` (la ligne de minima du metier dans la 3097), s'il en est un.
    pub fn du_role(&self, role: &CleDeGrille) -> Option<&Engagement3097> {
        const ROLES: [CleDeGrille; 5] = [
            CleDeGrille::du_moteur("figuration"),
            CleDeGrille::du_moteur("silhouette_muette"),
            CleDeGrille::du_moteur("silhouette_parlante"),
            CleDeGrille::du_moteur("doublure_simple"),
            CleDeGrille::du_moteur("doublure_polyvalente"),
        ];
        let engagements = [&self.figuration, &self.silhouette_muette, &self.silhouette_parlante, &self.doublure_simple, &self.doublure_polyvalente];
        ROLES.iter().zip(engagements).find(|(r, _)| *r == role).map(|(_, e)| e)
    }
}

forme_notee! {
    /// 3097, titre IV : le mensuel de base des permanents par niveau, et le niveau de chaque emploi repere.
    pub struct Permanents3097 {
        pub niveaux: BTreeMap<CleDeGrille, Nombre>,
        pub emplois_reperes: BTreeMap<CleDeGrille, CleDeGrille>,
    }
}

forme_notee! {
    /// UNE GRILLE D'UNE CONVENTION : qui elle vise (population, statut, unite de paie, categories du metier, codes,
    /// conditions sur le spectacle, l'entreprise et le salarie, anciennete, duree du contrat), ses valeurs par classement
    /// (dans l'ordre du texte), ses majorations, et ce qui la ramene a l'heure (`heures`, `facteur`) ou l'abat.
    pub struct Grille {
        pub nom: String,
        /// La population que la grille vise ; absente, toutes.
        #[serde(default, deserialize_with = "crate::natures::population::option::deserialize")]
        pub population: Option<crate::natures::Paie>,
        /// Le statut que la grille vise ; absent, tous.
        pub statut: Option<crate::fiche::Statut>,
        /// Ce que la valeur de la grille paie (le cachet, l'heure, le jour, la semaine, le mois) : l'unite d'une rubrique,
        /// la meme notion (`catalogue::Unite`). Une unite hors de sa liste refuse le fichier a la lecture, au chemin de la
        /// grille.
        pub unite: crate::catalogue::Unite,
        pub heures: Option<Nombre>,
        /// Les lignes de minima des metiers qu'elle vise (`emplois-base.json`, `minima.<idcc>`).
        pub categories: Option<Vec<CleDeGrille>>,
        pub codes: Option<Vec<crate::emplois::CodeEmploi>>,
        pub si_spectacle: Option<SiSpectacle>,
        pub si_entreprise: Option<SiEntreprise>,
        pub si_salarie: Option<SiSalarie>,
        pub anciennete: Option<Anciennete>,
        pub duree_contrat_max_jours: Option<i64>,
        /// Les grilles qu'elle remplace, par leur `nom`.
        pub remplace: Option<Vec<String>>,
        #[serde(deserialize_with = "socle::forme::en_ordre")]
        pub valeurs: Vec<(String, Nombre)>,
        pub majorations: Option<Vec<Majoration>>,
        pub abattement_max: Option<Nombre>,
        pub abattement_motif: Option<String>,
        pub facteur: Option<Nombre>,
        pub facteur_motif: Option<String>,
    }
}

forme_notee! {
    /// Une majoration d'une grille : ses conditions, son groupe (seule la derniere d'un groupe compte), et ce qu'elle
    /// ajoute a chaque valeur.
    pub struct Majoration {
        pub nom: String,
        pub groupe: Option<CleDeGrille>,
        pub categories: Option<Vec<CleDeGrille>>,
        pub codes: Option<Vec<crate::emplois::CodeEmploi>>,
        pub si_spectacle: Option<SiSpectacle>,
        pub si_entreprise: Option<SiEntreprise>,
        pub si_salarie: Option<SiSalarie>,
        pub anciennete: Option<Anciennete>,
        pub duree_contrat_max_jours: Option<i64>,
        #[serde(deserialize_with = "socle::forme::en_ordre")]
        pub par_cle: Vec<(String, Nombre)>,
    }
}

/// L'anciennete qu'une grille ou une majoration demande : `ans` revolus depuis la case `champ` de la fiche du salarie.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anciennete {
    pub champ: ChampDAnciennete,
    pub ans: i64,
}

/// LA CASE D'OU SE COMPTE UNE ANCIENNETE (`anciennete.champ`) : une date de la fiche du salarie, qu'une grille nomme.
/// Une autre se refuse a la lecture de la table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChampDAnciennete {
    /// Le poste occupe depuis (`ChampsSalarie::metier`, `poste_depuis`).
    PosteDepuis,
}

impl ChampDAnciennete {
    /// La case de la fiche du salarie.
    pub fn cle(self) -> crate::fiche::CasePieceSalarie {
        match self {
            ChampDAnciennete::PosteDepuis => crate::fiche::CasePieceSalarie::PosteDepuis,
        }
    }

    /// Sa date dans les cases du metier de la fiche ; `None` : la fiche ne la dit pas.
    pub fn dans(self, metier: &crate::fiche::CasesDuMetier) -> Option<Jour> {
        match self {
            ChampDAnciennete::PosteDepuis => metier.poste_depuis,
        }
    }
}

/// La case attendue `attendu` est-elle celle de la fiche ? Aucune attente : oui ; une case vide ne remplit aucune
/// attente.
fn vaut<T: PartialEq>(attendu: &Option<T>, case: &Option<T>) -> bool {
    attendu.as_ref().is_none_or(|a| case.as_ref() == Some(a))
}

/// CE QU'UNE CONDITION DE GRILLE ATTEND DU SPECTACLE DE LA PAIE (`si_spectacle`) : chaque case qu'elle nomme, dans la
/// forme de la fiche (`droit::fiche::ChampsSpectacle`) ; une case qu'elle ne declare pas se refuse a la lecture.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiSpectacle {
    pub genre_audiovisuel: Option<crate::fiche::GenreAudiovisuel>,
    pub derogation_annexe_iii_3097: Option<bool>,
}

impl SiSpectacle {
    /// Le spectacle `sp` remplit-il la condition ? Une paie sans spectacle ne remplit aucune condition sur lui.
    pub fn remplie(&self, sp: Option<&crate::fiche::ChampsSpectacle>) -> bool {
        match sp {
            Some(sp) => vaut(&self.genre_audiovisuel, &sp.genre_audiovisuel) && vaut(&self.derogation_annexe_iii_3097, &sp.derogation_annexe_iii_3097),
            None => self.genre_audiovisuel.is_none() && self.derogation_annexe_iii_3097.is_none(),
        }
    }
}

/// CE QU'UNE CONDITION DE GRILLE ATTEND DE LA FICHE ENTREPRISE (`si_entreprise`), dans sa forme
/// (`droit::fiche::ChampsEntreprise`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiEntreprise {
    pub type_radio_1922: Option<crate::fiche::TypeRadio1922>,
    pub cinquante_salaries_3241: Option<bool>,
}

impl SiEntreprise {
    pub fn remplie(&self, ent: &crate::fiche::ChampsEntreprise) -> bool {
        vaut(&self.type_radio_1922, &ent.type_radio_1922) && vaut(&self.cinquante_salaries_3241, &ent.cinquante_salaries_3241)
    }
}

/// CE QU'UNE CONDITION DE GRILLE ATTEND DE LA FICHE DU SALARIE (`si_salarie`), dans sa forme : des cases du metier
/// (`ChampsSalarie::metier`), et le forfait en jours, case d'un permanent (`ChampsSalarie::permanent`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiSalarie {
    pub stagiaire_ccneac: Option<crate::fiche::StagiaireCcneac>,
    pub experimente_3241: Option<bool>,
    pub forfait_jours: Option<bool>,
}

impl SiSalarie {
    /// Les cases du metier `metier` (que la grille trouvee ouvre) et celles d'un permanent `permanent` (`None` : le
    /// contrat n'est pas paye au mois, elles ne disent rien) remplissent-elles la condition ?
    pub fn remplie(&self, metier: &crate::fiche::CasesDuMetier, permanent: Option<&crate::fiche::CasesDuPermanent>) -> bool {
        vaut(&self.stagiaire_ccneac, &metier.stagiaire_ccneac)
            && vaut(&self.experimente_3241, &metier.experimente_3241)
            && match permanent {
                Some(p) => vaut(&self.forfait_jours, &p.forfait_jours),
                None => self.forfait_jours.is_none(),
            }
    }

    /// Les cases de la fiche du salarie qu'elle nomme.
    pub fn cases(&self) -> Vec<crate::fiche::CasePieceSalarie> {
        use crate::fiche::CasePieceSalarie;
        [
            self.stagiaire_ccneac.is_some().then_some(CasePieceSalarie::StagiaireCcneac),
            self.experimente_3241.is_some().then_some(CasePieceSalarie::Experimente3241),
            self.forfait_jours.is_some().then_some(CasePieceSalarie::ForfaitJours),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

/// LES CONDITIONS D'UNE GRILLE OU D'UNE MAJORATION, telles que l'engagement les confronte a une ligne de paie.
pub struct Conditions<'a> {
    pub categories: Option<&'a [CleDeGrille]>,
    pub codes: Option<&'a [crate::emplois::CodeEmploi]>,
    pub si_spectacle: Option<&'a SiSpectacle>,
    pub si_entreprise: Option<&'a SiEntreprise>,
    pub si_salarie: Option<&'a SiSalarie>,
    pub anciennete: Option<&'a Anciennete>,
    pub duree_contrat_max_jours: Option<i64>,
}

impl Grille {
    pub fn conditions(&self) -> Conditions<'_> {
        Conditions {
            categories: self.categories.as_deref(),
            codes: self.codes.as_deref(),
            si_spectacle: self.si_spectacle.as_ref(),
            si_entreprise: self.si_entreprise.as_ref(),
            si_salarie: self.si_salarie.as_ref(),
            anciennete: self.anciennete.as_ref(),
            duree_contrat_max_jours: self.duree_contrat_max_jours,
        }
    }
}

impl Majoration {
    pub fn conditions(&self) -> Conditions<'_> {
        Conditions {
            categories: self.categories.as_deref(),
            codes: self.codes.as_deref(),
            si_spectacle: self.si_spectacle.as_ref(),
            si_entreprise: self.si_entreprise.as_ref(),
            si_salarie: self.si_salarie.as_ref(),
            anciennete: self.anciennete.as_ref(),
            duree_contrat_max_jours: self.duree_contrat_max_jours,
        }
    }
}

const ILLISIBLE: &str = "Les minima conventionnels livrés avec le logiciel ne se lisent pas : le signaler au développeur.";

/// LA SEULE LECTURE DE `minima-conventionnels.json` : sa forme.
pub fn table() -> R<Arc<Table>> {
    static CACHE: OnceLock<FormeGardee<Table>> = OnceLock::new();
    forme_en_cache(&CACHE, "minima-conventionnels.json", ILLISIBLE, Ok)
}

// ------------------------------------------------------------ les lectures

/// Ce que l'IDCC d'un contrat lit du spectacle : s'il est une emission de television de la 1734 (`emission_tv_1734`),
/// et sa clé (le message de refus le nomme par elle).
pub struct SpectacleDuContrat<'a> {
    pub emission_tv_1734: bool,
    pub cle: &'a crate::contexte::IdSpectacle,
}

/// L'IDCC d'un contrat : celui de la compagnie (`idcc`), sauf l'artiste-interprète d'une émission de télévision
/// (`emission_tv_1734` du spectacle) d'une compagnie de la 2642 ou de la 3241, qui relève de la 1734
/// (`minima-conventionnels.json`, la note de la 1734). Le contrat le déclare (S21.G00.40.017), le bulletin l'imprime, le
/// minimum lit sa grille. Artiste-interprète se lit au métier du catalogue (familles « Artistes », « Cirque et rue ») :
/// un métier hors catalogue ne dit pas s'il l'est, l'IDCC se refuse (règle 1).
pub fn idcc_du_contrat(idcc: &crate::fiche::Idcc, sp: &SpectacleDuContrat, sal: &crate::fiche::ChampsSalarie) -> R<crate::fiche::Idcc> {
    let idcc = idcc.clone();
    if (idcc != crate::fiche::IDCC_2642 && idcc != crate::fiche::IDCC_3241) || !sp.emission_tv_1734 || sal.statut.dite().is_some_and(|s| *s != crate::fiche::Statut::Artiste) {
        return Ok(idcc);
    }
    let code = sal.code_emploi_ft.dite();
    let metier = match code {
        Some(c) => crate::emplois::metier(c)?,
        None => None,
    };
    match metier.as_ref().and_then(|m| m.au_catalogue()).map(|f| f.famille.as_str()) {
        None => Err(socle::refus!(
            "Spectacle {}, émission de télévision (convention 1734) : le métier {} de {} n'est pas au catalogue, et \
             le logiciel ne sait pas s'il est artiste-interprète (1734) ou musicien, metteur en scène, artiste de \
             complément (convention de l'entreprise). Choisir un métier du catalogue.",
            sp.cle,
            match code {
                Some(c) => c.as_ref(),
                None => "(aucun)",
            },
            match sal.nom.remplie() {
                Some(nom) => nom,
                None => "ce salarié",
            }
        )),
        Some("Artistes" | "Cirque et rue") => Ok(crate::fiche::IDCC_1734),
        Some(_) => Ok(idcc),
    }
}

/// Le plus bas des candidats selon `valeur` (le premier a egalite) : un minimum prend, quand la paie ne dit pas tout,
/// l'hypothese la plus basse.
pub fn plus_bas<T>(v: Vec<T>, valeur: impl Fn(&T) -> Decimal) -> Option<T> {
    let mut mini: Option<(Decimal, T)> = None;
    for x in v {
        let k = valeur(&x);
        if mini.as_ref().is_none_or(|(m, _)| k < *m) {
            mini = Some((k, x));
        }
    }
    mini.map(|(_, x)| x)
}

/// Les annees revolues de `a` au `b` (une anciennete, un age).
pub fn ans_revolus(a: Jour, b: Jour) -> i64 {
    b.an as i64 - a.an as i64 - i64::from((b.mois, b.jour) < (a.mois, a.jour))
}

/// Un minimum : le montant, d'ou il vient, et s'il se compare a l'unite.
pub struct Plancher {
    pub montant: Decimal,
    pub quoi: String,
    pub unite: bool,
}

/// Un plancher qui se compare a la ligne entiere.
pub fn plancher(montant: Decimal, quoi: String) -> Option<Plancher> {
    Some(Plancher { montant, quoi, unite: false })
}

/// Le montant du palier ou tombe `nombre` de representations.
pub fn palier(paliers: &[Palier], nombre: i64) -> Option<Decimal> {
    paliers.iter().find(|p| p.jusqu_a.is_none_or(|j| nombre <= j)).map(|p| p.montant)
}

impl Table {
    /// La convention `idcc`, si la table la connait.
    pub fn convention(&self, idcc: &crate::fiche::Idcc) -> Option<&Convention> {
        self.conventions.get(idcc)
    }
}

/// L'accord de salaires `id` d'une convention de la table (`<idcc>.accords`), ou `None`.
pub fn accord<'a>(t: &'a Table, id: &AccordDeSalaires) -> Option<&'a Convention> {
    t.conventions.values().flat_map(|c| c.accords.iter().flatten()).find(|a| a.id.as_ref() == Some(id))
}

crate::cle_opaque! {
    /// UNE CLE DES GRILLES DES MINIMA : ce qui nomme une ligne ou une colonne des grilles de `minima-conventionnels.json`
    /// (une grille de cachets `theatre.role`, une fonction de la 2642 ou de la 3097, un groupe de la 1285, un niveau de
    /// la 3090 `cadre_groupe_1`, une jauge, une periode, une categorie d'une grille), et ce qui la designe ailleurs : la
    /// ligne de minima d'un metier et sa classification 3090 (`emplois-base.json`), la grille et le niveau que la prime
    /// d'anciennete lit (`clauses-conventionnelles-base.json`). `decisions-t8.md` Q4 (b) : un type opaque, sans liste
    /// dans le code ; celles que le moteur lit sont des constantes.
    pub struct CleDeGrille, "clé de grille des minima";
}

/// La grille technique de la classification 3090 (`classification_3090.grille`).
pub const TECHNIQUE: CleDeGrille = CleDeGrille::du_moteur("technique");
/// La grille administrative de la classification 3090.
pub const ADMINISTRATIF: CleDeGrille = CleDeGrille::du_moteur("administratif");
/// La ligne de minima de l'artiste-interprete de la 3097 (titre III).
pub const ACTEUR: CleDeGrille = CleDeGrille::du_moteur("acteur");
/// La ligne de minima d'un artiste de cirque de la 1285.
pub const CIRQUE: CleDeGrille = CleDeGrille::du_moteur("cirque");

crate::cle_opaque! {
    /// L'IDENTIFIANT D'UN ACCORD DE SALAIRES (`kali-1285-accord-salaires-20260701`) : l'`id` d'un accord de
    /// `minima-conventionnels.json`, que nomme une valeur du bareme datee par lui (`accord`) ; il relie le bareme aux
    /// minima.
    pub struct AccordDeSalaires, "accord de salaires";
}

/// La compagnie adherente de `organisation` l'est-elle d'une organisation d'employeurs signataire de l'accord ?
/// `organisation` est la cle que le bareme reconnait (`forme_bareme::Pile::organisation`, la seule porte :
/// `Contexte::organisation_reconnue`, `forme_bareme::lire_le_socle`) ; les signataires s'ecrivent par ces cles.
pub fn signataire(accord: &Convention, organisation: Option<&OrganisationPatronale>) -> bool {
    organisation.is_some_and(|o| accord.signataires.iter().flatten().any(|x| x == o))
}

/// LE JOUR OU UNE STIPULATION D'UN ACCORD, DATEE `depuis` PAR L'ACCORD, LIE LA COMPAGNIE ADHERENTE DE `organisation` :
/// `depuis` pour l'adherente d'une organisation signataire (C. trav. L2262-1 : l'accord oblige « les signataires ou
/// membres des organisations ou groupements signataires »), le jour de l'extension pour toute autre compagnie de la
/// convention (`etendu_le` ; L2261-15), au plus tot `depuis` ; `None` tant que l'accord n'est pas etendu et ne la lie
/// pas. Les deux articles : `atelier/reference/sources/legi-entrepot/articles.jsonl.gz`. Les minima d'un accord
/// (`grille_au`) et les valeurs datees du bareme qui en viennent (`accord` dans l'entree,
/// `droit::contexte::dater_les_accords`) se lisent ici.
pub fn lie_depuis(accord: &Convention, organisation: Option<&OrganisationPatronale>, depuis: Jour) -> Option<Jour> {
    if signataire(accord, organisation) {
        return Some(depuis);
    }
    Some(accord.etendu_le?.max(depuis))
}

/// LA GRILLE DE LA CONVENTION `idcc` QUI LIE LA COMPAGNIE AU `jour` : celle du dernier de ses accords (`accords`) dont
/// les minima (`en_vigueur`) la lient a ce jour (`lie_depuis`), avec le jour depuis lequel il la lie ; a defaut, la
/// grille de la convention elle-meme. `None` : la table ne connait pas la convention. Avec elle, la cle des cases du
/// metier (`ParLaGrille`) : seule une grille trouvee les ouvre.
pub fn grille_au<'a>(t: &'a Table, idcc: &crate::fiche::Idcc, organisation: Option<&OrganisationPatronale>, jour: Option<Jour>) -> Option<(&'a Convention, Option<Jour>, ParLaGrille)> {
    let base = t.convention(idcc)?;
    let lie = |a: &'a Convention| {
        // Un accord qui ne dit pas quand ses minima entrent en vigueur ne lie personne.
        let depuis = lie_depuis(a, organisation, a.en_vigueur?)?;
        jour.filter(|j| *j >= depuis).map(|_| (a, Some(depuis)))
    };
    let (grille, depuis) = match base.accords.iter().flatten().rev().find_map(lie) {
        Some(retenu) => retenu,
        None => (base, None),
    };
    Some((grille, depuis, ParLaGrille(())))
}

/// LA CLE DES CASES DU METIER (`fiche::Piece::Metier`) : l'echelon, le poste, l'artiste stagiaire, l'experience que
/// seule une grille des minima lit, et la jauge de la salle que seule la prime d'anciennete d'un technicien de l'annexe I
/// lit. Seule une grille trouvee pour la convention a un jour la donne (`grille_au`) ; la prime d'anciennete tient la
/// sienne (`clauses::prime_anciennete`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParLaGrille(());

impl ParLaGrille {
    /// La cle de la grille de l'annexe I qui donne la prime d'anciennete d'un technicien (`clauses::prime_anciennete`).
    pub(crate) fn de_l_annexe() -> ParLaGrille {
        ParLaGrille(())
    }
}

/// Ce qu'un minimum compare : le prix d'une heure, d'un cachet, d'un service, ou d'une unite de grille (la journee,
/// la semaine, le mois).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Par {
    Heure,
    Cachet,
    Service,
    Unite,
}

/// Le minimum conventionnel d'une ligne de paie, tel que les avertissements le lisent : le montant, la grille d'ou il
/// vient, ce qu'il compare. `service_3090` : le service de repetition d'un artiste de la 3090 (sa phrase est a part).
#[derive(Debug, Clone, PartialEq)]
pub struct Minimum {
    pub montant: Decimal,
    pub quoi: String,
    pub par: Par,
    pub service_3090: bool,
}
