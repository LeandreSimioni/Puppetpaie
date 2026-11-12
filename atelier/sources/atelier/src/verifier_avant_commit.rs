//! `moteur-atelier verifier-avant-commit [--indexes]` : le garde avant commit. Il cherche, dans ce que Git emporterait du dépôt, les
//! données personnelles : trois motifs génériques (un NIR, un IBAN, un BIC), puis, si `prive/config/` est là, les VRAIES
//! valeurs de la compagnie du poste qu'il y lit (SIRET, SIREN, comptes Urssaf, Congés Spectacles et France Travail, IBAN,
//! BIC, et pour chaque salarié son NIR, son numéro Congés Spectacles, son courriel, son adresse, son nom et son prénom).
//!
//! Il lit chaque fichier tel qu'il est écrit, octet par octet : un fichier compressé (un PDF à flux compressés, un xlsx,
//! un docx, un zip, un png) n'est pas décompressé, et ce qu'il porte dedans ne se voit pas.
//!
//! Le verrou de la suite (`tests/fichiers/donnees_reelles.rs`) passe, à chaque passage de tests, la même liste
//! (`ce_qui_part`) au même détecteur de NIR (`nirs_non_inventes`), avec les mêmes textes déposés excusés (leur empreinte,
//! `crate::empreintes`), et y cherche le nom
//! de la compagnie du poste dans le code. Ce garde-ci fait ce que le verrou ne peut pas faire sans porter lui-même ce
//! qu'il cherche : il LIT les vraies valeurs dans `prive/config/` (jamais
//! `prive/config/*.bin`) et les cherche dans ce qui part. **Il ne contient aucune donnée réelle** : un garde-fou qui
//! porterait la liste des NIR à protéger serait exactement ce qu'il prétend empêcher. Il ne tourne pas dans la suite (un
//! test ne lit jamais `prive/`) ; il tourne avant chaque commit, en crochet git (`atelier/.githooks/pre-commit`, posé par
//! `git config core.hooksPath atelier/.githooks`), et à la demande.
//!
//! Ce qu'il ne faut surtout pas en faire : nettoyer un test parce qu'il le signale. Une trouvaille se regarde : une valeur
//! réelle sort du dépôt public, une valeur inventée s'inscrit dans `FICTIVES` (ou porte la marque d'invention d'un NIR,
//! `INVENTE`, la convention du verrou).
//!
//! Il ne dit jamais la valeur trouvée : le fichier, la ligne, et ce que c'est (le NIR d'un salarié, l'IBAN de la
//! compagnie…), rien d'autre ; la sortie d'un crochet se lit partout, dans un terminal comme dans la séance d'un agent.
//!
//! Sortie : 0 si rien n'a été trouvé, 1 sinon, avec la liste fichier par fichier ; 2 s'il n'a pas pu vérifier (hors d'un
//! dépôt Git, ou un fichier de `prive/config/` illisible : un garde qui ne lit pas ce qu'il protège ne passe pas).

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use regex::Regex;

use socle::echec::{refus, R};

/// Ce qui ne part jamais : les données de la compagnie du poste.
const PRIVE: &str = "prive/";

/// La marque d'un NIR inventé : une commune ou un rang que personne n'a, dans les treize premiers chiffres.
const INVENTE: [&str; 3] = ["999", "123456", "000"];

