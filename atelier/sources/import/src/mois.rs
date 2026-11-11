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
use engagement::importe::{Attestation, BulletinSource, ContratAtteste, ContratLu, DatesTravaillees, Declaration, Importe, LigneImprimee, PaieLue, PaieVersee};
use envois::etats;
use socle::date::Jour;
use socle::echec::{Classe, R};
use socle::mois::{Graphie, Mois};
use socle::num::Decimal;
use crate::proposee::A_COMPLETER;

/// Les engagements importes qui touchent `mois`.
fn importes_du_mois(racine: &Path, mois: Mois) -> R<Vec<Importe>> {
    engagement::importe::du_mois(racine, mois)
}

/// Un bulletin que l'ancien logiciel a fait, tel que l'import le garde : sa copie (relative a la compagnie), et le
/// salarie qu'il paie.
pub struct BulletinRepris {
    pub chemin: String,
    pub salarie: String,
}

/// LES BULLETINS REPRIS DE `mois` : la copie que l'import garde de chaque fichier lu (`historique/bulletins/`, le nom que
/// porte la paie importee), un par fichier, avec le salarie qu'il paie. Un fichier qui manque ne se liste pas.
pub fn bulletins_repris(racine: &Path, mois: Mois) -> R<Vec<BulletinRepris>> {
    let mut out: Vec<BulletinRepris> = Vec::new();
    for p in engagement::importe::paies_lues(racine)? {
        if p.mois != mois {
            continue;
        }
        let Some(fichier) = p.paie.bulletin.as_ref().map(|b| b.fichier.trim().to_string()) else { continue };
        if fichier.is_empty() || fichier.contains(['/', '\\']) || fichier.contains("..") {
            continue;
        }
        let chemin = format!("historique/bulletins/{}", fichier);
        if !racine.join(&chemin).is_file() || out.iter().any(|x| x.chemin == chemin) {
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
fn nom_du_salarie(ctx: &Contexte, cle: &str) -> Option<String> {
    ctx.salaries.get(cle).and_then(|s| s.nomme())
}

/// La fiche du spectacle de ce numero d'objet : `(cle, titre)` ; le titre est vide quand la fiche ne le dit pas, ou
/// qu'il est « A COMPLETER ».
fn spectacle_de(ctx: &Contexte, numero: &str) -> Option<(String, Option<String>)> {
    if numero.is_empty() {
        return None;
    }
    ctx.spectacles.iter().find(|(_, s)| s.numero_objet.portee().is_some_and(|n| n.trim().eq_ignore_ascii_case(numero))).map(|(k, s)| {
        let t = s.titre.portee().map(str::trim).filter(|t| !t.eq_ignore_ascii_case(A_COMPLETER)).map(str::to_string);
        (k.to_string(), t)
    })
}

/// LA PERIODE D'UN CONTRAT IMPORTE DANS `mois`, en clair, quand le bulletin ne dit pas ses jours : « 20–27 » pour un
/// contrat qui commence et finit dans le mois (le 1er quand il a commence avant), « 20/11 – 03/12 » pour un contrat a
/// cheval sur un autre mois (« 01/09/2022 – 31/12 » quand il a commence une autre annee), « … » pour une borne que
/// l'import n'a pas lue.
fn periode_en_clair(d: Option<Jour>, f: Option<Jour>, mois: Mois) -> String {
    let dans = |j: &Jour| mois.contient(j);
    let dehors = d.as_ref().is_some_and(|j| !dans(j)) || f.as_ref().is_some_and(|j| !dans(j));
    let borne = |j: Option<&Jour>, inconnue: &str| match j {
        Some(j) if dehors && j.an != mois.annee() => j.jj_mm_aaaa(),
        Some(j) if dehors => j.jj_mm(),
        Some(j) => j.jour.to_string(),
        None => inconnue.to_string(),
    };
    let (a, b) = (borne(d.as_ref(), if dehors { "…" } else { "1" }), borne(f.as_ref(), "…"));
    match (a == b, dehors) {
        (true, _) => a,
        (false, true) => format!("{} – {}", a, b),
        (false, false) => format!("{}–{}", a, b),
    }
}

/// Les jours des dates travaillees d'un bulletin, dans `mois`.
fn jours_du_mois(dates: &[DatesTravaillees], mois: Mois) -> R<Vec<Jour>> {
    let mut out = Vec::new();
    for d in dates {
        let (mut j, b) = (d.debut, d.fin);
        while j <= b {
            if mois.contient(&j) && !out.contains(&j) {
                out.push(j);
            }
            if j == b {
                break;
            }
            j = j.plus_jours(1).map_err(|m| m.reclasser(Classe::CalculImpossible))?;
        }
    }
    out.sort();
    Ok(out)
}

/// UNE CASE QU'AUCUNE PIECE NE PORTE : laquelle, son libelle, et ce qui l'aurait donnee.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Manque {
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
    pub(crate) couleur: &'static str,
    courte: &'static str,
    phrase: String,
}

/// Les etats des actes d'une carte.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EtatsDeCarte {
    pub(crate) dpae: EtatDeCarte,
    signature: EtatDeCarte,
    documents: EtatDeCarte,
}

/// L'etat de la DPAE d'une embauche tenue avant : le journal (reel accepte, par le numero du contrat), puis les certificats
/// de `retours/dpae/` (par le salarie et le jour d'embauche qu'ils attestent).
fn dpae_de(journal: &std::collections::HashMap<etats::Paire, socle::paris::InstantOuJour>, certifiees: &[envois::reprise_journal::DpaeAttestee], salarie: Option<&str>, numero: Option<&str>, debut: Option<Jour>) -> EtatDeCarte {
    if let Some(q) = salarie.and_then(|s| journal.get(&(s.to_string(), numero.map(str::to_string)))) {
        return EtatDeCarte { couleur: "vert", courte: "DPAE faite", phrase: format!("DPAE déposée le {}", q.dit()) };
    }
    let certifiee = certifiees.iter().find(|c| c.embauches.iter().any(|e| Some(e.salarie.as_str()) == salarie && Some(e.debut) == debut));
    if let Some(c) = certifiee {
        let quand: String = c.quand.iter().map(|q| format!(" ({})", q.dit())).collect();
        return EtatDeCarte { couleur: "vert", courte: "DPAE faite", phrase: format!("DPAE certifiée par l'Urssaf{}", quand) };
    }
    EtatDeCarte { couleur: "gris", courte: "avant PuppetPaie", phrase: "DPAE faite avant PuppetPaie : aucun certificat de l'Urssaf n'est relevé pour cette embauche".to_string() }
}

/// Les montants qu'une piece importee porte, chacun exact ; un montant qu'aucune ne porte n'y est pas.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct Montants {
    #[serde(skip_serializing_if = "Option::is_none")]
    brut: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    net_imposable: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    net_a_payer: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pas: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    net_verse: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cotisations_salariales: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cotisations_patronales: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cout_employeur: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    net_avant_impot: Option<Decimal>,
}

