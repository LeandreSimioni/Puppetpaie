//! LE BRIDAGE. Il n'y a qu'un logiciel, BRIDE ou DEBLOQUE, et pas de version de demonstration a part : c'est toujours
//! le meme logiciel, bride ou pas. Bride, l'accueil ne propose que les
//! compagnies fictives, et son « + » ajoute une FICTIVE AJOUTEE (`compagnies::ajouter`) dont les envois sont
//! refuses (`branchement` : le message) ; debloque, il propose aussi les compagnies reelles. Rien d'autre ne change :
//! le meme moteur, la meme mise a jour (le bridage n'y joue aucun role, `compagnie::maj`).
//!
//! BRIDE TANT QUE RIEN NE DIT QU'IL EST DEBLOQUE : le logiciel n'est jamais debloque par defaut. Une installation neuve -- l'installeur, une archive de la Release decompressee dans un dossier
//! neuf, un `logiciel/` dont un fichier aurait disparu -- part bridee ; seul un fichier qui dit « debloque » la debloque.
//!
//! Deux fichiers le disent, lus ici et nulle part ailleurs :
//! - `.edition.json`, a cote de `logiciel/` : ce que le geste de l'accueil a retenu sur ce poste, `"edition": "bride"` ou
//!   `"debloque"`. C'est le seul chemin du deblocage.
//! - `logiciel/EDITION.json`, `"edition": "bride"` : le PAQUET SOUDE, bride pour toujours, dont les envois sont joues et
//!   qui ne poste a l'editeur que ses incidents ; le geste n'y fait rien. Propre a l'installation : la mise a jour ne le
//!   touche pas (`compagnie::maj`, `PROPRES_A_L_INSTALLATION`), aucune archive n'en porte.
//!
//! Une cle `defaut` dans `logiciel/EDITION.json` ne se lit pas, et l'installeur ne pose aucun fichier.
//!
//! Le geste ecrit `"bride"` ou `"debloque"`. `"demo"` se lit aussi « bride », et `"complete"` « debloque » : des
//! installations posees et debloquees portent ces mots ; les lire autrement les briderait et bloquerait leurs envois.
//! C'est le contrat avec les installations posees, comme la forme de la mise a jour.
//!
//! L'INSTALLATION dont on parle : celle du logiciel qui tourne (`compagnies::base()`), ou celle qu'on nomme --
//! l'administrateur et la liste des compagnies lisent l'installation qu'on leur donne.

use std::path::Path;

const FICHIER_GESTE: &str = ".edition.json";
const FICHIER_INSTALLATION: &str = "EDITION.json";

/// L'ETAT DU BRIDAGE : bride ou debloque. Il s'ecrit `bride`, `debloque` ; `demo` se lit aussi « bride » et `complete`
/// « debloque », les mots que portent des installations posees (en tete du module).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EtatDuBridage {
    #[serde(alias = "demo")]
    Bride,
    #[serde(alias = "complete")]
    Debloque,
}

impl EtatDuBridage {
    /// Le mot tel qu'il s'ecrit.
    pub fn texte(self) -> &'static str {
        match self {
            EtatDuBridage::Bride => "bride",
            EtatDuBridage::Debloque => "debloque",
        }
    }
}

impl std::fmt::Display for EtatDuBridage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.texte())
    }
}

/// CE QUE DIT UN FICHIER DU BRIDAGE (`.edition.json`, `logiciel/EDITION.json`) : le mot (`edition`), et ce qu'il est
/// (`quoi`, que le geste ecrit pour qui l'ouvre).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Edition {
    edition: EtatDuBridage,
    #[serde(skip_serializing_if = "Option::is_none")]
    quoi: Option<String>,
}

/// Ce que le fichier `p` dit du bridage ; `None` s'il ne le dit pas (absent, ou illisible : un autre mot ne se lit pas).
/// Le bridage tient a ce qui est dit : rien de ce qui ne se lit pas ne debloque.
fn dit(p: &Path) -> Option<EtatDuBridage> {
    if !p.exists() {
        return None;
    }
    match socle::forme::lire_fichier::<Edition>(p, "Le fichier du bridage ne se lit pas.") {
        Ok(e) => Some(e.edition),
        // Il ne debloque pas ; le developpeur le saura (le fil du moteur).
        Err(illisible) => {
            crate::diagnostic::noter_moteur(&format!("bridage : {}", illisible.phrase));
            None
        }
    }
}

/// Le PAQUET SOUDE : `logiciel/EDITION.json` dit `"edition": "bride"` (ou `"demo"`). Bride pour toujours ; ses envois
/// sont joues.
pub fn paquet_soude() -> bool {
    paquet_soude_de(&crate::installation::base())
}

