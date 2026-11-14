//! Le tri de la boite aux lettres `import/`. Une compagnie y met tout ce qu'elle a -- des
//! PDF (un bulletin par fichier, une annee par fichier), des DSN, des AEM, des zip, des doublons, des
//! fichiers qui n'ont rien a y faire -- et le moteur TRIE, PAR LE CONTENU, JAMAIS PAR LE NOM : le
//! « 2023.pdf » qui contient 2026 le prouve.
//!
//! Chaque fichier recoit une sorte, et une seule :
//!
//! | sorte | ce que le contenu dit |
//! |---|---|
//! | `dsn` | l'en-tete `S10.G00.00.001,` (`declarations::dsn_lue::est_une_dsn`), et un mois declare (`dsn_lue::lire_une_dsn`) |
//! | `aem` | chaque enregistrement fait 1000 caracteres, commence par `AEMEDIC2` et porte ses dates aux positions que `importation::lire_aem` lit |
//! | `pdf_texte` | un PDF (`%PDF-`) que la fabrique ouvre, et dont au moins une page a des mots |
//! | `pdf_sans_texte` | un PDF que la fabrique ouvre, sans un seul mot (lettres en traces, ou page scannee : pas d'OCR) |
//! | `archive` | un zip, ouvert dans le dossier de travail ; son contenu est trie comme le reste |
//! | `doublon` | la meme empreinte SHA-256 qu'un fichier deja trie (`doublon_de` dit lequel) |
//! | `illisible` | un PDF que la fabrique n'ouvre pas, un zip qui ne s'ouvre pas, ce qui commence comme une DSN ou une AEM sans en avoir la forme |
//! | `autre` | tout le reste, avec son extension |
//!
//! RIEN NE SE DEVINE (regle 1) : un fichier qu'on ne reconnait pas est `autre`, pas « sans doute une DSN ».
//! Rien n'est ecrit dans les donnees de la compagnie : seuls les zip s'ouvrent, et dans `travail/`
//! seulement, un sous-dossier par archive ; une entree dont le nom ne fait pas un chemin sous l'archive
//! (`CheminRelatif::ecrit_ailleurs`) ne s'ecrit pas, elle est mise de cote (`MiseDeCote`).

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use socle::fichiers::{CheminRelatif, Empreinte, NomDeFichier};
use socle::texte::{lignes_du_texte, texte_latin1};
use socle::echec::R;

/// Une archive dans une archive dans une archive... s'ouvre jusqu'a cette profondeur, pas au-dela.
const PROFONDEUR_MAX: usize = 4;

/// Ce qu'une archive peut rendre une fois decompressee, en tout : au-dela, elle n'est pas ouverte (une
/// « bombe » de zip remplirait le disque).
const DECOMPRESSE_MAX: u64 = 2 * 1024 * 1024 * 1024;

/// Le gabarit AEM que l'import lit (`importation::lire_aem`) : 1000 caracteres par enregistrement.
const AEM_LONGUEUR: usize = 1000;
const AEM_MARQUE: &str = "AEMEDIC2";

/// Ce que l'import accepte, dit a l'ecran (le guide) et a chaque fichier refuse.
pub const ACCEPTE: &str = "L'import accepte les PDF de bulletins de paie, d'AEM, de certificats Congés Spectacles et d'AER Cinéma Spectacle, et les fichiers DSN et AEM déposés.";

/// La raison d'un PDF sans texte : une image ne se lit pas.
pub const IMAGE: &str = "ce document est une image ; déposez le PDF d'origine";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sorte {
    Dsn,
    Aem,
    PdfTexte,
    PdfSansTexte,
    Archive,
    Doublon,
    Illisible,
    Autre,
}

impl Sorte {
    pub fn nom(self) -> &'static str {
        match self {
            Sorte::Dsn => "dsn",
            Sorte::Aem => "aem",
            Sorte::PdfTexte => "pdf_texte",
            Sorte::PdfSansTexte => "pdf_sans_texte",
            Sorte::Archive => "archive",
            Sorte::Doublon => "doublon",
            Sorte::Illisible => "illisible",
            Sorte::Autre => "autre",
        }
    }
}

