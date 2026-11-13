//! Les sources des donnees livrees avec le logiciel, et leur veille. Un outil de l'ATELIER, lance avant chaque
//! publication (`moteur-atelier sources --verifier`) ; une compagnie ne fait que recevoir la mise a jour signee.
//!
//! `donnees/sources.json` dit, pour chaque texte dont derive un fichier de donnees (nomenclature des emplois, catalogue
//! des metiers, minima, baremes), ce qu'il contient, de quand il date, quand on l'a releve, ou en est la copie et quel est
//! son condensat. Ce module le lit (`Manifeste`) et sait retourner voir si la source en ligne a bouge.
//!
//! Le principe : une donnee ne se recopie pas d'un editeur, elle se lit dans le texte qui fait foi ; la copie est gardee
//! dans le depot public, a cote du code (`atelier/reference/sources/` du depot :
//! les textes de reference rendent le logiciel verifiable) pour qu'on puisse toujours montrer d'ou
//! vient un nombre ; et quand le condensat en ligne ne colle plus, c'est le signal de RELIRE -- pas de remplacer le
//! fichier a l'aveugle, puisque les donnees derivees ont ete ecrites a la main depuis la lecture. `--relever` pose la
//! nouvelle version A COTE de l'ancienne, suffixee de la date, jamais par-dessus.
//!
//! Legifrance refuse les telechargements automatiques : ces sources-la sont en verification « manuelle » -- la copie ne
//! se reverifie pas en ligne. Mais quand la source porte un bloc `kali` (IDCC, conteneur, texte), on lit les livraisons
//! open data KALI de la DILA depuis le releve (`kali.rs`) : elles disent si la convention ou le texte ont bouge, et la
//! commande dit quelle page ouvrir.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use socle::fichiers::Empreinte;

use crate::releve::kali::{self, Evenement, Rapport, Suivi};
use crate::EnOrdre;
use socle::date::Jour;
use socle::echec::{refus, R};

pub const MANIFESTE: &str = "sources.json";
pub const AGENT: &str = "PuppetPaie (veille des sources ; +https://github.com/puppetpaie)";

/// Ce qu'on telecharge : les octets et l'en-tete `Last-Modified`, ou la panne en une phrase.
pub type Telecharger<'a> = dyn FnMut(&str) -> R<(Vec<u8>, Option<String>)> + 'a;

/// LE MANIFESTE DES SOURCES (`donnees/sources.json`) : chaque source, dans l'ordre du fichier, et les conventions mises de
/// cote pour etre relues avant d'ecrire leur couche.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifeste {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
    pub sources: EnOrdre<Source>,
    pub conventions_a_relever: EnOrdre<ARelever>,
}

/// Une source : ce qu'elle est, ou elle est, sa copie et son condensat, comment elle se verifie, ce qu'elle nourrit.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub titre: String,
    pub editeur: String,
    pub url: String,
    pub contenu: String,
    pub version: String,
    /// Le jour du releve.
    pub releve_le: Jour,
    pub fichier: String,
    pub sha256: Empreinte,
    /// L'original d'un organisme prive n'est pas dans le depot public : seule sa transcription y est (chemin relatif au
    /// meme dossier).
    pub transcription: Option<String>,
    pub verification: Verification,
    pub verification_comment: Option<String>,
    pub nourrit: Vec<String>,
    pub page_de_veille: Option<String>,
    pub en_vigueur: Option<String>,
    pub etendu_le: Option<String>,
    pub kali: Option<KaliSuivi>,
    /// Ou la version se lit, la page, l'article, l'adresse open data, l'encodage : des notes pour le lecteur, que la
    /// veille ne lit pas.
    #[serde(rename = "version_lue_dans")]
    _version_lue_dans: Option<String>,
    #[serde(rename = "page")]
    _page: Option<String>,
    #[serde(rename = "article")]
    _article: Option<String>,
    #[serde(rename = "open_data")]
    _open_data: Option<String>,
    #[serde(rename = "encodage")]
    _encodage: Option<String>,
}

