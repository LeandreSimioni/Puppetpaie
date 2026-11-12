//! Le chiffrement d'un PDF : `fabrique-pdf --verrouiller`, et la lecture d'un PDF ainsi verrouillé.
//!
//! C'est le chiffrement standard de PDF 2.0 (ISO 32000-2) : `/V 5`, `/R 6`, AES-256 en mode CBC
//! (`/CFM /AESV3`), clé de fichier de 32 octets tirée au hasard. Le mot de passe utilisateur est
//! vide (le fichier s'ouvre partout sans mot de passe) ; le mot de passe propriétaire est tiré au
//! hasard et jeté : personne ne peut plus lever les restrictions. Les permissions sont :
//! impression, copie et accessibilité, rien d'autre.
//!
//! Chaînes et flux sont chiffrés un à un (IV de 16 octets tiré au hasard, en tête ; bourrage
//! PKCS#7), à l'exception du dictionnaire `/Encrypt` et des flux de table de références.
//!
//! Le déchiffrement (`dechiffrer`) lit ce même chiffrement avec un mot de passe utilisateur donné
//! (vide, ici) ; pour les anciens chiffrements (RC4, `/V` 1 ou 2) il s'en remet à lopdf.

use aes::cipher::block_padding::{NoPadding, Pkcs7};
use aes::cipher::{BlockDecryptMut, BlockEncrypt, BlockEncryptMut, KeyInit, KeyIvInit};
use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};
use sha2::{Digest, Sha256, Sha384, Sha512};

use crate::description::{illisible, Refus};
use crate::lecture::{self, directe};

type CbcChiffre256 = cbc::Encryptor<aes::Aes256>;
type CbcDechiffre256 = cbc::Decryptor<aes::Aes256>;
type CbcChiffre128 = cbc::Encryptor<aes::Aes128>;

/// Les permissions du dictionnaire de chiffrement (`/P`) :
/// bits 3 (imprimer), 5 (copier) et 10 (accessibilité), les bits réservés à 1 (7, 8 et 13 à 32).
/// Vaut -3372 en entier signé.
pub const PERMISSIONS: u32 = 0xFFFF_F0C0 | 4 | 16 | 512;

/// Les métadonnées d'un fichier chiffré qui ne dit pas `/EncryptMetadata` le sont aussi : « Default value: true »
/// (ISO 32000-1, table 21).
const METADONNEES_CHIFFREES: bool = true;

/// ISO 32000-1, table 20 : `StmF` et `StrF`, « Default value: Identity » ; table 26 : `Identity`, « Input data shall be
/// passed through without any processing ».
const FILTRE_PAR_DEFAUT: &[u8] = b"Identity";

/// D'ou viennent les tirages d'un verrou (`verrouiller_avec`) ; `None` : le systeme. Un tirage impossible est un
/// defaut (`Refus::defaut`).
type Tirage = fn(&mut [u8]) -> Result<(), Refus>;

thread_local! {
    static TIRAGE: std::cell::Cell<Option<Tirage>> = const { std::cell::Cell::new(None) };
}

fn hasard(n: usize) -> Result<Vec<u8>, Refus> {
    let mut v = vec![0u8; n];
    match TIRAGE.with(|t| t.get()) {
        Some(tirer) => tirer(&mut v).map_err(|e| e.reformuler(|p| format!("tirage au hasard impossible ({})", p)))?,
        None => getrandom::getrandom(&mut v).map_err(|e| Refus::defaut(format!("tirage au hasard impossible ({})", e)))?,
    }
    Ok(v)
}

/// `verrouiller`, ses tirages pris a `tirer` : ceux du systeme, ou la suite fixe d'un rejeu (`socle::temps::aleas`),
/// pour que deux rejeux ecrivent les memes octets. Le mot de passe du proprietaire est jete : il ne protege que contre
/// la modification du fichier, que le salarie peut lire sans mot de passe.
pub fn verrouiller_avec(octets: &[u8], tirer: Tirage) -> Result<Vec<u8>, Refus> {
    TIRAGE.with(|t| t.set(Some(tirer)));
    let r = verrouiller(octets);
    TIRAGE.with(|t| t.set(None));
    r
}

// ---------------------------------------------------------------------------------------------
// Les empreintes de mot de passe.
// ---------------------------------------------------------------------------------------------

