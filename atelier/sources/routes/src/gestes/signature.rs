//! Les contrats qui partent : `demander-signature` (par courriel, avec le bouton « Je signe »), `contrats-imprimer`
//! (pour une remise en main propre), et les avenants que `demander-signature` fait signer avec eux.
//!
//! Un courriel par salarie, un PDF par courriel (le contrat, ou les avenants), le bouton « Je signe » (un lien `mailto:`
//! vers la boite de la compagnie, objet et corps deja ecrits) et un CODE UNIQUE (`SIG-XXXXXX`) par courriel, garde au
//! journal avec la demande : c'est par lui que la releve des signatures reconnait la reponse. Les messages sont ceux que
//! `signature-apercu` et `avenant-apercu` ont montres : les paies (`ecrire_documents::paies_aux_numeros_declares`), les
//! articles modifies (`avenant-apercu` lui-meme, appele ici), les etats (`signature-apercu`, pour dire ce qui part en rappel).
//!
//! La route lit la charge, ouvre la boite, exige la preuve (le mot de passe du logiciel) et fabrique les apercus
//! (`signature-apercu`, `avenant-apercu`) avant d'appeler ce module ; ici, ce qui reste se verifie avant que rien ne
//! parte, puis les PDF s'ecrivent : rien n'est ecrit sur un refus.
//!
//! Chaque contrat part sous le numero que son engagement porte depuis sa validation (`engagement::numeros`) ; le
//! journal note qu'il est parti.
//!
//! Le moteur fait les pieces (`documents::contrat`) et compose les messages ; `envois` les transporte
//! (`envoi::envoyer_suivi`) et note le depart (`ligne_d_envoi`).

use droit::fiche::{ChampsSalarie, Lue as _};
use socle::echec::Classe;
use crate::dossier_salarie::cle_de;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use envois::courriel::Courriel;
use envois::envoi::{self, Message};
use socle::echec::{erreur, R};
use socle::echec::Echec;
use socle::fichiers::nom_du;
use socle::html::{html_echapper, mailto};
use socle::texte::capitaliser;
use envois::ligne_d_envoi::{noter, noter_echec_d_envoi, par_courriel};
use envois::journal::{ArticleModifie, ContratParti, DetailSignature, Ligne, Version};
use socle::date::Jour;
use socle::mois::Mois;
use droit::contexte::IdSalarie;
use engagement::numeros::{numero_dit, NumeroContrat};

/// Un refus ordinaire, avec sa phrase.
fn refus(message: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, message)
}

/// La case `cle` de la fiche entreprise manque.
#[track_caller]
fn requis_manque(cle: &str) -> Echec {
    socle::fichiers::absent(cle).reclasser(Classe::Manque)
}

use documents::marqueurs::Marqueur;

/// Les clauses qu'un avenant peut modifier : ce que L1242-12 et les conventions mettent au contrat et qui bouge
/// d'une paie a l'autre -- l'emploi, les dates, le lieu, la remuneration.
const CLAUSES_MODIFIABLES: [Marqueur; 8] = [
    Marqueur::SalarieEmploi,
    Marqueur::ContratDebut,
    Marqueur::ContratFin,
    Marqueur::ContratDatesTravail,
    Marqueur::ContratLieu,
    Marqueur::RemunerationDetail,
    Marqueur::RemunerationBrut,
    Marqueur::RemunerationHorsBrut,
];

// ------------------------------------------------------------------ les textes

const SUJET_SIGNATURE: &str = "Contrat d'engagement {spectacle} - {mois} {annee}";
const SUJET_SIGNATURE_PLURIEL: &str = "Contrats d'engagement - {mois} {annee}";
const PREFIXE_RELANCE: &str = "Rappel - ";
const RAPPEL_RELANCE: &str = "Petit rappel : ta signature n'est pas encore arrivée. C'est le même contrat que la dernière fois.\n\n";
const RAPPEL_RELANCE_PLURIEL: &str = "Petit rappel : ta signature n'est pas encore arrivée. Ce sont les mêmes contrats que la dernière fois.\n\n";
const RAPPEL_RELANCE_AVENANT: &str = "Petit rappel : ta signature n'est pas encore arrivée. C'est le même avenant que la dernière fois.\n\n";
const RAPPEL_RELANCE_AVENANTS: &str = "Petit rappel : ta signature n'est pas encore arrivée. Ce sont les mêmes avenants que la dernière fois.\n\n";
/// Un courriel qui redemande certaines de ses pieces seulement : le rappel les nomme.
const RAPPEL_EN_PARTIE: &str = "Petit rappel pour {pieces} : ta signature n'est pas encore arrivée, c'est la même pièce que la dernière fois.\n\n";

/// LE RAPPEL D'UN COURRIEL, piece par piece : chacune dit si un courriel l'a deja demandee (le journal le dit). Toutes :
/// l'objet porte « Rappel » et le corps le dit ; quelques-unes : le corps les nomme ; aucune : rien. Rend l'objet et
/// le debut du corps.
fn rappel_du_courriel(sujet: String, liste: String, pieces: &[(String, bool)], toutes: &str) -> (String, String) {
    let rappels: Vec<String> = pieces.iter().filter(|(_, r)| *r).map(|(p, _)| p.clone()).collect();
    if rappels.is_empty() {
        (sujet, liste)
    } else if rappels.len() == pieces.len() {
        (format!("{}{}", PREFIXE_RELANCE, sujet), format!("{}{}", toutes, liste))
    } else {
        (sujet, format!("{}{}", envoi::formater(RAPPEL_EN_PARTIE, &[("pieces", et_fr(&rappels))]), liste))
    }
}
const CORPS_SIGNATURE: &str = "Bonjour {prenom},\n\n{contrats}\n\nPour {le_signer}, un clic suffit : appuie sur le bouton « Je signe » de ce message, puis envoie la réponse qui s'ouvre, sans la modifier. Si ta messagerie n'affiche pas le bouton, réponds simplement à ce message en écrivant « Je signe » suivi de ce code : {code}\n\nTu recevras aussitôt {exemplaire} signé des deux parties, à conserver.{avant_travail}\n\nSi quelque chose ne correspond pas à ce qui était convenu, dis-le avant de signer plutôt qu'après : c'est plus simple à corriger.\n\nBien à toi,\n{signataire}\n{raison_sociale}\n";
const HTML_SIGNATURE: &str = "<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:15px;line-height:1.5;color:#2C3E50\">\n<p>Bonjour {prenom},</p>\n<p>{contrats}</p>\n<p>Pour {le_signer}, un clic suffit : appuie sur le bouton, puis envoie la réponse qui s'ouvre, sans la modifier.</p>\n<p style=\"margin:26px 0\">\n<a href=\"{mailto}\" style=\"display:inline-block;background:#B22222;color:#ffffff;padding:14px 26px;border-radius:6px;text-decoration:none;font-weight:bold;font-size:16px\">Je signe</a>\n</p>\n<p style=\"font-size:13px;color:#555\">Si le bouton ne fait rien, réponds simplement à ce message en écrivant « Je signe » suivi de ce code : <b>{code}</b></p>\n<p>Tu recevras aussitôt {exemplaire} signé des deux parties, à conserver.{avant_travail}</p>\n<p>Si quelque chose ne correspond pas à ce qui était convenu, dis-le avant de signer plutôt qu'après : c'est plus simple à corriger.</p>\n<p>Bien à toi,<br>{signataire}<br>{raison_sociale}</p>\n</div>\n";
pub(crate) const CORPS_REPONSE: &str = "Je soussigné(e) {nom} déclare avoir lu et accepter {quoi}.\n\nCode de signature : {code}\n";

// ------------------------------------------------------------------ les petits outils

/// Un contrat vise par la page : (salarie, debut, numero facultatif).
pub type Cible = (IdSalarie, String, Option<engagement::numeros::NumeroContrat>);

/// LES CONTRATS QUE LA PAGE CHOISIT. La selection est par contrat (`contrats`) : deux contrats du meme salarie qui
/// commencent le meme jour se distinguent par leur numero ; sans numero, elle vaut pour tous ceux de ce salarie a ce
/// debut. `salaries` prend tous les contrats de ces salaries. Rien : tous les contrats du mois.
#[derive(Default)]
pub(crate) struct Choisis {
    pub salaries: Option<Vec<String>>,
    pub contrats: Option<Vec<Cible>>,
}

/// « du 12/01/2026 au 30/06/2026 », ou « à partir du 12/01/2026 » pour un contrat sans terme.
fn periode_de(p: &Paie) -> String {
    match p.engagement.fin {
        Some(f) => format!("du {} au {}", p.engagement.debut.jj_mm_aaaa(), f.jj_mm_aaaa()),
        None => format!("à partir du {}", p.engagement.debut.jj_mm_aaaa()),
    }
}

