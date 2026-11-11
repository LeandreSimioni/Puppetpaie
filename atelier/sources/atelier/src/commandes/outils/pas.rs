//! `moteur-atelier pas [--amorcer] [--demander [--motif 07] [--salaries A,B] [--mois AAAA-MM] [--test] [--forcer]]` : les
//! taux de prelevement a la source recus (la fiche de chaque salarie, `droit::pas`), l'amorcage depuis les DSN deposees
//! (`taux_pas::derniers_taux_des_dsn`), et les signalements d'amorcage a ecrire (`amorcage_demande::produire`, nature 08 :
//! ecrits, jamais deposes ici).

use super::dit;
use moteur::commandes::analyse::{Args, Opt, Spec};
use moteur::commandes::Cadre;
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
    let export = routes::fiches::pieces_importees(ctx);
    if !export.exists() {
        dire!("Dossier introuvable : {}", export.display());
        return Ok(1);
    }
    let releves = compagnie::taux_pas::derniers_taux_des_dsn(ctx, &export)?;
    let chgts = routes::gestes::taux_pas::enregistrer(ctx, &releves, &format!("amorcage depuis les DSN deposees ({})", super::nom(&export)))?;
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
    let diag = droit::pas::diagnostic(ctx, None, None, None)?;
    dire!("Taux de prelevement a la source recus\n{}", "-".repeat(62));
    for s in &diag {
        let cle = &s.salarie;
        let recus = droit::pas::recus(ctx, cle)?;
        let Some(e) = recus.last() else {
            dire!("  {:<12} aucun taux recu", cle);
            continue;
        };
        let ecrit = |t: Option<String>| dit(t.as_deref()).to_string();
        let libelle = match e.r#type {
            Some(t) => t.libelle(),
            None => "None",
        };
        dire!("  {:<12} taux {:<6} type {} ({})", cle, ecrit(e.taux.map(|t| t.to_string())), ecrit(e.r#type.map(|t| t.to_string())), libelle);
        dire!("  {:<12} identifiant {}, mis a disposition {}", "", dit(e.identifiant.as_deref()), ecrit(e.mis_a_disposition.map(|m| m.compact())));
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
    for s in diag.iter().filter(|s| s.situation == droit::pas::SituationDuTaux::SansNir) {
        dire!("  {:<12} {} -- rien a demander, c'est le bareme.", s.salarie, s.libelle);
    }
    Ok(0)
}

/// `--demander` : ecrire les signalements d'amorcage (nature 08), un fichier par individu. RIEN N'EST DEPOSE : le depot est la
/// route `deposer` (avec `fichier`), sa preuve et son mot de passe.
fn demander(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    use compagnie::amorcage_demande::{produire, Demande};
    let ctx = &cadre.ctx;
    let mois = match args.tape("mois") {
        Some(m) => match m.mois() {
            Ok(m) => m,
            Err(_) => {
                dire!("--mois attend AAAA-MM, recu {}.", socle::fichiers::repr(m.texte()));
                return Ok(1);
            }
        },
        None => match engagement::stock::mois_saisis(&ctx.racine)?.into_iter().last() {
            Some(m) => m,
            None => {
                dire!("Aucun mois saisi : un signalement d'amorcage declare un contrat (bloc 40), et il n'y en a aucun.");
                return Ok(1);
            }
        },
    };
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let paies = declarations::dsn::contrats_du_mois(ctx, mois, &departs)?;
    if paies.is_empty() {
        dire!("Aucun engagement valide ne touche {} : rien n'est ecrit.", mois);
        return Ok(1);
    }
    let salaries: Vec<String> = args.texte("salaries").into_iter().flat_map(|t| t.split(',')).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let demande = Demande {
        motif: super::avec_defaut(args, "motif")?,
        mois,
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
