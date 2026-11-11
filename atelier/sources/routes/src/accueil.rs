//! L'ACCUEIL DES COMPAGNIES : les routes de l'installation (le dossier qui porte `logiciel/`, `prive/`, `compagnies/`,
//! `fictives/` : `compagnie/installation.rs`), et non d'une compagnie. Les lister, en ajouter une, choisir sa couleur, remettre
//! une fictive livree a zero, garder au deblocage les compagnies creees en demo, completer un dossier portable des systemes
//! qui lui manquent (`maj::completer_le_portable`) ; et ce que l'administrateur fait sur
//! toutes ses compagnies a la fois (son tableau, son depot groupe).
//!
//! L'INSTALLATION D'UNE ROUTE : celle de la compagnie nommee (`--racine`, `administrateur::installation_de`), sinon celle
//! du programme (`compagnies::base`, a cote de `logiciel/`) : `moteur route` sans `--racine` sert l'installation ou il est
//! pose. La fenetre sert ces routes a l'accueil, avant toute compagnie ouverte (`interface::api::route_de_l_installation`).
//!
//! Ouvrir une compagnie dans la fenetre (sa tuile, son mot de passe) n'est pas une route : la route rend son `chemin`, la
//! fenetre l'ouvre ; en ligne de commande, on la nomme par `--racine`. Le bridage (la demo) est un reglage de
//! l'installation : sa route est `basculer`, que le geste secret de l'accueil appelle.
//!
//! CE QUE L'ACCUEIL FAIT DES COMPAGNIES : en ajouter une (le « + », `ajouter_venant`), en supprimer une, garder au
//! deblocage celles qu'on a ajoutees en demo (`suppression`). Ce qu'est une compagnie sur le poste (sa racine, son
//! marqueur, sa tuile) : `compagnie::installation`. Le dossier portable complete (`compagnie::maj::PortableComplete`, la
//! route `portable-completer`) se dit ici (`Dite`).

pub mod suppression;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use compagnie::installation as compagnies;
use compagnie::installation::{entree, nom_de_dossier, nom_dossier, Compagnie, Creation, Marqueur, Venue, DOSSIER_FICTIVES, FICHIER_CREATION, MARQUEUR, MARQUE_FICTIVE, PREFIXE_AJOUTEE};
use crate::gestes::administrateur;
use crate::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, Place, Refus, MANQUE, OPERATION};
use crate::catalogue::Route;
use crate::enveloppe::{en_json, Charge, Dite, Options, Rendu};
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
        phrase: 80,
        ecran: Ecran::DessineAvance(Place::Accueil),
    },
    Fiche {
        route: Route::CompagnieRemettreAZero,
        geste: "Remettre une compagnie fictive livrée dans son état d'origine.",
        charge: &[COMPAGNIE],
        exemple: r#"{"compagnie":"fictive:fictive-essai"}"#,
        rend: "phrase",
        refus: &[MANQUE, Refus { classe: Classe::Refuse, corriger: "une compagnie fictive livrée ; rien n'est effacé" }],
        acte: Acte::Ecrit,
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
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Accueil),
    },
    Fiche {
        route: Route::AdministrateurDeposer,
        geste: "Déposer d'un geste ce qui est prêt dans chaque compagnie de l'administrateur.",
        charge: &[
            Champ { nom: "reel", libelle: Affiche::new("En réel"), forme: Forme::OuiNon, sens: "vrai : en réel (les fichiers testés conformes) ; en test sinon", avance: false },
            Champ { nom: "motdepasse", libelle: Affiche::new("Mot de passe de l'administrateur"), forme: Forme::Texte, sens: "le mot de passe de l'administrateur ; sans lui, le moteur le demande à un humain", avance: false },
        ],
        exemple: r#"{"reel":false}"#,
        rend: "partis, refuses, resultats (par dépôt : la compagnie, ce qui est parti ou le refus, et suivre pour un dépôt réel), tableau ; rien de prêt : phrase",
        refus: &[Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe, tapé par un humain ; rien n'est déposé" }, OPERATION],
        acte: Acte::Engage,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Accueil),
    },
    Fiche {
        route: Route::AdministrateurSuivre,
        geste: "Après un dépôt groupé réel, relever les retours de chaque déclaration partie.",
        charge: &[Champ {
            nom: "suivis",
            libelle: Affiche::new("Déclarations à suivre"),
            forme: Forme::Objet,
            sens: "la liste des « suivre » des résultats partis en réel, tels qu'administrateur-deposer les rend",
            avance: false,
        }],
        exemple: r#"{"suivis":[]}"#,
        rend: "fini (toutes tranchées), suivies (par déclaration : la compagnie, passée ou refusée, la phrase et sa couleur), phrase",
        refus: &[OPERATION],
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Accueil),
    },
    Fiche {
        route: Route::PortableCompleter,
        geste: "Ajouter au dossier les systèmes qui lui manquent (Windows, Linux, macOS).",
        charge: &[Champ { nom: "portable", libelle: Affiche::new("Rendre ce dossier portable"), forme: Forme::OuiNon, sens: "vrai : faire aussi de ce dossier un dossier portable, pour une clé USB (PORTABLE.json)", avance: false }],
        exemple: r#"{"portable":true}"#,
        rend: "ajoutes, absents (les systèmes que la version n'a pas), version, portable, phrase",
        refus: &[Refus { classe: Classe::Refuse, corriger: "le réseau, la mise à jour en attente, ou ce que la phrase dit ; rien n'est posé" }],
        acte: Acte::Ecrit,
        phrase: 140,
        ecran: Ecran::Auto(Place::Accueil, Affiche::new("Ajouter les systèmes qui manquent")),
    },
    Fiche {
        route: Route::Basculer,
        geste: "Brider le logiciel (la démo) ou le débloquer, sur ce poste.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "bride (vrai : la démo), phrase ; un paquet soudé ne change pas",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        phrase: 80,
        ecran: ECRAN,
    },
];