/// CE QUI A ATTEINT LE SALARIE EST PARTI : d'un envoi coupe en route, les versions des seuls salaries servis (les
/// `servis` premiers messages, qui partent dans l'ordre), pour la ligne d'echec du journal.
fn versions_des_servis(versions: &[Version], messages: &[Message], servis: usize) -> Vec<Version> {
    let cles: Vec<&str> = messages[..servis.min(messages.len())].iter().map(|m| m.cle.as_str()).collect();
    versions.iter().filter(|v| cles.contains(&v.salarie.as_str())).cloned().collect()
}

/// `a, b et c`.
fn et_fr(morceaux: &[String]) -> String {
    if morceaux.len() < 2 {
        return morceaux.concat();
    }
    format!("{} et {}", morceaux[..morceaux.len() - 1].join(", "), morceaux[morceaux.len() - 1])
}

/// `code_signature` : `SIG-` puis six signes tires au hasard, sans 0/O ni 1/I/L, pour qu'il se relise sans
/// hesiter s'il faut le recopier a la main.
pub(crate) fn code_signature() -> R<String> {
    documents::fiche_questionnaire::code_au_hasard("SIG-").map_err(|m| m.reclasser(Classe::Disque))
}

/// `signataire` et `raison` des courriels : le prenom d'affichage du signataire (sinon son prenom, capitalise), et
/// le nom d'affichage de la compagnie (sinon sa raison sociale).
pub(crate) fn signataire_et_raison(ctx: &Contexte) -> R<(String, String)> {
    let sg = ctx.entreprise.signataire.dite().ok_or_else(|| requis_manque("signataire"))?;
    let signataire = match sg.prenom_affichage.portee() {
        Some(p) => p.to_string(),
        None => capitaliser(sg.prenom.portee().ok_or_else(|| requis_manque("prenom"))?),
    };
    let raison = match ctx.entreprise.nom_affichage.portee() {
        Some(r) => r.to_string(),
        None => ctx.entreprise.raison_sociale.ecrite("raison_sociale").map_err(|_| requis_manque("raison_sociale"))?.to_string(),
    };
    Ok((signataire, raison))
}

/// Le prenom d'affichage, sinon le prenom.
fn prenom_de(sal: &ChampsSalarie) -> R<String> {
    match sal.prenom_affichage.portee() {
        Some(p) => Ok(p.to_string()),
        None => exige(&sal.prenom, "prenom"),
    }
}

/// Le texte d'une case de la fiche, exigee.
fn exige<T: AsRef<str>>(c: &Option<T>, cle: &str) -> R<String> {
    c.portee().map(str::to_string).ok_or_else(|| socle::fichiers::absent(cle).reclasser(Classe::Manque))
}

/// « Prenom Nom » tel que la reponse « Je signe » le dit : `prenom_affichage or prenom`, `nom_affichage or nom`.
fn nom_complet_de(sal: &ChampsSalarie) -> R<String> {
    let nom = match sal.nom_affichage.portee() {
        Some(n) => n.to_string(),
        None => exige(&sal.nom, "nom")?,
    };
    Ok(format!("{} {}", prenom_de(sal)?, nom))
}

/// Ce qu'une phrase dit d'un contrat qui n'a pas de numero (`engagement::numeros::numero_dit`).
const SANS_NUMERO: &str = "(sans numéro)";

/// Ce que le nom d'un fichier porte pour un contrat qui n'a pas de numero.
const SANS_NUMERO_FICHIER: &str = "sans-numero";

/// Le numero d'un contrat tel que le nom d'un fichier le porte : le sien, ou qu'il n'en a pas.
fn numero_de_fichier(n: Option<&str>) -> String {
    match n.filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => SANS_NUMERO_FICHIER.to_string(),
    }
}

/// Le nom d'un salarie tel qu'une phrase le dit : celui de sa fiche, sinon sa cle.
fn nom_ou_cle(s: &ChampsSalarie, cle: &str) -> String {
    match s.nomme() {
        Some(n) => n,
        None => cle.to_string(),
    }
}

/// Ce que le contrat dit, clause par clause (l'avenant qui le modifiera doit citer l'ancien
/// texte a cote du nouveau). Un contrat qui ne s'ecrit pas se refuse.
pub(crate) fn clauses_du_contrat(ctx: &Contexte, p: &Paie, ent: &droit::contexte::EntrepriseDuDocument) -> R<std::collections::BTreeMap<String, Option<String>>> {
    let v = documents::contrat::valeurs_de_paie(ctx, ent, p)?;
    Ok(CLAUSES_MODIFIABLES.iter().map(|k| (k.nom().to_string(), v.get(k).cloned())).collect())
}

/// Le message d'une demande de signature : le texte, sa version HTML avec le bouton « Je signe », le PDF en piece
/// jointe. `liste` : ce que le courriel dit en premiere ligne ; `quoi` : ce que la reponse « Je signe » declare avoir
/// lu et accepte.
struct Demande<'a> {
    cle: &'a str,
    sal: &'a ChampsSalarie,
    adresse: &'a str,
    sujet: &'a str,
    liste: &'a str,
    quoi: &'a str,
    le_signer: &'a str,
    exemplaire: &'a str,
    avant_travail: &'a str,
    code: &'a str,
    expediteur: &'a str,
    signataire: &'a str,
    raison: &'a str,
}

fn message_de_signature(d: &Demande, pieces: Vec<PathBuf>) -> R<Message> {
    let prenom = prenom_de(d.sal)?;
    let reponse = envoi::formater(CORPS_REPONSE, &[("nom", nom_complet_de(d.sal)?), ("quoi", d.quoi.to_string()), ("code", d.code.to_string())]);
    let lien = mailto(d.expediteur, &format!("Re: {}", d.sujet), &reponse);
    let champs = |echappe: bool| -> Vec<(&'static str, String)> {
        let e = |s: &str| if echappe { html_echapper(s) } else { s.to_string() };
        vec![
            ("prenom", e(&prenom)),
            ("signataire", e(d.signataire)),
            ("raison_sociale", e(d.raison)),
            ("le_signer", e(d.le_signer)),
            ("expediteur", e(d.expediteur)),
            ("code", e(d.code)),
            ("exemplaire", e(d.exemplaire)),
            ("avant_travail", e(d.avant_travail)),
        ]
    };
    let mut texte = champs(false);
    texte.push(("contrats", d.liste.to_string()));
    let mut html = champs(true);
    html.push(("contrats", html_echapper(d.liste).replace('\n', "<br>")));
    html.push(("mailto", html_echapper(&lien)));
    Ok(Message {
        cle: d.cle.to_string(),
        nom: exige(&d.sal.nom, "nom")?,
        destinataire: d.adresse.to_string(),
        sujet: d.sujet.to_string(),
        corps: envoi::formater(CORPS_SIGNATURE, &texte),
        html: Some(envoi::formater(HTML_SIGNATURE, &html)),
        pieces,
    })
}

// ------------------------------------------------------------------ les contrats choisis

/// LES CONTRATS DEJA REVENUS SIGNES, par (salarie, numero), que le journal dit : le dernier mot est « recu », et le
/// contrat n'est pas reparti depuis sur une autre version (`etats::renvoyes_depuis_la_signature`).
pub(crate) fn contrats_revenus_signes(ctx: &Contexte) -> R<HashSet<envois::etats::Paire>> {
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let renvoyes = envois::etats::renvoyes_depuis_la_signature(&lignes);
    Ok(envois::etats::contrats_recus(&lignes).into_keys().filter(|k| !renvoyes.contains(k)).collect())
}

/// Les contrats choisis parmi `paies`, par salarie, dans l'ordre du mois : `(cle, ses paies, leur rang)`. Seuls partent
/// ceux qui se font signer ici (`engagement::contrat_a_signer`). UN CONTRAT DEJA REVENU SIGNE (`signes`) n'a plus rien a
/// signer : choisi par son salarie ou par le mois, il est ecarte ; vise lui-meme (`contrats`), il refuse la demande.
fn contrats_choisis(ctx: &Contexte, choisis: &Choisis, mois: Mois, paies: &[Paie], signes: &HashSet<envois::etats::Paire>) -> R<Vec<(String, Vec<Paie>, Vec<usize>)>> {
    let voulus = &choisis.salaries;
    let cibles = &choisis.contrats;
    // Une cible sans numero vise le contrat de ce salarie qui commence ce jour-la, quel que soit son numero.
    let vise = |c: &Cible, cle: &str, p: &Paie| c.0 == cle && c.1 == p.engagement.debut.iso() && c.2.as_ref().is_none_or(|n| p.engagement.numero.as_ref() == Some(n));
    let mut par_salarie: Vec<(String, Vec<Paie>, Vec<usize>)> = Vec::new();
    for (rang, p) in paies.iter().enumerate() {
        let cle = cle_de(p);
        if !engagement::contrat_a_signer(&ctx.racine, &p.engagement)? {
            continue;
        }
        if voulus.as_ref().is_some_and(|v| !v.is_empty() && !v.iter().any(|x| *x == cle)) {
            continue;
        }
        if cibles.as_ref().is_some_and(|c| !c.iter().any(|x| vise(x, &cle, p))) {
            continue;
        }
        if signes.contains(&(cle.clone(), p.engagement.numero.clone())) {
            if cibles.is_some() {
                return Err(refus(format!(
                    "Le contrat n° {} de {} est déjà revenu signé : il n'a plus rien à signer. Rien n'est parti.",
                    numero_dit(p.engagement.numero.as_ref(), "", "", SANS_NUMERO),
                    nom_ou_cle(&p.salarie, &cle)
                )));
            }
            continue;
        }
        match par_salarie.iter_mut().find(|(k, _, _)| *k == cle) {
            Some((_, ps, rs)) => {
                ps.push(p.clone());
                rs.push(rang);
            }
            None => par_salarie.push((cle.to_string(), vec![p.clone()], vec![rang])),
        }
    }
    if let Some(c) = &cibles {
        let mut manquants: Vec<&Cible> = c.iter().filter(|x| !par_salarie.iter().any(|(k, ps, _)| ps.iter().any(|p| vise(x, k, p)))).collect();
        manquants.sort();
        manquants.dedup();
        if !manquants.is_empty() {
            return Err(refus(format!(
                "Ces contrats ne sont pas dans la saisie de {} : {}. Rien n'est parti.",
                mois,
                manquants.iter().map(|x| format!("{} au {}{}", x.0, x.1, x.2.iter().map(|n| format!(" (n° {})", n)).collect::<String>())).collect::<Vec<_>>().join(", ")
            )));
        }
    }
    controler_contrat(ctx, par_salarie.iter().map(|(cle, ps, _)| (cle.as_str(), &ps[0].salarie.champs)))?;
    Ok(par_salarie)
}

