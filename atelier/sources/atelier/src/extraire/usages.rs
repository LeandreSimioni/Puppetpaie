//! `moteur-atelier extraire usages <cahier.pdf | tableau-des-usages.xlsx> <sortie.json>` : le tableau des usages de la norme DSN,
//! pour `logiciel/donnees/usages-dsn-base.json` (P26V01) et `usages-dsn-p27v01-base.json` (P27V01), que lit `usages.rs`.
//!
//! Pour chaque rubrique et chaque nature de declaration : obligatoire (O), conditionnelle (C), interdite (I), sans objet
//! (N). Deux sources, selon l'extension, toutes deux deposees dans `atelier/reference/norme-dsn/` (son `LISEZMOI.md`) :
//!
//! - LE CLASSEUR (depuis la norme 2027.1.2, fiche 3358 de la base de connaissances net-entreprises :
//!   `dsn-tableau-des-usages-<version>-<AAAAMMJJ>.xlsx`) : la feuille « 1 - Tableau des usages », l'entete des natures sur
//!   sa ligne (« 01 - DSN mensuelle »...), une ligne par rubrique (colonne C). Les libelles sont ramenes a ce que le releve
//!   sur PDF rend : blancs ramenes a un, sans blanc final, en forme composee (NFC ; le classeur 2027.1.2 ecrit
//!   S21.G00.72.001 en accents decomposes).
//!
//! - LE CAHIER TECHNIQUE EN PDF (`dsn-cahier-technique-<version>.pdf`, jusqu'a la norme 2026.1), lu par la fabrique de PDF
//!   (`fabrique_pdf::lecture::texte` pour choisir les pages, `fabrique_pdf::inspection::inspecter` pour les mots, leurs
//!   rectangles et les filets du tableau), comme tout PDF du logiciel. Les pages du tableau sont tournees de 90 degres :
//!   l'inspection rend les rectangles dans le repere de la page NON tournee, on les remet donc d'abord a l'endroit. Puis :
//!   - les COLONNES sont les filets verticaux (traits fins, longs), regroupes par abscisse : une abscisse n'est une
//!     frontiere de colonne que si ses filets, mis bout a bout, couvrent une grande part du tableau ;
//!   - les LIGNES sont les filets horizontaux, regroupes par ordonnee de la meme facon ; une ligne de rubrique peut etre
//!     haute de plusieurs lignes de texte (un libelle long) ;
//!   - une CELLULE est le texte des mots dont le centre tombe dans la case, dans l'ordre du flux du PDF, separes par une
//!     espace.
//!   Une page dont le tableau a moins de huit colonnes n'est pas un tableau d'usages. Dans le cahier 2026.1, le tableau
//!   des usages par rubrique occupe les pages 363 a 371 ; les pages 372 a 382 portent un autre tableau, celui des usages
//!   des CONTROLES (« Contrôle », « Oui »/« Non » : p. 362, « Tableau des usages de contrôles : explications »), dont aucune
//!   ligne n'a la forme d'une ligne de rubrique : la `note` dit les pages ou une rubrique a ete lue.
//!
//! Le PDF se lit une fois, a son depot : sa transcription (`transcrire`, `super::Transcription`) redonne le meme releve.

use std::collections::HashMap;
use std::path::Path;

use regex::Regex;
use serde::Serialize;

use super::{case, date_du_releve, en_transcription, nom_de, propre, remplie, sha256, EnOrdre, Rang, Releve, Transcription};
use fabrique_pdf::mesure::{mesure, Mesure};
use import::releve::PageRelevee;
use socle::date::Jour;
use socle::lecture_xlsx;
use socle::echec::{refus, R};

const QUOI: &str = "Le tableau des usages du cahier technique NEODeS : pour chaque rubrique et chaque nature de declaration, si \
                    elle est obligatoire (O), conditionnelle (C), interdite (I) ou sans objet (N). Ecrire une rubrique que la \
                    nature n'admet pas est une anomalie BLOQUANTE (CST-04) et le fichier ne declare rien.";

/// Le referentiel des usages, apres `quoi` et `note`.
#[derive(Serialize)]
struct Usages {
    #[serde(flatten)]
    empreinte: Empreinte,
    natures: EnOrdre<String>,
    valeurs: Valeurs,
    usages: EnOrdre<Usage>,
}

