//! Un faux Drive en memoire (le trait `drive::Drive`), pour `fichiers/sauvegarde.rs`, `fichiers/archivage.rs`, `ouverture.rs` et
//! `routes/ouverture.rs`.
//!
//! AUCUN RESEAU : il ne tient que des dossiers (et leur marque) et des fichiers en memoire, et un journal des appels.
//! Comme le vrai client : `lister` rend les FICHIERS d'un dossier par nom (homonymes compris), jamais les
//! sous-dossiers ; `televerser` remplace le fichier `connu` ou en cree un nouveau (un homonyme reste, rien n'est cherche
//! par nom) ; `sous_dossiers` rend les dossiers d'un dossier (`root` compris), homonymes compris, avec leur marque, sans
//! rien creer ; `telecharger` rend le contenu tel qu'il est tenu ; un dossier mis a la corbeille n'est plus en place, ni
//! ce qu'il porte, et ne se liste plus.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use envois::drive::{Depose, Distant, Drive, EmpreinteMd5, ErreurGoogle, FichiersDuDossier, IdDrive, MarqueDuDossier, SousDossier};

pub struct FichierDrive {
    pub id: IdDrive,
    pub parent: IdDrive,
    pub nom: String,
    pub md5: Option<EmpreinteMd5>,
    pub contenu: Vec<u8>,
}

/// Une panne a la demande : `operation` (`chemin`, `lister`, `televerser`), un `motif` que doit contenir la cle
/// (le chemin du dossier, ou celui du fichier), et l'erreur rendue.
pub struct Panne {
    pub operation: &'static str,
    pub motif: String,
    pub erreur: ErreurGoogle,
}

#[derive(Default)]
pub struct FauxDrive {
    pub compte: Option<String>,
    /// id -> (parent, chemin complet depuis la racine)
    dossiers: Vec<(IdDrive, IdDrive, String)>,
    /// La marque de chaque dossier qui en porte une.
    pub marques: BTreeMap<IdDrive, MarqueDuDossier>,
    /// Les dossiers mis a la corbeille.
    pub corbeille: Vec<IdDrive>,
    pub fichiers: Vec<FichierDrive>,
    /// Chaque appel, en une ligne : `chemin a/b`, `lister a/b`, `televerser a/b/nom connu=ID|-`.
    pub journal: Vec<String>,
    suivant: u32,
    /// Un Drive qui ne donne pas d'empreinte (un fichier Google Docs).
    pub sans_md5: bool,
    pub pannes: Vec<Panne>,
    /// Fichiers locaux a effacer quand `televerser` du nom indique est appele, AVANT qu'il lise le contenu.
    pub effacer_au_televerser: Vec<(String, PathBuf)>,
    /// Fichiers locaux a effacer quand `lister` d'un dossier dont le chemin Drive contient le motif est appele.
    pub effacer_au_lister: Vec<(String, PathBuf)>,
}

impl FauxDrive {
    pub fn nouveau() -> FauxDrive {
        FauxDrive { compte: Some("compte@exemple.invalid".into()), ..Default::default() }
    }

    fn prochain(&mut self, lettre: &str) -> IdDrive {
        self.suivant += 1;
        IdDrive::lire(&format!("{}{}", lettre, self.suivant)).unwrap()
    }

    pub fn id_du_dossier(&self, chemin: &str) -> IdDrive {
        self.dossiers.iter().find(|(_, _, c)| c == chemin).map(|(i, _, _)| i.clone()).unwrap_or_else(|| panic!("pas de dossier {}", chemin))
    }

    fn chemin_de(&self, id: &IdDrive) -> String {
        self.dossiers.iter().find(|(i, _, _)| i == id).map(|(_, _, c)| c.clone()).unwrap_or_else(|| panic!("dossier inconnu : {}", id))
    }

    /// Un fichier deja la (hors de toute sauvegarde) : dans le dossier `chemin` (cree au besoin).
    pub fn deposer(&mut self, chemin: &str, nom: &str, contenu: &[u8], avec_md5: bool) -> IdDrive {
        let segments: Vec<&str> = chemin.split('/').collect();
        let parent = self.chemin(&IdDrive::RACINE, &segments).unwrap();
        let id = self.prochain("F");
        let md5 = avec_md5.then(|| EmpreinteMd5::des_octets(contenu));
        self.fichiers.push(FichierDrive { id: id.clone(), parent, nom: nom.into(), md5, contenu: contenu.to_vec() });
        id
    }

