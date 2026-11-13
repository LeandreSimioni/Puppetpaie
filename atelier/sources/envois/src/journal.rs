//! LE JOURNAL DE CE QUI EST PARTI : `journal/depots.jsonl`, qu'on relit filtre (`lire`), l'empreinte d'un fichier, et ce
//! qui est parti de chaque engagement, sur quelle version (`lu` : une lecture, d'ou `departs`, que `moteur` passe a
//! l'engagement). Les etats du mois, les resumes et le rattachement des retours sont dans `etats` ; la forme d'une ligne,
//! dans `ligne`.
//!
//! SA FORME : un enregistrement JSON par ligne (`Enregistrement`), et l'on n'ecrit qu'en ajoutant des lignes a la fin ;
//! ce qui est ecrit ne se reecrit jamais. Trois enregistrements :
//! - un envoi, `{"id", "ligne"}` (`noter`) : `ligne` est ce que le journal lu en montre (`ligne::Ligne`), `id`
//!   l'empreinte de son texte ;
//! - un retour, `{"a", "retour"}` (`rattacher`, `ranger`) : il s'ajoute aux `retours` de l'envoi `a`, dans la forme de
//!   sa sorte (`ligne::RetourOrganisme`, `RetourContrat`, `RetourFiche`) ;
//! - un complement, `{"a", "nature", "publication", "complete"}` (`ranger`) : le verdict (`ligne::VerdictDeLaPiece`) s'ecrit dans
//!   le retour de l'envoi `a` qui porte cette nature et cette publication (le verdict d'un bilan arrive apres son annonce).
//! `lu` replie ces enregistrements dans l'ordre en `lignes` ; un processus ne relit que ce qui s'est ajoute depuis sa
//! derniere lecture. Un enregistrement qui ne se lit pas strictement (une cle inconnue, une valeur d'une autre forme)
//! arrete la lecture, sa ligne et sa cle nommees.
//!
//! DEUX JOURNAUX EN CONFLIT (deux postes, une sauvegarde) SE REUNISSENT EN METTANT LEURS LIGNES BOUT A BOUT, dans
//! n'importe quel ordre qui garde celui de chacun : un envoi deja lu (le meme `id`) et un retour deja range
//! (la meme nature et la meme publication, ou le meme retour, sur le meme envoi) ne comptent qu'une fois. Rien ne se
//! perd, rien ne compte double (regle 9 b).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use socle::fichiers::{Empreinte, EmpreinteCourte};
use socle::mois::Mois;
use socle::echec::{refus, Echec, R};
use socle::paris::{InstantOuJour, Moment};

pub use crate::ligne::*;

/// Le dossier du journal d'une compagnie, sous sa racine : ce qui est parti, les incidents, les messages.
pub const JOURNAL: &str = "journal";

/// Le journal de ce qui est parti, sous la racine de la compagnie.
pub const DEPOTS: &str = "journal/depots.jsonl";

/// `<racine>/journal/depots.jsonl`.
pub fn chemin(racine: &Path) -> PathBuf {
    racine.join(DEPOTS)
}

/// L'empreinte d'un fichier, comme le journal l'écrit.
pub fn empreinte_fichier(chemin: &Path) -> R<EmpreinteCourte> {
    Ok(Empreinte::du_fichier(chemin)?.courte())
}

/// L'IDENTIFIANT D'UN ENVOI AU JOURNAL, que `a` reprend : l'empreinte courte du texte de sa ligne, suivie de `-<rang>`
/// quand une ligne au meme texte porte deja l'empreinte seule (le rang, a partir de 2). C'est la forme que le seul
/// ecrivain fait (`noter`) ; lu, il refuse toute autre forme.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
struct IdDEnvoi {
    empreinte: EmpreinteCourte,
    /// 1 : l'empreinte seule ; au-dela, le rang ecrit apres elle.
    rang: u32,
}

impl IdDEnvoi {
    /// L'identifiant de la premiere ligne a ce texte : son empreinte seule.
    fn premier(empreinte: EmpreinteCourte) -> IdDEnvoi {
        IdDEnvoi { empreinte, rang: 1 }
    }

    /// Celui de la ligne suivante au meme texte : le rang d'apres.
    fn suivant(&self) -> IdDEnvoi {
        IdDEnvoi { empreinte: self.empreinte.clone(), rang: self.rang + 1 }
    }

    /// L'identifiant ecrit `texte`, ou la phrase qui le refuse.
    fn lire(texte: &str) -> R<IdDEnvoi> {
        let refuse = || socle::refus!("{} n'est pas l'identifiant d'un envoi : 16 caracteres hexadecimaux minuscules, suivis de « -<rang> » a partir de 2.", socle::fichiers::repr(texte));
        let (empreinte, rang) = match texte.split_once('-') {
            None => (texte, 1),
            Some((e, r)) => {
                let chiffres = !r.is_empty() && !r.starts_with('0') && r.bytes().all(|o| o.is_ascii_digit());
                match r.parse::<u32>() {
                    Ok(n) if chiffres && n >= 2 => (e, n),
                    _ => return Err(refuse()),
                }
            }
        };
        let empreinte = EmpreinteCourte::lire(empreinte).map_err(|_| refuse())?;
        Ok(IdDEnvoi { empreinte, rang })
    }
}