/// CE QUE LE CONTRAT EXIGE DES FICHES (`exige_par` de `champs-base.json`, la liste que « Pour demarrer » lit), pour la
/// compagnie et ces `salaries` (`(cle, fiche)`) : tout ce qui manque, d'un coup, avant d'ecrire le moindre PDF. Un
/// avenant est un contrat : il passe par le meme controle.
fn controler_contrat<'a>(ctx: &'a Contexte, salaries: impl Iterator<Item = (&'a str, &'a ChampsSalarie)>) -> R<()> {
    use declarations::champs::Saisie;
    use droit::fiche::NomDeFiche;
    let mut saisies: Vec<(NomDeFiche, String, Saisie)> = vec![(NomDeFiche::Entreprise, "la fiche entreprise".into(), Saisie::de_la_fiche(NomDeFiche::Entreprise, &ctx.entreprise)?)];
    for (cle, s) in salaries {
        saisies.push((NomDeFiche::Salarie, format!("la fiche de {}", nom_ou_cle(s, cle)), Saisie::de_la_fiche(NomDeFiche::Salarie, s)?));
    }
    let fiches: Vec<(NomDeFiche, String, &Saisie)> = saisies.iter().map(|(n, q, s)| (*n, q.clone(), s)).collect();
    match declarations::champs::refus_des_manques("contrat", &fiches, &[])? {
        Some(m) => Err(refus(m)),
        None => Ok(()),
    }
}

/// Le controle du contrat (`controler_contrat`) pour ces avenants : la fiche de chaque salarie qui en recoit un.
pub(crate) fn controler_avenants(ctx: &Contexte, avenants: &[(IdSalarie, Vec<UnAvenant>)]) -> R<()> {
    controler_contrat(ctx, avenants.iter().filter_map(|(k, _)| ctx.salaries.get(k.as_str()).map(|s| (k.as_str(), &s.champs))))
}

/// Le contrat tel qu'il part, pour le journal : qui, quand, sous quel numero, dans quel fichier, et ce qu'il dit, clause
/// par clause, et le modele qu'il suit (`modele`, son empreinte, `modeles_figes`). La version de l'engagement sur
/// laquelle il part, la ligne du journal la porte (`journal::versions_des_contrats`).
fn entree_du_contrat(ctx: &Contexte, p: &Paie, cle: &str, pdf: &std::path::Path, titre: &str, mois: Mois, ent: &droit::contexte::EntrepriseDuDocument, modele: String) -> R<ContratParti> {
    Ok(ContratParti {
        salarie: IdSalarie::lire(cle)?,
        debut: p.engagement.debut,
        fin: p.engagement.fin,
        numero: p.engagement.numero.clone(),
        fichier: nom_du(pdf),
        spectacle: Some(titre.to_string()),
        mois,
        acte: None,
        avenant: None,
        remplace: None,
        effet: None,
        modifie: None,
        modifications: None,
        clauses: Some(clauses_du_contrat(ctx, p, ent)?),
        code: None,
        adresse: None,
        relance: None,
        empreinte: None,
        message: None,
        ailleurs: false,
        modele: Some(modele),
    })
}

/// LES MODELES QUE SUIVENT CES CONTRATS QUI PARTENT (`modeles_contrat::du_contrat`), chacun fige avec la compagnie : les
/// modeles, et leurs empreintes, que le journal note.
fn modeles_figes(ctx: &Contexte, ps: &[Paie]) -> R<(Vec<documents::contrat_ecrit::ModeleDuContrat>, Vec<String>)> {
    let mut modeles = Vec::new();
    let mut figes = Vec::new();
    for p in ps {
        let m = crate::modeles_contrat::du_contrat(ctx, p)?;
        figes.push(crate::modeles_contrat::figer(ctx, &m)?);
        modeles.push(m);
    }
    Ok((modeles, figes))
}

/// Le nom du PDF des contrats d'un salarie : `Contrat_<cle>_<numero>_<mois>.pdf`, `Contrats_..._<n1>+<n2>_...` quand il en
/// porte plusieurs (une page par contrat).
fn nom_du_pdf(cle: &str, ps: &[Paie], etiq: &str, suffixe: &str) -> String {
    let numeros: Vec<String> = ps.iter().map(|p| numero_de_fichier(p.engagement.numero.as_deref())).collect();
    format!("Contrat{}_{}_{}_{}{}.pdf", if ps.len() > 1 { "s" } else { "" }, cle, numeros.join("+"), etiq, suffixe)
}

// ------------------------------------------------------------------ contrats-imprimer
/// LE PDF A IMPRIMER : les pieces `pieces`, dans leur ordre, reunies en un seul fichier `<dossier>/Impression_<quoi>_<etiq>.pdf`
/// (une impression d'un mois remplace la precedente : les pieces parties gardent chacune leur fichier).
/// Le debut du nom du PDF a imprimer : ce fichier n'est la piece de personne, il ne se liste pas parmi les documents.
pub(crate) const PREFIXE_IMPRESSION: &str = "Impression_";

pub(crate) fn reunir_pour_imprimer(dossier: &std::path::Path, quoi: &str, etiq: &str, pieces: &[std::path::PathBuf]) -> R<std::path::PathBuf> {
    let octets = documents::fabrique::fusion_pieces(&pieces.iter().map(|p| (p.clone(), None)).collect::<Vec<_>>())?;
    let f = dossier.join(format!("{}{}_{}.pdf", PREFIXE_IMPRESSION, quoi, etiq));
    // Un mois dont seul un avenant s'imprime n'a pas encore son dossier d'envoi.
    std::fs::create_dir_all(dossier).map_err(|e| socle::echec::du_disque(&dossier, &e).reclasser(Classe::Disque))?;
    std::fs::write(&f, octets).map_err(|e| socle::echec::du_disque(&f, &e).reclasser(Classe::Disque))?;
    Ok(f)
}

/// Ce que rend `contrats_imprimer` : chaque piece partie (son chemin), le PDF a imprimer (son nom et son chemin), les
/// contrats et les avenants partis tels que le journal les note.
#[derive(Debug, serde::Serialize)]
pub(crate) struct Impression {
    pub fichiers: Vec<String>,
    pub impression: String,
    pub chemin: String,
    pub contrats: Vec<ContratParti>,
    pub avenants: Vec<ContratParti>,
}

