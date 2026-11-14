//! Les routes des documents du mois : ce que la boite montre avant l'envoi (`envoi-apercu`), les envoyer par courriel
//! (`envoyer-documents`) ou les imprimer (`documents-imprimer`) ; l'un et l'autre sont un depart (bulletins et
//! certificats numerotes, une ligne au journal, la paie figee). Et ce que les envois partagent avec `contrats` et
//! `archives` : la boite qui envoie (`client_courriel`), les lectures de la charge.
//!
//! L'apercu ne fait que lire : il nomme les pieces dues a chacun, que l'envoi fabrique.
//!
//! Chaque charge et chaque reponse est une forme declaree ici, lue une fois (`Charge::lire`) ; l'horloge d'une
//! compagnie fictive appelle les memes fonctions, sur les memes formes.

use droit::fiche::Lue as _;
use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use compagnie::preuve::{verifier_preuve, Preuve};
use crate::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, ILLISIBLE, MANQUE, MOIS, OPERATION};
use crate::catalogue::Route;
use crate::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use crate::etapes::{ChargeMois, Moteur};
use compagnie::INTERFACE;
use socle::ecran::Pastille;
use droit::contexte::{Contexte, IdSalarie};
use envois::courriel::{BoiteSimulee, Courriel};
use envois::envoi;
use envois::journal;
use envois::ligne_d_envoi::{en_liste, noter, noter_echec_d_envoi, par_courriel};
use socle::echec::{erreur, Classe, Echec};
use socle::fichiers::{CheminRelatif, NomDeFichier};
use socle::mois::{Graphie, Mois, MoisOuJour};
use crate::catalogue::{Ecran, Place};

pub(crate) type R<T> = Result<T, Echec>;

/// De quoi ouvrir la boite : appele UNE fois par envoi. Les tests passent un faux client du trait `Courriel`, ou la
/// boite simulee ; une route ouvre la vraie (`client_courriel`).
pub(crate) type Ouvrir<'a> = &'a mut dyn FnMut() -> R<Box<dyn Courriel>>;

const SALARIES: Champ = Champ { nom: "salaries", libelle: Affiche::new("Salariés"), forme: Forme::Liste, sens: "seulement ceux-ci (les clés de leurs fiches) ; tous sinon", avance: false };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::EnvoiApercu,
        geste: "Montrer les documents du mois de chaque salarié, et à qui ils partiraient.",
        charge: &[MOIS],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "pret, empechements, informations, avertissements, boite_liee, expediteur, messages (cle, nom, destinataire, pieces, renvoi, changements), deja",
        refus: &[MANQUE, ILLISIBLE],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::EnvoyerDocuments,
        geste: "Envoyer à chaque salarié ses documents du mois par courriel ; la paie se fige.",
        charge: &[MOIS, SALARIES],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "phrase, partis (les salariés servis), avertissements (ceux sans courriel, la boîte qui envoie)",
        refus: &[
            Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe du logiciel, tapé par un humain au moment de l'envoi ; rien n'est écrit" },
            Refus { classe: Classe::AucunDocument, corriger: "valider un engagement du mois" },
            Refus { classe: Classe::BoiteAbsente, corriger: "poser la boîte courriel (Configuration, Mots de passe), ou imprimer" },
            OPERATION,
        ],
        acte: Acte::Engage,
        phrase: 300,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::DocumentsImprimer,
        geste: "Imprimer les documents du mois, à remettre en main propre ; la paie se fige.",
        charge: &[MOIS, SALARIES],
        exemple: r#"{"mois":"2026-11","salaries":["SVOBODA"]}"#,
        rend: "impression (le PDF, dans sorties/<mois>/) et son chemin, salaries, avertissements",
        refus: &[Refus { classe: Classe::AucunDocument, corriger: "valider un engagement du mois" }, OPERATION],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::PiecePartie,
        geste: "Dire où est une pièce partie d'un mois, refaite à l'identique si elle manque.",
        charge: &[MOIS, Champ { nom: "fichier", libelle: Affiche::new("Pièce"), forme: Forme::Texte, sens: "le nom de la pièce, tel que le journal le dit", avance: false }],
        exemple: r#"{"mois":"2026-11","fichier":"DPAE_Novembre2026.txt"}"#,
        rend: "chemin (relatif à la compagnie)",
        refus: &[MANQUE, OPERATION],
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Paie),
    },
];

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::EnvoiApercu => en_json(envoi_apercu(ctx, charge.lire::<ChargeMois>()?.mois)?),
        Route::EnvoyerDocuments => en_json(envoyer_documents(ctx, charge.lire()?, charge.mot_de_passe(), &mut || client_courriel(ctx))?),
        Route::DocumentsImprimer => en_json(documents_imprimer(ctx, charge.lire()?, INTERFACE)?),
        Route::PiecePartie => en_json(piece_partie(ctx, charge.lire()?)?),
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

impl Dite for ApercuEnvoi {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for DocumentsEnvoyes {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for DocumentsImprimes {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for PiecePartie {
    fn phrase(&self) -> &str {
        self.phrase
    }
}

// ------------------------------------------------------------------ les formes

/// La charge de `documents-imprimer` (et de l'horloge d'une compagnie fictive, pour ceux qui n'ont pas de courriel) :
/// le mois, les salaries choisis (absent : tous).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeDocumentsImprimer {
    pub(crate) mois: Mois,
    pub(crate) salaries: Option<Vec<IdSalarie>>,
}

/// Ce que rend `documents-imprimer` : sa phrase, le PDF a imprimer et son chemin, les salaries servis, ce que la fabrique
/// a dit (aussi en `phrases`).
#[derive(Debug, Serialize)]
pub(crate) struct DocumentsImprimes {
    phrase: String,
    impression: String,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    chemin: std::path::PathBuf,
    salaries: Vec<IdSalarie>,
    avertissements: Vec<String>,
    phrases: Vec<String>,
}

/// Ce que rend `envoyer-documents` : sa phrase, les adresses servies, ce qui n'est pas parti (pas de courriel) et la
/// boite qui envoie.
#[derive(Debug, Serialize)]
pub(crate) struct DocumentsEnvoyes {
    phrase: String,
    partis: Vec<droit::fiche::Courriel>,
    avertissements: Vec<String>,
}

/// La charge de `piece-partie` : le mois, et la piece telle que le journal la nomme.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargePiecePartie {
    mois: Mois,
    fichier: NomDeFichier,
}

/// Ce que rend `piece-partie` : le chemin de la piece, relatif a la compagnie.
#[derive(Debug, Serialize)]
struct PiecePartie {
    phrase: &'static str,
    chemin: CheminRelatif,
}

/// Un message de l'apercu : a qui, quelles pieces (leur nom de fichier, et leur sorte, ce que l'ecran en dit), et, pour un
/// renvoi de documents corriges, sa pastille et ce qui a change.
#[derive(Debug, Serialize)]
struct MessageVu {
    cle: IdSalarie,
    nom: String,
    /// Le courriel de la fiche ; vide, ses documents s'impriment.
    destinataire: Option<droit::fiche::Courriel>,
    pieces: Vec<NomDeFichier>,
    sortes: Vec<&'static str>,
    renvoi: bool,
    pastille: Option<Pastille>,
    changements: Vec<String>,
}

/// Le dernier depart des documents, quand tous sont deja partis.
#[derive(Debug, Serialize)]
struct DejaPartis {
    quand: socle::paris::InstantOuJour,
    quand_dit: String,
}

/// Ce que rend `envoi-apercu`.
#[derive(Debug, Serialize)]
struct ApercuEnvoi {
    phrase: String,
    expediteur: String,
    expediteur_dit: String,
    avertissements: Vec<String>,
    messages: Vec<MessageVu>,
    deja: Option<DejaPartis>,
    pret: bool,
    empechements: Vec<String>,
    informations: Vec<String>,
    boite_liee: bool,
    phrases: Vec<String>,
}

// ------------------------------------------------------------------ piece-partie

/// `piece-partie` : le chemin de la piece partie, refaite si son fichier manque (`depart::piece_partie`), relatif a la
/// compagnie (`sorties/<mois>/[envoi/]<fichier>`).
fn piece_partie(ctx: &Contexte, c: ChargePiecePartie) -> R<PiecePartie> {
    let f = crate::gestes::depart::piece_partie(ctx, c.mois, &c.fichier)?;
    let chemin = CheminRelatif::sous(&ctx.racine, &f).map_err(|e| {
        let lieu = socle::echec::Lieu { detail: Some(e.phrase), ..Default::default() };
        Echec { a_part: Some(lieu.into()), ..erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : la pièce partie n'est pas dans le dossier de la compagnie.") }
    })?;
    Ok(PiecePartie { phrase: "La pièce partie, telle qu'elle est partie.", chemin })
}

// ------------------------------------------------------------------ les outils

/// Les noms d'un dossier ; un dossier absent n'en a aucun, un dossier qui ne se lit pas se refuse.
fn noms_du_dossier(dossier: &Path) -> R<Vec<String>> {
    match std::fs::read_dir(dossier) {
        Ok(l) => l.map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()).map_err(|x| socle::echec::du_disque(dossier, &x).reclasser(Classe::du_disque(&x)))).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(socle::echec::du_disque(dossier, &e).reclasser(Classe::du_disque(&e))),
    }
}

