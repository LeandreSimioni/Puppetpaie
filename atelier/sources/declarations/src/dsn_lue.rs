//! UNE DSN DEPOSEE, LUE UNE FOIS, EN SA FORME (`DsnLue`) : son en-tete (l'envoi S10, la declaration S20.G00.05), ses
//! montants en decimal, ses dates en jours, ses blocs tels que la norme les emboite (l'individu S21.G00.30, ses contrats
//! S21.G00.40, leur arret S21.G00.60 et leur fin S21.G00.62, ses versements S21.G00.50, et dans chacun ses remunerations
//! S21.G00.51, leurs activites S21.G00.53, ses bases S21.G00.78). Une seule fonction la lit (`lire`) ; l'import, le depot
//! et son apercu, les organismes simules, le controle des NTT et l'AER d'une fictive la prennent de la. Les DSN d'un
//! dossier se listent par leur contenu (`fichiers`). Lecture seule, sans reseau.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use droit::base_assujettie::BaseAssujettie;
use droit::fiche::{Bic, Civilite, CodeMedecine, CodeNaf, CodeNationalite, CodePostal, Courriel, DepartementNaissance, Iban, Idcc, Nic, Nir, NumeroObjetSpectacle, Sexe, Siren};
use droit::nature_declaration::{EssaiOuReel, NatureDeLaDeclaration};
use droit::natures::{Fraction, IdentifiantDeDeclaration, ListeDeLaNorme, NumeroDOrdre, RubriqueDeLaNorme, TypeDActivite, TypeDeRemuneration, UniteDeMesure, VersionDeLaNorme};
use droit::organismes::IdentifiantDOrganisme;
use droit::type_declaration::TypeDeLaDeclaration;
use engagement::numeros::NumeroContrat;

use socle::date::Jour;
use socle::echec::{erreur, refus, Classe, Echec, R};
use socle::fichiers::NomDeFichier;
use socle::mois::{Mois, MoisOuJour};
use socle::num::Decimal;
use socle::pays::CodePays;
use socle::texte::{blanc, lignes_du_texte, texte_latin1};

/// Ce qu'on dit d'un fichier qui ne dit pas le mois qu'il declare (`DsnLue::mois_declare`).
pub const SANS_MOIS: &str = "il ne porte pas de mois principal déclaré (S20.G00.05.005)";

/// Ce que porte une rubrique de SIREN (S10.G00.01.001, S21.G00.06.001, X [9,9]) et de NIC (S21.G00.06.002,
/// S21.G00.11.001, X [5,5]) : `droit::fiche::Siren`, `Nic`.
const SIREN: &str = "un SIREN (9 chiffres)";
const NIC: &str = "un NIC (5 chiffres)";
/// Ce que porte une rubrique d'IDCC (S21.G00.11.022, S21.G00.40.017, X [4,4]) : `droit::fiche::Idcc`.
const IDCC: &str = "un IDCC (4 chiffres)";
/// Ce que portent les rubriques du compte preleve (S21.G00.20.003, .004) : `droit::fiche::Bic`, `Iban`.
const BIC: &str = "un BIC (ISO 9362)";
const IBAN: &str = "un IBAN (ISO 13616) à la clé juste";
/// Ce que porte le courriel d'un contact (S20.G00.07.003) : `droit::fiche::Courriel`.
const COURRIEL: &str = "une adresse de courriel";
/// Ce que porte le numero d'objet d'un contrat (S21.G00.40.050, X [12,12]) : `droit::fiche::NumeroObjetSpectacle`.
const OBJET: &str = "un numéro d'objet (12 lettres ou chiffres)";
/// Ce que porte le numero d'un contrat (S21.G00.40.009, X [5,20]), et ce que le citent les remunerations (51.010) et les
/// bases (78.006) : `engagement::numeros::NumeroContrat`.
const NUMERO: &str = "un numéro de contrat (5 à 20 caractères)";
/// Ce que porte l'identifiant d'un organisme (S21.G00.15.002, S21.G00.20.001, X [2,14]) :
/// `droit::organismes::IdentifiantDOrganisme`.
const ORGANISME: &str = "un identifiant d'organisme (2 à 14 caractères)";
/// Ce que porte le code emploi d'un contrat (S21.G00.40.005, X [2,6]) : `droit::fiche::CodeEmploiFt`.
const CODE_EMPLOI: &str = "un code emploi (2 à 6 caractères)";
/// S21.G00.40.004 : « X [4,4] » (cahier technique DSN 2026.1 p. 183, `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`).
const PCS_ESE: &str = "un code PCS-ESE (4 caractères)";

// ------------------------------------------------------------------ la forme

/// Le compte qui paie : le BIC et l'IBAN « du compte sur lequel doit etre preleve le montant de l'ordre de reglement »
/// (S21.G00.20.003, .004 ; cahier technique 2026.1, p. 156, `bloc-S21.G00.20.txt`), la `banque` de la fiche Entreprise
/// que le moteur y ecrit (`dsn.rs`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Banque {
    pub bic: Bic,
    pub iban: Iban,
}

/// Un versement a un organisme (bloc S21.G00.20) tel que la lecture le garde : l'identifiant de l'organisme (.001),
/// l'entite d'affectation (.002), le BIC et l'IBAN du compte preleve (.003, .004).
struct Bloc20 {
    organisme: Option<IdentifiantDOrganisme>,
    affectation: String,
    bic: Option<Bic>,
    iban: Option<Iban>,
}

/// Un contact chez le declare (S20.G00.07 : « nom et prenom » en une rubrique, .001 ; telephone .002 ; courriel .003 ;
/// type .004 ; cahier technique 2026.1, p. 139, `bloc-S20.G00.07.txt` l. 63-84), tel que la fiche Entreprise le porte
/// (`declarant_dsn`). Son courriel se garde tel que la DSN le declare, capitales comprises : la casse de ce qui precede
/// l'arobase est a l'adresse (« The local-part of a mailbox MUST BE treated as case sensitive », RFC 5321, § 2.4 :
/// `ietf/ietf-rfc5321-simple-mail-transfer-protocol-20081001.txt`, l. 869).
#[derive(Clone, Debug, Serialize)]
pub struct Contact {
    pub nom_prenom: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Courriel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualite: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub civilite: Option<Civilite>,
}

/// CE QUE LA DSN DECLARE DE L'ENTREPRISE, sous les cles de sa fiche (`champs-base.json`) : les blocs 06 (l'entreprise),
/// 11 (l'etablissement) et 40, PAS l'en-tete S10.G00.01, qui est l'EMETTEUR de l'envoi -- « dans le cas d'un tiers
/// declarant ou d'un concentrateur, c'est l'identifiant SIREN de ce tiers qui doit figurer ici » (cahier technique
/// 2026.1, p. 130, `atelier/reference/norme-dsn/texte/bloc-S10.G00.01.txt`) : un cabinet qui depose y met son nom et son
/// adresse. Le Siren et l'APEN se lisent en S21.G00.06 (p. 141-142, `bloc-S21.G00.06.txt`) ; le NIC, l'adresse et l'APET
/// de l'etablissement, en S21.G00.11 (p. 144-145, `bloc-S21.G00.11.txt`) -- la fiche ne porte qu'un etablissement, celui
/// de son NIC, que le moteur declare en 06.002 et en 11.001 (`dsn.rs`) ; l'adresse du siege (06.004-006) et son NIC
/// (06.002) ne servent qu'a defaut. La DSN ne declare pas la raison sociale de l'entreprise : celle de l'emetteur
/// (S10.G00.01.003) n'est reprise que si l'emetteur EST l'entreprise (meme Siren en S10.G00.01.001 et en S21.G00.06.001)
/// -- une inference : la norme ne dit pas que l'entreprise qui depose elle-meme y met son nom, mais l'emetteur « ayant
/// elabore le present envoi » est alors elle. L'IDCC de la fiche est la convention principale de l'etablissement
/// (S21.G00.11.022, p. 147, `bloc-S21.G00.11.txt`), qui vient avant tout contrat dans le fichier ; l'IDCC d'un contrat
/// (S21.G00.40.017) n'en tient lieu qu'a defaut : il en differe pour la 1734 (`droit::minima::idcc_du_contrat`). Le code
/// convention DSN de la fiche, le moteur le tire de l'IDCC (`routes::fiches::deriver`).
#[derive(Clone, Debug, Default, Serialize)]
pub struct EntrepriseDeclaree {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub siren: Option<Siren>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub naf: Option<CodeNaf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nic: Option<Nic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adresse: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_postal: Option<CodePostal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commune: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idcc: Option<Idcc>,
    /// La DSN declare « 97 puis le numero d'adherent » (S21.G00.40.022, `conges_spectacles::code_dsn`) : la fiche ne
    /// garde que le numero d'adherent, et seulement s'il a la forme que dit la fiche d'Audiens. La cle est
    /// `documents::conges_spectacles::CLE_ADHERENT`.
    #[serde(rename = "congesspectacles_adherent", skip_serializing_if = "Option::is_none")]
    pub adherent_conges_spectacles: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raison_sociale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banque: Option<Banque>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub france_travail_dossier_recouvrement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declarant_dsn: Option<Contact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medecine_travail: Option<CodeMedecine>,
    /// La licence du premier contrat (S21.G00.40.049) : la fiche la date (`LectureDsn::licences` du moteur).
    #[serde(skip)]
    pub licence: Option<String>,
}

/// Ce que l'emploi d'un contrat (bloc S21.G00.40) dit du salarie, sous les cles de sa fiche.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EmploiDeclare {
    /// S21.G00.40.002.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statut_conventionnel: Option<droit::fiche::StatutConventionnel>,
    /// S21.G00.40.003.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statut_categoriel_rc: Option<droit::fiche::StatutCategorielRc>,
    /// S21.G00.40.004.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pcs_ese: Option<droit::fiche::CodePcsEse>,
    /// S21.G00.40.005.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_emploi_ft: Option<droit::fiche::CodeEmploiFt>,
    /// S21.G00.40.006.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emploi: Option<String>,
}

