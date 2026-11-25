//! « PRENDRE LA MAIN » RECUPERE LE DRIVE (Leandre, 25/11/2026 : « on lui dit des changements ont ete faits ailleurs que
//! sur ce poste. Prendre la main va effacer tous les changements effectues. Etes-vous sur ? Et s'il dit oui, et ben ca
//! efface tout. […] Je veux pas de double copie. ») : la compagnie de ce poste devient ce qu'il y a sur le Drive. Ce
//! qu'elle a ecrit sans la main s'efface avec elle, sans copie de cote ; seul ce qui est propre au poste reste.
//!
//! CE QU'ELLE FAIT, dans cet ordre :
//! 1. le Drive revient dans un dossier cache a cote de la compagnie (`.recuperee-<dossier>`, que `installation::est_cache`
//!    ecarte de toute liste), par le chemin de « Reprendre » (`restauration::restaurer`, sous son verrou) ; la compagnie
//!    n'est pas touchee. Pas tout revenu : elle reste telle quelle, le dossier cache garde ce qui est revenu, et la
//!    recuperation suivante le continue ;
//! 2. ce qui est propre au poste y passe depuis la compagnie (`PROPRES_AU_POSTE`, et les coffres mis de cote) : le coffre
//!    et ses mots de passe restent ceux de ce poste (« Le coffre reste celui du poste »), comme le jeton Google, le
//!    diagnostic, le journal des incidents et celui de l'interface ;
//! 3. son index dit encore la main ailleurs (`Index::sans_la_main`) : tant que la main n'est pas notee sur le Drive, la
//!    compagnie revenue ne se sauvegarde pas, et l'appelant l'efface une fois la main prise ;
//! 4. les deux dossiers s'echangent : la compagnie devient `.remplacee-<dossier>`, le dossier cache prend sa place, puis
//!    l'ancienne s'efface. Le second renommage refuse : le premier se defait, rien n'a bouge. Coupe entre les deux
//!    (le logiciel tue, une panne), le lancement suivant remet l'ancienne a sa place (`reparer_un_echange_coupe`).

use std::path::{Path, PathBuf};

use envois::drive::{Drive, ErreurGoogle};
use socle::echec::{Classe, Echec, R};
use socle::fichiers::CheminRelatif;

use crate::restauration::{est_un_coffre_de_cote, restaurer, Restauration, VerrouDeRestauration};
use crate::sauvegarde::{ecrire_index, lire_index, Index, SansLaMain};
use crate::sur_le_drive::DossierVu;

/// Le dossier cache ou le Drive revient, a cote de la compagnie (`.recuperee-<dossier>`).
pub const PREFIXE_RECUPEREE: &str = ".recuperee-";
/// L'ancienne compagnie, le temps de l'echange (`.remplacee-<dossier>`).
pub const PREFIXE_REMPLACEE: &str = ".remplacee-";

/// CE QUI EST PROPRE AU POSTE ET PASSE DE LA COMPAGNIE A CELLE QUI REVIENT : rien de cela ne monte au Drive
/// (`sauvegarde::chemins_exclus`), sauf le coffre, qui reste celui de ce poste. L'index, la trace de l'ouverture et l'etat
/// garde sont ceux de la compagnie qui revient : ils ne passent pas.
const PROPRES_AU_POSTE: [CheminRelatif; 5] = [
    socle::fichiers::JETON_GOOGLE,
    crate::incidents::DIAGNOSTIC,
    crate::incidents::INCIDENTS,
    crate::installation::JOURNAL_INTERFACE,
    CheminRelatif::du_moteur("config/coffre.bin"),
];

/// CE QUE LA RECUPERATION A FAIT (`recuperer`).
#[derive(Debug)]
pub enum Recuperation {
    /// Pas tout revenu : la compagnie n'a pas bouge ; le dossier cache garde ce qui est revenu, la suivante le continue.
    PasAuBout(Restauration),
    /// Tout est revenu, mais les dossiers ne s'echangent pas (un fichier tenu, le disque) : la compagnie n'a pas bouge, et
    /// le refus le dit.
    PasEchangee(Echec),
    /// La compagnie est celle du Drive. `reste` : l'ancienne ne s'est pas effacee entiere ; cachee, elle part au
    /// lancement suivant (sa phrase).
    Echangee { incidents: Vec<String>, reste: Option<String> },
}

