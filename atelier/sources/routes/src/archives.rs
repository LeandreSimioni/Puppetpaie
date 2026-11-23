//! Les routes des Exports : ce qui reste d'une annee de paies (le dossier d'un salarie, les exports et leur logiciel
//! comptable, les droits des intermittents), et ce qui part d'elles (le dossier par courriel, la regularisation de la
//! CASC-SVP).

use droit::fiche::Lue as _;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use compagnie::preuve::{verifier_preuve, Preuve};
use crate::importation::casc::{marquer_regularise, RegularisationNotee};
use crate::dossier_salarie::{construire, DemandeDeDossier, DossierEnvoye, DossierRendu};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use compagnie::INTERFACE;
use crate::SansCharge;
use socle::mois::{Annee, Mois};
use crate::catalogue::{Acte, Affiche, Forme, Fiche, Refus, FORME, MANQUE, OPERATION};
use crate::declaration::{charge, cle, dit, legendes, Legende, ParSaForme, ValeurDeLaCharge};
use crate::imprevus::GesteDeRoute;
use crate::catalogue::Route;
use crate::documents::{client_courriel, expediteur_de, refus, Ouvrir};

use documents::etats_compta::ChoixCompta;
use documents::plan_comptable::{controler_plan, tables, EtatDExport, FormatDExport, LogicielComptable, RegroupementDEtat};
use droit::contexte::Contexte;
use envois::envoi::{self, Message};
use envois::ligne_d_envoi::{en_liste, noter, noter_echec_d_envoi, par_courriel};
use envois::journal;
use socle::date::Jour;
use socle::echec::{erreur, Classe, Echec, R};
use socle::fichiers::NomDeFichier;
use socle::html::courriel_en_html;
use crate::catalogue::{Ecran, Place};

const ANNEE: Legende = dit("Année", "l'année, AAAA");
pub(crate) const TOUTES_LES_ANNEES: Legende = dit("Toutes les années", "vrai : toutes les années, à la place du mois ou de l'année").avancee();
const LOGICIEL: Legende = dit("Logiciel", "le logiciel du comptable ; absent : aucun");
const ANALYTIQUE: Legende = dit("Par spectacle", "vrai : par spectacle").avancee();

/// Une lecture sans charge de l'Historique ou des Exports.
const fn lecture(route: Route, geste: &'static str, rend: &'static str) -> Fiche {
    Fiche { route, geste, charge: &SansCharge::CHARGE, exemple: "{}", rend, refus: &[OPERATION], acte: Acte::Lit, phrase: 0, ecran: ECRAN, pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee }
}

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Exports);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    lecture(Route::DossierFormes, "Dire ce que peut porter le dossier d'un salarié : catégories, périodes.", "gestes (l'aperçu, l'écrire, et l'envoyer quand une boîte est posée, avec la demande du mot de passe : leurs cases, montrant la dernière année et offrant les salariés), ecran (ce que les cases veulent dire, dit ; le bouton du dossier, dossier_dit), le dossier où il s'écrit"),
    lecture(Route::ExportsPeriodes, "Dire ce qui s'exporte : mois, années, états, formats, logiciel comptable.", "mois, annees, familles (chacune ses états : leurs questions, les formats et regroupements offerts et proposés, ce que dit le regroupement, la période proposée), logiciel retenu, ecran (les mots des cases et des boutons)"),
    Fiche {
        route: Route::RetenirLogiciel,
        geste: "Retenir le logiciel du comptable, dont les écritures prennent le format.",
        charge: &ChargeRetenirLogiciel::CHARGE,
        exemple: r#"{}"#,
        rend: "le logiciel retenu",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::CascRegularise,
        geste: "Noter régularisée, hors DSN, la CASC-SVP d'une année de mois importés.",
        charge: &ChargeCascRegularise::CHARGE,
        exemple: r#"{"annee":2025,"vire_le":"2026-11-02","montant":"1.12"}"#,
        rend: "la régularisation notée : année, mois, montant, calculé, vire_le",
        refus: &[OPERATION, FORME, MANQUE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Exports),
    },
    Fiche {
        route: Route::Exporter,
        geste: "Exporter un état (livre de paie, charges…) en tableau ou en fichier.",
        charge: &ChargeExporter::CHARGE,
        exemple: r#"{"etat":"livre-de-paie","mois":"2026-11","regroupement":"salarie","format":"csv","apercu":true}"#,
        rend: "le tableau, ou le fichier écrit",
        refus: &[OPERATION, MANQUE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::DossierEnvoyer,
        geste: "Refaire le dossier de chaque salarié choisi et le lui envoyer par courriel.",
        charge: &ChargeDossierEnvoyer::CHARGE,
        exemple: r#"{"salaries":["FULLER"],"mois":"2026-11"}"#,
        rend: "le dossier, et envoi (partis)",
        refus: &[
            Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe du logiciel, tapé par un humain au moment de l'envoi ; rien n'est écrit" },
            OPERATION,
        ],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Engage,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Droits,
        geste: "Dire les heures et cachets cumulés de chaque intermittent (annexes 8 et 10).",
        charge: &ChargeDroits::CHARGE,
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "salaries (chacun : nom, fenetre, lignes [quoi, valeur], phrase, notes, colonnes du tableau), phrase (ce que vaut ce compte)",
        refus: &[OPERATION, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::CascEnvoyer,
        geste: "Envoyer à Audiens et au CASC-SVP la régularisation des mois importés.",
        charge: &ChargeCasc::CHARGE,
        exemple: r#"{"vire_le":{"2025":"2026-11-02"}}"#,
        rend: "partis (les destinataires), expediteur, periode, total, annees (les régularisations notées), eml",
        refus: &[
            Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe du logiciel, tapé par un humain ; rien n'est parti" },
            Refus { classe: Classe::BoiteAbsente, corriger: "poser la boîte courriel (Configuration, Mots de passe)" },
            Refus { classe: Classe::Refuse, corriger: "la date du virement fait, ou l'année que la phrase nomme ; rien n'est parti" },
        ],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Engage,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Exports),
    },
];

/// Une route de ce module.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::DossierFormes => {
            let SansCharge {} = charge.lire()?;
            en_json(dossier_formes(ctx)?)
        }
        Route::ExportsPeriodes => {
            let SansCharge {} = charge.lire()?;
            en_json(exports_periodes(ctx)?)
        }
        Route::RetenirLogiciel => en_json(retenir_logiciel(ctx, charge.lire()?)?),
        Route::CascRegularise => en_json(casc_regularise(ctx, charge.lire()?)?),
        Route::Exporter => en_json(exporter(ctx, &charge.lire()?)?),
        Route::DossierEnvoyer => en_json(dossier_envoyer(ctx, charge.lire()?, charge.mot_de_passe(), &mut || client_courriel(ctx))?),
        Route::CascEnvoyer => en_json(casc_envoyer(ctx, charge.lire()?, charge.mot_de_passe(), &mut || client_courriel(ctx))?),
        Route::Droits => en_json(droits(ctx, charge.lire()?)?),
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

/// La phrase de chaque reponse de ce module, en tete de ce qu'elle rend.
macro_rules! dite {
    ($($t:ty),*) => {
        $(impl Dite for $t {
            fn phrase(&self) -> &Phrase {
                &self.phrase
            }
        })*
    };
}

dite!(FormesDuDossier, ExportsPossibles, LogicielRetenu, CascNotee, EtatExporte, CascEnvoyee, Droits);

/// Les noms des PDF d'un dossier (sensible à la casse, fichiers cachés compris),
/// en ordre de points de code.
fn pdf_de(d: &Path) -> R<Vec<String>> {
    let mut noms = Vec::new();
    // Ce qui n'est pas un dossier, ou un dossier interdit, n'a aucun PDF :
    // rien ne se refuse.
    let lecture = match std::fs::read_dir(d) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(20) => return Ok(noms),
        Err(e) => return Err(erreur_io(&e, d)),
    };
    for e in lecture {
        let e = e.map_err(|e| erreur_io(&e, d))?;
        let n = e.file_name().to_string_lossy().into_owned();
        if n.ends_with(".pdf") {
            noms.push(n);
        }
    }
    noms.sort();
    Ok(noms)
}

/// Une piece du mois dans `sorties/<mois>/envoi/` : son nom, et la cle du salarie qu'elle nomme, si elle en nomme un.
pub(crate) struct PieceDeLEnvoi {
    pub(crate) nom: String,
    pub(crate) salarie: Option<droit::contexte::IdSalarie>,
}