impl std::fmt::Display for IdDEnvoi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.rang {
            1 => write!(f, "{}", self.empreinte),
            r => write!(f, "{}-{}", self.empreinte, r),
        }
    }
}

impl TryFrom<String> for IdDEnvoi {
    type Error = Echec;
    fn try_from(s: String) -> R<IdDEnvoi> {
        IdDEnvoi::lire(&s)
    }
}

impl From<IdDEnvoi> for String {
    fn from(i: IdDEnvoi) -> String {
        i.to_string()
    }
}

/// UN ENREGISTREMENT DU FICHIER : un envoi (`id`, `ligne`), un retour (`a`, `retour`) ou un complement (`a`, `nature`,
/// `publication`, `complete`). Le retour se lit dans la forme de la sorte de son envoi (`Tenu::replier`).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Enregistrement {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<IdDEnvoi>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ligne: Option<Ligne>,
    #[serde(skip_serializing_if = "Option::is_none")]
    a: Option<IdDEnvoi>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retour: Option<Retour>,
    #[serde(rename = "nature", skip_serializing_if = "Option::is_none")]
    nature_retour: Option<crate::retours::NatureDuRetour>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "socle::paris::ecrit::compacte::option")]
    publication: Option<socle::paris::Moment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    complete: Option<VerdictDeLaPiece>,
}

/// CE QUI REVIENT D'UN ENVOI, dans la forme de sa sorte : un compte rendu d'organisme (une DSN, une DPAE), un contrat
/// signe, une fiche recue. Les trois formes n'ont pas les memes cles exigees, et aucune n'accepte une cle de plus : le
/// retour se reconnait a sa forme, et `Tenu::replier` verifie qu'elle est celle de la sorte de son envoi.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Retour {
    Organisme(RetourOrganisme),
    Contrat(RetourContrat),
    Fiche(RetourFiche),
}

/// LE JOURNAL LU : ses lignes (ce qui est parti et ce qui en est revenu : bilans, contrats signes, anomalies), et ce qui
/// est parti de chaque engagement, sur quelle version (`departs`), la part que le moteur passe a l'engagement. Le
/// processus le tient replie (`Tenu`) : une lecture ne replie que les enregistrements ajoutes depuis la precedente, et
/// ce que le processus ajoute lui-meme se replie sans relire le fichier.
#[derive(Debug, Clone, Default)]
pub struct Journal {
    pub lignes: Arc<Vec<Ligne>>,
    pub departs: engagement::departs::Departs,
    /// L'`id` de chaque ligne, dans le meme ordre.
    ids: Arc<Vec<IdDEnvoi>>,
}

/// Le journal d'un dossier tel que le processus l'a replie : les octets lus jusqu'a la derniere fin de ligne, ce que
/// le systeme de fichiers en disait (taille, date ; `None` : le fichier n'existait pas), le journal, et l'indice de
/// chaque `id`.
#[derive(Default)]
struct Tenu {
    octets: Vec<u8>,
    vu: Option<(u64, std::time::SystemTime)>,
    journal: Journal,
    index: HashMap<IdDEnvoi, usize>,
}

impl Tenu {
    /// Un journal dont rien n'est encore lu : aucun octet, aucune ligne.
    fn rien_de_lu() -> Tenu {
        Tenu::default()
    }
}

type Tenus = Mutex<HashMap<PathBuf, Tenu>>;

fn tenus() -> std::sync::MutexGuard<'static, HashMap<PathBuf, Tenu>> {
    static TENUS: OnceLock<Tenus> = OnceLock::new();
    socle::memoire::tenu(TENUS.get_or_init(Default::default))
}

/// Ce que le systeme de fichiers dit du journal `p` (taille, date).
fn vu(meta: &std::fs::Metadata, p: &Path) -> R<(u64, std::time::SystemTime)> {
    Ok((meta.len(), meta.modified().map_err(|e| socle::echec::du_disque(&p, &e))?))
}

/// Une ligne de journal qui compte pour ce qui est parti malgre ses retours : un envoi au salarie (documents,
/// contrat) ne depend pas de ses retours ; une declaration refusee au bilan n'est pas partie.
fn depend_de_ses_retours(l: &Ligne) -> Option<bool> {
    matches!(l.acte(), Acte::Dsn | Acte::Dpae).then(|| crate::etats::refuse_par_un_retour(l))
}

/// La phrase d'un journal qui ne se lit pas, sa ligne nommee.
fn abime(n: usize) -> String {
    format!("Le journal des envois de la compagnie ne se lit pas : sa ligne {} est abîmée.", n)
}

/// Le refus du journal `p` dont la ligne `n` ne se lit pas : la phrase dit ce qui ne va pas (`quoi`), le chemin va a
/// part (`echec::du_disque`).
fn abime_a(p: &Path, n: usize, quoi: &str) -> Echec {
    socle::echec::du_disque(&p, &format!("Le journal des envois de la compagnie ne se lit pas : sa ligne {} est abîmée ({}).", n, quoi))
}