/// L'installation de la route : celle de la compagnie nommee (`--racine`), sinon celle du programme.
pub fn installation(o: &Options) -> PathBuf {
    match &o.racine {
        Some(r) => administrateur::installation_de(r),
        None => compagnies::base(),
    }
}

/// La charge de `compagnie-ajouter` : son nom, et d'ou elle vient.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeAjouter {
    pub nom: String,
    pub venue: compagnies::Venue,
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
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeTableau {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub garde: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relever: Option<bool>,
}

/// La charge de `administrateur-deposer` : en reel, ou en test (le mot de passe est a l'enveloppe).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeDeposer {
    reel: bool,
}

/// La charge de `administrateur-suivre` : les declarations a suivre, telles que le depot groupe les a rendues
/// (`suivre` de chaque resultat parti en reel).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeSuivre {
    suivis: Vec<administrateur::ASuivre>,
}

/// La charge de `portable-completer`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargePortable {
    portable: Option<bool>,
}

/// Une compagnie de l'accueil, telle qu'une tuile la montre.
#[derive(Debug, Serialize)]
pub struct Tuile {
    pub id: String,
    pub nom: String,
    pub lieu: String,
    chemin: String,
    pub reelle: bool,
    pub remise_a_zero: bool,
    pub couleur: String,
    pub logo: Option<String>,
}

/// Ce que rend `compagnies`, et ce que l'accueil de la fenetre dessine (`interface::page_accueil`).
#[derive(Debug, Serialize)]
pub struct Liste {
    phrase: String,
    pub compagnies: Vec<Tuile>,
    pub bride: bool,
    pub a_garder: Vec<suppression::Gardee>,
    phrases: Vec<String>,
    pub auteur: compagnie::auteur::Auteur,
}

/// Ce que rend `compagnie-ajouter` : la compagnie, et le chemin que la fenetre ouvre.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ajoutee {
    phrase: String,
    id: String,
    nom: String,
    pub chemin: String,
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