/// Une page d'un PDF, telle que la fabrique l'a relevee (`fabrique_pdf::inspection`).
#[derive(Clone, Debug, serde::Serialize)]
pub struct PageLue {
    pub mots: usize,
    pub polices: usize,
    pub images: usize,
    pub traits: usize,
}

/// Un fichier trie.
#[derive(Clone, Debug)]
pub struct Fichier {
    /// Le chemin relatif au dossier trie ; dans une archive : `lot.zip/sous/fichier.dsn`.
    pub chemin: CheminRelatif,
    /// Ou il se lit sur le disque (sous `travail/` pour ce qui sort d'une archive).
    pub disque: PathBuf,
    pub sorte: Sorte,
    pub taille: u64,
    /// Son empreinte ; aucune pour une entree d'archive qui n'a pas ete extraite.
    pub empreinte: Option<Empreinte>,
    /// Les pages d'un PDF ouvert ; vide pour le reste.
    pub pages: Vec<PageLue>,
    /// Pourquoi, en une phrase, quand il y a quelque chose a dire.
    pub raison: Option<String>,
    /// Le fichier dont celui-ci est la copie exacte.
    pub doublon_de: Option<CheminRelatif>,
    /// L'archive d'ou il sort.
    pub archive: Option<CheminRelatif>,
    /// Le mois qu'une DSN declare.
    pub mois: Option<socle::mois::Mois>,
}

/// UNE ENTREE MISE DE COTE : un nom qui ne fait pas un chemin sous le dossier trie, ou une entree qui ne se lit pas.
/// Elle ne s'ecrit jamais sur le disque ; le compte rendu la dit (`dit`), et le reste se trie.
#[derive(Clone, Debug)]
pub struct MiseDeCote {
    /// Le dossier ou l'archive qui la porte ; aucun a la racine du dossier trie.
    pub dans: Option<CheminRelatif>,
    /// Son nom tel qu'il est ecrit ; une entree qui ne se lit pas n'en a pas : « entrée n° N ».
    pub nom: String,
    pub raison: RaisonDeMiseDeCote,
}

/// POURQUOI UNE ENTREE EST MISE DE COTE, avec sa phrase.
#[derive(Clone, Debug)]
pub enum RaisonDeMiseDeCote {
    /// Le nom d'une entree d'archive ne fait pas un chemin sous l'archive (`CheminRelatif::ecrit_ailleurs`).
    HorsDeLArchive,
    /// Le nom d'un fichier du disque n'est pas un `NomDeFichier`.
    PasUnNom,
    /// L'entree ne se lit pas : l'erreur, telle que le disque ou l'archive la dit.
    Illisible(String),
}

impl std::fmt::Display for RaisonDeMiseDeCote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RaisonDeMiseDeCote::HorsDeLArchive => f.write_str("son nom n'est pas un chemin sous l'archive (il remonte par « .. », porte « : », ne nomme rien, ou il est absolu)"),
            RaisonDeMiseDeCote::PasUnNom => f.write_str("son nom n'est pas un nom de fichier (il porte « : » ou une barre inverse)"),
            RaisonDeMiseDeCote::Illisible(e) => write!(f, "l'entrée ne se lit pas ({})", e),
        }
    }
}

impl MiseDeCote {
    /// « « ../x.dsn » (dans lot.zip) est mis de côté, sans être lu : … ».
    pub fn dit(&self) -> String {
        let dans: String = self.dans.iter().map(|d| format!(" (dans {})", d)).collect();
        format!("« {} »{} est mis de côté, sans être lu : {}.", self.nom, dans, self.raison)
    }
}

/// Le tri d'un dossier : chaque fichier, dans l'ordre du parcours (noms tries, une archive suivie de
/// son contenu), et les entrees mises de cote.
#[derive(Clone, Debug)]
pub struct Tri {
    pub dossier: PathBuf,
    pub fichiers: Vec<Fichier>,
    pub mises_de_cote: Vec<MiseDeCote>,
}