/// Les pieces du mois a ouvrir ou a remettre a la main : les PDF de `sorties/<mois>/envoi/`, chacune seule (jamais le PDF
/// a imprimer, qui reunit celles de plusieurs salaries : ce fichier n'est la piece de personne).
pub(crate) fn pieces_de_l_envoi(ctx: &Contexte, mois: Mois) -> R<Vec<PieceDeLEnvoi>> {
    let envoi = documents::DossierDuMois::Envoi.dans(&ctx.racine, mois);
    if !envoi.exists() {
        return Ok(Vec::new());
    }
    pdf_de(&envoi)?
        .into_iter()
        .filter(|nom| !matches!(documents::SorteDePieceProduite::du_nom(nom), Some((documents::SorteDePieceProduite::Impression, _))))
        .map(|nom| {
            // `<Piece>_<cle>_<etiquette>.pdf` : la cle du salarie est le deuxieme morceau.
            let morceaux: Vec<&str> = nom.split('_').collect();
            let salarie = (morceaux.len() > 2).then(|| droit::contexte::IdSalarie::lire(morceaux[1]).map_err(|_| piece_etrangere(&format!("envoi/{}", nom)))).transpose()?;
            Ok(PieceDeLEnvoi { nom, salarie })
        })
        .collect()
}

/// Le refus d'un fichier des sorties d'un mois (`chemin`, depuis `sorties/<mois>/`) dont le nom ne dit pas de salarie
/// (`<Piece>__<etiquette>.pdf`) : le moteur ne l'a pas ecrit.
pub(crate) fn piece_etrangere(chemin: &str) -> socle::echec::Echec {
    socle::refus!("`{}` n'est pas un document du moteur : son nom ne dit pas de quel salarié il est. Le retirer de ce dossier.", chemin)
}

// -------------------------------------------------------------- dossier-formes


/// Les mois qu'il y a matiere a exporter (`documents::etats_paie::mois_disponibles`), dans l'ordre.
fn mois_disponibles(ctx: &Contexte) -> R<Vec<Mois>> {
    Ok(documents::etats_paie::mois_disponibles(ctx)?.into_keys().collect())
}

/// Les années des mois, de la plus récente à la plus ancienne.
fn annees_de(dispo: &[Mois]) -> Vec<Annee> {
    let mut annees: Vec<Annee> = dispo.iter().map(|m| m.annee()).collect::<HashSet<_>>().into_iter().collect();
    annees.sort();
    annees.reverse();
    annees
}

/// Un geste sur le dossier d'un salarie : l'apercu, l'ecrire (`dossier-salarie`), l'envoyer (`dossier-envoyer`, une
/// boite posee : il exige le mot de passe du logiciel). Leurs cases se saisissent ensemble, une fois chacune ; chaque
/// geste n'en prend que les siennes.
type GesteDuDossier = GesteDeRoute<DemandeDeDossier, ValeursDuDossier, ChoixDuDossier>;

/// Ce que les cases du dossier montrent d'abord : la derniere annee qu'il y a matiere a exporter.
#[derive(Debug, Clone, Default, Serialize)]
struct ValeursDuDossier {
    #[serde(skip_serializing_if = "Option::is_none")]
    annee: Option<Annee>,
}

/// Ce qu'offrent les cases du dossier : les salaries, ceux en activite d'abord, puis les archives (ainsi dits).
#[derive(Debug, Clone, Default, Serialize)]
struct ChoixDuDossier {
    salaries: Vec<crate::ouverture::Choix>,
}

/// Ce que l'ecran dit du dossier : ce que ses cases veulent dire (`dit`), et le bouton qui ouvre le dossier ou il
/// s'ecrit (`dossier_dit`).
#[derive(Debug, Serialize)]
struct EcranDuDossier {
    dit: Vec<&'static str>,
    dossier_dit: &'static str,
}

/// Ce que rend `dossier-formes` : sa phrase, les gestes du dossier (leurs cases, ce qu'elles montrent et offrent), ce que
/// l'ecran en dit, et le dossier ou le dossier s'ecrit.
#[derive(Debug, Serialize)]
struct FormesDuDossier {
    phrase: Phrase,
    gestes: Vec<GesteDuDossier>,
    ecran: EcranDuDossier,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    dossier: PathBuf,
}

const APERCU_DU_DOSSIER: Affiche = Affiche::new("Aperçu");
const ECRIRE_LE_DOSSIER: Affiche = Affiche::new("Écrire le dossier");
const ENVOYER_LE_DOSSIER: Affiche = Affiche::new("L'envoyer à chaque salarié");

/// Les salaries qu'offrent les cases du dossier : ceux en activite, puis les archives ; chacun par sa cle, sous son nom.
fn salaries_offerts(ctx: &Contexte) -> Vec<crate::ouverture::Choix> {
    let mut fiches: Vec<(&droit::contexte::IdSalarie, &droit::fiche::FicheSalarie)> = ctx.salaries.iter().collect();
    fiches.sort_by(|a, b| a.1.archivee().cmp(&b.1.archivee()).then(a.0.cmp(b.0)));
    fiches
        .into_iter()
        .map(|(k, v)| {
            let nom = documents::tableaux_export::qui_souple(v);
            crate::ouverture::Choix::de(k.as_str(), if v.archivee() { format!("{} — archivé", nom) } else { nom })
        })
        .collect()
}

/// `dossier-formes` : les gestes du dossier d'un salarie -- l'apercu et l'ecrire, et, une boite posee, l'envoyer --,
/// leurs cases montrant la derniere annee et offrant les salaries.
fn dossier_formes(ctx: &Contexte) -> R<FormesDuDossier> {
    let salaries = salaries_offerts(ctx);
    // `periodes_exportables` : les mois, puis (sans rien en dire) le logiciel du comptable.
    let mois = mois_disponibles(ctx)?;
    let annees = annees_de(&mois);
    etat_logiciel(ctx)?;
    let boite_liee = crate::ouverture::boite_liee(ctx)?;
    let n = salaries.len();
    let valeurs = ValeursDuDossier { annee: annees.first().cloned() };
    let choix = ChoixDuDossier { salaries };
    let periode = [cle!(DemandeDeDossier, salaries), cle!(DemandeDeDossier, mois), cle!(DemandeDeDossier, annee), cle!(DemandeDeDossier, toutes_les_annees), cle!(DemandeDeDossier, categories)];
    let mut ecrire: Vec<&'static str> = periode.to_vec();
    ecrire.push(cle!(DemandeDeDossier, ensemble));
    let mut gestes = vec![
        GesteDeRoute::avec_cases(Route::DossierSalarie, DemandeDeDossier { apercu: Some(true), ..DemandeDeDossier::default() }, APERCU_DU_DOSSIER, ecrire.clone(), valeurs.clone(), choix.clone()),
        GesteDeRoute::avec_cases(Route::DossierSalarie, DemandeDeDossier::default(), ECRIRE_LE_DOSSIER, ecrire, valeurs.clone(), choix.clone()),
    ];
    if boite_liee {
        let envoyer = vec![
            cle!(ChargeDossierEnvoyer, salaries),
            cle!(ChargeDossierEnvoyer, mois),
            cle!(ChargeDossierEnvoyer, annee),
            cle!(ChargeDossierEnvoyer, toutes_les_annees),
            cle!(ChargeDossierEnvoyer, categories),
        ];
        gestes.push(GesteDeRoute::avec_cases(Route::DossierEnvoyer, DemandeDeDossier::default(), ENVOYER_LE_DOSSIER, envoyer, valeurs, choix).exigeant(Some(crate::coffre::demande(&ctx.racine, GESTE_DOSSIER)?)));
    }
    Ok(FormesDuDossier {
        phrase: socle::phrase!("Dossiers possibles : {} ; sortes de pièces : {}.", socle::ecran::compte(n, "salarié", "salariés"), documents::CategorieDeDocument::TOUTES.len()),
        gestes,
        ecran: EcranDuDossier {
            dit: vec![
                "Aucun salarié coché : le dossier de chacun.",
                "Une période : un mois, une année, ou toutes les années.",
                "Aucune sorte cochée : toutes. Une seule : un PDF. Plusieurs : un ZIP, un PDF par sorte.",
            ],
            dossier_dit: "Ouvrir le dossier",
        },
        dossier: documents::DossierHorsDuMois::DossiersDeSalaries.sous(&ctx.racine),
    })
}

// ------------------------------------------------- le logiciel du comptable

/// Un logiciel du comptable que la page propose : tel que les tables des exports le livrent
/// (`documents::plan_comptable::tables`), et ce que l'ecran en dit une fois choisi.
#[derive(Debug, Serialize)]
struct LogicielPropose {
    #[serde(flatten)]
    livre: documents::plan_comptable::Logiciel,
    dit: String,
}

/// Ce que la page montre du logiciel du comptable : les logiciels, celui retenu (absent : aucun), l'analytique, et ce
/// que le plan comptable ne tiendrait pas dans son format.
#[derive(Debug, Serialize)]
pub(crate) struct LogicielDuComptable {
    logiciels: Vec<LogicielPropose>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logiciel: Option<LogicielComptable>,
    analytique: bool,
    controle: Vec<String>,
}

