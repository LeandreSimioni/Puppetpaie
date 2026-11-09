//! L'ouverture du logiciel et la veille : `ouverture`, `Ouverture`, `derniere_ouverture`.
//!
//! Ce que le logiciel fait tout seul quand on l'ouvre -- interface ou ligne de commande, c'est la meme fonction -- et
//! toutes les vingt minutes ensuite : l'horloge de la compagnie fictive, les retours DSN et DPAE,
//! les taux de PAS, les signatures et les fiches, les incidents et les reponses de l'editeur, la
//! sauvegarde en dernier, puis la trace `config/.ouverture.json`.
//!
//! REGLE : L'OUVERTURE N'ECHOUE JAMAIS. `ouverture` ne rend pas de `Result` : pas de reseau, pas de compte Google, pas de
//! secret pose, un coffre qui refuse de s'ouvrir -- elle le NOTE (un incident, un manque, `a_poser`) et rend la main.
//! La paie se calcule tres bien hors ligne.
//!
//! Chaque acte est celui de son module, appele tel quel --
//! `releve_retours::{relever_retours, relever_retours_dpae, rattacher_simulation}`,
//! `releve_signatures::{relever_signatures, relever_signatures_fictive}`,
//! `crate::routes::fiches::relever_fiches`, `horloge::{rattraper, dernier_rattrapage}`,
//! `incidents::envoyer` et `problemes::relever_reponses` (le canal de l'editeur), `sauvegarde::{sauvegarder,
//! rien_n_a_bouge, hors_ligne}`, `crate::routes::coffre::autorise`, `coffre::Coffre`.
//!
//! CHAQUE CLIENT RESEAU PEUT ETRE REMPLACE (`Clients`) : le compte net-entreprises, le compte Urssaf (DPAE), la boite
//! courriel (signatures et fiches), le Drive. Un client donne est utilise a la place du vrai, SANS regarder le coffre.
//! Le serveur des incidents est celui de la configuration (`incidents::configuration`), qu'un serveur local peut tenir.
//!
//! LA MISE A JOUR N'EST PAS ICI : le logiciel la cherche pour l'installation (`maj::chercher_en_fond`) et l'indicateur la
//! dit (`maj::Recherche::etat`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use droit::contexte::Contexte;
use envois::courriel::Courriel;
use envois::drive::Drive;
use envois::etats;
use socle::echec::R;
use envois::releve_retours::{Client, Releve};
use envois::releve_signatures::ReleveDite;
use crate::gestes::releve_signatures::{self, ReleveSignatures};
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use crate::routes::fiches::{FicheRecue, ReleveFiches};
use crate::compagnie::sauvegarde::{self, Sauvegarde};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Auto(Place::Historique, Affiche::new("Relever maintenant"));

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Ouverture,
        geste: "Ouvrir la compagnie : sauvegarder, relever retours, signatures et fiches.",
        charge: &[
            Champ { nom: "sauvegarder", libelle: Affiche::new("Sauvegarder"), forme: Forme::OuiNon, sens: "faux : ne pas sauvegarder dans le Drive", avance: true },
            Champ { nom: "retours", libelle: Affiche::new("Relever les retours"), forme: Forme::OuiNon, sens: "faux : ne pas relever les retours des organismes", avance: true },
            Champ { nom: "signatures", libelle: Affiche::new("Relever les signatures"), forme: Forme::OuiNon, sens: "faux : ne pas relever les signatures", avance: true },
            Champ { nom: "veille", libelle: Affiche::new("Un tour de veille"), forme: Forme::OuiNon, sens: "vrai : un tour de veille", avance: true },
        ],
        exemple: r#"{}"#,
        rend: "ce que chaque passage a fait (sauvegarde, retours, signatures, fiches, filet), et les incidents en phrases ; ne refuse jamais",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// `compagnies.MARQUEUR`.
use socle::fichiers::MARQUEUR_COPIE;
use crate::routes::catalogue::{Ecran, Place};

/// La trace de la derniere ouverture : `config/.ouverture.json`.
const TRACE: &str = ".ouverture.json";

// ------------------------------------------------------------------ les options et les clients

/// Les options de `ouverture(...)`.
#[derive(Debug, Clone)]
pub struct Options {
    /// Sauvegarder le dossier prive dans le Drive (en dernier).
    pub sauvegarder: bool,
    /// Relever les retours DSN et DPAE.
    pub retours: bool,
    /// Relever les signatures et les fiches.
    pub signatures: bool,
    /// Ce qui part chez l'editeur (incidents, reponses aux signalements).
    pub filet: bool,
    /// Combien de jours en arriere on cherche les retours.
    pub jours: i64,
    /// Le dossier racine dans le Drive.
    pub racine_drive: String,
    /// Ne rien refaire si la derniere ouverture a moins de N minutes (0 : toujours).
    pub pas_avant: i64,
    /// Le passage periodique : saute la sauvegarde quand rien n'a bouge sur le disque.
    pub veille: bool,
    /// Envoyer chez l'editeur ce que le journal des incidents a note, et relever ses reponses, avant de rendre la main ;
    /// un processus `moteur route` ne peut pas attendre un fil. Mettre `false` quand l'appelant le lance lui-meme
    /// (route `incidents-envoyer`, dans son propre fil).
    pub incidents_editeur: bool,
    /// La SECONDE moitie de la veille : ne fait que la sauvegarde, et met la trace du premier appel a
    /// jour (sauvegarde, hors ligne, incidents, manque `google`) sans rien effacer de ce qu'il a ecrit. Le premier appel
    /// (`sauvegarder: false`) a releve et ecrit la trace ; l'appelant a dit « releve fait » ; celui-ci sauvegarde, en dernier.
    pub sauvegarde_seule: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            sauvegarder: true,
            retours: true,
            signatures: true,
            filet: true,
            jours: 30,
            racine_drive: sauvegarde::RACINE_DRIVE.to_string(),
            pas_avant: 0,
            veille: false,
            incidents_editeur: true,
            sauvegarde_seule: false,
        }
    }
}

