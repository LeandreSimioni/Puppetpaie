//! Les routes de l'import du passe d'une compagnie : ce qu'il ferait, rien d'ecrit (`importation-apercu`), ou en est
//! la boite aux lettres (`importation-etat`), l'import lui-meme (`importer`, `import/ecrite.rs`) et la nature d'un
//! contrat importe (`importation-nature`).
//!
//! `importation-apercu` lit la boite aux lettres `import/` entiere (`{"dossier": ...}` en nomme une autre ;
//! `import::bulletins`) : le tri, les DSN et AEM qu'il trouve (`import/dsn_aem.rs`, par `apercu_lu`), puis
//! les bulletins de paie.
//!
//! `contrats_nouveaux` : les contrats des AEM deposees que le registre de l'import retient (`import::attestes`) : un
//! contrat du meme salarie dont les dates chevauchent les siennes l'atteste deja.
//!
//! LES FICHES QUE L'IMPORT PROPOSE sont des saisies de leur fiche (`declarations::champs::Saisie`), composees des formes
//! lues (`import::proposee::de`) ; la convention collective que les pieces declarent les accompagne
//! (`Apercu::convention`), et c'est sous elle que les metiers se resolvent.

use droit::fiche::Lue as _;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::import::dsn_aem::{self, SalarieDeclare};
use crate::import::proposee;
use crate::import::reponse::{en_mots, n_mots, DitPar, Ecart, EtatFiche, FicheImportee, Importation, Origine};
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, FORME, OPERATION, SALARIE};
use crate::routes::catalogue::Route;
use crate::routes::catalogue::{Ecran, Place};
use crate::routes::enveloppe::{self, en_json, Charge, Dite, Porte, Rendu};
use declarations::champs::{Saisie, Valeur};
use droit::contexte::Contexte;
use socle::echec::{erreur, Classe, Echec, R};

const DOSSIER: Champ = Champ { nom: "dossier", libelle: Affiche::new("Dossier"), forme: Forme::Texte, sens: "le dossier choisi ; la boîte import/ de la compagnie sinon", avance: true };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Import);

/// Les routes de ce module, et leurs fiches : dans l'ordre d'un import (regarder, reprendre, dire la nature).
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::ImportationEtat,
        geste: "Dire ce qui est repris d'un ancien logiciel, et la boîte où déposer.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "importe_le, contrats, salaries, du, au, dossier (la boîte import/), reprises, a_dire (les natures à dire), editeur",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ImportationApercu,
        geste: "Dire ce qu'un import reprendrait (bulletins, DSN, AEM), sans rien écrire.",
        charge: &[DOSSIER],
        exemple: r#"{}"#,
        rend: "le tri des pièces (chacune son sort), entreprise et salaries (les fiches à créer ou compléter), les écarts, et leurs pastilles",
        refus: &[Refus { classe: Classe::Refuse, corriger: "le dossier, ou les pièces que la phrase nomme" }, FORME],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Importer,
        geste: "Reprendre les pièces d'un ancien logiciel : fiches, engagements importés.",
        charge: &[DOSSIER],
        exemple: r#"{}"#,
        rend: "ce que importation-apercu rend, écrit : les fiches créées ou complétées, les mois importés",
        refus: &[Refus { classe: Classe::Refuse, corriger: "le dossier, ou les pièces que la phrase nomme ; rien n'est écrit" }, FORME],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ImportationNature,
        geste: "Dire la nature des contrats repris d'un permanent, qu'aucune pièce ne dit.",
        charge: &[
            SALARIE,
            Champ { nom: "nature", libelle: Affiche::new("Nature du contrat"), forme: Forme::Texte, sens: "la nature d'un contrat de permanent (cdi, cdd…)", avance: false },
            Champ { nom: "debut", libelle: Affiche::new("Début"), forme: Forme::Date, sens: "le contrat court encore : son début", avance: true },
            Champ { nom: "fin", libelle: Affiche::new("Terme"), forme: Forme::Date, sens: "son terme, s'il en a un", avance: true },
            Champ { nom: "numero", libelle: Affiche::new("Numéro déclaré"), forme: Forme::Texte, sens: "son numéro, tel que la DSN de l'ancien logiciel l'a déclaré", avance: true },
            Champ { nom: "salaire", libelle: Affiche::new("Salaire mensuel"), forme: Forme::Nombre, sens: "son salaire mensuel", avance: true },
            Champ { nom: "quotite", libelle: Affiche::new("Heures par mois"), forme: Forme::Nombre, sens: "sa durée du travail, en heures par mois", avance: true },
        ],
        exemple: r#"{"salarie":"TARTEMPION","nature":"cdi"}"#,
        rend: "ecrits, a_dire (ce qui reste à dire), permanents_declares",
        refus: &[Refus { classe: Classe::Refuse, corriger: "le salarié, la nature ou le champ que la phrase nomme ; rien n'est écrit" }],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Une route de ce module.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    // Chaque PDF de la boite aux lettres se releve une fois pour toute la route (`import::releve`).
    let _releves = crate::import::releve::Releves::ouvrir();
    let ctx = porte.ctx;
    match porte.route {
        // `{"dossier": ...}` (vide ou absent : `import/` de la compagnie) : la boite aux lettres entiere, bulletins
        // compris (`import::bulletins`).
        Route::ImportationApercu => en_json(crate::importation::bulletins::apercu_dossier(ctx, &dossier_import(ctx, &charge.lire()?)?)?),
        Route::ImportationEtat => {
            let _: super::SansCharge = charge.lire()?;
            en_json(etat_importation(&ctx.racine)?)
        }
        Route::Importer => en_json(crate::importation::ecrite::importer(ctx, &charge.lire()?)?),
        // La nature des contrats repris qu'aucun bulletin ne dit.
        Route::ImportationNature => en_json(crate::importation::nature::dire(ctx, &charge.lire()?)?),
        autre => Err(enveloppe::route_inconnue(autre.nom())),
    }
}

/// La charge de `importation-apercu` et d'`importer` : le dossier a lire (vide ou absent : la boite aux lettres).
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeDossier {
    dossier: Option<String>,
}