impl Tenu {
    /// Replie un enregistrement (`n` : sa ligne dans le fichier, pour le dire s'il ne se lit pas).
    fn replier(&mut self, e: Enregistrement, n: usize, p: &Path) -> R<()> {
        let illisible = |quoi: &str| abime_a(p, n, quoi);
        match e {
            Enregistrement { id: Some(id), ligne: Some(ligne), a: None, retour: None, nature_retour: None, publication: None, complete: None } => {
                if self.index.contains_key(&id) {
                    // Le meme envoi (son `id` est l'empreinte du texte qu'il a note), lu dans deux journaux reunis, ne compte
                    // qu'une fois ; ce qui lui est revenu sur chaque poste s'est range a la suite.
                    return Ok(());
                }
                noter_ligne(&mut self.journal.departs, &ligne);
                self.index.insert(id.clone(), self.journal.lignes.len());
                Arc::make_mut(&mut self.journal.ids).push(id);
                Arc::make_mut(&mut self.journal.lignes).push(ligne);
                Ok(())
            }
            Enregistrement { id: None, ligne: None, a: Some(a), retour, nature_retour: nature, publication, complete } => {
                let i = *self.index.get(&a).ok_or_else(|| illisible("ce qui revient d'un envoi que le journal ne porte pas"))?;
                let lignes = Arc::make_mut(&mut self.journal.lignes);
                let avant = depend_de_ses_retours(&lignes[i]);
                match (retour, nature, complete) {
                    (Some(r), None, None) if publication.is_none() => match (&mut lignes[i], r) {
                        // LE MEME RETOUR, RANGE SUR DEUX POSTES, NE COMPTE QU'UNE FOIS. Un compte rendu d'organisme se
                        // reconnait a sa nature et a sa publication (`etats::rattacher_retours` n'en range qu'un par envoi) :
                        // ce que l'autre poste en savait de plus (un verdict) le complete ; ce qui revient d'un envoi au
                        // salarie, a lui-meme.
                        (Ligne::Dsn(l), Retour::Organisme(r)) => ranger_organisme(&mut l.retours, r),
                        (Ligne::Dpae(l), Retour::Organisme(r)) => ranger_organisme(&mut l.retours, r),
                        (Ligne::ContratSignature(l), Retour::Contrat(r)) => ranger_une_fois(&mut l.retours, r),
                        (Ligne::FicheDemande(l), Retour::Fiche(r)) => ranger_une_fois(&mut l.retours, r),
                        _ => return Err(illisible("un retour qui n'a pas la forme de ce que son envoi reçoit")),
                    },
                    (None, Some(nature), Some(v)) => {
                        let retours = match &mut lignes[i] {
                            Ligne::Dsn(l) => &mut l.retours,
                            Ligne::Dpae(l) => &mut l.retours,
                            _ => return Err(illisible("le complément d'un retour, sur un envoi dont rien ne revient")),
                        };
                        let r = retours
                            .iter_mut()
                            .find(|r| r.nature_retour == nature && r.publication == publication)
                            .ok_or_else(|| illisible("le complément d'un retour que l'envoi ne porte pas"))?;
                        r.poser(v);
                    }
                    _ => return Err(illisible("ce qui revient d'un envoi n'y est ni un retour ni son complément")),
                }
                if depend_de_ses_retours(&lignes[i]) != avant {
                    self.journal.departs = departs_des_lignes(lignes);
                }
                Ok(())
            }
            _ => Err(illisible("ni un envoi, ni ce qui lui revient")),
        }
    }

    /// Replie les lignes completes de `octets` (le texte qui suit `self.octets`) ; une ligne que rien ne termine encore
    /// (un ajout en train de s'ecrire) attend la lecture suivante.
    fn replier_le_texte(&mut self, octets: &[u8], p: &Path) -> R<()> {
        let Some(fin) = octets.iter().rposition(|&o| o == b'\n') else { return Ok(()) };
        let mut n = self.octets.iter().filter(|&&o| o == b'\n').count();
        for texte in octets[..fin].split(|&o| o == b'\n') {
            n += 1;
            if texte.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let e = lire_enregistrement(texte, p, n)?;
            self.replier(e, n, p)?;
        }
        self.octets.extend_from_slice(&octets[..=fin]);
        Ok(())
    }
}

/// L'enregistrement de la ligne `n` du fichier `p`, lu strictement.
fn lire_enregistrement(texte: &[u8], p: &Path, n: usize) -> R<Enregistrement> {
    let texte = std::str::from_utf8(texte).map_err(|_| abime_a(p, n, "elle n'est pas du texte"))?;
    socle::forme::lire_texte(texte, &abime(n)).map_err(|e| e.au_fichier(p))
}

/// Un compte rendu d'organisme de plus, ou ce qu'il apporte au meme (sa nature, sa publication) deja range.
fn ranger_organisme(retours: &mut Vec<RetourOrganisme>, r: RetourOrganisme) {
    match retours.iter_mut().find(|x| x.meme(&r)) {
        Some(x) => x.completer(&r),
        None => retours.push(r),
    }
}