/// L'algorithme 2.B de la norme ISO 32000-2 (révision 6) : SHA-256, puis au moins 64 tours de
/// AES-128-CBC dont la somme des 16 premiers octets (modulo 3) choisit SHA-256, 384 ou 512.
fn empreinte_r6(mot_de_passe: &[u8], sel: &[u8], u: &[u8]) -> Vec<u8> {
    let mut k: Vec<u8> = Sha256::new()
        .chain_update(mot_de_passe)
        .chain_update(sel)
        .chain_update(u)
        .finalize()
        .to_vec();
    let mut tour = 0usize;
    loop {
        let mut k1 = Vec::with_capacity(64 * (mot_de_passe.len() + k.len() + u.len()));
        for _ in 0..64 {
            k1.extend_from_slice(mot_de_passe);
            k1.extend_from_slice(&k);
            k1.extend_from_slice(u);
        }
        // k1 est de longueur multiple de 16 (64 répétitions) : pas de bourrage.
        let e = CbcChiffre128::new_from_slices(&k[..16], &k[16..32])
            .expect("clé et IV de 16 octets")
            .encrypt_padded_vec_mut::<NoPadding>(&k1);
        let reste: u32 = e[..16].iter().map(|b| *b as u32).sum::<u32>() % 3;
        k = match reste {
            0 => Sha256::digest(&e).to_vec(),
            1 => Sha384::digest(&e).to_vec(),
            2.. => Sha512::digest(&e).to_vec(),
        };
        tour += 1;
        // `e` a la longueur de `k1`, 64 répétitions d'au moins 32 octets : il a un dernier octet.
        if tour >= 64 && (e[e.len() - 1] as usize) <= tour - 32 {
            break;
        }
    }
    k.truncate(32);
    k
}

/// L'empreinte de la révision `r` (5 : un simple SHA-256, ancienne extension d'Adobe ; 6 : 2.B).
fn empreinte(r: i64, mot_de_passe: &[u8], sel: &[u8], u: &[u8]) -> Vec<u8> {
    if r >= 6 {
        empreinte_r6(mot_de_passe, sel, u)
    } else {
        Sha256::new().chain_update(mot_de_passe).chain_update(sel).chain_update(u).finalize().to_vec()
    }
}

// ---------------------------------------------------------------------------------------------
// Les chiffrements de données.
// ---------------------------------------------------------------------------------------------

/// AES-256-CBC, IV tiré au hasard et placé en tête, bourrage PKCS#7.
fn chiffrer_donnees(cle: &[u8], donnees: &[u8]) -> Result<Vec<u8>, Refus> {
    let iv = hasard(16)?;
    let mut sortie = iv.clone();
    sortie.extend(
        CbcChiffre256::new_from_slices(cle, &iv)
            .map_err(|e| Refus::defaut(format!("clé de chiffrement invalide ({})", e)))?
            .encrypt_padded_vec_mut::<Pkcs7>(donnees),
    );
    Ok(sortie)
}

/// L'inverse (ISO 32000-1, 7.6.2 : « the initialization vector is a 16-byte random number that is stored as the first
/// 16 bytes of the encrypted stream or string » ; « Strings and streams encrypted with AES shall use a padding
/// scheme that is described in Internet RFC 2898, PKCS #5 »). Des données qui n'ont pas cette forme ne sont pas
/// chiffrées comme le fichier le dit : la pièce se refuse.
fn dechiffrer_donnees(cle: &[u8], donnees: &[u8]) -> Result<Vec<u8>, Refus> {
    if donnees.len() < 32 || donnees.len() % 16 != 0 {
        return illisible(format!("une donnée chiffrée de {} octets (AES : l'IV, puis des blocs de 16)", donnees.len()));
    }
    CbcDechiffre256::new_from_slices(cle, &donnees[..16])
        .map_err(|e| Refus::defaut(format!("clé de chiffrement invalide ({})", e)))?
        .decrypt_padded_vec_mut::<Pkcs7>(&donnees[16..])
        .map_err(|e| Refus::illisible(format!("une donnée chiffrée ne se déchiffre pas ({})", e)))
}

/// Applique `f` à chaque chaîne de l'objet, en profondeur, et aux flux : `fl` à leur contenu.
fn parcourir(
    o: &mut Object,
    chaine: &mut dyn FnMut(&[u8]) -> Result<Vec<u8>, Refus>,
    flux: &mut dyn FnMut(&[u8]) -> Result<Vec<u8>, Refus>,
) -> Result<(), Refus> {
    match o {
        // Le format (littéral ou hexadécimal) reste celui de la chaîne : l'écriture de lopdf
        // échappe ce qu'il faut dans une chaîne littérale, quels que soient les octets.
        Object::String(s, _) => *s = chaine(s)?,
        Object::Array(v) => {
            for x in v {
                parcourir(x, chaine, flux)?;
            }
        }
        Object::Dictionary(d) => {
            for (_, x) in d.iter_mut() {
                parcourir(x, chaine, flux)?;
            }
        }
        Object::Stream(s) => {
            for (_, x) in s.dict.iter_mut() {
                parcourir(x, chaine, flux)?;
            }
            let neuf = flux(&s.content)?;
            s.set_content(neuf);
        }
        // Ni chaîne ni flux à chiffrer, en eux ni sous eux.
        Object::Null | Object::Boolean(_) | Object::Integer(_) | Object::Real(_) | Object::Name(_) | Object::Reference(_) => {}
    }
    Ok(())
}

