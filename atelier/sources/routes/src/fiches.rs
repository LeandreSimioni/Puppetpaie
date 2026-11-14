//! Configuration › Entreprise, Salariés, Spectacles : les fiches des salaries, des spectacles et de l'entreprise
//! (`verifier-fiche`, `activer-fiche`, `enregistrer-fiche`, leur avertissement, `permanents`), leur demande par courriel
//! (`fiche-demander`, `fiches-relever`, `fiche-oublier-demande`), le taux AT/MP (`taux-atmp`), le regime de la taxe sur
//! les salaires, le cout de l'annee (`couts`), la sauvegarde et la suppression de la compagnie.
//!
//! Les refus portent leur classe (`Manque`, `MalFormee`, `Refuse`...), que la page releve.

use droit::fiche::Lue as _;
use std::collections::HashMap;
use std::path::Path;

use compagnie::preuve::{verifier_preuve, Preuve};
use crate::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, FORME, ILLISIBLE, MANQUE, OPERATION};
use crate::catalogue::Route;
use crate::documents::client_courriel;
use crate::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use compagnie::INTERFACE;
use declarations::champs::{self, FicheAJuger};
use droit::fiche::ChampsEntreprise;
use documents::fiche_questionnaire as fq;
use documents::fiche_questionnaire::CodeDeDemande;
use droit::contexte::{Contexte, IdSalarie, IdSpectacle};
use droit::fiche::{CodeEmploiFt, ChangementEntreprise, ChangementSalarie, ChangementSpectacle, FicheEntrepriseDatee, FicheSalarieDatee, FicheSpectacleDatee, CaseDeFiche, NomDeFiche, Piece};
use envois::courriel::Courriel;
use envois::courriel_lecture as lecture;
use envois::envoi::{self, Message};
use envois::journal;
use envois::ligne_d_envoi::{noter, par_courriel};
use envois::simulation_courriels::BoiteQuiRepond;
use socle::date::Jour;
use socle::echec::{erreur, Classe, Echec, R};
use socle::ecriture::ecrire_json;
use socle::paris::{InstantOuJour, Moment};
use socle::html::{html_echapper, mailto};
use socle::texte::{capitaliser, strip};

mod ecriture;
pub(crate) use ecriture::{avec_libelles, case_vide, comblee, ecrire_fiche, inscrire_etablissements, inscrire_taux_atmp, lire_fichier, poser, preparer, ChangementDit, FicheEnCours, Forcer, SorteDeFiche};

const NOM_FICHE: Champ = Champ { nom: "nom", libelle: Affiche::new("Fiche"), forme: Forme::Texte, sens: "la fiche : entreprise, salarie ou spectacle", avance: false };
const CLE_FICHE: Champ = Champ { nom: "cle", libelle: Affiche::new("Clé"), forme: Forme::Texte, sens: "la clé d'un salarié ou d'un spectacle ; absente pour une fiche neuve ou l'entreprise", avance: false };
const DONNEES: Champ = Champ { nom: "donnees", libelle: Affiche::new("Cases de la fiche"), forme: Forme::Objet, sens: "les cases de la fiche, que la route configuration décrit (description : nom, forme, valeurs) ; une case inconnue se dit", avance: false };
const CHANGEMENTS: Champ = Champ { nom: "changements", libelle: Affiche::new("Cases qui changent"), forme: Forme::Objet, sens: "[{case, valeur}, …], au moins un, dans l'ordre, tout ou rien : chacun une case de la fiche (description de la route configuration : nom, forme, valeurs ; une case inconnue se refuse), null pour la vider (\"\" se refuse) ; une structure se donne entière ; une case non nommée garde sa valeur", avance: false };
pub(crate) const REFUSEE: Refus = Refus { classe: Classe::FicheRefusee, corriger: "les cases que les anomalies nomment ; forcer passe outre une erreur de forme seulement" };
const PREUVE: Refus = Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe du logiciel, tapé par un humain ; rien n'est envoyé" };
const BOITE: Refus = Refus { classe: Classe::BoiteAbsente, corriger: "poser la boîte courriel (Configuration, Mots de passe)" };
const ETAT_REND: &str = "etat : ce que la page lit de la compagnie (la route etat), relu après l'écriture";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Fiches);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Permanents,
        geste: "Dire les cases que montre la fiche en cours de saisie.",
        charge: &[
            NOM_FICHE,
            Champ { nom: "cle", libelle: Affiche::new("Clé"), forme: Forme::Texte, sens: "pour un salarié déjà enregistré, sa clé : les cases d'un permanent qui lui est dû, et celles que le métier d'un de ses engagements demande, se montrent ; absente sinon", avance: false },
            Champ { nom: "donnees", libelle: Affiche::new("Fiche en cours"), forme: Forme::Objet, sens: "la fiche en cours de saisie ; pour un salarié ou un spectacle, l'entreprise est celle de la compagnie", avance: false },
        ],
        exemple: r#"{"nom":"entreprise","donnees":{}}"#,
        rend: "montres (les cases que la fiche montre au jour de l'écran)",
        refus: &[FORME],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::TaxeSurLesSalairesRegime,
        geste: "Dire le régime de la taxe sur les salaires d'après les réponses de la fiche.",
        charge: &[
            Champ { nom: "taxe_sur_les_salaires", libelle: Affiche::new("Réponses"), forme: Forme::Objet, sens: "les réponses en cours de saisie ; sans elles, celles de la compagnie", avance: false },
            Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année des salaires (ses plafonds) ; l'année en cours sinon", avance: true },
        ],
        exemple: r#"{}"#,
        rend: "franchise (la question et ses plafonds), champs à montrer, consequence en clair, complet, due, part_taxee",
        refus: &[FORME, ILLISIBLE],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::VerifierFiche,
        geste: "Vérifier une fiche avant de l'écrire : anomalies, valeurs déclarées.",
        charge: &[NOM_FICHE, DONNEES],
        exemple: r#"{"nom":"spectacle","donnees":{"titre":"Les Bancs publics"}}"#,
        rend: "anomalies (case, gravité, message), declare (ce que la DSN dirait), suggestions",
        refus: &[MANQUE, FORME, OPERATION],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::EnregistrerFiche,
        geste: "Écrire une fiche (entreprise, salarié, spectacle) ; ce qui change se date.",
        charge: &[
            NOM_FICHE,
            CLE_FICHE,
            CHANGEMENTS,
            Champ { nom: "confirme", libelle: Affiche::new("Avertissement lu"), forme: Forme::OuiNon, sens: "vrai : l'avertissement rendu (ce qui change) a été lu ; la fiche s'écrit", avance: false },
            Champ { nom: "depuis", libelle: Affiche::new("Depuis le"), forme: Forme::Date, sens: "la date d'effet de ce qui change, AAAA-MM-JJ ; la date proposée sinon", avance: false },
            Champ { nom: "note", libelle: Affiche::new("Pourquoi"), forme: Forme::Texte, sens: "pourquoi cela change (correction, déménagement…)", avance: true },
            Champ { nom: "forcer", libelle: Affiche::new("Écrire malgré une erreur de forme"), forme: Forme::OuiNon, sens: "vrai : écrire malgré une erreur de forme", avance: true },
        ],
        exemple: r#"{"nom":"spectacle","changements":[{"case":"titre","valeur":"Les Bancs publics"},{"case":"numero_objet","valeur":"266Z00952012"}]}"#,
        rend: "une valeur existante change, ou un choix se confirme : avertissement (changements, depuis proposé, pourquoi ; usage, la phrase à confirmer) et ses phrases, rien n'est écrit ; sinon : cle, ecrits (les champs changés), enregistre_a (vide si rien n'a changé), phrase",
        refus: &[REFUSEE, OPERATION, MANQUE, FORME, ILLISIBLE],
        acte: Acte::Ecrit,
        phrase: 120,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ActiverFiche,
        geste: "Archiver une fiche de salarié ou de spectacle, ou la réactiver.",
        charge: &[NOM_FICHE, CLE_FICHE, Champ { nom: "actif", libelle: Affiche::new("Active"), forme: Forme::OuiNon, sens: "vrai : réactiver ; faux : archiver", avance: false }],
        exemple: r#"{"nom":"spectacle","cle":"le-banc-des-menteurs","actif":true}"#,
        rend: ETAT_REND,
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::FicheDemander,
        geste: "Demander à une personne, par courriel, de remplir sa fiche.",
        charge: &[
            Champ { nom: "adresse", libelle: Affiche::new("Courriel"), forme: Forme::Texte, sens: "son courriel", avance: false },
            Champ { nom: "prenom", libelle: Affiche::new("Prénom"), forme: Forme::Texte, sens: "son prénom, pour lui écrire", avance: false },
            Champ { nom: "motdepasse", libelle: Affiche::new("Mot de passe du logiciel"), forme: Forme::Texte, sens: "le mot de passe du logiciel, s'il est posé ; sans lui, le moteur le demande à un humain", avance: false },
        ],
        exemple: r#"{"adresse":"isadora@exemple.invalid","prenom":"Isadora"}"#,
        rend: "code (celui de la demande), adresse",
        refus: &[PREUVE, BOITE, OPERATION],
        acte: Acte::Engage,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::FichesRelever,
        geste: "Relever dans la boîte les fiches que les personnes ont remplies.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "attendues, recues (les fiches écrites), incidents",
        refus: &[BOITE, OPERATION],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::FicheOublierDemande,
        geste: "Renoncer aux demandes de fiche restées sans réponse d'une personne.",
        charge: &[Champ { nom: "adresse", libelle: Affiche::new("Courriel"), forme: Forme::Texte, sens: "le courriel à qui la fiche a été demandée", avance: false }],
        exemple: r#"{"adresse":"isadora@exemple.invalid"}"#,
        rend: "adresse",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::TauxAtmp,
        geste: "Dire le taux AT/MP d'avant, en vigueur et à venir, et d'où ils viennent.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "phrase et explication du taux de l'année, avant, maintenant et a_venir (code, taux, date, provenance, phrase), le risque tel que l'arrêté l'écrit, la date proposée pour une ligne neuve, les codes que le socle connaît, les mêmes choses pour chaque autre établissement",
        refus: &[FORME, ILLISIBLE],
        acte: Acte::Lit,
        phrase: 60,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Effectif,
        geste: "Dire l'effectif d'une année ; le poser quand l'Urssaf ne l'a pas renvoyé.",
        charge: &[
            Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année civile mesurée, AAAA ; absente : celles qui manquent pour payer cette année", avance: false },
            Champ { nom: "valeur", libelle: Affiche::new("Effectif annuel moyen"), forme: Forme::Nombre, sens: "son effectif annuel moyen, tel que l'Urssaf l'a calculé (au centième)", avance: false },
        ],
        exemple: r#"{"annee":2024,"valeur":6.5}"#,
        rend: "annee, valeur et d'où elle vient (l'Urssaf, la compagnie, la première embauche), a_dire (les années qui manquent pour payer cette année), phrase",
        refus: &[OPERATION, FORME, ILLISIBLE],
        acte: Acte::Ecrit,
        phrase: 300,
        ecran: Ecran::Auto(Place::Fiches, Affiche::new("Dire ou poser l'effectif")),
    },
    Fiche {
        route: Route::Couts,
        geste: "Dire le coût de l'année, spectacle par spectacle.",
        charge: &[Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année, AAAA", avance: false }],
        exemple: r#"{"annee":2026}"#,
        rend: "le coût de l'année par spectacle",
        refus: &[MANQUE, OPERATION],
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Fiches),
    },
    Fiche {
        route: Route::SauvegardeMaintenant,
        geste: "Sauvegarder maintenant le dossier de la compagnie dans le Drive.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "sauvegarde (la dernière : quand, ce qui est parti), erreur (ce qui l'a empêchée) ; ne refuse jamais",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Fiches),
    },
    Fiche {
        route: Route::CompagnieSupprimer,
        geste: "Supprimer la compagnie de ce poste : avertir, puis effacer tout son dossier.",
        charge: &[
            Champ { nom: "confirme", libelle: Affiche::new("Effacer"), forme: Forme::OuiNon, sens: "vrai : effacer ; sans lui, la route dit seulement ce qui sera effacé et ce qu'elle demande", avance: false },
            Champ { nom: "motdepasse", libelle: Affiche::new("Mot de passe du logiciel"), forme: Forme::Texte, sens: "une compagnie réelle : le mot de passe du logiciel, tapé dans la page ; par moteur route, le moteur le demande lui-même à un humain", avance: false },
        ],
        exemple: r#"{"confirme": true}"#,
        rend: "sans confirme : nom, genre (ajoutee ou reelle), avertissement, conservation et sauvegarde à proposer, ou la phrase qui dit pourquoi elle ne se supprime pas ; avec : supprimee, phrase",
        refus: &[Refus { classe: Classe::Refuse, corriger: "ce que la phrase dit (une fictive livrée se remet à zéro, le mot de passe du logiciel refusé, un dossier tenu ouvert) ; rien n'est effacé" }],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Fiches),
    },
];
/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::Permanents => en_json(permanents(ctx, charge.lire()?)?),
        Route::TaxeSurLesSalairesRegime => en_json(taxe_sur_les_salaires_regime(ctx, charge.lire()?)?),
        Route::TauxAtmp => {
            let super::SansCharge {} = charge.lire()?;
            en_json(crate::atmp::route(ctx)?)
        }
        Route::FicheDemander => en_json(fiche_demander(ctx, charge.lire()?, charge.mot_de_passe())?),
        Route::FicheOublierDemande => en_json(oublier_demande(ctx, charge.lire()?)?),
        Route::Effectif => en_json(effectif(ctx, charge.lire()?)?),
        Route::Couts => en_json(couts(ctx, charge.lire()?)?),
        Route::ActiverFiche => en_json(activer_fiche_route(ctx, charge.lire()?)?),
        Route::SauvegardeMaintenant => {
            let super::SansCharge {} = charge.lire()?;
            let faite: SauvegardeFaite = sauvegarde_maintenant(ctx)?;
            en_json(faite)
        }
        Route::FichesRelever => {
            let super::SansCharge {} = charge.lire()?;
            let r = relever_fiches(ctx, None, "interface")?;
            let phrase = match (r.recues.is_empty(), r.incidents.first()) {
                (false, _) => format!("{}.", r.resume()),
                (true, Some(i)) => i.clone(),
                (true, None) => format!("Aucune réponse pour l'instant ; fiches attendues : {}.", r.attendues),
            };
            en_json(FichesRelevees { phrase, attendues: r.attendues, recues: r.recues, incidents: r.incidents })
        }
        Route::VerifierFiche => en_json(self::ecriture::verifier_fiche(ctx, charge.lire()?)?),
        Route::EnregistrerFiche => ecriture::enregistrer_fiche(ctx, charge.lire()?),
        // Supprimer la compagnie : avertir, puis effacer. Par `moteur route`, rien a lacher ; la fenetre sert elle-meme la
        // suppression confirmee, pour lacher d'abord ce qu'elle tient de la compagnie (`interface::Fenetre`).
        Route::CompagnieSupprimer => {
            let r: crate::accueil::suppression::Suppression =
                crate::accueil::suppression::route(&ctx.racine, charge.lire()?, charge.mot_de_passe(), &|_| {}).map_err(|m| m.reclasser(Classe::Refuse))?;
            en_json(r)
        }
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

