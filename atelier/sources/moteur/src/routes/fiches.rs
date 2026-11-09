//! Configuration › Entreprise, Salariés, Spectacles : les fiches des salaries, des spectacles et de l'entreprise
//! (`verifier-fiche`, `activer-fiche`, `enregistrer-fiche`, leur avertissement, `permanents`), leur demande par courriel
//! (`fiche-demander`, `fiches-relever`, `fiche-oublier-demande`), le taux AT/MP (`taux-atmp`), le regime de la taxe sur
//! les salaires, le cout de l'annee (`couts`), la sauvegarde et la suppression de la compagnie.
//!
//! Les refus portent leur classe (`Manque`, `MalFormee`, `Refuse`...), que la page releve.

use droit::fiche::Lue as _;
use std::collections::HashMap;
use std::path::Path;

use crate::compagnie::depot_acte::{verifier_preuve, Preuve};
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, FORME, ILLISIBLE, MANQUE, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::documents::client_courriel;
use crate::routes::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use crate::routes::INTERFACE;
use declarations::champs::{self, Saisie};
use documents::fiche_questionnaire as fq;
use droit::contexte::Contexte;
use droit::fiche::{ChangementEntreprise, ChangementSalarie, ChangementSpectacle, FicheEntrepriseDatee, FicheSalarieDatee, FicheSpectacleDatee};
use envois::courriel::Courriel;
use envois::courriel_lecture as lecture;
use envois::envoi::{self, Message};
use envois::journal;
use envois::ligne_d_envoi::{echec_d_envoi, noter, par_courriel};
use envois::simulation_courriels::BoiteQuiRepond;
use socle::date::Jour;
use socle::echec::{erreur, Classe, Echec, R};
use socle::ecriture::ecrire_json;
use socle::html::{html_echapper, mailto};
use socle::texte::{capitaliser, strip};

