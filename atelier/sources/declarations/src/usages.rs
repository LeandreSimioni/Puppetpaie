//! Le tableau des usages de la norme : ce qu'une nature de declaration admet.
//! Un releve par version de la norme : le JSON que
//! `norme-dsn-base.json` nomme (`usages-dsn-base.json` pour P26V01,
//! `usages-dsn-p27v01-base.json` pour P27V01), choisi d'apres
//! `S10.G00.00.006` du fichier controle (`norme::du_fichier`).

use crate::norme::Norme;
use droit::nature_declaration::NatureDeLaDeclaration;
use droit::natures::RubriqueDeLaNorme;
use socle::echec::{refus_avec, Lieu, R};
use serde::de::{Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
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
    #[serde(deserialize_with = "socle::forme::non_lue")]
    quoi: socle::forme::NonLue,
    #[serde(deserialize_with = "socle::forme::non_lue")]
    note: socle::forme::NonLue,
    /// L'empreinte du texte releve (le cahier en PDF ou le classeur) : pour le releve de l'atelier.
    #[serde(default, deserialize_with = "socle::forme::non_lue")]
    sha256_du_pdf: socle::forme::NonLue,
    #[serde(default, deserialize_with = "socle::forme::non_lue")]
    sha256_du_classeur: socle::forme::NonLue,
    /// Le libelle de chaque nature de declaration.
    pub natures: BTreeMap<NatureDeLaDeclaration, String>,
    /// Le sens de chaque lettre du tableau (O, C, I, N) : pour qui le lit.
    #[serde(deserialize_with = "socle::forme::non_lue")]
    valeurs: socle::forme::NonLue,
    /// Chaque rubrique du tableau.
    pub usages: BTreeMap<RubriqueDeLaNorme, Entree>,
}

/// UNE RUBRIQUE DU TABLEAU : son libelle, et son usage pour chaque nature de declaration (S20.G00.05.001), toutes
/// dites. Le tableau la donne en un objet : `libelle`, puis une cle par code de nature ; une cle qui n'est ni l'un ni
/// l'autre, une nature en double ou une nature absente refusent le tableau.
pub struct Entree {
    pub libelle: String,
    usages: BTreeMap<NatureDeLaDeclaration, Usage>,
}

impl Entree {
    /// L'usage pour cette nature.
    pub fn pour(&self, nature: NatureDeLaDeclaration) -> Usage {
        self.usages[&nature]
    }
}

/// Une cle d'une rubrique du tableau : son libelle, ou le code d'une nature.
#[derive(Deserialize)]
#[serde(untagged)]
enum CleDEntree {
    Nature(NatureDeLaDeclaration),
    Libelle(MotLibelle),
}

/// La cle `libelle`.
#[derive(Deserialize)]
enum MotLibelle {
    #[serde(rename = "libelle")]
    Libelle,
}

impl<'de> Deserialize<'de> for Entree {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_map(LectureDEntree)
    }
}

struct LectureDEntree;

