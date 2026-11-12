//! `moteur-atelier extraire formats <cahier.pdf | categories-controles-datatypes.xlsx> <sortie.json>` : le FORMAT de chaque rubrique
//! de la norme DSN, pour `logiciel/donnees/formats-dsn-base.json` (P26V01) et `formats-dsn-p27v01-base.json` (P27V01),
//! que lit `formats.rs`.
//!
//! Le tableau des usages (`extraire::usages`) dit si une rubrique a le droit d'etre la. Il ne dit pas ce qu'elle doit
//! CONTENIR. Or chaque rubrique du cahier technique porte, en face de sa definition, sa longueur `[min,max]` et une
//! expression reguliere de controle nommee CSL -- par exemple, pour un montant, `-?(([0-9]){1,12})\.[0-9]{2}`, et pour
//! une date `[0-9]{8}`. Un montant d'indemnite de rupture ecrit dans `S21.G00.52.004`, qui est
//! une DATE, passe les usages : les quatre rubriques du bloc 52 ont le droit d'etre la, c'est leur CONTENU qui est faux. Une
//! expression de controle le dit tout de suite.
//!
//! Deux sources, selon l'extension, deposees dans `atelier/reference/norme-dsn/` (son `LISEZMOI.md`) :
//!
//! - LE CLASSEUR (depuis la norme 2027.1.2, fiche 3357 :
//!   `dsn-tableaux-categories-controles-datatypes-usages-<version>-<AAAAMMJJ>.xlsx`). Feuille « Fields », le type de
//!   donnees de chaque rubrique (colonne « DataType Id ») ; feuille « Data Types », pour chaque type son expression
//!   (« Regexp », la CSL 00 du cahier) et sa longueur (« Lg Min », « Lg Max »). Le releve recompose le meme JSON :
//!   longueur [Lg Min, Lg Max] quand le type en porte une, `csl` = [Regexp] quand il en porte une. Une rubrique que la
//!   feuille « Fields » type d'un type inconnu de « Data Types » n'a pas de format (aucun controle) : le releve la nomme
//!   (`types_inconnus`), il ne la reprend pas d'ailleurs. Preuve de la methode : sur la norme 2027, le classeur 2027.1.2
//!   redonne le releve du PDF 2027.1.1 rubrique pour rubrique, a l'exception des deux CSL que le journal de maintenance
//!   2027.1.2 change (evolutions 15 et 18 : S21.G00.40.070, S21.G00.41.045) et de S89.G00.32.014, absente de « Fields ».
//!
//! - LE CAHIER TECHNIQUE EN PDF (`dsn-cahier-technique-<version>.pdf`), dont le texte est lu par la fabrique de PDF
//!   (`fabrique_pdf::lecture::texte`) : chaque identifiant de rubrique seul sur sa ligne ouvre un corps, qui va jusqu'au
//!   suivant ; on y lit la premiere longueur `[min,max]` et chaque expression `CSL nn : ...`.
//!
//! LES LISTES DE VALEURS. « Une rubrique assortie d'une liste de valeurs ne peut contenir qu'une des valeurs indiquees
//! dans cette liste » (cahier 2027.1.1 p. 86, `texte-2027/chapitre-4-controles.txt` l. 164). Le releve la prend, code et
//! libelle, la ou le texte la donne : dans le classeur, la colonne « Values » du type (`code=libelle;...`) ; dans le
//! cahier en PDF, les lignes `code - libelle` qui suivent la longueur de la rubrique (`valeurs_du_cahier`). Le moteur
//! ne s'en sert pas pour refuser une valeur : les `enum` qui disent une liste de la norme
//! (`droit::natures::MotifDeRecours`, `MotifDeRupture`, `ModaliteDuTempsDeTravail`) s'y tiennent, par un test de
//! structure (`moteur/tests/fichiers/structure.rs`).
//!
//! UNE EXPRESSION QUE LE MOTEUR NE SAIT PAS COMPILER (`formats::Regex::compiler`) EST LAISSEE TOMBER, plutot que reparee a
//! la main : une expression devinee refuserait des valeurs licites. Elle est nommee dans `expressions_ecartees`. Le
//! critere est celui du moteur, qui seul se sert de ces expressions ; sur les sources deposees, il ecarte trois
//! expressions, coupees par la mise en page.
//!
//! CE QUE LE RELEVE VAUT. Il est lu dans un document, donc il peut etre faux, et un controle faux refuserait une
//! declaration licite -- pire que pas de controle du tout (regle 7). Une expression qui refuse une valeur deja acceptee
//! par net-entreprises est retiree du releve, pas imposee au moteur.
//!
//! Le PDF se lit une fois, a son depot : sa transcription (`transcrire`, `super::Transcription`) redonne le meme releve.

