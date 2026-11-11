//! UN CLASSEUR EXCEL (.xlsx) ECRIT PAR LE LOGICIEL : une feuille, ses cellules (texte, ou nombre exact), et ce que nos
//! etats en mettent en forme (gras, corps, retour a la ligne, alignement, format de nombre, largeur des colonnes,
//! hauteur d'une ligne, lignes figees, cellules fusionnees), et rien d'autre.
//!
//! Un .xlsx est une archive zip de XML (Office Open XML, ECMA-376, partie 1 : SpreadsheetML) : `[Content_Types].xml`,
//! `_rels/.rels`, `xl/workbook.xml` et ses liens, `xl/styles.xml`, `xl/worksheets/sheet1.xml`. Un nombre s'y ecrit en
//! texte (`<v>1234.56</v>`) : le decimal s'y ecrit exact, dans sa forme la plus courte. Un texte s'ecrit en ligne
//! (`t="inlineStr"`), comme `socle::lecture_xlsx` le relit.

use std::collections::BTreeMap;
use std::io::Write;

use socle::echec::{erreur, Classe, R};
use socle::num::Decimal;

/// L'alignement vertical d'une cellule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Vertical {
    Centre,
    Haut,
}

/// La mise en forme d'une cellule ; par defaut, celle du classeur (Calibri 11, sans rien de plus).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Style {
    pub gras: bool,
    /// Le corps de la police, en points ; sans lui, 11.
    pub corps: Option<u32>,
    pub retour_a_la_ligne: bool,
    /// Centre horizontalement.
    pub centre: bool,
    pub vertical: Option<Vertical>,
    /// Le format d'un nombre (`# ##0.00 €`).
    pub format: Option<&'static str>,
}

impl Style {
    /// Celle du classeur : rien de plus que sa police.
    pub const CLASSEUR: Style = Style { gras: false, corps: None, retour_a_la_ligne: false, centre: false, vertical: None, format: None };

    /// En gras, au corps dit (ou celui du classeur).
    pub fn gras(corps: Option<u32>) -> Style {
        Style { gras: true, corps, ..Style::CLASSEUR }
    }
}

/// Ce qu'une cellule porte.
#[derive(Clone, Debug, PartialEq)]
enum Valeur {
    Vide,
    Texte(String),
    Nombre(Decimal),
}

/// UNE FEUILLE, le seul contenu d'un classeur qu'on ecrit.
#[derive(Debug)]
pub struct Feuille {
    nom: String,
    cellules: BTreeMap<(u32, u16), (Valeur, Style)>,
    /// (premiere colonne, derniere colonne, largeur en caracteres).
    largeurs: Vec<(u16, u16, u32)>,
    /// La hauteur d'une ligne, en points.
    hauteurs: BTreeMap<u32, u32>,
    /// Les lignes du haut qui restent en place.
    figees: Option<u32>,
    fusions: Vec<(u32, u16, u32, u16)>,
}

impl Feuille {
    /// Une feuille vide, nommee.
    pub fn nouvelle(nom: &str) -> Feuille {
        Feuille { nom: nom.to_string(), cellules: BTreeMap::new(), largeurs: Vec::new(), hauteurs: BTreeMap::new(), figees: None, fusions: Vec::new() }
    }

    /// Un texte en (`ligne`, `colonne`), a partir de 0.
    pub fn texte(&mut self, ligne: u32, colonne: u16, texte: impl Into<String>, style: &Style) {
        self.cellules.insert((ligne, colonne), (Valeur::Texte(texte.into()), style.clone()));
    }

    /// Un nombre exact en (`ligne`, `colonne`).
    pub fn nombre(&mut self, ligne: u32, colonne: u16, nombre: Decimal, style: &Style) {
        self.cellules.insert((ligne, colonne), (Valeur::Nombre(nombre), style.clone()));
    }

    /// La largeur des colonnes `premiere` a `derniere`, en caracteres.
    pub fn largeur(&mut self, premiere: u16, derniere: u16, caracteres: u32) {
        self.largeurs.push((premiere, derniere, caracteres));
    }

    /// La hauteur de la ligne `ligne`, en points.
    pub fn hauteur(&mut self, ligne: u32, points: u32) {
        self.hauteurs.insert(ligne, points);
    }

    /// Les `lignes` du haut restent en place quand on fait defiler.
    pub fn figer(&mut self, lignes: u32) {
        self.figees = Some(lignes);
    }

