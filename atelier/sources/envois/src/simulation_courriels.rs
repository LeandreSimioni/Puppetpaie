//! `courriels-fictifs` : les courriels joues de la compagnie fictive
//! (`courriels-simules/*.eml`), tels que le salarie les recoit -- et, s'ils portent un bouton, la reponse toute
//! ecrite que ce bouton prepare. `dernier` : le repere a redonner la fois suivante.
//!
//! Lecture MIME par `mailparse` : un en-tete `From`/`To` est rendu tel que `mailparse` le decode, sans guillemets
//! autour d'un nom a point ou virgule ; les pieces jointes sont les feuilles du message qui ne sont ni le texte ni le HTML
//! du corps (`iter_attachments`).

use std::collections::HashMap;
use std::path::Path;

use mailparse::{parse_mail, MailHeaderMap, ParsedMail};

use droit::contexte::Contexte;
use socle::echec::{Echec, R};
use crate::courriel::{BoiteSimulee, Courriel, ErreurCourriel, DOSSIER_COURRIELS};

/// Le parametre MIME qui nomme une piece jointe (`Content-Disposition`), puis celui de son type (`Content-Type`).
const PARAMETRE_FICHIER: &str = "filename";
const PARAMETRE_NOM: &str = "name";

/// Le disque qui refuse un courriel joue.
fn du_disque(chemin: &Path, e: &std::io::Error) -> Echec {
    Echec::from(socle::echec::du_disque(chemin, e))
}

/// Un courriel joue que `mailparse` ne lit pas : il a ete ecrit par la boite simulee, c'est un defaut du logiciel.
fn illisible(e: mailparse::MailParseError) -> Echec {
    Echec::from(socle::echec::avec("Un courriel joué ne se lit pas.", socle::echec::APart { detail: Some(e.to_string()), ..socle::echec::APart::default() }))
}

/// Les courriels joues, du plus ancien au plus recent : (nom, octets). Aucun encore : le dossier n'existe pas.
pub fn courriels(racine: &Path) -> R<Vec<(String, Vec<u8>)>> {
    let dossier = racine.join(DOSSIER_COURRIELS);
    let lecture = match std::fs::read_dir(&dossier) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(du_disque(&dossier, &e)),
    };
    let mut noms = Vec::new();
    for e in lecture {
        let n = e.map_err(|x| du_disque(&dossier, &x))?.file_name().to_string_lossy().into_owned();
        if n.ends_with(".eml") {
            noms.push(n);
        }
    }
    noms.sort();
    noms.into_iter()
        .map(|n| {
            let chemin = dossier.join(&n);
            let o = std::fs::read(&chemin).map_err(|e| du_disque(&chemin, &e))?;
            Ok((n, o))
        })
        .collect()
}

pub fn nom_de_fichier(p: &ParsedMail) -> Option<String> {
    let d = p.get_content_disposition();
    d.params.get(PARAMETRE_FICHIER).cloned().or_else(|| p.ctype.params.get(PARAMETRE_NOM).cloned())
}

pub fn feuilles<'a>(p: &'a ParsedMail<'a>, sortie: &mut Vec<&'a ParsedMail<'a>>) {
    if p.subparts.is_empty() {
        sortie.push(p);
    } else {
        for s in &p.subparts {
            feuilles(s, sortie);
        }
    }
}

/// `binascii.a2b_qp` : les retours a la ligne restent ceux du fichier (`mailparse` les rend tous en CRLF).
fn decoder_qp(o: &[u8]) -> Vec<u8> {
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut b = Vec::with_capacity(o.len());
    let mut i = 0;
    while i < o.len() {
        if o[i] != b'=' {
            b.push(o[i]);
            i += 1;
        } else if o.get(i + 1) == Some(&b'\n') {
            i += 2;
        } else if o.get(i + 1) == Some(&b'\r') && o.get(i + 2) == Some(&b'\n') {
            i += 3;
        } else if let (Some(h), Some(l)) = (o.get(i + 1).and_then(|c| hex(*c)), o.get(i + 2).and_then(|c| hex(*c))) {
            b.push(h * 16 + l);
            i += 3;
        } else {
            b.push(b'=');
            i += 1;
        }
    }
    b
}

