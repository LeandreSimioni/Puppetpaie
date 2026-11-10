//! Le tableau des usages de la norme : ce qu'une nature de declaration admet.
//! Un releve par version de la norme : le JSON que
//! `norme-dsn-base.json` nomme (`usages-dsn-base.json` pour P26V01,
//! `usages-dsn-p27v01-base.json` pour P27V01), choisi d'apres
//! `S10.G00.00.006` du fichier controle (`norme::du_fichier`).

use crate::norme::Norme;
use socle::echec::{refus_avec, Lieu, R};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

/// L'usage d'une rubrique pour une nature : obligatoire, conditionnelle, interdite, sans objet.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Usage {
    O,
    C,
    I,
    N,
}

impl Usage {
    /// La lettre du tableau.
    pub fn lettre(self) -> &'static str {
        match self {
            Usage::O => "O",
            Usage::C => "C",
            Usage::I => "I",
            Usage::N => "N",
        }
    }
}

/// Un tableau des usages (`usages-dsn-base.json`, `usages-dsn-p27v01-base.json`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    #[allow(dead_code)]
    quoi: String,
    #[allow(dead_code)]
    note: String,
    #[allow(dead_code)]
    sha256_du_pdf: Option<String>,
    #[allow(dead_code)]
    sha256_du_classeur: Option<String>,
    /// Le libelle de chaque nature de declaration.
    pub natures: BTreeMap<String, String>,
    #[allow(dead_code)]
    valeurs: BTreeMap<String, String>,
    /// Chaque rubrique du tableau.
    pub usages: BTreeMap<String, Entree>,
}

/// Une rubrique du tableau : son libelle, son usage pour chaque nature.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entree {
    pub libelle: String,
    #[serde(rename = "01")]
    mensuelle: Usage,
    #[serde(rename = "04")]
    arret: Usage,
    #[serde(rename = "05")]
    reprise: Usage,
    #[serde(rename = "07")]
    fin_de_contrat: Usage,
    #[serde(rename = "08")]
    amorcage: Usage,
    #[serde(rename = "09")]
    substitution: Usage,
    #[serde(rename = "10")]
    dpae: Usage,
}

impl Entree {
    /// L'usage pour la nature `nature` (S20.G00.05.001), `None` pour une nature que le tableau n'a pas.
    pub fn pour(&self, nature: &str) -> Option<Usage> {
        match nature {
            "01" => Some(self.mensuelle),
            "04" => Some(self.arret),
            "05" => Some(self.reprise),
            "07" => Some(self.fin_de_contrat),
            "08" => Some(self.amorcage),
            "09" => Some(self.substitution),
            "10" => Some(self.dpae),
            _ => None,
        }
    }
}

pub(crate) use socle::texte::{blanc as espace, lignes_du_texte as lignes, strip};

/// `ligne.partition(",")` puis la valeur sans espaces ni apostrophes.
pub fn rubrique_valeur(ligne: &str) -> (&str, &str) {
    match ligne.split_once(',') {
        Some((r, v)) => (r, strip(v).trim_matches('\'')),
        None => (ligne, ""),
    }
}

/// Le tableau des usages de cette version de la norme.
pub fn table_de(norme: &Norme) -> R<Arc<Table>> {
    crate::norme::livree(&norme.table_usages()?)
}

/// `S\d\d\.G00\.\d\d\.\d\d\d`, en entier.
pub fn est_rubrique(r: &str) -> bool {
    let c: Vec<char> = r.chars().collect();
    c.len() == 14
        && c[0] == 'S'
        && c[3] == '.'
        && c[4..7] == ['G', '0', '0']
        && c[7] == '.'
        && c[10] == '.'
        && [1, 2, 8, 9, 11, 12, 13].iter().all(|&i| c[i].is_ascii_digit())
}

/// La rubrique est-elle au tableau des usages de cette version, quelle que soit la nature ?
pub fn rubrique_connue(norme: &Norme, rubrique: &str) -> R<bool> {
    Ok(table_de(norme)?.usages.contains_key(rubrique))
}

/// L'usage de la rubrique pour cette nature dans le tableau de cette version ; `None` si le tableau n'a pas la
/// rubrique, ou pas la nature.
pub fn usage(norme: &Norme, rubrique: &str, nature: &str) -> R<Option<Usage>> {
    Ok(table_de(norme)?.usages.get(rubrique).and_then(|e| e.pour(nature)))
}

