//! `historique-mois` : tout ce qu'un mois a produit, envoye et recu, pour l'Historique : tous les envois et tous
//! les retours se lisent dans l'onglet Historique.
//!
//! La page Paie est l'endroit ou l'on FAIT le mois ; l'Historique, celui ou l'on VOIT ce qu'il a donne. Cette route
//! rend, pour un mois :
//!
//! - `documents` : chaque fichier de `sorties/<mois>/` et de `sorties/<mois>/envoi/` (les memes listes que
//!   l'Historique : `crate::ouverture::mois_connus`, `crate::archives::pieces_de_l_envoi`), range
//!   par sorte, chacun avec son chemin, que la fenetre ouvre ;
//! - `envois` : chaque ligne du journal des depots du mois (DPAE, DSN, signalements, documents, contrats), TESTS
//!   COMPRIS : le journal ne porte que ce que CE logiciel a envoye (un test d'ailleurs n'y entre pas, `reprise_journal`),
//!   et ce qu'il a envoye se montre partout : le logiciel est d'une transparence totale, ce sont les informations et les
//!   tests de la compagnie ;
//! - sous chaque envoi, ses `retours` dans l'ordre d'arrivee, chacun LU : l'organisme, ce que dit son statut, et ce
//!   que dit sa piece archivee (`lire_piece` : bilans harmonises, NEOReS, organismes complementaires, certificats DPAE,
//!   AER). Le format de l'AGIRC-ARRCO, que personne ne publie, garde son statut et sa piece telle quelle.
//!
//! Ce que les pieces et le journal ne disent pas reste vide : une phrase ne dit que ce qui est la.
//!
//! Lecture seule : rien ne s'ecrit, rien ne part.

use std::path::{Path, PathBuf};

use serde::Serialize;

use droit::contexte::{Contexte, IdSalarie};
use droit::nature_declaration::{EssaiOuReel, NatureDeLaDeclaration};
use droit::organismes::IdentifiantDOrganisme;
use droit::type_declaration::TypeDeLaDeclaration;
use envois::etats;
use envois::journal::Ligne;
use envois::retours::{
    self, AnomalieDuRapport, CriticiteOc, CrmOc, DeclencheurNeores, IdDeFlux, NatureDuRetour, Neores, Rapport, ResultatDuControleNeores, StatutDuRetour, SubstituableNeores, TypeDActionNeores,
    TypeDeCrmOc, Verdict,
};
use socle::echec::{Classe, Echec, R};
use socle::ecran::Pastille;
use socle::fichiers::{CheminRelatif, NomDeFichier};
use socle::mois::Mois;
use socle::paris::Moment;

use crate::catalogue::{Acte, Ecran, Fiche, Place, Refus, Route, FORME, MOIS};
use crate::enveloppe::{en_json, route_inconnue, Charge, Dite, Porte, Rendu};
use crate::etapes::ChargeMois;

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Historique);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[Fiche {
    route: Route::HistoriqueMois,
    geste: "Dire l'Historique d'un mois : ses documents, ses envois et ce qui en est revenu.",
    charge: &[MOIS],
    exemple: r#"{"mois":"2026-11"}"#,
    rend: "mois, documents (chacun son chemin ; fichier : à refaire par piece-partie avant de l'ouvrir), envois (chacun ses retours, du plus récent au plus ancien)",
    refus: &[FORME, Refus { classe: Classe::Disque, corriger: "le journal des envois ou une liste des bulletins repris, que la phrase dit illisible" }],
    acte: Acte::Lit,
    phrase: 0,
    ecran: ECRAN,
}];

/// La route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::HistoriqueMois => {
            let ChargeMois { mois } = charge.lire()?;
            en_json(historique(porte.ctx, mois)?)
        }
        autre => Err(route_inconnue(autre.nom())),
    }
}

// ------------------------------------------------------------------ les tables

/// La nature et le type d'une declaration (S20.G00.05.001 et .002, `atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt`) ;
/// une nature qui n'est pas dite est une DSN.
fn nature_dsn(nature: Option<NatureDeLaDeclaration>, type_: Option<TypeDeLaDeclaration>) -> String {
    let n = match nature {
        Some(n) => n.libelle(),
        None => "DSN",
    };
    match type_ {
        Some(t) if t.remplace() => format!("{} (annule et remplace)", n),
        Some(TypeDeLaDeclaration::Annule) => format!("{} (annulation)", n),
        _ => n.to_string(),
    }
}

// ------------------------------------------------------------------ ce qui se dit, s'il est la

/// Un texte des formes du journal et des retours (`envois`), ou une zone a blanc s'ecrit vide : dit, ou absent.
fn dit(t: &str) -> Option<&str> {
    Some(t).filter(|x| !x.trim().is_empty())
}

/// Un texte facultatif d'un compte rendu, dit s'il porte quelque chose.
fn lu(t: &Option<String>) -> Option<&str> {
    t.as_deref().and_then(dit)
}

/// Un morceau de phrase qui ne se dit que si sa valeur l'est : `avant`, la valeur, `apres` ; rien sinon.
fn morceau(avant: &str, v: Option<&str>, apres: &str) -> String {
    v.into_iter().map(|x| format!("{}{}{}", avant, x, apres)).collect()
}

/// Les valeurs dites, jointes par `sep` ; aucune si aucune n'est dite.
fn joint<'a>(v: impl IntoIterator<Item = Option<&'a str>>, sep: &str) -> Option<String> {
    let dites: Vec<&str> = v.into_iter().flatten().filter(|x| !x.trim().is_empty()).collect();
    (!dites.is_empty()).then(|| dites.join(sep))
}

/// Le pluriel d'un compte : `un` pour un, `plusieurs` sinon.
fn plur(n: usize, un: &'static str, plusieurs: &'static str) -> &'static str {
    if n > 1 { plusieurs } else { un }
}

/// Le nom d'un salarie que porte `cle` : celui de sa fiche, s'il la dit ; sinon la cle elle-meme.
fn qui_de_la_cle(ctx: &Contexte, cle: &IdSalarie) -> String {
    match ctx.salaries.get(cle).map(|s| documents::tableaux_export::qui_souple(s)).filter(|q| !q.is_empty()) {
        Some(q) => q,
        None => cle.to_string(),
    }
}

/// Le salarie d'une ligne de contrat : le nom que la ligne porte (`detail.nom`, « NOM Prenom », ecrit par
/// `crate::contrats::contrat_recu` et `releve_signatures`), sinon celui de sa fiche (`detail.salarie` est sa cle),
/// sinon la cle elle-meme.
fn qui_du_contrat(ctx: &Contexte, nom: &str, cle: &IdSalarie) -> String {
    match dit(nom) {
        Some(n) => n.to_string(),
        None => qui_de_la_cle(ctx, cle),
    }
}

// ------------------------------------------------------------------ ce que la route rend

/// L'Historique d'un mois.
#[derive(Debug, Serialize)]
struct Historique {
    phrase: String,
    mois: Mois,
    documents: Vec<Document>,
    envois: Vec<Envoi>,
}

