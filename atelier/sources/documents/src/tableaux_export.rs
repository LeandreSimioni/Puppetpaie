//! La machinerie commune des etats exportes : un tableau, six facons de l'ecrire --
//! CSV, texte tabule, XLSX, PDF, HTML, JSON --, et les ecrivains du livre de paie, qui a les siens.
//!
//! UN ETAT DECLARE, LA MACHINERIE ECRIT. Un etat rend un `Tableau` : ses colonnes (et lesquelles portent un
//! montant), ses sections (un groupe de lignes et son sous-total), un total general, ses avertissements et les
//! sources qui le fondent. Chaque ligne est un dictionnaire ordonne cle -> valeur : une ligne qui oublie une
//! colonne laisse une case vide. Une cle PRESENTE a `None` (la source ne dit pas ce montant) se lit « — » au PDF
//! et a la page HTML ; une cle ABSENTE reste vide.
//!
//! LES TOTAUX se calculent ici, avec la regle du livre de paie : un montant `None` ne compte pas pour zero, et
//! une colonne qu'aucune ligne ne declare a un total VIDE, pas 0,00.
//!
//! Le `.xlsx` s'ecrit par `classeur` : montants en nombres exacts, formats `# ##0.00 €`, largeurs 14 / 26,
//! volet fige sous l'en-tete. Il se prouve par son contenu, pas a l'octet. Le PDF est celui de la
//! fabrique (`fabrique_pdf`), decrit comme `tableaux.description_pdf`.

use droit::fiche::Lue as _;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fabrique_pdf::forme as pdf;
use fabrique_pdf::mesure::mesure;
use crate::classeur::{Feuille, Style, Vertical};
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use droit::contexte::Contexte;
use socle::echec::{erreur, Classe};
use socle::num::Decimal;
use socle::echec::Echec;
use socle::mois::Mois;
pub use socle::echec::R;

/// Un refus `Refuse`.
pub fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}

/// `FORMATS_TABLEAU` : les six formats de la machinerie, dans l'ordre du format.
pub const FORMATS_TABLEAU: [&str; 6] = ["csv", "tsv", "xlsx", "pdf", "html", "json"];

// ------------------------------------------------------------------ valeurs

/// Une valeur de cellule : un `Decimal` (il s'ecrit en texte), un entier (un compte, un rang), un mois (`AAAA-MM`), un
/// texte.
#[derive(Clone, Debug)]
pub enum Val {
    Dec(Decimal),
    Ent(i64),
    Mois(Mois),
    Txt(String),
}

impl Val {
    pub fn txt(s: impl Into<String>) -> Val {
        Val::Txt(s.into())
    }

    pub fn entier(n: i64) -> Val {
        Val::Ent(n)
    }

    /// La valeur en texte.
    pub fn en_texte_ecrit(&self) -> String {
        match self {
            Val::Dec(d) => d.to_string(),
            Val::Ent(n) => n.to_string(),
            Val::Mois(m) => m.to_string(),
            Val::Txt(s) => s.clone(),
        }
    }

    /// `Decimal(str(v))`.
    pub fn decimal(&self) -> R<Decimal> {
        match self {
            Val::Dec(d) => Ok(*d),
            Val::Ent(n) => Ok(Decimal::from(*n)),
            Val::Mois(m) => Err(erreur(Classe::CalculImpossible, socle::num::illisible(&m.to_string()))),
            Val::Txt(t) => socle::num::decimal_lu(t).ok_or_else(|| erreur(Classe::CalculImpossible, socle::num::illisible(t))),
        }
    }
}

impl Serialize for Val {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Val::Dec(d) => s.serialize_str(&d.to_string()),
            Val::Ent(n) => s.serialize_i64(*n),
            Val::Mois(m) => m.serialize(s),
            Val::Txt(t) => s.serialize_str(t),
        }
    }
}

/// Une ligne : un dictionnaire ordonne (l'ordre d'insertion compte pour le JSON). Une cle presente a `None` s'ecrit
/// `null` ; une cle absente ne s'ecrit pas.
#[derive(Clone, Debug, Default)]
pub struct Ligne {
    pub champs: Vec<(String, Option<Val>)>,
}

