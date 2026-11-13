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
use crate::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use crate::etapes::Moteur;
use compagnie::etat_dsn::Etat;
use declarations::champs::{self, Valeur};
use droit::fiche::{CaseDeFiche, CaseEntreprise, CaseEtablissement, CaseSalarie, CaseSpectacle, NomDeCase, NomDeFiche};
use droit::autre_revenu_brut::AutreElementDeRevenuBrut;
use droit::catalogue::{self, Rubrique};
use droit::contexte::{Contexte, IdSalarie};
use droit::emplois::{self};
use droit::natures::{self, MotifDeRupture, Nature};
use droit::permanents;
use engagement::{self};
use socle::echec::{erreur, Classe, R};
use socle::fichiers::{dossier_du_code, NomDeFichier};
use socle::mois::Mois;
use crate::catalogue::{Ecran, Place};
use crate::fiches::{reporter, vue, SorteDeFiche};
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
    ("metier", "Le métier de ce contrat, s'il diffère de la fiche."),
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
    ("taux_cachet", "Le taux du cachet de ce contrat quand aucune ligne de cachet ne le dit."),
    ("jours", "Les jours travaillés que la DSN déclare (sinon les jours cochés)."),
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
    ("fiche_cle", "La clé de la fiche se fabrique à l'enregistrement."),
    ("signature_lourde", "Trop lourde : une image collée pèse tout son poids, préférez une image en ligne."),
    ("urssaf_changer", "D'après l'adresse de l'entreprise, vos déclarations vont à l'Urssaf de son code postal : c'est elle qui reçoit normalement DPAE et DSN. Une Urssaf qui ne vous gère pas peut faire rejeter vos déclarations. Voulez-vous vraiment la changer ?"),
    ("historique_titre", "Historique des valeurs"),
    ("historique_fiche", "Toutes les valeurs de chaque case, passées et à venir, avec leurs dates et d'où elles viennent ; en gras, celle d'aujourd'hui. Celles d'avant restent pour les déclarations déjà envoyées."),
    ("non_enregistres", "Vos changements ne sont pas encore enregistrés."),
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
    ("import_dossier", "Vide : le dossier d'import de la compagnie."),
    ("import_relire", "Un fichier ajouté depuis, un salarié ou une année qui manquait."),
    ("import_relire_note", "Les fiches qui existent déjà ne sont pas touchées ; ce que les DSN ou les bulletins disaient autrement est seulement signalé. Ce qui est déjà repris ne se double pas."),
    ("import_relire_intro", "Mettez dans ce dossier ce qui manquait, puis « Aperçu ». Rien n'est écrit avant « Reprendre »."),
    ("import_intro", "Mettez dans le dossier ci-dessous tout ce que votre ancien logiciel vous a laissé, puis « Importer » : PuppetPaie crée la fiche de la compagnie, ses salariés, ses spectacles et leurs contrats avec ce que les pièces disent."),
    ("import_natures", "Un bulletin de paie ne dit pas si le contrat est un CDI ou un CDD. Dites-le pour chacun : la DSN et les documents de fin de contrat en dépendent."),
    ("import_numero", "Le numéro du contrat : la rubrique S21.G00.40.009 de sa dernière DSN."),
    ("import_quotite", "La durée du travail, en heures par mois : la rubrique S21.G00.40.013 de la DSN."),
    ("import_ecarts", "Le bulletin fait foi : c'est son montant qui s'écrit, l'écart reste noté ici."),
    ("import_copies", "Les fichiers lus sont copiés dans l'historique de la compagnie ; vous pouvez vider le dossier d'import."),
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
        rend: "configuration (les fiches vérifiées), historique (les mois), les listes où l'on choisit, les référentiels, auteur (phrase, nom, adresse)",
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
        rend: "entreprise, salaries, spectacles, leurs anomalies, l'identité, la description des cases",
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
            fn phrase(&self) -> &str {
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
    titre_dit: String,
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
    historique: Vec<TempsDeLaCase<Valeur>>,
}

/// Une case seule, ecrite comme une fiche qui ne porterait qu'elle.
#[derive(serde::Serialize)]
#[serde(transparent, bound(serialize = "T: serde::Serialize"))]
struct UneCase<'a, T>(#[serde(serialize_with = "socle::ecriture::en_ordre")] [(CaseDeFiche, &'a Option<T>); 1]);

