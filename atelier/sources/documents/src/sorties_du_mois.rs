//! LES SORTIES D'UN MOIS, OU QU'ELLES SOIENT : en vrac sous `sorties/<mois>/` tant que son annee est ouverte, dans
//! l'archive de son exercice (`sorties/exercices/Exercice_AAAA.zip`, sous le meme chemin relatif) des qu'elle est close.
//! Tout ce qui lit une piece d'un mois passe par ici (`SortiesDuMois`) : la liste de ses pieces, leurs octets, et, pour ce
//! qui s'ouvre par un chemin (la fabrique PDF, le poste, la lecture d'une DSN), un fichier. Une annee close a toujours
//! ses pieces dans son archive, une annee ouverte toujours en vrac : il n'y a pas d'autre rangement a lire.
//!
//! Et les exercices clos (`config/exercices.json`), qui disent ou sont les sorties d'un mois : leur forme est lue ici,
//! une fois, et ecrite ici.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use socle::echec::{Classe, R};
use socle::fichiers::{CheminRelatif, Empreinte, NomDeFichier};
use socle::grandeurs::Octets;
use socle::mois::{Annee, Mois};
use socle::paris::Moment;

use crate::DossierDuMois;

// ------------------------------------------------------------------ les exercices clos

/// Les exercices clos de la compagnie, sous sa racine.
pub const EXERCICES: CheminRelatif = CheminRelatif::du_moteur("config/exercices.json");

/// `config/exercices.json` : chaque annee close (`clos`, sa cloture sous son annee) et les clotures retirees (`rouverts`),
/// dans l'ordre ou elles l'ont ete. Lu par sa forme (`lire_exercices`), ecrit par `ecrire_exercices`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Exercices {
    pub clos: BTreeMap<Annee, ExerciceNote>,
    pub rouverts: Vec<ExerciceRouvertNote>,
}

/// La cloture d'une annee : quand, par qui, l'archive (son chemin sous la racine, sa taille, son empreinte, le nombre de
/// fichiers qu'elle reunit), si elle a ete forcee, et les mois clos sans DSN acceptee.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExerciceNote {
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub le: Moment,
    pub qui: String,
    pub archive: CheminRelatif,
    pub octets: Octets,
    pub empreinte: Empreinte,
    pub fichiers: u64,
    pub forcee: bool,
    pub sans_dsn: Vec<Mois>,
}

/// Une cloture retiree : la cloture telle qu'elle etait, son annee, quand et par qui elle l'a ete.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExerciceRouvertNote {
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub le: Moment,
    pub qui: String,
    pub archive: CheminRelatif,
    pub octets: Octets,
    pub empreinte: Empreinte,
    pub fichiers: u64,
    pub forcee: bool,
    pub sans_dsn: Vec<Mois>,
    pub annee: Annee,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub rouvert_le: Moment,
    pub rouvert_par: String,
}

impl Exercices {
    /// La cloture de l'annee `annee`, ou `None`.
    pub fn de(&self, annee: Annee) -> Option<&ExerciceNote> {
        self.clos.get(&annee)
    }

    /// Les annees closes, et leur cloture.
    pub fn clos(&self) -> &BTreeMap<Annee, ExerciceNote> {
        &self.clos
    }
}

/// `config/exercices.json`, lu par sa forme : aucune annee close s'il n'existe pas. Le texte est lu une fois tant qu'il
/// ne change pas (l'etat d'un mois le demande pour chaque mois) ; le refus d'un fichier illisible ne se garde pas.
pub fn lire_exercices(racine: &Path) -> R<Exercices> {
    let p = EXERCICES.dans(racine);
    match socle::forme::partagee::<Exercices>(&p, "Le fichier des exercices clos ne se lit pas.").map_err(|m| m.reclasser(Classe::Illisible))? {
        Some(e) => Ok((*e).clone()),
        None => Ok(Exercices { clos: BTreeMap::new(), rouverts: Vec::new() }),
    }
}

/// Ecrit `config/exercices.json` sous la compagnie `racine`.
pub fn ecrire_exercices(racine: &Path, tous: &Exercices) -> R<()> {
    socle::ecriture::ecrire_json(&EXERCICES.dans(racine), tous, 1).map_err(|m| m.reclasser(Classe::Disque))
}

// ------------------------------------------------------------------ la porte

/// LES SORTIES D'UN MOIS : en vrac sous `sorties/<mois>/` (son annee est ouverte), ou dans l'archive de son exercice
/// (son annee est close), sous le meme chemin relatif a la compagnie. Ce que `config/exercices.json` dit decide, et rien
/// d'autre. Chaque lecture d'une piece d'un mois passe par elle (`du_mois`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortiesDuMois {
    /// L'annee est ouverte : `sorties/<mois>/`, sous la compagnie `racine`.
    EnVrac { racine: PathBuf, mois: Mois },
    /// L'annee est close : l'archive de son exercice (`archive`, sous la compagnie `racine`).
    DansLExercice { racine: PathBuf, mois: Mois, archive: PathBuf },
}

