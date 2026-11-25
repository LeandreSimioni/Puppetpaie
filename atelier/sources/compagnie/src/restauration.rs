//! La restauration d'une compagnie depuis le Drive : le retour de `sauvegarde.rs`. UN SEUL CHEMIN : la reprise d'une
//! compagnie a l'accueil (`routes::accueil::drive`, la route `drive-reprendre`, que `moteur restaurer` sert aussi), et
//! « Prendre la main », qui recupere le Drive a cote de la compagnie ouverte (`recuperation`).
//!
//! Aucun depot Git ne porte les donnees de la compagnie (regle 9) : la sauvegarde Drive est la seule copie de `prive/`
//! hors du poste ; sans retour, ce n'est pas une copie.
//!
//! CE QU'ELLE FAIT, dans cet ordre, et rien n'est ecrit avant la troisieme etape :
//! 1. elle prend l'arbre du dossier choisi dans la vue du Drive (`sur_le_drive`) : `sauvegarde/` du dossier d'une
//!    compagnie, ou l'ancien dossier commun lui-meme ; SANS RIEN Y CREER (`Drive::sous_dossiers`, pas `Drive::chemin`),
//!    elle en fait l'inventaire complet : chaque fichier, son empreinte (`md5Checksum`), son dossier ;
//! 2. elle compare le dossier local a cet inventaire (voir « un dossier qui n'est pas vide ») ;
//! 3. elle redescend ce qui manque, fichier par fichier : le contenu recu est compare a l'empreinte que Drive publie AVANT
//!    d'etre ecrit (un `.part`, renomme ensuite : un fichier coupe en route n'a jamais le nom du vrai) ;
//! 4. elle reconstruit l'index local (`config/.sauvegarde.json`) : la sauvegarde suivante connait les dossiers Drive sans
//!    les rechercher, et `rien_n_a_bouge` dit oui tant que rien n'a change.
//!
//! RIEN D'EFFACE, NI ICI NI LA-BAS. Le trait `Drive` n'a pas de suppression (`envois::drive::Corbeille` est a part) ; ici, un
//! fichier qui existe n'est jamais remplace.
//!
//! UN DOSSIER QUI N'EST PAS VIDE : il est accepte si tout ce que la sauvegarde y porterait (son
//! `inventaire`) est DEJA identique a la copie du Drive (meme empreinte), et refuse sinon, avant toute ecriture, en nommant
//! les fichiers. Pourquoi pas « ne recevoir que ce qui manque » : ce serait fabriquer une compagnie qu'aucun poste n'a eue
//! (un journal de depots d'ici, des compteurs de la-bas : regle 4, un numero de contrat se lit, il ne se recalcule pas), et
//! l'index reconstruit dirait a la sauvegarde suivante que tout est a jour. Pourquoi pas « vide ou rien » : (1) un poste neuf
//! a deja relie son Drive, et le jeton (`google-jeton.bin`) est dans `config/` — ecarte de la sauvegarde, il ne compte pas ;
//! (2) une restauration coupee (reseau, fermeture) se RELANCE et reprend ou elle en etait ; (3) un `google.json` recopie
//! a la main pour relier la bonne application Google n'empeche rien s'il est celui de la sauvegarde.
//!
//! Ce qu'elle ne redescend pas : ce que la sauvegarde n'envoie pas (`sauvegarde::ecarte`), un document sans empreinte
//! (un Google Docs : rien a comparer), un nom qui ne peut pas etre un nom de fichier ici, et les homonymes (deux
//! dossiers, ou deux fichiers, de meme nom au meme endroit du Drive : elle ne choisit pas, elle le dit).
//!
//! UNE REPRISE A L'ACCUEIL (`routes::accueil::drive`) se fait toujours dans un dossier neuf, qu'elle marque d'abord
//! (`RepriseEnCours`, `config/reprise-en-cours.json`, propre au poste : il ne monte pas) ; tant qu'elle est la, le dossier
//! n'est pas une compagnie du poste (`installation::liste_de`) ; coupee, elle se relance dans ce meme dossier, et dans
//! aucun autre ; allee au bout (`Restauration::complete`), elle efface sa marque. Le coffre descendu du Drive que rien
//! n'ouvrira ici se met de cote (`ranger_le_coffre`, `OrigineDuCoffre`), et ce qu'il reste a faire se dit (`a_faire`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};


use crate::coffre::{Coffre, SERVICES};
use envois::drive::{Drive, EmpreinteMd5, ErreurGoogle, IdDrive, SorteDErreur};
use crate::sauvegarde::{ecarte, ecrire_index, inventaire, DossierDuDrive, Index, SAUVEGARDE};
use crate::sur_le_drive::{DossierVu, GenreDuDossier};
use socle::fichiers::{CheminRelatif, NomDeFichier};

/// La marque d'une reprise du Drive pas finie, sous la compagnie qu'elle remplit (`RepriseEnCours`).
pub const REPRISE_EN_COURS: CheminRelatif = CheminRelatif::du_moteur("config/reprise-en-cours.json");

/// UNE REPRISE DU DRIVE PAS FINIE SUR CE POSTE : posee la premiere dans le dossier neuf qu'elle remplit, avant tout autre
/// fichier ; elle dit de quel dossier du Drive il revient, pour qu'une reprise coupee se relance la, et nulle part
/// ailleurs. La reprise allee au bout l'efface (`finir_la_reprise`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepriseEnCours {
    /// Le dossier du Drive qui revient (son identifiant, tel que la vue du Drive le rend).
    pub dossier: IdDrive,
    /// Son nom dans le Drive.
    pub nom: String,
    #[serde(with = "socle::paris::ecrit::saisie")]
    pub depuis: socle::paris::Moment,
    pub quoi: String,
}

