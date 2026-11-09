//! `moteur sources [--verifier] [--relever] [--json]` : d'ou viennent les donnees livrees, de quand elles datent, et -- avec
//! `--verifier` -- si la source en ligne a bouge depuis qu'on l'a lue (le travail est dans
//! `sources.rs` et `kali.rs`).
//!
//! UN OUTIL DE L'EDITEUR, lance avant chaque publication ; rien pour une compagnie, qui ne fait que recevoir la mise a
//! jour signee. Il est dans le `moteur` livre, comme `moteur maj --publier`, parce que c'est ce
//! programme-la que le poste de l'editeur fait tourner (il ne compile pas le Rust ; `moteur-atelier` n'est jamais livre),
//! et il est refuse ailleurs que sur ce poste, de la meme facon que la publication (`maj::exiger_editeur` :
//! le reglage de l'installation, `"editeur": true`, et jamais depuis une copie). CACHEE : elle n'apparait pas dans l'aide.
//!
//! ATTENTION : `--verifier` va au RESEAU (chaque source « telechargement », et l'index KALI de la DILA). Lecture seule,
//! hors `atelier/reference/sources/` de l'installation (le depot, sur le poste de l'editeur ; `sources::COPIES`) : le
//! resume des livraisons KALI (`veille-kali.json`), et avec `--relever`, la nouvelle version d'une source a cote de
//! l'ancienne et le XML des textes KALI touches (`kali/`).

use crate::atelier::sources::{EnLigne, EtatSource};

use super::analyse::{Args, Opt, Spec};
use super::rendu::rendre;
use super::Cadre;

pub static SPEC: Spec = Spec {
    commande: "sources",
    positionnels: &[],
    options: &[Opt::drapeau("verifier"), Opt::drapeau("relever"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    if let Err(m) = crate::atelier::publier::exiger_editeur(&cadre.racine, "la verification des sources") {
        eprintln!("{} La verification des sources est un outil de l'editeur, lance avant chaque publication.", m);
        return 1;
    }
    let code = cadre.ctx.code.clone();
    let manifeste = match crate::atelier::sources::manifeste(&code) {
        Ok(m) => m,
        Err(m) => {
            eprintln!("{}", m);
            return 1;
        }
    };
    // `--relever` ne se fait qu'en retournant voir : il emporte `--verifier`.
    let relever = args.drapeau("relever");
    let verifier = args.drapeau("verifier") || relever;
    // Les copies sont les textes du logiciel, pas ceux d'une compagnie : elles vivent dans l'installation.
    let installation = crate::compagnie::maj::installation();
    let liste = if verifier {
        let mut tele = |u: &str| crate::atelier::sources::telecharger(u, 30);
        let mut kali = |u: &str| crate::atelier::sources::telecharger(u, 60).map(|(o, _)| o);
        crate::atelier::sources::verifier(&installation, &manifeste, &cadre.ctx.aujourdhui, &[], relever, &mut tele, &mut kali)
    } else {
        crate::atelier::sources::etats(&installation, &manifeste, &cadre.ctx.aujourdhui)
    };
    let liste = match liste {
        Ok(l) => l,
        Err(m) => {
            eprintln!("{}", m);
            return 1;
        }
    };
    if args.drapeau("json") {
        /// Ce que rend `moteur sources --json` : les sources, et leur nombre.
        #[derive(serde::Serialize)]
        struct Sources<'a> {
            total: usize,
            sources: &'a [EtatSource],
        }
        return rendre("sources", Sources { total: liste.len(), sources: &liste });
    }
    dire!();
    for e in &liste {
        dire!("{}", e.ident);
        dire!("   {}", e.titre);
        dire!("   {} -- version {}, relevee le {} (il y a {} j)", e.editeur, e.version, e.releve_le, e.age_jours);
        dire!("   {}", e.contenu);
        let copie = match (e.copie_conforme, e.copie_presente, e.transcription_presente, &e.transcription) {
            (Some(true), _, _, _) => "copie conforme".to_string(),
            (_, true, _, _) => "COPIE DIFFERENTE du manifeste".to_string(),
            (_, _, Some(true), Some(t)) => format!("original hors du depot public, transcription : {}/{}", crate::atelier::sources::COPIES, t),
            _ => format!("copie absente ({}/{})", crate::atelier::sources::COPIES, e.fichier),
        };
        dire!("   {} ; nourrit : {}", copie, if e.nourrit.is_empty() { "rien (verification)".to_string() } else { e.nourrit.join(", ") });
        if let Some(en_ligne) = e.en_ligne {
            let marque = match en_ligne {
                EnLigne::Inchangee => "OK",
                EnLigne::Changee => "A RELIRE",
                EnLigne::Injoignable => "??",
                EnLigne::Manuelle => "A LA MAIN",
            };
            let detail = e.detail.as_ref().map(|d| format!(" {}", d)).into_iter().collect::<String>();
            dire!("   en ligne : {}{}", marque, detail);
            if let (EnLigne::Manuelle | EnLigne::Changee, Some(page)) = (en_ligne, &e.page_de_veille) {
                dire!("   ouvrir : {}", page);
            }
        }
        dire!();
    }
    if !verifier {
        dire!("`--verifier` retelecharge chaque source et compare ; `--relever` pose la nouvelle version a cote de l'ancienne quand elle a change.");
    }
    0
}
