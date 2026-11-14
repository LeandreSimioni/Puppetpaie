//! Ce que la page ecrit pour l'exercice : `cloture` (et sa
//! reouverture, `rouvrir`) et `export-compta`.
//!
//! - L'export compta : les ecritures de l'annee (ou de `du` a `au`), mois par mois, et leur CSV
//!   (`documents::ecritures_comptables`).
//! - La cloture : fige l'annee (`config/exercices.json`) et reunit ce qu'il faut garder en un zip
//!   (`sorties/exercices/Exercice_AAAA.zip`) : saisies, sorties, retours des organismes, journal des depots,
//!   fiches, bareme, export compta, LISEZ-MOI. Le zip est ecrit par le zip de `routes::dossier_salarie` (deflate,
//!   CRC-32, entrees dans un ordre fixe) : la liste des fichiers et chacun d'eux sont fixes, octet pour octet ; les
//!   octets COMPRESSES (donc l'empreinte et la taille de l'archive) dependent du compresseur, et l'empreinte rendue
//!   est celle de l'archive que ce programme a reellement ecrite.
//!
//! Un refus de l'export pendant la cloture (un refus metier, `Refuse`, dont les refus du calcul) n'arrete pas la
//! cloture : il est rendu dans `manques`, et l'archive part sans export.

use std::collections::BTreeMap;
use socle::echec::Classe;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use droit::contexte::Contexte;
use socle::echec::erreur;
use socle::fichiers::{CheminRelatif, Empreinte, NomDeFichier};
use socle::mois::{Annee, Mois};
use socle::num::{zero2, Decimal};
use socle::paris::Moment;
use socle::echec::{Echec, R};
use crate::dossier_salarie::{entree_ecrite, entree_fichier, entree_tel_quel, zip, Entree};
use documents::ecritures_comptables::{csv_compta, ecritures_du_mois, Ecriture};
use documents::plan_comptable::plan_comptable;
use crate::rappels::mois_de_l_annee;

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}

#[track_caller]
fn en_os(e: &std::io::Error, p: &Path) -> Echec {
    socle::echec::du_disque(p, e).reclasser(Classe::Disque)
}

/// Une erreur d'operation, que l'appelant rattrape : celles que le moteur nomme, et
/// les refus du calcul (sans classe, `SaisieIncomplete`), qui en sont.
fn est_une_erreur_d_operation(e: &Echec) -> bool {
    matches!(e.classe, None | Some(Classe::Refuse) | Some(Classe::SaisieIncomplete))
}

/// L'heure de Paris, a la seconde.
fn maintenant() -> Moment {
    socle::temps::secondes().a_la_seconde()
}


// ------------------------------------------------------------------ le journal des depots

/// Une DSN est-elle partie pour ce mois, et pas refusee au bilan ? (`compagnie::etat_dsn`)
fn dsn_acceptee(ctx: &Contexte, mois: Mois) -> R<bool> {
    Ok(compagnie::etat_dsn::du_mois(ctx, mois, None)?.partie())
}

// ------------------------------------------------------------------ l'export compta

/// L'export compta d'un exercice.
pub(crate) struct ExportCompta {
    pub(crate) annee: Annee,
    pub(crate) du: u32,
    pub(crate) au: u32,
    pub(crate) mois: Vec<Mois>,
    pub(crate) ecritures: Vec<Ecriture>,
    pub(crate) total_debit: Decimal,
    pub(crate) total_credit: Decimal,
    pub(crate) fichier: Option<PathBuf>,
    pub(crate) plan: String,
    pub(crate) avertissements: Vec<String>,
}

/// Une ecriture de l'export, telle que la route la rend : les montants en texte.
#[derive(Debug, Serialize)]
pub(crate) struct EcritureRendue {
    journal: documents::ecritures_comptables::CodeJournal,
    date: socle::date::Jour,
    compte: documents::ecritures_comptables::NumeroDeCompte,
    libelle: String,
    #[serde(serialize_with = "socle::ecriture::decimal_en_texte::serialize")]
    debit: Decimal,
    #[serde(serialize_with = "socle::ecriture::decimal_en_texte::serialize")]
    credit: Decimal,
    analytique: Option<String>,
    mois: socle::mois::Mois,
    auxiliaire: Option<documents::ecritures_comptables::NumeroDeCompte>,
    auxiliaire_libelle: Option<String>,
}