/// Y a-t-il dans le dossier un nom qui commence par `prefixe` et finit par `suffixe` ? Un dossier absent n'en a pas.
pub(crate) fn un_nom_commence_et_finit(dossier: &Path, prefixe: &str, suffixe: &str) -> R<bool> {
    Ok(noms_du_dossier(dossier)?.iter().any(|n| n.len() >= prefixe.len() + suffixe.len() && n.starts_with(prefixe) && n.ends_with(suffixe)))
}


// ------------------------------------------------------------------ signature-apercu


// ------------------------------------------------------------------ envoi-apercu

/// Le dossier des retours ou vont les attestations qu'aucune fiche ne porte.
const A_RANGER: &str = "aer-a-ranger";

/// Le nom d'une piece dans l'archive d'un retour, sans ses dossiers.
fn nom_dans_l_archive(chemin: &str) -> &str {
    match chemin.rsplit_once('/') {
        Some((_, nom)) => nom,
        None => chemin,
    }
}

/// Une attestation d'un retour de nature 46, rattachee ou non a une fiche.
struct Attestation {
    nom_archive: String,
    qui: String,
    donnees: Vec<u8>,
    cle: Option<droit::contexte::IdSalarie>,
}

/// Une attestation d'un retour, par sa periode (`None` : le retour ne la dit pas) et le nom qu'elle porte.
type CleAttestation = (Option<Mois>, String);

/// (periode, qui) -> les attestations du DERNIER retour qui le porte, lues
/// dans les retours 46, dans l'ordre de l'archive. `mois` : seulement cette periode.
fn attestations_archivees(ctx: &Contexte, mois: Option<Mois>, seulement: Option<&[std::path::PathBuf]>) -> R<HashMap<CleAttestation, Vec<Attestation>>> {
    let dossier = ctx.racine.join(envois::retours::RETOURS);
    // `fichiers` : seulement ces archives (la releve range ce qu'elle vient de telecharger).
    let mut fichiers: Vec<String> = match seulement {
        Some(liste) => liste.iter().map(|f| NomDeFichier::du(f).map(String::from)).collect::<R<_>>()?,
        None => noms_du_dossier(&dossier)?.into_iter().filter(|n| n.contains("_nature46_")).collect(),
    };
    fichiers.sort();
    let index: Vec<(droit::contexte::IdSalarie, String)> =
        ctx.salaries.iter().map(|(cle, v)| (cle.clone(),[v.nom.portee(), v.prenom.portee()].into_iter().flatten().collect::<Vec<_>>().join(" "))).collect();
    let mut vues: HashMap<CleAttestation, Vec<Attestation>> = HashMap::new();
    for nom in fichiers {
        let chemin = dossier.join(&nom);
        let octets = crate::depots::lire_octets(&chemin.to_string_lossy())?;
        let liste = envois::retours::aer_spectacle(&octets)?;
        let cles = envois::retours::rattacher(&liste.iter().map(|a| a.qui.as_str()).collect::<Vec<_>>(), &index);
        let mut de_ce_retour: HashMap<CleAttestation, Vec<Attestation>> = HashMap::new();
        for (a, cle) in liste.into_iter().zip(cles) {
            if let Some(m) = mois {
                if a.periode != Some(m) {
                    continue;
                }
            }
            de_ce_retour.entry((a.periode, a.qui.clone())).or_default().push(Attestation { nom_archive: a.nom_archive, qui: a.qui, donnees: a.donnees, cle });
        }
        vues.extend(de_ce_retour);
    }
    Ok(vues)
}