use std::path::Path;

use regex::Regex;
use serde::Serialize;

use super::usages::{en_classeur, version_du_cahier};
use super::{blancs_ramenes, date_du_releve, en_transcription, nfc, nom_de, sha256, EnOrdre, Releve, Transcription};
use droit::natures::RubriqueDeLaNorme;
use socle::date::Jour;
use socle::lecture_xlsx::{self, Cellule};
use socle::echec::R;
use socle::texte::{blanc as espace, strip};

const QUOI: &str = "Le format de chaque rubrique du cahier technique NEODeS : sa longueur [min,max], ses expressions de \
                      controle CSL et, quand elle en a une, sa liste de valeurs (code -> libelle). Le tableau des usages dit si une rubrique a le droit d'etre la ; ceci dit ce qu'elle doit \
                      contenir. Ce n'est pas un validateur : il verifie la FORME d'une valeur, pas sa justesse : un montant \
                      faux au bon format passe.";

/// Le referentiel des formats, apres `quoi` et `note`.
#[derive(Serialize)]
struct Formats {
    #[serde(flatten)]
    empreinte: Empreinte,
    expressions_ecartees: Vec<Ecartee>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    types_inconnus: Vec<SansType>,
    formats: EnOrdre<Format>,
}

/// L'empreinte du texte depose lu : le classeur, ou le cahier en PDF.
#[derive(Serialize)]
enum Empreinte {
    #[serde(rename = "sha256_du_classeur")]
    Classeur(String),
    #[serde(rename = "sha256_du_pdf")]
    Pdf(String),
}

