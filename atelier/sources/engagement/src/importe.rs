//! L'ENGAGEMENT IMPORTE : un engagement que l'import a tire des pieces d'un autre logiciel (les DSN deposees, les AEM,
//! les bulletins de paie). Il vit avec les autres, dans `engagements/AAAA-MM.json`, le fichier du mois ou il commence,
//! et porte `importe` (`Pieces`) : quand il a ete importe, le contrat qu'une AEM, un bulletin ou un certificat atteste
//! (`ContratAtteste`), et ce qui a ete verse, mois par mois (`PaieVersee`). Cette zone se lit et s'ecrit par sa forme,
//! et par elle seule (`Pieces::lire`, `Pieces::en_valeur`) ; l'engagement la lit une fois, avec son fichier
//! (`stock::Objet::importe`).
//!
//! Ses mois sont valides et leurs documents sont partis : ce qui en est parti, c'est l'import lui-meme (les pieces de
//! l'autre logiciel). SES MOIS REPRIS SE LISENT, ILS NE SE MODIFIENT PAS ET NE SE RECALCULENT JAMAIS : ils ne sont
//! aucun mois de la saisie (`stock::Objet::couvre`), donc aucun calcul ne les refait, aucune declaration ni aucun
//! document qui part ne les prend (un calcul d'un autre mois lit ce qu'ils ont verse : `reprises_du_salarie`) ; l'ecran ne peut ni les reecrire ni en poser un double (`stock::ecrire_la_carte`). Son numero de
//! contrat est celui que la DSN deposee lui a donne (S21.G00.40.009), jamais un numero tire : il compte parmi les
//! numeros pris (`numeros`).
//!
//! UN CONTRAT DE PERMANENT EST UN ENGAGEMENT, QU'IL AIT COMMENCE AILLEURS OU ICI. Les bulletins d'un permanent attestent
//! chacun le mois qu'ils paient, tous du meme contrat : ils font un engagement importe, dont ses paies sont les mois.
//! Paye le dernier mois repris et sans terme avant le mois qui suit (`continue_ici`), il continue ici : il vit dans le
//! fichier de son premier mois neuf (`ecrire_tous`), son mois d'entree, et ce mois et les suivants se saisissent et
//! se calculent comme ceux de tout engagement, sous la meme identite et le meme numero -- la DSN declare le contrat
//! sous le numero (S21.G00.40.009) et la date de debut (.001) que l'ancien logiciel lui a donnes, reportes a
//! l'identique d'une DSN a la suivante (`norme-dsn/texte/bloc-S21.G00.40.txt` ; cahier technique § 2.1.2,
//! `norme-dsn/texte/chapitre-2-principes-de-constitution.txt`). Il ne se modifie pas avant ce mois : ce qui le definit
//! se lit a partir de lui.
//!
//! Ce qui le lit : les cumuls des annexes 8 et 10 (`cumul`, par ce que le contrat atteste), le mois tenu avant
//! PuppetPaie (`contrats_lus`, `paies_lues` : un contrat atteste, une paie versee), les etats, qui lisent chacune de
//! ses paies dans les mots du moteur (`Importe::paie`, par `etats::paies_du_mois`), et les calculs qui lisent le
//! salaire d'un autre mois, qui lisent ses paies telles que les pieces les disent, sans rien en calculer
//! (`reprises_du_salarie`, par `etats::paies_du_salarie`). L'import manipule les memes formes,
//! avec les bornes de leur engagement (`ContratLu`, `PaieLue`), de la lecture des pieces a l'ecriture (`Neuf`).

use std::collections::BTreeMap;

use droit::assiette::Assiette;
use droit::base_assujettie::BaseAssujettie;
use droit::fiche::{Lue, NumeroObjetSpectacle};
use droit::natures::NatureDeContrat;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::numeros::NumeroContrat;
use crate::saisie::Carte;
use crate::stock::{self, Objet};
use droit::contexte::{Contexte, IdSalarie};
use socle::date::{Jour, PeriodeOuverte};
use socle::echec::R;
use socle::fichiers::NomDeFichier;
use socle::mois::Mois;
use socle::grandeurs::{Cachets, Heures, Jours, Montant, Taux};

/// Dans un engagement importe : ce que l'import a lu de lui (`Pieces`).
pub const IMPORTE: &str = "importe";

// ------------------------------------------------------------------ la forme

/// LA ZONE `importe` D'UN ENGAGEMENT : le jour de l'import, le contrat atteste, s'il l'est, et les paies versees, par
/// mois (`AAAA-MM`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pieces {
    pub le: Jour,
    pub contrat: Option<ContratAtteste>,
    pub paies: BTreeMap<Mois, PaieVersee>,
}

/// La piece qui atteste un contrat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attestation {
    #[serde(rename = "AEM")]
    Aem,
    #[serde(rename = "bulletin")]
    Bulletin,
    #[serde(rename = "certificat")]
    Certificat,
}

/// CE QU'UNE AEM, UN BULLETIN OU UN CERTIFICAT CONGES SPECTACLES ATTESTE DU CONTRAT (ses bornes sont celles de
/// l'engagement) : ce qu'il compte, son brut, son numero d'objet, la piece et sa page. Le contrat entier d'un permanent ne
/// garde ni compteurs, ni brut, ni piece : ce qu'un bulletin atteste de son seul mois reste a la paie de ce mois.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContratAtteste {
    pub atteste_par: Attestation,
    pub numero_objet: Option<NumeroObjetSpectacle>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::entier::option::serialize")]
    pub heures: Option<Heures>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::entier::option::serialize")]
    pub cachets: Option<Cachets>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::entier::option::serialize")]
    pub jours: Option<Jours>,
    /// Les heures d'enseignement que les heures comptent (annexe 10).
    #[serde(serialize_with = "socle::grandeurs::ecrit::entier::option::serialize")]
    pub heures_enseignement: Option<Heures>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub brut: Option<Montant>,
    /// La base conge qu'un certificat Congés Spectacles porte, pour le contrat entier.
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub base_conge: Option<Montant>,
    /// La nature du contrat, que la piece atteste (`ContratAtteste::vide`) ou qui a ete dite ; `None` : elle reste a dire
    /// (le bulletin d'un permanent ne la porte pas : elle se demande apres l'import, `import::nature`).
    pub nature: Option<NatureDeContrat>,
    pub numero_bulletin: Option<String>,
    /// Le numero du contrat qu'un bulletin imprime (il se lit, il ne se tire pas : regle 4).
    pub numero_contrat: Option<NumeroContrat>,
    pub numero_attestation: Option<String>,
    pub numero_certificat: Option<String>,
    /// Le jour que la piece porte (sa signature), tel qu'il s'y lit.
    pub atteste_le: Option<Jour>,
    /// La copie de la piece, sous `historique/bulletins/`, et sa page.
    pub fichier: Option<NomDeFichier>,
    pub page: Option<u64>,
    pub importe_le: Option<Jour>,
}

/// CE QUI RECONNAIT UNE PAIE VERSEE D'UN IMPORT A L'AUTRE, ET CE QUI L'A DECLAREE : le numero du contrat que la DSN
/// deposee declare (S21.G00.51.010) ; sinon un bulletin seul, par le numero qu'il imprime (le texte d'autrui), ou, s'il
/// n'en imprime pas, par sa copie (sous `historique/bulletins/`) et sa page quand la copie en a plusieurs. Elle s'ecrit
/// avec son origine : `{"dsn": "<numero>"}`, `{"bulletin": "<numero imprime>"}`, `{"copie": {"fichier", "page"}}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum CleDePaieVersee {
    Dsn(NumeroContrat),
    Bulletin(String),
    Copie { fichier: NomDeFichier, page: Option<u64> },
}

impl CleDePaieVersee {
    /// Le numero du contrat qu'une DSN deposee declare ; aucun : un bulletin seul l'a declaree.
    pub fn numero_declare(&self) -> Option<&NumeroContrat> {
        match self {
            CleDePaieVersee::Dsn(n) => Some(n),
            CleDePaieVersee::Bulletin(_) | CleDePaieVersee::Copie { .. } => None,
        }
    }

    /// Un bulletin seul l'a-t-il declaree (par son numero ou par sa copie) ?
    pub fn d_un_bulletin(&self) -> bool {
        self.numero_declare().is_none()
    }

    /// Ce que la page en montre (la carte d'un engagement importe) : le numero declare, `bulletin <numero>`, ou la copie
    /// et sa page.
    pub fn dite(&self) -> String {
        match self {
            CleDePaieVersee::Dsn(n) => n.to_string(),
            CleDePaieVersee::Bulletin(n) => format!("bulletin {}", n),
            CleDePaieVersee::Copie { fichier, page: Some(p) } => format!("{} page {}", fichier, p),
            CleDePaieVersee::Copie { fichier, page: None } => fichier.to_string(),
        }
    }
}

/// Les paies d'un import se rangent dans l'ordre de ce qu'elles disent (`dite`) ; deux origines qui le disent pareil (un
/// numero declare « bulletin 12345 » et le bulletin 12345), dans l'ordre de leur origine.
impl Ord for CleDePaieVersee {
    fn cmp(&self, autre: &Self) -> std::cmp::Ordering {
        let rang = |c: &CleDePaieVersee| match c {
            CleDePaieVersee::Dsn(_) => 0,
            CleDePaieVersee::Bulletin(_) => 1,
            CleDePaieVersee::Copie { .. } => 2,
        };
        self.dite().cmp(&autre.dite()).then_with(|| rang(self).cmp(&rang(autre)))
    }
}