/// Ce que rend `activer-fiche` : la phrase, et ce que la page relit de la compagnie.
#[derive(Debug, serde::Serialize)]
struct FicheActivee {
    phrase: String,
    etat: crate::ouverture::EtatDeLaPage,
}

/// Ce que rend `sauvegarde-maintenant` : la derniere sauvegarde (`compagnie::sauvegarde`), et ce qui l'a empechee.
#[derive(Debug, serde::Serialize)]
struct SauvegardeFaite {
    phrase: String,
    sauvegarde: Option<compagnie::sauvegarde::Derniere>,
    erreur: Option<String>,
}

/// Ce que rend `fiches-relever` : les fiches attendues, celles recues (`ReleveFiches`), et ce qui a manque.
#[derive(Debug, serde::Serialize)]
struct FichesRelevees {
    phrase: String,
    attendues: usize,
    recues: Vec<FicheRecue>,
    incidents: Vec<String>,
}

/// La charge de `activer-fiche` : la fiche (`salarie` ou `spectacle`) et sa cle, active ou archivee.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "nom", deny_unknown_fields)]
pub(crate) enum ChargeActiver {
    #[serde(rename = "salarie")]
    Salarie { cle: IdSalarie, actif: bool },
    #[serde(rename = "spectacle")]
    Spectacle { cle: IdSpectacle, actif: bool },
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
dites!(FicheActivee, SauvegardeFaite, FichesRelevees, RegimeDeLaTaxe, DemandeDeFiche);

impl Dite for Permanents {
    fn phrase(&self) -> &str {
        self.phrase
    }
}

/// La sauvegarde seule, comme la seconde moitie d'un passage de la veille (`gestes::ouverture`).
fn sauvegarde_maintenant(ctx: &Contexte) -> R<SauvegardeFaite> {
    use crate::gestes::ouverture::{ouverture, Clients, Options};
    let opt = Options { sauvegarde_seule: true, incidents_editeur: false, ..Options::entiere() };
    let r = ouverture(ctx, &opt, Clients::default(), None)?;
    let erreurs: Vec<String> = r.manquants.into_iter().chain(r.incidents).collect();
    let s = compagnie::sauvegarde::derniere(&ctx.racine).map_err(|e| e.echec)?;
    let phrase = match (erreurs.is_empty(), &s) {
        (false, _) => format!("Sauvegarde : {}", erreurs.join(" ")),
        (true, None) => "Aucune sauvegarde faite.".to_string(),
        (true, Some(d)) => format!("Dernière sauvegarde : {}, {} fichiers.", socle::paris::ecrit::saisie::texte(d.quand), d.fichiers),
    };
    Ok(SauvegardeFaite { phrase, sauvegarde: s, erreur: (!erreurs.is_empty()).then(|| erreurs.join(" ")) })
}

// ------------------------------------------------------------------ l'effectif

/// Ou une compagnie trouve l'effectif d'une annee : la moyenne de ses effectifs mensuels (L. 130-1, I), chaque salarie a due
/// proportion de ses jours et de son temps partiel (R. 130-1, II), que l'Urssaf renvoie en compte rendu de nature 118.
const OU_TROUVER_L_EFFECTIF: &str =
    "C'est la moyenne de ses effectifs mensuels, chaque salarié compté au prorata de ses jours sous contrat et de son temps partiel (R. 130-1 CSS) ; l'Urssaf la renvoie en compte rendu « Effectifs » (nature 118) : `annee` et `valeur`.";

/// L'EFFECTIF D'UNE ANNEE (`config/effectifs.json`, A01) : le dire, et le poser quand la compagnie le donne pour une annee
/// que l'Urssaf n'a pas renvoyee en compte rendu de nature 118 (`envois::releve_retours::poser_effectif`). L'effectif
/// annuel est la moyenne des mois de l'annee civile, tronquee au centieme (CSS, L. 130-1, I, et R. 130-1, I :
/// `atelier/reference/sources/code-de-la-securite-sociale/LISEZ-MOI.md`) ; il ne se dit qu'une annee finie. Sans
/// `annee` : les annees qui manquent pour payer cette annee (`Effectif::a_dire`).
fn effectif(ctx: &Contexte, charge: ChargeEffectif) -> R<EffectifDit> {
    let an_paie = ctx.aujourdhui.an;
    // Les annees qui manquent : celles des seuils (`Effectif::a_dire`), et celle d'avant quand la paie la lit elle-meme et
    // ne la connait pas (le FNAS de la CCNEAC sans CSE dit) : ce que « Pour démarrer » demande (`demarrage::manques`).
    let a_dire = |ctx: &Contexte| -> R<Vec<i32>> {
        let e = ctx.effectif()?;
        let mut v = e.a_dire(an_paie);
        let demande = declarations::demarrage::effectif_a_dire(ctx)?;
        if demande && e.valeur(an_paie).is_none() && !v.contains(&(an_paie - 1)) {
            v.push(an_paie - 1);
            v.sort();
        }
        Ok(v)
    };
    let Some(an) = charge.annee else {
        let manquent = a_dire(ctx)?;
        let phrase = if manquent.is_empty() {
            format!("L'effectif de la paie de {} est connu.", an_paie)
        } else {
            format!(
                "Pour payer en {}, il manque l'effectif de {}. {}",
                an_paie,
                manquent.iter().map(i32::to_string).collect::<Vec<_>>().join(", "),
                OU_TROUVER_L_EFFECTIF
            )
        };
        return Ok(EffectifDit { phrase, annee: None, valeur: None, source: None, a_dire: manquent });
    };
    let en_clair = |x: socle::num::Decimal| socle::texte::virgule(x.round_dp(2));
    let mut pose = None;
    if let Some(v) = &charge.valeur {
        if an >= an_paie {
            return Err(erreur(Classe::Refuse, format!("L'effectif de {} se calcule sur l'année entière : il se dit une fois l'année finie. Rien n'est écrit.", an)));
        }
        let x = v.court().map_err(|m| m.reclasser(Classe::Illisible))?;
        if x < socle::num::Decimal::ZERO {
            return Err(erreur(Classe::Illisible, format!("Effectif illisible : {}.", v.ecrit())));
        }
        // Tronque au centieme (R. 130-1, I).
        let x = x.round_dp_with_strategy(2, rust_decimal::RoundingStrategy::ToZero);
        if !envois::releve_retours::poser_effectif(&ctx.racine, an, x, ctx.aujourdhui)? {
            return Err(erreur(Classe::Refuse, format!("L'Urssaf a renvoyé l'effectif de {} : c'est le sien qui vaut. Rien n'est écrit.", an)));
        }
        pose = Some(format!("Effectif de {} : {}.", an, en_clair(x)));
    }
    let e = ctx.effectif()?;
    let dit = envois::releve_retours::effectif_dit_par_la_compagnie(&ctx.racine, an)?;
    let (valeur, source) = match e.annees.get(&an) {
        Some(v) => (Some(*v), Some(if dit { "la compagnie" } else { "l'Urssaf" })),
        None => match e.annee(an) {
            Some(v) => (Some(v), Some("la première embauche")),
            None => (None, None),
        },
    };
    let phrase = match (pose, valeur, source) {
        (Some(p), _, _) => p,
        (None, Some(v), Some(s)) => format!("Effectif de {} : {}, d'après {}.", an, en_clair(v), s),
        (None, _, _) => format!("L'effectif de {} n'est pas connu. {}", an, OU_TROUVER_L_EFFECTIF),
    };
    Ok(EffectifDit { phrase, annee: Some(an), valeur, source, a_dire: a_dire(ctx)? })
}

/// La charge de `effectif` : l'annee mesuree (absente : celles qui manquent), et l'effectif que la compagnie en dit.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeEffectif {
    annee: Option<i32>,
    valeur: Option<droit::fiche::Montant>,
}

