//! Les lectures que les ecrans partagent : `etat` (tout le JSON que la page lit en s'ouvrant -- la configuration
//! verifiee, l'historique des mois, les listes ou l'on choisit, les referentiels livres avec le logiciel), `configuration`,
//! `maj-etat` et `courriels-fictifs`.
//!
//! AUCUN CALCUL DE PAIE, et aucun secret lu : la boite courriel se dit par un
//! booleen (l'adresse posee et le secret present dans l'environnement ou le
//! coffre, jamais sa valeur), la compagnie fictive par son marqueur.

use droit::fiche::Lue as _;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};


use crate::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, OPERATION};
use crate::catalogue::Route;
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use crate::etapes::Moteur;
use compagnie::etat_dsn::Etat;
use declarations::champs;
use droit::fiche::{CaseDeFiche, CaseEntreprise, CaseEtablissement, CaseSalarie, CaseSpectacle, FicheSaisie as _, MotEcrit, NomDeCase, NomDeFiche, ValeurDeCase};
use droit::autre_revenu_brut::AutreElementDeRevenuBrut;
use droit::catalogue::{self, Rubrique};
use droit::contexte::{Contexte, IdSalarie};
use droit::emplois::{self};
use droit::natures::{self, MotifDeRupture};
use engagement::{self};
use socle::echec::{erreur, Classe, R};
use socle::fichiers::NomDeFichier;
use socle::mois::Mois;
use crate::catalogue::{Ecran, Place};
use crate::fiches::{anomalies_enregistrees, comblee, SorteDeFiche};
use crate::temps::TempsDeLaCase;

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Lecture;

/// CE QUE LA CARTE D'UN ENGAGEMENT DIT DE SES CASES (`aides` de la route `etat`) : l'aide de chaque case, qu'elle montre
/// au survol ou sous la case ; la page n'en ecrit aucune. Les articles cites sont ceux du code du travail, sauf mention.
const AIDES: &[(&str, &str)] = &[
    ("heures_par_mois", "La durée du travail du contrat, en heures par mois (S21.G00.40.013)."),
    ("net", "Ce que le salarié touche sur son compte."),
    ("cout", "Le brut et les cotisations patronales : ce que l'employeur sort."),
    ("viser", "Écrire dans une case : les taux se recalculent pour y tomber."),
    ("metier", "Le métier de ce contrat ; une carte neuve prend celui du dernier contrat du salarié."),
    ("recours", "Pourquoi ce CDD est conclu : la DSN le déclare (motif de recours)."),
    ("periode_essai_jours", "La durée de la période d'essai que le contrat prévoit ; vide : aucune."),
    ("personne_remplacee", "Le contrat d'un remplacement nomme la personne remplacée et sa qualification (L. 1242-12, 1°)."),
    ("repartition_horaire", "À temps partiel, le contrat dit la répartition des heures entre les jours de la semaine ou les semaines du mois (L. 3123-6) : par exemple, lundi, mardi et jeudi, de 9 h à 17 h."),
    ("clauses_particulieres", "Les clauses propres à ce contrat : le paragraphe « Clauses particulières » du modèle les imprime ; vide : il se tait."),
    ("modele_contrat", "Le modèle que suit le contrat de cet engagement ; vide : le modèle par défaut de sa nature (Configuration › Modèles de contrat)."),
    ("engagement_licenciement", "La convocation à l'entretien préalable (L1232-2 ; L1332-2 pour une faute)."),
    ("signature_convention", "La date de signature de la convention de rupture (L1237-13)."),
    ("dernier_paye", "Le dernier jour payé au salaire habituel : la fin du contrat, sauf préavis non effectué."),
    ("en_droit_a_la_retraite", "CGI, art. 80 duodecies, 6° : l'indemnité n'est imposable que si le salarié est en droit de bénéficier d'une pension de retraite d'un régime obligatoire."),
    ("preavis_non_effectue", "L1234-5 : le préavis dont l'employeur dispense le salarié est payé en indemnité compensatrice."),
    ("preavis_mois", "La durée du préavis, en mois ; vide : celle de la loi et de la convention."),
    ("precarite_due", "L1243-10 : deux cas ne se lisent pas dans le contrat, l'étudiant pendant ses vacances et le salarié qui refuse un CDI ; dire lequel."),
    ("date_paiement", "Le jour du virement de ce contrat, s'il diffère de celui de la configuration."),
    ("lieu", "Le lieu que le contrat imprime quand une ligne de paie n'en dit pas."),
    ("jours", "Les jours travaillés que la DSN déclare : ceux que les jours cochés disent, pour un CDDU."),
    ("quotite_reference", "La durée de référence de l'entreprise pour ce contrat (S21.G00.40.012)."),
    ("indemnite_licenciement", "Le montant retenu, entre l'indemnité légale et la conventionnelle."),
    ("faute_grave", "Une fin de période d'essai pour faute grave : pas d'indemnité du délai de prévenance (L1221-25)."),
    ("bornes_au_mois", "Un contrat payé au mois n'a pas de jour coché : ses bornes se disent ici. La date de fin d'un CDD est celle qui déclenche le signalement de fin de contrat (nature 07) : son délai et sa date butoir se lisent sous « Fin de contrat »."),
    ("bornes_au_calendrier", "Le contrat va du premier au dernier jour coché, planning compris, quand on a coché des jours dans les mois suivants depuis le calendrier d'une ligne. Ses cachets suivants se paient dans le mois suivant (le mois venu, le contrat y est repris, ses jours cochés) ; la DPAE et le contrat imprimé ne partent qu'avec ce mois-ci, le certificat Congés Spectacles à la fin du contrat."),
    ("fin_signalee", "Ce que le signalement de fin de contrat (nature 07) déclarera."),
    ("fin_derogatoire", "Elle part dans la DSN du mois, par le circuit dérogatoire : rien à signaler."),
    ("numero_contrat", "Attribué à la production, il ne change plus."),
    ("numero", "Le numéro sous lequel votre ancien logiciel a déjà déclaré ce contrat en DSN ; vide : le logiciel en donne un."),
    ("debut_au_mois", "Sa date de début, celle du contrat signé."),
    ("bornes_jouees", "Du premier au dernier jour joué."),
    ("plusieurs_mois", "Les cachets des mois suivants se paieront dans le fichier de leur mois (« reprendre un contrat en cours » les retrouvera cochés) ; ce mois-ci partent le contrat et la DPAE pour le contrat entier."),
    ("imprevu_valide", "L'engagement est validé : cliquer sur sa carte pour l'ouvrir, retirer ces jours, puis « Modifier l'engagement »."),
    ("jour_hors_ligne", "Ce jour n'est sur aucune ligne ouverte de cette carte : le décocher au calendrier."),
    ("a_dire_avant_signaler", "À dire dans sa carte, sous « Contrat », avant de signaler :"),
    ("absence", "Saisie une fois : chaque mois qu'elle traverse la déclare et, si elle n'est pas payée, en retire le salaire."),
    ("jours_deja_indemnises", "Les jours d'arrêt déjà indemnisés dans les douze mois : ils réduisent la durée du maintien."),
    ("indemnites_journalieres", "Le montant d'un jour, tel que l'écrit le relevé de la CPAM ou son bordereau de paiement : brut, avant CSG et CRDS, puis net. Le maintien de salaire s'en déduit."),
    ("jours_payes", "Des jours sont payés pendant l'imprévu : cocher ceux à retirer de l'engagement."),
    ("signalement_dsn", "Le logiciel écrit le fichier du signalement ; le dépôt, sur net-entreprises, est l'étape suivante. La DSN du mois le répétera : l'une ne remplace pas l'autre."),
    ("deja_signale", "Un nouveau signalement ne remplace rien : pour corriger ce qui est parti, un annule-et-remplace vise l'identifiant rendu par le point de dépôt (sur le certificat de conformité)."),
    ("signalement_fin", "Il va à France Travail, qui en tire l'attestation employeur : c'est elle qui ouvre les droits du salarié. La DSN du mois ne le remplace pas."),
    ("identifiant_remplace", "JJMMAAAA suivi du rang de l'envoi, lu sur son certificat de conformité ; vide pour un signalement normal."),
    ("reference_faite", "L'accusé de réception, le numéro du recommandé, s'il y en a un."),
    ("hors_dsn", "Elle ne part pas avec la DSN."),
    ("date_accident", "La date de l'accident, ou de sa première constatation."),
    ("debut_arret", "Le premier jour que l'avis prescrit ; prescrit un jour travaillé, ce jour-là, aussi le dernier jour travaillé."),
    ("prolongation", "La nouvelle fin prescrite, celle de l'avis de prolongation."),
    ("note_imprevu", "Une note pour vous : elle ne part nulle part."),
    ("cloture", "Les saisies et les sorties de l'année ne se modifieront plus (rouvrir reste possible, depuis l'Historique). Tout ce qu'il faut garder (saisies, bulletins, contrats signés, DPAE, DSN, certificats, retours des organismes, journal des dépôts, fiches, export compta) est réuni dans l'archive de l'exercice."),
    ("cloture_refus", "La clôture vient après la dernière DSN de l'année : elle est refusée si un mois avec des paies n'a pas de DSN acceptée au journal."),
    ("cloture_forcer", "Si une DSN manque au journal (déposée ailleurs, par exemple)."),
    ("signature_lourde", "Trop lourde : une image collée pèse tout son poids, préférez une image en ligne."),
    ("urssaf_changer", "D'après l'adresse de l'entreprise, vos déclarations vont à l'Urssaf de son code postal : c'est elle qui reçoit normalement DPAE et DSN. Une Urssaf qui ne vous gère pas peut faire rejeter vos déclarations. Voulez-vous vraiment la changer ?"),
    ("historique_titre", "Historique des valeurs"),
    ("historique_fiche", "Toutes les valeurs de chaque case, passées et à venir, avec leurs dates et d'où elles viennent ; en gras, celle d'aujourd'hui. Celles d'avant restent pour les déclarations déjà envoyées."),
    ("non_enregistres", "Vos changements ne sont pas encore enregistrés."),
    ("non_enregistres_titre", "Changements non enregistrés"),
    ("garder_changements", "Enregistrer les changements"),
    ("jeter_changements", "Annuler les changements"),
    ("fiche_annuler", "Annuler"),
    ("fiche_enregistrer", "Enregistrer"),
    ("fiche_modifiee", "modifications non enregistrées"),
    ("dater_titre", "Ce qui change"),
    ("dater_metier", "Métier"),
    ("dater_depuis", "À partir du"),
    ("dater_note", "ajouter une note"),
    ("dater_motif", "motif"),
    ("valeur_vide", "vide"),
    ("valeur_effacee", "effacé"),
    ("refus_titre", "Enregistrement refusé"),
    ("refus_corriger", "Corriger"),
    ("refus_forcer", "Enregistrer quand même"),
    ("cout_attente", "Coût…"),
    ("dater", "Ce que vous changez vaut à partir d'une date : la voici proposée, changez-la si besoin, puis enregistrez."),
    ("dater_avant", "Avant cette date, la fiche garde ce qu'elle disait."),
    ("rien_enregistre", "Rien n'est enregistré : vos saisies restent dans la fiche, à enregistrer quand vous voulez."),
    ("importes", "Engagements de l'ancien logiciel : rien ne se calcule ici."),
    ("apercu_vide", "Ajouter un engagement, une ligne de paie et ses jours joués."),
    ("apercu_vide_permanents", "Ajouter un engagement, une ligne de paie et ses jours joués, ou, pour un permanent, son salaire mensuel."),
    ("sans_cachet", "Pas de bulletin, la DSN porte le contrat."),
    ("net_attend", "Le net s'affiche dès que l'engagement est complet."),
    ("qui_d_abord", "Choisir d'abord qui est payé."),
    ("lieu_jours", "Le lieu et les jours se posent sur chaque ligne de paie."),
    ("importe_manque", "Un clic sur la carte montre ses cases, et sous chacune pourquoi."),
    ("importe_dates", "Début et fin du contrat : les jours exacts ne sont pas lus."),
    ("importe_brut", "Le brut : le net à payer n'est pas lu."),
    ("a_completer", "Un engagement au moins reste à compléter avant de le valider : sa carte dit ce qui manque."),
    ("piece_absente", "N'est plus dans le dossier du mois."),
    ("qui_engage", "Choisir qui est engagé."),
    ("engagement_retire", "Engagement retiré."),
    ("dpae_cochees", "Cochées d'avance : les embauches sans DPAE, ou dont la DPAE a été refusée. Une DPAE déjà passée ne se redéclare pas : l'Urssaf recevrait deux déclarations pour un seul contrat."),
    ("dpae_heure", "Heure d'embauche déclarée : 08h00. Toutes les embauches cochées partent ensemble, dans un seul dépôt. D'abord le test, qui ne déclare rien ; puis, une fois le test revenu conforme, l'envoi réel."),
    ("revue_spectacles", "Décochés, ils s'archivent : ils restent lisibles et se réactivent d'un clic (Configuration › Spectacles). Ceux qui continuent gardent leur numéro d'objet."),
    ("mot_de_passe", "Le mot de passe du logiciel ne sert qu'à prouver que quelqu'un est là. Les mots de passe des organismes et de la boîte restent dans le coffre."),
    ("import_natures", "Un bulletin de paie ne dit pas si le contrat est un CDI ou un CDD ; la DSN déposée du même mois pourrait le dire. Sinon, dites-le pour chacun : la DSN et les documents de fin de contrat en dépendent."),
    ("import_numero", "Le numéro du contrat : la rubrique S21.G00.40.009 de sa dernière DSN."),
    ("import_quotite", "La durée du travail, en heures par mois : la rubrique S21.G00.40.013 de la DSN."),
    ("import_ecarts", "Le bulletin fait foi : c'est son montant qui s'écrit, l'écart reste noté ici."),
    ("import_copies", "Les fichiers lus sont copiés dans l'historique de la compagnie ; gardez-les aussi dans le dossier d'import : une version suivante du logiciel pourra en lire davantage."),
    ("import_rien", "Rien à reprendre : tout est déjà là."),
    ("import_avant_clic", "Rien n'est écrit avant ce clic."),
];

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Etat,
        geste: "Dire tout ce que la page lit en s'ouvrant : configuration, Historique, listes.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "configuration (les fiches vérifiées ; pour_payer : ce qui manque encore pour payer, ou null), historique (les mois), les listes où l'on choisit, les référentiels (niveaux_sante : null tant que le barème ne se compose pas), auteur (phrase, nom, adresse)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Configuration,
        geste: "Dire les fiches de la compagnie, vérifiées.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "entreprise, salaries, spectacles, leurs anomalies, l'identité, la description des cases, pour_payer (ce qui manque encore pour payer, ou null)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::MajEtat,
        geste: "Dire la version du logiciel.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "la version installée et ce que la mise à jour en sait, phrase",
        refus: &[OPERATION],
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Auto(Place::Fiches, Affiche::new("Dire la version")),
    },
    Fiche {
        route: Route::CourrielsFictifs,
        geste: "Dire les courriels joués d'une compagnie fictive.",
        charge: &[Champ { nom: "apres", libelle: Affiche::new("Depuis"), forme: Forme::Texte, sens: "seulement ceux d'après cet instant", avance: true }],
        exemple: r#"{}"#,
        rend: "les courriels joués (à qui, quoi, quand)",
        refus: &[Refus { classe: Classe::Refuse, corriger: "rien : seulement dans une compagnie fictive" }],
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::LectureAvancee,
    },
];

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::Etat => {
            let super::SansCharge {} = charge.lire()?;
            en_json(etat(ctx)?)
        }
        // La part « configuration » de l'etat, pour les routes qui la rendent apres un enregistrement.
        Route::Configuration => {
            let super::SansCharge {} = charge.lire()?;
            en_json(configuration_dite(ctx)?)
        }
        Route::MajEtat => {
            let super::SansCharge {} = charge.lire()?;
            en_json(version()?)
        }
        Route::CourrielsFictifs => en_json(courriels_fictifs(ctx, &charge.lire()?)?),
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

