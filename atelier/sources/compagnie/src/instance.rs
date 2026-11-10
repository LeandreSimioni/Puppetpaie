//! UNE FENETRE PAR INSTALLATION : une installation sur une cle USB s'ouvre sur n'importe quel poste, meme si ce poste a
//! deja un logiciel. Un logiciel = un
//! administrateur = une installation (le dossier qui porte `logiciel/`, et a cote `prive/`, `compagnies/*`, `fictives/*`,
//! `administrateur.json`). Deux installations s'ouvrent cote a cote sur le meme poste ; la meme, lancee deux fois, ramene
//! sa fenetre devant.
//!
//! LE CRENEAU. Une installation ouverte tient un CRENEAU : deux ports voisins de la boucle locale, dans une plage fixe
//! (`Plage::DU_POSTE` : 7330 a 7429, cinquante creneaux). Le premier est le VERROU (une prise d'ecoute que le systeme
//! rend quand le processus meurt, meme tue) ; le second, le PORT DE LA PAGE. Pourquoi un port de page STABLE : le
//! navigateur (WebView2, WebKit, Chromium) retient un mot de passe, et le reste, PAR ORIGINE (`http://hote:port`) ; un port
//! tire au hasard ferait de chaque lancement un site neuf. Pourquoi un port par installation : deux pages au meme port ne
//! peuvent pas tourner ensemble, et deux installations a la meme origine se meleraient leurs mots de passe.
//!
//! QUEL CRENEAU. Celui que l'installation a RETENU (`.fenetre.json`, a sa racine : il voyage avec elle, une cle USB garde
//! son port d'un poste a l'autre) ; a defaut, un creneau tire de l'emplacement de l'installation (une empreinte de son
//! chemin), pour que deux installations neuves ne visent pas le meme. Puis, a partir de lui, le premier creneau qui va :
//!   - son verrou est libre (et son port de page aussi) : il est pris, et retenu s'il ne l'etait pas ;
//!   - son verrou est tenu par CETTE installation (elle repond par son dossier, le meme) : elle revient devant, `Devant` ;
//!   - tenu par une AUTRE installation (un autre dossier), ou par un autre programme, ou son port de page est pris : le
//!     creneau suivant. Plus de page « deja ouvert » : l'autre installation n'empeche rien.
//! Aucun creneau ne va (cinquante pris) : la fenetre s'ouvre quand meme, a un port que le systeme donne, sans verrou --
//! un verrou ne doit jamais empecher une paie.
//!
//! Le dossier compare est le chemin canonique de l'installation : une copie (un autre dossier) est une autre
//! installation, meme si elle a emporte le `.fenetre.json` de l'original -- elle prend alors le creneau suivant, et le
//! retient pour elle.

use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use socle::echec::R;

/// Le fichier, a la racine de l'installation, qui retient son creneau.
pub const FICHIER: &str = ".fenetre.json";

const DEMANDE_DEVANT: &str = "PuppetPaie devant";
const DEMANDE_QUI: &str = "PuppetPaie qui";
const REPONSE: &str = "PuppetPaie ok";
const DELAI: Duration = Duration::from_secs(2);

/// La plage des creneaux : `creneaux` paires de ports a partir de `premier` (le verrou, puis la page).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plage {
    pub premier: u16,
    pub creneaux: u16,
}

impl Plage {
    /// La plage du poste : 7330 a 7429. 7330/7331 etaient le verrou et la page d'avant (une seule fenetre par poste).
    pub const DU_POSTE: Plage = Plage { premier: 7330, creneaux: 50 };

    pub fn verrou(&self, creneau: u16) -> u16 {
        self.premier + 2 * creneau
    }

    pub fn page(&self, creneau: u16) -> u16 {
        self.premier + 2 * creneau + 1
    }
}

/// Ce que tient une fenetre ouverte : le verrou de son creneau (a garder tant qu'elle vit), l'ecoute de sa page, et son
/// port. `creneau` (et `verrou`) : `None` quand aucun creneau n'allait (pas de verrou, un port du systeme).
pub struct Prise {
    pub verrou: Option<Verrou>,
    pub page: TcpListener,
    pub port: u16,
    pub creneau: Option<u16>,
}