impl EmploiDeclare {
    /// Une valeur que la NATURE du contrat fixe (`contrats-base.json`, `codes` : « 999SPT » en 40.005 pour tout permanent,
    /// CCH-18 du bloc 40) ne dit rien du salarie : elle ne passe pas dans sa fiche. `999SPT` n'est pas un metier, et une
    /// DSN de PuppetPaie reimportee le donnerait comme `code_emploi_ft` d'un permanent.
    pub fn du_salarie(&self) -> R<EmploiDeclare> {
        let natures = droit::natures::natures()?;
        fn garder<T: AsRef<str> + Clone>(natures: &[std::sync::Arc<droit::natures::Nature>], rubrique: RubriqueDeLaNorme, v: &Option<T>) -> Option<T> {
            v.clone().filter(|v| !natures.iter().any(|nat| nat.code(rubrique).as_deref() == Some(v.as_ref())))
        }
        Ok(EmploiDeclare {
            statut_conventionnel: garder(&natures, droit::rubrique!("S21.G00.40.002"), &self.statut_conventionnel),
            statut_categoriel_rc: garder(&natures, droit::rubrique!("S21.G00.40.003"), &self.statut_categoriel_rc),
            pcs_ese: garder(&natures, droit::rubrique!("S21.G00.40.004"), &self.pcs_ese),
            code_emploi_ft: garder(&natures, droit::rubrique!("S21.G00.40.005"), &self.code_emploi_ft),
            emploi: garder(&natures, droit::rubrique!("S21.G00.40.006"), &self.emploi),
        })
    }
}

/// UN INDIVIDU (bloc S21.G00.30), sous les cles de la fiche salarie. Le sexe (30.005, « 01 - masculin », « 02 -
/// féminin ») devient le « 1 » ou « 2 » de la fiche (`sexe_de_la_fiche`) : la norme ne le fait porter que s'il differe
/// de celui du NIR ou si le NIR manque (cahier technique 2026.1, p. 171, `atelier/reference/norme-dsn/texte/
/// bloc-S21.G00.30.txt`, l. 154-163). Le nom d'usage (30.003, meme bloc, l. 138-144) n'a pas de case dans la fiche
/// salarie (`champs-base.json`) : il n'est pas lu. La date de naissance n'est pas un jour : la norme admet un jour ou un
/// mois inconnu (« 00MMAAAA ou 99MMAAAA », « JJ00AAAA ou JJ99AAAA », p. 172, l. 175-177) ; elle passe a la fiche en
/// AAAA-MM-JJ, que la fiche controle.
#[derive(Clone, Debug, Serialize)]
pub struct IndividuDeclare {
    /// S21.G00.30.001, absent quand l'individu commence a son nom ou que la DSN l'ecrit hors de sa forme
    /// (`Nir::d_une_piece`).
    #[serde(skip)]
    pub nir: Option<Nir>,
    /// .002.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nom: Option<String>,
    /// .004.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prenom: Option<String>,
    /// .005.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sexe: Option<Sexe>,
    /// .006.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_naissance: Option<NaissanceDeclaree>,
    /// .007.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lieu_naissance: Option<String>,
    /// .008.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adresse: Option<String>,
    /// .009.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_postal: Option<CodePostal>,
    /// .010.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commune: Option<String>,
    /// .013.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_nationalite_dsn: Option<CodeNationalite>,
    /// .014.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub departement_naissance: Option<DepartementNaissance>,
    /// .015.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pays_naissance: Option<CodePays>,
    /// .018.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Courriel>,
    /// .019.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_salarie: Option<droit::fiche::Matricule>,
    /// .020, le numero technique temporaire d'un individu declare sans NIR (`ntt`) : la fiche n'en a pas.
    #[serde(skip)]
    pub ntt: Option<crate::ntt::Ntt>,
}

/// Un contrat (bloc S21.G00.40) de l'individu qui le precede, son arret (bloc S21.G00.60) et sa fin (bloc S21.G00.62)
/// s'il les declare.
#[derive(Clone, Debug)]
pub struct ContratDeclare {
    pub nir: Option<Nir>,
    /// Le rang de son individu dans `DsnLue::individus` : un individu sans NIR n'a que lui.
    pub individu: usize,
    /// .001.
    pub debut: Option<Jour>,
    pub emploi: EmploiDeclare,
    /// .007, la nature du contrat.
    pub nature_dsn: Option<String>,
    /// .009.
    pub numero: Option<NumeroContrat>,
    /// .010.
    pub fin: Option<Jour>,
    /// .012, la quotite de reference de la categorie ; .013, celle du contrat ; .014, la modalite du temps de travail.
    pub quotite_reference: Option<Decimal>,
    pub quotite: Option<Decimal>,
    pub modalite: Option<droit::natures::ModaliteDuTempsDeTravail>,
    /// .017, la convention du contrat.
    pub idcc: Option<Idcc>,
    /// .021, le motif de recours.
    pub motif_recours: Option<droit::natures::MotifDeRecours>,
    /// .022, la caisse de conges payes, telle que la DSN l'ecrit.
    pub caisse_conges: Option<String>,
    /// .040 et .043 : le code risque et le taux AT/MP (`bloc-S21.G00.40.txt` l. 1224-1315).
    pub code_risque: Option<droit::bareme::CodeRisqueAt>,
    pub taux_at: Option<Decimal>,
    /// .049, la licence.
    pub licence: Option<String>,
    /// .050, le numero d'objet spectacle.
    pub objet: Option<NumeroObjetSpectacle>,
    /// S21.G00.60.002 : le dernier jour travaille avant l'arret que la DSN declare.
    pub dernier_jour_travaille: Option<Jour>,
    /// S21.G00.62.001 et .002 : la fin que la DSN declare, et son motif.
    pub fin_declaree: Option<Jour>,
    pub motif_fin: Option<droit::natures::MotifDeRupture>,
}

/// Une activite (bloc S21.G00.53) de la remuneration qui la precede : son type (.001), sa mesure (.002), son unite (.003).
#[derive(Clone, Debug, Default)]
pub struct Activite {
    pub genre: Option<TypeDActivite>,
    pub mesure: Option<Decimal>,
    pub unite: Option<UniteDeMesure>,
}

/// UNE REMUNERATION (bloc S21.G00.51), « contrepartie de l'activite de l'individu dans le cadre du contrat »
/// (`norme-dsn/texte/bloc-S21.G00.51.txt` l. 22) : sa periode de paie (.001, « date de debut de periode a laquelle la
/// remuneration est rattachee », l. 38-40), LE CONTRAT QU'ELLE PAIE (.010, « identique a la valeur renseignee dans une
/// rubrique Numéro du contrat - S21.G00.40.009 », CCH-11, l. 91-98), son type (.011), son montant (.013), ses activites.
#[derive(Clone, Debug, Default)]
pub struct Remuneration {
    pub debut: Option<Jour>,
    pub contrat: Option<NumeroContrat>,
    pub genre: Option<TypeDeRemuneration>,
    pub montant: Option<Decimal>,
    pub activites: Vec<Activite>,
}

impl Remuneration {
    /// Celle d'une periode anterieure au mois principal declare (S20.G00.05.005) est le rappel d'une paie deja declaree
    /// (chapitre 2, §2.1.3.6, `norme-dsn/texte/chapitre-2-principes-de-constitution.txt` l. 607-616), pas la paie du mois.
    pub fn rappel(&self, mois: Mois) -> bool {
        self.debut.is_some_and(|d| d < mois.premier_jour())
    }
}

/// UN VERSEMENT (bloc 50) et ce qui s'y rattache. Ses montants sont ceux du VERSEMENT, pas d'un contrat : le bloc 50 n'a
/// pas de numero de contrat, seules ses remunerations en portent un (S21.G00.51.010), et ses bases (bloc 78) ne le
/// portent que « dans le cas de la declaration de plusieurs blocs Contrat » (S21.G00.78.006, `bloc-S21.G00.79.txt`
/// l. 9-14). 50.004 est le montant net verse de la norme (la remuneration nette fiscale moins CSG/CRDS non deductible et
/// sante patronale, p. 244), pas le net a payer. Le taux du prelevement a la source, son type et son identifiant (50.006
/// a 50.008) se lisent avec lui.
#[derive(Clone, Debug)]
pub struct VersementLu {
    pub nir: Option<Nir>,
    /// .001.
    pub verse_le: Option<Jour>,
    /// .002.
    pub net_imposable: Option<Decimal>,
    /// .004.
    pub net_verse: Option<Decimal>,
    /// .006 a .008.
    pub taux_pas: Option<Decimal>,
    pub type_pas: Option<droit::type_taux_pas::TypeDeTauxPas>,
    pub identifiant_pas: Option<droit::pas::IdentifiantDuTauxPas>,
    /// .009.
    pub pas: Option<Decimal>,
    pub remunerations: Vec<Remuneration>,
    /// LES BASES ASSUJETTIES QUE LA DSN DECLARE (bloc 78 ; `norme-dsn/texte/bloc-S21.G00.78.txt`), dans l'ordre.
    pub bases: Vec<BaseLue>,
}

/// Une base assujettie (bloc S21.G00.78) : son code (.001), son montant (.004), et le contrat qu'elle nomme (.006, « dans
/// le cas de la declaration de plusieurs blocs Contrat », `bloc-S21.G00.79.txt` l. 9-14 ; « dans le cas de la declaration
/// d'un contrat unique, cette rubrique n'est pas a renseigner »).
#[derive(Clone, Debug)]
pub struct BaseLue {
    pub code: BaseAssujettie,
    pub montant: Decimal,
    pub contrat: Option<NumeroContrat>,
}

impl Contact {
    /// Un contact a son nom (S20.G00.07.001), le reste dit par les rubriques qui le suivent.
    fn nomme(nom_prenom: String) -> Contact {
        Contact { nom_prenom, tel: None, email: None, qualite: None, civilite: None }
    }
}

