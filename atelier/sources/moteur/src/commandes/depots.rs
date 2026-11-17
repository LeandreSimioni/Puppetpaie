//! `moteur depots [--depuis JJ/MM/AAAA] [--jusqu-a JJ/MM/AAAA] [--environnement production|editeur]` : la liste des depots que
//! net-entreprises a vus, telle quelle. CACHEE : elle marche, elle
//! n'apparait pas dans l'aide. C'est une commande d'enquete pour l'editeur, sur le poste d'une compagnie -- retrouver
//! quelque chose sur un depot passe, par exemple la date de constitution d'un fichier qu'on n'a plus. Volontairement
//! BRUTE : tous les champs que le service renvoie, sans en choisir.
//!
//! ATTENTION : elle va au RESEAU (net-entreprises). Lecture seule.

use super::analyse::{Args, Opt, Spec};
use socle::echec::Classe;
use socle::paris::Moment;
use super::Cadre;
use envois::releve_retours::{Client, EnvironnementNetEntreprises, Panne};
use socle::echec::Echec;
use std::process::ExitCode;

pub static SPEC: Spec = Spec {
    commande: "depots",
    positionnels: &[],
    options: &[
        Opt::valeur("depuis"),
        Opt::valeur("jusqu-a"),
        Opt::choix("environnement", EnvironnementNetEntreprises::NOMS, Some("production")),
        Opt::drapeau("json"),
    ],
};

pub fn executer(cadre: &Cadre, args: &Args) -> ExitCode {
    match faire(cadre, args) {
        Ok(c) => c,
        Err(e) => refus_reseau_ou_plantage(cadre, e),
    }
}

fn faire(cadre: &Cadre, args: &Args) -> Result<ExitCode, Echec> {
    // Le moteur ouvre le coffre comme pour `retours` s'il est ferme.
    cadre.ouvrir_le_coffre();
    let Some(mdp) = compagnie::consultation::mot_de_passe_de_consultation(&cadre.ctx, "net-entreprises")? else {
        dire!("Mot de passe absent. Le poser une fois : `route coffre-poser, service net-entreprises`.");
        return Ok(ExitCode::from(2));
    };
    let client = Client::dsn(&cadre.ctx, environnement(args)?, Some(mdp));
    derouler(args, client)
}

/// La plate-forme que la ligne de commande nomme (`--environnement`).
fn environnement(args: &Args) -> Result<EnvironnementNetEntreprises, Echec> {
    EnvironnementNetEntreprises::lire(args.exige("environnement")?)
}

/// Le compte rendu, avec un client donne (un test le pointe sur un faux serveur local).
pub fn derouler(args: &Args, mut client: Client) -> Result<ExitCode, Echec> {
    let environnement = environnement(args)?;
    let (debut, fin) = fenetre(args)?;
    dire!("Depots {}, du {} au {}.\n", environnement, debut.jour_de_paris().jj_mm_aaaa(), fin.jour_de_paris().jj_mm_aaaa());
    let mut depots = Vec::new();
    let tranches = envois::releve_retours::tranches(debut, fin)?;
    let resultat: Result<(), Panne> = (|| {
        client.authentifier()?;
        for (t0, t1) in tranches {
            depots.extend(client.lister_depots(t0, t1)?);
        }
        Ok(())
    })();
    match resultat {
        Ok(()) => {}
        Err(p) if p.est_api() => {
            dire!("Echec : {}", p.echec.en_lignes());
            return Ok(ExitCode::FAILURE);
        }
        Err(p) => return Err(p.en_echec()),
    }
    if depots.is_empty() {
        dire!("Aucun depot sur la periode. Elargir avec --depuis JJ/MM/AAAA.");
        return Ok(ExitCode::SUCCESS);
    }
    for (i, dep) in depots.iter().enumerate() {
        dire!("  depot {}", i + 1);
        for (k, v) in [("idflux", dep.idflux.to_string()),("date", socle::paris::ecrit::compacte::texte(dep.date)), ("statut", dep.statut.clone())] {
            dire!("     {:<24} {}", k, v);
        }
        dire!();
    }
    dire!("{} depot(s).", depots.len());
    Ok(ExitCode::SUCCESS)
}

/// Le reseau injoignable ou coupe, ou le coffre qui ne s'ouvre pas : une phrase et un code, pas un plantage.
fn refus_reseau_ou_plantage(cadre: &Cadre, e: Echec) -> ExitCode {
    if matches!(e.classe, Some(Classe::Reseau | Classe::DelaiDepasse)) {
        dire!("Consultation interrompue : le reseau est injoignable ou a ete coupe ({}).\nRien n'est perdu : relancer la commande quand la connexion est revenue.", e.en_lignes());
        return ExitCode::FAILURE;
    }
    // Le coffre ferme et la seance refusee : « rien ici ne l'ouvre » est une phrase pour l'utilisateur, pas un defaut du logiciel.
    // Code 2, comme « le mot de passe n'est nulle part ».
    if e.classe == Some(Classe::Coffre) {
        dire!("{}", e.en_lignes());
        return ExitCode::from(2);
    }
    cadre.plantage(e)
}

/// L'instant d'une borne de la plage, s'il est du calendrier.
fn borne(m: Option<Moment>) -> Result<Moment, Echec> {
    m.ok_or_else(|| socle::echec::erreur(Classe::Illisible, socle::date::HORS_CALENDRIER))
}

/// LA PLAGE DE RECHERCHE DES RETOURS, la seule : du debut du jour `depuis` a la fin du jour `jusqu-a` (inclus). Par
/// defaut, jusqu'a maintenant, et les 30 derniers jours de 24 heures (`ouverture::TRENTE_JOURS`).
pub fn fenetre(args: &Args) -> Result<(Moment, Moment), Echec> {
    let fin = match args.tape("jusqu-a") {
        Some(j) => borne(Moment::derniere_seconde_a_paris(j.jour().map_err(|m| m.reclasser(Classe::Illisible))?))?,
        None => socle::temps::secondes(),
    };
    let debut = match args.tape("depuis") {
        Some(j) => borne(Moment::a_paris(j.jour().map_err(|m| m.reclasser(Classe::Illisible))?, socle::paris::HeureDuJour::MINUIT))?,
        None => fin.moins_duree(routes::gestes::ouverture::TRENTE_JOURS).ok_or_else(|| socle::echec::refus(socle::date::HORS_CALENDRIER))?,
    };
    Ok((debut, fin))
}