/// La reprise pas finie que porte la compagnie `racine`, s'il y en a une ; une marque qui ne se lit pas se refuse.
pub fn reprise_en_cours(racine: &Path) -> socle::echec::R<Option<RepriseEnCours>> {
    let p = REPRISE_EN_COURS.dans(racine);
    if !socle::fichiers::present(&p)? {
        return Ok(None);
    }
    socle::forme::lire_fichier(&p, "La marque d'une reprise du Drive commencée sur ce poste ne se lit pas : elle a été changée à la main.").map(Some)
}

/// Pose dans `racine` (un dossier neuf) la marque de la reprise du dossier `vu` : la premiere chose qu'elle y ecrit.
pub fn commencer_la_reprise(racine: &Path, vu: &DossierVu) -> socle::echec::R<()> {
    let marque = RepriseEnCours {
        dossier: vu.id.clone(),
        nom: vu.nom.clone(),
        depuis: socle::temps::secondes().a_la_seconde(),
        quoi: "Une reprise du Drive commencée sur ce poste et pas finie : la reprendre de nouveau la continue dans ce dossier. Elle s'efface quand tout est revenu.".to_string(),
    };
    socle::ecriture::ecrire_json(&REPRISE_EN_COURS.dans(racine), &marque, 2)
}

/// La reprise de `racine` est allee au bout : sa marque s'efface.
pub fn finir_la_reprise(racine: &Path) -> socle::echec::R<()> {
    socle::fichiers::effacer(&REPRISE_EN_COURS.dans(racine))
}

/// Le compte rendu d'une restauration.
#[derive(Debug, Clone, PartialEq)]
pub struct Restauration {
    pub cible: PathBuf,
    /// Les fichiers redescendus, chacun par son chemin dans la compagnie.
    pub redescendus: Vec<CheminRelatif>,
    /// CE QUI NE DESCENDRA JAMAIS PAR CETTE VOIE, quelle que soit la tentative : deux dossiers ou deux fichiers de meme
    /// nom au meme endroit du Drive, un nom impossible ici, un document sans empreinte (un document Google), un fichier
    /// dont Google dit lui-meme qu'il ne se donne pas (`envois::drive::SorteDErreur::DuFichier`), un fichier deja la
    /// qu'elle garde tel quel. Un fichier par ligne.
    pub incidents: Vec<String>,
    /// CE QUI N'EST PAS DESCENDU ET SE RETENTE : ce que Google n'a pas rendu cette fois (`SorteDErreur::Passagere`), le
    /// fichier sur lequel elle s'est arretee (`arret`, sa cause en derniere ligne), un contenu dont l'empreinte ne colle
    /// pas, une ecriture que le disque a refusee. Une nouvelle restauration du meme dossier le retente. Un fichier par
    /// ligne.
    pub a_retenter: Vec<String>,
    /// Arretee en route (`ArretDeLaRestauration`), et pourquoi ; aucune : elle a tente chaque fichier.
    pub arret: Option<ArretDeLaRestauration>,
}

/// POURQUOI UNE RESTAURATION S'EST ARRETEE EN ROUTE, sans tenter les fichiers suivants : chaque cas laisse ce qui est
/// descendu, et une nouvelle restauration du meme dossier reprend ou elle en est.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArretDeLaRestauration {
    /// Le reseau est tombe (`ErreurGoogle::injoignable`).
    HorsLigne,
    /// L'autorisation du compte Google ne vaut plus (`SorteDErreur::Arret`, `autorisation`) : relier le Drive, puis
    /// reprendre.
    AutorisationPerdue,
    /// Google a refuse pour une autre raison que ce fichier-la (`SorteDErreur::Arret`, pas `autorisation` : une limite du
    /// projet, « domainPolicy », la page d'un relais…) : sa phrase est la derniere ligne de `a_retenter` ; reprendre plus
    /// tard.
    RefusDeGoogle,
    /// Le verrou de cette restauration n'est plus le sien (`VerrouDeRestauration`) : un autre geste a pris la main, et
    /// elle n'ecrit plus rien, ni fichier ni index.
    VerrouPerdu,
}

impl Restauration {
    /// LA RESTAURATION A-T-ELLE TOUT REDESCENDU ? Chaque fichier est la, sauf ceux qui ne descendront jamais par cette
    /// voie (`incidents`) : aucun arret en route, aucun fichier a retenter.
    pub fn complete(&self) -> bool {
        self.arret.is_none() && self.a_retenter.is_empty()
    }

    /// D'OU VIENT LE COFFRE `coffre` de la compagnie restauree : descendu du Drive dans cette restauration meme, ou dans
    /// le dossier d'une reprise pas finie (`RepriseEnCours` : ce dossier neuf ne porte que ce qui est venu du Drive) ;
    /// sinon il etait la, et c'est peut-etre le coffre vivant d'une compagnie.
    pub fn origine_du_coffre(&self, coffre: &Coffre) -> OrigineDuCoffre {
        let chemin = coffre.chemin();
        if self.redescendus.iter().any(|f| f.dans(&self.cible) == chemin) || crate::installation::en_reprise(&self.cible) {
            OrigineDuCoffre::DuDrive
        } else {
            OrigineDuCoffre::DejaLa
        }
    }
}