/// L'empreinte du texte depose lu : le classeur, ou le cahier en PDF.
#[derive(Serialize)]
enum Empreinte {
    #[serde(rename = "sha256_du_classeur")]
    Classeur(String),
    #[serde(rename = "sha256_du_pdf")]
    Pdf(String),
}

/// Ce que dit chaque lettre du tableau.
#[derive(Serialize)]
struct Valeurs {
    #[serde(rename = "O")]
    o: &'static str,
    #[serde(rename = "C")]
    c: &'static str,
    #[serde(rename = "I")]
    i: &'static str,
    #[serde(rename = "N")]
    n: &'static str,
}

/// L'usage d'une rubrique : son libelle, puis sa lettre pour chaque nature, dans l'ordre de l'entete.
#[derive(Serialize)]
struct Usage {
    libelle: String,
    #[serde(flatten)]
    par_nature: EnOrdre<String>,
}

/// La feuille du classeur des usages (fiche 3358).
pub const FEUILLE_USAGES: &str = "1 - Tableau des usages";

/// Un filet est un trait de moins de 1,2 pt d'epaisseur et d'au moins 20 pt de long.
const EPAISSEUR_MAX: Mesure = mesure("1.2");
const LONGUEUR_MIN: Mesure = Mesure::entier(20);
/// Deux filets dont les centres different de moins de 1 pt sont le meme.
const TOLERANCE: Mesure = Mesure::UN;
/// Une frontiere de colonne : ses filets couvrent au moins 40 % de ceux de la colonne la mieux tracee ; une frontiere de
/// ligne : au moins 150 pt de filet (les sept colonnes de natures, a elles seules, en font 170).
const PART_COLONNE: Mesure = mesure("0.4");
const COUVERTURE_LIGNE: Mesure = Mesure::entier(150);

fn rubrique() -> Regex {
    Regex::new(r"\AS\d\d\.G00\.\d\d\.\d\d\d\z").expect("motif")
}

/// Le rectangle d'une page tournee de /Rotate, dans le repere de la page telle qu'on la lit (origine en haut a gauche).
fn a_l_endroit(page: &PageRelevee, b: [Mesure; 4]) -> [Mesure; 4] {
    let (largeur, hauteur) = (page.largeur, page.hauteur);
    let [x0, y0, x1, y1] = b;
    match page.rotation.rem_euclid(360) {
        90 => [largeur - y1, x0, largeur - y0, x1],
        180 => [largeur - x1, hauteur - y1, largeur - x0, hauteur - y0],
        270 => [y0, hauteur - x1, y1, hauteur - x0],
        _ => [x0, y0, x1, y1],
    }
}

/// [(centre, longueur)] -> [(centre moyen, longueur cumulee)] : les filets de meme position sont mis bout a bout.
fn regrouper(mut filets: Vec<(Mesure, Mesure)>) -> Vec<(Mesure, Mesure)> {
    filets.sort();
    let mut groupes: Vec<Vec<(Mesure, Mesure)>> = Vec::new();
    for (centre, longueur) in filets {
        match groupes.last_mut() {
            Some(g) if centre - g[g.len() - 1].0 <= TOLERANCE => g.push((centre, longueur)),
            _ => groupes.push(vec![(centre, longueur)]),
        }
    }
    groupes
        .iter()
        .map(|g| {
            (g.iter().map(|x| x.0).sum::<Mesure>() / g.len() as i64, g.iter().map(|x| x.1).sum::<Mesure>())
        })
        .collect()
}

