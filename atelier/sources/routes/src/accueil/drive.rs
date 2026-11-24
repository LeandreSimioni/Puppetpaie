//! REPRENDRE UNE COMPAGNIE DE SON DRIVE, A L'ACCUEIL (le « + » : « De mon Drive ») : lier un compte Google a l'accueil
//! (`drive-lier`) ou l'oublier (`drive-oublier`), voir ce qu'il porte (`drive-compagnies`, l'ecran de
//! `Place::AccueilDrive`), et ramener une compagnie sur ce poste (`drive-reprendre`) comme une compagnie neuve de
//! l'installation, par la restauration (`compagnie::restauration`, le seul chemin, que `moteur restaurer` prend aussi).
//!
//! LE COMPTE GOOGLE DE L'ACCUEIL. Le jeton d'une compagnie vit sous elle (`config/google-jeton.bin`), scelle par la cle
//! du poste (`routes::coffre`) ; avant qu'il y ait une compagnie, il vit de meme sous l'installation
//! (`<installation>/config/google-jeton.bin`), scelle par la meme cle : la meme autorisation (`coffre::lier_google`), le
//! meme client (`coffre::client_drive`). Rien de ce jeton n'entre dans une sauvegarde (`sauvegarde::ecarte`).
//!
//! UNE REPRISE NE VA JAMAIS DANS UNE COMPAGNIE QUI EXISTE : elle cree un dossier neuf, y pose d'abord sa marque
//! (`restauration::RepriseEnCours`), puis la restauration le remplit. Coupee (le reseau, un fichier qui n'est pas
//! revenu), elle se relance dans ce dossier-la, que sa marque designe, et dans aucun autre. Allee au bout, la compagnie
//! recoit une copie du jeton de l'accueil (elle est liee au meme compte, et sa sauvegarde repart dans le meme dossier du
//! Drive), sa marque s'efface, et la fenetre l'ouvre (`ouvrir`). Avant, aucune compagnie de ce poste n'est liee a un
//! compte Google par la reprise : rien de ce qu'elle n'a pas fini ne part au Drive.

use std::path::{Path, PathBuf};

use serde::Serialize;

use compagnie::installation::{IdDeCompagnie, Marqueur, MARQUEUR, MARQUE_FICTIVE, PREFIXE_AJOUTEE};
use compagnie::restauration::RepriseEnCours;
use compagnie::sur_le_drive::{DossierVu, GenreDuDossier};
use envois::drive::IdDrive;
use socle::echec::{erreur, Classe, R};
use socle::ecran::{EtatDEcran, Pastille};
use socle::fichiers::NomDeFichier;

use crate::catalogue::{Affiche, Route};
use crate::declaration::{charge, dit};
use crate::drive::DossierDuRang;
use crate::ecran::{EcranRendu, RangDeLEcran};
use crate::enveloppe::{Dite, Phrase};
use crate::imprevus::GesteDeRoute;
use crate::{Encart, SansCharge};

const LIER: Affiche = Affiche::new("Lier le Drive");
const RELIER: Affiche = Affiche::new("Relier le Drive");
const OUBLIER: Affiche = Affiche::new("Oublier ce compte");
const REPRENDRE: Affiche = Affiche::new("Reprendre");