impl Serialize for Ligne {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.champs.len()))?;
        for (k, v) in &self.champs {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

impl Ligne {
    pub fn nouvelle() -> Ligne {
        Ligne::default()
    }

    /// `d[k] = v` : remplace en gardant la place, ou ajoute a la fin.
    pub fn poser(&mut self, k: &str, v: Option<Val>) -> &mut Ligne {
        match self.champs.iter_mut().find(|(c, _)| c == k) {
            Some(x) => x.1 = v,
            None => self.champs.push((k.to_string(), v)),
        }
        self
    }

    pub fn t(&mut self, k: &str, s: impl Into<String>) -> &mut Ligne {
        self.poser(k, Some(Val::Txt(s.into())))
    }

    /// Un texte, ou rien quand il est vide.
    pub fn ot(&mut self, k: &str, s: Option<impl Into<String>>) -> &mut Ligne {
        self.poser(k, s.map(|s| Val::Txt(s.into())))
    }

    pub fn m(&mut self, k: &str, m: Mois) -> &mut Ligne {
        self.poser(k, Some(Val::Mois(m)))
    }

    pub fn d(&mut self, k: &str, v: Decimal) -> &mut Ligne {
        self.poser(k, Some(Val::Dec(v)))
    }

    pub fn od(&mut self, k: &str, v: Option<Decimal>) -> &mut Ligne {
        self.poser(k, v.map(Val::Dec))
    }

    pub fn n(&mut self, k: &str, v: i64) -> &mut Ligne {
        self.poser(k, Some(Val::entier(v)))
    }

    pub fn rien(&mut self, k: &str) -> &mut Ligne {
        self.poser(k, None)
    }

    /// `d.get(k)` distinguant l'absente (`None`) de la presente a `None` (`Some(None)`).
    pub fn get(&self, k: &str) -> Option<&Option<Val>> {
        self.champs.iter().find(|(c, _)| c == k).map(|(_, v)| v)
    }

    /// La valeur, ou rien.
    pub fn val(&self, k: &str) -> Option<&Val> {
        self.get(k).and_then(|v| v.as_ref())
    }

    /// Le texte de la case `k`, tel qu'il s'ecrit : rien pour une case vide (absente ou `None`). C'est ce qui range et
    /// groupe les lignes.
    pub fn texte(&self, k: &str) -> String {
        self.val(k).map(Val::en_texte_ecrit).into_iter().collect()
    }

    pub fn decimal(&self, k: &str) -> Option<Decimal> {
        match self.val(k) {
            Some(Val::Dec(d)) => Some(*d),
            _ => None,
        }
    }

    /// Le mois de la case `k`, quand elle en porte un (`m`).
    pub fn mois(&self, k: &str) -> Option<Mois> {
        match self.val(k) {
            Some(Val::Mois(m)) => Some(*m),
            _ => None,
        }
    }
}

// ------------------------------------------------------------------ le tableau

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Genre {
    Montant,
    Nombre,
    Taux,
    Texte,
}

impl Genre {
    pub fn nom(self) -> &'static str {
        match self {
            Genre::Montant => "montant",
            Genre::Nombre => "nombre",
            Genre::Taux => "taux",
            Genre::Texte => "texte",
        }
    }
}

impl Serialize for Genre {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nom())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Colonne {
    pub cle: String,
    pub libelle: String,
    #[serde(rename = "type")]
    pub genre: Genre,
}

impl Colonne {
    pub fn totalisee(&self) -> bool {
        matches!(self.genre, Genre::Montant | Genre::Nombre)
    }

    pub fn a_droite(&self) -> bool {
        matches!(self.genre, Genre::Montant | Genre::Nombre | Genre::Taux)
    }
}

/// Une colonne de texte.
pub fn col_t(cle: &str, libelle: &str) -> Colonne {
    Colonne { cle: cle.into(), libelle: libelle.into(), genre: Genre::Texte }
}

/// Une colonne de montant.
pub fn col_m(cle: &str, libelle: &str) -> Colonne {
    Colonne { cle: cle.into(), libelle: libelle.into(), genre: Genre::Montant }
}

/// Une colonne de quantite.
pub fn col_n(cle: &str, libelle: &str) -> Colonne {
    Colonne { cle: cle.into(), libelle: libelle.into(), genre: Genre::Nombre }
}

/// Une colonne de taux (a droite, jamais totalisee).
pub fn col_x(cle: &str, libelle: &str) -> Colonne {
    Colonne { cle: cle.into(), libelle: libelle.into(), genre: Genre::Taux }
}

#[derive(Clone, Debug, Serialize)]
pub struct Section {
    pub libelle: String,
    pub lignes: Vec<Ligne>,
    pub total: Option<Ligne>,
}

impl Section {
    pub fn nouvelle(libelle: impl Into<String>, lignes: Vec<Ligne>) -> Section {
        Section { libelle: libelle.into(), lignes, total: None }
    }
}

/// UN TABLEAU, tel que la route le rend : son fichier ne s'ecrit que par son nom.
#[derive(Clone, Debug, Serialize)]
pub struct Tableau {
    pub etat: String,
    pub titre: String,
    pub periode: String,
    pub colonnes: Vec<Colonne>,
    pub sections: Vec<Section>,
    pub total: Option<Ligne>,
    pub avertissements: Vec<String>,
    pub sources: Vec<String>,
    pub mois: Vec<Mois>,
    pub format: String,
    #[serde(serialize_with = "nom_du_fichier")]
    pub fichier: Option<PathBuf>,
    pub sous_totaux: bool,
    pub avec_total: bool,
    pub donnees_personnelles: bool,
}

/// Le nom seul d'un fichier ecrit (`null` s'il n'est pas ecrit).
pub fn nom_du_fichier<S: Serializer>(f: &Option<PathBuf>, s: S) -> Result<S::Ok, S::Error> {
    match f.as_ref().and_then(|p| p.file_name()) {
        Some(n) => s.serialize_str(&n.to_string_lossy()),
        None => s.serialize_none(),
    }
}

impl Tableau {
    pub fn nouveau(etat: &str, titre: impl Into<String>, periode: impl Into<String>) -> Tableau {
        Tableau {
            etat: etat.to_string(),
            titre: titre.into(),
            periode: periode.into(),
            colonnes: Vec::new(),
            sections: Vec::new(),
            total: None,
            avertissements: Vec::new(),
            sources: Vec::new(),
            mois: Vec::new(),
            format: "csv".to_string(),
            fichier: None,
            sous_totaux: true,
            avec_total: true,
            donnees_personnelles: false,
        }
    }

}