/// Le format d'une rubrique : sa longueur [min, max], ses expressions de controle que le moteur compile, et sa liste de
/// valeurs quand le texte lui en donne une (code -> libelle, dans l'ordre du texte).
#[derive(Serialize)]
struct Format {
    #[serde(skip_serializing_if = "Option::is_none")]
    longueur: Option<[i64; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    csl: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    valeurs: Option<EnOrdre<String>>,
}

/// Une liste de valeurs, code -> libelle (blancs ramenes a un, forme composee) ; un code deja pose garde son premier
/// libelle, et le double se dit au bilan (`notes`).
fn liste_de(paires: Vec<(String, String)>, rubrique: RubriqueDeLaNorme, notes: &mut Vec<String>) -> Option<EnOrdre<String>> {
    let mut liste = EnOrdre::new();
    for (code, libelle) in paires {
        if liste.valeur(&code).is_some() {
            notes.push(format!("{} : le code {} deux fois dans la liste (le premier libelle garde)", rubrique, code));
            continue;
        }
        liste.poser(code, nfc(&blancs_ramenes(&libelle)));
    }
    Some(liste).filter(|l| !l.is_empty())
}

/// LA LISTE DE VALEURS D'UN TYPE DU CLASSEUR (colonne « Values » de la feuille « Data Types ») : `code=libelle`,
/// separes par `;`. Un morceau sans `=` ne se lit pas : refus, plutot qu'un libelle devine.
fn valeurs_du_classeur(texte: &str, rubrique: RubriqueDeLaNorme) -> R<Vec<(String, String)>> {
    let mut paires = Vec::new();
    for morceau in texte.split(';') {
        if strip(morceau).is_empty() {
            continue;
        }
        let Some((code, libelle)) = morceau.split_once('=') else {
            return Err(refus!("{} : la liste de valeurs du type a un morceau sans « = » : {}", rubrique, socle::fichiers::repr(morceau)));
        };
        paires.push((strip(code).to_string(), libelle.to_string()));
    }
    Ok(paires)
}

/// Le pied d'une page du cahier qui commence a la ligne `i` (le bloc, `S21.G00.40` ; la date du cahier ; des lignes
/// blanches ; `195 / 361`) : l'indice de la ligne qui le suit.
fn pied_de_page(lignes: &[&str], i: usize) -> Option<usize> {
    let bloc = Regex::new(r"^S\d\d\.G00\.\d\d$").expect("motif");
    let date = Regex::new(r"^\d{4}-\d\d-\d\d$").expect("motif");
    let page = Regex::new(r"^\d+ / \d+$").expect("motif");
    if !bloc.is_match(lignes.get(i)?) || !date.is_match(lignes.get(i + 1)?) {
        return None;
    }
    let mut j = i + 2;
    while lignes.get(j).is_some_and(|l| l.is_empty()) {
        j += 1;
    }
    page.is_match(lignes.get(j)?).then_some(j + 1)
}

/// LA LISTE DE VALEURS D'UNE RUBRIQUE DU CAHIER EN PDF : ce qui suit sa longueur (`zone`). Les pieds de page, et les
/// lignes blanches qui les entourent, s'otent d'abord : une liste continue d'une page a l'autre. La liste commence a
/// la premiere ligne non blanche, qui doit etre une valeur (`01 - libelle` : un code en chiffres et capitales, un
/// tiret, le libelle) -- sinon la rubrique n'a pas de liste (une table de nomenclature, une expression). Chaque ligne
/// suivante est une valeur, ou la suite du libelle de la precedente ; la liste finit a la premiere ligne blanche, ou a
/// une expression de controle (`CSL`). Une liste qui court jusqu'au bout du corps finit par le nom de la rubrique
/// suivante, ecrit sans ligne blanche (S21.G00.40.021, « Code caisse professionnelle de congés payés ») : sa derniere
/// ligne, si elle n'est pas une valeur, n'est pas a elle.
fn valeurs_du_cahier(zone: &str) -> Vec<(String, String)> {
    let valeur = Regex::new(r"^([0-9A-Z]{1,8}) [-–] (.+)$").expect("motif");
    let lignes: Vec<&str> = zone.split('\n').map(strip).collect();
    let mut gardees: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < lignes.len() {
        if let Some(apres) = pied_de_page(&lignes, i) {
            while gardees.last().is_some_and(|l| l.is_empty()) {
                gardees.pop();
            }
            i = apres;
            while lignes.get(i).is_some_and(|l| l.is_empty()) {
                i += 1;
            }
            continue;
        }
        gardees.push(lignes[i]);
        i += 1;
    }
    let mut paires: Vec<(String, String)> = Vec::new();
    // Le libelle de la derniere valeur avant sa derniere suite : la rendre si la liste court jusqu'au bout du corps.
    let mut avant_la_suite: Option<String> = None;
    let mut jusqu_au_bout = true;
    for l in gardees.iter().skip_while(|l| l.is_empty()) {
        if l.is_empty() || l.starts_with("CSL") {
            jusqu_au_bout = false;
            break;
        }
        if let Some(c) = valeur.captures(l) {
            paires.push((c[1].to_string(), c[2].to_string()));
            avant_la_suite = None;
            continue;
        }
        let Some((_, libelle)) = paires.last_mut() else {
            jusqu_au_bout = false;
            break;
        };
        avant_la_suite = Some(libelle.clone());
        libelle.push(' ');
        libelle.push_str(l);
    }
    if jusqu_au_bout {
        if let (Some(avant), Some((_, libelle))) = (avant_la_suite, paires.last_mut()) {
            *libelle = avant;
        }
    }
    paires
}

/// Une expression ecartee : la rubrique, et l'expression que le moteur ne compile pas.
#[derive(Serialize)]
struct Ecartee {
    rubrique: RubriqueDeLaNorme,
    coupee: String,
}

/// Une rubrique dont le type de donnees n'est pas dans la feuille des types : la case telle qu'elle est (vide : `null`).
#[derive(Serialize)]
struct SansType {
    rubrique: RubriqueDeLaNorme,
    #[serde(rename = "type")]
    type_: Option<Cellule>,
}

/// L'expression est-elle de celles que le moteur sait compiler ?
fn compile(expression: &str) -> bool {
    match declarations::formats::Regex::compiler(expression) {
        Ok(_) => true,
        // Le refus est la reponse : le releve note l'expression comme non compilee.
        Err(la_reponse) => {
            let _ = la_reponse;
            false
        }
    }
}

/// Les groupes de `CSL\s*\d*\s*:\s*(.*?)(?=\n[ \t]*\n)` (drapeau `S` : `.` prend aussi le saut de ligne), cherches de
/// gauche a droite, sans chevauchement. UNE EXPRESSION DE CONTROLE VA JUSQU'A LA LIGNE VIDE SUIVANTE : le PDF la coupe en
/// plein milieu d'un motif, sans espace (`(20)[0-` puis `9]{2}`) ; on la recolle ensuite en retirant les sauts de ligne.
/// La recherche suit pas a pas un moteur a retour arriere (le motif a une anticipation), y compris quand la ligne vide est
/// mangee par les blancs qui suivent les deux-points.
fn csl_trouvees(corps: &str) -> Vec<String> {
    let entete = Regex::new(r"\ACSL[\s\x1c-\x1f]*\d*[\s\x1c-\x1f]*:").expect("motif");
    let octets = corps.as_bytes();
    // Une ligne vide commence-t-elle a `p` : `\n[ \t]*\n` ?
    let ligne_vide = |p: usize| -> bool {
        if octets.get(p) != Some(&b'\n') {
            return false;
        }
        let mut q = p + 1;
        while matches!(octets.get(q), Some(b' ') | Some(b'\t')) {
            q += 1;
        }
        octets.get(q) == Some(&b'\n')
    };
    let mut trouvees = Vec::new();
    let mut depart = 0;
    while let Some(i) = corps[depart..].find("CSL").map(|i| depart + i) {
        let Some(m) = entete.find(&corps[i..]) else {
            depart = i + 1;
            continue;
        };
        let apres = i + m.end();
        // `\s*` gourmand : la fin des blancs, puis, a reculons, chaque point ou il aurait pu s'arreter.
        let mut fin_blancs = apres;
        for c in corps[apres..].chars() {
            if !espace(c) {
                break;
            }
            fin_blancs += c.len_utf8();
        }
        let mut positions = vec![fin_blancs];
        let mut q = fin_blancs;
        while q > apres {
            let Some(c) = corps[..q].chars().next_back() else { break };
            q -= c.len_utf8();
            positions.push(q);
        }
        // `(.*?)` paresseux : la premiere ligne vide a partir du debut du groupe.
        let mut retenue = None;
        for debut_groupe in positions {
            if let Some(p) =
                (debut_groupe..=corps.len()).filter(|p| corps.is_char_boundary(*p)).find(|p| ligne_vide(*p))
            {
                retenue = Some((debut_groupe, p));
                break;
            }
        }
        match retenue {
            Some((a, b)) => {
                trouvees.push(corps[a..b].to_string());
                depart = b.max(i + 1);
            }
            None => depart = i + 1,
        }
    }
    trouvees
}

/// Les formats lus dans le texte du cahier.
fn relever_pdf(texte: &str, ecartees: &mut Vec<Ecartee>, notes: &mut Vec<String>) -> R<EnOrdre<Format>> {
    let rubrique = Regex::new(r"(?m)^[\s\x1c-\x1f]*(S\d\d\.G00\.\d\d\.\d\d\d)[\s\x1c-\x1f]*$").expect("motif");
    let longueur = Regex::new(r"\[(\d+),(\d+)\]").expect("motif");
    // `RUBRIQUE.split(texte)` : [avant, id, corps, id, corps...].
    let mut morceaux: Vec<(&str, &str)> = Vec::new();
    let mut ouvert: Option<(&str, usize)> = None;
    for c in rubrique.captures_iter(texte) {
        let (tout, id) = (c.get(0).expect("match"), c.get(1).expect("groupe"));
        if let Some((rid, debut)) = ouvert {
            morceaux.push((rid, &texte[debut..tout.start()]));
        }
        ouvert = Some((id.as_str(), tout.end()));
    }
    if let Some((rid, debut)) = ouvert {
        morceaux.push((rid, &texte[debut..]));
    }
    let mut out = EnOrdre::new();
    // Chaque identifiant apparait deux fois : dans la liste des rubriques du bloc (sans format) et dans sa definition
    // (avec). On garde celle qui porte un format, et la derniere si plusieurs en portent.
    for (rid, corps) in morceaux {
        let rid = RubriqueDeLaNorme::lire(rid)?;
        // ET ELLE S'ARRETE A LA PREMIERE ESPACE. Aucune CSL du cahier n'en contient ; en revanche la mise en page colle
        // parfois le libelle suivant derriere (S10.G00.01.002 recuperait « Table OES - Organismes Emetteurs... », ce qui
        // faisait refuser un NIC pourtant accepte cinquante fois).
        let csl: Vec<String> = csl_trouvees(corps)
            .iter()
            .map(|c| {
                let c = strip(&c.replace('\n', "")).to_string();
                match c.split_once(' ') {
                    Some((avant, _)) => avant.to_string(),
                    None => c,
                }
            })
            .collect();
        let lon = longueur.captures(corps);
        if csl.is_empty() && lon.is_none() {
            continue;
        }
        let (longueur, valeurs) = match lon {
            Some(l) => {
                let n = |i: usize| l[i].parse::<i64>().map_err(|e| refus!("{} : longueur illisible {} ({})", rid, &l[0], e));
                let apres = l.get_match().end();
                (Some([n(1)?, n(2)?]), liste_de(valeurs_du_cahier(&corps[apres..]), rid, notes))
            }
            None => (None, None),
        };
        // Plusieurs CSL = plusieurs formes admises (CSL 00, CSL 01...), sans doublon, dans l'ordre.
        let mut vues: Vec<&String> = Vec::new();
        let mut bonnes = Vec::new();
        for c in &csl {
            if vues.contains(&c) {
                continue;
            }
            vues.push(c);
            if compile(c) {
                bonnes.push(c.clone());
            } else {
                ecartees.push(Ecartee { rubrique: rid, coupee: c.clone() });
            }
        }
        out.poser(rid.to_string(), Format { longueur, csl: Some(bonnes).filter(|b| !b.is_empty()), valeurs });
    }
    Ok(out)
}

/// La cle d'une case : un entier, un nombre a virgule de valeur entiere et un booleen (0 ou 1) de meme valeur se
/// confondent ; un texte n'egale qu'un texte.
fn cle(c: &Cellule) -> R<String> {
    Ok(match c {
        Cellule::Texte(s) => format!("t{}", s),
        Cellule::Entier(i) => format!("n{}", i),
        Cellule::AVirgule(n) => match entier_exact(n)? {
            Some(i) => format!("n{}", i),
            None => format!("f{}", n.ecrit()),
        },
        Cellule::Booleen(b) => format!("n{}", *b as i64),
    })
}

/// L'entier qu'un nombre a virgule vaut exactement (`12.0`), ou `None` ; un nombre qui ne tient pas en decimal refuse.
fn entier_exact(n: &socle::forme::NombreLu) -> R<Option<i64>> {
    use rust_decimal::prelude::ToPrimitive;
    Ok(Some(n.decimal()?).filter(|d| d.fract().is_zero()).and_then(|d| d.to_i64()))
}

/// La valeur de la derniere colonne titree `nom` (une ligne plus courte que l'en-tete se complete de vides), ou `None`.
fn champ<'a>(entete: &[Option<Cellule>], ligne: &'a [Option<Cellule>], nom: &str) -> Option<Option<&'a Cellule>> {
    let j = entete.iter().rposition(|t| matches!(t, Some(Cellule::Texte(s)) if s == nom))?;
    Some(ligne.get(j).and_then(Option::as_ref))
}