/// Le logiciel du comptable, tel que la page le montre.
pub(crate) fn etat_logiciel(ctx: &Contexte) -> R<LogicielDuComptable> {
    let choix = ChoixCompta::lu(&ctx.racine)?;
    let livres = &tables().logiciels;
    // Le controle du plan pour le logiciel retenu ; aucun logiciel retenu, aucun plan a controler.
    let controle: Vec<String> = livres.iter().find(|l| Some(l.logiciel) == choix.logiciel).map(|l| controler_plan(ctx, l.format)).transpose()?.into_iter().flatten().collect();
    // Ce que l'ecran dit de chaque logiciel, une fois choisi.
    let logiciels = livres
        .iter()
        .map(|l| LogicielPropose { dit: format!("Fichier {} : {}. Retenu pour les prochaines fois.", l.format_lisible, l.pourquoi), livre: l.clone() })
        .collect();
    Ok(LogicielDuComptable { logiciels, logiciel: choix.logiciel, analytique: choix.analytique, controle })
}

/// La page ne propose pas la CASC-SVP de la 3090 a une compagnie de la 1285, ni un etat de convention a qui n'en a pas
/// dit, ni la dette de conges a qui n'a pas de permanent sur les mois a exporter (`permanents` :
/// `engagement::permanents::Permanents::ouverts`), ni la taxe sur les salaires a qui ne la doit aucune des annees a
/// exporter (`taxe_due`).
fn etat_concerne(ctx: &Contexte, etat: EtatDExport, piece: Option<droit::fiche::Piece>, permanents: bool, taxe_due: bool) -> bool {
    let idcc = ctx.entreprise.idcc.dite();
    let t = tables();
    if let Some(liste) = t.si_convention.get(&etat) {
        if !liste.iter().any(|c| Some(c) == idcc) {
            return false;
        }
    }
    // Un etat de la piece d'un salarie au mois (`plan_comptable::Tables::etats`).
    if piece.is_some() {
        return permanents;
    }
    if t.si_taxe_sur_les_salaires.contains(&etat) {
        return taxe_due;
    }
    true
}

/// La compagnie doit-elle la taxe sur les salaires d'une des annees `annees` ? Le regime de la fiche telle qu'elle est
/// au 31 decembre de chacune (`taxe_sur_les_salaires::regime_taxe_sur_les_salaires`, CGI art. 231, 1) : une annee ou la
/// fiche ne dit pas encore tout compte, l'etat dit ce qu'il faut repondre ; une annee ou elle n'est pas due, non.
fn taxe_due_une_annee(ctx: &Contexte, annees: &[Annee]) -> R<bool> {
    for a in annees {
        let fin = a.dernier_jour();
        let regime = documents::taxe_sur_les_salaires::regime_taxe_sur_les_salaires(&ctx.entreprise_au(&fin), None);
        if regime.decision != documents::taxe_sur_les_salaires::Decision::NonDue {
            return Ok(true);
        }
    }
    Ok(false)
}


/// CE QUE REND `exports-periodes` : sa phrase, les mois et les annees qu'il y a matiere a exporter, les etats que la
/// compagnie peut demander, les formats et les regroupements (tels que les tables des exports les livrent), le dossier
/// des exports, le logiciel du comptable, et ce que l'ecran dit (`ecran`).
#[derive(Debug, Serialize)]
struct ExportsPossibles {
    phrase: Phrase,
    mois: Vec<Mois>,
    annees: Vec<Annee>,
    familles: Vec<FamilleDEtats>,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    dossier: PathBuf,
    compta: LogicielDuComptable,
    ecran: EcranDesExports,
}

/// Les etats d'une famille (son libelle des tables), dans l'ordre des tables : la page les propose sous elle.
#[derive(Debug, Serialize)]
struct FamilleDEtats {
    libelle: String,
    etats: Vec<EtatPropose>,
}

/// Un etat que la compagnie peut exporter, tel que les tables le livrent, et ce que l'ecran en dit : les formats et les
/// regroupements qu'il offre (`formats_offerts`, `regroupements_offerts`), ceux que ses cases montrent d'abord, ce que
/// dit son regroupement (`regroupement_dit`), et la periode que son calendrier montre d'abord (`periode_proposee`).
#[derive(Debug, Serialize)]
struct EtatPropose {
    #[serde(flatten)]
    livre: documents::plan_comptable::EtatLivre,
    formats_offerts: Vec<crate::ouverture::Choix>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format_propose: Option<FormatDExport>,
    regroupements_offerts: Vec<crate::ouverture::Choix>,
    #[serde(skip_serializing_if = "Option::is_none")]
    regroupement_propose: Option<RegroupementDEtat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    regroupement_dit: Option<&'static str>,
    periode_proposee: PeriodeProposee,
}

/// La periode que le calendrier d'un etat montre d'abord : la plus recente qu'il admet (le dernier mois, sinon la
/// derniere annee, sinon toutes les annees), et ce qu'elle dit.
#[derive(Debug, Serialize)]
struct PeriodeProposee {
    quand: documents::plan_comptable::PeriodeDExport,
    #[serde(skip_serializing_if = "Option::is_none")]
    periode: Option<String>,
    dit: String,
}

/// CE QUE L'ECRAN DES EXPORTS DIT : rien a exporter, les mots de ses cases et de ses boutons, l'analytique, le format,
/// les coordonnees, le dossier.
#[derive(Debug, Serialize)]
struct EcranDesExports {
    vide: Option<&'static str>,
    quoi: &'static str,
    lequel: &'static str,
    choisir: &'static str,
    quand: &'static str,
    tout: &'static str,
    toutes_dit: &'static str,
    regroupement_libelle: &'static str,
    logiciel_libelle: &'static str,
    aucun_logiciel: &'static str,
    analytique_libelle: &'static str,
    format_libelle: &'static str,
    coordonnees_libelle: &'static str,
    apercu: &'static str,
    dossier_dit: &'static str,
    analytique: &'static str,
    format_libre: &'static str,
    coordonnees: &'static str,
    dossier: String,
}

/// Ce que dit le regroupement du livre de paie : ses trois regroupements disent chacun a qui il sert.
const REGROUPEMENT_LIVRE: &str = "Par salarié, ce qu'on envoie au comptable ; par mois, ce qu'on rapproche de la banque ; par spectacle, ce qui justifie une subvention. Les trois disent la même chose, et le total ne bouge pas.";
/// Ce que dit le regroupement d'un autre etat.
const REGROUPEMENT: &str = "Le total ne bouge pas : seule la répartition des lignes change.";
/// Ce que dit la periode de toutes les annees.
const TOUTES_LES_ANNEES_DIT: &str = "toutes les années";

/// L'etat `e`, tel que l'ecran le propose (`EtatPropose`), aux mois `mois` qu'il y a matiere a exporter, dans l'ordre.
fn etat_propose(e: documents::plan_comptable::EtatLivre, mois: &[Mois]) -> EtatPropose {
    use documents::plan_comptable::PeriodeDExport as P;
    let t = tables();
    let formats_offerts = t.formats.iter().filter(|f| e.formats.contains(&f.format)).map(|f| crate::ouverture::Choix::de(f.format, &f.quoi)).collect();
    let regroupements_offerts = e
        .regroupements
        .iter()
        .filter_map(|r| t.regroupements.iter().find(|x| x.regroupement == *r))
        .map(|r| crate::ouverture::Choix::de(r.regroupement, &r.quoi))
        .collect();
    let a_des_regroupements = e.questions.iter().any(|q| q == "regroupement");
    let regroupement_dit = a_des_regroupements.then_some(if e.etat == EtatDExport::LivreDePaie { REGROUPEMENT_LIVRE } else { REGROUPEMENT });
    let dernier = mois.last().copied();
    let periode_proposee = match dernier {
        Some(m) if e.periodes.contains(&P::Mois) => PeriodeProposee { quand: P::Mois, periode: Some(m.to_string()), dit: m.libelle(socle::mois::Graphie::Accentuee) },
        Some(m) if e.periodes.contains(&P::Annee) => PeriodeProposee { quand: P::Annee, periode: Some(m.annee().to_string()), dit: m.annee().to_string() },
        _ => PeriodeProposee { quand: P::Toutes, periode: None, dit: TOUTES_LES_ANNEES_DIT.to_string() },
    };
    EtatPropose {
        formats_offerts,
        format_propose: e.formats.contains(&FormatDExport::Csv).then_some(FormatDExport::Csv),
        regroupements_offerts,
        regroupement_propose: e.regroupements.first().copied(),
        regroupement_dit,
        periode_proposee,
        livre: e,
    }
}

/// Les etats ranges par famille, chacune a la place de son premier etat.
fn familles_de(etats: Vec<EtatPropose>) -> Vec<FamilleDEtats> {
    let mut familles: Vec<FamilleDEtats> = Vec::new();
    for e in etats {
        match familles.iter_mut().find(|f| f.libelle == e.livre.famille) {
            Some(f) => f.etats.push(e),
            None => familles.push(FamilleDEtats { libelle: e.livre.famille.clone(), etats: vec![e] }),
        }
    }
    familles
}

