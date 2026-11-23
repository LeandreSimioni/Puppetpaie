//! UN MOIS IMPORTE SE LIT PAR SES ENGAGEMENTS IMPORTES (`engagement::importe`) : un mois tenu avant ce logiciel n'a
//! que des engagements que l'import a ecrits, valides et partis, qui se lisent et ne se modifient pas. Ce module le rend
//! tel que la page Paie le montre : une CARTE PAR ENGAGEMENT (`CarteImportee`), la ligne fermee « importé », avec ses
//! cases (salarie, spectacle, debut et fin, jours travailles, cachets et heures, montants, etats), remplies de ce que
//! les pieces importees portent, et rapprochees de ce que le moteur sait par ailleurs -- la fiche du salarie (son nom),
//! celle du spectacle (son titre), le journal des depots et les certificats DPAE (`retours/dpae/`). Aucune carte n'a de
//! geste.
//!
//! RIEN NE SE DEVINE (regle 1) : une case qu'aucune piece ne porte reste vide, et la carte DIT pourquoi (`manque` : quel
//! champ, et ce qui l'aurait donne). Rien ne s'ecrit ici : c'est une lecture.

use droit::fiche::Lue as _;
use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use droit::contexte::Contexte;
use engagement::importe::{Attestation, BulletinSource, ContratRelu, Importe, LigneImprimee, PaieRelue, PaieVersee};
use envois::etats;
use socle::date::{Jour, Periode};
use socle::echec::R;
use socle::ecran::{ColonneRendue, EtatDEcran, QuandDeLaCarte, RangeeRendue, SorteDeRangee};
use crate::reponse::Source;
use socle::fichiers::{CheminRelatif, NomDeFichier};
use socle::grandeurs::Cachets;
use socle::mois::{Graphie, Mois};
use socle::grandeurs::Montant;
use crate::proposee::A_COMPLETER;

/// Les engagements importes qui touchent `mois`.
fn importes_du_mois(racine: &Path, mois: Mois) -> R<Vec<Importe>> {
    engagement::importe::du_mois(racine, mois)
}

/// Un bulletin que l'ancien logiciel a fait, tel que l'import le garde : sa copie (relative a la compagnie), et le
/// salarie qu'il paie.
pub struct BulletinRepris {
    pub chemin: CheminRelatif,
    pub salarie: droit::contexte::IdSalarie,
}

/// Le dossier de l'historique importe, dans la compagnie : il ne tient que les copies (`BULLETINS_REPRIS`).
pub const HISTORIQUE: NomDeFichier = NomDeFichier::du_moteur("historique");

/// Le dossier des copies que l'import garde, dans `HISTORIQUE`.
const BULLETINS_REPRIS: NomDeFichier = NomDeFichier::du_moteur("bulletins");

/// Le dossier des copies que l'import garde, relatif a la compagnie (`historique/bulletins`).
fn bulletins_repris_relatif() -> CheminRelatif {
    CheminRelatif::from(HISTORIQUE).joint(&BULLETINS_REPRIS)
}

/// Le dossier des copies que l'import garde, sous la compagnie `racine`.
pub fn dossier_des_bulletins(racine: &Path) -> std::path::PathBuf {
    bulletins_repris_relatif().dans(racine)
}

/// LES BULLETINS REPRIS DE `mois` : la copie que l'import garde de chaque fichier lu (`historique/bulletins/`, le nom que
/// porte la paie importee), un par fichier, avec le salarie qu'il paie. Un fichier qui manque ne se liste pas.
pub fn bulletins_repris(racine: &Path, mois: Mois) -> R<Vec<BulletinRepris>> {
    let mut out: Vec<BulletinRepris> = Vec::new();
    for p in engagement::importe::paies_lues(racine)? {
        if p.mois != mois {
            continue;
        }
        let Some(fichier) = p.paie.bulletin.as_ref().map(|b| &b.fichier) else { continue };
        let chemin = bulletins_repris_relatif().joint(fichier);
        if !chemin.dans(racine).is_file() || out.iter().any(|x| x.chemin == chemin) {
            continue;
        }
        out.push(BulletinRepris { chemin, salarie: p.salarie });
    }
    Ok(out)
}

/// Les mois que les engagements importes touchent, qu'aucun engagement de la saisie ne touche.
pub fn mois_importes(racine: &Path) -> R<Vec<Mois>> {
    let mois: BTreeSet<Mois> = engagement::importe::tous(racine)?.iter().flat_map(Importe::mois).collect();
    let mut out = Vec::new();
    for m in mois {
        if !engagement::stock::a_une_saisie(racine, m)? {
            out.push(m);
        }
    }
    Ok(out)
}

