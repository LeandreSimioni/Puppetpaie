//! REPRENDRE UNE COMPAGNIE DE SON DRIVE, A L'ACCUEIL (le « + » : « De mon Drive ») : lier un compte Google a l'accueil
//! (`drive-lier`) ou l'oublier (`drive-oublier`), voir ce qu'il porte (`drive-compagnies`, l'ecran de
//! `Place::AccueilDrive`), ramener une compagnie sur ce poste (`drive-reprendre`) comme une compagnie neuve de
//! l'installation, par la restauration (`compagnie::restauration`, le seul chemin, que `moteur restaurer` prend aussi), et
//! l'ouvrir (`compagnie-ouvrir`). Ces routes sont celles de l'installation (`accueil::DE_L_INSTALLATION`), a leur place.
//!
//! LE COMPTE GOOGLE DE L'ACCUEIL. Le jeton d'une compagnie vit sous elle (`config/google-jeton.bin`), scelle par la cle
//! du poste (`routes::coffre`) ; avant qu'il y ait une compagnie, il vit de meme sous l'installation
//! (`<installation>/config/google-jeton.bin`), scelle par la meme cle : la meme autorisation (`coffre::lier_google`), le
//! meme client (`coffre::client_drive`). Rien de ce jeton n'entre dans une sauvegarde (`sauvegarde::ecarte`).
//!
//! UNE REPRISE NE VA JAMAIS DANS UNE COMPAGNIE QUI EXISTE : elle cree un dossier neuf, y pose d'abord sa marque
//! (`restauration::RepriseEnCours`), puis la restauration le remplit. Tant que la marque est la, ce dossier n'est pas une
//! compagnie du poste (`installation::liste_de` l'ecarte : ni tuile, ni ouverte) ; les reprises commencees se cherchent par
//! leur marque (`installation::reprises_commencees`). Coupee (le reseau), ou sans sa fiche Entreprise revenue, elle se
//! continue dans ce dossier-la, et dans aucun autre. Allee au bout (tous les fichiers tentes, la fiche revenue), la
//! compagnie recoit une copie du jeton de l'accueil (elle est liee au meme compte, et sa sauvegarde repart dans le meme
//! dossier du Drive, qui ne perd rien : elle n'y supprime jamais), sa marque s'efface, et l'ecran de fin de reprise
//! propose de l'ouvrir (`compagnie-ouvrir`) : rien ne l'ouvre avant ce geste.
//!
//! CE QUE L'ON FAIT D'UN DOSSIER DU DRIVE (`ce_qu_on_en_fait`), une seule verite pour l'ecran et pour le geste : deja sur
//! ce poste (une compagnie porte son identite), une reprise a continuer, ou une reprise neuve.

use std::path::{Path, PathBuf};

use serde::Serialize;

use compagnie::installation::{IdDeCompagnie, Marqueur, MARQUEUR, MARQUE_FICTIVE, PREFIXE_AJOUTEE};
use compagnie::restauration::RepriseEnCours;
use compagnie::sauvegarde::IdentiteSurLeDrive;
use compagnie::sur_le_drive::{DossierVu, GenreDuDossier};
use envois::drive::IdDrive;
use socle::echec::{erreur, Classe, R};
use socle::ecran::{EtatDEcran, Pastille};
use socle::fichiers::NomDeFichier;

use crate::catalogue::{Acte, Affiche, Ecran, Fiche, Place, Refus, Route, MANQUE, OPERATION};
use crate::declaration::{charge, dit};
use crate::ecran::{EcranRendu, RangDeLEcran};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Rendu};
use crate::imprevus::GesteDeRoute;
use crate::{Encart, SansCharge};