/// Les lignes en sections, une par clé, les clés triées,
/// les lignes de chaque section triées par `tri` s'il y en a un (tri stable).
pub fn sections_de<K: Ord + Clone>(
    lignes: Vec<Ligne>, cle: impl Fn(&Ligne) -> K, libelle: impl Fn(&K) -> R<String>, tri: Option<&dyn Fn(&Ligne) -> Vec<String>>,
) -> R<Vec<Section>> {
    let mut paquets: BTreeMap<K, Vec<Ligne>> = BTreeMap::new();
    for l in lignes {
        paquets.entry(cle(&l)).or_default().push(l);
    }
    paquets
        .into_iter()
        .map(|(k, mut v)| {
            if let Some(t) = tri {
                v.sort_by_cached_key(|l| t(l));
            }
            Ok(Section { libelle: libelle(&k)?, lignes: v, total: None })
        })
        .collect()
}

/// Le libelle d'une section par mois (la case `mois` de ses lignes, `Ligne::mois`) : « Septembre 2026 ».
pub fn section_du_mois(m: &Option<Mois>) -> R<String> {
    m.map(libelle_mois).ok_or_else(sans_mois)
}

/// Le refus d'une ligne rangee par son mois qui n'en porte pas.
pub fn sans_mois() -> Echec {
    erreur(Classe::Illisible, "Une ligne de l'état n'a pas de mois.")
}

// ------------------------------------------------------------------ totaux

/// La somme de ce qui est DECLARE, ou `None` quand aucune ligne ne l'est.
fn somme(lignes: &[&Ligne], cle: &str) -> R<Option<Val>> {
    let mut total: Option<Decimal> = None;
    for l in lignes {
        let v = match l.get(cle) {
            Some(Some(Val::Txt(t))) if t.is_empty() => continue,
            Some(Some(v)) => v.decimal()?,
            _ => continue,
        };
        total = Some(match total {
            Some(t) => t + v,
            None => v,
        });
    }
    Ok(total.map(Val::Dec))
}

/// Pose le sous-total de chaque section et le total general. La premiere colonne de
/// texte porte le libelle (« Total ... »), les colonnes totalisees leur somme, les autres restent vides.
pub fn totaliser(t: &mut Tableau, libelle_total: &str) -> R<()> {
    let porte = t.colonnes.iter().find(|c| c.genre == Genre::Texte).map(|c| c.cle.clone());
    let ligne = |colonnes: &[Colonne], lignes: &[&Ligne], libelle: &str| -> R<Ligne> {
        let mut d = Ligne::nouvelle();
        for c in colonnes.iter().filter(|c| c.totalisee()) {
            d.poser(&c.cle, somme(lignes, &c.cle)?);
        }
        if let Some(p) = &porte {
            d.t(p, libelle);
        }
        Ok(d)
    };
    for s in t.sections.iter_mut() {
        s.total = if s.libelle.is_empty() {
            None
        } else {
            let l: Vec<&Ligne> = s.lignes.iter().collect();
            Some(ligne(&t.colonnes, &l, &format!("Total {}", s.libelle))?)
        };
    }
    t.total = if t.avec_total {
        let toutes: Vec<&Ligne> = t.sections.iter().flat_map(|s| s.lignes.iter()).collect();
        Some(ligne(&t.colonnes, &toutes, libelle_total)?)
    } else {
        None
    };
    if !t.avec_total {
        for s in t.sections.iter_mut() {
            s.total = None;
        }
    }
    Ok(())
}

/// Le libelle du total general : « TOTAL GÉNÉRAL ».
pub const TOTAL_GENERAL: &str = "TOTAL GÉNÉRAL";

// ------------------------------------------------------------ mise en forme

/// Une cellule en texte francais, virgule decimale ; une case vide ne s'ecrit pas (rien).
pub fn texte(v: Option<&Val>, c: &Colonne) -> String {
    v.map(|v| match v {
        Val::Dec(d) if c.a_droite() => d.to_string().replace('.', ","),
        autre => autre.en_texte_ecrit(),
    })
    .into_iter()
    .collect()
}