/// Le paquet soude de l'installation `inst` (le dossier qui porte `logiciel/`).
pub fn paquet_soude_de(inst: &Path) -> bool {
    dit(&inst.join("logiciel").join(FICHIER_INSTALLATION)) == Some(EtatDuBridage::Bride)
}

/// Bride ou debloque, pour le logiciel qui tourne (`est_bride_de`).
pub fn est_bride() -> bool {
    est_bride_de(&crate::installation::base())
}

/// Bride ou debloque, pour l'installation `inst` : le paquet soude reste bride ; sinon ce que le geste a retenu sur le
/// poste (`.edition.json`) ; SINON BRIDE -- rien d'autre ne debloque.
pub fn est_bride_de(inst: &Path) -> bool {
    if paquet_soude_de(inst) {
        return true;
    }
    dit(&inst.join(FICHIER_GESTE)) != Some(EtatDuBridage::Debloque)
}

/// LE REFUS D'UN ENVOI DEPUIS UNE FICTIVE AJOUTEE, mot pour mot : le meme pour tout geste
/// qui ferait partir quelque chose vers un organisme ou une personne. Sa classe : `EnvoiBride` (la page la reconnait et
/// montre la phrase dans sa boite, `js/tuyau.js`).
pub const REFUS_D_ENVOI: &str = "L'envoi n'est pas possible depuis la version démo. Pour 2027, et afin d'offrir un service de qualité, le nombre d'utilisateurs est restreint. N'hésitez pas à contacter Léandre (puppetpaie@gmail.com) avec une courte présentation et votre motivation pour débloquer le logiciel et faire partie des premiers utilisateurs.";

/// LE BRANCHEMENT DE LA COMPAGNIE `racine` (`envois::branchement`), choisi ici et nulle part ailleurs :
/// - une FICTIVE AJOUTEE (`compagnies::est_ajoutee`) n'envoie rien : le message, `REFUS_D_ENVOI` ;
/// - une compagnie FICTIVE (son marqueur, `compagnies::est_fictive`), ou toute compagnie du PAQUET SOUDE
///   (`paquet_soude`) : le simulateur ;
/// - toute autre : le monde. Le logiciel simplement bride ne joue rien.
///
/// Chaque envoi vers un organisme ou une personne l'ouvre avant que rien ne s'ecrive (`Branchement::ouvrir`). Ce qui va
/// a l'editeur (incidents, « Un probleme, une idee ? ») et la verification d'un mot de passe ne sont pas des envois.
pub fn branchement(racine: &Path) -> socle::echec::R<envois::branchement::Branchement> {
    use crate::installation::{est_ajoutee, est_fictive};
    use envois::branchement::{Branchement, Refus};
    Ok(if est_ajoutee(racine)? {
        Branchement::Message(Refus { classe: socle::echec::Classe::EnvoiBride, phrase: REFUS_D_ENVOI })
    } else if paquet_soude() || est_fictive(racine)? {
        Branchement::Simulateur
    } else {
        Branchement::Monde
    })
}

/// Ce qu'il est, pour le logiciel qui tourne (le journal des incidents, la description du poste).
pub fn etat() -> EtatDuBridage {
    if est_bride() {
        EtatDuBridage::Bride
    } else {
        EtatDuBridage::Debloque
    }
}

/// L'installation `inst` bridee en toutes lettres, comme le geste de l'accueil la laisse (`.edition.json`) : celle du rejeu
/// d'un scenario (`routes::scenario`).
pub fn brider(inst: &Path) -> socle::echec::R<()> {
    socle::ecriture::ecrire_json(&inst.join(FICHIER_GESTE), &Edition { edition: EtatDuBridage::Bride, quoi: None }, 2)
}

/// LE GESTE SECRET DE L'ACCUEIL (la route `basculer`), sur l'installation `inst` : bride <-> debloque, retenu sur le
/// poste. Sans effet sur un paquet soude.
pub fn basculer_de(inst: &Path) -> socle::echec::R<()> {
    if paquet_soude_de(inst) {
        return Ok(());
    }
    let nouveau = if est_bride_de(inst) { EtatDuBridage::Debloque } else { EtatDuBridage::Bride };
    socle::ecriture::ecrire_json(
        &inst.join(FICHIER_GESTE),
        &Edition {
            edition: nouveau,
            quoi: Some("Le bridage de PuppetPaie sur ce poste : bride (l'accueil ne propose que les compagnies fictives) ou debloque. Change par le geste de l'accueil.".to_string()),
        },
        2,
    )
}
