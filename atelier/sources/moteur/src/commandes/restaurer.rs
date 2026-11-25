//! `moteur restaurer DOSSIER_DRIVE` : reprendre une compagnie du Drive sur ce poste. C'EST LA ROUTE `drive-reprendre`,
//! et rien d'autre (Leandre : « tout doit etre des routes ») : la meme charge (`{"dossier": DOSSIER_DRIVE}`, l'identifiant
//! du dossier tel que `drive-compagnies` le rend), servie comme `moteur route` la sert, la meme reponse en une ligne de
//! JSON, le meme code de sortie (1 pour un refus). Le compte Google est celui de l'accueil (`drive-lier`), la compagnie
//! revient dans un dossier neuf de l'installation, et ce poste n'en prend la main qu'au bout, fiche Entreprise lue
//! (`routes::accueil::drive::reprendre`).

use std::process::ExitCode;

use super::analyse::{Args, Pos, Spec};
use super::Cadre;

pub static SPEC: Spec = Spec { commande: "restaurer", positionnels: &[Pos::facultatif("dossier", None)], options: &[] };

/// La demande que `moteur route` lirait sur son entree standard.
#[derive(serde::Serialize)]
struct Demande<'a> {
    route: &'static str,
    charge: Reprendre<'a>,
}

/// La charge de `drive-reprendre`, telle qu'elle est tapee : la route la lit (`ChargeReprendre`) et refuse ce qui ne
/// s'y lit pas.
#[derive(serde::Serialize)]
struct Reprendre<'a> {
    dossier: &'a str,
}

pub fn executer(cadre: &Cadre, args: &Args) -> ExitCode {
    let dossier = match args.exige("dossier") {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}", e.en_lignes());
            return ExitCode::from(2);
        }
    };
    let demande = socle::ecriture::json_en_ligne(&Demande { route: routes::catalogue::Route::DriveReprendre.nom(), charge: Reprendre { dossier } });
    let o = routes::enveloppe::Options { racine: Some(cadre.racine.clone()), date: Some(cadre.aujourdhui) };
    let reponse = routes::commande(&o, &demande);
    dire!("{}", reponse.en_json());
    if reponse.est_rendue() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
