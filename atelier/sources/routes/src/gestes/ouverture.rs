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
//! `crate::fiches::relever_fiches`, `horloge::{rattraper, dernier_rattrapage}`,
//! `incidents::envoyer` et `problemes::relever_reponses` (le canal de l'editeur), `sauvegarde::{sauvegarder,
//! rien_n_a_bouge, hors_ligne}`, `crate::coffre::autorise`, `coffre::Coffre`.
//!
//! CHAQUE CLIENT RESEAU PEUT ETRE REMPLACE (`Clients`) : le compte net-entreprises, le compte Urssaf (DPAE), la boite
//! courriel (signatures et fiches), le Drive. Un client donne est utilise a la place du vrai, SANS regarder le coffre.
//! Le serveur des incidents est celui de la configuration (`incidents::configuration`), qu'un serveur local peut tenir.
//!
//! LA MISE A JOUR N'EST PAS ICI : le logiciel la cherche pour l'installation (`maj::chercher_en_fond`) et l'indicateur la
//! dit (`maj::Recherche::etat`).

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use droit::contexte::Contexte;
use envois::courriel::Courriel;
use envois::drive::Drive;
use envois::etats;
use socle::echec::R;
use socle::paris::{ecrit, Moment};
use envois::releve_retours::{Client, Releve};
use envois::releve_signatures::ReleveDite;
use crate::gestes::releve_signatures::{self, ReleveSignatures};
use crate::catalogue::{Acte, Affiche, Forme, Fiche, OPERATION};
use crate::declaration::{charge, dit, ParSaForme, ValeurDeLaCharge};
use crate::catalogue::Route;
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use crate::fiches::{FicheRecue, ReleveFiches};
use compagnie::sauvegarde::{self, Sauvegarde};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Auto(Place::Historique, Affiche::new("Relever maintenant"));

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Ouverture,
        geste: "Ouvrir la compagnie : sauvegarder, relever retours, signatures et fiches.",
        charge: &ChargeOuverture::CHARGE,
        exemple: r#"{}"#,
        rend: "ce que chaque passage a fait (sauvegarde, retours, signatures, fiches, filet, spectacles_ranges : ceux que 24 mois sans engagement ont rangés dans les archives), et les incidents en phrases ; ne refuse jamais",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// `compagnies.MARQUEUR`.
use socle::fichiers::{NomDeFichier, MARQUEUR_COPIE};
use std::path::PathBuf;
use crate::catalogue::{Ecran, Place};
use socle::fichiers::{ETAPE_DE_L_OUVERTURE, TRACE_DE_L_OUVERTURE};

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
    /// Jusqu'ou en arriere on cherche les retours au plus loin : des jours de 24 heures avant maintenant
    /// (`Moment::moins_duree`). Le releve repart de ce que le releve complet precedent a lu, s'il est plus pres
    /// (`envois::releve_retours::PlageDuReleve::Complete`).
    pub jours: Duration,
    /// Le dossier racine dans le Drive.
    pub racine_drive: String,
    /// Ne rien refaire si la derniere ouverture est plus recente (zero : toujours).
    pub pas_avant: Duration,
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

/// Les trente jours que la commande des depots liste par defaut.
pub const TRENTE_JOURS: Duration = socle::temps::jours_de_24_heures(30);

/// Les quatre-vingt-dix jours de retours qu'une ouverture releve au plus loin, des la premiere (Leandre, 19/11/2026 :
/// « il faut toujours prendre 90 jours ») : « Les retours disponibles au téléchargement ont une période de rétention de
/// 3 mois par le SI DSN » (guide de l'API DSN, § 8.2, `atelier/reference/sources/gip-mds-api-dsn/LISEZ-MOI.md`). Le
/// releve complet ne redemande ensuite que la suite (`envois::releve_retours::ReleveComplet`).
pub const QUATRE_VINGT_DIX_JOURS: Duration = envois::releve_retours::HORIZON_DU_RELEVE_COMPLET;

impl Options {
    /// L'ouverture entiere : tout se releve, se depose chez l'editeur et se sauvegarde, sur quatre-vingt-dix jours de retours au plus.
    pub fn entiere() -> Options {
        Options {
            sauvegarder: true,
            retours: true,
            signatures: true,
            filet: true,
            jours: QUATRE_VINGT_DIX_JOURS,
            racine_drive: sauvegarde::RACINE_DRIVE.to_string(),
            pas_avant: Duration::ZERO,
            veille: false,
            incidents_editeur: true,
            sauvegarde_seule: false,
        }
    }
}

/// Une duree mesuree, dans une charge : un nombre, dans l'unite que son attribut serde dit (`socle::temps::ecrit`).
impl ValeurDeLaCharge<ParSaForme> for Duration {
    const FORME: Forme = Forme::Nombre;
}

charge! {
    /// LA CHARGE DE LA ROUTE `ouverture` : chaque option dite, ou absente. Les interrupteurs disent `false` pour ne pas
    /// faire (sauvegarder, retours, signatures, filet, incidents_editeur) et `true` pour faire autrement (veille,
    /// sauvegarde_seule) ; `jours` (en jours), `pas_avant` (en minutes) et `racine_drive` remplacent ce que fait un passage
    /// ordinaire (`Options::entiere`).
    #[derive(Debug, Default, Serialize)]
    pub struct ChargeOuverture {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub sauvegarder: Option<bool> = dit("Sauvegarder", "faux : ne pas sauvegarder dans le Drive").avancee(),
        #[serde(skip_serializing_if = "Option::is_none")]
        pub retours: Option<bool> = dit("Relever les retours", "faux : ne pas relever les retours des organismes").avancee(),
        #[serde(skip_serializing_if = "Option::is_none")]
        pub signatures: Option<bool> = dit("Relever les signatures", "faux : ne pas relever les signatures").avancee(),
        #[serde(skip_serializing_if = "Option::is_none")]
        pub filet: Option<bool> = dit("Envoyer à l'éditeur", "faux : rien ne part chez l'éditeur (incidents, réponses aux signalements)").avancee(),
        #[serde(skip_serializing_if = "Option::is_none")]
        pub veille: Option<bool> = dit("Un tour de veille", "vrai : un tour de veille").avancee(),
        #[serde(skip_serializing_if = "Option::is_none")]
        pub incidents_editeur: Option<bool> = dit("Incidents à l'éditeur", "faux : ne pas envoyer chez l'éditeur les incidents notés, ni relever ses réponses").avancee(),
        #[serde(skip_serializing_if = "Option::is_none")]
        pub sauvegarde_seule: Option<bool> = dit("Sauvegarde seule", "vrai : seulement la sauvegarde, après un premier passage qui a relevé sans elle").avancee(),
        #[serde(default, skip_serializing_if = "Option::is_none", with = "socle::temps::ecrit::jours_opt")]
        pub jours: Option<Duration> = dit("Jours de retours", "jusqu'où chercher les retours, en jours avant maintenant ; 90 sinon").avancee(),
        #[serde(default, skip_serializing_if = "Option::is_none", with = "socle::temps::ecrit::minutes_opt")]
        pub pas_avant: Option<Duration> = dit("Pas avant", "en minutes : rien ne se refait si la dernière ouverture est plus récente ; zéro sinon").avancee(),
        #[serde(skip_serializing_if = "Option::is_none")]
        pub racine_drive: Option<String> = dit("Dossier du Drive", "le dossier racine de la sauvegarde dans le Drive").avancee(),
    }
}

