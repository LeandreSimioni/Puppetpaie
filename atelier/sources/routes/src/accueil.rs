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

pub mod drive;
pub mod suppression;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use compagnie::installation as compagnies;
use compagnie::installation::{entree, nom_de_dossier, Compagnie, CouleurDeCompagnie, Creation, IdDeCompagnie, Marqueur, Venue, FICHIER_CREATION, MARQUEUR, MARQUE_FICTIVE, PREFIXE_AJOUTEE};
use socle::fichiers::NomDeFichier;
use crate::gestes::administrateur;
use crate::catalogue::{Acte, Affiche, Ecran, Fiche, Place, Refus, MANQUE, OPERATION};
use crate::catalogue::Route;
use crate::declaration::{charge, dit, Legende};
use crate::enveloppe::{en_json, Charge, Dite, Options, Phrase, Rendu};
use socle::echec::{erreur, Classe, Echec, R};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Accueil);

const COMPAGNIE: Legende = dit("Compagnie", "l'identifiant de la compagnie, tel que `compagnies` le rend");

/// Les routes de l'installation, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Compagnies,
        geste: "Dire les compagnies de cette installation, et où est chacune.",
        charge: &crate::SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "compagnies (id, nom, lieu, chemin — le --racine de ses routes —, reelle, remise_a_zero, couleur), bride, a_garder (au déblocage), phrase, phrases (une par compagnie), auteur (phrase, nom, adresse)",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Lit,
        phrase: 80,
        ecran: Ecran::Lecture,
    },
    Fiche {
        route: Route::CompagnieAjouter,
        geste: "Ajouter une compagnie : neuve, ou venant d'un logiciel précédent.",
        charge: &ChargeAjouter::CHARGE,
        exemple: r#"{"nom":"Les Tréteaux d'essai","venue":"neuve"}"#,
        rend: "id, nom, chemin (le --racine de ses routes), phrase ; logiciel bridé : une compagnie fictive ajoutée",
        refus: &[MANQUE, OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 120,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::CompagnieCouleur,
        geste: "Choisir la couleur d'une compagnie.",
        charge: &ChargeCouleur::CHARGE,
        exemple: r##"{"compagnie":"cie:la-compagnie","couleur":"#2E5A7D"}"##,
        rend: "couleur, phrase",
        refus: &[MANQUE, Refus { classe: Classe::Refuse, corriger: "une couleur #RRGGBB, une compagnie réelle ; rien n'est écrit" }],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 80,
        ecran: Ecran::DessineAvance(Place::Accueil),
    },
    Fiche {
        route: Route::CompagnieRemettreAZero,
        geste: "Remettre une compagnie fictive livrée dans son état d'origine.",
        charge: &ChargeCompagnie::CHARGE,
        exemple: r#"{"compagnie":"fictive:fictive-essai"}"#,
        rend: "phrase",
        refus: &[MANQUE, Refus { classe: Classe::Refuse, corriger: "une compagnie fictive livrée ; rien n'est effacé" }],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 120,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::CompagniesGarder,
        geste: "Au déblocage : garder les compagnies créées en démo, ou les supprimer.",
        charge: &ChargeGarder::CHARGE,
        exemple: r#"{"garder":["fictive:ajoutee-demo"],"ne_pas_garder":[]}"#,
        rend: "gardees, supprimees, erreurs, phrase",
        refus: &[Refus { classe: Classe::Refuse, corriger: "une compagnie créée en démo, d'un logiciel débloqué ; rien n'a changé" }],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 200,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::AdministrateurTableau,
        geste: "Dire les déclarations à faire de chaque compagnie de l'administrateur.",
        charge: &ChargeTableau::CHARGE,
        exemple: r#"{}"#,
        rend: "compagnies (chacune ses déclarations triées par urgence, et où en est leur fichier), groupe (ce que le dépôt groupé propose)",
        refus: &[OPERATION],
        // Le tableau d'une compagnie qui attend une valeur a saisir de nouveau ne se calcule pas (`etat_garde::Cle::route`).
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Accueil),
    },
    Fiche {
        route: Route::AdministrateurDeposer,
        geste: "Déposer d'un geste ce qui est prêt dans chaque compagnie de l'administrateur.",
        charge: &ChargeDeposer::CHARGE,
        exemple: r#"{"mode":"test"}"#,
        rend: "partis, refuses, resultats (par dépôt : la compagnie, ce qui est parti ou le refus, et suivre pour un dépôt réel), tableau ; rien de prêt : phrase",
        refus: &[Refus { classe: Classe::PreuveInsuffisante, corriger: "le mot de passe, tapé par un humain ; rien n'est déposé" }, OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Engage,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Accueil),
    },
    Fiche {
        route: Route::AdministrateurSuivre,
        geste: "Après un dépôt groupé réel, relever les retours de chaque déclaration partie.",
        charge: &ChargeSuivre::CHARGE,
        exemple: r#"{"suivis":[]}"#,
        rend: "fini (toutes tranchées), suivies (par déclaration : la compagnie, passée ou refusée, la phrase et sa couleur), phrase",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::DessineAvance(Place::Accueil),
    },
    Fiche {
        route: Route::PortableCompleter,
        geste: "Ajouter au dossier les systèmes qui lui manquent (Windows, Linux, macOS).",
        charge: &ChargePortable::CHARGE,
        exemple: r#"{"portable":true}"#,
        rend: "ajoutes, absents (les systèmes que la version n'a pas), version, portable, phrase",
        refus: &[Refus { classe: Classe::Refuse, corriger: "le réseau, la mise à jour en attente, ou ce que la phrase dit ; rien n'est posé" }],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 140,
        ecran: Ecran::Auto(Place::Accueil, Affiche::new("Ajouter les systèmes qui manquent")),
    },
    Fiche {
        route: Route::Basculer,
        geste: "Brider le logiciel (la démo) ou le débloquer, sur ce poste.",
        charge: &crate::SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "bride (vrai : la démo), phrase ; un paquet soudé ne change pas",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 80,
        ecran: ECRAN,
    },
];