/// Ce que rend `effectif` : l'annee, son effectif et d'ou il vient (l'Urssaf, la compagnie, la premiere embauche), les
/// annees qui manquent pour payer cette annee.
#[derive(Debug, serde::Serialize)]
struct EffectifDit {
    phrase: String,
    annee: Option<i32>,
    #[serde(serialize_with = "socle::forme::ecrire_a_virgule_opt")]
    valeur: Option<socle::num::Decimal>,
    source: Option<&'static str>,
    a_dire: Vec<i32>,
}

// ------------------------------------------------------------------ permanents

/// Ce que rend `permanents` : la phrase, les cases que la fiche montre et les pieces que ses engagements ouvrent.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Permanents {
    phrase: &'static str,
    montres: Vec<CaseDeFiche>,
    pieces_ouvertes: Vec<Piece>,
}

/// `permanents` : les cases que la fiche en cours de saisie montre au jour de l'ecran (la configuration se montre au
/// jour : `ouverture::configuration`) ; la page montre ou replie chaque case selon cette reponse, et ne decide rien
/// quand on coche ou decoche la case des permanents.
fn permanents(ctx: &Contexte, charge: FicheEnCours) -> R<Permanents> {
    let jour = ctx.aujourdhui;
    let p = engagement::permanents::Permanents::de(ctx, &jour, &jour)?;
    // Une case que son type refuse est vide dans la fiche typee : elle ne change pas ce qui se montre.
    let (Montres { cases, pieces_ouvertes }, entreprise) = match &charge {
        FicheEnCours::Entreprise { donnees } => (montres(ctx, FicheAJuger::Entreprise(&donnees.fiche), None, &donnees.fiche, &p)?, &donnees.fiche),
        FicheEnCours::Salarie { cle, donnees } => (montres(ctx, FicheAJuger::Salarie(&donnees.fiche), cle.as_ref(), &ctx.entreprise, &p)?, &ctx.entreprise),
        FicheEnCours::Spectacle { donnees } => (montres(ctx, FicheAJuger::Spectacle(&donnees.fiche), None, &ctx.entreprise, &p)?, &ctx.entreprise),
    };
    Ok(Permanents {
        phrase: if permanents_de(entreprise) { "La compagnie emploie des permanents." } else { "La compagnie n'emploie pas de permanents." },
        montres: cases,
        pieces_ouvertes,
    })
}

/// CE QUE LA FICHE MONTRE (`montres`) : ses cases, et les pieces que ses engagements ouvrent, entieres (celle d'un
/// permanent, celle d'une compagnie qui en emploie) ou pour une case au moins (celle du metier). La page n'ouvre le tiroir
/// d'une piece que sur elles.
pub(crate) struct Montres {
    pub(crate) cases: Vec<CaseDeFiche>,
    pub(crate) pieces_ouvertes: Vec<Piece>,
}

/// La case des permanents de la fiche entreprise telle que l'ecran la saisit.
fn permanents_de(entreprise: &ChampsEntreprise) -> bool {
    entreprise.emploie_des_permanents.oui()
}

/// CE QUE LA FICHE `nom` (de cle `cle` pour un salarie enregistre) MONTRE pour `donnees` : chaque case que
/// `champs::Champ::montre` dit a l'ecran, d'apres la fiche, l'entreprise `entreprise` (ses conditions), les
/// etablissements de la compagnie, et la piece de la case (`droit::fiche::Piece`) : elle ne se montre que si la fiche
/// peut avoir un calcul qui l'ouvre. La piece d'un permanent s'ouvre par la nature d'un engagement paye au mois : sur la
/// fiche d'un salarie, un des siens (`engagement::permanents::cle_du_salarie`) ; sur celle de l'entreprise, quand elle
/// dit employer des permanents (telle qu'elle est saisie : un CDI ou un CDD neuf se propose) ou qu'un engagement paye au
/// mois est du (`engagement::permanents::Permanents`). Celle d'une compagnie qui emploie des permanents, par sa case
/// telle qu'elle est saisie. Celles du metier, par la grille du metier de ses engagements (la jauge de la salle, par un
/// engagement paye au mois : `cases_des_metiers`). Celles que l'effectif ouvre, par lui (`pieces_de_l_effectif`). La
/// page ne montre que ces cases-la.
pub(crate) fn montres(ctx: &Contexte, donnees: FicheAJuger<'_>, cle: Option<&IdSalarie>, entreprise: &ChampsEntreprise, p: &engagement::permanents::Permanents) -> R<Montres> {
    let nom = donnees.nom();
    let au_mois = match nom {
        NomDeFiche::Entreprise => permanents_de(entreprise) || !p.dus.is_empty(),
        // Un salarie pas encore enregistre n'a aucun engagement.
        NomDeFiche::Salarie => cle.map(|c| engagement::permanents::cle_du_salarie(ctx, c)).transpose()?.flatten().is_some(),
        // Un spectacle n'a pas la piece d'un permanent.
        NomDeFiche::Spectacle => false,
    };
    let avec_permanents = nom == NomDeFiche::Entreprise && permanents_de(entreprise);
    let code_emploi = match donnees {
        FicheAJuger::Salarie(s) => s.code_emploi_ft.clone(),
        FicheAJuger::Entreprise(_) | FicheAJuger::Spectacle(_) => None,
    };
    let du_metier = cases_des_metiers(ctx, nom, cle, code_emploi)?;
    let (cse, casc) = if nom == NomDeFiche::Entreprise { pieces_de_l_effectif(ctx, entreprise)? } else { (false, false) };
    let ouverte = |piece: Piece, case: CaseDeFiche| match piece {
        Piece::Permanent => au_mois,
        Piece::EmploieDesPermanents => avec_permanents,
        Piece::Metier | Piece::MetierAuMois => du_metier.contains(&case),
        Piece::Cse => cse,
        Piece::Casc => casc,
    };
    let etablissements = super::ouverture::choix_d_etablissement(ctx)?.is_some();
    let autre = (nom != NomDeFiche::Entreprise).then_some(entreprise);
    let cases = champs::fiche(nom)?.iter().filter(|c| donnees.montre(c, autre, &ouverte, etablissements)).map(|c| c.cle).collect();
    // Les cases du metier d'un salarie ; la jauge de la salle, sur la fiche de l'entreprise.
    let metier = if nom == NomDeFiche::Entreprise { Piece::MetierAuMois } else { Piece::Metier };
    let pieces_ouvertes = [(Piece::Permanent, au_mois), (Piece::EmploieDesPermanents, avec_permanents), (metier, !du_metier.is_empty()), (Piece::Cse, cse), (Piece::Casc, casc)]
        .into_iter()
        .filter_map(|(piece, ouverte)| ouverte.then_some(piece))
        .collect();
    Ok(Montres { cases, pieces_ouvertes })
}

/// LES PIECES DE L'ENTREPRISE QUE SON EFFECTIF OUVRE, au jour de l'ecran (`droit::contexte::Effectif`) : la
/// representation du personnel quand un comite peut exister (`Effectif::cse`) et qu'un calcul la lit (une contribution
/// de la convention qui en depend, ou l'exemption de la CASC-SVP, que cinquante salaries rendent possible) ; l'exemption
/// de la CASC-SVP quand la compagnie compte cinquante salaries et que la fiche `entreprise`, telle qu'elle est saisie, dit
/// un CSE (`droit::fiche::ChampsEntreprise::exemption_casc`), sur la fiche typee. Un effectif inconnu n'ouvre rien : c'est
/// lui que « Pour démarrer » demande (`Effectif::a_dire`).
fn pieces_de_l_effectif(ctx: &Contexte, entreprise: &ChampsEntreprise) -> R<(bool, bool)> {
    let (jour, eff) = (ctx.aujourdhui, ctx.effectif()?);
    let (cse, cinquante) = (eff.cse(jour.an), eff.cinquante(jour.an));
    let Some(cse) = cse else { return Ok((false, false)) };
    let lue = cinquante.is_some() || ctx.contribution_par_representation(&jour)?;
    let Some(cinquante) = cinquante else { return Ok((lue, false)) };
    Ok((lue, entreprise.exemption_casc(&cinquante, &cse).is_some()))
}

