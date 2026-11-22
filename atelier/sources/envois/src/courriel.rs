//! La boite courriel par IMAP et SMTP, avec un mot de passe d'application : la boite s'ouvre par un MOT DE PASSE
//! D'APPLICATION que chaque compagnie fabrique dans son compte Google (aucune permission Google, aucun audit). Le compte
//! Google autorise ne sert qu'au Drive.
//!
//! CE QUE CE MODULE OFFRE : le trait `Courriel` (`compte`, `envoyer(mime) -> identifiant`, `rechercher`,
//! `message_brut`, `fermer`), avec DEUX realisations : `Client`, le vrai (IMAP et SMTP ecrits ici sur un flux
//! rustls), et `BoiteSimulee`, la boite de la compagnie fictive, qui GARDE les messages et ne les envoie pas. Un
//! identifiant de message ENVOYE est son en-tete `Message-ID` ; celui d'un message RECU est son UID IMAP. La
//! recherche est celle du serveur (`UID SEARCH TEXT`), dans la boite de reception seule.
//!
//! LE SECRET : `Client::nouveau` recoit l'adresse et le mot de passe de la boite ; ce module n'ouvre jamais le coffre
//! (regle 3). Les espaces du mot de passe d'application (quatre groupes de quatre lettres) sont retires.
//!
//! LES SERVEURS : Gmail par defaut (`imap.gmail.com:993`, `smtp.gmail.com:465`, TLS implicite) ; une autre messagerie
//! se dit dans `config/courriel.json` (`{"imap": ..., "smtp": ..., "port_smtp": 465}`). Le port SMTP 587 declenche
//! STARTTLS ; tout autre port, TLS implicite.
//!
//! LE CERTIFICAT, ET L'ANTIVIRUS : verification normale contre les racines du SYSTEME (le verificateur de la
//! plate-forme, comme `transport.rs`), de sorte que la racine d'un antivirus (Avast « Web/Mail Shield », qui
//! s'intercale sur les ports 993 et 465) installee dans le magasin Windows passe, comme chez Thunderbird ou Outlook.
//! Delai de 60 s par connexion.
//!
//! L'IMAP : le moteur ne fait que `LOGIN`, `EXAMINE` (lecture seule), `UID SEARCH TEXT`, `UID FETCH BODY.PEEK[]`,
//! `LOGOUT`. Ce dialogue est ecrit ici sur un flux quelconque (`Read + Write` : un faux serveur en memoire dans les
//! tests), et les reponses sont lues par `imap-proto` ; seuls les litteraux `{n}` se lisent a la main, par leur
//! longueur.
//!
//! LE SMTP : `EHLO`, `STARTTLS` sur le port 587, `AUTH PLAIN` (ou `LOGIN` si le serveur n'offre que lui), `MAIL FROM`,
//! `RCPT TO`, `DATA`, `QUIT`, ecrits ici sur le meme flux rustls que l'IMAP (`Smtp`) ; la reponse se lit par son code
//! (RFC 5321, § 4.2), et un refus se dit par lui (`envoi_refuse`).
//!
//! Un mot de passe d'un caractere de controle est refuse ; le message envoye part avec des fins de ligne CRLF sur le
//! fil -- le message archive reste celui d'`envoi::en_mime_nomme`, en LF. Une ligne vide precede le point final quand le
//! message ne finit pas par CRLF (`en_mime_nomme` finit par LF).

use droit::fiche::Lue as _;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use imap_proto::{AttributeValue, MailboxDatum, Response, Status};

use droit::contexte::Contexte;
use socle::echec::{Classe, Echec, R};

pub const FICHIER_SERVEURS: &str = "courriel.json";
/// L'attente, par connexion.
pub const DELAI: Duration = Duration::from_secs(60);
pub const DOSSIER_RECEPTION: &str = "INBOX";
pub const DOSSIER_COURRIELS: &str = "courriels-simules";

/// La boite refuse, ou le reseau : un refus de classe `Courriel`. Jamais un mot de passe dedans.
#[track_caller]
fn erreur(m: impl Into<String>) -> Echec {
    socle::echec::erreur(Classe::Courriel, m)
}

/// La surface de `courriel.Client` : celle que `envoi` et la releve des signatures utilisent.
pub trait Courriel {
    /// L'adresse de la boite.
    fn compte(&self) -> &str;
    /// Envoie un message deja assemble ; rend son `Message-ID`.
    fn envoyer(&mut self, message_mime: &[u8]) -> R<crate::envoi::IdDeCourriel>;
    /// Les UID des messages de la boite de reception qui contiennent le texte, les plus recents d'abord.
    fn rechercher(&mut self, requete: &str, maximum: usize) -> R<Vec<String>>;
    /// Un message entier (RFC 822), par son UID.
    fn message_brut(&mut self, identifiant: &str) -> R<Vec<u8>>;
    /// Ferme la boite ; le serveur qui refuse la deconnexion se dit.
    fn fermer(&mut self) -> R<()>;
    /// La compagnie de la boite : c'est sa fiche qui porte la signature des courriels et le nom affiche
    /// (`signature_courriel`). Rien : les courriels partent tels que les modeles les ecrivent.
    fn racine(&self) -> Option<&Path> {
        None
    }
}

// ---------------------------------------------------------------------------------------------------- le disque

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serveurs {
    pub imap: String,
    pub port_imap: u16,
    pub smtp: String,
    pub port_smtp: u16,
}

impl Serveurs {
    pub fn gmail() -> Serveurs {
        Serveurs { imap: "imap.gmail.com".into(), port_imap: 993, smtp: "smtp.gmail.com".into(), port_smtp: 465 }
    }
}

/// LES SERVEURS QUE LA COMPAGNIE DIT (`config/courriel.json`) : chacun est facultatif, ce qu'il ne dit pas reste celui
/// de Gmail.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ServeursDits {
    imap: Option<String>,
    port_imap: Option<u16>,
    smtp: Option<String>,
    port_smtp: Option<u16>,
    #[serde(rename = "quoi")]
    _quoi: Option<String>,
    #[serde(rename = "note")]
    _note: Option<String>,
    #[serde(rename = "source")]
    _source: Option<String>,
}

/// Gmail par defaut ; `config/courriel.json` change les cles `imap`, `port_imap`, `smtp`, `port_smtp`.
pub fn serveurs(racine: &Path) -> R<Serveurs> {
    let p = racine.join(socle::fichiers::CONFIG).join(FICHIER_SERVEURS);
    let mut s = Serveurs::gmail();
    if !p.exists() {
        return Ok(s);
    }
    let dits: ServeursDits = socle::forme::lire_fichier(&p, "Les serveurs de la messagerie de la compagnie ne se lisent pas.").map_err(|e| e.reclasser(Classe::Courriel))?;
    if let Some(h) = dits.imap {
        s.imap = h;
    }
    if let Some(h) = dits.smtp {
        s.smtp = h;
    }
    if let Some(port) = dits.port_imap {
        s.port_imap = port;
    }
    if let Some(port) = dits.port_smtp {
        s.port_smtp = port;
    }
    Ok(s)
}

