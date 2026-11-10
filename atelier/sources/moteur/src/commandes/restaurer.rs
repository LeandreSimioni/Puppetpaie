//! `moteur restaurer [DOSSIER] [--racine-drive NOM] [--json]` : redescendre le dossier prive du Drive. La decision est
//! dans `restauration.rs` ; ici, l'affichage.
//!
//! Le Drive est celui que la compagnie de ce poste a relie (dans la fenetre, Configuration > Mots de passe > Lier le Drive) : le client lit son jeton dans le dossier
//! des donnees du poste. `DOSSIER` est ou ecrire ; par defaut, ce meme dossier des donnees (le cas d'un poste neuf : relier le
//! Drive, puis restaurer). Dans `moteur-atelier`, hors compagnie fictive, la commande exige le faux Google, comme `sauvegarder`.
//!
//! Codes : 0 tout est revenu ; 1 des incidents (un fichier, une empreinte) ; 2 un refus, rien n'a ete ecrit.

use std::path::PathBuf;

use super::actes::refuser_si_pas_redirige;
use super::actes::Organisme;
use super::analyse::{Args, Opt, Pos, Spec};
use super::Cadre;
use compagnie::{restauration, sauvegarde};

pub static SPEC: Spec = Spec {
    commande: "restaurer",
    positionnels: &[Pos::facultatif("dossier", None)],
    options: &[Opt::avec_defaut("racine-drive", sauvegarde::RACINE_DRIVE), Opt::drapeau("json")],
};

/// Ce que `moteur restaurer --json` ecrit d'un refus : sa classe et sa phrase.
#[derive(serde::Serialize)]
struct Refuse<'a> {
    ok: bool,
    erreur: ErreurDite<'a>,
}

#[derive(serde::Serialize)]
struct ErreurDite<'a> {
    #[serde(rename = "type")]
    classe: Option<socle::echec::Classe>,
    message: &'a str,
}

/// Ce que `moteur restaurer --json` ecrit d'une restauration : `ok` quand tout est revenu.
#[derive(serde::Serialize)]
struct Restauree {
    ok: bool,
    resultat: restauration::RestaurationDite,
}

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    let json = args.drapeau("json");
    let refus = |classe: Option<socle::echec::Classe>, message: &str| -> i32 {
        if json {
            dire!("{}", socle::ecriture::json_en_ligne(&Refuse { ok: false, erreur: ErreurDite { classe, message } }));
        } else {
            dire!("! {}", message);
        }
        2
    };
    if let Err(e) = refuser_si_pas_redirige(cadre, Organisme::Google, "la restauration depuis le Drive") {
        return refus(Some(socle::echec::Classe::Refuse), &String::from(e));
    }
    if !routes::coffre::autorise(&cadre.racine) {
        return refus(Some(socle::echec::Classe::Google), "Compte Google non autorise : relier d'abord le Drive de la compagnie : la route google-autoriser, ou dans la fenetre Configuration > Mots de passe > Lier le Drive.");
    }
    let racine_drive = match args.exige("racine-drive") {
        Ok(r) => r,
        Err(e) => return refus(e.classe, &String::from(e)),
    };
    // `DOSSIER` : ou ecrire ; sans lui, le dossier des donnees de la commande (voir l'en-tete).
    let cible = match args.texte("dossier").filter(|d| !d.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => cadre.racine.clone(),
    };
    let mut client = match routes::coffre::client_drive(&cadre.racine) {
        Ok(c) => c,
        Err(e) => return refus(e.classe, &e.message),
    };
    if !json {
        dire!("Restauration de Drive/{}/{} vers {}", racine_drive, sauvegarde::DOSSIER_DRIVE, cible.display());
    }
    let r = match restauration::restaurer(&cible, racine_drive, &mut client) {
        Ok(r) => r,
        Err(e) => return refus(e.classe, &e.message),
    };
    let reste = restauration::a_faire(&compagnie::coffre::Coffre::du_poste(&cible, ""));
    let code = if r.incidents.is_empty() { 0 } else { 1 };
    if json {
        dire!("{}", socle::ecriture::json_en_ligne(&Restauree { ok: code == 0, resultat: r.dite(reste) }));
        return code;
    }
    for f in &r.redescendus {
        dire!("  redescendu {}", f.relatif);
    }
    dire!("\n{}", r.resume());
    if r.ecartes > 0 {
        dire!("{} fichier(s) du Drive laisse(s) la-bas : ce que la sauvegarde n'envoie pas.", r.ecartes);
    }
    for i in &r.incidents {
        dire!("  ! {}", i);
    }
    if !reste.is_empty() {
        dire!("\nCe qu'il reste a faire a la main :");
        for l in &reste {
            dire!("  {}", l);
        }
    }
    code
}
