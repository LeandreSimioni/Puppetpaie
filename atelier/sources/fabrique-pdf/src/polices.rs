//! Les polices : Liberation Sans, et pour les blocs à position fixe Liberation Serif et Mono,
//! embarquées dans le PDF **en sous-ensemble** : seuls les glyphes des caractères imprimés
//! (`sous_ensemble.rs`), avec la table `ToUnicode` qui fait que le texte extrait est
//! exactement le texte imprimé.
//!
//! Liberation (licence SIL OFL 1.1, voir `polices/LICENCE-Liberation-OFL.txt`) a les métriques
//! d'Helvetica/Arial (Sans), de Times New Roman (Serif) et de Courier New (Mono) : la mise en page
//! est celle qu'avaient les polices standard, que le PDF nommait sans les embarquer. Tout
//! caractère que la police possède s'imprime (« Ł », « ő », « č », cyrillique, grec…) ; un
//! caractère qu'elle n'a pas est un refus (`mise_en_page::verifier_texte`).
//!
//! Le romain, le gras, l'italique et le gras italique de Sans sont quatre fichiers. Le Serif et
//! le Mono n'ont ni italique ni gras italique dans les blocs fixes (le format ne les offre pas) :
//! la case reprend le romain, ou le gras, du même fichier.

use std::sync::Arc;

use crate::description::{defaut, Refus};

// Comprimées par build.rs (zlib) : 0,4 Mo de moins dans le programme.
static REGULIER_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSans-Regular.ttf.z"));
static GRAS_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSans-Bold.ttf.z"));
static ITALIQUE_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSans-Italic.ttf.z"));
static GRAS_ITALIQUE_Z: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSans-BoldItalic.ttf.z"));
static SERIF_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSerif-Regular.ttf.z"));
static SERIF_GRAS_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSerif-Bold.ttf.z"));
static MONO_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationMono-Regular.ttf.z"));
static MONO_GRAS_Z: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LiberationMono-Bold.ttf.z"));

/// Le fichier de la police comprimee `z`, decomprime une fois par processus : chaque document le reprend.
fn decomprimer(z: &'static [u8]) -> Result<Arc<Vec<u8>>, Refus> {
    use std::io::Read;
    static DECOMPRIMEES: std::sync::Mutex<Vec<(usize, Arc<Vec<u8>>)>> = std::sync::Mutex::new(Vec::new());
    let cle = z.as_ptr() as usize;
    if let Some((_, f)) = DECOMPRIMEES.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(c, _)| *c == cle) {
        return Ok(f.clone());
    }
    let mut ttf = Vec::new();
    flate2::read::ZlibDecoder::new(z)
        .read_to_end(&mut ttf)
        .or_else(|e| defaut(format!("police interne illisible : {}", e)))?;
    let f = Arc::new(ttf);
    DECOMPRIMEES.lock().unwrap_or_else(|e| e.into_inner()).push((cle, f.clone()));
    Ok(f)
}

/// Le fichier de Liberation Sans romain (celui dont la fabrique tire ses sous-ensembles), pour
/// l'embarquer dans un PDF existant (`marquage.rs`).
pub fn sans_regulier() -> Result<Arc<Vec<u8>>, Refus> {
    decomprimer(REGULIER_Z)
}

/// Le fichier de Liberation Sans gras, pour le texte gras d'un SVG posé dans un PDF existant (`formulaire.rs`).
pub fn sans_gras() -> Result<Arc<Vec<u8>>, Refus> {
    decomprimer(GRAS_Z)
}

/// Tous les fichiers de Liberation, dans cet ordre : Sans romain, gras, italique, gras italique ; Serif (Times)
/// romain et gras ; Mono (Courier, chasse fixe de 0,6 em) romain et gras. Ce sont les polices du dessin
/// (`dessin::Polices`) et ce que le texte d'un SVG peut demander quand on le pose dans un PDF (`formulaire.rs`).
/// Aucune police du système n'est lue : le même document donne le même dessin sur tous les postes.
pub fn toutes() -> Result<Vec<Arc<Vec<u8>>>, Refus> {
    [REGULIER_Z, GRAS_Z, ITALIQUE_Z, GRAS_ITALIQUE_Z, SERIF_Z, SERIF_GRAS_Z, MONO_Z, MONO_GRAS_Z]
        .iter()
        .map(|z| decomprimer(z))
        .collect()
}

/// Les largeurs d'une police standard (Helvetica, Times, Courier et leurs variantes) qu'un PDF
/// cite sans `/Widths` : celles de Liberation, qui a les métriques de ces polices. Sert à
/// l'extraction de texte (`texte.rs`) et au marquage (`marquage.rs`).
pub struct Metrique {
    donnees: Arc<Vec<u8>>,
}

impl Metrique {
    /// La métrique de la police standard que `base_font` nomme (`None` : aucune police connue).
    pub fn pour(base_font: &str) -> Option<Metrique> {
        let n = base_font.to_ascii_lowercase();
        let n = n.rsplit('+').next().unwrap_or("");
        let gras = n.contains("bold") || n.contains("black") || n.contains("heavy");
        let z = if n.contains("times") || (n.contains("serif") && !n.contains("sans")) {
            if gras { SERIF_GRAS_Z } else { SERIF_Z }
        } else if n.contains("courier") || n.contains("mono") || n.contains("consolas") {
            if gras { MONO_GRAS_Z } else { MONO_Z }
        } else if n.contains("helvetica") || n.contains("arial") || n.contains("sans") || n.is_empty() {
            if gras { GRAS_Z } else { REGULIER_Z }
        } else {
            return None;
        };
        Some(Metrique { donnees: decomprimer(z).ok()? })
    }

    /// La largeur du caractère, en fraction d'em (`None` : la police ne le dessine pas).
    pub fn largeur(&self, c: char) -> Option<crate::mesure::Mesure> {
        let face = ttf_parser::Face::parse(&self.donnees, 0).ok()?;
        let g = face.glyph_index(c)?;
        crate::mesure::Mesure::entier(face.glyph_hor_advance(g)? as i64).sur(crate::mesure::Mesure::entier(face.units_per_em() as i64))
    }
}
