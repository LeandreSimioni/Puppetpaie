//! LA LECTURE D'UN MODELE DE DOCUMENT ET LES VALEURS D'UN DOCUMENT, par marqueur (`droit::marqueurs::Marqueur`, la liste
//! des champs qu'un modele cite, `{{employeur.siret}}`). Un modele -- livre (`donnees/modeles/`) ou pose par la
//! compagnie dans son dossier `gabarits/` -- se lit en morceaux texte et marqueur (`ModeleLu::lire`) : un marqueur que
//! le logiciel ne connait pas se refuse a la lecture, en le nommant. Les valeurs d'un document (`Valeurs`) se posent par
//! marqueur ; la fabrique de PDF, qui ne connait que des noms, les recoit par `pour_la_fabrique`, rangees par nom.

use std::collections::BTreeMap;
use std::path::Path;

use droit::marqueurs::Marqueur;
use socle::echec::{erreur, Classe, R};

/// Ce que la fabrique de PDF pose elle-meme : le contrat remplace (bloc `ANNULE:`), chaque modification d'un avenant
/// (bloc `MODIFICATION:`).
pub const DU_REMPLACEMENT: [Marqueur; 2] = [Marqueur::RemplaceNumero, Marqueur::RemplaceDate];

/// LES VALEURS D'UN DOCUMENT, par marqueur.
pub type Valeurs = BTreeMap<Marqueur, String>;

/// Les valeurs telles que la fabrique de PDF les lit, par leur nom.
pub fn pour_la_fabrique(v: &Valeurs) -> fabrique_pdf::modele::Valeurs {
    v.iter().map(|(k, x)| (k.nom().to_string(), x.clone())).collect()
}

/// Les champs que ce texte cite, dans l'ordre : chacun connu (`Ok`) ou non (`Err`, son nom).
pub fn cites(texte: &str) -> Vec<Result<Marqueur, &str>> {
    let mut sortie = Vec::new();
    let mut i = 0;
    while let Some(k) = texte[i..].find("{{") {
        let a = i + k;
        match fabrique_pdf::modele::marqueur_en(texte, a) {
            Some((cle, fin)) => {
                sortie.push(Marqueur::lire(cle).ok_or(cle));
                i = fin;
            }
            None => i = a + 2,
        }
    }
    sortie
}

/// UN MORCEAU D'UN MODELE : du texte, ou un marqueur.
#[derive(Debug, Clone, PartialEq)]
pub enum Morceau {
    Texte(String),
    Marqueur(Marqueur),
}

/// UN MODELE LU : son texte, tel que la fabrique le met en page, et ses morceaux hors de ses commentaires (les lignes en
/// `#`, que la fabrique saute).
#[derive(Debug, Clone)]
pub struct ModeleLu {
    pub texte: String,
    pub morceaux: Vec<Morceau>,
}

impl ModeleLu {
    /// Le modele `texte` (`nom` : ce que le refus nomme), en morceaux ; un marqueur inconnu se refuse, et le refus les
    /// nomme tous.
    pub fn lire(texte: &str, nom: &str) -> R<ModeleLu> {
        let mut morceaux = Vec::new();
        let mut inconnus: Vec<&str> = Vec::new();
        for l in texte.lines().filter(|l| !l.trim_start().starts_with('#')) {
            let (mut i, mut depuis) = (0, 0);
            while let Some(k) = l[i..].find("{{") {
                let a = i + k;
                match fabrique_pdf::modele::marqueur_en(l, a) {
                    Some((cle, fin)) => {
                        if a > depuis {
                            morceaux.push(Morceau::Texte(l[depuis..a].to_string()));
                        }
                        match Marqueur::lire(cle) {
                            Some(m) => morceaux.push(Morceau::Marqueur(m)),
                            None if !inconnus.contains(&cle) => inconnus.push(cle),
                            None => {}
                        }
                        (i, depuis) = (fin, fin);
                    }
                    None => i = a + 2,
                }
            }
            morceaux.push(Morceau::Texte(format!("{}\n", &l[depuis..])));
        }
        if !inconnus.is_empty() {
            return Err(erreur(Classe::ModeleIncomplet, format!(
                "Le modèle « {} » cite des champs que le logiciel ne connaît pas : {}. Les corriger d'après la liste des champs (Configuration › Modèles de contrat), ou les écrire en texte.",
                nom,
                inconnus.iter().map(|c| format!("{{{{{}}}}}", c)).collect::<Vec<_>>().join(", ")
            )));
        }
        Ok(ModeleLu { texte: texte.to_string(), morceaux })
    }

    /// Le fichier `chemin`, lu (UTF-8) puis `lire`.
    pub fn du_fichier(chemin: &Path) -> R<ModeleLu> {
        let octets = std::fs::read(chemin).map_err(|e| {
            let classe = if e.kind() == std::io::ErrorKind::NotFound { Classe::FichierAbsent } else { Classe::Disque };
            erreur(classe, socle::fichiers::panne_fichier(&e, chemin))
        })?;
        let texte = String::from_utf8(octets).map_err(|e| erreur(Classe::Illisible, format!("'utf-8' codec can't decode bytes: {}", e.utf8_error())))?;
        // Le refus nomme le fichier ; un chemin sans nom de fichier se nomme entier.
        let nom = match chemin.file_name() {
            Some(n) => n.to_string_lossy().into_owned(),
            None => chemin.to_string_lossy().into_owned(),
        };
        ModeleLu::lire(&texte, &nom)
    }

    /// Le modele cite-t-il `m` ?
    pub fn cite(&self, m: Marqueur) -> bool {
        self.morceaux.contains(&Morceau::Marqueur(m))
    }

    /// Son decoupage en titre, preambule, articles et signatures (`fabrique_pdf::modele::lire_modele`).
    pub fn decoupe(&self, nom: &str) -> R<fabrique_pdf::modele::Modele> {
        fabrique_pdf::modele::lire_modele(&self.texte, nom).map_err(crate::fabrique::echec)
    }
}