/// Le dictionnaire d'un objet (le sien, ou celui de son flux) ; aucun pour un autre objet.
fn dictionnaire_de(o: &Object) -> Option<&Dictionary> {
    match o {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(&s.dict),
        Object::Null | Object::Boolean(_) | Object::Integer(_) | Object::Real(_) | Object::Name(_) | Object::String(..) | Object::Array(_) | Object::Reference(_) => None,
    }
}

/// Les objets de structure, lus directs (avant le déchiffrement) : les flux d'objets et de références (ISO 32000-1,
/// 7.5.7 et 7.5.8, `/Type /ObjStm`, `/Type /XRef`) et le dictionnaire de linéarisation (annexe F, la clé `Linearized`).
fn est_de_structure(o: &Object) -> bool {
    dictionnaire_de(o).is_some_and(|d| {
        matches!(directe(d, b"Type"), Some(Object::Name(n)) if n == b"XRef" || n == b"ObjStm") || directe(d, b"Linearized").is_some()
    })
}

// ---------------------------------------------------------------------------------------------
// Verrouiller.
// ---------------------------------------------------------------------------------------------

/// Le PDF `octets`, chiffré en AES-256 (PDF 2.0, `/V 5 /R 6`), mot de passe utilisateur vide.
pub fn verrouiller(octets: &[u8]) -> Result<Vec<u8>, Refus> {
    let mut doc = crate::lecture::charger(octets)?;
    // Un fichier déjà chiffré (mot de passe vide) est d'abord ouvert : on le verrouille de neuf.
    dechiffrer(&mut doc, b"")?;
    if doc.get_pages().is_empty() {
        return illisible("PDF sans page");
    }
    // Les flux de structure (objets compressés, table de références) sont réécrits par lopdf, et
    // leurs objets ont déjà été lus : on ne les garde pas.
    doc.objects.retain(|_, o| !est_de_structure(o));
    doc.compress();
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    doc.version = "2.0".to_string();
    doc.trailer.remove(b"Encrypt");

    // La clé de fichier, et tout ce qu'il faut pour que l'utilisateur (mot de passe vide) et le
    // propriétaire (mot de passe jeté) la retrouvent.
    let cle = hasard(32)?;
    let utilisateur: &[u8] = b"";
    let proprietaire = hasard(32)?;

    let sel_u = hasard(16)?;
    let (sel_valid_u, sel_cle_u) = sel_u.split_at(8);
    let mut u = empreinte_r6(utilisateur, sel_valid_u, b"");
    u.extend_from_slice(sel_valid_u);
    u.extend_from_slice(sel_cle_u);
    let ue = chiffrer_cle(&empreinte_r6(utilisateur, sel_cle_u, b""), &cle)?;

    let sel_o = hasard(16)?;
    let (sel_valid_o, sel_cle_o) = sel_o.split_at(8);
    let mut o = empreinte_r6(&proprietaire, sel_valid_o, &u);
    o.extend_from_slice(sel_valid_o);
    o.extend_from_slice(sel_cle_o);
    let oe = chiffrer_cle(&empreinte_r6(&proprietaire, sel_cle_o, &u), &cle)?;

    // /Perms : les permissions, une marque et 4 octets quelconques, en un bloc AES-256 (BCE).
    let mut perms = [0u8; 16];
    perms[0..4].copy_from_slice(&PERMISSIONS.to_le_bytes());
    perms[4..8].copy_from_slice(&[0xFF; 4]);
    perms[8] = b'T'; // /EncryptMetadata true
    perms[9..12].copy_from_slice(b"adb");
    perms[12..16].copy_from_slice(&hasard(4)?);
    let aes = aes::Aes256::new_from_slice(&cle).map_err(|e| Refus::defaut(format!("clé de chiffrement invalide ({})", e)))?;
    aes.encrypt_block((&mut perms).into());

    // Chaînes et flux, tous chiffrés (même les métadonnées, comme /EncryptMetadata true le dit).
    let mut erreur: Option<Refus> = None;
    for (_, objet) in doc.objects.iter_mut() {
        let mut chiffre = |d: &[u8]| chiffrer_donnees(&cle, d);
        let mut chiffre2 = |d: &[u8]| chiffrer_donnees(&cle, d);
        if let Err(e) = parcourir(objet, &mut chiffre, &mut chiffre2) {
            erreur = Some(e);
            break;
        }
    }
    if let Some(e) = erreur {
        return Err(e);
    }

    let mut cf = Dictionary::new();
    cf.set("AuthEvent", Object::Name(b"DocOpen".to_vec()));
    cf.set("CFM", Object::Name(b"AESV3".to_vec()));
    cf.set("Length", Object::Integer(32));
    let mut filtres = Dictionary::new();
    filtres.set("StdCF", Object::Dictionary(cf));
    let chaine = |v: Vec<u8>| Object::String(v, StringFormat::Hexadecimal);
    let mut e = Dictionary::new();
    e.set("Filter", Object::Name(b"Standard".to_vec()));
    e.set("V", Object::Integer(5));
    e.set("R", Object::Integer(6));
    e.set("Length", Object::Integer(256));
    e.set("P", Object::Integer(PERMISSIONS as i32 as i64));
    e.set("EncryptMetadata", Object::Boolean(true));
    e.set("CF", Object::Dictionary(filtres));
    e.set("StmF", Object::Name(b"StdCF".to_vec()));
    e.set("StrF", Object::Name(b"StdCF".to_vec()));
    e.set("O", chaine(o));
    e.set("U", chaine(u));
    e.set("OE", chaine(oe));
    e.set("UE", chaine(ue));
    e.set("Perms", chaine(perms.to_vec()));
    let id_chiffrement = doc.add_object(Object::Dictionary(e));
    doc.trailer.set("Encrypt", Object::Reference(id_chiffrement));
    // L'identifiant du fichier (deux chaînes de 16 octets, jamais chiffrées).
    let id = chaine(hasard(16)?);
    doc.trailer.set("ID", Object::Array(vec![id.clone(), id]));

    let mut sortie = Vec::new();
    doc.save_to(&mut sortie).map_err(|e| Refus::defaut(format!("écriture du PDF impossible ({})", e)))?;
    Ok(sortie)
}

