//! `moteur-atelier depot-dsn (FICHIER | AAAA-MM) [--ordre N] [--environnement production|editeur]` : un depot de TEST d'une
//! DSN, a un ordre donne (la base d'essai a deja retenu des essais de ce SIRET pour ce mois que le journal ignore : un
//! autre poste, une compagnie d'essai) ou sur la plate-forme d'essai de l'editeur. Le depot est celui du moteur
//! (`depot_acte::deposer_dsn_dans`), qui refuse un ordre hors d'un essai de mensuelle et tout depot reel ailleurs qu'en
//! production ; le depot ordinaire, reel ou de test, est la route `deposer`. Un mois `AAAA-MM` : la DSN que la route
//! designerait (`depots::designee`).


use moteur::commandes::actes::{refuser_si_pas_redirige, Organisme};
use moteur::commandes::analyse::{Args, Opt, Pos, Spec};
use moteur::commandes::Cadre;
use compagnie::preuve::Preuve;
use routes::gestes::depot_acte;
use socle::echec::Echec;

pub static SPEC: Spec = Spec {
    commande: "depot-dsn",
    positionnels: &[Pos::facultatif("fichier", None)],
    options: &[Opt::valeur("ordre"), Opt::choix("environnement", &["production", "editeur"], Some("production"))],
};

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    super::code(faire(cadre, args))
}

fn faire(cadre: &Cadre, args: &Args) -> Result<i32, Echec> {
    let Some(fichier) = args.tape("fichier").filter(|f| !f.texte().is_empty()) else {
        dire!("depot-dsn FICHIER|AAAA-MM [--ordre N] [--environnement editeur] : un depot de TEST.");
        return Ok(2);
    };
    let ordre = match args.texte("ordre").filter(|t| !t.is_empty()) {
        None => None,
        Some(t) => match t.trim().parse::<i64>() {
            Ok(n) => Some(n),
            Err(e) => {
                dire!("--ordre N : un entier positif ({}).", e);
                return Ok(2);
            }
        },
    };
    refuser_si_pas_redirige(cadre, Organisme::Dsn, "le depot de test")?;
    // Un mois tape (`AAAA-MM`) designe la DSN du mois, comme la route (`depots::designee`) ; tout autre texte, un fichier.
    let designee = match fichier.designe_un_mois() {
        Some(m) => routes::depots::designee(&cadre.ctx, &routes::depots::Designation::du_mois(routes::depots::Quoi::Dsn, m))?.0,
        None => fichier.texte().to_string(),
    };
    // Un depot de test n'engage rien : il lui faut seulement le secret de l'organisme, donc le coffre ouvert.
    cadre.ouvrir_le_coffre();
    let environnement = super::avec_defaut(args, "environnement")?;
    let fait = depot_acte::deposer_dsn_dans(&cadre.ctx, &designee, false, Some(&Preuve::default()), "atelier", environnement, false, "", ordre)?;
    dire!("Depot de TEST {} ({}) : rien n'est conserve.", fait.mois, environnement);
    dire!("Reponse du point de depot :\n{}", if fait.reponse.is_empty() { "(vide)" } else { fait.reponse.as_str() });
    if !fait.flux.is_empty() {
        dire!("Flux : {} -- les retours s'y rattacheront (outil retours, ou la route ouverture).", fait.flux);
    }
    Ok(0)
}
