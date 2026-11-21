//! La releve des retours des organismes et le client qui leur parle (net-entreprises pour la DSN, l'Urssaf pour la DPAE).
//!
//! RELEVER EST UNE LECTURE : on archive sans rien demander a personne, aucune seance n'est exigee, rien
//! n'est applique. L'archive brute est la piece justificative : elle est ecrite avant toute analyse. Un 404
//! sur une tranche veut dire << rien sur cette tranche >>, tout autre refus arrete (atelier/AGENTS.md, regle 5).
//!
//! CE MODULE NE CHERCHE JAMAIS UN SECRET (regle 3) : `Client` recoit le mot de passe, que `moteur` prend a
//! la variable d'environnement du service ou au coffre. Le mot de passe sert le temps de l'authentification
//! (`transport::authentifier`), n'est ni journalise, ni affiche, ni renvoye.
//!
//! La releve obeit au branchement : le simulateur pose les retours des depots joues, sans reseau (`rattacher_simulation`) ;
//! le message ne releve rien. Ce qu'elle trouve en chemin et qui touche a la compagnie (les incidents des retours, les
//! attestations France Travail a ranger) va dans les suites (`suites::Suites`).

use socle::secret::Secret;
use droit::fiche::Lue as _;
use std::path::{Path, PathBuf};

use droit::contexte::{Contexte, EffectifEcrit, FichierEffectifs, TypeDeParametreNeores};
use std::collections::BTreeMap;
use socle::echec::R;
use socle::fichiers::CheminRelatif;
use socle::mois::Annee;
use socle::echec::erreur;
use socle::echec::{Classe, Echec};
use crate::retours::{Annonce, Depot, IdDeFlux, NatureDuRetour, StatutDuRetour, Verdict};
use socle::date::Jour;
use socle::grandeurs::Effectif;
use socle::paris::Moment;
use socle::garde_reseau::Url;

use crate::branchement::Branchement;
use crate::journal::{Acte, Ligne, RetourOrganisme};
use crate::suites::Suites;

/// `AAAAMMJJ-HHMMSS` d'un instant, a l'heure de Paris.
fn horodatage_archive(instant: socle::paris::Moment) -> String {
    let c = socle::paris::ecrit::compacte::texte(instant);
    format!("{}-{}", &c[..8], &c[8..])
}

// -------------------------------------------------------------------- les pannes

/// Une panne de la plate-forme : `crate::transport::EchecApi` (son refus, et le statut HTTP quand il y en a un).
pub use crate::transport::EchecApi as Panne;

/// Une panne sans statut HTTP.
#[track_caller]
fn panne(classe: Classe, message: String) -> Panne {
    Panne::from(socle::echec::erreur(classe, message))
}

/// Une liste que la plate-forme rend et qui ne se lit pas : sa classe, ou `Illisible` quand le refus n'en dit pas.
fn de_lecture(e: Echec) -> Panne {
    let classe = match e.classe {
        Some(c) => c,
        None => Classe::Illisible,
    };
    Panne::from(e.reclasser(classe))
}

// ----------------------------------------------------------------------- le client

pub const SERVICE_DSN_REGIME_GENERAL: &str = "25";
pub const SERVICE_DSN_REGIME_GENERAL_TEST: &str = "97";
pub const VARIABLE_MOTDEPASSE: &str = "NET_ENTREPRISES_MOTDEPASSE";
pub const VARIABLE_MOTDEPASSE_DPAE: &str = "URSSAF_MOTDEPASSE";

/// Les adresses d'un service (`netentreprises.ADRESSES`, `dpae_api.ADRESSES_DPAE`).
#[derive(Clone)]
pub struct Adresses {
    pub authentification: Url,
    pub retours: Url,
    pub retours_flux: Url,
    pub depot: Url,
    pub service: Option<String>,
}

/// LA PLATE-FORME DE NET-ENTREPRISES a laquelle le client de la DSN parle : la production, ou la plate-forme d'essai de
/// l'editeur (rien n'y est declare). Une liste du logiciel, ecrite `production`, `editeur` (la ligne de commande la
/// nomme ainsi, `NOMS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironnementNetEntreprises {
    Production,
    Editeur,
}

impl EnvironnementNetEntreprises {
    /// Leurs noms, dans l'ordre, tels que la ligne de commande les propose.
    pub const NOMS: &'static [&'static str] = &["production", "editeur"];

    /// Son nom.
    pub fn nom(self) -> &'static str {
        match self {
            EnvironnementNetEntreprises::Production => "production",
            EnvironnementNetEntreprises::Editeur => "editeur",
        }
    }

    /// La plate-forme que `texte` nomme ; un autre texte se refuse en le nommant.
    pub fn lire(texte: &str) -> R<EnvironnementNetEntreprises> {
        match texte {
            "production" => Ok(EnvironnementNetEntreprises::Production),
            "editeur" => Ok(EnvironnementNetEntreprises::Editeur),
            autre => Err(erreur(Classe::Illisible, format!("environnement inconnu : '{}'", autre))),
        }
    }
}

impl std::fmt::Display for EnvironnementNetEntreprises {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.nom())
    }
}

fn adresses_dsn(environnement: EnvironnementNetEntreprises) -> Adresses {
    // L'atelier seulement : un faux net-entreprises local (`redirection`). Le service reste celui de l'environnement.
    #[cfg(feature = "atelier")]
    if let Some(b) = crate::redirection::base(crate::redirection::VARIABLE_DSN) {
        let service = match environnement {
            EnvironnementNetEntreprises::Editeur => SERVICE_DSN_REGIME_GENERAL_TEST,
            EnvironnementNetEntreprises::Production => SERVICE_DSN_REGIME_GENERAL,
        };
        return Adresses {
            authentification: b.joindre("/authentifier/"),
            retours: b.joindre("/lister-retours-declarant/1.0/"),
            retours_flux: b.joindre("/lister-retours-flux/1.0/"),
            depot: b.joindre("/deposer-dsn/1.0/"),
            service: Some(service.into()),
        };
    }
    let (auth, hote_consultation, depot, service) = match environnement {
        EnvironnementNetEntreprises::Production => (
            Url::du_moteur("https://services.net-entreprises.fr/authentifier/1.0/"),
            Url::du_moteur("https://consultation.dsnrg.net-entreprises.fr"),
            Url::du_moteur("https://depot.dsnrg.net-entreprises.fr/deposer-dsn/1.0/"),
            SERVICE_DSN_REGIME_GENERAL,
        ),
        EnvironnementNetEntreprises::Editeur => (
            Url::du_moteur("https://test-services.net-entreprises.fr/authentifier/1.0/"),
            Url::du_moteur("https://consultation-edit.dsnrg.net-entreprises.fr"),
            Url::du_moteur("https://depot-edit.dsnrg.net-entreprises.fr/deposer-dsn/1.0/"),
            SERVICE_DSN_REGIME_GENERAL_TEST,
        ),
    };
    Adresses {
        authentification: auth,
        retours: hote_consultation.joindre("/lister-retours-declarant/1.0/"),
        retours_flux: hote_consultation.joindre("/lister-retours-flux/1.0/"),
        depot,
        service: Some(service.into()),
    }
}

/// LE SERVICE DE L'AUTHENTIFICATION A LA DPAE : « Pour un déclarant ou tiers déclarant, il convient de paramétrer le
/// service 25. Pour un concentrateur, le service 98 doit être paramétré. » (FAQ de l'API DPAE,
/// `atelier/reference/norme-dpae/5493-API-DPAE-FAQ.pdf`, « Quel service faut-il paramétrer dans le module API ? »). Une
/// compagnie declare pour elle-meme : c'est 25. Sans lui, l'Urssaf repond 404 a l'authentification.
const SERVICE_DPAE_DU_DECLARANT: &str = "25";

fn adresses_dpae(compte: Option<&droit::fiche::ApiDpae>) -> Adresses {
    // Le service que la fiche dit (un concentrateur dit le sien) ; une compagnie n'en dit pas : celui du declarant.
    let service = match compte.and_then(|c| c.service.dite()).filter(|s| !s.is_empty()) {
        Some(s) => Some(s.clone()),
        None => Some(SERVICE_DPAE_DU_DECLARANT.to_string()),
    };
    // L'atelier seulement : un faux Urssaf local (`redirection`).
    #[cfg(feature = "atelier")]
    if let Some(b) = crate::redirection::base(crate::redirection::VARIABLE_DPAE) {
        return Adresses {
            authentification: b.joindre("/authentifier_dpae"),
            retours: b.joindre("/lister-retours-declarant/1.0/"),
            retours_flux: b.joindre("/lister-retours-flux/1.0/"),
            depot: b.joindre("/deposer-dsn/1.0/"),
            service,
        };
    }
    Adresses {
        authentification: Url::du_moteur("https://mon.urssaf.fr/authentifier_dpae"),
        retours: Url::du_moteur("https://consultation.dpae-edi.urssaf.fr/lister-retours-declarant/1.0/"),
        retours_flux: Url::du_moteur("https://consultation.dpae-edi.urssaf.fr/lister-retours-flux/1.0/"),
        depot: Url::du_moteur("https://depot.dpae-edi.urssaf.fr/deposer-dsn/1.0/"),
        service,
    }
}