mod ecriture;
pub(crate) use ecriture::{avec_libelles, deriver, ecrire_fiche, lire_fichier, poser, preparer, reporter, vide, vue, ChangementDit, Forcer, SaisieDeFiche, SorteDeFiche};

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
            Champ { nom: "donnees", libelle: Affiche::new("Fiche en cours"), forme: Forme::Objet, sens: "la fiche en cours de saisie ; pour un salarié ou un spectacle, l'entreprise est celle de la compagnie", avance: false },
        ],
        exemple: r#"{"nom":"entreprise","donnees":{}}"#,
        rend: "permanents (vrai ou faux), montres (les cases que la fiche montre)",
        refus: &[FORME],
        acte: Acte::Lit,
        avance: false,
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
        avance: false,
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
        avance: false,
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
        avance: false,
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
        avance: false,
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
        avance: false,
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
        avance: false,
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
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::TauxAtmp,
        geste: "Dire le code risque et le taux AT/MP d'une année ; les poser s'il y a lieu.",
        charge: &[
            Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année, AAAA", avance: false },
            Champ { nom: "code_risque", libelle: Affiche::new("Code risque"), forme: Forme::Texte, sens: "le code risque notifié par la CARSAT", avance: false },
            Champ { nom: "valeur", libelle: Affiche::new("Taux net"), forme: Forme::Nombre, sens: "le taux net notifié, en %", avance: false },
        ],
        exemple: r#"{"annee":2026}"#,
        rend: "le code, le taux, leur libellé et d'où ils viennent, les codes que le socle connaît ; phrase : ce que le geste a écrit",
        refus: &[OPERATION, MANQUE, FORME, ILLISIBLE],
        acte: Acte::Ecrit,
        avance: true,
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
        avance: true,
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
        avance: false,
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
        avance: true,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Fiches),
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
        avance: true,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Fiches),
    },
];
/// Une route de ce module, branchee sur l'enveloppe. La suppression de la compagnie lit encore sa charge en JSON
/// (`Charge::brute`, `interface::suppression`) et rend le sien (`Rendu::brut`).
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::Permanents => en_json(permanents(ctx, charge.lire()?)?),
        Route::TaxeSurLesSalairesRegime => en_json(taxe_sur_les_salaires_regime(ctx, charge.lire()?)?),
        Route::TauxAtmp => en_json(crate::routes::atmp::route(ctx, charge.lire()?)?),
        Route::FicheDemander => en_json(fiche_demander(ctx, charge.lire()?, charge.mot_de_passe())?),
        Route::FicheOublierDemande => en_json(oublier_demande(ctx, charge.lire()?)?),
        Route::Effectif => en_json(effectif(ctx, charge.lire()?)?),
        Route::Couts => en_json(couts(ctx, charge.lire()?)?),
        Route::ActiverFiche => en_json(activer_fiche_route(ctx, charge.lire()?)?),
        Route::SauvegardeMaintenant => {
            let super::SansCharge {} = charge.lire()?;
            let faite: SauvegardeFaite = sauvegarde_maintenant(ctx);
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
        Route::VerifierFiche => en_json(self::ecriture::verifier_fiche(charge.lire()?)?),
        Route::EnregistrerFiche => en_json(ecriture::enregistrer_fiche(ctx, charge.lire()?)?),
        // Supprimer la compagnie : avertir, puis effacer. Par `moteur route`, rien a lacher ; la fenetre sert elle-meme la
        // suppression confirmee, pour lacher d'abord ce qu'elle tient de la compagnie (`interface::Fenetre`).
        Route::CompagnieSupprimer => crate::interface::suppression::route(&ctx.racine, charge.brute(), &|_| {}).map(Rendu::brut).map_err(|m| erreur(Classe::Refuse, m)),
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

/// Ce que rend `activer-fiche` : la phrase, et ce que la page relit de la compagnie.
#[derive(Debug, serde::Serialize)]
struct FicheActivee {
    phrase: String,
    etat: crate::routes::ouverture::EtatDeLaPage,
}

/// Ce que rend `sauvegarde-maintenant` : la derniere sauvegarde (`compagnie::sauvegarde`), et ce qui l'a empechee.
#[derive(Debug, serde::Serialize)]
struct SauvegardeFaite {
    phrase: String,
    sauvegarde: crate::compagnie::sauvegarde::Derniere,
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
#[serde(deny_unknown_fields)]
struct ChargeActiver {
    nom: String,
    cle: String,
    actif: bool,
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

/// La sauvegarde seule, comme la seconde moitie d'un passage de la veille (`compagnie::ouverture`).
fn sauvegarde_maintenant(ctx: &Contexte) -> SauvegardeFaite {
    use crate::compagnie::ouverture::{ouverture, Clients, Options};
    let opt = Options { sauvegarde_seule: true, incidents_editeur: false, ..Options::default() };
    let r = ouverture(ctx, &opt, Clients::default(), None);
    let erreurs: Vec<String> = r.manquants.into_iter().chain(r.incidents).collect();
    let s = crate::compagnie::sauvegarde::derniere(&ctx.racine);
    let phrase = match (erreurs.is_empty(), s.quand.as_str().filter(|q| !q.is_empty())) {
        (false, _) => format!("Sauvegarde : {}", erreurs.join(" ")),
        (true, None) => "Aucune sauvegarde faite.".to_string(),
        (true, Some(q)) => format!("Dernière sauvegarde : {}, {} fichiers.", q, s.fichiers),
    };
    SauvegardeFaite { phrase, sauvegarde: s, erreur: (!erreurs.is_empty()).then(|| erreurs.join(" ")) }
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
        let e = ctx.effectif().map_err(Echec::from)?;
        let mut v = e.a_dire(an_paie);
        let demande = declarations::demarrage::manques(ctx).map_err(Echec::from)?.iter().any(|m| m.cle == "effectif");
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
    let en_clair = |x: f64| -> R<String> { Ok(socle::texte::virgule(socle::num::dec_f64(x).map_err(Echec::from)?.round_dp(2))) };
    let mut pose = None;
    if let Some(v) = &charge.valeur {
        if an >= an_paie {
            return Err(erreur(Classe::Refuse, format!("L'effectif de {} se calcule sur l'année entière : il se dit une fois l'année finie. Rien n'est écrit.", an)));
        }
        let x = v.flottant().map_err(|m| erreur(Classe::Illisible, m))?;
        if !x.is_finite() || x < 0.0 {
            return Err(erreur(Classe::Illisible, format!("Effectif illisible : {}.", v.ecrit())));
        }
        // Tronque au centieme (R. 130-1, I).
        let x = (x * 100.0).trunc() / 100.0;
        let le = ctx.aujourdhui.iso();
        if !envois::releve_retours::poser_effectif(&ctx.racine, an, x, &le)? {
            return Err(erreur(Classe::Refuse, format!("L'Urssaf a renvoyé l'effectif de {} : c'est le sien qui vaut. Rien n'est écrit.", an)));
        }
        pose = Some(format!("Effectif de {} : {}.", an, en_clair(x)?));
    }
    let e = ctx.effectif().map_err(Echec::from)?;
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
        (None, Some(v), Some(s)) => format!("Effectif de {} : {}, d'après {}.", an, en_clair(v)?, s),
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
    valeur: Option<f64>,
    source: Option<&'static str>,
    a_dire: Vec<i32>,
}

// ------------------------------------------------------------------ permanents

/// Ce que rend `permanents` : la phrase, le curseur des permanents, et les cases que la fiche montre.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct Permanents {
    phrase: &'static str,
    permanents: bool,
    montres: Vec<String>,
}

/// `permanents` : rend le curseur (`contexte::emploie_des_permanents`) : la page montre ou replie ce qui ne vaut que
/// pour les permanents selon cette reponse, et ne le decide pas elle-meme quand on coche la case.
fn permanents(ctx: &Contexte, charge: SaisieDeFiche) -> R<Permanents> {
    let (nom, donnees) = charge.ouverte();
    let entreprise = if nom == "entreprise" { donnees.clone() } else { Saisie::de_la_fiche("entreprise", &ctx.entreprise)? };
    let p = permanents_de(&entreprise);
    Ok(Permanents {
        phrase: if p { "La compagnie emploie des permanents." } else { "La compagnie n'emploie pas de permanents." },
        permanents: p,
        montres: montres(ctx, nom, &donnees, &entreprise)?,
    })
}

/// Le curseur des permanents (`contexte::emploie_des_permanents`) de la fiche entreprise telle que l'ecran la saisit.
fn permanents_de(entreprise: &Saisie) -> bool {
    entreprise.case("emploie_des_permanents") == Some(&champs::Valeur::OuiNon(true))
}

/// CE QUE LA FICHE `nom` MONTRE pour `donnees` : chaque case que `champs::Champ::montre` dit a l'ecran, d'apres la
/// fiche, l'entreprise `entreprise` (ses permanents, ses conditions) et les etablissements de la compagnie. La page ne
/// montre que ces cases-la.
pub(crate) fn montres(ctx: &Contexte, nom: &str, donnees: &Saisie, entreprise: &Saisie) -> R<Vec<String>> {
    let permanents = permanents_de(entreprise);
    let etablissements = super::ouverture::choix_d_etablissement(ctx)?.is_some();
    let autre = (nom != "entreprise").then_some(entreprise);
    Ok(champs::fiche(nom)?.iter().filter(|c| c.montre(donnees, autre, permanents, etablissements)).map(|c| c.cle.clone()).collect())
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
    prestations_de_services: String,
    total: String,
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
    part_taxee: Option<String>,
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
    let bar = match (socle::date::Jour::nouveau(annee, 1, 1), droit::contexte::socle_livre_lu(annee).map_err(plain)?) {
        (Some(j), Some(_)) => Some(ctx.bareme(&j).map_err(plain)?),
        _ => None,
    };
    let q = documents::taxe_sur_les_salaires::question_franchise(bar.as_deref(), annee).map_err(|m| erreur(Classe::Illisible, m))?;
    // La conclusion nomme les deux plafonds.
    let r = documents::taxe_sur_les_salaires::regime_taxe_sur_les_salaires(&ent, q.plafonds);
    Ok(RegimeDeLaTaxe {
        phrase: r.consequence.clone(),
        franchise: Franchise {
            aide: q.aide,
            choix: ChoixDeLaFranchise { sous: q.sous, dessus: q.dessus },
            plafonds: q.plafonds.map(|(s, t)| PlafondsDeLaFranchise { prestations_de_services: s.normalize().to_string(), total: t.normalize().to_string() }),
            annee,
        },
        champs: r.champs,
        consequence: r.consequence,
        complet: r.decision.is_ok(),
        due: matches!(r.decision, Ok(Some(_))),
        part_taxee: match r.decision {
            Ok(Some(p)) => Some(p.normalize().to_string()),
            _ => None,
        },
    })
}

// ------------------------------------------------------------------ outils


// ------------------------------------------------------ l'avertissement d'une fiche

/// Ou l'import copie les pieces qu'il reprend de l'ancien logiciel, les DSN deposees comprises (`historique/bulletins/`).
pub(crate) fn pieces_importees(ctx: &Contexte) -> std::path::PathBuf {
    ctx.racine.join("historique").join("bulletins")
}

/// `usages_declares` : combien de DSN deja deposees declarent cette valeur dans la rubrique de la case (`rubrique`, de la
/// table des champs), telle qu'une DSN l'ecrit, et sur quels mois (`AAAA-MM`). Une case sans rubrique n'est dans aucune
/// DSN.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct DejaDeclare {
    pub(crate) declarations: usize,
    pub(crate) du: Option<String>,
    pub(crate) au: Option<String>,
}

fn usages_declares(ctx: &Contexte, rubrique: Option<&str>, valeur: Option<&str>) -> R<DejaDeclare> {
    let (mut n, mut mois) = (0usize, Vec::<String>::new());
    if let (Some(rubrique), Some(valeur)) = (rubrique, valeur) {
        for dossier in [pieces_importees(ctx), ctx.racine.join("sorties")] {
            for f in declarations::dsn_lue::fichiers(&dossier)? {
                let Some(d) = declarations::dsn_lue::lire_une_dsn(&f)? else { continue };
                if d.declare(rubrique, &valeur) {
                    // Un signalement declare la valeur sans mois principal (S20.G00.05.005).
                    n += 1;
                    mois.extend(d.mois.map(|m| m.to_string()));
                }
            }
        }
    }
    Ok(DejaDeclare { declarations: n, du: mois.iter().min().cloned(), au: mois.iter().max().cloned() })
}

/// Le chiffre (0 a 9) que `c` represente : les chiffres ASCII, et
/// ceux des ecritures les plus courantes (arabe, devanagari, pleine chasse).
pub(crate) fn chiffre_lu(c: char) -> Option<u32> {
    const DEBUTS: [u32; 4] = [0x0660, 0x06F0, 0x0966, 0xFF10];
    let u = c as u32;
    if c.is_ascii_digit() {
        return Some(u - 0x30);
    }
    DEBUTS.iter().find(|d| u >= **d && u < **d + 10).map(|d| u - d)
}


#[track_caller]
pub(crate) fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

#[track_caller]
pub(super) fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}

