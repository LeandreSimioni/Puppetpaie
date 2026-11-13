//! La veille des conventions collectives par les livraisons open data KALI de la DILA. Un outil de l'ATELIER
//! (`moteur-atelier sources --verifier`), jamais d'une compagnie.
//!
//! Legifrance refuse les robots (403). Mais la DILA publie chaque soir, en open data et sans compte, ce qui a change dans
//! la base KALI (les conventions collectives) : une archive de quelques centaines de Ko par jour, a
//! `https://echanges.dila.gouv.fr/OPENDATA/KALI/` (`KALI_AAAAMMJJ-HHMMSS.tar.gz`). L'index garde environ un an de
//! livraisons. La base entiere existe aussi (180 Mo) : on ne la prend jamais, on n'en a pas besoin.
//!
//! Dans une livraison, deux choses nous interessent :
//!   - `conteneur/.../KALICONT*.xml` : la fiche d'une convention (IDCC, titre, et la liste de TOUS ses textes attaches,
//!     avec leur titre). Elle n'est livree que le jour ou la convention bouge. C'est le premier signal.
//!   - `texte/version/.../KALITEXT*.xml` : un texte modifie ce jour-la (un avenant nouveau, un arrete d'extension qui
//!     change son etat). Le texte ne dit pas a quelle convention il appartient : on le sait par la liste du conteneur,
//!     vue dans cette livraison ou une precedente, ou parce que le manifeste des sources le cite deja.
//!
//! La liste des textes d'une convention ne vient que de sa fiche : a la premiere veille on remonte l'index a rebours
//! jusqu'a la trouver (une fois), pour savoir ensuite rattacher un texte modifie a sa convention.
//!
//! La lecture est incrementale : chaque livraison lue est resumee une fois dans `atelier/reference/sources/veille-kali.json` (ce
//! qui touche NOS conventions, rien d'autre ; sa forme : `Cache`), et n'est jamais retelechargee. Le rapport est recalcule
//! a chaque fois depuis la date de releve de la source : un signal vu hier ne disparait pas parce qu'on a relance la
//! commande. Le fichier garde ses cles d'une version a l'autre : la veille reprend ou elle s'etait arretee.
//!
//! Une fiche ou un texte KALI est un XML de la DILA : on n'en lit que les elements nommes ici ; un element absent ou vide
//! reste vide (`None`), et le resume ne l'ecrit pas.
//!
//! Ce module ne decide rien : il rend ce qui a bouge (`Rapport`), `sources::verifier` en fait un etat, la commande
//! l'affiche.

use std::path::Path;

use serde::{Deserialize, Serialize};
use droit::fiche::Idcc;
use socle::date::Jour;

use crate::releve::entrepot_dila::{EtatDuTexteDila, IdDila, LivraisonDila};
use crate::EnOrdre;

pub const INDEX: &str = "https://echanges.dila.gouv.fr/OPENDATA/KALI/";
pub const CACHE: &str = "veille-kali.json";
/// Un texte connu par le manifeste, pas par une fiche.
pub const CITE: &str = "(cite par sources.json)";

use socle::echec::{refus, R};

/// Ce qu'on telecharge : une adresse, ses octets (ou la panne, en une phrase).
pub type Telecharger<'a> = dyn FnMut(&str) -> R<Vec<u8>> + 'a;

/// Les livraisons de l'index, sans doublon, dans l'ordre (`entrepot_dila::livraisons`, que l'entrepot KALI lit aussi).
pub fn livraisons(index_html: &str) -> R<Vec<LivraisonDila>> {
    crate::releve::entrepot_dila::livraisons(index_html, "KALI")
}

/// Le jour d'une estampille (`AAAAMMJJ...`, que l'index ecrit) ; une estampille qui ne commence pas par un jour refuse.
pub fn jour(livraison: &str) -> R<socle::date::Jour> {
    livraison
        .get(..8)
        .and_then(|t| socle::piece::Zone::lue(t, socle::piece::format::Aaaammjj).valeur())
        .ok_or_else(|| refus(format!("l'estampille KALI « {} » ne commence pas par un jour (AAAAMMJJ)", livraison)))
}