/// Le texte qu'un corps donne. `mailparse` ne refuse qu'un base64 mal forme (que `texte_de` decode lui-meme) : son
/// quoted-printable se lit en mode robuste, et le jeu de caracteres decode toujours (`ascii` pour un jeu inconnu).
fn en_texte(texte: Result<String, mailparse::MailParseError>) -> String {
    match texte {
        Ok(t) => t,
        Err(e) => unreachable!("un corps de courriel hors base64 se lit toujours : {}", e),
    }
}

/// Le corps décodé d'une feuille texte, sans toucher aux fins de ligne.
pub fn texte_de(f: &ParsedMail) -> String {
    use mailparse::body::Body;
    let charset = f.ctype.charset.to_lowercase();
    if let Body::Base64(b) = f.get_body_encoded() {
        // Un corps base64 se decode avec indulgence, et n'est JAMAIS vide faute de base64 valide (le
        // texte brut, lignes jointes, si la longueur est impossible) : `mailparse` rendrait une erreur. Le texte se tire
        // d'un message fait ici, son jeu de caracteres sans rien qui romprait l'en-tete.
        let octets = decoder_base64_indulgent(b.get_raw());
        let jeu: String = f.ctype.charset.chars().filter(|c| !c.is_control() && *c != '"' && *c != '\\').collect();
        let synthetique = [format!("Content-Type: text/plain; charset=\"{}\"\r\nContent-Transfer-Encoding: binary\r\n\r\n", jeu).into_bytes(), octets].concat();
        return en_texte(parse_mail(&synthetique).and_then(|m| m.get_body()));
    }
    if matches!(charset.as_str(), "" | "us-ascii" | "utf-8" | "utf8") {
        if let Body::QuotedPrintable(b) = f.get_body_encoded() {
            return String::from_utf8_lossy(&decoder_qp(b.get_raw())).into_owned();
        }
    }
    let t = en_texte(f.get_body());
    match f.get_body_encoded() {
        // `mailparse` rend CRLF les fins de ligne d'un corps quoted-printable ; on garde celles du fichier.
        Body::QuotedPrintable(b) if !b.get_raw().contains(&b'\r') => t.replace("\r\n", "\n"),
        _ => t,
    }
}

const ALPHABET_B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Le base64 lu octet par octet : `strict`, tout signe hors alphabet et toute donnee apres le bourrage sont refuses ; sinon les
/// signes hors alphabet sont ignores. `None` : la longueur ou le signe ne se lit pas.
fn base64_vers_octets(donnees: &[u8], strict: bool) -> Option<Vec<u8>> {
    let valeur = |c: u8| ALPHABET_B64.iter().position(|x| *x == c).map(|v| v as u32);
    let (mut sortie, mut tampon, mut bits, mut quad, mut pads) = (Vec::new(), 0u32, 0u32, 0usize, 0usize);
    let mut fini = false;
    for (i, &c) in donnees.iter().enumerate() {
        if c == b'=' {
            if quad >= 2 && quad + pads + 1 >= 4 {
                if strict && i + 1 < donnees.len() {
                    return None; // des donnees apres le bourrage
                }
                fini = true;
                break;
            }
            pads += 1;
            continue;
        }
        let Some(v) = valeur(c) else {
            if strict {
                return None;
            }
            continue;
        };
        pads = 0;
        tampon = (tampon << 6) | v;
        bits += 6;
        quad = (quad + 1) % 4;
        if bits >= 8 {
            bits -= 8;
            sortie.push((tampon >> bits) as u8);
            tampon &= (1 << bits) - 1;
        }
    }
    if !fini && quad != 0 {
        return None;
    }
    Some(sortie)
}

