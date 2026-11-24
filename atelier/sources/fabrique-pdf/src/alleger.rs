//! Une image de la compagnie (logo, signature, tampon) allégée avant d'être posée : elle part dans chaque document
//! qui la porte, et chaque document part par courriel et se garde. Le dessin ne change pas à l'œil : l'image est
//! ramenée à la largeur qu'un document imprime, à `RESOLUTION`, puis écrite dans le format qui lui va (PNG pour un
//! dessin à aplats ou une image transparente, JPEG pour une photo) ; la résolution ne baisse, jusqu'à
//! `RESOLUTION_MIN`, que si l'objectif de poids l'exige. Un SVG reste tel quel (un dessin vectoriel, net à toute
//! taille) ; son texte, que les documents n'impriment pas, se dessine en tracés à la pose (`textes_en_traces`). Une
//! image déjà assez fine et assez légère reste telle quelle, octet pour octet.

use image::{DynamicImage, GenericImageView};

use crate::description::Refus;
use crate::mesure;
use crate::mesure::Mesure;

/// La résolution visée, en points par pouce : ce qu'une imprimante de bureau rend d'une image posée.
pub const RESOLUTION: Mesure = Mesure::entier(200);
/// La résolution au-dessous de laquelle on ne descend pas pour tenir l'objectif de poids.
pub const RESOLUTION_MIN: Mesure = Mesure::entier(120);
/// Les qualités JPEG essayées, de la meilleure à la plus légère.
const QUALITES: [u8; 4] = [85, 75, 65, 55];

/// LE FORMAT D'UNE IMAGE POSEE : PNG, JPEG ou SVG, reconnu à ses premiers octets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatDImage {
    Png,
    Jpeg,
    Svg,
}

impl FormatDImage {
    /// Les trois, dans l'ordre ou l'on cherche l'image posee.
    pub const TOUS: [FormatDImage; 3] = [FormatDImage::Png, FormatDImage::Jpeg, FormatDImage::Svg];

    /// L'extension du fichier qui le porte, avec son point.
    pub fn extension(self) -> &'static str {
        match self {
            FormatDImage::Png => ".png",
            FormatDImage::Jpeg => ".jpg",
            FormatDImage::Svg => ".svg",
        }
    }

    /// Son nom, l'extension sans son point.
    pub fn nom(self) -> &'static str {
        match self {
            FormatDImage::Png => "png",
            FormatDImage::Jpeg => "jpg",
            FormatDImage::Svg => "svg",
        }
    }

    /// Son type MIME.
    pub const fn mime(self) -> &'static str {
        match self {
            FormatDImage::Png => "image/png",
            FormatDImage::Jpeg => "image/jpeg",
            FormatDImage::Svg => "image/svg+xml",
        }
    }

    /// LE FORMAT DE CES OCTETS, reconnu a leurs premiers octets et non a une extension, la seule reconnaissance : la
    /// signature d'un PNG ou d'un JPEG ; un SVG commence, apres ses blancs, par `<?xml`, `<svg` ou `<!--`, et porte
    /// `<svg` dans ses 8 192 premiers octets.
    pub fn des_octets(o: &[u8]) -> Option<FormatDImage> {
        if o.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Some(FormatDImage::Png);
        }
        if o.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some(FormatDImage::Jpeg);
        }
        let tete = &o[..o.len().min(2048)];
        // Que des blancs : rien d'un SVG.
        let tete = match tete.iter().position(|c| !c.is_ascii_whitespace() && *c != 0x0b) {
            Some(debut) => &tete[debut..],
            None => return None,
        };
        if tete.starts_with(b"<?xml") || tete.starts_with(b"<svg") || tete.starts_with(b"<!--") {
            let long = o[..o.len().min(8192)].to_ascii_lowercase();
            if long.windows(4).any(|w| w == b"<svg") {
                return Some(FormatDImage::Svg);
            }
        }
        None
    }
}

/// L'image allégée.
pub struct Allegee {
    /// Les octets à poser.
    pub octets: Vec<u8>,
    pub format: FormatDImage,
    /// Sa largeur en pixels (0 pour un SVG).
    pub largeur: u32,
    /// Le poids de l'image reçue.
    pub avant: usize,
    /// L'image reçue est gardée telle quelle.
    pub telle_quelle: bool,
}