impl PartialOrd for CleDePaieVersee {
    fn partial_cmp(&self, autre: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(autre))
    }
}

/// CE QUI A ETE VERSE UN MOIS : ce que la DSN deposee declare (bloc 50, ses bases du bloc 78), complete par le bulletin
/// de ce mois, qui fait foi sur ce qu'il imprime (`import::bulletins::completer`). `contrat` est ce qui la reconnait d'un
/// import a l'autre, et dit qui l'a declaree (`CleDePaieVersee`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaieVersee {
    pub contrat: CleDePaieVersee,
    pub nir: Option<droit::fiche::Nir>,
    pub numero_objet: Option<NumeroObjetSpectacle>,
    /// Les dates du contrat qu'elle paie (S21.G00.40.001, .010 : un CDI n'a pas de fin), quand une piece les dit.
    pub periode: Option<socle::date::PeriodeOuverte>,
    /// La quotite que la DSN deposee declare pour le contrat (S21.G00.40.013), dans l'unite de 40.011.
    pub quotite: Option<droit::natures::QuotiteDuContrat>,
    /// Le motif de recours que la DSN deposee declare pour le contrat (S21.G00.40.021).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motif_recours: Option<droit::natures::MotifDeRecours>,
    /// La nature du contrat d'usage que la DSN deposee declare (S21.G00.40.007 et .021 : `droit::natures::d_usage_declaree`) ;
    /// `None` : elle n'en declare pas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nature: Option<NatureDeContrat>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub brut: Option<Montant>,
    /// Ses nets et son prelevement a la source, que la DSN deposee (bloc 50) ou son bulletin disent.
    pub nets: Nets,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub net_a_payer: Option<Montant>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub net_avant_impot: Option<Montant>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub net_social: Option<Montant>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub cotisations_salariales: Option<Montant>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub cotisations_patronales: Option<Montant>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub cout_employeur: Option<Montant>,
    pub verse_le: Option<Jour>,
    /// Le depot de la DSN qui la declare, et la copie de son fichier.
    pub depose_le: Option<Jour>,
    pub fichier: Option<NomDeFichier>,
    /// Les bases assujetties que la DSN declare, par code du bloc 78.
    #[serde(with = "bases_en_texte")]
    pub bases: BTreeMap<BaseAssujettie, Montant>,
    /// L'ACTIVITE DU MOIS (heures, cachets, jours) : ce que la DSN deposee declare du travail remunere du contrat pour le
    /// mois (S21.G00.53 de type 01, `bloc-S21.G00.53.txt` : le « temps alloue par le salarie a un type d'activite »,
    /// « valorise par periode de paie » et « rattache a un et un seul contrat » ; lu par `declarations::dsn_lue::Mesures`,
    /// le rappel d'une periode anterieure hors du compte), tel qu'elle l'ecrit, au centieme pres : des heures en unites
    /// 10 (« heure ») et 35 (« heures intermittents du spectacle »), des jours en unite 12 (« journee »), des cachets en
    /// unite 37 (« cachets isoles ») ; une mesure nulle ne dit rien. Le bulletin du mois d'un permanent les remplace par ce
    /// qu'il imprime (il fait foi sur ce qu'il imprime : `routes::importation::bulletins`). Ce que l'AEM, le bulletin ou
    /// le certificat d'un contrat d'usage atteste est au contrat entier (`ContratAtteste`) ; lequel l'emporte pour un
    /// mois, `activite_du_mois` le dit.
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub heures: Option<Heures>,
    /// LA DUREE DU TRAVAIL QUE LE BULLETIN IMPRIME en en-tete (« Durée du travail : 151,67 h ») : le nombre d'heures de
    /// travail auquel le salaire se rapporte (C. trav. R. 3243-1, 5°), quand il ne l'imprime pas sur sa ligne de
    /// salaire (« Salaire mensuel de base »).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "socle::grandeurs::ecrit::texte::option::serialize",
        deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize"
    )]
    pub duree_du_travail: Option<Heures>,
    /// Des cachets entiers : la DSN ne les ecrit pas autrement (`declarations::dsn::MesureDActivite::lue`).
    #[serde(serialize_with = "socle::grandeurs::ecrit::entier::option::serialize")]
    pub cachets: Option<Cachets>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub jours: Option<Jours>,
    pub spectacle: Option<String>,
    pub analytique: Option<String>,
    pub lignes: Vec<LigneImprimee>,
    /// Les jours travailles que le bulletin imprime, en periodes.
    pub dates: Vec<socle::date::Periode>,
    /// Les lignes de cotisation du bulletin ; `None` : aucun bulletin ne les a donnees.
    pub cotisations: Option<Vec<CotisationImprimee>>,
    /// L'emploi que CE bulletin imprime, en champs de la fiche (`import::bulletins::statut_et_metier`) : celui de
    /// l'engagement importe.
    pub emploi_imprime: Option<droit::emplois::EmploiEngagement>,
    pub bulletin: Option<BulletinSource>,
    pub importe_le: Option<Jour>,
}

/// LES NETS D'UNE PAIE VERSEE ET SON PRELEVEMENT A LA SOURCE, tels que le bloc 50 d'une DSN les dit du versement : ses
/// nets (.002, .004) et son prelevement (.006 a .008 : le taux, son type et son identifiant ; .009 : l'impot preleve).
/// Le bloc n'a pas de numero de contrat (`bloc-S21.G00.50.txt`) : ces montants ne sont ceux d'un contrat que si le
/// versement ne paie que lui, et lui seul (`declarations::dsn_lue::PaieDuContrat::nets`). Une paie versee les porte
/// (`PaieVersee::nets`), son bulletin completant ce que la DSN tait. Ce qu'une piece tait est vide.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nets {
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub net_imposable: Option<Montant>,
    /// Le net verse (50.004), qui n'est pas le net a payer.
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub net_verse: Option<Montant>,
    pub taux_pas: Option<droit::pas::TauxDePas>,
    pub type_pas: Option<droit::type_taux_pas::TypeDeTauxPas>,
    pub identifiant_pas: Option<droit::pas::IdentifiantDuTauxPas>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub pas: Option<Montant>,
}

/// Une ligne de remuneration que le bulletin imprime ; `rubrique` : celle du catalogue a laquelle l'import l'a rattachee,
/// et `paiement`, sa quantite et son taux dans l'unite de cette rubrique, quand le bulletin les imprime
/// (`ligne::PaiementDeLigne::imprime`). Une ligne sans rubrique ne garde que son libelle et son montant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LigneImprimee {
    pub libelle: String,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub montant: Option<Montant>,
    pub soumis: bool,
    pub rubrique: Option<droit::catalogue::CodeDeRubrique>,
    pub paiement: Option<crate::ligne::PaiementDeLigne>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub heures: Option<socle::grandeurs::HeuresParUnite>,
}

impl LigneImprimee {
    /// LE RATTACHEMENT D'UNE LIGNE IMPRIMEE, le seul : sa ligne de remuneration, quand l'import l'a rattachee a une
    /// rubrique et qu'elle dit son montant, meme sans quantite ni taux (un forfait n'a pas de quantite) : ce montant, tel
    /// quel, et ce qu'elle paie et ses heures quand elle les dit, sous le code et le libelle de sa rubrique
    /// (`calcul::ligne_rattachee`). Une ligne qui n'est pas rattachee garde ce qu'elle imprime
    /// (`PaieImportee::remuneration`) : elle n'est pas une ligne de paie.
    pub(crate) fn rattachee(&self, ctx: &Contexte) -> R<Option<crate::calcul::LigneRattachee>> {
        let (Some(rubrique), Some(montant)) = (&self.rubrique, self.montant) else { return Ok(None) };
        crate::calcul::ligne_rattachee(rubrique, self.paiement, self.heures, montant, ctx).map(Some)
    }
}

/// UNE LIGNE DE COTISATION QU'UN BULLETIN IMPRIME ; `cotisation` : celle du bareme que l'import lui a donnee (`None` : il
/// ne l'a pas qualifiee) ; ce que le bulletin n'imprime pas est vide. Un etat la lit d'une paie du mois importee
/// (`etats::CotisationDuMois`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CotisationImprimee {
    pub famille: Option<droit::forme_bareme::FamilleDeCotisation>,
    pub libelle: String,
    pub cotisation: Option<crate::ecarts::RegroupementDeComparaison>,
    /// La base imprimee, dans la grandeur que sa cotisation compte, posee quand l'import la qualifie
    /// (`calcul::BaseDeCotisation::imprimee`) : des euros, ou des heures pour la deduction des heures supplementaires.
    pub base: Option<crate::calcul::BaseDeCotisation>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub taux_employeur: Option<Taux>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub montant_employeur: Option<Montant>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub taux_salarie: Option<Taux>,
    #[serde(default, serialize_with = "socle::grandeurs::ecrit::texte::option::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::option::deserialize")]
    pub montant_salarie: Option<Montant>,
    pub hors_total: bool,
}

/// Les bases du bloc 78 d'une paie versee, chacune en texte (`socle::grandeurs::ecrit::texte`), relue d'un nombre ou
/// d'un texte (`socle::grandeurs::lu::nombre_ou_texte`).
mod bases_en_texte {
    use droit::base_assujettie::BaseAssujettie;
    use serde::{Deserialize, Serialize};
    use socle::grandeurs::Montant;
    use std::collections::BTreeMap;

    #[derive(Serialize, Deserialize)]
    struct EnTexte(
        #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::deserialize")] Montant,
    );