impl Adhesion {
    /// Une adhesion a sa reference (S21.G00.15.001), le reste dit par les rubriques qui la suivent.
    fn de(libelle: String) -> Adhesion {
        Adhesion { libelle, organisme: None, reference: None }
    }
}

impl IndividuDeclare {
    /// Un individu a son NIR (S21.G00.30.001), aucun quand il commence a son nom ; le reste dit par les rubriques qui le
    /// suivent.
    fn de(nir: Option<Nir>) -> IndividuDeclare {
        IndividuDeclare {
            nir,
            nom: None,
            prenom: None,
            sexe: None,
            date_naissance: None,
            lieu_naissance: None,
            adresse: None,
            code_postal: None,
            commune: None,
            code_nationalite_dsn: None,
            departement_naissance: None,
            pays_naissance: None,
            email: None,
            code_salarie: None,
            ntt: None,
        }
    }

    /// Ce qui l'identifie : son NIR, sinon son NTT ; aucun s'il n'a ni l'un ni l'autre.
    pub fn identifiant(&self) -> Option<crate::ntt::IdentifiantDIndividu> {
        match (&self.nir, &self.ntt) {
            (Some(nir), _) => Some(crate::ntt::IdentifiantDIndividu::Nir(nir.clone())),
            (None, Some(ntt)) => Some(crate::ntt::IdentifiantDIndividu::Ntt(ntt.clone())),
            (None, None) => None,
        }
    }
}

impl ContratDeclare {
    /// Un contrat de l'individu `individu` a son debut (S21.G00.40.001), le reste dit par les rubriques qui le suivent.
    fn de(nir: Option<Nir>, individu: usize, debut: Option<Jour>) -> ContratDeclare {
        ContratDeclare {
            nir,
            individu,
            debut,
            emploi: EmploiDeclare::default(),
            nature_dsn: None,
            numero: None,
            fin: None,
            quotite_reference: None,
            quotite: None,
            modalite: None,
            idcc: None,
            motif_recours: None,
            caisse_conges: None,
            code_risque: None,
            taux_at: None,
            licence: None,
            objet: None,
            dernier_jour_travaille: None,
            fin_declaree: None,
            motif_fin: None,
        }
    }
}

impl VersementLu {
    /// Un versement de l'individu a sa date (S21.G00.50.001), le reste dit par les rubriques qui le suivent.
    fn de(nir: Option<Nir>, verse_le: Option<Jour>) -> VersementLu {
        VersementLu {
            nir,
            verse_le,
            net_imposable: None,
            net_verse: None,
            taux_pas: None,
            type_pas: None,
            identifiant_pas: None,
            pas: None,
            remunerations: Vec::new(),
            bases: Vec::new(),
        }
    }
    /// Les contrats que ses remunerations paient (S21.G00.51.010), dans l'ordre ou ils paraissent.
    pub fn contrats(&self) -> Vec<&NumeroContrat> {
        let mut v: Vec<&NumeroContrat> = Vec::new();
        for n in self.remunerations.iter().filter_map(|r| r.contrat.as_ref()) {
            if !v.contains(&n) {
                v.push(n);
            }
        }
        v
    }
}

/// Ce que le bloc 50 dit du versement : ses nets (.002, .004) et son prelevement a la source (.006 a .009). Le bloc n'a
/// pas de numero de contrat (`bloc-S21.G00.50.txt`) : ces montants ne sont ceux d'un contrat que si le versement ne paie
/// que lui, et lui seul (`PaieDuContrat::nets`).
#[derive(Clone, Debug, Default)]
pub struct Nets {
    pub net_imposable: Option<Decimal>,
    pub net_verse: Option<Decimal>,
    pub taux_pas: Option<Decimal>,
    pub type_pas: Option<droit::type_taux_pas::TypeDeTauxPas>,
    pub identifiant_pas: Option<droit::pas::IdentifiantDuTauxPas>,
    pub pas: Option<Decimal>,
}

/// CE QUE LA DSN ATTRIBUE A UN CONTRAT POUR LE MOIS, de tout ce que ses versements portent : ce que la norme rattache au
/// contrat par son numero -- ses remunerations et leurs activites (S21.G00.51.010, CCH-11, `bloc-S21.G00.51.txt`
/// l. 91-98 : `mesures`), ses bases (S21.G00.78.006, `bloc-S21.G00.79.txt` l. 9-14) -- et, quand un seul versement le
/// paie et ne paie que lui, ce que ce versement porte sans le nommer : ses nets, son prelevement (`nets`) et ses bases sans
/// 78.006. Un versement de plusieurs contrats n'attribue a aucun ses nets ni ses bases sans numero : rien ne se repartit.
#[derive(Clone, Debug)]
pub struct PaieDuContrat {
    pub nir: Option<Nir>,
    /// Le contrat (S21.G00.51.010).
    pub numero: NumeroContrat,
    /// Le jour du dernier versement qui le paie (S21.G00.50.001).
    pub verse_le: Option<Jour>,
    pub mesures: Mesures,
    /// Ses bases, par code (78.001), sommees.
    pub bases: BTreeMap<BaseAssujettie, Decimal>,
    pub nets: Option<Nets>,
    /// Les autres contrats que ses versements paient (vide : ils ne paient que lui).
    pub avec: Vec<NumeroContrat>,
}

/// Une adhesion prevoyance (S21.G00.15 : la reference .001, l'organisme .002, la reference de l'adhesion .005).
#[derive(Clone, Debug)]
pub struct Adhesion {
    pub libelle: String,
    pub organisme: Option<IdentifiantDOrganisme>,
    pub reference: Option<String>,
}

/// Ce qu'un contrat declare pour des periodes de paie : son brut (51 de type 001, « remuneration brute non plafonnee » ;
/// `None` : aucun), son salaire habituel (51 de type 010, « la remuneration brute habituelle du salarie »,
/// `bloc-S21.G00.51.txt` : le salaire du contrat d'un permanent) et son activite (53 de type 01).
#[derive(Clone, Debug)]
pub struct Mesures {
    pub brut: Option<Decimal>,
    pub salaire_habituel: Option<Decimal>,
    pub jours: Decimal,
    pub cachets: Decimal,
    pub heures: Decimal,
    pub remunere: bool,
}

/// CE QUE DECLARE UNE DSN DEPOSEE.
#[derive(Clone, Debug)]
pub struct DsnLue {
    /// Le logiciel qui l'a ecrite (S10.G00.00.001), la nature de l'envoi (.005 : `01` test, `02` reel).
    pub logiciel: Option<String>,
    pub nature_envoi: Option<EssaiOuReel>,
    /// Le Siren de l'emetteur (S10.G00.01.001) : celui du cabinet quand un tiers depose (`EntrepriseDeclaree`).
    pub emetteur: Option<Siren>,
    /// La nature de la declaration (S20.G00.05.001 : `01` mensuelle, un signalement sinon).
    pub nature_declaration: Option<NatureDeLaDeclaration>,
    /// Le mois principal declare (S20.G00.05.005) ; un signalement n'en declare pas.
    pub mois: Option<Mois>,
    /// Le type, la fraction et le numero d'ordre de la declaration (S20.G00.05.002, .003, .004), et l'identifiant de
    /// celle qu'elle remplace (.006) ; une valeur hors de sa forme laisse la case vide.
    pub type_declaration: Option<TypeDeLaDeclaration>,
    pub fraction: Option<Fraction>,
    pub ordre: Option<NumeroDOrdre>,
    pub remplace: Option<IdentifiantDeDeclaration>,
    /// Le jour de constitution de l'envoi (S20.G00.05.007).
    pub depot: Option<Jour>,
    /// Le nom du fichier lu ; aucun pour une DSN lue de ses seuls octets.
    pub fichier: Option<NomDeFichier>,
    /// Les lignes du fichier.
    pub lignes: usize,
    /// Le montant de chaque versement a un organisme (S21.G00.20.005), dans l'ordre.
    pub versements_organismes: Vec<Decimal>,
    pub entreprise: EntrepriseDeclaree,
    pub individus: Vec<IndividuDeclare>,
    pub contrats: Vec<ContratDeclare>,
    pub versements: Vec<VersementLu>,
    /// Les etablissements declares (S21.G00.11.001) et le NIC du siege (S21.G00.06.002).
    pub etablissements: Vec<Nic>,
    pub nic_siege: Option<Nic>,
    pub adhesions: Vec<Adhesion>,
    /// Les references d'adhesion auxquelles un contrat se rattache (S21.G00.70.013).
    pub rattachements: Vec<String>,
    /// Chaque rubrique lue et la valeur qu'elle declare (`declare`).
    rubriques: BTreeSet<(RubriqueDeLaNorme, String)>,
}

/// La nature d'un envoi reel (S10.G00.00.005 ; `01` : un test).
pub const NATURE_REEL: EssaiOuReel = EssaiOuReel::Reel;
/// La nature d'une DSN mensuelle (S20.G00.05.001).
pub const MENSUELLE: NatureDeLaDeclaration = NatureDeLaDeclaration::Mensuelle;

impl DsnLue {
    /// Un envoi reel (S10.G00.00.005), pas un test.
    pub fn reel(&self) -> bool {
        self.nature_envoi == Some(NATURE_REEL)
    }

    /// La nature de la declaration (S20.G00.05.001), obligatoire dans toute declaration (tableau des usages) : une DSN
    /// qui ne la dit pas refuse, en le disant.
    pub fn nature(&self) -> R<NatureDeLaDeclaration> {
        self.nature_declaration.ok_or_else(|| erreur(Classe::Refuse, "La DSN ne porte pas sa nature (S20.G00.05.001, obligatoire) : elle ne se lit pas."))
    }

    pub fn est_signalement(&self) -> bool {
        self.nature_declaration.is_some_and(NatureDeLaDeclaration::signalement_envoye)
    }