/// (frontieres de colonnes, frontieres de lignes) du tableau de la page, a l'endroit ; deux listes vides si la page n'a pas
/// de filets.
fn cases(page: &PageRelevee) -> (Vec<Mesure>, Vec<Mesure>) {
    let (mut verticaux, mut horizontaux) = (Vec::new(), Vec::new());
    for t in &page.traits {
        let [x0, y0, x1, y1] = a_l_endroit(page, *t);
        if x1 - x0 <= EPAISSEUR_MAX && y1 - y0 >= LONGUEUR_MIN {
            verticaux.push(((x0 + x1) / 2, y1 - y0));
        } else if y1 - y0 <= EPAISSEUR_MAX && x1 - x0 >= LONGUEUR_MIN {
            horizontaux.push(((y0 + y1) / 2, x1 - x0));
        }
    }
    let colonnes = regrouper(verticaux);
    if colonnes.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let seuil = PART_COLONNE * colonnes.iter().map(|c| c.1).fold(Mesure::MIN, Mesure::max);
    (
        colonnes.iter().filter(|c| c.1 >= seuil).map(|c| c.0).collect(),
        regrouper(horizontaux).iter().filter(|c| c.1 >= COUVERTURE_LIGNE).map(|c| c.0).collect(),
    )
}

/// L'indice de la case de `bornes` qui contient `valeur`.
fn place(valeur: Mesure, bornes: &[Mesure]) -> Option<usize> {
    (0..bornes.len().saturating_sub(1)).find(|&i| bornes[i] <= valeur && valeur < bornes[i + 1])
}

/// Les lignes du tableau de la page : une liste de cellules (texte, blancs ramenes a un ; `None` pour une case sans mot),
/// de haut en bas ; rien s'il n'y a pas de tableau.
fn lignes_du_tableau(page: &PageRelevee) -> Vec<Rang> {
    let (colonnes, lignes) = cases(page);
    if colonnes.len() < 9 || lignes.len() < 2 {
        return Vec::new();
    }
    let mut cellules: HashMap<(usize, usize), Vec<&str>> = HashMap::new();
    for mot in &page.mots {
        let [x0, y0, x1, y1] = a_l_endroit(page, mot.rect.connue());
        if let (Some(i), Some(j)) = (place((y0 + y1) / 2, &lignes), place((x0 + x1) / 2, &colonnes)) {
            cellules.entry((i, j)).or_default().push(&mot.texte);
        }
    }
    (0..lignes.len() - 1).map(|i| (0..colonnes.len() - 1).map(|j| cellules.get(&(i, j)).map(|m| m.join(" "))).collect()).collect()
}

/// L'entete du tableau : sa colonne E est « 01 - DSN... ».
fn est_l_entete(r: &[Option<String>]) -> bool {
    case(r, 4).is_some_and(|t| t.starts_with("01 - DSN"))
}

/// Une ligne de rubrique : sa colonne C est un identifiant de rubrique.
fn est_une_rubrique(r: &[Option<String>], motif: &Regex) -> bool {
    case(r, 2).is_some_and(|t| motif.is_match(t))
}

/// Les 400 premiers caracteres d'une page (`texte[:400]`).
fn debut(texte: &str) -> &str {
    match texte.char_indices().nth(400) {
        Some((i, _)) => &texte[..i],
        None => texte,
    }
}

/// (entete, lignes de rubriques, pages ou une rubrique a ete lue) du tableau des usages du cahier en PDF.
fn releve_pdf(textes: &[String], pages: &[PageRelevee]) -> R<(Rang, Vec<Rang>, Vec<usize>)> {
    let motif = rubrique();
    let (mut entete, mut lignes, mut lues) = (None, Vec::new(), Vec::new());
    for (numero, (texte, page)) in (1..).zip(textes.iter().zip(pages)) {
        let d = debut(texte);
        if !d.contains("Tableau") && !d.contains("01 - DSN mensuelle") {
            continue;
        }
        for r in lignes_du_tableau(page) {
            if est_l_entete(&r) {
                entete = Some(r);
            } else if est_une_rubrique(&r, &motif) {
                lignes.push(r);
                if !lues.contains(&numero) {
                    lues.push(numero);
                }
            }
        }
    }
    Ok((entete.ok_or_else(|| refus("entete du tableau introuvable"))?, lignes, lues))
}