/// CE QUE LA BOITE A REPONDU, OU CE QUE LE SYSTEME OU UNE BIBLIOTHEQUE DIT D'ELLE, dit a l'administrateur : `phrase`, en
/// francais ; `detail` (le texte du serveur, du systeme ou de la bibliotheque) va a part (`Lieu::detail`), que le journal
/// des incidents porte a l'editeur, et dans la memoire des echecs -- comme une panne d'organisme
/// (`transport::panne_dite`).
#[track_caller]
fn dite(phrase: &str, detail: impl std::fmt::Display) -> Echec {
    let detail = detail.to_string();
    socle::echec::noter_echec(Some(Classe::Courriel), &detail);
    let a_part = socle::echec::Lieu { detail: Some(detail), ..socle::echec::Lieu::default() };
    Echec { classe: Some(Classe::Courriel), phrase: phrase.to_string(), a_part: Some(a_part.into()) }
}

/// LA SORTE D'UN REFUS DE LA BOITE, posee la ou le refus se fabrique (`mot_de_passe_refuse`, `injoignable`) : ce que le
/// journal des incidents en fait. Le mot de passe refuse est un refus d'usage ; la boite injoignable, le reseau, qui ne
/// dit rien du logiciel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SorteDeRefusDeLaBoite {
    MotDePasseRefuse,
    Injoignable,
    Autre,
}

/// UN REFUS DE LA BOITE, et sa sorte : ce que rend l'ouverture de la boite (`verifier`). Il se lit en `Echec` partout
/// ou la sorte ne sert pas.
#[derive(Debug, Clone)]
pub struct RefusDeLaBoite {
    pub echec: Echec,
    pub sorte: SorteDeRefusDeLaBoite,
}

impl From<RefusDeLaBoite> for Echec {
    fn from(r: RefusDeLaBoite) -> Echec {
        r.echec
    }
}

impl From<Echec> for RefusDeLaBoite {
    fn from(echec: Echec) -> RefusDeLaBoite {
        RefusDeLaBoite { echec, sorte: SorteDeRefusDeLaBoite::Autre }
    }
}

/// A QUEL MOMENT DU DIALOGUE IMAP la boite refuse : a l'identification (`LOGIN`, et l'accueil qui ferme), ou la boite
/// deja ouverte. Seule l'identification renvoie au mot de passe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Moment {
    Identification,
    Connectee,
}

/// LE REFUS DU SERVEUR A L'IDENTIFICATION (`texte`, tel qu'il l'a ecrit), en francais : le mot de passe refuse, que
/// l'on reconnait a ce que le serveur ecrit, ou un refus que l'on ne reconnait pas ; le texte du serveur va a part.
#[track_caller]
fn refus_a_l_identification(texte: &str) -> RefusDeLaBoite {
    if texte.contains("AUTHENTICATIONFAILED") || texte.contains("Username and Password not accepted") {
        return mot_de_passe_refuse(texte);
    }
    dite("La boîte courriel a refusé la connexion : vérifier l'adresse et le mot de passe d'application (Configuration › Mots de passe) ; nouvel essai plus tard.", texte).into()
}

/// LE REFUS DU SERVEUR LA BOITE DEJA OUVERTE (`texte`, tel qu'il l'a ecrit) : l'identification a passe, rien ne
/// renvoie au mot de passe ; le texte du serveur va a part.
#[track_caller]
fn refus_apres_connexion(texte: &str) -> RefusDeLaBoite {
    dite("La boîte courriel a refusé une demande du logiciel, la connexion ouverte : nouvel essai plus tard.", texte).into()
}

/// La boite refuse le mot de passe (`detail` : ce que le serveur a ecrit).
#[track_caller]
fn mot_de_passe_refuse(detail: impl std::fmt::Display) -> RefusDeLaBoite {
    RefusDeLaBoite {
        echec: dite(
            "La boîte refuse le mot de passe : pour Gmail, il faut un mot de passe d'application (validation en deux étapes activée, puis Compte Google › Sécurité › Mots de passe des applications), pas celui du compte.",
            detail,
        ),
        sorte: SorteDeRefusDeLaBoite::MotDePasseRefuse,
    }
}

/// UN MESSAGE QUE LE SERVEUR D'ENVOI (SMTP) N'A PAS PRIS, dit a l'administrateur ; le texte du serveur ou du systeme
/// va a part (`dite`). Le client se regle sur le code seul, jamais sur le texte : « An SMTP client MUST determine its
/// actions only by the reply code, not by the text » (RFC 5321, § 4.2,
/// `atelier/reference/sources/ietf/ietf-rfc5321-simple-mail-transfer-protocol-20081001.txt`, l. 2610-2611) ; un code
/// que la liste ne connait pas se lit par son premier chiffre (l. 2623-2625), 4yz passager, 5yz definitif (§ 4.2.1,
/// l. 2667-2690). Les codes nommes sont ceux du § 4.2.2-4.2.3 (l. 2870-2930) : 550 « mailbox unavailable (e.g., mailbox
/// not found, no access, or command rejected for policy reasons) », 551 « User not local », 553 « mailbox name not
/// allowed (e.g., mailbox syntax incorrect) ». Lequel des envois les rend, le § 4.3.2 (l. 3100-3115) le dit : 551 ne
/// repond qu'a `RCPT` (le destinataire) ; 553 repond a `MAIL` (l'expediteur, la boite d'envoi) comme a `RCPT` ; 550 a
/// `MAIL`, a `RCPT` et a `DATA` (« rejections for policy reasons ») : la phrase de 550 et 553 dit les possibilites sans
/// trancher. 535 est le mot de passe refuse (RFC 4954, § 6,
/// `ietf-rfc4954-smtp-service-extension-for-authentication-20070701.txt`, l. 600-605 : « Authentication credentials
/// invalid […] the client SHOULD ask the user to supply new credentials »).
#[track_caller]
fn envoi_refuse(arret: &ArretDeLEnvoi) -> Echec {
    match arret {
        ArretDeLEnvoi::Refus { code, texte } => {
            let detail = format!("réponse {} : {}", code, texte);
            match code {
                535 => mot_de_passe_refuse(detail).echec,
                551 => dite("Le serveur d'envoi refuse l'adresse du destinataire (une boîte qu'il ne sert pas) : vérifier l'adresse courriel du salarié. Le message n'est pas parti.", detail),
                553 => dite("Le serveur d'envoi refuse une adresse mal écrite, sans dire laquelle : celle du salarié destinataire, ou celle de la boîte d'envoi. Vérifier l'adresse courriel du salarié, puis celle de la boîte (Configuration › Mots de passe). Le message n'est pas parti.", detail),
                550 => dite("Le serveur d'envoi refuse le message sans dire pourquoi : l'adresse du salarié destinataire (boîte introuvable), celle de la boîte d'envoi, ou une règle du serveur. Vérifier d'abord l'adresse courriel du salarié. Le message n'est pas parti.", detail),
                500..=599 => dite("Le serveur d'envoi a refusé le message : il n'est pas parti. Nouvel essai plus tard.", detail),
                _ => dite("Le serveur d'envoi refuse le message pour l'instant : nouvel essai plus tard.", detail),
            }
        }
        ArretDeLEnvoi::Delai(detail) => dite("Le serveur d'envoi n'a pas répondu à temps : nouvel essai plus tard.", detail),
        ArretDeLEnvoi::Tls(detail) => tls(detail),
        ArretDeLEnvoi::Interrompu(detail) => dite("L'échange avec le serveur d'envoi s'est interrompu : vérifier la connexion internet ; nouvel essai plus tard.", detail),
    }
}

