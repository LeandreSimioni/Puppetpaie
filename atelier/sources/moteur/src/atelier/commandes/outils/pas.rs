//! `moteur-atelier pas [--amorcer] [--demander [--motif 07] [--salaries A,B] [--mois AAAA-MM] [--test] [--forcer]]` : les
//! taux de prelevement a la source recus (la fiche de chaque salarie, `droit::pas`), l'amorcage depuis les DSN deposees
//! (`taux_pas::derniers_taux_des_dsn`), et les signalements d'amorcage a ecrire (`amorcage_demande::produire`, nature 08 :
//! ecrits, jamais deposes ici).

use super::dit;
use crate::commandes::analyse::{Args, Opt, Spec};
use crate::commandes::Cadre;
use compagnie::taux_pas::ko;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "pas",
    positionnels: &[],
    options: &[
        Opt::drapeau("amorcer"),
        Opt::drapeau("demander"),
        Opt::avec_defaut("motif", "07"),
        Opt::valeur("salaries"),
        Opt::valeur("mois"),
        Opt::drapeau("test"),
        Opt::drapeau("forcer"),
    ],
};

/// Les types de taux, cahier technique 2026.1, rubrique S21.G00.50.007 (`atelier/reference/sources/norme-dsn/texte/`).
const TYPES: [(&str, &str); 8] = [
    ("01", "Taux transmis par la DGFIP"),
    ("13", "Bareme mensuel metropole"),
    ("17", "Bareme mathematique sur base mensuelle metropole"),
    ("23", "Bareme mensuel Guadeloupe, Reunion et Martinique"),
    ("27", "Bareme mathematique sur base mensuelle Guadeloupe, Reunion, Martinique"),
    ("33", "Bareme mensuel Guyane et Mayotte"),
    ("37", "Bareme mathematique sur base mensuelle Guyane et Mayotte"),
    ("99", "Indu relatif a un exercice anterieur - pas de taux de PAS"),
];

/// Le libelle du type de taux `t`, s'il est un code de la norme.
fn libelle_type(t: Option<&str>) -> &'static str {
    match TYPES.iter().find(|(c, _)| Some(*c) == t) {
        Some((_, l)) => l,
        None => "type hors de la norme",
    }
}

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    super::code(if args.drapeau("demander") {
        demander(cadre, args)
    } else if args.drapeau("amorcer") {
        amorcer(cadre)
    } else {
        lire(cadre)
    })
}

/// `--amorcer` : l'etat courant pose depuis la derniere DSN deposee de chaque salarie (celles de l'import).
fn amorcer(cadre: &Cadre) -> Result<i32, Echec> {
    let ctx = &cadre.ctx;
    let export = crate::routes::fiches::pieces_importees(ctx);
    if !export.exists() {
        dire!("Dossier introuvable : {}", export.display());
        return Ok(1);
    }
    let releves = compagnie::taux_pas::derniers_taux_des_dsn(ctx, &export)?;
    let chgts = crate::gestes::taux_pas::enregistrer(ctx, &releves, &format!("amorcage depuis les DSN deposees ({})", super::nom(&export)))?;
    dire!("Amorcage depuis les DSN deposees ({}) :", export.display());
    for c in &chgts {
        dire!("  {}", c);
    }
    if chgts.is_empty() {
        dire!("  rien a changer.");
    }
    dire!("\nCes valeurs sont celles REELLEMENT DECLAREES dans les dernieres DSN, pas des valeurs calculees. Elles tiennent jusqu'au\nprochain compte-rendu DGFiP (la releve des retours, route `ouverture`).");
    Ok(0)
}