/// Comment une source se reverifie : en la retelechargeant, ou a la main (Legifrance refuse les robots).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verification {
    Telechargement,
    Manuelle,
}

/// Le suivi KALI d'une source : la convention (IDCC, conteneur) et le texte cite.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KaliSuivi {
    pub idcc: String,
    pub conteneur: String,
    pub texte: String,
}

/// Une convention a relever : son titre, son conteneur KALI.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ARelever {
    #[serde(rename = "titre")]
    _titre: String,
    pub conteneur: String,
}

/// Le manifeste livre (`donnees/sources.json`, a cote des autres referentiels).
pub fn manifeste(code: &Path) -> R<Manifeste> {
    let p = code.join(MANIFESTE);
    socle::forme::lire_fichier(&p, &format!("{} ne se lit pas.", p.display()))
}

/// Ou vivent les copies : `atelier/reference/sources/` de l'installation (`maj::installation`), c'est-a-dire du depot,
/// a cote de `logiciel/`. Une compagnie n'y est pour rien : ce sont les textes du logiciel.
pub const COPIES: &str = "atelier/reference/sources";

pub fn dossier(installation: &Path) -> PathBuf {
    installation.join(COPIES)
}

/// Ce que la veille sait d'une source en ligne.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EnLigne {
    Inchangee,
    Changee,
    Injoignable,
    Manuelle,
}

/// Les dates et le commentaire de verification d'une source, quand elle les dit.
#[derive(Debug, Clone, Serialize)]
pub struct Extra {
    #[serde(skip_serializing_if = "Option::is_none")]
    en_vigueur: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    etendu_le: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verification_comment: Option<String>,
}

/// L'ETAT D'UNE SOURCE : ce que le manifeste dit, confronte au disque, puis (`verifier`) a ce qui est en ligne. La copie
/// n'a pas d'etat de conformite quand elle est absente ; la veille n'a rien dit (`en_ligne`) tant qu'on ne l'a pas lancee.
#[derive(Debug, Clone, Serialize)]
pub struct EtatSource {
    pub(crate) ident: String,
    pub(crate) titre: String,
    pub(crate) editeur: String,
    url: String,
    pub(crate) contenu: String,
    pub(crate) version: String,
    pub(crate) releve_le: String,
    verification: Verification,
    pub(crate) nourrit: Vec<String>,
    pub(crate) fichier: String,
    pub(crate) copie_presente: bool,
    pub(crate) copie_conforme: Option<bool>,
    pub(crate) transcription: Option<String>,
    pub(crate) transcription_presente: Option<bool>,
    pub(crate) en_ligne: Option<EnLigne>,
    pub(crate) detail: Option<String>,
    pub(crate) page_de_veille: Option<String>,
    pub(crate) age_jours: i64,
    extra: Extra,
    kali: Option<KaliSuivi>,
    evenements: Vec<Evenement>,
    #[serde(skip)]
    releve: Jour,
}

/// Ce que le manifeste dit, confronte au disque -- sans reseau. Un etat par source, dans l'ordre du manifeste.
pub fn etats(installation: &Path, manifeste: &Manifeste, aujourdhui: &Jour) -> R<Vec<EtatSource>> {
    etats_lus(installation, manifeste, aujourdhui)
}

fn etats_lus(installation: &Path, manifeste: &Manifeste, aujourdhui: &Jour) -> R<Vec<EtatSource>> {
    let d = dossier(installation);
    let mut out = Vec::new();
    for (ident, s) in &manifeste.sources.0 {
        let p = d.join(&s.fichier);
        let presente = p.is_file();
        // La transcription ne se confronte a aucun condensat : c'est l'original qui fait foi.
        let transcription_presente = s.transcription.as_ref().map(|t| d.join(t).is_file());
        let conforme = if presente { Some(Empreinte::du_fichier(&p)? == s.sha256) } else { None };
        let releve = s.releve_le;
        out.push(EtatSource {
            ident: ident.clone(),
            titre: s.titre.clone(),
            editeur: s.editeur.clone(),
            url: s.url.clone(),
            contenu: s.contenu.clone(),
            version: s.version.clone(),
            releve_le: s.releve_le.iso(),
            verification: s.verification,
            nourrit: s.nourrit.clone(),
            fichier: s.fichier.clone(),
            copie_presente: presente,
            copie_conforme: conforme,
            transcription: s.transcription.clone(),
            transcription_presente,
            en_ligne: None,
            detail: None,
            page_de_veille: s.page_de_veille.clone(),
            age_jours: aujourdhui.moins(&releve),
            extra: Extra { en_vigueur: s.en_vigueur.clone(), etendu_le: s.etendu_le.clone(), verification_comment: s.verification_comment.clone() },
            kali: s.kali.clone(),
            evenements: Vec::new(),
            releve,
        });
    }
    Ok(out)
}