impl Dite for Historique {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// Un document du mois : son nom lisible, sa sorte, le salarie qu'il sert (s'il est dit), son chemin, et le fichier que
/// `piece-partie` refait quand le journal le dit parti et qu'il manque.
#[derive(Debug, Serialize)]
struct Document {
    nom: String,
    sorte: &'static str,
    salarie: Option<droit::contexte::IdSalarie>,
    chemin: CheminRelatif,
    #[serde(skip_serializing_if = "Option::is_none")]
    fichier: Option<NomDeFichier>,
}

/// Un envoi du mois (une ligne du journal), et ce qui en est revenu.
#[derive(Debug, Serialize)]
struct Envoi {
    quoi: &'static str,
    acte: String,
    mode: &'static str,
    quand: socle::paris::InstantOuJour,
    fichier: Option<NomDeFichier>,
    flux: Option<IdDeFlux>,
    destinataire: Option<String>,
    identite: String,
    accepte: bool,
    resume: String,
    attend_retour: bool,
    retours: Vec<RetourLu>,
}

/// Un retour sous son envoi : son titre, quand (sa reponse, ou la publication d'un retour d'organisme, ecrite telle que
/// la plate-forme la donne : `envois::retours::Annonce`), ce qu'il dit, sa pastille, sa piece ; pour un retour
/// d'organisme, ce que sa piece en dit (`organisme`).
#[derive(Debug, Serialize)]
struct RetourLu {
    titre: String,
    #[serde(with = "socle::paris::ecrit::seconde::option")]
    quand: Option<Moment>,
    phrases: Vec<String>,
    pastille: Option<Pastille>,
    /// Relative a la compagnie : la fenetre l'ouvre.
    piece: Option<CheminRelatif>,
    #[serde(flatten)]
    organisme: Option<RetourDOrganisme>,
}

/// Ce qu'un retour d'organisme dit : en titre, sa nature, quand, l'organisme, son statut et le verdict de sa piece, ses
/// anomalies, et si la piece s'est traduite.
#[derive(Debug, Serialize)]
struct RetourDOrganisme {
    titre_dit: String,
    nature_dit: String,
    quand_dit: Option<String>,
    #[serde(rename = "nature")]
    nature_retour: NatureDuRetour,
    libelle: String,
    organisme: Option<String>,
    statut: Option<StatutDuRetour>,
    verdict: Option<Verdict>,
    anomalies: Vec<Anomalie>,
    traduite: bool,
}

/// Une anomalie d'un retour, telle que la page l'affiche : ce qu'elle dit, et ce qui est dit d'elle.
#[derive(Debug, Serialize)]
struct Anomalie {
    code: Option<String>,
    categorie: Option<String>,
    message: Option<String>,
    rubrique: Option<String>,
    ligne: Option<String>,
    nombre: Option<String>,
    salarie: Option<String>,
}

impl Anomalie {
    fn du_rapport(a: &AnomalieDuRapport) -> Anomalie {
        let o = |t: &str| dit(t).map(String::from);
        Anomalie { code: o(&a.code), categorie: a.categorie.map(|c| c.to_string()), message: o(&a.message), rubrique: o(&a.rubrique), ligne: o(&a.ligne), nombre: o(&a.nombre), salarie: o(&a.salarie) }
    }

    fn du_message(message: &str) -> Anomalie {
        Anomalie { code: None, categorie: None, message: dit(message).map(String::from), rubrique: None, ligne: None, nombre: None, salarie: None }
    }
}

// ------------------------------------------------------------------ les envois

/// CE QUI EST REVENU D'UN CONTRAT PARTI (`envois::etats::revenus`), lu : le contrat ou l'avenant, de qui, et comment (une
/// reponse « Je signe », une mention a la main, ou son retrait) ; la piece signee s'il y en a une.
fn revenu_lu(ctx: &Contexte, ligne: &Ligne, r: &envois::journal::RetourContrat) -> R<RetourLu> {
    let piece = match (r.avenant, r.numero.as_deref()) {
        (None, Some(n)) => format!("Contrat n° {}", n),
        (None, None) => "Contrat sans numéro".to_string(),
        (Some(a), Some(n)) => format!("Avenant n° {} au contrat n° {}", a, n),
        (Some(a), None) => format!("Avenant n° {} au contrat sans numéro", a),
    };
    let quand = r.quand.dit();
    let (titre, phrase) = if !r.recu {
        (format!("{} : mention « signé » retirée", piece), format!("Mention retirée le {}.", quand))
    } else if r.par_courriel() {
        (format!("{} revenu signé", piece), format!("Signé d'un clic le {}.", quand))
    } else {
        (format!("{} revenu signé", piece), format!("Reçu signé le {}, noté à la main.", quand))
    };
    let signe = match (ligne.mois(), r.fichier_signe.as_ref()) {
        (Some(m), Some(f)) => envois::etats::chemin_piece(&ctx.racine, m, f),
        _ => None,
    };
    Ok(RetourLu {
        titre: format!("{} — {}", titre, qui_du_contrat(ctx, &r.nom, &r.salarie)),
        quand: Some(r.quand),
        phrases: vec![phrase],
        pastille: Some(Pastille::de(if !r.recu { socle::ecran::EtatDEcran::Information } else { socle::ecran::EtatDEcran::Fait }, "signé")),
        piece: signe.map(|p| CheminRelatif::sous(&ctx.racine, &p)).transpose()?,
        organisme: None,
    })
}

/// L'acte d'une ligne du journal, en mots ; un contrat qui repart dit de qui il est.
fn acte(ctx: &Contexte, l: &Ligne) -> String {
    match l {
        Ligne::Dsn(e) => nature_dsn(e.detail.nature_declaration, e.detail.type_declaration),
        Ligne::Dpae(_) => "DPAE".into(),
        Ligne::Documents(_) => "Documents aux salariés".into(),
        Ligne::ContratSignature(e) => match (e.detail.main_propre, e.detail.acte.as_deref() == Some("avenant")) {
            (true, true) => "Avenant imprimé, à remettre en main propre".into(),
            (true, false) => "Contrat imprimé, à remettre en main propre".into(),
            (false, true) => "Avenant envoyé à la signature".into(),
            (false, false) => "Contrat envoyé à la signature".into(),
        },
        Ligne::ContratContresigne(e) => format!("Contrat contresigné renvoyé — {}", qui_du_contrat(ctx, &e.detail.nom, &e.detail.salarie)),
        Ligne::RegularisationCasc(_) => "Régularisation Congés Spectacles".into(),
        Ligne::DossierSalarie(_) => "Dossier d'un salarié".into(),
        Ligne::FicheDemande(_) => "Fiche demandée à un salarié".into(),
    }
}

// ------------------------------------------------------------------ les documents

/// Les sortes de documents, dans l'ordre ou on les cherche, par le debut du nom (les fichiers du mois et ceux d'envoi).
const SORTES: [(&str, &[&str]); 8] = [
    ("Bulletins de paie", &["Bulletins_", "Bulletin_"]),
    ("Certificats Congés Spectacles", &["CertificatsCS_", "CertificatCS_"]),
    ("Attestations employeur (AER)", &["AER_"]),
    ("Contrats", &["Contrats_", "Contrat_", "Avenant"]),
    ("DPAE", &["DPAE_"]),
    ("DSN", &["DSN_"]),
    ("Demandes de taux de prélèvement à la source", &["Amorcage"]),
    ("AEM", &["AEM_"]),
];

/// La sorte d'un document dont le nom ne commence comme aucune autre : elle se range apres elles.
const AUTRES: &str = "Autres pièces";

/// CE QUE LA PERSONNE LIT d'une demande de taux de prelevement a la source (le signalement d'amorcage,
/// `amorcage_demande::produire` : `Amorcage<motif>_<salarie>_<AAAAMMJJ>.dsn`, et sa note
/// `Amorcage<motif>_<AAAAMMJJ>_recapitulatif.txt`) : pour qui, et quel jour. Un autre nom se lit tel quel.
fn nom_lisible(ctx: &Contexte, nom: &str) -> String {
    let jour = |t: &str| socle::piece::Zone::lue(t, socle::piece::format::Aaaammjj).valeur().map(|j| j.jj_mm_aaaa());
    let Some(reste) = nom.strip_prefix("Amorcage").and_then(|r| r.get(3..)) else { return nom.to_string() };
    if let Some(j) = reste.strip_suffix("_recapitulatif.txt").and_then(jour) {
        return format!("Note des demandes du {}", j);
    }
    if let Some((cle, j)) = reste.strip_suffix(".dsn").and_then(|s| s.rsplit_once('_')).and_then(|(c, t)| Some((IdSalarie::lire(c).ok()?, jour(t)?))) {
        return format!("Taux de {}, demandé le {}", qui_de_la_cle(ctx, &cle), j);
    }
    nom.to_string()
}

/// CE QUI DIT QUEL ENVOI C'EST, en mots : pour qui (une demande de taux nomme son salarie), le numero de depot que
/// l'organisme lui a donne (celui qu'on cite pour le retrouver chez lui), et ou il est parti. Le nom du fichier n'y
/// figure pas : la piece s'ouvre depuis « Les documents ».
fn identite(ctx: &Contexte, l: &Ligne) -> String {
    let mut morceaux: Vec<String> = Vec::new();
    if let Some(fichier) = l.fichier() {
        let lisible = nom_lisible(ctx, fichier.texte());
        if lisible != fichier.texte() {
            morceaux.push(lisible);
        }
    }
    if let Some(f) = l.flux() {
        morceaux.push(format!("numéro de dépôt {}", f));
    }
    match dit(l.destinataire()) {
        None => {}
        Some("net-entreprises") => morceaux.push("sur net-entreprises".into()),
        Some("urssaf") => morceaux.push("à l'Urssaf".into()),
        Some(autre) => morceaux.push(format!("à {}", autre)),
    }
    morceaux.join(" · ")
}

/// La sorte d'un document, par le debut de son nom (`SORTES`, sinon `AUTRES`).
pub(crate) fn sorte_de(nom: &str) -> &'static str {
    match SORTES.iter().find(|(_, p)| p.iter().any(|x| nom.starts_with(x))) {
        Some((s, _)) => s,
        None => AUTRES,
    }
}