/// `netentreprises.Client` et `dpae_api.ClientDpae`.
pub struct Client {
    /// Le canal de l'organisme auquel ce client parle : ses pannes le nomment (`transport::panne_dite`).
    pub canal: CanalDesRetours,
    pub adresses: Adresses,
    pub jeton: Option<Secret>,
    pub declarant: crate::transport::Identite,
    motdepasse: Option<Secret>,
    majuscules: bool,
}

impl Client {
    /// `Client(ctx, environnement, motdepasse=...)` : le compte net-entreprises de la DSN.
    pub fn dsn(ctx: &Contexte, environnement: EnvironnementNetEntreprises, motdepasse: Option<Secret>) -> Client {
        let declarant = crate::transport::declarant(&ctx.entreprise);
        Client {
            canal: CanalDesRetours::Dsn,
            adresses: adresses_dsn(environnement),
            jeton: None,
            declarant,
            motdepasse,
            majuscules: false,
        }
    }

    /// `ClientDpae(ctx, majuscules, motdepasse)` : le compte Urssaf, qui n'est pas celui de la DSN.
    pub fn dpae(ctx: &Contexte, majuscules: bool, motdepasse: Option<Secret>) -> Client {
        // Sans compte DPAE dit, le declarant est l'entreprise, sans nom ni prenom, et le service n'est pas dit.
        let compte = ctx.entreprise.api_dpae.dite();
        let siret = compte.and_then(|c| c.siret.dite()).or_else(|| ctx.entreprise.siret.dite()).cloned();
        Client {
            canal: CanalDesRetours::Dpae,
            adresses: adresses_dpae(compte),
            jeton: None,
            declarant: crate::transport::Identite { siret, nom: compte.and_then(|c| c.nom.en_texte()), prenom: compte.and_then(|c| c.prenom.en_texte()) },
            motdepasse,
            majuscules,
        }
    }

    /// Le mot de passe que l'appelant a donne (le moteur le prend a l'environnement ou au coffre) : le client n'en
    /// cherche aucun.
    fn secret(&self) -> Option<&Secret> {
        self.motdepasse.as_ref().filter(|m| !m.devoiler().is_empty())
    }

    /// `ClientDpae.verifier_identifiants` : ce qui ferait echouer la connexion MtoM. Le mot de passe ne
    /// quitte pas le client : on ne sait que s'il porte un `@`.
    pub fn verifier_identifiants(&self) -> Result<Vec<String>, Echec> {
        let arobase = self.secret().is_some_and(|m| m.devoiler().contains('@'));
        crate::depot::identifiants(&self.declarant, arobase)
    }

    /// `Client.authentifier` : ouvre une session. Le mot de passe ne survit pas a cet appel.
    pub fn authentifier(&mut self) -> Result<(), Panne> {
        let mdp = self.secret().map(|m| if self.majuscules { m.transforme(str::to_uppercase) } else { m.clone() });
        let session = crate::transport::authentifier(&crate::transport::Authentification {
            canal: self.canal,
            url: &self.adresses.authentification,
            declarant: &self.declarant,
            motdepasse: mdp.as_ref(),
            service: self.adresses.service.as_deref(),
            jeton: self.jeton.as_ref(),
        })?;
        self.jeton = Some(session.jeton);
        Ok(())
    }

    pub fn assurer_session(&mut self) -> Result<(), Panne> {
        // Un jeton vide n'ouvre pas de session.
        if !self.jeton.as_ref().is_some_and(|j| !j.devoiler().is_empty()) {
            self.authentifier()?;
        }
        Ok(())
    }

    fn appel(&self, url: &Url) -> Result<(Vec<u8>, crate::transport::Entetes), Panne> {
        Ok(crate::transport::appeler(self.canal, url, self.jeton.as_ref())?)
    }

    /// La consultation d'une plage.
    fn consulter(&mut self, base: &Url, debut: Moment, fin: Option<Moment>) -> Result<Vec<u8>, Panne> {
        self.assurer_session()?;
        Ok(crate::transport::consulter(self.canal, base, self.jeton.as_ref(), debut, fin)?)
    }

    pub fn lister_retours(&mut self, debut: Moment, fin: Moment) -> Result<Vec<Annonce>, Panne> {
        let base = self.adresses.retours.clone();
        let brut = self.consulter(&base, debut, Some(fin))?;
        crate::retours::liste_retours_d_octets(&brut).map_err(de_lecture)
    }

    /// `Client.lister_depots` : les depots que net-entreprises a vus sur une plage. L'adresse est celle de la consultation
    /// des retours, dont seul le service change (`lister-depots/1.0/` au lieu de `lister-retours-declarant/1.0/`).
    pub fn lister_depots(&mut self, debut: Moment, fin: Moment) -> Result<Vec<Depot>, Panne> {
        let base = Url::lire(&self.adresses.retours.texte().replace("lister-retours-declarant", "lister-depots"))?;
        let brut = self.consulter(&base, debut, Some(fin))?;
        crate::retours::liste_depots_d_octets(&brut).map_err(de_lecture)
    }

    pub fn lister_retours_du_flux(&mut self, idflux: &IdDeFlux) -> Result<Vec<Annonce>, Panne> {
        self.assurer_session()?;
        let (brut, _) = self.appel(&self.adresses.retours_flux.joindre(idflux.texte()))?;
        crate::retours::liste_retours_d_octets(&brut).map_err(de_lecture)
    }

    /// Recupere un retour : (contenu brut, type de contenu, s'il est dit).
    pub fn telecharger(&mut self, url: &Url) -> Result<(Vec<u8>, Option<String>), Panne> {
        self.assurer_session()?;
        let (brut, entetes) = self.appel(url)?;
        let ctype = crate::transport::entete(&entetes, "Content-Type").map(str::to_string);
        Ok((brut, ctype))
    }

    /// L'envoi du guide (text/plain, gzip, jeton DSNLogin) des octets deja prepares. `forme` : `dsn` (`DepotRefuse`)
    /// ou `dpae` (`Organisme`). `au_jour` : la porte du depot, ouverte le jour reel de la machine. Rend la reponse et son
    /// type de contenu, s'il est dit.
    pub fn deposer_octets(&mut self, au_jour: &crate::branchement::AuJourDuJour, url: &Url, contenu: &[u8], forme: &str) -> Result<(String, Option<String>), Panne> {
        self.assurer_session()?;
        let corps = gzip(contenu)?;
        let jeton = self.jeton.clone().ok_or_else(|| panne(Classe::Disque, "La session ouverte n'a pas de jeton.".into()))?;
        let (brut, entetes) = crate::transport::depot(au_jour, url, &jeton, &corps, forme, crate::transport::DELAI_DEPOT)?;
        let texte = String::from_utf8_lossy(&brut).trim_matches(socle::texte::blanc).to_string();
        let ctype = crate::transport::entete(&entetes, "Content-Type").map(str::to_string);
        Ok((texte, ctype))
    }
}

/// `gzip.compress`.
fn gzip(donnees: &[u8]) -> Result<Vec<u8>, Panne> {
    use std::io::Write;
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(donnees).map_err(|x| panne(Classe::Disque, x.to_string()))?;
    e.finish().map_err(|x| panne(Classe::Disque, x.to_string()))
}

// ------------------------------------------------------------------ la releve

/// Un retour telecharge et archive.
pub struct RetourArchive {
    pub retour: Annonce,
    pub chemin: PathBuf,
    pub contenu: Vec<u8>,
}

/// `ReleveRetours`.
pub struct Releve {
    pub trouves: Vec<Annonce>,
    pub nouveaux: Vec<Annonce>,
    pub archives: Vec<RetourArchive>,
    /// CE QUI A EMPECHE DE LIRE LA PLATE-FORME, des incidents que l'appelant dit (`ordre_signalements::relever_la_plateforme`
    /// en une phrase, la releve de toutes les natures en tete de ce qu'elle a a dire) : la liste des retours (le mot de
    /// passe, l'authentification, la consultation), ou un bilan (nature 11) qui ne s'est pas telecharge. Un autre retour
    /// rate n'y est pas : il ne dit rien de ce que la plate-forme sait des ordres, et se dit dans les suites
    /// (`Suites::dit`).
    pub plateforme_non_lue: Vec<String>,
    /// Combien de ces retours ont trouve le depot auquel ils repondent.
    pub rattaches: usize,
}

impl Releve {
    /// Un releve ou rien n'est encore trouve.
    pub fn vide() -> Releve {
        Releve { trouves: Vec::new(), nouveaux: Vec::new(), archives: Vec::new(), plateforme_non_lue: Vec::new(), rattaches: 0 }
    }
}

/// Le jour de publication d'un retour, en JJ/MM/AAAA ; aucun quand il ne le dit pas.
pub fn jour_de_publication_dit(r: &Annonce) -> Option<String> {
    r.publication.map(|p| p.jour_de_paris().jj_mm_aaaa())
}

// ------------------------------------------------------------------ l'archive des retours

/// LE CANAL D'UN RETOUR D'ORGANISME (regle 5 de `atelier/AGENTS.md` : deux canaux de retours, services, mots de passe et
/// archives distincts) : la DSN sur net-entreprises, la DPAE sur `consultation.dpae-edi.urssaf.fr`. Une liste du
/// logiciel ; il ne s'ecrit dans aucun fichier (s'il s'ecrivait, ce serait `dsn` ou `dpae`) ; `dossier` dit ou
/// s'archivent ses pieces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CanalDesRetours {
    Dsn,
    Dpae,
}

