//! La description validée : la forme donnée à la fabrique (`forme.rs`) devient ce que la mise en
//! page emploie, chaque préséance résolue (la valeur de ce qui contient, que la forme nomme).
//!
//! Toute erreur sort en une ligne de français, avec le numéro du bloc quand on le connaît.

use std::collections::BTreeMap;

use crate::forme as f;
use crate::modele;
use crate::style_contrat;

pub use crate::forme::{Alignement, PoliceFixe};
pub use crate::mesure::Mesure;

/// À qui revient un refus de la fabrique : son lecteur lit cette classe, jamais la phrase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classe {
    /// Un défaut du logiciel : ce que la fabrique tient d'elle-même ou du moteur ne tient pas (un modèle livré qui
    /// sort de sa page, une police interne illisible, un PDF qui ne s'écrit pas) ; à signaler au développeur.
    DefautDuLogiciel,
    /// Une pièce reçue (un PDF, une image, un modèle de contrat) que la fabrique ne lit pas, ou qui porte ce qu'elle
    /// ne connaît pas : la phrase la nomme.
    PieceIllisible,
    /// Ce que l'appelant doit corriger : la description, le remplissage ou les champs demandés.
    ACorriger,
}

/// Un refus : sa classe, et sa phrase, une ligne de français prête à être affichée telle quelle.
#[derive(Debug, Clone, PartialEq)]
pub struct Refus {
    pub classe: Classe,
    pub phrase: String,
}

impl Refus {
    pub fn defaut(phrase: impl Into<String>) -> Refus {
        Refus { classe: Classe::DefautDuLogiciel, phrase: phrase.into() }
    }
    pub fn illisible(phrase: impl Into<String>) -> Refus {
        Refus { classe: Classe::PieceIllisible, phrase: phrase.into() }
    }
    pub fn a_corriger(phrase: impl Into<String>) -> Refus {
        Refus { classe: Classe::ACorriger, phrase: phrase.into() }
    }
    /// Le même refus, sa phrase recomposée par `f` (« pièce 2 : <la phrase> ») : sa classe reste.
    pub fn reformuler(self, f: impl FnOnce(&str) -> String) -> Refus {
        Refus { phrase: f(&self.phrase), ..self }
    }
}

impl std::fmt::Display for Refus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.phrase)
    }
}

impl std::error::Error for Refus {}

/// Ce que l'appelant doit corriger (`Classe::ACorriger`).
pub fn refus<T>(message: impl Into<String>) -> Result<T, Refus> {
    Err(Refus::a_corriger(message))
}

/// Une pièce reçue qui ne se lit pas (`Classe::PieceIllisible`).
pub fn illisible<T>(message: impl Into<String>) -> Result<T, Refus> {
    Err(Refus::illisible(message))
}

/// Un défaut du logiciel (`Classe::DefautDuLogiciel`).
pub fn defaut<T>(message: impl Into<String>) -> Result<T, Refus> {
    Err(Refus::defaut(message))
}

/// Le numéro du format que cette fabrique parle (`--version`). Le 2 ajoute les documents « modèle »
/// (FORMAT.md), le 3 les modes de lecture et de retouche de PDF existants (`--pages`, `--texte`,
/// `--marquer`, `--verrouiller`) et le champ facultatif `largeur_max` de `texte_fixe`. Une
/// description dit chaque valeur (FORMAT.md) quel que soit son numéro.
pub const FORMAT: i64 = 3;

/// Le corps en dessous duquel `largeur_max` ne réduit pas un texte (6 pt : sous
/// cette taille la case est illisible, et c'est le signe que la donnée ne va pas).
pub const CORPS_PLANCHER: Mesure = Mesure::entier(6);

// ---------------------------------------------------------------------------
// Ce qui est validé.
// ---------------------------------------------------------------------------

pub type Rvb = (u8, u8, u8);

#[derive(Debug, Clone)]
pub struct Page {
    pub largeur: Mesure,
    pub hauteur: Mesure,
    pub haut: Mesure,
    pub droite: Mesure,
    pub bas: Mesure,
    pub gauche: Mesure,
}

impl Page {
    pub fn largeur_utile(&self) -> Mesure {
        self.largeur - self.gauche - self.droite
    }
    pub fn hauteur_utile(&self) -> Mesure {
        self.hauteur - self.haut - self.bas
    }
}

#[derive(Debug, Clone)]
pub struct Police {
    pub taille: Mesure,
    pub interligne: Mesure,
    pub couleur: Rvb,
}

#[derive(Debug, Clone)]
pub struct Logo {
    pub chemin: String,
    pub largeur: Mesure,
    pub marge_haut: Mesure,
    pub marge_droite: Mesure,
}