impl<'de> Visitor<'de> for LectureDEntree {
    type Value = Entree;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("une rubrique du tableau des usages : son libelle et l'usage de chaque nature de declaration")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Entree, A::Error> {
        let (mut libelle, mut usages) = (None, BTreeMap::new());
        while let Some(cle) = m.next_key::<CleDEntree>()? {
            match cle {
                CleDEntree::Libelle(MotLibelle::Libelle) => {
                    if libelle.replace(m.next_value::<String>()?).is_some() {
                        return Err(A::Error::duplicate_field("libelle"));
                    }
                }
                CleDEntree::Nature(n) => {
                    if usages.insert(n, m.next_value::<Usage>()?).is_some() {
                        return Err(A::Error::custom(format!("la nature {} est dite deux fois", n)));
                    }
                }
            }
        }
        let libelle = libelle.ok_or_else(|| A::Error::missing_field("libelle"))?;
        if let Some(n) = NatureDeLaDeclaration::TOUS.iter().find(|n| !usages.contains_key(*n)) {
            return Err(A::Error::custom(format!("la rubrique « {} » ne dit pas son usage pour la nature {}", libelle, n)));
        }
        Ok(Entree { libelle, usages })
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
    crate::norme::livree(norme.table_usages()?.texte())
}

/// La rubrique est-elle au tableau des usages de cette version, quelle que soit la nature ?
pub fn rubrique_connue(norme: &Norme, rubrique: RubriqueDeLaNorme) -> R<bool> {
    Ok(table_de(norme)?.usages.contains_key(&rubrique))
}

/// L'usage de la rubrique pour cette nature dans le tableau de cette version ; `None` si le tableau n'a pas la
/// rubrique.
pub fn usage(norme: &Norme, rubrique: RubriqueDeLaNorme, nature: NatureDeLaDeclaration) -> R<Option<Usage>> {
    Ok(table_de(norme)?.usages.get(&rubrique).map(|e| e.pour(nature)))
}

/// Les rubriques ecrites que cette nature n'admet pas : (rubrique, usage, `None` si le tableau ne la connait pas).
pub fn interdites(texte: &str, nature: NatureDeLaDeclaration, norme: &Norme) -> R<Vec<(RubriqueDeLaNorme, Option<Usage>)>> {
    let mut vues = HashSet::new();
    let mut out = Vec::new();
    for ligne in lignes(texte) {
        let r = strip(match ligne.split_once(',') {
            Some((r, _)) => r,
            None => ligne,
        });
        let Some(r) = RubriqueDeLaNorme::lue(r) else { continue };
        if !vues.insert(r) {
            continue;
        }
        let u = usage(norme, r, nature)?;
        if !matches!(u, Some(Usage::O | Usage::C)) {
            out.push((r, u));
        }
    }
    Ok(out)
}

/// Refuse avant le depot ce que le point de depot refuserait (CST-04).
pub fn verifier(texte: &str, nature: NatureDeLaDeclaration, norme: &Norme) -> R<()> {
    let mauvaises = interdites(texte, nature, norme)?;
    if mauvaises.is_empty() {
        return Ok(());
    }
    let (t, table) = (table_de(norme)?, norme.table_usages()?);
    let libelle = t.natures.get(&nature).ok_or_else(|| socle::echec::livree(table.texte(), &format!("natures.{}", nature)))?;
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
        Lieu { fichier: Some(table.to_string()), ..Lieu::default() },
    ))
}

/// La nature declaree dans le fichier lui-meme (S20.G00.05.001), `None` s'il n'en porte pas ; un code hors de la liste
/// de la norme refuse.
pub fn nature_du_fichier(texte: &str) -> R<Option<NatureDeLaDeclaration>> {
    let Some(code) = lignes(texte).into_iter().find_map(|ligne| ligne.strip_prefix("S20.G00.05.001,").map(|reste| strip(reste).trim_matches('\''))) else {
        return Ok(None);
    };
    NatureDeLaDeclaration::du_code(code).map(Some).ok_or_else(|| {
        socle::refus!("Le fichier porte, en S20.G00.05.001, « {} », qui n'est pas un code de sa liste dans la norme : il ne se controle pas.", code)
    })
}

/// Confronte un fichier produit a sa propre nature, dans le tableau de sa propre version (S10.G00.00.006) : un
/// fichier sans version, ou d'une version que le logiciel ne livre pas, refuse. Un signalement de fin de contrat
/// passe d'abord les deux controles que sa nature ajoute (`fctu::verifier_controles_07` : CCH-14, CCH-15).
pub fn verifier_fichier(texte: &str) -> R<()> {
    let norme = crate::norme::du_fichier(texte)?;
    let Some(nature) = nature_du_fichier(texte)? else { return Ok(()) };
    if nature == NatureDeLaDeclaration::SignalementFinDeContrat {
        crate::fctu::verifier_controles_07(texte).map_err(|e| e.en_classe(socle::echec::Classe::Refuse))?;
    }
    verifier(texte, nature, &norme)
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
    /// 2027.1.2 (fiches 3357 et 3358) : 20 rubriques ajoutees, 2 retirees.
    /// `atelier/reference/norme-dsn/dsn-tableau-des-usages-2027.1.2-20251224.xlsx`, `dsn-journal-maintenance-2027.1.2-20260615.txt`.
    #[test]
    fn les_usages_2027_sont_ceux_du_cahier_2027() {
        use droit::natures::VersionDeLaNorme;
        let (p26, p27) = (Norme::connue(VersionDeLaNorme::P26V01).unwrap(), Norme::connue(VersionDeLaNorme::P27V01).unwrap());
        let (u26, u27) = (table_de(&p26).unwrap(), table_de(&p27).unwrap());
        let (a, b) = (&u26.usages, &u27.usages);
        let retirees: Vec<String> = a.keys().filter(|k| !b.contains_key(*k)).map(|k| k.to_string()).collect();
        let ajoutees: Vec<&RubriqueDeLaNorme> = b.keys().filter(|k| !a.contains_key(*k)).collect();
        assert_eq!(retirees, ["S21.G00.06.003", "S21.G00.11.002"]);
        assert_eq!(ajoutees.len(), 20, "{:?}", ajoutees);
    }
}