impl CanalDesRetours {
    /// Le canal des retours d'un envoi : la DPAE a le sien, tout le reste passe par celui de la DSN.
    pub fn de_l_acte(acte: Acte) -> CanalDesRetours {
        match acte {
            Acte::Dpae => CanalDesRetours::Dpae,
            _ => CanalDesRetours::Dsn,
        }
    }

    /// L'organisme qui sert ce canal, tel qu'une phrase le nomme en tete (`transport::panne_dite`).
    pub fn organisme(self) -> &'static str {
        match self {
            CanalDesRetours::Dsn => "net-entreprises",
            CanalDesRetours::Dpae => "L'Urssaf (DPAE)",
        }
    }

    /// Le service dont le mot de passe ouvre ce canal, tel qu'une phrase le nomme apres « le mot de passe de ».
    pub fn service_du_mot_de_passe(self) -> &'static str {
        match self {
            CanalDesRetours::Dsn => "net-entreprises",
            CanalDesRetours::Dpae => "l'Urssaf (DPAE)",
        }
    }

    /// La declaration que ce canal depose, telle qu'une phrase la nomme (`transport::panne_dite`).
    pub fn declaration(self) -> &'static str {
        match self {
            CanalDesRetours::Dsn => "la DSN",
            CanalDesRetours::Dpae => "la DPAE",
        }
    }

    /// Ou se relit l'avis de rejet d'un depot dont la reponse ne donne pas les motifs : la DSN, dans les retours (« L'AEE
    /// et l'avis de rejet sont téléchargeables à nouveau par les services de gestion des retours », guide de l'API DSN
    /// v6.9, § 3.2.2, l. 1338-1340) ; la DPAE, sur le tableau de bord du declarant (« L'avis de rejet ou l'accusé
    /// d'enregistrement électronique sont mis à disposition sur le tableau de bord du déclarant »,
    /// `atelier/reference/norme-dpae/5491-API-DPAE-Fonctionnement.pdf`, p. 6).
    pub fn ou_se_relit_l_avis_de_rejet(self) -> &'static str {
        match self {
            CanalDesRetours::Dsn => "ses motifs se relisent dans les retours de net-entreprises.",
            CanalDesRetours::Dpae => "ses motifs sont sur le tableau de bord du compte Urssaf.",
        }
    }

    /// Le sous-dossier de `retours/` ou s'archivent ses pieces : aucun pour la DSN (`retours/` meme), `dpae` pour la DPAE.
    pub fn dossier(self) -> Option<&'static str> {
        match self {
            CanalDesRetours::Dsn => None,
            CanalDesRetours::Dpae => Some("dpae"),
        }
    }

    /// Le dossier de ses pieces sous la racine d'une compagnie : `retours/` (la DSN) ou `retours/dpae/` (la DPAE).
    pub fn chemin(self, racine: &Path) -> PathBuf {
        let p = racine.join(crate::retours::RETOURS);
        match self.dossier() {
            Some(d) => p.join(d),
            None => p,
        }
    }
}

/// `retours/` (la DSN) ou `retours/dpae/` (la DPAE), cree.
fn dossier_archive(racine: &Path, canal: CanalDesRetours) -> R<PathBuf> {
    let p = canal.chemin(racine);
    std::fs::create_dir_all(&p).map_err(|e| crate::etats::erreur_io(&e, &p))?;
    Ok(p)
}

/// La piece telle quelle, nommee `AAAAMMJJ-hhmmss_natureN_flux.ext` ; l'heure est celle de la publication, ou
/// `maintenant` (`AAAAMMJJ-hhmmss`) quand le retour n'en porte pas ; `sansflux` quand il ne dit pas son flux.
/// L'extension suit le type de contenu, sinon le contenu (`<` en tete : du XML). Une piece deja archivee sous ce nom
/// n'est pas reecrite.
pub(crate) fn archiver_retour(racine: &Path, canal: CanalDesRetours, retour: &Annonce, contenu: &[u8], ctype: Option<&str>, maintenant: &str) -> R<PathBuf> {
    let dit = |m: &str| ctype.is_some_and(|c| c.contains(m));
    let ext = if dit("pdf") {
        ".pdf"
    } else {
        let tete = &contenu[..contenu.len().min(200)];
        let utile = tete.iter().position(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)).map(|i| &tete[i..]);
        if dit("xml") || utile.is_some_and(|u| u.starts_with(b"<")) {
            ".xml"
        } else {
            ".txt"
        }
    };
    let nom = format!("{}{}", nom_d_archive(retour, maintenant), ext);
    let dest = dossier_archive(racine, canal)?.join(nom);
    // L'archive se relit du disque (`piece_deja_la`) : elle n'y est qu'entiere, posee d'un seul renommage.
    if !dest.exists() {
        let provisoire = dest.with_extension("part");
        std::fs::write(&provisoire, contenu).map_err(|e| crate::etats::erreur_io(&e, &provisoire))?;
        std::fs::rename(&provisoire, &dest).map_err(|e| crate::etats::erreur_io(&e, &dest))?;
    }
    Ok(dest)
}

/// La piece d'un retour, archivee a l'heure de la releve.
fn archiver(ctx: &Contexte, retour: &Annonce, contenu: &[u8], ctype: Option<&str>, canal: CanalDesRetours) -> R<PathBuf> {
    archiver_retour(&ctx.racine, canal, retour, contenu, ctype, &horodatage_archive(socle::temps::secondes()))
}

/// Le journal des retours vus sur le canal.
fn journal_des_vus(ctx: &Contexte, canal: CanalDesRetours) -> R<PathBuf> {
    Ok(dossier_archive(&ctx.racine, canal)?.join("journal.json"))
}

/// Les retours deja vus, tels que le journal du canal les garde (absent : aucun).
fn lire_vus(p: &Path) -> R<Vec<Annonce>> {
    if !p.exists() {
        return Ok(Vec::new());
    }
    socle::forme::lire_fichier(p, "Le journal des retours vus ne se lit pas.")
}

/// Ajoute au journal du canal les retours qu'il n'a pas encore vus : un retour vu ne se retelecharge plus.
pub fn noter_vus(ctx: &Contexte, entrees: &[Annonce], canal: CanalDesRetours) -> R<()> {
    let p = journal_des_vus(ctx, canal)?;
    let mut deja = lire_vus(&p)?;
    let connus: Vec<CleDeRetour> = deja.iter().map(crate::etats::cle_du_contenu).collect();
    for e in entrees {
        if !connus.contains(&crate::etats::cle_du_contenu(e)) {
            deja.push(e.clone());
        }
    }
    socle::ecriture::ecrire_json(&p, &deja, 2)
}

/// La cle d'un retour : son flux, sa nature, sa publication.
type CleDeRetour = crate::etats::CleDuContenu;

// ------------------------------------------------------------------ la plage, et jusqu'ou le releve complet a lu

/// LA PLAGE D'UN RELEVE.
pub enum PlageDuReleve {
    /// LE RELEVE COMPLET (celui de l'ouverture) : toutes les natures, depuis l'instant jusqu'ou le releve complet
    /// precedent du canal a lu (`ReleveComplet`), jusqu'a maintenant ; il avance cet instant. Sans releve complet
    /// precedent, ou s'il remonte plus loin, la plage remonte de `jusqu_a` avant maintenant.
    Complete { jusqu_a: std::time::Duration },
    /// UNE PLAGE BORNEE, qui n'avance rien (suivre un depot, le releve avant un ordre, l'enquete de l'editeur). `nature` :
    /// la seule nature de retour que ce releve prend (le releve avant un ordre ne lit que les bilans) ; aucune : toutes.
    /// Un retour d'une autre nature n'est ni trouve, ni telecharge, ni note vu : le releve suivant de toutes les natures
    /// le prend, avec ses suites.
    Bornee { debut: Moment, fin: Moment, nature: Option<NatureDuRetour> },
}

impl PlageDuReleve {
    /// La derniere journee, de hier a maintenant, toutes natures (`suivre-depot`, `bilan-repetition`, la relance d'un
    /// depot dont la repetition manque).
    pub fn derniere_journee() -> R<PlageDuReleve> {
        let fin = socle::temps::secondes();
        let debut = fin.moins_duree(UNE_JOURNEE).ok_or_else(|| socle::echec::refus(socle::date::HORS_CALENDRIER))?;
        Ok(PlageDuReleve::Bornee { debut, fin, nature: None })
    }
}

/// LE RECOUVREMENT : le releve complet repart une journee avant l'instant ou le precedent s'est arrete. La liste filtre
/// sur la date de publication d'un retour, « l'horodatage de son enregistrement sur le SI DSN » (guide de l'API DSN v6.9,
/// § 3.4.2, l. 1608-1619), la « Date à laquelle le SI DSN a mis le retour à disposition des services de recherche »
/// (§ 3.4.4, l. 1694-1695) : un retour publie apres la fin d'un releve tombe dans le suivant. INFERENCE : aucun texte ne
/// dit le delai entre cet horodatage et le moment ou la liste montre le retour, ni l'ecart entre la montre du poste (qui
/// ecrit les bornes) et celle du SI DSN ; une journee les couvre, pour une tranche de plus. Les bornes s'ecrivent a la
/// seconde et les tranches sont contigues (`tranches`) : rien ne tombe entre deux. Un retour revu est deja vu : il ne se
/// retelecharge pas.
/// Pour la DPAE par plage, aucun texte de l'Urssaf depose ne dit sur quelle date la liste filtre : INFERENCE, on la
/// releve comme la DSN.
const RECOUVREMENT: std::time::Duration = UNE_JOURNEE;