/// (cellules, genre) dans l'ordre d'ecriture : lignes, sous-totaux, total.
fn rangees(t: &Tableau) -> Vec<(&Ligne, &'static str)> {
    let plusieurs = t.sections.len() > 1 && t.sous_totaux;
    let mut out: Vec<(&Ligne, &'static str)> = Vec::new();
    for s in &t.sections {
        for l in &s.lignes {
            out.push((l, ""));
        }
        if plusieurs {
            if let Some(tot) = &s.total {
                out.push((tot, "total"));
            }
        }
    }
    if let Some(tot) = &t.total {
        out.push((tot, "general"));
    }
    out
}

/// `(texte, genre)` de chaque cellule d'une ligne, `—` pour un montant de ligne que la source ne dit pas.
fn cellule_lisible(l: &Ligne, c: &Colonne, genre: &str) -> String {
    let v = l.get(&c.cle);
    if matches!(v, Some(None)) && c.genre == Genre::Montant && genre.is_empty() {
        "—".to_string()
    } else {
        texte(v.and_then(|x| x.as_ref()), c)
    }
}

/// Le nom de la compagnie, s'il est dit.
pub fn nom_compagnie(ctx: &Contexte) -> Option<&str> {
    ctx.entreprise.nommee()
}

fn entete_texte(t: &Tableau, nom: Option<&str>) -> Vec<String> {
    let mut out = vec![format!("{} — {}", t.titre, t.periode)];
    if let Some(nom) = nom {
        out.insert(0, nom.to_string());
    }
    out
}

fn lire_nom(ctx: Option<&Contexte>) -> Option<&str> {
    ctx.and_then(nom_compagnie)
}

// ---------------------------------------------------------------------- csv

/// `csv_tableau` : point-virgule, virgule decimale, UTF-8 avec BOM.
pub fn csv_tableau(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let mut s = String::from("\u{feff}");
    let ligne = |s: &mut String, c: &[String]| s.push_str(&socle::csv::ligne(c, ';'));
    for x in entete_texte(t, lire_nom(ctx)) {
        ligne(&mut s, &[x]);
    }
    ligne(&mut s, &[]);
    ligne(&mut s, &t.colonnes.iter().map(|c| c.libelle.clone()).collect::<Vec<_>>());
    let mut precedent = "";
    for (d, genre) in rangees(t) {
        if genre == "general" && precedent == "total" {
            ligne(&mut s, &[]);
        }
        ligne(&mut s, &t.colonnes.iter().map(|c| texte(d.val(&c.cle), c)).collect::<Vec<_>>());
        if genre == "total" {
            ligne(&mut s, &[]);
        }
        precedent = genre;
    }
    for a in &t.avertissements {
        ligne(&mut s, &[]);
        ligne(&mut s, &[format!("! {}", a)]);
    }
    if !t.sources.is_empty() {
        ligne(&mut s, &[]);
        for src in &t.sources {
            ligne(&mut s, &[format!("Source : {}", src)]);
        }
    }
    Ok(s.into_bytes())
}

// ---------------------------------------------------------------------- tsv

/// `tsv_tableau` : les donnees seules. Une colonne « Groupe » en tete quand il y a plusieurs sections.
pub fn tsv_tableau(t: &Tableau, _ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let groupe = t.sections.len() > 1;
    let mut s = String::new();
    let mut entete: Vec<String> = if groupe { vec!["Groupe".to_string()] } else { Vec::new() };
    entete.extend(t.colonnes.iter().map(|c| c.libelle.clone()));
    s.push_str(&socle::csv::ligne(&entete, '\t'));
    for sec in &t.sections {
        for l in &sec.lignes {
            let mut cel: Vec<String> = if groupe { vec![sec.libelle.clone()] } else { Vec::new() };
            cel.extend(t.colonnes.iter().map(|c| texte(l.val(&c.cle), c)));
            s.push_str(&socle::csv::ligne(&cel, '\t'));
        }
    }
    Ok(s.into_bytes())
}

// --------------------------------------------------------------------- json

/// CE QUE LE FICHIER JSON D'UN TABLEAU PORTE : le fichier n'y est pas encore nomme quand il s'ecrit (`null`).
#[derive(Serialize)]
struct FichierTableau<'a> {
    etat: &'a str,
    titre: &'a str,
    periode: &'a str,
    mois: &'a [Mois],
    colonnes: &'a [Colonne],
    sections: &'a [Section],
    total: Option<&'a Ligne>,
    avertissements: &'a [String],
    sources: &'a [String],
    donnees_personnelles: bool,
    format: &'a str,
    fichier: Option<&'a str>,
}

/// Le tableau en JSON indente d'une espace.
pub fn json_tableau(t: &Tableau, _ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let f = FichierTableau {
        etat: &t.etat,
        titre: &t.titre,
        periode: &t.periode,
        mois: &t.mois,
        colonnes: &t.colonnes,
        sections: &t.sections,
        total: t.total.as_ref(),
        avertissements: &t.avertissements,
        sources: &t.sources,
        donnees_personnelles: t.donnees_personnelles,
        format: &t.format,
        fichier: None,
    };
    Ok(socle::ecriture::json_indente(&f, 1).into_bytes())
}

// --------------------------------------------------------------------- xlsx

/// Une cellule du classeur.
enum Cel {
    Vide,
    Nombre(Decimal),
    Texte(String),
}

/// Le nombre d'une colonne de chiffres (un texte qui se lit en nombre aussi), le texte sinon.
fn cellule_classeur(v: Option<&Val>, c: &Colonne) -> R<Cel> {
    let Some(v) = v else { return Ok(Cel::Vide) };
    Ok(match v {
        Val::Txt(s) if s.is_empty() => Cel::Vide,
        Val::Dec(d) => Cel::Nombre(*d),
        Val::Ent(n) => Cel::Nombre(Decimal::from(*n)),
        Val::Mois(m) => Cel::Texte(m.to_string()),
        Val::Txt(s) if c.a_droite() => match socle::num::decimal_lu(s) {
            Some(d) => Cel::Nombre(d),
            None => Cel::Texte(s.clone()),
        },
        Val::Txt(s) => Cel::Texte(s.clone()),
    })
}

fn ecrire_cellule(ws: &mut Feuille, ligne: u32, col: u16, cel: &Cel, style: &Style) {
    match cel {
        Cel::Vide => {}
        Cel::Nombre(d) => ws.nombre(ligne, col, *d, style),
        Cel::Texte(t) => ws.texte(ligne, col, t.clone(), style),
    }
}