    /// La nature en clair ; `None` si la DSN ne la porte pas.
    pub fn nature_lisible(&self) -> Option<String> {
        self.nature_declaration.map(|n| n.libelle().to_string())
    }

    /// L'etablissement declare (le premier S21.G00.11.001).
    pub fn etablissement(&self) -> Option<&droit::fiche::Nic> {
        self.etablissements.first()
    }

    /// Le jour de l'evenement d'un signalement : l'arret par son dernier jour travaille (S21.G00.60.002), la fin de
    /// contrat par sa date de fin (S21.G00.62.001), l'amorcage par le debut du contrat (S21.G00.40.001), un amorcage de
    /// changement de logiciel (nature 08, sans aucun de ces jours) par sa constitution.
    pub fn evenement(&self) -> Option<Jour> {
        let premier = |f: fn(&ContratDeclare) -> Option<Jour>| self.contrats.iter().find_map(f);
        premier(|c| c.dernier_jour_travaille)
            .or_else(|| premier(|c| c.fin_declaree))
            .or_else(|| self.contrats.first().and_then(|c| c.debut))
            .or_else(|| self.depot.filter(|_| self.nature_declaration == Some(crate::amorcage::NATURE)))
    }

    /// LE MOIS QUE LE FICHIER DECLARE (S20.G00.05.005) ; un signalement se date par son evenement, et se range dans le
    /// mois de ce jour ; `None` s'il ne le dit pas (`SANS_MOIS`).
    pub fn mois_declare(&self) -> Option<Mois> {
        match (self.mois, self.evenement()) {
            (Some(m), _) => Some(m),
            (None, Some(j)) if self.est_signalement() => Some(Mois::du_jour(&j)),
            _ => None,
        }
    }

    /// Ce qu'on fait recopier pour prouver qu'on a regarde : le jour de l'evenement d'un signalement (`AAAA-MM-JJ`), le
    /// mois sinon. Un fichier qui ne dit ni l'un ni l'autre refuse : il n'y a rien a recopier.
    pub fn confirmation_attendue(&self) -> R<MoisOuJour> {
        let refuse = |phrase: &str| erreur(Classe::Refuse, phrase);
        if self.est_signalement() {
            self.evenement().map(MoisOuJour::Jour).ok_or_else(|| refuse("Le signalement ne porte pas le jour de son événement : il n'y a rien à confirmer."))
        } else {
            self.mois_declare().map(MoisOuJour::Mois).ok_or_else(|| refuse(SANS_MOIS))
        }
    }

    /// Les versements dont le prelevement a la source est declare (S21.G00.50.007), et ceux preleves sur la grille (un
    /// type autre que `01`, le taux transmis par la DGFiP).
    pub fn pas_versements(&self) -> usize {
        self.versements.iter().filter(|v| v.type_pas.is_some()).count()
    }

    pub fn pas_bareme(&self) -> usize {
        self.versements.iter().filter(|v| v.type_pas.is_some_and(|t| !t.transmis())).count()
    }

    /// Les contrats de l'individu `nir` (aucun : ceux des individus sans NIR).
    pub fn contrats_de<'a>(&'a self, nir: Option<&'a Nir>) -> impl Iterator<Item = &'a ContratDeclare> + 'a {
        self.contrats.iter().filter(move |c| c.nir.as_ref() == nir)
    }

    /// Les versements de l'individu `nir` (aucun : ceux des individus sans NIR).
    pub fn versements_de<'a>(&'a self, nir: Option<&'a Nir>) -> impl Iterator<Item = &'a VersementLu> + 'a {
        self.versements.iter().filter(move |v| v.nir.as_ref() == nir)
    }

    /// CE QUE LE CONTRAT `numero` DE L'INDIVIDU `nir` DECLARE, pour le mois (`rappel` faux) ou en rappel d'un mois deja
    /// declare : ses remunerations (S21.G00.51.010 le nomme) dans tous ses versements, le brut de type 001, le salaire
    /// habituel de type 010, et le travail remunere (S21.G00.53 de type 01 : en heures, unites 10 et 35 ; en jours, 12 ;
    /// en cachets, 37).
    pub fn mesures(&self, nir: Option<&Nir>, numero: &NumeroContrat, rappel: bool) -> Mesures {
        // Les sommes du contrat, a zero avant sa premiere remuneration.
        let mut m = Mesures { brut: None, salaire_habituel: None, jours: Decimal::ZERO, cachets: Decimal::ZERO, heures: Decimal::ZERO, remunere: false };
        for r in self.versements_de(nir).flat_map(|v| v.remunerations.iter()) {
            if r.contrat.as_ref() != Some(numero) || self.mois.is_some_and(|m| r.rappel(m)) != rappel {
                continue;
            }
            if let Some(montant) = r.montant {
                m.remunere = true;
                match r.genre {
                    Some(TypeDeRemuneration::Brute) => m.brut = Some(m.brut.iter().sum::<Decimal>() + montant),
                    Some(TypeDeRemuneration::SalaireDeBase) => m.salaire_habituel = Some(montant),
                    _ => {}
                }
            }
            // Une activite sans mesure (53.002) ne compte rien.
            for (a, mesure) in r.activites.iter().filter(|a| a.genre == Some(TypeDActivite::TravailRemunere)).filter_map(|a| a.mesure.map(|m| (a, m))) {
                match a.unite {
                    Some(UniteDeMesure::Heure | UniteDeMesure::HeuresIntermittents) => m.heures += mesure,
                    Some(UniteDeMesure::Journee) => m.jours += mesure,
                    Some(UniteDeMesure::CachetsIsoles) => m.cachets += mesure,
                    _ => {}
                }
            }
        }
        m
    }

    /// LES PAIES DU MOIS QUE LA DSN DECLARE, une par contrat qu'un versement paie (S21.G00.51.010), dans l'ordre ou
    /// elles paraissent, chacune avec ce que la DSN lui attribue (`PaieDuContrat`).
    pub fn paies(&self) -> Vec<PaieDuContrat> {
        let mut contrats: Vec<(Option<&Nir>, &NumeroContrat)> = Vec::new();
        for v in &self.versements {
            for c in v.contrats() {
                if !contrats.iter().any(|(nir, numero)| *nir == v.nir.as_ref() && *numero == c) {
                    contrats.push((v.nir.as_ref(), c));
                }
            }
        }
        contrats.into_iter().map(|(nir, numero)| self.paie_du_contrat(nir, numero)).collect()
    }

    /// Ce que la DSN attribue au contrat `numero` de l'individu `nir` (`paies`).
    fn paie_du_contrat(&self, nir: Option<&Nir>, numero: &NumeroContrat) -> PaieDuContrat {
        let payeurs: Vec<&VersementLu> = self.versements_de(nir).filter(|v| v.contrats().contains(&numero)).collect();
        let seul = match payeurs.as_slice() {
            [v] if v.contrats().len() == 1 => Some(*v),
            _ => None,
        };
        let mut avec: Vec<NumeroContrat> = Vec::new();
        for c in payeurs.iter().flat_map(|v| v.contrats()) {
            if c != numero && !avec.contains(c) {
                avec.push(c.clone());
            }
        }
        let mut bases: BTreeMap<BaseAssujettie, Decimal> = BTreeMap::new();
        for v in self.versements_de(nir) {
            let sans_numero = seul.is_some_and(|s| std::ptr::eq(s, v));
            let a_lui = |b: &&BaseLue| match b.contrat.as_ref() {
                Some(c) => c == numero,
                None => sans_numero,
            };
            for b in v.bases.iter().filter(a_lui) {
                *bases.entry(b.code).or_default() += b.montant;
            }
        }
        PaieDuContrat {
            nir: nir.cloned(),
            numero: numero.clone(),
            verse_le: payeurs.last().and_then(|v| v.verse_le),
            mesures: self.mesures(nir, numero, false),
            bases,
            nets: seul.map(|v| Nets {
                net_imposable: v.net_imposable,
                net_verse: v.net_verse,
                taux_pas: v.taux_pas,
                type_pas: v.type_pas,
                identifiant_pas: v.identifiant_pas.clone(),
                pas: v.pas,
            }),
            avec,
        }
    }

    /// LA DSN DECLARE-T-ELLE `valeur` EN `rubrique` ? La rubrique est celle que la table des champs donne a une case
    /// (`champs-base.json`, `rubrique`) ; la valeur, telle que la DSN l'ecrit.
    pub fn declare(&self, rubrique: RubriqueDeLaNorme, valeur: &str) -> bool {
        self.rubriques.contains(&(rubrique, valeur.to_string()))
    }
}

impl EntrepriseDeclaree {
    /// La DSN ne dit rien de ce que la fiche Entreprise porte (la licence, que la fiche date a part, n'y compte pas).
    pub fn est_vide(&self) -> bool {
        self.siren.is_none()
            && self.nic.is_none()
            && self.idcc.is_none()
            && self.naf.is_none()
            && self.code_postal.is_none()
            && self.medecine_travail.is_none()
            && [&self.adresse, &self.commune, &self.adherent_conges_spectacles, &self.raison_sociale, &self.france_travail_dossier_recouvrement]
            .iter()
            .all(|c| c.is_none())
            && self.banque.is_none()
            && self.declarant_dsn.is_none()
    }

    /// Ce qu'une DSN plus recente declare remplace ce que disait la precedente.
    pub fn suivre(&mut self, r: &EntrepriseDeclaree) {
        fn prendre<T: Clone>(a: &mut Option<T>, b: &Option<T>) {
            if b.is_some() {
                *a = b.clone();
            }
        }
        prendre(&mut self.siren, &r.siren);
        prendre(&mut self.naf, &r.naf);
        prendre(&mut self.nic, &r.nic);
        prendre(&mut self.adresse, &r.adresse);
        prendre(&mut self.code_postal, &r.code_postal);
        prendre(&mut self.commune, &r.commune);
        prendre(&mut self.idcc, &r.idcc);
        prendre(&mut self.adherent_conges_spectacles, &r.adherent_conges_spectacles);
        prendre(&mut self.raison_sociale, &r.raison_sociale);
        prendre(&mut self.banque, &r.banque);
        prendre(&mut self.france_travail_dossier_recouvrement, &r.france_travail_dossier_recouvrement);
        prendre(&mut self.declarant_dsn, &r.declarant_dsn);
        prendre(&mut self.medecine_travail, &r.medecine_travail);
    }
}

