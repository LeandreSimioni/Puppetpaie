//! La forme de ce qu'on donne à la fabrique (FORMAT.md), déclarée une fois : la description d'un
//! document (`Description`), le remplissage d'un formulaire (`Remplissage`), les champs qu'on lui
//! ajoute (`AjoutDeChamps`). Le moteur les construit en Rust ; la ligne de commande les lit d'un texte
//! JSON, une fois, strictement (`lire`) : aucune clé inconnue.
//!
//! L'appelant dit chaque valeur : aucune n'a de défaut. Un champ `Option` ne dit qu'une préséance — sans lui,
//! la valeur de ce qui le contient (la police, la ligne, la colonne, le style, le modèle), nommée sur le champ —
//! ou une absence (rien à cette place) ; elle ne se résout qu'à un seul endroit, la validation (`description.rs`).

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::description::{francais, Refus};
pub use crate::mesure::Mesure;

/// La largeur d'une page A4, en points (210 mm).
pub const A4_LARGEUR: Mesure = Mesure::milliemes(595_280);
/// La hauteur d'une page A4, en points (297 mm).
pub const A4_HAUTEUR: Mesure = Mesure::milliemes(841_890);

/// Lit `texte` en `T`, strictement ; un refus est une ligne de français.
pub fn lire<T: serde::de::DeserializeOwned>(texte: &str, quoi: &str) -> Result<T, Refus> {
    serde_json::from_str(texte).map_err(|e| Refus::a_corriger(format!("{} illisible : {}", quoi, francais(&e))))
}

// ---------------------------------------------------------------------------
// La description d'un document.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Description {
    /// 1, 2 ou 3 (FORMAT.md) ; un `modele` demande 2 au moins.
    pub format: i64,
    /// Sans elle : celle du style ; une description sans style la dit.
    pub page: Option<Page>,
    /// Sans elle : celle du style ; une description sans style la dit.
    pub police: Option<Police>,
    /// La mise en page des documents « modèle ».
    pub style: Option<Style>,
    pub logo: Option<Logo>,
    /// Sans lui : celui du premier modèle, sinon le PDF n'a pas de titre.
    pub titre: Option<String>,
    /// Chacun commence sur une nouvelle page.
    pub documents: Vec<Document>,
    pub entete_page: Option<Repere>,
    pub pied_page: Option<Repere>,
}

impl Description {
    /// La porte du texte : une description JSON (FORMAT.md) lue une fois, strictement.
    pub fn lire(texte: &str) -> Result<Description, Refus> {
        lire(texte, "description")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    Contrat,
    Avenant,
    Attestation,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub largeur: Mesure,
    pub hauteur: Mesure,
    pub marges: Marges,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marges {
    pub haut: Mesure,
    pub droite: Mesure,
    pub bas: Mesure,
    pub gauche: Mesure,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Police {
    pub taille: Mesure,
    /// Le multiple du corps d'une ligne à la suivante.
    pub interligne: Mesure,
    pub couleur: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Logo {
    pub chemin: PathBuf,
    pub largeur: Mesure,
    pub marge_haut: Mesure,
    pub marge_droite: Mesure,
}

/// Un en-tête ou un pied de page répété sur chaque page (FORMAT.md, « En-tête et pied de page »).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repere {
    /// Chaque texte non dit : rien à cette place.
    pub gauche: Option<String>,
    pub centre: Option<String>,
    pub droite: Option<String>,
    /// Sans elle : celle de la police.
    pub taille: Option<Mesure>,
    /// Sans elle : celle de la police.
    pub couleur: Option<String>,
    pub gras: bool,
    /// Du bord de la page à la ligne de base, en points.
    pub distance: Mesure,
    /// Le repère est aussi sur la première page.
    pub premiere_page: bool,
    pub image: Option<ImageRepere>,
}

/// L'image qui suit le texte de droite d'un repère.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRepere {
    pub chemin: PathBuf,
    pub largeur: Mesure,
    /// Ce que le bas de l'image descend sous la ligne de base (0 : posé dessus).
    pub descente: Mesure,
}

/// Un document : ses blocs, ou un modèle que la fabrique met en page selon le style. En JSON,
/// `{"blocs": [...]}` ou `{"modele": {...}}`.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "DocumentDit")]
pub enum Document {
    Blocs(Vec<Bloc>),
    Modele(Modele),
}

/// Un document tel que le texte le dit : l'un des deux, que `Document` départage.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentDit {
    blocs: Option<Vec<Bloc>>,
    modele: Option<Modele>,
}

/// Ce qu'un document ecrit a de faux : il dit ses blocs ou son modele, l'un des deux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FauteDeDocument {
    BlocsEtModele,
    NiBlocsNiModele,
}

impl std::fmt::Display for FauteDeDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FauteDeDocument::BlocsEtModele => "un document : \"blocs\" et \"modele\" s'excluent",
            FauteDeDocument::NiBlocsNiModele => "un document dit ses \"blocs\" ou son \"modele\"",
        })
    }
}

