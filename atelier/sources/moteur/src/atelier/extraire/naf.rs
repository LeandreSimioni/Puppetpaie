//! `moteur extraire naf <structure.xlsx> <correspondances.xlsx> <sortie.json>` : la NAF 2025 et sa table de correspondance
//! depuis la NAF rev. 2 (INSEE), pour `logiciel/donnees/naf-base.json` (que lit `naf.rs`).
//!
//! Les deux classeurs sont ceux de la page INSEE « NAF 2025 » (https://www.insee.fr/fr/information/8181066), deposes dans
//! `atelier/reference/sources/insee-naf-2025/` (son `LISEZ-MOI.md`) :
//!   - la structure (`insee-naf-2025-structure-20241004.xlsx`), feuille « Sous-classes » : le code (`90.20Y`) et
//!     l'intitule de chaque sous-classe ;
//!   - la correspondance (`insee-naf-2025-correspondances-naf-rev2-vers-naf-2025-20260107.xlsx`), feuille « NAF rév. 2
//!     vers NAF 2025 » : pour chaque sous-classe NAF rev. 2, une ligne par sous-classe NAF 2025 ou passe une partie de son
//!     contenu, avec ce contenu et le type (« Unique », « Multiple »).
//!
//! Les codes s'ecrivent dans le JSON comme la fiche entreprise les porte : quatre chiffres et une lettre, sans le point
//! (`9020Y`) ; le code INSEE, avec son point, est garde a cote. Rien n'est interprete : le releve recopie. La date
//! d'entree en vigueur vient du decret n° 2025-736 du 31/07/2025, depose a cote
//! (`legifrance-decret-2025-736-naf-2025-20250731.txt`, art. 2 et 6).
//!
//! Le classeur ne se lit que par les colonnes nommees ici ; une case qu'une ligne retenue doit porter (un intitule, un
//! type, un contenu) et qui est vide refuse le releve.

use std::path::Path;

use regex::Regex;
use serde::Serialize;

use super::{case, date_du_releve, nom_de, propre, remplie, sha256, EnOrdre, Rang, Releve};
use socle::date::Jour;
use socle::fichiers::repr;
use socle::lecture_xlsx;
use socle::num::Resultat;

const QUOI: &str = "La NAF 2025 (sous-classes) et la correspondance de chaque sous-classe NAF rev. 2 vers la NAF 2025. \
                    La fiche entreprise porte le code APE sans point (`9020Y`). Le moteur s'en sert pour reconnaitre un \
                    code saisi et pour refuser, dans un fichier DSN P27V01, un code NAF rev. 2 qui n'a plus cours, en \
                    disant vers quels codes NAF 2025 il passe : il ne choisit jamais entre eux, c'est l'INSEE qui \
                    attribue le code (`declarations/src/naf.rs`).";

/// Le referentiel `naf-base.json`, apres `quoi` et `note`.
#[derive(Serialize)]
struct Naf {
    sha256: EnOrdre<String>,
    en_vigueur_naf_2025: &'static str,
    naf_2025: EnOrdre<SousClasse>,
    naf_rev2: EnOrdre<Rev2>,
}

/// Une sous-classe NAF 2025 : son code INSEE (avec le point) et son intitule.
#[derive(Serialize)]
struct SousClasse {
    code: String,
    intitule: String,
}

/// Une sous-classe NAF rev. 2, le type de sa correspondance et les sous-classes NAF 2025 ou passe son contenu.
#[derive(Serialize)]
struct Rev2 {
    code: String,
    intitule: String,
    #[serde(rename = "type")]
    type_: String,
    vers: Vec<Vers>,
}

/// Une destination NAF 2025 d'une sous-classe NAF rev. 2.
#[derive(Serialize)]
struct Vers {
    code: String,
    code_insee: String,
    intitule: String,
    contenu: String,
}

fn sans_point(code: &str) -> String {
    code.replace('.', "")
}