/// Un retour de plus, s'il n'est pas deja range.
fn ranger_une_fois<T: PartialEq>(retours: &mut Vec<T>, r: T) {
    if !retours.contains(&r) {
        retours.push(r);
    }
}

/// Le journal de `p` replie a jour de ce que porte le fichier ; vide (`vu` a `None`) s'il n'existe pas.
fn a_jour<'t>(tenus: &'t mut HashMap<PathBuf, Tenu>, p: &Path) -> R<&'t mut Tenu> {
    let meta = match std::fs::metadata(p) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let t = tenus.entry(p.to_path_buf()).or_default();
            *t = Tenu::default();
            return Ok(t);
        }
        Err(e) => return Err(socle::echec::du_disque(&p, &e)),
    };
    let vu = Some(vu(&meta, p)?);
    if tenus.get(p).is_some_and(|t| t.vu == vu) {
        return Ok(tenus.entry(p.to_path_buf()).or_default());
    }
    let octets = std::fs::read(p).map_err(|e| socle::echec::du_disque(&p, &e))?;
    // Ce qui a ete lu reste en tete du fichier, puisqu'on n'y fait qu'ajouter : seul le reste se replie. Un fichier
    // qui ne commence plus par ce qui a ete lu (un journal remplace) se replie en entier.
    let mut t = match tenus.remove(p) {
        Some(t) if octets.starts_with(&t.octets) => t,
        // Rien de lu, ou un journal remplace : il se relit depuis son debut.
        None | Some(_) => Tenu::rien_de_lu(),
    };
    let debut = t.octets.len();
    t.replier_le_texte(&octets[debut..], p)?;
    t.vu = vu;
    Ok(tenus.entry(p.to_path_buf()).insert_entry(t).into_mut())
}

/// `journal/depots.jsonl` lu (`Journal`) ; vide s'il n'existe pas.
pub fn lu(racine: &Path) -> R<Journal> {
    let p = chemin(racine);
    let mut tenus = tenus();
    let t = a_jour(&mut tenus, &p)?;
    if t.vu.is_some() {
        socle::fichiers::noter_si_besoin(&p);
    }
    Ok(t.journal.clone())
}

/// LE SEUL ENDROIT QUI ECRIT LE JOURNAL : les enregistrements que `faire` tire du journal a jour s'ajoutent a la fin du
/// fichier, d'une seule ecriture, et se replient tels qu'ils se reliront.
fn ajouter(racine: &Path, faire: impl FnOnce(&Journal, &HashMap<IdDEnvoi, usize>) -> R<Vec<Enregistrement>>) -> R<()> {
    let p = chemin(racine);
    let mut tenus = tenus();
    let t = a_jour(&mut tenus, &p)?;
    let enregistrements = faire(&t.journal, &t.index)?;
    if enregistrements.is_empty() {
        return Ok(());
    }
    let mut texte = String::new();
    for e in &enregistrements {
        texte.push_str(&socle::ecriture::json_en_ligne(e));
        texte.push('\n');
    }
    if let Some(dossier) = p.parent() {
        std::fs::create_dir_all(dossier).map_err(|e| socle::echec::du_disque(&dossier, &e))?;
    }
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&p).map_err(|e| socle::echec::du_disque(&p, &e))?;
        f.write_all(texte.as_bytes()).map_err(|e| socle::echec::du_disque(&p, &e))?;
    }
    let meta = std::fs::metadata(&p).map_err(|e| socle::echec::du_disque(&p, &e))?;
    // Ce que le processus tient se replie de ce qu'il vient d'ecrire ; si un autre processus a ajoute entre la lecture
    // et l'ecriture, le journal se relira du fichier.
    let t = tenus.remove(&p);
    if let Some(mut t) = t.filter(|t| t.octets.len() + texte.len() == meta.len() as usize) {
        t.replier_le_texte(texte.as_bytes(), &p)?;
        t.vu = Some(vu(&meta, &p)?);
        tenus.insert(p, t);
    }
    Ok(())
}

/// Ne garde du journal de `racine` que les envois que `garder` retient, et ce qui leur est revenu : le journal d'une
/// compagnie montee depuis une autre (`compagnie::essai`). Le fichier se remplace d'un coup ; sans journal, rien
/// n'est a garder.
pub fn ne_garder_que(racine: &Path, garder: impl Fn(&Ligne) -> bool) -> R<()> {
    let p = chemin(racine);
    let octets = match std::fs::read(&p) {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(socle::echec::du_disque(&p, &e)),
    };
    let mut gardes: std::collections::HashSet<IdDEnvoi> = Default::default();
    let mut texte = String::new();
    for (n, l) in octets.split(|&o| o == b'\n').enumerate() {
        if l.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let e = lire_enregistrement(l, &p, n + 1)?;
        let garde = match (&e.ligne, &e.id, &e.a) {
            (Some(ligne), Some(id), _) => garder(ligne) && gardes.insert(id.clone()),
            (_, _, Some(a)) => gardes.contains(a),
            (None, None, None) | (None, Some(_), None) | (Some(_), None, None) => false,
        };
        if garde {
            texte.push_str(std::str::from_utf8(l).map_err(|x| refus(x.to_string()))?);
            texte.push('\n');
        }
    }
    let provisoire = socle::ecriture::provisoire(&p);
    std::fs::write(&provisoire, texte).map_err(|e| socle::echec::du_disque(&provisoire, &e))?;
    std::fs::rename(&provisoire, &p).map_err(|e| socle::echec::du_disque(&p, &e))?;
    tenus().remove(&p);
    Ok(())
}