/// LES ROUTES DE L'INSTALLATION, chaque module de l'accueil sa liste (sa place) : celles-ci, et reprendre une compagnie de
/// son Drive (`drive::ROUTES`). Elles se servent sans compagnie (`route`).
pub const DE_L_INSTALLATION: [&[Fiche]; 2] = [ROUTES, drive::ROUTES];

/// `route` est-elle une route de l'installation (`DE_L_INSTALLATION`) ?
pub fn est_de_l_installation(route: Route) -> bool {
    DE_L_INSTALLATION.iter().any(|l| crate::catalogue::dans(l, route).is_some())
}

/// L'installation de la route : celle de la compagnie nommee (`--racine`), sinon celle du programme.
pub fn installation(o: &Options) -> PathBuf {
    match &o.racine {
        Some(r) => IdDeCompagnie::installation_de(r),
        None => compagnies::base(),
    }
}

charge! {
    /// La charge de `compagnie-ajouter` : son nom, et d'ou elle vient.
    #[derive(Debug)]
    pub struct ChargeAjouter {
        pub nom: String = dit("Nom de la compagnie", "le nom de la compagnie"),
        pub venue: compagnies::Venue = dit("Venue", "d'où elle vient : venue d'un autre logiciel, elle commence par reprendre son passé"),
    }
}

charge! {
    /// La charge de `compagnie-couleur`.
    #[derive(Debug)]
    struct ChargeCouleur {
        compagnie: IdDeCompagnie = COMPAGNIE,
        couleur: CouleurDeCompagnie = dit("Couleur", "#RRGGBB"),
    }
}

charge! {
    /// La charge qui nomme une compagnie de l'accueil.
    #[derive(Debug)]
    struct ChargeCompagnie {
        compagnie: IdDeCompagnie = COMPAGNIE,
    }
}