/// Les mois et les annees qu'il y a matiere a exporter, les etats, les formats, et le logiciel du comptable.
fn exports_periodes(ctx: &Contexte) -> R<ExportsPossibles> {
    let mois = mois_disponibles(ctx)?;
    let t = tables();
    // Les mois a exporter, du premier au dernier : un permanent s'y propose-t-il, ou y est-il du ?
    // Aucun mois a exporter : aucun permanent n'y est.
    let permanents = mois
        .first()
        .zip(mois.last())
        .map(|(premier, dernier)| engagement::permanents::Permanents::de(ctx, &socle::date::Periode::du_au(premier.premier_jour(), dernier.dernier_jour())?))
        .transpose()?
        .is_some_and(|p| p.ouverts());
    let annees = annees_de(&mois);
    let taxe_due = taxe_due_une_annee(ctx, &annees)?;
    let etats: Vec<EtatPropose> = t.etats().filter(|(e, piece)| etat_concerne(ctx, e.etat, *piece, permanents, taxe_due)).map(|(e, _)| etat_propose(e.clone(), &mois)).collect();
    let n = etats.len();
    let dossier = documents::DossierDeTravail::Exports.sous(&ctx.racine);
    let ecran = EcranDesExports {
        vide: mois.is_empty().then_some("Aucune paie saisie : il n'y a rien à exporter pour l'instant."),
        quoi: "Quoi",
        lequel: "Lequel",
        choisir: "— choisir —",
        quand: "Quand",
        tout: "Tout",
        toutes_dit: TOUTES_LES_ANNEES_DIT,
        regroupement_libelle: "Regroupement",
        logiciel_libelle: "Logiciel du comptable",
        aucun_logiciel: "Aucun",
        analytique_libelle: "Analytique par spectacle",
        format_libelle: "Format",
        coordonnees_libelle: "Coordonnées des salariés",
        apercu: "Aperçu",
        dossier_dit: "Ouvrir le dossier",
        analytique: "Les salaires bruts ventilés par spectacle. Le code de chaque spectacle se règle sur sa fiche : Configuration › Spectacles, case « Code analytique ». Vide, il est tiré du titre.",
        format_libre: "Le format se choisit ci-dessous.",
        coordonnees: "Le NIR, la naissance, l'adresse. Sans la case, rien de tout cela ne sort ; un fichier qui les porte ne s'envoie qu'à qui en a besoin.",
        dossier: format!("{} — et nulle part ailleurs : un export emporte des salaires", dossier.display()),
    };
    Ok(ExportsPossibles {
        phrase: socle::phrase!("Exports : {} ; mois payés : {} ; années : {}.", socle::ecran::compte(n, "état", "états"), mois.len(), annees.len()),
        mois,
        annees,
        familles: familles_de(etats),
        dossier,
        compta: etat_logiciel(ctx)?,
        ecran,
    })
}

charge! {
    /// La charge de `retenir-logiciel` : le logiciel du comptable (absent ou vide : aucun), et l'analytique.
    #[derive(Debug)]
    pub(crate) struct ChargeRetenirLogiciel {
        logiciel: Option<LogicielComptable> = LOGICIEL,
        analytique: Option<bool> = ANALYTIQUE,
    }
}

/// Ce que rend `retenir-logiciel` : sa phrase, et le logiciel du comptable tel que la page le montre.
#[derive(Debug, Serialize)]
struct LogicielRetenu {
    phrase: Phrase,
    #[serde(flatten)]
    compta: LogicielDuComptable,
}

/// Le logiciel du comptable, et l'analytique, retenus une fois pour
/// toutes. Rend le controle du plan pour ce logiciel. Ce n'est pas un export : un reglage, pas une donnee de paie.
fn retenir_logiciel(ctx: &Contexte, c: ChargeRetenirLogiciel) -> R<LogicielRetenu> {
    if let Some(nom) = c.logiciel {
        if !documents::plan_comptable::logiciels_connus().contains(&nom) {
            return Err(refus(format!("Logiciel inconnu : « {} ».", nom)));
        }
    }
    ChoixCompta::nouveau(c.logiciel, c.analytique == Some(true)).ecrire(&ctx.racine)?;
    let compta = etat_logiciel(ctx)?;
    let phrase = match &compta.logiciel {
        None => Phrase::fixe("Aucun logiciel comptable retenu."),
        Some(l) => socle::phrase!("Logiciel comptable retenu : {}.", l),
    };
    Ok(LogicielRetenu { phrase, compta })
}

charge! {
    /// La charge de `casc-regularise` : l'annee, le jour du virement, le montant vire (tape, il se convertit a la porte :
    /// `Charge::lire`).
    #[derive(Debug)]
    struct ChargeCascRegularise {
        annee: Annee = ANNEE,
        vire_le: Jour = dit("Viré le", "le jour du virement, AAAA-MM-JJ"),
        montant: socle::grandeurs::Montant = dit("Montant viré", "le montant viré"),
    }
}

/// Ce que rend `casc-regularise` : sa phrase, et la regularisation notee.
#[derive(Debug, Serialize)]
struct CascNotee {
    phrase: Phrase,
    #[serde(flatten)]
    notee: RegularisationNotee,
}

/// Note qu'une annee de CASC-SVP est regularisee (apres le virement).
fn casc_regularise(ctx: &Contexte, c: ChargeCascRegularise) -> R<CascNotee> {
    let aujourd_hui = socle::temps::secondes().a_la_seconde().jour_de_paris();
    let vire = socle::grandeurs::sans_zero(c.montant);
    let notee = marquer_regularise(ctx, c.annee, c.vire_le, vire, INTERFACE, aujourd_hui)?;
    let montant = socle::texte::euros(notee.montant);
    Ok(CascNotee { phrase: socle::phrase!("CASC-SVP {} notée régularisée : {} virés le {}.", notee.annee, montant, notee.vire_le.jj_mm_aaaa()), notee })
}

charge! {
    /// La charge de `exporter` : l'etat (le livre de paie sans lui), la periode (le mois, l'annee, ou toutes les annees),
    /// le regroupement, le format, les coordonnees des salaries, l'apercu (rien n'est ecrit), et pour les ecritures
    /// comptables le logiciel du comptable et l'analytique. Le regroupement et le format se disent quand l'etat les
    /// demande (ses `questions`, ses `formats` : les tables des exports) ; ce qui n'est pas dit est absent.
    #[derive(Debug)]
    pub(crate) struct ChargeExporter {
        etat: EtatDExport = dit("État", "l'état ; exports-periodes dit ceux que la compagnie peut exporter"),
        mois: Option<Mois> = legendes::MOIS,
        annee: Option<Annee> = ANNEE,
        toutes_les_annees: Option<bool> = TOUTES_LES_ANNEES,
        regroupement: Option<RegroupementDEtat> = dit("Regroupement", "le regroupement des lignes, parmi ceux que l'état propose (exports-periodes)").avancee(),
        format: Option<FormatDExport> = dit("Format", "le format du fichier, parmi ceux que l'état propose (exports-periodes)").avancee(),
        coordonnees: Option<bool> = dit("Coordonnées", "vrai : les coordonnées des salariés, quand l'état les propose").avancee(),
        apercu: Option<bool> = dit("Aperçu seulement", "vrai : le tableau, sans écrire de fichier").avancee(),
        logiciel: Option<LogicielComptable> = dit("Logiciel", "le logiciel du comptable, pour les écritures comptables").avancee(),
        analytique: Option<bool> = ANALYTIQUE,
    }
}

/// Ce que l'etat `forme` demande et que la charge ne dit pas, ou dit hors de ce qu'il permet : le refus qui le nomme.
fn choisi<T: Copy + PartialEq + std::fmt::Display>(quoi: &str, dit: Option<T>, permis: &[T]) -> R<T> {
    let permis_dits = || permis.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
    match dit {
        Some(v) if permis.contains(&v) => Ok(v),
        Some(v) => Err(socle::echec::cle(format!("{} « {} » : cet état se fait en {}.", quoi, v, permis_dits()), quoi).reclasser(Classe::SaisieIncomplete)),
        None => Err(socle::echec::cle(format!("Choisir le {} de cet état : {}.", quoi, permis_dits()), quoi).reclasser(Classe::SaisieIncomplete)),
    }
}

/// CE QUE REND `exporter` : sa phrase, l'etat tel que son module le rend (le livre de paie, un tableau, la
/// regularisation CASC-SVP), ce qu'il dit de lui-meme sous la phrase (`phrases` : ses avertissements, ce qui est du, ce
/// qui est infere), et ce que l'ecran en dit (`ecran`).
#[derive(Serialize)]
pub(crate) struct EtatExporte {
    phrase: Phrase,
    #[serde(flatten)]
    etat: EtatRendu,
    phrases: Vec<String>,
    ecran: EcranDUnEtat,
}

/// L'etat lui-meme, tel que la route le rend : la regularisation CASC-SVP, le livre de paie, ou un tableau.
#[derive(Serialize)]
#[serde(untagged)]
enum EtatRendu {
    Casc(crate::importation::casc::Regularisation),
    Livre(documents::etats_paie::LivreDePaie),
    Tableau(documents::tableaux_export::Tableau),
}