/// Un corps de courriel en base64, lu avec indulgence (les corps recus ne sont pas toujours bien formes) : d'abord strict avec le
/// bourrage complete, puis sans les signes hors alphabet, puis avec `==` ; si la longueur est impossible, le texte lui-meme.
fn decoder_base64_indulgent(brut: &[u8]) -> Vec<u8> {
    let encode: Vec<u8> = brut.iter().copied().filter(|c| *c != b'\n' && *c != b'\r').collect();
    let reste = encode.len() % 4;
    let mut complete = encode.clone();
    if reste != 0 {
        complete.extend_from_slice(&b"==="[..4 - reste]);
    }
    if let Some(o) = base64_vers_octets(&complete, true) {
        return o;
    }
    if let Some(o) = base64_vers_octets(&encode, false) {
        return o;
    }
    let mut deux = encode.clone();
    deux.extend_from_slice(b"==");
    if let Some(o) = base64_vers_octets(&deux, false) {
        return o;
    }
    encode
}

/// Le texte de la premiere feuille `text/<sous>` sans nom de fichier, s'il y en a une.
fn corps(feuilles: &[&ParsedMail], sous: &str) -> Option<String> {
    feuilles.iter().find(|f| f.ctype.mimetype == format!("text/{}", sous) && nom_de_fichier(f).is_none()).map(|f| texte_de(f))
}

/// Un nombre ecrit en `base` (des chiffres seuls) ; `None` s'il n'en est pas un, ou ne tient pas dans un `u32`.
fn nombre(chiffres: &str, base: u32) -> Option<u32> {
    if chiffres.is_empty() {
        return None;
    }
    chiffres.chars().try_fold(0u32, |n, x| n.checked_mul(base)?.checked_add(x.to_digit(base)?))
}

/// `html.unescape`, pour ce qu'un lien `mailto:` porte.
fn deseschapper(t: &str) -> String {
    let mut out = String::new();
    let mut reste = t;
    while let Some(i) = reste.find('&') {
        out.push_str(&reste[..i]);
        let suite = &reste[i..];
        let fin = suite.find(';');
        let rempl = fin.and_then(|f| {
            let e = &suite[1..f];
            let c = match e {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => e
                    .strip_prefix("#x")
                    .or_else(|| e.strip_prefix("#X"))
                    .and_then(|h| nombre(h, 16))
                    .or_else(|| e.strip_prefix('#').and_then(|d| nombre(d, 10)))
                    .and_then(char::from_u32),
            }?;
            Some((c, f))
        });
        match rempl {
            Some((c, f)) => {
                out.push(c);
                reste = &suite[f + 1..];
            }
            None => {
                out.push('&');
                reste = &suite[1..];
            }
        }
    }
    out.push_str(reste);
    out
}