/// D'OU VIENT LE COFFRE QU'UNE RESTAURATION LAISSE (`Restauration::origine_du_coffre`) : seul un coffre descendu du Drive
/// se met de cote (`ranger_le_coffre`) ; un coffre qui etait la n'est jamais deplace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrigineDuCoffre {
    /// Descendu du Drive par cette restauration, ou dans le dossier d'une reprise pas finie.
    DuDrive,
    /// Il etait la avant elle.
    DejaLa,
}

/// CE QU'UN REFUS « DOSSIER NON VIDE » PORTE A PART : le dossier ou la compagnie devait revenir, et chaque fichier deja
/// la qui n'est pas la copie du Drive (son chemin depuis ce dossier, et pourquoi).
#[derive(Debug, Clone, serde::Serialize)]
struct FichiersDejaLa {
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    dossier: PathBuf,
    fichiers: Vec<String>,
}

/// LE NOM VENU DU DRIVE, s'il peut etre un nom de fichier ici sans sortir du dossier : un `NomDeFichier`, et sous
/// Windows, ni deux-points (`C:x` changerait de lecteur) ni ce que Windows refuse ; `None` sinon, que la restauration
/// dit en incident pour ce nom, sans le redescendre.
fn nom_sur(nom: &str) -> Option<NomDeFichier> {
    match NomDeFichier::lire(nom) {
        Ok(n) if !(cfg!(windows) && nom.contains([':', '<', '>', '"', '|', '?', '*'])) => Some(n),
        Ok(_) | Err(_) => None,
    }
}

/// Un fichier de la sauvegarde, tel que l'inventaire du Drive le voit.
struct Entree {
    id: IdDrive,
    md5: Option<EmpreinteMd5>,
    /// Le dossier, relatif a `sauvegarde/` (`None` a la racine).
    dossier: Option<CheminRelatif>,
}

/// Le chemin du nom `nom` dans le dossier `dossier` (`None` a la racine de `sauvegarde/`).
fn dans(dossier: &Option<CheminRelatif>, nom: NomDeFichier) -> CheminRelatif {
    match dossier {
        Some(d) => d.joint(&nom),
        None => CheminRelatif::from(nom),
    }
}

/// Le seul dossier de ce nom sous `parent`, sans rien creer. `Ok(None)` s'il n'y en a pas ; un refus s'il y en a deux.
fn le_dossier(client: &mut dyn Drive, parent: &IdDrive, nom: &str, chemin: &str) -> Result<Option<IdDrive>, ErreurGoogle> {
    let trouves: Vec<IdDrive> = client.sous_dossiers(parent)?.into_iter().filter(|d| d.nom == nom).map(|d| d.id).collect();
    match trouves.len() {
        0 => Ok(None),
        1 => Ok(trouves.into_iter().next()),
        n => Err(ErreurGoogle::nouvelle(
            socle::echec::Classe::SauvegardeAmbigue,
            format!("{} dossiers « {} » dans le Drive : la restauration ne choisit pas. Les reunir a la main dans le Drive, puis relancer.", n, chemin),
        )),
    }
}

/// UNE RESTAURATION A LA FOIS PAR DOSSIER, d'un processus a l'autre (la fenetre sert ses routes en parallele, et `moteur
/// route` est un autre processus) : un fichier cache a cote du dossier cible
/// (`.restauration-<dossier>.verrou`, que `installation::est_cache` ecarte ; `socle::verrou`), pris avant de restaurer
/// ou d'abandonner une reprise, rendu a la fin du geste. `restaurer` l'exige et le rafraichit a chaque fichier : une
/// longue restauration ne passe pas pour abandonnee, et celle qui l'a perdu s'arrete.
pub struct VerrouDeRestauration {
    cible: PathBuf,
    verrou: socle::verrou::Verrou,
}

impl VerrouDeRestauration {
    /// Le verrou du dossier `cible` ; tenu par un autre geste, un refus qui le dit, et rien n'est fait.
    pub fn prendre(cible: &Path) -> socle::echec::R<VerrouDeRestauration> {
        let nom = NomDeFichier::du(cible)?;
        let p = cible.with_file_name(format!(".restauration-{}.verrou", nom));
        let qui = socle::verrou::marque("restauration du Drive", socle::processus::numero(), &socle::paris::ecrit::seconde::texte(socle::temps::secondes()));
        match socle::verrou::Verrou::prendre(&p, &qui, socle::verrou::DELAI_VERROU).map_err(|m| m.reclasser(socle::echec::Classe::Disque))? {
            Ok(verrou) => Ok(VerrouDeRestauration { cible: cible.to_path_buf(), verrou }),
            Err(_) => Err(socle::echec::erreur(socle::echec::Classe::Refuse, "Un autre geste tient déjà ce dossier sur ce poste (une reprise, un abandon) : réessayer quand il aura fini. Rien n'est fait.")),
        }
    }

    /// Le verrou est-il encore le sien ? La date de son fichier repart de maintenant (`Verrou::rafraichir`). Le disque qui
    /// refuse de le relire ou de le dater arrete la restauration et le dit pour ce qu'il est ; ce qui est deja
    /// revenu, chaque appelant le dit (le garder, ou l'effacer avec un dossier neuf).
    fn rafraichir(&self) -> Result<bool, ErreurGoogle> {
        match self.verrou.rafraichir() {
            Ok(r) => Ok(r.is_ok()),
            Err(e) => Err(ErreurGoogle::from(socle::echec::Echec {
                classe: Some(socle::echec::Classe::Disque),
                phrase: format!("{} Le disque refuse le verrou de la restauration : elle s'arrête là.", e.phrase),
                ..e
            })),
        }
    }
}