/// Les routes de ce module, et leurs fiches, dans l'ordre ou leurs exemples se jouent : lier le compte, reprendre une
/// compagnie, l'ouvrir, puis oublier le compte.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::DriveCompagnies,
        geste: "Dire les compagnies que le Drive lié à l'accueil porte, et le geste qui reprend chacune.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "phrase, encarts, rangs : le compte Google de l'accueil (le lier, l'oublier), puis chaque compagnie du Drive (sa dernière sauvegarde, son volume, une reprise pas finie sur ce poste, ou la compagnie de ce poste qui l'est déjà) et le geste qui la reprend ; ce qui ne se lit pas se dit en encart",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Lit,
        phrase: 120,
        ecran: Ecran::Rendu(Place::AccueilDrive),
    },
    Fiche {
        route: Route::DriveLier,
        geste: "Lier un compte Google à l'accueil, pour reprendre une compagnie de son Drive.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "compte : l'adresse du compte Google lié, phrase ; la fenêtre de Google s'ouvre dans le navigateur, son adresse est dite sur la sortie d'erreur",
        refus: &[Refus { classe: Classe::Refuse, corriger: "ce que la phrase dit (Google a refusé, aucune réponse en cinq minutes, l'application absente) ; rien n'est enregistré" }],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 120,
        ecran: Ecran::Geste(Place::AccueilDrive),
    },
    Fiche {
        route: Route::DriveReprendre,
        geste: "Ramener sur ce poste une compagnie du Drive lié à l'accueil, dans un dossier neuf (une reprise pas finie se continue dans le sien).",
        charge: &ChargeReprendre::CHARGE,
        exemple: r#"{"dossier":"cie-essai"}"#,
        rend: "l'écran de fin de reprise (phrase, encarts, rangs : ce qui est revenu, ce qui reste dans le Drive, ce qui reste à faire pour son coffre, et le geste qui l'ouvre, ou celui qui continue une reprise pas finie), id, nom, chemin (le --racine de ses routes), au_bout, redescendus, incidents (le détail de ce qui n'est pas revenu, un fichier par ligne)",
        refus: &[
            Refus { classe: Classe::Refuse, corriger: "lier d'abord le Drive, ou un dossier que drive-compagnies montre et qui n'est pas déjà sur ce poste (le refus nomme la compagnie) ; rien n'est repris" },
            Refus { classe: Classe::DossierNonVide, corriger: "la compagnie d'une reprise pas finie a changé depuis : mettre ses fichiers de côté ; rien n'est écrit" },
            OPERATION,
        ],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 200,
        ecran: Ecran::Geste(Place::AccueilDrive),
    },
    Fiche {
        route: Route::CompagnieOuvrir,
        geste: "Désigner une compagnie de ce poste à ouvrir : la fenêtre l'ouvre.",
        charge: &ChargeOuvrir::CHARGE,
        exemple: r#"{"compagnie":"fictive:ajoutee-la-reprise-d-essai"}"#,
        rend: "ouvrir (la compagnie, que la fenêtre ouvre), nom, chemin (le --racine de ses routes), phrase",
        refus: &[MANQUE, Refus { classe: Classe::Refuse, corriger: "une compagnie que compagnies montre (une reprise pas finie n'en est pas encore une) ; rien n'est ouvert" }],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Lit,
        phrase: 120,
        ecran: Ecran::Geste(Place::AccueilDrive),
    },
    Fiche {
        route: Route::DriveOublier,
        geste: "Oublier le compte Google de l'accueil : son jeton s'efface de ce poste.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "efface (un jeton s'est effacé), phrase",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 120,
        ecran: Ecran::Geste(Place::AccueilDrive),
    },
];

const LIER: Affiche = Affiche::new("Lier le Drive");
const RELIER: Affiche = Affiche::new("Relier le Drive");
const OUBLIER: Affiche = Affiche::new("Oublier ce compte");
const REPRENDRE: Affiche = Affiche::new("Reprendre");
const OUVRIR: Affiche = Affiche::new("Ouvrir la compagnie");

charge! {
    /// La charge de `drive-reprendre` : le dossier du Drive, tel que `drive-compagnies` le designe. Le geste « Reprendre »
    /// d'un rang la porte telle quelle.
    #[derive(Debug, Clone, Serialize)]
    pub struct ChargeReprendre {
        pub dossier: IdDrive = dit("Dossier du Drive", "l'identifiant du dossier, tel que drive-compagnies le rend dans la charge de son geste"),
    }
}

charge! {
    /// La charge de `compagnie-ouvrir` : la compagnie, telle que `compagnies` ou l'ecran de fin de reprise la designe.
    #[derive(Debug, Clone, Serialize)]
    pub struct ChargeOuvrir {
        pub compagnie: IdDeCompagnie = dit("Compagnie", "l'identifiant de la compagnie, tel que compagnies le rend"),
    }
}