/// Les documents du mois : les fichiers de `sorties/<mois>/` (`mois_connus`, la liste de l'Historique) et les PDF de
/// `sorties/<mois>/envoi/` (`pieces_de_l_envoi`). Une archive `.zip` n'est que l'emballage
/// du fichier de meme nom qui part au depot : elle ne se liste pas.
fn documents(ctx: &Contexte, mois: Mois, lignes: &[Ligne]) -> R<Vec<Document>> {
    let mut out: Vec<Document> = Vec::new();
    let sorties = CheminRelatif::sorties(mois);
    let envoi = sorties.joint(&const { NomDeFichier::du_moteur("envoi") });
    let noms = crate::ouverture::mois_connus(&ctx.racine)?.into_iter().filter(|(m, _)| *m == mois).flat_map(|(_, n)| n);
    for nom in noms.filter(|n| !n.texte().ends_with(".zip")) {
        out.push(Document { sorte: sorte_de(nom.texte()), salarie: None, chemin: sorties.joint(&nom), fichier: None, nom: nom.into() });
    }
    for d in crate::archives::pieces_de_l_envoi(ctx, mois)? {
        out.push(Document { sorte: sorte_de(&d.nom), salarie: d.salarie, chemin: envoi.joint(&NomDeFichier::lire(&d.nom)?), fichier: None, nom: d.nom });
    }
    // CE QUE LE JOURNAL DIT PARTI, meme si son fichier manque : `fichier` le nomme pour `piece-partie`, qui le refait
    // (`depart::piece_partie`) et rend son chemin.
    for nom in pieces_du_journal(lignes) {
        if out.iter().any(|d| d.nom == nom.texte()) {
            continue;
        }
        let dans_l_envoi = ["Bulletin_", "CertificatCS_", "AER_", "Contrat_", "Avenant"].iter().any(|p| nom.texte().starts_with(p));
        let chemin = if dans_l_envoi { envoi.joint(&nom) } else { sorties.joint(&nom) };
        out.push(Document { sorte: sorte_de(nom.texte()), salarie: None, chemin, fichier: Some(nom.clone()), nom: nom.into() });
    }
    // LES BULLETINS QUE L'ANCIEN LOGICIEL A FAITS, tels que l'import les garde : ce sont les bulletins du mois.
    for b in import::mois::bulletins_repris(&ctx.racine, mois)? {
        let nom = format!("Bulletin de {} (ancien logiciel)", qui_de_la_cle(ctx, &b.salarie));
        out.push(Document { nom, sorte: "Bulletins de paie", salarie: Some(b.salarie.clone()), chemin: b.chemin, fichier: None });
    }
    for d in out.iter_mut() {
        d.nom = nom_lisible(ctx, &d.nom);
    }
    // Dans l'ordre des sortes, les autres pieces a la fin.
    out.sort_by_key(|d| {
        let rang = SORTES.iter().position(|(s, _)| *s == d.sorte);
        (rang.is_none(), rang)
    });
    Ok(out)
}

/// Les noms des pieces que les lignes acceptees du journal d'un mois disent parties : le fichier d'un depot, les contrats
/// d'une demande de signature, les pieces d'un envoi de documents.
fn pieces_du_journal(lignes: &[Ligne]) -> Vec<NomDeFichier> {
    let mut noms: Vec<NomDeFichier> = Vec::new();
    let mut noter = |n: &NomDeFichier| {
        if !noms.contains(n) {
            noms.push(n.clone());
        }
    };
    for l in lignes.iter().filter(|l| l.accepte()) {
        if let Some(f) = l.fichier() {
            noter(f);
        }
        match l {
            Ligne::ContratSignature(e) => e.detail.contrats.iter().for_each(|c| noter(&c.fichier)),
            Ligne::ContratContresigne(e) => e.detail.contrats.iter().for_each(|c| noter(&c.fichier)),
            Ligne::Documents(e) => e.detail.salaries.iter().flat_map(|s| &s.pieces).for_each(|p| noter(p)),
            _ => {}
        }
    }
    noms
}