macro_rules! dites {
    ($($t:ty),*) => {
        $(impl Dite for $t {
            fn phrase(&self) -> &Phrase {
                &self.phrase
            }
        })*
    };
}
dites!(EtatDeLaPage, ConfigurationDite, Version, CourrielsFictifs);

// ------------------------------------------------------------ la configuration

/// La pastille d'une fiche, et la raison de ce qui est a corriger ou a verifier, qui se lit au survol.
#[derive(Debug, Clone, serde::Serialize)]
struct PastilleDeFiche {
    #[serde(flatten)]
    pastille: socle::ecran::Pastille,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

/// UNE FICHE TELLE QUE L'ECRAN LA MONTRE (entreprise, salarie, spectacle) : ce que sa ligne et son en-tete disent, ses
/// cases comblees (`donnees`, par le nom que `champs-base.json` leur donne, avec leur `historique`), ce qu'elle declare,
/// ses anomalies, sa pastille, et les cases que la page en montre (`fiches::montres`).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct VueFiche<P, K> {
    quoi_dit: String,
    geste_dit: &'static str,
    matricule_dit: Option<String>,
    erreurs_titre: Option<String>,
    erreurs_note: &'static str,
    nom: NomDeFiche,
    cle: Option<K>,
    donnees: DonneesDeLaFiche<P>,
    declare: champs::Apercu,
    anomalies: Vec<champs::Anomalie>,
    pastille: PastilleDeFiche,
    sans_titre: bool,
    montres: Vec<CaseDeFiche>,
    /// Les pieces que les engagements de la fiche ouvrent (`fiches::Montres::pieces_ouvertes`) : la page n'ouvre le tiroir
    /// d'une piece que sur elles.
    pieces_ouvertes: Vec<droit::fiche::Piece>,
    /// Les cases du tiroir « Avancé » qui s'ecartent du cas ordinaire (`champs::Champ::ecart`) : le tiroir s'ouvre pour
    /// elles.
    ecarts: Vec<CaseDeFiche>,
}

impl<P, K> VueFiche<P, K> {
    /// Ses erreurs (gravite « erreur »), celles que le bandeau compte.
    fn erreurs(&self) -> usize {
        self.anomalies.iter().filter(|a| a.gravite == champs::Gravite::Erreur).count()
    }
}

/// LES CASES D'UNE FICHE TELLES QUE L'ECRAN LES MONTRE : la fiche a un jour, sans ses notes, et les temps de chaque case
/// datee qui a plus d'une valeur.
#[derive(Debug, Clone, serde::Serialize)]
struct DonneesDeLaFiche<P> {
    #[serde(flatten)]
    fiche: P,
    historique: Vec<TempsDeLaCase<ValeurDeCase>>,
}

/// LES TEMPS DE CHAQUE CASE DATEE de la fiche `brute` a `jour` (`droit::fiche::temps_au`), chaque valeur ecrite comme le
/// fichier l'ecrit (`ValeurDeCase`) ; une case qui n'a qu'une valeur n'en a pas. La page les met en page.
pub(crate) fn temps_de<F: SorteDeFiche>(brute: Option<&F>, jour: &socle::date::Jour) -> R<Vec<TempsDeLaCase<ValeurDeCase>>> {
    struct Toutes {
        listes: Vec<(CaseDeFiche, R<Vec<droit::fiche::Datee<ValeurDeCase>>>)>,
    }
    impl droit::fiche::VoirListe for Toutes {
        fn liste<T: socle::ecriture::EcritureDeCase>(&mut self, cle: CaseDeFiche, liste: &[droit::fiche::Datee<T>]) {
            let lue = liste.iter().map(|e| Ok(droit::fiche::Datee { depuis: e.depuis, valeur: ValeurDeCase::de(&socle::ecriture::CaseEcrite(&e.valeur)), note: e.note.clone() })).collect();
            self.listes.push((cle, lue));
        }
    }
    // Une fiche absente n'a aucune case datee.
    let mut t = Toutes { listes: Vec::new() };
    if let Some(brute) = brute {
        brute.voir_listes(&mut t);
    }
    let mut out = Vec::new();
    for (champ, lue) in t.listes {
        // Les autres etablissements sont une liste : leurs temps se disent case par case, ligne par ligne
        // (`temps_des_etablissements`).
        if champ == CaseEntreprise::Etablissements.into() {
            continue;
        }
        if let Some(temps) = crate::temps::d_une_case(crate::temps::CaseDatee::Case(champ), &lue?, jour, |a, b| a == b)? {
            out.push(temps);
        }
    }
    Ok(out)
}

/// LES TEMPS DES CASES DE CHAQUE AUTRE ETABLISSEMENT a `jour`, rangees par NIC, sous la case `etablissements` : la case de
/// chacun se lit dans les versions datees de la liste (sa valeur a chaque version ou il figure, une fois tant qu'elle ne
/// change pas, avec la note de la version qui l'a changee). Son taux AT/MP a sa case a lui, que `taux-atmp` dit
/// (`atmp::case_d_un_etablissement`).
fn temps_des_etablissements(ctx: &Contexte, jour: &socle::date::Jour) -> R<Option<TempsDeLaCase<ValeurDeCase>>> {
    type Cases = Vec<(CaseDeFiche, Vec<droit::fiche::Datee<ValeurDeCase>>)>;
    let Some(liste) = ctx.entreprise_brute().etablissements.as_deref() else { return Ok(None) };
    let principal = ctx.entreprise.nic_principal();
    let declarees = <droit::fiche::Etablissement as droit::fiche::FormeDeCase>::cases();
    let mut par_nic: Vec<(droit::fiche::Nic, Cases)> = Vec::new();
    for version in droit::fiche::valeurs_datees(liste) {
        // Chaque etablissement de la liste, dans son ordre : le NIC se lit dans l'etablissement, ses cases ecrites a cote.
        for e in version.valeur.iter().flatten() {
            let Some(nic) = e.nic.dite().filter(|n| Some(*n) != principal).cloned() else { continue };
            if !par_nic.iter().any(|(n, _)| *n == nic) {
                par_nic.push((nic.clone(), Vec::new()));
            }
            let Some((_, cases)) = par_nic.iter_mut().find(|(n, _)| *n == nic) else { continue };
            let ecrites = ValeurDeCase::de(e).and_then(|v| v.cases()).into_iter().flatten().filter_map(|(k, v)| v.map(|v| (k, v))).collect::<Vec<_>>();
            for d in &declarees {
                let cle: CaseDeFiche = d.cle.into();
                if cle != CaseEtablissement::TauxAtmp.into() && ecrites.iter().any(|(k, _)| k == d.cle.nom()) && !cases.iter().any(|(k, _)| *k == cle) {
                    cases.push((cle, Vec::new()));
                }
            }
            for (cle, datees) in cases.iter_mut() {
                let valeur = ecrites.iter().find(|(k, _)| k == cle.nom()).map(|(_, v)| v.clone());
                if datees.last().is_some_and(|d| d.valeur == valeur) {
                    continue;
                }
                datees.push(droit::fiche::Datee { depuis: version.depuis, valeur, note: version.note.clone() });
            }
        }
    }
    let mut lignes = Vec::new();
    for (nic, cases) in par_nic {
        // Il se nomme par son dernier libelle, sinon par son NIC.
        let libelle = cases.iter().find(|(k, _)| *k == CaseEtablissement::Libelle.into()).and_then(|(_, d)| d.last()).and_then(|d| d.valeur.as_ref()).and_then(|v| v.ecrite().mot().map(str::to_string));
        let dit = match libelle {
            Some(l) => l,
            None => nic.to_string(),
        };
        let mut temps = Vec::new();
        for (cle, datees) in cases {
            if let Some(t) = crate::temps::d_une_case(crate::temps::CaseDatee::Case(cle), &datees, jour, |a, b| a == b)? {
                temps.push(t);
            }
        }
        lignes.push(crate::temps::TempsDUneLigne { cle: nic, dit, cases: temps });
    }
    Ok(crate::temps::d_une_liste(CaseEntreprise::Etablissements.into(), lignes))
}

/// La fiche telle que l'ecran la montre. Ses cases sont COMBLEES : une fiche d'avant ne porte que la forme declaree, et
/// l'ecran doit proposer une forme lisible de depart. Rien n'est ecrit pour autant. `montres` : les cases que la page en
/// montre (`fiches::montres`, que la fiche de l'entreprise `ent` et les permanents du jour `p` commandent).
fn fiche_verifiee<F: SorteDeFiche>(
    ctx: &Contexte,
    cle: Option<&F::Cle>,
    plate: F::Plate,
    historique: Vec<TempsDeLaCase<ValeurDeCase>>,
    ent: &droit::fiche::ChampsEntreprise,
    p: &engagement::permanents::Permanents,
) -> R<VueFiche<F::Plate, F::Cle>> {
    let nom = F::NOM;
    let plate = F::retenue(plate);
    // La fiche se verifie telle qu'elle est ; sa forme lisible comblee n'est qu'un depart pour l'ecran.
    let anomalies = anomalies_enregistrees::<F>(&plate)?;
    let montree = comblee::<F>(&plate)?;
    let declare = champs::apercu_derive(&montree)?;
    // Sa pastille : archivee (une information), ses erreurs (a corriger, la compagnie le peut), ce qui est a verifier,
    // ou complete.
    let grave = |a: &champs::Anomalie| a.gravite == champs::Gravite::Erreur;
    let erreurs = anomalies.iter().filter(|a| grave(a)).count();
    let a_verifier = anomalies.len() - erreurs;
    let pluriel = |n: usize| if n > 1 { "s" } else { "" };
    // Ce qui est a corriger ou a verifier se nomme (le libelle de la case), et sa raison se lit au survol (`detail`).
    let nommer = |erreur: bool| -> R<(String, String)> {
        let siennes: Vec<&champs::Anomalie> = anomalies.iter().filter(|a| grave(a) == erreur).collect();
        let mut cases: Vec<String> = Vec::new();
        for a in &siennes {
            let libelle = match champs::champ(nom, a.champ)?.map(|c| c.libelle).filter(|l| !l.is_empty()) {
                Some(l) => l,
                None => a.champ.to_string(),
            };
            let mut lettres = libelle.chars();
            let libelle: String = lettres.next().into_iter().flat_map(char::to_lowercase).chain(lettres).collect();
            if !cases.contains(&libelle) {
                cases.push(libelle);
            }
        }
        let detail = siennes.iter().map(|a| a.message.as_str()).collect::<Vec<_>>().join(" ");
        Ok((cases.join(", "), detail))
    };
    let archivee = montree.ecrite(CaseSalarie::Actif.into()) == MotEcrit::OuiNon(false);
    let pastille = if archivee {
        PastilleDeFiche { pastille: socle::ecran::Pastille::de(socle::ecran::EtatDEcran::Information, "archivé"), detail: None }
    } else if erreurs > 0 {
        let (cases, detail) = nommer(true)?;
        PastilleDeFiche { pastille: socle::ecran::Pastille::de(socle::ecran::EtatDEcran::AFaire, &format!("{} erreur{} : {}", erreurs, pluriel(erreurs), cases)), detail: Some(detail) }
    } else if a_verifier > 0 {
        let (cases, detail) = nommer(false)?;
        PastilleDeFiche { pastille: socle::ecran::Pastille::de(socle::ecran::EtatDEcran::AFaire, &format!("à vérifier : {}", cases)), detail: Some(detail) }
    } else {
        PastilleDeFiche { pastille: socle::ecran::Pastille::de(socle::ecran::EtatDEcran::Fait, "complet"), detail: None }
    };
    // Ce que la ligne de la fiche dit, dans la liste : ce qui l'identifie (le metier et le matricule d'un salarie, le
    // numero d'objet et le code analytique d'un spectacle), et le geste qui l'archive ou la reactive.
    let lu = |k: NomDeCase| montree.ecrite(k.into()).mot().map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    // Un spectacle dont l'import n'a lu que le numero d'objet porte le titre « A COMPLETER » : la page le range a part.
    let sans_titre = nom == NomDeFiche::Spectacle && a_nommer(lu(CaseSpectacle::Titre.into()).as_deref());
    // Ce que l'en-tete de la fiche dit : les champs a corriger.
    let (erreurs_titre, erreurs_note) = erreurs_dites(erreurs);
    let quoi = match nom {
        NomDeFiche::Salarie => [
            lu(CaseSalarie::EmploiAffichage.into()).or_else(|| lu(CaseSalarie::Emploi.into())),
            lu(CaseSalarie::CodeSalarie.into()).map(|m| format!("matricule {}", m)),
        ],
        _ => [lu(CaseSpectacle::NumeroObjet.into()), lu(CaseSpectacle::CodeAnalytique.into()).map(|a| format!("analytique {}", a))],
    };
    let quoi_dit = quoi.into_iter().flatten().collect::<Vec<_>>().join(" · ");
    let geste_dit = if archivee { "Réactiver" } else { "Archiver" };
    let matricule_dit = lu(CaseSalarie::CodeSalarie.into()).filter(|_| nom == NomDeFiche::Salarie).map(|m| format!("matricule {}", m));
    let super::fiches::Montres { cases: montres, pieces_ouvertes } = super::fiches::montres(ctx, F::a_juger(&montree), cle.and_then(F::salarie), ent, &|| Ok(!p.dus.is_empty()))?;
    // L'organisme Urssaf que le code postal de la compagnie donne : le cas ordinaire du code Urssaf.
    let du_code_postal = match ctx.entreprise.code_postal.portee() {
        Some(cp) => declarations::urssaf::base()?.du_code_postal(cp),
        None => None,
    };
    let ecarts = champs::fiche(nom)?.iter().filter(|c| c.ecart(&montree, du_code_postal.as_ref().map(|c| c.as_str()))).map(|c| c.cle).collect();
    Ok(VueFiche {
        quoi_dit,
        geste_dit,
        matricule_dit,
        erreurs_titre,
        erreurs_note,
        nom,
        cle: cle.cloned(),
        donnees: DonneesDeLaFiche { fiche: F::sans_notes(montree), historique },
        declare,
        anomalies,
        pastille,
        sans_titre,
        montres,
        pieces_ouvertes,
        ecarts,
    })
}

