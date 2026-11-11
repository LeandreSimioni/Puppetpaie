//! LE DESSIN D'UN PDF, en mesures exactes : les polices et leurs métriques, les images, les pages et ce qu'on y
//! pose (du texte, des traits, des aplats, des images), puis le fichier.
//!
//! Les coordonnées se donnent depuis le coin haut gauche de la page, en points (`Mesure`) ; le PDF compte depuis le
//! coin bas gauche, et la conversion se fait ici, à l'écriture. Les nombres s'écrivent en texte : les positions au
//! dix-millième (`marquage::nb`, moins d'un dix-millième de point d'écart), le corps d'un texte au millionième.
//!
//! Le texte s'écrit avec des polices embarquées en sous-ensemble (`sous_ensemble`) : codes de deux octets qui sont
//! le point de code Unicode, et seuls les glyphes imprimés.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroI64;
use std::rc::Rc;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat};

use crate::description::{defaut, Refus, Rvb};
use crate::marquage::nb;
use crate::mesure::Mesure;

// ---------------------------------------------------------------------------
// Les polices.
// ---------------------------------------------------------------------------

/// La famille d'une police : Liberation Sans (Helvetica), Serif (Times) ou Mono (Courier).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Famille {
    Sans,
    Times,
    Courier,
}

/// Le style d'un texte : sa police (famille, gras, italique), son corps et sa couleur.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Style {
    pub famille: Famille,
    pub gras: bool,
    pub italique: bool,
    pub corps: Mesure,
    pub couleur: Rvb,
}

impl Style {
    /// Liberation Sans romain, noir, au corps `corps`.
    pub fn new(corps: Mesure) -> Style {
        Style { famille: Famille::Sans, gras: false, italique: false, corps, couleur: (0, 0, 0) }
    }

    pub fn gras(self, oui: bool) -> Style {
        Style { gras: self.gras || oui, ..self }
    }

    pub fn italique(self, oui: bool) -> Style {
        Style { italique: self.italique || oui, ..self }
    }

    pub fn couleur(self, couleur: Rvb) -> Style {
        Style { couleur, ..self }
    }

    pub fn corps(self, corps: Mesure) -> Style {
        Style { corps, ..self }
    }

    pub fn famille(self, famille: Famille) -> Style {
        Style { famille, ..self }
    }

    /// Le fichier de police (dans l'ordre de `polices::toutes`) : Sans en quatre styles, Serif et Mono en romain et
    /// gras (leur italique reprend le romain, ou le gras, du même fichier).
    fn fichier(&self) -> usize {
        match self.famille {
            Famille::Sans => self.gras as usize + 2 * self.italique as usize,
            Famille::Times => 4 + self.gras as usize,
            Famille::Courier => 6 + self.gras as usize,
        }
    }
}

/// Un fichier de police et ses métriques, en unités de la police.
struct Fonte {
    face: ttf_parser::Face<'static>,
    unites: NonZeroI64,
    /// La hauteur au-dessus et au-dessous de la ligne de base (`hhea`, la seconde négative).
    ascendante: i64,
    descendante: i64,
    /// L'avance de chaque glyphe, par son numéro (`hmtx`), lue une fois : le glyphe 0 est `.notdef`.
    avances: Vec<i64>,
}

impl Fonte {
    fn lire(donnees: &'static [u8]) -> Result<Fonte, Refus> {
        let face = ttf_parser::Face::parse(donnees, 0).or_else(|e| defaut(format!("police interne illisible : {}", e)))?;
        let Some(unites) = NonZeroI64::new(face.units_per_em() as i64) else {
            return defaut("police interne illisible : sans unités");
        };
        let hhea = face.tables().hhea;
        let (ascendante, descendante) = (hhea.ascender as i64, hhea.descender as i64);
        let avances = (0..face.number_of_glyphs())
            .map(|g| match face.glyph_hor_advance(ttf_parser::GlyphId(g)) {
                Some(a) => Ok(a as i64),
                None => defaut(format!("police interne illisible : le glyphe {} n'a pas d'avance", g)),
            })
            .collect::<Result<Vec<i64>, Refus>>()?;
        if avances.is_empty() {
            return defaut("police interne illisible : sans glyphe");
        }
        Ok(Fonte { face, unites, ascendante, descendante, avances })
    }