/// LA DSN D'UN MOIS DONT L'ANCIEN LOGICIEL A DEPOSE LA DSN, que la page Paie et la boite d'ouverture disent avec les
/// memes mots : `None` si l'ancien logiciel n'a pas depose la DSN de `mois`. Sinon ce que la saisie ajoute a ce mois et
/// qui n'est pas encore declare (`engagements`, en saisie ou valides) se declare dans la DSN de `porteur`, la premiere
/// DSN d'apres qui n'est pas partie, comme un contrat ajoute apres la DSN de son mois
/// (`produire_regularisations::regularisations_pour` : cahier technique DSN 2026.1, § 2.1.2.1,
/// `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`). La DSN de l'ancien logiciel ne se
/// refait pas ici (regle 4).
pub fn dsn_de_l_ancien_logiciel(ctx: &Contexte, mois: Mois) -> R<Option<DsnDeLAncienLogiciel>> {
    if engagement::importe::dsn_deposee(&ctx.racine, mois)?.is_none() {
        return Ok(None);
    }
    let porteur = compagnie::etat_dsn::dsn_qui_porte(ctx, mois)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let mut n = 0;
    for e in engagement::etats::du_mois(ctx, mois, &departs)? {
        // Un engagement valide ne s'y declare que si sa correction est due ; un engagement en saisie, toujours.
        if let Some(v) = e.valide() {
            if engagement::etats::correction_due(ctx, v, &departs, mois, Some(porteur))?.is_none() {
                continue;
            }
        }
        n += 1;
    }
    let lib = porteur.libelle(Graphie::Accentuee);
    let phrase = match n {
        0 => "DSN déposée par l'ancien logiciel.".to_string(),
        1 => format!("DSN déposée par l'ancien logiciel ; cet engagement se déclare dans la DSN de {}.", lib),
        _ => format!("DSN déposée par l'ancien logiciel ; ces {} engagements se déclarent dans la DSN de {}.", n, lib),
    };
    Ok(Some(DsnDeLAncienLogiciel { porteur, engagements: n, phrase }))
}

/// La DSN d'un mois deposee par l'ancien logiciel (`dsn_de_l_ancien_logiciel`) : le mois de la DSN qui porte ce que la
/// saisie y ajoute, combien d'engagements s'y declarent, et la phrase qui le dit.
pub struct DsnDeLAncienLogiciel {
    pub porteur: Mois,
    pub engagements: usize,
    pub phrase: String,
}

/// LA DSN MENSUELLE D'UN MOIS DE L'ANCIEN LOGICIEL NE SE FABRIQUE PAS ICI (regle 4 ; un mois importe ne recoit pas
/// d'engagement neuf) : celle que l'import a lue (`dsn_de_l_ancien_logiciel`), ou, sans elle, celle d'un mois tenu avant
/// ce logiciel (`importe` : `compagnie::etat_dsn` le dit « aucune trace de sa DSN ») -- l'ancien logiciel a calcule ces
/// paies, sa DSN est la sienne, et elle se garde en l'important (la route `importer`). Le refus que `fabriquer_dsn`
/// oppose avant d'ecrire quoi que ce soit, le seul.
pub fn refuser_la_dsn_de_l_ancien_logiciel(ctx: &Contexte, mois: Mois) -> R<()> {
    let phrase = match dsn_de_l_ancien_logiciel(ctx, mois)? {
        Some(d) => format!("{} Elle ne se refait pas ici.", d.phrase),
        None if compagnie::etat_dsn::importe(&ctx.racine, mois)? => "mois tenu avant PuppetPaie, sa DSN est celle de l'ancien logiciel et ne se fait pas ici. Pour la garder, mettez son fichier dans le dossier d'import, puis « Importer » (Configuration › Reprendre l'historique).".to_string(),
        None => return Ok(()),
    };
    Err(socle::echec::erreur(socle::echec::Classe::DsnDeLAncienLogiciel, format!("{} : {} Rien n'est écrit.", mois.libelle(Graphie::Accentuee), phrase)))
}

// ------------------------------------------------------------------ les cartes

/// Le nom d'un salarié à l'écran, ou `None` sans fiche.
fn nom_du_salarie(ctx: &Contexte, cle: &droit::contexte::IdSalarie) -> Option<String> {
    ctx.salaries.get(cle).and_then(|s| s.nomme())
}

/// LE TITRE DE LA FICHE DU SPECTACLE d'une paie importee `p` ou d'un contrat atteste `c`, par la seule regle
/// (`engagement::importe::spectacle_de_la_paie`) ; vide quand la fiche ne le dit pas, qu'il est « A COMPLETER », ou
/// qu'aucune fiche ne porte encore ce spectacle : cela manque a la carte, et ne ferme pas la lecture du mois. Une panne
/// (une fiche illisible) remonte.
fn titre_du_spectacle(ctx: &Contexte, p: Option<&PaieVersee>, c: Option<&engagement::importe::ContratAtteste>) -> R<Option<String>> {
    let cle = match engagement::importe::spectacle_de_la_paie(ctx, p, c) {
        Ok(Some(cle)) => cle,
        Ok(None) => return Ok(None),
        Err(e) if e.dit_un_manque() => return Ok(None),
        Err(e) => return Err(e),
    };
    Ok(ctx.spectacles.get(&cle).and_then(|s| s.titre.portee()).map(str::trim).filter(|t| !t.eq_ignore_ascii_case(A_COMPLETER)).map(str::to_string))
}