/// La route `contrats-imprimer` : les contrats choisis (comme `demander-signature` ; `None` : aucun) et les avenants
/// choisis (`avenants`, deja verifies par la route), chacun dans son PDF dans `sorties/<mois>/envoi/` (la piece partie de
/// chacun : un PDF de contrats par salarie, un PDF par avenant), et tous reunis en un seul PDF a imprimer (`impression`),
/// a remettre en main propre. L'IMPRESSION EST UN DEPART : chaque contrat y part sous son numero, chaque avenant sous le
/// sien, et le journal le note (`contrat-signature`, `main_propre` ; une ligne pour les contrats, une pour les avenants) ;
/// ce qui revient signe se note ensuite d'un clic (`contrat-recu`). Rien ne quitte le poste : aucun mot de passe. `qui` :
/// la ligne du journal.
pub(crate) fn contrats_imprimer(ctx: &Contexte, mois: Mois, choisis: Option<&Choisis>, avenants: &[(IdSalarie, Vec<UnAvenant>)], qui: &str) -> R<Impression> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let (data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, mois, None, &departs)?;
    let s_mois = data.mois;
    // Sans contrats choisis, seuls les avenants s'impriment.
    let signes = contrats_revenus_signes(ctx)?;
    let par_salarie: Vec<(String, Vec<Paie>, Vec<usize>)> = choisis.map(|c| contrats_choisis(ctx, c, mois, &paies, &signes)).transpose()?.into_iter().flatten().collect();
    if par_salarie.is_empty() && avenants.is_empty() {
        return Err(refus(format!("Aucun contrat a imprimer pour {}.", mois)));
    }
    controler_avenants(ctx, avenants)?;
    let dossier = ctx.racine.join("sorties").join(s_mois.to_string()).join("envoi");
    let etiq = s_mois.etiquette();
    let ent = documents::avenant::ent_du_mois(ctx, s_mois);
    let (mut fichiers, mut contrats) = (Vec::new(), Vec::new());
    for (cle, ps, _) in &par_salarie {
        let pdf = dossier.join(nom_du_pdf(cle, ps, &etiq, ""));
        let (modeles, figes) = modeles_figes(ctx, ps)?;
        documents::contrat::ecrire_contrat(ctx, ps, &modeles, &ent, &pdf, &[])?;
        fichiers.push(pdf.to_string_lossy().into_owned());
        for (p, fige) in ps.iter().zip(figes) {
            contrats.push(entree_du_contrat(ctx, p, cle, &pdf, &p.titre()?, s_mois, &ent, fige)?);
        }
    }
    let mut saisies: HashMap<Mois, (engagement::saisie::VueDuMois, Vec<Paie>)> = HashMap::new();
    let mut lignes_avenants: Vec<ContratParti> = Vec::new();
    for (salarie, aa) in avenants {
        for a in aa {
            let (pdf, c, _) = avenant_ecrit(ctx, salarie, a, &mut saisies, &departs)?;
            fichiers.push(pdf.to_string_lossy().into_owned());
            lignes_avenants.push(c);
        }
    }
    let pdfs: Vec<std::path::PathBuf> = fichiers.iter().map(std::path::PathBuf::from).collect();
    let impression = reunir_pour_imprimer(&dossier, "contrats", &etiq, &pdfs)?;
    let le_mois = Some(mois);
    if !contrats.is_empty() {
        let detail = DetailSignature { contrats: contrats.clone(), main_propre: true, expediteur: None, relance: None, acte: None, partis: None, echec_a: None };
        let versions = envois::journal::versions_des_contrats(ctx, &contrats, &departs)?;
        let reponse = format!("{} contrat(s) imprime(s) pour une remise en main propre", contrats.len());
        noter(ctx, Ligne::ContratSignature(par_courriel(le_mois, qui, true, "", &reponse, detail, Some(versions))))?;
    }
    if !lignes_avenants.is_empty() {
        let detail = DetailSignature { contrats: lignes_avenants.clone(), main_propre: true, expediteur: None, relance: None, acte: Some("avenant".into()), partis: None, echec_a: None };
        let versions = envois::journal::versions_des_contrats(ctx, &lignes_avenants, &departs)?;
        let reponse = format!(
            "{} imprime(s) pour une remise en main propre",
            lignes_avenants.iter().map(|c| format!("avenant n° {} au contrat {}", numero_dit(c.avenant, "", "", SANS_NUMERO), numero_dit(c.numero.as_ref(), "", "", SANS_NUMERO))).collect::<Vec<_>>().join("; ")
        );
        noter(ctx, Ligne::ContratSignature(par_courriel(le_mois, qui, true, "", &reponse, detail, Some(versions))))?;
    }
    Ok(Impression { fichiers, impression: nom_du(&impression), chemin: impression.to_string_lossy().into_owned(), contrats, avenants: lignes_avenants })
}

// ------------------------------------------------------------------ demander-signature

/// Ce que rend `demander_signature` : les adresses servies, les contrats partis tels que le journal les note, et ce que
/// la demande dit des contrats qui ne partent pas (leur salarie n'a pas de courriel).
#[derive(Debug, Default, serde::Serialize)]
pub(crate) struct ContratsEnvoyes {
    pub partis: Vec<String>,
    pub contrats: Vec<ContratParti>,
    pub avertissements: Vec<String>,
}

