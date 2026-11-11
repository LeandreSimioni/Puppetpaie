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
