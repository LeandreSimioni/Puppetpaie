//! L'assemblage d'un dossier : fusionner des pages de PDF existants (`FORMAT.md`, section
//! « L'assemblage »). Un seul chemin, `assembler`, qui rend les octets du PDF ou un refus d'une
//! ligne de français.
//!
//! Le principe : on ne recopie que les objets que les pages choisies atteignent (une page d'un
//! gros fichier n'emporte pas le fichier), les attributs hérités du nœud `Pages` (ressources,
//! cadre, rotation) sont posés sur la page, puis les objets identiques sont fusionnés — le logo
//! de chaque bulletin n'est écrit qu'une fois, comme `garbage=4` de PyMuPDF. Aucune date, aucun
//! identifiant : même description, mêmes octets.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::Hasher;

use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::Deserialize;

use crate::description::{illisible, refus, Refus};

/// Un assemblage : `{"format": 1, "pieces": [ … ]}` sur l'entrée standard (`--assembler`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assemblage {
    /// 1 s'il est dit.
    pub format: Option<u32>,
    pub pieces: Vec<Piece>,
}

impl Assemblage {
    /// La porte du texte (`--assembler`) : lu une fois, strictement.
    pub fn lire(texte: &str) -> Result<Assemblage, Refus> {
        crate::forme::lire(texte, "description d'assemblage")
    }
}

/// Une pièce : un fichier PDF, en entier ou pour les pages données (numérotées à partir de 1,
/// dans l'ordre voulu, une page peut revenir) ; sans pages dites, toutes.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Piece {
    pub fichier: std::path::PathBuf,
    pub pages: Option<Vec<u32>>,
}

/// Les attributs qu'une page hérite de son arbre.
const HERITES: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];
/// Au-delà, un arbre des pages qui ne remonte pas à sa racine (un fichier abîmé) se refuse.
const ANCETRES_MAX: usize = 64;

/// Les octets du PDF assemblé, et vrai quand une pièce était verrouillée : une pièce s'ouvre par
/// `lecture::ouvrir` (le mot de passe vide), et c'est à qui écrit l'assemblage de le verrouiller
/// comme elle (`chiffrement::verrouiller`), pour que le verrou de la pièce ne se perde pas en route.
pub fn assembler(a: &Assemblage) -> Result<(Vec<u8>, bool), Refus> {
    if let Some(f) = a.format {
        if f != 1 {
            return refus(format!("format {} inconnu : cette fabrique parle le format 1", f));
        }
    }
    if a.pieces.is_empty() {
        return refus("assemblage vide : aucune pièce");
    }

    let mut sortie = Document::with_version("1.7");
    let id_pages = sortie.new_object_id();
    let id_racine = sortie.new_object_id();
    let mut feuilles: Vec<Object> = Vec::new();
    // Chaque fichier n'est lu qu'une fois, même s'il revient dans plusieurs pièces.
    let mut lus: HashMap<std::path::PathBuf, Document> = HashMap::new();
    let mut verrouille = false;

    for (n, piece) in a.pieces.iter().enumerate() {
        let n = n + 1;
        if !lus.contains_key(&piece.fichier) {
            let (doc, verrou) = crate::lecture::ouvrir(&piece.fichier, true).map_err(|r| r.reformuler(|p| format!("pièce {} : {}", n, p)))?;
            verrouille |= verrou;
            lus.insert(piece.fichier.clone(), doc);
        }
        let src = &lus[&piece.fichier];
        let pages = src.get_pages();
        let choix: Vec<ObjectId> = match &piece.pages {
            None => pages.values().copied().collect(),
            Some(liste) => {
                let mut v = Vec::new();
                for &p in liste {
                    match pages.get(&p) {
                        Some(id) if p >= 1 => v.push(*id),
                        _ => {
                            return refus(format!(
                                "pièce {} : la page {} n'existe pas dans {} ({} page(s))",
                                n, p, piece.fichier.display(), pages.len()
                            ))
                        }
                    }
                }
                v
            }
        };
        let mut copie = Copie::new(src, id_pages);
        for id in &choix {
            feuilles.push(Object::Reference(copie.page(&mut sortie, *id)?));
        }
    }
    if feuilles.is_empty() {
        return refus("assemblage vide : aucune page");
    }

    let mut arbre = Dictionary::new();
    arbre.set("Type", Object::Name(b"Pages".to_vec()));
    arbre.set("Count", Object::Integer(feuilles.len() as i64));
    arbre.set("Kids", Object::Array(feuilles));
    sortie.objects.insert(id_pages, Object::Dictionary(arbre));
    let mut racine = Dictionary::new();
    racine.set("Type", Object::Name(b"Catalog".to_vec()));
    racine.set("Pages", Object::Reference(id_pages));
    sortie.objects.insert(id_racine, Object::Dictionary(racine));
    sortie.trailer.set("Root", Object::Reference(id_racine));

    fusionner_identiques(&mut sortie, &[id_pages, id_racine]);
    sortie.renumber_objects();
    sortie.compress();
    let octets = crate::ecriture::compact(&sortie)?;
    Ok((octets, verrouille))
}