/// CE QUE L'ECRAN DIT D'UN ETAT EXPORTE : ses donnees personnelles, ses totaux partiels, et pour la regularisation
/// CASC-SVP ce que disent ses deux etapes.
#[derive(Debug, Serialize)]
struct EcranDUnEtat {
    personnelles: Option<&'static str>,
    partiels: Option<String>,
    virement: &'static str,
    automatique: &'static str,
    a_la_main: &'static str,
    /// La regularisation CASC-SVP, une boite posee : la demande du mot de passe avant `casc-envoyer`.
    #[serde(skip_serializing_if = "Option::is_none")]
    demande: Option<compagnie::demande::DemandeDuMotDePasse>,
}

/// Le geste de `casc-envoyer`, tel que la demande du mot de passe le dit.
const GESTE_CASC: &str = "envoyer à Audiens et au CASC-SVP les deux courriels de la régularisation";

/// Le geste de `dossier-envoyer`, tel que la demande du mot de passe le dit.
const GESTE_DOSSIER: &str = "envoyer par courriel à chaque salarié choisi son dossier";

/// Le verbe unique, `exporter <etat>` : l'etat demande, tel que l'ecran le montre.
fn exporter(ctx: &Contexte, c: &ChargeExporter) -> R<EtatExporte> {
    let nom = |f: &Option<std::path::PathBuf>| -> R<Option<String>> { f.as_deref().map(|p| NomDeFichier::du(p).map(String::from)).transpose() };
    // (le titre, les fichiers ecrits, ses avertissements, ses donnees personnelles, ses totaux partiels, la boite posee)
    let (etat, titre, ecrits, avertissements, personnelles, partiels, boite_liee) = match exporter_objet(ctx, c)? {
        Exporte::Casc(r) => {
            let (periode, ecrits, avertissements, boite_liee) = (r.periode.clone(), r.fichiers.iter().map(|f| NomDeFichier::du(f).map(String::from)).collect::<R<Vec<_>>>()?, r.avertissements.clone(), r.boite_liee);
            (EtatRendu::Casc(r), periode, ecrits, avertissements, false, Vec::new(), boite_liee)
        }
        Exporte::Livre(l) => {
            let bulletins = l.totaux.bulletins();
            let partiels = l
                .totaux
                .inconnus()
                .iter()
                .map(|(colonne, n)| {
                    let sur = bulletins.saturating_sub(*n);
                    format!("{} sur {} bulletin{} sur {}", colonne.cle().replace('_', " "), sur, if sur == 1 { "" } else { "s" }, bulletins)
                })
                .collect();
            let (periode, ecrit, avertissements) = (l.periode.clone(), nom(&l.fichier)?.into_iter().collect(), l.avertissements.clone());
            (EtatRendu::Livre(l), periode, ecrit, avertissements, false, partiels, false)
        }
        Exporte::Tableau(t) => {
            let (titre, ecrit, avertissements, personnelles) = (t.titre.clone(), nom(&t.fichier)?.into_iter().collect(), t.avertissements.clone(), t.donnees_personnelles);
            (EtatRendu::Tableau(t), titre, ecrit, avertissements, personnelles, Vec::new(), false)
        }
    };
    // Le nom de chaque fichier ecrit part dans la reponse (`fichier`, `fichiers`) : la phrase le dit en mots.
    let phrase = match ecrits.len() {
        0 => socle::phrase!("{} : aperçu, rien n'est écrit.", titre),
        1 => socle::phrase!("{} : le fichier est écrit.", titre),
        n => socle::phrase!("{} : {} fichiers écrits.", titre, n),
    };
    let ecran = EcranDUnEtat {
        personnelles: personnelles.then_some("Cet état porte des données personnelles : il reste dans prive/, et ne s'envoie qu'à qui en a besoin."),
        partiels: (!partiels.is_empty()).then(|| format!("Totaux partiels : {}.", partiels.join(" ; "))),
        virement: "Vers Audiens Santé Prévoyance – Encaissement. L'IBAN se lit dans l'espace employeur Audiens ou sur la page employeurs du CASC-SVP.",
        automatique: if boite_liee {
            "Audiens reçoit les masses et les dates des virements ; le CASC-SVP, la liste nominative. Une copie reste dans le dossier, et les années sont notées régularisées."
        } else {
            "La boîte courriel n'est pas posée : Configuration › Courriel. Une fois posée, l'envoi se fait d'ici en un clic."
        },
        a_la_main: "Joindre la liste nominative au courriel du CASC-SVP, recopier les deux courriels ci-dessous, puis noter la régularisation.",
        demande: if boite_liee { Some(crate::coffre::demande(&ctx.racine, GESTE_CASC)?) } else { None },
    };
    Ok(EtatExporte { phrase, etat, phrases: avertissements, ecran })
}

/// Ce qu'un export rend, avant d'etre mis en JSON (le chemin ecrit en entier, chaque montant dans sa grandeur).
pub(crate) enum Exporte {
    Casc(crate::importation::casc::Regularisation),
    Livre(documents::etats_paie::LivreDePaie),
    Tableau(documents::tableaux_export::Tableau),
}

/// `exporter`, avant la mise en JSON.
pub(crate) fn exporter_objet(ctx: &Contexte, c: &ChargeExporter) -> R<Exporte> {
    use documents::etats_paie::Demande;
    let etat = c.etat;
    let etats: Vec<&documents::plan_comptable::EtatLivre> = documents::plan_comptable::tables().etats().map(|(e, _)| e).collect();
    let Some(forme) = etats.iter().find(|e| e.etat == etat) else {
        let mut noms: Vec<&str> = etats.iter().map(|e| e.etat.as_str()).collect();
        noms.sort_unstable();
        return Err(refus(format!("État inconnu : « {} ». Les états possibles sont {}.", etat, noms.join(", "))));
    };
    let demande = |q: &str| forme.questions.iter().any(|x| x == q);
    // Ce que l'etat ne demande pas ne lui est pas passe (`Demande` n'en porte pas) : un regroupement n'a pas de sens pour
    // les cotisations.
    let regroupement = if demande("regroupement") { Some(choisi("regroupement", c.regroupement, &forme.regroupements)?) } else { None };
    let format = if forme.formats.is_empty() { None } else { Some(choisi("format", c.format, &forme.formats)?) };
    let d = Demande {
        mois: c.mois,
        annee: c.annee,
        toutes: c.toutes_les_annees == Some(true),
        regroupement,
        format,
        coordonnees: demande("coordonnees") && c.coordonnees == Some(true),
        ecrire: c.apercu != Some(true),
        logiciel: c.logiciel,
        analytique: c.analytique,
        departs: envois::journal::lu(&ctx.racine).map(|j| j.departs)?,
    };
    use EtatDExport as E;
    let tableau = match etat {
        E::RegularisationCascSvp => {
            let demande = crate::importation::casc::Demande { mois: d.mois, annee: d.annee, toutes_les_annees: d.toutes, boite_liee: crate::ouverture::boite_liee(ctx)?, jusqu_a: None, annees: Vec::new(), vire_le: Vec::new() };
            let mut r = crate::importation::casc::regularisation(ctx, &demande)?.dossier()?;
            if d.ecrire {
                crate::importation::casc::ecrire(ctx, &mut r)?;
            }
            return Ok(Exporte::Casc(r));
        }
        E::LivreDePaie => return Ok(Exporte::Livre(documents::etats_paie::livre_de_paie(ctx, &d)?)),
        E::LivreDePaieDetaille => documents::etats_paie::livre_detaille(ctx, &d),
        E::RecapitulatifAnnuel => documents::etats_paie::recapitulatif_annuel(ctx, &d),
        E::MasseSalariale => documents::etats_paie::masse_salariale(ctx, &d),
        E::EtatDesVirements => documents::etats_paie::etat_des_virements(ctx, &d),
        E::PrelevementALaSource => documents::etats_paie::prelevement_a_la_source(ctx, &d),
        E::CotisationsParOrganisme => declarations::etats_organismes::cotisations_par_organisme(ctx, &d),
        E::VersementsOrganismes => declarations::etats_organismes::versements_organismes(ctx, &d, &crate::gestes::depart::dsn_du_mois),
        E::BordereauUrssaf => declarations::etats_organismes::bordereau_urssaf(ctx, &d, &crate::gestes::depart::dsn_du_mois),
        E::FcapSvp => declarations::etats_organismes::fcap_svp(ctx, &d),
        E::Afdas2642 => declarations::etats_organismes::afdas_2642(ctx, &d),
        E::ParitarismeBranche => declarations::etats_organismes::paritarisme_branche(ctx, &d),
        E::ReductionGenerale => documents::etats_paie::reduction_generale(ctx, &d),
        E::Assiettes => documents::etats_paie::assiettes(ctx, &d),
        E::HeuresEtCachets => documents::etats_paie::heures_et_cachets(ctx, &d),
        E::CoutParSpectacle => documents::etats_paie::cout_par_spectacle(ctx, &d),
        E::Contrats => documents::etats_personnel::liste_des_contrats(ctx, &d),
        E::RegistreDuPersonnel => documents::etats_personnel::registre_du_personnel(ctx, &d),
        E::FichierDuPersonnel => documents::etats_personnel::fichier_du_personnel(ctx, &d),
        E::JournalDesDeclarations => documents::etats_personnel::journal_des_declarations(ctx, &d, &actes_du_journal(ctx)?),
        E::EcrituresComptables => documents::etats_compta::ecritures_comptables(ctx, &d),
        E::GrandLivrePaie => documents::etats_compta::grand_livre(ctx, &d),
        E::BalancePaie => documents::etats_compta::balance(ctx, &d),
        E::ProvisionConges => documents::etats_compta::provision_conges(ctx, &d),
        E::TaxeSurLesSalaires => documents::etats_compta::taxe_sur_les_salaires(ctx, &d),
    }?;
    Ok(Exporte::Tableau(tableau))
}