/// LA PERIODE D'UN CONTRAT IMPORTE DANS `mois`, en clair, quand le bulletin ne dit pas ses jours : « 20–27 » pour un
/// contrat qui commence et finit dans le mois (le 1er quand il a commence avant), « 20/11 – 03/12 » pour un contrat a
/// cheval sur un autre mois (« 01/09/2022 – 31/12 » quand il a commence une autre annee), « … » pour une borne que
/// l'import n'a pas lue. Vrai avec elle : la periode est a cheval sur un autre mois (elle se dit eparse).
fn periode_en_clair(d: Option<Jour>, f: Option<Jour>, mois: Mois) -> (String, bool) {
    let dans = |j: &Jour| mois.contient(j);
    let dehors = d.as_ref().is_some_and(|j| !dans(j)) || f.as_ref().is_some_and(|j| !dans(j));
    let borne = |j: Option<&Jour>, inconnue: &str| match j {
        Some(j) if dehors && j.annee() != mois.annee() =>j.jj_mm_aaaa(),
        Some(j) if dehors => j.jj_mm(),
        Some(j) => j.quantieme_ecrit(),
        None => inconnue.to_string(),
    };
    let (a, b) = (borne(d.as_ref(), if dehors { "…" } else { "1" }), borne(f.as_ref(), "…"));
    match (a == b, dehors) {
        (true, _) => (a, false),
        (false, true) => (format!("{} – {}", a, b), true),
        (false, false) => (format!("{}–{}", a, b), false),
    }
}

/// Les jours des dates travaillees d'un bulletin, dans `mois`.
fn jours_du_mois(dates: &[Periode], mois: Mois) -> Vec<Jour> {
    let mut out = Vec::new();
    for j in dates.iter().flat_map(|d| d.jours()) {
        if mois.contient(&j) && !out.contains(&j) {
            out.push(j);
        }
    }
    out.sort();
    out
}

/// UNE CASE QU'AUCUNE PIECE NE PORTE : laquelle, son libelle, et ce qui l'aurait donnee.
#[derive(Debug, Clone)]
struct Manque {
    champ: &'static str,
    libelle: &'static str,
    phrase: String,
    dit: &'static str,
}

fn manque(champ: &'static str, libelle: &'static str, phrase: String) -> Manque {
    Manque { champ, libelle, phrase, dit: "non lu" }
}

/// L'ETAT D'UN ACTE D'UNE CARTE (la DPAE, la signature, les documents) : sa couleur, ses mots courts, sa phrase.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EtatDeCarte {
    /// Son etat, ecrit par sa couleur (`socle::ecran::couleur_de_l_etat`).
    #[serde(rename = "couleur", serialize_with = "couleur_ecrite")]
    pub(crate) etat: EtatDEcran,
    courte: &'static str,
    phrase: String,
}

/// L'etat d'un acte d'une carte, ecrit par sa couleur.
fn couleur_ecrite<S: serde::Serializer>(etat: &EtatDEcran, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(socle::ecran::couleur_de_l_etat(*etat))
}

/// Les etats des actes d'une carte.
#[derive(Debug, Clone)]
pub(crate) struct EtatsDeCarte {
    pub(crate) dpae: EtatDeCarte,
    signature: EtatDeCarte,
    documents: EtatDeCarte,
}

/// L'etat de la DPAE d'une embauche tenue avant : le journal (reel accepte, par le numero du contrat), puis les certificats
/// de `retours/dpae/` (par le salarie et le jour d'embauche qu'ils attestent).
fn dpae_de(journal: &std::collections::HashMap<etats::Paire, socle::paris::InstantOuJour>, certifiees: &[envois::reprise_journal::DpaeAttestee], salarie: Option<&droit::contexte::IdSalarie>, numero: Option<&engagement::numeros::NumeroContrat>, debut: Option<Jour>) -> EtatDeCarte {
    if let Some(q) = salarie.and_then(|s| journal.get(&(s.clone(), numero.cloned()))) {
        return EtatDeCarte { etat: EtatDEcran::Fait, courte: "DPAE faite", phrase: format!("DPAE déposée le {}", q.dit()) };
    }
    let certifiee = certifiees.iter().find(|c| c.embauches.iter().any(|e| Some(&e.salarie) == salarie && Some(e.debut) == debut));
    if let Some(c) = certifiee {
        let quand: String = c.quand.iter().map(|q| format!(" ({})", q.dit())).collect();
        return EtatDeCarte { etat: EtatDEcran::Fait, courte: "DPAE faite", phrase: format!("DPAE certifiée par l'Urssaf{}", quand) };
    }
    EtatDeCarte { etat: EtatDEcran::Information, courte: "avant PuppetPaie", phrase: "DPAE faite avant PuppetPaie : aucun certificat de l'Urssaf n'est relevé pour cette embauche".to_string() }
}

/// UNE CARTE D'ENGAGEMENT IMPORTE, telle qu'elle se lit (la page Paie la pose : `carteImportee` de js/saisie-mois.js) :
/// le salarie qu'elle paie, son resume (la carte fermee), ce que l'import n'a pas lu, ses cases en lecture et les lignes
/// de son bulletin. Rien ne s'y saisit.
#[derive(Debug, Clone, Serialize)]
pub struct CarteImportee {
    salarie: Option<droit::contexte::IdSalarie>,
    resume: ResumeDeLaCarte,
    #[serde(skip_serializing_if = "Option::is_none")]
    non_lu: Option<NonLuDeLaCarte>,
    cases: Vec<CaseLue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lignes: Option<LignesDeLaCarte>,
    /// Son debut et son nom, qui rangent les cartes du mois.
    #[serde(skip)]
    debut: Option<Jour>,
    #[serde(skip)]
    nom: Option<String>,
    #[serde(skip)]
    pub(crate) bulletin: Option<BulletinSource>,
    #[serde(skip)]
    pub(crate) etats: EtatsDeCarte,
}