// ------------------------------------------------------------------ les retours, lus

/// Le refus d'un dossier des retours qui ne se lit pas : la phrase en mots, le dossier et l'erreur a part.
#[track_caller]
fn retours_illisibles(dossier: &Path, e: std::io::Error) -> Echec {
    socle::echec::refus_avec(
        "Le dossier des retours ne se lit pas.",
        socle::echec::Lieu { chemin: Some(dossier.to_string_lossy().into_owned()), detail: Some(e.to_string()), ..Default::default() },
    )
}

/// La piece archivee d'un retour : `retours/[dpae/]AAAAMMJJ-hhmmss_natureN_<flux>.<ext>` (`envois::releve_retours::archiver_retour`,
/// qui nomme `sansflux` un retour sans flux) : celle de sa publication ; sans publication datee, la premiere de sa nature
/// pour ce flux (`etats::archive_du_retour`). Aucune s'il n'y en a pas ; un dossier des retours qui ne se lit pas le dit.
fn piece_du_retour(racine: &Path, canal: Option<&str>, flux: Option<&IdDeFlux>, nature: NatureDuRetour, publication: Option<Moment>) -> R<Option<PathBuf>> {
    let mut dossier = racine.join(envois::retours::RETOURS);
    if let Some(c) = canal {
        dossier = dossier.join(c);
    }
    let flux: String = match flux {
        Some(f) => f.texte().chars().take(24).collect(),
        None => "sansflux".into(),
    };
    let suffixe = format!("_nature{}_{}", nature, flux);
    let lus = match std::fs::read_dir(&dossier) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(retours_illisibles(&dossier, e)),
    };
    let mut noms: Vec<String> = Vec::new();
    for e in lus {
        let n = e.map_err(|x| retours_illisibles(&dossier, x))?.file_name().to_string_lossy().into_owned();
        let radical = match n.rsplit_once('.') {
            Some((r, _)) => r,
            None => n.as_str(),
        };
        if radical.ends_with(&suffixe) {
            noms.push(n);
        }
    }
    noms.sort();
    let horo = publication.map(socle::paris::ecrit::compacte::texte).map(|p| format!("{}-{}", &p[..8], &p[8..]));
    // Une publication datee n'a que sa piece : celle d'une autre publication du meme flux dirait un autre retour.
    let choisi = match horo {
        Some(h) => noms.iter().find(|n| n.starts_with(&h)),
        None => noms.first(),
    };
    Ok(choisi.map(|c| dossier.join(c)))
}

/// Ce qu'on lit d'une piece, en phrases.
struct Lecture {
    titre: String,
    /// L'organisme que la piece nomme (NEOReS : `RO.B002.R005` ; OC : le producteur du compte rendu), s'il le dit.
    organisme: Option<String>,
    phrases: Vec<String>,
    anomalies: Vec<Anomalie>,
    traduite: bool,
    /// Ce que la piece dit d'elle-meme (un rapport harmonise : `OK`, `ANO`, `KO`), si elle le dit.
    verdict: Option<Verdict>,
}

impl Lecture {
    fn nouvelle(titre: impl Into<String>) -> Lecture {
        Lecture { titre: titre.into(), organisme: None, phrases: Vec::new(), anomalies: Vec::new(), traduite: true, verdict: None }
    }
}

/// Une piece lue : le certificat DPAE (nature 41 du canal DPAE, `dpae::lire_certificat`), un rapport au format
/// harmonise (`retours::lire_rapport`), une archive d'AER (`retours::aer_spectacle`), un PDF. Autre chose ne se
/// traduit pas : son format est dit, ce qui l'empeche de se lire aussi, et la piece s'ouvre telle quelle.
fn lire_piece(o: &[u8], canal: Option<&str>, nature: NatureDuRetour, format: Option<&str>) -> Lecture {
    if canal == Some("dpae") && nature == NatureDuRetour::CertificatDpae {
        let texte: String = o.iter().map(|&b| b as char).collect();
        let lignes = declarations::dpae::lire_certificat(&texte);
        if !lignes.is_empty() {
            let mut l = Lecture::nouvelle("Certificat de conformité des DPAE");
            for x in &lignes {
                let qui = joint([dit(&x.nom), dit(&x.prenom)], " ");
                let etat = if x.enregistree {
                    format!("enregistrée{}", morceau(", dossier ", dit(&x.dossier), ""))
                } else if x.deja_existante {
                    "déjà enregistrée auparavant".to_string()
                } else {
                    format!("refusée{}", morceau(" : ", joint([dit(&x.code), dit(&x.libelle)], " — ").as_deref(), ""))
                };
                l.phrases.push(format!("{}{} : {}.", morceau("", qui.as_deref(), ""), morceau(" (embauche du ", dit(&x.embauche), ")"), etat));
            }
            return l;
        }
    }
    if o.starts_with(b"%PDF") {
        let mut l = Lecture::nouvelle("Pièce PDF");
        l.phrases.push("Un document PDF : il s'ouvre tel quel.".into());
        return l;
    }
    if o.starts_with(b"PK") {
        let mut l = Lecture::nouvelle("Archive d'attestations employeur (AER)");
        match retours::aer_spectacle(o) {
            Ok(aer) if !aer.is_empty() => {
                let n = aer.len();
                l.phrases.push(format!("{} attestation{} employeur rematérialisée{} :", n, plur(n, "", "s"), plur(n, "", "s")));
                for a in &aer {
                    l.phrases.push(format!("{}{}", a.qui, morceau(" (période ", a.periode.map(|m| m.to_string()).as_deref(), ")")));
                }
                l.phrases.push("Chaque AER se range aussi parmi les documents du mois, prête à partir au salarié.".into());
            }
            Ok(_) => match retours::membres_du_zip(o) {
                Ok(m) => l.phrases.push(format!("Une archive de {} fichier{}, sans attestation.", m.len(), plur(m.len(), "", "s"))),
                Err(m) => l.phrases.push(format!("Cette archive ne se lit pas : {}", m.phrase)),
            },
            Err(m) => l.phrases.push(format!("Cette archive ne se lit pas : {}", m.phrase)),
        }
        return l;
    }
    // Un format qui ne se lit pas n'est pas celui de la piece : le suivant s'essaie, et ce qui l'empeche se dit a la fin.
    let mut pas_lus: Vec<String> = Vec::new();
    match retours::lire_rapport(o) {
        Ok(Some(rapport)) => return rapport_lu(&rapport, o, nature),
        Ok(None) => {}
        Err(m) => pas_lus.push(m.phrase),
    }
    match retours::lire_neores(o) {
        Ok(Some(crm)) => return neores_lu(&crm),
        Ok(None) => {}
        Err(m) => pas_lus.push(m.phrase),
    }
    match retours::lire_crm_oc(o) {
        Ok(Some(crm)) => return crm_oc_lu(&crm),
        Ok(None) => {}
        Err(m) => pas_lus.push(m.phrase),
    }
    let mut l = Lecture::nouvelle("Compte rendu");
    l.traduite = false;
    l.phrases.push(match format {
        // Le format temporaire de l'AGIRC-ARRCO (guide v6.8, « Documents de reference », [7] : `Schéma XSD RC V07.xsd`,
        // `accuseReceptionDSNDTO.xsd`, `notificationCcmdDSNDTO.xsd`) n'est publie ni avec le guide, ni sur net-entreprises,
        // ni sur agirc-arrco.fr (releve du 20/10/2026 : `atelier/reference/sources/gip-mds-api-dsn/LISEZ-MOI.md`).
        Some("agirc-arrco") => "Ce compte rendu de l'AGIRC-ARRCO est dans un format que l'AGIRC-ARRCO ne publie pas : ce logiciel ne peut donc pas le traduire. Le statut ci-dessus est celui que donne net-entreprises, et la pièce s'ouvre telle quelle.".to_string(),
        _ => "Cette pièce n'a pas la forme que son format annonce : ce logiciel ne sait pas la lire. Le statut ci-dessus est celui que donne net-entreprises, et la pièce s'ouvre telle quelle.".to_string(),
    });
    l.phrases.extend(pas_lus);
    l
}