/// Envoie a chaque salarie son contrat, avec le bouton « Je signe ».
///
/// `remplace` : le contrat refait EN ENTIER, qui annule et remplace celui qui est deja parti -- une phrase en tete
/// le dit. Refuse pour un contrat jamais parti, ou dont l'execution a commence : celui-la se modifie par un
/// avenant. UN CONTRAT QU'UN COURRIEL A DEJA DEMANDE (le journal le dit, `DemandeDeSignature::rappel`) repart TEL
/// QUEL, en rappel : objet « Rappel », une ligne de rappel, et le journal le dit, contrat par contrat ; ce qu'un rappel
/// refuse, `verifier_demande` le dit avant le mot de passe.
///
/// LA SELECTION EST PAR CONTRAT, pas par salarie (`contrats`). `salaries` reste accepte (tous les contrats de ces
/// salaries). UN COURRIEL PAR SALARIE, UN PDF PAR COURRIEL, tous ses contrats dedans (une page par contrat), ecrit
/// dans `sorties/<mois>/envoi/` avec les MEMES numeros que le fichier groupe.
pub(crate) fn demander_signature(ctx: &Contexte, d: &DemandeDeSignature, client: &mut dyn Courriel) -> R<ContratsEnvoyes> {
    let (mois, remplace) = (d.mois, d.remplace);
    // Chaque contrat sous le numero qu'il porte, ou celui qui lui serait donne : c'est celui-la qui part.
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let (data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, mois, None, &departs)?;
    let s_mois = data.mois;
    let dossier = ctx.racine.join("sorties").join(s_mois.to_string()).join("envoi");
    let etiq = s_mois.etiquette();

    let (an, nom_mois) = (s_mois.annee(), s_mois.nom(socle::mois::Graphie::Accentuee));
    let ent = documents::avenant::ent_du_mois(ctx, s_mois);
    // UN SALARIE SANS COURRIEL n'empeche pas les autres : son contrat ne part pas par courriel, il s'imprime ; les
    // autres partent. Les adresses se lisent avant d'ecrire le moindre PDF : un refus ne laisse rien sur le disque.
    let (par_salarie, noms_sans) = avec_et_sans_courriel(d.contrats(ctx, &paies)?)?;

    let mut remplacements: Vec<((String, Jour, String), (String, Jour))> = Vec::new();
    if remplace {
        let lignes = envois::journal::lire(&ctx.racine, None, None, false)?;
        for (k, ps, _) in &par_salarie {
            for p in ps {
                let debut = p.engagement.debut;
                // Le contrat qui part est remplace, lui (son numero) : un autre contrat du salarie n'a rien a remplacer.
                let Some(parti) = envois::etats::contrat_parti(&lignes, k, p.engagement.numero.as_deref()) else {
                    return Err(refus(format!("Le contrat de {} du {} n'est jamais parti : il n'a rien à remplacer, il s'envoie. Rien n'est parti.", k, debut.jj_mm_aaaa())));
                };
                if debut <= ctx.aujourdhui {
                    return Err(refus(format!(
                        "Le contrat de {} du {} a commencé : un contrat en cours d'exécution ne s'annule pas, il se modifie par un avenant. Rien n'est parti.",
                        k,
                        debut.jj_mm_aaaa()
                    )));
                }
                let numero = numero_dit(parti.numero.as_ref(), "", "", SANS_NUMERO);
                // Le contrat qui remplace se reconnait a son numero : sans numero, rien ne le designe.
                if let Some(propre) = p.engagement.numero.as_deref() {
                    remplacements.push(((k.clone(), debut, propre.to_string()), (numero, parti.date())));
                }
            }
        }
    }

    let (signataire, raison) = signataire_et_raison(ctx)?;
    let expediteur = client.compte().to_lowercase();
    let mut messages: Vec<Message> = Vec::new();
    let mut contrats: Vec<ContratParti> = Vec::new();
    for (cle, ps, _) in &par_salarie {
        let sal = &ps[0].salarie;
        let adresse = exige(&sal.email, "email")?.trim().to_lowercase();
        let code = code_signature()?;
        // Le contrat qui en remplace un autre porte son propre nom : le PDF parti avant lui reste sur le disque.
        let pdf = dossier.join(nom_du_pdf(cle, ps, &etiq, if remplace { "_remplace" } else { "" }));
        let (modeles, figes) = modeles_figes(ctx, ps)?;
        documents::contrat::ecrire_contrat(ctx, ps, &modeles, &ent, &pdf, &remplacements)?;
        // Le PDF tel qu'il part : son exemplaire contresigne commencera par ces octets-la (`depart::contresigner`).
        let empreinte = envois::journal::empreinte_fichier(&pdf).map_err(|m| m.reclasser(Classe::Disque))?;
        let mut spectacles: Vec<String> = Vec::new();
        let mut titres: Vec<String> = Vec::new();
        for p in ps {
            let t = p.titre()?;
            if !spectacles.contains(&t) {
                spectacles.push(t.clone());
            }
            titres.push(t);
        }
        let sujet_de = |spectacle: &str| envoi::formater(SUJET_SIGNATURE, &[("spectacle", spectacle.to_string()), ("mois", nom_mois.to_string()), ("annee", an.to_string())]);
        let du_au = |p: &Paie, titre: &str| -> R<String> { Ok(format!("{}, {}", titre, periode_de(p))) };
        let numero_de = |p: &Paie| numero_dit(p.engagement.numero.as_ref(), "", "", SANS_NUMERO);
        let (mut sujet, mut liste, le_signer, quoi, exemplaire);
        if ps.len() == 1 {
            let p = &ps[0];
            // Le contrat d'un spectacle se nomme par lui ; celui d'un permanent par sa nature (`contrat_ecrit.nom`).
            let nom_c = p.engagement.nat()?.nom_du_contrat();
            let pour = if p.spectacle.is_some() { format!(" pour {}", titres[0]) } else { String::new() };
            sujet = if p.spectacle.is_some() { sujet_de(&titres[0]) } else { format!("{} - {} {}", capitaliser(&nom_c), nom_mois, an) };
            liste = format!("Tu trouveras ci-joint ton {}{}, {}.", nom_c, pour, periode_de(p));
            le_signer = "le signer";
            quoi = format!("le {} n° {}{}, {}", nom_c, numero_de(p), pour, periode_de(p));
            exemplaire = "ton contrat";
        } else {
            sujet = if spectacles.len() == 1 {
                sujet_de(&spectacles[0])
            } else {
                envoi::formater(SUJET_SIGNATURE_PLURIEL, &[("mois", nom_mois.to_string()), ("annee", an.to_string())])
            };
            let mut lignes = Vec::new();
            for (p, t) in ps.iter().zip(&titres) {
                lignes.push(format!("  - {}", du_au(p, t)?));
            }
            liste = format!("Tu trouveras ci-joint tes {} contrats d'engagement, dans un seul document (un par page) :\n{}", ps.len(), lignes.join("\n"));
            le_signer = "les signer";
            let mut morceaux = Vec::new();
            for (p, t) in ps.iter().zip(&titres) {
                morceaux.push(format!("n° {} pour {}, {}", numero_de(p), t, periode_de(p)));
            }
            quoi = format!("les contrats d'engagement suivants : {}", morceaux.join(" ; "));
            exemplaire = "tes contrats";
        }
        let rappels: Vec<bool> = ps.iter().map(|p| d.rappel(cle, p.engagement.numero.as_deref())).collect();
        let pieces: Vec<(String, bool)> = ps.iter().zip(&rappels).map(|(p, r)| (format!("le contrat n° {}", numero_de(p)), *r)).collect();
        (sujet, liste) = rappel_du_courriel(sujet, liste, &pieces, if ps.len() == 1 { RAPPEL_RELANCE } else { RAPPEL_RELANCE_PLURIEL });
        if remplace {
            liste.push_str(&format!(
                "\n\n{} celui que tu as reçu avant, qui est réputé n'avoir jamais produit d'effet.",
                if ps.len() == 1 { "Il annule et remplace" } else { "Chacun annule et remplace" }
            ));
        }
        let message = message_de_signature(
            &Demande {
                cle,
                sal,
                adresse: &adresse,
                sujet: &sujet,
                liste: &liste,
                quoi: &quoi,
                le_signer,
                exemplaire,
                avant_travail: " Le contrat doit être signé avant le début du travail.",
                code: &code,
                expediteur: &expediteur,
                signataire: &signataire,
                raison: &raison,
            },
            vec![pdf.clone()],
        )?;
        messages.push(message);
        for (((p, t), fige), rappel) in ps.iter().zip(&titres).zip(figes).zip(&rappels) {
            let debut = p.engagement.debut;
            let mut c = entree_du_contrat(ctx, p, cle, &pdf, t, s_mois, &ent, fige)?;
            // Ce qu'il faut pour reconnaitre la reponse : le code, et l'adresse dont elle doit venir.
            c.code = Some(code.clone());
            c.adresse = Some(adresse.clone());
            c.relance = Some(*rappel);
            c.empreinte = Some(empreinte.clone());
            if remplace {
                c.acte = Some("remplace".into());
                c.remplace = p.engagement.numero.as_deref().and_then(|propre| remplacements.iter().find(|(k, _)| *k == (cle.clone(), debut, propre.to_string())).map(|(_, (n, _))| n.clone()));
            }
            contrats.push(c);
        }
    }

    if messages.is_empty() {
        return Err(refus(aucun_contrat_a_envoyer(mois)));
    }
    // La version de l'engagement sur laquelle chaque contrat part, lue avant l'envoi.
    let versions = envois::journal::versions_des_contrats(ctx, &contrats, &departs)?;
    let mut identifiants: Vec<String> = Vec::new();
    let partis = match envoi::envoyer_suivi(&messages, &expediteur, client, Some(&mut identifiants), None) {
        Ok(p) => p,
        Err(e) => {
            let servis = versions_des_servis(&versions, &messages, e.partis.len());
            let detail = DetailSignature { contrats: contrats.clone(), main_propre: false, expediteur: None, relance: None, acte: None, partis: None, echec_a: None };
            noter_echec_d_envoi(ctx, Ligne::ContratSignature, Some(mois), "interface", &e.partis, &e.echoue, &e.echec.phrase, detail, Some(servis))?;
            return Err(e.echec);
        }
    };
    // L'identifiant de chaque message parti, sur chacun de ses contrats : la releve l'ecarte de ses candidats.
    for c in contrats.iter_mut() {
        c.message = messages.iter().zip(&identifiants).find(|(m, _)| m.cle == c.salarie.as_str()).map(|(_, i)| i.clone());
    }
    let n_rappels = contrats.iter().filter(|c| c.relance == Some(true)).count();
    let detail = DetailSignature { contrats: contrats.clone(), main_propre: false, expediteur: Some(expediteur.clone()), relance: Some(n_rappels == contrats.len()), acte: None, partis: None, echec_a: None };
    let reponse = match n_rappels {
        0 => format!("{} contrat(s) envoye(s) a la signature", contrats.len()),
        n if n == contrats.len() => format!("{} contrat(s) relance(s) a la signature", n),
        n => format!("{} contrat(s) envoye(s) a la signature, dont {} relance(s)", contrats.len(), n),
    };
    noter(ctx, Ligne::ContratSignature(par_courriel(Some(mois), "interface", true, &partis.join(", "), &reponse, detail, Some(versions))))?;
    let avertissements: Vec<String> = if noms_sans.is_empty() { Vec::new() } else { vec![sans_courriel_dit(&noms_sans)] };
    Ok(ContratsEnvoyes { partis, contrats, avertissements })
}

/// Ce que la demande de signature et son apercu disent des contrats coches dont le salarie n'a pas de courriel : ils ne
/// partent pas par courriel, ils s'impriment pour se signer en main propre, ou le courriel se saisit dans la fiche.
pub(crate) fn sans_courriel_dit(noms: &[String]) -> String {
    if noms.len() == 1 {
        format!(
            "Pas de courriel dans la fiche de {} : son contrat ne part pas à la signature par courriel. Imprimez-le pour le faire signer en main propre, ou saisissez son courriel dans sa fiche.",
            noms[0]
        )
    } else {
        format!(
            "Pas de courriel dans les fiches de {} : leurs contrats ne partent pas à la signature par courriel. Imprimez-les pour les faire signer en main propre, ou saisissez leur courriel dans leur fiche.",
            noms.join(", ")
        )
    }
}

/// Ce que l'envoi des avenants et l'apercu de la signature disent des avenants coches dont le salarie n'a pas de
/// courriel : ils ne partent pas par courriel, ils s'impriment pour se signer en main propre, ou le courriel se saisit.
pub(crate) fn avenants_sans_courriel_dit(noms: &[String]) -> String {
    if noms.len() == 1 {
        format!(
            "Pas de courriel dans la fiche de {} : son avenant ne part pas à la signature par courriel. Imprimez-le pour le faire signer en main propre, ou saisissez son courriel dans sa fiche.",
            noms[0]
        )
    } else {
        format!(
            "Pas de courriel dans les fiches de {} : leurs avenants ne partent pas à la signature par courriel. Imprimez-les pour les faire signer en main propre, ou saisissez leur courriel dans leur fiche.",
            noms.join(", ")
        )
    }
}

/// LA DEMANDE DE SIGNATURE : le mois, ce qu'elle est (un contrat refait qui remplace celui qui est parti, ou non), les
/// contrats choisis, et l'etat de chaque contrat du mois tel que `signature-apercu` le lit au journal (demande, par
/// courriel ou non, revenu, caduc) : c'est lui qui dit, contrat par contrat, ce qui part en rappel.
pub(crate) struct DemandeDeSignature<'a> {
    pub mois: Mois,
    pub remplace: bool,
    pub choisis: Choisis,
    pub etats_des_contrats: &'a [crate::contrats::ContratASigner],
}