const CORPS_DOSSIER: &str = "Bonjour {prenom},\n\nTu trouveras ci-joint ton dossier de salarié{periode} : {liste}.\n\n{reedition}Bien à toi,\n{signataire}\n{raison_sociale}\n";

/// Ce que le courriel dit des contrats, quand le dossier en porte : eux seuls se reeditent (les bulletins et les
/// certificats sont les pages memes de l'archive ; le reste se prend tel quel).
const REEDITION_CONTRATS: &str = "Les contrats ont été réédités par notre logiciel de paie depuis les données\nd'origine : les numéros et les montants sont ceux des contrats qui t'ont été\nremis ; seule la mise en page peut avoir changé depuis.\n\n";

charge! {
    /// La charge de `dossier-envoyer` : les salaries (aucun : chacun), la periode (le mois, l'annee, ou toutes les annees),
    /// ce que le dossier porte (tout sinon). L'envoi refait le dossier de chacun : ni apercu, ni dossier d'ensemble.
    #[derive(Debug)]
    struct ChargeDossierEnvoyer {
        salaries: Option<Vec<droit::contexte::IdSalarie>> = dit("Salariés", "les clés de leurs fiches"),
        mois: Option<Mois> = legendes::MOIS,
        annee: Option<Annee> = ANNEE,
        toutes_les_annees: Option<bool> = TOUTES_LES_ANNEES,
        categories: Option<Vec<documents::CategorieDeDocument>> = dit("Ce que le dossier porte", "ce que le dossier porte ; tout sinon").avancee(),
    }
}

/// La route `dossier-envoyer` : le dossier se refait ici (`dossier_salarie`, un fichier par salarie, ecrit dans
/// `exports/dossiers/`), puis part a chaque salarie par la boite de la compagnie. La reponse est celle du dossier
/// (`DossierRendu`) et, sous `envoi`, ce que l'envoi a fait.
fn dossier_envoyer(ctx: &Contexte, c: ChargeDossierEnvoyer, motdepasse: Option<&socle::secret::Secret>, ouvrir: Ouvrir) -> R<DossierRendu> {
    let demande = |apercu: bool| DemandeDeDossier {
        salaries: c.salaries.clone(),
        mois: c.mois,
        annee: c.annee.clone(),
        toutes_les_annees: c.toutes_les_annees,
        categories: c.categories.clone(),
        ensemble: Some(false),
        apercu: Some(apercu),
    };
    // CE QUI PARTIRAIT, SANS RIEN ECRIRE : chacun a son adresse, puis la preuve ; le dossier ne s'ecrit qu'ensuite.
    let vu = construire(ctx, &demande(true))?;
    adresses_de(ctx, &vu.salaries)?;
    let forme = verifier_preuve(ctx, Some(&Preuve::du_mot_de_passe(motdepasse.cloned())), &crate::coffre::demande(&ctx.racine, GESTE_DOSSIER)?)?;
    let d = construire(ctx, &demande(false))?;
    let envoi = envoyer_dossier(ctx, &d, &Preuve { donnee: Some(forme), ..Preuve::default() }, INTERFACE, ouvrir)?;
    let phrase = socle::phrase!("Dossier de {} ({}) : {}.", d.salaries.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", "), d.periode, socle::ecran::compte(d.fichiers.len(), "fichier joint", "fichiers joints"));
    Ok(DossierRendu::de(d, phrase, Some(envoi)))
}

/// Chacun de `cles` a une adresse dans sa fiche ; sinon le refus qui le nomme. Rien n'est parti.
fn adresses_de(ctx: &Contexte, cles: &[droit::contexte::IdSalarie]) -> R<Vec<String>> {
    cles.iter()
        .map(|cle| {
            let s = ctx.salaries.get(cle).ok_or_else(|| socle::fichiers::manque(cle.as_str()))?;
            s.email.en_texte().ok_or_else(|| {
                refus(format!("Pas d'adresse e-mail pour {} : compléter sa fiche avant d'envoyer. Rien n'est parti.", ctx.nomme(cle)))
            })
        })
        .collect()
}

/// Envoie a chaque salarie SON dossier, par la boite de la compagnie.
/// Irreversible : la meme preuve que les pieces du mois, le meme journal. Un dossier `ensemble` ne part a personne.
fn envoyer_dossier(ctx: &Contexte, d: &crate::dossier_salarie::Dossier, preuve: &Preuve, qui: &str, ouvrir: Ouvrir) -> R<DossierEnvoye> {
    if d.fichiers.iter().map(|f| NomDeFichier::du(f)).collect::<R<Vec<_>>>()?.iter().any(|n| matches!(documents::SorteDePieceProduite::du_nom(n.texte()), Some((documents::SorteDePieceProduite::Dossiers, _)))) {
        return Err(refus(
            "Un dossier qui réunit tous les salariés ne s'envoie à personne : il porte la paie de chacun. Refaire un fichier par salarié.",
        ));
    }
    if d.fichiers.len() != d.salaries.len() {
        return Err(refus("Le dossier n'a pas été écrit : rien à envoyer."));
    }
    let mut messages: Vec<Message<droit::contexte::IdSalarie>> = Vec::new();
    let adresses = adresses_de(ctx, &d.salaries)?;
    for ((cle, f), email) in d.salaries.iter().zip(&d.fichiers).zip(adresses) {
        let s = ctx.salaries.get(cle).ok_or_else(|| socle::fichiers::manque(cle.as_str()))?;
        let prenom = s.prenom_affichage.portee().or(s.prenom.portee()).ok_or_else(|| socle::fichiers::manque("prenom"))?.to_string();
        let signataire = ctx.entreprise.signataire.dite().ok_or_else(|| socle::fichiers::manque("signataire"))?;
        let signe = signataire.prenom_affichage.remplie().or(signataire.prenom.portee()).ok_or_else(|| socle::fichiers::manque("prenom"))?;
        let ses_pages = d.pages.get(cle.as_str());
        let liste = d
            .categories
            .iter()
            .filter(|c| ses_pages.is_some_and(|p| p.get(c.texte()).is_some()))
            .map(|c| c.libelle().to_lowercase())
            .collect::<Vec<_>>()
            .join(", ");
        let corps = envoi::formater(
            CORPS_DOSSIER,
            &[
                ("prenom", prenom.clone()),
                ("periode", if d.periode == "toute la période" { String::new() } else { format!(" ({})", d.periode) }),
                ("liste", liste),
                ("reedition", if ses_pages.is_some_and(|p| p.get(documents::CategorieDeDocument::Contrats.texte()).is_some()) { REEDITION_CONTRATS.to_string() } else { String::new() }),
                ("signataire", signe.to_string()),
                ("raison_sociale", ctx.entreprise.nommee().ok_or_else(|| socle::fichiers::manque("raison_sociale"))?.to_string()),
            ],
        );
        messages.push(Message {
            pour: cle.clone(),
            nom: ctx.nomme(cle),
            destinataire: email.to_lowercase(),
            sujet: format!("Ton dossier de salarié — {}", d.periode),
            corps,
            pieces: vec![f.clone()],
            html: None,
        });
    }
    let mut client = ouvrir()?;
    verifier_preuve(ctx, Some(preuve), &crate::coffre::demande(&ctx.racine, GESTE_DOSSIER)?)?;
    let expediteur = expediteur_de(client.as_ref())?;
    let detail = journal::DetailDossier {
        periode: d.periode.clone(),
        categories: d.categories.clone(),
        salaries: d
            .salaries
            .iter()
            .zip(&messages)
            .map(|(cle, m)| Ok(journal::DossierServi { cle: cle.clone(), piece: m.pieces.first().map(|p| NomDeFichier::du(p)).transpose()? }))
            .collect::<R<Vec<_>>>()?,
        expediteur: expediteur.clone(),
        partis: None,
        echec_a: None,
    };
    let partis = match envoi::envoyer_suivi(&messages, &expediteur, client.as_mut(), None, None) {
        Ok(p) => p,
        Err(e) => {
            noter_echec_d_envoi(ctx, journal::Ligne::DossierSalarie, None, qui, &e.partis, &e.echoue, &e.echec.phrase, detail, None)?;
            return Err(e.echec);
        }
    };
    noter(ctx, journal::Ligne::DossierSalarie(par_courriel(None, qui, true, &en_liste(&partis, ", "), "", detail, None)))?;
    Ok(DossierEnvoye { partis, messages: messages.len(), expediteur })
}