/// LA CARTE FERMEE : quand (`QuandDeLaCarte` : les jours travailles, ou la periode du contrat, et au survol leurs dates,
/// comme sur la carte d'un engagement) ; qui (le nom, le mot
/// « importé », les pieces lues au survol) ; quoi ; ses pastilles ; le montant que la carte dit (le net a payer ; sans
/// lui, le brut, que son mot precede) et ce qu'il est, au survol.
#[derive(Debug, Clone, Serialize)]
struct ResumeDeLaCarte {
    quand: QuandDeLaCarte,
    /// Le nom du salarie ; aucun : ni la piece ni la fiche ne le disent.
    #[serde(skip_serializing_if = "Option::is_none")]
    nom: Option<String>,
    mot: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    origine_dit: Option<String>,
    quoi_dit: String,
    pastilles: Vec<PastilleDeLaCarte>,
    #[serde(skip_serializing_if = "Option::is_none")]
    montant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    montant_mot: Option<&'static str>,
    montant_titre: &'static str,
}

/// Une pastille de la carte : son titre, la couleur de son etat (`socle::ecran::couleur_de_l_etat`) et sa phrase.
#[derive(Debug, Clone, Serialize)]
struct PastilleDeLaCarte {
    titre: &'static str,
    #[serde(flatten)]
    etat: EtatDeCarte,
}

/// CE QUE L'IMPORT N'A PAS LU, en une ligne sur la carte fermee : son titre, les cases qu'il n'a pas lues, et ou lire
/// pourquoi.
#[derive(Debug, Clone, Serialize)]
struct NonLuDeLaCarte {
    titre: &'static str,
    quoi: String,
    dit: &'static str,
}

/// UNE CASE DE LA CARTE, EN LECTURE : son libelle ; sa valeur dite et la piece d'ou elle vient ; vide, ce qu'elle montre
/// a sa place (`vide_dit` : ce que l'import en dit quand il ne l'a pas lue, sinon le tiret), si l'import ne l'a pas lue
/// (`non_lu`), et pourquoi, sous la premiere case qui le dit.
#[derive(Debug, Clone, Serialize)]
struct CaseLue {
    libelle: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    valeur: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vide_dit: Option<&'static str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    non_lu: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pourquoi: Option<String>,
}

/// LES LIGNES DU BULLETIN, en tableau : ses colonnes (a droite pour un nombre), et chaque ligne ses cellules dites (vide :
/// ce que le bulletin n'imprime pas).
#[derive(Debug, Clone, Serialize)]
struct LignesDeLaCarte {
    colonnes: Vec<ColonneRendue>,
    rangees: Vec<RangeeRendue>,
}

/// Ce que dit une carte dont la periode est lue sans ses jours, au survol de « quand ».
const DATES_NON_LUES: &str = "Début et fin du contrat : les jours exacts ne sont pas lus.";
/// Ce que dit le montant d'une carte qui n'a pas lu son net a payer, au survol.
const BRUT_SEUL: &str = "Le brut : le net à payer n'est pas lu.";
/// Ou lire pourquoi une case n'est pas lue.
const POURQUOI_NON_LU: &str = "Un clic sur la carte montre ses cases, et sous chacune pourquoi.";


/// La piece qui atteste un contrat, ecrite comme la carte la montre.
fn attestation_dite(a: Attestation) -> &'static str {
    match a {
        Attestation::Aem => "AEM",
        Attestation::Bulletin => "bulletin",
        Attestation::Certificat => "certificat",
    }
}

/// La piece d'ou vient une valeur, ecrite comme la carte la montre : « DSN » en capitales, les autres par leur nom.
fn piece_dite(s: Source) -> String {
    match s {
        Source::Dsn => "DSN".to_string(),
        autre => autre.nom().to_string(),
    }
}

/// Une carte : une paie importee (et le contrat atteste qui va avec), ou un contrat atteste seul.
/// LES ECARTS CONNUS D'UNE PAIE IMPORTEE, tels que la carte les dit : ceux que son registre garde pour elle, ou son
/// registre illisible.
enum ConnusDeLaPaie<'a> {
    Lus(Vec<&'a crate::ecarts::EcartConnuNote>),
    Illisibles,
}