/// LES CASES DU METIER (`droit::fiche::Piece::Metier`) QUE LA FICHE `nom` MONTRE : celles que la grille du metier de
/// chaque engagement actif lit. Sur la fiche du salarie `cle`, les cases que son metier demande
/// (`engagement::minima::cases_du_metier`), au premier jour de chacun de ses engagements : le metier de la carte ou
/// d'un avenant, sinon l'emploi de la fiche en cours de saisie (`donnees`), comme la paie le lit. Sur la fiche de
/// l'entreprise, la jauge de la salle quand un engagement paye au mois (`paye_au_mois`) est celui d'un technicien de
/// l'annexe I, que sa prime
/// d'anciennete peut viser (`droit::clauses::prime_anciennete_possible`, le metier de la carte ou de la fiche de son
/// salarie). Un engagement qui n'est plus actif ne demande rien ; une fiche sans engagement, rien.
fn cases_des_metiers(ctx: &Contexte, nom: NomDeFiche, cle: Option<&IdSalarie>, de_la_fiche: Option<CodeEmploiFt>) -> R<std::collections::BTreeSet<CaseDeFiche>> {
    let mut cases = std::collections::BTreeSet::new();
    // Le code emploi de la fiche en cours de saisie, type a sa porte ; hors de sa forme, sa faute le dit.
    // (metier, statut dit par la carte, jour, salarie)
    let mut metiers: std::collections::BTreeSet<(CodeEmploiFt, Option<droit::fiche::Statut>, Jour, IdSalarie)> = std::collections::BTreeSet::new();
    for f in engagement::stock::fichiers(&ctx.racine)? {
        let Some(lu) = engagement::stock::lire_fichier(&ctx.racine, f)? else { continue };
        for e in lu.engagements.iter().filter(|e| e.non_actif.is_none()) {
            let s = &e.saisie;
            let Some(salarie) = s.carte.salarie.as_ref() else { continue };
            let du_salarie = match (nom, cle) {
                (NomDeFiche::Salarie, Some(c)) if c == salarie => de_la_fiche.clone(),
                (NomDeFiche::Entreprise, _) if paye_au_mois(s)? => ctx.fiche_de(salarie)?.code_emploi_ft.dite().cloned(),
                _ => continue,
            };
            // La carte au premier jour de son mois, chaque avenant au jour ou il vaut.
            let termes = std::iter::once((&s.carte, lu.mois.premier_jour())).chain(s.avenants.iter().map(|a| (&a.termes, a.depuis)));
            for (t, jour) in termes {
                if let Some(code) = t.code_emploi_ft.clone().or_else(|| t.metier.as_ref().map(CodeEmploiFt::from)).or_else(|| du_salarie.clone()) {
                    metiers.insert((code, t.statut, jour.clone(), salarie.clone()));
                }
            }
        }
    }
    for (code, statut, jour, salarie) in &metiers {
        if nom == NomDeFiche::Salarie {
            cases.extend(engagement::minima::cases_du_metier(ctx, code, jour)?.into_iter().map(CaseDeFiche::from));
            continue;
        }
        let ent = &ctx.entreprise;
        let statut = match statut {
            Some(s) => Some(*s),
            None => ctx.fiche_de(salarie)?.statut.dite().copied(),
        };
        if droit::clauses::prime_anciennete_possible(ent.idcc.dite(), ent.annexe_convention.dite().copied(), Some(code), statut)? {
            cases.insert(droit::fiche::CasePieceEntreprise::JaugeAnnexe1.into());
        }
    }
    Ok(cases)
}

/// L'ENGAGEMENT `s` EST-IL PAYE AU MOIS, par sa carte ou un avenant (`droit::natures::Nature::au_mois`) : seul son calcul
/// lit la jauge de la salle de l'annexe I (`droit::fiche::Piece::MetierAuMois`).
fn paye_au_mois(s: &engagement::stock::Saisie) -> R<bool> {
    for t in std::iter::once(&s.carte).chain(s.avenants.iter().map(|a| &a.termes)) {
        if let Some(n) = t.nature {
            if n.proprietes()?.au_mois().is_some() {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

// ------------------------------------------------------- taxe-sur-les-salaires-regime

/// La charge de `taxe-sur-les-salaires-regime` : les reponses en cours de saisie (absentes ou nulles : celles de la
/// compagnie ; `{}` : aucune), et l'annee des salaires (absente : l'annee en cours).
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeTaxeSurLesSalaires {
    taxe_sur_les_salaires: Option<droit::fiche::TaxeSurLesSalaires>,
    annee: Option<i64>,
}

/// Les plafonds de la franchise en base de TVA, au bareme de l'annee.
#[derive(Debug, Clone, serde::Serialize)]
struct PlafondsDeLaFranchise {
    #[serde(serialize_with = "socle::ecriture::decimal_en_texte::serialize")]
    prestations_de_services: socle::num::Decimal,
    #[serde(serialize_with = "socle::ecriture::decimal_en_texte::serialize")]
    total: socle::num::Decimal,
}

/// Les deux reponses a la question de la franchise : sous les plafonds (`true`), au-dessus (`false`).
#[derive(Debug, Clone, serde::Serialize)]
struct ChoixDeLaFranchise {
    #[serde(rename = "true")]
    sous: String,
    #[serde(rename = "false")]
    dessus: String,
}

/// La question de la franchise : son aide, ses reponses, ses plafonds et l'annee.
#[derive(Debug, Clone, serde::Serialize)]
struct Franchise {
    aide: String,
    choix: ChoixDeLaFranchise,
    plafonds: Option<PlafondsDeLaFranchise>,
    annee: i32,
}

/// Ce que rend `taxe-sur-les-salaires-regime` : la phrase (la consequence), la question de la franchise, les cases a
/// montrer, la consequence, si la reponse est complete, si la taxe est due et sur quelle part.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct RegimeDeLaTaxe {
    phrase: String,
    franchise: Franchise,
    champs: Vec<&'static str>,
    consequence: String,
    complet: bool,
    due: bool,
    #[serde(serialize_with = "socle::ecriture::decimal_en_texte::option::serialize")]
    part_taxee: Option<socle::num::Decimal>,
}

/// `taxe-sur-les-salaires-regime` : ce que la fiche en cours de saisie dit -- sans lui, celle de la compagnie. Rend la
/// conclusion du moteur (`taxe_sur_les_salaires::regime_taxe_sur_les_salaires`, CGI art. 231, 1) : les cases a montrer,
/// la consequence en clair, et si la reponse est complete. La page n'en decide rien.
fn taxe_sur_les_salaires_regime(ctx: &Contexte, charge: ChargeTaxeSurLesSalaires) -> R<RegimeDeLaTaxe> {
    let ent = match charge.taxe_sur_les_salaires {
        Some(t) => droit::fiche::ChampsEntreprise { taxe_sur_les_salaires: Some(t), ..Default::default() },
        None => ctx.entreprise.clone(),
    };
    // LES PLAFONDS DE LA FRANCHISE, CHIFFRES : ceux du bareme de l'annee des salaires -- `annee` dans la
    // charge, sinon l'annee en cours. Une annee sans socle livre ne fait pas refuser : la question se pose sans montants.
    let annee = match charge.annee {
        None => ctx.aujourdhui.an,
        Some(a) if (1900..=2999).contains(&a) => a as i32,
        Some(a) => return Err(erreur(Classe::Illisible, socle::date::annee_impossible(a))),
    };
    let bar = match (socle::date::Jour::nouveau(annee, 1, 1), droit::contexte::socle_livre_lu(annee)?) {
        (Some(j), Some(_)) => Some(ctx.bareme(&j)?),
        _ => None,
    };
    let q = documents::taxe_sur_les_salaires::question_franchise(bar.as_deref(), annee).map_err(|m| m.reclasser(Classe::Illisible))?;
    // La conclusion nomme les deux plafonds.
    use documents::taxe_sur_les_salaires::Decision;
    let r = documents::taxe_sur_les_salaires::regime_taxe_sur_les_salaires(&ent, q.plafonds);
    Ok(RegimeDeLaTaxe {
        phrase: r.consequence.clone(),
        franchise: Franchise {
            aide: q.aide,
            choix: ChoixDeLaFranchise { sous: q.sous, dessus: q.dessus },
            plafonds: q.plafonds.map(|(s, t)| PlafondsDeLaFranchise { prestations_de_services: s.normalize(), total: t.normalize() }),
            annee,
        },
        champs: r.champs,
        consequence: r.consequence,
        complet: !matches!(r.decision, Decision::ADire(_)),
        due: matches!(r.decision, Decision::DueSur(_)),
        part_taxee: match r.decision {
            Decision::DueSur(p) => Some(p.normalize()),
            _ => None,
        },
    })
}

// ------------------------------------------------------------------ outils


// ------------------------------------------------------ l'avertissement d'une fiche

/// Ou l'import copie les pieces qu'il reprend de l'ancien logiciel, les DSN deposees comprises (`historique/bulletins/`).
pub fn pieces_importees(ctx: &Contexte) -> std::path::PathBuf {
    ctx.racine.join("historique").join("bulletins")
}

/// `usages_declares` : combien de DSN deja deposees declarent cette valeur dans la rubrique de la case (`rubrique`, de la
/// table des champs), telle qu'une DSN l'ecrit, et sur quels mois (`AAAA-MM`). Une case sans rubrique n'est dans aucune
/// DSN.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct DejaDeclare {
    pub(crate) declarations: usize,
    pub(crate) du: Option<socle::mois::Mois>,
    pub(crate) au: Option<socle::mois::Mois>,
}

fn usages_declares(ctx: &Contexte, rubrique: Option<droit::natures::RubriqueDeLaNorme>, valeur: Option<&str>) -> R<DejaDeclare> {
    let (mut n, mut mois) = (0usize, Vec::<socle::mois::Mois>::new());
    if let (Some(rubrique), Some(valeur)) = (rubrique, valeur) {
        for dossier in [pieces_importees(ctx), ctx.racine.join("sorties")] {
            for f in declarations::dsn_lue::fichiers(&dossier)? {
                let Some(d) = declarations::dsn_lue::lire_une_dsn(&f)? else { continue };
                if d.declare(rubrique, &valeur) {
                    // Un signalement declare la valeur sans mois principal (S20.G00.05.005).
                    n += 1;
                    mois.extend(d.mois);
                }
            }
        }
    }
    Ok(DejaDeclare { declarations: n, du: mois.iter().min().cloned(), au: mois.iter().max().cloned() })
}


#[track_caller]
pub(crate) fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

#[track_caller]
pub(super) fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}


pub(crate) fn creer_dossier(d: &Path) -> R<()> {
    std::fs::create_dir_all(d).map_err(|e| socle::echec::du_disque(&d, &e).reclasser(Classe::Disque))
}

/// Une forme ecrite dans le fichier `chemin` (`socle::ecriture::ecrire_json`).
pub(crate) fn ecrire<T: serde::Serialize + ?Sized>(chemin: &Path, v: &T) -> R<()> {
    ecrire_json(chemin, v, 2).map_err(|e| e.reclasser(Classe::Disque))
}

/// La configuration, relue apres l'ecriture (`web._configuration(Contexte(racine))`).
/// Ce que la page lit de la compagnie (la route `etat`), relu apres une ecriture : la page n'a rien d'autre a relire.
pub(crate) fn etat_relu(ctx: &Contexte) -> R<crate::ouverture::EtatDeLaPage> {
    let neuf = Contexte::nouveau(&ctx.racine, None)?;
    crate::ouverture::etat(&neuf)
}

/// Aujourd'hui, a Paris (`date.today()`).
pub(crate) fn aujourd_hui() -> Jour {
    socle::temps::secondes().a_la_seconde().jour_de_paris()
}

// --------------------------------------------------------------- activer-fiche

/// Seul `actif` change, a partir d'aujourd'hui (archiver, c'est ne plus proposer a partir de ce jour ; ce qui precede
/// reste ce qu'il etait), par l'ecrivain de la fiche (`poser`).
pub(crate) fn activer_fiche(ctx: &Contexte, c: &ChargeActiver) -> R<()> {
    let jour = Some(ctx.aujourdhui);
    match c {
        ChargeActiver::Salarie { cle, actif } => poser::<FicheSalarieDatee>(ctx, Some(cle), vec![ChangementSalarie::Actif(Some(*actif))], jour, None),
        ChargeActiver::Spectacle { cle, actif } => poser::<FicheSpectacleDatee>(ctx, Some(cle), vec![ChangementSpectacle::Actif(Some(*actif))], jour, None),
    }
}

fn activer_fiche_route(ctx: &Contexte, c: ChargeActiver) -> R<FicheActivee> {
    activer_fiche(ctx, &c)?;
    // Le nom que la liste montre : le salarie par son nom, le spectacle par son titre ; la cle seule si la fiche n'en a pas.
    let phrase = match &c {
        ChargeActiver::Salarie { cle, actif } => {
            let qui = match ctx.fiche_de(cle).map_err(|m| m.reclasser(Classe::Manque))?.nomme() {
                Some(n) => n,
                None => cle.to_string(),
            };
            if *actif { format!("{} : fiche réactivée.", qui) } else { format!("{} : fiche archivée.", qui) }
        }
        ChargeActiver::Spectacle { cle, actif } => {
            let fiche = ctx.spectacles.get(cle).ok_or_else(|| refus(format!("Spectacle inconnu : {}.", socle::fichiers::repr(cle.as_str()))))?;
            let qui = match fiche.titre.portee().filter(|t| !t.trim().is_empty()) {
                Some(t) => t.to_string(),
                None => cle.to_string(),
            };
            if *actif { format!("Spectacle « {} » réactivé.", qui) } else { format!("Spectacle « {} » archivé.", qui) }
        }
    };
    Ok(FicheActivee { phrase, etat: etat_relu(ctx)? })
}

// ----------------------------------------------------- fiche-oublier-demande

/// `fiche-oublier-demande` : `{"adresse"}`, la personne a qui la fiche a ete demandee ; ses demandes en attente sont
/// abandonnees.
fn oublier_demande(ctx: &Contexte, c: ChargeOublier) -> R<DemandeOubliee> {
    let adresse = strip(&c.adresse).to_lowercase();
    let codes: Vec<CodeDeDemande> = fiches_attendues(ctx)?.into_iter().filter(|a| *a.adresse == *adresse).map(|a| a.code).collect();
    if codes.is_empty() {
        return Err(refus(format!("Aucune demande de fiche en attente pour {}.", adresse)));
    }
    // CE QUI REVIENT SE RANGE SUR LA LIGNE DE LA DEMANDE : ici, qu'on y renonce.
    for code in &codes {
        let retour = journal::RetourFiche {
            code: code.clone(),
            abandon: true,
            salarie: None,
            illisible: None,
            erreurs: None,
            quand: socle::temps::secondes().a_la_seconde(),
            qui: "interface".into(),
            de: None,
            message: None,
            reponse: "demande abandonnee, sans reponse".into(),
        };
        journal::rattacher(&ctx.racine, |l| demande_de(l, code), journal::Revenu::Fiche(retour))?;
    }
    // Ni NIR ni IBAN dans la reponse : la page relit `configuration` (comme apres `fiche-demander`).
    Ok(DemandeOubliee { phrase: format!("Demande à {} abandonnée.", adresse), adresse })
}

/// La charge de `fiche-oublier-demande` : le courriel a qui la fiche a ete demandee.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeOublier {
    adresse: String,
}