/// Les actes du journal des envois, tels que le journal des declarations les montre.
fn actes_du_journal(ctx: &Contexte) -> R<Vec<documents::etats_personnel::ActeDuJournal>> {
    let lignes = journal::lu(&ctx.racine).map_err(|m| m.reclasser(Classe::Disque))?.lignes;
    Ok(lignes
        .iter()
        .map(|l| documents::etats_personnel::ActeDuJournal {
            quand: l.quand(),
            quoi: l.acte(),
            remise: l.remise(),
            mois: l.mois(),
            mode: l.mode(),
            accepte: l.accepte(),
            fichier: l.fichier().cloned(),
            destinataire: l.destinataire().to_string(),
            reponse: l.reponse().to_string(),
            origine: l.origine().to_string(),
        })
        .collect())
}

/// Le jour de chaque virement fait, par annee : un objet `{annee: jour}`.
impl ValeurDeLaCharge<ParSaForme> for std::collections::BTreeMap<Annee, Jour> {
    const FORME: Forme = Forme::Objet;
}

charge! {
    motdepasse = dit("Mot de passe du logiciel", "le mot de passe du logiciel ; sans lui, le moteur le demande à un humain");
    /// La charge de `casc-envoyer` : le jour de chaque virement fait, par annee (AAAA-MM-JJ) ; le mot de passe du
    /// logiciel tape pour l'envoi, l'enveloppe le prend (`Charge::mot_de_passe`).
    #[derive(Debug)]
    struct ChargeCasc {
        vire_le: std::collections::BTreeMap<Annee, Jour> = dit("Virements faits", "{année: jour du virement fait, AAAA-MM-JJ}"),
    }
}

/// Envoie les deux courriels de la regularisation, APRES les virements.
/// Irreversible. `vire_le` : {annee: AAAA-MM-JJ}, une date par virement fait ; le dossier envoye est celui de ces
/// annees-la, exactement.
///
/// Dans cet ordre : la boite, la preuve, le dossier (ecrit, pour que la liste nominative jointe soit celle du jour),
/// l'envoi, puis ce qui s'en garde -- les deux `.eml` dans `exports/casc-svp/envoyes/`, la ligne de
/// journal, et les annees notees regularisees. Rien de tout cela ne s'ecrit si l'envoi casse.
fn casc_envoyer(ctx: &Contexte, c: ChargeCasc, motdepasse: Option<&socle::secret::Secret>, ouvrir: Ouvrir) -> R<CascEnvoyee> {
    let qui = INTERFACE;
    use crate::importation::casc::{ecrire, regularisation, Demande};
    let aujourdhui = socle::temps::jour_de_paris();
    let mut dates: Vec<(socle::mois::Annee, Jour)> = Vec::new();
    for (a, &jour) in &c.vire_le {
        if jour > aujourdhui {
            return Err(refus(format!(
                "Le virement {} est date du {}, qui n'est pas encore arrive : le courriel annonce un virement FAIT. Rien n'est parti.",
                a,
                jour.jj_mm_aaaa()
            )));
        }
        dates.push((*a, jour));
    }
    if dates.is_empty() {
        return Err(refus("Aucune date de virement : les courriels annoncent des virements faits, ils partent apres. Rien n'est parti."));
    }
    let mut annees_voulues: Vec<socle::mois::Annee> = dates.iter().map(|(a, _)| *a).collect();
    annees_voulues.sort();

    // La boite et la preuve, puis le dossier, ecrit : la liste nominative jointe est celle du jour.
    let mut r = regularisation(ctx, &Demande { mois: None, annee: None, toutes_les_annees: true, jusqu_a: None, annees: annees_voulues.clone(), vire_le: dates.clone(), boite_liee: false })?.dossier()?;
    let manquent: Vec<String> = annees_voulues.iter().filter(|a| !r.annees.iter().any(|x| x.annee == **a)).map(|a| a.to_string()).collect();
    if !manquent.is_empty() {
        return Err(refus(format!("Rien a regulariser pour {} (deja fait, ou aucune DSN deposee). Rien n'est parti.", manquent.join(", "))));
    }

    let mut client = ouvrir()?;
    verifier_preuve(ctx, Some(&Preuve::du_mot_de_passe(motdepasse.cloned())), &crate::coffre::demande(&ctx.racine, GESTE_CASC)?)?;

    // La liste nominative est la premiere chose qu'ecrit le dossier (`casc::ecrire`) : si elle ne peut s'ecrire, le refus
    // tombe avant tout autre fichier, et rien ne part.
    let dossier = ecrire(ctx, &mut r)?;
    let classeur = dossier.join("CASC-SVP_liste-nominative.xlsx");

    // Chaque courriel porte le nom court de son organisme : celui du `.eml` garde.
    let messages: Vec<Message<NomDeFichier>> = vec![
        Message {
            pour: const { NomDeFichier::du_moteur("audiens") },
            nom: "Audiens".into(),
            destinataire: r.courriel_audiens.a.clone(),
            sujet: r.courriel_audiens.objet.clone(),
            corps: r.courriel_audiens.corps.clone(),
            pieces: Vec::new(),
            html: Some(courriel_en_html(&r.courriel_audiens.corps)),
        },
        Message {
            pour: const { NomDeFichier::du_moteur("casc") },
            nom: "CASC-SVP".into(),
            destinataire: r.courriel_casc.a.clone(),
            sujet: r.courriel_casc.objet.clone(),
            corps: r.courriel_casc.corps.clone(),
            pieces: vec![classeur],
            html: None,
        },
    ];
    let compte = droit::fiche::Courriel::lire(client.compte())?;
    // Chaque annee regularisee est une annee voulue : son virement a sa date.
    let date_de = |a: socle::mois::Annee| {
        dates
            .iter()
            .find(|(k, _)| *k == a)
            .map(|(_, d)| *d)
            .ok_or_else(|| socle::echec::a_la_cle("Défaut du logiciel : une année régularisée n'a pas la date de son virement.", "annee", a).reclasser(Classe::DefautDuLogiciel))
    };
    let annees = r.annees.iter().map(|a| Ok(journal::AnneeCasc { annee: a.annee, vire_le: date_de(a.annee)?, montant: a.cotisation })).collect::<R<Vec<_>>>()?;
    let mut detail = journal::DetailCasc {
        annees,
        mois: r.mois.iter().map(|m| m.mois).collect(),
        total: r.total,
        expediteur: compte.clone(),
        eml: Vec::new(),
        partis: None,
        echec_a: None,
    };
    // Le nom de chaque `.eml` garde se fait avant l'envoi : rien ne se refuse apres que les courriels sont partis.
    let iso = socle::paris::ecrit::seconde::texte(socle::temps::secondes());
    let horodate = NomDeFichier::lire(&format!("{}_{}{}{}", &iso[..10], &iso[11..13], &iso[14..16], &iso[17..19]))?;
    // Le nom d'un `.eml` garde : `<horodate>_CASC-SVP_<pour>.eml`.
    let casc = horodate.suivi_de(&const { NomDeFichier::du_moteur("_CASC-SVP_") });
    let emls: Vec<NomDeFichier> = messages.iter().map(|m| casc.suivi_de(&m.pour).avec_l_extension(&const { NomDeFichier::du_moteur("eml") })).collect();
    let mut copies: Vec<Vec<u8>> = Vec::new();
    let partis = match envoi::envoyer_suivi(&messages, &compte, client.as_mut(), None, Some(&mut copies)) {
        Ok(p) => p,
        Err(e) => {
            noter_echec_d_envoi(ctx, journal::Ligne::RegularisationCasc, None, qui, &e.partis, &e.echoue, &e.echec.phrase, detail, None)?;
            return Err(e.echec);
        }
    };

    // Ce que l'envoi garde : les deux messages tels qu'ils sont partis.
    let garde = dossier.join("envoyes");
    std::fs::create_dir_all(&garde).map_err(|e| socle::echec::du_disque(&garde, &e).reclasser(Classe::Disque))?;
    for (nom, brut) in emls.iter().zip(&copies) {
        let p = garde.join(nom);
        std::fs::write(&p, brut).map_err(|e| socle::echec::du_disque(&p, &e).reclasser(Classe::Disque))?;
    }
    detail.eml = emls.clone();
    noter(ctx, journal::Ligne::RegularisationCasc(par_courriel(None, qui, true, &en_liste(&partis, ", "), &format!("{} courriel(s) envoye(s)", partis.len()), detail, None)))?;
    // Les annees regularisees : leurs mois sortent des dossiers suivants. S'ecrit APRES le virement, jamais avant.
    let auj = socle::temps::secondes().a_la_seconde().jour_de_paris();
    let mut annees: Vec<RegularisationNotee> = Vec::new();
    for a in &r.annees {
        annees.push(marquer_regularise(ctx, a.annee, date_de(a.annee)?, a.cotisation, qui, auj)?);
    }
    let dites: Vec<String> = annees.iter().map(|a| a.annee.to_string()).collect();
    let notee = if dites.len() > 1 { "notées régularisées" } else { "notée régularisée" };
    Ok(CascEnvoyee {
        phrase: socle::phrase!("CASC-SVP, {} : {}, envoyé à {} ; {} {}.", r.periode, socle::texte::euros(r.total), en_liste(&partis, " et "), dites.join(" et "), notee),
        partis,
        expediteur: compte,
        periode: r.periode,
        total: r.total,
        annees,
        eml: emls,
        dossier: garde,
    })
}