/// (entete, lignes de rubriques) lues dans le classeur des usages : l'entete est la ligne dont la colonne E est
/// « 01 - DSN... », une ligne de rubrique celle dont la colonne C est un identifiant de rubrique.
fn releve_xlsx(source: &Path) -> R<(Rang, Vec<Rang>)> {
    let motif = rubrique();
    let (mut entete, mut lignes) = (None, Vec::new());
    for r in lecture_xlsx::lignes(source, FEUILLE_USAGES)? {
        let r: Rang = r.iter().map(propre).collect();
        if est_l_entete(&r) {
            entete = Some(r);
        } else if est_une_rubrique(&r, &motif) {
            lignes.push(r);
        }
    }
    let entete = entete.ok_or_else(|| refus!("entete du tableau introuvable dans la feuille {}", FEUILLE_USAGES))?;
    Ok((entete, lignes))
}

/// Le tableau des usages du cahier en PDF, lu : (entete, lignes de rubriques, pages ou une rubrique a ete lue).
fn lire_le_cahier(source: &Path) -> R<(Rang, Vec<Rang>, (usize, usize))> {
    let chemin = source.to_string_lossy();
    let (pages, _) = fabrique_pdf::lecture::texte(&chemin).map_err(documents::fabrique::echec)?;
    let textes = pages.iter().map(|p| p.lu().map_err(documents::fabrique::echec)).collect::<R<Vec<String>>>()?;
    let releve = import::releve::du_pdf(source).map_err(|r| r.en_echec().reformuler(|p| format!("{} : {}", chemin, p)))?;
    let (entete, lignes, pages) = releve_pdf(&textes, &releve.releve.pages)?;
    match (pages.iter().min(), pages.iter().max()) {
        (Some(premiere), Some(derniere)) => Ok((entete, lignes, (*premiere, *derniere))),
        _ => Err(refus!("{} : aucune ligne de rubrique dans le tableau des usages", chemin)),
    }
}

/// LA TRANSCRIPTION DU TABLEAU DES USAGES du cahier en PDF (`super::Transcription`) : une ligne par ligne du tableau, ses
/// cellules separees par une tabulation (une case vide n'y ecrit rien), l'entete d'abord ; l'en-tete dit les pages ou
/// une rubrique a ete lue.
pub fn transcrire(pdf: &Path) -> R<String> {
    version_du_cahier(pdf)?;
    let (entete, lignes, (premiere, derniere)) = lire_le_cahier(pdf)?;
    let mut corps = String::new();
    for r in std::iter::once(&entete).chain(&lignes) {
        for (j, c) in r.iter().enumerate() {
            if j > 0 {
                corps.push('\t');
            }
            if let Some(c) = c {
                corps.push_str(c);
            }
        }
        corps.push('\n');
    }
    let t = Transcription::de(pdf, vec![("pages".into(), format!("{}-{}", premiere, derniere))], corps)?;
    Ok(t.texte(&format!("Le tableau des usages de {}, cellule par cellule (`moteur extraire transcrire`)", nom_de(pdf))))
}

/// (entete, lignes de rubriques, pages) relus dans une transcription : une case sans texte est vide.
fn lire_la_transcription(t: &Transcription) -> R<(Rang, Vec<Rang>, (String, String))> {
    let mut rangs = t.corps.lines().map(|l| l.split('\t').map(|c| Some(c.to_string()).filter(|c| !c.is_empty())).collect::<Rang>());
    let entete = rangs.next().ok_or_else(|| refus("transcription vide"))?;
    let pages = t.champ("pages").and_then(|p| p.split_once('-')).ok_or_else(|| refus("la transcription ne dit pas ses pages"))?;
    Ok((entete, rangs.collect(), (pages.0.to_string(), pages.1.to_string())))
}

/// La version, lue dans le nom du fichier depose (le releve ne la devine pas).
fn version(source: &Path, motif: &str, attendu: &str) -> R<String> {
    let chemin = source.to_string_lossy();
    Regex::new(motif)
        .expect("motif")
        .captures(&chemin)
        .map(|c| c[1].to_string())
        .ok_or_else(|| refus!("version illisible dans le nom du fichier : {} (attendu {})", chemin, attendu))
}

pub(crate) fn version_du_cahier(source: &Path) -> R<String> {
    version(source, r"cahier-technique-(\d[\d.]*\d)\.pdf$", "dsn-cahier-technique-<version>.pdf")
}

pub(crate) fn en_classeur(source: &Path) -> bool {
    source.to_string_lossy().to_lowercase().ends_with(".xlsx")
}