/// Un rapport au format harmonise (`retours::lire_rapport`), dit en phrases : son titre, son message, l'envoi, chaque
/// declaration, les informations, les taux communiques (nature 114), les anomalies, et le verdict de la piece.
fn rapport_lu(rapport: &Rapport, o: &[u8], nature: NatureDuRetour) -> Lecture {
    let titre = match (lu(&rapport.libelle), lu(&rapport.type_rapport)) {
        (Some(libelle), _) => libelle.to_string(),
        (None, Some(t)) => format!("Rapport {}", t),
        (None, None) => "Compte rendu".into(),
    };
    let mut l = Lecture::nouvelle(titre);
    if let Some(m) = &rapport.message {
        l.phrases.push(m.clone());
    }
    if rapport.essai_reel == Some(EssaiOuReel::Essai) {
        l.phrases.push("Envoi d'essai (test) : rien n'est déclaré ni conservé.".into());
    }
    let etat = rapport.etat_envoi;
    if let Some(v) = etat {
        l.phrases.push(format!("L'envoi est {} ({}).", v.de_l_envoi(), v.code()));
    }
    for d in &rapport.declarations {
        let verdict = match d.etat {
            Some(e) => format!("{} ({})", e.en_mots(), e.code()),
            None => "sans état".to_string(),
        };
        l.phrases.push(format!(
            "{}{}{} : {}.",
            nature_dsn(d.nature_declaration, d.type_declaration),
            morceau(" de ", d.mois.map(|m| m.to_string()).as_deref(), ""),
            morceau(", ordre ", d.ordre.map(|o| o.to_string()).as_deref(), ""),
            verdict
        ));
    }
    for i in &rapport.informations {
        let quoi = match dit(&i.type_information) {
            Some("atmp") => "Information : taux accident du travail et maladie professionnelle".to_string(),
            Some("bmcc") => "Information : bonus-malus de la contribution chômage".to_string(),
            None => "Information".to_string(),
            Some(autre) => format!("Information ({})", autre),
        };
        let donnees: Vec<String> = i.donnees.iter().map(|d| format!("{} : {}", d.nom, d.valeur)).collect();
        l.phrases.push(if donnees.is_empty() { format!("{}.", quoi) } else { format!("{} — {}.", quoi, donnees.join(" ; ")) });
    }
    if nature == NatureDuRetour::TauxPas {
        match retours::taux_pas_d_octets(o) {
            Ok(t) => l.phrases.push(format!("{} taux de prélèvement à la source communiqué{}.", t.releves.len(), plur(t.releves.len(), "", "s"))),
            Err(m) => l.phrases.push(format!("Les taux de prélèvement à la source communiqués ne se lisent pas : {}", m.phrase)),
        }
    }
    l.anomalies = rapport.anomalies.iter().map(Anomalie::du_rapport).collect();
    // LE VERDICT EST CELUI DE LA PIECE : France Travail publie son compte rendu « cinéma, spectacle »
    // (nature 45) toujours au statut ANOBLOQUANTE, même quand il ne relève rien (guide « De la DSN à l'attestation
    // employeur », v3.2, p. 10, dépose dans `net-entreprises-base-connaissances/`). Ce que dit la piece passe devant.
    let verdicts: Vec<Verdict> = etat.into_iter().chain(rapport.declarations.iter().filter_map(|d| d.etat)).collect();
    l.verdict = if verdicts.contains(&Verdict::Ko) {
        Some(Verdict::Ko)
    } else if verdicts.contains(&Verdict::Ano) || !l.anomalies.is_empty() {
        Some(Verdict::Ano)
    } else if verdicts.contains(&Verdict::Ok) {
        Some(Verdict::Ok)
    } else {
        None
    };
    l
}

/// `AAAA-MM-JJ` (ou `AAAA-MM-JJThh:mm:ss`) en `JJ/MM/AAAA` ; autre chose tel quel.
fn date_lisible(d: &str) -> String {
    etats::quand_lisible(&d.chars().take(10).collect::<String>())
}

/// La premiere lettre en capitale.
fn majuscule(t: &str) -> String {
    let mut c = t.chars();
    c.next().into_iter().flat_map(char::to_uppercase).chain(c).collect()
}