/// `restaurer` : rend `cible` tel que le dossier `depuis` (de la vue du Drive, sous la racine `racine_drive`) le porte,
/// sous son verrou (`VerrouDeRestauration`, pris sur ce meme dossier). Rend le compte rendu, n'imprime rien.
///
/// Un `Err` est un refus : rien n'a ete ecrit (sauvegarde introuvable ou ambigue, dossier qui n'est pas une sauvegarde,
/// reseau tombe pendant l'inventaire, dossier local qui n'est pas vide ou qui n'existe pas), ou le dossier a disparu en
/// route (rien de plus n'est ecrit : `restaurer` ne cree jamais `cible`), ou le disque a refuse en route (sa phrase dit
/// ce qui est deja revenu, classe `Disque`). Un incident en route (un fichier) n'arrete pas les autres et laisse l'index
/// juste. Le verrou perdu (`ArretDeLaRestauration::VerrouPerdu`), elle n'ecrit plus rien, ni fichier ni index.
pub fn restaurer(cible: &Path, depuis: &DossierVu, racine_drive: &str, client: &mut dyn Drive, verrou: &VerrouDeRestauration) -> Result<Restauration, ErreurGoogle> {
    if verrou.cible != cible {
        return Err(ErreurGoogle::from(socle::echec::erreur(socle::echec::Classe::DefautDuLogiciel, "Défaut du logiciel : la restauration tient le verrou d'un autre dossier. Rien n'est fait.")));
    }
    if !cible.is_dir() {
        return Err(ErreurGoogle::from(socle::echec::erreur(socle::echec::Classe::Refuse, "Le dossier où la compagnie doit revenir n'existe pas (abandonné, ou supprimé) : rien n'est redescendu.")));
    }
    let mut resultat = Restauration {
        cible: cible.to_path_buf(),
        redescendus: Vec::new(),
        incidents: Vec::new(),
        a_retenter: Vec::new(),
        arret: None,
    };

    // 1. L'inventaire du Drive, sans rien y creer : l'arbre du dossier choisi.
    // Le dossier, tel qu'on le voit dans le Drive.
    let chemin = format!("{} › {} › {}", racine_drive, depuis.nom, SAUVEGARDE);
    let (sauvegarde_id, compagnie) = match &depuis.genre {
        GenreDuDossier::Compagnie(_) => {
            let arbre = le_dossier(client, &depuis.id, SAUVEGARDE, &chemin)?.ok_or_else(|| {
                ErreurGoogle::nouvelle(
                    socle::echec::Classe::SauvegardeAbsente,
                    format!("Le dossier « {} » du Drive n'a pas encore de sauvegarde (« {} » ne s'y trouve pas) : rien n'a été redescendu.", depuis.nom, chemin),
                )
            })?;
            (arbre, Some(depuis.id.clone()))
        }
        GenreDuDossier::AncienneSauvegarde => (depuis.id.clone(), None),
        GenreDuDossier::Ancien => {
            return Err(ErreurGoogle::nouvelle(
                socle::echec::Classe::SauvegardeAbsente,
                format!("Le dossier « {} » du Drive n'est pas la sauvegarde d'une compagnie : rien n'a été redescendu.", depuis.nom),
            ))
        }
    };

    let mut distants: BTreeMap<CheminRelatif, Entree> = BTreeMap::new();
    let mut dossiers: BTreeMap<Option<CheminRelatif>, IdDrive> = BTreeMap::new();
    let mut a_voir: Vec<(Option<CheminRelatif>, IdDrive)> = vec![(None, sauvegarde_id)];
    while let Some((dossier, id)) = a_voir.pop() {
        dossiers.insert(dossier.clone(), id.clone());
        // Le nom tel que le Drive le donne, dans son dossier : pour dire celui qui ne peut pas etre un nom ici.
        let ici = |nom: &str| match &dossier {
            Some(d) => format!("{}/{}", d, nom),
            None => nom.to_string(),
        };
        let fichiers = client.lister(&id)?;
        for (nom, _) in fichiers.par_nom() {
            let Some(n) = nom_sur(nom) else {
                resultat.incidents.push(format!("{} : ce nom ne peut pas etre un nom de fichier ici, il n'est pas redescendu.", ici(nom)));
                continue;
            };
            let relatif = dans(&dossier, n);
            if ecarte(&relatif) {
                continue;
            }
            match fichiers.le_seul(nom) {
                Ok(Some(d)) => {
                    distants.insert(relatif, Entree { id: d.id.clone(), md5: d.md5.clone(), dossier: dossier.clone() });
                }
                Ok(None) => {}
                Err(e) => resultat.incidents.push(format!("{} : {} Aucun n'est redescendu : ils restent dans le Drive.", relatif, e.echec.phrase)),
            }
        }
        let mut par_nom: BTreeMap<String, Vec<IdDrive>> = BTreeMap::new();
        for d in client.sous_dossiers(&id)? {
            par_nom.entry(d.nom).or_default().push(d.id);
        }
        for (nom, ids) in par_nom {
            if let Some(n) = nom_sur(&nom) {
                let relatif = dans(&dossier, n);
                match ids.as_slice() {
                    [seul] => a_voir.push((Some(relatif), seul.clone())),
                    _ => resultat.incidents.push(format!(
                        "{}/ : {} dossiers de ce nom dans le Drive ; la restauration ne choisit pas, rien n'en est redescendu : ils restent dans le Drive.",
                        relatif,
                        ids.len()
                    )),
                }
            } else {
                resultat.incidents.push(format!("{}/ : ce nom ne peut pas etre un nom de dossier ici, rien n'en est redescendu.", ici(&nom)));
            }
        }
    }

    // 2. Le dossier local : ce que la sauvegarde y porterait doit deja etre la copie du Drive.
    let (presents, _) = inventaire(cible)?;
    let mut deja: BTreeSet<CheminRelatif> = BTreeSet::new();
    let mut ecarts: Vec<String> = Vec::new();
    for (relatif, _) in &presents {
        let chemin = relatif.dans(&cible);
        let locale = EmpreinteMd5::du_fichier(&chemin).map_err(|e| ErreurGoogle::fichier(&e, &chemin))?;
        match distants.get(relatif) {
            Some(d) if d.md5.as_ref() == Some(&locale) => {
                deja.insert(relatif.clone());
            }
            Some(_) => ecarts.push(format!("{} (differe de la copie du Drive)", relatif)),
            None => ecarts.push(format!("{} (absent de la sauvegarde)", relatif)),
        }
    }
    if !ecarts.is_empty() {
        // La phrase dit le cas en mots ; le dossier et les fichiers, pour qui veut les voir, vont a part. Ce qui se fait
        // ensuite, chaque appelant le dit (une reprise neuve efface son dossier ; une reprise continuee s'abandonne).
        let phrase = format!(
            "Le dossier où la compagnie doit revenir n'est pas vide : {} fichier(s) y sont déjà, et ne sont pas la copie du Drive. Rien n'a été redescendu, rien n'a été effacé.",
            ecarts.len()
        );
        let a_part = FichiersDejaLa { dossier: cible.to_path_buf(), fichiers: ecarts };
        let echec = socle::echec::Echec { a_part: Some(socle::echec::APart::forme(a_part)), ..socle::echec::erreur(socle::echec::Classe::DossierNonVide, phrase) };
        return Err(ErreurGoogle::from(echec));
    }

    // 3. Ce qui manque, verifie avant d'etre ecrit.
    let mut verifies = deja;
    for (relatif, d) in &distants {
        if verifies.contains(relatif) {
            continue;
        }
        let Some(attendue) = d.md5.as_ref() else {
            resultat.incidents.push(format!("{} : le Drive n'en donne pas d'empreinte (un document Google ?), il n'est pas redescendu.", relatif));
            continue;
        };
        // Le verrou se rafraichit a chaque fichier ; perdu (un autre geste a la main), la restauration s'arrete.
        if !verrou.rafraichir()? {
            resultat.arret = Some(ArretDeLaRestauration::VerrouPerdu);
            return Ok(resultat);
        }
        let chemin = relatif.texte().split('/').fold(cible.to_path_buf(), |p, s| p.join(s));
        if socle::fichiers::present(&chemin)? {
            resultat.incidents.push(format!("{} : il existe deja ici, il est garde tel quel.", relatif));
            continue;
        }
        let contenu = match client.telecharger(&d.id) {
            Ok(c) => c,
            Err(e) if e.injoignable => {
                resultat.arret = Some(ArretDeLaRestauration::HorsLigne);
                resultat.a_retenter.push(format!("{} : {}", relatif, e.echec.phrase));
                break;
            }
            // LE SEUL CLASSEMENT DES ERREURS DE GOOGLE (`envois::drive::sorte`) : ce que Google dit de ce fichier-la ne
            // descendra pas par cette voie, et la reprise peut aller au bout ; le passager se retente ; tout le reste
            // arrete la restauration, pas au bout (la cause dite, ce qui la leve : relier le Drive, ou reprendre plus tard).
            Err(e) => match envois::drive::sorte(&e) {
                SorteDErreur::DuFichier => {
                    resultat.incidents.push(format!(
                        "{} : Google ne le donne pas à PuppetPaie (introuvable pour l'application, pas un fichier qui se télécharge, ou restreint). Il reste dans le Drive.",
                        relatif
                    ));
                    continue;
                }
                SorteDErreur::Passagere => {
                    resultat.a_retenter.push(format!("{} : {}", relatif, e.echec.phrase));
                    continue;
                }
                SorteDErreur::Arret { autorisation } => {
                    resultat.arret = Some(if autorisation { ArretDeLaRestauration::AutorisationPerdue } else { ArretDeLaRestauration::RefusDeGoogle });
                    resultat.a_retenter.push(format!("{} : {}", relatif, e.echec.phrase));
                    break;
                }
            },
        };
        // Le dossier a disparu en route (une reprise abandonnee) : rien ne le recree.
        if !cible.is_dir() {
            return Err(disparu());
        }
        let recue = EmpreinteMd5::des_octets(&contenu);
        if recue != *attendue {
            resultat.a_retenter.push(format!(
                "{} : l'empreinte ne colle pas (le Drive annonce {}, le contenu recu donne {}) : rien n'est ecrit ici, il reste dans le Drive.",
                relatif, attendue, recue
            ));
            continue;
        }
        // Le verrou encore le sien juste avant d'ecrire (le telechargement a pu durer) ; perdu, rien de plus ne s'ecrit.
        if !verrou.rafraichir()? {
            resultat.arret = Some(ArretDeLaRestauration::VerrouPerdu);
            resultat.a_retenter.push(format!("{} : un autre geste a pris ce dossier, il n'est pas écrit ici.", relatif));
            return Ok(resultat);
        }
        match ecrire_sans_remplacer(cible, &chemin, &contenu) {
            Ok(true) => {
                resultat.redescendus.push(relatif.clone());
                verifies.insert(relatif.clone());
            }
            Ok(false) => resultat.incidents.push(format!("{} : il est apparu ici pendant la restauration, il est garde tel quel.", relatif)),
            Err(e) => resultat.a_retenter.push(format!("{} : {}", relatif, e.phrase)),
        }
    }

    // 4. L'index, comme la sauvegarde l'ecrit, pour ce qui est verifie seulement ; jamais dans un dossier disparu.
    if !cible.is_dir() {
        return Err(disparu());
    }
    // Ni index sous un verrou perdu.
    if !verrou.rafraichir()? {
        resultat.arret = Some(ArretDeLaRestauration::VerrouPerdu);
        return Ok(resultat);
    }
    let (presents, _) = inventaire(cible)?;
    let mut fichiers = BTreeMap::new();
    let mut index_dossiers = BTreeMap::new();
    for (relatif, etat) in &presents {
        if !verifies.contains(relatif) {
            continue;
        }
        fichiers.insert(relatif.clone(), *etat);
        if let Some(d) = distants.get(relatif) {
            if let Some(id) = dossiers.get(&d.dossier) {
                index_dossiers.insert(DossierDuDrive::de(d.dossier.as_ref()), id.clone());
            }
        }
    }
    // Restauree de l'ancien dossier commun, la compagnie n'a pas de dossier a elle : sa sauvegarde suivante le cree, et
    // l'index n'en garde aucun dossier.
    if compagnie.is_none() {
        index_dossiers.clear();
    }
    let maintenant = socle::temps::secondes();
    let index = Index {
        quand: Some(maintenant.a_la_minute()),
        racine_drive: racine_drive.to_string(),
        compagnie,
        dossiers: index_dossiers,
        fichiers,
        derniere_complete: None,
        echecs: None,
        sans_la_main: None,
    };
    ecrire_index(cible, &index)?;
    Ok(resultat)
}