/// Le jour d'une estampille, sans l'heure (`AAAAMMJJ`).
fn huit(estampille: &str) -> R<String> {
    Ok(jour(estampille)?.aaaammjj())
}

// ------------------------------------------------------------------ l'archive

/// Les fichiers d'une archive `.tar.gz` : (nom, octets). Le format tar se lit dans `entrepot_dila::LecteurTar`, sans
/// dependance (le meme lecteur fait defiler l'archive globale de LEGI en flux).
pub fn fichiers_tar_gz(octets: &[u8]) -> R<Vec<(String, Vec<u8>)>> {
    let mut tar = crate::releve::entrepot_dila::archive(octets);
    let mut out = Vec::new();
    while let Some(m) = tar.suivant()? {
        if m.fichier {
            out.push((m.nom, tar.lire()?));
        }
    }
    Ok(out)
}

/// Le texte de l'element au bout de `chemin` sous `n`, s'il y est et dit quelque chose.
fn texte_de(n: roxmltree::Node, chemin: &[&str]) -> Option<String> {
    let mut courant = n;
    for nom in chemin {
        courant = courant.children().find(|c| c.is_element() && c.tag_name().name() == *nom)?;
    }
    courant.text().map(str::trim).filter(|t| !t.is_empty()).map(str::to_string)
}

/// LA FICHE D'UNE CONVENTION (un conteneur KALI) : son IDCC, son titre, son etat, et ses textes attaches (identifiant ->
/// titre, dans l'ordre de la fiche).
pub struct Fiche {
    idcc: Option<String>,
    titre: Option<String>,
    etat: Option<EtatDuTexteDila>,
    textes: EnOrdre<String>,
}

/// CE QUE DIT UN TEXTE MODIFIE : son titre, ses dates, son etat, son extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texte {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_texte: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifie_le: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etat: Option<EtatDuTexteDila>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<String>,
}

/// L'etat que porte l'element `ETAT` du fichier `nom`, s'il en porte un.
fn etat_lu(nom: &str, etat: Option<String>) -> R<Option<EtatDuTexteDila>> {
    match etat {
        Some(e) => EtatDuTexteDila::lire(&e).map(Some).map_err(|x| x.reformuler(|p| format!("{} : {}", nom, p))),
        None => Ok(None),
    }
}

/// Le jour que porte l'element `element` du fichier `nom` (`AAAA-MM-JJ`), s'il en porte un.
fn jour_lu(nom: &str, element: &str, jour: Option<String>) -> R<Option<Jour>> {
    match jour {
        Some(j) => match socle::piece::Zone::lue(&j, socle::piece::format::JourIso).valeur() {
            Some(d) => Ok(Some(d)),
            None => Err(refus!("{} : {} « {} » n'est pas un jour AAAA-MM-JJ", nom, element, j)),
        },
        None => Ok(None),
    }
}