/// Ce que rend `export-compta` : sa phrase, l'annee et ses mois (de `du` a `au`), les ecritures et leurs totaux, le
/// fichier ecrit (son nom ; `dossier`, celui ou il est), le plan comptable pris, et ce qu'il faut savoir.
#[derive(Debug, Serialize)]
pub(crate) struct ExportComptaRendu {
    phrase: String,
    annee: Annee,
    du: u32,
    au: u32,
    mois: Vec<Mois>,
    ecritures: Vec<EcritureRendue>,
    #[serde(serialize_with = "socle::ecriture::decimal_en_texte::serialize")]
    total_debit: Decimal,
    #[serde(serialize_with = "socle::ecriture::decimal_en_texte::serialize")]
    total_credit: Decimal,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    dossier: Option<PathBuf>,
    fichier: Option<NomDeFichier>,
    plan: String,
    avertissements: Vec<String>,
}

impl crate::enveloppe::Dite for ExportComptaRendu {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl ExportCompta {
    /// Ce que la route rend : un chemin ne rend que son nom ; `dossier`, celui ou il est ecrit.
    pub(crate) fn rendu(self) -> R<ExportComptaRendu> {
        let dossier = self.fichier.as_ref().and_then(|f| f.parent()).map(Path::to_path_buf);
        let fichier = self.fichier.as_deref().map(NomDeFichier::du).transpose()?;
        let phrase = match &fichier {
            Some(f) => format!("Export comptable de {} : {} écritures, dans {} (le dossier des exercices).", self.annee, self.ecritures.len(), f),
            None => format!("Export comptable de {} : {} écritures, rien n'est écrit.", self.annee, self.ecritures.len()),
        };
        Ok(ExportComptaRendu {
            phrase,
            annee: self.annee,
            du: self.du,
            au: self.au,
            mois: self.mois,
            ecritures: self
                .ecritures
                .into_iter()
                .map(|e| EcritureRendue {
                    journal: e.journal,
                    date: e.date,
                    compte: e.compte,
                    libelle: e.libelle,
                    debit: e.debit,
                    credit: e.credit,
                    analytique: e.analytique,
                    mois: e.mois,
                    auxiliaire: e.auxiliaire,
                    auxiliaire_libelle: e.auxiliaire_libelle,
                })
                .collect(),
            total_debit: self.total_debit,
            total_credit: self.total_credit,
            dossier,
            fichier,
            plan: self.plan,
            avertissements: self.avertissements,
        })
    }
}

fn dossier_exercices(ctx: &Contexte) -> PathBuf {
    ctx.racine.join("sorties").join("exercices")
}

/// Le journal de paie de l'annee (ou de `du` a `au`), et son fichier CSV.
/// Les mois pris sont ceux qui ont une saisie.
pub(crate) fn export_compta(ctx: &Contexte, annee: Annee, du: u32, au: u32, ecrire: bool) -> R<ExportCompta> {
    let plan = plan_comptable(ctx)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let propre = ctx.racine.join(documents::plan_comptable::PLAN_COMPTABLE);
    let origine = if propre.exists() { documents::plan_comptable::PLAN_COMPTABLE } else { "plan livré avec le logiciel (plan-comptable-base.json)" };
    let mut mois: Vec<Mois> = Vec::new();
    for m in mois_de_l_annee(ctx, annee)? {
        let n = m.numero();
        if du <= n && n <= au && engagement::stock::a_une_saisie(&ctx.racine, m).map_err(|e| e.reclasser(Classe::Refuse))? {
            mois.push(m);
        }
    }
    if mois.is_empty() {
        return Err(refus(format!(
            "Aucune paie saisie en {}{}.",
            annee,
            if (du, au) == (1, 12) { String::new() } else { format!(" entre les mois {} et {}", du, au) }
        )));
    }
    let mut avert: Vec<String> = Vec::new();
    let mut ecritures: Vec<Ecriture> = Vec::new();
    for &m in &mois {
        if !ctx.racine.join("sorties").join(m.to_string()).is_dir() {
            avert.push(format!("{} : saisi mais jamais écrit (pas de bulletin) -- pris quand même, depuis la saisie.", m));
        } else if !dsn_acceptee(ctx, m)? {
            avert.push(format!("{} : pas de DSN réelle acceptée au journal.", m));
        }
        ecritures.extend(ecritures_du_mois(ctx, m, &plan, &departs)?);
    }
    if !propre.exists() {
        avert.push("Plan comptable : celui livré avec le logiciel, à faire valider par le comptable (config/plan-comptable.json pour le remplacer).".to_string());
    }
    let zero = zero2();
    let total_debit = ecritures.iter().fold(zero, |a, e| a + e.debit);
    let total_credit = ecritures.iter().fold(zero, |a, e| a + e.credit);
    let mut fichier = None;
    if ecrire {
        let dossier = dossier_exercices(ctx);
        std::fs::create_dir_all(&dossier).map_err(|e| en_os(&e, &dossier))?;
        let nom = if (du, au) == (1, 12) { format!("Compta_{}.csv", annee) } else { format!("Compta_{}_{:02}-{:02}.csv", annee, du, au) };
        let f = dossier.join(nom);
        std::fs::write(&f, csv_compta(&ecritures)).map_err(|e| en_os(&e, &f))?;
        // L'export de l'annee entiere note les saisies qu'il a lues (`export_compta_a_jour`).
        let lues = match (du, au) == (JANVIER, DECEMBRE) {
            true => Some(compagnie::pieces_ecrites::EmpreinteNotee::Annee(empreintes_des_saisies(&ctx.racine, annee)?)),
            false => None,
        };
        compagnie::pieces_ecrites::noter_piece_ecrite(&ctx.racine, &compagnie::pieces_ecrites::cle_de_piece(&f), lues, None)?;
        fichier = Some(f);
    }
    Ok(ExportCompta { annee, du, au, mois, ecritures, total_debit, total_credit, fichier, plan: origine.to_string(), avertissements: avert })
}

/// LES SAISIES DE L'ANNEE, PAR LEUR EMPREINTE : `{"AAAA-MM": empreinte}` pour chaque mois saisi de l'annee, l'empreinte
/// de sa saisie (`engagement::stock::empreinte_de_la_saisie_du_mois` : ce qu'elle dit qui change la paie). Une saisie
/// illisible se refuse. L'export compta de l'annee la note en s'ecrivant (`noter_piece_ecrite`) : c'est ce qu'il a lu.
pub(crate) fn empreintes_des_saisies(racine: &Path, annee: Annee) -> R<BTreeMap<socle::mois::Mois, socle::fichiers::EmpreinteDeSaisie>> {
    let mut m = BTreeMap::new();
    for mois in engagement::stock::mois_saisis(racine).map_err(|e| e.reclasser(Classe::Refuse))?.into_iter().filter(|x| x.annee_civile() == annee) {
        if let Some(e) = engagement::stock::empreinte_de_la_saisie_du_mois(racine, mois, None).map_err(|e| e.reclasser(Classe::Refuse))? {
            m.insert(mois, e);
        }
    }
    Ok(m)
}

/// L'EXPORT COMPTA DE L'ANNEE EST-IL A JOUR ? Un seul critere, celui des rappels (`routes::rappels`) et de
/// l'horloge d'une fictive (`horloge::l_annee`) : `Compta_<annee>.csv` existe, et les saisies de l'annee sont celles
/// qu'il a lues -- les empreintes notees au journal des ecritures en l'ecrivant (`empreintes_des_saisies`). Jamais la
/// date des fichiers : une fictive decompressee chez un visiteur a des dates qui ne disent rien. Un export dont le
/// journal ne dit pas ce qu'il a lu n'est pas celui du moteur (pose a la main) : il ne compte pas, comme une DSN que le
/// moteur n'a pas notee (`depart::mois_ecrit`).
pub(crate) fn export_compta_a_jour(racine: &Path, annee: Annee) -> R<bool> {
    use compagnie::pieces_ecrites::EmpreinteNotee;
    let f = racine.join("sorties").join("exercices").join(format!("Compta_{}.csv", annee));
    if !f.is_file() {
        return Ok(false);
    }
    let a_jour = |notee: EmpreinteNotee| -> R<bool> {
        match notee {
            EmpreinteNotee::Annee(lues) => Ok(lues == empreintes_des_saisies(racine, annee)?),
            EmpreinteNotee::Saisie(saisie) => Err(socle::echec::erreur(
                Classe::DefautDuLogiciel,
                format!("Défaut du logiciel : l'export compta {} est noté avec l'empreinte d'un seul mois ({}), et non celles de l'année.", annee, saisie),
            )),
        }
    };
    // Le moteur ne l'a pas note : il ne l'a pas ecrit.
    Ok(compagnie::pieces_ecrites::piece_notee(racine, &f)?.and_then(|n| n.empreinte_saisie).map(a_jour).transpose()?.is_some_and(|oui| oui))
}

/// La charge de `export-compta` : l'annee, du mois au mois (toute l'annee sans eux), et l'apercu (rien n'est ecrit).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeExportCompta {
    annee: crate::dossier_salarie::AnneeDite,
    du: Option<u32>,
    au: Option<u32>,
    apercu: Option<bool>,
}