fn champ_exige<'a>(
    entete: &[Option<Cellule>],
    ligne: &'a [Option<Cellule>],
    nom: &str,
    feuille: &str,
) -> R<Option<&'a Cellule>> {
    champ(entete, ligne, nom).ok_or_else(|| refus!("la feuille « {} » n'a pas de colonne « {} »", feuille, nom))
}

/// L'entier d'une case : un entier, un nombre a virgule tronque, un booleen (0 ou 1), un texte d'entier.
fn entier_de(c: &Cellule) -> R<i64> {
    use rust_decimal::prelude::ToPrimitive;
    match c {
        Cellule::Entier(i) => Ok(*i),
        Cellule::AVirgule(n) => n.decimal()?.trunc().to_i64().ok_or_else(|| refus!("longueur illisible : {}", n.ecrit())),
        Cellule::Booleen(b) => Ok(*b as i64),
        Cellule::Texte(s) => {
            strip(s).parse::<i64>().map_err(|_| refus!("longueur illisible : {}", socle::fichiers::repr(s)))
        }
    }
}

/// Les formats lus dans le classeur des types de donnees (fiche 3357).
fn relever_xlsx(
    chemin: &Path,
    ecartees: &mut Vec<Ecartee>,
    sans_type: &mut Vec<SansType>,
    notes: &mut Vec<String>,
) -> R<EnOrdre<Format>> {
    let mut types: std::collections::HashMap<String, Vec<Option<Cellule>>> = std::collections::HashMap::new();
    let mut entete_types: Option<Vec<Option<Cellule>>> = None;
    for r in lecture_xlsx::lignes(chemin, "Data Types")? {
        let Some(entete) = &entete_types else {
            entete_types = Some(r);
            continue;
        };
        if let Some(Some(premiere)) = r.first() {
            if premiere.vrai() {
                let mut valeurs = r.clone();
                valeurs.resize(entete.len().max(valeurs.len()), None);
                types.insert(cle(premiere)?, valeurs);
            }
        }
    }
    let entete_types = entete_types.ok_or_else(|| refus!("{} : la feuille « Data Types » est vide", chemin.display()))?;
    let mut out = EnOrdre::new();
    let mut entete: Option<Vec<Option<Cellule>>> = None;
    for r in lecture_xlsx::lignes(chemin, "Fields")? {
        let Some(e) = &entete else {
            entete = Some(r);
            continue;
        };
        if !matches!(r.first(), Some(Some(c)) if c.vrai()) {
            continue;
        }
        let bloc = champ_exige(e, &r, "Block Id", "Fields")?;
        let id = champ_exige(e, &r, "Id", "Fields")?;
        let type_id = champ_exige(e, &r, "DataType Id", "Fields")?;
        let texte = |c: Option<&Cellule>, nom: &str| match c {
            Some(c) => Ok(strip(&c.en_texte()).to_string()),
            None => Err(refus!("la feuille « Fields » a une ligne sans « {} »", nom)),
        };
        let rid = RubriqueDeLaNorme::lire(&format!("{}.{}", texte(bloc, "Block Id")?, texte(id, "Id")?))?;
        let genre = match type_id {
            Some(t) => types.get(&cle(t)?),
            None => None,
        };
        let Some(genre) = genre else {
            sans_type.push(SansType { rubrique: rid, type_: type_id.cloned() });
            continue;
        };
        let lg = |nom: &str| champ(&entete_types, genre, nom).flatten();
        let longueur = match (lg("Lg Min"), lg("Lg Max")) {
            (Some(min), Some(max)) => Some([entier_de(min)?, entier_de(max)?]),
            _ => None,
        };
        let expression = match lg("Regexp") {
            Some(Cellule::Texte(s)) => Some(strip(s).to_string()).filter(|e| !e.is_empty()),
            Some(c) if c.vrai() => {
                return Err(refus!("{} : l'expression du type n'est pas un texte ({})", rid, c.en_texte()))
            }
            _ => None,
        };
        let csl = match expression {
            Some(e) if compile(&e) => Some(vec![e]),
            Some(e) => {
                ecartees.push(Ecartee { rubrique: rid, coupee: e });
                None
            }
            None => None,
        };
        let valeurs = match champ_exige(&entete_types, genre, "Values", "Data Types")? {
            Some(Cellule::Texte(s)) => liste_de(valeurs_du_classeur(s, rid)?, rid, notes),
            Some(c) if c.vrai() => {
                return Err(refus!("{} : la liste de valeurs du type n'est pas un texte ({})", rid, c.en_texte()))
            }
            _ => None,
        };
        if longueur.is_some() || csl.is_some() {
            out.poser(rid.to_string(), Format { longueur, csl, valeurs });
        } else if valeurs.is_some() {
            notes.push(format!("{} : une liste de valeurs, sans longueur ni expression (non reprise)", rid));
        }
    }
    Ok(out)
}