/// Le telechargement reel : un GET, l'agent de la veille, 30 s, TLS verifie par la plate-forme, le mandataire de
/// l'environnement. Un statut d'erreur est une panne. L'en-tete `Last-Modified`, quand le serveur le donne en texte.
pub fn telecharger(url: &str, secondes: u64) -> R<(Vec<u8>, Option<String>)> {
    let mut c = ureq::Agent::config_builder().user_agent(AGENT).timeout_global(Some(std::time::Duration::from_secs(secondes)));
    if url.starts_with("https:") {
        c = c.tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build());
    }
    // En essai, le garde-fou reseau : rien ne sort de la machine.
    let agent: ureq::Agent = socle::garde_reseau::regler(c, url)?.build().into();
    let mut r = agent.get(url).call().map_err(|e| refus(e.to_string()))?;
    let modifie = match r.headers().get(ureq::http::header::LAST_MODIFIED) {
        Some(v) => Some(v.to_str().map_err(|e| refus!("{} : Last-Modified n'est pas du texte ({})", url, e))?.to_string()),
        None => None,
    };
    let o = r.body_mut().with_config().limit(512 * 1024 * 1024).read_to_vec().map_err(|e| refus(e.to_string()))?;
    Ok((o, modifie))
}

/// Retourne voir chaque source en ligne (`seulement` : ces identifiants-la, sinon toutes).
///
/// Une source « telechargement » : on la reprend, on compare le condensat a celui du manifeste. `inchangee` : rien a
/// faire. `changee` : la source a bouge, il faut la RELIRE et mettre a jour les donnees qu'elle nourrit ; avec `relever`
/// la nouvelle version est posee a cote de l'ancienne, suffixee de la date, jamais par-dessus. `injoignable` : pas de
/// reseau, ou le lien a change -- ce qui est aussi une information.
///
/// Une source « manuelle » : on rend la page a ouvrir ; on ne cache pas qu'on ne sait pas la verifier. Si elle porte un
/// bloc `kali`, on lit ce que la DILA a livre depuis le releve (`kali_telecharger`) : `inchangee` quand rien ne touche la
/// convention ou que rien ne parle de salaires, `changee` quand un texte salaires (ou le texte cite) a bouge.
pub fn verifier(
    installation: &Path,
    manifeste: &Manifeste,
    aujourdhui: &Jour,
    seulement: &[String],
    relever: bool,
    telecharger: &mut Telecharger,
    kali_telecharger: &mut kali::Telecharger,
) -> R<Vec<EtatSource>> {
    let d = dossier(installation);
    let mut liste: Vec<EtatSource> = etats_lus(installation, manifeste, aujourdhui)?.into_iter().filter(|e| seulement.is_empty() || seulement.contains(&e.ident)).collect();
    let rapport = veiller_kali(&d, &liste, relever, kali_telecharger);
    for e in liste.iter_mut() {
        if e.verification != Verification::Telechargement {
            e.en_ligne = Some(EnLigne::Manuelle);
            e.detail = e.extra.verification_comment.clone();
            if e.kali.is_some() {
                if let Some(r) = &rapport {
                    etat_kali(e, r);
                }
            }
            continue;
        }
        let (octets, modifie) = match telecharger(&e.url) {
            Ok(x) => x,
            Err(m) => {
                e.en_ligne = Some(EnLigne::Injoignable);
                e.detail = Some(m.phrase);
                continue;
            }
        };
        // Le serveur ne donne pas toujours la date de son fichier : rien ne s'en dit alors.
        let modifie = modifie.map(|m| format!("Last-Modified : {}", m));
        let attendu = match manifeste.sources.valeur(&e.ident) {
            Some(s) => &s.sha256,
            None => return Err(refus!("sources.json : la source « {} » n'y est plus", e.ident)),
        };
        if Empreinte::des_octets(&octets) == *attendu {
            e.en_ligne = Some(EnLigne::Inchangee);
            e.detail = modifie;
            continue;
        }
        e.en_ligne = Some(EnLigne::Changee);
        let entre_parentheses = modifie.map(|m| format!(" ({})", m)).into_iter().collect::<String>();
        let mut detail = format!("le fichier en ligne n'est plus celui qui a ete lu{}. Relire, puis mettre a jour : {}", entre_parentheses, nourrit(e));
        if relever {
            let p = d.join(&e.fichier);
            let souche = p.file_stem().ok_or_else(|| refus!("sources.json : « {} » n'a pas de nom de fichier ({})", e.ident, e.fichier))?.to_string_lossy();
            let nom = match p.extension() {
                Some(x) => format!("{}.{}.{}", souche, aujourdhui.iso(), x.to_string_lossy()),
                None => format!("{}.{}", souche, aujourdhui.iso()),
            };
            let neuf = p.with_file_name(&nom);
            if let Some(parent) = neuf.parent() {
                std::fs::create_dir_all(parent).map_err(|e| refus!("{} : {}", parent.display(), e))?;
            }
            std::fs::write(&neuf, &octets).map_err(|e| refus!("{} : {}", neuf.display(), e))?;
            detail.push_str(&format!(". Copie posee : {}", nom));
        }
        e.detail = Some(detail);
    }
    Ok(liste)
}