/// LES TEMPS DE CHAQUE CASE DATEE de la fiche `brute` a `jour` (`droit::fiche::temps_au`), chaque valeur dans la forme
/// que sa ligne de `champs-base.json` lui donne ; une case qui n'a qu'une valeur n'en a pas. La page les met en page.
fn temps_de<F: SorteDeFiche>(brute: Option<&F>, jour: &socle::date::Jour) -> R<Vec<TempsDeLaCase<Valeur>>> {
    struct Toutes {
        nom: NomDeFiche,
        listes: Vec<(CaseDeFiche, R<Vec<droit::fiche::Datee<Valeur>>>)>,
    }
    impl droit::fiche::VoirListe for Toutes {
        fn liste<T: serde::Serialize>(&mut self, cle: CaseDeFiche, liste: &[droit::fiche::Datee<T>]) {
            let lue = liste
                .iter()
                .map(|e| {
                    let valeur = champs::Saisie::de_la_fiche(self.nom, &UneCase([(cle, &e.valeur)]))?.valeur(cle).cloned();
                    Ok(droit::fiche::Datee { depuis: e.depuis, valeur, note: e.note.clone() })
                })
                .collect();
            self.listes.push((cle, lue));
        }
    }
    // Une fiche absente n'a aucune case datee.
    let mut t = Toutes { nom: F::NOM, listes: Vec::new() };
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
fn temps_des_etablissements(ctx: &Contexte, jour: &socle::date::Jour) -> R<Option<TempsDeLaCase<Valeur>>> {
    type Cases = Vec<(CaseDeFiche, Vec<droit::fiche::Datee<Valeur>>)>;
    let Some(liste) = ctx.entreprise_brute().etablissements.as_deref() else { return Ok(None) };
    let principal = ctx.entreprise.nic_principal();
    let mut par_nic: Vec<(droit::fiche::Nic, Cases)> = Vec::new();
    for version in droit::fiche::valeurs_datees(liste) {
        let lue = champs::Saisie::de_la_fiche(NomDeFiche::Entreprise, &UneCase([(CaseEntreprise::Etablissements.into(), &version.valeur)]))?;
        let Some(Valeur::Lignes(lignes)) = lue.case(CaseEntreprise::Etablissements) else { continue };
        // Une ligne de la saisie par etablissement de la liste, dans son ordre : le NIC se lit dans l'etablissement.
        for (e, l) in version.valeur.iter().flatten().zip(lignes) {
            let Some(nic) = e.nic.dite().filter(|n| Some(*n) != principal).cloned() else { continue };
            if !par_nic.iter().any(|(n, _)| *n == nic) {
                par_nic.push((nic.clone(), Vec::new()));
            }
            let Some((_, cases)) = par_nic.iter_mut().find(|(n, _)| *n == nic) else { continue };
            for (cle, _) in l.cases() {
                if *cle != CaseEtablissement::TauxAtmp.into() && !cases.iter().any(|(k, _)| k == cle) {
                    cases.push((*cle, Vec::new()));
                }
            }
            for (cle, datees) in cases.iter_mut() {
                let valeur = l.valeur(*cle).cloned();
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
        let libelle = cases.iter().find(|(k, _)| *k == CaseEtablissement::Libelle.into()).and_then(|(_, d)| d.last()).and_then(|d| d.valeur.as_ref()).and_then(Valeur::mot);
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
    historique: Vec<TempsDeLaCase<Valeur>>,
    ent: &champs::Saisie,
    p: &engagement::permanents::Permanents,
) -> R<VueFiche<F::Plate, F::Cle>> {
    let nom = F::NOM;
    let plate = F::retenue(plate);
    let lue = vue::<F>(&plate)?;
    let anomalies = champs::verifier_fiche(nom, &lue)?;
    let comble = lue.comblee(nom)?;
    let mut montree = plate;
    reporter::<F>(&mut montree, &lue, &comble)?;
    let declare = champs::apercu_derive(nom, &comble)?;
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
    let archivee = comble.case(CaseSalarie::Actif) == Some(&Valeur::OuiNon(false));
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
    let lu = |k: NomDeCase| comble.mot(k).map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    // Un spectacle dont l'import n'a lu que le numero d'objet porte le titre « A COMPLETER » : la page le range a part.
    let sans_titre = nom == NomDeFiche::Spectacle && lu(CaseSpectacle::Titre.into()).is_some_and(|t| t.eq_ignore_ascii_case(import::proposee::A_COMPLETER));
    // Ce que l'en-tete de la fiche dit : le titre d'un spectacle, et les champs a corriger.
    let titre_dit = match (cle, lu(CaseSpectacle::Titre.into())) {
        (None, _) => "Nouveau spectacle".to_string(),
        (Some(_), _) if sans_titre => "Spectacle à nommer".to_string(),
        (Some(_), Some(t)) => t,
        (Some(c), None) => F::cle_dite(c),
    };
    let erreurs_titre = match erreurs {
        0 => None,
        1 => Some("Un champ à corriger".to_string()),
        n => Some(format!("{} champs à corriger", n)),
    };
    let erreurs_note = if erreurs > 1 { "marqués ci-dessous" } else { "marqué ci-dessous" };
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
    let super::fiches::Montres { cases: montres, pieces_ouvertes } = super::fiches::montres(ctx, nom, cle.and_then(F::salarie), &comble, ent, p)?;
    // L'organisme Urssaf que le code postal de la compagnie donne : le cas ordinaire du code Urssaf.
    let du_code_postal = match ctx.entreprise.code_postal.portee() {
        Some(cp) => declarations::urssaf::base()?.du_code_postal(cp),
        None => None,
    };
    let ecarts = champs::fiche(nom)?.iter().filter(|c| c.ecart(&comble, du_code_postal.as_deref())).map(|c| c.cle).collect();
    Ok(VueFiche {
        quoi_dit,
        geste_dit,
        matricule_dit,
        titre_dit,
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


/// Un etablissement qu'un engagement choisit : son NIC (vide : le siege), son nom, son ouverture et son dernier jour s'il
/// est ferme, et ce que l'option en dit.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ChoixEtablissement {
    periode_dit: Option<String>,
    /// Le NIC de l'etablissement ; aucun pour le siege (un engagement du siege n'en ecrit pas).
    nic: Option<droit::fiche::Nic>,
    nom: String,
    ouverture: Option<String>,
    fermeture: Option<String>,
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
        out.push(ChoixEtablissement { periode_dit, nom, nic: Some(nic), ouverture: e.ouverture.map(|j| j.iso()), fermeture: fermeture.map(|j| j.iso()) });
    }
    Ok(Some(out))
}

/// Les lignes des autres etablissements demandent-elles leur versement mobilite ? Oui quand le siege en paie un : le
/// controle des etablissements l'exige alors de chacun (`droit::contexte::controler_etablissements`). Un bareme du jour qui
/// ne se compose pas ne le demande pas : la configuration s'ouvre quand meme (c'est la qu'on la corrige), et dit pourquoi
/// (`ConfigurationDeLaPage::bareme`).
fn versement_mobilite_des_etablissements(ctx: &Contexte) -> (bool, Option<String>) {
    match ctx.bareme_fondu(&ctx.aujourdhui) {
        Ok(b) => (matches!(b.versement_mobilite, Some(droit::forme_bareme::VersementMobiliteEcrit::Ecrit(_))), None),
        Err(e) => (false, Some(format!("Le barème d'aujourd'hui ne se compose pas : {}", e.phrase))),
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
    let code = dossier_du_code();
    let Some(logiciel) = code.parent() else { return code };
    match logiciel.parent().map(|p| p.join("prive")) {
        Some(voisin) if voisin.join(socle::fichiers::CONFIG).is_dir() => voisin,
        _ => logiciel.to_path_buf(),
    }
}

/// `DOCUMENTS_IDENTITE` : ou va chaque image.
fn documents_identite(quoi: &str) -> &'static [&'static str] {
    match quoi {
        "signature" => &[
            "les contrats d'engagement", "l'AEM en PDF", "l'attestation employeur (AER)",
            "le certificat Congés Spectacles",
            "le certificat de travail et le reçu pour solde de tout compte",
        ],
        "cachet" => &["l'attestation employeur (AER), case « Cachet de l'entreprise »"],
        "logo" => &[
            "les bulletins de paie", "les contrats d'engagement",
            "le certificat de travail et le reçu pour solde de tout compte",
        ],
        _ => &[],
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
    pub(crate) octets: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) format: Option<String>,
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
    let image = |quoi: &str| -> R<ImageIdentite> {
        let pose = [".png", ".jpg", ".svg"].iter().map(|ext| dossier.join(format!("{}{}", quoi, ext))).find(|f| f.exists());
        let mut e = ImageIdentite {
            pose: pose.is_some(),
            automatique: quoi == "cachet",
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
            Some(f) => {
                let st = std::fs::metadata(&f).map_err(|x| socle::echec::du_disque(&f, &x).reclasser(Classe::Disque))?;
                e.octets = Some(st.len());
                e.format = f.extension().map(|x| x.to_string_lossy().into_owned());
                // Datee hors du calendrier, l'image posee se dit sans sa date.
                e.quand = socle::paris::mtime(&st).map(socle::paris::Moment::a_la_seconde);
                e.fichier = Some(f);
            }
        }
        Ok(e)
    };
    let mut signature = image("signature")?;
    // LA SIGNATURE EST CELLE D'UNE PERSONNE : celle que la fiche Entreprise nomme sous « Qui signe ». Si l'image a ete
    // posee pour une autre, la page le dit.
    let signataire = signataire_de(&ctx.entreprise);
    let pour = if signature.pose { registre_identite(&dossier)?.signature.and_then(|s| s.de) } else { None };
    signature.autre_personne = Some(matches!((&pour, &signataire.nom), (Some(p), Some(n)) if p != n));
    signature.posee_pour = pour;
    signature.signataire = Some(signataire);
    Ok(Identite { signature, cachet: image("cachet")?, logo: image("logo")? })
}

impl Identite {
    /// Les trois images, dans l'ordre ou l'ecran les montre.
    pub(crate) fn images_mut(&mut self) -> [(&'static str, &mut ImageIdentite); 3] {
        [("signature", &mut self.signature), ("cachet", &mut self.cachet), ("logo", &mut self.logo)]
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
    /// Pourquoi le bareme du jour ne se compose pas, s'il ne se compose pas.
    bareme: Option<String>,
    /// CE QUI EST POSE : signature et logo, avec leur date.
    identite: Identite,
    invitation: Invitation,
    boite_liee: bool,
    fiches_attendues: Vec<FicheAttendue>,
}

/// La route `configuration` : la configuration et sa phrase.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ConfigurationDite {
    phrase: String,
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
    let ent = champs::Saisie::de_la_fiche(NomDeFiche::Entreprise, &ctx.entreprise)?;
    let autres = ctx.etablissements_dits();
    let montree = droit::fiche::ChampsEntreprise { etablissements: (!autres.is_empty()).then_some(autres), ..ctx.entreprise.clone() };
    let mut temps = temps_de(Some(ctx.entreprise_brute()), &jour)?;
    temps.extend(temps_des_etablissements(ctx, &jour)?);
    // LES PERMANENTS DU JOUR DE L'ECRAN : ce qui ne vaut que pour eux se montre s'ils se proposent ce jour-la, ou qu'un
    // permanent est du (`engagement::permanents`).
    let p = engagement::permanents::Permanents::de(ctx, &jour, &jour)?;
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
    let ans: Vec<i32> = sans_titre.iter().filter_map(|s| s.numero_objet.dite().and_then(droit::fiche::NumeroObjetSpectacle::annee_de_la_demande)).collect();
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
    let compteurs_dit = compteurs.iter().filter(|(k, _)| k.len() == 4 && k.chars().all(|c| c.is_ascii_digit())).max_by(|a, b| a.0.cmp(b.0)).map(|(an, c)| {
        let n = |x: Option<i64>| match x {
            Some(n) => n.to_string(),
            None => "aucun".to_string(),
        };
        format!("compteurs {} — bulletin {} · CS {}", an, n(c.bulletin), n(c.certificat_cs))
    });
    let fiches_attendues = fiches_attendues(ctx)?;
    let liste = |un: &str, les: &str| MotsDeLaListe { nouvelle: format!("+ un {}", un), aucun: format!("Aucun {} actif", un), archives: format!("Afficher les {} archivés", les) };
    let boite_liee = boite_liee(ctx)?;
    let (versement_mobilite_des_etablissements, bareme) = versement_mobilite_des_etablissements(ctx);
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
        bareme,
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

/// `SORTES` : les sortes de documents d'un mois, par le debut de leur nom.
const SORTES: [(&str, &str); 6] =
    [("Bulletins_", "bulletins"), ("Contrats_", "contrats"), ("DPAE_", "DPAE"), ("DSN_", "DSN"), ("CertificatsCS_", "certificats CS"), ("AEM_", "AEM")];

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
    for d in socle::fichiers::nommes_d_un_mois(&racine.join("sorties"), None, false)? {
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
    /// Un mois dont la DSN ne se lit pas : ses fichiers, et rien a corriger.
    fn nu(mois: Mois, fichiers: &[NomDeFichier]) -> MoisDuBilan {
        MoisDuBilan {
            mois,
            produit: !fichiers.is_empty(),
            dsn: None,
            saisi_dit: "saisi",
            pastille: None,
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
fn bilan_des_mois(m: &mut Moteur, ctx: &Contexte, mois: &[(Mois, Vec<NomDeFichier>)]) -> R<Vec<MoisDuBilan>> {
    let mut out = Vec::new();
    for &(le_mois, ref noms) in mois {
        let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, le_mois).map_err(|m| m.reclasser(Classe::Illisible))?;
        let limite = echeance.jour;
        let en_retard = echeance.passee_au(&ctx.aujourdhui);
        // L'ETAT DE LA DSN, lu une fois (`compagnie::etat_dsn`) ; une DSN a deposer passee l'echeance est en retard.
        let importe = compagnie::etat_dsn::importe(&ctx.racine, le_mois)?;
        let lu = compagnie::etat_dsn::du_mois(ctx, le_mois, None)?;
        let retard = lu.etat == Etat::ADeposer && en_retard;
        let depose = lu.deposee();
        let mut sortes: Vec<&'static str> = SORTES.iter().filter(|(p, _)| noms.iter().any(|n| n.texte().starts_with(p))).map(|(_, l)| *l).collect();
        // Un mois tenu avant : ses bulletins sont ceux de l'ancien logiciel, que l'import garde.
        if importe && !import::mois::bulletins_repris(&ctx.racine, le_mois)?.is_empty() {
            sortes.insert(0, "bulletins repris");
        }
        // LES AER DE FRANCE TRAVAIL : rangees avec les documents des salaries (`sorties/<mois>/envoi/`), pas a la
        // racine du mois ; elles se nomment avec les autres sortes.
        if crate::documents::un_nom_commence_et_finit(&ctx.racine.join("sorties").join(le_mois.to_string()).join("envoi"), "AER_", ".pdf")? {
            // Avant les AEM, ou en dernier.
            let rang = match sortes.iter().position(|s| *s == "AEM") {
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
        out.push(MoisDuBilan {
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
            echeance_depot: Some(echeance.dite()),
            // Deposee, passee, et l'echeance derriere : la correction part dans
            // la DSN du mois courant.
            corrigeable: lu.partie() && en_retard && !importe,
            importe: Some(importe),
            a_faire,
            sortes,
            fichiers: noms.clone(),
            clos: Some(m.exercice_clos(le_mois.annee_civile())?),
        });
    }
    Ok(out)
}

/// Ce que l'Historique montre de chaque mois.
fn historique(m: &mut Moteur, ctx: &Contexte) -> R<(Vec<MoisDuBilan>, Option<String>)> {
    let mut mois = mois_connus(&ctx.racine)?;
    // Les mois tenus avant ce logiciel sont des mois comme les autres : l'Historique les montre aussi.
    for im in import::mois::mois_importes(&ctx.racine)? {
        if !mois.iter().any(|(x, _)| *x == im) {
            mois.push((im, Vec::new()));
        }
    }
    mois.sort_by(|a, b| b.0.cmp(&a.0));
    // Une fiche entreprise incomplete (effectif, echeance) n'empeche pas la page de s'ouvrir : la liste nue, et pourquoi
    // le bilan des mois ne se fait pas.
    Ok(match bilan_des_mois(m, ctx, &mois) {
        Ok(b) => (b, None),
        Err(e) => (mois.iter().map(|(nom, noms)| MoisDuBilan::nu(*nom, noms)).collect(), Some(format!("Le bilan des mois ne se fait pas : {}", e.phrase))),
    })
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

/// Un code et son libelle, dans une liste ou l'on choisit (le genre d'un avantage, le lieu d'une ligne de frais).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Choix {
    code: String,
    libelle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    defaut: Option<bool>,
}

/// Un nombre lu du fichier, rendu entier quand il l'est (`12`, pas `12.0`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct NombreLu(socle::num::Decimal);

impl serde::Serialize for NombreLu {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use rust_decimal::prelude::ToPrimitive;
        match self.0.fract().is_zero().then(|| self.0.to_i64()).flatten().filter(|i| i.unsigned_abs() < 1_000_000_000_000_000) {
            Some(i) => s.serialize_i64(i),
            None => socle::forme::ecrire_a_virgule(&self.0, s),
        }
    }
}

/// UNE RUBRIQUE QUI PAIE, telle que la carte d'un engagement la propose : ce que le catalogue en dit, et ce que la page en
/// tire sans rien decider (ses genres, ses lieux, ses formes, ce qu'elle promet sans que rien ne le tienne, ses mots).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct RubriqueDeLaPage {
    lieux: Vec<Choix>,
    lieu_aide: Option<&'static str>,
    code: droit::catalogue::CodeDeRubrique,
    libelle: String,
    unite: droit::catalogue::Unite,
    nature: droit::catalogue::NatureDeRubrique,
    saisie: Vec<String>,
    sens: String,
    heures_forfait: Option<NombreLu>,
    plafond: Option<String>,
    aem: Option<String>,
    dsn_type: Option<String>,
    dsn_autres_bruts: Option<AutreElementDeRevenuBrut>,
    annexe: Option<String>,
    soumis: bool,
    imposable: bool,
    conges_spectacles: bool,
    compte_pour: String,
    plafond_mensuel: Option<i64>,
    plafond_annuel_heures: Option<droit::catalogue::PlafondAnnuelHeures>,
    origine: String,
    genres: Vec<Choix>,
    /// CE QUE LA RUBRIQUE PROMET SANS QUE RIEN NE LE TIENNE.
    non_appliques: Vec<&'static str>,
    source: String,
    dit: Option<String>,
    /// Les mots entre les cases de sa ligne : « quantité à taux € », ou « base € à taux % » pour un pourcentage.
    mots: [&'static str; 2],
    /// LES CLES QU'UNE LIGNE DE CETTE RUBRIQUE DIT, toutes et seules (`engagement::ligne::FormeSaisie`) : la page les
    /// ecrit, `null` pour une case vide.
    cles: Vec<&'static str>,
}

pub(crate) fn rubrique_de_la_page(r: &Rubrique) -> R<RubriqueDeLaPage> {
    let cles = match engagement::ligne::FormeSaisie::de(r)? {
        Some(f) => f.noms(),
        None => return Err(socle::refus!("La rubrique « {} » ne se saisit pas : le moteur la pose lui-même. Le signaler au développeur.", r.libelle)),
    };
    let non_appliques: Vec<&'static str> = [
        ("annexe", r.annexe.as_deref()),
        ("plafond", r.plafond.as_deref()),
        ("aem", r.aem.as_deref()),
        ("dsn_type", r.dsn_type.as_deref()),
    ]
    .into_iter()
    .filter(|(_, v)| v.is_some_and(|s| !s.is_empty()))
    .map(|(c, _)| c)
    .collect();
    // Les valeurs de `genre`, quand la rubrique le demande : les cinq avantages en nature du bloc 54.
    let avec_genre = r.saisie.iter().any(|s| s == "genre") && r.dsn_autres_bruts.is_some_and(AutreElementDeRevenuBrut::avantage_en_nature);
    let genres: Vec<Choix> = if avec_genre {
        AutreElementDeRevenuBrut::TOUS
            .iter()
            .filter_map(|g| genre_en_clair(*g).map(|libelle| Choix { code: g.code().to_string(), libelle: libelle.to_string(), defaut: Some(Some(*g) == r.dsn_autres_bruts) }))
            .collect()
    } else {
        vec![]
    };
    // LE LIEU D'UNE LIGNE DE NUITEE OU DE REPAS (`engagement::LignePaie::territoire`), sous « Avancé » : la metropole d'abord (rien
    // a choisir), l'outre-mer et Monaco, puis chaque lieu de la table des missions a l'etranger (`droit::missions::lieux`).
    let choix = |code: &str, libelle: &str| Choix { code: code.to_string(), libelle: libelle.to_string(), defaut: None };
    let lieux: Vec<Choix> = if [engagement::NUITEE, engagement::REPAS].contains(&r.code) {
        std::iter::once(choix("", "Métropole"))
            .chain(engagement::TerritoireHorsMetropole::TOUS.into_iter().map(|t| choix(t.texte(), t.dit())))
            .chain(droit::missions::lieux()?.into_iter().map(|(c, l)| choix(c.texte(), &l)))
            .collect()
    } else {
        vec![]
    };
    Ok(RubriqueDeLaPage {
        cles,
        lieu_aide: (!lieux.is_empty()).then_some(LIEU_AIDE),
        lieux,
        code: r.code.clone(),
        libelle: r.libelle.clone(),
        unite: r.unite,
        nature: r.nature,
        saisie: r.saisie.clone(),
        sens: r.sens.clone(),
        heures_forfait: r.heures_forfait.map(NombreLu),
        plafond: r.plafond.clone(),
        aem: r.aem.clone(),
        dsn_type: r.dsn_type.clone(),
        dsn_autres_bruts: r.dsn_autres_bruts,
        annexe: r.annexe.clone(),
        soumis: r.soumis,
        imposable: r.imposable,
        conges_spectacles: r.conges_spectacles,
        compte_pour: r.compte_pour.clone(),
        plafond_mensuel: r.plafond_mensuel,
        plafond_annuel_heures: r.plafond_annuel_heures,
        origine: r.origine.clone(),
        genres,
        non_appliques,
        source: r.source.clone(),
        dit: r.dit.clone(),
        mots: if r.saisie.iter().any(|s| s == "taux_majoration") { ["€ à", "%"] } else { ["à", "€"] },
    })
}

/// `faits_de_rupture` : les faits a demander a l'ecran pour CE motif, et eux
/// seuls (« si compagnie simple, interface simple »).
fn faits_de_rupture(n: &Nature, m: MotifDeRupture) -> R<Vec<&'static str>> {
    use MotifDeRupture::*;
    let sans_notification = matches!(m, FinDeCdd | RuptureConventionnelle | DecesDeLEmployeur | ForceMajeure | CommunAccord);
    let licenciement = matches!(
        m,
        LicenciementEconomique | LicenciementAutreMotif | LicenciementFauteGrave | LicenciementFauteLourde | InaptitudeNonProfessionnelle | InaptitudeProfessionnelle | RuptureAnticipeeFauteGrave | RuptureAnticipeeFauteLourde
    );
    let mut faits = Vec::new();
    // La rupture conventionnelle, ou le moteur refuse sans le fait (CGI 80
    // duodecies 6°).
    let sec = permanents::rupture(Some(m))?;
    if sec.is_some_and(|(fin, _)| fin == permanents::FinDeCdi::RuptureConventionnelle) {
        faits.push("en_droit_a_la_retraite");
    }
    // Un contrat a duree indeterminee dont le motif ouvre un preavis, et dont le preavis non effectue se paie : pas
    // celui que le texte n'indemnise pas (`preavis.indemnite_compensatrice.motifs_sans_indemnite` : 059, 039, 091).
    let sans_indemnite = matches!(permanents::indemnite_de_preavis(Some(m))?, permanents::IndemniteDePreavis::Aucune(_));
    if n.duree_indeterminee && !sans_indemnite {
        if !permanents::sans_preavis(Some(m))? {
            faits.push("preavis_non_effectue");
        }
    }
    // Un CDD dont l'indemnite de fin de contrat est due.
    if !n.duree_indeterminee && permanents::precarite_due(n, Some(m))?.is_none() {
        faits.push("precarite_due");
    }
    // LES DATES ET LA TRANSACTION SELON LE MOTIF : une rupture decidee par l'un
    // des deux se NOTIFIE ; un licenciement s'ENGAGE par la convocation a
    // l'entretien prealable ; une rupture conventionnelle se date par la
    // SIGNATURE de la convention ; la TRANSACTION suit un litige.
    // Et toujours quand le signalement l'exige (CCH-12 de S21.G00.62.003, `fctu::notification_exigee`) : la force majeure
    // (083) est de la liste ci-dessus, et sa date de notification est pourtant obligatoire dans un FCTU.
    if !sans_notification || declarations::fctu::notification_exigee(m)? {
        faits.push("notification_rupture");
    }
    if licenciement {
        faits.push("engagement_licenciement");
    }
    if m == RuptureConventionnelle {
        faits.push("signature_convention");
    }
    if licenciement || matches!(m, MiseALaRetraite | PriseDActe) {
        faits.push("transaction");
    }
    Ok(faits)
}

/// UNE NATURE DE CONTRAT telle que l'ecran la propose : ce que `contrats-base.json` en dit, et ce que la page en tire sans
/// rien decider (les faits a demander et le signalement, motif par motif ; ses mots).
#[derive(Debug, Clone, serde::Serialize)]
struct NatureDeLaPage {
    cle: droit::natures::NatureDeContrat,
    libelle: String,
    aide: String,
    /// Les cases de sa carte, obligatoires ou facultatives, et ce que chaque motif de recours porte en plus, tirees du
    /// type (`engagement::carte::CarteValide::natures`) : ce que la page montre, et seulement cela.
    carte: engagement::carte::NatureDecrite,
    mensuelle: bool,
    duree_indeterminee: bool,
    /// Un permanent ne se rattache a AUCUN spectacle : le numero d'objet (S21.G00.40.050) est INTERDIT hors des natures 02
    /// et 10 (CCH-11).
    spectacle: bool,
    /// La fin se signale-t-elle (`Nature::signalement_fctu`) : au motif par defaut, et motif par motif -- un CDDU a son
    /// terme (031) part dans la mensuelle, rompu avant (036, 037...) il se signale. La page ne decide rien.
    signalement_fctu: bool,
    signalement_par_motif: BTreeMap<MotifDeRupture, bool>,
    motifs_rupture: Vec<natures::RuptureOfferte>,
    faits_de_rupture: BTreeMap<MotifDeRupture, Vec<&'static str>>,
    motif_rupture_defaut: Option<MotifDeRupture>,
    motifs_recours: Vec<natures::RecoursOffert>,
    motif_recours_exige: bool,
    quotite_defaut: Option<NombreLu>,
    /// Le bouton qui propose la duree d'un temps plein, et le choix vide des motifs de fin (le defaut d'une fin qui ne se
    /// signale pas : le CDDU a son terme).
    temps_plein_dit: Option<String>,
    motif_rupture_vide_dit: String,
    unite_quotite: natures::UniteDeQuotite,
}

/// Les natures de contrat, telles que l'ecran les DECRIT : toutes, pour que toute carte se dessine (celle d'un permanent
/// engage avant que la case des permanents se decoche aussi). Tout vient de `contrats-base.json`. Ce qu'une carte neuve
/// PROPOSE se dit au mois (`mois::NouvelleCarte`, `engagement::permanents::Permanents::natures_neuves`).
fn natures() -> R<Vec<NatureDeLaPage>> {
    let toutes = natures::natures()?;
    let cartes = engagement::carte::CarteValide::natures()?;
    // L'ordre est celui du fichier.
    let mut sortie = Vec::new();
    for n in toutes.iter() {
        let motifs = n.motifs_rupture();
        let (mut faits, mut signales) = (BTreeMap::new(), BTreeMap::new());
        for m in motifs {
            faits.insert(m.code, faits_de_rupture(n, m.code)?);
            signales.insert(m.code, n.signalement_fctu(Some(m.code)));
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
            signalement_fctu: n.signalement_fctu(None),
            signalement_par_motif: signales,
            motifs_rupture: motifs.to_vec(),
            faits_de_rupture: faits,
            motif_rupture_defaut: n.motif_rupture_par_defaut(),
            motifs_recours: n.motifs_recours().into_iter().flatten().cloned().collect(),
            motif_recours_exige: n.motif_recours_exige(),
            quotite_defaut: n.quotite_reference_defaut().map(NombreLu),
            temps_plein_dit: n.quotite_reference_defaut().map(|d| format!("temps plein {} h", socle::texte::virgule(d))),
            motif_rupture_vide_dit: match n.motif_rupture_par_defaut() {
                Some(d) if !n.signalement_fctu(None) => format!("— {}, à son terme —", d),
                _ => "— à choisir —".to_string(),
            },
            unite_quotite: n.unite_quotite(),
        });
    }
    Ok(sortie)
}

/// Un niveau de complementaire sante que le bareme connait : son numero, sa cotisation (en % du PMSS) pour l'assure, le
/// conjoint et chaque enfant, et ce que le choix en dit.
#[derive(Debug, Clone, serde::Serialize)]
struct NiveauDeSante {
    niveau: String,
    #[serde(serialize_with = "socle::forme::ecrire_a_virgule")]
    assure: socle::num::Decimal,
    #[serde(serialize_with = "socle::forme::ecrire_a_virgule")]
    conjoint: socle::num::Decimal,
    #[serde(serialize_with = "socle::forme::ecrire_a_virgule")]
    enfant: socle::num::Decimal,
    libelle: String,
}

/// Les niveaux de garantie que le bareme connait, pour la fiche salarie (`releve_svp_sante` de la rubrique `mutuelle` des
/// permanents). Vide quand le bareme n'en porte pas, ou quand il ne se compose pas encore (une fiche entreprise
/// incomplete : la configuration le dit, `ConfigurationDeLaPage::bareme`).
fn niveaux_sante(ctx: &Contexte, compose: bool) -> R<Vec<NiveauDeSante>> {
    if !compose {
        return Ok(Vec::new());
    }
    let pile = ctx.bareme(&ctx.aujourdhui)?;
    // La complementaire sante est celle des permanents : la couche de leur nature.
    let releve = pile.par_nature.get(&droit::natures::Paie::Mensuelle).and_then(|p| p.rubriques_ajoutees.as_ref()?.iter().find(|r| r.id == droit::forme_bareme::MUTUELLE)).and_then(|r| r.releve_svp_sante.as_ref());
    // Une couche sans releve de la complementaire n'en dit aucun niveau.
    Ok(releve
        .into_iter()
        .flat_map(|r| r.niveaux.iter())
        .map(|(n, t)| NiveauDeSante {
            niveau: n.clone(),
            assure: t.assure,
            conjoint: t.conjoint,
            enfant: t.enfant,
            libelle: format!("niveau {} — {} % du PMSS pour l'assuré seul", n, t.assure),
        })
        .collect())
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
    code: String,
    nom: String,
}

/// CE QUE LA PAGE LIT EN S'OUVRANT (`etat`) : la configuration verifiee et sa phrase, l'auteur, ce qu'une compagnie
/// fictive en dit, l'historique des mois, les listes ou l'on choisit et les referentiels livres avec le logiciel.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct EtatDeLaPage {
    phrase: String,
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
    /// Pourquoi l'Historique ne montre que la liste nue des mois, quand le bilan ne se fait pas.
    historique_incomplet: Option<String>,
    salaries: Vec<SalarieDeLaListe>,
    spectacles: Vec<SpectacleDeLaListe>,
    rubriques: Vec<RubriqueDeLaPage>,
    emplois: Vec<emplois::MetierDuDeroulant>,
    familles_emplois: Vec<String>,
    emplois_autres: Vec<super::metiers::MetierAutre>,
    natures: Vec<NatureDeLaPage>,
    types_preavis: Vec<droit::preavis::TypePreavis>,
    aides: Aides,
    transaction: Vec<droit::preavis::Code<droit::preavis::TransactionEnCours>>,
    niveaux_sante: Vec<NiveauDeSante>,
    sante_hors_fiche: Option<String>,
    sante_aide: &'static str,
    conventions: Vec<ConventionServie>,
    organismes_urssaf: Vec<OrganismeUrssaf>,
    urssaf_deduit_dit: String,
    /// L'organisme Urssaf que le code postal de la fiche designe, s'il en designe un.
    urssaf_deduit: Option<String>,
    pays: Vec<droit::nationalite::PaysDeNationalite>,
    pays_naissance: Vec<socle::pays::PaysServi>,
}

/// La route `etat` : tout ce que la page lit en s'ouvrant.
pub(crate) fn etat(ctx: &Contexte) -> R<EtatDeLaPage> {
    let mut m = Moteur::nouveau(ctx);
    let configuration = configuration(ctx)?;
    let niveaux_sante = niveaux_sante(ctx, configuration.bareme.is_none())?;
    let (historique, historique_incomplet) = historique(&mut m, ctx)?;

    // Les listes ou l'on CHOISIT : les fiches courantes seulement. Une archive
    // reste dans `configuration`, ou on peut la relire et la reactiver.
    let mut cles: Vec<&IdSalarie> = ctx.salaries_actifs();
    cles.sort();
    let salaries = cles
        .into_iter()
        .map(|k| SalarieDeLaListe { cle: k.clone(), libelle: ctx.nomme(k) })
        .collect();
    let spectacles = ctx
        .spectacles_actifs()
        .into_iter()
        .map(|k| {
            let v = &ctx.spectacles[k];
            let titre = v.titre.clone().ok_or_else(|| socle::fichiers::absent("titre").reclasser(Classe::Manque))?;
            Ok(SpectacleDeLaListe { cle: k.clone(), titre, numero_objet: v.numero_objet.clone() })
        })
        .collect::<R<Vec<_>>>()?;

    // Le catalogue qui paie, deja dans l'ordre ou le proposer.
    let cat = catalogue::catalogue(&ctx.racine, &ctx.lue_au)?;
    // Une rubrique que le moteur pose (`saisie` vide) ne se propose pas.
    let rubriques: Vec<RubriqueDeLaPage> = cat.iter().filter(|r| !r.saisie.is_empty()).map(rubrique_de_la_page).collect::<R<_>>()?;
    // LA FIN D'UN CONTRAT, telle qu'elle se saisit. Les types de preavis
    // SAISISSABLES seulement.
    let fctu = droit::preavis::base()?;
    let types_preavis: Vec<droit::preavis::TypePreavis> = fctu.types_preavis.codes.into_iter().filter(|c| c.saisissable).collect();
    let transaction = fctu.transaction.codes;

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
        supprimable: crate::accueil::suppression::supprimable(&crate::gestes::administrateur::installation_de(&ctx.racine), &ctx.racine)?,
        suppression: "Tout son dossier est effacé de ce poste : fiches, paies, documents, journaux.",
        // UNE COMPAGNIE CREEE EN DEMO PUIS GARDEE : sa mention, et les mois saisis en demo, ou la page la montre.
        demo: crate::accueil::suppression::mention(&ctx.racine)?,
        // UNE COMPAGNIE VENUE D'UN LOGICIEL PRECEDENT, DONT RIEN N'EST ENCORE REPRIS : la page s'ouvre sur l'import et
        // son guide (`import::guide::commencer_par_l_import`) ; ce qui manque pour demarrer se dit apres lui.
        commencer_par_l_import: import::guide::commencer_par_l_import(&ctx.racine)?,
        historique,
        historique_incomplet,
        salaries,
        spectacles,
        rubriques,
        // Le catalogue des metiers, deja dans l'ordre des familles.
        emplois: super::metiers::metiers(ctx)?,
        familles_emplois: emplois::familles()?,
        // Les metiers des autres conventions : on les AJOUTE a son deroulant,
        // ils n'y sont pas d'office.
        emplois_autres: super::metiers::metiers_autres_conventions(ctx)?,
        // LES NATURES DE CONTRAT : celles que la compagnie engage (une carte dit toujours la sienne).
        natures: natures()?,
        types_preavis,
        aides: Aides(AIDES),
        transaction,
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
    phrase: String,
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
        phrase: format!("Version installée : {}.{}", courante_v, dite),
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
fn dire_la_configuration(c: &ConfigurationDeLaPage) -> String {
    let nom = match c.entreprise.donnees.fiche.nommee() {
        Some(n) => n.to_string(),
        None => "Compagnie sans nom".to_string(),
    };
    let erreurs: String = c.erreurs.iter().map(|p| format!(" ; {}", p.mots)).collect();
    format!(
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
    phrase: String,
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
        phrase: format!("Courriels simulés : {}.", n),
        courriels: joues.courriels,
        dernier: joues.dernier,
        intro: format!("{} courriel{} joué{} — rien n'a quitté le poste.{}", n, s, s, if n > 1 { " Chacun reçoit les siens :" } else { "" }),
        circuits: circuits_fictifs(),
        phrases,
    })
}