/// UN SPECTACLE A NOMMER : l'import n'a lu que son numero d'objet, son titre est « A COMPLETER »
/// (`import::proposee::A_COMPLETER`).
pub(crate) fn a_nommer(titre: Option<&str>) -> bool {
    titre.is_some_and(|t| t.trim().eq_ignore_ascii_case(import::proposee::A_COMPLETER))
}

/// Ce que l'en-tete d'une fiche dit de ses erreurs (`n`, celles de gravite « erreur ») : le titre de l'encart, aucun sans
/// erreur, et sa note.
pub(crate) fn erreurs_dites(n: usize) -> (Option<String>, &'static str) {
    let titre = match n {
        0 => None,
        1 => Some("Un champ à corriger".to_string()),
        n => Some(format!("{} champs à corriger", n)),
    };
    (titre, if n > 1 { "marqués ci-dessous" } else { "marqué ci-dessous" })
}

/// Un etablissement qu'un engagement choisit : son NIC (vide : le siege), son nom, son ouverture et son dernier jour s'il
/// est ferme, et ce que l'option en dit.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ChoixEtablissement {
    periode_dit: Option<String>,
    /// Le NIC de l'etablissement ; aucun pour le siege (un engagement du siege n'en ecrit pas).
    nic: Option<droit::fiche::Nic>,
    nom: String,
    ouverture: Option<socle::date::Jour>,
    fermeture: Option<socle::date::Jour>,
}

/// CE QU'UN ENGAGEMENT CHOISIT quand la compagnie a d'autres etablissements que le siege : le siege (`nic` absent) puis
/// chacun, nomme par son nom et sa commune, avec son ouverture, et son dernier jour s'il est ferme (un engagement passe
/// le garde). `None` : un seul etablissement, rien a choisir.
pub(crate) fn choix_d_etablissement(ctx: &Contexte) -> R<Option<Vec<ChoixEtablissement>>> {
    let principal = ctx.entreprise.nic_principal();
    let autres: Vec<(droit::fiche::Etablissement, Option<socle::date::Jour>)> = droit::contexte::histoire_des_etablissements(ctx.entreprise_brute().etablissements.as_deref())?
        .into_iter()
        .filter(|(e, _)| e.nic.dite() != principal)
        .collect();
    if autres.is_empty() {
        return Ok(None);
    }
    let nom_du_siege = match ctx.entreprise.commune_affichage.remplie().or(ctx.entreprise.commune.portee()) {
        Some(commune) => format!("Le siège, {}", commune),
        None => "Le siège".to_string(),
    };
    let mut out = vec![ChoixEtablissement { periode_dit: None, nic: None, nom: nom_du_siege, ouverture: None, fermeture: None }];
    for (e, fermeture) in autres {
        // Un etablissement se choisit par son NIC : sans lui, il ne se choisit pas.
        let Some(nic) = e.nic.dite().cloned() else { continue };
        let commune = e.commune_affichage.remplie().or(e.commune.portee());
        let nom = [e.libelle.remplie(), commune].into_iter().flatten().filter(|s| !s.trim().is_empty()).collect::<Vec<_>>().join(", ");
        // Son ouverture, et son dernier jour s'il est ferme, tels que l'option les dit.
        let periode_dit = e.ouverture.map(|o| match &fermeture {
            Some(f) => format!("ouvert le {}, jusqu'au {}", o.jj_mm_aaaa(), f.jj_mm_aaaa()),
            None => format!("ouvert le {}", o.jj_mm_aaaa()),
        });
        let nom = if nom.is_empty() { nic.to_string() } else { nom };
        out.push(ChoixEtablissement { periode_dit, nom, nic: Some(nic), ouverture: e.ouverture, fermeture });
    }
    Ok(Some(out))
}

/// Les lignes des autres etablissements demandent-elles leur versement mobilite ? Oui quand le siege en paie un : le
/// controle des etablissements l'exige alors de chacun (`droit::contexte::controler_etablissements`). Un bareme du jour qui
/// ne se compose pas ne le demande pas : la configuration s'ouvre quand meme (c'est la qu'on la corrige), et dit pourquoi
/// (`ConfigurationDeLaPage::pour_payer`, que le meme empilement refuse).
pub(crate) fn versement_mobilite_des_etablissements(ctx: &Contexte) -> bool {
    match ctx.bareme_fondu(&ctx.aujourdhui) {
        Ok(b) => droit::bareme::versement_mobilite(&b).is_some(),
        Err(_) => false,
    }
}

/// La racine des donnees du poste (`contexte.RACINE`) : `PUPPETPAIE_DONNEES`,
/// sinon le dossier `prive/` voisin du code, sinon le code lui-meme. LA LIGNE DE COMMANDE SEULEMENT (`moteur COMMANDE`) :
/// une route recoit sa compagnie (`o["racine"]`, `Contexte::racine`) et ne lit jamais cette
/// variable -- la fenetre sert ses compagnies dans un seul processus.
pub fn racine_du_poste() -> PathBuf {
    if let Some(e) = std::env::var_os("PUPPETPAIE_DONNEES").filter(|e| !e.is_empty()) {
        return PathBuf::from(e);
    }
    compagnie::installation::dossier_reel()
}

/// `DOCUMENTS_IDENTITE` : ou va chaque image.
fn documents_identite(quoi: documents::SorteDImage) -> &'static [&'static str] {
    match quoi {
        documents::SorteDImage::Signature => &[
            "les contrats d'engagement", "l'AEM en PDF", "l'attestation employeur (AER)",
            "le certificat Congés Spectacles",
            "le certificat de travail et le reçu pour solde de tout compte",
        ],
        documents::SorteDImage::Cachet => &["l'attestation employeur (AER), case « Cachet de l'entreprise »"],
        documents::SorteDImage::Logo => &[
            "les bulletins de paie", "les contrats d'engagement",
            "le certificat de travail et le reçu pour solde de tout compte",
        ],
    }
}

/// La personne qui signe, telle que la fiche Entreprise la nomme, en forme lisible.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Signataire {
    /// Son prenom et son nom ; vide quand la fiche ne dit pas qui signe.
    pub(crate) nom: Option<String>,
    pub(crate) fonction: Option<String>,
}

/// `signataire_de` : la personne qui signe, telle que la fiche Entreprise la nomme, en forme lisible.
pub(crate) fn signataire_de(ent: &droit::fiche::ChampsEntreprise) -> Signataire {
    let Some(s) = ent.signataire.dite() else { return Signataire { nom: None, fonction: None } };
    let lisible = |affichage: &Option<String>, brut: &Option<String>| -> Option<String> {
        match affichage.remplie() {
            Some(v) => Some(v.to_string()),
            None => brut.portee().map(champs::titre),
        }
    };
    let complet: Vec<String> = [lisible(&s.prenom_affichage, &s.prenom), lisible(&s.nom_affichage, &s.nom)].into_iter().flatten().collect();
    Signataire { nom: (!complet.is_empty()).then(|| complet.join(" ")), fonction: s.fonction.en_texte() }
}

/// `identite/identite.json` des gabarits : pour qui la signature a ete posee, et quand (`poser-identite` l'ecrit).
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegistreIdentite {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signature: Option<PoseePour>,
}

/// La personne pour qui une image est posee, et le jour.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PoseePour {
    /// La personne qui signait ; vide quand la fiche ne le disait pas.
    pub(crate) de: Option<String>,
    pub(crate) le: socle::date::Jour,
}

/// Le registre des images posees, lu par sa forme ; vide s'il n'existe pas. Sa seule lecture.
pub(crate) fn registre_identite(dossier: &Path) -> R<RegistreIdentite> {
    let p = dossier.join("identite.json");
    if !p.exists() {
        return Ok(RegistreIdentite::default());
    }
    socle::forme::lire_fichier(&p, "Le registre des images de la compagnie (gabarits/identite/identite.json) ne se lit pas.")
}

/// UNE IMAGE DE L'IDENTITE (signature, cachet, logo) : posee ou non, dessinee par le logiciel (le cachet), son fichier et
/// sa forme quand elle est posee, les documents qui la portent ; la signature dit pour qui elle est posee. La route
/// `identite` y ajoute sa pastille et ce que son ecran en dit.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ImageIdentite {
    pub(crate) pose: bool,
    pub(crate) automatique: bool,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::fichiers::ecrire_chemin")]
    dossier: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) octets: Option<socle::grandeurs::Octets>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "format_ecrit")]
    pub(crate) format: Option<fabrique_pdf::alleger::FormatDImage>,
    #[serde(skip_serializing_if = "Option::is_none", with = "socle::paris::ecrit::seconde::option")]
    pub(crate) quand: Option<socle::paris::Moment>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::fichiers::ecrire_chemin")]
    fichier: Option<PathBuf>,
    pub(crate) documents: &'static [&'static str],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signataire: Option<Signataire>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) posee_pour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) autre_personne: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pastille: Option<socle::ecran::Pastille>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub(crate) ecran: Option<EcranDeLImage>,
}

/// Le format d'une image posee, ecrit par son nom (`png`, `jpg`, `svg`), l'extension de son fichier.
fn format_ecrit<S: serde::Serializer>(f: &Option<fabrique_pdf::alleger::FormatDImage>, s: S) -> Result<S::Ok, S::Error> {
    match f {
        Some(f) => s.serialize_str(f.nom()),
        None => s.serialize_none(),
    }
}

/// Ce que l'ecran de la signature et du tampon dit d'une image : son titre, ce qu'elle engage et ou elle va, ce que dit
/// l'image posee, et les mots de ses gestes.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct EcranDeLImage {
    pub(crate) titre: String,
    pub(crate) dit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) automatique_dit: Option<&'static str>,
    pub(crate) pose_dit: Option<String>,
    pub(crate) retirer_dit: &'static str,
    pub(crate) importer_dit: &'static str,
}

/// `identite` : ce qui est pose (signature, cachet, logo), et depuis quand.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Identite {
    pub(crate) signature: ImageIdentite,
    pub(crate) cachet: ImageIdentite,
    pub(crate) logo: ImageIdentite,
}

/// `identite` : ce qui est pose (signature, cachet, logo), et depuis quand. `gabarits` est celui du poste
/// (`contexte.GABARITS`).
pub(crate) fn identite(ctx: &Contexte, gabarits: &Path) -> R<Identite> {
    let dossier = gabarits.join("identite");
    let image = |quoi: documents::SorteDImage| -> R<ImageIdentite> {
        let pose = fabrique_pdf::alleger::FormatDImage::TOUS.into_iter().map(|f| (dossier.join(format!("{}{}", quoi.nom(), f.extension())), f)).find(|(c, _)| c.exists());
        let mut e = ImageIdentite {
            pose: pose.is_some(),
            automatique: quoi == documents::SorteDImage::Cachet,
            dossier: None,
            octets: None,
            format: None,
            quand: None,
            fichier: None,
            documents: documents_identite(quoi),
            signataire: None,
            posee_pour: None,
            autre_personne: None,
            pastille: None,
            ecran: None,
        };
        match pose {
            None => e.dossier = Some(dossier.clone()),
            Some((f, format)) => {
                let st = std::fs::metadata(&f).map_err(|x| socle::echec::du_disque(&f, &x).reclasser(Classe::Disque))?;
                e.octets = Some(socle::grandeurs::Octets::du_disque(&st));
                e.format = Some(format);
                // Datee hors du calendrier, l'image posee se dit sans sa date.
                e.quand = socle::paris::mtime(&st).map(socle::paris::Moment::a_la_seconde);
                e.fichier = Some(f);
            }
        }
        Ok(e)
    };
    let mut signature = image(documents::SorteDImage::Signature)?;
    // LA SIGNATURE EST CELLE D'UNE PERSONNE : celle que la fiche Entreprise nomme sous « Qui signe ». Si l'image a ete
    // posee pour une autre, la page le dit.
    let signataire = signataire_de(&ctx.entreprise);
    let pour = if signature.pose { registre_identite(&dossier)?.signature.and_then(|s| s.de) } else { None };
    signature.autre_personne = Some(matches!((&pour, &signataire.nom), (Some(p), Some(n)) if p != n));
    signature.posee_pour = pour;
    signature.signataire = Some(signataire);
    Ok(Identite { signature, cachet: image(documents::SorteDImage::Cachet)?, logo: image(documents::SorteDImage::Logo)? })
}

impl Identite {
    /// Les trois images, dans l'ordre ou l'ecran les montre.
    pub(crate) fn images_mut(&mut self) -> [(documents::SorteDImage, &mut ImageIdentite); 3] {
        [(documents::SorteDImage::Signature, &mut self.signature), (documents::SorteDImage::Cachet, &mut self.cachet), (documents::SorteDImage::Logo, &mut self.logo)]
    }
}