/// La route `export-compta` : sans `du` ni `au`, de janvier a decembre.
pub(crate) fn export_compta_route(ctx: &Contexte, c: ChargeExportCompta) -> R<ExportComptaRendu> {
    let du = match c.du {
        Some(m) => m,
        None => JANVIER,
    };
    let au = match c.au {
        Some(m) => m,
        None => DECEMBRE,
    };
    export_compta(ctx, c.annee.annee()?, du, au, c.apercu != Some(true))?.rendu()
}

/// Le premier et le dernier mois d'une annee.
const JANVIER: u32 = 1;
const DECEMBRE: u32 = 12;

// ------------------------------------------------------------------ la cloture

/// Les exercices clos de la compagnie, sous sa racine.
pub(crate) const EXERCICES: &str = "config/exercices.json";

/// `config/exercices.json` : chaque annee close (`clos`, sa cloture sous son annee) et les clotures retirees (`rouverts`),
/// dans l'ordre ou elles l'ont ete. Lu par sa forme (`lire_exercices`), ecrit par `ecrire_exercices`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Exercices {
    clos: BTreeMap<Annee, ExerciceNote>,
    rouverts: Vec<ExerciceRouvertNote>,
}

/// La cloture d'une annee : quand, par qui, l'archive (son chemin sous la racine, sa taille, son empreinte, le nombre de
/// fichiers qu'elle reunit), si elle a ete forcee, et les mois clos sans DSN acceptee.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExerciceNote {
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub(crate) le: Moment,
    qui: String,
    archive: CheminRelatif,
    octets: u64,
    empreinte: Empreinte,
    fichiers: u64,
    forcee: bool,
    sans_dsn: Vec<Mois>,
}