/// `trier(dossier, travail)` : parcourt `dossier` et ses sous-dossiers, ouvre chaque zip dans `travail/`
/// (cree s'il manque ; s'il est sous `dossier`, il n'est pas parcouru), et donne a chaque fichier sa sorte.
pub fn trier(dossier: &Path, travail: &Path) -> R<Tri> {
    if !dossier.is_dir() {
        let a_part = socle::echec::Lieu { chemin: Some(dossier.to_string_lossy().into_owned()), ..Default::default() };
        return Err(socle::echec::refus_avec("Le dossier à trier n'existe pas.", a_part).reclasser(socle::echec::Classe::Refuse));
    }
    std::fs::create_dir_all(travail).map_err(|e| socle::echec::du_disque(travail, &e).reclasser(socle::echec::Classe::Refuse))?;
    let travail_canon = std::fs::canonicalize(travail).map_err(|e| socle::echec::du_disque(travail, &e).reclasser(socle::echec::Classe::Disque))?;
    let mut t = Trieur { travail: travail.to_path_buf(), travail_canon, vus: HashMap::new(), fichiers: Vec::new(), mises_de_cote: Vec::new() };
    t.parcourir(dossier, None, None, 0)?;
    Ok(Tri { dossier: dossier.to_path_buf(), fichiers: t.fichiers, mises_de_cote: t.mises_de_cote })
}

struct Trieur {
    travail: PathBuf,
    travail_canon: PathBuf,
    /// Empreinte -> chemin du premier fichier qui l'a portee.
    vus: HashMap<Empreinte, CheminRelatif>,
    fichiers: Vec<Fichier>,
    mises_de_cote: Vec<MiseDeCote>,
}

impl Trieur {
    fn nouveau(&self, chemin: CheminRelatif, disque: PathBuf, archive: Option<&CheminRelatif>) -> Fichier {
        Fichier {
            chemin,
            disque,
            sorte: Sorte::Autre,
            taille: 0,
            empreinte: None,
            pages: Vec::new(),
            raison: None,
            doublon_de: None,
            archive: archive.cloned(),
            mois: None,
        }
    }

    /// Les entrees d'un dossier, par nom, sous-dossiers compris ; `prefixe` est son chemin sous le dossier trie (aucun
    /// pour le dossier trie lui-meme). Le dossier trie qui ne se lit pas : son erreur remonte ; un sous-dossier se dit
    /// illisible ; une entree qui ne se lit pas est mise de cote.
    fn parcourir(&mut self, dossier: &Path, prefixe: Option<&CheminRelatif>, archive: Option<&CheminRelatif>, profondeur: usize) -> R<()> {
        let lues = match (std::fs::read_dir(dossier), prefixe) {
            (Ok(l), _) => l,
            (Err(e), None) => return Err(socle::echec::du_disque(dossier, &e).reclasser(socle::echec::Classe::Disque)),
            (Err(e), Some(p)) => {
                let mut f = self.nouveau(p.clone(), dossier.to_path_buf(), archive);
                f.sorte = Sorte::Illisible;
                f.raison = Some(format!("dossier qui ne se lit pas ({})", e));
                self.fichiers.push(f);
                return Ok(());
            }
        };
        let mut entrees: Vec<(String, PathBuf, std::fs::FileType)> = Vec::new();
        for (i, e) in lues.enumerate() {
            let e = match e {
                Ok(e) => e,
                Err(x) => {
                    self.mises_de_cote.push(MiseDeCote { dans: prefixe.cloned(), nom: format!("entrée n° {}", i + 1), raison: RaisonDeMiseDeCote::Illisible(x.to_string()) });
                    continue;
                }
            };
            let nom = e.file_name().to_string_lossy().into_owned();
            match e.file_type() {
                Ok(genre) => entrees.push((nom, e.path(), genre)),
                Err(x) => self.mises_de_cote.push(MiseDeCote { dans: prefixe.cloned(), nom, raison: RaisonDeMiseDeCote::Illisible(x.to_string()) }),
            }
        }
        entrees.sort_by(|a, b| a.0.cmp(&b.0));
        for (nom, chemin, genre) in entrees {
            let nom_lu = match NomDeFichier::lire(&nom) {
                Ok(n) => n,
                Err(_) => {
                    self.mises_de_cote.push(MiseDeCote { dans: prefixe.cloned(), nom, raison: RaisonDeMiseDeCote::PasUnNom });
                    continue;
                }
            };
            let rel = match prefixe {
                Some(p) => p.joint(&nom_lu),
                None => CheminRelatif::from(nom_lu),
            };
            if genre.is_dir() {
                // Le dossier de travail, s'il est sous le dossier trie, ne se trie pas.
                match std::fs::canonicalize(&chemin) {
                    Ok(c) if c == self.travail_canon => continue,
                    Ok(_) => {}
                    Err(e) => {
                        let mut f = self.nouveau(rel, chemin, archive);
                        f.sorte = Sorte::Illisible;
                        f.raison = Some(format!("dossier qui ne se lit pas ({})", e));
                        self.fichiers.push(f);
                        continue;
                    }
                }
                self.parcourir(&chemin, Some(&rel), archive, profondeur)?;
            } else if genre.is_symlink() {
                let mut f = self.nouveau(rel, chemin, archive);
                f.raison = Some("lien symbolique : il n'est pas suivi".into());
                self.fichiers.push(f);
            } else {
                self.classer(rel, chemin, archive, profondeur)?;
            }
        }
        Ok(())
    }