/// Ce que `usage_en` rend pour une rubrique ou une nature que le tableau n'a pas.
const INCONNUE: &str = "";

/// 'O', 'C', 'I', 'N' -- ou "" si la rubrique est inconnue du tableau de cette version.
pub fn usage_en(norme: &Norme, rubrique: &str, nature: &str) -> R<String> {
    Ok(match usage(norme, rubrique, nature)? {
        Some(u) => u.lettre().to_string(),
        None => INCONNUE.to_string(),
    })
}

/// Les rubriques ecrites que cette nature n'admet pas : (rubrique, usage, `None` si le tableau ne la connait pas).
pub fn interdites(texte: &str, nature: &str, norme: &Norme) -> R<Vec<(String, Option<Usage>)>> {
    let mut vues = HashSet::new();
    let mut out = Vec::new();
    for ligne in lignes(texte) {
        let r = strip(match ligne.split_once(',') {
            Some((r, _)) => r,
            None => ligne,
        });
        if !est_rubrique(r) || !vues.insert(r.to_string()) {
            continue;
        }
        let u = usage(norme, r, nature)?;
        if !matches!(u, Some(Usage::O | Usage::C)) {
            out.push((r.to_string(), u));
        }
    }
    Ok(out)
}

/// Refuse avant le depot ce que le point de depot refuserait (CST-04).
pub fn verifier(texte: &str, nature: &str, norme: &Norme) -> R<()> {
    let mauvaises = interdites(texte, nature, norme)?;
    if mauvaises.is_empty() {
        return Ok(());
    }
    let (t, table) = (table_de(norme)?, norme.table_usages()?);
    let libelle = t.natures.get(nature).ok_or_else(|| socle::echec::livree(&table, &format!("natures.{}", nature)))?;
    let lignes: Vec<String> = mauvaises
        .iter()
        .map(|(r, u)| {
            let quoi = match u {
                Some(Usage::I) => "interdite",
                Some(Usage::N) => "sans objet",
                Some(autre) => autre.lettre(),
                None => "inconnue du tableau",
            };
            match t.usages.get(r) {
                Some(e) => format!("    {} : {} ({})", r, quoi, e.libelle),
                None => format!("    {} : {}", r, quoi),
            }
        })
        .collect();
    Err(refus_avec(
        format!(
            "Rubrique(s) que la nature {} ({}) n'admet pas -- anomalie bloquante CST-04, le fichier ne declarerait rien :\n{}\n  Tableau des usages du cahier technique NEODeS.",
            nature,
            libelle,
            lignes.join("\n")
        ),
        Lieu { fichier: Some(table), ..Lieu::default() },
    ))
}

/// La nature declaree dans le fichier lui-meme (S20.G00.05.001).
pub fn nature_du_fichier(texte: &str) -> Option<String> {
    lignes(texte).into_iter().find_map(|ligne| ligne.strip_prefix("S20.G00.05.001,").map(|reste| strip(reste).trim_matches('\'').to_string()))
}

const CCH_14: &str = concat!(
    "CCH-14 : Si la \"Nature de la declaration - S20.G00.05.001\" est ",
    "\"07 - Signalement Fin du contrat de travail Unique\", alors la valeur ",
    "\"31 - Elements de cotisation Prevoyance, Sante, retraite ",
    "supplementaire\" est interdite pour la rubrique \"Code de base ",
    "assujettie - S21.G00.78.001\". ",
    "(Cahier technique NEODeS 2026.1, p. 293.)"
);

const CCH_15: &str = concat!(
    "CCH-15 : Si la \u{ab} Nature de la declaration - S20.G00.05.001 \u{bb} ",
    "est \u{ab} 07 \u{bb} alors, pour un meme bloc \u{ab} Versement ",
    "individu - S21.G00.50 \u{bb}, au moins trois blocs \u{ab} Remuneration ",
    "- S21.G00.51 \u{bb} enfants sont requis : un de type (S21.G00.51.011) ",
    "\u{ab} 001 - Remuneration brute non plafonnee \u{bb}, un de type ",
    "\u{ab} 002 - Salaire brut servant aux calculs des droits de l'Assurance ",
    "chomage \u{bb} et un de type \u{ab} 010 - Salaire de base \u{bb}. ",
    "(Cahier technique NEODeS 2026.1, p. 248.)"
);

