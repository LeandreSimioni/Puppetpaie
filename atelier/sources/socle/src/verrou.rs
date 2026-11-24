//! Le verrou des compteurs : le fichier `config/.compteurs.verrou` qui exclut deux attributions de numeros
//! simultanees ; son contenu dit qui le tient (`QUI pid N JJ/MM/AAAA HH:MM:SS`).
//!
//! Un verrou est un fichier cree en exclusif ; passe `delai` secondes le
//! detenteur est tenu pour mort et on reprend la main ; le sortant ne retire
//! que SA marque.

use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::echec::R;

/// Au-dela, un verrou est tenu pour abandonne (10 minutes) : une duree mesuree.
pub const DELAI_VERROU: Duration = Duration::from_secs(600);

/// Le verrou est tenu par un autre : sa marque et son age.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verrouille {
    pub chemin: PathBuf,
    pub detenteur: String,
    age: AgeDuVerrou,
}

/// L'AGE D'UN VERROU, une duree mesuree depuis la date de son fichier (`SystemTime::duration_since`) : ecoulee, ou, pour
/// un fichier date du futur (une horloge reculee), l'avance de sa date sur la montre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgeDuVerrou {
    Ecoule(Duration),
    DateDuFutur(Duration),
}

impl AgeDuVerrou {
    /// Le verrou tient-il encore, a ce `delai` ? Compte en secondes entieres, tronquees : un fichier date du futur d'une
    /// seconde au moins tient toujours ; de moins d'une seconde, il a zero seconde.
    fn tient(self, delai: Duration) -> bool {
        match self {
            AgeDuVerrou::Ecoule(d) => d.as_secs() < delai.as_secs(),
            AgeDuVerrou::DateDuFutur(d) => d.as_secs() > 0 || delai.as_secs() > 0,
        }
    }
}

/// En secondes entieres, tronquees, precedees de `-` pour une date du futur (« 12 », « -3 »).
impl std::fmt::Display for AgeDuVerrou {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgeDuVerrou::Ecoule(d) => write!(f, "{}", d.as_secs()),
            AgeDuVerrou::DateDuFutur(d) if d.as_secs() == 0 => f.write_str("0"),
            AgeDuVerrou::DateDuFutur(d) => write!(f, "-{}", d.as_secs()),
        }
    }
}

impl Verrouille {
    /// La phrase de `CompteursVerrouilles`.
    pub fn message(&self) -> String {
        format!(
            "Une autre production tient les compteurs depuis {} s ({}).\nAttendre qu'elle finisse. Si elle a ete interrompue, effacer :\n  {}",
            self.age,
            self.detenteur,
            self.chemin.display()
        )
    }
}

/// Le verrou des compteurs, sous la racine de donnees.
pub const COMPTEURS: crate::fichiers::CheminRelatif = crate::fichiers::CheminRelatif::du_moteur("config/.compteurs.verrou");

/// `config/.compteurs.verrou` d'une racine de donnees.
pub fn chemin_compteurs(racine: &Path) -> PathBuf {
    COMPTEURS.dans(racine)
}

/// La marque que le detenteur ecrit : `QUI pid N JJ/MM/AAAA HH:MM:SS`.
pub fn marque(qui: &str, pid: u32, maintenant: &str) -> String {
    format!("{} pid {} {}", qui, pid, maintenant)
}

fn age_du_verrou(chemin: &Path) -> std::io::Result<AgeDuVerrou> {
    let modifie = std::fs::metadata(chemin)?.modified()?;
    Ok(match SystemTime::now().duration_since(modifie) {
        Ok(d) => AgeDuVerrou::Ecoule(d),
        Err(e) => AgeDuVerrou::DateDuFutur(e.duration()),
    })
}