/// Un compte rendu NEOReS (`retours::lire_neores`, d'apres le cahier technique NEOReS 2026.1.2 et 2027.1.0,
/// `atelier/reference/sources/neores/`), dit en phrases : qui l'emet et pourquoi, la periode, la conformite, chaque
/// action attendue (en anomalie : le code du traitement, son libelle, les rubriques et les salaries en jeu ; en phrase :
/// le controle, l'action attendue, ce que vaut l'absence de correction), les parametres a prendre en paie, les ecarts
/// d'identite, les informations.
fn neores_lu(crm: &Neores) -> Lecture {
    let mut l = Lecture::nouvelle(match lu(&crm.libelle) {
        Some(t) => t.to_string(),
        None => "Compte rendu NEOReS".to_string(),
    });
    l.organisme = lu(&crm.organisme).map(String::from);
    if let Some(pourquoi) = crm.declencheur.map(DeclencheurNeores::libelle) {
        l.phrases.push(format!("{}{}.", majuscule(pourquoi), morceau(" (compte rendu ", lu(&crm.code), ")")));
    }
    if crm.remplace {
        l.phrases.push("Ce compte rendu annule et remplace un compte rendu précédent de l'organisme.".into());
    }
    if crm.essai_reel == Some(EssaiOuReel::Essai) {
        l.phrases.push("Il répond à un envoi d'essai (test).".into());
    }
    if let Some(debut) = crm.debut {
        l.phrases.push(match crm.fin.filter(|f| *f != debut) {
            None => format!("Période : {}.", debut.jj_mm_aaaa()),
            Some(fin) => format!("Période : du {} au {}.", debut.jj_mm_aaaa(), fin.jj_mm_aaaa()),
        });
    }
    let metier = morceau(" (", lu(&crm.metier), ")");
    match crm.conformite {
        Some(Verdict::Ok) => l.phrases.push(format!("Aucun écart constaté sur les données analysées{}.", metier)),
        Some(Verdict::Ano) => l.phrases.push(format!("Des écarts ont été constatés sur les données analysées{} : chacun appelle une correction dans la paie, au plus tard dans la DSN suivante.", metier)),
        _ => {}
    }
    for a in &crm.actions {
        let mut rubriques: Vec<String> = Vec::new();
        let mut salaries: Vec<String> = Vec::new();
        let mut precisions: Vec<String> = Vec::new();
        for d in &a.details {
            if let Some(atypie) = lu(&d.atypie) {
                precisions.push(atypie.trim_end_matches('.').to_string());
            }
            for i in &d.individus {
                let qui = joint([lu(&i.nom), lu(&i.prenom)], " ");
                let q = match (qui, lu(&i.contrat)) {
                    (Some(q), Some(c)) => Some(format!("{} (contrat {})", q, c)),
                    (Some(q), None) => Some(q),
                    (None, Some(c)) => Some(format!("(contrat {})", c)),
                    (None, None) => None,
                };
                if let Some(q) = q.filter(|q| !salaries.contains(q)) {
                    salaries.push(q);
                }
            }
            for x in &d.donnees {
                let r = lu(&x.rubrique);
                if let Some(r) = r.filter(|r| !rubriques.iter().any(|x| x == r)) {
                    rubriques.push(r.to_string());
                }
                let mut v = Vec::new();
                if let Some(t) = lu(&x.declaree) {
                    v.push(format!("déclaré « {} »", t));
                }
                if let Some(t) = lu(&x.attendue) {
                    v.push(format!("attendu « {} »", t));
                }
                if !x.acceptees.is_empty() {
                    v.push(format!("valeurs acceptées : {}", x.acceptees.join(", ")));
                }
                if let Some(t) = x.resultat {
                    v.push(t.libelle().to_string());
                }
                if let (false, Some(r)) = (v.is_empty(), r) {
                    precisions.push(format!("{} : {}", r, v.join(", ")));
                }
            }
            if let Some(t) = d.substituable {
                precisions.push(t.libelle().to_string());
            }
        }
        let message = joint([lu(&a.libelle), a.resultat.map(ResultatDuControleNeores::libelle)], " — ");
        // Un ecart a corriger (types 01 a 03, ou un resultat de controle) se range parmi les anomalies ; un parametre a
        // prendre (04, 05) n'en est pas une : il se dit en phrase, avec sa valeur (`RO.B008`, plus bas).
        let parametre = matches!(a.type_action, Some(TypeDActionNeores::PriseEnCompteDUnParametre | TypeDActionNeores::RegularisationDUnParametre)) && a.resultat.is_none();
        if !parametre {
            l.anomalies.push(Anomalie {
                code: lu(&a.code).map(String::from),
                categorie: a.type_action.map(|t| t.libelle().to_string()),
                message: message.clone(),
                rubrique: joint(rubriques.iter().map(|r| Some(r.as_str())), ", "),
                ligne: None,
                nombre: lu(&a.individus).map(String::from),
                salarie: joint(salaries.iter().map(|s| Some(s.as_str())), ", "),
            });
        } else if let Some(m) = &message {
            l.phrases.push(format!("{}{}.", morceau("", a.type_action.map(|t| majuscule(t.libelle())).as_deref(), " : "), m));
        }
        let tete = morceau("", lu(&a.code), " — ");
        if let Some(c) = lu(&a.controle) {
            l.phrases.push(format!("{}Contrôle : {}", tete, c));
        }
        if !precisions.is_empty() {
            l.phrases.push(format!("{}Détail : {}.", tete, precisions.join(" ; ")));
        }
        if let Some(t) = lu(&a.action) {
            l.phrases.push(format!("{}Action attendue : {}", tete, t));
        }
        for (v, quoi) in [
            (lu(&a.consequences), "Sans correction"),
            (a.substituable.map(SubstituableNeores::libelle), "Substitution"),
            (lu(&a.rappel), "Avant correction par l'organisme"),
            (lu(&a.exception), "Cas d'exception"),
            (lu(&a.reference), "Référence"),
            (lu(&a.aide), "Aide"),
        ] {
            if let Some(v) = v {
                l.phrases.push(format!("{}{} : {}", tete, quoi, v));
            }
        }
    }
    for p in &crm.parametres {
        let quoi = match p.libelle.and_then(dit) {
            Some(libelle) => format!("{}{}", libelle, morceau(" (", lu(&p.type_parametre), ")")),
            None => format!("Paramètre{}", morceau(" ", lu(&p.type_parametre), "")),
        };
        // Sans jour d'effet, la phrase ne dit pas de periode.
        let periode: String = p
            .effet
            .map(|e| match p.fin {
                Some(f) => format!(", du {} au {}", e.jj_mm_aaaa(), f.jj_mm_aaaa()),
                None => format!(", à partir du {}", e.jj_mm_aaaa()),
            })
            .into_iter()
            .collect();
        let calcul = morceau(" (calculé le ", p.calcule_le.map(|j| j.jour_de_paris().jj_mm_aaaa()).as_deref(), ")");
        l.phrases.push(format!("{}{}{}{}{}.", majuscule(&quoi), morceau(" : ", lu(&p.valeur), ""), periode, morceau(", rubrique ", lu(&p.rubrique), ""), calcul));
    }
    if crm.parametres.iter().any(|x| lu(&x.type_parametre).is_some_and(|t| t.starts_with('A'))) {
        l.phrases.push("Ces effectifs sont repris d'eux-mêmes dans config/effectifs.json à chaque relevé des retours : ce sont eux qui décident des seuils de cotisation.".into());
    }
    for i in &crm.identites {
        let declare = joint([lu(&i.nom_declare), lu(&i.prenom_declare)], " ");
        let reference = joint([lu(&i.nom_reference), lu(&i.prenom_reference)], " ");
        fn differe<'a>(reference: &'a Option<String>, declare: &Option<String>) -> Option<&'a str> {
            lu(reference).filter(|r| Some(*r) != lu(declare))
        }
        let mut ecarts = Vec::new();
        if let Some(r) = differe(&i.nom_reference, &i.nom_declare) {
            ecarts.push(format!("nom de référence « {} »", r));
        }
        if let Some(r) = differe(&i.prenom_reference, &i.prenom_declare) {
            ecarts.push(format!("prénom de référence « {} »", r));
        }
        if let Some(r) = differe(&i.naissance_reference, &i.naissance_declaree) {
            ecarts.push(format!("née ou né le {} selon le répertoire{}", date_lisible(r), morceau(" (déclaré : ", lu(&i.naissance_declaree).map(date_lisible).as_deref(), ")")));
        }
        if let Some(r) = differe(&i.nir_reference, &i.nir_declare) {
            ecarts.push(format!("NIR de référence {}{}", r, morceau(" (déclaré : ", lu(&i.nir_declare), ")")));
        }
        let qui = morceau(" : ", declare.or(reference).as_deref(), "");
        let quoi = if ecarts.is_empty() { "à vérifier".to_string() } else { ecarts.join(", ") };
        l.phrases.push(format!("Identité en écart avec le répertoire national (SNGI){} — {}. La fiche du salarié est à corriger avec l'identité de référence.", qui, quoi));
    }
    for i in &crm.informations {
        let qui: Vec<String> = i.individus.iter().filter_map(|x| joint([lu(&x.nom), lu(&x.prenom)], " ")).collect();
        let qui = (!qui.is_empty()).then(|| qui.join(", "));
        l.phrases.push(format!(
            "Information{}{}{}{}{}.",
            morceau(" ", lu(&i.code), ""),
            morceau(" : ", joint([lu(&i.libelle), lu(&i.information)], " — ").as_deref(), ""),
            morceau(", n° ", lu(&i.numero), ""),
            morceau(" (", qui.as_deref(), ")"),
            morceau(" — ", lu(&i.aide), "")
        ));
    }
    l
}