/// Ce qu'une livraison contient : les conteneurs (par identifiant) et les textes modifies (par identifiant : ce qu'il dit,
/// et le nom de son fichier dans l'archive). Tout, sans filtrer : c'est l'appelant qui sait ce qu'il suit.
fn lire_livraison(octets: &[u8]) -> R<(EnOrdre<Fiche>, EnOrdre<(Texte, String)>)> {
    let mut conteneurs = EnOrdre::new();
    let mut textes = EnOrdre::new();
    for (nom, xml) in fichiers_tar_gz(octets)? {
        if !nom.ends_with(".xml") || !(nom.contains("/conteneur/") || nom.contains("/texte/version/")) {
            continue;
        }
        let t = String::from_utf8(xml).map_err(|e| refus!("{} : pas de l'UTF-8 ({})", nom, e))?;
        let doc = roxmltree::Document::parse(&t).map_err(|e| refus!("{} illisible : {}", nom, e))?;
        let racine = doc.root_element();
        let ident = texte_de(racine, &["META", "META_COMMUN", "ID"]).ok_or_else(|| refus!("{} : sans identifiant (META_COMMUN/ID)", nom))?;
        match racine.tag_name().name() {
            "IDCC" => {
                let champ = |c: &str| texte_de(racine, &["META", "META_SPEC", "META_CONTENEUR", c]);
                let mut liens = EnOrdre::new();
                for l in racine.descendants().filter(|n| n.has_tag_name("LIEN_TXT")) {
                    if let Some(id) = l.attribute("idtxt").filter(|s| !s.is_empty()) {
                        let titre = l.attribute("titretxt").ok_or_else(|| refus!("{} : le lien {} n'a pas de titre", nom, id))?;
                        liens.poser(id.to_string(), titre.trim().to_string());
                    }
                }
                let etat = etat_lu(&nom, champ("ETAT"))?;
                conteneurs.poser(ident, Fiche { idcc: champ("NUM"), titre: champ("TITRE"), etat, textes: liens });
            }
            "TEXTE_VERSION" => {
                let chron = |c: &str| texte_de(racine, &["META", "META_SPEC", "META_TEXTE_CHRONICLE", c]);
                let vers = |c: &str| texte_de(racine, &["META", "META_SPEC", "META_TEXTE_VERSION", c]);
                let titre = vers("TITRE").or_else(|| vers("TITREFULL"));
                let texte = Texte {
                    titre,
                    date_texte: jour_lu(&nom, "DATE_TEXTE", chron("DATE_TEXTE"))?,
                    modifie_le: jour_lu(&nom, "DERNIERE_MODIFICATION", chron("DERNIERE_MODIFICATION"))?,
                    etat: etat_lu(&nom, vers("ETAT"))?,
                    extension: vers("LIBELLE_EXTENSION"),
                };
                textes.poser(ident, (texte, nom));
            }
            _ => {}
        }
    }
    Ok((conteneurs, textes))
}

// ------------------------------------------------------------------ le resume des livraisons lues

/// LE RESUME DES LIVRAISONS LUES (`veille-kali.json`, a cote des sources). Sa note `quoi` dit ce qu'est le fichier.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cache {
    quoi: String,
    /// Par estampille, ce que la livraison avait de nos conventions.
    livraisons: EnOrdre<LivraisonLue>,
    /// Par conteneur, ses textes connus (identifiant -> titre, ou `CITE`).
    textes_connus: EnOrdre<EnOrdre<String>>,
    /// Les plages de livraisons sorties de l'index sans avoir ete lues.
    trous: Vec<Trou>,
    /// De la premiere veille a la derniere livraison vue.
    couverture: Plage,
    /// Par conteneur, la fiche qui a pose la liste de reference.
    amorces: EnOrdre<Amorce>,
}

const QUOI_DU_CACHE: &str = "Les livraisons open data KALI deja lues, resumees a ce qui touche les conventions suivies dans sources.json. Une livraison lue n'est jamais retelechargee ; supprimer ce fichier force la relecture.";

/// Une plage d'estampilles.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plage {
    de: String,
    a: String,
}

/// Une plage de jours (`AAAAMMJJ`) sortie de l'index sans avoir ete lue.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Trou {
    de: String,
    a: String,
}

/// La fiche qui a pose la liste de reference d'une convention : son jour, le nombre de ses textes, son titre.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Amorce {
    pub livraison: Jour,
    pub textes: usize,
    pub titre: Option<String>,
}

/// Ce qu'une livraison avait de nos conventions : leurs fiches, et leurs textes modifies.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LivraisonLue {
    conteneurs: EnOrdre<ConteneurLu>,
    textes: EnOrdre<TexteLu>,
}

/// La fiche d'une convention suivie, telle qu'une livraison l'a portee : `premiere`, la premiere vue (elle pose la liste
/// de reference : rien n'y est nouveau) ; `nouveaux`, ses textes qu'on ne connaissait pas.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConteneurLu {
    idcc: Option<Idcc>,
    etat: Option<EtatDuTexteDila>,
    premiere: bool,
    nouveaux: EnOrdre<String>,
}

/// Un texte d'une convention suivie modifie dans une livraison.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TexteLu {
    #[serde(flatten)]
    texte: Texte,
    conteneur: IdDila,
}