// ---------------------------------------------------------------------------
// La copie des objets d'un fichier source.
// ---------------------------------------------------------------------------

/// Les objets d'une pièce, de la source vers la sortie. Une copie par pièce : les identifiants
/// ne sont pas partagés, c'est `fusionner_identiques` qui rassemble ce qui se ressemble.
struct Copie<'a> {
    src: &'a Document,
    id_pages: ObjectId,
    /// objet source -> objet de sortie
    table: HashMap<ObjectId, ObjectId>,
    /// les pages source retenues -> leur page de sortie : seules références de page gardées
    retenues: HashMap<ObjectId, ObjectId>,
    /// objets de sortie à remplir : (identifiant de sortie, objet source)
    a_faire: Vec<(ObjectId, ObjectId)>,
}

impl<'a> Copie<'a> {
    fn new(src: &'a Document, id_pages: ObjectId) -> Self {
        Copie { src, id_pages, table: HashMap::new(), retenues: HashMap::new(), a_faire: Vec::new() }
    }

    /// Copie une page (attributs hérités posés dessus) et rend son identifiant de sortie.
    fn page(&mut self, sortie: &mut Document, id: ObjectId) -> Result<ObjectId, Refus> {
        // Une page demandée deux fois est copiée deux fois : deux pages distinctes dans l'arbre.
        let neuf = sortie.new_object_id();
        self.retenues.insert(id, neuf);
        let mut dico = crate::lecture::dictionnaire_de(self.src, id)?.clone();
        for cle in HERITES {
            if !dico.has(cle) {
                if let Some(v) = self.herite(id, cle)? {
                    dico.set(cle.to_vec(), v);
                }
            }
        }
        dico.remove(b"Parent");
        let mut objet = Object::Dictionary(dico);
        self.traduire(sortie, &mut objet);
        if let Object::Dictionary(d) = &mut objet {
            d.set("Parent", Object::Reference(self.id_pages));
        }
        sortie.objects.insert(neuf, objet);
        self.vider(sortie);
        Ok(neuf)
    }

    /// La valeur d'un attribut hérité, telle qu'elle s'écrit (une référence reste une référence), en remontant
    /// l'arbre (ISO 32000-1, 7.7.3.4) ; `None` quand aucun ancêtre ne la porte. `Parent` est une référence, absente à
    /// la racine (table 30).
    fn herite(&self, id: ObjectId, cle: &[u8]) -> Result<Option<Object>, Refus> {
        let mut courant = id;
        for _ in 0..ANCETRES_MAX {
            let d = crate::lecture::dictionnaire_de(self.src, courant)?;
            if let Some(v) = crate::lecture::directe(d, cle) {
                return Ok(Some(v.clone()));
            }
            courant = match crate::lecture::directe(d, b"Parent") {
                Some(Object::Reference(p)) => *p,
                Some(_) => return illisible("/Parent d'un nœud des pages n'est pas une référence"),
                None => return Ok(None),
            };
        }
        illisible(format!("l'arbre des pages remonte au-delà de {} ancêtres", ANCETRES_MAX))
    }

    /// Remplace chaque référence par celle de la sortie ; l'objet atteint pour la première fois
    /// est mis en file pour être copié à son tour.
    fn traduire(&mut self, sortie: &mut Document, o: &mut Object) {
        match o {
            Object::Reference(r) => *o = self.reference(sortie, *r),
            Object::Array(v) => v.iter_mut().for_each(|x| self.traduire(sortie, x)),
            Object::Dictionary(d) => self.traduire_dico(sortie, d),
            Object::Stream(s) => self.traduire_dico(sortie, &mut s.dict),
            _ => {}
        }
    }

    fn traduire_dico(&mut self, sortie: &mut Document, d: &mut Dictionary) {
        for (_, v) in d.iter_mut() {
            self.traduire(sortie, v);
        }
    }

    fn reference(&mut self, sortie: &mut Document, r: ObjectId) -> Object {
        if let Some(p) = self.retenues.get(&r) {
            return Object::Reference(*p);
        }
        if let Some(p) = self.table.get(&r) {
            return Object::Reference(*p);
        }
        match self.src.objects.get(&r) {
            // Une page non retenue (cible d'un lien, retour d'une annotation) ou un nœud de
            // l'arbre : sans eux le dossier ne traînerait pas le reste du fichier.
            Some(Object::Dictionary(d))
                if matches!(crate::lecture::directe(d, b"Type"), Some(Object::Name(t)) if t == b"Page" || t == b"Pages") =>
            {
                Object::Null
            }
            Some(_) => {
                let neuf = sortie.new_object_id();
                self.table.insert(r, neuf);
                self.a_faire.push((neuf, r));
                Object::Reference(neuf)
            }
            // Une référence à un objet que le fichier n'a pas désigne l'objet nul (ISO 32000-1, 7.3.10).
            None => Object::Null,
        }
    }