/// Le dossier `prefixe<nom>` a cote de la compagnie `racine`.
fn a_cote(racine: &Path, prefixe: &str) -> R<PathBuf> {
    let nom = crate::installation::nom_dossier(racine)?;
    let parent = racine.parent().ok_or_else(|| socle::echec::refus("Le dossier de la compagnie n'a pas de parent."))?;
    Ok(parent.join(format!("{}{}", prefixe, nom)))
}

/// UNE RECUPERATION COMMENCEE SUR CE POSTE ET PAS FINIE : son dossier cache est la ; « Prendre la main » la continue.
pub fn commencee(racine: &Path) -> R<bool> {
    socle::fichiers::present(&a_cote(racine, PREFIXE_RECUPEREE)?)
}

/// Les coffres mis de cote sous `config/` de `racine` (`restauration::ranger_le_coffre`).
fn coffres_de_cote(racine: &Path) -> R<Vec<CheminRelatif>> {
    let config = racine.join(socle::fichiers::CONFIG.texte());
    let entrees = match std::fs::read_dir(&config) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(socle::echec::du_disque(&config, &e)),
    };
    let mut v = Vec::new();
    for e in entrees {
        let e = e.map_err(|x| socle::echec::du_disque(&config, &x))?;
        let Ok(nom) = socle::fichiers::NomDeFichier::lire(&e.file_name().to_string_lossy()) else { continue };
        let rel = CheminRelatif::from(socle::fichiers::CONFIG).joint(&nom);
        if est_un_coffre_de_cote(&rel) {
            v.push(rel);
        }
    }
    Ok(v)
}

/// Ce qui est propre au poste dans `racine` : la liste fixe, et ses coffres mis de cote.
fn propres(racine: &Path) -> R<Vec<CheminRelatif>> {
    let mut v = PROPRES_AU_POSTE.to_vec();
    v.extend(coffres_de_cote(racine)?);
    Ok(v)
}

/// RETIRE DE `cache` ce qui est propre au poste (une recuperation coupee l'y avait peut-etre deja passe) : la
/// restauration ne compte pas comme un ecart le coffre de ce poste, et le reprend du Drive avant qu'il soit remplace.
fn retirer_ce_qui_est_au_poste(cache: &Path) -> R<()> {
    for rel in propres(cache)? {
        socle::fichiers::effacer(&rel.dans(cache))?;
    }
    Ok(())
}

/// PASSE DE `depuis` A `vers` ce qui est propre au poste : chacun remplace celui de `vers` ; absent de `depuis`, il n'est
/// pas dans `vers` non plus (un coffre que ce poste n'avait pas ne vient pas de l'autre).
fn passer_ce_qui_est_au_poste(depuis: &Path, vers: &Path) -> R<()> {
    retirer_ce_qui_est_au_poste(vers)?;
    for rel in propres(depuis)? {
        let source = rel.dans(depuis);
        if !socle::fichiers::present(&source)? {
            continue;
        }
        let cible = rel.dans(vers);
        if let Some(d) = cible.parent() {
            std::fs::create_dir_all(d).map_err(|e| socle::echec::du_disque(d, &e))?;
        }
        std::fs::copy(&source, &cible).map_err(|e| socle::echec::du_disque(&cible, &e))?;
    }
    Ok(())
}