/// L'HORIZON DU RELEVE COMPLET : sans releve complet precedent, ou quand il remonte plus loin, la plage part de
/// quatre-vingt-dix jours avant maintenant, ce que le SI DSN garde au telechargement (guide de l'API DSN, § 8.2 :
/// « une période de rétention de 3 mois »). Un releve qui remonte moins loin que cela n'est pas complet.
pub const HORIZON_DU_RELEVE_COMPLET: std::time::Duration = socle::temps::jours_de_24_heures(90);

/// JUSQU'OU LE DERNIER RELEVE COMPLET A LU, sur un canal, ET CE QUI RESTE A REPRENDRE : `retours/releve-complet.json` (la
/// DSN), `retours/dpae/releve-complet.json` (la DPAE), a cote du journal des vus. `lu_jusqu_a` : la fin de la derniere
/// tranche que le releve complet a listee sans echec, toutes celles d'avant listees aussi. `a_reprendre` : les retours
/// listes et pas encore vus (un telechargement rate, un hote muet, une piece que son lecteur ne lit pas, un taux que ses
/// suites liront), leur annonce telle que la liste l'a rendue : le releve complet suivant les reprend par elle, sans
/// relister leur journee ; vus, ils en sortent. La liste coute, pas la reprise : un retour a reprendre ne retient pas
/// `lu_jusqu_a`. Seul le releve complet l'ecrit (`PlageDuReleve::Complete`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleveComplet {
    pub quoi: String,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub lu_jusqu_a: Moment,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub a_reprendre: Vec<Annonce>,
}

/// Ce que `releve-complet.json` dit de lui-meme.
const QUOI_RELEVE_COMPLET: &str = "Jusqu'où le dernier relevé complet des retours (celui de l'ouverture) a lu ce canal, et les retours listés pas encore vus, à reprendre : le relevé suivant repart une journée avant, et reprend ceux-là sans relister.";

/// Le fichier de `ReleveComplet` du canal.
fn fichier_du_releve_complet(ctx: &Contexte, canal: CanalDesRetours) -> R<PathBuf> {
    Ok(dossier_archive(&ctx.racine, canal)?.join("releve-complet.json"))
}

/// Jusqu'ou le dernier releve complet du canal a lu ; aucun : il n'y en a pas encore eu.
fn releve_complet(ctx: &Contexte, canal: CanalDesRetours) -> R<Option<ReleveComplet>> {
    let p = fichier_du_releve_complet(ctx, canal)?;
    if !p.exists() {
        return Ok(None);
    }
    socle::forme::lire_fichier(&p, "Le relevé complet des retours ne se lit pas.").map(Some)
}

/// Ecrit jusqu'ou le releve complet du canal a lu, et ce qui reste a reprendre.
fn retenir_le_releve_complet(ctx: &Contexte, canal: CanalDesRetours, lu_jusqu_a: Moment, a_reprendre: Vec<Annonce>) -> R<()> {
    let p = fichier_du_releve_complet(ctx, canal)?;
    socle::ecriture::ecrire_json(&p, &ReleveComplet { quoi: QUOI_RELEVE_COMPLET.to_string(), lu_jusqu_a, a_reprendre }, 2)
}

/// UN RETOUR A REPRENDRE QUE LE SI DSN NE SERT PLUS : publie il y a plus de quatre-vingt-dix jours
/// (`HORIZON_DU_RELEVE_COMPLET`, guide de l'API DSN, § 8.2 : « une période de rétention de 3 mois »). Il sort de la
/// reprise, note vu, et l'editeur le sait comme d'un retour retire (`Suites::retires`). Sans publication dite, son age ne
/// se sait pas : il reste a reprendre. Le canal DPAE a la meme retention (guide d'implementation de l'API DPAE, « Période
/// de rétention des retours » : « une période de rétention de 3 mois par le SI DPAE. Au-delà, les retours sont purgés »,
/// `atelier/reference/norme-dpae/API-DPAE-Guide-Implementation-NAT5492.pdf`).
fn hors_de_l_horizon(r: &Annonce, maintenant: Moment) -> bool {
    r.publication.is_some_and(|p| maintenant.depuis(p).is_some_and(|d| d > HORIZON_DU_RELEVE_COMPLET))
}

/// Le nom de l'archive d'un retour, sans son extension (`archiver_retour`).
fn nom_d_archive(retour: &Annonce, maintenant: &str) -> String {
    let horo = match retour.publication {
        Some(p) => horodatage_archive(p),
        None => maintenant.to_string(),
    };
    let flux = match &retour.flux {
        Some(f) => f.texte(),
        None => "sansflux",
    };
    format!("{}_nature{}_{}", horo, retour.nature_retour, flux)
}

/// Les extensions qu'une archive de retour peut porter (`archiver_retour`).
const EXTENSIONS_D_ARCHIVE: [&str; 3] = [".pdf", ".xml", ".txt"];

/// L'ARCHIVE D'UN RETOUR DEJA SUR LE DISQUE (un releve precedent l'a telechargee, son lecteur ne l'a pas lue, ou ses
/// suites ne l'ont pas encore notee vue) : elle se relit sans reseau. Son nom tient a sa publication ; un retour qui ne la
/// dit pas s'archive a l'heure du releve, et ne se retrouve pas : il se retelecharge.
fn archive_deja_la(racine: &Path, canal: CanalDesRetours, retour: &Annonce) -> R<Option<PathBuf>> {
    if retour.publication.is_none() {
        return Ok(None);
    }
    let dossier = canal.chemin(racine);
    let nom = nom_d_archive(retour, "");
    Ok(EXTENSIONS_D_ARCHIVE.iter().map(|ext| dossier.join(format!("{}{}", nom, ext))).find(|p| p.is_file()))
}

/// La piece d'un retour, relue de son archive si elle est deja sur le disque (`archive_deja_la`), sinon aucune.
fn piece_deja_la(racine: &Path, canal: CanalDesRetours, retour: &Annonce) -> R<Option<(Vec<u8>, PathBuf)>> {
    match archive_deja_la(racine, canal, retour)? {
        Some(chemin) => {
            let contenu = std::fs::read(&chemin).map_err(|e| crate::etats::erreur_io(&e, &chemin))?;
            Ok(Some((contenu, chemin)))
        }
        None => Ok(None),
    }
}

/// L'ADRESSE DE LA PIECE D'UN RETOUR, quand il a une piece a telecharger. Une nature au format « Aucun » de la liste des
/// retours (guide de l'API DSN v6.9, l. 615-764, `FormatDuRetour::Aucun`) n'a pas de document, et son annonce entiere va
/// au journal des vus. Le mandat a acquitter (nature 90) porte pourtant une adresse : « En mode API,
/// l'URL renseignée dans le retour est celle d'acquittement du mandat » (§ 5.17.1, l. 4389-4392) ; c'est la page ou le
/// declarant acquitte le mandat, pas une piece, et elle ne s'appelle pas. Cette liste est celle de la DSN : sur le canal
/// DPAE, tout retour qui porte une adresse se telecharge.
fn piece_a_telecharger(r: &Annonce, canal: CanalDesRetours) -> Option<Url> {
    match (canal, r.nature_retour.format()) {
        (CanalDesRetours::Dsn, Some(crate::retours::FormatDuRetour::Aucun)) => None,
        _ => r.url.clone(),
    }
}

/// Telecharge la piece d'un retour et l'archive : son contenu, son archive.
fn telecharger_et_archiver(ctx: &Contexte, client: &mut Client, canal: CanalDesRetours, retour: &Annonce, url: &Url) -> Result<(Vec<u8>, PathBuf), Panne> {
    let (contenu, ctype) = client.telecharger(url)?;
    let chemin = archiver(ctx, retour, &contenu, ctype.as_deref(), canal)?;
    Ok((contenu, chemin))
}

/// Les cles des retours deja vus sur le canal.
fn deja_vus(ctx: &Contexte, canal: CanalDesRetours) -> R<Vec<CleDeRetour>> {
    Ok(lire_vus(&journal_des_vus(ctx, canal)?)?.iter().map(crate::etats::cle_du_contenu).collect())
}

/// Le retour est-il deja vu ? Ses trois champs, comme `(flux, nature, publication) in connus`.
fn est_connu(connus: &[CleDeRetour], r: &Annonce) -> bool {
    let k = crate::etats::cle_du_contenu(r);
    connus.contains(&k)
}

/// Les retours de ce releve dont l'archive est une piece que son lecteur n'a pas lue (`illisibles`, par le nom de
/// l'archive et la nature).
fn archives_non_lues(archives: &[RetourArchive], illisibles: &[crate::retours::PieceIllisible]) -> R<Vec<Annonce>> {
    let mut non_lus = Vec::new();
    for a in archives {
        let nom = socle::fichiers::NomDeFichier::du(&a.chemin)?;
        if illisibles.iter().any(|p| p.nature == a.retour.nature_retour && p.archive == nom) {
            non_lus.push(a.retour.clone());
        }
    }
    Ok(non_lus)
}