/// Le texte d'une URL décodé : `+` en espace, `%XX` en octet.
fn decoder(t: &str) -> String {
    let o = t.as_bytes();
    let mut b: Vec<u8> = Vec::with_capacity(o.len());
    let mut i = 0;
    while i < o.len() {
        match o[i] {
            b'+' => b.push(b' '),
            b'%' if i + 2 < o.len() => {
                let hexa = |c: u8| (c as char).to_digit(16);
                match (hexa(o[i + 1]), hexa(o[i + 2])) {
                    (Some(h), Some(l)) => {
                        b.push((h * 16 + l) as u8);
                        i += 2;
                    }
                    // Un `%` que deux chiffres hexadecimaux ne suivent pas reste tel quel.
                    _ => b.push(b'%'),
                }
            }
            c => b.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&b).into_owned()
}

/// (objet, corps) du lien `mailto:` du premier HTML qui en porte un. Un lien qui ne dit pas l'un et l'autre ne prepare
/// pas de reponse : ce n'est pas un bouton.
pub fn bouton(feuilles: &[&ParsedMail]) -> Option<(String, String)> {
    let re = regex::Regex::new(r#"href="(mailto:[^"]+)""#).expect("motif du lien mailto");
    for f in feuilles {
        if f.ctype.mimetype != "text/html" {
            continue;
        }
        let html = texte_de(f);
        if let Some(c) = re.captures(&html) {
            let lien = deseschapper(&c[1]);
            let Some((_, requete)) = lien.split_once('?') else { return None };
            let requete = match requete.split_once('#') {
                Some((avant, _)) => avant,
                None => requete,
            };
            let (mut sujet, mut texte) = (None::<String>, None::<String>);
            for paire in requete.split('&') {
                let Some((k, v)) = paire.split_once('=') else { continue };
                if v.is_empty() {
                    continue;
                }
                match decoder(k).as_str() {
                    "subject" if sujet.is_none() => sujet = Some(decoder(v)),
                    "body" if texte.is_none() => texte = Some(decoder(v)),
                    _ => {}
                }
            }
            return sujet.zip(texte);
        }
    }
    None
}

/// Le nombre de pages d'une piece PDF ; `None` pour une autre piece. Un PDF joue qui ne se lit pas refuse : la boite
/// simulee l'a ecrit, c'est un defaut du logiciel.
fn pages(f: &ParsedMail) -> R<Option<usize>> {
    if f.ctype.mimetype != "application/pdf" {
        return Ok(None);
    }
    let octets = f.get_body_raw().map_err(illisible)?;
    let pdf = fabrique_pdf::lecture::charger(&octets).map_err(|e| Echec::from(socle::echec::avec("Un PDF joint à un courriel joué ne se lit pas.", socle::echec::APart { detail: Some(e.to_string()), ..socle::echec::APart::default() })))?;
    Ok(Some(pdf.get_pages().len()))
}

/// Les adresses d'un champ, un nom a point, virgule ou arobase entre guillemets ; l'en-tete tel quel s'il ne se lit pas
/// en adresses. Un courriel joue dit toujours qui l'envoie et a qui.
fn adresses(m: &ParsedMail, champ: &str) -> R<String> {
    let (Some(brut), Some(h)) = (m.headers.get_first_value(champ), m.headers.get_first_header(champ)) else {
        return Err(Echec::from(socle::echec::cle("Un courriel joué ne dit pas qui l'envoie ou à qui.", champ)));
    };
    let liste = match mailparse::addrparse_header(h) {
        Ok(l) => l,
        Err(pas_des_adresses) => {
            drop(pas_des_adresses);
            return Ok(brut);
        }
    };
    let mut vus = vec![];
    for a in liste.iter() {
        match a {
            mailparse::MailAddr::Single(i) => match i.display_name.as_deref().filter(|n| !n.is_empty()) {
                Some(n) => {
                    let n = if n.chars().any(|c| "()<>@,:;.\\\"[]".contains(c)) {
                        format!("\"{}\"", n.replace('\\', "\\\\").replace('"', "\\\""))
                    } else {
                        n.to_string()
                    };
                    vus.push(format!("{} <{}>", n, i.addr));
                }
                None => vus.push(i.addr.clone()),
            },
            mailparse::MailAddr::Group(_) => return Ok(brut),
        }
    }
    Ok(vus.join(", "))
}

/// Une piece jointe d'un courriel joue : son nom et, pour un PDF, son nombre de pages.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PieceJouee {
    pub nom: Option<String>,
    pub pages: Option<usize>,
}

/// Le bouton d'un courriel joue : la reponse toute ecrite qu'il ouvre (son sujet, son corps).
#[derive(Debug, Clone, serde::Serialize)]
pub struct BoutonJoue {
    pub sujet: String,
    pub corps: String,
}

/// Un courriel joue, tel que la page le montre : son fichier, de qui, a qui, son sujet, ses corps texte et HTML, ses
/// pieces, s'il renvoie un contrat contresigne, et son bouton.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CourrielJoue {
    pub nom: String,
    pub de: String,
    pub a: String,
    pub sujet: Option<String>,
    pub texte: Option<String>,
    pub html: Option<String>,
    pub pieces: Vec<PieceJouee>,
    pub contresigne: bool,
    pub bouton: Option<BoutonJoue>,
}