/// Le verrou d'un creneau : tant qu'il vit, le creneau est a la fenetre ; il le rend en tombant (et, bien sur, quand le
/// processus meurt).
pub struct Verrou {
    ferme: Arc<AtomicBool>,
}

impl Drop for Verrou {
    fn drop(&mut self) {
        self.ferme.store(true, Ordering::SeqCst);
    }
}

/// Ce que dit le verrou.
pub enum Instance {
    /// Cette fenetre est celle de l'installation : sa prise.
    Tenue(Prise),
    /// La fenetre de CETTE installation est deja ouverte : elle vient d'etre ramenee devant.
    Devant,
}

/// `.fenetre.json` : le creneau que l'installation a retenu.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Retenu {
    creneau: u16,
    quoi: String,
}

/// Le dossier d'une installation, tel que le verrou le compare : canonique, sans le prefixe `\\?\` de Windows.
pub fn dossier_de(inst: &Path) -> R<String> {
    let c = std::fs::canonicalize(inst).map_err(|e| socle::echec::du_disque(inst, &e))?;
    let t = c.to_string_lossy();
    Ok(match t.strip_prefix(r"\\?\") {
        Some(sans) => sans.to_string(),
        None => t.into_owned(),
    })
}

/// Le creneau retenu par l'installation ; `None` sans `.fenetre.json`, ou si le creneau retenu n'est pas dans la plage.
pub fn creneau_retenu(inst: &Path, plage: Plage) -> R<Option<u16>> {
    let p = inst.join(FICHIER);
    if !p.is_file() {
        return Ok(None);
    }
    let r: Retenu = socle::forme::lire_fichier(&p, "Le creneau de la fenetre (.fenetre.json) ne se lit pas : l'effacer, la fenetre en retiendra un.")?;
    Ok((r.creneau < plage.creneaux).then_some(r.creneau))
}

/// Retient `creneau` pour l'installation. Une installation ou l'on n'ecrit pas (un support en lecture seule) garde son
/// creneau tire de son emplacement : rien ne casse, le port est seulement moins stable -- l'echec n'empeche pas la fenetre.
fn retenir(inst: &Path, creneau: u16) -> R<()> {
    let r = Retenu {
        creneau,
        quoi: "Le creneau de la fenetre de cette installation (moteur/src/interface/instance.rs) : son port de page reste le meme d'un lancement a l'autre, et le navigateur y retrouve ce qu'il a retenu.".to_string(),
    };
    socle::ecriture::ecrire_json(&inst.join(FICHIER), &r, 2)
}

/// Le creneau que vise une installation qui n'en a pas retenu : une empreinte de son dossier.
fn creneau_tire(dossier: &str, plage: Plage) -> u16 {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(dossier.as_bytes());
    u16::from_be_bytes([h[0], h[1]]) % plage.creneaux.max(1)
}

/// Le premier creneau a essayer : le retenu, sinon le tire.
pub fn creneau_vise(inst: &Path, plage: Plage) -> R<u16> {
    Ok(match creneau_retenu(inst, plage)? {
        Some(k) => k,
        None => creneau_tire(&dossier_de(inst)?, plage),
    })
}

/// Qui tient le verrou d'un creneau.
enum Occupant {
    /// Personne ne repond a sa porte (rien n'y ecoute, ou rien ne prend la demande a temps).
    Personne,
    /// Un autre programme : il ne lit pas la demande, ou ne repond pas en PuppetPaie.
    AutreProgramme,
    /// Une fenetre de PuppetPaie : le dossier de son installation.
    Installation(String),
}

