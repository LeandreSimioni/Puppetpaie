//! REPRENDRE UNE COMPAGNIE DE SON DRIVE, A L'ACCUEIL (le « + » : « De mon Drive ») : lier un compte Google a l'accueil
//! (`drive-lier`) ou l'oublier (`drive-oublier`), voir ce qu'il porte (`drive-compagnies`, l'ecran de
//! `Place::AccueilDrive`), ramener une compagnie sur ce poste (`drive-reprendre`) comme une compagnie neuve de
//! l'installation, par la restauration (`compagnie::restauration`, le seul chemin, que `moteur restaurer` prend aussi),
//! l'ouvrir (`compagnie-ouvrir`), ou abandonner une reprise pas finie (`drive-abandonner`). Ces routes sont celles de
//! l'installation (`accueil::DE_L_INSTALLATION`), a leur place. Chaque geste dit ce que la page fait de sa reponse
//! (`ecran::ApresLeGeste`).
//!
//! LE COMPTE GOOGLE DE L'ACCUEIL. Le jeton d'une compagnie vit sous elle (`config/google-jeton.bin`), scelle par la cle
//! du poste (`routes::coffre`) ; avant qu'il y ait une compagnie, il vit de meme sous l'installation
//! (`<installation>/config/google-jeton.bin`), scelle par la meme cle : la meme autorisation (`coffre::lier_google`), le
//! meme client (`coffre::client_drive`). Rien de ce jeton n'entre dans une sauvegarde (`sauvegarde::ecarte`).
//!
//! UNE REPRISE NE VA JAMAIS DANS UNE COMPAGNIE QUI EXISTE : elle cree un dossier neuf, y pose d'abord sa marque
//! (`restauration::RepriseEnCours`), puis la restauration le remplit. Tant que la marque est la, ce dossier n'est pas une
//! compagnie du poste (`installation::liste_de` l'ecarte : ni tuile, ni ouverte) ; les reprises commencees se cherchent par
//! leur marque (`installation::reprises_commencees`), et aucune route ne s'y sert (`service::contexte`). Coupee (le
//! reseau, un fichier a retenter), elle se continue dans ce dossier-la, et dans aucun autre. Allee au bout (chaque
//! fichier descendu, sauf ceux qui ne descendront jamais par cette voie, et la fiche Entreprise revenue et lisible), la
//! compagnie recoit une copie du jeton de l'accueil (elle est liee au meme compte, et sa sauvegarde repart dans le meme
//! dossier du Drive, qui ne perd rien : elle n'y supprime jamais), sa marque s'efface, et l'ecran de fin de reprise
//! propose de l'ouvrir (`compagnie-ouvrir`) : rien ne l'ouvre avant ce geste.
//!
//! UNE REPRISE PAS FINIE SE VOIT TOUJOURS (`drive-compagnies`), que son dossier du Drive se voie ou non (supprime, a la
//! corbeille, sur un autre compte, un Drive qui ne se lit pas, une marque illisible), avec le geste qui l'abandonne : son
//! dossier sur ce poste s'efface, ce qui ne perd rien, puisque tout ce qu'il porte vient du Drive et y reste.
//!
//! CE QUE L'ON FAIT D'UN DOSSIER DU DRIVE (`ce_qu_on_en_fait`), une seule verite pour l'ecran et pour le geste : deja sur
//! ce poste (une compagnie porte son identite), une reprise a continuer, ou une reprise neuve.

use std::path::{Path, PathBuf};

use serde::Serialize;

use compagnie::installation::{IdDeCompagnie, Marqueur, MARQUEUR, MARQUE_FICTIVE, PREFIXE_AJOUTEE};
use compagnie::restauration::{ArretDeLaRestauration, RepriseEnCours};
use compagnie::sauvegarde::IdentiteSurLeDrive;
use compagnie::sur_le_drive::{DossierVu, GenreDuDossier};
use envois::drive::IdDrive;
use socle::echec::{erreur, Classe, R};
use socle::ecran::{EtatDEcran, Pastille};
use socle::fichiers::NomDeFichier;

use crate::catalogue::{Acte, Affiche, Ecran, Fiche, Place, Refus, Route, MANQUE, OPERATION};
use crate::declaration::{charge, dit};
use crate::ecran::{ApresLeGeste, EcranRendu, RangDeLEcran};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Rendu};
use crate::imprevus::GesteDeRoute;
use crate::{Encart, SansCharge};