    /// Le glyphe du caractère (`None` : la police ne l'a pas) et son avance ; un caractère absent a l'avance du
    /// glyphe `.notdef`.
    fn glyphe(&self, c: char) -> (Option<u16>, i64) {
        let id = self.face.glyph_index(c).filter(|g| g.0 != 0).map(|g| g.0);
        let numero = match id {
            Some(g) => g as usize,
            None => NOTDEF,
        };
        (id, self.avances[numero])
    }

    /// `v` unités de la police, au corps `corps`, en points.
    fn en_points(&self, v: i64, corps: Mesure) -> Mesure {
        corps.fois(v, self.unites)
    }
}

/// Le glyphe `.notdef` d'une police TrueType : le premier.
const NOTDEF: usize = 0;

/// Les polices d'un document : les huit fichiers de Liberation.
pub struct Polices {
    fontes: Vec<Fonte>,
}

impl Polices {
    pub fn charger() -> Result<Polices, Refus> {
        Ok(Polices { fontes: crate::polices::toutes()?.into_iter().map(Fonte::lire).collect::<Result<_, _>>()? })
    }

    fn fonte(&self, st: &Style) -> &Fonte {
        &self.fontes[st.fichier()]
    }

    /// La largeur du texte, sans crénage : la somme des avances, au corps du style.
    pub fn largeur(&self, st: &Style, texte: &str) -> Mesure {
        let f = self.fonte(st);
        let unites: i64 = texte.chars().map(|c| f.glyphe(c).1).sum();
        f.en_points(unites, st.corps)
    }

    /// La police du style dessine le caractère (du plan de base).
    pub fn possede(&self, st: &Style, c: char) -> bool {
        (c as u32) <= 0xFFFF && self.fonte(st).glyphe(c).0.is_some()
    }

    /// La hauteur de la police au-dessus de la ligne de base.
    pub fn ascendante(&self, st: &Style) -> Mesure {
        let f = self.fonte(st);
        f.en_points(f.ascendante, st.corps)
    }

    /// La hauteur de la police au-dessous de la ligne de base (positive).
    pub fn descendante(&self, st: &Style) -> Mesure {
        let f = self.fonte(st);
        f.en_points(-f.descendante, st.corps)
    }

    /// La hauteur d'un glyphe : l'ascendante et la descendante.
    pub fn hauteur(&self, st: &Style) -> Mesure {
        let f = self.fonte(st);
        f.en_points(f.ascendante - f.descendante, st.corps)
    }
}

// ---------------------------------------------------------------------------
// Les images.
// ---------------------------------------------------------------------------

/// Le message des SVG qui demandent plus que des tracés et des aplats unis.
pub const SVG_NON_PRIS_EN_CHARGE: &str = "SVG avec dégradé, motif, transparence ou image intégrée : seuls les tracés et les aplats unis sont pris en charge";

/// La résolution, en pixels par pouce, à laquelle une image est ramenée à sa largeur posée : celle de l'impression
/// courante. Au-delà, les pixels ne s'impriment ni ne se voient, ils ne font que peser.
const RESOLUTION: i64 = 300;
/// Un point PDF est le 72e d'un pouce.
const POINTS_PAR_POUCE: NonZeroI64 = match NonZeroI64::new(72) {
    Some(n) => n,
    None => panic!("72 n'est pas nul"),
};

/// La qualité d'un JPEG réencodé après réduction (sur 100).
const QUALITE_JPEG: u8 = 90;

/// Ce qu'une image écrit dans le PDF.
#[derive(PartialEq)]
enum Contenu {
    /// Des pixels : `filtre` (`DCTDecode` pour un JPEG recopié) ou bruts, et le plan alpha d'une image transparente.
    Pixels { largeur: u32, hauteur: u32, espace: &'static str, filtre: Option<&'static str>, donnees: Vec<u8>, alpha: Option<Vec<u8>> },
    /// Un SVG, écrit en objet formulaire (`Image::forme`) ; `source` : le SVG lu.
    Svg { source: Vec<u8> },
}

