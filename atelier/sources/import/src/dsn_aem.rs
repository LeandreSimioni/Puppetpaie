//! Lire ce que l'ancien logiciel a depose (DSN et AEM), pour ne pas le ressaisir.
//!
//! Ce module LIT et rend des donnees: il n'ecrit rien. Chaque DSN se lit une fois, en sa forme
//! (`declarations::dsn_lue::DsnLue`) ; les AEM par positions fixes (1000 caracteres, latin-1). La fiche qu'une DSN
//! remplit n'est ecrite qu'a la fin (`EntrepriseDeclaree`, `SalarieDeclare::fiche`).

use droit::fiche::Lue as _;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use serde::Serialize;

use declarations::champs::Saisie;

use declarations::dsn_lue::{Adhesion, ContratDeclare, DsnLue, EmploiDeclare, EntrepriseDeclaree, IndividuDeclare, PaieDuContrat};
use socle::date::Jour;
use socle::echec::Echec;
use socle::num::Decimal;
use socle::texte::{blanc, lignes_du_texte, texte_latin1};

// ------------------------------------------------------------------ toutes les DSN d'un dossier

/// Ce qu'une DSN dit d'un salarie.
pub struct SalarieDeclare {
    pub nir: String,
    pub individu: IndividuDeclare,
    /// L'emploi de son dernier contrat de la DSN ; rien quand la DSN ne lui declare aucun contrat.
    pub emploi: Option<EmploiDeclare>,
    pub mois: String,
    pub initiales: String,
    pub premier_contrat: Option<Jour>,
}

impl SalarieDeclare {
    /// La fiche salarie que la DSN remplit : son identite, puis son emploi.
    pub fn fiche(&self) -> socle::echec::R<Saisie> {
        #[derive(Serialize)]
        struct Fiche<'a> {
            #[serde(flatten)]
            individu: &'a IndividuDeclare,
            #[serde(flatten)]
            emploi: &'a Option<EmploiDeclare>,
        }
        crate::proposee::de("salarie", &Fiche { individu: &self.individu, emploi: &self.emploi })
    }
}

/// La fiche Entreprise que la DSN remplit (`champs-base.json`), telle que les fiches la lisent.
pub fn fiche_entreprise(e: &EntrepriseDeclaree) -> socle::echec::R<Saisie> {
    crate::proposee::de("entreprise", e)
}

/// Un numero d'objet spectacle, vu dans une DSN ou dans une AEM.
pub struct ObjetDeclare {
    pub numero: String,
    pub premier_mois: String,
    pub dernier_mois: String,
    pub contrats: i64,
    pub source: &'static str,
}

/// UN CONTRAT QU'UN VERSEMENT DE LA DSN PAIE, avec ce que la DSN lui attribue (`PaieDuContrat` : son brut, son salaire
/// habituel et ses bases par son numero ; ses nets et son prelevement seulement si un versement ne paie que lui) et le
/// contrat que la DSN declare sous ce numero. Ce qu'elle n'attribue a aucun contrat, ce sont les bulletins qui l'apportent.
#[derive(Clone, Debug)]
pub struct PaieDeclaree {
    pub paie: PaieDuContrat,
    pub contrat: Option<ContratDeclare>,
    pub depot: Option<Jour>,
    pub fichier: String,
}

/// Les listes gardent l'ordre ou les cles sont apparues.
#[derive(Default)]
pub struct LectureDsn {
    pub fichiers: usize,
    pub salaries: Vec<SalarieDeclare>,
    pub objets: Vec<ObjetDeclare>,
    pub entreprise: EntrepriseDeclaree,
    pub licences: Vec<(String, String)>,
    pub premier_mois: String,
    pub dernier_mois: String,
    pub illisibles: Vec<String>,
    /// Les etablissements declares (S21.G00.11.001), et le NIC du siege (S21.G00.06.002), d'une DSN a l'autre.
    pub etablissements: BTreeSet<String>,
    pub nics_siege: BTreeSet<String>,
    /// Par (mois, NIR, numero du contrat).
    pub paies: BTreeMap<(String, String, String), PaieDeclaree>,
    /// Le code risque et le taux AT/MP que les contrats declarent (S21.G00.40.040, .043), par annee : ceux de la derniere DSN
    /// de l'annee (son mois, le code, le taux ; aucun taux pour 999ZZ), quand tous ses contrats disent le meme.
    pub atmp: BTreeMap<i32, (String, String, Option<Decimal>)>,
    /// Les adhesions prevoyance de la derniere DSN qui en declare.
    pub adhesions: Vec<Adhesion>,
    /// Les conventions collectives que chaque DSN declare, dans l'ordre des mois (`conventions_divergentes`).
    pub(crate) conventions: Vec<ConventionsDuMois>,
}

