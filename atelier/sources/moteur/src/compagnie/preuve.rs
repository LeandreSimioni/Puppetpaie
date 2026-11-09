//! LES PORTES DE L'ACTE REEL, du cote de la compagnie : la preuve qu'un humain veut cet acte, maintenant (le mot de passe
//! du logiciel, retape), la version exigee qui attend d'etre appliquee, et le secret de l'organisme pour un depot, pris
//! au coffre seulement une fois la preuve verifiee. Un depot, un envoi, une signature passent tous par ici.

use socle::echec::Classe;
use std::path::Path;

use droit::contexte::Contexte;
use socle::echec::R;
use socle::echec::erreur;

/// `Preuve` : ce qui prouve qu'un humain veut CET acte, MAINTENANT. Le mot de passe DU LOGICIEL, retape.
#[derive(Default, Clone)]
pub struct Preuve {
    pub motdepasse: Option<String>,
    pub mois_recopie: Option<String>,
    /// La preuve deja donnee pour ce geste (sa forme), quand un acte en emporte d'autres : la demande de taux de PAS part
    /// avec la DPAE sous la meme preuve, sans redemander. Jamais lue d'une charge.
    pub(crate) donnee: Option<String>,
}

impl Preuve {
    /// La preuve que la charge d'un acte porte : le mot de passe du logiciel tape pour lui (vide : rien n'a ete tape).
    pub fn du_mot_de_passe(motdepasse: Option<String>) -> Preuve {
        Preuve { motdepasse: motdepasse.filter(|m| !m.is_empty()), ..Preuve::default() }
    }
}

// -------------------------------------------------------------------- la preuve

/// Une version lue en entiers separes par des points, pour la comparer ; illisible, elle se refuse.
fn cle_de_version(v: &str) -> R<Vec<i64>> {
    v.trim().split('.').map(|x| x.trim().parse::<i64>()).collect::<Result<Vec<i64>, _>>().map_err(|e| {
        erreur(Classe::Illisible, socle::echec::avec("Un numéro de version ne se lit pas : le signaler au développeur.", socle::echec::APart { detail: Some(format!("« {} » : {}", v, e)), ..socle::echec::APart::default() }))
    })
}