/// Une image posée à une largeur donnée ; sa hauteur suit ses proportions.
#[derive(Clone)]
pub struct Image {
    contenu: Rc<Contenu>,
    forme: Option<Rc<Stream>>,
    largeur: Mesure,
    hauteur: Mesure,
}

/// Le format d'une image, reconnu à son contenu.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Png,
    Jpeg,
    Svg,
}

/// Le format reconnu à l'octet de tête : signature PNG, signature JPEG, sinon balise `<svg` dans les premiers octets.
fn reconnaitre(data: &[u8]) -> Option<Format> {
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(Format::Png)
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Format::Jpeg)
    } else {
        let tete = &data[..data.len().min(4096)];
        tete.windows(4).any(|w| w.eq_ignore_ascii_case(b"<svg")).then_some(Format::Svg)
    }
}

impl Image {
    /// L'image (PNG, JPEG ou SVG, reconnue à son contenu ; `svg` : l'extension dit SVG) posée à `largeur` points.
    pub fn lire(donnees: &[u8], largeur: Mesure, svg: bool) -> Result<Image, String> {
        match reconnaitre(donnees) {
            Some(Format::Svg) => Image::svg(donnees, largeur),
            Some(f) => Image::pixels(donnees, f, largeur),
            None if svg => Image::svg(donnees, largeur),
            None => Err("format d'image non reconnu (ni PNG, ni JPEG, ni SVG)".into()),
        }
    }

    pub fn largeur(&self) -> Mesure {
        self.largeur
    }

    pub fn hauteur(&self) -> Mesure {
        self.hauteur
    }