/// Les routes de ce module, et leurs fiches, dans l'ordre ou leurs exemples se jouent : lier le compte, reprendre une
/// compagnie, l'ouvrir, puis oublier le compte.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::DriveCompagnies,
        geste: "Dire les compagnies du Drive lié à l'accueil, et le geste qui reprend chacune.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "phrase, encarts, rangs : le compte Google de l'accueil (le lier, l'oublier), puis chaque compagnie du Drive (sa dernière sauvegarde, son volume, une reprise pas finie sur ce poste, ou la compagnie de ce poste qui l'est déjà) et le geste qui la reprend, puis chaque reprise pas finie dont le dossier du Drive ne se voit pas, et le geste qui l'abandonne ; ce qui ne se lit pas se dit en encart ; detail_du_drive : ce que Google a répondu quand le Drive ne se lit pas",
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
        rend: "compte : l'adresse du compte Google lié, phrase, apres (relire) ; la fenêtre de Google s'ouvre dans le navigateur, son adresse est dite sur la sortie d'erreur",
        refus: &[Refus { classe: Classe::Refuse, corriger: "ce que la phrase dit (Google a refusé, aucune réponse en cinq minutes, l'application absente) ; rien n'est enregistré" }],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 120,
        ecran: Ecran::Geste(Place::AccueilDrive),
    },
    Fiche {
        route: Route::DriveReprendre,
        geste: "Ramener une compagnie du Drive de l'accueil dans un dossier neuf de ce poste.",
        charge: &ChargeReprendre::CHARGE,
        exemple: r#"{"dossier":"cie-essai"}"#,
        rend: "apres (poser_l_ecran), l'écran de fin de reprise (phrase, encarts, rangs : ce qui est revenu, ce qui reste dans le Drive, ce qui reste à faire pour son coffre, et le geste qui l'ouvre ; pas finie, le geste qui la continue ou celui qui l'abandonne, et pourquoi), id, nom, chemin (le --racine de ses routes, au bout seulement), au_bout, redescendus, incidents (ce qui ne descendra jamais par cette voie, un fichier par ligne), a_retenter (ce qu'une nouvelle reprise retente, un fichier par ligne)",
        refus: &[
            Refus { classe: Classe::Refuse, corriger: "lier d'abord le Drive, ou un dossier que drive-compagnies montre et qui n'est pas déjà sur ce poste (le refus nomme la compagnie), ou attendre qu'un autre geste sur la même reprise ait fini ; rien n'est repris" },
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
        rend: "apres (ouvrir), ouvrir (la compagnie, que la fenêtre ouvre), nom, chemin (le --racine de ses routes), phrase",
        refus: &[MANQUE, Refus { classe: Classe::Refuse, corriger: "une compagnie que compagnies montre (une reprise pas finie n'en est pas encore une) ; rien n'est ouvert" }],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Lit,
        phrase: 120,
        ecran: Ecran::Geste(Place::AccueilDrive),
    },
    Fiche {
        route: Route::DriveAbandonner,
        geste: "Abandonner une reprise du Drive pas finie : son dossier sur ce poste s'efface.",
        charge: &ChargeAbandonner::CHARGE,
        exemple: r#"{"reprise":"fictive:ajoutee-reprise-coupee"}"#,
        rend: "apres (relire), efface (le nom de la compagnie dont la reprise est abandonnée), reste_cache (une partie du dossier résiste, cachée : elle s'efface au prochain lancement), phrase ; rien ne change dans le Drive",
        refus: &[Refus { classe: Classe::Refuse, corriger: "une reprise pas finie que drive-compagnies montre, qu'aucun autre geste ne tient ; rien n'est effacé" }, OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 160,
        ecran: Ecran::Geste(Place::AccueilDrive),
    },
    Fiche {
        route: Route::DriveOublier,
        geste: "Oublier le compte Google de l'accueil : son jeton s'efface de ce poste.",
        charge: &SansCharge::CHARGE,
        exemple: r#"{}"#,
        rend: "apres (relire), efface (un jeton s'est effacé), phrase",
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
        acte: Acte::Ecrit,
        phrase: 120,
        ecran: Ecran::Geste(Place::AccueilDrive),
    },
];

const LIER: Affiche = Affiche::new("Lier le Drive");
const RELIER: Affiche = Affiche::new("Relier le Drive");
/// Ce que dit une reprise que l'autorisation du compte Google arrete (`SorteDErreur::Autorisation`).
const RELIER_PUIS_REPRENDRE: &str = "L'autorisation du compte Google ne vaut plus : « Relier le Drive », puis « Reprendre ».";

/// Le geste qui lie (ou relie) le compte Google de l'accueil, libelle `libelle`.
fn geste_lier(libelle: Affiche) -> GesteDeRoute<GesteDeLaReprise, SansCharge, SansCharge> {
    GesteDeRoute::avec_cases(Route::DriveLier, GesteDeLaReprise::Compte(SansCharge {}), libelle, Vec::new(), SansCharge {}, SansCharge {}).disant(crate::coffre::ouvrir_google_dit())
}
const OUBLIER: Affiche = Affiche::new("Oublier ce compte");
const REPRENDRE: Affiche = Affiche::new("Reprendre");
const OUVRIR: Affiche = Affiche::new("Ouvrir la compagnie");
const ABANDONNER: Affiche = Affiche::new("Abandonner cette reprise");

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

charge! {
    /// La charge de `drive-abandonner` : le dossier d'une reprise pas finie sur ce poste, tel que `drive-compagnies` le
    /// designe (son identifiant de compagnie : elle n'en est pas encore une).
    #[derive(Debug, Clone, Serialize)]
    pub struct ChargeAbandonner {
        pub reprise: IdDeCompagnie = dit("Reprise", "l'identifiant du dossier de la reprise pas finie, tel que drive-compagnies le rend dans la charge de son geste"),
    }
}

/// CE QU'UN GESTE D'UN ECRAN DE LA REPRISE PORTE : la charge de sa route, rien de plus. Lier ou oublier le compte Google
/// de l'accueil n'ont rien a designer ; reprendre porte un dossier du Drive ; ouvrir, une compagnie de ce poste ;
/// abandonner, le dossier d'une reprise pas finie.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum GesteDeLaReprise {
    Compte(SansCharge),
    Reprendre(ChargeReprendre),
    Ouvrir(ChargeOuvrir),
    Abandonner(ChargeAbandonner),
}

type EcranDeLaReprise = EcranRendu<GesteDeLaReprise>;

/// Ce que rend `drive-compagnies` : sa phrase, l'ecran de la reprise (le compte Google, chaque compagnie du Drive, chaque
/// reprise pas finie dont le dossier du Drive ne se voit pas), et, quand le Drive ne se lit pas, ce que Google a repondu
/// (l'ecran le dit en mots ; le detail, pour qui lit la reponse, peut porter un chemin).
#[derive(Debug, Serialize)]
pub(crate) struct CompagniesDuDrive {
    phrase: Phrase,
    #[serde(flatten)]
    ecran: EcranDeLaReprise,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail_du_drive: Option<String>,
}

