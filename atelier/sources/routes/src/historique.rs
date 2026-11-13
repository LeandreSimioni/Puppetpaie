//! Les routes de l'Historique, hors du detail d'un mois (`historique-mois`) : les annees (leurs DSN, leur cloture, leur
//! export comptable), les corrections par mois d'origine, l'archive d'un mois dans le Drive, et la reprise du journal
//! depuis ce que les retours attestent.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, Place, Refus, MANQUE, MOIS, OPERATION};
use crate::catalogue::Route;
use crate::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use crate::ouverture::mois_connus;
use crate::SansCharge;
use droit::contexte::Contexte;
use envois::ligne::Mode;
use envois::reprise_journal::DepotVu;
use socle::echec::{Classe, R};

const ANNEE: Champ = Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année, AAAA", avance: false };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Historique);

/// Une lecture sans charge.
const fn lecture(route: Route, geste: &'static str, rend: &'static str) -> Fiche {
    Fiche { route, geste, charge: &[], exemple: "{}", rend, refus: &[OPERATION], acte: Acte::Lit, phrase: 0, ecran: ECRAN }
}

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    lecture(Route::Exercices, "Lister les années : leurs DSN, leur clôture, leur export comptable.", "annees (chacune : DSN, clôture, export)"),
    lecture(Route::Regularisations, "Lister les corrections à porter et portées, par mois d'origine.", "les corrections, mois par mois"),
    Fiche {
        route: Route::Cloture,
        geste: "Clore une année passée : archiver ses engagements, documents et déclarations.",
        charge: &[
            ANNEE,
            Champ { nom: "forcer", libelle: Affiche::new("Clore malgré tout"), forme: Forme::OuiNon, sens: "vrai : clore malgré un mois sans DSN réelle acceptée", avance: true },
            Champ { nom: "rouvrir", libelle: Affiche::new("Rouvrir l'année"), forme: Forme::OuiNon, sens: "vrai : rouvrir l'année close (son archive reste)", avance: true },
        ],
        exemple: r#"{"annee":"2025"}"#,
        rend: "annee, archive, fichiers, empreinte, manques (ce qui n'a pu s'archiver) ; rouverte : etait_clos_depuis",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ExportCompta,
        geste: "Écrire l'export comptable d'une année, du mois du au mois au.",
        charge: &[
            ANNEE,
            Champ { nom: "du", libelle: Affiche::new("Du mois"), forme: Forme::Nombre, sens: "le premier mois, 1 à 12", avance: true },
            Champ { nom: "au", libelle: Affiche::new("Au mois"), forme: Forme::Nombre, sens: "le dernier mois, 1 à 12", avance: true },
            Champ { nom: "apercu", libelle: Affiche::new("Aperçu seulement"), forme: Forme::OuiNon, sens: "vrai : dire ce qui s'écrirait, sans écrire", avance: true },
        ],
        exemple: r#"{"annee":"2026","apercu":true}"#,
        rend: "les fichiers de l'export, ou ce qu'ils porteraient",
        refus: &[MANQUE, OPERATION],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Archiver,
        geste: "Déposer les documents d'un mois dans le Drive de la compagnie.",
        charge: &[MOIS],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "mois, dossier (dans le Drive), compte, fichiers (nom, octets, remplace), phrase ; une fictive joue le dépôt, rien ne part",
        refus: &[
            Refus { classe: Classe::SortieAbsente, corriger: "un mois dont les documents sont écrits (une paie validée)" },
            Refus { classe: Classe::Google, corriger: "lier le Drive (route google-autoriser), ou ce que la phrase de Google dit" },
            Refus { classe: Classe::CopieDeDeveloppement, corriger: "rien : une copie de développement n'écrit jamais dans le Drive" },
            OPERATION,
        ],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Historique),
    },
    Fiche {
        route: Route::JournalReprise,
        geste: "Reprendre le journal des envois d'après les retours relevés.",
        charge: &[Champ { nom: "ecrire", libelle: Affiche::new("Écrire ce qui se reprend"), forme: Forme::OuiNon, sens: "vrai : écrire ce qui se reprend ; sinon, le dire seulement", avance: false }],
        exemple: r#"{}"#,
        rend: "ce qui se reprend, ligne par ligne, et ses pastilles",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Historique),
    },
];

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::Exercices => {
            let SansCharge {} = charge.lire()?;
            en_json(exercices_route(ctx)?)
        }
        Route::Regularisations => {
            let SansCharge {} = charge.lire()?;
            en_json(regularisations(ctx)?)
        }
        Route::Cloture => en_json(cloture(ctx, charge.lire()?)?),
        Route::ExportCompta => en_json(crate::gestes::exercice_ecrit::export_compta_route(ctx, charge.lire()?)?),
        Route::Archiver => en_json(archiver(ctx, charge.lire()?)?),
        Route::JournalReprise => en_json(reprise(ctx, charge.lire()?)?),
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