/// Une cloture retiree : la cloture telle qu'elle etait, son annee, quand et par qui elle l'a ete.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExerciceRouvertNote {
    #[serde(with = "socle::paris::ecrit::seconde")]
    le: Moment,
    qui: String,
    archive: CheminRelatif,
    octets: u64,
    empreinte: Empreinte,
    fichiers: u64,
    forcee: bool,
    sans_dsn: Vec<Mois>,
    annee: Annee,
    #[serde(with = "socle::paris::ecrit::seconde")]
    rouvert_le: Moment,
    rouvert_par: String,
}

impl Exercices {
    /// La cloture de l'annee `annee`, ou `None`.
    pub(crate) fn de(&self, annee: Annee) -> Option<&ExerciceNote> {
        self.clos.get(&annee)
    }

    /// Les annees closes, et leur cloture.
    pub(crate) fn clos(&self) -> &BTreeMap<Annee, ExerciceNote> {
        &self.clos
    }
}

/// `config/exercices.json`, lu par sa forme : aucune annee close s'il n'existe pas. Le texte est lu une fois tant qu'il
/// ne change pas (l'etat d'un mois le demande pour chaque mois) ; le refus d'un fichier illisible ne se garde pas.
pub(crate) fn lire_exercices(racine: &Path) -> R<Exercices> {
    let p = racine.join(EXERCICES);
    match socle::forme::partagee::<Exercices>(&p, "Le fichier des exercices clos ne se lit pas.").map_err(|m| m.reclasser(Classe::Illisible))? {
        Some(e) => Ok((*e).clone()),
        None => Ok(Exercices { clos: BTreeMap::new(), rouverts: Vec::new() }),
    }
}

