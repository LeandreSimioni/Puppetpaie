//! REPRENDRE UNE COMPAGNIE DE SON DRIVE, A L'ACCUEIL (le « + » : « De mon Drive ») : lier un compte Google a l'accueil
//! (`drive-lier`), voir ce qu'il porte (`drive-compagnies`), et ramener une compagnie sur ce poste (`drive-reprendre`)
//! comme une compagnie neuve de l'installation, par la restauration (`compagnie::restauration`, le seul chemin, que
//! `moteur restaurer` prend aussi).
//!
//! LE COMPTE GOOGLE DE L'ACCUEIL. Le jeton d'une compagnie vit sous elle (`config/google-jeton.bin`), scelle par la cle
//! du poste (`routes::coffre`) ; avant qu'il y ait une compagnie, il vit de meme sous l'installation
//! (`<installation>/config/google-jeton.bin`), scelle par la meme cle : la meme autorisation (`coffre::lier_google`), le
//! meme client (`coffre::client_drive`). La compagnie reprise en recoit une copie : elle est liee au meme compte, et sa
//! sauvegarde repart dans son dossier. Rien de ce jeton n'entre dans une sauvegarde (`sauvegarde::noms_exclus`).

use std::path::{Path, PathBuf};

use serde::Serialize;

use compagnie::installation::{IdDeCompagnie, Marqueur, MARQUEUR, MARQUE_FICTIVE, PREFIXE_AJOUTEE};
use compagnie::sur_le_drive::{DossierVu, GenreDuDossier};
use envois::drive::IdDrive;
use socle::echec::{erreur, Classe, R};
use socle::ecran::{EtatDEcran, Pastille};
use socle::fichiers::NomDeFichier;

use crate::catalogue::{Affiche, Route};
use crate::declaration::{charge, dit};
use crate::ecran::{EcranRendu, RangDeLEcran};
use crate::enveloppe::{Dite, Phrase};
use crate::imprevus::GesteDeRoute;
use crate::{Encart, SansCharge};

const LIER: Affiche = Affiche::new("Lier le Drive");
const RELIER: Affiche = Affiche::new("Relier le Drive");
const REPRENDRE: Affiche = Affiche::new("Reprendre");

charge! {
    /// La charge de `drive-reprendre` : le dossier du Drive, tel que `drive-compagnies` le designe.
    #[derive(Debug, Clone, Serialize)]
    pub struct ChargeReprendre {
        pub dossier: IdDrive = dit("Dossier du Drive", "l'identifiant du dossier, tel que drive-compagnies le rend dans la charge de son geste"),
    }
}

/// Ce que l'ecran de la reprise donne aux gestes d'un rang : rien (lier le Drive), ou le dossier a reprendre.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct DossierDuRang {
    #[serde(skip_serializing_if = "Option::is_none")]
    dossier: Option<IdDrive>,
}

/// Ce que rend `drive-compagnies` : sa phrase, et l'ecran de la reprise (le compte Google, puis chaque compagnie du Drive).
#[derive(Debug, Serialize)]
pub(crate) struct CompagniesDuDrive {
    phrase: Phrase,
    #[serde(flatten)]
    ecran: EcranRendu<DossierDuRang>,
}

/// Ce que rend `drive-lier`.
#[derive(Debug, Serialize)]
pub(crate) struct DriveLie {
    phrase: Phrase,
    compte: String,
}

/// Ce que rend `drive-reprendre` : la compagnie reprise (son identifiant, que la fenetre ouvre, son nom, son dossier), ce
/// qui est redescendu, ce qui n'a pas pu l'etre, et ce qui reste a faire pour son coffre.
#[derive(Debug, Serialize)]
pub(crate) struct Reprise {
    phrase: Phrase,
    id: IdDeCompagnie,
    nom: String,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    chemin: PathBuf,
    redescendus: usize,
    incidents: Vec<String>,
    a_faire: Vec<String>,
}