/// Le dossier a trier : `charge.dossier`, ou `import/` a la racine de la compagnie s'il est vide ou absent.
///
/// LA BOITE AUX LETTRES DE LA COMPAGNIE SE CREE VIDE : `import/` qui n'existe pas encore est
/// cree, et l'apercu (ou l'import) n'a rien a reprendre, au lieu de refuser « dossier absent » ; de meme quand l'ecran le
/// nomme (`charge.dossier`, le chemin que `importation-etat` lui a donne). Un AUTRE dossier donne qui n'existe pas reste
/// refuse (`import::tri::trier`).
pub(crate) fn dossier_import(ctx: &Contexte, charge: &ChargeDossier) -> R<PathBuf> {
    let boite = ctx.racine.join("import");
    let d = match charge.dossier.as_deref().filter(|s| !s.trim().is_empty()) {
        None => boite.clone(),
        Some(s) => PathBuf::from(crate::routes::depots::normaliser(s)),
    };
    // Le dossier nomme est-il la boite : sous la racine de la compagnie, sous son nom. Un parent qui ne se resout pas
    // n'est pas la racine.
    let la_boite = d == boite
        || (d.file_name() == boite.file_name()
            && match (d.parent().map(Path::canonicalize), ctx.racine.canonicalize()) {
                (Some(Ok(p)), Ok(r)) => p == r,
                _ => false,
            });
    if la_boite && !d.exists() {
        std::fs::create_dir_all(&d).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&d, &e)))?;
    }
    Ok(d)
}

/// Un dossier de travail temporaire du systeme, propre a cet appel (a effacer avant de rendre).
pub(crate) fn dossier_de_travail() -> PathBuf {
    // Les nanosecondes depuis 1970 (avant : leur distance) font la marque.
    let marque = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_nanos(),
        Err(e) => e.duration().as_nanos(),
    };
    std::env::temp_dir().join(format!("puppetpaie-tri-{}-{}", std::process::id(), marque))
}

/// Chaque mot d'un texte avec sa première lettre en capitale ; rien s'il manque.
pub(crate) fn titre_de(v: Option<&str>) -> Option<String> {
    v.map(str::trim).filter(|t| !t.is_empty()).map(declarations::champs::titre)
}

/// Les champs DECLARES que la DSN disait autrement que la fiche.
fn ecarts(nom: &str, fiche: &Saisie, declare: &Saisie, mois: &str) -> R<Vec<Ecart>> {
    let mut out = Vec::new();
    for c in declarations::champs::fiche(nom)? {
        let (Some(b), Some(a)) = (declare.case(&c.cle).filter(|_| c.declare), fiche.case(&c.cle)) else { continue };
        // Une case vide ou « A COMPLETER » n'est pas un ecart : l'import la complete (`import::completer`).
        if crate::importation::completer::vide(Some(a)) || en_mots(a) == en_mots(b) {
            continue;
        }
        // Une structure (la banque) ne differe que par ce que la DSN en declare : ses notes et autres cases restent.
        if let (Valeur::Bloc(x), Valeur::Bloc(y)) = (a, b) {
            if y.cases().iter().all(|(k, v)| x.case(k).map(en_mots) == Some(en_mots(v))) {
                continue;
            }
        }
        out.push(Ecart::new(&c.cle, a.clone(), DitPar::Dsn { dsn: b.clone(), mois: mois.to_string() }));
    }
    Ok(out)
}

/// La fiche d'un salarie telle qu'elle est, case par case.
fn vue_salarie(ctx: &Contexte, cle: &str) -> R<Saisie> {
    proposee::de("salarie", &ctx.salaries[cle].champs)
}

/// CE QU'UNE FICHE NEUVE RECOIT D'UNE DSN, a cote de son identite et de son emploi (`SalarieDeclare::fiche`).
#[derive(Serialize)]
struct SalarieDeLaDsn<'a> {
    nir: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    nir_cle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initiales_contrat: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    date_anciennete: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    statut: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    emploi_affichage: Option<String>,
    actif: bool,
}

/// Ce qu'une fiche neuve recoit d'une DSN.
fn fiche_salarie(decl: &SalarieDeclare) -> R<Saisie> {
    let emploi = decl.emploi.as_ref();
    // Le statut de bareme se deduit du code emploi France Travail, a defaut de la PCS (354x : artistes ; une autre PCS :
    // technicien) ; sans l'un ni l'autre, rien ne le dit.
    let m = match emploi.and_then(|e| e.code_emploi_ft.as_deref()) {
        Some(code) => droit::emplois::metier(code)?,
        None => None,
    };
    let statut = match (&m, emploi.and_then(|e| e.pcs_ese.as_deref())) {
        (Some(m), _) => Some(m.statut().as_str()),
        (None, Some(pcs)) if pcs.starts_with("354") => Some("artiste"),
        (None, Some(_)) => Some("technicien"),
        (None, None) => None,
    };
    // L'EMPLOI EN CLAIR d'un metier : son libelle, au feminin pour une salariee (le sexe que porte son NIR), comme quand
    // on le choisit dans la fiche (`routes::mois::emploi`) ; la forme declaree reste celle de la DSN. Le metier est celui
    // du code emploi, sinon celui du catalogue que nomme l'intitule declare (S21.G00.40.006), s'il en nomme un seul
    // (`emplois::par_intitule`, comme pour un bulletin). Le libelle ne depend ni de la convention ni de la qualite de
    // cadre : il se lit sur le metier.
    let du_catalogue = match m {
        Some(m) => Some(m),
        None => match emploi.and_then(|e| e.emploi.as_deref()) {
            Some(intitule) => match droit::emplois::par_intitule(intitule, crate::importation::bulletins::compare)? {
                Some((c, _)) => droit::emplois::metier(&c)?,
                None => None,
            },
            None => None,
        },
    };
    let femme = declarations::ntt::sexe_de(Some(decl.nir.as_str()), None) == Some("2");
    let emploi_affichage = du_catalogue.map(|m| if femme { m.feminin() } else { m.libelle() }).filter(|l| !l.is_empty());
    let de_la_dsn = SalarieDeLaDsn {
        nir: &decl.nir,
        nir_cle: declarations::champs::cle_nir(&decl.nir),
        initiales_contrat: Some(decl.initiales.as_str()).filter(|i| !i.is_empty()),
        date_anciennete: decl.premier_contrat.map(|j| j.iso()),
        statut,
        emploi_affichage,
        actif: true,
    };
    let mut f = decl.fiche()?;
    proposee::fondre(&mut f, &proposee::de("salarie", &de_la_dsn)?);
    f.comblee("salarie")
}

