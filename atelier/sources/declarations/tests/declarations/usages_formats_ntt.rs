//! Les controles avant depot (regle 7) contre `usages-formats-ntt.json` : le tableau des usages (P26V01), le moteur
//! d'expressions des formats (motif par motif), la mauvaise valeur et les caracteres refuses d'une rubrique
//! (`atelier/reference/norme-dsn/dsn-cahier-technique-2026.1-usages.txt`, `dsn-cahier-technique-2026.1-formats.txt`).
//! `PUPPETPAIE_REECRIRE_ORACLES=1` reecrit l'oracle.

use declarations::formats::{self, Regex};
use declarations::usages;
use serde::{Deserialize, Serialize, Serializer};

const ORACLE: &str = "donnees/usages-formats-ntt.json";
use droit::nature_declaration::NatureDeLaDeclaration;

/// L'oracle : l'usage de chaque rubrique par nature (dans l'ordre du fichier), des motifs et ce qu'ils admettent, des
/// rubriques avec une valeur, sa mauvaise valeur et ses caracteres refuses.
#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct Oracle {
    usages: Usages,
    regex: Vec<(String, Vec<(String, bool)>)>,
    rubriques: Vec<(String, String, String, Vec<String>)>,
}

/// Les usages, rubrique par rubrique, dans l'ordre du fichier : une lettre par nature, `.` si le tableau ne la connait pas.
#[derive(PartialEq, Debug)]
struct Usages(Vec<(String, String)>);

impl<'de> Deserialize<'de> for Usages {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        socle::forme::en_ordre(d).map(Usages)
    }
}

impl Serialize for Usages {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (r, codes) in &self.0 {
            m.serialize_entry(r, codes)?;
        }
        m.end()
    }
}

fn chemin() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join(ORACLE)
}

fn lu() -> Oracle {
    socle::forme::lire_fichier(&chemin(), "L'oracle des usages et des formats ne se lit pas.").unwrap_or_else(|e| panic!("{}", e.phrase))
}

fn reecrire() -> bool {
    std::env::var("PUPPETPAIE_REECRIRE_ORACLES").is_ok_and(|v| v == "1")
}

/// Compare ce que le moteur rend a l'oracle ; sous `PUPPETPAIE_REECRIRE_ORACLES=1`, l'ecrit a la place (en une ligne,
/// comme il est livre). Les tests de ce fichier se partagent l'oracle : chacun relit, remplace sa partie, reecrit.
fn comparer(obtenu: Oracle) {
    if reecrire() {
        std::fs::write(chemin(), socle::ecriture::json_en_ligne(&obtenu)).unwrap();
        return;
    }
    let attendu = lu();
    assert!(obtenu.usages == attendu.usages, "usages : le tableau differe de l'oracle");
    assert!(obtenu.regex == attendu.regex, "regex : les expressions different de l'oracle");
    assert!(obtenu.rubriques == attendu.rubriques, "rubriques : les controles different de l'oracle");
}

/// L'oracle est celui des tables 2026 : ses comparaisons se font dans la version P26V01 (la table se choisit par version
/// de la norme).
fn p26() -> declarations::norme::Norme {
    declarations::norme::Norme::connue("P26V01").unwrap()
}

fn usages_calcules(o: &Oracle) -> Usages {
    Usages(
        o.usages
            .0
            .iter()
            .map(|(r, _)| {
                let codes = NatureDeLaDeclaration::TOUS
                    .iter()
                    .map(|nat| match usages::usage(&p26(), r, *nat).unwrap() {
                        Some(u) => u.lettre(),
                        None => ".",
                    })
                    .collect();
                (r.clone(), codes)
            })
            .collect(),
    )
}

fn regex_calcules(o: &Oracle) -> Vec<(String, Vec<(String, bool)>)> {
    o.regex
        .iter()
        .map(|(motif, cas)| {
            let re = Regex::compiler(motif).unwrap();
            (motif.clone(), cas.iter().map(|(x, _)| (x.clone(), re.fullmatch(x))).collect())
        })
        .collect()
}

fn rubriques_calculees(o: &Oracle) -> Vec<(String, String, String, Vec<String>)> {
    o.rubriques
        .iter()
        .map(|(r, v, _, _)| {
            let mauvaise = formats::mauvaise_en(&p26(), r, v).unwrap();
            let refuses = formats::caracteres_refuses_en(&p26(), r, v).unwrap().iter().map(|c| c.to_string()).collect();
            (r.clone(), v.clone(), mauvaise, refuses)
        })
        .collect()
}

#[test]
fn tableau_des_usages() {
    let o = lu();
    assert!(o.usages.0.len() * NatureDeLaDeclaration::TOUS.len() > 4000);
    let usages = usages_calcules(&o);
    comparer(Oracle { usages, ..o });
}

#[test]
fn expressions_des_formats() {
    let o = lu();
    let regex = regex_calcules(&o);
    let n: usize = regex.iter().map(|(_, cas)| cas.len()).sum();
    assert!(n > 4000, "{}", n);
    comparer(Oracle { regex, ..o });
}

#[test]
fn toutes_les_expressions_de_la_table_se_compilent() {
    // Les tables de toutes les versions livrees (`norme-dsn-base.json`) : celle de 2027 porte des expressions que celle
    // de 2026 n'avait pas (S89.G00.88.008, entiere en 2027).
    for norme in declarations::norme::versions().unwrap() {
        let t = formats::table_de(&norme).unwrap();
        let mut n = 0;
        for f in t.formats.values() {
            for c in f.csl.iter().flatten() {
                Regex::compiler(c).unwrap();
                n += 1;
            }
        }
        assert!(n > 100, "{} : {}", norme.version, n);
    }
}

#[test]
fn mauvaise_valeur_et_caracteres_refuses() {
    let o = lu();
    let rubriques = rubriques_calculees(&o);
    assert!(rubriques.len() > 5000);
    comparer(Oracle { rubriques, ..o });
}