/// Un en-tête ou un pied de page : trois textes d'une ligne (`{page}` et `{pages}` y sont remplacés
/// par le numéro de la page et par le nombre de pages), à `distance` points du bord de la page (la
/// ligne de base). Un texte absent ou vide n'écrit rien.
#[derive(Debug, Clone)]
pub struct Repere {
    pub gauche: Option<String>,
    pub centre: Option<String>,
    pub droite: Option<String>,
    pub taille: Mesure,
    pub couleur: Rvb,
    pub gras: bool,
    pub distance: Mesure,
    pub premiere_page: bool,
    /// Une image posée au bout de la ligne, à droite, après le texte de droite.
    pub image: Option<ImageRepere>,
}

/// L'image d'un repère : `largeur` en points, hauteur selon les proportions ; son bas descend de
/// `descente` points sous la ligne de base (la jambe d'un mot dessiné).
#[derive(Debug, Clone)]
pub struct ImageRepere {
    pub chemin: String,
    pub largeur: Mesure,
    pub descente: Mesure,
}

impl Repere {
    /// Le total de pages ne se connaît qu'après une première mise en page (FORMAT.md).
    pub fn emploie_total(&self) -> bool {
        [&self.gauche, &self.centre, &self.droite].into_iter().flatten().any(|t| t.contains("{pages}"))
    }
}

#[derive(Debug, Clone)]
pub struct Morceau {
    pub texte: String,
    pub gras: bool,
    pub italique: bool,
    /// La sienne, ou celle du bloc (de la cellule, de la ligne) qui la porte.
    pub couleur: Rvb,
}

/// Un trait : couleur et épaisseur.
pub type Filet = (Rvb, Mesure);

#[derive(Debug, Clone)]
pub struct CelluleTableau {
    /// Couleur et graisse déjà résolues (morceau, cellule, ligne, tableau).
    pub morceaux: Vec<Morceau>,
    pub etendue: usize,
    pub alignement: Alignement,
}

#[derive(Debug, Clone)]
pub struct LigneTableau {
    pub cellules: Vec<CelluleTableau>,
    pub fond: Option<Rvb>,
    pub taille: Mesure,
    pub filet_haut: Option<Filet>,
    pub filet_bas: Option<Filet>,
}

#[derive(Debug, Clone)]
pub enum Bloc {
    Paragraphe {
        /// Chacun dans sa couleur, ou celle du paragraphe.
        morceaux: Vec<Morceau>,
        taille: Mesure,
        alignement: Alignement,
        espace_avant: Mesure,
        espace_apres: Mesure,
        /// Marge à gauche, en points : le texte se met en page dans une largeur d'autant réduite.
        retrait: Mesure,
    },
    Espace(Mesure),
    Colonnes {
        filet: Option<(Rvb, Mesure)>,
        espace_avant: Mesure,
        marge_interne_haut: Mesure,
        colonnes: Vec<(Mesure, Vec<BlocEtiquete>)>,
    },
    Image {
        chemin: String,
        largeur: Mesure,
        /// Posée à l'endroit courant sans occuper de hauteur (FORMAT.md).
        flottante: bool,
    },
    /// Position absolue sur la page, en points depuis son coin haut-gauche : ne coule pas.
    TexteFixe {
        texte: String,
        x: Mesure,
        y: Mesure,
        taille: Mesure,
        police: PoliceFixe,
        gras: bool,
        couleur: Rvb,
        /// Gauche, Droite ou Centre, par rapport à `x`.
        alignement: Alignement,
        /// Largeur à ne pas dépasser, en points : au-delà, le corps est réduit pour que le texte
        /// tienne, sans descendre sous `CORPS_PLANCHER`.
        largeur_max: Option<Mesure>,
    },
    Trait {
        x1: Mesure,
        y1: Mesure,
        x2: Mesure,
        y2: Mesure,
        epaisseur: Mesure,
        couleur: Rvb,
    },
    ImageFixe {
        chemin: String,
        x: Mesure,
        y: Mesure,
        largeur: Mesure,
        hauteur: Mesure,
        centree: bool,
    },
    Tableau {
        /// Fraction de la largeur utile et alignement de chaque colonne.
        colonnes: Vec<(Mesure, Alignement)>,
        interligne: Mesure,
        /// haut, droite, bas, gauche : l'espace entre le bord d'une cellule et son texte.
        marge_cellule: [Mesure; 4],
        filet_lignes: Option<Filet>,
        filets_cellules: Option<Filet>,
        entete: Vec<LigneTableau>,
        lignes: Vec<LigneTableau>,
        espace_avant: Mesure,
        espace_apres: Mesure,
    },
}

impl Bloc {
    /// Un bloc à position absolue sur la page (il ne coule pas, n'occupe aucune hauteur).
    pub fn est_fixe(&self) -> bool {
        matches!(self, Bloc::TexteFixe { .. } | Bloc::Trait { .. } | Bloc::ImageFixe { .. })
    }
}

