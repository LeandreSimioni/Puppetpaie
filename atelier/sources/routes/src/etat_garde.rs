//! L'ETAT D'UN MOIS ET L'ETAT DE L'OUVERTURE, tels que l'ecran les lit.
//!
//! L'etat d'un mois porte TOUT ce que l'ecran d'un mois lit : l'etat de chaque engagement (`etat-par-engagement`), celui
//! du mois (`etat-du-mois`), et les lectures que la page faisait une a une (`LECTURES` : la saisie, les contrats en
//! cours, les fins, les imprevus, l'apercu de la saisie enregistree). Il se fait dans le processus de la
//! fenetre, par UNE file (`FileDesEtats`) -- des le lancement pour les compagnies de l'installation (`Fil`) --, puis
//! apres chaque ecriture, a l'ouverture d'une compagnie et apres chaque tour de veille, le mois affiche et ses voisins
//! d'abord. Il ne s'ecrit pas : la file le garde en memoire, sous l'EMPREINTE de tout ce dont il est tire (`empreinte`),
//! et la fenetre le sert sans processus (`FileDesEtats::ecran`, route `mois` de `interface/api.rs`) : le dernier etat
//! connu, TOUT DE SUITE, meme perime -- marque `perime`, refait aussitot, et la page se repeint quand l'etat frais
//! arrive. Les paies des engagements valides sont ecrites dans l'engagement (`engagement::etats`) : un etat se refait
//! sans les recalculer.
//!
//! CE QU'UN ETAT GARDE EST UNE REPONSE DEJA FAITE : chaque lecture est gardee ecrite, telle que la porte l'a rendue
//! (`Reponse`, son texte JSON), et rendue telle quelle ; l'etat garde ne la relit pas. Ce qu'il lit lui-meme (le mois a
//! ouvrir, le bilan du mois) il le garde en sa forme.
//!
//! L'empreinte ne relit pas le contenu des sources : elle prend, pour chacune, sa date de modification et sa taille (une
//! modification a la main change la date). S'y ajoutent la date du jour (une echeance passee change ce qui se dit : un
//! etat se refait donc une fois par jour), le moteur lui-meme et l'emplacement du dossier (l'etat porte des chemins).
//! Tant que l'empreinte est la meme, il se sert sans rien refaire ; si quoi que ce soit a bouge, par le logiciel ou a la
//! main, il se refait une fois.
//!
//! L'OUVERTURE D'UNE COMPAGNIE, en une demi-seconde au plus : l'affichage d'abord ; une fois la page affichee, les petits
//! ajustements du moteur se font sans la faire bouger. Le meme mecanisme,
//! sous une autre cle (`OUVERTURE`) : `ouverture.etat.json`, a la racine de la compagnie, porte ce que la page lit en
//! s'ouvrant et qui n'est pas l'ecran d'un mois (`LECTURES_OUVERTURE` : la configuration et l'historique, le mois a
//! ouvrir, l'echeancier, les rappels, ce qui manque pour demarrer, les conseils) ; lui s'ecrit, pour qu'une compagnie
//! s'ouvre tout de suite au lancement suivant. Meme empreinte (dates et tailles), mais de TOUTE la compagnie ; meme
//! file, qui le refait a l'ouverture, apres chaque ecriture et chaque tour de veille ; meme service sans processus
//! (`FileDesEtats::demarrage`, route `demarrage`) : ses lectures justes (refaites si besoin), avec
//! l'ecran du mois a ouvrir, marque `perime` s'il l'est. Ce qui se refait, la fenetre le fait faire et dit ce que
//! fait le moteur (`phrase`), au lieu d'une roue muette.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, LockResult, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::{erreur, Classe, Echec, R};
use socle::mois::Mois;

use crate::catalogue::{Acte, Affiche, Champ, Ecran, Fiche, Forme, Route, ILLISIBLE, MOIS};
use crate::enveloppe::{en_json, route_inconnue, Charge, Dite, Options, Porte, Rendu, Reponse};
use crate::etapes::{Bilan, ChargeMois};

/// Le format du fichier de l'ouverture : un autre se refait avant de se montrer. 5 : le mois a ouvrir en sa forme
/// (`ouvre`), les lectures ecrites.
const FORMAT: u64 = 5;

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Lecture;

/// Les routes que l'etat d'un mois sert, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Mois,
        geste: "Lire tout l'écran d'un mois d'un coup : engagements, étapes, imprévus.",
        charge: &[
            MOIS,
            Champ {
                nom: "attendre",
                libelle: Affiche::new("État frais"),
                forme: Forme::OuiNon,
                sens: "vrai : l'état juste, refait s'il le faut ; sinon, dans la fenêtre, le dernier état connu, marqué perime",
                avance: true,
            },
        ],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "lectures : la réponse de chaque route de l'écran du mois (saisie, etat-par-engagement, etat-du-mois…), perime",
        refus: &[ILLISIBLE],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Demarrage,
        geste: "Lire d'un coup ce que l'écran d'une compagnie montre en s'ouvrant.",
        charge: &[Champ {
            nom: "patienter",
            libelle: Affiche::new("Patienter"),
            forme: Forme::Nombre,
            sens: "les millisecondes que la fenêtre attend un état juste (5000 au plus) ; hors de la fenêtre, l'état se refait",
            avance: true,
        }],
        exemple: r#"{"patienter":400}"#,
        rend: "pret ; prêt : le mois à ouvrir et son écran, aujourdhui, les lectures de l'ouverture (etat, echeancier, rappels…), la veille, la mise à jour, les signalements ; sinon la phrase de ce que fait le moteur",
        refus: &[ILLISIBLE],
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::LectureAvancee,
    },
    Fiche {
        route: Route::EtatParEngagement,
        geste: "Lire l'état de chaque engagement du mois : ses carrés DPAE, Contrat, Documents.",
        charge: &[MOIS],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "engagements (une ligne chacun : ses carrés, ce qui est à refaire), non_actifs",
        refus: &[ILLISIBLE],
        acte: Acte::Lit,
        phrase: 160,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::EtatDuMois,
        geste: "Lire les quatre étapes du mois : DPAE, Contrats, DSN, Documents.",
        charge: &[MOIS],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "les groupes de chaque étape : état, compte, phrase, échéance, et le geste d'un clic",
        refus: &[ILLISIBLE],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Les lectures de l'ecran d'un mois qui ne sont pas les deux etats, chacune sur la charge `{"mois"}`. La page les
/// demandait une a une, chacune dans un processus ; elles sont faites ici, une fois, avec les memes routes, et rendues
/// telles quelles (resultat ou refus). Les arrets n'y sont pas : la page lit les imprevus du mois, chaque arret avec ses
/// obligations. La reprise du journal non plus : elle se montre dans l'Historique, avec les envois et leurs retours.
const LECTURES: [Route; 4] = [Route::Saisie, Route::ContratsEnCours, Route::FinsDeContrat, Route::Imprevus];

/// La cle de l'etat garde de l'OUVERTURE dans la file : a cote des mois, jamais confondue avec eux.
pub(crate) const OUVERTURE: &str = "ouverture";

/// La cle du TABLEAU de l'administrateur dans la file : `config/.tableau.json`, refait pour une compagnie
/// rattachee (`administrateur::tableau_compagnie`), sous l'empreinte de l'ouverture (toute la compagnie).
const TABLEAU: &str = "tableau";

/// Ce que la page lit en s'ouvrant, hors de l'ecran d'un mois (`js/demarrage.js`) : la route et sa charge. Chacune est
/// rendue telle quelle (resultat ou refus), au format d'une reponse de la page. Le mois a ouvrir y est aussi, lu en sa
/// forme (`etapes::le_mois_a_ouvrir`). Ce qui vit dans la memoire de la fenetre (la veille, la mise a jour) et ce que
/// l'ecran ne garde pas (`journal/signalements.json`) se lit a chaque ouverture, a cote (`interface/api.rs`).
const LECTURES_OUVERTURE: [(Route, &str); 6] = [
    (Route::Etat, "{}"),
    (Route::Echeancier, "{}"),
    (Route::Rappels, "{}"),
    (Route::PourDemarrer, "{}"),
    (Route::ConseilsResume, "{}"),
    (Route::ConseilDuJour, r#"{"decalage": 0}"#),
];

/// Le fichier de l'etat de l'OUVERTURE, a la racine de la compagnie : le seul etat qui s'ecrit.
fn chemin(racine: &Path) -> PathBuf {
    racine.join("ouverture.etat.json")
}

// ------------------------------------------------------------------------------------------------- les cles

/// LA CLE D'UN ETAT GARDE : un mois, l'ouverture de la compagnie, ou le tableau de l'administrateur.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Cle {
    Mois(Mois),
    Ouverture,
    Tableau,
}

impl Cle {
    /// La cle ecrite `texte` (le banc la nomme ainsi) : `ouverture`, `tableau` ou un mois AAAA-MM.
    fn lire(texte: &str) -> R<Cle> {
        match texte {
            OUVERTURE => Ok(Cle::Ouverture),
            TABLEAU => Ok(Cle::Tableau),
            m => Mois::lire(m).map(Cle::Mois).map_err(|e| Echec { classe: Some(Classe::Illisible), ..socle::echec::a_la_cle("Le mois de l'état ne se lit pas : il s'écrit AAAA-MM.", "cle", String::from(e)) }),
        }
    }
}

impl std::fmt::Display for Cle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cle::Mois(m) => write!(f, "{}", m),
            Cle::Ouverture => f.write_str(OUVERTURE),
            Cle::Tableau => f.write_str(TABLEAU),
        }
    }
}