#[track_caller]
fn illisible(archive: &Path, e: impl std::fmt::Display) -> socle::echec::Echec {
    socle::refus!("L'archive de l'exercice {} ne se lit pas : {}", archive.display(), e).reclasser(Classe::Illisible)
}

/// L'archive `archive`, ouverte.
fn ouverte(archive: &Path) -> R<zip::ZipArchive<std::fs::File>> {
    let f = std::fs::File::open(archive).map_err(|e| socle::echec::du_disque(archive, &e).reclasser(Classe::Disque))?;
    zip::ZipArchive::new(f).map_err(|e| illisible(archive, e))
}

/// Les octets de l'entree `nom` de l'archive ouverte `z` ; aucun si elle n'y est pas.
fn entree(z: &mut zip::ZipArchive<std::fs::File>, archive: &Path, nom: &str) -> R<Option<Vec<u8>>> {
    let mut f = match z.by_name(nom) {
        Ok(f) => f,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(illisible(archive, e)),
    };
    let mut o = Vec::new();
    f.read_to_end(&mut o).map_err(|e| illisible(archive, e))?;
    Ok(Some(o))
}

impl SortiesDuMois {
    /// Les sorties du mois `mois` de la compagnie `racine` : dans l'archive de son exercice si son annee est close, en
    /// vrac sinon.
    pub fn du_mois(racine: &Path, mois: Mois) -> R<SortiesDuMois> {
        Ok(match lire_exercices(racine)?.de(mois.annee()) {
            Some(clos) => SortiesDuMois::DansLExercice { racine: racine.to_path_buf(), mois, archive: clos.archive.dans(racine) },
            None => SortiesDuMois::EnVrac { racine: racine.to_path_buf(), mois },
        })
    }

    /// LE FICHIER A OUVRIR POUR LE CHEMIN `chemin`, relatif a la compagnie, qu'une route a rendu a la page : la piece
    /// d'un mois d'une annee close (`sorties/<mois>/…`, ou son `envoi/`) s'ouvre par sa copie (`fichier`), et se refuse
    /// si son archive ne la porte pas ; tout autre chemin est sous la compagnie, tel quel.
    pub fn a_ouvrir(racine: &Path, chemin: &CheminRelatif) -> R<PathBuf> {
        let Some(dossier) = chemin.parent() else {
            return Ok(chemin.dans(racine));
        };
        for annee in lire_exercices(racine)?.clos().keys() {
            for m in annee.les_mois() {
                if let Some(d) = DossierDuMois::TOUS.into_iter().find(|d| d.chemin(m) == dossier) {
                    return match SortiesDuMois::du_mois(racine, m)?.fichier(d, &chemin.nom())? {
                        Some(p) => Ok(p),
                        None => Err(socle::refus!("{} n'est pas dans l'archive de l'exercice {}.", chemin, annee)),
                    };
                }
            }
        }
        Ok(chemin.dans(racine))
    }

    /// Son mois.
    pub fn mois(&self) -> Mois {
        match self {
            SortiesDuMois::EnVrac { mois, .. } | SortiesDuMois::DansLExercice { mois, .. } => *mois,
        }
    }

    /// Le chemin, relatif a la compagnie, du fichier `nom` du dossier `d` de ce mois : celui du vrac et de l'entree de
    /// l'archive.
    fn chemin_de(&self, d: DossierDuMois, nom: &NomDeFichier) -> CheminRelatif {
        d.chemin(self.mois()).joint(nom)
    }

    /// Les noms des fichiers du dossier `d` de ce mois (pas ceux de ses sous-dossiers), tries ; aucun s'il n'en a pas.
    pub fn noms(&self, d: DossierDuMois) -> R<Vec<NomDeFichier>> {
        let mut out = Vec::new();
        match self {
            SortiesDuMois::EnVrac { racine, mois } => {
                let dossier = d.dans(racine, *mois);
                for p in socle::fichiers::entrees(&dossier)?.into_iter().flatten() {
                    if p.is_file() {
                        out.push(NomDeFichier::du(&p)?);
                    }
                }
            }
            SortiesDuMois::DansLExercice { mois, archive, .. } => {
                let prefixe = format!("{}/", d.chemin(*mois));
                for n in ouverte(archive)?.file_names() {
                    if let Some(nom) = n.strip_prefix(&prefixe).filter(|r| !r.is_empty() && !r.contains('/')) {
                        out.push(NomDeFichier::lire(nom)?);
                    }
                }
            }
        }
        out.sort();
        Ok(out)
    }

    /// Le fichier `nom` du dossier `d` de ce mois est-il la ?
    pub fn existe(&self, d: DossierDuMois, nom: &NomDeFichier) -> R<bool> {
        match self {
            SortiesDuMois::EnVrac { racine, .. } => Ok(self.chemin_de(d, nom).dans(racine).is_file()),
            SortiesDuMois::DansLExercice { archive, .. } => {
                let n = self.chemin_de(d, nom);
                Ok(ouverte(archive)?.file_names().any(|x| x == n.texte()))
            }
        }
    }

