//! Les aides des tests des declarations : les donnees livrees, et les anomalies d'une fiche tapee, lue par sa porte
//! (`droit::fiche::FicheSaisie::lire_cases`).

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

/// Les donnees livrees (`logiciel/donnees`).
pub fn code_livre() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../logiciel/donnees")
}

/// Le fichier `rel` sous `racine`, lu par la porte du socle, dans son type.
pub fn lire<T: DeserializeOwned>(racine: &Path, rel: &str) -> T {
    let chemin = racine.join(rel);
    let texte = std::fs::read_to_string(&chemin).unwrap_or_else(|e| panic!("{} illisible : {}", chemin.display(), e));
    socle::forme::lire_texte(&texte, &format!("{} ne se lit pas.", chemin.display()))
        .unwrap_or_else(|e| panic!("{} : {}", chemin.display(), e.phrase))
}

/// LES ANOMALIES DE LA FICHE `nom` TAPEE de ces cases-la (des textes), lue par sa porte (`lire_cases`) : les fautes de
/// ses cases (`champs::des_fautes`), puis ce que la table en dit (`champs::verifier_fiche`).
pub fn anomalies(nom: droit::fiche::NomDeFiche, cases: &[(&str, &str)]) -> Vec<declarations::champs::Anomalie> {
    use declarations::champs::{des_fautes, verifier_fiche, FicheAJuger};
    use droit::fiche::FicheSaisie;
    let fiche: std::collections::BTreeMap<&str, &str> = cases.iter().copied().collect();
    let o = socle::forme::Objet::du_texte(&socle::ecriture::json_en_ligne(&fiche), "illisible", "pas un objet").unwrap_or_else(|e| panic!("{}", e.phrase));
    let echec = |e: socle::echec::Echec| -> ! { panic!("{}", e.phrase) };
    let (fautes, autres) = match nom {
        droit::fiche::NomDeFiche::Entreprise => {
            let (f, fautes) = droit::fiche::ChampsEntreprise::lire_cases(&o).unwrap_or_else(|e| echec(e));
            (fautes, verifier_fiche(FicheAJuger::Entreprise(&f)))
        }
        droit::fiche::NomDeFiche::Salarie => {
            let (f, fautes) = droit::fiche::ChampsSalarie::lire_cases(&o).unwrap_or_else(|e| echec(e));
            (fautes, verifier_fiche(FicheAJuger::Salarie(&f)))
        }
        droit::fiche::NomDeFiche::Spectacle => {
            let (f, fautes) = droit::fiche::ChampsSpectacle::lire_cases(&o).unwrap_or_else(|e| echec(e));
            (fautes, verifier_fiche(FicheAJuger::Spectacle(&f)))
        }
    };
    let mut out = des_fautes(nom, &fautes).unwrap_or_else(|e| echec(e));
    out.extend(autres.unwrap_or_else(|e| echec(e)));
    out
}