    fn pixels(donnees: &[u8], format: Format, largeur: Mesure) -> Result<Image, String> {
        let (format_image, nom) = match format {
            Format::Png => (image::ImageFormat::Png, "PNG"),
            _ => (image::ImageFormat::Jpeg, "JPEG"),
        };
        // On décode toujours : un fichier tronqué ou corrompu est un refus, pas un blanc.
        let decodee = image::load_from_memory_with_format(donnees, format_image).map_err(|e| format!("{} illisible : {}", nom, e))?;
        let (Some(large), true) = (NonZeroI64::new(decodee.width() as i64), decodee.height() > 0) else {
            return Err(format!("{} vide", nom));
        };
        // Les proportions posées sont celles de l'original : la réduction arrondit les pixels.
        let hauteur = largeur.fois(decodee.height() as i64, large);
        let (decodee, reduite) = reduire(decodee, largeur);
        let (l, h) = (decodee.width(), decodee.height());
        let contenu = match (format, composantes_jpeg(donnees)) {
            // JPEG ramené à la résolution d'impression : réencodé en JPEG.
            (Format::Jpeg, Some(n)) if reduite && (n == 1 || n == 3) => {
                let (pixels, espace, couleur) = if n == 1 {
                    (decodee.to_luma8().into_raw(), "DeviceGray", image::ColorType::L8)
                } else {
                    (decodee.to_rgb8().into_raw(), "DeviceRGB", image::ColorType::Rgb8)
                };
                let mut jpeg = Vec::new();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, QUALITE_JPEG)
                    .encode(&pixels, l, h, couleur)
                    .map_err(|e| format!("JPEG : réduction impossible : {}", e))?;
                Contenu::Pixels { largeur: l, hauteur: h, espace, filtre: Some("DCTDecode"), donnees: jpeg, alpha: None }
            }
            // JPEG en niveaux de gris ou en couleurs, déjà à la bonne résolution : recopié tel quel.
            (Format::Jpeg, Some(n)) if n == 1 || n == 3 => Contenu::Pixels {
                largeur: l,
                hauteur: h,
                espace: if n == 1 { "DeviceGray" } else { "DeviceRGB" },
                filtre: Some("DCTDecode"),
                donnees: donnees.to_vec(),
                alpha: None,
            },
            // Le reste (PNG, JPEG CMJN ou exotique) : les pixels décodés, en niveaux de gris, en RVB, ou en RVB avec
            // un masque de transparence.
            _ if decodee.color().has_alpha() => {
                let rvba = decodee.to_rgba8();
                let mut rvb = Vec::with_capacity(rvba.len() / 4 * 3);
                let mut alpha = Vec::with_capacity(rvba.len() / 4);
                for p in rvba.pixels() {
                    rvb.extend_from_slice(&p.0[..3]);
                    alpha.push(p.0[3]);
                }
                Contenu::Pixels { largeur: l, hauteur: h, espace: "DeviceRGB", filtre: None, donnees: rvb, alpha: Some(alpha) }
            }
            _ if decodee.color().has_color() => {
                Contenu::Pixels { largeur: l, hauteur: h, espace: "DeviceRGB", filtre: None, donnees: decodee.to_rgb8().into_raw(), alpha: None }
            }
            _ => Contenu::Pixels { largeur: l, hauteur: h, espace: "DeviceGray", filtre: None, donnees: decodee.to_luma8().into_raw(), alpha: None },
        };
        Ok(Image { contenu: Rc::new(contenu), forme: None, largeur, hauteur })
    }

    fn svg(donnees: &[u8], largeur: Mesure) -> Result<Image, String> {
        use usvg::TreeParsing as _;
        let texte = std::str::from_utf8(donnees).map_err(|_| "SVG illisible : pas de l'UTF-8".to_string())?;
        if texte.contains("<text") {
            return Err("SVG avec du texte : le texte n'est pas pris en charge (à convertir en tracés)".into());
        }
        let arbre = usvg::Tree::from_str(texte, &usvg::Options::default()).map_err(|e| format!("SVG illisible : {}", e))?;
        let (l, h) = match (Mesure::par_son_texte(arbre.size.width()), Mesure::par_son_texte(arbre.size.height())) {
            (Some(l), Some(h)) if l > Mesure::ZERO && h > Mesure::ZERO => (l, h),
            _ => return Err("SVG sans dimensions".into()),
        };
        // svg2pdf écrit l'objet formulaire (le dessin dans le carré unité) dans un PDF minimal, que lopdf relit.
        let mut pdf = pdf_writer::Pdf::new();
        let (catalogue, arbre_pages) = (pdf_writer::Ref::new(1), pdf_writer::Ref::new(2));
        pdf.catalog(catalogue).pages(arbre_pages);
        pdf.pages(arbre_pages).count(0);
        let depart = pdf_writer::Ref::new(3);
        let mut morceau = pdf_writer::Chunk::new();
        svg2pdf::convert_tree_into(&arbre, svg2pdf::Options::default(), &mut morceau, depart);
        pdf.extend(&morceau);
        let lu = Document::load_mem(&pdf.finish()).map_err(|e| format!("SVG illisible : {}", e))?;
        let forme = match lu.get_object((depart.get() as u32, 0)) {
            Ok(Object::Stream(s)) => s.clone(),
            Ok(_) => return Err("SVG illisible : la conversion ne donne pas un objet formulaire".to_string()),
            Err(e) => return Err(format!("SVG illisible : la conversion n'a rien donné ({})", e)),
        };
        // Un objet formulaire qui en cite d'autres (dégradés, motifs, états de transparence, images intégrées)
        // demande plus que des tracés et des aplats unis.
        let mut cites = Vec::new();
        crate::ecriture::references(&Object::Stream(forme.clone()), &mut cites);
        if !cites.is_empty() {
            return Err(SVG_NON_PRIS_EN_CHARGE.into());
        }
        let contenu = Contenu::Svg { source: donnees.to_vec() };
        let Some(large) = NonZeroI64::new(l.en_millioniemes()) else {
            return Err("SVG sans dimensions".into());
        };
        let hauteur = largeur.fois(h.en_millioniemes(), large);
        Ok(Image { contenu: Rc::new(contenu), forme: Some(Rc::new(forme)), largeur, hauteur })
    }
}

/// L'image ramenée à `RESOLUTION` à la largeur `largeur` (en points), si elle la dépasse de plus d'un quart (en
/// deçà, l'écart ne vaut pas un rééchantillonnage) ; `true` si elle a été réduite.
fn reduire(image: image::DynamicImage, largeur: Mesure) -> (image::DynamicImage, bool) {
    let cible = largeur.fois(RESOLUTION, POINTS_PAR_POUCE).plafond().clamp(1, u32::MAX as i64) as u32;
    if image.width() <= cible + cible / 4 {
        return (image, false);
    }
    (reduire_a(image, cible), true)
}