charge! {
    /// La charge de `compagnies-garder` : les compagnies a garder, et celles a supprimer.
    #[derive(Debug)]
    struct ChargeGarder {
        garder: Vec<IdDeCompagnie> = dit("À garder", "les identifiants des compagnies à garder (elles deviennent réelles)"),
        ne_pas_garder: Vec<IdDeCompagnie> = dit("À supprimer", "les identifiants des compagnies à supprimer"),
    }
}

charge! {
    /// La charge de `administrateur-tableau` : le tableau garde, ou refait (apres un releve, s'il est demande).
    #[derive(Debug, Default, Serialize)]
    pub struct ChargeTableau {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub garde: Option<bool> = dit("Tableau gardé", "vrai : le tableau gardé, sans calcul"),
        #[serde(skip_serializing_if = "Option::is_none")]
        pub relever: Option<bool> = dit("Relever d'abord les comptes rendus", "vrai : relever d'abord les comptes rendus attendus"),
    }
}

charge! {
    motdepasse = dit("Mot de passe de l'administrateur", "le mot de passe de l'administrateur ; sans lui, le moteur le demande à un humain");
    /// La charge de `administrateur-deposer` : en reel, ou en test.
    #[derive(Debug)]
    struct ChargeDeposer {
        mode: socle::mode::Mode = dit("Test ou réel", "le réel ne dépose que les fichiers testés conformes"),
    }
}

charge! {
    /// La charge de `administrateur-suivre` : les declarations a suivre, telles que le depot groupe les a rendues
    /// (`suivre` de chaque resultat parti en reel).
    #[derive(Debug)]
    struct ChargeSuivre {
        suivis: Vec<administrateur::ASuivre> = dit("Déclarations à suivre", "la liste des « suivre » des résultats partis en réel, tels qu'administrateur-deposer les rend"),
    }
}

charge! {
    /// La charge de `portable-completer`.
    #[derive(Debug)]
    struct ChargePortable {
        portable: Option<bool> = dit("Rendre ce dossier portable", "vrai : faire aussi de ce dossier un dossier portable, pour une clé USB (PORTABLE.json)"),
    }
}

/// Une compagnie de l'accueil, telle qu'une tuile la montre.
#[derive(Debug, Serialize)]
pub struct Tuile {
    pub id: IdDeCompagnie,
    pub nom: String,
    pub lieu: String,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    chemin: PathBuf,
    pub reelle: bool,
    pub remise_a_zero: bool,
    pub couleur: CouleurDeCompagnie,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub logo: Option<PathBuf>,
}

/// Ce que rend `compagnies`, et ce que l'accueil de la fenetre dessine (`interface::page_accueil`).
#[derive(Debug, Serialize)]
pub struct Liste {
    phrase: Phrase,
    pub compagnies: Vec<Tuile>,
    pub bride: bool,
    pub a_garder: Vec<suppression::Gardee>,
    phrases: Vec<Phrase>,
    pub auteur: compagnie::auteur::Auteur,
}

/// Ce que rend `compagnie-ajouter` : la compagnie, et le chemin que la fenetre ouvre.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ajoutee {
    phrase: Phrase,
    id: IdDeCompagnie,
    nom: String,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub chemin: PathBuf,
}

/// Ce que rend `compagnie-couleur`.
#[derive(Debug, Serialize)]
struct Couleur {
    phrase: Phrase,
    couleur: CouleurDeCompagnie,
}

/// Ce que rend une route qui n'a que sa phrase.
#[derive(Debug, Serialize)]
struct Dit {
    phrase: Phrase,
}

/// Ce que rend `basculer` : l'etat du logiciel sur ce poste, et sa phrase.
#[derive(Debug, Serialize)]
struct Bascule {
    bride: bool,
    phrase: Phrase,
}

/// Ce que rend `compagnies-garder`.
#[derive(Debug, Serialize)]
struct Gardees {
    phrase: Phrase,
    #[serde(flatten)]
    reponse: suppression::Reponse,
}