/// LA TRANSCRIPTION DES DEFINITIONS du cahier en PDF (`super::Transcription`) : son texte, tel que la fabrique de PDF le
/// lit, les pages separees par un saut de ligne -- ce que `relever_pdf` lit.
pub fn transcrire(pdf: &Path) -> R<String> {
    version_du_cahier(pdf)?;
    let (pages, _) = fabrique_pdf::lecture::texte(&pdf.to_string_lossy()).map_err(documents::fabrique::echec)?;
    let textes = pages.iter().map(|p| p.lu().map_err(documents::fabrique::echec)).collect::<R<Vec<String>>>()?;
    let t = Transcription::de(pdf, Vec::new(), textes.join("\n"))?;
    Ok(t.texte(&format!("Le texte de {}, tel que la fabrique de PDF le lit (`moteur extraire transcrire`)", nom_de(pdf))))
}

/// Le releve des formats, depuis le classeur, le cahier en PDF ou sa transcription.
pub fn relever(source: &Path, jour: &Jour) -> R<Releve> {
    let date = date_du_releve(jour);
    let (mut ecartees, mut sans_type, mut notes) = (Vec::new(), Vec::new(), Vec::new());
    let classeur = en_classeur(source);
    let (formats, provenance, empreinte) = if classeur {
        let chemin = source.to_string_lossy();
        let v = Regex::new(r"datatypes-usages-(\d[\d.]*\d)-\d{8}\.xlsx$")
            .expect("motif")
            .captures(&chemin)
            .map(|c| c[1].to_string())
            .ok_or_else(|| {
                refus!(
                    "version illisible dans le nom du classeur : {} (attendu \
                     dsn-tableaux-categories-controles-datatypes-usages-<version>-<AAAAMMJJ>.xlsx)",
                    chemin
                )
            })?;
        let formats = relever_xlsx(source, &mut ecartees, &mut sans_type, &mut notes)?;
        let provenance = format!(
            "Formats des rubriques de la norme DSN {}, classeur {} (feuilles « Fields » et « Data Types », les listes \
             de valeurs dans sa colonne « Values »). Releve \
             automatiquement par `moteur extraire formats` le {}.",
            v,
            nom_de(source),
            date
        );
        (formats, provenance, sha256(source)?)
    } else {
        // Le texte du cahier : sa transcription (`dsn-cahier-technique-<version>-formats.txt`), ou le PDF lui-meme.
        let (pdf, texte, empreinte) = if en_transcription(source) {
            let t = Transcription::lire(source)?;
            (t.pdf, t.corps, t.sha256)
        } else {
            let (pages, _) = fabrique_pdf::lecture::texte(&source.to_string_lossy()).map_err(documents::fabrique::echec)?;
            let textes = pages.iter().map(|p| p.lu().map_err(documents::fabrique::echec)).collect::<R<Vec<String>>>()?;
            (nom_de(source), textes.join("\n"), sha256(source)?)
        };
        let v = version_du_cahier(Path::new(&pdf))?;
        let formats = relever_pdf(&texte, &mut ecartees, &mut notes)?;
        let provenance = format!(
            "Cahier technique NEODeS {} ({}), definitions des rubriques (les listes de valeurs : ce qui suit la \
             longueur). Releve automatiquement par \
             `moteur extraire formats` le {}.",
            v,
            pdf,
            date
        );
        (formats, provenance, empreinte)
    };
    let mut bilan = vec![format!(
        "{} rubriques, dont {} avec expression de controle, {} avec liste de valeurs",
        formats.len(),
        formats.0.iter().filter(|(_, f)| f.csl.is_some()).count(),
        formats.0.iter().filter(|(_, f)| f.valeurs.is_some()).count()
    )];
    for n in &notes {
        bilan.push(format!("  {}", n));
    }
    for e in &ecartees {
        bilan.push(format!(
            "  ecartee (le moteur ne la compile pas) : {} -- {}",
            e.rubrique,
            e.coupee.chars().take(40).collect::<String>()
        ));
    }
    for s in &sans_type {
        bilan.push(format!("  type de donnees inconnu : {} -- {}", s.rubrique, socle::ecriture::json_en_ligne(&s.type_)));
    }
    let note = format!(
            "{} Toute declaration que le moteur ecrit (DSN, signalement, FCTU) passe ce releve avant d'etre ecrite \
               (`declarations::formats::verifier_fichier`), et `declarations/tests/declarations/usages_formats_ntt.rs` \
               en eprouve les expressions motif par motif. Une expression qui refuserait une valeur reellement acceptee \
               serait fausse, et c'est elle qu'on retire -- un controle faux refuse une declaration licite, ce qui est \
               pire que pas de controle.",
            provenance
        );
    let corps = Formats {
        empreinte: if classeur { Empreinte::Classeur(empreinte) } else { Empreinte::Pdf(empreinte) },
        expressions_ecartees: ecartees,
        types_inconnus: sans_type,
        formats,
    };
    Ok(Releve::nouveau(QUOI, note, corps, 1, false, bilan))
}