charge! {
    /// La charge de `drive-reprendre` : le dossier du Drive, tel que `drive-compagnies` le designe.
    #[derive(Debug, Clone, Serialize)]
    pub struct ChargeReprendre {
        pub dossier: IdDrive = dit("Dossier du Drive", "l'identifiant du dossier, tel que drive-compagnies le rend dans la charge de son geste"),
    }
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

/// Ce que rend `drive-oublier` : si un jeton s'est efface.
#[derive(Debug, Serialize)]
pub(crate) struct DriveOublie {
    phrase: Phrase,
    efface: bool,
}

/// Ce que rend `drive-reprendre` : la compagnie que la reprise remplit (son nom, son dossier sur ce poste), ce qui est
/// redescendu, le detail de ce qui ne l'est pas (`incidents`, un fichier par ligne), ce qui reste a faire pour son coffre
/// (`a_faire`, en mots) ; et, la reprise allee au bout, la compagnie que la fenetre ouvre (`ouvrir`).
#[derive(Debug, Serialize)]
pub(crate) struct Reprise {
    phrase: Phrase,
    #[serde(skip_serializing_if = "Option::is_none")]
    ouvrir: Option<IdDeCompagnie>,
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

impl Dite for DriveOublie {
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
/// Ce que dit la boite d'« Oublier ce compte ».
const OUBLIER_DIT: &str = "Ce poste oublie le compte Google de l'accueil. Les compagnies déjà reprises gardent le leur. L'accès accordé côté Google se retire sur myaccount.google.com.";

/// Une reprise pas finie sur ce poste : la compagnie qu'elle remplit, sa racine, sa marque.
type Commencee = (IdDeCompagnie, PathBuf, RepriseEnCours);

/// LES REPRISES PAS FINIES DE L'INSTALLATION `inst` : chaque compagnie qui porte la marque d'une reprise
/// (`RepriseEnCours`), avec cette marque ; et, a part, le nom de chaque compagnie dont la marque ne se lit pas, et
/// pourquoi. Une marque illisible ne designe aucun dossier du Drive : une reprise ne la continue pas, et l'ecran le dit.
fn reprises_en_cours(inst: &Path) -> R<(Vec<Commencee>, Vec<(String, socle::echec::Echec)>)> {
    let (mut lues, mut illisibles) = (Vec::new(), Vec::new());
    for c in compagnie::installation::liste_de(inst)? {
        match compagnie::restauration::reprise_en_cours(&c.racine) {
            Ok(Some(r)) => lues.push((c.id, c.racine, r)),
            Ok(None) => {}
            Err(e) => illisibles.push((c.nom, e)),
        }
    }
    Ok((lues, illisibles))
}

/// L'ECRAN DE LA REPRISE (`drive-compagnies`) : le compte Google de l'accueil (le lier, le relier, l'oublier), puis chaque
/// dossier du Drive qui se reprend (le dossier d'une compagnie, l'ancienne sauvegarde commune), sa derniere sauvegarde,
/// son volume, une reprise pas finie sur ce poste, et le geste « Reprendre ». Doux : sans compte lie, un Drive qui ne se
/// lit pas, des compagnies du poste qui ne se listent pas, cela se dit en encart.
pub(crate) fn compagnies(inst: &Path) -> R<CompagniesDuDrive> {
    let mut encarts = vec![Encart::de(EtatDEcran::Information, "", "Une compagnie que PuppetPaie a sauvegardée dans votre Drive revient sur ce poste, telle qu'elle y est, dans un dossier neuf.")];
    if compagnie::bridage::est_bride_de(inst) {
        encarts.push(Encart::de(EtatDEcran::Information, "Logiciel en démo", "La compagnie reprise sera une compagnie fictive : on y regarde tout, rien n'en part."));
    }
    let compte = crate::coffre::compte_autorise(inst);
    let lier = |libelle| GesteDeRoute::avec_cases(Route::DriveLier, DossierDuRang { dossier: None }, libelle, Vec::new(), SansCharge {}, SansCharge {}).disant(vec![OUVRIR_GOOGLE.to_string()]);
    let gestes = match &compte {
        Some(_) => vec![
            lier(RELIER),
            GesteDeRoute::avec_cases(Route::DriveOublier, DossierDuRang { dossier: None }, OUBLIER, Vec::new(), SansCharge {}, SansCharge {}).disant(vec![OUBLIER_DIT.to_string()]),
        ],
        None => vec![lier(LIER).principal()],
    };
    let (pastille, dit) = match &compte {
        Some(_) => (Pastille::de(EtatDEcran::Fait, "lié"), "Le compte dont le Drive porte la sauvegarde de vos compagnies."),
        None => (Pastille::de(EtatDEcran::AFaire, "aucun Drive lié"), "Lier le compte Google qui porte la sauvegarde de vos compagnies."),
    };
    let mut rangs = vec![RangDeLEcran { titre: "Compte Google".to_string(), dit: vec![dit.to_string()], pastille, etat: compte.clone(), montre: None, gestes }];
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
    let en_cours = match reprises_en_cours(inst) {
        Ok((l, illisibles)) => {
            for (nom, e) in illisibles {
                encarts.push(Encart::de(EtatDEcran::Refuse, format!("La reprise commencée dans « {} » ne se lit pas : « Reprendre » la recommence dans un dossier neuf", nom), e.phrase));
            }
            l
        }
        Err(e) => {
            encarts.push(Encart::de(EtatDEcran::Refuse, "Les compagnies de ce poste ne se listent pas : une reprise commencée ne se voit pas", e.phrase));
            Vec::new()
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
        dit.extend(vu.illisible.iter().cloned());
        let commencee = en_cours.iter().find(|(_, _, r)| r.dossier == vu.id);
        let (pastille, boite) = match commencee {
            Some((_, _, r)) => {
                dit.push(format!("Une reprise commencée sur ce poste le {} n'est pas finie : « Reprendre » la continue.", socle::paris::ecrit::saisie::texte(r.depuis)));
                (Pastille::de(EtatDEcran::EnAttente, "reprise pas finie"), format!("La reprise de « {} » continue où elle en était, puis la compagnie s'ouvre.", vu.nom))
            }
            None => (
                Pastille::de(EtatDEcran::Information, "dans le Drive"),
                format!("« {} » revient sur ce poste dans un dossier neuf, puis s'ouvre. Son coffre revient aussi : il s'ouvre avec le mot de passe du logiciel de cette compagnie.", vu.nom),
            ),
        };
        let reprendre = GesteDeRoute::avec_cases(Route::DriveReprendre, DossierDuRang { dossier: Some(vu.id.clone()) }, REPRENDRE, Vec::new(), SansCharge {}, SansCharge {})
            .disant(vec![boite])
            .principal();
        rangs.push(RangDeLEcran { titre: vu.nom.clone(), dit, pastille, etat: None, montre: None, gestes: vec![reprendre] });
    }
    Ok(fini(encarts, rangs, socle::phrase!("Le Drive de {} : {} compagnie(s) à reprendre.", compte, vus.len())))
}

/// LIER LE COMPTE GOOGLE DE L'ACCUEIL (`drive-lier`) : la meme autorisation que celle d'une compagnie, scellee sous
/// l'installation. Aucun mot de passe : il n'y a pas encore de compagnie, et rien ne part de l'accueil.
pub(crate) fn lier(inst: &Path) -> R<DriveLie> {
    let compte = crate::coffre::lier_google(inst)?;
    Ok(DriveLie { phrase: socle::phrase!("Drive lié : {}.", compte), compte })
}

/// OUBLIER LE COMPTE GOOGLE DE L'ACCUEIL (`drive-oublier`) : son jeton s'efface, sans reseau. Les compagnies deja reprises
/// gardent leur copie. L'acces accorde cote Google se retire sur myaccount.google.com.
pub(crate) fn oublier(inst: &Path) -> R<DriveOublie> {
    let p = socle::fichiers::JETON_GOOGLE.dans(inst);
    let efface = socle::fichiers::present(&p)?;
    if efface {
        socle::fichiers::effacer(&p)?;
    }
    let phrase = if efface {
        Phrase::fixe("Compte Google de l'accueil oublié. L'accès accordé côté Google se retire sur myaccount.google.com.")
    } else {
        Phrase::fixe("Aucun compte Google n'était lié à l'accueil : rien n'a changé.")
    };
    Ok(DriveOublie { phrase, efface })
}

/// LA COMPAGNIE OU REVIENT LE DOSSIER `vu` : celle de ce poste dont la reprise de ce dossier n'est pas finie (sa marque le
/// designe) ; sinon un dossier neuf, comme « Ajouter une compagnie » le fait (une fictive ajoutee d'un logiciel bride, une
/// compagnie reelle sinon), ou la reprise pose sa marque avant toute autre chose. Rend son identifiant, sa racine, et si
/// elle est neuve.
fn ou_revient(inst: &Path, vu: &DossierVu, bride: bool) -> R<(IdDeCompagnie, PathBuf, bool)> {
    if let Some((id, racine, _)) = reprises_en_cours(inst)?.0.into_iter().find(|(_, _, r)| r.dossier == vu.id) {
        return Ok((id, racine, false));
    }
    let nom = compagnie::installation::nom_de_dossier(&vu.nom);
    let (id, racine) = if bride {
        IdDeCompagnie::libre(inst, IdDeCompagnie::Fictive, &NomDeFichier::lire(&format!("{}{}", PREFIXE_AJOUTEE, nom))?)?
    } else {
        IdDeCompagnie::libre(inst, IdDeCompagnie::Cie, &NomDeFichier::lire(&nom)?)?
    };
    let config = racine.join(socle::fichiers::CONFIG);
    std::fs::create_dir_all(&config).map_err(|e| socle::echec::du_disque(&config, &e))?;
    compagnie::restauration::commencer_la_reprise(&racine, vu)?;
    Ok((id, racine, true))
}

/// REPRENDRE UNE COMPAGNIE DU DRIVE (`drive-reprendre`) : le dossier, s'il est sous la racine du Drive lie a l'accueil
/// (relue) et se reprend, revient sur ce poste (`ou_revient`), par la restauration. Allee au bout (ni coupee, ni un
/// fichier qui n'est pas revenu), la compagnie recoit le jeton de l'accueil (d'un logiciel bride, elle devient une
/// fictive ajoutee, et rien n'en part), sa marque s'efface, et `ouvrir` la designe. Un refus de la restauration n'ecrit
/// rien : le dossier neuf qu'elle aurait rempli s'efface ; celui d'une reprise relancee reste tel qu'il etait.
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
    let r = match compagnie::restauration::restaurer(&racine, &vu, racine_drive, &mut client) {
        Ok(r) => r,
        Err(e) => {
            if neuve {
                // Rien n'a ete redescendu : le dossier neuf ne porte que la marque de la reprise.
                std::fs::remove_dir_all(&racine).map_err(|x| socle::echec::du_disque(&racine, &x))?;
            }
            return Err(e.echec);
        }
    };
    let au_bout = !r.hors_ligne && r.incidents.is_empty();
    if au_bout {
        if bride {
            let marqueur = Marqueur {
                copie_de: format!("{} ajoutee sur ce poste", MARQUE_FICTIVE),
                fictive: None,
                ajoutee: Some(true),
                quand: Some(socle::temps::secondes().a_la_seconde()),
                quoi: "Une compagnie reprise du Drive sur ce poste, le logiciel etant bride : tout s'y fait, rien n'en part vers un organisme ni vers une personne.".to_string(),
            };
            socle::ecriture::ecrire_json(&racine.join(MARQUEUR), &marqueur, 2)?;
        } else {
            // Le jeton de l'accueil, tel qu'il est scelle : la compagnie reprise est liee au meme compte.
            let jeton = socle::fichiers::JETON_GOOGLE.dans(inst);
            let le_sien = socle::fichiers::JETON_GOOGLE.dans(&racine);
            if !socle::fichiers::present(&le_sien)? {
                std::fs::copy(&jeton, &le_sien).map_err(|e| socle::echec::du_disque(&jeton, &e))?;
            }
        }
        compagnie::restauration::finir_la_reprise(&racine)?;
    }
    let a_faire = compagnie::restauration::a_faire(&compagnie::coffre::Coffre::du_poste(&racine));
    let nom = compagnie::installation::nom_de(&racine)?;
    let revenus = r.redescendus.len();
    let phrase = if au_bout {
        socle::phrase!("« {} » est reprise de votre Drive : {} fichier(s) revenu(s). Elle s'ouvre.", nom, revenus)
    } else if r.hors_ligne {
        socle::phrase!("« {} » : {} fichier(s) revenu(s), puis la connexion est tombée. « Reprendre » de nouveau continue où elle en était.", nom, revenus)
    } else {
        socle::phrase!(
            "« {} » : {} fichier(s) revenu(s), {} ne sont pas revenus. « Reprendre » de nouveau les redemande ; elle s'ouvre quand tout est revenu.",
            nom,
            revenus,
            r.incidents.len()
        )
    };
    Ok(Reprise { phrase, ouvrir: au_bout.then_some(id), nom, chemin: racine, redescendus: revenus, incidents: r.incidents, a_faire })
}
