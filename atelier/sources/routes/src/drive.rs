//! LE DRIVE, VU D'UNE COMPAGNIE (Configuration › Le Drive) : ce que le compte Google lie porte sous `PuppetPaie/`
//! (`drive-etat` : chaque dossier, ce qu'il est, sa derniere sauvegarde, son volume), et en supprimer un
//! (`drive-supprimer` : avec le mot de passe du logiciel, comme un acte reel ; le dossier va a la corbeille de Google), et
//! prendre la main sur le dossier de cette compagnie (`drive-prendre-la-main`, `compagnie::sauvegarde::LaMain`).
//! La vue elle-meme est `compagnie::sur_le_drive` ; reprendre une compagnie du Drive se fait a l'accueil
//! (`accueil::drive`).

use serde::Serialize;

use compagnie::sur_le_drive::{DossierVu, GenreDuDossier};
use envois::drive::{Drive, IdDrive};
use socle::echec::{Classe, R};
use socle::ecran::{EtatDEcran, Pastille};

use crate::catalogue::{Acte, Affiche, Ecran, Fiche, Place, Refus, Route, OPERATION};
use crate::declaration::{charge, dit};
use crate::ecran::{EcranRendu, RangDeLEcran};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use crate::imprevus::GesteDeRoute;
use crate::{Encart, SansCharge};

/// Une sauvegarde plus recente que cela : un poste s'en sert peut-etre encore (Leandre, 24/11/2026, #594).
const RECENTE: std::time::Duration = std::time::Duration::from_secs(7 * 86_400);
/// Le geste, tel que la demande du mot de passe le dit.
const GESTE_SUPPRIMER: &str = "mettre un dossier du Drive à la corbeille";
const SUPPRIMER: Affiche = Affiche::new("Supprimer…");
/// Le geste, tel que la demande du mot de passe le dit.
const GESTE_MAIN: &str = "prendre la main sur le Drive de cette compagnie";
const PRENDRE_LA_MAIN: Affiche = Affiche::new("Prendre la main…");

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::DriveEtat,
        geste: "Dire chaque dossier du Drive lié, sa dernière sauvegarde, son volume.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "phrase, encarts, rangs : chaque dossier sous PuppetPaie (le dossier de cette compagnie, celui d'une autre, l'ancienne sauvegarde commune, un dossier d'une ancienne version), sa dernière sauvegarde, son volume, et le geste qui le supprime quand le mot de passe du logiciel est posé (sinon un encart dit pourquoi) ; ce qui ne se lit pas se dit en encart",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Rendu(Place::Drive),
    },
    Fiche {
        route: Route::DriveSupprimer,
        geste: "Mettre un dossier du Drive à la corbeille, avec le mot de passe du logiciel.",
        charge: &ChargeSupprimer::CHARGE,
        exemple: r#"{"dossier":"essai"}"#,
        rend: "dossier (son nom), phrase",
        refus: &[
            Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe du logiciel, tapé par un humain ; rien n'est supprimé" },
            Refus { classe: Classe::Refuse, corriger: "un dossier que drive-etat montre ; rien n'est supprimé" },
            OPERATION,
        ],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Engage,
        phrase: 200,
        ecran: Ecran::Geste(Place::Drive),
    },
    Fiche {
        route: Route::DrivePrendreLaMain,
        geste: "Prendre la main sur le Drive de la compagnie : ce poste seul y sauvegarde.",
        charge: &ChargePrendreLaMain::CHARGE,
        exemple: r#"{}"#,
        rend: "phrase ; quand un autre poste tient la main (ou qu'elle ne se lit pas), la compagnie revient d'abord du Drive : apres (rouvrir : la compagnie a changé, elle se relit), incidents (ce qui n'est pas redescendu, un fichier par ligne), a_retenter (pas tout revenu : la compagnie n'a pas bougé, la main n'est pas prise, le geste de nouveau continue) ; sinon ensuite : la sauvegarde qui suit, sa route (sauvegarde-maintenant) et sa charge, à appeler ensuite (absente pour une compagnie fictive)",
        refus: &[
            Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe du logiciel, tapé par un humain ; rien ne change" },
            Refus { classe: Classe::Refuse, corriger: "un envoi de cette compagnie en cours, quand la compagnie doit revenir du Drive : attendre qu'il soit parti ; rien ne change" },
            OPERATION,
        ],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 400,
        ecran: Ecran::Geste(Place::Drive),
    },
];