/// Le nom de la feuille : celui de l'etat (`livre-de-paie`), qui tient dans les 31 caracteres qu'Excel admet et n'en
/// porte aucun qu'il refuse ; le titre entier est la premiere ligne de la feuille.
fn nom_de_feuille(t: &Tableau) -> R<&str> {
    if t.etat.is_empty() || t.etat.chars().count() > 31 || t.etat.chars().any(|c| "[]:*?/\\'".contains(c)) {
        return Err(refus(format!(
            "L'état « {} » ne peut pas nommer une feuille Excel : 31 caractères au plus, sans crochet, deux-points, astérisque, point d'interrogation, barre ni apostrophe. Le renommer dans le moteur.",
            t.etat
        )));
    }
    Ok(&t.etat)
}

/// `xlsx_tableau`.
pub fn xlsx_tableau(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let mut ws = Feuille::nouvelle(nom_de_feuille(t)?);
    let mut ligne: u32 = 0;
    for x in entete_texte(t, lire_nom(ctx)) {
        ws.texte(ligne, 0, x, &Style::gras(Some(13)));
        ligne += 1;
    }
    ligne += 1;
    let entete = Style { gras: true, retour_a_la_ligne: true, vertical: Some(Vertical::Centre), ..Style::default() };
    for (i, c) in t.colonnes.iter().enumerate() {
        ws.texte(ligne, i as u16, c.libelle.clone(), &entete);
    }
    let ligne_entete = ligne;
    ligne += 1;
    let nombre_de = |c: &Colonne| -> Option<&'static str> {
        match c.genre {
            Genre::Montant => Some("# ##0.00 \u{20ac}"),
            Genre::Nombre => Some("# ##0.##"),
            Genre::Taux => Some("0.000##"),
            Genre::Texte => None,
        }
    };
    for (d, genre) in rangees(t) {
        let (gras, corps) = match genre {
            "general" => (true, Some(12)),
            "total" => (true, Some(11)),
            _ => (false, None),
        };
        for (i, c) in t.colonnes.iter().enumerate() {
            let cel = cellule_classeur(d.val(&c.cle), c)?;
            let style = Style { gras, corps, format: if matches!(cel, Cel::Nombre(_)) { nombre_de(c) } else { None }, ..Style::default() };
            ecrire_cellule(&mut ws, ligne, i as u16, &cel, &style);
        }
        ligne += 1;
    }
    let simple = Style::default();
    for a in &t.avertissements {
        ligne += 1;
        ws.texte(ligne, 0, format!("! {}", a), &simple);
        ligne += 1;
    }
    for s in &t.sources {
        ws.texte(ligne, 0, format!("Source : {}", s), &simple);
        ligne += 1;
    }
    for (i, c) in t.colonnes.iter().enumerate() {
        ws.largeur(i as u16, i as u16, if c.a_droite() { 14 } else { 26 });
    }
    ws.figer(ligne_entete + 1);
    ws.octets()
}

// --------------------------------------------------------------- html / pdf

const CSS: &str = "\n* { font-family: Arial, sans-serif; }\nbody { font-size: 7pt; color: #2C3E50; }\nh1 { font-size: 11pt; color: #B22222; margin: 0 0 1pt 0;\n     text-transform: uppercase; letter-spacing: 0.5pt; }\n.sub { font-size: 6.8pt; color: #555; margin: 0 0 6pt 0; }\n.rs { font-size: 8.5pt; font-weight: bold; color: #2C3E50; margin: 0 0 1pt 0; }\ntable { width: 100%; border-collapse: collapse; }\nth { font-size: 6pt; color: #B22222; text-transform: uppercase;\n     letter-spacing: 0.3pt; text-align: left; padding: 0 3pt 2pt 0;\n     border-bottom: 0.7pt solid #B22222; }\ntd { font-size: 7pt; padding: 1.6pt 3pt 1.6pt 0; border-bottom: 0.3pt solid #eee; }\nth.n, td.n { text-align: right; }\ntr.total td { font-weight: bold; border-top: 0.5pt solid #999;\n              border-bottom: 0.5pt solid #999; }\ntr.general td { font-weight: bold; font-size: 8pt; color: #B22222; }\n.note { font-size: 6.3pt; color: #777; margin: 6pt 0 0 0; }\n.source { font-size: 6pt; color: #999; margin: 2pt 0 0 0; }\n";