/// `boite_liee` : peut-on envoyer, et les signatures reviennent-elles seules ?
/// Une seule reponse aux deux : le mot de passe de la boite est pose. Ne lit
/// jamais ce mot de passe : l'adresse (`compte`), et si un secret existe.
pub(crate) fn boite_liee(ctx: &Contexte) -> R<bool> {
    if compagnie::bridage::branchement(&ctx.racine)?.joue() {
        return Ok(true);
    }
    let coffre = compagnie::coffre::Coffre::du_poste(&ctx.racine);
    // L'adresse posee dans l'environnement, sinon celle du coffre.
    let adresse = match std::env::var("COURRIEL_COMPTE") {
        Ok(a) if !a.is_empty() => Some(a),
        Ok(_) | Err(std::env::VarError::NotPresent) => coffre.compte_pose("courriel")?,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(socle::echec::refus_avec(
                "La variable qui pose l'adresse du compte de courriel ne se lit pas : ce n'est pas du texte. Corrigez-la, ou retirez-la.",
                socle::echec::Lieu { cle: Some("COURRIEL_COMPTE".to_string()), ..socle::echec::Lieu::default() },
            ))
        }
    };
    // Sans adresse, rien ne s'envoie.
    if !adresse.is_some_and(|a| !a.trim().is_empty()) {
        return Ok(false);
    }
    coffre.disponible("courriel")
}

/// Une demande de fiche partie et pas encore revenue : son code, a qui, quand, et ce que la page en dit.
#[derive(Debug, Clone, serde::Serialize)]
struct FicheAttendue {
    code: documents::fiche_questionnaire::CodeDeDemande,
    adresse: droit::fiche::Courriel,
    prenom: Option<String>,
    quand: socle::paris::InstantOuJour,
    demande_dit: String,
}

/// Les demandes de fiche parties et pas encore revenues, les plus anciennes d'abord, comme la page les lit.
fn fiches_attendues(ctx: &Contexte) -> R<Vec<FicheAttendue>> {
    Ok(crate::fiches::fiches_attendues(ctx)?
        .into_iter()
        .map(|a| {
            let demande_dit = format!("demandée le {}", a.quand.jour().jj_mm_aaaa());
            FicheAttendue { code: a.code, adresse: a.adresse, prenom: a.prenom, quand: a.quand, demande_dit }
        })
        .collect())
}


/// Les mots de la liste des fiches d'une sorte : le bouton qui en ajoute une, la liste vide, la case des archives.
#[derive(Debug, Clone, serde::Serialize)]
struct MotsDeLaListe {
    nouvelle: String,
    aucun: String,
    archives: String,
}

/// Les mots des listes de fiches, par sorte, et le mot d'une valeur trouvee dans l'historique d'une fiche.
#[derive(Debug, Clone, serde::Serialize)]
struct EcranDesFiches {
    salarie: MotsDeLaListe,
    spectacle: MotsDeLaListe,
    ancienne: &'static str,
}

/// La description des cases de chaque fiche (`champs::description`, lue de `champs-base.json`).
#[derive(Debug, Clone, serde::Serialize)]
struct Descriptions {
    entreprise: Vec<champs::Description<'static>>,
    salarie: Vec<champs::Description<'static>>,
    spectacle: Vec<champs::Description<'static>>,
}

/// LA FICHE A FAIRE REMPLIR PAR LA PERSONNE : ce que dit l'invitation, et ce qui lui manque sans boite liee.
#[derive(Debug, Clone, serde::Serialize)]
struct Invitation {
    aide: &'static str,
    sans_boite: Option<&'static str>,
    ou: &'static str,
}

/// TOUTE LA CONFIGURATION, VERIFIEE (`configuration`, et `configuration` de `etat`), et ce que la page en tire sans un
/// appel de plus.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ConfigurationDeLaPage {
    ecran: EcranDesFiches,
    fiches_attendues_titre: String,
    compteurs_dit: Option<String>,
    deduction_forfaitaire: String,
    numeros_sans_titre: Option<String>,
    entreprise: VueFiche<droit::fiche::ChampsEntreprise, ()>,
    salaries: Vec<VueFiche<droit::fiche::ChampsSalarie, droit::contexte::IdSalarie>>,
    spectacles: Vec<VueFiche<droit::fiche::ChampsSpectacle, droit::contexte::IdSpectacle>>,
    compteurs: documents::ecrire_documents::Compteurs,
    erreurs: Option<socle::ecran::Pastille>,
    description: Descriptions,
    /// Ce que la liste des etablissements commande.
    etablissements: Option<Vec<ChoixEtablissement>>,
    versement_mobilite_des_etablissements: bool,
    /// CE QUI MANQUE POUR PAYER AUJOURD'HUI : le refus du bareme du jour, controle (`Contexte::bareme` : l'effectif, le
    /// taux AT/MP...), que le bandeau montre ; « Pour démarrer » demande la case (`declarations::demarrage`). Lire
    /// l'etat de la compagnie n'est pas payer : la paie refuse a son calcul.
    pour_payer: Option<String>,
    /// CE QUI EST POSE : signature et logo, avec leur date.
    identite: Identite,
    invitation: Invitation,
    boite_liee: bool,
    fiches_attendues: Vec<FicheAttendue>,
}

/// La route `configuration` : la configuration et sa phrase.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ConfigurationDite {
    phrase: Phrase,
    #[serde(flatten)]
    configuration: ConfigurationDeLaPage,
}

/// La route `configuration` : celle aussi que rendent les routes qui l'ont changee (`rappels::revue`).
pub(crate) fn configuration_dite(ctx: &Contexte) -> R<ConfigurationDite> {
    let configuration = configuration(ctx)?;
    Ok(ConfigurationDite { phrase: dire_la_configuration(&configuration), configuration })
}

/// Toute la configuration, verifiee, et ce que la page en tire sans un appel de plus.
fn configuration(ctx: &Contexte) -> R<ConfigurationDeLaPage> {
    // LES FICHES SE MONTRENT A LA DATE DU JOUR, chaque case avec sa valeur en vigueur et ses temps a ce jour-la.
    let du_jour;
    let ctx = if ctx.lue_au == ctx.aujourdhui {
        ctx
    } else {
        du_jour = ctx.au(&ctx.aujourdhui)?;
        &du_jour
    };
    let jour = ctx.aujourdhui;
    // La fiche de l'entreprise : ses autres etablissements sont ceux que la fiche dit en dernier (`etablissements::ecrire`),
    // ceux qui ouvrent plus tard compris ; les temps de leurs cases se disent ligne par ligne.
    let ent = ctx.entreprise.clone();
    let autres = ctx.etablissements_dits();
    let montree = droit::fiche::ChampsEntreprise { etablissements: (!autres.is_empty()).then_some(autres), ..ctx.entreprise.clone() };
    let mut temps = temps_de(Some(ctx.entreprise_brute()), &jour)?;
    temps.extend(temps_des_etablissements(ctx, &jour)?);
    // LES PERMANENTS DU JOUR DE L'ECRAN : ce qui ne vaut que pour eux se montre s'ils se proposent ce jour-la, ou qu'un
    // permanent est du (`engagement::permanents`).
    let p = engagement::permanents::Permanents::de(ctx, &socle::date::Periode::du_jour(jour))?;
    let entreprise = fiche_verifiee::<droit::fiche::FicheEntrepriseDatee>(ctx, None, montree, temps, &ent, &p)?;
    let salaries = ctx
        .salaries
        .iter()
        .map(|(k, v)| fiche_verifiee::<droit::fiche::FicheSalarieDatee>(ctx, Some(k), v.champs.clone(), temps_de(ctx.salarie_brut(k), &jour)?, &ent, &p))
        .collect::<R<Vec<_>>>()?;
    let spectacles = ctx
        .spectacles
        .iter()
        .map(|(k, v)| fiche_verifiee::<droit::fiche::FicheSpectacleDatee>(ctx, Some(k), v.clone(), temps_de(ctx.spectacle_brut(k.as_str()), &jour)?, &ent, &p))
        .collect::<R<Vec<_>>>()?;
    // LES ERREURS DE CONFIGURATION, toutes fiches comptees, que le bandeau montre (orange) : une valeur saisie et fausse,
    // les memes que la pastille de chaque fiche. Une case vide n'en est jamais une : ce qu'un acte exige, « Pour demarrer » le dit.
    let erreurs: usize = entreprise.erreurs() + salaries.iter().map(VueFiche::erreurs).sum::<usize>() + spectacles.iter().map(VueFiche::erreurs).sum::<usize>();
    let erreurs = (erreurs > 0).then(|| socle::ecran::Pastille::de(socle::ecran::EtatDEcran::AFaire, &format!("{} erreur{} de configuration", erreurs, if erreurs > 1 { "s" } else { "" })));
    // LA DEDUCTION FORFAITAIRE SPECIFIQUE vue de la fiche d'un salarie : l'option est celle de l'employeur, dans la fiche de
    // l'entreprise ; la fiche du salarie dit si elle s'applique (`calcul::deduction_forfaitaire` decide).
    let description_entreprise = champs::description(NomDeFiche::Entreprise)?;
    let deduction = match ctx.entreprise.deduction_forfaitaire_option.code() {
        None => "La compagnie ne l'applique pas : c'est une option de l'employeur, qui se règle dans la fiche de l'entreprise (« Avancé — cas particuliers »). Une fois l'option posée, la profession et l'accord du salarié se disent ici.".to_string(),
        Some(option) => {
            let nom = description_entreprise
                .iter()
                .find(|c| c.cle == CaseEntreprise::DeductionForfaitaireOption.into())
                .and_then(|c| c.libelles.and_then(|l| l.de(option)))
                .filter(|n| !n.is_empty());
            let nom = match nom {
                Some(n) => n.to_string(),
                None => option.to_string(),
            };
            format!("La compagnie l'applique (fiche de l'entreprise) : « {} ».", nom)
        }
    };
    // LES NUMEROS D'OBJET SANS TITRE (importes des DSN, « A COMPLETER »), archives : la liste des spectacles les groupe sous
    // cette phrase.
    let sans_titre: Vec<&droit::fiche::ChampsSpectacle> = ctx.spectacles.values().filter(|s| s.archivee() && s.titre.portee().is_some_and(|t| t.trim().eq_ignore_ascii_case("a completer"))).collect();
    // L'annee ou chaque numero a ete demande (`NumeroObjetSpectacle::annee_de_la_demande`).
    let ans: Vec<socle::mois::Annee> = sans_titre.iter().filter_map(|s| s.numero_objet.dite().and_then(droit::fiche::NumeroObjetSpectacle::annee_de_la_demande)).collect();
    let numeros_sans_titre = match (sans_titre.len(), ans.iter().min(), ans.iter().max()) {
        (0, _, _) => None,
        (n, de, a) => {
            let s = if n > 1 { "s" } else { "" };
            // Aucun numero ne dit l'annee de sa demande : la phrase n'en dit pas.
            let annees: String = de
                .map(|de| match a.filter(|a| *a != de) {
                    Some(a) => format!(" (demandé{} de {} à {})", s, de, a),
                    None => format!(" (demandé{} en {})", s, de),
                })
                .into_iter()
                .collect();
            Some(format!("{} numéro{} sans titre, trouvé{} dans les DSN{} : les nommer si vous savez à quel spectacle ils étaient (cliquer sur la ligne).", n, s, s, annees))
        }
    };
    // Les compteurs de la derniere annee, en une ligne : ce que l'en-tete de la Paie montre.
    let compteurs = documents::ecrire_documents::compteurs(&ctx.racine)?;
    let compteurs_dit = compteurs.iter().next_back().map(|(an, c)| {
        let n = |x: Option<String>| match x {
            Some(n) => n,
            None => "aucun".to_string(),
        };
        format!("compteurs {} — bulletin {} · CS {}", an, n(c.bulletin.map(|b| b.to_string())), n(c.certificat_cs.map(|c| c.to_string())))
    });
    let fiches_attendues = fiches_attendues(ctx)?;
    let liste = |un: &str, les: &str| MotsDeLaListe { nouvelle: format!("+ un {}", un), aucun: format!("Aucun {} actif", un), archives: format!("Afficher les {} archivés", les) };
    let boite_liee = boite_liee(ctx)?;
    let versement_mobilite_des_etablissements = versement_mobilite_des_etablissements(ctx);
    let pour_payer = match ctx.bareme(&jour) {
        Ok(_) => None,
        Err(e) if e.dit_un_manque() => Some(e.phrase),
        // Une panne (le disque, le socle livre) n'est pas ce qui manque pour payer : elle remonte.
        Err(e) => return Err(e),
    };
    Ok(ConfigurationDeLaPage {
        ecran: EcranDesFiches { salarie: liste("salarié", "salariés"), spectacle: liste("spectacle", "spectacles"), ancienne: "valeur d'une autre date" },
        fiches_attendues_titre: format!("{} fiche(s) en attente", fiches_attendues.len()),
        compteurs_dit,
        deduction_forfaitaire: deduction,
        numeros_sans_titre,
        entreprise,
        salaries,
        spectacles,
        compteurs,
        erreurs,
        description: Descriptions { entreprise: description_entreprise, salarie: champs::description(NomDeFiche::Salarie)?, spectacle: champs::description(NomDeFiche::Spectacle)? },
        etablissements: choix_d_etablissement(ctx)?,
        versement_mobilite_des_etablissements,
        pour_payer,
        identite: identite(ctx, &ctx.gabarits())?,
        invitation: Invitation {
            aide: "Elle reçoit un courriel avec un bouton : une réponse s'ouvre avec les questions (identité, naissance, numéro de sécurité sociale, adresse, compte bancaire), elle y répond et l'envoie. Sa fiche se crée seule à l'arrivée de la réponse ; il vous restera l'emploi à choisir.",
            sans_boite: (!boite_liee).then_some("Il faut d'abord lier la boîte courriel de la compagnie (Configuration › Mots de passe)."),
            ou: "Ou remplissez la fiche vous-même ci-dessous. Le matricule se donne à l'enregistrement.",
        },
        boite_liee,
        fiches_attendues,
    })
}

// ------------------------------------------------------------- l'historique