thread_local! {
    /// L'heure et l'origine que portent les lignes notees pendant `dater_pendant`.
    static DATE: std::cell::RefCell<Option<(Moment, String)>> = const { std::cell::RefCell::new(None) };
}

/// `f`, chaque ligne qu'il note portant `quand` et `origine` : un pas que l'horloge d'une compagnie fictive joue a sa
/// date (`routes::horloge`).
pub fn dater_pendant<T>(quand: Moment, origine: &str, f: impl FnOnce() -> T) -> T {
    let avant = DATE.with(|d| d.replace(Some((quand, origine.to_string()))));
    let r = f();
    DATE.with(|d| *d.borrow_mut() = avant);
    r
}

/// Les lignes de `lignes` d'une sorte (`quoi`), d'un mois, reelles seulement ; un filtre absent ne filtre pas.
pub fn filtrer<'a>(lignes: &'a [Ligne], quoi: Option<Acte>, mois: Option<Mois>, reel_seulement: bool) -> impl DoubleEndedIterator<Item = &'a Ligne> {
    lignes
        .iter()
        .filter(move |l| quoi.is_none_or(|q| l.acte() == q) && mois.is_none_or(|m| l.mois() == Some(m)) && (!reel_seulement || l.reel()))
}

/// Les lignes du journal de `racine`, les plus recentes en dernier (`filtrer`).
pub fn lire(racine: &Path, quoi: Option<Acte>, mois: Option<Mois>, reel_seulement: bool) -> R<Vec<Ligne>> {
    Ok(filtrer(&lu(racine)?.lignes, quoi, mois, reel_seulement).cloned().collect())
}

impl<D, R> Envoi<D, R> {
    /// Son heure, que `noter` a posee (`Datation`).
    pub fn quand(&self) -> InstantOuJour {
        self.quand
    }

    /// L'envoi que `qui` fait maintenant, dans `mode` : `noter` le date (`Datation`), et y lit le nom et l'empreinte
    /// du fichier.
    pub fn nouveau(mode: Mode, qui: &str, accepte: bool, destinataire: &str, reponse: &str, detail: D) -> Envoi<D, R> {
        Envoi {
            quand: socle::temps::secondes().a_la_seconde().into(),
            origine: String::new(),
            mois: None,
            fichier: None,
            empreinte: None,
            mode,
            qui: qui.to_string(),
            accepte,
            destinataire: destinataire.to_string(),
            flux: None,
            retours: Vec::new(),
            reponse: reponse.to_string(),
            trace: None,
            detail,
            versions: None,
        }
    }
}

/// CE QUI DATE UNE LIGNE QU'ON NOTE : rien (l'acte a lieu maintenant), ou une heure lue dans une piece avec cette
/// piece, ensemble (une reprise : la ligne dit d'ou vient sa date).
#[derive(Debug, Clone, PartialEq)]
pub enum Datation {
    /// L'heure de l'acte, l'instant present.
    Maintenant,
    /// L'heure que la piece `origine` dit.
    Lue { quand: InstantOuJour, origine: String },
}

/// AJOUTE LA LIGNE `ligne` A LA FIN DU FICHIER ET LA REND, telle qu'elle se relira. Son heure et son origine sont
/// celles de `dater_pendant`, sinon celles de `date` ; ce que la ligne portait avant ne compte pas. `fichier` : le
/// fichier parti, dont la ligne garde le nom et l'empreinte (s'il existe).
pub fn noter(racine: &Path, mut ligne: Ligne, fichier: Option<&Path>, date: Datation) -> R<Ligne> {
    let (q, o) = match DATE.with(|d| d.borrow().clone()) {
        Some((q, o)) => (q.into(), o),
        None => match date {
            Datation::Maintenant => (socle::temps::secondes().a_la_seconde().into(), String::new()),
            Datation::Lue { quand, origine } => (quand, origine),
        },
    };
    let (quand, origine) = commun_mut(&mut ligne);
    (*quand, *origine) = (q, o);
    if let Some(f) = fichier.filter(|f| !f.as_os_str().is_empty()) {
        let nom = socle::fichiers::NomDeFichier::du(f)?;
        let e = if f.exists() { Some(empreinte_fichier(f)?) } else { None };
        poser_fichier(&mut ligne, nom, e);
    }
    let ecrite = ligne.clone();
    ajouter(racine, |_, index| {
        // L'empreinte du texte de la ligne ; deux lignes au meme texte se distinguent par leur rang.
        let mut id = IdDEnvoi::premier(Empreinte::des_octets(socle::ecriture::json_en_ligne(&ligne).as_bytes()).courte());
        while index.contains_key(&id) {
            id = id.suivant();
        }
        Ok(vec![Enregistrement { id: Some(id), ligne: Some(ligne), ..Default::default() }])
    })?;
    Ok(ecrite)
}