fn png(image: &DynamicImage) -> Result<Vec<u8>, Refus> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::ImageEncoder;
    let mut o = Vec::new();
    let image = match image {
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => image.clone(),
        i if i.color().has_alpha() => DynamicImage::ImageRgba8(i.to_rgba8()),
        i => DynamicImage::ImageRgb8(i.to_rgb8()),
    };
    PngEncoder::new_with_quality(&mut o, CompressionType::Best, FilterType::Adaptive)
        .write_image(image.as_bytes(), image.width(), image.height(), image.color())
        .map_err(|e| Refus::defaut(format!("l'image n'a pas pu être écrite en PNG ({})", e)))?;
    Ok(o)
}

fn jpeg(image: &DynamicImage, qualite: u8) -> Result<Vec<u8>, Refus> {
    let mut o = Vec::new();
    let image = if image.color().has_color() { DynamicImage::ImageRgb8(image.to_rgb8()) } else { DynamicImage::ImageLuma8(image.to_luma8()) };
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut o, qualite)
        .encode_image(&image)
        .map_err(|e| Refus::defaut(format!("l'image n'a pas pu être écrite en JPEG ({})", e)))?;
    Ok(o)
}

/// La largeur en pixels d'une image imprimée sur `points` à `resolution`.
fn pixels(points: Mesure, resolution: Mesure) -> u32 {
    (points * resolution).divisee_par(72).plafond().clamp(1, u32::MAX as i64) as u32
}

/// L'écriture de `image` (à sa taille) : PNG, sans perte, si l'image est transparente, si le PNG tient dans `objectif`
/// ou s'il pèse moins que le JPEG de qualité 85 ; sinon le JPEG de la meilleure qualité qui tient dans `objectif`, ou
/// le plus léger.
fn ecrire(image: &DynamicImage, objectif: usize) -> Result<(Vec<u8>, FormatDImage), Refus> {
    let transparente = image.color().has_alpha() && image.pixels().any(|(_, _, p)| p.0[3] < 255);
    // Une image opaque perd son canal alpha : il ne porterait que des 255, et un masque de plus dans chaque document.
    let opaque;
    let image = if !transparente && image.color().has_alpha() {
        opaque = if image.color().has_color() { DynamicImage::ImageRgb8(image.to_rgb8()) } else { DynamicImage::ImageLuma8(image.to_luma8()) };
        &opaque
    } else {
        image
    };
    let en_png = png(image)?;
    if transparente {
        return Ok((en_png, FormatDImage::Png));
    }
    let mut meilleur = jpeg(image, QUALITES[0])?;
    if en_png.len() <= meilleur.len() || en_png.len() <= objectif {
        return Ok((en_png, FormatDImage::Png));
    }
    for q in &QUALITES[1..] {
        if meilleur.len() <= objectif {
            break;
        }
        meilleur = jpeg(image, *q)?;
    }
    Ok((meilleur, FormatDImage::Jpeg))
}

/// `octets` (PNG, JPEG ou SVG) allégée pour être imprimée sur `largeur_points` au plus, à `objectif` octets au
/// plus si la résolution le permet (voir l'en-tête du module).
pub fn alleger(octets: &[u8], largeur_points: Mesure, objectif: usize) -> Result<Allegee, Refus> {
    let format = FormatDImage::des_octets(octets).ok_or_else(|| Refus::illisible("ni un PNG, ni un JPEG, ni un SVG"))?;
    if format == FormatDImage::Svg {
        return Ok(Allegee { octets: octets.to_vec(), format, largeur: 0, avant: octets.len(), telle_quelle: true });
    }
    let image = image::load_from_memory(octets).map_err(|e| Refus::illisible(format!("l'image est illisible ({})", e)))?;
    if image.width() == 0 || image.height() == 0 {
        return Err(Refus::illisible("l'image est vide"));
    }
    let cible = pixels(largeur_points, RESOLUTION);
    if image.width() <= cible + cible / 4 && octets.len() <= objectif {
        return Ok(Allegee { octets: octets.to_vec(), format, largeur: image.width(), avant: octets.len(), telle_quelle: true });
    }
    let mut resolution = RESOLUTION;
    loop {
        let cible = pixels(largeur_points, resolution);
        let reduite = if image.width() > cible { crate::dessin::reduire_a(image.clone(), cible) } else { image.clone() };
        let (o, f) = ecrire(&reduite, objectif)?;
        let plus_bas = resolution * mesure!("0.85");
        if o.len() <= objectif || plus_bas < RESOLUTION_MIN {
            // Une écriture plus lourde que l'image reçue, à la même taille, ne sert à rien.
            if o.len() >= octets.len() && reduite.width() == image.width() {
                return Ok(Allegee { octets: octets.to_vec(), format, largeur: image.width(), avant: octets.len(), telle_quelle: true });
            }
            return Ok(Allegee { octets: o, format: f, largeur: reduite.width(), avant: octets.len(), telle_quelle: false });
        }
        resolution = plus_bas;
    }
}