/// `SORTES` : les sortes de pieces produites d'un mois que le bilan nomme, dans son ordre, et leur mot (la sorte d'un nom :
/// `SorteDePieceProduite::du_nom`).
const SORTES: [(documents::SorteDePieceProduite, &str); 5] = [
    (documents::SorteDePieceProduite::Bulletins, "bulletins"),
    (documents::SorteDePieceProduite::Contrat, "contrats"),
    (documents::SorteDePieceProduite::Dpae, "DPAE"),
    (documents::SorteDePieceProduite::Dsn, "DSN"),
    (documents::SorteDePieceProduite::CertificatsCs, "certificats CS"),
];

/// Les AEM que l'ancien logiciel a laissees dans les sorties d'un mois (`AEM_…`) : pas une piece produite ; le bilan les
/// nomme apres les autres.
const AEM_DE_L_ANCIEN_LOGICIEL: (&str, &str) = ("AEM_", "AEM");

/// `mois_connus` : les mois saisis et les mois produits, du plus recent au plus
/// ancien -- `(mois, fichiers du dossier de sortie)`.
pub(crate) fn mois_connus(racine: &Path) -> R<Vec<(Mois, Vec<NomDeFichier>)>> {
    let mut vus: BTreeSet<Mois> = engagement::stock::mois_saisis(racine)?.into_iter().collect();
    // Les entrees d'un dossier (aucune s'il n'existe pas) ; un dossier qui ne se lit pas se refuse.
    let entrees = |d: &Path| -> R<Vec<std::fs::DirEntry>> {
        match std::fs::read_dir(d) {
            Ok(rd) => rd.map(|e| e.map_err(|x| socle::echec::du_disque(d, &x))).collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(socle::echec::du_disque(d, &e)),
        }
    };
    let mut dossiers: BTreeMap<Mois, PathBuf> = BTreeMap::new();
    // Les dossiers AAAA-MM seulement : `sorties/exercices/` n'est pas un mois.
    for d in socle::fichiers::nommes_d_un_mois(&documents::DossierDuMois::des_mois(racine), None, false)? {
        vus.insert(d.mois);
        dossiers.insert(d.mois, d.chemin);
    }
    let mut out = Vec::new();
    for mois in vus.into_iter().rev() {
        let mut fichiers: Vec<NomDeFichier> = Vec::new();
        if let Some(d) = dossiers.get(&mois) {
            for e in entrees(d)? {
                if e.path().is_file() {
                    fichiers.push(NomDeFichier::du(&e.path())?);
                }
            }
            fichiers.sort();
        }
        out.push((mois, fichiers));
    }
    Ok(out)
}

/// Ou en est la DSN d'un mois, telle que l'Historique la lit : l'etat lu (`compagnie::etat_dsn`), ou « en_retard » pour
/// une DSN a deposer passee l'echeance.
#[derive(Debug, Clone, Copy)]
enum DsnDuBilan {
    Lue(Etat),
    EnRetard,
}

impl serde::Serialize for DsnDuBilan {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            DsnDuBilan::Lue(e) => e.serialize(s),
            DsnDuBilan::EnRetard => s.serialize_str("en_retard"),
        }
    }
}

/// UN MOIS DE L'HISTORIQUE : le mois, s'il a des documents (`produit`), ou en est sa DSN, sa pastille, le jour du depot,
/// ses documents en mots, son echeance, s'il se corrige, s'il est tenu avant ce logiciel, ce qui reste a faire, ses
/// sortes et ses fichiers, et si son exercice est clos. Un mois dont la DSN ne se lit pas (une fiche entreprise
/// incomplete) ne dit que ses fichiers (`MoisDuBilan::nu`).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct MoisDuBilan {
    mois: Mois,
    produit: bool,
    dsn: Option<DsnDuBilan>,
    saisi_dit: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pastille: Option<socle::ecran::Pastille>,
    depose_le: Option<socle::paris::InstantOuJour>,
    depose_dit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    documents_dit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    echeance: Option<socle::date::Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    echeance_depot: Option<declarations::echeance_dsn::EcheanceDite>,
    corrigeable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    importe: Option<bool>,
    a_faire: bool,
    sortes: Vec<&'static str>,
    fichiers: Vec<NomDeFichier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    clos: Option<bool>,
}

impl MoisDuBilan {
    /// Un mois qui ne se lit pas : ses fichiers, rien a corriger, et pourquoi (sa pastille le dit).
    fn nu(mois: Mois, fichiers: &[NomDeFichier], pourquoi: String) -> MoisDuBilan {
        MoisDuBilan {
            mois,
            produit: !fichiers.is_empty(),
            dsn: None,
            saisi_dit: "saisi",
            pastille: Some(socle::ecran::Pastille { couleur: socle::ecran::couleur_de_l_etat(socle::ecran::EtatDEcran::AFaire), mots: pourquoi }),
            depose_le: None,
            depose_dit: None,
            documents_dit: None,
            echeance: None,
            echeance_depot: None,
            corrigeable: false,
            importe: None,
            a_faire: true,
            sortes: Vec::new(),
            fichiers: fichiers.to_vec(),
            clos: None,
        }
    }
}

/// `bilan_des_mois` : chaque mois connu, du plus recent au plus ancien, avec ou
/// en est sa DSN (en mots), s'il se corrige, et quelles SORTES de documents il
/// a. L'etat de la DSN : « passee » (bilan OK), « refusee », « attente »,
/// « a_deposer », « en_retard » ; la meme lecture que les
/// rappels : le journal des depots, et lui seul.
///
/// CHAQUE MOIS SE LIT SEUL (« Le logiciel est doux ») : un mois qui ne se lit pas (une paie ecrite abimee, une piece, un
/// calcul impossible, une case qui manque) se rend nu, avec sa phrase ; les autres se rendent. Seule une panne du disque
/// remonte.
fn bilan_des_mois(m: &mut Moteur, ctx: &Contexte, mois: &[(Mois, Vec<NomDeFichier>)]) -> R<Vec<MoisDuBilan>> {
    let mut out = Vec::new();
    for (le_mois, noms) in mois {
        out.push(match bilan_du_mois(m, ctx, *le_mois, noms) {
            Ok(b) => b,
            Err(e) if e.classe == Some(Classe::Disque) => return Err(e),
            Err(e) => MoisDuBilan::nu(*le_mois, noms, format!("Ce mois ne se lit pas : {}", e.phrase)),
        });
    }
    Ok(out)
}

/// Le bilan du mois `le_mois` (`bilan_des_mois`), dont `noms` sont les fichiers.
fn bilan_du_mois(m: &mut Moteur, ctx: &Contexte, le_mois: Mois, noms: &[NomDeFichier]) -> R<MoisDuBilan> {
    {
        let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, le_mois)?;
        let limite = echeance.jour;
        let en_retard = echeance.passee_au(&ctx.aujourdhui);
        // L'ETAT DE LA DSN, lu une fois (`compagnie::etat_dsn`) ; une DSN a deposer passee l'echeance est en retard.
        let importe = compagnie::etat_dsn::importe(&ctx.racine, le_mois)?;
        let lu = compagnie::etat_dsn::du_mois(ctx, le_mois, None)?;
        let retard = lu.etat == Etat::ADeposer && en_retard;
        let depose = lu.deposee();
        let de_la_sorte = |s: documents::SorteDePieceProduite| noms.iter().any(|n| documents::SorteDePieceProduite::du_nom(n.texte()).is_some_and(|(x, _)| x == s));
        let mut sortes: Vec<&'static str> = SORTES.iter().filter(|(s, _)| de_la_sorte(*s)).map(|(_, l)| *l).collect();
        let (aem, mot_aem) = AEM_DE_L_ANCIEN_LOGICIEL;
        if noms.iter().any(|n| n.texte().starts_with(aem)) {
            sortes.push(mot_aem);
        }
        // Un mois tenu avant : ses bulletins sont ceux de l'ancien logiciel, que l'import garde.
        if importe && !import::mois::bulletins_repris(&ctx.racine, le_mois)?.is_empty() {
            sortes.insert(0, "bulletins repris");
        }
        // LES AER DE FRANCE TRAVAIL : rangees avec les documents des salaries (`sorties/<mois>/envoi/`), pas a la
        // racine du mois ; elles se nomment avec les autres sortes.
        let aer = documents::PieceDuMois::Aer;
        if crate::documents::un_nom_commence_et_finit(&aer.dossier().dans(&ctx.racine, le_mois), aer.sorte().prefixe(), ".pdf")? {
            // Avant les AEM, ou en dernier.
            let rang = match sortes.iter().position(|s| *s == mot_aem) {
                Some(r) => r,
                None => sortes.len(),
            };
            sortes.insert(rang, "attestations AER");
        }
        // CE QUI RESTE A FAIRE DANS LA PAGE PAIE (Historique) : une DSN a deposer, refusee ou en
        // retard, ou une DSN passee dont la paie a change depuis, tant que l'annule-et-remplace est admis. Le reste se
        // lit dans l'Historique : un mois fini n'a plus de chemin vers la page Paie.
        let a_faire = match lu.etat {
            _ if importe => false,
            // La DSN de l'ancien logiciel, et la saisie qui ajoute au mois : ce qui reste a declarer va dans une DSN
            // suivante (`import::mois::dsn_de_l_ancien_logiciel`).
            Etat::Importee => import::mois::dsn_de_l_ancien_logiciel(ctx, le_mois)?.is_some_and(|d| d.engagements > 0),
            Etat::Passee => m.etat_remplacement(le_mois, None)?.a_envoyer,
            Etat::Attente => false,
            Etat::Refusee | Etat::SansTrace | Etat::ADeposer => true,
        };
        // CE QUE LA PASTILLE DIT, et sa couleur (`socle::ecran::couleur_de_l_etat`) : une DSN qui attend son bilan, ou dont
        // l'ancien logiciel garde la trace, n'est rien que la compagnie ait a faire -- une information, en gris.
        let (couleur, mots) = match lu.etat {
            _ if retard => ("orange", "DSN en retard"),
            Etat::Passee => ("vert", "DSN passée"),
            Etat::Attente => ("gris", "DSN partie, bilan attendu"),
            Etat::Refusee => ("rouge", "DSN refusée"),
            Etat::Importee => ("vert", "DSN déclarée avant PuppetPaie"),
            Etat::SansTrace => ("gris", "tenu avant PuppetPaie — sa DSN est celle de l'ancien logiciel"),
            Etat::ADeposer => ("orange", "DSN à déposer"),
        };
        let quand = if depose { lu.quand.clone() } else { None };
        Ok(MoisDuBilan {
            mois: le_mois,
            produit: !noms.is_empty(),
            dsn: Some(if retard { DsnDuBilan::EnRetard } else { DsnDuBilan::Lue(lu.etat) }),
            saisi_dit: "saisi",
            pastille: Some(socle::ecran::Pastille { couleur, mots: mots.to_string() }),
            depose_dit: match (depose, &quand) {
                (true, Some(q)) => Some(format!("le {}", q.jour().jj_mm_aaaa())),
                _ => None,
            },
            depose_le: quand,
            documents_dit: Some(if sortes.is_empty() { "aucun document".to_string() } else { sortes.join(" · ") }),
            echeance: Some(limite),
            echeance_depot: Some(echeance.dite()?),
            // Deposee, passee, et l'echeance derriere : la correction part dans
            // la DSN du mois courant.
            corrigeable: lu.partie() && en_retard && !importe,
            importe: Some(importe),
            a_faire,
            sortes,
            fichiers: noms.to_vec(),
            clos: Some(m.exercice_clos(le_mois.annee())?),
        })
    }
}

/// Ce que l'Historique montre de chaque mois.
fn historique(m: &mut Moteur, ctx: &Contexte) -> R<Vec<MoisDuBilan>> {
    let mut mois = mois_connus(&ctx.racine)?;
    // Les mois tenus avant ce logiciel sont des mois comme les autres : l'Historique les montre aussi.
    for im in import::mois::mois_importes(&ctx.racine)? {
        if !mois.iter().any(|(x, _)| *x == im) {
            mois.push((im, Vec::new()));
        }
    }
    mois.sort_by(|a, b| b.0.cmp(&a.0));
    bilan_des_mois(m, ctx, &mois)
}

// ----------------------------------------------------------- les referentiels

/// Ce que la liste du genre dit des cinq avantages en nature du bloc 54 (`AutreElementDeRevenuBrut::avantage_en_nature`).
fn genre_en_clair(a: AutreElementDeRevenuBrut) -> Option<&'static str> {
    match a {
        AutreElementDeRevenuBrut::AvantageRepas => Some("repas"),
        AutreElementDeRevenuBrut::AvantageLogement => Some("logement"),
        AutreElementDeRevenuBrut::AvantageVehicule => Some("véhicule"),
        AutreElementDeRevenuBrut::AvantageNtic => Some("NTIC"),
        AutreElementDeRevenuBrut::AvantageAutres => Some("autres"),
        AutreElementDeRevenuBrut::SommeVerseeParUnTiers
        | AutreElementDeRevenuBrut::FraisRemboursesAuForfait
        | AutreElementDeRevenuBrut::FraisPrisEnChargeParLEmployeur
        | AutreElementDeRevenuBrut::FraisRemboursesAuReel
        | AutreElementDeRevenuBrut::DeductionForfaitaireSpecifique
        | AutreElementDeRevenuBrut::Participation
        | AutreElementDeRevenuBrut::Interessement
        | AutreElementDeRevenuBrut::AbondementPee
        | AutreElementDeRevenuBrut::AbondementPei
        | AutreElementDeRevenuBrut::AbondementRetraiteCollectif
        | AutreElementDeRevenuBrut::TitresRestaurant
        | AutreElementDeRevenuBrut::TransportsPublics
        | AutreElementDeRevenuBrut::TransportsPersonnels
        | AutreElementDeRevenuBrut::DroitDAuteur
        | AutreElementDeRevenuBrut::DroitDeDoublage
        | AutreElementDeRevenuBrut::DroitDeRediffusion
        | AutreElementDeRevenuBrut::AvantagesDePreretraite
        | AutreElementDeRevenuBrut::CetVersUneEpargneRetraite
        | AutreElementDeRevenuBrut::PrestationsFamiliales
        | AutreElementDeRevenuBrut::FraisReintegresDfs
        | AutreElementDeRevenuBrut::CetEnPointsRafp
        | AutreElementDeRevenuBrut::ParticipationInteressementImmediats
        | AutreElementDeRevenuBrut::NouveauTypeA
        | AutreElementDeRevenuBrut::NouveauTypeB
        | AutreElementDeRevenuBrut::PpveVerseeImmediatement
        | AutreElementDeRevenuBrut::PpveAffecteeAUnPlan
        | AutreElementDeRevenuBrut::ServicesALaPersonne
        | AutreElementDeRevenuBrut::ChequesVacances
        | AutreElementDeRevenuBrut::CotisationPatronaleSante
        | AutreElementDeRevenuBrut::CotisationPrevoyanceRetraiteSupplementaire
        | AutreElementDeRevenuBrut::NouveauTypeC
        | AutreElementDeRevenuBrut::NouveauTypeD
        | AutreElementDeRevenuBrut::NouveauTypeE => None,
    }
}