/// L'heure et l'origine de la ligne, a poser.
fn commun_mut(l: &mut Ligne) -> (&mut InstantOuJour, &mut String) {
    match l {
        Ligne::Dsn(e) => (&mut e.quand, &mut e.origine),
        Ligne::Dpae(e) => (&mut e.quand, &mut e.origine),
        Ligne::Documents(e) => (&mut e.quand, &mut e.origine),
        Ligne::ContratSignature(e) => (&mut e.quand, &mut e.origine),
        Ligne::ContratContresigne(e) => (&mut e.quand, &mut e.origine),
        Ligne::RegularisationCasc(e) => (&mut e.quand, &mut e.origine),
        Ligne::DossierSalarie(e) => (&mut e.quand, &mut e.origine),
        Ligne::FicheDemande(e) => (&mut e.quand, &mut e.origine),
    }
}

fn poser_fichier(l: &mut Ligne, nom: socle::fichiers::NomDeFichier, e: Option<EmpreinteCourte>) {
    macro_rules! poser {
        ($x:ident) => {{
            $x.fichier = Some(nom);
            $x.empreinte = e;
        }};
    }
    match l {
        Ligne::Dsn(x) => poser!(x),
        Ligne::Dpae(x) => poser!(x),
        Ligne::Documents(x) => poser!(x),
        Ligne::ContratSignature(x) => poser!(x),
        Ligne::ContratContresigne(x) => poser!(x),
        Ligne::RegularisationCasc(x) => poser!(x),
        Ligne::DossierSalarie(x) => poser!(x),
        Ligne::FicheDemande(x) => poser!(x),
    }
}

/// Ce qui revient d'un envoi au salarie : un contrat signe (ou la mention retiree), une fiche recue (ou la demande
/// abandonnee).
pub enum Revenu {
    Contrat(RetourContrat),
    Fiche(RetourFiche),
}

/// CE QUI REVIENT D'UN ENVOI (un contrat signe, la mention retiree, une fiche) : `retour` se range dans les `retours` de
/// la derniere ligne que `envoi` reconnait, et la rend. `None` : aucune ligne ne la porte, rien n'est ecrit.
pub fn rattacher(racine: &Path, envoi: impl Fn(&Ligne) -> bool, retour: Revenu) -> R<Option<Ligne>> {
    let mut trouvee = None;
    ajouter(racine, |j, _| {
        trouvee = j.lignes.iter().rposition(|l| envoi(l));
        let ranger = |i: usize| -> R<Enregistrement> {
            let sien = matches!((&j.lignes[i], &retour), (Ligne::ContratSignature(_), Revenu::Contrat(_)) | (Ligne::FicheDemande(_), Revenu::Fiche(_)));
            if !sien {
                return Err(socle::refus!("journal : ce retour ne revient pas d'un envoi {}", j.lignes[i].acte().nom()));
            }
            let r = match &retour {
                Revenu::Contrat(r) => Retour::Contrat(r.clone()),
                Revenu::Fiche(r) => Retour::Fiche(r.clone()),
            };
            Ok(Enregistrement { a: Some(j.ids[i].clone()), retour: Some(r), ..Default::default() })
        };
        // Aucune ligne ne le porte : rien ne s'ecrit.
        Ok(trouvee.map(ranger).transpose()?.into_iter().collect())
    })?;
    Ok(match trouvee {
        Some(i) => lu(racine)?.lignes.get(i).cloned(),
        None => None,
    })
}

/// Ce qui se range sur la ligne d'une DSN ou d'une DPAE (`ranger`).
pub enum Rangement {
    /// Un compte rendu de plus.
    Retour(RetourOrganisme),
    /// Le verdict du compte rendu de cette nature et de cette publication.
    Complement { nature_retour: crate::retours::NatureDuRetour, publication: Option<socle::paris::Moment>, verdict: VerdictDeLaPiece },
}

/// RANGE CE QUI REVIENT des envois : chaque `(i, rangement)` sur la `i`-ieme ligne du journal lu (`lu`), dans l'ordre.
pub fn ranger(racine: &Path, rangements: Vec<(usize, Rangement)>) -> R<()> {
    ajouter(racine, |j, _| {
        rangements
            .into_iter()
            .map(|(i, r)| {
                let a = Some(j.ids.get(i).ok_or_else(|| socle::refus!("journal : aucune ligne {}", i))?.clone());
                Ok(match r {
                    Rangement::Retour(retour) => Enregistrement { a, retour: Some(Retour::Organisme(retour)), ..Default::default() },
                    Rangement::Complement { nature_retour, publication, verdict } => Enregistrement { a, nature_retour: Some(nature_retour), publication, complete: Some(verdict), ..Default::default() },
                })
            })
            .collect()
    })
}