/// Les courriels joues, et le dernier fichier lu (d'ou la page repart).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CourrielsJoues {
    pub courriels: Vec<CourrielJoue>,
    /// Le dernier courriel joue ; aucun encore : vide.
    pub dernier: Option<String>,
}

/// Les courriels joues de la compagnie fictive ecrits apres `apres` (un nom de fichier ; "" : tous), tels que la page
/// les montre.
pub fn courriels_joues(racine: &Path, apres: Option<&str>) -> R<CourrielsJoues> {
    let (mut sortie, mut dernier) = (vec![], None);
    for (nom, octets) in courriels(racine)? {
        dernier = Some(nom.clone());
        if apres.is_some_and(|a| nom.as_str() <= a) {
            continue;
        }
        let m = parse_mail(&octets).map_err(illisible)?;
        let mut fs = vec![];
        feuilles(&m, &mut fs);
        let b = bouton(&fs);
        let est_multipart = m.ctype.mimetype.starts_with("multipart/") && m.ctype.mimetype != "multipart/alternative";
        let mut pieces: Vec<PieceJouee> = Vec::new();
        for f in fs.iter().filter(|f| est_multipart && !(f.ctype.mimetype.starts_with("text/") && nom_de_fichier(f).is_none())) {
            pieces.push(PieceJouee { nom: nom_de_fichier(f), pages: pages(f)? });
        }
        // Le courriel qui renvoie au signataire son contrat contresigne : sa piece en est l'exemplaire.
        let contresigne = pieces.iter().any(|p| p.nom.as_deref().is_some_and(|n| n.ends_with(crate::releve_signatures::SUFFIXE_CONTRESIGNE)));
        sortie.push(CourrielJoue {
            nom,
            de: adresses(&m, "From")?,
            a: adresses(&m, "To")?,
            sujet: m.headers.get_first_value("Subject"),
            texte: corps(&fs, "plain"),
            html: corps(&fs, "html"),
            pieces,
            contresigne,
            bouton: b.map(|(sujet, corps)| BoutonJoue { sujet, corps }),
        });
    }
    Ok(CourrielsJoues { courriels: sortie, dernier })
}


// ------------------------------------------------------------------ la boite qui repond

/// Les courriels joues, par leur `Message-ID` : le
/// dernier d'un meme identifiant l'emporte.
pub fn courriels_par_identifiant(racine: &Path) -> R<HashMap<String, Vec<u8>>> {
    let mut envoyes = HashMap::new();
    for (_, octets) in courriels(racine)? {
        let (entetes, _) = mailparse::parse_headers(&octets).map_err(illisible)?;
        // Un courriel joue sans identifiant ne repond a aucune demande.
        if let Some(id) = crate::courriel_lecture::identifiant_des_entetes(&entetes) {
            envoyes.insert(id, octets);
        }
    }
    Ok(envoyes)
}

/// `_bouton_mailto(demande)` pour la demande dont on connait les identifiants (`a["envoyes"]`) : (objet, corps) du lien
/// `mailto:` de son courriel joue, s'il y en a un.
pub fn bouton_de_la_demande(envoyes: &HashMap<String, Vec<u8>>, identifiants: &[String]) -> R<Option<(String, String)>> {
    let Some(octets) = identifiants.iter().find_map(|i| envoyes.get(i)) else { return Ok(None) };
    let m = parse_mail(octets).map_err(illisible)?;
    let mut f = Vec::new();
    feuilles(&m, &mut f);
    Ok(bouton(&f))
}

