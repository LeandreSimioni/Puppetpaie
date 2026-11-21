//! `moteur-atelier extraire spst <urssaf-sst-dpae-AAAAMMJJ.csv> <sortie.json>` : les services de prevention et de sante au
//! travail, pour `logiciel/donnees/spst-base.json` (que lit `declarations::spst`).
//!
//! LA SOURCE : le referentiel « sst_dpae » de l'Urssaf Caisse nationale, sur son portail open data
//! (https://open.urssaf.fr/explore/dataset/sst_dpae/), depose dans `atelier/reference/sources/urssaf-sst-dpae/` (son
//! `LISEZ-MOI.md` ; `logiciel/donnees/sources.json`, clef `urssaf-sst-dpae`, dit sa date et son condensat). La page du
//! jeu le dit la liste « qui doit être utilisée dans le cadre de la Déclaration Préalable à l'Embauche (DPAE) » ; la
//! fiche consigne 3370 de net-entreprises, celle des valeurs de la DSN (« colonne « mt_lib_court » du référentiel »).
//!
//! UNE LIGNE PAR SERVICE. Le referentiel repete un service sous chaque Urssaf qui le connait (`mt_code_ur`) : on en tire
//! un service par identifiant (`mt_lib_court`), son nom (`mt_lib`, sans les chiffres de l'identifiant qui le precedent :
//! « 176 - THALIE SANTE » donne « THALIE SANTE ») et sa commune (`mt_bur_dist`), tels que le referentiel les ecrit. Un
//! identifiant qui porte deux noms ou deux communes refuse le releve : la source a bouge, elle se relit.
//!
//! La source et la sortie se nomment.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use super::urssaf::Table;
use super::{date_du_releve, Releve};
use droit::fiche::IdentifiantSpst;
use socle::date::Jour;
use socle::echec::R;
use socle::fichiers::{repr, NomDeFichier};
use socle::texte::strip;

/// Le referentiel `spst-base.json`, apres `quoi` et `note`.
#[derive(Serialize)]
struct Spst {
    services: Vec<Service>,
}

/// Un service : son identifiant, son nom, sa commune.
#[derive(Serialize)]
struct Service {
    code: IdentifiantSpst,
    nom: String,
    commune: String,
}

/// Le nom du service `code` dans `mt_lib` : sans les chiffres de son identifiant, ni le tiret qui les suit ; la medecine
/// interne (MT01) n'en porte pas.
fn nom(code: &IdentifiantSpst, lib: &str, fichier: &str) -> R<String> {
    let nom = if code.interne() {
        lib
    } else {
        let chiffres = &code.as_str()[2..];
        lib.strip_prefix(chiffres)
            .map(str::trim_start)
            .and_then(|r| r.strip_prefix('-'))
            .map(str::trim)
            .ok_or_else(|| refus!("{} : le nom {} de {} ne commence pas par « {} - » ; relire la source.", fichier, repr(lib), code, chiffres))?
    };
    if nom.is_empty() {
        return Err(refus!("{} : {} n'a pas de nom ; relire la source.", fichier, code));
    }
    Ok(nom.to_string())
}

/// Le releve : un service par identifiant, dans l'ordre des identifiants.
pub fn relever(csv: &Path, jour: &Jour) -> R<Releve> {
    let fichier = NomDeFichier::du(csv)?.to_string();
    let octets = std::fs::read(csv).map_err(|e| refus!("{} illisible ({})", csv.display(), e))?;
    let texte = String::from_utf8(octets).map_err(|e| refus!("{} n'est pas en UTF-8 ({})", csv.display(), e))?;
    let sans_bom = texte.strip_prefix('\u{feff}').unwrap_or(&texte);
    let t = Table::lire(sans_bom, ';');
    let mut services: BTreeMap<IdentifiantSpst, (String, String)> = BTreeMap::new();
    for l in &t.rangs {
        let code = IdentifiantSpst::lire(strip(t.exige(l, "mt_lib_court", &fichier)?))?;
        let nom = nom(&code, strip(t.exige(l, "mt_lib", &fichier)?), &fichier)?;
        let commune = strip(t.exige(l, "mt_bur_dist", &fichier)?).to_string();
        match services.get(&code) {
            Some(deja) if *deja != (nom.clone(), commune.clone()) => {
                return Err(refus!(
                    "{} : {} se dit « {}, {} » et « {}, {} » ; relire la source.",
                    fichier, code, deja.0, deja.1, nom, commune
                ));
            }
            Some(_) => {}
            None => {
                services.insert(code, (nom, commune));
            }
        }
    }
    let bilan = vec![format!("{} services, de {} lignes", services.len(), t.rangs.len())];
    let quoi = "Les services de prevention et de sante au travail (SPST) du referentiel « sst_dpae » de l'Urssaf, un par \
                identifiant (mt_lib_court : MT et trois chiffres, ou MT01 pour la medecine interne a l'etablissement), \
                que la DSN 2027 porte en S21.G00.11.025 et dont la DPAE tire son C_MT_DCL (MT999 pour la medecine \
                interne), avec le nom du service et sa commune.";
    let note = format!(
        "Une source relevee et datee -- `moteur sources` : urssaf-sst-dpae ({}), deposee dans \
         atelier/reference/sources/urssaf-sst-dpae/. Extrait par `moteur extraire spst` le {} ; ne pas editer a la main.",
        fichier,
        date_du_releve(jour)
    );
    let corps = Spst { services: services.into_iter().map(|(code, (nom, commune))| Service { code, nom, commune }).collect() };
    Ok(Releve::nouveau(quoi, note, corps, 2, true, bilan))
}