    /// Un fichier : son empreinte, puis ce que son contenu dit.
    fn classer(&mut self, rel: CheminRelatif, disque: PathBuf, archive: Option<&CheminRelatif>, profondeur: usize) -> R<()> {
        let mut f = self.nouveau(rel, disque, archive);
        let octets = match std::fs::read(&f.disque) {
            Ok(o) => o,
            Err(e) => {
                f.sorte = Sorte::Illisible;
                f.raison = Some(format!("le fichier ne se lit pas ({})", e));
                self.fichiers.push(f);
                return Ok(());
            }
        };
        f.taille = octets.len() as u64;
        let empreinte = Empreinte::des_octets(&octets);
        f.empreinte = Some(empreinte.clone());
        if let Some(premier) = self.vus.get(&empreinte) {
            f.sorte = Sorte::Doublon;
            f.doublon_de = Some(premier.clone());
            f.raison = Some(format!("copie exacte de {}", premier));
            self.fichiers.push(f);
            return Ok(());
        }
        self.vus.insert(empreinte.clone(), f.chemin.clone());
        if octets.is_empty() {
            f.raison = Some("fichier vide".into());
            self.fichiers.push(f);
            return Ok(());
        }
        if est_un_zip(&octets) {
            return self.ouvrir_archive(f, &octets, &empreinte, profondeur);
        }
        if est_un_pdf(&octets) {
            lire_pdf(&mut f);
        } else if declarations::dsn_lue::est_une_dsn(&octets) {
            match declarations::dsn_lue::lire_une_dsn(&f.disque) {
                Ok(Some(declarations::dsn_lue::DsnLue { mois: Some(m), .. })) => {
                    f.sorte = Sorte::Dsn;
                    f.mois = Some(m);
                }
                // Une DSN fautive reste une DSN : sa lecture refuse l'import, et dit quoi corriger (`dsn_aem::lire_dsn`).
                Err(e) => {
                    f.sorte = Sorte::Dsn;
                    f.raison = Some(e.phrase);
                }
                Ok(_) => {
                    f.sorte = Sorte::Illisible;
                    f.raison = Some("commence comme une DSN, mais ne dit pas son mois (S20.G00.05.005)".into());
                }
            }
        } else if apres_bom(&octets).starts_with(b"AEMEDI") {
            lire_aem(&mut f, &octets);
        } else {
            f.raison = Some(format!("ce fichier n'est ni un PDF, ni une DSN, ni une AEM, ni une archive zip. {}", ACCEPTE));
        }
        self.fichiers.push(f);
        Ok(())
    }