/// Les valeurs FABRIQUÉES qui remplacent les vraies dans les tests et les exemples. Elles ne désignent personne, mais elles
/// ont la forme du vrai -- c'est même leur intérêt : leurs clés se vérifient, donc un test ne peut pas passer sur un numéro
/// impossible. Sans cette liste, le motif générique les signalerait à chaque fois et le garde-fou finirait ignoré.
///
/// Y ajouter une valeur est un acte visible, relu comme le reste du code. N'y mettre QUE des valeurs inventées : cette
/// liste est publiée avec le dépôt.
pub const FICTIVES: [(&str, &str); 10] = [
    ("2930899999999", "NIR féminin fabriqué (clé 47)"),
    ("1850375999999", "NIR masculin fabriqué (clé 87)"),
    // Les deux ci-dessus finissent par six « 9 », ce que la norme n'admet pas (CCH-16) : ils servent d'exemples dans des
    // comptes rendus, jamais dans une fiche. Celui-ci est fabriqué ET déclarable -- c'est celui qu'il faut quand un test
    // construit une fiche de salarié.
    ("1850375999001", "NIR masculin fabriqué et déclarable (clé 18)"),
    ("1234567890123", "NIR de forme invalide, pour les tests de rejet"),
    ("12345678900017", "SIRET fabriqué (clé de Luhn juste)"),
    // Les compagnies fictives ont besoin d'un compte : la DSN porte un ordre de télépaiement, et un IBAN de forme fausse
    // ferait refuser le fichier par la plate-forme avant qu'on ait rien éprouvé. Celui-ci est fabriqué, sa clé de contrôle
    // (85) est juste, et il ne désigne aucun compte.
    ("FR7630001007941234567890185", "IBAN fabriqué (clé 85 juste)"),
    // Les deux suivants se disent inventés dans leur fichier : la vie de démonstration
    // `atelier/outils/demos/les-faisceaux/gestes.jsonl`, que reprend le scenario du banc `tests/donnees/scenarios/treteaux-du-banc/` (« Aucune valeur ne désigne une entreprise ou une personne réelle »), et
    // `tests/fichiers/caviardage.rs` (« les IBAN sont faits de zéros »).
    ("FR7630001007943252044000039", "IBAN de la démonstration Les Faisceaux"),
    ("FR7630001000000000000000185", "IBAN fait de zéros, test du caviardage"),
    // La fictive de référence du Fil de Fer (`atelier/fictives/fictive-fil-de-fer/LISEZ-MOI.md` : « tout ce qui suit est
    // inventé ») : le compte de la compagnie, clé juste, banque « 12345 ».
    ("FR7612345678900005556660781", "IBAN de la fictive Compagnie du Fil de Fer"),
    // La fictive de référence de l'Hôtel de Bourgogne (`atelier/fictives/fictive-hotel-de-bourgogne/LISEZ-MOI.md` : « tout
    // ce qui suit est inventé ») : le compte de la compagnie, clé juste, banque « 12345 ».
    ("FR7612345678900007778880147", "IBAN de la fictive Compagnie de l'Hôtel de Bourgogne"),
];

/// Les motifs génériques (une forme), par opposition aux valeurs réelles lues.
const GENERIQUES: [&str; 3] = ["NIR", "IBAN", "BIC"];

/// Une valeur trouvée : où, et ce que c'est. Jamais la valeur elle-même.
#[derive(Debug, Clone, PartialEq)]
pub struct Trouvaille {
    /// Relatif à la racine du dépôt, en `/`.
    pub fichier: String,
    /// À partir de 1, les lignes comptées par `socle::texte::lignes_du_texte`.
    pub ligne: usize,
    pub quoi: String,
}

/// Un motif et ce qu'il désigne : un NIR (`nirs_non_inventes`), une forme d'IBAN ou de BIC (une expression, que
/// `porte_fr` épargne aux lignes où elle ne peut rien trouver : la `regex` du moteur est compilée sans ses
/// optimisations, qui pèsent sur le binaire), ou une valeur lue, cherchée sans tenir compte de la casse.
enum Motif {
    Nir,
    Forme { re: Regex, quoi: &'static str },
    Valeur { minuscules: String, quoi: String },
}

impl Motif {
    fn quoi(&self) -> &str {
        match self {
            Motif::Nir => "NIR",
            Motif::Forme { quoi, .. } => quoi,
            Motif::Valeur { quoi, .. } => quoi,
        }
    }