/// CE QUE `octets` (PNG, JPEG ou SVG) PESE DANS CHAQUE DOCUMENT qui l'imprime sur `largeur_points` : ce que la fabrique
/// en ecrit (`dessin::Image`), ramenee a la resolution d'impression et comprimee comme le PDF l'ecrit. Une image qui ne
/// se lit pas est un refus, celui-la meme que la fabrique opposerait au document.
pub fn poids_dans_un_document(octets: &[u8], largeur_points: Mesure) -> Result<usize, Refus> {
    crate::dessin::Image::lire(octets, largeur_points, false)?.poids_ecrit()
}

/// LE TEXTE D'UN SVG MIS EN TRACES : le SVG reecrit sans texte, chaque lettre dessinee ; ce que ses textes disaient ; et
/// les polices qu'il nomme et que ce poste n'a pas (une autre les a remplacees : l'image le montre).
pub struct TexteEnTraces {
    pub octets: Vec<u8>,
    pub textes: Vec<String>,
    pub polices_absentes: Vec<String>,
}

/// Les familles generiques du CSS : elles designent toujours une police du poste, jamais une absente
/// (`poser_les_familles_generiques`).
const FAMILLES_GENERIQUES: [&str; 5] = ["serif", "sans-serif", "cursive", "fantasy", "monospace"];

/// LES FAMILLES GENERIQUES POSEES SUR DES POLICES DE CE POSTE : fontdb les nomme d'office Times New Roman, Arial, Comic
/// Sans MS, Impact et Courier New, qu'un poste n'a pas toujours. Chacune garde ce nom si le poste l'a, sinon prend la
/// premiere famille de sa liste que le poste a ; sans aucune, elle prend la famille sans empattement du poste, et
/// celle-ci, a defaut de sa liste, la premiere police du poste qui n'est pas a chasse fixe, sinon la premiere tout court.
/// Un poste sans police n'a rien a poser (`textes_en_traces` refuse alors en le disant).
fn poser_les_familles_generiques(polices: &mut usvg::fontdb::Database) {
    use usvg::fontdb::Family;
    let present = |polices: &usvg::fontdb::Database, nom: &str| polices.faces().any(|f| f.families.iter().any(|(n, _)| n == nom));
    let premiere = |polices: &usvg::fontdb::Database, d_office: &Family, liste: &[&str]| -> Option<String> {
        let nom = polices.family_name(d_office);
        if present(polices, nom) {
            return Some(nom.to_string());
        }
        liste.iter().find(|n| present(polices, n)).map(|n| n.to_string())
    };
    let sans = premiere(polices, &Family::SansSerif, &["Arial", "Helvetica", "Liberation Sans", "DejaVu Sans", "Noto Sans", "FreeSans", "Segoe UI", "Verdana"])
        .or_else(|| polices.faces().find(|f| !f.monospaced).or_else(|| polices.faces().next()).and_then(|f| f.families.first()).map(|(n, _)| n.clone()));
    let Some(sans) = sans else { return };
    let serif = premiere(polices, &Family::Serif, &["Times New Roman", "Times", "Liberation Serif", "DejaVu Serif", "Noto Serif", "FreeSerif", "Georgia"]);
    let mono = premiere(polices, &Family::Monospace, &["Courier New", "Courier", "Liberation Mono", "DejaVu Sans Mono", "Noto Sans Mono", "FreeMono", "Consolas"]);
    let cursive = premiere(polices, &Family::Cursive, &[]);
    let fantasy = premiere(polices, &Family::Fantasy, &[]);
    polices.set_serif_family(serif.unwrap_or_else(|| sans.clone()));
    polices.set_monospace_family(mono.unwrap_or_else(|| sans.clone()));
    polices.set_cursive_family(cursive.unwrap_or_else(|| sans.clone()));
    polices.set_fantasy_family(fantasy.unwrap_or_else(|| sans.clone()));
    polices.set_sans_serif_family(sans);
}