fn nourrit(e: &EtatSource) -> String {
    if e.nourrit.is_empty() {
        "rien".into()
    } else {
        e.nourrit.join(", ")
    }
}

// ------------------------------------------------------------------ la veille KALI

/// Ce qui, dans le titre d'un texte KALI, fait qu'on le classe « salaires ». C'est un tri d'affichage, pas une regle de
/// droit : les autres mouvements sont montres aussi, en second, avec leur titre, pour que l'oeil tranche.
const MOTS_SALAIRES: [&str; 9] = ["salaire", "minim", "remuneration", "rémunération", "nao", "cachet", "grille", "barème", "bareme"];

fn parle_salaires(titre: &str) -> bool {
    let t = titre.to_lowercase();
    MOTS_SALAIRES.iter().any(|m| t.contains(m))
}

/// Une seule lecture des livraisons pour toutes les sources qui en ont besoin, depuis le releve le plus ancien ; chaque
/// source filtre ensuite sur le sien. `None` : aucune source ne porte de bloc `kali`. `Some(Err)` : la panne.
fn veiller_kali(d: &Path, liste: &[EtatSource], relever: bool, tele: &mut kali::Telecharger) -> Option<R<Rapport>> {
    let mut suivis: Vec<Suivi> = Vec::new();
    let mut depuis: Option<Jour> = None;
    for e in liste {
        let Some(k) = &e.kali else { continue };
        let i = match suivis.iter().position(|s| s.conteneur == k.conteneur) {
            Some(i) => i,
            None => {
                suivis.push(Suivi { conteneur: k.conteneur.clone(), idcc: k.idcc.clone(), textes: Vec::new() });
                suivis.len() - 1
            }
        };
        suivis[i].textes.push(k.texte.clone());
        depuis = Some(match depuis {
            Some(j) => j.min(e.releve),
            None => e.releve,
        });
    }
    let depuis = depuis?;
    Some(kali::veiller(d, &suivis, &depuis, relever, tele))
}