    /// Une valeur qui n'est pas inventée, dans `ligne` (`minuscules` : la même, en minuscules) d'un fichier qui est un
    /// `binaire` ou non.
    fn trouve(&self, ligne: &str, minuscules: &str, binaire: bool) -> bool {
        match self {
            Motif::Nir => !nirs_non_inventes(ligne.as_bytes(), binaire).is_empty(),
            Motif::Forme { re, .. } => porte_fr(ligne) && re.find_iter(ligne).any(|t| !fictive(t.as_str())),
            Motif::Valeur { minuscules: v, .. } => minuscules.contains(v.as_str()),
        }
    }
}

/// Les numéros à la forme d'un NIR qui ne sont pas inventés, hors d'une adresse web : `(début, numéro)`. La forme : sexe,
/// année, mois (01-12, ou 20-99 pour les cas spéciaux), département (deux chiffres, 2A, 2B), six chiffres, puis la clé si
/// elle suit ; ni précédé d'une lettre ou d'un chiffre, ni suivi d'un troisième chiffre (un horodatage « 20260906080000 »
/// en a quatorze). Dans un `binaire` seulement (un fichier qui n'est pas de l'UTF-8, `binaire`), ni les décimales d'une
/// constante écrite « 0. » (les quinze chiffres qu'une bibliothèque porte dans les binaires livrés) ; dans un texte, un
/// numéro après « 0. » se signale comme un autre. Inventé : listé dans `FICTIVES`, ou la marque `INVENTE` dans ses treize
/// chiffres.
pub fn nirs_non_inventes(o: &[u8], binaire: bool) -> Vec<(usize, String)> {
    let chiffre = |i: usize| o.get(i).is_some_and(u8::is_ascii_digit);
    let mut trouves = Vec::new();
    for i in 0..o.len().saturating_sub(12) {
        let decimales = binaire && i > 1 && o[i - 1] == b'.' && o[i - 2] == b'0' && !(i > 2 && o[i - 3].is_ascii_digit());
        if !matches!(o[i], b'1' | b'2') || (i > 0 && o[i - 1].is_ascii_alphanumeric()) || decimales {
            continue;
        }
        let mois = matches!((o[i + 3], o[i + 4]), (b'0', b'1'..=b'9') | (b'1', b'0'..=b'2') | (b'2'..=b'9', b'0'..=b'9'));
        let departement = (chiffre(i + 5) && chiffre(i + 6)) || (o[i + 5] == b'2' && matches!(o[i + 6], b'A' | b'B'));
        if !(chiffre(i + 1) && chiffre(i + 2) && mois && departement && (i + 7..i + 13).all(chiffre)) {
            continue;
        }
        let suite = (i + 13..).take_while(|&j| chiffre(j)).count();
        let treize = String::from_utf8_lossy(&o[i..i + 13]);
        if (suite == 0 || suite == 2) && !INVENTE.iter().any(|m| treize.contains(m)) && !dans_une_url(o, i) {
            let nir = String::from_utf8_lossy(&o[i..i + 13 + suite]).into_owned();
            if !fictive(&nir) {
                trouves.push((i, nir));
            }
        }
    }
    trouves
}

/// Un fichier qui n'est pas de l'UTF-8 : un binaire.
pub fn binaire(o: &[u8]) -> bool {
    std::str::from_utf8(o).is_err()
}

/// Un numéro de document dans une adresse web (l'Unédic en a de treize chiffres, EUR-Lex en met dans ses requêtes)
/// n'est pas un NIR.
fn dans_une_url(o: &[u8], i: usize) -> bool {
    let debut = i - o[..i].iter().rev().take_while(|b| !b" \t\n\"'(<".contains(b)).count();
    o[debut..].starts_with(b"http://") || o[debut..].starts_with(b"https://")
}

/// « FR » : ce que tout IBAN et tout BIC du motif générique porte.
fn porte_fr(ligne: &str) -> bool {
    ligne.contains("FR")
}

/// Les trois motifs génériques. La casse compte : un BIC et un IBAN s'écrivent en capitales, et chercher « CHIFFRE » sans
/// tenir compte de la casse trouve le mot « chiffres » dans une phrase française.
fn generiques() -> Vec<Motif> {
    let formes = [
        (r"\bFR\d{2} ?(?:\d{4} ?){5}\d{3}\b", "IBAN"),
        // Un BIC est en capitales et porte au moins un chiffre chez nous.
        (r"\b[A-Z]{4}FR[A-Z0-9]{2}[0-9][A-Z0-9]{0,2}\b", "BIC"),
    ];
    std::iter::once(Motif::Nir)
        .chain(formes.into_iter().map(|(m, quoi)| Motif::Forme { re: Regex::new(m).unwrap(), quoi }))
        .collect()
}

/// Les valeurs qu'un champ d'une fiche datee a portees, a quelque date que ce soit, dans l'ordre ecrit : celles qui
/// disent quelque chose (ni nulles, ni vides).
fn portees<T: AsRef<str>>(liste: &Option<Vec<droit::fiche::Datee<T>>>) -> Vec<String> {
    liste.iter().flatten().filter_map(|d| d.valeur.as_ref()).map(|v| v.as_ref().to_string()).filter(|v| !v.is_empty()).collect()
}

/// Un fichier de `prive/config/`, lu par sa forme (`droit::fiche::lire_fichier`) : `None` s'il n'y est pas ; une erreur
/// s'il y est sans se lire.
fn lire_config<T: serde::de::DeserializeOwned>(cfg: &Path, nom: &str) -> R<Option<T>> {
    let p = cfg.join(nom);
    if !p.exists() {
        return Ok(None);
    }
    droit::fiche::lire_fichier(&p, &format!("prive/config/{}", nom)).map(Some)
}

/// Les valeurs à ne jamais publier, lues hors du dépôt (`<racine>/prive/config/`), dans l'ordre où on les lit, une fois
/// chacune (une valeur vue deux fois garde sa place et prend le dernier nom). Sous six
/// caractères, trop de fausses alertes : elles ne se cherchent pas. Sans `prive/config/` -- un clone frais, le nuage --,
/// rien : les motifs génériques suffisent à attraper un NIR ou un IBAN quelconque.
///
/// Un fichier absent ou illisible ne fait pas retomber en silence sur les seuls motifs génériques
/// (`entreprise.json`, `salaries.json`) : un fichier absent ne se lit pas et l'autre se lit quand même ; un
/// fichier présent mais illisible arrête tout (code 2) : un garde qui ne lit pas ce qu'il protège ne laisse pas passer.
/// Et le nom d'un salarié n'est jamais dans ce que la commande affiche (`salarie.nir`, pas la clé de sa fiche).
fn valeurs_reelles(racine: &Path) -> R<Vec<(String, String)>> {
    let cfg = racine.join("prive").join("config");
    let mut valeurs: Vec<(String, String)> = Vec::new();
    let mut poser = |v: String, quoi: String| match valeurs.iter_mut().find(|(x, _)| *x == v) {
        Some(e) => e.1 = quoi,
        None => valeurs.push((v, quoi)),
    };
    if !cfg.is_dir() {
        return Ok(Vec::new());
    }
    if let Some(ent) = lire_config::<droit::fiche::FicheEntrepriseDatee>(&cfg, "entreprise.json")? {
        // Toute valeur que la fiche a portee, a quelque date que ce soit (`portees`).
        let cles = [
            ("siret", portees(&ent.siret)),
            ("siren", portees(&ent.siren)),
            ("urssaf_compte", portees(&ent.urssaf_compte)),
            ("congesspectacles_adherent", portees(&ent.congesspectacles_adherent)),
            ("affiliation_france_travail", portees(&ent.affiliation_france_travail)),
        ];
        for (cle, valeurs) in cles {
            for v in valeurs {
                poser(v, cle.to_string());
            }
        }
        for banque in ent.banque.iter().flatten().filter_map(|d| d.valeur.as_ref()) {
            for (cle, v) in [("iban", &banque.iban), ("bic", &banque.bic)] {
                if let Some(v) = v.as_ref().filter(|v| !v.is_empty()) {
                    poser(v.clone(), format!("banque.{}", cle));
                }
            }
        }
    }
    if let Some(sal) = lire_config::<droit::fiche::Salaries>(&cfg, "salaries.json")? {
        for s in sal.fiches.values() {
            let cles = [("nir", portees(&s.nir)), ("num_conges_spectacles", portees(&s.num_conges_spectacles)), ("email", portees(&s.email)), ("adresse", portees(&s.adresse))];
            for (cle, valeurs) in cles {
                for v in valeurs {
                    poser(v, format!("salarie.{}", cle));
                }
            }
            // Le nom seul est trop courant pour être un motif ; le nom ET le prénom collés le sont beaucoup moins.
            for nom in portees(&s.nom) {
                for prenom in portees(&s.prenom) {
                    poser(format!("{} {}", nom, prenom), "salarie (identite)".to_string());
                }
            }
        }
    }
    Ok(valeurs.into_iter().filter(|(v, _)| v.chars().count() >= 6).collect())
}

/// Les motifs : les génériques, puis les vraies valeurs, sans tenir compte de la casse (les deux côtés en minuscules).
/// Une valeur lue qui est elle-même dans `FICTIVES` ne se cherche pas :
/// c'est une compagnie fictive copiée dans `prive/`, pas une donnée réelle.
fn motifs(racine: &Path) -> R<Vec<Motif>> {
    let mut m = generiques();
    for (v, quoi) in valeurs_reelles(racine)? {
        let sans_espaces = v.replace(' ', "");
        if !FICTIVES.iter().any(|(f, _)| f.eq_ignore_ascii_case(&sans_espaces)) {
            m.push(Motif::Valeur { minuscules: v.to_lowercase(), quoi });
        }
    }
    Ok(m)
}

fn git(racine: &Path, args: &[&str]) -> R<Vec<u8>> {
    let sortie = Command::new("git")
        .arg("-C")
        .arg(racine)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| refus!("git introuvable ({}) : ce garde lit ce que Git emporterait", e))?;
    if !sortie.status.success() {
        return Err(refus!("git {} : {}", args.join(" "), String::from_utf8_lossy(&sortie.stderr).trim()));
    }
    Ok(sortie.stdout)
}

