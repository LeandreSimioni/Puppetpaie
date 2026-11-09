//! LES OUTILS DE L'ATELIER SUR UNE COMPAGNIE : des enquetes et des gestes d'editeur qui ne sont pas des gestes de l'ecran,
//! dans le binaire de l'atelier seulement (`--features atelier`, `moteur-atelier`). Ils prennent le cadre des commandes
//! (`commandes::Cadre` : la compagnie, le jour, la demande du mot de passe) et appellent les fonctions du moteur ; aucun
//! ne recopie une regle.
//!
//! - `pas` : les taux de prelevement a la source connus, `--amorcer` depuis les DSN deposees, `--demander` les
//!   signalements d'amorcage (nature 08), ecrits sans etre deposes ;
//! - `aer` : ranger de nouveau les attestations France Travail recues, `--forcer` pour signer malgre un ecart ;
//! - `journal --dpae` : reprendre au journal les DPAE que les certificats de l'Urssaf attestent ;
//! - `retours` : relever les retours d'une plage, d'un flux, sur la plate-forme de l'editeur (l'enquete) ;
//! - `depot-dsn` : un depot de TEST a un ordre donne (`--ordre`) ou sur la plate-forme de l'editeur.

pub mod aer;
pub mod depot_dsn;
pub mod journal;
pub mod pas;
pub mod retours;

/// Le code de sortie d'un outil : le sien, ou, a l'erreur, sa phrase et le code 2.
pub(crate) fn code(r: Result<i32, socle::echec::Echec>) -> i32 {
    match r {
        Ok(c) => c,
        Err(e) => {
            dire!("! {}", e.message);
            2
        }
    }
}

/// Le nom du fichier `p`, sans son dossier ; un chemin qui ne finit pas par un nom (`..`) se dit entier.
pub(crate) fn nom(p: &std::path::Path) -> String {
    match p.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => p.display().to_string(),
    }
}

/// La valeur de l'option `nom`, que sa `Spec` pose par defaut : l'analyse l'a toujours ecrite.
pub(crate) fn avec_defaut<'a>(args: &'a crate::commandes::analyse::Args, nom: &str) -> Result<&'a str, socle::echec::Echec> {
    args.texte(nom).ok_or_else(|| socle::echec::Echec { classe: Some(socle::echec::Classe::Illisible), message: format!("--{} n'a pas de valeur : sa `Spec` lui en donne une par defaut.", nom) })
}

/// Un texte facultatif tel que l'outil l'affiche : `None` quand il n'est pas dit.
pub(crate) fn dit(t: Option<&str>) -> &str {
    match t {
        Some(t) => t,
        None => "None",
    }
}