impl TryFrom<DocumentDit> for Document {
    type Error = FauteDeDocument;
    fn try_from(d: DocumentDit) -> Result<Document, FauteDeDocument> {
        match (d.blocs, d.modele) {
            (Some(b), None) => Ok(Document::Blocs(b)),
            (None, Some(m)) => Ok(Document::Modele(m)),
            (Some(_), Some(_)) => Err(FauteDeDocument::BlocsEtModele),
            (None, None) => Err(FauteDeDocument::NiBlocsNiModele),
        }
    }
}

/// Un document dont le texte est dans un fichier `.modele` (`chemin`) ou donné tel quel (`texte`) :
/// l'un des deux.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modele {
    pub chemin: Option<PathBuf>,
    pub texte: Option<String>,
    pub valeurs: BTreeMap<String, String>,
    /// Le contrat que celui-ci annule et remplace (style « contrat »).
    pub remplace: Option<Remplace>,
    /// L'image de la signature de l'employeur dans sa case (styles « contrat » et « avenant »).
    pub signature: Option<PathBuf>,
    /// L'image de la signature de l'employeur en fin de document (style « attestation »).
    pub signature_employeur: Option<PathBuf>,
    /// Les articles que l'avenant modifie (style « avenant »).
    pub modifications: Option<Vec<Modification>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remplace {
    pub numero: String,
    pub date: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modification {
    /// Le numéro de l'article cité ; aucun pour une modification dite en clair (`phrase`).
    pub numero: Option<i64>,
    pub article: String,
    pub avant: Vec<String>,
    pub apres: Vec<String>,
    /// La modification dite en clair, sans article cité : le paragraphe `{{modification.phrase}}` du modèle la porte.
    pub phrase: Option<String>,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Alignement {
    Gauche,
    Justifie,
    Centre,
    Droite,
}

/// Les blocs d'un document, dans l'ordre (FORMAT.md, « Les blocs »).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Bloc {
    Paragraphe(Paragraphe),
    Espace(Espace),
    Colonnes(Colonnes),
    Image(Image),
    Tableau(Tableau),
    TexteFixe(TexteFixe),
    Trait(Trait),
    ImageFixe(ImageFixe),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Morceau {
    pub texte: String,
    /// Dans un tableau, gras aussi quand sa cellule ou sa ligne l'est.
    pub gras: bool,
    pub italique: bool,
    /// Sans elle : celle du bloc (ou de la cellule, de la ligne).
    pub couleur: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paragraphe {
    pub morceaux: Vec<Morceau>,
    /// Sans elle : celle de la police.
    pub taille: Option<Mesure>,
    /// Sans elle : celle de la police.
    pub couleur: Option<String>,
    pub alignement: Alignement,
    /// Le texte mis en capitales.
    pub majuscules: bool,
    pub espace_avant: Mesure,
    pub espace_apres: Mesure,
    /// Marge à gauche du paragraphe, en points (une citation, un alinéa).
    pub retrait: Mesure,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Espace {
    pub hauteur: Mesure,
}

/// Un trait le long d'un bloc ou d'une ligne.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filet {
    pub couleur: String,
    pub epaisseur: Mesure,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colonnes {
    pub filet_haut: Option<Filet>,
    pub espace_avant: Mesure,
    pub marge_interne_haut: Mesure,
    pub colonnes: Vec<Colonne>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colonne {
    /// La fraction de la largeur utile ; les colonnes font 1 à elles toutes.
    pub largeur: Mesure,
    /// Sans eux : une colonne vide.
    pub blocs: Option<Vec<Bloc>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub chemin: PathBuf,
    pub largeur: Mesure,
    /// Posée à l'endroit courant sans occuper de hauteur ; sinon elle occupe sa hauteur.
    pub flottante: bool,
}

/// La police d'un `texte_fixe` : Liberation Sans (Helvetica), Serif (Times) ou Mono (Courier).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PoliceFixe {
    Helvetica,
    Times,
    Courier,
}

/// Un texte posé à une position absolue de la page.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TexteFixe {
    pub texte: String,
    pub x: Mesure,
    /// L'ordonnée de la ligne de base, depuis le haut de la page.
    pub y: Mesure,
    /// Sans elle : celle de la police.
    pub taille: Option<Mesure>,
    pub police: PoliceFixe,
    pub gras: bool,
    /// Sans elle : celle de la police.
    pub couleur: Option<String>,
    /// Par rapport à `x`.
    pub alignement: Alignement,
    /// La largeur que le texte ne doit pas dépasser : sinon son corps est réduit (FORMAT.md).
    pub largeur_max: Option<Mesure>,
}

/// Un trait d'un point à un autre de la page.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trait {
    pub x1: Mesure,
    pub y1: Mesure,
    pub x2: Mesure,
    pub y2: Mesure,
    pub epaisseur: Mesure,
    pub couleur: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageFixe {
    pub chemin: PathBuf,
    pub x: Mesure,
    pub y: Mesure,
    pub largeur: Mesure,
    pub hauteur: Mesure,
    /// Centrée dans le rectangle à ses proportions ; sinon posée au coin (x, y).
    pub centree: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColonneTableau {
    /// La fraction de la largeur utile ; les colonnes font 1 à elles toutes.
    pub largeur: Mesure,
    pub alignement: Alignement,
}

/// L'espace entre le bord d'une cellule et son texte.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MargeCellule {
    pub haut: Mesure,
    pub droite: Mesure,
    pub bas: Mesure,
    pub gauche: Mesure,
}

/// Une cellule : son texte seul (en JSON, une chaîne), ou une cellule décrite (un objet).
#[derive(Debug, Clone)]
pub enum Cellule {
    Texte(String),
    Decrite(CelluleDecrite),
}

/// Lue à la main plutôt que sans étiquette : l'objet garde le refus de sa clé inconnue, et le reste dit
/// « une chaîne ou un objet ».
impl<'de> Deserialize<'de> for Cellule {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Cellule, D::Error> {
        struct Lecteur;
        impl<'de> serde::de::Visitor<'de> for Lecteur {
            type Value = Cellule;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("une chaîne ou un objet (une cellule)")
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Cellule, E> {
                Ok(Cellule::Texte(s.to_string()))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, m: A) -> Result<Cellule, A::Error> {
                CelluleDecrite::deserialize(serde::de::value::MapAccessDeserializer::new(m)).map(Cellule::Decrite)
            }
        }
        d.deserialize_any(Lecteur)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CelluleDecrite {
    /// `texte` et `morceaux` s'excluent ; sans l'un ni l'autre, la cellule est vide.
    pub texte: Option<String>,
    pub morceaux: Option<Vec<Morceau>>,
    /// Gras aussi quand sa ligne l'est.
    pub gras: bool,
    /// Sans elle : celle de la ligne.
    pub couleur: Option<String>,
    /// Sans lui : celui de la colonne où elle commence.
    pub alignement: Option<Alignement>,
    /// Le nombre de colonnes qu'elle couvre (une cellule donnée par son seul texte en couvre une).
    pub etendue: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ligne {
    pub cellules: Vec<Cellule>,
    pub gras: bool,
    pub fond: Option<String>,
    /// Sans elle : celle du tableau.
    pub couleur: Option<String>,
    /// Sans elle : celle du tableau.
    pub taille: Option<Mesure>,
    pub filet_haut: Option<Filet>,
    pub filet_bas: Option<Filet>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tableau {
    pub colonnes: Vec<ColonneTableau>,
    /// Répétées en haut de chaque page ; sans elles : aucune.
    pub entete: Option<Vec<Ligne>>,
    /// Sans elles : aucune.
    pub lignes: Option<Vec<Ligne>>,
    /// Sans elle : celle de la police.
    pub taille: Option<Mesure>,
    /// Sans elle : celle de la police.
    pub couleur: Option<String>,
    /// Sans lui : celui de la police.
    pub interligne: Option<Mesure>,
    pub marge_cellule: MargeCellule,
    pub filet_lignes: Option<Filet>,
    pub filets_cellules: Option<Filet>,
    pub espace_avant: Mesure,
    pub espace_apres: Mesure,
}

// ---------------------------------------------------------------------------
// Le remplissage d'un formulaire (FORMAT.md, « Remplir un formulaire »).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remplissage {
    /// `FORMAT`.
    pub format: i64,
    /// Nom du champ → texte à y écrire ; sans eux : aucun.
    pub champs: Option<BTreeMap<String, String>>,
    /// Sans elles : aucune.
    pub images: Option<Vec<ImagePosee>>,
}

impl Remplissage {
    /// La porte du texte (`--remplir`).
    pub fn lire(texte: &str) -> Result<Remplissage, Refus> {
        lire(texte, "description")
    }
}

/// Une image posée dans un rectangle d'une page : le plus grand rectangle à ses proportions qui tient, centré.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImagePosee {
    /// À partir de 1.
    pub page: usize,
    /// [x0, y0, x1, y1], en points, origine en haut à gauche (comme `--champs`).
    pub rect: [Mesure; 4],
    /// Le retrait de l'image sur chaque bord du rectangle.
    pub marge: Mesure,
    /// `fichier` (PNG, JPEG ou SVG) et `svg` (le texte du SVG) s'excluent.
    pub fichier: Option<PathBuf>,
    pub svg: Option<String>,
}

// ---------------------------------------------------------------------------
// Les champs ajoutés à un PDF (`formulaire::ajouter_champs`).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AjoutDeChamps {
    /// `FORMAT`.
    pub format: i64,
    pub champs: Vec<ChampAjoute>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChampAjoute {
    /// Non vide, sans point.
    pub nom: String,
    /// À partir de 1.
    pub page: usize,
    /// [x0, y0, x1, y1], origine en haut à gauche.
    pub rect: [Mesure; 4],
    pub valeur: Option<String>,
    pub lecture_seule: bool,
    /// Le corps ; sans lui : le corps automatique (celui qui tient, ISO 32000-1, 12.7.3.3).
    pub taille: Option<Mesure>,
    /// Gauche, centre ou droite.
    pub alignement: Alignement,
}