    /// Les octets du fichier `nom` du dossier `d` de ce mois ; aucun s'il n'y est pas.
    pub fn octets(&self, d: DossierDuMois, nom: &NomDeFichier) -> R<Option<Vec<u8>>> {
        match self {
            SortiesDuMois::EnVrac { racine, .. } => {
                let p = self.chemin_de(d, nom).dans(racine);
                match std::fs::read(&p) {
                    Ok(o) => Ok(Some(o)),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(e) => Err(socle::echec::du_disque(&p, &e).reclasser(Classe::Disque)),
                }
            }
            SortiesDuMois::DansLExercice { archive, .. } => entree(&mut ouverte(archive)?, archive, self.chemin_de(d, nom).texte()),
        }
    }

    /// LE FICHIER `nom` DU DOSSIER `d` DE CE MOIS, A OUVRIR PAR SON CHEMIN (la fabrique PDF, le poste, la lecture d'une
    /// DSN) : en vrac, le fichier lui-meme ; dans l'archive, sa copie dans le dossier temporaire du systeme, sous le meme
    /// chemin relatif. Aucun s'il n'y est pas.
    pub fn fichier(&self, d: DossierDuMois, nom: &NomDeFichier) -> R<Option<PathBuf>> {
        match self {
            SortiesDuMois::EnVrac { racine, .. } => {
                let p = self.chemin_de(d, nom).dans(racine);
                Ok(p.is_file().then_some(p))
            }
            SortiesDuMois::DansLExercice { .. } => {
                let Some(o) = self.octets(d, nom)? else {
                    return Ok(None);
                };
                let p = self.chemin_de(d, nom).dans(&socle::fichiers::dossier_temporaire().join(format!("puppetpaie-exercices-{}", socle::processus::numero())));
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| socle::echec::du_disque(parent, &e).reclasser(Classe::Disque))?;
                }
                std::fs::write(&p, &o).map_err(|e| socle::echec::du_disque(&p, &e).reclasser(Classe::Disque))?;
                Ok(Some(p))
            }
        }
    }

    /// LE DOSSIER DE CE MOIS QUI PORTE LE FICHIER `nom`, dont on ne sait que le nom : l'envoi, puis les sorties
    /// (`DossierDuMois::TOUS`) ; aucun s'il n'est dans aucun.
    pub fn chercher(&self, nom: &NomDeFichier) -> R<Option<DossierDuMois>> {
        for d in DossierDuMois::TOUS {
            if self.existe(d, nom)? {
                return Ok(Some(d));
            }
        }
        Ok(None)
    }

    /// REMET EN VRAC LES SORTIES DE CE MOIS, a l'octet pres, depuis l'archive de son exercice (sous-dossiers compris) :
    /// la reouverture de l'annee. Rien pour un mois en vrac.
    pub fn remettre_en_vrac(&self) -> R<()> {
        let SortiesDuMois::DansLExercice { racine, mois, archive } = self else {
            return Ok(());
        };
        let prefixe = format!("{}/", DossierDuMois::Sorties.chemin(*mois));
        let mut z = ouverte(archive)?;
        let noms: Vec<String> = z.file_names().filter(|n| n.starts_with(&prefixe) && !n.ends_with('/')).map(str::to_string).collect();
        for n in noms {
            let p = CheminRelatif::lire(&n)?.dans(racine);
            let Some(o) = entree(&mut z, archive, &n)? else {
                continue;
            };
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).map_err(|e| socle::echec::du_disque(parent, &e).reclasser(Classe::Disque))?;
            }
            std::fs::write(&p, &o).map_err(|e| socle::echec::du_disque(&p, &e).reclasser(Classe::Disque))?;
        }
        Ok(())
    }
}

/// LES MOIS QUI ONT DES SORTIES : les dossiers `sorties/<mois>/` en vrac, et les mois de chaque annee close dont
/// l'archive porte des sorties. Tries, chacun une fois.
pub fn mois_des_sorties(racine: &Path) -> R<Vec<Mois>> {
    let mut vus: std::collections::BTreeSet<Mois> = std::collections::BTreeSet::new();
    // Les dossiers AAAA-MM seulement : `sorties/exercices/` n'est pas un mois.
    for d in socle::fichiers::nommes_d_un_mois(&DossierDuMois::des_mois(racine), None, false)? {
        vus.insert(d.mois);
    }
    for (annee, clos) in lire_exercices(racine)?.clos() {
        let archive = clos.archive.dans(racine);
        let noms: Vec<String> = ouverte(&archive)?.file_names().map(str::to_string).collect();
        for m in annee.les_mois() {
            let prefixe = format!("{}/", DossierDuMois::Sorties.chemin(m));
            if noms.iter().any(|n| n.starts_with(&prefixe)) {
                vus.insert(m);
            }
        }
    }
    Ok(vus.into_iter().collect())
}