/// Le refus d'une restauration dont le dossier cible a disparu en route.
fn disparu() -> ErreurGoogle {
    ErreurGoogle::from(socle::echec::erreur(socle::echec::Classe::Refuse, "Le dossier où la compagnie revenait a disparu en route (une reprise abandonnée ?) : rien de plus n'est écrit."))
}

/// Ecrit `contenu` a `chemin` (sous le dossier `cible`) par un `.part` renomme, SANS JAMAIS REMPLACER : `Ok(false)` si
/// `chemin` existe deja. Les sous-dossiers se creent un a un sous `cible`, jamais `cible` lui-meme : un dossier cible
/// disparu entre-temps n'est pas recree, l'ecriture se refuse.
fn ecrire_sans_remplacer(cible: &Path, chemin: &Path, contenu: &[u8]) -> socle::echec::R<bool> {
    let panne = |e: std::io::Error, p: &Path| socle::echec::erreur(socle::echec::Classe::du_disque(&e), socle::fichiers::panne_fichier(&e, p));
    let sous = chemin.parent().and_then(|p| p.strip_prefix(cible).ok()).ok_or_else(|| {
        socle::echec::erreur(socle::echec::Classe::DefautDuLogiciel, format!("Défaut du logiciel : {} n'est pas sous le dossier de la restauration.", chemin.display()))
    })?;
    let mut dossier = cible.to_path_buf();
    for morceau in sous.components() {
        dossier.push(morceau);
        match std::fs::create_dir(&dossier) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && dossier.is_dir() => {}
            Err(e) => return Err(panne(e, &dossier)),
        }
    }
    let mut part = chemin.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    std::fs::write(&part, contenu).map_err(|e| panne(e, &part))?;
    if socle::fichiers::present(chemin)? {
        socle::fichiers::effacer(&part)?;
        return Ok(false);
    }
    std::fs::rename(&part, chemin).map_err(|e| panne(e, chemin))?;
    Ok(true)
}