/// Ce que les lignes `lignes` du journal disent de ce qui est parti (`departs`) : les lignes qui precedent un envoi
/// disent ce qui etait parti avant lui.
pub fn departs_des_lignes(lignes: &[Ligne]) -> engagement::departs::Departs {
    let mut d = engagement::departs::Departs::default();
    for l in lignes {
        noter_ligne(&mut d, l);
    }
    d
}

/// Ce que la ligne `l` dit de ce qui est parti, a la suite de `d`.
fn noter_ligne(d: &mut engagement::departs::Departs, l: &Ligne) {
    use engagement::departs::{Depart, Envoi};
    let envoi = match l.acte() {
        Acte::ContratSignature => Envoi::Contrat,
        Acte::Dpae => Envoi::Dpae,
        Acte::Documents => Envoi::Documents,
        Acte::Dsn if l.signalement() => Envoi::Signalement,
        Acte::Dsn => Envoi::Dsn,
        _ => return,
    };
    if !l.reel() {
        return;
    }
    // Une declaration refusee au depot ou au bilan n'a rien depose (`etats::refuse_par_un_retour`). Un courriel coupe en
    // route a servi ceux qui l'ont recu : la ligne ne porte que leurs versions (`ligne_d_envoi::noter_echec_d_envoi`),
    // et ce qui a atteint le salarie est parti.
    let au_salarie = matches!(envoi, Envoi::Documents | Envoi::Contrat);
    if !au_salarie && (!l.accepte() || crate::etats::refuse_par_un_retour(l)) {
        return;
    }
    let mois = matches!(envoi, Envoi::Documents | Envoi::Dsn).then(|| l.mois()).flatten();
    let le = l.quand();
    for v in l.versions().into_iter().flatten() {
        // La correction d'un mois passe que la DSN porte : sa version est celle de l'engagement du mois corrige.
        let corrige = (envoi == Envoi::Dsn).then_some(v.mois).flatten().filter(|m| Some(*m) != mois);
        let (m, par) = match corrige {
            Some(c) => (Some(c), mois),
            None => (mois, None),
        };
        // Un avenant : la ligne du journal le dit (`acte`), et son contrat porte son numero.
        let avenant = l
            .signature()
            .filter(|s| s.detail.acte.as_deref() == Some("avenant"))
            .and_then(|s| s.detail.contrats.iter().find(|c| c.salarie == v.salarie && c.numero.as_ref() == Some(&v.numero)))
            .and_then(|c| c.avenant)
            .map(|a| a.to_string());
        d.noter(&v.salarie, &v.numero, Depart { envoi, mois: m, par, avenant, version: v.version, le });
    }
}

/// LES VERSIONS QU'UN ENVOI EMPORTE : pour chaque engagement valide que `mois` montre et que `garder` retient (la cle du
/// salarie, le numero du contrat, l'engagement), sa version. Ce que la ligne du journal de l'envoi porte : elles se lisent
/// AVANT l'envoi, qui ne part pas d'un engagement a revalider (`Valide::a_revalider`).
pub fn versions_du_mois(ctx: &droit::contexte::Contexte, mois: Mois, departs: &engagement::departs::Departs, garder: impl Fn(&droit::contexte::IdSalarie, &str, &engagement::stock::Ecrit) -> bool) -> R<Vec<Version>> {
    let mut out = Vec::new();
    for e in engagement::etats::du_mois(ctx, mois, departs)? {
        let Some(v) = e.valide() else { continue };
        let brut = &*v.objet().brut;
        let (Some(cle), Some(numero)) = (&brut.saisie.carte.salarie, &brut.saisie.carte.numero) else { continue };
        if garder(cle, numero, brut) {
            if let Some(phrase) = v.a_revalider(ctx) {
                return Err(refus(phrase));
            }
            let version = v.version().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "version"))?;
            out.push(Version { salarie: cle.clone(), numero: numero.clone(), version, mois: None });
        }
    }
    Ok(out)
}

/// `versions_du_mois` des contrats qui partent, chacun lu dans le mois qu'il nomme.
pub fn versions_des_contrats(ctx: &droit::contexte::Contexte, contrats: &[ContratParti], departs: &engagement::departs::Departs) -> R<Vec<Version>> {
    let mut mois: Vec<Mois> = contrats.iter().map(|c| c.mois).collect();
    mois.sort();
    mois.dedup();
    let mut out: Vec<Version> = Vec::new();
    for m in mois {
        let siens = |cle: &droit::contexte::IdSalarie, n: &str, _: &engagement::stock::Ecrit| contrats.iter().any(|c| c.mois == m && c.salarie == *cle && c.numero.as_deref() == Some(n));
        for x in versions_du_mois(ctx, m, departs, siens)? {
            if !out.contains(&x) {
                out.push(x);
            }
        }
    }
    Ok(out)
}