/// Les taux recus (la fiche de chaque salarie, `droit::pas::recus`), et ce qu'il faudrait demander a la DGFiP.
fn lire(cadre: &Cadre) -> Result<i32, Echec> {
    let ctx = &cadre.ctx;
    let diag =droit::pas::diagnostic(ctx, None, None, None).map_err(ko)?;
    dire!("Taux de prelevement a la source recus\n{}", "-".repeat(62));
    for s in &diag {
        let cle = &s.salarie;
        let recus = droit::pas::recus(ctx, cle).map_err(ko)?;
        let Some(e) = recus.last() else {
            dire!("  {:<12} aucun taux recu", cle);
            continue;
        };
        dire!("  {:<12} taux {:<6} type {} ({})", cle, dit(e.taux.as_deref()), dit(e.r#type.as_deref()), libelle_type(e.r#type.as_deref()));
        dire!("  {:<12} identifiant {}, mis a disposition {}", "", dit(e.identifiant.as_deref()), dit(e.mis_a_disposition.as_deref()));
        match s.valable_jusqu_a {
            Some(j) => dire!("  {:<12} {}, jusqu'au {}\n", "", s.libelle, j.jj_mm_aaaa()),
            None => dire!("  {:<12} {}\n", "", s.libelle),
        }
    }
    let a_demander: Vec<&droit::pas::Situation> = diag.iter().filter(|s| s.a_demander).collect();
    if !a_demander.is_empty() {
        dire!("A demander a la DGFiP (signalement d'amorcage, nature 08)\n{}", "-".repeat(62));
        for s in a_demander {
            match compagnie::amorcage_demande::en_cours(ctx, &s.salarie)? {
                Some(d) => dire!("  {:<12} {}  -- demande partie le {}, reponse attendue avant le {}", s.salarie, s.libelle, d.quand.jj_mm_aaaa(), d.avant.jj_mm_aaaa()),
                None => dire!("  {:<12} {}", s.salarie, s.libelle),
            }
        }
        dire!("\nLes signalements s'ecrivent par `pas --demander` ; la DPAE les fait partir avec elle.");
    }
    for s in diag.iter().filter(|s| s.situation == droit::pas::SANS_NIR) {
        dire!("  {:<12} {} -- rien a demander, c'est le bareme.", s.salarie, s.libelle);
    }
    Ok(0)
}

/// `--demander` : ecrire les signalements d'amorcage (nature 08), un fichier par individu. RIEN N'EST DEPOSE : le depot est la
/// route `deposer` (avec `fichier`), sa preuve et son mot de passe.
fn demander(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    use compagnie::amorcage_demande::{produire, Demande};
    let ctx = &cadre.ctx;
    let mois = match args.texte("mois") {
        Some(m) if socle::mois::Mois::du_texte(m).is_some() => m.to_string(),
        Some(m) => {
            dire!("--mois attend AAAA-MM, recu {}.", socle::fichiers::repr(m));
            return Ok(1);
        }
        None => match engagement::stock::mois_saisis(&ctx.racine)?.into_iter().map(String::from).last() {
            Some(m) => m,
            None => {
                dire!("Aucun mois saisi : un signalement d'amorcage declare un contrat (bloc 40), et il n'y en a aucun.");
                return Ok(1);
            }
        },
    };
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let paies = declarations::dsn::contrats_du_mois(ctx, &mois, &departs)?;
    if paies.is_empty() {
        dire!("Aucun engagement valide ne touche {} : rien n'est ecrit.", mois);
        return Ok(1);
    }
    let salaries: Vec<String> = args.texte("salaries").into_iter().flat_map(|t| t.split(',')).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let demande = Demande {
        motif: super::avec_defaut(args, "motif")?,
        mois: &mois,
        salaries: if salaries.is_empty() { None } else { Some(&salaries) },
        envoi_test: args.drapeau("test"),
        forcer: args.drapeau("forcer"),
        constitution: cadre.aujourdhui,
    };
    let mut ecrits = Vec::new();
    let lot = produire(ctx, &paies, &demande, &mut ecrits)?;
    let nom = super::nom;
    dire!("Signalements d'amorcage des donnees variables (nature 08)");
    dire!("Motif {} - {}\n{}", lot.motif, declarations::amorcage::libelle_motif(&lot.motif)?, "-".repeat(62));
    for (ligne, f) in lot.lignes.iter().zip(&lot.fichiers) {
        dire!("  {:<12} {:<24} {}", ligne.salarie, ligne.libelle, nom(f));
    }
    for ligne in &lot.ecartes {
        dire!("  {:<12} ecarte : {}", ligne.salarie, ligne.pourquoi_pas);
    }
    dire!("\n  recapitulatif : {}", nom(&lot.recapitulatif));
    dire!("\nUN SIGNALEMENT, UN INDIVIDU : chaque fichier se depose separement, par la route `deposer` (avec `fichier`).");
    if args.drapeau("test") {
        dire!("Ces fichiers sont des ESSAIS (S10.G00.00.005 = 01) : ils ne demandent rien.");
    }
    Ok(0)
}