impl Dite for CompagniesDuDrive {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for DriveLie {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for Reprise {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// Ce qu'un geste de la reprise dit dans sa boite : lier, c'est Google qui s'ouvre.
const OUVRIR_GOOGLE: &str = "Le logiciel ouvre la fenêtre de Google dans votre navigateur : choisissez le compte, acceptez, puis revenez ici. Si Google dit que l'application n'est pas validée : Paramètres avancés › Accéder à PuppetPaie.";

/// L'ECRAN DE LA REPRISE (`drive-compagnies`) : le compte Google de l'accueil (le lier, le relier), puis chaque dossier du
/// Drive qui se reprend (le dossier d'une compagnie, l'ancienne sauvegarde commune), sa derniere sauvegarde, son volume,
/// et le geste « Reprendre ». Doux : sans compte lie, un Drive qui ne se lit pas, la liste se dit en encart.
pub(crate) fn compagnies(inst: &Path) -> R<CompagniesDuDrive> {
    let mut encarts = vec![Encart::de(EtatDEcran::Information, "", "Une compagnie que PuppetPaie a sauvegardée dans votre Drive revient sur ce poste, telle qu'elle y est.")];
    if compagnie::bridage::est_bride_de(inst) {
        encarts.push(Encart::de(EtatDEcran::Information, "Logiciel en démo", "La compagnie reprise sera une compagnie fictive : on y regarde tout, rien n'en part."));
    }
    let compte = crate::coffre::compte_autorise(inst);
    let geste_lier = GesteDeRoute::avec_cases(Route::DriveLier, DossierDuRang { dossier: None }, if compte.is_some() { RELIER } else { LIER }, Vec::new(), SansCharge {}, SansCharge {})
        .disant(vec![OUVRIR_GOOGLE.to_string()]);
    let (pastille, dit) = match &compte {
        Some(_) => (Pastille::de(EtatDEcran::Fait, "lié"), "Le compte dont le Drive porte la sauvegarde de vos compagnies."),
        None => (Pastille::de(EtatDEcran::AFaire, "aucun Drive lié"), "Lier le compte Google qui porte la sauvegarde de vos compagnies."),
    };
    let mut rangs = vec![RangDeLEcran { titre: "Compte Google".to_string(), dit: vec![dit.to_string()], pastille, etat: compte.clone(), montre: None, gestes: vec![geste_lier] }];
    let fini = |encarts, rangs, phrase| CompagniesDuDrive { phrase, ecran: EcranRendu { encarts, rangs, cases: Vec::new(), historique: Vec::new(), enregistrer: None } };
    let Some(compte) = compte else {
        return Ok(fini(encarts, rangs, Phrase::fixe("Aucun compte Google lié : le lier d'abord.")));
    };
    let vus = crate::coffre::client_drive(inst).and_then(|mut c| compagnie::sur_le_drive::voir(&mut c, compagnie::sauvegarde::RACINE_DRIVE));
    let vus: Vec<DossierVu> = match vus {
        Ok(v) => v.into_iter().filter(DossierVu::se_reprend).collect(),
        Err(e) => {
            encarts.push(Encart::de(EtatDEcran::Refuse, "Le Drive ne se lit pas", e.echec.phrase));
            return Ok(fini(encarts, rangs, socle::phrase!("Le Drive de {} ne se lit pas.", compte)));
        }
    };
    if vus.is_empty() {
        encarts.push(Encart::de(EtatDEcran::Information, "Aucune compagnie dans ce Drive", "Ce compte Google ne porte aucune sauvegarde de PuppetPaie. Est-ce le bon compte ?"));
    }
    for vu in &vus {
        let mut dit = vec![match vu.genre {
            GenreDuDossier::AncienneSauvegarde => "L'ancienne sauvegarde, d'avant le dossier par compagnie : si plusieurs compagnies partageaient ce compte, elles y sont mêlées.".to_string(),
            GenreDuDossier::Compagnie(_) | GenreDuDossier::Ancien => "La sauvegarde de cette compagnie.".to_string(),
        }];
        dit.push(crate::drive::derniere_dite(vu));
        if let Some(o) = vu.octets {
            dit.push(format!("Volume : {}.", o));
        }
        dit.extend(vu.illisible.iter().map(|i| format!("Sa marque : {}.", i)));
        let reprendre = GesteDeRoute::avec_cases(Route::DriveReprendre, DossierDuRang { dossier: Some(vu.id.clone()) }, REPRENDRE, Vec::new(), SansCharge {}, SansCharge {})
            .disant(vec![format!("« {} » revient sur ce poste, puis s'ouvre. Son coffre revient aussi : il s'ouvre avec le mot de passe du logiciel de cette compagnie.", vu.nom)])
            .principal();
        rangs.push(RangDeLEcran { titre: vu.nom.clone(), dit, pastille: Pastille::de(EtatDEcran::Information, "dans le Drive"), etat: None, montre: None, gestes: vec![reprendre] });
    }
    Ok(fini(encarts, rangs, socle::phrase!("Le Drive de {} : {} compagnie(s) à reprendre.", compte, vus.len())))
}

/// LIER LE COMPTE GOOGLE DE L'ACCUEIL (`drive-lier`) : la meme autorisation que celle d'une compagnie, scellee sous
/// l'installation. Aucun mot de passe : il n'y a pas encore de compagnie, et rien ne part de l'accueil.
pub(crate) fn lier(inst: &Path) -> R<DriveLie> {
    let compte = crate::coffre::lier_google(inst)?;
    Ok(DriveLie { phrase: socle::phrase!("Drive lié : {}.", compte), compte })
}

/// LA RACINE OU REVIENT LE DOSSIER `vu` : la compagnie de l'installation qui porte deja son identite (une reprise coupee
/// se relance et reprend ou elle en etait) ; sinon un dossier neuf, comme « Ajouter une compagnie » le fait : une fictive
/// ajoutee d'un logiciel bride, une compagnie reelle sinon. Rend son identifiant, sa racine, et si elle est neuve.
fn ou_revient(inst: &Path, vu: &DossierVu, bride: bool) -> R<(IdDeCompagnie, PathBuf, bool)> {
    if let GenreDuDossier::Compagnie(identite) = &vu.genre {
        for c in compagnie::installation::liste_de(inst)? {
            if compagnie::sauvegarde::identite_ecrite(&c.racine)?.as_ref() == Some(identite) {
                return Ok((c.id, c.racine, false));
            }
        }
    }
    let nom = compagnie::installation::nom_de_dossier(&vu.nom);
    let (id, racine) = if bride {
        IdDeCompagnie::libre(inst, IdDeCompagnie::Fictive, &NomDeFichier::lire(&format!("{}{}", PREFIXE_AJOUTEE, nom))?)?
    } else {
        IdDeCompagnie::libre(inst, IdDeCompagnie::Cie, &NomDeFichier::lire(&nom)?)?
    };
    Ok((id, racine, true))
}

/// REPRENDRE UNE COMPAGNIE DU DRIVE (`drive-reprendre`) : le dossier, s'il est sous la racine du Drive lie a l'accueil
/// (relue) et se reprend, revient sur ce poste (`ou_revient`), par la restauration ; la compagnie reprise recoit le jeton
/// de l'accueil (liee au meme compte). D'un logiciel bride, elle devient une fictive ajoutee (rien n'en part). Un refus
/// de la restauration n'ecrit rien : le dossier neuf qu'elle aurait rempli s'efface.
pub(crate) fn reprendre(inst: &Path, dossier: &IdDrive) -> R<Reprise> {
    if !crate::coffre::autorise(inst) {
        return Err(erreur(Classe::Refuse, "Aucun compte Google lié à l'accueil : le lier d'abord (« Lier le Drive »). Rien n'est repris."));
    }
    let racine_drive = compagnie::sauvegarde::RACINE_DRIVE;
    let mut client = crate::coffre::client_drive(inst).map_err(|e| e.echec)?;
    let vus = compagnie::sur_le_drive::voir(&mut client, racine_drive).map_err(|e| e.echec)?;
    let vu = compagnie::sur_le_drive::le_dossier(vus, dossier, racine_drive).map_err(|e| e.echec)?;
    if !vu.se_reprend() {
        return Err(erreur(Classe::Refuse, format!("« {} » n'est pas la sauvegarde d'une compagnie : rien n'est repris.", vu.nom)));
    }
    let bride = compagnie::bridage::est_bride_de(inst);
    let (id, racine, neuve) = ou_revient(inst, &vu, bride)?;
    let config = racine.join(socle::fichiers::CONFIG);
    std::fs::create_dir_all(&config).map_err(|e| socle::echec::du_disque(&config, &e))?;
    // Le jeton de l'accueil, tel qu'il est scelle : la compagnie reprise est liee au meme compte.
    let jeton = socle::fichiers::JETON_GOOGLE.dans(inst);
    if !bride && !socle::fichiers::JETON_GOOGLE.dans(&racine).exists() {
        std::fs::copy(&jeton, socle::fichiers::JETON_GOOGLE.dans(&racine)).map_err(|e| socle::echec::du_disque(&jeton, &e))?;
    }
    let r = match compagnie::restauration::restaurer(&racine, &vu, racine_drive, &mut client) {
        Ok(r) => r,
        Err(e) => {
            if neuve {
                // Rien n'a ete redescendu : le dossier neuf ne porte que le jeton copie.
                std::fs::remove_dir_all(&racine).map_err(|x| socle::echec::du_disque(&racine, &x))?;
            }
            return Err(e.echec);
        }
    };
    if bride && neuve {
        let marqueur = Marqueur {
            copie_de: format!("{} ajoutee sur ce poste", MARQUE_FICTIVE),
            fictive: None,
            ajoutee: Some(true),
            quand: Some(socle::temps::secondes().a_la_seconde()),
            quoi: "Une compagnie reprise du Drive sur ce poste, le logiciel etant bride : tout s'y fait, rien n'en part vers un organisme ni vers une personne.".to_string(),
        };
        socle::ecriture::ecrire_json(&racine.join(MARQUEUR), &marqueur, 2)?;
    }
    let a_faire = compagnie::restauration::a_faire(&compagnie::coffre::Coffre::du_poste(&racine));
    let nom = compagnie::installation::nom_de(&racine)?;
    let suite = if r.hors_ligne {
        " La connexion est tombée en route : reprendre de nouveau reprend où elle en était."
    } else if !r.incidents.is_empty() {
        " Des fichiers ne sont pas revenus : ils sont dits ci-dessous."
    } else {
        ""
    };
    Ok(Reprise {
        phrase: socle::phrase!("« {} » est reprise de votre Drive : {} fichier(s) revenu(s).{}", nom, r.redescendus.len(), suite),
        id,
        nom,
        chemin: racine,
        redescendus: r.redescendus.len(),
        incidents: r.incidents,
        a_faire,
    })
}