/// Ce que rend `casc-envoyer` : sa phrase, les destinataires servis, la boite qui envoie, la periode et le total
/// regularises, les annees notees regularisees, les deux messages gardes et leur dossier.
#[derive(Debug, Serialize)]
struct CascEnvoyee {
    phrase: Phrase,
    partis: Vec<droit::fiche::Courriel>,
    expediteur: droit::fiche::Courriel,
    periode: String,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    total: socle::grandeurs::Montant,
    annees: Vec<RegularisationNotee>,
    eml: Vec<NomDeFichier>,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    dossier: PathBuf,
}

/// L'erreur du disque, sous sa classe (`FichierAbsent`, `Disque`).
fn erreur_io(e: &std::io::Error, p: &Path) -> Echec {
    erreur(Classe::du_disque(e), socle::fichiers::panne_fichier(e, p))
}

// ------------------------------------------------------------------ les droits des intermittents, l'archive d'un mois

charge! {
    /// La charge de `droits` : le mois du tableau (aucun : celui du jour).
    #[derive(Debug)]
    struct ChargeDroits {
        mois: Option<Mois> = dit("Mois", "le mois du tableau (le mois, l'année civile), AAAA-MM ; absent : celui du jour"),
    }
}

/// Une rangee du tableau du mois : ce qu'elle compte, et sa valeur dans chaque colonne.
#[derive(Debug, Serialize)]
struct Rangee {
    quoi: &'static str,
    valeurs: Vec<String>,
}

/// Le tableau du mois d'un intermittent : les titres des colonnes, et ses rangees.
#[derive(Debug, Serialize)]
struct TableauDesDroits {
    titres: Vec<String>,
    lignes: Vec<Rangee>,
}

/// Ce que `droits` dit d'un intermittent : son nom, sa fenetre, ses comptes (`[quoi, valeur]`), ou il en est, ses notes,
/// et le tableau du mois.
#[derive(Debug, Serialize)]
struct DroitsDUnSalarie {
    cle: droit::contexte::IdSalarie,
    nom: String,
    fenetre: String,
    lignes: Vec<(String, String)>,
    phrase: Phrase,
    notes: Vec<String>,
    colonnes: TableauDesDroits,
}

/// Ce que rend `droits` : le mois, ce que vaut ce compte, chaque intermittent.
#[derive(Debug, Serialize)]
struct Droits {
    phrase: Phrase,
    mois: Mois,
    phrases: Vec<String>,
    salaries: Vec<DroitsDUnSalarie>,
}

/// LES HEURES ET CACHETS DES INTERMITTENTS au regard des annexes 8 et 10 (`droit::cumul` : la fenetre, le plafond des
/// cachets d'un mois, l'enseignement plafonne, le seuil, lus du bareme) : pour chaque salarie dont un engagement est connu
/// (importe ou valide), sa position dans sa fenetre et le tableau du mois. Le compte ne porte que les engagements de cette
/// compagnie : la phrase le dit.
fn droits(ctx: &Contexte, c: ChargeDroits) -> R<Droits> {
    use droit::cumul::{anniversaire_configure, position_droits, tableau_cumul};
    // Le mois dit ; sans lui, celui du jour (la fiche le dit).
    let mois = match c.mois {
        Some(m) => m,
        None => ctx.aujourdhui.mois_civil(),
    };
    let engs = engagement::cumul::lire_engagements(ctx, None, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)?;
    let cles: BTreeSet<&droit::contexte::IdSalarie> = engs.iter().map(|e| &e.salarie).collect();
    let mut salaries = Vec::new();
    for cle in cles {
        let anniv = anniversaire_configure(ctx, cle)?;
        let p = position_droits(ctx, cle, &engs, anniv, None)?;
        let c = &p.constantes;
        use socle::grandeurs::ecrit::sans_zero::dit;
        let h = |n: socle::grandeurs::Heures| format!("{} h", dit(n));
        let mut lignes: Vec<(String, String)> = vec![("Cachets déclarés".into(), dit(p.cachets()))];
        if !p.cachets_perdus().est_nul() {
            lignes.push((format!("Retenus ({} par mois au plus)", dit(c.cachets_par_mois)), dit(p.cachets_retenus())));
        }
        lignes.push((format!("Convertis ({} h par cachet)", dit(c.heures_par_cachet)), h(p.cachets_retenus() * c.heures_par_cachet)));
        lignes.push(("Heures déclarées".into(), h(p.heures())));
        lignes.push(("Total d'heures équivalentes".into(), h(p.heures_retenues())));
        if !p.heures_enseignement().est_nul() {
            // Deux comptes des qu'il y a de l'enseignement : ces heures ouvrent les droits (plafonnees), sans augmenter l'allocation.
            lignes.push(("Dont enseignement".into(), h(p.heures_enseignement())));
            lignes.push((format!("Enseignement retenu (plafond {} h)", dit(p.plafond_enseignement()?)), h(p.heures_enseignement_retenues()?)));
            lignes.push(("Pour l'ouverture des droits".into(), h(p.heures_ouverture()?)));
            lignes.push(("Pour le montant de l'allocation".into(), h(p.heures_montant())));
        }
        lignes.push(("Seuil des annexes 8 et 10".into(), h(c.seuil_heures)));
        let jours = p.jours_restants(ctx.aujourdhui);
        if anniv.is_some() && !jours.nulle() {
            lignes.push(("Avant le réexamen".into(), jours.dite_en_abrege()));
        }
        let phrase = if p.atteint()? {
            socle::phrase!("Seuil atteint, avec {} h de marge.", dit(p.heures_ouverture()? - c.seuil_heures))
        } else {
            socle::phrase!("Il manque {} h, soit environ {} cachets.", dit(p.manque()?), dit(p.cachets_manquants()?))
        };
        let mut notes = Vec::new();
        if !p.cachets_perdus().est_nul() {
            notes.push(format!(
                "{} cachet(s) dépassent les {} d'un mois ({}) : joués, payés et déclarés, mais sans effet sur les droits.",
                dit(p.cachets_perdus()),
                dit(c.cachets_par_mois),
                p.mois_plafonnes().iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
            ));
        }
        if !p.enseignement_perdu()?.est_nul() {
            notes.push(format!("{} h d'enseignement dépassent le plafond : payées et déclarées, mais sans effet sur les droits.", dit(p.enseignement_perdu()?)));
        }
        if !p.heures_enseignement().est_nul() && p.date_naissance.is_none() {
            notes.push("Date de naissance absente de la fiche : le plafond bas de l'enseignement s'applique, peut-être à tort.".to_string());
        }
        if anniv.is_none() {
            notes.push("Fenêtre déduite du dernier contrat déclaré : la date anniversaire que France Travail notifie (anniversaire_droits, dans la fiche du salarié) prime sur ce calcul.".to_string());
        }
        let colonnes = tableau_cumul(ctx, cle, mois, &engs)?;
        let rangee = |quoi: &'static str, valeur: &dyn Fn(&droit::cumul::Colonne) -> String| Rangee { quoi, valeurs: colonnes.iter().map(valeur).collect() };
        let heures = h;
        let tableau = TableauDesDroits {
            titres: colonnes.iter().map(|x| format!("{} {}", x.titre, x.sous_titre).trim().to_string()).collect(),
            lignes: vec![
                rangee("Cachets", &|x| dit(x.cachets)),
                rangee("Heures déclarées", &|x| heures(x.heures)),
                rangee("Total d'heures", &|x| heures(x.heures_equivalentes)),
                rangee("Salaire brut", &|x| socle::texte::euros(x.brut)),
            ],
        };
        // Un engagement dont la fiche n'est plus la se dit par sa cle.
        let nom = match ctx.salaries.get(cle) {
            Some(f) => crate::nom_a_l_ecran(f, cle),
            None => cle.to_string(),
        };
        salaries.push(DroitsDUnSalarie { cle: cle.clone(), nom, fenetre: p.libelle_fenetre.clone(), lignes, phrase, notes, colonnes: tableau });
    }
    let phrase = Phrase::fixe(if salaries.is_empty() {
        "Aucun engagement d'intermittent n'est encore validé ni importé."
    } else {
        "Ce compte ne porte que les engagements de cette compagnie : le relevé de France Travail seul fait foi, et le seuil se compte sur tous les employeurs. Ces totaux sont un plancher."
    });
    let phrases = salaries.iter().map(|s| format!("{} ({}) : {}", s.nom, s.fenetre, s.phrase)).collect();
    Ok(Droits { phrase, mois, phrases, salaries })
}