/// L'aide du choix « Lieu » d'une ligne de frais.
const LIEU_AIDE: &str = "Où dort et mange le salarié : hors de la métropole, la limite exonérée change ; à l'étranger, c'est l'indemnité du pays pour la nuitée et les repas du jour.";

/// Un code et son libelle, dans une liste ou l'on choisit (le genre d'un avantage, le lieu d'une ligne de frais, un
/// metier, un motif) ; ce que le choix dit de lui-meme une fois choisi (`dit` : un motif de rupture) ; le groupe sous
/// lequel il se range dans la liste (`groupe` : la famille d'un metier), la liste etant deja dans l'ordre des groupes.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Choix {
    code: String,
    libelle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    defaut: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    groupe: Option<String>,
    /// Les parties d'une valeur faite de parties que ce choix ouvre (`catalogue::Forme::Parties`).
    #[serde(skip_serializing_if = "Option::is_none")]
    ouvre: Option<Vec<&'static str>>,
}

impl Choix {
    /// Un code et son libelle, sans defaut.
    pub(crate) fn de(code: impl std::fmt::Display, libelle: impl std::fmt::Display) -> Choix {
        Choix { code: code.to_string(), libelle: libelle.to_string(), defaut: None, dit: None, groupe: None, ouvre: None }
    }

    /// Ce que le choix dit de lui-meme une fois choisi.
    pub(crate) fn disant(self, dit: Option<String>) -> Choix {
        Choix { dit, ..self }
    }

    /// Le groupe ou il se range.
    pub(crate) fn sous(self, groupe: Option<String>) -> Choix {
        Choix { groupe, ..self }
    }
}

/// CE QUE LES CASES D'UNE CARTE QUI SE CHOISISSENT DANS LES DONNEES OFFRENT (`Forme::ChoixRendu`), sous la cle de leur
/// case : les salaries et les spectacles courants, les metiers du deroulant (ceux de Configuration › Métiers, la seule
/// liste : `metiers::metiers`), les etablissements autres que le siege (aucun choisi : le siege), la transaction d'une
/// fin de contrat (S21.G00.62.008), le type d'un preavis (ceux qui se saisissent, S21.G00.62.006 ; chacun ouvre les dates
/// quand il en a). Le motif de la fin et les modeles de contrat, propres au contrat, sont rendus avec la carte de chaque
/// paie (`mois::CarteRendue`) ; les motifs de recours, avec la nature (`NatureDeLaPage::choix`).
#[derive(Debug, Clone, serde::Serialize)]
struct ChoixDesCases {
    salarie: Vec<Choix>,
    spectacle: Vec<Choix>,
    metier: Vec<Choix>,
    etablissement: Vec<Choix>,
    transaction: Vec<Choix>,
    preavis: Vec<Choix>,
}

/// LES PARTIES DES CASES D'UNE CARTE FAITES DE PARTIES, sous la cle de leur case, tirees de leur forme
/// (`catalogue::Forme::de_la_case`).
fn parties_des_cases() -> BTreeMap<&'static str, Vec<crate::catalogue::PartieEcrite>> {
    engagement::saisie::Champ::TOUS
        .iter()
        .filter_map(|c| match Forme::de_la_case(*c) {
            Forme::Parties(p) => Some((c.nom(), p.iter().map(crate::catalogue::Partie::ecrite).collect())),
            _ => None,
        })
        .collect()
}

/// UNE RUBRIQUE QUI PAIE, telle que la carte d'un engagement la propose : ce que le catalogue en dit, et ce que la page en
/// tire sans rien decider (ses genres, ses lieux, ses formes, ce qu'elle promet sans que rien ne le tienne, ses mots).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct RubriqueDeLaPage {
    lieu_aide: Option<&'static str>,
    code: droit::catalogue::CodeDeRubrique,
    libelle: String,
    unite: droit::catalogue::Unite,
    nature: droit::catalogue::NatureDeRubrique,
    saisie: Vec<droit::catalogue::CaseSaisie>,
    sens: socle::grandeurs::Signe,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    heures_forfait: Option<socle::grandeurs::HeuresParUnite>,
    plafond: Option<droit::catalogue::Unite>,
    aem: Option<droit::catalogue::Unite>,
    dsn_type: Option<droit::catalogue::DsnTypeDeRubrique>,
    dsn_autres_bruts: Option<AutreElementDeRevenuBrut>,
    annexe: Option<droit::emplois::AnnexeChomage>,
    soumis: bool,
    imposable: bool,
    conges_spectacles: bool,
    compte_pour: droit::catalogue::ComptePour,
    #[serde(serialize_with = "socle::grandeurs::ecrit::entier::option::serialize")]
    plafond_mensuel: Option<socle::grandeurs::Cachets>,
    plafond_annuel_heures: Option<droit::catalogue::PlafondAnnuelHeures>,
    origine: droit::catalogue::OrigineDeRubrique,
    /// CE QUE SES CLES QUI SE CHOISISSENT OFFRENT (`Forme::ChoixRendu`), sous le nom de la cle : le genre d'un avantage en
    /// nature, le lieu d'une ligne de nuitee ou de repas ; une cle qu'elle ne dit pas n'y est pas.
    choix: BTreeMap<&'static str, Vec<Choix>>,
    /// CE QUE LA RUBRIQUE PROMET SANS QUE RIEN NE LE TIENNE.
    non_appliques: Vec<&'static str>,
    source: String,
    dit: Option<String>,
    /// Les mots entre les cases de sa ligne : « quantité à taux € », ou « base € à taux % » pour un pourcentage.
    mots: [&'static str; 2],
    /// LES CLES QU'UNE LIGNE DE CETTE RUBRIQUE DIT, toutes et seules (`engagement::ligne::FormeSaisie`), chacune sa
    /// forme : la page les ecrit, `null` pour une case vide, et fait cocher ses jours dans le calendrier.
    cles: Vec<CleDeLaLigne>,
    /// LA CLE DE SA QUANTITE, dans l'unite de la rubrique (`engagement::ligne::QuantiteDeLigne::cle` : `{"cachets": 2}`),
    /// quand une ligne en dit une (`cles`).
    quantite: Option<&'static str>,
}

/// Une cle d'une ligne de paie telle que la page la saisit : son nom, sa forme (`Forme::de_la_cle`) et sa place
/// (`engagement::ligne::Cle::place` : le territoire d'un repas ou d'une nuitee sous « Avancé »).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct CleDeLaLigne {
    cle: &'static str,
    libelle: &'static str,
    forme: Forme,
    place: engagement::carte::PlaceDeLaCase,
}

/// Ce que l'ecran ecrit au-dessus de la case d'une cle d'une ligne de paie.
fn libelle_de_la_cle(c: engagement::ligne::Cle) -> &'static str {
    use engagement::ligne::Cle as C;
    match c {
        C::Rubrique => "Rubrique",
        C::Quantite => "Quantité",
        C::Taux => "Taux",
        C::Heures => "Heures",
        C::Base => "Base",
        C::TauxMajoration => "Pourcentage",
        C::Montant => "Montant",
        C::Genre => "Genre",
        C::Territoire => "Hors métropole",
        C::Cases => "Cases",
        C::Lieu => "Lieu",
        C::Dates => "Jours",
        C::Planning => "Mois suivants",
    }
}

pub(crate) fn rubrique_de_la_page(r: &Rubrique) -> R<RubriqueDeLaPage> {
    let saisie = match engagement::ligne::FormeSaisie::de(r)? {
        Some(f) => f.cles(),
        None => return Err(socle::refus!("La rubrique « {} » ne se saisit pas : le moteur la pose lui-même. Le signaler au développeur.", r.libelle)),
    };
    let cles: Vec<CleDeLaLigne> = saisie.iter().map(|c| CleDeLaLigne { cle: c.nom(), libelle: libelle_de_la_cle(*c), forme: Forme::de_la_cle(*c), place: c.place() }).collect();
    let non_appliques: Vec<&'static str> = [
        ("annexe", r.annexe.is_some()),
        ("plafond", r.plafond.is_some()),
        ("aem", r.aem.is_some()),
        ("dsn_type", r.dsn_type.is_some()),
    ]
    .into_iter()
    .filter(|(_, dite)| *dite)
    .map(|(c, _)| c)
    .collect();
    // Les valeurs de `genre`, quand la rubrique le demande : les cinq avantages en nature du bloc 54.
    let avec_genre = r.saisie.contains(&droit::catalogue::CaseSaisie::Genre) &&r.dsn_autres_bruts.is_some_and(AutreElementDeRevenuBrut::avantage_en_nature);
    let genres: Vec<Choix> = if avec_genre {
        AutreElementDeRevenuBrut::TOUS
            .iter()
            .filter_map(|g| genre_en_clair(*g).map(|libelle| Choix { defaut: Some(Some(*g) == r.dsn_autres_bruts), ..Choix::de(g.code(), libelle) }))
            .collect()
    } else {
        vec![]
    };
    // LE LIEU D'UNE LIGNE DE NUITEE OU DE REPAS (`engagement::LignePaie::territoire`), sous « Avancé » : la metropole d'abord (rien
    // a choisir), l'outre-mer et Monaco, puis chaque lieu de la table des missions a l'etranger (`droit::missions::lieux`).
    let choix = |code: &str, libelle: &str| Choix::de(code, libelle);
    let lieux: Vec<Choix> = if [engagement::NUITEE, engagement::REPAS].contains(&r.code) {
        std::iter::once(choix("", "Métropole"))
            .chain(engagement::TerritoireHorsMetropole::TOUS.into_iter().map(|t| choix(t.texte(), t.dit())))
            .chain(droit::missions::lieux()?.into_iter().map(|(c, l)| choix(c.texte(), &l)))
            .collect()
    } else {
        vec![]
    };
    let mut offerts = BTreeMap::new();
    if !genres.is_empty() {
        offerts.insert(engagement::ligne::Cle::Genre.nom(), genres);
    }
    let lieu_aide = (!lieux.is_empty()).then_some(LIEU_AIDE);
    if !lieux.is_empty() {
        offerts.insert(engagement::ligne::Cle::Territoire.nom(), lieux);
    }
    Ok(RubriqueDeLaPage {
        quantite: engagement::ligne::QuantiteDeLigne::cle(r.unite).filter(|_| saisie.contains(&engagement::ligne::Cle::Quantite)),
        cles,
        lieu_aide,
        choix: offerts,
        code: r.code.clone(),
        libelle: r.libelle.clone(),
        unite: r.unite,
        nature: r.nature,
        saisie: r.saisie.clone(),
        sens: r.sens,
        heures_forfait: r.heures_forfait,
        plafond: r.plafond,
        aem: r.aem,
        dsn_type: r.dsn_type,
        dsn_autres_bruts: r.dsn_autres_bruts,
        annexe: r.annexe,
        soumis: r.soumis,
        imposable: r.imposable,
        conges_spectacles: r.conges_spectacles,
        compte_pour: r.compte_pour,
        plafond_mensuel: r.plafond_mensuel,
        plafond_annuel_heures: r.plafond_annuel_heures,
        origine: r.origine,
        non_appliques,
        source: r.source.clone(),
        dit: r.dit.clone(),
        mots: if r.saisie.contains(&droit::catalogue::CaseSaisie::TauxMajoration) { ["€ à", "%"] } else { ["à", "€"] },
    })
}

/// UNE NATURE DE CONTRAT telle que l'ecran la propose : ce que `contrats-base.json` en dit, et ce que la page en tire sans
/// rien decider (les faits a demander et le signalement, motif par motif ; ses mots).
#[derive(Debug, Clone, serde::Serialize)]
struct NatureDeLaPage {
    cle: droit::natures::NatureDeContrat,
    libelle: String,
    aide: String,
    /// Les cases de sa carte, obligatoires ou facultatives, et ce que chaque motif de recours porte en plus, tirees du
    /// type (`crate::catalogue::natures_des_cartes`), chacune sa forme : ce que la page montre, et seulement cela ; une
    /// case de forme `date` s'y choisit dans le calendrier.
    carte: engagement::carte::NatureDecrite<Forme>,
    mensuelle: bool,
    duree_indeterminee: bool,
    /// Un permanent ne se rattache a AUCUN spectacle : le numero d'objet (S21.G00.40.050) est INTERDIT hors des natures 02
    /// et 10 (CCH-11).
    spectacle: bool,
    motif_rupture_defaut: Option<MotifDeRupture>,
    /// Ce qu'offrent les cases de sa carte qui se choisissent dans sa nature, sous la cle de leur case : ses motifs de
    /// recours (`motif_recours`). Le motif de la fin, qui depend du contrat, est rendu avec la carte de chaque paie
    /// (`mois::CarteRendue`), les faits que ce motif appelle aussi (ses cases ouvertes).
    choix: BTreeMap<&'static str, Vec<Choix>>,
    motif_recours_exige: bool,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    quotite_defaut: Option<socle::grandeurs::Heures>,
    /// Le bouton qui propose la duree d'un temps plein. Si la fin se signale, et le choix vide des motifs de fin, c'est
    /// le contrat qui le dit, pas sa nature (`mois::Rupture`).
    temps_plein_dit: Option<String>,
    unite_quotite: natures::UniteDeQuotite,
}