/// La racine du dépôt Git où l'on est (`git rev-parse --show-toplevel`) : c'est là que se trouve `prive/`.
pub fn racine_du_depot(depuis: &Path) -> R<PathBuf> {
    let o = git(depuis, &["rev-parse", "--show-toplevel"])?;
    let t = String::from_utf8(o).map_err(|_| refus("git rev-parse : chemin illisible"))?;
    Ok(PathBuf::from(t.trim_end_matches(['\n', '\r'])))
}

/// Ce qui part, relatif à la racine : ce que Git commiterait (`indexes`), ou ce qu'il emporterait du dépôt entier (suivis
/// et non suivis, les `.gitignore` appliqués par Git lui-même) -- hors de `prive/` dans les deux cas, quelle que soit
/// l'extension : un binaire se fouille en octets.
pub fn ce_qui_part(racine: &Path, indexes: bool) -> R<Vec<String>> {
    let args: &[&str] = if indexes {
        &["diff", "--cached", "--name-only", "-z", "--diff-filter=ACM"]
    } else {
        &["ls-files", "-z", "--cached", "--others", "--exclude-standard"]
    };
    let mut noms: Vec<String> = git(racine, args)?
        .split(|&b| b == 0)
        .filter(|n| !n.is_empty())
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .filter(|n| !n.starts_with(PRIVE))
        .collect();
    noms.sort();
    noms.dedup();
    if !indexes {
        noms.retain(|n| racine.join(n).is_file());
    }
    Ok(noms)
}