/// Le plus grand numero d'ordre (`S20.G00.05.004`) des signalements REELS deja envoyes par la compagnie, tous
/// evenements confondus (arret, reprise, fin de contrat, amorcage) ; aucun s'il n'y en a pas.
///
/// Cahier technique DSN 2026.1, p. 136 : « Pour un signalement, ce numero ne doit pas etre remis a zero afin
/// qu'il soit unique quel que soit le mois d'emission » ; p. 14 : un second signalement normal au meme SIRET et
/// au meme numero d'ordre n'est pas accepte. Le numero se LIT de ce qui est parti (`detail.ordre` des lignes
/// `dsn` reelles du journal), il ne se devine pas : une ligne reelle acceptee qui ne dit pas son ordre refuse.
/// Un signalement se reconnait a `detail.nature` (04, 05, 07, 08) ; une ligne reprise des retours, qui ne la porte pas,
/// a son mois : le bilan d'une mensuelle le dit, celui d'un signalement non. Une ligne non acceptee (le point de depot
/// n'a rien recu) ne consomme pas de numero.
pub fn dernier_ordre_des_signalements(racine: &Path) -> R<Option<droit::natures::NumeroDOrdre>> {
    let mut dernier = None;
    for l in lire(racine, Some(Acte::Dsn), None, true)? {
        let Some(e) = l.dsn() else { continue };
        let signalement = match e.detail.nature_declaration {
            Some(n) => n.signalement_envoye(),
            None => e.mois.is_none(),
        };
        if !signalement {
            continue;
        }
        let sans_ordre = || {
            socle::echec::refus_avec(
                format!(
                    "Le journal des envois note le signalement reel du {} sans son numero d'ordre, lu en S20.G00.05.004 du fichier envoye. Le numero d'ordre d'un signalement ne se remet jamais a zero (cahier technique DSN, p. 136) : sans lui, le suivant ne se numerote pas. Le signaler au developpeur. Rien n'est ecrit.",
                    e.quand,
                ),
                socle::echec::Lieu { chemin: Some(chemin(racine).to_string_lossy().into_owned()), fichier: e.fichier.as_ref().map(|f| f.texte().to_string()), cle: Some("detail.ordre".to_string()), detail: None },
            )
        };
        match e.detail.ordre {
            Some(o) => dernier = dernier.max(Some(o)),
            None if e.accepte => return Err(sans_ordre()),
            None => {}
        }
    }
    Ok(dernier)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bug reel : une DSN ou une DPAE refusee au bilan comptait pour partie ; le mois se figeait et sa correction allait a
    /// la DSN suivante. Rien n'a ete recu (cahier technique DSN, § 4.2, `norme-dsn/texte/chapitre-4-controles.txt`).
    #[test]
    fn une_declaration_refusee_au_bilan_n_est_pas_partie() {
        use crate::retours::NatureDuRetour::{AccuseDEnregistrement, CertificatDeConformite};
        use crate::retours::Verdict;
        let retour = |nature: crate::retours::NatureDuRetour, verdict: Verdict| RetourOrganisme { nature_retour: nature, verdict: Some(verdict), anomalies: None, dpae: None, libelle: String::new(), statut: None, publication: None };
        fn envoi<D>(salarie: &str, retours: Vec<RetourOrganisme>, detail: D) -> Envoi<D, RetourOrganisme> {
            let mut e = Envoi::nouveau(Mode::Reel, "t", true, "", "", detail);
            e.mois = Some(Mois::nouveau(2026, 6).unwrap());
            e.quand = socle::paris::Moment::a_paris(socle::date::Jour::nouveau(2026, 7, 5).unwrap(), 10, 0, 0).unwrap().into();
            e.retours = retours;
            e.versions = Some(vec![Version { salarie: droit::contexte::IdSalarie::lire(salarie).unwrap(), numero: engagement::numeros::NumeroContrat::lire("DE1.26.01").unwrap(), version: 1, mois: None }]);
            e
        }
        let ko = vec![retour(AccuseDEnregistrement, Verdict::Ok), retour(CertificatDeConformite, Verdict::Ko)];
        let lignes = vec![
            Ligne::Dsn(envoi("ALPHA", ko.clone(), DetailDsn::nouveau(false))),
            Ligne::Dsn(envoi("BETA", vec![], DetailDsn { etat_declaration: Some(Verdict::Ko), ..DetailDsn::nouveau(true) })),
            Ligne::Dpae(envoi("GAMMA", ko, DetailDpae::nouveau(false, false))),
            Ligne::Dsn(envoi("DELTA", vec![retour(CertificatDeConformite, Verdict::Ok)], DetailDsn::nouveau(false))),
        ];
        let d = departs_des_lignes(&lignes);
        let numero = engagement::numeros::NumeroContrat::lire("DE1.26.01").unwrap();
        for refusee in ["ALPHA", "BETA", "GAMMA"] {
            assert!(d.de(&droit::contexte::IdSalarie::lire(refusee).unwrap(), &numero).is_empty(), "{refusee}");
        }
        assert_eq!(d.de(&droit::contexte::IdSalarie::lire("DELTA").unwrap(), &numero).len(), 1);
    }
}