/// Les textes connus d'une convention ne viennent-ils que du manifeste (aucune fiche vue) ?
fn tous_cites(connus: Option<&EnOrdre<String>>) -> bool {
    connus.is_none_or(|m| m.0.iter().all(|(_, v)| v == CITE))
}

// ------------------------------------------------------------------ la veille

/// Une convention suivie : son conteneur, son IDCC, les textes que le manifeste cite.
pub struct Suivi {
    pub conteneur: IdDila,
    pub idcc: Idcc,
    pub textes: Vec<String>,
}

/// CE QUI A BOUGE depuis la date de releve : les livraisons lues, celles lues cette fois, le trou de l'index s'il y en a
/// un (une phrase), les fiches qui ont pose les listes de reference, et les evenements par conteneur.
pub struct Rapport {
    pub livraisons_lues: Vec<LivraisonDila>,
    pub nouvelles: Vec<LivraisonDila>,
    pub trou: Option<String>,
    pub amorces: EnOrdre<Amorce>,
    pub evenements: EnOrdre<Vec<Evenement>>,
}

/// Ce qui a bouge pour une convention : sa fiche relivree, ou un de ses textes modifie.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Evenement {
    Convention(EvenementConvention),
    Texte(EvenementTexte),
}

/// La fiche d'une convention relivree : `premiere` (la liste de reference posee), `nouveaux` (ses textes nouveaux).
#[derive(Debug, Clone, Serialize)]
pub struct EvenementConvention {
    pub livraison: Jour,
    pub genre: &'static str,
    pub ident: IdDila,
    pub titre: String,
    pub premiere: bool,
    pub nouveaux: EnOrdre<String>,
}

/// Un texte d'une convention modifie.
#[derive(Debug, Clone, Serialize)]
pub struct EvenementTexte {
    #[serde(flatten)]
    pub texte: Texte,
    pub conteneur: IdDila,
    pub livraison: Jour,
    pub genre: &'static str,
    pub ident: IdDila,
}

impl Evenement {
    /// Le jour de la livraison qui l'a porte.
    pub fn livraison(&self) -> Jour {
        match self {
            Evenement::Convention(c) => c.livraison,
            Evenement::Texte(t) => t.livraison,
        }
    }
}