// ------------------------------------------------------------------------------------------------- l'empreinte

/// Le dossier, absolu et comparable (sous Windows, sans le `\\?\` de `canonicalize`) ; un dossier qui ne se resout pas
/// se marque par son erreur (l'etat se refera quand il se resoudra).
fn absolu(p: &Path) -> String {
    let t = match std::fs::canonicalize(p) {
        Ok(abs) => abs.to_string_lossy().into_owned(),
        Err(e) => format!("{} ({:?})", p.to_string_lossy(), e.kind()),
    };
    match t.strip_prefix(r"\\?\") {
        Some(s) => s.to_string(),
        None => t,
    }
}

/// La date de modification d'un fichier, en nanosecondes, ou l'erreur qui l'empeche de se lire : ce qui entre dans
/// l'empreinte.
fn modifie(m: &std::fs::Metadata) -> String {
    match m.modified().map(|t| t.duration_since(std::time::UNIX_EPOCH)) {
        Ok(Ok(d)) => d.as_nanos().to_string(),
        Ok(Err(e)) => format!("avant 1970 ({:?})", e.duration()),
        Err(e) => format!("{:?}", e.kind()),
    }
}

/// Ajoute a `h` chaque fichier du dossier `rel` de `base` que `garder` retient (son chemin relatif a `base`), avec sa
/// date de modification et sa taille, dans l'ordre des noms. Ce qui ne se lit pas (un dossier absent, une entree, une
/// date) entre dans l'empreinte par son erreur : l'etat se refait quand cela change. Sous Windows, la date et la taille
/// viennent de l'enumeration du dossier : pas d'appel de plus par fichier.
fn signer_dossier(h: &mut Sha256, base: &Path, rel: &str, profond: bool, garder: &dyn Fn(&str) -> bool) {
    let signer = |h: &mut Sha256, nom: &str, quoi: &str| {
        h.update(nom.as_bytes());
        h.update([0]);
        h.update(quoi.as_bytes());
        h.update([0]);
    };
    let lus = match std::fs::read_dir(base.join(rel)) {
        Ok(l) => l,
        Err(e) => return signer(h, rel, &format!("{:?}", e.kind())),
    };
    let mut entrees: Vec<std::fs::DirEntry> = Vec::new();
    for e in lus {
        match e {
            Ok(e) => entrees.push(e),
            Err(e) => signer(h, rel, &format!("{:?}", e.kind())),
        }
    }
    entrees.sort_by_key(|e| e.file_name());
    for e in entrees {
        let fichier = e.file_name().to_string_lossy().into_owned();
        let nom = if rel.is_empty() { fichier } else { format!("{}/{}", rel, fichier) };
        match e.file_type() {
            Ok(t) if t.is_dir() => {
                if profond {
                    signer_dossier(h, base, &nom, true, garder);
                }
                continue;
            }
            Ok(_) => {}
            Err(x) => {
                signer(h, &nom, &format!("{:?}", x.kind()));
                continue;
            }
        }
        if !garder(&nom) {
            continue;
        }
        let quoi = match e.metadata() {
            Ok(m) => format!("{} {}", modifie(&m), m.len()),
            Err(x) => format!("{:?}", x.kind()),
        };
        signer(h, &nom, &quoi);
    }
}

/// CE QUE L'ECRAN D'UN MOIS NE LIT PAS, et qui bouge sans lui. Etabli par mesure (`strace` de la
/// route `mois` sur cinq mois d'une compagnie fictive 3090 : aucun de ces fichiers n'est ouvert ni teste) et par le code
/// (chacun n'a de lecteur que hors des lectures du mois : `LECTURES`, `etat-par-engagement`, `etat-du-mois`, l'apercu) :
/// - `config/.ouverture.json` : la trace de la veille (`ouverture.rs`, `chemin_trace`), reecrite a chaque tour ; lue par
///   la veille de la fenetre seulement (`interface::veille`) ;
/// - `config/.sauvegarde.json` : l'index de la sauvegarde (`sauvegarde::chemin_index`), reecrit a chaque sauvegarde ; lu
///   par la sauvegarde et la restauration seulement ;
/// - `config/diagnostic.json` : l'identite du poste chez l'editeur (`incidents.rs`, `POSTE_FICHIER`), ecrite par la
///   veille ; lue par l'envoi des incidents et des signalements et l'import ;
/// - `config/horloge.json` : la memoire de l'horloge d'une compagnie fictive (`horloge.rs`), reecrite a chaque tour de
///   veille ; lue par l'horloge seulement (le jour d'un `Contexte` ne la lit pas) ;
/// - `journal/signalements.json` : « Un probleme, une idee ? » (`problemes.rs`) ; lu par `signalements` et
///   `importation-etat` ;
/// - `journal/incidents.json` : le journal des incidents.
const HORS_DE_L_ECRAN: [&str; 7] = [
    "config/.ouverture.json",
    "config/.sauvegarde.json",
    compagnie::incidents::DIAGNOSTIC,
    compagnie::installation::FICHIER_HORLOGE,
    compagnie::problemes::SIGNALEMENTS,
    compagnie::incidents::INCIDENTS,
    // Le tableau de l'administrateur (`administrateur.rs`) : le resume de la compagnie que l'accueil lit.
    crate::gestes::administrateur::TABLEAU,
];