/// LE SVG `octets` SANS TEXTE : les documents n'impriment pas le texte d'un SVG (`dessin::Image::svg`), il se dessine
/// donc une fois, a la pose, avec les polices du poste (usvg, `fontdb::Database::load_system_fonts`), et c'est le SVG
/// reecrit qui se garde ; aucun texte, rien a faire (`None`). Un texte qu'aucune police du poste ne dessine se refuse
/// (sur un poste sans police, la demo du navigateur, tout texte).
pub fn textes_en_traces(octets: &[u8]) -> Result<Option<TexteEnTraces>, Refus> {
    use usvg::{TreeParsing as _, TreeWriting as _};
    // Le SVG seul fait foi : une image qu'il citerait par un chemin n'est pas lue.
    let options = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver { resolve_string: Box::new(|_, _| None), ..usvg::ImageHrefResolver::default() },
        ..usvg::Options::default()
    };
    let arbre = usvg::Tree::from_data(octets, &options).map_err(|e| Refus::illisible(format!("SVG illisible : {}", e)))?;
    let mut noeuds = Vec::new();
    noeuds_de_texte(&arbre.root, &mut noeuds);
    if noeuds.is_empty() {
        return Ok(None);
    }
    let mut polices = usvg::fontdb::Database::new();
    polices.load_system_fonts();
    poser_les_familles_generiques(&mut polices);
    let connue = |famille: &str| FAMILLES_GENERIQUES.contains(&famille) || polices.faces().any(|f| f.families.iter().any(|(n, _)| n == famille));
    let mut textes = Vec::new();
    let mut polices_absentes: Vec<String> = Vec::new();
    for noeud in &noeuds {
        // Un texte dont aucune famille n'est sur ce poste prend la famille sans empattement du poste : une autre police
        // le dessine, et la phrase le dit (`polices_absentes`).
        if let usvg::NodeKind::Text(ref mut t) = *noeud.borrow_mut() {
            for span in t.chunks.iter_mut().flat_map(|c| c.spans.iter_mut()) {
                for famille in &span.font.families {
                    if !connue(famille) && !polices_absentes.contains(famille) {
                        polices_absentes.push(famille.clone());
                    }
                }
                if !span.font.families.iter().any(|f| connue(f)) {
                    span.font.families.push("sans-serif".to_string());
                }
            }
        }
        let (dessin, dit) = {
            let usvg::NodeKind::Text(ref t) = *noeud.borrow() else { continue };
            let dit = t.chunks.iter().map(|c| c.text.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
            let Some(parent) = noeud.parent() else {
                return Err(Refus::defaut("SVG : un texte sans parent dans l'arbre lu"));
            };
            use usvg::{NodeExt as _, TextToPath as _};
            (t.convert(&polices, parent.abs_transform().pre_concat(t.transform)), dit)
        };
        match dessin {
            Some(d) => noeud.insert_after(d),
            // Un texte fait de blancs ne dessine rien, et n'a rien a dessiner.
            None if dit.is_empty() => {}
            None if polices.is_empty() => {
                return Err(Refus::a_corriger(format!(
                    "le texte « {} » du SVG ne se dessine pas : aucune police n'est installée ici. Le convertir en tracés dans le logiciel de dessin, ou poser l'image depuis le logiciel installé",
                    dit
                )))
            }
            None => return Err(Refus::a_corriger(format!("le texte « {} » du SVG ne se dessine avec aucune police de ce poste : le convertir en tracés dans le logiciel de dessin", dit))),
        }
        if !dit.is_empty() {
            textes.push(dit);
        }
    }
    for noeud in noeuds {
        noeud.detach();
    }
    Ok(Some(TexteEnTraces { octets: arbre.to_string(&usvg::XmlOptions::default()).into_bytes(), textes, polices_absentes }))
}

/// Les noeuds de texte sous `racine`, ceux des masques, des motifs et des chemins de decoupe compris.
fn noeuds_de_texte(racine: &usvg::Node, noeuds: &mut Vec<usvg::Node>) {
    use usvg::NodeExt as _;
    for noeud in racine.descendants() {
        if let usvg::NodeKind::Text(_) = *noeud.borrow() {
            noeuds.push(noeud.clone());
        }
        noeud.subroots(|sous| noeuds_de_texte(&sous, noeuds));
    }
}