    /// Un zip : ouvert dans son propre sous-dossier de `travail/`, puis son contenu trie a la suite.
    fn ouvrir_archive(&mut self, mut f: Fichier, octets: &[u8], empreinte: &Empreinte, profondeur: usize) -> R<()> {
        if profondeur >= PROFONDEUR_MAX {
            f.raison = Some(format!("zip rangé à plus de {} niveaux de zips l'un dans l'autre : pas ouvert", PROFONDEUR_MAX));
            self.fichiers.push(f);
            return Ok(());
        }
        let mut z = match zip::ZipArchive::new(std::io::Cursor::new(octets)) {
            Ok(z) => z,
            Err(e) => {
                f.sorte = Sorte::Illisible;
                f.raison = Some(format!("archive zip qui ne s'ouvre pas ({})", e));
                self.fichiers.push(f);
                return Ok(());
            }
        };
        let assaini: String = f.chemin.nom().texte().chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
        let cible = self.travail.join(format!("{}-{}", assaini, &empreinte.texte()[..12]));
        let pret = socle::fichiers::effacer(&cible).and_then(|()| std::fs::create_dir_all(&cible).map_err(|e| socle::echec::du_disque(&cible, &e)));
        if let Err(e) = pret {
            f.sorte = Sorte::Illisible;
            f.raison = Some(format!("le dossier de travail de l'archive ne se cree pas ({})", e.phrase));
            self.fichiers.push(f);
            return Ok(());
        }
        // Les entrees qui ne s'extraient pas : elles ont leur sort elles aussi, apres l'archive.
        let mut refusees: Vec<Fichier> = Vec::new();
        let mut extraites = 0usize;
        let mut total: u64 = 0;
        for i in 0..z.len() {
            let mut e = match z.by_index(i) {
                Ok(e) => e,
                Err(err) => {
                    self.mises_de_cote.push(MiseDeCote { dans: Some(f.chemin.clone()), nom: format!("entrée n° {}", i + 1), raison: RaisonDeMiseDeCote::Illisible(err.to_string()) });
                    continue;
                }
            };
            // Le nom de l'entree, tel qu'un autre systeme l'ecrit (la barre finale d'un dossier, `/` ou `\`, se retire) :
            // un chemin sous l'archive, ou elle ne s'ecrit pas.
            let brut = e.name().to_string();
            let rel = match CheminRelatif::ecrit_ailleurs(&brut) {
                Ok(r) => r,
                Err(_) => {
                    self.mises_de_cote.push(MiseDeCote { dans: Some(f.chemin.clone()), nom: brut, raison: RaisonDeMiseDeCote::HorsDeLArchive });
                    continue;
                }
            };
            let chemin = rel.dans(&cible);
            if e.is_dir() {
                if let Err(x) = std::fs::create_dir_all(&chemin) {
                    let mut r = self.nouveau(f.chemin.suivi(&rel), chemin.clone(), Some(&f.chemin));
                    r.sorte = Sorte::Illisible;
                    r.raison = Some(format!("dossier d'archive qui ne se cree pas ({})", x));
                    refusees.push(r);
                }
                continue;
            }
            let mut contenu = Vec::new();
            let lu = (&mut e).take(DECOMPRESSE_MAX - total + 1).read_to_end(&mut contenu);
            let ecrit = lu.map_err(|x| x.to_string()).and_then(|n| {
                total += n as u64;
                if total > DECOMPRESSE_MAX {
                    return Err(format!("l'archive rend plus de {} Mo decompresses", DECOMPRESSE_MAX / (1024 * 1024)));
                }
                if let Some(p) = chemin.parent() {
                    std::fs::create_dir_all(p).map_err(|x| x.to_string())?;
                }
                std::fs::write(&chemin, &contenu).map_err(|x| x.to_string())
            });
            match ecrit {
                Ok(()) => extraites += 1,
                Err(m) => {
                    let mut r = self.nouveau(f.chemin.suivi(&rel), chemin.clone(), Some(&f.chemin));
                    r.sorte = Sorte::Illisible;
                    r.taille = e.size();
                    r.raison = Some(match socle::fichiers::effacer(&chemin) {
                        Ok(()) => format!("entree d'archive qui ne s'extrait pas ({})", m),
                        Err(n) => format!("entree d'archive qui ne s'extrait pas ({}), et ce qui en est sorti ne s'efface pas ({})", m, n.phrase),
                    });
                    refusees.push(r);
                    if total > DECOMPRESSE_MAX {
                        break;
                    }
                }
            }
        }
        f.sorte = Sorte::Archive;
        f.raison = Some(format!("zip ouvert : {} fichier(s) extrait(s)", extraites));
        let chemin_archive = f.chemin.clone();
        self.fichiers.push(f);
        self.fichiers.extend(refusees);
        self.parcourir(&cible, Some(&chemin_archive), Some(&chemin_archive), profondeur + 1)
    }
}