impl Options {
    /// Les options d'une charge de route : un passage ordinaire, et ce que la charge dit autrement.
    pub(crate) fn de_la_charge(c: ChargeOuverture) -> Options {
        let mut o = Options::entiere();
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
pub struct Ouverture {
    /// L'heure du passage, a la minute.
    pub quand: Moment,
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
    /// Les spectacles que le temps a ranges dans les archives (`fiches::ranger_les_spectacles_endormis`), en une phrase.
    pub spectacles_ranges: Vec<String>,
    /// La duree de l'ouverture.
    pub duree: Duration,
    /// Le reseau manquait : dit une fois, pas par erreur.
    pub hors_ligne: bool,
    /// Ce qui n'a pas repondu, une phrase par service (l'erreur telle qu'elle est venue) : le reseau manquait pour CELA,
    /// pas forcement pour tout -- la releve a pu passer quand le Drive ne repondait pas.
    pub injoignables: Vec<String>,
    /// `pas_avant` a retenu ce passage (la derniere ouverture est trop recente) : rien n'a ete fait, la trace n'est pas
    /// reecrite.
    pub retenue: bool,
    /// Les etapes finies de ce passage, dans l'ordre, et la duree de chacune (`dire_l_etape`).
    pub etapes: Vec<EtapeMesuree>,
    /// L'etape en cours : ce qu'elle dit, et quand elle a commence.
    en_cours: Option<(String, std::time::Instant)>,
}

impl Ouverture {
    /// Le compte rendu d'un passage qui commence maintenant, rien encore de fait.
    fn commence() -> Ouverture {
        Ouverture {
            quand: socle::temps::secondes().a_la_minute(),
            sauvegarde: None,
            releve: None,
            releve_dpae: None,
            signatures: None,
            fiches: None,
            incidents: Vec::new(),
            manquants: Vec::new(),
            a_poser: Vec::new(),
            phrases_coffre: Vec::new(),
            horloge: Vec::new(),
            spectacles_ranges: Vec::new(),
            duree: Duration::ZERO,
            hors_ligne: false,
            injoignables: Vec::new(),
            retenue: false,
            etapes: Vec::new(),
            en_cours: None,
        }
    }

    /// Finit l'etape en cours, s'il y en a une : sa duree s'ajoute aux etapes du passage.
    fn finir_l_etape(&mut self) {
        if let Some((etape, debut)) = self.en_cours.take() {
            self.etapes.push(EtapeMesuree { etape, secondes: socle::temps::duree(debut) });
        }
    }

    /// `Ouverture.tout_va_bien`.
    pub(crate) fn tout_va_bien(&self) -> bool {
        self.incidents.is_empty() && self.manquants.is_empty()
    }

    /// Ce qu'une releve a a dire : ses incidents avec les incidents, ce qu'elle laisse a faire avec ce qui manque.
    fn dire(&mut self, dit: envois::suites::ADire) {
        self.incidents.extend(dit.incidents);
        self.manquants.extend(dit.a_faire);
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
    pub fn dite(self) -> R<OuvertureDite> {
        let retours_resume = self.releve.as_ref().map(Ouverture::resume_retours);
        let phrase = match &retours_resume {
            Some(r) => socle::phrase!("Ouverture du {} : {}", ecrit::francaise::texte(self.quand), r),
            None => socle::phrase!("Ouverture du {}.", ecrit::francaise::texte(self.quand)),
        };
        let phrases: Vec<String> = self.horloge.iter().chain(&self.spectacles_ranges).chain(&self.manquants).chain(&self.incidents).cloned().collect();
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
        let retours = self
            .releve
            .as_ref()
            .map(|r| -> R<RetoursDits> {
                Ok(RetoursDits {
                    resume: Ouverture::resume_retours(r),
                    trouves: r.trouves.len(),
                    nouveaux: r.nouveaux.len(),
                    archives: r.archives.iter().map(|a| a.chemin.clone()).collect(),
                    archives_detail: r
                        .archives
                        .iter()
                        .map(|a| -> R<RetourArchiveDit> {
                            Ok(RetourArchiveDit {
                                chemin: a.chemin.clone(),
                                fichier: a.chemin.file_name().map(|n| NomDeFichier::lire(&n.to_string_lossy())).transpose()?,
                                nature_retour: a.retour.nature_retour,
                                libelle: a.retour.nature_retour.libelle_dit(),
                            })
                        })
                        .collect::<R<_>>()?,
                    rattaches: r.rattaches,
                })
            })
            .transpose()?;
        let retours_dpae = self.releve_dpae.as_ref().map(|r| RetoursDpaeDits {
            resume: Ouverture::resume_retours(r),
            trouves: r.trouves.len(),
            nouveaux: r.nouveaux.len(),
            archives: r.archives.iter().map(|a| a.chemin.clone()).collect(),
            rattaches: r.rattaches,
        });
        let fiches = self.fiches.as_ref().map(|f| FichesDites { resume: f.resume(), attendues: f.attendues, recues: f.recues.clone(), incidents: f.incidents.clone() });
        let tout_va_bien = self.tout_va_bien();
        Ok(OuvertureDite {
            phrase,
            quand: self.quand,
            secondes: self.duree,
            sauvegarde,
            retours,
            signatures: self.signatures.map(ReleveSignatures::dite),
            incidents: self.incidents,
            manquants: self.manquants,
            retours_dpae,
            fiches,
            a_poser: self.a_poser,
            horloge: self.horloge,
            spectacles_ranges: self.spectacles_ranges,
            hors_ligne: self.hors_ligne,
            injoignables: self.injoignables,
            tout_va_bien,
            retenue: self.retenue,
            phrases,
        })
    }
}

/// CE QUE REND LA ROUTE `ouverture` (`Ouverture::dite`) : la phrase du passage, ce que chaque releve a fait, et ses lignes
/// (`phrases`).
#[derive(Debug, Serialize)]
pub struct OuvertureDite {
    pub phrase: Phrase,
    #[serde(with = "socle::paris::ecrit::francaise")]
    pub quand: Moment,
    #[serde(with = "socle::temps::ecrit::secondes_au_dixieme")]
    pub secondes: Duration,
    pub sauvegarde: Option<SauvegardeDite>,
    pub retours: Option<RetoursDits>,
    pub signatures: Option<ReleveDite>,
    pub incidents: Vec<String>,
    pub manquants: Vec<String>,
    pub retours_dpae: Option<RetoursDpaeDits>,
    pub fiches: Option<FichesDites>,
    pub a_poser: Vec<String>,
    pub horloge: Vec<String>,
    /// Les spectacles que le temps a ranges dans les archives, en une phrase ; vide sinon.
    pub spectacles_ranges: Vec<String>,
    pub hors_ligne: bool,
    pub injoignables: Vec<String>,
    pub tout_va_bien: bool,
    /// `pas_avant` a retenu ce passage : rien n'a ete fait.
    pub retenue: bool,
    pub phrases: Vec<String>,
}

impl Dite for OuvertureDite {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// La sauvegarde d'un passage : sa ligne, ce qui est parti, ce qui reste en souci (`sauvegarde::souci`).
#[derive(Debug, Serialize)]
pub struct SauvegardeDite {
    pub resume: String,
    pub deposes: Vec<socle::fichiers::CheminRelatif>,
    pub identiques: usize,
    pub octets: socle::grandeurs::Octets,
    pub deposes_detail: Vec<FichierDepose>,
    pub exclus: usize,
    pub hors_ligne: bool,
    pub souci: Option<String>,
}

/// Un fichier parti au Drive : son chemin dans le dossier de la compagnie, neuf ou modifie, son poids.
#[derive(Debug, Serialize)]
pub struct FichierDepose {
    pub relatif: socle::fichiers::CheminRelatif,
    pub etat: &'static str,
    pub octets: socle::grandeurs::Octets,
}

/// Les retours DSN d'un passage : sa ligne, combien, ce qui s'est archive et rattache.
#[derive(Debug, Serialize)]
pub struct RetoursDits {
    pub resume: String,
    pub trouves: usize,
    pub nouveaux: usize,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub archives: Vec<PathBuf>,
    pub archives_detail: Vec<RetourArchiveDit>,
    pub rattaches: usize,
}

/// Un retour archive : ou, sa nature et son libelle.
#[derive(Debug, Serialize)]
pub struct RetourArchiveDit {
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub chemin: PathBuf,
    pub fichier: Option<NomDeFichier>,
    #[serde(rename = "nature")]
    pub nature_retour: envois::retours::NatureDuRetour,
    pub libelle: String,
}

/// Les retours DPAE d'un passage.
#[derive(Debug, Serialize)]
pub struct RetoursDpaeDits {
    pub resume: String,
    pub trouves: usize,
    pub nouveaux: usize,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub archives: Vec<PathBuf>,
    pub rattaches: usize,
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
/// ne refuse que sa charge mal formee et un disque qui n'ecrit pas sa trace : une panne est une phrase de `incidents`.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::Ouverture => {
            let dite: OuvertureDite = ouverture(porte.ctx, &Options::de_la_charge(charge.lire()?), Clients::default(), None)?.dite()?;
            en_json(dite)
        }
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

// ------------------------------------------------------------------ l'etape en cours

/// L'ETAPE EN COURS D'UN PASSAGE : qui on appelle, puis qui d'autre, la sauvegarde -- un message dans la ligne qui
/// explique ce que fait la machine. L'ouverture tourne dans un processus a part (la veille
/// appelle la route `ouverture`) : elle ecrit son etape dans `config/.ouverture-etape.json` en y entrant, l'efface en
/// finissant ; la veille de la fenetre la lit (`Veille::etape`) tant qu'un passage tourne. Le fichier ne monte pas au Drive
/// (`sauvegarde::ecarte`). Une ecriture qui echoue fait echouer le passage : le disque refuse, la suite le
/// refuserait aussi.
fn chemin_etape(racine: &Path) -> std::path::PathBuf {
    ETAPE_DE_L_OUVERTURE.dans(racine)
}

/// L'etape d'un passage en cours (`config/.ouverture-etape.json`) : ce qu'il fait, depuis quand.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Etape {
    etape: String,
    #[serde(with = "socle::paris::ecrit::francaise")]
    depuis: Moment,
}

/// Entre dans l'etape `etape` : l'etape en cours finit (`Ouverture::finir_l_etape`), celle-ci s'ecrit et se mesure
/// jusqu'a la suivante, ou jusqu'a ce que le passage la finisse. Rend le moment ou elle commence.
fn dire_l_etape(ctx: &Contexte, r: &mut Ouverture, etape: &str) -> R<Moment> {
    r.finir_l_etape();
    let depuis = socle::temps::secondes().a_la_minute();
    redire_l_etape(&ctx.racine, etape, depuis)?;
    r.en_cours = Some((etape.to_string(), std::time::Instant::now()));
    Ok(depuis)
}

/// L'etape en cours, commencee a `depuis`, dit ce qu'elle fait maintenant (`etape`) : le transfert du Drive qui avance.
fn redire_l_etape(racine: &Path, etape: &str, depuis: Moment) -> R<()> {
    socle::ecriture::ecrire_json(&chemin_etape(racine), &Etape { etape: etape.to_string(), depuis }, 1)
}

fn finir_les_etapes(ctx: &Contexte, r: &mut Ouverture) -> R<()> {
    r.finir_l_etape();
    socle::fichiers::effacer(&chemin_etape(&ctx.racine))
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
    TRACE_DE_L_OUVERTURE.dans(&ctx.racine)
}

/// LA TRACE DE LA DERNIERE OUVERTURE (`config/.ouverture.json`), que le bandeau de la fenetre lit : quand, combien de
/// temps, la ligne de chaque releve qui a eu lieu, ce qui n'a pas repondu, ce qui manque et ce qui est a poser. Aucun
/// secret n'y figure, par construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    #[serde(with = "socle::paris::ecrit::francaise")]
    pub quand: Moment,
    #[serde(with = "socle::temps::ecrit::secondes_au_dixieme")]
    pub secondes: Duration,
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
    /// Chaque etape du passage (`dire_l_etape`), dans l'ordre, et ce qu'elle a dure : ou le temps est passe se lit
    /// ici. Un passage qui n'est entre dans aucune etape n'en ecrit pas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub etapes: Vec<EtapeMesuree>,
}

/// UNE ETAPE D'UN PASSAGE ET SA DUREE, telle que la trace la note : ce que l'etape a dit (`dire_l_etape`), et le temps
/// passe de son entree a sa fin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EtapeMesuree {
    pub etape: String,
    #[serde(with = "socle::temps::ecrit::secondes_au_dixieme")]
    pub secondes: Duration,
}

/// Un contrat revenu signe, tel que la trace le note.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignatureNotee {
    pub nom: String,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub quand: Moment,
    pub renvoye: bool,
    pub phrase: String,
}