    /// Fusionne les cellules de (`l0`, `c0`) a (`l1`, `c1`), dans le style dit ; un texte pose ensuite en (`l0`, `c0`)
    /// s'y ecrit.
    pub fn fusionner(&mut self, l0: u32, c0: u16, l1: u32, c1: u16, style: &Style) {
        for l in l0..=l1 {
            for c in c0..=c1 {
                self.cellules.entry((l, c)).or_insert((Valeur::Vide, style.clone()));
            }
        }
        self.fusions.push((l0, c0, l1, c1));
    }

    /// Les octets du classeur.
    pub fn octets(&self) -> R<Vec<u8>> {
        let mut styles: Vec<Style> = vec![Style::CLASSEUR];
        for (_, s) in self.cellules.values() {
            if !styles.contains(s) {
                styles.push(s.clone());
            }
        }
        let membres = [
            ("[Content_Types].xml", TYPES.to_string()),
            ("_rels/.rels", LIENS.to_string()),
            ("xl/workbook.xml", self.classeur()),
            ("xl/_rels/workbook.xml.rels", LIENS_CLASSEUR.to_string()),
            ("xl/styles.xml", feuille_de_styles(&styles)),
            ("xl/worksheets/sheet1.xml", self.feuille(&styles)),
        ];
        let disque = |e: &dyn std::fmt::Display| erreur(Classe::Disque, format!("classeur : {}", e));
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (nom, xml) in membres {
            z.start_file(nom, options).map_err(|e| disque(&e))?;
            z.write_all(xml.as_bytes()).map_err(|e| disque(&e))?;
        }
        Ok(z.finish().map_err(|e| disque(&e))?.into_inner())
    }

    fn classeur(&self) -> String {
        format!(
            "{}<workbook xmlns=\"{}\" xmlns:r=\"{}\"><sheets><sheet name=\"{}\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>",
            ENTETE_XML,
            NS_MAIN,
            NS_R,
            echappe(&self.nom)
        )
    }

    fn feuille(&self, styles: &[Style]) -> String {
        let mut x = format!("{}<worksheet xmlns=\"{}\" xmlns:r=\"{}\">", ENTETE_XML, NS_MAIN, NS_R);
        match self.figees {
            Some(n) if n > 0 => x.push_str(&format!(
                "<sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"{}\" topLeftCell=\"A{}\" activePane=\"bottomLeft\" state=\"frozen\"/><selection pane=\"bottomLeft\"/></sheetView></sheetViews>",
                n,
                n + 1
            )),
            _ => x.push_str("<sheetViews><sheetView workbookViewId=\"0\"/></sheetViews>"),
        }
        x.push_str("<sheetFormatPr defaultRowHeight=\"15\"/>");
        if !self.largeurs.is_empty() {
            x.push_str("<cols>");
            for (a, b, l) in &self.largeurs {
                // La largeur d'une colonne compte les marges de la cellule : celles de Calibri 11 font 5 pixels sur 7
                // par caractere, soit 182/256 de caractere en plus (la largeur affichee est `l`).
                x.push_str(&format!("<col min=\"{}\" max=\"{}\" width=\"{}.7109375\" customWidth=\"1\"/>", a + 1, b + 1, l));
            }
            x.push_str("</cols>");
        }
        x.push_str("<sheetData>");
        let mut lignes: BTreeMap<u32, Vec<(u16, &Valeur, &Style)>> = BTreeMap::new();
        for ((l, c), (v, s)) in &self.cellules {
            lignes.entry(*l).or_default().push((*c, v, s));
        }
        for l in self.hauteurs.keys() {
            lignes.entry(*l).or_default();
        }
        for (l, cellules) in lignes {
            match self.hauteurs.get(&l) {
                Some(h) => x.push_str(&format!("<row r=\"{}\" ht=\"{}\" customHeight=\"1\">", l + 1, h)),
                None => x.push_str(&format!("<row r=\"{}\">", l + 1)),
            }
            for (c, v, s) in cellules {
                let r = reference(l, c);
                let s = match styles.iter().position(|x| x == s) {
                    Some(i) => i,
                    None => unreachable!("chaque style d'une cellule est dans la feuille de styles"),
                };
                match v {
                    Valeur::Vide => x.push_str(&format!("<c r=\"{}\" s=\"{}\"/>", r, s)),
                    Valeur::Texte(t) => x.push_str(&format!("<c r=\"{}\" s=\"{}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>", r, s, echappe(t))),
                    Valeur::Nombre(d) => x.push_str(&format!("<c r=\"{}\" s=\"{}\"><v>{}</v></c>", r, s, d.normalize())),
                }
            }
            x.push_str("</row>");
        }
        x.push_str("</sheetData>");
        if !self.fusions.is_empty() {
            x.push_str(&format!("<mergeCells count=\"{}\">", self.fusions.len()));
            for (l0, c0, l1, c1) in &self.fusions {
                x.push_str(&format!("<mergeCell ref=\"{}:{}\"/>", reference(*l0, *c0), reference(*l1, *c1)));
            }
            x.push_str("</mergeCells>");
        }
        x.push_str("<pageMargins left=\"0.7\" right=\"0.7\" top=\"0.75\" bottom=\"0.75\" header=\"0.3\" footer=\"0.3\"/></worksheet>");
        x
    }
}