/// Ce que rend `basculer` : l'etat du logiciel sur ce poste, et sa phrase.
#[derive(Debug, Serialize)]
struct Bascule {
    bride: bool,
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

impl Dite for Bascule {
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
/// portable rendent ce que `gestes::administrateur` et `compagnie::maj` ecrivent.
pub fn route(o: &Options, nom: Route, charge: &Charge) -> R<Rendu> {
    // `--racine` nomme une compagnie (son dossier porte sa fiche entreprise) : l'installation est la sienne. Un autre
    // dossier n'en est pas une, et son parent n'est pas une installation : rien ne s'y ecrit.
    if let Some(r) = &o.racine {
        if !r.join(droit::contexte::ENTREPRISE).is_file() {
            let lieu = socle::echec::Lieu {
                chemin: Some(r.to_string_lossy().into_owned()),
                detail: Some("--racine nomme une compagnie (le chemin que rend la route compagnies) ; les routes de l'installation se servent sans --racine.".to_string()),
                ..socle::echec::Lieu::default()
            };
            return Err(Echec {
                a_part: Some(lieu.into()),
                ..erreur(Classe::Refuse, "Ce dossier n'est pas celui d'une compagnie. Les routes de l'installation se servent sans lui. Rien n'est écrit.")
            });
        }
    }
    let inst = installation(o);
    match nom {
        Route::Compagnies => {
            let super::SansCharge {} = charge.lire()?;
            let compagnies: Liste = liste(&inst).map_err(|e| e.reclasser(Classe::Refuse))?;
            en_json(compagnies)
        }
        Route::CompagnieAjouter => {
            let a: ChargeAjouter = charge.lire()?;
            let c = ajouter_venant(&inst, &a.nom, a.venue).map_err(|e| e.reclasser(Classe::Refuse))?;
            let phrase = if c.reelle {
                format!("{} est ajoutée.", c.nom)
            } else {
                format!("{} est ajoutée, en compagnie fictive : rien n'en part.", c.nom)
            };
            en_json(Ajoutee { phrase, id: c.id, nom: c.nom, chemin: c.racine.to_string_lossy().into_owned() })
        }
        Route::CompagnieCouleur => {
            let c: ChargeCouleur = charge.lire()?;
            let couleur = compagnies::poser_couleur(&inst, &c.compagnie, &c.couleur).map_err(|e| e.reclasser(Classe::Refuse))?;
            en_json(Couleur { phrase: format!("Couleur posée : {}.", couleur), couleur })
        }
        Route::CompagnieRemettreAZero => {
            let c: ChargeCompagnie = charge.lire()?;
            let (n, deja) = compagnies::remettre_a_zero(&inst, &c.compagnie).map_err(|e| e.reclasser(Classe::Refuse))?;
            en_json(Dit { phrase: format!("{} est {}.", n, if deja { "déjà dans son état d'origine" } else { "revenue à son état d'origine" }) })
        }
        Route::CompagniesGarder => {
            let c: ChargeGarder = charge.lire()?;
            let poste = administrateur::Poste::de_ce_poste();
            let reponse = suppression::repondre(&inst, &c.garder, &c.ne_pas_garder, &poste, &|_| {}).map_err(|e| e.reclasser(Classe::Refuse))?;
            let mut phrase = format!("{} compagnie(s) gardée(s), {} supprimée(s).", reponse.gardees.len(), reponse.supprimees.len());
            for e in &reponse.erreurs {
                phrase.push(' ');
                phrase.push_str(e);
            }
            en_json(Gardees { phrase, reponse })
        }
        Route::AdministrateurTableau => {
            let c: ChargeTableau = charge.lire()?;
            let tableau: administrateur::TableauAdministrateur = match c.garde {
                Some(true) => administrateur::tableau_garde(&inst)?,
                Some(false) | None => administrateur::tableau(o, &inst, c.relever == Some(true))?,
            };
            en_json(tableau)
        }
        Route::Basculer => {
            compagnie::bridage::basculer_de(&inst)?;
            let bride = compagnie::bridage::est_bride_de(&inst);
            let phrase = if bride { "Le logiciel est bridé : c'est la démo." } else { "Le logiciel est débloqué." }.to_string();
            en_json(Bascule { bride, phrase })
        }
        Route::PortableCompleter => {
            let c: ChargePortable = charge.lire()?;
            let cles = compagnie::maj::cles_de_l_editeur();
            let complete: compagnie::maj::PortableComplete = compagnie::maj::completer_le_portable(&inst, c.portable == Some(true), &cles).map_err(|e| e.reclasser(Classe::Refuse))?;
            en_json(complete)
        }
        Route::AdministrateurDeposer => {
            let c: ChargeDeposer = charge.lire()?;
            let depot: administrateur::DepotGroupe = administrateur::deposer(o, &inst, c.reel, charge.mot_de_passe().map(String::from))?;
            en_json(depot)
        }
        Route::AdministrateurSuivre => {
            let c: ChargeSuivre = charge.lire()?;
            let suivis: administrateur::SuivisDuGroupe = administrateur::suivre(o, &inst, c.suivis)?;
            en_json(suivis)
        }
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

/// Les compagnies de l'installation, dans l'ordre des tuiles de l'accueil, et ce que l'accueil en dit.
pub fn liste(inst: &std::path::Path) -> R<Liste> {
    let mut cies: Vec<Tuile> = Vec::new();
    for c in compagnies::liste_de(inst)? {
        cies.push(Tuile {
            chemin: c.racine.to_string_lossy().into_owned(),
            remise_a_zero: !c.reelle && compagnies::est_fictive_livree(&c.racine)?,
            logo: compagnies::logo(&c.racine).map(|p| p.to_string_lossy().into_owned()),
            id: c.id,
            nom: c.nom,
            lieu: c.lieu,
            reelle: c.reelle,
            couleur: c.couleur,
        });
    }
    let a_garder: Vec<suppression::Gardee> = suppression::a_garder(inst)?.into_iter().map(|c| suppression::Gardee { id: c.id, nom: c.nom }).collect();
    let reelles = cies.iter().filter(|c| c.reelle).count();
    let phrase = format!("{} compagnie(s), dont {} fictive(s).", cies.len(), cies.len() - reelles);
    let phrases: Vec<String> = cies.iter().map(|c| format!("{}{}", c.nom, if c.reelle { "" } else { ", fictive" })).collect();
    Ok(Liste { phrase, compagnies: cies, bride: compagnie::bridage::est_bride_de(inst), a_garder, phrases, auteur: compagnie::auteur::auteur() })
}

impl Dite for compagnie::maj::PortableComplete {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// « Ajouter une compagnie » (la route `compagnie-ajouter`) dans l'installation `inst`, par le chemin `venue` : le
/// dossier et les trois fiches sans lesquelles le contexte ne s'ouvre pas.
///
/// LOGICIEL BRIDE : la compagnie ajoutee est une FICTIVE AJOUTEE (`installation::est_ajoutee`) -- sous `fictives/`, dans
/// un dossier `ajoutee-…`, avec le marqueur d'une fictive et sa marque. Debloque : une compagnie reelle, sous `compagnies/`.
pub fn ajouter_venant(inst: &Path, nom: &str, venue: Venue) -> R<Compagnie> {
    let nom = nom.split_whitespace().collect::<Vec<_>>().join(" ");
    if nom.chars().count() < 2 {
        return Err(socle::echec::refus("Le nom de la compagnie, s'il vous plait."));
    }
    let bride = compagnie::bridage::est_bride_de(inst);
    let (parent, racine_nom) = if bride {
        (inst.join(DOSSIER_FICTIVES), format!("{}{}", PREFIXE_AJOUTEE, nom_de_dossier(&nom)))
    } else {
        (inst.join("compagnies"), nom_de_dossier(&nom))
    };
    let mut dossier = parent.join(&racine_nom);
    let mut n = 2;
    while dossier.exists() {
        dossier = parent.join(format!("{}-{}", racine_nom, n));
        n += 1;
    }
    let cfg = dossier.join(socle::fichiers::CONFIG);
    std::fs::create_dir_all(&cfg).map_err(|e| socle::echec::du_disque(&cfg, &e))?;
    // La raison sociale se deduit du nom comme a chaque enregistrement de la fiche (`fiches::deriver` : majuscules SANS
    // ACCENTS, la forme declaree) : la fiche neuve nait sans erreur de configuration.
    let mut saisie = declarations::champs::Saisie::default();
    saisie.poser(droit::fiche::CaseEntreprise::nom_affichage, declarations::champs::Valeur::Texte(nom.clone()));
    crate::fiches::deriver(droit::fiche::NomDeFiche::Entreprise, &mut saisie, false, &|_| true)?;
    // Toute la configuration est datee (`droit::fiche::dater`) : la premiere valeur vaut de tout temps.
    // Le jour d'ajout : le plus tot des deux jours (Paris, et celui du moteur, en UTC).
    let creation = socle::temps::jour_de_paris().min(droit::contexte::jour_reel());
    let plate: droit::fiche::ChampsEntreprise = droit::fiche::lire_saisie(&saisie, droit::contexte::ENTREPRISE)?;
    let fiche = <droit::fiche::FicheEntrepriseDatee as droit::fiche::FicheDatee>::neuve(&plate);
    socle::ecriture::ecrire_json(&dossier.join(droit::contexte::ENTREPRISE), &fiche, 2)?;
    socle::ecriture::ecrire_json(&dossier.join(droit::contexte::SALARIES), &droit::fiche::Salaries::default(), 2)?;
    socle::ecriture::ecrire_json(&dossier.join(droit::contexte::SPECTACLES), &droit::fiche::Spectacles::default(), 2)?;
    let quand = Creation {
        le: creation,
        venue,
        quoi: "Le jour ou la compagnie a ete ajoutee a PuppetPaie (« Ajouter une compagnie »). Ce qui tombait avant lui ne la regarde pas (l'horloge d'une fictive ne coche pas une echeance d'avant). `venue` : le chemin du « + », une compagnie neuve ou d'un logiciel precedent (elle commence alors par l'import).".to_string(),
    };
    socle::ecriture::ecrire_json(&cfg.join(FICHIER_CREATION), &quand, 2)?;
    // VENUE D'UN LOGICIEL PRECEDENT, elle commence par reprendre son passe : la boite ou poser ses pieces (`import/`,
    // celle que `importation-etat` nomme) est la des le premier jour.
    if matches!(venue, Venue::LogicielPrecedent) {
        let import = dossier.join("import");
        std::fs::create_dir_all(&import).map_err(|e| socle::echec::du_disque(&import, &e))?;
    }
    let nom_du_dossier = nom_dossier(&dossier)?;
    if !bride {
        return entree(format!("cie:{}", nom_du_dossier), &dossier, true);
    }
    // Le marqueur d'une fictive (`est_fictive`), et la marque qui la dit ajoutee. `copie_de` ne dit pas le nom d'une
    // fictive livree (`maj::marquee_comme_livree` compare « compagnie fictive <nom du dossier> ») : elle n'en est pas une.
    let marqueur = Marqueur {
        copie_de: format!("{} ajoutee sur ce poste", MARQUE_FICTIVE),
        fictive: None,
        ajoutee: Some(true),
        quand: Some(socle::paris::InstantEcrit::maintenant()),
        quoi: "Une compagnie fictive ajoutee sur ce poste, le logiciel etant bride : tout s'y fait, rien n'en part vers un organisme ni vers une personne.".to_string(),
    };
    socle::ecriture::ecrire_json(&dossier.join(MARQUEUR), &marqueur, 2)?;
    entree(format!("fictive:{}", nom_du_dossier), &dossier, false)
}