impl Dite for ExercicesDits {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for Regularisations {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for ClotureDite {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for Archive {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for RepriseDite {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// La charge de `journal-reprise` : `ecrire` vrai ecrit ce qui se reprend ; absent ou faux, la route le dit seulement.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeReprise {
    ecrire: Option<bool>,
}

/// Ce que rend `journal-reprise` : sa phrase, ce que la reprise a trouve, ajoute (ou ajouterait, chaque depot dit pour
/// l'ecran), deja connu, sans mois, illisible, si elle a ecrit, et ce que l'Historique en dit (`ecran`).
#[derive(Debug, Serialize)]
struct RepriseDite {
    phrase: String,
    trouves: Vec<DepotVu>,
    ajoutes: Vec<AjoutDit>,
    deja: Vec<DepotVu>,
    sans_mois: Vec<DepotVu>,
    illisibles: Vec<String>,
    ecrit: bool,
    rattaches: i64,
    ecran: EcranDeLaReprise,
}

/// Un depot que la reprise remet au journal, tel que l'ecran le dit : quand et pour quel mois (`dit`), qui l'a depose et
/// son flux (`origine_dit`), sa pastille.
#[derive(Debug, Serialize)]
struct AjoutDit {
    #[serde(flatten)]
    depot: DepotVu,
    dit: String,
    origine_dit: String,
    pastille: socle::ecran::Pastille,
}

/// Ce que l'Historique dit de la reprise : le resume replie, son titre, l'explication, les depots reels, le nom du
/// logiciel lu, le bouton.
#[derive(Debug, Serialize)]
struct EcranDeLaReprise {
    resume: String,
    titre: String,
    explication: &'static str,
    reels: Option<String>,
    logiciel: &'static str,
    bouton: String,
}

/// `journal-reprise` : la reprise du journal d'apres les retours (`envois::reprise_journal::reprise`), ses incidents
/// notes, et ce que l'ecran en dit.
fn reprise(ctx: &Contexte, c: ChargeReprise) -> R<RepriseDite> {
    let r = compagnie::incidents::noter_ceux_des_retours(
        &ctx.racine,
        envois::reprise_journal::reprise(ctx, c.ecrire == Some(true), compagnie::INTERFACE),
    )?;
    let n = r.ajoutes.len();
    let reels = r.ajoutes.iter().filter(|d| d.mode == Mode::Reel && d.accepte).count();
    let s = if n > 1 { "s" } else { "" };
    let dont = if reels > 0 { format!(" — dont {} réel{}", reels, if reels > 1 { "s" } else { "" }) } else { String::new() };
    let ecran = EcranDeLaReprise {
        resume: format!("{} dépôt{} retrouvé{} dans les retours, absent{} du journal{}", n, s, s, s, dont),
        titre: format!("{} dépôt(s) attestés par les retours, absents du journal", n),
        explication: "Le journal des dépôts est né le 11/09/2026 ; ce qui est parti avant ne s'y est pas écrit. Les accusés d'enregistrement reçus des organismes, eux, s'en souviennent. Les reprendre ne dépose rien et n'envoie rien : ça remet au journal ce qui a déjà eu lieu.",
        reels: (reels > 0).then(|| format!("{} sont des dépôts réels acceptés.", reels)),
        logiciel: "Le nom du logiciel n'est pas une annotation : il est lu dans le certificat de conformité. C'est lui qui distingue le dépôt d'un autre logiciel d'un dépôt PuppetPaie.",
        bouton: format!("Reprendre ces {} dépôt(s)", n),
    };
    let phrase = format!("Dépôts trouvés dans les retours : {} ; ajoutés au journal : {}{}.", r.trouves.len(), n, if r.ecrit { "" } else { " (rien n'est écrit)" });
    let ajoutes = r.ajoutes.into_iter().map(ajout_dit).collect();
    Ok(RepriseDite { phrase, trouves: r.trouves, ajoutes, deja: r.deja, sans_mois: r.sans_mois, illisibles: r.illisibles, ecrit: r.ecrit, rattaches: r.rattaches, ecran })
}

/// Un depot que la reprise remet, dit : quand il a ete recu et son mois, qui l'a depose ; sa pastille : un depot reel
/// accepte, fait ; refuse, refuse ; un depot de test, une information.
fn ajout_dit(d: DepotVu) -> AjoutDit {
    let mois = match &d.mois {
        Some(m) => m.libelle(socle::mois::Graphie::Accentuee),
        None => "mois inconnu".to_string(),
    };
    let dit = match &d.quand {
        Some(q) => format!("{} — {}", q.dit(), mois),
        None => mois,
    };
    let origine_dit = match &d.logiciel {
        Some(l) => format!("déposé par {} · flux {}", l, d.flux),
        None => format!("logiciel inconnu · flux {}", d.flux),
    };
    let reel = d.mode == Mode::Reel;
    let mots = format!("{}{}", if reel { "dépôt réel" } else { "test" }, if d.accepte { "" } else { " — refusé" });
    let pastille = socle::ecran::Pastille::de(
        if !reel {
            socle::ecran::EtatDEcran::Information
        } else if d.accepte {
            socle::ecran::EtatDEcran::Fait
        } else {
            socle::ecran::EtatDEcran::Refuse
        },
        &mots,
    );
    AjoutDit { depot: d, dit, origine_dit, pastille }
}

/// Ce que rend `regularisations` : sa phrase, et par mois d'origine les corrections que la page montre.
#[derive(Debug, Serialize)]
struct Regularisations {
    phrase: String,
    corrections: Vec<CorrectionsDuMois>,
}

/// Les corrections d'un mois d'origine.
#[derive(Debug, Serialize)]
struct CorrectionsDuMois {
    mois: socle::mois::Mois,
    corrections: Vec<CorrectionDite>,
}

/// Une correction, et ou elle en est : portee par une DSN partie, faite ; a porter par la prochaine, a faire.
#[derive(Debug, Serialize)]
struct CorrectionDite {
    #[serde(flatten)]
    correction: crate::regularisation::Correction,
    etat: &'static str,
    pastille: socle::ecran::Pastille,
}

/// La route `regularisations` : par mois d'origine, les corrections que la page montre
/// (`crate::regularisation::corrections_du_mois`) : celles deja portees par la DSN d'un mois suivant, puis celles a porter.
fn regularisations(ctx: &Contexte) -> R<Regularisations> {
    let mut out = Vec::new();
    for mois in engagement::stock::mois_saisis(&ctx.racine)? {
        let (a_porter, portees) = crate::regularisation::corrections_du_mois(ctx, mois)?;
        let dite = |correction, etat: &'static str, fait: bool| CorrectionDite { correction, etat, pastille: socle::ecran::Pastille::de(if fait { socle::ecran::EtatDEcran::Fait } else { socle::ecran::EtatDEcran::AFaire }, etat) };
        let mut corrections: Vec<CorrectionDite> = portees.into_iter().map(|c| dite(c, "portée", true)).collect();
        corrections.extend(a_porter.into_iter().map(|c| dite(c, "à porter", false)));
        if !corrections.is_empty() {
            out.push(CorrectionsDuMois { mois, corrections });
        }
    }
    Ok(Regularisations { phrase: format!("Mois qui portent des corrections : {}.", out.len()), corrections: out })
}

/// Ce que rend `cloture` : sa phrase, et l'exercice clos ou rouvert.
#[derive(Debug, serde::Serialize)]
struct ClotureDite {
    phrase: String,
    #[serde(flatten)]
    cloture: crate::gestes::exercice_ecrit::Cloture,
}

/// `cloture` : clore une annee passee, ou la rouvrir (`exercice_ecrit::cloture_route`), et le dire.
fn cloture(ctx: &Contexte, c: crate::gestes::exercice_ecrit::ChargeCloture) -> R<ClotureDite> {
    use crate::gestes::exercice_ecrit::Cloture;
    let cloture = crate::gestes::exercice_ecrit::cloture_route(ctx, c)?;
    let phrase = match &cloture {
        Cloture::Rouverte(r) => format!("Exercice {} rouvert ; son archive reste.", r.annee),
        Cloture::Close(e) => {
            let reste = if e.manques.is_empty() { String::new() } else { format!(" Pas archivé : {}.", e.manques.iter().map(|m| m.trim_end_matches('.')).collect::<Vec<_>>().join(" ; ")) };
            format!("Exercice {} clos : {} fichiers archivés.{}", e.annee, e.fichiers, reste)
        }
    };
    Ok(ClotureDite { phrase, cloture })
}

/// Ce que rend `exercices` : sa phrase, une ligne par annee (`phrases`), chaque annee, et le dossier des archives.
#[derive(Debug, serde::Serialize)]
struct ExercicesDits {
    phrase: String,
    phrases: Vec<String>,
    exercices: Vec<ExerciceDit>,
    dossier: String,
}

/// Une annee connue du dossier : ses mois, ses DSN acceptees, sa cloture, son export compta, sa pastille, et sa ligne.
#[derive(Debug, serde::Serialize)]
struct ExerciceDit {
    infos_dit: String,
    annee: socle::mois::Annee,
    mois: Vec<socle::mois::Mois>,
    dsn_acceptees: Vec<socle::mois::Mois>,
    clos: Option<crate::gestes::exercice_ecrit::ExerciceNote>,
    pastille: socle::ecran::Pastille,
    export: Option<ExportDeLAnnee>,
}

/// L'export compta d'une annee : son fichier, et quand il a ete ecrit.
#[derive(Debug, serde::Serialize)]
struct ExportDeLAnnee {
    fichier: String,
    #[serde(with = "socle::paris::ecrit::minute::option")]
    ecrit: Option<socle::paris::Moment>,
}

/// La route `exercices` : chaque annee connue du dossier -- ses mois, sa cloture, son export -- et le dossier des archives.
fn exercices_route(ctx: &Contexte) -> R<ExercicesDits> {
    let exercices = etat_exercices(ctx)?;
    Ok(ExercicesDits {
        phrase: format!("Exercices : {}.", exercices.len()),
        phrases: exercices.iter().map(|e| format!("{} : {}", e.annee, e.pastille.mots)).collect(),
        exercices,
        dossier: ctx.racine.join("sorties").join("exercices").display().to_string(),
    })
}

/// `etat_exercices` : chaque annee connue du dossier, de la plus recente a la plus ancienne.
fn etat_exercices(ctx: &Contexte) -> R<Vec<ExerciceDit>> {
    let lus = crate::gestes::exercice_ecrit::lire_exercices(&ctx.racine)?;
    let clos = lus.clos();
    let mois_tous: Vec<socle::mois::Mois> = mois_connus(&ctx.racine)?.into_iter().map(|(m, _)| m).collect();
    let mut annees: BTreeSet<socle::mois::Annee> = mois_tous.iter().map(|m| m.annee_civile()).collect();
    annees.extend(clos.keys().copied());
    let mut out = Vec::new();
    for &a in annees.iter().rev() {
        let mut mois: Vec<socle::mois::Mois> = mois_tous.iter().filter(|x| x.annee_civile() == a).copied().collect();
        mois.sort();
        let fichier = format!("Compta_{}.csv", a);
        let chemin = ctx.racine.join("sorties").join("exercices").join(&fichier);
        let mut acceptees = Vec::new();
        for &x in &mois {
            if compagnie::etat_dsn::du_mois(ctx, x, None)?.partie() {
                acceptees.push(x);
            }
        }
        let export = match chemin.exists() {
            true => Some(ExportDeLAnnee { fichier, ecrit: compagnie::pieces_ecrites::ecrit_le(&ctx.racine, &chemin)? }),
            false => None,
        };
        // Ce que la ligne de l'annee dit : ses mois, ses DSN acceptees, sa cloture et son export compta.
        let note = clos.get(&a);
        let clos_dit: String = note.iter().map(|c| format!(" · clos le {}", c.le.dit())).collect();
        let export_dit = match &export {
            Some(ExportDeLAnnee { ecrit: Some(e), .. }) => format!(" · export compta du {}", e.dit()),
            Some(_) => " · export compta".to_string(),
            None => " · pas d'export compta".to_string(),
        };
        let infos_dit = format!("{} mois · {}{}{}", mois.len(), socle::ecran::compte(acceptees.len(), "DSN acceptée", "DSN acceptées"), clos_dit, export_dit);
        out.push(ExerciceDit {
            infos_dit,
            annee: a,
            mois,
            dsn_acceptees: acceptees,
            clos: note.cloned(),
            // Sa pastille : close, faite ; ouverte, une information (une annee se clot quand la compagnie le veut).
            pastille: if note.is_some() { socle::ecran::Pastille::de(socle::ecran::EtatDEcran::Fait, "clos") } else { socle::ecran::Pastille::de(socle::ecran::EtatDEcran::Information, "ouvert") },
            export,
        });
    }
    Ok(out)
}

/// DEPOSER LES DOCUMENTS D'UN MOIS DANS LE DRIVE (`archivage::archiver`) : `PuppetPaie/<mois>` du compte lie. Une
/// compagnie fictive joue le depot (`archivage::DriveJoue`) : la meme liste, rien ne part.
fn archiver(ctx: &Contexte, charge: ChargeArchiver) -> R<Archive> {
    use crate::gestes::archivage;
    let joue = compagnie::bridage::branchement(&ctx.racine)?.joue();
    let mut drive_joue = archivage::DriveJoue;
    let client: Option<&mut dyn envois::drive::Drive> = if joue { Some(&mut drive_joue) } else { None };
    let r = archivage::archiver(&ctx.racine, charge.mois, compagnie::sauvegarde::RACINE_DRIVE, client).map_err(|e| e.echec)?;
    let dossier = format!("{}/{}", r.racine_drive, r.mois);
    // Le dossier du Drive tel que l'écran le dit : ses dossiers séparés comme les places de l'écran.
    let ou = format!("{} › {}", r.racine_drive, r.mois);
    let phrase = if joue {
        format!("Compagnie fictive : rien ne part au Drive. {} fichier(s) iraient dans le dossier {}.", r.fichiers.len(), ou)
    } else {
        format!("{} fichier(s) déposé(s) dans le Drive, dossier {}.", r.fichiers.len(), ou)
    };
    let fichiers = r.fichiers.into_iter().map(|f| FichierDepose { nom: f.nom.into(), octets: f.octets, remplace: f.remplace }).collect();
    Ok(Archive { phrase, mois: r.mois, dossier, compte: r.compte, fichiers })
}

/// La charge d'`archiver` : le mois.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeArchiver {
    mois: socle::mois::Mois,
}

/// Ce que rend `archiver` : sa phrase, le mois, le dossier du Drive, le compte lie, et chaque fichier depose.
#[derive(Debug, Serialize)]
struct Archive {
    phrase: String,
    mois: socle::mois::Mois,
    dossier: String,
    compte: Option<String>,
    fichiers: Vec<FichierDepose>,
}

/// Un fichier depose dans le Drive : son nom, ses octets, s'il en remplace un.
#[derive(Debug, Serialize)]
struct FichierDepose {
    nom: String,
    octets: u64,
    remplace: bool,
}