/// `aer_manquantes` : les salaries dont France Travail a renvoye l'AER du mois, mais dont le PDF signe
/// n'est pas dans `sorties/<mois>/envoi/`.
fn aer_manquantes(ctx: &Contexte, mois: Mois) -> R<Vec<String>> {
    let dossier = ctx.racine.join("sorties").join(mois.to_string()).join("envoi");
    let etiquette = mois.etiquette();
    let mut out: Vec<String> = Vec::new();
    for groupe in attestations_archivees(ctx, Some(mois), None)?.values() {
        let Some(att) = groupe.first() else { continue };
        if !att.cle.as_ref().is_some_and(|cle| dossier.join(envoi::nom_fichier("AER", cle, &etiquette)).exists()) {
            // Rattachee, la cle de sa fiche ; sinon, le nom que l'attestation porte.
            let nom = match &att.cle {
                Some(cle) => cle.to_string(),
                None => att.qui.clone(),
            };
            if !out.contains(&nom) {
                out.push(nom);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Les paies de `periode`, sous les numeros de contrat que la DSN a DECLARES (regle 4 : un numero se lit --
/// `ecrire_documents::paies_aux_numeros_declares`) ; ce qui les empeche de se lire se refuse.
fn paies_de_la_periode(ctx: &Contexte, periode: Mois) -> R<Vec<engagement::calcul::Paie>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let (_, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, periode, None, &departs)?;
    Ok(paies)
}

/// Toutes les periodes que l'AER de `cle` pour `periode` doit porter (`documents::aer::periodes_attendues`). `None` : la
/// saisie de la periode n'est pas dans le logiciel (un mois d'avant lui) -- rien a quoi comparer. Les paies d'un mois qui
/// ne se lisent pas refusent la comparaison.
fn periodes_attendues(ctx: &Contexte, par_mois: &mut HashMap<Mois, Vec<engagement::calcul::Paie>>, periode: Mois, cle: &droit::contexte::IdSalarie) -> R<Option<Vec<documents::aer::PaieAer>>> {
    if !engagement::stock::a_une_saisie(&ctx.racine, periode)? {
        return Ok(None);
    }
    // `documents::aer` demande les paies d'un mois sans refus possible : le premier refus est garde, et rendu apres.
    let mut faute: Option<Echec> = None;
    let attendues = {
        let mut du_mois = |m: Mois| -> Vec<engagement::calcul::Paie> {
            if let Some(p) = par_mois.get(&m) {
                return p.clone();
            }
            match paies_de_la_periode(ctx, m) {
                Ok(p) => {
                    par_mois.insert(m, p.clone());
                    p
                }
                Err(e) => {
                    faute.get_or_insert(e);
                    Vec::new()
                }
            }
        };
        documents::aer::periodes_attendues(periode, cle, &mut du_mois)
    };
    if let Some(e) = faute {
        return Err(e);
    }
    attendues.map(Some)}

/// `chemin_identite("cachet")` : le tampon pose, quelle que soit son extension (png, jpg, svg).
fn cachet_pose(gabarits: &Path) -> Option<String> {
    let dossier = gabarits.join("identite");
    [".png", ".jpg", ".svg"].iter().map(|e| dossier.join(format!("cachet{}", e))).find(|f| f.exists()).map(|f| f.to_string_lossy().into_owned())
}

/// `RangementAer` : ce que le rangement des AER a fait -- les PDF ecrits `(periode, cle, chemin)`, les attestations sans salarie
/// deposees a part `(qui, chemin)`, les refusees `(cle, periode, ecarts)`.
#[derive(Default)]
pub struct RangementAer {
    pub ecrits: Vec<(Mois, droit::contexte::IdSalarie, std::path::PathBuf)>,
    pub a_ranger: Vec<(String, std::path::PathBuf)>,
    pub refusees: Vec<(droit::contexte::IdSalarie, Mois, Vec<String>)>,
}

/// `ranger_aer(ctx, mois=mois)` : signe et range les AER deja archivees la ou l'envoi ira les chercher -- en phrases, pour la releve
/// et l'ouverture (qui ne signent jamais malgre un ecart).
pub(crate) fn ranger_aer(ctx: &Contexte, mois: Option<Mois>, fichiers: Option<&[std::path::PathBuf]>) -> R<Vec<String>> {
    let fait = ranger_aer_detail(ctx, mois, fichiers, false)?;
    let mut phrases: Vec<String> = Vec::new();
    // `RangementAer.a_faire()` : d'abord les attestations sans salarie, puis les refusees (la paie ne les confirme pas).
    for (qui, _) in &fait.a_ranger {
        phrases.push(format!("Attestation France Travail de {} : aucun salarié ne porte ce nom. Déposée dans les retours, dossier « {} » ; vérifier la fiche.", qui, A_RANGER));
    }
    for (cle, periode, ecarts) in &fait.refusees {
        phrases.push(format!("Attestation France Travail de {} ({}) NON SIGNEE : elle ne dit pas ce que la paie dit ({}). La correction se fait sur la DSN, pas sur le PDF.", cle, periode, ecarts.join("; ")));
    }
    Ok(phrases)
}

/// Le rangement des AER revenues et ce qu'il a laisse. `forcer` : signer malgre un ecart (`aer --forcer`).
pub fn ranger_aer_detail(ctx: &Contexte, mois: Option<Mois>, fichiers: Option<&[std::path::PathBuf]>, forcer: bool) -> R<RangementAer> {
    use documents::aer::{authentifier, Authentification, Echec as AerEchec};
    let vues = attestations_archivees(ctx, mois, fichiers)?;
    let mut par_mois: HashMap<Mois, Vec<engagement::calcul::Paie>> = HashMap::new();
    let mut cles: Vec<&CleAttestation> = vues.keys().collect();
    cles.sort();
    // `RangementAer.a_faire()` : d'abord les attestations sans salarie, puis les refusees (la paie ne les confirme pas).
    let mut fait = RangementAer::default();
    for k in cles {
        let groupe = &vues[k];
        let (Some(periode), Some(cle)) = (k.0, groupe[0].cle.clone()) else {
            // On ne devine pas : un nom que le rapprochement ne reconnait pas est peut-etre un salarie archive.
            let refuge = ctx.racine.join(envois::retours::RETOURS).join(A_RANGER);
            std::fs::create_dir_all(&refuge).map_err(|e| socle::echec::du_disque(&refuge, &e).reclasser(Classe::du_disque(&e)))?;
            for att in groupe {
                let chemin = refuge.join(nom_dans_l_archive(&att.nom_archive));
                std::fs::write(&chemin, &att.donnees).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::du_disque(&e)))?;
                fait.a_ranger.push((att.qui.clone(), chemin));
            }
            continue;
        };
        // LA VERIFICATION SE FAIT SUR TOUTES LES ATTESTATIONS DU SALARIE A LA FOIS : au-dela de cinq periodes, France
        // Travail en edite plusieurs, et chacune ne porte qu'une partie des periodes attendues.
        let mut imprime: Vec<documents::aer::Ligne> = Vec::new();
        for att in groupe {
            let champs = documents::aer::champs(&att.donnees).map_err(|e| match e {
                AerEchec::Fabrique(e) | AerEchec::Autre(e) => e,
                AerEchec::Ecarts(e) => socle::echec::refus(e.join("; ")),
            })?;
            imprime.extend(documents::aer::lire_quantites(&champs)?);
        }
        if let Some(attendues) = periodes_attendues(ctx, &mut par_mois, periode, &cle)? {
            let ecarts = documents::aer::ecarts(&imprime, &attendues);
            if !ecarts.is_empty() && !forcer {
                // Une seule fausse et rien ne part.
                fait.refusees.push((cle, periode, ecarts));
                continue;
            }
        }
        let signature = documents::bulletin::identite_de(&ctx.gabarits(), "signature").map(|f| f.to_string_lossy().into_owned());
        let cachet = cachet_pose(&ctx.gabarits());
        let ent = &ctx.entreprise;
        let mut signees: Vec<Vec<u8>> = Vec::new();
        for att in groupe {
            let a = Authentification {
                ent,
                aujourdhui: ctx.aujourdhui,
                signature: signature.as_deref(),
                // Deja verifiee ci-dessus, sur l'ensemble des attestations du salarie.
                paies: None,
                forcer,
                cachet_fichier: cachet.as_deref().map(std::path::Path::new),
            };
            match authentifier(&att.donnees, &a) {
                Ok(octets) => signees.push(octets),
                Err(AerEchec::Ecarts(e)) => return Err(socle::echec::refus(e.join("; "))),
                Err(AerEchec::Fabrique(e) | AerEchec::Autre(e)) => return Err(e),
            }
        }
        let donnees = if signees.len() == 1 { signees.remove(0) } else { documents::fabrique::fusion_octets(&signees, &std::env::temp_dir())? };
        let cible = ctx.racine.join("sorties").join(periode.to_string()).join("envoi");
        std::fs::create_dir_all(&cible).map_err(|e| erreur(Classe::Disque, e.to_string()))?;
        let chemin = cible.join(envoi::nom_fichier("AER", &cle, &periode.etiquette()));
        std::fs::write(&chemin, donnees).map_err(|e| erreur(Classe::Disque, e.to_string()))?;
        fait.ecrits.push((periode, cle, chemin));
    }
    Ok(fait)
}

/// Ce qui est du a chaque salarie du mois et ne se fabrique pas ici : l'AER Cinema Spectacle (`aer::due`), que France
/// Travail publie apres la DSN (retour 46), et l'attestation employeur de droit commun (`aer::attestation_fctu`), qu'elle
/// edite apres le signalement de fin de contrat. Par salarie : ce que son courriel dit absent.
fn dues_a_part(paies: &[engagement::calcul::Paie]) -> R<HashMap<IdSalarie, Vec<&'static str>>> {
    let mut out: HashMap<IdSalarie, Vec<&'static str>> = HashMap::new();
    for p in paies {
        let siennes = out.entry(p.salarie.id.clone()).or_default();
        if documents::aer::due(p)? && !siennes.contains(&"aer") {
            siennes.push("aer");
        }
        if documents::aer::attestation_fctu(&p.engagement)? && !siennes.contains(&"attestation_fctu") {
            siennes.push("attestation_fctu");
        }
    }
    Ok(out)
}

/// Peut-on envoyer les documents du mois ? Ce qui l'empeche, ce qui est a savoir, et si une boite est posee.
struct Prets {
    empechements: Vec<String>,
    informations: Vec<String>,
    boite_liee: bool,
}

/// `documents_prets` : peut-on envoyer les documents du mois ? Le moteur repond, et dit pourquoi.
fn documents_prets(ctx: &Contexte, mois: Mois, paies: &[engagement::calcul::Paie]) -> R<Prets> {
    // LES DOCUMENTS PARTENT DES QU'UN ENGAGEMENT EST VALIDE : ils se fabriquent a leur envoi, depuis sa paie ecrite.
    let mut empechements: Vec<String> = Vec::new();
    if paies.is_empty() {
        empechements.push("Aucun engagement valide ce mois-ci : rien ne part d'un engagement en saisie.".to_string());
    }
    // L'AER NE SE FABRIQUE PAS : France Travail la renvoie apres la DSN, et elle part avec les documents quand elle est la.
    let mut informations: Vec<String> = Vec::new();
    if compagnie::etat_dsn::du_mois(ctx, mois, None)?.aer_attendue() && dues_a_part(paies)?.values().any(|v| v.contains(&"aer")) {
        informations.push(
            "L'attestation employeur (AER) de France Travail n'est pas encore revenue : France Travail l'édite à partir de la DSN, et chaque relevé des retours la cherche. Elle partira avec un envoi suivant."
                .to_string(),
        );
    }
    let manquent = aer_manquantes(ctx, mois)?;
    if !manquent.is_empty() {
        informations.push(format!(
            "L'attestation employeur (AER) est revenue de France Travail, mais elle n'est pas signee pour {} : la paie ne confirme pas ce qu'elle affiche, ou le nom n'est pas reconnu. Voir les messages de l'ouverture.",
            manquent.join(", ")
        ));
    }
    Ok(Prets { empechements, informations, boite_liee: crate::ouverture::boite_liee(ctx)? })
}

/// Un message prepare pour un salarie : ce que l'ecran montre avant l'envoi.
struct Message {
    cle: IdSalarie,
    nom: String,
    destinataire: Option<droit::fiche::Courriel>,
    pieces: Vec<std::path::PathBuf>,
}

/// Un message par salarie, et les pieces qui lui sont DUES, nommees sans etre fabriquees : le bulletin d'une paie qui a des
/// cachets, le certificat Conges Spectacles d'un contrat qui finit (`certificats_dus`), l'AER que France Travail a rendue
/// et qui est rangee. Le corps n'est pas compose : seuls ses refus (`Manque` d'une fiche entreprise sans nom
/// d'affichage ou sans signataire) sont gardes.
fn preparer(ctx: &Contexte, paies: &[engagement::calcul::Paie], mois: Mois, dossier: &Path) -> R<Vec<Message>> {
    let etiq = mois.etiquette();
    let ent = &ctx.entreprise;
    let requis = |c: &Option<String>, k: &str| if c.is_none() { Err(socle::fichiers::absent(k).reclasser(Classe::Manque)) } else { Ok(()) };
    requis(&ent.nom_affichage, "nom_affichage")?;
    let sg = ent.signataire.dite().ok_or_else(|| socle::fichiers::absent("signataire").reclasser(Classe::Manque))?;
    if sg.prenom_affichage.remplie().is_none() {
        requis(&sg.prenom, "prenom")?;
    }
    let (certificats, _) = crate::dossier_salarie::certificats_dus(ctx, paies)?;
    let mut messages: Vec<Message> = Vec::new();
    for s in paies.iter().map(|p| &p.salarie) {
        let cle = s.id.clone();
        if messages.iter().any(|m| m.cle == cle) {
            continue;
        }
        let de_lui = |p: &&engagement::calcul::Paie| p.salarie.id == cle;
        let mut pieces = Vec::new();
        if paies.iter().filter(de_lui).any(|p| !p.engagement.sans_paie()) {
            pieces.push(dossier.join(envoi::nom_fichier("Bulletin", &cle, &etiq)));
        }
        if certificats.iter().any(|(c, _, pc)| *c == cle && pc.is_some()) {
            pieces.push(dossier.join(envoi::nom_fichier("CertificatCS", &cle, &etiq)));
        }
        let aer = dossier.join(envoi::nom_fichier("AER", &cle, &etiq));
        if aer.exists() {
            pieces.push(aer);
        }
        messages.push(Message {
            nom: ctx.nomme(&cle),
            // Une fiche sans courriel se montre : ses documents s'impriment ; c'est l'envoi par courriel qui la refuse.
            destinataire: s.email.dite().map(|e| droit::fiche::Courriel::lire(&e.to_lowercase())).transpose()?,
            cle,
            pieces,
        });
    }
    Ok(messages)
}

/// Ce qui peut partir de `mois` (`engagement::etats::validees_du_mois` : la saisie des engagements valides et leurs paies
/// ecrites), numerote. L'envoi des documents (`envoyer_documents`) prepare ses messages sur les memes paies que l'apercu.
pub(crate) fn charger_le_mois(ctx: &Contexte, mois: Mois) -> R<Vec<engagement::calcul::Paie>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    // Un mois sans engagement valide n'a aucune paie.
    let mut paies: Vec<engagement::calcul::Paie> = engagement::etats::validees_du_mois(ctx, mois, &departs)?.into_iter().flat_map(|(_, paies)| paies).collect();
    documents::ecrire_documents::numeroter(ctx, &mut paies, mois.annee(), &departs)?;
    Ok(paies)
}