    pub fn serialize<S: serde::Serializer>(t: &BTreeMap<BaseAssujettie, Montant>, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(t.iter().map(|(k, m)| (k, EnTexte(*m))))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<BaseAssujettie, Montant>, D::Error> {
        BTreeMap::<BaseAssujettie, EnTexte>::deserialize(d).map(|t| t.into_iter().map(|(k, EnTexte(m))| (k, m)).collect())
    }
}

/// Le bulletin qui a complete la paie : sa copie, sa page, le logiciel qui l'a fait, son numero, le numero du contrat
/// qu'il imprime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BulletinSource {
    pub fichier: NomDeFichier,
    pub page: u64,
    pub logiciel: Option<LogicielDUnBulletin>,
    pub numero: Option<String>,
    pub contrat: Option<NumeroContrat>,
}

droit::cle_opaque! {
    /// LE LOGICIEL DE PAIE D'UN BULLETIN IMPORTE, reconnu a sa signature : une cle de `logiciels` de la table de lecture
    /// des bulletins (`bulletins-lecture-base.json`). Une cle du logiciel (`droit::cle_opaque!`) ; aucun : un autre
    /// logiciel.
    pub struct LogicielDUnBulletin, "logiciel d'un bulletin";
}

/// UN CONTRAT ATTESTE, AVEC LES BORNES DE SON ENGAGEMENT : la ligne que l'import manipule, de la lecture de la piece a
/// l'engagement ecrit (`Neuf`). `salarie` est la cle de la fiche : une ligne de l'apercu peut ne pas en avoir (`None`
/// tant qu'elle n'existe pas, ecrit `<a creer>`) ; une ligne relue d'un engagement importe en a toujours une
/// (`ContratRelu`). `numero`, le numero de contrat de l'engagement qui le porte. Elle se rend a la page telle qu'elle
/// s'ecrit (`Serialize`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(bound(serialize = "S: CleDeLaLigne"))]
pub struct ContratLu<S = Option<IdSalarie>> {
    #[serde(serialize_with = "ecrit_salarie")]
    pub salarie: S,
    /// Les dates du contrat, quand la ligne dit son debut (sans fin : il court encore).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub periode: Option<PeriodeOuverte>,
    #[serde(rename = "contrat", skip_serializing_if = "Option::is_none")]
    pub numero: Option<crate::numeros::NumeroContrat>,
    #[serde(flatten)]
    pub contrat: ContratAtteste,
}

/// UNE PAIE VERSEE, AVEC SON MOIS ET LE SALARIE DE SON ENGAGEMENT : la ligne de paie que l'import manipule, de la
/// lecture de la DSN ou du bulletin a l'engagement ecrit. Sa cle de salarie, comme celle de `ContratLu` : facultative
/// dans l'apercu, toujours la (`PaieRelue`) dans une ligne relue d'un engagement importe.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(bound(serialize = "S: CleDeLaLigne"))]
pub struct PaieLue<S = Option<IdSalarie>> {
    pub mois: Mois,
    #[serde(serialize_with = "ecrit_salarie")]
    pub salarie: S,
    #[serde(flatten)]
    pub paie: PaieVersee,
}

/// Le contrat atteste relu d'un engagement importe : son salarie a sa fiche.
pub type ContratRelu = ContratLu<IdSalarie>;

/// Une paie versee relue d'un engagement importe : son salarie a sa fiche.
pub type PaieRelue = PaieLue<IdSalarie>;

/// LA CLE DU SALARIE D'UNE LIGNE LUE, telle que la page la lit : celle de sa fiche, ou `<a creer>` tant que sa fiche
/// n'existe pas (`None`, dans l'apercu).
pub trait CleDeLaLigne {
    fn ecrite(&self) -> &str;
}

impl CleDeLaLigne for IdSalarie {
    fn ecrite(&self) -> &str {
        self.as_str()
    }
}

impl CleDeLaLigne for Option<IdSalarie> {
    fn ecrite(&self) -> &str {
        match self {
            Some(c) => c.as_str(),
            None => "<a creer>",
        }
    }
}

pub fn ecrit_salarie<C:CleDeLaLigne, S: serde::Serializer>(salarie: &C, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(salarie.ecrite())
}

impl ContratRelu {
    /// La meme ligne, parmi celles de l'apercu.
    pub fn a_l_apercu(self) -> ContratLu {
        ContratLu { salarie: Some(self.salarie), periode: self.periode, numero: self.numero, contrat: self.contrat }
    }
}

impl PaieRelue {
    /// La meme ligne, parmi celles de l'apercu.
    pub fn a_l_apercu(self) -> PaieLue {
        PaieLue { mois: self.mois, salarie: Some(self.salarie), paie: self.paie }
    }
}

impl<S: Clone> PaieLue<S> {
    /// Ce qui la reconnait d'un import a l'autre : (mois, salarie, contrat).
    pub fn cle(&self) -> (Mois, S, CleDePaieVersee) {
        (self.mois, self.salarie.clone(), self.paie.contrat.clone())
    }

    /// La meme, sans ce qui dit quand elle a ete importee : deux lectures des memes pieces se comparent ainsi.
    pub fn sans_date_d_import(&self) -> PaieLue<S> {
        let mut p = self.clone();
        p.paie.importe_le = None;
        p
    }
}

impl PaieVersee {
    /// Une paie dont on ne sait encore que ce qui la reconnait et la declare.
    pub fn vide(contrat: CleDePaieVersee) -> PaieVersee {
        PaieVersee {
            contrat,
            nir: None,
            numero_objet: None,
            periode: None,
            quotite: None,
            motif_recours: None,
            nature: None,
            brut: None,
            nets: Nets::default(),
            net_a_payer: None,
            net_avant_impot: None,
            net_social: None,
            cotisations_salariales: None,
            cotisations_patronales: None,
            cout_employeur: None,
            verse_le: None,
            depose_le: None,
            fichier: None,
            bases: BTreeMap::new(),
            heures: None,
            duree_du_travail: None,
            cachets: None,
            jours: None,
            spectacle: None,
            analytique: None,
            lignes: Vec::new(),
            dates: Vec::new(),
            cotisations: None,
            emploi_imprime: None,
            bulletin: None,
            importe_le: None,
        }
    }
}

/// L'ACTIVITE QUE LES PIECES DISENT D'UN CONTRAT REPRIS POUR `mois` : (cachets, heures, jours), chacun vide quand elles
/// le taisent. Une seule regle, pour ce qui lit l'activite d'un mois repris :
/// - le contrat qu'une AEM, un bulletin ou un certificat atteste (`contrat`), quand il tient dans `mois` et en dit
///   quelque chose : la piece du contrat compte le contrat entier, qui est ici celui du mois. L'AEM est « l'attestation
///   employeur mensuelle » du contrat (S21.G00.40.025, `bloc-S21.G00.40.txt`) ; la DSN valorise la meme activite « par
///   periode de paie » (`bloc-S21.G00.53.txt`) : la piece du contrat l'emporte ;
/// - sinon, la paie du mois (`paie`) : le bulletin du mois d'un permanent, ou ce que la DSN declare pour le mois
///   (`PaieVersee::heures`). Un contrat qui deborde du mois ne dit pas ce qui est de ce mois : la paie le dit.
///
/// `debut` et `fin` : les bornes de l'engagement qui porte le contrat (`Importe::activite_du_mois` les lit sur lui).
pub fn activite_du_mois(
    mois: Mois, paie: Option<&PaieVersee>, contrat: Option<&ContratAtteste>, debut: Option<Jour>, fin: Option<Jour>,
) -> (Option<Cachets>, Option<Heures>, Option<Jours>) {
    let tient = [debut, fin].iter().all(|j| j.is_some_and(|j| j.mois_civil() == mois));
    let atteste = contrat.filter(|_| tient).map(|c| (c.cachets, c.heures, c.jours)).filter(|(c, h, j)| c.is_some() || h.is_some() || j.is_some());
    match (atteste, paie) {
        (Some(a), _) => a,
        (None, Some(p)) => (p.cachets, p.heures, p.jours),
        (None, None) => (None, None, None),
    }
}

impl ContratAtteste {
    /// Un contrat dont on ne sait encore que ce qui l'atteste, et la nature que la piece atteste (`None` : a dire).
    pub fn vide(atteste_par: Attestation, nature: Option<NatureDeContrat>) -> ContratAtteste {
        ContratAtteste {
            atteste_par,
            numero_objet: None,
            heures: None,
            cachets: None,
            jours: None,
            heures_enseignement: None,
            brut: None,
            base_conge: None,
            nature,
            numero_bulletin: None,
            numero_contrat: None,
            numero_attestation: None,
            numero_certificat: None,
            atteste_le: None,
            fichier: None,
            page: None,
            importe_le: None,
        }
    }
}

// ------------------------------------------------------------------ l'engagement importe

/// Un engagement importe : l'engagement tel qu'il est ecrit, ses pieces avec lui.
#[derive(Debug, Clone)]
pub struct Importe {
    objet: Objet,
    /// Le contrat d'un permanent : paye au mois, ou de nature encore a dire ; lu une fois, avec sa nature.
    permanent: bool,
}

impl Importe {
    /// L'engagement `o` s'il est importe ; une nature dite que le moteur ne connait pas se refuse.
    pub fn de(o: Objet) -> R<Option<Importe>> {
        if !o.est_importe() {
            return Ok(None);
        }
        let mut i = Importe { objet: o, permanent: true };
        if let Some(n) = i.nature() {
            i.permanent = n.proprietes()?.mensuelle();
        }
        Ok(Some(i))
    }

    /// L'engagement tel qu'il est ecrit.
    pub fn objet(&self) -> &Objet {
        &self.objet
    }

