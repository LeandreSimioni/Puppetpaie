//! `moteur-atelier retours [--dpae] [--flux IDFLUX] [--depuis JJ/MM/AAAA] [--jusqu-a JJ/MM/AAAA] [--tout]
//! [--environnement production|editeur]` : l'enquete de l'editeur sur les retours d'une compagnie -- une plage choisie,
//! un flux, tout ce que la plate-forme garde (`--tout`), la plate-forme d'essai de l'editeur. Le releve est celui du
//! moteur (`gestes::releve`), qui applique ce qui revient (taux de PAS, AER, effectif) ; la releve ordinaire est la route
//! `ouverture`. Ici ne reste que ce qui s'affiche.
//!
//! Dans le binaire de l'atelier, un organisme qui n'est pas redirige vers un faux serveur local n'est pas appele, hors
//! compagnie fictive (`commandes::actes::refuser_si_pas_redirige`).


use moteur::commandes::actes::{refuser_si_pas_redirige, Organisme};
use moteur::commandes::analyse::{Args, Opt, Spec};
use moteur::commandes::Cadre;
use socle::echec::Echec;
use socle::paris::Naif;

pub static SPEC: Spec = Spec {
    commande: "retours",
    positionnels: &[],
    options: &[
        Opt::drapeau("dpae"),
        Opt::valeur("flux"),
        Opt::valeur("depuis"),
        Opt::valeur("jusqu-a"),
        Opt::drapeau("tout"),
        Opt::choix("environnement", &["production", "editeur"], Some("production")),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    super::code(faire(cadre, args))
}

/// La plage : `--depuis` et `--jusqu-a` (inclus), par defaut les 30 derniers jours.
fn plage(args: &Args) -> Result<(Naif, Naif), Echec> {
    let a = |t: &socle::ligne::Argument, h: i64, m: i64, s: i64| {
        let j = t.jour().map_err(|e| e.reclasser(socle::echec::Classe::Illisible))?;
        Naif::du_jour(j, h, m, s).ok_or_else(|| socle::echec::erreur(socle::echec::Classe::Illisible, socle::date::HORS_CALENDRIER))
    };
    let fin = match args.tape("jusqu-a") {
        Some(j) => a(j, 23, 59, 59)?,
        None => socle::paris::maintenant_naif(),
    };
    let debut = match args.tape("depuis") {
        Some(j) => a(j, 0, 0, 0)?,
        None => fin.plus(-30 * 86_400)?,
    };
    Ok((debut, fin))
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let (debut, fin) = plage(args)?;
    let dpae = args.drapeau("dpae");
    refuser_si_pas_redirige(cadre, if dpae { Organisme::Dpae } else { Organisme::Dsn }, "la releve des retours")?;
    cadre.ouvrir_le_coffre();
    let flux = args.texte("flux").filter(|f| !f.is_empty());
    let environnement = super::avec_defaut(args, "environnement")?;
    let ou = if dpae { "DPAE".to_string() } else { format!("DSN ({})", environnement) };
    match flux {
        Some(f) => dire!("Retours {}, du {} au {}, flux {}.", ou, debut.jour().jj_mm_aaaa(), fin.jour().jj_mm_aaaa(), f),
        None => dire!("Retours {}, du {} au {}.", ou, debut.jour().jj_mm_aaaa(), fin.jour().jj_mm_aaaa()),
    }
    let tout = args.drapeau("tout");
    // La releve de la DPAE prend le flux s'il est dit ; sans lui, elle releve par plage.
    let r = if dpae {
        routes::gestes::releve::relever_retours_dpae(&cadre.ctx, debut, fin, tout, flux)?
    } else {
        routes::gestes::releve::relever_retours(&cadre.ctx, debut, fin, tout, environnement, None, None)?
    };
    for i in &r.incidents {
        dire!("  {}", i);
    }
    let mut trouves: Vec<&envois::retours::Annonce> = r.trouves.iter().filter(|x| flux.is_none() || x.flux.as_deref() == flux).collect();
    trouves.sort_by_key(|x| x.publication);
    dire!("{} retour(s), dont {} nouveau(x).\n", trouves.len(), r.nouveaux.len());
    for x in trouves {
        dire!(
            "  {} {}  nature {:<4} {:<8} {}  flux {}",
            if r.nouveaux.contains(x) { "NEUF" } else { "    " },
            super::dit(envois::releve_retours::jour_de_publication_dit(x).as_deref()),
            x.nature_retour,
            super::dit(x.statut.as_deref()),
            envois::retours::libelle_nature(&x.nature_retour),
            super::dit(x.flux.as_deref())
        );
    }
    for quoi in &r.a_faire {
        dire!("\n  A FAIRE : {}", quoi);
    }
    if !r.archives.is_empty() {
        dire!("\n{} fichier(s) archive(s) :", r.archives.len());
        for a in &r.archives {
            dire!("  {}", a.chemin.display());
        }
    }
    Ok(if r.incidents.is_empty() { 0 } else { 1 })
}