// ------------------------------------------------------------ ce qu'il reste a faire a la main

/// Le nom sous lequel un coffre revenu que ce poste n'ouvre pas est mis de cote, dans `config/` (`ranger_le_coffre`).
pub const COFFRE_DE_L_ANCIEN_POSTE: &str = "coffre-de-l-ancien-poste";

/// UN COFFRE MIS DE COTE PAR `ranger_le_coffre` (`config/coffre-de-l-ancien-poste.bin`, `-2`…) : propre au poste qui l'a
/// mis de cote, il ne monte pas au Drive (`sauvegarde::ecarte`), et la restauration ne le compte pas comme un fichier
/// present ici et absent du Drive.
pub(crate) fn est_un_coffre_de_cote(relatif: &CheminRelatif) -> bool {
    relatif.texte().strip_prefix("config/").is_some_and(|n| n.starts_with(COFFRE_DE_L_ANCIEN_POSTE) && n.ends_with(".bin") && !n.contains('/'))
}

/// LE COFFRE REVENU QUE CE POSTE N'OUVRE PAS, ET QUE RIEN N'OUVRIRA ICI : scelle par la seule cle de l'ancien poste, sans
/// mot de passe du logiciel (`ouverture_exigee` faux, `ouvre_a_ce_poste` faux). Poser un mot de passe du logiciel se
/// refuserait sur lui (`Coffre::poser_ouverture` l'ouvre d'abord : « Ce coffre ne s'ouvre pas ici ») ; il est donc mis
/// de cote dans `config/` (`coffre-de-l-ancien-poste.bin`, `-2`… s'il y en a deja un), sans rien effacer, et la
/// compagnie part sans coffre : son premier mot de passe du logiciel en fait un neuf. SEUL UN COFFRE DESCENDU DU DRIVE
/// (`OrigineDuCoffre::DuDrive`) se met de cote : un coffre qui etait la est peut-etre le coffre vivant d'une compagnie,
/// qu'un processus sans la cle du poste n'ouvre pas non plus. Rend vrai s'il a ete mis de cote.
pub fn ranger_le_coffre(coffre: &Coffre, origine: OrigineDuCoffre) -> socle::echec::R<bool> {
    let chemin = coffre.chemin();
    if origine == OrigineDuCoffre::DejaLa || !socle::fichiers::present(&chemin)? || coffre.ouverture_exigee()? || coffre.ouvre_a_ce_poste()? {
        return Ok(false);
    }
    let mut n = 1;
    let de_cote = loop {
        let nom = if n == 1 { format!("{}.bin", COFFRE_DE_L_ANCIEN_POSTE) } else { format!("{}-{}.bin", COFFRE_DE_L_ANCIEN_POSTE, n) };
        let p = chemin.with_file_name(nom);
        if !socle::fichiers::present(&p)? {
            break p;
        }
        n += 1;
    };
    std::fs::rename(&chemin, &de_cote).map_err(|e| socle::echec::du_disque(&chemin, &e))?;
    Ok(true)
}