/// Traduit le rapport en etat de CETTE source : ses evenements sont ceux de sa convention, posterieurs a son propre
/// releve.
fn etat_kali(e: &mut EtatSource, rapport: &R<Rapport>) {
    let rapport = match rapport {
        Ok(r) => r,
        Err(m) => {
            e.en_ligne = Some(EnLigne::Injoignable);
            e.detail = Some(format!("livraisons KALI (DILA) : {}", m.phrase));
            return;
        }
    };
    let Some(k) = e.kali.clone() else { return };
    let depuis = e.releve_le.clone();
    let evts: Vec<Evenement> = rapport.evenements.valeur(&k.conteneur).into_iter().flatten().filter(|x| x.livraison() > depuis.as_str()).cloned().collect();
    let lues = rapport.livraisons_lues.iter().filter(|j| **j > depuis).count();
    let ou = format!("IDCC {}, {} livraison(s) KALI lue(s) depuis le {}", k.idcc, lues, depuis);
    let (mut forts, mut faibles) = (Vec::new(), Vec::new());
    for x in &evts {
        match x {
            Evenement::Convention(c) => {
                let titres: Vec<&String> = c.nouveaux.0.iter().map(|(_, t)| t).collect();
                let mut sal: Vec<&String> = titres.iter().copied().filter(|t| parle_salaires(t)).collect();
                sal.sort();
                if !sal.is_empty() {
                    forts.push(format!("{} : texte(s) nouveau(x) attache(s) a la convention -- {}", c.livraison, sal.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(" ; ")));
                }
                let mut autres: Vec<&String> = titres.iter().copied().filter(|t| !parle_salaires(t)).collect();
                autres.sort();
                if !autres.is_empty() {
                    faibles.push(format!("{} : {} texte(s) nouveau(x) -- {}", c.livraison, autres.len(), autres.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(" ; ")));
                }
                if c.premiere {
                    faibles.push(format!("{} : {}", c.livraison, c.titre));
                }
            }
            Evenement::Texte(t) => {
                let mut dit = Vec::new();
                if let Some(d) = &t.texte.date_texte {
                    dit.push(format!("du {}", d));
                }
                if let Some(etat) = &t.texte.etat {
                    dit.push(format!("etat {}", etat));
                }
                if let Some(x) = &t.texte.extension {
                    dit.push(x.clone());
                }
                let titre = t.texte.titre.as_ref().map(|titre| format!(" -- {}", titre)).into_iter().collect::<String>();
                let ligne = format!("{} : texte {} modifie{} ({})", t.livraison, t.ident, titre, dit.join(", "));
                if t.ident == k.texte || t.texte.titre.as_deref().is_some_and(parle_salaires) {
                    forts.push(ligne);
                } else {
                    faibles.push(ligne);
                }
            }
        }
    }
    let mut detail = if !forts.is_empty() {
        e.en_ligne = Some(EnLigne::Changee);
        let mut d = format!("{} : un texte SALAIRES a bouge. {}. Relire, puis mettre a jour : {}", ou, forts.join(" | "), nourrit(e));
        if !faibles.is_empty() {
            d.push_str(&format!(". Autres mouvements : {}", faibles.join(" | ")));
        }
        d
    } else if !faibles.is_empty() {
        e.en_ligne = Some(EnLigne::Inchangee);
        format!("{} : la convention a bouge, mais rien qui parle de salaires d'apres les titres -- {}", ou, faibles.join(" | "))
    } else {
        e.en_ligne = Some(EnLigne::Inchangee);
        format!("{} : rien ne touche la convention", ou)
    };
    match rapport.amorces.valeur(&k.conteneur) {
        Some(am) => detail.push_str(&format!(" (liste de reference : {} textes, fiche du {})", am.textes, am.livraison)),
        None => detail.push_str(" (ATTENTION : aucune fiche de la convention trouvee dans l'index, seul le texte cite par le manifeste est suivi)"),
    }
    if let Some(t) = &rapport.trou {
        detail.push_str(&format!(". ATTENTION, {}", t));
    }
    e.detail = Some(detail);
    e.evenements = evts;
}