/// Les marques d'ordre d'octets UTF-8 en tete.
fn apres_bom(mut o: &[u8]) -> &[u8] {
    while o.starts_with(b"\xef\xbb\xbf") {
        o = &o[3..];
    }
    o
}

/// Un zip commence par un en-tete local (`PK\3\4`), ou, vide, par la fin du repertoire central (`PK\5\6`).
fn est_un_zip(o: &[u8]) -> bool {
    o.starts_with(b"PK\x03\x04") || o.starts_with(b"PK\x05\x06")
}

/// L'en-tete `%PDF-` se trouve dans les 1024 premiers octets (ISO 32000-1, annexe H : les lecteurs le
/// cherchent la, pas seulement au tout debut).
fn est_un_pdf(o: &[u8]) -> bool {
    o[..o.len().min(1024)].windows(5).any(|w| w == b"%PDF-")
}

/// Un PDF : la fabrique l'ouvre (dans le meme processus) et releve chaque page.
fn lire_pdf(f: &mut Fichier) {
    let r = match crate::releve::du_pdf(&f.disque) {
        Ok(r) => r,
        Err(crate::releve::Refus::Fabrique(r)) => {
            f.sorte = Sorte::Illisible;
            // Le refus de la fabrique commence par le chemin du disque : seule la cause compte ici.
            let cause = match r.phrase.strip_prefix(&*f.disque.to_string_lossy()) {
                Some(c) => c,
                None => r.phrase.as_str(),
            }
            .trim_start_matches([' ', ':'])
            .to_string();
            f.raison = Some(match r.classe {
                fabrique_pdf::Classe::PieceIllisible | fabrique_pdf::Classe::ACorriger => format!("PDF que la fabrique n'ouvre pas : {}", cause),
                fabrique_pdf::Classe::DefautDuLogiciel => format!("PDF que la fabrique n'ouvre pas, par un défaut du logiciel à signaler au développeur : {}", cause),
            });
            return;
        }
        Err(crate::releve::Refus::Releve(e)) => {
            f.sorte = Sorte::Illisible;
            f.raison = Some(format!("PDF dont le releve ne se lit pas ({})", e.phrase));
            return;
        }
    };
    let avertissements = &r.avertissements;
    f.pages = r.releve.pages.iter().map(|p| PageLue { mots: p.mots.len(), polices: p.polices.len(), images: p.images.len(), traits: p.traits.len() }).collect();
    let avec = f.pages.iter().filter(|p| p.mots > 0).count();
    let total = f.pages.len();
    let mut raisons: Vec<String> = Vec::new();
    if total == 0 {
        f.sorte = Sorte::PdfSansTexte;
        raisons.push("PDF sans aucune page".into());
    } else if avec == 0 {
        f.sorte = Sorte::PdfSansTexte;
        raisons.push(IMAGE.into());
    } else {
        f.sorte = Sorte::PdfTexte;
        if avec < total {
            raisons.push(format!("{} page(s) sur {} sans aucun mot", total - avec, total));
        }
    }
    if !avertissements.is_empty() {
        raisons.push(format!("la fabrique a signale : {}", avertissements.join(" ; ")));
    }
    if !raisons.is_empty() {
        f.raison = Some(raisons.join(" ; "));
    }
}