    /// Ce que l'import a lu de lui.
    pub fn pieces(&self) -> &Pieces {
        self.objet.pieces().expect("un engagement importe a ses pieces (`de`)")
    }

    /// La carte de l'engagement lui-meme (son salarie, ses bornes, son numero, sa nature).
    fn carte(&self) -> &Carte {
        self.objet.carte()
    }

    /// Quand il a ete importe.
    pub fn le(&self) -> Jour {
        self.pieces().le
    }

    /// Ce que l'AEM, le bulletin ou le certificat atteste du contrat, s'il l'est.
    pub fn contrat(&self) -> Option<&ContratAtteste> {
        self.pieces().contrat.as_ref()
    }

    /// Ce qui a ete verse, mois par mois.
    pub fn paies(&self) -> &BTreeMap<Mois, PaieVersee> {
        &self.pieces().paies
    }

    /// LE CONTRAT D'UN PERMANENT S'ATTESTE MOIS PAR MOIS (`permanent`), comme ses bulletins l'ont atteste : chaque mois
    /// verse, aux dates du mois dans le contrat (`crate::contrat_dans_le_mois`, qui refuse un mois verse hors de ses
    /// dates), avec la paie de ce mois. `contrat` : ses dates (`ContratLu::periode`, lues de l'engagement : `contrat_lu`) ;
    /// `qui` : le contrat tel qu'un refus le nomme. Le registre de l'import et le calendrier des mois passes le lisent.
    pub fn mois_par_mois(&self, contrat: &PeriodeOuverte, qui: &str) -> R<Vec<(socle::date::Periode, &PaieVersee)>> {
        self.paies().iter().map(|(m, p)| Ok((crate::contrat_dans_le_mois(contrat, *m, qui)?, p))).collect()
    }

    /// La nature du contrat : celle qui a ete dite (l'engagement), sinon celle que la piece atteste
    /// (`routes::importation::bulletins` : une AEM, un certificat Congés Spectacles, un bulletin qui porte une marque du
    /// spectacle), sinon le contrat d'usage que sa DSN declare (`PaieVersee::nature`). `None` : elle reste a dire
    /// (`a_dire`).
    pub fn nature(&self) -> Option<NatureDeContrat> {
        self.nature_dite()
    }

    /// LA NATURE SOUS LAQUELLE CE QUI A ETE VERSE SE RANGE (`paie`) : la sienne (`nature`). Une nature ne se devine pas
    /// (regle 6) : tant qu'elle reste a dire, le refus la nomme, et elle se dit par `importation-nature`.
    pub fn nature_rangee(&self) -> R<NatureDeContrat> {
        self.nature().ok_or_else(|| {
            socle::echec::cle(
                format!(
                    "{} (rang {}) : la nature de ce contrat repris reste à dire (Configuration, « Reprendre l'historique ») ; \
                     ce qu'il a versé ne se range pas sans elle.",
                    self.objet.fichier, self.objet.rang
                ),
                "nature",
            )
        })
    }

    /// LES DATES DU CONTRAT QUE PAIE `p`, l'une de ses paies : celles de l'engagement, sinon celles de la paie. Le contrat
    /// d'un permanent finit a sa fin, dite ou declaree, jamais a celle du mois que paie un bulletin ; sans fin dite, un
    /// autre finit avec la paie. `None` : ni l'engagement ni la paie ne disent son debut. Sa paie dans les mots du moteur
    /// (`paie`) et la saisie que la comparaison refait (`ecarts`) le lisent.
    pub(crate) fn contrat_de_la_paie(&self, p: &PaieVersee) -> R<Option<PeriodeOuverte>> {
        Ok(Some(match (self.carte().contrat()?, p.periode) {
            (Some(c), _) if c.fin().is_some() || self.permanent() => c,
            (Some(c), paie) => match paie.and_then(|o| o.fin()) {
                Some(fin) => c.fermee_au(fin)?.into(),
                None => c,
            },
            (None, Some(o)) if self.permanent() => o.sans_fin(),
            (None, Some(o)) => o,
            (None, None) => return Ok(None),
        }))
    }

    /// LE MOTIF DE RECOURS DU CONTRAT : celui que l'engagement dit, sinon celui que la derniere DSN deposee qui declare
    /// une de ses paies declare pour lui (S21.G00.40.021, `PaieVersee::motif_recours`) : une piece propre se prend telle
    /// quelle (regle 1). `None` : personne ne le dit.
    pub fn motif_recours(&self) -> Option<droit::natures::MotifDeRecours> {
        self.carte().motif_recours.or_else(|| self.paies().values().rev().find_map(|p| p.motif_recours))
    }

    /// L'activite que ses pieces disent pour `mois` (`activite_du_mois`, aux bornes de l'engagement).
    pub fn activite_du_mois(&self, mois: Mois) -> (Option<Cachets>, Option<Heures>, Option<Jours>) {
        activite_du_mois(mois, self.paies().get(&mois), self.contrat(), self.carte().debut, self.carte().fin)
    }

    /// La nature dite (l'engagement) ou attestee (la piece), sinon le contrat d'usage que la derniere DSN deposee qui
    /// declare une de ses paies declare (`PaieVersee::nature`) ; `None` : rien ne la porte.
    fn nature_dite(&self) -> Option<NatureDeContrat> {
        self.carte().nature.or_else(|| self.contrat().and_then(|c| c.nature)).or_else(|| self.paies().values().rev().find_map(|p| p.nature))
    }

    /// Sa nature reste-t-elle a dire ? Le bulletin d'un permanent ne la porte pas (`ContratAtteste::nature`), et personne ne l'a
    /// dite.
    pub fn a_dire(&self) -> bool {
        self.nature().is_none()
    }

    /// Le contrat d'un permanent : paye au mois, ou de nature encore a dire.
    pub fn permanent(&self) -> bool {
        self.permanent
    }

    /// LE CONTRAT D'USAGE QUI CONTINUE ICI : le dernier mois verse par l'ancien logiciel, quand le contrat que la DSN
    /// deposee declare finit apres lui (S21.G00.40.010, sa fin previsionnelle : `norme-dsn/texte/bloc-S21.G00.40.txt`).
    /// Les mois qui suivent se paient ici, sous le numero et les dates que cette DSN lui a donnes (regle 4). Un contrat
    /// qu'une AEM atteste est fini (l'AEM se remet a la fin du contrat) ; un contrat paye au mois a sa propre reprise,
    /// sa nature dite (`import::nature`) ; sans numero, rien ne se reprend.
    pub fn continue_apres(&self) -> Option<Mois> {
        let dernier = *self.paies().keys().max()?;
        let apres = self.carte().fin.is_some_and(|f| f.mois_civil() > dernier);
        let par_aem = self.contrat().is_some_and(|c| c.atteste_par == Attestation::Aem);
        let continue_ = apres && !par_aem && !self.permanent() && self.carte().numero.is_some();
        continue_.then_some(dernier)
    }

    /// Les mois qu'il touche : de son debut a sa fin (au dernier mois verse, s'il continue ici : `continue_apres`), et
    /// ceux ou il a ete verse. Le contrat d'un permanent ne touche que les mois ou il a ete verse : des mois d'avant le
    /// premier bulletin repris, les pieces ne disent rien, et ceux d'apres le dernier sont ceux d'ici.
    pub fn mois(&self) -> Vec<Mois> {
        let mut v: Vec<Mois> = self.paies().keys().copied().collect();
        if let (Some(debut), false) = (self.carte().debut.map(|j| j.mois_civil()), self.permanent()) {
            let fin = match (self.continue_apres(), self.carte().fin.map(|j| j.mois_civil())) {
                (Some(dernier), _) => dernier,
                (None, Some(fin)) => fin,
                (None, None) if !v.is_empty() => {
                    v.sort();
                    return v;
                }
                (None, None) => debut,
            };
            v.extend(debut.jusqu_a(fin));
        }
        v.sort();
        v.dedup();
        v
    }

    /// Le contrat atteste, avec les bornes de l'engagement. La nature dite est celle du contrat : il n'attend plus rien.
    pub fn contrat_lu(&self) -> R<Option<ContratRelu>> {
        let Some(contrat) = self.contrat() else { return Ok(None) };
        let mut contrat = contrat.clone();
        if let Some(n) = self.nature_dite() {
            contrat.nature = Some(n);
        }
        // Importe le jour ou il l'a ete la premiere fois ; sinon, le jour de l'import de ses pieces (`Pieces::le`).
        contrat.importe_le.get_or_insert(self.le());
        let numero = self.carte().numero.clone();
        Ok(Some(ContratLu { salarie: self.cle_ecrite()?, periode: self.carte().contrat()?, numero, contrat }))
    }

    /// Les paies versees, une par mois, avec le salarie de l'engagement.
    pub fn paies_lues(&self) -> R<Vec<PaieRelue>> {
        self.paies().iter().map(|(m, p)| self.paie_lue(*m, p)).collect()
    }

    fn paie_lue(&self, mois: Mois, p: &PaieVersee) -> R<PaieRelue> {
        let mut paie = p.clone();
        paie.importe_le.get_or_insert(self.le());
        Ok(PaieLue { mois, salarie: self.cle_ecrite()?, paie })
    }

    /// La cle de la fiche de son salarie, ecrite ; un engagement importe qui ne la dit pas se refuse.
    pub(crate) fn cle_ecrite(&self) -> R<droit::contexte::IdSalarie> {
        let o = &self.objet;
        o.carte().salarie.clone().ok_or_else(|| socle::refus!("l'engagement importé {} (rang {}) ne dit pas son salarié", o.fichier, o.rang))
    }
}