fn carte(ctx: &Contexte, mois: Mois, paie: Option<&PaieRelue>, contrat: Option<&ContratRelu>, dpae: EtatDeCarte, connus: ConnusDeLaPaie) -> R<CarteImportee> {
    let p = paie.map(|x| &x.paie);
    let c = contrat.map(|x| &x.contrat);
    let plein = |t: String| Some(t.trim().to_string()).filter(|t| !t.is_empty());
    let salarie = paie.map(|x| x.salarie.clone()).or_else(|| contrat.map(|x| x.salarie.clone()));
    let atteste = contrat.and_then(|x| x.periode);
    let debut = p.and_then(|p| p.periode).map(|o| o.debut()).or_else(|| atteste.map(|o| o.debut()));
    let fin = p.and_then(|p| p.periode).and_then(|o| o.fin()).or_else(|| atteste.and_then(|o| o.fin()));
    let numero = engagement::importe::numero_objet_de(p, c).cloned();
    let titre_bulletin = p.and_then(|p| p.spectacle.clone()).and_then(plein);
    let titre = titre_du_spectacle(ctx, p, c)?.or(titre_bulletin);
    let nom = salarie.as_ref().and_then(|s| nom_du_salarie(ctx, s));
    let dates: Vec<Jour> = p.map(|p| jours_du_mois(&p.dates, mois)).into_iter().flatten().collect();
    // L'activite du mois que les pieces disent (`engagement::importe::activite_du_mois`) ; quand elles n'en disent rien
    // pour le mois, ce que la piece du contrat entier atteste.
    let bornes = contrat.and_then(|x| x.periode);
    let du_mois = engagement::importe::activite_du_mois(mois, p, c, bornes.map(|o| o.debut()), bornes.and_then(|o| o.fin()));
    let (cachets, heures) = match du_mois.est_muette() {
        true => (c.and_then(|c| c.cachets), c.and_then(|c| c.heures)),
        false => (du_mois.cachets, du_mois.heures),
    };
    let bulletin = p.and_then(|p| p.bulletin.clone());
    // La piece qui declare la paie : sa cle le dit (`CleDePaieVersee`).
    let declaree_par = |p: &PaieVersee| if p.contrat.d_un_bulletin() { Source::Bulletin } else { Source::Dsn };
    let declare_par = p.map(declaree_par);
    let par_aem = c.is_some_and(|c| c.atteste_par == Attestation::Aem);

    // Les montants, chacun sa case et d'ou il vient : le bulletin fait foi sur ceux qu'il imprime
    // (`import::bulletins::completer`).
    let lus: [(&str, &'static str, fn(&PaieVersee) -> Option<Montant>); 8] = [
        ("brut", "Brut", |p| p.brut),
        ("net_imposable", "Net imposable", |p| p.nets.net_imposable),
        ("net_a_payer", "Net à payer", |p| p.net_a_payer),
        ("pas", "Prélèvement à la source", |p| p.nets.pas),
        ("net_verse", "Net versé (DSN)", |p| p.nets.net_verse),
        ("cotisations_salariales", "Cotisations salariales", |p| p.cotisations_salariales),
        ("cotisations_patronales", "Cotisations patronales", |p| p.cotisations_patronales),
        ("cout_employeur", "Coût employeur", |p| p.cout_employeur),
    ];
    let montants: Vec<(&str, &'static str, Option<(Montant, Source)>)> = lus
        .into_iter()
        .map(|(k, libelle, lu)| {
            // Le montant que la paie porte ; sans paie, le brut que l'AEM atteste.
            let porte = match (p, k == "brut") {
                (Some(p), _) => lu(p).map(|d| (d, if bulletin.is_some() && k != "net_verse" { Source::Bulletin } else { declaree_par(p) })),
                (None, true) => c.and_then(|c| c.brut).map(|b| (b, Source::Aem)),
                (None, false) => None,
            };
            (k, libelle, porte)
        })
        .collect();
    let montant = |k: &str| montants.iter().find(|(x, _, _)| *x == k).and_then(|(_, _, m)| m.map(|(v, _)| v));

    // CE QUI MANQUE, case par case, et ce qui l'aurait donne.
    let mut manques: Vec<Manque> = Vec::new();
    match (&salarie, &nom) {
        (None, _) => manques.push(manque("salarie", "salarié", "aucune pièce importée ne rattache cette paie à une fiche".into())),
        (Some(s), None) => manques.push(manque("salarie", "salarié", format!("la fiche « {} » n'existe pas (ou plus) : à créer dans Configuration › Salariés", s))),
        (Some(_), Some(_)) => {}
    }
    match (&numero, &titre) {
        (None, None) => manques.push(manque(
            "spectacle",
            "spectacle",
            "ni la DSN, ni l'AEM, ni le bulletin ne portent de numéro d'objet pour cette paie (un permanent n'en a pas)".into(),
        )),
        (Some(n), None) => manques.push(manque(
            "spectacle",
            "titre du spectacle",
            format!("la DSN ne porte que le numéro d'objet {} : son titre se complète dans Configuration › Spectacles", n),
        )),
        _ => {}
    }
    if dates.is_empty() {
        let pourquoi = match (bulletin.is_some(), declare_par, par_aem) {
            (true, _, _) => "le bulletin n'imprime pas ses dates travaillées",
            (false, Some(Source::Dsn), _) => "la DSN ne déclare que le début et la fin du contrat ; le bulletin de cette paie les donnerait",
            (false, _, true) => "l'AEM ne porte que le début, la fin et le nombre de cachets ; le bulletin de cette paie les donnerait",
            _ => "aucune pièce importée ne les porte",
        };
        manques.push(manque("dates", "jours travaillés", pourquoi.into()));
    }
    if contrat.is_none() {
        manques.push(manque(
            "cachets",
            "cachets et heures",
            if bulletin.is_some() {
                "le bulletin ne les donne pas en nombres entiers lisibles, et aucune AEM n'atteste ce contrat".into()
            } else {
                "la DSN ne les déclare pas, et aucune AEM ni aucun bulletin n'atteste ce contrat".into()
            },
        ));
    }
    for (k, libelle, porte, pourquoi) in [
        ("brut", "brut", montant("brut").is_some(), "aucune pièce importée ne le porte"),
        ("net_imposable", "net imposable", montant("net_imposable").is_some(), "aucune pièce importée ne le porte"),
        (
            "net_a_payer",
            "net à payer",
            montant("net_a_payer").is_some(),
            if declare_par == Some(Source::Dsn) {
                "la DSN ne le déclare pas (son 50.004 est le net versé) ; le bulletin de cette paie le donnerait"
            } else {
                "aucune pièce importée ne le porte ; le bulletin de cette paie le donnerait"
            },
        ),
        ("pas", "prélèvement à la source", montant("pas").is_some(), "aucune pièce importée ne le porte"),
    ] {
        if !porte {
            manques.push(manque(k, libelle, pourquoi.into()));
        }
    }

    let documents = match &bulletin {
        Some(b) => {
            let n: String = b.numero.iter().filter(|n| !n.is_empty()).map(|n| format!(" n° {}", n)).collect();
            EtatDeCarte { etat: EtatDEcran::Fait, courte: "bulletin importé", phrase: format!("bulletin de l'ancien logiciel{}, gardé : il se rouvre depuis l'Historique, à son mois", n) }
        }
        None => EtatDeCarte { etat: EtatDEcran::Information, courte: "avant PuppetPaie", phrase: "documents remis avant PuppetPaie : aucun bulletin importé pour cette paie".to_string() },
    };
    // CE QUE LA CARTE FERMEE DIT : le spectacle, les cachets et les heures ; au survol, les pieces lues.
    let mut quoi = Vec::new();
    if let Some(t) = titre.clone().or_else(|| numero.as_ref().map(ToString::to_string)) {
        quoi.push(t);
    }
    if let Some(n) = cachets.filter(|n| !n.est_nul()) {
        let dit = socle::texte::virgule(n);
        quoi.push(format!("{} {}", dit, if n > Cachets::UN { "cachets" } else { "cachet" }));
    }
    if let Some(n) = heures.filter(|n| !n.est_nul()) {
        quoi.push(format!("{} h", socle::texte::virgule(n)));
    }
    let quoi_dit = if quoi.is_empty() { "spectacle non lu".to_string() } else { quoi.join(" · ") };
    let mut lu = Vec::new();
    if declare_par == Some(Source::Dsn) {
        lu.push("DSN".to_string());
    }
    if let Some(b) = &bulletin {
        lu.push(match b.numero.as_deref().filter(|n| !n.is_empty()) {
            Some(n) => format!("bulletin {}", n),
            None => "bulletin".to_string(),
        });
    }
    if par_aem {
        lu.push("AEM".to_string());
    }
    let origine_dit = (!lu.is_empty()).then(|| format!("lu dans {}", lu.join(" + ")));
    let spectacle_dit = match (&titre, &numero) {
        (Some(t), Some(n)) if **n != **t => Some(format!("{} — n° {}", t, n)),
        (Some(t), _) => Some(t.clone()),
        (None, Some(n)) => Some(format!("n° {}", n)),
        (None, None) => None,
    };
    let nom = nom.or_else(|| salarie.as_ref().map(ToString::to_string));
    let etats = EtatsDeCarte {
        dpae,
        signature: EtatDeCarte { etat: EtatDEcran::Information, courte: "avant PuppetPaie", phrase: "contrat fait avant PuppetPaie : sa signature n'est pas suivie ici".to_string() },
        documents,
    };

    // QUAND : les jours travailles que le bulletin imprime ; sans eux, la periode du contrat.
    // EPARS : des jours qui ne se suivent pas, ou une periode a cheval sur un autre mois.
    let quand = match dates.is_empty() {
        false => QuandDeLaCarte::des_jours(&dates, mois)?,
        true => {
            let (jours, epars) = periode_en_clair(debut, fin, mois);
            QuandDeLaCarte { jours, epars, titre: DATES_NON_LUES.to_string(), mois: mois.nom(Graphie::Abregee) }
        }
    };
    // LE MONTANT : le net a payer ; sans lui (une DSN seule ne le declare pas), le brut, et la carte le dit.
    let (montant_dit, montant_mot) = match (montant("net_a_payer"), montant("brut")) {
        (Some(n), _) => (Some(socle::texte::euros(n)), None),
        (None, Some(b)) => (Some(socle::texte::euros(b)), Some("brut")),
        (None, None) => (None, None),
    };
    let resume = ResumeDeLaCarte {
        quand,
        nom: nom.clone(),
        mot: "importé",
        origine_dit,
        quoi_dit,
        pastilles: vec![
            PastilleDeLaCarte { titre: "DPAE", etat: etats.dpae.clone() },
            PastilleDeLaCarte { titre: "Contrat", etat: etats.signature.clone() },
            PastilleDeLaCarte { titre: "Documents", etat: etats.documents.clone() },
        ],
        montant_titre: if montant("net_a_payer").is_some() { "net à payer" } else { BRUT_SEUL },
        montant: montant_dit,
        montant_mot,
    };

    // CE QUE L'IMPORT N'A PAS LU : sur la carte fermee, en une ligne ; sous la premiere case vide qu'il touche, pourquoi.
    let non_lu = (!manques.is_empty()).then(|| NonLuDeLaCarte {
        titre: "Non lu par l'import",
        quoi: manques.iter().map(|m| m.libelle).collect::<Vec<_>>().join(" · "),
        dit: POURQUOI_NON_LU,
    });
    let mut dits: BTreeSet<&str> = BTreeSet::new();
    let mut case = |libelle: &'static str, valeur: Option<String>, source: Option<String>, champ: &str| {
        let valeur = valeur.filter(|v| !v.is_empty());
        let absent = match valeur {
            Some(_) => None,
            None => manques.iter().find(|m| m.champ == champ),
        };
        let pourquoi = absent.filter(|m| dits.insert(m.champ)).map(|m| m.phrase.clone());
        CaseLue {
            libelle,
            source: valeur.as_ref().and(source),
            vide_dit: match (&valeur, absent) {
                (Some(_), _) => None,
                (None, Some(m)) => Some(m.dit),
                (None, None) => Some("—"),
            },
            non_lu: absent.is_some(),
            pourquoi,
            valeur,
        }
    };
    let atteste = c.map(|c| attestation_dite(c.atteste_par).to_string());
    let mut cases = vec![
        case("Salarié", nom.clone(), Some("fiche".to_string()), "salarie"),
        case("Spectacle", spectacle_dit, None, "spectacle"),
        case("Contrat", p.map(|p| p.contrat.dite()), None, "contrat"),
        case("Début", debut.map(|j| j.jj_mm_aaaa()), None, "debut"),
        case("Fin", fin.map(|j| j.jj_mm_aaaa()), None, "fin"),
        case("Jours travaillés", (!dates.is_empty()).then(|| dates.iter().map(|j| j.cases()[0].clone()).collect::<Vec<_>>().join(" · ")), Some("bulletin".to_string()), "dates"),
        case("Cachets", cachets.map(socle::grandeurs::ecrit::sans_zero::dit), atteste.clone(), "cachets"),
        case("Heures", heures.map(socle::grandeurs::ecrit::sans_zero::dit), atteste, "cachets"),
    ];
    for &(k, libelle, porte) in &montants {
        cases.push(case(libelle, porte.map(|(v, _)| socle::texte::euros(v)), porte.map(|(_, s)| piece_dite(s)), k));
    }
    // CE QUE L'ANCIEN LOGICIEL A APPLIQUE AUTREMENT QUE LE TEXTE, deja releve (`engagement::ecarts_connus`) : la carte
    // dit l'explication et l'ecart chiffre.
    match connus {
        ConnusDeLaPaie::Lus(siens) => {
            for k in siens {
                cases.push(CaseLue { libelle: "Écart connu", valeur: Some(k.phrase()), source: Some(format!("bulletin {}", k.logiciel.as_str())), vide_dit: None, non_lu: false, pourquoi: None });
            }
        }
        ConnusDeLaPaie::Illisibles if p.is_some() => cases.push(CaseLue {
            libelle: "Écart connu",
            valeur: None,
            source: None,
            vide_dit: Some("ne se lit pas"),
            non_lu: true,
            pourquoi: Some(format!("Le registre des écarts connus ({}) ne se lit pas : l'explication des écarts connus de cette paie ne se montre pas. L'éditeur en est averti.", crate::ecarts::REGISTRE_DES_CONNUS)),
        }),
        ConnusDeLaPaie::Illisibles => {}
    }

    // LES LIGNES DU BULLETIN : ce que chacune paie, telle qu'une phrase le dit, et son montant.
    let lignes: Vec<RangeeRendue> = p
        .iter()
        .flat_map(|p| p.lignes.iter())
        .map(|l: &LigneImprimee| RangeeRendue {
            sorte: SorteDeRangee::Ligne,
            cellules: vec![
                Some(l.libelle.clone()),
                l.paiement.map(engagement::ligne::PaiementDeLigne::quantite_dite),
                l.paiement.map(|x| x.taux().dit()),
                l.montant.map(socle::texte::euros),
            ],
        })
        .collect();
    let lignes = (!lignes.is_empty()).then(|| LignesDeLaCarte {
        colonnes: vec![ColonneRendue::de_texte("Ligne du bulletin"), ColonneRendue::de_nombres("Quantité"), ColonneRendue::de_nombres("Taux"), ColonneRendue::de_nombres("Montant")],
        rangees: lignes,
    });
    Ok(CarteImportee { salarie, resume, non_lu, cases, lignes, debut, nom, bulletin, etats })
}

/// LES ETAPES D'UN MOIS TENU AVANT (`bilan_du_mois`, le panneau du mois) : tout s'est fait ailleurs,
/// aucune etape n'a de bouton -- ni « Fermer le mois », ni « Deposer la DSN » (regle 4). La DSN dit ce qu'on en
/// sait (`compagnie::etat_dsn`) ; sans trace, elle reste en alerte, sans geste.
pub fn etapes(vue: &VueImportee) -> EtapesImportees {
    let cartes = &vue.paies;
    let n = cartes.len();
    let pl = |n: usize| if n > 1 { "s" } else { "" };
    let dpae_faites = cartes.iter().filter(|c| c.etats.dpae.etat == EtatDEcran::Fait).count();
    let bulletins = cartes.iter().filter(|c| c.bulletin.is_some()).count();
    let ailleurs = n - dpae_faites;
    let retrouvees = format!("{} DPAE retrouvée{} dans le journal ou les certificats de l'Urssaf", dpae_faites, pl(dpae_faites));
    let sans_trace_ici = format!("{} DPAE faite{} ailleurs, sans trace relevée ici", ailleurs, pl(ailleurs));
    let dpae = match (dpae_faites, ailleurs) {
        (0, 0) => "Mois tenu avant PuppetPaie — aucune DPAE".to_string(),
        (_, 0) => format!("Mois tenu avant PuppetPaie — {}", retrouvees),
        (0, _) => format!("Mois tenu avant PuppetPaie — {}", sans_trace_ici),
        _ => format!("Mois tenu avant PuppetPaie — {} ; {}", retrouvees, sans_trace_ici),
    };
    let signature = match n {
        0 => "Mois tenu avant PuppetPaie — aucun contrat".to_string(),
        1 => "Mois tenu avant PuppetPaie — 1 contrat fait ailleurs : sa signature n'est pas suivie ici".to_string(),
        _ => format!("Mois tenu avant PuppetPaie — {} contrats faits ailleurs : leur signature n'est pas suivie ici", n),
    };
    let documents = match bulletins {
        0 => format!("Mois tenu avant PuppetPaie — aucun bulletin de l'ancien logiciel importé sur {} paie{}", n, pl(n)),
        b => format!("Mois tenu avant PuppetPaie — {} bulletin{} de l'ancien logiciel importé{} sur {} paie{}", b, pl(b), pl(b), n, pl(n)),
    };
    let dsn = &vue.dsn;
    let faite = |phrase: String| EtapeImportee { teinte: Teinte::Faite, courte: None, phrase: Some(phrase) };
    use compagnie::etat_dsn::Etat;
    EtapesImportees {
        dpae: faite(dpae),
        signature: faite(signature),
        // Faite ailleurs ; refusee ; sans trace ici, tenue par l'ancien logiciel : une information.
        dsn: EtapeImportee {
            teinte: match dsn.etat {
                Etat::Refusee => Teinte::Refusee,
                Etat::SansTrace => Teinte::SansTrace,
                _ => Teinte::Faite,
            },
            courte: dsn.courte.clone(),
            phrase: dsn.phrase.clone(),
        },
        documents: faite(documents),
    }
}

/// LES ETAPES D'UN MOIS TENU AVANT (`etapes`), une par etape du mois.
pub struct EtapesImportees {
    pub dpae: EtapeImportee,
    pub signature: EtapeImportee,
    pub dsn: EtapeImportee,
    pub documents: EtapeImportee,
}

/// Une etape d'un mois tenu avant : ce qu'on en sait, ce que la case en dit en court, et sa phrase.
pub struct EtapeImportee {
    pub teinte: Teinte,
    pub courte: Option<String>,
    pub phrase: Option<String>,
}

/// Ce qu'on sait d'une etape d'un mois tenu avant : faite ailleurs, refusee, ou sans trace ici.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Teinte {
    Faite,
    Refusee,
    SansTrace,
}