/// LA FICHE ENTREPRISE QUE LES PIECES DONNENT, a cote de ce qu'elles declarent : son Siret, sa licence, son nom en clair.
/// La fiche porte un numero de licence, comme quand on le saisit : le premier que les pieces donnent (`licences` :
/// chaque numero lu, avec le mois ou il apparait, du plus ancien au plus recent), qui vaut de tout temps ; ceux qui le
/// suivent se datent du mois ou les pieces le changent, a l'ecriture (`Apercu::licences`,
/// `routes::fiches::dater_licences_suivantes`), comme un numero renouvele se saisit « a partir du ».
#[derive(Serialize, Default)]
pub(crate) struct EntrepriseDesPieces {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) siret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) licence_spectacle: Option<String>,
}

/// La fiche entreprise `f`, comblee de ses formes lisibles ; son nom en clair, celui de sa raison sociale en capitales
/// initiales, quand rien ne le dit.
pub(crate) fn entreprise_comblee(f: &Saisie) -> R<Saisie> {
    let mut f = f.comblee("entreprise")?;
    if f.mot("nom_affichage").is_none() {
        if let Some(t) = titre_de(f.mot("raison_sociale").as_deref()) {
            #[derive(Serialize)]
            struct Nom {
                nom_affichage: String,
            }
            proposee::fondre(&mut f, &proposee::de("entreprise", &Nom { nom_affichage: t })?);
        }
    }
    Ok(f)
}

/// La fiche Entreprise que les DSN donnent : ce qu'elles declarent de l'entreprise, son Siret, sa premiere licence.
fn fiche_entreprise(lecture: &dsn_aem::LectureDsn) -> R<Saisie> {
    let mut e = dsn_aem::fiche_entreprise(&lecture.entreprise)?;
    let siret = match (&lecture.entreprise.siren, &lecture.entreprise.nic) {
        (Some(s), Some(n)) => Some(format!("{}{}", s, n)),
        _ => None,
    };
    let pieces = EntrepriseDesPieces { siret, licence_spectacle: lecture.licences.first().map(|l| l.1.clone()) };
    proposee::fondre(&mut e, &proposee::de("entreprise", &pieces)?);
    entreprise_comblee(&e)
}

/// Une fiche que l'import creerait : ou elle va dans la reponse, et ce qu'elle porte.
pub(crate) struct ACreer {
    pub(crate) nom: &'static str,
    /// Le rang dans `salaries` ou `spectacles` de la reponse ; rien pour l'entreprise.
    pub(crate) rang: Option<usize>,
    pub(crate) donnees: Saisie,
}

impl ACreer {
    /// Le NIR de la fiche d'un salarie a creer.
    pub(crate) fn nir(&self) -> Option<String> {
        self.donnees.mot("nir")
    }
}

/// Ce que l'import ferait : la reponse (`sortie_json.importation`), et ce dont l'ecriture a besoin.
pub(crate) struct Apercu {
    pub(crate) reponse: Importation,
    pub(crate) a_creer: Vec<ACreer>,
    /// Les fiches existantes que l'import completerait (`import::completer`).
    pub(crate) a_completer: Vec<crate::importation::completer::ACompleter>,
    pub(crate) contrats: Vec<dsn_aem::ContratAtteste>,
    /// Les versements declares par les DSN, rattaches a une fiche (ou `<a creer>`).
    pub(crate) paies: Vec<engagement::importe::PaieLue>,
    /// LES CONTRATS QUE L'IMPORT RETIENT, de toutes les pieces (AEM deposees, AEM en PDF, bulletins, certificats) : le
    /// registre de l'import les a dits nouveaux (`import::attestes`). Chacun avec le NIR qui le rattache (`salarie` vaut
    /// `<a creer>` tant que la fiche n'existe pas) et le bloc du compte rendu qui le compte.
    pub(crate) contrats_attestes: Vec<crate::import::attestes::Retenu>,
    /// CE QUE L'IMPORT CHANGE AUX CONTRATS DE L'HISTORIQUE : le contrat qu'un certificat atteste, reduit aux mois qu'il atteste
    /// seul ou retire, celui d'une AEM que le bulletin remplace (`import::attestes`).
    pub(crate) contrats_changes: Vec<crate::import::attestes::Change>,
    /// La suite des licences que les pieces donnent (mois `AAAA-MM`, numero), la plus ancienne d'abord.
    pub(crate) licences: Vec<(String, String)>,
    /// Les contrats que la DSN declare payes sans que les nets de leur versement se rattachent a eux (`PaieDuContrat::nets`).
    pub(crate) sans_nets: Vec<SansNets>,
    /// LA CONVENTION COLLECTIVE QUE LES DSN DECLARENT pour l'entreprise (S21.G00.11.022, a defaut S21.G00.40.017 :
    /// `declarations::dsn_lue`), la plus recente ; rien si aucune ne la dit.
    pub(crate) convention: Option<String>,
}

/// UN CONTRAT QUE LA DSN DECLARE SANS SES NETS : son versement paie aussi d'autres contrats (`avec`), ou plusieurs
/// versements le paient (`avec` vide). La DSN lui attribue son brut et ses bases ; ses nets et son prelevement viennent de
/// son bulletin, et sans lui l'apercu le dit (`phrase`).
#[derive(Clone, Debug)]
pub(crate) struct SansNets {
    pub(crate) mois: String,
    pub(crate) nir: String,
    pub(crate) contrat: String,
    pub(crate) qui: String,
    pub(crate) avec: Vec<String>,
}

impl SansNets {
    /// Ce que l'apercu dit d'un tel contrat qu'aucun bulletin ne complete.
    pub(crate) fn phrase(&self) -> String {
        let pourquoi = match self.avec.as_slice() {
            [] => "plusieurs versements de la DSN le paient".to_string(),
            [un] => format!("son versement dans la DSN paie aussi le contrat {}", un),
            autres => format!("son versement dans la DSN paie aussi les contrats {}", autres.join(", ")),
        };
        format!(
            "{}, contrat {} ({}) : {}, sans dire ses nets ni son prélèvement à la source, et aucun bulletin ne les donne. L'engagement est repris sans ses nets : importez son bulletin.",
            self.qui,
            self.contrat,
            socle::mois::Mois::en_clair(&self.mois),
            pourquoi
        )
    }
}

/// Le salarie qu'une DSN declare, en clair : son nom, son prenom.
fn libelle_de(i: &declarations::dsn_lue::IndividuDeclare) -> String {
    [i.nom.clone(), i.prenom.as_deref().map(declarations::champs::titre)].into_iter().flatten().collect::<Vec<_>>().join(" ")
}