/// LA PAIE D'UN ENGAGEMENT IMPORTE, DANS LES MOTS DU MOTEUR (`Importe::paie`) : ce que les pieces de l'ancien logiciel
/// disent de ce qui a ete verse, et rien de calcule. Un montant qu'elles taisent est vide (`None`) : non dit, jamais zero.
#[derive(Debug, Clone)]
pub struct PaieImportee {
    /// Le jour de l'import.
    pub le: Jour,
    pub engagement: crate::Engagement,
    pub salarie: droit::fiche::SalarieDuContrat,
    /// La fiche du spectacle dont le numero d'objet est celui que les pieces disent.
    pub spectacle: Option<droit::fiche::Spectacle>,
    /// Les lignes de remuneration que le bulletin imprime, dans son ordre (a defaut, le brut que la DSN declare), chacune
    /// avec sa ligne de remuneration quand l'import l'a rattachee (`LigneImprimee::rattachee`).
    pub remuneration: Vec<(LigneImprimee, Option<crate::calcul::LigneRattachee>)>,
    /// Les heures pour les annexes 8 et 10 (un cachet compte pour les heures que sa rubrique declare) : la somme de celles
    /// des lignes rattachees, dans sa forme la plus courte ; `None` : aucune ligne n'est rattachee.
    pub heures_annexes: Option<Heures>,
    /// Les lignes de cotisation que le bulletin imprime, sous la cotisation que l'import leur a donnee, chaque taux dans
    /// sa forme la plus courte ; vide quand aucun bulletin ne les donne.
    pub lignes: Vec<CotisationImprimee>,
    /// Les bases que les pieces disent, la ou le bareme de la paie les range, chacune dans la grandeur qu'elles disent
    /// (`BaseDeCotisation::imprimee`, une base du bloc 78 en euros).
    pub bases: BTreeMap<Assiette, crate::calcul::BaseDeCotisation>,
    pub brut: Option<Montant>,
    /// Le plafond applique : la base plafonnee que les pieces disent, quand elle est sous le brut.
    pub plafond: Option<Montant>,
    pub total_sal: Option<Montant>,
    pub total_pat: Option<Montant>,
    /// Le net avant impot : celui que le bulletin imprime, sinon le net paye ou verse plus l'impot preleve.
    pub net_a_payer: Option<Montant>,
    pub net_social: Option<Montant>,
    pub net_imposable: Option<Montant>,
    pub cout_employeur: Option<Montant>,
    pub numero_bulletin: Option<String>,
    /// Le prelevement a la source que les pieces disent ; `None` : elles n'en disent pas.
    pub pas: Option<PasImporte>,
}

/// Le prelevement a la source d'une paie importee : l'impot preleve, et le taux, son type et son identifiant que les
/// pieces disent (S21.G00.50.006 a .009). Elles ne disent ni l'abattement ni le montant soumis.
#[derive(Debug, Clone, PartialEq)]
pub struct PasImporte {
    pub taux: Option<droit::pas::TauxDePas>,
    pub type_taux: Option<droit::type_taux_pas::TypeDeTauxPas>,
    pub identifiant: Option<droit::pas::IdentifiantDuTauxPas>,
    pub prelevement: Montant,
}

impl Importe {
    /// LA PAIE DE `mois`, DANS LES MOTS DU MOTEUR (`PaieImportee`) : ce que les pieces de l'ancien logiciel disent de ce
    /// qui a ete verse, range comme le calcul range le sien, et rien de calcule. L'engagement porte l'identite importee
    /// (salarie, bornes, numero, nature : `nature_rangee`), le spectacle dont la fiche a son numero d'objet,
    /// l'emploi que le bulletin imprime, les lignes de remuneration lues et le jour du versement. Les lignes de
    /// cotisation sont celles du bulletin, sous la cotisation que l'import leur a donnee (`cotisation`).
    ///
    /// LES BASES se rangent ou le bareme de la paie les range : la base d'une ligne de cotisation du bulletin sous la
    /// base que le bareme donne a cette cotisation (`rubriques_bulletin`, `base`) ; une base que la DSN deposee declare
    /// (bloc 78, son code) sous celle que le bareme y declare (`cotisations_dsn`, `base78`), quand il n'en declare
    /// qu'une. Le bulletin passe devant la DSN, comme l'import le fait pour le brut et les nets.
    ///
    /// Ce que les pieces ne disent pas est vide.
    pub fn paie(&self, ctx: &Contexte, mois: Mois) -> R<PaieImportee> {
        let p = self.paies().get(&mois).ok_or_else(|| socle::refus!("{} (rang {}) : aucune paie importee en {}", self.objet.fichier, self.objet.rang, mois))?;
        let contrat = self.contrat();

        let nature = self.nature_rangee()?;
        let nat = nature.proprietes()?;
        let ctx = ctx.du_mois(mois)?;
        let cle = self.carte().salarie.as_ref().ok_or_else(|| socle::fichiers::absent("salarie"))?;
        let sal = crate::calcul::convention_de_l_employeur(&ctx, crate::fiche_du_salarie(&ctx, cle)?);
        let objet = p.numero_objet.as_ref().or_else(|| contrat.and_then(|c| c.numero_objet.as_ref()));
        let spectacle = objet.and_then(|o| ctx.spectacles.iter().find(|(_, f)| f.numero_objet.dite().is_some_and(|n| n.meme(o))).map(|(k, _)| k.clone()));
        let spec = match &spectacle {
            Some(k) => Some(crate::fiche_du_spectacle(&ctx, k)?),
            None => None,
        };
        // L'emploi que le bulletin imprime ; un bulletin qui ne l'imprime pas n'en dit rien.
        let emploi = match &p.emploi_imprime {
            Some(e) => e.clone(),
            None => droit::emplois::EmploiEngagement::default(),
        };
        let Some(contrat) = self.contrat_de_la_paie(p)? else {
            return Err(socle::refus!("{} (rang {}) : ni l'engagement ni la paie de {} ne disent le début du contrat", self.objet.fichier, self.objet.rang, mois));
        };
        let dates_travail: Vec<Jour> = p.dates.iter().flat_map(|d| d.jours()).collect();
        let brut = p.brut;
        // LES LIGNES DE REMUNERATION QUE LES PIECES DISENT : celles du bulletin ; a defaut, le brut que la DSN declare.
        let mut lignes_lues = p.lignes.clone();
        if lignes_lues.is_empty() {
            lignes_lues.push(LigneImprimee { libelle: "brut déclaré".into(), montant: brut, soumis: true, rubrique: None, paiement: None, heures: None });
        }
        // La quotite que la DSN declare (40.013) est, pour un salarie au mois, ses heures par mois (`quotite_contrat`) ; pour
        // un contrat d'usage, ses jours, que les jours du contrat disent (`Engagement::quotite`).
        let quotite = match p.quotite.filter(|_| nat.mensuelle()) {
            Some(droit::natures::QuotiteDuContrat::Heures(h)) => Some(socle::grandeurs::courte(h)),
            Some(droit::natures::QuotiteDuContrat::Jours(_)) | None => None,
        };
        let mut engagement = crate::Engagement {
            salarie: cle.clone(),
            nom: crate::qui_de(&sal)?,
            spectacle: spectacle.clone(),
            dates_travail,
            // Ses lignes de remuneration sont celles du bulletin (`remuneration`) : il n'a aucune ligne saisie.
            lignes: Vec::new(),
            contrat,
            numero: self.numero_de_la_paie(p),
            mois,
            planning: Vec::new(),
            planning_lieux: BTreeMap::new(),
            jours: Jours::ZERO,
            jours_contrat: Jours::ZERO,
            date_paiement: p.verse_le,
            versement: None,
            taux_cachet: None,
            cachets: socle::grandeurs::Cachets::ZERO,
            heures: None,
            lieu: String::new(),
            lieux: BTreeMap::new(),
            emploi,
            etablissement: None,
            nature,
            quotite_contrat: quotite,
            quotite_reference_saisie: None,
            modalite_temps_travail_saisie: None,
            motif_recours: self.motif_recours(),
            motif_rupture: None,
            notification_rupture: None,
            preavis: None,
            dernier_jour_paye: None,
            date_modification: None,
            transaction: None,
            suspensions: Vec::new(),
            avant: None,
            periode: None,
            sans_remuneration: None,
            ecrit: crate::ContratEcrit::default(),
        };
        // Chaque ligne imprimee, avec sa ligne de remuneration quand elle est rattachee (`rattachee`).
        let mut remuneration = Vec::new();
        for l in lignes_lues {
            let rattachee = l.rattachee(&ctx)?;
            remuneration.push((l, rattachee));
        }
        let rattachees: Vec<&crate::calcul::LigneRattachee> = remuneration.iter().filter_map(|(_, r)| r.as_ref()).collect();
        // Ses cachets et ses heures, comme ceux d'un engagement calcule (`cachets_et_heures`), des lignes rattachees.
        (engagement.cachets, engagement.heures) = crate::cachets_et_heures(rattachees.iter().map(|r| (r.paiement.and_then(crate::ligne::PaiementDeLigne::quantite), r.heures_annexes)), nat.mensuelle());
        let heures_annexes = (!rattachees.is_empty()).then(|| socle::grandeurs::courte(rattachees.iter().map(|r| r.heures_annexes).sum()));
        engagement.jours = Jours::de_dates(engagement.dates_travail.len());
        engagement.jours_contrat = engagement.jours;

        // Le bareme de la paie range les bases ; ses taux se lisent sur les pieces (`calcul::bareme_pour_ranger`).
        let fin_periode = engagement.periode_payee()?.fin();
        let caisse = droit::caisse_conges::affilie(&nat, &sal, &engagement.contrat())?;
        let bar = crate::calcul::bareme_pour_ranger(&ctx, &sal, &nat, &fin_periode, caisse, droit::majoration_chomage::due(&nat, engagement.motif_recours, &engagement.contrat())?)?;
        // Un regroupement n'est pas une rubrique du bareme : sa base ne se range pas.
        fn de_cotisation(l: &CotisationImprimee) -> Option<&droit::forme_bareme::IdDeCotisation> {
            match &l.cotisation {
                Some(crate::ecarts::RegroupementDeComparaison::Cotisation(id)) => Some(id),
                Some(_) | None => None,
            }
        }

        // Les lignes de cotisation du bulletin, sous leur cotisation ; une ligne que l'import n'a pas qualifiee garde
        // son libelle, sans cotisation.
        let taux = |x: Option<Taux>| x.map(socle::grandeurs::courte);
        let cotisations: Vec<CotisationImprimee> = p
            .cotisations
            .iter()
            .flatten()
            .map(|l| CotisationImprimee { taux_employeur: taux(l.taux_employeur), taux_salarie: taux(l.taux_salarie), ..l.clone() })
            .collect();

        let mut bases: BTreeMap<Assiette, crate::calcul::BaseDeCotisation> = BTreeMap::new();
        if let Some(b) = brut {
            bases.insert(Assiette::Brut, crate::calcul::BaseDeCotisation::Montant(b));
        }
        for (id, base) in p.cotisations.iter().flatten().filter_map(|l| de_cotisation(l).zip(l.base)).filter(|(_, b)| !b.est_nulle()) {
            if let Some(cle) = bar.rubrique(id).and_then(|r| r.base) {
                bases.entry(cle).or_insert(base);
            }
        }
        for (code, m) in &p.bases {
            let mut cles: Vec<Assiette> = bar.cotisations_dsn.iter().filter(|c| c.bloc78 == *code).map(|c| c.base78).filter(|k| *k != Assiette::Zero).collect();
            cles.sort();
            cles.dedup();
            if let [cle] = &cles[..] {
                if let std::collections::btree_map::Entry::Vacant(v) = bases.entry(*cle) {
                    // Une base du bloc 78 est un montant (78.004) : une assiette qui compte des heures la refuse.
                    v.insert(crate::calcul::BaseDeCotisation::calculee(*cle, crate::calcul::BaseDeCotisation::Montant(*m))?);
                }
            }
        }
        // Le plafond applique : la base plafonnee, quand elle est sous le brut.
        let plafonnee = [Assiette::BrutPlafonne, Assiette::T1Mensuelle].into_iter().find_map(|k| bases.get(&k).map(|t| t.en_euros(k))).transpose()?;
        let plafond = brut.zip(plafonnee).and_then(|(b, t)| (t < b).then_some(t));
        // Le net avant impot : celui que le bulletin imprime, sinon le net paye ou le net verse plus l'impot preleve.
        let n = &p.nets;
        let net_a_payer = p.net_avant_impot.or_else(|| p.net_a_payer.or(n.net_verse).zip(n.pas).map(|(net, pas)| net + pas));

        Ok(PaieImportee {
            le: self.le(),
            engagement,
            salarie: sal,
            spectacle: spec,
            remuneration,
            heures_annexes,
            lignes: cotisations,
            bases,
            brut,
            plafond,
            total_sal: p.cotisations_salariales,
            total_pat: p.cotisations_patronales,
            net_a_payer,
            net_social: p.net_social,
            net_imposable: n.net_imposable,
            cout_employeur: p.cout_employeur,
            numero_bulletin: p.bulletin.as_ref().and_then(|b| b.numero.clone()).filter(|n| !n.is_empty()),
            pas: n.pas.map(|prelevement| PasImporte {
                taux: n.taux_pas,
                type_taux: n.type_pas,
                identifiant: n.identifiant_pas.clone(),
                prelevement,
            }),
        })
    }
}