/// La clé de fichier chiffrée par la clé dérivée du mot de passe (AES-256-CBC, IV nul, sans bourrage).
fn chiffrer_cle(cle_derivee: &[u8], cle_fichier: &[u8]) -> Result<Vec<u8>, Refus> {
    Ok(CbcChiffre256::new_from_slices(cle_derivee, &[0u8; 16])
        .map_err(|e| Refus::defaut(format!("clé de chiffrement invalide ({})", e)))?
        .encrypt_padded_vec_mut::<NoPadding>(cle_fichier))
}

// ---------------------------------------------------------------------------------------------
// Déchiffrer.
// ---------------------------------------------------------------------------------------------

/// Le dictionnaire `/Encrypt` du fichier (direct dans la queue, ou par référence), et l'objet qui
/// le porte s'il est indirect.
fn dictionnaire_de_chiffrement(doc: &Document) -> Result<Option<(Dictionary, Option<ObjectId>)>, Refus> {
    let id = match directe(&doc.trailer, b"Encrypt") {
        None => return Ok(None),
        Some(Object::Reference(id)) => Some(*id),
        Some(_) => None,
    };
    match lecture::lue(doc, &doc.trailer, b"Encrypt", lecture::dictionnaire)? {
        Some(d) => Ok(Some((d.clone(), id))),
        None => Ok(None),
    }
}