    /// Le contenu de tous les fichiers de ce chemin Drive (`a/b/nom`) : plusieurs si homonymes.
    pub fn contenus(&self, chemin_fichier: &str) -> Vec<String> {
        self.fichiers
            .iter()
            .filter(|f| format!("{}/{}", self.chemin_de(&f.parent), f.nom) == chemin_fichier)
            .map(|f| String::from_utf8_lossy(&f.contenu).into_owned())
            .collect()
    }

    /// Tous les fichiers : `chemin/nom` -> contenus, triee.
    pub fn arbre(&self) -> BTreeMap<String, Vec<String>> {
        let mut a: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for f in &self.fichiers {
            a.entry(format!("{}/{}", self.chemin_de(&f.parent), f.nom)).or_default().push(String::from_utf8_lossy(&f.contenu).into_owned());
        }
        a
    }

    /// Un second dossier de meme chemin que `chemin` (qui doit exister) : ce que deux postes qui sauvegardent en meme
    /// temps peuvent laisser dans le vrai Drive.
    pub fn dossier_homonyme(&mut self, chemin: &str) -> IdDrive {
        let parent = self.dossiers.iter().find(|(_, _, c)| c == chemin).map(|(_, p, _)| p.clone()).unwrap_or_else(|| panic!("pas de dossier {}", chemin));
        let id = self.prochain("D");
        self.dossiers.push((id.clone(), parent, chemin.to_string()));
        id
    }

    /// Le fichier de ce chemin Drive (`a/b/nom`), a modifier (le premier si homonymes).
    pub fn fichier_mut(&mut self, chemin_fichier: &str) -> &mut FichierDrive {
        let i = (0..self.fichiers.len())
            .find(|&i| format!("{}/{}", self.chemin_de(&self.fichiers[i].parent), self.fichiers[i].nom) == chemin_fichier)
            .unwrap_or_else(|| panic!("pas de fichier {}", chemin_fichier));
        &mut self.fichiers[i]
    }

    pub fn nombre_de_dossiers(&self) -> usize {
        self.dossiers.len()
    }

    /// Le nombre d'appels dont le journal commence par `debut`.
    pub fn appels(&self, debut: &str) -> usize {
        self.journal.iter().filter(|l| l.starts_with(debut)).count()
    }

    /// `id` est-il a la corbeille, lui ou un dossier qui le porte ?
    fn jete(&self, id: &IdDrive) -> bool {
        let mut i = id.clone();
        loop {
            if self.corbeille.contains(&i) {
                return true;
            }
            match self.dossiers.iter().find(|(d, _, _)| *d == i) {
                Some((_, p, _)) => i = p.clone(),
                None => return false,
            }
        }
    }

    /// Le chemin du dossier `id` (vide pour la racine du compte).
    fn chemin_ou_racine(&self, id: &IdDrive) -> String {
        if *id == IdDrive::RACINE {
            String::new()
        } else {
            self.chemin_de(id)
        }
    }

    fn panne(&self, operation: &str, cle: &str) -> Option<ErreurGoogle> {
        self.pannes.iter().find(|p| p.operation == operation && cle.contains(&p.motif)).map(|p| p.erreur.clone())
    }
}

/// Le lien qu'il donne d'un fichier depose : une adresse qui ne mene nulle part.
fn lien(id: &IdDrive) -> socle::garde_reseau::Url {
    socle::garde_reseau::Url::lire(&format!("http://faux-drive.invalid/lien/{}", id)).unwrap()
}

impl Drive for FauxDrive {
    fn compte(&self) -> Option<String> {
        self.compte.clone()
    }