/// `apercu_envoi` : ce que chaque salarie recevrait, sans rien envoyer.
fn envoi_apercu(ctx: &Contexte, le_mois: Mois) -> R<ApercuEnvoi> {
    let mois = le_mois;
    // Ce qui peut partir du mois, numerote.
    let paies = charger_le_mois(ctx, mois)?;
    let m = Moteur::nouveau(ctx);
    let prets = documents_prets(ctx, mois, &paies)?;
    let pret = prets.empechements.is_empty();
    let dossier = ctx.racine.join("sorties").join(mois.to_string()).join("envoi");
    // A QUI ILS PARTENT : le meme choix que l'envoi (`choisir`).
    let choix = choisir(ctx, mois, &paies)?;
    // CE QUI PARTIRA, nomme sans etre fabrique : les documents s'ecrivent, numerotes, a l'envoi (`envoyer_documents`).
    let messages = if pret { preparer(ctx, &paies, mois, &dossier)? } else { Vec::new() };
    let tous_a_jour = messages.iter().all(|x| choix.a_jour.contains(&x.cle));
    let messages = choix.garder(messages, |x| &x.cle);
    let (mut expediteur, mut avertissements) = (String::new(), Vec::<String>::new());
    if prets.boite_liee {
        // La boite qui enverrait, quand des documents sont dus.
        if !messages.is_empty() {
            expediteur = compte_de_la_boite(ctx)?.to_lowercase();
            let sg = ctx.entreprise.signataire.dite().ok_or_else(|| socle::fichiers::absent("signataire").reclasser(Classe::Manque))?;
            if let Some(signataire) = sg.email.portee().map(str::to_lowercase).filter(|s| *s != expediteur) {
                avertissements.push(format!(
                    "Les messages partiront de {} (la boite posee), pas de {} (le courriel du signataire, fiche Entreprise).",
                    expediteur, signataire
                ));
            }
        }
        let sans: Vec<String> = messages.iter().filter(|x| x.destinataire.is_none()).map(|x| nom_de_la_paie(&paies, &x.cle)).collect();
        if !sans.is_empty() {
            avertissements.push(sans_courriel_dit(&sans.join(", ")));
        }
    }
    let lignes = m.lignes()?;
    // Ce qui est deja parti : la meme lecture que la case Documents de l'engagement (`etapes::documents_partis`).
    let partis = crate::etapes::documents_partis(ctx, mois, &m.etats_du_mois(le_mois)?, &lignes)?;
    // « Deja envoye » ne se dit que d'un renvoi voulu, a l'identique : tous ont deja leurs documents, et leur paie n'a
    // pas change. Un renvoi de documents corriges ou un premier envoi n'envoie rien une seconde fois.
    let deja = messages
        .iter()
        .filter_map(|x| partis.get(&x.cle))
        .map(|p| p.quand)
        .max()
        .filter(|_| tous_a_jour)
        .map(|q| DejaPartis { quand: q, quand_dit: format!("Le {}.", q.dit()) });
    let messages: Vec<MessageVu> = messages
        .into_iter()
        .map(|x| {
            let renvoi = choix.renvois.get(&x.cle);
            let pieces = x.pieces.iter().map(|p| NomDeFichier::du(p)).collect::<R<Vec<_>>>()?;
            Ok(MessageVu {
                // Ce que l'ecran en dit : la sorte de chaque piece, pas son nom de fichier.
                sortes: pieces.iter().map(|p| crate::historique_mois::sorte_de(p.texte())).collect(),
                pieces,
                renvoi: renvoi.is_some(),
                // Un renvoi : des documents corriges, qui remplacent ceux deja partis (a faire partir).
                pastille: renvoi.map(|r| Pastille::de(socle::ecran::EtatDEcran::AFaire, &format!("documents corrigés — remplacent ceux du {}", r.envoye_le.jj_mm_aaaa()))),
                changements: renvoi.into_iter().flat_map(changements_dits).collect(),
                cle: x.cle,
                nom: x.nom,
                destinataire: x.destinataire,
            })
        })
        .collect::<R<_>>()?;
    let mut phrases: Vec<String> = messages
        .iter()
        .map(|x| match &x.destinataire {
            Some(d) => format!("{} ({}) : {}", x.nom, d, x.sortes.join(", ")),
            None => format!("{} : {}", x.nom, x.sortes.join(", ")),
        })
        .collect();
    phrases.extend(prets.empechements.iter().chain(&avertissements).chain(&prets.informations).cloned());
    Ok(ApercuEnvoi {
        phrase: format!("Documents à envoyer par courriel, de {} : {}.", expediteur, socle::ecran::compte(messages.len(), "message", "messages")),
        expediteur_dit: format!("Courriels depuis {}", expediteur),
        expediteur,
        avertissements,
        messages,
        deja,
        pret,
        empechements: prets.empechements,
        informations: prets.informations,
        boite_liee: prets.boite_liee,
        phrases,
    })
}