/// Ce que rend `fiche-oublier-demande` : l'adresse, en minuscules.
#[derive(Debug, serde::Serialize)]
struct DemandeOubliee {
    phrase: String,
    adresse: String,
}

use crate::catalogue::{Ecran, Place};


/// LE DERNIER MOIS PARTI pour la fiche `nom` (`cle` pour un salarie) : le dernier dont les documents ou la DSN sont
/// partis (`engagement::departs`), ou que l'ancien logiciel a verse et declare (un engagement importe), pour ce salarie,
/// ou pour quiconque quand c'est la fiche de l'entreprise. `None` : aucun ; une fiche de spectacle n'en a pas.
pub(crate) fn dernier_mois_parti(ctx: &Contexte, nom: NomDeFiche, cle: Option<&IdSalarie>) -> R<Option<socle::mois::Mois>> {
    use engagement::departs::Envoi;
    if nom == NomDeFiche::Spectacle {
        return Ok(None);
    }
    let sien = |c: &IdSalarie| nom == NomDeFiche::Entreprise || Some(c) == cle;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let partis = departs
        .engagements()
        .filter(|(c, _)| sien(c))
        .flat_map(|(c, n)| departs.de(c, n))
        .filter(|d| matches!(d.envoi, Envoi::Documents | Envoi::Dsn))
        .filter_map(|d| d.mois.clone());
    let mut importes = Vec::new();
    for i in engagement::importe::tous(&ctx.racine)? {
        if i.objet().brut.saisie.carte.salarie.as_ref().is_some_and(sien) {
            importes.extend(i.paies().keys().cloned());
        }
    }
    Ok(partis.chain(importes).max())
}

/// LA DATE QU'UN CHANGEMENT DE FICHE PROPOSE, et pourquoi : le premier jour du mois qui suit le dernier mois parti
/// (`dernier_mois_parti`) : ce qui est parti garde ce qu'il a dit, le reste prend la valeur nouvelle. Aucun mois parti :
/// le premier jour du mois en cours. Une autre date se choisit, dans le passe aussi : ce qui est parti se corrige alors.
pub(crate) fn date_proposee(ctx: &Contexte, nom: NomDeFiche, cle: Option<&IdSalarie>) -> R<(Jour, String)> {
    let qui = if nom == NomDeFiche::Entreprise { "" } else { " pour ce salarié" };
    Ok(match dernier_mois_parti(ctx, nom, cle)? {
        Some(m) => {
            let phrase = format!(
                "Proposé : le premier mois dont rien n'est parti{}. Jusqu'à {} compris, ce qui est parti garde l'ancienne valeur.",
                qui,
                m.libelle(socle::mois::Graphie::Accentuee)
            );
            (m.suivant().premier_jour(), phrase)
        }
        None => {
            let mois = ctx.aujourdhui.mois_civil();
            let phrase = format!("Proposé : le premier jour de {}, rien n'étant encore parti{}.", mois.libelle(socle::mois::Graphie::Accentuee), qui);
            (mois.premier_jour(), phrase)
        }
    })
}

/// UN TAUX DE PAS RECU S'ECRIT DANS LA FICHE DE SON SALARIE (`taux_pas`), date du jour ou il commence de valoir
/// `depuis` : avant ce jour, la fiche ne le disait pas (la case vide). `note` dit d'ou il vient.
pub(crate) fn poser_taux_recu(ctx: &Contexte, salarie: &IdSalarie, valeur: droit::fiche::TauxPas, depuis: Jour, note: &str) -> R<()> {
    ecriture::poser_au::<FicheSalarieDatee>(ctx, Some(salarie), vec![ChangementSalarie::TauxPas(Some(valeur))], true, Some(depuis), Some(note), &depuis)
}

/// Une case de la fiche entreprise (`poser`).
pub(crate) fn poser_entreprise(ctx: &Contexte, case: ChangementEntreprise, depuis: Option<Jour>) -> R<()> {
    poser::<FicheEntrepriseDatee>(ctx, None, vec![case], depuis, None)
}

/// L'ANCIENNETE D'UN SALARIE PART DE SON EMBAUCHE (`engagement::calcul::Anciennete`, le chemin du calcul) : la validation
/// d'un engagement qui est son embauche (aucun engagement anterieur ne le nomme) la pose sur sa fiche, au premier jour de
/// ce contrat (`debut`), par l'ecrivain de la fiche (`poser`) ; elle se change ensuite dans « Avancé » de la fiche. Une
/// fiche qui la dit la garde ; une anciennete ni dite ni lue ne se pose pas : ce qui la lit refuse en la nommant.
pub(crate) fn poser_anciennete_a_l_embauche(ctx: &Contexte, salarie: &IdSalarie, debut: Option<Jour>) -> R<()> {
    let Some(fiche) = ctx.salaries.get(salarie) else { return Ok(()) };
    let debut = debut.ok_or_else(|| socle::fichiers::absent("debut"))?;
    match engagement::calcul::Anciennete::de(&ctx.racine, fiche, salarie, debut)? {
        engagement::calcul::Anciennete::Embauche(j) => poser::<FicheSalarieDatee>(ctx, Some(salarie), vec![ChangementSalarie::DateAnciennete(Some(j))], None, None),
        engagement::calcul::Anciennete::Dite(_) | engagement::calcul::Anciennete::NonDite => Ok(()),
    }
}