impl Importe {
    /// LE NUMERO DU CONTRAT QUE PAIE `p`, l'une de ses paies : celui de l'engagement, sinon celui que son bulletin
    /// imprime ; `None` : aucun des deux ne le dit.
    fn numero_de_la_paie(&self, p: &PaieVersee) -> Option<NumeroContrat> {
        self.carte().numero.clone().or_else(|| p.bulletin.as_ref().and_then(|b| b.contrat.clone()))
    }
}

impl PaieVersee {
    /// LA CLE DE LA REPRISE DES CUMULS DE L'ANNEE (`droit::fiche::RepriseDesCumuls`) : une paie reprise d'un ancien
    /// logiciel la donne, et elle seule ; ce que cet ancien logiciel avait cumule dans l'annee, ses pieces ne le disent
    /// pas, la fiche du salarie le dit (`cumuls_reprise`, lu par `conges::cumul_de_l_annee`).
    pub fn reprise_des_cumuls(&self) -> droit::fiche::RepriseDesCumuls {
        droit::fiche::RepriseDesCumuls::d_une_paie_reprise()
    }
}

/// UNE PAIE REPRISE DU SALARIE, TELLE QU'UN CALCUL D'UN AUTRE MOIS LA LIT (`reprises_du_salarie`) : ce qui a ete verse,
/// tel que les pieces le disent (`PaieVersee`), et le numero du contrat qu'elle paie (`Importe::numero_de_la_paie`).
/// Rien ne s'y range ni ne s'y calcule : ni la nature du contrat, ni le bareme ou la convention de son mois.
#[derive(Debug, Clone, Serialize)]
pub struct PaieReprise {
    pub numero: Option<NumeroContrat>,
    pub paie: PaieVersee,
}

/// LES PAIES DU SALARIE `salarie` (la cle de sa fiche) EN `mois` REPRISES D'UN ANCIEN LOGICIEL, une par engagement
/// importe qui le paie ce mois, telles que leurs pieces les disent (`PaieReprise`). Le calcul qui les lit note leur
/// empreinte (`reprises <mois> <salarie>`, `empreinte_des_reprises`, par `etats::paies_du_salarie`). Un engagement
/// importe non actif (un contrat repris qui a continue ici, puis retire apres un envoi : `etats::desactiver`) y est
/// comme les autres : ce que l'ancien logiciel a verse l'a ete. Un engagement importe qui ne dit pas son salarie n'est
/// celui de personne : il ne fait pas refuser les calculs des autres salaries (ce qui lit ses paies et son contrat pour
/// eux-memes le refuse, `cle_ecrite`).
pub fn reprises_du_salarie(racine: &Path, mois: Mois, salarie: &IdSalarie) -> R<Vec<PaieReprise>> {
    let mut out = Vec::new();
    for i in du_mois(racine, mois)?.into_iter().filter(|i| i.objet.carte().salarie.as_ref() == Some(salarie)) {
        if let Some(p) = i.paies().get(&mois) {
            out.push(PaieReprise { numero: i.numero_de_la_paie(p), paie: p.clone() });
        }
    }
    Ok(out)
}

/// LA CLE DE LA REPRISE DES CUMULS QUE LA FICHE DU SALARIE `salarie` (la cle de sa fiche) MONTRE
/// (`routes::fiches::montres`) : une paie reprise de lui (celles que les calculs lisent, `reprises_du_salarie` : d'un
/// engagement importe actif ou non), d'une annee civile qu'un engagement d'ici, actif, touche aussi. Les cumuls de
/// l'annee d'une paie de cet engagement lisent alors cette paie reprise, et la reprise des cumuls de la fiche quand elle
/// ne dit pas ce qu'ils comptent (`conges::cumul_de_l_annee`). `None` : aucun calcul ne la lit.
pub fn reprise_des_cumuls_de(racine: &Path, salarie: &IdSalarie) -> R<Option<droit::fiche::RepriseDesCumuls>> {
    let objets = stock::objets(racine)?;
    let siens = objets.iter().filter(|o| o.carte().salarie.as_ref() == Some(salarie));
    let mut reprises: Vec<(socle::mois::Annee, &PaieVersee)> = Vec::new();
    let mut ici: Vec<(socle::mois::Annee, Option<socle::mois::Annee>)> = Vec::new();
    for o in siens {
        if let Some(p) = o.pieces() {
            reprises.extend(p.paies.iter().map(|(m, paie)| (m.annee(), paie)));
        }
        // Un engagement non actif ne touche plus aucun mois d'ici (`stock::Objet::couvre`).
        if !o.importe_seul() && !o.est_non_actif() {
            ici.push((o.premier_mois().annee(), o.dernier_mois()?.map(Mois::annee)));
        }
    }
    let touchee = |an: socle::mois::Annee| ici.iter().any(|(premier, dernier)| *premier <= an && dernier.is_none_or(|d| an <= d));
    Ok(reprises.into_iter().find(|(an, _)| touchee(*an)).map(|(_, p)| p.reprise_des_cumuls()))
}

/// L'empreinte des paies reprises du salarie `salarie` en `mois` telles qu'un calcul les lit (`reprises_du_salarie`) :
/// un reimport, un bulletin qui complete une paie de DSN, une ligne rattachee la changent, et la paie validee qui les a
/// lues se recalcule.
pub fn empreinte_des_reprises(racine: &Path, mois: Mois, salarie: &IdSalarie) -> R<socle::fichiers::Empreinte> {
    Ok(empreinte_de(&reprises_du_salarie(racine, mois, salarie)?))
}