/// Ce que l'ecran dit d'un renvoi : chaque ligne de ce qui a change, ou que le resume de la paie n'a pas bouge.
fn changements_dits(r: &envoi::Renvoi) -> Vec<String> {
    if r.changements.is_empty() {
        vec![envois::envoi::RIEN_DANS_LE_RESUME.trim_start_matches([' ', '-']).to_string()]
    } else {
        r.changements.iter().map(|l| l.trim_start().trim_start_matches("- ").to_string()).collect()
    }
}

/// L'adresse de la boite qui enverrait. Celle de la compagnie fictive est le courriel du signataire (sa fiche
/// Entreprise le demande) ; sinon l'adresse posee avec le mot de passe (jamais le mot de passe), ou celle de
/// `COURRIEL_COMPTE` quand elle est posee.
fn compte_de_la_boite(ctx: &Contexte) -> R<String> {
    if compagnie::bridage::branchement(&ctx.racine)?.joue() {
        let mail = ctx.entreprise.signataire.dite().and_then(|s| s.email.remplie()).map(String::from);
        return mail.ok_or_else(|| erreur(Classe::Manque, "Le courriel du signataire manque (fiche Entreprise) : la compagnie fictive envoie de lui."));
    }
    let a = match std::env::var("COURRIEL_COMPTE") {
        Ok(a) => a,
        Err(std::env::VarError::NotPresent) => {
            let coffre = compagnie::coffre::Coffre::du_poste(&ctx.racine);
            coffre.compte_pose("courriel")?.ok_or_else(|| {
                erreur(Classe::Manque, "Le compte de la boîte courriel n'est pas posé : route coffre-poser, service courriel, compte <adresse>.")
            })?
        }
        Err(e) => {
            let lieu = socle::echec::Lieu { detail: Some(e.to_string()), ..Default::default() };
            return Err(Echec { a_part: Some(lieu.into()), ..erreur(Classe::Illisible, "Le compte de la boîte courriel posé dans l'environnement ne se lit pas.") });
        }
    };
    Ok(a.trim().to_string())
}

// ------------------------------------------------------------------ ce que toutes les routes partagent

/// `Refuse` : un refus ordinaire, avec sa phrase.
pub(crate) fn refus(message: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, message)
}

/// La boite qui envoie, selon le branchement de la compagnie (`bridage::branchement`) : le simulateur ecrit les messages
/// sans les envoyer ; le message refuse, avant la preuve et avant toute ligne au journal des depots ; le monde prend la
/// vraie boite, et sans mot de passe d'application au coffre, il n'y en a pas (`BoiteAbsente`).
pub(crate) fn client_courriel(ctx: &Contexte) -> R<Box<dyn Courriel>> {
    let branchement = compagnie::bridage::branchement(&ctx.racine)?;
    branchement.ouvrir()?;
    if branchement.joue() {
        return Ok(Box::new(BoiteSimulee::du_contexte(ctx)?));
    }
    if !compagnie::boite::disponible(&ctx.racine)? {
        return Err(socle::echec::cle("Aucune boîte courriel : poser le mot de passe d'application de la boîte dans Configuration › Mots de passe.", "courriel").reclasser(Classe::BoiteAbsente));
    }
    let client = compagnie::boite::client_du_contexte(ctx)?;
    Ok(Box::new(client))
}

/// Les identifiants de la saisie : `expediteur`, la boite qui envoie, en minuscules ; une adresse que le journal garde.
pub(super) fn expediteur_de(client: &dyn Courriel) -> R<droit::fiche::Courriel> {
    droit::fiche::Courriel::lire(&client.compte().to_lowercase())
}

// ------------------------------------------------------------------ envoyer-documents

/// La charge de `envoyer-documents` : le mois, les salaries choisis (tous sinon), et le mot de passe du logiciel tape
/// pour l'envoi.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeEnvoyerDocuments {
    mois: Mois,
    salaries: Option<Vec<IdSalarie>>,
}

/// Envoie a chaque salarie ses documents du mois, sur le mot de passe du logiciel tape pour l'envoi. Irreversible.
fn envoyer_documents(ctx: &Contexte, c: ChargeEnvoyerDocuments, mot_de_passe: Option<&socle::secret::Secret>, ouvrir: Ouvrir) -> R<DocumentsEnvoyes> {
    let preuve = Preuve::du_mot_de_passe(mot_de_passe.cloned());
    let prepare = preparer_envoi(ctx, c.mois, c.salaries.as_deref(), Some(&preuve), ouvrir)?;
    expedier_envoi(ctx, prepare, c.mois, &preuve, INTERFACE)
}

/// `envoyer_documents(ctx, mois, preuve=..., qui=...)` : la meme operation, `qui` et la preuve donnes (l'horloge de
/// la compagnie fictive passe `horloge` et une boite muette).
pub(crate) fn envoyer_documents_de(ctx: &Contexte, mois: Mois, preuve: &Preuve, qui: &str, ouvrir: Ouvrir) -> R<DocumentsEnvoyes> {
    let prepare = preparer_envoi(ctx, mois, None, Some(preuve), ouvrir)?;
    expedier_envoi(ctx, prepare, mois, preuve, qui)
}

/// Les pieces d'un salarie, dans l'ordre ou elles s'impriment : bulletin, certificat Conges Spectacles, attestation
/// France Travail (celles qui sont ecrites).
const PIECES_A_IMPRIMER: [&str; 3] = ["Bulletin", "CertificatCS", "AER"];