/// Ce qu'il reste a faire a la main apres une restauration, d'apres ce qui est revenu dans `coffre.racine` : le coffre (ce
/// qu'il porte, s'il s'ouvre a ce poste, par quoi). Ne lit aucun secret : les fiches
/// du coffre sont en clair (ce qui est pose, depuis quand), et `ouvre_a_ce_poste` ne rend qu'un oui ou un non. Le coffre
/// est celui du poste (`Coffre::du_poste`) ; la preuve en donne un dont la cle est ailleurs.
///
/// En mots, pour la personne : les gestes se nomment par l'endroit ou ils se font (Configuration › Mots de passe) ; leurs
/// routes, la fiche de chaque route les dit. Un coffre revenu que rien n'ouvrira ici est d'abord mis de cote
/// (`ranger_le_coffre`, s'il est descendu du Drive : `origine`) : chaque ligne dit un geste qui se fait vraiment. Un coffre
/// qui ne se lit pas se dit, en mots ; il n'est pas touche.
pub fn a_faire(coffre: &Coffre, origine: OrigineDuCoffre) -> Vec<String> {
    let mut lignes = Vec::new();
    let a_poser = |manquants: &[&str]| -> String {
        SERVICES.iter().filter(|s| manquants.contains(&s.nom)).map(|s| s.libelle).collect::<Vec<_>>().join(", ")
    };
    let tous: Vec<&str> = SERVICES.iter().map(|s| s.nom).collect();
    const OU: &str = "Configuration › Mots de passe";
    const ILLISIBLE: &str = "Le coffre est revenu, mais il ne se lit pas sur ce poste : il est peut-être abîmé. Rien n'y est changé.";
    let range = match ranger_le_coffre(coffre, origine) {
        Ok(r) => r,
        Err(e) if e.classe == Some(socle::echec::Classe::Disque) => {
            return vec!["Le coffre revenu ne s'ouvre pas ici, et le disque refuse de le mettre de côté : rien n'y est changé.".to_string()];
        }
        Err(_) => return vec![ILLISIBLE.to_string()],
    };
    if range {
        lignes.push(format!(
            "Le coffre revenu ne s'ouvre pas ici : seul l'ancien poste l'ouvrait. Il est mis de côté sur ce poste seulement, sous le nom « coffre de l'ancien poste », rien n'est effacé ; il ne monte pas au Drive. Poser un mot de passe du logiciel, puis ceux des organismes ({}), dans {}.",
            a_poser(&tous),
            OU
        ));
    } else if !coffre.chemin().exists() {
        lignes.push(format!(
            "Le coffre n'était pas dans la sauvegarde : poser le mot de passe du logiciel, puis ceux des organismes ({}), dans {}.",
            a_poser(&tous),
            OU
        ));
    } else {
        match (coffre.ouverture_exigee(), coffre.services(), coffre.ouvre_a_ce_poste()) {
            (Err(_), _, _) | (_, Err(_), _) | (_, _, Err(_)) => lignes.push(ILLISIBLE.to_string()),
            (Ok(avec_mot_de_passe), Ok(etat), Ok(ici)) => {
                let poses: Vec<String> = etat
                    .iter()
                    .filter(|f| f.pose)
                    .map(|f| match &f.pose_le {
                        Some(le) => format!("{} (posé le {})", f.service, socle::paris::ecrit::saisie::texte(*le)),
                        None => f.service.to_string(),
                    })
                    .collect();
                let manquants: Vec<&str> = etat.iter().filter(|f| !f.pose).map(|f| f.service).collect();
                let porte = if poses.is_empty() { "aucun mot de passe d'organisme".to_string() } else { poses.join(", ") };
                if ici {
                    lignes.push(format!("Le coffre est revenu, et ce poste l'ouvre. Il porte : {}.", porte));
                } else if !avec_mot_de_passe {
                    // Il etait la avant la restauration : il n'est pas deplace (`ranger_le_coffre`).
                    lignes.push(format!(
                        "Le coffre de ce dossier, qui y était déjà, ne s'ouvre pas sur ce poste et n'a pas de mot de passe du logiciel : seul le poste qui l'a fait l'ouvre. Rien n'y est changé. Il porte : {}.",
                        porte
                    ));
                } else {
                    lignes.push(format!(
                        "Le coffre est revenu ; il porte : {}. Ce poste ne l'ouvre pas seul (la clé de l'ancien poste ne voyage pas) : le mot de passe du logiciel de cette compagnie, oui. Dans {}, « Changer… » le mot de passe du logiciel (l'actuel, puis le nouveau, qui peut être le même) l'attache à ce poste.",
                        porte,
                        OU
                    ));
                }
                if !manquants.is_empty() {
                    lignes.push(format!("Ce qu'il ne porte pas, à poser s'il le faut dans {} : {}.", OU, a_poser(&manquants)));
                }
            }
        }
    }
    lignes
}