/// LA CHARGE DE LA ROUTE `ouverture` : chaque option dite, ou absente. Les interrupteurs disent `false` pour ne pas faire
/// (sauvegarder, retours, signatures, filet, incidents_editeur) et `true` pour faire autrement (veille, sauvegarde_seule) ;
/// `jours`, `pas_avant` et `racine_drive` remplacent ce que fait un passage ordinaire (`Options::default`).
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeOuverture {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sauvegarder: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retours: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signatures: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filet: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub veille: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incidents_editeur: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sauvegarde_seule: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jours: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pas_avant: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub racine_drive: Option<String>,
}

impl Options {
    /// Les options d'une charge de route : un passage ordinaire, et ce que la charge dit autrement.
    pub fn de_la_charge(c: ChargeOuverture) -> Options {
        let mut o = Options::default();
        o.sauvegarder = c.sauvegarder != Some(false);
        o.retours = c.retours != Some(false);
        o.signatures = c.signatures != Some(false);
        o.filet = c.filet != Some(false);
        o.incidents_editeur = c.incidents_editeur != Some(false);
        o.veille = c.veille == Some(true);
        o.sauvegarde_seule = c.sauvegarde_seule == Some(true);
        if let Some(j) = c.jours {
            o.jours = j;
        }
        if let Some(p) = c.pas_avant {
            o.pas_avant = p;
        }
        if let Some(r) = c.racine_drive {
            o.racine_drive = r;
        }
        o
    }
}

/// Les clients reseau que l'appelant peut donner a la place des vrais (la preuve : un faux, ou un serveur local). `None` :
/// le vrai, construit ici. Un client donne n'est jamais confronte au coffre.
#[derive(Default)]
pub struct Clients<'a> {
    /// Le compte net-entreprises (retours DSN).
    pub dsn: Option<Client>,
    /// Le compte Urssaf (retours DPAE).
    pub dpae: Option<Client>,
    /// La boite courriel, pour les signatures ET les fiches.
    pub courriel: Option<&'a mut (dyn Courriel + 'a)>,
    /// Le Drive.
    pub drive: Option<&'a mut (dyn Drive + 'a)>,
}

// ------------------------------------------------------------------ ce que l'ouverture rend

/// `Ouverture` : le compte rendu d'un passage.
#[derive(Default)]
pub struct Ouverture {
    /// `JJ/MM/AAAA HH:MM`, l'heure de Paris.
    pub quand: String,
    pub sauvegarde: Option<Sauvegarde>,
    /// Les retours DSN (dans une compagnie fictive : un releve vide qui ne porte que `rattaches`).
    pub releve: Option<Releve>,
    pub releve_dpae: Option<Releve>,
    pub signatures: Option<ReleveSignatures>,
    pub fiches: Option<ReleveFiches>,
    pub incidents: Vec<String>,
    pub manquants: Vec<String>,
    /// Ce qui manque au coffre, nomme : `google`, `net-entreprises`, `urssaf`.
    pub a_poser: Vec<String>,
    /// Les phrases de ces manques, a ne pas redire dans la trace (`a_poser` les dit).
    pub phrases_coffre: Vec<String>,
    /// Ce que l'horloge de la compagnie fictive a joue aujourd'hui, en phrases.
    pub horloge: Vec<String>,
    pub secondes: f64,
    /// Le reseau manquait : dit une fois, pas par erreur.
    pub hors_ligne: bool,
    /// Ce qui n'a pas repondu, une phrase par service (l'erreur telle qu'elle est venue) : le reseau manquait pour CELA,
    /// pas forcement pour tout -- la releve a pu passer quand le Drive ne repondait pas.
    pub injoignables: Vec<String>,
    /// `pas_avant` a retenu ce passage (la derniere ouverture est trop recente) : rien n'a ete fait, la trace n'est pas
    /// reecrite.
    pub retenue: bool,
}

impl Ouverture {
    /// `Ouverture.tout_va_bien`.
    pub fn tout_va_bien(&self) -> bool {
        self.incidents.is_empty() && self.manquants.is_empty()
    }

    /// Un secret absent -- la phrase du terminal, gardee pour ne pas la redire dans la fenetre.
    fn manque_coffre(&mut self, phrase: &str) {
        self.manquants.push(phrase.to_string());
        self.phrases_coffre.push(phrase.to_string());
    }

    /// `ReleveRetours.resume`.
    fn resume_retours(releve: &Releve) -> String {
        if releve.trouves.is_empty() {
            return "Aucun retour sur la période.".to_string();
        }
        format!("{} retour(s), dont {} nouveau(x), {} archive(s).", releve.trouves.len(), releve.nouveaux.len(), releve.archives.len())
    }

    /// Ce que la page et `moteur route` lisent du passage, avec le detail des fichiers deposes et des retours archives :
    /// la phrase du passage, et une ligne par chose faite ou trouvee (ce que l'horloge a joue, ce qui manque, les
    /// incidents).
    pub fn dite(self) -> OuvertureDite {
        let retours_resume = self.releve.as_ref().map(Ouverture::resume_retours);
        let phrase = match &retours_resume {
            Some(r) => format!("Ouverture du {} : {}", self.quand, r),
            None => format!("Ouverture du {}.", self.quand),
        };
        let phrases: Vec<String> = self.horloge.iter().chain(&self.manquants).chain(&self.incidents).cloned().collect();
        let sauvegarde = self.sauvegarde.as_ref().map(|s| SauvegardeDite {
            resume: s.resume(),
            deposes: s.deposes.iter().map(|f| f.relatif.clone()).collect(),
            identiques: s.identiques,
            octets: s.octets_deposes,
            deposes_detail: s.deposes.iter().map(|f| FichierDepose { relatif: f.relatif.clone(), etat: f.etat.texte(), octets: f.octets }).collect(),
            exclus: s.exclus,
            hors_ligne: s.hors_ligne,
            souci: s.persistant.clone(),
        });
        let retours = self.releve.as_ref().map(|r| RetoursDits {
            resume: Ouverture::resume_retours(r),
            trouves: r.trouves.len(),
            nouveaux: r.nouveaux.len(),
            archives: r.archives.iter().map(|a| a.chemin.to_string_lossy().into_owned()).collect(),
            archives_detail: r
                .archives
                .iter()
                .map(|a| RetourArchiveDit {
                    chemin: a.chemin.to_string_lossy().into_owned(),
                    fichier: a.chemin.file_name().map(|n| n.to_string_lossy().into_owned()),
                    nature: a.retour.nature.clone(),
                    libelle: envois::retours::libelle_nature(&a.retour.nature),
                })
                .collect(),
            rattaches: r.rattaches,
        });
        let retours_dpae = self.releve_dpae.as_ref().map(|r| RetoursDpaeDits {
            resume: Ouverture::resume_retours(r),
            trouves: r.trouves.len(),
            nouveaux: r.nouveaux.len(),
            archives: r.archives.iter().map(|a| a.chemin.to_string_lossy().into_owned()).collect(),
            rattaches: r.rattaches,
        });
        let fiches = self.fiches.as_ref().map(|f| FichesDites { resume: f.resume(), attendues: f.attendues, recues: f.recues.clone(), incidents: f.incidents.clone() });
        let tout_va_bien = self.tout_va_bien();
        OuvertureDite {
            phrase,
            quand: self.quand,
            secondes: self.secondes,
            sauvegarde,
            retours,
            signatures: self.signatures.map(ReleveSignatures::dite),
            incidents: self.incidents,
            manquants: self.manquants,
            retours_dpae,
            fiches,
            a_poser: self.a_poser,
            horloge: self.horloge,
            hors_ligne: self.hors_ligne,
            injoignables: self.injoignables,
            tout_va_bien,
            retenue: self.retenue,
            phrases,
        }
    }
}