/// La raison pour laquelle une version telechargee et pas encore appliquee est
/// indispensable avant de declarer -- ou `""`.
/// La version prete est celle de l'INSTALLATION du contexte (`<installation>/logiciel/donnees` = `ctx.code`).
pub(crate) fn exigee_en_attente(ctx: &Contexte) -> R<String> {
    let code = ctx.code.parent().map(Path::to_path_buf).ok_or_else(|| erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : le dossier des données livrées n'est dans aucun dossier."))?;
    let Some(p) = crate::compagnie::maj::prete(&code.join(".."))? else { return Ok(String::new()) };
    let (exigee, version) = (p.exigee, p.version);
    if exigee.is_empty() || cle_de_version(&version)? <= cle_de_version(&crate::compagnie::maj::courante(&code).map_err(|m| erreur(Classe::Illisible, m))?)? {
        return Ok(String::new());
    }
    Ok(format!(
        "la version {} est indispensable avant de declarer ({}) : fermez et rouvrez PuppetPaie, elle s'appliquera au demarrage",
        version, exigee
    ))
}

/// La forme de preuve retenue, ou un refus. Aucun acte sans elle.
/// LA REGLE : LE MOT DE PASSE DU LOGICIEL, toujours -- le mois recopie seul n'a jamais prouve qu'un humain
/// etait la. Un booleen « confirme=True » venu du front ne prouve rien du tout.
///
/// DEUX FACONS DE LE PROUVER, et aucune autre : le mot de passe que la fenetre ou la page a fait taper et qui
/// arrive dans la charge (`Preuve`) ; sinon LE MOTEUR LE DEMANDE LUI-MEME (`demande::autoriser_acte` : fenetre, terminal,
/// ou refus). Ni la seance (`garder_ouverture`), ni `PUPPETPAIE_MOTDEPASSE`, ni le mot de passe retenu pour ouvrir le
/// coffre ne prouvent quoi que ce soit : ils ouvrent, ils n'autorisent pas. `quoi` et `a_qui` disent a l'humain ce qui
/// va partir ; `mois_attendu` est le mois que la preuve recopiee devrait porter.
pub(crate) fn verifier_preuve(ctx: &Contexte, preuve: Option<&Preuve>, mois_attendu: &str, quoi: &str, a_qui: &str) -> R<String> {
    // Le binaire de l'ATELIER refuse tout acte reel, avant toute autre question (`atelier::redirection`). Un depot reel, un envoi,
    // une signature passent tous par ici.
    if let Some(forme) = preuve.and_then(|p| p.donnee.clone()) {
        return Ok(forme);
    }
    #[cfg(feature = "atelier")]
    return refus_atelier(ctx, preuve, mois_attendu, quoi, a_qui);
    #[cfg(not(feature = "atelier"))]
    verifier_preuve_du_logiciel(ctx, preuve, mois_attendu, quoi, a_qui)
}

#[cfg(feature = "atelier")]
fn refus_atelier(ctx: &Contexte, _preuve: Option<&Preuve>, _mois_attendu: &str, _quoi: &str, _a_qui: &str) -> R<String> {
    // UNE COMPAGNIE FICTIVE AUX ENVOIS SIMULES : son « acte reel » est joue (`FICTIF-…`), rien ne part -- comme dans le
    // binaire livre (`verifier_preuve_du_logiciel`). Le banc d'essai en vit : l'horloge y depose la DSN et la DPAE
    // « en reel ». Toute autre compagnie : refuse, mot de passe ou non (la commande `signatures` fait de meme).
    if crate::compagnie::bridage::branchement(&ctx.racine)?.joue() {
        return Ok("fictive".into());
    }
    Err(erreur(Classe::ActeReelRefuse, envois::redirection::REFUS_ACTE_REEL.to_string()))
}

#[cfg_attr(feature = "atelier", allow(dead_code))]
fn verifier_preuve_du_logiciel(ctx: &Contexte, preuve: Option<&Preuve>, mois_attendu: &str, quoi: &str, a_qui: &str) -> R<String> {
    // LA COMPAGNIE FICTIVE : rien ne part, il n'y a rien a prouver.
    if crate::compagnie::bridage::branchement(&ctx.racine)?.joue() {
        return Ok("fictive".into());
    }
    // Une version EXIGEE attend d'etre appliquee : l'ancienne ferait une declaration fausse.
    let presse = exigee_en_attente(ctx)?;
    if !presse.is_empty() {
        return Err(erreur(Classe::VersionExigee, format!("Rien n'est parti : {}.", presse)));
    }
    let coffre = crate::compagnie::coffre::Coffre::du_poste(&ctx.racine, "");
    let vers = crate::compagnie::coffre::vers_route;
    if !coffre.ouverture_exigee().map_err(vers)? {
        return Err(erreur(
            Classe::PreuveInsuffisante,
            "Aucun mot de passe du logiciel n'est pose : rien ne peut partir en reel. Le poser d'abord : la fenêtre PuppetPaie le demande à sa première ouverture. Rien n'est parti.",
        ));
    }
    let Some(preuve) = preuve.filter(|p| p.motdepasse.as_deref().is_some_and(|m| !m.is_empty())) else {
        // Rien n'a ete tape pour cette charge : c'est le moteur qui demande, a un humain, ou il refuse.
        let acte = crate::compagnie::demande::Acte::nouveau(quoi, mois_attendu, a_qui);
        return match crate::compagnie::demande::autoriser_acte(&ctx.racine, &acte) {
            crate::compagnie::demande::Verdict::Accepte => Ok("mot de passe du logiciel".into()),
            crate::compagnie::demande::Verdict::Refuse(phrase) => Err(erreur(Classe::PreuveInsuffisante, phrase)),
        };
    };
    if let Some(m) = preuve.mois_recopie.as_deref().filter(|m| !m.is_empty()) {
        if m.trim() != mois_attendu {
            return Err(erreur(Classe::PreuveInsuffisante, format!("Mois confirme '{}' alors que le fichier declare '{}'. Rien n'est parti.", m.trim(), mois_attendu)));
        }
    }
    // Aucun mot de passe tape n'ouvre rien.
    let ouvre = preuve.motdepasse.as_deref().map(|m| coffre.verifier_ouverture(m)).transpose().map_err(vers)?;
    if ouvre != Some(true) {
        return Err(erreur(Classe::PreuveInsuffisante, "Mot de passe du logiciel refuse. Rien n'est parti."));
    }
    Ok("mot de passe du logiciel".into())
}

// ------------------------------------------------------------ le secret de l'organisme

/// Le secret de l'organisme -- l'environnement d'abord (quelqu'un l'a tape dans une
/// fenetre, il vaut confirmation), puis le coffre. Ce qui autorise a y puiser pour un depot REEL, c'est la
/// preuve verifiee juste avant par l'appelant. Ce qui SORT d'ici est un secret : il va directement au client.
pub(crate) fn motdepasse_organisme(ctx: &Contexte, service: &str, ouverture: Option<&str>) -> R<Option<String>> {
    let coffre = crate::compagnie::coffre::Coffre::du_poste(&ctx.racine, "");
    let vers = crate::compagnie::coffre::vers_route;
    if !coffre.disponible(service).map_err(vers)? {
        return Err(erreur(
            Classe::Refuse,
            format!("Mot de passe {} absent : ni dans l'environnement, ni au coffre.\n  route coffre-poser, service {}", service, service),
        ));
    }
    coffre.lire(service, crate::compagnie::coffre::DEPOT, true, ouverture).map_err(vers)
}
