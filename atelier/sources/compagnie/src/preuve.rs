//! LES PORTES DE L'ACTE REEL, du cote de la compagnie : la preuve qu'un humain veut cet acte, maintenant (le mot de passe
//! du logiciel, retape), la version exigee qui attend d'etre appliquee, et le secret de l'organisme pour un depot, pris
//! au coffre seulement une fois la preuve verifiee. Un depot, un envoi, une signature passent tous par ici.

use socle::echec::Classe;

use droit::contexte::Contexte;
use socle::echec::{erreur, R};
use socle::mois::MoisOuJour;

/// `Preuve` : ce qui prouve qu'un humain veut CET acte, MAINTENANT. Le mot de passe DU LOGICIEL, retape.
#[derive(Default, Clone)]
pub struct Preuve {
    pub motdepasse: Option<socle::secret::Secret>,
    /// La preuve deja donnee pour ce geste (sa forme), quand un acte en emporte d'autres : la demande de taux de PAS part
    /// avec la DPAE sous la meme preuve, sans redemander. Jamais lue d'une charge.
    pub donnee: Option<String>,
}

impl Preuve {
    /// La preuve que la charge d'un acte porte : le mot de passe du logiciel tape pour lui (vide : rien n'a ete tape).
    pub fn du_mot_de_passe(motdepasse: Option<socle::secret::Secret>) -> Preuve {
        Preuve { motdepasse: motdepasse.filter(|m| !m.devoiler().is_empty()), ..Preuve::default() }
    }
}

// -------------------------------------------------------------------- la preuve