    fn vider(&mut self, sortie: &mut Document) {
        while let Some((neuf, vieux)) = self.a_faire.pop() {
            // `reference` ne met en file qu'un objet que la source a.
            let mut o = match self.src.objects.get(&vieux) {
                Some(o) => o.clone(),
                None => Object::Null,
            };
            self.traduire(sortie, &mut o);
            sortie.objects.insert(neuf, o);
        }
    }
}

// ---------------------------------------------------------------------------
// La fusion des objets identiques.
// ---------------------------------------------------------------------------

fn empreinte(octets: &[u8]) -> (u64, u64) {
    // Deux empreintes indépendantes (SipHash à clés fixes et FNV-1a) : 128 bits, déterministes.
    let mut a = std::collections::hash_map::DefaultHasher::new();
    a.write(octets);
    let mut f: u64 = 0xcbf29ce484222325;
    for b in octets {
        f ^= *b as u64;
        f = f.wrapping_mul(0x100000001b3);
    }
    (a.finish(), f)
}

/// L'écriture canonique d'un objet, ses références lues à travers `rep`.
fn ecrire(o: &Object, rep: &HashMap<ObjectId, ObjectId>, out: &mut Vec<u8>) {
    match o {
        Object::Null => out.push(b'n'),
        Object::Boolean(b) => out.extend_from_slice(if *b { b"T" } else { b"F" }),
        Object::Integer(i) => out.extend_from_slice(format!("i{};", i).as_bytes()),
        Object::Real(r) => out.extend_from_slice(format!("r{};", r.to_bits()).as_bytes()),
        Object::Name(n) => {
            out.extend_from_slice(format!("N{};", n.len()).as_bytes());
            out.extend_from_slice(n);
        }
        Object::String(s, _) => {
            out.extend_from_slice(format!("S{};", s.len()).as_bytes());
            out.extend_from_slice(s);
        }
        Object::Array(v) => {
            out.push(b'[');
            v.iter().for_each(|x| ecrire(x, rep, out));
            out.push(b']');
        }
        Object::Dictionary(d) => ecrire_dico(d, rep, out),
        Object::Stream(s) => {
            ecrire_dico(&s.dict, rep, out);
            let (a, b) = empreinte(&s.content);
            out.extend_from_slice(format!("#{};{};{}", s.content.len(), a, b).as_bytes());
        }
        Object::Reference(r) => {
            // Une référence que la fusion n'a pas remplacée reste elle-même.
            let r = match rep.get(r) {
                Some(remplacee) => remplacee,
                None => r,
            };
            out.extend_from_slice(format!("R{} {};", r.0, r.1).as_bytes());
        }
    }
}

fn ecrire_dico(d: &Dictionary, rep: &HashMap<ObjectId, ObjectId>, out: &mut Vec<u8>) {
    out.push(b'<');
    // Triées : deux dictionnaires qui ne diffèrent que par l'ordre sont le même objet.
    let tri: BTreeMap<&Vec<u8>, &Object> = d.iter().collect();
    for (k, v) in tri {
        out.extend_from_slice(format!("{};", k.len()).as_bytes());
        out.extend_from_slice(k);
        ecrire(v, rep, out);
    }
    out.push(b'>');
}

fn remplacer(o: &mut Object, rep: &HashMap<ObjectId, ObjectId>) {
    match o {
        Object::Reference(r) => {
            if let Some(n) = rep.get(r) {
                *r = *n;
            }
        }
        Object::Array(v) => v.iter_mut().for_each(|x| remplacer(x, rep)),
        Object::Dictionary(d) => d.iter_mut().for_each(|(_, v)| remplacer(v, rep)),
        Object::Stream(s) => s.dict.iter_mut().for_each(|(_, v)| remplacer(v, rep)),
        _ => {}
    }
}

/// Fusionne les objets identiques (hors `gardes` et hors pages), jusqu'à ce que plus rien ne
/// bouge : une police et son descripteur identiques n'en font qu'une, flux compris.
fn fusionner_identiques(doc: &mut Document, gardes: &[ObjectId]) {
    let pages: BTreeSet<ObjectId> = doc.get_pages().values().copied().collect();
    let candidats: Vec<ObjectId> = doc
        .objects
        .keys()
        .copied()
        .filter(|i| !pages.contains(i) && !gardes.contains(i))
        .collect();
    let mut rep: HashMap<ObjectId, ObjectId> = HashMap::new();
    for _ in 0..16 {
        let mut groupes: HashMap<Vec<u8>, ObjectId> = HashMap::new();
        let mut neuf: HashMap<ObjectId, ObjectId> = HashMap::new();
        for id in &candidats {
            let mut cle = Vec::new();
            ecrire(&doc.objects[id], &rep, &mut cle);
            let tete = *groupes.entry(cle).or_insert(*id);
            if tete != *id {
                neuf.insert(*id, tete);
            }
        }
        if neuf == rep {
            break;
        }
        rep = neuf;
    }
    if rep.is_empty() {
        return;
    }
    for id in rep.keys() {
        doc.objects.remove(id);
    }
    for o in doc.objects.values_mut() {
        remplacer(o, &rep);
    }
}