impl DemandeDeSignature<'_> {
    /// L'etat du contrat n° `numero` de `salarie`, tel que `signature-apercu` le montre.
    fn etat(&self, salarie: &str, numero: Option<&str>) -> Option<&crate::contrats::ContratASigner> {
        self.etats_des_contrats.iter().rev().find(|c| c.salarie == salarie && c.numero.as_deref() == numero)
    }

    /// UN RAPPEL : un courriel a deja demande de signer ce contrat (le journal le dit, `demande_par_courriel`). Un contrat
    /// refait qui annule et remplace est une autre piece : il n'est jamais un rappel.
    pub(crate) fn rappel(&self, salarie: &str, numero: Option<&str>) -> bool {
        !self.remplace && self.etat(salarie, numero).is_some_and(|c| c.demande_par_courriel)
    }

    /// LES CONTRATS QUE LA DEMANDE FAIT PARTIR : ceux qu'elle choisit (`contrats_choisis`), sans les contrats deja
    /// revenus signes, qui n'ont plus rien a signer ; un contrat refait qui annule et remplace peut remplacer un contrat
    /// signe. S'il ne reste rien, la demande le dit.
    fn contrats(&self, ctx: &Contexte, paies: &[Paie]) -> R<ParSalarie> {
        let signes = if self.remplace { HashSet::new() } else { contrats_revenus_signes(ctx)? };
        let choisis = contrats_choisis(ctx, &self.choisis, self.mois, paies, &signes)?;
        if choisis.is_empty() {
            return Err(refus(aucun_contrat_a_envoyer(self.mois)));
        }
        Ok(choisis)
    }
}

/// « Aucun contrat a envoyer pour 2026-11. » : la demande ne fait rien partir.
fn aucun_contrat_a_envoyer(mois: Mois) -> String {
    format!("Aucun contrat a envoyer pour {}.", mois)
}

/// Ce que la demande dit d'elle-meme avant que rien ne parte : ce qu'elle dira (`avertissements`), le nombre de contrats
/// qui partent, et combien en rappel.
pub(crate) struct DemandeVerifiee {
    pub avertissements: Vec<String>,
    pub contrats: usize,
    pub rappels: usize,
}

/// CE QUE LA DEMANDE REFUSE D'ELLE-MEME, avant la boite et avant le mot de passe : la preuve ne se demande que pour ce
/// qui part. Un rappel renvoie le meme contrat, et seulement un contrat pas revenu, a jour. Rien n'est ecrit. Un contrat
/// parti imprime peut partir par courriel : l'envoi ne bloque pas, il se dit (`deja_parti_dit`), et ces phrases sont
/// rendues.
pub(crate) fn verifier_demande(ctx: &Contexte, d: &DemandeDeSignature) -> R<DemandeVerifiee> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let (_, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, d.mois, None, &departs)?;
    let (par_salarie, _) = avec_et_sans_courriel(d.contrats(ctx, &paies)?)?;
    let cles: Vec<(String, Option<String>)> = par_salarie.iter().flat_map(|(k, ps, _)| ps.iter().map(|p| (k.clone(), p.engagement.numero.as_deref().map(String::from)))).collect();
    let mut verifiee = DemandeVerifiee { avertissements: Vec::new(), contrats: cles.len(), rappels: 0 };
    for (salarie, numero) in &cles {
        let numero = numero.as_deref();
        if d.rappel(salarie, numero) {
            verifier_rappel(d.etat(salarie, numero), numero)?;
            verifiee.rappels += 1;
        } else if let Some(dit) = deja_parti_dit(d.etats_des_contrats, salarie, numero) {
            verifiee.avertissements.push(dit);
        }
    }
    Ok(verifiee)
}

/// « Déjà parti, imprimé le 05/11/2026. » : ce que l'envoi par courriel d'un contrat deja parti dit, sans le refuser.
fn deja_parti_dit(etats: &[crate::contrats::ContratASigner], salarie: &str, numero: Option<&str>) -> Option<String> {
    let c = etats.iter().rev().find(|c| c.salarie == salarie && c.numero.as_deref() == numero)?;
    let quand = c.demande_le?.jour().jj_mm_aaaa();
    let comment = if c.par == Some("imprime") { "imprimé" } else { "envoyé" };
    Some(format!("Contrat n° {} de {} : déjà parti, {} le {}.", numero_dit(numero, "", "", SANS_NUMERO), c.nom, comment, quand))
}

/// LES CONTRATS QUI PARTENT PAR COURRIEL : ceux dont la fiche du salarie a un courriel ; les autres s'impriment (leurs
/// noms, pour le dire). Refus si aucun contrat choisi n'a de courriel : rien ne partirait.
type ParSalarie = Vec<(String, Vec<Paie>, Vec<usize>)>;
fn avec_et_sans_courriel(choisis: ParSalarie) -> R<(ParSalarie, Vec<String>)> {
    let (avec, sans): (ParSalarie, ParSalarie) = choisis.into_iter().partition(|(_, ps, _)| ps[0].salarie.email.is_some());
    let noms: Vec<String> = sans.iter().map(|(k, ps, _)| nom_ou_cle(&ps[0].salarie, k)).collect();
    if avec.is_empty() && !noms.is_empty() {
        return Err(refus(format!("{} Rien n'est parti.", sans_courriel_dit(&noms))));
    }
    Ok((avec, noms))
}

/// Un rappel ne vaut que pour un contrat a jour : il renvoie le meme contrat. Un contrat revenu signe n'arrive pas ici
/// (`DemandeDeSignature::contrats`).
fn verifier_rappel(c: Option<&crate::contrats::ContratASigner>, numero: Option<&str>) -> R<()> {
    match c {
        Some(c) if c.caduc => Err(refus(format!(
            "Le contrat n° {} de {} a changé depuis son envoi : il se modifie par un avenant, il ne se redemande pas. Rien n'est parti.",
            numero_dit(numero, "", "", SANS_NUMERO),
            c.nom
        ))),
        _ => Ok(()),
    }
}

// ------------------------------------------------------------------ les avenants

// ------------------------------------------------------------------ le contrat signe ailleurs

/// LE DOSSIER DU CONTRAT SIGNE HORS DU LOGICIEL (`engagement::signe_ailleurs`) du salarie `cle` commence le `debut` : la
/// compagnie y depose, si elle le veut, le PDF de ce contrat ; il est garde avec elle (la sauvegarde l'emporte) et part
/// joint a chaque avenant envoye a la signature, pour que le salarie l'ait sous les yeux. Le logiciel ne le lit pas.
pub(crate) fn dossier_du_contrat_signe(racine: &std::path::Path, cle: &str, debut: Jour) -> PathBuf {
    racine.join("salaries").join(cle).join(format!("contrat-du-{}", debut.iso()))
}

/// Les PDF deposes dans ce dossier, par ordre de nom.
pub(crate) fn contrat_signe_depose(racine: &std::path::Path, cle: &str, debut: Jour) -> R<Vec<PathBuf>> {
    Ok(socle::fichiers::entrees(&dossier_du_contrat_signe(racine, cle, debut))?
        .into_iter()
        .flatten()
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf")))
        .collect())
}

/// Ce que l'ecran et la ligne de commande disent du dossier du contrat signe hors du logiciel : son chemin (la fenetre
/// l'ouvre, et le cree), les PDF deposes, et la phrase.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ContratSigneDit {
    pub chemin: String,
    pub deposes: Vec<String>,
    pub phrase: String,
}

/// Ce dossier, dit (`ContratSigneDit`).
pub(crate) fn contrat_signe_dit(ctx: &Contexte, cle: &str, debut: Jour) -> R<ContratSigneDit> {
    let deposes: Vec<String> = contrat_signe_depose(&ctx.racine, cle, debut)?.iter().map(|p| nom_du(p)).collect();
    let qui = ctx.nomme(cle);
    let phrase = if deposes.is_empty() {
        format!("Le contrat de {} a été signé hors de PuppetPaie : son PDF, déposé dans son dossier, part joint à chaque avenant. Le dossier est vide.", qui)
    } else {
        format!("Le contrat signé de {} ({}) part joint à chaque avenant.", qui, deposes.join(", "))
    };
    Ok(ContratSigneDit { chemin: dossier_du_contrat_signe(&ctx.racine, cle, debut).to_string_lossy().into_owned(), deposes, phrase })
}

/// L'AVENANT A ENVOYER OU A IMPRIMER, verifie par la route : le contrat qu'il modifie (son salarie, son premier jour, le
/// mois ou il est parti, son numero et sa date), son numero d'avenant, son effet, ce qu'il change (`modifications`, que
/// `avenant-apercu` a montrees), le courriel du salarie ; un avenant parti et pas revenu (`repris`) repart tel quel.
#[derive(Debug, Clone)]
pub struct UnAvenant {
    pub salarie: IdSalarie,
    pub debut: Jour,
    pub mois: Mois,
    pub contrat: Option<NumeroContrat>,
    pub numero: i64,
    /// Le jour du contrat qu'il modifie ; aucun pour un avenant repris (la piece est deja ecrite).
    pub date_contrat: Option<Jour>,
    pub effet: Jour,
    pub modifications: Vec<ArticleModifie>,
    /// Le courriel du salarie, s'il est dit : sans lui, l'avenant ne part pas par courriel.
    pub destinataire: Option<String>,
    /// L'avenant deja parti et pas revenu, qui repart tel quel (`Repris`) ; aucun : un avenant neuf, ecrit ici.
    pub repris: Option<Repris>,
}