/// L'empreinte de tout ce dont l'etat `cle` est tire, au jour `jour` (AAAA-MM-JJ).
///
/// Pour un MOIS : ni le coffre (un `.bin` : ce qu'il dit se relit a chaque lecture), ni ce que l'ecran ne lit pas
/// (`HORS_DE_L_ECRAN`), ni les sorties des autres mois.
///
/// Pour l'OUVERTURE : toute la compagnie -- les engagements et les sorties de tous les mois (le mois a
/// ouvrir et l'historique les lisent), les images de l'identite (`gabarits/`), la date et la taille des fichiers du coffre
/// (la configuration dit si la boite courriel est liee, `boite_liee` : rien n'en est lu ici, ni ailleurs dans
/// l'empreinte), et l'adresse de la boite posee dans l'environnement (`COURRIEL_COMPTE`). Ce qui ne s'y lit pas reste
/// dehors (`HORS_DE_L_ECRAN` : les signalements se lisent a chaque ouverture, a cote).
pub(crate) fn empreinte_de(racine: &Path, cle: &str, jour: &str) -> String {
    let ouverture = cle == OUVERTURE;
    let mut h = Sha256::new();
    let mut marques = vec![env!("PAIE_SOURCES").to_string(), jour.to_string(), absolu(racine), absolu(&socle::fichiers::dossier_du_code())];
    if ouverture {
        let compte = match std::env::var("COURRIEL_COMPTE") {
            Ok(c) => c,
            Err(e) => format!("{:?}", e),
        };
        marques.push(format!("ouverture {}", compte));
    }
    for marque in marques {
        h.update(marque.as_bytes());
        h.update([0]);
    }
    let sorties_du_mois = format!("sorties/{}/", cle);
    // La compagnie : ses engagements (et `engagements/avant/`) et leurs paies, sa configuration, ses journaux, ses
    // imprevus, ses regularisations, son historique importe, ses retours, et ce qui est sorti pour ce mois-ci (pour
    // l'ouverture : pour tous les mois).
    signer_dossier(&mut h, racine, engagement::stock::DOSSIER, true, &|n| n.ends_with(".json"));
    signer_dossier(&mut h, racine, engagement::paies::DOSSIER, true, &|n| n.ends_with(".json"));
    signer_dossier(&mut h, racine, socle::fichiers::CONFIG, true, &|n| {
        (ouverture || !n.ends_with(".bin")) && !n.ends_with(".tmp") && !HORS_DE_L_ECRAN.contains(&n)
    });
    signer_dossier(&mut h, racine, envois::journal::JOURNAL, false, &|n| n.ends_with(".json") && !HORS_DE_L_ECRAN.contains(&n));
    const DOSSIERS_SIGNES: [&str; 3] = ["imprevus", "historique", "retours"];
    for d in DOSSIERS_SIGNES {
        signer_dossier(&mut h, racine, d, true, &|n| !n.ends_with(".tmp"));
    }
    if ouverture {
        signer_dossier(&mut h, racine, "sorties", true, &|n| !n.ends_with(".tmp"));
        signer_dossier(&mut h, racine, "gabarits", true, &|n| !n.ends_with(".tmp"));
    } else {
        signer_dossier(&mut h, racine, &format!("sorties/{}", cle), true, &|n| n.starts_with(&sorties_du_mois));
    }
    // Le code livre : baremes, bases, modeles (une mise a jour les change sans changer le moteur).
    let code = socle::fichiers::dossier_du_code();
    signer_dossier(&mut h, &code, "", true, &|_| true);
    h.finalize().iter().map(|o| format!("{:02x}", o)).collect()
}

/// L'empreinte de l'etat `cle` de la compagnie `racine`, au jour `jour`.
fn empreinte(racine: &Path, cle: Cle, jour: &Jour) -> String {
    empreinte_de(racine, &cle.to_string(), &jour.iso())
}

/// Le jour de l'empreinte : celui d'un `Contexte` (`aujourdhui`, que `date` ne regle pas).
fn jour() -> R<Jour> {
    droit::contexte::aujourdhui_sans_note().map_err(Echec::from)
}

/// Ce que le fil des calculs ne peut pas dire a un ecran (il tourne en fond) : note au diagnostic du moteur, que le
/// developpeur lit.
fn noter(quoi: &str, e: &str) {
    compagnie::diagnostic::noter_moteur(&format!("État gardé : {} ({}).", quoi, e));
}

/// Un verrou pris, meme si un fil est tombe en le tenant : ce qu'il garde reste coherent (chaque ecriture s'y fait d'un
/// coup).
fn tenu<T>(r: LockResult<T>) -> T {
    match r {
        Ok(g) => g,
        Err(p) => PoisonError::into_inner(p),
    }
}

// ------------------------------------------------------------------------------------------------- ce qui se garde

/// L'ETAT GARDE D'UN MOIS, en memoire : son empreinte, `etat-par-engagement` rendu, le bilan du mois en sa forme (ce que
/// dit le coffre s'y ajoute a chaque lecture : `du_mois`), et les autres lectures de l'ecran, ecrites.
struct EtatDuMois {
    mois: Mois,
    empreinte: String,
    par_engagement: Rendu,
    bilan: Bilan,
    lectures: Vec<(Route, String)>,
}

/// L'ETAT DE L'OUVERTURE, tel qu'il s'ecrit (`ouverture.etat.json`) : son format et le moteur qui l'a ecrit (un autre se
/// refait), son empreinte, le mois que `mois-a-ouvrir` ouvre (vide s'il refuse : l'ecran ouvre le mois du jour), et les
/// lectures de la page qui s'ouvre, chacune ecrite telle que la porte l'a rendue.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EtatDeLOuverture {
    format: u64,
    moteur: String,
    empreinte: String,
    ouvre: Option<Mois>,
    lectures: BTreeMap<String, String>,
}

/// LE TABLEAU D'UNE COMPAGNIE tel que l'administrateur l'ecrit (`config/.tableau.json`,
/// `administrateur::tableau_compagnie`) : son jour, ses declarations, l'empreinte de la compagnie qu'il a lue.
use crate::gestes::administrateur::TableauCompagnie;

/// Un etat garde : d'un mois, de l'ouverture, ou le tableau de l'administrateur (sa forme est la sienne).
#[derive(Clone)]
enum Garde {
    Mois(Arc<EtatDuMois>),
    Ouverture(Arc<EtatDeLOuverture>),
    Tableau(Arc<TableauCompagnie>),
}

impl Garde {
    fn defaut(quoi: &str) -> Echec {
        erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : l'état gardé n'est pas {}.", quoi))
    }

    fn du_mois(self) -> R<Arc<EtatDuMois>> {
        match self {
            Garde::Mois(e) => Ok(e),
            _ => Err(Garde::defaut("celui d'un mois")),
        }
    }

    fn de_l_ouverture(self) -> R<Arc<EtatDeLOuverture>> {
        match self {
            Garde::Ouverture(e) => Ok(e),
            _ => Err(Garde::defaut("celui de l'ouverture")),
        }
    }
}

/// Une lecture rendue a la page : ecrite (gardee), ou faite a l'instant.
enum ALaPage<'a> {
    Ecrite(&'a str),
    Faite(Reponse),
}

impl Serialize for ALaPage<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            // La reponse telle que la porte l'a ecrite, et non le texte qui la garde.
            ALaPage::Ecrite(t) => socle::forme::lire_texte::<Reponse>(t, "Une lecture gardée ne se relit pas.").map_err(|e| serde::ser::Error::custom(String::from(e)))?.serialize(s),
            ALaPage::Faite(r) => r.serialize(s),
        }
    }
}

/// `etat-du-mois` tel que la page le recoit : le bilan, et si le mot de passe du logiciel est pose. LE COFFRE N'EST PAS
/// DANS L'EMPREINTE (un `.bin`, qu'on ne lit pas) : ce qu'il dit se relit a chaque fois.
#[derive(Serialize)]
struct DuMois<'a> {
    #[serde(flatten)]
    bilan: &'a Bilan,
    mot_de_passe_pose: bool,
}

impl Dite for DuMois<'_> {
    fn phrase(&self) -> &str {
        self.bilan.phrase()
    }
}

fn du_mois(racine: &Path, bilan: &Bilan) -> R<Rendu> {
    let pose = compagnie::coffre::Coffre::du_poste(racine, "")
        .ouverture_exigee()
        .map_err(|x| Echec { classe: Some(x.classe), phrase: x.message, a_part: None })?;
    Rendu::de(&DuMois { bilan, mot_de_passe_pose: pose })
}

/// TOUT L'ECRAN D'UN MOIS, tel que la page le lit (`chargerMois`) : la phrase de ses etapes, le mois, `lectures` (une
/// entree par route, au format d'une reponse de la page), et s'il est perime.
#[derive(Serialize)]
struct EcranDuMois<'a> {
    phrase: &'a str,
    mois: Mois,
    lectures: BTreeMap<&'static str, ALaPage<'a>>,
    perime: bool,
}