/// Le releve : `naf_2025` (sous-classes) et `naf_rev2` (chaque sous-classe rev. 2 et ses destinations).
pub fn relever(structure: &Path, correspondances: &Path, jour: &Jour) -> Resultat<Releve> {
    let code = Regex::new(r"\A\d\d\.\d\d[A-Z]\z").expect("motif");
    let est_un_code = |t: Option<&str>| t.is_some_and(|t| code.is_match(t));
    let mut naf_2025: EnOrdre<SousClasse> = EnOrdre::new();
    for r in lecture_xlsx::lignes(structure, "Sous-classes")? {
        let r: Rang = r.iter().map(propre).collect();
        let Some(c) = case(&r, 0).filter(|c| code.is_match(c)) else { continue };
        let intitule = remplie(&r, 1, &format!("sous-classe {}", c))?;
        naf_2025.poser(sans_point(c), SousClasse { code: c.to_string(), intitule: intitule.to_string() });
    }
    let mut naf_rev2: EnOrdre<Rev2> = EnOrdre::new();
    for r in lecture_xlsx::lignes(correspondances, "NAF rév. 2 vers NAF 2025")? {
        let r: Rang = r.iter().map(propre).collect();
        let Some(c) = case(&r, 0).filter(|c| code.is_match(c)) else { continue };
        let vers = case(&r, 3);
        let Some(vers) = vers.filter(|v| est_un_code(Some(v)) && naf_2025.valeur(&sans_point(v)).is_some()) else {
            let dit = match vers {
                Some(v) => repr(v),
                None => "une case vide".to_string(),
            };
            return Err(format!("correspondance de {} vers un code absent de la structure : {}", c, dit));
        };
        let ou = format!("correspondance de {} vers {}", c, vers);
        let (intitule, contenu, intitule_vers, type_) = (remplie(&r, 1, &ou)?, remplie(&r, 2, &ou)?, remplie(&r, 4, &ou)?, remplie(&r, 5, &ou)?);
        let entree = naf_rev2.entree(sans_point(c), || Rev2 { code: c.to_string(), intitule: intitule.to_string(), type_: type_.to_string(), vers: Vec::new() });
        if entree.type_ != type_ {
            return Err(format!("type de correspondance incoherent pour {} : {} et {}", c, repr(&entree.type_), repr(type_)));
        }
        entree.vers.push(Vers { code: sans_point(vers), code_insee: vers.to_string(), intitule: intitule_vers.to_string(), contenu: contenu.to_string() });
    }
    let mut communs: Vec<&String> = naf_2025.0.iter().map(|(k, _)| k).filter(|k| naf_rev2.valeur(k).is_some()).collect();
    communs.sort();
    if !communs.is_empty() {
        let liste: Vec<String> = communs.iter().map(|c| repr(c)).collect();
        return Err(format!(
            "codes communs aux deux nomenclatures, le moteur ne saurait pas les dater : [{}]",
            liste.join(", ")
        ));
    }
    let (n_2025, n_rev2) = (naf_2025.len(), naf_rev2.len());
    let (nom_s, nom_c) = (nom_de(structure), nom_de(correspondances));
    let sha = EnOrdre(vec![(nom_s.clone(), sha256(structure)?), (nom_c.clone(), sha256(correspondances)?)]);
    let note = format!(
        "INSEE, page « NAF 2025 » (https://www.insee.fr/fr/information/8181066) : {} (structure, feuille \
         « Sous-classes ») et {} (feuille « NAF rév. 2 vers NAF 2025 »), deposes dans \
         atelier/reference/sources/insee-naf-2025/. Releve automatiquement par `moteur extraire naf` le {}. \
         Decret n° 2025-736 du 31/07/2025 (atelier/reference/sources/insee-naf-2025/\
         legifrance-decret-2025-736-naf-2025-20250731.txt) : art. 2, la NAF 2025 « entre en \
         vigueur le 1er janvier 2027 » ; art. 6, le decret de 2007 (NAF rev. 2) est abroge a la \
         meme date pour la nomenclature d'activites.",
        nom_s, nom_c, date_du_releve(jour)
    );
    let corps = Naf { sha256: sha, en_vigueur_naf_2025: "2027-01-01", naf_2025, naf_rev2 };
    Ok(Releve::nouveau(QUOI, note, corps, 1, false, vec![format!("{} sous-classes NAF 2025, {} sous-classes NAF rev. 2", n_2025, n_rev2)]))
}