/// Qui tient le verrou au port `port`. `devant` : lui demander de revenir devant (elle ne le fait que si c'est la
/// sienne, `ici`). Une erreur du systeme qui n'est aucun de ces etats remonte.
fn demander(port: u16, ici: &str, devant: bool) -> R<Occupant> {
    let refus = |e: std::io::Error| socle::refus!("Le creneau du port {} ne se demande pas : {}", port, e);
    let mut c = match TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)), DELAI) {
        Ok(c) => c,
        Err(e) if matches!(e.kind(), ErrorKind::ConnectionRefused | ErrorKind::TimedOut | ErrorKind::ConnectionReset) => return Ok(Occupant::Personne),
        Err(e) => return Err(refus(e)),
    };
    c.set_read_timeout(Some(DELAI)).map_err(refus)?;
    let verbe = if devant { DEMANDE_DEVANT } else { DEMANDE_QUI };
    match c.write_all(format!("{} {}\n", verbe, ici).as_bytes()) {
        Ok(()) => {}
        Err(e) if matches!(e.kind(), ErrorKind::BrokenPipe | ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted) => return Ok(Occupant::AutreProgramme),
        Err(e) => return Err(refus(e)),
    }
    let mut l = String::new();
    match BufReader::new(c).read_line(&mut l) {
        Ok(_) => {}
        Err(e)
            if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted | ErrorKind::InvalidData) =>
        {
            return Ok(Occupant::AutreProgramme)
        }
        Err(e) => return Err(refus(e)),
    }
    Ok(match l.trim_end().strip_prefix(REPONSE) {
        Some(r) => Occupant::Installation(r.trim().to_string()),
        None => Occupant::AutreProgramme,
    })
}

/// Le verrou tenu : il repond a qui demande, par le dossier de l'installation, et ramene la fenetre devant quand c'est
/// la MEME installation qui est relancee (une autre ne fait que demander qui est la). Le fil tient seul la prise ; il la
/// rend des que le `Verrou` tombe (la prise attend sans bloquer : le fil regarde le drapeau toutes les 50 ms).
fn repondre(ecoute: TcpListener, ici: String, devant: impl Fn() + Send + 'static) -> R<Verrou> {
    let ferme = Arc::new(AtomicBool::new(false));
    let vu = ferme.clone();
    ecoute.set_nonblocking(true).map_err(|e| socle::refus!("Le verrou de la fenetre n'attend pas sans bloquer : {}", e))?;
    std::thread::spawn(move || {
        while !vu.load(Ordering::SeqCst) {
            let c = match ecoute.accept() {
                Ok((c, _)) => c,
                // Personne ne demande (`WouldBlock`), ou une demande s'est perdue avant d'etre prise (le demandeur parti) :
                // le verrou attend la suivante. Il ne rend jamais le creneau tant que la fenetre vit.
                Err(attente) => {
                    drop(attente);
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
            };
            let _ = c.set_nonblocking(false);
            let _ = c.set_read_timeout(Some(DELAI));
            let mut lecteur = BufReader::new(&c);
            let mut ligne = String::new();
            // Une demande qui ne vient pas a temps, ou coupee : le demandeur est parti, rien a lui repondre.
            if let Err(partie) = lecteur.read_line(&mut ligne) {
                drop(partie);
                continue;
            }
            let ligne = ligne.trim_end();
            let (verbe, la_bas) = if let Some(r) = ligne.strip_prefix(DEMANDE_DEVANT) {
                (true, r.trim())
            } else if let Some(r) = ligne.strip_prefix(DEMANDE_QUI) {
                (false, r.trim())
            } else {
                continue;
            };
            let _ = (&c).write_all(format!("{} {}\n", REPONSE, ici).as_bytes());
            if verbe && la_bas == ici {
                devant();
            }
        }
    });
    Ok(Verrou { ferme })
}

/// L'ecoute du port `port` ; `None` s'il est tenu (par un autre, ou refuse par le systeme : une plage reservee de Windows).
fn ecouter(port: u16) -> R<Option<TcpListener>> {
    match TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
        Ok(l) => {
            sans_heritage(&l);
            Ok(Some(l))
        }
        Err(e) if matches!(e.kind(), ErrorKind::AddrInUse | ErrorKind::PermissionDenied) => Ok(None),
        Err(e) => Err(socle::refus!("Le port {} ne s'ecoute pas : {}", port, e)),
    }
}