impl Dite for EcranDuMois<'_> {
    fn phrase(&self) -> &str {
        self.phrase
    }
}

impl EtatDuMois {
    fn ecran(&self, racine: &Path, perime: bool) -> EcranDuMois<'_> {
        let mut lectures: BTreeMap<&'static str, ALaPage> = self.lectures.iter().map(|(r, t)| (r.nom(), ALaPage::Ecrite(t))).collect();
        lectures.insert(Route::EtatParEngagement.nom(), ALaPage::Faite(Reponse::de(Route::EtatParEngagement, Ok(self.par_engagement.clone()))));
        lectures.insert(Route::EtatDuMois.nom(), ALaPage::Faite(Reponse::de(Route::EtatDuMois, du_mois(racine, &self.bilan))));
        EcranDuMois { phrase: self.bilan.phrase(), mois: self.mois, lectures, perime }
    }
}

impl EtatDeLOuverture {
    fn a_la_page(&self) -> BTreeMap<&str, ALaPage<'_>> {
        self.lectures.iter().map(|(r, t)| (r.as_str(), ALaPage::Ecrite(t))).collect()
    }

    /// Le mois qu'ouvre l'ouverture : celui que rend `mois-a-ouvrir` ; s'il refuse (son refus est dans les lectures),
    /// le mois du jour.
    fn mois(&self, jour: &Jour) -> Mois {
        match self.ouvre {
            Some(m) => m,
            None => jour.mois_civil(),
        }
    }
}

/// CE QUE L'ECRAN D'UNE COMPAGNIE LIT EN S'OUVRANT (`demarrage`), d'un coup : pret, le mois a ouvrir et son ecran, le jour
/// du moteur, les lectures de l'ouverture (`LECTURES_OUVERTURE`), et ce qui se lit a cote (`ACote`). Pas encore pret (la
/// fenetre n'attend que `patienter`) : la phrase de ce que fait le moteur, et rien d'autre.
#[derive(Serialize)]
pub struct Demarrage<'a> {
    phrase: String,
    pret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    mois: Option<Mois>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aujourdhui: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lectures: Option<BTreeMap<&'a str, ALaPage<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ecran: Option<EcranDuMois<'a>>,
    /// La veille de la fenetre : hors d'elle, aucune.
    #[serde(skip_serializing_if = "Option::is_none")]
    veille: Option<crate::veille::EtatDeLaVeille>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maj: Option<compagnie::maj::EtatDeLaRecherche>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signalements: Option<Reponse>,
}

impl Dite for Demarrage<'_> {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// CE QUI SE LIT A COTE DE L'ETAT GARDE, a chaque demarrage : ce qui vit dans la memoire de la fenetre (la veille, la
/// recherche de la mise a jour) et ce que l'etat garde ne suit pas (les signalements, `journal/signalements.json`).
pub struct ACote {
    pub veille: Option<crate::veille::EtatDeLaVeille>,
    pub maj: compagnie::maj::EtatDeLaRecherche,
    pub signalements: Reponse,
}

impl ACote {
    /// Ce qui se lit a cote pour `racine` ; `veille` : celle de la fenetre, s'il y en a une.
    pub fn de(racine: &Path, veille: Option<crate::veille::EtatDeLaVeille>) -> ACote {
        let signalements = Reponse::de(Route::Signalements, crate::filet::signalements_route(racine).and_then(en_json));
        ACote { veille, maj: compagnie::maj::RECHERCHE.etat(), signalements }
    }
}

impl<'a> Demarrage<'a> {
    /// La compagnie prete : l'ouverture `ouv`, l'ecran de `etat` (perime ou non), et ce qui se lit a cote.
    fn pret(racine: &Path, ouv: &'a EtatDeLOuverture, etat: &'a EtatDuMois, perime: bool, jour: &Jour, a_cote: ACote) -> Demarrage<'a> {
        let ecran = etat.ecran(racine, perime);
        Demarrage {
            phrase: ecran.phrase.to_string(),
            pret: true,
            mois: Some(etat.mois),
            aujourdhui: Some(jour.iso()),
            lectures: Some(ouv.a_la_page()),
            ecran: Some(ecran),
            veille: a_cote.veille,
            maj: Some(a_cote.maj),
            signalements: Some(a_cote.signalements),
        }
    }

    /// Pas encore pret : ce que fait le moteur.
    fn en_attente(phrase: String) -> Demarrage<'static> {
        Demarrage { phrase, pret: false, mois: None, aujourdhui: None, lectures: None, ecran: None, veille: None, maj: None, signalements: None }
    }
}

/// La charge de `demarrage` : ce que la fenetre attend un etat juste, en millisecondes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeDemarrage {
    patienter: u64,
}

/// La charge de `mois` : le mois, et si la page veut l'etat juste (`attendre`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeEcranDuMois {
    mois: Mois,
    attendre: Option<bool>,
}

// ------------------------------------------------------------------------------------------------- refaire

/// Une route du moteur, faite dans ce processus, ecrite comme la page la recoit (l'enveloppe des routes,
/// `routes::enveloppe::Reponse`).
fn lecture(o: &Options, route: Route, charge: &Charge) -> String {
    socle::ecriture::json_en_ligne(&crate::servir_la_route(o, route, charge))
}

/// La charge `{"mois": mois}` d'une lecture de l'ecran du mois.
fn charge_du_mois(mois: Mois) -> R<Charge> {
    socle::forme::lire_texte(&socle::ecriture::json_en_ligne(&ChargeMois { mois }), "La charge d'un mois ne s'écrit pas.").map_err(Echec::from)
}

/// La charge de l'apercu de la saisie enregistree : le mois, et aucune modification en cours (`mois::ChargeApercu`).
#[derive(Serialize)]
struct ApercuEnregistre {
    mois: Mois,
    modifications: [Mois; 0],
}

/// L'etat de `mois`, refait : ses deux etats (`etapes::les_deux_etats`) et ses lectures (`LECTURES`, puis l'apercu de la
/// saisie enregistree).
fn refaire_le_mois(o: &Options, ctx: &Contexte, mois: Mois, empreinte: String) -> R<EtatDuMois> {
    let charge = charge_du_mois(mois)?;
    let mut lectures: Vec<(Route, String)> = LECTURES.iter().map(|r| (*r, lecture(o, *r, &charge))).collect();
    // L'APERCU : ce que la page demande des qu'un mois s'affiche (`calculerApercu`), sur la saisie qu'elle vient de lire
    // (`{"mois": mois, "modifications": []}`, le mois enregistre), quand le mois a une saisie ; la page ne le redemande
    // que quand on modifie la saisie.
    if engagement::stock::a_une_saisie(&ctx.racine, &mois.to_string())? {
        let apercu = ApercuEnregistre { mois, modifications: [] };
        let charge = socle::forme::lire_texte(&socle::ecriture::json_en_ligne(&apercu), "La charge de l'aperçu ne s'écrit pas.").map_err(Echec::from)?;
        lectures.push((Route::Apercu, lecture(o, Route::Apercu, &charge)));
    }
    let (par_engagement, bilan) = crate::etapes::les_deux_etats(ctx, mois)?;
    Ok(EtatDuMois { mois, empreinte, par_engagement, bilan, lectures })
}