/// Un bloc et son étiquette d'erreur (« bloc 3, colonne 2, bloc 1 »).
#[derive(Debug, Clone)]
pub struct BlocEtiquete {
    pub etiquette: String,
    pub bloc: Bloc,
}

/// La description validée, telle que la mise en page l'emploie.
#[derive(Debug, Clone)]
pub struct Resolue {
    pub page: Page,
    pub police: Police,
    pub logo: Option<Logo>,
    /// Sans lui, le PDF n'a pas de titre.
    pub titre: Option<String>,
    pub documents: Vec<Vec<BlocEtiquete>>,
    pub entete_page: Option<Repere>,
    pub pied_page: Option<Repere>,
}

// ---------------------------------------------------------------------------
// La validation.
// ---------------------------------------------------------------------------

/// Met une erreur de serde en français.
pub fn francais(e: &serde_json::Error) -> String {
    let brut = e.to_string();
    // serde_json ajoute « at line L column C » : on le retire du message et on le range à part.
    let (msg, lieu) = match brut.rfind(" at line ") {
        Some(i) => (brut[..i].to_string(), Some((e.line(), e.column()))),
        None => (brut.clone(), None),
    };
    let liste = |reste: &str| -> String {
        reste
            .split('`')
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, s)| s.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let nom = |reste: &str| -> String {
        match reste.split_once('`') {
            Some((n, _)) => n.to_string(),
            None => reste.to_string(),
        }
    };
    let corps = if let Some(reste) = msg.strip_prefix("unknown field `") {
        let n = nom(reste);
        match reste.find("expected ") {
            Some(i) if !reste[i..].contains("there are no") => format!(
                "champ inconnu \"{}\" (champs connus : {})",
                n,
                liste(&reste[i..])
            ),
            _ => format!("champ inconnu \"{}\"", n),
        }
    } else if let Some(reste) = msg.strip_prefix("missing field `") {
        format!("champ absent \"{}\"", nom(reste))
    } else if let Some(reste) = msg.strip_prefix("unknown variant `") {
        let n = nom(reste);
        match reste.find("expected ") {
            Some(i) => format!("valeur inconnue \"{}\" (attendu : {})", n, liste(&reste[i..])),
            None => format!("valeur inconnue \"{}\"", n),
        }
    } else if let Some(reste) = msg.strip_prefix("invalid type: ") {
        match reste.split_once(", expected ") {
            Some((recu, attendu)) => format!(
                "valeur de mauvais type : {} reçu, {} attendu",
                traduit_type(recu),
                traduit_type(attendu)
            ),
            None => format!("valeur de mauvais type : {}", reste),
        }
    } else if let Some(reste) = msg.strip_prefix("invalid value: ") {
        format!("valeur invalide : {}", reste)
    } else if msg.starts_with("EOF while parsing") {
        "le JSON s'arrête trop tôt".to_string()
    } else if e.is_syntax() {
        "JSON mal formé".to_string()
    } else {
        msg
    };
    match lieu {
        Some((l, c)) if l > 0 => format!("{} (ligne {}, colonne {})", corps, l, c),
        _ => corps,
    }
}

fn traduit_type(t: &str) -> String {
    let t = t
        .replace("floating point", "nombre décimal")
        .replace("integer", "entier")
        .replace("string", "chaîne")
        .replace("boolean", "booléen")
        .replace("sequence", "liste")
        .replace("map", "objet")
        .replace("null", "null")
        .replace("unit value", "null")
        .replace("a nombre décimal", "un nombre décimal")
        .replace("a JSON number", "un nombre")
        .replace("a chaîne", "une chaîne")
        .replace("a liste", "une liste")
        .replace("a booléen", "un booléen")
        .replace("a struct", "un objet")
        .replace("struct", "un objet");
    t
}

/// Lit « #RRGGBB » (majuscules ou minuscules).
pub fn couleur(texte: &str, ou: &str) -> Result<Rvb, Refus> {
    let chiffres: Option<Vec<u8>> = match texte.strip_prefix('#') {
        Some(h) if h.len() == 6 => h.chars().map(|c| c.to_digit(16).map(|d| d as u8)).collect(),
        Some(_) | None => None,
    };
    match chiffres.as_deref() {
        Some(&[a, b, c, d, e, f]) => Ok((a * 16 + b, c * 16 + d, e * 16 + f)),
        Some(_) | None => refus(format!("{} : couleur \"{}\" invalide (attendu : \"#RRGGBB\")", ou, texte)),
    }
}

fn positif_ou_nul(v: Mesure, nom: &str, ou: &str) -> Result<Mesure, Refus> {
    if v < Mesure::ZERO {
        refus(format!("{} : {} ne peut pas être négatif ({})", ou, nom, v))
    } else {
        Ok(v)
    }
}