/// L'image ramenée à `cible` pixels de large, les proportions gardées (Lanczos 3). Une image avec transparence est
/// réduite en couleurs prémultipliées (sur seize bits) : la couleur d'un pixel transparent ne déteint pas sur le bord
/// du dessin.
pub fn reduire_a(image: image::DynamicImage, cible: u32) -> image::DynamicImage {
    use image::imageops::FilterType;
    let (l, h) = (image.width() as u64, image.height() as u64);
    let hauteur = ((2 * h * cible as u64 + l) / (2 * l)).max(1) as u32;
    if !image.color().has_alpha() {
        return image.resize_exact(cible, hauteur, FilterType::Lanczos3);
    }
    let mut seize = image.to_rgba16();
    for p in seize.pixels_mut() {
        let a = p.0[3] as u32;
        for c in 0..3 {
            p.0[c] = ((p.0[c] as u32 * a + 32_767) / 65_535) as u16;
        }
    }
    let mut r = image::imageops::resize(&seize, cible, hauteur, FilterType::Lanczos3);
    for p in r.pixels_mut() {
        let a = p.0[3] as u32;
        for c in 0..3 {
            p.0[c] = if a > 0 { ((p.0[c] as u32 * 65_535 + a / 2) / a).min(65_535) as u16 } else { 0 };
        }
    }
    let reduite = image::DynamicImage::ImageRgba16(r);
    // Revenir aux huit bits par composante de l'original (couleur ou niveaux de gris).
    if image.color().has_color() {
        image::DynamicImage::ImageRgba8(reduite.to_rgba8())
    } else {
        image::DynamicImage::ImageLumaA8(reduite.to_luma_alpha8())
    }
}

/// Le nombre de composantes d'un JPEG recopiable tel quel (DCT de base ou progressive, 8 bits), ou `None`.
fn composantes_jpeg(data: &[u8]) -> Option<u8> {
    let mut i = 2; // après SOI
    while i + 4 <= data.len() {
        if data[i] != 0xFF {
            return None;
        }
        let marqueur = data[i + 1];
        match marqueur {
            0xFF => {
                i += 1;
                continue;
            }
            0x01 | 0xD0..=0xD8 => {
                i += 2;
                continue;
            }
            _ => {}
        }
        let longueur = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        if matches!(marqueur, 0xC0 | 0xC1 | 0xC2) {
            // SOFn : longueur, précision, hauteur, largeur, composantes.
            if i + 10 > data.len() || data[i + 4] != 8 {
                return None;
            }
            return Some(data[i + 9]);
        }
        if marqueur == 0xDA {
            return None;
        }
        i += 2 + longueur;
    }
    None
}

// ---------------------------------------------------------------------------
// Les pages.
// ---------------------------------------------------------------------------

/// Une page en cours : son flux de contenu, et l'état graphique qu'il a posé (couleurs, épaisseur du trait).
struct Page {
    contenu: String,
    remplissage: Rvb,
    contour: Rvb,
    epaisseur: Mesure,
    fontes: BTreeSet<usize>,
    images: BTreeSet<usize>,
}

/// Une couleur en composantes PDF (de 0 à 1).
fn composantes((r, v, b): Rvb) -> String {
    let c = |x: u8| nb(Mesure::entier(x as i64) / 255);
    format!("{} {} {}", c(r), c(v), c(b))
}

/// Des codes de deux octets en chaîne hexadécimale.
fn hexa(codes: &[char]) -> String {
    let mut s = String::from("<");
    for c in codes {
        s.push_str(&format!("{:04X}", *c as u32));
    }
    s.push('>');
    s
}

/// Un document en cours de dessin : sa taille de page, ses pages, les caractères imprimés avec chaque police et les
/// images posées.
pub struct Dessin {
    largeur: Mesure,
    hauteur: Mesure,
    pages: Vec<Page>,
    caracteres: BTreeMap<usize, BTreeSet<char>>,
    images: Vec<Image>,
}