/// La raison pour laquelle une version telechargee et pas encore appliquee est
/// indispensable avant de declarer ; `None` : aucune ne l'est. Une version illisible se refuse (`VersionDuLogiciel`) ;
/// le binaire de l'atelier ne la lit pas : il refuse tout acte reel avant (`refus_atelier`).
/// La version prete est celle de l'INSTALLATION du contexte (`<installation>/logiciel/donnees` = `ctx.code`).
#[cfg(not(feature = "atelier"))]
fn exigee_en_attente(ctx: &Contexte) -> R<Option<String>> {
    let code = ctx.code.parent().map(std::path::Path::to_path_buf).ok_or_else(|| erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : le dossier des données livrées n'est dans aucun dossier."))?;
    let Some(p) = crate::maj::prete(&code.join(".."))? else { return Ok(None) };
    let (exigee, version) = (p.exigee, p.version);
    if exigee.is_empty() || !version.plus_recente_que(&crate::maj::courante(&code).map_err(|e| e.reclasser(Classe::Illisible))?) {
        return Ok(None);
    }
    Ok(Some(format!(
        "la version {} est indispensable avant de declarer ({}) : fermez et rouvrez PuppetPaie, elle s'appliquera au demarrage",
        version, exigee
    )))
}

/// La forme de preuve retenue, ou un refus. Aucun acte sans elle.
/// LA REGLE : LE MOT DE PASSE DU LOGICIEL, toujours -- le mois recopie seul n'a jamais prouve qu'un humain
/// etait la. Un booleen « confirme=True » venu du front ne prouve rien du tout.
///
/// DEUX FACONS DE LE PROUVER, et aucune autre : le mot de passe que la fenetre ou la page a fait taper et qui
/// arrive dans la charge (`Preuve`) ; sinon LE MOTEUR LE DEMANDE LUI-MEME (`demande::autoriser_acte` : fenetre, terminal,
/// ou refus). Ni la seance (`garder_ouverture`), ni `PUPPETPAIE_MOTDEPASSE`, ni le mot de passe retenu pour ouvrir le
/// coffre ne prouvent quoi que ce soit : ils ouvrent, ils n'autorisent pas. `quand`, `quoi` et `a_qui` disent a l'humain
/// ce qui va partir.
pub fn verifier_preuve(ctx: &Contexte, preuve: Option<&Preuve>, quand: MoisOuJour, quoi: &str, a_qui: &str) -> R<String> {
    // Le binaire de l'ATELIER refuse tout acte reel, avant toute autre question (`atelier::redirection`). Un depot reel, un envoi,
    // une signature passent tous par ici.
    if let Some(forme) = preuve.and_then(|p| p.donnee.clone()) {
        return Ok(forme);
    }
    #[cfg(feature = "atelier")]
    return refus_atelier(ctx, preuve, quand, quoi, a_qui);
    #[cfg(not(feature = "atelier"))]
    verifier_preuve_du_logiciel(ctx, preuve, quand, quoi, a_qui)
}

#[cfg(feature = "atelier")]
fn refus_atelier(ctx: &Contexte, _preuve: Option<&Preuve>, _quand: MoisOuJour, _quoi: &str, _a_qui: &str) -> R<String> {
    // UNE COMPAGNIE FICTIVE AUX ENVOIS SIMULES : son « acte reel » est joue (`FICTIF-…`), rien ne part -- comme dans le
    // binaire livre (`verifier_preuve_du_logiciel`). Le banc d'essai en vit : l'horloge y depose la DSN et la DPAE
    // « en reel ». Toute autre compagnie : refuse, mot de passe ou non (la commande `signatures` fait de meme).
    if crate::bridage::branchement(&ctx.racine)?.joue() {
        return Ok("fictive".into());
    }
    Err(erreur(Classe::ActeReelRefuse, envois::redirection::REFUS_ACTE_REEL.to_string()))
}

#[cfg(not(feature = "atelier"))]
fn verifier_preuve_du_logiciel(ctx: &Contexte, preuve: Option<&Preuve>, quand: MoisOuJour, quoi: &str, a_qui: &str) -> R<String> {
    // LA COMPAGNIE FICTIVE : rien ne part, il n'y a rien a prouver.
    if crate::bridage::branchement(&ctx.racine)?.joue() {
        return Ok("fictive".into());
    }
    // Une version EXIGEE attend d'etre appliquee : l'ancienne ferait une declaration fausse.
    if let Some(presse) = exigee_en_attente(ctx)? {
        return Err(erreur(Classe::VersionExigee, format!("Rien n'est parti : {}.", presse)));
    }
    let coffre = crate::coffre::Coffre::du_poste(&ctx.racine);
    if !coffre.ouverture_exigee()? {
        return Err(erreur(
            Classe::PreuveInsuffisante,
            "Aucun mot de passe du logiciel n'est pose : rien ne peut partir en reel. Le poser d'abord : la fenêtre PuppetPaie le demande à sa première ouverture. Rien n'est parti.",
        ));
    }
    let Some(preuve) = preuve.filter(|p| p.motdepasse.as_ref().is_some_and(|m| !m.devoiler().is_empty())) else {
        // Rien n'a ete tape pour cette charge : c'est le moteur qui demande, a un humain, ou il refuse.
        let acte = crate::demande::Acte::nouveau(quoi, Some(quand), a_qui);
        return match crate::demande::autoriser_acte(&ctx.racine, &acte) {
            crate::demande::Verdict::Accepte => Ok("mot de passe du logiciel".into()),
            crate::demande::Verdict::Refuse(e) => Err(e.reclasser(Classe::PreuveInsuffisante)),
        };
    };
    // Aucun mot de passe tape n'ouvre rien.
    let ouvre = preuve.motdepasse.as_ref().map(|m| coffre.verifier_ouverture(m)).transpose()?;
    if ouvre != Some(true) {
        return Err(erreur(Classe::PreuveInsuffisante, "Mot de passe du logiciel refuse. Rien n'est parti."));
    }
    Ok("mot de passe du logiciel".into())
}

// ------------------------------------------------------------ le secret de l'organisme

/// Le secret de l'organisme -- l'environnement d'abord (quelqu'un l'a tape dans une
/// fenetre, il vaut confirmation), puis le coffre. Ce qui autorise a y puiser pour un depot REEL, c'est la
/// preuve verifiee juste avant par l'appelant. Ce qui SORT d'ici est un secret : il va directement au client.
pub fn motdepasse_organisme(ctx: &Contexte, service: &str, ouverture: Option<&socle::secret::Secret>) -> R<Option<socle::secret::Secret>> {
    let coffre = crate::coffre::Coffre::du_poste(&ctx.racine);
    if !coffre.disponible(service)? {
        return Err(erreur(
            Classe::Refuse,
            format!("Mot de passe {} absent : ni dans l'environnement, ni au coffre.\n  route coffre-poser, service {}", service, service),
        ));
    }
    coffre.lire(service, crate::coffre::DEPOT, true, ouverture)
}