/// La route `documents-imprimer` : les documents du mois des salaries choisis (`salaries`, tous sinon), fabriques comme
/// pour l'envoi par courriel (`depart::fabriquer_documents`), reunis en un seul PDF a imprimer, salarie par salarie, et
/// remis en main propre. L'IMPRESSION EST UN DEPART : ce qui est remis au salarie est parti. Les numeros des bulletins et
/// des certificats s'ecrivent dans les paies, le journal note `documents` (`main_propre`) avec la version de chaque
/// engagement, et la paie est figee comme par le courriel. Rien ne quitte le poste : aucun mot de passe.
pub(crate) fn documents_imprimer(ctx: &Contexte, c: ChargeDocumentsImprimer, qui: &str) -> R<DocumentsImprimes> {
    let mois = c.mois;
    let paies = charger_le_mois(ctx, mois)?;
    if paies.is_empty() {
        return Err(erreur(
            Classe::AucunDocument,
            format!("{} : aucun engagement valide, aucun document a imprimer. Rien ne part d'un engagement en saisie.", mois.libelle(Graphie::Accentuee)),
        ));
    }
    let mut cles: Vec<IdSalarie> = Vec::new();
    let mut noms: HashMap<IdSalarie, String> = HashMap::new();
    for p in &paies {
        let cle = p.salarie.id.clone();
        if cles.contains(&cle) || c.salaries.as_ref().is_some_and(|l| !l.iter().any(|s| cle == *s)) {
            continue;
        }
        noms.insert(cle.clone(), nom_du_salarie(&p.salarie));
        cles.push(cle);
    }
    if cles.is_empty() {
        return Err(refus("Aucun salarié coché. Rien n'est imprimé."));
    }
    let choix = choisir(ctx, mois, &paies)?;
    let documents = crate::gestes::depart::fabriquer_documents(ctx, mois, &cles, qui)?;
    let dossier = ctx.racine.join("sorties").join(mois.to_string()).join("envoi");
    let etiq = c.mois.etiquette();
    let mut pieces: Vec<std::path::PathBuf> = Vec::new();
    let mut salaries: Vec<journal::Servi> = Vec::new();
    for cle in &cles {
        let siennes: Vec<std::path::PathBuf> =
            PIECES_A_IMPRIMER.iter().map(|prefixe| dossier.join(format!("{}_{}_{}.pdf", prefixe, cle, etiq))).filter(|f| f.is_file()).collect();
        salaries.push(journal::Servi {
            cle: cle.clone(),
            nom: noms.get(cle).cloned(),
            pieces: siennes.iter().map(|p| NomDeFichier::du(p)).collect::<R<_>>()?,
            resume: choix.resumes.get(cle).cloned(),
            renvoi: choix.renvois.contains_key(cle),
        });
        pieces.extend(siennes);
    }
    if pieces.is_empty() {
        return Err(refus(format!("Aucun document écrit pour {} : rien à imprimer.", mois.libelle(Graphie::Accentuee))));
    }
    let versions = journal::versions_du_mois(ctx, mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?, |cle, _, _| cles.contains(cle))?;
    let impression = crate::gestes::signature::reunir_pour_imprimer(&dossier, "documents", &etiq, &pieces)?;
    let nom_impression = NomDeFichier::du(&impression)?;
    let avertissements = documents.avertissements.clone();
    documents.engager(ctx, mois)?;
    let detail = journal::DetailDocuments { salaries, main_propre: true, expediteur: None, durees_ms: None, partis: None, echec_a: None };
    let reponse = format!("documents de {} salarie(s) imprimes pour une remise en main propre", cles.len());
    noter(ctx, journal::Ligne::Documents(par_courriel(Some(c.mois), qui, true, "", &reponse, detail, Some(versions))))?;
    let qui_les_recoit: Vec<String> = cles.iter().filter_map(|cle| noms.get(cle).map(|nom| nom_a_l_ecran(ctx, cle, nom))).collect();
    Ok(DocumentsImprimes {
        phrase: format!("Documents {} imprimés pour {}, à remettre en main propre.", socle::texte::de(&mois.libelle(Graphie::Accentuee)), qui_les_recoit.join(", ")),
        impression: nom_impression.into(),
        chemin: impression,
        salaries: cles,
        phrases: avertissements.clone(),
        avertissements,
    })
}

/// Le nom du salarie `cle` tel que l'ecran le montre (`routes::nom_a_l_ecran`), lu de sa fiche ; quand la fiche n'est
/// plus la, celui que sa paie porte (`de_la_paie`).
fn nom_a_l_ecran(ctx: &Contexte, cle: &IdSalarie, de_la_paie: &str) -> String {
    match ctx.salaries.get(cle) {
        Some(s) => super::nom_a_l_ecran(s, cle),
        None => de_la_paie.to_string(),
    }
}

/// Le nom d'un salarie tel que sa paie le porte (`ChampsSalarie::nomme`) ; une fiche sans nom se designe par sa cle.
fn nom_du_salarie(s: &droit::fiche::SalarieDuContrat) -> String {
    super::nom_a_l_ecran(s, &s.id)
}

/// Les messages, le compte verifie -- rien n'est parti.
pub(crate) struct EnvoiPrepare {
    pub(crate) messages: Vec<envoi::Message<IdSalarie>>,
    pub(crate) expediteur: droit::fiche::Courriel,
    pub(crate) avertissements: Vec<String>,
    /// A qui les documents partent, et pourquoi (`choisir`) : le journal en garde l'empreinte et le resume.
    pub(crate) choix: Choix,
    /// Les documents fabriques pour ceux a qui ils partent ; leurs numeros s.ecrivent quand ils sont partis.
    documents: crate::gestes::depart::DocumentsFabriques,
    client: Box<dyn Courriel>,
    /// La preuve deja verifiee avant la fabrique (`preparer_envoi` avec une preuve) : l.expedition ne la redemande pas.
    preuve: Option<Preuve>,
    /// La duree de la preparation (lecture du mois, messages, boite), en millisecondes : le journal la garde.
    preparation_ms: u128,
}
/// A QUI LES DOCUMENTS DU MOIS PARTENT. Chaque salarie du mois est dans l'un de trois cas, lus dans le journal
/// (`crate::etapes::documents_partis` : ce que l'engagement sait de ce qui est parti) et dans ses engagements :
///
/// * ses documents ne sont jamais partis : ils partent, avec le premier courriel ;
/// * ils sont partis, et la paie d'un de ses engagements a change depuis la version sur laquelle ils sont partis
///   (`engagement::etats::a_refaire`, un bulletin rectificatif) : ils repartent refaits, avec le courriel qui dit ce qui
///   a change (`envoi::CORPS_RENVOI`, `envoi::ce_qui_a_change` : ce que dit la frise, puis le brut et le net) ;
/// * ils sont partis et sa paie n'a pas bouge : ils ne repartent pas.
///
/// Quand TOUS sont a jour, l'envoi est un renvoi voulu, a l'identique : tous repartent, avec le premier courriel.
#[derive(Default)]
pub(crate) struct Choix {
    /// Ceux dont la paie a change depuis l'envoi de leurs documents : le jour de l'envoi, ce qui a change.
    pub(crate) renvois: HashMap<IdSalarie, envoi::Renvoi>,
    /// Ceux dont les documents sont partis et dont la paie n'a pas change.
    pub(crate) a_jour: std::collections::HashSet<IdSalarie>,
    /// Le resume de la paie de chaque salarie du mois, a noter au journal avec l'envoi.
    pub(crate) resumes: HashMap<IdSalarie, envoi::ResumeDePaie>,
}

impl Choix {
    /// Ce qui part, parmi les messages de tout le mois : ceux qui ne sont pas a jour ; s'ils le sont tous, tous.
    pub(crate) fn garder<T>(&self, messages: Vec<T>, cle: impl Fn(&T) -> &IdSalarie) -> Vec<T> {
        if messages.iter().all(|m| self.a_jour.contains(cle(m))) {
            return messages;
        }
        messages.into_iter().filter(|m| !self.a_jour.contains(cle(m))).collect()
    }
}

/// Les salaries de `mois` dont un engagement a un bulletin a refaire (la paie de ce mois a change depuis la version sur
/// laquelle ses documents sont partis, `engagement::etats::a_refaire`), et ce qui a change dans leurs engagements depuis
/// cette version (`engagement::versions::depuis`, ce que dit la frise) ; un salarie a plusieurs engagements nomme
/// chacun par son contrat.
pub(crate) fn bulletins_a_refaire(ctx: &Contexte, mois: Mois) -> R<HashMap<IdSalarie, Vec<String>>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let mut par_salarie: HashMap<IdSalarie, Vec<(Option<engagement::numeros::NumeroContrat>, Vec<String>)>> = HashMap::new();
    for e in engagement::etats::du_mois(ctx, mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)? {
        let Some(v) = e.valide() else { continue };
        let depuis = engagement::etats::a_refaire(ctx, v, &departs)?.into_iter().find_map(|a| match a {
            engagement::etats::ARefaire::Bulletin { mois: m, depuis } if m == mois => Some(depuis),
            _ => None,
        });
        let Some(depuis) = depuis else { continue };
        let brut = &*v.objet().brut;
        if let Some(c) = &brut.saisie.carte.salarie {
            let contrat = brut.saisie.carte.numero.clone();
            par_salarie.entry(c.clone()).or_default().push((contrat, engagement::versions::depuis(ctx, v, depuis)?));
        }
    }
    Ok(par_salarie
        .into_iter()
        .map(|(c, engs)| {
            let plusieurs = engs.len() > 1;
            let faits = engs
                .into_iter()
                .flat_map(|(contrat, faits)| {
                    faits.into_iter().map(move |f| match (&contrat, plusieurs) {
                        (Some(n), true) => format!("contrat n° {} : {}", n, f),
                        _ => f,
                    })
                })
                .collect();
            (c, faits)
        })
        .collect())
}