/// L'AVENANT DEJA PARTI ET PAS REVENU, qui repart tel quel : sa piece, dans `sorties/<mois>/envoi/`, et comment il est
/// deja parti, que le journal dit.
#[derive(Debug, Clone)]
pub struct Repris {
    pub fichier: String,
    pub deja: DejaParti,
}

/// Comment un avenant repris est deja parti : un courriel l'a deja demande (le redemander est un rappel), ou il n'est
/// parti qu'imprime, le jour dit (il part par courriel pour la premiere fois, sans « Rappel », et le dit).
#[derive(Debug, Clone)]
pub enum DejaParti {
    ParCourriel,
    Imprime { le: socle::paris::InstantOuJour },
}

impl UnAvenant {
    /// Un rappel : un courriel a deja demande de signer cette piece.
    pub fn rappel(&self) -> bool {
        matches!(self.repris, Some(Repris { deja: DejaParti::ParCourriel, .. }))
    }

    /// « Avenant n° 1 de Nom : déjà parti, imprimé le 05/11/2026. », quand il n'est parti qu'imprimé.
    pub(crate) fn deja_parti(&self, nom: &str) -> Option<String> {
        let Some(Repris { deja: DejaParti::Imprime { le }, .. }) = &self.repris else { return None };
        Some(format!("Avenant n° {} de {} : déjà parti, imprimé le {}.", self.numero, nom, le.jour().jj_mm_aaaa()))
    }

    /// Le numero du contrat qu'il modifie, ou « (sans numéro) ».
    fn contrat(&self) -> String {
        numero_dit(self.contrat.as_ref(), "", "", SANS_NUMERO)
    }

    /// Des termes dits en clair (un contrat signe hors du logiciel), pas des articles cites.
    fn en_clair(&self) -> bool {
        self.modifications.iter().any(ArticleModifie::en_clair)
    }
}

/// Les modifications telles que la fabrique de l'avenant les met en page (`documents::contrat::ecrire_avenant`).
pub(crate) fn pour_la_piece(modifications: &[ArticleModifie]) -> Vec<fabrique_pdf::forme::Modification> {
    modifications
        .iter()
        .map(|m| fabrique_pdf::forme::Modification {
            numero: m.numero.map(|n| n as i64),
            article: m.article.clone(),
            avant: m.avant.clone(),
            apres: m.apres.clone(),
            phrase: m.phrase.clone(),
        })
        .collect()
}

/// Ce qui se dit du contrat d'une paie dans un courriel et dans l'apercu de son avenant : son nom (`contrat_ecrit.nom`
/// de sa nature), « pour » son spectacle s'il en a un, sa periode.
pub(crate) struct ContratNomme {
    pub(crate) spectacle: String,
    pub(crate) nom_du_contrat: String,
    pub(crate) pour: String,
    pub(crate) periode: String,
}

impl ContratNomme {
    pub(crate) fn de(p: &Paie) -> R<ContratNomme> {
        let spectacle = p.titre()?;
        Ok(ContratNomme {
            nom_du_contrat: p.engagement.nat()?.nom_du_contrat(),
            pour: if p.spectacle.is_some() { format!(" pour {}", spectacle) } else { String::new() },
            periode: periode_de(p),
            spectacle,
        })
    }
}