/// La boite ne se joint pas (`e` : ce que le systeme en dit). « injoignable » : le mot que `sauvegarde::hors_ligne` lit.
#[track_caller]
fn injoignable(e: impl std::fmt::Display) -> RefusDeLaBoite {
    RefusDeLaBoite {
        echec: dite("La boîte courriel est injoignable : vérifier la connexion internet ; nouvel essai plus tard.", format!("Serveur injoignable : {}", e)),
        sorte: SorteDeRefusDeLaBoite::Injoignable,
    }
}

/// La boite a rendu ce que ce dialogue ne lit pas (`detail` : ce qu'elle a rendu, ou ce que la bibliotheque en dit).
#[track_caller]
fn illisible(detail: impl std::fmt::Display) -> Echec {
    dite("La boîte courriel a rendu une réponse que le logiciel ne lit pas : nouvel essai plus tard.", detail)
}

/// La boite a ferme la connexion avant de tout rendre.
#[track_caller]
fn fermee_en_chemin() -> Echec {
    dite("La boîte courriel a fermé la connexion en chemin : nouvel essai plus tard.", "Connexion fermee par le serveur.")
}

/// La connexion securisee a la boite ne s'etablit pas (`e` : ce que la bibliotheque TLS en dit).
#[track_caller]
fn tls(e: impl std::fmt::Display) -> Echec {
    dite("La connexion sécurisée à la boîte courriel ne s'établit pas : nouvel essai plus tard.", format!("TLS : {}", e))
}

// ------------------------------------------------------------------------------------------------------- le TLS

/// Un flux lisible et inscriptible : une connexion TLS, ou un faux serveur en memoire.
pub trait Flux: Read + Write {}
impl<T: Read + Write + ?Sized> Flux for T {}

fn connecter(hote: &str, port: u16) -> Result<TcpStream, RefusDeLaBoite> {
    // En essai, le garde-fou reseau : aucune boite reelle.
    socle::garde_reseau::directe(hote).map_err(|e| RefusDeLaBoite { echec: e.reformuler(|m| format!("Serveur injoignable : {}", m)).reclasser(Classe::Courriel), sorte: SorteDeRefusDeLaBoite::Injoignable })?;
    let delai = DELAI;
    let adresses: Vec<_> = (hote, port).to_socket_addrs().map_err(injoignable)?.collect();
    let mut derniere = None;
    for a in adresses {
        match TcpStream::connect_timeout(&a, delai) {
            Ok(s) => {
                s.set_read_timeout(Some(delai)).map_err(injoignable)?;
                s.set_write_timeout(Some(delai)).map_err(injoignable)?;
                return Ok(s);
            }
            Err(e) => derniere = Some(e),
        }
    }
    Err(match derniere {
        Some(e) => injoignable(e),
        None => injoignable("aucune adresse"),
    })
}

/// La configuration TLS des connexions IMAP et SMTP : racines du systeme (`rustls-platform-verifier`).
fn configuration_tls() -> Result<Arc<rustls::ClientConfig>, rustls::Error> {
    use rustls_platform_verifier::BuilderVerifierExt;
    let fournisseur = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(fournisseur).with_safe_default_protocol_versions()?.with_platform_verifier()?.with_no_client_auth();
    Ok(Arc::new(config))
}

fn ouvrir_tls(hote: &str, port: u16) -> Result<Box<dyn Flux>, RefusDeLaBoite> {
    let tcp = connecter(hote, port)?;
    let nom = rustls::pki_types::ServerName::try_from(hote.to_string()).map_err(|e| erreur(format!("nom de serveur illisible : {}", e)))?;
    let connexion = rustls::ClientConnection::new(configuration_tls().map_err(tls)?, nom).map_err(tls)?;
    Ok(Box::new(rustls::StreamOwned::new(connexion, tcp)))
}

// ------------------------------------------------------------------------------------------------------- l'IMAP

/// Ce que le serveur a rendu d'utile a une commande.
#[derive(Debug, Default)]
pub struct Reponse {
    /// Les numeros d'une reponse `SEARCH`.
    pub recherche: Vec<u32>,
    /// Les corps `BODY[]` d'une reponse `FETCH`.
    pub corps: Vec<Vec<u8>>,
}

/// Le dialogue IMAP sur un flux. Pas de connexion ici : l'appelant donne le flux.
pub struct Imap<F: Read + Write> {
    flux: F,
    tampon: Vec<u8>,
    numero: u32,
}

/// Un litteral plus gros que cela n'est pas un courriel que ce logiciel attend.
const LITTERAL_MAXIMUM: usize = 256 * 1024 * 1024;

fn quoter(texte: &str) -> R<String> {
    if texte.chars().any(|c| c.is_control()) {
        return Err(erreur("Un caractere de controle ne passe pas dans une commande IMAP."));
    }
    Ok(format!("\"{}\"", texte.replace('\\', "\\\\").replace('"', "\\\"")))
}

/// La longueur d'un litteral annonce en fin de ligne (`... {123}\r\n`).
fn longueur_litteral(ligne: &[u8]) -> Option<usize> {
    let sans = ligne.strip_suffix(b"\r\n").or_else(|| ligne.strip_suffix(b"\n"))?;
    let sans = sans.strip_suffix(b"}")?;
    let ouvrante = sans.iter().rposition(|b| *b == b'{')?;
    let chiffres = &sans[ouvrante + 1..];
    if chiffres.is_empty() || !chiffres.iter().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // Une longueur qui ne tient pas dans un `usize` n'est pas un litteral que ce dialogue lit.
    chiffres.iter().try_fold(0usize, |n, c| n.checked_mul(10)?.checked_add(usize::from(c - b'0')))
}

fn es(e: std::io::Error) -> RefusDeLaBoite {
    use std::io::ErrorKind::*;
    match e.kind() {
        TimedOut | WouldBlock => injoignable(format!("delai depasse ({})", e)),
        _ => dite("La boîte courriel a coupé l'échange en chemin : nouvel essai plus tard.", e).into(),
    }
}

impl<F: Read + Write> Imap<F> {
    /// Lit l'accueil du serveur, puis `LOGIN` (l'identification) et `EXAMINE` (lecture seule) du dossier. Le refus rend
    /// sa sorte (`RefusDeLaBoite`).
    pub fn ouvrir(flux: F, compte: &str, mot_de_passe: &socle::secret::Secret, dossier: &str) -> Result<Imap<F>, RefusDeLaBoite> {
        let mut imap = Imap { flux, tampon: Vec::new(), numero: 0 };
        let accueil = imap.lire_unite()?;
        match Response::parse(&accueil) {
            Ok((_, Response::Data { status: Status::Bye, .. })) => return Err(refus_a_l_identification(String::from_utf8_lossy(&accueil).trim())),
            Ok(_) => {}
            Err(e) => return Err(illisible(format!("L'accueil du serveur ne se lit pas : {}", e)).into()),
        }
        imap.commande_en(&format!("LOGIN {} {}", quoter(compte)?, quoter(mot_de_passe.devoiler())?), Moment::Identification)?;
        imap.commande_en(&format!("EXAMINE {}", quoter(dossier)?), Moment::Connectee)?;
        Ok(imap)
    }

    fn lire_ligne(&mut self) -> Result<Vec<u8>, RefusDeLaBoite> {
        loop {
            if let Some(i) = self.tampon.iter().position(|b| *b == b'\n') {
                return Ok(self.tampon.drain(..=i).collect());
            }
            let mut bloc = [0u8; 8192];
            let n = self.flux.read(&mut bloc).map_err(es)?;
            if n == 0 {
                return Err(fermee_en_chemin().into());
            }
            self.tampon.extend_from_slice(&bloc[..n]);
        }
    }