/// CE QUE REND LA ROUTE `ouverture` (`Ouverture::dite`) : la phrase du passage, ce que chaque releve a fait, et ses lignes
/// (`phrases`).
#[derive(Debug, Serialize)]
pub struct OuvertureDite {
    pub phrase: String,
    pub quand: String,
    pub secondes: f64,
    pub sauvegarde: Option<SauvegardeDite>,
    pub retours: Option<RetoursDits>,
    pub signatures: Option<ReleveDite>,
    pub incidents: Vec<String>,
    pub manquants: Vec<String>,
    pub retours_dpae: Option<RetoursDpaeDits>,
    pub fiches: Option<FichesDites>,
    pub a_poser: Vec<String>,
    pub horloge: Vec<String>,
    pub hors_ligne: bool,
    pub injoignables: Vec<String>,
    pub tout_va_bien: bool,
    /// `pas_avant` a retenu ce passage : rien n'a ete fait.
    pub retenue: bool,
    pub phrases: Vec<String>,
}

impl Dite for OuvertureDite {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// La sauvegarde d'un passage : sa ligne, ce qui est parti, ce qui reste en souci (`sauvegarde::souci`).
#[derive(Debug, Serialize)]
pub struct SauvegardeDite {
    pub resume: String,
    pub deposes: Vec<String>,
    pub identiques: usize,
    pub octets: u64,
    pub deposes_detail: Vec<FichierDepose>,
    pub exclus: usize,
    pub hors_ligne: bool,
    pub souci: Option<String>,
}

/// Un fichier parti au Drive : son chemin dans le dossier de la compagnie, neuf ou modifie, son poids.
#[derive(Debug, Serialize)]
pub struct FichierDepose {
    pub relatif: String,
    pub etat: &'static str,
    pub octets: u64,
}

/// Les retours DSN d'un passage : sa ligne, combien, ce qui s'est archive et rattache.
#[derive(Debug, Serialize)]
pub struct RetoursDits {
    pub resume: String,
    pub trouves: usize,
    pub nouveaux: usize,
    pub archives: Vec<String>,
    pub archives_detail: Vec<RetourArchiveDit>,
    pub rattaches: i64,
}

/// Un retour archive : ou, sa nature et son libelle.
#[derive(Debug, Serialize)]
pub struct RetourArchiveDit {
    pub chemin: String,
    pub fichier: Option<String>,
    pub nature: String,
    pub libelle: String,
}

/// Les retours DPAE d'un passage.
#[derive(Debug, Serialize)]
pub struct RetoursDpaeDits {
    pub resume: String,
    pub trouves: usize,
    pub nouveaux: usize,
    pub archives: Vec<String>,
    pub rattaches: i64,
}

/// Les fiches de salarie revenues a un passage.
#[derive(Debug, Serialize)]
pub struct FichesDites {
    pub resume: String,
    pub attendues: usize,
    pub recues: Vec<FicheRecue>,
    pub incidents: Vec<String>,
}

// ------------------------------------------------------------------ la route

/// La route `ouverture` (`ChargeOuverture` -> `OuvertureDite`). **Hors d'une compagnie fictive, elle va au reseau.** Elle
/// ne refuse que sa charge mal formee : une panne est une phrase de `incidents`.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::Ouverture => {
            let dite: OuvertureDite = ouverture(porte.ctx, &Options::de_la_charge(charge.lire()?), Clients::default(), None).dite();
            en_json(dite)
        }
        autre => Err(crate::routes::enveloppe::route_inconnue(autre.nom())),
    }
}

// ------------------------------------------------------------------ l'etape en cours

/// L'ETAPE EN COURS D'UN PASSAGE : qui on appelle, puis qui d'autre, la sauvegarde -- un message dans la ligne qui
/// explique ce que fait la machine. L'ouverture tourne dans un processus a part (la veille
/// appelle la route `ouverture`) : elle ecrit son etape dans `config/.ouverture-etape.json` en y entrant, l'efface en
/// finissant ; la veille de la fenetre la lit (`Veille::etape`) tant qu'un passage tourne. Le fichier ne monte pas au Drive
/// (`sauvegarde::NOMS_EXCLUS`). Une ecriture qui echoue est passee sous silence : ce n'est qu'un message.
pub const ETAPE: &str = ".ouverture-etape.json";

fn chemin_etape(racine: &Path) -> std::path::PathBuf {
    racine.join(socle::fichiers::CONFIG).join(ETAPE)
}

/// L'etape d'un passage en cours (`config/.ouverture-etape.json`) : ce qu'il fait, depuis quand.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Etape {
    etape: String,
    depuis: String,
}

fn dire_l_etape(ctx: &Contexte, etape: &str) {
    let _ = socle::ecriture::ecrire_json(&chemin_etape(&ctx.racine), &Etape { etape: etape.to_string(), depuis: quand_maintenant() }, 1);
}