const TYPES_51_FCTU: [&str; 3] = ["001", "002", "010"];

/// Les deux controles que la nature 07 ajoute au tableau des usages.
pub fn verifier_fctu(texte: &str) -> R<()> {
    let mut versements: Vec<HashSet<String>> = Vec::new();
    for ligne in lignes(texte) {
        let (rubrique, valeur) = rubrique_valeur(ligne);
        if rubrique == "S21.G00.50.001" {
            versements.push(HashSet::new());
        } else if rubrique == "S21.G00.51.011" && !versements.is_empty() {
            versements.last_mut().unwrap().insert(valeur.to_string());
        } else if rubrique == "S21.G00.78.001" && valeur == "31" {
            return Err(socle::refus!(
                "Base assujettie de code 31 dans un signalement de fin de contrat.\n  {}",
                CCH_14
            ));
        }
    }
    for (i, types) in versements.iter().enumerate() {
        let manquants: Vec<&str> = TYPES_51_FCTU.iter().copied().filter(|x| !types.contains(*x)).collect();
        if !manquants.is_empty() {
            return Err(socle::refus!(
                "Versement individu n\u{b0} {} : il manque le(s) bloc(s) 51 de type {}.\n  {}",
                i + 1,
                manquants.join(", "),
                CCH_15
            ));
        }
    }
    Ok(())
}

/// Confronte un fichier produit a sa propre nature, dans le tableau de sa propre version (S10.G00.00.006) : un
/// fichier sans version, ou d'une version que le logiciel ne livre pas, refuse.
pub fn verifier_fichier(texte: &str) -> R<()> {
    let norme = crate::norme::du_fichier(texte)?;
    let Some(nature) = nature_du_fichier(texte) else { return Ok(()) };
    if table_de(&norme)?.natures.contains_key(&nature) {
        verifier(texte, &nature, &norme)?;
    }
    if nature == "07" {
        verifier_fctu(texte)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::norme::Norme;

    #[test]
    fn un_fichier_sans_version_connue_ne_se_controle_pas() {
        let e = verifier_fichier("S10.G00.00.006,'P25V01'\r\nS20.G00.05.001,'01'\r\n").unwrap_err();
        assert!(e.phrase.contains("Version de la norme DSN « P25V01 » inconnue du logiciel (versions livrées : P26V01, P27V01"), "{}", e.phrase);
    }

    /// Le releve de `moteur-atelier extraire usages`, sur le PDF 2027.1.1 puis sur les classeurs
    /// 2027.1.2 (fiches 3357 et 3358) : 20 rubriques ajoutees, 2 retirees, 11.024 qui change d'usage en
    /// mensuelle (N9) ; le 2027.1.2 passe 40.084 de O a C en mensuelle (journal de maintenance, evolution 21).
    /// `atelier/reference/norme-dsn/dsn-tableau-des-usages-2027.1.2-20251224.xlsx`, `dsn-journal-maintenance-2027.1.2-20260615.txt`.
    #[test]
    fn les_usages_2027_sont_ceux_du_cahier_2027() {
        let (p26, p27) = (Norme::connue("P26V01").unwrap(), Norme::connue("P27V01").unwrap());
        let (u26, u27) = (table_de(&p26).unwrap(), table_de(&p27).unwrap());
        let (a, b) = (&u26.usages, &u27.usages);
        let retirees: Vec<&String> = a.keys().filter(|k| !b.contains_key(*k)).collect();
        let ajoutees: Vec<&String> = b.keys().filter(|k| !a.contains_key(*k)).collect();
        assert_eq!(retirees, ["S21.G00.06.003", "S21.G00.11.002"]);
        assert_eq!(ajoutees.len(), 20, "{:?}", ajoutees);
        assert_eq!(usage_en(&p27, "S21.G00.40.084", "01").unwrap(), "C");
        assert_eq!(usage_en(&p26, "S21.G00.11.024", "01").unwrap(), "I");
        assert_eq!(usage_en(&p27, "S21.G00.11.024", "01").unwrap(), "C");
    }
}