/// RECUPERE DANS LA COMPAGNIE `racine` CE QUE PORTE SON DOSSIER `vu` DU DRIVE (sous `racine_drive`), ce poste n'ayant
/// pas la main (`sans`, ce que son index en dira tant que l'appelant ne l'a pas prise). L'appelant a ferme la compagnie
/// (sa veille, ses envois en fond) : l'echange la remplace sous elle. Un `Err` : rien n'a bouge (la restauration refuse,
/// le disque refuse avant l'echange).
pub fn recuperer(racine: &Path, vu: &DossierVu, racine_drive: &str, client: &mut dyn Drive, sans: SansLaMain) -> Result<Recuperation, ErreurGoogle> {
    let cache = a_cote(racine, PREFIXE_RECUPEREE)?;
    std::fs::create_dir_all(&cache).map_err(|e| ErreurGoogle::from(socle::echec::du_disque(&cache, &e)))?;
    let r = {
        let verrou = VerrouDeRestauration::prendre(&cache)?;
        retirer_ce_qui_est_au_poste(&cache)?;
        match restaurer(&cache, vu, racine_drive, client, &verrou) {
            // Le dossier cache porte ce qu'une autre recuperation y avait mis, d'un autre dossier du Drive ou d'une
            // sauvegarde qui a change depuis : il repart vide.
            Err(e) if e.echec.classe == Some(Classe::DossierNonVide) => {
                socle::fichiers::effacer(&cache)?;
                std::fs::create_dir_all(&cache).map_err(|e| ErreurGoogle::from(socle::echec::du_disque(&cache, &e)))?;
                restaurer(&cache, vu, racine_drive, client, &verrou)?
            }
            autre => autre?,
        }
    };
    if !r.complete() {
        return Ok(Recuperation::PasAuBout(r));
    }
    passer_ce_qui_est_au_poste(racine, &cache)?;
    let index = lire_index(&cache)?.ok_or_else(|| ErreurGoogle::from(socle::echec::erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : la compagnie revenue du Drive n'a pas d'index.")))?;
    ecrire_index(&cache, &Index { sans_la_main: Some(sans), ..index })?;
    Ok(echanger(racine, &cache, r.incidents))
}

/// ECHANGE la compagnie `racine` et le dossier `cache` qui la remplace ; l'ancienne s'efface ensuite. `incidents` : ce
/// que la restauration n'a pas fait redescendre (`Restauration::incidents`).
fn echanger(racine: &Path, cache: &Path, incidents: Vec<String>) -> Recuperation {
    let ancienne = match a_cote(racine, PREFIXE_REMPLACEE) {
        Ok(a) => a,
        Err(e) => return Recuperation::PasEchangee(e),
    };
    // Le reste d'un echange d'avant que rien n'a efface : la compagnie est la, il ne sert plus.
    if let Err(e) = socle::fichiers::effacer(&ancienne) {
        return Recuperation::PasEchangee(e);
    }
    let pas_echangee = |e: std::io::Error, p: &Path| {
        Recuperation::PasEchangee(socle::echec::erreur(
            Classe::Disque,
            format!("Le dossier de la compagnie ne s'échange pas avec ce qui est revenu du Drive ({}) : la compagnie n'a pas changé. « Prendre la main » de nouveau le refait, quand rien ne le tient plus.", socle::fichiers::panne_fichier(&e, p)),
        ))
    };
    if let Err(e) = std::fs::rename(racine, &ancienne) {
        return pas_echangee(e, racine);
    }
    if let Err(e) = std::fs::rename(cache, racine) {
        // L'ancienne revient a sa place ; qu'elle n'y revienne pas, le lancement suivant l'y remet.
        if let Err(retour) = std::fs::rename(&ancienne, racine) {
            return Recuperation::PasEchangee(socle::echec::erreur(
                Classe::Disque,
                format!("Le dossier de la compagnie ne s'échange pas, et ne revient pas à sa place ({}) : relancer le logiciel l'y remet.", socle::fichiers::panne_fichier(&retour, &ancienne)),
            ));
        }
        return pas_echangee(e, cache);
    }
    let reste = socle::fichiers::effacer(&ancienne).err().map(|e| format!("L'ancienne copie de la compagnie ne s'efface pas entière : elle reste cachée et part au prochain lancement ({}).", e.phrase));
    Recuperation::Echangee { incidents, reste }
}

/// AU LANCEMENT, UN ECHANGE COUPE (`echanger`) dans le dossier `porteur` : une compagnie remplacee dont la place est vide
/// y revient ; celle dont la place est prise (l'echange est alle au bout) s'efface. Rend ce qui s'est passe, une phrase
/// par dossier, et ce qui resiste.
pub fn reparer_un_echange_coupe(porteur: &Path) -> R<(Vec<String>, Vec<String>)> {
    let (mut faits, mut resistent) = (Vec::new(), Vec::new());
    let entrees = match std::fs::read_dir(porteur) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((faits, resistent)),
        Err(e) => return Err(socle::echec::du_disque(porteur, &e)),
    };
    for e in entrees {
        let e = e.map_err(|x| socle::echec::du_disque(porteur, &x))?;
        let nom = e.file_name().to_string_lossy().into_owned();
        let Some(place) = nom.strip_prefix(PREFIXE_REMPLACEE) else { continue };
        let ancienne = e.path();
        let racine = porteur.join(place);
        if socle::fichiers::present(&racine)? {
            match socle::fichiers::effacer(&ancienne) {
                Ok(()) => faits.push(format!("Ancienne copie d'une compagnie reprise du Drive effacée : {}.", ancienne.display())),
                Err(x) => resistent.push(x.phrase),
            }
        } else {
            match std::fs::rename(&ancienne, &racine) {
                Ok(()) => faits.push(format!("Compagnie remise à sa place après une reprise du Drive coupée : {}.", racine.display())),
                Err(x) => resistent.push(socle::fichiers::panne_fichier(&x, &ancienne)),
            }
        }
    }
    Ok((faits, resistent))
}