fn strictement_positif(v: Mesure, nom: &str, ou: &str) -> Result<Mesure, Refus> {
    if v <= Mesure::ZERO {
        refus(format!("{} : {} doit être positif ({})", ou, nom, v))
    } else {
        Ok(v)
    }
}

/// Une couleur facultative : sans elle, `heritee`.
fn couleur_ou(texte: &Option<String>, heritee: Rvb, ou: &str) -> Result<Rvb, Refus> {
    match texte {
        Some(c) => couleur(c, ou),
        None => Ok(heritee),
    }
}

/// Une taille facultative : sans elle, `heritee`.
fn taille_ou(valeur: Option<Mesure>, heritee: Mesure, nom: &str, ou: &str) -> Result<Mesure, Refus> {
    match valeur {
        Some(t) => strictement_positif(t, nom, ou),
        None => Ok(heritee),
    }
}

/// Les styles de modèle connus, pour le message d'erreur.
const STYLES: &str = "contrat, avenant, attestation";

fn morceau(m: &f::Morceau, majuscules: bool, couleur_du_bloc: Rvb, ou: &str) -> Result<Morceau, Refus> {
    Ok(Morceau {
        texte: if majuscules { m.texte.to_uppercase() } else { m.texte.clone() },
        gras: m.gras,
        italique: m.italique,
        couleur: couleur_ou(&m.couleur, couleur_du_bloc, ou)?,
    })
}

fn lire_bloc(bloc: &f::Bloc, etiquette: &str, police: &Police) -> Result<Bloc, Refus> {
    match bloc {
        f::Bloc::Paragraphe(p) => {
            let majuscules = p.majuscules;
            let couleur_du_bloc = couleur_ou(&p.couleur, police.couleur, etiquette)?;
            let mut morceaux = Vec::new();
            for m in &p.morceaux {
                morceaux.push(morceau(m, majuscules, couleur_du_bloc, etiquette)?);
            }
            Ok(Bloc::Paragraphe {
                morceaux,
                taille: taille_ou(p.taille, police.taille, "taille", etiquette)?,
                alignement: p.alignement,
                espace_avant: positif_ou_nul(p.espace_avant, "espace_avant", etiquette)?,
                espace_apres: positif_ou_nul(p.espace_apres, "espace_apres", etiquette)?,
                retrait: positif_ou_nul(p.retrait, "retrait", etiquette)?,
            })
        }
        f::Bloc::Espace(e) => Ok(Bloc::Espace(positif_ou_nul(e.hauteur, "hauteur", etiquette)?)),
        f::Bloc::Image(i) => Ok(Bloc::Image {
            largeur: strictement_positif(i.largeur, "largeur", etiquette)?,
            chemin: i.chemin.clone(),
            flottante: i.flottante,
        }),
        f::Bloc::Colonnes(c) => {
            if c.colonnes.is_empty() {
                return refus(format!("{} : une liste de colonnes vide", etiquette));
            }
            let filet = lire_filet(&c.filet_haut, etiquette)?;
            let mut somme = Mesure::ZERO;
            let mut colonnes = Vec::new();
            for (k, col) in c.colonnes.iter().enumerate() {
                let ou_col = format!("{}, colonne {}", etiquette, k + 1);
                let l = strictement_positif(col.largeur, "largeur", &ou_col)?;
                somme += l;
                let mut blocs = Vec::new();
                for (j, b) in col.blocs.iter().flatten().enumerate() {
                    let et = format!("{}, bloc {}", ou_col, j + 1);
                    let bloc = lire_bloc(b, &et, police)?;
                    if matches!(bloc, Bloc::Tableau { .. }) {
                        return refus(format!(
                            "{} : un tableau ne se place pas dans des colonnes (il se coupe entre deux pages, elles non)",
                            et
                        ));
                    }
                    if bloc.est_fixe() {
                        return refus(format!(
                            "{} : un bloc à position fixe (texte_fixe, trait, image_fixe) ne se place pas dans des colonnes",
                            et
                        ));
                    }
                    blocs.push(BlocEtiquete { bloc, etiquette: et });
                }
                colonnes.push((l, blocs));
            }
            if (somme - Mesure::UN).abs() > Mesure::milliemes(1) {
                return refus(format!(
                    "{} : la somme des largeurs de colonnes vaut {}, elle doit valoir 1",
                    etiquette, somme
                ));
            }
            Ok(Bloc::Colonnes {
                filet,
                espace_avant: positif_ou_nul(c.espace_avant, "espace_avant", etiquette)?,
                marge_interne_haut: positif_ou_nul(c.marge_interne_haut, "marge_interne_haut", etiquette)?,
                colonnes,
            })
        }
        f::Bloc::Tableau(t) => lire_tableau(t, etiquette, police),
        f::Bloc::TexteFixe(t) => {
            let alignement = t.alignement;
            if alignement == Alignement::Justifie {
                return refus(format!(
                    "{} : un texte à position fixe ne se justifie pas (gauche, droite ou centre)",
                    etiquette
                ));
            }
            Ok(Bloc::TexteFixe {
                x: t.x,
                y: t.y,
                taille: taille_ou(t.taille, police.taille, "taille", etiquette)?,
                couleur: couleur_ou(&t.couleur, police.couleur, etiquette)?,
                texte: t.texte.clone(),
                police: t.police,
                gras: t.gras,
                alignement,
                largeur_max: match t.largeur_max {
                    Some(v) => Some(strictement_positif(v, "largeur_max", etiquette)?),
                    None => None,
                },
            })
        }
        f::Bloc::Trait(t) => Ok(Bloc::Trait {
            x1: t.x1,
            y1: t.y1,
            x2: t.x2,
            y2: t.y2,
            epaisseur: strictement_positif(t.epaisseur, "epaisseur", etiquette)?,
            couleur: couleur(&t.couleur, etiquette)?,
        }),
        f::Bloc::ImageFixe(i) => Ok(Bloc::ImageFixe {
            x: i.x,
            y: i.y,
            largeur: strictement_positif(i.largeur, "largeur", etiquette)?,
            hauteur: strictement_positif(i.hauteur, "hauteur", etiquette)?,
            centree: i.centree,
            chemin: i.chemin.clone(),
        }),
    }
}