    fn chemin(&mut self, depuis: &IdDrive, noms: &[&str]) -> Result<IdDrive, ErreurGoogle> {
        let mut complet = self.chemin_ou_racine(depuis);
        let cle = if complet.is_empty() { noms.join("/") } else { format!("{}/{}", complet, noms.join("/")) };
        self.journal.push(format!("chemin {}", cle));
        if let Some(e) = self.panne("chemin", &cle) {
            return Err(e);
        }
        let mut parent = depuis.clone();
        for nom in noms {
            if !complet.is_empty() {
                complet.push('/');
            }
            complet.push_str(nom);
            parent = match self.dossiers.iter().find(|(i, p, c)| *p == parent && *c == complet && !self.corbeille.contains(i)) {
                Some((i, _, _)) => i.clone(),
                None => {
                    let id = self.prochain("D");
                    self.dossiers.push((id.clone(), parent.clone(), complet.clone()));
                    id
                }
            };
        }
        Ok(parent)
    }

    fn lister(&mut self, parent: &IdDrive) -> Result<FichiersDuDossier, ErreurGoogle> {
        let chemin = self.chemin_de(parent);
        self.journal.push(format!("lister {}", chemin));
        if let Some(e) = self.panne("lister", &chemin) {
            return Err(e);
        }
        for (motif, p) in &self.effacer_au_lister {
            if chemin.contains(motif.as_str()) {
                let _ = std::fs::remove_file(p);
            }
        }
        let mut l = FichiersDuDossier::default();
        for f in self.fichiers.iter().filter(|f| f.parent == *parent) {
            l.ajouter(f.nom.clone(), Distant { id: f.id.clone(), md5: f.md5.clone(), octets: Some(socle::grandeurs::Octets::de(&f.contenu)) });
        }
        Ok(l)
    }

    fn televerser(&mut self, fichier: &Path, parent: &IdDrive, nom: Option<&str>, connu: Option<&Distant>) -> Result<Depose, ErreurGoogle> {
        let nom = nom.map(str::to_string).unwrap_or_else(|| fichier.file_name().unwrap().to_string_lossy().into_owned());
        let chemin = format!("{}/{}", self.chemin_de(parent), nom);
        self.journal.push(format!("televerser {} connu={}", chemin, connu.map(|c| c.id.texte()).unwrap_or("-")));
        for (n, p) in &self.effacer_au_televerser {
            if *n == nom {
                let _ = std::fs::remove_file(p);
            }
        }
        if let Some(e) = self.panne("televerser", &chemin) {
            return Err(e);
        }
        let contenu = std::fs::read(fichier).map_err(|e| ErreurGoogle::fichier(&e, fichier))?;
        let md5 = if self.sans_md5 { None } else { Some(EmpreinteMd5::du_fichier(fichier).unwrap()) };
        if let Some(c) = connu {
            let f = self.fichiers.iter_mut().find(|f| f.id == c.id).expect("`connu` doit exister dans le Drive");
            assert_eq!(f.parent, *parent, "on ne remplace qu'un fichier du dossier");
            f.contenu = contenu;
            f.md5 = md5;
            return Ok(Depose { id: Some(f.id.clone()), nom: Some(nom), lien: Some(lien(&f.id)), remplace: true });
        }
        let id = self.prochain("F");
        self.fichiers.push(FichierDrive { id: id.clone(), parent: parent.clone(), nom: nom.clone(), md5, contenu });
        Ok(Depose { id: Some(id.clone()), nom: Some(nom), lien: Some(lien(&id)), remplace: false })
    }

    fn sous_dossiers(&mut self, parent: &IdDrive) -> Result<Vec<SousDossier>, ErreurGoogle> {
        let chemin = if *parent == IdDrive::RACINE { IdDrive::RACINE.to_string() } else { self.chemin_de(parent) };
        self.journal.push(format!("sous_dossiers {}", chemin));
        if let Some(e) = self.panne("sous_dossiers", &chemin) {
            return Err(e);
        }
        Ok(self
            .dossiers
            .iter()
            .filter(|(i, p, _)| p == parent && !self.jete(i))
            .map(|(i, _, c)| SousDossier { nom: c.rsplit('/').next().unwrap_or(c).to_string(), id: i.clone(), marque: self.marques.get(i).cloned() })
            .collect())
    }