/// LES CONVENTIONS COLLECTIVES QU'UNE DSN DECLARE : celle de l'etablissement (S21.G00.11.022 ; a defaut, celle du premier
/// contrat : `declarations::dsn_lue`, `EntrepriseDeclaree`), et celles de ses contrats (S21.G00.40.017). Les deux
/// rubriques sont obligatoires en mensuelle (`norme-dsn/texte/tableaux-des-usages.txt`).
#[derive(Clone, Debug)]
pub(crate) struct ConventionsDuMois {
    pub(crate) mois: String,
    pub(crate) etablissement: Option<String>,
    pub(crate) contrats: BTreeSet<String>,
}

impl LectureDsn {
    /// Aucune DSN lue : le dossier n'en a pas.
    pub fn aucune() -> LectureDsn {
        LectureDsn::default()
    }

    /// LES CONVENTIONS QUI DIVERGENT, dites : des contrats d'une meme DSN qui declarent d'autres conventions que
    /// l'etablissement ou entre eux, une convention de l'etablissement qui change d'une DSN a l'autre. La fiche
    /// Entreprise n'en porte qu'une, celle de la DSN la plus recente (`EntrepriseDeclaree::suivre`) : l'ecart se dit, rien
    /// ne se choisit en silence -- une convention etendue oblige tout employeur de son champ (C. trav. L2261-15,
    /// `atelier/reference/sources/conventions-champ-d-application/LISEZ-MOI.md`).
    pub fn conventions_divergentes(&self) -> Vec<String> {
        let mut out = Vec::new();
        for c in &self.conventions {
            let autres: Vec<&str> = c.contrats.iter().map(String::as_str).filter(|x| Some(*x) != c.etablissement.as_deref()).collect();
            if autres.is_empty() {
                continue;
            }
            let etablissement: String = c.etablissement.iter().map(|e| format!("l'établissement relève de la convention {} (S21.G00.11.022), ", e)).collect();
            out.push(format!(
                "la DSN de {} déclare plusieurs conventions collectives : {}des contrats de la convention {} (S21.G00.40.017) ; la fiche Entreprise n'en porte qu'une, à vérifier",
                socle::mois::Mois::en_clair(&c.mois),
                etablissement,
                autres.join(", ")
            ));
        }
        let mut avant: Option<(&str, &str)> = None;
        for c in &self.conventions {
            let Some(i) = c.etablissement.as_deref() else { continue };
            if let Some((a, m)) = avant.filter(|(a, _)| *a != i) {
                out.push(format!(
                    "la convention collective déclarée change d'une DSN à l'autre : {} dans la DSN de {}, {} dans celle de {} ; la fiche Entreprise prend la plus récente, à vérifier",
                    a,
                    socle::mois::Mois::en_clair(m),
                    i,
                    socle::mois::Mois::en_clair(&c.mois)
                ));
            }
            avant = Some((i, &c.mois));
        }
        out
    }
}

/// Les noms du dossier, tries. Un dossier ou une entree qui ne se lisent pas se refusent.
fn noms_tries(dossier: &Path) -> Result<Vec<String>, Echec> {
    let disque = |e: std::io::Error| socle::echec::du_disque(dossier, &e).reclasser(socle::echec::Classe::Disque);
    let mut v: Vec<String> = Vec::new();
    for e in std::fs::read_dir(dossier).map_err(disque)? {
        v.push(e.map_err(disque)?.file_name().to_string_lossy().into_owned());
    }
    v.sort();
    Ok(v)
}