/// Ce que l'index porte pour chaque fichier (`git cat-file --batch`, `:0:<chemin>`) : c'est cela, et non la copie de
/// travail, que le commit emportera. Un fichier indexé
/// puis retouché est jugé sur sa version indexée.
fn contenus_indexes(racine: &Path, noms: &[String]) -> R<Vec<Option<Vec<u8>>>> {
    if noms.is_empty() {
        return Ok(Vec::new());
    }
    let mut p = Command::new("git")
        .arg("-C")
        .arg(racine)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| refus!("git introuvable ({})", e))?;
    let mut entree = p.stdin.take().unwrap();
    let demande: String = noms.iter().map(|n| format!(":0:{}\n", n)).collect();
    let ecrivain = std::thread::spawn(move || entree.write_all(demande.as_bytes()));
    let mut o = Vec::new();
    p.stdout.take().unwrap().read_to_end(&mut o).map_err(|e| refus(e.to_string()))?;
    let fin = p.wait_with_output().map_err(|e| refus(e.to_string()))?;
    ecrivain.join().map_err(|_| refus("git cat-file : écriture interrompue"))?.map_err(|e| refus(e.to_string()))?;
    if !fin.status.success() {
        return Err(refus!("git cat-file : {}", String::from_utf8_lossy(&fin.stderr).trim()));
    }
    let mut contenus = Vec::new();
    let mut i = 0;
    for _ in noms {
        let nl = o[i..].iter().position(|&b| b == b'\n').ok_or_else(|| refus("git cat-file : réponse tronquée"))? + i;
        let entete = String::from_utf8_lossy(&o[i..nl]).into_owned();
        i = nl + 1;
        let champs: Vec<&str> = entete.split(' ').collect();
        if champs.len() == 3 && champs[1] == "blob" {
            let n: usize = champs[2].parse().map_err(|_| refus!("git cat-file : en-tête illisible ({})", entete))?;
            contenus.push(Some(o.get(i..i + n).ok_or_else(|| refus("git cat-file : réponse tronquée"))?.to_vec()));
            i += n + 1;
        } else {
            // `missing` : un sous-module, ou un chemin que l'index ne porte plus.
            contenus.push(None);
        }
    }
    Ok(contenus)
}