fn finir_les_etapes(ctx: &Contexte) {
    let _ = std::fs::remove_file(chemin_etape(&ctx.racine));
}

/// L'etape qu'un passage en cours a ecrite, ou `None` quand aucun n'en a ecrit.
pub fn etape_en_cours(racine: &Path) -> R<Option<String>> {
    let p = chemin_etape(racine);
    if !p.exists() {
        return Ok(None);
    }
    let e: Etape = socle::forme::lire_fichier(&p, "L'étape du passage en cours ne se lit pas.")?;
    Ok(Some(e.etape))
}

// ------------------------------------------------------------------ la trace

fn chemin_trace(ctx: &Contexte) -> std::path::PathBuf {
    ctx.racine.join(socle::fichiers::CONFIG).join(TRACE)
}

/// LA TRACE DE LA DERNIERE OUVERTURE (`config/.ouverture.json`), que le bandeau de la fenetre lit : quand, combien de
/// temps, la ligne de chaque releve qui a eu lieu, ce qui n'a pas repondu, ce qui manque et ce qui est a poser. Aucun
/// secret n'y figure, par construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    pub quand: String,
    pub secondes: f64,
    /// La ligne de la sauvegarde, quand elle a eu lieu.
    pub sauvegarde: Option<String>,
    /// Ce qui persiste de la sauvegarde (`sauvegarde::souci`).
    pub sauvegarde_souci: Option<String>,
    /// La ligne des retours, quand la releve a joint net-entreprises.
    pub retours: Option<String>,
    pub hors_ligne: bool,
    pub injoignables: Vec<String>,
    pub nouveaux: usize,
    /// La ligne des signatures, quand la releve a eu lieu.
    pub signatures: Option<String>,
    /// La ligne des fiches de salarie, quand la releve a eu lieu.
    pub fiches: Option<String>,
    pub signatures_recues: Vec<SignatureNotee>,
    pub incidents: Vec<String>,
    pub manquants: Vec<String>,
    pub a_poser: Vec<String>,
    pub horloge: Vec<String>,
}

/// Un contrat revenu signe, tel que la trace le note.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignatureNotee {
    pub nom: String,
    pub quand: String,
    pub renvoye: bool,
    pub phrase: String,
}

/// `derniere_ouverture` : la trace, ou `None` quand aucune ouverture ne l'a encore ecrite. Une trace qui ne se lit pas se
/// refuse.
pub fn derniere_ouverture(racine: &std::path::Path) -> R<Option<Trace>> {
    let p = racine.join(socle::fichiers::CONFIG).join(TRACE);
    if !p.exists() {
        return Ok(None);
    }
    Ok(Some(socle::forme::lire_fichier(&p, "La trace de la dernière ouverture ne se lit pas.")?))
}

/// La trace de la derniere ouverture. Une ecriture qui echoue est passee sous silence : la trace n'est qu'un compte rendu.
fn noter_ouverture(ctx: &Contexte, r: &mut Ouverture) {
    let p = chemin_trace(ctx);
    if let Some(dossier) = p.parent() {
        let _ = std::fs::create_dir_all(dossier);
    }
    // LES RETOURS DE LA COMPAGNIE, PAS CEUX DU BAC A SABLE : les retours des depots de test ne se comptent que
    // dans la copie de developpement.
    let tests: Vec<String> = if est_copie(&ctx.racine) {
        Vec::new()
    } else {
        // Un journal illisible n'empeche pas la trace (l'ouverture n'echoue jamais) : il est un incident.
        match envois::journal::lu(&ctx.racine) {
            Ok(j) => etats::flux_de_test(&j.lignes),
            Err(e) => {
                r.incidents.push(format!("Journal : {}", e));
                Vec::new()
            }
        }
    };
    let hors_tests = |liste: &[envois::retours::Annonce]| -> usize { liste.iter().filter(|x| !tests.iter().any(|t| x.flux.as_deref() == Some(t.as_str()))).count() };
    let (trouves, nouveaux) = match &r.releve {
        Some(rel) => (hors_tests(&rel.trouves), hors_tests(&rel.nouveaux)),
        None => (0, 0),
    };
    // La ligne des retours dit ce que la releve a fait, si ELLE a joint net-entreprises -- le Drive qui ne repond pas ne
    // la tait pas ; une releve sans reseau, elle, n'a rien vu, et « aucun retour » serait faux.
    let releve_passee = r.releve.as_ref().is_some_and(|rel| !rel.incidents.iter().any(|i| sauvegarde::hors_ligne(i)));
    let retours = releve_passee.then(|| {
        if trouves > 0 {
            format!("{} retour(s), dont {} nouveau(x).", trouves, nouveaux)
        } else {
            "Aucun retour sur la période.".to_string()
        }
    });
    let sauvegarde_souci = souci_de_la_sauvegarde(ctx, r);
    let trace = Trace {
        quand: r.quand.clone(),
        secondes: r.secondes,
        sauvegarde: r.sauvegarde.as_ref().map(Sauvegarde::resume),
        sauvegarde_souci,
        retours,
        hors_ligne: r.hors_ligne,
        injoignables: r.injoignables.clone(),
        nouveaux,
        signatures: r.signatures.as_ref().map(ReleveSignatures::resume),
        fiches: r.fiches.as_ref().map(ReleveFiches::resume),
        signatures_recues: r
            .signatures
            .iter()
            .flat_map(|s| &s.recues)
            .map(|x| SignatureNotee { nom: x.nom.clone(), quand: x.quand.clone(), renvoye: x.renvoye, phrase: format!("contrat signé par {}", x.nom) })
            .collect(),
        incidents: r.incidents.clone(),
        // Les phrases du coffre ne repartent pas : `a_poser` les dit.
        manquants: r.manquants.iter().filter(|m| !r.phrases_coffre.contains(m)).cloned().collect(),
        a_poser: r.a_poser.clone(),
        horloge: r.horloge.clone(),
    };
    let _ = socle::ecriture::ecrire_json(&p, &trace, 1);
}