/// L'ANCIENNETE D'UN SALARIE REPRIS QUE SON CONTRAT FAIT REMONTER : l'import la pose au premier bulletin lu
/// (`import::bulletins`) ; un contrat repris qui a commence avant (`debut`, celui que la DSN de l'ancien logiciel
/// declare, S21.G00.40.001) dit qu'il etait dans l'entreprise des ce jour-la (S21.G00.86.003, l'anciennete dans
/// l'entreprise : `norme-dsn/texte/bloc-S21.G00.86.txt` ; inference : un contrat en cours dans l'entreprise y compte).
/// Seule la valeur que l'import a posee, `pose_le`, se remplace : l'anciennete est un fait du passe (`FAITS_SANS_DATE`),
/// elle se corrige. Une date saisie depuis reste la sienne. Rend vrai si elle a ete remplacee.
pub(crate) fn reprendre_anciennete(ctx: &Contexte, salarie: &IdSalarie, pose_le: Jour, debut: Jour) -> R<bool> {
    let contenu = lire_fichier::<FicheSalarieDatee>(ctx)?;
    // La seule valeur de la fiche, celle que l'import a posee, quand le contrat commence avant elle.
    let posee = contenu.fiches.get(salarie).and_then(|f| f.date_anciennete.as_deref()).filter(|v| v.len() == 1).and_then(|v| v.first());
    let a_remplacer = posee.filter(|seule| seule.valeur.dite() == Some(&pose_le) && debut < pose_le);
    // La note de l'import (d'ou vient la valeur) reste : la valeur vient toujours des pieces importees.
    let remplacer = |seule: &droit::fiche::Datee<Jour>| poser::<FicheSalarieDatee>(ctx, Some(salarie), vec![ChangementSalarie::DateAnciennete(Some(debut))], None, seule.note.as_deref());
    Ok(a_remplacer.map(remplacer).transpose()?.is_some())
}



/// LES LICENCES QUI SUIVENT LA PREMIERE (`routes::import::poser_licence`) : chacune vaut a partir du premier jour du mois
/// ou les pieces importees la portent, comme un numero renouvele saisi « a partir du » (`poser`). Seulement quand la
/// fiche, sur le disque, porte pour seule licence la premiere que les pieces donnent : une licence saisie autrement reste
/// ce qu'elle est.
pub(crate) fn dater_licences_suivantes(ctx: &Contexte, licences: &[(socle::mois::Mois, String)], note: Option<&str>) -> R<()> {
    let Some(((_, premiere), suivantes)) = licences.split_first().filter(|(_, s)| !s.is_empty()) else { return Ok(()) };
    let fiche = lire_fichier::<FicheEntrepriseDatee>(ctx)?;
    if !matches!(fiche.licence_spectacle.as_deref(), Some([seule]) if seule.depuis.is_none() && seule.valeur.dite() == Some(premiere)) {
        return Ok(());
    }
    let note = note.filter(|n| !n.trim().is_empty());
    for (mois, numero) in suivantes {
        poser_entreprise_note(ctx, ChangementEntreprise::LicenceSpectacle(Some(numero.clone())), mois.premier_jour(), note)?;
    }
    Ok(())
}

/// Une case de la fiche entreprise, a partir de `jour`, avec sa note.
fn poser_entreprise_note(ctx: &Contexte, case: ChangementEntreprise, jour: Jour, note: Option<&str>) -> R<()> {
    poser::<FicheEntrepriseDatee>(ctx, None, vec![case], Some(jour), note)
}

// ------------------------------------------------------------------ la fiche demandee par courriel

/// Secondes entre une demande de fiche et la reponse de la personne fictive de la compagnie fictive (`DELAI_SIGNATURE`).
pub(crate) const DELAI_FICTIVE: i64 = 20;

/// Une adresse courriel de forme `x@y.z`, sans blanc ni second `@`.
fn adresse_valide(adresse: &str) -> bool {
    let blanc = socle::texte::blanc;
    if adresse.chars().any(blanc) || adresse.matches('@').count() != 1 {
        return false;
    }
    adresse.split_once('@').is_some_and(|(local, domaine)| {
        let d: Vec<char> = domaine.chars().collect();
        !local.is_empty() && d.len() >= 3 && (1..d.len() - 1).any(|i| d[i] == '.')
    })
}

/// Ce que `demander_fiche` rend : le code, l'adresse (en minuscules), la ligne du journal.
pub struct FicheDemandee {
    pub code: CodeDeDemande,
    pub adresse: droit::fiche::Courriel,
    pub ligne: envois::journal::Ligne,
}

/// Envoie a `adresse` le courriel « Remplir ma fiche ». Le journal garde
/// le code : c'est par lui que la releve reconnaitra la reponse. `client` : la boite, ouverte ici (boite simulee dans une

/// La charge de `fiche-demander` : l'adresse, le prenom (pour lui ecrire), et le mot de passe du logiciel tape pour l'envoi.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeFicheDemander {
    /// Lue a la porte dans son type (sans blancs de bord, de 6 a 100 caracteres).
    adresse: droit::fiche::Courriel,
    prenom: Option<socle::forme::TexteDit>,
}

/// Ce que rend `fiche-demander`.
#[derive(Debug, serde::Serialize)]
struct DemandeDeFiche {
    phrase: String,
    code: CodeDeDemande,
    adresse: droit::fiche::Courriel,
}

/// `fiche-demander` : le courriel « Remplir ma fiche » part (`demander_fiche`), sous la preuve que la charge porte.
fn fiche_demander(ctx: &Contexte, c: ChargeFicheDemander, motdepasse: Option<&socle::secret::Secret>) -> R<DemandeDeFiche> {
    let preuve = Preuve::du_mot_de_passe(motdepasse.cloned());
    let d = demander_fiche(ctx, &c.adresse, c.prenom.as_deref(), &preuve, INTERFACE, None)?;
    Ok(DemandeDeFiche { phrase: format!("Demande de fiche envoyée à {} (code {}) : sa fiche se crée à l'arrivée de sa réponse.", d.adresse, d.code), code: d.code, adresse: d.adresse })
}
/// compagnie fictive, sinon refus « boite absente ») quand l'appelant n'en donne pas.
pub(crate) fn demander_fiche(ctx: &Contexte, adresse: &droit::fiche::Courriel, prenom: Option<&str>, preuve: &Preuve, qui: &str, client: Option<&mut dyn Courriel>) -> R<FicheDemandee> {
    // L'adresse lue a la porte s'ecrit en minuscules, comme toute adresse que le moteur ecrit ; elle a la forme x@y.z.
    let adresse = droit::fiche::Courriel::lire(&adresse.to_lowercase())?;
    if !adresse_valide(&adresse) {
        return Err(refus(format!("« {} » n'est pas une adresse de courriel.", adresse)));
    }
    let mut ouverte: Option<Box<dyn Courriel>> = None;
    let client: &mut dyn Courriel = match client {
        Some(c) => c,
        None => ouverte.insert(client_courriel(ctx)?).as_mut(),
    };
    let mois = socle::temps::jour_de_paris().mois_civil();
    verifier_preuve(ctx, Some(preuve), socle::mois::MoisOuJour::Mois(mois), "demande de fiche (« Remplir ma fiche »)", &adresse)?;

    let compagnie = ctx.entreprise.nommee().ok_or_else(|| socle::fichiers::absent("raison_sociale"))?.to_string();
    let sg = ctx.entreprise.signataire.dite().ok_or_else(|| socle::fichiers::absent("signataire"))?;
    let signataire = match sg.prenom_affichage.remplie() {
        Some(p) => p.to_string(),
        None => capitaliser(sg.prenom.portee().ok_or_else(|| socle::fichiers::absent("prenom"))?),
    };
    let expediteur = droit::fiche::Courriel::lire(&client.compte().to_lowercase())?;
    let code = fq::code_au_hasard(fq::SorteDeDemande::Fiche).map_err(|e| e.reclasser(Classe::Disque))?;
    let utiles = champs::utiles(NomDeFiche::Salarie)?;
    let q = fq::questionnaire(code.texte(), &fq::posees(&|c| utiles.contains(&c)), &|_| String::new());
    let sujet = envoi::formater(fq::SUJET, &[("compagnie", compagnie.clone())]);
    let prenom: Option<String> = prenom.map(|p| strip(p).to_string());
    // « Bonjour Claire, » : le prenom, quand il est dit ; « Bonjour, » sinon.
    let virgule_prenom: String = prenom.iter().map(|p| format!(" {}", p)).collect();
    let champs = |echappe: bool| -> Vec<(&'static str, String)> {
        let e = |s: &str| if echappe { html_echapper(s) } else { s.to_string() };
        vec![("virgule_prenom", e(&virgule_prenom)), ("compagnie", e(&compagnie)), ("signataire", e(&signataire))]
    };
    let lien = mailto(&expediteur, &format!("Re: {}", sujet), &q);
    let mut texte = champs(false);
    texte.push(("questionnaire", q.clone()));
    let mut html = champs(true);
    html.push(("questionnaire", html_echapper(&q)));
    html.push(("mailto", html_echapper(&lien)));
    let message = Message {
        // Celui a qui sa fiche est demandee n'en a pas encore : la demande se retrouve par son code, au journal.
        pour: (),
        nom: match &prenom {
            Some(p) => p.clone(),
            None => adresse.to_string(),
        },
        destinataire: adresse.to_string(),
        sujet,
        corps: envoi::formater(fq::CORPS, &texte),
        html: Some(envoi::formater(fq::HTML, &html)),
        pieces: Vec::new(),
    };
    let mut identifiants: Vec<envoi::IdDeCourriel> = Vec::new();
    let demande = |message: Option<envoi::IdDeCourriel>, expediteur: Option<droit::fiche::Courriel>| journal::DetailFiche { code: code.clone(), adresse: adresse.clone(), prenom: prenom.clone(), message, expediteur };
    if let Err(e) = envoi::envoyer(&[message], &expediteur, client, Some(&mut identifiants), None) {
        noter(ctx, journal::Ligne::FicheDemande(par_courriel(None, qui, false, &adresse, &e.phrase, demande(None, None), None)))?;
        return Err(e);
    }
    let detail = demande(identifiants.first().cloned(), Some(expediteur.clone()));
    let ligne = noter(ctx, journal::Ligne::FicheDemande(par_courriel(None, qui, true, &adresse, &format!("fiche demandee a {}", adresse), detail, None)))?;
    Ok(FicheDemandee { code, adresse, ligne })
}

