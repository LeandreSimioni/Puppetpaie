//! L'archivage d'un mois dans le Drive.
//!
//! Les sorties d'un mois (`sorties/<mois>/`, sans le sous-dossier
//! `envoi/` : memes documents redecoupes par salarie, deja partis) vont dans
//! `PuppetPaie/<la compagnie>/archives/<mois>` du Drive du compte autorise, sous le dossier de la compagnie que sa
//! sauvegarde remplit (`compagnie::sauvegarde::dossier_de_la_compagnie`). Le fichier groupe y a toute sa place :
//! c'est l'archive de la compagnie, pas le courrier d'un salarie.
//!
//! Relancer `archiver` REMPLACE le contenu des fichiers de meme nom (`Depose::remplace`), au
//! lieu d'empiler des copies : on reproduit un mois quand un montant arrive ou qu'un bareme change.
//!
//! Le client Drive se passe en parametre (`drive::Drive`), comme pour la sauvegarde.

use std::path::Path;
use socle::echec::Classe;

use envois::drive::{Client, Drive, ErreurGoogle, IdDrive};
use compagnie::sauvegarde::{LaMain, PosteSurLeDrive};

/// `FichierArchive`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FichierArchive {
    pub nom: socle::fichiers::NomDeFichier,
    pub octets: socle::grandeurs::Octets,
    pub remplace: bool,
}

/// `Archivage`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Archivage {
    pub mois: socle::mois::Mois,
    pub racine_drive: String,
    /// Le nom du dossier de la compagnie sous la racine du Drive, tel qu'il a ete cree.
    pub compagnie: String,
    pub compte: Option<String>,
    pub fichiers: Vec<FichierArchive>,
}

/// `SortieAbsente` : on demande a archiver un mois qui n'a pas ete produit.
fn sortie_absente(dossier: &Path) -> ErreurGoogle {
    ErreurGoogle::nouvelle(Classe::SortieAbsente, format!("{} n'existe pas.", dossier.display()))
}

/// `archiver` : depose les sorties d'un mois dans le Drive du compte autorise.
///
/// `client` : `None` pour le vrai (`Client::nouveau`, construit APRES les controles du dossier,
/// pour qu'un mois absent se dit avant tout refus du client). Le vrai Drive porte la main de ce poste
/// (`PosteSurLeDrive::de_ce_poste`) ; un client donne est le Drive joue d'une compagnie fictive (`DriveJoue`), qui n'en
/// porte aucune.
pub(crate) fn archiver(racine: &Path, le_mois: socle::mois::Mois, racine_drive: &str, client: Option<&mut dyn Drive>) -> Result<Archivage, ErreurGoogle> {
    let mois = &le_mois.to_string();
    let dossier = documents::DossierDuMois::Sorties.dans(racine, le_mois);
    if !dossier.exists() {
        return Err(sortie_absente(&dossier));
    }
    let mut fichiers: Vec<std::path::PathBuf> = Vec::new();
    for e in std::fs::read_dir(&dossier).map_err(|e| ErreurGoogle::fichier(&e, &dossier))? {
        let p = e.map_err(|e| ErreurGoogle::fichier(&e, &dossier))?.path();
        if p.is_file() {
            fichiers.push(p);
        }
    }
    fichiers.sort();
    if fichiers.is_empty() {
        return Err(sortie_absente(&dossier));
    }

    let mut propre: Client;
    let (client, ce_poste): (&mut dyn Drive, Option<PosteSurLeDrive>) = match client {
        Some(c) => (c, None),
        None => {
            propre = crate::coffre::client_drive(racine)?;
            (&mut propre, Some(PosteSurLeDrive::de_ce_poste(racine).map_err(ErreurGoogle::from)?))
        }
    };
    let identite = compagnie::sauvegarde::identite(racine).map_err(|e| ErreurGoogle::from(e.reclasser(Classe::Illisible)))?;
    let nom = compagnie::installation::nom_de(racine).map_err(ErreurGoogle::from)?;
    let connu = compagnie::sauvegarde::lire_index(racine)?.and_then(|i| i.compagnie);
    let la_sienne = ce_poste.map(|poste| LaMain { poste, depuis: socle::temps::secondes().a_la_minute() });
    let (cie, marque) = compagnie::sauvegarde::dossier_de_la_compagnie(client, racine_drive, &identite, &nom, connu.as_ref(), la_sienne.as_ref())?;
    // LA MAIN, comme pour la sauvegarde : ce poste archive s'il la tient, la prend si personne ne la tient ; un autre poste
    // la tenant, ou une main qui ne se lit pas, rien ne part, et c'est dit.
    if let Some(la_sienne) = &la_sienne {
        match LaMain::de(marque.as_ref()) {
            Ok(Some(m)) if m.poste == la_sienne.poste => {}
            Ok(Some(m)) => {
                return Err(ErreurGoogle::nouvelle(
                    Classe::Refuse,
                    format!(
                        "Le Drive de cette compagnie est tenu par un autre poste depuis le {} : ce poste n'y archive pas. « Prendre la main » (Configuration › Le Drive) l'y remet.",
                        m.depuis.jour_de_paris().jj_mm()
                    ),
                ))
            }
            Err(e) => {
                return Err(ErreurGoogle::from(e.reformuler(|p| {
                    format!("{} Ce poste n'archive pas dans le Drive de cette compagnie : « Prendre la main » (Configuration › Le Drive) l'y remet.", p)
                })))
            }
            Ok(None) => client.marquer(&cie, &la_sienne.marque(&identite))?,
        }
    }
    let cible = client.chemin(&cie, &[compagnie::sauvegarde::ARCHIVES, mois])?;
    // Un seul listage du dossier du mois : c'est lui qui dit quels fichiers se remplacent (et
    // non une recherche par fichier, par nom seul, qui prendrait les dossiers).
    let deja = client.lister(&cible)?;
    let mut deposes = Vec::new();
    for f in &fichiers {
        let nom_local = socle::fichiers::NomDeFichier::du(f)?;
        let r = client.televerser(f, &cible, None, deja.le_seul(nom_local.texte())?)?;
        let octets = socle::grandeurs::Octets::du_disque(&std::fs::metadata(f).map_err(|e| ErreurGoogle::fichier(&e, f))?);
        // Le fichier porte dans Drive le nom sous lequel il y est depose (`televerser`, sans nom : celui du fichier).
        deposes.push(FichierArchive { nom: nom_local, octets, remplace: r.remplace });
    }
    Ok(Archivage { mois: le_mois, racine_drive: racine_drive.to_string(), compagnie: nom, compte: client.compte(), fichiers: deposes })
}