/// Retourne voir ce qui a bouge, pour chaque convention suivie, depuis un jour (`depuis` : la releve la plus ancienne ;
/// les livraisons de ce jour-la et d'avant ne comptent pas). `relever` : poser le XML des textes touches dans
/// `dossier/kali/`.
///
/// Une panne de l'index ou d'une archive se rend en `Err` : c'est a l'appelant de dire « injoignable » ; rien n'est
/// alors ecrit.
pub fn veiller(dossier: &Path, suivis: &[Suivi], depuis: &Jour, relever: bool, telecharger: &mut Telecharger) -> R<Rapport> {
    let cache_p = dossier.join(CACHE);
    let avant: Option<Cache> = if cache_p.exists() {
        Some(socle::forme::lire_fichier(&cache_p, &format!("{} ne se lit pas : le deplacer force la relecture.", cache_p.display()))?)
    } else {
        None
    };
    let (mut livraisons_lues, mut connus, mut trous, couverture, mut amorces) = match avant {
        Some(c) => (c.livraisons, c.textes_connus, c.trous, Some(c.couverture), c.amorces),
        None => (EnOrdre::new(), EnOrdre::new(), Vec::new(), None, EnOrdre::new()),
    };
    for s in suivis {
        let c = connus.entree(s.conteneur.texte().to_string(), EnOrdre::new);
        for t in &s.textes {
            c.entree(t.clone(), || CITE.to_string());
        }
    }

    let seuil = format!("{}-999999", depuis.aaaammjj());
    let index = String::from_utf8_lossy(&telecharger(INDEX)?).into_owned();
    let toutes = livraisons(&index)?;
    let (Some(premiere), Some(derniere)) = (toutes.first(), toutes.last()) else {
        return Err(refus("l'index KALI ne liste aucune livraison : la page a change ?"));
    };
    let a_lire: Vec<&LivraisonDila> = toutes.iter().filter(|l| l.texte() > seuil.as_str()).collect();

    // Ce qu'on a couvert sans trou : de la premiere veille a la derniere livraison vue. Si l'index ne remonte plus
    // jusque-la, des livraisons sont sorties sans avoir ete lues -- on le dit, et on s'en souvient.
    let couverture = match couverture {
        None => {
            if premiere.texte() > seuil.as_str() {
                trous.push(Trou { de: depuis.aaaammjj(), a: premiere.jour().aaaammjj() });
            }
            Plage { de: std::cmp::min(seuil.as_str(), premiere.texte()).to_string(), a: derniere.texte().to_string() }
        }
        Some(c) if premiere.texte() > c.a.as_str() => {
            trous.push(Trou { de: huit(&c.a)?, a: premiere.jour().aaaammjj() });
            Plage { de: c.de, a: derniere.texte().to_string() }
        }
        Some(c) => Plage { de: c.de, a: std::cmp::max(c.a, derniere.texte().to_string()) },
    };
    let mut trou = None;
    for t in &trous {
        if t.a > depuis.aaaammjj() {
            trou = Some(format!(
                "des livraisons entre le {} et le {} sont sorties de l'index sans avoir ete lues : verifier la page a la main pour cette periode",
                jour(&t.de)?.iso(),
                jour(&t.a)?.iso()
            ));
        }
    }

    // La liste de reference d'une convention (ses textes attaches) ne vient que d'une fiche livree. Tant qu'on n'en a pas
    // vu, on ne saurait pas rattacher un texte modifie a la convention -- sauf ceux que le manifeste cite. Alors, une
    // fois, on remonte l'index a rebours jusqu'a trouver la derniere fiche de chaque convention suivie.
    let mut a_amorcer: Vec<&str> = suivis.iter().map(|s| s.conteneur.texte()).filter(|c| tous_cites(connus.valeur(c))).collect();
    a_amorcer.dedup();
    for l in toutes.iter().filter(|l| l.texte() <= seuil.as_str()).rev() {
        if a_amorcer.is_empty() {
            break;
        }
        let (conteneurs, _) = lire_livraison(&telecharger(&format!("{}KALI_{}.tar.gz", INDEX, l))?)?;
        for cont in a_amorcer.clone() {
            if let Some(c) = conteneurs.valeur(cont) {
                let ses = connus.entree(cont.to_string(), EnOrdre::new);
                for (k, v) in &c.textes.0 {
                    ses.poser(k.clone(), v.clone());
                }
                amorces.poser(cont.to_string(), Amorce { livraison: l.jour(), textes: c.textes.len(), titre: c.titre.clone() });
                a_amorcer.retain(|x| *x != cont);
            }
        }
    }

    let mut nouvelles = Vec::new();
    for l in a_lire {
        if livraisons_lues.valeur(l.texte()).is_some() {
            continue;
        }
        let octets = telecharger(&format!("{}KALI_{}.tar.gz", INDEX, l))?;
        let (conteneurs, textes) = lire_livraison(&octets)?;
        let mut resume_c = EnOrdre::new();
        let mut resume_t = EnOrdre::new();
        for suivi in suivis {
            let cont = suivi.conteneur.texte();
            if let Some(c) = conteneurs.valeur(cont) {
                // La premiere fiche vue donne la liste de reference : rien n'y est « nouveau », on ne connaissait pas la
                // liste d'avant.
                let premiere = tous_cites(connus.valeur(cont));
                let anciens = connus.entree(cont.to_string(), EnOrdre::new);
                let nouveaux = if premiere { EnOrdre::new() } else { EnOrdre(c.textes.0.iter().filter(|(k, _)| anciens.valeur(k).is_none()).cloned().collect()) };
                // Le NUM de la fiche d'une convention suivie est son IDCC (quatre chiffres) : un autre texte refuse la veille.
                let idcc = match c.idcc.as_deref().map(Idcc::lire).transpose() {
                    Ok(i) => i,
                    Err(e) => return Err(refus!("la fiche KALI de {} ({}) : {}", cont, l, e.phrase)),
                };
                resume_c.poser(cont.to_string(), ConteneurLu { idcc, etat: c.etat, premiere, nouveaux });
                for (k, v) in &c.textes.0 {
                    anciens.poser(k.clone(), v.clone());
                }
            }
        }
        for (ident, (t, membre)) in &textes.0 {
            for suivi in suivis {
                if connus.valeur(suivi.conteneur.texte()).is_some_and(|c| c.valeur(ident).is_some()) {
                    if relever {
                        poser(dossier, l, ident, membre, &octets)?;
                    }
                    resume_t.poser(ident.clone(), TexteLu { texte: t.clone(), conteneur: suivi.conteneur.clone() });
                }
            }
        }
        livraisons_lues.poser(l.texte().to_string(), LivraisonLue { conteneurs: resume_c, textes: resume_t });
        nouvelles.push((*l).clone());
    }

    std::fs::create_dir_all(dossier).map_err(|e| refus!("{} : {}", dossier.display(), e))?;
    let cache = Cache { quoi: QUOI_DU_CACHE.to_string(), livraisons: livraisons_lues, textes_connus: connus, trous, couverture, amorces };
    socle::ecriture::ecrire_json(&cache_p, &cache, 1)?;

    let mut evenements: EnOrdre<Vec<Evenement>> = EnOrdre::new();
    for suivi in suivis {
        evenements.entree(suivi.conteneur.texte().to_string(), Vec::new);
    }
    let mut lues: Vec<&(String, LivraisonLue)> = cache.livraisons.0.iter().filter(|(l, _)| *l > seuil).collect();
    lues.sort_by(|a, b| a.0.cmp(&b.0));
    for (l, r) in lues {
        for (cont, c) in &r.conteneurs.0 {
            // Une fiche relivree sans texte nouveau n'apprend rien de plus que les textes modifies le meme jour : on ne la
            // rapporte pas.
            if c.nouveaux.is_empty() && !c.premiere {
                continue;
            }
            let n = match cache.textes_connus.valeur(cont) {
                Some(t) => t.len(),
                None => return Err(refus!("veille-kali.json : la convention {} n'a pas de textes connus", cont)),
            };
            let titre = if c.premiere {
                format!("premiere fiche de la convention vue : {} texte(s) attache(s), liste de reference posee", n)
            } else {
                match &c.etat {
                    Some(e) => format!("fiche de la convention relivree (etat {})", e),
                    None => "fiche de la convention relivree".to_string(),
                }
            };
            if let Some(ev) = evenements.0.iter_mut().find(|(k, _)| k == cont).map(|(_, v)| v) {
                ev.push(Evenement::Convention(EvenementConvention { livraison: jour(l)?, genre: "convention", ident: IdDila::lire(cont)?, titre, premiere: c.premiere, nouveaux: c.nouveaux.clone() }));
            }
        }
        for (ident, t) in &r.textes.0 {
            if let Some(ev) = evenements.0.iter_mut().find(|(k, _)| k == t.conteneur.texte()).map(|(_, v)| v) {
                ev.push(Evenement::Texte(EvenementTexte { texte: t.texte.clone(), conteneur: t.conteneur.clone(), livraison: jour(l)?, genre: "texte", ident: IdDila::lire(ident)? }));
            }
        }
    }
    Ok(Rapport {
        livraisons_lues: toutes.iter().filter(|l| l.texte() > seuil.as_str()).cloned().collect(),
        nouvelles,
        trou,
        amorces: cache.amorces,
        evenements,
    })
}

/// Le XML d'un texte touche, garde a cote des sources : on le relira, et on ne le perd pas quand la livraison sort de
/// l'index.
fn poser(dossier: &Path, livraison: &LivraisonDila, ident: &str, membre: &str, octets: &[u8]) -> R<()> {
    let Some((_, xml)) = fichiers_tar_gz(octets)?.into_iter().find(|(n, _)| n == membre) else {
        return Err(refus!("{} absent de la livraison {}", membre, livraison));
    };
    let kali = dossier.join("kali");
    let cible = kali.join(format!("{}-{}.xml", ident, livraison.jour().iso()));
    std::fs::create_dir_all(&kali).map_err(|e| refus!("{} : {}", kali.display(), e))?;
    std::fs::write(&cible, xml).map_err(|e| refus!("{} : {}", cible.display(), e))
}