/// `tableaux._e`.
fn echapper(v: &str) -> String {
    v.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `html_tableau`.
pub fn html_tableau(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let mut corps = String::new();
    for (d, genre) in rangees(t) {
        let mut cel = String::new();
        for c in &t.colonnes {
            let v = d.get(&c.cle);
            let s = if matches!(v, Some(None)) && c.genre == Genre::Montant && genre.is_empty() {
                "&mdash;".to_string()
            } else {
                echapper(&texte(v.and_then(|x| x.as_ref()), c))
            };
            cel.push_str(&format!("<td{}>{}</td>", if c.a_droite() { " class='n'" } else { "" }, s));
        }
        corps.push_str(&format!("<tr{}>{}</tr>", if genre.is_empty() { String::new() } else { format!(" class='{}'", genre) }, cel));
    }
    let th: String = t.colonnes.iter().map(|c| format!("<th{}>{}</th>", if c.a_droite() { " class='n'" } else { "" }, echapper(&c.libelle))).collect();
    let nom = lire_nom(ctx);
    let tete = format!(
        "{}<h1>{}</h1><p class='sub'>{}{}</p>",
        // Le nom de la compagnie, quand il est dit ; sinon la page n'en a pas la ligne.
        nom.map(|nom| format!("<p class='rs'>{}</p>", echapper(nom))).into_iter().collect::<String>(),
        echapper(&t.titre),
        echapper(&t.periode),
        if t.donnees_personnelles { " &mdash; porte des données personnelles" } else { "" }
    );
    let pied: String = t.avertissements.iter().map(|a| format!("<p class='note'>! {}</p>", echapper(a))).collect::<String>()
        + &t.sources.iter().map(|s| format!("<p class='source'>Source : {}</p>", echapper(s))).collect::<String>();
    let page = format!(
        "<!DOCTYPE html><html lang='fr'><head><meta charset='utf-8'><title>{} — {}</title><style>{} body{{font-size:10pt;margin:24px}}td,th{{font-size:9pt}}</style></head><body>{}<table><tr>{}</tr>{}</table>{}</body></html>",
        echapper(&t.titre),
        echapper(&t.periode),
        CSS,
        tete,
        th,
        corps,
        pied
    );
    Ok(page.into_bytes())
}

const PAGE_LARGEUR: pdf::Mesure = pdf::Mesure::milliemes(841_890);
const PAGE_HAUTEUR: pdf::Mesure = pdf::Mesure::milliemes(595_280);
const ROUGE: &str = "#B22222";
const ENCRE: &str = "#2C3E50";

const CHASSE: pdf::Mesure = mesure("3.9");
const CHASSE_ENTETE: pdf::Mesure = mesure("4.2");
const MARGE_CELLULE: pdf::Mesure = pdf::Mesure::entier(3);
/// La ligne de texte la plus longue, en caracteres, avant qu'elle raccourcisse.
const LIGNE_DE_TEXTE: i64 = 40;

/// La largeur de chaque colonne, en fraction de la largeur utile. Une longueur s'y compte en caracteres (gonfles d'un
/// dixieme pour un total, de trois pour le total general), une largeur en points.
pub fn fractions(entetes: &[(String, bool)], rangees: &[(Vec<String>, &str)], utile: pdf::Mesure) -> Vec<pdf::Mesure> {
    use pdf::Mesure;
    let n = entetes.len();
    let mut plus_long = vec![Mesure::ZERO; n];
    let mut plus_long_mot = vec![Mesure::ZERO; n];
    let mut mot_entete = vec![Mesure::ZERO; n];
    for (i, (libelle, _)) in entetes.iter().enumerate() {
        mot_entete[i] = Mesure::entier(libelle.split(socle::texte::blanc).map(|m| m.chars().count()).fold(0, usize::max) as i64);
    }
    let gonfle = |caracteres: usize, genre: &str| {
        let (n, d) = match genre {
            "total" => (11, 10),
            "general" => (13, 10),
            _ => (1, 1),
        };
        match Mesure::entier(caracteres as i64).fois_rapport(n, d) {
            Some(m) => m,
            None => unreachable!("un rapport de dix"),
        }
    };
    for (cellules, genre) in rangees {
        for (i, c) in cellules.iter().take(n).enumerate() {
            plus_long[i] = plus_long[i].max(gonfle(c.chars().count(), genre));
            for m in c.split(socle::texte::blanc).filter(|m| !m.is_empty()) {
                plus_long_mot[i] = plus_long_mot[i].max(gonfle(m.chars().count(), genre));
            }
        }
    }
    let texte: Vec<bool> = entetes.iter().map(|(_, a_droite)| !a_droite).collect();
    let besoins = |ligne: i64| -> Vec<Mesure> {
        (0..n)
            .map(|i| {
                let lg = if texte[i] { plus_long[i].min(Mesure::entier(ligne)).max(plus_long_mot[i]) } else { plus_long[i] };
                (lg * CHASSE).max(mot_entete[i] * CHASSE_ENTETE) + MARGE_CELLULE * 2
            })
            .collect()
    };
    let somme_de = |b: &[Mesure]| b.iter().sum::<Mesure>();
    // La ligne de texte la plus longue, en caracteres : elle raccourcit jusqu'a ce que les colonnes tiennent.
    let mut ligne = LIGNE_DE_TEXTE;
    let mut b = besoins(ligne);
    while somme_de(&b) > utile && ligne > 8 {
        ligne -= 2;
        b = besoins(ligne);
    }
    let total = somme_de(&b);
    b.iter()
        .map(|x| match x.sur(total) {
            Some(f) => f,
            None => unreachable!("chaque colonne a sa marge : le total n'est pas nul"),
        })
        .collect()
}

fn filets(couleur: &str, epaisseur: pdf::Mesure) -> Option<pdf::Filet> {
    Some(pdf::Filet { couleur: Some(couleur.to_string()), epaisseur: Some(epaisseur) })
}

/// Un paragraphe d'un seul morceau ; ce qui n'est pas dit ne s'ecrit pas.
fn paragraphe(texte: String, gras: bool, taille: pdf::Mesure, couleur: Option<&str>, majuscules: bool, avant: Option<pdf::Mesure>, apres: Option<pdf::Mesure>) -> pdf::Bloc {
    pdf::Bloc::Paragraphe(pdf::Paragraphe {
        morceaux: vec![pdf::Morceau { texte, gras: gras.then_some(true), italique: None, couleur: None }],
        taille: Some(taille),
        couleur: couleur.map(str::to_string),
        alignement: None,
        majuscules: majuscules.then_some(true),
        espace_avant: avant,
        espace_apres: apres,
        retrait: None,
    })
}

/// Une ligne de cellules en texte, sans rien d'autre.
fn ligne_de(cellules: Vec<String>) -> pdf::Ligne {
    pdf::Ligne {
        cellules: cellules.into_iter().map(pdf::Cellule::Texte).collect(),
        gras: None,
        fond: None,
        couleur: None,
        taille: None,
        filet_haut: None,
        filet_bas: None,
    }
}

/// La description (FORMAT.md) d'un etat -- A4 paysage, son titre, son tableau, ses
/// notes. `entetes` : (libelle, a droite) ; `rangees` : (cellules en texte, genre) avec `genre` dans « », « total »
/// ou « general ».
#[allow(clippy::too_many_arguments)]
pub fn description_pdf(
    titre: &str, periode: &str, nom: Option<&str>, sous_titre: &str, pied: &str, entetes: &[(String, bool)], rangees: &[(Vec<String>, &str)], notes: &[String],
    sources: &[String],
) -> pdf::Description {
    let fr = fractions(entetes, rangees, PAGE_LARGEUR - pdf::Mesure::entier(68));
    let colonnes = fr
        .iter()
        .zip(entetes)
        .map(|(f, e)| pdf::ColonneTableau { largeur: *f, alignement: e.1.then_some(pdf::Alignement::Droite) })
        .collect();
    let lignes = rangees
        .iter()
        .map(|(cellules, genre)| {
            let mut l = ligne_de(cellules.clone());
            if *genre == "total" {
                l.gras = Some(true);
                l.filet_haut = filets("#999999", mesure("0.5"));
                l.filet_bas = filets("#999999", mesure("0.5"));
            } else if *genre == "general" {
                l.gras = Some(true);
                l.taille = Some(pdf::Mesure::entier(8));
                l.couleur = Some(ROUGE.to_string());
                l.filet_haut = filets("#999999", mesure("0.5"));
                l.filet_bas = filets("#999999", mesure("0.5"));
            }
            l
        })
        .collect();
    let mut entete = ligne_de(entetes.iter().map(|e| e.0.clone()).collect());
    entete.gras = Some(true);
    entete.taille = Some(mesure("6.3"));
    entete.couleur = Some(ROUGE.to_string());
    entete.filet_bas = filets(ROUGE, mesure("0.7"));
    let mut blocs: Vec<pdf::Bloc> = Vec::new();
    let pt = pdf::Mesure::entier;
    if let Some(nom) = nom {
        blocs.push(paragraphe(nom.to_string(), true, mesure("8.5"), None, false, None, Some(pt(1))));
    }
    blocs.push(paragraphe(titre.to_string(), true, pt(11), Some(ROUGE), true, None, Some(pt(1))));
    blocs.push(paragraphe(sous_titre.to_string(), false, mesure("6.8"), Some("#555555"), false, None, Some(pt(6))));
    blocs.push(pdf::Bloc::Tableau(pdf::Tableau {
        colonnes,
        entete: Some(vec![entete]),
        lignes: Some(lignes),
        taille: Some(pt(7)),
        couleur: Some(ENCRE.to_string()),
        interligne: None,
        marge_cellule: Some(pdf::MargeCellule { haut: Some(mesure("1.6")), droite: Some(pt(3)), bas: Some(mesure("1.6")), gauche: Some(pt(3)) }),
        filet_lignes: filets("#EEEEEE", mesure("0.3")),
        filets_cellules: None,
        espace_avant: None,
        espace_apres: None,
    }));
    for a in notes {
        blocs.push(paragraphe(format!("! {}", a), false, mesure("6.3"), Some("#777777"), false, Some(pt(6)), None));
    }
    for src in sources {
        blocs.push(paragraphe(format!("Source : {}", src), false, pt(6), Some("#999999"), false, Some(pt(2)), None));
    }
    pdf::Description {
        format: crate::bulletin::FORMAT,
        page: Some(pdf::Page {
            largeur: Some(PAGE_LARGEUR),
            hauteur: Some(PAGE_HAUTEUR),
            marges: Some(pdf::Marges { haut: Some(pt(40)), droite: Some(pt(34)), bas: Some(pt(40)), gauche: Some(pt(34)) }),
        }),
        police: Some(pdf::Police { taille: Some(pt(7)), interligne: Some(mesure("1.25")), couleur: Some(ENCRE.to_string()) }),
        style: None,
        logo: None,
        titre: Some(format!("{} — {}", titre, periode)),
        documents: vec![pdf::Document::Blocs(blocs)],
        entete_page: Some(pdf::Repere {
            gauche: Some(format!("{} — {} (suite)", titre, periode)),
            centre: None,
            droite: None,
            taille: Some(mesure("8.5")),
            couleur: Some(ENCRE.to_string()),
            gras: Some(true),
            distance: Some(pt(24)),
            premiere_page: Some(false),
            image: None,
        }),
        pied_page: Some(pdf::Repere {
            gauche: Some(pied.to_string()),
            centre: None,
            droite: Some("page {page} sur {pages}".to_string()),
            taille: Some(mesure("6.5")),
            couleur: Some("#777777".to_string()),
            gras: None,
            distance: Some(pt(20)),
            premiere_page: None,
            image: None,
        }),
    }
}

/// `pdf_fabrique` : les octets du PDF mis en page par la fabrique. Un refus de la fabrique est `Refuse`.
pub fn pdf_fabrique(description: &pdf::Description) -> R<Vec<u8>> {
    fabrique_pdf::fabriquer(description).map_err(|r| crate::fabrique::echec(r).reformuler(|p| format!("PDF impossible : la fabrique de PDF refuse cet état ({}). Aucun PDF n'est fait autrement.", p)))
}

/// `pdf_tableau` : A4 paysage, l'en-tete des colonnes a chaque page, un pied numerote.
pub fn pdf_tableau(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let sous = format!("{}{}", t.periode, if t.donnees_personnelles { " — porte des données personnelles" } else { "" });
    let rangs: Vec<(Vec<String>, &str)> = rangees(t).into_iter().map(|(d, genre)| (t.colonnes.iter().map(|c| cellule_lisible(d, c, genre)).collect(), genre)).collect();
    let entetes: Vec<(String, bool)> = t.colonnes.iter().map(|c| (c.libelle.clone(), c.a_droite())).collect();
    let d = description_pdf(&t.titre, &t.periode, lire_nom(ctx), &sous, &format!("{} — {}", t.titre, t.periode), &entetes, &rangs, &t.avertissements, &t.sources);
    pdf_fabrique(&d)
}

// ------------------------------------------------------------------ l'ecriture

/// Les octets d'un tableau dans son format.
pub fn octets(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    match t.format.as_str() {
        "csv" => csv_tableau(t, ctx),
        "tsv" => tsv_tableau(t, ctx),
        "xlsx" => xlsx_tableau(t, ctx),
        "pdf" => pdf_tableau(t, ctx),
        "html" => html_tableau(t, ctx),
        "json" => json_tableau(t, ctx),
        autre => Err(refus(format!("Format inconnu pour cet état : « {} ».", autre))),
    }
}

/// Le format demande, s'il est l'un de `formats`. (Le XLSX et le PDF sont toujours
/// disponibles : ni openpyxl ni la fabrique externe ne sont plus en jeu.)
pub fn format_possible(fmt: &str, formats: &[&str]) -> R<String> {
    if !formats.contains(&fmt) {
        return Err(refus(format!("Format inconnu pour cet état : « {} ». Les formats possibles sont {}.", fmt, formats.join(", "))));
    }
    Ok(fmt.to_string())
}

/// Ecrit le tableau dans `dossier/nom.<ext>` (`tsv` s'ecrit `.txt`) et rend le chemin.
pub fn ecrire(t: &mut Tableau, dossier: &Path, nom: &str, ctx: &Contexte) -> R<PathBuf> {
    std::fs::create_dir_all(dossier).map_err(|e| socle::echec::du_disque(&dossier, &e).reclasser(Classe::Disque))?;
    let ext = if t.format == "tsv" { "txt" } else { t.format.as_str() };
    let chemin = dossier.join(format!("{}.{}", nom, ext));
    let contenu = octets(t, Some(ctx))?;
    std::fs::write(&chemin, contenu).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque))?;
    t.fichier = Some(chemin.clone());
    Ok(chemin)
}