/// Le releve du tableau des usages, depuis le classeur, le cahier en PDF, ou la transcription du cahier
/// (`dsn-cahier-technique-<version>-usages.txt`), qui en redonne le meme releve.
pub fn relever(source: &Path, jour: &Jour) -> R<Releve> {
    let date = date_du_releve(jour);
    let classeur = en_classeur(source);
    let provenance_du_cahier = |pdf: &str, premiere: &str, derniere: &str| -> R<String> {
        Ok(format!(
            "Cahier technique NEODeS {} ({}), tableau des usages (p. {}-{}). Releve automatiquement par \
             `moteur extraire usages` le {}.",
            version_du_cahier(Path::new(pdf))?,
            pdf,
            premiere,
            derniere,
            date
        ))
    };
    let (entete, lignes, provenance, empreinte) = if classeur {
        let (entete, lignes) = releve_xlsx(source)?;
        let v = version(
            source,
            r"tableau-des-usages-(\d[\d.]*\d)-\d{8}\.xlsx$",
            "dsn-tableau-des-usages-<version>-<AAAAMMJJ>.xlsx",
        )?;
        let provenance = format!(
            "Tableau des usages de la norme DSN {}, classeur {} (feuille « {} »). Releve automatiquement par \
             `moteur extraire usages` le {}.",
            v,
            nom_de(source),
            FEUILLE_USAGES,
            date
        );
        (entete, lignes, provenance, sha256(source)?)
    } else if en_transcription(source) {
        let t = Transcription::lire(source)?;
        let (entete, lignes, (premiere, derniere)) = lire_la_transcription(&t)?;
        (entete, lignes, provenance_du_cahier(&t.pdf, &premiere, &derniere)?, t.sha256)
    } else {
        version_du_cahier(source)?;
        let (entete, lignes, (premiere, derniere)) = lire_le_cahier(source)?;
        let provenance = provenance_du_cahier(&nom_de(source), &premiere.to_string(), &derniere.to_string())?;
        (entete, lignes, provenance, sha256(source)?)
    };
    let nature = Regex::new(r"\A(\d\d) - (.*)").expect("motif");
    let mut natures: Vec<(String, usize, String)> = Vec::new();
    for (j, titre) in entete.iter().enumerate().skip(4) {
        if let Some(m) = titre.as_deref().and_then(|t| nature.captures(t)) {
            let (n, libelle) = (m[1].to_string(), m[2].to_string());
            match natures.iter_mut().find(|x| x.0 == n) {
                Some(x) => *x = (n, j, libelle),
                None => natures.push((n, j, libelle)),
            }
        }
    }
    let mut usages: EnOrdre<Usage> = EnOrdre::new();
    for r in &lignes {
        let rubrique = remplie(r, 2, "une ligne de rubrique")?;
        let libelle = remplie(r, 3, rubrique)?;
        let mut par_nature = EnOrdre::new();
        for (n, colonne, _) in &natures {
            par_nature.poser(n.clone(), remplie(r, *colonne, &format!("{} (nature {})", rubrique, n))?.to_string());
        }
        usages.poser(rubrique.to_string(), Usage { libelle: libelle.to_string(), par_nature });
    }
    let libelles = EnOrdre(natures.iter().map(|(n, _, l)| (n.clone(), l.clone())).collect());
    let (n_usages, n_natures) = (usages.len(), natures.len());
    let note = format!(
        "{} Le releve est confronte a ce que le moteur produit : toute declaration qu'il ecrit (DSN, signalement, FCTU) \
           passe `declarations::usages::verifier_fichier`, qui refuse une rubrique que sa nature n'admet pas, et \
           `declarations/tests/declarations/usages_formats_ntt.rs` fige le tableau des usages par nature.",
        provenance
    );
    let corps = Usages {
        empreinte: if classeur { Empreinte::Classeur(empreinte) } else { Empreinte::Pdf(empreinte) },
        natures: libelles,
        valeurs: Valeurs { o: "obligatoire", c: "conditionnelle", i: "interdite", n: "sans objet / non prevue" },
        usages,
    };
    Ok(Releve::nouveau(QUOI, note, corps, 1, false, vec![format!("{} rubriques, {} natures", n_usages, n_natures)]))
}
