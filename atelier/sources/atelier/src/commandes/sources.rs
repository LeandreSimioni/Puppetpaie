//! `moteur-atelier sources [--verifier] [--relever] [--json]` : d'ou viennent les donnees livrees, de quand elles datent, et -- avec
//! `--verifier` -- si la source en ligne a bouge depuis qu'on l'a lue (le travail est dans
//! `sources.rs` et `kali.rs`).
//!
//! UN OUTIL DE L'ATELIER, lance avant chaque publication ; rien pour une compagnie, qui ne fait que recevoir la mise a
//! jour signee.
//!
//! ATTENTION : `--verifier` va au RESEAU (chaque source « telechargement », et l'index KALI de la DILA). Lecture seule,
//! hors `atelier/reference/sources/` de l'installation (le depot ; `sources::COPIES`) : le
//! resume des livraisons KALI (`veille-kali.json`), et avec `--relever`, la nouvelle version d'une source a cote de
//! l'ancienne et le XML des textes KALI touches (`kali/`).

use crate::sources::{EnLigne, EtatSource};

use moteur::commandes::analyse::{Args, Opt, Spec};
use moteur::commandes::rendu::rendre;
use moteur::commandes::Cadre;

pub static SPEC: Spec = Spec {
    commande: "sources",
    positionnels: &[],
    options: &[Opt::drapeau("verifier"), Opt::drapeau("relever"), Opt::drapeau("json")],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let code = cadre.ctx.code.clone();
    let manifeste = match crate::sources::manifeste(&code) {
        Ok(m) => m,
        Err(m) => {
            eprintln!("{}", m.en_lignes());
            return 1;
        }
    };
    // `--relever` ne se fait qu'en retournant voir : il emporte `--verifier`.
    let relever = args.drapeau("relever");
    let verifier = args.drapeau("verifier") || relever;
    // Les copies sont les textes du logiciel, pas ceux d'une compagnie : elles vivent dans l'installation.
    let installation = compagnie::maj::installation();
    let liste = if verifier {
        let mut tele = |u: &socle::garde_reseau::Url| crate::sources::telecharger(u, 30);
        let mut kali = |u: &str| socle::garde_reseau::Url::lire(u).and_then(|u| crate::sources::telecharger(&u, 60)).map(|(o, _)| o);
        crate::sources::verifier(&installation, &manifeste, &cadre.ctx.aujourdhui, &[], relever, &mut tele, &mut kali)
    } else {
        crate::sources::etats(&installation, &manifeste, &cadre.ctx.aujourdhui)
    };
    let liste = match liste {
        Ok(l) => l,
        Err(m) => {
            eprintln!("{}", m.en_lignes());
            return 1;
        }
    };
    if args.drapeau("json") {
        /// Ce que rend `moteur-atelier sources --json` : les sources, et leur nombre.
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
        dire!("   {} -- version {}, relevee le {} (il y a {} j)", e.editeur, e.version, e.releve_le.iso(), e.age_jours);
        dire!("   {}", e.contenu);
        let copie = match (e.copie_conforme, e.copie_presente, e.transcription_presente, &e.transcription) {
            (Some(true), _, _, _) => "copie conforme".to_string(),
            (_, true, _, _) => "COPIE DIFFERENTE du manifeste".to_string(),
            (_, _, Some(true), Some(t)) => format!("original hors du depot public, transcription : {}/{}", crate::sources::COPIES, t),
            _ => format!("copie absente ({}/{})", crate::sources::COPIES, e.fichier),
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