/// L'empreinte de paies reprises lues (`empreinte_des_reprises`).
pub fn empreinte_de(reprises: &[PaieReprise]) -> socle::fichiers::Empreinte {
    socle::fichiers::Empreinte::des_octets(socle::ecriture::json_en_ligne(&reprises).as_bytes())
}

/// LES PAIES IMPORTEES DE `mois`, dans les mots du moteur (`Importe::paie`), une par engagement importe paye ce mois, et
/// celles qui ne s'y rangent pas encore (`NatureADire`). REGARDER N'EST JAMAIS REFUSE (« Le logiciel est doux ») : un
/// contrat repris dont la nature reste a dire ne ferme pas la lecture des autres ; ce qui lit les paies du mois les range
/// et dit, sur ce contrat-la, ce qui lui manque.
pub fn paies_du_mois(ctx: &Contexte, mois: Mois) -> R<(Vec<PaieImportee>, Vec<NatureADire>)> {
    let (mut rangees, mut a_dire) = (Vec::new(), Vec::new());
    for i in du_mois(&ctx.racine, mois)?.iter().filter(|i| i.paies().contains_key(&mois)) {
        match i.a_dire() {
            true => a_dire.push(NatureADire { mois, qui: crate::etats::qui_de(ctx, &i.objet) }),
            false => rangees.push(i.paie(ctx, mois)?),
        }
    }
    Ok((rangees, a_dire))
}

/// UNE PAIE REPRISE QUI NE SE RANGE PAS ENCORE : celle de `mois` d'un contrat dont la nature reste a dire
/// (`Importe::a_dire`). Une nature ne se devine pas (regle 6) : ce qui a ete verse ne se range sous aucune
/// (`Importe::nature_rangee`), et ce qui lit les paies du mois la dit sur ce contrat (`phrase`), sans refuser les autres.
#[derive(Debug, Clone, PartialEq)]
pub struct NatureADire {
    pub mois: Mois,
    /// Le contrat, tel qu'une phrase le nomme (`etats::qui_de`).
    pub qui: String,
}

impl NatureADire {
    /// Ce que la lecture en dit.
    pub fn phrase(&self) -> String {
        format!(
            "{} : sa paie reprise de {} n'est pas comptée, la nature de ce contrat reste à dire (Configuration, « Reprendre l'historique »).",
            self.qui,
            self.mois.libelle(socle::mois::Graphie::Accentuee)
        )
    }
}

/// Tous les engagements importes, fichier par fichier.
pub fn tous(racine: &Path) -> R<Vec<Importe>> {
    Ok(stock::objets(racine)?.into_iter().map(Importe::de).collect::<R<Vec<_>>>()?.into_iter().flatten().collect())
}

/// Les engagements importes qui touchent `mois`.
pub fn du_mois(racine: &Path, mois: Mois) -> R<Vec<Importe>> {
    Ok(tous(racine)?.into_iter().filter(|i| i.mois().contains(&mois)).collect())
}

/// Les contrats attestes de tous les engagements importes (`Importe::contrat_lu`).
pub fn contrats_lus(racine: &Path) -> R<Vec<ContratRelu>> {
    Ok(tous(racine)?.iter().map(Importe::contrat_lu).collect::<R<Vec<_>>>()?.into_iter().flatten().collect())
}

/// Les paies versees de tous les engagements importes, une par mois (`Importe::paies_lues`).
pub fn paies_lues(racine: &Path) -> R<Vec<PaieRelue>> {
    Ok(tous(racine)?.iter().map(Importe::paies_lues).collect::<R<Vec<_>>>()?.into_iter().flatten().collect())
}

/// LA DSN DE `mois` QUE L'ANCIEN LOGICIEL A DEPOSEE : une paie importee de ce mois qu'une DSN deposee declare
/// (`CleDePaieVersee::Dsn` : son `depose_le`, son `fichier`), qu'une saisie touche aussi le mois ou non. Cette DSN n'est
/// pas faite ici et ne se refait pas ici (regle 4) : ce que la saisie ajoute a ce mois se declare dans une DSN suivante,
/// comme un contrat ajoute apres la DSN de son mois.
pub fn dsn_deposee(racine: &Path, mois: Mois) -> R<Option<PaieRelue>> {
    let trouve = tous(racine)?.iter().find_map(|i| i.paies().get(&mois).filter(|p| !p.contrat.d_un_bulletin()).map(|p| i.paie_lue(mois, p)));
    trouve.transpose()
}

/// L'engagement importe du salarie `cle` (la cle de sa fiche) qui commence le `debut` et touche `mois`, s'il y en a un.
pub fn celui_de(racine: &Path, mois: Mois, cle: &IdSalarie, debut: Jour) -> R<Option<Importe>> {
    Ok(du_mois(racine, mois)?.into_iter().find(|i| i.carte().salarie.as_ref() == Some(cle) && i.carte().debut == Some(debut)))
}

/// Les contrats d'usage importes qui continuent en `mois` (`Importe::continue_apres`) : verses avant lui par l'ancien
/// logiciel, ils finissent en `mois` ou apres.
pub fn qui_continuent(racine: &Path, mois: Mois) -> R<Vec<Importe>> {
    Ok(tous(racine)?
        .into_iter()
        .filter(|i| i.continue_apres().is_some_and(|d| d < mois) && i.carte().fin.is_some_and(|f| f.mois_civil() >= mois))
        .collect())
}

/// Le contrat importe que l'engagement `e` de la saisie continue : meme salarie, meme debut, meme numero, et il continue
/// apres son dernier mois verse (`continue_apres`).
pub fn continue_par(racine: &Path, e: &Carte) -> R<Option<Importe>> {
    let (Some(debut), Some(numero), Some(salarie)) = (e.debut, e.numero.as_ref(), &e.salarie) else { return Ok(None) };
    Ok(tous(racine)?.into_iter().find(|i| {
        i.continue_apres().is_some() && i.carte().salarie.as_ref() == Some(salarie) && i.carte().debut == Some(debut) && i.carte().numero.as_ref() == Some(numero)
    }))
}

/// LE CONTRAT `numero` DE `salarie` COMMENCE LE `debut` VIENT-IL DE L'ANCIEN LOGICIEL ? Un engagement importe porte ces
/// salarie, debut et numero : c'est lui, ou le contrat d'usage qu'il continue ici (`continue_par`). L'ecran le dit sur
/// la carte. Faux sans numero.
pub fn vient_d_un_ancien_logiciel(racine: &Path, salarie: &IdSalarie, debut: Jour, numero: Option<&NumeroContrat>) -> R<bool> {
    let vient = |numero: &NumeroContrat| -> R<bool> {
        Ok(tous(racine)?.iter().any(|i| {
            i.carte().salarie.as_ref() == Some(salarie) && i.carte().debut == Some(debut) && i.carte().numero.as_ref() == Some(numero)
        }))
    };
    Ok(numero.map(vient).transpose()?.is_some_and(|oui| oui))
}

/// LE NUMERO QU'UNE DSN DE L'ANCIEN LOGICIEL A DECLARE pour l'engagement `o` : celui de sa derniere paie importee
/// qu'une DSN deposee declare (`CleDePaieVersee::Dsn`, S21.G00.40.009). Cette DSN est partie : le numero est lu, il ne
/// se dit plus et ne change plus (`numeros::a_la_validation`). `None` pour un engagement d'ici, ou repris des seuls
/// bulletins.
pub fn numero_declare(o: &Objet) -> R<Option<NumeroContrat>> {
    let Some(pieces) = o.pieces() else { return Ok(None) };
    Ok(pieces.paies.values().rev().find_map(|p| p.contrat.numero_declare()).cloned())
}