fn lire_filet(f: &Option<f::Filet>, ou: &str) -> Result<Option<Filet>, Refus> {
    match f {
        Some(x) => Ok(Some((
            couleur(&x.couleur, ou)?,
            strictement_positif(x.epaisseur, "epaisseur", ou)?,
        ))),
        None => Ok(None),
    }
}

/// Une ligne de tableau : les couleurs, la graisse et la taille de la ligne descendent dans ses
/// cellules (celle du morceau, puis celle de la cellule, l'emportent).
fn lire_ligne(
    l: &f::Ligne,
    ou: &str,
    colonnes: &[(Mesure, Alignement)],
    taille: Mesure,
    couleur_table: Rvb,
) -> Result<LigneTableau, Refus> {
    let taille = taille_ou(l.taille, taille, "taille", ou)?;
    let couleur_ligne = couleur_ou(&l.couleur, couleur_table, ou)?;
    let fond = match &l.fond {
        Some(c) => Some(couleur(c, ou)?),
        None => None,
    };
    let gras_ligne = l.gras;
    let mut cellules = Vec::new();
    let mut occupe = 0usize;
    for (k, v) in l.cellules.iter().enumerate() {
        let ou_c = format!("{}, cellule {}", ou, k + 1);
        let texte_seul;
        let decrite = match v {
            // Une cellule dite par son seul texte : une colonne, sa graisse et sa couleur celles de la ligne.
            f::Cellule::Texte(s) => {
                texte_seul = f::CelluleDecrite {
                    texte: Some(s.clone()),
                    morceaux: None,
                    gras: false,
                    couleur: None,
                    alignement: None,
                    etendue: 1,
                };
                &texte_seul
            }
            f::Cellule::Decrite(d) => d,
        };
        let etendue = decrite.etendue;
        if etendue == 0 {
            return refus(format!("{} : etendue doit valoir au moins 1", ou_c));
        }
        if occupe + etendue > colonnes.len() {
            return refus(format!(
                "{} : la ligne occupe plus que les {} colonne(s) du tableau",
                ou_c,
                colonnes.len()
            ));
        }
        let couleur_cellule = couleur_ou(&decrite.couleur, couleur_ligne, &ou_c)?;
        let gras_cellule = decrite.gras;
        let bruts: Vec<f::Morceau> = match (&decrite.texte, &decrite.morceaux) {
            (Some(_), Some(_)) => {
                return refus(format!("{} : \"texte\" et \"morceaux\" s'excluent", ou_c))
            }
            // Le texte d'une cellule : un morceau droit, que la cellule ou la ligne met en gras.
            (Some(t), None) => vec![f::Morceau { texte: t.clone(), gras: false, italique: false, couleur: None }],
            (None, Some(m)) => m.clone(),
            (None, None) => Vec::new(),
        };
        let mut morceaux = Vec::new();
        for m in &bruts {
            morceaux.push(Morceau {
                texte: m.texte.clone(),
                gras: m.gras || gras_cellule || gras_ligne,
                italique: m.italique,
                couleur: couleur_ou(&m.couleur, couleur_cellule, &ou_c)?,
            });
        }
        cellules.push(CelluleTableau {
            morceaux,
            etendue,
            // Sans mot de la cellule, celui de la colonne où elle commence.
            alignement: match decrite.alignement {
                Some(a) => a,
                None => colonnes[occupe].1,
            },
        });
        occupe += etendue;
    }
    Ok(LigneTableau {
        cellules,
        fond,
        taille,
        filet_haut: lire_filet(&l.filet_haut, ou)?,
        filet_bas: lire_filet(&l.filet_bas, ou)?,
    })
}