/// Déchiffre `doc` avec ce mot de passe utilisateur (sans effet si le fichier n'est pas chiffré).
pub fn dechiffrer(doc: &mut Document, mot_de_passe: &[u8]) -> Result<(), Refus> {
    let (e, id_e) = match dictionnaire_de_chiffrement(doc)? {
        Some(x) => x,
        None => return Ok(()),
    };
    // V : « The default value if this entry is omitted shall be 0 » (ISO 32000-1, table 20), un algorithme non
    // documenté que la fabrique ne lit pas (comme lopdf, plus bas) ; R est exigé (table 21).
    let v = match lecture::lue(doc, &e, b"V", lecture::entier)? {
        Some(v) => v,
        None => return illisible("chiffrement illisible : /V manque (0, un algorithme non documenté)"),
    };
    let r = match lecture::lue(doc, &e, b"R", lecture::entier)? {
        Some(r) => r,
        None => return illisible("chiffrement illisible : /R manque"),
    };
    if v < 5 {
        // RC4 (V 1 et 2) : lopdf, qui veut le dictionnaire en objet indirect.
        if id_e.is_none() {
            let id = doc.add_object(Object::Dictionary(e));
            doc.trailer.set("Encrypt", Object::Reference(id));
        }
        return doc.decrypt(mot_de_passe).map_err(|err| {
            Refus::illisible(format!("chiffrement non pris en charge ou mot de passe refusé (V {}, R {} : {})", v, r, err))
        });
    }
    let (u, ue) = match (lecture::lue(doc, &e, b"U", lecture::chaine)?, lecture::lue(doc, &e, b"UE", lecture::chaine)?) {
        (Some(u), Some(ue)) if u.len() >= 48 && ue.len() >= 32 => (u.to_vec(), ue.to_vec()),
        (Some(_) | None, Some(_) | None) => return illisible("chiffrement illisible : /U ou /UE manque"),
    };
    // Le mot de passe utilisateur doit rendre l'empreinte de validation.
    let (empreinte_u, sel_valid, sel_cle) = (&u[..32], &u[32..40], &u[40..48]);
    if empreinte(r, mot_de_passe, sel_valid, b"") != empreinte_u {
        return illisible("le PDF est chiffré et ne s'ouvre pas sans mot de passe");
    }
    let cle_derivee = empreinte(r, mot_de_passe, sel_cle, b"");
    let cle = CbcDechiffre256::new_from_slices(&cle_derivee, &[0u8; 16])
        .map_err(|e| Refus::defaut(format!("clé de chiffrement invalide ({})", e)))?
        .decrypt_padded_vec_mut::<NoPadding>(&ue[..32])
        .map_err(|e| Refus::illisible(format!("clé de fichier illisible ({})", e)))?;

    // Les filtres de chiffrement : seul AESV3 est pris en charge. Un filtre non dit est `Identity`, qui laisse en
    // clair (ISO 32000-1, table 20, StmF et StrF : « Default value: Identity » ; table 26 : « Input data shall be
    // passed through without any processing »). Un autre nom est une clé de `/CF` (table 20).
    let filtre = |cle: &[u8]| -> Result<bool, Refus> {
        let n = match lecture::lue(doc, &e, cle, lecture::nom)? {
            Some(n) => n,
            None => FILTRE_PAR_DEFAUT,
        };
        if n == FILTRE_PAR_DEFAUT {
            return Ok(false);
        }
        let cf = match lecture::lue(doc, &e, b"CF", lecture::dictionnaire)? {
            Some(cf) => cf,
            None => return illisible(format!("chiffrement illisible : {} nomme /{}, et /CF manque", lecture::nom_de(cle), String::from_utf8_lossy(n))),
        };
        let f = match lecture::lue(doc, cf, n, lecture::dictionnaire)? {
            Some(f) => f,
            None => return illisible(format!("chiffrement illisible : /CF n'a pas le filtre /{}", String::from_utf8_lossy(n))),
        };
        match lecture::lue(doc, f, b"CFM", lecture::nom)? {
            Some(b"AESV3") => Ok(true),
            Some(_) | None => illisible("chiffrement non pris en charge (seul AES-256 l'est)"),
        }
    };
    let chiffre_flux = filtre(b"StmF")?;
    let chiffre_chaines = filtre(b"StrF")?;
    let metadonnees = match lecture::lue(doc, &e, b"EncryptMetadata", lecture::booleen)? {
        Some(b) => b,
        None => METADONNEES_CHIFFREES,
    };

    for (id, objet) in doc.objects.iter_mut() {
        if Some(*id) == id_e || est_de_structure(objet) {
            continue;
        }
        if !metadonnees && dictionnaire_de(objet).is_some_and(|d| matches!(directe(d, b"Type"), Some(Object::Name(n)) if n == b"Metadata")) {
            continue;
        }
        let objet_lu = format!("objet {} {}", id.0, id.1);
        let mut chaine = |d: &[u8]| if chiffre_chaines { dechiffrer_donnees(&cle, d) } else { Ok(d.to_vec()) };
        let mut flux = |d: &[u8]| if chiffre_flux { dechiffrer_donnees(&cle, d) } else { Ok(d.to_vec()) };
        parcourir(objet, &mut chaine, &mut flux).map_err(|r| r.reformuler(|p| format!("{} : {}", objet_lu, p)))?;
    }
    if let Some(id) = id_e {
        doc.objects.remove(&id);
    }
    doc.trailer.remove(b"Encrypt");
    Ok(())
}