/// L'etat de l'ouverture, refait : les lectures de la page qui s'ouvre (`LECTURES_OUVERTURE`), telles quelles, et le
/// mois a ouvrir.
fn refaire_l_ouverture(o: &Options, ctx: &Contexte, empreinte: String) -> R<EtatDeLOuverture> {
    let mut lectures = BTreeMap::new();
    for (route, charge) in LECTURES_OUVERTURE {
        let c: Charge = socle::forme::lire_texte(charge, "La charge d'une lecture de l'ouverture ne se lit pas.")?;
        lectures.insert(route.nom().to_string(), lecture(o, route, &c));
    }
    compagnie::incidents::brancher_l_engagement();
    let (ouvre, reponse) = match crate::etapes::le_mois_a_ouvrir(ctx) {
        Ok((m, rendu)) => (Some(m), Reponse::de(Route::MoisAOuvrir, Ok(rendu))),
        Err(e) => (None, Reponse::de(Route::MoisAOuvrir, Err(e))),
    };
    lectures.insert(Route::MoisAOuvrir.nom().to_string(), socle::ecriture::json_en_ligne(&reponse));
    // Le mois que l'ouverture montre se lit avec elle : ses paies validees se recalculent ici (`engagement::etats::du_mois`),
    // pas apres l'ecriture de l'etat, qui serait aussitot perime. Ce qui refuse ici refusera a l'ecran du mois, qui le dit.
    if let Some(m) = ouvre {
        let recalcul = envois::journal::lu(&ctx.racine).map_err(String::from).and_then(|j| engagement::etats::du_mois(ctx, &m.to_string(), &j.departs).map_err(String::from));
        if let Err(e) = recalcul {
            noter(&format!("le recalcul de {} à l'ouverture refuse", m), &e);
        }
    }
    Ok(EtatDeLOuverture { format: FORMAT, moteur: env!("PAIE_SOURCES").to_string(), empreinte, ouvre, lectures })
}

/// LE TABLEAU DE L'ADMINISTRATEUR pour cette compagnie (`config/.tableau.json`) : le garde s'il porte l'empreinte
/// d'aujourd'hui de toute la compagnie (celle de l'ouverture), sinon refait.
fn tableau_juste(ctx: &Contexte) -> R<Garde> {
    let f = ctx.racine.join(crate::gestes::administrateur::TABLEAU);
    if f.is_file() {
        let empreinte = empreinte(&ctx.racine, Cle::Ouverture, &ctx.aujourdhui);
        match socle::forme::lire_fichier::<TableauCompagnie>(&f, "Le tableau gardé de la compagnie ne se lit pas.") {
            Ok(t) if t.empreinte == empreinte => return Ok(Garde::Tableau(Arc::new(t))),
            Ok(_) => {}
            Err(m) => noter("le tableau gardé ne se lit pas, il se refait", &String::from(m)),
        }
    }
    Ok(Garde::Tableau(Arc::new(crate::gestes::administrateur::tableau_compagnie(ctx)?)))
}

/// Refait l'etat de `cle` (un mois, ou l'ouverture, qui s'ecrit ; le tableau, s'il n'est plus juste). Ce qui se lit
/// peut ecrire (une paie validee qui se recalcule d'elle-meme quand on la lit, `engagement::etats::du_mois`) :
/// l'empreinte d'apres n'est alors plus celle d'avant, et un second passage, sans recalcul cette fois, la retrouve.
fn refaire(o: &Options, ctx: &Contexte, cle: Cle) -> R<Garde> {
    for passage in 1..=2 {
        let avant = empreinte(&ctx.racine, cle, &ctx.aujourdhui);
        let garde = match cle {
            Cle::Tableau => return tableau_juste(ctx),
            Cle::Mois(m) => Garde::Mois(Arc::new(refaire_le_mois(o, ctx, m, avant.clone())?)),
            Cle::Ouverture => Garde::Ouverture(Arc::new(refaire_l_ouverture(o, ctx, avant.clone())?)),
        };
        let stable = empreinte(&ctx.racine, cle, &ctx.aujourdhui) == avant;
        if stable {
            if let Garde::Ouverture(e) = &garde {
                // Une ecriture impossible ne change rien au resultat : la prochaine lecture refera.
                if let Err(m) = socle::ecriture::ecrire_json(&chemin(&ctx.racine), e.as_ref(), 0) {
                    noter("l'état de l'ouverture ne s'écrit pas", &String::from(m));
                }
            }
        }
        if stable || passage == 2 {
            return Ok(garde);
        }
    }
    unreachable!("le second passage rend l'etat")
}

/// L'etat de l'ouverture tel qu'il est ecrit, de ce format et de ce moteur ; aucun s'il n'y en a pas, s'il ne se lit
/// pas (il se refait, et c'est note) ou s'il vient d'un autre moteur (sa forme peut n'etre pas celle que la page lit).
fn ouverture_ecrite(racine: &Path) -> Option<EtatDeLOuverture> {
    let f = chemin(racine);
    if !f.is_file() {
        return None;
    }
    match socle::forme::lire_fichier::<EtatDeLOuverture>(&f, "L'état gardé de l'ouverture ne se lit pas.") {
        Ok(e) if e.format == FORMAT && e.moteur == env!("PAIE_SOURCES") => Some(e),
        Ok(_) => None,
        Err(m) => {
            noter("l'état de l'ouverture ne se lit pas, il se refait", &String::from(m));
            None
        }
    }
}

/// L'etat de `cle` (le banc d'essai le rejoue : `scenario`) : celui de l'ouverture, ecrit, s'il est juste ;
/// sinon refait.
pub fn lire(o: &Options, ctx: &Contexte, cle: &str) -> R<()> {
    garde_juste(o, ctx, Cle::lire(cle)?).map(drop)
}

fn garde_juste(o: &Options, ctx: &Contexte, cle: Cle) -> R<Garde> {
    if cle == Cle::Ouverture {
        if let Some(e) = ouverture_ecrite(&ctx.racine).filter(|e| e.empreinte == empreinte(&ctx.racine, cle, &ctx.aujourdhui)) {
            return Ok(Garde::Ouverture(Arc::new(e)));
        }
    }
    refaire(o, ctx, cle)
}

/// `etat-par-engagement`, `etat-du-mois` et `mois` servis par l'etat du mois, branches sur l'enveloppe : les deux
/// premiers lisent les etats du mois (`etapes::les_deux_etats`), le troisieme refait tout l'ecran (ses lectures se font
/// par les routes, d'ou `o`).
pub fn route(o: &Options, porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::EtatParEngagement => {
            let ChargeMois { mois } = charge.lire()?;
            Ok(crate::etapes::les_deux_etats(porte.ctx, mois)?.0)
        }
        Route::EtatDuMois => {
            let ChargeMois { mois } = charge.lire()?;
            du_mois(&porte.ctx.racine, &crate::etapes::les_deux_etats(porte.ctx, mois)?.1)
        }
        // Hors de la fenetre, l'etat se refait toujours : `attendre` n'y change rien.
        Route::Mois => {
            let ChargeEcranDuMois { mois, .. } = charge.lire()?;
            let etat = refaire(o, porte.ctx, Cle::Mois(mois))?.du_mois()?;
            let ecran: EcranDuMois = etat.ecran(&porte.ctx.racine, false);
            en_json(ecran)
        }
        // Hors de la fenetre, l'ouverture et son mois se refont : rien a patienter, aucune veille.
        Route::Demarrage => {
            let _: ChargeDemarrage = charge.lire()?;
            let jour = jour()?;
            let ouv = refaire(o, porte.ctx, Cle::Ouverture)?.de_l_ouverture()?;
            let etat = refaire(o, porte.ctx, Cle::Mois(ouv.mois(&jour)))?.du_mois()?;
            let r: Demarrage = Demarrage::pret(&porte.ctx.racine, &ouv, &etat, false, &jour, ACote::de(&porte.ctx.racine, None));
            en_json(r)
        }
        autre => Err(route_inconnue(autre.nom())),
    }
}

/// Les mois dont l'etat se tient a jour : ceux que la saisie a touches, et le mois du jour.
fn mois_suivis(racine: &Path) -> R<Vec<Mois>> {
    let mut mois = engagement::stock::mois_saisis(racine)?;
    mois.push(jour()?.mois_civil());
    mois.sort();
    mois.dedup();
    Ok(mois)
}

/// Un passage de la file : l'etat garde, ou son refus (classe et phrase : un `Echec` ne se copie pas).
type Fait = Result<Garde, (Option<Classe>, String)>;

fn refus(f: &Fait) -> R<Garde> {
    match f {
        Ok(g) => Ok(g.clone()),
        Err((classe, message)) => Err(Echec { classe: *classe, phrase: message.clone(), a_part: None }),
    }
}