impl Dessin {
    /// Un document vide aux pages de `largeur` × `hauteur` points.
    pub fn new(largeur: Mesure, hauteur: Mesure) -> Dessin {
        Dessin { largeur, hauteur, pages: Vec::new(), caracteres: BTreeMap::new(), images: Vec::new() }
    }

    /// Ouvre une page, où tout ce qui suit se pose.
    pub fn nouvelle_page(&mut self) {
        self.pages.push(Page {
            contenu: String::new(),
            remplissage: (0, 0, 0),
            contour: (0, 0, 0),
            epaisseur: Mesure::UN,
            fontes: BTreeSet::new(),
            images: BTreeSet::new(),
        });
    }

    pub fn nombre_de_pages(&self) -> usize {
        self.pages.len()
    }

    fn page(&mut self) -> &mut Page {
        if self.pages.is_empty() {
            self.nouvelle_page();
        }
        self.pages.last_mut().expect("une page")
    }

    /// L'ordonnée PDF (depuis le bas) d'une ordonnée de la page (depuis le haut).
    fn y(&self, y: Mesure) -> Mesure {
        self.hauteur - y
    }

    fn remplissage(page: &mut Page, couleur: Rvb) {
        if page.remplissage != couleur {
            page.remplissage = couleur;
            page.contenu.push_str(&format!("{} rg\n", composantes(couleur)));
        }
    }

    /// Une ligne de texte : `morceaux` à la suite, le premier à l'abscisse `x`, sur la ligne de base `base`. `espace` :
    /// ce qu'on ajoute à chaque espace ordinaire (U+0020), en points (la justification ; l'espace insécable ne s'étire
    /// pas). L'opérateur `Tw` ne s'applique pas aux codes de deux octets : un `TJ` décale le glyphe qui suit chaque
    /// espace, en millièmes du corps, au centième.
    pub fn texte(&mut self, x: Mesure, base: Mesure, morceaux: &[(Style, &str)], espace: Mesure) {
        let y = self.y(base);
        let mut police: Option<(usize, Mesure)> = None;
        for (st, t) in morceaux {
            self.caracteres.entry(st.fichier()).or_default().extend(t.chars());
        }
        let page = self.page();
        page.contenu.push_str(&format!("BT\n{} {} Td\n", nb(x), nb(y)));
        for (st, t) in morceaux {
            Dessin::remplissage(page, st.couleur);
            let f = st.fichier();
            page.fontes.insert(f);
            if police != Some((f, st.corps)) {
                police = Some((f, st.corps));
                // Le corps au millionième : il multiplie toute la largeur de la ligne.
                page.contenu.push_str(&format!("/F{} {} Tf\n", f, st.corps));
            }
            let codes: Vec<char> = t.chars().collect();
            // Sans espace à étirer (ou sans corps, où rien ne s'étire), le texte tel quel.
            let rapport = match (espace * Mesure::entier(1000)).sur(st.corps) {
                Some(r) if !espace.est_nulle() && codes.contains(&' ') => r,
                Some(_) | None => {
                    page.contenu.push_str(&format!("{} Tj\n", hexa(&codes)));
                    continue;
                }
            };
            // Un décalage positif recule le glyphe suivant : l'étirement est son opposé.
            let etirement = -rapport.arrondie(2);
            let mut tableau = String::from("[");
            let mut debut = 0;
            for (i, c) in codes.iter().enumerate() {
                if *c == ' ' {
                    tableau.push_str(&format!("{} {} ", hexa(&codes[debut..=i]), etirement));
                    debut = i + 1;
                }
            }
            if debut < codes.len() {
                tableau.push_str(&hexa(&codes[debut..]));
            }
            page.contenu.push_str(&format!("{}] TJ\n", tableau.trim_end()));
        }
        page.contenu.push_str("ET\n");
    }