fn en_echec(p: Panne) -> Echec {
    p.en_echec()
}

/// UNE TRANCHE DE RECHERCHE : vingt-quatre heures. Pour la consultation des depots, c'est le texte : « La plage de
/// recherche est limitée à 24h » (guide de l'API DSN v6.9, § 3.3.1,
/// `atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.9-20260904.txt`, l. 1393). Pour les
/// retours, aucun texte ne fixe une journee : § 8.3.2 (l. 5155-5164) dit que le service annonce l'etendue permise dans
/// l'en-tete `Accept-Ranges`, et donne en exemple `minutes=0-60`. INFERENCE : une journee est l'etendue que le service
/// accepte de fait, les releves reels passent par tranches d'une journee ; on ne lit pas l'en-tete, qui, s'il disait
/// soixante minutes, ferait d'un releve de trente jours sept cent vingt appels.
const UNE_JOURNEE: std::time::Duration = std::time::Duration::from_secs(86_400);

/// Une plage decoupee par journee (`UNE_JOURNEE`), en tranches contigues : la fin de l'une est le debut de la suivante.
pub fn tranches(debut: Moment, fin: Moment) -> R<Vec<(Moment, Moment)>> {
    // La requete ecrit ses bornes a la seconde : une fraction ne fait pas une tranche de plus.
    let (debut, fin) = (debut.a_la_seconde(), fin.a_la_seconde());
    let mut v = Vec::new();
    let mut t = debut;
    while t < fin {
        let suivant = if fin.depuis(t).is_some_and(|d| d > UNE_JOURNEE) {
            t.plus_duree(UNE_JOURNEE).ok_or_else(|| socle::echec::refus(socle::date::HORS_CALENDRIER))?
        } else {
            fin
        };
        v.push((t, suivant));
        t = suivant;
    }
    Ok(v)
}

/// LE SIMULATEUR : pose les retours des depots joues qui les attendent, et rend le nombre de
/// depots completes. Ne touche a aucune autre ligne : un vrai depot (flux sans le prefixe) garde ses vrais retours. Les
/// attestations France Travail jouees s'archivent dans `retours/` comme les vraies, et vont dans les suites, a ranger.
pub fn rattacher_simulation(ctx: &Contexte, suites: &mut Suites) -> R<usize> {
    let mut a_archiver: Vec<(Annonce, Vec<u8>)> = Vec::new();
    // Ce que les organismes rendent de plus (refus, taux du PAS, effectif) : archive puis range par le meme chemin
    // qu'un retour reel (`rattacher_retours_avec_contenus`), qui en lit le verdict.
    let mut pieces: Vec<crate::organismes_simules::Piece> = Vec::new();
    let mut lignes = crate::journal::lu(&ctx.racine).map(|j| j.lignes.to_vec())?;
    let mut rangements: Vec<(usize, crate::journal::Rangement)> = Vec::new();
    let avant = crate::organismes_simules::memoire(&ctx.racine)?;
    let mut memoire = avant.clone();
    let pub_ = socle::temps::secondes().a_la_seconde();
    let mut faits = 0;
    for i in 0..lignes.len() {
        let l = &lignes[i];
        let quoi = l.acte();
        let Some(flux) = l.flux().filter(|f| f.est_fictif()).cloned() else { continue };
        if matches!(quoi, Acte::Dsn | Acte::Dpae) && l.accepte() && l.nombre_de_retours() == 0 {
            let base = |nature: NatureDuRetour, verdict: Option<Verdict>, libelle: &str| RetourOrganisme {
                nature_retour: nature,
                verdict,
                anomalies: Some(Vec::new()),
                dpae: None,
                libelle: libelle.into(),
                statut: Some(StatutDuRetour::Ok),
                publication: Some(pub_),
            };
            let mut retours: Vec<RetourOrganisme> = Vec::new();
            if quoi == Acte::Dpae {
                retours.push(base(NatureDuRetour::CertificatDeConformite, Some(Verdict::Ok), "Bilan de conformite (compagnie fictive)"));
                let mut m = base(NatureDuRetour::CertificatDpae, Some(Verdict::Ok), "Certificat de conformite des DPAE (compagnie fictive)");
                m.dpae = Some(Vec::new());
                retours.push(m);
            } else {
                match crate::organismes_simules::reponses_dsn(ctx, &mut memoire, &lignes, l, &flux, pub_)? {
                    // Refusee : ses deux bilans, rien d'autre ; le verdict se lit dans la piece.
                    Err(bilans) => {
                        pieces.extend(bilans);
                        faits += 1;
                        continue;
                    }
                    Ok(autres) => pieces.extend(autres),
                }
                retours.push(base(NatureDuRetour::AccuseDEnregistrement, Some(Verdict::Ok), "Accuse de reception (compagnie fictive)"));
                retours.push(base(NatureDuRetour::CertificatDeConformite, Some(Verdict::Ok), "Bilan de conformite (compagnie fictive)"));
                // L'AER : seulement quand elle est due, et avec sa piece, comme France Travail la publie.
                if let Some(archive) = aer_jouee(ctx, l)? {
                    let r46 = base(NatureDuRetour::AerSpectacle, None, "Attestation employeur rematerialisee (compagnie fictive)");
                    let annonce = Annonce {
                        publication: r46.publication.clone(),
                        production: None,
                        ops: None,
                        delegataire: None,
                        nature_retour: r46.nature_retour,
                        statut: r46.statut.clone(),
                        id: None,
                        url: None,
                        libelle: Some(r46.libelle.clone()),
                        flux: Some(flux.clone()),
                    };
                    a_archiver.push((annonce, archive));
                    retours.push(r46);
                }
            }
            rangements.extend(retours.iter().map(|r| (i, crate::journal::Rangement::Retour(r.clone()))));
            match &mut lignes[i] {
                Ligne::Dsn(e) => e.retours = retours,
                Ligne::Dpae(e) => e.retours = retours,
                _ => {}
            }
            faits += 1;
        }
    }
    crate::journal::ranger(&ctx.racine, rangements).map_err(|m| m.reclasser(Classe::Disque))?;
    crate::organismes_simules::garder(&ctx.racine, &avant, &memoire)?;
    // LE MEME CHEMIN QU'UNE VRAIE : l'archive dans `retours/`, puis le rangement (par `moteur`), qui signe le cadre 5 et la
    // pose dans `sorties/<mois>/envoi/`, ou l'envoi va la chercher.
    for (retour, archive) in &a_archiver {
        suites.aer.push(archiver(ctx, retour, archive, Some("application/zip"), CanalDesRetours::Dsn)?);
    }
    if !pieces.is_empty() {
        let mut contenus: Vec<(crate::etats::CleDuContenu, Vec<u8>)> = Vec::new();
        let mut decrits: Vec<Annonce> = Vec::new();
        for x in &pieces {
            let retour = &x.retour;
            let chemin = archiver(ctx, retour, &x.contenu, Some("application/xml"), CanalDesRetours::Dsn)?;
            if retour.nature_retour == NatureDuRetour::TauxPas {
                suites.taux_pas.push((retour.clone(), chemin));
            } else if retour.nature_retour == NatureDuRetour::TauxAtMp {
                suites.taux_atmp.push((retour.clone(), chemin));
            }
            contenus.push((crate::etats::cle_du_contenu(&retour), x.contenu.clone()));
            decrits.push(retour.clone());
        }
        suites.garder(crate::etats::rattacher_retours_avec_contenus(&ctx.racine, &decrits, &contenus))?;
        // L'EFFECTIF S'ECRIT SEUL, comme a la releve reelle.
        suites.illisibles.extend(reconstruire_effectifs(&ctx.racine)?);
    }
    Ok(faits)
}

/// L'AER Cinema Spectacle qu'une DSN mensuelle REELLE jouee appelle (`aer_fictive::archive`) : l'archive de nature 46,
/// ou `None` quand aucune n'est due (France Travail ne publie rien) ; un signalement (nature 04, 05, 07) n'en appelle pas.
/// Le fichier depose est relu la ou le depot l'a pris (`sorties/<mois>/<fichier>`).
fn aer_jouee(ctx: &Contexte, l: &Ligne) -> R<Option<Vec<u8>>> {
    if l.mode() == socle::mode::Mode::Test {
        return Ok(None);
    }
    if l.nature_dsn().is_some_and(|n| n != droit::nature_declaration::NatureDeLaDeclaration::Mensuelle) {
        return Ok(None);
    }
    let (Some(mois), Some(fichier)) = (l.mois(), l.fichier()) else { return Ok(None) };
    let chemin = documents::fichier_depose(&ctx.racine, mois, fichier.texte());
    // Un fichier depose qui n'est plus la n'appelle pas d'attestation jouee.
    let octets = match std::fs::read(&chemin) {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque)),
    };
    crate::aer_fictive::archive(ctx, &octets, mois, ctx.aujourdhui)
}