/// Le courriel que la personne fictive renvoie a la boite (de `de` a `a`), date de maintenant,
/// identifiant `<ident@fictive.puppetpaie>`.
pub fn reponse_jouee(de: &str, a: &str, sujet: &str, corps: &str, ident: &str) -> Result<Vec<u8>, String> {
    // Dans une compagnie fictive, une adresse de salarie AVEC NOM D'AFFICHAGE (`Nom <adresse>`) est
    // acceptee. `envoi::en_mime_avec` n'ecrit que des adresses simples : le message se fabrique avec l'adresse nue,
    // puis l'en-tete `From` est remis avec son nom d'affichage.
    let (nu, rendu) = adresse_d_affichage(de);
    let message = crate::envoi::Message {
        cle: ident.to_string(),
        nom: nu.clone(),
        destinataire: a.to_string(),
        sujet: sujet.to_string(),
        corps: corps.to_string(),
        ..Default::default()
    };
    let t = socle::temps::secondes().floor() as i64;
    let mut frontiere = || Ok(String::new());
    let octets = crate::envoi::en_mime_avec(
        &message,
        &nu,
        crate::envoi::Hasards { date: crate::envoi::date_de(t)?, message_id: format!("<{}@fictive.puppetpaie>", ident), frontiere: &mut frontiere },
    )?;
    let entete_nu = format!("From: {}", nu);
    match rendu {
        Some(rendu) if octets.starts_with(entete_nu.as_bytes()) => {
            let mut sortie = format!("From: {}", rendu).into_bytes();
            sortie.extend_from_slice(&octets[entete_nu.len()..]);
            Ok(sortie)
        }
        _ => Ok(octets),
    }
}

/// `Nom <adresse>` -> (l'adresse nue, l'en-tete `From` avec son nom d'affichage) ; une adresse simple -> (elle-meme, `None`). Le
/// nom d'affichage ASCII s'ecrit tel quel ; un nom qui ne l'est pas, en mot encode `=?utf-8?b?...?=` (court : pas de choix
/// entre `q` et `b`, pas de pliage des longs noms).
fn adresse_d_affichage(de: &str) -> (String, Option<String>) {
    let de = de.trim();
    let (Some(debut), true) = (de.rfind('<'), de.ends_with('>')) else { return (de.to_string(), None) };
    let nu = de[debut + 1..de.len() - 1].trim().to_string();
    let nom = de[..debut].trim();
    if nom.is_empty() {
        return (nu.clone(), Some(format!("<{}>", nu)));
    }
    let nom_ecrit = if nom.is_ascii() { nom.to_string() } else { format!("=?utf-8?b?{}?=", crate::envoi::base64_brut(nom.as_bytes())) };
    (nu.clone(), Some(format!("{} <{}>", nom_ecrit, nu)))
}

/// La boite de la compagnie fictive, vue par une releve. Chaque code attendu y trouve sa
/// reponse, fabriquee d'avance. Ce qu'on y envoie (le contrat contresigne) s'ecrit comme tout courriel joue.
pub struct BoiteQuiRepond {
    boite: BoiteSimulee,
    reponses: HashMap<String, Vec<u8>>,
}

impl BoiteQuiRepond {
    pub fn nouvelle(ctx: &Contexte) -> R<BoiteQuiRepond> {
        Ok(BoiteQuiRepond { boite: BoiteSimulee::du_contexte(ctx)?, reponses: HashMap::new() })
    }

    /// Fabrique d'avance la reponse a ce code.
    pub fn repondre(&mut self, code: &str, octets: Vec<u8>) {
        self.reponses.insert(code.to_string(), octets);
    }
}

impl Courriel for BoiteQuiRepond {
    fn compte(&self) -> &str {
        Courriel::compte(&self.boite)
    }

    fn racine(&self) -> Option<&Path> {
        Courriel::racine(&self.boite)
    }

    fn envoyer(&mut self, message_mime: &[u8]) -> Result<String, ErreurCourriel> {
        self.boite.envoyer(message_mime)
    }

    /// Le code cherche, s'il a sa reponse : `[code]`.
    fn rechercher(&mut self, requete: &str, _maximum: usize) -> Result<Vec<String>, ErreurCourriel> {
        let code = requete.trim_matches('"');
        Ok(if self.reponses.contains_key(code) { vec![code.to_string()] } else { Vec::new() })
    }

    fn message_brut(&mut self, identifiant: &str) -> Result<Vec<u8>, ErreurCourriel> {
        self.reponses.get(identifiant).cloned().ok_or_else(|| ErreurCourriel(format!("Message {} introuvable.", identifiant)))
    }

    fn fermer(&mut self) {}
}