/// CE QU'UN GESTE D'UN ECRAN DE LA REPRISE PORTE : la charge de sa route, rien de plus. Lier ou oublier le compte Google
/// de l'accueil n'ont rien a designer ; reprendre porte un dossier du Drive ; ouvrir, une compagnie de ce poste.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum GesteDeLaReprise {
    Compte(SansCharge),
    Reprendre(ChargeReprendre),
    Ouvrir(ChargeOuvrir),
}

type EcranDeLaReprise = EcranRendu<GesteDeLaReprise>;

/// Ce que rend `drive-compagnies` : sa phrase, et l'ecran de la reprise (le compte Google, puis chaque compagnie du Drive).
#[derive(Debug, Serialize)]
pub(crate) struct CompagniesDuDrive {
    phrase: Phrase,
    #[serde(flatten)]
    ecran: EcranDeLaReprise,
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

/// Ce que rend `drive-reprendre` : l'ecran de fin de reprise (ce qui est revenu, ce qui reste dans le Drive, ce qui reste
/// a faire pour le coffre, et le geste qui ouvre la compagnie, ou celui qui continue une reprise pas finie) ; puis, pour
/// qui lit la reponse, la compagnie (son identifiant, son nom, son dossier sur ce poste), si la reprise est allee au bout,
/// le nombre de fichiers revenus, et le detail de ce qui ne l'est pas (`incidents`, un fichier par ligne, chemins et
/// empreintes compris : l'ecran ne le montre pas, il en dit le nombre).
#[derive(Debug, Serialize)]
pub(crate) struct Reprise {
    phrase: Phrase,
    #[serde(flatten)]
    ecran: EcranDeLaReprise,
    id: IdDeCompagnie,
    nom: String,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    chemin: PathBuf,
    au_bout: bool,
    redescendus: usize,
    incidents: Vec<String>,
}

/// Ce que rend `compagnie-ouvrir` : la compagnie que la fenetre ouvre, et son dossier (le `--racine` de ses routes).
#[derive(Debug, Serialize)]
pub(crate) struct AOuvrir {
    phrase: Phrase,
    ouvrir: IdDeCompagnie,
    nom: String,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    chemin: PathBuf,
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

impl Dite for AOuvrir {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// Une route de ce module, branchee sur l'enveloppe, dans l'installation `inst` (`accueil::route`).
pub(crate) fn route(inst: &Path, nom: Route, charge: &Charge) -> R<Rendu> {
    match nom {
        Route::DriveCompagnies => {
            let SansCharge {} = charge.lire()?;
            en_json(compagnies(inst)?)
        }
        Route::DriveLier => {
            let SansCharge {} = charge.lire()?;
            en_json(lier(inst)?)
        }
        Route::DriveOublier => {
            let SansCharge {} = charge.lire()?;
            en_json(oublier(inst)?)
        }
        Route::DriveReprendre => {
            let c: ChargeReprendre = charge.lire()?;
            en_json(reprendre(inst, &c.dossier)?)
        }
        Route::CompagnieOuvrir => {
            let c: ChargeOuvrir = charge.lire()?;
            en_json(ouvrir(inst, c.compagnie)?)
        }
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

/// Ce que dit la boite d'« Oublier ce compte ».
const OUBLIER_DIT: &str = "Ce poste oublie le compte Google de l'accueil. Les compagnies déjà reprises gardent le leur. L'accès accordé côté Google se retire sur myaccount.google.com.";
/// Ce que dit l'ecran de fin d'une reprise allee au bout, du dossier du Drive qu'elle garde.
const UN_SEUL_POSTE: &str = "Si l'ancien poste se sert encore de cette compagnie, n'en gardez qu'un : les deux sauvegardes iraient au même dossier.";

/// Une reprise pas finie sur ce poste : la compagnie qu'elle remplit, sa racine, sa marque.
type Commencee = (IdDeCompagnie, PathBuf, RepriseEnCours);

/// LES REPRISES PAS FINIES DE L'INSTALLATION `inst`, cherchees par leur marque (`installation::reprises_commencees`),
/// chacune avec sa marque ; et, a part, le nom du dossier de chacune dont la marque ne se lit pas. Une marque illisible
/// ne designe aucun dossier du Drive : aucune reprise ne la continue, et l'ecran le dit.
fn reprises_en_cours(inst: &Path) -> R<(Vec<Commencee>, Vec<String>)> {
    let (mut lues, mut illisibles) = (Vec::new(), Vec::new());
    for (id, racine) in compagnie::installation::reprises_commencees(inst)? {
        match compagnie::restauration::reprise_en_cours(&racine) {
            Ok(Some(r)) => lues.push((id, racine, r)),
            Ok(None) => {}
            Err(_) => illisibles.push(compagnie::installation::nom_dossier(&racine)?.to_string()),
        }
    }
    Ok((lues, illisibles))
}

/// Une compagnie de ce poste : son nom, et son identite sur le Drive (aucune : elle ne s'est jamais sauvegardee), ou
/// pourquoi elle ne se lit pas.
type IdentiteDuPoste = (String, R<Option<IdentiteSurLeDrive>>);

/// L'IDENTITE SUR LE DRIVE DE CHAQUE COMPAGNIE DE L'INSTALLATION `inst` (`config/drive.json`) : une qui ne se lit pas
/// ne ferme rien, elle se dit. Un dossier qu'une reprise remplit encore n'en est pas (`installation::liste_de`).
fn identites_du_poste(inst: &Path) -> R<Vec<IdentiteDuPoste>> {
    Ok(compagnie::installation::liste_de(inst)?.into_iter().map(|c| {
        let i = compagnie::sauvegarde::identite_ecrite(&c.racine);
        (c.nom, i)
    }).collect())
}

/// CE QUE L'ON FAIT DU DOSSIER DU DRIVE `vu`, la meme reponse pour l'ecran (`compagnies`) et pour le geste (`reprendre`).
enum CeQuOnEnFait<'a> {
    /// Une compagnie de ce poste porte son identite : rien n'est repris ; leurs noms, et celles dont l'identite ne se lit
    /// pas qui portent son nom (elles le sont peut-etre aussi).
    DejaLa { noms: Vec<&'a str>, douteuses: Vec<&'a str> },
    /// Une reprise de ce dossier, commencee sur ce poste, se continue dans le sien.
    Continuer(&'a Commencee),
    /// Une reprise neuve, dans un dossier neuf.
    Neuve,
}

/// `ce_qu_on_en_fait` : une compagnie du poste qui porte l'identite du dossier l'emporte (une reprise commencee du meme
/// dossier reste alors de cote) ; sinon la premiere reprise commencee de ce dossier ; sinon une reprise neuve.
fn ce_qu_on_en_fait<'a>(du_poste: &'a [IdentiteDuPoste], en_cours: &'a [Commencee], vu: &DossierVu) -> CeQuOnEnFait<'a> {
    if let GenreDuDossier::Compagnie(identite) = &vu.genre {
        let noms: Vec<&str> = du_poste.iter().filter(|(_, i)| matches!(i, Ok(Some(j)) if j == identite)).map(|(n, _)| n.as_str()).collect();
        if !noms.is_empty() {
            let douteuses = du_poste.iter().filter(|(n, i)| i.is_err() && *n == vu.nom).map(|(n, _)| n.as_str()).collect();
            return CeQuOnEnFait::DejaLa { noms, douteuses };
        }
    }
    match en_cours.iter().find(|(_, _, r)| r.dossier == vu.id) {
        Some(c) => CeQuOnEnFait::Continuer(c),
        None => CeQuOnEnFait::Neuve,
    }
}

/// La phrase qui nomme les compagnies de ce poste qui sont deja celle d'un dossier du Drive.
fn deja_sur_ce_poste(noms: &[&str], douteuses: &[&str]) -> String {
    let mut p = format!("Cette compagnie est déjà sur ce poste : « {} ».", noms.join(" », « "));
    for d in douteuses {
        p.push_str(&format!(" « {} » aussi peut-être : son dossier du Drive ne se reconnaît pas.", d));
    }
    p
}

/// Le geste « Reprendre » du dossier `dossier`, et ce que sa boite dit.
fn geste_reprendre(dossier: &IdDrive, boite: String) -> GesteDeRoute<GesteDeLaReprise, SansCharge, SansCharge> {
    GesteDeRoute::avec_cases(Route::DriveReprendre, GesteDeLaReprise::Reprendre(ChargeReprendre { dossier: dossier.clone() }), REPRENDRE, Vec::new(), SansCharge {}, SansCharge {})
        .disant(vec![boite])
        .principal()
}

/// L'ECRAN DE LA REPRISE (`drive-compagnies`) : le compte Google de l'accueil (le lier, le relier, l'oublier), puis chaque
/// dossier du Drive qui se reprend (le dossier d'une compagnie, l'ancienne sauvegarde commune), sa derniere sauvegarde,
/// son volume, ce qu'on en fait (`ce_qu_on_en_fait`) et son geste. Doux : sans compte lie, un jeton ou un Drive qui ne se
/// lit pas, des compagnies du poste qui ne se listent pas, cela se dit en encart, en mots.
pub(crate) fn compagnies(inst: &Path) -> R<CompagniesDuDrive> {
    let mut encarts = vec![Encart::de(EtatDEcran::Information, "", "Une compagnie sauvegardée dans votre Drive revient sur ce poste, telle qu'elle y est, dans un dossier neuf.")];
    if compagnie::bridage::est_bride_de(inst) {
        encarts.push(Encart::de(EtatDEcran::Information, "Logiciel en démo", "La compagnie reprise sera une compagnie fictive : on y regarde tout, rien n'en part."));
    }
    let lu = crate::coffre::compte_autorise(inst);
    let lier = |libelle| GesteDeRoute::avec_cases(Route::DriveLier, GesteDeLaReprise::Compte(SansCharge {}), libelle, Vec::new(), SansCharge {}, SansCharge {}).disant(crate::coffre::ouvrir_google_dit());
    let oublier = || GesteDeRoute::avec_cases(Route::DriveOublier, GesteDeLaReprise::Compte(SansCharge {}), OUBLIER, Vec::new(), SansCharge {}, SansCharge {}).disant(vec![OUBLIER_DIT.to_string()]);
    // `compte` : l'adresse du compte lie, ou la phrase de la route quand aucun ne se lit.
    let (pastille, dit, gestes, compte) = match lu {
        Ok(Some(c)) => (Pastille::de(EtatDEcran::Fait, "lié"), "Le compte dont le Drive porte la sauvegarde de vos compagnies.", vec![lier(RELIER), oublier()], Ok(c)),
        Ok(None) => (Pastille::de(EtatDEcran::AFaire, "aucun Drive lié"), "Lier le compte Google qui porte la sauvegarde de vos compagnies.", vec![lier(LIER).principal()], Err("Aucun compte Google lié : le lier d'abord.")),
        // Un jeton qui ne se lit pas n'est pas « aucun Drive lié » : il se dit, se relie ou s'oublie.
        Err(_) => (
            Pastille::de(EtatDEcran::Refuse, "illisible"),
            "Le jeton de ce compte Google ne se lit pas sur ce poste : le relier.",
            vec![lier(RELIER).principal(), oublier()],
            Err("Le jeton du compte Google ne se lit pas : le relier."),
        ),
    };
    let mut rangs = vec![RangDeLEcran { titre: "Compte Google".to_string(), dit: vec![dit.to_string()], pastille, etat: compte.clone().ok(), montre: None, gestes }];
    let fini = |encarts, rangs, phrase| CompagniesDuDrive { phrase, ecran: EcranRendu { encarts, rangs, cases: Vec::new(), historique: Vec::new(), enregistrer: None } };
    let compte = match compte {
        Ok(c) => c,
        Err(sans_compte) => return Ok(fini(encarts, rangs, Phrase::fixe(sans_compte))),
    };
    let vus = crate::coffre::client_drive(inst).and_then(|mut c| compagnie::sur_le_drive::voir(&mut c, compagnie::sauvegarde::RACINE_DRIVE));
    let vus: Vec<DossierVu> = match vus {
        Ok(v) => v.into_iter().filter(DossierVu::se_reprend).collect(),
        Err(e) => {
            encarts.push(Encart::de(EtatDEcran::Refuse, "Le Drive ne se lit pas", e.echec.phrase));
            return Ok(fini(encarts, rangs, socle::phrase!("Le Drive de {} ne se lit pas.", compte)));
        }
    };
    // Ce qui ne se lit pas sur ce poste se dit en mots (une erreur du disque porte un chemin), et ne ferme rien.
    let en_cours = match reprises_en_cours(inst) {
        Ok((l, illisibles)) => {
            for nom in illisibles {
                encarts.push(Encart::de(
                    EtatDEcran::Refuse,
                    format!("Une reprise commencée dans « {} » ne se lit pas", nom),
                    "Ce dossier reste de côté sur ce poste, rien ne l'ouvre. « Reprendre » ramène la compagnie dans un dossier neuf.",
                ));
            }
            l
        }
        Err(_) => {
            encarts.push(Encart::de(EtatDEcran::Refuse, "Les dossiers de ce poste ne se lisent pas", "Une reprise commencée ici ne se voit pas : « Reprendre » en commence une neuve."));
            Vec::new()
        }
    };
    let du_poste = match identites_du_poste(inst) {
        Ok(l) => l,
        Err(_) => {
            encarts.push(Encart::de(EtatDEcran::Refuse, "Les compagnies de ce poste ne se lisent pas", "Celles déjà reprises ne se voient pas ici."));
            Vec::new()
        }
    };
    for (nom, i) in &du_poste {
        if i.is_err() {
            encarts.push(Encart::de(EtatDEcran::Refuse, format!("« {} » : son dossier du Drive ne se reconnaît pas", nom), "Sa marque sur ce poste ne se lit pas : elle a été changée à la main."));
        }
    }
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
        let (pastille, gestes) = match ce_qu_on_en_fait(&du_poste, &en_cours, vu) {
            // DEJA SUR CE POSTE : la reprise se refuserait ; le geste ne se propose pas, le rang dit ou elle est.
            CeQuOnEnFait::DejaLa { noms, douteuses } => {
                dit.push(deja_sur_ce_poste(&noms, &douteuses));
                if en_cours.iter().any(|(_, _, r)| r.dossier == vu.id) {
                    dit.push("Une reprise de ce dossier commencée sur ce poste reste de côté, inachevée.".to_string());
                }
                (Pastille::de(EtatDEcran::Fait, "déjà sur ce poste"), Vec::new())
            }
            CeQuOnEnFait::Continuer((_, _, r)) => {
                dit.push(format!("Une reprise commencée sur ce poste le {} n'est pas finie : « Reprendre » la continue.", socle::paris::ecrit::saisie::texte(r.depuis)));
                let boite = format!("La reprise de « {} » continue où elle en était.", vu.nom);
                (Pastille::de(EtatDEcran::EnAttente, "reprise pas finie"), vec![geste_reprendre(&vu.id, boite)])
            }
            CeQuOnEnFait::Neuve => {
                let boite = format!("« {} » revient sur ce poste dans un dossier neuf. Son coffre revient aussi ; l'écran de fin de reprise dit ce qu'il reste à faire pour lui.", vu.nom);
                (Pastille::de(EtatDEcran::Information, "dans le Drive"), vec![geste_reprendre(&vu.id, boite)])
            }
        };
        rangs.push(RangDeLEcran { titre: vu.nom.clone(), dit, pastille, etat: None, montre: None, gestes });
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

/// LA COMPAGNIE OU REVIENT LE DOSSIER `vu` : celle de la reprise commencee de ce dossier (`commencee`) ; sinon un dossier
/// neuf, comme « Ajouter une compagnie » le fait (une fictive ajoutee d'un logiciel bride, une compagnie reelle sinon), ou
/// la reprise pose sa marque avant toute autre chose. Rend son identifiant, sa racine, et si elle est neuve.
fn ou_revient(inst: &Path, vu: &DossierVu, bride: bool, commencee: Option<&Commencee>) -> R<(IdDeCompagnie, PathBuf, bool)> {
    if let Some((id, racine, _)) = commencee {
        return Ok((id.clone(), racine.clone(), false));
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
/// (relue) et se reprend, revient sur ce poste (`ou_revient`), par la restauration ; une compagnie deja sur ce poste la
/// refuse (`ce_qu_on_en_fait`). ALLEE AU BOUT : la connexion n'est pas tombee (`Restauration::hors_ligne`, que la panne
/// dit, jamais sa phrase) et la fiche Entreprise est revenue (sans elle, ce ne serait pas une compagnie) ; alors la
/// compagnie recoit le jeton de l'accueil (d'un logiciel bride, elle devient une fictive ajoutee, et rien n'en part), son
/// coffre se range s'il ne s'ouvrira pas ici (`restauration::a_faire`), sa marque s'efface, et l'ecran de fin de reprise
/// propose de l'ouvrir. Sinon la marque reste, et l'ecran propose de continuer. Un refus de la restauration n'ecrit rien :
/// le dossier neuf qu'elle aurait rempli s'efface ; celui d'une reprise continuee reste tel qu'il etait.
pub(crate) fn reprendre(inst: &Path, dossier: &IdDrive) -> R<Reprise> {
    if !crate::coffre::autorise(inst) {
        return Err(erreur(Classe::Refuse, "Aucun compte Google lisible n'est lié à l'accueil : le lier d'abord (« Lier le Drive »). Rien n'est repris."));
    }
    let racine_drive = compagnie::sauvegarde::RACINE_DRIVE;
    let mut client = crate::coffre::client_drive(inst).map_err(|e| e.echec)?;
    let vus = compagnie::sur_le_drive::voir(&mut client, racine_drive).map_err(|e| e.echec)?;
    let vu = compagnie::sur_le_drive::le_dossier(vus, dossier, racine_drive).map_err(|e| e.echec)?;
    if !vu.se_reprend() {
        return Err(erreur(Classe::Refuse, format!("« {} » n'est pas la sauvegarde d'une compagnie : rien n'est repris.", vu.nom)));
    }
    let (en_cours, _) = reprises_en_cours(inst)?;
    let du_poste = identites_du_poste(inst)?;
    let commencee = match ce_qu_on_en_fait(&du_poste, &en_cours, &vu) {
        CeQuOnEnFait::DejaLa { noms, douteuses } => {
            return Err(erreur(Classe::Refuse, format!("{} Rien n'est repris.", deja_sur_ce_poste(&noms, &douteuses))));
        }
        CeQuOnEnFait::Continuer(c) => Some(c),
        CeQuOnEnFait::Neuve => None,
    };
    let bride = compagnie::bridage::est_bride_de(inst);
    let (id, racine, neuve) = ou_revient(inst, &vu, bride, commencee)?;
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
    let fiche = droit::contexte::ENTREPRISE.dans(&racine).is_file();
    let au_bout = !r.hors_ligne && fiche;
    let mut a_faire = Vec::new();
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
            // Le coffre revenu : ce qu'il reste a faire pour lui, apres l'avoir mis de cote s'il ne s'ouvrira pas ici.
            a_faire = compagnie::restauration::a_faire(&compagnie::coffre::Coffre::du_poste(&racine));
        }
        compagnie::restauration::finir_la_reprise(&racine)?;
    }
    let nom = if fiche { compagnie::installation::nom_de(&racine)? } else { vu.nom.clone() };
    let revenus = r.redescendus.len();
    let ecran = fin_de_reprise(&vu, &id, &nom, au_bout, bride, &r, a_faire);
    let phrase = if au_bout {
        socle::phrase!("« {} » est reprise de votre Drive : {} fichier(s) revenu(s).", nom, revenus)
    } else {
        socle::phrase!("La reprise de « {} » n'est pas finie : {} fichier(s) revenu(s).", nom, revenus)
    };
    Ok(Reprise { phrase, ecran, id, nom, chemin: racine, au_bout, redescendus: revenus, incidents: r.incidents })
}

/// L'ECRAN DE FIN DE REPRISE : ce qui est revenu ; ce qui n'a pas pu revenir et reste dans le Drive (en mots, le detail
/// dans `Reprise::incidents`) ; pourquoi elle n'est pas finie (la connexion tombee, la fiche Entreprise absente) et le
/// geste qui la continue ; allee au bout, ce qui reste a faire pour son coffre (`a_faire`, une compagnie reelle), le
/// dossier du Drive qu'elle garde avec l'ancien poste, et le geste qui l'ouvre.
fn fin_de_reprise(vu: &DossierVu, id: &IdDeCompagnie, nom: &str, au_bout: bool, bride: bool, r: &compagnie::restauration::Restauration, a_faire: Vec<String>) -> EcranDeLaReprise {
    let mut encarts = Vec::new();
    if au_bout {
        encarts.push(Encart::de(EtatDEcran::Fait, "Revenue sur ce poste", format!("{} fichier(s) de sa sauvegarde sont revenus dans un dossier neuf.", r.redescendus.len())));
    } else if r.hors_ligne {
        encarts.push(Encart::de(EtatDEcran::AFaire, "La connexion est tombée", format!("{} fichier(s) sont revenus, puis la connexion est tombée. « Reprendre » continue où elle en était.", r.redescendus.len())));
    } else {
        encarts.push(Encart::de(EtatDEcran::AFaire, "Sa fiche Entreprise n'est pas revenue", "Sans elle, la compagnie ne s'ouvre pas. Ce qui est revenu reste sur ce poste ; « Reprendre » retente ce qui manque."));
    }
    if !r.incidents.is_empty() {
        encarts.push(Encart::de(
            EtatDEcran::AFaire,
            "Ce qui n'a pas pu revenir",
            format!("{} fichier(s) n'ont pas pu revenir : ils restent dans le Drive, rien n'y est effacé.", r.incidents.len()),
        ));
    }
    let mut rangs = Vec::new();
    if au_bout {
        // Reprise du dossier d'une compagnie, elle en garde l'identite : sa sauvegarde ira dans ce meme dossier. Reprise de
        // l'ancienne sauvegarde commune, sa premiere sauvegarde fera le sien.
        if !bride && matches!(vu.genre, GenreDuDossier::Compagnie(_)) {
            encarts.push(Encart::de(EtatDEcran::Information, "Un seul poste", UN_SEUL_POSTE));
        }
        if !a_faire.is_empty() {
            rangs.push(RangDeLEcran { titre: "Son coffre des mots de passe".to_string(), dit: a_faire, pastille: Pastille::de(EtatDEcran::AFaire, "à faire"), etat: None, montre: None, gestes: Vec::new() });
        }
        let ouvrir = GesteDeRoute::avec_cases(Route::CompagnieOuvrir, GesteDeLaReprise::Ouvrir(ChargeOuvrir { compagnie: id.clone() }), OUVRIR, Vec::new(), SansCharge {}, SansCharge {}).principal();
        rangs.push(RangDeLEcran { titre: nom.to_string(), dit: vec!["Reprise de votre Drive.".to_string()], pastille: Pastille::de(EtatDEcran::Fait, "sur ce poste"), etat: None, montre: None, gestes: vec![ouvrir] });
    } else {
        let boite = format!("La reprise de « {} » continue où elle en était.", vu.nom);
        rangs.push(RangDeLEcran { titre: nom.to_string(), dit: vec!["Reprise pas finie : la compagnie ne s'ouvre pas encore.".to_string()], pastille: Pastille::de(EtatDEcran::EnAttente, "reprise pas finie"), etat: None, montre: None, gestes: vec![geste_reprendre(&vu.id, boite)] });
    }
    EcranRendu { encarts, rangs, cases: Vec::new(), historique: Vec::new(), enregistrer: None }
}

/// OUVRIR UNE COMPAGNIE DE CE POSTE (`compagnie-ouvrir`) : une tuile de l'accueil seulement (`installation::racine_choisie`
/// : une reprise pas finie n'en est pas une), que la fenetre ouvre (`ouvrir`) ; en ligne de commande, son `chemin` est le
/// `--racine` de ses routes. Ne change rien.
pub(crate) fn ouvrir(inst: &Path, id: IdDeCompagnie) -> R<AOuvrir> {
    let racine = compagnie::installation::racine_choisie(inst, &id).map_err(|e| e.reclasser(Classe::Refuse))?;
    let nom = compagnie::installation::nom_de(&racine)?;
    Ok(AOuvrir { phrase: socle::phrase!("« {} » s'ouvre.", nom), ouvrir: id, nom, chemin: racine })
}