/// LE NOM QUE LA PERSONNE A DONNE A LA COMPAGNIE (« Ajouter une compagnie ») reste le sien : la fiche que l'import cree
/// garde son nom et sa raison sociale, que les pieces ecrivent en capitales (S21.G00.06.003) ; changer le nom d'une
/// fiche demande une date d'effet que l'import ne devine pas.
pub(crate) fn garder_le_nom(ctx: &Contexte, e: &mut Saisie) -> R<()> {
    #[derive(Serialize)]
    struct Nom {
        #[serde(skip_serializing_if = "Option::is_none")]
        nom_affichage: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        raison_sociale: Option<String>,
    }
    let dit = |v: Option<String>| v.filter(|v| !v.trim().is_empty());
    let nom = Nom {
        nom_affichage: dit(ctx.entreprise.nom_affichage.remplie().map(str::to_string)),
        raison_sociale: dit(ctx.entreprise.raison_sociale.en_texte().remplie().map(str::to_string)),
    };
    proposee::fondre(e, &proposee::de("entreprise", &nom)?);
    Ok(())
}

/// La cle d'une fiche a creer, tant qu'elle n'existe pas.
pub(crate) const A_CREER: &str = "<a creer>";

/// Les paies versees des engagements importes, une par mois.
pub(crate) fn paies_historique(ctx: &Contexte) -> R<Vec<engagement::importe::PaieLue>> {
    engagement::importe::paies_lues(&ctx.racine).map_err(Echec::from)
}

/// UN CONTRAT QU'UN VERSEMENT DE LA DSN PAIE, EN PAIE VERSEE : ses dates, et ce que la DSN lui attribue
/// (`dsn_aem::PaieDeclaree`), tel qu'elle l'ecrit (`declarations::dsn_lue::lire`, qui refuse une DSN dont un montant ou
/// une date ne se lit pas). Son activite (53) n'y passe pas : les heures, cachets et jours d'une paie sont ceux qu'un
/// bulletin atteste en unites entieres pour le mois d'un permanent, ceux d'un intermittent sont a son contrat (AEM,
/// bulletin) ; la DSN les declare en decimal.
fn paie_declaree(p: &dsn_aem::PaieDeclaree, objet: Option<String>) -> engagement::importe::PaieVersee {
    use engagement::importe::{Declaration, PaieVersee};
    let (c, d) = (p.contrat.as_ref(), &p.paie);
    let mut x = PaieVersee::vide(d.numero.clone(), Declaration::Dsn);
    x.nir = Some(d.nir.clone());
    x.numero_objet = objet;
    (x.debut, x.fin, x.verse_le, x.quotite) = (c.and_then(|c| c.debut), c.and_then(|c| c.fin), d.verse_le, c.and_then(|c| c.quotite));
    (x.brut, x.bases) = (d.mesures.brut, d.bases.clone());
    if let Some(n) = &d.nets {
        (x.net_imposable, x.net_verse) = (n.net_imposable, n.net_verse);
        (x.pas, x.taux_pas, x.type_pas, x.identifiant_pas) = (n.pas, n.taux_pas, n.type_pas.clone(), n.identifiant_pas.clone());
    }
    (x.depose_le, x.fichier) = (p.depot.map(|j| j.iso()), Some(p.fichier.clone()).filter(|f| !f.is_empty()));
    x
}

/// UN SPECTACLE QUE L'IMPORT CREE d'un numero d'objet qu'aucune fiche ne porte : sans titre (les pieces n'en impriment
/// pas, ou il se propose a valider : `declarations::demarrage::TITRE_PROPOSE`), actif s'il a ete declare dans l'annee.
#[derive(Serialize)]
pub(crate) struct SpectacleDesPieces {
    pub(crate) titre: &'static str,
    pub(crate) numero_objet: String,
    pub(crate) alias: Vec<String>,
    pub(crate) actif: bool,
    pub(crate) note: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) code_analytique: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) titre_propose: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) titre_lu_dans: Option<&'static str>,
}