/// D'ou vient chaque montant : la DSN, le bulletin, l'AEM, l'historique.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct Sources {
    #[serde(skip_serializing_if = "Option::is_none")]
    brut: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    net_imposable: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    net_a_payer: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pas: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    net_verse: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cotisations_salariales: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cotisations_patronales: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cout_employeur: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    net_avant_impot: Option<&'static str>,
}

/// Le spectacle d'une carte : sa fiche, son numero d'objet, son titre.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SpectacleDeCarte {
    cle: Option<String>,
    numero_objet: Option<String>,
    titre: Option<String>,
}

/// UNE CARTE D'ENGAGEMENT IMPORTE, telle que la page Paie la dessine (`carteImportee` de js/saisie-mois.js).
#[derive(Debug, Clone, Serialize)]
pub struct CarteImportee {
    quoi_dit: String,
    origine_dit: Option<String>,
    spectacle_dit: Option<String>,
    net_titre: Option<&'static str>,
    salarie: Option<String>,
    nom: Option<String>,
    spectacle: SpectacleDeCarte,
    contrat: Option<String>,
    debut: Option<Jour>,
    fin: Option<Jour>,
    periode: String,
    dates: Vec<Jour>,
    cachets: Option<i64>,
    heures: Option<i64>,
    jours: Option<i64>,
    montants: Montants,
    sources: Sources,
    lignes: Vec<LigneImprimee>,
    declare_par: Option<&'static str>,
    atteste_par: Option<Attestation>,
    pub(crate) bulletin: Option<BulletinSource>,
    pub(crate) etats: EtatsDeCarte,
    pub(crate) manque: Vec<Manque>,
}