    fn lire_octets(&mut self, n: usize) -> Result<Vec<u8>, RefusDeLaBoite> {
        while self.tampon.len() < n {
            let mut bloc = [0u8; 8192];
            let lus = self.flux.read(&mut bloc).map_err(es)?;
            if lus == 0 {
                return Err(fermee_en_chemin().into());
            }
            self.tampon.extend_from_slice(&bloc[..lus]);
        }
        Ok(self.tampon.drain(..n).collect())
    }

    /// Une reponse entiere : une ligne, et tout ce que ses litteraux `{n}` en font suivre, par leur longueur.
    fn lire_unite(&mut self) -> Result<Vec<u8>, RefusDeLaBoite> {
        let mut unite = Vec::new();
        loop {
            let ligne = self.lire_ligne()?;
            unite.extend_from_slice(&ligne);
            match longueur_litteral(&ligne) {
                Some(n) if n > LITTERAL_MAXIMUM => return Err(illisible(format!("Reponse du serveur trop grosse : un litteral de {} octets.", n)).into()),
                Some(n) => {
                    let corps = self.lire_octets(n)?;
                    unite.extend_from_slice(&corps);
                }
                None => return Ok(unite),
            }
        }
    }

    /// Une commande, la boite deja ouverte, et les reponses qui lui sont propres jusqu'a la reponse etiquetee. Un refus
    /// (`NO`, `BAD`) se dit en francais ; le texte du serveur va a part.
    pub fn commande(&mut self, texte: &str) -> R<Reponse> {
        Ok(self.commande_en(texte, Moment::Connectee)?)
    }