/// Une AEM : chaque enregistrement (ligne non vide) a la forme que `importation::lire_aem` lit -- 1000
/// caracteres, la marque `AEMEDIC2`, des dates de debut et de fin AAAAMMJJ aux positions 618 et 627.
fn lire_aem(f: &mut Fichier, octets: &[u8]) {
    let lignes: Vec<Vec<char>> =
        lignes_du_texte(&texte_latin1(apres_bom(octets))).into_iter().filter(|l| !l.is_empty()).map(|l| l.chars().collect()).collect();
    let date = |l: &[char], a: usize| socle::piece::Zone::lue(&l[a..a + 8].iter().collect::<String>(), socle::piece::format::Aaaammjj).valeur().is_some();
    for (i, l) in lignes.iter().enumerate() {
        let marque: String = l.iter().take(AEM_MARQUE.len()).collect();
        let defaut = if marque != AEM_MARQUE {
            Some(format!("ne porte pas la marque {} (« {} »)", AEM_MARQUE, marque))
        } else if l.len() != AEM_LONGUEUR {
            Some(format!("fait {} caracteres au lieu de {}", l.len(), AEM_LONGUEUR))
        } else if !date(l, 618) || !date(l, 627) {
            Some("n'a pas ses dates de debut et de fin (positions 618 et 627, AAAAMMJJ)".to_string())
        } else {
            None
        };
        if let Some(d) = defaut {
            f.sorte = Sorte::Illisible;
            f.raison = Some(format!("commence comme une AEM, mais l'enregistrement {} {}", i + 1, d));
            return;
        }
    }
    f.sorte = Sorte::Aem;
}

#[cfg(test)]
mod tests {
    use super::{trier, RaisonDeMiseDeCote, Tri};
    use std::io::Write;

    /// Un dossier `import/` qui ne porte que `lot.zip`, fait des entrees `entrees`, trie.
    fn trier_le_zip(test: &str, entrees: &[&str]) -> (std::path::PathBuf, Tri) {
        let d = std::env::temp_dir().join(format!("puppetpaie-tri-{}-{}", test, std::process::id()));
        if d.exists() {
            std::fs::remove_dir_all(&d).unwrap();
        }
        let import = d.join("import");
        std::fs::create_dir_all(&import).unwrap();
        let mut z = zip::ZipWriter::new(std::fs::File::create(import.join("lot.zip")).unwrap());
        for nom in entrees {
            z.start_file(*nom, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)).unwrap();
            if !nom.ends_with('\\') {
                z.write_all(nom.as_bytes()).unwrap();
            }
        }
        z.finish().unwrap();
        let tri = trier(&import, &d.join("travail")).unwrap();
        (d, tri)
    }

    /// Le `Compress-Archive` de PowerShell ecrit ses entrees avec des barres inverses (`sous\paie.dsn`) : le zip
    /// s'importe, son contenu sous ses dossiers ; `./a`, `a//b` se lisent `a`, `a/b`.
    #[test]
    fn un_zip_fait_sous_windows_s_importe() {
        let (d, tri) = trier_le_zip("windows", &["sous\\", "sous\\paie.txt", "./a.txt", "b//c.txt"]);
        let chemins: Vec<String> = tri.fichiers.iter().map(|f| f.chemin.to_string()).collect();
        std::fs::remove_dir_all(&d).unwrap();
        assert!(tri.mises_de_cote.is_empty(), "{:?}", tri.mises_de_cote);
        assert_eq!(chemins, ["lot.zip", "lot.zip/a.txt", "lot.zip/b/c.txt", "lot.zip/sous/paie.txt"]);
    }

    /// Une entree qui porterait un prefixe de lecteur (`C:x`, `C:\x`), qui remonte ou qui est absolue est mise de cote,
    /// et rien ne s'ecrit hors du dossier de l'archive : sous Windows, joindre `C:x` remplacerait tout le chemin.
    #[test]
    fn une_entree_piegee_ne_s_ecrit_pas_hors_de_l_archive() {
        let (d, tri) = trier_le_zip("piege", &["C:x.txt", "C:\\x.txt", "..\\x.txt", "\\x.txt", "bon.txt"]);
        let chemins: Vec<String> = tri.fichiers.iter().map(|f| f.chemin.to_string()).collect();
        let hors = [d.join("x.txt"), d.join("import").join("x.txt"), d.join("travail").join("x.txt")].iter().any(|p| p.exists());
        std::fs::remove_dir_all(&d).unwrap();
        assert_eq!(chemins, ["lot.zip", "lot.zip/bon.txt"]);
        assert_eq!(tri.mises_de_cote.len(), 4);
        assert!(tri.mises_de_cote.iter().all(|m| matches!(m.raison, RaisonDeMiseDeCote::HorsDeLArchive)));
        assert!(!hors);
    }
}