/// Le verrou tel que le disque le dit : son age et sa marque ; aucun s'il n'est pas sur le disque.
fn lu(chemin: &Path) -> std::io::Result<Option<(AgeDuVerrou, String)>> {
    match age_du_verrou(chemin).and_then(|age| std::fs::read(chemin).map(|b| (age, String::from_utf8_lossy(&b).trim().to_string()))) {
        Ok(l) => Ok(Some(l)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Prend le verrou, ou rend qui le tient. Un verrou plus vieux que `delai`
/// est leve, une fois (`lever`), puis on retente. Une erreur d'ecriture (dossier
/// absent...) est une erreur, pas un verrou tenu.
pub fn prendre(chemin: &Path, marque: &str, delai: Duration) -> R<Result<(), Verrouille>> {
    let disque = |e: std::io::Error| crate::echec::du_disque(&chemin, &e);
    let mut leve = false;
    loop {
        match OpenOptions::new().write(true).create_new(true).open(chemin) {
            Ok(mut f) => {
                f.write_all(marque.as_bytes()).map_err(disque)?;
                return Ok(Ok(()));
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let (age, detenteur) = match lu(chemin).map_err(disque)? {
                    Some(l) => l,
                    // Disparu entre-temps : on retente.
                    None => continue,
                };
                if age.tient(delai) || leve {
                    return Ok(Err(Verrouille { chemin: chemin.to_path_buf(), detenteur, age }));
                }
                lever(chemin, &detenteur)?;
                leve = true;
            }
            Err(e) => return Err(disque(e)),
        }
    }
}

/// Le detenteur d'un verrou qui n'est plus sur le disque quand son preneur le rafraichit (`Verrou::rafraichir`).
const LEVE_PAR_UN_AUTRE: &str = "un autre preneur, qui le lève";

/// Le numero de la prochaine levee dans ce processus : il entre dans le nom ou le verrou leve est mis a part.
static LEVEES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// LEVER LE VERROU ABANDONNE QUI PORTE `abandonne`, et lui seul : deux preneurs qui l'ont juge abandonne ensemble ne le
/// levent pas deux fois. Le verrou est d'abord mis a part sous un nom a ce preneur (un renommage : un seul preneur
/// emporte un meme fichier), puis relu : c'est celui qu'on a juge abandonne, il s'efface ; c'en est un autre (un
/// preneur l'a leve et repris juste avant), il retrouve sa place, et ce preneur-ci n'a rien leve. Disparu avant d'etre
/// mis a part : un autre l'a leve.
fn lever(chemin: &Path, abandonne: &str) -> R<()> {
    let mut nom = chemin.as_os_str().to_os_string();
    nom.push(format!(".leve-{}-{}", crate::processus::numero(), LEVEES.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
    let a_part = PathBuf::from(nom);
    match std::fs::rename(chemin, &a_part) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(crate::echec::du_disque(chemin, &e)),
    }
    let relu = std::fs::read(&a_part).map_err(|e| crate::echec::du_disque(&a_part, &e))?;
    if String::from_utf8_lossy(&relu).trim() == abandonne {
        return crate::fichiers::effacer(&a_part);
    }
    std::fs::rename(&a_part, chemin).map_err(|e| crate::echec::du_disque(chemin, &e))
}

/// LE VERROU EST-IL TENU, sans le prendre : par qui, s'il est sur le disque et ne passe pas pour abandonne a ce `delai` ;
/// aucun sinon (un verrou abandonne se levera a la prochaine prise).
pub fn tenu(chemin: &Path, delai: Duration) -> R<Option<Verrouille>> {
    Ok(lu(chemin).map_err(|e| crate::echec::du_disque(chemin, &e))?.filter(|(age, _)| age.tient(delai)).map(|(age, detenteur)| Verrouille { chemin: chemin.to_path_buf(), detenteur, age }))
}

/// Retire le verrou s'il porte encore NOTRE marque : si le delai a joue, un
/// autre l'a peut-etre repris entre-temps (ou deja retire).
pub fn rendre(chemin: &Path, marque: &str) -> R<()> {
    rendre_au_disque(chemin, marque).map_err(|e| crate::echec::du_disque(&chemin, &e))
}

/// `rendre`, tel que le disque le dit.
fn rendre_au_disque(chemin: &Path, marque: &str) -> std::io::Result<()> {
    match std::fs::read(chemin) {
        Ok(b) if String::from_utf8_lossy(&b).trim() == marque => crate::fichiers::effacer_du_disque(chemin),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Un verrou tenu : rendu a la sortie de la portee.
pub struct Verrou {
    chemin: PathBuf,
    marque: String,
}

impl Verrou {
    pub fn prendre(chemin: &Path, marque: &str, delai: Duration) -> R<Result<Verrou, Verrouille>> {
        Ok(prendre(chemin, marque, delai)?.map(|_| Verrou { chemin: chemin.to_path_buf(), marque: marque.to_string() }))
    }

    /// UN LONG PASSAGE DIT QU'IL VIT : la date du fichier redevient maintenant, et son age repart de zero ; un passage
    /// plus long que le delai ne passe donc pas pour abandonne tant qu'il se rafraichit. Un verrou qui ne porte plus
    /// notre marque (le delai a joue, un autre l'a repris) le dit, tenu par l'autre (`Verrouille`) : le passage s'arrete,
    /// l'autre le fait. Un verrou qui n'est plus sur le disque est en train d'etre leve par un autre preneur (`lever`) :
    /// il n'est plus le notre non plus.
    pub fn rafraichir(&self) -> R<Result<(), Verrouille>> {
        let disque = |e: std::io::Error| crate::echec::du_disque(&self.chemin, &e);
        let (age, detenteur) = match lu(&self.chemin).map_err(disque)? {
            Some(l) => l,
            None => (AgeDuVerrou::Ecoule(Duration::ZERO), LEVE_PAR_UN_AUTRE.to_string()),
        };
        if detenteur != self.marque {
            return Ok(Err(Verrouille { chemin: self.chemin.clone(), detenteur, age }));
        }
        OpenOptions::new().write(true).open(&self.chemin).and_then(|f| f.set_modified(SystemTime::now())).map_err(disque)?;
        Ok(Ok(()))
    }
}

impl Drop for Verrou {
    fn drop(&mut self) {
        // A la sortie de sa portee, rien ne remonte : un verrou qui ne se retire pas se note (`echec::noter_echec`, que les
        // incidents lisent), et le delai le levera.
        if let Err(e) = rendre_au_disque(&self.chemin, &self.marque) {
            crate::echec::noter_echec(Some(crate::echec::Classe::du_disque(&e)), &format!("Un verrou ne se retire pas ({}) : {}", self.chemin.display(), e));
        }
    }
}