/// Un compte rendu d'organisme complementaire (`retours::lire_crm_oc`, d'apres la specification et le schema CRM DSN
/// OC P3 V1.1.4, `atelier/reference/sources/gip-mds-api-dsn/ctip-crm-dsn-oc-p3-v1.1.4-*`), dit en phrases : l'organisme
/// qui l'a produit, le code retour global, son complement, les mandats SEPA a creer, le contact ; chaque retour metier en
/// anomalie (code, libelle, valeurs, rubrique, salarie).
fn crm_oc_lu(crm: &CrmOc) -> Lecture {
    let mut l = Lecture::nouvelle(match crm.type_crm {
        Some(TypeDeCrmOc::Traitement) => "Compte rendu de traitement (CRT)",
        Some(TypeDeCrmOc::Complet) => "Compte rendu complet (CRC)",
        None => "Compte rendu de l'organisme complémentaire",
    });
    if crm.essai_reel == Some(EssaiOuReel::Essai) {
        l.phrases.push("Il répond à un envoi d'essai (test).".into());
    }
    for d in &crm.declarations {
        if let Some(p) = lu(&d.producteur) {
            l.organisme = Some(p.to_string());
        }
        let organisme = joint([lu(&d.producteur), d.code_producteur.as_ref().map(IdentifiantDOrganisme::as_str)], ", code ");
        let retour = match d.code_retour {
            Some(g) => g.libelle(),
            None => "sans code retour",
        };
        l.phrases.push(format!(
            "{}{}{} : {}.",
            nature_dsn(d.nature_declaration, d.type_declaration),
            morceau(" de ", d.mois.map(|m| m.to_string()).as_deref(), ""),
            morceau(", vue par ", organisme.as_deref(), ""),
            retour
        ));
        if let Some(c) = lu(&d.complement) {
            l.phrases.push(c.to_string());
        }
        let compte = |n: &Option<String>, un: &str, plusieurs: &str| -> Option<String> {
            lu(n).filter(|n| *n != "0").map(|n| format!("{} {}", n, if n == "1" { un } else { plusieurs }))
        };
        let comptes: Vec<String> = [
            compte(&d.erreurs, "erreur", "erreurs"),
            compte(&d.critiques, "erreur critique", "erreurs critiques"),
            compte(&d.affiliations, "affiliation à prendre en compte", "affiliations à prendre en compte"),
            compte(&d.radiations, "radiation prise en compte", "radiations prises en compte"),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !comptes.is_empty() {
            l.phrases.push(format!("{}.", comptes.join(", ")));
        }
        for m in &d.mandats {
            let ou = match (m.net_entreprises, lu(&m.url)) {
                (true, _) => "à créer sur net-entreprises (module Mandat)".to_string(),
                (false, None) => "à créer auprès de l'organisme".to_string(),
                (false, Some(u)) => format!("à créer sur le site de l'organisme : {}", u),
            };
            l.phrases.push(format!("Pas de mandat de prélèvement SEPA{} : {}.", morceau(" pour le compte ", lu(&m.iban), ""), ou));
        }
        if let Some(contact) = joint([lu(&d.contact), lu(&d.courriel), lu(&d.telephone)], ", ") {
            l.phrases.push(format!("Contact chez l'organisme : {}.", contact));
        }
        for r in &d.retours {
            let genre = match lu(&r.code).and_then(|c| c.chars().next()) {
                Some('E') => Some("erreur"),
                Some('I') => Some("information"),
                _ => None,
            };
            let mut message: Vec<String> = lu(&r.libelle).into_iter().chain(lu(&r.description)).map(String::from).collect();
            if let Some(v) = joint([lu(&r.valeur), lu(&r.libelle_valeur).map(|x| format!("({})", x)).as_deref()], " ") {
                message.push(format!("déclaré « {} »", v));
            }
            if let Some(c) = joint([lu(&r.corrigee), lu(&r.libelle_corrigee).map(|x| format!("({})", x)).as_deref()], " ") {
                message.push(format!("{} « {} »", if genre == Some("information") { "pris en compte" } else { "attendu" }, c));
            }
            if let Some(e) = r.date_evenement {
                message.push(format!("événement du {}", e.jj_mm_aaaa()));
            }
            l.anomalies.push(Anomalie {
                code: lu(&r.code).map(String::from),
                categorie: joint([genre, r.criticite.map(CriticiteOc::libelle)], ", "),
                message: joint(message.iter().map(|m| Some(m.as_str())), " — "),
                rubrique: joint([lu(&r.rubrique), lu(&r.libelle_rubrique).map(|x| format!("({})", x)).as_deref()], " "),
                ligne: None,
                nombre: lu(&r.nombre).map(String::from),
                salarie: lu(&r.salarie).map(String::from),
            });
        }
    }
    l
}

/// La piece archivee d'un retour : aucune, illisible (le disque le dit), ou lue.
enum PieceDuRetour {
    Aucune,
    Illisible(String),
    Lue(PathBuf, Lecture),
}

/// Un retour d'une ligne du journal, lu.
fn retour_lu(racine: &Path, ligne: &Ligne, r: &envois::journal::RetourOrganisme) -> R<RetourLu> {
    let canal = (ligne.acte() == envois::journal::Acte::Dpae).then_some("dpae");
    let nature = r.nature_retour;
    let statut = r.statut;
    let publication = r.publication;
    let flux = ligne.flux();
    let (organisme, format) = match canal {
        Some(_) => (Some("l'Urssaf (DPAE)"), None),
        None => (nature.organisme(), nature.format()),
    };
    let libelle = match dit(&r.libelle) {
        Some(l) => l.to_string(),
        None => nature.libelle_dit(),
    };
    let piece = match (flux, publication) {
        (None, None) => PieceDuRetour::Aucune,
        _ => match piece_du_retour(racine, canal, flux, nature, publication) {
            Ok(None) => PieceDuRetour::Aucune,
            Ok(Some(p)) => match std::fs::read(&p) {
                Ok(o) => {
                    let l = lire_piece(&o, canal, nature, format);
                    PieceDuRetour::Lue(p, l)
                }
                Err(e) => PieceDuRetour::Illisible(format!("la pièce archivée ne se lit pas ({:?})", e.kind())),
            },
            Err(m) => PieceDuRetour::Illisible(m.phrase),
        },
    };
    let mut phrases: Vec<String> = Vec::new();
    let lu_verdict = match &piece {
        PieceDuRetour::Lue(_, l) => l.verdict,
        _ => None,
    };
    match (lu_verdict, statut) {
        (Some(v), Some(s)) if s.verdict() != Some(v) => {
            let sans_anomalie = matches!(&piece, PieceDuRetour::Lue(_, l) if l.anomalies.is_empty());
            let dit_la_piece = match (v, StatutDuRetour::du_verdict(v).phrase()) {
                (Verdict::Ok, _) if sans_anomalie => "aucune anomalie",
                (_, Some(p)) => p,
                (autre, None) => autre.code(),
            };
            phrases.push(format!("Publié au statut {}, mais la pièce dit : {} ({}).", s, dit_la_piece, v.code()));
        }
        (_, Some(s)) => {
            if let Some(p) = s.phrase() {
                phrases.push(format!("Statut {} : {}.", s, p));
            }
        }
        (_, None) => {}
    }
    let (titre, anomalies, traduite, lu_organisme, chemin) = match piece {
        PieceDuRetour::Lue(p, l) => {
            phrases.extend(l.phrases);
            (Some(l.titre), l.anomalies, l.traduite, l.organisme, Some(p))
        }
        sans => {
            // Sans piece lue : ce que le releve en a lu quand il l'a rattachee (`journal.rattacher_retours`), et rien d'autre.
            if let PieceDuRetour::Illisible(m) = &sans {
                phrases.push(format!("La pièce de ce retour ne se lit pas sur ce poste : {} ; ce qui est dit ici vient du relevé.", m));
            } else if format == Some("aucun") {
                phrases.push("Un accusé de réception, sans document : son statut dit tout.".into());
            } else if flux.is_some_and(IdDeFlux::est_fictif) {
                // Le retour d'un depot joue par une compagnie fictive (`envois::releve_retours::rattacher_simulation`, qui
                // le reconnait a son flux) : le bilan est joue sans piece ; seule l'AER (nature 46) a la sienne, fabriquee
                // (`aer_fictive`).
                phrases.push("Compagnie fictive : ce retour est joué par le logiciel, sans pièce d'organisme.".into());
            } else {
                phrases.push("La pièce de ce retour n'est pas archivée sur ce poste : ce qui est dit ici vient du relevé.".into());
            }
            let anomalies: Vec<Anomalie> = r.anomalies().into_iter().flatten().map(|a| Anomalie::du_message(a)).collect();
            for d in r.personnes().into_iter().flatten() {
                let etat = if d.enregistree { "enregistrée".to_string() } else if d.deja_existante { "déjà enregistrée auparavant".into() } else { format!("refusée : {}", d.libelle) };
                phrases.push(format!("{} : {}.", joint([dit(&d.nom), dit(&d.prenom)], " ").into_iter().collect::<String>(), etat));
            }
            (None, anomalies, false, None, None)
        }
    };
    // L'organisme que la piece nomme passe devant celui de la nature (« l'organisme complementaire » : lequel).
    let organisme = lu_organisme.or(organisme.map(String::from));
    let verdict = lu_verdict.or(r.verdict());
    // Sa pastille : celle du verdict (`Verdict::a_l_ecran`) ; un statut qui n'est pas un verdict (« EN COURS ») est une
    // information.
    let pastille = match (verdict, statut) {
        (Some(v), _) => Some(Pastille::de(v.a_l_ecran(), v.code())),
        (None, Some(s)) => Some(Pastille::de(
            match s.verdict() {
                Some(v) => v.a_l_ecran(),
                None => socle::ecran::EtatDEcran::Information,
            },
            s.code(),
        )),
        (None, None) => None,
    };
    let titre_dit = match titre.as_deref().and_then(dit) {
        Some(t) => t.to_string(),
        None => libelle.clone(),
    };
    let nature_dit = format!("Nature {} : {}", nature, libelle);
    let quand = publication;
    Ok(RetourLu {
        titre: titre_dit.clone(),
        quand,
        phrases,
        pastille,
        piece: chemin.map(|p| CheminRelatif::sous(racine, &p)).transpose()?,
        organisme: Some(RetourDOrganisme {
            titre_dit,
            nature_dit,
            quand_dit: quand.map(Moment::dit),
            nature_retour: nature,
            libelle,
            organisme,
            statut,
            verdict,
            anomalies,
            traduite,
        }),
    })
}

// ------------------------------------------------------------------ la route

/// `historique-mois {mois}`.
fn historique(ctx: &Contexte, le_mois: Mois) -> R<Historique> {
    // Les lignes du mois ; un signalement porte le mois de son evenement.
    let lignes = envois::journal::lire(&ctx.racine, None, Some(le_mois), false).map_err(|m| m.reclasser(Classe::Disque))?;
    let recus = etats::contrats_recus(&lignes);
    let avenants = etats::avenants_par_contrat(&lignes);
    let mut envois = Vec::new();
    for l in lignes.iter() {
        // Ce qui est revenu, sur la ligne de chaque envoi : un contrat signe, une fiche remplie (ou la demande abandonnee),
        // un retour d'organisme.
        let mut retours: Vec<RetourLu> = match l {
            Ligne::ContratSignature(e) => e.retours.iter().map(|r| revenu_lu(ctx, l, r)).collect::<R<_>>()?,
            Ligne::FicheDemande(e) => e
                .retours
                .iter()
                .map(|r| RetourLu {
                    titre: if r.abandon { "Demande abandonnée" } else { "Fiche reçue" }.to_string(),
                    quand: Some(r.quand),
                    phrases: Vec::new(),
                    pastille: None,
                    piece: None,
                    organisme: None,
                })
                .collect(),
            _ => l.retours_organisme().iter().map(|r| retour_lu(&ctx.racine, l, r)).collect::<R<_>>()?,
        };
        retours.sort_by(|a, b| a.quand.cmp(&b.quand));
        // Un depot attend ses retours par son flux (`journal.rattacher_retours`).
        envois.push(Envoi {
            quoi: l.acte().nom(),
            acte: acte(ctx, l),
            mode: l.mode().nom(),
            quand: l.quand(),
            fichier: l.fichier().cloned(),
            flux: l.flux().cloned(),
            destinataire: dit(l.destinataire()).map(String::from),
            identite: identite(ctx, l),
            accepte: l.accepte(),
            resume: etats::resume(Some(l), &recus, &avenants),
            attend_retour: l.flux().is_some(),
            retours,
        });
    }
    // Le plus recent en premier : c'est le dernier envoi qu'on vient lire ; a la meme heure, le reel devant son test.
    envois.sort_by(|a, b| (&b.quand, b.mode == "reel").cmp(&(&a.quand, a.mode == "reel")));
    let documents = documents(ctx, le_mois, &lignes)?;
    let phrase = format!("Historique de {} : {} documents, {} envois.", le_mois.libelle(socle::mois::Graphie::Accentuee), documents.len(), envois.len());
    Ok(Historique { phrase, mois: le_mois, documents, envois })
}