#[track_caller]
pub(crate) fn en_os(m: String) -> Echec {
    erreur(Classe::Disque, m)
}

pub(crate) fn creer_dossier(d: &Path) -> R<()> {
    std::fs::create_dir_all(d).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&d, &e)))
}

/// Une forme ecrite dans le fichier `chemin` (`socle::ecriture::ecrire_json`).
pub(crate) fn ecrire<T: serde::Serialize + ?Sized>(chemin: &Path, v: &T) -> R<()> {
    ecrire_json(chemin, v, 2).map_err(en_os)
}

/// La configuration, relue apres l'ecriture (`web._configuration(Contexte(racine))`).
/// Ce que la page lit de la compagnie (la route `etat`), relu apres une ecriture : la page n'a rien d'autre a relire.
pub(crate) fn etat_relu(ctx: &Contexte) -> R<crate::routes::ouverture::EtatDeLaPage> {
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(plain)?;
    crate::routes::ouverture::etat(&neuf)
}

/// L'heure de Paris, a la seconde.
pub(super) fn maintenant() -> String {
    let t = socle::temps::secondes();
    socle::paris::iso_secondes(t)
}

/// Aujourd'hui, a Paris (`date.today()`).
pub(crate) fn aujourd_hui() -> Jour {
    Jour::lire(&maintenant()[..10]).expect("date de Paris")
}

// --------------------------------------------------------------- activer-fiche

/// Seul `actif` change, a partir d'aujourd'hui (archiver, c'est ne plus proposer a partir de ce jour ; ce qui precede
/// reste ce qu'il etait), par l'ecrivain de la fiche (`poser`).
pub(crate) fn activer_fiche(ctx: &Contexte, nom: &str, cle: &str, actif: bool) -> R<()> {
    let jour = Some(ctx.aujourdhui);
    match nom {
        "salarie" => poser::<FicheSalarieDatee>(ctx, Some(cle), vec![ChangementSalarie::actif(Some(actif))], jour, None),
        "spectacle" => poser::<FicheSpectacleDatee>(ctx, Some(cle), vec![ChangementSpectacle::actif(Some(actif))], jour, None),
        autre => Err(refus(format!("Seuls un salarie ou un spectacle s'activent : {}.", socle::fichiers::repr(autre)))),
    }
}

fn activer_fiche_route(ctx: &Contexte, c: ChargeActiver) -> R<FicheActivee> {
    let (nom, cle, actif) = (c.nom.as_str(), c.cle.as_str(), c.actif);
    activer_fiche(ctx, nom, cle, actif)?;
    // Le nom que la liste montre : le salarie par son nom, le spectacle par son titre ; la cle seule si la fiche n'en a pas.
    let nomme = if nom == "salarie" {
        let id = ctx.cle_du_salarie(cle).map_err(|m| erreur(Classe::Manque, m))?;
        ctx.salarie(&id).map_err(|m| erreur(Classe::Manque, m))?.nomme()
    } else {
        let fiche = ctx.spectacles.get(cle).ok_or_else(|| refus(format!("Spectacle inconnu : {}.", socle::fichiers::repr(cle))))?;
        fiche.titre.portee().filter(|t| !t.trim().is_empty()).map(String::from)
    };
    let qui = match nomme {
        Some(n) => n,
        None => cle.to_string(),
    };
    let phrase = match (nom, actif) {
        ("salarie", true) => format!("{} : fiche réactivée.", qui),
        ("salarie", false) => format!("{} : fiche archivée.", qui),
        (_, true) => format!("Spectacle « {} » réactivé.", qui),
        (_, false) => format!("Spectacle « {} » archivé.", qui),
    };
    Ok(FicheActivee { phrase, etat: etat_relu(ctx)? })
}

// ----------------------------------------------------- fiche-oublier-demande

