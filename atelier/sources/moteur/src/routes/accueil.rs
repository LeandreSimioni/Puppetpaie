//! L'ACCUEIL DES COMPAGNIES : les routes de l'installation (le dossier qui porte `logiciel/`, `prive/`, `compagnies/`,
//! `fictives/` : `interface/compagnies.rs`), et non d'une compagnie. Les lister, en ajouter une, choisir sa couleur, remettre
//! une fictive livree a zero, garder au deblocage les compagnies creees en demo, completer un dossier portable des systemes
//! qui lui manquent (`maj::completer_le_portable`) ; et ce que l'administrateur fait sur
//! toutes ses compagnies a la fois (son tableau, son depot groupe).
//!
//! L'INSTALLATION D'UNE ROUTE : celle de la compagnie nommee (`--racine`, `administrateur::installation_de`), sinon celle
//! du programme (`compagnies::base`, a cote de `logiciel/`) : `moteur route` sans `--racine` sert l'installation ou il est
//! pose. La fenetre sert ces routes a l'accueil, avant toute compagnie ouverte (`interface::api::route_de_l_installation`).
//!
//! Ouvrir une compagnie dans la fenetre (sa tuile, son mot de passe) n'est pas une route : la route rend son `chemin`, la
//! fenetre l'ouvre ; en ligne de commande, on la nomme par `--racine`. Le bridage (la demo) est un reglage de l'accueil :
//! il n'a pas de route, son geste n'existe qu'a l'ecran.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::compagnie::administrateur;
use crate::interface::{compagnies, suppression};
use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, Place, Refus, MANQUE, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::enveloppe::{en_json, Charge, Dite, Rendu};
use socle::echec::{erreur, Classe, Echec, R};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Accueil);

const COMPAGNIE: Champ = Champ { nom: "compagnie", libelle: Affiche::new("Compagnie"), forme: Forme::Texte, sens: "l'identifiant de la compagnie, tel que `compagnies` le rend", avance: false };