// ------------------------------------------------------------ la main, au bout d'une reprise

/// LA MAIN SUR LE DOSSIER DU DRIVE QU'UNE REPRISE ALLEE AU BOUT GARDE (« Reprendre » la prend d'office, Leandre,
/// 25/11/2026, #594) : ce que `prendre_la_main` en a fait. La reprise a l'accueil (`routes::accueil::drive`) passe
/// par elle.
#[derive(Debug)]
pub enum MainDeLaReprise {
    /// Rien a prendre : le dossier repris est l'ancienne sauvegarde commune (la premiere sauvegarde de la compagnie fera
    /// son dossier, avec la main), ou la compagnie n'envoie rien au Drive (fictive, demo).
    PasAPrendre,
    /// Ce poste la tenait deja : rien n'a change sur le Drive.
    DejaLaSienne,
    /// Ce poste vient de la prendre.
    Prise,
    /// Elle n'est pas prise, et pourquoi.
    PasPrise(ErreurGoogle),
}

/// PREND LA MAIN SUR LE DOSSIER `vu`, celui d'une compagnie revenue au bout dans `cible`, pour ce poste.
pub fn prendre_la_main(cible: &Path, vu: &DossierVu, client: &mut dyn Drive) -> MainDeLaReprise {
    let GenreDuDossier::Compagnie(identite) = &vu.genre else {
        return MainDeLaReprise::PasAPrendre;
    };
    match crate::bridage::branchement(cible) {
        Ok(envois::branchement::Branchement::Monde) => {}
        Ok(_) => return MainDeLaReprise::PasAPrendre,
        Err(e) => return MainDeLaReprise::PasPrise(ErreurGoogle::from(e)),
    }
    let main = match crate::sauvegarde::LaMain::de_ce_poste(cible) {
        Ok(m) => m,
        Err(e) => return MainDeLaReprise::PasPrise(ErreurGoogle::from(e)),
    };
    if matches!(&vu.main, Ok(Some(m)) if m.poste == main.poste) {
        return MainDeLaReprise::DejaLaSienne;
    }
    match client.marquer(&vu.id, &main.marque(identite)) {
        Ok(()) => MainDeLaReprise::Prise,
        Err(e) => MainDeLaReprise::PasPrise(e),
    }
}

impl MainDeLaReprise {
    /// Ce que la fin de la reprise en dit (rien quand il n'y avait rien a prendre), selon la main que portait le dossier
    /// repris `vu` avant elle.
    pub fn phrase(&self, vu: &DossierVu) -> Option<String> {
        use crate::sauvegarde::{main_illisible_dite, PRENDRE_LA_MAIN_OU};
        const SEUL: &str = "Ce poste tient maintenant la main sur son Drive : lui seul y sauvegarde.";
        Some(match self {
            MainDeLaReprise::PasAPrendre => return None,
            MainDeLaReprise::DejaLaSienne => "Ce poste tenait déjà la main sur son Drive : lui seul y sauvegarde.".to_string(),
            MainDeLaReprise::Prise => match &vu.main {
                Ok(Some(m)) => format!(
                    "{} Le poste qui la tenait depuis le {} n'y sauvegarde plus, et le dit ; {} la lui rend.",
                    SEUL,
                    m.depuis.jour_de_paris().jj_mm(),
                    PRENDRE_LA_MAIN_OU
                ),
                Ok(None) => format!("{} Aucun poste ne la tenait.", SEUL),
                Err(e) => format!("{} Avant elle : {}", SEUL, main_illisible_dite(e)),
            },
            MainDeLaReprise::PasPrise(e) => match &vu.main {
                Ok(Some(m)) => format!(
                    "{} Le poste qui la tient depuis le {} la garde : ce poste ne sauvegarde pas tant qu'il ne la prend pas, par {}.",
                    e.echec.phrase,
                    m.depuis.jour_de_paris().jj_mm(),
                    PRENDRE_LA_MAIN_OU
                ),
                Ok(None) => format!("{} Aucun poste ne la tenait : la première sauvegarde de ce poste la prend.", e.echec.phrase),
                Err(illisible) => format!(
                    "{} {} Ce poste ne sauvegarde pas tant qu'il ne la prend pas, par {}.",
                    e.echec.phrase,
                    main_illisible_dite(illisible),
                    PRENDRE_LA_MAIN_OU
                ),
            },
        })
    }

    /// LA MAIN PAS PRISE, DANS LES INCIDENTS DE LA REPRISE : la ligne qu'elle y met, et son detail chez l'editeur par le
    /// journal des incidents de `cible`, comme la sauvegarde note les siens (une panne de Google ou du reseau ne s'y
    /// note pas : elle ne dit rien du logiciel). Rien quand elle est prise, ou qu'il n'y avait rien a prendre.
    pub fn noter(&self, cible: &Path) -> Vec<String> {
        let MainDeLaReprise::PasPrise(e) = self else {
            return Vec::new();
        };
        let mut lignes = vec![format!("La main sur le dossier du Drive ne s'est pas prise : {}", e.echec.phrase)];
        if !(e.injoignable || envois::drive::passagere(e) || envois::suites::panne_de_passage(&e.echec)) {
            if let Err(n) = crate::incidents::depuis_exception(cible, e.echec.classe, &e.echec.phrase, None, "reprise", e.echec.a_part.as_ref(), None) {
                lignes.push(format!("La main pas prise : le journal des incidents ne l'a pas notée : {}", n.phrase));
            }
        }
        lignes
    }
}
