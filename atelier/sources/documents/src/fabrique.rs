//! La fabrique de PDF, dans le meme processus : une description -> un fichier PDF.
//!
//! Le dossier parent est cree, un refus de
//! la fabrique est une `Err` portant sa ligne de francais telle quelle.

use std::path::{Path, PathBuf};

use fabrique_pdf::assemblage::{Assemblage, Piece};
use fabrique_pdf::Description;
use socle::echec::{erreur, Classe, R};

/// LE REFUS DE LA FABRIQUE, LU PAR SA CLASSE (jamais par sa phrase), sur une ligne : un defaut du logiciel (un modele
/// livre qui sort de sa page, une police interne illisible), une piece recue illisible, ou ce que l'appelant corrige.
pub fn echec(r: fabrique_pdf::Refus) -> socle::echec::Echec {
    let classe = match r.classe {
        fabrique_pdf::Classe::DefautDuLogiciel => Classe::DefautDuLogiciel,
        fabrique_pdf::Classe::PieceIllisible => Classe::Illisible,
        fabrique_pdf::Classe::ACorriger => Classe::FabriqueRefus,
    };
    erreur(classe, r.phrase.replace('\n', " ").trim().to_string())
}

/// Ecrit le PDF de `description` dans `chemin` et rend le chemin.
pub fn ecrire(description: &Description, chemin: &Path) -> R<PathBuf> {
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| socle::echec::du_disque(chemin, &e))?;
    }
    let octets = fabriquer_octets(description)?;
    if let Err(e) = std::fs::write(chemin, &octets) {
        return Err(socle::fichiers::effacer_apres(socle::echec::du_disque(chemin, &e), chemin));
    }
    Ok(chemin.to_path_buf())
}

/// Le PDF de `description` ecrit a `chemin` ; un refus de la fabrique dans sa classe (`echec`), un refus du disque
/// dans la sienne.
pub fn ecrire_pdf(description: &Description, chemin: &Path) -> R<()> {
    ecrire(description, chemin).map(|_| ())
}

/// Les octets du PDF de `description`, sans fichier.
pub fn fabriquer_octets(description: &Description) -> R<Vec<u8>> {
    fabrique_pdf::fabriquer(description).map_err(echec)
}

/// Le PDF verrouille en AES-256 (mot de passe utilisateur vide, impression et copie seules).
pub fn verrouiller(octets: &[u8]) -> R<Vec<u8>> {
    fabrique_pdf::chiffrement::verrouiller_avec(octets, |o| socle::temps::aleas(o).map_err(|e| fabrique_pdf::Refus::defaut(e.phrase))).map_err(echec)
}

// ------------------------------------------------------------------ l'assemblage

/// Des pieces (un PDF, ses pages 0-based ou toutes) fusionnees par la fabrique ;
/// un refus est `Refuse`, aucun PDF n'est assemble autrement.
pub fn fusion_pieces(pieces: &[(PathBuf, Option<Vec<usize>>)]) -> R<Vec<u8>> {
    let assemblage = Assemblage {
        format: Some(1),
        pieces: pieces
            .iter()
            .map(|(f, pages)| Piece {
                fichier: f.clone(),
                pages: pages.as_ref().map(|p| p.iter().map(|n| *n as u32 + 1).collect()),
            })
            .collect(),
    };
    let dire = |m: socle::echec::Echec| {
        m.reformuler(|p| format!("Assemblage impossible : la fabrique de PDF refuse la fusion ({}). Aucun PDF n'est assemblé autrement.", p))
    };
    match fabrique_pdf::assemblage::assembler(&assemblage).map_err(|r| dire(echec(r)))? {
        // Une piece verrouillee (un PDF venu d'ailleurs, tel qu'il a ete depose) garde son verrou dans ce qui la porte.
        (octets, true) => verrouiller(&octets).map_err(dire),
        (octets, false) => Ok(octets),
    }
}

/// La fusion de PDF en memoire : chacun va dans un fichier de `tmp` le temps de la fusion.
pub fn fusion_octets(morceaux: &[Vec<u8>], tmp: &Path) -> R<Vec<u8>> {
    let dossier = Temporaire::nouveau_dans(tmp, "assemblage")?;
    let mut pieces = Vec::new();
    for (i, octets) in morceaux.iter().enumerate() {
        let f = dossier.0.join(format!("piece{}.pdf", i));
        std::fs::write(&f, octets).map_err(|e| socle::echec::du_disque(&f, &e).reclasser(Classe::Disque))?;
        pieces.push((f, None));
    }
    fusion_pieces(&pieces)
}

/// Un dossier temporaire, retire quand on a fini (`tempfile.mkdtemp` puis `shutil.rmtree`).
pub struct Temporaire(pub PathBuf);

impl Temporaire {
    pub fn nouveau_dans(parent: &Path, prefixe: &str) -> R<Temporaire> {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = parent.join(format!("{}-{}-{}", prefixe, std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        std::fs::create_dir_all(&p).map_err(|e| socle::echec::du_disque(&p, &e).reclasser(Classe::Disque))?;
        Ok(Temporaire(p))
    }
}

impl Drop for Temporaire {
    fn drop(&mut self) {
        // A la sortie de sa portee, rien ne remonte : un dossier qui ne s'efface pas se note (`effacer_apres_coup`).
        socle::fichiers::effacer_apres_coup(&self.0);
    }
}