/// Ce qui persiste de la sauvegarde (`sauvegarde::souci`) ; un index qui ne se lit pas est un incident du passage.
fn souci_de_la_sauvegarde(ctx: &Contexte, r: &mut Ouverture) -> Option<String> {
    match sauvegarde::souci(&ctx.racine) {
        Ok(s) => s,
        Err(e) => {
            r.incidents.push(format!("Sauvegarde : {}", e));
            None
        }
    }
}

// ------------------------------------------------------------------ la version qui calcule

/// Ou la compagnie garde la version qui a passe sur ses mois finis : `config/.version-du-calcul.json`.
const VERSION_DU_CALCUL: &str = ".version-du-calcul.json";

/// LES MOIS FINIS ET LA VERSION QUI CALCULE : un mois fini (ses documents et sa DSN partis) ne se relit plus de lui-meme
/// (`engagement::etats::fini`). Une version qui change ce qui calcule (son numero, `VERSION`, et la marque du code,
/// `engagement::empreinte::MOTEUR`) passe une fois sur eux, a la premiere ouverture de la compagnie qui la porte
/// (`engagement::etats::corriger_les_mois_finis`) : une decision de l'editeur y refait la paie d'un mois, en
/// rectificatif ; puis plus rien jusqu'a la version suivante. La marque ne s'ecrit qu'apres un passage entier : un
/// passage interrompu se refait.
fn passer_la_version(ctx: &Contexte) -> R<()> {
    let marque = VersionDuCalcul { version: format!("{}+{}", crate::compagnie::maj::courante(&crate::compagnie::maj::dossier_installe()?)?, engagement::empreinte::MOTEUR) };
    let p = ctx.racine.join(socle::fichiers::CONFIG).join(VERSION_DU_CALCUL);
    if p.exists() {
        let lue: VersionDuCalcul = socle::forme::lire_fichier(&p, "La version qui a passé sur les mois finis ne se lit pas.")?;
        if lue.version == marque.version {
            return Ok(());
        }
    }
    dire_l_etape(ctx, "Nouvelle version : relecture des mois finis…");
    let departs = envois::journal::lu(&ctx.racine)?.departs;
    engagement::etats::corriger_les_mois_finis(ctx, &departs)?;
    socle::ecriture::ecrire_json(&p, &marque, 1).map_err(|m| socle::echec::erreur(socle::echec::Classe::Disque, m))?;
    Ok(())
}

/// La version qui a passe sur les mois finis (`config/.version-du-calcul.json`) : le numero et la marque du code.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionDuCalcul {
    version: String,
}

// ------------------------------------------------------------------ outils

/// La compagnie est une copie de developpement (`MARQUEUR_COPIE`).
fn est_copie(racine: &Path) -> bool {
    racine.join(MARQUEUR_COPIE).is_file()
}

/// L'heure de Paris, `JJ/MM/AAAA HH:MM`.
fn quand_maintenant() -> String {
    let iso = socle::paris::iso_de_naif(socle::paris::maintenant_naif());
    format!("{}/{}/{} {}:{}", &iso[8..10], &iso[5..7], &iso[0..4], &iso[11..13], &iso[14..16])
}

/// `pas_avant` : l'age, en secondes, de la derniere ouverture, d'apres le `quand` de sa trace (`JJ/MM/AAAA HH:MM`, ce
/// que `quand_maintenant` ecrit). Un `quand` qui ne se lit pas se refuse.
fn age_de_la_derniere(precedente: &Trace) -> R<i64> {
    let c: Vec<char> = precedente.quand.chars().collect();
    let illisible = || socle::echec::Echec::from(format!("La trace de la dernière ouverture ne dit pas son heure : « {} ».", precedente.quand));
    if c.len() != 16 || c[2] != '/' || c[5] != '/' {
        return Err(illisible());
    }
    let tranche = |a: usize, b: usize| -> String { c[a..b].iter().collect() };
    let iso = format!("{}-{}-{}T{}", tranche(6, 10), tranche(3, 5), tranche(0, 2), tranche(11, 16));
    let avant = socle::paris::naif_de_iso(&iso).ok_or_else(illisible)?;
    Ok(socle::paris::maintenant_naif() - avant)
}

/// Le coffre dit-il que `service` est pose (ou dans l'environnement) ? `Err` : le coffre refuse de s'ouvrir.
fn coffre_dispo(ctx: &Contexte, service: &str) -> Result<bool, String> {
    crate::compagnie::coffre::Coffre::du_poste(&ctx.racine, "").disponible(service).map_err(|e| e.message)
}

/// Une seule phrase pour l'absence de reseau, pas une erreur par dossier et par organisme. Les erreurs
/// qui ne disent que « pas de reseau » sortent des incidents ; les autres (un mot de passe refuse, un coffre ferme)
/// restent, elles demandent un geste.
/// Elles ne se perdent pas pour autant : `injoignables` garde de quoi dire QUI n'a pas repondu.
fn trier_hors_ligne(r: &mut Ouverture) {
    let reseau: Vec<String> = r.incidents.iter().filter(|i| sauvegarde::hors_ligne(i)).cloned().collect();
    let drive = r.sauvegarde.as_ref().is_some_and(|s| s.hors_ligne);
    if !reseau.is_empty() || drive {
        r.hors_ligne = true;
        r.incidents.retain(|i| !sauvegarde::hors_ligne(i));
        for i in reseau.into_iter().chain(drive.then(|| "Sauvegarde dans le Drive : réseau injoignable.".to_string())) {
            if !r.injoignables.contains(&i) {
                r.injoignables.push(i);
            }
        }
    }
}

/// Termine le compte rendu : le tri, la duree, la trace.
fn clore(ctx: &Contexte, r: &mut Ouverture, debut: std::time::Instant) {
    finir_les_etapes(ctx);
    trier_hors_ligne(r);
    r.secondes = (socle::temps::duree(debut).as_secs_f64() * 10.0).round() / 10.0;
    noter_ouverture(ctx, r);
}

