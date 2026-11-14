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

use droit::contexte::{Contexte, EffectifEcrit, EffectifsParAnnee, FichierEffectifs, TypeDeParametreNeores};
use socle::echec::R;
use socle::fichiers::CheminRelatif;
use socle::mois::Annee;
use socle::echec::erreur;
use socle::echec::{Classe, Echec};
use crate::retours::{Annonce, Depot, IdDeFlux, NatureDuRetour, StatutDuRetour, Verdict};
use socle::date::Jour;
use socle::num::Decimal;
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

fn adresses_dpae(compte: Option<&droit::fiche::ApiDpae>) -> Adresses {
    let service = compte.and_then(|c| c.service.dite()).filter(|s| !s.is_empty()).cloned();
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
    pub adresses: Adresses,
    pub jeton: Option<Secret>,
    pub declarant: crate::transport::Identite,
    motdepasse: Option<Secret>,
    variable: &'static str,
    majuscules: bool,
}

impl Client {
    /// `Client(ctx, environnement, motdepasse=...)` : le compte net-entreprises de la DSN.
    pub fn dsn(ctx: &Contexte, environnement: EnvironnementNetEntreprises, motdepasse: Option<Secret>) -> Client {
        let declarant = crate::transport::declarant(&ctx.entreprise);
        Client {
            adresses: adresses_dsn(environnement),
            jeton: None,
            declarant,
            motdepasse,
            variable: VARIABLE_MOTDEPASSE,
            majuscules: false,
        }
    }