/// Le nom d'un fichier, pour le dire.
fn nom_de(f: &Path) -> String {
    match f.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => f.display().to_string(),
    }
}

/// Toutes les DSN d'un dossier, la plus recente faisant foi par salarie. Une DSN fautive refuse
/// (`declarations::dsn_lue::lire`).
pub fn lire_dsn(dossier: &Path) -> Result<LectureDsn, Echec> {
    let mut out = LectureDsn::default();
    let mut lues: Vec<(socle::mois::Mois, DsnLue)> = Vec::new();
    for f in declarations::dsn_lue::fichiers(dossier)? {
        // Une DSN sans mois principal (S20.G00.05.005) ne dit pas quel mois elle declare : elle est ignoree, en le disant.
        match declarations::dsn_lue::lire_une_dsn(&f)?.and_then(|i| Some((i.mois?, i))) {
            Some(lue) => lues.push(lue),
            _ => out.illisibles.push(nom_de(&f)),
        }
    }
    // Par mois declare, puis par date de depot : un annule-et-remplace depose apres l'original fait foi.
    lues.sort_by(|(ma, a), (mb, b)| (ma, a.depot, &a.fichier).cmp(&(mb, b.depot, &b.fichier)));
    out.fichiers = lues.len();
    if let (Some(p), Some(d)) = (lues.first(), lues.last()) {
        out.premier_mois = p.0.to_string();
        out.dernier_mois = d.0.to_string();
    }
    let mut index_salaries: HashMap<String, usize> = HashMap::new();
    let mut index_objets: HashMap<String, usize> = HashMap::new();
    for (le_mois, info) in &lues {
        let mois = le_mois.to_string();
        out.etablissements.extend(info.etablissements.iter().cloned());
        out.nics_siege.extend(info.nic_siege.clone());
        out.entreprise.suivre(&info.entreprise);
        out.conventions.push(ConventionsDuMois {
            mois: mois.clone(),
            etablissement: info.entreprise.idcc.clone(),
            contrats: info.contrats.iter().filter_map(|c| c.idcc.clone()).collect(),
        });
        if let Some(l) = &info.entreprise.licence {
            if out.licences.last().is_none_or(|x| x.1 != *l) {
                out.licences.push((mois.clone(), l.clone()));
            }
        }
        // Le code risque de la DSN : un seul pour tous ses contrats, sinon rien (on ne choisit pas).
        let risques: BTreeSet<(String, Option<Decimal>)> =
            info.contrats.iter().filter_map(|c| c.code_risque.clone().map(|r| (r, c.taux_at))).collect();
        if risques.len() == 1 {
            if let Some((code, taux)) = risques.into_iter().next() {
                out.atmp.insert(le_mois.annee(), (mois.clone(), code, taux));
            }
        }
        if !info.adhesions.is_empty() {
            out.adhesions = info.adhesions.clone();
        }
        let mut dernier_contrat: HashMap<&str, &ContratDeclare> = HashMap::new();
        for c in &info.contrats {
            dernier_contrat.insert(c.nir.as_str(), c);
            if let Some(objet) = &c.objet {
                let i = *index_objets.entry(objet.clone()).or_insert_with(|| {
                    out.objets.push(ObjetDeclare {
                        numero: objet.clone(),
                        premier_mois: mois.clone(),
                        dernier_mois: mois.clone(),
                        contrats: 0,
                        source: "DSN",
                    });
                    out.objets.len() - 1
                });
                out.objets[i].dernier_mois = mois.clone();
                out.objets[i].contrats += 1;
            }
        }
        for paie in info.paies() {
            let contrat = info.contrats_de(&paie.nir).filter(|x| x.numero.as_deref() == Some(paie.numero.as_str())).last().cloned();
            let cle = (mois.clone(), paie.nir.clone(), paie.numero.clone());
            out.paies.insert(cle, PaieDeclaree { paie, contrat, depot: info.depot, fichier: info.fichier.clone() });
        }
        for ind in &info.individus {
            let nir = ind.nir.clone();
            let c = dernier_contrat.get(nir.as_str()).copied();
            // Les initiales du numero de son dernier contrat (avant le premier point) ; aucune sans numero.
            let initiales: String =
                c.and_then(|c| c.numero.as_deref()).and_then(|n| n.split('.').next()).into_iter().flat_map(str::chars).filter(|ch| ch.is_alphabetic()).take(2).collect();
            let avant = index_salaries.get(&nir).and_then(|&i| out.salaries[i].premier_contrat);
            let premier = info.contrats.iter().filter(|x| x.nir == nir).filter_map(|x| x.debut).chain(avant).min();
            let decl = SalarieDeclare {
                nir: nir.clone(),
                individu: ind.clone(),
                emploi: c.map(|c| c.emploi.du_salarie()).transpose()?,
                mois: mois.clone(),
                initiales,
                premier_contrat: premier,
            };
            match index_salaries.get(&nir) {
                Some(&i) => out.salaries[i] = decl,
                None => {
                    out.salaries.push(decl);
                    index_salaries.insert(nir, out.salaries.len() - 1);
                }
            }
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ les AEM

/// Nom -> (debut, fin), en caracteres.
/// `ligne[a:b]` : une ligne plus courte rend ce qu'elle a.
fn tranche(ligne: &[char], a: usize, b: usize) -> String {
    ligne.iter().skip(a).take(b.saturating_sub(a)).collect()
}

/// `19900723` -> `1990-07-23` (les dates de l'AEM), le reste tel quel.
fn aaaammjj(v: &str) -> String {
    let c: Vec<char> = v.chars().collect();
    // Des chiffres : les chiffres ASCII et les exposants de Latin-1 (qu'une AEM lue en latin-1 peut porter).
    if c.len() == 8 && c.iter().all(|x| x.is_ascii_digit() || matches!(x, '\u{b2}' | '\u{b3}' | '\u{b9}')) {
        return format!("{}-{}-{}", c[0..4].iter().collect::<String>(), c[4..6].iter().collect::<String>(), c[6..8].iter().collect::<String>());
    }
    v.to_string()
}

fn champ(ligne: &[char], a: usize, b: usize) -> String {
    ligne[a..b].iter().collect()
}

/// Un enregistrement AEM, un contrat tel qu'il a ete atteste.
pub struct ContratAtteste {
    pub nir: String,
    pub debut: socle::date::Jour,
    pub fin: socle::date::Jour,
    pub numero_objet: String,
    pub(crate) heures: i128,
    /// Les cachets, et les cachets « groupes » (la case que le gabarit garde a 00).
    pub(crate) cachets: i128,
    pub(crate) jours: i128,
    /// Le brut en euros, deux décimales.
    pub(crate) brut: socle::num::Decimal,
    /// Le numero de l'attestation (zone 36..42), et sa date de signature (858..866, AAAA-MM-JJ si c'en est une).
    pub(crate) numero_attestation: String,
    pub(crate) date_attestation: String,
    pub fichier: String,
}

/// Le contrat atteste, tel qu'un engagement importe le porte (`salarie` : la cle de sa fiche).
pub fn ligne_du_contrat(c: &ContratAtteste, salarie: &str) -> engagement::importe::ContratLu {
    use engagement::importe::{Attestation, ContratAtteste as Atteste, ContratLu};
    let plein = |s: &str| Some(s.to_string()).filter(|s| !s.trim().is_empty());
    let mut a = Atteste::vide(Attestation::Aem);
    a.numero_objet = plein(&c.numero_objet);
    (a.heures, a.cachets, a.jours) = (Some(c.heures as i64), Some(c.cachets as i64), Some(c.jours as i64));
    a.brut = Some(c.brut);
    a.numero_attestation = plein(&c.numero_attestation);
    a.atteste_le = plein(&c.date_attestation);
    a.fichier = plein(&c.fichier);
    ContratLu { salarie: salarie.to_string(), debut: Some(c.debut.iso()), fin: Some(c.fin.iso()), numero: None, contrat: a }
}

/// Tous les contrats attestes d'un dossier AEM_EDI, dedoublonnes, et les
/// fichiers ou enregistrements illisibles. Un `CalculImpossible` (un brut qui n'est pas un nombre)
/// n'est pas rattrape.
pub fn lire_aem(dossier: &Path) -> Result<(Vec<ContratAtteste>, Vec<String>), socle::echec::Echec> {
    use socle::num::{decimal_lu, entier_lu};
    let mut vus: Vec<(Vec<String>, ContratAtteste)> = Vec::new();
    let mut connus: std::collections::HashSet<Vec<String>> = std::collections::HashSet::new();
    let mut illisibles = Vec::new();
    // `glob.glob(".../*.txt")` : pas de fichier cache, ordre des chaines.
    let noms: Vec<String> = noms_tries(dossier)?.into_iter().filter(|n| !n.starts_with('.') && n.ends_with(".txt")).collect();
    for nom in noms {
        let f = dossier.join(&nom);
        let octets = match std::fs::read(&f) {
            Ok(o) => o,
            Err(e) => {
                illisibles.push(format!("{} : {}", nom, e));
                continue;
            }
        };
        for ligne in lignes_du_texte(&texte_latin1(&octets)) {
            let l: Vec<char> = ligne.chars().collect();
            if l.len() < 700 {
                continue;
            }
            let illisible = |illisibles: &mut Vec<String>| illisibles.push(format!("{} : un enregistrement illisible", nom));
            let nir: String = champ(&l, 375, 390).chars().take(13).collect();
            let (Some(debut), Some(fin)) =
                (socle::date::lire_aaaammjj(&champ(&l, 618, 626)), socle::date::lire_aaaammjj(&champ(&l, 627, 635)))
            else {
                illisible(&mut illisibles);
                continue;
            };
            let numero_objet = champ(&l, 638, 650).trim_matches(blanc).to_string();
            let entiers = [(650, 653), (653, 655), (655, 657), (657, 659)].map(|(a, b)| entier_lu(&champ(&l, a, b)));
            let [Some(heures), Some(c1), Some(c2), Some(jours)] = entiers else {
                illisible(&mut illisibles);
                continue;
            };
            let brut_texte = champ(&l, 659, 668);
            let Some(brut_cts) = decimal_lu(&brut_texte) else {
                return Err(socle::echec::Echec { classe: Some(socle::echec::Classe::CalculImpossible), phrase: socle::num::illisible(&brut_texte), a_part: None });
            };
            let mut brut = brut_cts / socle::num::Decimal::from(100);
            brut = brut.round_dp(2);
            brut.rescale(2);
            let cle = vec![
                nir.clone(),
                debut.iso(),
                fin.iso(),
                numero_objet.clone(),
                heures.to_string(),
                (c1 + c2).to_string(),
                jours.to_string(),
                brut.to_string(),
            ];
            if connus.insert(cle.clone()) {
                vus.push((cle, ContratAtteste {
                    nir,
                    debut,
                    fin,
                    numero_objet,
                    heures,
                    cachets: c1 + c2,
                    jours,
                    brut,
                    numero_attestation: tranche(&l, 36, 42).trim_matches(blanc).to_string(),
                    date_attestation: aaaammjj(&tranche(&l, 858, 866)),
                    fichier: nom.clone(),
                }));
            }
        }
    }
    let mut contrats: Vec<ContratAtteste> = vus.into_iter().map(|(_, c)| c).collect();
    contrats.sort_by(|a, b| (a.debut, a.fin, &a.nir).cmp(&(b.debut, b.fin, &b.nir)));
    Ok((contrats, illisibles))
}

// ------------------------------------------------------------------ l'entreprise de chaque piece

/// Ou l'AEM porte le Siret de l'employeur : `ligne[50:64]`, 14 chiffres. `CHAMPS` ne
/// nomme pas cette zone (elle est figee dans le gabarit, comme tout ce que l'AEM ne change pas d'un contrat a
/// l'autre) : elle a ete relevee sur 190 enregistrements AEM produits par CoolPaie de 2021 a 2026, ou le Siret de la
/// fiche de la compagnie se lit a cette place dans tous. Un Siret ne se tient pour lu que s'il a ses 14 chiffres et sa
/// cle de Luhn ; sinon l'AEM passe, avec un avertissement nomme (`AEM_SANS_SIRET`) -- on ne refuse pas au hasard.
pub(crate) const AEM_SIRET: (usize, usize) = (50, 64);

/// Ce qu'une piece (DSN, AEM) dit de son employeur.
#[derive(Default, Clone)]
pub struct Employeur {
    /// Les Siren qu'elle porte (un, d'ordinaire ; une AEM en porte un par enregistrement).
    pub sirens: std::collections::BTreeSet<String>,
    /// Les enregistrements (AEM) ou l'identifiant de l'employeur ne se lit pas.
    pub(crate) illisibles: usize,
}

/// Une DSN : le Siren de l'ENTREPRISE (`S21.G00.06.001`), pas celui de l'emetteur (`S10.G00.01.001`), qui est celui
/// du cabinet quand un tiers depose (`EntrepriseDeclaree`). Une DSN fautive refuse (`declarations::dsn_lue::lire`).
pub fn employeur_dsn(chemin: &Path) -> Result<Employeur, Echec> {
    let siren = declarations::dsn_lue::lire_une_dsn(chemin)?.and_then(|d| d.entreprise.siren);
    Ok(Employeur { sirens: siren.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(), illisibles: 0 })
}

/// Une AEM : le Siret de chaque enregistrement (`AEM_SIRET`), ramene a son Siren. Un fichier que le tri a lu et qui ne
/// se lit plus se refuse.
pub fn employeur_aem(chemin: &Path) -> Result<Employeur, Echec> {
    let mut e = Employeur::default();
    let octets = std::fs::read(chemin).map_err(|x| socle::echec::du_disque(chemin, &x).reclasser(socle::echec::Classe::Disque))?;
    for ligne in lignes_du_texte(&texte_latin1(&octets)) {
        let l: Vec<char> = ligne.chars().collect();
        // Les enregistrements que `lire_aem` lit.
        if l.len() < 700 {
            continue;
        }
        let s = tranche(&l, AEM_SIRET.0, AEM_SIRET.1);
        if s.len() == 14 && s.chars().all(|c| c.is_ascii_digit()) && declarations::champs::luhn(&s) {
            e.sirens.insert(s[..9].to_string());
        } else {
            e.illisibles += 1;
        }
    }
    Ok(e)
}

/// LE SIREN DE LA COMPAGNIE, pour tout ce que l'import lit (DSN, AEM, bulletins) : celui de la fiche Entreprise ;
/// sinon celui des DSN, si elles n'en portent qu'un (plusieurs : aucune reference) ; sinon, sans DSN qui le dise, celui
/// des autres pieces (bulletins, AEM) si elles n'en portent qu'un. `None` : rien ne dit laquelle est la compagnie.
pub fn siren_de_reference(
    fiche: &droit::fiche::ChampsEntreprise,
    dsn: &std::collections::BTreeSet<String>,
    autres: &std::collections::BTreeSet<String>,
) -> Option<String> {
    if let Some(s) = fiche.siren.portee() {
        return Some(s.trim().to_string());
    }
    let seul = |e: &std::collections::BTreeSet<String>| (e.len() == 1).then(|| e.iter().next().cloned()).flatten();
    if !dsn.is_empty() {
        return seul(dsn);
    }
    seul(autres)
}

/// Une piece dont l'entreprise ne se rapporte a rien : le dossier porte plusieurs entreprises, aucune ne se tranche.
pub const PLUSIEURS_ENTREPRISES: &str = "le dossier porte plusieurs entreprises (Siren des DSN, Siret des bulletins ou des AEM), et ni la fiche Entreprise ni une seule DSN ne dit laquelle est la compagnie : non repris";
pub(crate) const DSN_AUTRE: &str = "DSN d'une autre entreprise : son Siren n'est pas celui de la compagnie, non reprise";
pub(crate) const DSN_SANS_SIREN: &str = "DSN sans Siren lisible (S21.G00.06.001) : rien ne dit de quelle entreprise elle vient, non reprise";
pub(crate) const AEM_AUTRE: &str = "AEM d'une autre entreprise : son Siret n'est pas celui de la compagnie, non reprise";
pub(crate) const AEM_SANS_SIRET: &str =
    "son Siret ne se lit pas (caractères 51 à 64 de l'enregistrement) : l'entreprise n'est pas contrôlée";

/// Le sort d'une DSN ou d'une AEM devant la reference (`verdict_entreprise`) : ecartee, avec sa raison -- elle n'est pas
/// reprise et RIEN n'en est lu --, reprise avec un avertissement -- son entreprise ne se controle pas --, ou reprise.
pub enum Verdict {
    Ecartee(&'static str),
    Avertie(&'static str),
    Reprise,
}

/// Le sort d'une DSN (`aem` faux) ou d'une AEM devant la reference.
pub fn verdict_entreprise(aem: bool, e: &Employeur, reference: Option<&str>) -> Verdict {
    let lisible = if e.illisibles > 0 { Verdict::Avertie(AEM_SANS_SIRET) } else { Verdict::Reprise };
    if e.sirens.is_empty() {
        // Une AEM sans enregistrement lisible ne rend rien a `lire_aem` : rien a controler.
        return if aem { lisible } else { Verdict::Ecartee(DSN_SANS_SIREN) };
    }
    let Some(r) = reference else { return Verdict::Ecartee(PLUSIEURS_ENTREPRISES) };
    if e.sirens.iter().any(|s| s != r) {
        return Verdict::Ecartee(if aem { AEM_AUTRE } else { DSN_AUTRE });
    }
    lisible
}


#[cfg(test)]
mod tests_parametrage {
    /// Ce que l'import lit d'une DSN pour la configuration : le contact chez le declare, « nom et prenom » en une rubrique
    /// (S20.G00.07.001 a .004, `norme-dsn/texte/bloc-S20.G00.07.txt` l. 63-84), sa civilite par le contact de l'emetteur
    /// quand l'emetteur est l'entreprise et le nom le meme (S10.G00.02.001-002) ; le code risque et le taux AT/MP du contrat
    /// (S21.G00.40.040, .043, `bloc-S21.G00.40.txt` l. 1224-1315) ; les adhesions prevoyance (S21.G00.15.001, .002, .005).
    #[test]
    fn la_dsn_dit_son_contact_son_code_risque_et_ses_adhesions() {
        let d = std::env::temp_dir().join(format!("puppetpaie-dsn-parametrage-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("essai.dsn");
        let lignes = [
            "S10.G00.00.001,'PuppetPaie'", "S10.G00.01.001,'123456873'", "S10.G00.01.003,'COMPAGNIE DU FIL DE FER'",
            "S10.G00.02.001,'02'", "S10.G00.02.002,'FONTAINE CLAIRE'",
            "S20.G00.05.005,'01122025'", "S20.G00.07.001,'FONTAINE CLAIRE'", "S20.G00.07.002,'0478000000'",
            "S20.G00.07.003,'contact@exemple.invalid'", "S20.G00.07.004,'04'",
            "S21.G00.06.001,'123456873'", "S21.G00.11.001,'00017'",
            "S21.G00.15.001,'PREV SPECT VIVANT PRIV'", "S21.G00.15.002,'P0983'", "S21.G00.15.005,'104'",
            "S21.G00.30.001,'2930899999999'", "S21.G00.40.001,'01012025'", "S21.G00.40.040,'923AD'", "S21.G00.40.043,'1.65'",
        ];
        std::fs::write(&f, lignes.join("\n")).unwrap();
        let lu = super::lire_dsn(&d).unwrap();
        std::fs::remove_dir_all(&d).ok();
        let c = lu.entreprise.declarant_dsn.clone().unwrap_or_default();
        assert_eq!((c.nom_prenom.as_str(), c.tel.as_deref(), c.civilite.as_deref()), ("FONTAINE CLAIRE", Some("0478000000"), Some("02")));
        assert_eq!(lu.atmp.get(&2025), Some(&("2025-12".to_string(), "923AD".to_string(), socle::num::decimal_lu("1.65"))));
        assert_eq!(lu.adhesions[0].libelle, "PREV SPECT VIVANT PRIV");
        assert_eq!(lu.adhesions[0].organisme.as_deref(), Some("P0983"));
    }
}
