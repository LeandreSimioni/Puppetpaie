//! `moteur-atelier depot-dsn (FICHIER | AAAA-MM) [--environnement production|editeur]` : un depot de TEST d'une DSN, en
//! production ou sur la plate-forme d'essai de l'editeur. Le depot est celui du moteur (`depot_acte::deposer_dsn_dans`),
//! qui choisit l'ordre d'un essai de mensuelle apres avoir releve la plate-forme, et refuse tout depot reel ailleurs
//! qu'en production ; le depot ordinaire, reel ou de test, est la route `deposer`. Un mois `AAAA-MM` : la DSN que la
//! route designerait (`depots::designee`).


use moteur::commandes::actes::{refuser_si_pas_redirige, Organisme};
use moteur::commandes::analyse::{Args, Opt, Pos, Spec};
use moteur::commandes::Cadre;
use compagnie::preuve::Preuve;
use routes::gestes::depot_acte;
use socle::echec::Echec;
use std::process::ExitCode;

pub static SPEC: Spec = Spec {
    commande: "depot-dsn",
    positionnels: &[Pos::facultatif("fichier", None)],
    options: &[Opt::choix("environnement", envois::releve_retours::EnvironnementNetEntreprises::NOMS, Some("production"))],
};

pub fn executer(cadre: &Cadre, args: &Args) -> ExitCode {
    super::code(faire(cadre, args))
}

fn faire(cadre: &Cadre, args: &Args) -> Result<ExitCode, Echec> {
    let Some(fichier) = args.tape("fichier").filter(|f| !f.texte().is_empty()) else {
        dire!("depot-dsn FICHIER|AAAA-MM [--environnement editeur] : un depot de TEST.");
        return Ok(ExitCode::from(2));
    };
    refuser_si_pas_redirige(cadre, Organisme::Dsn, "le depot de test")?;
    // Un mois tape (`AAAA-MM`) designe la DSN du mois, comme la route (`depots::designee`) ; tout autre texte, un fichier.
    let designee = match fichier.designe_un_mois() {
        Some(m) => routes::depots::designee(&cadre.ctx, &routes::depots::Designation::du_mois(routes::depots::Quoi::Dsn, m))?.0,
        None => routes::depots::resoudre(&cadre.ctx.racine, std::path::Path::new(fichier.texte()))?,
    };
    // Un depot de test n'engage rien : il lui faut seulement le secret de l'organisme, donc le coffre ouvert.
    cadre.ouvrir_le_coffre();
    let environnement = envois::releve_retours::EnvironnementNetEntreprises::lire(super::avec_defaut(args, "environnement")?)?;
    let fait = depot_acte::deposer_dsn_dans(&cadre.ctx, &designee, socle::mode::Mode::Test, Some(&Preuve::default()), "atelier", environnement, false, "")?;
    dire!("Depot de TEST {} ({}) : rien n'est conserve.", fait.mois, environnement);
    dire!("Reponse du point de depot :\n{}", if fait.reponse.is_empty() { "(vide)" } else { fait.reponse.as_str() });
    if let Some(flux) = &fait.flux {
        dire!("Flux : {} -- les retours s'y rattacheront (outil retours, ou la route ouverture).", flux);
    }
    for s in &fait.suites {
        dire!("{}", s);
    }
    Ok(ExitCode::SUCCESS)
}