/// Les natures de contrat, telles que l'ecran les DECRIT : toutes, pour que toute carte se dessine (celle d'un permanent
/// engage avant que la case des permanents se decoche aussi). Tout vient de `contrats-base.json`. Ce qu'une carte neuve
/// PROPOSE se dit au mois (`mois::NouvelleCarte`, `engagement::permanents::Permanents::natures_neuves`).
fn natures() -> R<Vec<NatureDeLaPage>> {
    let toutes = natures::natures()?;
    let cartes = crate::catalogue::natures_des_cartes()?;
    // L'ordre est celui du fichier.
    let mut sortie = Vec::new();
    for n in toutes.iter() {
        let mut choix = BTreeMap::new();
        if let Some(motifs) = n.motifs_recours() {
            choix.insert(engagement::saisie::Champ::MotifRecours.nom(), motifs.iter().map(|m| Choix::de(m.code, &m.libelle)).collect());
        }
        let carte = cartes
            .iter()
            .cloned()
            .find(|d| d.nature == n.cle)
            .ok_or_else(|| erreur(Classe::Manque, format!("Le logiciel ne sait pas saisir un {}.", n.libelle)))?;
        sortie.push(NatureDeLaPage {
            cle: n.cle,
            libelle: n.libelle.clone(),
            aide: n.aide.clone(),
            carte,
            mensuelle: n.mensuelle(),
            duree_indeterminee: n.duree_indeterminee,
            spectacle: n.interdit(droit::rubrique!("S21.G00.40.050")).is_none_or(|s| s.is_empty()),
            motif_rupture_defaut: n.motif_rupture_par_defaut(),
            choix,
            motif_recours_exige: n.motif_recours_exige(),
            quotite_defaut: n.quotite_reference_defaut(),
            temps_plein_dit: n.quotite_reference_defaut().map(|h| format!("temps plein {} h", socle::texte::virgule(h))),
            unite_quotite: n.unite_quotite(),
        });
    }
    Ok(sortie)
}

/// Un niveau de complementaire sante que le bareme connait : son numero, sa cotisation (en % du PMSS) pour l'assure, le
/// conjoint et chaque enfant, et ce que le choix en dit.
#[derive(Debug, Clone, serde::Serialize)]
struct NiveauDeSante {
    niveau: droit::forme_bareme::NiveauDeGarantie,
    #[serde(serialize_with = "socle::grandeurs::ecrit::a_virgule::serialize")]
    assure: socle::grandeurs::Taux,
    #[serde(serialize_with = "socle::grandeurs::ecrit::a_virgule::serialize")]
    conjoint: socle::grandeurs::Taux,
    #[serde(serialize_with = "socle::grandeurs::ecrit::a_virgule::serialize")]
    enfant: socle::grandeurs::Taux,
    libelle: String,
}

/// Les niveaux de garantie que le bareme connait, pour la fiche salarie (`releve_svp_sante` de la rubrique `mutuelle` des
/// permanents), lus dans le bareme du jour avant son controle (`Contexte::bareme_fondu` : la couche de la convention les
/// porte, l'effectif n'y change rien). Vide quand le bareme n'en porte pas ; `None` quand il ne se compose pas (le taux
/// AT/MP, la convention, la surcouche) : pourquoi, la configuration le dit (`ConfigurationDeLaPage::pour_payer`, que le
/// meme empilement refuse).
fn niveaux_sante(ctx: &Contexte) -> R<Option<Vec<NiveauDeSante>>> {
    let pile = match ctx.bareme_fondu(&ctx.aujourdhui) {
        Ok(p) => p,
        Err(e) if e.dit_un_manque() => return Ok(None),
        Err(e) => return Err(e),
    };
    // La complementaire sante est celle des permanents : la couche de leur nature.
    let releve = pile.par_nature.get(&droit::natures::Paie::Mensuelle).and_then(|p| p.rubriques_ajoutees.as_ref()?.iter().find(|r| r.id == droit::forme_bareme::MUTUELLE)).and_then(|r| r.releve_svp_sante.as_ref());
    // Une couche sans releve de la complementaire n'en dit aucun niveau.
    Ok(Some(releve
        .into_iter()
        .flat_map(|r| r.niveaux.iter())
        .map(|(n, t)| NiveauDeSante {
            niveau: n.clone(),
            assure: t.assure,
            conjoint: t.conjoint,
            enfant: t.enfant,
            libelle: format!("niveau {} — {} % du PMSS pour l'assuré seul", n, socle::grandeurs::ecrit::texte::dit(t.assure)),
        })
        .collect()))
}

/// Une convention collective que le bareme sait servir : son IDCC, son libelle, si sa couche de taux est RELEVEE
/// (`applicable`), et l'option qui la propose.
#[derive(Debug, Clone, serde::Serialize)]
struct ConventionServie {
    idcc: droit::fiche::Idcc,
    libelle: String,
    applicable: bool,
    option_dit: String,
}

/// Les conventions collectives que le bareme sait servir, pour la fiche (`Contexte::conventions_servies`). La page ne les
/// invente pas, et elle n'en invente pas le libelle.
fn conventions(ctx: &Contexte) -> R<Vec<ConventionServie>> {
    Ok(ctx
        .conventions_servies()?
        .into_iter()
        .map(|(idcc, c)| {
            let libelle = if c.libelle.trim().is_empty() { idcc.to_string() } else { c.libelle };
            let option_dit = format!("IDCC {} — {}{}", idcc, libelle, if c.applicable { "" } else { " (taux non relevés)" });
            ConventionServie { idcc, libelle, applicable: c.applicable, option_dit }
        })
        .collect())
}

// -------------------------------------------------------------------- l'etat

/// Un salarie des listes ou l'on choisit : sa cle et son nom. Ce que sa carte propose, la carte le dit
/// (`engagement::proposees`).
#[derive(Debug, Clone, serde::Serialize)]
struct SalarieDeLaListe {
    cle: IdSalarie,
    libelle: String,
}

/// Un spectacle des listes ou l'on choisit : sa cle, son titre et son numero d'objet, tels que sa fiche les porte.
#[derive(Debug, Clone, serde::Serialize)]
struct SpectacleDeLaListe {
    cle: droit::contexte::IdSpectacle,
    titre: String,
    numero_objet: Option<droit::fiche::NumeroObjetSpectacle>,
}

/// Un organisme de recouvrement que la DPAE fait choisir.
#[derive(Debug, Clone, serde::Serialize)]
struct OrganismeUrssaf {
    code: declarations::urssaf::CodeUrssafDpae,
    nom: String,
}

/// CE QUE LA PAGE LIT EN S'OUVRANT (`etat`) : la configuration verifiee et sa phrase, l'auteur, ce qu'une compagnie
/// fictive en dit, l'historique des mois, les listes ou l'on choisit et les referentiels livres avec le logiciel.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct EtatDeLaPage {
    phrase: Phrase,
    configuration: ConfigurationDeLaPage,
    auteur: compagnie::auteur::Auteur,
    simulation: bool,
    coulisses: Option<Coulisses>,
    note_fictive: Option<super::Encart>,
    envoi_sans_mot_de_passe: bool,
    supprimable: bool,
    suppression: &'static str,
    demo: Option<crate::accueil::suppression::MentionDemo>,
    commencer_par_l_import: bool,
    historique: Vec<MoisDuBilan>,
    salaries: Vec<SalarieDeLaListe>,
    spectacles: Vec<SpectacleDeLaListe>,
    choix: ChoixDesCases,
    rubriques: Vec<RubriqueDeLaPage>,
    emplois: Vec<emplois::MetierDuDeroulant>,
    /// Les metiers que la fiche d'un salarie choisit (`metiers::metiers_d_une_fiche`).
    emplois_d_une_fiche: Vec<emplois::MetierDuDeroulant>,
    familles_emplois: Vec<emplois::FamilleDeMetier>,
    emplois_autres: Vec<super::metiers::MetierAutre>,
    natures: Vec<NatureDeLaPage>,
    /// Les parties de chaque case faite de parties (`catalogue::Forme::Parties`), sous la cle de sa case.
    parties: BTreeMap<&'static str, Vec<crate::catalogue::PartieEcrite>>,
    aides: Aides,
    niveaux_sante: Option<Vec<NiveauDeSante>>,
    sante_hors_fiche: Option<String>,
    sante_aide: &'static str,
    conventions: Vec<ConventionServie>,
    organismes_urssaf: Vec<OrganismeUrssaf>,
    urssaf_deduit_dit: String,
    /// L'organisme Urssaf que le code postal de la fiche designe, s'il en designe un.
    urssaf_deduit: Option<declarations::urssaf::CodeUrssafDpae>,
    pays: Vec<droit::nationalite::PaysDeNationalite>,
    pays_naissance: Vec<socle::pays::PaysServi>,
}

/// La route `etat` : tout ce que la page lit en s'ouvrant.
pub(crate) fn etat(ctx: &Contexte) -> R<EtatDeLaPage> {
    let mut m = Moteur::nouveau(ctx);
    let configuration = configuration(ctx)?;
    let niveaux_sante = niveaux_sante(ctx)?;
    let historique = historique(&mut m, ctx)?;

    // Les listes ou l'on CHOISIT : les fiches courantes seulement. Une archive
    // reste dans `configuration`, ou on peut la relire et la reactiver.
    let mut cles: Vec<&IdSalarie> = ctx.salaries_actifs();
    cles.sort();
    let salaries: Vec<SalarieDeLaListe> = cles
        .into_iter()
        .map(|k| SalarieDeLaListe { cle: k.clone(), libelle: ctx.nomme(k) })
        .collect();
    let spectacles = ctx
        .spectacles_actifs()
        .into_iter()
        .map(|k| {
            let v = &ctx.spectacles[k];
            let titre = v.titre.clone().ok_or_else(|| socle::fichiers::manque("titre"))?;
            Ok(SpectacleDeLaListe { cle: k.clone(), titre, numero_objet: v.numero_objet.clone() })
        })
        .collect::<R<Vec<_>>>()?;
    let emplois = super::metiers::metiers(ctx)?;
    let choix = ChoixDesCases {
        salarie: salaries.iter().map(|s: &SalarieDeLaListe| Choix::de(&s.cle, &s.libelle)).collect(),
        spectacle: spectacles.iter().map(|s: &SpectacleDeLaListe| Choix::de(&s.cle, &s.titre)).collect(),
        // Les metiers, deja dans l'ordre de leurs familles (`metiers::metiers`), chacun sous la sienne.
        metier: emplois.iter().map(|m| Choix::de(&m.code, &m.libelle).sous(Some(m.famille.to_string()))).collect(),
        etablissement: choix_d_etablissement(ctx)?
            .into_iter()
            .flatten()
            .filter_map(|e| {
                let libelle = [Some(e.nom.clone()), e.periode_dit.clone()].into_iter().flatten().collect::<Vec<_>>().join(" — ");
                e.nic.map(|n| Choix::de(n, libelle))
            })
            .collect(),
        transaction: droit::preavis::base()?.transaction.codes.iter().map(|c| Choix { defaut: c.defaut, ..Choix::de(c.code, &c.libelle) }).collect(),
        // Le type d'un preavis, parmi ceux qui se saisissent : un type qui a des dates ouvre les parties du preavis qui
        // ne sont pas de toujours (`catalogue::PARTIES_DU_PREAVIS`).
        preavis: droit::preavis::base()?
            .types_preavis
            .codes
            .iter()
            .filter(|t| t.saisissable)
            .map(|t| Choix {
                ouvre: t.dates.then(|| crate::catalogue::PARTIES_DU_PREAVIS.iter().filter(|p| !p.toujours).map(|p| p.cle).collect()),
                ..Choix::de(t.code, &t.libelle)
            })
            .collect(),
    };

    // Le catalogue qui paie, deja dans l'ordre ou le proposer.
    let cat = catalogue::catalogue(&ctx.racine, &ctx.lue_au)?;
    // Une rubrique que le moteur pose (`saisie` vide) ne se propose pas.
    let rubriques: Vec<RubriqueDeLaPage> = cat.iter().filter(|r| !r.saisie.is_empty()).map(rubrique_de_la_page).collect::<R<_>>()?;

    // Les organismes de recouvrement, pour que le code Urssaf de la DPAE se
    // CHOISISSE au lieu de se taper ; et celui que le moteur prend quand la
    // fiche n'en dit rien : l'organisme du territoire.
    let urssaf = declarations::urssaf::base()?;
    let deduit = ctx.entreprise.code_postal.portee().and_then(|cp| urssaf.du_code_postal(cp));
    let organismes_urssaf: Vec<OrganismeUrssaf> = urssaf.organismes.iter().map(|o| OrganismeUrssaf { code: o.code.clone(), nom: o.nom.clone() }).collect();
    let branchement = compagnie::bridage::branchement(&ctx.racine)?;
    let joue = branchement.joue();

    Ok(EtatDeLaPage {
        phrase: dire_la_configuration(&configuration),
        configuration,
        // L'auteur, au pied de la page (`compagnie::auteur`).
        auteur: compagnie::auteur::auteur(),
        // UNE COMPAGNIE FICTIVE : les envois sont joues (`bridage::branchement`).
        simulation: joue,
        // Ce que la boite d'un envoi joue en dit.
        coulisses: joue.then(coulisses_fictives),
        note_fictive: joue.then(|| super::Encart::de(socle::ecran::EtatDEcran::Information, "Compagnie fictive", "Rien ne part : la réponse est là aussitôt ; en vrai, elle peut prendre quelques jours.")),
        // UN ENVOI N'A RIEN A PROUVER : joue (`preuve::verifier_preuve` ne demande rien a une fictive), ou refuse
        // avant toute preuve (une fictive ajoutee) ; la page ne demande alors pas le mot de passe du logiciel.
        envoi_sans_mot_de_passe: joue || branchement.refus().is_some(),
        // SUPPRIMER LA COMPAGNIE : la Configuration ne le propose que si elle se supprime -- jamais une
        // fictive livree, qui se remet a zero (`accueil::suppression::sorte`).
        supprimable: crate::accueil::suppression::supprimable(&compagnie::installation::IdDeCompagnie::installation_de(&ctx.racine), &ctx.racine)?,
        suppression: "Tout son dossier est effacé de ce poste : fiches, paies, documents, journaux.",
        // UNE COMPAGNIE CREEE EN DEMO PUIS GARDEE : sa mention, et les mois saisis en demo, ou la page la montre.
        demo: crate::accueil::suppression::mention(&ctx.racine)?,
        // UNE COMPAGNIE VENUE D'UN LOGICIEL PRECEDENT, DONT RIEN N'EST ENCORE REPRIS : la page s'ouvre sur l'import et
        // son guide (`import::guide::commencer_par_l_import`) ; ce qui manque pour demarrer se dit apres lui.
        commencer_par_l_import: import::guide::commencer_par_l_import(&ctx.racine)?,
        historique,
        salaries,
        spectacles,
        choix,
        rubriques,
        // Le catalogue des metiers, deja dans l'ordre des familles.
        emplois_d_une_fiche: super::metiers::metiers_d_une_fiche(&emplois),
        emplois,
        familles_emplois: emplois::familles()?,
        // Les metiers des autres conventions : on les AJOUTE a son deroulant,
        // ils n'y sont pas d'office.
        emplois_autres: super::metiers::metiers_autres_conventions(ctx)?,
        // LES NATURES DE CONTRAT : celles que la compagnie engage (une carte dit toujours la sienne).
        natures: natures()?,
        parties: parties_des_cases(),
        aides: Aides(AIDES),
        // Les niveaux de complementaire sante, pour la fiche d'un permanent.
        niveaux_sante,
        // Ou, quand la convention ne la fait pas choisir sur la fiche, d'ou elle vient (`bareme::sante_hors_fiche`).
        sante_hors_fiche: droit::bareme::sante_hors_fiche(&ctx.entreprise)?,
        // Ce que la fiche d'un permanent dit de sa complementaire sante, sous le choix du niveau.
        sante_aide: "Obligatoire pour un salarié au mois (art. L911-7) : sans elle, le logiciel refuse de le payer plutôt que de cotiser moins que le contrat. Elle se lit sur son bulletin d'adhésion.",
        // Les conventions collectives servies, pour la fiche entreprise.
        conventions: conventions(ctx)?,
        organismes_urssaf,
        urssaf_deduit_dit: match &deduit {
            Some(c) => format!("— code postal ({}) —", c),
            None => "— code postal : aucun organisme ne s'en déduit —".to_string(),
        },
        urssaf_deduit: deduit,
        // LES PAYS : la liste livree, triee par nom, chacun avec la codification UE que le moteur en
        // deduit. Le pays de naissance et le pays de nationalite s'y choisissent.
        pays: droit::nationalite::servir()?,
        // Le pays de naissance : la table « Pays » de la DSN, ou se lit S21.G00.30.015.
        pays_naissance: socle::pays::servir_naissance()?,
    })
}