/// `fiche-oublier-demande` : `{"adresse"}`, la personne a qui la fiche a ete demandee ; ses demandes en attente sont
/// abandonnees.
fn oublier_demande(ctx: &Contexte, c: ChargeOublier) -> R<DemandeOubliee> {
    let adresse = strip(&c.adresse).to_lowercase();
    let codes: Vec<String> = fiches_attendues(ctx)?.into_iter().filter(|a| a.adresse == adresse).map(|a| a.code).collect();
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
            quand: maintenant(),
            qui: "interface".into(),
            de: None,
            message: None,
            reponse: "demande abandonnee, sans reponse".into(),
        };
        journal::rattacher(&ctx.racine, |l| demande_de(l, code), journal::Revenu::Fiche(retour)).map_err(plain)?;
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

use crate::routes::catalogue::{Ecran, Place};


/// LE DERNIER MOIS PARTI pour la fiche `nom` (`cle` pour un salarie) : le dernier dont les documents ou la DSN sont
/// partis (`engagement::departs`), ou que l'ancien logiciel a verse et declare (un engagement importe), pour ce salarie,
/// ou pour quiconque quand c'est la fiche de l'entreprise. `None` : aucun ; une fiche de spectacle n'en a pas.
pub(crate) fn dernier_mois_parti(ctx: &Contexte, nom: &str, cle: Option<&str>) -> R<Option<String>> {
    use engagement::departs::Envoi;
    if nom == "spectacle" {
        return Ok(None);
    }
    let sien = |c: &str| nom == "entreprise" || Some(c) == cle;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(plain)?;
    let partis = departs
        .engagements()
        .filter(|(c, _)| sien(c))
        .flat_map(|(c, n)| departs.de(c, n))
        .filter(|d| matches!(d.envoi, Envoi::Documents | Envoi::Dsn))
        .filter_map(|d| d.mois.clone());
    let mut importes = Vec::new();
    for i in engagement::importe::tous(&ctx.racine).map_err(plain)? {
        if i.objet().brut.saisie.carte.salarie.as_deref().is_some_and(sien) {
            importes.extend(i.paies().keys().cloned());
        }
    }
    Ok(partis.chain(importes).max())
}