/// Le choix, pour le mois `mois`, sur ses paies validees (`crate::documents::charger_le_mois`).
pub(crate) fn choisir(ctx: &Contexte, mois: Mois, paies: &[engagement::calcul::Paie]) -> R<Choix> {
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let partis = crate::etapes::documents_partis(ctx, mois, &engagement::etats::du_mois(ctx, mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)?, &lignes)?;
    let cles: Vec<IdSalarie> = paies.iter().map(|p| p.salarie.id.clone()).collect();
    let mut resumes: HashMap<IdSalarie, envoi::ResumeDePaie> = HashMap::new();
    for cle in &cles {
        if resumes.contains_key(cle) {
            continue;
        }
        let siennes: Vec<&engagement::calcul::Paie> = paies.iter().zip(&cles).filter(|(_, c)| *c == cle).map(|(p, _)| p).collect();
        resumes.insert(cle.clone(), envoi::resume_de(&siennes)?);
    }
    let changes = bulletins_a_refaire(ctx, mois)?;
    let mut choix = Choix { resumes, ..Choix::default() };
    let declaration = declaration_de_la_correction(ctx, mois)?;
    for (cle, parti) in &partis {
        let Some(faits) = changes.get(cle) else {
            choix.a_jour.insert(cle.clone());
            continue;
        };
        let envoye_le = parti.quand.jour();
        let changements = envoi::ce_qui_a_change(faits, parti.resume.as_ref(), choix.resumes.get(cle));
        let declaration = declaration.as_ref().map(|d| d(cle)).into_iter().collect();
        choix.renvois.insert(cle.clone(), envoi::Renvoi { envoye_le, changements, declaration });
    }
    Ok(choix)
}

/// La phrase du courriel de renvoi qui dit ou la correction d'un salarie (par sa cle) sera declaree.
type DeclarationDUneCorrection = Box<dyn Fn(&IdSalarie) -> String>;

/// LA CORRECTION D'UN MOIS DONT L'ANNULE-ET-REMPLACE EST PASSE : le courriel de renvoi ajoute une phrase, la correction
/// sera declaree avec la paie d'un mois suivant. La DSN du mois ne se refait pas (`se_corrige_ailleurs` : sa DSN est
/// partie et l'annule-et-remplace n'est plus admis, ou c'est celle de l'ancien logiciel) ; une correction deja portee
/// par une DSN partie se lit dans ce qui est parti (`corrections_du_mois`), une correction a porter part dans la DSN
/// qui porte le mois (`compagnie::etat_dsn::dsn_qui_porte`). `None` : le mois se corrige dans sa propre DSN.
fn declaration_de_la_correction(ctx: &Contexte, mois: Mois) -> R<Option<DeclarationDUneCorrection>> {
    let departs = journal::lu(&ctx.racine).map(|j| j.departs)?;
    if !declarations::produire_regularisations::se_corrige_ailleurs(ctx, &departs, mois)? {
        return Ok(None);
    }
    let porteur = compagnie::etat_dsn::dsn_qui_porte(ctx, mois)?;
    let (a_porter, portees) = crate::regularisation::corrections_du_mois(ctx, mois)?;
    let mut porteurs: HashMap<IdSalarie, Mois> = HashMap::new();
    for c in portees {
        if let Some(par) = c.portee_par {
            porteurs.insert(c.salarie, par);
        }
    }
    for c in &a_porter {
        porteurs.remove(&c.salarie);
    }
    let de = crate::regularisation::de_mois_en_clair(mois);
    Ok(Some(Box::new(move |cle: &IdSalarie| {
        format!(
            "Ta paie {} était déjà déclarée : la correction sera déclarée avec la paie {}.\n\n",
            de,
            crate::regularisation::de_mois_en_clair(match porteurs.get(cle) {
                Some(&deja) => deja,
                None => porteur,
            })
        )
    })))
}

/// La saisie du mois, la boite, la preuve (`preuve` : un envoi ; `None` : ce qui partirait, sans rien envoyer), puis les
/// documents et les messages. Une boite absente se voit avant la preuve ; une preuve refusee n.ecrit rien. Rien n.est parti.
pub(crate) fn preparer_envoi(ctx: &Contexte, mois: Mois, seulement: Option<&[IdSalarie]>, preuve: Option<&Preuve>, ouvrir: Ouvrir) -> R<EnvoiPrepare> {
    let debut = std::time::Instant::now();
    let paies = charger_le_mois(ctx, mois)?;
    if paies.is_empty() {
        return Err(erreur(
            Classe::AucunDocument,
            format!("{} : aucun engagement validé, aucun document à envoyer. Rien ne part d'un engagement en saisie.", mois.libelle(Graphie::Accentuee)),
        ));
    }
    let dossier = ctx.racine.join("sorties").join(mois.to_string()).join("envoi");
    let choix = choisir(ctx, mois, &paies)?;
    // LES DOCUMENTS SE FABRIQUENT ICI, pour ceux a qui ils partent : depuis les paies ecrites, a leurs numeros.
    let mut cles: Vec<IdSalarie> = Vec::new();
    for p in &paies {
        if !cles.contains(&p.salarie.id) {
            cles.push(p.salarie.id.clone());
        }
    }
    let cles: Vec<IdSalarie> = choix.garder(cles, |c| c).into_iter().filter(|c| seulement.is_none_or(|s| s.iter().any(|x| *c == *x))).collect();
    if cles.is_empty() {
        return Err(refus("Aucun salarié coché. Rien n'est parti."));
    }
    // UN SALARIE SANS COURRIEL n'empeche pas les autres : ses documents ne partent pas par courriel, ils s'impriment
    // (`documents-imprimer`) ; les autres partent.
    let (cles, sans): (Vec<IdSalarie>, Vec<IdSalarie>) = cles.into_iter().partition(|c| a_un_courriel(&paies, c));
    let noms_sans = sans.iter().map(|c| nom_de_la_paie(&paies, c)).collect::<Vec<_>>().join(", ");
    if cles.is_empty() {
        return Err(refus(format!(
            "Pas de courriel dans la fiche de {} : ses documents ne partent pas par courriel. Imprimez-les pour les remettre en main propre, ou saisissez son courriel dans sa fiche. Rien n'est parti.",
            noms_sans
        )));
    }
    // La boite, puis LA PREUVE, AVANT TOUTE FABRIQUE : un mot de passe refuse n'ecrit rien.
    let client = ouvrir()?;
    let expediteur = expediteur_de(client.as_ref())?;
    let preuve = match preuve {
        Some(p) => {
            let a_qui = format!("{} salarie(s) : {}", cles.len(), cles.iter().map(|c| nom_de_la_paie(&paies, c)).collect::<Vec<_>>().join(", "));
            Some(Preuve { donnee: Some(verifier_preuve(ctx, Some(p), MoisOuJour::Mois(mois), "envoi des documents du mois aux salaries", &a_qui)?), ..Preuve::default() })
        }
        None => None,
    };
    let documents = crate::gestes::depart::fabriquer_documents(ctx, mois, &cles, "interface")?;
    let paies_servies: Vec<engagement::calcul::Paie> = paies.iter().filter(|p| cles.contains(&p.salarie.id)).cloned().collect();
    let messages = envoi::preparer_avec(&paies_servies, &ctx.entreprise, mois, &dossier, &choix.renvois, &dues_a_part(&paies_servies)?)?;
    // Les cles sont deja gardees (`Choix::garder`, plus haut) : leurs messages partent tous.
    let messages: Vec<envoi::Message<IdSalarie>> = messages.into_iter().filter(|m| cles.contains(&m.pour)).collect();
    let mut avertissements: Vec<String> = documents.avertissements.clone();
    if !sans.is_empty() {
        avertissements.push(sans_courriel_dit(&noms_sans));
    }
    let signataire = ctx.entreprise.signataire.dite().ok_or_else(|| socle::fichiers::absent("signataire").reclasser(Classe::Manque))?.email.portee().map(str::to_lowercase);
    if let Some(signataire) = signataire.filter(|s| *s != *expediteur) {
        avertissements.push(format!(
            "Les messages partiront de {} (la boite posee), pas de {} (le courriel du signataire, fiche Entreprise).",
            expediteur, signataire
        ));
    }
    Ok(EnvoiPrepare { messages, expediteur, avertissements, choix, documents, client, preuve, preparation_ms: socle::temps::duree(debut).as_millis() })
}