/// Telecharge et archive ce que les organismes ont publie sur la plage. `client` : le client du service, avec son mot de
/// passe (le canal DPAE passe le sien, avec `canal` `Dpae`) ; `None` dans le monde : aucun mot de passe n'est pose, et
/// cela se dit.
pub fn relever_retours(
    ctx: &Contexte,
    branchement: Branchement,
    plage: PlageDuReleve,
    tout: bool,
    client: Option<Client>,
    canal: CanalDesRetours,
    suites: &mut Suites,
) -> R<Releve> {
    let mut releve = Releve::vide();
    match branchement {
        // Rien n'est parti, rien ne revient.
        Branchement::Message(_) => return Ok(releve),
        // Les retours des depots joues arrivent d'ici, sans reseau ni mot de passe.
        Branchement::Simulateur if client.is_none() => {
            releve.rattaches = rattacher_simulation(ctx, suites)?;
            return Ok(releve);
        }
        _ => {}
    }
    let Some(mut client) = client else {
        releve.plateforme_non_lue.push(
            format!("Le mot de passe de {} n'est pas posé : le poser dans Configuration › Mots de passe.", canal.service_du_mot_de_passe()),
        );
        return Ok(releve);
    };
    match client.authentifier() {
        Ok(()) => {}
        Err(p) if p.est_api() => {
            releve.plateforme_non_lue.push(p.echec.phrase.clone());
            suites.panne(p);
            return Ok(releve);
        }
        Err(p) => return Err(en_echec(p)),
    }
    let connus = deja_vus(ctx, canal)?;
    let fin = socle::temps::secondes();
    let (debut, fin, nature, complet, precedent) = match plage {
        PlageDuReleve::Complete { jusqu_a } => {
            let au_plus_loin = fin.moins_duree(jusqu_a).ok_or_else(|| socle::echec::refus(socle::date::HORS_CALENDRIER))?;
            let precedent = releve_complet(ctx, canal)?;
            // Un instant retenu posterieur a maintenant (une montre en avance le jour ou il a ete ecrit) ne dit rien :
            // la plage repart du plus loin, comme sans releve precedent.
            let repris = match precedent.as_ref().filter(|c| c.lu_jusqu_a <= fin) {
                Some(c) => Some(c.lu_jusqu_a.moins_duree(RECOUVREMENT).ok_or_else(|| socle::echec::refus(socle::date::HORS_CALENDRIER))?),
                None => None,
            };
            // CE RELEVE N'AVANCE L'INSTANT RETENU QUE S'IL A LU TOUT CE QU'UN RELEVE COMPLET DOIT LIRE : a la date reelle
            // de la machine (un jour pose par `--date` ecrirait un instant qui n'est pas celui de la montre), et sur une
            // plage que l'appelant n'a pas raccourcie (`jusqu_a` plus court que l'horizon laisserait un trou entre
            // l'instant retenu et le debut de cette plage, que l'instant avance refermerait pour toujours).
            let a_la_date_reelle = socle::temps::jour_de_paris() == socle::temps::jour_de_la_montre();
            let entiere = jusqu_a >= HORIZON_DU_RELEVE_COMPLET || repris.is_some_and(|r| r >= au_plus_loin);
            let debut = match repris {
                Some(r) => r.max(au_plus_loin),
                None => au_plus_loin,
            };
            (debut, fin, None, a_la_date_reelle && entiere, precedent)
        }
        PlageDuReleve::Bornee { debut, fin, nature } => (debut, fin, nature, false, None),
    };
    // CE QUE LE RELEVE COMPLET PRECEDENT A LAISSE A REPRENDRE (`ReleveComplet::a_reprendre`) : ce qu'un autre releve a
    // note vu depuis en sort ; ce que le SI DSN ne sert plus en sort aussi, note vu, et l'editeur le sait
    // (`hors_de_l_horizon`) ; le reste se reprend par son annonce, plus bas, avec les retours listes.
    let mut retenus: Vec<Annonce> = Vec::new();
    let mut expires: Vec<Annonce> = Vec::new();
    for r in precedent.iter().flat_map(|c| c.a_reprendre.iter()).filter(|r| !est_connu(&connus, r)) {
        // A un jour pose (`--date`), l'age d'un retour n'est pas le sien : rien n'en sort.
        if socle::temps::jour_de_paris() == socle::temps::jour_de_la_montre() && hors_de_l_horizon(r, fin) {
            expires.push(r.clone());
        } else {
            retenus.push(r.clone());
        }
    }
    for r in &expires {
        if let (Some(url), Some(publie)) = (r.url.clone(), jour_de_publication_dit(r)) {
            let pourquoi = match canal {
                CanalDesRetours::Dsn => "le SI DSN ne le sert plus après quatre-vingt-dix jours (guide de l'API DSN, § 8.2)",
                CanalDesRetours::Dpae => "le SI DPAE ne le sert plus après quatre-vingt-dix jours (guide d'implémentation de l'API DPAE, « Période de rétention des retours »)",
            };
            let phrase = format!("Retour nature {} publié le {}, jamais lu : {}.", r.nature_retour, publie, pourquoi);
            suites.retires.push((r.nature_retour, url, erreur(Classe::Organisme, phrase)));
        }
    }
    if !expires.is_empty() {
        noter_vus(ctx, &expires, canal)?;
    }
    let cles_retenues: Vec<CleDeRetour> = retenus.iter().map(crate::etats::cle_du_contenu).collect();
    // LA PLAGE SE DEMANDE PAR TRANCHES D'UNE JOURNEE (`UNE_JOURNEE`), contigues.
    // UN 404 SUR UNE TRANCHE VEUT DIRE << RIEN SUR CETTE TRANCHE >> ; tout autre code arrete : un 401 ne
    // s'ameliorera pas au jour suivant. `lu_jusqu_a` : la fin de la derniere tranche listee, toutes celles d'avant
    // listees aussi. Un retour retenu que la liste rend de nouveau n'est pas nouveau : il a deja ete compte.
    let mut lu_jusqu_a: Option<Moment> = None;
    // Les hotes qui n'ont pas repondu dans ce releve, avec la phrase de leur panne.
    let mut muets: Vec<(String, String)> = Vec::new();
    for (t0, t1) in tranches(debut, fin)? {
        match client.lister_retours(t0, t1) {
            Ok(trouves) => {
                for r in trouves.into_iter().filter(|r| nature.is_none_or(|n| r.nature_retour == n)) {
                    if !est_connu(&connus, &r) && !est_connu(&cles_retenues, &r) {
                        releve.nouveaux.push(r.clone());
                    }
                    releve.trouves.push(r);
                }
            }
            Err(p) if p.est_api() && p.code == Some(ureq::http::StatusCode::NOT_FOUND) => {}
            Err(p) if p.est_api() => {
                releve.plateforme_non_lue.push(p.echec.phrase.clone());
                // UN HOTE QUI NE REPOND PAS (aucun statut HTTP) n'est plus appele dans ce releve (plus bas).
                if p.code.is_none() {
                    muets.push((socle::garde_reseau::hote_de(client.adresses.retours.texte()), p.echec.phrase.clone()));
                }
                suites.panne(p);
                break;
            }
            Err(p) => return Err(en_echec(p)),
        }
        lu_jusqu_a = Some(t1);
    }
    // LES RETOURS RETENUS que la liste n'a pas rendus de nouveau se reprennent par leur annonce, sans relister leur
    // journee : ils rejoignent les retours trouves, et suivent leur chemin (telechargement ou archive deja la, lecture,
    // rangement, vus).
    let listes: Vec<CleDeRetour> = releve.trouves.iter().map(crate::etats::cle_du_contenu).collect();
    releve.trouves.extend(retenus.into_iter().filter(|r| !est_connu(&listes, r)));
    // La liste n'a pas pu se lire, et rien n'est trouve ni a reprendre : il n'y a rien a telecharger ni a ranger ; ce qui
    // a ete liste sans echec avance le releve complet.
    if !releve.plateforme_non_lue.is_empty() && releve.trouves.is_empty() {
        if let (true, Some(jusqu_a)) = (complet, lu_jusqu_a) {
            retenir_le_releve_complet(ctx, canal, jusqu_a, Vec::new())?;
        }
        return Ok(releve);
    }

    // UN RETOUR RATE N'EST PAS UN RETOUR VU : un echec reste NEUF, et se retente au releve suivant. Sauf un retour que
    // l'organisme a retire : HTTP 410, « la ressource n'est plus disponible » (guide de l'API DSN v6.9, § 9.1.7 « Codes
    // HTTP standard », l. 5413-5416) ; il ne reviendra pas, il se note vu, et l'editeur le sait (`Suites::retires`). Sur
    // le canal DPAE, aucun texte de l'Urssaf ne dit le 410 : INFERENCE, son service suit le code HTTP comme celui de la DSN.
    // UN HOTE QUI NE REPOND PAS (aucun statut HTTP) n'est plus appele dans ce releve : les autres retours qu'il sert
    // restent neufs, et se tentent au releve suivant ; un bilan parmi eux n'est pas lu, et cela se dit, de ce bilan.
    // UNE PIECE DEJA ARCHIVEE se relit du disque, sans reseau (`piece_deja_la`) : une piece que son lecteur n'a pas lue,
    // ou un taux que les suites n'ont pas encore note vu.
    let mut rates: Vec<Annonce> = Vec::new();
    let a_telecharger: Vec<Annonce> = if tout { releve.trouves.clone() } else { releve.trouves.iter().filter(|r| !est_connu(&connus, r)).cloned().collect() };
    for r in a_telecharger {
        let Some(url) = piece_a_telecharger(&r, canal) else { continue };
        let (contenu, chemin) = match piece_deja_la(&ctx.racine, canal, &r)? {
            Some(piece) => piece,
            None => {
                let hote = socle::garde_reseau::hote_de(url.texte());
                if let Some((_, panne)) = muets.iter().find(|(h, _)| *h == hote) {
                    if r.nature_retour == NatureDuRetour::CertificatDeConformite {
                        let phrase = format!("Le retour de nature {} ne s'est pas téléchargé : {}", r.nature_retour, panne);
                        if !releve.plateforme_non_lue.contains(&phrase) {
                            releve.plateforme_non_lue.push(phrase);
                        }
                    }
                    rates.push(r);
                    continue;
                }
                match telecharger_et_archiver(ctx, &mut client, canal, &r, &url) {
                    Ok(piece) => piece,
                    Err(p) if p.est_api() && p.code == Some(ureq::http::StatusCode::GONE) => {
                        suites.retires.push((r.nature_retour, url, p.echec));
                        continue;
                    }
                    Err(p) if p.est_api() => {
                        let phrase = format!("Le retour de nature {} ne s'est pas téléchargé : {}", r.nature_retour, p.echec.phrase);
                        if p.code.is_none() {
                            muets.push((hote, p.echec.phrase.clone()));
                        }
                        suites.panne(p);
                        match r.nature_retour {
                            NatureDuRetour::CertificatDeConformite => releve.plateforme_non_lue.push(phrase),
                            _ => suites.dit.incidents.push(phrase),
                        }
                        rates.push(r);
                        continue;
                    }
                    Err(p) => return Err(en_echec(p)),
                }
            }
        };
        // LE TAUX AT/MP NOTIFIE SE LIT DE LA NOTIFICATION (regle 1) : il se pose dans la surcouche, par les suites.
        if canal == CanalDesRetours::Dsn && r.nature_retour == NatureDuRetour::TauxAtMp {
            suites.taux_atmp.push((r.clone(), chemin.clone()));
        }
        // UN SIGNALEMENT REFUSE B1-208-10 : l'ordre etait deja pris ; la conduite a tenir se dit.
        if canal == CanalDesRetours::Dsn && r.nature_retour == NatureDuRetour::CertificatDeConformite {
            match crate::retours::declarations_du_bilan(&contenu) {
                Ok(ds) => {
                    for d in ds {
                        suites.dit.a_faire.extend(crate::ordre_signalements::conduite_b1_208_10(&d, r.flux.as_ref()));
                    }
                }
                Err(e) => suites.illisibles.push(crate::retours::PieceIllisible::de(r.nature_retour, &chemin, e)?),
            }
        }
        // LE TAUX DU PAS SE LIT DU COMPTE RENDU (regle 1) : il va dans la fiche du salarie, par les suites.
        if canal == CanalDesRetours::Dsn && r.nature_retour == NatureDuRetour::TauxPas {
            suites.taux_pas.push((r.clone(), chemin.clone()));
        }
        releve.archives.push(RetourArchive { retour: r, chemin, contenu });
    }

    if canal == CanalDesRetours::Dsn {
        // L'EFFECTIF S'ECRIT SEUL : `config/effectifs.json` se refait depuis les archives.
        suites.illisibles.extend(reconstruire_effectifs(&ctx.racine)?);
    }

    let mut vus: Vec<Annonce> = Vec::new();
    if !releve.trouves.is_empty() {
        // Un depot n'est fini que quand sa reponse est la : chaque retour se range en face du depot qui l'a
        // provoque -- par le flux, et par rien d'autre.
        let contenus: Vec<_> = releve.archives.iter().map(|a| (crate::etats::cle_du_contenu(&a.retour), a.contenu.clone())).collect();
        releve.rattaches = suites.garder(crate::etats::rattacher_retours_avec_contenus(&ctx.racine, &releve.trouves, &contenus))?;
        // UNE PIECE QUE SON LECTEUR NE LIT PAS N'EST PAS UN RETOUR VU, comme un retour rate : elle reste neuve, et le
        // releve suivant la relit de son archive (`piece_deja_la`). Le taux AT/MP et le taux
        // du PAS se lisent dans les suites : ils s'y notent vus, une fois lus.
        let lus_aux_suites = |r: &Annonce| canal == CanalDesRetours::Dsn && matches!(r.nature_retour, NatureDuRetour::TauxAtMp | NatureDuRetour::TauxPas);
        let non_lus = archives_non_lues(&releve.archives, &suites.illisibles)?;
        for r in releve.trouves.iter().filter(|r| !rates.contains(r) && !non_lus.contains(r)) {
            if lus_aux_suites(r) && releve.archives.iter().any(|a| &a.retour == r) {
                suites.vus_une_fois_lus.push(r.clone());
            } else {
                vus.push(r.clone());
            }
        }
        noter_vus(ctx, &vus, canal)?;
    }

    // LE RELEVE COMPLET AVANCE jusqu'a la fin de ce qu'il a liste sans echec, et retient ce qu'il n'a pas note vu (rate,
    // piece non lue, taux que les suites liront) : le releve suivant le reprend par son annonce. Une liste qui n'a lu
    // aucune tranche laisse l'instant ou il etait.
    if complet {
        let jusqu_a = match (lu_jusqu_a, &precedent) {
            (Some(t), _) => Some(t),
            (None, Some(c)) => Some(c.lu_jusqu_a),
            (None, None) => None,
        };
        if let Some(jusqu_a) = jusqu_a {
            let a_reprendre: Vec<Annonce> = releve.trouves.iter().filter(|r| !est_connu(&connus, r) && !vus.contains(r)).cloned().collect();
            retenir_le_releve_complet(ctx, canal, jusqu_a, a_reprendre)?;
        }
    }

    // UN DEPOT QUE LES RETOURS ATTESTENT EST UN DEPOT : ce qu'un autre logiciel a depose sous le meme compte
    // entre au journal de lui-meme, et le mois qu'il declare n'est plus « en retard ».
    if canal == CanalDesRetours::Dsn {
        suites.garder(crate::reprise_journal::reprendre_les_depots(ctx, "releve"))?;
    }

    // L'AER SE RANGE DES QU'ELLE ARRIVE : la boite d'envoi la voit (les suites).
    if canal == CanalDesRetours::Dsn {
        suites.aer.extend(
            releve
                .archives
                .iter()
                .filter(|a| a.retour.nature_retour == NatureDuRetour::AerSpectacle)
                .map(|a| a.chemin.clone()),
        );
    }
    Ok(releve)
}