/// La seconde moitie de la veille : la trace du premier appel, mise a jour par la sauvegarde (qui est faite en dernier) sans
/// rien effacer de ce que le premier a ecrit. Le resultat est celui d'une seule ouverture : la sauvegarde, la duree totale,
/// « hors ligne » si l'un des deux l'a vu (et alors plus de ligne « retours », comme la trace d'une seule ouverture), les
/// incidents et les manques a la suite des premiers. Sans trace d'avant : la trace ordinaire de ce passage.
fn completer_la_trace(ctx: &Contexte, r: &mut Ouverture, debut: std::time::Instant, precedente: Option<Trace>) {
    finir_les_etapes(ctx);
    let Some(mut trace) = precedente else {
        return clore(ctx, r, debut);
    };
    trier_hors_ligne(r);
    let ma_duree = (socle::temps::duree(debut).as_secs_f64() * 10.0).round() / 10.0;
    r.secondes = ma_duree;
    trace.sauvegarde = r.sauvegarde.as_ref().map(Sauvegarde::resume);
    trace.sauvegarde_souci = souci_de_la_sauvegarde(ctx, r);
    trace.secondes = ((trace.secondes + ma_duree) * 10.0).round() / 10.0;
    trace.hors_ligne = trace.hors_ligne || r.hors_ligne;
    // La sauvegarde qui ne passe pas ne tait plus la ligne des retours : la releve, elle, est passee.
    trace.injoignables.extend(r.injoignables.iter().cloned());
    trace.incidents.extend(r.incidents.iter().cloned());
    trace.a_poser.extend(r.a_poser.iter().cloned());
    trace.manquants.extend(r.manquants.iter().filter(|m| !r.phrases_coffre.contains(m)).cloned());
    let _ = socle::ecriture::ecrire_json(&chemin_trace(ctx), &trace, 1);
}

// ------------------------------------------------------------------ l'ouverture

