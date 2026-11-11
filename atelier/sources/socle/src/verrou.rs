//! Le verrou des compteurs : le fichier `config/.compteurs.verrou` qui exclut deux attributions de numeros
//! simultanees ; son contenu dit qui le tient (`QUI pid N JJ/MM/AAAA HH:MM:SS`).
//!
//! Un verrou est un fichier cree en exclusif ; passe `delai` secondes le
//! detenteur est tenu pour mort et on reprend la main ; le sortant ne retire
//! que SA marque.

use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::echec::R;

/// Au-dela, un verrou est tenu pour abandonne (10 minutes).
pub const DELAI_VERROU: i64 = 600;

/// Le verrou est tenu par un autre : sa marque et son age en secondes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verrouille {
    pub chemin: PathBuf,
    pub detenteur: String,
    pub age: i64,
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

/// `config/.compteurs.verrou` d'une racine de donnees.
pub fn chemin_compteurs(racine: &Path) -> PathBuf {
    racine.join(crate::fichiers::CONFIG).join(".compteurs.verrou")
}

/// La marque que le detenteur ecrit : `QUI pid N JJ/MM/AAAA HH:MM:SS`.
pub fn marque(qui: &str, pid: u32, maintenant: &str) -> String {
    format!("{} pid {} {}", qui, pid, maintenant)
}

fn age_secondes(chemin: &Path) -> std::io::Result<i64> {
    let modifie = std::fs::metadata(chemin)?.modified()?;
    // Les secondes entieres ecoulees, tronquees vers zero (un fichier date du futur a un age negatif).
    Ok(match SystemTime::now().duration_since(modifie) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    })
}

/// Prend le verrou, ou rend qui le tient. Un verrou plus vieux que `delai`
/// est leve, une fois, puis on retente. Une erreur d'ecriture (dossier
/// absent...) est une erreur, pas un verrou tenu.
pub fn prendre(chemin: &Path, marque: &str, delai: i64) -> R<Result<(), Verrouille>> {
    let disque = |e: std::io::Error| crate::echec::du_disque(&chemin, &e);
    let mut leve = false;
    loop {
        match OpenOptions::new().write(true).create_new(true).open(chemin) {
            Ok(mut f) => {
                f.write_all(marque.as_bytes()).map_err(disque)?;
                return Ok(Ok(()));
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let lu = age_secondes(chemin).and_then(|age| std::fs::read(chemin).map(|b| (age, String::from_utf8_lossy(&b).trim().to_string())));
                let (age, detenteur) = match lu {
                    Ok(l) => l,
                    // Disparu entre-temps : on retente.
                    Err(e) if e.kind() == ErrorKind::NotFound => continue,
                    Err(e) => return Err(disque(e)),
                };
                if age < delai || leve {
                    return Ok(Err(Verrouille { chemin: chemin.to_path_buf(), detenteur, age }));
                }
                crate::fichiers::effacer(chemin)?;
                leve = true;
            }
            Err(e) => return Err(disque(e)),
        }
    }
}

/// Retire le verrou s'il porte encore NOTRE marque : si le delai a joue, un
/// autre l'a peut-etre repris entre-temps (ou deja retire).
pub fn rendre(chemin: &Path, marque: &str) -> R<()> {
    match std::fs::read(chemin) {
        Ok(b) if String::from_utf8_lossy(&b).trim() == marque => crate::fichiers::effacer(chemin),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(crate::echec::du_disque(&chemin, &e)),
    }
}

/// Un verrou tenu : rendu a la sortie de la portee.
pub struct Verrou {
    chemin: PathBuf,
    marque: String,
}

impl Verrou {
    pub fn prendre(chemin: &Path, marque: &str, delai: i64) -> R<Result<Verrou, Verrouille>> {
        Ok(prendre(chemin, marque, delai)?.map(|_| Verrou { chemin: chemin.to_path_buf(), marque: marque.to_string() }))
    }
}

impl Drop for Verrou {
    fn drop(&mut self) {
        // A la sortie de sa portee, rien ne remonte : un verrou qui ne se retire pas est note a la naissance de son echec
        // (`echec::noter_echec`, que les incidents lisent), et le delai le levera.
        if let Err(note) = rendre(&self.chemin, &self.marque) {
            std::mem::drop(note);
        }
    }
}