fn ecrire_exercices(ctx: &Contexte, tous: &Exercices) -> R<()> {
    let chemin = ctx.racine.join(EXERCICES);
    socle::ecriture::ecrire_json(&chemin, tous, 1).map_err(|m| m.reclasser(Classe::Disque))
}

/// Les fichiers sous `d`, triés par chemin (les chemins se comparent composant par composant). Un dossier absent n'en a
/// aucun ; un dossier qui ne se lit pas se refuse.
fn fichiers_sous(d: &Path, out: &mut Vec<PathBuf>) -> R<()> {
    let rd = match std::fs::read_dir(d) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(en_os(&e, d)),
    };
    for e in rd {
        let p = e.map_err(|x| en_os(&x, d))?.path();
        if p.is_dir() {
            fichiers_sous(&p, out)?;
        } else if p.is_file() {
            out.push(p);
        }
    }
    Ok(())
}

/// Fige l'annee et reunit ce qu'il faut garder en un zip. Refuse si un mois
/// de l'annee qui a des paies n'a pas de DSN reelle acceptee (`forcer` passe outre, et c'est ecrit dans
/// `exercices.json`), une annee deja close, et l'annee en cours.
pub(crate) fn cloturer(ctx: &Contexte, annee: Annee, forcer: bool, qui: &str) -> R<ExerciceClos> {
    let mut tous = lire_exercices(&ctx.racine)?;
    if let Some(clos) = tous.de(annee) {
        return Err(refus(format!("L'exercice {} est déjà clos (le {}).", annee, clos.le.jour_de_paris().iso())));
    }
    if annee.nombre() >= ctx.aujourdhui.an {
        return Err(refus(format!("L'exercice {} n'est pas fini : on clôture une année passée.", annee)));
    }
    let mois = mois_de_l_annee(ctx, annee)?;
    let mut sans_dsn: Vec<Mois> = Vec::new();
    for &m in &mois {
        if engagement::stock::a_une_saisie(&ctx.racine, m).map_err(|e| e.reclasser(Classe::Refuse))? && !dsn_acceptee(ctx, m)? {
            sans_dsn.push(m);
        }
    }
    if !sans_dsn.is_empty() && !forcer {
        return Err(refus(format!(
            "Pas de DSN réelle acceptée pour {}. La clôture vient APRÈS la dernière DSN ; déposer d'abord, ou forcer en le sachant.",
            sans_dsn.iter().map(Mois::to_string).collect::<Vec<_>>().join(", ")
        )));
    }

    // L'export compta fait partie de ce qu'on garde : on l'ecrit si absent.
    let (fichier_export, manques) = match export_compta(ctx, annee, 1, 12, true) {
        Ok(e) => (e.fichier, Vec::new()),
        Err(e) if est_une_erreur_d_operation(&e) => (None, vec![format!("export compta : {}", e.phrase)]),
        Err(e) => return Err(e),
    };

    let dossier = dossier_exercices(ctx);
    std::fs::create_dir_all(&dossier).map_err(|e| en_os(&e, &dossier))?;
    let archive = dossier.join(format!("Exercice_{}.zip", annee));
    let mut entrees: Vec<Entree> = Vec::new();
    // Les engagements qui touchent l'annee : le fichier de chacun de ses mois, et celui du mois ou commence un engagement
    // qui court encore dans l'annee (un contrat commence l'annee d'avant).
    let mut fichiers_engagements: std::collections::BTreeSet<Mois> = std::collections::BTreeSet::new();
    for &m in &mois {
        fichiers_engagements.insert(m);
        for o in engagement::stock::objets_du_mois(&ctx.racine, m).map_err(|e| e.reclasser(Classe::Refuse))? {
            fichiers_engagements.insert(o.fichier);
        }
    }
    for &f in &fichiers_engagements {
        if let Some(t) = engagement::stock::tel_quel(&ctx.racine, f)? {
            entrees.push(entree_tel_quel(t));
        }
    }
    // Les paies de chaque mois de l'annee.
    for &m in &mois {
        if let Some(t) = engagement::paies::tel_quel(&ctx.racine, m)? {
            entrees.push(entree_tel_quel(t));
        }
    }
    for m in &mois {
        let d = ctx.racine.join("sorties").join(m.to_string());
        if d.is_dir() {
            let mut fichiers = Vec::new();
            fichiers_sous(&d, &mut fichiers)?;
            fichiers.sort();
            for p in fichiers {
                let rel: Vec<String> = p.strip_prefix(&d).map_err(|e| refus(e.to_string()))?.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
                entrees.push(entree_fichier(&p, format!("sorties/{}/{}", m, rel.join("/")))?);
            }
        }
    }
    // Les retours des organismes : les fichiers dont le nom commence par l'annee (AAAAMMJJ-HHMMSS_nature..). Ceux de
    // janvier suivant qui repondent a la DSN de decembre sont pris aussi : ils portent sur l'annee close.
    let (de_l_annee, suivante) = (annee.to_string(), format!("{}01", annee.nombre() + 1));
    // Les retours DSN a la racine des retours, ceux des DPAE sous `dpae/`.
    for canal in [envois::releve_retours::CanalDesRetours::Dsn, envois::releve_retours::CanalDesRetours::Dpae] {
        let d = canal.chemin(&ctx.racine);
        let prefixe = match canal.dossier() {
            Some(s) => format!("{}/", s),
            None => String::new(),
        };
        if d.is_dir() {
            let mut noms: Vec<(String, PathBuf)> = Vec::new();
            for e in std::fs::read_dir(&d).map_err(|e| en_os(&e, &d))? {
                let e = e.map_err(|x| en_os(&x, &d))?;
                noms.push((e.file_name().to_string_lossy().into_owned(), e.path()));
            }
            noms.sort_by(|a, b| a.1.cmp(&b.1));
            for (nom, p) in noms {
                if p.is_file() && (nom.starts_with(&de_l_annee) || nom.starts_with(&suivante)) {
                    entrees.push(entree_fichier(&p, format!("retours/{}{}", prefixe, nom))?);
                }
            }
        }
    }
    let j = envois::journal::chemin(&ctx.racine);
    if j.exists() {
        entrees.push(entree_fichier(&j, envois::journal::DEPOTS.to_string())?);
    }
    let cfg = ctx.racine.join(socle::fichiers::CONFIG);
    let fiches: [&str; 6] = ["entreprise.json", "salaries.json", "spectacles.json", "organismes.json", "rubriques.json", "compteurs.json"];
    for nom in fiches {
        if cfg.join(nom).exists() {
            entrees.push(entree_fichier(&cfg.join(nom), format!("config/{}", nom))?);
        }
    }
    let surcouche = droit::contexte::surcouche(annee.nombre())?;
    let bar = ctx.racine.join(surcouche.texte());
    if bar.exists() {
        entrees.push(entree_fichier(&bar, surcouche.texte().to_string())?);
    }
    if let Some(f) = &fichier_export {
        let nom = f.file_name().ok_or_else(|| refus("L'export compta n'a pas de nom de fichier."))?;
        entrees.push(entree_fichier(f, format!("compta/{}", nom.to_string_lossy()))?);
    }
    let aujourd_hui = ctx.aujourdhui;
    let lisez_moi = format!(
        "Exercice {} -- archive de cloture, PuppetPaie, le {:02}/{:02}/{:04}.\n\n\
         engagements/ les engagements de chaque mois ou ils commencent, et leurs paies validees\n\
         sorties/    bulletins, contrats (signes dans envoi/), DPAE, DSN, certificats, courriels\n\
         retours/    les reponses des organismes (net-entreprises, Urssaf DPAE)\n\
         journal/    le journal des depots\n\
         config/     les fiches telles qu'elles etaient a la cloture\n\
         compta/     le journal des ecritures de paie\n",
        annee, aujourd_hui.jour, aujourd_hui.mois, aujourd_hui.an
    );
    entrees.push(entree_ecrite("LISEZ-MOI.txt".to_string(), lisez_moi.into_bytes()));
    let n = entrees.len();
    let octets = zip(&entrees)?;
    std::fs::write(&archive, &octets).map_err(|e| en_os(&e, &archive))?;

    let empreinte = Empreinte::des_octets(&octets);
    let le = maintenant();
    let relatif = CheminRelatif::sous(&ctx.racine, &archive)?;
    tous.clos.insert(
        annee,
        ExerciceNote {
            le,
            qui: qui.to_string(),
            archive: relatif,
            octets: octets.len() as u64,
            empreinte: empreinte.clone(),
            fichiers: n as u64,
            forcee: forcer && !sans_dsn.is_empty(),
            sans_dsn: if forcer { sans_dsn } else { Vec::new() },
        },
    );
    ecrire_exercices(ctx, &tous)?;
    Ok(ExerciceClos { annee, le, archive, octets: octets.len(), empreinte, fichiers: n, manques })
}