/// L'aide d'une case (`AIDES`), pour une route qui la rend avec un geste ; une cle absente est un defaut du logiciel.
pub(crate) fn aide(cle: &str) -> R<&'static str> {
    AIDES.iter().find(|(k, _)| *k == cle).map(|(_, a)| *a).ok_or_else(|| erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : l'aide « {} » n'existe pas.", cle)))
}

/// Les aides des cases (`AIDES`), rendues comme un dictionnaire dans leur ordre.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(transparent)]
struct Aides(#[serde(serialize_with = "socle::ecriture::en_ordre")] &'static [(&'static str, &'static str)]);


/// LA VERSION DU LOGICIEL (`maj-etat`) : l'installee, le depot d'ou l'on recoit, la version
/// prete et ce qu'elle dit (`calcul`, `exigee`), la signature, la version d'avant, et la phrase.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Version {
    courante: compagnie::maj::VersionDuLogiciel,
    depot: compagnie::maj::DepotDesMisesAJour,
    prete: Option<compagnie::maj::VersionDuLogiciel>,
    calcul: Option<String>,
    exigee: Option<String>,
    signee: bool,
    avant: Option<compagnie::maj::VersionDuLogiciel>,
    phrase: Phrase,
}

/// La version du logiciel, ce qui est pret, ce
/// qui est configure. Ne va voir aucun depot.
pub(crate) fn version() -> R<Version> {
    let code = compagnie::maj::dossier_installe()?;
    // La mise a jour est celle de l'INSTALLATION : son reglage (`maj::reglage`), sa version prete (`maj::prete`).
    let inst = code.parent().map(Path::to_path_buf).ok_or_else(|| socle::echec::refus("Le logiciel n'est dans aucune installation."))?;
    let depot = compagnie::maj::reglage(&inst).map_err(|e| e.reclasser(Classe::Illisible))?;
    let prete = compagnie::maj::prete(&inst).map_err(|e| e.reclasser(Classe::Illisible))?;
    let courante_v = compagnie::maj::courante(&code)?;
    let avant = if code.join(".avant").join("donnees").is_dir() { Some(compagnie::maj::courante(&code.join(".avant"))?) } else { None };
    // Une version prete se dit dans la phrase ; sans elle, la phrase n'en dit rien.
    let dite: String = prete.iter().map(|p| format!(" La version {} est prête.", p.version)).collect();
    let (prete, calcul, exigee) = match prete {
        Some(p) => (Some(p.version), Some(p.calcul), Some(p.exigee)),
        None => (None, None, None),
    };
    Ok(Version {
        phrase: socle::phrase!("Version installée : {}.{}", courante_v, dite),
        courante: courante_v,
        // Le depot d'ou l'on recoit : celui qui est pose, sinon celui du moteur (`maj::DEPOT_PAR_DEFAUT`).
        depot: compagnie::maj::depot(depot),
        prete,
        calcul,
        exigee,
        // La cle publique de l'editeur est ecrite dans le moteur (`maj::CLE_PUBLIQUE`) : toujours posee.
        signee: true,
        avant,
    })
}

/// La configuration en une phrase : la compagnie, ses fiches, et ses erreurs s'il y en a.
fn dire_la_configuration(c: &ConfigurationDeLaPage) -> Phrase {
    let nom = match c.entreprise.donnees.fiche.nommee() {
        Some(n) => n.to_string(),
        None => "Compagnie sans nom".to_string(),
    };
    let erreurs: String = c.erreurs.iter().map(|p| format!(" ; {}", p.mots)).collect();
    socle::phrase!(
        "{} : {}, {}{}.",
        nom,
        socle::ecran::compte(c.salaries.len(), "salarié", "salariés"),
        socle::ecran::compte(c.spectacles.len(), "spectacle", "spectacles"),
        erreurs
    )
}

/// LE CIRCUIT D'UN COURRIEL JOUE PAR UNE COMPAGNIE FICTIVE, genre par genre (`circuits` de `courriels-fictifs`) : ce que
/// recoit le destinataire, ses etapes (bulles) et ce que chacune dit (notes), et ce qui se dit en attendant la reponse.
fn circuits_fictifs() -> Circuits {
    Circuits {
        signature: Circuit {
            titre: "Ce que reçoit le salarié",
            bulles: &["Le contrat part", "Il appuie sur « Je signe »", "Le contrat signé lui revient"],
            notes: &[
                "Un courriel par salarié, le contrat en pièce jointe et un bouton « Je signe ».",
                "Le bouton ouvre sa messagerie sur cette réponse toute écrite, avec son code unique : il n'a qu'à l'envoyer. Le logiciel la relève seul, à l'ouverture et avec la veille, et la garde comme preuve.",
                "Dès la signature relevée, le contrat contresigné part tout seul : le seul envoi sans clic du logiciel, puisque le clic est celui du salarié. La pastille du contrat passe au vert.",
            ],
            attente: Some("La signature du salarié fictif arrive une vingtaine de secondes après l'envoi."),
            creee: None,
        },
        fiche: Circuit {
            titre: "Ce que reçoit la personne",
            bulles: &["La demande part", "Elle appuie sur « Remplir ma fiche »", "Sa fiche se crée"],
            notes: &[
                "Un courriel avec un bouton « Remplir ma fiche », et les questions recopiées dans le texte pour les messageries qui n'affichent pas le bouton.",
                "Le bouton ouvre sa messagerie sur les questions, une par ligne : elle écrit après chaque deux-points et envoie. Seul ce qu'elle sait lui est demandé ; l'emploi reste à l'employeur.",
                "Sa réponse est relevée comme une signature : la fiche salarié se crée avec ce qu'elle a écrit, marquée « à compléter » pour l'emploi. Une réponse illisible laisse la case vide, et le dit.",
            ],
            attente: Some("La réponse de la personne fictive arrive une vingtaine de secondes après l'envoi."),
            creee: Some("Elle est dans Configuration › Salariés, avec ce que la personne a écrit ; il reste à choisir son emploi."),
        },
        documents: Circuit {
            titre: "Ce que reçoit le salarié",
            bulles: &["Les documents partent"],
            notes: &["Un courriel par salarié, avec ses documents en pièces jointes. L'envoi est noté au journal, et un second envoi le signale avant de repartir."],
            attente: None,
            creee: None,
        },
        courriel: Circuit {
            titre: "Le courriel envoyé",
            bulles: &["Le courriel part"],
            notes: &["Voici le message tel qu'il serait parti. Dans la compagnie fictive, il est écrit dans le dossier courriels-simules/ de la compagnie, et rien ne quitte le poste."],
            attente: None,
            creee: None,
        },
        sans_bouton: "Ce courriel n'a pas de bouton.",
    }
}

/// CE QU'UNE COMPAGNIE FICTIVE NE FAIT PAS, DIT AU LIEU D'ETRE TU (`coulisses` de la route `etat`, pour une fictive) : ce
/// que fait une compagnie reelle (la sauvegarde, `compagnie::sauvegarde` ; la mise a jour, `compagnie::maj`), et l'horloge
/// de la fictive (`horloge`).
fn coulisses_fictives() -> Coulisses {
    Coulisses {
        titre: "Les coulisses de la compagnie fictive",
        sections: [
            ["La sauvegarde", "Pour une compagnie réelle, à l'ouverture puis toutes les vingt minutes quand quelque chose a changé, le dossier de la compagnie (saisies, bulletins, contrats, déclarations, retours, journal) part dans son propre Google Drive. Seul ce qui a changé repart, et rien de ce qui est effacé sur le poste ne l'est dans le Drive. Les mots de passe, eux, ne quittent jamais l'ordinateur. Le logiciel ne voit dans le Drive que ce qu'il y a lui-même créé."],
            ["Les mises à jour", "Au lancement, le logiciel regarde si l'éditeur a publié une nouvelle version. Il la télécharge en arrière-plan, vérifie qu'elle est bien signée par l'éditeur, et l'installe au démarrage suivant, sans rien demander. Une version qui change un calcul de paie le dit en une phrase ; une version marquée « exigée » (une norme DSN qui change) bloque les dépôts réels tant qu'elle n'est pas installée."],
            ["Dans la compagnie fictive", "La sauvegarde ne tourne pas : aucun compte Google n'est lié, rien ne quitte le poste. La mise à jour, elle, est celle du logiciel : elle se fait de la même façon, qu'on ouvre une compagnie fictive ou réelle. Le bandeau dit « À jour » parce que la relève des retours, elle, est jouée."],
            ["L'horloge de la compagnie fictive", "La compagnie vit sans vous. À chaque ouverture, ce qui tombait depuis votre dernier passage a été fait par un administrateur fictif, chaque geste à sa date limite : le 1er du mois, le mois s'écrit ; le jour d'une embauche, sa DPAE part et son contrat revient signé ; le 15, la DSN du mois précédent part, puis les documents aux salariés ; fin janvier, l'année passée se clôt. Avant la date limite, c'est à vous : faites la DPAE deux jours avant, ou la DSN le 14, et l'horloge ne la refera pas. Ajoutez ou retirez un contrat : à votre retour, le mois est réécrit et tout est à jour."],
        ],
    }
}

/// Le circuit d'un genre de courriel joue : ce que recoit le destinataire, ses etapes, ce qu'elles disent, et ce qui se
/// dit en attendant la reponse et une fois la fiche creee.
#[derive(Debug, Clone, serde::Serialize)]
struct Circuit {
    titre: &'static str,
    bulles: &'static [&'static str],
    notes: &'static [&'static str],
    #[serde(skip_serializing_if = "Option::is_none")]
    attente: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    creee: Option<&'static str>,
}

/// Les circuits des courriels joues, genre par genre, et ce qu'on dit d'un courriel sans bouton.
#[derive(Debug, Clone, serde::Serialize)]
struct Circuits {
    signature: Circuit,
    fiche: Circuit,
    documents: Circuit,
    courriel: Circuit,
    sans_bouton: &'static str,
}

/// Les coulisses d'une compagnie fictive : un titre, et ses sections (titre, texte).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Coulisses {
    titre: &'static str,
    sections: [[&'static str; 2]; 4],
}

/// LES COURRIELS JOUES (`courriels-fictifs`), tels que la page les montre : sa phrase, les courriels et le dernier lu,
/// l'intro, le circuit de chaque genre et la phrase de chaque courriel.
#[derive(Debug, Clone, serde::Serialize)]
struct CourrielsFictifs {
    phrase: Phrase,
    courriels: Vec<envois::simulation_courriels::CourrielJoue>,
    dernier: Option<String>,
    intro: String,
    circuits: Circuits,
    phrases: Vec<String>,
}

/// La route `courriels-fictifs`.
fn courriels_fictifs(ctx: &Contexte, charge: &crate::simulation_courriels::ChargeCourrielsFictifs) -> R<CourrielsFictifs> {
    let joues = crate::simulation_courriels::courriels_fictifs(ctx, charge)?;
    let phrases: Vec<String> = joues
        .courriels
        .iter()
        .map(|m| match &m.sujet {
            Some(sujet) => format!("{} → {} : {}", m.de, m.a, sujet),
            None => format!("{} → {}", m.de, m.a),
        })
        .collect();
    // Ce que la fenetre d'une compagnie fictive en dit : combien sont joues, et le circuit de chaque genre.
    let n = phrases.len();
    let s = if n > 1 { "s" } else { "" };
    Ok(CourrielsFictifs {
        phrase: socle::phrase!("Courriels simulés : {}.", n),
        courriels: joues.courriels,
        dernier: joues.dernier,
        intro: format!("{} courriel{} joué{} — rien n'a quitté le poste.{}", n, s, s, if n > 1 { " Chacun reçoit les siens :" } else { "" }),
        circuits: circuits_fictifs(),
        phrases,
    })
}