/// L'apercu de ce qu'un dossier de DSN et un dossier d'AEM donneraient (l'un ou l'autre peut manquer : rien n'en
/// vient). `a_venir` : des NIR dont une fiche sera creee d'ailleurs (les bulletins) -- un contrat atteste qui les
/// porte n'est pas « sans fiche ».
pub(crate) fn apercu_lu(
    ctx: &Contexte,
    source: &str,
    dsn: Option<&Path>,
    aem: Option<&Path>,
    aujourdhui: socle::date::Jour,
    a_venir: &HashSet<String>,
) -> R<Apercu> {
    let lecture = match &dsn {
        Some(d) => dsn_aem::lire_dsn(d)?,
        None => dsn_aem::LectureDsn::aucune(),
    };
    let mut avertissements: Vec<String> = lecture.illisibles.iter().map(|f| format!("DSN illisible, ignoree : {}", f)).collect();
    // LA FICHE NE PORTE QU'UN ETABLISSEMENT (celui de son NIC, `champs-base.json`, `etablissements`) : des DSN qui en
    // declarent plusieurs, ou dont le siege n'est pas l'etablissement declare, le disent.
    if lecture.etablissements.len() > 1 {
        avertissements.push(format!(
            "les DSN déclarent plusieurs établissements (S21.G00.11.001 : {}) : la fiche Entreprise n'en porte qu'un, celui de son NIC ; le moteur ne déclare que celui-là",
            lecture.etablissements.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    if lecture.nics_siege.iter().any(|n| !lecture.etablissements.contains(n)) && !lecture.etablissements.is_empty() {
        avertissements.push(format!(
            "le siège de l'entreprise (S21.G00.06.002 : {}) n'est pas l'établissement déclaré (S21.G00.11.001) : la fiche Entreprise porte l'établissement, et le moteur déclare son NIC aux deux places",
            lecture.nics_siege.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    // LES CONVENTIONS QUI DIVERGENT, d'un contrat a l'autre ou d'une DSN a l'autre (`dsn_aem::LectureDsn::conventions`).
    avertissements.extend(lecture.conventions_divergentes());

    // 1. L'entreprise : seulement si la fiche est vide.
    let mut entreprise: Option<FicheImportee> = None;
    let mut a_creer = 0usize;
    let mut a_creer_liste: Vec<ACreer> = Vec::new();
    let mut a_completer: Vec<crate::importation::completer::ACompleter> = Vec::new();
    let vide = Saisie::default();
    if !lecture.entreprise.est_vide() {
        let mut e = fiche_entreprise(&lecture)?;
        garder_le_nom(ctx, &mut e)?;
        if ctx.entreprise.siren.is_none() {
            let anomalies = crate::importation::fiches::verifier_comme_a_l_ecriture("entreprise", &e)?;
            let libelle = match e.mot("raison_sociale") {
                Some(r) => r,
                None => "?".to_string(),
            };
            entreprise = Some(FicheImportee::new("entreprise", None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &e));
            a_creer_liste.push(ACreer { nom: "entreprise", rang: None, donnees: e });
            a_creer += 1;
        } else {
            let libelle = match ctx.entreprise.raison_sociale.portee() {
                Some(r) => r.to_string(),
                None => "?".to_string(),
            };
            let fiche = proposee::de("entreprise", &ctx.entreprise)?;
            let ec = ecarts("entreprise", &fiche, &dsn_aem::fiche_entreprise(&lecture.entreprise)?, &lecture.dernier_mois)?;
            let mut f = FicheImportee::new("entreprise", None, &libelle, EtatFiche::Existante, ec, Vec::new(), &vide);
            let champs = crate::importation::completer::champs_a_completer("entreprise", &fiche, &e)?;
            f.noter(&champs, &Origine::dsn(&lecture.dernier_mois));
            entreprise = Some(f);
            crate::importation::completer::ajouter(&mut a_completer, "entreprise", None, &champs);
        }
    }

    // 2. Les salaries, par NIR.
    let mut par_nir: HashMap<String, String> = HashMap::new();
    for (cle, s) in &ctx.salaries {
        if let Some(n) = s.nir.dite().filter(|n| !n.is_empty()) {
            par_nir.insert(n.to_string(), cle.to_string());
        }
    }
    let mut ordre: Vec<&SalarieDeclare> = lecture.salaries.iter().collect();
    ordre.sort_by(|a, b| a.mois.cmp(&b.mois));
    let mut salaries = Vec::new();
    let mut cles_a_venir: HashSet<String> = HashSet::new();
    for decl in ordre {
        let libelle = libelle_de(&decl.individu);
        if let Some(cle) = par_nir.get(&decl.nir) {
            let fiche = vue_salarie(ctx, cle)?;
            let ec = ecarts("salarie", &fiche, &decl.fiche()?, &decl.mois)?;
            let mut entree = FicheImportee::new("salarie", Some(cle), &libelle, EtatFiche::Existante, ec, Vec::new(), &vide);
            let champs = crate::importation::completer::champs_a_completer("salarie", &fiche, &fiche_salarie(decl)?)?;
            entree.noter(&champs, &Origine::dsn(&decl.mois));
            crate::importation::completer::ajouter(&mut a_completer, "salarie", Some(cle), &champs);
            salaries.push(entree);
            continue;
        }
        let donnees = fiche_salarie(decl)?;
        let anomalies = crate::importation::fiches::verifier_comme_a_l_ecriture("salarie", &donnees)?;
        salaries.push(FicheImportee::new("salarie", None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &donnees));
        a_creer_liste.push(ACreer { nom: "salarie", rang: Some(salaries.len() - 1), donnees });
        cles_a_venir.insert(decl.nir.clone());
        a_creer += 1;
    }

    cles_a_venir.extend(a_venir.iter().filter(|n| !par_nir.contains_key(*n)).cloned());

    // 3. Les spectacles, par numero d'objet -- ceux des DSN et ceux des AEM.
    let (contrats, illisibles) = match &aem {
        Some(d) => dsn_aem::lire_aem(d)?,
        None => (Vec::new(), Vec::new()),
    };
    avertissements.extend(illisibles.iter().map(|f| format!("AEM illisible, ignoree : {}", f)));
    let n_aem = contrats.iter().map(|c| c.fichier.clone()).collect::<HashSet<_>>().len();
    struct Objet {
        numero: String,
        premier: String,
        dernier: String,
        contrats: i64,
        aem: bool,
    }
    let mut objets: Vec<Objet> = lecture
        .objets
        .iter()
        .map(|o| Objet { numero: o.numero.clone(), premier: o.premier_mois.clone(), dernier: o.dernier_mois.clone(), contrats: o.contrats, aem: o.source == "AEM" })
        .collect();
    for c in &contrats {
        if c.numero_objet.is_empty() {
            continue;
        }
        let m = c.fin.mois_civil().to_string();
        let i = match objets.iter().position(|o| o.numero == c.numero_objet) {
            Some(i) => i,
            None => {
                objets.push(Objet { numero: c.numero_objet.clone(), premier: m.clone(), dernier: m.clone(), contrats: 0, aem: true });
                objets.len() - 1
            }
        };
        if objets[i].aem {
            if m > objets[i].dernier {
                objets[i].dernier = m;
            }
            objets[i].contrats += 1;
        }
    }
    let mut connus: HashMap<String, droit::contexte::IdSpectacle> = HashMap::new();
    for (cle, sp) in &ctx.spectacles {
        if let Some(n) = sp.numero_objet.portee() {
            connus.insert(n.to_uppercase(), cle.clone());
        }
    }
    objets.sort_by(|a, b| a.premier.cmp(&b.premier));
    let seuil = format!("{:04}-{:02}", aujourdhui.an - 1, aujourdhui.mois);
    let mut spectacles = Vec::new();
    for o in &objets {
        if let Some(cle) = connus.get(&o.numero.to_uppercase()) {
            let titre = match ctx.spectacles[cle].titre.portee() {
                Some(t) => t.to_string(),
                None => "?".to_string(),
            };
            spectacles.push(FicheImportee::new("spectacle", Some(cle), &format!("{} ({})", titre, o.numero), EtatFiche::Existante, Vec::new(), Vec::new(), &vide));
            continue;
        }
        let source = if o.aem { "AEM" } else { "DSN" };
        let sp = SpectacleDesPieces {
            titre: crate::import::proposee::A_COMPLETER,
            numero_objet: o.numero.clone(),
            alias: Vec::new(),
            actif: o.dernier >= seuil,
            note: format!("{} de {} a {} ({} contrat(s))", source, o.premier, o.dernier, o.contrats),
            code_analytique: None,
            titre_propose: None,
            titre_lu_dans: None,
        };
        let donnees = proposee::de("spectacle", &sp)?;
        let anomalies = crate::importation::fiches::verifier_comme_a_l_ecriture("spectacle", &donnees)?;
        spectacles.push(FicheImportee::new(
            "spectacle",
            None,
            &format!("numéro d'objet {}, sans titre (déclarations de {} à {})", o.numero, o.premier, o.dernier),
            EtatFiche::ACreer,
            Vec::new(),
            anomalies,
            &donnees,
        ));
        a_creer_liste.push(ACreer { nom: "spectacle", rang: Some(spectacles.len() - 1), donnees });
        a_creer += 1;
    }

    // 4. L'historique : les contrats attestes, rattaches par NIR aux fiches -- existantes ou a creer. Lesquels sont
    // nouveaux, c'est le registre de l'import qui le dit (`import::attestes`).
    let sans_fiche = contrats.iter().filter(|c| !par_nir.contains_key(&c.nir) && !cles_a_venir.contains(&c.nir)).count();
    let contrats_du = contrats.iter().map(|c| c.debut).min().map(|d| d.iso());
    let contrats_au = contrats.iter().map(|c| c.fin).max().map(|d| d.iso());
    if sans_fiche > 0 {
        avertissements.push(format!(
            "{} contrat(s) atteste(s) portent un NIR qu'aucune DSN ne declare : ils ne seront pas rattaches a une fiche, donc pas comptes dans les droits. Creer la fiche a la main, puis reimporter.",
            sans_fiche
        ));
    }

    // 5. Les paies declarees par les DSN : ce que chaque versement a porte.
    let deja_paies: HashSet<(String, String, String)> = paies_historique(ctx)?.iter().map(engagement::importe::PaieLue::cle).collect();
    let cle_de = |nir: &str| match par_nir.get(nir) {
        Some(c) => c.clone(),
        None => A_CREER.to_string(),
    };
    let mut paies_nouvelles = 0usize;
    for (mois, nir, contrat) in lecture.paies.keys() {
        if !par_nir.contains_key(nir) && !cles_a_venir.contains(nir) {
            continue;
        }
        if !deja_paies.contains(&(mois.clone(), cle_de(nir), contrat.clone())) {
            paies_nouvelles += 1;
        }
    }
    let rien_a_faire = a_creer == 0 && a_completer.is_empty() && paies_nouvelles == 0;
    // DEUX SOURCES POUR UNE LIGNE -- les montants viennent de la DSN, le numero d'objet peut
    // venir de l'AEM (rapprochee sur salarie, debut, fin).
    let objets_aem: HashMap<(String, String, String), String> = contrats
        .iter()
        .filter(|c| !c.numero_objet.is_empty())
        .map(|c| ((c.nir.clone(), c.debut.iso(), c.fin.iso()), c.numero_objet.clone()))
        .collect();
    let mut paies: Vec<engagement::importe::PaieLue> = Vec::new();
    let mut sans_nets: Vec<SansNets> = Vec::new();
    for ((mois, nir, contrat), v) in &lecture.paies {
        if !par_nir.contains_key(nir) && !cles_a_venir.contains(nir) {
            continue;
        }
        let c = v.contrat.as_ref();
        let objet = c.and_then(|c| c.objet.clone()).or_else(|| {
            let (debut, fin) = (c.and_then(|c| c.debut)?, c.and_then(|c| c.fin)?);
            objets_aem.get(&(nir.clone(), debut.iso(), fin.iso())).cloned()
        });
        if v.paie.nets.is_none() {
            let qui = match lecture.salaries.iter().find(|s| s.nir == *nir) {
                Some(s) => libelle_de(&s.individu),
                None => nir.clone(),
            };
            sans_nets.push(SansNets { mois: mois.clone(), nir: nir.clone(), contrat: contrat.clone(), qui, avec: v.paie.avec.clone() });
        }
        // 50.004 est le net verse, pas le net a payer : la DSN ne declare pas celui-ci, un bulletin le donnera.
        let paie = paie_declaree(v, objet);
        paies.push(engagement::importe::PaieLue { mois: mois.clone(), salarie: cle_de(nir), paie });
    }
    let reponse = Importation {
        source: source.to_string(),
        dsn: lecture.fichiers,
        aem: n_aem,
        premier_mois: lecture.premier_mois.clone(),
        dernier_mois: lecture.dernier_mois.clone(),
        entreprise,
        salaries,
        spectacles,
        contrats: contrats.len(),
        contrats_du,
        contrats_au,
        contrats_sans_fiche: sans_fiche,
        avertissements,
        rien_a_faire,
        ..Importation::default()
    };
    Ok(Apercu {
        reponse,
        a_creer: a_creer_liste,
        a_completer,
        contrats,
        paies,
        contrats_attestes: Vec::new(),
        contrats_changes: Vec::new(),
        licences: lecture.licences.clone(),
        sans_nets,
        convention: lecture.entreprise.idcc.clone(),
    })
}

/// `etat_importation` : ce qui a deja ete repris, d'apres les engagements importes, pour que la page le dise :
/// - `importe_le`, `contrats`, `salaries`, `objets`, `du`, `au` : les contrats attestes ;
/// - `dossier` : la boite aux lettres `import/` de la compagnie, que la page preremplit ;
/// - `reprises` : chaque source reprise -- `DSN` (les versements declares par une DSN ; `declarations` : une DSN par
///   mois declare), `AEM` (les contrats attestes par une AEM), `bulletins` (les paies qu'un bulletin a ecrites ou
///   completees, les contrats qu'il atteste, les PDF copies dans `historique/bulletins/`) --, avec combien, de quand a
///   quand, et le jour de la reprise ;
/// - `editeur` : ce qui attend l'editeur -- les incidents et les signalements partis (ou a partir) de ce poste qui
///   parlent de l'import, sans valeur personnelle (ils ont ete caviardes avant d'etre ecrits) ; `attend_editeur` en
///   compte ceux qui n'ont pas de reponse. Un journal des incidents ou une boite des messages qui ne se lit pas se
///   refuse.
fn etat_importation(racine: &Path) -> R<EtatImportation> {
    let contrats = engagement::importe::contrats_lus(racine).map_err(Echec::from)?;
    let paies = engagement::importe::paies_lues(racine).map_err(Echec::from)?;
    let plein = |t: &str| Some(t.to_string()).filter(|t| !t.is_empty());
    let importe_le = contrats.iter().filter_map(|c| c.contrat.importe_le.clone()).chain(paies.iter().filter_map(|p| p.paie.importe_le.clone())).filter(|t| !t.is_empty()).max();
    let debut = contrats.iter().filter_map(|c| c.debut.as_deref().and_then(plein)).min();
    let fin = contrats.iter().filter_map(|c| c.fin.as_deref().and_then(plein)).max();
    let salaries: HashSet<&str> = contrats.iter().map(|c| c.salarie.as_str()).filter(|s| !s.is_empty()).collect();
    let objets: HashSet<&str> = contrats.iter().filter_map(|c| c.contrat.numero_objet.as_deref()).collect();
    let editeur = attend_l_editeur(racine)?;
    let attend_editeur = editeur.iter().filter(|x| !x.repondu).count();
    let reprises = reprises_faites(racine, &contrats, &paies)?;
    // L'historique repris en une ligne (`repris_dit`), et son titre : le jour de la derniere reprise.
    let nombre = |source: &str| -> usize { reprises.iter().filter(|r| r.source == source).map(|r| r.nombre).sum() };
    let declarations: usize = reprises.iter().filter(|r| r.source == "DSN").filter_map(|r| r.declarations).sum();
    let quoi: Vec<String> = [
        n_mots(nombre("bulletins"), "bulletin"),
        n_mots(declarations + nombre("AEM"), "déclaration"),
        n_mots(nombre("certificats"), "certificat"),
        n_mots(contrats.len(), "contrat"),
    ]
    .into_iter()
    .filter(|x| !x.is_empty())
    .collect();
    let mut bornes: Vec<String> = debut
        .iter()
        .chain(fin.iter())
        .chain(reprises.iter().flat_map(|r| r.du.iter().chain(r.au.iter())))
        .map(|x| x.chars().take(7).collect())
        .collect();
    bornes.sort();
    let periode = match (bornes.first(), bornes.last()) {
        (Some(du), Some(au)) if du == au => format!(", en {}", socle::mois::Mois::en_clair(du)),
        (Some(du), Some(au)) => format!(", {} à {}", socle::texte::de(&socle::mois::Mois::en_clair(du)), socle::mois::Mois::en_clair(au)),
        _ => String::new(),
    };
    let repris_dit = (!quoi.is_empty()).then(|| format!("{}{}", quoi.join(", "), periode));
    let le = importe_le.iter().chain(reprises.iter().filter_map(|r| r.le.as_ref())).max();
    let repris_titre = match le {
        Some(l) => format!("Historique repris le {}", socle::ecran::quand(&l.chars().take(10).collect::<String>())),
        None => "Historique repris".to_string(),
    };
    let phrase = match (&importe_le, &debut, &fin) {
        (Some(le), Some(du), Some(au)) => {
            format!("Repris le {} : {} contrats, {} salariés, du {} au {}.", socle::ecran::quand(le), contrats.len(), salaries.len(), socle::ecran::quand(du), socle::ecran::quand(au))
        }
        (Some(le), _, _) => format!("Repris le {} : {} contrats, {} salariés.", socle::ecran::quand(le), contrats.len(), salaries.len()),
        (None, _, _) => "Rien n'est repris d'un ancien logiciel. Ses bulletins, DSN ou AEM se déposent dans le dossier d'import de la compagnie, puis s'importent.".to_string(),
    };
    Ok(EtatImportation {
        phrase,
        contrats: contrats.len(),
        salaries: salaries.len(),
        objets: objets.len(),
        importe_le,
        du: debut,
        au: fin,
        dossier: racine.join("import").display().to_string(),
        reprises,
        editeur,
        pastille_repris: socle::ecran::Pastille::de("fait", "fait"),
        attend_editeur,
        a_dire: crate::importation::nature::a_dire(racine)?,
        guide: crate::import::guide::guide(),
        repris_dit,
        repris_titre,
    })
}

/// `importation-etat` : ce qui a deja ete repris, et ce que la page en dit.
#[derive(Debug, Serialize)]
struct EtatImportation {
    phrase: String,
    importe_le: Option<String>,
    contrats: usize,
    salaries: usize,
    objets: usize,
    du: Option<String>,
    au: Option<String>,
    dossier: String,
    reprises: Vec<RepriseVue>,
    editeur: Vec<MessageEditeur>,
    /// La pastille de l'historique repris.
    pastille_repris: socle::ecran::Pastille,
    attend_editeur: usize,
    /// Les contrats repris dont la nature reste a dire, par salarie.
    a_dire: Vec<crate::importation::nature::ADire>,
    /// Ce qu'on met dans la boite aux lettres, ce qui s'en lit, ce qui restera a dire (`import::guide`).
    guide: crate::import::guide::Guide,
    repris_dit: Option<String>,
    repris_titre: String,
}

impl Dite for EtatImportation {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// Une source reprise, telle que la page la lit.
#[derive(Debug, Serialize)]
struct RepriseVue {
    source: &'static str,
    quoi: &'static str,
    nombre: usize,
    du: Option<String>,
    au: Option<String>,
    le: Option<String>,
    /// Les DSN reprises : une par mois declare.
    #[serde(skip_serializing_if = "Option::is_none")]
    declarations: Option<usize>,
    /// Les bulletins : les contrats qu'ils attestent, et les pieces copiees sous `historique/bulletins/`.
    #[serde(skip_serializing_if = "Option::is_none")]
    contrats: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    copies: Option<usize>,
}

/// Un incident ou un signalement de ce poste qui parle de l'import, tel que la page le lit.
#[derive(Debug, Serialize)]
struct MessageEditeur {
    canal: &'static str,
    quand: Option<String>,
    resume: String,
    parti: bool,
    repondu: bool,
    pastille: socle::ecran::Pastille,
    titre_dit: String,
}

/// Une source reprise : combien, de quand a quand, quel jour.
#[derive(Default)]
struct Reprise {
    nombre: usize,
    du: Option<String>,
    au: Option<String>,
    le: Option<String>,
}

/// `reprises` de `importation-etat` : DSN, AEM, certificats Congés Spectacles, bulletins, dans cet ordre, celles qui ont
/// repris quelque chose.
fn reprises_faites(racine: &Path, contrats: &[engagement::importe::ContratLu], paies: &[engagement::importe::PaieLue]) -> R<Vec<RepriseVue>> {
    use engagement::importe::{Attestation, Declaration};
    let (mut aem, mut contrats_bulletins, mut certificats) = (Reprise::default(), Reprise::default(), Reprise::default());
    for c in contrats {
        let cible = match c.contrat.atteste_par {
            Attestation::Bulletin => &mut contrats_bulletins,
            Attestation::Certificat => &mut certificats,
            Attestation::Aem => &mut aem,
        };
        cible.voir(c.debut.as_deref(), c.fin.as_deref(), c.contrat.importe_le.as_deref());
    }
    let (mut dsn, mut bulletins) = (Reprise::default(), Reprise::default());
    // Les DSN reprises : une par mois declare (la phrase de l'etat dit des declarations, pas des versements).
    let mut mois_dsn: BTreeSet<String> = BTreeSet::new();
    for x in paies {
        let le = x.paie.importe_le.as_deref();
        // Une paie declaree par une DSN et completee par un bulletin compte des deux cotes : elle a les deux sources.
        if x.paie.declare_par == Declaration::Dsn {
            dsn.voir(Some(&x.mois), Some(&x.mois), le);
            if !x.mois.is_empty() {
                mois_dsn.insert(x.mois.clone());
            }
        }
        if x.paie.declare_par == Declaration::Bulletin || x.paie.bulletin.is_some() {
            bulletins.voir(Some(&x.mois), Some(&x.mois), le);
        }
    }
    let copies = copies_reprises(racine)?;
    let mut out = Vec::new();
    let base = |source: &'static str, quoi: &'static str, r: &Reprise| RepriseVue {
        source,
        quoi,
        nombre: r.nombre,
        du: r.du.clone(),
        au: r.au.clone(),
        le: r.le.clone(),
        declarations: None,
        contrats: None,
        copies: None,
    };
    if dsn.nombre > 0 {
        out.push(RepriseVue { declarations: Some(mois_dsn.len()), ..base("DSN", "versements déclarés", &dsn) });
    }
    if aem.nombre > 0 {
        out.push(base("AEM", "contrats attestés", &aem));
    }
    if certificats.nombre > 0 {
        out.push(base("certificats", "certificats Congés Spectacles", &certificats));
    }
    if bulletins.nombre > 0 || contrats_bulletins.nombre > 0 || copies > 0 {
        let mut b = RepriseVue { contrats: Some(contrats_bulletins.nombre), copies: Some(copies), ..base("bulletins", "bulletins de paie", &bulletins) };
        if b.le.is_none() {
            b.le = contrats_bulletins.le.clone();
        }
        out.push(b);
    }
    Ok(out)
}

/// Les pieces copiees sous `historique/bulletins/` : aucune tant que le dossier n'existe pas. Un dossier qui ne se lit
/// pas se refuse.
fn copies_reprises(racine: &Path) -> R<usize> {
    let dossier = racine.join("historique").join("bulletins");
    if !dossier.exists() {
        return Ok(0);
    }
    let disque = |e: std::io::Error| erreur(Classe::Disque, socle::echec::du_disque(&dossier, &e));
    let mut n = 0;
    for e in std::fs::read_dir(&dossier).map_err(disque)? {
        if e.map_err(disque)?.path().is_file() {
            n += 1;
        }
    }
    Ok(n)
}

/// `editeur` de `importation-etat` : les incidents (`journal/incidents.json`) et les signalements
/// (`journal/signalements.json`) de ce poste qui parlent de l'import -- un incident de nature `refus` n'en est pas : la
/// phrase a ete dite a l'ecran. Rien que ce qu'ils portent deja (caviarde).
fn attend_l_editeur(racine: &Path) -> R<Vec<MessageEditeur>> {
    let parle_d_import = |t: &str| {
        let t = t.to_lowercase();
        t.contains("import") || t.contains("repris")
    };
    let mut out = Vec::new();
    for l in crate::compagnie::incidents::lire(racine, None, false)? {
        // Les ecarts de calcul de l'import sont l'etude de l'editeur (`import::ecarts`) : rien n'y attend la compagnie.
        if l.ou == crate::import::ecarts::OU {
            continue;
        }
        // Un refus (un dossier mal tape, un export absent) se dit a l'ecran ; il n'attend pas l'editeur.
        if l.quoi == crate::compagnie::incidents::REFUS || !parle_d_import(&format!("{} {}", l.ou, l.genre)) {
            continue;
        }
        if l.envoi.etat == crate::compagnie::incidents::SANS_OBJET {
            continue;
        }
        let parti = l.envoi.etat == crate::compagnie::incidents::DEPOSE;
        let dit = if l.resume.is_empty() { l.genre } else { l.resume };
        out.push(message("incident", Some(l.quand).filter(|q| !q.is_empty()), Some(dit), parti, false));
    }
    for s in crate::compagnie::problemes::signalements(racine)? {
        if !parle_d_import(&s.garde.texte) {
            continue;
        }
        let (parti, repondu) = (s.etat == crate::compagnie::problemes::EtatDuMessage::Arrive, s.garde.reponse.is_some());
        out.push(message("signalement", Some(s.garde.quand).filter(|q| !q.is_empty()), Some(s.garde.texte), parti, repondu));
    }
    Ok(out)
}

/// Un message a l'editeur, son titre et sa pastille : la reponse recue, faite ; le reste attend l'editeur, rien que la
/// compagnie ait a faire.
fn message(canal: &'static str, quand: Option<String>, resume: Option<String>, parti: bool, repondu: bool) -> MessageEditeur {
    let quoi = if canal == "signalement" { "Message à l'éditeur" } else { "Incident" };
    let titre_dit = match &quand {
        Some(q) => format!("{} du {}", quoi, socle::ecran::quand(q)),
        None => quoi.to_string(),
    };
    let pastille = match (repondu, parti) {
        (true, _) => socle::ecran::Pastille::de("fait", "réponse reçue"),
        (false, true) => socle::ecran::Pastille::de("information", "attend l'éditeur"),
        (false, false) => socle::ecran::Pastille::de("information", "pas encore parti"),
    };
    let resume: String = resume.into_iter().collect();
    MessageEditeur { canal, quand, resume, parti, repondu, pastille, titre_dit }
}

impl Reprise {
    /// Une ligne de plus : ses dates (un texte non vide), et le jour ou elle a ete reprise.
    fn voir(&mut self, debut: Option<&str>, fin: Option<&str>, le: Option<&str>) {
        let texte = |v: Option<&str>| v.filter(|s| !s.is_empty()).map(str::to_string);
        self.nombre += 1;
        if let Some(d) = texte(debut) {
            if self.du.as_ref().is_none_or(|x| &d < x) {
                self.du = Some(d);
            }
        }
        if let Some(f) = texte(fin) {
            if self.au.as_ref().is_none_or(|x| &f > x) {
                self.au = Some(f);
            }
        }
        if let Some(l) = texte(le) {
            if self.le.as_ref().is_none_or(|x| &l > x) {
                self.le = Some(l);
            }
        }
    }
}