/// Un exercice clos : l'annee, quand, l'archive (son chemin, sa taille, son empreinte, le nombre de fichiers qu'elle
/// reunit), et ce qui n'a pu s'y archiver.
#[derive(Debug, serde::Serialize)]
pub(crate) struct ExerciceClos {
    pub(crate) annee: Annee,
    #[serde(with = "socle::paris::ecrit::seconde")]
    le: Moment,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    archive: PathBuf,
    octets: usize,
    empreinte: Empreinte,
    pub(crate) fichiers: usize,
    pub(crate) manques: Vec<String>,
}

/// Un exercice rouvert : l'annee, depuis quand il etait clos, et son archive, qui reste.
#[derive(Debug, serde::Serialize)]
pub(crate) struct ExerciceRouvert {
    pub(crate) annee: Annee,
    #[serde(with = "socle::paris::ecrit::seconde")]
    etait_clos_depuis: Moment,
    archive: CheminRelatif,
}

/// Ce que fait la route `cloture` : clore l'exercice, ou le rouvrir.
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub(crate) enum Cloture {
    Close(ExerciceClos),
    Rouverte(ExerciceRouvert),
}

/// La charge de `cloture` : l'annee (comme `etapes::ChargeAnnee` l'ecrit, ou en nombre), clore malgre un mois sans DSN
/// acceptee (`forcer`), ou rouvrir ; `qui` : la ligne de commande ecrit `cli`, la page rien (`interface`).
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeCloture {
    annee: crate::dossier_salarie::AnneeDite,
    forcer: Option<bool>,
    rouvrir: Option<bool>,
    qui: Option<socle::forme::TexteDit>,
}