/// LE VERROU DE L'INSTALLATION `inst` (voir l'en-tete). `devant` : ce que fait la fenetre tenue quand la meme
/// installation est relancee.
pub fn prendre_de(inst: &Path, plage: Plage, devant: impl Fn() + Send + 'static) -> R<Instance> {
    let ici = dossier_de(inst)?;
    let retenu = creneau_retenu(inst, plage)?;
    let depart = match retenu {
        Some(k) => k,
        None => creneau_tire(&ici, plage),
    };
    for i in 0..plage.creneaux {
        let k = (depart + i) % plage.creneaux;
        let Some(verrou) = ecouter(plage.verrou(k))? else {
            // Tenu : par cette installation (elle revient devant), par une autre, ou par un autre programme.
            if matches!(demander(plage.verrou(k), &ici, true)?, Occupant::Installation(d) if d == ici) {
                return Ok(Instance::Devant);
            }
            continue;
        };
        // Le port de la page doit etre libre aussi : sinon le creneau ne va pas (un autre programme y est).
        let Some(page) = ecouter(plage.page(k))? else { continue };
        if retenu != Some(k) {
            // Un support en lecture seule ne retient rien (`retenir`) : la fenetre s'ouvre quand meme.
            let _ = retenir(inst, k);
        }
        let verrou = repondre(verrou, ici.clone(), devant)?;
        return Ok(Instance::Tenue(Prise { verrou: Some(verrou), page, port: plage.page(k), creneau: Some(k) }));
    }
    // Aucun creneau : la fenetre s'ouvre quand meme, sans verrou, a un port du systeme.
    let page = ecouter(0)?.ok_or_else(|| socle::echec::refus("Le systeme ne donne aucun port a la fenetre."))?;
    let port = page.local_addr().map_err(|e| socle::refus!("Le port de la fenetre ne se lit pas : {}", e))?.port();
    Ok(Instance::Tenue(Prise { verrou: None, page, port, creneau: None }))
}

/// Le verrou de CETTE installation (`installation::base()`), dans la plage du poste.
pub fn prendre(devant: impl Fn() + Send + 'static) -> R<Instance> {
    prendre_de(&super::installation::base(), Plage::DU_POSTE, devant)
}

/// LA FENETRE DE L'INSTALLATION `inst` EST-ELLE OUVERTE ? (la mise a jour ne s'applique jamais sous elle). On demande au
/// creneau qu'elle vise -- le retenu, sinon le tire -- et a ceux qui suivent tant qu'une autre installation les tient :
/// les memes creneaux que `prendre_de` parcourrait. Un creneau libre arrete la recherche (elle l'aurait pris).
pub fn ouverte(inst: &Path, plage: Plage) -> R<bool> {
    let ici = dossier_de(inst)?;
    let depart = creneau_vise(inst, plage)?;
    for i in 0..plage.creneaux {
        let k = (depart + i) % plage.creneaux;
        if let Some(l) = ecouter(plage.verrou(k))? {
            drop(l);
            return Ok(false);
        }
        if matches!(demander(plage.verrou(k), &ici, false)?, Occupant::Installation(d) if d == ici) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Une prise d'ecoute ne passe jamais a un processus enfant (une route, le relais de la mise a jour). La bibliotheque
/// standard la cree deja non heritable, mais un fournisseur reseau intercale (un antivirus) peut perdre ce drapeau : un
/// enfant la gardait alors ouverte apres la mort de la fenetre (port tenu par un PID mort).
pub fn sans_heritage(l: &TcpListener) {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawSocket;
        extern "system" {
            fn SetHandleInformation(objet: isize, masque: u32, drapeaux: u32) -> i32;
        }
        const HANDLE_FLAG_INHERIT: u32 = 1;
        // SAFETY : la prise est vivante (empruntee) ; on ne change que son drapeau d'heritage.
        unsafe {
            SetHandleInformation(l.as_raw_socket() as isize, HANDLE_FLAG_INHERIT, 0);
        }
    }
    #[cfg(not(windows))]
    let _ = l;
}