/// L'AVENANT `a` DU CONTRAT DE `salarie`, ECRIT : son PDF dans `sorties/<mois>/envoi/` (la piece partie, a cote de celle
/// du contrat), et ce que le journal en dira. Un avenant repris repart sur son PDF parti, sans le reecrire. Rend le PDF, l'entree
/// du journal et la paie du contrat.
fn avenant_ecrit(
    ctx: &Contexte,
    salarie: &str,
    a: &UnAvenant,
    saisies: &mut HashMap<Mois, (engagement::saisie::VueDuMois, Vec<Paie>)>,
    departs: &engagement::departs::Departs,
) -> R<(PathBuf, ContratParti, Paie)> {
    let mois_a = a.mois;
    if !saisies.contains_key(&mois_a) {
        let (d, p) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, mois_a, None, departs)?;
        saisies.insert(mois_a, (d, p));
    }
    let (_, paies) = &saisies[&mois_a];
    // Un avenant designe son contrat par son numero : sans numero, il n'en designe aucun.
    let rang = match a.contrat.as_deref() {
        Some(numero) => envois::etats::paie_du_contrat(paies, salarie, numero)?,
        None => None,
    };
    let Some(rang_p) = rang else {
        return Err(refus(format!("Aucun contrat de {} commençant le {} dans les engagements de {}.", salarie, a.debut.jj_mm_aaaa(), mois_a)));
    };
    let p = &paies[rang_p];
    let ailleurs = engagement::signe_ailleurs(&ctx.racine, &p.engagement)?;
    let ent = documents::avenant::ent_du_mois(ctx, mois_a);
    let dossier = ctx.racine.join("sorties").join(mois_a.to_string()).join("envoi");
    let pdf = match &a.repris {
        Some(Repris { fichier, .. }) => {
            let f = dossier.join(fichier);
            if fichier.is_empty() || !f.is_file() {
                return Err(socle::echec::refus_avec(
                    format!("L'avenant n° {} de {} n'est plus dans les envois de son mois : il ne repart pas tel quel. Rien n'est parti.", a.numero, salarie),
                    socle::echec::Lieu { chemin: Some(dossier.to_string_lossy().into_owned()), fichier: Some(fichier.clone()), ..Default::default() },
                )
                .reclasser(Classe::Refuse));
            }
            f
        }
        None => {
            let f = dossier.join(format!("Avenant{}_{}_{}_{}.pdf", a.numero, salarie, numero_de_fichier(a.contrat.as_deref()), a.mois.etiquette()));
            // Le modele du contrat qu'il modifie, dont il suit l'avenant : celui avec lequel ce contrat est parti ; signe
            // hors du logiciel, celui que son engagement suit.
            let modele = if ailleurs {
                crate::modeles_contrat::du_contrat(ctx, p)?
            } else {
                let j = envois::journal::lu(&ctx.racine)?;
                let parti = envois::etats::contrat_parti(&j.lignes, salarie, a.contrat.as_deref());
                crate::modeles_contrat::du_contrat_parti(ctx, parti.as_ref().and_then(|x| x.modele.as_deref()), p)?
            };
            documents::contrat::ecrire_avenant(ctx, p, &ent, &f, &pour_la_piece(&a.modifications), a.numero, a.date_contrat.ok_or_else(|| erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : un avenant neuf ne dit pas la date du contrat qu'il modifie."))?, a.effet, ctx.aujourdhui, &modele, ailleurs)?;
            f
        }
    };
    let c = ContratParti {
        salarie: IdSalarie::lire(salarie)?,
        debut: a.debut,
        fin: p.engagement.fin,
        numero: a.contrat.clone(),
        fichier: nom_du(&pdf),
        spectacle: Some(p.titre()?),
        mois: a.mois,
        acte: Some("avenant".into()),
        avenant: Some(a.numero),
        remplace: None,
        effet: Some(a.effet),
        modifie: Some(a.modifications.iter().map(|m| m.article.clone()).collect()),
        modifications: Some(a.modifications.clone()),
        clauses: Some(clauses_du_contrat(ctx, p, &ent)?),
        code: None,
        adresse: None,
        relance: None,
        // Le PDF tel qu'il part (un avenant repris repart sur celui qui est parti) : son exemplaire contresigne commencera par ces
        // octets-la.
        empreinte: Some(envois::journal::empreinte_fichier(&pdf).map_err(|m| m.reclasser(Classe::Disque))?),
        message: None,
        // Un avenant au contrat signe hors du logiciel : la piece se refait de meme (`depart::refaire_le_contrat`), et le
        // PDF du contrat signe, depose dans son dossier, part avec lui.
        ailleurs,
        modele: None,
    };
    Ok((pdf, c, p.clone()))
}

/// Ce que `envoyer_avenants` rend : les adresses servies, et les avenants partis.
pub(crate) struct Avenants {
    pub partis: Vec<String>,
    pub avenants: Vec<ContratParti>,
}

/// Ecrit les avenants et les envoie a la signature, avec le bouton « Je signe » -- le meme circuit que le contrat. UN
/// COURRIEL PAR SALARIE : ses avenants en pieces jointes -- un PDF chacun, qui garde son numero -- et un seul code, dont la
/// reponse vaut pour tous. `par_salarie` : les avenants deja verifies par la route (chacun a un objet, une date d'effet
/// lisible, un destinataire).
///
/// Le journal `contrat-signature` porte une ligne par courriel, `acte: "avenant"` et le numero de chacun ;
/// la version de chaque engagement y est notee, et c'est elle qui dit ensuite que le contrat est a jour.
pub(crate) fn envoyer_avenants(ctx: &Contexte, par_salarie: &[(IdSalarie, Vec<UnAvenant>)], client: &mut dyn Courriel) -> R<Avenants> {
    let (signataire, raison) = signataire_et_raison(ctx)?;
    let expediteur = client.compte().to_lowercase();
    // La saisie de chaque mois, lue une fois : rien ne l'ecrit pendant l'envoi.
    let mut saisies: HashMap<Mois, (engagement::saisie::VueDuMois, Vec<Paie>)> = HashMap::new();
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let (mut partis, mut faits): (Vec<String>, Vec<ContratParti>) = (Vec::new(), Vec::new());
    for (salarie, aa) in par_salarie {
        let mois = aa[0].mois;
        let (an, nom_mois) = (mois.annee(), mois.nom(socle::mois::Graphie::Accentuee));
        let mut pdfs: Vec<PathBuf> = Vec::new();
        let mut lignes_contrats: Vec<ContratParti> = Vec::new();
        let mut nommes: Vec<ContratNomme> = Vec::new();
        let mut sal: Option<droit::fiche::SalarieDuContrat> = None;
        for a in aa {
            let (pdf, mut c, p) = avenant_ecrit(ctx, salarie, a, &mut saisies, &departs)?;
            nommes.push(ContratNomme::de(&p)?);
            sal = Some(p.salarie.clone());
            c.adresse = a.destinataire.as_deref().map(|d| d.trim().to_lowercase());
            c.relance = Some(a.rappel());
            lignes_contrats.push(c);
            pdfs.push(pdf);
        }
        let Some(sal) = sal else { continue };
        let Some(adresse) = lignes_contrats[0].adresse.clone() else {
            return Err(refus(format!("L'avenant de {} n'a pas de destinataire : son courriel manque. Rien n'est parti.", salarie)));
        };
        let code = code_signature()?;
        let mut spectacles: Vec<String> = Vec::new();
        for n in &nommes {
            if !spectacles.contains(&n.spectacle) {
                spectacles.push(n.spectacle.clone());
            }
        }
        // LE CONTRAT SIGNE HORS DU LOGICIEL : ses avenants le nomment par son premier jour (le numero que le logiciel lui
        // a donne n'est pas sur le papier) et disent en clair ce qui change ; le PDF depose dans son dossier part avec eux.
        let mut joints: Vec<PathBuf> = Vec::new();
        for (a, c) in aa.iter().zip(&lignes_contrats) {
            if c.ailleurs {
                joints.extend(contrat_signe_depose(&ctx.racine, salarie, a.debut)?);
            }
        }
        let contrat_dit = |a: &UnAvenant, n: &ContratNomme| -> String {
            if a.en_clair() { format!("{} commencé le {}", n.nom_du_contrat, a.debut.jj_mm_aaaa()) } else { format!("{} n° {}", n.nom_du_contrat, a.contrat()) }
        };
        let un = |a: &UnAvenant, n: &ContratNomme| -> String {
            let ce_qui_change = if a.en_clair() {
                // Chaque phrase, sans son point, sa premiere lettre en minuscule : elles se suivent dans une seule.
                let phrases: Vec<String> = a
                    .modifications
                    .iter()
                    .filter_map(|m| m.phrase.as_deref())
                    .map(|p| p.trim_end_matches('.'))
                    .map(|p| p.chars().take(1).flat_map(char::to_lowercase).chain(p.chars().skip(1)).collect())
                    .collect();
                format!("qui dit : {}", phrases.join(" ; "))
            } else {
                format!(
                    "qui modifie {}",
                    et_fr(&a.modifications.iter().map(|m| format!("l'article {}« {} »", m.numero.iter().map(|x| format!("{} ", x)).collect::<String>(), m.article)).collect::<Vec<_>>())
                )
            };
            // Le contrat signe ailleurs se nomme deja par son premier jour : sa periode ne se redit pas.
            let periode = if a.en_clair() { String::new() } else { format!(", {}", n.periode) };
            format!("l'avenant n° {} à ton {}{}{}, {}", a.numero, contrat_dit(a, n), n.pour, periode, ce_qui_change)
        };
        let sujet_de = |spectacle: &str| envoi::formater(SUJET_SIGNATURE, &[("spectacle", spectacle.to_string()), ("mois", nom_mois.to_string()), ("annee", an.to_string())]);
        let (mut sujet, mut liste, quoi);
        if aa.len() == 1 {
            let (a, n) = (&aa[0], &nommes[0]);
            sujet = if n.pour.is_empty() {
                format!("Avenant n° {} - {} - {} {}", a.numero, capitaliser(&n.nom_du_contrat), nom_mois, an)
            } else {
                format!("Avenant n° {} - {}", a.numero, sujet_de(&n.spectacle))
            };
            liste = format!("Tu trouveras ci-joint {} ; le reste du contrat ne change pas.", un(a, n));
            quoi = format!("l'avenant n° {} au {}{}", a.numero, contrat_dit(a, n), n.pour);
        } else {
            sujet = format!(
                "Avenants - {}",
                if spectacles.len() == 1 {
                    sujet_de(&spectacles[0])
                } else {
                    envoi::formater(SUJET_SIGNATURE_PLURIEL, &[("mois", nom_mois.to_string()), ("annee", an.to_string())])
                }
            );
            let morceaux: Vec<String> = aa.iter().zip(&nommes).map(|(a, n)| format!("  - {} ;", un(a, n))).collect();
            liste = format!("Tu trouveras ci-joint {} avenants à tes contrats, un document chacun :\n{}\nLe reste de chaque contrat ne change pas.", aa.len(), morceaux.join("\n"));
            quoi = format!(
                "les avenants suivants : {}",
                aa.iter().zip(&nommes).map(|(a, n)| format!("n° {} au {}{}", a.numero, contrat_dit(a, n), n.pour)).collect::<Vec<_>>().join(" ; ")
            );
        }
        let pieces: Vec<(String, bool)> = aa.iter().map(|a| (format!("l'avenant n° {}", a.numero), a.rappel())).collect();
        (sujet, liste) = rappel_du_courriel(sujet, liste, &pieces, if aa.len() == 1 { RAPPEL_RELANCE_AVENANT } else { RAPPEL_RELANCE_AVENANTS });
        if !joints.is_empty() {
            liste = format!("{}\nTon contrat signé est joint aussi, pour que tu l'aies sous les yeux.", liste);
        }
        let message = message_de_signature(
            &Demande {
                cle: salarie,
                sal: &sal,
                adresse: &adresse,
                sujet: &sujet,
                liste: &liste,
                quoi: &quoi,
                le_signer: if aa.len() == 1 { "le signer" } else { "les signer" },
                exemplaire: if aa.len() == 1 { "ton avenant" } else { "tes avenants" },
                avant_travail: "",
                code: &code,
                expediteur: &expediteur,
                signataire: &signataire,
                raison: &raison,
            },
            pdfs.iter().chain(&joints).cloned().collect(),
        )?;
        for c in lignes_contrats.iter_mut() {
            c.code = Some(code.clone());
        }
        let versions = envois::journal::versions_des_contrats(ctx, &lignes_contrats, &departs)?;
        let le_mois = Some(mois);
        let mut identifiants: Vec<String> = Vec::new();
        let partis_ici = match envoi::envoyer_suivi(std::slice::from_ref(&message), &expediteur, client, Some(&mut identifiants), None) {
            Ok(p) => p,
            Err(e) => {
                let servis = versions_des_servis(&versions, std::slice::from_ref(&message), e.partis.len());
                let detail = DetailSignature { contrats: lignes_contrats.clone(), main_propre: false, expediteur: None, relance: None, acte: None, partis: None, echec_a: None };
                noter_echec_d_envoi(ctx, Ligne::ContratSignature, le_mois, "interface", &e.partis, &e.echoue, &e.echec.phrase, detail, Some(servis))?;
                return Err(e.echec);
            }
        };
        for c in lignes_contrats.iter_mut() {
            c.message = identifiants.first().cloned();
        }
        let reponse = format!(
            "{} envoye(s) a la signature",
            aa.iter().map(|a| format!("avenant n° {} au contrat {}{}", a.numero, numero_dit(a.contrat.as_ref(), "", "", SANS_NUMERO), if a.rappel() { " (relance)" } else { "" })).collect::<Vec<_>>().join("; ")
        );
        let detail = DetailSignature {
            contrats: lignes_contrats.clone(),
            main_propre: false,
            expediteur: Some(expediteur.clone()),
            relance: Some(aa.iter().all(UnAvenant::rappel)),
            acte: Some("avenant".into()),
            partis: None,
            echec_a: None,
        };
        noter(ctx, Ligne::ContratSignature(par_courriel(le_mois, "interface", true, &partis_ici.join(", "), &reponse, detail, Some(versions))))?;
        partis.extend(partis_ici);
        faits.extend(lignes_contrats);
    }
    Ok(Avenants { partis, avenants: faits })
}