/// Les retours de l'API DPAE -- UN AUTRE SERVICE que celui de la DSN, ses archives sont dans `retours/dpae/`. `flux` : le
/// seul chemin documente (la FAQ) quand on connait l'idflux ; sinon la consultation par plage, sur le canal `dpae`.
/// `client` : le client Urssaf, avec son mot de passe ; `None` dans le monde : aucun mot de passe n'est pose. Le chemin
/// par flux n'avance pas le releve complet (`ReleveComplet`) : il ne lit pas une plage.
pub fn relever_retours_dpae(
    ctx: &Contexte,
    branchement: Branchement,
    plage: PlageDuReleve,
    tout: bool,
    flux: Option<&IdDeFlux>,
    client: Option<Client>,
    suites: &mut Suites,
) -> R<Releve> {
    match branchement {
        Branchement::Message(_) => return Ok(Releve::vide()),
        Branchement::Simulateur => {
            let mut r = Releve::vide();
            r.rattaches = rattacher_simulation(ctx, suites)?;
            return Ok(r);
        }
        Branchement::Monde => {}
    }
    let Some(mut client) = client else {
        let mut r = Releve::vide();
        r.plateforme_non_lue.push(format!("Le mot de passe de {} n'est pas posé : le poser dans Configuration › Mots de passe.", CanalDesRetours::Dpae.service_du_mot_de_passe()));
        return Ok(r);
    };
    let Some(flux) = flux else {
        return relever_retours(ctx, branchement, plage, tout, Some(client), CanalDesRetours::Dpae, suites);
    };
    let mut r = Releve::vide();
    let trouves = client.authentifier().and_then(|_| client.lister_retours_du_flux(flux));
    match trouves {
        Ok(t) => r.trouves = t,
        Err(p) if p.est_api() => {
            r.plateforme_non_lue.push(format!("Le flux {} ne s'est pas consulté : {}", flux, p.echec.phrase));
            suites.panne(p);
            return Ok(r);
        }
        Err(p) => return Err(en_echec(p)),
    }
    // Un telechargement rate (hors 410) n'est pas vu : la consultation par plage le reprend.
    let mut rates: Vec<Annonce> = Vec::new();
    for x in r.trouves.clone() {
        let Some(url) = piece_a_telecharger(&x, CanalDesRetours::Dpae) else { continue };
        let piece = match piece_deja_la(&ctx.racine, CanalDesRetours::Dpae, &x)? {
            Some(piece) => Ok(piece),
            None => telecharger_et_archiver(ctx, &mut client, CanalDesRetours::Dpae, &x, &url),
        };
        match piece {
            Ok((contenu, chemin)) => r.archives.push(RetourArchive { retour: x, chemin, contenu }),
            // Un retour retire (HTTP 410) ne reviendra pas : l'editeur le sait, comme par la plage (INFERENCE, aucun
            // texte de l'Urssaf ne dit le 410 : `relever_retours`).
            Err(p) if p.est_api() && p.code == Some(ureq::http::StatusCode::GONE) => suites.retires.push((x.nature_retour, url, p.echec)),
            Err(p) if p.est_api() => {
                suites.dit.incidents.push(format!("Le retour de nature {} ne s'est pas téléchargé : {}", x.nature_retour, p.echec.phrase));
                suites.panne(p);
                rates.push(x);
            }
            Err(p) => return Err(en_echec(p)),
        }
    }
    if !r.trouves.is_empty() {
        // Le chemin par flux accroche aussi ce qu'il trouve au journal.
        let contenus: Vec<_> = r.archives.iter().map(|a| (crate::etats::cle_du_contenu(&a.retour), a.contenu.clone())).collect();
        r.rattaches = suites.garder(crate::etats::rattacher_retours_avec_contenus(&ctx.racine, &r.trouves, &contenus))?;
        // Une piece que son lecteur ne lit pas n'est pas vue : la consultation par plage la reprend aussi.
        let non_lus = archives_non_lues(&r.archives, &suites.illisibles)?;
        let vus: Vec<Annonce> = r.trouves.iter().filter(|x| !rates.contains(x) && !non_lus.contains(x)).cloned().collect();
        noter_vus(ctx, &vus, CanalDesRetours::Dpae)?;
    }
    Ok(r)
}