// ------------------------------------------------------------------ les demandes attendues

/// Une demande partie et pas encore revenue.
#[derive(Debug, Clone)]
pub(crate) struct Attendue {
    pub code: CodeDeDemande,
    pub adresse: droit::fiche::Courriel,
    pub prenom: Option<String>,
    /// L'heure de la demande, celle de la ligne du journal.
    pub quand: InstantOuJour,
    /// Les identifiants de NOS envois (le `Message-ID` gardé avec la demande) : jamais pris pour une reponse.
    pub envoyes: Vec<envoi::IdDeCourriel>,
}

impl Attendue {
    /// Ce texte, recu de la boite, est-il l'identifiant d'un de NOS envois ?
    fn est_a_nous(&self, recu: &str) -> bool {
        self.envoyes.iter().any(|i| i.texte() == recu)
    }
}

/// La ligne `l` est-elle la demande de fiche partie sous le code `code` ? Ce qui en revient se range sur elle.
fn demande_de(l: &journal::Ligne, code: &CodeDeDemande) -> bool {
    l.fiche().is_some_and(|e| e.accepte && e.detail.code == *code)
}

/// `fiches_attendues` : les demandes parties et pas encore revenues, les plus anciennes d'abord. Une demande a
/// laquelle on renonce se retire (`fiche-oublier-demande`, `fiches`).
pub(crate) fn fiches_attendues(ctx: &Contexte) -> R<Vec<Attendue>> {
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let mut sortie = Vec::new();
    for e in lignes.iter().filter_map(journal::Ligne::fiche) {
        if !e.accepte || !e.retours.is_empty() {
            continue;
        }
        sortie.push(Attendue {
            code: e.detail.code.clone(),
            adresse: e.detail.adresse.clone(),
            prenom: e.detail.prenom.clone(),
            quand: e.quand(),
            envoyes: e.detail.message.clone().into_iter().collect(),
        });
    }
    Ok(sortie)
}

// ------------------------------------------------------------------ la releve

/// Ce que la releve a fait : combien de demandes attendaient, les fiches creees, ce qui l'a empechee.
#[derive(Debug)]
pub struct ReleveFiches {
    pub attendues: usize,
    /// Chaque fiche creee ou completee.
    pub recues: Vec<FicheRecue>,
    pub incidents: Vec<String>,
}

/// Une fiche recue : le code de sa demande, sa cle, le nom de la personne, ce qui ne s'est pas lu, ses erreurs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FicheRecue {
    code: CodeDeDemande,
    cle: Option<IdSalarie>,
    nom: String,
    illisible: Vec<String>,
    erreurs: usize,
}

impl ReleveFiches {
    /// `ReleveFiches.resume` : la phrase de la veille, vide quand rien n'est arrive.
    pub fn resume(&self) -> String {
        if self.recues.is_empty() {
            return String::new();
        }
        let noms: Vec<String> = self.recues.iter().map(|r| r.nom.clone()).collect();
        format!("{} : {}", socle::ecran::compte(self.recues.len(), "fiche de salarié reçue", "fiches de salarié reçues"), noms.join(", "))
    }
}

/// Cherche les reponses aux demandes de fiche et cree chaque fiche.
///
/// `client` : la boite, ouverte ici quand l'appelant n'en donne pas -- la personne fictive de la compagnie fictive, ou la
/// vraie boite ; sans boite, un incident et rien d'autre. `qui` : `interface`, ou `veille`.
pub(crate) fn relever_fiches(ctx: &Contexte, client: Option<&mut dyn Courriel>, qui: &str) -> R<ReleveFiches> {
    let attendues = fiches_attendues(ctx)?;
    let mut r = ReleveFiches { attendues: attendues.len(), recues: Vec::new(), incidents: Vec::new() };
    if attendues.is_empty() {
        return Ok(r);
    }
    let mut ouverte: Option<Box<dyn Courriel>> = None;
    let branchement = compagnie::bridage::branchement(&ctx.racine)?;
    let client: &mut dyn Courriel = match client {
        Some(c) => c,
        None if branchement.joue() => {
            // LA COMPAGNIE FICTIVE : la personne fictive repond, et c'est la vraie lecture qui suit.
            ouverte.insert(Box::new(boite_des_fiches(ctx, &attendues, DELAI_FICTIVE)?)).as_mut()
        }
        None => {
            // Le message : rien n'est parti, rien ne revient.
            if let Some(refus) = branchement.refus() {
                r.incidents.push(refus.phrase.to_string());
                return Ok(r);
            }
            if !compagnie::boite::disponible(&ctx.racine)? {
                r.incidents.push("Aucune boite courriel : les fiches demandees ne peuvent pas etre relevees.".to_string());
                return Ok(r);
            }
            let vrai_client = compagnie::boite::client_du_contexte(ctx)?;
            ouverte.insert(Box::new(vrai_client)).as_mut()
        }
    };
    let expediteur = client.compte().to_lowercase();

    for a in &attendues {
        let code = &a.code;
        let identifiants = match client.rechercher(&format!("\"{}\"", code), 10) {
            Ok(i) => i,
            Err(e) => {
                r.incidents.push(format!("Recherche de {} : {}", code, e.phrase));
                break;
            }
        };
        let mut retenu: Option<(String, Vec<u8>, String, droit::fiche::Courriel, Moment)> = None;
        for ident in identifiants {
            if a.est_a_nous(&ident) {
                continue;
            }
            let brut = match client.message_brut(&ident) {
                Ok(b) => b,
                Err(e) => {
                    r.incidents.push(format!("Lecture du message {} : {}", ident, e.phrase));
                    continue;
                }
            };
            let m = match mailparse::parse_mail(&brut) {
                Ok(m) => m,
                Err(e) => {
                    r.incidents.push(format!("Lecture du message {} : message illisible ({}).", ident, e));
                    continue;
                }
            };
            if lecture::identifiant_de(&m).is_some_and(|i| a.est_a_nous(&i)) {
                continue;
            }
            let texte = lecture::texte_propre(&m);
            if !texte.to_uppercase().contains(code.texte()) || texte.contains("Remplir ma fiche") {
                continue;
            }
            let de = match lecture::adresse_de(&m) {
                lecture::Expediteur::Adresse(de) => de,
                sans_adresse => {
                    r.incidents.push(format!("Fiche {} : une reponse ne dit pas d'ou elle vient ({}) ; elle n'est pas retenue.", code, sans_adresse.pourquoi_aucune_adresse()));
                    continue;
                }
            };
            if de == expediteur && de != *a.adresse {
                continue;
            }
            if de != *a.adresse {
                r.incidents.push(format!("Fiche {} : une reponse vient de {} et non de {} ; elle n'est pas retenue.", code, de, a.adresse));
                continue;
            }
            let quand = lecture::date_de(&m, socle::temps::secondes().a_la_seconde());
            // Elle vient de l'adresse de la demande, a la lettre pres.
            retenu = Some((ident, brut.clone(), texte, a.adresse.clone(), quand));
            break;
        }
        let Some((ident, brut, texte, de, quand)) = retenu else { continue };
        let (lue, mut illisible) = fq::fiche_depuis_reponses(&fq::lire_reponses(&texte)).map_err(|m| m.reclasser(Classe::Illisible))?;
        // La reponse se lit par la porte d'une fiche saisie : une case hors de son type reste vide, et se dit a completer
        // dans la note, avec ce que le questionnaire n'a pas lu.
        let (mut recue, fautes) = import::proposee::de::<droit::fiche::ChampsSalarie, _>(&lue).map_err(|m| m.reclasser(Classe::Illisible))?;
        illisible.extend(fautes.into_iter().map(|f| format!("{} : {}", f.case.nom(), f.phrase)));
        recue.email = Some(de.clone());
        let nom = [recue.prenom_affichage.remplie(), recue.nom_affichage.remplie()].into_iter().flatten().collect::<Vec<_>>().join(" ");
        if nom.trim().is_empty() {
            r.incidents.push(format!("Fiche {} : la reponse de {} ne porte ni nom ni prenom lisible ; rien n'est cree.", code, de));
            continue;
        }
        // La reponse, le courriel tel qu'il est arrive : une piece produite, rangee hors d'un mois.
        let sorte = documents::SorteDePieceProduite::FicheDeSalarie;
        let dossier = documents::DossierHorsDuMois::FichesDeSalaries.sous(&ctx.racine);
        std::fs::create_dir_all(&dossier).map_err(|e| socle::echec::du_disque(&dossier, &e).reclasser(Classe::Disque))?;
        let eml = dossier.join(format!("{}{}.eml", sorte.prefixe(), code));
        std::fs::write(&eml, &brut).map_err(|e| socle::echec::du_disque(&eml, &e).reclasser(Classe::Disque))?;
        let mut note = format!("Fiche remplie par la personne, par courriel, le {}. Reste à choisir l'emploi.", quand.jour_de_paris().jj_mm_aaaa());
        if !illisible.is_empty() {
            note.push_str(&format!(" Non lu, à compléter : {}.", illisible.join(" ; ")));
        }
        let trouvee = ctx.salaries.iter().find(|(_, s)| s.email.portee().is_some_and(|e| strip(e).to_lowercase() == *de));
        let existante: Option<IdSalarie> = trouvee.map(|(k, _)| k.clone());
        // Une fiche qui existe deja : on ne complete que ses cases VIDES (ce qu'elle porte a peut-etre deja ete declare) ;
        // une fiche neuve prend toutes celles de la reponse, active. La note dit d'ou viennent les valeurs.
        let deja = trouvee.map(|(_, s)| s.champs.clone());
        let dites = droit::fiche::ChampsSalarie::default().cases_differentes(&recue);
        let p = preparer::<FicheSalarieDatee>(ctx, existante.as_ref(), |plate| {
            let mut nommees = Vec::new();
            for k in dites {
                let a_remplir = deja.as_ref().is_none_or(|v| case_vide(v.valeur_ecrite(k).as_ref()));
                if let (true, Some(ch)) = (a_remplir, <FicheSalarieDatee as droit::fiche::FicheDatee>::changement(&recue, k)) {
                    <FicheSalarieDatee as droit::fiche::FicheDatee>::appliquer(plate, ch);
                    nommees.push(k);
                }
            }
            if deja.is_none() {
                plate.actif = Some(true);
                nommees.push(droit::fiche::CaseSalarie::Actif.into());
            }
            nommees
        })?;
        // Les cases vides qu'elle remplit valent de tout temps ; ce qu'elle change d'une fiche qui existe se date comme un
        // changement de l'ecran (`date_proposee`).
        let depuis = match &existante {
            Some(k) => Some(date_proposee(ctx, NomDeFiche::Salarie, Some(k))?.0),
            None => None,
        };
        let f = ecrire_fiche::<FicheSalarieDatee>(ctx, p, Forcer::Tout, depuis, Some(&note))?;
        let erreurs = f.anomalies.iter().filter(|a| a.gravite == champs::Gravite::Erreur).count();
        let retour = journal::RetourFiche {
            code: code.clone(),
            abandon: false,
            salarie: f.cle.clone(),
            illisible: Some(illisible.clone()),
            erreurs: Some(erreurs),
            quand,
            qui: qui.to_string(),
            de: Some(de.clone()),
            message: Some(ident.clone()),
            reponse: format!("fiche {} par courriel", if existante.is_some() { "completee" } else { "creee" }),
        };
        journal::rattacher(&ctx.racine, |l| demande_de(l, &code), journal::Revenu::Fiche(retour)).map_err(|e| e.reclasser(Classe::Disque))?;
        r.recues.push(FicheRecue { code: code.clone(), cle: f.cle, nom: strip(&nom).to_string(), illisible, erreurs });
    }
    client.fermer()?;
    Ok(r)
}