impl Dite for Liste {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for Ajoutee {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for Couleur {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for Dit {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for Bascule {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for Gardees {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// Une route de l'installation, branchee sur l'enveloppe. Le tableau de l'administrateur, son depot groupe et le dossier
/// portable rendent ce que `gestes::administrateur` et `compagnie::maj` ecrivent.
pub fn route(o: &Options, nom: Route, charge: &Charge) -> R<Rendu> {
    // `--racine` nomme une compagnie (son dossier porte sa fiche entreprise) : l'installation est la sienne. Un autre
    // dossier n'en est pas une, et son parent n'est pas une installation : rien ne s'y ecrit.
    if let Some(r) = &o.racine {
        if !droit::contexte::ENTREPRISE.dans(&r).is_file() {
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
                socle::phrase!("{} est ajoutée.", c.nom)
            } else {
                socle::phrase!("{} est ajoutée, en compagnie fictive : rien n'en part.", c.nom)
            };
            en_json(Ajoutee { phrase, id: c.id, nom: c.nom, chemin: c.racine })
        }
        Route::CompagnieCouleur => {
            let c: ChargeCouleur = charge.lire()?;
            let couleur = compagnies::poser_couleur(&inst, &c.compagnie, &c.couleur).map_err(|e| e.reclasser(Classe::Refuse))?;
            en_json(Couleur { phrase: socle::phrase!("Couleur posée : {}.", couleur), couleur })
        }
        Route::CompagnieRemettreAZero => {
            let c: ChargeCompagnie = charge.lire()?;
            let (n, deja) = compagnies::remettre_a_zero(&inst, &c.compagnie).map_err(|e| e.reclasser(Classe::Refuse))?;
            en_json(Dit { phrase: socle::phrase!("{} est {}.", n, if deja { "déjà dans son état d'origine" } else { "revenue à son état d'origine" }) })
        }
        Route::CompagniesGarder => {
            let c: ChargeGarder = charge.lire()?;
            let poste = administrateur::Poste::de_ce_poste();
            let reponse = suppression::repondre(&inst, &c.garder, &c.ne_pas_garder, &poste, &|_| {}).map_err(|e| e.reclasser(Classe::Refuse))?;
            let erreurs: String = reponse.erreurs.iter().map(|e| format!(" {}", e)).collect();
            let phrase = socle::phrase!("{} compagnie(s) gardée(s), {} supprimée(s).{}", reponse.gardees.len(), reponse.supprimees.len(), erreurs);
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
        _ if crate::catalogue::dans(drive::ROUTES, nom).is_some() => drive::route(&inst, nom, charge),
        Route::Basculer => {
            compagnie::bridage::basculer_de(&inst)?;
            let bride = compagnie::bridage::est_bride_de(&inst);
            let phrase = Phrase::fixe(if bride { "Le logiciel est bridé : c'est la démo." } else { "Le logiciel est débloqué." });
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
            let depot: administrateur::DepotGroupe = administrateur::deposer(o, &inst, c.mode, charge.mot_de_passe().cloned())?;
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
            chemin: c.racine.clone(),
            remise_a_zero: !c.reelle && compagnies::est_fictive_livree(&c.racine)?,
            logo: compagnies::logo(&c.racine),
            id: c.id,
            nom: c.nom,
            lieu: c.lieu,
            reelle: c.reelle,
            couleur: c.couleur,
        });
    }
    let a_garder: Vec<suppression::Gardee> = suppression::a_garder(inst)?.into_iter().map(|c| suppression::Gardee { id: c.id, nom: c.nom }).collect();
    let reelles = cies.iter().filter(|c| c.reelle).count();
    let phrase = socle::phrase!("{} compagnie(s), dont {} fictive(s).", cies.len(), cies.len() - reelles);
    let phrases: Vec<Phrase> = cies.iter().map(|c| socle::phrase!("{}{}", c.nom, if c.reelle { "" } else { ", fictive" })).collect();
    Ok(Liste { phrase, compagnies: cies, bride: compagnie::bridage::est_bride_de(inst), a_garder, phrases, auteur: compagnie::auteur::auteur() })
}

impl Dite for compagnie::maj::PortableComplete {
    fn phrase(&self) -> &Phrase {
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
    let (id, dossier) = if bride {
        IdDeCompagnie::libre(inst, IdDeCompagnie::Fictive, &NomDeFichier::lire(&format!("{}{}", PREFIXE_AJOUTEE, nom_de_dossier(&nom)))?)?
    } else {
        IdDeCompagnie::libre(inst, IdDeCompagnie::Cie, &NomDeFichier::lire(&nom_de_dossier(&nom))?)?
    };
    let cfg = dossier.join(socle::fichiers::CONFIG);
    std::fs::create_dir_all(&cfg).map_err(|e| socle::echec::du_disque(&cfg, &e))?;
    // La raison sociale se deduit du nom comme a chaque enregistrement de la fiche (`SorteDeFiche::deriver` : majuscules
    // SANS ACCENTS, la forme declaree) : la fiche neuve nait sans erreur de configuration. Ce que son type refuse se dit.
    let mut plate = droit::fiche::ChampsEntreprise { nom_affichage: Some(nom.clone()), ..Default::default() };
    let fautes = <droit::fiche::FicheEntrepriseDatee as crate::fiches::SorteDeFiche>::deriver(&mut plate, false, &|_| true)?;
    if let Some(f) = fautes.first() {
        return Err(socle::echec::refus(format!("{} : {}", droit::contexte::ENTREPRISE, f.phrase)));
    }
    // Toute la configuration est datee (`droit::fiche::dater`) : la premiere valeur vaut de tout temps.
    // Le jour d'ajout : le plus tot des deux jours (Paris, et celui du moteur, en UTC).
    let creation = socle::temps::jour_de_paris().min(droit::contexte::jour_reel());
    let fiche = <droit::fiche::FicheEntrepriseDatee as droit::fiche::FicheDatee>::neuve(&plate);
    socle::ecriture::ecrire_json(&droit::contexte::ENTREPRISE.dans(&dossier), &fiche, 2)?;
    socle::ecriture::ecrire_json(&droit::contexte::SALARIES.dans(&dossier), &droit::fiche::Salaries::default(), 2)?;
    socle::ecriture::ecrire_json(&droit::contexte::SPECTACLES.dans(&dossier), &droit::fiche::Spectacles::default(), 2)?;
    let quand = Creation {
        le: creation,
        venue,
        quoi: "Le jour ou la compagnie a ete ajoutee a PuppetPaie (« Ajouter une compagnie »). Ce qui tombait avant lui ne la regarde pas (l'horloge d'une fictive ne coche pas une echeance d'avant). `venue` : le chemin du « + », une compagnie neuve ou d'un logiciel precedent (elle commence alors par l'import).".to_string(),
    };
    socle::ecriture::ecrire_json(&cfg.join(FICHIER_CREATION), &quand, 2)?;
    // VENUE D'UN LOGICIEL PRECEDENT, elle commence par reprendre son passe : la boite ou poser ses pieces (`import/`,
    // celle que `importation-etat` nomme) est la des le premier jour.
    if matches!(venue, Venue::LogicielPrecedent) {
        let import = dossier.join(import::BOITE_AUX_LETTRES);
        std::fs::create_dir_all(&import).map_err(|e| socle::echec::du_disque(&import, &e))?;
    }
    if !bride {
        return entree(id, &dossier, true);
    }
    // Le marqueur d'une fictive (`est_fictive`), et la marque qui la dit ajoutee. `copie_de` ne dit pas le nom d'une
    // fictive livree (`maj::marquee_comme_livree` compare « compagnie fictive <nom du dossier> ») : elle n'en est pas une.
    let marqueur = Marqueur {
        copie_de: format!("{} ajoutee sur ce poste", MARQUE_FICTIVE),
        fictive: None,
        ajoutee: Some(true),
        quand: Some(socle::temps::secondes().a_la_seconde()),
        quoi: "Une compagnie fictive ajoutee sur ce poste, le logiciel etant bride : tout s'y fait, rien n'en part vers un organisme ni vers une personne.".to_string(),
    };
    socle::ecriture::ecrire_json(&dossier.join(MARQUEUR), &marqueur, 2)?;
    entree(id, &dossier, false)
}