/// LA DATE QU'UN CHANGEMENT DE FICHE PROPOSE, et pourquoi : le premier jour du mois qui suit le dernier mois parti
/// (`dernier_mois_parti`) : ce qui est parti garde ce qu'il a dit, le reste prend la valeur nouvelle. Aucun mois parti :
/// le premier jour du mois en cours. Une autre date se choisit, dans le passe aussi : ce qui est parti se corrige alors.
pub(crate) fn date_proposee(ctx: &Contexte, nom: &str, cle: Option<&str>) -> R<(Jour, String)> {
    let qui = if nom == "entreprise" { "" } else { " pour ce salarié" };
    Ok(match dernier_mois_parti(ctx, nom, cle)? {
        Some(m) => {
            let m = socle::mois::Mois::lire(&m).map_err(plain)?;
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
pub(crate) fn poser_taux_recu(ctx: &Contexte, salarie: &str, valeur: droit::fiche::TauxPas, depuis: Jour, note: &str) -> R<()> {
    ecriture::poser_au::<FicheSalarieDatee>(ctx, Some(salarie), vec![ChangementSalarie::taux_pas(Some(valeur))], true, Some(depuis), Some(note), &depuis)
}

/// LE TAUX AT/MP DU SIEGE S'ECRIT DANS LA FICHE ENTREPRISE (`taux_atmp`), a sa date d'effet (`ecriture::dater_au`) ;
/// `note` dit d'ou il vient.
pub(crate) fn dater_taux_atmp(ctx: &Contexte, valeur: droit::fiche::TauxAtmp, depuis: &Jour, note: &str) -> R<()> {
    ecriture::dater_au::<FicheEntrepriseDatee>(ctx, None, ChangementEntreprise::taux_atmp(Some(valeur)), depuis, note)
}

/// Une case de la fiche entreprise (`poser`).
pub(crate) fn poser_entreprise(ctx: &Contexte, case: ChangementEntreprise, depuis: Option<Jour>) -> R<()> {
    poser::<FicheEntrepriseDatee>(ctx, None, vec![case], depuis, None)
}

/// L'ANCIENNETE D'UN SALARIE PART DE SON EMBAUCHE (S21.G00.86.003, l'anciennete dans l'entreprise :
/// `norme-dsn/texte/bloc-S21.G00.86.txt`) : la validation de son premier engagement la pose sur sa fiche, au premier jour
/// de ce contrat (`debut`), quand la fiche ne la dit pas, par l'ecrivain de la fiche (`poser`) ; elle se change ensuite
/// dans « Avancé » de la fiche. Une fiche qui la dit la garde. INFERENCE : l'anciennete commence a l'entree dans
/// l'entreprise.
pub(crate) fn poser_anciennete_a_l_embauche(ctx: &Contexte, salarie: &str, debut: &str) -> R<()> {
    if ctx.salaries.get(salarie).is_none_or(|f| f.date_anciennete.est_dite()) {
        return Ok(());
    }
    let debut = Jour::lire(debut).map_err(plain)?;
    poser::<FicheSalarieDatee>(ctx, Some(salarie), vec![ChangementSalarie::date_anciennete(Some(debut))], None, None)
}

/// L'ANCIENNETE D'UN SALARIE REPRIS QUE SON CONTRAT FAIT REMONTER : l'import la pose au premier bulletin lu
/// (`import::bulletins`) ; un contrat repris qui a commence avant (`debut`, celui que la DSN de l'ancien logiciel
/// declare, S21.G00.40.001) dit qu'il etait dans l'entreprise des ce jour-la (S21.G00.86.003, l'anciennete dans
/// l'entreprise : `norme-dsn/texte/bloc-S21.G00.86.txt` ; inference : un contrat en cours dans l'entreprise y compte).
/// Seule la valeur que l'import a posee, `pose_le`, se remplace : l'anciennete est un fait du passe (`FAITS_SANS_DATE`),
/// elle se corrige. Une date saisie depuis reste la sienne. Rend vrai si elle a ete remplacee.
pub(crate) fn reprendre_anciennete(ctx: &Contexte, salarie: &str, pose_le: &str, debut: &str) -> R<bool> {
    let contenu = lire_fichier::<FicheSalarieDatee>(ctx)?;
    let Some([seule]) = contenu.fiches.get(salarie).and_then(|f| f.date_anciennete.as_deref()) else { return Ok(false) };
    if seule.valeur.dite().map(Jour::iso).as_deref() != Some(pose_le) || debut >= pose_le {
        return Ok(false);
    }
    // La note de l'import (d'ou vient la valeur) reste : la valeur vient toujours des pieces importees.
    let debut = Jour::lire(debut).map_err(plain)?;
    poser::<FicheSalarieDatee>(ctx, Some(salarie), vec![ChangementSalarie::date_anciennete(Some(debut))], None, seule.note.as_deref())?;
    Ok(true)
}



/// LES LICENCES QUI SUIVENT LA PREMIERE (`routes::import::poser_licence`) : chacune vaut a partir du premier jour du mois
/// ou les pieces importees la portent, comme un numero renouvele saisi « a partir du » (`poser`). Seulement quand la
/// fiche, sur le disque, porte pour seule licence la premiere que les pieces donnent : une licence saisie autrement reste
/// ce qu'elle est.
pub(crate) fn dater_licences_suivantes(ctx: &Contexte, licences: &[(String, String)], note: Option<&str>) -> R<()> {
    let Some(((_, premiere), suivantes)) = licences.split_first().filter(|(_, s)| !s.is_empty()) else { return Ok(()) };
    let fiche = lire_fichier::<FicheEntrepriseDatee>(ctx)?;
    if !matches!(fiche.licence_spectacle.as_deref(), Some([seule]) if seule.depuis.is_none() && seule.valeur.dite() == Some(premiere)) {
        return Ok(());
    }
    let note = note.filter(|n| !n.trim().is_empty());
    for (mois, numero) in suivantes {
        let jour = socle::mois::Mois::lire(mois).map(socle::mois::Mois::premier_jour).map_err(plain)?;
        poser_entreprise_note(ctx, ChangementEntreprise::licence_spectacle(Some(numero.clone())), jour, note)?;
    }
    Ok(())
}

/// Une case de la fiche entreprise, a partir de `jour`, avec sa note.
fn poser_entreprise_note(ctx: &Contexte, case: ChangementEntreprise, jour: Jour, note: Option<&str>) -> R<()> {
    poser::<FicheEntrepriseDatee>(ctx, None, vec![case], Some(jour), note)
}

// ------------------------------------------------------------------ la fiche demandee par courriel

/// Secondes entre une demande de fiche et la reponse de la personne fictive de la compagnie fictive (`DELAI_SIGNATURE`).
pub const DELAI_FICTIVE: i64 = 20;

/// Une adresse courriel de forme `x@y.z`, sans blanc ni second `@`.
fn adresse_valide(adresse: &str) -> bool {
    let blanc = socle::texte::blanc;
    if adresse.chars().any(blanc) || adresse.matches('@').count() != 1 {
        return false;
    }
    let Some((local, domaine)) = adresse.split_once('@') else { return false };
    let d: Vec<char> = domaine.chars().collect();
    !local.is_empty() && d.len() >= 3 && (1..d.len() - 1).any(|i| d[i] == '.')
}

/// Ce que `demander_fiche` rend : le code, l'adresse (en minuscules), la ligne du journal.
pub struct FicheDemandee {
    pub code: String,
    pub adresse: String,
    pub ligne: envois::journal::Ligne,
}

/// Envoie a `adresse` le courriel « Remplir ma fiche ». Le journal garde
/// le code : c'est par lui que la releve reconnaitra la reponse. `client` : la boite, ouverte ici (boite simulee dans une

/// La charge de `fiche-demander` : l'adresse, le prenom (pour lui ecrire), et le mot de passe du logiciel tape pour l'envoi.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeFicheDemander {
    adresse: String,
    prenom: String,
}

/// Ce que rend `fiche-demander`.
#[derive(Debug, serde::Serialize)]
struct DemandeDeFiche {
    phrase: String,
    code: String,
    adresse: String,
}

/// `fiche-demander` : le courriel « Remplir ma fiche » part (`demander_fiche`), sous la preuve que la charge porte.
fn fiche_demander(ctx: &Contexte, c: ChargeFicheDemander, motdepasse: Option<&str>) -> R<DemandeDeFiche> {
    let preuve = Preuve::du_mot_de_passe(motdepasse.map(str::to_string));
    let d = demander_fiche(ctx, &c.adresse, &c.prenom, &preuve, INTERFACE, None)?;
    Ok(DemandeDeFiche { phrase: format!("Demande de fiche envoyée à {} (code {}) : sa fiche se crée à l'arrivée de sa réponse.", d.adresse, d.code), code: d.code, adresse: d.adresse })
}
/// compagnie fictive, sinon refus « boite absente ») quand l'appelant n'en donne pas.
pub fn demander_fiche(ctx: &Contexte, adresse: &str, prenom: &str, preuve: &Preuve, qui: &str, client: Option<&mut dyn Courriel>) -> R<FicheDemandee> {
    let adresse = strip(adresse).to_lowercase();
    if !adresse_valide(&adresse) {
        return Err(refus(format!("« {} » n'est pas une adresse de courriel.", adresse)));
    }
    let mut ouverte: Option<Box<dyn Courriel>> = None;
    let client: &mut dyn Courriel = match client {
        Some(c) => c,
        None => ouverte.insert(client_courriel(ctx)?).as_mut(),
    };
    verifier_preuve(ctx, Some(preuve), &socle::paris::maintenant()[..7], "demande de fiche (« Remplir ma fiche »)", &adresse)?;

    let compagnie = ctx.entreprise.nommee().ok_or_else(|| socle::fichiers::absent("raison_sociale"))?.to_string();
    let sg = ctx.entreprise.signataire.dite().ok_or_else(|| socle::fichiers::absent("signataire"))?;
    let signataire = match sg.prenom_affichage.remplie() {
        Some(p) => p.to_string(),
        None => capitaliser(sg.prenom.portee().ok_or_else(|| socle::fichiers::absent("prenom"))?),
    };
    let expediteur = client.compte().to_lowercase();
    let code = fq::code_fiche().map_err(|m| erreur(Classe::Disque, m))?;
    let utiles = champs::utiles("salarie")?;
    let q = fq::questionnaire(&code, &fq::posees(&|c| utiles.iter().any(|u| u == c)), &|_| String::new());
    let sujet = envoi::formater(fq::SUJET, &[("compagnie", compagnie.clone())]);
    let prenom = strip(prenom).to_string();
    let virgule_prenom = if prenom.is_empty() { String::new() } else { format!(" {}", prenom) };
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
        cle: format!("fiche-{}", code),
        nom: if prenom.is_empty() { adresse.clone() } else { prenom.clone() },
        destinataire: adresse.clone(),
        sujet,
        corps: envoi::formater(fq::CORPS, &texte),
        html: envoi::formater(fq::HTML, &html),
        pieces: Vec::new(),
    };
    let mut identifiants: Vec<String> = Vec::new();
    let demande = |message: Option<String>, expediteur: Option<String>| journal::DetailFiche { code: code.clone(), adresse: adresse.clone(), prenom: prenom.clone(), message, expediteur };
    if let Err(e) = envoi::envoyer(&[message], &expediteur, client, Some(&mut identifiants), None) {
        noter(ctx, journal::Ligne::FicheDemande(par_courriel(None, qui, false, &adresse, &e, demande(None, None), None)))?;
        return Err(echec_d_envoi(e));
    }
    let detail = demande(identifiants.first().cloned(), Some(expediteur.clone()));
    let ligne = noter(ctx, journal::Ligne::FicheDemande(par_courriel(None, qui, true, &adresse, &format!("fiche demandee a {}", adresse), detail, None)))?;
    Ok(FicheDemandee { code, adresse, ligne })
}

// ------------------------------------------------------------------ les demandes attendues

/// Une demande partie et pas encore revenue.
#[derive(Debug, Clone)]
pub struct Attendue {
    pub code: String,
    pub adresse: String,
    pub prenom: String,
    /// L'heure de la demande (ISO), celle de la ligne du journal.
    pub quand: String,
    /// Les identifiants de NOS envois (le `Message-ID` gardé avec la demande) : jamais pris pour une reponse.
    pub envoyes: Vec<String>,
}

/// La ligne `l` est-elle la demande de fiche partie sous le code `code` ? Ce qui en revient se range sur elle.
fn demande_de(l: &journal::Ligne, code: &str) -> bool {
    l.fiche().is_some_and(|e| e.accepte && e.detail.code == code)
}

/// `fiches_attendues` : les demandes parties et pas encore revenues, les plus anciennes d'abord. Une demande a
/// laquelle on renonce se retire (`fiche-oublier-demande`, `fiches`).
pub fn fiches_attendues(ctx: &Contexte) -> R<Vec<Attendue>> {
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let mut sortie = Vec::new();
    for e in lignes.iter().filter_map(journal::Ligne::fiche) {
        if !e.accepte || e.detail.code.is_empty() || !e.retours.is_empty() {
            continue;
        }
        sortie.push(Attendue {
            code: e.detail.code.clone(),
            adresse: e.detail.adresse.clone(),
            prenom: e.detail.prenom.clone(),
            quand: e.quand.clone(),
            envoyes: e.detail.message.clone().filter(|m| !m.is_empty()).into_iter().collect(),
        });
    }
    Ok(sortie)
}

// ------------------------------------------------------------------ la releve

/// Ce que la releve a fait : combien de demandes attendaient, les fiches creees, ce qui l'a empechee.
#[derive(Debug, Default)]
pub struct ReleveFiches {
    pub attendues: usize,
    /// Chaque fiche creee ou completee.
    pub recues: Vec<FicheRecue>,
    pub incidents: Vec<String>,
}

/// Une fiche recue : le code de sa demande, sa cle, le nom de la personne, ce qui ne s'est pas lu, ses erreurs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FicheRecue {
    code: String,
    cle: Option<String>,
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
        format!("{} : {}", super::compte(self.recues.len(), "fiche de salarié reçue", "fiches de salarié reçues"), noms.join(", "))
    }
}