// ------------------------------------------------------------------ la compagnie fictive

/// Des personnes inventees, comme celles des compagnies fictives : (prenom, nom, sexe INSEE). Le NIR se
/// fabrique a la demande, cle comprise.
const PERSONNES: [(&str, &str, &str); 4] = [("Marie", "Dorval", "2"), ("Pierre", "Bocage", "1"), ("Rose", "Cheri", "2"), ("Frederick", "Lemaitre", "1")];

/// Le questionnaire que la personne fictive renvoie, rempli ligne a ligne dans les termes memes
/// des questions, le code en dernier. `bouton` : (objet, corps) du lien du courriel de la demande, quand on l'a.
fn reponse_de_fiche(demande: &Attendue, compte: &str, bouton: Option<&(String, String)>) -> R<Vec<u8>> {
    let code = demande.code.texte();
    let somme: u32 = code.chars().map(|c| c as u32).sum();
    let (prenom, nom, sexe) = PERSONNES[somme as usize % PERSONNES.len()];
    let prenom = match &demande.prenom {
        Some(p) => p.clone(),
        None => prenom.to_string(),
    };
    let an = 80 + code.chars().count() % 15;
    let mois = 1 + somme % 12;
    let treize = format!("{}{:02}{:02}75056{:03}", sexe, an, mois, 1 + somme % 900);
    let cle = 97 - treize.parse::<u64>().map_err(|e| erreur(Classe::Illisible, e.to_string()))? % 97;
    let reponses: HashMap<&str, String> = HashMap::from([
        ("prenom", prenom),
        ("nom", nom.to_string()),
        ("naissance", format!("12/{:02}/19{:02}", mois, an)),
        ("lieu", "Paris".to_string()),
        ("pays", "France".to_string()),
        ("nationalite", "française".to_string()),
        ("nir", format!("{}{:02}", treize, cle)),
        ("adresse", "12 rue du Temple".to_string()),
        ("code_postal", "75003".to_string()),
        ("commune", "Paris".to_string()),
        // Le compte fabrique des fictives, a la cle juste (`socle::iban::faute_iban`).
        ("iban", "FR7630001007941234567890185".to_string()),
    ]);
    let utiles = champs::utiles(NomDeFiche::Salarie)?;
    // Une question que la personne fictive ne sait pas, elle la laisse sans reponse.
    let rempli = fq::questionnaire(code, &fq::posees(&|c| utiles.contains(&c)), &|cle| reponses.get(cle).into_iter().cloned().collect());
    let sujet = match bouton {
        Some(b) => b.0.clone(),
        None => "Re: fiche".to_string(),
    };
    envois::simulation_courriels::reponse_jouee(&demande.adresse, compte, &sujet, &rempli, &format!("fiche-{}", code))
}

/// La boite de la compagnie fictive, vue par la releve. Chaque demande de fiche partie
/// depuis `delai` secondes y trouve sa reponse remplie ; la vraie releve la lit et cree la fiche. Ce qu'on y envoie
/// s'ecrit comme tout courriel joue (`BoiteQuiRepond`, partagee avec `BoiteDesSignatures`).
pub(crate) fn boite_des_fiches(ctx: &Contexte, attendues: &[Attendue], delai: i64) -> R<BoiteQuiRepond> {
    let mut boite = BoiteQuiRepond::nouvelle(ctx)?;
    let maintenant = socle::temps::secondes();
    // Les courriels joues, par leur `Message-ID` : la demande retrouve ainsi son bouton.
    let envoyes = envois::simulation_courriels::courriels_par_identifiant(&ctx.racine)?;
    for a in attendues {
        // Une demande datee d'un jour seul est partie a minuit de ce jour.
        let partie = match a.quand {
            InstantOuJour::Instant(i) => Some(i),
            InstantOuJour::Jour(j) => Moment::a_paris(j, 0, 0, 0),
        };
        let Some(partie) = partie else { continue };
        if !maintenant.depuis(partie).is_some_and(|d| d.as_secs() as i64 >= delai) {
            continue;
        }
        let bouton = envois::simulation_courriels::bouton_de_la_demande(&envoyes, &a.envoyes)?;
        let brut = reponse_de_fiche(a, Courriel::compte(&boite), bouton.as_ref()).map_err(|m| m.reclasser(Classe::Illisible))?;
        boite.repondre(a.code.texte(), brut);
    }
    Ok(boite)
}

/// La charge de `couts` : l'annee.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeCouts {
    annee: i32,
}

/// Le cout d'un spectacle ou d'un salarie (`documents::fiche_recapitulative::Cout`), et sa ligne telle que sa fiche la
/// montre.
#[derive(Debug, serde::Serialize)]
struct CoutDit {
    #[serde(flatten)]
    cout: documents::fiche_recapitulative::Cout,
    phrase: String,
}

/// Ce que rend `couts` : le cout de l'annee, spectacle par spectacle et salarie par salarie, le total en tete.
#[derive(Debug, serde::Serialize)]
struct CoutsDits {
    phrase: String,
    phrases: Vec<String>,
    annee: socle::mois::Annee,
    jusqu_a: String,
    #[serde(serialize_with = "socle::ecriture::en_ordre")]
    spectacles: Vec<(String, CoutDit)>,
    #[serde(serialize_with = "socle::ecriture::en_ordre")]
    salaries: Vec<(String, CoutDit)>,
}

dites!(EffectifDit, CoutsDits, DemandeOubliee);

/// `couts` : le cout de l'annee, au jour du contexte (`documents::fiche_recapitulative::Recapitulatifs::couts_de_l_annee`).
fn couts(ctx: &Contexte, c: ChargeCouts) -> R<CoutsDits> {
    use socle::texte::euros;
    let mut r = documents::fiche_recapitulative::Recapitulatifs::nouveau(ctx, None, Some(socle::mois::Annee::nouvelle(i64::from(c.annee))?), false, envois::journal::lu(&ctx.racine).map(|j| j.departs)?);
    let couts = r.couts_de_l_annee()?;
    let bulletins = |x: &documents::fiche_recapitulative::Cout| socle::ecran::compte(x.bulletins as usize, "bulletin", "bulletins");
    // La ligne de chaque spectacle et de chaque salarie, telle que sa fiche la montre.
    let dit = |liste: Vec<(String, documents::fiche_recapitulative::Cout)>| -> R<Vec<(String, CoutDit)>> {
        liste
            .into_iter()
            .map(|(k, x)| {
                // Rien de saisi pour la suite de l'annee : la phrase n'en dit rien.
                let prevu: String = (!x.prevu.is_zero()).then(|| format!(" · + {} déjà saisis pour la suite de l'année", euros(x.prevu))).into_iter().collect();
                let phrase = format!(
                    "coût employeur {} — brut {}, charges {}, {}{}{}",
                    euros(x.cout), euros(x.brut), euros(x.charges), bulletins(&x),
                    if x.incomplet { ", coût incomplet avant PuppetPaie" } else { "" },
                    prevu
                );
                Ok((k, CoutDit { cout: x, phrase }))
            })
            .collect()
    };
    let mut total = socle::num::Decimal::ZERO;
    let mut phrases = Vec::new();
    for (nom, x) in &couts.spectacles {
        total += x.cout;
        phrases.push(format!("{} : {} ({}, brut {})", nom, euros(x.cout), bulletins(x), euros(x.brut)));
    }
    let phrase = format!(
        "Coût de {}, jusqu'à {} : {} pour {}.",
        couts.annee,
        couts.jusqu_a.libelle(socle::mois::Graphie::Accentuee),
        socle::texte::euros(total),
        socle::ecran::compte(couts.spectacles.len(), "spectacle", "spectacles")
    );
    Ok(CoutsDits { phrase, phrases, annee: couts.annee, jusqu_a: couts.jusqu_a.to_string(), spectacles: dit(couts.spectacles)?, salaries: dit(couts.salaries)? })
}

/// `poser-courriel` : le courriel `email` du salarie `cle`, saisi la ou il manque (la boite des contrats a
/// faire signer). La fiche entiere s'ecrit comme depuis l'ecran (`ecrire_fiche`), seul le courriel y change : il se
/// verifie avec elle, et une fiche qui refuse le dit.
pub(crate) fn poser_courriel(ctx: &Contexte, cle: &IdSalarie, email: &droit::fiche::Courriel) -> R<()> {
    if !ctx.salaries.contains_key(cle) {
        return Err(refus(format!("Salarié {} inconnu.", socle::fichiers::repr(cle.as_str()))));
    }
    let p = preparer::<FicheSalarieDatee>(ctx, Some(cle), |plate| {
        plate.email = Some(email.clone());
        vec![droit::fiche::CaseSalarie::Email.into()]
    })?;
    let (depuis, _) = date_proposee(ctx, NomDeFiche::Salarie, Some(cle))?;
    ecrire_fiche::<FicheSalarieDatee>(ctx, p, Forcer::Rien, Some(depuis), None)?;
    Ok(())
}