// ------------------------------------------------------------------ petits outils

/// « Septembre 2026 » (le nom en titre).
pub fn libelle_mois(m: Mois) -> String {
    m.libelle(socle::mois::Graphie::Titre)
}

/// AAAA-MM-JJ -> JJ/MM/AAAA ; le reste tel quel.
pub fn jj(iso: &str) -> String {
    let c: Vec<char> = iso.chars().collect();
    if c.len() >= 10 && c[4] == '-' {
        format!("{}/{}/{}", c[8..10].iter().collect::<String>(), c[5..7].iter().collect::<String>(), c[..4].iter().collect::<String>())
    } else {
        iso.to_string()
    }
}

/// « NOM Prenom ».
pub fn qui(fiche: &droit::fiche::ChampsSalarie) -> R<String> {
    if fiche.nom.is_none() {
        return Err(socle::fichiers::absent("nom").reclasser(Classe::Manque));
    }
    Ok(qui_souple(fiche))
}

/// Le « qui » d'une fiche qui n'a peut-etre pas de nom : le nom et le prenom (`prenom_affichage`, sinon `prenom`) qu'elle
/// dit.
pub fn qui_souple(fiche: &droit::fiche::ChampsSalarie) -> String {
    [fiche.nom.portee(), fiche.prenom_affichage.portee().or(fiche.prenom.portee())].into_iter().flatten().collect::<Vec<_>>().join(" ")
}

/// Un montant ecrit en texte, a deux decimales ; un texte qui n'est pas un nombre se refuse.
pub fn montant(t: &str) -> R<Decimal> {
    let d = socle::num::decimal_lu(t).ok_or_else(|| erreur(Classe::CalculImpossible, socle::num::illisible(t)))?;
    Ok(socle::num::eur2(d))
}