/// `derniere_ouverture` : la trace, ou `None` quand aucune ouverture ne l'a encore ecrite. Une trace qui ne se lit pas se
/// refuse.
pub fn derniere_ouverture(racine: &std::path::Path) -> R<Option<Trace>> {
    let p = TRACE_DE_L_OUVERTURE.dans(racine);
    if !socle::fichiers::existe_lu(&p)? {
        return Ok(None);
    }
    Ok(Some(socle::forme::lire_fichier(&p, "La trace de la dernière ouverture ne se lit pas.")?))
}

/// La trace de la derniere ouverture. Une ecriture qui echoue fait echouer le passage (`ecrire_json` cree le dossier).
fn noter_ouverture(ctx: &Contexte, r: &mut Ouverture) -> R<()> {
    let p = chemin_trace(ctx);
    // LES RETOURS DE LA COMPAGNIE, PAS CEUX DU BAC A SABLE : les retours des depots de test ne se comptent que
    // dans la copie de developpement.
    let tests: Vec<envois::retours::IdDeFlux> = if est_copie(&ctx.racine) {
        Vec::new()
    } else {
        // Un journal illisible n'empeche pas la trace (l'ouverture n'echoue jamais) : il est un incident.
        match envois::journal::lu(&ctx.racine) {
            Ok(j) => etats::flux_de_test(&j.lignes),
            Err(e) => {
                r.incidents.push(format!("Journal : {}", nomme(&ctx.racine, &e)));
                Vec::new()
            }
        }
    };
    let hors_tests = |liste: &[envois::retours::Annonce]| -> usize { liste.iter().filter(|x| !tests.iter().any(|t| x.flux.as_ref() == Some(t))).count() };
    let (trouves, nouveaux) = match &r.releve {
        Some(rel) => (hors_tests(&rel.trouves), hors_tests(&rel.nouveaux)),
        None => (0, 0),
    };
    // La ligne des retours dit ce que la releve a fait, si ELLE a joint net-entreprises -- le Drive qui ne repond pas ne
    // la tait pas ; une releve sans reseau, elle, n'a rien vu, et « aucun retour » serait faux.
    let releve_passee = r.releve.as_ref().is_some_and(|rel| !rel.plateforme_non_lue.iter().any(|i| sauvegarde::hors_ligne(i)));
    let retours = releve_passee.then(|| {
        if trouves > 0 {
            format!("{} retour(s), dont {} nouveau(x).", trouves, nouveaux)
        } else {
            "Aucun retour sur la période.".to_string()
        }
    });
    let sauvegarde_souci = souci_de_la_sauvegarde(ctx, r);
    let trace = Trace {
        quand: r.quand,
        secondes: r.duree,
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
            .map(|x| SignatureNotee { nom: x.nom.clone(), quand: x.quand, renvoye: x.renvoye, phrase: format!("contrat signé par {}", x.nom) })
            .collect(),
        incidents: r.incidents.clone(),
        // Les phrases du coffre ne repartent pas : `a_poser` les dit.
        manquants: r.manquants.iter().filter(|m| !r.phrases_coffre.contains(m)).cloned().collect(),
        a_poser: r.a_poser.clone(),
        horloge: r.horloge.clone(),
        etapes: r.etapes.clone(),
    };
    socle::ecriture::ecrire_json(&p, &trace, 1)
}