fn lire_tableau(t: &f::Tableau, etiquette: &str, police: &Police) -> Result<Bloc, Refus> {
    if t.colonnes.is_empty() {
        return refus(format!("{} : un tableau sans colonne", etiquette));
    }
    let mut colonnes = Vec::new();
    let mut somme = Mesure::ZERO;
    for (k, c) in t.colonnes.iter().enumerate() {
        let ou_col = format!("{}, colonne {}", etiquette, k + 1);
        let l = strictement_positif(c.largeur, "largeur", &ou_col)?;
        somme += l;
        colonnes.push((l, c.alignement));
    }
    if (somme - Mesure::UN).abs() > Mesure::milliemes(1) {
        return refus(format!(
            "{} : la somme des largeurs de colonnes vaut {}, elle doit valoir 1",
            etiquette, somme
        ));
    }
    let taille = taille_ou(t.taille, police.taille, "taille", etiquette)?;
    let couleur_table = couleur_ou(&t.couleur, police.couleur, etiquette)?;
    let interligne = taille_ou(t.interligne, police.interligne, "interligne", etiquette)?;
    let ou_m = format!("{}, marge_cellule", etiquette);
    let m = &t.marge_cellule;
    let marge_cellule = [
        positif_ou_nul(m.haut, "haut", &ou_m)?,
        positif_ou_nul(m.droite, "droite", &ou_m)?,
        positif_ou_nul(m.bas, "bas", &ou_m)?,
        positif_ou_nul(m.gauche, "gauche", &ou_m)?,
    ];
    let mut entete = Vec::new();
    for (i, l) in t.entete.iter().flatten().enumerate() {
        let ou = format!("{}, en-tête, ligne {}", etiquette, i + 1);
        entete.push(lire_ligne(l, &ou, &colonnes, taille, couleur_table)?);
    }
    let mut lignes = Vec::new();
    for (i, l) in t.lignes.iter().flatten().enumerate() {
        let ou = format!("{}, ligne {}", etiquette, i + 1);
        lignes.push(lire_ligne(l, &ou, &colonnes, taille, couleur_table)?);
    }
    Ok(Bloc::Tableau {
        colonnes,
        interligne,
        marge_cellule,
        filet_lignes: lire_filet(&t.filet_lignes, etiquette)?,
        filets_cellules: lire_filet(&t.filets_cellules, etiquette)?,
        entete,
        lignes,
        espace_avant: positif_ou_nul(t.espace_avant, "espace_avant", etiquette)?,
        espace_apres: positif_ou_nul(t.espace_apres, "espace_apres", etiquette)?,
    })
}

fn lire_repere(r: &Option<f::Repere>, nom: &str, police: &Police) -> Result<Option<Repere>, Refus> {
    let r = match r {
        Some(r) => r,
        None => return Ok(None),
    };
    for t in [&r.gauche, &r.centre, &r.droite].into_iter().flatten() {
        if t.contains('\n') || t.contains('\r') {
            return refus(format!("{} : chaque texte tient sur une ligne (pas de \\n)", nom));
        }
    }
    Ok(Some(Repere {
        taille: taille_ou(r.taille, police.taille, "taille", nom)?,
        couleur: couleur_ou(&r.couleur, police.couleur, nom)?,
        distance: positif_ou_nul(r.distance, "distance", nom)?,
        gauche: r.gauche.clone(),
        centre: r.centre.clone(),
        droite: r.droite.clone(),
        gras: r.gras,
        premiere_page: r.premiere_page,
        image: match &r.image {
            Some(i) => Some(ImageRepere {
                largeur: strictement_positif(i.largeur, "largeur", nom)?,
                descente: positif_ou_nul(i.descente, "descente", nom)?,
                chemin: i.chemin.clone(),
            }),
            None => None,
        },
    }))
}

/// La page : celle que la description dit, sinon celle du style ; une description sans l'une ni l'autre se refuse.
fn lire_page(p: &Option<f::Page>, style: Option<style_contrat::Variante>) -> Result<Page, Refus> {
    let brute = match (p, style) {
        (Some(p), _) => Page {
            largeur: p.largeur,
            hauteur: p.hauteur,
            haut: p.marges.haut,
            droite: p.marges.droite,
            bas: p.marges.bas,
            gauche: p.marges.gauche,
        },
        (None, Some(_)) => style_contrat::page(),
        (None, None) => return refus("description : sans \"style\", elle dit sa \"page\""),
    };
    let page = Page {
        largeur: strictement_positif(brute.largeur, "largeur", "page")?,
        hauteur: strictement_positif(brute.hauteur, "hauteur", "page")?,
        haut: positif_ou_nul(brute.haut, "haut", "page, marges")?,
        droite: positif_ou_nul(brute.droite, "droite", "page, marges")?,
        bas: positif_ou_nul(brute.bas, "bas", "page, marges")?,
        gauche: positif_ou_nul(brute.gauche, "gauche", "page, marges")?,
    };
    if page.largeur_utile() <= Mesure::ZERO || page.hauteur_utile() <= Mesure::ZERO {
        return refus("page : les marges ne laissent aucune place au texte");
    }
    Ok(page)
}