/// `ouverture` : ce qui se fait a l'ouverture du logiciel, et toutes les N minutes ensuite. Ne rend pas de `Result` : elle
/// n'echoue jamais, elle rend compte.
///
/// - `pas_avant` : ne rien refaire si la derniere ouverture a moins de N minutes. Trois fenetres ouvertes coup sur coup ne
///   relevent pas trois fois.
/// - `veille` : le passage periodique. Il saute la sauvegarde quand rien n'a bouge sur le disque -- une comparaison locale,
///   sans un octet de reseau.
/// - `apres_releve` : appele une fois les retours releves, AVANT la sauvegarde (qui passe en dernier :
///   elle peut prendre des minutes, et les retours n'attendent pas derriere elle). La trace de l'ouverture est ecrite a ce
///   moment-la, puis refaite a la fin. La route n'en a pas : un processus ne rappelle pas son appelant.
/// - `clients` : les clients reseau a utiliser a la place des vrais (la preuve).
///
/// LA COPIE DE DEVELOPPEMENT ne sauvegarde pas (elle ecrirait dans le Drive de la vraie compagnie), ne releve pas les
/// signatures d'elle-meme (la copie et l'original liraient la meme boite et renverraient chacun le contrat contresigne). Les
/// retours, eux, se relevent : ce sont ceux des depots de test qu'on y fait.
///
/// LA COMPAGNIE FICTIVE : l'horloge d'abord, les retours des depots joues, les signatures et les fiches qui repondent, le filet,
/// et rien d'autre (ni coffre, ni reseau, ni sauvegarde).
pub fn ouverture(ctx: &Contexte, opt: &Options, mut clients: Clients<'_>, mut apres_releve: Option<&mut dyn FnMut()>) -> Ouverture {
    let debut = std::time::Instant::now();
    let mut r = Ouverture { quand: quand_maintenant(), ..Ouverture::default() };
    let mut sauvegarder = opt.sauvegarder;
    let mut retours = opt.retours;
    let mut signatures = opt.signatures;
    // La trace d'avant ce passage : `pas_avant` la lit, la seconde moitie de la
    // veille la complete.
    let precedente = match derniere_ouverture(&ctx.racine) {
        Ok(p) => p,
        Err(e) => {
            r.incidents.push(format!("Trace de l'ouverture : {}", e.message));
            None
        }
    };

    if opt.sauvegarde_seule {
        return sauvegarder_seulement(ctx, opt, clients, r, debut, precedente);
    }

    if let (true, Some(p)) = (opt.pas_avant > 0, &precedente) {
        match age_de_la_derniere(p) {
            Ok(age) if age < opt.pas_avant * 60 => {
                r.incidents.push(format!("Ouverture deja faite il y a {} minute(s) : rien a refaire.", age.div_euclid(60)));
                r.retenue = true;
                return r;
            }
            Ok(_) => {}
            Err(e) => r.incidents.push(e.message),
        }
    }

    let copie = est_copie(&ctx.racine);
    if copie {
        sauvegarder = false;
        signatures = false;
    }
    // UNE FICTIVE AJOUTEE : rien n'en part (`bridage::branchement`), donc rien n'y revient -- ni retour de DSN ou de
    // DPAE, ni contrat signe. La veille ne les releve pas, et ne demande pas les mots de passe qui serviraient a les relever :
    // le bandeau ne presente pas comme une panne ce qui est voulu. Le coffre, lui, s'y pose comme ailleurs (Configuration).
    // Un marqueur qui ne se lit pas ne dit pas si la compagnie envoie : la veille s'arrete la, et le dit.
    let (branchement, fictive) = match crate::compagnie::bridage::branchement(&ctx.racine).and_then(|b| Ok((b, crate::compagnie::installation::est_fictive(&ctx.racine)?))) {
        Ok(x) => x,
        Err(e) => {
            r.incidents.push(e);
            return r;
        }
    };
    if branchement.refus().is_some() {
        retours = false;
        signatures = false;
    }

    // UNE VERSION QUI CALCULE AUTREMENT passe une fois sur les mois finis, avant le present.
    if let Err(e) = passer_la_version(ctx) {
        r.incidents.push(format!("Mois finis : {}", e.message));
    }

    // LA COMPAGNIE FICTIVE : les retours des depots joues et les signatures demandees arrivent d'ici, sans coffre ni reseau --
    // et rien ne manque au coffre, puisqu'il n'y a rien a y poser.
    if branchement.joue() {
        // L'HORLOGE D'ABORD : ce qui etait du avant aujourd'hui se joue, a sa date.
        dire_l_etape(ctx, "Compagnie fictive : l'administrateur fictif fait ce qui tombait…");
        let jouee = crate::horloge::rattraper(&ctx.racine, None, true).and_then(|fait| if fait.is_empty() { crate::horloge::dernier_rattrapage(&ctx.racine, None) } else { Ok(fait) });
        match jouee {
            Ok(fait) => r.horloge = fait,
            Err(e) => r.incidents.push(format!("Horloge de la compagnie fictive : {}", e.message)),
        }
        if retours {
            dire_l_etape(ctx, "Compagnie fictive : retours des dépôts joués…");
            let mut releve = Releve::default();
            match crate::gestes::releve::rattacher_simulation(ctx) {
                Ok((n, a_faire)) => (releve.rattaches, releve.a_faire) = (n, a_faire),
                // L'ouverture ne doit pas tomber.
                Err(e) => r.incidents.push(format!("Retours : {}", e.message)),
            }
            r.releve = Some(releve);
        }
        dire_l_etape(ctx, "Compagnie fictive : signatures et fiches des salariés…");
        // Le resultat de la relève jouee n'est pas gardé.
        if let Err(e) = releve_signatures::relever_signatures_fictive(ctx) {
            r.incidents.push(format!("Signatures : {}", e.message));
        }
        match crate::routes::fiches::relever_fiches(ctx, reborrow_courriel(&mut clients.courriel), crate::horloge::QUI_HORLOGE) {
            // Les incidents de la relève des fiches jouée ne sont pas repris.
            Ok(f) => r.fiches = Some(f),
            Err(e) => r.incidents.push(format!("Fiches salariés : {}", e.message)),
        }
        retours = false;
        signatures = false;
        sauvegarder = false;
    }

    if retours {
        relever_les_retours(ctx, opt, &mut clients, &mut r);
    }

    if signatures {
        dire_l_etape(ctx, "Boîte courriel : contrats revenus signés…");
        // Les reponses « Je signe ». La relève ne leve pas pour un compte absent ou une permission manquante : elle le dit.
        match releve_signatures::relever_signatures(ctx, reborrow_courriel(&mut clients.courriel), None, "veille") {
            Ok(s) => {
                r.incidents.extend(s.incidents.iter().cloned());
                r.signatures = Some(s);
            }
            Err(e) => r.incidents.push(format!("Signatures : {}", e.message)),
        }
        // Les fiches demandees aux salaries : la meme boite, le meme passage, la meme regle.
        dire_l_etape(ctx, "Boîte courriel : fiches des salariés…");
        match crate::routes::fiches::relever_fiches(ctx, reborrow_courriel(&mut clients.courriel), "veille") {
            Ok(f) => {
                r.incidents.extend(f.incidents.iter().cloned());
                r.fiches = Some(f);
            }
            Err(e) => r.incidents.push(format!("Fiches salariés : {}", e.message)),
        }
    }

    // LES ECARTS DE CALCUL DES PAIES IMPORTEES qui ne sont pas encore comparees (`import::ecarts::noter`) : une
    // compagnie completee depuis l'import compare celles que le calcul refusait. Ils partent avec les autres incidents.
    if let Err(e) = crate::import::ecarts::noter(ctx) {
        r.incidents.push(format!("Écarts de calcul de l'import : {}", e.message));
    }

    // LE TAUX COLLECTIF DE L'ARRETE pour un risque « TC », au 1er janvier de l'annee en cours et, des que son bareme est
    // livre, de la suivante (`routes::atmp::inscrire_l_arrete`) : il s'inscrit dans la case, qui le montre.
    for an in [ctx.aujourdhui.an, ctx.aujourdhui.an + 1] {
        if let Err(e) = crate::routes::atmp::inscrire_l_arrete(ctx, an) {
            r.incidents.push(format!("Taux AT/MP {} : {}", an, e.message));
        }
    }

    // LE DIAGNOSTIC A DISTANCE : ce que le journal des incidents a note part chez l'editeur -- pas depuis la
    // copie de developpement (ses refus sont ceux qu'on provoque pour essayer), mais depuis une compagnie fictive. Et les
    // reponses de l'editeur aux signalements de ce poste. Pas pendant les tests (`PUPPETPAIE_INCIDENTS` pose : le journal est
    // celui d'un bac a sable). C'est fait avant de rendre la main.
    let bac_a_sable = matches!(std::env::var("PUPPETPAIE_INCIDENTS"), Ok(v) if !v.is_empty());
    if opt.filet && opt.incidents_editeur && !bac_a_sable && (!copie || fictive) {
        dire_l_etape(ctx, "Serveur de l'éditeur : incidents, messages et réponses…");
        // Ce qui echoue ici ne fait pas refuser l'ouverture : la veille suivante reessaie, et l'ouverture le dit.
        if let Some(e) = crate::compagnie::incidents::envoyer(&ctx.racine).erreur {
            r.incidents.push(format!("Incidents chez l'éditeur : {}", e));
        }
        // LE NOMBRE DE PAIES DE L'ANNEE PASSEE, a partir du 16 janvier, une fois : `nombre_de_paies` decide
        // seul (ni fictive, ni copie de developpement, ni logiciel bride, ni envoi coupe) et retient ce qui est parti.
        if let Some(e) = crate::compagnie::nombre_de_paies::envoyer_si_du(ctx).erreur {
            r.incidents.push(format!("Nombre de paies chez l'éditeur : {}", e));
        }
        // Les messages « Un probleme, une idee ? » que l'envoi en fond n'a pas fait partir (serveur injoignable...).
        if let Some(e) = crate::compagnie::problemes::partir(&ctx.racine).erreur {
            r.incidents.push(format!("Messages à l'éditeur : {}", e));
        }
        if let Err(e) = crate::compagnie::problemes::relever_reponses(&ctx.racine) {
            r.incidents.push(format!("Réponses de l'éditeur : {}", e.message));
        }
    }

    if let Some(rappel) = apres_releve.as_mut() {
        clore(ctx, &mut r, debut);
        rappel();
    }

    if sauvegarder {
        sauvegarder_le_dossier(ctx, opt, &mut clients, &mut r);
    }

    clore(ctx, &mut r, debut);
    r
}