/// Ce qui persiste de la sauvegarde (`sauvegarde::souci`) ; un index qui ne se lit pas est un incident du passage.
fn souci_de_la_sauvegarde(ctx: &Contexte, r: &mut Ouverture) -> Option<String> {
    match sauvegarde::souci(&ctx.racine) {
        Ok(s) => s,
        Err(e) => {
            r.incidents.push(format!("Sauvegarde : {}", nomme(&ctx.racine, &e.echec)));
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
///
/// DEUX CHEMINS L'APPELLENT, et elle seule prend en compte une version : la route `ouverture` (`dire` y entre dans son
/// etape), et la file des etats de chaque compagnie, en tete, avant tout etat (`etat_garde::Cle::VersionDuCalcul`, qui
/// le dit par sa phrase) : du calcul local, ni coffre ni reseau. DANS LA FENETRE, L'ORDRE FAIT LA SURETE : la file la
/// passe d'abord, et la veille de la compagnie (qui sert la route `ouverture`) ne part qu'apres
/// (`etat_garde::FileDesEtats::attendre_la_version`) ; les deux ne se croisent pas. LE VERROU DE LA VERSION
/// (`VERROU_DE_LA_VERSION`) ne garde que d'un autre processus (une route en ligne de commande, `moteur route`) : il n'en
/// laisse passer qu'un, et l'autre l'apprend (`PriseEnCompte::EnCoursAilleurs`), y compris en route, quand son verrou,
/// rafraichi avant chaque mois, n'est plus le sien (passe le delai sans rafraichissement, un autre l'a leve et repris).
/// Pendant ce temps, une route qui ecrit, en ligne de commande, refuse (`porte_pendant_la_version`).
pub(crate) fn passer_la_version(ctx: &Contexte, dire: impl FnOnce() -> R<()>) -> R<PriseEnCompte> {
    let marque = VersionDuCalcul { version: format!("{}+{}", compagnie::maj::courante(&compagnie::maj::dossier_installe()?)?, engagement::empreinte::MOTEUR) };
    let p = ctx.racine.join(socle::fichiers::CONFIG).join(VERSION_DU_CALCUL);
    if deja_passee(&p, &marque)? {
        return Ok(PriseEnCompte::DejaAJour);
    }
    let passage = PASSAGES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let qui = socle::verrou::marque(&format!("mise a jour, passage {}", passage), socle::processus::numero(), &socle::paris::ecrit::seconde::texte(socle::temps::secondes()));
    let verrou = match socle::verrou::Verrou::prendre(&VERROU_DE_LA_VERSION.dans(&ctx.racine), &qui, socle::verrou::DELAI_VERROU).map_err(|m| m.reclasser(socle::echec::Classe::Disque))? {
        Ok(v) => v,
        Err(socle::verrou::Verrouille { .. }) => return Ok(PriseEnCompte::EnCoursAilleurs),
    };
    // Celui qui tenait le verrou vient peut-etre de finir.
    if deja_passee(&p, &marque)? {
        return Ok(PriseEnCompte::DejaAJour);
    }
    dire()?;
    let j = envois::journal::lu(&ctx.racine)?;
    if let Err(socle::verrou::Verrouille { .. }) = engagement::etats::corriger_les_mois_finis(ctx, &j.departs, &mut || verrou.rafraichir())? {
        return Ok(PriseEnCompte::EnCoursAilleurs);
    }
    socle::ecriture::ecrire_json(&p, &marque, 1).map_err(|m| m.reclasser(socle::echec::Classe::Disque))?;
    Ok(PriseEnCompte::Faite)
}

/// CE QU'A FAIT UNE PRISE EN COMPTE DE LA MISE A JOUR (`passer_la_version`) : elle s'est faite ici, la version avait
/// deja passe, ou un autre passage la fait en ce moment (il tient son verrou, `socle::verrou::Verrouille`) : celui-ci
/// s'est arrete, sans ecrire la marque, et l'autre la fait entiere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriseEnCompte {
    Faite,
    DejaAJour,
    EnCoursAilleurs,
}

/// Le numero du prochain passage de la version dans ce processus : il entre dans la marque de son verrou.
static PASSAGES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// LA ROUTE `ouverture` PASSE LA VERSION, en tete : son etape dite, puis effacee (un effacement rate se note a part, la
/// prise en compte, elle, est faite) ; un refus est un incident, et le passage continue. En cours dans un autre
/// processus : la route refuse (`EN_COURS_AILLEURS`), sans rien relever, envoyer ni sauvegarder, et le tour suivant de
/// la veille la reprend.
fn prendre_en_compte_la_version(ctx: &Contexte, r: &mut Ouverture) -> R<()> {
    let prise = passer_la_version(ctx, || dire_l_etape(ctx, r, PRISE_EN_COMPTE).map(|_| ()));
    r.finir_l_etape();
    match prise {
        Ok(PriseEnCompte::EnCoursAilleurs) => Err(socle::echec::refus(EN_COURS_AILLEURS)),
        Ok(PriseEnCompte::Faite) => {
            if let Err(e) = socle::fichiers::effacer(&chemin_etape(&ctx.racine)) {
                r.incidents.push(format!("L'étape du passage ne s'efface pas : {}", nomme(&ctx.racine, &e)));
            }
            Ok(())
        }
        Ok(PriseEnCompte::DejaAJour) => Ok(()),
        Err(e) => {
            r.incidents.push(format!("Mois finis : {}", nomme(&ctx.racine, &e)));
            Ok(())
        }
    }
}

/// Ce qu'une route qui ecrit dit pendant qu'un autre processus prend en compte la mise a jour (`porte_pendant_la_version`,
/// la route `ouverture`), et ce que la file des etats dit sans fil quand elle l'attend (`etat_garde::Fil::pomper`).
pub(crate) const EN_COURS_AILLEURS: &str = "Prise en compte de la mise à jour en cours : réessayer dans un instant.";

/// LA PORTE DE `moteur route` PENDANT UNE PRISE EN COMPTE DE LA MISE A JOUR : une route qui ecrit dans une compagnie
/// (`est_une_ecriture`) refuse tant qu'un passage tient le verrou de sa version (`VERROU_DE_LA_VERSION`), sans rien
/// ecrire : la reecriture des paies ne se croise avec aucune ecriture. Pas la fenetre, que l'ordre tient
/// (`passer_la_version`). Un verrou abandonne ne retient rien.
pub(crate) fn porte_pendant_la_version(o: &crate::enveloppe::Options, route: Route) -> R<()> {
    if !crate::est_une_ecriture(route) || !crate::service::compagnie_nommee(o) {
        return Ok(());
    }
    match socle::verrou::tenu(&VERROU_DE_LA_VERSION.dans(&crate::service::racine_de(o)), socle::verrou::DELAI_VERROU)? {
        Some(_) => Err(socle::echec::refus(EN_COURS_AILLEURS)),
        None => Ok(()),
    }
}

/// La marque `p` dit-elle deja la version `marque` ?
fn deja_passee(p: &Path, marque: &VersionDuCalcul) -> R<bool> {
    if !p.exists() {
        return Ok(false);
    }
    let lue: VersionDuCalcul = socle::forme::lire_fichier(p, "La version qui a passé sur les mois finis ne se lit pas.")?;
    Ok(lue.version == marque.version)
}

/// L'etape d'une prise en compte de la mise a jour, a la route `ouverture` comme a la file des etats
/// (`etat_garde::FileDesEtats::phrase`).
pub(crate) const PRISE_EN_COMPTE: &str = "Prise en compte de la mise à jour…";

/// LE VERROU DE LA VERSION (`passer_la_version`) : un seul passage a la fois sur les mois finis d'une compagnie, d'un
/// processus a l'autre (dans la fenetre, l'ordre de la file et de la veille suffit). Tenu au plus le temps d'un passage,
/// rafraichi avant chaque mois ; un verrou abandonne (le logiciel arrete en route) se leve passe `DELAI_VERROU` sans
/// rafraichissement, par un seul preneur (`socle::verrou::prendre`), et le passage se refait (la marque n'a pas ete
/// ecrite).
pub(crate) const VERROU_DE_LA_VERSION: socle::fichiers::CheminRelatif = socle::fichiers::CheminRelatif::du_moteur("config/.version-du-calcul.verrou");

/// La version qui a passe sur les mois finis (`config/.version-du-calcul.json`) : le numero et la marque du code.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionDuCalcul {
    version: String,
}

// ------------------------------------------------------------------ outils

/// UN REFUS DIT DANS LES INCIDENTS DE L'OUVERTURE : sa phrase, puis ce qu'il porte a part (`socle::echec::Lieu`) -- le
/// fichier, relatif a la compagnie `racine` quand il y est, et la donnee : une panne nomme ce qui la fait, sans quoi
/// « Un fichier de la compagnie manque. » ne dit pas lequel.
fn nomme(racine: &Path, e: &socle::echec::Echec) -> String {
    let Some(lieu) = e.a_part.as_ref().and_then(socle::echec::APart::lieu) else { return e.phrase.clone() };
    let chemin = lieu.chemin.as_deref().map(|c| match Path::new(c).strip_prefix(racine) {
        Ok(relatif) => relatif.display().to_string(),
        Err(_) => c.to_string(),
    });
    let fichier = match (&lieu.fichier, chemin) {
        (Some(f), _) => Some(f.texte().to_string()),
        (None, c) => c,
    };
    let cle = lieu.cle.as_deref().filter(|c| !c.is_empty());
    match (fichier, cle) {
        (Some(f), Some(c)) => format!("{} (fichier {}, donnée « {} »)", e.phrase, f, c),
        (Some(f), None) => format!("{} (fichier {})", e.phrase, f),
        (None, Some(c)) => format!("{} (donnée « {} »)", e.phrase, c),
        (None, None) => e.phrase.clone(),
    }
}

/// La phrase des spectacles que le temps a ranges ; aucune s'il n'y en a pas.
fn phrase_des_spectacles_ranges(ranges: &[String]) -> Option<String> {
    let noms = ranges.iter().map(|t| format!("« {} »", t)).collect::<Vec<_>>().join(", ");
    match ranges.len() {
        0 => None,
        1 => Some(format!("Spectacle rangé dans les archives après 24 mois sans engagement : {}. Il se réactive dans Configuration › Spectacles.", noms)),
        _ => Some(format!("Spectacles rangés dans les archives après 24 mois sans engagement : {}. Ils se réactivent dans Configuration › Spectacles.", noms)),
    }
}

/// La compagnie est une copie de developpement (`MARQUEUR_COPIE`).
fn est_copie(racine: &Path) -> bool {
    racine.join(MARQUEUR_COPIE).is_file()
}

/// `pas_avant` : l'age de la derniere ouverture (le `quand` de sa trace), dit en minutes entieres (« 3 » ; une trace
/// d'apres maintenant, « -1 »), quand elle est plus recente que `pas_avant` ; `None` sinon.
fn trop_recente(precedente: &Trace, pas_avant: Duration) -> Option<String> {
    let maintenant = socle::temps::secondes();
    match maintenant.depuis(precedente.quand) {
        Some(age) => (age < pas_avant).then(|| socle::temps::ecrit::minutes_entieres_dites(age)),
        None => precedente.quand.depuis(maintenant).map(|avance| format!("-{}", socle::temps::ecrit::minutes_commencees_dites(avance))),
    }
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
fn clore(ctx: &Contexte, r: &mut Ouverture, debut: std::time::Instant) -> R<()> {
    finir_les_etapes(ctx, r)?;
    trier_hors_ligne(r);
    r.duree = socle::temps::duree(debut);
    noter_ouverture(ctx, r)
}

/// La seconde moitie de la veille : la trace du premier appel, mise a jour par la sauvegarde (qui est faite en dernier) sans
/// rien effacer de ce que le premier a ecrit. Le resultat est celui d'une seule ouverture : la sauvegarde, la duree totale,
/// « hors ligne » si l'un des deux l'a vu (et alors plus de ligne « retours », comme la trace d'une seule ouverture), les
/// incidents et les manques a la suite des premiers. Sans trace d'avant : la trace ordinaire de ce passage.
fn completer_la_trace(ctx: &Contexte, r: &mut Ouverture, debut: std::time::Instant, precedente: Option<Trace>) -> R<()> {
    finir_les_etapes(ctx, r)?;
    let Some(mut trace) = precedente else {
        return clore(ctx, r, debut);
    };
    trier_hors_ligne(r);
    r.duree = socle::temps::duree(debut);
    trace.sauvegarde = r.sauvegarde.as_ref().map(Sauvegarde::resume);
    trace.sauvegarde_souci = souci_de_la_sauvegarde(ctx, r);
    trace.secondes += r.duree;
    trace.hors_ligne = trace.hors_ligne || r.hors_ligne;
    // La sauvegarde qui ne passe pas ne tait plus la ligne des retours : la releve, elle, est passee.
    trace.injoignables.extend(r.injoignables.iter().cloned());
    trace.incidents.extend(r.incidents.iter().cloned());
    trace.a_poser.extend(r.a_poser.iter().cloned());
    trace.manquants.extend(r.manquants.iter().filter(|m| !r.phrases_coffre.contains(m)).cloned());
    trace.etapes.extend(r.etapes.iter().cloned());
    socle::ecriture::ecrire_json(&chemin_trace(ctx), &trace, 1)
}

// ------------------------------------------------------------------ l'ouverture

/// `ouverture` : ce qui se fait a l'ouverture du logiciel, et toutes les N minutes ensuite. Ce qui ne repond pas (un
/// organisme, le Drive, un fichier illisible) est un incident qu'elle rend ; seul le disque qui refuse d'ecrire sa trace
/// ou son etape la fait echouer.
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
pub fn ouverture(ctx: &Contexte, opt: &Options, mut clients: Clients<'_>, mut apres_releve: Option<&mut dyn FnMut()>) -> R<Ouverture> {
    let debut = std::time::Instant::now();
    let mut r = Ouverture::commence();
    // UNE VALEUR D'UNE FICHE A SAISIR DE NOUVEAU (`routes::redire`) : rien ne se releve, ne s'inscrit ni ne se sauvegarde
    // tant que la compagnie n'a pas repondu ; l'ouverture rend la main sans rien ecrire, et le dit.
    if let Some(phrase) = crate::redire::en_attente(ctx)? {
        r.manquants.push(phrase);
        r.duree = debut.elapsed();
        return Ok(r);
    }
    let mut sauvegarder = opt.sauvegarder;
    let mut retours = opt.retours;
    let mut signatures = opt.signatures;
    // La trace d'avant ce passage : `pas_avant` la lit, la seconde moitie de la
    // veille la complete.
    let precedente = match derniere_ouverture(&ctx.racine) {
        Ok(p) => p,
        Err(e) => {
            r.incidents.push(format!("Trace de l'ouverture : {}", nomme(&ctx.racine, &e)));
            None
        }
    };

    if opt.sauvegarde_seule {
        return sauvegarder_seulement(ctx, opt, clients, r, debut, precedente);
    }

    if let (false, Some(p)) = (opt.pas_avant.is_zero(), &precedente) {
        if let Some(minutes) = trop_recente(p, opt.pas_avant) {
            r.incidents.push(format!("Ouverture deja faite il y a {} minute(s) : rien a refaire.", minutes));
            r.retenue = true;
            return Ok(r);
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
    let (branchement, fictive) = match compagnie::bridage::branchement(&ctx.racine).and_then(|b| Ok((b, compagnie::installation::est_fictive(&ctx.racine)?))) {
        Ok(x) => x,
        Err(e) => {
            r.incidents.push(nomme(&ctx.racine, &e));
            return Ok(r);
        }
    };
    if branchement.refus().is_some() {
        retours = false;
        signatures = false;
    }

    // UNE VERSION QUI CALCULE AUTREMENT passe une fois sur les mois finis, avant le present.
    prendre_en_compte_la_version(ctx, &mut r)?;

    // LES SPECTACLES QUE LE TEMPS RANGE : plus de 24 mois sans engagement ni reactivation, ils s'archivent. Des engagements
    // qui ne se lisent pas ne rangent rien, et le passage le dit.
    match crate::fiches::ranger_les_spectacles_endormis(ctx) {
        Ok(ranges) => r.spectacles_ranges = phrase_des_spectacles_ranges(&ranges).into_iter().collect(),
        Err(e) => r.incidents.push(format!("Spectacles sans engagement : {}", nomme(&ctx.racine, &e))),
    }

    // LA COMPAGNIE FICTIVE : les retours des depots joues et les signatures demandees arrivent d'ici, sans coffre ni reseau --
    // et rien ne manque au coffre, puisqu'il n'y a rien a y poser.
    if branchement.joue() {
        // L'HORLOGE D'ABORD : ce qui etait du avant aujourd'hui se joue, a sa date.
        dire_l_etape(ctx, &mut r, "Compagnie fictive : l'administrateur fictif fait ce qui tombait…")?;
        let jouee = crate::horloge::rattraper(&ctx.racine, None, true).and_then(|fait| if fait.is_empty() { crate::horloge::dernier_rattrapage(&ctx.racine, None) } else { Ok(fait) });
        match jouee {
            Ok(fait) => r.horloge = fait,
            Err(e) => r.incidents.push(format!("Horloge de la compagnie fictive : {}", nomme(&ctx.racine, &e))),
        }
        if retours {
            dire_l_etape(ctx, &mut r, "Compagnie fictive : retours des dépôts joués…")?;
            let mut releve = Releve::vide();
            match crate::gestes::releve::rattacher_simulation(ctx) {
                Ok((n, dit)) => {
                    releve.rattaches = n;
                    r.dire(dit);
                }
                // L'ouverture ne doit pas tomber.
                Err(e) => r.incidents.push(format!("Retours : {}", nomme(&ctx.racine, &e))),
            }
            r.releve = Some(releve);
        }
        dire_l_etape(ctx, &mut r, "Compagnie fictive : signatures et fiches des salariés…")?;
        // Le resultat de la relève jouee n'est pas gardé.
        if let Err(e) = releve_signatures::relever_signatures_fictive(ctx) {
            r.incidents.push(format!("Signatures : {}", nomme(&ctx.racine, &e)));
        }
        match crate::fiches::relever_fiches(ctx, reborrow_courriel(&mut clients.courriel), crate::horloge::QUI_HORLOGE) {
            // Les incidents de la relève des fiches jouée ne sont pas repris.
            Ok(f) => r.fiches = Some(f),
            Err(e) => r.incidents.push(format!("Fiches salariés : {}", nomme(&ctx.racine, &e))),
        }
        r.finir_l_etape();
        retours = false;
        signatures = false;
        sauvegarder = false;
    }

    if retours {
        relever_les_retours(ctx, opt, &mut clients, &mut r)?;
    }

    if signatures {
        dire_l_etape(ctx, &mut r, "Boîte courriel : contrats revenus signés…")?;
        // Les reponses « Je signe ». La relève ne leve pas pour un compte absent ou une permission manquante : elle le dit.
        match releve_signatures::relever_signatures(ctx, reborrow_courriel(&mut clients.courriel), None, "veille") {
            Ok(s) => {
                r.incidents.extend(s.incidents.iter().cloned());
                r.signatures = Some(s);
            }
            Err(e) => r.incidents.push(format!("Signatures : {}", nomme(&ctx.racine, &e))),
        }
        // Les fiches demandees aux salaries : la meme boite, le meme passage, la meme regle.
        dire_l_etape(ctx, &mut r, "Boîte courriel : fiches des salariés…")?;
        match crate::fiches::relever_fiches(ctx, reborrow_courriel(&mut clients.courriel), "veille") {
            Ok(f) => {
                r.incidents.extend(f.incidents.iter().cloned());
                r.fiches = Some(f);
            }
            Err(e) => r.incidents.push(format!("Fiches salariés : {}", nomme(&ctx.racine, &e))),
        }
        r.finir_l_etape();
    }

    // LES ECARTS DE CALCUL DES PAIES IMPORTEES qui ne sont pas encore comparees (`import::ecarts::noter`) : une
    // compagnie completee depuis l'import compare celles que le calcul refusait. Ils partent avec les autres incidents.
    if let Err(e) = import::ecarts::noter(ctx) {
        r.incidents.push(format!("Écarts de calcul de l'import : {}", nomme(&ctx.racine, &e)));
    }

    // LE TAUX COLLECTIF DE L'ARRETE pour un risque « TC », au 1er janvier de l'annee en cours et, des que son bareme est
    // livre, de la suivante (`routes::atmp::inscrire_l_arrete`) : il s'inscrit dans la case, qui le montre.
    let an = ctx.aujourdhui.annee();
    for an in an.jusqu_a(an.suivante()?) {
        if let Err(e) = crate::atmp::inscrire_l_arrete(ctx, an) {
            r.incidents.push(format!("Taux AT/MP {} : {}", an, nomme(&ctx.racine, &e)));
        }
    }

    // LE DIAGNOSTIC A DISTANCE : ce que le journal des incidents a note part chez l'editeur -- pas depuis la
    // copie de developpement (ses refus sont ceux qu'on provoque pour essayer), mais depuis une compagnie fictive. Et les
    // reponses de l'editeur aux signalements de ce poste. Pas pendant les tests (`PUPPETPAIE_INCIDENTS` pose : le journal est
    // celui d'un bac a sable). C'est fait avant de rendre la main.
    let bac_a_sable = matches!(std::env::var("PUPPETPAIE_INCIDENTS"), Ok(v) if !v.is_empty());
    if opt.filet && opt.incidents_editeur && !bac_a_sable && (!copie || fictive) {
        dire_l_etape(ctx, &mut r, "Serveur de l'éditeur : incidents, messages et réponses…")?;
        // Ce qui echoue ici ne fait pas refuser l'ouverture : la veille suivante reessaie, et l'ouverture le dit.
        if let Some(e) = compagnie::incidents::envoyer(&ctx.racine).erreur {
            r.incidents.push(format!("Incidents chez l'éditeur : {}", e));
        }
        // LE NOMBRE DE PAIES DE L'ANNEE PASSEE, a partir du 16 janvier, une fois : `nombre_de_paies` decide
        // seul (ni fictive, ni copie de developpement, ni logiciel bride, ni envoi coupe) et retient ce qui est parti.
        if let Some(e) = compagnie::nombre_de_paies::envoyer_si_du(ctx).erreur {
            r.incidents.push(format!("Nombre de paies chez l'éditeur : {}", e));
        }
        // Les messages « Un probleme, une idee ? » que l'envoi en fond n'a pas fait partir (serveur injoignable...).
        if let Some(e) = compagnie::problemes::partir(&ctx.racine).erreur {
            r.incidents.push(format!("Messages à l'éditeur : {}", e));
        }
        if let Err(e) = compagnie::problemes::relever_reponses(&ctx.racine) {
            r.incidents.push(format!("Réponses de l'éditeur : {}", nomme(&ctx.racine, &e)));
        }
        r.finir_l_etape();
    }

    if let Some(rappel) = apres_releve.as_mut() {
        clore(ctx, &mut r, debut)?;
        rappel();
    }

    if sauvegarder {
        sauvegarder_le_dossier(ctx, opt, &mut clients, &mut r)?;
    }

    clore(ctx, &mut r, debut)?;
    Ok(r)
}

/// `sauvegarde_seule` : la seconde moitie de la veille. La copie de developpement et la compagnie fictive ne sauvegardent pas
/// (rien a faire, la trace n'est pas touchee) ; sinon la sauvegarde, en dernier, et la trace du premier appel completee.
fn sauvegarder_seulement(ctx: &Contexte, opt: &Options, mut clients: Clients<'_>, mut r: Ouverture, debut: std::time::Instant, precedente: Option<Trace>) -> R<Ouverture> {
    let joue = match compagnie::bridage::branchement(&ctx.racine) {
        Ok(b) => b.joue(),
        Err(e) => {
            r.incidents.push(nomme(&ctx.racine, &e));
            return Ok(r);
        }
    };
    if !opt.sauvegarder || est_copie(&ctx.racine) || joue {
        return Ok(r);
    }
    sauvegarder_le_dossier(ctx, opt, &mut clients, &mut r)?;
    completer_la_trace(ctx, &mut r, debut, precedente)?;
    Ok(r)
}

/// Reprend la boite donnee sans la perdre : les signatures puis les fiches la relisent l'une apres l'autre.
fn reborrow_courriel<'b>(c: &'b mut Option<&mut dyn Courriel>) -> Option<&'b mut dyn Courriel> {
    match c {
        Some(x) => Some(&mut **x),
        None => None,
    }
}

/// L'ACCES `service` EST-IL POSE, MAINTENANT ? (`a_poser` : `net-entreprises`, `urssaf`, `google`.) Le coffre le dit sans
/// rien ouvrir (le mot de passe d'un organisme, pose ou dans l'environnement ; le jeton Google, lu) ; `Err` : le coffre ne se
/// lit pas. Le passage s'en sert pour noter ce qui manque, et le bandeau pour ne redire que ce qui manque encore
/// (`veille::acces_a_poser`) : un acces pose apres le releve disparait du bandeau tout de suite.
pub(crate) fn acces_pose(racine: &Path, service: &str) -> R<bool> {
    match service {
        "google" => Ok(crate::coffre::autorise(racine)),
        organisme => compagnie::coffre::Coffre::du_poste(racine).disponible(organisme),
    }
}

/// UN RELEVE QUI S'ARRETE (un delai depasse, une reponse coupee, un disque qui refuse) : la compagnie en lit la phrase
/// (`quoi : phrase`) ; ce qu'il porte a part part chez l'editeur par le journal des incidents, sauf une panne de passage
/// que la veille retente (`suites::noter_le_releve_arrete`), puisque l'ouverture ne refuse pas et qu'aucune route ne le
/// notera.
fn noter_le_releve_refuse(ctx: &Contexte, r: &mut Ouverture, quoi: &str, e: &socle::echec::Echec) {
    r.incidents.push(format!("{} : {}", quoi, nomme(&ctx.racine, e)));
    if let Err(n) = crate::gestes::suites::noter_le_releve_arrete(&ctx.racine, "ouverture", e) {
        r.incidents.push(format!("{} : le journal des incidents ne l'a pas noté : {}", quoi, n.phrase));
    }
}

/// Les retours DSN (net-entreprises) puis DPAE (Urssaf) : un secret absent est un manque, une panne est un incident.
fn relever_les_retours(ctx: &Contexte, opt: &Options, clients: &mut Clients<'_>, r: &mut Ouverture) -> R<()> {
    // Le canal DSN.
    let client_dsn = clients.dsn.take();
    let dispo = match &client_dsn {
        Some(_) => Some(true),
        None => match acces_pose(&ctx.racine, "net-entreprises") {
            Ok(b) => Some(b),
            Err(m) => {
                // Une erreur du coffre : l'ouverture ne tombe pas.
                r.incidents.push(format!("Retours : {}", nomme(&ctx.racine, &m)));
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
            dire_l_etape(ctx, r, "net-entreprises : retours des DSN…")?;
            let plage = envois::releve_retours::PlageDuReleve::Complete { jusqu_a: opt.jours };
            match crate::gestes::releve::relever_retours(ctx, plage, false, envois::releve_retours::EnvironnementNetEntreprises::Production, client_dsn, envois::releve_retours::CanalDesRetours::Dsn) {
                Ok((rel, dit)) => {
                    r.dire(dit);
                    r.releve = Some(rel);
                }
                Err(e) => noter_le_releve_refuse(ctx, r, "Retours", &e),
            }
            r.finir_l_etape();
        }
        None => {}
    }

    // LE CANAL DPAE AUSSI : une DPAE refusee en douze secondes ne reste pas « pas encore de retour »
    // jusqu'a ce qu'on tape la commande a la main.
    let client_dpae = clients.dpae.take();
    let dispo = match &client_dpae {
        Some(_) => Some(true),
        None => match acces_pose(&ctx.racine, "urssaf") {
            Ok(b) => Some(b),
            Err(m) => {
                r.incidents.push(format!("Retours DPAE : {}", nomme(&ctx.racine, &m)));
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
            dire_l_etape(ctx, r, "Urssaf : retours des DPAE…")?;
            // Un client donne passe par la meme releve que le vrai (canal `dpae`) ; sans client, `relever_retours_dpae`
            // cherche le mot de passe de consultation et construit le sien.
            let plage = envois::releve_retours::PlageDuReleve::Complete { jusqu_a: opt.jours };
            let rel = match client_dpae {
                Some(c) => crate::gestes::releve::relever_retours(ctx, plage, false, envois::releve_retours::EnvironnementNetEntreprises::Production, Some(c), envois::releve_retours::CanalDesRetours::Dpae),
                None => crate::gestes::releve::relever_retours_dpae(ctx, plage, false, None),
            };
            match rel {
                Ok((rel, dit)) => {
                    r.dire(dit);
                    r.releve_dpae = Some(rel);
                }
                Err(e) => noter_le_releve_refuse(ctx, r, "Retours DPAE", &e),
            }
            r.finir_l_etape();
        }
        None => {}
    }
    Ok(())
}

/// La sauvegarde, en dernier : un compte Google non lie est un manque ; rien de neuf sur le disque, en veille, est un
/// silence (aucun appel) ; une panne est un incident.
fn sauvegarder_le_dossier(ctx: &Contexte, opt: &Options, clients: &mut Clients<'_>, r: &mut Ouverture) -> R<()> {
    let pret = if clients.drive.is_some() {
        true
    } else {
        match acces_pose(&ctx.racine, "google") {
            Ok(b) => b,
            Err(e) => {
                r.incidents.push(format!("Sauvegarde : {}", nomme(&ctx.racine, &e)));
                return Ok(());
            }
        }
    };
    if !pret {
        r.a_poser.push("google".to_string());
        r.manque_coffre(
            "Compte Google non lie : la sauvegarde ne peut pas partir. Lier le Drive : Configuration > Mots de passe, ou la route google-autoriser.",
        );
        return Ok(());
    }
    if opt.veille {
        match sauvegarde::rien_n_a_bouge(&ctx.racine, &opt.racine_drive) {
            // rien de neuf sur le disque : on n'appelle pas
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => {
                r.incidents.push(format!("Sauvegarde : {}", nomme(&ctx.racine, &e.echec)));
                return Ok(());
            }
        }
    }
    let depuis = dire_l_etape(ctx, r, "Google Drive : sauvegarde…")?;
    let drive: Option<&mut dyn Drive> = match clients.drive.as_mut() {
        Some(d) => Some(&mut **d),
        None => None,
    };
    // LE TRANSFERT QUI AVANCE, dans l'etape que le bandeau relit chaque seconde : la premiere sauvegarde envoie tout.
    let mut avancer = |fait: usize, total: usize| redire_l_etape(&ctx.racine, &format!("Transfert Drive en cours : {} fichiers sur {}", socle::texte::compte(fait), socle::texte::compte(total)), depuis);
    match crate::gestes::sauvegarde::sauvegarder(&ctx.racine, &opt.racine_drive, false, drive, &mut avancer) {
        // CE QUI EST REPORTE N'EST PAS UN INCIDENT : un dossier que Google a refuse une fois (un 500,
        // deja retente par le client) repart au passage suivant, en silence. Ce qui persiste se dit dans la ligne de la
        // sauvegarde (`sauvegarde::souci`, la trace `sauvegarde_souci`), pas dans les incidents du releve.
        Ok(s) => r.sauvegarde = Some(s),
        Err(e) => r.incidents.push(format!("Sauvegarde : {}", nomme(&ctx.racine, &e.echec))),
    }
    r.finir_l_etape();
    Ok(())
}