/// Une DSN de l'ancien logiciel a-t-elle declare le numero du contrat de `salarie` commence le `debut`
/// (`numero_declare`, lu sur l'engagement repris qui le porte) ?
pub fn numero_declare_du_contrat(racine: &Path, salarie: &IdSalarie, debut: Jour) -> R<bool> {
    for i in tous(racine)?.iter().filter(|i| i.carte().salarie.as_ref() == Some(salarie) && i.carte().debut == Some(debut)) {
        if numero_declare(&i.objet)?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Importe {
    /// LES JOURS DU CONTRAT ENTIER (S21.G00.40.013 d'un contrat d'usage : `Engagement::quotite`) d'un contrat qui continue
    /// ici, `propres` etant ceux que la saisie lui donne : les jours que les bulletins importes impriment, plus les siens ;
    /// a defaut d'un bulletin pour chaque mois verse, la quotite que la DSN deposee a declaree pour ce contrat (elle le
    /// dit pour le contrat entier, `norme-dsn/texte/bloc-S21.G00.40.txt`). `None` quand aucune piece ne les donne.
    pub fn jours_du_contrat_continue(&self, propres: usize) -> Option<Jours> {
        let jours = |p: &PaieVersee| -> Option<usize> {
            let n: usize = p.dates.iter().map(|d| d.jours().count()).sum();
            (n > 0).then_some(n)
        };
        let lus: Option<Vec<usize>> = self.paies().values().map(jours).collect();
        if let Some(l) = lus.filter(|l| !l.is_empty()) {
            return Some(Jours::de_dates(l.iter().sum::<usize>() + propres));
        }
        // La quotite que la DSN deposee declare (S21.G00.40.013) en journees : des jours entiers.
        let dernier = self.paies().values().last()?;
        match dernier.quotite? {
            droit::natures::QuotiteDuContrat::Jours(j) => Some(j).filter(|q| *q > Jours::ZERO).and_then(socle::grandeurs::entiere),
            droit::natures::QuotiteDuContrat::Heures(_) => None,
        }
    }
}

/// Le refus d'un geste sur un engagement importe : il se lit, il ne se modifie pas.
pub fn refus(qui: &str, debut: Jour) -> String {
    let jour = debut.jj_mm_aaaa();
    format!(
        "L'engagement de {} du {} est importé : il se lit tel que l'ancien logiciel l'a déclaré, il ne se modifie pas, ne se \
         recalcule pas et ne se supprime pas. Rien n'est écrit.",
        qui, jour
    )
}

// ------------------------------------------------------------------ l'ecriture

/// UN ENGAGEMENT IMPORTE A ECRIRE : l'engagement lui-meme (son salarie, ses bornes, son numero, ce qui a ete dit de lui :
/// la carte de son premier mois), et ses pieces.
#[derive(Debug, Clone)]
pub struct Neuf {
    pub carte: Carte,
    pub pieces: Pieces,
}

/// LE CONTRAT DE PERMANENT QUI CONTINUE ICI : sa nature est dite et payee au mois, il a ete verse le dernier mois repris
/// (`dernier`), et il ne finit pas avant le mois qui suit.
fn continue_ici(carte: &Carte, pieces: &Pieces, dernier: Mois) -> R<bool> {
    let mensuel = carte.nature.map(|n| n.proprietes().map(|x| x.mensuelle())).transpose()?.is_some_and(|m| m);
    let premier_jour_suivant = dernier.suivant().premier_jour();
    Ok(mensuel && pieces.paies.contains_key(&dernier) && carte.fin.is_none_or(|f| f >= premier_jour_suivant))
}

/// LE MOIS D'ENTREE D'UN ENGAGEMENT IMPORTE, le fichier qui le porte (`ecrire_tous`), `dernier` etant le dernier mois
/// repris.
fn mois_d_entree(carte: &Carte, pieces: &Pieces, dernier: Option<Mois>) -> R<Mois> {
    Ok(match (dernier, carte.debut) {
        (Some(d), _) if continue_ici(carte, pieces, d)? => d.suivant(),
        (_, Some(premier)) => premier.mois_civil(),
        _ => pieces.paies.keys().next().copied().ok_or_else(|| socle::echec::refus("un engagement importe sans debut ni paie"))?,
    })
}

/// ECRIT LES ENGAGEMENTS IMPORTES, tous : chacun dans le fichier de son mois d'entree -- le mois ou il commence (son
/// `debut`, sinon le premier mois ou il a ete verse), ou, pour le contrat de permanent qui continue ici (`continue_ici`),
/// le mois qui suit le dernier mois repris : son premier mois neuf. Ils vont apres les engagements de la saisie. Ceux qui
/// y etaient s'en vont ; rien d'autre ne change. LE MEME CONTRAT DEJA ECRIT (meme salarie, un mois verse en commun) garde
/// ce qui a ete dit de lui et ce que la saisie lui a ecrit ; seules ses pieces se remplacent (ce qu'une DSN qui arrive
/// declare autrement se remplace ensuite, et se dit : `routes::importation::nature::la_dsn_fait_foi`). Rend les fichiers
/// ecrits.
pub fn ecrire_tous(racine: &Path, importes: Vec<Neuf>) -> R<Vec<PathBuf>> {
    let existants = tous(racine)?;
    let dernier = importes.iter().flat_map(|e| e.pieces.paies.keys().copied()).max();
    let mut repris = vec![false; existants.len()];
    let mut par_mois: BTreeMap<Mois, Vec<stock::Ecrit>> = BTreeMap::new();
    for mut e in importes {
        let salarie = &e.carte.salarie;
        let meme = (0..existants.len()).find(|&k| {
            let x = &existants[k];
            !repris[k] && x.carte().salarie == *salarie && x.paies().keys().any(|m| e.pieces.paies.contains_key(m))
        });
        let mut ecrit = stock::Ecrit { saisie: stock::Saisie { carte: e.carte.clone(), ..Default::default() }, ..Default::default() };
        if let Some(k) = meme {
            repris[k] = true;
            let x = &existants[k].objet.brut;
            if e.carte.nature.is_none() && x.saisie.carte.nature.is_some() {
                ecrit = (**x).clone();
                e.carte = ecrit.saisie.carte.clone();
            }
        }
        let mois = mois_d_entree(&e.carte, &e.pieces, dernier)?;
        ecrit.importe = Some(e.pieces);
        par_mois.entry(mois).or_default().push(ecrit);
    }
    let mut touches: Vec<Mois> = existants.iter().map(|i| i.objet.fichier).collect();
    touches.extend(par_mois.keys().copied());
    touches.sort();
    touches.dedup();
    let mut ecrits = Vec::new();
    for f in touches {
        let existe = stock::lire_fichier(racine, f)?;
        // Un mois que l'import ne touche que pour en retirer n'a rien de neuf ; un fichier qui n'existe pas se cree.
        let neufs: Vec<stock::Ecrit> = par_mois.remove(&f).into_iter().flatten().collect();
        let mut contenu = match existe.clone() {
            Some(c) => c,
            None => stock::FichierDuMois { mois: f, taux_cachet: None, engagements: Vec::new() },
        };
        contenu.engagements.retain(|e| e.importe.is_none());
        contenu.engagements.extend(neufs);
        if existe.as_ref() == Some(&contenu) {
            continue;
        }
        if contenu.engagements.is_empty() && contenu.taux_cachet.is_none() {
            if existe.is_some() {
                ecrits.push(stock::retirer(racine, f)?);
            }
            continue;
        }
        ecrits.push(stock::poser(racine, &contenu)?);
    }
    Ok(ecrits)
}

/// CE QUE L'IMPORT COMPLETE DE LUI-MEME : ce que les pieces ne disaient pas et que la personne dit apres l'import (la
/// nature d'un contrat de permanent, son numero). `completer` recoit chaque engagement importe NON VALIDE (sa carte, a
/// changer, et ses pieces) ; un engagement valide a ete dit et arrete : sa carte ne se change plus ici. SEULE LA CARTE
/// CHANGE : sa saisie d'ici (avenants, faits des mois), ses versions, son numero donne restent ; un engagement dont la
/// carte ne change pas ne se reecrit pas. Celui dont le mois d'entree change avec sa carte (`mois_d_entree`) passe dans
/// le fichier de ce mois, a la place de son mois d'origine parmi les importes. Rend combien ont change.
pub fn completer(racine: &Path, mut completer: impl FnMut(&mut Carte, &Pieces) -> bool) -> R<usize> {
    let tous = tous(racine)?;
    let dernier = tous.iter().flat_map(|i| i.paies().keys().copied()).max();
    // (fichier, rang, mois d'entree, l'engagement tel qu'il s'ecrit)
    let mut changes: Vec<(Mois, usize, Mois, stock::Ecrit)> = Vec::new();
    for i in tous.iter().filter(|i| !i.objet.est_valide()) {
        let mut carte = i.carte().clone();
        if !completer(&mut carte, i.pieces()) || carte == *i.carte() {
            continue;
        }
        let mois = mois_d_entree(&carte, i.pieces(), dernier)?;
        let mut ecrit = (*i.objet.brut).clone();
        // Un numero que la carte recoit n'est plus celui que le moteur avait donne (`numero_pour`) : il se lit, il ne se
        // reforme pas (regle 4).
        if carte.numero != i.carte().numero {
            ecrit.numero_pour = None;
        }
        ecrit.saisie.carte = carte;
        changes.push((i.objet.fichier, i.objet.rang, mois, ecrit));
    }
    let mut fichiers: Vec<Mois> = changes.iter().flat_map(|(f, _, m, _)| [*f, *m]).collect();
    fichiers.sort();
    fichiers.dedup();
    for f in fichiers {
        let mut contenu = match stock::lire_fichier(racine, f)? {
            Some(c) => c,
            None => stock::FichierDuMois { mois: f, taux_cachet: None, engagements: Vec::new() },
        };
        // Ceux qui restent dans ce fichier se remplacent a leur rang ; ceux qui en partent s'en vont (du dernier rang au
        // premier, pour que les rangs lus tiennent) ; ceux qui y arrivent vont apres.
        let mut partent: Vec<usize> = Vec::new();
        for (de, rang, vers, ecrit) in &changes {
            if *de != f {
                continue;
            }
            let place = contenu.engagements.get_mut(*rang).ok_or_else(|| socle::refus!("{} : l'engagement {} a disparu", f, rang + 1))?;
            if *vers == f {
                *place = ecrit.clone();
            } else {
                partent.push(*rang);
            }
        }
        partent.sort_unstable_by(|a, b| b.cmp(a));
        for rang in partent {
            contenu.engagements.remove(rang);
        }
        // L'ordre des engagements importes d'un fichier est celui de leur mois d'origine, puis de leur rang : ceux qui
        // viennent d'un mois d'avant se placent devant les importes deja la, ceux d'un mois d'apres, derriere. L'ordre ne
        // depend pas de celui ou la compagnie a repondu.
        let arrivent = |avant: bool| changes.iter().filter(move |(de, _, vers, _)| *vers == f && if avant { *de < f } else { *de > f }).map(|(_, _, _, e)| e.clone());
        let premier_importe = contenu.engagements.iter().take_while(|e| e.importe.is_none()).count();
        let derriere: Vec<stock::Ecrit> = arrivent(false).collect();
        contenu.engagements.splice(premier_importe..premier_importe, arrivent(true));
        contenu.engagements.extend(derriere);
        if contenu.engagements.is_empty() && contenu.taux_cachet.is_none() {
            stock::retirer(racine, f)?;
        } else {
            stock::poser(racine, &contenu)?;
        }
    }
    Ok(changes.len())
}