    /// Une commande, a l'identification (`LOGIN`) ou la boite ouverte : le refus du serveur en suit la phrase et la
    /// sorte (`refus_a_l_identification`, `refus_apres_connexion`).
    fn commande_en(&mut self, texte: &str, moment: Moment) -> Result<Reponse, RefusDeLaBoite> {
        let refus = |texte: &str| match moment {
            Moment::Identification => refus_a_l_identification(texte),
            Moment::Connectee => refus_apres_connexion(texte),
        };
        self.numero += 1;
        let etiquette = format!("A{:04}", self.numero);
        let ligne = format!("{} {}\r\n", etiquette, texte);
        self.flux.write_all(ligne.as_bytes()).map_err(es)?;
        self.flux.flush().map_err(es)?;
        let mut reponse = Reponse::default();
        loop {
            let unite = self.lire_unite()?;
            if unite.starts_with(format!("{} ", etiquette).as_bytes()) {
                return match Response::parse(&unite) {
                    Ok((_, Response::Done { status: Status::Ok, .. })) => Ok(reponse),
                    Ok((_, Response::Done { .. })) => {
                        let brut = String::from_utf8_lossy(&unite[etiquette.len() + 1..]).trim().to_string();
                        Err(refus(&brut))
                    }
                    _ => Err(illisible(crate::transport::debut_lisible(&unite, 800)).into()),
                };
            }
            if unite.starts_with(b"* ") {
                match Response::parse(&unite) {
                    Ok((_, Response::MailboxData(MailboxDatum::Search(v)))) => reponse.recherche.extend(v),
                    Ok((_, Response::Fetch(_, attributs))) => {
                        for a in attributs {
                            match a {
                                AttributeValue::BodySection { data: Some(d), .. } | AttributeValue::Rfc822(Some(d)) => {
                                    reponse.corps.push(d.into_owned())
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok((_, Response::Data { status: Status::Bye, .. })) => {
                        return Err(refus(String::from_utf8_lossy(&unite).trim()));
                    }
                    // Le reste (FLAGS, EXISTS, CAPABILITY...) n'interesse pas ce dialogue ; une ligne que le
                    // parseur ne lit pas non plus.
                    _ => {}
                }
                continue;
            }
            if unite.starts_with(b"+") {
                return Err(illisible("Le serveur attend une suite que ce dialogue ne donne jamais.").into());
            }
        }
    }

    /// `UID SEARCH TEXT "..."` : les UID, les plus recents d'abord, `maximum` au plus. Les guillemets de la syntaxe
    /// Gmail sont toleres et retires.
    pub fn rechercher(&mut self, requete: &str, maximum: usize) -> R<Vec<String>> {
        let texte = requete.trim().trim_matches('"');
        let r = self.commande(&format!("UID SEARCH TEXT {}", quoter(texte)?))?;
        Ok(r.recherche.iter().rev().take(maximum).map(|u| u.to_string()).collect())
    }

    /// `UID FETCH <uid> (BODY.PEEK[])` : un message entier, sans le marquer lu.
    pub fn message_brut(&mut self, uid: &str) -> R<Vec<u8>> {
        if uid.is_empty() || !uid.bytes().all(|c| c.is_ascii_digit()) {
            return Err(erreur(format!("Message {} introuvable.", uid)));
        }
        let r = self.commande(&format!("UID FETCH {} (BODY.PEEK[])", uid))?;
        r.corps.into_iter().next().ok_or_else(|| erreur(format!("Message {} introuvable.", uid)))
    }

    /// `LOGOUT` ; un serveur qui la refuse se dit. Le `* BYE` qui la suit est la reponse attendue, pas un refus : « The
    /// server MUST send a BYE untagged response before the (tagged) OK response » (RFC 3501, § 6.1.3,
    /// `atelier/reference/sources/ietf-rfc3501-imap/ietf-rfc3501-imap4rev1-20261121.txt`, l. 1416-1419).
    pub fn deconnecter(&mut self) -> R<()> {
        self.numero += 1;
        let etiquette = format!("A{:04}", self.numero);
        self.flux.write_all(format!("{} LOGOUT\r\n", etiquette).as_bytes()).map_err(es)?;
        self.flux.flush().map_err(es)?;
        loop {
            let unite = self.lire_unite()?;
            if !unite.starts_with(format!("{} ", etiquette).as_bytes()) {
                continue;
            }
            return match Response::parse(&unite) {
                Ok((_, Response::Done { status: Status::Ok, .. })) => Ok(()),
                Ok((_, Response::Done { .. })) => Err(refus_apres_connexion(String::from_utf8_lossy(&unite[etiquette.len() + 1..]).trim()).echec),
                _ => Err(illisible(crate::transport::debut_lisible(&unite, 800))),
            };
        }
    }
}

// ------------------------------------------------------------------------------------------------------- le SMTP

/// CE QUI ARRETE UN ENVOI SMTP, avant d'etre dit a l'administrateur (`envoi_refuse`). Le detail est le texte du serveur
/// ou du systeme, jamais le mot de passe.
#[derive(Debug)]
enum ArretDeLEnvoi {
    /// Le serveur a repondu par un code negatif, 4yz ou 5yz (RFC 5321, § 4.2.1) : son code et son texte.
    Refus { code: u16, texte: String },
    /// Le serveur n'a pas repondu dans le delai (`DELAI`).
    Delai(String),
    /// La configuration de la connexion securisee ne se fabrique pas.
    Tls(String),
    /// Le reste : le reseau, une reponse que ce dialogue ne lit pas, un serveur qui n'offre pas ce que l'envoi demande.
    Interrompu(String),
}

fn interrompu(detail: impl Into<String>) -> ArretDeLEnvoi {
    ArretDeLEnvoi::Interrompu(detail.into())
}

fn arret_es(e: std::io::Error) -> ArretDeLEnvoi {
    use std::io::ErrorKind::*;
    match e.kind() {
        TimedOut | WouldBlock => ArretDeLEnvoi::Delai(format!("délai dépassé ({})", e)),
        _ => interrompu(e.to_string()),
    }
}

/// Une reponse plus longue n'est pas une reponse que ce dialogue lit : une ligne de reponse fait 512 octets au plus
/// (RFC 5321, § 4.5.3.1.5, l. 3506-3507), et la reponse a `EHLO`, la plus longue, en compte quelques dizaines.
const REPONSE_SMTP_MAXIMUM: usize = 64 * 1024;

/// Une reponse du serveur : son code, et le texte de chacune de ses lignes.
struct ReponseSmtp {
    code: u16,
    lignes: Vec<String>,
}

/// Ce que le serveur offre, lu dans sa reponse a `EHLO` : apres la premiere ligne, qui le nomme, une ligne par
/// extension, `ehlo-keyword *( SP ehlo-param )`, le mot-cle reconnu sans egard a la casse (RFC 5321, § 4.1.1.1,
/// l. 1832-1872).
#[derive(Default)]
struct Extensions {
    starttls: bool,
    plain: bool,
    login: bool,
    huit_bits: bool,
    smtputf8: bool,
}

impl Extensions {
    fn de(reponse: &ReponseSmtp) -> Extensions {
        let mut e = Extensions::default();
        for ligne in reponse.lignes.iter().skip(1) {
            let mut mots = ligne.split_ascii_whitespace();
            let Some(cle) = mots.next() else { continue };
            match cle.to_ascii_uppercase().as_str() {
                "STARTTLS" => e.starttls = true,
                "8BITMIME" => e.huit_bits = true,
                "SMTPUTF8" => e.smtputf8 = true,
                "AUTH" => {
                    for mecanisme in mots {
                        match mecanisme.to_ascii_uppercase().as_str() {
                            "PLAIN" => e.plain = true,
                            "LOGIN" => e.login = true,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        e
    }
}

/// Une ligne sans sa fin, CRLF ou LF.
fn sans_fin_de_ligne(ligne: &[u8]) -> &[u8] {
    match ligne.strip_suffix(b"\r\n") {
        Some(l) => l,
        None => match ligne.strip_suffix(b"\n") {
            Some(l) => l,
            None => ligne,
        },
    }
}

/// La transparence (RFC 5321, § 4.5.2, l. 3431-3433) : « Before sending a line of mail text, the SMTP client checks
/// the first character of the line. If it is a period, one additional period is inserted at the beginning of the
/// line. » `donnees` est en CRLF (`donnees_du_fil`).
fn transparent(donnees: &[u8]) -> Vec<u8> {
    let mut sortie = Vec::with_capacity(donnees.len() + 16);
    let mut debut_de_ligne = true;
    for &octet in donnees {
        if debut_de_ligne && octet == b'.' {
            sortie.push(b'.');
        }
        sortie.push(octet);
        debut_de_ligne = octet == b'\n';
    }
    sortie
}

/// Le dialogue SMTP sur un flux ; l'appelant donne le flux.
struct Smtp<F: Read + Write> {
    flux: F,
    tampon: Vec<u8>,
}

impl<F: Read + Write> Smtp<F> {
    fn sur(flux: F) -> Smtp<F> {
        Smtp { flux, tampon: Vec::new() }
    }

    /// Une ligne ; `lus` : ce que la reponse a deja de lignes, pour la borner.
    fn lire_ligne(&mut self, lus: usize) -> Result<Vec<u8>, ArretDeLEnvoi> {
        loop {
            if let Some(i) = self.tampon.iter().position(|b| *b == b'\n') {
                return Ok(self.tampon.drain(..=i).collect());
            }
            if lus + self.tampon.len() > REPONSE_SMTP_MAXIMUM {
                return Err(interrompu("Réponse du serveur trop longue."));
            }
            let mut bloc = [0u8; 4096];
            let n = self.flux.read(&mut bloc).map_err(arret_es)?;
            if n == 0 {
                return Err(interrompu("Connexion fermée par le serveur."));
            }
            self.tampon.extend_from_slice(&bloc[..n]);
        }
    }

    /// Une reponse entiere (RFC 5321, § 4.2, l. 2597-2598) : `Reply-code "-" [ textstring ]` pour chaque ligne qui en
    /// annonce une autre, `Reply-code [ SP textstring ]` pour la derniere, le meme code a chaque ligne ; le code seul est
    /// une reponse (l. 2570-2573). Un premier chiffre hors de 2 a 5 est une faute du serveur (l. 2639-2640).
    fn lire_reponse(&mut self) -> Result<ReponseSmtp, ArretDeLEnvoi> {
        let mut lignes = Vec::new();
        let mut code_lu: Option<u16> = None;
        let mut lus = 0;
        loop {
            let brute = self.lire_ligne(lus)?;
            lus += brute.len();
            let ligne = sans_fin_de_ligne(&brute);
            let illisible = || interrompu(format!("Réponse du serveur illisible : {}", String::from_utf8_lossy(ligne).trim()));
            let chiffres = match ligne.get(..3) {
                Some(c) if c.iter().all(u8::is_ascii_digit) => c,
                _ => return Err(illisible()),
            };
            let code = u16::from(chiffres[0] - b'0') * 100 + u16::from(chiffres[1] - b'0') * 10 + u16::from(chiffres[2] - b'0');
            if !(2..=5).contains(&(code / 100)) || code_lu.is_some_and(|c| c != code) {
                return Err(illisible());
            }
            code_lu = Some(code);
            let (encore, texte): (bool, &[u8]) = match ligne.get(3) {
                Some(b'-') => (true, &ligne[4..]),
                Some(b' ') => (false, &ligne[4..]),
                None => (false, &[]),
                Some(_) => return Err(illisible()),
            };
            lignes.push(String::from_utf8_lossy(texte).into_owned());
            if !encore {
                return Ok(ReponseSmtp { code, lignes });
            }
        }
    }

    fn ecrire(&mut self, octets: &[u8]) -> Result<(), ArretDeLEnvoi> {
        self.flux.write_all(octets).map_err(arret_es)?;
        self.flux.flush().map_err(arret_es)
    }

    /// La reponse a ce qui vient d'etre ecrit : negative (4yz, 5yz), elle arrete l'envoi avec son code ; positive, elle
    /// doit etre de la sorte attendue (`chiffre` : 2, l'action faite ; 3, le serveur attend la suite ; RFC 5321,
    /// § 4.2.1, l. 2656-2666).
    fn attendre(&mut self, chiffre: u16) -> Result<ReponseSmtp, ArretDeLEnvoi> {
        let r = self.lire_reponse()?;
        match r.code / 100 {
            4 | 5 => Err(ArretDeLEnvoi::Refus { code: r.code, texte: r.lignes.join(" / ") }),
            c if c == chiffre => Ok(r),
            _ => Err(interrompu(format!("Réponse {} inattendue : {}", r.code, r.lignes.join(" / ")))),
        }
    }

    fn commande(&mut self, ligne: &str, chiffre: u16) -> Result<ReponseSmtp, ArretDeLEnvoi> {
        self.ecrire(format!("{}\r\n", ligne).as_bytes())?;
        self.attendre(chiffre)
    }

    /// L'accueil du serveur (`220`, RFC 5321, § 3.1 et § 4.2, l. 2588-2593), puis `EHLO`.
    fn saluer(&mut self) -> Result<Extensions, ArretDeLEnvoi> {
        self.attendre(2)?;
        self.ehlo()
    }

    /// `EHLO` : le client n'a pas de nom de domaine qui ait un sens, il envoie un litteral d'adresse (RFC 5321,
    /// § 4.1.1.1, l. 1785-1801 ; § 4.1.3, l. 2386-2392), celui de la machine elle-meme.
    fn ehlo(&mut self) -> Result<Extensions, ArretDeLEnvoi> {
        let r = self.commande("EHLO [127.0.0.1]", 2)?;
        Ok(Extensions::de(&r))
    }

    /// Le flux, pour y poser TLS apres `STARTTLS` : le serveur n'a rien dit d'autre avant la negociation.
    fn flux(self) -> Result<F, ArretDeLEnvoi> {
        if !self.tampon.is_empty() {
            return Err(interrompu("Le serveur a écrit avant la négociation TLS."));
        }
        Ok(self.flux)
    }

    /// L'identification. `AUTH PLAIN` et sa reponse initiale (RFC 4954, § 4, l. 139-150), le message de la RFC 4616
    /// (§ 2, l. 122-126 : `[authzid] UTF8NUL authcid UTF8NUL passwd`) ; si le serveur n'offre que lui, `AUTH LOGIN`
    /// (draft-murchison-sasl-login-00, § 2.1 : l'identifiant, puis le mot de passe, chacun apres un defi dont le contenu
    /// « SHOULD be ignored »). Chaque reponse du client est en base64 (RFC 4954, § 4, l. 192-194). Un serveur qui
    /// n'offre ni l'un ni l'autre arrete l'envoi.
    fn identifier(&mut self, ext: &Extensions, compte: &str, mot_de_passe: &socle::secret::Secret) -> Result<(), ArretDeLEnvoi> {
        if ext.plain {
            let message = format!("\0{}\0{}", compte, mot_de_passe.devoiler());
            self.commande(&format!("AUTH PLAIN {}", socle::base64::ecrire(message.as_bytes())), 2)?;
            return Ok(());
        }
        if ext.login {
            self.commande("AUTH LOGIN", 3)?;
            self.commande(&socle::base64::ecrire(compte.as_bytes()), 3)?;
            self.commande(&socle::base64::ecrire(mot_de_passe.devoiler().as_bytes()), 2)?;
            return Ok(());
        }
        Err(interrompu("Le serveur d'envoi n'offre ni AUTH PLAIN ni AUTH LOGIN."))
    }

    /// La transaction (RFC 5321, § 3.3, l. 1017-1027) : `MAIL FROM:<...>`, un `RCPT TO:<...>` par destinataire, `DATA`
    /// (que suit `354`), les donnees rendues transparentes (`transparent`) et leur fin, `<CRLF>.<CRLF>` (§ 4.1.1.4,
    /// l. 2003-2006). Une adresse qui n'est pas en ASCII part avec `SMTPUTF8` (RFC 6531, § 3.4) ; un message qui n'est
    /// pas en ASCII, avec `BODY=8BITMIME` (RFC 6152, § 3) : si le serveur ne l'offre pas, l'envoi s'arrete.
    fn transmettre(&mut self, ext: &Extensions, expediteur: &str, destinataires: &[String], donnees: &[u8]) -> Result<(), ArretDeLEnvoi> {
        let mut options = String::new();
        if !expediteur.is_ascii() || destinataires.iter().any(|d| !d.is_ascii()) {
            if !ext.smtputf8 {
                return Err(interrompu("Une adresse n'est pas en ASCII, et le serveur d'envoi n'offre pas SMTPUTF8."));
            }
            options.push_str(" SMTPUTF8");
        }
        if !donnees.is_ascii() {
            if !ext.huit_bits {
                return Err(interrompu("Le message n'est pas en ASCII, et le serveur d'envoi n'offre pas 8BITMIME."));
            }
            options.push_str(" BODY=8BITMIME");
        }
        self.commande(&format!("MAIL FROM:<{}>{}", expediteur, options), 2)?;
        for d in destinataires {
            self.commande(&format!("RCPT TO:<{}>", d), 2)?;
        }
        self.commande("DATA", 3)?;
        let mut corps = transparent(donnees);
        corps.extend_from_slice(b"\r\n.\r\n");
        self.ecrire(&corps)?;
        self.attendre(2)?;
        Ok(())
    }
}

/// La connexion au serveur d'envoi ; ce que le systeme en dit arrete l'envoi.
fn connecter_smtp(hote: &str, port: u16) -> Result<TcpStream, ArretDeLEnvoi> {
    let adresses: Vec<_> = (hote, port).to_socket_addrs().map_err(arret_es)?.collect();
    let mut derniere = None;
    for a in adresses {
        match TcpStream::connect_timeout(&a, DELAI) {
            Ok(s) => {
                s.set_read_timeout(Some(DELAI)).map_err(arret_es)?;
                s.set_write_timeout(Some(DELAI)).map_err(arret_es)?;
                return Ok(s);
            }
            Err(e) => derniere = Some(e),
        }
    }
    Err(match derniere {
        Some(e) => arret_es(e),
        None => interrompu("Le nom du serveur ne donne aucune adresse."),
    })
}

/// TLS sur la connexion, avec la configuration de l'IMAP (`configuration_tls`) ; la poignee de main se fait au premier
/// echange.
fn tls_smtp(hote: &str, tcp: TcpStream) -> Result<Box<dyn Flux>, ArretDeLEnvoi> {
    let config = configuration_tls().map_err(|e| ArretDeLEnvoi::Tls(e.to_string()))?;
    let nom = rustls::pki_types::ServerName::try_from(hote.to_string()).map_err(|e| interrompu(format!("nom de serveur illisible : {}", e)))?;
    let connexion = rustls::ClientConnection::new(config, nom).map_err(|e| interrompu(e.to_string()))?;
    Ok(Box::new(rustls::StreamOwned::new(connexion, tcp)))
}

/// L'ENVOI D'UN MESSAGE au serveur `hote:port`. TLS des la connexion, sauf sur le port 587 : la connexion commence en
/// clair et passe en TLS par `STARTTLS` (RFC 3207, § 4, l. 96-110), puis reprend a `EHLO`, le client oubliant ce que le
/// serveur a dit avant (§ 4.2, l. 208-220). `QUIT` clot l'echange quand le serveur parle encore, la transaction finie
/// ou refusee (RFC 5321, § 4.1.1.10, l. 2207-2214) ; sa reponse ne change rien a ce qui est fait : le serveur « MUST
/// cancel any pending transaction, but not undo any previously completed transaction » (l. 2215-2218).
fn session_smtp(hote: &str, port: u16, compte: &str, mot_de_passe: &socle::secret::Secret, destinataires: &[String], donnees: &[u8]) -> Result<(), ArretDeLEnvoi> {
    let tcp = connecter_smtp(hote, port)?;
    let (mut smtp, ext) = if port == 587 {
        let mut clair = Smtp::sur(tcp);
        let ext = clair.saluer()?;
        if !ext.starttls {
            return Err(interrompu("Le serveur d'envoi n'offre pas STARTTLS."));
        }
        clair.commande("STARTTLS", 2)?;
        let mut chiffre = Smtp::sur(tls_smtp(hote, clair.flux()?)?);
        let ext = chiffre.ehlo()?;
        (chiffre, ext)
    } else {
        let mut chiffre = Smtp::sur(tls_smtp(hote, tcp)?);
        let ext = chiffre.saluer()?;
        (chiffre, ext)
    };
    let issue = match smtp.identifier(&ext, compte, mot_de_passe) {
        Ok(()) => smtp.transmettre(&ext, compte, destinataires, donnees),
        Err(arret) => Err(arret),
    };
    if matches!(issue, Ok(()) | Err(ArretDeLEnvoi::Refus { .. })) {
        let _ = smtp.commande("QUIT", 2);
    }
    issue
}

/// UNE ADRESSE QUI SE MET DANS L'ENVELOPPE : `Local-part "@" Domain` (RFC 5321, § 4.1.2, l. 2314), entre chevrons
/// (`Path`, l. 2264), sans espace, caractere de controle ni chevron, qui casseraient la commande. Le reste de sa
/// syntaxe, le serveur le juge (553).
fn adresse_d_enveloppe(adresse: &str) -> Result<(), &'static str> {
    let interdit = |c: char| c.is_whitespace() || c.is_control() || c == '<' || c == '>';
    match adresse.rsplit_once('@') {
        Some((locale, domaine)) if !locale.is_empty() && !domaine.is_empty() && !adresse.chars().any(interdit) => Ok(()),
        _ => Err("Une adresse sans partie avant ou après l'arobase, ou avec un espace, un caractère de contrôle ou un chevron."),
    }
}

// --------------------------------------------------------------------------------------------------- le client

/// IMAP pour lire, SMTP pour envoyer. Le mot de passe ne vit qu'en memoire, le temps d'une connexion ; rien n'est
/// journalise.
pub struct Client {
    racine: PathBuf,
    compte: String,
    mot_de_passe: socle::secret::Secret,
    pub serveurs: Serveurs,
    imap: Option<Imap<Box<dyn Flux>>>,
}

impl Client {
    /// `compte` : l'adresse de la boite ; `mot_de_passe` : celui que l'appelant a lu au coffre (les espaces d'un mot de
    /// passe d'application se retirent ici). Les serveurs se lisent dans `config/courriel.json` de `racine`.
    pub fn nouveau(racine: &Path, compte: &str, mot_de_passe: &socle::secret::Secret) -> R<Client> {
        Ok(Client {
            racine: racine.to_path_buf(),
            compte: compte.trim().to_lowercase(),
            mot_de_passe: mot_de_passe.transforme(|m| m.replace(' ', "")),
            serveurs: serveurs(racine)?,
            imap: None,
        })
    }

    fn boite(&mut self) -> Result<&mut Imap<Box<dyn Flux>>, RefusDeLaBoite> {
        if self.imap.is_none() {
            let flux = ouvrir_tls(&self.serveurs.imap, self.serveurs.port_imap)?;
            self.imap = Some(Imap::ouvrir(flux, &self.compte, &self.mot_de_passe, DOSSIER_RECEPTION)?);
        }
        Ok(self.imap.as_mut().expect("boite ouverte"))
    }
}

/// `\r\n|\n|\r(?!\n)` -> CRLF.
fn crlf(o: &[u8]) -> Vec<u8> {
    let mut s = Vec::with_capacity(o.len() + o.len() / 40);
    let mut i = 0;
    while i < o.len() {
        match o[i] {
            b'\r' if o.get(i + 1) == Some(&b'\n') => {
                s.extend_from_slice(b"\r\n");
                i += 2;
            }
            b'\r' | b'\n' => {
                s.extend_from_slice(b"\r\n");
                i += 1;
            }
            c => {
                s.push(c);
                i += 1;
            }
        }
    }
    s
}

/// Les donnees que `DATA` transmet : le message en CRLF, sans sa derniere fin de ligne, que la fin des donnees
/// (`<CRLF>.<CRLF>`, `Smtp::transmettre`) porte. Le protocole en veut une quand le message ne finit pas par CRLF (et
/// les octets d'`en_mime_nomme` finissent par un LF seul : une ligne vide precede donc le point). On ne retire la fin
/// de ligne que si le message en avait deja une, CRLF.
pub fn donnees_du_fil(brut: &[u8]) -> Vec<u8> {
    let mut sur_le_fil = crlf(brut);
    if brut.ends_with(b"\r\n") && sur_le_fil.ends_with(b"\r\n") {
        sur_le_fil.truncate(sur_le_fil.len() - 2);
    }
    sur_le_fil
}

/// Le premier en-tete de ce nom, sa valeur dans le texte du message.
fn premier_entete(entetes: &[mailparse::MailHeader], nom: &str) -> Option<String> {
    entetes.iter().find(|h| h.get_key_ref().eq_ignore_ascii_case(nom)).map(|h| String::from_utf8_lossy(h.get_value_raw()).into_owned())
}

/// Les en-tetes qui portent les destinataires d'un message.
const CHAMPS_DESTINATAIRES: [&str; 3] = ["To", "Cc", "Bcc"];

/// Une adresse de destinataire qui ne se lit pas (`detail` : ce que la bibliotheque en dit, a part) : le message ne part
/// pas.
#[track_caller]
fn destinataire_illisible(detail: impl std::fmt::Display) -> Echec {
    dite("Une adresse de destinataire ne se lit pas : vérifier l'adresse courriel du salarié. Le message n'est pas parti.", detail)
}

/// Les adresses de `To`, `Cc` et `Bcc` (le premier en-tete de chaque nom, comme `m.get`). Un en-tete qui ne se lit pas
/// en adresses refuse : le message ne part pas sans une partie de ses destinataires.
fn destinataires(entetes: &[mailparse::MailHeader]) -> R<Vec<String>> {
    let mut sortie = Vec::new();
    for champ in CHAMPS_DESTINATAIRES {
        let Some(valeur) = premier_entete(entetes, champ) else { continue };
        let liste = mailparse::addrparse(&valeur).map_err(|e| destinataire_illisible(format!("{} : {}", champ, e)))?;
        for a in liste.iter() {
            match a {
                mailparse::MailAddr::Single(s) => sortie.push(s.addr.clone()),
                mailparse::MailAddr::Group(g) => sortie.extend(g.addrs.iter().map(|s| s.addr.clone())),
            }
        }
    }
    Ok(sortie)
}

impl Courriel for Client {
    fn compte(&self) -> &str {
        &self.compte
    }

    fn racine(&self) -> Option<&Path> {
        Some(&self.racine)
    }

    /// Envoie un message deja assemble. Rend son `Message-ID` (ajoute s'il manque : c'est par lui que la releve
    /// reconnait nos propres envois).
    fn envoyer(&mut self, message_mime: &[u8]) -> R<crate::envoi::IdDeCourriel> {
        let (entetes, _) = mailparse::parse_headers(message_mime).map_err(|e| dite("Le message que le logiciel a assemblé ne se lit pas : il n'est pas parti.", e))?;
        let mut brut = message_mime.to_vec();
        let ident = match premier_entete(&entetes, "Message-ID").map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
            // Le message, le logiciel l'a assemble : son identifiant a la forme du sien.
            Some(i) => crate::envoi::IdDeCourriel::lire(&i).map_err(|e| e.reclasser(Classe::Courriel))?,
            None => {
                let domaine = match self.compte.rsplit_once('@') {
                    Some((_, d)) => d,
                    None => self.compte.as_str(),
                };
                let i = crate::envoi::nouveau_message_id(domaine).map_err(|e| e.reclasser(Classe::Courriel))?;
                let mut avec = format!("Message-ID: {}\n", i).into_bytes();
                avec.extend_from_slice(&brut);
                brut = avec;
                i
            }
        };
        let destinataires = destinataires(&entetes)?;
        for a in &destinataires {
            adresse_d_enveloppe(a).map_err(destinataire_illisible)?;
        }
        adresse_d_enveloppe(&self.compte).map_err(|e| dite("L'adresse de la boîte courriel ne se lit pas : la reposer dans Configuration › Mots de passe. Le message n'est pas parti.", e))?;
        if destinataires.is_empty() {
            return Err(dite("Le message n'a aucun destinataire : vérifier l'adresse courriel du salarié. Il n'est pas parti.", "Aucune adresse dans To, Cc ni Bcc."));
        }
        let (hote, port) = (self.serveurs.smtp.clone(), self.serveurs.port_smtp);
        // En essai, le garde-fou reseau : aucun serveur d'envoi reel.
        socle::garde_reseau::directe(&hote).map_err(|e| e.reformuler(|m| format!("Serveur injoignable : {}", m)).reclasser(Classe::Courriel))?;
        session_smtp(&hote, port, &self.compte, &self.mot_de_passe, &destinataires, &donnees_du_fil(&brut)).map_err(|a| envoi_refuse(&a))?;
        Ok(ident)
    }

    fn rechercher(&mut self, requete: &str, maximum: usize) -> R<Vec<String>> {
        self.boite()?.rechercher(requete, maximum)
    }

    fn message_brut(&mut self, identifiant: &str) -> R<Vec<u8>> {
        self.boite()?.message_brut(identifiant)
    }

    fn fermer(&mut self) -> R<()> {
        match self.imap.take() {
            Some(mut i) => i.deconnecter(),
            None => Ok(()),
        }
    }
}

// ----------------------------------------------------------------------------------------------- la verification

/// La boite s'ouvre-t-elle avec ce client ? UNE tentative IMAP. Rend ce qui est ouvert, ou le refus de la boite --
/// jamais le mot de passe -- avec sa sorte (`RefusDeLaBoite`).
pub fn verifier(mut c: Client) -> Result<String, RefusDeLaBoite> {
    c.boite()?;
    c.fermer()?;
    Ok(format!("Boîte {} ouverte ({}).", c.compte, c.serveurs.imap))
}

// ---------------------------------------------------------------------------------------------- la boite simulee

/// La boite courriel de la compagnie fictive : elle garde, elle n'envoie pas. Meme surface que `Client`. Chaque message
/// est ecrit tel quel dans `courriels-simules/`, pour qu'on puisse l'ouvrir et voir ce qui serait parti.
pub struct BoiteSimulee {
    racine: PathBuf,
    compte: String,
}

impl BoiteSimulee {
    /// `entreprise` : la fiche de la compagnie (le courriel du signataire est l'adresse de la boite ; sans lui, elle
    /// refuse en le nommant).
    pub fn nouvelle(racine: &Path, entreprise: &droit::fiche::ChampsEntreprise) -> R<BoiteSimulee> {
        let adresse = entreprise.signataire.exigee("signataire")?.email.ecrite("signataire.email")?;
        Ok(BoiteSimulee { racine: racine.to_path_buf(), compte: adresse.to_lowercase() })
    }

    pub fn du_contexte(ctx: &Contexte) -> R<BoiteSimulee> {
        BoiteSimulee::nouvelle(&ctx.racine, &ctx.entreprise)
    }
}

/// Le `Message-ID` d'un message brut (l'en-tête, sans égard à la casse, en début de ligne).
fn message_id_de(brut: &[u8]) -> Option<String> {
    let blanc = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c);
    let mut debut = 0;
    loop {
        let reste = &brut[debut..];
        if reste.len() >= 11 && reste[..11].eq_ignore_ascii_case(b"message-id:") {
            let mut i = 11;
            while i < reste.len() && blanc(reste[i]) {
                i += 1;
            }
            if reste.get(i) == Some(&b'<') {
                let fin = reste[i + 1..].iter().position(|c| matches!(c, b'>' | b'\r' | b'\n'));
                if let Some(f) = fin.filter(|f| *f > 0 && reste[i + 1 + f] == b'>') {
                    return Some(String::from_utf8_lossy(&reste[i..i + f + 2]).into_owned());
                }
            }
        }
        match brut[debut..].iter().position(|c| *c == b'\n') {
            Some(p) => debut += p + 1,
            None => return None,
        }
    }
}

/// `octets` octets des aleas du systeme, en hexadecimal ; un systeme qui n'en donne pas refuse.
pub fn hexa_au_hasard(octets: usize) -> R<String> {
    let mut o = vec![0u8; octets];
    socle::temps::aleas(&mut o).map_err(|e| e.reformuler(|p| format!("Le systeme ne donne pas d'aleas : {}.", p)).reclasser(Classe::Courriel))?;
    Ok(o.iter().map(|c| format!("{:02x}", c)).collect())
}

/// `%Y%m%d-%H%M%S-%f` de l'heure de Paris : l'ordre des noms est celui des envois.
fn horodatage_de(t: socle::paris::Moment) -> String {
    // Les microsecondes entieres de la fraction de seconde.
    let micros = t.nanosecondes() / 1_000;
    format!("{}-{}-{:06}", t.jour_de_paris().aaaammjj(), socle::paris::ecrit::heure_minute_seconde(t, ""), micros)
}

impl Courriel for BoiteSimulee {
    fn compte(&self) -> &str {
        &self.compte
    }

    fn racine(&self) -> Option<&Path> {
        Some(&self.racine)
    }

    /// Ecrit le message dans `courriels-simules/` ; l'identifiant est son `Message-ID`, comme chez le vrai client :
    /// c'est par lui que la compagnie fictive retrouve ce qui est parti.
    fn envoyer(&mut self, message_mime: &[u8]) -> R<crate::envoi::IdDeCourriel> {
        let dossier = self.racine.join(DOSSIER_COURRIELS);
        std::fs::create_dir_all(&dossier).map_err(|e| socle::echec::du_disque(&dossier, &e).reclasser(Classe::Courriel))?;
        // Un message sans `Message-ID` en recoit un, comme chez le vrai client.
        let ident = match message_id_de(message_mime) {
            Some(i) => i,
            None => format!("<{}@fictive.puppetpaie>", hexa_au_hasard(8)?),
        };
        let ident = crate::envoi::IdDeCourriel::lire(&ident).map_err(|e| e.reclasser(Classe::Courriel))?;
        let nom = format!("{}_{}.eml", horodatage_de(socle::temps::secondes()), hexa_au_hasard(4)?);
        let chemin = dossier.join(&nom);
        std::fs::write(&chemin, message_mime).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::Courriel))?;
        Ok(ident)
    }

    fn rechercher(&mut self, _requete: &str, _maximum: usize) -> R<Vec<String>> {
        Ok(Vec::new())
    }

    /// La boite simulee ne recoit rien : sa recherche ne trouve aucun message a lire.
    fn message_brut(&mut self, identifiant: &str) -> R<Vec<u8>> {
        Err(erreur(format!("Message {} introuvable.", identifiant)))
    }

    fn fermer(&mut self) -> R<()> {
        Ok(())
    }
}