fn os(m: String) -> Echec {
    erreur(Classe::Disque, m)
}

/// Cherche les reponses aux demandes de fiche et cree chaque fiche.
///
/// `client` : la boite, ouverte ici quand l'appelant n'en donne pas -- la personne fictive de la compagnie fictive, ou la
/// vraie boite ; sans boite, un incident et rien d'autre. `qui` : `interface`, ou `veille`.
pub fn relever_fiches(ctx: &Contexte, client: Option<&mut dyn Courriel>, qui: &str) -> R<ReleveFiches> {
    let mut r = ReleveFiches::default();
    let attendues = fiches_attendues(ctx)?;
    r.attendues = attendues.len();
    if attendues.is_empty() {
        return Ok(r);
    }
    let mut ouverte: Option<Box<dyn Courriel>> = None;
    let branchement = crate::compagnie::bridage::branchement(&ctx.racine);
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
            if !crate::compagnie::boite::disponible(&ctx.racine) {
                r.incidents.push("Aucune boite courriel : les fiches demandees ne peuvent pas etre relevees.".to_string());
                return Ok(r);
            }
            let vrai_client = crate::compagnie::boite::client_du_contexte(ctx).map_err(|e| erreur(Classe::Courriel, e.0))?;
            ouverte.insert(Box::new(vrai_client)).as_mut()
        }
    };
    let expediteur = client.compte().to_lowercase();

    for a in &attendues {
        let code = &a.code;
        let identifiants = match client.rechercher(&format!("\"{}\"", code), 10) {
            Ok(i) => i,
            Err(e) => {
                r.incidents.push(format!("Recherche de {} : {}", code, e));
                break;
            }
        };
        let mut retenu: Option<(String, Vec<u8>, String, String, String)> = None;
        for ident in identifiants {
            if a.envoyes.contains(&ident) {
                continue;
            }
            let brut = match client.message_brut(&ident) {
                Ok(b) => b,
                Err(e) => {
                    r.incidents.push(format!("Lecture du message {} : {}", ident, e));
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
            if lecture::identifiant_de(&m).is_some_and(|i| a.envoyes.contains(&i)) {
                continue;
            }
            let texte = lecture::texte_propre(&m);
            if !texte.to_uppercase().contains(code.as_str()) || texte.contains("Remplir ma fiche") {
                continue;
            }
            let Some(de) = lecture::adresse_de(&m) else {
                r.incidents.push(format!("Fiche {} : une reponse ne dit pas d'ou elle vient ; elle n'est pas retenue.", code));
                continue;
            };
            if de == expediteur && de != a.adresse {
                continue;
            }
            if de != a.adresse {
                r.incidents.push(format!("Fiche {} : une reponse vient de {} et non de {} ; elle n'est pas retenue.", code, de, a.adresse));
                continue;
            }
            let quand = lecture::date_de(&m, &socle::paris::maintenant());
            retenu = Some((ident, brut.clone(), texte, de, quand));
            break;
        }
        let Some((ident, brut, texte, de, quand)) = retenu else { continue };
        let (lue, illisible) = fq::fiche_depuis_reponses(&fq::lire_reponses(&texte)).map_err(|m| erreur(Classe::Illisible, m))?;
        let mut recue: droit::fiche::ChampsSalarie = droit::fiche::lire_saisie(&lue, droit::contexte::SALARIES).map_err(|m| erreur(Classe::Illisible, m))?;
        recue.email = Some(de.clone());
        let nom = [recue.prenom_affichage.remplie(), recue.nom_affichage.remplie()].into_iter().flatten().collect::<Vec<_>>().join(" ");
        if nom.trim().is_empty() {
            r.incidents.push(format!("Fiche {} : la reponse de {} ne porte ni nom ni prenom lisible ; rien n'est cree.", code, de));
            continue;
        }
        let dossier = ctx.racine.join("sorties").join("fiches-salaries");
        std::fs::create_dir_all(&dossier).map_err(|e| os(socle::echec::du_disque(&dossier, &e)))?;
        let eml = dossier.join(format!("Fiche_{}.eml", code));
        std::fs::write(&eml, &brut).map_err(|e| os(socle::echec::du_disque(&eml, &e)))?;
        let jour: Vec<&str> = quand.get(..10).into_iter().flat_map(|j| j.split('-').rev()).collect();
        let mut note = format!("Fiche remplie par la personne, par courriel, le {}. Reste à choisir l'emploi.", jour.join("/"));
        if !illisible.is_empty() {
            note.push_str(&format!(" Non lu, à compléter : {}.", illisible.join(" ; ")));
        }
        let existante: Option<String> = ctx
            .salaries
            .iter()
            .find(|(_, s)| s.email.portee().is_some_and(|e| strip(e).to_lowercase() == de))
            .map(|(k, _)| k.to_string());
        // Une fiche qui existe deja : on ne complete que ses cases VIDES (ce qu'elle porte a peut-etre deja ete declare) ;
        // une fiche neuve prend toutes celles de la reponse, active. La note dit d'ou viennent les valeurs.
        let deja = match &existante {
            Some(k) => Some(vue::<FicheSalarieDatee>(&ctx.salaries[k.as_str()].champs)?),
            None => None,
        };
        let dites = droit::fiche::ChampsSalarie::default().cases_differentes(&recue);
        let p = preparer::<FicheSalarieDatee>(ctx, existante.as_deref(), |plate| {
            let mut nommees = Vec::new();
            for k in dites {
                let a_remplir = deja.as_ref().is_none_or(|v| vide(v.case(k)));
                if let (true, Some(ch)) = (a_remplir, <FicheSalarieDatee as droit::fiche::FicheDatee>::changement(&recue, k)) {
                    <FicheSalarieDatee as droit::fiche::FicheDatee>::appliquer(plate, ch);
                    nommees.push(k);
                }
            }
            if deja.is_none() {
                plate.actif = Some(true);
                nommees.push("actif");
            }
            nommees
        })?;
        // Les cases vides qu'elle remplit valent de tout temps ; ce qu'elle change d'une fiche qui existe se date comme un
        // changement de l'ecran (`date_proposee`).
        let depuis = match &existante {
            Some(k) => Some(date_proposee(ctx, "salarie", Some(k))?.0),
            None => None,
        };
        let f = ecrire_fiche::<FicheSalarieDatee>(ctx, p, Forcer::Tout, depuis, Some(&note))?;
        let erreurs = f.anomalies.iter().filter(|a| a.gravite == champs::Gravite::Erreur).count();
        let retour = journal::RetourFiche {
            code: code.clone(),
            abandon: false,
            salarie: f.cle.as_deref().map(droit::contexte::IdSalarie::lire).transpose().map_err(os)?,
            illisible: Some(illisible.clone()),
            erreurs: Some(erreurs),
            quand: quand.clone(),
            qui: qui.to_string(),
            de: Some(de.clone()),
            message: Some(ident.clone()),
            reponse: format!("fiche {} par courriel", if existante.is_some() { "completee" } else { "creee" }),
        };
        journal::rattacher(&ctx.racine, |l| demande_de(l, &code), journal::Revenu::Fiche(retour)).map_err(os)?;
        r.recues.push(FicheRecue { code: code.clone(), cle: f.cle, nom: strip(&nom).to_string(), illisible, erreurs });
    }
    client.fermer();
    Ok(r)
}

// ------------------------------------------------------------------ la compagnie fictive

/// Des personnes inventees, comme celles des compagnies fictives : (prenom, nom, sexe INSEE). Le NIR se
/// fabrique a la demande, cle comprise.
const PERSONNES: [(&str, &str, &str); 4] = [("Marie", "Dorval", "2"), ("Pierre", "Bocage", "1"), ("Rose", "Cheri", "2"), ("Frederick", "Lemaitre", "1")];

/// Le questionnaire que la personne fictive renvoie, rempli ligne a ligne dans les termes memes
/// des questions, le code en dernier. `bouton` : (objet, corps) du lien du courriel de la demande, quand on l'a.
fn reponse_de_fiche(demande: &Attendue, compte: &str, bouton: Option<&(String, String)>) -> Result<Vec<u8>, String> {
    let code = &demande.code;
    let somme: u32 = code.chars().map(|c| c as u32).sum();
    let (prenom, nom, sexe) = PERSONNES[somme as usize % PERSONNES.len()];
    let prenom = if demande.prenom.is_empty() { prenom.to_string() } else { demande.prenom.clone() };
    let an = 80 + code.chars().count() % 15;
    let mois = 1 + somme % 12;
    let treize = format!("{}{:02}{:02}75056{:03}", sexe, an, mois, 1 + somme % 900);
    let cle = 97 - treize.parse::<u64>().map_err(|e| e.to_string())? % 97;
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
    let utiles = champs::utiles("salarie").map_err(|e| e.message)?;
    // Une question que la personne fictive ne sait pas, elle la laisse sans reponse.
    let rempli = fq::questionnaire(code, &fq::posees(&|c| utiles.iter().any(|u| u == c)), &|cle| reponses.get(cle).into_iter().cloned().collect());
    let sujet = match bouton {
        Some(b) => b.0.clone(),
        None => "Re: fiche".to_string(),
    };
    envois::simulation_courriels::reponse_jouee(&demande.adresse, compte, &sujet, &rempli, &format!("fiche-{}", code))
}

/// La boite de la compagnie fictive, vue par la releve. Chaque demande de fiche partie
/// depuis `delai` secondes y trouve sa reponse remplie ; la vraie releve la lit et cree la fiche. Ce qu'on y envoie
/// s'ecrit comme tout courriel joue (`BoiteQuiRepond`, partagee avec `BoiteDesSignatures`).
pub fn boite_des_fiches(ctx: &Contexte, attendues: &[Attendue], delai: i64) -> R<BoiteQuiRepond> {
    let mut boite = BoiteQuiRepond::nouvelle(ctx)?;
    let maintenant_naif = socle::paris::maintenant_naif();
    // Les courriels joues, par leur `Message-ID` : la demande retrouve ainsi son bouton.
    let envoyes = envois::simulation_courriels::courriels_par_identifiant(&ctx.racine)?;
    for a in attendues {
        // Une demande sans heure lisible n'a pas d'age : la personne fictive n'y repond pas.
        let Some(partie) = socle::paris::naif_de_iso(&a.quand) else { continue };
        if maintenant_naif - partie < delai {
            continue;
        }
        let bouton = envois::simulation_courriels::bouton_de_la_demande(&envoyes, &a.envoyes)?;
        let brut = reponse_de_fiche(a, Courriel::compte(&boite), bouton.as_ref()).map_err(|m| erreur(Classe::Illisible, m))?;
        boite.repondre(&a.code, brut);
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
    annee: String,
    jusqu_a: String,
    #[serde(serialize_with = "en_objet")]
    spectacles: Vec<(String, CoutDit)>,
    #[serde(serialize_with = "en_objet")]
    salaries: Vec<(String, CoutDit)>,
}

/// Des paires (cle, cout) ecrites en objet, dans leur ordre.
fn en_objet<S: serde::Serializer>(v: &[(String, CoutDit)], s: S) -> Result<S::Ok, S::Error> {
    s.collect_map(v.iter().map(|(k, c)| (k, c)))
}

dites!(EffectifDit, CoutsDits, DemandeOubliee);

/// Une somme que le cout ecrit (en texte, `Cout`), en euros.
fn euros_ecrits(somme: &str) -> R<String> {
    match socle::num::decimal_lu(somme.trim()) {
        Some(d) => Ok(socle::texte::euros(d)),
        None => Err(erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : une somme du coût de l'année ne se lit pas ({}).", somme))),
    }
}

/// `couts` : le cout de l'annee, au jour du contexte (`documents::fiche_recapitulative::Recapitulatifs::couts_de_l_annee`).
fn couts(ctx: &Contexte, c: ChargeCouts) -> R<CoutsDits> {
    let mut r = documents::fiche_recapitulative::Recapitulatifs::nouveau(ctx, None, Some(c.annee.to_string()), false, envois::journal::lu(&ctx.racine).map(|j| j.departs)?);
    let couts = r.couts_de_l_annee().map_err(|e| Echec { classe: e.classe, message: e.message })?;
    let bulletins = |x: &documents::fiche_recapitulative::Cout| super::compte(x.bulletins as usize, "bulletin", "bulletins");
    // La ligne de chaque spectacle et de chaque salarie, telle que sa fiche la montre.
    let dit = |liste: Vec<(String, documents::fiche_recapitulative::Cout)>| -> R<Vec<(String, CoutDit)>> {
        liste
            .into_iter()
            .map(|(k, x)| {
                let prevu = match socle::num::decimal_lu(x.prevu.trim()) {
                    Some(p) if !p.is_zero() => format!(" · + {} déjà saisis pour la suite de l'année", socle::texte::euros(p)),
                    Some(_) => String::new(),
                    None => return Err(erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : une somme du coût de l'année ne se lit pas ({}).", x.prevu))),
                };
                let phrase = format!(
                    "coût employeur {} — brut {}, charges {}, {}{}{}",
                    euros_ecrits(&x.cout)?, euros_ecrits(&x.brut)?, euros_ecrits(&x.charges)?, bulletins(&x),
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
        total += socle::num::decimal_lu(x.cout.trim()).ok_or_else(|| erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : une somme du coût de l'année ne se lit pas ({}).", x.cout)))?;
        phrases.push(format!("{} : {} ({}, brut {})", nom, euros_ecrits(&x.cout)?, bulletins(x), euros_ecrits(&x.brut)?));
    }
    let phrase = format!(
        "Coût de {}, jusqu'à {} : {} pour {}.",
        couts.annee,
        socle::mois::Mois::en_clair(&couts.jusqu_a),
        socle::texte::euros(total),
        super::compte(couts.spectacles.len(), "spectacle", "spectacles")
    );
    Ok(CoutsDits { phrase, phrases, annee: couts.annee, jusqu_a: couts.jusqu_a, spectacles: dit(couts.spectacles)?, salaries: dit(couts.salaries)? })
}

/// `poser-courriel` : le courriel `email` du salarie `cle`, saisi la ou il manque (la boite des contrats a
/// faire signer). La fiche entiere s'ecrit comme depuis l'ecran (`ecrire_fiche`), seul le courriel y change : il se
/// verifie avec elle, et une fiche qui refuse le dit.
pub(crate) fn poser_courriel(ctx: &Contexte, cle: &str, email: &str) -> R<()> {
    if !ctx.salaries.contains_key(cle) {
        return Err(refus(format!("Salarié {} inconnu.", socle::fichiers::repr(cle))));
    }
    let p = preparer::<FicheSalarieDatee>(ctx, Some(cle), |plate| {
        plate.email = Some(email.trim().to_string());
        vec!["email"]
    })?;
    let (depuis, _) = date_proposee(ctx, "salarie", Some(cle))?;
    ecrire_fiche::<FicheSalarieDatee>(ctx, p, Forcer::Rien, Some(depuis), None)?;
    Ok(())
}
