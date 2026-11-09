//! CE QUE L'ACCUEIL FAIT DES COMPAGNIES DE L'INSTALLATION, au niveau des routes (`routes::accueil` les sert) : en
//! ajouter une (le « + », `ajouter_venant`), en supprimer une, garder au deblocage celles qu'on a ajoutees en demo
//! (`suppression`). Ce qu'est une compagnie sur le poste (sa racine, son marqueur, sa tuile) : `compagnie::installation`.

pub mod suppression;

use std::path::Path;

use crate::compagnie::installation::{entree, nom_de_dossier, nom_dossier, Compagnie, Creation, Marqueur, Venue, DOSSIER_FICTIVES, FICHIER_CREATION, MARQUEUR, MARQUE_FICTIVE, PREFIXE_AJOUTEE};

/// « Ajouter une compagnie » (la route `compagnie-ajouter`) dans l'installation `inst`, par le chemin `venue` : le
/// dossier et les trois fiches sans lesquelles le contexte ne s'ouvre pas.
///
/// LOGICIEL BRIDE : la compagnie ajoutee est une FICTIVE AJOUTEE (`installation::est_ajoutee`) -- sous `fictives/`, dans
/// un dossier `ajoutee-…`, avec le marqueur d'une fictive et sa marque. Debloque : une compagnie reelle, sous `compagnies/`.
pub fn ajouter_venant(inst: &Path, nom: &str, venue: Venue) -> Result<Compagnie, String> {
    let nom = nom.split_whitespace().collect::<Vec<_>>().join(" ");
    if nom.chars().count() < 2 {
        return Err("Le nom de la compagnie, s'il vous plait.".into());
    }
    let bride = crate::compagnie::bridage::est_bride_de(inst);
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
    // La raison sociale se deduit du nom comme a chaque enregistrement de la fiche (`routes::fiches::deriver` : majuscules
    // SANS ACCENTS, la forme declaree) : la fiche neuve nait sans erreur de configuration.
    let mut saisie = declarations::champs::Saisie::default();
    saisie.poser("nom_affichage", declarations::champs::Valeur::Texte(nom.clone()));
    crate::routes::fiches::deriver("entreprise", &mut saisie, false, &|_| true).map_err(|e| e.message)?;
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
        depuis: None,
        ajoutee: Some(true),
        quand: Some(socle::paris::maintenant()),
        quoi: "Une compagnie fictive ajoutee sur ce poste, le logiciel etant bride : tout s'y fait, rien n'en part vers un organisme ni vers une personne.".to_string(),
    };
    socle::ecriture::ecrire_json(&dossier.join(MARQUEUR), &marqueur, 2)?;
    entree(format!("fictive:{}", nom_du_dossier), &dossier, false)
}