charge! {
    motdepasse = dit("Mot de passe du logiciel", "le mot de passe du logiciel, tapé dans la page ; par moteur route, le moteur le demande lui-même à un humain");
    /// La charge de `drive-supprimer` : le dossier, tel que `drive-etat` le designe. Le geste « Supprimer… » de chaque
    /// rang la porte telle quelle.
    #[derive(Debug, Clone, Serialize)]
    pub(crate) struct ChargeSupprimer {
        dossier: IdDrive = dit("Dossier du Drive", "l'identifiant du dossier, tel que drive-etat le rend dans la charge de son geste"),
    }
}

charge! {
    motdepasse = dit("Mot de passe du logiciel", "le mot de passe du logiciel, tapé dans la page ; par moteur route, le moteur le demande lui-même à un humain");
    /// La charge de `drive-prendre-la-main` : rien que le mot de passe du logiciel.
    #[derive(Debug)]
    pub(crate) struct ChargePrendreLaMain {}
}

/// CE QU'UN GESTE DE L'ECRAN DU DRIVE PORTE : la charge de sa route. Supprimer porte un dossier ; prendre la main, rien
/// (c'est le dossier de cette compagnie).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum GesteDuDrive {
    Supprimer(ChargeSupprimer),
    PrendreLaMain(SansCharge),
}

type EcranDuDrive = EcranRendu<GesteDuDrive>;

/// Ce que rend `drive-etat`.
#[derive(Debug, Serialize)]
struct DriveDit {
    phrase: Phrase,
    #[serde(flatten)]
    ecran: EcranDuDrive,
}

/// Ce que rend `drive-supprimer`.
#[derive(Debug, Serialize)]
struct DossierSupprime {
    phrase: Phrase,
    dossier: String,
}