/// Ce que rend `drive-lier`.
#[derive(Debug, Serialize)]
pub(crate) struct DriveLie {
    phrase: Phrase,
    apres: ApresLeGeste,
    compte: String,
}

/// Ce que rend `drive-oublier` : si un jeton s'est efface.
#[derive(Debug, Serialize)]
pub(crate) struct DriveOublie {
    phrase: Phrase,
    apres: ApresLeGeste,
    efface: bool,
}

/// Ce que rend `drive-abandonner` : le nom de la compagnie dont la reprise est abandonnee, et si une partie de son
/// dossier resiste (cachee, elle s'efface au lancement suivant : `suppression::effacer_les_restes`).
#[derive(Debug, Serialize)]
pub(crate) struct RepriseAbandonnee {
    phrase: Phrase,
    apres: ApresLeGeste,
    efface: String,
    reste_cache: bool,
}

/// Ce que rend `drive-reprendre` : l'ecran de fin de reprise (ce qui est revenu, ce qui reste dans le Drive, ce qui reste
/// a faire pour le coffre, et le geste qui ouvre la compagnie ; pas finie, pourquoi, et le geste qui la continue ou celui
/// qui l'abandonne) ; puis, pour qui lit la reponse, la compagnie (son identifiant, son nom, et son dossier sur ce poste,
/// au bout seulement : avant, ce n'est pas une compagnie, et aucune route ne s'y sert), si la reprise est allee au bout,
/// le nombre de fichiers revenus, et le detail de ce qui ne l'est pas, un fichier par ligne, chemins et empreintes
/// compris (l'ecran en dit le nombre) : ce qui ne descendra jamais par cette voie (`incidents`), ce qu'une nouvelle
/// reprise retente (`a_retenter`).
#[derive(Debug, Serialize)]
pub(crate) struct Reprise {
    phrase: Phrase,
    apres: ApresLeGeste,
    #[serde(flatten)]
    ecran: EcranDeLaReprise,
    id: IdDeCompagnie,
    nom: String,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin", skip_serializing_if = "Option::is_none")]
    chemin: Option<PathBuf>,
    au_bout: bool,
    redescendus: usize,
    incidents: Vec<String>,
    a_retenter: Vec<String>,
}