/// La police : celle que la description dit, sinon celle du style ; une description sans l'une ni l'autre se refuse.
fn lire_police(p: &Option<f::Police>, style: Option<style_contrat::Variante>) -> Result<Police, Refus> {
    let (taille, interligne, teinte) = match (p, style) {
        (Some(p), _) => (p.taille, p.interligne, couleur(&p.couleur, "police")?),
        (None, Some(_)) => {
            let q = style_contrat::police();
            (q.taille, q.interligne, q.couleur)
        }
        (None, None) => return refus("description : sans \"style\", elle dit sa \"police\""),
    };
    Ok(Police {
        taille: strictement_positif(taille, "taille", "police")?,
        interligne: strictement_positif(interligne, "interligne", "police")?,
        couleur: teinte,
    })
}

/// Valide une description : ce que la mise en page emploie.
pub fn resoudre(d: &f::Description) -> Result<Resolue, Refus> {
    // Le 3 est le numéro de la fabrique (`--version`) : une description qui le porte est celle
    // du format 2, avec `largeur_max` en plus. Les trois se lisent de la même façon.
    if !matches!(d.format, 1..=3) {
        return refus(format!(
            "format {} non pris en charge : cette fabrique lit les formats 1, 2 et 3",
            d.format
        ));
    }
    // Le style (« contrat ») donne la page et la police quand la description ne les dit pas.
    let style = d.style.map(|s| match s {
        f::Style::Contrat => style_contrat::Variante::Contrat,
        f::Style::Avenant => style_contrat::Variante::Avenant,
        f::Style::Attestation => style_contrat::Variante::Attestation,
    });
    let page = lire_page(&d.page, style)?;
    let police = lire_police(&d.police, style)?;
    let logo = match &d.logo {
        Some(l) => Some(Logo {
            largeur: strictement_positif(l.largeur, "largeur", "logo")?,
            marge_haut: positif_ou_nul(l.marge_haut, "marge_haut", "logo")?,
            marge_droite: positif_ou_nul(l.marge_droite, "marge_droite", "logo")?,
            chemin: l.chemin.clone(),
        }),
        None => None,
    };
    if d.documents.is_empty() {
        return refus("description : la liste \"documents\" est vide");
    }
    let plusieurs = d.documents.len() > 1;
    let mut documents = Vec::new();
    let mut titre_modele: Option<String> = None;
    for (n, doc) in d.documents.iter().enumerate() {
        let blocs_dits = match doc {
            f::Document::Modele(mb) => {
                let ou = if plusieurs { format!("document {}", n + 1) } else { "document".to_string() };
                let prefixe = if plusieurs { format!("document {}, ", n + 1) } else { String::new() };
                let (blocs, titre) = lire_modele(mb, d.format, style, &ou, prefixe, titre_modele.is_none())?;
                if titre_modele.is_none() {
                    titre_modele = titre;
                }
                documents.push(blocs);
                continue;
            }
            f::Document::Blocs(b) => b,
        };
        let mut blocs = Vec::new();
        for (i, b) in blocs_dits.iter().enumerate() {
            let etiquette = if plusieurs {
                format!("document {}, bloc {}", n + 1, i + 1)
            } else {
                format!("bloc {}", i + 1)
            };
            blocs.push(BlocEtiquete {
                bloc: lire_bloc(b, &etiquette, &police)?,
                etiquette,
            });
        }
        // La page d'un document à position fixe est fixe : ses blocs ne se mêlent pas à ceux qui
        // coulent (leur origine serait celle de l'endroit où l'on en est, pas celle de la page).
        if let Some(premier_fixe) = blocs.iter().position(|b| b.bloc.est_fixe()) {
            if let Some(autre) = blocs.iter().find(|b| !b.bloc.est_fixe()) {
                return refus(format!(
                    "{} : un document à position fixe ne mêle pas les blocs fixes (par exemple {}) et ceux qui coulent (par exemple {}) ; un document par page fixe",
                    autre.etiquette, blocs[premier_fixe].etiquette, autre.etiquette
                ));
            }
        }
        documents.push(blocs);
    }
    let entete_page = lire_repere(&d.entete_page, "entete_page", &police)?;
    let pied_page = lire_repere(&d.pied_page, "pied_page", &police)?;
    Ok(Resolue {
        page,
        police,
        logo,
        // Sans titre dit, celui du premier modèle (`texte_brut`, sans gras).
        titre: d.titre.clone().or(titre_modele),
        documents,
        entete_page,
        pied_page,
    })
}