impl Dite for DriveDit {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for DossierSupprime {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// Ce que rend `drive-prendre-la-main` : la phrase, la sauvegarde qui suit (`ensuite`, `SauvegardeQuiSuit`), ce que la
/// page fait ensuite (`apres` : `Rouvrir`, quand la compagnie est revenue du Drive), et ce que la recuperation n'a pas
/// fait redescendre (`incidents`, un fichier par ligne) ou laisse a retenter (`a_retenter`).
#[derive(Debug, Serialize)]
struct MainPrise {
    phrase: Phrase,
    #[serde(skip_serializing_if = "Option::is_none")]
    ensuite: Option<crate::veille::SauvegardeQuiSuit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apres: Option<crate::ecran::ApresLeGeste>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    incidents: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    a_retenter: Vec<String>,
}

impl MainPrise {
    fn dite(phrase: Phrase) -> MainPrise {
        MainPrise { phrase, ensuite: None, apres: None, incidents: Vec::new(), a_retenter: Vec::new() }
    }
}

impl Dite for MainPrise {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::DriveEtat => {
            let SansCharge {} = charge.lire()?;
            en_json(etat(porte.ctx)?)
        }
        Route::DriveSupprimer => {
            let c: ChargeSupprimer = charge.lire()?;
            en_json(supprimer(porte.ctx, &c.dossier, charge.mot_de_passe())?)
        }
        Route::DrivePrendreLaMain => {
            let ChargePrendreLaMain {} = charge.lire()?;
            en_json(prendre_la_main(&porte.ctx.racine, charge.mot_de_passe(), porte.options.lacher.as_ref())?)
        }
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

// ------------------------------------------------------------------ ce qu'un dossier dit

/// Ce qu'est le dossier `vu`, pour la compagnie dont l'identite est `la_sienne`.
fn ce_qu_il_est(vu: &DossierVu, la_sienne: Option<&compagnie::sauvegarde::IdentiteSurLeDrive>) -> &'static str {
    match &vu.genre {
        _ if compagnie::sur_le_drive::est_celui_de(vu, la_sienne) => "Le dossier de cette compagnie : sa sauvegarde et ses archives.",
        GenreDuDossier::Compagnie(_) => "Le dossier d'une autre compagnie de ce compte Google.",
        GenreDuDossier::AncienneSauvegarde => "L'ancienne sauvegarde, d'avant le dossier par compagnie : toutes les compagnies de ce compte Google y étaient mêlées.",
        GenreDuDossier::Ancien => "Un dossier d'une ancienne version du logiciel (une archive de mois).",
    }
}

/// La date de sa derniere sauvegarde, en mots.
pub(crate) fn derniere_dite(vu: &DossierVu) -> String {
    match vu.derniere {
        Some(d) => format!("Dernière sauvegarde le {}.", d.dit()),
        None => "La date de sa dernière sauvegarde n'est pas connue.".to_string(),
    }
}

/// Ce dossier a-t-il ete sauvegarde il y a moins de 7 jours (`RECENTE`) ?
fn recent(vu: &DossierVu, maintenant: socle::paris::Moment) -> bool {
    vu.derniere.is_some_and(|d| maintenant.depuis(d).is_none_or(|ecart| ecart < RECENTE))
}

/// CE QUE LA BOITE DE « SUPPRIMER » DIT AVANT : la date de la derniere sauvegarde, et l'avertissement d'une sauvegarde de
/// moins de 7 jours ou d'une date inconnue (un poste s'en sert peut-etre encore) ; puis ou va le dossier, et pour combien
/// de temps (« Files in the trash are automatically deleted after 30 days » :
/// `atelier/reference/sources/google-drive-api/google-drive-api-corbeille-20261124.txt`, « Move a file to the trash »).
fn avant_de_supprimer(vu: &DossierVu, le_sien: bool, maintenant: socle::paris::Moment) -> Vec<String> {
    let mut l = vec![derniere_dite(vu)];
    if le_sien {
        l.push("C'est le dossier de cette compagnie : sa prochaine sauvegarde renverra tout dans un dossier neuf.".to_string());
    } else if recent(vu, maintenant) {
        l.push("Elle date de moins de 7 jours : un poste s'en sert peut-être encore.".to_string());
    } else if vu.derniere.is_none() {
        l.push("Un poste d'une ancienne version s'en sert peut-être encore.".to_string());
    }
    l.push("Le dossier va à la corbeille de Google, avec tout ce qu'il porte : on l'en sort pendant 30 jours.".to_string());
    l
}

/// Qui tient la main sur le dossier de cette compagnie, en mots (`a_ce_poste` : ce poste).
fn main_dite(main: &Result<Option<compagnie::sauvegarde::LaMain>, socle::echec::Echec>, a_ce_poste: bool) -> String {
    match main {
        Ok(Some(m)) if a_ce_poste => format!("Ce poste tient la main depuis le {} : lui seul y sauvegarde.", m.depuis.jour_de_paris().jj_mm()),
        Ok(Some(m)) => format!("Un autre poste tient la main depuis le {} : ce poste n'y sauvegarde pas.", m.depuis.jour_de_paris().jj_mm()),
        Ok(None) => "Aucun poste n'en tient la main : la prochaine sauvegarde de ce poste la prend.".to_string(),
        Err(e) => format!("{} Ce poste n'y sauvegarde pas tant qu'il ne prend pas la main.", compagnie::sauvegarde::main_illisible_dite(e)),
    }
}

/// LE MOT DE PASSE QUE « PRENDRE LA MAIN » DEMANDE : celui du logiciel s'il est pose (`coffre::exiger_mot_de_passe`),
/// aucun sinon.
pub(crate) fn demande_de_la_main(racine: &std::path::Path) -> R<Option<compagnie::demande::DemandeDuMotDePasse>> {
    compagnie::coffre::Coffre::du_poste(racine).ouverture_exigee().and_then(|pose| pose.then(|| crate::coffre::demande(racine, GESTE_MAIN)).transpose())
}

/// LE GESTE « PRENDRE LA MAIN… » (`drive-prendre-la-main`), tel que l'ecran du Drive et le bandeau le rendent : `charge`,
/// la sienne dans l'ecran qui le porte ; `demande`, celle de `demande_de_la_main` ; `recupere` : un autre poste tient la
/// main, ou elle ne se lit pas, et le geste recupere d'abord le Drive (`a_recuperer`).
pub(crate) fn geste_prendre_la_main<C>(charge: C, demande: Option<compagnie::demande::DemandeDuMotDePasse>, recupere: bool) -> GesteDeRoute<C, SansCharge, SansCharge> {
    GesteDeRoute::avec_cases(Route::DrivePrendreLaMain, charge, PRENDRE_LA_MAIN, Vec::new(), SansCharge {}, SansCharge {})
        .disant(avant_de_prendre_la_main(recupere))
        .exigeant(demande)
        .principal()
}

/// CE QUE LA BOITE DE « PRENDRE LA MAIN » DIT AVANT : ce qu'il change pour l'autre poste ; puis, quand un autre poste
/// tient la main (ou qu'elle ne se lit pas), que la compagnie de ce poste repart de ce qu'il y a sur le Drive, en effacant
/// ce qui s'est fait ici, dans les mots de Leandre (25/11/2026 : « on lui dit des changements ont ete faits ailleurs que
/// sur ce poste. Prendre la main va effacer tous les changements effectues. Etes-vous sur ? ») ; sinon, que la sauvegarde
/// part aussitot.
fn avant_de_prendre_la_main(recupere: bool) -> Vec<String> {
    let mut l = vec!["Ce poste seul sauvegardera dans le dossier de cette compagnie ; un poste qui tenait la main n'y sauvegardera plus, et le dira.".to_string()];
    if recupere {
        l.push("Des changements ont été faits ailleurs que sur ce poste. Prendre la main va effacer tous les changements effectués sur ce poste. Êtes-vous sûr ?".to_string());
    } else {
        l.push("La sauvegarde de ce poste part aussitôt dans le dossier de cette compagnie.".to_string());
    }
    l
}

/// LA MAIN NOTEE SUR LE DRIVE DEMANDE-T-ELLE DE LE RECUPERER, pour ce poste (`ce_poste`) ? Un autre poste la tient, ou
/// elle ne se lit pas : ce qu'en dira l'index de la compagnie revenue (`SansLaMain`). Personne, ou ce poste : rien a
/// recuperer.
fn a_recuperer(avant: &Result<Option<compagnie::sauvegarde::LaMain>, socle::echec::Echec>, ce_poste: &compagnie::sauvegarde::PosteSurLeDrive) -> Option<compagnie::sauvegarde::SansLaMain> {
    match avant {
        Ok(Some(m)) if m.poste != *ce_poste => Some(compagnie::sauvegarde::SansLaMain::AutrePoste { depuis: m.depuis }),
        Ok(_) => None,
        Err(_) => Some(compagnie::sauvegarde::SansLaMain::Illisible),
    }
}

// ------------------------------------------------------------------ drive-prendre-la-main

/// PRENDRE LA MAIN SUR LE DRIVE DE CETTE COMPAGNIE (`LaMain`) : le mot de passe du logiciel s'il est pose
/// (`coffre::exiger_mot_de_passe`), puis le dossier de la compagnie (celui que sa sauvegarde trouve, ou un neuf).
/// UN AUTRE POSTE TIENT LA MAIN, OU ELLE NE SE LIT PAS (`a_recuperer`) : ce poste recupere d'abord ce qu'il y a sur le
/// Drive (`compagnie::recuperation`), et ce qu'il a ecrit sans la main s'efface (la boite l'a dit), puis prend la main, et la
/// page relit la compagnie revenue (`apres` : `Rouvrir`). Un envoi de cette compagnie en cours (`envois_en_fond`) se
/// refuse d'abord : la compagnie qu'il ecrit va etre remplacee. `lacher` (`LacherLaCompagnie`, que la fenetre seule pose)
/// lache ce qui tient la compagnie, une fois tout revenu. Pas tout revenu, ou les dossiers ne s'echangent pas : la
/// compagnie reste telle quelle, la main n'est pas prise, la phrase le dit, et le geste de nouveau continue. PERSONNE, OU
/// CE POSTE : le dossier recoit la main de ce poste, l'index ne dit plus qu'il ne l'a pas, et la sauvegarde suit
/// (`ensuite`). Sert aussi quand on s'est trompe, ou que le poste qui la tenait est perdu. Une compagnie fictive n'a rien
/// sur le Drive : rien ne change.
fn prendre_la_main(racine: &std::path::Path, mdp: Option<&socle::secret::Secret>, lacher: Option<&crate::enveloppe::LacherLaCompagnie>) -> R<MainPrise> {
    if compagnie::bridage::branchement(racine)?.joue() {
        return Ok(MainPrise::dite(Phrase::fixe("Compagnie fictive : rien ne part au Drive, il n'y a pas de main à prendre.")));
    }
    crate::coffre::exiger_mot_de_passe(racine, mdp, GESTE_MAIN)?;
    let mut client = crate::coffre::client_drive(racine).map_err(|e| e.echec)?;
    let identite = compagnie::sauvegarde::identite(racine)?;
    let nom = compagnie::installation::nom_de(racine)?;
    let connu = compagnie::sauvegarde::lire_index(racine).map_err(|e| e.echec)?.and_then(|i| i.compagnie);
    let main = compagnie::sauvegarde::LaMain::de_ce_poste(racine)?;
    // Le dossier que remplit la sauvegarde qui suit (`ensuite` : `sauvegarde-maintenant`, un passage ordinaire), sous la
    // racine du Drive de ses options.
    let racine_drive = crate::gestes::ouverture::Options::entiere().racine_drive;
    let (id, marque) = compagnie::sauvegarde::dossier_de_la_compagnie(&mut client, &racine_drive, &identite, &nom, connu.as_ref(), Some(&main)).map_err(|e| e.echec)?;
    // Qui la tenait, avant ce geste : ce que la phrase en dit.
    let avant = compagnie::sauvegarde::LaMain::de(marque.as_ref());
    if let Some(sans) = a_recuperer(&avant, &main.poste) {
        if crate::envois_en_fond::existants(racine)?.is_some_and(|e| e.en_vol()) {
            return Err(socle::echec::erreur(Classe::Refuse, "Un envoi de cette compagnie est en cours : attendez que le bandeau le dise parti, puis recommencez. Rien n'a changé."));
        }
        return recuperer_puis_prendre(racine, &mut client, &racine_drive, &id, &main, &identite, sans, lacher);
    }
    client.marquer(&id, &main.marque(&identite)).map_err(|e| e.echec)?;
    compagnie::sauvegarde::main_reprise(racine).map_err(|e| e.echec)?;
    // Hors de la recuperation, la main notee n'etait que celle de ce poste, ou aucune.
    let phrase = if matches!(avant, Ok(Some(_))) {
        Phrase::fixe("Ce poste tenait déjà la main sur le Drive de cette compagnie : sa sauvegarde suit.")
    } else {
        Phrase::fixe("Ce poste tient la main sur le Drive de cette compagnie, qu'aucun poste ne tenait : sa sauvegarde suit.")
    };
    Ok(MainPrise { ensuite: Some(crate::veille::SauvegardeQuiSuit::du_drive()), ..MainPrise::dite(phrase) })
}

/// RECUPERER LE DRIVE PUIS PRENDRE LA MAIN (`prendre_la_main`, quand un autre poste la tient ou qu'elle ne se lit pas) :
/// le dossier `id` revient dans la compagnie (`compagnie::recuperation::recuperer`) ; au bout, la main de ce poste s'y
/// note, et l'index de la compagnie revenue ne dit plus qu'il ne l'a pas. La main qui ne se note pas laisse l'index dire
/// `sans` : ce poste n'y sauvegarde pas, et le geste de nouveau la prend.
#[allow(clippy::too_many_arguments)]
fn recuperer_puis_prendre(
    racine: &std::path::Path,
    client: &mut envois::drive::Client,
    racine_drive: &str,
    id: &IdDrive,
    main: &compagnie::sauvegarde::LaMain,
    identite: &compagnie::sauvegarde::IdentiteSurLeDrive,
    sans: compagnie::sauvegarde::SansLaMain,
    lacher: Option<&crate::enveloppe::LacherLaCompagnie>,
) -> R<MainPrise> {
    use compagnie::recuperation::Recuperation;
    let qui = match &sans {
        compagnie::sauvegarde::SansLaMain::AutrePoste { depuis } => format!("le poste qui tenait la main depuis le {}", depuis.jour_de_paris().jj_mm()),
        compagnie::sauvegarde::SansLaMain::Illisible => "le dernier poste qui l'a sauvegardée".to_string(),
    };
    let vus = compagnie::sur_le_drive::voir(client, racine_drive).map_err(|e| e.echec)?;
    let vu = compagnie::sur_le_drive::le_dossier(vus, id, racine_drive).map_err(|e| e.echec)?;
    let (incidents, reste) = match compagnie::recuperation::recuperer(racine, &vu, racine_drive, client, sans, &|r| match lacher {
        Some(l) => l.lacher(r),
        // En ligne de commande, rien ne tient la compagnie.
        None => Ok(()),
    }).map_err(|e| e.echec)? {
        Recuperation::PasAuBout(r) => {
            let phrase = socle::phrase!(
                "Le Drive n'est pas tout revenu ({} fichier(s) à retenter) : la compagnie reste comme elle était, et la main n'est pas prise. « Prendre la main » de nouveau continue là où cela s'est arrêté.",
                r.a_retenter.len()
            );
            return Ok(MainPrise { incidents: r.incidents, a_retenter: r.a_retenter, ..MainPrise::dite(phrase) });
        }
        Recuperation::PasEchangee(e) => return Ok(MainPrise::dite(socle::phrase!("{} La main n'est pas prise.", e.phrase))),
        Recuperation::Echangee { incidents, reste } => (incidents, reste),
    };
    let mut phrase = match client.marquer(id, &main.marque(identite)).map_err(|e| e.echec).and_then(|()| compagnie::sauvegarde::main_reprise(racine).map_err(|e| e.echec)) {
        Ok(()) => format!("La compagnie est revenue telle que {} l'a sauvegardée, et ce poste tient maintenant la main : lui seul y sauvegarde.", qui),
        Err(e) => format!(
            "La compagnie est revenue telle que {} l'a sauvegardée, mais la main ne se prend pas ({}) : ce poste ne la sauvegarde pas, et « Prendre la main » de nouveau la prend.",
            qui, e.phrase
        ),
    };
    if let Some(r) = reste {
        phrase = format!("{} {}", phrase, r);
    }
    Ok(MainPrise { apres: Some(crate::ecran::ApresLeGeste::Rouvrir), incidents, ..MainPrise::dite(socle::phrase!("{}", phrase)) })
}

// ------------------------------------------------------------------ drive-etat

/// L'ECRAN DU DRIVE : un rang par dossier sous `PuppetPaie/`, et son geste « Supprimer… » quand le mot de passe du
/// logiciel est pose (sa demande ; sans lui, le geste ne se propose pas et un encart dit pourquoi). Doux : une compagnie
/// fictive, un Drive non lie, un Drive qui ne se lit pas, une identite ou un coffre illisibles se disent en encart, et
/// rien ne se refuse.
fn etat(ctx: &droit::contexte::Contexte) -> R<DriveDit> {
    let mut encarts = vec![Encart::de(EtatDEcran::Information, "", "Ce que votre compte Google porte dans son dossier PuppetPaie : un dossier par compagnie, et ce qu'ont laissé les anciennes versions.")];
    let fini = |encarts: Vec<Encart>, phrase: Phrase| DriveDit { phrase, ecran: EcranRendu { encarts, rangs: Vec::new(), cases: Vec::new(), historique: Vec::new(), enregistrer: None } };
    if compagnie::bridage::branchement(&ctx.racine)?.joue() {
        encarts.push(Encart::de(EtatDEcran::Information, "Compagnie fictive : rien ne part au Drive", "Sa sauvegarde et ses archives sont jouées."));
        return Ok(fini(encarts, Phrase::fixe("Compagnie fictive : rien ne part au Drive.")));
    }
    let compte = match crate::coffre::compte_autorise(&ctx.racine) {
        Ok(Some(c)) => c,
        Ok(None) => {
            encarts.push(Encart::de(EtatDEcran::AFaire, "Aucun compte Google lié", "Lier le Drive : Configuration › Mots de passe."));
            return Ok(fini(encarts, Phrase::fixe("Aucun compte Google lié.")));
        }
        // Un jeton illisible n'est pas « aucun compte » : il se dit, et se relie.
        Err(_) => {
            encarts.push(Encart::de(EtatDEcran::Refuse, "Le jeton du compte Google ne se lit pas", "Relier le Drive de nouveau : Configuration › Mots de passe."));
            return Ok(fini(encarts, Phrase::fixe("Le jeton du compte Google ne se lit pas.")));
        }
    };
    let vus = crate::coffre::client_drive(&ctx.racine).and_then(|mut c| compagnie::sur_le_drive::voir(&mut c, compagnie::sauvegarde::RACINE_DRIVE));
    let vus = match vus {
        Ok(v) => v,
        Err(e) => {
            encarts.push(Encart::de(EtatDEcran::Refuse, "Le Drive ne se lit pas", e.echec.phrase));
            return Ok(fini(encarts, socle::phrase!("Le Drive de {} ne se lit pas.", compte)));
        }
    };
    // Ce qui ne se lit pas ici se dit en encart ; le reste de l'ecran se montre.
    let la_sienne = match compagnie::sauvegarde::identite_ecrite(&ctx.racine) {
        Ok(i) => i,
        Err(e) => {
            encarts.push(Encart::de(EtatDEcran::Refuse, "Le dossier de cette compagnie ne se reconnaît pas", e.phrase));
            None
        }
    };
    // SUPPRIMER DEMANDE LE MOT DE PASSE DU LOGICIEL : sans lui, le geste ne se propose pas, et l'ecran dit pourquoi.
    let demande = match compagnie::coffre::Coffre::du_poste(&ctx.racine).ouverture_exigee().and_then(|pose| pose.then(|| crate::coffre::demande(&ctx.racine, GESTE_SUPPRIMER)).transpose()) {
        Ok(Some(d)) => Some(d),
        Ok(None) => {
            encarts.push(Encart::de(
                EtatDEcran::Information,
                "Supprimer un dossier demande le mot de passe du logiciel",
                "Aucun n'est posé pour cette compagnie : le poser d'abord, dans Configuration › Mots de passe.",
            ));
            None
        }
        Err(e) => {
            encarts.push(Encart::de(EtatDEcran::Refuse, "Le mot de passe du logiciel ne se lit pas : supprimer un dossier attend", e.phrase));
            None
        }
    };
    // Qu'il ne se lise pas, et le geste qui prend la main ne se propose pas (l'encart d'au-dessus le dit deja).
    let demande_main = demande_de_la_main(&ctx.racine);
    // Ce poste, tel que la main le nomme : qu'il ne se lise pas, et la main ne se dit pas.
    let ce_poste = match compagnie::sauvegarde::PosteSurLeDrive::de_ce_poste(&ctx.racine) {
        Ok(p) => Some(p),
        Err(e) => {
            encarts.push(Encart::de(EtatDEcran::Refuse, "Ce poste ne se nomme pas sur le Drive", e.phrase));
            None
        }
    };
    let maintenant = socle::temps::secondes();
    if vus.is_empty() {
        encarts.push(Encart::de(EtatDEcran::Information, "Rien encore dans le Drive", "La prochaine sauvegarde y crée le dossier de cette compagnie."));
    }
    let mut rangs = Vec::new();
    for vu in &vus {
        let le_sien = compagnie::sur_le_drive::est_celui_de(vu, la_sienne.as_ref());
        let mut dit = vec![ce_qu_il_est(vu, la_sienne.as_ref()).to_string(), derniere_dite(vu)];
        if let Some(o) = vu.octets {
            dit.push(format!("Volume : {}.", o));
        }
        dit.extend(vu.illisible.iter().cloned());
        let pastille = if le_sien {
            Pastille::de(EtatDEcran::Fait, "cette compagnie")
        } else if recent(vu, maintenant) {
            Pastille::de(EtatDEcran::EnAttente, "sauvegardé il y a moins de 7 jours")
        } else {
            Pastille::de(EtatDEcran::Information, "ancien ou d'une autre compagnie")
        };
        let mut gestes = Vec::new();
        // LA MAIN, sur le dossier de cette compagnie : qui la tient, et le geste qui la prend quand ce n'est pas ce poste.
        // Une main illisible sur un autre dossier se dit comme le reste de ce qui ne s'y lit pas.
        if let (true, Some(p)) = (le_sien, &ce_poste) {
            let a_ce_poste = vu.main.as_ref().is_ok_and(|m| m.as_ref().is_some_and(|m| m.poste == *p));
            dit.push(main_dite(&vu.main, a_ce_poste));
            // Une recuperation du Drive commencee et pas finie se voit ; « Prendre la main » la continue.
            match compagnie::recuperation::commencee(&ctx.racine) {
                Ok(true) => dit.push("Une récupération de ce dossier, commencée sur ce poste, n'est pas allée au bout : « Prendre la main » la continue.".to_string()),
                Ok(false) => {}
                Err(e) => dit.push(e.phrase),
            }
            if let (false, Ok(d)) = (a_ce_poste, &demande_main) {
                gestes.push(geste_prendre_la_main(GesteDuDrive::PrendreLaMain(SansCharge {}), d.clone(), a_recuperer(&vu.main, p).is_some()));
            }
        } else if let Err(e) = &vu.main {
            dit.push(compagnie::sauvegarde::main_illisible_dite(e));
        }
        if let Some(d) = &demande {
            gestes.push(
                GesteDeRoute::avec_cases(Route::DriveSupprimer, GesteDuDrive::Supprimer(ChargeSupprimer { dossier: vu.id.clone() }), SUPPRIMER, Vec::new(), SansCharge {}, SansCharge {})
                    .disant(avant_de_supprimer(vu, le_sien, maintenant))
                    .exigeant(Some(d.clone())),
            );
        }
        rangs.push(RangDeLEcran { titre: vu.nom.clone(), dit, pastille, etat: None, montre: None, gestes });
    }
    let phrase = socle::phrase!("Le Drive de {} : {} dossier(s).", compte, vus.len());
    Ok(DriveDit { phrase, ecran: EcranRendu { encarts, rangs, cases: Vec::new(), historique: Vec::new(), enregistrer: None } })
}

// ------------------------------------------------------------------ drive-supprimer

/// SUPPRIMER UN DOSSIER DU DRIVE : la corbeille ne s'obtient qu'avec le mot de passe du logiciel, comme un acte reel
/// (`coffre::corbeille_du_drive`), puis le dossier, s'il est sous la racine du Drive lie (relue), va a la corbeille de
/// Google. Une compagnie fictive n'a rien sur le Drive : rien ne se supprime, et la phrase le dit.
fn supprimer(ctx: &droit::contexte::Contexte, dossier: &IdDrive, mdp: Option<&socle::secret::Secret>) -> R<DossierSupprime> {
    if compagnie::bridage::branchement(&ctx.racine)?.joue() {
        return Ok(DossierSupprime { phrase: Phrase::fixe("Compagnie fictive : rien n'est sur le Drive, rien n'est supprimé."), dossier: dossier.to_string() });
    }
    let mut corbeille = crate::coffre::corbeille_du_drive(ctx, mdp, GESTE_SUPPRIMER)?;
    let racine_drive = compagnie::sauvegarde::RACINE_DRIVE;
    let vus = compagnie::sur_le_drive::voir(corbeille.drive(), racine_drive).map_err(|e| e.echec)?;
    let vu = compagnie::sur_le_drive::le_dossier(vus, dossier, racine_drive).map_err(|e| e.echec)?;
    corbeille.mettre(&vu.id).map_err(|e| e.echec)?;
    Ok(DossierSupprime { phrase: socle::phrase!("« {} » est dans la corbeille de Google : on l'en sort pendant 30 jours.", vu.nom), dossier: vu.nom })
}