/// Ce qui suit la preparation : la PREUVE (le mot de passe du logiciel), l'envoi, le journal. `qui` : la ligne du journal.
pub(crate) fn expedier_envoi(ctx: &Contexte, prepare: EnvoiPrepare, mois: Mois, preuve: &Preuve, qui: &str) -> R<DocumentsEnvoyes> {
    let EnvoiPrepare { messages, expediteur, avertissements, choix, documents, mut client, preuve: verifiee, preparation_ms } = prepare;
    let preuve = match &verifiee {
        Some(p) => p,
        None => preuve,
    };
    let a_qui = format!("{} salarie(s) : {}", messages.len(), messages.iter().map(|m| m.nom.as_str()).collect::<Vec<_>>().join(", "));
    // LA VERSION SUR LAQUELLE ILS PARTENT, engagement par engagement des salaries servis.
    let servis: Vec<&IdSalarie> = messages.iter().map(|m| &m.pour).collect();
    let versions = journal::versions_du_mois(ctx, mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?, |cle, _, _| servis.contains(&cle))?;
    // CE QUI SERT APRES L'ACTE SE CALCULE AVANT LUI : une fois les courriels partis, rien ne peut plus echouer avant que
    // les numeros et la ligne du journal ne s'ecrivent. Ce que chaque salarie servi recoit : ses pieces, le resume de sa
    // paie (ce qui dira, si elle change, ce qui a change), et si c'est un renvoi ; son nom a l'ecran.
    let recus: Vec<journal::Servi> = messages
        .iter()
        .map(|m| {
            Ok(journal::Servi {
                nom: Some(m.nom.clone()),
                pieces: m.pieces.iter().map(|p| NomDeFichier::du(p)).collect::<R<_>>()?,
                resume: choix.resumes.get(&m.pour).cloned(),
                renvoi: choix.renvois.contains_key(&m.pour),
                cle: m.pour.clone(),
            })
        })
        .collect::<R<_>>()?;
    let noms: Vec<String> = messages.iter().zip(&recus).map(|(m, r)| nom_a_l_ecran(ctx, &r.cle, &m.nom)).collect();
    // LA DUREE DE CHAQUE ETAPE (le journal dit ou va le temps d'un envoi).
    let t_preuve = std::time::Instant::now();
    verifier_preuve(ctx, Some(preuve), MoisOuJour::Mois(mois), "envoi des documents du mois aux salaries", &a_qui)?;
    let preuve_ms = socle::temps::duree(t_preuve).as_millis();
    let t_envoi = std::time::Instant::now();
    let envoye = envoi::envoyer_suivi(&messages, &expediteur, client.as_mut(), None, None);
    let envoi_ms = socle::temps::duree(t_envoi).as_millis();
    let durees = journal::Durees { preparation: preparation_ms as u64, mot_de_passe: preuve_ms as u64, envoi: envoi_ms as u64 };
    let le_mois = Some(mois);
    let detail = |servis: &[journal::Servi]| journal::DetailDocuments {
        salaries: servis.to_vec(),
        main_propre: false,
        expediteur: Some(expediteur.clone()),
        durees_ms: Some(durees.clone()),
        partis: None,
        echec_a: None,
    };
    let partis = match envoye {
        Ok(p) => p,
        Err(e) => {
            // CE QUI A ATTEINT LE SALARIE EST PARTI : les messages partent dans l'ordre, les premiers sont servis. Leurs
            // numeros, et seulement les leurs, s'ecrivent d'abord ; la ligne porte ensuite la version de leurs
            // engagements, et seulement les leurs.
            let n = e.partis.len().min(messages.len());
            let mut versions_servies = Vec::new();
            if n > 0 {
                let cles: Vec<&IdSalarie> = recus[..n].iter().map(|r| &r.cle).collect();
                documents.engager_servis(ctx, mois, &cles)?;
                versions_servies = versions.iter().filter(|v| cles.contains(&&v.salarie)).cloned().collect();
            }
            noter_echec_d_envoi(ctx, journal::Ligne::Documents, le_mois, qui, &e.partis, &e.echoue, &e.echec.phrase, detail(&recus[..n]), Some(versions_servies))?;
            return Err(e.echec);
        }
    };
    // LES DOCUMENTS SONT PARTIS : leurs numeros s'ecrivent dans les paies des engagements, puis la ligne du journal dit
    // qu'ils sont partis, et sur quelle version.
    documents.engager(ctx, mois)?;
    let reponse = format!("{} message(s) envoye(s)", partis.len());
    noter(ctx, journal::Ligne::Documents(par_courriel(le_mois, qui, true, &en_liste(&partis, ", "), &reponse, detail(&recus), Some(versions))))?;
    Ok(DocumentsEnvoyes {
        phrase: format!("Documents {} envoyés à {} : {}.", socle::texte::de(&mois.libelle(Graphie::Accentuee)), socle::ecran::compte(partis.len(), "salarié", "salariés"), noms.join(", ")),
        partis,
        avertissements,
    })
}

/// La fiche du salarie `cle`, telle que ses paies du mois la portent, a-t-elle un courriel ?
fn a_un_courriel(paies: &[engagement::calcul::Paie], cle: &IdSalarie) -> bool {
    paies.iter().any(|p| p.salarie.id == *cle && p.salarie.email.is_some())
}

/// « NOM Prenom » du salarie `cle`, lu dans ses paies du mois (`cle` est celle d'une de ces paies).
fn nom_de_la_paie(paies: &[engagement::calcul::Paie], cle: &IdSalarie) -> String {
    paies.iter().find(|p| p.salarie.id == *cle).map(|p| nom_du_salarie(&p.salarie)).into_iter().collect()
}

/// Ce que l'envoi dit de ceux qui n'ont pas de courriel : leurs documents ne sont pas partis, ils s'impriment.
fn sans_courriel_dit(noms: &str) -> String {
    format!(
        "Pas de courriel dans la fiche de {} : ses documents ne partent pas par courriel. Imprimez-les pour les remettre en main propre, ou saisissez son courriel dans sa fiche.",
        noms
    )
}

/// Ceux a qui les documents du mois sont a faire partir (`choisir`) et qui n'ont pas de courriel dans leur fiche : leurs
/// documents s'impriment. Leur cle, et leur nom.
pub(crate) fn sans_courriel(ctx: &Contexte, mois: Mois) -> R<Vec<(IdSalarie, String)>> {
    let paies = charger_le_mois(ctx, mois)?;
    let mut cles: Vec<IdSalarie> = Vec::new();
    for p in &paies {
        let c = p.salarie.id.clone();
        if !cles.contains(&c) {
            cles.push(c);
        }
    }
    let choix = choisir(ctx, mois, &paies)?;
    Ok(cles.into_iter().filter(|c| !choix.a_jour.contains(c) && !a_un_courriel(&paies, c)).map(|c| { let nom = nom_de_la_paie(&paies, &c); (c, nom) }).collect())
}