/// LES CARTES DES ENGAGEMENTS IMPORTES QUI TOUCHENT `mois`, qu'il soit tenu avant ce logiciel ou qu'il ait aussi des
/// engagements de la saisie : une carte par engagement importe, son contrat atteste et ce qui lui a ete verse ce
/// mois-la, dans l'ordre de leur debut.
pub fn cartes(ctx: &Contexte, mois: Mois) -> R<Vec<CarteImportee>> {
    let importes = importes_du_mois(&ctx.racine, mois)?;
    if importes.is_empty() {
        return Ok(Vec::new());
    }
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let embauches = etats::embauches_par_depot(&lignes);
    let (certifiees, _) = envois::reprise_journal::embauches_attestees(ctx)?;
    // UN REGISTRE DES ECARTS CONNUS ILLISIBLE ne ferme pas le mois : chaque carte qui a une paie dit que son explication
    // ne se lit pas, et l'editeur le sait par un incident.
    let connus = match crate::ecarts::ecarts_connus_notes(&ctx.racine) {
        Ok(v) => Some(v),
        Err(e) => {
            crate::ecarts::noter_registre_illisible(&ctx.racine, crate::ecarts::REGISTRE_DES_CONNUS, &e);
            None
        }
    };
    let mut cartes = Vec::new();
    for i in importes {
        let contrat = i.contrat_lu()?;
        let paie = i.paies_lues()?.into_iter().find(|p| p.mois == mois);
        let brut = i.objet().carte();
        let dpae = dpae_de(&embauches, &certifiees, brut.salarie.as_ref(), brut.numero.as_ref(), brut.debut);
        // Les ecarts connus que la comparaison de cette paie a constates (`crate::ecarts::EcartConnuNote`).
        let empreinte = i.paies().get(&mois).map(|p| engagement::ecarts::empreinte(&i, mois, p));
        let siens = match &connus {
            Some(v) => ConnusDeLaPaie::Lus(v.iter().filter(|k| Some(&k.paie) == empreinte.as_ref()).collect()),
            None => ConnusDeLaPaie::Illisibles,
        };
        cartes.push(carte(ctx, mois, paie.as_ref(), contrat.as_ref(), dpae, siens)?);
    }
    cartes.sort_by(|a, b| (&a.debut, &a.nom).cmp(&(&b.debut, &b.nom)));
    Ok(cartes)
}

/// LE MOIS TENU AVANT : la DSN que le moteur en sait, une carte par engagement importe, les totaux de ce qui est connu
/// et combien de cartes sont incompletes.
pub struct VueImportee {
    pub(crate) dsn: compagnie::etat_dsn::EtatDsn,
    pub(crate) paies: Vec<CarteImportee>,
}

/// LE MOIS TENU AVANT, comme le panneau du mois le lit : `None` si un engagement de la saisie touche le mois ou si aucun
/// engagement importe ne le touche.
pub fn vue(ctx: &Contexte, mois: Mois) -> R<Option<VueImportee>> {
    if !compagnie::etat_dsn::importe(&ctx.racine, mois)? {
        return Ok(None);
    }
    let paies = cartes(ctx, mois)?;
    let dsn = compagnie::etat_dsn::du_mois(ctx, mois, None)?;
    Ok(Some(VueImportee { dsn, paies }))
}