/// Un document modèle mis en page selon le style : ses blocs, et son titre (`texte_brut`, sans gras) si `veut_titre`.
fn lire_modele(
    mb: &f::Modele,
    format: i64,
    style: Option<style_contrat::Variante>,
    ou: &str,
    prefixe: String,
    veut_titre: bool,
) -> Result<(Vec<BlocEtiquete>, Option<String>), Refus> {
    use style_contrat::Variante;
    if format < 2 {
        return refus(format!("{} : \"modele\" demande \"format\": 2", ou));
    }
    let variante = match style {
        Some(v) => v,
        None => {
            return refus(format!(
                "{} : \"modele\" demande un \"style\" au niveau de la description (styles connus : {})",
                ou, STYLES
            ))
        }
    };
    // Le modèle vient d'un fichier (`chemin`) ou de son texte (`texte`) : l'un des deux.
    let (m, nom_modele) = match (&mb.chemin, &mb.texte) {
        (Some(_), Some(_)) => {
            return refus(format!("{} : \"chemin\" et \"texte\" s'excluent (le modèle vient de l'un ou de l'autre)", ou))
        }
        (None, None) => {
            return refus(format!("{} : \"modele\" demande \"chemin\" (le fichier) ou \"texte\" (le modèle même)", ou))
        }
        (Some(c), None) => (modele::lire_fichier(c)?, c.clone()),
        (None, Some(t)) => (modele::lire_modele(t, "le modèle donné en texte")?, "le modèle donné en texte".to_string()),
    };
    // Un avenant dit ce qu'il modifie ; un contrat et une attestation ne modifient rien.
    let dites: &[f::Modification] = match (&mb.modifications, variante) {
        (Some(m), Variante::Avenant) if !m.is_empty() => m,
        (Some(_) | None, Variante::Avenant) => {
            return refus(format!("{} : rien n'a changé, pas d'avenant à écrire (\"modifications\" est vide)", ou))
        }
        (None, Variante::Contrat | Variante::Attestation) => &[],
        (Some(_), Variante::Contrat | Variante::Attestation) => {
            return refus(format!("{} : \"modifications\" ne va qu'avec le style \"avenant\"", ou))
        }
    };
    if variante != Variante::Contrat && mb.remplace.is_some() {
        return refus(format!("{} : \"remplace\" ne va qu'avec le style \"contrat\"", ou));
    }
    if variante == Variante::Attestation && mb.signature.is_some() {
        return refus(format!(
            "{} : le style \"attestation\" n'a pas de case de signature (\"signature\")",
            ou
        ));
    }
    if variante != Variante::Attestation && mb.signature_employeur.is_some() {
        return refus(format!(
            "{} : \"signature_employeur\" ne va qu'avec le style \"attestation\"",
            ou
        ));
    }
    if variante == Variante::Avenant {
        if m.modification.is_empty() {
            return refus(format!("{} n'a pas de bloc MODIFICATION:.", nom_modele));
        }
        for (i, x) in dites.iter().enumerate() {
            if x.phrase.is_some() && !m.modification.iter().any(|b| b.contains("modification.phrase")) {
                return refus(format!("{} : modification {} : le bloc MODIFICATION: n'a pas de paragraphe {{{{modification.phrase}}}}", ou, i + 1));
            }
            if x.phrase.is_none() && x.numero.is_none() {
                return refus(format!("{} : modification {} : ni \"numero\" ni \"phrase\"", ou, i + 1));
            }
        }
    }
    let modifications: Vec<style_contrat::Modification> = dites
        .iter()
        .map(|x| style_contrat::Modification {
            numero: x.numero,
            article: x.article.clone(),
            avant: x.avant.clone(),
            apres: x.apres.clone(),
            phrase: x.phrase.clone(),
        })
        .collect();
    let valeurs: &BTreeMap<String, String> = &mb.valeurs;
    let m = m.retenir(valeurs);
    let c = style_contrat::Contrat {
        variante,
        valeurs,
        remplace: mb.remplace.as_ref().map(|r| (r.numero.as_str(), r.date.as_str())),
        signature: mb.signature.as_deref(),
        modifications: &modifications,
        signature_employeur: mb.signature_employeur.as_deref(),
    };
    let titre = match (veut_titre, &m.titre) {
        (true, Some(t)) => Some(modele::texte_brut(t, valeurs)?),
        (true, None) | (false, Some(_)) | (false, None) => None,
    };
    Ok((style_contrat::blocs(&m, &c, &prefixe)?, titre))
}