// ------------------------------------------------------------------ quel fichier est une DSN

/// UNE DSN SE RECONNAIT A SON CONTENU, PAS A SON NOM : elle commence par la rubrique S10.G00.00.001, apres les marques
/// d'ordre d'octets UTF-8 qu'un editeur aurait posees.
pub fn est_une_dsn(tete: &[u8]) -> bool {
    let mut t = tete;
    while t.starts_with(b"\xef\xbb\xbf") {
        t = &t[3..];
    }
    t.starts_with(b"S10.G00.00.001,")
}

/// LES DSN D'UN DOSSIER ET DE SES SOUS-DOSSIERS (sans suivre un lien vers un dossier), par leur contenu
/// (`est_une_dsn`), dans l'ordre des noms. Un dossier qui n'existe pas n'en a aucune ; un disque qui refuse se dit.
pub fn fichiers(dossier: &Path) -> R<Vec<PathBuf>> {
    let mut out = Vec::new();
    parcourir(dossier, &mut out)?;
    Ok(out)
}

fn parcourir(dossier: &Path, out: &mut Vec<PathBuf>) -> R<()> {
    let panne = |e: std::io::Error, p: &Path| refus(socle::fichiers::panne_fichier(&e, p));
    let entrees = match std::fs::read_dir(dossier) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(panne(e, dossier)),
    };
    let mut entrees: Vec<std::fs::DirEntry> = entrees.collect::<Result<_, _>>().map_err(|e| panne(e, dossier))?;
    entrees.sort_by_key(|e| e.file_name());
    for e in entrees {
        let chemin = e.path();
        if e.file_type().map_err(|x| panne(x, &chemin))?.is_dir() {
            parcourir(&chemin, out)?;
        } else if chemin.is_file() && est_une_dsn(&tete(&chemin).map_err(|x| panne(x, &chemin))?) {
            out.push(chemin);
        }
    }
    Ok(())
}

/// Les premiers octets d'un fichier, assez pour reconnaitre l'en-tete d'une DSN derriere des marques d'ordre d'octets.
fn tete(chemin: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut t = Vec::with_capacity(64);
    std::fs::File::open(chemin)?.take(64).read_to_end(&mut t)?;
    Ok(t)
}

// ------------------------------------------------------------------ la lecture

/// Le sexe declare (S21.G00.30.005 : « 01 - masculin », « 02 - féminin ») en celui de la fiche ; un autre code laisse la
/// case vide (`dans_sa_forme`).
fn sexe_de_la_fiche(v: &str) -> Option<Sexe> {
    match v {
        "01" => Some(Sexe::Masculin),
        "02" => Some(Sexe::Feminin),
        _ => None,
    }
}

/// UNE CASE DE LA FICHE QUE LA DSN REMPLIT, lue dans son type : hors de sa forme, elle reste vide, comme une DSN qui ne
/// la dirait pas (une DSN lue : un inconnu ne s'invente pas, et ne refuse pas l'import).
fn dans_sa_forme<T>(v: &str, lire: fn(&str) -> R<T>) -> Option<T> {
    match lire(v) {
        Ok(t) => Some(t),
        Err(_) => None,
    }
}

/// UNE DATE DE NAISSANCE DECLAREE (S21.G00.30.006) : `JJMMAAAA`, dont le jour ou le mois peuvent etre inconnus, ecrits
/// `00` ou `99` (cahier technique 2026.1, p. 172, `atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt`, l. 175-178 :
/// « Jour de naissance inconnu : 00MMAAAA ou 99MMAAAA, Mois de naissance inconnu : JJ00AAAA ou JJ99AAAA, Si jour et
/// mois inconnus : 0000AAAA ou 9999AAAA » ; CSL 00 et CSL-11, l. 185-189). Elle se lit dans cette forme et elle seule,
/// garde ses chiffres tels qu'ils sont declares (`jjmmaaaa`), et passe a la fiche en `AAAA-MM-JJ` (`iso`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NaissanceDeclaree {
    jour: u32,
    mois: u32,
    an: i32,
}

impl NaissanceDeclaree {
    /// Le motif de CSL 00 (`(0[0-9]|[1-2][0-9]|3[0-1]|99)(0[0-9]|1[0-2]|99)(18|19|20)[0-9]{2}`), et un jour du
    /// calendrier quand le jour et le mois sont connus (CSL-11) ; `None` sinon.
    pub fn lire(v: &str) -> Option<NaissanceDeclaree> {
        if v.len() != 8 || !v.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let n = |a: usize, b: usize| v.as_bytes()[a..b].iter().fold(0u32, |x, c| x * 10 + u32::from(c - b'0'));
        let (jour, mois, an) = (n(0, 2), n(2, 4), n(4, 8) as i32);
        let inconnu = |x: u32| x == 0 || x == 99;
        let forme = (jour <= 31 || jour == 99) && (mois <= 12 || mois == 99) && (1800..=2099).contains(&an);
        let calendrier = inconnu(jour) || inconnu(mois) || Jour::nouveau(an, mois, jour).is_some();
        (forme && calendrier).then_some(NaissanceDeclaree { jour, mois, an })
    }

    /// `JJMMAAAA`, tel que declare.
    pub fn jjmmaaaa(&self) -> String {
        format!("{:02}{:02}{:04}", self.jour, self.mois, self.an)
    }

    /// `AAAA-MM-JJ`, le jour et le mois tels que declares (`socle::date::iso`).
    pub fn iso(&self) -> String {
        socle::date::iso(self.an, self.mois, self.jour)
    }
}

impl Serialize for NaissanceDeclaree {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.iso())
    }
}

/// Une date de la DSN, JJMMAAAA, qui est un jour.
pub(crate) fn jour_dsn(v: &str) -> Option<Jour> {
    socle::piece::Zone::lue(v, socle::piece::format::Jjmmaaaa).valeur()
}