/// Les routes de l'installation, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Compagnies,
        geste: "Dire les compagnies de cette installation, et où est chacune.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "compagnies (id, nom, lieu, chemin — le --racine de ses routes —, reelle, remise_a_zero, couleur), bride, a_garder (au déblocage), phrase, phrases (une par compagnie), auteur (phrase, nom, adresse)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: false,
        phrase: 80,
        ecran: Ecran::Lecture,
    },
    Fiche {
        route: Route::CompagnieAjouter,
        geste: "Ajouter une compagnie : neuve, ou venant d'un logiciel précédent.",
        charge: &[
            Champ { nom: "nom", libelle: Affiche::new("Nom de la compagnie"), forme: Forme::Texte, sens: "le nom de la compagnie", avance: false },
            Champ { nom: "venue", libelle: Affiche::new("Venue"), forme: Forme::Texte, sens: "neuve, ou logiciel_precedent (elle commence par reprendre son passé)", avance: false },
        ],
        exemple: r#"{"nom":"Les Tréteaux d'essai","venue":"neuve"}"#,
        rend: "id, nom, chemin (le --racine de ses routes), phrase ; logiciel bridé : une compagnie fictive ajoutée",
        refus: &[MANQUE, OPERATION],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 120,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::CompagnieCouleur,
        geste: "Choisir la couleur d'une compagnie.",
        charge: &[COMPAGNIE, Champ { nom: "couleur", libelle: Affiche::new("Couleur"), forme: Forme::Texte, sens: "#RRGGBB", avance: false }],
        exemple: r##"{"compagnie":"cie:la-compagnie","couleur":"#2E5A7D"}"##,
        rend: "couleur, phrase",
        refus: &[MANQUE, Refus { classe: Classe::Refuse, corriger: "une couleur #RRGGBB, une compagnie réelle ; rien n'est écrit" }],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 80,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::CompagnieRemettreAZero,
        geste: "Remettre une compagnie fictive livrée dans son état d'origine.",
        charge: &[COMPAGNIE],
        exemple: r#"{"compagnie":"fictive:fictive-essai"}"#,
        rend: "phrase",
        refus: &[MANQUE, Refus { classe: Classe::Refuse, corriger: "une compagnie fictive livrée ; rien n'est effacé" }],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 120,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::CompagniesGarder,
        geste: "Au déblocage : garder les compagnies créées en démo, ou les supprimer.",
        charge: &[
            Champ { nom: "garder", libelle: Affiche::new("À garder"), forme: Forme::Liste, sens: "les identifiants des compagnies à garder (elles deviennent réelles)", avance: false },
            Champ { nom: "ne_pas_garder", libelle: Affiche::new("À supprimer"), forme: Forme::Liste, sens: "les identifiants des compagnies à supprimer", avance: false },
        ],
        exemple: r#"{"garder":["fictive:ajoutee-demo"],"ne_pas_garder":[]}"#,
        rend: "gardees, supprimees, erreurs, phrase",
        refus: &[Refus { classe: Classe::Refuse, corriger: "une compagnie créée en démo, d'un logiciel débloqué ; rien n'a changé" }],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 200,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::AdministrateurTableau,
        geste: "Dire les déclarations à faire de chaque compagnie de l'administrateur.",
        charge: &[
            Champ { nom: "garde", libelle: Affiche::new("Tableau gardé"), forme: Forme::OuiNon, sens: "vrai : le tableau gardé, sans calcul", avance: false },
            Champ { nom: "relever", libelle: Affiche::new("Relever d'abord les comptes rendus"), forme: Forme::OuiNon, sens: "vrai : relever d'abord les comptes rendus attendus", avance: false },
        ],
        exemple: r#"{}"#,
        rend: "compagnies (chacune ses déclarations triées par urgence, et où en est leur fichier), groupe (ce que le dépôt groupé propose)",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::AdministrateurDeposer,
        geste: "Déposer d'un geste ce qui est prêt dans chaque compagnie de l'administrateur.",
        charge: &[
            Champ { nom: "reel", libelle: Affiche::new("En réel"), forme: Forme::OuiNon, sens: "vrai : en réel (les fichiers testés conformes) ; en test sinon", avance: false },
            Champ { nom: "motdepasse", libelle: Affiche::new("Mot de passe de l'administrateur"), forme: Forme::Texte, sens: "le mot de passe de l'administrateur ; sans lui, le moteur le demande à un humain", avance: false },
        ],
        exemple: r#"{"reel":false}"#,
        rend: "partis, refuses, resultats (par dépôt : la compagnie, ce qui est parti ou le refus), tableau ; rien de prêt : phrase",
        refus: &[Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe, tapé par un humain ; rien n'est déposé" }, OPERATION],
        acte: Acte::Engage,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::PortableCompleter,
        geste: "Ajouter au dossier les systèmes qui lui manquent (Windows, Linux, macOS).",
        charge: &[Champ { nom: "portable", libelle: Affiche::new("Rendre ce dossier portable"), forme: Forme::OuiNon, sens: "vrai : faire aussi de ce dossier un dossier portable, pour une clé USB (PORTABLE.json)", avance: false }],
        exemple: r#"{"portable":true}"#,
        rend: "ajoutes, absents (les systèmes que la version n'a pas), version, portable, phrase",
        refus: &[Refus { classe: Classe::Refuse, corriger: "le réseau, la mise à jour en attente, ou ce que la phrase dit ; rien n'est posé" }],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 140,
        ecran: Ecran::Auto(Place::Accueil, Affiche::new("Ajouter les systèmes qui manquent")),
    },
];

/// L'installation de la route : celle de la compagnie nommee (`--racine`), sinon celle du programme.
pub fn installation(o: &HashMap<String, String>) -> PathBuf {
    match o.get("racine").filter(|r| !r.is_empty()) {
        Some(r) => administrateur::installation_de(std::path::Path::new(r)),
        None => compagnies::base(),
    }
}

fn operation(m: String) -> Echec {
    erreur(Classe::Refuse, m)
}