/// Une valeur listée dans `FICTIVES`. Un IBAN se compare sans ses espaces (il s'écrit par groupes de quatre).
fn fictive(trouve: &str) -> bool {
    let sans_espaces = trouve.replace(' ', "");
    FICTIVES.iter().any(|(f, _)| *f == sans_espaces)
}

/// Ce que `motifs` trouvent dans un texte : au plus une trouvaille par ligne, celle du premier motif qui y trouve une
/// valeur qui n'est pas inventée.
///
/// Un texte depose tel que son emetteur l'a publie (son empreinte est celle que son `LISEZ-MOI` porte, `deposes`) :
/// un numero qui y a la forme d'un NIR, d'un IBAN ou d'un BIC est le sien, seuls les motifs GENERIQUES s'y taisent,
/// jamais les vraies valeurs lues dans `prive/config/`.
fn chercher(fichier: &str, o: &[u8], motifs: &[Motif], deposes: &BTreeMap<String, String>, trouvailles: &mut Vec<Trouvaille>) {
    let exemple = depose_tel_quel(deposes, fichier, o);
    let siens: Vec<&Motif> = motifs.iter().filter(|m| !(exemple && GENERIQUES.contains(&m.quoi()))).collect();
    // Un binaire se fouille en octets : ce qui n'est pas de l'UTF-8 devient un remplaçant, qui sépare les chiffres
    // d'avant et d'après au lieu de les recoller.
    let texte = String::from_utf8_lossy(o);
    let binaire = binaire(o);
    // Les séparateurs de lignes n'ont pas de casse : les deux textes ont les mêmes lignes. Sans valeur lue (un clone
    // frais, le nuage), les minuscules ne servent à rien.
    let valeurs = siens.iter().any(|m| matches!(m, Motif::Valeur { .. }));
    let minuscules = if valeurs { texte.to_lowercase() } else { String::new() };
    let lignes_min = if valeurs { socle::texte::lignes_du_texte(&minuscules) } else { Vec::new() };
    let lignes_min = lignes_min.into_iter().chain(std::iter::repeat(""));
    for (n, (ligne, min)) in socle::texte::lignes_du_texte(&texte).into_iter().zip(lignes_min).enumerate() {
        if let Some(m) = siens.iter().find(|m| m.trouve(ligne, min, binaire)) {
            trouvailles.push(Trouvaille { fichier: fichier.to_string(), ligne: n + 1, quoi: m.quoi().to_string() });
        }
    }
}