/// Une carte : une paie importee (et le contrat atteste qui va avec), ou un contrat atteste seul.
fn carte(ctx: &Contexte, mois: Mois, paie: Option<&PaieLue>, contrat: Option<&ContratLu>, dpae: EtatDeCarte) -> R<CarteImportee> {
    let p = paie.map(|x| &x.paie);
    let c = contrat.map(|x| &x.contrat);
    let plein = |t: String| Some(t.trim().to_string()).filter(|t| !t.is_empty());
    let salarie = paie.and_then(|x| plein(x.salarie.clone())).or_else(|| contrat.and_then(|x| plein(x.salarie.clone())));
    let debut = p.and_then(|p| p.debut).or_else(|| contrat.and_then(|x| x.debut));
    let fin = p.and_then(|p| p.fin).or_else(|| contrat.and_then(|x| x.fin));
    let numero = p.and_then(|p| p.numero_objet.clone()).or_else(|| c.and_then(|c| c.numero_objet.clone())).and_then(plein);
    let spectacle = numero.as_deref().and_then(|n| spectacle_de(ctx, n));
    let titre_bulletin = p.and_then(|p| p.spectacle.clone()).and_then(plein);
    let titre = match &spectacle {
        Some((_, Some(t))) => Some(t.clone()),
        _ => titre_bulletin,
    };
    let nom = salarie.as_deref().and_then(|s| nom_du_salarie(ctx, s));
    let dates: Vec<Jour> = p.map(|p| jours_du_mois(&p.dates, mois)).transpose()?.into_iter().flatten().collect();
    // Ce que la piece du mois atteste de lui (la paie d'un mois d'un permanent), sinon le contrat.
    let nombre = |de_la_paie: fn(&PaieVersee) -> Option<i64>, du_contrat: fn(&ContratAtteste) -> Option<i64>| p.and_then(de_la_paie).or_else(|| c.and_then(du_contrat));
    let (cachets, heures, jours) = (nombre(|p| p.cachets, |c| c.cachets), nombre(|p| p.heures, |c| c.heures), nombre(|p| p.jours, |c| c.jours));
    let bulletin = p.and_then(|p| p.bulletin.clone());
    let declare_par = match p.map(|p| p.declare_par) {
        Some(Declaration::Dsn) => Some("DSN"),
        Some(Declaration::Bulletin) => Some("bulletin"),
        None => None,
    };
    let par_aem = c.is_some_and(|c| c.atteste_par == Attestation::Aem);

    // Les montants, et d'ou chacun vient : le bulletin fait foi sur ceux qu'il imprime (`import::bulletins::completer`).
    let (mut montants, mut sources) = (Montants::default(), Sources::default());
    let lus: [(&str, fn(&PaieVersee) -> Option<Decimal>, for<'a> fn(&'a mut Montants, &'a mut Sources) -> (&'a mut Option<Decimal>, &'a mut Option<&'static str>)); 9] = [
        ("brut", |p| p.brut, |m, s| (&mut m.brut, &mut s.brut)),
        ("net_imposable", |p| p.net_imposable, |m, s| (&mut m.net_imposable, &mut s.net_imposable)),
        ("net_a_payer", |p| p.net_a_payer, |m, s| (&mut m.net_a_payer, &mut s.net_a_payer)),
        ("pas", |p| p.pas, |m, s| (&mut m.pas, &mut s.pas)),
        ("net_verse", |p| p.net_verse, |m, s| (&mut m.net_verse, &mut s.net_verse)),
        ("cotisations_salariales", |p| p.cotisations_salariales, |m, s| (&mut m.cotisations_salariales, &mut s.cotisations_salariales)),
        ("cotisations_patronales", |p| p.cotisations_patronales, |m, s| (&mut m.cotisations_patronales, &mut s.cotisations_patronales)),
        ("cout_employeur", |p| p.cout_employeur, |m, s| (&mut m.cout_employeur, &mut s.cout_employeur)),
        ("net_avant_impot", |p| p.net_avant_impot, |m, s| (&mut m.net_avant_impot, &mut s.net_avant_impot)),
    ];
    for (k, lu, place) in lus {
        // Le montant que la paie porte ; sans paie, le brut que l'AEM atteste.
        let (v, source) = match (p.and_then(lu), k == "brut" && paie.is_none()) {
            (Some(d), _) => (Some(d), if bulletin.is_some() && k != "net_verse" { Some("bulletin") } else { declare_par }),
            (None, true) => (c.and_then(|c| c.brut), Some("AEM")),
            (None, false) => (None, None),
        };
        if let Some(v) = v {
            let (m, s) = place(&mut montants, &mut sources);
            *m = Some(v);
            *s = Some(match source {
                Some(s) => s,
                None => "historique",
            });
        }
    }

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
            (false, Some("DSN"), _) => "la DSN ne déclare que le début et la fin du contrat ; le bulletin de cette paie les donnerait",
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
        ("brut", "brut", montants.brut.is_some(), "aucune pièce importée ne le porte"),
        ("net_imposable", "net imposable", montants.net_imposable.is_some(), "aucune pièce importée ne le porte"),
        (
            "net_a_payer",
            "net à payer",
            montants.net_a_payer.is_some(),
            if declare_par == Some("DSN") {
                "la DSN ne le déclare pas (son 50.004 est le net versé) ; le bulletin de cette paie le donnerait"
            } else {
                "aucune pièce importée ne le porte ; le bulletin de cette paie le donnerait"
            },
        ),
        ("pas", "prélèvement à la source", montants.pas.is_some(), "aucune pièce importée ne le porte"),
    ] {
        if !porte {
            manques.push(manque(k, libelle, pourquoi.into()));
        }
    }

    let documents = match &bulletin {
        Some(b) => {
            let n: String = b.numero.iter().filter(|n| !n.is_empty()).map(|n| format!(" n° {}", n)).collect();
            EtatDeCarte { couleur: "vert", courte: "bulletin importé", phrase: format!("bulletin de l'ancien logiciel{}, copié dans historique/bulletins/", n) }
        }
        None => EtatDeCarte { couleur: "gris", courte: "avant PuppetPaie", phrase: "documents remis avant PuppetPaie : aucun bulletin importé pour cette paie".to_string() },
    };
    // CE QUE LA CARTE FERMEE DIT : le spectacle, les cachets et les heures ; au survol, les pieces lues.
    let mut quoi = Vec::new();
    if let Some(t) = titre.clone().or_else(|| numero.clone()) {
        quoi.push(t);
    }
    if let Some(n) = cachets.filter(|n| *n != 0) {
        let dit = socle::texte::virgule(Decimal::from(n));
        quoi.push(format!("{} {}", dit, if n > 1 { "cachets" } else { "cachet" }));
    }
    if let Some(n) = heures.filter(|n| *n != 0) {
        quoi.push(format!("{} h", socle::texte::virgule(Decimal::from(n))));
    }
    let quoi_dit = if quoi.is_empty() { "spectacle non lu".to_string() } else { quoi.join(" · ") };
    let mut lu = Vec::new();
    if declare_par == Some("DSN") {
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
        (Some(t), Some(n)) if n != t => Some(format!("{} — n° {}", t, n)),
        (Some(t), _) => Some(t.clone()),
        (None, Some(n)) => Some(format!("n° {}", n)),
        (None, None) => None,
    };
    let net_titre = montants.net_a_payer.is_some().then_some("net à payer");
    let periode = periode_en_clair(debut, fin, mois);
    Ok(CarteImportee {
        quoi_dit,
        origine_dit,
        spectacle_dit,
        net_titre,
        nom: nom.or_else(|| salarie.clone()),
        salarie,
        spectacle: SpectacleDeCarte { cle: spectacle.as_ref().map(|s| s.0.clone()), numero_objet: numero, titre },
        contrat: p.map(|p| p.contrat.clone()),
        debut,
        fin,
        periode,
        dates,
        cachets,
        heures,
        jours,
        montants,
        sources,
        lignes: p.map(|p| p.lignes.clone()).into_iter().flatten().collect(),
        declare_par,
        atteste_par: c.map(|c| c.atteste_par),
        bulletin,
        etats: EtatsDeCarte {
            dpae,
            signature: EtatDeCarte { couleur: "gris", courte: "avant PuppetPaie", phrase: "contrat fait avant PuppetPaie : sa signature n'est pas suivie ici".to_string() },
            documents,
        },
        manque: manques,
    })
}

/// LES ETAPES D'UN MOIS TENU AVANT (`bilan_du_mois`, le panneau du mois) : tout s'est fait ailleurs,
/// aucune etape n'a de bouton -- ni « Fermer le mois », ni « Deposer la DSN » (regle 4). La DSN dit ce qu'on en
/// sait (`compagnie::etat_dsn`) ; sans trace, elle reste en alerte, sans geste.
pub fn etapes(vue: &VueImportee) -> EtapesImportees {
    let cartes = &vue.paies;
    let n = cartes.len();
    let pl = |n: usize| if n > 1 { "s" } else { "" };
    let dpae_faites = cartes.iter().filter(|c| c.etats.dpae.couleur == "vert").count();
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
    let mut cartes = Vec::new();
    for i in importes {
        let contrat = i.contrat_lu()?;
        let paie = i.paies_lues()?.into_iter().find(|p| p.mois == mois);
        let brut = i.objet().carte();
        let salarie = brut.salarie.as_ref().map(ToString::to_string);
        let dpae = dpae_de(&embauches, &certifiees, salarie.as_deref(), brut.numero.as_deref(), brut.debut);
        cartes.push(carte(ctx, mois, paie.as_ref(), contrat.as_ref(), dpae)?);
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