    /// Un trait qui suit `points`, d'épaisseur `epaisseur` et de couleur `couleur`.
    pub fn trait_(&mut self, points: &[(Mesure, Mesure)], epaisseur: Mesure, couleur: Rvb) {
        let points: Vec<(Mesure, Mesure)> = points.iter().map(|(x, y)| (*x, self.y(*y))).collect();
        let page = self.page();
        if page.epaisseur != epaisseur {
            page.epaisseur = epaisseur;
            page.contenu.push_str(&format!("{} w\n", nb(epaisseur)));
        }
        if page.contour != couleur {
            page.contour = couleur;
            page.contenu.push_str(&format!("{} RG\n", composantes(couleur)));
        }
        for (i, (x, y)) in points.iter().enumerate() {
            page.contenu.push_str(&format!("{} {} {}\n", nb(*x), nb(*y), if i == 0 { "m" } else { "l" }));
        }
        page.contenu.push_str("S\n");
    }

    /// Un rectangle plein, de coin haut gauche (`x`, `y`).
    pub fn rectangle(&mut self, x: Mesure, y: Mesure, largeur: Mesure, hauteur: Mesure, couleur: Rvb) {
        let (haut, bas) = (self.y(y), self.y(y + hauteur));
        let page = self.page();
        Dessin::remplissage(page, couleur);
        page.contenu.push_str(&format!(
            "{x} {haut} m\n{d} {haut} l\n{d} {bas} l\n{x} {bas} l\nh\nf\n",
            x = nb(x),
            d = nb(x + largeur),
            haut = nb(haut),
            bas = nb(bas)
        ));
    }

    /// L'image `image`, de coin haut gauche (`x`, `y`), à sa taille. Une image déjà posée (le logo de chaque page,
    /// la même image deux fois) ne s'écrit qu'une fois dans le fichier.
    pub fn image(&mut self, image: &Image, x: Mesure, y: Mesure) {
        let n = match self.images.iter().position(|i| Rc::ptr_eq(&i.contenu, &image.contenu) || *i.contenu == *image.contenu) {
            Some(n) => n,
            None => {
                self.images.push(image.clone());
                self.images.len() - 1
            }
        };
        let bas = self.y(y + image.hauteur);
        let page = self.page();
        page.images.insert(n);
        page.contenu.push_str(&format!(
            "q\n{} 0 0 {} {} {} cm\n/I{} Do\nQ\n",
            nb(image.largeur),
            nb(image.hauteur),
            nb(x),
            nb(bas),
            n
        ));
    }