/// Le contenu d'un fichier que git nomme : rien s'il n'est plus sur le disque (effacé, il ne partira pas tel quel).
fn contenu(p: &Path) -> R<Option<Vec<u8>>> {
    match std::fs::read(p) {
        Ok(o) => Ok(Some(o)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(socle::echec::du_disque(p, &e)),
    }
}

/// Le garde, sur le dépôt de `racine` : ce qui partirait (`indexes` : ce que l'index porte), et les valeurs de
/// `<racine>/prive/config/`.
pub fn verifier(racine: &Path, indexes: bool) -> R<Vec<Trouvaille>> {
    let motifs = motifs(racine)?;
    let noms = ce_qui_part(racine, indexes)?;
    let contenus: Vec<Option<Vec<u8>>> = if indexes { contenus_indexes(racine, &noms)? } else { noms.iter().map(|n| contenu(&racine.join(n))).collect::<R<_>>()? };
    let deposes = crate::empreintes::lues(racine)?;
    let mut trouvailles = Vec::new();
    for (nom, o) in noms.iter().zip(contenus) {
        if let Some(o) = o {
            chercher(nom, &o, &motifs, &deposes, &mut trouvailles);
        }
    }
    Ok(trouvailles)
}

/// LE TEXTE `fichier` (relatif a la racine du depot), de contenu `o`, EST-IL DEPOSE TEL QUE SON EMETTEUR L'A PUBLIE :
/// son empreinte est celle que le `LISEZ-MOI` de son dossier porte (`deposes`, `crate::empreintes::lues`). Le verrou de
/// la suite s'en sert aussi.
pub fn depose_tel_quel(deposes: &BTreeMap<String, String>, fichier: &str, o: &[u8]) -> bool {
    deposes.get(fichier).is_some_and(|e| *e == crate::empreintes::empreinte(o))
}

/// Ce que la commande affiche : la phrase de passage, ou chaque fichier et ses lignes, et ce qu'il faut en faire.
pub fn rapport(trouvailles: &[Trouvaille]) -> String {
    if trouvailles.is_empty() {
        return "Rien de personnel dans le code. C'est publiable.\n".to_string();
    }
    let mut s = format!("ARRET - {} occurrence(s) de donnees personnelles :\n\n", trouvailles.len());
    let mut par_fichier: Vec<&Trouvaille> = trouvailles.iter().collect();
    par_fichier.sort_by(|a, b| a.fichier.cmp(&b.fichier));
    let mut courant: Option<&str> = None;
    for t in par_fichier {
        if courant != Some(t.fichier.as_str()) {
            if courant.is_some() {
                s.push('\n');
            }
            s.push_str(&format!("  {}\n", t.fichier));
            courant = Some(&t.fichier);
        }
        s.push_str(&format!("    ligne {:<5} {}\n", t.ligne, t.quoi));
    }
    s.push('\n');
    s.push_str(
        "Ces valeurs identifient des personnes reelles. L'historique d'un\n\
         depot est definitif : une fois pousse, efface ne veut plus rien dire.\n\
         Retirer les identites, garder les chiffres -- ce sont eux qui prouvent\n\
         que le moteur est cale. Voir AVANT-LE-PREMIER-COMMIT.md.\n",
    );
    s
}

const USAGE: &str = "usage : moteur-atelier verifier-avant-commit [--indexes]\n\
\x20 cherche les donnees personnelles dans ce que git emporterait (hors de prive/) :\n\
\x20 NIR, IBAN, BIC, et les vraies valeurs de prive/config/ s'il est la.\n\
\x20 --indexes   ce que l'index porte (le crochet atelier/.githooks/pre-commit)\n\
\x20 sortie : 0 rien trouve, 1 trouve, 2 pas pu verifier\n";

/// `moteur-atelier verifier-avant-commit [--indexes]`, depuis n'importe où dans le dépôt.
pub fn commande(args: &[socle::ligne::Argument]) -> i32 {
    let args: &[String] = &socle::ligne::textes(args);
    let mut indexes = false;
    for a in args {
        match a.as_str() {
            "--indexes" => indexes = true,
            "--aide" | "--help" | "-h" => {
                dire_sans_ligne!("{}", USAGE);
                return 0;
            }
            _ => {
                eprint!("{}", USAGE);
                return 2;
            }
        }
    }
    let r = std::env::current_dir().map_err(|e| refus(e.to_string())).and_then(|d| racine_du_depot(&d)).and_then(|r| verifier(&r, indexes));
    match r {
        Ok(t) => {
            dire_sans_ligne!("{}", rapport(&t));
            i32::from(!t.is_empty())
        }
        Err(m) => {
            eprintln!("verifier-avant-commit : {}", m.en_lignes());
            2
        }
    }
}
