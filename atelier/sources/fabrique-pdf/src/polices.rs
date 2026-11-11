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

use std::sync::OnceLock;

use crate::description::{defaut, Refus};

/// Un fichier de police comprimé par build.rs (zlib : 0,4 Mo de moins dans le programme), décomprimé une fois par
/// processus : chaque document le reprend.
struct Comprimee {
    z: &'static [u8],
    ttf: OnceLock<Vec<u8>>,
}

impl Comprimee {
    const fn de(z: &'static [u8]) -> Comprimee {
        Comprimee { z, ttf: OnceLock::new() }
    }

    fn lire(&'static self) -> Result<&'static [u8], Refus> {
        use std::io::Read;
        if let Some(f) = self.ttf.get() {
            return Ok(f);
        }
        let mut ttf = Vec::new();
        flate2::read::ZlibDecoder::new(self.z)
            .read_to_end(&mut ttf)
            .or_else(|e| defaut(format!("police interne illisible : {}", e)))?;
        Ok(self.ttf.get_or_init(move || ttf))
    }
}

static REGULIER: Comprimee = Comprimee::de(include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSans-Regular.ttf.z")));
static GRAS: Comprimee = Comprimee::de(include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSans-Bold.ttf.z")));
static ITALIQUE: Comprimee = Comprimee::de(include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSans-Italic.ttf.z")));
static GRAS_ITALIQUE: Comprimee = Comprimee::de(include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSans-BoldItalic.ttf.z")));
static SERIF: Comprimee = Comprimee::de(include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSerif-Regular.ttf.z")));
static SERIF_GRAS: Comprimee = Comprimee::de(include_bytes!(concat!(env!("OUT_DIR"), "/LiberationSerif-Bold.ttf.z")));
static MONO: Comprimee = Comprimee::de(include_bytes!(concat!(env!("OUT_DIR"), "/LiberationMono-Regular.ttf.z")));
static MONO_GRAS: Comprimee = Comprimee::de(include_bytes!(concat!(env!("OUT_DIR"), "/LiberationMono-Bold.ttf.z")));

/// Le fichier de Liberation Sans romain (celui dont la fabrique tire ses sous-ensembles), pour
/// l'embarquer dans un PDF existant (`marquage.rs`).
pub fn sans_regulier() -> Result<&'static [u8], Refus> {
    REGULIER.lire()
}

/// Le fichier de Liberation Sans gras, pour le texte gras d'un SVG posé dans un PDF existant (`formulaire.rs`).
pub fn sans_gras() -> Result<&'static [u8], Refus> {
    GRAS.lire()
}

/// Tous les fichiers de Liberation, dans cet ordre : Sans romain, gras, italique, gras italique ; Serif (Times)
/// romain et gras ; Mono (Courier, chasse fixe de 0,6 em) romain et gras. Ce sont les polices du dessin
/// (`dessin::Polices`) et ce que le texte d'un SVG peut demander quand on le pose dans un PDF (`formulaire.rs`).
/// Aucune police du système n'est lue : le même document donne le même dessin sur tous les postes.
pub fn toutes() -> Result<Vec<&'static [u8]>, Refus> {
    [&REGULIER, &GRAS, &ITALIQUE, &GRAS_ITALIQUE, &SERIF, &SERIF_GRAS, &MONO, &MONO_GRAS].iter().map(|p| p.lire()).collect()
}

/// Les largeurs d'une police standard (Helvetica, Times, Courier et leurs variantes) qu'un PDF
/// cite sans `/Widths` : celles de Liberation, qui a les métriques de ces polices. Sert à
/// l'extraction de texte (`texte.rs`) et au marquage (`marquage.rs`).
pub struct Metrique {
    face: ttf_parser::Face<'static>,
    unites: std::num::NonZeroI64,
}

impl Metrique {
    /// La métrique de la police standard que `base_font` nomme (`None` : aucune police connue).
    pub fn pour(base_font: &str) -> Result<Option<Metrique>, Refus> {
        let minuscules = base_font.to_ascii_lowercase();
        // Le nom sans le préfixe d'un sous-ensemble (« ABCDEF+ », ISO 32000-1, 9.6.4).
        let n = match minuscules.rsplit_once('+') {
            Some((_, apres)) => apres,
            None => minuscules.as_str(),
        };
        let gras = n.contains("bold") || n.contains("black") || n.contains("heavy");
        let p = if n.contains("times") || (n.contains("serif") && !n.contains("sans")) {
            if gras { &SERIF_GRAS } else { &SERIF }
        } else if n.contains("courier") || n.contains("mono") || n.contains("consolas") {
            if gras { &MONO_GRAS } else { &MONO }
        } else if n.contains("helvetica") || n.contains("arial") || n.contains("sans") {
            if gras { &GRAS } else { &REGULIER }
        } else {
            return Ok(None);
        };
        let face = ttf_parser::Face::parse(p.lire()?, 0).or_else(|e| defaut(format!("police interne illisible : {}", e)))?;
        let Some(unites) = std::num::NonZeroI64::new(face.units_per_em() as i64) else {
            return defaut("police interne illisible : sans unités");
        };
        Ok(Some(Metrique { face, unites }))
    }

    /// La largeur du caractère, en fraction d'em (`None` : la police ne le dessine pas).
    pub fn largeur(&self, c: char) -> Option<crate::mesure::Mesure> {
        let g = self.face.glyph_index(c)?;
        Some(crate::mesure::Mesure::UN.fois(self.face.glyph_hor_advance(g)? as i64, self.unites))
    }
}