/// La charge de `compagnie-ajouter` : son nom, et d'ou elle vient.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeAjouter {
    nom: String,
    venue: compagnies::Venue,
}

/// La charge de `compagnie-couleur`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeCouleur {
    compagnie: String,
    couleur: String,
}

/// La charge qui nomme une compagnie de l'accueil.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeCompagnie {
    compagnie: String,
}

/// La charge de `compagnies-garder` : les compagnies a garder, et celles a supprimer.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeGarder {
    garder: Vec<String>,
    ne_pas_garder: Vec<String>,
}

/// La charge de `administrateur-tableau` : le tableau garde, ou refait (apres un releve, s'il est demande).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeTableau {
    garde: Option<bool>,
    relever: Option<bool>,
}

/// La charge de `administrateur-deposer` : en reel, ou en test (le mot de passe est a l'enveloppe).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeDeposer {
    reel: bool,
}

/// La charge de `portable-completer`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargePortable {
    portable: Option<bool>,
}

/// Une compagnie de l'accueil, telle qu'une tuile la montre.
#[derive(Debug, Serialize)]
struct Tuile {
    id: String,
    nom: String,
    lieu: String,
    chemin: String,
    reelle: bool,
    remise_a_zero: bool,
    couleur: String,
    logo: Option<String>,
}

/// Ce que rend `compagnies`.
#[derive(Debug, Serialize)]
struct Liste {
    phrase: String,
    compagnies: Vec<Tuile>,
    bride: bool,
    a_garder: Vec<suppression::Gardee>,
    phrases: Vec<String>,
    auteur: crate::interface::auteur::Auteur,
}

/// Ce que rend `compagnie-ajouter` : la compagnie, et le chemin que la fenetre ouvre.
#[derive(Debug, Serialize)]
struct Ajoutee {
    phrase: String,
    id: String,
    nom: String,
    chemin: String,
}

/// Ce que rend `compagnie-couleur`.
#[derive(Debug, Serialize)]
struct Couleur {
    phrase: String,
    couleur: String,
}

/// Ce que rend une route qui n'a que sa phrase.
#[derive(Debug, Serialize)]
struct Dit {
    phrase: String,
}

/// Ce que rend `compagnies-garder`.
#[derive(Debug, Serialize)]
struct Gardees {
    phrase: String,
    #[serde(flatten)]
    reponse: suppression::Reponse,
}

