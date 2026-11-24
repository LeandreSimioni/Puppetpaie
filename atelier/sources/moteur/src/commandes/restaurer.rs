//! `moteur restaurer [DOSSIER] [--racine-drive NOM] [--dossier-drive ID] [--json]` : redescendre une compagnie du
//! Drive. La decision est dans `restauration.rs`, le meme chemin que la reprise a l'accueil (`routes::accueil::drive`) ;
//! ici, l'affichage. `--dossier-drive` nomme le dossier de la compagnie (son identifiant, tel que `drive-etat` ou
//! `drive-compagnies` le rend) ; sans lui, le seul dossier qui se reprend sous la racine.
//!
//! Le Drive est celui que la compagnie de ce poste a relie (dans la fenetre, Configuration > Mots de passe > Lier le Drive) : le client lit son jeton dans le dossier
//! des donnees du poste. `DOSSIER` est ou ecrire ; par defaut, ce meme dossier des donnees (le cas d'un poste neuf : relier le
//! Drive, puis restaurer). Dans `moteur-atelier`, hors compagnie fictive, la commande exige le faux Google, comme `sauvegarder`.
//!
//! Codes : 0 tout est revenu ; 1 des fichiers a retenter (relancer la commande) ou qui ne descendront pas ainsi ; 2 un
//! refus, rien n'a ete ecrit.

use std::path::PathBuf;
use std::process::ExitCode;

use super::actes::refuser_si_pas_redirige;
use super::actes::Organisme;
use super::analyse::{Args, Opt, Pos, Spec};
use super::Cadre;
use compagnie::{restauration, sauvegarde, sur_le_drive};

pub static SPEC: Spec = Spec {
    commande: "restaurer",
    positionnels: &[Pos::facultatif("dossier", None)],
    options: &[Opt::avec_defaut("racine-drive", sauvegarde::RACINE_DRIVE), Opt::valeur("dossier-drive"), Opt::drapeau("json")],
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

pub fn executer(cadre: &Cadre, args: &Args) -> ExitCode {
    let json = args.drapeau("json");
    let refus = |classe: Option<socle::echec::Classe>, message: &str| -> ExitCode {
        if json {
            dire!("{}", socle::ecriture::json_en_ligne(&Refuse { ok: false, erreur: ErreurDite { classe, message } }));
        } else {
            dire!("! {}", message);
        }
        ExitCode::from(2)
    };
    if let Err(e) = refuser_si_pas_redirige(cadre, Organisme::Google, "la restauration depuis le Drive") {
        return refus(Some(socle::echec::Classe::Refuse), &e.en_lignes());
    }
    if !routes::coffre::autorise(&cadre.racine) {
        return refus(Some(socle::echec::Classe::Google), "Compte Google non autorise : relier d'abord le Drive de la compagnie : la route google-autoriser, ou dans la fenetre Configuration > Mots de passe > Lier le Drive.");
    }
    let racine_drive = match args.exige("racine-drive") {
        Ok(r) => r,
        Err(e) => return refus(e.classe, &e.en_lignes()),
    };
    // `DOSSIER` : ou ecrire ; sans lui, le dossier des donnees de la commande (voir l'en-tete).
    let cible = match args.texte("dossier").filter(|d| !d.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => cadre.racine.clone(),
    };
    let mut client = match routes::coffre::client_drive(&cadre.racine) {
        Ok(c) => c,
        Err(e) => return refus(e.echec.classe, &e.echec.en_lignes()),
    };
    // LE DOSSIER DU DRIVE : celui que `--dossier-drive` nomme ; sans lui, le seul qui se reprend sous la racine. Plusieurs :
    // la commande ne choisit pas, elle les dit.
    let vus = match sur_le_drive::voir(&mut client, racine_drive) {
        Ok(v) => v,
        Err(e) => return refus(e.echec.classe, &e.echec.en_lignes()),
    };
    // Dit, `--dossier-drive` se lit tel quel : vide, il se refuse (`IdDrive::lire`), il ne vaut pas absent.
    let depuis = match args.texte("dossier-drive") {
        Some(d) => match envois::drive::IdDrive::lire(d).map_err(envois::drive::ErreurGoogle::from).and_then(|id| sur_le_drive::le_dossier(vus, &id, racine_drive)) {
            Ok(v) => v,
            Err(e) => return refus(e.echec.classe, &e.echec.en_lignes()),
        },
        None => {
            let mut a_reprendre: Vec<sur_le_drive::DossierVu> = vus.into_iter().filter(sur_le_drive::DossierVu::se_reprend).collect();
            match a_reprendre.len() {
                0 => return refus(Some(socle::echec::Classe::SauvegardeAbsente), &format!("Aucune sauvegarde de compagnie sous {} dans le Drive lié : le compte relié est-il celui de la compagnie ? Seuls les dossiers créés par PuppetPaie, avec la même application Google, sont visibles.", racine_drive)),
                1 => a_reprendre.remove(0),
                _ => {
                    let liste: Vec<String> = a_reprendre.iter().map(|v| format!("  {} : --dossier-drive {}", v.nom, v.id)).collect();
                    return refus(Some(socle::echec::Classe::SauvegardeAmbigue), &format!("Plusieurs compagnies dans le Drive : dire laquelle.\n{}", liste.join("\n")));
                }
            }
        }
    };
    if !json {
        dire!("Restauration de Drive/{}/{} vers {}", racine_drive, depuis.nom, cible.display());
    }
    let r = match restauration::restaurer(&cible, &depuis, racine_drive, &mut client) {
        Ok(r) => r,
        Err(e) if e.echec.classe == Some(socle::echec::Classe::DossierNonVide) => {
            return refus(e.echec.classe, &format!("{}\nRestaurer dans un dossier vide, ou mettre ces fichiers de cote, puis relancer.", e.echec.en_lignes()));
        }
        Err(e) => return refus(e.echec.classe, &e.echec.en_lignes()),
    };
    // Le coffre ne se met de cote que s'il vient de descendre du Drive (`OrigineDuCoffre`) : jamais celui qui etait la.
    let coffre = compagnie::coffre::Coffre::du_poste(&cible);
    let reste = restauration::a_faire(&coffre, r.origine_du_coffre(&coffre));
    let ok = r.complete() && r.incidents.is_empty();
    let code = if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    if json {
        dire!("{}", socle::ecriture::json_en_ligne(&Restauree { ok, resultat: r.dite(reste) }));
        return code;
    }
    for f in &r.redescendus {
        dire!("  redescendu {}", f.relatif);
    }
    dire!("\n{}", r.resume());
    if r.ecartes > 0 {
        dire!("{} fichier(s) du Drive laisse(s) la-bas : ce que la sauvegarde n'envoie pas.", r.ecartes);
    }
    for i in &r.a_retenter {
        dire!("  ! a retenter : {}", i);
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