/// Ce qu'une panique dit, pour la phrase d'un calcul tombe.
fn pourquoi(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        return s.to_string();
    }
    if let Some(s) = p.downcast_ref::<String>() {
        return s.clone();
    }
    "sans phrase".to_string()
}

#[derive(Default)]
struct EtatDeLaFile {
    /// Les cles a verifier, dans l'ordre ou elles se feront.
    attente: VecDeque<Cle>,
    /// Le dernier numero de demande de chaque cle, et le numero de la demande que son dernier passage a servie.
    voulu: HashMap<Cle, u64>,
    fait: HashMap<Cle, (u64, Fait)>,
    numero: u64,
    affiche: Option<Mois>,
    /// Ce que le fil refait en ce moment : la phrase d'attente le dit (`phrase`).
    en_cours: Option<Cle>,
    arretee: bool,
}

/// LE FIL DES CALCULS EN FOND : n'importe quelle compagnie s'ouvre instantanement, meme a la premiere ouverture du jour.
/// UN fil pour TOUTES les compagnies que la fenetre tient (`FileDesEtats`, une par compagnie) : un
/// calcul a la fois sur le poste, jamais deux qui se disputent le processeur pendant qu'on ouvre une compagnie. Il sert
/// d'abord la compagnie OUVERTE (`devant`), puis les autres, chacune son tour (une cle chacune : leurs ouvertures
/// d'abord, puis les mois a ouvrir, puis le reste), dans l'ordre ou la fenetre les a donnees (la derniere ouverte
/// d'abord). Un calcul commence se finit : la compagnie qu'on ouvre attend au plus celui qui tourne.
///
/// CE FIL NE SORT PAS ET N'OUVRE AUCUN COFFRE (`fil_de_fond`) : il ne fait que lire les fichiers de la compagnie et ecrire
/// ses etats gardes. Une sortie reseau ou l'usage de la cle du poste ou d'un mot de passe y est refuse, quoi qu'une
/// lecture demande -- c'est ce qui permet de le faire tourner AVANT le mot de passe, des le lancement.
///
/// LE JOUR CHANGE (la fenetre ouverte passe minuit) : tout se reverifie (l'empreinte d'un etat porte la date du jour).
pub struct Fil {
    etat: Mutex<EtatDuFil>,
    reveil: Condvar,
}

#[derive(Default)]
struct EtatDuFil {
    files: Vec<Arc<FileDesEtats>>,
    devant: Option<Arc<FileDesEtats>>,
    /// La prochaine compagnie a servir parmi les autres (chacune son tour).
    tour: usize,
    /// Le jour de la derniere verification complete.
    jour: Option<Jour>,
    arrete: bool,
}

impl Fil {
    /// Un fil, qui tourne aussitot (il attend du travail).
    pub fn nouveau() -> Arc<Fil> {
        let f = Arc::new(Fil { etat: Mutex::new(EtatDuFil::default()), reveil: Condvar::new() });
        let fil = f.clone();
        crate::fil_de_routes(move || {
            socle::fil_de_fond::marquer();
            fil.tourner();
        });
        f
    }

    fn verrou(&self) -> std::sync::MutexGuard<'_, EtatDuFil> {
        tenu(self.etat.lock())
    }

    /// Du travail est arrive (une cle inscrite) : le fil se reveille. Pris sous le verrou du fil : le fil qui regardait
    /// les files a ce moment l'a lache pour attendre, il ne manque pas le signal.
    fn reveiller(&self) {
        let _g = self.verrou();
        self.reveil.notify_all();
    }

    /// Le jour, tel que le fil le suit ; un jour qui ne se lit pas est note, et rien ne se reverifie.
    fn jour_suivi() -> Option<Jour> {
        match jour() {
            Ok(j) => Some(j),
            Err(e) => {
                noter("le jour ne se lit pas", &String::from(e));
                None
            }
        }
    }

    /// La file de la compagnie `racine`, sur ce fil. `date` : la date des calculs (les tests).
    pub fn file(self: &Arc<Self>, racine: &Path, date: Option<Jour>) -> Arc<FileDesEtats> {
        let o = Options { racine: Some(racine.to_path_buf()), date };
        let f = Arc::new(FileDesEtats {
            o,
            racine: racine.to_path_buf(),
            etat: Mutex::new(EtatDeLaFile::default()),
            signal: Condvar::new(),
            fil: self.clone(),
        });
        let mut g = self.verrou();
        g.files.push(f.clone());
        if g.jour.is_none() {
            g.jour = Fil::jour_suivi();
        }
        drop(g);
        f
    }

    /// `file` passe devant toutes les autres (la compagnie qu'on ouvre) ; `None` : aucune (retour a l'accueil).
    pub fn devant(&self, file: Option<&Arc<FileDesEtats>>) {
        let mut g = self.verrou();
        g.devant = file.cloned();
        if let Some(f) = file {
            if let Some(i) = g.files.iter().position(|x| Arc::ptr_eq(x, f)) {
                let f = g.files.remove(i);
                g.files.insert(0, f);
            }
        }
        self.reveil.notify_all();
    }

    /// Le prochain travail : la compagnie devant d'abord, puis les autres, chacune son tour.
    fn choisir(g: &mut EtatDuFil) -> Option<(Arc<FileDesEtats>, Cle, u64)> {
        if let Some(d) = g.devant.clone() {
            if let Some((cle, n)) = d.prendre_travail() {
                return Some((d, cle, n));
            }
        }
        let n = g.files.len();
        for i in 0..n {
            let k = (g.tour + i) % n;
            let f = g.files[k].clone();
            if g.devant.as_ref().is_some_and(|d| Arc::ptr_eq(d, &f)) {
                continue;
            }
            if let Some((cle, numero)) = f.prendre_travail() {
                g.tour = k + 1;
                return Some((f, cle, numero));
            }
        }
        None
    }

    fn tourner(&self) {
        loop {
            let mut g = self.verrou();
            let (file, cle, numero) = loop {
                if g.arrete {
                    return;
                }
                if let Some(x) = Self::choisir(&mut g) {
                    break x;
                }
                let (h, attente) = tenu(self.reveil.wait_timeout(g, Duration::from_secs(60)));
                g = h;
                // LE JOUR A CHANGE : tout se reverifie (hors du verrou du fil : `tout_verifier` le reprend pour reveiller).
                if attente.timed_out() && !g.files.is_empty() {
                    let jour = Fil::jour_suivi();
                    if jour.is_some() && jour != g.jour {
                        g.jour = jour;
                        let files = g.files.clone();
                        drop(g);
                        for f in &files {
                            f.tout_verifier();
                        }
                        g = self.verrou();
                    }
                }
            };
            drop(g);
            let rendu = file.faire(cle);
            file.rendre(cle, numero, rendu);
        }
    }
}

/// LA FILE DES ETATS D'UNE COMPAGNIE : changer de mois est instantane. Dans le processus de la fenetre, le fil des calculs (`Fil`) refait ses etats, un a la fois : jamais
/// deux calculs du meme mois en meme temps, pas un processus par mois (sous Windows chaque processus coute, l'antivirus
/// l'inspecte). Le calcul est celui de `moteur route` (`garde_juste`, qui fait les memes routes par `routes::servir_la_route`),
/// garde d'une panique (`catch_unwind`) : une route qui tombe rend un refus, la fenetre reste.
///
/// Trois entrees : `demander` (une cle, devant ou a la suite), `tout_verifier` (au lancement de la fenetre, apres une
/// ecriture, a l'ouverture, apres chaque tour de veille : le mois affiche et ses voisins d'abord, puis l'ouverture, puis
/// les autres, du plus recent au plus ancien), et `attendre` (l'etat frais d'une cle, quand la page le demande ou
/// qu'aucun etat n'est garde). Un mois deja juste ne se recalcule pas : son passage dans la file ne coute que son
/// empreinte.
pub struct FileDesEtats {
    o: Options,
    racine: PathBuf,
    etat: Mutex<EtatDeLaFile>,
    signal: Condvar,
    fil: Arc<Fil>,
}