/// `sauvegarde_seule` : la seconde moitie de la veille. La copie de developpement et la compagnie fictive ne sauvegardent pas
/// (rien a faire, la trace n'est pas touchee) ; sinon la sauvegarde, en dernier, et la trace du premier appel completee.
fn sauvegarder_seulement(ctx: &Contexte, opt: &Options, mut clients: Clients<'_>, mut r: Ouverture, debut: std::time::Instant, precedente: Option<Trace>) -> Ouverture {
    let joue = match crate::compagnie::bridage::branchement(&ctx.racine) {
        Ok(b) => b.joue(),
        Err(e) => {
            r.incidents.push(e);
            return r;
        }
    };
    if !opt.sauvegarder || est_copie(&ctx.racine) || joue {
        return r;
    }
    sauvegarder_le_dossier(ctx, opt, &mut clients, &mut r);
    completer_la_trace(ctx, &mut r, debut, precedente);
    r
}

/// Reprend la boite donnee sans la perdre : les signatures puis les fiches la relisent l'une apres l'autre.
fn reborrow_courriel<'b>(c: &'b mut Option<&mut dyn Courriel>) -> Option<&'b mut dyn Courriel> {
    match c {
        Some(x) => Some(&mut **x),
        None => None,
    }
}

/// Les retours DSN (net-entreprises) puis DPAE (Urssaf) : un secret absent est un manque, une panne est un incident.
fn relever_les_retours(ctx: &Contexte, opt: &Options, clients: &mut Clients<'_>, r: &mut Ouverture) {
    // Le canal DSN.
    let client_dsn = clients.dsn.take();
    let dispo = match &client_dsn {
        Some(_) => Some(true),
        None => match coffre_dispo(ctx, "net-entreprises") {
            Ok(b) => Some(b),
            Err(m) => {
                // Une erreur du coffre : l'ouverture ne tombe pas.
                r.incidents.push(format!("Retours : {}", m));
                None
            }
        },
    };
    match dispo {
        Some(false) => {
            r.a_poser.push("net-entreprises".to_string());
            r.manque_coffre(
                "Mot de passe net-entreprises absent du coffre : les retours ne peuvent pas etre releves seuls. `route coffre-poser, service net-entreprises`.",
            );
        }
        Some(true) => {
            dire_l_etape(ctx, "net-entreprises : retours des DSN…");
            let fin = socle::paris::maintenant_naif();
            match crate::gestes::releve::relever_retours(ctx, fin - opt.jours * 86_400, fin, false, "production", client_dsn, None) {
                Ok(rel) => {
                    r.incidents.extend(rel.incidents.iter().cloned());
                    r.manquants.extend(rel.a_faire.iter().cloned());
                    r.releve = Some(rel);
                }
                Err(e) => r.incidents.push(format!("Retours : {}", e.message)),
            }
        }
        None => {}
    }

    // LE CANAL DPAE AUSSI : une DPAE refusee en douze secondes ne reste pas « pas encore de retour »
    // jusqu'a ce qu'on tape la commande a la main.
    let client_dpae = clients.dpae.take();
    let dispo = match &client_dpae {
        Some(_) => Some(true),
        None => match coffre_dispo(ctx, "urssaf") {
            Ok(b) => Some(b),
            Err(m) => {
                r.incidents.push(format!("Retours DPAE : {}", m));
                None
            }
        },
    };
    match dispo {
        Some(false) => {
            r.a_poser.push("urssaf".to_string());
            r.manque_coffre("Mot de passe Urssaf (DPAE) absent du coffre : les retours de DPAE ne peuvent pas etre releves seuls. `route coffre-poser, service urssaf`.");
        }
        Some(true) => {
            dire_l_etape(ctx, "Urssaf : retours des DPAE…");
            let fin = socle::paris::maintenant_naif();
            let debut = fin - opt.jours * 86_400;
            // Un client donne passe par la meme releve que le vrai (canal `dpae`) ; sans client, `relever_retours_dpae`
            // cherche le mot de passe de consultation et construit le sien.
            let rel = match client_dpae {
                Some(c) => crate::gestes::releve::relever_retours(ctx, debut, fin, false, "production", Some(c), Some(envois::etats::DPAE)),
                None => crate::gestes::releve::relever_retours_dpae(ctx, debut, fin, false, None),
            };
            match rel {
                Ok(rel) => {
                    r.incidents.extend(rel.incidents.iter().cloned());
                    r.releve_dpae = Some(rel);
                }
                Err(e) => r.incidents.push(format!("Retours DPAE : {}", e.message)),
            }
        }
        None => {}
    }
}

/// La sauvegarde, en dernier : un compte Google non lie est un manque ; rien de neuf sur le disque, en veille, est un
/// silence (aucun appel) ; une panne est un incident.
fn sauvegarder_le_dossier(ctx: &Contexte, opt: &Options, clients: &mut Clients<'_>, r: &mut Ouverture) {
    let pret = clients.drive.is_some() || crate::routes::coffre::autorise(&ctx.racine);
    if !pret {
        r.a_poser.push("google".to_string());
        r.manque_coffre(
            "Compte Google non lie : la sauvegarde ne peut pas partir. Lier le Drive : Configuration > Mots de passe, ou la route google-autoriser.",
        );
        return;
    }
    if opt.veille {
        match sauvegarde::rien_n_a_bouge(&ctx.racine, &opt.racine_drive) {
            // rien de neuf sur le disque : on n'appelle pas
            Ok(true) => return,
            Ok(false) => {}
            Err(e) => {
                r.incidents.push(format!("Sauvegarde : {}", e));
                return;
            }
        }
    }
    dire_l_etape(ctx, "Google Drive : sauvegarde du dossier…");
    let drive: Option<&mut dyn Drive> = match clients.drive.as_mut() {
        Some(d) => Some(&mut **d),
        None => None,
    };
    match crate::gestes::sauvegarde::sauvegarder(&ctx.racine, &opt.racine_drive, false, drive) {
        // CE QUI EST REPORTE N'EST PAS UN INCIDENT : un dossier que Google a refuse une fois (un 500,
        // deja retente par le client) repart au passage suivant, en silence. Ce qui persiste se dit dans la ligne de la
        // sauvegarde (`sauvegarde::souci`, la trace `sauvegarde_souci`), pas dans les incidents du releve.
        Ok(s) => r.sauvegarde = Some(s),
        Err(e) => r.incidents.push(format!("Sauvegarde : {}", e)),
    }
}