    /// Le fichier PDF : les polices en sous-ensemble, les images, les pages, puis l'écriture au plus court, dans
    /// un ordre canonique (le même document donne toujours les mêmes octets ; les dates et l'identifiant sont fixes).
    pub fn finir(mut self, titre: &str) -> Result<Vec<u8>, Refus> {
        if self.pages.is_empty() {
            self.nouvelle_page();
        }
        let mut doc = Document::with_version("1.5");
        let fichiers = crate::polices::toutes()?;
        let mut polices: BTreeMap<usize, ObjectId> = BTreeMap::new();
        for (f, caracteres) in &self.caracteres {
            if !caracteres.is_empty() {
                polices.insert(*f, crate::sous_ensemble::embarquer(&mut doc, &fichiers[*f], caracteres)?);
            }
        }
        let mut images: Vec<ObjectId> = Vec::new();
        for i in &self.images {
            let objet = match (&*i.contenu, &i.forme) {
                (Contenu::Pixels { largeur, hauteur, espace, filtre, donnees, alpha }, _) => {
                    let mut d = Dictionary::new();
                    d.set("Type", Object::Name(b"XObject".to_vec()));
                    d.set("Subtype", Object::Name(b"Image".to_vec()));
                    d.set("Width", Object::Integer(*largeur as i64));
                    d.set("Height", Object::Integer(*hauteur as i64));
                    d.set("ColorSpace", Object::Name(espace.as_bytes().to_vec()));
                    d.set("BitsPerComponent", Object::Integer(8));
                    d.set("Interpolate", Object::Boolean(true));
                    if let Some(f) = filtre {
                        d.set("Filter", Object::Name(f.as_bytes().to_vec()));
                    }
                    if let Some(a) = alpha {
                        let mut m = Dictionary::new();
                        m.set("Type", Object::Name(b"XObject".to_vec()));
                        m.set("Subtype", Object::Name(b"Image".to_vec()));
                        m.set("Width", Object::Integer(*largeur as i64));
                        m.set("Height", Object::Integer(*hauteur as i64));
                        m.set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
                        m.set("BitsPerComponent", Object::Integer(8));
                        let mut masque = Stream::new(m, a.clone());
                        crate::ecriture::predire_image(&mut masque)?;
                        d.set("SMask", Object::Reference(doc.add_object(Object::Stream(masque))));
                    }
                    let mut s = Stream::new(d, donnees.clone());
                    crate::ecriture::predire_image(&mut s)?;
                    Object::Stream(s)
                }
                (Contenu::Svg { .. }, Some(forme)) => Object::Stream((**forme).clone()),
                (Contenu::Svg { .. }, None) => return defaut("écriture du PDF impossible : un SVG sans dessin"),
            };
            images.push(doc.add_object(objet));
        }
        let arbre = doc.new_object_id();
        let mut enfants = Vec::new();
        for p in &self.pages {
            let mut ressources = Dictionary::new();
            if !p.fontes.is_empty() {
                let mut d = Dictionary::new();
                for f in &p.fontes {
                    if let Some(id) = polices.get(f) {
                        d.set(format!("F{}", f), Object::Reference(*id));
                    }
                }
                ressources.set("Font", Object::Dictionary(d));
            }
            if !p.images.is_empty() {
                let mut d = Dictionary::new();
                for n in &p.images {
                    d.set(format!("I{}", n), Object::Reference(images[*n]));
                }
                ressources.set("XObject", Object::Dictionary(d));
            }
            let mut page = Dictionary::new();
            page.set("Type", Object::Name(b"Page".to_vec()));
            page.set("Parent", Object::Reference(arbre));
            page.set(
                "MediaBox",
                Object::Array(vec![Object::Integer(0), Object::Integer(0), self.largeur.en_objet()?, self.hauteur.en_objet()?]),
            );
            page.set("Resources", Object::Dictionary(ressources));
            if !p.contenu.is_empty() {
                let flux = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), p.contenu.clone().into_bytes())));
                page.set("Contents", Object::Reference(flux));
            }
            enfants.push(Object::Reference(doc.add_object(Object::Dictionary(page))));
        }
        let mut pages = Dictionary::new();
        pages.set("Type", Object::Name(b"Pages".to_vec()));
        pages.set("Count", Object::Integer(enfants.len() as i64));
        pages.set("Kids", Object::Array(enfants));
        doc.objects.insert(arbre, Object::Dictionary(pages));
        let mut catalogue = Dictionary::new();
        catalogue.set("Type", Object::Name(b"Catalog".to_vec()));
        catalogue.set("Pages", Object::Reference(arbre));
        let racine = doc.add_object(Object::Dictionary(catalogue));
        // Les propriétés : le titre (en UTF-16, sa marque en tête), le nom du logiciel, des dates fixes.
        let mut titre_utf16 = vec![0xFE, 0xFF];
        for u in titre.encode_utf16() {
            titre_utf16.extend_from_slice(&u.to_be_bytes());
        }
        let date = || Object::String(b"D:19700101000000+00'00'".to_vec(), StringFormat::Literal);
        let mut info = Dictionary::new();
        info.set("Title", Object::String(titre_utf16, StringFormat::Literal));
        info.set("Creator", Object::String(b"PuppetPaie".to_vec(), StringFormat::Literal));
        info.set("Producer", Object::String(b"PuppetPaie".to_vec(), StringFormat::Literal));
        info.set("CreationDate", date());
        info.set("ModDate", date());
        let info = doc.add_object(Object::Dictionary(info));
        let identifiant = || Object::String(IDENTIFIANT.as_bytes().to_vec(), StringFormat::Literal);
        doc.trailer.set("Root", Object::Reference(racine));
        doc.trailer.set("Info", Object::Reference(info));
        doc.trailer.set("ID", Object::Array(vec![identifiant(), identifiant()]));
        doc.compress();
        crate::ecriture::canonique(&mut doc);
        crate::ecriture::compact(&doc)
    }
}

/// L'identifiant fixe écrit dans la queue du fichier (le même document donne les mêmes octets).
const IDENTIFIANT: &str = "50757070657450616965000000000001";