impl Dite for Liste {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for Ajoutee {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for Couleur {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for Dit {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for Gardees {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// Une route de l'installation, branchee sur l'enveloppe. Le tableau de l'administrateur, son depot groupe et le dossier
/// portable rendent ce que `compagnie::administrateur` et `compagnie::maj` ecrivent.
pub fn route(o: &HashMap<String, String>, nom: Route, charge: &Charge) -> R<Rendu> {
    // `--racine` nomme une compagnie (son dossier porte sa fiche entreprise) : l'installation est la sienne. Un autre
    // dossier n'en est pas une, et son parent n'est pas une installation : rien ne s'y ecrit.
    if let Some(r) = o.get("racine").filter(|r| !r.is_empty()) {
        if !std::path::Path::new(r).join(droit::contexte::ENTREPRISE).is_file() {
            return Err(operation(socle::echec::avec(
                "Ce dossier n'est pas celui d'une compagnie. Les routes de l'installation se servent sans lui. Rien n'est écrit.",
                socle::echec::APart {
                    chemin: Some(r.clone()),
                    detail: Some("--racine nomme une compagnie (le chemin que rend la route compagnies) ; les routes de l'installation se servent sans --racine.".to_string()),
                    ..socle::echec::APart::default()
                },
            )));
        }
    }
    let inst = installation(o);
    match nom {
        Route::Compagnies => {
            let super::SansCharge {} = charge.lire()?;
            let compagnies: Liste = liste(&inst);
            en_json(compagnies)
        }
        Route::CompagnieAjouter => {
            let a: ChargeAjouter = charge.lire()?;
            let c = compagnies::ajouter_venant(&inst, &a.nom, a.venue).map_err(operation)?;
            let phrase = if c.reelle {
                format!("{} est ajoutée.", c.nom)
            } else {
                format!("{} est ajoutée, en compagnie fictive : rien n'en part.", c.nom)
            };
            en_json(Ajoutee { phrase, id: c.id, nom: c.nom, chemin: c.racine.to_string_lossy().into_owned() })
        }
        Route::CompagnieCouleur => {
            let c: ChargeCouleur = charge.lire()?;
            let couleur = compagnies::poser_couleur(&inst, &c.compagnie, &c.couleur).map_err(operation)?;
            en_json(Couleur { phrase: format!("Couleur posée : {}.", couleur), couleur })
        }
        Route::CompagnieRemettreAZero => {
            let c: ChargeCompagnie = charge.lire()?;
            let (n, deja) = compagnies::remettre_a_zero(&inst, &c.compagnie).map_err(operation)?;
            en_json(Dit { phrase: format!("{} est {}.", n, if deja { "déjà dans son état d'origine" } else { "revenue à son état d'origine" }) })
        }
        Route::CompagniesGarder => {
            let c: ChargeGarder = charge.lire()?;
            let poste = administrateur::Poste::de_ce_poste();
            let reponse = suppression::repondre(&inst, &c.garder, &c.ne_pas_garder, &poste, &|_| {}).map_err(operation)?;
            let mut phrase = format!("{} compagnie(s) gardée(s), {} supprimée(s).", reponse.gardees.len(), reponse.supprimees.len());
            for e in &reponse.erreurs {
                phrase.push(' ');
                phrase.push_str(e);
            }
            en_json(Gardees { phrase, reponse })
        }
        Route::AdministrateurTableau => {
            let c: ChargeTableau = charge.lire()?;
            let tableau = match c.garde {
                Some(true) => administrateur::tableau_garde(&inst)?,
                Some(false) | None => administrateur::tableau(o, &inst, c.relever == Some(true))?,
            };
            en_json(tableau)
        }
        Route::PortableCompleter => {
            let c: ChargePortable = charge.lire()?;
            let cles = crate::compagnie::maj::cles_de_l_editeur();
            crate::compagnie::maj::completer_le_portable(&inst, c.portable == Some(true), &cles).map_err(operation).map(Rendu::brut)
        }
        Route::AdministrateurDeposer => {
            let c: ChargeDeposer = charge.lire()?;
            en_json(administrateur::deposer(o, &inst, c.reel, charge.mot_de_passe().map(String::from))?)
        }
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

/// Les compagnies de l'installation, dans l'ordre des tuiles de l'accueil, et ce que l'accueil en dit.
fn liste(inst: &std::path::Path) -> Liste {
    let cies: Vec<Tuile> = compagnies::liste_de(inst)
        .into_iter()
        .map(|c| Tuile {
            chemin: c.racine.to_string_lossy().into_owned(),
            remise_a_zero: !c.reelle && compagnies::est_fictive_livree(&c.racine),
            logo: compagnies::logo(&c.racine).map(|p| p.to_string_lossy().into_owned()),
            id: c.id,
            nom: c.nom,
            lieu: c.lieu,
            reelle: c.reelle,
            couleur: c.couleur,
        })
        .collect();
    let a_garder: Vec<suppression::Gardee> = suppression::a_garder(inst).into_iter().map(|c| suppression::Gardee { id: c.id, nom: c.nom }).collect();
    let reelles = cies.iter().filter(|c| c.reelle).count();
    let phrase = format!("{} compagnie(s), dont {} fictive(s).", cies.len(), cies.len() - reelles);
    let phrases: Vec<String> = cies.iter().map(|c| format!("{}{}", c.nom, if c.reelle { "" } else { ", fictive" })).collect();
    Liste { phrase, compagnies: cies, bride: crate::compagnie::bridage::est_bride_de(inst), a_garder, phrases, auteur: crate::interface::auteur::auteur() }
}