    fn creer_dossier(&mut self, nom: &str, parent: &IdDrive, marque: &MarqueDuDossier) -> Result<IdDrive, ErreurGoogle> {
        let base = self.chemin_ou_racine(parent);
        let complet = if base.is_empty() { nom.to_string() } else { format!("{}/{}", base, nom) };
        self.journal.push(format!("creer_dossier {}", complet));
        let id = self.prochain("D");
        self.dossiers.push((id.clone(), parent.clone(), complet));
        self.marques.insert(id.clone(), marque.clone());
        Ok(id)
    }

    fn marquer(&mut self, id: &IdDrive, marque: &MarqueDuDossier) -> Result<(), ErreurGoogle> {
        self.journal.push(format!("marquer {}", self.chemin_de(id)));
        let m = self.marques.entry(id.clone()).or_default();
        if marque.compagnie.is_some() {
            m.compagnie = marque.compagnie.clone();
        }
        if marque.derniere.is_some() {
            m.derniere = marque.derniere.clone();
        }
        if marque.octets.is_some() {
            m.octets = marque.octets.clone();
        }
        if marque.poste.is_some() {
            m.poste = marque.poste.clone();
        }
        if marque.main.is_some() {
            m.main = marque.main.clone();
        }
        Ok(())
    }

    fn en_place(&mut self, id: &IdDrive) -> Result<envois::drive::DossierEnPlace, ErreurGoogle> {
        use envois::drive::DossierEnPlace;
        self.journal.push(format!("en_place {}", self.chemin_de(id)));
        Ok(if self.jete(id) { DossierEnPlace::Retire } else { DossierEnPlace::EnPlace(self.marques.get(id).cloned()) })
    }

    fn telecharger(&mut self, id: &IdDrive) -> Result<Vec<u8>, ErreurGoogle> {
        let f = self.fichiers.iter().find(|f| f.id == *id).unwrap_or_else(|| panic!("fichier inconnu : {}", id));
        let chemin = format!("{}/{}", self.chemin_de(&f.parent), f.nom);
        let contenu = f.contenu.clone();
        self.journal.push(format!("telecharger {}", chemin));
        if let Some(e) = self.panne("telecharger", &chemin) {
            return Err(e);
        }
        Ok(contenu)
    }
}

impl FauxDrive {
    /// Ce que fait la corbeille du Drive (`envois::drive::Corbeille::mettre`) : le dossier `id` y va, avec ce qu'il porte.
    pub fn mettre_a_la_corbeille(&mut self, id: &IdDrive) {
        self.journal.push(format!("corbeille {}", self.chemin_de(id)));
        self.corbeille.push(id.clone());
    }
}

// ------------------------------------------------------------ le disque local

pub fn racine(prefixe: &str, nom: &str) -> PathBuf {
    let r = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{}-{}", prefixe, nom));
    let _ = std::fs::remove_dir_all(&r);
    std::fs::create_dir_all(&r).unwrap();
    r
}

/// LE POSTE DES TESTS, tel que la main le nomme sur le Drive : sa cle (32 octets inventes, les memes a chaque appel, donc
/// le meme poste) est posee dans un dossier de test a lui, un par appel (`nom`, le processus et un compteur : deux tests
/// en meme temps ne se marchent pas dessus), jamais dans celui de la machine.
pub fn ce_poste(nom: &str) -> compagnie::sauvegarde::PosteSurLeDrive {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let d = racine("poste", &format!("{}-{}-{}", nom, std::process::id(), n));
    std::fs::write(d.join(compagnie::coffre::FICHIER_CLE), [7u8; 32]).unwrap();
    compagnie::sauvegarde::PosteSurLeDrive::du_coffre(&compagnie::coffre::Coffre::sur(&d, &d, socle::temps::secondes().a_la_minute())).unwrap()
}

pub fn ecrire(racine: &Path, relatif: &str, contenu: &str) {
    let p = racine.join(relatif);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, contenu).unwrap();
    fixer_date(&p, 1_700_000_000, 0);
}

/// La date de modification d'un fichier : `secondes` et `nanos` apres 1970.
pub fn fixer_date(p: &Path, secondes: u64, nanos: u32) {
    let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
    f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::new(secondes, nanos)).unwrap();
}