/// LE DRIVE JOUE d'une compagnie fictive (ses envois sont joues, `bridage::branchement`) : le depot se deroule, chaque
/// fichier se nomme et se compte, et rien ne part. Il n'a pas de compte, ne connait aucun fichier et n'en rend aucun.
pub(crate) struct DriveJoue;

impl Drive for DriveJoue {
    fn compte(&self) -> Option<String> {
        None
    }
    fn chemin(&mut self, depuis: &IdDrive, noms: &[&str]) -> Result<IdDrive, ErreurGoogle> {
        Ok(IdDrive::lire(&format!("{}/{}", depuis, noms.join("/")))?)
    }
    fn lister(&mut self, _parent: &IdDrive) -> Result<envois::drive::FichiersDuDossier, ErreurGoogle> {
        Ok(envois::drive::FichiersDuDossier::default())
    }
    fn televerser(&mut self, fichier: &Path, _parent: &IdDrive, nom: Option<&str>, _connu: Option<&envois::drive::Distant>) -> Result<envois::drive::Depose, ErreurGoogle> {
        let nom = nom.map(String::from).or_else(|| fichier.file_name().map(|n| n.to_string_lossy().into_owned()));
        Ok(envois::drive::Depose { id: None, nom, lien: None, remplace: false })
    }
    fn sous_dossiers(&mut self, _parent: &IdDrive) -> Result<Vec<envois::drive::SousDossier>, ErreurGoogle> {
        Ok(Vec::new())
    }
    fn telecharger(&mut self, id: &IdDrive) -> Result<Vec<u8>, ErreurGoogle> {
        Err(ErreurGoogle::nouvelle(Classe::Refuse, format!("Le Drive d'une compagnie fictive ne garde rien : {} n'y est pas.", id)))
    }
    fn creer_dossier(&mut self, nom: &str, parent: &IdDrive, _marque: &envois::drive::MarqueDuDossier) -> Result<IdDrive, ErreurGoogle> {
        Ok(IdDrive::lire(&format!("{}/{}", parent, nom))?)
    }
    fn marquer(&mut self, _id: &IdDrive, _marque: &envois::drive::MarqueDuDossier) -> Result<(), ErreurGoogle> {
        Ok(())
    }
    fn en_place(&mut self, _id: &IdDrive) -> Result<Option<Option<envois::drive::MarqueDuDossier>>, ErreurGoogle> {
        Ok(Some(None))
    }
}