/// Retire la cloture. L'archive reste sur le disque : elle dit ce
/// qu'etait l'annee au moment ou on l'a fermee.
fn rouvrir_exercice(ctx: &Contexte, annee: Annee, qui: &str) -> R<ExerciceRouvert> {
    let mut tous = lire_exercices(&ctx.racine)?;
    let Some(clos) = tous.clos.remove(&annee) else {
        return Err(refus(format!("L'exercice {} n'est pas clos.", annee)));
    };
    let (etait_clos_depuis, archive) = (clos.le, clos.archive.clone());
    let ExerciceNote { le, qui: clos_par, archive: a, octets, empreinte, fichiers, forcee, sans_dsn } = clos;
    tous.rouverts.push(ExerciceRouvertNote {
        le,
        qui: clos_par,
        archive: a,
        octets,
        empreinte,
        fichiers,
        forcee,
        sans_dsn,
        annee,
        rouvert_le: maintenant(),
        rouvert_par: qui.to_string(),
    });
    ecrire_exercices(ctx, &tous)?;
    Ok(ExerciceRouvert { annee, etait_clos_depuis, archive })
}

/// La route `cloture` : clore l'annee, ou la rouvrir.
pub(crate) fn cloture_route(ctx: &Contexte, c: ChargeCloture) -> R<Cloture> {
    let annee = c.annee.annee()?;
    // Qui ecrit : la ligne de commande le dit (`cli`), la page non (l'interface).
    let qui = match c.qui {
        Some(q) => q.to_string(),
        None => compagnie::INTERFACE.to_string(),
    };
    if c.rouvrir == Some(true) {
        return rouvrir_exercice(ctx, annee, &qui).map(Cloture::Rouverte);
    }
    cloturer(ctx, annee, c.forcer == Some(true), &qui).map(Cloture::Close)
}