// ------------------------------------------------------------------- l'effectif

/// `effectif.lire_crm118` : les effectifs d'un retour de nature 118 (code, valeur, periode, calcul), les parametres
/// (`RO.B008`) que lit le lecteur des comptes rendus NEOReS (`retours::lire_neores`, d'apres le cahier technique NEOReS
/// 2026.1.2, `atelier/reference/sources/neores/`) ; un XML qui ne se lit pas ne dit rien.
fn lire_crm118(contenu: &[u8]) -> R<Option<Vec<Mesure118>>> {
    let Some(crm) = crate::retours::lire_neores(contenu)? else { return Ok(None) };
    let mut out = Vec::new();
    for p in crm.parametres {
        let (Some(code), Some(valeur)) = (p.type_parametre, p.valeur) else { continue };
        let v = socle::grandeurs::lu::en_texte::<Effectif>(&valeur).map_err(|_| erreur(Classe::Illisible, format!("Le retour ne se lit pas en RO_B008_R002 (la valeur de l'effectif {}) : {} n'est pas un nombre.", code, socle::fichiers::repr(&valeur))))?;
        out.push(Mesure118 { code, valeur: v, debut: p.effet, fin: p.fin, calcule_le: p.calcule_le });
    }
    Ok(Some(out))
}

/// Un effectif qu'un compte rendu de nature 118 communique : son code, sa valeur, sa periode, le jour de son calcul.
struct Mesure118 {
    code: TypeDeParametreNeores,
    valeur: Effectif,
    debut: Option<Jour>,
    fin: Option<Jour>,
    calcule_le: Option<socle::paris::Moment>,
}

/// Ce que `config/effectifs.json` dit de lui-meme.
const QUOI_EFFECTIFS: &str = "Effectifs par année civile. A01 = effectif de sécurité sociale (art. L. 130-1 CSS), celui des seuils de cotisation. Ceux que l'Urssaf calcule et renvoie en compte rendu de nature 118 s'écrivent au relevé des retours ; ceux que la compagnie dit (dit_le) pour une année sans 118, par la route effectif, jusqu'à ce qu'un 118 de cette année les remplace.";

/// `config/effectifs.json` lu par sa forme (`droit::contexte::effectifs_lus`) ; aucun s'il n'existe pas.
fn effectifs_lus(racine: &Path) -> R<Option<FichierEffectifs>> {
    droit::contexte::effectifs_lus(racine).map_err(|m| m.reclasser(Classe::Illisible))
}

/// Ecrit `config/effectifs.json` s'il change : les effectifs par annee et par code.
fn ecrire_effectifs(racine: &Path, existant: Option<&FichierEffectifs>, annees: BTreeMap<Annee, BTreeMap<TypeDeParametreNeores, EffectifEcrit>>) -> R<()> {
    let donnees = FichierEffectifs { quoi: QUOI_EFFECTIFS.to_string(), annees };
    if existant != Some(&donnees) {
        socle::ecriture::ecrire_json(&droit::contexte::EFFECTIFS.dans(racine), &donnees, 2).map_err(|m| m.reclasser(Classe::Disque))?;
    }
    Ok(())
}

/// L'effectif de l'annee `an` est-il celui que la compagnie a dit (`dit_le`), et non celui d'un 118 ?
pub fn effectif_dit_par_la_compagnie(racine: &Path, an: Annee) -> R<bool> {
    Ok(dits(effectifs_lus(racine)?.as_ref()).iter().any(|(a, _)| *a == an))
}

/// Les effectifs que la compagnie a dits (`dit_le`), par annee.
fn dits(existant: Option<&FichierEffectifs>) -> Vec<(Annee, EffectifEcrit)> {
    existant
        .into_iter()
        .flat_map(|f| f.annees.iter())
        .filter_map(|(an, c)| c.get(&TypeDeParametreNeores::EffectifMoyen).filter(|a| a.dit_le.is_some()).map(|a| (*an, a.clone())))
        .collect()
}

/// Relit tous les 118 archives et reecrit `config/effectifs.json`. Pour chaque annee
/// et chaque code, le calcul le plus RECENT l'emporte ; l'effectif qu'une annee sans 118 a recu de la compagnie reste.
/// Ne touche pas au fichier s'il n'y a aucun 118. Un 118 que son lecteur ne lit pas se rend et ne compte pas (les autres
/// comptent : ce qu'un 118 lisible a dit reste) ; le disque remonte.
pub fn reconstruire_effectifs(racine: &Path) -> R<Vec<crate::retours::PieceIllisible>> {
    let dossier = racine.join(crate::retours::RETOURS);
    let mut illisibles = Vec::new();
    // Sans dossier des retours, aucun 118 n'est arrive.
    let lecture = match std::fs::read_dir(&dossier) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(illisibles),
        Err(e) => return Err(socle::echec::du_disque(&dossier, &e).reclasser(Classe::Disque)),
    };
    let mut noms: Vec<String> = Vec::new();
    for e in lecture {
        let nom = e.map_err(|x| socle::echec::du_disque(&dossier, &x).reclasser(Classe::Disque))?.file_name().to_string_lossy().into_owned();
        if nom.ends_with(".xml") && crate::retours::archive_de_nature(&nom, NatureDuRetour::Effectifs) {
            noms.push(nom);
        }
    }
    noms.sort();
    let mut annees: BTreeMap<Annee, BTreeMap<TypeDeParametreNeores, EffectifEcrit>> = BTreeMap::new();
    for nom in &noms {
        let p = dossier.join(nom);
        let octets = std::fs::read(&p).map_err(|e| socle::echec::du_disque(&p, &e).reclasser(Classe::Disque))?;
        let mesures = match lire_crm118(&octets) {
            Ok(Some(m)) => m,
            Ok(None) => continue,
            Err(e) => {
                illisibles.push(crate::retours::PieceIllisible::de(NatureDuRetour::Effectifs, &p, e)?);
                continue;
            }
        };
        for m in mesures {
            let Some(ref_an) = m.debut.or(m.fin) else { continue };
            let codes = annees.entry(Annee::du_mois(ref_an.mois_civil())).or_default();
            if codes.get(&m.code).is_some_and(|deja| deja.calcule_le > m.calcule_le) {
                continue;
            }
            codes.insert(m.code, EffectifEcrit { valeur: m.valeur, calcule_le: m.calcule_le, source: Some(CheminRelatif::sous(racine, &p)?), dit_le: None });
        }
    }
    if annees.is_empty() {
        return Ok(illisibles);
    }
    let existant = effectifs_lus(racine)?;
    for (an, dit) in dits(existant.as_ref()) {
        annees.entry(an).or_default().entry(TypeDeParametreNeores::EffectifMoyen).or_insert(dit);
    }
    ecrire_effectifs(racine, existant.as_ref(), annees)?;
    Ok(illisibles)
}

/// L'EFFECTIF QUE LA COMPAGNIE DIT POUR L'ANNEE `an` (l'effectif annuel moyen de l'annee civile, L. 130-1, I, CSS ;
/// `atelier/reference/sources/code-de-la-securite-sociale/LISEZ-MOI.md`, « L'effectif au sens de la sécurité sociale ») :
/// une compagnie venue d'un autre logiciel n'a pas les 118 de ses annees passees. `Ok(false)` : un 118 de cette annee
/// est deja la, et c'est lui qui vaut ; rien n'est ecrit.
pub fn poser_effectif(racine: &Path, an: Annee, valeur: Effectif, le: Jour) -> R<bool> {
    let existant = effectifs_lus(racine)?;
    let mut annees: BTreeMap<_, _> = existant.iter().flat_map(|f| f.annees.iter()).map(|(an, c)| (*an, c.clone())).collect();
    let codes = annees.entry(an).or_default();
    if codes.get(&TypeDeParametreNeores::EffectifMoyen).is_some_and(|a| a.dit_le.is_none()) {
        return Ok(false);
    }
    codes.insert(TypeDeParametreNeores::EffectifMoyen, EffectifEcrit { valeur, calcule_le: None, source: None, dit_le: Some(le) });
    ecrire_effectifs(racine, existant.as_ref(), annees)?;
    Ok(true)
}