/// Ce qu'une DSN porte et qui ne se lit pas : une valeur qui n'est pas ce que sa rubrique dit, ou une ligne qui n'est
/// pas une rubrique de la norme.
enum Faute {
    Valeur { rubrique: RubriqueDeLaNorme, valeur: String, quoi: &'static str },
    Rubrique { ligne: String, version: Option<VersionDeLaNorme> },
}

/// La premiere faute de la lecture.
#[derive(Default)]
struct Fautes(Option<Faute>);

impl Fautes {
    fn noter(&mut self, f: Faute) {
        self.0.get_or_insert(f);
    }
    fn valeur<T>(&mut self, k: RubriqueDeLaNorme, v: &str, quoi: &'static str, lu: Option<T>) -> Option<T> {
        if lu.is_none() && !v.is_empty() {
            self.noter(Faute::Valeur { rubrique: k, valeur: v.to_string(), quoi });
        }
        lu
    }
    fn montant(&mut self, k: RubriqueDeLaNorme, v: &str) -> Option<Decimal> {
        self.valeur(k, v, "un montant", socle::num::decimal_lu(v))
    }
    fn nombre(&mut self, k: RubriqueDeLaNorme, v: &str) -> Option<Decimal> {
        self.valeur(k, v, "un nombre", socle::num::decimal_lu(v))
    }
    fn jour(&mut self, k: RubriqueDeLaNorme, v: &str) -> Option<Jour> {
        self.valeur(k, v, "une date (JJMMAAAA)", jour_dsn(v))
    }
    /// Une valeur que son type lit (`droit::fiche::Siren`, `Nic`...) : un texte qu'il refuse est une faute, que `quoi`
    /// nomme.
    fn lue<T>(&mut self, k: RubriqueDeLaNorme, v: &str, quoi: &'static str, lire: fn(&str) -> R<T>) -> Option<T> {
        let lu = match lire(v) {
            Ok(t) => Some(t),
            Err(_) => None,
        };
        self.valeur(k, v, quoi, lu)
    }
    /// Un code d'une liste de la norme (`droit::natures::ListeDeLaNorme`) : un code qu'elle n'a pas est une faute.
    fn code<T: ListeDeLaNorme>(&mut self, k: RubriqueDeLaNorme, v: &str) -> Option<T> {
        self.valeur(k, v, "un code de sa liste dans la norme", T::du_code_de_la_norme(v))
    }
    /// Un refus dit quoi corriger et n'ecrit rien.
    fn refus(self, mois: Option<Mois>) -> Option<Echec> {
        let de: String = mois.iter().map(|m| format!(" de {}", m.libelle(socle::mois::Graphie::Accentuee))).collect();
        let ce = match self.0? {
            Faute::Valeur { rubrique, valeur, quoi } => format!("porte, en {}, « {} », qui n'est pas {}", rubrique, valeur, quoi),
            Faute::Rubrique { ligne, version: Some(v) } => format!("porte la rubrique {}, que la norme DSN {} ne connaît pas", ligne, v),
            Faute::Rubrique { ligne, version: None } => format!("porte la ligne « {} », qui n'est pas une rubrique de la norme DSN", ligne),
        };
        Some(erreur(Classe::Refuse, format!("La DSN{} {} : corrigez-la ou retirez-la, puis recommencez.", de, ce)))
    }
}

/// Le premier qui le dit.
fn premier<T: Clone>(a: &mut Option<T>, v: &Option<T>) {
    if a.is_none() {
        *a = v.clone();
    }
}

/// La DSN du fichier `chemin` (`lire`) ; `Ok(None)` s'il n'est pas une DSN (`est_une_dsn`). Un disque qui refuse se dit.
pub fn lire_une_dsn(chemin: &Path) -> R<Option<DsnLue>> {
    let octets = std::fs::read(chemin).map_err(|e| refus(socle::fichiers::panne_fichier(&e, chemin)))?;
    if !est_une_dsn(&octets) {
        return Ok(None);
    }
    lire(&octets, Some(&NomDeFichier::du(chemin)?)).map(Some)
}

/// UNE DSN, LUE (`fichier` : son nom, aucun pour des octets sans fichier), un signalement sans mois principal compris. Une valeur qui n'est pas ce que sa
/// rubrique dit (un montant, une date), ou une ligne qui n'est pas une
/// rubrique, refuse : rien ne se devine (regle 1). La rubrique se controle au tableau des usages de la version que le
/// fichier declare (S10.G00.00.006, `usages-dsn-base.json`) ; une version que le logiciel ne livre pas (une DSN d'avant
/// 2026) ne se controle que par la forme de ses rubriques (`RubriqueDeLaNorme::lue`).
pub fn lire(octets: &[u8], fichier: Option<&NomDeFichier>) -> R<DsnLue> {
    let contenu = texte_latin1(octets);
    // L'indicateur d'ordre des octets d'UTF-8, lu en latin-1, n'est pas une rubrique.
    let lignes = lignes_du_texte(contenu.trim_start_matches("\u{ef}\u{bb}\u{bf}"));
    let valeur = |v: &str| v.trim_matches(blanc).trim_matches('\'').to_string();
    let norme = match lignes.iter().find_map(|l| l.split_once(',').filter(|(k, _)| RubriqueDeLaNorme::lue(k) == Some(VersionDeLaNorme::RUBRIQUE)).map(|(_, v)| valeur(v))) {
        // Une version que le logiciel ne livre pas (une DSN d'avant 2026) ne se controle que par la forme.
        Some(v) => match VersionDeLaNorme::du_code(&v) {
            Some(version) => crate::norme::Norme::livree(version)?,
            None => None,
        },
        None => None,
    };
    let mut fautes = Fautes::default();
    let (mut mois, mut depot): (Option<Mois>, Option<Jour>) = (None, None);
    let mut type_declaration: Option<TypeDeLaDeclaration> = None;
    let (mut fraction, mut ordre, mut remplace): (Option<Fraction>, Option<NumeroDOrdre>, Option<IdentifiantDeDeclaration>) = (None, None, None);
    let (mut logiciel, mut nature_envoi): (Option<String>, Option<EssaiOuReel>) = (None, None);
    let mut nature: Option<NatureDeLaDeclaration> = None;
    let mut versements_organismes: Vec<Decimal> = Vec::new();
    let mut e = EntrepriseDeclaree::default();
    let mut individus: Vec<IndividuDeclare> = Vec::new();
    let mut contrats: Vec<ContratDeclare> = Vec::new();
    let mut versements: Vec<VersementLu> = Vec::new();
    // Une base (78.001) est ouverte dans le versement en cours.
    let mut base = false;
    // Le contrat dont la DSN declare la fin (S21.G00.62.001) : le dernier contrat lu.
    let mut fin_de: Option<usize> = None;
    // L'emetteur, son contact, le siege et l'APET : ce qui ne sert qu'a defaut, ou sous condition (voir
    // `EntrepriseDeclaree`).
    let (mut emetteur_siren, mut emetteur_nom, mut civilite_emetteur, mut nom_emetteur) = (None, None, None, None);
    let (mut nic_siege, mut adresse_siege, mut code_postal_siege, mut commune_siege, mut apet) = (None, None, None, None, None);
    let mut etablissements: Vec<Nic> = Vec::new();
    // Les versements aux organismes (bloc 20) : identifiant, entite d'affectation, BIC, IBAN.
    let mut blocs20: Vec<Bloc20> = Vec::new();
    let mut contacts: Vec<Contact> = Vec::new();
    let mut adhesions: Vec<Adhesion> = Vec::new();
    let mut rattachements: Vec<String> = Vec::new();
    let mut rubriques: BTreeSet<(RubriqueDeLaNorme, String)> = BTreeSet::new();
    let mut spst = String::new();
    for ligne in &lignes {
        if ligne.trim_matches(blanc).is_empty() {
            continue;
        }
        // `k` la rubrique telle que la ligne l'ecrit, `rub` la meme, lue.
        let Some((k, v, rub)) = ligne.split_once(',').and_then(|(k, v)| Some((k, v, RubriqueDeLaNorme::lue(k)?))) else {
            fautes.noter(Faute::Rubrique { ligne: ligne.to_string(), version: None });
            continue;
        };
        if let Some(n) = &norme {
            if !crate::usages::rubrique_connue(n, rub)? {
                fautes.noter(Faute::Rubrique { ligne: k.to_string(), version: Some(n.version) });
                continue;
            }
        }
        let v = valeur(v);
        rubriques.insert((rub, v.clone()));
        let plein = Some(v.clone()).filter(|x| !x.is_empty());
        // UN INDIVIDU SANS NIR (S21.G00.30.001 absent, « En l'absence de NIR ou de NIA », `bloc-S21.G00.30.txt`
        // l. 338-354) commence a son nom (.002).
        if k == "S21.G00.30.002" && individus.last().is_none_or(|i| i.nom.is_some()) {
            individus.push(IndividuDeclare::de(None));
        }
        let individu = individus.len().checked_sub(1);
        let versement = versements.last_mut();
        let contrat = contrats.last_mut();
        match k {
            "S10.G00.00.001" => premier(&mut logiciel, &plein),
            "S10.G00.00.005" => premier(&mut nature_envoi, &EssaiOuReel::du_code(&v)),
            "S20.G00.05.001" => {
                let n = fautes.code(rub, &v);
                nature = nature.or(n);
            }
            "S20.G00.05.003" => premier(&mut fraction, &Fraction::lue(&v)),
            "S20.G00.05.006" => premier(&mut remplace, &IdentifiantDeDeclaration::lu(&v)),
            "S21.G00.20.005" => versements_organismes.extend(fautes.montant(rub, &v)),
            "S21.G00.30.020" => {
                if let Some(i) = individu {
                    individus[i].ntt = crate::ntt::Ntt::lu(&v)?;
                }
            }
            "S21.G00.40.012" => {
                let q = fautes.nombre(rub, &v);
                contrat.into_iter().for_each(|c| c.quotite_reference = q);
            }
            "S21.G00.40.014" => {
                let m = fautes.code(rub, &v);
                contrat.into_iter().for_each(|c| c.modalite = m);
            }
            "S21.G00.40.021" => {
                let m = fautes.code(rub, &v);
                contrat.into_iter().for_each(|c| c.motif_recours = m);
            }
            "S21.G00.60.002" => {
                let jour = fautes.jour(rub, &v);
                contrat.into_iter().for_each(|c| c.dernier_jour_travaille = jour);
            }
            "S10.G00.01.001" => premier(&mut emetteur_siren, &fautes.lue(rub, &v, SIREN, Siren::lire)),
            "S10.G00.01.003" => premier(&mut emetteur_nom, &plein),
            "S10.G00.02.001" => civilite_emetteur = Civilite::du_code(&v),
            "S10.G00.02.002" => nom_emetteur = plein,
            "S20.G00.05.002" => type_declaration = fautes.code(rub, &v),
            "S20.G00.05.004" => ordre = NumeroDOrdre::lu(&v),
            "S20.G00.05.005" => {
                // 01MMAAAA : le premier jour du mois.
                let premier_jour = jour_dsn(&v).filter(|j| j.jour == 1);
                mois = fautes.valeur(rub, &v, "un mois (01MMAAAA)", premier_jour.map(|j| Mois::du_jour(&j)));
            }
            "S20.G00.05.007" => depot = fautes.jour(rub, &v),
            "S20.G00.07.001" => contacts.push(Contact::nomme(v)),
            "S20.G00.07.002" | "S20.G00.07.004" => {
                if let Some(c) = contacts.last_mut() {
                    *match k { "S20.G00.07.002" => &mut c.tel, _ => &mut c.qualite } = plein;
                }
            }
            "S20.G00.07.003" => {
                let email = fautes.lue(rub, &v, COURRIEL, Courriel::lire);
                if let Some(c) = contacts.last_mut() {
                    c.email = email;
                }
            }
            "S21.G00.06.001" => premier(&mut e.siren, &fautes.lue(rub, &v, SIREN, Siren::lire)),
            "S21.G00.06.002" => premier(&mut nic_siege, &fautes.lue(rub, &v, NIC, Nic::lire)),
            "S21.G00.06.003" => premier(&mut e.naf, &dans_sa_forme(&v, CodeNaf::lire)),
            "S21.G00.06.004" => premier(&mut adresse_siege, &plein),
            "S21.G00.06.005" => premier(&mut code_postal_siege, &dans_sa_forme(&v, CodePostal::lire)),
            "S21.G00.06.006" => premier(&mut commune_siege, &plein),
            "S21.G00.11.001" => {
                let nic = fautes.lue(rub, &v, NIC, Nic::lire);
                premier(&mut e.nic, &nic);
                if let Some(nic) = nic.filter(|n| !etablissements.contains(n)) {
                    etablissements.push(nic);
                }
            }
            "S21.G00.11.002" => premier(&mut apet, &dans_sa_forme(&v, CodeNaf::lire)),
            "S21.G00.11.003" => premier(&mut e.adresse, &plein),
            "S21.G00.11.004" => premier(&mut e.code_postal, &dans_sa_forme(&v, CodePostal::lire)),
            "S21.G00.11.005" => premier(&mut e.commune, &plein),
            "S21.G00.11.022" => premier(&mut e.idcc, &fautes.lue(rub, &v, IDCC, Idcc::lire)),
            "S21.G00.11.025" => spst = v,
            "S21.G00.15.001" => adhesions.push(Adhesion::de(v)),
            "S21.G00.15.002" => {
                let organisme = fautes.lue(rub, &v, ORGANISME, IdentifiantDOrganisme::lire);
                if let Some(a) = adhesions.last_mut() {
                    a.organisme = organisme;
                }
            }
            "S21.G00.15.005" => {
                if let Some(a) = adhesions.last_mut() {
                    a.reference = plein;
                }
            }
            "S21.G00.20.001" => {
                let organisme = fautes.lue(rub, &v, ORGANISME, IdentifiantDOrganisme::lire);
                blocs20.push(Bloc20 { organisme, affectation: String::new(), bic: None, iban: None })
            }
            "S21.G00.20.002" => {
                if let Some(b) = blocs20.last_mut() {
                    b.affectation = v;
                }
            }
            "S21.G00.20.003" => {
                let bic = fautes.lue(rub, &v, BIC, Bic::lire);
                if let Some(b) = blocs20.last_mut() {
                    b.bic = bic;
                }
            }
            "S21.G00.20.004" => {
                let iban = fautes.lue(rub, &v, IBAN, Iban::lire);
                if let Some(b) = blocs20.last_mut() {
                    b.iban = iban;
                }
            }
            "S21.G00.30.001" => individus.push(IndividuDeclare::de(plein.as_deref().and_then(Nir::d_une_piece))),
            "S21.G00.30.006" => {
                let Some(i) = individu else { continue };
                individus[i].date_naissance = fautes.valeur(rub, &v, "une date de naissance (JJMMAAAA)", NaissanceDeclaree::lire(&v));
            }
            // 30.019 : un matricule hors de sa forme (X [1,30]) laisse la case vide, comme une DSN qui ne le dirait pas
            // (la regle d'une DSN lue : un code hors liste ou hors forme ne refuse pas l'import).
            "S21.G00.30.019" => {
                let Some(i) = individu else { continue };
                individus[i].code_salarie = match droit::fiche::Matricule::lire(&v) {
                    Ok(m) => Some(m),
                    Err(_) => None,
                };
            }
            "S21.G00.30.002" | "S21.G00.30.004" | "S21.G00.30.007" | "S21.G00.30.008" | "S21.G00.30.010" => {
                let Some(i) = individu else { continue };
                let s = &mut individus[i];
                let case = match k {
                    "S21.G00.30.002" => &mut s.nom,
                    "S21.G00.30.004" => &mut s.prenom,
                    "S21.G00.30.007" => &mut s.lieu_naissance,
                    "S21.G00.30.008" => &mut s.adresse,
                    _ => &mut s.commune,
                };
                *case = Some(v);
            }
            "S21.G00.30.005" | "S21.G00.30.009" | "S21.G00.30.013" | "S21.G00.30.014" | "S21.G00.30.015" => {
                let Some(i) = individu else { continue };
                let s = &mut individus[i];
                match k {
                    "S21.G00.30.005" => s.sexe = sexe_de_la_fiche(&v),
                    "S21.G00.30.009" => s.code_postal = dans_sa_forme(&v, CodePostal::lire),
                    "S21.G00.30.013" => s.code_nationalite_dsn = CodeNationalite::du_code(&v),
                    "S21.G00.30.014" => s.departement_naissance = dans_sa_forme(&v, DepartementNaissance::lire),
                    _ => s.pays_naissance = dans_sa_forme(&v, CodePays::lire),
                }
            }
            "S21.G00.30.018" => {
                let Some(i) = individu else { continue };
                individus[i].email = fautes.lue(rub, &v, COURRIEL, Courriel::lire);
            }
            "S21.G00.40.001" => {
                let Some(i) = individu else { continue };
                let debut = fautes.jour(rub, &v);
                contrats.push(ContratDeclare::de(individus[i].nir.clone(), i, debut));
                fin_de = Some(contrats.len() - 1);
            }
            // 40.002 et 40.003 : un code que la fiche connait se lit ; un autre code de la norme (01, 02, 08 a 10 ; 98, 99)
            // laisse la case vide, comme une DSN qui ne le dirait pas (decide par Leandre : la fiche ne les porte pas).
            "S21.G00.40.002" => {
                let Some(c) = contrat else { continue };
                c.emploi.statut_conventionnel = droit::fiche::StatutConventionnel::du_code(&v);
            }
            "S21.G00.40.003" => {
                let Some(c) = contrat else { continue };
                c.emploi.statut_categoriel_rc = droit::fiche::StatutCategorielRc::du_code(&v);
            }
            "S21.G00.40.006" => {
                let Some(c) = contrat else { continue };
                c.emploi.emploi = plein;
            }
            "S21.G00.40.004" => {
                let pcs = fautes.lue(rub, &v, PCS_ESE, droit::fiche::CodePcsEse::lire);
                contrat.into_iter().for_each(|c| c.emploi.pcs_ese = pcs.clone());
            }
            "S21.G00.40.005" => {
                let code = fautes.lue(rub, &v, CODE_EMPLOI, droit::fiche::CodeEmploiFt::lire);
                contrat.into_iter().for_each(|c| c.emploi.code_emploi_ft = code.clone());
            }
            "S21.G00.40.007" => contrat.into_iter().for_each(|c| c.nature_dsn = plein.clone()),
            "S21.G00.40.009" => {
                let numero = fautes.lue(rub, &v, NUMERO, NumeroContrat::lire);
                contrat.into_iter().for_each(|c| c.numero = numero.clone());
            }
            "S21.G00.40.010" => {
                let fin = fautes.jour(rub, &v);
                contrat.into_iter().for_each(|c| c.fin = fin);
            }
            "S21.G00.40.013" => {
                let q = fautes.nombre(rub, &v);
                contrat.into_iter().for_each(|c| c.quotite = q);
            }
            "S21.G00.40.017" => {
                let idcc = fautes.lue(rub, &v, IDCC, Idcc::lire);
                premier(&mut e.idcc, &idcc);
                contrat.into_iter().for_each(|c| c.idcc = idcc.clone());
            }
            "S21.G00.40.022" => {
                if let Some(n) = v.strip_prefix("97").filter(|n| documents::conges_spectacles::faute_adherent(n).is_none()) {
                    premier(&mut e.adherent_conges_spectacles, &Some(n.to_string()));
                }
                contrat.into_iter().for_each(|c| c.caisse_conges = plein.clone());
            }
            // Un code hors de sa forme laisse la case vide (une DSN lue : lire, un inconnu ne s'invente pas).
            "S21.G00.40.040" => contrat.into_iter().for_each(|c| c.code_risque = droit::bareme::CodeRisqueAt::lire(&v).ok()),
            "S21.G00.40.043" => {
                let t = fautes.nombre(rub, &v);
                contrat.into_iter().for_each(|c| c.taux_at = t);
            }
            "S21.G00.40.049" => {
                premier(&mut e.licence, &plein);
                contrat.into_iter().for_each(|c| c.licence = plein.clone());
            }
            "S21.G00.40.050" => {
                let objet = fautes.lue(rub, &v, OBJET, NumeroObjetSpectacle::lire);
                contrat.into_iter().for_each(|c| c.objet = objet.clone());
            }
            "S21.G00.62.001" => {
                let fin = fautes.jour(rub, &v);
                if let Some(c) = fin_de.and_then(|i| contrats.get_mut(i)) {
                    c.fin_declaree = fin;
                }
            }
            "S21.G00.62.002" => {
                let m = fautes.code(rub, &v);
                if let Some(c) = fin_de.and_then(|i| contrats.get_mut(i)) {
                    c.motif_fin = m;
                }
            }
            "S21.G00.50.001" => {
                let Some(i) = individu else { continue };
                let verse_le = fautes.jour(rub, &v);
                versements.push(VersementLu::de(individus[i].nir.clone(), verse_le));
                base = false;
            }
            "S21.G00.50.002" => {
                let m = fautes.montant(rub, &v);
                versement.into_iter().for_each(|x| x.net_imposable = m);
            }
            "S21.G00.50.004" => {
                let m = fautes.montant(rub, &v);
                versement.into_iter().for_each(|x| x.net_verse = m);
            }
            "S21.G00.50.006" => {
                let t = fautes.nombre(rub, &v);
                versement.into_iter().for_each(|x| x.taux_pas = t);
            }
            "S21.G00.50.007" => {
                let t = fautes.code(rub, &v);
                versement.into_iter().for_each(|x| x.type_pas = t);
            }
            // Un identifiant hors de sa forme laisse la case vide (une DSN lue : un inconnu ne s'invente pas).
            "S21.G00.50.008" => versement.into_iter().for_each(|x| x.identifiant_pas = droit::pas::IdentifiantDuTauxPas::lire(&v).ok()),
            "S21.G00.50.009" => {
                let m = fautes.montant(rub, &v);
                versement.into_iter().for_each(|x| x.pas = m);
            }
            "S21.G00.51.001" => {
                let Some(x) = versement else { continue };
                let debut = fautes.jour(rub, &v);
                x.remunerations.push(Remuneration { debut, ..Remuneration::default() });
            }
            "S21.G00.51.010" | "S21.G00.51.011" | "S21.G00.51.013" => {
                let Some(r) = versement.and_then(|x| x.remunerations.last_mut()) else { continue };
                match k {
                    "S21.G00.51.010" => r.contrat = fautes.lue(rub, &v, NUMERO, NumeroContrat::lire),
                    "S21.G00.51.011" => r.genre = TypeDeRemuneration::du_code(&v),
                    _ => r.montant = fautes.montant(rub, &v),
                }
            }
            "S21.G00.53.001" => {
                let Some(r) = versement.and_then(|x| x.remunerations.last_mut()) else { continue };
                r.activites.push(Activite { genre: TypeDActivite::du_code(&v), ..Activite::default() });
            }
            "S21.G00.53.002" | "S21.G00.53.003" => {
                let mesure = if k == "S21.G00.53.002" { fautes.nombre(rub, &v) } else { None };
                let Some(a) = versement.and_then(|x| x.remunerations.last_mut()).and_then(|r| r.activites.last_mut()) else { continue };
                match k {
                    "S21.G00.53.002" => a.mesure = mesure,
                    _ => a.unite = UniteDeMesure::du_code(&v),
                }
            }
            "S21.G00.70.013" => rattachements.push(v),
            "S21.G00.78.001" => {
                let code = fautes.code(rub, &v);
                let Some(x) = versement else { continue };
                base = code.is_some();
                if let Some(code) = code {
                    x.bases.push(BaseLue { code, montant: Decimal::ZERO, contrat: None });
                }
            }
            "S21.G00.78.004" | "S21.G00.78.006" => {
                let m = if k == "S21.G00.78.004" { fautes.montant(rub, &v) } else { None };
                let Some(b) = versement.filter(|_| base).and_then(|x| x.bases.last_mut()) else { continue };
                match k {
                    "S21.G00.78.004" => b.montant += m.iter().sum::<Decimal>(),
                    _ => b.contrat = fautes.lue(rub, &v, NUMERO, NumeroContrat::lire),
                }
            }
            _ => {}
        }
    }
    if let Some(refus) = fautes.refus(mois) {
        return Err(refus);
    }
    let emetteur_est_l_entreprise = emetteur_siren.is_some() && emetteur_siren == e.siren;
    if emetteur_est_l_entreprise && emetteur_nom.is_some() {
        e.raison_sociale = emetteur_nom;
    }
    premier(&mut e.nic, &nic_siege);
    premier(&mut e.naf, &apet);
    if e.adresse.is_none() && e.code_postal.is_none() {
        (e.adresse, e.code_postal, e.commune) = (adresse_siege, code_postal_siege, commune_siege.or(e.commune));
    }
    // LE COMPTE QUI PAIE (`Banque`), quand tous les versements de la DSN disent le meme compte ; deux comptes, rien (on ne
    // choisit pas). Le dossier de recouvrement France Travail : l'entite d'affectation (20.002) du versement a France
    // Travail, « identifiant attribue par le centre de recouvrement Spectacle (CS suivi de 8 caracteres numeriques) »
    // (p. 155), que le moteur ecrit depuis `france_travail_dossier_recouvrement`.
    let comptes: BTreeSet<(&Bic, &Iban)> = blocs20.iter().filter_map(|b| b.bic.as_ref().zip(b.iban.as_ref())).collect();
    if let (1, Some((bic, iban))) = (comptes.len(), comptes.iter().next()) {
        e.banque = Some(Banque { bic: (*bic).clone(), iban: (*iban).clone() });
    }
    e.france_travail_dossier_recouvrement =
        blocs20.iter().find(|b| b.organisme == Some(crate::dsn::SIRET_FRANCE_TRAVAIL) && !b.affectation.is_empty()).map(|b| b.affectation.clone());
    // QUI LES ORGANISMES CONTACTENT : le contact chez le declare de type 04 (« recouvrant, entre autres, des cotisations de
    // Securite sociale ») s'il y en a plusieurs, sinon le premier : le `declarant_dsn` de la fiche, que le moteur ecrit la
    // (`dsn.rs`, `type_du_contact`). Sa civilite n'est pas dans ce bloc : elle se lit dans celle du contact de l'emetteur
    // (S10.G00.02.001, `bloc-S10.G00.02.txt` l. 33-40) quand l'emetteur est l'entreprise (meme Siren, comme pour la raison
    // sociale) et que son contact porte le meme nom et prenom (S10.G00.02.002) -- une inference : c'est alors la meme
    // personne.
    let contact = contacts.iter().find(|c| c.qualite.as_deref() == Some(crate::dsn::TYPE_CONTACT_ORDINAIRE)).or(contacts.first());
    if let Some(c) = contact.filter(|c| !c.nom_prenom.trim().is_empty()) {
        let mut d = c.clone();
        if emetteur_est_l_entreprise && nom_emetteur.as_deref() == Some(c.nom_prenom.as_str()) {
            if let Some(civ) = civilite_emetteur {
                d.civilite = Some(civ);
            }
        }
        e.declarant_dsn = Some(d);
    }
    // LA MEDECINE DU TRAVAIL INTERNE : « MT01 - MEDECINE INTERNE ETABLISSEMENT » en S21.G00.11.025 (norme 2027,
    // `dsn::spst_ecrit`) : la fiche garde le code de la DPAE, ou la medecine interne s'ecrit MT999
    // (`CodeMedecine::INTERNE`). L'identifiant d'un service externe ne se reprend pas : sa definition est dans une fiche
    // consigne non deposee, et qu'il soit le code de la DPAE serait une inference.
    if spst == "MT01" {
        e.medecine_travail = Some(CodeMedecine::lire(CodeMedecine::INTERNE)?);
    }
    Ok(DsnLue {
        logiciel,
        nature_envoi,
        emetteur: emetteur_siren,
        nature_declaration: nature,
        mois,
        type_declaration,
        fraction,
        ordre,
        remplace,
        depot,
        fichier: fichier.cloned(),
        lignes: lignes.len(),
        versements_organismes,
        entreprise: e,
        individus,
        contrats,
        versements,
        etablissements,
        nic_siege,
        adhesions,
        rattachements,
        rubriques,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ce qu'un contrat declare pour le mois et ce qu'il rappelle d'un mois deja declare : un bloc 51 d'une periode
    /// anterieure au mois principal est un rappel (`bloc-S21.G00.51.txt` l. 38-40 ; §2.1.3.6) ; un montant qui n'en est
    /// pas un refuse.
    #[test]
    fn un_contrat_dit_son_mois_et_ses_rappels() {
        let f = "S20.G00.05.005,'01082026'\nS21.G00.30.001,'1850575123456'\nS21.G00.30.002,'FAVART'\nS21.G00.40.001,'01082026'\nS21.G00.40.009,'FA1.26.13'\n\
                 S21.G00.62.001,'31082026'\nS21.G00.50.001,'31082026'\n\
                 S21.G00.51.001,'01082026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.011,'001'\nS21.G00.51.013,'150.50'\n\
                 S21.G00.51.001,'01082026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.011,'001'\nS21.G00.51.013,'49.50'\n\
                 S21.G00.53.001,'01'\nS21.G00.53.002,'2'\nS21.G00.53.003,'37'\n\
                 S21.G00.51.001,'01072026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.011,'001'\nS21.G00.51.013,'30.00'\n\
                 S21.G00.53.001,'01'\nS21.G00.53.002,'1'\nS21.G00.53.003,'37'\n";
        let d = lire(f.as_bytes(), None).unwrap();
        let numero = NumeroContrat::lire("FA1.26.13").unwrap();
        let nir = Nir::lire("1850575123456").unwrap();
        let (mois, rappel) = (d.mesures(Some(&nir), &numero, false), d.mesures(Some(&nir), &numero, true));
        assert_eq!((socle::num::q2(mois.brut.unwrap()).to_string(), mois.cachets.normalize().to_string(), mois.remunere), ("200.00".to_string(), "2".to_string(), true));
        assert_eq!((socle::num::q2(rappel.brut.unwrap()).to_string(), rappel.cachets.normalize().to_string()), ("30.00".to_string(), "1".to_string()));
        assert_eq!(d.contrats[0].fin_declaree.map(|j| j.iso()).as_deref(), Some("2026-08-31"));
        let faux = "S20.G00.05.005,'01082026'\nS21.G00.30.001,'1850575123456'\nS21.G00.50.001,'31082026'\nS21.G00.51.001,'01082026'\nS21.G00.51.010,'FA1.26.13'\nS21.G00.51.013,'12,5'\n";
        assert!(lire(faux.as_bytes(), None).unwrap_err().phrase.contains("« 12,5 », qui n'est pas un montant"));
    }

    /// UN VERSEMENT DE DEUX CONTRATS attribue a chacun ce que la norme rattache a son numero : ses remunerations
    /// (S21.G00.51.010, `bloc-S21.G00.51.txt` l. 91-98) et ses bases (S21.G00.78.006, `bloc-S21.G00.79.txt` l. 9-14) ;
    /// ses nets (bloc 50) et une base sans numero ne sont a aucun.
    #[test]
    fn un_versement_de_deux_contrats_attribue_a_chacun_ce_qui_le_nomme() {
        let f = "S20.G00.05.005,'01122025'\nS21.G00.30.001,'1800575123456'\nS21.G00.40.001,'05122025'\nS21.G00.40.009,'MO4.25.16'\n\
                 S21.G00.40.001,'19122025'\nS21.G00.40.009,'MO4.25.17'\nS21.G00.50.001,'31122025'\nS21.G00.50.002,'700.00'\n\
                 S21.G00.51.001,'05122025'\nS21.G00.51.010,'MO4.25.16'\nS21.G00.51.011,'001'\nS21.G00.51.013,'525.00'\n\
                 S21.G00.51.001,'19122025'\nS21.G00.51.010,'MO4.25.17'\nS21.G00.51.011,'001'\nS21.G00.51.013,'350.00'\n\
                 S21.G00.78.001,'02'\nS21.G00.78.004,'525.00'\nS21.G00.78.006,'MO4.25.16'\n\
                 S21.G00.78.001,'02'\nS21.G00.78.004,'350.00'\nS21.G00.78.006,'MO4.25.17'\n\
                 S21.G00.78.001,'31'\nS21.G00.78.004,'875.00'\n";
        let p = lire(f.as_bytes(), None).unwrap().paies();
        let dit = |i: usize| (p[i].numero.as_str(), p[i].mesures.brut.map(|d| socle::num::q2(d).to_string()), p[i].bases.get(&BaseAssujettie::BrutePlafonnee).copied().map(|d| socle::num::q2(d).to_string()), p[i].bases.len(), p[i].nets.is_some(), p[i].avec.iter().map(ToString::to_string).collect::<Vec<String>>());
        assert_eq!(dit(0), ("MO4.25.16", Some("525.00".into()), Some("525.00".into()), 1, false, vec!["MO4.25.17".to_string()]));
        assert_eq!(dit(1), ("MO4.25.17", Some("350.00".into()), Some("350.00".into()), 1, false, vec!["MO4.25.16".to_string()]));
    }
}