/// La feuille de styles : les polices, les formats de nombre et un format de cellule par style, dans l'ordre de
/// `styles` (le premier est celui du classeur).
fn feuille_de_styles(styles: &[Style]) -> String {
    let mut formats: Vec<&'static str> = Vec::new();
    let mut polices: Vec<(bool, u32)> = vec![(false, 11)];
    for s in styles {
        if let Some(f) = s.format {
            if !formats.contains(&f) {
                formats.push(f);
            }
        }
        let p = (s.gras, s.corps.unwrap_or(11));
        if !polices.contains(&p) {
            polices.push(p);
        }
    }
    let mut x = format!("{}<styleSheet xmlns=\"{}\">", ENTETE_XML, NS_MAIN);
    if !formats.is_empty() {
        x.push_str(&format!("<numFmts count=\"{}\">", formats.len()));
        for (i, f) in formats.iter().enumerate() {
            // Les formats propres au classeur se numerotent a partir de 164 (les precedents sont ceux d'Excel).
            x.push_str(&format!("<numFmt numFmtId=\"{}\" formatCode=\"{}\"/>", 164 + i, echappe(f)));
        }
        x.push_str("</numFmts>");
    }
    x.push_str(&format!("<fonts count=\"{}\">", polices.len()));
    for (gras, corps) in &polices {
        x.push_str(&format!("<font>{}<sz val=\"{}\"/><color theme=\"1\"/><name val=\"Calibri\"/><family val=\"2\"/><scheme val=\"minor\"/></font>", if *gras { "<b/>" } else { "" }, corps));
    }
    x.push_str("</fonts>");
    x.push_str("<fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills>");
    x.push_str("<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>");
    x.push_str("<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>");
    x.push_str(&format!("<cellXfs count=\"{}\">", styles.len()));
    for s in styles {
        let format = s.format.and_then(|f| formats.iter().position(|x| *x == f)).map_or(0, |i| 164 + i);
        let police = match polices.iter().position(|p| *p == (s.gras, s.corps.unwrap_or(11))) {
            Some(i) => i,
            None => unreachable!("chaque police d'un style est dans la liste des polices"),
        };
        let mut attributs = format!("numFmtId=\"{}\" fontId=\"{}\" fillId=\"0\" borderId=\"0\" xfId=\"0\"", format, police);
        if format != 0 {
            attributs.push_str(" applyNumberFormat=\"1\"");
        }
        if police != 0 {
            attributs.push_str(" applyFont=\"1\"");
        }
        let alignement = s.retour_a_la_ligne || s.centre || s.vertical.is_some();
        if alignement {
            let mut a = String::from("<alignment");
            if s.centre {
                a.push_str(" horizontal=\"center\"");
            }
            match s.vertical {
                Some(Vertical::Centre) => a.push_str(" vertical=\"center\""),
                Some(Vertical::Haut) => a.push_str(" vertical=\"top\""),
                None => {}
            }
            if s.retour_a_la_ligne {
                a.push_str(" wrapText=\"1\"");
            }
            a.push_str("/>");
            x.push_str(&format!("<xf {} applyAlignment=\"1\">{}</xf>", attributs, a));
        } else {
            x.push_str(&format!("<xf {}/>", attributs));
        }
    }
    x.push_str("</cellXfs>");
    x.push_str("<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles></styleSheet>");
    x
}

/// `B4` : la reference d'une cellule (ligne et colonne a partir de 0).
fn reference(ligne: u32, colonne: u16) -> String {
    let mut n = u32::from(colonne) + 1;
    let mut lettres = Vec::new();
    while n > 0 {
        let r = (n - 1) % 26;
        lettres.push(char::from(b'A' + r as u8));
        n = (n - 1) / 26;
    }
    lettres.iter().rev().collect::<String>() + &(ligne + 1).to_string()
}

/// Un texte dans du XML : `&`, `<`, `>` et `"` echappes.
fn echappe(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const ENTETE_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

const TYPES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/></Types>";

const LIENS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>";

const LIENS_CLASSEUR: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>";