impl FileDesEtats {
    /// La file de la compagnie `racine`, sur un fil a elle seule. `date` : la date des calculs (`moteur route --date`, les
    /// tests). La fenetre, elle, met toutes ses compagnies sur le meme fil (`Fil::file`).
    pub fn nouvelle(racine: &Path, date: Option<Jour>) -> Arc<FileDesEtats> {
        Fil::nouveau().file(racine, date)
    }

    fn verrou(&self) -> std::sync::MutexGuard<'_, EtatDeLaFile> {
        tenu(self.etat.lock())
    }

    /// La compagnie de cette file.
    pub fn racine(&self) -> &Path {
        &self.racine
    }

    /// Le dernier etat connu de `cle`, JUSTE OU NON : celui de l'ouverture, ecrit (`ouverture_ecrite`) ; celui d'un mois,
    /// que la file garde en memoire depuis son dernier passage. Aucun pour le tableau (`tableau_juste` le lit).
    fn connu(&self, cle: Cle, jour: &Jour) -> Option<(Garde, bool)> {
        match cle {
            Cle::Ouverture => ouverture_ecrite(&self.racine).map(|e| {
                let juste = e.empreinte == empreinte(&self.racine, cle, jour);
                (Garde::Ouverture(Arc::new(e)), juste)
            }),
            Cle::Mois(_) => {
                let e = match self.verrou().fait.get(&cle) {
                    Some((_, Ok(Garde::Mois(e)))) => e.clone(),
                    _ => return None,
                };
                let juste = e.empreinte == empreinte(&self.racine, cle, jour);
                Some((Garde::Mois(e), juste))
            }
            Cle::Tableau => None,
        }
    }

    /// La prochaine cle a refaire, si la file en a une (sous le verrou du fil, `Fil::choisir`).
    fn prendre_travail(&self) -> Option<(Cle, u64)> {
        let mut e = self.verrou();
        if e.arretee {
            return None;
        }
        let m = e.attente.pop_front()?;
        // Chaque cle en attente a son numero (`inscrire` pose les deux ensemble).
        let n = *e.voulu.get(&m)?;
        e.en_cours = Some(m);
        Some((m, n))
    }

    /// Un passage fini : son rendu, et qui l'attendait est libere.
    fn rendre(&self, cle: Cle, numero: u64, rendu: Fait) {
        let mut e = self.verrou();
        e.en_cours = None;
        // L'OUVERTURE FAITE, le mois qu'elle ouvre passe devant : la page le demande juste apres.
        if e.affiche.is_none() {
            if let Ok(Garde::Ouverture(o)) = &rendu {
                if let Some(m) = o.ouvre {
                    Self::inscrire(&mut e, Cle::Mois(m), true);
                }
            }
        }
        e.fait.insert(cle, (numero, rendu));
        self.signal.notify_all();
    }

    /// L'etat juste de `cle` : le garde s'il l'est, sinon refait (`garde_juste`), ici, sans processus.
    fn faire(&self, cle: Cle) -> Fait {
        let o = &self.o;
        let _code = socle::fichiers::code_fige();
        let _geste = socle::forme::geste();
        let refaire = || {
            let rendu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let jour = jour()?;
                if let Some((g, true)) = self.connu(cle, &jour) {
                    return Ok(g);
                }
                let ctx = crate::service::contexte(o).map_err(|m| Echec { classe: None, phrase: m, a_part: None })?;
                garde_juste(o, &ctx, cle)
            }));
            // Un calcul tombe a pu laisser son fil sale (un registre ouvert, les nombres « fideles ») : le travail
            // suivant, d'une autre compagnie peut-etre, repart d'un fil propre.
            crate::remettre_le_fil();
            rendu
        };
        // LE BANC D'ESSAI : ce que l'etat refait ecrit entre dans le scenario (`scenario::Enregistreur::fond`).
        #[cfg(feature = "atelier")]
        let rendu = match crate::scenario::enregistreur_actif() {
            Some(e) => e.fond(&self.racine, &cle.to_string(), refaire),
            None => refaire(),
        };
        #[cfg(not(feature = "atelier"))]
        let rendu = refaire();
        match rendu {
            Ok(Ok(g)) => Ok(g),
            Ok(Err(e)) => Err((e.classe, String::from(e))),
            Err(p) => Err((Some(Classe::DefautDuLogiciel), format!("Défaut du logiciel : l'état de {} ne s'est pas refait (le calcul s'est arrêté : {}).", cle, pourquoi(p.as_ref())))),
        }
    }

    /// `cle` a verifier : devant (elle est attendue), ou a la suite si elle n'est pas deja dans la file. Rend le numero
    /// de la demande : un passage qui commence apres elle la sert.
    fn demander(&self, cle: Cle, devant: bool) -> u64 {
        let n = {
            let mut e = self.verrou();
            Self::inscrire(&mut e, cle, devant)
        };
        self.fil.reveiller();
        n
    }

    fn inscrire(e: &mut EtatDeLaFile, cle: Cle, devant: bool) -> u64 {
        e.numero += 1;
        let n = e.numero;
        e.voulu.insert(cle, n);
        let deja = e.attente.iter().position(|m| *m == cle);
        match (deja, devant) {
            (Some(i), true) => {
                e.attente.remove(i);
                e.attente.push_front(cle);
            }
            (Some(_), false) => {}
            (None, true) => e.attente.push_front(cle),
            (None, false) => e.attente.push_back(cle),
        }
        n
    }

    /// TOUS LES MOIS SUIVIS A VERIFIER (au lancement, apres une ecriture, a l'ouverture, apres un tour de veille) : le mois
    /// affiche et ses voisins d'abord, puis l'ouverture (qui fait passer devant le mois qu'elle ouvre), puis -- pour une
    /// compagnie de l'administrateur -- son tableau, puis les autres mois, du plus recent au plus ancien. Ceux qui sont
    /// deja dans la file y restent a leur place, sauf les premiers, qui passent devant. Des mois saisis qui ne se lisent
    /// pas ne se verifient pas d'avance (c'est note) : chacun dira son refus quand il s'affichera.
    pub fn tout_verifier(&self) {
        let suivis = match mois_suivis(&self.racine) {
            Ok(s) => s,
            Err(e) => {
                noter("les mois suivis ne se lisent pas", &String::from(e));
                Vec::new()
            }
        };
        let tableau = match crate::gestes::administrateur::est_rattachee(&crate::gestes::administrateur::installation_de(&self.racine), &self.racine) {
            Ok(t) => t,
            Err(m) => {
                noter("le registre de l'administrateur ne se lit pas", &m);
                false
            }
        };
        {
            let mut e = self.verrou();
            let mut premiers: Vec<Mois> = Vec::new();
            if let Some(a) = e.affiche {
                premiers.push(a);
                premiers.extend([a.precedent(), a.suivant()].into_iter().filter(|v| suivis.contains(v)));
            }
            for m in suivis.iter().rev().filter(|m| !premiers.contains(m)) {
                Self::inscrire(&mut e, Cle::Mois(*m), false);
            }
            if tableau {
                Self::inscrire(&mut e, Cle::Tableau, true);
            }
            // L'OUVERTURE : juste apres le mois affiche et ses voisins ; la premiere quand aucun mois n'est
            // encore affiche (au lancement, a l'ouverture d'une compagnie).
            Self::inscrire(&mut e, Cle::Ouverture, true);
            for m in premiers.iter().rev() {
                Self::inscrire(&mut e, Cle::Mois(*m), true);
            }
        }
        self.fil.reveiller();
    }

    /// L'etat frais de `cle` : un passage commence apres cet appel, et l'on attend sa fin.
    fn attendre(&self, cle: Cle) -> R<Garde> {
        let n = self.demander(cle, true);
        let mut e = self.verrou();
        loop {
            if let Some((fait, rendu)) = e.fait.get(&cle) {
                if *fait >= n {
                    return refus(rendu);
                }
            }
            if e.arretee {
                return Err(erreur(Classe::Refuse, "La compagnie a été fermée."));
            }
            e = tenu(self.signal.wait(e));
        }
    }

    /// LE TABLEAU DE CETTE COMPAGNIE, frais : refait par le fil (devant), ou le garde s'il est juste.
    pub fn tableau(&self) -> R<TableauCompagnie> {
        match self.attendre(Cle::Tableau)? {
            Garde::Tableau(t) => Ok(t.as_ref().clone()),
            _ => Err(Garde::defaut("le tableau")),
        }
    }

    /// L'ECRAN DE `mois` (la route `mois` de la fenetre) : le dernier etat connu, tout de suite, meme perime -- marque
    /// `perime`, et refait aussitot, devant tous les autres. `attendre` (la page qui veut l'etat frais) ou aucun etat
    /// garde : l'etat frais, une fois refait.
    pub fn ecran(&self, charge: &Charge) -> R<Rendu> {
        let ChargeEcranDuMois { mois: m, attendre } = charge.lire()?;
        let attendre = attendre == Some(true);
        let cle = Cle::Mois(m);
        self.verrou().affiche = Some(m);
        let jour = jour()?;
        let (etat, perime) = match self.connu(cle, &jour) {
            Some((etat, true)) => (etat, false),
            Some((etat, false)) if !attendre => {
                self.demander(cle, true);
                (etat, true)
            }
            _ => (self.attendre(cle)?, false),
        };
        let etat = etat.du_mois()?;
        en_json(etat.ecran(&self.racine, perime))
    }

    /// L'etat garde `cle`, s'il y en a un (juste, si `juste` l'exige) ; sinon il est demande devant et attendu jusqu'a
    /// `limite` -- `Ok(None)` si elle passe avant lui. Le passage rend l'etat (juste) ou son refus.
    fn obtenir(&self, cle: Cle, jour: &Jour, limite: Instant, juste: bool) -> R<Option<(Garde, bool)>> {
        if let Some(x) = self.connu(cle, jour).filter(|(_, j)| *j || !juste) {
            return Ok(Some(x));
        }
        // Deja en route (la page qui redemande) : on attend ce passage-la, sans en demander un de plus.
        let en_route = {
            let e = self.verrou();
            (e.en_cours == Some(cle) || e.attente.contains(&cle)).then(|| e.voulu.get(&cle).copied()).flatten()
        };
        let n = match en_route {
            Some(n) => n,
            None => self.demander(cle, true),
        };
        let mut e = self.verrou();
        loop {
            if let Some((fait, rendu)) = e.fait.get(&cle) {
                if *fait >= n {
                    return refus(rendu).map(|g| Some((g, true)));
                }
            }
            if e.arretee {
                return Err(erreur(Classe::Refuse, "La compagnie a été fermée."));
            }
            let reste = limite.saturating_duration_since(Instant::now());
            if reste.is_zero() {
                return Ok(None);
            }
            e = tenu(self.signal.wait_timeout(e, reste)).0;
        }
    }

    /// CE QUE FAIT LE MOTEUR, en une phrase (jamais une roue qui tourne sans dire pourquoi) : ce que le fil refait en ce
    /// moment.
    pub fn phrase(&self) -> String {
        match self.verrou().en_cours {
            Some(Cle::Ouverture) => "Lecture de la compagnie : sa configuration, ses échéances, ses rappels…".into(),
            Some(Cle::Tableau) => "Tableau des déclarations de la compagnie…".into(),
            Some(Cle::Mois(m)) => format!("Calcul de {}…", m.libelle(socle::mois::Graphie::Accentuee)),
            None => "Préparation de l'écran…".into(),
        }
    }

    /// L'OUVERTURE D'UNE COMPAGNIE (la route `ouverture` de la fenetre) : d'un coup, ce que la page lit en
    /// s'ouvrant -- les lectures de l'ouverture (`lectures`, une entree par route, au format d'une reponse de la page), le
    /// mois a ouvrir (`mois`) et l'ecran de ce mois (`ecran`, celui de la route `mois`). Les lectures sont JUSTES : la
    /// page ne les lit qu'une fois. L'ecran du mois est le dernier connu, tout de suite, marque `perime` s'il l'est
    /// (`ecran.perime`), et refait aussitot. `pret: true`.
    ///
    /// Sans lectures justes ou sans etat connu du mois (compagnie neuve, etat illisible ou d'un autre moteur, une
    /// ecriture depuis) : il se fait, et l'on attend au plus `patienter` ; passe ce delai, `{"pret": false, "phrase"}` dit
    /// ce que fait le moteur, et la page redemande. `frais` : les lectures de l'ouverture une fois refaites, sans l'ecran
    /// du mois (l'Historique qui se relit).
    ///
    /// `aujourdhui` : LE JOUR DU MOTEUR, celui que la page prend pour tout ce qu'elle date ou presente au jour (une
    /// date d'effet proposee, l'annee du taux AT/MP, le mois en cours de la grille) -- jamais l'horloge du navigateur :
    /// au banc d'essai le moteur a son temps simule (`temps`), et c'est le seul qui vaille (regle 2).
    pub fn demarrage(&self, charge: &Charge, a_cote: impl FnOnce() -> ACote) -> R<Rendu> {
        let ChargeDemarrage { patienter } = charge.lire()?;
        let jour = jour()?;
        let limite = Instant::now() + Duration::from_millis(patienter.min(5000));
        let attente = || en_json(Demarrage::en_attente(self.phrase()));
        let Some((ouv, _)) = self.obtenir(Cle::Ouverture, &jour, limite, true)? else { return attente() };
        let ouv = ouv.de_l_ouverture()?;
        let mois = ouv.mois(&jour);
        self.verrou().affiche = Some(mois);
        let Some((etat, mois_juste)) = self.obtenir(Cle::Mois(mois), &jour, limite, false)? else { return attente() };
        // Refait en fond, devant : ses cartes sont ce qui se voit le plus.
        if !mois_juste {
            self.demander(Cle::Mois(mois), true);
        }
        let etat = etat.du_mois()?;
        en_json(Demarrage::pret(&self.racine, &ouv, &etat, !mois_juste, &jour, a_cote()))
    }

    /// La compagnie n'est plus tenue : son travail s'arrete apres le calcul en cours, et qui attendait est libere. Le fil
    /// qui n'a plus aucune compagnie s'arrete aussi (celui d'une file seule, `nouvelle`).
    pub fn arreter(&self) {
        self.fermer();
        let mut g = self.fil.verrou();
        g.files.retain(|f| !std::ptr::eq(Arc::as_ptr(f), self));
        if g.devant.as_ref().is_some_and(|d| std::ptr::eq(Arc::as_ptr(d), self)) {
            g.devant = None;
        }
        if g.files.is_empty() {
            g.arrete = true;
        }
        self.fil.reveil.notify_all();
    }

    /// LA COMPAGNIE S'EN VA (supprimee, ou deplacee quand on la garde au deblocage : `accueil::suppression`) : sa file
    /// quitte le fil, qui continue de servir les autres -- meme s'il n'en reste aucune, la fenetre lui en donnera d'autres --,
    /// et l'on attend la fin du calcul qui tourne pour elle : rien ne s'ecrit plus dans son dossier ensuite.
    pub fn quitter(&self) {
        self.fermer();
        {
            let mut g = self.fil.verrou();
            g.files.retain(|f| !std::ptr::eq(Arc::as_ptr(f), self));
            if g.devant.as_ref().is_some_and(|d| std::ptr::eq(Arc::as_ptr(d), self)) {
                g.devant = None;
            }
        }
        let mut e = self.verrou();
        while e.en_cours.is_some() {
            e = tenu(self.signal.wait(e));
        }
    }

    fn fermer(&self) {
        self.verrou().arretee = true;
        self.signal.notify_all();
    }
}