    /// `ClientDpae(ctx, majuscules, motdepasse)` : le compte Urssaf, qui n'est pas celui de la DSN.
    pub fn dpae(ctx: &Contexte, majuscules: bool, motdepasse: Option<Secret>) -> Client {
        // Sans compte DPAE dit, le declarant est l'entreprise, sans nom ni prenom, et le service n'est pas dit.
        let compte = ctx.entreprise.api_dpae.dite();
        let siret = compte.and_then(|c| c.siret.dite()).or_else(|| ctx.entreprise.siret.dite()).cloned();
        Client {
            adresses: adresses_dpae(compte),
            jeton: None,
            declarant: crate::transport::Identite { siret, nom: compte.and_then(|c| c.nom.en_texte()), prenom: compte.and_then(|c| c.prenom.en_texte()) },
            motdepasse,
            variable: VARIABLE_MOTDEPASSE_DPAE,
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
            url: &self.adresses.authentification,
            declarant: &self.declarant,
            motdepasse: mdp.as_ref(),
            variable: self.variable,
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
        Ok(crate::transport::appeler(url, self.jeton.as_ref())?)
    }

    /// La consultation d'une plage.
    fn consulter(&mut self, base: &Url, debut: Moment, fin: Option<Moment>) -> Result<Vec<u8>, Panne> {
        self.assurer_session()?;
        Ok(crate::transport::consulter(base, self.jeton.as_ref(), debut, fin)?)
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
    /// ou `dpae` (`Organisme`). Rend la reponse et son type de contenu, s'il est dit.
    pub fn deposer_octets(&mut self, url: &Url, contenu: &[u8], forme: &str) -> Result<(String, Option<String>), Panne> {
        self.assurer_session()?;
        let corps = gzip(contenu)?;
        let jeton = self.jeton.clone().ok_or_else(|| panne(Classe::Disque, "La session ouverte n'a pas de jeton.".into()))?;
        let (brut, entetes) = crate::transport::depot(url, &jeton, &corps, forme, crate::transport::DELAI_DEPOT)?;
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
    pub incidents: Vec<String>,
    /// Combien de ces retours ont trouve le depot auquel ils repondent.
    pub rattaches: i64,
    /// Ce qu'un retour recu demande de faire (un taux AT/MP a reporter).
    pub a_faire: Vec<String>,
}

impl Releve {
    /// Un releve ou rien n'est encore trouve.
    pub fn vide() -> Releve {
        Releve { trouves: Vec::new(), nouveaux: Vec::new(), archives: Vec::new(), incidents: Vec::new(), rattaches: 0, a_faire: Vec::new() }
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
    let horo = match retour.publication {
        Some(p) => horodatage_archive(p),
        None => maintenant.to_string(),
    };
    let flux = match &retour.flux {
        Some(f) => f.texte(),
        None => "sansflux",
    };
    let nom = format!("{}_nature{}_{}{}", horo, retour.nature_retour, flux, ext);
    let dest = dossier_archive(racine, canal)?.join(nom);
    if !dest.exists() {
        std::fs::write(&dest, contenu).map_err(|e| crate::etats::erreur_io(&e, &dest))?;
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

/// Ajoute au journal du canal les retours qu'il n'a pas encore vus.
fn noter_vus(ctx: &Contexte, entrees: &[Annonce], canal: CanalDesRetours) -> R<()> {
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

/// Les cles des retours deja vus sur le canal.
fn deja_vus(ctx: &Contexte, canal: CanalDesRetours) -> R<Vec<CleDeRetour>> {
    Ok(lire_vus(&journal_des_vus(ctx, canal)?)?.iter().map(crate::etats::cle_du_contenu).collect())
}

/// Le retour est-il deja vu ? Ses trois champs, comme `(flux, nature, publication) in connus`.
fn est_connu(connus: &[CleDeRetour], r: &Annonce) -> bool {
    let k = crate::etats::cle_du_contenu(r);
    connus.contains(&k)
}

fn en_echec(p: Panne) -> Echec {
    p.en_echec()
}

/// Le SI DSN limite l'etendue d'une recherche, on decoupe par jour.
pub fn tranches(debut: Moment, fin: Moment) -> R<Vec<(Moment, Moment)>> {
    // La requete ecrit ses bornes a la seconde : une fraction ne fait pas une tranche de plus.
    let (debut, fin) = (debut.a_la_seconde(), fin.a_la_seconde());
    let mut v = Vec::new();
    let mut t = debut;
    while t < fin {
        let suivant = if fin.depuis(t).is_some_and(|d| d > std::time::Duration::from_secs(86_400)) {
            t.plus(86_400).ok_or_else(|| socle::echec::refus(socle::date::HORS_CALENDRIER))?
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
pub fn rattacher_simulation(ctx: &Contexte, suites: &mut Suites) -> R<i64> {
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
        reconstruire_effectifs(&ctx.racine)?;
    }
    Ok(faits)
}

/// L'AER Cinema Spectacle qu'une DSN mensuelle REELLE jouee appelle (`aer_fictive::archive`) : l'archive de nature 46,
/// ou `None` quand aucune n'est due (France Travail ne publie rien) ; un signalement (nature 04, 05, 07) n'en appelle pas.
/// Le fichier depose est relu la ou le depot l'a pris (`sorties/<mois>/<fichier>`).
fn aer_jouee(ctx: &Contexte, l: &Ligne) -> R<Option<Vec<u8>>> {
    if !l.reel() {
        return Ok(None);
    }
    if l.nature_dsn().is_some_and(|n| n != droit::nature_declaration::NatureDeLaDeclaration::Mensuelle) {
        return Ok(None);
    }
    let (Some(mois), Some(fichier)) = (l.mois(), l.fichier()) else { return Ok(None) };
    let chemin = ctx.racine.join("sorties").join(mois.to_string()).join(fichier);
    // Un fichier depose qui n'est plus la n'appelle pas d'attestation jouee.
    let octets = match std::fs::read(&chemin) {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque)),
    };
    crate::aer_fictive::archive(ctx, &octets, mois, ctx.aujourdhui)
}

/// Telecharge et archive ce que les organismes ont publie, de `debut` a `fin`. `client` : le client du service, avec son mot de passe (le canal DPAE passe le sien, avec
/// `canal` `Dpae`) ; `None` dans le monde : aucun mot de passe n'est pose, et cela se dit.
pub fn relever_retours(
    ctx: &Contexte,
    branchement: Branchement,
    debut: Moment,
    fin: Moment,
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
        releve.incidents.push(
            "Mot de passe net-entreprises absent : ni dans l'environnement, ni dans le coffre. `route coffre-poser, service net-entreprises`.".into(),
        );
        return Ok(releve);
    };
    match client.authentifier() {
        Ok(()) => {}
        Err(p) if p.est_api() => {
            releve.incidents.push(format!("Authentification refusee : {}", p.echec.phrase));
            return Ok(releve);
        }
        Err(p) => return Err(en_echec(p)),
    }
    let connus = deja_vus(ctx, canal)?;
    // UN 404 SUR UNE TRANCHE VEUT DIRE << RIEN SUR CETTE TRANCHE >> ; tout autre code arrete : un 401 ne
    // s'ameliorera pas au jour suivant.
    for (t0, t1) in tranches(debut, fin)? {
        match client.lister_retours(t0, t1) {
            Ok(trouves) => {
                for r in trouves {
                    if !est_connu(&connus, &r) {
                        releve.nouveaux.push(r.clone());
                    }
                    releve.trouves.push(r);
                }
            }
            Err(p) if p.est_api() => {
                if p.code == Some(404) {
                    continue;
                }
                releve.incidents.push(format!("Consultation interrompue : {}", p.echec.phrase));
                break;
            }
            Err(p) => return Err(en_echec(p)),
        }
    }
    if !releve.incidents.is_empty() && releve.trouves.is_empty() {
        return Ok(releve);
    }

    // UN RETOUR RATE N'EST PAS UN RETOUR VU : un echec reste NEUF, et se retente au releve suivant.
    let mut rates: Vec<Annonce> = Vec::new();
    let a_telecharger: Vec<Annonce> = if tout { releve.trouves.clone() } else { releve.nouveaux.clone() };
    for r in a_telecharger {
        let Some(url) = r.url.clone() else { continue };
        match client.telecharger(&url) {
            Ok((contenu, ctype)) => {
                let chemin = archiver(ctx, &r, &contenu, ctype.as_deref(), canal)?;
                // LE TAUX AT/MP NOTIFIE SE LIT DE LA NOTIFICATION (regle 1) : il se pose dans la surcouche, par les suites.
                if canal == CanalDesRetours::Dsn && r.nature_retour == NatureDuRetour::TauxAtMp {
                    suites.taux_atmp.push((r.clone(), chemin.clone()));
                }
                // UN SIGNALEMENT REFUSE B1-208-10 : l'ordre etait deja pris ; la conduite a tenir se dit.
                if canal == CanalDesRetours::Dsn && r.nature_retour == NatureDuRetour::CertificatDeConformite {
                    for d in crate::retours::declarations_du_bilan(&contenu)? {
                        releve.a_faire.extend(crate::ordre_signalements::conduite_b1_208_10(&d, r.flux.as_ref()));
                    }
                }
                // LE TAUX DU PAS SE LIT DU COMPTE RENDU (regle 1) : il va dans la fiche du salarie, par les suites.
                if canal == CanalDesRetours::Dsn && r.nature_retour == NatureDuRetour::TauxPas {
                    suites.taux_pas.push((r.clone(), chemin.clone()));
                }
                releve.archives.push(RetourArchive { retour: r, chemin, contenu });
            }
            Err(p) if p.est_api() => {
                releve.incidents.push(format!("Telechargement du retour nature {} : {}", r.nature_retour, p.echec.phrase));
                rates.push(r);
            }
            Err(p) => return Err(en_echec(p)),
        }
    }

    if canal == CanalDesRetours::Dsn {
        // L'EFFECTIF S'ECRIT SEUL : `config/effectifs.json` se refait depuis les archives.
        reconstruire_effectifs(&ctx.racine)?;
    }

    if !releve.trouves.is_empty() {
        let vus: Vec<Annonce> = releve.trouves.iter().filter(|r| !rates.contains(r)).cloned().collect();
        noter_vus(ctx, &vus, canal)?;
        // Un depot n'est fini que quand sa reponse est la : chaque retour se range en face du depot qui l'a
        // provoque -- par le flux, et par rien d'autre.
        let contenus: Vec<_> = releve.archives.iter().map(|a| (crate::etats::cle_du_contenu(&a.retour), a.contenu.clone())).collect();
        releve.rattaches = suites.garder(crate::etats::rattacher_retours_avec_contenus(&ctx.racine, &releve.trouves, &contenus))?;
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
/// `client` : le client Urssaf, avec son mot de passe ; `None` dans le monde : aucun mot de passe n'est pose.
pub fn relever_retours_dpae(
    ctx: &Contexte,
    branchement: Branchement,
    debut: Moment,
    fin: Moment,
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
        r.incidents.push("Mot de passe DPAE absent : ni dans l'environnement, ni dans le coffre. `route coffre-poser, service urssaf`.".into());
        return Ok(r);
    };
    let Some(flux) = flux else {
        return relever_retours(ctx, branchement, debut, fin, tout, Some(client), CanalDesRetours::Dpae, suites);
    };
    let mut r = Releve::vide();
    let trouves = client.authentifier().and_then(|_| client.lister_retours_du_flux(flux));
    match trouves {
        Ok(t) => r.trouves = t,
        Err(p) if p.est_api() => {
            r.incidents.push(format!("Consultation du flux {} : {}", flux, p.echec.phrase));
            return Ok(r);
        }
        Err(p) => return Err(en_echec(p)),
    }
    for x in r.trouves.clone() {
        let Some(url) = x.url.clone() else { continue };
        match client.telecharger(&url) {
            Ok((contenu, ctype)) => {
                let chemin = archiver(ctx, &x, &contenu, ctype.as_deref(), CanalDesRetours::Dpae)?;
                r.archives.push(RetourArchive { retour: x, chemin, contenu });
            }
            Err(p) if p.est_api() => r.incidents.push(format!("Telechargement nature {} : {}", x.nature_retour, p.echec.phrase)),
            Err(p) => return Err(en_echec(p)),
        }
    }
    if !r.trouves.is_empty() {
        noter_vus(ctx, &r.trouves, CanalDesRetours::Dpae)?;
        // Le chemin par flux accroche aussi ce qu'il trouve au journal.
        let contenus: Vec<_> = r.archives.iter().map(|a| (crate::etats::cle_du_contenu(&a.retour), a.contenu.clone())).collect();
        r.rattaches = suites.garder(crate::etats::rattacher_retours_avec_contenus(&ctx.racine, &r.trouves, &contenus))?;
    }
    Ok(r)
}

/// La derniere journee, comme `suivre_le_depot` et `bilan_de_la_repetition` la relevent : de hier a maintenant.
pub fn derniere_journee() -> R<(Moment, Moment)> {
    let fin = socle::temps::secondes();
    Ok((fin.plus(-86_400).ok_or_else(|| socle::echec::refus(socle::date::HORS_CALENDRIER))?, fin))
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
        let v = socle::num::dec_texte(&valeur).map_err(|_| erreur(Classe::Illisible, format!("nombre illisible : '{}'", valeur)))?;
        out.push(Mesure118 { code, valeur: v, debut: p.effet, fin: p.fin, calcule_le: p.calcule_le });
    }
    Ok(Some(out))
}

/// Un effectif qu'un compte rendu de nature 118 communique : son code, sa valeur, sa periode, le jour de son calcul.
struct Mesure118 {
    code: TypeDeParametreNeores,
    valeur: Decimal,
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
fn ecrire_effectifs(racine: &Path, existant: Option<&FichierEffectifs>, annees: EffectifsParAnnee) -> R<()> {
    let donnees = FichierEffectifs { quoi: QUOI_EFFECTIFS.to_string(), annees };
    if existant != Some(&donnees) {
        socle::ecriture::ecrire_json(&racine.join(droit::contexte::EFFECTIFS), &donnees, 2).map_err(|m| m.reclasser(Classe::Disque))?;
    }
    Ok(())
}

/// L'effectif de l'annee `an` est-il celui que la compagnie a dit (`dit_le`), et non celui d'un 118 ?
pub fn effectif_dit_par_la_compagnie(racine: &Path, an: i32) -> R<bool> {
    let an = Annee::nouvelle(i64::from(an))?;
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
/// Ne touche pas au fichier s'il n'y a aucun 118.
pub fn reconstruire_effectifs(racine: &Path) -> R<()> {
    let dossier = racine.join(crate::retours::RETOURS);
    // Sans dossier des retours, aucun 118 n'est arrive.
    let lecture = match std::fs::read_dir(&dossier) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
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
    let mut annees = EffectifsParAnnee::new();
    for nom in &noms {
        let p = dossier.join(nom);
        let octets = std::fs::read(&p).map_err(|e| socle::echec::du_disque(&p, &e).reclasser(Classe::Disque))?;
        let Some(mesures) = lire_crm118(&octets)? else { continue };
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
        return Ok(());
    }
    let existant = effectifs_lus(racine)?;
    for (an, dit) in dits(existant.as_ref()) {
        annees.entry(an).or_default().entry(TypeDeParametreNeores::EffectifMoyen).or_insert(dit);
    }
    ecrire_effectifs(racine, existant.as_ref(), annees)
}

/// L'EFFECTIF QUE LA COMPAGNIE DIT POUR L'ANNEE `an` (l'effectif annuel moyen de l'annee civile, L. 130-1, I, CSS ;
/// `atelier/reference/sources/code-de-la-securite-sociale/LISEZ-MOI.md`, « L'effectif au sens de la sécurité sociale ») :
/// une compagnie venue d'un autre logiciel n'a pas les 118 de ses annees passees. `Ok(false)` : un 118 de cette annee
/// est deja la, et c'est lui qui vaut ; rien n'est ecrit.
pub fn poser_effectif(racine: &Path, an: i32, valeur: Decimal, le: Jour) -> R<bool> {
    let existant = effectifs_lus(racine)?;
    let mut annees: EffectifsParAnnee = existant.iter().flat_map(|f| f.annees.iter()).map(|(an, c)| (*an, c.clone())).collect();
    let codes = annees.entry(Annee::nouvelle(i64::from(an))?).or_default();
    if codes.get(&TypeDeParametreNeores::EffectifMoyen).is_some_and(|a| a.dit_le.is_none()) {
        return Ok(false);
    }
    codes.insert(TypeDeParametreNeores::EffectifMoyen, EffectifEcrit { valeur, calcule_le: None, source: None, dit_le: Some(le) });
    ecrire_effectifs(racine, existant.as_ref(), annees)?;
    Ok(true)
}

/// Le resume d'une relève, pour une phrase d'incident : `"; ".join(rel.incidents)`.
pub fn incidents_en_phrase(r: &Releve) -> String {
    r.incidents.join("; ")
}