/// Ce que rend `compagnie-ouvrir` : la compagnie que la fenetre ouvre, et son dossier (le `--racine` de ses routes).
#[derive(Debug, Serialize)]
pub(crate) struct AOuvrir {
    phrase: Phrase,
    apres: ApresLeGeste,
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

impl Dite for RepriseAbandonnee {
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
        Route::DriveAbandonner => {
            let c: ChargeAbandonner = charge.lire()?;
            en_json(abandonner(inst, c.reprise)?)
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

/// Une reprise pas finie dont la marque ne se lit pas : son identifiant, et le nom de son dossier sur ce poste.
type Illisible = (IdDeCompagnie, String);

/// LES REPRISES PAS FINIES DE L'INSTALLATION `inst`, cherchees par leur marque (`installation::reprises_commencees`),
/// chacune avec sa marque ; et, a part, chacune dont la marque ne se lit pas. Une marque illisible ne designe aucun
/// dossier du Drive : aucune reprise ne la continue ; l'ecran la montre, et le geste qui l'abandonne.
fn reprises_en_cours(inst: &Path) -> R<(Vec<Commencee>, Vec<Illisible>)> {
    let (mut lues, mut illisibles) = (Vec::new(), Vec::new());
    for (id, racine) in compagnie::installation::reprises_commencees(inst)? {
        match compagnie::restauration::reprise_en_cours(&racine) {
            Ok(Some(r)) => lues.push((id, racine, r)),
            Ok(None) => {}
            Err(_) => illisibles.push((id, compagnie::installation::nom_dossier(&racine)?.to_string())),
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

/// Le geste « Abandonner cette reprise » du dossier `reprise` sur ce poste, d'une reprise de `nom` : sa boite dit que
/// rien ne se perd.
fn geste_abandonner(reprise: &IdDeCompagnie, nom: &str) -> GesteDeRoute<GesteDeLaReprise, SansCharge, SansCharge> {
    let boite = format!(
        "La reprise de « {} » s'arrête et son dossier s'efface de ce poste. Rien ne se perd : tout ce qu'il porte est venu du Drive, et y reste.",
        nom
    );
    GesteDeRoute::avec_cases(Route::DriveAbandonner, GesteDeLaReprise::Abandonner(ChargeAbandonner { reprise: reprise.clone() }), ABANDONNER, Vec::new(), SansCharge {}, SansCharge {})
        .disant(vec![boite])
}

/// Ce que dit une reprise reelle commencee qui ne continue pas sur une installation en demo (`reelle_en_demo`).
const REELLE_EN_DEMO: &str = "Elle a commencé pour une compagnie réelle, et le logiciel est en démo : elle ne continue que hors démo. L'abandonner pour la reprendre en démo.";

/// UNE REPRISE REELLE COMMENCEE (`compagnies/`) NE SE CONTINUE PAS SUR UNE INSTALLATION EN DEMO : allee au bout, elle
/// serait une compagnie reelle que la demo ne liste pas (`installation::liste_de`), et « Ouvrir la compagnie » se
/// refuserait. L'inverse se continue : une installation debloquee liste les fictives, une reprise commencee dans
/// `fictives/` y finit fictive et s'ouvre.
fn reelle_en_demo(inst: &Path, id: &IdDeCompagnie) -> bool {
    matches!(id, IdDeCompagnie::Cie(_)) && compagnie::bridage::est_bride_de(inst)
}

/// Le rang d'une reprise pas finie que rien ne continue d'ici : `titre`, ce qu'elle `dit`, et le geste qui l'abandonne.
fn rang_a_abandonner(titre: String, dit: Vec<String>, reprise: &IdDeCompagnie, nom: &str) -> RangDeLEcran<GesteDeLaReprise, SansCharge, SansCharge> {
    RangDeLEcran {
        titre,
        dit,
        pastille: Pastille::de(EtatDEcran::EnAttente, "reprise pas finie"),
        etat: None,
        montre: None,
        gestes: vec![geste_abandonner(reprise, nom).principal()],
    }
}

/// L'ECRAN DE LA REPRISE (`drive-compagnies`) : le compte Google de l'accueil (le lier, le relier, l'oublier), puis chaque
/// dossier du Drive qui se reprend (le dossier d'une compagnie, l'ancienne sauvegarde commune), sa derniere sauvegarde,
/// son volume, ce qu'on en fait (`ce_qu_on_en_fait`) et ses gestes ; puis chaque reprise pas finie qu'aucun de ces rangs
/// ne montre (son dossier du Drive ne se voit pas, ou sa marque ne se lit pas), et le geste qui l'abandonne. Doux : sans
/// compte lie, un jeton ou un Drive qui ne se lit pas, des compagnies du poste qui ne se listent pas, cela se dit en
/// encart, en mots, et ne ferme rien ; seul « Reprendre » ne se propose pas tant que ce poste ne se lit pas.
pub(crate) fn compagnies(inst: &Path) -> R<CompagniesDuDrive> {
    let mut encarts = vec![Encart::de(EtatDEcran::Information, "", "Une compagnie sauvegardée dans votre Drive revient sur ce poste, telle qu'elle y est, dans un dossier neuf.")];
    if compagnie::bridage::est_bride_de(inst) {
        encarts.push(Encart::de(EtatDEcran::Information, "Logiciel en démo", "La compagnie reprise sera une compagnie fictive : on y regarde tout, rien n'en part."));
    }
    let lu = crate::coffre::compte_autorise(inst);
    let lier = geste_lier;
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
    // Ce qui ne se lit pas sur ce poste se dit en mots (une erreur du disque porte un chemin), et ne ferme pas l'ecran ;
    // mais tant qu'il ne se lit pas, la reprise se refuserait (elle ne saurait pas ce qui est deja ici) : son geste ne se
    // propose pas.
    let mut se_reprend = true;
    let (en_cours, illisibles) = match reprises_en_cours(inst) {
        Ok(l) => l,
        Err(_) => {
            se_reprend = false;
            encarts.push(Encart::de(EtatDEcran::Refuse, "Les dossiers de ce poste ne se lisent pas", "Une reprise commencée ici ne se voit pas : rien ne se reprend tant qu'ils ne se lisent pas."));
            (Vec::new(), Vec::new())
        }
    };
    let du_poste = match identites_du_poste(inst) {
        Ok(l) => l,
        Err(_) => {
            se_reprend = false;
            encarts.push(Encart::de(EtatDEcran::Refuse, "Les compagnies de ce poste ne se lisent pas", "Celles déjà reprises ne se voient pas : rien ne se reprend tant qu'elles ne se lisent pas."));
            Vec::new()
        }
    };
    for (nom, i) in &du_poste {
        if i.is_err() {
            encarts.push(Encart::de(EtatDEcran::Refuse, format!("« {} » : son dossier du Drive ne se reconnaît pas", nom), "Sa marque sur ce poste ne se lit pas : elle a été changée à la main."));
        }
    }
    // LE DRIVE : rien sans compte lisible ; un Drive qui ne se lit pas se dit en mots, ce que Google a repondu a part.
    let mut detail_du_drive = None;
    let drive = match &compte {
        Err(sans_compte) => Err(Phrase::fixe(*sans_compte)),
        Ok(c) => match crate::coffre::client_drive(inst).and_then(|mut cl| compagnie::sur_le_drive::voir(&mut cl, compagnie::sauvegarde::RACINE_DRIVE)) {
            Ok(v) => Ok((c.clone(), v.into_iter().filter(DossierVu::se_reprend).collect::<Vec<DossierVu>>())),
            Err(e) => {
                let dit = if e.injoignable {
                    "Google ne répond pas : la connexion est peut-être coupée. Rien ne se reprend tant que la liste ne se lit pas."
                } else {
                    "Google n'a pas rendu la liste des dossiers de PuppetPaie : rien ne se reprend tant qu'elle ne se lit pas."
                };
                encarts.push(Encart::de(EtatDEcran::Refuse, "Le Drive ne se lit pas", dit));
                detail_du_drive = Some(e.echec.en_lignes());
                Err(socle::phrase!("Le Drive de {} ne se lit pas.", c))
            }
        },
    };
    let vus: &[DossierVu] = match &drive {
        Ok((_, v)) => v,
        Err(_) => &[],
    };
    if let Ok((_, v)) = &drive {
        if v.is_empty() {
            encarts.push(Encart::de(EtatDEcran::Information, "Aucune compagnie dans ce Drive", "Ce compte Google ne porte aucune sauvegarde de PuppetPaie. Est-ce le bon compte ?"));
        }
    }
    // Les reprises pas finies que le rang d'un dossier du Drive montre ; les autres ont leur rang a elles, plus bas.
    let mut montrees: Vec<&Path> = Vec::new();
    for vu in vus {
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
            // DEJA SUR CE POSTE : la reprise se refuserait ; le geste ne se propose pas, le rang dit ou elle est. Une
            // reprise de ce dossier commencee ici ne continuera pas : le geste qui l'abandonne.
            CeQuOnEnFait::DejaLa { noms, douteuses } => {
                dit.push(deja_sur_ce_poste(&noms, &douteuses));
                let mut gestes = Vec::new();
                for (id, racine, _) in en_cours.iter().filter(|(_, _, r)| r.dossier == vu.id) {
                    dit.push("Une reprise de ce dossier commencée sur ce poste ne continuera pas : l'abandonner.".to_string());
                    montrees.push(racine);
                    gestes.push(geste_abandonner(id, &vu.nom));
                }
                (Pastille::de(EtatDEcran::Fait, "déjà sur ce poste"), gestes)
            }
            CeQuOnEnFait::Continuer((id, racine, r)) => {
                montrees.push(racine);
                let depuis = socle::paris::ecrit::saisie::texte(r.depuis);
                let mut gestes = Vec::new();
                if reelle_en_demo(inst, id) {
                    dit.push(format!("Une reprise commencée sur ce poste le {} n'est pas finie.", depuis));
                    dit.push(REELLE_EN_DEMO.to_string());
                    gestes.push(geste_abandonner(id, &vu.nom).principal());
                } else {
                    // Ce que la reprise trouvera, seul « Reprendre » le sait (le Drive a pu changer) : il le dit, et
                    // continue si elle le peut.
                    dit.push(format!("Une reprise commencée sur ce poste le {} n'est pas finie : « Reprendre » dit où elle en est, et la continue si elle le peut.", depuis));
                    if se_reprend {
                        gestes.push(geste_reprendre(&vu.id, format!("La reprise de « {} » continue où elle en était, si elle le peut.", vu.nom)));
                    }
                    gestes.push(geste_abandonner(id, &vu.nom));
                }
                (Pastille::de(EtatDEcran::EnAttente, "reprise pas finie"), gestes)
            }
            CeQuOnEnFait::Neuve => {
                let boite = format!("« {} » revient sur ce poste dans un dossier neuf. Son coffre revient aussi ; l'écran de fin de reprise dit ce qu'il reste à faire pour lui.", vu.nom);
                let gestes = if se_reprend { vec![geste_reprendre(&vu.id, boite)] } else { Vec::new() };
                (Pastille::de(EtatDEcran::Information, "dans le Drive"), gestes)
            }
        };
        rangs.push(RangDeLEcran { titre: vu.nom.clone(), dit, pastille, etat: None, montre: None, gestes });
    }
    // LES REPRISES PAS FINIES QU'AUCUN DOSSIER DU DRIVE NE MONTRE : leur dossier n'y est plus (supprime, a la corbeille, sur
    // un autre compte), ou le Drive ne se lit pas ; rien ne les continue d'ici, le geste les abandonne.
    for (id, _, r) in en_cours.iter().filter(|(_, racine, _)| !montrees.contains(&racine.as_path())) {
        let pourquoi = match (&compte, &drive) {
            // Le dossier se voit : une autre reprise du meme dossier est celle qui se continue (`ce_qu_on_en_fait`).
            _ if vus.iter().any(|v| v.id == r.dossier) => "Une autre reprise du même dossier du Drive se continue sur ce poste : celle-ci reste de côté, l'abandonner.",
            (Err(_), _) => "Sans compte Google lisible à l'accueil, son dossier du Drive ne se voit pas : elle ne peut pas continuer d'ici.",
            (Ok(_), Err(_)) => "Son dossier du Drive ne se voit pas tant que le Drive ne se lit pas : elle ne peut pas continuer d'ici.",
            (Ok(_), Ok(_)) => "Son dossier n'est plus dans ce Drive (supprimé, à la corbeille, ou sur un autre compte Google) : elle ne peut pas continuer.",
        };
        let dit = vec![format!("Une reprise commencée sur ce poste le {} n'est pas finie.", socle::paris::ecrit::saisie::texte(r.depuis)), pourquoi.to_string()];
        rangs.push(rang_a_abandonner(r.nom.clone(), dit, id, &r.nom));
    }
    for (id, nom) in &illisibles {
        let dit = vec!["Une reprise commencée dans ce dossier de ce poste ne se lit pas (sa marque a été changée à la main) : rien ne la continue.".to_string()];
        rangs.push(rang_a_abandonner(nom.clone(), dit, id, nom));
    }
    let phrase = match drive {
        Ok((c, v)) => socle::phrase!("Le Drive de {} : {} compagnie(s) à reprendre.", c, v.len()),
        Err(p) => p,
    };
    Ok(CompagniesDuDrive { phrase, ecran: EcranRendu { encarts, rangs, cases: Vec::new(), historique: Vec::new(), enregistrer: None }, detail_du_drive })
}

/// LIER LE COMPTE GOOGLE DE L'ACCUEIL (`drive-lier`) : la meme autorisation que celle d'une compagnie, scellee sous
/// l'installation. Aucun mot de passe : il n'y a pas encore de compagnie, et rien ne part de l'accueil.
pub(crate) fn lier(inst: &Path) -> R<DriveLie> {
    let compte = crate::coffre::lier_google(inst)?;
    Ok(DriveLie { phrase: socle::phrase!("Drive lié : {}.", compte), apres: ApresLeGeste::Relire, compte })
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
    Ok(DriveOublie { phrase, apres: ApresLeGeste::Relire, efface })
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
/// refuse (`ce_qu_on_en_fait`). ALLEE AU BOUT : chaque fichier est descendu, sauf ceux qui ne descendront jamais par cette
/// voie (`Restauration::complete` : ni la connexion tombee, que la panne dit, ni un fichier a retenter), et la fiche
/// Entreprise est revenue et se lit (sans elle, ce ne serait pas une compagnie) ; alors la compagnie recoit le jeton de
/// l'accueil (une fictive, d'un logiciel bride ou commencee dans `fictives/`, recoit sa marque de fictive ajoutee, et rien
/// n'en part), son coffre descendu du Drive se range s'il ne s'ouvrira pas ici (`restauration::a_faire`), sa marque
/// s'efface, et l'ecran de fin de reprise propose de l'ouvrir. Sinon la marque reste, et l'ecran propose de continuer
/// quand une nouvelle tentative peut changer quelque chose, et d'abandonner. Un refus de la restauration n'ecrit rien : le
/// dossier neuf qu'elle aurait rempli s'efface ; une reprise continuee qui se refuse rend l'ecran qui dit pourquoi et
/// propose de l'abandonner (`reprise_arretee`).
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
    // LA DEMO SE LIT SUR LE DOSSIER : une reprise commencee dans `fictives/` reste une fictive, une commencee dans
    // `compagnies/` reste reelle, quelle que soit l'installation quand elle continue ; une neuve suit l'installation.
    let bride = match commencee {
        Some((id, _, _)) => matches!(id, IdDeCompagnie::Fictive(_)),
        None => compagnie::bridage::est_bride_de(inst),
    };
    if let Some((id, _, _)) = commencee.filter(|(id, _, _)| reelle_en_demo(inst, id)) {
        return Ok(arretee(&vu, id.clone(), REELLE_EN_DEMO, "Reprise pas finie : elle ne continue pas sur ce logiciel en démo.", false, false));
    }
    let (id, racine, neuve) = ou_revient(inst, &vu, bride, commencee)?;
    // Une restauration du dossier a la fois (`VerrouDeRestauration`, que `restaurer` rafraichit a chaque fichier) ; une
    // reprise continuee abandonnee entre-temps ne se recree pas.
    let verrou = compagnie::restauration::VerrouDeRestauration::prendre(&racine)?;
    if !compagnie::installation::en_reprise(&racine) {
        return Err(erreur(Classe::Refuse, "Cette reprise vient d'être abandonnée : rien n'est repris. Relire la liste."));
    }
    let r = match compagnie::restauration::restaurer(&racine, &vu, racine_drive, &mut client, &verrou) {
        Ok(r) => r,
        Err(e) if neuve => {
            // Rien n'a ete redescendu : le dossier neuf ne porte que la marque de la reprise ; il s'efface cache d'abord.
            crate::accueil::suppression::effacer(&racine, "La reprise ne s'est pas faite, et son dossier neuf ne s'efface pas : il se voit à l'accueil, l'abandonner.")?;
            // L'autorisation perdue se dit pour l'accueil (la phrase de Google renvoie a une compagnie).
            if envois::drive::sorte(&e) == envois::drive::SorteDErreur::Autorisation {
                return Err(erreur(Classe::Google, format!("{} Rien n'est repris.", RELIER_PUIS_REPRENDRE)));
            }
            return Err(e.echec);
        }
        // UNE REPRISE COMMENCEE QUI NE CONTINUE PAS (ce qu'elle a deja ramene n'est plus la copie du Drive, le dossier du
        // Drive n'a plus sa sauvegarde, ou deux) : rien n'est ecrit ; l'ecran dit pourquoi, et propose de l'abandonner
        // (et de la reprendre, sauf quand ce qu'elle a deja ramene a change : reprendre redirait la meme chose).
        Err(e) => {
            let retenter = e.echec.classe != Some(Classe::DossierNonVide);
            let relier = envois::drive::sorte(&e) == envois::drive::SorteDErreur::Autorisation;
            // Les refus de la restauration elle-meme sont en mots ; une reponse de Google peut porter une adresse.
            let pourquoi = match e.echec.classe {
                _ if e.injoignable => "Google ne répond pas : la connexion est peut-être coupée. Rien n'est écrit.".to_string(),
                _ if relier => format!("{} Rien n'est écrit.", RELIER_PUIS_REPRENDRE),
                Some(Classe::DossierNonVide | Classe::SauvegardeAbsente | Classe::SauvegardeAmbigue) => e.echec.phrase.clone(),
                _ => "Google n'a pas rendu la sauvegarde de ce dossier : rien n'est écrit.".to_string(),
            };
            let dit = if retenter {
                "Reprise pas finie : la compagnie ne s'ouvre pas encore. Le Drive corrigé, « Reprendre » la continue ; sinon, l'abandonner."
            } else {
                "Ce qu'elle a déjà ramené sur ce poste n'est plus la copie du Drive : elle ne peut pas continuer. L'abandonner, puis la reprendre de nouveau."
            };
            return Ok(arretee(&vu, id, &pourquoi, dit, retenter, relier));
        }
    };
    let fiche = if !droit::contexte::ENTREPRISE.dans(&racine).is_file() {
        FicheRevenue::Absente
    } else if compagnie::installation::fiche_se_lit(&racine) {
        FicheRevenue::Lisible
    } else {
        FicheRevenue::Illisible
    };
    let au_bout = r.complete() && fiche == FicheRevenue::Lisible;
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
            // Le coffre revenu : ce qu'il reste a faire pour lui, apres l'avoir mis de cote s'il ne s'ouvrira pas ici. La
            // marque est encore la : le coffre de ce dossier est descendu du Drive (`Restauration::origine_du_coffre`).
            let coffre = compagnie::coffre::Coffre::du_poste(&racine);
            a_faire = compagnie::restauration::a_faire(&coffre, r.origine_du_coffre(&coffre));
        }
        compagnie::restauration::finir_la_reprise(&racine)?;
    }
    let nom = if fiche == FicheRevenue::Lisible { compagnie::installation::nom_de(&racine)? } else { vu.nom.clone() };
    let revenus = r.redescendus.len();
    let ecran = fin_de_reprise(&vu, &id, &nom, au_bout, bride, fiche, &r, a_faire);
    let phrase = if au_bout {
        socle::phrase!("« {} » est reprise de votre Drive : {} fichier(s) revenu(s).", nom, revenus)
    } else {
        socle::phrase!("La reprise de « {} » n'est pas finie : {} fichier(s) revenu(s).", nom, revenus)
    };
    Ok(Reprise {
        phrase,
        apres: ApresLeGeste::PoserLEcran,
        ecran,
        id,
        nom,
        chemin: au_bout.then_some(racine),
        au_bout,
        redescendus: revenus,
        incidents: r.incidents,
        a_retenter: r.a_retenter,
    })
}

/// LA FICHE ENTREPRISE D'UNE COMPAGNIE QUI REVIENT : sans elle, ou illisible, ce n'est pas une compagnie, et la reprise
/// n'est pas au bout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FicheRevenue {
    Absente,
    Illisible,
    Lisible,
}

/// CE QUE REND UNE REPRISE COMMENCEE QUI NE CONTINUE PAS (rien n'est ecrit) : son ecran dit pourquoi (`pourquoi`, en mots,
/// et `dit` sur son rang), et ce qui se fait : l'abandonner (tout ce qu'elle a ramene vient du Drive, et y reste), ou la
/// reprendre une fois le Drive corrige (`retenter`), ou le compte Google relie (`relier`).
fn arretee(vu: &DossierVu, id: IdDeCompagnie, pourquoi: &str, dit: &str, retenter: bool, relier: bool) -> Reprise {
    let encarts = vec![Encart::de(EtatDEcran::Refuse, "Cette reprise ne continue pas", pourquoi)];
    let mut gestes = Vec::new();
    if relier {
        gestes.push(geste_lier(RELIER).principal());
    }
    let abandonner = geste_abandonner(&id, &vu.nom);
    gestes.push(if relier { abandonner } else { abandonner.principal() });
    if retenter {
        gestes.push(GesteDeRoute::avec_cases(Route::DriveReprendre, GesteDeLaReprise::Reprendre(ChargeReprendre { dossier: vu.id.clone() }), REPRENDRE, Vec::new(), SansCharge {}, SansCharge {}).disant(vec![format!("La reprise de « {} » continue où elle en était, si elle le peut.", vu.nom)]));
    }
    let rang = RangDeLEcran { titre: vu.nom.clone(), dit: vec![dit.to_string()], pastille: Pastille::de(EtatDEcran::EnAttente, "reprise pas finie"), etat: None, montre: None, gestes };
    Reprise {
        phrase: socle::phrase!("La reprise de « {} » ne continue pas : rien n'est écrit.", vu.nom),
        apres: ApresLeGeste::PoserLEcran,
        ecran: EcranRendu { encarts, rangs: vec![rang], cases: Vec::new(), historique: Vec::new(), enregistrer: None },
        id,
        nom: vu.nom.clone(),
        chemin: None,
        au_bout: false,
        redescendus: 0,
        incidents: Vec::new(),
        a_retenter: Vec::new(),
    }
}

/// L'ECRAN DE FIN DE REPRISE : ce qui est revenu ; ce qui ne reviendra pas par cette voie et reste dans le Drive (en
/// mots, le detail dans `Reprise::incidents`) ; pourquoi elle n'est pas finie (la connexion tombee, des fichiers a
/// retenter, la fiche Entreprise absente ou illisible) et ce qui se fait vraiment : la continuer quand une nouvelle
/// tentative peut changer quelque chose, sinon l'abandonner ; allee au bout, ce qui reste a faire pour son coffre
/// (`a_faire`, une compagnie reelle), le dossier du Drive qu'elle garde avec l'ancien poste, et le geste qui l'ouvre.
#[allow(clippy::too_many_arguments)]
fn fin_de_reprise(vu: &DossierVu, id: &IdDeCompagnie, nom: &str, au_bout: bool, bride: bool, fiche: FicheRevenue, r: &compagnie::restauration::Restauration, a_faire: Vec<String>) -> EcranDeLaReprise {
    let mut encarts = Vec::new();
    // Pas finie : une nouvelle tentative change-t-elle quelque chose (la connexion, des fichiers a retenter) ?
    let continuer = !au_bout && !r.complete();
    if au_bout {
        encarts.push(Encart::de(EtatDEcran::Fait, "Revenue sur ce poste", format!("{} fichier(s) de sa sauvegarde sont revenus dans un dossier neuf.", r.redescendus.len())));
    } else if r.arret == Some(ArretDeLaRestauration::HorsLigne) {
        encarts.push(Encart::de(EtatDEcran::AFaire, "La connexion est tombée", format!("{} fichier(s) sont revenus, puis la connexion est tombée. « Reprendre » continue où elle en était.", r.redescendus.len())));
    } else if r.arret == Some(ArretDeLaRestauration::AutorisationPerdue) {
        encarts.push(Encart::de(EtatDEcran::AFaire, "L'autorisation Google ne vaut plus", format!("{} fichier(s) sont revenus, puis Google a refusé l'autorisation. {} Ce qui est revenu reste ici.", r.redescendus.len(), RELIER_PUIS_REPRENDRE)));
    } else if r.arret == Some(ArretDeLaRestauration::VerrouPerdu) {
        encarts.push(Encart::de(EtatDEcran::AFaire, "Un autre geste a pris la main", format!("{} fichier(s) sont revenus, puis un autre geste a pris ce dossier : celle-ci s'est arrêtée. Relire la liste.", r.redescendus.len())));
    } else if !r.a_retenter.is_empty() {
        encarts.push(Encart::de(
            EtatDEcran::AFaire,
            "Des fichiers ne sont pas revenus cette fois",
            format!("{} fichier(s) ne sont pas revenus : Google n'a pas répondu à temps, un contenu est arrivé abîmé, ou le disque a refusé. « Reprendre » les retente.", r.a_retenter.len()),
        ));
    } else if fiche == FicheRevenue::Illisible {
        encarts.push(Encart::de(EtatDEcran::Refuse, "Sa fiche Entreprise ne se lit pas", "Elle est revenue telle que le Drive la porte, mais ne se lit pas : la compagnie ne s'ouvre pas. Reprendre n'y change rien ; l'abandonner."));
    } else {
        encarts.push(Encart::de(EtatDEcran::Refuse, "Sa fiche Entreprise n'est pas revenue", "Le Drive ne la donne pas : sans elle, la compagnie ne s'ouvre pas, et reprendre n'y change rien tant que le Drive ne change pas. L'abandonner."));
    }
    if !r.incidents.is_empty() {
        encarts.push(Encart::de(
            EtatDEcran::AFaire,
            "Ce qui ne reviendra pas ainsi",
            format!("{} fichier(s) ne peuvent pas revenir par cette voie (deux du même nom, un nom impossible ici, un document Google, un refus de Google) : ils restent dans le Drive.", r.incidents.len()),
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
    } else if continuer {
        let boite = format!("La reprise de « {} » continue où elle en était.", vu.nom);
        let mut gestes = Vec::new();
        // L'autorisation perdue : relier d'abord, puis reprendre.
        if r.arret == Some(ArretDeLaRestauration::AutorisationPerdue) {
            gestes.push(geste_lier(RELIER));
        }
        gestes.extend([geste_reprendre(&vu.id, boite), geste_abandonner(id, &vu.nom)]);
        rangs.push(RangDeLEcran { titre: nom.to_string(), dit: vec!["Reprise pas finie : la compagnie ne s'ouvre pas encore.".to_string()], pastille: Pastille::de(EtatDEcran::EnAttente, "reprise pas finie"), etat: None, montre: None, gestes });
    } else {
        rangs.push(rang_a_abandonner(nom.to_string(), vec!["Reprise pas finie : la compagnie ne s'ouvre pas.".to_string()], id, &vu.nom));
    }
    EcranRendu { encarts, rangs, cases: Vec::new(), historique: Vec::new(), enregistrer: None }
}

/// OUVRIR UNE COMPAGNIE DE CE POSTE (`compagnie-ouvrir`) : une tuile de l'accueil seulement (`installation::racine_choisie`
/// : une reprise pas finie n'en est pas une), que la fenetre ouvre (`ouvrir`) ; en ligne de commande, son `chemin` est le
/// `--racine` de ses routes. Ne change rien.
pub(crate) fn ouvrir(inst: &Path, id: IdDeCompagnie) -> R<AOuvrir> {
    let racine = compagnie::installation::racine_choisie(inst, &id).map_err(|e| e.reclasser(Classe::Refuse))?;
    let nom = compagnie::installation::nom_de(&racine)?;
    Ok(AOuvrir { phrase: socle::phrase!("« {} » s'ouvre.", nom), apres: ApresLeGeste::Ouvrir, ouvrir: id, nom, chemin: racine })
}

/// ABANDONNER UNE REPRISE PAS FINIE (`drive-abandonner`) : le dossier qu'elle remplit sur ce poste, et lui seul (une
/// reprise commencee, cherchee par sa marque : `installation::reprises_commencees`, jamais une compagnie), s'efface. Rien
/// ne se perd : tout ce qu'il porte est venu du Drive, qui n'est pas touche (le jeton de l'accueil n'y est copie qu'au
/// bout). Sa marque peut ne pas se lire : elle s'abandonne de meme.
pub(crate) fn abandonner(inst: &Path, reprise: IdDeCompagnie) -> R<RepriseAbandonnee> {
    let (_, racine) = compagnie::installation::reprises_commencees(inst)?
        .into_iter()
        .find(|(id, _)| *id == reprise)
        .ok_or_else(|| erreur(Classe::Refuse, "Ce dossier n'est pas une reprise du Drive pas finie sur ce poste : rien n'est effacé. Relire la liste."))?;
    // Pas pendant qu'une reprise de ce dossier tourne ; et il porte encore sa marque (elle a pu finir entre-temps).
    let _tenue = compagnie::restauration::VerrouDeRestauration::prendre(&racine)?;
    if !compagnie::installation::en_reprise(&racine) {
        return Err(erreur(Classe::Refuse, "Cette reprise vient de finir : ce dossier est une compagnie, il ne s'abandonne pas. Rien n'est effacé."));
    }
    let nom = match compagnie::restauration::reprise_en_cours(&racine) {
        Ok(Some(r)) => r.nom,
        Ok(None) | Err(_) => compagnie::installation::nom_dossier(&racine)?.to_string(),
    };
    // Cache d'abord, puis efface (`suppression::effacer`) : un effacement coupe ne laisse jamais une moitie de compagnie
    // qui deviendrait une tuile.
    let efface = crate::accueil::suppression::effacer(&racine, "La reprise ne s'abandonne pas : son dossier ne se renomme pas (fermez ce qui l'utilise, puis recommencez). Rien n'est effacé.")?;
    let phrase = match efface {
        crate::accueil::suppression::Efface::Tout => socle::phrase!("La reprise de « {} » est abandonnée : son dossier sur ce poste est effacé, le Drive n'a pas changé.", nom),
        crate::accueil::suppression::Efface::ResteCache => {
            socle::phrase!("La reprise de « {} » est abandonnée ; une partie de son dossier résiste, cachée : elle s'efface au prochain lancement.", nom)
        }
    };
    Ok(RepriseAbandonnee { phrase, apres: ApresLeGeste::Relire, efface: nom, reste_cache: efface == crate::accueil::suppression::Efface::ResteCache })
}
