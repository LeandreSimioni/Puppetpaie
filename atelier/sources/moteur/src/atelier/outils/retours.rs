//! `moteur-atelier retours [--dpae] [--flux IDFLUX] [--depuis JJ/MM/AAAA] [--jusqu-a JJ/MM/AAAA] [--tout]
//! [--environnement production|editeur]` : l'enquete de l'editeur sur les retours d'une compagnie -- une plage choisie,
//! un flux, tout ce que la plate-forme garde (`--tout`), la plate-forme d'essai de l'editeur. Le releve est celui du
//! moteur (`compagnie::releve`), qui applique ce qui revient (taux de PAS, AER, effectif) ; la releve ordinaire est la route
//! `ouverture`. Ici ne reste que ce qui s'affiche.
//!
//! Dans le binaire de l'atelier, un organisme qui n'est pas redirige vers un faux serveur local n'est pas appele, hors
//! compagnie fictive (`commandes::actes::refuser_si_pas_redirige`).


use crate::commandes::actes::{refuser_si_pas_redirige, Organisme};
use crate::commandes::analyse::{Args, Opt, Spec};
use crate::commandes::Cadre;
use socle::date::Jour;
use socle::echec::Echec;

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

const JOUR: i64 = 86_400;

/// Un jour en heure locale naive (secondes), a minuit.
fn secondes(j: &Jour) -> i64 {
    (j.ordinal() - 719_163) * JOUR
}

fn jour_de(t: i64) -> Result<String, Echec> {
    match Jour::depuis_jours_unix(t.div_euclid(JOUR)) {
        Some(j) => Ok(j.jj_mm_aaaa()),
        None => Err(Echec { classe: Some(socle::echec::Classe::Illisible), message: format!("{} secondes ne tombent pas dans le calendrier.", t) }),
    }
}

/// La plage : `--depuis` et `--jusqu-a` (inclus), par defaut les 30 derniers jours.
fn plage(args: &Args) -> Result<(i64, i64), Echec> {
    let lire = |t: &str| Jour::lire(t).map_err(|m| Echec { classe: Some(socle::echec::Classe::Illisible), message: m });
    let fin = match args.texte("jusqu-a") {
        Some(j) => secondes(&lire(j)?) + JOUR - 1,
        None => socle::paris::maintenant_naif(),
    };
    let debut = match args.texte("depuis") {
        Some(j) => secondes(&lire(j)?),
        None => fin - 30 * JOUR,
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
        Some(f) => dire!("Retours {}, du {} au {}, flux {}.", ou, jour_de(debut)?, jour_de(fin)?, f),
        None => dire!("Retours {}, du {} au {}.", ou, jour_de(debut)?, jour_de(fin)?),
    }
    let tout = args.drapeau("tout");
    // La releve de la DPAE prend le flux en texte, vide quand elle releve par plage.
    let r = match (dpae, flux) {
        (true, Some(f)) => crate::compagnie::releve::relever_retours_dpae(&cadre.ctx, debut, fin, tout, f)?,
        (true, None) => crate::compagnie::releve::relever_retours_dpae(&cadre.ctx, debut, fin, tout, "")?,
        (false, _) => crate::compagnie::releve::relever_retours(&cadre.ctx, debut, fin, tout, environnement, None, "")?,
    };
    for i in &r.incidents {
        dire!("  {}", i);
    }
    let mut trouves: Vec<&envois::retours::Annonce> = r.trouves.iter().filter(|x| flux.is_none() || x.flux.as_deref() == flux).collect();
    trouves.sort_by_key(|x| x.publication.clone());
    dire!("{} retour(s), dont {} nouveau(x).\n", trouves.len(), r.nouveaux.len());
    for x in trouves {
        dire!(
            "  {} {}  nature {:<4} {:<8} {}  flux {}",
            if r.nouveaux.contains(x) { "NEUF" } else { "    " },
            super::dit(envois::releve_retours::date_publication(x).as_deref()),
            x.nature,
            super::dit(x.statut.as_deref()),
            envois::retours::libelle_nature(&x.nature),
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
